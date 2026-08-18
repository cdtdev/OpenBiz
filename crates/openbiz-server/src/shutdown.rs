//! When to stop serving.
//!
//! Shutdown matters more here than in a stateless service, because OpenBiz *is* the database. A
//! process killed mid-write is the incumbents' recovery-procedure problem arriving in a product
//! that promised the operator would never need one. So the contract is:
//!
//! 1. A stop signal makes the server stop accepting new connections and finish the ones in flight.
//! 2. Only then is the store flushed and closed.
//! 3. The log says, unambiguously, that the store closed cleanly — so an operator reading the tail
//!    of a container log can tell a graceful stop from a `SIGKILL` without guessing.
//!
//! **`SIGTERM` is the important one.** `docker stop`, a Kubernetes pod eviction, and `systemctl
//! stop` all send it, and a service that only handles `Ctrl-C` gets killed ten seconds later by
//! the runtime's escalation to `SIGKILL` — a hard kill on every routine restart, which is exactly
//! the case a store must survive and nobody tests.

/// Resolves when the process is asked to stop.
///
/// Listens for `Ctrl-C` and, on Unix, `SIGTERM`. Handed to
/// [`axum::serve::Serve::with_graceful_shutdown`].
///
/// Registering the handler can fail — it is one of the few genuine startup failures where there is
/// no useful recovery and no caller to report to. Rather than `expect` (barred by `CLAUDE.md` §6
/// outside startup) or crash, a failed registration logs and then waits forever, which degrades to
/// "this signal will not be handled gracefully" rather than "the server refuses to run".
pub async fn shutdown_signal() {
    let ctrl_c = async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => tracing::info!(signal = "SIGINT", "stop requested"),
            Err(error) => {
                tracing::error!(%error, "could not listen for Ctrl-C; it will not stop this server gracefully");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
                tracing::info!(signal = "SIGTERM", "stop requested");
            }
            Err(error) => {
                tracing::error!(%error, "could not listen for SIGTERM; a container stop will hard-kill this server");
                std::future::pending::<()>().await;
            }
        }
    };

    // Non-Unix has no SIGTERM; the Ctrl-C branch is the whole contract there.
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The signal future must not resolve on its own. If it did, the server would shut down
    /// immediately on every start — and because `axum::serve` returns `Ok(())` in that case, the
    /// binary would exit zero and look healthy while serving nothing.
    #[tokio::test]
    async fn waits_when_no_signal_arrives() {
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(250), shutdown_signal()).await;

        assert!(
            result.is_err(),
            "shutdown_signal resolved without a signal, which would stop the server at startup"
        );
    }

    // The `SIGTERM` path is deliberately *not* unit-tested in process. Raising a real signal
    // against the test binary is global state: if the handler has not finished registering, the
    // default disposition kills the whole test run, and the failure looks like an unrelated
    // harness crash. It is covered instead by `tests/graceful_shutdown.rs`, which sends `SIGTERM`
    // to a real spawned `openbiz` process and asserts on its exit status and its log — a stronger
    // claim than this file could make anyway, because it also proves the store closed.
}
