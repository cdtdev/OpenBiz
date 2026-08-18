//! SKOS-XL — labels as resources you can say things about.
//!
//! In plain SKOS a label is a literal hanging off a concept, so there is nowhere to record who
//! approved it, when it entered the thesaurus, or that this acronym stands for that term. ISO
//! 25964 needs all three, which is why `CLAUDE.md` §2 lists SKOS-XL as part of the authoring model
//! rather than as an optional extra: **plain SKOS cannot faithfully represent an ISO 25964
//! thesaurus.** SKOS-XL makes the label a resource with its own IRI, and everything else follows.
//!
//! # What the specification says, and where
//!
//! SKOS-XL is Appendix B of the SKOS Reference (W3C Recommendation, 18 August 2009). Statements
//! S47–S62 are numbered there exactly as S1–S46 are numbered in the body.
//!
//! - **B.2** the class. [`S49`](crate::SkosRule::S49)–[`S52`](crate::SkosRule::S52) define
//!   `skosxl:literalForm` and constrain a label to exactly one of them.
//! - **B.3** the three labelling properties, their range ([`S54`](crate::SkosRule::S54)), the
//!   **dumbing-down** chains ([`S55`](crate::SkosRule::S55)–[`S57`](crate::SkosRule::S57)), and
//!   their pairwise disjointness ([`S58`](crate::SkosRule::S58)).
//! - **B.4** `skosxl:labelRelation`, S59–S62 — a separate build-plan item, and deliberately not
//!   here. It is an extension point with no built-in refinements, so nothing in the authoring path
//!   depends on it; the dumbing-down does, and that is what this module is for.
//!
//! # Dumbing down is the whole point
//!
//! S55–S57 make the property *chain* `(skosxl:prefLabel, skosxl:literalForm)` a sub-property of
//! `skos:prefLabel`, and the same for alternative and hidden. Example 83 spells the consequence
//! out: a concept labelled through an XL label also carries the plain SKOS label, by entailment.
//!
//! So a vocabulary authored in SKOS-XL is still readable by a tool that has never heard of
//! SKOS-XL — but only if somebody performs the entailment. We do, and the derived labels go into
//! the same place as the asserted ones, carrying a [`LabelOrigin`] that says where each came from.
//! That placement is not a convenience: it is what makes Examples 84–87 come out inconsistent,
//! because B.3.4.2 says they are inconsistent *because of* S13 and S14 — the two integrity
//! conditions on the plain labels the chains produce.
//!
//! # Appendix B has no "Integrity Conditions" heading, and we say so rather than pretending
//!
//! §1.7 sets out the structure every section follows, and "Integrity Conditions — if there are
//! any integrity conditions, those are given" is one of its parts. §4.4, §5.4, §8.4, §9.4 and
//! §10.4 all have one. **Appendix B has none** — B.2.2, B.3.2 and B.4.2 are all headed "Class and
//! Property Definitions". Three consequences, each taken deliberately:
//!
//! 1. **Two different literal forms on one label is [`Severity::Inconsistent`].** Not by our
//!    analogy — the specification itself marks Examples 76, 77, 78 and 79 "(not consistent)". That
//!    is a more direct statement than a section heading, so we take it.
//! 2. **A violated disjointness — S48 or S58 — is [`Severity::Inconsistent`] by our reading.** No
//!    example marks one, but a resource in two disjoint classes and a pair of disjoint properties
//!    sharing a value are logical contradictions, and `Severity::IllFormed` means "SKOS permits it
//!    and we disagree", which would be false. S58 is worded identically to S13, which §5.4 does
//!    call an integrity condition. The classification is ours; the contradiction is not.
//! 3. **Everything else about a literal form is [`Severity::IllFormed`]** — our judgement, said as
//!    ours. See below.
//!
//! [`Severity::Inconsistent`]: crate::Severity
//! [`Severity::IllFormed`]: crate::Severity
//!
//! # A label with *no* literal form is not inconsistent, and getting that wrong would be a bug
//!
//! S52 says `skosxl:Label` is a sub-class of a restriction on `skosxl:literalForm` cardinality
//! **exactly 1**. It is tempting to read "exactly 1" as "a label with none is broken". Under OWL's
//! open-world assumption it is not: the restriction *entails that a form exists*, it does not
//! require the graph to state it. A partial export, a federated query, or a half-finished import
//! all produce labels whose forms are elsewhere, and calling those inconsistent would refuse valid
//! data — the failure `docs/COMPETITIVE.md` records against the incumbents.
//!
//! So it is reported as ill-formed, with the reason, because a label an author cannot see is still
//! something they need told about. Two forms is the other half of "exactly 1" and *is* a
//! contradiction, because both cannot be the one value.
//!
//! # `skosxl:literalForm` is a datatype property; `skos:prefLabel` is an annotation property
//!
//! This is why the two treat an IRI differently, and the difference is the specification's, not
//! ours. S10 makes `skos:prefLabel` an `owl:AnnotationProperty`, and OWL 2 annotation properties
//! take IRIs as values quite legally — so `skos:prefLabel <http://…>` is odd, reportable, and not
//! a contradiction. [`S49`](crate::SkosRule::S49) makes `skosxl:literalForm` an
//! `owl:DatatypeProperty`, whose values are literals by definition — so a node there *is* a
//! contradiction, exactly as a literal on an object property is under S3 and S30.
//!
//! [`S51`](crate::SkosRule::S51)'s range — "the class of RDF plain literals" — is word for word
//! S12's, so a form that is a literal but not a plain one is treated as S12's is: reported,
//! discarded, and the vocabulary still stands. **The analogy is ours.** §5.6.2's "an application
//! may reject such data but is not required to" is said about §5 and is not restated in Appendix
//! B. We extend it because the two cases are one step apart — a non-plain form that dumbed down
//! would produce exactly the non-plain `skos:prefLabel` that §5.6.2 is about — and because the
//! conservative reading is the one that does not turn a customer's thesaurus away.

use crate::model::SkosRule;

/// `skosxl:Label` — the class of label resources. B.2.
pub const SKOSXL_LABEL: &str = "http://www.w3.org/2008/05/skos-xl#Label";
/// `skosxl:literalForm` — the one literal a label carries. B.2.
pub const SKOSXL_LITERAL_FORM: &str = "http://www.w3.org/2008/05/skos-xl#literalForm";
/// `skosxl:prefLabel`. B.3.
pub const SKOSXL_PREF_LABEL: &str = "http://www.w3.org/2008/05/skos-xl#prefLabel";
/// `skosxl:altLabel`. B.3.
pub const SKOSXL_ALT_LABEL: &str = "http://www.w3.org/2008/05/skos-xl#altLabel";
/// `skosxl:hiddenLabel`. B.3.
pub const SKOSXL_HIDDEN_LABEL: &str = "http://www.w3.org/2008/05/skos-xl#hiddenLabel";

/// Where a lexical label came from.
///
/// The same shape as [`ClassOrigin`](crate::ClassOrigin) and for the same reason: a report that
/// cannot distinguish what the graph said from what we concluded is not an audit trail. A label
/// stated outright *and* reachable through a chain is [`Asserted`](LabelOrigin::Asserted) — the
/// graph said so, and claiming to have deduced it would be a derivation nobody needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LabelOrigin {
    /// The graph carries the `skos:prefLabel`, `skos:altLabel` or `skos:hiddenLabel` statement.
    Asserted,
    /// It follows from an XL label's literal form, under one of the S55–S57 chains.
    DumbedDown(SkosRule),
}

impl std::fmt::Display for LabelOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LabelOrigin::Asserted => write!(f, "asserted"),
            LabelOrigin::DumbedDown(rule) => write!(f, "from SKOS-XL, {}", rule.number()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ns, LabelKind};

    /// The IRIs are written out in full above; this is the check that they are the right ones.
    #[test]
    fn the_vocabulary_iris_are_in_the_skos_xl_namespace() {
        for (iri, local) in [
            (SKOSXL_LABEL, "Label"),
            (SKOSXL_LITERAL_FORM, "literalForm"),
            (SKOSXL_PREF_LABEL, "prefLabel"),
            (SKOSXL_ALT_LABEL, "altLabel"),
            (SKOSXL_HIDDEN_LABEL, "hiddenLabel"),
        ] {
            assert_eq!(iri, format!("{}{local}", ns::SKOSXL));
        }
    }

    /// The three XL labelling properties have the same local names as the plain ones, which is
    /// what B.3.1 means by "analogous", and is also the easiest pair of constants to swap.
    #[test]
    fn the_xl_labelling_properties_map_to_the_same_kinds_as_the_plain_ones() {
        for kind in LabelKind::ALL {
            assert_eq!(LabelKind::from_xl_iri(&kind.xl_property_iri()), Some(kind));
            assert_eq!(
                kind.xl_property_iri(),
                format!("{}{}", ns::SKOSXL, kind.local_name())
            );
        }
        assert_eq!(LabelKind::from_xl_iri(SKOSXL_LITERAL_FORM), None);
        // The two namespaces are not interchangeable, and a reader that confused them would dumb
        // a label down to itself.
        assert_eq!(
            LabelKind::from_xl_iri(&LabelKind::Preferred.property_iri()),
            None
        );
        assert_eq!(
            LabelKind::from_iri(&LabelKind::Preferred.xl_property_iri()),
            None
        );
    }

    #[test]
    fn each_kind_names_the_chain_that_dumbs_it_down() {
        assert_eq!(LabelKind::Preferred.dumbing_down_rule(), SkosRule::S55);
        assert_eq!(LabelKind::Alternative.dumbing_down_rule(), SkosRule::S56);
        assert_eq!(LabelKind::Hidden.dumbing_down_rule(), SkosRule::S57);
    }

    #[test]
    fn an_origin_says_which_chain_produced_the_label() {
        assert_eq!(LabelOrigin::Asserted.to_string(), "asserted");
        assert_eq!(
            LabelOrigin::DumbedDown(SkosRule::S55).to_string(),
            "from SKOS-XL, S55"
        );
    }
}
