//! OpenBiz — the single binary.

use std::io::IsTerminal;
use std::sync::Arc;

use anyhow::Context;
use openbiz_server::{app, shutdown_signal, AppState, Config};
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

    // Shared with the router, which clones the state per connection. `main` keeps a handle so it
    // can still close the store — see the reclaim below, which is where that ordering is enforced.
    let store = Arc::new(store);

    // Read the graph registry now rather than on first request. It is the store's own account of
    // what it holds, so a registry it cannot describe — an unknown graph kind, an entry that
    // breaks the namespace rule — is a store we would be guessing about, and guessing is how a
    // governance tool loses the right to be believed. Failing here keeps that on the same footing
    // as a store that will not open: better never up than up and wrong.
    let graphs = store
        .graphs()
        .context("the store's graph registry could not be read")?;
    tracing::info!(graphs = graphs.len(), "graph registry read");
    for graph in &graphs {
        // Vocabulary IRIs are customer metadata, so they are named at debug and counted at info.
        tracing::debug!(graph = %graph, kind = %graph.kind(), "registered graph");
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

    axum::serve(listener, app(AppState::new(Arc::clone(&store))))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server terminated unexpectedly")?;

    // Reached only after the graceful shutdown above has drained in-flight requests, so nothing
    // is still writing when the store is flushed.
    //
    // `close` consumes the store, so the shared handle has to be reclaimed first. If it cannot be,
    // something is still holding a clone — a leaked task, a request that outlived the drain — and
    // the honest response is to say so rather than to skip the flush quietly. A silent skip is the
    // exact failure `Store::close` exists to make impossible: an operator reading a clean shutdown
    // log while the last writes never reached disk.
    let store = Arc::into_inner(store).context(
        "the store was still in use after the server drained, so it could not be closed cleanly; \
         recent writes may not have reached disk",
    )?;

    store
        .close()
        .context("the store did not close cleanly; recent writes may not have reached disk")?;
    tracing::info!("store closed cleanly");

    Ok(())
}
