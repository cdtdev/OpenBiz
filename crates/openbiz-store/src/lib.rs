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

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use oxigraph::io::RdfSerializer;
use oxigraph::model::vocab::{rdf, xsd};
use oxigraph::model::{GraphName, Literal, NamedNode, NamedNodeRef, Quad, Term};
use oxigraph::store::Store as Backend;
use thiserror::Error;

mod backup;
mod graph;
mod query;
mod results;
/// Conformance of the line-based exports against the specifications' own grammars. Test-only:
/// it holds a reader written from the published EBNF, which exists to disagree with the one we
/// ship. See the module's own documentation for why round-trip fidelity is not conformance.
#[cfg(test)]
mod spec_conformance;
mod syntax;

/// What Oxigraph's query evaluation costs at 10k, 100k, and 1M concepts, measured through our own
/// query entry point against the queries the interface will issue. Test-only: it generates and
/// loads its own vocabulary, and the sizes that take minutes are `#[ignore]`d. See
/// `docs/adr/0013-oxigraph-query-scale.md` for the numbers.
#[cfg(test)]
mod scale;

/// Where a typed literal stops being a value: the ranges the backend interprets, what it does at
/// the edge of each, and the datatype substitution found while measuring them. Test-only. See
/// `docs/adr/0014-literal-precision-boundaries.md` for the decision.
#[cfg(test)]
mod literal_precision;

pub use backup::{BackupReport, RestoreReport, BACKUP_SYNTAX};
pub use query::{QueryFormats, QueryLimits, QueryReport, QueryShape};
pub use results::ResultsSyntax;
pub use syntax::RdfSyntax;

pub use graph::{
    GraphId, GraphIdError, GraphKind, INFERRED_GRAPH_PREFIX, OPENBIZ_NAMESPACE, SYSTEM_GRAPH_IRI,
};

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

/// Subject describing the store itself, within the system graph.
const STORE_IRI: &str = "urn:openbiz:store";

/// Predicate carrying [`FORMAT_VERSION`].
const FORMAT_VERSION_IRI: &str = "urn:openbiz:storeFormatVersion";

/// Class every registered graph is typed with, in the system graph.
const GRAPH_CLASS_IRI: &str = "urn:openbiz:Graph";

/// Predicate carrying a registered graph's [`GraphKind`].
const GRAPH_KIND_IRI: &str = "urn:openbiz:graphKind";

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
    #[error(
        "graph {0} is not directly writable: it holds materialised inferences, which only a \
         reasoner may assert"
    )]
    NotWritable(String),
    /// A graph was created at an IRI that is already registered.
    #[error(
        "a graph is already registered at {iri}; reuse or extend it rather than creating a \
         second one at the same IRI"
    )]
    GraphExists {
        /// The IRI that is already taken.
        iri: String,
    },
    /// A transaction was started from inside another transaction on the same store.
    #[error(
        "a write transaction is already open on this store from this thread; \
         a transaction cannot be nested inside another"
    )]
    NestedTransaction,
    /// A graph was asked for that the registry does not know about.
    ///
    /// Distinct from "the graph is empty" on purpose. Handing back an empty export for a
    /// vocabulary that does not exist is the failure mode this variant exists to prevent: the
    /// caller gets a well-formed, valid, entirely wrong file and no reason to doubt it.
    #[error("no graph is registered at {iri}")]
    NoSuchGraph {
        /// The IRI that is not registered.
        iri: String,
    },
    /// A graph could not be written out in the requested syntax.
    #[error("the graph {iri} could not be written as {syntax}: {source}")]
    Export {
        /// The graph being exported.
        iri: String,
        /// The syntax it was being written in.
        syntax: RdfSyntax,
        /// The underlying cause — usually the caller's writer failing, not the store.
        #[source]
        source: std::io::Error,
    },
    /// The backup could not be written.
    #[error("the backup could not be written: {source}")]
    Backup {
        /// The underlying cause — usually the caller's writer failing, not the store.
        #[source]
        source: std::io::Error,
    },
    /// The backup file could not be read.
    #[error("the backup could not be read: {source}")]
    RestoreRead {
        /// The underlying cause.
        #[source]
        source: std::io::Error,
    },
    /// The backup file is not well-formed in the syntax a backup is written in.
    ///
    /// Carries the position, because a backup is a file a human may have edited, concatenated, or
    /// truncated, and "it is invalid somewhere" is not something an operator can act on.
    #[error("the backup is not valid {BACKUP_SYNTAX}{}: {detail}", match line {
        Some(line) => format!(" at line {line}"),
        None => String::new(),
    })]
    RestoreSyntax {
        /// One-based line the parser stopped at, when it reported one.
        line: Option<u64>,
        /// The parser's complaint, verbatim.
        detail: String,
    },
    /// A restore was asked for into a store that already holds something.
    ///
    /// A restore replaces a whole store, so merging it into a populated one would interleave two
    /// unrelated histories with no way to tell them apart afterwards. Refusing keeps the
    /// destructive reading — "this overwrote my vocabulary" — impossible rather than merely
    /// unlikely.
    #[error(
        "the store at {} is not empty, and a restore replaces a whole store rather than merging          into one; restore into a fresh data directory instead",
        path.display()
    )]
    RestoreNotEmpty {
        /// The store that was refused.
        path: PathBuf,
    },
    /// The file offered as a backup is not one.
    ///
    /// Distinct from a syntax error on purpose: the file may be perfectly good RDF. It is an
    /// *export* — a single vocabulary — being handed to a whole-store restore, and the difference
    /// matters because restoring an export would produce a store with content and no registry.
    #[error("that file is not an OpenBiz backup: {detail}")]
    NotABackup {
        /// What was missing or wrong.
        detail: String,
    },
    /// The backup was written by a newer OpenBiz than this one.
    #[error(
        "the backup was written by a newer OpenBiz (format version {found}); this build reads up          to version {supported}. Restore it with that build, or upgrade this one"
    )]
    RestoreFormatTooNew {
        /// The version found in the file.
        found: u32,
        /// The highest version this build understands.
        supported: u32,
    },
    /// The backup predates this build's store format.
    #[error(
        "the backup is in store format version {found} and this build writes version          {supported}; migrating an older backup is not implemented yet, so restoring it would          produce a store this build reads incorrectly"
    )]
    RestoreNeedsMigration {
        /// The version found in the file.
        found: u32,
        /// The version this build writes.
        supported: u32,
    },
    /// The backup describes a store this build would refuse to open.
    ///
    /// Raised *before* the restore commits, so the refusal leaves the target store exactly as it
    /// was. A backup that restores into an unopenable store is the worst outcome available here:
    /// the operator has already lost the original, which is why they are restoring.
    #[error("the backup would not restore into a store this build can open: {detail}")]
    RestoreRefused {
        /// What about the file was unreconstructable.
        detail: String,
    },
    /// The text offered as a query is not valid SPARQL 1.1.
    ///
    /// Carries the parser's own words, which is safe here in a way it is not elsewhere: the detail
    /// is about the *caller's own query text*, not about the customer's data or their deployment.
    /// A caller who cannot see where their query is wrong will guess.
    #[error("that is not a valid SPARQL 1.1 query: {detail}")]
    QuerySyntax {
        /// The parser's complaint, verbatim.
        detail: String,
    },
    /// The text offered as a query parses as a SPARQL Update.
    ///
    /// Distinct from [`StoreError::QuerySyntax`] because the caller's mistake is different and so
    /// is the fix. An update refused as a syntax error sends someone hunting for a typo in text
    /// that has none.
    #[error("that is a SPARQL Update, not a query, and querying never writes")]
    QueryIsUpdate,
    /// A query answered with more than it was allowed to.
    ///
    /// A refusal rather than a truncation, on purpose. Handing back the first `limit` rows of a
    /// larger answer produces a document that is complete-looking, valid, and wrong — and in a
    /// governance tool it is wrong in the direction of "the row you were looking for is not here".
    #[error("the query answered with more than {limit} results")]
    QueryTooLarge {
        /// The cap that was exceeded.
        limit: u64,
    },
    /// A query ran past its deadline and was cancelled.
    #[error("the query ran for longer than it is allowed to and was cancelled")]
    QueryTimedOut,
    /// A query needs SPARQL 1.1 Federated Query, which this build does not have.
    ///
    /// Not a failure and not a fault: it is a capability this binary was deliberately built
    /// without. Oxigraph is linked with its `http-client` feature off (`docs/adr/0006`) so that
    /// nothing in the product can open an outbound connection, which is what makes `CLAUDE.md`
    /// §1.1's air-gapped operation a property of the binary rather than a promise. A caller told
    /// only "the query failed" would reasonably retry it.
    #[error(
        "the query uses SERVICE, and this build has no federated query: it is compiled without an \
         HTTP client so that it can run air-gapped"
    )]
    QueryNeedsFederation,
    /// A query could not be evaluated.
    #[error("the query could not be evaluated: {detail}")]
    QueryFailed {
        /// The evaluator's complaint.
        detail: String,
    },
    /// An answer could not be written out.
    #[error("the query's answer could not be written: {source}")]
    AnswerWrite {
        /// The underlying cause — usually the caller's writer failing, not the store.
        #[source]
        source: std::io::Error,
    },
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
    /// Serialises write transactions, so a read-modify-write is atomic against other writers.
    ///
    /// The backend does **not** do this for us. Its transaction is a snapshot plus an in-memory
    /// write batch, and committing is an unconditional write of that batch — there is no
    /// conflict detection and no validation that the snapshot is still current. Two transactions
    /// that both read "this IRI is free" therefore both commit, and the second silently
    /// overwrites the first's decision. That is a lost update, and in a governance product a
    /// lost update is an approval that vanishes.
    ///
    /// Serialising writers is cheap here in a way it would not be for a shared server: the
    /// single-binary rule (`CLAUDE.md` §1.2) means exactly one process owns this store, and the
    /// backend's exclusive file lock enforces it. Readers never take this lock, so concurrent
    /// reads stay concurrent. See `docs/adr/0009`.
    writes: Mutex<()>,
}

thread_local! {
    /// Addresses of the stores this thread already has a write transaction open on.
    ///
    /// [`Store::writes`] is not reentrant, so a transaction opened inside another transaction on
    /// the same store would block forever against itself. A silent deadlock in the write path is
    /// the worst possible failure — no error, no log line, just a request that never returns —
    /// so we detect the case and refuse it. Keyed by store address rather than by a global flag
    /// so a process holding two stores over two data directories is not falsely refused.
    static OPEN_TRANSACTIONS: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

/// Held for the duration of a write transaction; releases the lock and the reentrancy mark.
struct WriteAccess<'a> {
    _lock: MutexGuard<'a, ()>,
    store: usize,
}

impl Drop for WriteAccess<'_> {
    fn drop(&mut self) {
        let store = self.store;
        OPEN_TRANSACTIONS.with_borrow_mut(|open| open.retain(|held| *held != store));
    }
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

        let mut store = Self {
            backend,
            path,
            format_version: FORMAT_VERSION,
            writes: Mutex::new(()),
        };

        // Stamping the format version and registering the system graph happen in **one**
        // transaction, because a store that is stamped but has no system graph in its registry is
        // a store this build would report as inconsistent. Before transactions existed these were
        // two independent writes, and a kill in the gap between them left exactly that state on
        // disk — a first start is the likeliest moment for a container to be killed, so the gap
        // was not hypothetical.
        //
        // The system graph is registered on **every** open, not only when the store is created,
        // so a store written before the registry existed acquires one by being opened. That is
        // additive, so it needs no format bump and no migration, and an older build reading the
        // same store simply ignores quads it does not look for.
        let (format_version, wrote) = store.transaction(|txn| {
            let (version, stamped) = stamp_or_check_format_version(txn, &store.path)?;
            let registered = txn.ensure_registered(&GraphId::system())?;
            Ok((version, stamped || registered))
        })?;

        store.format_version = format_version;

        if wrote {
            // Flush immediately: the stamp and the registry must survive a hard kill in the
            // seconds after a first start, or the next open sees an unstamped store that already
            // holds data.
            store
                .backend
                .flush()
                .map_err(|error| StoreError::Backend(error.to_string()))?;
        }

        Ok(store)
    }

    /// Where the store's files live.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The on-disk format version in force for this store.
    pub fn format_version(&self) -> u32 {
        self.format_version
    }

    /// Every graph this store knows about, ordered by IRI.
    ///
    /// Read from the registry in the system graph, not by scanning the store: asking the backend
    /// which graphs contain quads would be a whole-store scan, and it would also miss a
    /// vocabulary that has been created but not yet populated — which is exactly the vocabulary a
    /// user is looking at when they wonder where it went.
    ///
    /// A registry entry that does not satisfy the [`GraphId`] invariants is a [`StoreError::Corrupt`],
    /// never a silently skipped row. A graph we cannot describe is one we must not pretend is absent.
    pub fn graphs(&self) -> Result<Vec<GraphId>, StoreError> {
        graphs_in(&self.backend, &self.path)
    }

    /// Whether a graph is registered at `iri`, whatever kind it was registered as.
    ///
    /// Kind-blind on purpose: this answers "is this IRI taken", which is the question the creation
    /// path needs, and a caller that wants the kind should ask [`Store::graphs`].
    pub fn contains_graph(&self, iri: &str) -> Result<bool, StoreError> {
        contains_graph_in(&self.backend, iri)
    }

    /// Write one graph's contents to `writer` in `syntax`.
    ///
    /// # What is in the file, exactly
    ///
    /// The statements of that one graph. Nothing else — no OpenBiz bookkeeping, no other
    /// vocabulary, no materialised inference. That is not achieved by filtering on the way out; it
    /// is the named-graph model (`docs/adr/0007`) paying for itself: our own metadata lives in the
    /// system graph and inferences live in a derived graph, so a vocabulary export cannot contain
    /// them because they were never in the vocabulary. This is the round-trip guarantee `CLAUDE.md`
    /// §1.3 requires — the incumbents that keep project metadata alongside content produce exports
    /// a standards-compliant consumer has to be *told* to ignore parts of.
    ///
    /// The graph *name* is another matter, and an honest one:
    /// [`RdfSyntax::records_graph_names`] is false for Turtle, N-Triples, and RDF/XML, because a
    /// triple syntax has nowhere to put it. In those three the statements are written to the
    /// default graph and the IRI is not in the file. Callers are expected to say so rather than
    /// let a user find out when the re-import lands somewhere unexpected.
    ///
    /// # Refusals
    ///
    /// An IRI with no registry entry is [`StoreError::NoSuchGraph`], never an empty file. A
    /// *registered but empty* graph is an empty file, which is the correct and different answer —
    /// a vocabulary that has been created and not yet populated genuinely has no statements.
    ///
    /// # Cost
    ///
    /// Streams: quads go to `writer` as they are read, so peak memory is one quad rather than the
    /// whole graph. Takes no write lock, so an export never blocks an author and an author never
    /// blocks an export — and it does not need one, because the backend's iterator holds a single
    /// snapshot for the whole scan (read from its source, not assumed). A commit landing mid-export
    /// therefore cannot tear the file: the export is the graph as it stood when the scan began.
    ///
    /// The registry check runs on its own, earlier snapshot, so an export can in principle be of a
    /// graph deregistered a moment ago. Nothing deregisters a graph in this build; when something
    /// does, the check belongs inside a read transaction. Recorded in `docs/UNTESTED.md`.
    pub fn export_graph(
        &self,
        iri: &str,
        syntax: RdfSyntax,
        writer: impl std::io::Write,
    ) -> Result<(), StoreError> {
        // The registry is the authority on what exists, exactly as it is for `graphs()`. Asking
        // the backend whether any quad names this graph would report a created-but-empty
        // vocabulary as absent, which is the vocabulary a user is most likely to be looking for.
        let (Ok(graph_name), true) = (NamedNode::new(iri), self.contains_graph(iri)?) else {
            return Err(StoreError::NoSuchGraph {
                iri: iri.to_owned(),
            });
        };

        let failed = |source| StoreError::Export {
            iri: iri.to_owned(),
            syntax,
            source,
        };

        let mut serializer = RdfSerializer::from_format(syntax.backend()).for_writer(writer);
        for quad in
            self.backend
                .quads_for_pattern(None, None, None, Some(graph_name.as_ref().into()))
        {
            let quad = quad.map_err(|error| StoreError::Backend(error.to_string()))?;

            // Two calls rather than one because the engine's `serialize_quad` *errors* on a named
            // graph in a triple syntax rather than dropping it. Choosing per syntax is what turns
            // that into the documented lossy-but-successful behaviour above.
            if syntax.records_graph_names() {
                serializer.serialize_quad(&quad).map_err(failed)?;
            } else {
                serializer.serialize_triple(quad.as_ref()).map_err(failed)?;
            }
        }
        serializer.finish().map_err(failed)?;

        Ok(())
    }

    /// Register a new vocabulary graph.
    ///
    /// Refuses an IRI that is already registered rather than quietly adopting it. Silently
    /// succeeding here is how a user ends up with two vocabularies believing they own one graph,
    /// and it is the store-level face of `CLAUDE.md` §1.7 — creating something new where something
    /// existing would serve is the failure mode this product exists to attack, so the store never
    /// makes it the path of least resistance.
    ///
    /// This creates the *container*, not its contents; a freshly created vocabulary graph holds no
    /// quads. The discovery-first creation path (§1.7) and the authoring API sit above this.
    pub fn create_vocabulary_graph(&self, graph: &GraphId) -> Result<(), StoreError> {
        self.transaction(|txn| txn.create_vocabulary_graph(graph))
    }

    /// Run `work` as a single all-or-nothing write.
    ///
    /// Everything `work` writes lands together or not at all. Returning `Err` — or panicking —
    /// discards every write it made, so a caller cannot leave the store half-changed by giving
    /// up partway. That is the whole point of the closure: the backend's own transaction handle
    /// rolls back when dropped, which means the *safe* outcome is the one a forgetful caller gets
    /// by default, whereas a `begin`/`commit` pair makes silently-never-committed the default
    /// instead.
    ///
    /// Writers are serialised (see [`Store::writes`]) and readers are not, so `work` blocks other
    /// writers for its duration and blocks nobody else. Keep it short: a transaction holds its
    /// whole change set in memory, and it is the one thing in this store that other writers wait
    /// behind.
    ///
    /// Reads made *inside* `work` see the store as it was when the transaction began, plus that
    /// transaction's own uncommitted writes — so a check-then-write inside one transaction is
    /// sound, which is exactly what [`Store::create_vocabulary_graph`] relies on.
    ///
    /// Nesting a transaction inside another on the same store returns
    /// [`StoreError::NestedTransaction`] rather than deadlocking against the lock this one holds.
    pub fn transaction<T>(
        &self,
        work: impl FnOnce(&mut Transaction<'_>) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let _access = self.begin_write()?;

        let mut transaction = Transaction {
            inner: self
                .backend
                .start_transaction()
                .map_err(|error| StoreError::Backend(error.to_string()))?,
        };

        // `?` here is the rollback: it drops `transaction` on the way out, and the backend
        // discards an uncommitted transaction's write batch. A panic inside `work` unwinds
        // through the same drop, so that rolls back too.
        let outcome = work(&mut transaction)?;

        transaction
            .inner
            .commit()
            .map_err(|error| StoreError::Backend(error.to_string()))?;

        Ok(outcome)
    }

    /// Take exclusive write access, refusing a nested transaction rather than deadlocking on it.
    fn begin_write(&self) -> Result<WriteAccess<'_>, StoreError> {
        let store = std::ptr::from_ref(self) as usize;

        if OPEN_TRANSACTIONS.with_borrow(|open| open.contains(&store)) {
            return Err(StoreError::NestedTransaction);
        }

        // Recover from poisoning instead of propagating it. The mutex guards no data — it is a
        // serialisation token — and a panic inside a transaction leaves the *store* untouched,
        // because unwinding drops the transaction and discards its writes. Refusing every later
        // write because an earlier one panicked would turn a rolled-back edit into a store that
        // has silently gone read-only.
        let lock = self
            .writes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        OPEN_TRANSACTIONS.with_borrow_mut(|open| open.push(store));

        Ok(WriteAccess { _lock: lock, store })
    }

    /// A [`StoreError::Corrupt`] naming this store's path.
    fn corrupt(&self, detail: String) -> StoreError {
        StoreError::Corrupt {
            path: self.path.clone(),
            detail,
        }
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
/// Returns the version in force and whether this call stamped it. Refuses a store from the
/// future, and refuses one whose stamp is not a single integer — a store with two stamps is one
/// we cannot reason about, and guessing which is right is how a migration corrupts data.
///
/// Runs inside the caller's transaction so that refusing a store is a decision taken against a
/// single consistent snapshot, and so that a stamp written here commits together with the system
/// graph's registry entry rather than as a separate write that a kill could land without it.
fn stamp_or_check_format_version(
    transaction: &mut Transaction<'_>,
    path: &Path,
) -> Result<(u32, bool), StoreError> {
    let subject = named_node(STORE_IRI);
    let predicate = named_node(FORMAT_VERSION_IRI);

    let found: Vec<Term> = transaction
        .inner
        .system_quads(Some(subject), predicate)?
        .into_iter()
        .map(|quad| quad.object)
        .collect();

    match found.as_slice() {
        [] => {
            let version = Literal::new_typed_literal(FORMAT_VERSION.to_string(), xsd::INTEGER);
            transaction.insert(
                &GraphId::system(),
                vec![(subject.into_owned(), predicate.into_owned(), version.into())],
            )?;
            Ok((FORMAT_VERSION, true))
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
            Ok((found, false))
        }
        other => Err(StoreError::Corrupt {
            path: path.to_path_buf(),
            detail: format!("expected exactly one format version, found {}", other.len()),
        }),
    }
}

/// A write in progress.
///
/// Obtained only from [`Store::transaction`], and everything written through it lands together
/// when that call commits or vanishes entirely if it does not. The backend type it wraps is
/// deliberately private: `CLAUDE.md` §3 keeps third-party engine types out of our API so the
/// engine stays swappable.
///
/// Its read methods see the store as it stood when the transaction opened, **plus** this
/// transaction's own uncommitted writes. That is what makes a check-then-write inside one
/// transaction sound where the same pair of calls against [`Store`] would race.
pub struct Transaction<'a> {
    inner: oxigraph::store::Transaction<'a>,
}

impl Transaction<'_> {
    /// Whether a graph is registered at `iri`, including one registered by this transaction.
    pub fn contains_graph(&self, iri: &str) -> Result<bool, StoreError> {
        contains_graph_in(&self.inner, iri)
    }

    /// Register a new vocabulary graph, as part of this transaction.
    ///
    /// Refuses an IRI that is already registered rather than quietly adopting it. Silently
    /// succeeding here is how a user ends up with two vocabularies believing they own one graph,
    /// and it is the store-level face of `CLAUDE.md` §1.7 — creating something new where
    /// something existing would serve is the failure mode this product exists to attack, so the
    /// store never makes it the path of least resistance.
    ///
    /// The check and the write are one atomic step because they are in one transaction. They were
    /// not always: two callers racing on the same IRI both used to read "free" and both write,
    /// leaving the IRI registered twice — which does not merely duplicate a row, it makes
    /// [`Store::graphs`] refuse the whole registry as [`StoreError::Corrupt`]. One user's
    /// mistimed second click took the entire vocabulary list down.
    ///
    /// This creates the *container*, not its contents; a freshly created vocabulary graph holds
    /// no quads. The discovery-first creation path (§1.7) and the authoring API sit above this.
    pub fn create_vocabulary_graph(&mut self, graph: &GraphId) -> Result<(), StoreError> {
        if !graph.is_directly_writable() {
            return Err(StoreError::NotWritable(graph.iri().to_owned()));
        }
        if self.contains_graph(graph.iri())? {
            return Err(StoreError::GraphExists {
                iri: graph.iri().to_owned(),
            });
        }
        self.register(graph)
    }

    /// Register `graph` if it is not already registered. Reports whether it wrote anything.
    fn ensure_registered(&mut self, graph: &GraphId) -> Result<bool, StoreError> {
        if self.contains_graph(graph.iri())? {
            return Ok(false);
        }
        self.register(graph)?;
        Ok(true)
    }

    /// Write a graph's registry entry into the system graph.
    fn register(&mut self, graph: &GraphId) -> Result<(), StoreError> {
        let subject = NamedNode::new_unchecked(graph.iri());
        self.insert(
            &GraphId::system(),
            vec![
                (
                    subject.clone(),
                    rdf::TYPE.into_owned(),
                    named_node(GRAPH_CLASS_IRI).into_owned().into(),
                ),
                (
                    subject,
                    named_node(GRAPH_KIND_IRI).into_owned(),
                    Literal::new_simple_literal(graph.kind().as_str()).into(),
                ),
            ],
        )
    }

    /// The single point through which every write to the store passes.
    ///
    /// Two things are true of it and neither is negotiable. **Every quad names a graph** —
    /// nothing is written to the default graph, so no statement can exist without a vocabulary it
    /// belongs to. And **the target graph must be directly writable**, so
    /// `GraphId::is_directly_writable` is a rule the store enforces rather than a comment a
    /// caller may forget.
    ///
    /// Since transactions became the only way to write, this is the choke point in a stronger
    /// sense than before: there is no non-transactional path to route around it. Today's writes
    /// are all to the system graph, so the refusal branch does not yet fire in production; the
    /// point of putting the choke point in now is that the first import, materialisation, or
    /// agent proposal to arrive cannot bypass it.
    ///
    /// Not public: the triple type here is the backend's, and §3 keeps that out of our API. The
    /// public write vocabulary is the domain methods above, and it grows when Phase 1's RDF
    /// parsing item gives us a term model of our own.
    ///
    /// The writability check is a **runtime refusal**, not a debug assertion. A caller that has
    /// already checked makes it redundant, and that is the point: the rule must hold for the
    /// caller who has not, including one added in a later phase by someone reading only this
    /// method's signature.
    fn insert(
        &mut self,
        graph: &GraphId,
        triples: Vec<(NamedNode, NamedNode, Term)>,
    ) -> Result<(), StoreError> {
        if !graph.is_directly_writable() {
            return Err(StoreError::NotWritable(graph.iri().to_owned()));
        }

        // Unchecked because `GraphId` validated this IRI through the same parser when it was
        // constructed, and its fields are private so nothing can have changed it since. `expect`
        // is barred outside tests (`CLAUDE.md` §6) and there is no error here to propagate.
        let graph_name: GraphName = NamedNode::new_unchecked(graph.iri()).into();

        let quads: Vec<Quad> = triples
            .into_iter()
            .map(|(subject, predicate, object)| {
                Quad::new(subject, predicate, object, graph_name.clone())
            })
            .collect();

        self.inner.extend(&quads);

        Ok(())
    }
}

/// Anything the registry can be read out of: the store itself, or a transaction over it.
///
/// One implementation of each registry read, used by both, so a rule enforced against the store
/// cannot quietly differ from the same rule enforced inside a transaction.
trait RegistryReader {
    /// Every quad in the system graph with this `predicate`, optionally narrowed to one subject.
    fn system_quads(
        &self,
        subject: Option<NamedNodeRef<'_>>,
        predicate: NamedNodeRef<'_>,
    ) -> Result<Vec<Quad>, StoreError>;
}

impl RegistryReader for Backend {
    fn system_quads(
        &self,
        subject: Option<NamedNodeRef<'_>>,
        predicate: NamedNodeRef<'_>,
    ) -> Result<Vec<Quad>, StoreError> {
        self.quads_for_pattern(
            subject.map(Into::into),
            Some(predicate),
            None,
            Some(named_node(SYSTEM_GRAPH_IRI).into()),
        )
        .map(|quad| quad.map_err(|error| StoreError::Backend(error.to_string())))
        .collect()
    }
}

impl RegistryReader for oxigraph::store::Transaction<'_> {
    fn system_quads(
        &self,
        subject: Option<NamedNodeRef<'_>>,
        predicate: NamedNodeRef<'_>,
    ) -> Result<Vec<Quad>, StoreError> {
        self.quads_for_pattern(
            subject.map(Into::into),
            Some(predicate),
            None,
            Some(named_node(SYSTEM_GRAPH_IRI).into()),
        )
        .map(|quad| quad.map_err(|error| StoreError::Backend(error.to_string())))
        .collect()
    }
}

/// Every graph `source`'s registry knows about, ordered by IRI.
///
/// One implementation shared by [`Store::graphs`] and by the restore path, which has to read the
/// registry it has just written *inside the transaction that wrote it*. Two copies of this would
/// eventually disagree about what a valid registry is, and the restore path is precisely where
/// that matters: it is the one place a registry arrives from outside this process.
///
/// A registry entry that does not satisfy the [`GraphId`] invariants is a [`StoreError::Corrupt`],
/// never a silently skipped row. A graph we cannot describe is one we must not pretend is absent.
fn graphs_in(source: &impl RegistryReader, path: &Path) -> Result<Vec<GraphId>, StoreError> {
    let corrupt = |detail: String| StoreError::Corrupt {
        path: path.to_path_buf(),
        detail,
    };

    let mut graphs = Vec::new();

    for quad in source.system_quads(None, named_node(GRAPH_KIND_IRI))? {
        let iri = match quad.subject {
            oxigraph::model::NamedOrBlankNode::NamedNode(node) => node.into_string(),
            other => {
                return Err(corrupt(format!(
                    "a registered graph is identified by {other}, which is not an IRI"
                )))
            }
        };
        let Term::Literal(token) = quad.object else {
            return Err(corrupt(format!("the kind of graph {iri} is not a literal")));
        };
        let Some(kind) = GraphKind::parse(token.value()) else {
            return Err(corrupt(format!(
                "graph {iri} has kind {:?}, which this build does not recognise",
                token.value()
            )));
        };

        graphs.push(
            GraphId::from_registry(iri, kind)
                .map_err(|error| corrupt(format!("the registry is inconsistent: {error}")))?,
        );
    }

    // Sorted so the order is a property of the data rather than of the backend's iteration
    // order, which nothing upstream promises. A UI list that reshuffles between reloads reads
    // as a bug even when the contents are identical.
    graphs.sort();

    if let Some(duplicate) = first_duplicate(&graphs) {
        return Err(corrupt(format!(
            "graph {duplicate} is registered more than once, with different kinds"
        )));
    }

    Ok(graphs)
}

/// Whether `iri` has a registry entry in `source`.
///
/// An IRI the backend cannot even parse is reported absent rather than as an error: the question
/// is "is this taken", and something that can never have been written is not taken.
fn contains_graph_in(source: &impl RegistryReader, iri: &str) -> Result<bool, StoreError> {
    let Ok(subject) = NamedNode::new(iri) else {
        return Ok(false);
    };

    Ok(!source
        .system_quads(Some(subject.as_ref()), named_node(GRAPH_KIND_IRI))?
        .is_empty())
}

/// The first value that appears twice in a sorted slice, if any.
fn first_duplicate(sorted: &[GraphId]) -> Option<&GraphId> {
    sorted
        .windows(2)
        .find(|pair| pair[0].iri() == pair[1].iri())
        .map(|pair| &pair[0])
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
    use oxigraph::model::QuadRef;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("a temporary directory")
    }

    fn vocabulary(iri: impl Into<String>) -> GraphId {
        GraphId::vocabulary(iri).expect("a valid absolute IRI outside the reserved namespace")
    }

    #[test]
    fn opening_a_fresh_directory_creates_a_stamped_store() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh directory opens");

        assert_eq!(store.format_version(), FORMAT_VERSION);
        assert_eq!(store.path(), dir.path().join(STORE_SUBDIR));
        assert_eq!(
            store.quad_count().expect("countable"),
            3,
            "a new store holds its format stamp and the system graph's two registry quads, \
             and nothing else"
        );
    }

    /// The system graph is registered as a graph, in the graph registry, from the first open.
    /// Without this the registry is a special case with a hole in it, and "list every graph"
    /// silently means "list every graph except ours".
    #[test]
    fn a_fresh_store_registers_the_system_graph_and_nothing_else() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh directory opens");

        let graphs = store.graphs().expect("the registry is readable");

        assert_eq!(graphs, vec![GraphId::system()]);
        assert_eq!(graphs[0].kind(), GraphKind::System);
        assert!(store.contains_graph(SYSTEM_GRAPH_IRI).expect("readable"));
    }

    /// A store written before the registry existed must acquire one by being opened. If this
    /// needed a format bump then every additive piece of system metadata would need a migration,
    /// and the store would be far harder to evolve than it has to be.
    #[test]
    fn a_store_without_a_registry_gains_one_on_open_without_a_format_bump() {
        let dir = temp_dir();

        {
            // Strip the registry back out, leaving exactly what a pre-registry build wrote.
            let store = Store::open(dir.path()).expect("first open");
            store
                .backend
                .clear_graph(named_node(SYSTEM_GRAPH_IRI))
                .expect("the system graph is clearable");
            let version = Literal::new_typed_literal(FORMAT_VERSION.to_string(), xsd::INTEGER);
            store
                .backend
                .insert(QuadRef::new(
                    named_node(STORE_IRI),
                    named_node(FORMAT_VERSION_IRI),
                    &version,
                    named_node(SYSTEM_GRAPH_IRI),
                ))
                .expect("the stamp is rewritable");
            assert!(
                store.graphs().expect("readable").is_empty(),
                "the fixture must actually represent a store with no registry"
            );
            store.close().expect("a clean close");
        }

        let store = Store::open(dir.path()).expect("an older store still opens");

        assert_eq!(
            store.format_version(),
            FORMAT_VERSION,
            "acquiring a registry is additive and must not look like a format change"
        );
        assert_eq!(store.graphs().expect("readable"), vec![GraphId::system()]);
    }

    #[test]
    fn creating_a_vocabulary_graph_registers_it() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");
        let graph = vocabulary("http://example.org/v/animals");

        store.create_vocabulary_graph(&graph).expect("creatable");

        assert!(store.contains_graph(graph.iri()).expect("readable"));
        assert_eq!(
            store.graphs().expect("readable"),
            vec![
                vocabulary("http://example.org/v/animals"),
                GraphId::system()
            ],
            "the registry lists the new vocabulary alongside the system graph"
        );
    }

    /// The store-level face of `CLAUDE.md` §1.7. Quietly adopting an existing graph is how two
    /// vocabularies end up believing they own one, and it makes "create another one" the cheapest
    /// action available — the exact behaviour this product exists to attack.
    #[test]
    fn creating_a_vocabulary_graph_that_already_exists_is_refused() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");
        let graph = vocabulary("http://example.org/v/animals");

        store
            .create_vocabulary_graph(&graph)
            .expect("the first creation succeeds");
        let error = store
            .create_vocabulary_graph(&graph)
            .expect_err("the second must be refused");

        assert!(
            matches!(error, StoreError::GraphExists { ref iri } if iri == graph.iri()),
            "expected GraphExists, got: {error}"
        );
        assert!(
            error.to_string().contains("reuse or extend it"),
            "the message must point at the reuse ladder, not just refuse: {error}"
        );
        assert_eq!(
            store.graphs().expect("readable").len(),
            2,
            "a refused creation must leave the registry unchanged"
        );
    }

    /// The system graph is registered at open, so trying to create a vocabulary over it is caught
    /// by the same rule that catches any other collision — no special case to forget.
    #[test]
    fn the_system_graph_cannot_be_created_over() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");

        let error = store
            .create_vocabulary_graph(&GraphId::system())
            .expect_err("the system graph already exists");

        assert!(
            matches!(error, StoreError::GraphExists { .. }),
            "expected GraphExists, got: {error}"
        );
    }

    /// The writability rule, enforced rather than documented. An inferred graph is derived by a
    /// reasoner; a caller that could create one by hand could assert into it, and the
    /// asserted-versus-inferred distinction every "why?" explanation rests on would be gone.
    #[test]
    fn an_inferred_graph_cannot_be_created_by_a_caller() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");
        let inferred =
            GraphId::inferred_for(&vocabulary("http://example.org/v/animals")).expect("derivable");

        let error = store
            .create_vocabulary_graph(&inferred)
            .expect_err("inferred graphs are derived, not created");

        assert!(
            matches!(error, StoreError::NotWritable(ref iri) if iri == inferred.iri()),
            "expected NotWritable, got: {error}"
        );
        assert!(
            error.to_string().contains("only a reasoner may assert"),
            "the message must explain the rule, not just cite it: {error}"
        );
        assert!(!store.contains_graph(inferred.iri()).expect("readable"));
    }

    /// The choke point every write passes through, exercised directly against a graph the rule
    /// forbids. `create_vocabulary_graph` refuses earlier, so without this the refusal branch of
    /// `insert_into` itself would never be executed by any test.
    #[test]
    fn no_write_reaches_an_inferred_graph() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");
        let inferred =
            GraphId::inferred_for(&vocabulary("http://example.org/v/animals")).expect("derivable");

        let error = store
            .transaction(|txn| {
                txn.insert(
                    &inferred,
                    vec![(
                        NamedNode::new_unchecked("http://example.org/v/animals/cat"),
                        rdf::TYPE.into_owned(),
                        NamedNode::new_unchecked("http://www.w3.org/2004/02/skos/core#Concept")
                            .into(),
                    )],
                )
            })
            .expect_err("the choke point must refuse");

        assert!(
            matches!(error, StoreError::NotWritable(_)),
            "expected NotWritable, got: {error}"
        );
        assert_eq!(
            store.quad_count().expect("countable"),
            3,
            "a refused write must leave the store untouched"
        );
    }

    /// Backend iteration order is not a documented property, so the store imposes one. A list that
    /// reshuffles between reloads reads as a bug even when its contents are identical.
    #[test]
    fn graphs_are_listed_in_a_stable_order() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");

        for iri in [
            "http://example.org/v/zebra",
            "http://example.org/v/animals",
            "http://example.org/v/machines",
        ] {
            store
                .create_vocabulary_graph(&vocabulary(iri))
                .expect("creatable");
        }

        let graphs = store.graphs().expect("readable");
        let iris: Vec<&str> = graphs.iter().map(GraphId::iri).collect();

        assert_eq!(
            iris,
            vec![
                "http://example.org/v/animals",
                "http://example.org/v/machines",
                "http://example.org/v/zebra",
                SYSTEM_GRAPH_IRI,
            ]
        );
        assert_eq!(
            store.graphs().expect("readable"),
            store.graphs().expect("readable")
        );
    }

    #[test]
    fn the_registry_survives_close_and_reopen() {
        let dir = temp_dir();

        {
            let store = Store::open(dir.path()).expect("first open");
            store
                .create_vocabulary_graph(&vocabulary("http://example.org/v/animals"))
                .expect("creatable");
            store.close().expect("a clean close");
        }

        let store = Store::open(dir.path()).expect("second open");
        assert_eq!(
            store.graphs().expect("readable"),
            vec![
                vocabulary("http://example.org/v/animals"),
                GraphId::system()
            ]
        );
    }

    /// A store written by a build that knew a fourth kind must be refused, not downgraded to
    /// whatever this build's default happens to be. Same class of mistake as misreading a format
    /// version, and the same answer.
    #[test]
    fn a_graph_registered_with_an_unknown_kind_is_refused_rather_than_guessed() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");

        store
            .backend
            .insert(QuadRef::new(
                NamedNodeRef::new_unchecked("http://example.org/v/animals"),
                named_node(GRAPH_KIND_IRI),
                Literal::new_simple_literal("shapes").as_ref(),
                named_node(SYSTEM_GRAPH_IRI),
            ))
            .expect("a nonsense kind is writable");

        let error = store.graphs().expect_err("an unknown kind must be refused");

        assert!(
            matches!(error, StoreError::Corrupt { .. }),
            "expected Corrupt, got: {error}"
        );
        assert!(
            error.to_string().contains("shapes"),
            "the message must quote the value it could not read: {error}"
        );
    }

    /// The registry is data on disk. A doctored backup that registers a vocabulary at the system
    /// graph's own IRI would hand a user write access to our bookkeeping through the ordinary
    /// authoring path, so reading re-applies every invariant that writing did.
    #[test]
    fn a_registry_entry_that_breaks_the_namespace_rule_is_refused_on_read() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");

        store
            .backend
            .insert(QuadRef::new(
                named_node(SYSTEM_GRAPH_IRI),
                named_node(GRAPH_KIND_IRI),
                Literal::new_simple_literal(GraphKind::Vocabulary.as_str()).as_ref(),
                named_node(SYSTEM_GRAPH_IRI),
            ))
            .expect("a doctored entry is writable");

        let error = store
            .graphs()
            .expect_err("an impossible pairing must be refused");

        assert!(
            matches!(error, StoreError::Corrupt { .. }),
            "expected Corrupt, got: {error}"
        );
        assert!(
            error.to_string().contains("registry is inconsistent"),
            "the message must say the registry is the problem: {error}"
        );
    }

    #[test]
    fn contains_graph_does_not_treat_an_unparseable_iri_as_present() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");

        assert!(!store.contains_graph("not an iri").expect("readable"));
        assert!(!store
            .contains_graph("http://example.org/v/never-created")
            .expect("readable"));
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
            3,
            "reopening must read the existing stamp and registry, not write a second copy of \
             either"
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

    /// Two threads racing to create the *same* vocabulary IRI must not both succeed.
    ///
    /// This is the store-level face of `CLAUDE.md` §1.7. `create_vocabulary_graph` asks
    /// "is this IRI taken?" and then writes; without a transaction those are two separate
    /// operations against the backend, so both racers can read "no" before either writes "yes".
    /// The damage is not a duplicate row — it is that `graphs()` then refuses to read the
    /// registry at all, because a graph registered twice is a `Corrupt` store. One user's
    /// mis-timed second click takes the whole vocabulary list down.
    #[test]
    fn racing_creates_of_one_iri_leave_exactly_one_registration() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh directory opens");
        let iri = "https://example.org/contested";

        let racers = 8;
        let start = std::sync::Barrier::new(racers);
        let outcomes: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..racers)
                .map(|_| {
                    scope.spawn(|| {
                        start.wait();
                        store.create_vocabulary_graph(&vocabulary(iri))
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("no racer panics"))
                .collect()
        });

        let winners = outcomes.iter().filter(|outcome| outcome.is_ok()).count();
        assert_eq!(
            winners, 1,
            "exactly one racer may create the graph; the rest must be refused"
        );
        assert!(
            outcomes
                .iter()
                .filter_map(|outcome| outcome.as_ref().err())
                .all(|error| matches!(error, StoreError::GraphExists { .. })),
            "every loser must be told the IRI is taken, not given a backend error"
        );

        let graphs = store
            .graphs()
            .expect("the registry must still be readable after the race");
        assert_eq!(
            graphs.iter().filter(|graph| graph.iri() == iri).count(),
            1,
            "the contested IRI must appear exactly once in the registry"
        );
    }

    /// Atomicity, in the form a caller actually hits: a transaction that gives up partway must
    /// leave nothing behind. Without this, the first import or agent proposal that fails on its
    /// hundredth concept leaves ninety-nine in the store and no record that it was ever running.
    #[test]
    fn a_transaction_that_fails_partway_writes_nothing() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");
        let landed = "https://example.org/landed";
        let refused = "https://example.org/refused";

        let error = store
            .transaction(|txn| {
                txn.create_vocabulary_graph(&vocabulary(landed))?;
                // Registered a moment ago in this same transaction, so this is the transaction
                // reading its own uncommitted write and refusing on it.
                txn.create_vocabulary_graph(&vocabulary(landed))?;
                txn.create_vocabulary_graph(&vocabulary(refused))
            })
            .expect_err("the second create of one IRI must be refused");

        assert!(
            matches!(error, StoreError::GraphExists { .. }),
            "expected GraphExists, got: {error}"
        );
        assert!(
            !store.contains_graph(landed).expect("readable"),
            "the write that succeeded before the failure must be rolled back too"
        );
        assert!(!store.contains_graph(refused).expect("readable"));
        assert_eq!(
            store.quad_count().expect("countable"),
            3,
            "a rolled-back transaction must leave the store byte-identical"
        );
    }

    /// Everything in one transaction commits together, so a reader never sees half a change set.
    #[test]
    fn a_transaction_commits_every_write_together() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");
        let iris = ["https://example.org/a", "https://example.org/b"];

        store
            .transaction(|txn| {
                for iri in iris {
                    txn.create_vocabulary_graph(&vocabulary(iri))?;
                }
                Ok(())
            })
            .expect("the transaction commits");

        for iri in iris {
            assert!(
                store.contains_graph(iri).expect("readable"),
                "{iri} is absent"
            );
        }
    }

    /// A panic is not a tidy `Err`, and it is the case a caller cannot opt into handling. The
    /// backend rolls back on drop and unwinding drops the transaction, so the store is untouched —
    /// and, just as importantly, the store is still *writable* afterwards. The write lock is a
    /// `std::sync::Mutex`, which poisons when a holder panics; propagating that poison would turn
    /// one rolled-back edit into a store that had silently gone read-only for the process's life.
    #[test]
    fn a_panicking_transaction_rolls_back_and_leaves_the_store_writable() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");
        let abandoned = "https://example.org/abandoned";
        let afterwards = "https://example.org/afterwards";

        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            store.transaction(|txn| -> Result<(), StoreError> {
                txn.create_vocabulary_graph(&vocabulary(abandoned))?;
                panic!("a transaction that panics partway through");
            })
        }));

        assert!(panicked.is_err(), "the panic must reach the caller");
        assert!(
            !store.contains_graph(abandoned).expect("readable"),
            "unwinding out of a transaction must discard its writes"
        );

        store
            .create_vocabulary_graph(&vocabulary(afterwards))
            .expect("a panicked transaction must not leave the store read-only");
        assert!(store.contains_graph(afterwards).expect("readable"));
    }

    /// Repeatable read, from the outside. While a transaction holds an uncommitted create, every
    /// reader must still see the store as it was — and must see it *without waiting*, which is the
    /// second half of the claim and the one a lock-based fix would quietly break. The readers here
    /// run to completion while the writer is parked mid-transaction; if reads took the write lock,
    /// this test would deadlock rather than fail.
    #[test]
    fn readers_are_neither_blocked_by_an_open_transaction_nor_shown_its_writes() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");
        let pending = "https://example.org/pending";

        let readers = 4;
        let written = std::sync::Barrier::new(readers + 1);
        let inspected = std::sync::Barrier::new(readers + 1);

        std::thread::scope(|scope| {
            scope.spawn(|| {
                store
                    .transaction(|txn| {
                        txn.create_vocabulary_graph(&vocabulary(pending))?;
                        // Written, not committed. Hold here until every reader has looked.
                        written.wait();
                        inspected.wait();
                        Ok(())
                    })
                    .expect("the transaction commits");
            });

            let lookers: Vec<_> = (0..readers)
                .map(|_| {
                    scope.spawn(|| {
                        written.wait();
                        let seen = store.contains_graph(pending).expect("readable");
                        inspected.wait();
                        seen
                    })
                })
                .collect();

            for looker in lookers {
                assert!(
                    !looker.join().expect("no reader panics"),
                    "a reader must not see a write that has not committed"
                );
            }
        });

        assert!(
            store.contains_graph(pending).expect("readable"),
            "and must see it once the transaction commits"
        );
    }

    /// Distinct IRIs created concurrently must all land. Serialising writers makes them wait; it
    /// must not make them fail, and it must not lose any of them.
    #[test]
    fn concurrent_creates_of_distinct_iris_all_land() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");

        let writers = 8;
        let start = std::sync::Barrier::new(writers);
        std::thread::scope(|scope| {
            for index in 0..writers {
                let store = &store;
                let start = &start;
                scope.spawn(move || {
                    start.wait();
                    store
                        .create_vocabulary_graph(&vocabulary(format!(
                            "https://example.org/v{index}"
                        )))
                        .expect("a distinct IRI must not be refused");
                });
            }
        });

        let graphs = store.graphs().expect("readable");
        for index in 0..writers {
            let iri = format!("https://example.org/v{index}");
            assert!(
                graphs.iter().any(|graph| graph.iri() == iri),
                "{iri} was lost"
            );
        }
    }

    /// Writers are serialised by a non-reentrant lock, so a transaction opened inside another on
    /// the same store would block forever against itself. Detecting it turns a silent hang — no
    /// error, no log line, a request that never returns — into an error a caller can read.
    ///
    /// Note the failure mode of this test if the guard is removed: it hangs rather than failing.
    /// That is the honest shape, because the bug it guards against is itself a hang.
    #[test]
    fn a_nested_transaction_is_refused_rather_than_deadlocking() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open");
        let inner_iri = "https://example.org/inner";

        let error = store
            .transaction(|_outer| {
                store.transaction(|inner| inner.create_vocabulary_graph(&vocabulary(inner_iri)))
            })
            .expect_err("a nested transaction must be refused");

        assert!(
            matches!(error, StoreError::NestedTransaction),
            "expected NestedTransaction, got: {error}"
        );
        assert!(
            error.to_string().contains("cannot be nested"),
            "the message must say what the caller did wrong: {error}"
        );
        assert!(!store.contains_graph(inner_iri).expect("readable"));

        // The refusal must release the lock it never took, or the store is now unwritable.
        store
            .create_vocabulary_graph(&vocabulary("https://example.org/after-nesting"))
            .expect("a refused nesting must not leave the store locked");
    }

    /// Two stores over two data directories are independent, so a transaction on one must not be
    /// mistaken for a nesting attempt on the other. This is why the reentrancy mark is keyed by
    /// store rather than being a single per-thread flag.
    #[test]
    fn a_transaction_on_one_store_does_not_block_a_transaction_on_another() {
        let first_dir = temp_dir();
        let second_dir = temp_dir();
        let first = Store::open(first_dir.path()).expect("open");
        let second = Store::open(second_dir.path()).expect("open");

        first
            .transaction(|_| second.create_vocabulary_graph(&vocabulary("https://example.org/x")))
            .expect("a different store is not a nesting");

        assert!(second
            .contains_graph("https://example.org/x")
            .expect("readable"));
    }

    /// Serialisation, and the two questions an export has to answer honestly: *is this everything
    /// that was in the graph*, and *is this only what was in the graph*.
    mod export {
        use super::*;
        use oxigraph::io::RdfParser;
        use oxigraph::model::{BlankNode, GraphName, NamedOrBlankNode};
        use std::collections::BTreeSet;

        const VOCABULARY: &str = "http://acme.example/v/finance";
        const SKOS: &str = "http://www.w3.org/2004/02/skos/core#";

        fn skos(term: &str) -> NamedNode {
            NamedNode::new_unchecked(format!("{SKOS}{term}"))
        }

        /// Content chosen to break a serialiser rather than to flatter one: two language tags in
        /// non-Latin and accented scripts, a typed literal, a literal carrying the quote, newline
        /// and backslash every syntax escapes differently, an IRI with a percent-encoded space,
        /// and a blank node. A round trip over ASCII proves almost nothing.
        fn rich_statements() -> Vec<(NamedNode, NamedNode, Term)> {
            let subject = NamedNode::new_unchecked(format!("{VOCABULARY}#Derivative"));
            vec![
                (
                    subject.clone(),
                    rdf::TYPE.into_owned(),
                    skos("Concept").into(),
                ),
                (
                    subject.clone(),
                    skos("prefLabel"),
                    Literal::new_language_tagged_literal_unchecked("Dérivé financier", "fr").into(),
                ),
                (
                    subject.clone(),
                    skos("prefLabel"),
                    Literal::new_language_tagged_literal_unchecked("金融派生商品", "ja").into(),
                ),
                (
                    subject.clone(),
                    skos("notation"),
                    Literal::new_typed_literal("42", xsd::INTEGER).into(),
                ),
                (
                    subject.clone(),
                    skos("scopeNote"),
                    Literal::new_simple_literal(
                        "A \"quoted\" note,\nspanning lines, with a \\ backslash and an emoji 🧾",
                    )
                    .into(),
                ),
                (
                    subject.clone(),
                    skos("exactMatch"),
                    NamedNode::new_unchecked("http://other.example/scheme/a%20b").into(),
                ),
                (
                    subject,
                    skos("related"),
                    BlankNode::new_unchecked("unnamedConcept").into(),
                ),
            ]
        }

        fn populated_store(dir: &tempfile::TempDir) -> Store {
            let store = Store::open(dir.path()).expect("a fresh store opens");
            let graph = vocabulary(VOCABULARY);
            store
                .transaction(|txn| {
                    txn.create_vocabulary_graph(&graph)?;
                    txn.insert(&graph, rich_statements())
                })
                .expect("a fresh vocabulary takes its statements");
            store
        }

        fn export(store: &Store, iri: &str, syntax: RdfSyntax) -> Vec<u8> {
            let mut bytes = Vec::new();
            store
                .export_graph(iri, syntax, &mut bytes)
                .unwrap_or_else(|error| panic!("exporting {iri} as {syntax}: {error}"));
            bytes
        }

        /// Blank node labels are not preserved across a round trip and are not meant to be — the
        /// specifications say they are scoped to the document. Comparing them would test the
        /// serialiser's label generator rather than whether the statement survived, so every blank
        /// node collapses to one placeholder before comparison. There is exactly one in the
        /// fixture, so this cannot mask two being conflated.
        fn anonymise(quad: Quad) -> Quad {
            fn subject(node: NamedOrBlankNode) -> NamedOrBlankNode {
                match node {
                    NamedOrBlankNode::BlankNode(_) => BlankNode::new_unchecked("anonymous").into(),
                    named => named,
                }
            }
            Quad {
                subject: subject(quad.subject),
                predicate: quad.predicate,
                object: match quad.object {
                    Term::BlankNode(_) => BlankNode::new_unchecked("anonymous").into(),
                    other => other,
                },
                graph_name: quad.graph_name,
            }
        }

        fn reparse(bytes: &[u8], syntax: RdfSyntax) -> BTreeSet<String> {
            RdfParser::from_format(syntax.backend())
                .for_slice(bytes)
                .map(|quad| {
                    let quad =
                        quad.unwrap_or_else(|error| panic!("re-reading our own {syntax}: {error}"));
                    anonymise(quad).to_string()
                })
                .collect()
        }

        fn expected(syntax: RdfSyntax) -> BTreeSet<String> {
            let graph_name: GraphName = if syntax.records_graph_names() {
                NamedNode::new_unchecked(VOCABULARY).into()
            } else {
                GraphName::DefaultGraph
            };
            rich_statements()
                .into_iter()
                .map(|(subject, predicate, object)| {
                    anonymise(Quad::new(subject, predicate, object, graph_name.clone())).to_string()
                })
                .collect()
        }

        /// The item's central claim, tested the only way it can honestly be tested: write the
        /// graph out, read it back with a parser for that syntax, and require the statements to be
        /// the same set. A serialiser that drops a language tag, mangles an escape, or silently
        /// omits a statement fails here rather than in a customer's re-import.
        #[test]
        fn every_syntax_round_trips_the_statements_it_was_given() {
            let dir = temp_dir();
            let store = populated_store(&dir);

            for syntax in RdfSyntax::ALL {
                assert_eq!(
                    expected(syntax).len(),
                    rich_statements().len(),
                    "the comparison is worthless if the expected set collapsed"
                );

                let bytes = export(&store, VOCABULARY, syntax);
                assert!(
                    !bytes.is_empty(),
                    "{syntax} produced no bytes for seven statements"
                );
                assert_eq!(
                    reparse(&bytes, syntax),
                    expected(syntax),
                    "{syntax} did not survive a round trip"
                );
            }
        }

        /// Three of the six cannot carry a graph name, and this asserts what *actually* happens
        /// rather than what the flag says: the statements land in the default graph, the IRI is
        /// nowhere in the file, and nothing errors. The engine's `serialize_quad` would have
        /// errored instead, so this is the branch that makes the documented behaviour true.
        #[test]
        fn a_triple_syntax_drops_the_graph_name_and_says_so() {
            let dir = temp_dir();
            let store = populated_store(&dir);

            for syntax in RdfSyntax::ALL {
                let bytes = export(&store, VOCABULARY, syntax);
                let text = String::from_utf8(bytes.clone()).expect("UTF-8 output");
                let in_default_graph = RdfParser::from_format(syntax.backend())
                    .for_slice(&bytes)
                    .all(|quad| quad.expect("re-readable").graph_name == GraphName::DefaultGraph);

                if syntax.records_graph_names() {
                    assert!(
                        text.contains(VOCABULARY),
                        "{syntax} claims to record graph names and did not write one"
                    );
                    assert!(!in_default_graph, "{syntax} wrote to the default graph");
                } else {
                    assert!(
                        in_default_graph,
                        "{syntax} cannot name a graph, so everything must be in the default one"
                    );
                }
            }
        }

        /// The round-trip guarantee `CLAUDE.md` §1.3 asks for. The vocabulary *has* a registry
        /// entry — it is how the store knows it exists — and that entry must not be in the export.
        /// This is the failure PoolParty and TopBraid EDG ship: project bookkeeping in the same
        /// store as the content, so a consumer has to be told which parts to ignore.
        #[test]
        fn a_vocabulary_export_carries_none_of_openbizs_own_bookkeeping() {
            let dir = temp_dir();
            let store = populated_store(&dir);

            assert!(
                store.contains_graph(VOCABULARY).expect("readable"),
                "the test is worthless unless the registry entry it looks for exists"
            );

            for syntax in RdfSyntax::ALL {
                let text = String::from_utf8(export(&store, VOCABULARY, syntax)).expect("UTF-8");
                assert!(
                    !text.contains(OPENBIZ_NAMESPACE),
                    "{syntax} leaked the reserved namespace into a vocabulary export:\n{text}"
                );
            }
        }

        /// The other half of the same rule: an inferred graph is a separate graph, so it is not
        /// quietly folded into its vocabulary's export. A user who cannot tell an asserted
        /// statement from a derived one cannot defend the vocabulary to an auditor.
        #[test]
        fn materialised_inferences_are_not_folded_into_the_vocabulary() {
            let dir = temp_dir();
            let store = populated_store(&dir);
            let graph = vocabulary(VOCABULARY);
            let inferred = GraphId::inferred_for(&graph).expect("an inferred graph derives");

            // Registered through the same path a reasoner would use; the choke point refuses a
            // direct write to it, which is the point of `GraphKind::Inferred`.
            store
                .transaction(|txn| txn.ensure_registered(&inferred).map(|_| ()))
                .expect("an inferred graph registers");

            let text =
                String::from_utf8(export(&store, VOCABULARY, RdfSyntax::NQuads)).expect("UTF-8");
            assert!(!text.contains(inferred.iri()));
            assert_eq!(
                reparse(text.as_bytes(), RdfSyntax::NQuads),
                expected(RdfSyntax::NQuads)
            );
        }

        /// An operator's "what is actually in my store?" question. The system graph is exportable
        /// because hiding it would be the opacity `CLAUDE.md` §1 exists to attack — the rule is
        /// that our bookkeeping is never mixed into the user's work, not that it is unreachable.
        #[test]
        fn the_system_graph_can_be_exported_for_support() {
            let dir = temp_dir();
            let store = populated_store(&dir);

            let text = String::from_utf8(export(&store, SYSTEM_GRAPH_IRI, RdfSyntax::TriG))
                .expect("UTF-8");

            assert!(text.contains(SYSTEM_GRAPH_IRI), "its own registry entry");
            assert!(text.contains(VOCABULARY), "the vocabulary's registry entry");
            assert!(text.contains(FORMAT_VERSION_IRI), "the format stamp");
        }

        /// A created-but-empty vocabulary is a real state — it is what every vocabulary is for the
        /// moment between creation and its first concept. The export must be an *empty document*,
        /// not zero bytes: RDF/XML and JSON-LD both need a wrapper to parse at all, so a
        /// serialiser that skipped `finish()` on an empty graph would emit a file no consumer can
        /// read, and would look fine in a byte-length assertion.
        #[test]
        fn an_empty_vocabulary_exports_as_a_readable_empty_document() {
            let dir = temp_dir();
            let store = Store::open(dir.path()).expect("a fresh store opens");
            let graph = vocabulary("http://acme.example/v/brand-new");
            store
                .create_vocabulary_graph(&graph)
                .expect("a fresh IRI registers");

            for syntax in RdfSyntax::ALL {
                let bytes = export(&store, graph.iri(), syntax);
                assert!(
                    reparse(&bytes, syntax).is_empty(),
                    "{syntax} invented statements for an empty graph"
                );
            }
        }

        /// The difference between "empty" and "absent" is the whole reason this refusal exists.
        /// Returning an empty file for a vocabulary that does not exist hands the caller a valid,
        /// well-formed, entirely wrong document and no reason to doubt it.
        #[test]
        fn a_graph_that_is_not_registered_is_refused_rather_than_exported_empty() {
            let dir = temp_dir();
            let store = populated_store(&dir);

            for iri in [
                "http://acme.example/v/never-created",
                "not an iri at all",
                "",
                SYSTEM_GRAPH_IRI.strip_suffix("system").unwrap_or_default(),
            ] {
                let mut sink = Vec::new();
                let error = store
                    .export_graph(iri, RdfSyntax::Turtle, &mut sink)
                    .expect_err("an unregistered graph must be refused");
                assert!(
                    matches!(&error, StoreError::NoSuchGraph { iri: reported } if reported == iri),
                    "exporting {iri:?} gave {error}"
                );
                assert!(sink.is_empty(), "nothing may be written before the refusal");
            }
        }

        /// A graph that holds quads but has no registry entry is still absent, because the
        /// registry is the authority on what exists. The alternative — deciding from the data —
        /// would report a created-but-empty vocabulary as missing, which is the vocabulary a user
        /// is most likely to be looking for.
        #[test]
        fn the_registry_decides_what_exists_not_the_presence_of_quads() {
            let dir = temp_dir();
            let store = Store::open(dir.path()).expect("a fresh store opens");
            let orphan = "http://acme.example/v/unregistered";
            store
                .backend
                .insert(QuadRef::new(
                    NamedNodeRef::new_unchecked(orphan),
                    rdf::TYPE,
                    NamedNodeRef::new_unchecked(orphan),
                    NamedNodeRef::new_unchecked(orphan),
                ))
                .expect("the backend accepts a quad");

            assert!(matches!(
                store.export_graph(orphan, RdfSyntax::Turtle, Vec::new()),
                Err(StoreError::NoSuchGraph { .. })
            ));
        }

        /// The writer belongs to the caller, so it is the thing most likely to fail in production
        /// — a client that hangs up mid-download. It must surface as our error naming the graph
        /// and the syntax, not as a panic and not as a silently truncated file.
        #[test]
        fn a_failing_writer_is_reported_as_an_export_failure() {
            struct Broken;
            impl std::io::Write for Broken {
                fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "the client hung up",
                    ))
                }
                fn flush(&mut self) -> std::io::Result<()> {
                    Ok(())
                }
            }

            let dir = temp_dir();
            let store = populated_store(&dir);

            for syntax in RdfSyntax::ALL {
                let error = store
                    .export_graph(VOCABULARY, syntax, Broken)
                    .expect_err("a broken writer must fail the export");
                let StoreError::Export {
                    iri,
                    syntax: reported,
                    source,
                } = &error
                else {
                    panic!("{syntax} reported a writer failure as {error}");
                };
                assert_eq!(iri, VOCABULARY);
                assert_eq!(*reported, syntax);
                assert_eq!(source.kind(), std::io::ErrorKind::BrokenPipe);
            }
        }

        /// Exporting takes no write lock, so it cannot be the thing that blocks an author. Asserted
        /// by exporting from inside an open transaction: if `export_graph` took the write lock this
        /// would deadlock against the transaction holding it, and the test would hang rather than
        /// fail — noted in `docs/UNTESTED.md` as a bad failure shape, kept because the property is
        /// worth more than the shape costs.
        #[test]
        fn exporting_does_not_block_on_the_write_lock() {
            let dir = temp_dir();
            let store = populated_store(&dir);

            let bytes = store
                .transaction(|_| {
                    let mut bytes = Vec::new();
                    store.export_graph(VOCABULARY, RdfSyntax::NQuads, &mut bytes)?;
                    Ok(bytes)
                })
                .expect("an export inside a write transaction");

            assert_eq!(
                reparse(&bytes, RdfSyntax::NQuads),
                expected(RdfSyntax::NQuads)
            );
        }
    }
}
