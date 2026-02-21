use axum::{
    Router,
    routing::{get, post},
};
use scheduling_service::{
    api::{handler::schedule, state::SchedulingAppState},
    domain::{scheduler::SchedulingConfig, service::SchedulingService},
    infrastructure::{client::HttpDataServiceClient, job::PgJobRepository},
};
use sqlx::postgres::PgPoolOptions;
use std::{env, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tower_governor::{
    GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor,
};
use tower_http::trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        schedule::submit_schedule,
        schedule::get_status,
        schedule::get_result,
    ),
    tags(
        (name = "Schedules", description = "Schedule job management"),
    )
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
    let _guard = shared::telemetry::init_telemetry("scheduling-service");

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let port = env::var("SERVER_PORT").unwrap_or_else(|_| "8081".to_string());
    let data_service_url =
        env::var("DATA_SERVICE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to establish connection into Postgres");

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    let job_repo = Arc::new(PgJobRepository::new(pool.clone()));
    let data_client = Arc::new(HttpDataServiceClient::new(data_service_url));
    let config_path =
        env::var("SCHEDULING_CONFIG_PATH").unwrap_or_else(|_| "scheduling.toml".to_string());
    let config = SchedulingConfig::load(&config_path).expect("Failed to load scheduling config");

    let scheduling_service = Arc::new(SchedulingService::new(job_repo, data_client, config));

    if let Err(e) = scheduling_service.recover_stale_jobs().await {
        tracing::warn!("Failed to recover stale jobs: {e}");
    }

    let state = Arc::new(SchedulingAppState {
        scheduling_service: scheduling_service.clone(),
    });

    let governor_conf = GovernorConfigBuilder::default()
        .per_second(2)
        .burst_size(10)
        .key_extractor(SmartIpKeyExtractor)
        .use_headers()
        .finish()
        .expect("Failed to build governor config");

    let app = Router::new()
        .route(
            "/headpat",
            get(|| async {
                axum::Json(shared::responses::HeadpatResponse {
                    message: "nyaa~! all systems operational, senpai! (=^-w-^=)",
                })
            }),
        )
        .route("/api/v1/schedules", post(schedule::submit_schedule))
        .route(
            "/api/v1/schedules/{schedule_id}/status",
            get(schedule::get_status),
        )
        .route(
            "/api/v1/schedules/{schedule_id}/result",
            get(schedule::get_result),
        )
        // Swagger UI
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        // Rate limiting (per-IP, 2 req/s with burst of 10)
        .layer(GovernorLayer::new(governor_conf))
        // tracing log (turn request into info level)
        .layer(
            TraceLayer::new_for_http()
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .latency_unit(tower_http::LatencyUnit::Millis),
                ),
        )
        .with_state(state);

    tracing::info!("scheduling-service listening on 0.0.0.0:{port}");

    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shared::shutdown::shutdown_signal())
    .await
    .expect("Oppsie! Server crashed!");

    // Server stopped accepting new requests; wait for in-flight background jobs
    let task_tracker = scheduling_service.task_tracker();
    task_tracker.close();
    tracing::info!("Waiting for background jobs to finish...");
    if tokio::time::timeout(
        shared::shutdown::DEFAULT_SHUTDOWN_TIMEOUT,
        task_tracker.wait(),
    )
    .await
    .is_err()
    {
        tracing::warn!("Shutdown timeout reached, some background jobs may not have finished");
    }
    tracing::info!("scheduling-service shut down");
}
