//! The named-graph model: what a graph is, what it may hold, and who may write to it.
//!
//! Every quad in an OpenBiz store lives in exactly one named graph. Nothing is written to the
//! default graph, ever — a quad with no graph cannot be exported, versioned, permissioned, or
//! attributed, and "which vocabulary is this statement part of?" is the first question every
//! later phase asks.
//!
//! Three kinds, and the distinction between them is load-bearing rather than descriptive:
//!
//! - A **vocabulary** graph is one user-authored artefact. One graph per vocabulary is what makes
//!   a vocabulary a unit you can export whole, diff against its previous version, hand to a
//!   reviewer, or delete without collateral damage.
//! - The **system** graph is OpenBiz's own bookkeeping — the store's format stamp, the graph
//!   registry, and later the workflow and provenance records. It is kept apart so our metadata
//!   never leaks into a customer's exported vocabulary, which is the failure that makes an export
//!   un-round-trippable through a standards-compliant tool (`CLAUDE.md` §1.3).
//! - An **inferred** graph holds materialised entailments. It is derived, never asserted, so
//!   application code may not write into it — see [`GraphId::is_directly_writable`]. Losing the
//!   asserted/inferred distinction is how a governance tool ends up unable to answer "why?"
//!   (`CLAUDE.md` §3, explainability).
//! - A **candidate** graph holds the statements of one *proposed* change, staged where they are
//!   visible and reviewable but are not yet part of anybody's vocabulary. It is the named-graph
//!   half of the candidate seam (`CLAUDE.md` §3): a change arrives as a proposal, a human sees
//!   exactly what it would do, and only approval moves the statements into the vocabulary. It is
//!   ours — it lives under [`OPENBIZ_NAMESPACE`], so a user cannot author into it directly — but
//!   unlike an inferred graph it *is* written by application code, because staging a proposal is
//!   the whole point of it. A candidate has **two** of them, one for what it would add and one
//!   for what it would take away — see [`CandidatePart`].
//!
//! # The reserved namespace
//!
//! Everything OpenBiz mints for itself lives under [`OPENBIZ_NAMESPACE`], and a vocabulary graph
//! may not. Without that rule a user could register a graph at the system graph's own IRI — or at
//! the IRI of an inferred graph — and quietly acquire write access to our bookkeeping through a
//! path that looks like ordinary authoring. The check is cheap, so it is unconditional.

use oxigraph::model::NamedNode;
use thiserror::Error;

/// Prefix under which OpenBiz mints IRIs for its own use.
///
/// A `urn:` IRI, not an `http:` one: we do not own a domain, and minting an IRI under someone
/// else's namespace — or one that 404s — is worse than being honestly non-dereferenceable.
pub const OPENBIZ_NAMESPACE: &str = "urn:openbiz:";

/// Named graph holding OpenBiz's own metadata.
pub const SYSTEM_GRAPH_IRI: &str = "urn:openbiz:graph:system";

/// Prefix under which the staging graph of a proposed change is minted.
///
/// The full IRI is this prefix followed by the candidate's identifier, so the graph a proposal is
/// staged in is derived from the proposal rather than chosen by whoever raised it. Inside
/// [`OPENBIZ_NAMESPACE`] because a staging graph is OpenBiz's own: a user must not be able to
/// register a vocabulary at the IRI where a pending change is waiting to be reviewed.
pub const CANDIDATE_GRAPH_PREFIX: &str = "urn:openbiz:graph:candidate:";

/// Suffix distinguishing a candidate's removal staging graph from its addition one.
///
/// Both halves sit under [`CANDIDATE_GRAPH_PREFIX`] rather than under two prefixes, so the rule
/// "anything under this prefix is a staging graph" stays a single string comparison — the one
/// [`GraphId::classify`] applies to every quad of a restore.
pub const CANDIDATE_REMOVALS_SUFFIX: &str = ":removals";

/// Which half of a candidate's payload a staging graph holds.
///
/// A proposal that can only add is a proposal that cannot express a merge, a split, a move, or a
/// deprecation — every one of those is "these statements go, those arrive". Keeping the two halves
/// in *separate graphs* rather than in one graph with a marker on each statement means a reviewer
/// can export either half on its own, and the applying transaction never has to work out which of
/// two intentions a statement carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CandidatePart {
    /// Statements the candidate proposes to add to its target.
    Additions,
    /// Statements the candidate proposes to remove from its target.
    Removals,
}

impl CandidatePart {
    /// What this half's graph IRI carries after the candidate's identifier.
    ///
    /// Additions have no suffix, so the IRI a store already holds for an additions-only candidate
    /// is unchanged by removals arriving. That is not laziness about naming: rewriting the graph
    /// IRIs of proposals a customer has already reviewed would rewrite their audit trail.
    const fn suffix(self) -> &'static str {
        match self {
            Self::Additions => "",
            Self::Removals => CANDIDATE_REMOVALS_SUFFIX,
        }
    }
}

impl std::fmt::Display for CandidatePart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Additions => "additions",
            Self::Removals => "removals",
        })
    }
}

/// Prefix for the graph holding a vocabulary's materialised entailments.
///
/// The full IRI is this prefix followed by the vocabulary's own IRI, so two vocabularies can never
/// share an inferred graph and the derivation is readable straight off the IRI. It sits inside
/// [`OPENBIZ_NAMESPACE`] because an inferred graph is *ours*: we materialise it, we invalidate it,
/// and a user must not be able to author into it.
pub const INFERRED_GRAPH_PREFIX: &str = "urn:openbiz:graph:inferred:";

/// How a named graph is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GraphKind {
    /// A user-authored vocabulary.
    Vocabulary,
    /// OpenBiz's own metadata: the graph registry, workflow state, provenance, configuration.
    System,
    /// Materialised inferences, kept separate so they are never confused with asserted facts.
    Inferred,
    /// The staged statements of one proposed change, kept out of every vocabulary until approved.
    Candidate,
}

impl GraphKind {
    /// The token written to the registry.
    ///
    /// Stable on-disk vocabulary: changing one of these strings is a store format change and needs
    /// a migration, not an edit.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vocabulary => "vocabulary",
            Self::System => "system",
            Self::Inferred => "inferred",
            Self::Candidate => "candidate",
        }
    }

    /// Read a token back from the registry.
    ///
    /// Returns `None` rather than a default for anything unrecognised. A store written by a build
    /// that knew a fourth kind must be refused, not silently downgraded to "vocabulary" — that is
    /// the same class of mistake as misreading a format version.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "vocabulary" => Some(Self::Vocabulary),
            "system" => Some(Self::System),
            "inferred" => Some(Self::Inferred),
            "candidate" => Some(Self::Candidate),
            _ => None,
        }
    }
}

impl std::fmt::Display for GraphKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why an IRI could not name a graph.
///
/// Deliberately not a store error: constructing a [`GraphId`] touches no store, and a caller
/// validating user input should not have to handle `AlreadyInUse` to do it.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum GraphIdError {
    /// No IRI was given.
    #[error("a graph IRI is required")]
    Empty,
    /// The IRI is not a syntactically valid absolute IRI.
    #[error("{iri:?} is not a valid absolute IRI: {detail}")]
    Invalid {
        /// What was offered.
        iri: String,
        /// The parser's complaint, in its own words.
        detail: String,
    },
    /// The IRI is inside the namespace OpenBiz reserves for itself.
    #[error(
        "{iri:?} is inside the {OPENBIZ_NAMESPACE} namespace, which OpenBiz reserves for its own \
         metadata; choose an IRI in a namespace you control"
    )]
    Reserved {
        /// What was offered.
        iri: String,
    },
    /// The IRI is under [`CANDIDATE_GRAPH_PREFIX`] but does not name a candidate we could read.
    #[error("{iri:?} is not the staging graph of any candidate this build can identify: {detail}")]
    NotACandidateGraph {
        /// What was offered.
        iri: String,
        /// Why the identifier after the prefix was not one.
        detail: String,
    },
    /// A derived graph was asked for from something that is not a vocabulary.
    #[error("{iri:?} is a {kind} graph; only a vocabulary graph has inferences derived from it")]
    NotAVocabulary {
        /// What was offered.
        iri: String,
        /// The kind it actually had.
        kind: GraphKind,
    },
}

/// Identifies a named graph.
///
/// The fields are private because the pairing of IRI and kind is an invariant, not two
/// independent values: a `System` graph is *only* [`SYSTEM_GRAPH_IRI`], an `Inferred` graph is
/// *only* something under [`INFERRED_GRAPH_PREFIX`], and a `Vocabulary` graph is *never* either.
/// A public struct literal would let any caller — including one reading a tampered registry —
/// assemble a combination the store then trusts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GraphId {
    iri: String,
    kind: GraphKind,
}

impl GraphId {
    /// A vocabulary graph at `iri`.
    ///
    /// The IRI must be a valid absolute IRI outside [`OPENBIZ_NAMESPACE`]. Validating here rather
    /// than at write time means a bad IRI is rejected while the user still has the form open,
    /// instead of surfacing as a backend error three layers down.
    pub fn vocabulary(iri: impl Into<String>) -> Result<Self, GraphIdError> {
        let iri = iri.into();
        validate_iri(&iri)?;
        if iri.starts_with(OPENBIZ_NAMESPACE) {
            return Err(GraphIdError::Reserved { iri });
        }
        Ok(Self {
            iri,
            kind: GraphKind::Vocabulary,
        })
    }

    /// The system graph. There is exactly one, and this is it.
    pub fn system() -> Self {
        Self {
            iri: SYSTEM_GRAPH_IRI.to_owned(),
            kind: GraphKind::System,
        }
    }

    /// The graph in which candidate `id`'s proposed statements are staged.
    ///
    /// Derived rather than chosen, for the same reason [`GraphId::inferred_for`] is: two
    /// proposals must not be able to share a staging graph, and nobody may aim a proposal at a
    /// graph a human authored.
    ///
    /// Crate-private because there is no legitimate reason for a caller outside this crate to
    /// mint one. A staging graph exists because a candidate exists, so the way to name one is to
    /// ask a [`crate::Candidate`] for it.
    pub(crate) fn candidate(id: &crate::CandidateId, part: CandidatePart) -> Self {
        Self {
            // The prefix is a valid `urn:` IRI, a `CandidateId` is a decimal number, and the
            // suffix is a compile-time constant, so the concatenation always parses.
            // `from_registry` re-validates it on the way back in, which is where an IRI that came
            // off disk is judged.
            iri: format!("{CANDIDATE_GRAPH_PREFIX}{id}{}", part.suffix()),
            kind: GraphKind::Candidate,
        }
    }

    /// The graph holding `vocabulary`'s materialised entailments.
    ///
    /// Derived rather than chosen, so a caller cannot point two vocabularies at one inferred graph
    /// and cannot aim materialisation at a graph a human authored.
    pub fn inferred_for(vocabulary: &GraphId) -> Result<Self, GraphIdError> {
        if vocabulary.kind != GraphKind::Vocabulary {
            return Err(GraphIdError::NotAVocabulary {
                iri: vocabulary.iri.clone(),
                kind: vocabulary.kind,
            });
        }
        let iri = format!("{INFERRED_GRAPH_PREFIX}{}", vocabulary.iri);
        // The prefix is a valid `urn:` IRI and the suffix is an already-validated absolute IRI, so
        // the concatenation parses — but assert it rather than assuming it, because the day that
        // stops being true is the day we would otherwise write an unparseable IRI to disk.
        validate_iri(&iri)?;
        Ok(Self {
            iri,
            kind: GraphKind::Inferred,
        })
    }

    /// Rebuild a graph identifier read back from the registry.
    ///
    /// Applies exactly the same invariants as the public constructors, because the registry is
    /// data on disk: a store that has been edited by hand, restored from a doctored backup, or
    /// written by a build with a bug must be refused rather than trusted.
    pub(crate) fn from_registry(iri: String, kind: GraphKind) -> Result<Self, GraphIdError> {
        match kind {
            GraphKind::Vocabulary => Self::vocabulary(iri),
            GraphKind::System => {
                if iri == SYSTEM_GRAPH_IRI {
                    Ok(Self::system())
                } else {
                    Err(GraphIdError::Reserved { iri })
                }
            }
            GraphKind::Inferred => {
                validate_iri(&iri)?;
                if iri.starts_with(INFERRED_GRAPH_PREFIX) {
                    Ok(Self {
                        iri,
                        kind: GraphKind::Inferred,
                    })
                } else {
                    Err(GraphIdError::Reserved { iri })
                }
            }
            GraphKind::Candidate => {
                validate_iri(&iri)?;
                let Some(rest) = iri.strip_prefix(CANDIDATE_GRAPH_PREFIX) else {
                    return Err(GraphIdError::Reserved { iri });
                };
                // Both halves of a candidate's payload live under the one prefix, so the text
                // after it is the identifier with an optional part suffix. Anything else is a
                // graph named as if it were ours and is refused rather than adopted.
                let id = rest.strip_suffix(CANDIDATE_REMOVALS_SUFFIX).unwrap_or(rest);
                // The identifier is re-parsed rather than accepted as text, because it is the
                // link back to the candidate's record in the system graph. A staging graph whose
                // identifier names no candidate is a graph the store cannot explain, and the
                // registry is data on disk that a doctored backup can reach.
                crate::CandidateId::parse(id).map_err(|error| {
                    GraphIdError::NotACandidateGraph {
                        detail: error.to_string(),
                        iri: iri.clone(),
                    }
                })?;
                Ok(Self {
                    iri,
                    kind: GraphKind::Candidate,
                })
            }
        }
    }

    /// Work out what kind of graph an IRI names, from the IRI alone.
    ///
    /// The registry says what kind a graph *was registered as*; this says what kind an IRI *may*
    /// be. They are different questions and the restore path needs the second one: a backup
    /// arrives as a stream of quads whose graph names have to be judged before anything has been
    /// written, and long before the registry inside that same file has been read back.
    ///
    /// The classification is total and follows the same invariants as the constructors —
    /// [`SYSTEM_GRAPH_IRI`] is the system graph, anything under [`INFERRED_GRAPH_PREFIX`] is
    /// inferred, anything else is a vocabulary — so an IRI that is inside
    /// [`OPENBIZ_NAMESPACE`] but is neither of the two forms we define is
    /// [`GraphIdError::Reserved`] rather than being quietly adopted as a vocabulary. A file that
    /// invents `urn:openbiz:graph:whatever` is a file written against a different build, and
    /// guessing what it meant is how a restore silently produces a store nobody can explain.
    pub(crate) fn classify(iri: &str) -> Result<Self, GraphIdError> {
        if iri == SYSTEM_GRAPH_IRI {
            return Ok(Self::system());
        }
        if iri.starts_with(INFERRED_GRAPH_PREFIX) {
            return Self::from_registry(iri.to_owned(), GraphKind::Inferred);
        }
        if iri.starts_with(CANDIDATE_GRAPH_PREFIX) {
            return Self::from_registry(iri.to_owned(), GraphKind::Candidate);
        }
        Self::vocabulary(iri.to_owned())
    }

    /// The graph's IRI.
    pub fn iri(&self) -> &str {
        &self.iri
    }

    /// What the graph holds.
    pub fn kind(&self) -> GraphKind {
        self.kind
    }

    /// Whether callers may write to this graph directly.
    ///
    /// Inferred graphs are written only by a reasoner-driven materialisation pass; letting
    /// application code assert into them would destroy the asserted-versus-inferred distinction
    /// the UI and every "why?" explanation depend on.
    ///
    /// A **candidate** graph is directly writable and that is deliberate rather than an
    /// oversight of the same rule: staging a proposal *is* an application-code write. What keeps
    /// it safe is not this flag but [`OPENBIZ_NAMESPACE`] — a user cannot register a vocabulary
    /// at a staging graph's IRI, so the only writer is the seam that created it.
    pub fn is_directly_writable(&self) -> bool {
        !matches!(self.kind, GraphKind::Inferred)
    }
}

impl std::fmt::Display for GraphId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.iri)
    }
}

/// Reject anything that is not a syntactically valid absolute IRI.
///
/// Delegated to the backend's parser so what we accept is exactly what the store, the
/// serialisers, and SPARQL will accept — a second, hand-rolled notion of validity would drift and
/// the drift would only show up as an export that will not re-import. The error is flattened to a
/// string so no `oxigraph::` type reaches our public API (`CLAUDE.md` §3).
fn validate_iri(iri: &str) -> Result<(), GraphIdError> {
    if iri.is_empty() {
        return Err(GraphIdError::Empty);
    }
    NamedNode::new(iri)
        .map(|_| ())
        .map_err(|error| GraphIdError::Invalid {
            iri: iri.to_owned(),
            detail: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vocabulary_graph_is_directly_writable() {
        let graph = GraphId::vocabulary("http://example.org/v/1").expect("a valid absolute IRI");
        assert_eq!(graph.kind(), GraphKind::Vocabulary);
        assert!(graph.is_directly_writable());
    }

    #[test]
    fn the_system_graph_is_directly_writable() {
        assert!(GraphId::system().is_directly_writable());
        assert_eq!(GraphId::system().iri(), SYSTEM_GRAPH_IRI);
    }

    #[test]
    fn inferred_graphs_are_not_directly_writable() {
        let vocabulary = GraphId::vocabulary("http://example.org/v/1").expect("valid");
        let inferred = GraphId::inferred_for(&vocabulary).expect("derivable from a vocabulary");

        assert_eq!(inferred.kind(), GraphKind::Inferred);
        assert!(
            !inferred.is_directly_writable(),
            "only materialisation may write inferred graphs"
        );
    }

    #[test]
    fn an_inferred_graph_names_the_vocabulary_it_was_derived_from() {
        let vocabulary = GraphId::vocabulary("http://example.org/v/1").expect("valid");
        let inferred = GraphId::inferred_for(&vocabulary).expect("derivable");

        assert_eq!(
            inferred.iri(),
            "urn:openbiz:graph:inferred:http://example.org/v/1"
        );
        assert!(inferred.iri().starts_with(OPENBIZ_NAMESPACE));
    }

    /// Two vocabularies must never share an inferred graph; if they did, materialising one would
    /// silently publish entailments under the other's name.
    #[test]
    fn distinct_vocabularies_get_distinct_inferred_graphs() {
        let first = GraphId::vocabulary("http://example.org/v/1").expect("valid");
        let second = GraphId::vocabulary("http://example.org/v/2").expect("valid");

        assert_ne!(
            GraphId::inferred_for(&first).expect("derivable").iri(),
            GraphId::inferred_for(&second).expect("derivable").iri()
        );
    }

    #[test]
    fn only_a_vocabulary_has_inferences_derived_from_it() {
        let system = GraphId::system();
        let error = GraphId::inferred_for(&system).expect_err("the system graph is not a source");

        assert!(
            matches!(
                error,
                GraphIdError::NotAVocabulary {
                    kind: GraphKind::System,
                    ..
                }
            ),
            "expected NotAVocabulary, got: {error}"
        );

        let vocabulary = GraphId::vocabulary("http://example.org/v/1").expect("valid");
        let inferred = GraphId::inferred_for(&vocabulary).expect("derivable");
        assert!(
            GraphId::inferred_for(&inferred).is_err(),
            "inferences over inferences would have no asserted source at all"
        );
    }

    #[test]
    fn an_empty_iri_is_rejected() {
        assert_eq!(GraphId::vocabulary(""), Err(GraphIdError::Empty));
    }

    #[test]
    fn a_relative_iri_is_rejected() {
        let error =
            GraphId::vocabulary("/vocabularies/1").expect_err("relative IRIs are ambiguous");
        assert!(
            matches!(error, GraphIdError::Invalid { .. }),
            "expected Invalid, got: {error}"
        );
    }

    #[test]
    fn a_malformed_iri_is_rejected_with_the_parsers_own_complaint() {
        let error = GraphId::vocabulary("http://exa mple.org/v/1").expect_err("a space is illegal");

        match error {
            GraphIdError::Invalid { iri, detail } => {
                assert_eq!(iri, "http://exa mple.org/v/1");
                assert!(
                    !detail.is_empty(),
                    "the parser's reason must reach the user"
                );
            }
            other => panic!("expected Invalid, got: {other}"),
        }
    }

    /// The isolation rule, stated as a test. Without it a user can author directly into OpenBiz's
    /// own bookkeeping through a path that looks like ordinary vocabulary creation.
    #[test]
    fn a_vocabulary_may_not_be_created_inside_the_openbiz_namespace() {
        for iri in [
            SYSTEM_GRAPH_IRI,
            "urn:openbiz:graph:inferred:http://example.org/v/1",
            "urn:openbiz:anything",
        ] {
            let error = GraphId::vocabulary(iri).expect_err("the namespace is reserved");
            assert!(
                matches!(error, GraphIdError::Reserved { .. }),
                "expected Reserved for {iri}, got: {error}"
            );
            assert!(
                error
                    .to_string()
                    .contains("choose an IRI in a namespace you control"),
                "refusing is not enough; the message must say what to do: {error}"
            );
        }
    }

    /// A near-miss must *not* be reserved: `urn:openbiz-example:` is somebody else's namespace
    /// that merely starts with our letters, and a prefix check that used the wrong boundary would
    /// lock a legitimate user out of their own IRIs.
    #[test]
    fn a_namespace_that_merely_resembles_ours_is_allowed() {
        assert!(GraphId::vocabulary("urn:openbiz-example:v/1").is_ok());
        assert!(GraphId::vocabulary("http://example.org/urn:openbiz:v").is_ok());
    }

    #[test]
    fn every_kind_round_trips_through_its_registry_token() {
        for kind in [
            GraphKind::Vocabulary,
            GraphKind::System,
            GraphKind::Inferred,
        ] {
            assert_eq!(GraphKind::parse(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn an_unrecognised_kind_token_is_not_guessed_at() {
        assert_eq!(GraphKind::parse("Vocabulary"), None);
        assert_eq!(GraphKind::parse("ontology"), None);
        assert_eq!(GraphKind::parse(""), None);
    }

    /// The registry is data on disk, so rebuilding from it re-applies every invariant rather than
    /// trusting what it reads.
    #[test]
    fn the_registry_cannot_reintroduce_an_impossible_pairing() {
        assert!(GraphId::from_registry(SYSTEM_GRAPH_IRI.to_owned(), GraphKind::System).is_ok());
        assert!(GraphId::from_registry(
            "urn:openbiz:graph:inferred:http://example.org/v/1".to_owned(),
            GraphKind::Inferred
        )
        .is_ok());

        assert!(
            GraphId::from_registry(SYSTEM_GRAPH_IRI.to_owned(), GraphKind::Vocabulary).is_err(),
            "a vocabulary claiming the system graph's IRI must be refused"
        );
        assert!(
            GraphId::from_registry("http://example.org/v/1".to_owned(), GraphKind::System).is_err(),
            "there is exactly one system graph"
        );
        assert!(
            GraphId::from_registry("http://example.org/v/1".to_owned(), GraphKind::Inferred)
                .is_err(),
            "an inferred graph outside the reserved namespace would be user-writable"
        );
    }
}
