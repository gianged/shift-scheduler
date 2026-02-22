use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

/// RAII guard that shuts down the OpenTelemetry tracer provider on drop.
pub struct TelemetryGuard {
    provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take()
            && let Err(e) = provider.shutdown()
        {
            eprintln!("Failed to shutdown tracer provider: {e}");
        }
    }
}

/// Initializes tracing with optional JSON formatting and optional OTLP export.
///
/// Configuration is driven by environment variables:
/// - `RUST_LOG` / `LOG_FORMAT` for log filtering and formatting
/// - `OTEL_EXPORTER_OTLP_ENDPOINT` to enable trace export
///
/// Returns a guard that must be held for the lifetime of the application.
pub fn init_telemetry(service_name: &str) -> TelemetryGuard {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let log_format = std::env::var("LOG_FORMAT").unwrap_or_default();
    let otel_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();

    let registry = Registry::default().with(env_filter);

    match (log_format.as_str(), otel_endpoint) {
        ("json", Some(endpoint)) => {
            let fmt_layer = tracing_subscriber::fmt::layer().json().flatten_event(true);
            let (otel_layer, provider) = build_otel_layer(service_name, &endpoint);
            registry.with(fmt_layer).with(otel_layer).init();
            TelemetryGuard {
                provider: Some(provider),
            }
        }
        ("json", None) => {
            let fmt_layer = tracing_subscriber::fmt::layer().json().flatten_event(true);
            registry.with(fmt_layer).init();
            TelemetryGuard { provider: None }
        }
        (_, Some(endpoint)) => {
            let fmt_layer = tracing_subscriber::fmt::layer();
            let (otel_layer, provider) = build_otel_layer(service_name, &endpoint);
            registry.with(fmt_layer).with(otel_layer).init();
            TelemetryGuard {
                provider: Some(provider),
            }
        }
        _ => {
            let fmt_layer = tracing_subscriber::fmt::layer();
            registry.with(fmt_layer).init();
            TelemetryGuard { provider: None }
        }
    }
}

fn build_otel_layer<S>(
    service_name: &str,
    endpoint: &str,
) -> (
    tracing_opentelemetry::OpenTelemetryLayer<S, opentelemetry_sdk::trace::Tracer>,
    opentelemetry_sdk::trace::SdkTracerProvider,
)
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .build()
        .expect("Failed to build OTLP span exporter");

    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            opentelemetry_sdk::Resource::builder()
                .with_service_name(service_name.to_owned())
                .build(),
        )
        .build();

    let tracer = provider.tracer(service_name.to_owned());
    opentelemetry::global::set_tracer_provider(provider.clone());

    let layer = tracing_opentelemetry::layer().with_tracer(tracer);
    (layer, provider)
}
