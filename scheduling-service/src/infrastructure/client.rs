use async_trait::async_trait;
use opentelemetry::global;
use opentelemetry::propagation::Injector;
use reqwest::{Client, header};
use shared::{responses::ApiResponse, types::Staff};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use uuid::Uuid;

use crate::{domain::client::DataServiceClient, error::SchedulingServiceError};

pub struct HttpDataServiceClient {
    client: Client,
    base_url: String,
}

impl HttpDataServiceClient {
    pub fn new(base_url: String) -> Self {
        let client = Client::new();
        Self { client, base_url }
    }
}

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
        let url = format!(
            "{}/api/v1/groups/{staff_group_id}/resolved-members",
            self.base_url
        );

        let mut headers = header::HeaderMap::new();
        let cx = tracing::Span::current().context();
        global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut HeaderMapInjector(&mut headers));
        });

        tracing::debug!(%url, "Requesting resolved members");

        let res = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| {
                SchedulingServiceError::DataService(format!("Failed to reach Data Service:{e}"))
            })?;

        tracing::debug!(status = %res.status(), "Data service responded");

        if !res.status().is_success() {
            return Err(SchedulingServiceError::DataService(format!(
                "Data Service returned status {}",
                res.status()
            )));
        }

        let api_response = res.json::<ApiResponse<Vec<Staff>>>().await.map_err(|e| {
            SchedulingServiceError::DataService(format!("Failed to deserialize response: {e}"))
        })?;

        api_response
            .data
            .ok_or_else(|| SchedulingServiceError::DataService("No data in response".to_string()))
    }
}
