//! Semantic relations — the links that make a vocabulary a *structure* and not a word list.
//!
//! §8 of the SKOS Reference (W3C Recommendation, 18 August 2009), statements S18–S27. A thesaurus
//! is bought for its hierarchy: broader/narrower is ISO 25964's BT/NT and `skos:related` is its
//! RT, and everything a governance team does with a vocabulary — browse it, roll a report up it,
//! check it against a standard — walks these links.
//!
//! # Six properties, and only five of them hold anything
//!
//! [`SemanticRelation`] is the five that carry a link a caller reads back:
//! `skos:broader`, `skos:narrower`, `skos:related`, `skos:broaderTransitive` and
//! `skos:narrowerTransitive`.
//!
//! `skos:semanticRelation` itself is deliberately **not** one of them. S21 makes the other five
//! its sub-properties, so every link entails one — but the entailment runs *upwards*, and from
//! `<A> skos:semanticRelation <B>` nothing follows about which of the five holds. Materialising it
//! would be a sixth copy of every link in the vocabulary answering a question the five already
//! answer. What it is for is S19 and S20, its domain and range, which is where it does its work
//! and where we do read it: see [`crate::SkosRule::S19`].
//!
//! A graph may of course state `skos:semanticRelation` outright, and Example 25's family do not
//! but a federated one might. That statement is read — S18 refuses a literal on it and S19/S20
//! type both ends — and then it stops, because there is no sub-property it could be filed under.
//!
//! # What is closed, and what is not yet
//!
//! Closed here, each entailed link carrying a [`RelationOrigin`] that names the statement that
//! licensed it:
//!
//! - **S25** — `skos:narrower` is `owl:inverseOf` `skos:broader`. Stating one direction gives the
//!   other, which is what lets an author write a hierarchy downwards or upwards as they please.
//! - **S26** — the same for the two transitive variants.
//! - **S22** — `skos:broader` is a sub-property of `skos:broaderTransitive`, and `skos:narrower`
//!   of `skos:narrowerTransitive`. So every asserted link is also a transitive-variant link.
//! - **S23** — `skos:related` is an `owl:SymmetricProperty`, so a relation entails its converse.
//!
//! **Not yet closed: S24**, which makes the two transitive variants `owl:TransitiveProperty`. So
//! `skos:broaderTransitive` here holds the one-step links S22 lifted and the ones the graph stated,
//! and *not* their closure. It is the next build-plan item, together with the integrity condition
//! S27 that needs it — §8.6's Examples 27 and 29 are inconsistent only once the closure exists, so
//! neither the closure nor the condition is claimed until both land. In `docs/UNTESTED.md`.
//!
//! **And when it does land, it will not be stored here.** `docs/adr/0024` measured what the
//! closure would cost and decided against materialising it at any size: a chain of 100 000 links
//! is a legal SKOS graph and licenses five thousand million pairs, and a stored
//! `(Node, RelationOrigin)` can cite S24 but cannot name the path it took, which
//! `CLAUDE.md` §3 requires of every inference. Ancestry is therefore a traversal answered on read.
//! A caller reading [`SemanticRelation::BroaderTransitive`] out of a [`Resource`](crate::Resource)
//! will keep getting one-step links after S24 lands, permanently and by design — which is why the
//! accessor is named for the property and never for "ancestors".
//!
//! # Polyhierarchy is not a defect and is not treated as one
//!
//! A concept with two broader concepts is ordinary in a thesaurus and §8 states nothing against it.
//! It is counted — an author usually wants to know how much of it there is, and a migration from a
//! strictly-monohierarchical source always does — and it is never a [`Finding`](crate::Finding).
//! Reporting it as one would be inventing an integrity condition the specification does not state,
//! which is the incumbents' failure that `docs/COMPETITIVE.md` records.

use std::fmt;

use crate::model::SkosRule;
use crate::ns;

/// `skos:semanticRelation` — the super-property of the five, and the one S19 and S20 constrain.
pub const SKOS_SEMANTIC_RELATION: &str = "http://www.w3.org/2004/02/skos/core#semanticRelation";
/// `skos:broader` — ISO 25964's BT. §8.
pub const SKOS_BROADER: &str = "http://www.w3.org/2004/02/skos/core#broader";
/// `skos:narrower` — ISO 25964's NT. §8.
pub const SKOS_NARROWER: &str = "http://www.w3.org/2004/02/skos/core#narrower";
/// `skos:related` — ISO 25964's RT. §8.
pub const SKOS_RELATED: &str = "http://www.w3.org/2004/02/skos/core#related";
/// `skos:broaderTransitive` — the transitive super-property of `skos:broader`. §8.
pub const SKOS_BROADER_TRANSITIVE: &str = "http://www.w3.org/2004/02/skos/core#broaderTransitive";
/// `skos:narrowerTransitive` — the transitive super-property of `skos:narrower`. §8.
pub const SKOS_NARROWER_TRANSITIVE: &str = "http://www.w3.org/2004/02/skos/core#narrowerTransitive";

/// One of the five semantic relation properties a link can be filed under.
///
/// Ordered so that a report reads down the hierarchy and then across it: the two directions an
/// author writes, then the two the transitivity rules use, then the associative one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SemanticRelation {
    /// `skos:broader` — the subject has the object as a broader concept.
    Broader,
    /// `skos:narrower` — the subject has the object as a narrower concept.
    Narrower,
    /// `skos:broaderTransitive`. Holds S22's lifted links and the graph's own; **not** closed
    /// under S24 yet.
    BroaderTransitive,
    /// `skos:narrowerTransitive`. As above.
    NarrowerTransitive,
    /// `skos:related` — associative, and symmetric under S23.
    Related,
}

impl SemanticRelation {
    /// Every relation, in a stable order.
    pub const ALL: [SemanticRelation; 5] = [
        SemanticRelation::Broader,
        SemanticRelation::Narrower,
        SemanticRelation::BroaderTransitive,
        SemanticRelation::NarrowerTransitive,
        SemanticRelation::Related,
    ];

    /// The property's local name within the SKOS namespace.
    pub fn local_name(self) -> &'static str {
        match self {
            SemanticRelation::Broader => "broader",
            SemanticRelation::Narrower => "narrower",
            SemanticRelation::BroaderTransitive => "broaderTransitive",
            SemanticRelation::NarrowerTransitive => "narrowerTransitive",
            SemanticRelation::Related => "related",
        }
    }

    /// The property's IRI.
    pub fn iri(self) -> String {
        format!("{}{}", ns::SKOS, self.local_name())
    }

    /// The relation an IRI names, or `None` for anything else — which is most predicates.
    pub fn from_iri(iri: &str) -> Option<Self> {
        let local = iri.strip_prefix(ns::SKOS)?;
        SemanticRelation::ALL
            .into_iter()
            .find(|relation| relation.local_name() == local)
    }

    /// The property this one is `owl:inverseOf`, and the statement that says so.
    ///
    /// Every one of the five has an inverse: S25 pairs `skos:broader` with `skos:narrower`, S26
    /// pairs the two transitive variants, and `skos:related` is its own inverse because S23 makes
    /// it symmetric. Saying "symmetric is self-inverse" here is not a liberty — it is what
    /// `owl:SymmetricProperty` means — and it lets one pass close all five while still citing the
    /// statement the specification actually prints for each.
    pub fn inverse(self) -> (SemanticRelation, SkosRule) {
        match self {
            SemanticRelation::Broader => (SemanticRelation::Narrower, SkosRule::S25),
            SemanticRelation::Narrower => (SemanticRelation::Broader, SkosRule::S25),
            SemanticRelation::BroaderTransitive => {
                (SemanticRelation::NarrowerTransitive, SkosRule::S26)
            }
            SemanticRelation::NarrowerTransitive => {
                (SemanticRelation::BroaderTransitive, SkosRule::S26)
            }
            SemanticRelation::Related => (SemanticRelation::Related, SkosRule::S23),
        }
    }

    /// The transitive variant this one is a sub-property of under S22, if it has one.
    ///
    /// `None` for the transitive variants themselves — they are sub-properties of
    /// `skos:semanticRelation` under S21, not of each other — and `None` for `skos:related`, whose
    /// only super-property is `skos:semanticRelation`. A hierarchy is *not* an association and
    /// lifting one into the other would state something the graph does not.
    pub fn transitive_variant(self) -> Option<(SemanticRelation, SkosRule)> {
        match self {
            SemanticRelation::Broader => Some((SemanticRelation::BroaderTransitive, SkosRule::S22)),
            SemanticRelation::Narrower => {
                Some((SemanticRelation::NarrowerTransitive, SkosRule::S22))
            }
            _ => None,
        }
    }

    /// The statement making this property a sub-property of `skos:semanticRelation`.
    ///
    /// S21 names only the three: `skos:broaderTransitive`, `skos:narrowerTransitive` and
    /// `skos:related`. `skos:broader` and `skos:narrower` reach `skos:semanticRelation` through
    /// their transitive variants under S22, so their chain is two steps and the derivation list
    /// prints both — a citation that skipped the middle step would name a statement that does not
    /// mention the property the graph actually used.
    pub fn semantic_relation_rule(self) -> SkosRule {
        SkosRule::S21
    }
}

impl fmt::Display for SemanticRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "skos:{}", self.local_name())
    }
}

/// How a resource came to be linked to another.
///
/// Used by `skosxl:labelRelation` and by the five semantic relations, because the question is the
/// same one in both places and it is the question the whole model exists to answer: a report that
/// cannot distinguish what the graph said from what we concluded is not an audit trail.
///
/// A link the graph states in both directions is [`Asserted`](RelationOrigin::Asserted) at both
/// ends; only the direction it left out is an inference, and only that one is counted as one.
/// The rule is carried rather than assumed so that no entailed link can arrive without saying
/// which statement licensed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationOrigin {
    /// The graph carries the statement in this direction.
    Asserted,
    /// We concluded it, under this rule.
    Entailed(SkosRule),
}

impl fmt::Display for RelationOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RelationOrigin::Asserted => write!(f, "asserted"),
            RelationOrigin::Entailed(rule) => write!(f, "inferred, {}", rule.number()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_property_iris_are_in_the_skos_namespace() {
        for relation in SemanticRelation::ALL {
            assert_eq!(
                relation.iri(),
                format!("{}{}", ns::SKOS, relation.local_name())
            );
            assert_eq!(SemanticRelation::from_iri(&relation.iri()), Some(relation));
        }
        assert_eq!(
            SKOS_SEMANTIC_RELATION,
            format!("{}semanticRelation", ns::SKOS)
        );
        for (iri, relation) in [
            (SKOS_BROADER, SemanticRelation::Broader),
            (SKOS_NARROWER, SemanticRelation::Narrower),
            (SKOS_RELATED, SemanticRelation::Related),
            (SKOS_BROADER_TRANSITIVE, SemanticRelation::BroaderTransitive),
            (
                SKOS_NARROWER_TRANSITIVE,
                SemanticRelation::NarrowerTransitive,
            ),
        ] {
            assert_eq!(iri, relation.iri());
        }
    }

    /// `skos:semanticRelation` is a property we read and never a bucket we file under, so
    /// `from_iri` must refuse it. Filing a link under the super-property would claim we knew which
    /// of the five held when the statement says only that one of them does.
    #[test]
    fn the_super_property_is_not_one_of_the_five() {
        assert_eq!(SemanticRelation::from_iri(SKOS_SEMANTIC_RELATION), None);
        assert_eq!(
            SemanticRelation::from_iri("http://example.com/broader"),
            None
        );
        assert_eq!(
            SemanticRelation::from_iri(&format!("{}member", ns::SKOS)),
            None
        );
    }

    /// Inversion is an involution: taking it twice returns the property you started with. A table
    /// that got one row wrong would close a hierarchy in the wrong direction, which is the one
    /// mistake in this module a reader could not see in a report.
    #[test]
    fn every_inverse_is_its_own_inverse() {
        for relation in SemanticRelation::ALL {
            let (inverse, _) = relation.inverse();
            assert_eq!(inverse.inverse().0, relation, "{relation}");
        }
        assert_eq!(
            SemanticRelation::Related.inverse(),
            (SemanticRelation::Related, SkosRule::S23),
            "S23 makes skos:related symmetric, so it is its own inverse"
        );
    }

    /// The two directions must not share a transitive variant, and the transitive variants must
    /// not have one of their own — S22 lifts `skos:broader` to `skos:broaderTransitive` and stops.
    #[test]
    fn only_the_two_base_directions_lift_to_a_transitive_variant() {
        assert_eq!(
            SemanticRelation::Broader.transitive_variant(),
            Some((SemanticRelation::BroaderTransitive, SkosRule::S22))
        );
        assert_eq!(
            SemanticRelation::Narrower.transitive_variant(),
            Some((SemanticRelation::NarrowerTransitive, SkosRule::S22))
        );
        assert_eq!(
            SemanticRelation::BroaderTransitive.transitive_variant(),
            None
        );
        assert_eq!(
            SemanticRelation::NarrowerTransitive.transitive_variant(),
            None
        );
        // The associative relation is not a hierarchy and must never be lifted into one.
        assert_eq!(SemanticRelation::Related.transitive_variant(), None);
    }

    #[test]
    fn a_relation_origin_says_whether_the_graph_stated_the_direction() {
        assert_eq!(RelationOrigin::Asserted.to_string(), "asserted");
        assert_eq!(
            RelationOrigin::Entailed(SkosRule::S25).to_string(),
            "inferred, S25"
        );
    }

    #[test]
    fn a_relation_prints_as_the_specification_writes_it() {
        assert_eq!(SemanticRelation::Broader.to_string(), "skos:broader");
        assert_eq!(
            SemanticRelation::BroaderTransitive.to_string(),
            "skos:broaderTransitive"
        );
    }
}
