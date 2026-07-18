use anyhow::Result;
use bridge_gateway::{AppState, Config, router};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("bridge_gateway=info,tower_http=info")),
        )
        .init();
    let config = Config::from_env()?;
    tracing::info!(
        address = %config.bind,
        database = %config.database_path.display(),
        ollama = %format_args!("{}:{}", config.ollama_host, config.ollama_port),
        model = %config.model,
        tool_timeout_seconds = config.tools.timeout.as_secs(),
        "Starting Bridge gateway"
    );
    let listener = TcpListener::bind(config.bind).await?;
    let state = AppState::connect(&config).await?;
    tracing::info!(address = %config.bind, "Bridge gateway is ready to accept requests");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown())
        .await?;
    tracing::info!("Bridge gateway stopped cleanly");
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler")
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => tracing::info!(signal = "SIGINT", "Shutdown requested"),
        _ = terminate => tracing::info!(signal = "SIGTERM", "Shutdown requested"),
    }
}
