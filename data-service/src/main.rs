use sqlx::postgres::PgPoolOptions;
use std::env;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
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

    let app = axum::Router::new();

    let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .await
        .expect("Oppsie! Server crashed!");
}
