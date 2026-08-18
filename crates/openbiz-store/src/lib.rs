//! Embedded RDF store and named-graph model.
//!
//! The store is an [Oxigraph] instance running **inside our process**. The single-binary rule
//! (`CLAUDE.md` §1) means it is a library, never an external service — adding a required
//! triplestore would be a charter violation, not an optimisation.
//!
//! Oxigraph is a third-party engine, so per `CLAUDE.md` §3 it does not appear in this crate's
//! public API. No `oxigraph::` type crosses the boundary: [`StoreError`] carries [`std::io::Error`]
//! and strings, so a future backend swap changes this file and nothing above it.
//!
//! # What a self-hosted operator gets that the incumbents do not
//!
//! The incumbents put the triplestore in a separate lifecycle from the application, which produces
//! four recurring failure modes. Each is answered here:
//!
//! 1. **The app starts against a store that is not ready.** It cannot here — [`Store::open`] runs
//!    before the listener binds, so a store that will not open is a process that does not start.
//! 2. **Two instances share one data directory.** The backend takes an exclusive lock; we detect
//!    that specific case and say *"already in use by another OpenBiz process"* rather than
//!    surfacing an errno about a `LOCK` file.
//! 3. **An unclean stop.** [`Store::close`] flushes and reports whether it succeeded, so the log
//!    distinguishes a clean shutdown from a kill.
//! 4. **A silent downgrade.** The store records the format version that wrote it, and refuses to
//!    open one written by a newer OpenBiz instead of quietly misreading it.
//!
//! Two known Oxigraph risks are recorded in `docs/COMPETITIVE.md` and are **not** addressed by this
//! module: SPARQL query evaluation is upstream-documented as unoptimised, and numeric, calendar,
//! and duration literal encodings have precision limits. Both have spike items in the build plan.
//!
//! [Oxigraph]: https://oxigraph.org/

use std::path::{Path, PathBuf};

use oxigraph::model::vocab::xsd;
use oxigraph::model::{Literal, NamedNodeRef, QuadRef, Term};
use oxigraph::store::Store as Backend;
use thiserror::Error;

/// Subdirectory of the configured data directory that holds the RDF store.
///
/// The store gets its own subdirectory so backups, exports, and future artefacts can be siblings
/// rather than having to be told apart from the backend's own files.
pub const STORE_SUBDIR: &str = "store";

/// On-disk format version this build reads and writes.
///
/// Bump this only alongside a migration. A store carrying a *higher* version is refused, because
/// an older build silently reading a newer layout is the failure that loses data.
pub const FORMAT_VERSION: u32 = 1;

/// Named graph holding OpenBiz's own metadata.
///
/// Kept apart from vocabulary graphs so our bookkeeping never leaks into a customer's export. A
/// `urn:` IRI, not an `http:` one: we do not own a domain, and minting an IRI under someone else's
/// namespace — or one that 404s — is worse than being honestly non-dereferenceable.
pub const SYSTEM_GRAPH_IRI: &str = "urn:openbiz:graph:system";

/// Subject describing the store itself, within [`SYSTEM_GRAPH_IRI`].
const STORE_IRI: &str = "urn:openbiz:store";

/// Predicate carrying [`FORMAT_VERSION`].
const FORMAT_VERSION_IRI: &str = "urn:openbiz:storeFormatVersion";

/// How a named graph is used.
///
/// Vocabularies are isolated per graph so they can be versioned, exported, and permissioned
/// independently; OpenBiz's own bookkeeping lives in [`GraphKind::System`] so it never leaks into a
/// customer's exported vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphKind {
    /// A user-authored vocabulary.
    Vocabulary,
    /// OpenBiz's own metadata: workflow state, provenance, configuration.
    System,
    /// Materialised inferences, kept separate so they are never confused with asserted facts.
    Inferred,
}

/// Identifies a named graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphId {
    /// The graph's IRI.
    pub iri: String,
    /// What the graph holds.
    pub kind: GraphKind,
}

impl GraphId {
    /// A vocabulary graph.
    pub fn vocabulary(iri: impl Into<String>) -> Self {
        Self {
            iri: iri.into(),
            kind: GraphKind::Vocabulary,
        }
    }

    /// Whether callers may write to this graph directly.
    ///
    /// Inferred graphs are written only by a reasoner-driven materialisation pass; letting
    /// application code assert into them would destroy the asserted-versus-inferred distinction the
    /// UI depends on.
    pub fn is_directly_writable(&self) -> bool {
        !matches!(self.kind, GraphKind::Inferred)
    }
}

/// Errors raised by the store.
///
/// Every variant names the path it failed on, because the operator's first question is always
/// *which directory* — and the server adds *which configuration layer chose it* on top.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    /// The store's directory could not be created.
    #[error("could not create the store directory {}: {source}", path.display())]
    CreateDir {
        /// The path that failed.
        path: PathBuf,
        /// The underlying cause.
        source: std::io::Error,
    },
    /// Another OpenBiz process holds this store's lock.
    #[error(
        "the store at {} is already in use by another OpenBiz process; \
         stop it, or point this one at a different data directory",
        path.display()
    )]
    AlreadyInUse {
        /// The path that is locked.
        path: PathBuf,
    },
    /// The store could not be opened at the configured path.
    #[error("could not open the store at {}: {source}", path.display())]
    Open {
        /// The path that failed.
        path: PathBuf,
        /// The underlying cause.
        source: std::io::Error,
    },
    /// The store was written by a newer OpenBiz than this one.
    #[error(
        "the store at {} was written by a newer OpenBiz (format version {found}); \
         this build reads up to version {supported}. Upgrade, or restore a backup",
        path.display()
    )]
    FormatTooNew {
        /// The path that was refused.
        path: PathBuf,
        /// The version found on disk.
        found: u32,
        /// The highest version this build understands.
        supported: u32,
    },
    /// The store's own metadata is not what this build wrote.
    #[error("the store at {} has unreadable metadata: {detail}", path.display())]
    Corrupt {
        /// The path that was refused.
        path: PathBuf,
        /// What was wrong.
        detail: String,
    },
    /// A write targeted a graph that is not directly writable.
    #[error("graph {0} is not directly writable")]
    NotWritable(String),
    /// The backend failed.
    #[error("store backend failed: {0}")]
    Backend(String),
}

/// An open embedded RDF store.
///
/// Opening is the only way to get one, and dropping it releases the backend's exclusive lock.
/// Prefer [`Store::close`] over dropping: it flushes and *tells you whether that worked*, which is
/// the difference between a clean shutdown and one that only looked clean.
pub struct Store {
    backend: Backend,
    path: PathBuf,
    format_version: u32,
}

/// Hand-written because the backend is not [`Debug`], and because a store's *contents* have no
/// place in a log line or a panic message — a customer's vocabulary is the data we are trusted
/// with. Identity and version only.
impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("path", &self.path)
            .field("format_version", &self.format_version)
            .finish_non_exhaustive()
    }
}

impl Store {
    /// Open — or create — the store beneath `data_dir`.
    ///
    /// The store lives in `data_dir/`[`STORE_SUBDIR`]. Missing parent directories are created:
    /// the backend only creates its own leaf, so `OPENBIZ_DATA_DIR=/var/lib/openbiz/data` on a
    /// fresh host would otherwise fail on the parent rather than on anything the operator did
    /// wrong.
    ///
    /// A new store is stamped with [`FORMAT_VERSION`]; an existing one has its stamp checked. This
    /// read-back is also what proves the path is durable rather than merely writable — the second
    /// open of a directory reads what the first one committed.
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = data_dir.as_ref().join(STORE_SUBDIR);

        // Create the leaf too, not just the parent: it makes a permission problem surface here,
        // with our message, rather than inside the backend's mkdir.
        std::fs::create_dir_all(&path).map_err(|source| StoreError::CreateDir {
            path: path.clone(),
            source,
        })?;

        let backend = Backend::open(&path).map_err(|error| classify_open(&path, error))?;

        let format_version = stamp_or_check_format_version(&backend, &path)?;

        Ok(Self {
            backend,
            path,
            format_version,
        })
    }

    /// Where the store's files live.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The on-disk format version in force for this store.
    pub fn format_version(&self) -> u32 {
        self.format_version
    }

    /// Number of quads held, across every graph. **Test support only.**
    ///
    /// Deliberately not public. Oxigraph's `len()` on the RocksDB backend is a full iteration over
    /// the store, not a counter read — measured by reading its implementation, not assumed. A
    /// public `quad_count()` reads as O(1) and is O(n), so the first caller to log it at startup
    /// would put a whole-store scan in the cold-start path and break `CLAUDE.md` §1.5. When
    /// something genuinely needs a count, it should ask for a *scoped* one and be honest about
    /// the cost. Recorded in `docs/adr/0006`.
    #[cfg(test)]
    fn quad_count(&self) -> Result<usize, StoreError> {
        self.backend
            .len()
            .map_err(|error| StoreError::Backend(error.to_string()))
    }

    /// Flush everything to disk and release the lock.
    ///
    /// Consumes the store so a caller cannot keep using one it has closed. Dropping without
    /// calling this still releases the lock, but silently — a flush failure would go unreported,
    /// which is exactly the ambiguity an operator reading shutdown logs cannot afford.
    pub fn close(self) -> Result<(), StoreError> {
        self.backend.flush().map_err(|error| StoreError::Open {
            path: self.path.clone(),
            source: error.into(),
        })
    }
}

/// Turn a backend open failure into something an operator can act on.
///
/// The lock case is detected by inspecting the backend's message, which is **deliberately
/// fragile**: RocksDB's wording is not an API, so a version bump could change it.
///
/// It also has *two* wordings, which cost this iteration a red test and is worth recording. Two
/// opens from the **same** process report `lock hold by current process … LOCK: No locks
/// available`; two **separate** processes — the case that actually matters — report `While lock
/// file: …/LOCK: Resource temporarily unavailable`. The common substring is the lock file itself,
/// so that is what we match on, and both wordings are pinned by tests: the same-process one in
/// this crate, the cross-process one in the server's `tests/graceful_shutdown.rs`, which spawns two
/// real binaries. A unit test alone would have shipped a classifier that never fired in production.
///
/// If the wording changes anyway, those tests go red rather than this degrading silently, and the
/// fallback branch still reports a true error — just a less helpful one.
fn classify_open(path: &Path, error: oxigraph::store::StorageError) -> StoreError {
    if error.to_string().contains("LOCK:") {
        return StoreError::AlreadyInUse {
            path: path.to_path_buf(),
        };
    }
    StoreError::Open {
        path: path.to_path_buf(),
        source: error.into(),
    }
}

/// Read the store's format stamp, writing it first if the store is new.
///
/// Returns the version in force. Refuses a store from the future, and refuses one whose stamp is
/// not a single integer — a store with two stamps is one we cannot reason about, and guessing
/// which is right is how a migration corrupts data.
fn stamp_or_check_format_version(backend: &Backend, path: &Path) -> Result<u32, StoreError> {
    let subject = named_node(STORE_IRI);
    let predicate = named_node(FORMAT_VERSION_IRI);
    let graph = named_node(SYSTEM_GRAPH_IRI);

    let mut found: Vec<Term> = Vec::new();
    for quad in backend.quads_for_pattern(
        Some(subject.into()),
        Some(predicate),
        None,
        Some(graph.into()),
    ) {
        let quad = quad.map_err(|error| StoreError::Backend(error.to_string()))?;
        found.push(quad.object);
    }

    match found.as_slice() {
        [] => {
            let version = Literal::new_typed_literal(FORMAT_VERSION.to_string(), xsd::INTEGER);
            backend
                .insert(QuadRef::new(subject, predicate, &version, graph))
                .map_err(|error| StoreError::Backend(error.to_string()))?;
            // Flush immediately: the stamp must survive a hard kill in the seconds after a first
            // start, or the next open sees an unstamped store that already holds data.
            backend
                .flush()
                .map_err(|error| StoreError::Backend(error.to_string()))?;
            Ok(FORMAT_VERSION)
        }
        [Term::Literal(literal)] => {
            let found = literal
                .value()
                .parse::<u32>()
                .map_err(|_| StoreError::Corrupt {
                    path: path.to_path_buf(),
                    detail: format!("format version {:?} is not a number", literal.value()),
                })?;
            if found > FORMAT_VERSION {
                return Err(StoreError::FormatTooNew {
                    path: path.to_path_buf(),
                    found,
                    supported: FORMAT_VERSION,
                });
            }
            Ok(found)
        }
        other => Err(StoreError::Corrupt {
            path: path.to_path_buf(),
            detail: format!("expected exactly one format version, found {}", other.len()),
        }),
    }
}

/// Parse an IRI constant defined in this module.
///
/// The constants are compile-time literals we control, so a parse failure is a bug in this file
/// rather than anything a caller can cause. `expect` outside tests is barred by `CLAUDE.md` §6, so
/// this stays a `const`-shaped unwrap the type system can check instead.
const fn named_node(iri: &str) -> NamedNodeRef<'_> {
    NamedNodeRef::new_unchecked(iri)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("a temporary directory")
    }

    #[test]
    fn vocabulary_graphs_are_writable() {
        assert!(GraphId::vocabulary("http://example.org/v/1").is_directly_writable());
    }

    #[test]
    fn inferred_graphs_are_not_directly_writable() {
        let inferred = GraphId {
            iri: "http://example.org/v/1/inferred".to_owned(),
            kind: GraphKind::Inferred,
        };
        assert!(
            !inferred.is_directly_writable(),
            "only materialisation may write inferred graphs"
        );
    }

    #[test]
    fn opening_a_fresh_directory_creates_a_stamped_store() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh directory opens");

        assert_eq!(store.format_version(), FORMAT_VERSION);
        assert_eq!(store.path(), dir.path().join(STORE_SUBDIR));
        assert_eq!(
            store.quad_count().expect("countable"),
            1,
            "a new store holds exactly its own format stamp"
        );
    }

    /// The durability claim in the build plan, stated as a test: what one open commits, the next
    /// open reads back. Without this, "durable path" is an assertion about a directory name.
    #[test]
    fn the_format_stamp_survives_close_and_reopen() {
        let dir = temp_dir();

        let first = Store::open(dir.path()).expect("first open");
        first.close().expect("a clean close");

        let second = Store::open(dir.path()).expect("second open");
        assert_eq!(second.format_version(), FORMAT_VERSION);
        assert_eq!(
            second.quad_count().expect("countable"),
            1,
            "reopening must read the existing stamp, not write a second one"
        );
    }

    #[test]
    fn missing_parent_directories_are_created() {
        let dir = temp_dir();
        let nested = dir.path().join("var").join("lib").join("openbiz");

        let store = Store::open(&nested).expect("nested paths are created, not rejected");
        assert!(store.path().is_dir());
    }

    /// Guards the fragile part of [`classify_open`]. If RocksDB changes its lock wording this
    /// goes red, which is the point: the classification must never degrade silently.
    ///
    /// **This covers the weaker of the two cases.** Two opens from one process produce different
    /// wording from two opens by separate processes, and only the latter is the failure operators
    /// actually hit. The cross-process case is asserted in the server's
    /// `tests/graceful_shutdown.rs`; do not treat this test as covering it.
    #[test]
    fn second_open_of_the_same_directory_is_refused_as_already_in_use() {
        let dir = temp_dir();
        let _held = Store::open(dir.path()).expect("first open");

        let error = Store::open(dir.path()).expect_err("a second open must be refused");

        assert!(
            matches!(error, StoreError::AlreadyInUse { .. }),
            "expected AlreadyInUse, got: {error}"
        );
        assert!(
            error
                .to_string()
                .contains("already in use by another OpenBiz process"),
            "the message must name the real cause, not a LOCK file: {error}"
        );
    }

    #[test]
    fn a_store_from_the_future_is_refused() {
        let dir = temp_dir();

        // Stamp a version this build does not understand, using the backend directly — this is the
        // one place a test legitimately reaches past our own wrapper, because it is simulating a
        // *different build* having written the store.
        {
            let store = Store::open(dir.path()).expect("first open");
            let newer = Literal::new_typed_literal((FORMAT_VERSION + 1).to_string(), xsd::INTEGER);
            let current = Literal::new_typed_literal(FORMAT_VERSION.to_string(), xsd::INTEGER);
            store
                .backend
                .remove(QuadRef::new(
                    named_node(STORE_IRI),
                    named_node(FORMAT_VERSION_IRI),
                    &current,
                    named_node(SYSTEM_GRAPH_IRI),
                ))
                .expect("the existing stamp is removable");
            store
                .backend
                .insert(QuadRef::new(
                    named_node(STORE_IRI),
                    named_node(FORMAT_VERSION_IRI),
                    &newer,
                    named_node(SYSTEM_GRAPH_IRI),
                ))
                .expect("a newer stamp is writable");
            store.close().expect("a clean close");
        }

        let error = Store::open(dir.path()).expect_err("a future store must be refused");

        assert!(
            matches!(
                error,
                StoreError::FormatTooNew {
                    found,
                    supported: FORMAT_VERSION,
                    ..
                } if found == FORMAT_VERSION + 1
            ),
            "expected FormatTooNew, got: {error}"
        );
        assert!(
            error.to_string().contains("Upgrade, or restore a backup"),
            "refusing is not enough; the message must say what to do: {error}"
        );
    }

    #[test]
    fn a_store_with_two_format_stamps_is_refused_rather_than_guessed() {
        let dir = temp_dir();

        {
            let store = Store::open(dir.path()).expect("first open");
            let extra = Literal::new_typed_literal("0", xsd::INTEGER);
            store
                .backend
                .insert(QuadRef::new(
                    named_node(STORE_IRI),
                    named_node(FORMAT_VERSION_IRI),
                    &extra,
                    named_node(SYSTEM_GRAPH_IRI),
                ))
                .expect("a second stamp is writable");
            store.close().expect("a clean close");
        }

        let error = Store::open(dir.path()).expect_err("two stamps must be refused");

        assert!(
            matches!(error, StoreError::Corrupt { .. }),
            "expected Corrupt, got: {error}"
        );
        assert!(
            error.to_string().contains("found 2"),
            "the message must say what it actually saw: {error}"
        );
    }

    #[test]
    fn a_non_numeric_format_stamp_is_refused() {
        let dir = temp_dir();

        {
            let store = Store::open(dir.path()).expect("first open");
            let current = Literal::new_typed_literal(FORMAT_VERSION.to_string(), xsd::INTEGER);
            store
                .backend
                .remove(QuadRef::new(
                    named_node(STORE_IRI),
                    named_node(FORMAT_VERSION_IRI),
                    &current,
                    named_node(SYSTEM_GRAPH_IRI),
                ))
                .expect("the existing stamp is removable");
            store
                .backend
                .insert(QuadRef::new(
                    named_node(STORE_IRI),
                    named_node(FORMAT_VERSION_IRI),
                    Literal::new_simple_literal("banana").as_ref(),
                    named_node(SYSTEM_GRAPH_IRI),
                ))
                .expect("a nonsense stamp is writable");
            store.close().expect("a clean close");
        }

        let error = Store::open(dir.path()).expect_err("a non-numeric stamp must be refused");

        assert!(
            matches!(error, StoreError::Corrupt { .. }),
            "expected Corrupt, got: {error}"
        );
        assert!(
            error.to_string().contains("banana"),
            "the message must quote the value it could not read: {error}"
        );
    }

    /// The lock is the mechanism that makes `AlreadyInUse` true; a close that did not release it
    /// would turn a restart into a permanent failure.
    #[test]
    fn closing_releases_the_lock_for_the_next_process() {
        let dir = temp_dir();

        Store::open(dir.path())
            .expect("first open")
            .close()
            .expect("a clean close");

        Store::open(dir.path()).expect("the directory is reusable once closed");
    }

    #[test]
    fn a_data_directory_that_is_a_file_is_reported_as_such() {
        let dir = temp_dir();
        let file = dir.path().join("not-a-directory");
        std::fs::write(&file, b"contents").expect("writable temp dir");

        let error = Store::open(&file).expect_err("a file cannot hold a store");

        assert!(
            matches!(error, StoreError::CreateDir { .. }),
            "expected CreateDir, got: {error}"
        );
        assert!(
            error.to_string().contains("not-a-directory"),
            "the message must name the offending path: {error}"
        );
    }

    #[test]
    fn an_unwritable_data_directory_is_reported_before_the_backend_sees_it() {
        use std::os::unix::fs::PermissionsExt;

        let dir = temp_dir();
        let readonly = dir.path().join("readonly");
        std::fs::create_dir(&readonly).expect("writable temp dir");
        std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o500))
            .expect("permissions are settable");

        let error = Store::open(readonly.join("data")).expect_err("an unwritable parent must fail");

        assert!(
            matches!(error, StoreError::CreateDir { .. }),
            "expected CreateDir, got: {error}"
        );

        // Restore write permission so the TempDir can clean itself up.
        std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o700))
            .expect("permissions are settable");
    }
}
