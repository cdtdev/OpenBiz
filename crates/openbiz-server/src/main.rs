//! OpenBiz — the single binary.

use std::io::IsTerminal;
use std::sync::Arc;

use anyhow::Context;
use openbiz_server::{app, AppState, Command, Config, StopSignals, USAGE};
use openbiz_store::{Decision, Store};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Exit status for arguments we could not make sense of.
///
/// Distinct from the 1 that a failed operation returns, because a script that retries a failed
/// backup should *not* retry a mistyped one.
const EXIT_BAD_USAGE: i32 = 2;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Arguments first: `openbiz help` must work on a machine with no configuration, no data
    // directory, and no permission to create one. Anything that reads the environment before
    // knowing what was asked for turns "how do I use this?" into a configuration error.
    let command = match Command::parse(std::env::args_os().skip(1)) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("{error}\n\n{USAGE}");
            std::process::exit(EXIT_BAD_USAGE);
        }
    };

    if command == Command::Help {
        println!("{USAGE}");
        return Ok(());
    }

    // A one-shot command's *result* is its stdout, so its logs go to stderr — that is what lets a
    // cron job capture the outcome without the provenance lines, and still keep both. The server
    // has no such split to make: everything it emits is a log.
    let serving = command == Command::Serve;
    // Colour only when a human is watching. Redirected to a file, a journald unit, or a container
    // log collector, ANSI escapes are noise that breaks `grep` and every log shipper's parser —
    // and the logs an operator most needs to read are exactly the ones nobody is watching live.
    let ansi = if serving {
        std::io::stdout().is_terminal()
    } else {
        std::io::stderr().is_terminal()
    };

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_env("OPENBIZ_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(ansi)
                .with_writer(move || -> Box<dyn std::io::Write> {
                    if serving {
                        Box::new(std::io::stdout())
                    } else {
                        Box::new(std::io::stderr())
                    }
                }),
        )
        .init();

    let config = Config::load().context("failed to load configuration")?;

    // Log the provenance of every setting before doing anything with it. An operator debugging a
    // deployment should never have to guess which of the defaults, the file, and the environment
    // won — that guess is most of the pain of configuring the incumbents. It matters just as much
    // for a backup, where the question is "which store did it actually copy?".
    for (key, setting) in config.settings() {
        tracing::info!(setting = key, value = %setting, source = %setting.source(), "configuration");
    }

    // Open the store *before* binding. A store that will not open must be a process that never
    // starts, not one that accepts requests and fails each of them — "up but useless" is the
    // ambiguity the split app/triplestore deployments of the incumbents can never resolve. The
    // error names the configuration layer that chose the path, so the operator knows which file
    // or variable to edit rather than only which directory failed.
    //
    // The one-shot commands open it the same way and for the same reason, and the backend's
    // exclusive lock is what makes "stop the server first" enforced rather than advised: a backup
    // taken against a running deployment fails with `already in use` instead of copying a store
    // that is being written underneath it.
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

    // A store upgrade is the one change to a customer's data nobody asked for, so it is said out
    // loud rather than inferred from a version number that has quietly moved. `CLAUDE.md` §3
    // requires an auto-applied change to explain itself; the same facts are in the store's system
    // graph for the audit that comes later.
    if store.migrations().migrated() {
        tracing::warn!(
            from_version = store.migrations().previous_version(),
            to_version = store.migrations().current_version(),
            "{}",
            store.migrations()
        );
        for step in store.migrations().steps() {
            tracing::info!(migration = step.id, "{}", step.description);
        }
    }

    match command {
        Command::Backup { file } => one_shot(store, |store| openbiz_server::back_up(store, &file)),
        Command::Restore { file } => one_shot(store, |store| openbiz_server::restore(store, &file)),
        Command::Import { graph, file } => {
            one_shot(store, |store| openbiz_server::import(store, &graph, &file))
        }
        Command::Retract { graph, file } => {
            one_shot(store, |store| openbiz_server::retract(store, &graph, &file))
        }
        Command::Inspect { graph } => {
            one_shot(store, |store| openbiz_server::inspect(store, &graph))
        }
        Command::Integrity { graph } => {
            one_shot(store, |store| openbiz_server::integrity(store, &graph))
        }
        Command::Ancestors { graph, concept } => one_shot(store, |store| {
            openbiz_server::ancestors(store, &graph, &concept)
        }),
        Command::Paths { graph, concept } => one_shot(store, |store| {
            openbiz_server::paths(store, &graph, &concept)
        }),
        Command::Tree { graph, concept } => {
            one_shot(store, |store| openbiz_server::tree(store, &graph, &concept))
        }
        Command::Search {
            graph,
            query,
            current_only,
        } => one_shot(store, |store| {
            openbiz_server::search(store, &graph, &query, current_only)
        }),
        Command::Mint {
            graph,
            label,
            pattern,
        } => one_shot(store, |store| {
            openbiz_server::mint(store, &graph, label.as_deref(), pattern.as_deref())
        }),
        Command::Policy { graph, pattern } => one_shot(store, |store| {
            openbiz_server::policy(store, &graph, pattern.as_deref())
        }),
        Command::Move {
            graph,
            concept,
            to,
            from,
        } => one_shot(store, |store| {
            openbiz_server::relocate(store, &graph, &concept, &to, from.as_deref())
        }),
        Command::Merge {
            graph,
            source,
            target,
        } => one_shot(store, |store| {
            openbiz_server::merge(store, &graph, &source, &target)
        }),
        Command::Split {
            graph,
            concept,
            labels,
            placement,
            language,
            pattern,
        } => one_shot(store, |store| {
            openbiz_server::split(
                store,
                &graph,
                &concept,
                &labels,
                placement,
                language.as_deref(),
                pattern.as_deref(),
            )
        }),
        Command::Deprecate {
            graph,
            concept,
            replaced_by,
            note,
            language,
        } => one_shot(store, |store| {
            openbiz_server::deprecate(
                store,
                &graph,
                &concept,
                replaced_by.as_deref(),
                note.as_deref(),
                language.as_deref(),
            )
        }),
        Command::Reinstate {
            graph,
            resource,
            note,
            language,
        } => one_shot(store, |store| {
            openbiz_server::reinstate(
                store,
                &graph,
                &resource,
                note.as_deref(),
                language.as_deref(),
            )
        }),
        Command::Notes { graph, resource } => one_shot(store, |store| {
            openbiz_server::notes(store, &graph, &resource)
        }),
        Command::Mappings { graph, resource } => one_shot(store, |store| {
            openbiz_server::mappings(store, &graph, &resource)
        }),
        Command::Candidates => one_shot(store, openbiz_server::candidates),
        Command::Show { id } => one_shot(store, |store| openbiz_server::show(store, &id)),
        Command::Approve { id } => one_shot(store, |store| {
            openbiz_server::decide(store, &id, Decision::Approve)
        }),
        Command::Reject { id } => one_shot(store, |store| {
            openbiz_server::decide(store, &id, Decision::Reject)
        }),
        Command::Serve => serve(config, store).await,
        // Answered above, before anything was configured or opened.
        Command::Help => Ok(()),
    }
}

/// Run a command that owns the store for its whole life, then close it.
///
/// The close is not optional and is not `Drop`'s job: a restore that is not flushed is a restore
/// the next start may not find, and `Store::close` is the only thing that reports whether the
/// flush worked. The result line goes to stdout only once that has succeeded, so a script that
/// reads "restored 12 000 statements" is reading a statement about the disk.
fn one_shot(
    store: Store,
    work: impl FnOnce(&Store) -> Result<String, openbiz_server::CommandError>,
) -> anyhow::Result<()> {
    let outcome = work(&store)?;

    store
        .close()
        .context("the store did not close cleanly; the operation may not have reached disk")?;

    // `println!` rather than `tracing`: this is the command's *output*, not a log of it. A backup
    // script pipes stdout; timestamps and levels belong on the other stream (`CLAUDE.md` §6's rule
    // is about logging, and this is not a log line).
    println!("{outcome}");
    Ok(())
}

/// Serve until a shutdown signal arrives, then close the store.
async fn serve(config: Config, store: Store) -> anyhow::Result<()> {
    // Before anything else a hard kill would interrupt. Until these are registered the kernel's
    // default disposition applies, and for SIGTERM that is immediate termination — so a
    // `docker stop` arriving during the registry read or the bind below would kill the process
    // outright, mid-open, which is precisely what `shutdown.rs` exists to prevent. Registering
    // lazily, at the first poll inside `axum::serve`, left that window open across both.
    let stop = StopSignals::install();

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

    // Shared with the router, which clones the state per connection. This keeps a handle so it
    // can still close the store — see the reclaim below, which is where that ordering is enforced.
    let store = Arc::new(store);

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
        .with_graceful_shutdown(stop.wait())
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
