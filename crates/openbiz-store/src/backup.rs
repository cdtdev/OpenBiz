//! The whole store, out to one file and back.
//!
//! # Why this is RDF and not the backend's own snapshot
//!
//! Every embedded store ships a checkpoint API, and using it would have been three lines. It was
//! refused, because the thing an operator needs from a backup is the thing a checkpoint cannot
//! give them: **independence from the software that wrote it.** A RocksDB checkpoint is a
//! directory of SST files readable by one version of one storage engine — so the backup of a
//! governance system is only as durable as our choice of embedded store, and `CLAUDE.md` §3 is
//! explicit that the engine is swappable. A backup that a backend swap invalidates is not a
//! backup; it is a copy.
//!
//! So a backup here is [N-Quads]: the statements themselves, in a W3C-Recommendation syntax, that
//! any conforming tool on any platform can read. It is line-based, so it streams, `grep`s, `diff`s
//! in review, and compresses well; and it carries the graph name on every line, which the three
//! triple syntaxes cannot (see [`RdfSyntax::records_graph_names`]) — a whole-store backup written
//! in Turtle would silently collapse every vocabulary into one.
//!
//! The price is honest and worth stating: it is larger than a checkpoint and slower to write,
//! because it is text and it is a full scan.
//!
//! # It is the *whole* store, including our own bookkeeping
//!
//! `GET /api/export` deliberately hands back one vocabulary and none of OpenBiz's metadata. A
//! backup is the opposite job and so it is the opposite rule: the system graph goes in the file,
//! because the registry — which graphs exist and what kind each is — *is* the thing that turns a
//! pile of statements back into a store. That is also what makes the file self-describing: the
//! store's format version is a statement in the system graph, so the backup carries its own
//! version stamp without our inventing a header, and [`Store::restore`] refuses a file whose
//! stamp it does not understand rather than reconstructing something it would then misread.
//!
//! # What restore refuses
//!
//! Restore is not an import and it is not a merge. It reconstructs a store into an empty one, and
//! it fails whole rather than in part: every refusal below is raised inside the transaction, so a
//! refused restore leaves the target store exactly as it was.
//!
//! - A store that already holds something ([`StoreError::RestoreNotEmpty`]) — merging two stores
//!   interleaves two histories with no way to separate them afterwards.
//! - A file with no store stamp ([`StoreError::NotABackup`]) — most likely an *export* of one
//!   vocabulary, which would restore as content with no registry.
//! - A stamp from a build newer than this one ([`StoreError::RestoreFormatTooNew`]), or an older
//!   one this build has no migration chain for ([`StoreError::RestoreNoMigrationPath`]). An older
//!   stamp that *is* reachable is migrated forward inside the restoring transaction instead.
//! - A statement in no graph, in a graph named by a blank node, or in a graph IRI that breaks the
//!   [`GraphId`] invariants ([`StoreError::RestoreRefused`]).
//! - Content in a graph the file's own registry does not list, or a registry this build could not
//!   read back ([`StoreError::RestoreRefused`]) — a restore that produced a store we would refuse
//!   to open is the one outcome that must not happen, because the operator restoring has already
//!   lost the original.
//!
//! [N-Quads]: https://www.w3.org/TR/n-quads/

use std::collections::BTreeSet;
use std::io::{Read, Write};

use oxigraph::io::{RdfParser, RdfSerializer};
use oxigraph::model::{GraphName, NamedOrBlankNode, Quad, Term};

use crate::{
    graphs_in, named_node, GraphId, GraphKind, MigrationReport, RdfSyntax, Store, StoreError,
    Transaction, FORMAT_VERSION, FORMAT_VERSION_IRI, STORE_IRI, SYSTEM_GRAPH_IRI,
};

/// The syntax a backup is written in, and the only one restore reads.
///
/// Public because it is a contract with the operator, not an implementation detail: it is what
/// tells them the file they are holding can be opened by something that is not us, and it is what
/// a `.nq` on the end of their filename should mean.
pub const BACKUP_SYNTAX: RdfSyntax = RdfSyntax::NQuads;

/// How many quads are handed to the backend at once during a restore.
///
/// Bounds *our* buffer, not the transaction's: the whole restore is one transaction, so the
/// backend's write batch grows for the entire file regardless (see [`Store::restore`]). This just
/// keeps the parser's output from being collected in full before any of it moves.
const RESTORE_BATCH: usize = 10_000;

/// What a backup contained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupReport {
    quads: u64,
    graphs: usize,
}

impl BackupReport {
    /// Statements written, across every graph.
    pub fn quads(&self) -> u64 {
        self.quads
    }

    /// Graphs the registry listed at the moment of the backup, including OpenBiz's own.
    pub fn graphs(&self) -> usize {
        self.graphs
    }
}

/// What a restore reconstructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreReport {
    quads: u64,
    graphs: usize,
    migrations: MigrationReport,
}

impl RestoreReport {
    /// Statements written into the store.
    ///
    /// One lower than the backup's count, and deliberately: the format stamp in the file is
    /// checked rather than restored, because the target store stamped itself when it was opened
    /// and two stamps are a store this build refuses as corrupt.
    pub fn quads(&self) -> u64 {
        self.quads
    }

    /// Graphs the restored registry lists, including OpenBiz's own.
    pub fn graphs(&self) -> usize {
        self.graphs
    }

    /// What restoring the file did to its store format.
    ///
    /// Empty when the backup was written by this build. When it was not, the file's contents were
    /// brought forward by the same migration chain [`Store::open`] runs, inside the same
    /// transaction that wrote them — so a backup that cannot be migrated restores nothing at all
    /// rather than restoring a store in a shape this build misreads.
    pub fn migrations(&self) -> &MigrationReport {
        &self.migrations
    }
}

impl Store {
    /// Write every statement in this store to `writer`, in [`BACKUP_SYNTAX`].
    ///
    /// # Cost
    ///
    /// A full scan, streamed: quads reach `writer` as they are read, so peak memory is one quad
    /// rather than the whole store. Takes no write lock, so a backup blocks no author.
    ///
    /// # What it does *not* do
    ///
    /// It does not take a consistent snapshot of the registry and the content together. The
    /// registry is read first, on its own snapshot, so the count in [`BackupReport`] can in
    /// principle disagree with a scan that began a moment later. That gap cannot matter today —
    /// the backend's exclusive lock means the only process that can be writing is this one, and
    /// the CLI's backup runs with the server stopped — and it is recorded in `docs/UNTESTED.md`
    /// against the day an online backup exists.
    pub fn backup(&self, writer: impl Write) -> Result<BackupReport, StoreError> {
        let graphs = self.graphs()?.len();

        let mut serializer = RdfSerializer::from_format(BACKUP_SYNTAX.backend()).for_writer(writer);
        let mut quads = 0;

        for quad in self.backend.quads_for_pattern(None, None, None, None) {
            let quad = quad.map_err(|error| StoreError::Backend(error.to_string()))?;
            serializer
                .serialize_quad(&quad)
                .map_err(|source| StoreError::Backup { source })?;
            quads += 1;
        }

        serializer
            .finish()
            .map_err(|source| StoreError::Backup { source })?;

        Ok(BackupReport { quads, graphs })
    }

    /// Reconstruct this store from a backup written by [`Store::backup`].
    ///
    /// The store must be empty — freshly opened on a new data directory is the intended case.
    /// See the module documentation for every refusal and why each one exists.
    ///
    /// # Atomicity
    ///
    /// One transaction for the whole file, so a restore that fails anywhere — a syntax error two
    /// thirds of the way through, a graph we will not accept, a registry that does not read back —
    /// leaves the target store exactly as it was rather than half-reconstructed. Half-restored is
    /// the state an operator cannot reason about: it looks like a store, opens like a store, and
    /// is missing an unknown subset of their vocabulary.
    ///
    /// # Cost
    ///
    /// Atomicity is paid for in memory. The backend holds a transaction's whole write batch in
    /// memory until it commits, so restoring a large store needs room for it — the alternative,
    /// committing in chunks, buys a bounded footprint with exactly the half-restored state above,
    /// and that is not a trade this operation may make. The ceiling is unmeasured and recorded in
    /// `docs/UNTESTED.md`.
    pub fn restore(&self, reader: impl Read) -> Result<RestoreReport, StoreError> {
        self.refuse_unless_empty()?;

        let parser = RdfParser::from_format(BACKUP_SYNTAX.backend()).for_reader(reader);

        self.transaction(|txn| {
            let mut batch: Vec<Quad> = Vec::with_capacity(RESTORE_BATCH);
            let mut named_graphs: BTreeSet<String> = BTreeSet::new();
            let mut stamps: Vec<Term> = Vec::new();
            let mut quads = 0;

            for quad in parser {
                let quad = quad.map_err(parse_failure)?;

                // The stamp is checked, never written: the target store stamped itself when it
                // was opened, and a second stamp — even an identical one at a different lexical
                // form — is a store `Store::open` reports as corrupt from then on.
                if is_format_stamp(&quad) {
                    stamps.push(quad.object);
                    continue;
                }

                let iri = graph_iri(&quad)?;

                // Classify each graph once. A million-quad vocabulary is one entry here, so the
                // set is the size of the registry rather than of the file.
                if !named_graphs.contains(iri) {
                    GraphId::classify(iri).map_err(|error| StoreError::RestoreRefused {
                        detail: error.to_string(),
                    })?;
                    named_graphs.insert(iri.to_owned());
                }

                batch.push(quad);
                quads += 1;

                if batch.len() == RESTORE_BATCH {
                    txn.restore(&batch)?;
                    batch.clear();
                }
            }

            txn.restore(&batch)?;

            // Migrate the *file's* contents, not the target store's stamp: the target stamped
            // itself at the current version when it was opened, and the shape that needs bringing
            // forward is the one that just arrived from disk. Running inside this transaction is
            // what makes an unmigratable backup restore nothing rather than something.
            let file_version = check_stamp(&stamps)?;
            let migrations = crate::migrate::migrate(txn, file_version, self.path()).map_err(
                |error| match error {
                    StoreError::NoMigrationPath {
                        found,
                        supported,
                        missing,
                        ..
                    } => StoreError::RestoreNoMigrationPath {
                        found,
                        supported,
                        missing,
                    },
                    other => other,
                },
            )?;

            // Read the registry back through the *same* code `Store::open` and `Store::graphs`
            // use, inside the transaction that just wrote it. This is the check that makes the
            // refusals worth having: it asks "would this build open the store I am about to
            // commit?" while there is still something to roll back to.
            let registry = graphs_in(&txn.inner, self.path())?;

            for iri in &named_graphs {
                if !registry.iter().any(|graph| graph.iri() == iri) {
                    return Err(StoreError::RestoreRefused {
                        detail: format!(
                            "the backup has statements in graph {iri}, which its own registry does \
                             not list; a graph the store cannot describe is one it cannot show, \
                             export, or govern"
                        ),
                    });
                }
            }

            Ok(RestoreReport {
                quads,
                graphs: registry.len(),
                migrations,
            })
        })
    }

    /// Refuse a restore into a store that already holds anything of its own.
    ///
    /// Two checks, cheapest first. The registry answers for every store this build has written,
    /// and costs one lookup. The scan behind it catches the case the registry cannot see —
    /// statements in a graph nothing registered — and only runs when the registry came back
    /// clean, so it walks a near-empty store rather than a populated one.
    fn refuse_unless_empty(&self) -> Result<(), StoreError> {
        let not_empty = || StoreError::RestoreNotEmpty {
            path: self.path().to_path_buf(),
        };

        if self
            .graphs()?
            .iter()
            .any(|graph| graph.kind() != GraphKind::System)
        {
            return Err(not_empty());
        }

        let system: GraphName = named_node(SYSTEM_GRAPH_IRI).into_owned().into();
        for quad in self.backend.quads_for_pattern(None, None, None, None) {
            let quad = quad.map_err(|error| StoreError::Backend(error.to_string()))?;
            if quad.graph_name != system {
                return Err(not_empty());
            }
        }

        Ok(())
    }
}

impl Transaction<'_> {
    /// Write quads that are being **restored** rather than authored.
    ///
    /// This is the one write path that may target a graph
    /// [`GraphId::is_directly_writable`] says no to, and the exception is narrow enough to state
    /// exactly: a restore is not a caller asserting something, it is a store that already existed
    /// being put back. Refusing an inferred graph here would silently drop a materialised
    /// entailment from every backup that had one, which is a data loss dressed as a safety rule.
    ///
    /// What it does *not* relax is that every quad names a graph OpenBiz can describe — the caller
    /// has already put each graph name through [`GraphId::classify`], and the registry is read
    /// back before the transaction commits.
    pub(crate) fn restore(&mut self, quads: &[Quad]) -> Result<(), StoreError> {
        if quads.is_empty() {
            return Ok(());
        }
        self.inner.extend(quads);
        Ok(())
    }
}

/// Whether this quad is the store's own format stamp.
fn is_format_stamp(quad: &Quad) -> bool {
    let GraphName::NamedNode(graph) = &quad.graph_name else {
        return false;
    };
    let NamedOrBlankNode::NamedNode(subject) = &quad.subject else {
        return false;
    };
    graph.as_str() == SYSTEM_GRAPH_IRI
        && subject.as_str() == STORE_IRI
        && quad.predicate.as_str() == FORMAT_VERSION_IRI
}

/// The IRI of the graph a restored quad belongs in, or a refusal naming why it has none.
fn graph_iri(quad: &Quad) -> Result<&str, StoreError> {
    match &quad.graph_name {
        GraphName::NamedNode(node) => Ok(node.as_str()),
        GraphName::BlankNode(node) => Err(StoreError::RestoreRefused {
            detail: format!(
                "a statement is in the graph named by the blank node _:{}, and every graph \
                 OpenBiz can describe is named by an IRI",
                node.as_str()
            ),
        }),
        GraphName::DefaultGraph => Err(StoreError::RestoreRefused {
            detail: "a statement is in the default graph, and every statement in an OpenBiz store \
                     belongs to a named vocabulary — this is most likely an export of one \
                     vocabulary rather than a backup of a store"
                .to_owned(),
        }),
    }
}

/// Read the file's format stamp, refusing anything that is not a single version this build could
/// act on.
///
/// A version *older* than this build's is returned rather than refused: the caller migrates it.
/// A version newer than this build's is refused here, because there is nothing to be done with a
/// shape we have never seen.
fn check_stamp(stamps: &[Term]) -> Result<u32, StoreError> {
    let found = match stamps {
        [] => {
            return Err(StoreError::NotABackup {
                detail: format!(
                    "it carries no <{FORMAT_VERSION_IRI}> statement, so it does not say which \
                     store format it is in. An OpenBiz backup always does"
                ),
            })
        }
        [Term::Literal(literal)] => {
            literal
                .value()
                .parse::<u32>()
                .map_err(|_| StoreError::NotABackup {
                    detail: format!(
                        "its store format version is {:?}, which is not a version number",
                        literal.value()
                    ),
                })?
        }
        [other] => {
            return Err(StoreError::NotABackup {
                detail: format!("its store format version is {other}, which is not a literal"),
            })
        }
        many => {
            return Err(StoreError::NotABackup {
                detail: format!(
                    "it carries {} store format versions, and there is no way to tell which one \
                     describes the file",
                    many.len()
                ),
            })
        }
    };

    if found > FORMAT_VERSION {
        return Err(StoreError::RestoreFormatTooNew {
            found,
            supported: FORMAT_VERSION,
        });
    }

    Ok(found)
}

/// Turn the parser's failure into ours, keeping the position when it gave one.
fn parse_failure(error: oxigraph::io::RdfParseError) -> StoreError {
    match error {
        oxigraph::io::RdfParseError::Io(source) => StoreError::RestoreRead { source },
        oxigraph::io::RdfParseError::Syntax(error) => StoreError::RestoreSyntax {
            // The parser counts lines from zero and every editor counts from one. Reporting its
            // number verbatim sends an operator to the line above the broken one.
            line: error.location().map(|at| at.start.line + 1),
            detail: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxigraph::model::{Literal, NamedNode};

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("a temporary directory")
    }

    /// A store holding two vocabularies with real statements in them.
    ///
    /// Writes through the crate's own transaction, which is the only write path there is — there
    /// is no authoring API yet (Phase 2), so this is what "a live store with content" means today
    /// and the limit is recorded in `docs/UNTESTED.md`.
    fn populated(dir: &std::path::Path) -> Store {
        let store = Store::open(dir).expect("open the store");

        for (iri, concepts) in [
            ("https://example.org/animals", 3),
            ("https://example.org/plants", 2),
        ] {
            let graph = GraphId::vocabulary(iri).expect("a vocabulary IRI");
            store
                .create_vocabulary_graph(&graph)
                .expect("create the vocabulary");

            store
                .transaction(|txn| {
                    for n in 0..concepts {
                        let subject =
                            NamedNode::new(format!("{iri}/concept/{n}")).expect("a concept IRI");
                        txn.insert(
                            &graph,
                            vec![
                                (
                                    subject.clone(),
                                    NamedNode::new_unchecked(
                                        "http://www.w3.org/2004/02/skos/core#prefLabel",
                                    ),
                                    Literal::new_language_tagged_literal_unchecked(
                                        format!("concept {n}"),
                                        "en",
                                    )
                                    .into(),
                                ),
                                (
                                    subject,
                                    NamedNode::new_unchecked(
                                        "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
                                    ),
                                    NamedNode::new_unchecked(
                                        "http://www.w3.org/2004/02/skos/core#Concept",
                                    )
                                    .into(),
                                ),
                            ],
                        )?;
                    }
                    Ok(())
                })
                .expect("write the concepts");
        }

        store
    }

    /// Every quad in a store, as sorted N-Quads lines — the comparison a round trip has to pass.
    fn contents(store: &Store) -> Vec<String> {
        let mut bytes = Vec::new();
        store.backup(&mut bytes).expect("back the store up");
        let mut lines: Vec<String> = String::from_utf8(bytes)
            .expect("N-Quads is UTF-8")
            .lines()
            .map(str::to_owned)
            .collect();
        lines.sort();
        lines
    }

    #[test]
    fn a_backup_restores_into_a_store_that_holds_exactly_what_the_first_one_did() {
        let source_dir = temp_dir();
        let source = populated(source_dir.path());
        let before = contents(&source);
        let registry: Vec<String> = source
            .graphs()
            .expect("read the registry")
            .iter()
            .map(|graph| graph.to_string())
            .collect();

        let mut backup = Vec::new();
        let report = source.backup(&mut backup).expect("back the store up");
        assert_eq!(report.graphs(), 3, "two vocabularies and the system graph");
        assert!(report.quads() >= 10, "10 concept statements plus metadata");
        let source_quads = source.quad_count().expect("count the source store");
        source.close().expect("close the source store");

        let target_dir = temp_dir();
        let target = Store::open(target_dir.path()).expect("open the target store");
        let restored = target.restore(backup.as_slice()).expect("restore");

        assert_eq!(
            restored.quads(),
            report.quads() - 1,
            "everything but the format stamp, which is checked rather than rewritten"
        );
        assert_eq!(restored.graphs(), report.graphs());
        assert_eq!(
            contents(&target),
            before,
            "the restored store holds different statements from the one that was backed up"
        );
        // Counted as well as compared: the comparison above goes through the serialiser, so it
        // proves the two stores *write* the same file, and this proves they hold the same number
        // of statements without that round trip in the way.
        //
        // Neither can see one thing, and it is worth writing down because a deliberate break was
        // run against these tests to find out. Dropping a statement the target store writes for
        // *itself* when it opens — its own registry entry, its own stamp — is invisible to both,
        // because restoring a statement the store already holds is idempotent and the two stores
        // agree afterwards either way. That mutation is genuinely equivalent rather than
        // undetected; dropping any other statement fails both assertions, which was checked.
        assert_eq!(
            target.quad_count().expect("count the restored store"),
            source_quads,
            "the restored store holds a different number of statements from the source"
        );

        let restored_registry: Vec<String> = target
            .graphs()
            .expect("read the restored registry")
            .iter()
            .map(|graph| graph.to_string())
            .collect();
        assert_eq!(restored_registry, registry);
    }

    /// The point of the round trip is that the *store* comes back, not that the bytes do — so it
    /// is checked through the reading API a user would actually use.
    #[test]
    fn a_restored_vocabulary_exports_and_answers_queries_like_any_other() {
        let source_dir = temp_dir();
        let source = populated(source_dir.path());
        let mut backup = Vec::new();
        source.backup(&mut backup).expect("back the store up");
        source.close().expect("close the source store");

        let target_dir = temp_dir();
        let target = Store::open(target_dir.path()).expect("open the target store");
        target.restore(backup.as_slice()).expect("restore");

        let mut exported = Vec::new();
        target
            .export_graph(
                "https://example.org/animals",
                RdfSyntax::NQuads,
                &mut exported,
            )
            .expect("export the restored vocabulary");
        let exported = String::from_utf8(exported).expect("N-Quads is UTF-8");
        assert_eq!(
            exported.lines().count(),
            6,
            "three concepts, two statements each: {exported}"
        );
        assert!(
            !exported.contains("urn:openbiz:"),
            "the restored export leaked OpenBiz's own bookkeeping: {exported}"
        );
    }

    /// A store that reopens is a store that committed. Without this the round trip could be
    /// passing on a transaction that never reached disk.
    #[test]
    fn a_restore_survives_closing_and_reopening_the_store() {
        let source_dir = temp_dir();
        let source = populated(source_dir.path());
        let before = contents(&source);
        let mut backup = Vec::new();
        source.backup(&mut backup).expect("back the store up");
        source.close().expect("close the source store");

        let target_dir = temp_dir();
        let target = Store::open(target_dir.path()).expect("open the target store");
        target.restore(backup.as_slice()).expect("restore");
        target.close().expect("close the target store");

        let reopened = Store::open(target_dir.path()).expect("reopen the target store");
        assert_eq!(contents(&reopened), before);
    }

    #[test]
    fn a_restore_into_a_populated_store_is_refused() {
        let source_dir = temp_dir();
        let source = populated(source_dir.path());
        let mut backup = Vec::new();
        source.backup(&mut backup).expect("back the store up");

        let before = contents(&source);
        let error = source
            .restore(backup.as_slice())
            .expect_err("restoring over a populated store must be refused");
        assert!(
            matches!(error, StoreError::RestoreNotEmpty { .. }),
            "unexpected error: {error}"
        );
        assert_eq!(
            contents(&source),
            before,
            "the refused restore changed the store"
        );
    }

    /// The registry can say "empty" while quads exist, if something wrote a graph nobody
    /// registered. The second check exists for exactly that, so it is tested on its own.
    #[test]
    fn a_restore_is_refused_when_content_exists_that_the_registry_does_not_know_about() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open the store");

        // Straight past the registry, on purpose: this is the state the scan is for.
        store
            .transaction(|txn| {
                txn.restore(&[Quad::new(
                    NamedNode::new_unchecked("https://example.org/orphan"),
                    NamedNode::new_unchecked("https://example.org/p"),
                    NamedNode::new_unchecked("https://example.org/o"),
                    NamedNode::new_unchecked("https://example.org/unregistered"),
                )])
            })
            .expect("write the unregistered quad");

        let error = store
            .restore(b"".as_slice())
            .expect_err("a store with orphaned content is not empty");
        assert!(
            matches!(error, StoreError::RestoreNotEmpty { .. }),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn an_export_of_one_vocabulary_is_refused_as_a_backup() {
        let source_dir = temp_dir();
        let source = populated(source_dir.path());
        let mut exported = Vec::new();
        source
            .export_graph(
                "https://example.org/animals",
                RdfSyntax::NQuads,
                &mut exported,
            )
            .expect("export one vocabulary");
        source.close().expect("close the source store");

        let target_dir = temp_dir();
        let target = Store::open(target_dir.path()).expect("open the target store");
        let error = target
            .restore(exported.as_slice())
            .expect_err("an export is not a backup");

        // It is refused for having no stamp, not for the statements it carries — the message has
        // to send the operator to "you exported one vocabulary", not to a syntax hunt.
        assert!(
            matches!(error, StoreError::NotABackup { .. }),
            "unexpected error: {error}"
        );
        assert!(
            target.graphs().expect("read the registry").len() == 1,
            "the refused restore registered something"
        );
    }

    #[test]
    fn a_backup_from_a_newer_build_is_refused_rather_than_misread() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open the store");

        let backup = format!(
            "<{STORE_IRI}> <{FORMAT_VERSION_IRI}> \
             \"{}\"^^<http://www.w3.org/2001/XMLSchema#integer> <{SYSTEM_GRAPH_IRI}> .\n",
            FORMAT_VERSION + 1
        );
        let error = store
            .restore(backup.as_bytes())
            .expect_err("a newer format must be refused");
        assert!(
            matches!(
                error,
                StoreError::RestoreFormatTooNew {
                    found,
                    supported
                } if found == FORMAT_VERSION + 1 && supported == FORMAT_VERSION
            ),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn a_backup_from_a_version_with_no_migration_out_of_it_is_refused_whole() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open the store");

        // Version 0 never existed and never will have a migration, so it stands in for the real
        // case: a build that reads a version it has lost the step for. The refusal must name the
        // *missing* version, because that is what identifies the release an operator needs.
        let backup = format!(
            "<{STORE_IRI}> <{FORMAT_VERSION_IRI}> \
             \"0\"^^<http://www.w3.org/2001/XMLSchema#integer> <{SYSTEM_GRAPH_IRI}> .\n\
             <https://example.org/vocab> \
             <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <urn:openbiz:Graph> \
             <{SYSTEM_GRAPH_IRI}> .\n"
        );
        let error = store
            .restore(backup.as_bytes())
            .expect_err("a version with no migration out of it must be refused");
        assert!(
            matches!(
                error,
                StoreError::RestoreNoMigrationPath {
                    found: 0,
                    missing: 0,
                    ..
                }
            ),
            "unexpected error: {error}"
        );
        assert!(
            error.to_string().contains("upgrade one release at a time"),
            "refusing is not enough; the message must say what to do: {error}"
        );

        // Refused *whole*: the vocabulary registration in the file must not have landed.
        assert_eq!(
            store.graphs().expect("read the registry"),
            vec![GraphId::system()],
            "a refused restore left something behind"
        );
    }

    #[test]
    fn a_backup_from_an_older_format_is_migrated_as_it_is_restored() {
        let source = temp_dir();
        let backup = {
            // A version-1 store, written as a version-1 build would have: content, a registry for
            // the vocabulary, a stamp of 1, and *no* registry entry for the system graph — which
            // is precisely the invariant version 2 exists to guarantee.
            let store = populated(source.path());
            let mut backup = Vec::new();
            store.backup(&mut backup).expect("back the store up");
            store.close().expect("a clean close");

            let text = String::from_utf8(backup).expect("N-Quads is UTF-8");
            let downgraded: String = text
                .lines()
                .filter(|line| {
                    !line.contains(FORMAT_VERSION_IRI)
                        && !line.starts_with(&format!("<{SYSTEM_GRAPH_IRI}>"))
                })
                .map(|line| format!("{line}\n"))
                .collect();
            format!(
                "{downgraded}<{STORE_IRI}> <{FORMAT_VERSION_IRI}> \
                 \"1\"^^<http://www.w3.org/2001/XMLSchema#integer> <{SYSTEM_GRAPH_IRI}> .\n"
            )
        };

        let target_dir = temp_dir();
        let target = Store::open(target_dir.path()).expect("open the target store");
        let report = target
            .restore(backup.as_bytes())
            .expect("an older backup restores");

        assert!(
            report.migrations().migrated(),
            "a version-1 backup must be reported as migrated, not silently accepted"
        );
        assert_eq!(report.migrations().previous_version(), 1);
        assert_eq!(report.migrations().current_version(), FORMAT_VERSION);
        assert_eq!(
            report
                .migrations()
                .steps()
                .iter()
                .map(|step| step.id)
                .collect::<Vec<_>>(),
            vec![
                "0002-register-system-graph",
                "0003-allow-candidate-graphs",
                "0004-allow-candidate-removals",
                "0005-retype-iri-policy-stamps"
            ],
            "a version-1 backup runs the whole chain up to this build's format version"
        );

        // The content survived, and the store the migration produced is one we open again
        // without further work.
        assert!(target
            .graphs()
            .expect("read the registry")
            .iter()
            .any(|graph| graph.iri() == "https://example.org/animals"));
        target.close().expect("a clean close");

        let reopened = Store::open(target_dir.path()).expect("reopen the restored store");
        assert_eq!(reopened.format_version(), FORMAT_VERSION);
        assert!(
            !reopened.migrations().migrated(),
            "the migration must be a one-off, not something every open repeats"
        );
    }

    #[test]
    fn a_statement_in_no_graph_is_refused() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open the store");

        let backup = format!(
            "<{STORE_IRI}> <{FORMAT_VERSION_IRI}> \
             \"{FORMAT_VERSION}\"^^<http://www.w3.org/2001/XMLSchema#integer> \
             <{SYSTEM_GRAPH_IRI}> .\n\
             <https://example.org/s> <https://example.org/p> <https://example.org/o> .\n"
        );
        let error = store
            .restore(backup.as_bytes())
            .expect_err("a statement outside every graph must be refused");
        assert!(
            matches!(&error, StoreError::RestoreRefused { detail } if detail.contains("default graph")),
            "unexpected error: {error}"
        );
    }

    /// A graph IRI inside our reserved namespace that is neither the system graph nor an inferred
    /// one is a file written against a build that is not this one.
    #[test]
    fn a_graph_in_the_reserved_namespace_that_we_do_not_define_is_refused() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open the store");

        let backup = format!(
            "<{STORE_IRI}> <{FORMAT_VERSION_IRI}> \
             \"{FORMAT_VERSION}\"^^<http://www.w3.org/2001/XMLSchema#integer> \
             <{SYSTEM_GRAPH_IRI}> .\n\
             <https://example.org/s> <https://example.org/p> <https://example.org/o> \
             <urn:openbiz:graph:invented> .\n"
        );
        let error = store
            .restore(backup.as_bytes())
            .expect_err("an undefined reserved graph must be refused");
        assert!(
            matches!(&error, StoreError::RestoreRefused { detail } if detail.contains("urn:openbiz:")),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn content_in_a_graph_the_backups_own_registry_omits_is_refused() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open the store");

        let backup = format!(
            "<{STORE_IRI}> <{FORMAT_VERSION_IRI}> \
             \"{FORMAT_VERSION}\"^^<http://www.w3.org/2001/XMLSchema#integer> \
             <{SYSTEM_GRAPH_IRI}> .\n\
             <https://example.org/s> <https://example.org/p> <https://example.org/o> \
             <https://example.org/unregistered> .\n"
        );
        let error = store
            .restore(backup.as_bytes())
            .expect_err("unregistered content must be refused");
        assert!(
            matches!(&error, StoreError::RestoreRefused { detail }
                if detail.contains("https://example.org/unregistered")),
            "unexpected error: {error}"
        );
        assert_eq!(
            store.graphs().expect("read the registry").len(),
            1,
            "the refused restore left something behind"
        );
    }

    /// The registry is data in the file, so a file can carry one this build would refuse to read.
    /// It must be refused *before* the commit, not on the next open.
    #[test]
    fn a_registry_this_build_cannot_read_is_refused_before_it_commits() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open the store");

        let backup = format!(
            "<{STORE_IRI}> <{FORMAT_VERSION_IRI}> \
             \"{FORMAT_VERSION}\"^^<http://www.w3.org/2001/XMLSchema#integer> \
             <{SYSTEM_GRAPH_IRI}> .\n\
             <https://example.org/v> <urn:openbiz:graphKind> \"encyclopaedia\" \
             <{SYSTEM_GRAPH_IRI}> .\n"
        );
        let error = store
            .restore(backup.as_bytes())
            .expect_err("an unreadable registry must be refused");
        assert!(
            matches!(&error, StoreError::Corrupt { detail, .. } if detail.contains("encyclopaedia")),
            "unexpected error: {error}"
        );
        assert!(
            store.graphs().is_ok(),
            "the refused restore left the store's own registry unreadable"
        );
    }

    #[test]
    fn a_malformed_backup_is_refused_with_the_line_it_broke_on() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open the store");

        let backup = format!(
            "<{STORE_IRI}> <{FORMAT_VERSION_IRI}> \
             \"{FORMAT_VERSION}\"^^<http://www.w3.org/2001/XMLSchema#integer> \
             <{SYSTEM_GRAPH_IRI}> .\n\
             this is not a statement\n"
        );
        let error = store
            .restore(backup.as_bytes())
            .expect_err("a malformed backup must be refused");
        let StoreError::RestoreSyntax { line, .. } = &error else {
            panic!("unexpected error: {error}");
        };
        assert_eq!(*line, Some(2), "the position must be one-based: {error}");
    }

    /// A restore that fails part way through must leave nothing behind, including when the
    /// failure is at the very end of a file that parsed cleanly until then.
    #[test]
    fn a_restore_that_fails_late_rolls_back_everything_before_it() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("open the store");

        let mut backup = format!(
            "<{STORE_IRI}> <{FORMAT_VERSION_IRI}> \
             \"{FORMAT_VERSION}\"^^<http://www.w3.org/2001/XMLSchema#integer> \
             <{SYSTEM_GRAPH_IRI}> .\n\
             <https://example.org/v> <urn:openbiz:graphKind> \"vocabulary\" \
             <{SYSTEM_GRAPH_IRI}> .\n"
        );
        for n in 0..(RESTORE_BATCH + 1) {
            backup.push_str(&format!(
                "<https://example.org/c/{n}> <https://example.org/p> \"{n}\" \
                 <https://example.org/v> .\n"
            ));
        }
        backup.push_str("<https://example.org/s> <https://example.org/p> broken .\n");

        let error = store
            .restore(backup.as_bytes())
            .expect_err("a late failure must still be a failure");
        assert!(
            matches!(error, StoreError::RestoreSyntax { .. }),
            "unexpected error: {error}"
        );
        assert_eq!(
            store.graphs().expect("read the registry").len(),
            1,
            "more than one batch had already been written when it failed, and it was not rolled \
             back"
        );
    }

    /// An empty vocabulary is a real state — created and not yet populated — and it is the one a
    /// content-based backup could most easily lose, because it has no content.
    #[test]
    fn a_created_but_empty_vocabulary_survives_the_round_trip() {
        let source_dir = temp_dir();
        let source = Store::open(source_dir.path()).expect("open the store");
        let graph = GraphId::vocabulary("https://example.org/empty").expect("a vocabulary IRI");
        source
            .create_vocabulary_graph(&graph)
            .expect("create the vocabulary");

        let mut backup = Vec::new();
        source.backup(&mut backup).expect("back the store up");
        source.close().expect("close the source store");

        let target_dir = temp_dir();
        let target = Store::open(target_dir.path()).expect("open the target store");
        target.restore(backup.as_slice()).expect("restore");

        let graphs = target.graphs().expect("read the restored registry");
        assert_eq!(graphs.len(), 2, "the system graph and the empty vocabulary");
        assert!(target
            .contains_graph("https://example.org/empty")
            .expect("look the vocabulary up"));
    }

    #[test]
    fn the_backup_syntax_is_one_that_records_which_graph_a_statement_is_in() {
        // The whole file rests on this: a backup in a triple syntax would collapse every
        // vocabulary in the store into one indistinguishable pile.
        assert!(BACKUP_SYNTAX.records_graph_names());
    }
}
