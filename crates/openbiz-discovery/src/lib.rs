//! Discovery: what already exists, asked **before** anything new is created.
//!
//! # The failure this crate exists to prevent
//!
//! `CLAUDE.md` §1.7 and `docs/adr/0003` name it exactly: the enterprise that owns nine overlapping
//! taxonomies and cannot tell. A tool that makes a new vocabulary cheap and the existing nine
//! invisible is a silo generator, and every incumbent is one, because a new-concept wizard is easy
//! to build and cross-enterprise search is not.
//!
//! So discovery is not a feature a curator has to remember to invoke. It is on the creation path,
//! and the creation path in this build is `openbiz mint` — the command that answers what a new
//! concept's IRI would be. Nothing may be minted without first saying what is already there.
//!
//! # Three rules this crate is built to keep
//!
//! **1. A source that fails is reported, never fatal** (`adr/0003` §7 and its consequences). A
//! provider answers with [`SourceAnswer`] or with [`Unavailable`], and [`Discovery::across`]
//! records the unavailable ones and carries on. A broken connector must degrade to "source
//! unavailable", because the alternative — creation blocked by a catalog that is down — is a tool
//! nobody will keep on the creation path, and a tool that is off the creation path prevents
//! nothing.
//!
//! **2. Nothing found is never reported as nothing exists.** [`Discovered`] carries what was
//! consulted, what each source actually looked at, and what could not be reached, so a report can
//! never print a bare "no matches". That reading is precisely how the tenth overlapping vocabulary
//! gets created, and it is the reading a silently broken connector produces.
//!
//! **3. Every match says why it matched.** A [`Match`] carries the label that matched, the kind it
//! is held under, and the quality of the match, so a report can explain itself the way `CLAUDE.md`
//! §3 requires of every answer this build gives.
//!
//! # What is not here
//!
//! Sources beyond the local store — federated OpenBiz peers, SPARQL endpoints, public registries,
//! and enterprise catalog connectors — are Phase 12 (`adr/0003` §2). The trait is theirs to
//! implement when they arrive; nothing in this crate can reach the network, and adding a source
//! that can is a decision with its own ADR.

mod local;

use std::collections::BTreeMap;

use openbiz_skos::{LabelKind, LabelQuery, LexicalLabel, MatchQuality, Node};
use thiserror::Error;

pub use local::{CorpusPart, LocalCorpus, LocalVocabularies, PartKind};

/// One label, in one source, that the query matched.
///
/// Everything a report needs to name the thing found, say where it is, and say why it is here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// The source that answered — a [`DiscoveryProvider::name`].
    pub source: String,
    /// Where within that source, in the words the reader needs: which vocabulary, which change
    /// waiting for a decision, which registry.
    pub within: String,
    /// Whether this is in the vocabulary the new concept would go into.
    ///
    /// The distinction is the reuse ladder's (`adr/0003` §3): a match **at home** is a duplicate
    /// about to be created inside one vocabulary, and a match elsewhere is something to map to or
    /// extend. Sources that are not the local store set this false — they are all "elsewhere".
    pub home: bool,
    /// The resource the label belongs to.
    pub resource: Node,
    /// Its preferred label, for display. Never a hidden one — SKOS §5.1.
    pub display: Option<String>,
    /// The label that matched, exactly as the source carries it.
    pub label: LexicalLabel,
    /// The kind it is held under, best first — preferred over alternative over hidden.
    pub kind: Option<LabelKind>,
    /// How it matched. The whole label, its beginning, or somewhere inside it.
    pub quality: MatchQuality,
}

impl Match {
    /// The total order a report ranks by: best quality first, then preferred labels, then home.
    ///
    /// Total rather than merely consistent — the resource and the label text are both in the key —
    /// so truncating a sorted list gives the same answer however the sources were interleaved.
    fn key(&self) -> (MatchQuality, Option<LabelKind>, bool, &str, &Node) {
        (
            self.quality,
            self.kind,
            // `false` sorts first, so negating puts the home vocabulary's matches above the rest:
            // a duplicate inside the vocabulary being authored is the loudest thing here.
            !self.home,
            self.label.text.as_str(),
            &self.resource,
        )
    }

    /// Whether the label matched in full — the case where creating a second one is a duplicate.
    pub fn is_exact(&self) -> bool {
        self.quality == MatchQuality::Exact
    }
}

/// What one source found, and what it looked at to find it.
///
/// `searched` is not decoration. A source that answers "nothing" has said something useful only if
/// the reader can tell how far it looked; without it, an empty answer from a source that read one
/// vocabulary is indistinguishable from an empty answer from one that read the enterprise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAnswer {
    /// Every match, in any order. [`Discovery::across`] ranks and bounds them.
    pub matches: Vec<Match>,
    /// What this source actually looked at, in the reader's words.
    pub searched: String,
    /// How many (resource, label) pairs were considered, for the same reason.
    pub labels_read: usize,
}

/// A source could not be consulted.
///
/// Never fatal. `adr/0003`'s consequences are explicit: a broken connector degrades to "source
/// unavailable" and must never block creation. The reason is carried so the report can name it —
/// "the catalog is unreachable" and "the catalog holds nothing like this" are opposite facts and
/// a reader who cannot tell them apart will act on the wrong one.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{reason}")]
pub struct Unavailable {
    /// Why, in a sentence a curator can act on.
    pub reason: String,
}

impl Unavailable {
    /// A source that could not answer, and why.
    pub fn because(reason: impl Into<String>) -> Self {
        Unavailable {
            reason: reason.into(),
        }
    }
}

/// A source discovery can ask.
///
/// `CLAUDE.md` §3: every engine sits behind a trait we own. The local store, a federated peer, a
/// SPARQL endpoint, a public registry and an enterprise catalog are all this trait, and none of
/// them may be called from application code by any other route.
pub trait DiscoveryProvider {
    /// What this source is called in a report.
    ///
    /// The reader has to be able to tell which source answered and which one did not, so this is
    /// how a deployment's own name for a catalog reaches the page.
    fn name(&self) -> &str;

    /// Ask this source for everything it holds that matches.
    ///
    /// Returning [`Unavailable`] is a normal outcome, not an error path the caller must handle:
    /// [`Discovery::across`] records it and carries on.
    fn search(&self, query: &LabelQuery) -> Result<SourceAnswer, Unavailable>;

    /// Ask this source about several labels at once, answering **one per query, in order**.
    ///
    /// A creation path does not always create one thing. `openbiz split` names two or more
    /// concepts in one command, and every one of them is a creation `CLAUDE.md` §1.7 wants
    /// discovery run in front of. Asking [`search`] once per label works and costs the whole
    /// source once per label — for the local store that is every vocabulary re-read from disk for
    /// each name, in front of somebody who is waiting.
    ///
    /// So a source that can answer many questions from one reading of itself overrides this. The
    /// default is the honest loop, which is what a remote source with one request per query wants
    /// anyway.
    ///
    /// The returned vector **must** be the same length as `queries`; an implementation that
    /// cannot honour that should return [`Unavailable`] instead, because a caller cannot line up
    /// answers with the labels that produced them and a match attached to the wrong name is worse
    /// than no match at all.
    ///
    /// [`search`]: DiscoveryProvider::search
    fn search_each(&self, queries: &[LabelQuery]) -> Result<Vec<SourceAnswer>, Unavailable> {
        queries.iter().map(|query| self.search(query)).collect()
    }
}

/// How many matches a discovery pass reports before it starts counting instead.
///
/// The rest are counted, never dropped silently: [`Discovered::withheld`] is what stands between
/// "you were shown the first ten" and "there are ten".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoveryBound {
    /// The most matches to report.
    pub max_matches: usize,
}

impl DiscoveryBound {
    /// 20 matches.
    ///
    /// Reasoning, not measurement, and `docs/UNTESTED.md` says so: this sits above a report a
    /// curator is reading in the middle of creating one concept, where twenty already fills a
    /// screen, and the count of what was withheld is what makes the twenty-first discoverable.
    pub const DEFAULT: DiscoveryBound = DiscoveryBound { max_matches: 20 };
}

/// What one source was asked, and what came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consulted {
    /// The source's name.
    pub source: String,
    /// What it said.
    pub outcome: Outcome,
}

/// How a source answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It answered.
    Answered {
        /// How many matches it returned, before the bound.
        matched: usize,
        /// What it looked at.
        searched: String,
        /// How many (resource, label) pairs it considered.
        labels_read: usize,
    },
    /// It could not be reached, or refused.
    Unavailable {
        /// Why.
        reason: String,
    },
}

/// Everything discovery found, and everything it could not.
///
/// There is no `Result` around this on purpose. Discovery as a whole cannot fail — a pass where
/// every source was unavailable is a pass that found nothing and says so loudly, which is a
/// different and much safer thing than an error that stops a curator from working.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    matches: Vec<Match>,
    matched: usize,
    consulted: Vec<Consulted>,
    bound: DiscoveryBound,
}

impl Discovered {
    /// The matches, best first, up to the bound.
    pub fn matches(&self) -> &[Match] {
        &self.matches
    }

    /// The ones whose label is the query in full — where creating another is creating a duplicate.
    pub fn exact(&self) -> impl Iterator<Item = &Match> {
        self.matches.iter().filter(|found| found.is_exact())
    }

    /// The ones that matched on part of a label: related terms, not the same term.
    pub fn related(&self) -> impl Iterator<Item = &Match> {
        self.matches.iter().filter(|found| !found.is_exact())
    }

    /// How many matches every source returned in total, including those the bound withheld.
    pub fn matched(&self) -> usize {
        self.matched
    }

    /// How many matched but are not listed, because the bound was reached.
    pub fn withheld(&self) -> usize {
        self.matched - self.matches.len()
    }

    /// Whether every match is listed.
    pub fn is_complete(&self) -> bool {
        self.withheld() == 0
    }

    /// Whether nothing at all was found — which is only ever meaningful beside [`unavailable`].
    ///
    /// [`unavailable`]: Discovered::unavailable
    pub fn is_empty(&self) -> bool {
        self.matched == 0
    }

    /// Every source asked, in the order they were asked.
    pub fn consulted(&self) -> &[Consulted] {
        &self.consulted
    }

    /// The sources that could not answer, with their reasons.
    ///
    /// A caller that ignores this is reporting an incomplete search as a complete one, which is
    /// the silo-generating reading `adr/0003` §7 exists to prevent.
    pub fn unavailable(&self) -> impl Iterator<Item = (&str, &str)> {
        self.consulted
            .iter()
            .filter_map(|entry| match &entry.outcome {
                Outcome::Unavailable { reason } => Some((entry.source.as_str(), reason.as_str())),
                Outcome::Answered { .. } => None,
            })
    }

    /// The bound this pass ran under.
    pub fn bound(&self) -> DiscoveryBound {
        self.bound
    }

    /// One consultation record covering several passes over the same sources.
    ///
    /// [`Discovery::across_each`] gives every label its own [`Discovered`], each carrying the same
    /// sources with that label's own counts. A report about three parts of a split does not want
    /// to say "discovery consulted 1 source" three times; it wants to say it once, for the whole
    /// command, without losing what any single pass learned. So each source appears once, first
    /// asked first, with its counts summed.
    ///
    /// **A source unavailable to any pass is unavailable here**, whatever the others said. The
    /// alternative — a source that answered two labels out of three reported as having answered —
    /// is the "nothing found" reading `adr/0003` §7 refuses, arrived at by averaging.
    pub fn consulted_across(passes: &[Discovered]) -> Vec<Consulted> {
        let mut order: Vec<String> = Vec::new();
        let mut merged: BTreeMap<String, Outcome> = BTreeMap::new();

        for pass in passes {
            for entry in pass.consulted() {
                let outcome = match merged.remove(&entry.source) {
                    None => {
                        order.push(entry.source.clone());
                        entry.outcome.clone()
                    }
                    Some(existing) => merge(existing, entry.outcome.clone()),
                };
                merged.insert(entry.source.clone(), outcome);
            }
        }

        order
            .into_iter()
            .filter_map(|source| {
                merged
                    .remove(&source)
                    .map(|outcome| Consulted { source, outcome })
            })
            .collect()
    }
}

/// Two outcomes for one source, over two labels, as one outcome for that source.
fn merge(existing: Outcome, next: Outcome) -> Outcome {
    match (existing, next) {
        (
            Outcome::Answered {
                matched,
                searched,
                labels_read,
            },
            Outcome::Answered {
                matched: more,
                labels_read: read,
                ..
            },
        ) => Outcome::Answered {
            matched: matched + more,
            // The first pass's, not the last: every pass over one source looked at the same
            // thing, and the sentence describes the source rather than the query.
            searched,
            labels_read: labels_read + read,
        },
        // Unavailable wins, in either position and however many passes answered.
        (Outcome::Unavailable { reason }, _) => Outcome::Unavailable { reason },
        (_, Outcome::Unavailable { reason }) => Outcome::Unavailable { reason },
    }
}

/// The pass itself: ask every source, rank what comes back, and account for the rest.
#[derive(Debug, Clone, Copy)]
pub struct Discovery {
    bound: DiscoveryBound,
}

impl Default for Discovery {
    fn default() -> Self {
        Discovery {
            bound: DiscoveryBound::DEFAULT,
        }
    }
}

impl Discovery {
    /// A pass under [`DiscoveryBound::DEFAULT`].
    pub fn new() -> Self {
        Discovery::default()
    }

    /// A pass reporting at most `bound` matches.
    pub fn with_bound(mut self, bound: DiscoveryBound) -> Self {
        self.bound = bound;
        self
    }

    /// Ask every source and merge the answers.
    ///
    /// Sources are asked in the order given and each is asked exactly once. One that fails is
    /// recorded as [`Outcome::Unavailable`] and the pass continues — a curator whose catalog is
    /// down still gets everything the local store knows, and is told what was missing from the
    /// answer rather than being stopped.
    pub fn across(&self, sources: &[&dyn DiscoveryProvider], query: &LabelQuery) -> Discovered {
        let mut passes = self.across_each(sources, std::slice::from_ref(query));
        match passes.pop() {
            Some(found) => found,
            // One query in, one pass out, by construction. Written out rather than unwrapped
            // because `CLAUDE.md` §6 forbids the unwrap, and an empty pass is a truthful answer:
            // nothing found, nothing consulted.
            None => self.finish(Vec::new(), 0, Vec::new()),
        }
    }

    /// Ask every source about several labels at once, and rank each label's answers separately.
    ///
    /// One [`Discovered`] per query, in the order the queries were given. Each source is asked
    /// **once** — through [`DiscoveryProvider::search_each`] — so a source that can read itself
    /// once and answer every question does, which is the difference between a split of three parts
    /// costing one pass over the store and costing three.
    ///
    /// A source that fails, or that answers a number of questions it was not asked, is recorded as
    /// [`Outcome::Unavailable`] against **every** query and the pass continues. Silently dropping
    /// it from one label's consultation record and not another's would let one part of a split
    /// report a complete search while its neighbour reported a partial one, from the same pass.
    pub fn across_each(
        &self,
        sources: &[&dyn DiscoveryProvider],
        queries: &[LabelQuery],
    ) -> Vec<Discovered> {
        let mut matches: Vec<Vec<Match>> = queries.iter().map(|_| Vec::new()).collect();
        let mut matched = vec![0usize; queries.len()];
        let mut consulted: Vec<Vec<Consulted>> = queries.iter().map(|_| Vec::new()).collect();

        for source in sources {
            let answers = match source.search_each(queries) {
                Ok(answers) if answers.len() == queries.len() => Ok(answers),
                // A source that answered a different number of questions than it was asked cannot
                // be lined up with the labels, and guessing the alignment would show a curator a
                // match under the wrong name. That is the one failure this whole crate exists to
                // prevent, so the source is treated as one that could not answer.
                Ok(answers) => Err(Unavailable::because(format!(
                    "it was asked about {} label(s) and answered {}, so its answers cannot be \
                     told apart",
                    queries.len(),
                    answers.len()
                ))),
                Err(unavailable) => Err(unavailable),
            };
            match answers {
                Ok(answers) => {
                    for (index, answer) in answers.into_iter().enumerate() {
                        matched[index] += answer.matches.len();
                        consulted[index].push(Consulted {
                            source: source.name().to_owned(),
                            outcome: Outcome::Answered {
                                matched: answer.matches.len(),
                                searched: answer.searched,
                                labels_read: answer.labels_read,
                            },
                        });
                        matches[index].extend(answer.matches);
                    }
                }
                Err(unavailable) => {
                    for record in consulted.iter_mut() {
                        record.push(Consulted {
                            source: source.name().to_owned(),
                            outcome: Outcome::Unavailable {
                                reason: unavailable.reason.clone(),
                            },
                        });
                    }
                }
            }
        }

        matches
            .into_iter()
            .zip(matched)
            .zip(consulted)
            .map(|((matches, matched), consulted)| self.finish(matches, matched, consulted))
            .collect()
    }

    /// Rank one query's matches, bound them, and keep the count of what the bound withheld.
    fn finish(
        &self,
        mut matches: Vec<Match>,
        matched: usize,
        consulted: Vec<Consulted>,
    ) -> Discovered {
        matches.sort_by(|left, right| left.key().cmp(&right.key()));
        matches.truncate(self.bound.max_matches);

        Discovered {
            matches,
            matched,
            consulted,
            bound: self.bound,
        }
    }
}

#[cfg(test)]
mod tests {
    use openbiz_skos::{LabelKind, LabelQuery, LexicalLabel, MatchQuality, Node};

    use super::{
        Discovered, Discovery, DiscoveryBound, DiscoveryProvider, Match, Outcome, SourceAnswer,
        Unavailable,
    };

    /// A source that answers with whatever it was built with.
    struct Fixed {
        name: String,
        answer: Result<SourceAnswer, Unavailable>,
    }

    impl DiscoveryProvider for Fixed {
        fn name(&self) -> &str {
            &self.name
        }

        fn search(&self, _query: &LabelQuery) -> Result<SourceAnswer, Unavailable> {
            self.answer.clone()
        }
    }

    fn found(iri: &str, text: &str, quality: MatchQuality, home: bool) -> Match {
        Match {
            source: "a source".to_owned(),
            within: "somewhere".to_owned(),
            home,
            resource: Node::iri(iri),
            display: Some(text.to_owned()),
            label: LexicalLabel {
                language: Some("en".to_owned()),
                text: text.to_owned(),
            },
            kind: Some(LabelKind::Preferred),
            quality,
        }
    }

    fn answering(name: &str, matches: Vec<Match>) -> Fixed {
        Fixed {
            name: name.to_owned(),
            answer: Ok(SourceAnswer {
                labels_read: matches.len(),
                matches,
                searched: "a fixture".to_owned(),
            }),
        }
    }

    fn failing(name: &str, reason: &str) -> Fixed {
        Fixed {
            name: name.to_owned(),
            answer: Err(Unavailable::because(reason)),
        }
    }

    fn query() -> LabelQuery {
        LabelQuery::new("energy").expect("a query")
    }

    /// A source that answers each query with the matches whose label text is that query.
    ///
    /// Counts how many times it was read, which is the property `search_each` exists for.
    struct PerLabel {
        matches: Vec<Match>,
        reads: std::cell::Cell<usize>,
    }

    impl DiscoveryProvider for PerLabel {
        fn name(&self) -> &str {
            "a source that reads itself"
        }

        fn search(&self, query: &LabelQuery) -> Result<SourceAnswer, Unavailable> {
            self.reads.set(self.reads.get() + 1);
            let matches: Vec<Match> = self
                .matches
                .iter()
                .filter(|found| found.label.text == query.text())
                .cloned()
                .collect();
            Ok(SourceAnswer {
                labels_read: self.matches.len(),
                matches,
                searched: "a fixture".to_owned(),
            })
        }
    }

    /// Every query gets its own ranked answer, and they do not bleed into one another.
    #[test]
    fn a_pass_over_several_labels_answers_each_one_separately() {
        let source = PerLabel {
            matches: vec![
                found("http://example.org/wind", "wind", MatchQuality::Exact, true),
                found(
                    "http://example.org/tidal",
                    "tidal",
                    MatchQuality::Exact,
                    false,
                ),
            ],
            reads: std::cell::Cell::new(0),
        };
        let queries = [
            LabelQuery::new("wind").expect("a query"),
            LabelQuery::new("tidal").expect("a query"),
            LabelQuery::new("solar").expect("a query"),
        ];

        let passes = Discovery::new().across_each(&[&source], &queries);

        assert_eq!(passes.len(), 3);
        assert_eq!(
            passes[0]
                .matches()
                .iter()
                .map(|found| found.resource.to_string())
                .collect::<Vec<_>>(),
            ["<http://example.org/wind>"]
        );
        assert_eq!(
            passes[1]
                .matches()
                .iter()
                .map(|found| found.resource.to_string())
                .collect::<Vec<_>>(),
            ["<http://example.org/tidal>"]
        );
        assert!(passes[2].is_empty(), "nothing is called solar");
        // Every pass says what was consulted, including the one that found nothing — a bare
        // "nothing found" for the third label is exactly the reading that creates a duplicate.
        for pass in &passes {
            assert_eq!(pass.consulted().len(), 1);
        }
    }

    /// The default `search_each` is the honest loop: a source that does not override it is asked
    /// once per label and still lines its answers up.
    #[test]
    fn a_source_that_does_not_override_is_asked_once_per_label() {
        let source = PerLabel {
            matches: vec![found(
                "http://example.org/wind",
                "wind",
                MatchQuality::Exact,
                true,
            )],
            reads: std::cell::Cell::new(0),
        };
        let queries = [
            LabelQuery::new("wind").expect("a query"),
            LabelQuery::new("solar").expect("a query"),
        ];

        let passes = Discovery::new().across_each(&[&source], &queries);

        assert_eq!(source.reads.get(), 2);
        assert_eq!(passes[0].matched(), 1);
        assert_eq!(passes[1].matched(), 0);
    }

    /// A source that answers a different number of questions than it was asked is **unavailable**,
    /// against every query. Lining its answers up would attach a match to the wrong label.
    #[test]
    fn a_source_whose_answers_cannot_be_lined_up_is_unavailable_to_every_query() {
        struct Miscounting;
        impl DiscoveryProvider for Miscounting {
            fn name(&self) -> &str {
                "a miscounting source"
            }
            fn search(&self, _query: &LabelQuery) -> Result<SourceAnswer, Unavailable> {
                unreachable!("search_each is overridden")
            }
            fn search_each(
                &self,
                _queries: &[LabelQuery],
            ) -> Result<Vec<SourceAnswer>, Unavailable> {
                Ok(vec![SourceAnswer {
                    matches: vec![found(
                        "http://example.org/wind",
                        "wind",
                        MatchQuality::Exact,
                        true,
                    )],
                    searched: "a fixture".to_owned(),
                    labels_read: 1,
                }])
            }
        }

        let queries = [
            LabelQuery::new("wind").expect("a query"),
            LabelQuery::new("solar").expect("a query"),
        ];

        let passes = Discovery::new().across_each(&[&Miscounting], &queries);

        assert_eq!(passes.len(), 2);
        for pass in &passes {
            assert!(
                pass.is_empty(),
                "no match may be kept from an answer nothing can align"
            );
            let unavailable: Vec<_> = pass.unavailable().collect();
            assert_eq!(unavailable.len(), 1);
            assert!(
                unavailable[0]
                    .1
                    .contains("asked about 2 label(s) and answered 1"),
                "the reason has to say what went wrong: {}",
                unavailable[0].1
            );
            assert!(
                !unavailable[0].1.contains("  "),
                "a line continuation ate a space: {}",
                unavailable[0].1
            );
        }
    }

    /// A source that fails is unavailable to **every** label of the pass, not just the first.
    #[test]
    fn a_failing_source_is_unavailable_to_every_label() {
        let queries = [
            LabelQuery::new("wind").expect("a query"),
            LabelQuery::new("solar").expect("a query"),
        ];

        let passes = Discovery::new().across_each(
            &[&failing("a catalog", "the catalog is unreachable")],
            &queries,
        );

        for pass in &passes {
            assert_eq!(
                pass.unavailable().collect::<Vec<_>>(),
                [("a catalog", "the catalog is unreachable")]
            );
        }
    }

    /// Several passes over one source read as one consultation, with the counts summed.
    #[test]
    fn one_consultation_record_covers_every_label_of_a_pass() {
        let source = PerLabel {
            matches: vec![
                found("http://example.org/wind", "wind", MatchQuality::Exact, true),
                found(
                    "http://example.org/tidal",
                    "tidal",
                    MatchQuality::Exact,
                    false,
                ),
            ],
            reads: std::cell::Cell::new(0),
        };
        let queries = [
            LabelQuery::new("wind").expect("a query"),
            LabelQuery::new("tidal").expect("a query"),
        ];

        let passes = Discovery::new().across_each(&[&source], &queries);
        let merged = Discovered::consulted_across(&passes);

        assert_eq!(
            merged.len(),
            1,
            "one source, asked about two labels, is one source"
        );
        match &merged[0].outcome {
            Outcome::Answered {
                matched,
                searched,
                labels_read,
            } => {
                assert_eq!(*matched, 2, "one match per label, summed");
                assert_eq!(*labels_read, 4, "two labels read, twice");
                assert_eq!(searched, "a fixture");
            }
            Outcome::Unavailable { reason } => panic!("the source answered: {reason}"),
        }
    }

    /// A source unavailable to **one** label of a pass is unavailable in the merged record, even
    /// though it answered the others. Averaging it away is how a partial search reads as complete.
    #[test]
    fn a_source_unavailable_to_one_label_is_unavailable_in_the_merged_record() {
        let answered = Discovery::new().across(&[&answering("a catalog", vec![])], &query());
        let refused =
            Discovery::new().across(&[&failing("a catalog", "the catalog went away")], &query());

        let merged = Discovered::consulted_across(&[answered, refused]);

        assert_eq!(merged.len(), 1);
        match &merged[0].outcome {
            Outcome::Unavailable { reason } => assert_eq!(reason, "the catalog went away"),
            Outcome::Answered { .. } => panic!("a source that failed once has not answered"),
        }
    }

    /// **`adr/0003` §7 in one test.** A source that fails does not fail the pass: what the other
    /// sources found still comes back, and the failure is named rather than swallowed.
    #[test]
    fn a_source_that_fails_is_reported_and_does_not_block_the_rest() {
        let working = answering(
            "this store",
            vec![found("urn:a", "Energy", MatchQuality::Exact, true)],
        );
        let broken = failing("the catalog", "the catalog did not answer in time");

        let discovered = Discovery::new().across(&[&working, &broken], &query());

        assert_eq!(discovered.matched(), 1, "the working source still answered");
        let unavailable: Vec<_> = discovered.unavailable().collect();
        assert_eq!(
            unavailable,
            vec![("the catalog", "the catalog did not answer in time")],
            "the failure is carried, with its reason"
        );
    }

    /// Every source unavailable is still an answer, and it is emphatically not "nothing exists".
    #[test]
    fn every_source_unavailable_is_an_empty_answer_that_says_why() {
        let broken = failing("the catalog", "unreachable");
        let also = failing("a peer", "refused");

        let discovered = Discovery::new().across(&[&broken, &also], &query());

        assert!(discovered.is_empty());
        assert_eq!(discovered.unavailable().count(), 2);
        assert_eq!(
            discovered.consulted().len(),
            2,
            "a source that could not answer was still consulted, and the report must say so"
        );
    }

    /// Ranking: an exact match outranks a partial one whatever order the sources came back in,
    /// and a duplicate inside the vocabulary being authored outranks the same match elsewhere.
    #[test]
    fn exact_matches_rank_above_partial_ones_and_home_above_elsewhere() {
        let elsewhere = answering(
            "a peer",
            vec![
                found("urn:partial", "Energy policy", MatchQuality::Prefix, false),
                found("urn:away", "Energy", MatchQuality::Exact, false),
            ],
        );
        let home = answering(
            "this store",
            vec![found("urn:home", "Energy", MatchQuality::Exact, true)],
        );

        let discovered = Discovery::new().across(&[&elsewhere, &home], &query());
        let order: Vec<_> = discovered
            .matches()
            .iter()
            .map(|found| found.resource.to_string())
            .collect();

        assert_eq!(
            order,
            vec!["<urn:home>", "<urn:away>", "<urn:partial>"],
            "{order:?}"
        );
        assert_eq!(discovered.exact().count(), 2);
        assert_eq!(discovered.related().count(), 1);
    }

    /// The bound withholds, and says how much. A truncated list that reported its own length as
    /// the total is the "nothing more exists" reading one level down.
    #[test]
    fn the_bound_withholds_and_counts_what_it_withheld() {
        let many = answering(
            "this store",
            (0..5)
                .map(|n| found(&format!("urn:c{n}"), "Energy", MatchQuality::Exact, false))
                .collect(),
        );

        let discovered = Discovery::new()
            .with_bound(DiscoveryBound { max_matches: 2 })
            .across(&[&many], &query());

        assert_eq!(discovered.matches().len(), 2);
        assert_eq!(
            discovered.matched(),
            5,
            "the total is the total, not the listing"
        );
        assert_eq!(discovered.withheld(), 3);
        assert!(!discovered.is_complete());
    }

    /// A source that answered nothing still says what it looked at — the difference between "not
    /// here" and "not looked for".
    #[test]
    fn a_source_that_found_nothing_still_reports_what_it_searched() {
        let empty = answering("this store", Vec::new());

        let discovered = Discovery::new().across(&[&empty], &query());

        assert!(discovered.is_empty());
        assert!(discovered.is_complete());
        match &discovered.consulted()[0].outcome {
            Outcome::Answered {
                matched, searched, ..
            } => {
                assert_eq!(*matched, 0);
                assert_eq!(searched, "a fixture");
            }
            other => panic!("the source answered: {other:?}"),
        }
    }

    /// Asking nothing is a pass that found nothing and consulted nobody — and can never be read
    /// as a clean bill of health, because there is no source in the list to have given one.
    #[test]
    fn no_sources_is_an_empty_pass_with_nobody_consulted() {
        let discovered = Discovery::new().across(&[], &query());

        assert!(discovered.is_empty());
        assert!(discovered.consulted().is_empty());
    }
}
