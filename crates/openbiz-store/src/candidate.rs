//! The candidate seam: a change to a vocabulary arrives as a proposal, not as a write.
//!
//! `CLAUDE.md` §3 states the rule this module implements: *any path that changes a vocabulary
//! takes candidates, not just direct writes.* A candidate is a proposed change carrying its
//! provenance, its source, and a confidence where one is meaningful, which a human reviews before
//! it lands. The shape is the same whether the proposal came from a file import, a discovery match
//! against another vocabulary, a bulk edit, or — in Phase 10 — an agent. Building it once means
//! every later producer slots in behind an existing seam instead of being retrofitted.
//!
//! # Why the payload is a named graph rather than a serialised blob
//!
//! A proposal has to be *reviewable*, and the thing a reviewer wants is the statements. Keeping
//! them as RDF quads in their own named graph — [`crate::graph::CANDIDATE_GRAPH_PREFIX`] plus the
//! candidate's identifier — means the review question is answered by the tools that already exist:
//! `GET /api/export?graph=…` serialises a pending change into any of the six syntaxes, and a
//! SPARQL query naming `FROM <urn:openbiz:graph:candidate:7>` asks anything else about it. The
//! alternative — a literal holding a chunk of Turtle — would have been quicker to write and would
//! have made the proposed statements opaque to every query in the product.
//!
//! It also means approval is a *copy between graphs inside one transaction*, so a partly-applied
//! candidate is not a state that can exist.
//!
//! # What a candidate does not pollute
//!
//! A staging graph is registered, so `GET /api/graphs` and an operator asking "what is actually in
//! my store?" can both see it — statements the store holds and cannot describe are the opacity
//! `CLAUDE.md` §1 exists to attack. But it is registered as [`crate::GraphKind::Candidate`], which
//! keeps it out of the interface's vocabulary list and out of the SPARQL endpoint's default
//! dataset, which is the registered *vocabulary* graphs. So a pending proposal is visible to
//! anyone who looks for it and invisible to everyone querying their vocabularies — which is
//! exactly what "not yet approved" has to mean.
//!
//! # A candidate has two halves, and the removing one has a precondition
//!
//! A proposal that can only add cannot express a merge, a split, a move, or a deprecation, so a
//! candidate carries two staging graphs: what it would add and what it would take away. They are
//! separate graphs rather than one graph with a marker on each statement, so a reviewer can export
//! either half on its own through the export path that already exists.
//!
//! Removals differ from additions in a way that is easy to miss and expensive to get wrong.
//! Adding a statement that is already there is a no-op, so an addition is true whenever it is
//! applied. **A removal names statements that must already exist**, and the vocabulary can change
//! between the moment a proposal is raised and the moment somebody approves it. Applying a stale
//! removal would take away fewer statements than the reviewer agreed to and report success, which
//! is the quietest possible way for a governance tool to lie.
//!
//! So a removal is checked twice against the vocabulary, and refused rather than trimmed:
//! [`Store::propose_retraction`] refuses to stage a statement the vocabulary does not hold, and
//! [`Store::decide`] refuses to *apply* a candidate whose removals are no longer all present. The
//! second check is the one that matters — the first only catches a producer working from a stale
//! read, whereas the second catches the vocabulary moving underneath a pending review.
//!
//! # Scope of this build
//!
//! A candidate raised from a *file* has exactly one of its two halves, because a file is one half:
//! `openbiz import` raises additions and `openbiz retract` raises removals. A candidate carrying
//! **both** is what a bulk operation raises, through [`Store::propose_edit`], which takes computed
//! statements rather than a stream — `openbiz move` is its first producer and the rest of the bulk
//! operations in `docs/BUILD-PLAN.md` are the others. Everything a producer is allowed to compute
//! is checked here rather than trusted, because a computed statement has had no parser look at it.
//!
//! Approval applies immediately, so the terminal states are [`CandidateState::Applied`] and
//! [`CandidateState::Rejected`] rather than an "approved but not yet applied" limbo. When Phase 6
//! gives approval a workflow of its own, that limbo becomes real and gets its own state; naming a
//! state today that nothing can produce would be a claim about a capability we do not have.

use std::collections::{BTreeMap, HashSet};
use std::io::Read;

use oxigraph::io::RdfParser;
use oxigraph::model::vocab::{rdf, xsd};
use oxigraph::model::{BlankNode, GraphName, Literal, NamedNode, NamedOrBlankNode, Quad, Term};
use thiserror::Error;

use crate::{
    named_node, CandidatePart, GraphId, GraphKind, RdfSyntax, RecordedAt, RegistryReader,
    StatementRef, StatementTerm, Store, StoreError, Transaction,
};

/// The class every candidate's record is typed with, in the system graph.
const CANDIDATE_CLASS_IRI: &str = "urn:openbiz:Candidate";

/// Prefix under which a candidate's record subject is minted.
const CANDIDATE_SUBJECT_PREFIX: &str = "urn:openbiz:candidate:";

/// Predicate naming the vocabulary graph a candidate proposes to change.
const TARGET_IRI: &str = "urn:openbiz:candidateTarget";

/// Predicate naming the graph the proposed additions are staged in.
const PAYLOAD_IRI: &str = "urn:openbiz:candidatePayload";

/// Predicate naming the graph the proposed removals are staged in.
///
/// Absent when the candidate removes nothing — which is every candidate a build before format
/// version 4 could write, so its absence means "removes nothing" rather than "record is broken".
const REMOVAL_PAYLOAD_IRI: &str = "urn:openbiz:candidateRemovalPayload";

/// Predicate carrying what kind of producer raised the candidate.
const SOURCE_IRI: &str = "urn:openbiz:candidateSource";

/// Predicate carrying who or what raised the candidate.
const AGENT_IRI: &str = "urn:openbiz:candidateAgent";

/// Predicate carrying the candidate's own one-line account of why it exists.
const NOTE_IRI: &str = "urn:openbiz:candidateNote";

/// Predicate carrying when the candidate was raised.
const PROPOSED_AT_IRI: &str = "urn:openbiz:candidateProposedAt";

/// Predicate carrying how many statements the candidate proposes to add.
const ADDITIONS_IRI: &str = "urn:openbiz:candidateAdditions";

/// Predicate carrying how many statements the candidate proposes to remove.
///
/// Absent means zero, for the same reason [`REMOVAL_PAYLOAD_IRI`] may be absent.
const REMOVALS_IRI: &str = "urn:openbiz:candidateRemovals";

/// Predicate carrying the producer's confidence, where it has one.
const CONFIDENCE_IRI: &str = "urn:openbiz:candidateConfidence";

/// Predicate carrying where the candidate is in its lifecycle.
const STATE_IRI: &str = "urn:openbiz:candidateState";

/// Predicate carrying who decided the candidate's fate.
const DECIDED_BY_IRI: &str = "urn:openbiz:candidateDecidedBy";

/// Predicate carrying when that decision was taken.
const DECIDED_AT_IRI: &str = "urn:openbiz:candidateDecidedAt";

/// How many staged quads are held before being handed to the transaction.
///
/// The same figure the restore path uses, for the same reason: it bounds the *intermediate* Vec,
/// not the transaction's write batch, which the backend holds in memory until commit either way.
const STAGE_BATCH: usize = 10_000;

/// Identifies one candidate.
///
/// A decimal ordinal rather than a random identifier, and deliberately so: candidates are an audit
/// trail, and an operator reading one wants to know which proposal came before which. It is minted
/// by the store — the next number after the highest one it holds — under the write lock that
/// serialises every write, so two proposals raced against each other cannot receive the same
/// number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CandidateId(u64);

/// The text offered was not a candidate identifier.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "{offered:?} does not identify a candidate: {detail}. A candidate is identified by a decimal \
     number, as in 7"
)]
pub struct CandidateIdError {
    /// What was offered.
    offered: String,
    /// Why it was not one.
    detail: &'static str,
}

impl CandidateId {
    /// Read an identifier a human typed or a store held.
    ///
    /// Strict about the lexical form, not merely about the value: `007` is refused even though it
    /// parses as seven. The identifier appears in an IRI — the record's subject and the staging
    /// graph's name — so two spellings of one number would be two graphs, and a proposal that can
    /// be addressed two ways is one an audit trail cannot pin down.
    pub fn parse(offered: &str) -> Result<Self, CandidateIdError> {
        let refuse = |detail: &'static str| CandidateIdError {
            offered: offered.to_owned(),
            detail,
        };

        let value: u64 = offered
            .parse()
            .map_err(|_| refuse("it is not a decimal number"))?;

        if offered != value.to_string() {
            return Err(refuse(
                "it is written in a form that is not the number's own spelling",
            ));
        }

        Ok(Self(value))
    }

    /// The IRI of this candidate's record in the system graph.
    fn subject(self) -> NamedNode {
        // Unchecked because the prefix is a compile-time constant of ours and the suffix is a
        // decimal number, so the concatenation is always a valid IRI. `expect` is barred outside
        // tests (`CLAUDE.md` §6) and there is no runtime input here that could make it fail.
        NamedNode::new_unchecked(format!("{CANDIDATE_SUBJECT_PREFIX}{}", self.0))
    }
}

impl std::fmt::Display for CandidateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// What kind of producer raised a candidate.
///
/// A closed set, written to disk as a token, because the whole point of recording a source is that
/// a reviewer can filter and rank by it — "everything an assistant proposed" is a question a
/// governance team will ask on their first day with the product, and a free-text field cannot
/// answer it. `#[non_exhaustive]` because later phases add producers; [`Self::parse`] refuses a
/// token it does not know rather than defaulting, so a store written by a build that knew one more
/// is refused rather than misread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum CandidateSource {
    /// Statements read from a file or stream in one of the RDF syntaxes.
    Import,
    /// A match found against another vocabulary by a `DiscoveryProvider` (Phase 12).
    Discovery,
    /// A single change made by a person, through the interface or the API.
    Manual,
    /// One change of many raised together by an operation over a set of concepts (Phase 2).
    BulkEdit,
    /// A proposal from an LLM agent (Phase 10), which may never write without approval.
    Assistant,
}

impl CandidateSource {
    /// The token written to the store.
    ///
    /// Stable on-disk vocabulary: changing one of these strings rewrites somebody's audit trail,
    /// so it is a store format change and not an edit.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::Discovery => "discovery",
            Self::Manual => "manual",
            Self::BulkEdit => "bulk-edit",
            Self::Assistant => "assistant",
        }
    }

    /// Read a token back. `None` for anything this build does not know.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "import" => Some(Self::Import),
            "discovery" => Some(Self::Discovery),
            "manual" => Some(Self::Manual),
            "bulk-edit" => Some(Self::BulkEdit),
            "assistant" => Some(Self::Assistant),
            _ => None,
        }
    }
}

impl std::fmt::Display for CandidateSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a candidate is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum CandidateState {
    /// Raised, staged, and waiting for a human.
    Proposed,
    /// Approved, and its statements are in the target vocabulary.
    Applied,
    /// Refused. The statements stay staged; nothing reached the vocabulary.
    Rejected,
}

impl CandidateState {
    /// The token written to the store. Stable on-disk vocabulary, as [`CandidateSource::as_str`].
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Applied => "applied",
            Self::Rejected => "rejected",
        }
    }

    /// Read a token back. `None` for anything this build does not know.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "proposed" => Some(Self::Proposed),
            "applied" => Some(Self::Applied),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

impl std::fmt::Display for CandidateState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a reviewer is told about where a proposed change came from.
///
/// Every field is required except the confidence, and that asymmetry is the point: a proposal with
/// no account of who raised it and why is one a reviewer cannot judge, whereas a confidence is only
/// meaningful for a producer that computes one. A file import has no confidence; a discovery match
/// and an agent do.
#[derive(Debug, Clone, PartialEq)]
pub struct Provenance {
    /// What kind of producer raised it.
    pub source: CandidateSource,
    /// Who or what raised it, named the way an auditor would want it named.
    pub agent: String,
    /// One line on why this change is being proposed, for someone who does not have the diff.
    pub note: String,
    /// How sure the producer is, between 0 and 1, where it computes such a thing.
    pub confidence: Option<f64>,
}

impl Provenance {
    /// Refuse provenance a reviewer could not act on.
    fn validate(&self) -> Result<(), StoreError> {
        let refuse = |detail: String| Err(StoreError::CandidateProvenance { detail });

        if self.agent.trim().is_empty() {
            return refuse(
                "it does not say who or what raised the change, and an unattributed proposal is \
                 one no reviewer can weigh"
                    .to_owned(),
            );
        }
        if self.note.trim().is_empty() {
            return refuse(
                "it does not say why the change is being proposed, and a reviewer who has to \
                 infer that from the statements is doing the producer's job"
                    .to_owned(),
            );
        }
        match self.confidence {
            None => Ok(()),
            Some(value) if (0.0..=1.0).contains(&value) => Ok(()),
            Some(value) => refuse(format!(
                "its confidence is {value}, and a confidence is a number between 0 and 1; a scale \
                 nobody has stated is worse than no confidence at all"
            )),
        }
    }
}

/// A proposed change to one vocabulary, as recorded in the store.
///
/// Read-only: a candidate is produced by [`Store::propose_import`] and changed only by
/// [`Store::decide`], both of which write the record inside a transaction. Handing out a mutable
/// struct would make it possible to hold a candidate that disagrees with the store.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    id: CandidateId,
    target: GraphId,
    payload: Option<GraphId>,
    removal_payload: Option<GraphId>,
    provenance: Provenance,
    proposed_at: RecordedAt,
    additions: u64,
    removals: u64,
    state: CandidateState,
    decided_by: Option<String>,
    decided_at: Option<RecordedAt>,
}

impl Candidate {
    /// Its identifier.
    pub fn id(&self) -> CandidateId {
        self.id
    }

    /// The vocabulary graph it proposes to change.
    pub fn target(&self) -> &GraphId {
        &self.target
    }

    /// The graph its proposed *additions* are staged in, if it adds anything.
    ///
    /// This is what a reviewer exports or queries to see what would arrive. It stays after a
    /// decision, applied or rejected, so "what exactly was approved" remains answerable.
    pub fn payload(&self) -> Option<&GraphId> {
        self.payload.as_ref()
    }

    /// The graph its proposed *removals* are staged in, if it removes anything.
    ///
    /// Kept after a decision for the same reason the additions are: an approved removal is the
    /// one change whose evidence a vocabulary no longer holds, so if the staging graph went with
    /// it there would be nothing left anywhere saying what was taken away.
    pub fn removal_payload(&self) -> Option<&GraphId> {
        self.removal_payload.as_ref()
    }

    /// Where it came from and why.
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// When it was raised, as an `xsd:dateTime` lexical form.
    pub fn proposed_at(&self) -> &str {
        self.proposed_at.as_str()
    }

    /// How many statements it proposes to add.
    pub fn additions(&self) -> u64 {
        self.additions
    }

    /// How many statements it proposes to remove.
    pub fn removals(&self) -> u64 {
        self.removals
    }

    /// Where it is in its lifecycle.
    pub fn state(&self) -> CandidateState {
        self.state
    }

    /// Who decided it, if anyone has.
    pub fn decided_by(&self) -> Option<&str> {
        self.decided_by.as_deref()
    }

    /// When it was decided, as an `xsd:dateTime` lexical form, if it has been.
    pub fn decided_at(&self) -> Option<&str> {
        self.decided_at.as_ref().map(RecordedAt::as_str)
    }
}

/// What a reviewer decided about a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Decision {
    /// Apply the staged statements to the target vocabulary.
    Approve,
    /// Do not. The statements stay staged and the vocabulary is untouched.
    Reject,
}

impl Decision {
    /// The state a candidate is left in.
    const fn outcome(self) -> CandidateState {
        match self {
            Self::Approve => CandidateState::Applied,
            Self::Reject => CandidateState::Rejected,
        }
    }
}

impl Store {
    /// Propose the statements in `reader` as an addition to the vocabulary `target`.
    ///
    /// Nothing reaches `target`. The statements are parsed, staged in the candidate's own graph,
    /// and recorded with their provenance; a human then approves or rejects with [`Store::decide`].
    /// This is the seam `CLAUDE.md` §3 requires, and an import is its first producer.
    ///
    /// # What it refuses, and why each refusal is not a nuisance
    ///
    /// - **A target that is not a registered vocabulary.** Importing into a graph that does not
    ///   exist would create a vocabulary as a side effect, and creating one is the path `CLAUDE.md`
    ///   §1.7 requires to run through discovery with a recorded justification. An import is not a
    ///   way around that.
    /// - **Statements naming a graph other than the target.** A quad syntax can carry graph names,
    ///   and a proposal goes to *one* vocabulary. Silently dropping the names would land somebody's
    ///   multi-graph file in one place and tell them it worked; silently honouring them would let
    ///   one import write to vocabularies the operator never named. Statements in the default graph
    ///   and statements naming the target itself are both accepted, so an export of a vocabulary
    ///   re-imports into it in any of the six syntaxes.
    /// - **A file with no statements.** Proposing nothing is almost always a file in a different
    ///   syntax from the one that was named, and a candidate with an empty payload is a decision
    ///   nobody can take.
    ///
    /// # Base IRI and blank nodes
    ///
    /// Relative IRIs resolve against the target graph's IRI. Blank node labels are renamed as they
    /// are read, so two imports that both use `_:b1` do not silently merge into one node.
    ///
    /// # Cost
    ///
    /// One transaction, so the backend holds the whole import in memory until it commits — the
    /// same ceiling `Store::restore` documents, for the same reason: a half-staged candidate is a
    /// proposal whose diff is a lie.
    pub fn propose_import(
        &self,
        target: &GraphId,
        syntax: RdfSyntax,
        reader: impl Read,
        provenance: &Provenance,
    ) -> Result<Candidate, StoreError> {
        self.propose(target, CandidatePart::Additions, syntax, reader, provenance)
    }

    /// Propose the statements in `reader` as a removal from the vocabulary `target`.
    ///
    /// The mirror of [`Store::propose_import`] and the half of the seam that makes a merge, a
    /// split, a move, and a deprecation expressible: each of those is "these statements go".
    /// Nothing reaches `target` here either — the statements are staged in the candidate's removal
    /// graph, where a reviewer can export them, and [`Store::decide`] is what applies them.
    ///
    /// It refuses everything [`Store::propose_import`] refuses, and one thing more:
    ///
    /// - **A statement the vocabulary does not hold.** A removal that matches nothing is not a
    ///   small waste; it is a proposal whose reviewed effect and actual effect differ, and the
    ///   difference is invisible in the diff. The likeliest cause is a producer working from a
    ///   stale copy of the vocabulary, and the second likeliest is a file in the right syntax
    ///   describing the wrong thing. The refusal names how many were missing and shows one.
    ///
    /// # Blank nodes are *not* renamed here, and that is the difference from an import
    ///
    /// An import renames blank node labels so two files using `_:b1` do not merge into one node.
    /// A removal has to name statements that already exist, so renaming would guarantee that none
    /// of them matched. They are therefore taken as written — which means a blank node matches
    /// only if it is spelled the way the store spells it, and anything else is refused by the
    /// presence check above rather than silently removing something adjacent.
    pub fn propose_retraction(
        &self,
        target: &GraphId,
        syntax: RdfSyntax,
        reader: impl Read,
        provenance: &Provenance,
    ) -> Result<Candidate, StoreError> {
        self.propose(target, CandidatePart::Removals, syntax, reader, provenance)
    }

    /// Stage one half of a proposed change and record it.
    ///
    /// One body for both halves, so a rule stated for an import — the target must be a registered
    /// vocabulary, the file may not name another graph, an empty file is refused — cannot quietly
    /// fail to hold for a retraction.
    fn propose(
        &self,
        target: &GraphId,
        part: CandidatePart,
        syntax: RdfSyntax,
        reader: impl Read,
        provenance: &Provenance,
    ) -> Result<Candidate, StoreError> {
        provenance.validate()?;

        if target.kind() != GraphKind::Vocabulary {
            return Err(StoreError::CandidateTargetNotVocabulary {
                iri: target.iri().to_owned(),
                kind: target.kind(),
            });
        }

        let parser = RdfParser::from_format(syntax.backend())
            .with_base_iri(target.iri())
            .map_err(|error| {
                // Unreachable in practice: `GraphId` validated this IRI through the same parser.
                StoreError::Backend(format!(
                    "the target graph's IRI is not usable as a base IRI: {error}"
                ))
            })?;
        let parser = match part {
            CandidatePart::Additions => parser.rename_blank_nodes(),
            CandidatePart::Removals => parser,
        };
        let parser = parser.for_reader(reader);

        self.transaction(|txn| {
            if !txn.contains_graph(target.iri())? {
                return Err(StoreError::NoSuchGraph {
                    iri: target.iri().to_owned(),
                });
            }

            let id = next_candidate_id(txn)?;
            let payload = GraphId::candidate(&id, part);
            let payload_name: GraphName = NamedNode::new_unchecked(payload.iri()).into();
            let target_name: GraphName = NamedNode::new_unchecked(target.iri()).into();

            let mut batch: Vec<Quad> = Vec::with_capacity(STAGE_BATCH);
            let mut staged: u64 = 0;
            let mut absent: u64 = 0;
            let mut example: Option<String> = None;

            for quad in parser {
                let quad = quad.map_err(|error| import_failure(syntax, error))?;

                match &quad.graph_name {
                    GraphName::DefaultGraph => {}
                    GraphName::NamedNode(node) if node.as_str() == target.iri() => {}
                    other => {
                        return Err(StoreError::ImportGraphMismatch {
                            found: other.to_string(),
                            target: target.iri().to_owned(),
                        })
                    }
                }

                if part == CandidatePart::Removals {
                    let in_target = Quad::new(
                        quad.subject.clone(),
                        quad.predicate.clone(),
                        quad.object.clone(),
                        target_name.clone(),
                    );
                    if !txn.contains_quad(&in_target)? {
                        absent += 1;
                        example.get_or_insert_with(|| in_target.to_string());
                        continue;
                    }
                }

                batch.push(Quad::new(
                    quad.subject,
                    quad.predicate,
                    quad.object,
                    payload_name.clone(),
                ));
                staged += 1;

                if batch.len() == STAGE_BATCH {
                    txn.extend_graph(&payload, &batch)?;
                    batch.clear();
                }
            }
            txn.extend_graph(&payload, &batch)?;

            if absent > 0 {
                return Err(StoreError::RetractionNotPresent {
                    target: target.iri().to_owned(),
                    absent,
                    // Always `Some` when `absent` is non-zero; the default keeps the refusal on
                    // its feet rather than making the message's shape an invariant to uphold.
                    example: example.unwrap_or_default(),
                });
            }
            if staged == 0 {
                return Err(StoreError::ImportEmpty { syntax });
            }

            txn.register(&payload)?;

            let (additions, removals) = match part {
                CandidatePart::Additions => (staged, 0),
                CandidatePart::Removals => (0, staged),
            };
            let (payload, removal_payload) = match part {
                CandidatePart::Additions => (Some(payload), None),
                CandidatePart::Removals => (None, Some(payload)),
            };

            let candidate = Candidate {
                id,
                target: target.clone(),
                payload,
                removal_payload,
                provenance: provenance.clone(),
                proposed_at: RecordedAt::now(),
                additions,
                removals,
                state: CandidateState::Proposed,
                decided_by: None,
                decided_at: None,
            };
            write_record(txn, &candidate)?;

            Ok(candidate)
        })
    }

    /// Propose a change that both adds and removes, computed rather than read from a file.
    ///
    /// The half of the seam a *bulk operation* needs. `openbiz import` and `openbiz retract` each
    /// carry one half because a file is one half; a move, a merge, a split, and a deprecation are
    /// each "these statements go, those arrive" — **one decision a reviewer takes once**, not two
    /// candidates that must be approved in the right order and that leave the vocabulary in a
    /// state nobody proposed if only the first one lands.
    ///
    /// The statements arrive already computed, in [`StatementRef`], because the producer is an
    /// operation over the vocabulary's own model rather than a stream of bytes. Nothing is parsed
    /// here and no base IRI applies: the caller has already decided exactly which statements it
    /// means.
    ///
    /// # What it refuses
    ///
    /// - **A target that is not a registered vocabulary**, for the reason
    ///   [`Store::propose_import`] gives: a proposal is not a way to create a vocabulary behind
    ///   `CLAUDE.md` §1.7's back.
    /// - **A removal the vocabulary does not hold**, for the reason [`Store::propose_retraction`]
    ///   gives, and by the same check — a proposal whose reviewed effect and actual effect differ
    ///   is worse than no proposal.
    /// - **A change that neither adds nor removes anything.** An operation that computed no
    ///   statements decided nothing, and [`read_record`] treats such a record as corrupt, so it
    ///   must not be writable in the first place.
    /// - **A literal in subject position**, which RDF does not have. Unreachable through this
    ///   crate's own reads, and refused rather than mapped to something adjacent because a caller
    ///   that computed one has a bug the store should not smooth over.
    ///
    /// Blank nodes are taken exactly as given, in **both** halves — the removals for the reason
    /// [`Store::propose_retraction`] gives, and the additions because a computed change refers to
    /// the graph it was computed from, so renaming them would break the reference an import's
    /// renaming exists to protect.
    pub fn propose_edit(
        &self,
        target: &GraphId,
        additions: &[StatementRef<'_>],
        removals: &[StatementRef<'_>],
        provenance: &Provenance,
    ) -> Result<Candidate, StoreError> {
        provenance.validate()?;

        if target.kind() != GraphKind::Vocabulary {
            return Err(StoreError::CandidateTargetNotVocabulary {
                iri: target.iri().to_owned(),
                kind: target.kind(),
            });
        }
        if additions.is_empty() && removals.is_empty() {
            return Err(StoreError::CandidateEmpty);
        }

        self.transaction(|txn| {
            if !txn.contains_graph(target.iri())? {
                return Err(StoreError::NoSuchGraph {
                    iri: target.iri().to_owned(),
                });
            }

            let id = next_candidate_id(txn)?;
            let target_name: GraphName = NamedNode::new_unchecked(target.iri()).into();

            // Removals first, so a stale one is refused before any addition has been staged. The
            // transaction would roll the staging back either way; doing it in this order means the
            // *reason* the caller gets back is the interesting one rather than whichever check
            // happened to run first.
            let mut absent: u64 = 0;
            let mut example: Option<String> = None;
            let removal_graph = GraphId::candidate(&id, CandidatePart::Removals);
            // The counts are of *distinct* statements, because a computed change is a set: an
            // operation that named the same statement twice would otherwise report a number one
            // greater than the diff a reviewer reads. The file paths cannot do this cheaply and
            // say so in `docs/UNTESTED.md`; here the whole change is already in memory, so the
            // seen-set costs nothing and the order the producer chose is kept for the diff.
            let mut seen: HashSet<Quad> = HashSet::new();
            let mut distinct_removals: Vec<Quad> = Vec::with_capacity(removals.len());
            for statement in removals {
                let quad = quad_of(statement, &target_name)?;
                if !txn.contains_quad(&quad)? {
                    absent += 1;
                    example.get_or_insert_with(|| quad.to_string());
                    continue;
                }
                let staged = restage(&quad, &removal_graph);
                if seen.insert(staged.clone()) {
                    distinct_removals.push(staged);
                }
            }
            if absent > 0 {
                return Err(StoreError::RetractionNotPresent {
                    target: target.iri().to_owned(),
                    absent,
                    example: example.unwrap_or_default(),
                });
            }

            let addition_graph = GraphId::candidate(&id, CandidatePart::Additions);
            let mut seen: HashSet<Quad> = HashSet::new();
            let mut distinct_additions: Vec<Quad> = Vec::with_capacity(additions.len());
            for statement in additions {
                let quad = quad_of(statement, &target_name)?;
                let staged = restage(&quad, &addition_graph);
                if seen.insert(staged.clone()) {
                    distinct_additions.push(staged);
                }
            }

            let additions_count = distinct_additions.len() as u64;
            let removals_count = distinct_removals.len() as u64;

            // A half with no statements gets no graph and no count, because `read_record` holds
            // "names a graph" and "has a non-zero count" to be one fact written twice.
            let payload = if distinct_additions.is_empty() {
                None
            } else {
                for batch in distinct_additions.chunks(STAGE_BATCH) {
                    txn.extend_graph(&addition_graph, batch)?;
                }
                txn.register(&addition_graph)?;
                Some(addition_graph)
            };
            let removal_payload = if distinct_removals.is_empty() {
                None
            } else {
                for batch in distinct_removals.chunks(STAGE_BATCH) {
                    txn.extend_graph(&removal_graph, batch)?;
                }
                txn.register(&removal_graph)?;
                Some(removal_graph)
            };

            let candidate = Candidate {
                id,
                target: target.clone(),
                payload,
                removal_payload,
                provenance: provenance.clone(),
                proposed_at: RecordedAt::now(),
                additions: additions_count,
                removals: removals_count,
                state: CandidateState::Proposed,
                decided_by: None,
                decided_at: None,
            };
            write_record(txn, &candidate)?;

            Ok(candidate)
        })
    }

    /// Every candidate the store holds, oldest first.
    pub fn candidates(&self) -> Result<Vec<Candidate>, StoreError> {
        let mut ids = Vec::new();
        for quad in self.backend.system_quads(None, rdf::TYPE)? {
            if quad.object == Term::from(named_node(CANDIDATE_CLASS_IRI).into_owned()) {
                ids.push(candidate_id_of(&quad.subject, self.path())?);
            }
        }
        ids.sort_unstable();

        ids.into_iter()
            .map(|id| read_record(&self.backend, id, self.path()))
            .collect()
    }

    /// One candidate, or [`StoreError::NoSuchCandidate`] if the store has never held it.
    pub fn candidate(&self, id: CandidateId) -> Result<Candidate, StoreError> {
        read_record(&self.backend, id, self.path())
    }

    /// Approve or reject a candidate, applying it if approved.
    ///
    /// Approval copies the staged statements into the target vocabulary **inside the transaction
    /// that records the decision**, so a store never holds a candidate marked applied whose
    /// statements are not there, or statements in a vocabulary with no record of who let them in.
    /// That pairing is the whole value of the seam to an auditor.
    ///
    /// The payload graphs are kept either way. Deleting the evidence of what was approved is not a
    /// default a governance product may take; what it costs is recorded in `docs/UNTESTED.md`.
    ///
    /// Refuses a candidate that has already been decided, naming the state it is in — deciding one
    /// twice would either duplicate its statements or silently do nothing, and both are worse than
    /// being told.
    ///
    /// **Also refuses a candidate whose removals have gone stale**: if the vocabulary no longer
    /// holds every statement the candidate proposes to remove, approving it would take away less
    /// than the reviewer agreed to and say it had succeeded. The refusal names how many are
    /// missing. Rejecting such a candidate is always allowed — a proposal that can no longer be
    /// applied is exactly one somebody should be able to close.
    pub fn decide(
        &self,
        id: CandidateId,
        decision: Decision,
        decided_by: &str,
    ) -> Result<Candidate, StoreError> {
        if decided_by.trim().is_empty() {
            return Err(StoreError::CandidateProvenance {
                detail: "the decision does not say who took it, and an unattributed approval is \
                         the one thing an audit trail cannot afford to be missing"
                    .to_owned(),
            });
        }

        self.transaction(|txn| {
            let mut candidate = read_record(&txn.inner, id, self.path())?;

            if candidate.state != CandidateState::Proposed {
                return Err(StoreError::CandidateDecided {
                    id: id.to_string(),
                    state: candidate.state,
                });
            }

            if decision == Decision::Approve {
                if !txn.contains_graph(candidate.target.iri())? {
                    return Err(StoreError::NoSuchGraph {
                        iri: candidate.target.iri().to_owned(),
                    });
                }
                apply_payload(txn, &candidate)?;
            }

            let decided_at = RecordedAt::now();
            txn.remove_graph_quads(
                &GraphId::system(),
                &[Quad::new(
                    candidate.id.subject(),
                    named_node(STATE_IRI).into_owned(),
                    Literal::new_simple_literal(candidate.state.as_str()),
                    NamedNode::new_unchecked(GraphId::system().iri()),
                )],
            )?;

            candidate.state = decision.outcome();
            candidate.decided_by = Some(decided_by.to_owned());
            candidate.decided_at = Some(decided_at.clone());

            let subject = candidate.id.subject();
            txn.insert(
                &GraphId::system(),
                vec![
                    (
                        subject.clone(),
                        named_node(STATE_IRI).into_owned(),
                        Literal::new_simple_literal(candidate.state.as_str()).into(),
                    ),
                    (
                        subject.clone(),
                        named_node(DECIDED_BY_IRI).into_owned(),
                        Literal::new_simple_literal(decided_by).into(),
                    ),
                    (
                        subject,
                        named_node(DECIDED_AT_IRI).into_owned(),
                        Literal::new_typed_literal(decided_at.as_str(), xsd::DATE_TIME).into(),
                    ),
                ],
            )?;

            Ok(candidate)
        })
    }
}

/// Apply a candidate's staged halves to its target vocabulary.
///
/// **Removals first, then additions.** The order is only observable for a statement staged in both
/// halves, which no producer can raise today, and it is fixed now rather than left to whichever
/// half happens to be written first: removing then adding means such a statement survives, which
/// is what "replace this with itself" has to mean. The opposite order would delete it, which is
/// the answer nobody asks for.
///
/// Each half is read out in full before any of it is written: the backend's iterator borrows the
/// transaction, and a copy that streamed would be reading a graph while writing another through
/// the same handle.
///
/// The removal half is checked against the vocabulary *as it is now* before anything is written.
/// See [`Store::decide`] for why a stale removal is refused rather than trimmed.
fn apply_payload(txn: &mut Transaction<'_>, candidate: &Candidate) -> Result<(), StoreError> {
    let target: GraphName = NamedNode::new_unchecked(candidate.target.iri()).into();

    let restage = |quads: Vec<Quad>| -> Vec<Quad> {
        quads
            .into_iter()
            .map(|quad| Quad::new(quad.subject, quad.predicate, quad.object, target.clone()))
            .collect()
    };

    if let Some(payload) = &candidate.removal_payload {
        let staged = restage(txn.graph_quads(payload)?);

        let mut missing = 0;
        for quad in &staged {
            if !txn.contains_quad(quad)? {
                missing += 1;
            }
        }
        if missing > 0 {
            return Err(StoreError::CandidateStale {
                id: candidate.id.to_string(),
                target: candidate.target.iri().to_owned(),
                missing,
                removals: staged.len() as u64,
            });
        }

        txn.remove_graph_quads(&candidate.target, &staged)?;
    }

    if let Some(payload) = &candidate.payload {
        let staged = restage(txn.graph_quads(payload)?);
        txn.extend_graph(&candidate.target, &staged)?;
    }

    Ok(())
}

/// One computed statement as a quad in `graph`.
///
/// The only thing that can go wrong is a literal in subject position, which RDF does not have and
/// which no read out of this store can produce — [`StatementRef`] carries the same term type in
/// both positions because it is the shape a *scan* hands back, where the impossibility is the
/// backend's to enforce. A caller computing statements has no such guarantee, so it is checked
/// here and refused rather than mapped to a blank node that would silently be about something
/// else.
fn quad_of(statement: &StatementRef<'_>, graph: &GraphName) -> Result<Quad, StoreError> {
    let subject: NamedOrBlankNode = match statement.subject {
        StatementTerm::Iri(iri) => NamedNode::new(iri)
            .map_err(|error| StoreError::CandidateStatementInvalid {
                detail: format!("{iri} is not a usable IRI in subject position: {error}"),
            })?
            .into(),
        StatementTerm::Blank(label) => BlankNode::new(label)
            .map_err(|error| StoreError::CandidateStatementInvalid {
                detail: format!("_:{label} is not a usable blank node label: {error}"),
            })?
            .into(),
        StatementTerm::Literal { value, .. } => {
            return Err(StoreError::CandidateStatementInvalid {
                detail: format!(
                    "{value:?} is a literal in subject position, which RDF does not have"
                ),
            })
        }
    };
    let predicate = NamedNode::new(statement.predicate).map_err(|error| {
        StoreError::CandidateStatementInvalid {
            detail: format!(
                "{} is not a usable IRI in predicate position: {error}",
                statement.predicate
            ),
        }
    })?;
    let object: Term = match statement.object {
        StatementTerm::Iri(iri) => NamedNode::new(iri)
            .map_err(|error| StoreError::CandidateStatementInvalid {
                detail: format!("{iri} is not a usable IRI in object position: {error}"),
            })?
            .into(),
        StatementTerm::Blank(label) => BlankNode::new(label)
            .map_err(|error| StoreError::CandidateStatementInvalid {
                detail: format!("_:{label} is not a usable blank node label: {error}"),
            })?
            .into(),
        StatementTerm::Literal {
            value,
            language: Some(language),
            ..
        } => Literal::new_language_tagged_literal(value, language)
            .map_err(|error| StoreError::CandidateStatementInvalid {
                detail: format!("{language:?} is not a usable language tag: {error}"),
            })?
            .into(),
        StatementTerm::Literal {
            value,
            language: None,
            datatype,
        } => Literal::new_typed_literal(
            value,
            NamedNode::new(datatype).map_err(|error| StoreError::CandidateStatementInvalid {
                detail: format!("{datatype} is not a usable datatype IRI: {error}"),
            })?,
        )
        .into(),
    };
    Ok(Quad::new(subject, predicate, object, graph.clone()))
}

/// The same statement, in a staging graph instead of the vocabulary.
///
/// A removal is checked against the vocabulary and then staged, so it exists twice with two graph
/// names; building it once and moving it means the thing checked and the thing staged cannot drift
/// apart.
fn restage(quad: &Quad, graph: &GraphId) -> Quad {
    Quad::new(
        quad.subject.clone(),
        quad.predicate.clone(),
        quad.object.clone(),
        NamedNode::new_unchecked(graph.iri()),
    )
}

/// The next identifier to mint, one past the highest the store holds.
///
/// Runs inside the caller's transaction, which holds the store's write lock, so no second proposal
/// can mint the same number between this read and the write that uses it.
fn next_candidate_id(txn: &mut Transaction<'_>) -> Result<CandidateId, StoreError> {
    let mut highest = 0;
    for quad in txn.inner.system_quads(None, rdf::TYPE)? {
        if quad.object != Term::from(named_node(CANDIDATE_CLASS_IRI).into_owned()) {
            continue;
        }
        if let Term::NamedNode(subject) = Term::from(quad.subject) {
            if let Some(id) = subject
                .as_str()
                .strip_prefix(CANDIDATE_SUBJECT_PREFIX)
                .and_then(|text| CandidateId::parse(text).ok())
            {
                highest = highest.max(id.0);
            }
        }
    }
    Ok(CandidateId(highest + 1))
}

/// Read the identifier out of a candidate record's subject.
fn candidate_id_of(
    subject: &oxigraph::model::NamedOrBlankNode,
    path: &std::path::Path,
) -> Result<CandidateId, StoreError> {
    let corrupt = |detail: String| StoreError::Corrupt {
        path: path.to_path_buf(),
        detail,
    };

    let oxigraph::model::NamedOrBlankNode::NamedNode(node) = subject else {
        return Err(corrupt(format!(
            "a candidate record is identified by {subject}, which is not an IRI"
        )));
    };
    let Some(text) = node.as_str().strip_prefix(CANDIDATE_SUBJECT_PREFIX) else {
        return Err(corrupt(format!(
            "a candidate record is identified by {node}, which is not under \
             {CANDIDATE_SUBJECT_PREFIX}"
        )));
    };
    CandidateId::parse(text).map_err(|error| corrupt(error.to_string()))
}

/// Write a freshly raised candidate's record into the system graph.
fn write_record(txn: &mut Transaction<'_>, candidate: &Candidate) -> Result<(), StoreError> {
    let subject = candidate.id.subject();
    let literal = |value: &str| Term::from(Literal::new_simple_literal(value));

    let mut triples = vec![
        (
            subject.clone(),
            rdf::TYPE.into_owned(),
            named_node(CANDIDATE_CLASS_IRI).into_owned().into(),
        ),
        (
            subject.clone(),
            named_node(TARGET_IRI).into_owned(),
            NamedNode::new_unchecked(candidate.target.iri()).into(),
        ),
        (
            subject.clone(),
            named_node(SOURCE_IRI).into_owned(),
            literal(candidate.provenance.source.as_str()),
        ),
        (
            subject.clone(),
            named_node(AGENT_IRI).into_owned(),
            literal(&candidate.provenance.agent),
        ),
        (
            subject.clone(),
            named_node(NOTE_IRI).into_owned(),
            literal(&candidate.provenance.note),
        ),
        (
            subject.clone(),
            named_node(PROPOSED_AT_IRI).into_owned(),
            Literal::new_typed_literal(candidate.proposed_at.as_str(), xsd::DATE_TIME).into(),
        ),
        (
            subject.clone(),
            named_node(ADDITIONS_IRI).into_owned(),
            Literal::new_typed_literal(candidate.additions.to_string(), xsd::INTEGER).into(),
        ),
        (
            subject.clone(),
            named_node(REMOVALS_IRI).into_owned(),
            Literal::new_typed_literal(candidate.removals.to_string(), xsd::INTEGER).into(),
        ),
        (
            subject.clone(),
            named_node(STATE_IRI).into_owned(),
            literal(candidate.state.as_str()),
        ),
    ];

    // A half with nothing in it names no graph, because there is no graph: an empty staging graph
    // is indistinguishable from one whose statements were lost, and a registry entry for a graph
    // holding nothing is a thing an operator has to explain to themselves.
    for (predicate, graph) in [
        (PAYLOAD_IRI, candidate.payload.as_ref()),
        (REMOVAL_PAYLOAD_IRI, candidate.removal_payload.as_ref()),
    ] {
        if let Some(graph) = graph {
            triples.push((
                subject.clone(),
                named_node(predicate).into_owned(),
                NamedNode::new_unchecked(graph.iri()).into(),
            ));
        }
    }

    if let Some(confidence) = candidate.provenance.confidence {
        triples.push((
            subject,
            named_node(CONFIDENCE_IRI).into_owned(),
            Literal::new_typed_literal(confidence.to_string(), xsd::DOUBLE).into(),
        ));
    }

    txn.insert(&GraphId::system(), triples)
}

/// Read one candidate's record back, through the same code whichever side of a transaction asks.
///
/// Every field is re-validated rather than trusted. The record is data on disk, so a store that
/// has been hand-edited, restored from a doctored backup, or written by a build with a bug must be
/// refused rather than turned into a `Candidate` the rest of the product then acts on.
fn read_record(
    source: &impl RegistryReader,
    id: CandidateId,
    path: &std::path::Path,
) -> Result<Candidate, StoreError> {
    let corrupt = |detail: String| StoreError::Corrupt {
        path: path.to_path_buf(),
        detail,
    };

    let subject = id.subject();
    let mut held: BTreeMap<String, Vec<Term>> = BTreeMap::new();
    for quad in source.system_subject_quads(subject.as_ref())? {
        held.entry(quad.predicate.into_string())
            .or_default()
            .push(quad.object);
    }

    if held.is_empty() {
        return Err(StoreError::NoSuchCandidate { id: id.to_string() });
    }

    let one = |predicate: &str| -> Result<Option<Term>, StoreError> {
        match held.get(predicate).map(Vec::as_slice) {
            None | Some([]) => Ok(None),
            Some([term]) => Ok(Some(term.clone())),
            Some(many) => Err(corrupt(format!(
                "candidate {id} has {} values for <{predicate}>, and it may have at most one",
                many.len()
            ))),
        }
    };

    let text = |predicate: &str| -> Result<String, StoreError> {
        match one(predicate)? {
            Some(Term::Literal(literal)) => Ok(literal.value().to_owned()),
            Some(other) => Err(corrupt(format!(
                "candidate {id} has {other} for <{predicate}>, which is not a literal"
            ))),
            None => Err(corrupt(format!(
                "candidate {id} has no <{predicate}>, which every candidate record carries"
            ))),
        }
    };

    let iri = |predicate: &str| -> Result<String, StoreError> {
        match one(predicate)? {
            Some(Term::NamedNode(node)) => Ok(node.into_string()),
            Some(other) => Err(corrupt(format!(
                "candidate {id} has {other} for <{predicate}>, which is not an IRI"
            ))),
            None => Err(corrupt(format!(
                "candidate {id} has no <{predicate}>, which every candidate record carries"
            ))),
        }
    };

    let target = GraphId::classify(&iri(TARGET_IRI)?).map_err(|error| {
        corrupt(format!(
            "candidate {id} names a target we cannot describe: {error}"
        ))
    })?;
    // Either half may be absent — a candidate that only adds has no removal graph, and one that
    // only removes has no addition graph — but a half whose graph is named must name *its own*
    // graph. The IRI is derived from the identifier rather than chosen, so a record pointing
    // anywhere else came from a doctored store and is refused rather than followed.
    let optional_payload =
        |predicate: &str, part: CandidatePart| -> Result<Option<GraphId>, StoreError> {
            let Some(term) = one(predicate)? else {
                return Ok(None);
            };
            let Term::NamedNode(node) = term else {
                return Err(corrupt(format!(
                    "candidate {id} has {term} for <{predicate}>, which is not an IRI"
                )));
            };
            let named = node.into_string();
            let payload = GraphId::classify(&named).map_err(|error| {
                corrupt(format!(
                    "candidate {id} names a payload we cannot describe: {error}"
                ))
            })?;
            if payload != GraphId::candidate(&id, part) {
                return Err(corrupt(format!(
                "candidate {id} names {payload} as its {part} payload, and a candidate's payload \
                 graphs are derived from its identifier rather than chosen"
            )));
            }
            Ok(Some(payload))
        };

    let payload = optional_payload(PAYLOAD_IRI, CandidatePart::Additions)?;
    let removal_payload = optional_payload(REMOVAL_PAYLOAD_IRI, CandidatePart::Removals)?;

    let source_token = text(SOURCE_IRI)?;
    let Some(candidate_source) = CandidateSource::parse(&source_token) else {
        return Err(corrupt(format!(
            "candidate {id} came from {source_token:?}, which this build does not recognise"
        )));
    };

    let state_token = text(STATE_IRI)?;
    let Some(state) = CandidateState::parse(&state_token) else {
        return Err(corrupt(format!(
            "candidate {id} is in state {state_token:?}, which this build does not recognise"
        )));
    };

    let additions = text(ADDITIONS_IRI)?
        .parse::<u64>()
        .map_err(|_| corrupt(format!("candidate {id} has a non-numeric addition count")))?;

    // Absent means zero rather than broken: every candidate a build before format version 4 wrote
    // was additions-only, and those records are still in stores this build opens.
    let removals = match one(REMOVALS_IRI)? {
        None => 0,
        Some(_) => text(REMOVALS_IRI)?
            .parse::<u64>()
            .map_err(|_| corrupt(format!("candidate {id} has a non-numeric removal count")))?,
    };

    // The counts and the graphs are one fact written twice, so they are checked against each
    // other. A record claiming removals with no graph to hold them would present a reviewer with
    // a change whose statements nobody can read, which is the one thing the seam exists to stop.
    for (count, graph, what) in [
        (additions, payload.as_ref(), "add"),
        (removals, removal_payload.as_ref(), "remove"),
    ] {
        if (count == 0) != graph.is_none() {
            return Err(corrupt(format!(
                "candidate {id} says it would {what} {count} statements and {} a graph staging \
                 them",
                if graph.is_none() { "names no" } else { "names" }
            )));
        }
    }
    if additions == 0 && removals == 0 {
        return Err(corrupt(format!(
            "candidate {id} would neither add nor remove anything, and a proposal to do nothing \
             is not a decision anyone can take"
        )));
    }

    let confidence = match one(CONFIDENCE_IRI)? {
        None => None,
        Some(Term::Literal(literal)) => Some(
            literal
                .value()
                .parse::<f64>()
                .map_err(|_| corrupt(format!("candidate {id} has a non-numeric confidence")))?,
        ),
        Some(other) => {
            return Err(corrupt(format!(
                "candidate {id} has {other} for its confidence, which is not a literal"
            )))
        }
    };

    let decided_by = match one(DECIDED_BY_IRI)? {
        None => None,
        Some(_) => Some(text(DECIDED_BY_IRI)?),
    };
    // The two stamps are re-validated like every other field, and for the same reason: a record
    // is data on disk. A candidate whose `proposed_at` reads "yesterday", or names no timezone so
    // that no reader can place it against any other record, is refused here rather than shown to
    // a reviewer as though it were evidence.
    let stamp = |what: &str, lexical: String| -> Result<RecordedAt, StoreError> {
        RecordedAt::parse(&lexical).map_err(|error| {
            corrupt(format!(
                "candidate {id} records when it was {what} in a form this build cannot act \
                 on: {error}"
            ))
        })
    };
    let proposed_at = stamp("raised", text(PROPOSED_AT_IRI)?)?;
    let decided_at = match one(DECIDED_AT_IRI)? {
        None => None,
        Some(_) => Some(stamp("decided", text(DECIDED_AT_IRI)?)?),
    };

    // The pairing is an invariant, not a convention: a decided candidate that cannot say who
    // decided it is exactly the record an audit is for, so it is refused rather than shown.
    if (state == CandidateState::Proposed) != decided_by.is_none() {
        return Err(corrupt(format!(
            "candidate {id} is {state} and {} say who decided it",
            if decided_by.is_none() {
                "does not"
            } else {
                "does"
            }
        )));
    }

    Ok(Candidate {
        id,
        target,
        payload,
        removal_payload,
        provenance: Provenance {
            source: candidate_source,
            agent: text(AGENT_IRI)?,
            note: text(NOTE_IRI)?,
            confidence,
        },
        proposed_at,
        additions,
        removals,
        state,
        decided_by,
        decided_at,
    })
}

/// Turn a parse failure into something an operator can act on, naming the syntax they chose.
fn import_failure(syntax: RdfSyntax, error: oxigraph::io::RdfParseError) -> StoreError {
    match error {
        oxigraph::io::RdfParseError::Io(source) => StoreError::ImportRead { source },
        oxigraph::io::RdfParseError::Syntax(error) => StoreError::ImportSyntax {
            syntax,
            // The parser counts lines from zero and every editor counts from one. Reporting its
            // number verbatim sends an operator to the line above the broken one.
            line: error.location().map(|at| at.start.line + 1),
            detail: error.to_string(),
        },
    }
}

impl Transaction<'_> {
    /// Every quad in `graph`, read inside this transaction.
    pub(crate) fn graph_quads(&self, graph: &GraphId) -> Result<Vec<Quad>, StoreError> {
        let name: GraphName = NamedNode::new_unchecked(graph.iri()).into();
        self.inner
            .quads_for_pattern(None, None, None, Some((&name).into()))
            .map(|quad| quad.map_err(|error| StoreError::Backend(error.to_string())))
            .collect()
    }

    /// Whether the store holds this exact quad, read inside this transaction.
    ///
    /// Exact: same subject, predicate, object *and* graph. A removal is only safe to apply if the
    /// statement it names is there, and "there" has to mean there in the vocabulary being changed
    /// rather than somewhere in the store.
    pub(crate) fn contains_quad(&self, quad: &Quad) -> Result<bool, StoreError> {
        self.inner
            .contains(quad.as_ref())
            .map_err(|error| StoreError::Backend(error.to_string()))
    }

    /// Remove quads from one graph. The mirror of [`Transaction::extend_graph`], and the only way
    /// anything leaves the store.
    ///
    /// It enforces the same rules for the same reasons: the graph must be directly writable,
    /// nothing is removed from the default graph, and every quad must already name the graph it is
    /// being removed from — a caller handing over a quad naming somewhere else would delete
    /// statements from a graph it did not think it was touching.
    ///
    /// **A vocabulary's statements only ever reach this through an approved candidate.** There is
    /// no direct-delete path above it, and there is not going to be one: `CLAUDE.md` §3 says a
    /// change to a vocabulary arrives as a proposal a human approves, and a removal is the change
    /// where that matters most, because what it destroys is not recoverable from the vocabulary
    /// afterwards.
    pub(crate) fn remove_graph_quads(
        &mut self,
        graph: &GraphId,
        quads: &[Quad],
    ) -> Result<(), StoreError> {
        if !graph.is_directly_writable() {
            return Err(StoreError::NotWritable(graph.iri().to_owned()));
        }

        for quad in quads {
            let GraphName::NamedNode(name) = &quad.graph_name else {
                return Err(StoreError::Backend(
                    "a retraction named no graph, and every quad in an OpenBiz store is in one"
                        .to_owned(),
                ));
            };
            if name.as_str() != graph.iri() {
                return Err(StoreError::NotWritable(name.as_str().to_owned()));
            }
        }

        for quad in quads {
            self.inner.remove(quad.as_ref());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GraphKind, CANDIDATE_GRAPH_PREFIX, CANDIDATE_REMOVALS_SUFFIX};

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("a temporary directory")
    }

    /// The graph a candidate's additions are staged in, for a test that knows it has some.
    fn additions_graph(candidate: &Candidate) -> &GraphId {
        candidate.payload().expect("an additions payload")
    }

    /// The graph a candidate's removals are staged in, for a test that knows it has some.
    fn removals_graph(candidate: &Candidate) -> &GraphId {
        candidate.removal_payload().expect("a removals payload")
    }

    fn vocabulary(iri: &str) -> GraphId {
        GraphId::vocabulary(iri).expect("a valid absolute IRI outside the reserved namespace")
    }

    /// A store holding one vocabulary with a handful of ordinary SKOS statements in it.
    fn store_with_vocabulary(dir: &tempfile::TempDir) -> (Store, GraphId) {
        let store = Store::open(dir.path()).expect("open a store");
        let graph = vocabulary("https://example.org/animals");
        store
            .create_vocabulary_graph(&graph)
            .expect("create the vocabulary");
        (store, graph)
    }

    const CAT_TURTLE: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        <https://example.org/animals/cat> a skos:Concept ;
            skos:prefLabel "Cat"@en, "Chat"@fr ;
            skos:broader <https://example.org/animals/mammal> .
    "#;

    fn import_provenance() -> Provenance {
        Provenance {
            source: CandidateSource::Import,
            agent: "openbiz import".to_owned(),
            note: "imported from animals.ttl".to_owned(),
            confidence: None,
        }
    }

    /// Every statement in `graph`, as sortable text, so two graphs can be compared.
    fn statements(store: &Store, graph: &GraphId) -> Vec<String> {
        let name: GraphName = NamedNode::new_unchecked(graph.iri()).into();
        let mut found: Vec<String> = store
            .backend
            .quads_for_pattern(None, None, None, Some((&name).into()))
            .map(|quad| {
                let quad = quad.expect("readable");
                format!("{} {} {}", quad.subject, quad.predicate, quad.object)
            })
            .collect();
        found.sort();
        found
    }

    // ----------------------------------------------------------------------------------------
    // Removals: the half of the seam a merge, a split, a move, and a deprecation all need.
    // ----------------------------------------------------------------------------------------

    /// A store whose vocabulary already holds the cat, so there is something to propose removing.
    fn store_with_content(dir: &tempfile::TempDir) -> (Store, GraphId) {
        let (store, graph) = store_with_vocabulary(dir);
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("proposed");
        store
            .decide(candidate.id(), Decision::Approve, "a reviewer")
            .expect("approved");
        (store, graph)
    }

    fn retraction_provenance() -> Provenance {
        Provenance {
            source: CandidateSource::Import,
            agent: "openbiz retract".to_owned(),
            note: "the French label was added to the wrong concept".to_owned(),
            confidence: None,
        }
    }

    const FRENCH_LABEL: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        <https://example.org/animals/cat> skos:prefLabel "Chat"@fr .
    "#;

    /// The mirror of the import test, and the claim the whole half rests on: proposing a removal
    /// removes nothing.
    #[test]
    fn a_retraction_proposes_rather_than_removes() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);
        let before = statements(&store, &graph);

        let candidate = store
            .propose_retraction(
                &graph,
                RdfSyntax::Turtle,
                FRENCH_LABEL.as_bytes(),
                &retraction_provenance(),
            )
            .expect("proposed");

        assert_eq!(
            statements(&store, &graph),
            before,
            "proposing a removal must not remove anything"
        );
        assert_eq!(candidate.removals(), 1);
        assert_eq!(candidate.additions(), 0);
        assert!(
            candidate.payload().is_none(),
            "a removal-only candidate has no additions graph to register"
        );
        assert_eq!(
            statements(&store, removals_graph(&candidate)).len(),
            1,
            "the statement to remove is staged where a reviewer can read it"
        );
        assert_eq!(candidate.state(), CandidateState::Proposed);
    }

    /// Approval is what removes, and the evidence outlives the statement.
    #[test]
    fn approving_a_retraction_removes_the_statements_and_keeps_the_evidence() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);
        let before = statements(&store, &graph);

        let candidate = store
            .propose_retraction(
                &graph,
                RdfSyntax::Turtle,
                FRENCH_LABEL.as_bytes(),
                &retraction_provenance(),
            )
            .expect("proposed");
        let decided = store
            .decide(candidate.id(), Decision::Approve, "a reviewer")
            .expect("approved");

        let after = statements(&store, &graph);
        assert_eq!(
            after.len(),
            before.len() - 1,
            "exactly the proposed statement is gone: {before:?} -> {after:?}"
        );
        assert!(
            !after.iter().any(|line| line.contains("Chat")),
            "the French label must be gone: {after:?}"
        );
        assert!(
            after.iter().any(|line| line.contains("Cat")),
            "and nothing else may be: {after:?}"
        );

        assert_eq!(decided.state(), CandidateState::Applied);
        assert_eq!(decided.decided_by(), Some("a reviewer"));
        assert_eq!(
            statements(&store, removals_graph(&decided)).len(),
            1,
            "an approved removal is the one change the vocabulary no longer records, so the \
             staged evidence must survive it"
        );
    }

    /// A removal that matches nothing is refused at the point of proposal, with the count and an
    /// example, rather than staged as a change that would silently do less than it says.
    #[test]
    fn a_retraction_of_statements_the_vocabulary_does_not_hold_is_refused() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);
        let before = statements(&store, &graph);

        let refused = store.propose_retraction(
            &graph,
            RdfSyntax::Turtle,
            r#"
                @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
                <https://example.org/animals/cat> skos:prefLabel "Katze"@de .
                <https://example.org/animals/dog> skos:prefLabel "Hund"@de .
            "#
            .as_bytes(),
            &retraction_provenance(),
        );

        let Err(StoreError::RetractionNotPresent {
            absent, example, ..
        }) = refused
        else {
            panic!("a removal of statements that are not there must be refused: {refused:?}");
        };
        assert_eq!(absent, 2);
        assert!(
            example.contains("Katze") || example.contains("Hund"),
            "the refusal must show one of the missing statements, got {example:?}"
        );
        assert_eq!(
            statements(&store, &graph),
            before,
            "a refused proposal changes nothing"
        );
        assert_eq!(
            store.candidates().expect("listed").len(),
            1,
            "and leaves no candidate behind — only the import that put the content there"
        );
    }

    /// A file that is *partly* right is refused whole. Staging the matching half would produce a
    /// candidate whose diff is a subset of what the operator asked for and does not say so.
    #[test]
    fn a_retraction_is_refused_whole_when_only_some_of_it_matches() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        let refused = store.propose_retraction(
            &graph,
            RdfSyntax::Turtle,
            r#"
                @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
                <https://example.org/animals/cat> skos:prefLabel "Chat"@fr .
                <https://example.org/animals/cat> skos:prefLabel "Katze"@de .
            "#
            .as_bytes(),
            &retraction_provenance(),
        );

        assert!(
            matches!(
                refused,
                Err(StoreError::RetractionNotPresent { absent: 1, .. })
            ),
            "one missing statement is enough to refuse the file: {refused:?}"
        );
    }

    /// The question iteration 17 could not answer: a candidate raised on Monday against a
    /// vocabulary edited on Tuesday.
    #[test]
    fn a_retraction_that_has_gone_stale_is_refused_at_approval() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        // Two people propose removing the same statement.
        let first = store
            .propose_retraction(
                &graph,
                RdfSyntax::Turtle,
                FRENCH_LABEL.as_bytes(),
                &retraction_provenance(),
            )
            .expect("first proposed");
        let second = store
            .propose_retraction(
                &graph,
                RdfSyntax::Turtle,
                FRENCH_LABEL.as_bytes(),
                &retraction_provenance(),
            )
            .expect("second proposed");

        store
            .decide(first.id(), Decision::Approve, "a reviewer")
            .expect("the first is applied");
        let before = statements(&store, &graph);

        let refused = store.decide(second.id(), Decision::Approve, "another reviewer");
        let Err(StoreError::CandidateStale {
            missing, removals, ..
        }) = refused
        else {
            panic!("approving a stale removal must be refused: {refused:?}");
        };
        assert_eq!((missing, removals), (1, 1));
        assert_eq!(
            statements(&store, &graph),
            before,
            "a refused approval leaves the vocabulary exactly as it was"
        );
        assert_eq!(
            store
                .candidate(second.id())
                .expect("still readable")
                .state(),
            CandidateState::Proposed,
            "and leaves the candidate open rather than half-decided"
        );
    }

    /// A proposal that can no longer be applied is exactly the one somebody wants to close.
    #[test]
    fn a_stale_retraction_can_still_be_rejected() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        let first = store
            .propose_retraction(
                &graph,
                RdfSyntax::Turtle,
                FRENCH_LABEL.as_bytes(),
                &retraction_provenance(),
            )
            .expect("first proposed");
        let second = store
            .propose_retraction(
                &graph,
                RdfSyntax::Turtle,
                FRENCH_LABEL.as_bytes(),
                &retraction_provenance(),
            )
            .expect("second proposed");
        store
            .decide(first.id(), Decision::Approve, "a reviewer")
            .expect("the first is applied");

        let rejected = store
            .decide(second.id(), Decision::Reject, "another reviewer")
            .expect("a stale candidate can be closed");
        assert_eq!(rejected.state(), CandidateState::Rejected);
    }

    /// Both halves are staged in graphs whose IRIs are derived from the candidate, and both are
    /// registered as candidate graphs so they stay out of everybody's vocabulary list.
    #[test]
    fn the_two_halves_are_staged_in_two_registered_candidate_graphs() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        let added = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                r#"<https://example.org/animals/dog> a
                   <http://www.w3.org/2004/02/skos/core#Concept> ."#
                    .as_bytes(),
                &import_provenance(),
            )
            .expect("proposed");
        let removed = store
            .propose_retraction(
                &graph,
                RdfSyntax::Turtle,
                FRENCH_LABEL.as_bytes(),
                &retraction_provenance(),
            )
            .expect("proposed");

        assert_eq!(
            additions_graph(&added).iri(),
            format!("{CANDIDATE_GRAPH_PREFIX}{}", added.id())
        );
        assert_eq!(
            removals_graph(&removed).iri(),
            format!(
                "{CANDIDATE_GRAPH_PREFIX}{}{CANDIDATE_REMOVALS_SUFFIX}",
                removed.id()
            )
        );

        let registered = store.graphs().expect("the registry");
        for staged in [additions_graph(&added), removals_graph(&removed)] {
            let entry = registered
                .iter()
                .find(|entry| entry.iri() == staged.iri())
                .unwrap_or_else(|| panic!("{staged} must be registered"));
            assert_eq!(
                entry.kind(),
                GraphKind::Candidate,
                "a staging graph is never a vocabulary"
            );
        }
    }

    /// The migrated-store case: a record written before format version 4 carries no removal count,
    /// and that means "removes nothing" rather than "this record is broken".
    #[test]
    fn a_record_with_no_removal_count_reads_as_a_candidate_that_removes_nothing() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("proposed");

        // Take the record back to the shape a version-3 build would have written.
        store
            .transaction(|txn| {
                let system = GraphId::system();
                txn.remove_graph_quads(
                    &system,
                    &[Quad::new(
                        candidate.id().subject(),
                        named_node(REMOVALS_IRI).into_owned(),
                        Literal::new_typed_literal("0", xsd::INTEGER),
                        NamedNode::new_unchecked(system.iri()),
                    )],
                )
            })
            .expect("age the record");

        let read = store.candidate(candidate.id()).expect("still readable");
        assert_eq!(read.removals(), 0);
        assert!(read.removal_payload().is_none());
        assert_eq!(read, candidate, "and is otherwise the candidate we raised");
    }

    /// The counts and the graphs are the same fact written twice, so a record where they disagree
    /// is a store that has been edited underneath us and is refused rather than acted on.
    #[test]
    fn a_record_claiming_removals_with_no_graph_to_hold_them_is_refused() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("proposed");

        store
            .transaction(|txn| {
                let system = GraphId::system();
                txn.remove_graph_quads(
                    &system,
                    &[Quad::new(
                        candidate.id().subject(),
                        named_node(REMOVALS_IRI).into_owned(),
                        Literal::new_typed_literal("0", xsd::INTEGER),
                        NamedNode::new_unchecked(system.iri()),
                    )],
                )?;
                txn.insert(
                    &system,
                    vec![(
                        candidate.id().subject(),
                        named_node(REMOVALS_IRI).into_owned(),
                        Literal::new_typed_literal("3", xsd::INTEGER).into(),
                    )],
                )
            })
            .expect("forge the record");

        let read = store.candidate(candidate.id());
        assert!(
            matches!(read, Err(StoreError::Corrupt { .. })),
            "a candidate that claims removals it cannot show must be refused: {read:?}"
        );
    }

    /// A payload graph is derived from the identifier, so a record aiming one half at another
    /// candidate's graph — or at a vocabulary — is refused.
    #[test]
    fn a_record_naming_someone_elses_removal_graph_is_refused() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);
        let candidate = store
            .propose_retraction(
                &graph,
                RdfSyntax::Turtle,
                FRENCH_LABEL.as_bytes(),
                &retraction_provenance(),
            )
            .expect("proposed");

        store
            .transaction(|txn| {
                let system = GraphId::system();
                txn.remove_graph_quads(
                    &system,
                    &[Quad::new(
                        candidate.id().subject(),
                        named_node(REMOVAL_PAYLOAD_IRI).into_owned(),
                        NamedNode::new_unchecked(removals_graph(&candidate).iri()),
                        NamedNode::new_unchecked(system.iri()),
                    )],
                )?;
                txn.insert(
                    &system,
                    vec![(
                        candidate.id().subject(),
                        named_node(REMOVAL_PAYLOAD_IRI).into_owned(),
                        NamedNode::new_unchecked(format!(
                            "{CANDIDATE_GRAPH_PREFIX}999{CANDIDATE_REMOVALS_SUFFIX}"
                        ))
                        .into(),
                    )],
                )
            })
            .expect("forge the record");

        let read = store.candidate(candidate.id());
        assert!(
            matches!(read, Err(StoreError::Corrupt { .. })),
            "a payload graph is derived, not chosen: {read:?}"
        );
    }

    /// An empty file is refused for a retraction exactly as it is for an import.
    #[test]
    fn an_empty_retraction_file_is_refused() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        let refused = store.propose_retraction(
            &graph,
            RdfSyntax::Turtle,
            "".as_bytes(),
            &retraction_provenance(),
        );
        assert!(
            matches!(refused, Err(StoreError::ImportEmpty { .. })),
            "proposing nothing is not a decision anyone can take: {refused:?}"
        );
    }

    /// The rule that a proposal goes to one vocabulary holds for both halves.
    #[test]
    fn a_retraction_naming_another_graph_is_refused() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        let refused = store.propose_retraction(
            &graph,
            RdfSyntax::NQuads,
            "<https://example.org/animals/cat> \
             <http://www.w3.org/2004/02/skos/core#prefLabel> \"Chat\"@fr \
             <https://example.org/elsewhere> .\n"
                .as_bytes(),
            &retraction_provenance(),
        );
        assert!(
            matches!(refused, Err(StoreError::ImportGraphMismatch { .. })),
            "a removal aimed at another vocabulary must be refused: {refused:?}"
        );
    }

    /// A retraction may not be aimed at OpenBiz's own graphs, for the same reason an import may
    /// not: the registry is not a vocabulary and is not authored.
    #[test]
    fn a_retraction_against_a_system_graph_is_refused() {
        let dir = temp_dir();
        let (store, _) = store_with_content(&dir);

        let refused = store.propose_retraction(
            &GraphId::system(),
            RdfSyntax::Turtle,
            FRENCH_LABEL.as_bytes(),
            &retraction_provenance(),
        );
        assert!(
            matches!(
                refused,
                Err(StoreError::CandidateTargetNotVocabulary {
                    kind: GraphKind::System,
                    ..
                })
            ),
            "OpenBiz's own graphs are not authored: {refused:?}"
        );
    }

    /// What an operator will actually do: export the vocabulary, cut it down to the statements
    /// they want gone, and hand that back. It has to round-trip, or the workflow does not exist.
    #[test]
    fn a_retraction_reads_the_syntax_the_export_writes() {
        for syntax in RdfSyntax::ALL {
            let dir = temp_dir();
            let (store, graph) = store_with_content(&dir);

            let mut exported = Vec::new();
            store
                .export_graph(graph.iri(), syntax, &mut exported)
                .expect("export the vocabulary");

            let candidate = store
                .propose_retraction(
                    &graph,
                    syntax,
                    exported.as_slice(),
                    &retraction_provenance(),
                )
                .unwrap_or_else(|error| {
                    panic!("an export of a vocabulary must retract from it as {syntax}: {error}")
                });
            assert_eq!(candidate.removals(), 4, "as {syntax}");

            store
                .decide(candidate.id(), Decision::Approve, "a reviewer")
                .expect("approved");
            assert!(
                statements(&store, &graph).is_empty(),
                "retracting a whole export must empty the vocabulary, as {syntax}"
            );
        }
    }

    /// A blank node survives export-edit-retract, and a *hand-written* one does not. Both halves
    /// are measured rather than assumed.
    ///
    /// An import renames blank node labels so two files using `_:b1` do not merge; a retraction
    /// cannot, because a renamed label would match nothing. That leaves the workflow's fate resting
    /// on whether our serialiser writes labels our parser reads back as the same node — which no
    /// RDF specification promises, so it is pinned here. It holds: an export of a vocabulary
    /// retracts the vocabulary, blank nodes included.
    ///
    /// What does *not* hold, and must not silently half-work, is a file somebody typed. `_:note`
    /// in a hand-written file is a different node from the one in the store no matter how it is
    /// spelled, so it is refused by the presence check rather than removing something adjacent.
    #[test]
    fn a_blank_node_retracts_from_our_own_export_and_never_from_a_hand_written_label() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);
        let imported = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                r#"
                    @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
                    <https://example.org/animals/cat> skos:note [
                        skos:prefLabel "an unnamed note"@en
                    ] .
                "#
                .as_bytes(),
                &import_provenance(),
            )
            .expect("proposed");
        store
            .decide(imported.id(), Decision::Approve, "a reviewer")
            .expect("approved");
        assert_eq!(statements(&store, &graph).len(), 2);

        // A label nobody in the store uses: refused, and the vocabulary is untouched.
        let typed_by_hand = store.propose_retraction(
            &graph,
            RdfSyntax::NTriples,
            "_:note <http://www.w3.org/2004/02/skos/core#prefLabel> \"an unnamed note\"@en .\n"
                .as_bytes(),
            &retraction_provenance(),
        );
        assert!(
            matches!(
                typed_by_hand,
                Err(StoreError::RetractionNotPresent { absent: 1, .. })
            ),
            "a blank node label somebody invented names no statement in the store: \
             {typed_by_hand:?}"
        );
        assert_eq!(statements(&store, &graph).len(), 2);

        // Our own export of the same statements: accepted, and it removes all of them.
        let mut exported = Vec::new();
        store
            .export_graph(graph.iri(), RdfSyntax::NTriples, &mut exported)
            .expect("export the vocabulary");
        let candidate = store
            .propose_retraction(
                &graph,
                RdfSyntax::NTriples,
                exported.as_slice(),
                &retraction_provenance(),
            )
            .expect("an export of a vocabulary must retract from it, blank nodes included");
        store
            .decide(candidate.id(), Decision::Approve, "a reviewer")
            .expect("approved");
        assert!(
            statements(&store, &graph).is_empty(),
            "a blank node that round-trips must round-trip completely"
        );
    }

    /// The whole point of the seam: an import proposes, and the vocabulary does not change.
    #[test]
    fn an_import_proposes_rather_than_writes() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);

        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("the import is proposed");

        assert_eq!(candidate.state(), CandidateState::Proposed);
        assert_eq!(candidate.additions(), 4);
        assert_eq!(candidate.target(), &graph);
        assert_eq!(candidate.provenance().source, CandidateSource::Import);
        assert_eq!(candidate.decided_by(), None);

        assert!(
            statements(&store, &graph).is_empty(),
            "an unapproved candidate must not have reached the vocabulary"
        );
        assert_eq!(
            statements(&store, additions_graph(&candidate)).len(),
            4,
            "the proposed statements must be staged where a reviewer can read them"
        );
    }

    /// A staged proposal is visible to an operator and is not one of the user's vocabularies.
    #[test]
    fn a_staging_graph_is_registered_as_a_candidate_and_not_as_a_vocabulary() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("the import is proposed");

        let registry = store.graphs().expect("read the registry");
        let staged = registry
            .iter()
            .find(|entry| entry.iri() == additions_graph(&candidate).iri())
            .expect("the staging graph is in the registry, because the store holds its statements");
        assert_eq!(staged.kind(), GraphKind::Candidate);
        assert_eq!(
            registry
                .iter()
                .filter(|entry| entry.kind() == GraphKind::Vocabulary)
                .count(),
            1,
            "a pending proposal must not appear as a vocabulary"
        );
    }

    /// Approval is what moves statements, and it records who moved them.
    #[test]
    fn approval_applies_the_statements_and_says_who_approved_them() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("the import is proposed");

        let decided = store
            .decide(candidate.id(), Decision::Approve, "ada@example.org")
            .expect("the candidate is approved");

        assert_eq!(decided.state(), CandidateState::Applied);
        assert_eq!(decided.decided_by(), Some("ada@example.org"));
        assert!(decided.decided_at().is_some());

        assert_eq!(
            statements(&store, &graph),
            statements(&store, additions_graph(&candidate)),
            "approval must land exactly the statements that were staged"
        );

        // The decision survives a reopen, because it is in the store rather than in a log.
        store.close().expect("a clean close");
        let reopened = Store::open(dir.path()).expect("reopen");
        let read_back = reopened.candidate(candidate.id()).expect("still there");
        assert_eq!(read_back.state(), CandidateState::Applied);
        assert_eq!(read_back.decided_by(), Some("ada@example.org"));
        assert_eq!(read_back, decided);
    }

    #[test]
    fn rejection_changes_nothing_in_the_vocabulary_and_keeps_the_evidence() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("the import is proposed");

        let decided = store
            .decide(candidate.id(), Decision::Reject, "ada@example.org")
            .expect("the candidate is rejected");

        assert_eq!(decided.state(), CandidateState::Rejected);
        assert!(
            statements(&store, &graph).is_empty(),
            "a rejected candidate must not have reached the vocabulary"
        );
        assert_eq!(
            statements(&store, additions_graph(&candidate)).len(),
            4,
            "what was refused must stay readable; deleting the evidence is not a default"
        );
    }

    #[test]
    fn a_candidate_is_decided_once() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("the import is proposed");
        store
            .decide(candidate.id(), Decision::Approve, "ada@example.org")
            .expect("approved");

        let again = store.decide(candidate.id(), Decision::Approve, "ada@example.org");
        assert!(
            matches!(
                again,
                Err(StoreError::CandidateDecided {
                    state: CandidateState::Applied,
                    ..
                })
            ),
            "approving twice must be refused, not silently repeated: {again:?}"
        );

        let reversal = store.decide(candidate.id(), Decision::Reject, "bob@example.org");
        assert!(
            matches!(reversal, Err(StoreError::CandidateDecided { .. })),
            "a decision is not undone by taking the opposite one: {reversal:?}"
        );
    }

    /// The round trip, for every syntax `CLAUDE.md` §2 commits to: what the serialiser wrote is
    /// what the parser proposes, statement for statement.
    #[test]
    fn every_syntax_round_trips_through_a_candidate() {
        for syntax in RdfSyntax::ALL {
            let dir = temp_dir();
            let (store, graph) = store_with_vocabulary(&dir);
            store
                .propose_import(
                    &graph,
                    RdfSyntax::Turtle,
                    CAT_TURTLE.as_bytes(),
                    &import_provenance(),
                )
                .and_then(|candidate| store.decide(candidate.id(), Decision::Approve, "setup"))
                .expect("seed the vocabulary");

            let mut exported = Vec::new();
            store
                .export_graph(graph.iri(), syntax, &mut exported)
                .unwrap_or_else(|error| panic!("export as {syntax}: {error}"));

            let candidate = store
                .propose_import(&graph, syntax, exported.as_slice(), &import_provenance())
                .unwrap_or_else(|error| panic!("re-import {syntax}: {error}"));

            assert_eq!(
                statements(&store, additions_graph(&candidate)),
                statements(&store, &graph),
                "a {syntax} export did not propose back the statements it was written from"
            );
            store.close().expect("a clean close");
        }
    }

    /// A quad syntax can name graphs, and an import goes to one vocabulary.
    #[test]
    fn a_file_naming_another_graph_is_refused_rather_than_flattened() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);

        let elsewhere = "<https://example.org/animals/cat> \
                         <http://www.w3.org/2004/02/skos/core#prefLabel> \"Cat\" \
                         <https://example.org/plants> .\n";
        let refused = store.propose_import(
            &graph,
            RdfSyntax::NQuads,
            elsewhere.as_bytes(),
            &import_provenance(),
        );
        assert!(
            matches!(refused, Err(StoreError::ImportGraphMismatch { .. })),
            "a file naming another vocabulary must be refused: {refused:?}"
        );

        // Naming the target itself is the ordinary case — it is what our own export writes.
        let same = format!(
            "<https://example.org/animals/cat> \
             <http://www.w3.org/2004/02/skos/core#prefLabel> \"Cat\" <{}> .\n",
            graph.iri()
        );
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::NQuads,
                same.as_bytes(),
                &import_provenance(),
            )
            .expect("a file naming the target graph is the round-trip case");
        assert_eq!(candidate.additions(), 1);
    }

    #[test]
    fn an_import_into_something_that_is_not_a_registered_vocabulary_is_refused() {
        let dir = temp_dir();
        let (store, _graph) = store_with_vocabulary(&dir);

        let unregistered = vocabulary("https://example.org/never-created");
        let missing = store.propose_import(
            &unregistered,
            RdfSyntax::Turtle,
            CAT_TURTLE.as_bytes(),
            &import_provenance(),
        );
        assert!(
            matches!(missing, Err(StoreError::NoSuchGraph { .. })),
            "importing must not create a vocabulary as a side effect (CLAUDE.md §1.7): {missing:?}"
        );

        let system = store.propose_import(
            &GraphId::system(),
            RdfSyntax::Turtle,
            CAT_TURTLE.as_bytes(),
            &import_provenance(),
        );
        assert!(
            matches!(
                system,
                Err(StoreError::CandidateTargetNotVocabulary {
                    kind: GraphKind::System,
                    ..
                })
            ),
            "OpenBiz's own metadata is not a place a user imports into: {system:?}"
        );
    }

    #[test]
    fn a_file_that_proposes_nothing_is_refused() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);

        for empty in ["", "# just a comment\n"] {
            let refused = store.propose_import(
                &graph,
                RdfSyntax::Turtle,
                empty.as_bytes(),
                &import_provenance(),
            );
            assert!(
                matches!(refused, Err(StoreError::ImportEmpty { .. })),
                "an empty proposal is a decision nobody can take: {refused:?}"
            );
        }
    }

    #[test]
    fn a_syntax_error_names_the_line_the_editor_shows() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);

        let broken =
            "<https://example.org/a> <https://example.org/b> \"ok\" .\nthis is not turtle\n";
        let refused = store.propose_import(
            &graph,
            RdfSyntax::Turtle,
            broken.as_bytes(),
            &import_provenance(),
        );
        let Err(StoreError::ImportSyntax { line, syntax, .. }) = refused else {
            panic!("a broken file must be refused as a syntax error: {refused:?}");
        };
        assert_eq!(
            syntax,
            RdfSyntax::Turtle,
            "the message names what we read it as"
        );
        assert_eq!(
            line,
            Some(2),
            "the parser counts from zero and editors count from one"
        );

        assert!(
            store.candidates().expect("readable").is_empty(),
            "a refused import must leave no half-staged candidate behind"
        );
    }

    #[test]
    fn provenance_a_reviewer_could_not_act_on_is_refused() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);

        let cases = [
            Provenance {
                agent: "  ".to_owned(),
                ..import_provenance()
            },
            Provenance {
                note: String::new(),
                ..import_provenance()
            },
            Provenance {
                confidence: Some(1.5),
                ..import_provenance()
            },
            Provenance {
                confidence: Some(-0.1),
                ..import_provenance()
            },
        ];

        for provenance in cases {
            let refused = store.propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &provenance,
            );
            assert!(
                matches!(refused, Err(StoreError::CandidateProvenance { .. })),
                "{provenance:?} must not be proposable: {refused:?}"
            );
        }
    }

    #[test]
    fn a_decision_that_says_who_took_it_is_the_only_kind() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("proposed");

        let anonymous = store.decide(candidate.id(), Decision::Approve, "   ");
        assert!(
            matches!(anonymous, Err(StoreError::CandidateProvenance { .. })),
            "an unattributed approval is the one thing an audit trail cannot afford: {anonymous:?}"
        );
        assert_eq!(
            store
                .candidate(candidate.id())
                .expect("still there")
                .state(),
            CandidateState::Proposed,
            "a refused decision must not have half-applied"
        );
    }

    #[test]
    fn candidates_are_numbered_in_the_order_they_were_raised() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);

        let first = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("first");
        let second = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("second");

        assert_ne!(first.id(), second.id());
        assert_ne!(
            additions_graph(&first).iri(),
            additions_graph(&second).iri(),
            "two proposals must not share a staging graph"
        );
        assert_eq!(
            store
                .candidates()
                .expect("listed")
                .iter()
                .map(Candidate::id)
                .collect::<Vec<_>>(),
            vec![first.id(), second.id()]
        );

        // Numbering survives a restart: the store, not a counter in memory, is the authority.
        store.close().expect("a clean close");
        let reopened = Store::open(dir.path()).expect("reopen");
        let third = reopened
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("third");
        assert_ne!(third.id(), first.id());
        assert_ne!(third.id(), second.id());
        assert_eq!(reopened.candidates().expect("listed").len(), 3);
    }

    /// Two files that both say `_:b1` are not talking about the same thing.
    #[test]
    fn blank_nodes_from_separate_imports_do_not_merge() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);

        let with_blank = "<https://example.org/animals/cat> \
                          <http://www.w3.org/2004/02/skos/core#note> _:b1 .\n\
                          _:b1 <http://www.w3.org/2000/01/rdf-schema#label> \"a note\" .\n";

        for _ in 0..2 {
            let candidate = store
                .propose_import(
                    &graph,
                    RdfSyntax::NTriples,
                    with_blank.as_bytes(),
                    &import_provenance(),
                )
                .expect("proposed");
            store
                .decide(candidate.id(), Decision::Approve, "ada@example.org")
                .expect("approved");
        }

        assert_eq!(
            statements(&store, &graph).len(),
            4,
            "two imports of one file must produce two notes, not silently merge into one"
        );
    }

    #[test]
    fn a_candidate_identifier_has_exactly_one_spelling() {
        assert_eq!(CandidateId::parse("7").expect("seven").to_string(), "7");
        for bad in ["", "007", "-1", " 7", "7 ", "seven", "7.0", "0x7"] {
            assert!(
                CandidateId::parse(bad).is_err(),
                "{bad:?} must not identify a candidate"
            );
        }
        assert!(
            CandidateId::parse("0").is_ok(),
            "zero is a spelling, even if nothing mints it"
        );
    }

    #[test]
    fn asking_for_a_candidate_that_never_existed_says_so() {
        let dir = temp_dir();
        let (store, _graph) = store_with_vocabulary(&dir);
        let missing = store.candidate(CandidateId::parse("42").expect("valid"));
        assert!(
            matches!(missing, Err(StoreError::NoSuchCandidate { .. })),
            "{missing:?}"
        );
    }

    /// The record is data on disk, so it is judged rather than trusted on the way back in.
    #[test]
    fn a_record_that_cannot_say_who_decided_it_is_refused() {
        let dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&dir);
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("proposed");

        // Forge the state without the attribution that must accompany it — which is what a
        // hand-edited store or a doctored backup would look like.
        store
            .transaction(|txn| {
                let system = GraphId::system();
                txn.remove_graph_quads(
                    &system,
                    &[Quad::new(
                        candidate.id().subject(),
                        named_node(STATE_IRI).into_owned(),
                        Literal::new_simple_literal(CandidateState::Proposed.as_str()),
                        NamedNode::new_unchecked(system.iri()),
                    )],
                )?;
                txn.insert(
                    &system,
                    vec![(
                        candidate.id().subject(),
                        named_node(STATE_IRI).into_owned(),
                        Literal::new_simple_literal(CandidateState::Applied.as_str()).into(),
                    )],
                )
            })
            .expect("write the forged record");

        let read = store.candidate(candidate.id());
        assert!(
            matches!(read, Err(StoreError::Corrupt { .. })),
            "a decided candidate with no decider must be refused, not shown: {read:?}"
        );
    }

    /// The stamps get the same treatment as every other field, which they did not until format
    /// version 5: `proposed_at` came back through the same reader that returns a note or an
    /// agent's name, so a record saying it was raised "last Tuesday" — or at a time naming no
    /// timezone, which no reader can order against any other record — was read, kept, and printed
    /// to a reviewer as though the trail could account for it.
    #[test]
    fn a_record_whose_stamp_nobody_can_place_is_refused() {
        for (predicate, what, lexical, expected) in [
            (
                PROPOSED_AT_IRI,
                "raised",
                "2026-08-19T14:17:03",
                "names no timezone",
            ),
            (PROPOSED_AT_IRI, "raised", "2026-08-19", "is not a date"),
            (PROPOSED_AT_IRI, "raised", "last Tuesday", "is not a date"),
            (
                DECIDED_AT_IRI,
                "decided",
                "2026-08-19T14:17:03",
                "names no timezone",
            ),
        ] {
            let dir = temp_dir();
            let (store, graph) = store_with_vocabulary(&dir);
            let candidate = store
                .propose_import(
                    &graph,
                    RdfSyntax::Turtle,
                    CAT_TURTLE.as_bytes(),
                    &import_provenance(),
                )
                .expect("proposed");
            let candidate = if predicate == DECIDED_AT_IRI {
                store
                    .decide(candidate.id(), Decision::Reject, "ada")
                    .expect("decided")
            } else {
                candidate
            };

            let system = GraphId::system();
            let existing = store
                .backend
                .system_quads(
                    Some(candidate.id().subject().as_ref()),
                    named_node(predicate),
                )
                .expect("the system graph is readable");
            store
                .transaction(|txn| {
                    txn.remove_graph_quads(&system, &existing)?;
                    txn.insert(
                        &system,
                        vec![(
                            candidate.id().subject(),
                            named_node(predicate).into_owned(),
                            Literal::new_typed_literal(lexical, xsd::DATE_TIME).into(),
                        )],
                    )
                })
                .expect("write the forged record");

            let read = store.candidate(candidate.id());
            let Err(StoreError::Corrupt { detail, .. }) = &read else {
                panic!("{lexical:?} as <{predicate}> should be refused, not {read:?}");
            };
            assert!(
                detail.contains(expected) && detail.contains(what),
                "the message must say which stamp and why: {detail}"
            );
            assert!(
                !detail.contains("  "),
                "a run of spaces means a lost line continuation: {detail:?}"
            );
        }
    }

    /// A pending proposal is part of the store, so it has to survive the store's own disaster
    /// recovery — including `GraphId::classify`, which judges every graph name in a backup.
    #[test]
    fn a_pending_candidate_survives_a_backup_and_restore() {
        let source_dir = temp_dir();
        let (store, graph) = store_with_vocabulary(&source_dir);
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                CAT_TURTLE.as_bytes(),
                &import_provenance(),
            )
            .expect("proposed");

        let mut backup = Vec::new();
        store.backup(&mut backup).expect("back the store up");
        store.close().expect("a clean close");

        let target_dir = temp_dir();
        let restored = Store::open(target_dir.path()).expect("open an empty store");
        restored
            .restore(backup.as_slice())
            .expect("a store with a pending proposal restores");

        let read_back = restored
            .candidate(candidate.id())
            .expect("the record came back");
        assert_eq!(read_back, candidate);
        assert_eq!(
            statements(&restored, additions_graph(&read_back)).len(),
            4,
            "the proposed statements came back with the record"
        );

        // And it is still decidable, which is the thing that would be lost if only the record
        // survived: an approval reads the payload out of the restored store.
        restored
            .decide(candidate.id(), Decision::Approve, "ada@example.org")
            .expect("approve after a restore");
        assert_eq!(statements(&restored, &graph).len(), 4);
    }

    // ----------------------------------------------------------------------------------------
    // Both halves at once: what a bulk operation raises, through `propose_edit`.
    // ----------------------------------------------------------------------------------------

    fn edit_provenance() -> Provenance {
        Provenance {
            source: CandidateSource::BulkEdit,
            agent: "ada@example.org (openbiz move)".to_owned(),
            note: "moved the cat under carnivore".to_owned(),
            confidence: None,
        }
    }

    const CAT: &str = "https://example.org/animals/cat";
    const MAMMAL: &str = "https://example.org/animals/mammal";
    const CARNIVORE: &str = "https://example.org/animals/carnivore";
    const BROADER: &str = "http://www.w3.org/2004/02/skos/core#broader";

    /// `<subject> <predicate> <object>`, all three IRIs, which is every statement a move computes.
    fn link<'a>(subject: &'a str, predicate: &'a str, object: &'a str) -> StatementRef<'a> {
        StatementRef {
            subject: StatementTerm::Iri(subject),
            predicate,
            object: StatementTerm::Iri(object),
        }
    }

    /// The shape of the whole feature: one candidate, two halves, applied as one decision.
    #[test]
    fn a_computed_change_stages_both_halves_and_applies_both() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        let candidate = store
            .propose_edit(
                &graph,
                &[link(CAT, BROADER, CARNIVORE)],
                &[link(CAT, BROADER, MAMMAL)],
                &edit_provenance(),
            )
            .expect("a move is proposable");

        assert_eq!(candidate.additions(), 1);
        assert_eq!(candidate.removals(), 1);
        assert_eq!(statements(&store, additions_graph(&candidate)).len(), 1);
        assert_eq!(statements(&store, removals_graph(&candidate)).len(), 1);

        // Nothing has reached the vocabulary: the old link is still there and the new one is not.
        let before = statements(&store, &graph);
        assert!(before.iter().any(|line| line.contains(MAMMAL)));
        assert!(!before.iter().any(|line| line.contains(CARNIVORE)));

        store
            .decide(candidate.id(), Decision::Approve, "a reviewer")
            .expect("approve");

        let after = statements(&store, &graph);
        assert!(
            !after.iter().any(|line| line.contains(MAMMAL)),
            "the removal half must have been applied: {after:?}"
        );
        assert!(
            after.iter().any(|line| line.contains(CARNIVORE)),
            "the addition half must have been applied: {after:?}"
        );
        assert_eq!(
            after.len(),
            before.len(),
            "one statement out and one in leaves the count where it was"
        );
    }

    /// The order `apply_payload` documents, and the only shape that can observe it.
    ///
    /// A statement in **both** halves is removed and then added back, so it survives. If the two
    /// halves ever ran the other way round it would be added and then removed, and the vocabulary
    /// would silently lose a statement the reviewer was told would stay. No producer in this build
    /// computes such a change; the order is still a promise the record makes, so it is pinned here
    /// rather than left to the next producer to discover.
    #[test]
    fn a_statement_in_both_halves_survives_because_removals_run_first() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        let candidate = store
            .propose_edit(
                &graph,
                &[link(CAT, BROADER, MAMMAL)],
                &[link(CAT, BROADER, MAMMAL)],
                &edit_provenance(),
            )
            .expect("a change that removes and re-adds one statement is expressible");
        store
            .decide(candidate.id(), Decision::Approve, "a reviewer")
            .expect("approve");

        assert!(
            statements(&store, &graph)
                .iter()
                .any(|line| line.contains(MAMMAL)),
            "removals run before additions, so a statement in both halves stays"
        );
    }

    /// A removal the vocabulary does not hold is refused here for the same reason a file's is.
    #[test]
    fn a_computed_removal_the_vocabulary_does_not_hold_is_refused() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        let error = store
            .propose_edit(
                &graph,
                &[link(CAT, BROADER, CARNIVORE)],
                &[link(CAT, BROADER, CARNIVORE)],
                &edit_provenance(),
            )
            .expect_err("a removal of a statement that is not there is refused");
        assert!(
            matches!(error, StoreError::RetractionNotPresent { absent: 1, .. }),
            "{error}"
        );
        assert!(
            store.candidates().expect("readable").len() == 1,
            "the refusal rolled the whole proposal back, including its identifier"
        );
    }

    /// A half with nothing in it gets no graph and no count, because `read_record` pairs them.
    #[test]
    fn a_change_that_only_adds_is_a_candidate_with_one_half() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        let candidate = store
            .propose_edit(
                &graph,
                &[link(CAT, BROADER, CARNIVORE)],
                &[],
                &edit_provenance(),
            )
            .expect("proposable");
        assert_eq!(candidate.removals(), 0);
        assert!(candidate.removal_payload().is_none());
        assert_eq!(
            store.candidate(candidate.id()).expect("readable"),
            candidate,
            "the record round-trips, which is what the count-and-graph invariant guards"
        );
    }

    /// A proposal to do nothing is refused at the door rather than written and refused on read.
    #[test]
    fn a_change_that_neither_adds_nor_removes_is_refused() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        let error = store
            .propose_edit(&graph, &[], &[], &edit_provenance())
            .expect_err("nothing is not a decision");
        assert!(matches!(error, StoreError::CandidateEmpty), "{error}");
        assert_eq!(
            store.candidates().expect("readable").len(),
            1,
            "only the import that seeded the vocabulary; no identifier was burned"
        );
    }

    /// The counts are of distinct statements, which is the thing the file paths cannot do cheaply.
    #[test]
    fn a_statement_computed_twice_is_counted_once() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        let candidate = store
            .propose_edit(
                &graph,
                &[link(CAT, BROADER, CARNIVORE), link(CAT, BROADER, CARNIVORE)],
                &[link(CAT, BROADER, MAMMAL), link(CAT, BROADER, MAMMAL)],
                &edit_provenance(),
            )
            .expect("proposable");
        assert_eq!(
            (candidate.additions(), candidate.removals()),
            (1, 1),
            "an RDF graph is a set, so the count a reviewer reads must be of distinct statements"
        );
    }

    /// A producer computing statements has had no parser check them, so the store checks.
    #[test]
    fn a_statement_rdf_cannot_express_is_refused_rather_than_mapped() {
        let dir = temp_dir();
        let (store, graph) = store_with_content(&dir);

        let literal_subject = StatementRef {
            subject: StatementTerm::Literal {
                value: "Cat",
                language: Some("en"),
                datatype: "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString",
            },
            predicate: BROADER,
            object: StatementTerm::Iri(CARNIVORE),
        };
        let error = store
            .propose_edit(&graph, &[literal_subject], &[], &edit_provenance())
            .expect_err("a literal in subject position is not RDF");
        assert!(
            matches!(error, StoreError::CandidateStatementInvalid { .. }),
            "{error}"
        );

        let not_an_iri = link(CAT, "not a predicate IRI", CARNIVORE);
        let error = store
            .propose_edit(&graph, &[not_an_iri], &[], &edit_provenance())
            .expect_err("a predicate that is not an IRI is refused");
        assert!(
            matches!(error, StoreError::CandidateStatementInvalid { .. }),
            "{error}"
        );
    }

    /// The same refusal `propose_import` gives, because a computed change is not a way around it.
    #[test]
    fn a_computed_change_against_a_graph_that_is_not_a_vocabulary_is_refused() {
        let dir = temp_dir();
        let (store, _) = store_with_content(&dir);

        let error = store
            .propose_edit(
                &GraphId::system(),
                &[link(CAT, BROADER, CARNIVORE)],
                &[],
                &edit_provenance(),
            )
            .expect_err("OpenBiz's own graphs are not authored");
        assert!(
            matches!(error, StoreError::CandidateTargetNotVocabulary { .. }),
            "{error}"
        );
    }
}
