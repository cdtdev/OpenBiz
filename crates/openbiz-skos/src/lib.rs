//! SKOS and SKOS-XL domain model — the product's core noun.
//!
//! SKOS-XL is **not optional** here. The documented ISO 25964 mapping relies on SKOS-XL wherever
//! the base SKOS model lacks a construct for a thesaurus feature, so plain SKOS cannot faithfully
//! represent an ISO 25964 thesaurus — and our enterprise buyers have ISO 25964 in their
//! requirements. See `docs/COMPETITIVE.md`.

mod ancestry;
mod deprecate;
mod equivalence;
mod fold;
mod hierarchy;
mod integrity;
mod labels;
mod mapping;
mod merge;
mod mint;
mod model;
mod notes;
mod paths;
mod refinement;
mod reinstate;
mod relations;
mod relocate;
mod search;
mod split;
mod status;
mod tree;
mod xl;

/// What the semantic relation model costs at 10k, 100k and 1M links, and what S24's transitive
/// closure would cost on top of it — counted before it is built. Test-only: it generates its own
/// vocabulary, and the sizes that take minutes are `#[ignore]`d. See
/// `docs/adr/0024-semantic-relation-closure-scale.md` for the numbers and the decision.
#[cfg(test)]
mod scale;

pub use ancestry::{Above, Ancestry};
pub use deprecate::{
    Deprecation, DeprecationError, DeprecationScan, DeprecationScanBuilder, StatusBound, Stranded,
    DCTERMS_IS_REPLACED_BY, OWL_DEPRECATED, XSD_BOOLEAN,
};
pub use equivalence::{EquivalenceBound, ExactMatchCluster};
pub use fold::fold;
pub use hierarchy::WalkBound;
pub use integrity::{
    newly_violated, Authority, Caveat, ConditionOutcome, Declaration, DeclaredRefinements,
    IntegrityCondition, RefinementScan, RefinementScanBound, UnreadRefinement, Verdict, CONDITIONS,
    RDFS_SUB_CLASS_OF,
};
pub use labels::{
    LabelKind, LanguageCoverage, LexicalLabel, RDF_LANG_STRING, SKOS_ALT_LABEL, SKOS_HIDDEN_LABEL,
    SKOS_PREF_LABEL, XSD_STRING,
};
pub use mapping::{
    ExactMatchDisjointness, MappingProperty, SKOS_BROAD_MATCH, SKOS_CLOSE_MATCH, SKOS_EXACT_MATCH,
    SKOS_MAPPING_RELATION, SKOS_NARROW_MATCH, SKOS_RELATED_MATCH,
};
pub use merge::{Demotion, Merge, MergeError, MergeScan, MergeScanBuilder, ReferenceBound};
pub use mint::{
    mint, slug, Evidence, HighestInUse, IriConvention, MintDerivation, MintError, MintPattern,
    MintPolicy, MintScan, Minted, NoConvention, PatternError, Placeholder, Slug, SlugBound,
    SlugError, Suggestion,
};
pub use model::{
    ClassOrigin, CoreModel, CoreModelBuilder, Derivation, Finding, ListDefect, Literal, MemberList,
    Node, RdfsRule, Resource, Rule, Severity, SkosClass, SkosRule, Statement, Term, RDF_FIRST,
    RDF_NIL, RDF_REST, RDF_TYPE, SKOS_HAS_TOP_CONCEPT, SKOS_IN_SCHEME, SKOS_MEMBER,
    SKOS_MEMBER_LIST, SKOS_TOP_CONCEPT_OF,
};
pub use notes::{
    DocumentationCoverage, NoteKind, NoteOrigin, SKOS_CHANGE_NOTE, SKOS_DEFINITION,
    SKOS_EDITORIAL_NOTE, SKOS_EXAMPLE, SKOS_HISTORY_NOTE, SKOS_NOTE, SKOS_SCOPE_NOTE,
};
pub use paths::{HierarchyCycle, Offered, PathBound, RootPath, RootPaths, RouteStep};
pub use refinement::{
    PropertyRefinements, PropertyRefinementsBuilder, RefinementBound, RefinementExhaustion,
    RDFS_SUB_PROPERTY_OF,
};
pub use reinstate::{
    Reinstatement, ReinstatementError, ReinstatementScan, ReinstatementScanBuilder,
};
pub use relations::{
    RelationOrigin, SemanticRelation, SKOS_BROADER, SKOS_BROADER_TRANSITIVE, SKOS_NARROWER,
    SKOS_NARROWER_TRANSITIVE, SKOS_RELATED, SKOS_SEMANTIC_RELATION,
};
pub use relocate::{Relocation, RelocationError};
pub use search::{
    LabelHit, LabelQuery, LabelSearch, LanguageFilter, LanguageRange, MatchMode, MatchQuality,
    QueryError, SearchBound,
};
pub use split::{
    Part, PartRequest, Placement, Split, SplitError, Unapportioned, PROV_WAS_DERIVED_FROM,
};
pub use status::{Retirement, Retirements, RetirementsBuilder};
pub use tree::{Descent, Pruned, Siblings};
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
    /// The OWL 2 namespace. Used for `owl:deprecated`, which SKOS has no equivalent of.
    pub const OWL: &str = "http://www.w3.org/2002/07/owl#";
}
