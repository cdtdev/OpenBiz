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
//!   under a heading of that name. A vocabulary in this state is not a SKOS vocabulary.
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

/// One of the four classes of the SKOS core model.
///
/// SKOS-XL's `skosxl:Label` is a fifth class and is not here: it is its own build-plan item, and
/// adding it as a placeholder would make [`CoreModel`] report on something it does not understand.
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
}

impl SkosClass {
    /// Every class, in a stable order.
    pub const ALL: [SkosClass; 4] = [
        SkosClass::Concept,
        SkosClass::ConceptScheme,
        SkosClass::Collection,
        SkosClass::OrderedCollection,
    ];

    /// The class's IRI.
    pub fn iri(self) -> String {
        format!("{}{}", ns::SKOS, self.local_name())
    }

    /// The local name within the SKOS namespace.
    pub fn local_name(self) -> &'static str {
        match self {
            SkosClass::Concept => "Concept",
            SkosClass::ConceptScheme => "ConceptScheme",
            SkosClass::Collection => "Collection",
            SkosClass::OrderedCollection => "OrderedCollection",
        }
    }

    /// The class an IRI names, or `None` if it names something outside the SKOS core model.
    ///
    /// A vocabulary is full of `rdf:type` statements about classes that are not ours — `owl:Class`
    /// and `owl:Ontology` are both explicitly permitted alongside SKOS types by Examples 3 and 7 —
    /// so this returning `None` is the ordinary case, not an error.
    pub fn from_iri(iri: &str) -> Option<Self> {
        let local = iri.strip_prefix(ns::SKOS)?;
        SkosClass::ALL
            .into_iter()
            .find(|class| class.local_name() == local)
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
        write!(f, "skos:{}", self.local_name())
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
    S29,
    S31,
    S33,
    S36,
    S37,
    S3,
    S30,
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
            SkosRule::S29 => "S29",
            SkosRule::S30 => "S30",
            SkosRule::S31 => "S31",
            SkosRule::S33 => "S33",
            SkosRule::S36 => "S36",
            SkosRule::S37 => "S37",
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
    /// An object property was given a literal value. S3 or S30.
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
            Finding::DefectiveMemberList { .. }
            | Finding::MultipleMemberLists { .. }
            | Finding::NonPlainLiteralLabel { .. } => Severity::IllFormed,
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
    labels: BTreeMap<LexicalLabel, BTreeSet<LabelKind>>,
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

    /// Every label it carries, and which properties carry each one.
    ///
    /// Ordered by language tag and then by lexical form, so iterating groups a resource's labels
    /// by language. A label under more than one property is a [`Finding::LabelPropertiesClash`];
    /// the map keeps it once, with both kinds, rather than reporting it twice.
    pub fn labels(&self) -> &BTreeMap<LexicalLabel, BTreeSet<LabelKind>> {
        &self.labels
    }

    /// Its labels of one kind, in the same order.
    pub fn labels_of(&self, kind: LabelKind) -> impl Iterator<Item = &LexicalLabel> {
        self.labels
            .iter()
            .filter(move |(_, kinds)| kinds.contains(&kind))
            .map(|(label, _)| label)
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
                for kind in kinds {
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
/// scans, and what is kept is proportional to the resources the model has something to say about
/// rather than to the size of the graph — a vocabulary's labels and notes, which are most of its
/// statements, are counted and dropped.
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
        self.attach_labels(&mut model);
        Self::check_disjointness(&mut model);
        Self::check_label_conditions(&mut model);

        model
    }

    /// Hand each resource the labels read for it.
    ///
    /// Labels entail no class. §5.6.1 states that the three properties have **no domain**, so
    /// their effective domain is `rdfs:Resource` — Example 16 labels an `owl:Class` and is
    /// consistent. A model that made a `skos:Concept` out of anything with a `skos:prefLabel`
    /// would miscount every vocabulary that labels its own concept scheme, which is most of them.
    fn attach_labels(&mut self, model: &mut CoreModel) {
        for (node, labels) in std::mem::take(&mut self.labels) {
            model.resources.entry(node).or_default().labels = labels;
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
                        kinds: kinds.iter().copied().collect(),
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
        const DISJOINT: [(SkosClass, SkosClass, SkosRule); 3] = [
            (SkosClass::ConceptScheme, SkosClass::Concept, SkosRule::S9),
            (SkosClass::Collection, SkosClass::Concept, SkosRule::S37),
            (
                SkosClass::Collection,
                SkosClass::ConceptScheme,
                SkosRule::S37,
            ),
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
        // Right namespace, not one of ours: SKOS-XL's Label is a separate build-plan item.
        assert_eq!(SkosClass::from_iri(&skos("Label")), None);
        assert_eq!(SkosClass::from_iri(&format!("{}Label", ns::SKOSXL)), None);
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

    #[test]
    fn statements_outside_the_core_model_are_counted_and_dropped() {
        let model = CoreModel::from_statements([
            typed(&ex("A"), SkosClass::Concept),
            Statement::new(ex("A"), skos("prefLabel"), plain("Chemistry")),
            Statement::new(ex("A"), skos("broader"), ex("B")),
        ]);

        assert_eq!(model.statements_read(), 3);
        // Labels and semantic relations are their own items; the model must not half-report them.
        assert_eq!(model.count_of(SkosClass::Concept), 1);
        assert!(model.resource(&ex("B")).is_none());
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
}
