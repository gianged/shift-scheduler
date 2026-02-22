use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::domain::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::domain::service::SchedulingService;

/// Serializable health check settings, typically loaded from the scheduling config file.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct HealthCheckSettings {
    pub interval_secs: u64,
    pub timeout_secs: u64,
}

impl Default for HealthCheckSettings {
    fn default() -> Self {
        Self {
            interval_secs: 10,
            timeout_secs: 5,
        }
    }
}

/// Resolved health check configuration with concrete `Duration` values and the full endpoint URL.
pub struct HealthCheckConfig {
    pub interval: Duration,
    pub health_endpoint: String,
    pub timeout: Duration,
}

impl HealthCheckConfig {
    /// Converts serializable settings into a resolved config using the data service base URL.
    pub fn from_settings(settings: &HealthCheckSettings, data_service_url: &str) -> Self {
        Self {
            interval: Duration::from_secs(settings.interval_secs),
            health_endpoint: format!("{data_service_url}/headpat"),
            timeout: Duration::from_secs(settings.timeout_secs),
        }
    }
}

/// Spawns a periodic health check task that pings the data service.
///
/// When the data service recovers after an outage, the health check force-closes the
/// circuit breaker and triggers a retry of all `WaitingForRetry` jobs.
///
/// # Panics
///
/// Panics if the HTTP client cannot be built.
pub fn spawn_health_check(
    config: HealthCheckConfig,
    breaker: Arc<Mutex<CircuitBreaker>>,
    scheduling_service: Arc<SchedulingService>,
    task_tracker: &TaskTracker,
    cancel_token: CancellationToken,
) {
    let client = Client::builder()
        .timeout(config.timeout)
        .build()
        .expect("Failed to build health check HTTP client");

    let retrying = Arc::new(AtomicBool::new(false));

    tracing::info!(
        endpoint = %config.health_endpoint,
        interval_secs = config.interval.as_secs(),
        "Starting data service health check"
    );

    task_tracker.spawn(async move {
        let mut interval = tokio::time::interval(config.interval);

        loop {
            tokio::select! {
                () = cancel_token.cancelled() => {
                    tracing::info!("Health check task shutting down");
                    break;
                }
                _ = interval.tick() => {
                    check_health(
                        &client,
                        &config.health_endpoint,
                        &breaker,
                        &scheduling_service,
                        &retrying,
                    ).await;
                }
            }
        }
    });
}

async fn check_health(
    client: &Client,
    endpoint: &str,
    breaker: &Arc<Mutex<CircuitBreaker>>,
    scheduling_service: &Arc<SchedulingService>,
    retrying: &Arc<AtomicBool>,
) {
    match client.get(endpoint).send().await {
        Ok(res) if res.status().is_success() => {
            tracing::debug!("Data service health check passed");

            let was_not_closed = {
                let mut b = breaker.lock().await;
                let was = b.state() != CircuitState::Closed;
                b.force_close();
                was
            };

            if was_not_closed && !retrying.load(Ordering::SeqCst) {
                retrying.store(true, Ordering::SeqCst);
                tracing::info!("Data service recovered, retrying waiting jobs");

                if let Err(e) = scheduling_service.retry_waiting_jobs().await {
                    tracing::error!("Failed to retry waiting jobs: {e}");
                }

                retrying.store(false, Ordering::SeqCst);
            }
        }
        Ok(res) => {
            tracing::warn!(
                status = %res.status(),
                "Data service health check returned non-success"
            );
            breaker.lock().await.record_failure();
        }
        Err(e) => {
            tracing::warn!(error = %e, "Data service health check failed");
            breaker.lock().await.record_failure();
        }
    }
}
