use std::time::Duration;

use async_trait::async_trait;
use opentelemetry::global;
use opentelemetry::propagation::Injector;
use reqwest::{Client, header};
use shared::{responses::ApiResponse, types::Staff};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use crate::{domain::client::DataServiceClient, error::SchedulingServiceError};

/// HTTP client for the data-service, with retry logic and OpenTelemetry trace propagation.
pub struct HttpDataServiceClient {
    client: Client,
    base_url: String,
}

/// Maximum number of retry attempts for transient HTTP failures.
const MAX_RETRIES: u32 = 3;
/// Per-request timeout applied to the underlying HTTP client.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

impl HttpDataServiceClient {
    /// Builds an HTTP client with the configured timeout.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be built (invalid TLS configuration).
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("Failed to build HTTP client");
        Self { client, base_url }
    }
}

/// Adapter to inject OpenTelemetry trace context into HTTP request headers.
struct HeaderMapInjector<'a>(&'a mut header::HeaderMap);

impl Injector for HeaderMapInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(name) = header::HeaderName::from_bytes(key.as_bytes())
            && let Ok(val) = header::HeaderValue::from_str(&value)
        {
            self.0.insert(name, val);
        }
    }
}

#[async_trait]
impl DataServiceClient for HttpDataServiceClient {
    #[tracing::instrument(skip(self))]
    async fn get_resolved_members(
        &self,
        staff_group_id: Uuid,
    ) -> Result<Vec<Staff>, SchedulingServiceError> {
        let base_url = &self.base_url;
        let url = format!("{base_url}/api/v1/groups/{staff_group_id}/resolved-members");

        tracing::debug!(%url, "Requesting resolved members");

        let mut last_err = None;

        for attempt in 1..=MAX_RETRIES {
            let mut headers = header::HeaderMap::new();
            let cx = tracing::Span::current().context();
            global::get_text_map_propagator(|propagator| {
                propagator.inject_context(&cx, &mut HeaderMapInjector(&mut headers));
            });

            match self.client.get(&url).headers(headers).send().await {
                Ok(res) => {
                    tracing::debug!(status = %res.status(), attempt, "Data service responded");

                    if !res.status().is_success() {
                        return Err(SchedulingServiceError::DataService(format!(
                            "Data Service returned status {}",
                            res.status()
                        )));
                    }

                    let api_response =
                        res.json::<ApiResponse<Vec<Staff>>>().await.map_err(|e| {
                            SchedulingServiceError::DataService(format!(
                                "Failed to deserialize response: {e}"
                            ))
                        })?;

                    return api_response.data.ok_or_else(|| {
                        SchedulingServiceError::DataService("No data in response".into())
                    });
                }
                Err(e) => {
                    tracing::warn!(
                        attempt,
                        max_retries = MAX_RETRIES,
                        error = %e,
                        "Request to Data Service failed, retrying"
                    );
                    last_err = Some(e);
                    if attempt < MAX_RETRIES {
                        tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(attempt - 1)))
                            .await;
                    }
                }
            }
        }

        let last_err = last_err.expect("at least one error occurred");
        Err(SchedulingServiceError::DataServiceUnavailable(format!(
            "Failed to reach Data Service after {MAX_RETRIES} attempts: {last_err}"
        )))
    }
}
