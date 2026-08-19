//! The local store as a discovery source — the one source every deployment has.
//!
//! # Why this is the first provider, and why it is not the last
//!
//! `adr/0003` §7: federation degrades to nothing, and an air-gapped deployment keeps local and
//! peer discovery. So the local source is the one that must always work, and it is the baseline
//! everything else is added to. It is also, on its own, enough to prevent the commonest form of
//! the failure: two curators in the same organisation, working in two vocabularies in one store,
//! each creating the concept the other already has.
//!
//! # This crate never opens the store
//!
//! The provider reads through [`LocalCorpus`], which the composition root implements over the
//! real store. Two things follow. The store's own conversion from RDF statements to the SKOS
//! model stays in one place instead of being copied here, and this crate keeps no dependency that
//! could reach a database or a network — so what a source may do is a decision made at the
//! boundary, where `CLAUDE.md` §3 puts it, rather than a property of a crate nobody re-reads.
//!
//! The corpus is read **one part at a time**. A store holding forty vocabularies is not held in
//! memory at once; each model is built, searched, and dropped. That bounds what discovery costs
//! to the largest single vocabulary rather than to the store, which matters because this runs on
//! the creation path, in front of somebody who is waiting.

use openbiz_skos::{CoreModel, LabelQuery, Resource};

use crate::{DiscoveryProvider, Match, SourceAnswer, Unavailable};

/// What kind of part of the store this is.
///
/// Three cases and not a boolean, because the report says something different about each and
/// because the alternative — reading the kind back out of the description string — is a rule
/// nobody can see from the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartKind {
    /// The vocabulary the new concept would go into.
    ///
    /// A match here is a duplicate about to be created inside one vocabulary; a match anywhere
    /// else is something to reuse, map to, or extend (`adr/0003` §3).
    Home,
    /// Another vocabulary in the same store.
    Vocabulary,
    /// A change staged against a vocabulary and still waiting for a decision.
    ///
    /// Counted apart from the vocabularies: a label that exists only in a change nobody has
    /// approved is a different fact from one that is in the vocabulary, and a curator deciding
    /// whether to reuse a concept needs to know which they are looking at.
    Pending,
}

/// One part of the local store that can be searched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusPart {
    /// Where the corpus reads this part from — its own identifier for it, a named-graph IRI in
    /// the case of the real store.
    ///
    /// Never interpreted here. Discovery hands it back to the corpus that produced it and does
    /// nothing else with it, which is what keeps the store's addressing out of this crate.
    pub at: String,
    /// What this part is called in a report — "this vocabulary", "the vocabulary <iri>",
    /// "candidate 7, which is waiting for a decision".
    pub within: String,
    /// Which kind of part it is.
    pub kind: PartKind,
}

impl CorpusPart {
    /// A part of the store, of a given kind.
    pub fn new(at: impl Into<String>, within: impl Into<String>, kind: PartKind) -> Self {
        CorpusPart {
            at: at.into(),
            within: within.into(),
            kind,
        }
    }

    /// Another vocabulary in the store.
    pub fn elsewhere(at: impl Into<String>, within: impl Into<String>) -> Self {
        CorpusPart::new(at, within, PartKind::Vocabulary)
    }

    /// The vocabulary the new concept would go into.
    pub fn home(at: impl Into<String>, within: impl Into<String>) -> Self {
        CorpusPart::new(at, within, PartKind::Home)
    }

    /// A change waiting for a decision.
    pub fn pending(at: impl Into<String>, within: impl Into<String>) -> Self {
        CorpusPart::new(at, within, PartKind::Pending)
    }

    /// Whether this is the vocabulary the new concept would go into.
    pub fn is_home(&self) -> bool {
        self.kind == PartKind::Home
    }
}

/// Where the local store's searchable parts come from.
///
/// Implemented at the composition root, over the real store. Both methods may fail, and both
/// failures are [`Unavailable`] rather than errors: a store that cannot list its graphs is a
/// source that cannot answer, and `adr/0003` says a source that cannot answer must not stop
/// somebody creating a concept.
pub trait LocalCorpus {
    /// Every part worth searching, in the order a report should read them.
    fn parts(&self) -> Result<Vec<CorpusPart>, Unavailable>;

    /// Read one part into the SKOS model, which is then searched and dropped.
    fn model(&self, part: &CorpusPart) -> Result<CoreModel, Unavailable>;
}

/// Discovery over the vocabularies — and pending changes — of the local store.
pub struct LocalVocabularies<'a> {
    corpus: &'a dyn LocalCorpus,
    name: String,
}

impl<'a> LocalVocabularies<'a> {
    /// Search `corpus`, under the name `name` in reports.
    ///
    /// The name is the caller's because a deployment's store is not always "this store" to the
    /// person reading — a federated peer's store is the same provider with a different name.
    pub fn named(name: impl Into<String>, corpus: &'a dyn LocalCorpus) -> Self {
        LocalVocabularies {
            corpus,
            name: name.into(),
        }
    }
}

impl LocalVocabularies<'_> {
    /// Read the corpus **once** and answer every query from each part while it is in hand.
    ///
    /// The reading is the expensive half — a part is fetched from the store, parsed into a model,
    /// searched, and dropped — and it does not depend on the query. So a split naming three parts
    /// pays for one pass over the store rather than three, and the memory ceiling is unchanged:
    /// still one model at a time, whatever the number of labels.
    fn answer(&self, queries: &[LabelQuery]) -> Result<Vec<SourceAnswer>, Unavailable> {
        let parts = self.corpus.parts()?;
        let mut matches: Vec<Vec<Match>> = queries.iter().map(|_| Vec::new()).collect();
        let mut labels_read = vec![0usize; queries.len()];
        let mut vocabularies = 0;
        let mut pending = 0;

        for part in &parts {
            // A part that cannot be read stops the source rather than being skipped. Skipping it
            // would produce a shorter answer that looks exactly like a complete one, which is the
            // "nothing found" reading `adr/0003` §7 refuses — and the whole source degrading is
            // reported, so the reader knows the answer is partial.
            let model = self.corpus.model(part)?;
            match part.kind {
                PartKind::Home | PartKind::Vocabulary => vocabularies += 1,
                PartKind::Pending => pending += 1,
            }

            for (index, query) in queries.iter().enumerate() {
                let found = model.search(query);
                labels_read[index] += found.labels_read();

                for hit in found.hits() {
                    matches[index].push(Match {
                        source: self.name.clone(),
                        within: part.within.clone(),
                        home: part.is_home(),
                        display: model
                            .resource(&hit.resource)
                            .and_then(Resource::display_label)
                            .map(|label| label.text.clone()),
                        resource: hit.resource.clone(),
                        label: hit.label.clone(),
                        kind: hit.best_kind(),
                        quality: hit.quality,
                    });
                }
            }
        }

        let searched = searched(vocabularies, pending);
        Ok(matches
            .into_iter()
            .zip(labels_read)
            .map(|(matches, labels_read)| SourceAnswer {
                matches,
                searched: searched.clone(),
                labels_read,
            })
            .collect())
    }
}

impl DiscoveryProvider for LocalVocabularies<'_> {
    fn name(&self) -> &str {
        &self.name
    }

    fn search(&self, query: &LabelQuery) -> Result<SourceAnswer, Unavailable> {
        let mut answers = self.answer(std::slice::from_ref(query))?;
        // One query in, one answer out, by construction. Spelled out rather than unwrapped
        // (`CLAUDE.md` §6): a source that answered nothing at all is a source that did not answer.
        answers.pop().ok_or_else(|| {
            Unavailable::because("the local store returned no answer to the one label it was asked")
        })
    }

    fn search_each(&self, queries: &[LabelQuery]) -> Result<Vec<SourceAnswer>, Unavailable> {
        self.answer(queries)
    }
}

/// What the pass looked at, in the words a report prints.
fn searched(vocabularies: usize, pending: usize) -> String {
    match pending {
        0 => format!("{vocabularies} vocabular{}", plural(vocabularies)),
        _ => format!(
            "{vocabularies} vocabular{} and {pending} change(s) waiting for a decision",
            plural(vocabularies)
        ),
    }
}

/// English, so a report does not print "1 vocabularies" at the one moment it is being trusted.
fn plural(count: usize) -> &'static str {
    match count {
        1 => "y",
        _ => "ies",
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use openbiz_skos::{
        CoreModel, LabelKind, LabelQuery, Literal, MatchQuality, Node, Statement, Term, RDF_TYPE,
        SKOS_ALT_LABEL, SKOS_PREF_LABEL,
    };

    use crate::{Discovery, DiscoveryProvider, Unavailable};

    use super::{CorpusPart, LocalCorpus, LocalVocabularies};

    /// One concept's label, as the fixture writes it: the IRI, the kind, and the text.
    type Labelled = (&'static str, LabelKind, &'static str);

    /// A corpus held in memory, which can also be told to fail.
    struct Fixture {
        parts: Vec<(CorpusPart, Vec<Labelled>)>,
        fails_on: Option<String>,
        listing_fails: bool,
        read: RefCell<Vec<String>>,
    }

    impl Fixture {
        fn new() -> Self {
            Fixture {
                parts: Vec::new(),
                fails_on: None,
                listing_fails: false,
                read: RefCell::new(Vec::new()),
            }
        }

        fn with(mut self, part: CorpusPart, labels: Vec<Labelled>) -> Self {
            self.parts.push((part, labels));
            self
        }
    }

    impl LocalCorpus for Fixture {
        fn parts(&self) -> Result<Vec<CorpusPart>, Unavailable> {
            match self.listing_fails {
                true => Err(Unavailable::because("the store could not list its graphs")),
                false => Ok(self.parts.iter().map(|(part, _)| part.clone()).collect()),
            }
        }

        fn model(&self, part: &CorpusPart) -> Result<CoreModel, Unavailable> {
            if self.fails_on.as_deref() == Some(part.within.as_str()) {
                return Err(Unavailable::because(format!(
                    "{} is unreadable",
                    part.within
                )));
            }
            self.read.borrow_mut().push(part.within.clone());
            let labels = self
                .parts
                .iter()
                .find(|(candidate, _)| candidate == part)
                .map(|(_, labels)| labels.clone())
                .unwrap_or_default();
            let mut statements = Vec::new();
            for (iri, kind, text) in labels {
                statements.push(Statement {
                    subject: Node::iri(iri),
                    predicate: RDF_TYPE.to_owned(),
                    object: Term::Node(Node::iri("http://www.w3.org/2004/02/skos/core#Concept")),
                });
                statements.push(Statement {
                    subject: Node::iri(iri),
                    predicate: match kind {
                        LabelKind::Preferred => SKOS_PREF_LABEL.to_owned(),
                        _ => SKOS_ALT_LABEL.to_owned(),
                    },
                    object: Term::Literal(Literal {
                        value: text.to_owned(),
                        language: Some("en".to_owned()),
                        datatype: "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString"
                            .to_owned(),
                    }),
                });
            }
            Ok(CoreModel::from_statements(statements))
        }
    }

    fn query(text: &str) -> LabelQuery {
        LabelQuery::new(text).expect("a query")
    }

    /// **The item's point.** A concept in *another* vocabulary in the same store is found, which
    /// is the match `openbiz mint` could not make before this existed.
    #[test]
    fn a_concept_in_another_vocabulary_is_found() {
        let corpus = Fixture::new()
            .with(CorpusPart::home("urn:home", "this vocabulary"), Vec::new())
            .with(
                CorpusPart::elsewhere(
                    "urn:materials",
                    "the vocabulary https://example.org/materials",
                ),
                vec![("urn:m:1", LabelKind::Preferred, "Solar power")],
            );
        let source = LocalVocabularies::named("this store", &corpus);

        let answer = source.search(&query("Solar power")).expect("an answer");

        assert_eq!(answer.matches.len(), 1);
        let found = &answer.matches[0];
        assert_eq!(found.resource, Node::iri("urn:m:1"));
        assert_eq!(found.within, "the vocabulary https://example.org/materials");
        assert!(!found.home, "it is not in the vocabulary being authored");
        assert_eq!(found.quality, MatchQuality::Exact);
        assert_eq!(found.display.as_deref(), Some("Solar power"));
    }

    /// The forgiving default earns its keep: a term nobody would have typed exactly is still
    /// surfaced, ranked below the exact hits rather than mixed in with them.
    #[test]
    fn a_partial_match_is_found_and_ranked_below_an_exact_one() {
        let corpus = Fixture::new().with(
            CorpusPart::home("urn:home", "this vocabulary"),
            vec![
                ("urn:e:1", LabelKind::Preferred, "Solar power generation"),
                ("urn:e:2", LabelKind::Preferred, "Solar power"),
            ],
        );
        let source = LocalVocabularies::named("this store", &corpus);

        let discovered = Discovery::new().across(&[&source], &query("Solar power"));
        let order: Vec<_> = discovered
            .matches()
            .iter()
            .map(|found| found.resource.to_string())
            .collect();

        assert_eq!(order, vec!["<urn:e:2>", "<urn:e:1>"], "{order:?}");
        assert_eq!(discovered.exact().count(), 1);
        assert_eq!(discovered.related().count(), 1);
    }

    /// An alternative label is a way in, and the report has to be able to say it was one: a
    /// concept found under `skos:altLabel` is displayed under its preferred label, per §5.1.
    #[test]
    fn a_hit_on_an_alternative_label_is_shown_under_the_preferred_one() {
        let corpus = Fixture::new().with(
            CorpusPart::home("urn:home", "this vocabulary"),
            vec![
                ("urn:e:1", LabelKind::Preferred, "Photovoltaics"),
                ("urn:e:1", LabelKind::Alternative, "Solar power"),
            ],
        );
        let source = LocalVocabularies::named("this store", &corpus);

        let answer = source.search(&query("Solar power")).expect("an answer");

        assert_eq!(answer.matches.len(), 1);
        assert_eq!(answer.matches[0].kind, Some(LabelKind::Alternative));
        assert_eq!(answer.matches[0].display.as_deref(), Some("Photovoltaics"));
    }

    /// What was searched is reported in the reader's terms, and the pending changes are counted
    /// apart from the vocabularies — a label that exists only in a change nobody has approved is
    /// a different fact from one that is in the vocabulary.
    #[test]
    fn the_answer_says_what_it_looked_at() {
        let corpus = Fixture::new()
            .with(CorpusPart::home("urn:home", "this vocabulary"), Vec::new())
            .with(
                CorpusPart::elsewhere("urn:other", "the vocabulary urn:other"),
                Vec::new(),
            )
            .with(
                CorpusPart::pending("urn:c3", "candidate 3, which is waiting for a decision"),
                Vec::new(),
            );
        let source = LocalVocabularies::named("this store", &corpus);

        let answer = source.search(&query("anything")).expect("an answer");

        assert_eq!(
            answer.searched,
            "2 vocabularies and 1 change(s) waiting for a decision"
        );
    }

    /// One vocabulary is one vocabulary. A report that prints "1 vocabularies" at the moment it
    /// is being trusted reads as machine output nobody checked.
    #[test]
    fn one_vocabulary_is_not_pluralised() {
        let corpus =
            Fixture::new().with(CorpusPart::home("urn:home", "this vocabulary"), Vec::new());
        let source = LocalVocabularies::named("this store", &corpus);

        assert_eq!(
            source
                .search(&query("anything"))
                .expect("an answer")
                .searched,
            "1 vocabulary"
        );
    }

    /// **`adr/0003` §7 at the store.** A part that cannot be read makes the source unavailable —
    /// reported, and not a shorter answer that reads like a complete one.
    #[test]
    fn a_part_that_cannot_be_read_makes_the_source_unavailable() {
        let mut corpus = Fixture::new()
            .with(
                CorpusPart::home("urn:home", "this vocabulary"),
                vec![("urn:e:1", LabelKind::Preferred, "Solar power")],
            )
            .with(
                CorpusPart::elsewhere("urn:other", "the vocabulary urn:other"),
                Vec::new(),
            );
        corpus.fails_on = Some("the vocabulary urn:other".to_owned());
        let source = LocalVocabularies::named("this store", &corpus);

        let discovered = Discovery::new().across(&[&source], &query("Solar power"));

        assert!(
            discovered.is_empty(),
            "a partial answer must not be offered as a whole one"
        );
        let unavailable: Vec<_> = discovered.unavailable().collect();
        assert_eq!(
            unavailable,
            vec![("this store", "the vocabulary urn:other is unreadable")]
        );
    }

    /// The same, one level up: a store that cannot even list its graphs is unavailable rather
    /// than empty.
    #[test]
    fn a_store_that_cannot_list_its_graphs_is_unavailable_not_empty() {
        let mut corpus = Fixture::new();
        corpus.listing_fails = true;
        let source = LocalVocabularies::named("this store", &corpus);

        let discovered = Discovery::new().across(&[&source], &query("Solar power"));

        assert_eq!(discovered.unavailable().count(), 1);
    }

    /// Each part is read once and dropped: the memory this costs is one vocabulary, not the
    /// store. Pinned because it is a property of the loop that a refactor could silently lose.
    #[test]
    fn every_part_is_read_exactly_once() {
        let corpus = Fixture::new()
            .with(CorpusPart::home("urn:home", "this vocabulary"), Vec::new())
            .with(
                CorpusPart::elsewhere("urn:other", "the vocabulary urn:other"),
                Vec::new(),
            );
        let source = LocalVocabularies::named("this store", &corpus);

        source.search(&query("anything")).expect("an answer");

        assert_eq!(
            *corpus.read.borrow(),
            vec!["this vocabulary", "the vocabulary urn:other"]
        );
    }

    /// **The reason `search_each` exists.** Three labels asked at once cost one reading of the
    /// store, not three — `openbiz split` names several concepts in one command and every one of
    /// them is a creation discovery has to run in front of.
    #[test]
    fn several_labels_cost_one_reading_of_the_corpus() {
        let corpus = Fixture::new()
            .with(
                CorpusPart::home("urn:home", "this vocabulary"),
                vec![("urn:wind", LabelKind::Preferred, "Wind power")],
            )
            .with(
                CorpusPart::elsewhere("urn:other", "the vocabulary urn:other"),
                vec![("urn:tidal", LabelKind::Preferred, "Tidal power")],
            );
        let source = LocalVocabularies::named("this store", &corpus);
        let queries = [query("Wind power"), query("Tidal power"), query("Solar")];

        let answers = source.search_each(&queries).expect("an answer per label");

        assert_eq!(
            *corpus.read.borrow(),
            vec!["this vocabulary", "the vocabulary urn:other"],
            "three labels must not read the store three times"
        );
        assert_eq!(answers.len(), 3);
        assert_eq!(answers[0].matches.len(), 1, "Wind power is at home");
        assert!(answers[0].matches[0].home);
        assert_eq!(answers[1].matches.len(), 1, "Tidal power is elsewhere");
        assert!(!answers[1].matches[0].home);
        assert!(answers[2].matches.is_empty(), "nothing is called Solar");
        // Every answer accounts for the whole corpus: a label that found nothing still says how
        // far the looking went, which is what stops "nothing found" reading as "nothing exists".
        for answer in &answers {
            assert_eq!(answer.labels_read, 2);
            assert_eq!(answer.searched, "2 vocabularies");
        }
    }
}
