//! Splitting one concept into several: the parts a split creates, and the apportioning it refuses
//! to guess at.
//!
//! The third of `docs/BUILD-PLAN.md`'s bulk operations. Like [`relocate`](crate::relocate) and
//! [`merge`](crate::merge), nothing here writes: a [`Split`] is an *answer* — the statements a
//! split would add — computed against a [`CoreModel`] read a moment ago. The caller stages them as
//! a candidate; a human approves them.
//!
//! # Why a split adds and removes nothing, when a merge does both
//!
//! A merge has a determinate answer for every statement it touches: everything that mentioned the
//! duplicate now mentions the survivor, and there is exactly one place for each to go. A split has
//! the opposite property. The concept being split is one concept *because* somebody once thought
//! it was one; the whole reason to split it is that its labels, its narrower concepts, its
//! `skos:related` links and its notes belong to **different** things, and which one each belongs
//! to is precisely the editorial judgement that cannot be computed. A vocabulary tool that
//! apportioned them automatically would be inventing meaning.
//!
//! So this operation does the determinate half and names the rest. It creates the parts, gives
//! each one a preferred label and a position in the hierarchy, records that each was derived from
//! the concept, and **leaves the original entirely alone** — then reports every statement still
//! attached to it that a human now has to apportion, with the commands that do the apportioning
//! (`openbiz move` for a narrower concept, `openbiz merge` when a part turns out to be a duplicate
//! of something that already exists).
//!
//! That the original survives is not a limitation this works around. Retiring it is
//! `docs/BUILD-PLAN.md`'s *next* item — deprecation with replacement — and a split that also
//! deleted the concept would leave every reference to it dangling, which is exactly the state the
//! deprecation lifecycle exists to avoid.
//!
//! # Where the parts go, and why the caller must say
//!
//! Two placements, and there is no default:
//!
//! - [`Placement::Beside`] — each part takes the concept's own position: its broader concepts,
//!   stated in whichever direction the vocabulary states them, its concept schemes, and its place
//!   as a top concept where it is one. This is a **polysemy** split: `Banks` was two senses under
//!   one term, and `Banks (financial)` and `Banks (river)` stand where it stood.
//! - [`Placement::Below`] — each part is `skos:broader` the concept, which becomes their genus.
//!   This is a **granularity** split: `Vehicles` was too coarse, and `Cars` and `Trucks` go under
//!   it.
//!
//! Choosing wrongly produces a vocabulary that is consistent SKOS and says something false —
//! `Banks (river)` is not narrower than `Banks`, because homonymy is not hierarchy and §8.1's
//! `skos:broader` is a relation between *concepts* rather than between terms. Nothing downstream
//! reports it, which is the same argument [`relocate`](crate::relocate) makes about cycles, and it
//! is why the caller states the placement rather than inheriting a guess.
//!
//! # What it refuses
//!
//! - **A split into fewer than two parts**, which is not a split.
//! - **The concept not being a `skos:Concept`** the vocabulary knows about.
//! - **Two parts with the same IRI or the same label**, which is a typed-it-twice rather than a
//!   split, and a part whose IRI is the concept's own or already denotes something here.
//! - **A label with nothing in it.**
//! - **No language to label the parts in.** The parts are labelled in the language the concept's
//!   own preferred label is in, because that is the language the vocabulary is being authored in.
//!   A concept with preferred labels in more than one language, or with none at all, gets no
//!   guess: the caller says which.
//!
//! What it does **not** refuse is a part whose label something in this vocabulary already carries.
//! `CLAUDE.md` §1.7 puts reuse above creation and that is a reason to *tell* the operator — loudly,
//! before anything else in the report — but not to refuse: a large vocabulary has legitimate
//! homonyms, and this is the same position [`mint`](crate::mint) takes for the same reason. Nothing
//! here reaches the vocabulary without a human reading the report first.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::labels::{LabelKind, LexicalLabel, RDF_LANG_STRING, SKOS_PREF_LABEL, XSD_STRING};
use crate::model::{
    CoreModel, Literal, Node, SkosClass, Statement, Term, RDF_TYPE, SKOS_IN_SCHEME,
    SKOS_TOP_CONCEPT_OF,
};
use crate::relations::SemanticRelation;

/// `prov:wasDerivedFrom` — the statement that records where a part came from.
///
/// PROV-O is the vocabulary `CLAUDE.md` §2 commits to for provenance, and this is the one thing a
/// split knows that no later reader could reconstruct: that these concepts exist because that one
/// was divided. It goes in the **vocabulary**, not in OpenBiz's own graphs, because it is a
/// statement about the concept rather than bookkeeping about the edit — so it survives an export
/// and answers "why does this concept exist?" in a tool that has never heard of OpenBiz.
pub const PROV_WAS_DERIVED_FROM: &str = "http://www.w3.org/ns/prov#wasDerivedFrom";

/// Where the parts of a split are placed relative to the concept they came from.
///
/// There is deliberately no `Default`. See the module note: both readings are ordinary, and the
/// wrong one is consistent SKOS that says something false.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Each part takes the concept's own position — its broader concepts and its schemes.
    Beside,
    /// Each part is `skos:broader` the concept, which stays as their genus.
    Below,
}

impl Placement {
    /// The word the command line uses for this placement.
    pub fn as_str(self) -> &'static str {
        match self {
            Placement::Beside => "beside",
            Placement::Below => "below",
        }
    }

    /// The placement a word names, or `None`.
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "beside" => Some(Placement::Beside),
            "below" => Some(Placement::Below),
            _ => None,
        }
    }
}

impl fmt::Display for Placement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One part the caller is asking for: an IRI it has minted, and what the part is to be called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartRequest {
    /// The IRI the part will have. Minted by the caller under the vocabulary's own policy.
    pub iri: Node,
    /// The lexical form of the part's preferred label. The language is resolved by the split.
    pub label: String,
}

/// One part a split would create.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    /// The part's IRI.
    pub iri: Node,
    /// Its preferred label, in the resolved language.
    pub label: LexicalLabel,
    /// Concepts in this vocabulary that already carry this exact preferred label.
    ///
    /// Not a refusal — see the module note — but the first thing the report says, because
    /// `CLAUDE.md` §1.7 ranks reusing one of these above creating another.
    pub already_called_that: Vec<Node>,
}

/// What is still attached to the concept after the split, which a human now apportions.
///
/// Every one of these is a statement this operation deliberately did not touch. The counts are
/// what makes the report honest: a split that said nothing about them would read as if the work
/// were finished.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Unapportioned {
    /// The concepts directly below it, each of which belongs under one part or another.
    pub narrower: Vec<Node>,
    /// The concepts it is associatively linked to.
    pub related: Vec<Node>,
    /// How many mapping links it carries into other vocabularies.
    pub mappings: usize,
    /// How many documentation notes it carries.
    pub notes: usize,
    /// How many lexical labels it carries, of every kind.
    pub labels: usize,
}

impl Unapportioned {
    /// Whether anything at all is left needing a decision.
    pub fn is_empty(&self) -> bool {
        self.narrower.is_empty()
            && self.related.is_empty()
            && self.mappings == 0
            && self.notes == 0
            && self.labels == 0
    }
}

/// What splitting one concept into several would add to a vocabulary.
///
/// Produced by [`CoreModel::split`] and applied by nobody: the statements are a proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split {
    concept: Node,
    placement: Placement,
    language: Option<String>,
    parts: Vec<Part>,
    additions: Vec<Statement>,
    parents: Vec<Node>,
    schemes: Vec<Node>,
    top_concept_of: Vec<Node>,
    unapportioned: Unapportioned,
}

impl Split {
    /// The concept being split, which this change leaves exactly as it found it.
    pub fn concept(&self) -> &Node {
        &self.concept
    }

    /// Where the parts were placed.
    pub fn placement(&self) -> Placement {
        self.placement
    }

    /// The language the parts' labels are in, or `None` for an untagged label.
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// The parts, in the order the caller asked for them.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// The statements to add. A split removes nothing.
    pub fn additions(&self) -> &[Statement] {
        &self.additions
    }

    /// The broader concepts the parts inherited, which is empty under [`Placement::Below`].
    pub fn parents(&self) -> &[Node] {
        &self.parents
    }

    /// The concept schemes the parts were put in.
    pub fn schemes(&self) -> &[Node] {
        &self.schemes
    }

    /// The schemes the parts were made top concepts of, which is empty under [`Placement::Below`].
    pub fn top_concept_of(&self) -> &[Node] {
        &self.top_concept_of
    }

    /// What is still attached to the original and needs a human's judgement.
    pub fn unapportioned(&self) -> &Unapportioned {
        &self.unapportioned
    }
}

/// Nothing could be split, and this says exactly what stopped it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SplitError {
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
    /// Fewer than two parts, which is not a split.
    TooFewParts {
        /// How many were given.
        given: usize,
    },
    /// A part was given the IRI of the concept being split.
    PartIsTheConcept {
        /// The concept.
        concept: Node,
    },
    /// Two parts were given the same IRI.
    RepeatedPartIri {
        /// The IRI given twice.
        iri: Node,
    },
    /// Two parts were given the same label.
    RepeatedPartLabel {
        /// The label given twice.
        label: String,
    },
    /// A part's IRI already denotes something in this vocabulary.
    PartIriInUse {
        /// The IRI that is taken.
        iri: Node,
    },
    /// A label with no lexical form to it.
    UnusableLabel,
    /// The concept has no preferred label to take a language from, and none was given.
    NoLanguage {
        /// The concept.
        concept: Node,
    },
    /// The concept's preferred labels are in more than one language, and none was given.
    AmbiguousLanguage {
        /// The concept.
        concept: Node,
        /// The languages it is labelled in, `None` for an untagged label.
        languages: Vec<Option<String>>,
    },
}

impl fmt::Display for SplitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SplitError::NoSuchConcept { concept } => write!(
                f,
                "this vocabulary says nothing about {concept}, so there is nothing to split"
            ),
            SplitError::NotAConcept { resource } => write!(
                f,
                "{resource} is not a skos:Concept, and splitting one is the only thing this does"
            ),
            SplitError::TooFewParts { given } => write!(
                f,
                "a split needs at least two parts and {} given; \
                 splitting a concept into one part changes nothing",
                match given {
                    0 => "none were".to_owned(),
                    1 => "one was".to_owned(),
                    many => format!("{many} were"),
                }
            ),
            SplitError::PartIsTheConcept { concept } => write!(
                f,
                "{concept} is the concept being split, so it cannot also be one of the parts"
            ),
            SplitError::RepeatedPartIri { iri } => {
                write!(f, "{iri} was given as more than one part")
            }
            SplitError::RepeatedPartLabel { label } => write!(
                f,
                "{label:?} was given as more than one part; \
                 two concepts with one name is the problem a split is meant to end, not start"
            ),
            SplitError::PartIriInUse { iri } => write!(
                f,
                "{iri} already denotes something in this vocabulary, \
                 so a new concept must not be given it"
            ),
            SplitError::UnusableLabel => {
                write!(f, "a part was given a label with nothing in it")
            }
            SplitError::NoLanguage { concept } => write!(
                f,
                "{concept} has no preferred label, so there is no language to label the parts in; \
                 name one"
            ),
            SplitError::AmbiguousLanguage { concept, languages } => {
                write!(
                    f,
                    "{concept} has a preferred label in {} languages and the parts get one label \
                     each, so which language they are in has to be said:",
                    languages.len()
                )?;
                for language in languages {
                    match language {
                        Some(tag) => write!(f, " {tag}")?,
                        None => write!(f, " (untagged)")?,
                    }
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for SplitError {}

impl CoreModel {
    /// The statements that would divide `concept` into `parts`.
    ///
    /// `language` overrides the language the parts' labels are written in; without it the split
    /// uses the one language the concept's own preferred label is in, and refuses rather than
    /// guessing when there is not exactly one.
    ///
    /// Nothing is written, and **nothing about `concept` is removed**: the answer is a [`Split`]
    /// holding statements to add and a list of what a human still has to apportion. Read the
    /// module note for why that division is the honest one.
    pub fn split(
        &self,
        concept: &Node,
        parts: &[PartRequest],
        placement: Placement,
        language: Option<&str>,
    ) -> Result<Split, SplitError> {
        let Some(resource) = self.resource(concept) else {
            return Err(SplitError::NoSuchConcept {
                concept: concept.clone(),
            });
        };
        if !resource.is_a(SkosClass::Concept) {
            return Err(SplitError::NotAConcept {
                resource: concept.clone(),
            });
        }
        if parts.len() < 2 {
            return Err(SplitError::TooFewParts { given: parts.len() });
        }

        let mut seen_iris: BTreeSet<&Node> = BTreeSet::new();
        let mut seen_labels: BTreeSet<&str> = BTreeSet::new();
        for part in parts {
            if &part.iri == concept {
                return Err(SplitError::PartIsTheConcept {
                    concept: concept.clone(),
                });
            }
            if !seen_iris.insert(&part.iri) {
                return Err(SplitError::RepeatedPartIri {
                    iri: part.iri.clone(),
                });
            }
            if part.label.trim().is_empty() {
                return Err(SplitError::UnusableLabel);
            }
            if !seen_labels.insert(part.label.as_str()) {
                return Err(SplitError::RepeatedPartLabel {
                    label: part.label.clone(),
                });
            }
            // The store checks this against the whole store when it mints; this checks it against
            // the vocabulary in front of us, which is the one that would be corrupted.
            if self.resource(&part.iri).is_some() {
                return Err(SplitError::PartIriInUse {
                    iri: part.iri.clone(),
                });
            }
        }

        let language = match language {
            Some(given) => {
                let given = given.trim();
                if given.is_empty() {
                    return Err(SplitError::NoLanguage {
                        concept: concept.clone(),
                    });
                }
                Some(given.to_ascii_lowercase())
            }
            None => {
                let languages: Vec<Option<String>> = resource
                    .labels_of(LabelKind::Preferred)
                    .map(|label| label.language.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                match languages.len() {
                    0 => {
                        return Err(SplitError::NoLanguage {
                            concept: concept.clone(),
                        })
                    }
                    1 => languages[0].clone(),
                    _ => {
                        return Err(SplitError::AmbiguousLanguage {
                            concept: concept.clone(),
                            languages,
                        })
                    }
                }
            }
        };

        let resolved: Vec<Part> = parts
            .iter()
            .map(|part| {
                let label = LexicalLabel {
                    language: language.clone(),
                    text: part.label.clone(),
                };
                Part {
                    already_called_that: self.already_called(&label),
                    iri: part.iri.clone(),
                    label,
                }
            })
            .collect();

        let parents: Vec<Node> = match placement {
            Placement::Beside => resource
                .relations(SemanticRelation::Broader)
                .map(|links| links.keys().cloned().collect())
                .unwrap_or_default(),
            Placement::Below => Vec::new(),
        };
        let schemes: Vec<Node> = resource.in_schemes().iter().cloned().collect();
        let top_concept_of: Vec<Node> = match placement {
            Placement::Beside => resource.top_concept_of().iter().cloned().collect(),
            // A part below the concept is below a top concept, so it is not one itself.
            Placement::Below => Vec::new(),
        };

        let mut additions = Vec::new();
        for part in &resolved {
            additions.push(Statement::new(
                part.iri.clone(),
                RDF_TYPE.to_owned(),
                Node::iri(SkosClass::Concept.iri()),
            ));
            additions.push(Statement::new(
                part.iri.clone(),
                SKOS_PREF_LABEL.to_owned(),
                Term::Literal(literal(&part.label)),
            ));
            additions.push(Statement::new(
                part.iri.clone(),
                PROV_WAS_DERIVED_FROM.to_owned(),
                concept.clone(),
            ));
            for scheme in &schemes {
                additions.push(Statement::new(
                    part.iri.clone(),
                    SKOS_IN_SCHEME.to_owned(),
                    scheme.clone(),
                ));
            }
            for scheme in &top_concept_of {
                // `skos:topConceptOf` and not its converse `skos:hasTopConcept`, because the model
                // closes S8 on read and so cannot say which of the two the graph asserted. That is
                // a real gap and it is recorded in `docs/UNTESTED.md`; the subject-first direction
                // is chosen so a part reads the way `skos:broader` does everywhere else here.
                additions.push(Statement::new(
                    part.iri.clone(),
                    SKOS_TOP_CONCEPT_OF.to_owned(),
                    scheme.clone(),
                ));
            }
            match placement {
                Placement::Beside => {
                    for parent in &parents {
                        // The vocabulary's own habit, kept: a thesaurus authored downwards in
                        // `skos:narrower` gets parts stated the same way, so the split does not
                        // quietly change how the file reads. S25 makes the two equivalent, which
                        // is why nothing downstream would have caught it.
                        let stated = self.stated_directions(concept, parent);
                        if stated.broader || !stated.narrower {
                            additions.push(Statement::new(
                                part.iri.clone(),
                                SemanticRelation::Broader.iri(),
                                parent.clone(),
                            ));
                        }
                        if stated.narrower {
                            additions.push(Statement::new(
                                parent.clone(),
                                SemanticRelation::Narrower.iri(),
                                part.iri.clone(),
                            ));
                        }
                    }
                }
                Placement::Below => additions.push(Statement::new(
                    part.iri.clone(),
                    SemanticRelation::Broader.iri(),
                    concept.clone(),
                )),
            }
        }

        let unapportioned = Unapportioned {
            narrower: resource
                .relations(SemanticRelation::Narrower)
                .map(|links| links.keys().cloned().collect())
                .unwrap_or_default(),
            related: resource
                .relations(SemanticRelation::Related)
                .map(|links| links.keys().cloned().collect())
                .unwrap_or_default(),
            mappings: resource
                .mappings()
                .values()
                .map(BTreeMap::len)
                .sum::<usize>(),
            notes: resource.notes().len(),
            labels: resource.labels().len(),
        };

        Ok(Split {
            concept: concept.clone(),
            placement,
            language,
            parts: resolved,
            additions,
            parents,
            schemes,
            top_concept_of,
            unapportioned,
        })
    }

    /// The concepts in this vocabulary already carrying `label` as a preferred label.
    fn already_called(&self, label: &LexicalLabel) -> Vec<Node> {
        self.instances_of(SkosClass::Concept)
            .filter(|(_, resource)| {
                resource
                    .labels_of(LabelKind::Preferred)
                    .any(|carried| carried == label)
            })
            .map(|(node, _)| node.clone())
            .collect()
    }
}

/// The RDF literal behind a label, with the datatype RDF 1.1 gives it.
fn literal(label: &LexicalLabel) -> Literal {
    match &label.language {
        Some(tag) => Literal {
            value: label.text.clone(),
            language: Some(tag.clone()),
            datatype: RDF_LANG_STRING.to_owned(),
        },
        None => Literal {
            value: label.text.clone(),
            language: None,
            datatype: XSD_STRING.to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ns;

    fn ex(name: &str) -> Node {
        Node::iri(format!("http://example.org/{name}"))
    }

    fn s(subject: &Node, predicate: &str, object: &Node) -> Statement {
        Statement::new(subject.clone(), predicate.to_owned(), object.clone())
    }

    /// `<name> a skos:Concept`.
    fn concept(name: &Node) -> Statement {
        Statement::new(
            name.clone(),
            RDF_TYPE.to_owned(),
            Node::iri(SkosClass::Concept.iri()),
        )
    }

    /// `<name> skos:prefLabel "text"@language`, or untagged when `language` is `None`.
    fn pref(name: &Node, text: &str, language: Option<&str>) -> Statement {
        labelled(name, SKOS_PREF_LABEL, text, language)
    }

    fn labelled(name: &Node, property: &str, text: &str, language: Option<&str>) -> Statement {
        Statement::new(
            name.clone(),
            property.to_owned(),
            Term::Literal(match language {
                Some(tag) => Literal {
                    value: text.to_owned(),
                    language: Some(tag.to_owned()),
                    datatype: RDF_LANG_STRING.to_owned(),
                },
                None => Literal {
                    value: text.to_owned(),
                    language: None,
                    datatype: XSD_STRING.to_owned(),
                },
            }),
        )
    }

    fn skos(local: &str) -> String {
        format!("{}{local}", ns::SKOS)
    }

    fn request(iri: &Node, label: &str) -> PartRequest {
        PartRequest {
            iri: iri.clone(),
            label: label.to_owned(),
        }
    }

    /// `Banks` under `Institutions`, in one scheme, labelled in English.
    fn banks() -> (CoreModel, Node) {
        let banks = ex("banks");
        let model = CoreModel::from_statements([
            concept(&banks),
            concept(&ex("institutions")),
            pref(&banks, "Banks", Some("en")),
            s(&banks, &skos("broader"), &ex("institutions")),
            s(&banks, &skos("inScheme"), &ex("scheme")),
        ]);
        (model, banks)
    }

    fn two_parts() -> Vec<PartRequest> {
        vec![
            request(&ex("banks-financial"), "Banks (financial)"),
            request(&ex("banks-river"), "Banks (river)"),
        ]
    }

    #[test]
    fn beside_stands_each_part_where_the_concept_stands() {
        let (model, banks) = banks();
        let split = model
            .split(&banks, &two_parts(), Placement::Beside, None)
            .expect("a two-part split of a concept with one parent");

        assert_eq!(split.parents(), [ex("institutions")]);
        assert_eq!(split.schemes(), [ex("scheme")]);
        for part in split.parts() {
            assert!(
                split
                    .additions()
                    .contains(&s(&part.iri, &skos("broader"), &ex("institutions"))),
                "{} should be under the concept's own broader concept",
                part.iri
            );
            assert!(split
                .additions()
                .contains(&s(&part.iri, &skos("inScheme"), &ex("scheme"))));
            assert!(split.additions().contains(&concept(&part.iri)));
        }
    }

    /// S25 makes the two directions equivalent, so nothing downstream would report a split that
    /// silently switched a downward-authored thesaurus to `skos:broader`. The file would just
    /// start reading differently, which is the sort of change a reviewer blames on the tool.
    #[test]
    fn beside_states_the_hierarchy_the_way_the_vocabulary_states_it() {
        let banks = ex("banks");
        let model = CoreModel::from_statements([
            concept(&banks),
            concept(&ex("institutions")),
            pref(&banks, "Banks", Some("en")),
            s(&ex("institutions"), &skos("narrower"), &banks),
        ]);
        let split = model
            .split(&banks, &two_parts(), Placement::Beside, None)
            .expect("a split of a downward-authored hierarchy");

        for part in split.parts() {
            assert!(
                split
                    .additions()
                    .contains(&s(&ex("institutions"), &skos("narrower"), &part.iri)),
                "{} should be stated downwards, as this vocabulary states its hierarchy",
                part.iri
            );
            assert!(
                !split
                    .additions()
                    .contains(&s(&part.iri, &skos("broader"), &ex("institutions"))),
                "and not upwards as well, which would say the same thing twice"
            );
        }
    }

    #[test]
    fn below_makes_the_concept_the_genus_of_every_part() {
        let (model, banks) = banks();
        let split = model
            .split(&banks, &two_parts(), Placement::Below, None)
            .expect("a two-part split below the concept");

        assert!(
            split.parents().is_empty(),
            "the parts are under the concept"
        );
        for part in split.parts() {
            assert!(split
                .additions()
                .contains(&s(&part.iri, &skos("broader"), &banks)));
            assert!(
                !split
                    .additions()
                    .contains(&s(&part.iri, &skos("broader"), &ex("institutions"))),
                "a part below the concept does not also stand beside it"
            );
        }
    }

    #[test]
    fn beside_makes_each_part_a_top_concept_when_the_concept_is_one() {
        let banks = ex("banks");
        let model = CoreModel::from_statements([
            concept(&banks),
            pref(&banks, "Banks", Some("en")),
            s(&banks, &skos("topConceptOf"), &ex("scheme")),
        ]);
        let split = model
            .split(&banks, &two_parts(), Placement::Beside, None)
            .expect("a split of a top concept");

        assert_eq!(split.top_concept_of(), [ex("scheme")]);
        for part in split.parts() {
            assert!(split.additions().contains(&s(
                &part.iri,
                &skos("topConceptOf"),
                &ex("scheme")
            )));
        }
    }

    /// A part below the concept is below a top concept, which is the one thing a top concept is
    /// defined not to be.
    #[test]
    fn below_never_makes_a_part_a_top_concept() {
        let banks = ex("banks");
        let model = CoreModel::from_statements([
            concept(&banks),
            pref(&banks, "Banks", Some("en")),
            s(&banks, &skos("topConceptOf"), &ex("scheme")),
        ]);
        let split = model
            .split(&banks, &two_parts(), Placement::Below, None)
            .expect("a split below a top concept");

        assert!(split.top_concept_of().is_empty());
        assert!(!split
            .additions()
            .iter()
            .any(|statement| statement.predicate == skos("topConceptOf")));
        // It is still in the scheme, which S7 entails for the concept it is under anyway.
        for part in split.parts() {
            assert!(split
                .additions()
                .contains(&s(&part.iri, &skos("inScheme"), &ex("scheme"))));
        }
    }

    #[test]
    fn every_part_records_the_concept_it_was_derived_from() {
        let (model, banks) = banks();
        let split = model
            .split(&banks, &two_parts(), Placement::Beside, None)
            .expect("a two-part split");

        for part in split.parts() {
            assert!(
                split
                    .additions()
                    .contains(&s(&part.iri, PROV_WAS_DERIVED_FROM, &banks)),
                "{} should say where it came from",
                part.iri
            );
        }
    }

    #[test]
    fn the_derivation_property_is_the_one_prov_o_defines() {
        assert_eq!(
            PROV_WAS_DERIVED_FROM,
            format!("{}wasDerivedFrom", ns::PROV),
            "the audit trail is PROV-O, and a near-miss IRI is a vocabulary nobody else reads"
        );
    }

    /// The concept is left exactly as it was found: this operation adds and never removes, and
    /// nothing it adds is a statement *about* the concept. Retiring it is the deprecation item.
    #[test]
    fn the_concept_being_split_is_left_alone() {
        let (model, banks) = banks();
        let split = model
            .split(&banks, &two_parts(), Placement::Beside, None)
            .expect("a two-part split");

        assert!(
            !split
                .additions()
                .iter()
                .any(|statement| statement.subject == banks),
            "a split says nothing new about the concept it divides"
        );
    }

    #[test]
    fn the_parts_are_labelled_in_the_language_the_concept_is_labelled_in() {
        let banks = ex("banks");
        let model = CoreModel::from_statements([
            concept(&banks),
            pref(&banks, "Rives", Some("FR")),
            // An alternative label in another language is not what names the parts.
            labelled(&banks, &skos("altLabel"), "Banks", Some("en")),
        ]);
        let split = model
            .split(&banks, &two_parts(), Placement::Below, None)
            .expect("a split of a French-labelled concept");

        assert_eq!(
            split.language(),
            Some("fr"),
            "and lower-cased, as RDF 1.1 has it"
        );
        for part in split.parts() {
            assert_eq!(part.label.language.as_deref(), Some("fr"));
        }
    }

    #[test]
    fn an_untagged_preferred_label_gives_the_parts_untagged_labels() {
        let banks = ex("banks");
        let model = CoreModel::from_statements([concept(&banks), pref(&banks, "Banks", None)]);
        let split = model
            .split(&banks, &two_parts(), Placement::Below, None)
            .expect("a split of an untagged concept");

        assert_eq!(split.language(), None);
        assert!(split.additions().contains(&pref(
            &ex("banks-financial"),
            "Banks (financial)",
            None
        )));
    }

    #[test]
    fn a_concept_labelled_in_two_languages_refuses_rather_than_choosing_one() {
        let banks = ex("banks");
        let model = CoreModel::from_statements([
            concept(&banks),
            pref(&banks, "Banks", Some("en")),
            pref(&banks, "Rives", Some("fr")),
        ]);
        let error = model
            .split(&banks, &two_parts(), Placement::Below, None)
            .expect_err("two languages and one label per part is a question, not a default");

        match error {
            SplitError::AmbiguousLanguage { languages, .. } => assert_eq!(
                languages,
                [Some("en".to_owned()), Some("fr".to_owned())],
                "and the refusal names them, so the operator can pick"
            ),
            other => panic!("expected an ambiguous language, got {other}"),
        }
    }

    #[test]
    fn a_named_language_overrides_the_concepts_own_and_is_lower_cased() {
        let banks = ex("banks");
        let model = CoreModel::from_statements([
            concept(&banks),
            pref(&banks, "Banks", Some("en")),
            pref(&banks, "Rives", Some("fr")),
        ]);
        let split = model
            .split(&banks, &two_parts(), Placement::Below, Some("DE"))
            .expect("a named language answers the ambiguity");

        assert_eq!(split.language(), Some("de"));
    }

    #[test]
    fn a_concept_with_no_preferred_label_refuses_rather_than_labelling_untagged() {
        let banks = ex("banks");
        let model = CoreModel::from_statements([concept(&banks)]);
        let error = model
            .split(&banks, &two_parts(), Placement::Below, None)
            .expect_err("nothing to take a language from");

        assert!(
            matches!(error, SplitError::NoLanguage { .. }),
            "got {error}"
        );
    }

    #[test]
    fn a_split_into_fewer_than_two_parts_is_refused() {
        let (model, banks) = banks();
        for parts in [Vec::new(), vec![request(&ex("one"), "One")]] {
            let given = parts.len();
            let error = model
                .split(&banks, &parts, Placement::Beside, None)
                .expect_err("one part is not a division");
            assert_eq!(error, SplitError::TooFewParts { given });
        }
    }

    #[test]
    fn a_part_that_is_the_concept_itself_is_refused() {
        let (model, banks) = banks();
        let parts = vec![request(&banks, "Banks"), request(&ex("river"), "Rivers")];
        let error = model
            .split(&banks, &parts, Placement::Beside, None)
            .expect_err("the concept cannot be one of its own parts");

        assert_eq!(
            error,
            SplitError::PartIsTheConcept {
                concept: banks.clone()
            }
        );
    }

    #[test]
    fn the_same_part_twice_is_refused_by_iri_and_by_label() {
        let (model, banks) = banks();

        let repeated_iri = vec![request(&ex("a"), "One"), request(&ex("a"), "Two")];
        assert_eq!(
            model
                .split(&banks, &repeated_iri, Placement::Beside, None)
                .expect_err("one IRI cannot denote two parts"),
            SplitError::RepeatedPartIri { iri: ex("a") }
        );

        let repeated_label = vec![request(&ex("a"), "One"), request(&ex("b"), "One")];
        assert_eq!(
            model
                .split(&banks, &repeated_label, Placement::Beside, None)
                .expect_err("two parts with one name is what a split is meant to end"),
            SplitError::RepeatedPartLabel {
                label: "One".to_owned()
            }
        );
    }

    #[test]
    fn a_part_iri_this_vocabulary_already_uses_is_refused() {
        let (model, banks) = banks();
        let parts = vec![
            request(&ex("institutions"), "Institutions"),
            request(&ex("river"), "Rivers"),
        ];
        let error = model
            .split(&banks, &parts, Placement::Beside, None)
            .expect_err("a new concept must not take an IRI something else denotes");

        assert_eq!(
            error,
            SplitError::PartIriInUse {
                iri: ex("institutions")
            }
        );
    }

    #[test]
    fn a_label_with_nothing_in_it_is_refused() {
        let (model, banks) = banks();
        let parts = vec![request(&ex("a"), "   "), request(&ex("b"), "Rivers")];
        assert_eq!(
            model
                .split(&banks, &parts, Placement::Beside, None)
                .expect_err("a concept with a blank name is unfindable"),
            SplitError::UnusableLabel
        );
    }

    #[test]
    fn splitting_something_that_is_not_a_concept_is_refused() {
        let collection = ex("collection");
        let model = CoreModel::from_statements([
            Statement::new(
                collection.clone(),
                RDF_TYPE.to_owned(),
                Node::iri(SkosClass::Collection.iri()),
            ),
            pref(&collection, "A collection", Some("en")),
        ]);
        assert_eq!(
            model
                .split(&collection, &two_parts(), Placement::Beside, None)
                .expect_err("a collection is not a concept"),
            SplitError::NotAConcept {
                resource: collection.clone()
            }
        );

        let unknown = ex("nothing");
        assert_eq!(
            model
                .split(&unknown, &two_parts(), Placement::Beside, None)
                .expect_err("and an IRI the vocabulary never mentions is not one either"),
            SplitError::NoSuchConcept { concept: unknown }
        );
    }

    /// The point of the report: everything the split deliberately did not decide.
    #[test]
    fn what_is_left_needing_a_human_is_counted() {
        let banks = ex("banks");
        let model = CoreModel::from_statements([
            concept(&banks),
            concept(&ex("child-a")),
            concept(&ex("child-b")),
            concept(&ex("money")),
            pref(&banks, "Banks", Some("en")),
            labelled(&banks, &skos("altLabel"), "Bank", Some("en")),
            s(&banks, &skos("narrower"), &ex("child-a")),
            s(&banks, &skos("narrower"), &ex("child-b")),
            s(&banks, &skos("related"), &ex("money")),
            s(&banks, &skos("closeMatch"), &ex("other:banks")),
            labelled(&banks, &skos("scopeNote"), "Both senses.", Some("en")),
        ]);
        let split = model
            .split(&banks, &two_parts(), Placement::Beside, None)
            .expect("a split of a well-furnished concept");

        let left = split.unapportioned();
        assert!(!left.is_empty());
        assert_eq!(left.narrower, [ex("child-a"), ex("child-b")]);
        assert_eq!(left.related, [ex("money")]);
        assert_eq!(left.mappings, 1);
        assert_eq!(left.notes, 1);
        assert_eq!(left.labels, 2, "the preferred one and the alternative one");
    }

    #[test]
    fn a_concept_with_nothing_hanging_off_it_leaves_nothing_to_apportion() {
        let banks = ex("banks");
        let model =
            CoreModel::from_statements([concept(&banks), pref(&banks, "Banks", Some("en"))]);
        let split = model
            .split(&banks, &two_parts(), Placement::Below, None)
            .expect("a bare concept splits cleanly");

        assert_eq!(
            split.unapportioned().labels,
            1,
            "its own preferred label is still its own"
        );
        assert!(split.unapportioned().narrower.is_empty());
    }

    /// `CLAUDE.md` §1.7 ranks reuse above creation, so this is said — loudly — and not refused.
    /// A large vocabulary has legitimate homonyms and refusing would make them unauthorable.
    #[test]
    fn a_part_named_what_something_here_is_already_named_is_reported_not_refused() {
        let banks = ex("banks");
        let model = CoreModel::from_statements([
            concept(&banks),
            concept(&ex("rivers")),
            pref(&banks, "Banks", Some("en")),
            pref(&ex("rivers"), "Banks (river)", Some("en")),
        ]);
        let split = model
            .split(&banks, &two_parts(), Placement::Below, None)
            .expect("a colliding label is a warning, not a wall");

        let river = split
            .parts()
            .iter()
            .find(|part| part.iri == ex("banks-river"))
            .expect("the river part");
        assert_eq!(river.already_called_that, [ex("rivers")]);

        let financial = split
            .parts()
            .iter()
            .find(|part| part.iri == ex("banks-financial"))
            .expect("the financial part");
        assert!(financial.already_called_that.is_empty());
    }

    #[test]
    fn a_placement_round_trips_through_the_word_the_command_line_uses() {
        for placement in [Placement::Beside, Placement::Below] {
            assert_eq!(Placement::from_word(placement.as_str()), Some(placement));
        }
        assert_eq!(Placement::from_word("under"), None);
    }
}
