//! Backup and restore end to end, against the real `openbiz` binary as a child process.
//!
//! The plan item asks for restore to be *verified against a live store*, and that is what this
//! file is: a backup goes in through the command line, and the vocabulary it carried comes back
//! out of a running server's own API. Driving the store in-process would prove the transaction and
//! nothing about whether an operator can actually use it — the arguments, the exit status, the
//! store lock, and the fact that a restored store is a store the server will open are all
//! properties of a *program*.
//!
//! The backup fixture below is **hand-written rather than produced by `openbiz backup`**, on
//! purpose. The claim being tested is that a backup is a portable, standard file: if the only
//! thing that can make one is us, that claim is untested and the format is free to drift into
//! something private. A human wrote these seven lines from the specification, and the product has
//! to accept them.
//!
//! Unix-only, matching `graceful_shutdown.rs`: it kills the server with a signal.
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// How long to wait for the child to start serving, or to stop.
const PATIENCE: Duration = Duration::from_secs(30);

/// A whole store as an operator could type it: OpenBiz's own registry, then one vocabulary.
///
/// Line by line: the format stamp that makes this a backup rather than an export; the system
/// graph's registration; the vocabulary's registration; and two statements of actual content.
const BACKUP: &str = concat!(
    "<urn:openbiz:store> <urn:openbiz:storeFormatVersion> ",
    "\"3\"^^<http://www.w3.org/2001/XMLSchema#integer> <urn:openbiz:graph:system> .\n",
    "<urn:openbiz:graph:system> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<urn:openbiz:graph:system> <urn:openbiz:graphKind> \"system\" <urn:openbiz:graph:system> .\n",
    "<https://example.org/regions> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<https://example.org/regions> <urn:openbiz:graphKind> \"vocabulary\" ",
    "<urn:openbiz:graph:system> .\n",
    "<https://example.org/regions/emea> ",
    "<http://www.w3.org/2004/02/skos/core#prefLabel> \"Europe, Middle East and Africa\"@en ",
    "<https://example.org/regions> .\n",
    "<https://example.org/regions/emea> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<http://www.w3.org/2004/02/skos/core#Concept> <https://example.org/regions> .\n",
);

/// The same store as [`BACKUP`], as a **format version 1** build would have written it: stamped 1,
/// and with no registry entry for the system graph — the invariant version 2 exists to guarantee.
///
/// Restoring this is the end-to-end proof that an older backup is migrated as it is read, through
/// the real binary rather than through the store's own tests.
const BACKUP_VERSION_1: &str = concat!(
    "<urn:openbiz:store> <urn:openbiz:storeFormatVersion> ",
    "\"1\"^^<http://www.w3.org/2001/XMLSchema#integer> <urn:openbiz:graph:system> .\n",
    "<https://example.org/regions> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<https://example.org/regions> <urn:openbiz:graphKind> \"vocabulary\" ",
    "<urn:openbiz:graph:system> .\n",
    "<https://example.org/regions/emea> ",
    "<http://www.w3.org/2004/02/skos/core#prefLabel> \"Europe, Middle East and Africa\"@en ",
    "<https://example.org/regions> .\n",
    "<https://example.org/regions/emea> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<http://www.w3.org/2004/02/skos/core#Concept> <https://example.org/regions> .\n",
);

/// The same store as [`BACKUP`], as a **format version 2** build wrote it.
///
/// Version 2 is version 3 in every byte except the stamp — the candidate seam's change to the
/// store was additive — so this fixture is the proof that the 2 → 3 step, which rewrites nothing,
/// nevertheless runs, reports itself, and leaves the store stamped where this build expects.
/// A migration that does nothing is exactly the one that could silently not happen.
const BACKUP_VERSION_2: &str = concat!(
    "<urn:openbiz:store> <urn:openbiz:storeFormatVersion> ",
    "\"2\"^^<http://www.w3.org/2001/XMLSchema#integer> <urn:openbiz:graph:system> .\n",
    "<urn:openbiz:graph:system> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<urn:openbiz:graph:system> <urn:openbiz:graphKind> \"system\" <urn:openbiz:graph:system> .\n",
    "<https://example.org/regions> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<https://example.org/regions> <urn:openbiz:graphKind> \"vocabulary\" ",
    "<urn:openbiz:graph:system> .\n",
    "<https://example.org/regions/emea> ",
    "<http://www.w3.org/2004/02/skos/core#prefLabel> \"Europe, Middle East and Africa\"@en ",
    "<https://example.org/regions> .\n",
    "<https://example.org/regions/emea> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<http://www.w3.org/2004/02/skos/core#Concept> <https://example.org/regions> .\n",
);

/// Run `openbiz <args>` against `data_dir` and wait for it to finish.
fn run(data_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openbiz"))
        .args(args)
        .current_dir(data_dir)
        .env("OPENBIZ_DATA_DIR", data_dir)
        .env("OPENBIZ_LOG", "info")
        .env_remove("OPENBIZ_CONFIG")
        .output()
        .expect("run the openbiz binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A spawned `openbiz` server, with its log captured to a file.
struct Server {
    child: Child,
    log_path: PathBuf,
}

impl Server {
    fn start(data_dir: &Path) -> Self {
        let log_path = data_dir.join("server.log");
        let log = std::fs::File::create(&log_path).expect("create the log file");
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

        let server = Self { child, log_path };
        server.wait_until_serving();
        server
    }

    fn log(&self) -> String {
        std::fs::read_to_string(&self.log_path).unwrap_or_default()
    }

    /// The address the child actually bound, read from its own startup log.
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

    /// `GET path`, returning the whole response as text.
    ///
    /// Hand-rolled HTTP/1.0 rather than a client dependency: one request, no keep-alive, no
    /// redirects. `Connection: close` is what lets the read run to EOF instead of needing a
    /// parser for chunked framing.
    fn get(&self, path: &str) -> String {
        let addr = self.listening_on().expect("the server logged its address");
        let mut socket = TcpStream::connect(addr).expect("connect to the server");
        socket
            .write_all(
                format!("GET {path} HTTP/1.0\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .expect("send the request");
        let mut response = String::new();
        socket
            .read_to_string(&mut response)
            .expect("read the response");
        response
    }

    fn stop(mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status();
        let deadline = Instant::now() + PATIENCE;
        while Instant::now() < deadline {
            if self.child.try_wait().expect("poll the child").is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("the server did not stop. Log:\n{}", self.log());
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Never leave a child holding a store lock behind a failed assertion.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The whole loop, in the order an operator would live it: restore a backup into a fresh data
/// directory, start the server on it, and find the vocabulary there — then back it up again.
#[test]
fn a_restored_backup_is_a_vocabulary_the_running_server_serves() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let data_dir = temp.path().join("data");
    std::fs::create_dir(&data_dir).expect("create the data directory");
    let file = temp.path().join("yesterday.nq");
    assert_eq!(
        openbiz_store::FORMAT_VERSION,
        3,
        "the fixture is a version-3 backup; bumping the format means writing the fixture for the \
         new one and adding an older-format test beside it, not editing this number"
    );
    std::fs::write(&file, BACKUP).expect("write the backup fixture");

    let restore = run(
        &data_dir,
        &["restore", file.to_str().expect("a UTF-8 path")],
    );
    assert!(
        restore.status.success(),
        "restore failed: {}{}",
        stdout(&restore),
        stderr(&restore)
    );
    // Six of the seven lines: the format stamp is checked, not rewritten.
    assert!(
        stdout(&restore).contains("restored 6 statements into 2 graphs"),
        "restore must report what it did, got {:?}",
        stdout(&restore)
    );
    // A script pipes stdout and must not have to filter log lines out of it.
    assert_eq!(
        stdout(&restore).lines().count(),
        1,
        "stdout is the result, and the logs belong on stderr: {:?}",
        stdout(&restore)
    );
    assert!(
        stderr(&restore).contains("store open"),
        "the provenance of the store it restored into must still be recorded: {}",
        stderr(&restore)
    );

    let server = Server::start(&data_dir);

    let graphs = server.get("/api/graphs");
    assert!(
        graphs.contains(r#"{"iri":"https://example.org/regions","kind":"vocabulary"}"#),
        "the restored vocabulary must be in the registry the server serves, as a vocabulary: \
         {graphs}"
    );
    // The registry is served whole and the *kind* is what separates ours from theirs. A restore
    // that got this wrong would put OpenBiz's own metadata graph in front of a user as something
    // they could edit.
    assert!(
        graphs.contains(r#"{"iri":"urn:openbiz:graph:system","kind":"system"}"#),
        "the system graph must survive the restore, still marked as ours: {graphs}"
    );

    let exported =
        server.get("/api/export?graph=https%3A%2F%2Fexample.org%2Fregions&format=turtle");
    assert!(
        exported.contains("Europe, Middle East and Africa"),
        "the restored statements must come back out of the export: {exported}"
    );
    assert!(
        !exported.contains("urn:openbiz:"),
        "the export of a restored vocabulary must not carry OpenBiz's bookkeeping: {exported}"
    );

    server.stop();

    // And the store that came back can be backed up again — the round trip closes.
    let second = temp.path().join("today.nq");
    let backup = run(
        &data_dir,
        &["backup", second.to_str().expect("a UTF-8 path")],
    );
    assert!(
        backup.status.success(),
        "backup failed: {}{}",
        stdout(&backup),
        stderr(&backup)
    );
    assert!(
        stdout(&backup).contains("backed up 7 statements from 2 graphs"),
        "backup must report what it wrote, got {:?}",
        stdout(&backup)
    );

    let written = std::fs::read_to_string(&second).expect("read the backup back");
    let mut lines: Vec<&str> = written.lines().collect();
    let mut expected: Vec<&str> = BACKUP.lines().collect();
    lines.sort_unstable();
    expected.sort_unstable();
    assert_eq!(
        lines, expected,
        "the backup of a restored store is not the file it was restored from"
    );
}

/// An operator restoring last year's backup onto this year's build: the file is brought forward
/// as it is read, the command says so, and the store that results is one the server serves.
#[test]
fn a_backup_from_an_older_format_is_migrated_as_it_is_restored() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let data_dir = temp.path().join("data");
    std::fs::create_dir(&data_dir).expect("create the data directory");
    let file = temp.path().join("last-year.nq");
    std::fs::write(&file, BACKUP_VERSION_1).expect("write the backup fixture");

    let restore = run(
        &data_dir,
        &["restore", file.to_str().expect("a UTF-8 path")],
    );
    assert!(
        restore.status.success(),
        "restore failed: {}{}",
        stdout(&restore),
        stderr(&restore)
    );

    // The operator is told their data was changed, and by what. A count alone would look
    // identical whether or not the file had been migrated.
    let said = stdout(&restore);
    assert!(
        said.contains("restored 4 statements into 2 graphs"),
        "restore must report what it did, got {said:?}"
    );
    assert!(
        said.contains("migrated the store format from version 1 to 3"),
        "a migration must be reported, not silently performed: {said:?}"
    );
    assert!(
        said.contains("registered the system graph"),
        "the report must say *why* the migration ran: {said:?}"
    );
    assert_eq!(
        said.lines().count(),
        1,
        "stdout is the result, and the logs belong on stderr: {said:?}"
    );

    // The migrated store is one the server opens without further work, and the vocabulary is
    // there — including the system-graph registration the file did not carry.
    let server = Server::start(&data_dir);
    let graphs = server.get("/api/graphs");
    assert!(
        graphs.contains(r#"{"iri":"https://example.org/regions","kind":"vocabulary"}"#),
        "the restored vocabulary must be in the registry the server serves: {graphs}"
    );
    assert!(
        graphs.contains(r#"{"iri":"urn:openbiz:graph:system","kind":"system"}"#),
        "the migration must have registered the system graph the file did not list: {graphs}"
    );
    server.stop();

    // And the migration is *in the store*, not only in a log line that has scrolled away: a
    // backup of the migrated store carries the record and the new stamp.
    let second = temp.path().join("today.nq");
    let backup = run(
        &data_dir,
        &["backup", second.to_str().expect("a UTF-8 path")],
    );
    assert!(
        backup.status.success(),
        "backup failed: {}{}",
        stdout(&backup),
        stderr(&backup)
    );
    let written = std::fs::read_to_string(&second).expect("read the backup back");
    assert!(
        written.contains(
            "<urn:openbiz:store> <urn:openbiz:storeFormatVersion> \
             \"3\"^^<http://www.w3.org/2001/XMLSchema#integer> <urn:openbiz:graph:system> ."
        ),
        "the migrated store must be stamped at the current version: {written}"
    );
    for step in [
        "urn:openbiz:migration:0002-register-system-graph",
        "urn:openbiz:migration:0003-allow-candidate-graphs",
    ] {
        assert!(
            written.contains(step),
            "every migration that ran must have left a record in the store: {step} is not in \
             {written}"
        );
    }
    assert!(
        written.contains("XMLSchema#dateTime"),
        "the record must say when the migration ran: {written}"
    );
    assert!(
        written.contains("Europe, Middle East and Africa"),
        "the content must have survived the migration: {written}"
    );

    // Opening it again is a no-op: a migration is a one-off, not something every start repeats.
    let again = run(
        &data_dir,
        &[
            "backup",
            temp.path().join("t.nq").to_str().expect("a UTF-8 path"),
        ],
    );
    assert!(
        !stderr(&again).contains("migrated the store format"),
        "the migration ran a second time: {}",
        stderr(&again)
    );
}

/// The file most likely to be in the way is the last good backup, and overwriting it with a
/// partial one turns a bad day into an unrecoverable one.
#[test]
fn a_backup_refuses_to_overwrite_an_existing_file() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let data_dir = temp.path().join("data");
    std::fs::create_dir(&data_dir).expect("create the data directory");
    let file = temp.path().join("precious.nq");
    std::fs::write(&file, "the last good backup").expect("write the existing file");

    let output = run(&data_dir, &["backup", file.to_str().expect("a UTF-8 path")]);

    assert!(
        !output.status.success(),
        "overwriting a backup must fail: {}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("never overwrites"),
        "the refusal must say why: {}",
        stderr(&output)
    );
    assert_eq!(
        std::fs::read_to_string(&file).expect("read the file back"),
        "the last good backup",
        "the existing backup was modified"
    );
}

/// A restore replaces a store. Merging one into a populated store would interleave two histories
/// with no way to separate them again, so it is refused rather than attempted.
#[test]
fn a_restore_into_a_store_that_already_holds_something_is_refused() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let data_dir = temp.path().join("data");
    std::fs::create_dir(&data_dir).expect("create the data directory");
    let file = temp.path().join("yesterday.nq");
    std::fs::write(&file, BACKUP).expect("write the backup fixture");

    let first = run(
        &data_dir,
        &["restore", file.to_str().expect("a UTF-8 path")],
    );
    assert!(first.status.success(), "the first restore must work");

    let second = run(
        &data_dir,
        &["restore", file.to_str().expect("a UTF-8 path")],
    );
    assert!(
        !second.status.success(),
        "a second restore must be refused: {}",
        stdout(&second)
    );
    assert!(
        stderr(&second).contains("is not empty"),
        "the refusal must say why, and what to do instead: {}",
        stderr(&second)
    );
    assert!(
        stderr(&second).contains("fresh data directory"),
        "the refusal must name the way forward: {}",
        stderr(&second)
    );
}

/// A typo must never leave an operator with a running server and the belief that a backup was
/// taken. The exit status distinguishes it from a failed operation, so a wrapper script can tell
/// "retry this" from "you typed it wrong".
#[test]
fn a_mistyped_command_exits_two_and_prints_the_usage() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let output = run(temp.path(), &["backupp", "/tmp/nowhere.nq"]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "a usage error has its own exit status: {}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("backupp"),
        "the message must quote what was typed: {}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("openbiz backup <file>"),
        "the usage must follow the complaint: {}",
        stderr(&output)
    );
    assert!(
        !temp.path().join("store").exists(),
        "a refused command must not create a store"
    );
}

/// `openbiz help` has to work before anything is configured — on a machine with no data
/// directory and no permission to make one, which is exactly where somebody types it.
#[test]
fn help_works_without_touching_the_store() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let output = run(temp.path(), &["help"]);

    assert!(output.status.success(), "help must exit zero");
    assert!(
        stdout(&output).contains("openbiz restore <file>"),
        "help goes to stdout: {}",
        stdout(&output)
    );
    assert!(
        !temp.path().join("store").exists(),
        "help must not create a store"
    );
}

/// An export is a single vocabulary with no registry, so restoring one would produce a store with
/// content nothing can describe. It is the likeliest wrong file to be handed, and it is refused by
/// what it *lacks* rather than by a syntax error — the message has to say so.
#[test]
fn restoring_an_export_instead_of_a_backup_is_refused_for_the_right_reason() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let data_dir = temp.path().join("data");
    std::fs::create_dir(&data_dir).expect("create the data directory");

    let file = temp.path().join("regions.nq");
    let export = BACKUP
        .lines()
        .filter(|line| line.contains("<https://example.org/regions>") && !line.contains("openbiz"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&file, format!("{export}\n")).expect("write the export");

    let output = run(
        &data_dir,
        &["restore", file.to_str().expect("a UTF-8 path")],
    );

    assert!(
        !output.status.success(),
        "an export is not a backup: {}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("not an OpenBiz backup"),
        "the refusal must name what the file is not: {}",
        stderr(&output)
    );
}

/// The migration that changes nothing still has to happen.
///
/// Format version 3 introduced the candidate seam. Every version-2 store was already a valid
/// version-3 store, so the step rewrites no bytes — which makes it the one migration that could
/// silently fail to run and leave no trace anybody would notice, until an older build read the
/// store and reported its registry as corrupt instead of saying "upgrade". This restores a real
/// version-2 file through the real binary and checks the stamp moved and the step said so.
#[test]
fn a_version_two_backup_is_brought_forward_by_a_migration_that_rewrites_nothing() {
    let temp = tempfile::tempdir().expect("a temporary directory");
    let data_dir = temp.path().join("data");
    std::fs::create_dir(&data_dir).expect("create the data directory");
    let file = temp.path().join("version-2.nq");
    std::fs::write(&file, BACKUP_VERSION_2).expect("write the backup fixture");

    let restore = run(
        &data_dir,
        &["restore", file.to_str().expect("a UTF-8 path")],
    );
    assert!(
        restore.status.success(),
        "restore failed: {}{}",
        stdout(&restore),
        stderr(&restore)
    );

    let said = stdout(&restore);
    assert!(
        said.contains("migrated the store format from version 2 to 3"),
        "a migration that writes nothing must still be reported: {said:?}"
    );
    assert!(
        said.contains("candidate graphs"),
        "and must still say why it exists: {said:?}"
    );

    let second = temp.path().join("today.nq");
    let backup = run(
        &data_dir,
        &["backup", second.to_str().expect("a UTF-8 path")],
    );
    assert!(
        backup.status.success(),
        "backup failed: {}",
        stderr(&backup)
    );
    let written = std::fs::read_to_string(&second).expect("read the backup back");
    assert!(
        written.contains(
            "<urn:openbiz:store> <urn:openbiz:storeFormatVersion> \
             \"3\"^^<http://www.w3.org/2001/XMLSchema#integer> <urn:openbiz:graph:system> ."
        ),
        "the store must be stamped at the current version: {written}"
    );
    assert!(
        written.contains("urn:openbiz:migration:0003-allow-candidate-graphs"),
        "the step must have left a record even though it wrote no data: {written}"
    );
    assert!(
        written.contains("Europe, Middle East and Africa"),
        "the content must have survived: {written}"
    );
}
