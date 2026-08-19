//! SKOS and SKOS-XL domain model — the product's core noun.
//!
//! SKOS-XL is **not optional** here. The documented ISO 25964 mapping relies on SKOS-XL wherever
//! the base SKOS model lacks a construct for a thesaurus feature, so plain SKOS cannot faithfully
//! represent an ISO 25964 thesaurus — and our enterprise buyers have ISO 25964 in their
//! requirements. See `docs/COMPETITIVE.md`.

mod ancestry;
mod labels;
mod model;
mod relations;
mod xl;

/// What the semantic relation model costs at 10k, 100k and 1M links, and what S24's transitive
/// closure would cost on top of it — counted before it is built. Test-only: it generates its own
/// vocabulary, and the sizes that take minutes are `#[ignore]`d. See
/// `docs/adr/0024-semantic-relation-closure-scale.md` for the numbers and the decision.
#[cfg(test)]
mod scale;

pub use ancestry::{Ancestry, AncestryBound};
pub use labels::{
    LabelKind, LanguageCoverage, LexicalLabel, RDF_LANG_STRING, SKOS_ALT_LABEL, SKOS_HIDDEN_LABEL,
    SKOS_PREF_LABEL, XSD_STRING,
};
pub use model::{
    ClassOrigin, CoreModel, CoreModelBuilder, Derivation, Finding, ListDefect, Literal, MemberList,
    Node, Resource, Severity, SkosClass, SkosRule, Statement, Term, RDF_FIRST, RDF_NIL, RDF_REST,
    RDF_TYPE, SKOS_HAS_TOP_CONCEPT, SKOS_IN_SCHEME, SKOS_MEMBER, SKOS_MEMBER_LIST,
    SKOS_TOP_CONCEPT_OF,
};
pub use relations::{
    RelationOrigin, SemanticRelation, SKOS_BROADER, SKOS_BROADER_TRANSITIVE, SKOS_NARROWER,
    SKOS_NARROWER_TRANSITIVE, SKOS_RELATED, SKOS_SEMANTIC_RELATION,
};
pub use xl::{
    LabelOrigin, SKOSXL_ALT_LABEL, SKOSXL_HIDDEN_LABEL, SKOSXL_LABEL, SKOSXL_LABEL_RELATION,
    SKOSXL_LITERAL_FORM, SKOSXL_PREF_LABEL,
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
