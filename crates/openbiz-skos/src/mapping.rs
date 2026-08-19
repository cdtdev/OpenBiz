//! Mapping properties — the links that join one vocabulary to another.
//!
//! §10 of the SKOS Reference (W3C Recommendation, 18 August 2009), statements S38–S46. This is
//! the anti-silo half of the model: a mapping is how an enterprise says "our `Customer` is their
//! `Client`" without merging two vocabularies into one, and `CLAUDE.md` §1.7 puts reuse and
//! mapping above creating a tenth overlapping thesaurus. A build that reads a vocabulary's
//! internal structure and drops its outward links reports every mapped vocabulary as an island.
//!
//! # Five properties, and a sixth that is only a bucket
//!
//! [`MappingProperty`] is the five a link is filed under: `skos:broadMatch`, `skos:narrowMatch`,
//! `skos:relatedMatch`, `skos:closeMatch` and `skos:exactMatch`.
//!
//! `skos:mappingRelation` is deliberately **not** one of them, for the reason
//! [`SemanticRelation`](crate::SemanticRelation) leaves out `skos:semanticRelation`: S40 makes
//! four of the five its sub-properties, so the entailment runs *upwards* and from
//! `<A> skos:mappingRelation <B>` nothing follows about which of the five holds. A graph may
//! state it outright, and then it is read — S38 refuses a literal on it, and S39 carries it to
//! `skos:semanticRelation` so S19 and S20 type both ends — and it is filed under nothing.
//!
//! # What §10 entails, and where each part of it is applied
//!
//! Closed in [`model`](crate::model), each entailed link carrying a
//! [`RelationOrigin`](crate::RelationOrigin) naming the statement that licensed it:
//!
//! - **S43** — `skos:narrowMatch` is `owl:inverseOf` `skos:broadMatch`, so a hierarchical mapping
//!   written in one direction reads in the other.
//! - **S44** — `skos:relatedMatch`, `skos:closeMatch` and `skos:exactMatch` are symmetric, so each
//!   entails its converse.
//! - **S42** — every `skos:exactMatch` link is also a `skos:closeMatch` link.
//! - **S41** — `skos:broadMatch` is lifted into `skos:broader`, `skos:narrowMatch` into
//!   `skos:narrower` and `skos:relatedMatch` into `skos:related`. This is the load-bearing one:
//!   it is what puts a mapping link into the hierarchy the rest of the build already walks, so
//!   `openbiz ancestors` climbs through a mapped concept and §8.4's S27 catches Example 61's
//!   clash without §10 having to restate it.
//! - **S40 and S39** — the route to `skos:semanticRelation`, and through S19 and S20 the reason
//!   both ends of every mapping are `skos:Concept` (Examples 54–57). `skos:exactMatch` reaches
//!   `skos:mappingRelation` through S42 rather than directly, because S40 does not name it, and
//!   the derivation prints that step rather than skipping it.
//!
//! **Not applied here: S45**, which makes `skos:exactMatch` an `owl:TransitiveProperty`. The set
//! of links this module produces is the graph's own plus the one-step entailments above; the
//! closure of `skos:exactMatch` over a chain is not in it. That is the same decision `adr/0025`
//! records for S24 — a transitive closure is walked, not stored — and until the walk exists the
//! honest position is that Example 62's entailment is **not** supported. `docs/UNTESTED.md` says
//! so, and `docs/BUILD-PLAN.md` carries it as part 2 of this item rather than as done.
//!
//! # What §10 states no condition against, and what we therefore do not report
//!
//! S46 is §10's only integrity condition, and it is narrow: `skos:exactMatch` is disjoint with
//! `skos:broadMatch` and `skos:relatedMatch`. Everything else §10 explicitly permits, and each
//! of these has a test asserting our silence, because a tool that invents a rule here would
//! reject perfectly good enterprise data:
//!
//! - **A mapping inside one concept scheme** (Example 58). §10.6.1 calls using the mapping
//!   properties only across schemes a *convention*, and says there are "no formal integrity
//!   conditions" against the other case.
//! - **A reflexive mapping** (Example 66). None of the five is irreflexive.
//! - **Cycles and alternate paths in `skos:broadMatch`** (Examples 67, 68).
//!
//! Note what the second and third of those cost, since S41 lifts them: a `skos:broadMatch` cycle
//! becomes a `skos:broader` cycle, which the ancestry walk must survive rather than report. It
//! does, and §10's examples are the test.

use std::fmt;

use crate::model::SkosRule;
use crate::ns;
use crate::relations::SemanticRelation;

/// `skos:mappingRelation` — the super-property, and the route to `skos:semanticRelation`. §10.
pub const SKOS_MAPPING_RELATION: &str = "http://www.w3.org/2004/02/skos/core#mappingRelation";
/// `skos:broadMatch` — a hierarchical mapping link. §10.
pub const SKOS_BROAD_MATCH: &str = "http://www.w3.org/2004/02/skos/core#broadMatch";
/// `skos:narrowMatch` — the inverse of `skos:broadMatch` under S43. §10.
pub const SKOS_NARROW_MATCH: &str = "http://www.w3.org/2004/02/skos/core#narrowMatch";
/// `skos:relatedMatch` — an associative mapping link. §10.
pub const SKOS_RELATED_MATCH: &str = "http://www.w3.org/2004/02/skos/core#relatedMatch";
/// `skos:closeMatch` — "sufficiently similar that they can be used interchangeably in some
/// information retrieval applications", and deliberately not transitive. §10.
pub const SKOS_CLOSE_MATCH: &str = "http://www.w3.org/2004/02/skos/core#closeMatch";
/// `skos:exactMatch` — a sub-property of `skos:closeMatch`, and the only transitive one. §10.
pub const SKOS_EXACT_MATCH: &str = "http://www.w3.org/2004/02/skos/core#exactMatch";

/// One of the five mapping properties a link can be filed under.
///
/// Ordered as [`SemanticRelation`] is, so the two sections of a report read the same way: the two
/// hierarchical directions an author writes, then the associative one, then the two equivalence
/// ones from weaker to stronger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MappingProperty {
    /// `skos:broadMatch` — the object is broader than the subject, across schemes.
    BroadMatch,
    /// `skos:narrowMatch` — the object is narrower than the subject, across schemes.
    NarrowMatch,
    /// `skos:relatedMatch` — associative, and symmetric under S44.
    RelatedMatch,
    /// `skos:closeMatch` — interchangeable in some applications. Symmetric, **not** transitive:
    /// §10.1 says so outright, to avoid "compound errors" when mappings are chained across more
    /// than two schemes.
    CloseMatch,
    /// `skos:exactMatch` — interchangeable across a wide range of applications. Symmetric and,
    /// under S45, transitive — which this build does not yet close. See the module note.
    ExactMatch,
}

impl MappingProperty {
    /// Every mapping property, in a stable order.
    pub const ALL: [MappingProperty; 5] = [
        MappingProperty::BroadMatch,
        MappingProperty::NarrowMatch,
        MappingProperty::RelatedMatch,
        MappingProperty::CloseMatch,
        MappingProperty::ExactMatch,
    ];

    /// The property's local name within the SKOS namespace.
    pub fn local_name(self) -> &'static str {
        match self {
            MappingProperty::BroadMatch => "broadMatch",
            MappingProperty::NarrowMatch => "narrowMatch",
            MappingProperty::RelatedMatch => "relatedMatch",
            MappingProperty::CloseMatch => "closeMatch",
            MappingProperty::ExactMatch => "exactMatch",
        }
    }

    /// The property's IRI.
    pub fn iri(self) -> String {
        format!("{}{}", ns::SKOS, self.local_name())
    }

    /// The mapping property an IRI names, or `None` for anything else.
    ///
    /// `skos:mappingRelation` is `None`, deliberately: see the module note.
    pub fn from_iri(iri: &str) -> Option<Self> {
        let local = iri.strip_prefix(ns::SKOS)?;
        MappingProperty::ALL
            .into_iter()
            .find(|property| property.local_name() == local)
    }

    /// The property this one is `owl:inverseOf`, and the statement that says so.
    ///
    /// S43 pairs `skos:broadMatch` with `skos:narrowMatch`. The other three are each their own
    /// inverse, because S44 makes them symmetric — the same reading [`SemanticRelation::inverse`]
    /// takes of S23, and for the same reason: one pass then closes all five while still citing
    /// the statement the specification prints for each.
    pub fn inverse(self) -> (MappingProperty, SkosRule) {
        match self {
            MappingProperty::BroadMatch => (MappingProperty::NarrowMatch, SkosRule::S43),
            MappingProperty::NarrowMatch => (MappingProperty::BroadMatch, SkosRule::S43),
            MappingProperty::RelatedMatch => (MappingProperty::RelatedMatch, SkosRule::S44),
            MappingProperty::CloseMatch => (MappingProperty::CloseMatch, SkosRule::S44),
            MappingProperty::ExactMatch => (MappingProperty::ExactMatch, SkosRule::S44),
        }
    }

    /// The semantic relation this mapping property is a sub-property of under S41, if it has one.
    ///
    /// `None` for the two equivalence properties, and that is §10's design rather than an
    /// omission: an equivalence mapping is not a hierarchy and not an association, so there is
    /// nothing in §8 for it to lift into. Lifting `skos:exactMatch` into `skos:related` to make
    /// the table total would state something the graph does not — and would then collide with S27.
    pub fn semantic_counterpart(self) -> Option<(SemanticRelation, SkosRule)> {
        match self {
            MappingProperty::BroadMatch => Some((SemanticRelation::Broader, SkosRule::S41)),
            MappingProperty::NarrowMatch => Some((SemanticRelation::Narrower, SkosRule::S41)),
            MappingProperty::RelatedMatch => Some((SemanticRelation::Related, SkosRule::S41)),
            MappingProperty::CloseMatch | MappingProperty::ExactMatch => None,
        }
    }

    /// The mapping property this one is a sub-property of under S42, if any.
    ///
    /// Only `skos:exactMatch`, which S42 puts under `skos:closeMatch`. Nothing else in §10 sits
    /// under another of the five.
    pub fn super_property(self) -> Option<(MappingProperty, SkosRule)> {
        match self {
            MappingProperty::ExactMatch => Some((MappingProperty::CloseMatch, SkosRule::S42)),
            _ => None,
        }
    }

    /// The property whose S40 statement carries this one up to `skos:mappingRelation`.
    ///
    /// S40 names four of the five and not `skos:exactMatch`, which reaches the super-property
    /// through S42 instead. So the chain for an exact match is two steps and the derivation list
    /// prints both — a citation that skipped the middle step would name a statement that does not
    /// mention the property the author used. The same choice [`SemanticRelation`] records for
    /// S22-then-S21.
    pub fn mapping_relation_via(self) -> (MappingProperty, SkosRule) {
        match self.super_property() {
            Some((super_property, _)) => (super_property, SkosRule::S40),
            None => (self, SkosRule::S40),
        }
    }

    /// Whether S46 makes this property disjoint with `skos:exactMatch`, and how that is argued.
    ///
    /// S46 names two: `skos:broadMatch` and `skos:relatedMatch`. `skos:narrowMatch` is the third,
    /// and §10.4's own note is the argument — "because skos:exactMatch is a symmetric property,
    /// and skos:broadMatch and skos:narrowMatch are inverses, skos:exactMatch is therefore also
    /// disjoint with skos:narrowMatch". A report that cited S46 flatly for a `skos:narrowMatch`
    /// clash would name a statement that does not mention the property in front of the reader, so
    /// the two cases are distinguished here and printed differently.
    pub fn disjoint_with_exact_match(self) -> Option<ExactMatchDisjointness> {
        match self {
            MappingProperty::BroadMatch | MappingProperty::RelatedMatch => {
                Some(ExactMatchDisjointness::Stated)
            }
            MappingProperty::NarrowMatch => Some(ExactMatchDisjointness::ByInverse),
            // Its own super-property under S42, so every exact match is a close match: the two
            // holding together is the entailment and never a clash.
            MappingProperty::CloseMatch | MappingProperty::ExactMatch => None,
        }
    }
}

impl fmt::Display for MappingProperty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "skos:{}", self.local_name())
    }
}

/// How S46's disjointness reaches a property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExactMatchDisjointness {
    /// S46 names the property outright.
    Stated,
    /// S46 reaches it through S43's inverse and S44's symmetry, which is the argument §10.4's own
    /// note makes for `skos:narrowMatch`.
    ByInverse,
}

impl fmt::Display for ExactMatchDisjointness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExactMatchDisjointness::Stated => write!(f, "{}", SkosRule::S46),
            ExactMatchDisjointness::ByInverse => write!(
                f,
                "{}\n    \u{a7}10.4 extends it: \"because skos:exactMatch is a symmetric \
                 property, and skos:broadMatch and skos:narrowMatch are inverses, \
                 skos:exactMatch is therefore also disjoint with skos:narrowMatch\"",
                SkosRule::S46
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_property_iris_are_in_the_skos_namespace() {
        for property in MappingProperty::ALL {
            assert_eq!(
                property.iri(),
                format!("{}{}", ns::SKOS, property.local_name())
            );
            assert_eq!(MappingProperty::from_iri(&property.iri()), Some(property));
        }
        assert_eq!(
            SKOS_MAPPING_RELATION,
            format!("{}mappingRelation", ns::SKOS)
        );
        for (iri, property) in [
            (SKOS_BROAD_MATCH, MappingProperty::BroadMatch),
            (SKOS_NARROW_MATCH, MappingProperty::NarrowMatch),
            (SKOS_RELATED_MATCH, MappingProperty::RelatedMatch),
            (SKOS_CLOSE_MATCH, MappingProperty::CloseMatch),
            (SKOS_EXACT_MATCH, MappingProperty::ExactMatch),
        ] {
            assert_eq!(iri, property.iri());
        }
    }

    /// `skos:mappingRelation` is read and never a bucket we file under, for the reason
    /// `skos:semanticRelation` is not one of the five semantic relations.
    #[test]
    fn the_super_property_is_not_one_of_the_five() {
        assert_eq!(MappingProperty::from_iri(SKOS_MAPPING_RELATION), None);
        assert_eq!(
            MappingProperty::from_iri("http://example.com/exactMatch"),
            None
        );
        assert_eq!(
            MappingProperty::from_iri(&format!("{}broader", ns::SKOS)),
            None
        );
    }

    /// Inversion is an involution. A table with one row wrong would point a mapped hierarchy the
    /// wrong way round, which is the one mistake here a reader could not see in a report.
    #[test]
    fn every_inverse_is_its_own_inverse() {
        for property in MappingProperty::ALL {
            let (inverse, _) = property.inverse();
            assert_eq!(inverse.inverse().0, property, "{property}");
        }
        assert_eq!(
            MappingProperty::BroadMatch.inverse(),
            (MappingProperty::NarrowMatch, SkosRule::S43)
        );
        for symmetric in [
            MappingProperty::RelatedMatch,
            MappingProperty::CloseMatch,
            MappingProperty::ExactMatch,
        ] {
            assert_eq!(
                symmetric.inverse(),
                (symmetric, SkosRule::S44),
                "S44 makes {symmetric} symmetric, so it is its own inverse"
            );
        }
    }

    /// S41 names three and only three. The equivalence properties must lift into nothing — a
    /// `skos:exactMatch` read as a `skos:related` would put every exact mapping into the
    /// associative relation and, through S27, start reporting clashes SKOS does not state.
    #[test]
    fn only_the_three_lift_into_a_semantic_relation() {
        assert_eq!(
            MappingProperty::BroadMatch.semantic_counterpart(),
            Some((SemanticRelation::Broader, SkosRule::S41))
        );
        assert_eq!(
            MappingProperty::NarrowMatch.semantic_counterpart(),
            Some((SemanticRelation::Narrower, SkosRule::S41))
        );
        assert_eq!(
            MappingProperty::RelatedMatch.semantic_counterpart(),
            Some((SemanticRelation::Related, SkosRule::S41))
        );
        assert_eq!(MappingProperty::CloseMatch.semantic_counterpart(), None);
        assert_eq!(MappingProperty::ExactMatch.semantic_counterpart(), None);
    }

    /// The route to `skos:mappingRelation`: four properties reach it in one step under S40, and
    /// `skos:exactMatch` reaches it through `skos:closeMatch` because S40 does not name it.
    #[test]
    fn the_exact_match_reaches_the_super_property_through_close_match() {
        assert_eq!(
            MappingProperty::ExactMatch.super_property(),
            Some((MappingProperty::CloseMatch, SkosRule::S42))
        );
        assert_eq!(
            MappingProperty::ExactMatch.mapping_relation_via(),
            (MappingProperty::CloseMatch, SkosRule::S40)
        );
        for named_by_s40 in [
            MappingProperty::BroadMatch,
            MappingProperty::NarrowMatch,
            MappingProperty::RelatedMatch,
            MappingProperty::CloseMatch,
        ] {
            assert_eq!(named_by_s40.super_property(), None, "{named_by_s40}");
            assert_eq!(
                named_by_s40.mapping_relation_via(),
                (named_by_s40, SkosRule::S40)
            );
        }
    }

    /// S46 names two properties and §10.4's note reaches a third. The fourth and fifth are not
    /// disjoint with `skos:exactMatch` at all, and getting that wrong would report every exact
    /// mapping as a violation of the statement that entails it.
    #[test]
    fn the_disjointness_covers_three_properties_and_argues_the_third_differently() {
        assert_eq!(
            MappingProperty::BroadMatch.disjoint_with_exact_match(),
            Some(ExactMatchDisjointness::Stated)
        );
        assert_eq!(
            MappingProperty::RelatedMatch.disjoint_with_exact_match(),
            Some(ExactMatchDisjointness::Stated)
        );
        assert_eq!(
            MappingProperty::NarrowMatch.disjoint_with_exact_match(),
            Some(ExactMatchDisjointness::ByInverse)
        );
        assert_eq!(
            MappingProperty::CloseMatch.disjoint_with_exact_match(),
            None
        );
        assert_eq!(
            MappingProperty::ExactMatch.disjoint_with_exact_match(),
            None
        );

        assert!(ExactMatchDisjointness::Stated.to_string().contains("S46"));
        assert!(ExactMatchDisjointness::ByInverse
            .to_string()
            .contains("also disjoint with skos:narrowMatch"));
    }

    #[test]
    fn a_property_prints_as_the_specification_writes_it() {
        assert_eq!(MappingProperty::BroadMatch.to_string(), "skos:broadMatch");
        assert_eq!(MappingProperty::ExactMatch.to_string(), "skos:exactMatch");
    }
}
