use std::time::Duration;

/// Maximum time to wait for background tasks during graceful shutdown.
pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

/// Waits for either Ctrl+C or SIGTERM, then returns to trigger graceful shutdown.
///
/// # Panics
///
/// Panics if the Ctrl+C or SIGTERM signal handler cannot be installed.
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
