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
//! # Scope of this build
//!
//! Candidates are **additive**: a candidate proposes statements to add. Proposing removals — which
//! a merge, a deprecation, or a corrective agent all need — is the next slice of the seam and is
//! recorded as such in `docs/BUILD-PLAN.md`. The record shape leaves room for it; nothing here
//! assumes additions are all there will ever be.
//!
//! Approval applies immediately, so the terminal states are [`CandidateState::Applied`] and
//! [`CandidateState::Rejected`] rather than an "approved but not yet applied" limbo. When Phase 6
//! gives approval a workflow of its own, that limbo becomes real and gets its own state; naming a
//! state today that nothing can produce would be a claim about a capability we do not have.

use std::collections::BTreeMap;
use std::io::Read;

use oxigraph::io::RdfParser;
use oxigraph::model::vocab::{rdf, xsd};
use oxigraph::model::{GraphName, Literal, NamedNode, Quad, Term};
use oxsdatatypes::DateTime;
use thiserror::Error;

use crate::{
    named_node, GraphId, GraphKind, RdfSyntax, RegistryReader, Store, StoreError, Transaction,
};

/// The class every candidate's record is typed with, in the system graph.
const CANDIDATE_CLASS_IRI: &str = "urn:openbiz:Candidate";

/// Prefix under which a candidate's record subject is minted.
const CANDIDATE_SUBJECT_PREFIX: &str = "urn:openbiz:candidate:";

/// Predicate naming the vocabulary graph a candidate proposes to change.
const TARGET_IRI: &str = "urn:openbiz:candidateTarget";

/// Predicate naming the graph the proposed statements are staged in.
const PAYLOAD_IRI: &str = "urn:openbiz:candidatePayload";

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
    payload: GraphId,
    provenance: Provenance,
    proposed_at: String,
    additions: u64,
    state: CandidateState,
    decided_by: Option<String>,
    decided_at: Option<String>,
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

    /// The graph its proposed statements are staged in.
    ///
    /// This is what a reviewer exports or queries to see what would happen. It stays after a
    /// decision, applied or rejected, so "what exactly was approved" remains answerable.
    pub fn payload(&self) -> &GraphId {
        &self.payload
    }

    /// Where it came from and why.
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// When it was raised, as an `xsd:dateTime` lexical form.
    pub fn proposed_at(&self) -> &str {
        &self.proposed_at
    }

    /// How many statements it proposes to add.
    pub fn additions(&self) -> u64 {
        self.additions
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
        self.decided_at.as_deref()
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
    ///   and an import goes to *one* vocabulary. Silently dropping the names would land somebody's
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
        provenance.validate()?;

        if target.kind() != GraphKind::Vocabulary {
            return Err(StoreError::CandidateTargetNotVocabulary {
                iri: target.iri().to_owned(),
                kind: target.kind(),
            });
        }

        let parser = RdfParser::from_format(syntax.backend())
            .rename_blank_nodes()
            .with_base_iri(target.iri())
            .map_err(|error| {
                // Unreachable in practice: `GraphId` validated this IRI through the same parser.
                StoreError::Backend(format!(
                    "the target graph's IRI is not usable as a base IRI: {error}"
                ))
            })?
            .for_reader(reader);

        self.transaction(|txn| {
            if !txn.contains_graph(target.iri())? {
                return Err(StoreError::NoSuchGraph {
                    iri: target.iri().to_owned(),
                });
            }

            let id = next_candidate_id(txn)?;
            let payload = GraphId::candidate(&id);
            let payload_name: GraphName = NamedNode::new_unchecked(payload.iri()).into();

            let mut batch: Vec<Quad> = Vec::with_capacity(STAGE_BATCH);
            let mut additions: u64 = 0;

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

                batch.push(Quad::new(
                    quad.subject,
                    quad.predicate,
                    quad.object,
                    payload_name.clone(),
                ));
                additions += 1;

                if batch.len() == STAGE_BATCH {
                    txn.extend_graph(&payload, &batch)?;
                    batch.clear();
                }
            }
            txn.extend_graph(&payload, &batch)?;

            if additions == 0 {
                return Err(StoreError::ImportEmpty { syntax });
            }

            txn.register(&payload)?;

            let candidate = Candidate {
                id,
                target: target.clone(),
                payload,
                provenance: provenance.clone(),
                proposed_at: DateTime::now().to_string(),
                additions,
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
    /// The payload graph is kept either way. Deleting the evidence of what was approved is not a
    /// default a governance product may take; what it costs is recorded in `docs/UNTESTED.md`.
    ///
    /// Refuses a candidate that has already been decided, naming the state it is in — deciding one
    /// twice would either duplicate its statements or silently do nothing, and both are worse than
    /// being told.
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

            let decided_at = DateTime::now().to_string();
            txn.retract(&[Quad::new(
                candidate.id.subject(),
                named_node(STATE_IRI).into_owned(),
                Literal::new_simple_literal(candidate.state.as_str()),
                NamedNode::new_unchecked(GraphId::system().iri()),
            )])?;

            candidate.state = decision.outcome();
            candidate.decided_by = Some(decided_by.to_owned());
            candidate.decided_at = Some(decided_at);

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
                        Literal::new_typed_literal(
                            candidate.decided_at.clone().unwrap_or_default(),
                            xsd::DATE_TIME,
                        )
                        .into(),
                    ),
                ],
            )?;

            Ok(candidate)
        })
    }
}

/// Copy a candidate's staged statements into its target vocabulary.
///
/// Reads the payload out in full before writing any of it: the backend's iterator borrows the
/// transaction, and a copy that streamed would be reading a graph while writing another through
/// the same handle.
fn apply_payload(txn: &mut Transaction<'_>, candidate: &Candidate) -> Result<(), StoreError> {
    let target: GraphName = NamedNode::new_unchecked(candidate.target.iri()).into();

    let staged: Vec<Quad> = txn
        .graph_quads(&candidate.payload)?
        .into_iter()
        .map(|quad| Quad::new(quad.subject, quad.predicate, quad.object, target.clone()))
        .collect();

    txn.extend_graph(&candidate.target, &staged)
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
            named_node(PAYLOAD_IRI).into_owned(),
            NamedNode::new_unchecked(candidate.payload.iri()).into(),
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
            Literal::new_typed_literal(candidate.proposed_at.clone(), xsd::DATE_TIME).into(),
        ),
        (
            subject.clone(),
            named_node(ADDITIONS_IRI).into_owned(),
            Literal::new_typed_literal(candidate.additions.to_string(), xsd::INTEGER).into(),
        ),
        (
            subject.clone(),
            named_node(STATE_IRI).into_owned(),
            literal(candidate.state.as_str()),
        ),
    ];

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
    let payload = GraphId::classify(&iri(PAYLOAD_IRI)?).map_err(|error| {
        corrupt(format!(
            "candidate {id} names a payload we cannot describe: {error}"
        ))
    })?;

    if payload != GraphId::candidate(&id) {
        return Err(corrupt(format!(
            "candidate {id} names {payload} as its payload, and a candidate's payload graph is \
             derived from its identifier rather than chosen"
        )));
    }

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
    let decided_at = match one(DECIDED_AT_IRI)? {
        None => None,
        Some(_) => Some(text(DECIDED_AT_IRI)?),
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
        provenance: Provenance {
            source: candidate_source,
            agent: text(AGENT_IRI)?,
            note: text(NOTE_IRI)?,
            confidence,
        },
        proposed_at: text(PROPOSED_AT_IRI)?,
        additions,
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

    /// Remove quads. Only ever OpenBiz's own bookkeeping, which is why it is not public.
    ///
    /// A vocabulary's statements are never removed this way: a removal of authored content is a
    /// change to a vocabulary, and `CLAUDE.md` §3 says a change to a vocabulary arrives as a
    /// candidate. The one caller here retracts a candidate's own `state` triple so the replacement
    /// can be written, which is a change to a record about a change rather than to anybody's
    /// content.
    pub(crate) fn retract(&mut self, quads: &[Quad]) -> Result<(), StoreError> {
        for quad in quads {
            let GraphName::NamedNode(graph) = &quad.graph_name else {
                return Err(StoreError::Backend(
                    "a retraction named no graph, and every quad in an OpenBiz store is in one"
                        .to_owned(),
                ));
            };
            if graph.as_str() != GraphId::system().iri() {
                return Err(StoreError::NotWritable(graph.as_str().to_owned()));
            }
            self.inner.remove(quad.as_ref());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GraphKind;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("a temporary directory")
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
            statements(&store, candidate.payload()).len(),
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
            .find(|entry| entry.iri() == candidate.payload().iri())
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
            statements(&store, candidate.payload()),
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
            statements(&store, candidate.payload()).len(),
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
                statements(&store, candidate.payload()),
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
            first.payload().iri(),
            second.payload().iri(),
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
                txn.retract(&[Quad::new(
                    candidate.id().subject(),
                    named_node(STATE_IRI).into_owned(),
                    Literal::new_simple_literal(CandidateState::Proposed.as_str()),
                    NamedNode::new_unchecked(system.iri()),
                )])?;
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
            statements(&restored, read_back.payload()).len(),
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
}
