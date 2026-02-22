use std::sync::Arc;

use async_trait::async_trait;
use shared::types::Staff;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::domain::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::domain::client::DataServiceClient;
use crate::error::SchedulingServiceError;

/// Decorator around a [`DataServiceClient`] that checks the circuit breaker before
/// delegating calls and records successes/failures.
pub struct CircuitBreakerClient {
    inner: Arc<dyn DataServiceClient>,
    breaker: Arc<Mutex<CircuitBreaker>>,
}

impl CircuitBreakerClient {
    /// Creates a new circuit-breaker-wrapped client. Returns both the client and a
    /// shared handle to the breaker (used by the health check to force-close it).
    pub fn new(
        inner: Arc<dyn DataServiceClient>,
        config: CircuitBreakerConfig,
    ) -> (Self, Arc<Mutex<CircuitBreaker>>) {
        let breaker = Arc::new(Mutex::new(CircuitBreaker::new(config)));
        let client = Self {
            inner,
            breaker: Arc::clone(&breaker),
        };
        (client, breaker)
    }
}

#[async_trait]
impl DataServiceClient for CircuitBreakerClient {
    #[tracing::instrument(skip(self))]
    async fn get_resolved_members(
        &self,
        staff_group_id: Uuid,
    ) -> Result<Vec<Staff>, SchedulingServiceError> {
        {
            let mut breaker = self.breaker.lock().await;
            if !breaker.can_execute() {
                tracing::warn!("Circuit breaker is open, fast-failing request");
                return Err(SchedulingServiceError::CircuitOpen);
            }
        }

        match self.inner.get_resolved_members(staff_group_id).await {
            Ok(result) => {
                self.breaker.lock().await.record_success();
                Ok(result)
            }
            Err(e) => {
                let mut breaker = self.breaker.lock().await;
                breaker.record_failure();
                let state = breaker.state();
                drop(breaker);

                tracing::warn!(
                    circuit_state = ?state,
                    "Data service call failed, circuit breaker recorded failure"
                );
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::circuit_breaker::CircuitState;
    use crate::domain::client::MockDataServiceClient;

    fn make_config(threshold: u32) -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: threshold,
            cooldown_secs: 100,
        }
    }

    #[tokio::test]
    async fn delegates_to_inner_when_closed() {
        let mut mock = MockDataServiceClient::new();
        mock.expect_get_resolved_members().returning(|_| Ok(vec![]));

        let (client, _breaker) = CircuitBreakerClient::new(Arc::new(mock), make_config(3));
        let result = client.get_resolved_members(Uuid::new_v4()).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn returns_circuit_open_when_open() {
        let mut mock = MockDataServiceClient::new();
        mock.expect_get_resolved_members().returning(|_| {
            Err(SchedulingServiceError::DataServiceUnavailable(
                "connection refused".into(),
            ))
        });

        let (client, _breaker) = CircuitBreakerClient::new(Arc::new(mock), make_config(2));

        // Trigger 2 failures to open the circuit
        let _ = client.get_resolved_members(Uuid::new_v4()).await;
        let _ = client.get_resolved_members(Uuid::new_v4()).await;

        // Third call should be fast-failed by circuit breaker
        let result = client.get_resolved_members(Uuid::new_v4()).await;
        assert!(matches!(
            result.unwrap_err(),
            SchedulingServiceError::CircuitOpen
        ));
    }

    #[tokio::test]
    async fn records_success_and_keeps_closed() {
        let mut mock = MockDataServiceClient::new();
        mock.expect_get_resolved_members().returning(|_| Ok(vec![]));

        let (client, breaker) = CircuitBreakerClient::new(Arc::new(mock), make_config(3));
        let _ = client.get_resolved_members(Uuid::new_v4()).await;

        let state = breaker.lock().await.state();
        assert_eq!(state, CircuitState::Closed);
    }

    #[tokio::test]
    async fn records_failure_and_opens_at_threshold() {
        let mut mock = MockDataServiceClient::new();
        mock.expect_get_resolved_members().returning(|_| {
            Err(SchedulingServiceError::DataServiceUnavailable(
                "timeout".into(),
            ))
        });

        let (client, breaker) = CircuitBreakerClient::new(Arc::new(mock), make_config(2));
        let _ = client.get_resolved_members(Uuid::new_v4()).await;
        assert_eq!(breaker.lock().await.state(), CircuitState::Closed);

        let _ = client.get_resolved_members(Uuid::new_v4()).await;
        assert_eq!(breaker.lock().await.state(), CircuitState::Open);
    }
}
