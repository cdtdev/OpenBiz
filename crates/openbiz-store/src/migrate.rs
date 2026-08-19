//! Store-format migrations: how a store written by an older OpenBiz becomes one this build reads.
//!
//! A store carries the format version that wrote it ([`crate::FORMAT_VERSION`], stamped in the
//! system graph). Three cases, and each has exactly one answer:
//!
//! - **Newer than this build** — refused. `StoreError::FormatTooNew`, because an older build
//!   silently misreading a newer layout is the failure that loses data.
//! - **Equal** — nothing happens. No writes, no records, no log line worth reading.
//! - **Older** — brought forward, one version at a time, by the chain below.
//!
//! # Why the shape is what it is
//!
//! **Forward-only.** A [`Migration`] declares the version it starts from and always ends one
//! higher; there is no `revert`. Downgrading a store is not a thing this product will offer,
//! because the only honest way to go back is to restore the backup you took before upgrading —
//! and a half-working downgrade invites an operator to skip taking one. `openbiz backup` is the
//! supported answer and it is a command, not a promise.
//!
//! **One transaction for the whole chain.** Every step, the migration records, and the new stamp
//! commit together or not at all. A store stamped 3 that only got as far as migration 2 is the
//! state nobody can reason about — it opens, it looks fine, and some unknown subset of it is in
//! the old shape. The same argument the restore path makes about half-restored stores
//! (`docs/adr/0015`) applies with more force here, because there is no file to try again from.
//!
//! **It explains itself, in the log and on disk.** `CLAUDE.md` §3 requires that every
//! auto-applied change can answer *"why?"*. A migration is the most invisible auto-applied change
//! in the product: it happens during startup, to a customer's data, without anybody asking for it.
//! So each one that runs is (a) returned to the caller as a [`MigrationReport`], which the server
//! logs at startup, and (b) **written into the system graph** — what ran, from and to which
//! version, why, and when. The log line scrolls away; the record is still there in a year when an
//! auditor asks why a statement changed shape. It is ordinary RDF in the system graph, so the
//! answer comes back from a SPARQL query naming `FROM <urn:openbiz:graph:system>` rather than
//! from a proprietary log format.
//!
//! **A gap in the chain refuses rather than skips.** If a build ever ships without the migration
//! for a version it claims to read, the store is refused with the missing step named. Skipping
//! ahead — running 2→3 against a version-1 store because 1→2 is absent — would apply a
//! transformation to a shape it was never written for.

use std::path::Path;

use oxigraph::model::vocab::xsd;
use oxigraph::model::{Literal, NamedNode, Quad, Term};

use crate::{
    named_node, GraphId, RecordedAt, RegistryReader, StoreError, Transaction, FORMAT_VERSION,
    FORMAT_VERSION_IRI, STORE_IRI,
};

/// Predicate linking the store to a migration that has been applied to it.
const MIGRATION_APPLIED_IRI: &str = "urn:openbiz:migrationApplied";

/// Predicate carrying the version a recorded migration started from.
const MIGRATION_FROM_IRI: &str = "urn:openbiz:migrationFrom";

/// Predicate carrying the version a recorded migration ended at.
const MIGRATION_TO_IRI: &str = "urn:openbiz:migrationTo";

/// Predicate carrying a recorded migration's own description of what it did.
const MIGRATION_DESCRIPTION_IRI: &str = "urn:openbiz:migrationDescription";

/// Predicate carrying when a recorded migration ran.
const MIGRATION_AT_IRI: &str = "urn:openbiz:migrationAt";

/// Prefix under which a migration record's subject is minted.
const MIGRATION_SUBJECT_PREFIX: &str = "urn:openbiz:migration:";

/// One forward step between two adjacent store format versions.
///
/// A migration goes from `applies_at()` to `applies_at() + 1` and no further. Multi-version
/// jumps are not expressible on purpose: an upgrade across four versions is four reviewable,
/// individually-testable steps, not one function that has to know every intermediate shape.
///
/// Implementations run **inside the caller's transaction**, so a step that fails rolls back every
/// step before it along with the stamp. A step must therefore not assume it is the only writer or
/// try to flush anything itself.
///
/// Each is expected to be **idempotent** where it cheaply can be. A version stamp says what a
/// build guaranteed, not what it necessarily wrote — a store may already satisfy the invariant a
/// migration exists to establish, and the migration must handle that by doing nothing rather than
/// by duplicating.
pub trait Migration: Send + Sync {
    /// Stable identifier, written into the store's migration record and never changed afterwards.
    ///
    /// Changing one rewrites history in somebody's audit trail, so treat it as on-disk vocabulary
    /// in the same way [`crate::GraphKind::as_str`] is.
    fn id(&self) -> &'static str;

    /// The format version a store must be at for this migration to apply.
    ///
    /// Not `from_version`, which reads better and which Clippy rightly refuses on a method taking
    /// `self`: a `from_*` name is the convention for a constructor.
    fn applies_at(&self) -> u32;

    /// Why this migration exists, in one sentence an operator reads in a log or an audit trail.
    ///
    /// Written to the store alongside the record. "What changed" is more useful here than "what
    /// the code does" — this is read by someone who does not have the diff.
    fn describe(&self) -> &'static str;

    /// Apply it, inside the caller's transaction.
    fn apply(&self, transaction: &mut Transaction<'_>) -> Result<(), StoreError>;
}

/// The version this migration leaves the store at.
fn produces(migration: &dyn Migration) -> u32 {
    migration.applies_at() + 1
}

/// Every migration this build knows, in the order they apply.
///
/// The invariant — checked by [`plan`] at runtime and by a test at build time — is that the
/// versions form an unbroken run from 1 up to [`FORMAT_VERSION`]. Adding a migration and bumping
/// `FORMAT_VERSION` are one change; either alone is a bug the chain check catches.
static MIGRATIONS: &[&dyn Migration] = &[
    &RegisterSystemGraph,
    &AllowCandidateGraphs,
    &AllowCandidateRemovals,
    &RetypeIriPolicyStamps,
];

/// 1 → 2: the system graph is listed in the graph registry.
///
/// Format version 1 stamped the store before the graph registry existed, and builds that shipped
/// afterwards repaired it by re-registering the system graph on **every** open — an unconditional
/// idempotent write on the startup path, running forever for the benefit of stores that needed it
/// once. That is a migration wearing a self-heal's clothes: it could not say which stores had
/// needed it, it left no record that anything had happened, and every later additive change would
/// have been tempted to add a second one beside it.
///
/// Version 2 means the invariant *holds*. [`crate::Store::open`] checks it after migrating and
/// refuses a store that violates it, rather than silently repairing a store nobody has explained.
struct RegisterSystemGraph;

impl Migration for RegisterSystemGraph {
    fn id(&self) -> &'static str {
        "0002-register-system-graph"
    }

    fn applies_at(&self) -> u32 {
        1
    }

    fn describe(&self) -> &'static str {
        "registered the system graph in the graph registry, which format version 1 wrote before \
         the registry existed and so did not guarantee"
    }

    fn apply(&self, transaction: &mut Transaction<'_>) -> Result<(), StoreError> {
        transaction.ensure_registered(&GraphId::system())?;
        Ok(())
    }
}

/// 2 → 3: the registry may hold graphs of kind `candidate`.
///
/// This is the first migration that **changes nothing on disk**, and that is worth stating rather
/// than hiding, because iteration 16 set the rule it appears to break: a version that records no
/// real difference teaches the next reader that versions are decorative.
///
/// The difference is real; it is just additive. Format version 3 introduced the candidate seam,
/// whose staging graphs are registered under a fourth [`crate::GraphKind`]. Every version-2 store
/// is already a valid version-3 store — there is nothing to bring forward — but a version-3 store
/// is **not** readable by a build that predates the seam: it reads `candidate` out of the
/// registry, finds a kind it does not know, and reports the whole registry as corrupt metadata.
/// That is the right refusal for the wrong reason, and it sends an operator who has merely
/// downgraded off to disaster recovery.
///
/// So the version exists to move that refusal to where it belongs: an older build sees a stamp
/// from the future and says *upgrade*. The invariant version 3 records is that every graph kind
/// and every system-graph record in the store is one a build with the candidate seam understands.
/// A migration that writes nothing is the honest implementation of an invariant that was already
/// satisfied — the alternative, inventing a write so the step looks substantial, is the thing that
/// would actually teach the wrong lesson.
struct AllowCandidateGraphs;

impl Migration for AllowCandidateGraphs {
    fn id(&self) -> &'static str {
        "0003-allow-candidate-graphs"
    }

    fn applies_at(&self) -> u32 {
        2
    }

    fn describe(&self) -> &'static str {
        "recorded that this store may hold candidate graphs and candidate records, which nothing \
         on disk needed changing for; the version exists so a build without the candidate seam \
         refuses the store as too new rather than reporting its registry as corrupt"
    }

    fn apply(&self, _transaction: &mut Transaction<'_>) -> Result<(), StoreError> {
        Ok(())
    }
}

/// 3 → 4: a candidate may propose removals as well as additions.
///
/// The second migration that changes nothing on disk, and the reason is sharper than the last
/// one's. Format version 4 gives a candidate a second staging graph and two more fields on its
/// record, both of which a version-3 build ignores rather than refuses — `read_record` looks up
/// the predicates it knows and does not object to ones it does not.
///
/// That is precisely why the version exists. A version-3 build opening this store would read a
/// candidate that removes twelve statements as a candidate that removes nothing, show a reviewer a
/// diff missing half its content, and on approval apply only the additions **while recording that
/// the whole candidate was applied**. Every step of that succeeds; nothing anywhere says the
/// vocabulary is now different from what was approved. A refusal at open is the only place that
/// can be caught, and the stamp is what makes the refusal happen.
///
/// Nothing is brought forward because nothing needs to be: a version-3 store's candidates are
/// additions-only, and an absent removal count means zero by construction rather than by a
/// default this migration writes in.
struct AllowCandidateRemovals;

impl Migration for AllowCandidateRemovals {
    fn id(&self) -> &'static str {
        "0004-allow-candidate-removals"
    }

    fn applies_at(&self) -> u32 {
        3
    }

    fn describe(&self) -> &'static str {
        "recorded that a candidate in this store may propose removals, which no existing candidate \
         needed changing for; the version exists because a build without removals would read such \
         a candidate as removing nothing, show a reviewer half the diff, and apply half the change \
         while recording that it had applied all of it"
    }

    fn apply(&self, _transaction: &mut Transaction<'_>) -> Result<(), StoreError> {
        Ok(())
    }
}

/// 4 → 5: a recorded IRI policy's timestamp is an `xsd:dateTime` rather than a plain string.
///
/// **The first migration in the chain that actually rewrites data**, and the defect it repairs is
/// small to describe and awkward to live with. A candidate's `proposed_at` and a migration's own
/// `migrationAt` were written as typed `xsd:dateTime` literals from the day each shipped. An IRI
/// policy's `iriPatternRecordedAt` was written as a plain string, holding the identical lexical
/// form, through an oversight nothing surfaced — the value looked right everywhere a person read
/// it, because everywhere a person read it, it was being printed rather than compared.
///
/// It is not orderable. A plain literal is `xsd:string` to SPARQL, so `ORDER BY`, `FILTER (?at >
/// …)`, and a comparison against any other timestamp in the trail all either sort it
/// lexicographically by accident or fail to relate it at all. The one question the field exists to
/// answer — *which convention was this vocabulary minting under when that concept was created* —
/// is a comparison between a policy's stamp and a candidate's, and it silently could not be asked.
/// `CLAUDE.md` §3 makes the audit trail ordinary RDF answerable by SPARQL precisely so an auditor
/// does not have to take our word for it; a field that is only correct when a human reads it is
/// not that.
///
/// **What it deliberately does not do is refuse.** A value it cannot parse as a date and time with
/// an explicit timezone is left exactly as it was found, rather than retyped into an ill-typed
/// literal or turned into a fatal error at open. Retyping garbage would mint a lie; refusing at
/// open would turn one unreadable field in one vocabulary into a store that will not start, and
/// send an operator to disaster recovery for a record they could have read the pattern out of by
/// hand. [`crate::policy`] already refuses such a value at the read, naming the vocabulary — which
/// is where a per-vocabulary problem belongs.
struct RetypeIriPolicyStamps;

/// The predicate [`RetypeIriPolicyStamps`] repairs. Kept here rather than imported so the
/// migration still describes a shape the current code has since moved on from — a migration reads
/// the store as it *was*, and coupling it to a constant later versions may rename is how a step
/// stops meaning what it meant.
const LEGACY_IRI_PATTERN_AT_IRI: &str = "urn:openbiz:iriPatternRecordedAt";

impl Migration for RetypeIriPolicyStamps {
    fn id(&self) -> &'static str {
        "0005-retype-iri-policy-stamps"
    }

    fn applies_at(&self) -> u32 {
        4
    }

    fn describe(&self) -> &'static str {
        "retyped every recorded IRI-minting policy's timestamp from a plain string to an \
         xsd:dateTime, so the trail can be ordered and compared in SPARQL rather than only read \
         by eye; a value that is not a date and time with an explicit timezone was left as found, \
         because retyping one would assert something the record does not say"
    }

    fn apply(&self, transaction: &mut Transaction<'_>) -> Result<(), StoreError> {
        let predicate = named_node(LEGACY_IRI_PATTERN_AT_IRI);
        let found = transaction.inner.system_quads(None, predicate)?;

        let mut stale = Vec::new();
        let mut fresh = Vec::new();
        for quad in found {
            let Term::Literal(literal) = &quad.object else {
                // Not a literal at all. Out of this step's remit and refused at the read, which is
                // where a record nobody can account for should surface.
                continue;
            };
            if literal.datatype() == xsd::DATE_TIME {
                // Already typed: either written by a build at version 5 or later, or repaired by
                // an earlier run of this step. Idempotent, as the trait asks.
                continue;
            }
            if RecordedAt::parse(literal.value()).is_err() {
                continue;
            }
            fresh.push(Quad::new(
                quad.subject.clone(),
                quad.predicate.clone(),
                Literal::new_typed_literal(literal.value(), xsd::DATE_TIME),
                quad.graph_name.clone(),
            ));
            stale.push(quad);
        }

        transaction.remove_graph_quads(&GraphId::system(), &stale)?;
        transaction.extend_graph(&GraphId::system(), &fresh)
    }
}

/// One migration that ran, as reported to a caller.
///
/// Owns nothing borrowed from the chain: a report outlives the transaction that produced it and
/// is what the server logs after the store is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationStep {
    /// The migration's stable identifier.
    pub id: &'static str,
    /// The version it started from.
    pub from_version: u32,
    /// The version it ended at.
    pub to_version: u32,
    /// Its own description of what it did.
    pub description: &'static str,
}

impl std::fmt::Display for MigrationStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({} → {}): {}",
            self.id, self.from_version, self.to_version, self.description
        )
    }
}

/// What opening or restoring a store did to its format, if anything.
///
/// An empty report is the normal case and says so: a store already at the current version is not
/// migrated, and reporting "0 migrations" is different from reporting nothing at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    from: u32,
    to: u32,
    steps: Vec<MigrationStep>,
}

impl MigrationReport {
    /// A report for a store that needed no migration.
    pub(crate) fn none(version: u32) -> Self {
        Self {
            from: version,
            to: version,
            steps: Vec::new(),
        }
    }

    /// The version the store was at before.
    pub fn previous_version(&self) -> u32 {
        self.from
    }

    /// The version the store is at now.
    pub fn current_version(&self) -> u32 {
        self.to
    }

    /// Every migration that ran, in the order they ran.
    pub fn steps(&self) -> &[MigrationStep] {
        &self.steps
    }

    /// Whether anything was migrated.
    pub fn migrated(&self) -> bool {
        !self.steps.is_empty()
    }
}

impl std::fmt::Display for MigrationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.migrated() {
            return write!(f, "no migration needed; store format version {}", self.to);
        }
        write!(
            f,
            "migrated the store format from version {} to {}: ",
            self.from, self.to
        )?;
        for (index, step) in self.steps.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{step}")?;
        }
        Ok(())
    }
}

/// The chain that takes a store from `from` to `target`.
///
/// `Err(missing)` names the first version with no migration out of it, which is the only useful
/// thing to tell an operator: it identifies the build that should have shipped one.
///
/// Takes the chain and the target rather than reading [`MIGRATIONS`] and [`FORMAT_VERSION`], so
/// the engine can be tested on migrations that do not exist — including ones that fail, which the
/// real chain has no example of and should not acquire one for the sake of a test.
fn plan_from<'a>(
    from: u32,
    migrations: &[&'a dyn Migration],
    target: u32,
) -> Result<Vec<&'a dyn Migration>, u32> {
    let mut chain = Vec::new();
    let mut at = from;

    while at < target {
        match migrations
            .iter()
            .find(|migration| migration.applies_at() == at)
        {
            Some(migration) => {
                chain.push(*migration);
                at = produces(*migration);
            }
            None => return Err(at),
        }
    }

    Ok(chain)
}

/// Bring a store at version `found` forward to [`FORMAT_VERSION`], inside the caller's transaction.
///
/// Applies every step in order, records each one in the system graph, and re-stamps the store.
/// Does nothing at all — no writes, no records — when the store is already current, so the common
/// path costs one comparison.
///
/// Refuses a store from the future rather than trusting the caller to have checked, because this
/// is the function that would otherwise run a 1→2 transformation over a version-9 store.
pub(crate) fn migrate(
    transaction: &mut Transaction<'_>,
    found: u32,
    path: &Path,
) -> Result<MigrationReport, StoreError> {
    migrate_from(transaction, found, path, MIGRATIONS, FORMAT_VERSION)
}

/// [`migrate`] against an arbitrary chain and target. See [`plan_from`] for why it exists.
fn migrate_from(
    transaction: &mut Transaction<'_>,
    found: u32,
    path: &Path,
    migrations: &[&dyn Migration],
    target: u32,
) -> Result<MigrationReport, StoreError> {
    if found > target {
        return Err(StoreError::FormatTooNew {
            path: path.to_path_buf(),
            found,
            supported: target,
        });
    }

    if found == target {
        return Ok(MigrationReport::none(found));
    }

    let chain =
        plan_from(found, migrations, target).map_err(|missing| StoreError::NoMigrationPath {
            path: path.to_path_buf(),
            found,
            supported: target,
            missing,
        })?;

    // One timestamp for the whole chain, not one per step. They commit together, so they happened
    // together; giving them microsecond-apart times would imply an ordering that is not a fact.
    let at = RecordedAt::now();

    let mut steps = Vec::with_capacity(chain.len());
    for migration in chain {
        migration.apply(transaction)?;

        let step = MigrationStep {
            id: migration.id(),
            from_version: migration.applies_at(),
            to_version: produces(migration),
            description: migration.describe(),
        };
        record(transaction, &step, &at)?;
        steps.push(step);
    }

    stamp(transaction, target)?;

    Ok(MigrationReport {
        from: found,
        to: target,
        steps,
    })
}

/// Write a migration's record into the system graph.
fn record(
    transaction: &mut Transaction<'_>,
    step: &MigrationStep,
    at: &RecordedAt,
) -> Result<(), StoreError> {
    // Unchecked because the identifier is a compile-time constant from this crate and the prefix
    // is ours; `expect` is barred outside tests (`CLAUDE.md` §6) and there is no runtime input
    // here that could make it fail.
    let subject = NamedNode::new_unchecked(format!("{MIGRATION_SUBJECT_PREFIX}{}", step.id));

    transaction.insert(
        &GraphId::system(),
        vec![
            (
                named_node(STORE_IRI).into_owned(),
                named_node(MIGRATION_APPLIED_IRI).into_owned(),
                subject.clone().into(),
            ),
            (
                subject.clone(),
                named_node(MIGRATION_FROM_IRI).into_owned(),
                Literal::new_typed_literal(step.from_version.to_string(), xsd::INTEGER).into(),
            ),
            (
                subject.clone(),
                named_node(MIGRATION_TO_IRI).into_owned(),
                Literal::new_typed_literal(step.to_version.to_string(), xsd::INTEGER).into(),
            ),
            (
                subject.clone(),
                named_node(MIGRATION_DESCRIPTION_IRI).into_owned(),
                Literal::new_simple_literal(step.description).into(),
            ),
            (
                subject,
                named_node(MIGRATION_AT_IRI).into_owned(),
                Literal::new_typed_literal(at.as_str(), xsd::DATE_TIME).into(),
            ),
        ],
    )
}

/// Set the store's format stamp to `version`, removing whatever was there.
///
/// Removes *every* existing stamp rather than the one it expects to find. A store with two stamps
/// is refused on open, so this can only meet one — but the operation that repairs the store's
/// version is the wrong place to depend on the store's version already being sane.
pub(crate) fn stamp(transaction: &mut Transaction<'_>, version: u32) -> Result<(), StoreError> {
    let subject = named_node(STORE_IRI);
    let predicate = named_node(FORMAT_VERSION_IRI);

    let existing = transaction.inner.system_quads(Some(subject), predicate)?;
    for quad in existing {
        transaction.inner.remove(&quad);
    }

    transaction.insert(
        &GraphId::system(),
        vec![(
            subject.into_owned(),
            predicate.into_owned(),
            Literal::new_typed_literal(version.to_string(), xsd::INTEGER).into(),
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chain_runs_unbroken_from_version_one_to_the_current_format() {
        // The check that makes every other migration test meaningful: bumping `FORMAT_VERSION`
        // without adding the migration for it — or the reverse — fails here rather than on a
        // customer's store.
        for version in 1..FORMAT_VERSION {
            let chain = plan_from(version, MIGRATIONS, FORMAT_VERSION).unwrap_or_else(|missing| {
                panic!(
                    "no migration out of format version {missing}, so a store at version \
                     {version} cannot reach {FORMAT_VERSION}"
                )
            });
            assert_eq!(
                chain.first().map(|step| step.applies_at()),
                Some(version),
                "the chain from {version} must start there"
            );
            assert_eq!(
                chain.last().map(|step| produces(*step)),
                Some(FORMAT_VERSION),
                "the chain from {version} must end at the current format"
            );
        }

        assert!(
            plan_from(FORMAT_VERSION, MIGRATIONS, FORMAT_VERSION)
                .expect("a current store plans cleanly")
                .is_empty(),
            "a store already at the current version needs no steps"
        );
    }

    /// A store as format version 4 left it: the policy stamp holding the right lexical form in
    /// the wrong datatype.
    fn store_with_a_legacy_policy_stamp(lexical: &str) -> (tempfile::TempDir, crate::Store) {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = crate::Store::open(dir.path()).expect("a fresh store opens");
        let graph = GraphId::vocabulary("https://example.org/energy").expect("a vocabulary IRI");
        store
            .create_vocabulary_graph(&graph)
            .expect("the vocabulary is created");
        store
            .record_iri_policy(&graph, "https://example.org/energy/c_{n}", "ada")
            .expect("the policy is recorded");
        put_stamp(&store, Literal::new_simple_literal(lexical));
        (dir, store)
    }

    /// Replace whatever stamp is recorded with `object`, leaving exactly one.
    fn put_stamp(store: &crate::Store, object: Literal) {
        let system = GraphId::system();
        let existing = store
            .backend
            .system_quads(None, named_node(LEGACY_IRI_PATTERN_AT_IRI))
            .expect("the system graph is readable");
        let subject = existing
            .first()
            .map(|quad| quad.subject.clone())
            .expect("a policy was recorded, so a stamp exists");
        store
            .transaction(|txn| {
                txn.remove_graph_quads(&system, &existing)?;
                txn.extend_graph(
                    &system,
                    &[Quad::new(
                        subject.clone(),
                        named_node(LEGACY_IRI_PATTERN_AT_IRI).into_owned(),
                        object.clone(),
                        NamedNode::new_unchecked(system.iri()),
                    )],
                )
            })
            .expect("the stamp is written");
    }

    /// The one stamp recorded in the store, as it stands.
    fn recorded_stamp(store: &crate::Store) -> Literal {
        let found = store
            .backend
            .system_quads(None, named_node(LEGACY_IRI_PATTERN_AT_IRI))
            .expect("the system graph is readable");
        let [quad] = found.as_slice() else {
            panic!("one policy records one stamp: {found:?}");
        };
        let Term::Literal(literal) = &quad.object else {
            panic!("the stamp is a literal: {quad:?}");
        };
        literal.clone()
    }

    #[test]
    fn a_legacy_policy_stamp_is_retyped_without_changing_what_it_says() {
        let (_dir, store) = store_with_a_legacy_policy_stamp("2026-08-19T14:17:03Z");
        assert_ne!(
            recorded_stamp(&store).datatype(),
            xsd::DATE_TIME,
            "the fixture must start in the shape version 4 left"
        );

        store
            .transaction(|txn| RetypeIriPolicyStamps.apply(txn))
            .expect("the step runs");

        let repaired = recorded_stamp(&store);
        assert_eq!(repaired.datatype(), xsd::DATE_TIME);
        assert_eq!(
            repaired.value(),
            "2026-08-19T14:17:03Z",
            "the step changes the datatype and not the instant"
        );
    }

    #[test]
    fn retyping_twice_leaves_one_stamp() {
        // The trait asks for idempotence where it is cheap, and it is cheap here. A step that
        // inserted a second typed quad on a re-run would leave a policy recording two times, which
        // `policy::read_policy` refuses as corrupt — a migration turning a good store bad.
        let (_dir, store) = store_with_a_legacy_policy_stamp("2026-08-19T14:17:03Z");
        for _ in 0..2 {
            store
                .transaction(|txn| RetypeIriPolicyStamps.apply(txn))
                .expect("the step runs");
        }
        assert_eq!(recorded_stamp(&store).datatype(), xsd::DATE_TIME);
    }

    #[test]
    fn a_stamp_the_step_cannot_read_is_left_exactly_as_found() {
        // Deliberate: retyping it would assert it is a date and time, which it is not, and
        // refusing here would turn one unreadable field in one vocabulary into a store that will
        // not open. The read refuses it, naming the vocabulary, which is where it belongs.
        for lexical in ["last Tuesday", "2026-08-19T14:17:03", "2026-08-19", ""] {
            let (_dir, store) = store_with_a_legacy_policy_stamp(lexical);
            store
                .transaction(|txn| RetypeIriPolicyStamps.apply(txn))
                .expect("the step runs");

            let untouched = recorded_stamp(&store);
            assert_ne!(
                untouched.datatype(),
                xsd::DATE_TIME,
                "{lexical:?} is not a date and time and must not be labelled one"
            );
            assert_eq!(untouched.value(), lexical, "the record is left as found");
        }
    }

    /// The whole thing through the only caller that matters: a store on disk stamped at the
    /// previous format version, opened by this build.
    ///
    /// The other chain tests drive fixture migrations, which prove the machinery and not the step.
    /// This one puts a real version-4 store in front of `Store::open` — the stamp rolled back, the
    /// policy timestamp in the shape that build wrote — and asks what an operator upgrading would
    /// actually get.
    #[test]
    fn opening_a_version_four_store_retypes_its_policy_stamp_and_records_that_it_did() {
        let (dir, store) = store_with_a_legacy_policy_stamp("2026-08-19T14:17:03Z");
        store
            .transaction(|txn| stamp(txn, FORMAT_VERSION - 1))
            .expect("roll the store back to the previous format version");
        store.close().expect("the store closes cleanly");

        let reopened = crate::Store::open(dir.path()).expect("an older store opens");

        assert_eq!(reopened.format_version(), FORMAT_VERSION);
        assert_eq!(
            recorded_stamp(&reopened).datatype(),
            xsd::DATE_TIME,
            "opening it is what repairs it; nothing else runs on an operator's behalf"
        );

        let ran: Vec<&str> = reopened
            .migrations()
            .steps()
            .iter()
            .map(|step| step.id)
            .collect();
        assert_eq!(
            ran,
            ["0005-retype-iri-policy-stamps"],
            "one step was owed and one ran: {ran:?}"
        );

        // And the policy still reads, which is the point of having repaired it.
        let graph = GraphId::vocabulary("https://example.org/energy").expect("a vocabulary IRI");
        let policy = reopened
            .iri_policy(&graph)
            .expect("readable")
            .expect("a policy");
        assert_eq!(policy.recorded_at(), "2026-08-19T14:17:03Z");
    }

    #[test]
    fn every_migration_advances_by_exactly_one_version_and_is_uniquely_identified() {
        let mut ids: Vec<&str> = MIGRATIONS.iter().map(|migration| migration.id()).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(count, ids.len(), "two migrations share an identifier");

        let mut froms: Vec<u32> = MIGRATIONS
            .iter()
            .map(|migration| migration.applies_at())
            .collect();
        froms.sort_unstable();
        let count = froms.len();
        froms.dedup();
        assert_eq!(
            count,
            froms.len(),
            "two migrations claim the same starting version, so the chain is ambiguous"
        );

        for migration in MIGRATIONS {
            assert!(
                migration.applies_at() >= 1,
                "{}: version 0 never existed",
                migration.id()
            );
            assert!(
                produces(*migration) <= FORMAT_VERSION,
                "{}: migrates past the version this build claims to write",
                migration.id()
            );
            assert!(
                !migration.describe().is_empty(),
                "{}: a migration that cannot explain itself is one `CLAUDE.md` §3 forbids",
                migration.id()
            );
        }
    }

    #[test]
    fn a_gap_in_the_chain_is_reported_by_the_version_that_is_missing() {
        // Version 0 has no migration out of it and never will — nothing was ever stamped 0. It
        // stands in here for the real case: a build that bumps the format and forgets the step.
        assert!(
            matches!(plan_from(0, MIGRATIONS, FORMAT_VERSION), Err(0)),
            "a gap must name the version it is at"
        );
    }

    #[test]
    fn a_report_that_migrated_nothing_says_so_rather_than_saying_nothing() {
        let report = MigrationReport::none(FORMAT_VERSION);

        assert!(!report.migrated());
        assert_eq!(report.previous_version(), FORMAT_VERSION);
        assert_eq!(report.current_version(), FORMAT_VERSION);
        assert_eq!(
            report.to_string(),
            format!("no migration needed; store format version {FORMAT_VERSION}")
        );
    }

    /// A migration that appends a statement naming itself, so a test can see which ran and in
    /// what order without depending on what the real chain happens to do.
    struct Marker {
        id: &'static str,
        from: u32,
    }

    impl Migration for Marker {
        fn id(&self) -> &'static str {
            self.id
        }
        fn applies_at(&self) -> u32 {
            self.from
        }
        fn describe(&self) -> &'static str {
            "left a marker"
        }
        fn apply(&self, transaction: &mut Transaction<'_>) -> Result<(), StoreError> {
            transaction.insert(
                &GraphId::system(),
                vec![(
                    NamedNode::new_unchecked("urn:openbiz:test:marker"),
                    NamedNode::new_unchecked("urn:openbiz:test:ran"),
                    Literal::new_simple_literal(self.id).into(),
                )],
            )
        }
    }

    /// A migration that always fails, to prove a chain rolls back rather than half-applying.
    struct Explodes;

    impl Migration for Explodes {
        fn id(&self) -> &'static str {
            "test-explodes"
        }
        fn applies_at(&self) -> u32 {
            2
        }
        fn describe(&self) -> &'static str {
            "fails on purpose"
        }
        fn apply(&self, _: &mut Transaction<'_>) -> Result<(), StoreError> {
            Err(StoreError::Backend("the migration failed".to_owned()))
        }
    }

    fn markers(store: &crate::Store) -> Vec<String> {
        store
            .backend
            .quads_for_pattern(
                None,
                Some(named_node("urn:openbiz:test:ran")),
                None,
                Some(named_node(crate::SYSTEM_GRAPH_IRI).into()),
            )
            .map(|quad| quad.expect("readable").object.to_string())
            .collect()
    }

    #[test]
    fn a_chain_applies_every_step_in_order_and_stamps_once_at_the_end() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = crate::Store::open(dir.path()).expect("open the store");

        let first = Marker {
            id: "test-first",
            from: 1,
        };
        let second = Marker {
            id: "test-second",
            from: 2,
        };
        // Deliberately out of order in the slice: the *chain* is what orders the steps, not the
        // order somebody happened to list them in.
        let chain: &[&dyn Migration] = &[&second, &first];

        let report = store
            .transaction(|txn| migrate_from(txn, 1, dir.path(), chain, 3))
            .expect("the chain applies");

        assert_eq!(report.previous_version(), 1);
        assert_eq!(report.current_version(), 3);
        assert_eq!(
            report
                .steps()
                .iter()
                .map(|step| step.id)
                .collect::<Vec<_>>(),
            vec!["test-first", "test-second"]
        );
        assert_eq!(markers(&store), vec![r#""test-first""#, r#""test-second""#]);

        // Exactly one stamp, at the end of the chain, not one per step.
        let stamps = store
            .backend
            .system_quads(Some(named_node(STORE_IRI)), named_node(FORMAT_VERSION_IRI))
            .expect("readable");
        assert_eq!(stamps.len(), 1, "a store may carry exactly one stamp");
        assert_eq!(
            stamps[0].object.to_string(),
            r#""3"^^<http://www.w3.org/2001/XMLSchema#integer>"#
        );
    }

    #[test]
    fn a_step_that_fails_rolls_back_every_step_before_it_and_the_stamp() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = crate::Store::open(dir.path()).expect("open the store");

        let first = Marker {
            id: "test-first",
            from: 1,
        };
        let chain: &[&dyn Migration] = &[&first, &Explodes];

        let error = store
            .transaction(|txn| migrate_from(txn, 1, dir.path(), chain, 3))
            .expect_err("the second step fails");
        assert!(
            matches!(error, StoreError::Backend(_)),
            "unexpected error: {error}"
        );

        assert!(
            markers(&store).is_empty(),
            "the first step's write survived a failure in the second, so the store is now in a \
             shape no format version describes"
        );
        assert_eq!(
            store.format_version(),
            FORMAT_VERSION,
            "the stamp moved despite the chain failing"
        );
    }

    #[test]
    fn a_chain_with_a_gap_refuses_rather_than_skipping_the_missing_step() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = crate::Store::open(dir.path()).expect("open the store");

        // 1 → 2 is missing; 2 → 3 exists. Running it against a version-1 store would apply a
        // transformation to a shape it was never written for.
        let second = Marker {
            id: "test-second",
            from: 2,
        };
        let chain: &[&dyn Migration] = &[&second];

        let error = store
            .transaction(|txn| migrate_from(txn, 1, dir.path(), chain, 3))
            .expect_err("a gap must be refused");

        assert!(
            matches!(
                error,
                StoreError::NoMigrationPath {
                    found: 1,
                    supported: 3,
                    missing: 1,
                    ..
                }
            ),
            "unexpected error: {error}"
        );
        assert!(
            markers(&store).is_empty(),
            "a refused chain applied a step anyway"
        );
    }

    #[test]
    fn a_report_names_every_step_it_ran_and_why() {
        let report = MigrationReport {
            from: 1,
            to: 2,
            steps: vec![MigrationStep {
                id: "0002-register-system-graph",
                from_version: 1,
                to_version: 2,
                description: "registered the system graph",
            }],
        };

        let text = report.to_string();
        assert!(text.contains("version 1 to 2"), "{text}");
        assert!(text.contains("0002-register-system-graph"), "{text}");
        assert!(
            text.contains("registered the system graph"),
            "the *why* is the part an operator needs: {text}"
        );
    }
}
