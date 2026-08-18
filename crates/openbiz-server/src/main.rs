//! OpenBiz — the single binary.

use std::io::IsTerminal;

use anyhow::Context;
use openbiz_server::{app, shutdown_signal, Config};
use openbiz_store::Store;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Colour only when a human is watching. Redirected to a file, a journald unit, or a container
    // log collector, ANSI escapes are noise that breaks `grep` and every log shipper's parser —
    // and the logs an operator most needs to read are exactly the ones nobody is watching live.
    let ansi = std::io::stdout().is_terminal();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_env("OPENBIZ_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_ansi(ansi))
        .init();

    let config = Config::load().context("failed to load configuration")?;

    // Log the provenance of every setting before doing anything with it. An operator debugging a
    // deployment should never have to guess which of the defaults, the file, and the environment
    // won — that guess is most of the pain of configuring the incumbents.
    for (key, setting) in config.settings() {
        tracing::info!(setting = key, value = %setting, source = %setting.source(), "configuration");
    }

    // Open the store *before* binding. A store that will not open must be a process that never
    // starts, not one that accepts requests and fails each of them — "up but useless" is the
    // ambiguity the split app/triplestore deployments of the incumbents can never resolve. The
    // error names the configuration layer that chose the path, so the operator knows which file
    // or variable to edit rather than only which directory failed.
    let store = Store::open(config.data_dir.value()).with_context(|| {
        format!(
            "failed to open the store in {}, from {}",
            config.data_dir,
            config.data_dir.source()
        )
    })?;
    tracing::info!(
        path = %store.path().display(),
        format_version = store.format_version(),
        "store open"
    );

    let listener = tokio::net::TcpListener::bind(config.bind.value())
        .await
        .with_context(|| {
            format!(
                "failed to bind {}, from {}",
                config.bind,
                config.bind.source()
            )
        })?;

    // Log the address actually bound, not only the one requested. They differ whenever the
    // request was not fully specified — `:0` for an ephemeral port, or a host that resolves — and
    // "which port am I actually on" should never require `ss` or `lsof` to answer.
    let listening = listener
        .local_addr()
        .context("the listener reported no local address")?;

    tracing::info!(
        bind = %config.bind,
        listening = %listening,
        data_dir = %config.data_dir,
        "OpenBiz starting"
    );

    axum::serve(listener, app())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server terminated unexpectedly")?;

    // Reached only after the graceful shutdown above has drained in-flight requests, so nothing
    // is still writing when the store is flushed.
    store
        .close()
        .context("the store did not close cleanly; recent writes may not have reached disk")?;
    tracing::info!("store closed cleanly");

    Ok(())
}
