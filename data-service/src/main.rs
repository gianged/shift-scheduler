use axum::{
    Router,
    routing::{delete, get, patch, post},
};
use data_service::{
    api::{
        handler::{group, membership, staff},
        state::DataServiceAppState,
    },
    infrastructure::{
        group::PgGroupRepository, membership::PgMembershipRepository, staff::PgStaffRepository,
    },
};
use sqlx::postgres::PgPoolOptions;
use std::{env, sync::Arc};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    let _guard = shared::telemetry::init_telemetry("data-service");

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let port = env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to establish connection into Postgres");

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    let state = Arc::new(DataServiceAppState {
        staff_repo: Arc::new(PgStaffRepository::new(pool.clone())),
        group_repo: Arc::new(PgGroupRepository::new(pool.clone())),
        membership_repo: Arc::new(PgMembershipRepository::new(pool.clone())),
    });

    let app = Router::new()
        // Staff routes
        .route("/api/v1/staff", get(staff::find_all).post(staff::create))
        .route("/api/v1/staff/batch", post(staff::batch_create))
        .route(
            "/api/v1/staff/{id}",
            get(staff::find_by_id)
                .put(staff::update)
                .delete(staff::delete),
        )
        .route("/api/v1/staff/{id}/deactivate", patch(staff::deactivate))
        // Group routes
        .route("/api/v1/groups", get(group::find_all).post(group::create))
        .route("/api/v1/groups/batch", post(group::batch_create))
        .route(
            "/api/v1/groups/{id}",
            get(group::find_by_id)
                .put(group::update)
                .delete(group::delete),
        )
        // Membership routes
        .route(
            "/api/v1/groups/{group_id}/members",
            get(membership::get_group_members).post(membership::add_member),
        )
        .route(
            "/api/v1/groups/{group_id}/members/{staff_id}",
            delete(membership::remove_member),
        )
        .route(
            "/api/v1/groups/{group_id}/resolved-members",
            get(membership::resolve_members),
        )
        // Staff's groups (optional)
        .route(
            "/api/v1/staff/{id}/groups",
            get(membership::get_staff_groups),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!("data-service listening on 0.0.0.0:{port}");

    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .await
        .expect("Oppsie! Server crashed!");
}
