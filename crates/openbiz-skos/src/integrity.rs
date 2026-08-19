//! The integrity conditions, as a roll-call — every one named, and each verdict said out loud.
//!
//! Five sections of the SKOS Reference carry a heading called "Integrity Conditions" — §4.4, §5.4,
//! §8.4, §9.4 and §10.4 — and between them they state **six** conditions: [`S9`](SkosRule::S9),
//! [`S13`](SkosRule::S13), [`S14`](SkosRule::S14), [`S27`](SkosRule::S27),
//! [`S37`](SkosRule::S37) and [`S46`](SkosRule::S46). §6, §7 and Appendix B have no such heading,
//! and the `notes` and `xl` modules record what follows from that.
//!
//! Each of the six was implemented by the item that owned its section, and each has had a test
//! citing its S-number since the day it landed. What did not exist until this module is the
//! **roll-call**: the answer to "which of them did you check on *my* vocabulary, and what did each
//! one say". That is a different question from "is it consistent", and it is the one a governance
//! team has to answer to an auditor.
//!
//! # Held, violated, and the third answer that matters most
//!
//! [`Verdict`] has three values and the third is the point of the module.
//!
//! - [`Verdict::Violated`] — we found a counter-example, and it is carried.
//! - [`Verdict::Held`] — the check ran over the whole vocabulary and found nothing.
//! - [`Verdict::Unchecked`] — the check did **not** run over the whole vocabulary, so this
//!   condition has no verdict at all.
//!
//! Collapsing the third into the second is the false green this build spends most of its
//! defensive effort on, and [`Severity::Unchecked`](crate::Severity::Unchecked) already exists
//! for it. What is new here is that incompleteness is **attributed**: an exhausted ancestry walk
//! makes S27 unchecked and says nothing whatever about S13, and until now
//! [`CoreModel::checks_are_complete`](crate::CoreModel::checks_are_complete) answered for the
//! whole model at once, so one bounded walk clouded every condition equally.
//!
//! # The second reason a condition can be unchecked, and it is not a bound
//!
//! §7.1 hands a vocabulary `rdfs:subPropertyOf` as an extension point, and the `refinement` module
//! resolves it — **for the seven documentation properties only**. Nothing else is resolved, and
//! `rdfs:subClassOf` is not read at all. So a vocabulary declaring
//!
//! ```turtle
//! ex:seeAlso rdfs:subPropertyOf skos:related .
//! ```
//!
//! makes statements this build reads as non-SKOS, and S27 is then checked over a hierarchy with
//! the author's own associative links missing from it. Reporting that as "S27 held" would be a
//! false negative produced by an entailment we chose not to perform, which is exactly the shape
//! iteration 33 found in S46. So a declared refinement that reaches a term a condition is stated
//! over makes that condition [`Verdict::Unchecked`], names the declaration, and prints the chain.
//!
//! Two things this deliberately does **not** do. It does not perform the entailment — that is a
//! decision about closure with the same shape as the one B.4.4.1 blocks for
//! `skosxl:labelRelation`, and it belongs to an item of its own rather than to a report. And it
//! does not fire on the SKOS ontology's own statements: a vocabulary that imports SKOS carries
//! `skos:broader rdfs:subPropertyOf skos:broaderTransitive`, which is S22 and is already applied,
//! so a declaration whose subject is itself a SKOS or SKOS-XL term is read, counted and ignored.
//!
//! # Sixteen rows, in two groups, because ten of them are ours
//!
//! [`CONDITIONS`] is longer than six. Ten further statements can make this build call a graph
//! inconsistent — the SKOS-XL disjointness statements, the "exactly one literal form" restriction,
//! and the object- and datatype-property typing rules — and none of them sits under an "Integrity
//! Conditions" heading. Leaving them out would produce a report saying all six conditions held on
//! a vocabulary this build calls inconsistent, which is worse than a long table. So they are in,
//! under [`Authority::OurReading`], separated in the report and labelled as our judgement.
//!
//! That gives the roll-call a property worth stating as a test rather than as prose: **every
//! finding this build classifies [`Severity::Inconsistent`](crate::Severity::Inconsistent) is
//! attributed to a row in this table**, so a graph is consistent exactly when no row is violated.
//! A future finding that forgets to register here fails that test rather than silently going
//! unreported.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::labels::{SKOS_ALT_LABEL, SKOS_HIDDEN_LABEL, SKOS_PREF_LABEL};
use crate::mapping::{
    SKOS_BROAD_MATCH, SKOS_CLOSE_MATCH, SKOS_EXACT_MATCH, SKOS_MAPPING_RELATION, SKOS_NARROW_MATCH,
    SKOS_RELATED_MATCH,
};
use crate::model::{
    CoreModel, Finding, Node, SkosRule, Term, SKOS_HAS_TOP_CONCEPT, SKOS_IN_SCHEME, SKOS_MEMBER,
    SKOS_MEMBER_LIST, SKOS_TOP_CONCEPT_OF,
};
use crate::ns;
use crate::relations::{
    SKOS_BROADER, SKOS_BROADER_TRANSITIVE, SKOS_NARROWER, SKOS_NARROWER_TRANSITIVE, SKOS_RELATED,
    SKOS_SEMANTIC_RELATION,
};
use crate::xl::{
    SKOSXL_ALT_LABEL, SKOSXL_HIDDEN_LABEL, SKOSXL_LABEL, SKOSXL_LABEL_RELATION,
    SKOSXL_LITERAL_FORM, SKOSXL_PREF_LABEL,
};

/// `rdfs:subClassOf` — read here and nowhere else, and read only to say we did not use it.
pub const RDFS_SUB_CLASS_OF: &str = "http://www.w3.org/2000/01/rdf-schema#subClassOf";

/// `rdfs:subPropertyOf`.
pub const RDFS_SUB_PROPERTY_OF: &str = crate::refinement::RDFS_SUB_PROPERTY_OF;

const SKOS_CONCEPT: &str = "http://www.w3.org/2004/02/skos/core#Concept";
const SKOS_CONCEPT_SCHEME: &str = "http://www.w3.org/2004/02/skos/core#ConceptScheme";
const SKOS_COLLECTION: &str = "http://www.w3.org/2004/02/skos/core#Collection";
const SKOS_ORDERED_COLLECTION: &str = "http://www.w3.org/2004/02/skos/core#OrderedCollection";

/// Every term whose statements can put a resource into one of the four SKOS classes.
///
/// One list serves S9, S37 and S48 rather than three narrower ones, and that is conservative on
/// purpose: the classes are entailed through S4–S8, S19–S21, S31, S33 and S39–S41, so a link this
/// build never read can produce a class membership several steps from the property that was
/// written. A caveat that names one condition too many costs a reader a sentence; one that names
/// one too few is the false negative the module exists to prevent.
const CLASS_BEARING: &[&str] = &[
    SKOS_CONCEPT,
    SKOS_CONCEPT_SCHEME,
    SKOS_COLLECTION,
    SKOS_ORDERED_COLLECTION,
    SKOS_IN_SCHEME,
    SKOS_HAS_TOP_CONCEPT,
    SKOS_TOP_CONCEPT_OF,
    SKOS_MEMBER,
    SKOS_MEMBER_LIST,
    SKOS_SEMANTIC_RELATION,
    SKOS_BROADER,
    SKOS_NARROWER,
    SKOS_RELATED,
    SKOS_BROADER_TRANSITIVE,
    SKOS_NARROWER_TRANSITIVE,
    SKOS_MAPPING_RELATION,
    SKOS_CLOSE_MATCH,
    SKOS_EXACT_MATCH,
    SKOS_BROAD_MATCH,
    SKOS_NARROW_MATCH,
    SKOS_RELATED_MATCH,
];

/// The same, plus the SKOS-XL terms that make a resource an `skosxl:Label`.
const CLASS_BEARING_WITH_XL: &[&str] = &[
    SKOS_CONCEPT,
    SKOS_CONCEPT_SCHEME,
    SKOS_COLLECTION,
    SKOS_ORDERED_COLLECTION,
    SKOS_IN_SCHEME,
    SKOS_HAS_TOP_CONCEPT,
    SKOS_TOP_CONCEPT_OF,
    SKOS_MEMBER,
    SKOS_MEMBER_LIST,
    SKOS_SEMANTIC_RELATION,
    SKOS_BROADER,
    SKOS_NARROWER,
    SKOS_RELATED,
    SKOS_BROADER_TRANSITIVE,
    SKOS_NARROWER_TRANSITIVE,
    SKOS_MAPPING_RELATION,
    SKOS_CLOSE_MATCH,
    SKOS_EXACT_MATCH,
    SKOS_BROAD_MATCH,
    SKOS_NARROW_MATCH,
    SKOS_RELATED_MATCH,
    SKOSXL_LABEL,
    SKOSXL_LITERAL_FORM,
    SKOSXL_PREF_LABEL,
    SKOSXL_ALT_LABEL,
    SKOSXL_HIDDEN_LABEL,
    SKOSXL_LABEL_RELATION,
];

/// Every term that can carry a lexical label into the buckets S13 compares.
const LABEL_BEARING: &[&str] = &[
    SKOS_PREF_LABEL,
    SKOS_ALT_LABEL,
    SKOS_HIDDEN_LABEL,
    SKOSXL_PREF_LABEL,
    SKOSXL_ALT_LABEL,
    SKOSXL_HIDDEN_LABEL,
    SKOSXL_LITERAL_FORM,
];

/// Every term that can carry a *preferred* label, which is all S14 counts.
const PREFERRED_LABEL_BEARING: &[&str] = &[SKOS_PREF_LABEL, SKOSXL_PREF_LABEL, SKOSXL_LITERAL_FORM];

/// §8's properties and §10's, because S39–S41 lift every mapping link into a semantic relation.
const HIERARCHY_BEARING: &[&str] = &[
    SKOS_SEMANTIC_RELATION,
    SKOS_BROADER,
    SKOS_NARROWER,
    SKOS_RELATED,
    SKOS_BROADER_TRANSITIVE,
    SKOS_NARROWER_TRANSITIVE,
    SKOS_MAPPING_RELATION,
    SKOS_CLOSE_MATCH,
    SKOS_EXACT_MATCH,
    SKOS_BROAD_MATCH,
    SKOS_NARROW_MATCH,
    SKOS_RELATED_MATCH,
];

/// §10's six properties.
const MAPPING_BEARING: &[&str] = &[
    SKOS_MAPPING_RELATION,
    SKOS_CLOSE_MATCH,
    SKOS_EXACT_MATCH,
    SKOS_BROAD_MATCH,
    SKOS_NARROW_MATCH,
    SKOS_RELATED_MATCH,
];

/// The three SKOS-XL labelling properties.
const XL_LABEL_PROPERTIES: &[&str] = &[SKOSXL_PREF_LABEL, SKOSXL_ALT_LABEL, SKOSXL_HIDDEN_LABEL];

/// Who says a violation of this statement makes a graph inconsistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Authority {
    /// The SKOS Reference, under a heading called "Integrity Conditions".
    Specification,
    /// Us. The specification states the axiom and does not call a violation of it an
    /// inconsistency; a resource in two disjoint classes is a contradiction whatever heading it
    /// sits under, and saying so is our reading rather than the document's.
    OurReading,
}

impl fmt::Display for Authority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Authority::Specification => write!(f, "an integrity condition of the specification"),
            Authority::OurReading => write!(f, "a contradiction by our reading"),
        }
    }
}

/// One row of the roll-call: a statement whose violation this build calls an inconsistency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegrityCondition {
    rule: SkosRule,
    section: &'static str,
    forbids: &'static str,
    authority: Authority,
    terms: &'static [&'static str],
}

impl IntegrityCondition {
    /// The specification statement, by number.
    pub fn rule(&self) -> SkosRule {
        self.rule
    }

    /// Where the SKOS Reference states it. A `§N.4` is a section headed "Integrity Conditions".
    pub fn section(&self) -> &'static str {
        self.section
    }

    /// What a violating graph contains, in one clause an author can act on.
    pub fn forbids(&self) -> &'static str {
        self.forbids
    }

    /// Whether the specification calls a violation an inconsistency, or we do.
    pub fn authority(&self) -> Authority {
        self.authority
    }

    /// The SKOS and SKOS-XL terms the condition is checked over.
    ///
    /// Used to decide whether an unread `rdfs:subPropertyOf` or `rdfs:subClassOf` declaration
    /// leaves this condition without a verdict.
    pub fn terms(&self) -> &'static [&'static str] {
        self.terms
    }
}

/// Every statement whose violation makes this build call a graph inconsistent.
///
/// The first six are the specification's integrity conditions, in the order the document states
/// them. The rest are ours; see the module documentation for why they are here at all.
pub const CONDITIONS: &[IntegrityCondition] = &[
    IntegrityCondition {
        rule: SkosRule::S9,
        section: "§4.4",
        forbids: "a resource that is both a concept and a concept scheme",
        authority: Authority::Specification,
        terms: CLASS_BEARING,
    },
    IntegrityCondition {
        rule: SkosRule::S13,
        section: "§5.4",
        forbids: "one resource carrying the same label under two of the three labelling properties",
        authority: Authority::Specification,
        terms: LABEL_BEARING,
    },
    IntegrityCondition {
        rule: SkosRule::S14,
        section: "§5.4",
        forbids: "a resource with two preferred labels in one language",
        authority: Authority::Specification,
        terms: PREFERRED_LABEL_BEARING,
    },
    IntegrityCondition {
        rule: SkosRule::S27,
        section: "§8.4",
        forbids: "two concepts joined both associatively and hierarchically, however indirectly",
        authority: Authority::Specification,
        terms: HIERARCHY_BEARING,
    },
    IntegrityCondition {
        rule: SkosRule::S37,
        section: "§9.4",
        forbids: "a collection that is also a concept or a concept scheme",
        authority: Authority::Specification,
        terms: CLASS_BEARING,
    },
    IntegrityCondition {
        rule: SkosRule::S46,
        section: "§10.4",
        forbids: "an exact match between two concepts that are also broadly or associatively \
                  matched",
        authority: Authority::Specification,
        terms: MAPPING_BEARING,
    },
    IntegrityCondition {
        rule: SkosRule::S3,
        section: "§4.2",
        forbids: "a literal where §4's object properties take a resource",
        authority: Authority::OurReading,
        terms: &[SKOS_IN_SCHEME, SKOS_HAS_TOP_CONCEPT, SKOS_TOP_CONCEPT_OF],
    },
    IntegrityCondition {
        rule: SkosRule::S18,
        section: "§8.2",
        forbids: "a literal where §8's semantic relations take a concept",
        authority: Authority::OurReading,
        terms: &[
            SKOS_SEMANTIC_RELATION,
            SKOS_BROADER,
            SKOS_NARROWER,
            SKOS_RELATED,
            SKOS_BROADER_TRANSITIVE,
            SKOS_NARROWER_TRANSITIVE,
        ],
    },
    IntegrityCondition {
        rule: SkosRule::S30,
        section: "§9.2",
        forbids: "a literal where skos:member or skos:memberList takes a resource",
        authority: Authority::OurReading,
        terms: &[SKOS_MEMBER, SKOS_MEMBER_LIST],
    },
    IntegrityCondition {
        rule: SkosRule::S38,
        section: "§10.2",
        forbids: "a literal where §10's mapping properties take a concept",
        authority: Authority::OurReading,
        terms: MAPPING_BEARING,
    },
    IntegrityCondition {
        rule: SkosRule::S48,
        section: "Appendix B.2",
        forbids: "an SKOS-XL label that is also a concept, a concept scheme or a collection",
        authority: Authority::OurReading,
        terms: CLASS_BEARING_WITH_XL,
    },
    IntegrityCondition {
        rule: SkosRule::S49,
        section: "Appendix B.2",
        forbids: "a resource where skosxl:literalForm takes a literal",
        authority: Authority::OurReading,
        terms: &[SKOSXL_LITERAL_FORM],
    },
    IntegrityCondition {
        rule: SkosRule::S52,
        section: "Appendix B.2",
        forbids: "an SKOS-XL label with two literal forms",
        authority: Authority::OurReading,
        terms: &[SKOSXL_LITERAL_FORM, SKOSXL_LABEL],
    },
    IntegrityCondition {
        rule: SkosRule::S53,
        section: "Appendix B.3",
        forbids: "a literal where the SKOS-XL labelling properties take a label resource",
        authority: Authority::OurReading,
        terms: XL_LABEL_PROPERTIES,
    },
    IntegrityCondition {
        rule: SkosRule::S58,
        section: "Appendix B.3",
        forbids: "one resource carrying the same label resource under two SKOS-XL properties",
        authority: Authority::OurReading,
        terms: XL_LABEL_PROPERTIES,
    },
    IntegrityCondition {
        rule: SkosRule::S59,
        section: "Appendix B.4",
        forbids: "a literal where skosxl:labelRelation takes a label resource",
        authority: Authority::OurReading,
        terms: &[SKOSXL_LABEL_RELATION],
    },
];

/// What the roll-call says about one condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// A counter-example was found. The graph violates the condition.
    Violated,
    /// The check ran over the whole vocabulary and found nothing.
    Held,
    /// The check did not run over the whole vocabulary, so there is no verdict.
    ///
    /// **Not a weaker "held".** A bounded walk that gave up, or an entailment this build does not
    /// perform on a vocabulary that relies on it, both leave the condition genuinely unanswered.
    Unchecked,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Verdict::Violated => write!(f, "violated"),
            Verdict::Held => write!(f, "held"),
            Verdict::Unchecked => write!(f, "unchecked"),
        }
    }
}

/// Why a condition's check did not cover the whole vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caveat {
    /// A bounded walk stopped before the end. Carries the finding that says so.
    BoundReached(Finding),
    /// The vocabulary declares a refinement of a term this condition is checked over, and this
    /// build entails nothing from it.
    UnreadRefinement(UnreadRefinement),
    /// There were more `rdfs:subPropertyOf` and `rdfs:subClassOf` declarations than the scan
    /// would read, so we cannot say whether any of them reach this condition's terms.
    RefinementScanExhausted {
        /// Declarations read.
        read: usize,
        /// Declarations refused, either by the term ceiling or by the step budget.
        unread: usize,
    },
}

impl fmt::Display for Caveat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Caveat::BoundReached(finding) => write!(f, "{finding}"),
            Caveat::UnreadRefinement(refinement) => {
                write!(f, "{refinement}, and this build entails nothing from it")
            }
            Caveat::RefinementScanExhausted { read, unread } => write!(
                f,
                "{read} rdfs:subPropertyOf and rdfs:subClassOf declarations were resolved and \
                 {unread} were not, so whether any of them refines a term this condition is \
                 checked over is unknown"
            ),
        }
    }
}

/// One condition, and what this vocabulary had to say about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionOutcome {
    /// The condition.
    pub condition: IntegrityCondition,
    /// Every finding that violates it, in the order the model raised them.
    pub violations: Vec<Finding>,
    /// Everything that stops the check being complete.
    pub caveats: Vec<Caveat>,
}

impl ConditionOutcome {
    /// The verdict.
    ///
    /// A violation outranks a caveat: a counter-example that was found is found whether or not
    /// the search was exhaustive. The caveats stay on the outcome, because in that case they mean
    /// "and there may be more", which a report should still say.
    pub fn verdict(&self) -> Verdict {
        if !self.violations.is_empty() {
            Verdict::Violated
        } else if self.caveats.is_empty() {
            Verdict::Held
        } else {
            Verdict::Unchecked
        }
    }
}

/// The integrity conditions a change would break: violated after it, and not before.
///
/// This is how a **proposed** change is checked, and it is deliberately the whole set rather than
/// a hand-rolled subset of the conditions the change is thought likely to break. A bulk operation
/// that rewrites statements can break a condition nobody predicted — merging two concepts is the
/// worked case: it obviously risks S14, because both concepts have a preferred label, and it also
/// breaks S27 when one of them is associatively linked to something above the other, which is not
/// obvious at all and which a check written from the author's expectations would have missed.
///
/// **Only newly broken conditions count.** A vocabulary that already violates a condition must not
/// have every subsequent edit refused for a fault the edit did not introduce; the operator would
/// then be unable to use the tool to fix it. `Unchecked` before and `Violated` after counts as
/// newly broken: a bound that was hit is not evidence the condition held.
pub fn newly_violated(before: &CoreModel, after: &CoreModel) -> Vec<ConditionOutcome> {
    let was: BTreeSet<SkosRule> = before
        .integrity()
        .iter()
        .filter(|outcome| outcome.verdict() == Verdict::Violated)
        .map(|outcome| outcome.condition.rule())
        .collect();

    after
        .integrity()
        .into_iter()
        .filter(|outcome| {
            outcome.verdict() == Verdict::Violated && !was.contains(&outcome.condition.rule())
        })
        .collect()
}

/// A declared refinement whose consequences this build does not draw.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnreadRefinement {
    /// Whether it was declared with `rdfs:subPropertyOf` or `rdfs:subClassOf`.
    pub declaration: Declaration,
    /// The chain from the vocabulary's own term to the SKOS term it reaches, inclusive. Two
    /// entries for a direct declaration, more when it was reached through the graph's own terms.
    pub chain: Vec<String>,
}

impl UnreadRefinement {
    /// The vocabulary's own term.
    pub fn declared(&self) -> &str {
        self.chain.first().map(String::as_str).unwrap_or_default()
    }

    /// The SKOS or SKOS-XL term it reaches.
    pub fn reaches(&self) -> &str {
        self.chain.last().map(String::as_str).unwrap_or_default()
    }
}

impl fmt::Display for UnreadRefinement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The two ends, and the middle only when there is one. Printing the whole chain beside
        // the declared term repeats it — which is what the first draft did, and what running the
        // command against a store on disk showed as "declares <ex:seeAlso> a sub-property of
        // <ex:seeAlso> → skos:related".
        write!(
            f,
            "{} is declared {} {}",
            short(self.declared()),
            self.declaration,
            short(self.reaches())
        )?;
        if self.chain.len() > 2 {
            let through: Vec<String> = self.chain[1..self.chain.len() - 1]
                .iter()
                .map(|term| short(term))
                .collect();
            write!(f, ", through {}", through.join(" → "))?;
        }
        Ok(())
    }
}

/// Which RDFS declaration a refinement was written with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Declaration {
    /// `rdfs:subPropertyOf`.
    SubPropertyOf,
    /// `rdfs:subClassOf`.
    SubClassOf,
}

impl fmt::Display for Declaration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Declaration::SubPropertyOf => write!(f, "a sub-property of"),
            Declaration::SubClassOf => write!(f, "a sub-class of"),
        }
    }
}

/// How far the refinement scan may read before it stops and says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefinementScanBound {
    /// How many distinct declared terms to keep.
    pub max_terms: usize,
    /// How many edges to follow, **across the whole scan** and not per term.
    ///
    /// Per-term times one walk per term is not a bound — `docs/adr/0027` and iteration 33's
    /// budget test are the two places that lesson was learned the hard way.
    pub max_steps: usize,
}

impl RefinementScanBound {
    /// Generous enough that no real vocabulary's schema reaches it, small enough that a graph
    /// built to exhaust us stops.
    pub const DEFAULT: Self = Self {
        max_terms: 10_000,
        max_steps: 50_000,
    };
}

impl Default for RefinementScanBound {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The `rdfs:subPropertyOf` and `rdfs:subClassOf` declarations a graph makes, collected as it is
/// read and resolved once at the end.
///
/// Held on the model rather than resolved during the read for the reason `refinement` gives: a
/// declaration may arrive after every statement that uses it, and RDF has no document order.
#[derive(Debug, Clone, Default)]
pub struct DeclaredRefinements {
    sub_property: BTreeMap<String, BTreeSet<String>>,
    sub_class: BTreeMap<String, BTreeSet<String>>,
    declarations: usize,
    overflowed: usize,
    bound: RefinementScanBound,
}

impl DeclaredRefinements {
    /// Use a different bound. The default is [`RefinementScanBound::DEFAULT`].
    pub fn with_bound(bound: RefinementScanBound) -> Self {
        Self {
            bound,
            ..Self::default()
        }
    }

    /// Offer one statement. Anything that is not an RDFS declaration between two IRIs is dropped.
    ///
    /// A declaration whose *subject* is a SKOS or SKOS-XL term is dropped too, and that is the
    /// one exclusion with teeth: a vocabulary that imports the SKOS ontology carries S22's
    /// `skos:broader rdfs:subPropertyOf skos:broaderTransitive` as an ordinary statement, and
    /// treating that as an unread refinement would make every importing vocabulary's whole
    /// roll-call unchecked. Those statements are applied — from the specification, which is where
    /// the citation belongs — so nothing is lost by ignoring the graph's copy.
    pub fn push(&mut self, subject: &Node, predicate: &str, object: &Term) {
        let declaration = match predicate {
            RDFS_SUB_PROPERTY_OF => Declaration::SubPropertyOf,
            RDFS_SUB_CLASS_OF => Declaration::SubClassOf,
            _ => return,
        };
        let (Some(sub), Some(sup)) = (subject.as_iri(), object.as_node().and_then(Node::as_iri))
        else {
            return;
        };
        if is_skos_term(sub) {
            return;
        }

        self.declarations += 1;
        let edges = match declaration {
            Declaration::SubPropertyOf => &mut self.sub_property,
            Declaration::SubClassOf => &mut self.sub_class,
        };
        if !edges.contains_key(sub) && edges.len() >= self.bound.max_terms {
            self.overflowed += 1;
            return;
        }
        edges
            .entry(sub.to_owned())
            .or_default()
            .insert(sup.to_owned());
    }

    /// Resolve every declared term up to the SKOS and SKOS-XL terms it reaches.
    ///
    /// One walk answers both halves — what was found, and whether the scan got to the end — so
    /// the two can never disagree about the same graph.
    pub fn resolve(&self) -> RefinementScan {
        let mut steps = 0usize;
        let mut unread = Vec::new();
        for (declaration, edges) in [
            (Declaration::SubPropertyOf, &self.sub_property),
            (Declaration::SubClassOf, &self.sub_class),
        ] {
            for start in edges.keys() {
                for chain in walk(edges, start, &mut steps, self.bound.max_steps) {
                    unread.push(UnreadRefinement { declaration, chain });
                }
            }
        }
        unread.sort();
        unread.dedup();

        let exhausted = steps >= self.bound.max_steps;
        let exhaustion = (self.overflowed > 0 || exhausted).then_some((
            self.declarations - self.overflowed,
            self.overflowed + usize::from(exhausted),
        ));

        RefinementScan { unread, exhaustion }
    }

    /// How many declarations were read, whatever became of them.
    pub fn declarations(&self) -> usize {
        self.declarations
    }
}

/// What the scan of a graph's RDFS declarations found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefinementScan {
    /// Every declared refinement that reaches a SKOS or SKOS-XL term, in a stable order.
    pub unread: Vec<UnreadRefinement>,
    /// `Some((read, unread))` when the term ceiling refused a declaration or the step budget ran
    /// out. That makes **every** condition unchecked, because a declaration we never read could
    /// have reached any term at all.
    pub exhaustion: Option<(usize, usize)>,
}

/// Whether an IRI is in the SKOS or SKOS-XL namespace.
fn is_skos_term(iri: &str) -> bool {
    iri.starts_with(ns::SKOS) || iri.starts_with(ns::SKOSXL)
}

/// Breadth-first upward from one declared term, returning the shortest chain to each SKOS term
/// reachable from it.
///
/// Breadth-first for the reason `refinement::walk` is: an author checking the explanation against
/// their own file should be handed the shortest path that reaches the conclusion. The visited set
/// is the cycle guard, so `ex:a rdfs:subPropertyOf ex:b rdfs:subPropertyOf ex:a` terminates.
///
/// The walk does not continue *through* a SKOS term: once `ex:seeAlso` reaches `skos:related`,
/// what lies above `skos:related` is the specification's business and S21 already applies it.
fn walk(
    edges: &BTreeMap<String, BTreeSet<String>>,
    start: &str,
    steps: &mut usize,
    max_steps: usize,
) -> Vec<Vec<String>> {
    let mut found = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut came_from: BTreeMap<&str, &str> = BTreeMap::new();
    let mut queue: VecDeque<&str> = VecDeque::new();

    seen.insert(start);
    queue.push_back(start);

    while let Some(current) = queue.pop_front() {
        let Some(supers) = edges.get(current) else {
            continue;
        };
        for sup in supers {
            if *steps >= max_steps {
                return found;
            }
            *steps += 1;
            if !seen.insert(sup.as_str()) {
                continue;
            }
            came_from.insert(sup.as_str(), current);
            if is_skos_term(sup) {
                found.push(chain(start, sup.as_str(), &came_from));
            } else {
                queue.push_back(sup.as_str());
            }
        }
    }

    found
}

/// Reconstruct the path from `start` to `end`, `start` first.
fn chain(start: &str, end: &str, came_from: &BTreeMap<&str, &str>) -> Vec<String> {
    let mut path = vec![end.to_owned()];
    let mut cursor = end;
    while cursor != start {
        let Some(previous) = came_from.get(cursor) else {
            // Unreachable: every node in `came_from` was reached from `start`. Returning the
            // partial path rather than panicking, for `CLAUDE.md` §6's reason — a broken
            // explanation beats an aborted report on a customer's vocabulary.
            break;
        };
        cursor = previous;
        path.push(cursor.to_owned());
    }
    path.reverse();
    path
}

/// A term as a CURIE where we know the prefix, angle-bracketed otherwise.
fn short(iri: &str) -> String {
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

/// The condition a finding violates, if it violates one.
///
/// Every [`Severity::Inconsistent`](crate::Severity::Inconsistent) finding must answer here, and
/// a test asserts it: a finding that forgets to would go unattributed, and the roll-call would
/// report a graph as consistent that this build calls inconsistent.
pub(crate) fn violated_by(finding: &Finding) -> Option<SkosRule> {
    match finding {
        Finding::DisjointClasses { rule, .. } => Some(*rule),
        Finding::LiteralOnObjectProperty { rule, .. }
        | Finding::NodeOnDatatypeProperty { rule, .. } => Some(*rule),
        Finding::MultiplePreferredLabels { .. } => Some(SkosRule::S14),
        Finding::LabelPropertiesClash { .. } => Some(SkosRule::S13),
        Finding::MultipleLiteralForms { .. } => Some(SkosRule::S52),
        Finding::XlLabelPropertiesClash { .. } => Some(SkosRule::S58),
        Finding::RelatedAndBroaderTransitive { .. } => Some(SkosRule::S27),
        Finding::ExactMatchClash { .. } | Finding::ExactMatchChainClash { .. } => {
            Some(SkosRule::S46)
        }
        Finding::DefectiveMemberList { .. }
        | Finding::MultipleMemberLists { .. }
        | Finding::NonPlainLiteralLabel { .. }
        | Finding::NoLiteralForm { .. }
        | Finding::NonPlainLiteralForm { .. }
        | Finding::ExactMatchClusterBoundReached { .. }
        | Finding::ExactMatchSweepExhausted { .. }
        | Finding::AncestryBoundReached { .. }
        | Finding::DisjointnessSweepExhausted { .. }
        | Finding::RefinementBoundReached { .. } => None,
    }
}

/// The conditions a bounded walk leaves without a verdict.
///
/// `RefinementBoundReached` answers with nothing, and that is the interesting entry: §7 states no
/// integrity condition, the refinement pass resolves note properties and nothing else, so a
/// resolution that gave up cannot hide a violation of any condition in the table. It makes the
/// documentation counts a floor, which is a different claim and one `openbiz inspect` already
/// makes.
pub(crate) fn left_unchecked_by(finding: &Finding) -> &'static [SkosRule] {
    match finding {
        Finding::AncestryBoundReached { .. } | Finding::DisjointnessSweepExhausted { .. } => {
            &[SkosRule::S27]
        }
        Finding::ExactMatchClusterBoundReached { .. }
        | Finding::ExactMatchSweepExhausted { .. } => &[SkosRule::S46],
        // Exhaustive rather than a wildcard, and deliberately: a new bounded check added later
        // must decide here which condition it leaves unanswered, because the alternative is a
        // walk that silently gives up and a report that still says the condition held.
        Finding::RefinementBoundReached { .. }
        | Finding::DisjointClasses { .. }
        | Finding::LiteralOnObjectProperty { .. }
        | Finding::NodeOnDatatypeProperty { .. }
        | Finding::MultiplePreferredLabels { .. }
        | Finding::LabelPropertiesClash { .. }
        | Finding::MultipleLiteralForms { .. }
        | Finding::XlLabelPropertiesClash { .. }
        | Finding::RelatedAndBroaderTransitive { .. }
        | Finding::ExactMatchClash { .. }
        | Finding::ExactMatchChainClash { .. }
        | Finding::DefectiveMemberList { .. }
        | Finding::MultipleMemberLists { .. }
        | Finding::NonPlainLiteralLabel { .. }
        | Finding::NoLiteralForm { .. }
        | Finding::NonPlainLiteralForm { .. } => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::{LabelKind, LexicalLabel};
    use crate::mapping::{ExactMatchDisjointness, MappingProperty};
    use crate::model::{ClassOrigin, ListDefect, Literal, Severity, SkosClass};
    use crate::relations::RelationOrigin;

    /// One of every [`Finding`] the model can raise.
    ///
    /// Hand-written rather than harvested from a fixture, because the point is to be exhaustive
    /// over the *enum* rather than over any one graph. It stays in step with the enum because
    /// [`violated_by`] and [`left_unchecked_by`] match every variant by name: a variant added
    /// later fails to compile there, and the author who fixes that arrives here.
    fn one_of_every_finding() -> Vec<Finding> {
        let node = Node::iri(iri("a"));
        let other = Node::iri(iri("b"));
        let literal = Literal {
            value: "x".to_owned(),
            language: None,
            datatype: crate::labels::XSD_STRING.to_owned(),
        };
        vec![
            Finding::DisjointClasses {
                resource: node.clone(),
                first: (SkosClass::Concept, ClassOrigin::Asserted),
                second: (SkosClass::ConceptScheme, ClassOrigin::Asserted),
                rule: SkosRule::S9,
            },
            Finding::DisjointClasses {
                resource: node.clone(),
                first: (SkosClass::Collection, ClassOrigin::Asserted),
                second: (SkosClass::Concept, ClassOrigin::Asserted),
                rule: SkosRule::S37,
            },
            Finding::DisjointClasses {
                resource: node.clone(),
                first: (SkosClass::Label, ClassOrigin::Asserted),
                second: (SkosClass::Concept, ClassOrigin::Asserted),
                rule: SkosRule::S48,
            },
            Finding::DefectiveMemberList {
                collection: node.clone(),
                head: other.clone(),
                defect: ListDefect::NoFirst {
                    cell: other.clone(),
                },
                read: 0,
            },
            Finding::MultipleMemberLists {
                resource: node.clone(),
                lists: 2,
            },
            Finding::MultiplePreferredLabels {
                resource: node.clone(),
                language: Some("en".to_owned()),
                labels: vec!["one".to_owned(), "two".to_owned()],
            },
            Finding::LabelPropertiesClash {
                resource: node.clone(),
                label: LexicalLabel {
                    language: Some("en".to_owned()),
                    text: "one".to_owned(),
                },
                kinds: vec![LabelKind::Preferred, LabelKind::Alternative],
            },
            Finding::NonPlainLiteralLabel {
                resource: node.clone(),
                property: "skos:prefLabel".to_owned(),
                value: Term::Literal(literal.clone()),
            },
            Finding::LiteralOnObjectProperty {
                subject: node.clone(),
                property: "skos:inScheme".to_owned(),
                literal: literal.clone(),
                rule: SkosRule::S3,
            },
            Finding::LiteralOnObjectProperty {
                subject: node.clone(),
                property: "skos:broader".to_owned(),
                literal: literal.clone(),
                rule: SkosRule::S18,
            },
            Finding::LiteralOnObjectProperty {
                subject: node.clone(),
                property: "skos:member".to_owned(),
                literal: literal.clone(),
                rule: SkosRule::S30,
            },
            Finding::LiteralOnObjectProperty {
                subject: node.clone(),
                property: "skos:exactMatch".to_owned(),
                literal: literal.clone(),
                rule: SkosRule::S38,
            },
            Finding::LiteralOnObjectProperty {
                subject: node.clone(),
                property: "skosxl:prefLabel".to_owned(),
                literal: literal.clone(),
                rule: SkosRule::S53,
            },
            Finding::LiteralOnObjectProperty {
                subject: node.clone(),
                property: "skosxl:labelRelation".to_owned(),
                literal: literal.clone(),
                rule: SkosRule::S59,
            },
            Finding::NodeOnDatatypeProperty {
                subject: node.clone(),
                property: "skosxl:literalForm".to_owned(),
                node: other.clone(),
                rule: SkosRule::S49,
            },
            Finding::MultipleLiteralForms {
                label: node.clone(),
                forms: vec!["one".to_owned(), "two".to_owned()],
            },
            Finding::NoLiteralForm {
                label: node.clone(),
            },
            Finding::NonPlainLiteralForm {
                label: node.clone(),
                value: literal,
            },
            Finding::XlLabelPropertiesClash {
                resource: node.clone(),
                label: other.clone(),
                kinds: vec![LabelKind::Preferred, LabelKind::Hidden],
            },
            Finding::RelatedAndBroaderTransitive {
                concept: node.clone(),
                related: other.clone(),
                path: vec![node.clone(), other.clone()],
            },
            Finding::ExactMatchClash {
                concept: node.clone(),
                other: other.clone(),
                exact: RelationOrigin::Asserted,
                clashes: vec![(
                    MappingProperty::BroadMatch,
                    RelationOrigin::Asserted,
                    ExactMatchDisjointness::Stated,
                )],
            },
            Finding::ExactMatchChainClash {
                concept: node.clone(),
                other: other.clone(),
                chain: vec![node.clone(), Node::iri(iri("c")), other.clone()],
                clashes: vec![(
                    MappingProperty::RelatedMatch,
                    RelationOrigin::Entailed(SkosRule::S44),
                    ExactMatchDisjointness::Stated,
                )],
            },
            Finding::ExactMatchClusterBoundReached {
                concept: node.clone(),
                reached: 1,
                links_walked: 2,
            },
            Finding::ExactMatchSweepExhausted {
                checked: 1,
                unchecked: 2,
                links_walked: 3,
            },
            Finding::AncestryBoundReached {
                concept: node,
                reached: 1,
                links_walked: 2,
            },
            Finding::DisjointnessSweepExhausted {
                checked: 1,
                unchecked: 2,
                links_walked: 3,
            },
            Finding::RefinementBoundReached {
                resolved: 1,
                unresolved: 2,
                steps_walked: 3,
            },
        ]
    }

    fn iri(local: &str) -> String {
        format!("https://example.org/{local}")
    }

    fn declares(sub: &str, predicate: &str, sup: &str) -> (Node, String, Term) {
        (
            Node::iri(sub),
            predicate.to_owned(),
            Term::Node(Node::iri(sup)),
        )
    }

    fn scan(edges: &[(Node, String, Term)]) -> DeclaredRefinements {
        let mut declared = DeclaredRefinements::default();
        for (subject, predicate, object) in edges {
            declared.push(subject, predicate, object);
        }
        declared
    }

    /// The specification states six integrity conditions and this table says so.
    #[test]
    fn the_specification_states_six_integrity_conditions() {
        let specified: Vec<SkosRule> = CONDITIONS
            .iter()
            .filter(|condition| condition.authority() == Authority::Specification)
            .map(IntegrityCondition::rule)
            .collect();
        assert_eq!(
            specified,
            vec![
                SkosRule::S9,
                SkosRule::S13,
                SkosRule::S14,
                SkosRule::S27,
                SkosRule::S37,
                SkosRule::S46
            ]
        );
    }

    /// And each of the six cites a section headed "Integrity Conditions" — §4.4, §5.4, §8.4,
    /// §9.4, §10.4. Nothing we classify ourselves may cite one, because those headings are the
    /// whole of the specification's authority for calling a violation an inconsistency.
    #[test]
    fn only_the_specifications_own_conditions_cite_an_integrity_conditions_heading() {
        for condition in CONDITIONS {
            let heading = matches!(
                condition.section(),
                "§4.4" | "§5.4" | "§8.4" | "§9.4" | "§10.4"
            );
            assert_eq!(
                heading,
                condition.authority() == Authority::Specification,
                "{} cites {} and is {:?}",
                condition.rule(),
                condition.section(),
                condition.authority()
            );
        }
    }

    /// No condition appears twice, so a violation is attributed once.
    #[test]
    fn every_condition_appears_once() {
        let mut seen = BTreeSet::new();
        for condition in CONDITIONS {
            assert!(
                seen.insert(condition.rule()),
                "{} is in the table twice",
                condition.rule()
            );
        }
    }

    /// Every term a condition is checked over is a real SKOS or SKOS-XL IRI.
    ///
    /// A typo here would silently stop a caveat firing, which is a false "held" — the exact
    /// failure the caveat exists to prevent, reintroduced by a misspelling.
    #[test]
    fn every_term_named_is_in_the_skos_or_skos_xl_namespace() {
        for condition in CONDITIONS {
            assert!(!condition.terms().is_empty(), "{}", condition.rule());
            for term in condition.terms() {
                assert!(is_skos_term(term), "{} names {term}", condition.rule());
            }
        }
    }

    /// The class IRIs written out here are the ones `SkosClass` uses.
    #[test]
    fn the_class_iris_match_the_model() {
        use crate::model::SkosClass;
        assert_eq!(SkosClass::Concept.iri(), SKOS_CONCEPT);
        assert_eq!(SkosClass::ConceptScheme.iri(), SKOS_CONCEPT_SCHEME);
        assert_eq!(SkosClass::Collection.iri(), SKOS_COLLECTION);
        assert_eq!(SkosClass::OrderedCollection.iri(), SKOS_ORDERED_COLLECTION);
    }

    /// A one-step declaration is found, and the chain names both ends.
    #[test]
    fn a_declared_refinement_of_a_skos_property_is_found() {
        let found = scan(&[declares(
            &iri("seeAlso"),
            RDFS_SUB_PROPERTY_OF,
            SKOS_RELATED,
        )])
        .resolve()
        .unread;

        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].declared(), iri("seeAlso"));
        assert_eq!(found[0].reaches(), SKOS_RELATED);
        assert_eq!(found[0].declaration, Declaration::SubPropertyOf);
    }

    /// And a two-step one is, which is the whole reason this walks rather than matching objects.
    ///
    /// A scan that only looked at the object of each declaration would report nothing here and
    /// S27 would read "held" on a vocabulary whose associative links this build never saw.
    #[test]
    fn a_refinement_reached_through_the_vocabularys_own_terms_is_found() {
        let found = scan(&[
            declares(&iri("seeAlso"), RDFS_SUB_PROPERTY_OF, &iri("linkedTo")),
            declares(&iri("linkedTo"), RDFS_SUB_PROPERTY_OF, SKOS_RELATED),
        ])
        .resolve()
        .unread;

        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(
            found[0].chain,
            vec![iri("linkedTo"), SKOS_RELATED.to_owned()]
        );
        assert_eq!(
            found[1].chain,
            vec![iri("seeAlso"), iri("linkedTo"), SKOS_RELATED.to_owned()]
        );
    }

    /// A vocabulary that imports the SKOS ontology is not thereby unchecked.
    ///
    /// The import carries S22 and S42 as ordinary statements. Both are applied from the
    /// specification, so reading the graph's copy as an unread refinement would make every
    /// importing vocabulary's entire roll-call unchecked — a report so cautious it says nothing.
    #[test]
    fn the_skos_ontologys_own_statements_are_not_unread_refinements() {
        let found = scan(&[
            declares(SKOS_BROADER, RDFS_SUB_PROPERTY_OF, SKOS_BROADER_TRANSITIVE),
            declares(SKOS_EXACT_MATCH, RDFS_SUB_PROPERTY_OF, SKOS_CLOSE_MATCH),
            declares(SKOSXL_PREF_LABEL, RDFS_SUB_PROPERTY_OF, SKOS_PREF_LABEL),
        ])
        .resolve()
        .unread;

        assert!(found.is_empty(), "{found:?}");
    }

    /// A refinement of something outside SKOS reaches nothing and raises no caveat.
    #[test]
    fn a_refinement_of_a_non_skos_term_is_not_a_caveat() {
        let found = scan(&[declares(
            &iri("created"),
            RDFS_SUB_PROPERTY_OF,
            "http://purl.org/dc/terms/date",
        )])
        .resolve()
        .unread;

        assert!(found.is_empty(), "{found:?}");
    }

    /// A cycle terminates and entails nothing.
    #[test]
    fn a_cyclic_declaration_terminates() {
        let found = scan(&[
            declares(&iri("a"), RDFS_SUB_PROPERTY_OF, &iri("b")),
            declares(&iri("b"), RDFS_SUB_PROPERTY_OF, &iri("a")),
        ])
        .resolve()
        .unread;

        assert!(found.is_empty(), "{found:?}");
    }

    /// `rdfs:subClassOf` is read too, and says which declaration it was.
    ///
    /// It is the commoner of the two in enterprise data — `ex:ProductCategory rdfs:subClassOf
    /// skos:Concept` is how a house ontology joins SKOS — and this build reads no `rdfs:subClassOf`
    /// at all, so S9 and S37 are checked over the types the graph states outright.
    #[test]
    fn a_sub_class_declaration_is_found_and_named_as_one() {
        let found = scan(&[declares(
            &iri("ProductCategory"),
            RDFS_SUB_CLASS_OF,
            SKOS_CONCEPT,
        )])
        .resolve()
        .unread;

        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].declaration, Declaration::SubClassOf);
        assert!(
            found[0].to_string().contains("a sub-class of"),
            "{}",
            found[0]
        );
    }

    /// The chain is printed as two ends and a middle, not as a term repeated beside its own path.
    ///
    /// Found by running `openbiz integrity` against a store on disk with everything green: the
    /// line read "declares <ex:seeAlso> a sub-property of <ex:seeAlso> → skos:related".
    #[test]
    fn a_refinement_names_its_two_ends_and_only_a_longer_chains_middle() {
        let direct = scan(&[declares(
            &iri("seeAlso"),
            RDFS_SUB_PROPERTY_OF,
            SKOS_RELATED,
        )])
        .resolve()
        .unread;
        assert_eq!(
            direct[0].to_string(),
            format!(
                "<{}> is declared a sub-property of skos:related",
                iri("seeAlso")
            )
        );

        let indirect = scan(&[
            declares(&iri("seeAlso"), RDFS_SUB_PROPERTY_OF, &iri("linkedTo")),
            declares(&iri("linkedTo"), RDFS_SUB_PROPERTY_OF, SKOS_RELATED),
        ])
        .resolve()
        .unread;
        let long = indirect
            .iter()
            .find(|refinement| refinement.declared() == iri("seeAlso"))
            .expect("the two-step declaration");
        assert_eq!(
            long.to_string(),
            format!(
                "<{}> is declared a sub-property of skos:related, through <{}>",
                iri("seeAlso"),
                iri("linkedTo")
            )
        );
    }

    /// A declaration whose object is a literal or a blank node is dropped rather than recorded.
    #[test]
    fn a_declaration_that_is_not_between_two_iris_is_dropped() {
        let mut declared = DeclaredRefinements::default();
        declared.push(
            &Node::iri(iri("seeAlso")),
            RDFS_SUB_PROPERTY_OF,
            &Term::Node(Node::blank("b0")),
        );
        declared.push(
            &Node::blank("b1"),
            RDFS_SUB_PROPERTY_OF,
            &Term::Node(Node::iri(SKOS_RELATED)),
        );
        assert_eq!(declared.declarations(), 0);
        assert!(declared.resolve().unread.is_empty());
    }

    /// The step budget is spent across the whole scan, not per term.
    ///
    /// The shape is iteration 33's: several small clusters and a budget that covers some of them,
    /// because one long chain with a budget that one walk exhausts cannot tell a per-walk bound
    /// from a shared one.
    #[test]
    fn the_scan_budget_is_shared_across_every_declared_term() {
        let declarations: Vec<(Node, String, Term)> = (0..5)
            .map(|n| declares(&iri(&format!("p{n}")), RDFS_SUB_PROPERTY_OF, SKOS_RELATED))
            .collect();

        let mut declared = DeclaredRefinements::with_bound(RefinementScanBound {
            max_terms: 100,
            max_steps: 2,
        });
        for (subject, predicate, object) in &declarations {
            declared.push(subject, predicate, object);
        }

        // Two of the five walks fit in the budget. A per-walk budget of 2 would resolve all five.
        let scan = declared.resolve();
        assert_eq!(scan.unread.len(), 2);
        assert_eq!(scan.exhaustion.map(|(_, unread)| unread), Some(1));
    }

    /// The term ceiling refuses a declaration rather than growing without limit, and says so.
    #[test]
    fn the_term_ceiling_reports_what_it_refused() {
        let declarations: Vec<(Node, String, Term)> = (0..5)
            .map(|n| declares(&iri(&format!("p{n}")), RDFS_SUB_PROPERTY_OF, SKOS_RELATED))
            .collect();

        let mut declared = DeclaredRefinements::with_bound(RefinementScanBound {
            max_terms: 2,
            max_steps: 1_000,
        });
        for (subject, predicate, object) in &declarations {
            declared.push(subject, predicate, object);
        }

        let scan = declared.resolve();
        assert_eq!(scan.unread.len(), 2);
        assert_eq!(scan.exhaustion, Some((2, 3)));
    }

    /// A scan that read everything reports no exhaustion at all.
    #[test]
    fn a_complete_scan_reports_no_exhaustion() {
        let declared = scan(&[declares(
            &iri("seeAlso"),
            RDFS_SUB_PROPERTY_OF,
            SKOS_RELATED,
        )]);
        assert_eq!(declared.resolve().exhaustion, None);
    }

    /// Every finding this build calls inconsistent is attributed to a row in the table.
    ///
    /// This is the property that makes the roll-call trustworthy rather than decorative: a new
    /// `Severity::Inconsistent` finding that forgets to register would be unreportable, and the
    /// report would say every condition held on a graph the model calls inconsistent.
    #[test]
    fn every_inconsistent_finding_is_attributed_to_a_condition() {
        let known: BTreeSet<SkosRule> = CONDITIONS.iter().map(IntegrityCondition::rule).collect();

        for finding in one_of_every_finding() {
            match finding.severity() {
                Severity::Inconsistent => {
                    let rule = violated_by(&finding)
                        .unwrap_or_else(|| panic!("{finding:?} is inconsistent and unattributed"));
                    assert!(known.contains(&rule), "{rule} is not in CONDITIONS");
                }
                Severity::IllFormed => assert!(
                    violated_by(&finding).is_none(),
                    "{finding:?} is ill-formed and must violate no integrity condition"
                ),
                Severity::Unchecked => {
                    assert!(violated_by(&finding).is_none(), "{finding:?}");
                    for rule in left_unchecked_by(&finding) {
                        assert!(known.contains(rule), "{rule} is not in CONDITIONS");
                    }
                }
            }
        }
    }

    /// A resolution that gave up leaves no integrity condition unchecked, and that is a claim
    /// rather than an oversight.
    ///
    /// The refinement pass resolves the seven documentation properties and nothing else, and §7
    /// states no integrity condition, so a resolution that stopped cannot hide a violation of
    /// anything in the table. It makes the documentation counts a floor, which `openbiz inspect`
    /// already says.
    #[test]
    fn an_exhausted_note_refinement_leaves_no_condition_unchecked() {
        assert!(left_unchecked_by(&Finding::RefinementBoundReached {
            resolved: 1,
            unresolved: 2,
            steps_walked: 3,
        })
        .is_empty());
    }

    // --- The roll-call over a real model -----------------------------------------------------

    use crate::equivalence::EquivalenceBound;
    use crate::hierarchy::WalkBound;
    use crate::model::{CoreModel, Statement};

    fn node(local: &str) -> Node {
        Node::iri(iri(local))
    }

    fn stated(subject: &Node, predicate: &str, object: &Node) -> Statement {
        Statement::new(subject.clone(), predicate.to_owned(), object.clone())
    }

    fn is_a(subject: &Node, class: crate::model::SkosClass) -> Statement {
        Statement::new(
            subject.clone(),
            crate::model::RDF_TYPE,
            Node::iri(class.iri()),
        )
    }

    fn labelled(subject: &Node, predicate: &str, text: &str, language: &str) -> Statement {
        Statement::new(
            subject.clone(),
            predicate.to_owned(),
            Term::Literal(Literal {
                value: text.to_owned(),
                language: Some(language.to_owned()),
                datatype: crate::labels::RDF_LANG_STRING.to_owned(),
            }),
        )
    }

    fn declaring(sub: &str, predicate: &str, sup: &str) -> Statement {
        Statement::new(Node::iri(sub), predicate.to_owned(), Node::iri(sup))
    }

    /// The verdict on one condition, by S-number.
    fn verdict(model: &CoreModel, rule: SkosRule) -> Verdict {
        outcome(model, rule).verdict()
    }

    fn outcome(model: &CoreModel, rule: SkosRule) -> ConditionOutcome {
        model
            .integrity()
            .into_iter()
            .find(|outcome| outcome.condition.rule() == rule)
            .unwrap_or_else(|| panic!("{rule} is not in the roll-call"))
    }

    /// Every condition this build checks appears in the roll-call of every vocabulary, whatever
    /// is in it — a condition that only appears when it is violated is a condition an operator
    /// cannot tell was checked.
    #[test]
    fn every_condition_is_reported_for_every_vocabulary() {
        let empty = CoreModel::from_statements([]);
        assert_eq!(empty.integrity().len(), CONDITIONS.len());
        assert!(empty
            .integrity()
            .iter()
            .all(|outcome| outcome.verdict() == Verdict::Held));

        let ordinary = CoreModel::from_statements([
            is_a(&node("a"), crate::model::SkosClass::Concept),
            stated(&node("a"), SKOS_BROADER, &node("b")),
        ]);
        assert_eq!(ordinary.integrity().len(), CONDITIONS.len());
    }

    /// S9 — §4.4. A resource that is both a concept and a concept scheme.
    #[test]
    fn s9_is_reported_violated_and_the_other_conditions_still_hold() {
        let model = CoreModel::from_statements([
            is_a(&node("both"), crate::model::SkosClass::Concept),
            is_a(&node("both"), crate::model::SkosClass::ConceptScheme),
        ]);

        assert_eq!(verdict(&model, SkosRule::S9), Verdict::Violated);
        assert_eq!(outcome(&model, SkosRule::S9).violations.len(), 1);
        // Every other row still reports, and reports held. A roll-call that went silent on the
        // rest as soon as one failed would leave an operator unable to say what else was checked.
        for other in model.integrity() {
            if other.condition.rule() != SkosRule::S9 {
                assert_eq!(other.verdict(), Verdict::Held, "{:?}", other);
            }
        }
    }

    /// S13 — §5.4. The same label under two of the three labelling properties.
    #[test]
    fn s13_is_reported_violated() {
        let model = CoreModel::from_statements([
            labelled(&node("a"), SKOS_PREF_LABEL, "love", "en"),
            labelled(&node("a"), SKOS_ALT_LABEL, "love", "en"),
        ]);
        assert_eq!(verdict(&model, SkosRule::S13), Verdict::Violated);
        assert_eq!(verdict(&model, SkosRule::S14), Verdict::Held);
    }

    /// S14 — §5.4. Two preferred labels in one language.
    #[test]
    fn s14_is_reported_violated() {
        let model = CoreModel::from_statements([
            labelled(&node("a"), SKOS_PREF_LABEL, "colour", "en"),
            labelled(&node("a"), SKOS_PREF_LABEL, "color", "en"),
        ]);
        assert_eq!(verdict(&model, SkosRule::S14), Verdict::Violated);
        assert_eq!(verdict(&model, SkosRule::S13), Verdict::Held);
    }

    /// S27 — §8.4. Example 26: an associative link between two concepts that are also
    /// hierarchically related.
    #[test]
    fn s27_is_reported_violated() {
        let model = CoreModel::from_statements([
            stated(&node("a"), SKOS_BROADER, &node("b")),
            stated(&node("a"), SKOS_RELATED, &node("b")),
        ]);
        assert_eq!(verdict(&model, SkosRule::S27), Verdict::Violated);
    }

    /// S37 — §9.4. A collection that is also a concept.
    #[test]
    fn s37_is_reported_violated() {
        let model = CoreModel::from_statements([
            is_a(&node("thing"), crate::model::SkosClass::Collection),
            is_a(&node("thing"), crate::model::SkosClass::Concept),
        ]);
        assert_eq!(verdict(&model, SkosRule::S37), Verdict::Violated);
        assert_eq!(verdict(&model, SkosRule::S9), Verdict::Held);
    }

    /// S46 — §10.4. Example 52: an exact match that is also a broad match.
    #[test]
    fn s46_is_reported_violated() {
        let model = CoreModel::from_statements([
            stated(&node("a"), SKOS_EXACT_MATCH, &node("b")),
            stated(&node("a"), SKOS_BROAD_MATCH, &node("b")),
        ]);
        assert_eq!(verdict(&model, SkosRule::S46), Verdict::Violated);
    }

    /// A bounded ancestry walk leaves **S27** without a verdict and says nothing about the rest.
    ///
    /// This is the attribution the roll-call exists for. `CoreModel::checks_are_complete` answers
    /// `false` for the whole model here, which is true and unhelpful: it reads as though every
    /// condition were in doubt, when what the bound cost is §8.4's check and only that.
    #[test]
    fn a_bounded_ancestry_walk_leaves_s27_unchecked_and_nothing_else() {
        let mut builder = CoreModel::builder().with_ancestry_bound(WalkBound {
            max_nodes: 1,
            max_links: 1,
        });
        for statement in [
            stated(&node("a"), SKOS_BROADER, &node("b")),
            stated(&node("b"), SKOS_BROADER, &node("c")),
            stated(&node("a"), SKOS_RELATED, &node("z")),
        ] {
            builder.push(statement);
        }
        let model = builder.build();

        assert!(!model.checks_are_complete());
        assert_eq!(verdict(&model, SkosRule::S27), Verdict::Unchecked);
        for other in model.integrity() {
            if other.condition.rule() != SkosRule::S27 {
                assert_eq!(other.verdict(), Verdict::Held, "{:?}", other);
            }
        }
    }

    /// And a bounded exact-match walk leaves **S46** and nothing else.
    #[test]
    fn a_bounded_exact_match_walk_leaves_s46_unchecked_and_nothing_else() {
        let mut builder = CoreModel::builder().with_equivalence_bound(EquivalenceBound {
            max_members: 1,
            max_links: 1,
        });
        for statement in [
            stated(&node("a"), SKOS_EXACT_MATCH, &node("b")),
            stated(&node("b"), SKOS_EXACT_MATCH, &node("c")),
        ] {
            builder.push(statement);
        }
        let model = builder.build();

        assert_eq!(verdict(&model, SkosRule::S46), Verdict::Unchecked);
        assert_eq!(verdict(&model, SkosRule::S27), Verdict::Held);
    }

    /// A vocabulary that refines `skos:related` leaves S27 unchecked rather than held.
    ///
    /// The statements made with `ex:seeAlso` are read as non-SKOS, so §8.4's check ran over a
    /// graph missing the author's own associative links. "Held" would be a false negative
    /// produced by an entailment we chose not to perform.
    #[test]
    fn a_declared_refinement_of_skos_related_leaves_s27_unchecked() {
        let model = CoreModel::from_statements([
            declaring(&iri("seeAlso"), RDFS_SUB_PROPERTY_OF, SKOS_RELATED),
            stated(&node("a"), &iri("seeAlso"), &node("b")),
            stated(&node("a"), SKOS_BROADER, &node("b")),
        ]);

        // The violation is real and invisible: with `ex:seeAlso` read, this is Example 26.
        assert!(model.is_consistent());
        let s27 = outcome(&model, SkosRule::S27);
        assert_eq!(s27.verdict(), Verdict::Unchecked);
        assert!(
            matches!(s27.caveats.as_slice(), [Caveat::UnreadRefinement(unread)]
                if unread.reaches() == SKOS_RELATED),
            "{:?}",
            s27.caveats
        );
        // And it says nothing about the labels.
        assert_eq!(verdict(&model, SkosRule::S13), Verdict::Held);
    }

    /// `rdfs:subClassOf skos:Concept` leaves the two class conditions unchecked.
    #[test]
    fn a_declared_sub_class_of_skos_concept_leaves_s9_and_s37_unchecked() {
        let model = CoreModel::from_statements([
            declaring(&iri("ProductCategory"), RDFS_SUB_CLASS_OF, SKOS_CONCEPT),
            is_a(&node("shoes"), crate::model::SkosClass::ConceptScheme),
            stated(
                &node("shoes"),
                crate::model::RDF_TYPE,
                &node("ProductCategory"),
            ),
        ]);

        assert_eq!(verdict(&model, SkosRule::S9), Verdict::Unchecked);
        assert_eq!(verdict(&model, SkosRule::S37), Verdict::Unchecked);
        assert_eq!(verdict(&model, SkosRule::S14), Verdict::Held);
    }

    /// A refinement of a documentation property clouds no condition at all.
    ///
    /// §7 states none, and the `refinement` pass resolves those properly. A caveat here would be
    /// noise on the commonest extension in enterprise data.
    #[test]
    fn a_declared_refinement_of_a_note_property_leaves_every_condition_held() {
        let model = CoreModel::from_statements([
            declaring(
                &iri("usageNote"),
                RDFS_SUB_PROPERTY_OF,
                crate::notes::SKOS_SCOPE_NOTE,
            ),
            stated(&node("a"), &iri("usageNote"), &node("b")),
        ]);

        assert!(model
            .integrity()
            .iter()
            .all(|outcome| outcome.verdict() == Verdict::Held));
    }

    /// A violation outranks a caveat, and the caveat stays on the outcome.
    ///
    /// A counter-example that was found is found whether or not the search was exhaustive, and
    /// "violated, and there may be more" is the honest sentence.
    #[test]
    fn a_violation_outranks_a_caveat_and_the_caveat_survives() {
        let mut builder = CoreModel::builder().with_ancestry_bound(WalkBound {
            max_nodes: 1,
            max_links: 2,
        });
        for statement in [
            stated(&node("a"), SKOS_BROADER, &node("b")),
            stated(&node("a"), SKOS_RELATED, &node("b")),
            stated(&node("m"), SKOS_BROADER, &node("n")),
            stated(&node("n"), SKOS_BROADER, &node("o")),
            stated(&node("m"), SKOS_RELATED, &node("z")),
        ] {
            builder.push(statement);
        }
        let model = builder.build();

        let s27 = outcome(&model, SkosRule::S27);
        assert_eq!(s27.verdict(), Verdict::Violated);
        assert!(!s27.violations.is_empty());
        assert!(
            !s27.caveats.is_empty(),
            "the caveat must survive the verdict"
        );
    }

    /// An exhausted scan of the graph's own declarations leaves every condition unchecked.
    ///
    /// A declaration we never read could have reached any term at all, so there is nothing to
    /// attribute it to and everything is in doubt. Blunt, and the only honest answer.
    #[test]
    fn an_exhausted_declaration_scan_leaves_every_condition_unchecked() {
        let mut builder = CoreModel::builder().with_refinement_scan_bound(RefinementScanBound {
            max_terms: 1,
            max_steps: 10,
        });
        for n in 0..4 {
            builder.push(declaring(
                &iri(&format!("p{n}")),
                RDFS_SUB_PROPERTY_OF,
                crate::notes::SKOS_SCOPE_NOTE,
            ));
        }
        let model = builder.build();

        assert!(model
            .integrity()
            .iter()
            .all(|outcome| outcome.verdict() == Verdict::Unchecked));
    }

    /// The roll-call and `is_consistent` cannot disagree.
    ///
    /// A graph is consistent exactly when no row is violated. Asserted over every finding the
    /// model can raise rather than over one graph, because the risk is a finding that is
    /// classified inconsistent and attributed to nothing.
    #[test]
    fn a_graph_is_consistent_exactly_when_no_condition_is_violated() {
        for statements in [
            vec![],
            vec![
                is_a(&node("both"), crate::model::SkosClass::Concept),
                is_a(&node("both"), crate::model::SkosClass::ConceptScheme),
            ],
            vec![
                labelled(&node("a"), SKOS_PREF_LABEL, "colour", "en"),
                labelled(&node("a"), SKOS_PREF_LABEL, "color", "en"),
            ],
            vec![
                stated(&node("a"), SKOS_EXACT_MATCH, &node("b")),
                stated(&node("a"), SKOS_BROAD_MATCH, &node("b")),
            ],
            vec![
                is_a(&node("label"), crate::model::SkosClass::Label),
                is_a(&node("label"), crate::model::SkosClass::Concept),
            ],
        ] {
            let model = CoreModel::from_statements(statements);
            let violated = model
                .integrity()
                .iter()
                .any(|outcome| outcome.verdict() == Verdict::Violated);
            assert_eq!(model.is_consistent(), !violated, "{:?}", model.findings());
        }
    }
}
