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
        cache::{
            client::RedisCache, group::CachedGroupRepository,
            membership::CachedMembershipRepository, staff::CachedStaffRepository,
        },
        group::PgGroupRepository,
        membership::PgMembershipRepository,
        staff::PgStaffRepository,
    },
};
use sqlx::postgres::PgPoolOptions;
use std::{env, sync::Arc};
use tokio::net::TcpListener;
use tower_http::trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::Level;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        staff::find_all,
        staff::create,
        staff::batch_create,
        staff::find_by_id,
        staff::update,
        staff::deactivate,
        staff::delete,
        group::find_all,
        group::create,
        group::batch_create,
        group::find_by_id,
        group::update,
        group::delete,
        membership::add_member,
        membership::remove_member,
        membership::get_group_members,
        membership::get_staff_groups,
        membership::resolve_members,
        membership::batch_add_members,
    ),
    tags(
        (name = "Staff", description = "Staff management"),
        (name = "Groups", description = "Staff group management"),
        (name = "Membership", description = "Group membership management"),
    )
)]
struct ApiDoc;

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

    let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set");
    let cache = RedisCache::new(&redis_url)
        .await
        .expect("Failed to connect to Redis");

    let state = Arc::new(DataServiceAppState {
        staff_repo: Arc::new(CachedStaffRepository::new(
            Arc::new(PgStaffRepository::new(pool.clone())),
            cache.clone(),
        )),
        group_repo: Arc::new(CachedGroupRepository::new(
            Arc::new(PgGroupRepository::new(pool.clone())),
            cache.clone(),
        )),
        membership_repo: Arc::new(CachedMembershipRepository::new(
            Arc::new(PgMembershipRepository::new(pool.clone())),
            cache,
        )),
    });

    let app = Router::new()
        .route(
            "/headpat",
            get(|| async {
                axum::Json(shared::responses::HeadpatResponse {
                    message: "nyaa~! all systems operational, senpai! (=^-w-^=)",
                })
            }),
        )
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
            "/api/v1/memberships/batch",
            post(membership::batch_add_members),
        )
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
        // Swagger UI
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
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

    tracing::info!("data-service listening on 0.0.0.0:{port}");

    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .with_graceful_shutdown(shared::shutdown::shutdown_signal())
        .await
        .expect("Oppsie! Server crashed!");

    tracing::info!("data-service shut down");
}
