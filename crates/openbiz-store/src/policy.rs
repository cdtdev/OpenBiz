//! The IRI-minting policy a vocabulary mints under, recorded rather than inferred each time.
//!
//! # Why a policy is stored at all
//!
//! `openbiz mint` can read a pattern off a vocabulary's own concepts, and for one curator at one
//! command line that is better than a setting nobody ever checked. It is the wrong basis for a
//! deployment. Inference answers "what do most of this vocabulary's concepts look like *now*", so
//! a vocabulary whose first ten concepts arrived in one namespace and whose next ten arrive in
//! another changes its own convention part-way through, and every IRI minted after the tipping
//! point disagrees with every IRI minted before it. Nothing announces that; the IRIs are already
//! permanent by the time anybody notices.
//!
//! An import, a discovery match, and an agent proposal have to mint the *same* way as the curator,
//! and the only thing that makes that true is a policy written down in one place that all of them
//! read. That is what this module keeps.
//!
//! # Where it lives, and why that is not the vocabulary
//!
//! In the system graph, on the vocabulary's own registry subject — beside its `urn:openbiz:graphKind`
//! entry, which is the other fact OpenBiz records *about* a vocabulary rather than *in* it. Putting
//! it in the vocabulary would publish it: a `skos:ConceptScheme` exported to another tool would
//! carry an OpenBiz configuration statement that no standard defines and that the receiving tool
//! would either drop or preserve as noise. The policy is ours, so it goes in our graph.
//!
//! That placement has a consequence worth stating plainly: a whole-store backup carries the policy,
//! and an export of a single vocabulary does not. Round-tripping a vocabulary through Turtle and
//! back therefore loses it, which is recorded in `docs/UNTESTED.md`.
//!
//! # What this module does not do
//!
//! It does not know what a pattern *means*. `MintPattern` lives in `openbiz-skos`, which is
//! engine-free and has no dependency on a store, and this crate has none on it — so the pattern
//! arrives here as text that has already been parsed by the caller, and leaves as text the caller
//! parses again. The store refuses an empty pattern and an unattributed one, and takes the caller's
//! word for the rest. `openbiz-server` is where the two meet, and it is where a pattern is validated
//! before it is ever recorded.

use oxigraph::model::{Literal, NamedNode, Quad, Term};
use oxsdatatypes::DateTime;

use crate::{named_node, GraphId, GraphKind, RegistryReader, Store, StoreError};

/// The pattern a vocabulary's new concepts are minted under.
const IRI_PATTERN_IRI: &str = "urn:openbiz:iriPattern";

/// Who recorded that pattern.
const IRI_PATTERN_BY_IRI: &str = "urn:openbiz:iriPatternRecordedBy";

/// When they recorded it.
const IRI_PATTERN_AT_IRI: &str = "urn:openbiz:iriPatternRecordedAt";

/// A recorded decision about how one vocabulary mints the IRIs of its new concepts.
///
/// Carries its attribution because it is a governance decision and not a preference: the IRIs it
/// produces are the one thing about a concept that can never be corrected, so "who decided this,
/// and when" is a question an auditor will ask about the pattern and not only about the concepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IriPolicy {
    pattern: String,
    recorded_by: String,
    recorded_at: String,
}

impl IriPolicy {
    /// The pattern itself, as it was recorded.
    ///
    /// Text, not a parsed pattern: see the module documentation for why the store does not know
    /// what one means. A caller that is going to mint with it must parse it, and must be ready for
    /// it to fail — a pattern written by an older or newer build is exactly the case where a
    /// silent fallback to inference would mint into the wrong namespace and call it normal.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    /// Who recorded it, named the way an auditor would want it named.
    pub fn recorded_by(&self) -> &str {
        &self.recorded_by
    }

    /// When it was recorded, as an `xsd:dateTime`.
    pub fn recorded_at(&self) -> &str {
        &self.recorded_at
    }
}

/// What recording a policy did, including what it displaced.
///
/// `replaced` is the only moment the previous policy is visible: recording a new one overwrites it,
/// so this is handed back for the caller to *tell somebody*. A convention that changed with nobody
/// told is how a vocabulary ends up with two generations of IRI and no record of the decision that
/// divided them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRecorded {
    /// The policy now in force.
    pub policy: IriPolicy,
    /// The policy this one displaced, if the vocabulary already had one.
    pub replaced: Option<IriPolicy>,
}

impl Store {
    /// The IRI-minting policy recorded for `graph`, or `None` if nobody has recorded one.
    ///
    /// `None` is a real answer and not a defect: a vocabulary with no recorded policy is every
    /// vocabulary in every store written before this existed, and the caller's fallback — inferring
    /// from the vocabulary's own concepts — is what `openbiz mint` did before and still does.
    ///
    /// Refuses an IRI that is not a vocabulary graph, and one the registry does not know about.
    /// Answering `None` for a vocabulary that does not exist would tell a caller "no policy is
    /// recorded" about a graph whose absence is the actual news.
    pub fn iri_policy(&self, graph: &GraphId) -> Result<Option<IriPolicy>, StoreError> {
        only_a_vocabulary(graph)?;
        if !self.contains_graph(graph.iri())? {
            return Err(StoreError::NoSuchGraph {
                iri: graph.iri().to_owned(),
            });
        }
        read_policy(&self.backend, graph, self.path())
    }

    /// Record the pattern `graph` mints under, replacing any pattern already recorded for it.
    ///
    /// The pattern is **not** validated here — see the module documentation. What is refused is a
    /// record no auditor could use: an empty pattern, and an empty `recorded_by`. The attribution
    /// rule is the same one [`Store::decide`] applies to an approval, for the same reason.
    ///
    /// This writes to the system graph and never to the vocabulary, so it is not a change to a
    /// vocabulary and does not go through the candidate seam (`CLAUDE.md` §3). No statement about
    /// any concept changes, and no IRI already minted is affected: a policy governs the *next*
    /// mint. The IRIs already in the vocabulary are the record of what the previous policy was, and
    /// they stay exactly as they are.
    ///
    /// The read and the write are one transaction, so two operators recording a policy at the same
    /// moment cannot interleave into a record that names one operator's pattern and the other's
    /// attribution.
    pub fn record_iri_policy(
        &self,
        graph: &GraphId,
        pattern: &str,
        recorded_by: &str,
    ) -> Result<PolicyRecorded, StoreError> {
        only_a_vocabulary(graph)?;

        if pattern.trim().is_empty() {
            return Err(StoreError::PolicyRejected {
                detail: "it records no pattern, and a vocabulary whose recorded policy is blank \
                         is one that falls back to inferring a pattern while looking as though \
                         somebody had decided"
                    .to_owned(),
            });
        }
        if recorded_by.trim().is_empty() {
            return Err(StoreError::PolicyRejected {
                detail: "it does not say who recorded it, and the pattern a vocabulary mints \
                         under is a decision somebody is answerable for"
                    .to_owned(),
            });
        }

        self.transaction(|txn| {
            if !txn.contains_graph(graph.iri())? {
                return Err(StoreError::NoSuchGraph {
                    iri: graph.iri().to_owned(),
                });
            }

            let replaced = read_policy(&txn.inner, graph, self.path())?;
            if let Some(previous) = &replaced {
                txn.remove_graph_quads(&GraphId::system(), &quads_of(graph, previous))?;
            }

            let policy = IriPolicy {
                pattern: pattern.to_owned(),
                recorded_by: recorded_by.to_owned(),
                recorded_at: DateTime::now().to_string(),
            };
            txn.extend_graph(&GraphId::system(), &quads_of(graph, &policy))?;

            Ok(PolicyRecorded { policy, replaced })
        })
    }
}

/// Refuse a graph that is not a vocabulary.
///
/// OpenBiz's own graphs are not authored (`GraphId::is_directly_writable` is a different question —
/// the system graph *is* writable, by us) and nothing mints concepts into them, so a minting policy
/// for one is a category error rather than a permission problem, and it is refused as one.
fn only_a_vocabulary(graph: &GraphId) -> Result<(), StoreError> {
    if graph.kind() == GraphKind::Vocabulary {
        return Ok(());
    }
    Err(StoreError::PolicyRejected {
        detail: format!(
            "{} is a {} graph, and a minting policy belongs to a vocabulary; nothing mints \
             concepts into OpenBiz's own graphs",
            graph.iri(),
            graph.kind()
        ),
    })
}

/// The three quads a policy is stored as.
///
/// One function, used by both the write and the retraction of the previous record, so a policy can
/// never be half-removed by the two disagreeing about its shape.
fn quads_of(graph: &GraphId, policy: &IriPolicy) -> Vec<Quad> {
    let subject = NamedNode::new_unchecked(graph.iri());
    let system = NamedNode::new_unchecked(GraphId::system().iri());
    [
        (IRI_PATTERN_IRI, policy.pattern.as_str()),
        (IRI_PATTERN_BY_IRI, policy.recorded_by.as_str()),
        (IRI_PATTERN_AT_IRI, policy.recorded_at.as_str()),
    ]
    .into_iter()
    .map(|(predicate, value)| {
        Quad::new(
            subject.clone(),
            named_node(predicate).into_owned(),
            Literal::new_simple_literal(value),
            system.clone(),
        )
    })
    .collect()
}

/// Read one vocabulary's recorded policy out of the registry.
///
/// Two things are refused rather than papered over. **A second pattern** for one vocabulary is
/// [`StoreError::Corrupt`]: guessing which of two recorded conventions is in force is how a
/// vocabulary acquires IRIs from both. And **a pattern with no attribution** is corrupt too, for a
/// governance reason rather than a technical one — a policy nobody is recorded as having set is a
/// policy that arrived from somewhere this build cannot account for, and reporting it as though it
/// were a decision would put OpenBiz's name to an unsigned one.
fn read_policy(
    source: &impl RegistryReader,
    graph: &GraphId,
    path: &std::path::Path,
) -> Result<Option<IriPolicy>, StoreError> {
    let subject = named_node(graph.iri());
    let one = |predicate: &str, what: &str| -> Result<Option<String>, StoreError> {
        let found: Vec<Term> = source
            .system_quads(Some(subject), named_node(predicate))?
            .into_iter()
            .map(|quad| quad.object)
            .collect();
        match found.as_slice() {
            [] => Ok(None),
            [Term::Literal(literal)] => Ok(Some(literal.value().to_owned())),
            other => Err(StoreError::Corrupt {
                path: path.to_path_buf(),
                detail: format!(
                    "the vocabulary {} records {} {what}s for minting IRIs, and which of them is \
                     in force is not something this build may guess at",
                    graph.iri(),
                    other.len()
                ),
            }),
        }
    };

    let Some(pattern) = one(IRI_PATTERN_IRI, "pattern")? else {
        return Ok(None);
    };
    let recorded_by = one(IRI_PATTERN_BY_IRI, "recording actor")?;
    let recorded_at = one(IRI_PATTERN_AT_IRI, "recording time")?;

    match (recorded_by, recorded_at) {
        (Some(recorded_by), Some(recorded_at)) => Ok(Some(IriPolicy {
            pattern,
            recorded_by,
            recorded_at,
        })),
        _ => Err(StoreError::Corrupt {
            path: path.to_path_buf(),
            detail: format!(
                "the vocabulary {} records the pattern {pattern:?} for minting IRIs with no \
                 record of who recorded it or when, and an unattributed policy is not one this \
                 build will report as a decision",
                graph.iri()
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENERGY: &str = "https://example.org/energy";
    const WATER: &str = "https://example.org/water";

    /// The vocabulary IRI as the API takes it.
    fn id(iri: &str) -> GraphId {
        GraphId::vocabulary(iri).expect("a vocabulary IRI")
    }

    /// A store holding two registered, empty vocabularies.
    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path()).expect("a fresh store opens");
        for iri in [ENERGY, WATER] {
            store
                .create_vocabulary_graph(&id(iri))
                .expect("the vocabulary is created");
        }
        (dir, store)
    }

    /// The plain case: nothing recorded, then something recorded, then read back.
    #[test]
    fn a_recorded_pattern_is_what_comes_back() {
        let (_dir, store) = store();

        assert_eq!(
            store.iri_policy(&id(ENERGY)).expect("readable"),
            None,
            "a vocabulary nobody has decided about records nothing"
        );

        let recorded = store
            .record_iri_policy(
                &id(ENERGY),
                "https://example.org/energy/c_{n}",
                "ada@example.org",
            )
            .expect("the policy is recorded");
        assert_eq!(recorded.replaced, None);
        assert_eq!(
            recorded.policy.pattern(),
            "https://example.org/energy/c_{n}"
        );
        assert_eq!(recorded.policy.recorded_by(), "ada@example.org");

        let read = store
            .iri_policy(&id(ENERGY))
            .expect("readable")
            .expect("a policy");
        assert_eq!(read, recorded.policy, "what was read is what was written");
        assert!(
            !read.recorded_at().is_empty(),
            "a policy records when it was set: {read:?}"
        );
    }

    /// The whole point of the item: it survives the process that wrote it.
    #[test]
    fn a_recorded_pattern_survives_reopening_the_store() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        {
            let store = Store::open(dir.path()).expect("a fresh store opens");
            store
                .create_vocabulary_graph(&id(ENERGY))
                .expect("the vocabulary is created");
            store
                .record_iri_policy(&id(ENERGY), "https://example.org/energy/{slug}", "ada")
                .expect("the policy is recorded");
            store.close().expect("the store closes cleanly");
        }

        let store = Store::open(dir.path()).expect("the store reopens");
        let read = store
            .iri_policy(&id(ENERGY))
            .expect("readable")
            .expect("a policy");
        assert_eq!(read.pattern(), "https://example.org/energy/{slug}");
        assert_eq!(read.recorded_by(), "ada");
    }

    /// Replacing one, which is how a convention is changed on purpose.
    #[test]
    fn recording_a_second_pattern_replaces_the_first_and_says_what_it_replaced() {
        let (_dir, store) = store();
        store
            .record_iri_policy(&id(ENERGY), "https://example.org/energy/c_{n}", "ada")
            .expect("the first policy is recorded");

        let recorded = store
            .record_iri_policy(&id(ENERGY), "https://example.org/energy/{slug}", "bob")
            .expect("the second policy is recorded");

        let replaced = recorded.replaced.expect("the first policy is handed back");
        assert_eq!(replaced.pattern(), "https://example.org/energy/c_{n}");
        assert_eq!(replaced.recorded_by(), "ada");

        let read = store
            .iri_policy(&id(ENERGY))
            .expect("readable")
            .expect("a policy");
        assert_eq!(
            read.pattern(),
            "https://example.org/energy/{slug}",
            "the second pattern is the one in force"
        );
        assert_eq!(read.recorded_by(), "bob");
    }

    /// Replacing a policy must retract the *old* quads, not merely add new ones. If it does not,
    /// the next read finds two patterns and refuses the whole record as corrupt — which is the
    /// right refusal and the wrong situation, and it would be caused by us.
    #[test]
    fn replacing_a_pattern_leaves_exactly_one_recorded() {
        let (_dir, store) = store();
        for pattern in [
            "https://example.org/energy/a_{n}",
            "https://example.org/energy/b_{n}",
        ] {
            store
                .record_iri_policy(&id(ENERGY), pattern, "ada")
                .expect("the policy is recorded");
        }

        let patterns = store
            .backend
            .system_quads(Some(named_node(ENERGY)), named_node(IRI_PATTERN_IRI))
            .expect("the system graph is readable")
            .len();
        assert_eq!(patterns, 1, "one vocabulary records one pattern");
    }

    /// One vocabulary's decision is not another's.
    #[test]
    fn a_policy_belongs_to_the_vocabulary_it_was_recorded_for() {
        let (_dir, store) = store();
        store
            .record_iri_policy(&id(ENERGY), "https://example.org/energy/c_{n}", "ada")
            .expect("the policy is recorded");

        assert_eq!(
            store.iri_policy(&id(WATER)).expect("readable"),
            None,
            "the other vocabulary was not decided about"
        );
    }

    /// A policy is a fact about a vocabulary and is kept out of it.
    #[test]
    fn recording_a_policy_writes_nothing_into_the_vocabulary() {
        let (_dir, store) = store();
        store
            .record_iri_policy(&id(ENERGY), "https://example.org/energy/c_{n}", "ada")
            .expect("the policy is recorded");

        assert_eq!(
            store.for_each_statement(ENERGY, |_| {}).expect("readable"),
            0,
            "the vocabulary itself is untouched"
        );
    }

    /// The registry must still be readable afterwards. The policy hangs off the same subject as the
    /// vocabulary's `graphKind` entry, and a registry read that did not expect a neighbour would
    /// take the whole vocabulary list down (see `Transaction::create_vocabulary_graph`).
    #[test]
    fn the_registry_still_lists_every_graph_after_a_policy_is_recorded() {
        let (_dir, store) = store();
        let before = store.graphs().expect("the registry is readable");
        store
            .record_iri_policy(&id(ENERGY), "https://example.org/energy/c_{n}", "ada")
            .expect("the policy is recorded");

        assert_eq!(
            store.graphs().expect("the registry is still readable"),
            before,
            "recording a policy changes what a vocabulary is minted under, not what exists"
        );
    }

    /// A vocabulary that does not exist is news, not an absent policy.
    #[test]
    fn a_policy_cannot_be_read_or_recorded_for_a_graph_that_is_not_there() {
        let (_dir, store) = store();
        let missing = "https://example.org/nothing";

        assert!(
            matches!(
                store.iri_policy(&id(missing)),
                Err(StoreError::NoSuchGraph { iri }) if iri == missing
            ),
            "reading a policy for an unregistered graph is refused"
        );
        assert!(
            matches!(
                store.record_iri_policy(&id(missing), "https://example.org/nothing/c_{n}", "ada"),
                Err(StoreError::NoSuchGraph { iri }) if iri == missing
            ),
            "recording one is refused too"
        );
    }

    /// OpenBiz's own graphs are not authored and are not minted into either.
    #[test]
    fn openbiz_own_graphs_have_no_minting_policy() {
        let (_dir, store) = store();

        assert!(
            matches!(
                store.iri_policy(&GraphId::system()),
                Err(StoreError::PolicyRejected { .. })
            ),
            "the system graph is not a vocabulary"
        );
        assert!(
            matches!(
                store.record_iri_policy(&GraphId::system(), "urn:openbiz:c_{n}", "ada"),
                Err(StoreError::PolicyRejected { .. })
            ),
            "and nothing may record a minting policy for it"
        );
    }

    /// Both halves of a record a reviewer could not act on.
    #[test]
    fn a_record_nobody_could_audit_is_refused() {
        let (_dir, store) = store();

        assert!(
            matches!(
                store.record_iri_policy(&id(ENERGY), "   ", "ada"),
                Err(StoreError::PolicyRejected { .. })
            ),
            "a blank pattern is not a decision"
        );
        assert!(
            matches!(
                store.record_iri_policy(&id(ENERGY), "https://example.org/energy/c_{n}", "  "),
                Err(StoreError::PolicyRejected { .. })
            ),
            "and neither is an unattributed one"
        );
        assert_eq!(
            store.iri_policy(&id(ENERGY)).expect("readable"),
            None,
            "a refused recording leaves the vocabulary as it was"
        );
    }

    /// The store takes the caller's word on syntax, and this pins that it does — so the day the
    /// server stops validating, the test that fails is one about the server and not this one.
    #[test]
    fn the_store_does_not_judge_the_pattern_itself() {
        let (_dir, store) = store();

        store
            .record_iri_policy(&id(ENERGY), "not a pattern at all", "ada")
            .expect("the store records what it is given");
        assert_eq!(
            store
                .iri_policy(&id(ENERGY))
                .expect("readable")
                .expect("a policy")
                .pattern(),
            "not a pattern at all",
            "and hands it back for the caller to refuse"
        );
    }
}
