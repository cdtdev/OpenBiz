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

/// The stop signals, **registered**, waiting to be awaited.
///
/// # Why registration is separate from waiting, and happens first
///
/// A signal handler that has not been installed yet is not a handler: the kernel applies the
/// default disposition, and for `SIGTERM` that is immediate termination. So there is a window
/// between "this process exists" and "this process handles `SIGTERM`", and anything that happens
/// inside it is a hard kill — the very thing this module exists to prevent.
///
/// Handing [`axum::serve`]'s `with_graceful_shutdown` a future that registers on **first poll**
/// puts that window somewhere terrible: it stays open across the store's registry read and the
/// listener bind, so a server that has already logged the port it is listening on can still be
/// killed outright by a `docker stop`. `tokio::signal::ctrl_c()` and an inline
/// `tokio::signal::unix::signal(..)` both do exactly that.
///
/// [`StopSignals::install`] closes the window as far as it can be closed. It is synchronous, it
/// registers both dispositions before it returns, and it logs when it has — so the moment a
/// caller has a `StopSignals` in hand, a signal will be *queued* rather than fatal, even if
/// nothing is awaiting it yet. The residue is the part no program can fix: from `exec` until this
/// runs, the default disposition applies.
///
/// [`axum::serve`]: https://docs.rs/axum/latest/axum/fn.serve.html
pub struct StopSignals {
    #[cfg(unix)]
    interrupt: Option<tokio::signal::unix::Signal>,
    #[cfg(unix)]
    terminate: Option<tokio::signal::unix::Signal>,
}

impl StopSignals {
    /// Register the stop signals now. Call before doing anything a hard kill would interrupt.
    ///
    /// Registration can fail, and it is one of the few genuine startup failures with no useful
    /// recovery and no caller to report to. Rather than `expect` (barred by `CLAUDE.md` §6 outside
    /// startup) or refusing to run, a failed registration logs and that signal then waits forever
    /// — which degrades to "this signal will not be handled gracefully" rather than "the server
    /// will not start". The log line is what tells an operator which of the two they have.
    #[cfg(unix)]
    pub fn install() -> Self {
        use tokio::signal::unix::{signal, SignalKind};

        let register = |kind: SignalKind, name: &'static str| match signal(kind) {
            Ok(stream) => Some(stream),
            Err(error) => {
                tracing::error!(
                    %error,
                    signal = name,
                    "could not register a stop signal; it will not stop this server gracefully"
                );
                None
            }
        };

        let interrupt = register(SignalKind::interrupt(), "SIGINT");
        let terminate = register(SignalKind::terminate(), "SIGTERM");

        // Logged *after* both registrations and before the caller does anything else, so a reader
        // of the log — or a test — can take this line as proof that a stop signal from here on is
        // queued rather than fatal. `tests/graceful_shutdown.rs` depends on exactly that.
        tracing::info!(
            sigint = interrupt.is_some(),
            sigterm = terminate.is_some(),
            "stop signals registered"
        );

        Self {
            interrupt,
            terminate,
        }
    }

    /// Register the stop signals now.
    ///
    /// There is no `SIGTERM` off Unix, so `Ctrl-C` is the whole contract and `tokio::signal` has
    /// no synchronous registration for it. The window this type exists to close therefore stays
    /// open on this platform; saying so here is better than a type that quietly implies otherwise.
    #[cfg(not(unix))]
    pub fn install() -> Self {
        tracing::info!(sigint = true, sigterm = false, "stop signals registered");
        Self {}
    }

    /// Resolve when the process is asked to stop.
    ///
    /// Handed to [`axum::serve`]'s `with_graceful_shutdown`. A signal that arrived *before* this
    /// was awaited still resolves it: the streams were registered by [`StopSignals::install`] and
    /// have been buffering since.
    ///
    /// [`axum::serve`]: https://docs.rs/axum/latest/axum/fn.serve.html
    #[cfg(unix)]
    pub async fn wait(self) {
        let Self {
            mut interrupt,
            mut terminate,
        } = self;

        let interrupt = async {
            match interrupt.as_mut() {
                Some(stream) => {
                    stream.recv().await;
                    tracing::info!(signal = "SIGINT", "stop requested");
                }
                None => std::future::pending().await,
            }
        };
        let terminate = async {
            match terminate.as_mut() {
                Some(stream) => {
                    stream.recv().await;
                    tracing::info!(signal = "SIGTERM", "stop requested");
                }
                None => std::future::pending().await,
            }
        };

        tokio::select! {
            () = interrupt => {},
            () = terminate => {},
        }
    }

    /// Resolve when the process is asked to stop.
    #[cfg(not(unix))]
    pub async fn wait(self) {
        match tokio::signal::ctrl_c().await {
            Ok(()) => tracing::info!(signal = "SIGINT", "stop requested"),
            Err(error) => {
                tracing::error!(%error, "could not listen for Ctrl-C; it will not stop this server gracefully");
                std::future::pending::<()>().await;
            }
        }
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
        let signals = StopSignals::install();
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(250), signals.wait()).await;

        assert!(
            result.is_err(),
            "the stop future resolved without a signal, which would stop the server at startup"
        );
    }

    /// Registration is synchronous, so it is finished before `install` returns. This is the whole
    /// point of splitting it from the wait, and it is asserted rather than assumed: a future
    /// implementation that made `install` lazy would reopen the window with no test to notice.
    #[cfg(unix)]
    #[tokio::test]
    async fn install_registers_both_dispositions_before_it_returns() {
        let signals = StopSignals::install();

        assert!(signals.interrupt.is_some(), "SIGINT must be registered");
        assert!(signals.terminate.is_some(), "SIGTERM must be registered");
    }

    // The `SIGTERM` path is deliberately *not* unit-tested in process. Raising a real signal
    // against the test binary is global state: if the handler has not finished registering, the
    // default disposition kills the whole test run, and the failure looks like an unrelated
    // harness crash. It is covered instead by `tests/graceful_shutdown.rs`, which sends `SIGTERM`
    // to a real spawned `openbiz` process and asserts on its exit status and its log — a stronger
    // claim than this file could make anyway, because it also proves the store closed.
}
