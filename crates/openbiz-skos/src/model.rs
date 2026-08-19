//! The SKOS core model — `skos:Concept`, `skos:ConceptScheme`, `skos:Collection`, and
//! `skos:OrderedCollection`.
//!
//! This module answers one question about a graph: **what is in it, in SKOS terms?** How many
//! concepts, which concept schemes, what is top of each, which collections there are and what
//! they contain — including an ordered collection's members *in order*, which no other structure
//! in RDF gives you for free.
//!
//! # It is engine-free on purpose
//!
//! Nothing here knows about Oxigraph, or about any RDF library. A caller feeds [`Statement`]s in
//! and gets a [`CoreModel`] out, so the model is testable from a literal array of statements and
//! the store is one of several possible sources rather than the only one. That is `CLAUDE.md` §3
//! applied downwards as well as upwards: the domain crate does not depend on the storage crate,
//! so a candidate's staging graph, a parsed file, or a discovery result can all be classified by
//! the same code.
//!
//! The cost is one type translation at the boundary — the store has its own engine-free statement
//! type and the caller maps between them. That is a real cost and it is the price of the layering;
//! see `docs/adr/0019`.
//!
//! # What it infers, and why every inference explains itself
//!
//! A graph that says `<C> skos:inScheme <S>` and never types `<S>` still *has* a concept scheme —
//! S4 says so. A reader that only counted `rdf:type` statements would report zero schemes for a
//! large fraction of real vocabularies, which is not a conservative answer but a wrong one.
//!
//! So the model applies the SKOS axioms that bear on class membership and on the two properties
//! that are defined in terms of others. Each one is small, each is quoted from the specification,
//! and **each derived fact carries its premise and its rule** — [`CoreModel::derivations`] is the
//! answer to `CLAUDE.md` §3's "never add an inference path that cannot explain itself". The rules
//! applied are [`SkosRule::S4`], [`S5`](SkosRule::S5), [`S6`](SkosRule::S6),
//! [`S7`](SkosRule::S7), [`S8`](SkosRule::S8), [`S29`](SkosRule::S29), [`S31`](SkosRule::S31),
//! [`S33`](SkosRule::S33), and [`S36`](SkosRule::S36).
//!
//! Two axioms are deliberately **not** applied. S32 gives `skos:member` a range that is a *union*
//! of two classes, which entails membership of neither — inferring `skos:Concept` from it would be
//! a guess wearing a citation. And nothing here reasons about labels, semantic relations, or
//! mapping properties; those are their own items in `docs/BUILD-PLAN.md`.
//!
//! # What it reports, and the distinction the specification itself draws
//!
//! [`CoreModel::findings`] separates two things the incumbents tend to blur:
//!
//! - **Inconsistent** — the graph violates a SKOS *integrity condition*. Only S9 and S37 are
//!   integrity conditions among the core classes, and the specification says so by putting them
//!   under a heading of that name. A vocabulary in this state is not a SKOS vocabulary. The
//!   SKOS-XL statements this module applies are classified in the [`xl`](crate::xl) module, which
//!   also records why: Appendix B has no "Integrity Conditions" heading at all, so two of its
//!   three inconsistencies are ours by reading and one is the specification's own word.
//! - **Ill-formed** — SKOS itself permits it, but it is almost certainly a mistake. A cyclic or
//!   truncated `rdf:List` behind `skos:memberList` is the case that matters. [RDF-SEMANTICS]
//!   §3.3.3, cited by the SKOS Reference §9.6.2, explicitly allows a semantic extension to impose
//!   well-formedness restrictions on the collection vocabulary; this is that, and it is reported
//!   as our judgement rather than dressed up as the specification's.
//!
//! Getting that the wrong way round is how a tool ends up refusing valid enterprise data. Two
//! `skos:memberList` values on one resource look like an error and *are* consistent with SKOS —
//! §9.6.2 and Example 43 explain why the functional-property axiom S35 cannot be used as an
//! integrity condition — so we report them and do not call them a violation.
//!
//! [RDF-SEMANTICS]: https://www.w3.org/TR/rdf-mt/

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::labels::{LabelKind, LanguageCoverage, LexicalLabel};
use crate::ns;
use crate::relations::{RelationOrigin, SemanticRelation, SKOS_SEMANTIC_RELATION};
use crate::xl::{LabelOrigin, SKOSXL_LABEL_RELATION, SKOSXL_LITERAL_FORM};

/// `rdf:type`.
pub const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
/// `rdf:first` — the item of an RDF collection cell.
pub const RDF_FIRST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#first";
/// `rdf:rest` — the tail of an RDF collection cell.
pub const RDF_REST: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#rest";
/// `rdf:nil` — the empty RDF collection, and the terminator of a well-formed one.
pub const RDF_NIL: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nil";

/// `skos:inScheme`.
pub const SKOS_IN_SCHEME: &str = "http://www.w3.org/2004/02/skos/core#inScheme";
/// `skos:hasTopConcept`.
pub const SKOS_HAS_TOP_CONCEPT: &str = "http://www.w3.org/2004/02/skos/core#hasTopConcept";
/// `skos:topConceptOf`.
pub const SKOS_TOP_CONCEPT_OF: &str = "http://www.w3.org/2004/02/skos/core#topConceptOf";
/// `skos:member`.
pub const SKOS_MEMBER: &str = "http://www.w3.org/2004/02/skos/core#member";
/// `skos:memberList`.
pub const SKOS_MEMBER_LIST: &str = "http://www.w3.org/2004/02/skos/core#memberList";

/// One of the four classes of the SKOS core model, or SKOS-XL's fifth.
///
/// `skosxl:Label` is here rather than in a model of its own because S48 makes it disjoint with
/// three of the four below, and a disjointness check can only be run over classes that share a
/// map. Its namespace is the one thing that differs, so [`SkosClass::iri`] asks the class rather
/// than assuming.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SkosClass {
    /// `skos:Concept` — a unit of thought. S1.
    Concept,
    /// `skos:ConceptScheme` — an aggregation of concepts. S2.
    ConceptScheme,
    /// `skos:Collection` — a labelled or ordered grouping, which is *not* a concept. S28, S37.
    Collection,
    /// `skos:OrderedCollection` — a collection whose members have a meaningful order. S28, S29.
    OrderedCollection,
    /// `skosxl:Label` — a label with an IRI of its own, so a thesaurus can say things about it.
    /// Appendix B.2, S47, S48, S52.
    Label,
}

impl SkosClass {
    /// Every class, in a stable order.
    pub const ALL: [SkosClass; 5] = [
        SkosClass::Concept,
        SkosClass::ConceptScheme,
        SkosClass::Collection,
        SkosClass::OrderedCollection,
        SkosClass::Label,
    ];

    /// The class's IRI.
    pub fn iri(self) -> String {
        format!("{}{}", self.namespace(), self.local_name())
    }

    /// The namespace the class is defined in — SKOS for four of them, SKOS-XL for `Label`.
    pub fn namespace(self) -> &'static str {
        match self {
            SkosClass::Label => ns::SKOSXL,
            _ => ns::SKOS,
        }
    }

    /// The prefix a report writes before the local name.
    fn prefix(self) -> &'static str {
        match self {
            SkosClass::Label => "skosxl",
            _ => "skos",
        }
    }

    /// The local name within the class's namespace.
    pub fn local_name(self) -> &'static str {
        match self {
            SkosClass::Concept => "Concept",
            SkosClass::ConceptScheme => "ConceptScheme",
            SkosClass::Collection => "Collection",
            SkosClass::OrderedCollection => "OrderedCollection",
            SkosClass::Label => "Label",
        }
    }

    /// The class an IRI names, or `None` if it names something outside the SKOS+XL data model.
    ///
    /// A vocabulary is full of `rdf:type` statements about classes that are not ours — `owl:Class`
    /// and `owl:Ontology` are both explicitly permitted alongside SKOS types by Examples 3 and 7 —
    /// so this returning `None` is the ordinary case, not an error.
    pub fn from_iri(iri: &str) -> Option<Self> {
        SkosClass::ALL
            .into_iter()
            .find(|class| iri.strip_prefix(class.namespace()) == Some(class.local_name()))
    }

    /// The class this one is a sub-class of, with the rule that says so.
    ///
    /// Only S29 — `skos:OrderedCollection` is a sub-class of `skos:Collection`. The other three
    /// stand alone; SKOS deliberately declines to relate `skos:Concept` to `owl:Class` (§3.5.1).
    pub fn super_class(self) -> Option<(SkosClass, SkosRule)> {
        match self {
            SkosClass::OrderedCollection => Some((SkosClass::Collection, SkosRule::S29)),
            _ => None,
        }
    }
}

impl fmt::Display for SkosClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.prefix(), self.local_name())
    }
}

/// A numbered statement of the SKOS Reference that this module relies on.
///
/// Carrying the number *and* the specification's own wording is the point. A derivation that says
/// "inferred" is a claim; one that says "S4: The rdfs:range of skos:inScheme is the class
/// skos:ConceptScheme" is something a governance team can take to an auditor, which is the gap
/// `CLAUDE.md` §3 names as the incumbents' weakest ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)] // Each variant's meaning is its `statement()`, quoted from the spec.
pub enum SkosRule {
    S4,
    S5,
    S6,
    S7,
    S8,
    S9,
    S12,
    S13,
    S14,
    S18,
    S19,
    S20,
    S21,
    S22,
    S23,
    S25,
    S26,
    S29,
    S31,
    S33,
    S36,
    S37,
    S3,
    S30,
    S48,
    S49,
    S50,
    S51,
    S52,
    S53,
    S54,
    S55,
    S56,
    S57,
    S58,
    S59,
    S60,
    S61,
    S62,
}

impl SkosRule {
    /// The statement's number, as the specification prints it.
    pub fn number(self) -> &'static str {
        match self {
            SkosRule::S3 => "S3",
            SkosRule::S4 => "S4",
            SkosRule::S5 => "S5",
            SkosRule::S6 => "S6",
            SkosRule::S7 => "S7",
            SkosRule::S8 => "S8",
            SkosRule::S9 => "S9",
            SkosRule::S12 => "S12",
            SkosRule::S13 => "S13",
            SkosRule::S14 => "S14",
            SkosRule::S18 => "S18",
            SkosRule::S19 => "S19",
            SkosRule::S20 => "S20",
            SkosRule::S21 => "S21",
            SkosRule::S22 => "S22",
            SkosRule::S23 => "S23",
            SkosRule::S25 => "S25",
            SkosRule::S26 => "S26",
            SkosRule::S29 => "S29",
            SkosRule::S30 => "S30",
            SkosRule::S31 => "S31",
            SkosRule::S33 => "S33",
            SkosRule::S36 => "S36",
            SkosRule::S37 => "S37",
            SkosRule::S48 => "S48",
            SkosRule::S49 => "S49",
            SkosRule::S50 => "S50",
            SkosRule::S51 => "S51",
            SkosRule::S52 => "S52",
            SkosRule::S53 => "S53",
            SkosRule::S54 => "S54",
            SkosRule::S55 => "S55",
            SkosRule::S56 => "S56",
            SkosRule::S57 => "S57",
            SkosRule::S58 => "S58",
            SkosRule::S59 => "S59",
            SkosRule::S60 => "S60",
            SkosRule::S61 => "S61",
            SkosRule::S62 => "S62",
        }
    }

    /// The statement, quoted from the SKOS Reference (W3C Recommendation, 18 August 2009).
    pub fn statement(self) -> &'static str {
        match self {
            SkosRule::S3 => {
                "skos:inScheme, skos:hasTopConcept and skos:topConceptOf are each instances of \
                 owl:ObjectProperty."
            }
            SkosRule::S4 => "The rdfs:range of skos:inScheme is the class skos:ConceptScheme.",
            SkosRule::S5 => {
                "The rdfs:domain of skos:hasTopConcept is the class skos:ConceptScheme."
            }
            SkosRule::S6 => "The rdfs:range of skos:hasTopConcept is the class skos:Concept.",
            SkosRule::S7 => "skos:topConceptOf is a sub-property of skos:inScheme.",
            SkosRule::S8 => "skos:topConceptOf is owl:inverseOf the property skos:hasTopConcept.",
            SkosRule::S9 => "skos:ConceptScheme is disjoint with skos:Concept.",
            SkosRule::S12 => {
                "The rdfs:range of each of skos:prefLabel, skos:altLabel and skos:hiddenLabel is \
                 the class of RDF plain literals."
            }
            SkosRule::S13 => {
                "skos:prefLabel, skos:altLabel and skos:hiddenLabel are pairwise disjoint \
                 properties."
            }
            SkosRule::S14 => {
                "A resource has no more than one value of skos:prefLabel per language tag."
            }
            SkosRule::S18 => {
                "skos:semanticRelation, skos:broader, skos:narrower, skos:related, \
                 skos:broaderTransitive and skos:narrowerTransitive are each instances of \
                 owl:ObjectProperty."
            }
            SkosRule::S19 => "The rdfs:domain of skos:semanticRelation is the class skos:Concept.",
            SkosRule::S20 => "The rdfs:range of skos:semanticRelation is the class skos:Concept.",
            SkosRule::S21 => {
                "skos:broaderTransitive, skos:narrowerTransitive and skos:related are each \
                 sub-properties of skos:semanticRelation."
            }
            SkosRule::S22 => {
                "skos:broader is a sub-property of skos:broaderTransitive, and skos:narrower is a \
                 sub-property of skos:narrowerTransitive."
            }
            SkosRule::S23 => "skos:related is an instance of owl:SymmetricProperty.",
            SkosRule::S25 => "skos:narrower is owl:inverseOf the property skos:broader.",
            SkosRule::S26 => {
                "skos:narrowerTransitive is owl:inverseOf the property skos:broaderTransitive."
            }
            SkosRule::S29 => "skos:OrderedCollection is a sub-class of skos:Collection.",
            SkosRule::S30 => {
                "skos:member and skos:memberList are each instances of owl:ObjectProperty."
            }
            SkosRule::S31 => "The rdfs:domain of skos:member is the class skos:Collection.",
            SkosRule::S33 => {
                "The rdfs:domain of skos:memberList is the class skos:OrderedCollection."
            }
            SkosRule::S36 => {
                "For any resource, every item in the list given as the value of the \
                 skos:memberList property is also a value of the skos:member property."
            }
            SkosRule::S37 => {
                "skos:Collection is disjoint with each of skos:Concept and skos:ConceptScheme."
            }
            SkosRule::S48 => {
                "skosxl:Label is disjoint with each of skos:Concept, skos:ConceptScheme and \
                 skos:Collection."
            }
            SkosRule::S49 => "skosxl:literalForm is an instance of owl:DatatypeProperty.",
            SkosRule::S50 => "The rdfs:domain of skosxl:literalForm is the class skosxl:Label.",
            SkosRule::S51 => {
                "The rdfs:range of skosxl:literalForm is the class of RDF plain literals."
            }
            SkosRule::S52 => {
                "skosxl:Label is a sub-class of a restriction on skosxl:literalForm cardinality \
                 exactly 1."
            }
            SkosRule::S53 => {
                "skosxl:prefLabel, skosxl:altLabel and skosxl:hiddenLabel are each instances of \
                 owl:ObjectProperty."
            }
            SkosRule::S54 => {
                "The rdfs:range of each of skosxl:prefLabel, skosxl:altLabel and \
                 skosxl:hiddenLabel is the class skosxl:Label."
            }
            SkosRule::S55 => {
                "The property chain (skosxl:prefLabel, skosxl:literalForm) is a sub-property of \
                 skos:prefLabel."
            }
            SkosRule::S56 => {
                "The property chain (skosxl:altLabel, skosxl:literalForm) is a sub-property of \
                 skos:altLabel."
            }
            SkosRule::S57 => {
                "The property chain (skosxl:hiddenLabel, skosxl:literalForm) is a sub-property of \
                 skos:hiddenLabel."
            }
            SkosRule::S58 => {
                "skosxl:prefLabel, skosxl:altLabel and skosxl:hiddenLabel are pairwise disjoint \
                 properties."
            }
            SkosRule::S59 => "skosxl:labelRelation is an instance of owl:ObjectProperty.",
            SkosRule::S60 => "The rdfs:domain of skosxl:labelRelation is the class skosxl:Label.",
            SkosRule::S61 => "The rdfs:range of skosxl:labelRelation is the class skosxl:Label.",
            SkosRule::S62 => "skosxl:labelRelation is an instance of owl:SymmetricProperty.",
        }
    }
}

impl fmt::Display for SkosRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.number(), self.statement())
    }
}

/// An IRI or a blank node — the two things an RDF statement can be *about*.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Node {
    /// An IRI.
    Iri(String),
    /// A blank node, identified by its label within the graph it was read from.
    Blank(String),
}

impl Node {
    /// An IRI node.
    pub fn iri(iri: impl Into<String>) -> Self {
        Node::Iri(iri.into())
    }

    /// A blank node with the given label.
    pub fn blank(label: impl Into<String>) -> Self {
        Node::Blank(label.into())
    }

    /// The IRI, if this is one.
    pub fn as_iri(&self) -> Option<&str> {
        match self {
            Node::Iri(iri) => Some(iri),
            Node::Blank(_) => None,
        }
    }

    /// Whether this is the IRI `iri`.
    pub fn is_iri(&self, iri: &str) -> bool {
        self.as_iri() == Some(iri)
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Iri(iri) => write!(f, "<{iri}>"),
            Node::Blank(label) => write!(f, "_:{label}"),
        }
    }
}

/// An RDF literal, kept whole because the label items that come next need the language tag.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Literal {
    /// The lexical form.
    pub value: String,
    /// The BCP 47 language tag, for a language-tagged string.
    pub language: Option<String>,
    /// The datatype IRI.
    pub datatype: String,
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.value)?;
        match &self.language {
            Some(language) => write!(f, "@{language}"),
            None => write!(f, "^^<{}>", self.datatype),
        }
    }
}

/// The object of a statement: a node or a literal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Term {
    /// An IRI or blank node.
    Node(Node),
    /// A literal.
    Literal(Literal),
}

impl Term {
    /// The node, if the object is one.
    pub fn as_node(&self) -> Option<&Node> {
        match self {
            Term::Node(node) => Some(node),
            Term::Literal(_) => None,
        }
    }
}

impl From<Node> for Term {
    fn from(node: Node) -> Self {
        Term::Node(node)
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::Node(node) => write!(f, "{node}"),
            Term::Literal(literal) => write!(f, "{literal}"),
        }
    }
}

/// One RDF statement, in the shape this module reads.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Statement {
    /// What the statement is about.
    pub subject: Node,
    /// The predicate IRI.
    pub predicate: String,
    /// What it says.
    pub object: Term,
}

impl Statement {
    /// A statement.
    pub fn new(subject: Node, predicate: impl Into<String>, object: impl Into<Term>) -> Self {
        Statement {
            subject,
            predicate: predicate.into(),
            object: object.into(),
        }
    }
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.subject,
            curie(&self.predicate),
            self.object
        )
    }
}

/// A predicate IRI written the short way where we know the namespace, and in full where we do not.
///
/// Only for human-readable output. Nothing parses these back.
fn curie(iri: &str) -> String {
    for (prefix, namespace) in [
        ("skos", ns::SKOS),
        ("skosxl", ns::SKOSXL),
        ("rdf", ns::RDF),
        ("rdfs", ns::RDFS),
    ] {
        if let Some(local) = iri.strip_prefix(namespace) {
            return format!("{prefix}:{local}");
        }
    }
    format!("<{iri}>")
}

/// How a resource came to be an instance of a class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClassOrigin {
    /// The graph said so, with `rdf:type`.
    Asserted,
    /// We concluded it, under this rule.
    Entailed(SkosRule),
}

impl fmt::Display for ClassOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClassOrigin::Asserted => write!(f, "asserted"),
            ClassOrigin::Entailed(rule) => write!(f, "inferred, {}", rule.number()),
        }
    }
}

/// A fact the model concluded, the statement it concluded it from, and the rule that licensed it.
///
/// A derivation's `premise` may itself have been derived — S8 produces a `skos:topConceptOf`
/// statement which S7 then turns into a `skos:inScheme` statement. The list is emitted in the
/// order the rules ran, so a chain reads downwards. It is not rendered as a tree, and that is
/// recorded in `docs/UNTESTED.md` rather than claimed as complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derivation {
    /// What was concluded.
    pub conclusion: String,
    /// The statement it followed from.
    pub premise: String,
    /// The specification statement that licensed the step.
    pub rule: SkosRule,
}

impl fmt::Display for Derivation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n    because {}\n    and {}",
            self.conclusion, self.premise, self.rule
        )
    }
}

/// How seriously to take a [`Finding`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// A SKOS **integrity condition** is violated. The graph is not consistent with SKOS.
    Inconsistent,
    /// SKOS permits it; we think it is a mistake. Our judgement, not the specification's.
    IllFormed,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Inconsistent => write!(f, "inconsistent"),
            Severity::IllFormed => write!(f, "ill-formed"),
        }
    }
}

/// What went wrong while walking the `rdf:List` behind a `skos:memberList`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListDefect {
    /// A cell was reached twice, so the list does not terminate.
    Cyclic {
        /// The cell reached for the second time.
        cell: Node,
    },
    /// A cell has no `rdf:first`, so it holds no item.
    NoFirst {
        /// The cell.
        cell: Node,
    },
    /// A cell has no `rdf:rest`, so the list stops without reaching `rdf:nil`.
    NoRest {
        /// The cell.
        cell: Node,
    },
    /// A cell has more than one `rdf:first` or `rdf:rest`, so it branches. See Example 43.
    Branches {
        /// The cell.
        cell: Node,
        /// The property with more than one value, as a CURIE.
        property: &'static str,
        /// How many values it has.
        values: usize,
    },
    /// An `rdf:first` or `rdf:rest` value is a literal, where the list vocabulary needs a node.
    NotANode {
        /// The cell.
        cell: Node,
        /// The property, as a CURIE.
        property: &'static str,
    },
}

impl fmt::Display for ListDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListDefect::Cyclic { cell } => {
                write!(f, "the list returns to {cell}, so it does not terminate")
            }
            ListDefect::NoFirst { cell } => write!(f, "{cell} has no rdf:first"),
            ListDefect::NoRest { cell } => {
                write!(
                    f,
                    "{cell} has no rdf:rest, so the list never reaches rdf:nil"
                )
            }
            ListDefect::Branches {
                cell,
                property,
                values,
            } => write!(f, "{cell} has {values} values for {property}"),
            ListDefect::NotANode { cell, property } => {
                write!(f, "the {property} of {cell} is a literal, not a node")
            }
        }
    }
}

/// Something the model noticed about the graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// A resource is an instance of two disjoint classes. S9 or S37.
    DisjointClasses {
        /// The resource.
        resource: Node,
        /// One class, and how the resource came to be in it.
        first: (SkosClass, ClassOrigin),
        /// The other.
        second: (SkosClass, ClassOrigin),
        /// The integrity condition violated.
        rule: SkosRule,
    },
    /// The `rdf:List` behind a `skos:memberList` is not well formed.
    ///
    /// No `skos:member` is inferred from a defective list: S36 is about "every item in the list",
    /// and a list we could not read to the end has no known set of items. Reporting the items read
    /// before the defect, and inferring nothing from them, is the honest half-answer.
    DefectiveMemberList {
        /// The collection whose list it is.
        collection: Node,
        /// The head of the list.
        head: Node,
        /// What went wrong.
        defect: ListDefect,
        /// How many items were read before it.
        read: usize,
    },
    /// A resource has more than one `skos:memberList`.
    ///
    /// **Consistent with SKOS**, despite S35 making `skos:memberList` functional: §9.6.2 and
    /// Example 43 explain that the condition cannot be enforced without also stating the two lists
    /// are different objects. Reported because it is almost always a mistake, and reported as ours.
    MultipleMemberLists {
        /// The resource.
        resource: Node,
        /// How many distinct list heads it names.
        lists: usize,
    },
    /// A resource has two or more `skos:prefLabel` values with the same language tag. S14.
    ///
    /// Per *tag*, not per language: `"color"@en` beside `"colour"@en-GB` is consistent, and
    /// §5.6.5 with Example 18 says so. The tags are compared lower-cased, which is the value
    /// space RDF 1.1 Concepts §3.3 defines.
    MultiplePreferredLabels {
        /// The over-labelled resource.
        resource: Node,
        /// The language tag they share, lower-cased, or `None` for untagged labels.
        language: Option<String>,
        /// The competing lexical forms, in a stable order.
        labels: Vec<String>,
    },
    /// One resource carries the same label under two of the three labelling properties. S13.
    ///
    /// The label is the same *RDF term* — same lexical form, same language tag. Example 19 is
    /// consistent precisely because `"love"@en` and `"love"@en-GB` are different terms.
    LabelPropertiesClash {
        /// The resource.
        resource: Node,
        /// The label they share.
        label: LexicalLabel,
        /// The properties that carry it, in a stable order.
        kinds: Vec<LabelKind>,
    },
    /// A labelling property was given something that is not an RDF plain literal. S12.
    ///
    /// **Not an integrity condition.** §5.4 lists exactly two of those and this is not one of
    /// them; §5.6.2 says of this case that "an application may reject such data but is not
    /// required to". We report it and read on, because refusing it would mean turning away data
    /// the standard permits. The value takes no part in S13 or S14: a term with no language tag
    /// and no claim to be a string cannot be put in the per-language buckets those are about.
    NonPlainLiteralLabel {
        /// The labelled resource.
        resource: Node,
        /// Which labelling property, as a CURIE.
        property: String,
        /// What it was given.
        value: Term,
    },
    /// An object property was given a literal value. S3, S30 or S53.
    LiteralOnObjectProperty {
        /// The subject.
        subject: Node,
        /// The property, as a CURIE.
        property: String,
        /// The literal it was given.
        literal: Literal,
        /// The specification statement that makes the property an object property.
        rule: SkosRule,
    },
    /// A datatype property was given an IRI or a blank node. S49.
    ///
    /// The mirror of [`Finding::LiteralOnObjectProperty`], and inconsistent for the same reason:
    /// the values of an `owl:DatatypeProperty` are literals by definition. This is *not* how a
    /// node under `skos:prefLabel` is treated, and the difference is the specification's — S10
    /// makes that an `owl:AnnotationProperty`, and OWL 2 annotation properties take IRIs legally.
    NodeOnDatatypeProperty {
        /// The subject.
        subject: Node,
        /// The property, as a CURIE.
        property: String,
        /// The node it was given.
        node: Node,
        /// The specification statement that makes the property a datatype property.
        rule: SkosRule,
    },
    /// An `skosxl:Label` has more than one `skosxl:literalForm`. S52.
    ///
    /// The specification's own word for this is "not consistent" — Examples 76, 77, 78 and 79 are
    /// each marked so, and 78 makes the point that `"love"@en-GB` and `"love"@en-US` are two forms
    /// and not one. A label has exactly one, so two cannot both be it.
    MultipleLiteralForms {
        /// The label resource.
        label: Node,
        /// The competing forms, in a stable order.
        forms: Vec<String>,
    },
    /// An `skosxl:Label` has no `skosxl:literalForm`. S52 — **and this is not an inconsistency.**
    ///
    /// "Cardinality exactly 1" entails that a form exists; under OWL's open-world assumption it
    /// does not require the graph to state it. A partial export or a half-finished import
    /// legitimately produces one. Reported because a label an author cannot read is still a
    /// problem they need told about, and reported as ours.
    NoLiteralForm {
        /// The label resource.
        label: Node,
    },
    /// A `skosxl:literalForm` was given a literal that is not an RDF plain literal. S51.
    ///
    /// Handled as S12's case is, and the analogy is ours: §5.6.2's "an application may reject such
    /// data but is not required to" is said about §5 and is not restated in Appendix B. The value
    /// is discarded, so the label dumbs down to nothing rather than to a plain label that would
    /// then be S12's finding one step later.
    NonPlainLiteralForm {
        /// The label resource.
        label: Node,
        /// What it was given.
        value: Literal,
    },
    /// One resource carries the same `skosxl:Label` under two XL labelling properties. S58.
    ///
    /// The same *resource*, not merely the same literal form: two distinct labels sharing a form
    /// are Example 85's case, which is caught by S13 after the chains have dumbed them down.
    XlLabelPropertiesClash {
        /// The labelled resource.
        resource: Node,
        /// The label resource they share.
        label: Node,
        /// The properties that carry it, in a stable order.
        kinds: Vec<LabelKind>,
    },
}

impl Finding {
    /// How seriously to take it.
    pub fn severity(&self) -> Severity {
        match self {
            Finding::DisjointClasses { .. } | Finding::LiteralOnObjectProperty { .. } => {
                Severity::Inconsistent
            }
            Finding::MultiplePreferredLabels { .. } | Finding::LabelPropertiesClash { .. } => {
                Severity::Inconsistent
            }
            // Appendix B has no "Integrity Conditions" heading, so two of these three are
            // classified by us. The specification marks Examples 76–79 "(not consistent)", which
            // settles `MultipleLiteralForms` outright; a violated disjointness is a contradiction
            // whatever heading it sits under. See the `xl` module.
            Finding::NodeOnDatatypeProperty { .. }
            | Finding::MultipleLiteralForms { .. }
            | Finding::XlLabelPropertiesClash { .. } => Severity::Inconsistent,
            Finding::DefectiveMemberList { .. }
            | Finding::MultipleMemberLists { .. }
            | Finding::NonPlainLiteralLabel { .. }
            | Finding::NoLiteralForm { .. }
            | Finding::NonPlainLiteralForm { .. } => Severity::IllFormed,
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Finding::DisjointClasses {
                resource,
                first,
                second,
                rule,
            } => write!(
                f,
                "{resource} is both {} ({}) and {} ({})\n    and {rule}",
                first.0, first.1, second.0, second.1
            ),
            Finding::DefectiveMemberList {
                collection,
                head,
                defect,
                read,
            } => write!(
                f,
                "the skos:memberList of {collection} starting at {head} is not a well-formed \
                 rdf:List: {defect}\n    {read} item(s) were read before it, and no skos:member \
                 was inferred from them",
            ),
            Finding::MultipleMemberLists { resource, lists } => write!(
                f,
                "{resource} has {lists} skos:memberList values\n    SKOS permits this — S35 makes \
                 skos:memberList functional but §9.6.2 explains why that cannot be an integrity \
                 condition — so this is our judgement, not the specification's",
            ),
            Finding::MultiplePreferredLabels {
                resource,
                language,
                labels,
            } => write!(
                f,
                "{resource} has {} skos:prefLabel values in {}: {}\n    and {}",
                labels.len(),
                match language {
                    Some(language) => format!("@{language}"),
                    None => "no language".to_owned(),
                },
                labels.join(", "),
                SkosRule::S14,
            ),
            Finding::LabelPropertiesClash {
                resource,
                label,
                kinds,
            } => write!(
                f,
                "{resource} carries {label} under {}\n    and {}",
                kinds
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" and "),
                SkosRule::S13,
            ),
            Finding::NonPlainLiteralLabel {
                resource,
                property,
                value,
            } => write!(
                f,
                "{resource} {property} {value}, which is not an RDF plain literal\n    and \
                 {}\n    SKOS permits this — \u{a7}5.4 lists two integrity conditions on labels \
                 and this is not one of them, and \u{a7}5.6.2 says an application \"may reject \
                 such data but is not required to\" — so this is our judgement, not the \
                 specification's",
                SkosRule::S12,
            ),
            Finding::LiteralOnObjectProperty {
                subject,
                property,
                literal,
                rule,
            } => write!(
                f,
                "{subject} {property} {literal}, but the value of an object property cannot be a \
                 literal\n    and {rule}",
            ),
            Finding::NodeOnDatatypeProperty {
                subject,
                property,
                node,
                rule,
            } => write!(
                f,
                "{subject} {property} {node}, but the value of a datatype property cannot be an \
                 IRI or a blank node\n    and {rule}",
            ),
            Finding::MultipleLiteralForms { label, forms } => write!(
                f,
                "{label} has {} skosxl:literalForm values: {}\n    and {}\n    Appendix B.2.3 \
                 marks Examples 76\u{2013}79 \"not consistent\" for exactly this, including \
                 \"love\"@en-GB beside \"love\"@en-US",
                forms.len(),
                forms.join(", "),
                SkosRule::S52,
            ),
            Finding::NoLiteralForm { label } => write!(
                f,
                "{label} is a skosxl:Label with no skosxl:literalForm, so nothing can read it\n    \
                 and {}\n    SKOS-XL permits this — \"cardinality exactly 1\" entails that a form \
                 exists but, under OWL's open-world assumption, does not require the graph to \
                 state it — so this is our judgement, not the specification's",
                SkosRule::S52,
            ),
            Finding::NonPlainLiteralForm { label, value } => write!(
                f,
                "the skosxl:literalForm of {label} is {value}, which is not an RDF plain \
                 literal\n    and {}\n    Appendix B states no integrity conditions and does not \
                 restate \u{a7}5.6.2's \"may reject such data but is not required to\", so \
                 treating this as S12's case is our judgement rather than the specification's",
                SkosRule::S51,
            ),
            Finding::XlLabelPropertiesClash {
                resource,
                label,
                kinds,
            } => write!(
                f,
                "{resource} carries {label} under {}\n    and {}",
                kinds
                    .iter()
                    .map(|kind| format!("skosxl:{}", kind.local_name()))
                    .collect::<Vec<_>>()
                    .join(" and "),
                SkosRule::S58,
            ),
        }
    }
}

/// A `skos:memberList` and what walking it produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberList {
    /// The head cell of the list.
    pub head: Node,
    /// The items, in list order, as far as the list could be read.
    pub items: Vec<Node>,
    /// What stopped the walk, if anything did.
    pub defect: Option<ListDefect>,
}

impl MemberList {
    /// Whether the list terminated at `rdf:nil` with no defect.
    pub fn is_well_formed(&self) -> bool {
        self.defect.is_none()
    }
}

/// One resource, and what the graph says about it in SKOS terms.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Resource {
    classes: BTreeMap<SkosClass, ClassOrigin>,
    in_schemes: BTreeSet<Node>,
    top_concept_of: BTreeSet<Node>,
    has_top_concept: BTreeSet<Node>,
    members: BTreeSet<Node>,
    member_lists: Vec<MemberList>,
    labels: BTreeMap<LexicalLabel, BTreeMap<LabelKind, LabelOrigin>>,
    literal_forms: BTreeSet<Literal>,
    xl_labels: BTreeMap<Node, BTreeSet<LabelKind>>,
    label_relations: BTreeMap<Node, RelationOrigin>,
    semantic_relations: BTreeMap<SemanticRelation, BTreeMap<Node, RelationOrigin>>,
}

impl Resource {
    /// The classes it is an instance of, and how each was established.
    pub fn classes(&self) -> &BTreeMap<SkosClass, ClassOrigin> {
        &self.classes
    }

    /// Whether it is an instance of `class`, asserted or inferred.
    pub fn is_a(&self, class: SkosClass) -> bool {
        self.classes.contains_key(&class)
    }

    /// The concept schemes it is in, including those inferred from `skos:topConceptOf` under S7.
    pub fn in_schemes(&self) -> &BTreeSet<Node> {
        &self.in_schemes
    }

    /// The schemes it is a top concept of, in both directions under S8.
    pub fn top_concept_of(&self) -> &BTreeSet<Node> {
        &self.top_concept_of
    }

    /// Its top concepts, if it is a scheme — in both directions under S8.
    pub fn has_top_concept(&self) -> &BTreeSet<Node> {
        &self.has_top_concept
    }

    /// Its members, including those inferred from a well-formed `skos:memberList` under S36.
    pub fn members(&self) -> &BTreeSet<Node> {
        &self.members
    }

    /// Its `skos:memberList` values. More than one is a [`Finding::MultipleMemberLists`].
    pub fn member_lists(&self) -> &[MemberList] {
        &self.member_lists
    }

    /// Every label it carries, which properties carry each one, and where each came from.
    ///
    /// Ordered by language tag and then by lexical form, so iterating groups a resource's labels
    /// by language. A label under more than one property is a [`Finding::LabelPropertiesClash`];
    /// the map keeps it once, with both kinds, rather than reporting it twice.
    ///
    /// **A label dumbed down from SKOS-XL is in here too**, carrying a
    /// [`LabelOrigin::DumbedDown`]. That is what makes Appendix B's Examples 84–87 come out
    /// inconsistent — B.3.4.2 says they are inconsistent because of S13 and S14, which are
    /// conditions on the labels this map holds.
    pub fn labels(&self) -> &BTreeMap<LexicalLabel, BTreeMap<LabelKind, LabelOrigin>> {
        &self.labels
    }

    /// Its labels of one kind, in the same order, whichever way they arrived.
    pub fn labels_of(&self, kind: LabelKind) -> impl Iterator<Item = &LexicalLabel> {
        self.labels
            .iter()
            .filter(move |(_, kinds)| kinds.contains_key(&kind))
            .map(|(label, _)| label)
    }

    /// Where one of its labels came from, if it carries that label under that property.
    pub fn label_origin(&self, label: &LexicalLabel, kind: LabelKind) -> Option<LabelOrigin> {
        self.labels.get(label)?.get(&kind).copied()
    }

    /// The values of `skosxl:literalForm` on this resource, if it is an `skosxl:Label`.
    ///
    /// Raw literals, not [`LexicalLabel`]s, and deliberately: S52 counts *values of the property*,
    /// so a form that is not a plain literal still competes for the one slot the restriction
    /// allows. It is a [`Finding::NonPlainLiteralForm`] as well, and it does not dumb down.
    pub fn literal_forms(&self) -> &BTreeSet<Literal> {
        &self.literal_forms
    }

    /// The `skosxl:Label` resources this one is labelled with, and under which properties.
    ///
    /// Empty for a vocabulary authored in plain SKOS, which is most of them.
    pub fn xl_labels(&self) -> &BTreeMap<Node, BTreeSet<LabelKind>> {
        &self.xl_labels
    }

    /// Its `skosxl:Label` resources of one kind, in a stable order.
    pub fn xl_labels_of(&self, kind: LabelKind) -> impl Iterator<Item = &Node> {
        self.xl_labels
            .iter()
            .filter(move |(_, kinds)| kinds.contains(&kind))
            .map(|(label, _)| label)
    }

    /// The labels this one is linked to by `skosxl:labelRelation`, and how each link arrived.
    ///
    /// **Closed under S62**, which makes the property symmetric: a graph that states the link one
    /// way has it both ways here, and the direction it did not state carries
    /// [`RelationOrigin::Entailed`]. Non-empty only on a `skosxl:Label`, because S60 and S61 make
    /// both ends of a link one — asserting a link from a `skos:Concept` does not put the link
    /// somewhere else, it makes that concept a label as well and so violates S48.
    ///
    /// A *refinement* of `skosxl:labelRelation` — Example 89's `ex:acronym` — is **not** in here.
    /// See the [`xl`](crate::LabelOrigin) module: we read no `rdfs:subPropertyOf`, and closing a
    /// refinement would be wrong even if we did.
    pub fn label_relations(&self) -> &BTreeMap<Node, RelationOrigin> {
        &self.label_relations
    }

    /// The concepts this one is linked to under one semantic relation, and how each link arrived.
    ///
    /// Closed under **S25** and **S26** (the two inverse pairs), **S23** (`skos:related` is
    /// symmetric) and **S22** (each direction is also its transitive variant), so a hierarchy an
    /// author wrote downwards reads upwards too and every link appears under four properties. Each
    /// entry says which of those statements produced it, or that the graph stated it outright.
    ///
    /// **Not closed under S24.** `skos:broaderTransitive` holds one-step links only — those S22
    /// lifted from `skos:broader` and those the graph stated itself. Walking an ancestor chain is
    /// the next build-plan item; a caller that treats this map as the closure will get a partial
    /// answer, which is why the accessor is named for the property and not for "ancestors".
    pub fn relations(&self, relation: SemanticRelation) -> Option<&BTreeMap<Node, RelationOrigin>> {
        self.semantic_relations.get(&relation)
    }

    /// Every semantic relation this resource takes part in, in a stable order.
    pub fn semantic_relations(
        &self,
    ) -> &BTreeMap<SemanticRelation, BTreeMap<Node, RelationOrigin>> {
        &self.semantic_relations
    }

    /// How many concepts it has as broader concepts.
    ///
    /// More than one is **polyhierarchy**, which is ordinary in a thesaurus and is never a
    /// [`Finding`]: §8 states nothing against it and ISO 25964 relies on it. Counted because an
    /// author migrating from a strictly-monohierarchical source wants the number.
    pub fn broader_count(&self) -> usize {
        self.relations(SemanticRelation::Broader)
            .map_or(0, BTreeMap::len)
    }

    /// Its preferred label in `language`, if it has one.
    ///
    /// The tag is matched exactly, lower-cased — `en` does not answer a request for `en-GB`, and
    /// §5.6.5 is explicit that they are different tags. BCP 47's "lookup" fallback, which §5.6.5
    /// *suggests* an application implement, is deliberately not done here: it is a presentation
    /// policy, it needs a configured preference order to be useful, and doing it silently would
    /// mean a caller asking for French sometimes getting English with no way to tell.
    pub fn preferred_label_in(&self, language: &str) -> Option<&LexicalLabel> {
        self.labels_of(LabelKind::Preferred)
            .find(|label| label.is_in(language))
    }

    /// A label to show when nothing has said which language it wants.
    ///
    /// The first preferred label in language-tag order, or failing that the first alternative —
    /// §5.6.4 says a resource may have alternatives and no preferred label, and showing its IRI
    /// when it has a perfectly good `skos:altLabel` would be a worse answer than an imprecise one.
    ///
    /// **Deterministic but arbitrary across languages**, which is why every caller in this build
    /// prints the tag beside it. A configured display-language order is a separate decision and
    /// is recorded in `docs/UNTESTED.md` rather than guessed at here.
    pub fn display_label(&self) -> Option<&LexicalLabel> {
        self.labels_of(LabelKind::Preferred)
            .next()
            .or_else(|| self.labels_of(LabelKind::Alternative).next())
    }
}

/// Everything the SKOS core model can say about one graph.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoreModel {
    resources: BTreeMap<Node, Resource>,
    derivations: Vec<Derivation>,
    findings: Vec<Finding>,
    statements_read: usize,
}

impl CoreModel {
    /// Start reading a graph.
    pub fn builder() -> CoreModelBuilder {
        CoreModelBuilder::default()
    }

    /// Read a graph that is already in hand.
    pub fn from_statements(statements: impl IntoIterator<Item = Statement>) -> Self {
        let mut builder = CoreModel::builder();
        for statement in statements {
            builder.push(statement);
        }
        builder.build()
    }

    /// How many statements were offered, including the ones the model ignored.
    pub fn statements_read(&self) -> usize {
        self.statements_read
    }

    /// Every resource the model has something to say about, in a stable order.
    pub fn resources(&self) -> impl Iterator<Item = (&Node, &Resource)> {
        self.resources.iter()
    }

    /// One resource, if the graph mentioned it.
    pub fn resource(&self, node: &Node) -> Option<&Resource> {
        self.resources.get(node)
    }

    /// The resources that are instances of `class`, in a stable order.
    pub fn instances_of(&self, class: SkosClass) -> impl Iterator<Item = (&Node, &Resource)> {
        self.resources
            .iter()
            .filter(move |(_, resource)| resource.is_a(class))
    }

    /// How many instances of `class` the graph has.
    pub fn count_of(&self, class: SkosClass) -> usize {
        self.instances_of(class).count()
    }

    /// Every fact the model concluded, with its premise and its rule.
    pub fn derivations(&self) -> &[Derivation] {
        &self.derivations
    }

    /// Everything the model noticed about the graph.
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    /// How many labels of each kind the graph carries, per language tag.
    ///
    /// Ordered by tag, with untagged labels first. This is the shape of the question a
    /// multilingual programme asks — *which languages is this thesaurus actually in, and how far
    /// behind is each one?* — and it is counts rather than labels, so its size is the number of
    /// languages and not the size of the vocabulary.
    pub fn label_coverage(&self) -> Vec<LanguageCoverage> {
        let mut by_language: BTreeMap<Option<String>, LanguageCoverage> = BTreeMap::new();
        for resource in self.resources.values() {
            let mut preferred_here: BTreeSet<Option<String>> = BTreeSet::new();
            for (label, kinds) in &resource.labels {
                let entry = by_language
                    .entry(label.language.clone())
                    .or_insert_with(|| LanguageCoverage {
                        language: label.language.clone(),
                        preferred: 0,
                        alternative: 0,
                        hidden: 0,
                        resources_with_preferred: 0,
                    });
                for kind in kinds.keys() {
                    match kind {
                        LabelKind::Preferred => {
                            entry.preferred += 1;
                            preferred_here.insert(label.language.clone());
                        }
                        LabelKind::Alternative => entry.alternative += 1,
                        LabelKind::Hidden => entry.hidden += 1,
                    }
                }
            }
            for language in preferred_here {
                if let Some(entry) = by_language.get_mut(&language) {
                    entry.resources_with_preferred += 1;
                }
            }
        }
        by_language.into_values().collect()
    }

    /// Whether any finding says the graph violates a SKOS integrity condition.
    pub fn is_consistent(&self) -> bool {
        !self
            .findings
            .iter()
            .any(|finding| finding.severity() == Severity::Inconsistent)
    }
}

/// Reads statements one at a time and resolves the model when it has them all.
///
/// Incremental because a graph does not fit in memory twice. The store hands statements over as it
/// scans, and a vocabulary's notes and its non-SKOS statements, which are most of its statements,
/// are counted and dropped.
///
/// # What it keeps is not small
///
/// This used to say that what is kept is proportional to the resources the model has something to
/// say about rather than to the size of the graph. That stopped being true when the semantic
/// relations landed, and it is corrected here rather than left reading well: **every stated
/// `skos:broader` costs about 3.9 KiB of resident memory** — four `(Node, RelationOrigin)` entries
/// and three [`Derivation`]s, each derivation holding two eagerly-rendered strings — against
/// 0.70 KiB for a typed concept that states nothing. A million-link vocabulary measured 4.4 GiB.
///
/// The numbers, the decomposition, and what is and is not being done about them are in
/// `docs/adr/0024-semantic-relation-closure-scale.md`; the harness that produced them is this
/// crate's `scale` module. The one thing that ADR settles is that **S24's transitive closure is
/// never added to this** — it is answered by traversal on read, because a legal chain of 100 000
/// links would otherwise materialise five thousand million pairs.
#[derive(Debug, Clone, Default)]
pub struct CoreModelBuilder {
    types: BTreeMap<Node, BTreeSet<SkosClass>>,
    in_scheme: BTreeSet<(Node, Node)>,
    top_concept_of: BTreeSet<(Node, Node)>,
    has_top_concept: BTreeSet<(Node, Node)>,
    member: BTreeSet<(Node, Node)>,
    member_list: BTreeMap<Node, BTreeSet<Node>>,
    first: BTreeMap<Node, Vec<Term>>,
    rest: BTreeMap<Node, Vec<Term>>,
    labels: BTreeMap<Node, BTreeMap<LexicalLabel, BTreeSet<LabelKind>>>,
    literal_form: BTreeMap<Node, BTreeSet<Literal>>,
    xl_labels: BTreeMap<Node, BTreeMap<Node, BTreeSet<LabelKind>>>,
    label_relations: BTreeSet<(Node, Node)>,
    semantic_relations: BTreeMap<SemanticRelation, BTreeSet<(Node, Node)>>,
    semantic_relation_ends: BTreeSet<(Node, Node)>,
    findings: Vec<Finding>,
    statements_read: usize,
}

impl CoreModelBuilder {
    /// Offer one statement. Anything outside the core model is counted and discarded.
    pub fn push(&mut self, statement: Statement) {
        self.statements_read += 1;
        let Statement {
            subject,
            predicate,
            object,
        } = statement;

        match predicate.as_str() {
            RDF_TYPE => {
                if let Some(class) = object
                    .as_node()
                    .and_then(Node::as_iri)
                    .and_then(SkosClass::from_iri)
                {
                    self.types.entry(subject).or_default().insert(class);
                }
            }
            SKOS_IN_SCHEME => {
                self.object_property(subject, &predicate, object, SkosRule::S3, |b, s, o| {
                    b.in_scheme.insert((s, o));
                })
            }
            SKOS_TOP_CONCEPT_OF => {
                self.object_property(subject, &predicate, object, SkosRule::S3, |b, s, o| {
                    b.top_concept_of.insert((s, o));
                })
            }
            SKOS_HAS_TOP_CONCEPT => {
                self.object_property(subject, &predicate, object, SkosRule::S3, |b, s, o| {
                    b.has_top_concept.insert((s, o));
                })
            }
            SKOS_MEMBER => {
                self.object_property(subject, &predicate, object, SkosRule::S30, |b, s, o| {
                    b.member.insert((s, o));
                })
            }
            SKOS_MEMBER_LIST => {
                self.object_property(subject, &predicate, object, SkosRule::S30, |b, s, o| {
                    b.member_list.entry(s).or_default().insert(o);
                })
            }
            SKOSXL_LITERAL_FORM => match object {
                // S49 makes this an owl:DatatypeProperty, so a node here is a contradiction and
                // not merely odd — unlike `skos:prefLabel`, which S10 makes an annotation
                // property. The value is dropped, so it competes for neither S52's one slot nor
                // S51's plainness: it was never a literal to begin with.
                Term::Node(node) => self.findings.push(Finding::NodeOnDatatypeProperty {
                    subject,
                    property: curie(&predicate),
                    node,
                    rule: SkosRule::S49,
                }),
                Term::Literal(literal) => {
                    self.literal_form
                        .entry(subject)
                        .or_default()
                        .insert(literal);
                }
            },
            SKOSXL_LABEL_RELATION => {
                self.object_property(subject, &predicate, object, SkosRule::S59, |b, s, o| {
                    b.label_relations.insert((s, o));
                })
            }
            SKOS_SEMANTIC_RELATION => {
                // The super-property. S18 refuses a literal on it and S19/S20 type both ends, but
                // there is no sub-property it could be filed under: the entailment runs upwards,
                // and from `<A> skos:semanticRelation <B>` nothing follows about which of the five
                // holds. So it is kept only as a pair for the class rules to read.
                self.object_property(subject, &predicate, object, SkosRule::S18, |b, s, o| {
                    b.semantic_relation_ends.insert((s, o));
                })
            }
            _ if SemanticRelation::from_iri(&predicate).is_some() => {
                // Unreachable `None` — the guard has already matched the IRI. Written as a `let`
                // for the reason the label arm below gives.
                if let Some(relation) = SemanticRelation::from_iri(&predicate) {
                    self.object_property(subject, &predicate, object, SkosRule::S18, |b, s, o| {
                        b.semantic_relations
                            .entry(relation)
                            .or_default()
                            .insert((s, o));
                    })
                }
            }
            _ if LabelKind::from_xl_iri(&predicate).is_some() => {
                // Unreachable `None`, as below: the guard has already matched the IRI.
                if let Some(kind) = LabelKind::from_xl_iri(&predicate) {
                    self.object_property(subject, &predicate, object, SkosRule::S53, |b, s, o| {
                        b.xl_labels
                            .entry(s)
                            .or_default()
                            .entry(o)
                            .or_default()
                            .insert(kind);
                    })
                }
            }
            _ if LabelKind::from_iri(&predicate).is_some() => {
                // Unreachable `None` — the guard has already matched the IRI. Written as a `let`
                // rather than an `unwrap()` because `CLAUDE.md` §6 forbids the latter outside
                // tests, and because a mis-edited guard should drop a label, not abort a scan.
                if let Some(kind) = LabelKind::from_iri(&predicate) {
                    self.label(subject, &predicate, kind, object);
                }
            }
            // The list vocabulary is RDF's, not SKOS's, so a literal `rdf:first` is legal RDF and
            // is not a SKOS finding. It becomes a `ListDefect` if it turns up in a list we walk,
            // and stays silent otherwise — plenty of graphs carry lists that are nothing to do
            // with `skos:memberList`.
            RDF_FIRST => self.first.entry(subject).or_default().push(object),
            RDF_REST => self.rest.entry(subject).or_default().push(object),
            _ => {}
        }
    }

    /// Record a statement whose object must be a node, or raise the finding if it is not.
    fn object_property(
        &mut self,
        subject: Node,
        predicate: &str,
        object: Term,
        rule: SkosRule,
        record: impl FnOnce(&mut Self, Node, Node),
    ) {
        match object {
            Term::Node(object) => record(self, subject, object),
            Term::Literal(literal) => self.findings.push(Finding::LiteralOnObjectProperty {
                subject,
                property: curie(predicate),
                literal,
                rule,
            }),
        }
    }

    /// Record a label, or raise S12's finding if the value is not one.
    ///
    /// A value that is not a plain literal is **not** kept: S13 asks whether two properties carry
    /// the same label and S14 asks how many preferred labels a language has, and a term that is
    /// neither a language-tagged string nor a string has no answer to either. Keeping it would
    /// mean inventing a bucket for it and then reporting a clash that the specification does not
    /// describe.
    fn label(&mut self, subject: Node, predicate: &str, kind: LabelKind, object: Term) {
        match LexicalLabel::of(&object) {
            Some(label) => {
                self.labels
                    .entry(subject)
                    .or_default()
                    .entry(label)
                    .or_default()
                    .insert(kind);
            }
            None => self.findings.push(Finding::NonPlainLiteralLabel {
                resource: subject,
                property: curie(predicate),
                value: object,
            }),
        }
    }

    /// Resolve what was read into a [`CoreModel`].
    pub fn build(mut self) -> CoreModel {
        let mut model = CoreModel {
            statements_read: self.statements_read,
            findings: std::mem::take(&mut self.findings),
            ..CoreModel::default()
        };

        for (node, classes) in std::mem::take(&mut self.types) {
            let entry = model.resources.entry(node).or_default();
            for class in classes {
                entry.classes.insert(class, ClassOrigin::Asserted);
            }
        }

        // S8 first: it is the only rule that produces statements the *other* property rules then
        // read, so running it later would make the model depend on the order the graph happened to
        // state its top concepts in.
        self.close_top_concepts(&mut model);
        self.apply_scheme_rules(&mut model);
        self.apply_collection_rules(&mut model);
        // S29 before S36 so that an ordered collection becomes a collection *because it is an
        // ordered collection*, which is the reason the specification gives in §9.6.1, rather than
        // because of a `skos:member` this pass is about to derive. Both citations would be true;
        // the shorter one is the one a reader can check.
        Self::entail_super_classes(&mut model);
        self.resolve_member_lists(&mut model);
        // The semantic relations close first and are typed second, which is the opposite order to
        // SKOS-XL's — and for a stated reason. S60 and S61 constrain `skosxl:labelRelation`
        // itself, so a label can be typed straight from the graph's own statement. S19 and S20
        // constrain `skos:semanticRelation`, which no author writes, so the citation for a
        // concept typed out of a `skos:broader` runs through S22 and S21 first and those two
        // steps must already be in the derivation list for it to read.
        self.close_semantic_relations(&mut model);
        self.apply_relation_class_rules(&mut model);
        self.attach_labels(&mut model);
        // The SKOS-XL passes run in the order the specification's dependencies do: the classes
        // first (S50, S54, S60, S61), then the symmetric closure of the links those classes came
        // from (S62), then the forms the classes constrain (S51, S52), then the chains that need a
        // form to dumb down (S55–S57, S58). S52's "no literal form" is why the class rules must
        // precede it: a label established only by a link is still a label with no form, and the
        // report should say so. Disjointness runs after all of it because
        // S48 has to see a `skosxl:Label` established by S50, S54, S60 or S61, not only by
        // `rdf:type`; the
        // label conditions run last because S13 and S14 are what the chains feed, and B.3.4.2
        // says Examples 84–87 are inconsistent *because of* them.
        self.apply_xl_class_rules(&mut model);
        self.close_label_relations(&mut model);
        self.resolve_literal_forms(&mut model);
        self.entail_dumbed_down_labels(&mut model);
        Self::check_disjointness(&mut model);
        Self::check_label_conditions(&mut model);

        model
    }

    /// S50, S54, S60 and S61 — the four rules that make something a `skosxl:Label` without an
    /// `rdf:type`.
    ///
    /// S50 is the domain of `skosxl:literalForm`; S54 is the range of the three XL labelling
    /// properties. Between them, Example 84 needs no `rdf:type` statement at all and still has
    /// two labels — which is why it is written without one. S60 and S61 are the domain and range
    /// of `skosxl:labelRelation`, so both ends of a link are labels too.
    ///
    /// The links are read as the graph **stated** them, before S62's closure, so that each end is
    /// classified by the statement that is actually in the file: the subject under S60, the object
    /// under S61. Running after the closure would give half the labels a citation resting on a
    /// statement we inferred, when a one-step citation resting on the graph's own was available —
    /// the same choice `apply_scheme_rules` records for S5 against S8-then-S7-then-S4.
    fn apply_xl_class_rules(&mut self, model: &mut CoreModel) {
        for (label, forms) in &self.literal_form {
            let Some(form) = forms.iter().next() else {
                continue;
            };
            entail_class(
                model,
                label,
                SkosClass::Label,
                SkosRule::S50,
                &format!("{label} skosxl:literalForm {form}"),
            );
        }

        for (resource, labels) in &self.xl_labels {
            for (label, kinds) in labels {
                let Some(kind) = kinds.iter().next() else {
                    continue;
                };
                entail_class(
                    model,
                    label,
                    SkosClass::Label,
                    SkosRule::S54,
                    &format!("{resource} skosxl:{} {label}", kind.local_name()),
                );
            }
        }

        for (from, to) in &self.label_relations {
            let premise = format!("{from} skosxl:labelRelation {to}");
            entail_class(model, from, SkosClass::Label, SkosRule::S60, &premise);
            entail_class(model, to, SkosClass::Label, SkosRule::S61, &premise);
        }
    }

    /// S62 — `skosxl:labelRelation` is symmetric, so a link entails its converse.
    ///
    /// The converse goes into the same map as the asserted direction, carrying a
    /// [`RelationOrigin`] that says which it is, for the reason every origin in this crate exists:
    /// a caller that cannot tell the graph's statement from ours has an answer and not an audit
    /// trail. A graph that states both directions gets two asserted links and no derivation — it
    /// said so, and claiming to have deduced it would be a derivation nobody needed.
    ///
    /// A link from a label to **itself** is its own converse. `owl:SymmetricProperty` says nothing
    /// against it and neither do we: it is inserted once, entails nothing, and is not a finding.
    /// Inventing an integrity condition Appendix B does not state is the failure `docs/COMPETITIVE.md`
    /// records against tools that are stricter than the standard.
    fn close_label_relations(&mut self, model: &mut CoreModel) {
        for (from, to) in &self.label_relations {
            model
                .resources
                .entry(from.clone())
                .or_default()
                .label_relations
                .insert(to.clone(), RelationOrigin::Asserted);
        }

        for (from, to) in &self.label_relations {
            if self.label_relations.contains(&(to.clone(), from.clone())) {
                continue;
            }
            model
                .resources
                .entry(to.clone())
                .or_default()
                .label_relations
                .insert(from.clone(), RelationOrigin::Entailed(SkosRule::S62));
            model.derivations.push(Derivation {
                conclusion: format!("{to} skosxl:labelRelation {from}"),
                premise: format!("{from} skosxl:labelRelation {to}"),
                rule: SkosRule::S62,
            });
        }
    }

    /// S22, S23, S25 and S26 — every link the graph stated, and the ones those four entail.
    ///
    /// Two passes, in this order and not the other:
    ///
    /// 1. **The inverses.** S25 pairs `skos:broader` with `skos:narrower`, S26 pairs the two
    ///    transitive variants, and S23 makes `skos:related` its own inverse. So a hierarchy an
    ///    author wrote downwards reads upwards, which is the whole reason SKOS defines both
    ///    directions rather than one.
    /// 2. **The sub-property lift.** S22 puts every `skos:broader` link under
    ///    `skos:broaderTransitive` and every `skos:narrower` link under
    ///    `skos:narrowerTransitive`. It runs *after* the inverses so that it lifts the converses
    ///    too: a graph stating only `<A> skos:broader <B>` ends with all four links, and one
    ///    stating only `<B> skos:narrower <A>` ends with the same four. Running it first would
    ///    make the model's answer depend on which direction the author happened to type.
    ///
    /// A direction the graph stated is never overwritten by one we derived, so a graph that
    /// states both directions has two asserted links and no derivation — symmetry means they are
    /// the same link, not two.
    ///
    /// **S24 is not applied here.** `skos:broaderTransitive` comes out of this holding one-step
    /// links only. See the [`relations`](crate::SemanticRelation) module.
    fn close_semantic_relations(&self, model: &mut CoreModel) {
        for (relation, links) in &self.semantic_relations {
            for (from, to) in links {
                model
                    .resources
                    .entry(from.clone())
                    .or_default()
                    .semantic_relations
                    .entry(*relation)
                    .or_default()
                    .insert(to.clone(), RelationOrigin::Asserted);
            }
        }

        // What holds after each pass, so the second lifts what the first derived. Asserted links
        // are in here from the start, which is what stops a derivation being recorded for a
        // direction the graph already stated.
        let mut held: BTreeMap<SemanticRelation, BTreeSet<(Node, Node)>> =
            self.semantic_relations.clone();
        let mut derived: Vec<(SemanticRelation, Node, Node, SkosRule, String)> = Vec::new();

        for (relation, links) in &self.semantic_relations {
            let (inverse, rule) = relation.inverse();
            for (from, to) in links {
                if held
                    .entry(inverse)
                    .or_default()
                    .insert((to.clone(), from.clone()))
                {
                    derived.push((
                        inverse,
                        to.clone(),
                        from.clone(),
                        rule,
                        format!("{from} {relation} {to}"),
                    ));
                }
            }
        }

        // Over every relation and not over the two that have a variant, so that
        // `transitive_variant` is the single place that decides which do — a table saying
        // `skos:related` lifts into the hierarchy would otherwise be wrong in the table and right
        // in the closure, and only the table's own test would notice.
        for relation in SemanticRelation::ALL {
            let Some((variant, rule)) = relation.transitive_variant() else {
                continue;
            };
            let links = held.get(&relation).cloned().unwrap_or_default();
            for (from, to) in links {
                if held
                    .entry(variant)
                    .or_default()
                    .insert((from.clone(), to.clone()))
                {
                    let premise = format!("{from} {relation} {to}");
                    derived.push((variant, from, to, rule, premise));
                }
            }
        }

        for (relation, from, to, rule, premise) in derived {
            model
                .resources
                .entry(from.clone())
                .or_default()
                .semantic_relations
                .entry(relation)
                .or_default()
                .insert(to.clone(), RelationOrigin::Entailed(rule));
            model.derivations.push(Derivation {
                conclusion: format!("{from} {relation} {to}"),
                premise,
                rule,
            });
        }
    }

    /// S19 and S20 — both ends of a semantic relation are `skos:Concept`, by way of S21.
    ///
    /// Like S60 and S61 before them these usually report **nothing**: a vocabulary that types its
    /// concepts already has the answer, and the pass is silent. What they do is make a mistake
    /// visible. A `skos:broader` pointing at a `skos:Collection` types that collection as a
    /// concept, and S37's disjointness then says so — without the domain and range the same graph
    /// reads as clean, because nothing else in it would ever type the collection.
    ///
    /// **The citation runs through the super-property, because that is where the rule lives.**
    /// S19 constrains `skos:semanticRelation`, not `skos:broader`, so a report that cited S19
    /// against a `skos:broader` statement would name a statement that does not mention the
    /// property the author used. The chain is therefore printed: the S22 lift is already in the
    /// derivation list from the pass above, this adds the S21 step to `skos:semanticRelation`,
    /// and S19 and S20 conclude from that.
    ///
    /// The S21 step is recorded **only when a class actually follows from it**. Emitting it for
    /// every link in the vocabulary would double the derivation list to state something no reader
    /// asked about; emitting none would leave the class entailment citing a premise that appears
    /// nowhere. A graph that states `skos:semanticRelation` outright needs no step at all.
    fn apply_relation_class_rules(&self, model: &mut CoreModel) {
        let mut links: Vec<(Node, Node, Option<SemanticRelation>)> = Vec::new();
        for (relation, pairs) in &self.semantic_relations {
            // The property the S21 step will cite: the transitive variant for the two base
            // directions, because S21 does not name them, and the relation itself for the three
            // it does name.
            let via = relation
                .transitive_variant()
                .map_or(*relation, |(variant, _)| variant);
            for (from, to) in pairs {
                links.push((from.clone(), to.clone(), Some(via)));
            }
        }
        for (from, to) in &self.semantic_relation_ends {
            links.push((from.clone(), to.clone(), None));
        }

        for (from, to, via) in links {
            let typed = |model: &CoreModel, node: &Node| {
                model
                    .resources
                    .get(node)
                    .is_some_and(|resource| resource.is_a(SkosClass::Concept))
            };
            if typed(model, &from) && typed(model, &to) {
                continue;
            }
            let conclusion = format!("{from} skos:semanticRelation {to}");
            if let Some(via) = via {
                model.derivations.push(Derivation {
                    conclusion: conclusion.clone(),
                    premise: format!("{from} {via} {to}"),
                    rule: SemanticRelation::semantic_relation_rule(via),
                });
            }
            entail_class(model, &from, SkosClass::Concept, SkosRule::S19, &conclusion);
            entail_class(model, &to, SkosClass::Concept, SkosRule::S20, &conclusion);
        }
    }

    /// S51 and S52 — what a label's literal form must be, and how many of them there are.
    ///
    /// Runs over every resource that is an `skosxl:Label`, so the "no literal form" case reaches
    /// a label established by S54 as well as one stated with `rdf:type`. A resource carrying a
    /// `skosxl:literalForm` is a label under S50, so the two sets are the same set.
    fn resolve_literal_forms(&mut self, model: &mut CoreModel) {
        let mut found = Vec::new();
        let labels: Vec<Node> = model
            .instances_of(SkosClass::Label)
            .map(|(node, _)| node.clone())
            .collect();

        for label in labels {
            let forms = self.literal_form.remove(&label).unwrap_or_default();

            if forms.is_empty() {
                found.push(Finding::NoLiteralForm {
                    label: label.clone(),
                });
            } else if forms.len() > 1 {
                found.push(Finding::MultipleLiteralForms {
                    label: label.clone(),
                    forms: forms.iter().map(ToString::to_string).collect(),
                });
            }

            for form in &forms {
                if LexicalLabel::of_literal(form).is_none() {
                    found.push(Finding::NonPlainLiteralForm {
                        label: label.clone(),
                        value: form.clone(),
                    });
                }
            }

            model.resources.entry(label).or_default().literal_forms = forms;
        }

        model.findings.extend(found);
    }

    /// S55, S56, S57 — dumbing an XL label down to the plain SKOS label it stands for.
    ///
    /// Only a **well-formed** label dumbs down: one plain literal form, no more and no fewer. A
    /// label with two forms would otherwise produce two plain labels and so a second, derived
    /// finding for a fault already reported once, and there is no principled way to choose which
    /// of the two the concept is really called.
    ///
    /// S58 is checked here because it needs the same map: a label resource carried under two of
    /// the three XL properties, which is a different fault from two label resources sharing a
    /// literal form (Example 85, caught by S13 after this pass).
    fn entail_dumbed_down_labels(&mut self, model: &mut CoreModel) {
        for (resource, labels) in std::mem::take(&mut self.xl_labels) {
            for (label, kinds) in labels {
                if kinds.len() > 1 {
                    model.findings.push(Finding::XlLabelPropertiesClash {
                        resource: resource.clone(),
                        label: label.clone(),
                        kinds: kinds.iter().copied().collect(),
                    });
                }

                let form = model
                    .resources
                    .get(&label)
                    .filter(|held| held.literal_forms.len() == 1)
                    .and_then(|held| held.literal_forms.iter().next())
                    .and_then(LexicalLabel::of_literal);

                model
                    .resources
                    .entry(resource.clone())
                    .or_default()
                    .xl_labels
                    .insert(label.clone(), kinds.clone());

                let Some(plain) = form else {
                    continue;
                };
                for kind in kinds {
                    let rule = kind.dumbing_down_rule();
                    // An asserted label is never overwritten by a dumbed-down one, exactly as an
                    // asserted class is never overwritten by an entailed one: the graph said it,
                    // so claiming to have deduced it would be a derivation nobody needed.
                    let concluded = {
                        let held = model
                            .resources
                            .entry(resource.clone())
                            .or_default()
                            .labels
                            .entry(plain.clone())
                            .or_default();
                        match held.entry(kind) {
                            std::collections::btree_map::Entry::Vacant(slot) => {
                                slot.insert(LabelOrigin::DumbedDown(rule));
                                true
                            }
                            std::collections::btree_map::Entry::Occupied(_) => false,
                        }
                    };
                    if concluded {
                        model.derivations.push(Derivation {
                            conclusion: format!("{resource} skos:{} {plain}", kind.local_name()),
                            premise: format!(
                                "{resource} skosxl:{} {label}, whose skosxl:literalForm is {plain}",
                                kind.local_name()
                            ),
                            rule,
                        });
                    }
                }
            }
        }
    }

    /// Hand each resource the labels read for it.
    ///
    /// Labels entail no class. §5.6.1 states that the three properties have **no domain**, so
    /// their effective domain is `rdfs:Resource` — Example 16 labels an `owl:Class` and is
    /// consistent. A model that made a `skos:Concept` out of anything with a `skos:prefLabel`
    /// would miscount every vocabulary that labels its own concept scheme, which is most of them.
    fn attach_labels(&mut self, model: &mut CoreModel) {
        for (node, labels) in std::mem::take(&mut self.labels) {
            model.resources.entry(node).or_default().labels = labels
                .into_iter()
                .map(|(label, kinds)| {
                    let origins = kinds
                        .into_iter()
                        .map(|kind| (kind, LabelOrigin::Asserted))
                        .collect();
                    (label, origins)
                })
                .collect();
        }
    }

    /// S13 and S14 — the two integrity conditions the specification states on lexical labels.
    ///
    /// Both are per resource, and neither is affected by anything inferred, so this runs last and
    /// reads only what was asserted.
    fn check_label_conditions(model: &mut CoreModel) {
        let mut found = Vec::new();
        for (node, resource) in &model.resources {
            // S13: the three properties are pairwise disjoint, so one label under two of them is
            // a violation. The map is keyed by the label, so a clash is a key with two kinds —
            // no pairwise comparison is needed, and none is done.
            for (label, kinds) in &resource.labels {
                if kinds.len() > 1 {
                    found.push(Finding::LabelPropertiesClash {
                        resource: node.clone(),
                        label: label.clone(),
                        kinds: kinds.keys().copied().collect(),
                    });
                }
            }

            // S14: at most one preferred label per language tag. Untagged labels are one bucket
            // of their own — the condition says "per language tag", and a resource with two
            // untagged preferred labels has two values for a tag that happens to be absent.
            let mut by_language: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
            for label in resource.labels_of(LabelKind::Preferred) {
                by_language
                    .entry(label.language.clone())
                    .or_default()
                    .push(format!("{label}"));
            }
            for (language, labels) in by_language {
                if labels.len() > 1 {
                    found.push(Finding::MultiplePreferredLabels {
                        resource: node.clone(),
                        language,
                        labels,
                    });
                }
            }
        }
        model.findings.extend(found);
    }

    /// S8 — `skos:topConceptOf` and `skos:hasTopConcept` are inverses, so each implies the other.
    fn close_top_concepts(&mut self, model: &mut CoreModel) {
        for (concept, scheme) in self.top_concept_of.clone() {
            if self
                .has_top_concept
                .insert((scheme.clone(), concept.clone()))
            {
                model.derivations.push(Derivation {
                    conclusion: format!("{scheme} skos:hasTopConcept {concept}"),
                    premise: format!("{concept} skos:topConceptOf {scheme}"),
                    rule: SkosRule::S8,
                });
            }
        }
        for (scheme, concept) in self.has_top_concept.clone() {
            if self
                .top_concept_of
                .insert((concept.clone(), scheme.clone()))
            {
                model.derivations.push(Derivation {
                    conclusion: format!("{concept} skos:topConceptOf {scheme}"),
                    premise: format!("{scheme} skos:hasTopConcept {concept}"),
                    rule: SkosRule::S8,
                });
            }
        }

        for (scheme, concept) in &self.has_top_concept {
            model
                .resources
                .entry(scheme.clone())
                .or_default()
                .has_top_concept
                .insert(concept.clone());
            model
                .resources
                .entry(concept.clone())
                .or_default()
                .top_concept_of
                .insert(scheme.clone());
        }
    }

    /// S5, S6, S7, S4 — everything that follows from the scheme properties.
    ///
    /// The direct domain and range rules run **before** the sub-property chain, and the order is
    /// the difference between two true explanations. `<S> skos:hasTopConcept <C>` makes `<S>` a
    /// concept scheme in one step under S5; it also does so in three — S8 to `skos:topConceptOf`,
    /// S7 to `skos:inScheme`, S4 to the class. Both are sound. The one-step citation is the one a
    /// person can check against the specification without holding three rules in their head, so it
    /// is the one recorded.
    fn apply_scheme_rules(&mut self, model: &mut CoreModel) {
        for (scheme, concept) in &self.has_top_concept {
            // S5: the domain of skos:hasTopConcept is skos:ConceptScheme.
            entail_class(
                model,
                scheme,
                SkosClass::ConceptScheme,
                SkosRule::S5,
                &format!("{scheme} skos:hasTopConcept {concept}"),
            );
            // S6: its range is skos:Concept.
            entail_class(
                model,
                concept,
                SkosClass::Concept,
                SkosRule::S6,
                &format!("{scheme} skos:hasTopConcept {concept}"),
            );
        }

        // S7: topConceptOf is a sub-property of inScheme.
        for (concept, scheme) in self.top_concept_of.clone() {
            if self.in_scheme.insert((concept.clone(), scheme.clone())) {
                model.derivations.push(Derivation {
                    conclusion: format!("{concept} skos:inScheme {scheme}"),
                    premise: format!("{concept} skos:topConceptOf {scheme}"),
                    rule: SkosRule::S7,
                });
            }
        }

        for (concept, scheme) in &self.in_scheme {
            model
                .resources
                .entry(concept.clone())
                .or_default()
                .in_schemes
                .insert(scheme.clone());
            // S4: the range of skos:inScheme is skos:ConceptScheme.
            entail_class(
                model,
                scheme,
                SkosClass::ConceptScheme,
                SkosRule::S4,
                &format!("{concept} skos:inScheme {scheme}"),
            );
        }
    }

    /// S31, S33 — the domains of the two collection properties.
    ///
    /// S32 is **not** applied: the range of `skos:member` is a *union* of `skos:Concept` and
    /// `skos:Collection`, and a union entails membership of neither disjunct. Concluding either
    /// would be a guess with a citation attached to it, which is worse than concluding nothing.
    fn apply_collection_rules(&mut self, model: &mut CoreModel) {
        for (collection, member) in &self.member {
            model
                .resources
                .entry(collection.clone())
                .or_default()
                .members
                .insert(member.clone());
            entail_class(
                model,
                collection,
                SkosClass::Collection,
                SkosRule::S31,
                &format!("{collection} skos:member {member}"),
            );
        }

        for (collection, heads) in &self.member_list {
            let Some(head) = heads.iter().next() else {
                continue;
            };
            entail_class(
                model,
                collection,
                SkosClass::OrderedCollection,
                SkosRule::S33,
                &format!("{collection} skos:memberList {head}"),
            );
            if heads.len() > 1 {
                model.findings.push(Finding::MultipleMemberLists {
                    resource: collection.clone(),
                    lists: heads.len(),
                });
            }
        }
    }

    /// Walk each `skos:memberList` and apply S36 to the ones that are well formed.
    fn resolve_member_lists(&mut self, model: &mut CoreModel) {
        for (collection, heads) in std::mem::take(&mut self.member_list) {
            for head in heads {
                let list = self.walk_list(&head);

                if let Some(defect) = list.defect.clone() {
                    model.findings.push(Finding::DefectiveMemberList {
                        collection: collection.clone(),
                        head: head.clone(),
                        defect,
                        read: list.items.len(),
                    });
                } else {
                    // S36: every item in the list is also a value of skos:member.
                    for item in &list.items {
                        let entry = model.resources.entry(collection.clone()).or_default();
                        if entry.members.insert(item.clone()) {
                            model.derivations.push(Derivation {
                                conclusion: format!("{collection} skos:member {item}"),
                                premise: format!(
                                    "{collection} skos:memberList {head}, whose items include \
                                     {item}"
                                ),
                                rule: SkosRule::S36,
                            });
                        }
                    }
                }

                model
                    .resources
                    .entry(collection.clone())
                    .or_default()
                    .member_lists
                    .push(list);
            }
        }
    }

    /// Follow `rdf:first`/`rdf:rest` from `head`, stopping at `rdf:nil` or at the first defect.
    fn walk_list(&self, head: &Node) -> MemberList {
        let mut items = Vec::new();
        let mut seen = BTreeSet::new();
        let mut cell = head.clone();
        let mut defect = None;

        loop {
            if cell.is_iri(RDF_NIL) {
                break;
            }
            if !seen.insert(cell.clone()) {
                defect = Some(ListDefect::Cyclic { cell });
                break;
            }

            match self.single(&self.first, &cell, "rdf:first") {
                Ok(item) => items.push(item),
                Err(found) => {
                    defect = Some(found);
                    break;
                }
            }
            match self.single(&self.rest, &cell, "rdf:rest") {
                Ok(next) => cell = next,
                Err(found) => {
                    defect = Some(found);
                    break;
                }
            }
        }

        MemberList {
            head: head.clone(),
            items,
            defect,
        }
    }

    /// The one node value of a list property on a cell, or what is wrong with it.
    fn single(
        &self,
        values: &BTreeMap<Node, Vec<Term>>,
        cell: &Node,
        property: &'static str,
    ) -> Result<Node, ListDefect> {
        let Some(found) = values.get(cell) else {
            return Err(if property == "rdf:first" {
                ListDefect::NoFirst { cell: cell.clone() }
            } else {
                ListDefect::NoRest { cell: cell.clone() }
            });
        };
        if found.len() > 1 {
            return Err(ListDefect::Branches {
                cell: cell.clone(),
                property,
                values: found.len(),
            });
        }
        found[0]
            .as_node()
            .cloned()
            .ok_or_else(|| ListDefect::NotANode {
                cell: cell.clone(),
                property,
            })
    }

    /// S29 — an ordered collection is a collection.
    fn entail_super_classes(model: &mut CoreModel) {
        let ordered: Vec<Node> = model
            .instances_of(SkosClass::OrderedCollection)
            .map(|(node, _)| node.clone())
            .collect();
        for node in ordered {
            entail_class(
                model,
                &node,
                SkosClass::Collection,
                SkosRule::S29,
                &format!("{node} rdf:type skos:OrderedCollection"),
            );
        }
    }

    /// S9 and S37 — the two integrity conditions among the core classes.
    fn check_disjointness(model: &mut CoreModel) {
        const DISJOINT: [(SkosClass, SkosClass, SkosRule); 6] = [
            (SkosClass::ConceptScheme, SkosClass::Concept, SkosRule::S9),
            (SkosClass::Collection, SkosClass::Concept, SkosRule::S37),
            (
                SkosClass::Collection,
                SkosClass::ConceptScheme,
                SkosRule::S37,
            ),
            // S48. `skos:OrderedCollection` is absent on purpose and is not an omission: it is a
            // sub-class of `skos:Collection` under S29, which this pass has already entailed, so
            // an ordered collection that is also a label is caught by the row above with the
            // citation the specification actually states.
            (SkosClass::Label, SkosClass::Concept, SkosRule::S48),
            (SkosClass::Label, SkosClass::ConceptScheme, SkosRule::S48),
            (SkosClass::Label, SkosClass::Collection, SkosRule::S48),
        ];

        let mut found = Vec::new();
        for (node, resource) in &model.resources {
            for (left, right, rule) in DISJOINT {
                if let (Some(&first), Some(&second)) =
                    (resource.classes.get(&left), resource.classes.get(&right))
                {
                    found.push(Finding::DisjointClasses {
                        resource: node.clone(),
                        first: (left, first),
                        second: (right, second),
                        rule,
                    });
                }
            }
        }
        model.findings.extend(found);
    }
}

/// Conclude that `node` is an instance of `class`, unless the graph already said so.
///
/// An asserted class is never overwritten by an inferred one: the derivation list should not claim
/// to have concluded something the graph stated outright.
///
/// Where more than one rule licenses the same class — S4 and S5 both make a scheme out of the
/// object of a `skos:inScheme` that also has a top concept — the first to reach the conclusion is
/// the one recorded. Both citations are true; the model does not go looking for the others, and a
/// caller should not depend on which appears. Recorded in `docs/UNTESTED.md`.
fn entail_class(
    model: &mut CoreModel,
    node: &Node,
    class: SkosClass,
    rule: SkosRule,
    premise: &str,
) {
    let entry = model.resources.entry(node.clone()).or_default();
    if entry.classes.contains_key(&class) {
        return;
    }
    entry.classes.insert(class, ClassOrigin::Entailed(rule));
    model.derivations.push(Derivation {
        conclusion: format!("{node} rdf:type {class}"),
        premise: premise.to_owned(),
        rule,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The SKOS Reference's example namespace, so a test reads like the specification's own text.
    const EX: &str = "http://example.com/ns/";

    fn ex(local: &str) -> Node {
        Node::iri(format!("{EX}{local}"))
    }

    fn skos(local: &str) -> String {
        format!("{}{local}", ns::SKOS)
    }

    /// `<subject> <predicate> <object>`, where the object is a node.
    fn s(subject: &Node, predicate: &str, object: &Node) -> Statement {
        Statement::new(subject.clone(), predicate.to_owned(), object.clone())
    }

    /// `<subject> rdf:type skos:<class>`.
    fn typed(subject: &Node, class: SkosClass) -> Statement {
        Statement::new(subject.clone(), RDF_TYPE, Node::iri(class.iri()))
    }

    fn plain(value: &str) -> Term {
        Term::Literal(Literal {
            value: value.to_owned(),
            language: None,
            datatype: "http://www.w3.org/2001/XMLSchema#string".to_owned(),
        })
    }

    /// An RDF collection as the Turtle `( ... )` syntax expands it: one blank node cell per item.
    ///
    /// The tests use this rather than writing the cells out because the specification writes its
    /// examples in the sugar, and expanding it by hand in each test is where a typo would hide.
    fn rdf_list(prefix: &str, items: &[Node]) -> (Node, Vec<Statement>) {
        let mut statements = Vec::new();
        let cells: Vec<Node> = (0..items.len())
            .map(|index| Node::blank(format!("{prefix}{index}")))
            .collect();
        for (index, item) in items.iter().enumerate() {
            statements.push(Statement::new(
                cells[index].clone(),
                RDF_FIRST,
                item.clone(),
            ));
            let rest = cells
                .get(index + 1)
                .cloned()
                .unwrap_or_else(|| Node::iri(RDF_NIL));
            statements.push(Statement::new(cells[index].clone(), RDF_REST, rest));
        }
        (
            cells.first().cloned().unwrap_or_else(|| Node::iri(RDF_NIL)),
            statements,
        )
    }

    fn names(nodes: &BTreeSet<Node>) -> Vec<String> {
        nodes.iter().map(Node::to_string).collect()
    }

    #[test]
    fn the_property_constants_match_the_namespace_they_claim_to_be_in() {
        // A typo in one of these would make the model silently ignore a property, which is the
        // failure mode that looks exactly like "the vocabulary does not use it".
        for (constant, local) in [
            (SKOS_IN_SCHEME, "inScheme"),
            (SKOS_HAS_TOP_CONCEPT, "hasTopConcept"),
            (SKOS_TOP_CONCEPT_OF, "topConceptOf"),
            (SKOS_MEMBER, "member"),
            (SKOS_MEMBER_LIST, "memberList"),
        ] {
            assert_eq!(constant, skos(local), "skos:{local}");
        }
        for (constant, local) in [
            (RDF_TYPE, "type"),
            (RDF_FIRST, "first"),
            (RDF_REST, "rest"),
            (RDF_NIL, "nil"),
        ] {
            assert_eq!(constant, format!("{}{local}", ns::RDF), "rdf:{local}");
        }
    }

    #[test]
    fn every_core_class_round_trips_through_its_iri() {
        for class in SkosClass::ALL {
            assert_eq!(SkosClass::from_iri(&class.iri()), Some(class), "{class}");
        }
        assert_eq!(
            SkosClass::from_iri("http://www.w3.org/2002/07/owl#Class"),
            None
        );
        // The two namespaces are not interchangeable. `skosxl:Label` is the class; there is no
        // `skos:Label`, and reading one as the other would put a label in the SKOS core model.
        assert_eq!(
            SkosClass::from_iri(&format!("{}Label", ns::SKOSXL)),
            Some(SkosClass::Label)
        );
        assert_eq!(SkosClass::from_iri(&skos("Label")), None);
        assert_eq!(
            SkosClass::from_iri(&format!("{}Concept", ns::SKOSXL)),
            None,
            "skos:Concept is not in the SKOS-XL namespace"
        );
        assert_eq!(SkosClass::Label.to_string(), "skosxl:Label");
    }

    // --- The specification's own examples -------------------------------------------------

    #[test]
    fn example_2_a_typed_concept_is_a_concept() {
        // <MyConcept> rdf:type skos:Concept .
        let model = CoreModel::from_statements([typed(&ex("MyConcept"), SkosClass::Concept)]);

        assert_eq!(model.count_of(SkosClass::Concept), 1);
        assert_eq!(
            model.resource(&ex("MyConcept")).unwrap().classes()[&SkosClass::Concept],
            ClassOrigin::Asserted
        );
        assert!(model.findings().is_empty(), "{:?}", model.findings());
    }

    #[test]
    fn examples_3_and_4_a_concept_may_also_be_an_owl_class_or_property() {
        // <MyConcept> rdf:type skos:Concept , owl:Class .
        // <MyConcept> rdf:type skos:Concept , owl:ObjectProperty .
        // §3.5.1: SKOS deliberately states no relationship between skos:Concept and either, so
        // neither graph may be reported as a problem.
        for other in [
            "http://www.w3.org/2002/07/owl#Class",
            "http://www.w3.org/2002/07/owl#ObjectProperty",
        ] {
            let model = CoreModel::from_statements([
                typed(&ex("MyConcept"), SkosClass::Concept),
                Statement::new(ex("MyConcept"), RDF_TYPE, Node::iri(other)),
            ]);
            assert_eq!(model.count_of(SkosClass::Concept), 1, "{other}");
            assert!(model.is_consistent(), "{other}: {:?}", model.findings());
        }
    }

    #[test]
    fn example_5_a_scheme_its_top_concept_and_a_concept_in_it() {
        // <MyScheme> rdf:type skos:ConceptScheme ; skos:hasTopConcept <MyConcept> .
        // <MyConcept> skos:topConceptOf <MyScheme> .
        // <AnotherConcept> skos:inScheme <MyScheme> .
        let model = CoreModel::from_statements([
            typed(&ex("MyScheme"), SkosClass::ConceptScheme),
            s(&ex("MyScheme"), SKOS_HAS_TOP_CONCEPT, &ex("MyConcept")),
            s(&ex("MyConcept"), SKOS_TOP_CONCEPT_OF, &ex("MyScheme")),
            s(&ex("AnotherConcept"), SKOS_IN_SCHEME, &ex("MyScheme")),
        ]);

        assert_eq!(model.count_of(SkosClass::ConceptScheme), 1);
        let scheme = model.resource(&ex("MyScheme")).unwrap();
        assert_eq!(
            names(scheme.has_top_concept()),
            [ex("MyConcept").to_string()]
        );

        // S7 turns topConceptOf into inScheme, so the top concept is in the scheme without the
        // graph having to say so twice.
        let top = model.resource(&ex("MyConcept")).unwrap();
        assert_eq!(names(top.in_schemes()), [ex("MyScheme").to_string()]);
        assert!(model.derivations().iter().any(|derivation| {
            derivation.rule == SkosRule::S7
                && derivation.conclusion.contains("MyConcept")
                && derivation.conclusion.contains("skos:inScheme")
        }));

        // **`<AnotherConcept>` is not a concept.** §4.6.5 states that skos:inScheme has *no*
        // domain, so nothing follows about its subject. A reader that guessed here would be
        // inventing types for every resource anyone ever filed under a scheme.
        assert!(!model
            .resource(&ex("AnotherConcept"))
            .unwrap()
            .is_a(SkosClass::Concept));
        assert_eq!(model.count_of(SkosClass::Concept), 1);
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    #[test]
    fn example_6_a_concept_may_be_in_two_schemes() {
        let model = CoreModel::from_statements([
            typed(&ex("MyScheme"), SkosClass::ConceptScheme),
            typed(&ex("AnotherScheme"), SkosClass::ConceptScheme),
            s(&ex("MyConcept"), SKOS_IN_SCHEME, &ex("MyScheme")),
            s(&ex("MyConcept"), SKOS_IN_SCHEME, &ex("AnotherScheme")),
        ]);

        assert_eq!(
            names(model.resource(&ex("MyConcept")).unwrap().in_schemes()),
            [ex("AnotherScheme").to_string(), ex("MyScheme").to_string()]
        );
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    #[test]
    fn example_7_a_scheme_may_also_be_an_owl_ontology() {
        let model = CoreModel::from_statements([
            typed(&ex("MyScheme"), SkosClass::ConceptScheme),
            Statement::new(
                ex("MyScheme"),
                RDF_TYPE,
                Node::iri("http://www.w3.org/2002/07/owl#Ontology"),
            ),
            s(&ex("MyConcept"), SKOS_IN_SCHEME, &ex("MyScheme")),
        ]);
        assert_eq!(model.count_of(SkosClass::ConceptScheme), 1);
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    #[test]
    fn example_8_an_untyped_scheme_and_top_concept_are_still_found() {
        // <MyScheme> skos:hasTopConcept <MyConcept> . — no rdf:type anywhere.
        // S5 gives the scheme its class and S6 gives the concept its class, and the graph is
        // consistent even though §4.6.3 says it breaks the usage convention for hasTopConcept.
        let model = CoreModel::from_statements([
            s(&ex("MyScheme"), SKOS_HAS_TOP_CONCEPT, &ex("MyConcept")),
            s(&ex("AnotherConcept"), SKOS_IN_SCHEME, &ex("MyScheme")),
        ]);

        let scheme = model.resource(&ex("MyScheme")).unwrap();
        // Two rules license this — S5 from the hasTopConcept and S4 from the inScheme — and the
        // model records the first one to reach the conclusion rather than searching for every
        // reason. What is guaranteed is that it is *inferred* and says which rule it used; which
        // of two equally true citations appears is not something a caller may rely on.
        assert!(matches!(
            scheme.classes()[&SkosClass::ConceptScheme],
            ClassOrigin::Entailed(SkosRule::S4 | SkosRule::S5)
        ));
        let concept = model.resource(&ex("MyConcept")).unwrap();
        assert_eq!(
            concept.classes()[&SkosClass::Concept],
            ClassOrigin::Entailed(SkosRule::S6)
        );
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    #[test]
    fn s5_alone_finds_a_scheme_from_a_top_concept() {
        // Isolated from S4: nothing here says anything is *in* the scheme, so hasTopConcept's
        // domain is the only route to the class.
        let model = CoreModel::from_statements([s(
            &ex("MyScheme"),
            SKOS_HAS_TOP_CONCEPT,
            &ex("MyConcept"),
        )]);
        assert_eq!(
            model.resource(&ex("MyScheme")).unwrap().classes()[&SkosClass::ConceptScheme],
            ClassOrigin::Entailed(SkosRule::S5)
        );
    }

    #[test]
    fn s4_finds_a_scheme_nothing_typed() {
        // The common real-world shape: concepts say what scheme they are in and the scheme itself
        // is never typed. Counting only rdf:type would report zero schemes.
        let model = CoreModel::from_statements([
            typed(&ex("A"), SkosClass::Concept),
            s(&ex("A"), SKOS_IN_SCHEME, &ex("MyScheme")),
        ]);
        assert_eq!(
            model.resource(&ex("MyScheme")).unwrap().classes()[&SkosClass::ConceptScheme],
            ClassOrigin::Entailed(SkosRule::S4)
        );
    }

    #[test]
    fn example_40_a_collection_has_members_and_they_are_not_typed_by_it() {
        // <MyCollection> rdf:type skos:Collection ; skos:member <X> , <Y> , <Z> .
        let members = [ex("X"), ex("Y"), ex("Z")];
        let mut statements = vec![typed(&ex("MyCollection"), SkosClass::Collection)];
        statements.extend(
            members
                .iter()
                .map(|member| s(&ex("MyCollection"), SKOS_MEMBER, member)),
        );
        let model = CoreModel::from_statements(statements);

        let collection = model.resource(&ex("MyCollection")).unwrap();
        assert_eq!(collection.members().len(), 3);

        // S32 gives skos:member a range that is the *union* of skos:Concept and skos:Collection.
        // A union entails neither disjunct, so <X> gets no class at all — inferring skos:Concept
        // here would be a guess with a citation stapled to it.
        for member in &members {
            let classes = model
                .resource(member)
                .map(|resource| resource.classes().len())
                .unwrap_or_default();
            assert_eq!(classes, 0, "{member} was given a class");
        }
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    #[test]
    fn examples_41_and_42_an_ordered_collection_keeps_its_order_and_entails_its_members() {
        // <MyOrderedCollection> rdf:type skos:OrderedCollection ; skos:memberList ( <X> <Y> <Z> ) .
        // entails
        // <MyOrderedCollection> rdf:type skos:Collection ; skos:member <X> , <Y> , <Z> .
        let items = [ex("X"), ex("Y"), ex("Z")];
        let (head, mut statements) = rdf_list("cell", &items);
        statements.push(typed(
            &ex("MyOrderedCollection"),
            SkosClass::OrderedCollection,
        ));
        statements.push(s(&ex("MyOrderedCollection"), SKOS_MEMBER_LIST, &head));
        let model = CoreModel::from_statements(statements);

        let collection = model.resource(&ex("MyOrderedCollection")).unwrap();

        // The order is the whole point of an ordered collection, and it is the one thing a plain
        // set of skos:member statements cannot carry.
        assert_eq!(collection.member_lists().len(), 1);
        assert_eq!(collection.member_lists()[0].items, items);
        assert!(collection.member_lists()[0].is_well_formed());

        // Example 42's stated entailment, both halves.
        assert_eq!(
            collection.classes()[&SkosClass::Collection],
            ClassOrigin::Entailed(SkosRule::S29)
        );
        assert_eq!(
            names(collection.members()),
            names(&items.iter().cloned().collect())
        );

        // And it explains itself: S36 for the members, S29 for the class.
        let rules: BTreeSet<SkosRule> = model
            .derivations()
            .iter()
            .map(|derivation| derivation.rule)
            .collect();
        assert!(rules.contains(&SkosRule::S36), "{:?}", model.derivations());
        assert!(rules.contains(&SkosRule::S29), "{:?}", model.derivations());
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    #[test]
    fn example_43_two_member_lists_are_consistent_and_still_worth_saying() {
        // <OrderedCollectionResource> skos:memberList ( <A> <B> ) , ( <X> <Y> ) .
        // §9.6.2: S35 makes skos:memberList functional, but that cannot be used as an integrity
        // condition without stating the two lists are different objects. So this is consistent —
        // and reporting it as a violation would be us overriding the specification.
        let (first_head, mut statements) = rdf_list("a", &[ex("A"), ex("B")]);
        let (second_head, second) = rdf_list("b", &[ex("X"), ex("Y")]);
        statements.extend(second);
        statements.push(s(&ex("Resource"), SKOS_MEMBER_LIST, &first_head));
        statements.push(s(&ex("Resource"), SKOS_MEMBER_LIST, &second_head));
        let model = CoreModel::from_statements(statements);

        assert!(model.is_consistent(), "{:?}", model.findings());
        let finding = model
            .findings()
            .iter()
            .find(|finding| matches!(finding, Finding::MultipleMemberLists { .. }))
            .expect("two member lists should be reported");
        assert_eq!(finding.severity(), Severity::IllFormed);

        // Both lists are kept, in a stable order, rather than one silently winning.
        let resource = model.resource(&ex("Resource")).unwrap();
        assert_eq!(resource.member_lists().len(), 2);
        assert_eq!(resource.members().len(), 4);
    }

    #[test]
    fn example_44_a_collection_may_contain_a_collection() {
        let model = CoreModel::from_statements([
            typed(&ex("MyCollection"), SkosClass::Collection),
            s(&ex("MyCollection"), SKOS_MEMBER, &ex("A")),
            s(&ex("MyCollection"), SKOS_MEMBER, &ex("MyNestedCollection")),
            typed(&ex("MyNestedCollection"), SkosClass::Collection),
            s(&ex("MyNestedCollection"), SKOS_MEMBER, &ex("X")),
        ]);
        assert_eq!(model.count_of(SkosClass::Collection), 2);
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    // --- The integrity conditions ---------------------------------------------------------

    #[test]
    fn s9_a_resource_cannot_be_a_concept_and_a_concept_scheme() {
        let model = CoreModel::from_statements([
            typed(&ex("Both"), SkosClass::Concept),
            typed(&ex("Both"), SkosClass::ConceptScheme),
        ]);

        assert!(!model.is_consistent());
        let Finding::DisjointClasses {
            rule,
            first,
            second,
            ..
        } = &model.findings()[0]
        else {
            panic!(
                "expected a disjointness finding, got {:?}",
                model.findings()
            );
        };
        assert_eq!(*rule, SkosRule::S9);
        assert_eq!(first.1, ClassOrigin::Asserted);
        assert_eq!(second.1, ClassOrigin::Asserted);
    }

    #[test]
    fn s37_a_collection_cannot_be_a_concept_even_when_the_class_was_inferred() {
        // The graph never says <Thing> is a collection; S31 does, because it has a member. The
        // finding has to say so, or a reviewer cannot tell whether to fix the data or us.
        let model = CoreModel::from_statements([
            typed(&ex("Thing"), SkosClass::Concept),
            s(&ex("Thing"), SKOS_MEMBER, &ex("X")),
        ]);

        assert!(!model.is_consistent());
        let Finding::DisjointClasses {
            first,
            second,
            rule,
            ..
        } = &model.findings()[0]
        else {
            panic!(
                "expected a disjointness finding, got {:?}",
                model.findings()
            );
        };
        assert_eq!(*rule, SkosRule::S37);
        assert_eq!(
            *first,
            (SkosClass::Collection, ClassOrigin::Entailed(SkosRule::S31))
        );
        assert_eq!(*second, (SkosClass::Concept, ClassOrigin::Asserted));
    }

    #[test]
    fn s37_also_catches_a_collection_that_is_a_concept_scheme() {
        let model = CoreModel::from_statements([
            typed(&ex("Thing"), SkosClass::Collection),
            typed(&ex("Thing"), SkosClass::ConceptScheme),
        ]);
        assert!(!model.is_consistent());
        assert!(model.findings().iter().any(|finding| matches!(
            finding,
            Finding::DisjointClasses {
                rule: SkosRule::S37,
                ..
            }
        )));
    }

    #[test]
    fn an_ordered_collection_that_is_a_concept_is_caught_through_s29() {
        // The disjointness is stated for skos:Collection, not for skos:OrderedCollection. Without
        // S29 materialised, this graph would pass.
        let (head, mut statements) = rdf_list("cell", &[ex("X")]);
        statements.push(typed(&ex("Thing"), SkosClass::Concept));
        statements.push(s(&ex("Thing"), SKOS_MEMBER_LIST, &head));
        let model = CoreModel::from_statements(statements);

        assert!(!model.is_consistent(), "{:?}", model.findings());
        assert!(model.findings().iter().any(|finding| matches!(
            finding,
            Finding::DisjointClasses {
                rule: SkosRule::S37,
                ..
            }
        )));
    }

    #[test]
    fn s3_and_s30_refuse_a_literal_where_an_object_property_needs_a_resource() {
        for (predicate, rule) in [
            (SKOS_IN_SCHEME, SkosRule::S3),
            (SKOS_TOP_CONCEPT_OF, SkosRule::S3),
            (SKOS_HAS_TOP_CONCEPT, SkosRule::S3),
            (SKOS_MEMBER, SkosRule::S30),
            (SKOS_MEMBER_LIST, SkosRule::S30),
        ] {
            let model = CoreModel::from_statements([Statement::new(
                ex("A"),
                predicate.to_owned(),
                plain("Chemistry"),
            )]);
            assert!(!model.is_consistent(), "{predicate}");
            let Finding::LiteralOnObjectProperty { rule: found, .. } = &model.findings()[0] else {
                panic!(
                    "{predicate}: expected a literal finding, got {:?}",
                    model.findings()
                );
            };
            assert_eq!(*found, rule, "{predicate}");
        }
    }

    // --- Ill-formed member lists ----------------------------------------------------------

    /// Build an ordered collection whose list is `statements`, headed at `head`.
    fn ordered_with(head: Node, mut statements: Vec<Statement>) -> CoreModel {
        statements.push(typed(&ex("Ordered"), SkosClass::OrderedCollection));
        statements.push(Statement::new(ex("Ordered"), SKOS_MEMBER_LIST, head));
        CoreModel::from_statements(statements)
    }

    fn defect(model: &CoreModel) -> ListDefect {
        model
            .findings()
            .iter()
            .find_map(|finding| match finding {
                Finding::DefectiveMemberList { defect, .. } => Some(defect.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected a list defect, got {:?}", model.findings()))
    }

    #[test]
    fn a_cyclic_member_list_is_reported_and_entails_nothing() {
        // b0 -> b1 -> b0. Left alone this is an infinite walk, which is the reason the check
        // exists rather than a hypothetical.
        let model = ordered_with(
            Node::blank("b0"),
            vec![
                Statement::new(Node::blank("b0"), RDF_FIRST, ex("X")),
                Statement::new(Node::blank("b0"), RDF_REST, Node::blank("b1")),
                Statement::new(Node::blank("b1"), RDF_FIRST, ex("Y")),
                Statement::new(Node::blank("b1"), RDF_REST, Node::blank("b0")),
            ],
        );

        assert_eq!(
            defect(&model),
            ListDefect::Cyclic {
                cell: Node::blank("b0")
            }
        );
        // S36 says "every item in the list". A list with no end has no such set, so nothing is
        // concluded from it — the items read are reported and left as data.
        assert!(model.resource(&ex("Ordered")).unwrap().members().is_empty());
        assert!(!model
            .derivations()
            .iter()
            .any(|derivation| derivation.rule == SkosRule::S36));
        // SKOS itself does not forbid it, so the graph is not called inconsistent.
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    #[test]
    fn a_truncated_member_list_is_reported_with_what_was_read() {
        let model = ordered_with(
            Node::blank("b0"),
            vec![
                Statement::new(Node::blank("b0"), RDF_FIRST, ex("X")),
                Statement::new(Node::blank("b0"), RDF_REST, Node::blank("b1")),
                Statement::new(Node::blank("b1"), RDF_FIRST, ex("Y")),
                // no rdf:rest on b1 — the list never reaches rdf:nil.
            ],
        );

        assert_eq!(
            defect(&model),
            ListDefect::NoRest {
                cell: Node::blank("b1")
            }
        );
        let Finding::DefectiveMemberList { read, .. } = &model.findings()[0] else {
            panic!("{:?}", model.findings());
        };
        assert_eq!(*read, 2);
        assert!(model.resource(&ex("Ordered")).unwrap().members().is_empty());
    }

    #[test]
    fn a_cell_with_no_item_is_reported() {
        let model = ordered_with(
            Node::blank("b0"),
            vec![Statement::new(
                Node::blank("b0"),
                RDF_REST,
                Node::iri(RDF_NIL),
            )],
        );
        assert_eq!(
            defect(&model),
            ListDefect::NoFirst {
                cell: Node::blank("b0")
            }
        );
    }

    #[test]
    fn a_branching_cell_is_reported_rather_than_one_branch_chosen() {
        // The shape Example 43 produces when two lists are merged onto one blank node. Picking a
        // branch would report an order the graph does not state.
        let model = ordered_with(
            Node::blank("b0"),
            vec![
                Statement::new(Node::blank("b0"), RDF_FIRST, ex("X")),
                Statement::new(Node::blank("b0"), RDF_FIRST, ex("A")),
                Statement::new(Node::blank("b0"), RDF_REST, Node::iri(RDF_NIL)),
            ],
        );
        assert_eq!(
            defect(&model),
            ListDefect::Branches {
                cell: Node::blank("b0"),
                property: "rdf:first",
                values: 2
            }
        );
    }

    #[test]
    fn a_literal_in_the_list_vocabulary_is_reported_only_where_the_list_is_walked() {
        let model = ordered_with(
            Node::blank("b0"),
            vec![
                Statement::new(Node::blank("b0"), RDF_FIRST, plain("Chemistry")),
                Statement::new(Node::blank("b0"), RDF_REST, Node::iri(RDF_NIL)),
            ],
        );
        assert_eq!(
            defect(&model),
            ListDefect::NotANode {
                cell: Node::blank("b0"),
                property: "rdf:first"
            }
        );

        // The same statement outside a skos:memberList is ordinary RDF and says nothing about
        // SKOS, so it raises nothing at all.
        let unrelated = CoreModel::from_statements([Statement::new(
            Node::blank("b0"),
            RDF_FIRST,
            plain("Chemistry"),
        )]);
        assert!(
            unrelated.findings().is_empty(),
            "{:?}",
            unrelated.findings()
        );
    }

    #[test]
    fn an_empty_member_list_is_well_formed_and_has_no_members() {
        let model = ordered_with(Node::iri(RDF_NIL), Vec::new());

        let collection = model.resource(&ex("Ordered")).unwrap();
        assert!(collection.member_lists()[0].is_well_formed());
        assert!(collection.member_lists()[0].items.is_empty());
        assert!(collection.members().is_empty());
        assert!(model.findings().is_empty(), "{:?}", model.findings());
        // It is still a collection: S33 then S29, neither of which depends on the list's contents.
        assert!(collection.is_a(SkosClass::Collection));
    }

    // --- The model as a whole -------------------------------------------------------------

    /// The model reads what it has rules for and counts the rest. `skos:notation` and
    /// `skos:scopeNote` are §6 and §7, which are their own build-plan items, so they leave nothing
    /// behind but a number — and a resource mentioned only by one of them is not in the model at
    /// all, which is what stops the model growing with the graph rather than with its structure.
    #[test]
    fn statements_outside_the_core_model_are_counted_and_dropped() {
        let model = CoreModel::from_statements([
            typed(&ex("A"), SkosClass::Concept),
            Statement::new(ex("A"), skos("prefLabel"), plain("Chemistry")),
            Statement::new(ex("A"), skos("notation"), plain("CHEM")),
            Statement::new(ex("A"), skos("scopeNote"), plain("The study of matter.")),
            Statement::new(
                ex("A"),
                "http://example.com/ns/approvedBy".to_owned(),
                ex("Board"),
            ),
        ]);

        assert_eq!(model.statements_read(), 5);
        assert_eq!(model.count_of(SkosClass::Concept), 1);
        assert!(model.resource(&ex("Board")).is_none());
    }

    /// The counterpart, and the behaviour this item changed: a `skos:broader` **is** in the core
    /// model now, so its object is too — as a `skos:Concept`, under S19 and S20. Before this item
    /// the same graph left `<B>` unmentioned. Kept beside the test above so the boundary between
    /// what is read and what is counted is written down in one place.
    #[test]
    fn a_semantic_relation_brings_its_object_into_the_model() {
        let model = CoreModel::from_statements([
            typed(&ex("A"), SkosClass::Concept),
            Statement::new(ex("A"), skos("broader"), ex("B")),
        ]);

        assert_eq!(model.count_of(SkosClass::Concept), 2);
        assert_eq!(
            model
                .resource(&ex("B"))
                .and_then(|resource| resource.classes().get(&SkosClass::Concept)),
            Some(&ClassOrigin::Entailed(SkosRule::S20))
        );
    }

    #[test]
    fn every_derivation_names_its_premise_and_the_statement_that_licensed_it() {
        // The charter's rule is that no inference path may exist that cannot explain itself. This
        // is that rule as a test rather than as a comment.
        let items = [ex("X")];
        let (head, mut statements) = rdf_list("cell", &items);
        statements.extend([
            s(&ex("MyScheme"), SKOS_HAS_TOP_CONCEPT, &ex("MyConcept")),
            typed(&ex("Ordered"), SkosClass::OrderedCollection),
            s(&ex("Ordered"), SKOS_MEMBER_LIST, &head),
        ]);
        let model = CoreModel::from_statements(statements);

        assert!(!model.derivations().is_empty());
        for derivation in model.derivations() {
            assert!(!derivation.conclusion.is_empty());
            assert!(!derivation.premise.is_empty());
            let rendered = derivation.to_string();
            assert!(rendered.contains("because"), "{rendered}");
            assert!(rendered.contains(derivation.rule.number()), "{rendered}");
            assert!(rendered.contains(derivation.rule.statement()), "{rendered}");
        }
    }

    #[test]
    fn an_asserted_class_is_never_reported_as_inferred() {
        let model = CoreModel::from_statements([
            typed(&ex("MyScheme"), SkosClass::ConceptScheme),
            s(&ex("A"), SKOS_IN_SCHEME, &ex("MyScheme")),
        ]);
        assert_eq!(
            model.resource(&ex("MyScheme")).unwrap().classes()[&SkosClass::ConceptScheme],
            ClassOrigin::Asserted
        );
        assert!(!model
            .derivations()
            .iter()
            .any(|derivation| derivation.conclusion.contains("MyScheme")
                && derivation.conclusion.contains("ConceptScheme")));
    }

    #[test]
    fn reading_the_same_graph_twice_gives_the_same_model() {
        // Determinism is what makes the report diffable, which is what makes it usable in CI.
        let statements = vec![
            s(&ex("MyScheme"), SKOS_HAS_TOP_CONCEPT, &ex("B")),
            s(&ex("A"), SKOS_TOP_CONCEPT_OF, &ex("MyScheme")),
            typed(&ex("Coll"), SkosClass::Collection),
            s(&ex("Coll"), SKOS_MEMBER, &ex("A")),
        ];
        let forwards = CoreModel::from_statements(statements.clone());
        let backwards = CoreModel::from_statements(statements.into_iter().rev());

        assert_eq!(forwards.resources, backwards.resources);
        assert_eq!(forwards.findings, backwards.findings);
    }

    // ---------------------------------------------------------------------------------------
    // Lexical labels — SKOS Reference §5. Every test below is one of the specification's own
    // numbered examples, asserted to be what the specification says it is. §5.4 states exactly
    // two integrity conditions, S13 and S14, and the examples are the specification's own
    // evidence for where the line falls.
    // ---------------------------------------------------------------------------------------

    /// `"value"@tag`, the shape every SKOS label arrives in.
    fn tagged(value: &str, language: &str) -> Term {
        Term::Literal(Literal {
            value: value.to_owned(),
            language: Some(language.to_owned()),
            datatype: crate::labels::RDF_LANG_STRING.to_owned(),
        })
    }

    /// `<subject> skos:<kind> <object>`.
    fn labelled(subject: &Node, kind: LabelKind, object: Term) -> Statement {
        Statement::new(subject.clone(), kind.property_iri(), object)
    }

    /// Every finding of one shape, rendered, so an assertion can name what it expected to see.
    fn findings_matching(model: &CoreModel, needle: &str) -> Vec<String> {
        model
            .findings()
            .iter()
            .map(ToString::to_string)
            .filter(|finding| finding.contains(needle))
            .collect()
    }

    /// Example 10 — labels in two languages, all three kinds. Consistent.
    #[test]
    fn example_10_labels_in_two_languages_are_consistent() {
        let model = CoreModel::from_statements(vec![
            labelled(
                &ex("MyResource"),
                LabelKind::Preferred,
                tagged("animals", "en"),
            ),
            labelled(
                &ex("MyResource"),
                LabelKind::Alternative,
                tagged("fauna", "en"),
            ),
            labelled(
                &ex("MyResource"),
                LabelKind::Hidden,
                tagged("aminals", "en"),
            ),
            labelled(
                &ex("MyResource"),
                LabelKind::Preferred,
                tagged("animaux", "fr"),
            ),
            labelled(
                &ex("MyResource"),
                LabelKind::Alternative,
                tagged("faune", "fr"),
            ),
        ]);

        let resource = model
            .resource(&ex("MyResource"))
            .expect("a labelled resource");
        assert_eq!(resource.labels().len(), 5);
        assert_eq!(
            resource
                .preferred_label_in("en")
                .map(ToString::to_string)
                .as_deref(),
            Some("\"animals\"@en")
        );
        assert_eq!(
            resource
                .preferred_label_in("fr")
                .map(ToString::to_string)
                .as_deref(),
            Some("\"animaux\"@fr")
        );
        assert!(model.is_consistent(), "{:?}", model.findings());
        assert!(model.findings().is_empty(), "{:?}", model.findings());
    }

    /// Example 11 — four preferred labels on one resource, in four Japanese script tags.
    ///
    /// The example that would fail a model comparing primary subtags rather than whole tags:
    /// all four are Japanese, and all four are consistent.
    #[test]
    fn example_11_four_japanese_script_tags_are_four_different_languages() {
        let model = CoreModel::from_statements(vec![
            labelled(
                &ex("AnotherResource"),
                LabelKind::Preferred,
                tagged("\u{6771}", "ja-Hani"),
            ),
            labelled(
                &ex("AnotherResource"),
                LabelKind::Preferred,
                tagged("\u{3072}\u{304c}\u{3057}", "ja-Hira"),
            ),
            labelled(
                &ex("AnotherResource"),
                LabelKind::Alternative,
                tagged("\u{3042}\u{305a}\u{307e}", "ja-Hira"),
            ),
            labelled(
                &ex("AnotherResource"),
                LabelKind::Preferred,
                tagged("\u{30d2}\u{30ac}\u{30b7}", "ja-Kana"),
            ),
            labelled(
                &ex("AnotherResource"),
                LabelKind::Alternative,
                tagged("\u{30a2}\u{30ba}\u{30de}", "ja-Kana"),
            ),
            labelled(
                &ex("AnotherResource"),
                LabelKind::Preferred,
                tagged("higashi", "ja-Latn"),
            ),
            labelled(
                &ex("AnotherResource"),
                LabelKind::Alternative,
                tagged("azuma", "ja-Latn"),
            ),
        ]);

        assert!(model.is_consistent(), "{:?}", model.findings());
        let coverage = model.label_coverage();
        assert_eq!(coverage.len(), 4, "four tags, four languages: {coverage:?}");
        assert!(coverage.iter().all(|language| language.preferred == 1));
    }

    /// Example 12 — two preferred labels with the same tag. **Not consistent.** S14.
    #[test]
    fn example_12_two_preferred_labels_in_one_language_violate_s14() {
        let model = CoreModel::from_statements(vec![
            labelled(&ex("Love"), LabelKind::Preferred, tagged("love", "en")),
            labelled(&ex("Love"), LabelKind::Preferred, tagged("adoration", "en")),
        ]);

        assert!(!model.is_consistent(), "{:?}", model.findings());
        assert_eq!(
            model.findings(),
            [Finding::MultiplePreferredLabels {
                resource: ex("Love"),
                language: Some("en".to_owned()),
                labels: vec!["\"adoration\"@en".to_owned(), "\"love\"@en".to_owned()],
            }]
        );
        assert_eq!(model.findings()[0].severity(), Severity::Inconsistent);
        let rendered = model.findings()[0].to_string();
        assert!(rendered.contains("S14"), "{rendered}");
        assert!(rendered.contains("no more than one value"), "{rendered}");
    }

    /// Examples 13, 14 and 15 — the three ways one label can sit under two properties. S13.
    #[test]
    fn examples_13_14_and_15_a_label_under_two_properties_violates_s13() {
        for (first, second) in [
            (LabelKind::Preferred, LabelKind::Alternative),
            (LabelKind::Alternative, LabelKind::Hidden),
            (LabelKind::Preferred, LabelKind::Hidden),
        ] {
            let model = CoreModel::from_statements(vec![
                labelled(&ex("Love"), first, tagged("love", "en")),
                labelled(&ex("Love"), second, tagged("love", "en")),
            ]);

            assert!(
                !model.is_consistent(),
                "{first} with {second}: {:?}",
                model.findings()
            );
            assert_eq!(
                model.findings(),
                [Finding::LabelPropertiesClash {
                    resource: ex("Love"),
                    label: LexicalLabel {
                        language: Some("en".to_owned()),
                        text: "love".to_owned(),
                    },
                    kinds: vec![first, second],
                }],
                "{first} with {second}"
            );
            let rendered = model.findings()[0].to_string();
            assert!(rendered.contains("S13"), "{rendered}");
            assert!(rendered.contains("pairwise disjoint"), "{rendered}");
        }
    }

    /// One label under two properties is **one** finding, not one per property.
    ///
    /// The map is keyed by the label, so this is a property of the structure rather than of a
    /// de-duplicating pass — which is why it is worth pinning: a change to the key would turn
    /// every clash into two identical findings and nothing else would notice.
    #[test]
    fn a_clashing_label_is_held_once_with_both_properties() {
        let model = CoreModel::from_statements(vec![
            labelled(&ex("Love"), LabelKind::Preferred, tagged("love", "en")),
            labelled(&ex("Love"), LabelKind::Alternative, tagged("love", "en")),
        ]);

        let resource = model.resource(&ex("Love")).expect("a labelled resource");
        assert_eq!(resource.labels().len(), 1);
        assert_eq!(model.findings().len(), 1);
        assert_eq!(resource.labels_of(LabelKind::Preferred).count(), 1);
        assert_eq!(resource.labels_of(LabelKind::Alternative).count(), 1);
    }

    /// Example 16 — labelling an `owl:Class` is consistent, and entails no SKOS class.
    ///
    /// §5.6.1: the three properties have no stated domain. A model that made a `skos:Concept`
    /// out of everything with a `skos:prefLabel` would miscount every vocabulary that labels its
    /// own concept scheme, which is nearly all of them.
    #[test]
    fn example_16_labelling_any_resource_is_consistent_and_entails_no_class() {
        let model = CoreModel::from_statements(vec![
            Statement::new(
                ex("MyClass"),
                RDF_TYPE,
                Node::iri("http://www.w3.org/2002/07/owl#Class"),
            ),
            labelled(
                &ex("MyClass"),
                LabelKind::Preferred,
                tagged("animals", "en"),
            ),
            labelled(
                &ex("MyClass"),
                LabelKind::Alternative,
                tagged("fauna", "en"),
            ),
            labelled(&ex("MyClass"), LabelKind::Hidden, tagged("aminals", "en")),
            labelled(
                &ex("MyClass"),
                LabelKind::Preferred,
                tagged("animaux", "fr"),
            ),
            labelled(
                &ex("MyClass"),
                LabelKind::Alternative,
                tagged("faune", "fr"),
            ),
        ]);

        let resource = model.resource(&ex("MyClass")).expect("a labelled resource");
        assert!(resource.classes().is_empty(), "{:?}", resource.classes());
        assert_eq!(model.count_of(SkosClass::Concept), 0);
        assert!(model.derivations().is_empty(), "{:?}", model.derivations());
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    /// Example 17 — alternatives with no preferred label. Consistent, and no entailments follow.
    #[test]
    fn example_17_alternatives_without_a_preferred_label_are_consistent() {
        let model = CoreModel::from_statements(vec![
            labelled(
                &ex("Love"),
                LabelKind::Alternative,
                tagged("adoration", "en"),
            ),
            labelled(&ex("Love"), LabelKind::Alternative, tagged("desire", "en")),
        ]);

        let resource = model.resource(&ex("Love")).expect("a labelled resource");
        assert!(model.is_consistent(), "{:?}", model.findings());
        assert!(model.findings().is_empty(), "{:?}", model.findings());
        assert_eq!(resource.preferred_label_in("en"), None);
        assert_eq!(
            resource.display_label().map(ToString::to_string).as_deref(),
            Some("\"adoration\"@en"),
            "an alternative is a better answer than an IRI when there is no preferred label"
        );
    }

    /// Example 18 — `en`, `en-US` and `en-GB` are three tags, so three preferred labels are fine.
    #[test]
    fn example_18_three_english_tags_are_three_languages() {
        let model = CoreModel::from_statements(vec![
            labelled(&ex("Colour"), LabelKind::Preferred, tagged("color", "en")),
            labelled(
                &ex("Colour"),
                LabelKind::Preferred,
                tagged("color", "en-US"),
            ),
            labelled(
                &ex("Colour"),
                LabelKind::Preferred,
                tagged("colour", "en-GB"),
            ),
        ]);

        assert!(model.is_consistent(), "{:?}", model.findings());
        let resource = model.resource(&ex("Colour")).expect("a labelled resource");
        assert_eq!(
            resource
                .preferred_label_in("en-GB")
                .map(ToString::to_string)
                .as_deref(),
            Some("\"colour\"@en-gb")
        );
        assert_eq!(
            resource
                .preferred_label_in("en")
                .map(ToString::to_string)
                .as_deref(),
            Some("\"color\"@en"),
            "a request for en is not answered with en-GB — \u{a7}5.6.5 makes them different tags"
        );
    }

    /// Example 19 — the same lexical form under two properties, in two tags. Consistent.
    ///
    /// The narrow edge of S13: the condition is about the same *RDF term*, and `"love"@en` and
    /// `"love"@en-GB` are not the same term. A model comparing lexical forms alone would refuse
    /// this graph, which the specification says is fine.
    #[test]
    fn example_19_the_same_text_under_two_tags_does_not_clash() {
        let model = CoreModel::from_statements(vec![
            labelled(&ex("Love"), LabelKind::Preferred, tagged("love", "en")),
            labelled(&ex("Love"), LabelKind::Alternative, tagged("love", "en-GB")),
        ]);

        assert!(model.is_consistent(), "{:?}", model.findings());
        assert!(model.findings().is_empty(), "{:?}", model.findings());
    }

    /// RDF 1.1 Concepts §3.3 — language tags have a lower-case value space, so `@EN` is `@en`.
    ///
    /// Oxigraph normalises tags on the way in, so this can only be reached by a caller that is
    /// not the store — a parsed file, a discovery result, an agent's proposal. The crate is
    /// engine-free by `docs/adr/0019` and so cannot rely on the store having done it.
    #[test]
    fn a_language_tag_in_upper_case_is_the_same_language() {
        let clash = CoreModel::from_statements(vec![
            labelled(&ex("Love"), LabelKind::Preferred, tagged("love", "EN")),
            labelled(&ex("Love"), LabelKind::Alternative, tagged("love", "en")),
        ]);
        assert_eq!(
            findings_matching(&clash, "S13").len(),
            1,
            "{:?}",
            clash.findings()
        );

        let too_many = CoreModel::from_statements(vec![
            labelled(&ex("Love"), LabelKind::Preferred, tagged("love", "EN")),
            labelled(&ex("Love"), LabelKind::Preferred, tagged("adoration", "en")),
        ]);
        assert_eq!(
            findings_matching(&too_many, "S14").len(),
            1,
            "{:?}",
            too_many.findings()
        );
    }

    /// S14 counts per language *tag*, and "no tag" is a bucket like any other.
    #[test]
    fn two_untagged_preferred_labels_violate_s14() {
        let model = CoreModel::from_statements(vec![
            labelled(&ex("Love"), LabelKind::Preferred, plain("love")),
            labelled(&ex("Love"), LabelKind::Preferred, plain("adoration")),
        ]);

        assert!(!model.is_consistent(), "{:?}", model.findings());
        let rendered = model.findings()[0].to_string();
        assert!(rendered.contains("in no language"), "{rendered}");
    }

    /// S12 — not an integrity condition. §5.4 lists two and this is not one of them.
    ///
    /// The distinction is the whole point: a vocabulary carrying `skos:prefLabel "4"^^xsd:integer`
    /// is odd, and it is still a SKOS vocabulary. §5.6.2 says an application "may reject such
    /// data but is not required to", so we report and read on.
    #[test]
    fn s12_a_label_that_is_not_a_plain_literal_is_ill_formed_and_not_inconsistent() {
        let typed_literal = Term::Literal(Literal {
            value: "4".to_owned(),
            language: None,
            datatype: "http://www.w3.org/2001/XMLSchema#integer".to_owned(),
        });
        let model = CoreModel::from_statements(vec![
            labelled(&ex("Love"), LabelKind::Preferred, typed_literal.clone()),
            labelled(
                &ex("Love"),
                LabelKind::Alternative,
                Term::Node(ex("Adoration")),
            ),
        ]);

        assert!(
            model.is_consistent(),
            "S12 is a usage convention, not an integrity condition: {:?}",
            model.findings()
        );
        assert_eq!(model.findings().len(), 2);
        assert!(model
            .findings()
            .iter()
            .all(|finding| finding.severity() == Severity::IllFormed));
        let rendered = model.findings()[0].to_string();
        assert!(rendered.contains("S12"), "{rendered}");
        assert!(rendered.contains("our judgement"), "{rendered}");

        // And it takes no part in either integrity condition, because it is in no language. A
        // resource whose *only* SKOS statements are refused labels is therefore not in the model
        // at all — the findings name it, and `resources()` keeps its documented meaning of "what
        // the model has something to say about". The two together are the honest answer: the
        // graph mentioned it, and we learned nothing about it in SKOS terms.
        assert_eq!(model.resource(&ex("Love")), None);
        assert!(model
            .findings()
            .iter()
            .all(|finding| finding.to_string().contains("<http://example.com/ns/Love>")));
    }

    /// A concept with one good label and one refused value keeps the good one.
    #[test]
    fn a_refused_label_does_not_take_the_rest_of_the_resource_with_it() {
        let model = CoreModel::from_statements(vec![
            typed(&ex("Cat"), SkosClass::Concept),
            labelled(&ex("Cat"), LabelKind::Preferred, tagged("cat", "en")),
            labelled(
                &ex("Cat"),
                LabelKind::Alternative,
                Term::Literal(Literal {
                    value: "4".to_owned(),
                    language: None,
                    datatype: "http://www.w3.org/2001/XMLSchema#integer".to_owned(),
                }),
            ),
        ]);

        let resource = model.resource(&ex("Cat")).expect("a typed concept");
        assert!(resource.is_a(SkosClass::Concept));
        assert_eq!(
            resource.display_label().map(ToString::to_string).as_deref(),
            Some("\"cat\"@en")
        );
        assert_eq!(findings_matching(&model, "S12").len(), 1);
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    /// A value that is not a label does not silently become one under a different property.
    ///
    /// Two properties, one non-plain value: with the value discarded there is nothing for S13 to
    /// compare, and inventing a bucket for it would report a clash the specification does not
    /// describe.
    #[test]
    fn a_non_plain_value_under_two_properties_is_two_findings_and_no_clash() {
        let typed_literal = Term::Literal(Literal {
            value: "4".to_owned(),
            language: None,
            datatype: "http://www.w3.org/2001/XMLSchema#integer".to_owned(),
        });
        let model = CoreModel::from_statements(vec![
            labelled(&ex("Love"), LabelKind::Preferred, typed_literal.clone()),
            labelled(&ex("Love"), LabelKind::Alternative, typed_literal),
        ]);

        assert_eq!(findings_matching(&model, "S12").len(), 2);
        assert!(findings_matching(&model, "S13").is_empty());
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    /// The multilingual question a programme actually asks: how far behind is each language?
    #[test]
    fn label_coverage_counts_every_language_and_the_resources_it_reaches() {
        let model = CoreModel::from_statements(vec![
            typed(&ex("Cat"), SkosClass::Concept),
            labelled(&ex("Cat"), LabelKind::Preferred, tagged("cat", "en")),
            labelled(&ex("Cat"), LabelKind::Alternative, tagged("feline", "en")),
            labelled(&ex("Cat"), LabelKind::Preferred, tagged("chat", "fr")),
            typed(&ex("Dog"), SkosClass::Concept),
            labelled(&ex("Dog"), LabelKind::Preferred, tagged("dog", "en")),
            labelled(&ex("Dog"), LabelKind::Hidden, tagged("dgo", "en")),
        ]);

        let coverage = model.label_coverage();
        assert_eq!(coverage.len(), 2);
        assert_eq!(coverage[0].language.as_deref(), Some("en"));
        assert_eq!(coverage[0].preferred, 2);
        assert_eq!(coverage[0].alternative, 1);
        assert_eq!(coverage[0].hidden, 1);
        assert_eq!(coverage[0].resources_with_preferred, 2);
        assert_eq!(coverage[0].total(), 4);
        assert_eq!(coverage[1].language.as_deref(), Some("fr"));
        assert_eq!(coverage[1].preferred, 1);
        assert_eq!(
            coverage[1].resources_with_preferred, 1,
            "one of the two concepts has a French preferred label, which is the gap a \
             translation programme is looking for"
        );
    }

    /// A label under two properties is counted under both, because it is two labellings.
    #[test]
    fn coverage_counts_a_clashing_label_under_each_property_that_carries_it() {
        let model = CoreModel::from_statements(vec![
            labelled(&ex("Love"), LabelKind::Preferred, tagged("love", "en")),
            labelled(&ex("Love"), LabelKind::Alternative, tagged("love", "en")),
        ]);

        let coverage = model.label_coverage();
        assert_eq!(coverage.len(), 1);
        assert_eq!(coverage[0].preferred, 1);
        assert_eq!(coverage[0].alternative, 1);
        assert_eq!(coverage[0].resources_with_preferred, 1);
    }

    #[test]
    fn the_display_label_is_stable_whichever_order_the_labels_arrive_in() {
        let statements = vec![
            labelled(&ex("Love"), LabelKind::Preferred, tagged("amour", "fr")),
            labelled(&ex("Love"), LabelKind::Preferred, tagged("love", "en")),
            labelled(
                &ex("Love"),
                LabelKind::Alternative,
                tagged("adoration", "en"),
            ),
        ];
        let forwards = CoreModel::from_statements(statements.clone());
        let backwards = CoreModel::from_statements(statements.into_iter().rev());

        let of = |model: &CoreModel| {
            model
                .resource(&ex("Love"))
                .and_then(Resource::display_label)
                .map(ToString::to_string)
        };
        assert_eq!(of(&forwards), of(&backwards));
        assert_eq!(of(&forwards).as_deref(), Some("\"love\"@en"));
    }

    /// A resource with no labels at all has no coverage and no findings.
    #[test]
    fn an_unlabelled_vocabulary_reports_no_languages() {
        let model = CoreModel::from_statements(vec![typed(&ex("Cat"), SkosClass::Concept)]);

        assert!(model.label_coverage().is_empty());
        assert_eq!(
            model
                .resource(&ex("Cat"))
                .expect("a concept")
                .display_label(),
            None
        );
        assert!(model.is_consistent());
    }

    // ---------------------------------------------------------------------------------------
    // SKOS-XL — SKOS Reference Appendix B. Every test whose name carries an example number is
    // that example, asserted to be what the specification marks it: "(consistent)", "(not
    // consistent)", "(entailment)" or "(non-entailment)". Appendix B states no integrity
    // conditions of its own, so where the specification does not use the word "consistent" the
    // classification is ours and the test says which it is testing.
    // ---------------------------------------------------------------------------------------

    /// `<subject> skosxl:<kind> <label>`.
    fn xl_labelled(subject: &Node, kind: LabelKind, label: &Node) -> Statement {
        Statement::new(subject.clone(), kind.xl_property_iri(), label.clone())
    }

    /// `<label> skosxl:literalForm <form>`.
    fn literal_form(label: &Node, form: Term) -> Statement {
        Statement::new(label.clone(), SKOSXL_LITERAL_FORM, form)
    }

    /// Only the derivations produced by an S55–S57 chain, rendered.
    ///
    /// The class entailments S50 and S54 are derivations too, so "nothing was dumbed down" has to
    /// be asserted against the chains rather than against the whole list.
    fn dumbing_down(model: &CoreModel) -> Vec<String> {
        model
            .derivations()
            .iter()
            .filter(|derivation| {
                matches!(
                    derivation.rule,
                    SkosRule::S55 | SkosRule::S56 | SkosRule::S57
                )
            })
            .map(|derivation| format!("{} [{}]", derivation.conclusion, derivation.rule.number()))
            .collect()
    }

    /// The plain labels a resource ended up with, rendered with the kind and where each came from.
    fn plain_labels(model: &CoreModel, node: &Node) -> Vec<String> {
        let Some(resource) = model.resource(node) else {
            return Vec::new();
        };
        resource
            .labels()
            .iter()
            .flat_map(|(label, kinds)| {
                kinds
                    .iter()
                    .map(move |(kind, origin)| format!("{kind} {label} ({origin})"))
            })
            .collect()
    }

    /// Example 75 — a label with a literal form. Consistent.
    #[test]
    fn example_75_a_label_with_one_literal_form_is_consistent() {
        let a = ex("A");
        let model = CoreModel::from_statements(vec![
            typed(&a, SkosClass::Label),
            literal_form(&a, tagged("love", "en")),
        ]);

        let label = model.resource(&a).expect("the label");
        assert!(label.is_a(SkosClass::Label));
        assert_eq!(label.literal_forms().len(), 1);
        assert!(model.is_consistent(), "{:?}", model.findings());
        assert!(model.findings().is_empty(), "{:?}", model.findings());
    }

    /// Examples 76, 77, 78 and 79 — all four are marked "(not consistent)" for the same reason:
    /// an `skosxl:Label` is described with two different literal forms, and S52 allows one.
    ///
    /// 78 is the one worth having in a suite. `"love"@en-GB` and `"love"@en-US` differ only in
    /// their tags, and a model that bucketed forms by language before counting them — which is
    /// exactly what S14 does for plain labels — would call it consistent.
    #[test]
    fn examples_76_to_79_two_literal_forms_are_not_consistent() {
        for (name, first, second) in [
            ("76", plain("love"), plain("adoration")),
            ("77", tagged("love", "en"), tagged("love", "fr")),
            ("78", tagged("love", "en-GB"), tagged("love", "en-US")),
            (
                "79",
                tagged("\u{6771}", "ja-Hani"),
                tagged("\u{3072}\u{304c}\u{3057}", "ja-Hira"),
            ),
        ] {
            let b = ex("B");
            let model = CoreModel::from_statements(vec![
                typed(&b, SkosClass::Label),
                literal_form(&b, first),
                literal_form(&b, second),
            ]);

            assert!(
                !model.is_consistent(),
                "Example {name} must not be consistent"
            );
            assert_eq!(
                findings_matching(&model, "skosxl:literalForm values").len(),
                1,
                "Example {name}: {:?}",
                model.findings()
            );
        }
    }

    /// The same form twice is one form, so it is consistent. RDF graphs are sets, and S52 counts
    /// values of the property rather than statements — the contrast that makes Example 76 mean
    /// what it says.
    #[test]
    fn the_same_literal_form_stated_twice_is_one_form() {
        let b = ex("B");
        let model = CoreModel::from_statements(vec![
            typed(&b, SkosClass::Label),
            literal_form(&b, tagged("love", "en")),
            literal_form(&b, tagged("love", "en")),
        ]);

        assert_eq!(
            model.resource(&b).expect("the label").literal_forms().len(),
            1
        );
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    /// Example 80 — two labels with the same literal form are *not* entailed to be the same
    /// resource. B.2.4.1: the function from labels to literals is not injective.
    #[test]
    fn example_80_two_labels_sharing_a_form_are_not_the_same_resource() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements(vec![
            literal_form(&a, tagged("love", "en")),
            literal_form(&b, tagged("love", "en")),
        ]);

        assert_ne!(model.resource(&a), None);
        assert_ne!(model.resource(&b), None);
        assert!(
            !model
                .derivations()
                .iter()
                .any(|derivation| derivation.conclusion.contains("sameAs")),
            "no identity may be concluded: {:?}",
            model.derivations()
        );
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    /// Example 81 — a label may be a member of a concept scheme. Consistent.
    ///
    /// S4 makes the *object* of `skos:inScheme` a concept scheme; it says nothing about the
    /// subject, so the label stays a label and S48 is not touched. A model that entailed a class
    /// for the subject would turn this consistent example into a disjointness violation.
    #[test]
    fn example_81_a_label_may_be_in_a_concept_scheme() {
        let (a, scheme) = (ex("A"), ex("MyScheme"));
        let model = CoreModel::from_statements(vec![
            typed(&a, SkosClass::Label),
            literal_form(&a, tagged("love", "en")),
            s(&a, SKOS_IN_SCHEME, &scheme),
        ]);

        let label = model.resource(&a).expect("the label");
        assert!(label.is_a(SkosClass::Label));
        assert!(!label.is_a(SkosClass::Concept));
        assert_eq!(names(label.in_schemes()), vec![scheme.to_string()]);
        assert!(model
            .resource(&scheme)
            .expect("the scheme")
            .is_a(SkosClass::ConceptScheme));
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    /// Examples 82 and 83 — the whole point of SKOS-XL, in one test.
    ///
    /// 82 says the three XL labelling properties together are consistent; 83 says that same graph
    /// *entails* the three plain SKOS labels. Asserting them separately would let the model pass
    /// 82 by ignoring SKOS-XL entirely.
    #[test]
    fn examples_82_and_83_xl_labels_dumb_down_to_plain_skos_labels() {
        let love = ex("Love");
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements(vec![
            xl_labelled(&love, LabelKind::Preferred, &a),
            xl_labelled(&love, LabelKind::Alternative, &b),
            xl_labelled(&love, LabelKind::Hidden, &c),
            typed(&a, SkosClass::Label),
            literal_form(&a, tagged("love", "en")),
            typed(&b, SkosClass::Label),
            literal_form(&b, tagged("adoration", "en")),
            typed(&c, SkosClass::Label),
            literal_form(&c, tagged("luv", "en")),
        ]);

        assert!(model.is_consistent(), "{:?}", model.findings());
        assert!(model.findings().is_empty(), "{:?}", model.findings());
        assert_eq!(
            plain_labels(&model, &love),
            vec![
                // Ordered by language tag and then lexical form, which is why the alternative
                // comes first: "adoration" < "love" < "luv".
                "skos:altLabel \"adoration\"@en (from SKOS-XL, S56)",
                "skos:prefLabel \"love\"@en (from SKOS-XL, S55)",
                "skos:hiddenLabel \"luv\"@en (from SKOS-XL, S57)",
            ]
        );

        // Each of the three is explained, and by the chain that licensed that one — not by S55
        // three times over, which would be true only for the preferred label.
        assert_eq!(
            dumbing_down(&model),
            vec![
                "<http://example.com/ns/Love> skos:prefLabel \"love\"@en [S55]",
                "<http://example.com/ns/Love> skos:altLabel \"adoration\"@en [S56]",
                "<http://example.com/ns/Love> skos:hiddenLabel \"luv\"@en [S57]",
            ]
        );
        assert!(model.derivations()[0]
            .premise
            .contains("skosxl:prefLabel <http://example.com/ns/A>"));
    }

    /// Example 84 — two preferred XL labels in one language. Not consistent, under S14, which is
    /// a condition on the *plain* labels the chains produced.
    #[test]
    fn example_84_two_preferred_xl_labels_in_one_language_are_not_consistent() {
        let love = ex("Love");
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements(vec![
            xl_labelled(&love, LabelKind::Preferred, &a),
            xl_labelled(&love, LabelKind::Preferred, &b),
            literal_form(&a, tagged("love", "en")),
            literal_form(&b, tagged("adoration", "en")),
        ]);

        assert!(!model.is_consistent(), "{:?}", model.findings());
        let s14 = findings_matching(&model, "S14");
        assert_eq!(s14.len(), 1, "{:?}", model.findings());
        assert!(
            s14[0].contains("\"adoration\"@en, \"love\"@en"),
            "{}",
            s14[0]
        );
        // Neither label was typed. S50 is what makes them labels, and without it there would be
        // no literal form to dumb down and the example would come out consistent.
        assert!(model
            .resource(&a)
            .expect("the label")
            .is_a(SkosClass::Label));
    }

    /// Examples 85, 86 and 87 — two *different* XL labels with the same literal form, under two
    /// different properties. Not consistent, under S13.
    ///
    /// S58 is deliberately not what catches these: the two label resources are distinct, so no
    /// property pair shares a value. It is the dumbed-down plain labels that clash, which is
    /// precisely what B.3.4.2 says.
    #[test]
    fn examples_85_to_87_two_labels_sharing_a_form_across_properties_are_not_consistent() {
        for (name, first, second) in [
            ("85", LabelKind::Preferred, LabelKind::Alternative),
            ("86", LabelKind::Alternative, LabelKind::Hidden),
            ("87", LabelKind::Preferred, LabelKind::Hidden),
        ] {
            let love = ex("Love");
            let (a, b) = (ex("A"), ex("B"));
            let model = CoreModel::from_statements(vec![
                xl_labelled(&love, first, &a),
                xl_labelled(&love, second, &b),
                literal_form(&a, tagged("love", "en")),
                literal_form(&b, tagged("love", "en")),
            ]);

            assert!(
                !model.is_consistent(),
                "Example {name} must not be consistent"
            );
            assert_eq!(
                findings_matching(&model, "S13").len(),
                1,
                "Example {name}: {:?}",
                model.findings()
            );
            assert!(
                findings_matching(&model, "S58").is_empty(),
                "Example {name} is S13's case, not S58's: {:?}",
                model.findings()
            );
        }
    }

    /// S58 — one label resource under two XL properties. Inconsistent, and a different fault from
    /// Example 85's, which is why both are checked.
    #[test]
    fn s58_one_label_under_two_xl_properties_is_inconsistent() {
        let love = ex("Love");
        let a = ex("A");
        let model = CoreModel::from_statements(vec![
            xl_labelled(&love, LabelKind::Preferred, &a),
            xl_labelled(&love, LabelKind::Alternative, &a),
            literal_form(&a, tagged("love", "en")),
        ]);

        assert!(!model.is_consistent(), "{:?}", model.findings());
        let s58 = findings_matching(&model, "S58");
        assert_eq!(s58.len(), 1, "{:?}", model.findings());
        assert!(
            s58[0].contains("skosxl:prefLabel and skosxl:altLabel"),
            "{}",
            s58[0]
        );
        // And it also clashes once dumbed down, because one literal is now under two plain
        // properties. Two findings for one fault, each with its own true citation.
        assert_eq!(findings_matching(&model, "S13").len(), 1);
    }

    /// S48 — `skosxl:Label` is disjoint with each of the three core classes.
    #[test]
    fn s48_a_label_may_not_also_be_a_concept_scheme_or_collection() {
        for class in [
            SkosClass::Concept,
            SkosClass::ConceptScheme,
            SkosClass::Collection,
        ] {
            let a = ex("A");
            let model = CoreModel::from_statements(vec![
                typed(&a, SkosClass::Label),
                typed(&a, class),
                literal_form(&a, tagged("love", "en")),
            ]);

            assert!(
                !model.is_consistent(),
                "{class} must be disjoint with skosxl:Label"
            );
            assert_eq!(
                findings_matching(&model, "S48").len(),
                1,
                "{class}: {:?}",
                model.findings()
            );
        }
    }

    /// An *ordered* collection that is also a label is caught under S37, not S48 — because S29
    /// has already made it a collection, and S48's own wording names `skos:Collection`.
    #[test]
    fn an_ordered_collection_that_is_a_label_is_reported_under_the_rule_that_says_so() {
        let a = ex("A");
        let model = CoreModel::from_statements(vec![
            typed(&a, SkosClass::Label),
            typed(&a, SkosClass::OrderedCollection),
            literal_form(&a, tagged("love", "en")),
        ]);

        assert!(!model.is_consistent(), "{:?}", model.findings());
        let s48 = findings_matching(&model, "S48");
        assert_eq!(s48.len(), 1, "{:?}", model.findings());
        assert!(
            s48[0].contains("skos:Collection (inferred, S29)"),
            "{}",
            s48[0]
        );
    }

    /// S54 — being the object of an XL labelling property makes something a label, with no
    /// `rdf:type` anywhere. Example 84 relies on this and states no types at all.
    #[test]
    fn s54_the_object_of_an_xl_labelling_property_is_a_label() {
        let love = ex("Love");
        let a = ex("A");
        let model = CoreModel::from_statements(vec![xl_labelled(&love, LabelKind::Hidden, &a)]);

        assert_eq!(
            model
                .resource(&a)
                .expect("the label")
                .classes()
                .get(&SkosClass::Label),
            Some(&ClassOrigin::Entailed(SkosRule::S54))
        );
        assert_eq!(
            model
                .resource(&love)
                .expect("the concept")
                .xl_labels_of(LabelKind::Hidden)
                .collect::<Vec<_>>(),
            vec![&a]
        );
    }

    /// S52 — a label with no literal form. **Ill-formed, not inconsistent**, and the distinction
    /// is the test: "cardinality exactly 1" entails a form exists, it does not require the graph
    /// to state one, so a partial export is not a broken vocabulary.
    #[test]
    fn s52_a_label_with_no_literal_form_is_ill_formed_and_still_consistent() {
        let a = ex("A");
        let model = CoreModel::from_statements(vec![typed(&a, SkosClass::Label)]);

        assert!(model.is_consistent(), "{:?}", model.findings());
        assert_eq!(model.findings().len(), 1);
        assert_eq!(model.findings()[0].severity(), Severity::IllFormed);
        assert!(
            findings_matching(&model, "open-world assumption").len() == 1,
            "the reason must be in the report: {:?}",
            model.findings()
        );
    }

    /// S51 — a literal form that is not a plain literal is reported and **does not dumb down**.
    ///
    /// Dumbing it down would produce a `skos:prefLabel` that is S12's finding one step later, so
    /// the same fault would be reported twice under two rules and the concept would appear to
    /// have a preferred label it cannot display.
    #[test]
    fn s51_a_typed_literal_form_is_ill_formed_and_does_not_dumb_down() {
        let love = ex("Love");
        let a = ex("A");
        let model = CoreModel::from_statements(vec![
            xl_labelled(&love, LabelKind::Preferred, &a),
            literal_form(
                &a,
                Term::Literal(Literal {
                    value: "4".to_owned(),
                    language: None,
                    datatype: "http://www.w3.org/2001/XMLSchema#integer".to_owned(),
                }),
            ),
        ]);

        assert!(model.is_consistent(), "{:?}", model.findings());
        assert_eq!(
            findings_matching(&model, "S51").len(),
            1,
            "{:?}",
            model.findings()
        );
        assert!(plain_labels(&model, &love).is_empty());
        assert!(dumbing_down(&model).is_empty(), "{:?}", model.derivations());
        // Kept on the label all the same: it is what the graph says, and S52 counts it.
        assert_eq!(
            model.resource(&a).expect("the label").literal_forms().len(),
            1
        );
    }

    /// S49 — `skosxl:literalForm` is an `owl:DatatypeProperty`, so an IRI is a contradiction.
    ///
    /// The contrast with `skos:prefLabel` is the specification's, not ours: S10 makes that an
    /// `owl:AnnotationProperty`, whose values may legally be IRIs, so the same shape of mistake
    /// is ill-formed there and inconsistent here.
    #[test]
    fn s49_a_node_as_a_literal_form_is_inconsistent_unlike_a_node_as_a_plain_label() {
        let (a, elsewhere) = (ex("A"), ex("Elsewhere"));
        let model = CoreModel::from_statements(vec![
            typed(&a, SkosClass::Label),
            s(&a, SKOSXL_LITERAL_FORM, &elsewhere),
        ]);

        assert!(!model.is_consistent(), "{:?}", model.findings());
        assert_eq!(
            findings_matching(&model, "S49").len(),
            1,
            "{:?}",
            model.findings()
        );

        let plain = CoreModel::from_statements(vec![Statement::new(
            ex("Cat"),
            LabelKind::Preferred.property_iri(),
            elsewhere,
        )]);
        assert!(
            plain.is_consistent(),
            "S10 permits an IRI on an annotation property"
        );
        assert_eq!(plain.findings()[0].severity(), Severity::IllFormed);
    }

    /// A literal on an XL labelling property is inconsistent under S53, as on any object property.
    #[test]
    fn s53_a_literal_on_an_xl_labelling_property_is_inconsistent() {
        let model = CoreModel::from_statements(vec![Statement::new(
            ex("Love"),
            LabelKind::Preferred.xl_property_iri(),
            tagged("love", "en"),
        )]);

        assert!(!model.is_consistent(), "{:?}", model.findings());
        assert_eq!(
            findings_matching(&model, "S53").len(),
            1,
            "{:?}",
            model.findings()
        );
    }

    /// A label with two forms does not dumb down either — there is no principled way to choose
    /// which of the two the concept is really called, and inventing one would put a label the
    /// author never wrote in front of them.
    #[test]
    fn a_label_with_two_literal_forms_does_not_dumb_down() {
        let love = ex("Love");
        let a = ex("A");
        let model = CoreModel::from_statements(vec![
            xl_labelled(&love, LabelKind::Preferred, &a),
            literal_form(&a, tagged("love", "en")),
            literal_form(&a, tagged("adoration", "en")),
        ]);

        assert!(
            plain_labels(&model, &love).is_empty(),
            "{:?}",
            plain_labels(&model, &love)
        );
        assert!(dumbing_down(&model).is_empty(), "{:?}", model.derivations());
        assert_eq!(findings_matching(&model, "S52").len(), 1);
    }

    /// An asserted plain label is never restated as a dumbed-down one, exactly as an asserted
    /// class is never overwritten by an entailed one.
    #[test]
    fn an_asserted_plain_label_keeps_its_origin_when_xl_says_the_same_thing() {
        let love = ex("Love");
        let a = ex("A");
        let model = CoreModel::from_statements(vec![
            labelled(&love, LabelKind::Preferred, tagged("love", "en")),
            xl_labelled(&love, LabelKind::Preferred, &a),
            literal_form(&a, tagged("love", "en")),
        ]);

        assert_eq!(
            plain_labels(&model, &love),
            vec!["skos:prefLabel \"love\"@en (asserted)"]
        );
        assert!(dumbing_down(&model).is_empty(), "{:?}", model.derivations());
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    /// A dumbed-down label is counted in the coverage a multilingual programme reads, because to
    /// the person asking "how much of this is in French?" an XL label is a French label.
    #[test]
    fn dumbed_down_labels_count_towards_language_coverage() {
        let love = ex("Love");
        let a = ex("A");
        let model = CoreModel::from_statements(vec![
            typed(&love, SkosClass::Concept),
            xl_labelled(&love, LabelKind::Preferred, &a),
            literal_form(&a, tagged("amour", "fr")),
        ]);

        let coverage = model.label_coverage();
        assert_eq!(coverage.len(), 1);
        assert_eq!(coverage[0].language.as_deref(), Some("fr"));
        assert_eq!(coverage[0].preferred, 1);
        assert_eq!(coverage[0].resources_with_preferred, 1);
        assert_eq!(
            model
                .resource(&love)
                .expect("the concept")
                .preferred_label_in("fr")
                .map(ToString::to_string)
                .as_deref(),
            Some("\"amour\"@fr")
        );
    }

    /// The order statements arrive in does not change the answer, for SKOS-XL as for the rest.
    ///
    /// This is the assertion that would catch a pass reading a literal form before it was stored,
    /// which is the failure a streaming builder invites.
    #[test]
    fn a_skos_xl_vocabulary_reads_the_same_in_either_direction() {
        let love = ex("Love");
        let (a, b) = (ex("A"), ex("B"));
        let statements = vec![
            typed(&love, SkosClass::Concept),
            xl_labelled(&love, LabelKind::Preferred, &a),
            xl_labelled(&love, LabelKind::Alternative, &b),
            literal_form(&a, tagged("love", "en")),
            typed(&b, SkosClass::Label),
            literal_form(&b, tagged("adoration", "en")),
        ];

        let forwards = CoreModel::from_statements(statements.clone());
        let backwards = CoreModel::from_statements(statements.into_iter().rev());

        assert_eq!(forwards.resources, backwards.resources);
        assert_eq!(forwards.findings, backwards.findings);
        assert_eq!(plain_labels(&forwards, &love).len(), 2);
    }

    // ---------------------------------------------------------------------------------------
    // SKOS-XL B.4 — links between labels. S59–S62.
    // ---------------------------------------------------------------------------------------

    /// `<from> skosxl:labelRelation <to>`.
    fn label_relation(from: &Node, to: &Node) -> Statement {
        Statement::new(from.clone(), SKOSXL_LABEL_RELATION, to.clone())
    }

    /// A resource's label relations, rendered with where each one came from.
    fn relations(model: &CoreModel, node: &Node) -> Vec<String> {
        let Some(resource) = model.resource(node) else {
            return Vec::new();
        };
        resource
            .label_relations()
            .iter()
            .map(|(to, origin)| format!("{to} ({origin})"))
            .collect()
    }

    /// Example 88 — a link between two labels. Consistent, and it entails its converse under S62.
    #[test]
    fn example_88_a_link_between_two_labels_is_consistent_and_closes_under_s62() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements(vec![
            typed(&a, SkosClass::Label),
            literal_form(&a, plain("love")),
            typed(&b, SkosClass::Label),
            literal_form(&b, plain("adoration")),
            label_relation(&a, &b),
        ]);

        assert!(model.is_consistent(), "{:?}", model.findings());
        assert!(model.findings().is_empty(), "{:?}", model.findings());
        assert_eq!(
            relations(&model, &a),
            ["<http://example.com/ns/B> (asserted)"]
        );
        // The direction the graph did not state, and it says so.
        assert_eq!(
            relations(&model, &b),
            ["<http://example.com/ns/A> (inferred, S62)"]
        );
        assert_eq!(
            model
                .derivations()
                .iter()
                .filter(|d| d.rule == SkosRule::S62)
                .map(|d| format!("{} because {}", d.conclusion, d.premise))
                .collect::<Vec<_>>(),
            ["<http://example.com/ns/B> skosxl:labelRelation <http://example.com/ns/A> because <http://example.com/ns/A> skosxl:labelRelation <http://example.com/ns/B>"]
        );
    }

    /// A graph that states both directions has two asserted links and nothing inferred.
    ///
    /// The same rule every origin in this crate follows: the graph said it, so claiming to have
    /// deduced it would be a derivation nobody needed.
    #[test]
    fn a_link_stated_both_ways_is_asserted_at_both_ends_and_entails_nothing() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements(vec![
            typed(&a, SkosClass::Label),
            literal_form(&a, plain("love")),
            typed(&b, SkosClass::Label),
            literal_form(&b, plain("adoration")),
            label_relation(&a, &b),
            label_relation(&b, &a),
        ]);

        assert_eq!(
            relations(&model, &a),
            ["<http://example.com/ns/B> (asserted)"]
        );
        assert_eq!(
            relations(&model, &b),
            ["<http://example.com/ns/A> (asserted)"]
        );
        assert!(
            !model.derivations().iter().any(|d| d.rule == SkosRule::S62),
            "{:?}",
            model.derivations()
        );
    }

    /// S60 and S61 — a link is enough to make both ends a label, with no `rdf:type` anywhere.
    ///
    /// And the consequence, which is the point of running the class rules before S52's count: two
    /// labels with no literal form, each reported as ill-formed, and the graph still consistent.
    #[test]
    fn s60_and_s61_make_both_ends_of_a_link_a_label_without_any_rdf_type() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements(vec![label_relation(&a, &b)]);

        for (node, rule) in [(&a, SkosRule::S60), (&b, SkosRule::S61)] {
            let resource = model.resource(node).expect("a label");
            assert_eq!(
                resource.classes().get(&SkosClass::Label),
                Some(&ClassOrigin::Entailed(rule)),
                "{node}"
            );
        }
        assert_eq!(model.count_of(SkosClass::Label), 2);
        assert_eq!(
            model
                .findings()
                .iter()
                .filter(|finding| matches!(finding, Finding::NoLiteralForm { .. }))
                .count(),
            2,
            "{:?}",
            model.findings()
        );
        // Ill-formed, not inconsistent: S52 entails that a form exists, it does not require the
        // graph to state one.
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    /// An `rdf:type` already in the graph is not restated as an inference by S60 or S61.
    #[test]
    fn a_link_does_not_re_derive_a_label_class_the_graph_asserted() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements(vec![
            typed(&a, SkosClass::Label),
            typed(&b, SkosClass::Label),
            literal_form(&a, plain("love")),
            literal_form(&b, plain("adoration")),
            label_relation(&a, &b),
        ]);

        for node in [&a, &b] {
            assert_eq!(
                model
                    .resource(node)
                    .and_then(|resource| resource.classes().get(&SkosClass::Label)),
                Some(&ClassOrigin::Asserted),
                "{node}"
            );
        }
        assert!(
            !model
                .derivations()
                .iter()
                .any(|d| matches!(d.rule, SkosRule::S60 | SkosRule::S61)),
            "{:?}",
            model.derivations()
        );
    }

    /// S59 — the property is an `owl:ObjectProperty`, so a literal is a contradiction.
    #[test]
    fn s59_a_literal_label_relation_is_inconsistent() {
        let a = ex("A");
        let model = CoreModel::from_statements(vec![
            typed(&a, SkosClass::Label),
            literal_form(&a, plain("love")),
            Statement::new(a.clone(), SKOSXL_LABEL_RELATION, plain("adoration")),
        ]);

        assert!(!model.is_consistent(), "{:?}", model.findings());
        assert!(
            model.findings().iter().any(|finding| matches!(
                finding,
                Finding::LiteralOnObjectProperty {
                    property,
                    rule: SkosRule::S59,
                    ..
                } if property == "skosxl:labelRelation"
            )),
            "{:?}",
            model.findings()
        );
        // The value is dropped rather than kept as a link to a literal.
        assert!(relations(&model, &a).is_empty());
    }

    /// S60 and S48 together — linking to a concept by mistake is caught, and says why.
    ///
    /// This is the case that makes S60 and S61 worth applying rather than merely quoting: nothing
    /// in the graph types `<Love>` as a label, so without the domain rule the mistake is invisible.
    #[test]
    fn linking_a_concept_by_mistake_violates_s48_through_s60() {
        let (love, a) = (ex("Love"), ex("A"));
        let model = CoreModel::from_statements(vec![
            typed(&love, SkosClass::Concept),
            typed(&a, SkosClass::Label),
            literal_form(&a, plain("love")),
            label_relation(&love, &a),
        ]);

        assert!(!model.is_consistent(), "{:?}", model.findings());
        assert!(
            model.findings().iter().any(|finding| matches!(
                finding,
                Finding::DisjointClasses {
                    first: (SkosClass::Label, ClassOrigin::Entailed(SkosRule::S60)),
                    second: (SkosClass::Concept, ClassOrigin::Asserted),
                    rule: SkosRule::S48,
                    ..
                }
            )),
            "{:?}",
            model.findings()
        );
        // And the cascade is honest about itself: `<Love>` is now a label, so S52 reports that it
        // has no literal form as well. Two findings for one mistake, each true and each citing a
        // different statement, is the right answer — suppressing the second would mean deciding
        // which of two applicable rules the author meant to break.
        assert!(
            model.findings().iter().any(
                |finding| matches!(finding, Finding::NoLiteralForm { label } if *label == love)
            ),
            "{:?}",
            model.findings()
        );
    }

    /// A label linked to itself. Symmetric says nothing against it, so neither do we.
    #[test]
    fn a_label_linked_to_itself_is_its_own_converse_and_is_not_a_finding() {
        let a = ex("A");
        let model = CoreModel::from_statements(vec![
            typed(&a, SkosClass::Label),
            literal_form(&a, plain("love")),
            label_relation(&a, &a),
        ]);

        assert_eq!(
            relations(&model, &a),
            ["<http://example.com/ns/A> (asserted)"]
        );
        assert!(
            !model.derivations().iter().any(|d| d.rule == SkosRule::S62),
            "{:?}",
            model.derivations()
        );
        assert!(model.findings().is_empty(), "{:?}", model.findings());
    }

    /// Example 89 — and B.4.4.1's warning, which is the trap in this section.
    ///
    /// "Note that a sub-property of a symmetric property is not necessarily symmetric." "FAO" is
    /// an acronym for "Food and Agriculture Organization" and the converse is false, so closing
    /// `ex:acronym` would be wrong. We read no `rdfs:subPropertyOf`, so the refinement is
    /// invisible rather than mis-inferred — and this asserts both halves of that: no `ex:acronym`
    /// is invented in either direction, and the refinement does **not** reach
    /// `skosxl:labelRelation` either, which is the sound inference we are not yet making.
    #[test]
    fn example_89_a_refinement_of_a_symmetric_property_is_not_closed() {
        let acronym = format!("{EX}acronym");
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements(vec![
            Statement::new(
                Node::iri(acronym.clone()),
                format!("{}subPropertyOf", ns::RDFS),
                Node::iri(SKOSXL_LABEL_RELATION),
            ),
            typed(&a, SkosClass::Label),
            literal_form(&a, tagged("FAO", "en")),
            typed(&b, SkosClass::Label),
            literal_form(&b, tagged("Food and Agriculture Organization", "en")),
            Statement::new(b.clone(), acronym.clone(), a.clone()),
        ]);

        assert!(model.is_consistent(), "{:?}", model.findings());
        assert!(model.findings().is_empty(), "{:?}", model.findings());
        // Not closed, and not translated into the property it refines.
        assert!(
            relations(&model, &a).is_empty(),
            "{:?}",
            relations(&model, &a)
        );
        assert!(
            relations(&model, &b).is_empty(),
            "{:?}",
            relations(&model, &b)
        );
        assert!(
            !model
                .derivations()
                .iter()
                .any(|d| d.conclusion.contains("acronym") || d.rule == SkosRule::S62),
            "{:?}",
            model.derivations()
        );
    }

    /// The order statements arrive in does not change the answer for B.4 either.
    #[test]
    fn label_relations_read_the_same_in_either_direction() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let statements = vec![
            literal_form(&a, tagged("love", "en")),
            label_relation(&a, &b),
            label_relation(&c, &a),
            literal_form(&b, tagged("adoration", "en")),
            typed(&c, SkosClass::Label),
        ];

        let forwards = CoreModel::from_statements(statements.clone());
        let backwards = CoreModel::from_statements(statements.into_iter().rev());

        assert_eq!(forwards.resources, backwards.resources);
        assert_eq!(forwards.findings, backwards.findings);
        assert_eq!(
            relations(&forwards, &a),
            [
                "<http://example.com/ns/B> (asserted)",
                "<http://example.com/ns/C> (inferred, S62)"
            ]
        );
    }

    /// Every statement this crate relies on carries the specification's own wording, and B.4's
    /// four are the ones a reader is most likely to have to check: the appendix states no
    /// integrity conditions, so the citation is all the authority a finding here has.
    #[test]
    fn the_b4_statements_are_quoted_from_the_specification() {
        assert_eq!(
            SkosRule::S59.statement(),
            "skosxl:labelRelation is an instance of owl:ObjectProperty."
        );
        assert_eq!(
            SkosRule::S60.statement(),
            "The rdfs:domain of skosxl:labelRelation is the class skosxl:Label."
        );
        assert_eq!(
            SkosRule::S61.statement(),
            "The rdfs:range of skosxl:labelRelation is the class skosxl:Label."
        );
        assert_eq!(
            SkosRule::S62.statement(),
            "skosxl:labelRelation is an instance of owl:SymmetricProperty."
        );
    }

    // ---- Semantic relations, §8 (S18–S26). S24 and S27 are the next build-plan item. ----

    /// The link the graph stated, and the three the four closure rules entail from it.
    #[test]
    fn one_broader_statement_produces_all_four_links() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements([s(&a, &skos("broader"), &b)]);

        let from_a = model.resource(&a).expect("A is in the model");
        assert_eq!(
            from_a
                .relations(SemanticRelation::Broader)
                .and_then(|links| links.get(&b)),
            Some(&RelationOrigin::Asserted)
        );
        assert_eq!(
            from_a
                .relations(SemanticRelation::BroaderTransitive)
                .and_then(|links| links.get(&b)),
            Some(&RelationOrigin::Entailed(SkosRule::S22)),
            "S22 lifts skos:broader to skos:broaderTransitive"
        );

        let from_b = model.resource(&b).expect("B is in the model");
        assert_eq!(
            from_b
                .relations(SemanticRelation::Narrower)
                .and_then(|links| links.get(&a)),
            Some(&RelationOrigin::Entailed(SkosRule::S25)),
            "S25 makes skos:narrower the inverse of skos:broader"
        );
        assert_eq!(
            from_b
                .relations(SemanticRelation::NarrowerTransitive)
                .and_then(|links| links.get(&a)),
            Some(&RelationOrigin::Entailed(SkosRule::S22)),
            "the converse lifts too, because the inverses run first"
        );
    }

    /// Whichever direction the author typed, the model holds the same four links. If the S22 lift
    /// ran before the inverse pass this test would fail in exactly one of its two halves.
    #[test]
    fn the_direction_the_author_typed_does_not_change_the_answer() {
        let (a, b) = (ex("A"), ex("B"));
        let downwards = CoreModel::from_statements([s(&a, &skos("broader"), &b)]);
        let upwards = CoreModel::from_statements([s(&b, &skos("narrower"), &a)]);

        for relation in SemanticRelation::ALL {
            for node in [&a, &b] {
                let one = downwards
                    .resource(node)
                    .and_then(|resource| resource.relations(relation))
                    .map(|links| links.keys().cloned().collect::<Vec<_>>());
                let other = upwards
                    .resource(node)
                    .and_then(|resource| resource.relations(relation))
                    .map(|links| links.keys().cloned().collect::<Vec<_>>());
                assert_eq!(one, other, "{node} {relation}");
            }
        }
    }

    /// S23 — `skos:related` is symmetric, so one statement is one link in both directions.
    #[test]
    fn a_related_link_is_symmetric_under_s23() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements([s(&a, &skos("related"), &b)]);

        assert_eq!(
            model
                .resource(&b)
                .and_then(|resource| resource.relations(SemanticRelation::Related))
                .and_then(|links| links.get(&a)),
            Some(&RelationOrigin::Entailed(SkosRule::S23))
        );
        // Symmetry must not leak into the hierarchy: an associative link is not a broader one.
        assert!(model
            .resource(&a)
            .and_then(|resource| resource.relations(SemanticRelation::Broader))
            .is_none());
        assert!(model
            .resource(&a)
            .and_then(|resource| resource.relations(SemanticRelation::BroaderTransitive))
            .is_none());
    }

    /// A graph that states both directions has two asserted links and nothing to derive. Claiming
    /// to have concluded what the file says outright would be a derivation nobody needed.
    #[test]
    fn a_direction_the_graph_states_is_never_reported_as_inferred() {
        let (a, b) = (ex("A"), ex("B"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("narrower"), &a)]);

        assert_eq!(
            model
                .resource(&b)
                .and_then(|resource| resource.relations(SemanticRelation::Narrower))
                .and_then(|links| links.get(&a)),
            Some(&RelationOrigin::Asserted)
        );
        assert!(
            !model
                .derivations()
                .iter()
                .any(|derivation| derivation.rule == SkosRule::S25),
            "S25 had nothing left to conclude: {:?}",
            model.derivations()
        );
    }

    /// S26 — the two transitive variants are each other's inverse, which is the only way an
    /// asserted `skos:broaderTransitive` gets its converse. S22 cannot supply it: sub-property
    /// entailment runs upwards, so nothing lifts a transitive link down to `skos:broader`.
    #[test]
    fn an_asserted_transitive_link_gets_its_converse_under_s26() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements([s(&a, &skos("broaderTransitive"), &b)]);

        assert_eq!(
            model
                .resource(&b)
                .and_then(|resource| resource.relations(SemanticRelation::NarrowerTransitive))
                .and_then(|links| links.get(&a)),
            Some(&RelationOrigin::Entailed(SkosRule::S26))
        );
        assert!(
            model
                .resource(&a)
                .and_then(|resource| resource.relations(SemanticRelation::Broader))
                .is_none(),
            "skos:broaderTransitive does not entail skos:broader"
        );
    }

    /// Polyhierarchy: §8 states nothing against a concept with two broader concepts, ISO 25964
    /// relies on it, and it is counted rather than reported.
    #[test]
    fn a_concept_may_have_two_broader_concepts_and_it_is_not_a_finding() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&a, &skos("broader"), &c)]);

        assert_eq!(
            model.resource(&a).map(Resource::broader_count),
            Some(2),
            "both broader concepts are kept"
        );
        assert!(model.findings().is_empty(), "{:?}", model.findings());
        assert!(model.is_consistent());
    }

    /// S19 and S20 report nothing when the graph has already typed its concepts — the ordinary
    /// case, and the reason this pass is quiet on a well-formed vocabulary.
    #[test]
    fn the_domain_and_range_are_silent_on_a_vocabulary_that_types_its_concepts() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements([
            typed(&a, SkosClass::Concept),
            typed(&b, SkosClass::Concept),
            s(&a, &skos("broader"), &b),
        ]);

        assert!(
            !model
                .derivations()
                .iter()
                .any(|derivation| matches!(derivation.rule, SkosRule::S19 | SkosRule::S20)),
            "{:?}",
            model.derivations()
        );
        assert!(
            !model
                .derivations()
                .iter()
                .any(|derivation| derivation.rule == SkosRule::S21),
            "the S21 step is only recorded when a class follows from it"
        );
    }

    /// The chain a report prints for a concept typed out of a `skos:broader` statement: S22 to the
    /// transitive variant, S21 to `skos:semanticRelation`, then S19 and S20. Citing S19 against
    /// the `skos:broader` statement itself would name a statement that does not mention the
    /// property the author wrote.
    #[test]
    fn typing_a_concept_from_a_relation_prints_the_whole_chain() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements([s(&a, &skos("broader"), &b)]);

        assert_eq!(
            model
                .resource(&a)
                .and_then(|r| r.classes().get(&SkosClass::Concept)),
            Some(&ClassOrigin::Entailed(SkosRule::S19))
        );
        assert_eq!(
            model
                .resource(&b)
                .and_then(|r| r.classes().get(&SkosClass::Concept)),
            Some(&ClassOrigin::Entailed(SkosRule::S20))
        );

        let step = |rule: SkosRule| {
            model
                .derivations()
                .iter()
                .find(|derivation| derivation.rule == rule)
                .unwrap_or_else(|| panic!("no {} step in {:?}", rule.number(), model.derivations()))
        };
        assert_eq!(
            step(SkosRule::S22).conclusion,
            format!("<{EX}A> skos:broaderTransitive <{EX}B>")
        );
        assert_eq!(
            step(SkosRule::S21).premise,
            format!("<{EX}A> skos:broaderTransitive <{EX}B>"),
            "S21 does not name skos:broader, so the citation goes through the variant"
        );
        assert_eq!(
            step(SkosRule::S19).premise,
            format!("<{EX}A> skos:semanticRelation <{EX}B>")
        );
    }

    /// The mistake the domain and range exist to make visible. Without S19 and S20 this graph
    /// reads as clean: nothing else in it would ever type the collection as a concept.
    #[test]
    fn a_broader_link_to_a_collection_is_caught_by_s37() {
        let (a, group) = (ex("A"), ex("Group"));
        let model = CoreModel::from_statements([
            typed(&group, SkosClass::Collection),
            s(&a, &skos("broader"), &group),
        ]);

        assert!(!model.is_consistent());
        assert!(
            model.findings().iter().any(|finding| matches!(
                finding,
                Finding::DisjointClasses {
                    resource,
                    rule: SkosRule::S37,
                    ..
                } if resource == &group
            )),
            "{:?}",
            model.findings()
        );
    }

    /// S18 makes all six object properties, so a literal is the contradiction it is under S3 and
    /// S30 — and the link is dropped rather than filed under a property whose value it cannot be.
    #[test]
    fn a_literal_on_a_semantic_relation_is_refused_under_s18() {
        let a = ex("A");
        let model = CoreModel::from_statements([Statement::new(
            a.clone(),
            skos("broader"),
            plain("something broader"),
        )]);

        assert!(model.findings().iter().any(|finding| matches!(
            finding,
            Finding::LiteralOnObjectProperty {
                rule: SkosRule::S18,
                ..
            }
        )));
        assert!(model
            .resource(&a)
            .and_then(|resource| resource.relations(SemanticRelation::Broader))
            .is_none());
    }

    /// `skos:semanticRelation` stated outright: S19 and S20 type both ends, and no sub-property
    /// is invented, because the entailment runs upwards and the statement says only that *one* of
    /// the five holds.
    #[test]
    fn the_super_property_types_both_ends_and_is_filed_under_nothing() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements([s(&a, &skos("semanticRelation"), &b)]);

        assert!(model
            .resource(&a)
            .is_some_and(|r| r.is_a(SkosClass::Concept)));
        assert!(model
            .resource(&b)
            .is_some_and(|r| r.is_a(SkosClass::Concept)));
        for relation in SemanticRelation::ALL {
            assert!(
                model
                    .resource(&a)
                    .and_then(|resource| resource.relations(relation))
                    .is_none(),
                "{relation} was invented from the super-property"
            );
        }
        assert!(
            !model
                .derivations()
                .iter()
                .any(|derivation| derivation.rule == SkosRule::S21),
            "there is no sub-property step to take: the graph stated the super-property"
        );
    }

    /// §8.6.2's Example 26 is inconsistent, and we do **not** say so yet. S27 is the next
    /// build-plan item and this asserts the honest current behaviour rather than a claim we
    /// cannot back — see `docs/UNTESTED.md`. It also pins what *is* true today: both links are
    /// there, closed, and waiting for the condition that reads them.
    #[test]
    fn example_26_is_not_yet_reported_and_its_links_are_both_present() {
        let (a, b) = (ex("A"), ex("B"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&a, &skos("related"), &b)]);

        assert!(model
            .resource(&a)
            .and_then(|resource| resource.relations(SemanticRelation::BroaderTransitive))
            .is_some_and(|links| links.contains_key(&b)));
        assert!(model
            .resource(&a)
            .and_then(|resource| resource.relations(SemanticRelation::Related))
            .is_some_and(|links| links.contains_key(&b)));
        assert!(
            model.is_consistent(),
            "S27 is not implemented, so nothing may claim to have checked it"
        );
    }

    /// S24 is not applied, and a test says so rather than leaving it to be discovered. Example 27
    /// needs the closure — `<A> broader <B>`, `<B> broader <C>` makes `<A>` transitively broader
    /// than `<C>` — and the model must not pretend it has it.
    #[test]
    fn the_transitive_closure_is_not_taken_and_stops_at_one_step() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        let from_a = model
            .resource(&a)
            .and_then(|resource| resource.relations(SemanticRelation::BroaderTransitive))
            .expect("A has a transitive link to B");
        assert!(from_a.contains_key(&b));
        assert!(
            !from_a.contains_key(&c),
            "S24 is the next item; claiming the closure now would be a green we did not earn"
        );
    }

    /// A blank node is a perfectly good concept, and the closure must not lose one.
    #[test]
    fn a_relation_between_blank_nodes_closes_like_any_other() {
        let (a, b) = (Node::blank("a"), Node::blank("b"));
        let model = CoreModel::from_statements([s(&a, &skos("narrower"), &b)]);

        assert_eq!(
            model
                .resource(&b)
                .and_then(|resource| resource.relations(SemanticRelation::Broader))
                .and_then(|links| links.get(&a)),
            Some(&RelationOrigin::Entailed(SkosRule::S25))
        );
    }

    /// A concept related to itself, or broader than itself. §8 states no condition against either
    /// — S27 is about `skos:related` and `skos:broaderTransitive` *together* — so neither is a
    /// finding, and inventing one would be the failure `docs/COMPETITIVE.md` records against the
    /// incumbents. The same decision `adr/0022` records for a label linked to itself.
    #[test]
    fn a_concept_related_to_itself_is_not_a_finding() {
        let a = ex("A");
        let model =
            CoreModel::from_statements([s(&a, &skos("related"), &a), s(&a, &skos("broader"), &a)]);

        assert!(model.findings().is_empty(), "{:?}", model.findings());
        assert_eq!(
            model
                .resource(&a)
                .and_then(|resource| resource.relations(SemanticRelation::Related))
                .and_then(|links| links.get(&a)),
            Some(&RelationOrigin::Asserted),
            "the reflexive pair is its own converse, so S23 has nothing to add"
        );
    }
}
