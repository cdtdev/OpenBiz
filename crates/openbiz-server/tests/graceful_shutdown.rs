//! End-to-end store lifecycle, against the real `openbiz` binary as a child process.
//!
//! Everything here needs a *process*, not a router: signal disposition, exit status, the
//! backend's inter-process lock, and the wiring between `std::env` and `Config::load` are all
//! properties of a running program. Driving the library in-process would prove none of them.
//!
//! Unix-only. `SIGTERM` is the thing under test, and it does not exist elsewhere.
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// How long to wait for the child to start serving, or to stop.
const PATIENCE: Duration = Duration::from_secs(30);

/// A spawned `openbiz`, with its log captured to a file.
struct Server {
    child: Child,
    log_path: std::path::PathBuf,
}

impl Server {
    /// Start `openbiz` on an ephemeral port with `data_dir` as its store, and wait until it is
    /// serving.
    ///
    /// `cwd` is the temp directory so a stray `openbiz.toml` in the checkout cannot influence the
    /// test — the point is to exercise the environment layer in isolation.
    fn start(data_dir: &Path, log_path: &Path, ready: bool) -> Self {
        // Both streams into one file: `tracing` writes to stdout, but a panic or an `anyhow`
        // bail-out from `main` goes to stderr, and a test that captured only one of them would
        // report "the server never started" with an empty log for the most interesting failures.
        let log = std::fs::File::create(log_path).expect("create the log file");
        let log_err = log.try_clone().expect("clone the log file handle");

        let child = Command::new(env!("CARGO_BIN_EXE_openbiz"))
            .current_dir(data_dir)
            .env("OPENBIZ_BIND", "127.0.0.1:0")
            .env("OPENBIZ_DATA_DIR", data_dir)
            .env("OPENBIZ_LOG", "info")
            .env_remove("OPENBIZ_CONFIG")
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .spawn()
            .expect("spawn the openbiz binary");

        let server = Self {
            child,
            log_path: log_path.to_path_buf(),
        };

        if ready {
            server.wait_until_serving();
        }

        server
    }

    fn log(&self) -> String {
        let mut contents = String::new();
        std::fs::File::open(&self.log_path)
            .expect("open the log file")
            .read_to_string(&mut contents)
            .expect("read the log file");
        contents
    }

    /// The address the child actually bound.
    ///
    /// Read from the startup log rather than chosen in advance: picking a free port here and
    /// handing it over leaves a window in which something else takes it, which is exactly the kind
    /// of intermittent failure that gets a real test deleted.
    fn listening_on(&self) -> Option<SocketAddr> {
        self.log()
            .split_whitespace()
            .find_map(|token| token.strip_prefix("listening="))
            .and_then(|value| value.parse().ok())
    }

    /// Block until `needle` appears in the child's log, or panic.
    ///
    /// Waiting on a *log line* rather than on a socket is what makes the test below deterministic:
    /// the line is written after the thing it announces, so seeing it is proof the thing happened.
    fn wait_for_log(&self, needle: &str) {
        let deadline = Instant::now() + PATIENCE;
        while Instant::now() < deadline {
            if self.log().contains(needle) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("{needle:?} never appeared in the log. Log:\n{}", self.log());
    }

    /// Block until the server **answers a request**, not merely until its port is bound.
    ///
    /// The distinction is not pedantry. `serve` binds the listener and logs the port it got
    /// *before* it hands the listener to `axum::serve`, so a TCP connect succeeds — out of the
    /// kernel's accept backlog — while the process is still some way from serving anything. A
    /// probe that stopped there would return early and, worse, would leave an accepted-but-never-
    /// answered connection behind for the graceful drain to reason about. Completing one exchange
    /// proves the whole path and leaves nothing dangling.
    fn wait_until_serving(&self) {
        let deadline = Instant::now() + PATIENCE;
        while Instant::now() < deadline {
            if let Some(addr) = self.listening_on() {
                if try_get(addr, "/healthz").is_some_and(|response| response.contains("200")) {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("the server never started serving. Log:\n{}", self.log());
    }

    fn signal(&self, name: &str) {
        let status = Command::new("kill")
            .args([&format!("-{name}"), &self.child.id().to_string()])
            .status()
            .expect("run kill");
        assert!(status.success(), "kill -{name} failed");
    }

    /// Wait for the child to exit, returning its status. Panics rather than hanging.
    fn wait_for_exit(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + PATIENCE;
        while Instant::now() < deadline {
            match self.child.try_wait().expect("poll the child") {
                Some(status) => return status,
                None => std::thread::sleep(Duration::from_millis(50)),
            }
        }
        let _ = self.child.kill();
        panic!("the server did not exit. Log:\n{}", self.log());
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Never leave a child holding a store lock behind a failed assertion.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `SIGTERM` is what `docker stop`, a Kubernetes eviction, and `systemctl stop` send. A server
/// that ignores it is hard-killed on every routine restart, so this is the ordinary path, not the
/// exceptional one: the process must exit zero, say the store closed cleanly, and leave a store
/// the next process can open.
#[test]
fn sigterm_closes_the_store_cleanly_and_exits_zero() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let log = temp.path().join("first.log");
    let mut server = Server::start(temp.path(), &log, true);

    assert!(
        temp.path().join("store").is_dir(),
        "the store must exist on disk under the configured data directory"
    );

    server.signal("TERM");
    let status = server.wait_for_exit();
    let log_text = server.log();

    assert!(
        status.success(),
        "a SIGTERM stop must exit zero, got {status}. Log:\n{log_text}"
    );
    assert!(
        log_text.contains("signal=\"SIGTERM\"") || log_text.contains("signal=SIGTERM"),
        "the log must record which signal stopped it. Log:\n{log_text}"
    );
    assert!(
        log_text.contains("store closed cleanly"),
        "an operator must be able to tell a clean stop from a kill. Log:\n{log_text}"
    );

    // The lock is released, so the same data directory is immediately reusable. Without this, a
    // "clean" shutdown that leaked the lock would turn every restart into a failure.
    let second_log = temp.path().join("second.log");
    let mut restarted = Server::start(temp.path(), &second_log, true);
    restarted.signal("TERM");
    assert!(
        restarted.wait_for_exit().success(),
        "the restarted server must also stop cleanly. Log:\n{}",
        restarted.log()
    );
}

/// Two instances over one data directory is the classic way to corrupt a self-hosted store — a
/// second container scheduled before the first is fully gone, or an operator who forgot one is
/// running. The second must refuse, and must say *why* in terms of OpenBiz rather than surfacing
/// a RocksDB `LOCK` errno.
#[test]
fn a_second_instance_refuses_to_share_the_data_directory() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let first_log = temp.path().join("first.log");
    let _first = Server::start(temp.path(), &first_log, true);

    let second_log = temp.path().join("second.log");
    let mut second = Server::start(temp.path(), &second_log, false);

    let status = second.wait_for_exit();
    let log_text = second.log();

    assert!(
        !status.success(),
        "a second instance must not start against a locked store. Log:\n{log_text}"
    );
    assert!(
        log_text.contains("already in use by another OpenBiz process"),
        "the refusal must name the real cause. Log:\n{log_text}"
    );
    assert!(
        log_text.contains("$OPENBIZ_DATA_DIR"),
        "the failure must name the configuration layer that chose the path. Log:\n{log_text}"
    );
}

/// The wiring between the real process environment and `Config::resolve`. The resolver is unit
/// tested with an injected lookup because `std::env::set_var` is not thread-safe; that leaves
/// `Config::load` — the code that supplies `std::env::var` — provable only from outside the
/// process. A typo in a variable name inside `load` would pass every other test in this workspace.
#[test]
fn the_process_environment_reaches_the_configuration_with_its_provenance() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let log = temp.path().join("server.log");
    let mut server = Server::start(temp.path(), &log, true);

    let log_text = server.log();

    assert!(
        log_text.contains("setting=\"data_dir\"") || log_text.contains("setting=data_dir"),
        "every setting's provenance must be logged at startup. Log:\n{log_text}"
    );
    assert!(
        log_text.contains("$OPENBIZ_DATA_DIR"),
        "data_dir came from the environment and the log must say so. Log:\n{log_text}"
    );
    assert!(
        log_text.contains("$OPENBIZ_BIND"),
        "bind came from the environment and the log must say so. Log:\n{log_text}"
    );
    assert!(
        server
            .listening_on()
            .is_some_and(|addr| addr.port() != 0 && addr.ip().is_loopback()),
        "requesting port 0 must log the port actually allocated, not the request. Log:\n{log_text}"
    );

    server.signal("TERM");
    server.wait_for_exit();
}

/// The graph registry is read at startup, not on first request. A store whose registry cannot be
/// described is a store we would be guessing about, and this is what makes that a startup failure
/// rather than a surprise later — the same standing as a store that will not open at all.
#[test]
fn the_graph_registry_is_read_at_startup() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let log = temp.path().join("server.log");
    let mut server = Server::start(temp.path(), &log, true);

    let log_text = server.log();

    assert!(
        log_text.contains("graph registry read"),
        "the registry must be read before the server serves. Log:\n{log_text}"
    );
    assert!(
        log_text.contains("graphs=1"),
        "a fresh store holds exactly the system graph, and the count must say so. \
         Log:\n{log_text}"
    );

    server.signal("TERM");
    let status = server.wait_for_exit();
    assert!(
        status.success(),
        "the server must exit zero on SIGTERM. Status: {status}. Log:\n{}",
        server.log()
    );
}

/// `GET <path>` against a child that may not be answering yet, for the readiness probe.
///
/// Returns `None` for anything that is not a completed exchange — a refused connection, a reset
/// mid-response, a server that is bound but not yet serving. A readiness probe must never turn a
/// "not yet" into a panic, which is why this is separate from [`http_get`] rather than a flag on
/// it.
fn try_get(addr: SocketAddr, path: &str) -> Option<String> {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(500)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok()?;
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .ok()?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response).ok()?;
    Some(String::from_utf8_lossy(&response).into_owned())
}

/// `GET <path>` against a running child, returning the whole response as text.
///
/// Hand-rolled over `std::net::TcpStream` rather than pulling in an HTTP client: `Connection:
/// close` makes read-to-end a complete response, and every dependency is a liability
/// (`CLAUDE.md` §1.5).
fn http_get(addr: SocketAddr, path: &str) -> String {
    let mut stream =
        TcpStream::connect_timeout(&addr, PATIENCE).expect("connect to the running server");
    stream
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .expect("write the request");

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("read the response");
    String::from_utf8_lossy(&response).into_owned()
}

/// The registry endpoint, served by the **real binary** over a real socket, and then stopped.
///
/// Two things are under test that no in-process test reaches. First, that `main` hands the open
/// store to the router at all — the unit tests build their own router, so a `main` that forgot to
/// would leave them green. Second, and less obvious: the store is shared with the router through
/// an `Arc`, and `Store::close` consumes it, so `main` has to reclaim sole ownership after the
/// drain. Serving a request first is what makes that reclaim meaningful — a connection clones the
/// state, so this is the only test in which the reclaim can actually fail. If it did, the process
/// would exit non-zero without logging `store closed cleanly`, silently skipping the flush.
#[test]
fn the_graph_registry_is_served_over_http_and_the_store_still_closes_cleanly() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let log = temp.path().join("server.log");
    let mut server = Server::start(temp.path(), &log, true);
    let addr = server
        .listening_on()
        .expect("the server logged its address");

    let response = http_get(addr, "/api/graphs");
    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "the binary must serve the registry, got:\n{response}"
    );
    assert!(
        response.contains(r#"{"iri":"urn:openbiz:graph:system","kind":"system"}"#),
        "the store `main` opened must be the store the handler reads, got:\n{response}"
    );

    server.signal("TERM");
    let status = server.wait_for_exit();
    let log_text = server.log();

    assert!(
        status.success(),
        "a server that has served a request must still stop cleanly, got {status}. \
         Log:\n{log_text}"
    );
    assert!(
        log_text.contains("store closed cleanly"),
        "the store must still be reclaimed and flushed after the router held a handle. \
         Log:\n{log_text}"
    );
}

/// A `SIGTERM` that arrives **before the server is accepting** must still be graceful.
///
/// This is a regression test for a real defect, found by CI rather than by reasoning: the stop
/// signals used to be registered lazily, on the first poll of the future handed to
/// `axum::serve(..).with_graceful_shutdown(..)`. That poll happens *after* the graph registry is
/// read and after the listener binds — so the process could log the port it was listening on and
/// still be killed outright by the kernel's default disposition, because no handler existed yet.
/// `the_graph_registry_is_read_at_startup` hit exactly that window on a loaded CI runner and
/// failed with a non-zero exit; the same race is a hard kill on any `docker stop` that lands early.
///
/// The test is deterministic rather than a tightened race, and that is the point. `install()` is
/// synchronous and logs *after* both dispositions are registered, so once "stop signals
/// registered" is in the log a signal is queued rather than fatal — by construction, for every
/// caller, not just for this one. The store must still close cleanly, because a stop before the
/// first request is still a stop.
#[test]
fn a_stop_signal_arriving_before_the_server_accepts_is_still_graceful() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let log = temp.path().join("early.log");
    // Deliberately *not* waiting until it serves: the window under test is before that.
    let mut server = Server::start(temp.path(), &log, false);

    server.wait_for_log("stop signals registered");
    server.signal("TERM");

    let status = server.wait_for_exit();
    assert!(
        status.success(),
        "a stop signal after registration must never be a hard kill, however early it lands. \
         Status: {status}. Log:\n{}",
        server.log()
    );
    assert!(
        server.log().contains("store closed cleanly"),
        "a stop before the first request is still a stop, and the store must still be flushed. \
         Log:\n{}",
        server.log()
    );
}

/// The registration happens before the port is announced, not after.
///
/// The ordering is the whole fix, and asserting it on the log is cheap. Without it, an operator
/// reading "listening=" and issuing `docker stop` is inside the window the test above closes.
#[test]
fn the_stop_signals_are_registered_before_the_server_announces_its_port() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let log = temp.path().join("order.log");
    let mut server = Server::start(temp.path(), &log, true);

    let log_text = server.log();
    let registered = log_text
        .find("stop signals registered")
        .expect("the registration must be logged");
    let listening = log_text
        .find("listening=")
        .expect("the bound port must be logged");
    assert!(
        registered < listening,
        "stop signals must be registered before the port is announced, or a client that reads \
         the port and stops the server is inside the window. Log:\n{log_text}"
    );

    server.signal("TERM");
    let status = server.wait_for_exit();
    assert!(status.success(), "Status: {status}. Log:\n{}", server.log());
}
