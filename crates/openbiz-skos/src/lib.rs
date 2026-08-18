//! SKOS and SKOS-XL domain model — the product's core noun.
//!
//! SKOS-XL is **not optional** here. The documented ISO 25964 mapping relies on SKOS-XL wherever
//! the base SKOS model lacks a construct for a thesaurus feature, so plain SKOS cannot faithfully
//! represent an ISO 25964 thesaurus — and our enterprise buyers have ISO 25964 in their
//! requirements. See `docs/COMPETITIVE.md`.

mod model;

pub use model::{
    ClassOrigin, CoreModel, CoreModelBuilder, Derivation, Finding, ListDefect, Literal, MemberList,
    Node, Resource, Severity, SkosClass, SkosRule, Statement, Term, RDF_FIRST, RDF_NIL, RDF_REST,
    RDF_TYPE, SKOS_HAS_TOP_CONCEPT, SKOS_IN_SCHEME, SKOS_MEMBER, SKOS_MEMBER_LIST,
    SKOS_TOP_CONCEPT_OF,
};

/// Namespace IRIs used throughout the SKOS model.
pub mod ns {
    /// The SKOS namespace.
    pub const SKOS: &str = "http://www.w3.org/2004/02/skos/core#";
    /// The SKOS-XL namespace.
    pub const SKOSXL: &str = "http://www.w3.org/2008/05/skos-xl#";
    /// The RDF namespace.
    pub const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
    /// The RDFS namespace.
    pub const RDFS: &str = "http://www.w3.org/2000/01/rdf-schema#";
    /// The Dublin Core Terms namespace.
    pub const DCTERMS: &str = "http://purl.org/dc/terms/";
    /// The PROV-O namespace, used for the audit trail.
    pub const PROV: &str = "http://www.w3.org/ns/prov#";
}

/// The kind of label a term carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    /// `skos:prefLabel` — at most one per language, per concept.
    Preferred,
    /// `skos:altLabel` — synonyms and variants.
    Alternative,
    /// `skos:hiddenLabel` — matched by search, never displayed.
    Hidden,
}

impl LabelKind {
    /// The SKOS property IRI for this label kind.
    pub fn property_iri(self) -> String {
        let local = match self {
            LabelKind::Preferred => "prefLabel",
            LabelKind::Alternative => "altLabel",
            LabelKind::Hidden => "hiddenLabel",
        };
        format!("{}{local}", ns::SKOS)
    }

    /// Whether SKOS permits at most one of these per language on a concept.
    ///
    /// This is integrity condition S14: a resource has no more than one `skos:prefLabel` per
    /// language tag.
    pub fn is_unique_per_language(self) -> bool {
        matches!(self, LabelKind::Preferred)
    }
}

/// A language-tagged label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    /// The literal text.
    pub text: String,
    /// BCP 47 language tag.
    pub language: String,
    /// Which SKOS label property this is.
    pub kind: LabelKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_kinds_map_to_skos_properties() {
        assert_eq!(
            LabelKind::Preferred.property_iri(),
            "http://www.w3.org/2004/02/skos/core#prefLabel"
        );
        assert_eq!(
            LabelKind::Alternative.property_iri(),
            "http://www.w3.org/2004/02/skos/core#altLabel"
        );
        assert_eq!(
            LabelKind::Hidden.property_iri(),
            "http://www.w3.org/2004/02/skos/core#hiddenLabel"
        );
    }

    #[test]
    fn only_preferred_labels_are_unique_per_language() {
        // SKOS integrity condition S14.
        assert!(LabelKind::Preferred.is_unique_per_language());
        assert!(!LabelKind::Alternative.is_unique_per_language());
        assert!(!LabelKind::Hidden.is_unique_per_language());
    }
}
