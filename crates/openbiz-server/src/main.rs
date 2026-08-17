//! OpenBiz — the single binary.

use anyhow::Context;
use openbiz_server::{app, Config};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_env("OPENBIZ_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();

    let listener = tokio::net::TcpListener::bind(&config.bind)
        .await
        .with_context(|| format!("failed to bind {}", config.bind))?;

    tracing::info!(bind = %config.bind, data_dir = %config.data_dir, "OpenBiz starting");

    axum::serve(listener, app())
        .await
        .context("server terminated unexpectedly")?;

    Ok(())
}
