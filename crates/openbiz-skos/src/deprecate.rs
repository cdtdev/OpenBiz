//! Retiring a concept without deleting it, and saying what still points at the retired thing.
//!
//! The fourth of `docs/BUILD-PLAN.md`'s bulk operations, and the one that exists because of what
//! the other three cannot do. A [`merge`](crate::merge) makes an IRI stop existing; a
//! [`split`](crate::split) leaves the original in place with no way to retire it. Neither is what
//! a governance team wants when a term goes out of use: the IRI has been published, other systems
//! have stored it, and an auditor asked in three years' time what it meant needs the concept to
//! still be there and to say plainly that it is no longer current and what to use instead.
//!
//! So this **removes nothing**. Like every other operation in this crate it writes nothing either:
//! a [`Deprecation`] is an *answer* — the statements it would add — computed against a
//! [`CoreModel`] and a [`DeprecationScan`] read a moment ago. The caller stages them as a
//! candidate; a human approves them.
//!
//! # The three statements, and why they are these three
//!
//! **SKOS defines no deprecation term.** That is a fact about the specification rather than a gap
//! in this build: SKOS 2009 has no status vocabulary at all, and `CLAUDE.md` §2 forbids inventing
//! a proprietary substitute for something already standardised elsewhere. So the marker comes from
//! OWL 2 and the replacement from Dublin Core, which is what published SKOS vocabularies do:
//!
//! - **`owl:deprecated true`** — OWL 2 §5.5 defines it as an annotation property whose value is
//!   `"true"^^xsd:boolean`, deliberately with no logical consequences, which is exactly right for
//!   a status marker: nothing about the concept's meaning changes, only how it should be used.
//! - **`dcterms:isReplacedBy`** — "a related resource that supplants, displaces, or supersedes the
//!   described resource". Optional, because a term can go out of use with nothing taking its
//!   place, and refusing that would be refusing an ordinary editorial act.
//! - **`skos:changeNote`** — the operator's own sentence about why, when they give one. SKOS §7
//!   separates a note about a *modification* from `skos:historyNote`'s note about a past state;
//!   deprecating is the modification, so this is a change note. A vocabulary that prefers history
//!   notes can write one with `openbiz import` — this command does not stop it.
//!
//! What is **not** written here is who did it and when. That goes in the candidate, where every
//! other command in this build records it, and the consequence is stated rather than hidden: an
//! export of the vocabulary carries the deprecation and its replacement but not its date or its
//! author. It is in `docs/UNTESTED.md`.
//!
//! # The half it refuses to do
//!
//! Deprecating a concept does not move its children, retract its links, or repoint the mappings
//! other vocabularies have made to it. Every one of those is a decision only a person can make —
//! a live child under a retired parent may want re-parenting under the replacement, or may want
//! retiring too, and nothing in the graph says which. This is the same division
//! [`split`](crate::split) makes for the same reason, and the report is where it is honoured:
//! [`Deprecation::stranded`] counts what is still attached, and a report that omitted it would
//! read as if retiring a concept finished the job.
//!
//! In particular, **a replacement does not repoint anything**. `dcterms:isReplacedBy` is a
//! signpost, not a rewrite: the vocabulary still says what it said. Repointing every reference is
//! what [`merge`](crate::merge) does, and it does it by making the old IRI stop existing — which
//! is the thing a deprecation exists to avoid.
//!
//! # What it refuses
//!
//! - **A concept the vocabulary says nothing about**, or one that is not a `skos:Concept`.
//! - **Deprecating what is already deprecated** when there is nothing new to say. Proposing a
//!   candidate that changes nothing wastes a reviewer's attention.
//! - **Replacing a concept with itself**, and **replacing it with something this vocabulary holds
//!   that is not a concept** — a scheme or a collection as a replacement is a typo, not a
//!   decision.
//! - **A replacement that is itself deprecated**, which is a trail that leads nowhere.
//! - **A second, different replacement** for a concept that already records one. Changing it means
//!   retracting a statement, and this operation removes nothing.
//! - **A scan that hit its bound**, because an incomplete scan cannot establish that a concept is
//!   *not* already deprecated, and every refusal above rests on that absence.
//!
//! What it does **not** refuse is a replacement this vocabulary has never heard of. A term retired
//! here in favour of one in the corporate vocabulary next door is ordinary governance, and
//! `dcterms:isReplacedBy` is deliberately a link to any resource. The caller warns; it does not
//! block.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::labels::{LabelKind, LexicalLabel, RDF_LANG_STRING, XSD_STRING};
use crate::model::{CoreModel, Literal, Node, SkosClass, Statement, Term};
use crate::notes::SKOS_CHANGE_NOTE;
use crate::relations::SemanticRelation;

/// `owl:deprecated` — OWL 2's status marker, and the one this build writes.
///
/// OWL 2 §5.5 makes it an annotation property with no logical consequences. That is the property
/// this needs: a deprecated concept means exactly what it always meant, and every inference drawn
/// from it before is still sound. What changed is whether anyone should use it again.
pub const OWL_DEPRECATED: &str = "http://www.w3.org/2002/07/owl#deprecated";

/// `dcterms:isReplacedBy` — the signpost from the retired concept to the current one.
///
/// Only this direction is written. DCMI defines `dcterms:replaces` as the converse in prose but
/// declares no `owl:inverseOf` between them, so asserting both would be two claims where the
/// standard licenses one — and the second would be a statement about the *replacement*, which is
/// a live concept this change has no business editing.
pub const DCTERMS_IS_REPLACED_BY: &str = "http://purl.org/dc/terms/isReplacedBy";

/// `xsd:boolean`, the datatype OWL 2 requires on `owl:deprecated`.
pub const XSD_BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

/// How many `dcterms:isReplacedBy` statements about one concept a scan will hold.
///
/// Bounded like every other enumeration in this crate, and for the same reason: the input is a
/// customer's vocabulary and nothing in RDF stops one concept being the subject of a million
/// statements. Unlike the walks, hitting this bound means the scan **cannot answer** rather than
/// answering partially — "this concept records no replacement" is an absence, and a truncated scan
/// cannot establish one — so [`CoreModel::deprecate`] refuses rather than proceeding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusBound {
    /// The most replacement statements about one concept that will be kept.
    pub max_replacements: usize,
}

impl StatusBound {
    /// The bound every caller in this build uses unless it says otherwise.
    ///
    /// 1 000 replacements for one concept. **This is a judgement measured against nothing**, and
    /// it is recorded as such in `docs/UNTESTED.md` alongside the constants before it. The
    /// reasoning is thinner than it looks only because the quantity is: a concept superseded by a
    /// thousand others is not a deprecation, it is a corrupt graph, and the number is here to stop
    /// one from exhausting memory rather than to describe anything real.
    pub const DEFAULT: StatusBound = StatusBound {
        max_replacements: 1_000,
    };
}

impl Default for StatusBound {
    fn default() -> Self {
        StatusBound::DEFAULT
    }
}

/// What a vocabulary already says about the status of a concept and of its proposed replacement.
///
/// Built by streaming the whole graph past [`DeprecationScanBuilder::push`]. It exists because
/// `owl:deprecated` and `dcterms:isReplacedBy` are not SKOS, so [`CoreModel`] — which reads a
/// graph *as SKOS* — has nothing to say about them. Keeping this crate engine-free means the
/// statements arrive from the caller rather than from a store.
///
/// It keeps counts and a small set rather than the statements themselves: unlike a merge, nothing
/// here has to rewrite what it read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeprecationScan {
    concept: Node,
    replacement: Option<Node>,
    concept_deprecated: bool,
    replaced_by: BTreeSet<Node>,
    replacement_deprecated: bool,
    incoming: usize,
    complete: bool,
}

impl DeprecationScan {
    /// Start a scan for the concept being retired, and the replacement if one was named.
    pub fn builder(concept: Node, replacement: Option<Node>) -> DeprecationScanBuilder {
        DeprecationScanBuilder {
            scan: DeprecationScan {
                concept,
                replacement,
                concept_deprecated: false,
                replaced_by: BTreeSet::new(),
                replacement_deprecated: false,
                incoming: 0,
                complete: true,
            },
            bound: StatusBound::DEFAULT,
        }
    }

    /// The concept being retired.
    pub fn concept(&self) -> &Node {
        &self.concept
    }

    /// The replacement that was named, if one was.
    pub fn replacement(&self) -> Option<&Node> {
        self.replacement.as_ref()
    }

    /// Whether the vocabulary already marks the concept deprecated.
    pub fn concept_deprecated(&self) -> bool {
        self.concept_deprecated
    }

    /// Whether the vocabulary already marks the *replacement* deprecated.
    ///
    /// Always `false` for a replacement in another vocabulary, which this graph cannot see. The
    /// caller says so rather than letting the silence read as an answer.
    pub fn replacement_deprecated(&self) -> bool {
        self.replacement_deprecated
    }

    /// What the vocabulary already records as replacing the concept.
    pub fn replaced_by(&self) -> &BTreeSet<Node> {
        &self.replaced_by
    }

    /// How many statements in this vocabulary point *at* the concept.
    ///
    /// Every one of them keeps working after a deprecation — that is the point of not deleting —
    /// and every one of them now points at something no longer current.
    pub fn incoming(&self) -> usize {
        self.incoming
    }

    /// Whether the scan kept everything it saw rather than stopping at its bound.
    pub fn is_complete(&self) -> bool {
        self.complete
    }
}

/// Collects what a deprecation needs while the graph streams past.
#[derive(Debug, Clone)]
pub struct DeprecationScanBuilder {
    scan: DeprecationScan,
    bound: StatusBound,
}

impl DeprecationScanBuilder {
    /// Use a different bound. See [`StatusBound::DEFAULT`] for what the standing one is.
    pub fn with_bound(mut self, bound: StatusBound) -> Self {
        self.bound = bound;
        self
    }

    /// Offer one statement of the vocabulary.
    pub fn push(&mut self, statement: Statement) {
        if statement.object.as_node() == Some(&self.scan.concept)
            && statement.subject != self.scan.concept
        {
            self.scan.incoming += 1;
        }

        let deprecated = statement.predicate == OWL_DEPRECATED && says_true(&statement.object);
        if statement.subject == self.scan.concept {
            if deprecated {
                self.scan.concept_deprecated = true;
            }
            if statement.predicate == DCTERMS_IS_REPLACED_BY {
                if let Some(node) = statement.object.as_node() {
                    // A truncating set would make the absence unreadable: "this concept records no
                    // replacement" is what every refusal below rests on, and a half-kept set
                    // answers it wrongly rather than not at all.
                    match self.scan.replaced_by.len() < self.bound.max_replacements {
                        true => {
                            self.scan.replaced_by.insert(node.clone());
                        }
                        false => self.scan.complete = false,
                    }
                }
            }
        }

        if deprecated && Some(&statement.subject) == self.scan.replacement.as_ref() {
            self.scan.replacement_deprecated = true;
        }
    }

    /// The finished scan.
    pub fn build(self) -> DeprecationScan {
        self.scan
    }
}

/// Whether a term is the `"true"^^xsd:boolean` OWL 2 asks for.
///
/// Lenient on read, strict on write. OWL 2 §5.5 requires the typed literal and that is what
/// [`CoreModel::deprecate`] produces — but a vocabulary that arrived from another tool carrying a
/// plain `"true"` is still telling us the concept is deprecated, and reading that as "not
/// deprecated" would make this command propose a second marker for a concept that already has one.
pub(crate) fn says_true(term: &Term) -> bool {
    match term {
        Term::Literal(literal) => {
            literal.value == "true"
                && (literal.datatype == XSD_BOOLEAN || literal.datatype == XSD_STRING)
        }
        Term::Node(_) => false,
    }
}

/// What is still attached to a concept after it is retired, which a human now has to decide about.
///
/// Nothing here is touched by the deprecation. The counts are what makes the report honest: a
/// retirement that said nothing about them would read as if the work were finished, and the most
/// consequential of them — a live child under a retired parent — is invisible in every tree view
/// that does not check its parent's status.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stranded {
    /// The concepts directly below it, still current, still under something that is not.
    pub narrower: Vec<Node>,
    /// The concepts above it, which keep a retired child.
    pub broader: Vec<Node>,
    /// The concepts it is associatively linked to.
    pub related: Vec<Node>,
    /// How many *resources* it is still mapped to, across all five mapping properties.
    ///
    /// Resources and not statements, which is a distinction a test found the hard way: SKOS §10.2
    /// makes `skos:exactMatch` a sub-property of `skos:closeMatch` (S42), so the model holds two
    /// links for one stated `skos:exactMatch` and counting statements reports a concept mapped
    /// once as mapped twice. What a reviewer has to decide about is the other resource, so that is
    /// what is counted.
    pub mapped_to: usize,
    /// The schemes it is a top concept of, where it heads a browse tree.
    pub top_concept_of: Vec<Node>,
    /// The collections that still list it as a member.
    pub collections: Vec<Node>,
    /// How many statements in this vocabulary point at it, from the raw graph.
    ///
    /// A superset of the links above: it counts every statement, including the ones SKOS has no
    /// reading of, which is the same reason a merge reads the raw graph.
    pub incoming: usize,
}

impl Stranded {
    /// Whether anything at all is left needing a decision.
    pub fn is_empty(&self) -> bool {
        self.narrower.is_empty()
            && self.broader.is_empty()
            && self.related.is_empty()
            && self.mapped_to == 0
            && self.top_concept_of.is_empty()
            && self.collections.is_empty()
    }
}

/// What retiring one concept would add to a vocabulary.
///
/// Produced by [`CoreModel::deprecate`] and applied by nobody: the statements are a proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deprecation {
    concept: Node,
    replacement: Option<Node>,
    note: Option<LexicalLabel>,
    marks: bool,
    additions: Vec<Statement>,
    stranded: Stranded,
}

impl Deprecation {
    /// The concept being retired, which this change leaves in place.
    pub fn concept(&self) -> &Node {
        &self.concept
    }

    /// What supersedes it, if anything was named.
    pub fn replacement(&self) -> Option<&Node> {
        self.replacement.as_ref()
    }

    /// The change note this would write, if one was given.
    pub fn note(&self) -> Option<&LexicalLabel> {
        self.note.as_ref()
    }

    /// Whether this writes the `owl:deprecated` marker, or the concept already carried it.
    ///
    /// `false` is the second call: a concept retired earlier, now given the replacement that was
    /// not known at the time.
    pub fn marks(&self) -> bool {
        self.marks
    }

    /// The statements to add. A deprecation removes nothing.
    pub fn additions(&self) -> &[Statement] {
        &self.additions
    }

    /// What is still attached to the concept and needs a human's judgement.
    pub fn stranded(&self) -> &Stranded {
        &self.stranded
    }
}

/// Nothing could be deprecated, and this says exactly what stopped it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DeprecationError {
    /// The vocabulary has nothing to say about the concept.
    NoSuchConcept {
        /// The IRI that was asked for.
        concept: Node,
    },
    /// The resource exists but is not a `skos:Concept`.
    NotAConcept {
        /// The resource that was asked for.
        resource: Node,
    },
    /// The concept is already deprecated and nothing new was given to record.
    AlreadyDeprecated {
        /// The concept.
        concept: Node,
        /// What the vocabulary already records as replacing it.
        replaced_by: Vec<Node>,
    },
    /// The concept already records a replacement, and a different one was given.
    AlreadyReplaced {
        /// The concept.
        concept: Node,
        /// What it is already recorded as replaced by.
        by: Vec<Node>,
    },
    /// A concept was offered as its own replacement.
    ReplacementIsTheConcept {
        /// The concept.
        concept: Node,
    },
    /// The replacement is in this vocabulary but is not a `skos:Concept`.
    ReplacementNotAConcept {
        /// The resource offered.
        resource: Node,
    },
    /// The replacement is itself deprecated.
    ReplacementDeprecated {
        /// The resource offered.
        replacement: Node,
    },
    /// A change note with nothing in it.
    EmptyNote,
    /// The scan hit its bound, so what the vocabulary already records cannot be established.
    ScanTruncated {
        /// The concept.
        concept: Node,
    },
}

impl fmt::Display for DeprecationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeprecationError::NoSuchConcept { concept } => write!(
                f,
                "this vocabulary says nothing about {concept}, so there is nothing to retire"
            ),
            DeprecationError::NotAConcept { resource } => write!(
                f,
                "{resource} is not a skos:Concept, and retiring one is the only thing this does"
            ),
            DeprecationError::AlreadyDeprecated {
                concept,
                replaced_by,
            } => {
                write!(f, "{concept} is already deprecated")?;
                match replaced_by.is_empty() {
                    true => write!(
                        f,
                        ", with nothing recorded as replacing it; name one with --replaced-by"
                    ),
                    false => {
                        write!(f, ", replaced by")?;
                        for node in replaced_by {
                            write!(f, " {node}")?;
                        }
                        Ok(())
                    }
                }
            }
            DeprecationError::AlreadyReplaced { concept, by } => {
                write!(f, "{concept} already records a replacement:")?;
                for node in by {
                    write!(f, " {node}")?;
                }
                write!(
                    f,
                    ". Recording a different one means retracting that statement, and this \
                     operation removes nothing — retract it with `openbiz retract` first"
                )
            }
            DeprecationError::ReplacementIsTheConcept { concept } => write!(
                f,
                "{concept} cannot replace itself; a concept replaced by itself is not retired"
            ),
            DeprecationError::ReplacementNotAConcept { resource } => write!(
                f,
                "{resource} is in this vocabulary and is not a skos:Concept, \
                 so pointing a retired concept at it would send a reader nowhere useful"
            ),
            DeprecationError::ReplacementDeprecated { replacement } => write!(
                f,
                "{replacement} is itself deprecated, so it cannot be what to use instead"
            ),
            DeprecationError::EmptyNote => {
                write!(f, "the change note given has nothing in it")
            }
            DeprecationError::ScanTruncated { concept } => write!(
                f,
                "there are more statements about what replaces {concept} than this can hold, \
                 so whether it is already retired cannot be established, and every refusal here \
                 depends on that"
            ),
        }
    }
}

impl std::error::Error for DeprecationError {}

impl CoreModel {
    /// The statements that would retire `concept`, leaving everything it already says in place.
    ///
    /// `scan` carries what the raw graph says about the status of the concept and its replacement,
    /// which this model cannot know: `owl:deprecated` and `dcterms:isReplacedBy` are not SKOS.
    /// `note` is the operator's own sentence about why, written as a `skos:changeNote`; `language`
    /// overrides the tag it is given.
    ///
    /// Nothing is written, and **nothing is removed**: the answer is a [`Deprecation`] holding
    /// statements to add and a count of what is still attached to the retired concept. Read the
    /// module note for why that division is the honest one.
    pub fn deprecate(
        &self,
        scan: &DeprecationScan,
        note: Option<&str>,
        language: Option<&str>,
    ) -> Result<Deprecation, DeprecationError> {
        let concept = scan.concept();
        if !scan.is_complete() {
            return Err(DeprecationError::ScanTruncated {
                concept: concept.clone(),
            });
        }

        let Some(resource) = self.resource(concept) else {
            return Err(DeprecationError::NoSuchConcept {
                concept: concept.clone(),
            });
        };
        if !resource.is_a(SkosClass::Concept) {
            return Err(DeprecationError::NotAConcept {
                resource: concept.clone(),
            });
        }

        let note = match note {
            Some(text) if text.trim().is_empty() => return Err(DeprecationError::EmptyNote),
            Some(text) => Some(LexicalLabel {
                language: self.note_language(resource, language),
                text: text.to_owned(),
            }),
            None => None,
        };

        let recorded: Vec<Node> = scan.replaced_by().iter().cloned().collect();
        let replacement = match scan.replacement() {
            Some(replacement) => {
                if replacement == concept {
                    return Err(DeprecationError::ReplacementIsTheConcept {
                        concept: concept.clone(),
                    });
                }
                if let Some(offered) = self.resource(replacement) {
                    if !offered.is_a(SkosClass::Concept) {
                        return Err(DeprecationError::ReplacementNotAConcept {
                            resource: replacement.clone(),
                        });
                    }
                }
                if scan.replacement_deprecated() {
                    return Err(DeprecationError::ReplacementDeprecated {
                        replacement: replacement.clone(),
                    });
                }
                // Already recorded, by this exact IRI: not a second replacement, just a repeat.
                match recorded.contains(replacement) {
                    true => None,
                    false => match recorded.is_empty() {
                        true => Some(replacement.clone()),
                        // A different replacement is a change of mind about a published
                        // statement, and changing it means retracting one. This adds only.
                        false => {
                            return Err(DeprecationError::AlreadyReplaced {
                                concept: concept.clone(),
                                by: recorded,
                            })
                        }
                    },
                }
            }
            None => None,
        };

        let marks = !scan.concept_deprecated();
        if !marks && replacement.is_none() && note.is_none() {
            return Err(DeprecationError::AlreadyDeprecated {
                concept: concept.clone(),
                replaced_by: recorded,
            });
        }

        let mut additions = Vec::new();
        if marks {
            additions.push(Statement::new(
                concept.clone(),
                OWL_DEPRECATED.to_owned(),
                Term::Literal(Literal {
                    value: "true".to_owned(),
                    language: None,
                    datatype: XSD_BOOLEAN.to_owned(),
                }),
            ));
        }
        if let Some(replacement) = &replacement {
            additions.push(Statement::new(
                concept.clone(),
                DCTERMS_IS_REPLACED_BY.to_owned(),
                replacement.clone(),
            ));
        }
        if let Some(note) = &note {
            additions.push(Statement::new(
                concept.clone(),
                SKOS_CHANGE_NOTE.to_owned(),
                Term::Literal(literal(note)),
            ));
        }

        let stranded = Stranded {
            narrower: resource
                .relations(SemanticRelation::Narrower)
                .map(|links| links.keys().cloned().collect())
                .unwrap_or_default(),
            broader: resource
                .relations(SemanticRelation::Broader)
                .map(|links| links.keys().cloned().collect())
                .unwrap_or_default(),
            related: resource
                .relations(SemanticRelation::Related)
                .map(|links| links.keys().cloned().collect())
                .unwrap_or_default(),
            mapped_to: resource
                .mappings()
                .values()
                .flat_map(BTreeMap::keys)
                .collect::<BTreeSet<_>>()
                .len(),
            top_concept_of: resource.top_concept_of().iter().cloned().collect(),
            collections: self.collections_holding(concept),
            incoming: scan.incoming(),
        };

        Ok(Deprecation {
            concept: concept.clone(),
            replacement,
            note,
            marks,
            additions,
            stranded,
        })
    }

    /// The language a change note is written in.
    ///
    /// The caller's tag if it gave one; otherwise the language of the concept's preferred label
    /// when there is exactly one, because that is the language the vocabulary is being authored
    /// in. With none or with several, the note is **untagged** rather than guessed. That is not the
    /// refusal [`split`](crate::split) makes for the same ambiguity, and the difference is what is
    /// at stake: a part's label in the wrong language is a wrong label on a new concept, where an
    /// untagged note is a true note that claims no language.
    pub(crate) fn note_language(
        &self,
        resource: &crate::model::Resource,
        language: Option<&str>,
    ) -> Option<String> {
        if let Some(given) = language {
            let given = given.trim();
            if !given.is_empty() {
                return Some(given.to_ascii_lowercase());
            }
        }
        let languages: BTreeSet<Option<String>> = resource
            .labels_of(LabelKind::Preferred)
            .map(|label| label.language.clone())
            .collect();
        match languages.len() {
            1 => languages.into_iter().next().unwrap_or_default(),
            _ => None,
        }
    }

    /// The collections in this vocabulary that still list `concept` as a member.
    ///
    /// Both ways of listing one: `skos:member`, and the `skos:memberList` of an ordered
    /// collection, which is a different statement about the same membership and would otherwise be
    /// missed in exactly the vocabularies that took the trouble to order their collections.
    fn collections_holding(&self, concept: &Node) -> Vec<Node> {
        self.instances_of(SkosClass::Collection)
            .filter(|(_, resource)| {
                resource.members().contains(concept)
                    || resource
                        .member_lists()
                        .iter()
                        .any(|list| list.items.contains(concept))
            })
            .map(|(node, _)| node.clone())
            .collect()
    }
}

/// The RDF literal behind a note, with the datatype RDF 1.1 gives it.
pub(crate) fn literal(note: &LexicalLabel) -> Literal {
    match &note.language {
        Some(tag) => Literal {
            value: note.text.clone(),
            language: Some(tag.clone()),
            datatype: RDF_LANG_STRING.to_owned(),
        },
        None => Literal {
            value: note.text.clone(),
            language: None,
            datatype: XSD_STRING.to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::MappingProperty;
    use crate::model::{RDF_TYPE, SKOS_MEMBER};
    use crate::ns;

    fn ex(name: &str) -> Node {
        Node::iri(format!("http://example.org/{name}"))
    }

    fn concept(name: &str, label: &str) -> Vec<Statement> {
        vec![
            Statement::new(
                ex(name),
                RDF_TYPE.to_owned(),
                Node::iri(SkosClass::Concept.iri()),
            ),
            Statement::new(
                ex(name),
                format!("{}prefLabel", ns::SKOS),
                Term::Literal(Literal {
                    value: label.to_owned(),
                    language: Some("en".to_owned()),
                    datatype: RDF_LANG_STRING.to_owned(),
                }),
            ),
        ]
    }

    /// A model and a scan built from the same statements, which is what a caller does.
    fn read(
        statements: &[Statement],
        concept: Node,
        replacement: Option<Node>,
    ) -> (CoreModel, DeprecationScan) {
        let model = CoreModel::from_statements(statements.iter().cloned());
        let mut scan = DeprecationScan::builder(concept, replacement);
        for statement in statements {
            scan.push(statement.clone());
        }
        (model, scan.build())
    }

    fn deprecated(name: &str) -> Statement {
        Statement::new(
            ex(name),
            OWL_DEPRECATED.to_owned(),
            Term::Literal(Literal {
                value: "true".to_owned(),
                language: None,
                datatype: XSD_BOOLEAN.to_owned(),
            }),
        )
    }

    #[test]
    fn a_retirement_marks_the_concept_and_removes_nothing() {
        let statements = concept("wireless", "Wireless telegraphy");
        let (model, scan) = read(&statements, ex("wireless"), None);

        let deprecation = model.deprecate(&scan, None, None).expect("a retirement");

        assert!(deprecation.marks());
        assert_eq!(deprecation.replacement(), None);
        assert_eq!(deprecation.additions(), &[deprecated("wireless")]);
    }

    /// The marker is `"true"^^xsd:boolean` and not a plain string, because OWL 2 §5.5 says so and
    /// a tool reading it as an annotation value will compare it as a boolean.
    #[test]
    fn the_marker_carries_the_datatype_owl_requires() {
        let statements = concept("wireless", "Wireless telegraphy");
        let (model, scan) = read(&statements, ex("wireless"), None);

        let deprecation = model.deprecate(&scan, None, None).expect("a retirement");

        let Term::Literal(literal) = &deprecation.additions()[0].object else {
            panic!("the marker is a literal");
        };
        assert_eq!(literal.value, "true");
        assert_eq!(literal.datatype, XSD_BOOLEAN);
        assert_eq!(literal.language, None);
    }

    #[test]
    fn a_replacement_is_written_in_one_direction_only() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.extend(concept("radio", "Radio"));
        let (model, scan) = read(&statements, ex("wireless"), Some(ex("radio")));

        let deprecation = model.deprecate(&scan, None, None).expect("a retirement");

        assert_eq!(deprecation.replacement(), Some(&ex("radio")));
        assert!(deprecation.additions().contains(&Statement::new(
            ex("wireless"),
            DCTERMS_IS_REPLACED_BY.to_owned(),
            ex("radio"),
        )));
        // Nothing is said about the replacement itself: DCMI declares no inverse, and the
        // replacement is a live concept this change has no business editing.
        assert!(deprecation
            .additions()
            .iter()
            .all(|statement| statement.subject == ex("wireless")));
    }

    #[test]
    fn a_note_takes_the_language_the_vocabulary_is_authored_in() {
        let statements = concept("wireless", "Wireless telegraphy");
        let (model, scan) = read(&statements, ex("wireless"), None);

        let deprecation = model
            .deprecate(&scan, Some("Superseded by broadcasting terms."), None)
            .expect("a retirement");

        let note = deprecation.note().expect("a note");
        assert_eq!(note.language.as_deref(), Some("en"));
        assert!(deprecation.additions().contains(&Statement::new(
            ex("wireless"),
            SKOS_CHANGE_NOTE.to_owned(),
            Term::Literal(Literal {
                value: "Superseded by broadcasting terms.".to_owned(),
                language: Some("en".to_owned()),
                datatype: RDF_LANG_STRING.to_owned(),
            }),
        )));
    }

    /// Two languages is the case a split refuses. A note is not a label: untagged is a true note
    /// that claims no language, where an untagged label would be a claim about a term.
    #[test]
    fn a_note_on_a_concept_labelled_twice_over_is_untagged_rather_than_refused() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(Statement::new(
            ex("wireless"),
            format!("{}prefLabel", ns::SKOS),
            Term::Literal(Literal {
                value: "Télégraphie sans fil".to_owned(),
                language: Some("fr".to_owned()),
                datatype: RDF_LANG_STRING.to_owned(),
            }),
        ));
        let (model, scan) = read(&statements, ex("wireless"), None);

        let deprecation = model
            .deprecate(&scan, Some("Out of use."), None)
            .expect("a retirement");

        let note = deprecation.note().expect("a note");
        assert_eq!(note.language, None);
    }

    #[test]
    fn a_given_language_overrides_the_concepts_own() {
        let statements = concept("wireless", "Wireless telegraphy");
        let (model, scan) = read(&statements, ex("wireless"), None);

        let deprecation = model
            .deprecate(&scan, Some("Hors d'usage."), Some("FR"))
            .expect("a retirement");

        assert_eq!(
            deprecation.note().expect("a note").language.as_deref(),
            Some("fr")
        );
    }

    #[test]
    fn an_empty_note_is_refused() {
        let statements = concept("wireless", "Wireless telegraphy");
        let (model, scan) = read(&statements, ex("wireless"), None);

        assert_eq!(
            model.deprecate(&scan, Some("   "), None),
            Err(DeprecationError::EmptyNote)
        );
    }

    #[test]
    fn a_concept_the_vocabulary_has_never_heard_of_is_refused() {
        let statements = concept("wireless", "Wireless telegraphy");
        let (model, scan) = read(&statements, ex("nothing"), None);

        assert_eq!(
            model.deprecate(&scan, None, None),
            Err(DeprecationError::NoSuchConcept {
                concept: ex("nothing")
            })
        );
    }

    #[test]
    fn a_concept_scheme_is_not_a_concept_and_is_refused() {
        let statements = vec![Statement::new(
            ex("scheme"),
            RDF_TYPE.to_owned(),
            Node::iri(SkosClass::ConceptScheme.iri()),
        )];
        let (model, scan) = read(&statements, ex("scheme"), None);

        assert_eq!(
            model.deprecate(&scan, None, None),
            Err(DeprecationError::NotAConcept {
                resource: ex("scheme")
            })
        );
    }

    #[test]
    fn retiring_what_is_already_retired_changes_nothing_and_is_refused() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(deprecated("wireless"));
        let (model, scan) = read(&statements, ex("wireless"), None);

        assert_eq!(
            model.deprecate(&scan, None, None),
            Err(DeprecationError::AlreadyDeprecated {
                concept: ex("wireless"),
                replaced_by: Vec::new(),
            })
        );
    }

    /// The workflow this exists for: retired when it went out of use, and the replacement only
    /// agreed on later.
    #[test]
    fn a_replacement_can_be_added_to_a_concept_already_retired() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.extend(concept("radio", "Radio"));
        statements.push(deprecated("wireless"));
        let (model, scan) = read(&statements, ex("wireless"), Some(ex("radio")));

        let deprecation = model.deprecate(&scan, None, None).expect("a retirement");

        assert!(!deprecation.marks());
        assert_eq!(
            deprecation.additions(),
            &[Statement::new(
                ex("wireless"),
                DCTERMS_IS_REPLACED_BY.to_owned(),
                ex("radio"),
            )]
        );
    }

    #[test]
    fn a_second_different_replacement_is_refused_because_this_removes_nothing() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.extend(concept("radio", "Radio"));
        statements.extend(concept("broadcasting", "Broadcasting"));
        statements.push(deprecated("wireless"));
        statements.push(Statement::new(
            ex("wireless"),
            DCTERMS_IS_REPLACED_BY.to_owned(),
            ex("radio"),
        ));
        let (model, scan) = read(&statements, ex("wireless"), Some(ex("broadcasting")));

        assert_eq!(
            model.deprecate(&scan, None, None),
            Err(DeprecationError::AlreadyReplaced {
                concept: ex("wireless"),
                by: vec![ex("radio")],
            })
        );
    }

    /// The same replacement again is a repeat, not a second one — and there is nothing left to
    /// propose, so it is the already-deprecated refusal rather than a candidate that adds nothing.
    #[test]
    fn recording_the_same_replacement_twice_proposes_nothing() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.extend(concept("radio", "Radio"));
        statements.push(deprecated("wireless"));
        statements.push(Statement::new(
            ex("wireless"),
            DCTERMS_IS_REPLACED_BY.to_owned(),
            ex("radio"),
        ));
        let (model, scan) = read(&statements, ex("wireless"), Some(ex("radio")));

        assert_eq!(
            model.deprecate(&scan, None, None),
            Err(DeprecationError::AlreadyDeprecated {
                concept: ex("wireless"),
                replaced_by: vec![ex("radio")],
            })
        );
    }

    #[test]
    fn a_concept_cannot_replace_itself() {
        let statements = concept("wireless", "Wireless telegraphy");
        let (model, scan) = read(&statements, ex("wireless"), Some(ex("wireless")));

        assert_eq!(
            model.deprecate(&scan, None, None),
            Err(DeprecationError::ReplacementIsTheConcept {
                concept: ex("wireless")
            })
        );
    }

    #[test]
    fn a_collection_offered_as_a_replacement_is_refused() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(Statement::new(
            ex("bundle"),
            RDF_TYPE.to_owned(),
            Node::iri(SkosClass::Collection.iri()),
        ));
        let (model, scan) = read(&statements, ex("wireless"), Some(ex("bundle")));

        assert_eq!(
            model.deprecate(&scan, None, None),
            Err(DeprecationError::ReplacementNotAConcept {
                resource: ex("bundle")
            })
        );
    }

    #[test]
    fn a_replacement_that_is_itself_retired_is_refused() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.extend(concept("radio", "Radio"));
        statements.push(deprecated("radio"));
        let (model, scan) = read(&statements, ex("wireless"), Some(ex("radio")));

        assert_eq!(
            model.deprecate(&scan, None, None),
            Err(DeprecationError::ReplacementDeprecated {
                replacement: ex("radio")
            })
        );
    }

    /// A replacement in another vocabulary is ordinary governance. The scan sees nothing about it
    /// and this proceeds; warning is the caller's job.
    #[test]
    fn a_replacement_this_vocabulary_has_never_heard_of_is_allowed() {
        let statements = concept("wireless", "Wireless telegraphy");
        let (model, scan) = read(
            &statements,
            ex("wireless"),
            Some(Node::iri("https://corporate.example/vocab/radio")),
        );

        let deprecation = model.deprecate(&scan, None, None).expect("a retirement");

        assert_eq!(
            deprecation.replacement(),
            Some(&Node::iri("https://corporate.example/vocab/radio"))
        );
    }

    #[test]
    fn what_is_still_attached_is_counted_and_named() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.extend(concept("telegraphy", "Telegraphy"));
        statements.extend(concept("morse", "Morse code"));
        statements.extend(concept("signals", "Signals"));
        statements.push(Statement::new(
            ex("wireless"),
            SemanticRelation::Broader.iri().to_owned(),
            ex("telegraphy"),
        ));
        statements.push(Statement::new(
            ex("morse"),
            SemanticRelation::Broader.iri().to_owned(),
            ex("wireless"),
        ));
        statements.push(Statement::new(
            ex("wireless"),
            SemanticRelation::Related.iri().to_owned(),
            ex("signals"),
        ));
        statements.push(Statement::new(
            ex("wireless"),
            MappingProperty::CloseMatch.iri().to_owned(),
            Node::iri("https://other.example/w"),
        ));
        statements.push(Statement::new(
            ex("wireless"),
            crate::model::SKOS_TOP_CONCEPT_OF.to_owned(),
            ex("scheme"),
        ));
        statements.push(Statement::new(
            ex("bundle"),
            RDF_TYPE.to_owned(),
            Node::iri(SkosClass::Collection.iri()),
        ));
        statements.push(Statement::new(
            ex("bundle"),
            SKOS_MEMBER.to_owned(),
            ex("wireless"),
        ));
        let (model, scan) = read(&statements, ex("wireless"), None);

        let deprecation = model.deprecate(&scan, None, None).expect("a retirement");
        let stranded = deprecation.stranded();

        assert!(!stranded.is_empty());
        assert_eq!(stranded.narrower, vec![ex("morse")]);
        assert_eq!(stranded.broader, vec![ex("telegraphy")]);
        assert_eq!(stranded.related, vec![ex("signals")]);
        assert_eq!(stranded.mapped_to, 1);
        assert_eq!(stranded.top_concept_of, vec![ex("scheme")]);
        assert_eq!(stranded.collections, vec![ex("bundle")]);
        // `morse broader wireless`, `bundle member wireless`, and the scheme's `hasTopConcept`
        // that S8 does not state — only the two the graph actually carries.
        assert_eq!(stranded.incoming, 2);
    }

    /// An ordered collection lists its members through `skos:memberList` and never through
    /// `skos:member`, so a check of one property alone would miss exactly the vocabularies that
    /// took the trouble to order theirs.
    #[test]
    fn an_ordered_collection_holding_the_concept_is_found() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(Statement::new(
            ex("ordered"),
            RDF_TYPE.to_owned(),
            Node::iri(SkosClass::OrderedCollection.iri()),
        ));
        statements.push(Statement::new(
            ex("ordered"),
            crate::model::SKOS_MEMBER_LIST.to_owned(),
            Node::blank("l0"),
        ));
        statements.push(Statement::new(
            Node::blank("l0"),
            crate::model::RDF_FIRST.to_owned(),
            ex("wireless"),
        ));
        statements.push(Statement::new(
            Node::blank("l0"),
            crate::model::RDF_REST.to_owned(),
            Node::iri(crate::model::RDF_NIL),
        ));
        let (model, scan) = read(&statements, ex("wireless"), None);

        let deprecation = model.deprecate(&scan, None, None).expect("a retirement");

        assert_eq!(deprecation.stranded().collections, vec![ex("ordered")]);
    }

    #[test]
    fn a_concept_with_nothing_attached_strands_nothing() {
        let statements = concept("wireless", "Wireless telegraphy");
        let (model, scan) = read(&statements, ex("wireless"), None);

        let deprecation = model.deprecate(&scan, None, None).expect("a retirement");

        assert!(deprecation.stranded().is_empty());
        assert_eq!(deprecation.stranded().incoming, 0);
    }

    /// Lenient on read: a vocabulary from another tool carrying a plain `"true"` is still saying
    /// the concept is deprecated, and reading it as "not deprecated" would propose a second marker.
    #[test]
    fn an_untyped_true_from_another_tool_still_reads_as_deprecated() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(Statement::new(
            ex("wireless"),
            OWL_DEPRECATED.to_owned(),
            Term::Literal(Literal {
                value: "true".to_owned(),
                language: None,
                datatype: XSD_STRING.to_owned(),
            }),
        ));
        let (model, scan) = read(&statements, ex("wireless"), None);

        assert!(scan.concept_deprecated());
        assert!(matches!(
            model.deprecate(&scan, None, None),
            Err(DeprecationError::AlreadyDeprecated { .. })
        ));
    }

    /// `owl:deprecated false` is a vocabulary saying the concept is current, which is not the same
    /// statement and must not read as one.
    #[test]
    fn an_explicit_false_does_not_read_as_deprecated() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(Statement::new(
            ex("wireless"),
            OWL_DEPRECATED.to_owned(),
            Term::Literal(Literal {
                value: "false".to_owned(),
                language: None,
                datatype: XSD_BOOLEAN.to_owned(),
            }),
        ));
        let (model, scan) = read(&statements, ex("wireless"), None);

        assert!(!scan.concept_deprecated());
        let deprecation = model.deprecate(&scan, None, None).expect("a retirement");
        assert!(deprecation.marks());
    }

    #[test]
    fn a_truncated_scan_cannot_establish_an_absence_and_is_refused() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(Statement::new(
            ex("wireless"),
            DCTERMS_IS_REPLACED_BY.to_owned(),
            ex("radio"),
        ));
        statements.push(Statement::new(
            ex("wireless"),
            DCTERMS_IS_REPLACED_BY.to_owned(),
            ex("broadcasting"),
        ));
        let model = CoreModel::from_statements(statements.iter().cloned());
        let mut scan = DeprecationScan::builder(ex("wireless"), None).with_bound(StatusBound {
            max_replacements: 1,
        });
        for statement in &statements {
            scan.push(statement.clone());
        }
        let scan = scan.build();

        assert!(!scan.is_complete());
        assert_eq!(
            model.deprecate(&scan, None, None),
            Err(DeprecationError::ScanTruncated {
                concept: ex("wireless")
            })
        );
    }

    /// One `skos:exactMatch` is also a `skos:closeMatch` by S42, so a count of statements reports
    /// one mapped resource as two. This is the case that found it.
    #[test]
    fn an_exact_match_is_one_mapped_resource_and_not_the_two_s42_licenses() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(Statement::new(
            ex("wireless"),
            MappingProperty::ExactMatch.iri().to_owned(),
            Node::iri("https://other.example/w"),
        ));
        let (model, scan) = read(&statements, ex("wireless"), None);

        let deprecation = model.deprecate(&scan, None, None).expect("a retirement");

        assert_eq!(deprecation.stranded().mapped_to, 1);
    }

    /// Five properties pointing at five resources is five.
    #[test]
    fn a_concept_is_counted_once_per_resource_it_is_mapped_to() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        let every = [
            MappingProperty::BroadMatch,
            MappingProperty::NarrowMatch,
            MappingProperty::RelatedMatch,
            MappingProperty::CloseMatch,
            MappingProperty::ExactMatch,
        ];
        for property in every {
            statements.push(Statement::new(
                ex("wireless"),
                property.iri().to_owned(),
                Node::iri(format!("https://other.example/{}", property.local_name())),
            ));
        }
        let (model, scan) = read(&statements, ex("wireless"), None);

        let deprecation = model.deprecate(&scan, None, None).expect("a retirement");

        assert_eq!(deprecation.stranded().mapped_to, every.len());
    }
}
