//! The record `adr/0003` §3 requires when somebody creates rather than reuses.
//!
//! # Why this is a record and not a note
//!
//! `CLAUDE.md` §1.7 and `adr/0003` §3 say the same thing in different words: creating something new
//! when something existing would serve requires a recorded justification, and *the justification is
//! the mechanism*. The ADR is explicit about what it is not — "not a warning dialog, those get
//! clicked through, but an auditable record that makes proliferation visible to the people
//! accountable for it".
//!
//! "Visible to the people accountable" is the whole specification, and it is a query: **which
//! concepts were created despite something already existing under that name?** A sentence in a
//! candidate's provenance note cannot be asked that. A record with the considered resources as
//! their own statements can, and that is the difference this module exists to make.
//!
//! # Why it is keyed to the concept and not to the candidate
//!
//! Two reasons, and the second is the one that settles it.
//!
//! A candidate can create several concepts at once — `openbiz split` divides one concept into
//! three — and a single field on that candidate could not say which of the three had a match and
//! which did not. The unit being justified is a *creation*, so the record names the created IRI.
//!
//! And `openbiz mint`, the creation path a curator actually uses, has no candidate at all. It
//! computes an IRI and writes nothing, which is deliberate (see `openbiz_server::mint`). A field on
//! the candidate record would therefore cover the half of the creation surface that already has a
//! reviewer looking at it, and miss the half that has nobody.
//!
//! # Why it lives in the system graph
//!
//! Same reason the IRI-minting policy does (see [`crate::policy`]): it is a fact *about* a
//! vocabulary rather than one *in* it. Putting it in the vocabulary would export an OpenBiz
//! governance statement, that no standard defines, to whatever tool read the Turtle next. The same
//! consequence applies and is recorded in `docs/UNTESTED.md`: a whole-store backup carries these
//! records and an export of a single vocabulary does not.
//!
//! Because it is written to the system graph and never to a vocabulary, recording one is not a
//! change to a vocabulary and does not go through the candidate seam (`CLAUDE.md` §3). It is still
//! attributed, by the rule an approval and a policy are both held to.
//!
//! # What honesty costs here, and why the record carries it
//!
//! A justification is evidence that somebody looked before they created. Evidence produced by a
//! search that could not reach one of its sources, or that stopped at a bound, is weaker evidence —
//! and a record that does not say so invites an auditor to read diligence into a search that was
//! cut short. So [`Justification::search_was_complete`] is a required field, not an optional one:
//! every record states whether the looking behind it actually finished.
//!
//! # What this module does not do
//!
//! It does not run discovery, and it does not know what a match is. The considered resources arrive
//! as IRIs the caller has already found; `openbiz-discovery` is a crate this one does not depend on
//! and must not. Nor does it refuse a creation: nothing here can, because nothing in this build
//! creates a concept in one step. What it does is make the creation answerable afterwards.

use std::collections::BTreeMap;

use oxigraph::model::vocab::{rdf, xsd};
use oxigraph::model::{Literal, NamedNode, Quad, Term};
use thiserror::Error;

use crate::{
    named_node, GraphId, GraphKind, RecordedAt, RegistryReader, Store, StoreError, Transaction,
};

/// The class every justification record is typed with, in the system graph.
const JUSTIFICATION_CLASS_IRI: &str = "urn:openbiz:Justification";

/// Prefix under which a justification's record subject is minted.
const JUSTIFICATION_SUBJECT_PREFIX: &str = "urn:openbiz:justification:";

/// Predicate naming the IRI that was created.
const CONCEPT_IRI: &str = "urn:openbiz:justificationConcept";

/// Predicate naming the vocabulary the IRI was created for.
const GRAPH_IRI: &str = "urn:openbiz:justificationGraph";

/// Predicate carrying the label the new concept was created under.
const LABEL_IRI: &str = "urn:openbiz:justificationLabel";

/// Predicate naming one resource that already existed and was not reused. Repeated, or absent.
///
/// This is the predicate the whole record is for. `?j <justificationConsidered> ?resource` is the
/// auditor's question — which concepts were created despite an existing match — and it is a query
/// rather than a reading exercise precisely because each considered resource is its own statement.
const CONSIDERED_IRI: &str = "urn:openbiz:justificationConsidered";

/// Predicate carrying why none of the considered resources fitted.
const REASON_IRI: &str = "urn:openbiz:justificationReason";

/// Predicate carrying whether the search behind the record actually finished.
const COMPLETE_IRI: &str = "urn:openbiz:justificationSearchWasComplete";

/// Predicate carrying who recorded it.
const RECORDED_BY_IRI: &str = "urn:openbiz:justificationRecordedBy";

/// Predicate carrying when it was recorded, as an `xsd:dateTime` on the UTC clock.
const RECORDED_AT_IRI: &str = "urn:openbiz:justificationRecordedAt";

/// Identifies one justification.
///
/// A decimal ordinal, minted by the store as one past the highest it holds, for the same reason a
/// [`crate::CandidateId`] is: these are an audit trail, and a reader wants to know which came
/// first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JustificationId(u64);

impl JustificationId {
    /// Read an identifier a store held.
    ///
    /// Strict about the lexical form as well as the value — `007` is refused — because the
    /// identifier appears in the record's subject IRI, and two spellings of one number would be
    /// two records an audit trail could not pin down.
    fn parse(offered: &str) -> Option<Self> {
        let value: u64 = offered.parse().ok()?;
        (offered == value.to_string()).then_some(Self(value))
    }

    /// The IRI of this justification's record in the system graph.
    fn subject(self) -> NamedNode {
        // Unchecked because the prefix is a compile-time constant of ours and the suffix is a
        // decimal number, so the concatenation is always a valid IRI.
        NamedNode::new_unchecked(format!("{JUSTIFICATION_SUBJECT_PREFIX}{}", self.0))
    }
}

impl std::fmt::Display for JustificationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Why a justification was refused.
///
/// Its own error rather than a reuse of [`StoreError::PolicyRejected`], because the two are refused
/// for different reasons and an operator reading one wants to know which record was turned away.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "that justification for creating {concept} cannot be recorded, because {detail}. A \
     justification is the record adr/0003 §3 requires when something new is created rather than \
     reused, and one nobody can act on is worse than none"
)]
pub struct JustificationRejected {
    /// The IRI whose creation was being justified.
    pub concept: String,
    /// Why the record was refused.
    pub detail: String,
}

/// One recorded account of why a new concept was created rather than an existing one reused.
///
/// Read-only: produced by [`Store::record_justification`] and never changed afterwards. A
/// justification is a statement somebody made at a time, and a record of that kind that can be
/// edited later is not evidence of anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Justification {
    id: JustificationId,
    concept: String,
    graph: GraphId,
    label: String,
    considered: Vec<String>,
    reason: String,
    search_was_complete: bool,
    recorded_by: String,
    recorded_at: RecordedAt,
}

impl Justification {
    /// Its identifier.
    pub fn id(&self) -> JustificationId {
        self.id
    }

    /// The IRI that was created.
    pub fn concept(&self) -> &str {
        &self.concept
    }

    /// The vocabulary it was created for.
    pub fn graph(&self) -> &GraphId {
        &self.graph
    }

    /// The label it was created under, which is the string discovery was asked about.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Every resource that already existed under that label and was not reused.
    ///
    /// Empty is a real and useful answer: it says somebody looked, found nothing, and recorded
    /// that. It is not the same as no record at all, which says nobody looked.
    pub fn considered(&self) -> &[String] {
        &self.considered
    }

    /// Why none of them fitted, in the words of the person who decided that.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Whether the search behind this record reached every source and listed every match.
    ///
    /// `false` means the evidence is partial — a source was unavailable, or the match list stopped
    /// at its bound — and an auditor weighing this record should know that without having to
    /// reconstruct the search.
    pub fn search_was_complete(&self) -> bool {
        self.search_was_complete
    }

    /// Who recorded it, named the way an auditor would want it named.
    pub fn recorded_by(&self) -> &str {
        &self.recorded_by
    }

    /// When it was recorded, as an `xsd:dateTime` on the UTC clock.
    pub fn recorded_at(&self) -> &str {
        self.recorded_at.as_str()
    }
}

impl Store {
    /// Record why a new concept was created for `graph` rather than one of `considered` reused.
    ///
    /// Appends; it never replaces. Two justifications for one IRI are two statements made at two
    /// times, and overwriting the first would delete evidence — the opposite of what the record is
    /// for. [`Store::justifications`] returns both, oldest first.
    ///
    /// Refuses a record no auditor could act on: a blank reason, a blank attribution, a blank
    /// label, a concept or considered resource that is not an IRI, a graph that is not a
    /// vocabulary, and a vocabulary the store does not hold.
    ///
    /// Writes to the system graph and never to a vocabulary, so it is not a change to a vocabulary
    /// and does not go through the candidate seam (`CLAUDE.md` §3).
    #[allow(clippy::too_many_arguments)]
    pub fn record_justification(
        &self,
        graph: &GraphId,
        concept: &str,
        label: &str,
        considered: &[String],
        reason: &str,
        search_was_complete: bool,
        recorded_by: &str,
    ) -> Result<Justification, StoreError> {
        let refuse = |detail: String| {
            Err(StoreError::JustificationRejected(JustificationRejected {
                concept: concept.to_owned(),
                detail,
            }))
        };

        if graph.kind() != GraphKind::Vocabulary {
            return refuse(format!(
                "{} is a {} graph, and concepts are created in vocabularies; nothing creates one \
                 in OpenBiz's own graphs",
                graph.iri(),
                graph.kind()
            ));
        }
        let Some(subject) = NamedNode::new(concept).ok() else {
            return refuse(
                "the thing it says was created is not an IRI, and a justification that cannot \
                 name what it justifies is unattached to anything"
                    .to_owned(),
            );
        };
        if label.trim().is_empty() {
            return refuse(
                "it records no label, and the label is the question discovery was asked; a \
                 justification that does not say what was searched for is not evidence that \
                 anything was searched for"
                    .to_owned(),
            );
        }
        if reason.trim().is_empty() {
            return refuse(
                "it gives no reason, and adr/0003 §3 asks for a reason naming why nothing that \
                 already exists fitted; a blank one is the click-through that section rules out"
                    .to_owned(),
            );
        }
        if recorded_by.trim().is_empty() {
            return refuse(
                "it does not say who decided that nothing existing fitted, and creating a \
                 concept anyway is a judgement somebody is answerable for"
                    .to_owned(),
            );
        }
        let mut considered_nodes = Vec::with_capacity(considered.len());
        for resource in considered {
            match NamedNode::new(resource) {
                Ok(node) => considered_nodes.push(node),
                Err(_) => {
                    return refuse(format!(
                        "it lists {resource:?} among what already existed, and that is not an \
                         IRI; a considered resource nobody can look up cannot be weighed against \
                         the reason for passing it over"
                    ))
                }
            }
        }

        self.transaction(|txn| {
            if !txn.contains_graph(graph.iri())? {
                return Err(StoreError::NoSuchGraph {
                    iri: graph.iri().to_owned(),
                });
            }

            let record = Justification {
                id: next_justification_id(txn)?,
                concept: subject.as_str().to_owned(),
                graph: graph.clone(),
                label: label.to_owned(),
                considered: considered_nodes
                    .iter()
                    .map(|node| node.as_str().to_owned())
                    .collect(),
                reason: reason.to_owned(),
                search_was_complete,
                recorded_by: recorded_by.to_owned(),
                recorded_at: RecordedAt::now(),
            };
            txn.extend_graph(&GraphId::system(), &quads_of(&record))?;
            Ok(record)
        })
    }

    /// Every justification the store holds, oldest first.
    ///
    /// Not narrowed to one vocabulary, because proliferation is an organisation-wide fact: the
    /// question "what did we create anyway" is asked across the store and not one vocabulary at a
    /// time. A caller wanting one vocabulary filters on [`Justification::graph`].
    pub fn justifications(&self) -> Result<Vec<Justification>, StoreError> {
        let mut ids = Vec::new();
        for quad in self.backend.system_quads(None, rdf::TYPE)? {
            if quad.object != Term::from(named_node(JUSTIFICATION_CLASS_IRI).into_owned()) {
                continue;
            }
            let Term::NamedNode(subject) = Term::from(quad.subject) else {
                return Err(StoreError::Corrupt {
                    path: self.path().to_path_buf(),
                    detail: "a justification record in the system graph has a blank node for its \
                             subject, and a record of who created what despite what cannot be \
                             addressed by an auditor"
                        .to_owned(),
                });
            };
            let id = subject
                .as_str()
                .strip_prefix(JUSTIFICATION_SUBJECT_PREFIX)
                .and_then(JustificationId::parse)
                .ok_or_else(|| StoreError::Corrupt {
                    path: self.path().to_path_buf(),
                    detail: format!(
                        "the system graph types <{subject}> as a justification, and that is not an \
                         identifier this build can read; a record it cannot address is one it \
                         cannot report"
                    ),
                })?;
            ids.push(id);
        }
        ids.sort_unstable();

        ids.into_iter()
            .map(|id| read_record(&self.backend, id, self.path()))
            .collect()
    }
}

/// The next identifier to mint, one past the highest the store holds.
///
/// Runs inside the caller's transaction, which holds the store's write lock, so two justifications
/// recorded at the same moment cannot receive the same number.
fn next_justification_id(txn: &mut Transaction<'_>) -> Result<JustificationId, StoreError> {
    let mut highest = 0;
    for quad in txn.inner.system_quads(None, rdf::TYPE)? {
        if quad.object != Term::from(named_node(JUSTIFICATION_CLASS_IRI).into_owned()) {
            continue;
        }
        if let Term::NamedNode(subject) = Term::from(quad.subject) {
            if let Some(id) = subject
                .as_str()
                .strip_prefix(JUSTIFICATION_SUBJECT_PREFIX)
                .and_then(JustificationId::parse)
            {
                highest = highest.max(id.0);
            }
        }
    }
    Ok(JustificationId(highest + 1))
}

/// The quads a justification is stored as.
///
/// The considered resources are **named nodes and not literals**, which is the one representational
/// decision in this module that matters: a query joining a justification to the concept it passed
/// over is only possible if that concept is an IRI in the object position. Written as text, the
/// record would read the same to a human and answer nothing to an auditor.
///
/// The stamp is typed for the reason `adr/0047` gives — it is the field a reader will want to order
/// — and the boolean is typed so `FILTER (?complete = false)` finds the partial evidence.
fn quads_of(record: &Justification) -> Vec<Quad> {
    let subject = record.id.subject();
    let system = NamedNode::new_unchecked(GraphId::system().iri());
    let quad = |predicate: &str, object: Term| {
        Quad::new(
            subject.clone(),
            named_node(predicate).into_owned(),
            object,
            system.clone(),
        )
    };

    let mut quads = vec![
        quad(
            rdf::TYPE.as_str(),
            named_node(JUSTIFICATION_CLASS_IRI).into_owned().into(),
        ),
        quad(
            CONCEPT_IRI,
            NamedNode::new_unchecked(record.concept.clone()).into(),
        ),
        quad(
            GRAPH_IRI,
            NamedNode::new_unchecked(record.graph.iri().to_owned()).into(),
        ),
        quad(
            LABEL_IRI,
            Literal::new_simple_literal(record.label.as_str()).into(),
        ),
        quad(
            REASON_IRI,
            Literal::new_simple_literal(record.reason.as_str()).into(),
        ),
        quad(
            COMPLETE_IRI,
            Literal::new_typed_literal(record.search_was_complete.to_string(), xsd::BOOLEAN).into(),
        ),
        quad(
            RECORDED_BY_IRI,
            Literal::new_simple_literal(record.recorded_by.as_str()).into(),
        ),
        quad(
            RECORDED_AT_IRI,
            Literal::new_typed_literal(record.recorded_at.as_str(), xsd::DATE_TIME).into(),
        ),
    ];
    for resource in &record.considered {
        quads.push(quad(
            CONSIDERED_IRI,
            NamedNode::new_unchecked(resource.clone()).into(),
        ));
    }
    quads
}

/// Read one justification record whole.
///
/// Every field except the considered list is required, and a record missing one is
/// [`StoreError::Corrupt`] rather than a record with a default. A justification with no reason, or
/// none that says whether the search finished, would be reported to an auditor as evidence while
/// being nothing of the kind.
fn read_record(
    source: &impl RegistryReader,
    id: JustificationId,
    path: &std::path::Path,
) -> Result<Justification, StoreError> {
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

    let one = |predicate: &str| -> Result<Term, StoreError> {
        match held.get(predicate).map(Vec::as_slice) {
            None | Some([]) => Err(corrupt(format!(
                "justification {id} has no <{predicate}>, which every justification record carries"
            ))),
            Some([term]) => Ok(term.clone()),
            Some(many) => Err(corrupt(format!(
                "justification {id} has {} values for <{predicate}>, and it may have at most one",
                many.len()
            ))),
        }
    };

    let text = |predicate: &str| -> Result<String, StoreError> {
        match one(predicate)? {
            Term::Literal(literal) => Ok(literal.value().to_owned()),
            other => Err(corrupt(format!(
                "justification {id} has {other} for <{predicate}>, which is not a literal"
            ))),
        }
    };

    let iri = |predicate: &str| -> Result<String, StoreError> {
        match one(predicate)? {
            Term::NamedNode(node) => Ok(node.into_string()),
            other => Err(corrupt(format!(
                "justification {id} has {other} for <{predicate}>, which is not an IRI"
            ))),
        }
    };

    let graph = GraphId::classify(&iri(GRAPH_IRI)?).map_err(|error| {
        corrupt(format!(
            "justification {id} names a vocabulary we cannot describe: {error}"
        ))
    })?;

    // Read back rather than trusted, exactly as `policy::read_policy` re-parses its stamp: a
    // record whose instant cannot be placed against anything else is not evidence of when a
    // decision was taken, and presenting it as though it were would put OpenBiz's name to a date
    // nobody can order.
    let recorded_at = RecordedAt::parse(&text(RECORDED_AT_IRI)?).map_err(|error| {
        corrupt(format!(
            "justification {id} records when it was written in a form this build cannot act on: \
             {error}"
        ))
    })?;

    let search_was_complete = match text(COMPLETE_IRI)?.as_str() {
        "true" => true,
        "false" => false,
        other => {
            return Err(corrupt(format!(
                "justification {id} says its search was {other:?} complete, and that is neither \
                 true nor false; whether the looking finished is the difference between evidence \
                 and the appearance of it"
            )))
        }
    };

    let mut considered: Vec<String> = Vec::new();
    for term in held.get(CONSIDERED_IRI).map(Vec::as_slice).unwrap_or(&[]) {
        match term {
            Term::NamedNode(node) => considered.push(node.as_str().to_owned()),
            other => {
                return Err(corrupt(format!(
                    "justification {id} lists {other} among what already existed, and that is not \
                     an IRI; a considered resource nobody can look up is not something an auditor \
                     can weigh"
                )))
            }
        }
    }
    considered.sort_unstable();

    Ok(Justification {
        id,
        concept: iri(CONCEPT_IRI)?,
        graph,
        label: text(LABEL_IRI)?,
        considered,
        reason: text(REASON_IRI)?,
        search_was_complete,
        recorded_by: text(RECORDED_BY_IRI)?,
        recorded_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENERGY: &str = "https://example.org/energy";
    const WATER: &str = "https://example.org/water";
    const SOLAR: &str = "https://example.org/energy/c_1";
    const EXISTING: &str = "https://example.org/energy/c_9";

    /// The vocabulary IRI as the API takes it.
    fn id(iri: &str) -> GraphId {
        GraphId::vocabulary(iri).expect("a vocabulary IRI")
    }

    /// A store holding two registered vocabularies, one of which describes [`EXISTING`].
    ///
    /// The concept is there so a query can *join* to it, which is the property the record's shape
    /// exists to support and which a test over the record alone cannot see.
    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path()).expect("a fresh store opens");
        for iri in [ENERGY, WATER] {
            store
                .create_vocabulary_graph(&id(iri))
                .expect("the vocabulary is created");
        }
        store
            .transaction(|txn| {
                txn.extend_graph(
                    &id(ENERGY),
                    &[Quad::new(
                        NamedNode::new_unchecked(EXISTING),
                        NamedNode::new_unchecked("http://www.w3.org/2004/02/skos/core#prefLabel"),
                        Literal::new_simple_literal("Solar power"),
                        NamedNode::new_unchecked(ENERGY),
                    )],
                )
            })
            .expect("the concept is written");
        (dir, store)
    }

    /// Record one, read it back whole. Every field survives the round trip.
    #[test]
    fn a_recorded_justification_is_what_comes_back() {
        let (_dir, store) = store();

        let written = store
            .record_justification(
                &id(ENERGY),
                SOLAR,
                "Solar power",
                &[EXISTING.to_owned()],
                "the existing one is a funding programme, not the technology",
                true,
                "ada",
            )
            .expect("a complete justification is recorded");

        let read = store.justifications().expect("readable");
        assert_eq!(read, vec![written.clone()]);

        let only = &read[0];
        assert_eq!(only.concept(), SOLAR);
        assert_eq!(only.graph(), &id(ENERGY));
        assert_eq!(only.label(), "Solar power");
        assert_eq!(only.considered(), [EXISTING.to_owned()]);
        assert_eq!(
            only.reason(),
            "the existing one is a funding programme, not the technology"
        );
        assert!(only.search_was_complete());
        assert_eq!(only.recorded_by(), "ada");
        assert!(
            RecordedAt::parse(only.recorded_at()).is_ok(),
            "the stamp is a value the trail can order: {}",
            only.recorded_at()
        );
    }

    /// The auditor's question is a query, which is the whole reason the considered resources are
    /// IRIs in the object position rather than prose. If this stops answering, the record has
    /// become a note with extra steps.
    #[test]
    fn which_concepts_were_created_despite_a_match_is_answerable_in_sparql() {
        let (_dir, store) = store();

        store
            .record_justification(
                &id(ENERGY),
                SOLAR,
                "Solar power",
                &[EXISTING.to_owned()],
                "a different sense of the term",
                true,
                "ada",
            )
            .expect("recorded");
        store
            .record_justification(
                &id(ENERGY),
                "https://example.org/energy/c_2",
                "Tidal power",
                &[],
                "nothing was found under this name",
                true,
                "ada",
            )
            .expect("recorded");

        // The join is the point, and it is why the resource passed over is an IRI in the object
        // position. `?passed` is used here as a *subject* against the vocabulary that holds it,
        // which is precisely what a record written as prose could never support — and precisely
        // what an auditor does when they ask what the thing we passed over actually was.
        let query = format!(
            "SELECT ?concept ?label FROM <{}> FROM <{ENERGY}> WHERE {{ \
             ?j <{CONCEPT_IRI}> ?concept ; <{CONSIDERED_IRI}> ?passed . \
             ?passed <http://www.w3.org/2004/02/skos/core#prefLabel> ?label }}",
            crate::SYSTEM_GRAPH_IRI
        );
        let mut written = Vec::new();
        let report = store
            .query(
                &query,
                crate::QueryFormats::default(),
                crate::QueryLimits::default(),
                &mut written,
            )
            .expect("the query runs");
        let answer = String::from_utf8_lossy(&written).into_owned();

        assert_eq!(
            report.answers(),
            1,
            "only the creation that passed something over is returned: {answer}"
        );
        assert!(answer.contains(SOLAR), "{answer}");
        assert!(
            answer.contains("Solar power"),
            "the resource passed over joins to what the vocabulary says about it, which prose \
             could not do: {answer}"
        );
    }

    /// Recording twice appends. A justification is a statement made at a time; overwriting the
    /// first would delete the evidence the record exists to keep.
    #[test]
    fn a_second_justification_for_one_concept_does_not_replace_the_first() {
        let (_dir, store) = store();

        for reason in [
            "a different sense",
            "and on reflection, still a different sense",
        ] {
            store
                .record_justification(&id(ENERGY), SOLAR, "Solar power", &[], reason, true, "ada")
                .expect("recorded");
        }

        let read = store.justifications().expect("readable");
        assert_eq!(read.len(), 2, "both statements are kept");
        assert_eq!(read[0].id(), JustificationId(1));
        assert_eq!(read[1].id(), JustificationId(2));
        assert_eq!(read[0].reason(), "a different sense");
        assert_eq!(
            read[1].reason(),
            "and on reflection, still a different sense"
        );
    }

    /// An empty considered list is a real answer — somebody looked and found nothing — and is not
    /// the same as no record at all.
    #[test]
    fn nothing_found_is_recorded_rather_than_refused() {
        let (_dir, store) = store();

        let written = store
            .record_justification(
                &id(ENERGY),
                SOLAR,
                "Solar power",
                &[],
                "nothing discovery reached is called this",
                true,
                "ada",
            )
            .expect("a justification with nothing considered is still a justification");

        assert!(written.considered().is_empty());
        assert_eq!(store.justifications().expect("readable").len(), 1);
    }

    /// A search that could not finish is weaker evidence, and the record says so rather than
    /// letting an auditor read diligence into a search that was cut short.
    #[test]
    fn an_incomplete_search_is_recorded_as_incomplete() {
        let (_dir, store) = store();

        store
            .record_justification(
                &id(ENERGY),
                SOLAR,
                "Solar power",
                &[],
                "nothing was found, but one source could not be reached",
                false,
                "ada",
            )
            .expect("recorded");

        let read = store.justifications().expect("readable");
        assert!(
            !read[0].search_was_complete(),
            "the record must not claim a search finished when it did not"
        );
    }

    /// Every refusal, and each one names what an auditor would have been unable to do.
    #[test]
    fn a_record_nobody_could_act_on_is_refused() {
        let (_dir, store) = store();

        /// One refusal to provoke: the arguments, and the word the message must contain.
        struct Case {
            concept: &'static str,
            label: &'static str,
            considered: Vec<String>,
            reason: &'static str,
            by: &'static str,
            wanted: &'static str,
        }
        let case = |concept, label, considered: &[&str], reason, by, wanted| Case {
            concept,
            label,
            considered: considered.iter().map(|one| (*one).to_owned()).collect(),
            reason,
            by,
            wanted,
        };

        let cases = [
            case(SOLAR, "Solar power", &[], "   ", "ada", "reason"),
            case(SOLAR, "Solar power", &[], "a reason", "  ", "who decided"),
            case(SOLAR, "   ", &[], "a reason", "ada", "label"),
            case("not an iri", "Solar power", &[], "a reason", "ada", "IRI"),
            case(
                SOLAR,
                "Solar power",
                &["a thing we saw"],
                "a reason",
                "ada",
                "IRI",
            ),
        ];

        for Case {
            concept,
            label,
            considered,
            reason,
            by,
            wanted,
        } in cases
        {
            let considered = considered.as_slice();
            let error = store
                .record_justification(&id(ENERGY), concept, label, considered, reason, true, by)
                .expect_err("refused");
            let detail = error.to_string();
            assert!(
                detail.contains(wanted),
                "the refusal must name what is missing; wanted {wanted:?} in {detail}"
            );
            assert!(
                !detail.contains("  "),
                "a refusal an operator reads must not carry a run of spaces: {detail}"
            );
        }

        assert!(
            store.justifications().expect("readable").is_empty(),
            "nothing refused reached the store"
        );
    }

    /// A justification for a graph that is not a vocabulary is a category error, and for one the
    /// store does not hold it is the absence that is the news.
    #[test]
    fn only_a_vocabulary_the_store_holds_can_be_justified_against() {
        let (_dir, store) = store();

        let error = store
            .record_justification(
                &GraphId::system(),
                SOLAR,
                "Solar power",
                &[],
                "a reason",
                true,
                "ada",
            )
            .expect_err("refused");
        assert!(
            error.to_string().contains("OpenBiz's own graphs"),
            "{error}"
        );

        let error = store
            .record_justification(
                &id("https://example.org/absent"),
                SOLAR,
                "Solar power",
                &[],
                "a reason",
                true,
                "ada",
            )
            .expect_err("refused");
        assert!(
            matches!(error, StoreError::NoSuchGraph { .. }),
            "a vocabulary the store does not hold is refused as absent: {error}"
        );
    }

    /// A record missing a required field is corrupt rather than a record with a default. Written
    /// by hand into the system graph, because nothing in this build can produce one.
    #[test]
    fn a_record_missing_a_field_is_corrupt() {
        let (_dir, store) = store();

        store
            .record_justification(
                &id(ENERGY),
                SOLAR,
                "Solar power",
                &[],
                "a reason",
                true,
                "ada",
            )
            .expect("recorded");

        let subject = JustificationId(1).subject();
        store
            .transaction(|txn| {
                txn.remove_graph_quads(
                    &GraphId::system(),
                    &[Quad::new(
                        subject.clone(),
                        named_node(REASON_IRI).into_owned(),
                        Literal::new_simple_literal("a reason"),
                        NamedNode::new_unchecked(GraphId::system().iri()),
                    )],
                )
            })
            .expect("the field is removed");

        let error = store.justifications().expect_err("refused");
        assert!(
            matches!(error, StoreError::Corrupt { .. }),
            "a justification with no reason is not evidence of anything: {error}"
        );
        assert!(
            error.to_string().contains(REASON_IRI),
            "the refusal names the missing field: {error}"
        );
    }

    /// The completeness flag is neither true nor false, which is exactly the case where reporting
    /// a default would turn the appearance of evidence into evidence.
    #[test]
    fn a_completeness_that_is_not_a_boolean_is_corrupt() {
        let (_dir, store) = store();

        store
            .record_justification(
                &id(ENERGY),
                SOLAR,
                "Solar power",
                &[],
                "a reason",
                true,
                "ada",
            )
            .expect("recorded");

        let subject = JustificationId(1).subject();
        let system = NamedNode::new_unchecked(GraphId::system().iri());
        store
            .transaction(|txn| {
                txn.remove_graph_quads(
                    &GraphId::system(),
                    &[Quad::new(
                        subject.clone(),
                        named_node(COMPLETE_IRI).into_owned(),
                        Literal::new_typed_literal("true", xsd::BOOLEAN),
                        system.clone(),
                    )],
                )?;
                txn.extend_graph(
                    &GraphId::system(),
                    &[Quad::new(
                        subject.clone(),
                        named_node(COMPLETE_IRI).into_owned(),
                        Literal::new_typed_literal("mostly", xsd::BOOLEAN),
                        system.clone(),
                    )],
                )
            })
            .expect("the field is replaced");

        let error = store.justifications().expect_err("refused");
        assert!(matches!(error, StoreError::Corrupt { .. }), "{error}");
        assert!(error.to_string().contains("mostly"), "{error}");
    }

    /// Records from two vocabularies come back together and in order, because proliferation is an
    /// organisation-wide fact rather than a per-vocabulary one.
    #[test]
    fn justifications_span_the_store_oldest_first() {
        let (_dir, store) = store();

        for (graph, concept) in [(ENERGY, SOLAR), (WATER, EXISTING), (ENERGY, SOLAR)] {
            store
                .record_justification(&id(graph), concept, "a label", &[], "a reason", true, "ada")
                .expect("recorded");
        }

        let read = store.justifications().expect("readable");
        assert_eq!(
            read.iter().map(|one| one.id().0).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "oldest first, and the ordinal is not restarted per vocabulary"
        );
        assert_eq!(
            read.iter()
                .map(|one| one.graph().iri().to_owned())
                .collect::<Vec<_>>(),
            vec![ENERGY, WATER, ENERGY]
        );
    }
}
