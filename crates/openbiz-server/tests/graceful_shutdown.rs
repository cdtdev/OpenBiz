//! End-to-end store lifecycle, against the real `openbiz` binary as a child process.
//!
//! Everything here needs a *process*, not a router: signal disposition, exit status, the
//! backend's inter-process lock, and the wiring between `std::env` and `Config::load` are all
//! properties of a running program. Driving the library in-process would prove none of them.
//!
//! Unix-only. `SIGTERM` is the thing under test, and it does not exist elsewhere.
#![cfg(unix)]

use std::io::Read;
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

    fn wait_until_serving(&self) {
        let deadline = Instant::now() + PATIENCE;
        while Instant::now() < deadline {
            if let Some(addr) = self.listening_on() {
                if TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok() {
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
    assert!(server.wait_for_exit().success());
}
