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

    let config = Config::load().context("failed to load configuration")?;

    // Log the provenance of every setting before doing anything with it. An operator debugging a
    // deployment should never have to guess which of the defaults, the file, and the environment
    // won — that guess is most of the pain of configuring the incumbents.
    for (key, setting) in config.settings() {
        tracing::info!(setting = key, value = %setting, source = %setting.source(), "configuration");
    }

    let listener = tokio::net::TcpListener::bind(config.bind.value())
        .await
        .with_context(|| {
            format!(
                "failed to bind {}, from {}",
                config.bind,
                config.bind.source()
            )
        })?;

    tracing::info!(bind = %config.bind, data_dir = %config.data_dir, "OpenBiz starting");

    axum::serve(listener, app())
        .await
        .context("server terminated unexpectedly")?;

    Ok(())
}
