//! Reading the retirement marker back out — what a vocabulary says is no longer current.
//!
//! [`deprecate`](crate::deprecate) is the write half: it computes the statements that retire one
//! concept. This is the read half, and it exists because of what that half deliberately does not
//! do. A deprecation **removes nothing** (`docs/adr/0040`), so a retired concept keeps its type,
//! its labels, its place in the hierarchy and every link into it — which means every command that
//! browses a vocabulary shows it exactly as it showed it before unless something reads the marker.
//! Marking a term obsolete and then being offered it by the next search is the failure this
//! module exists to prevent.
//!
//! # Why this is not part of [`CoreModel`](crate::CoreModel)
//!
//! `owl:deprecated` is not SKOS. SKOS 2009 has no status vocabulary at all, which is why the write
//! half borrows OWL 2's annotation property and Dublin Core's replacement predicate in the first
//! place. [`CoreModel`](crate::CoreModel) reads a graph **as SKOS** — its resources, its classes,
//! its integrity conditions are all SKOS's — and putting a non-SKOS status inside it would make
//! that boundary a matter of taste rather than a rule. So a [`Retirements`] index is built beside
//! the model, from the same stream of statements, exactly as
//! [`DeprecationScan`](crate::DeprecationScan) is built beside it for one concept.
//!
//! The difference from [`DeprecationScan`](crate::DeprecationScan) is scope, and it is why this is
//! a separate type rather than a loop over that one: a scan answers about **one** concept named in
//! advance, and a browse command does not know which of the concepts it is about to print are
//! retired until it has printed them.
//!
//! # Why nothing here is bounded
//!
//! Every other enumeration in this crate carries a bound, and the six of them are recorded in
//! `docs/UNTESTED.md` as constants measured against nothing. This one deliberately has none, and
//! the reason is a containment argument rather than an optimism: the retired resources are a
//! **subset** of the resources [`CoreModel`](crate::CoreModel) already holds unbounded, and the
//! replacements recorded about them are a subset of the statements it already read. A caller that
//! can hold the model can hold this. Adding a seventh unmeasured constant to guard something
//! strictly smaller than an unguarded thing would be a ritual, not a safeguard.
//!
//! # Lenient on read
//!
//! OWL 2 §5.5 requires `"true"^^xsd:boolean` and that is what this build writes. A vocabulary that
//! arrived from another tool carrying a plain `"true"` is still saying the concept is retired, and
//! reading that as "current" would be the same false negative from the other direction. The same
//! leniency [`deprecate`](crate::deprecate) applies, from the same function.
//!
//! # The half-retirement this can see and the write half cannot
//!
//! A resource carrying `dcterms:isReplacedBy` and **no** `owl:deprecated` is a vocabulary that
//! recorded a successor without retiring the predecessor. Nothing in either standard forbids it,
//! and `openbiz deprecate` cannot produce it — it writes both or neither. It arrives by import,
//! from another tool, or from an editor who wrote one statement and not the other, and it reads to
//! every browse command as a perfectly current concept that happens to point somewhere. It is
//! recorded here, and reported by `openbiz inspect`, because the alternative is that the most
//! likely way a retirement goes wrong is the one thing nothing looks at.

use std::collections::{BTreeMap, BTreeSet};

use crate::deprecate::{says_true, DCTERMS_IS_REPLACED_BY, OWL_DEPRECATED};
use crate::model::{Node, Statement};

/// What one vocabulary says about the status of one resource.
///
/// Present in a [`Retirements`] index only for a resource the vocabulary says *something* about:
/// either that it is deprecated, or what replaces it, or both. A resource with neither has no
/// entry, which is what makes [`Retirements::is_retired`] a lookup rather than a scan.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Retirement {
    retired: bool,
    replaced_by: BTreeSet<Node>,
}

impl Retirement {
    /// Whether the vocabulary marks it `owl:deprecated`.
    ///
    /// False for the half-retirement this module's documentation describes: something recorded as
    /// replaced, and never marked.
    pub fn is_retired(&self) -> bool {
        self.retired
    }

    /// What the vocabulary records as superseding it, in IRI order.
    ///
    /// Usually none or one. More than one is not an error — `dcterms:isReplacedBy` states no
    /// cardinality — and a concept split across several successors is exactly how a vocabulary
    /// says so, so this is a set and the callers say "one of these" rather than picking.
    pub fn replaced_by(&self) -> &BTreeSet<Node> {
        &self.replaced_by
    }

    /// Whether it is marked retired with nothing recorded to use instead.
    ///
    /// A legitimate and common state — a term can go out of use with no successor — and the one a
    /// reader most needs telling apart from "retired, use this instead", because only the second
    /// gives them somewhere to go.
    pub fn is_dead_end(&self) -> bool {
        self.retired && self.replaced_by.is_empty()
    }

    /// Whether it records a replacement without being marked retired.
    ///
    /// See the module documentation: `openbiz deprecate` cannot produce this, so it arrived from
    /// somewhere else, and it reads as a current concept everywhere.
    pub fn is_unmarked(&self) -> bool {
        !self.retired && !self.replaced_by.is_empty()
    }
}

/// What a whole vocabulary says is no longer current.
///
/// Built by streaming every statement past [`RetirementsBuilder::push`], beside the
/// [`CoreModel`](crate::CoreModel) and in the same pass. Holds an entry only for a resource the
/// vocabulary says something about, so an ordinary vocabulary with no retirements at all costs an
/// empty map.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Retirements {
    entries: BTreeMap<Node, Retirement>,
}

impl Retirements {
    /// Start collecting.
    pub fn builder() -> RetirementsBuilder {
        RetirementsBuilder {
            retirements: Retirements::default(),
        }
    }

    /// Read a whole vocabulary at once, for a caller that already holds the statements.
    pub fn from_statements(statements: impl IntoIterator<Item = Statement>) -> Self {
        let mut builder = Retirements::builder();
        for statement in statements {
            builder.push(statement);
        }
        builder.build()
    }

    /// Whether the vocabulary marks this resource `owl:deprecated`.
    pub fn is_retired(&self, node: &Node) -> bool {
        self.entries.get(node).is_some_and(Retirement::is_retired)
    }

    /// What the vocabulary says about this resource's status, if it says anything.
    pub fn get(&self, node: &Node) -> Option<&Retirement> {
        self.entries.get(node)
    }

    /// Every resource the vocabulary marks retired, in IRI order.
    pub fn retired(&self) -> impl Iterator<Item = (&Node, &Retirement)> {
        self.entries
            .iter()
            .filter(|(_, retirement)| retirement.is_retired())
    }

    /// How many resources the vocabulary marks retired.
    pub fn count(&self) -> usize {
        self.retired().count()
    }

    /// Every resource recording a replacement while not marked retired, in IRI order.
    pub fn unmarked(&self) -> impl Iterator<Item = (&Node, &Retirement)> {
        self.entries
            .iter()
            .filter(|(_, retirement)| retirement.is_unmarked())
    }

    /// Whether the vocabulary says nothing about the status of anything.
    ///
    /// True for the overwhelming majority of vocabularies, and what lets a report leave the whole
    /// section out rather than printing a row of zeroes on every one of them.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Collects retirement statements while the vocabulary streams past.
#[derive(Debug, Clone, Default)]
pub struct RetirementsBuilder {
    retirements: Retirements,
}

impl RetirementsBuilder {
    /// Offer one statement of the vocabulary.
    pub fn push(&mut self, statement: Statement) {
        if statement.predicate == OWL_DEPRECATED && says_true(&statement.object) {
            self.retirements
                .entries
                .entry(statement.subject)
                .or_default()
                .retired = true;
            return;
        }

        if statement.predicate == DCTERMS_IS_REPLACED_BY {
            if let Some(node) = statement.object.as_node() {
                // A resource replaced by itself says nothing and would render as a signpost
                // pointing at the sign. Dropped here rather than at every reader.
                if *node == statement.subject {
                    return;
                }
                let node = node.clone();
                self.retirements
                    .entries
                    .entry(statement.subject)
                    .or_default()
                    .replaced_by
                    .insert(node);
            }
        }
    }

    /// The finished index.
    pub fn build(self) -> Retirements {
        self.retirements
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::XSD_STRING;
    use crate::model::{Literal, Term};
    use crate::ns;

    fn ex(name: &str) -> Node {
        Node::iri(format!("http://example.org/{name}"))
    }

    /// `owl:deprecated "true"^^xsd:boolean`, exactly as `openbiz deprecate` writes it.
    fn marked(name: &str) -> Statement {
        Statement::new(
            ex(name),
            OWL_DEPRECATED.to_owned(),
            Term::Literal(Literal {
                value: "true".to_owned(),
                language: None,
                datatype: crate::deprecate::XSD_BOOLEAN.to_owned(),
            }),
        )
    }

    fn replaced(name: &str, by: &str) -> Statement {
        Statement::new(
            ex(name),
            DCTERMS_IS_REPLACED_BY.to_owned(),
            Node::iri(format!("http://example.org/{by}")),
        )
    }

    #[test]
    fn a_vocabulary_that_retires_nothing_produces_an_empty_index() {
        let index = Retirements::from_statements([Statement::new(
            ex("Radio"),
            format!("{}prefLabel", ns::SKOS),
            Term::Literal(Literal {
                value: "Radio".to_owned(),
                language: Some("en".to_owned()),
                datatype: crate::labels::RDF_LANG_STRING.to_owned(),
            }),
        )]);

        assert!(index.is_empty());
        assert_eq!(index.count(), 0);
        assert!(!index.is_retired(&ex("Radio")));
        assert!(index.get(&ex("Radio")).is_none());
    }

    #[test]
    fn the_marker_and_the_replacement_are_read_together() {
        let index =
            Retirements::from_statements([marked("Wireless"), replaced("Wireless", "Radio")]);

        assert!(index.is_retired(&ex("Wireless")));
        let entry = index.get(&ex("Wireless")).expect("an entry");
        assert!(entry.is_retired());
        assert_eq!(entry.replaced_by(), &BTreeSet::from([ex("Radio")]));
        assert!(!entry.is_dead_end());
        assert!(!entry.is_unmarked());
        assert_eq!(index.count(), 1);
    }

    /// A term can go out of use with nothing taking its place. That is ordinary, and it is the
    /// state a reader most needs told apart from "retired, use this instead".
    #[test]
    fn a_retirement_with_no_replacement_is_a_dead_end_and_not_an_error() {
        let index = Retirements::from_statements([marked("Wireless")]);

        let entry = index.get(&ex("Wireless")).expect("an entry");
        assert!(entry.is_dead_end());
        assert!(entry.replaced_by().is_empty());
    }

    /// OWL 2 §5.5 requires the typed literal; another tool's plain `"true"` still means retired.
    /// Reading it as current would offer an obsolete term to the next person who searches.
    #[test]
    fn a_plain_string_true_is_read_as_retired() {
        let index = Retirements::from_statements([Statement::new(
            ex("Wireless"),
            OWL_DEPRECATED.to_owned(),
            Term::Literal(Literal {
                value: "true".to_owned(),
                language: None,
                datatype: XSD_STRING.to_owned(),
            }),
        )]);

        assert!(index.is_retired(&ex("Wireless")));
    }

    #[test]
    fn owl_deprecated_false_leaves_the_concept_current() {
        let index = Retirements::from_statements([Statement::new(
            ex("Wireless"),
            OWL_DEPRECATED.to_owned(),
            Term::Literal(Literal {
                value: "false".to_owned(),
                language: None,
                datatype: crate::deprecate::XSD_BOOLEAN.to_owned(),
            }),
        )]);

        assert!(!index.is_retired(&ex("Wireless")));
        assert!(index.is_empty());
    }

    /// The half-retirement `openbiz deprecate` cannot produce and every browse command misreads.
    #[test]
    fn a_replacement_without_the_marker_is_recorded_as_unmarked() {
        let index = Retirements::from_statements([replaced("Wireless", "Radio")]);

        assert!(!index.is_retired(&ex("Wireless")));
        let entry = index.get(&ex("Wireless")).expect("an entry");
        assert!(entry.is_unmarked());
        assert_eq!(
            index
                .unmarked()
                .map(|(node, _)| node.clone())
                .collect::<Vec<_>>(),
            vec![ex("Wireless")]
        );
        assert_eq!(index.count(), 0);
    }

    /// `dcterms:isReplacedBy` states no cardinality, and a concept superseded by several is how a
    /// vocabulary records a split that has already happened. Kept, not picked between.
    #[test]
    fn several_replacements_are_all_kept() {
        let index = Retirements::from_statements([
            marked("Banks"),
            replaced("Banks", "RiverBanks"),
            replaced("Banks", "FinancialBanks"),
        ]);

        let entry = index.get(&ex("Banks")).expect("an entry");
        assert_eq!(
            entry.replaced_by(),
            &BTreeSet::from([ex("FinancialBanks"), ex("RiverBanks")])
        );
    }

    /// A signpost pointing at its own sign says nothing, and would render as one in every report.
    #[test]
    fn a_concept_replaced_by_itself_records_no_replacement() {
        let index =
            Retirements::from_statements([marked("Wireless"), replaced("Wireless", "Wireless")]);

        let entry = index.get(&ex("Wireless")).expect("an entry");
        assert!(entry.is_dead_end());
    }

    /// A literal replacement is not a resource to send anyone to. Dropped rather than rendered.
    #[test]
    fn a_literal_replacement_is_not_recorded() {
        let index = Retirements::from_statements([
            marked("Wireless"),
            Statement::new(
                ex("Wireless"),
                DCTERMS_IS_REPLACED_BY.to_owned(),
                Term::Literal(Literal {
                    value: "Radio".to_owned(),
                    language: Some("en".to_owned()),
                    datatype: crate::labels::RDF_LANG_STRING.to_owned(),
                }),
            ),
        ]);

        assert!(index.get(&ex("Wireless")).expect("an entry").is_dead_end());
    }

    /// The index is over the whole vocabulary and not over one named concept, which is the reason
    /// it exists beside `DeprecationScan` rather than being a loop over it.
    #[test]
    fn every_retired_resource_in_the_vocabulary_is_listed_in_iri_order() {
        let index = Retirements::from_statements([
            marked("Wireless"),
            marked("Aerials"),
            replaced("Aerials", "Antennas"),
            marked("Telegraphy"),
        ]);

        assert_eq!(index.count(), 3);
        assert_eq!(
            index
                .retired()
                .map(|(node, _)| node.clone())
                .collect::<Vec<_>>(),
            vec![ex("Aerials"), ex("Telegraphy"), ex("Wireless")]
        );
    }

    /// Nothing here is SKOS, so nothing here requires the subject to be a concept. A retired
    /// scheme or collection is a legitimate thing for a vocabulary to say.
    #[test]
    fn a_retired_resource_need_not_be_a_concept() {
        let index = Retirements::from_statements([marked("OldScheme")]);

        assert!(index.is_retired(&ex("OldScheme")));
    }

    /// The round trip that makes this the read half of a pair rather than a second reading of the
    /// same standards. What `CoreModel::deprecate` writes is exactly what this reads back, so a
    /// change to either side that broke the other would fail here rather than in a report.
    #[test]
    fn what_the_write_half_produces_is_what_this_reads() {
        use crate::model::{CoreModel, SkosClass, RDF_TYPE};

        let vocabulary = vec![
            Statement::new(
                ex("Wireless"),
                RDF_TYPE.to_owned(),
                Node::iri(SkosClass::Concept.iri()),
            ),
            Statement::new(
                ex("Radio"),
                RDF_TYPE.to_owned(),
                Node::iri(SkosClass::Concept.iri()),
            ),
        ];
        let model = CoreModel::from_statements(vocabulary.iter().cloned());
        let mut scan =
            crate::deprecate::DeprecationScan::builder(ex("Wireless"), Some(ex("Radio")));
        for statement in &vocabulary {
            scan.push(statement.clone());
        }
        let deprecation = model
            .deprecate(&scan.build(), Some("out of use since 1930"), Some("en"))
            .expect("a retirable concept");

        let index = Retirements::from_statements(
            vocabulary
                .into_iter()
                .chain(deprecation.additions().iter().cloned()),
        );

        assert!(index.is_retired(&ex("Wireless")));
        assert_eq!(
            index.get(&ex("Wireless")).expect("an entry").replaced_by(),
            &BTreeSet::from([ex("Radio")])
        );
        // And the replacement is untouched, which is the write half's other promise.
        assert!(!index.is_retired(&ex("Radio")));
        assert!(index.get(&ex("Radio")).is_none());
    }
}
