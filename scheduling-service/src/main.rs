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
use std::{env, sync::Arc};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

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
    let config = SchedulingConfig::default();

    let scheduling_service = Arc::new(SchedulingService::new(job_repo, data_client, config));

    let state = Arc::new(SchedulingAppState { scheduling_service });

    let app = Router::new()
        .route("/api/v1/schedules", post(schedule::submit_schedule))
        .route(
            "/api/v1/schedules/{schedule_id}/status",
            get(schedule::get_status),
        )
        .route(
            "/api/v1/schedules/{schedule_id}/result",
            get(schedule::get_result),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!("scheduling-service listening on 0.0.0.0:{port}");

    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .await
        .expect("Oppsie! Server crashed!");
}
