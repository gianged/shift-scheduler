use std::time::Duration;

pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("Ctrl+C pressed, starting graceful shutdown, bye bye!"),
        () = terminate => tracing::info!("Received SIGTERM, starting graceful shutdown, bye bye!"),
    }
}
