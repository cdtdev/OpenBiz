//! What Oxigraph's query evaluation actually costs, at the sizes an enterprise vocabulary reaches.
//!
//! `CLAUDE.md` §3 records the risk in one line: *"query evaluation is explicitly not yet optimised
//! upstream. Benchmark before depending on it for large-vocabulary paths."* This module is that
//! benchmark. It exists so the number is **measured before Phase 3 builds an interface on top of
//! it**, rather than discovered by a taxonomist whose concept tree stopped opening.
//!
//! # What is different about these numbers
//!
//! Every triplestore vendor publishes a benchmark. Two things make almost all of them useless to
//! the person asking "will this hold my vocabulary":
//!
//! 1. **They measure a benchmark suite, not the product.** BSBM and LUBM report aggregate query
//!    throughput over a synthetic e-commerce or university dataset. Neither tells you whether
//!    *"list the top concepts of this scheme"* stays interactive, and that is the query a
//!    taxonomist issues before any other. So the queries here are the ones **our own interface
//!    will issue** — top concepts, children, concept detail, label search, ancestors,
//!    descendants — written as they will actually be written, and timed through
//!    [`Store::query`], the same entry point `/api/sparql` calls. The measured time includes
//!    parsing, evaluation, **and serialising the answer**, because that is the whole of what a
//!    caller waits for.
//! 2. **They are unreproducible.** A number from vendor hardware you do not have, on a dataset
//!    you cannot generate, is marketing. The generator, the queries, and the harness are all in
//!    this file; anyone can run them on their own machine and get their own table. `CLAUDE.md` §1
//!    says the roadmap is the repo — so is the benchmark.
//!
//! # The honest limits of what this measures
//!
//! The vocabulary is **synthetic and regular**: a balanced ten-way tree with uniform label
//! lengths. Real thesauri are lumpy — a handful of concepts with thousands of children, label
//! lengths spanning two orders of magnitude, sparse translations. A regular shape flatters an
//! index. Read these numbers as *"no worse than this on this machine"*, never as a ceiling.
//!
//! It also measures **one process on one machine**, with no concurrent load. Concurrency is
//! `docs/BUILD-PLAN.md` Phase 13's problem, and `CLAUDE.md` §8 puts hardware-bound load testing
//! outside the loop entirely.
//!
//! # Running it
//!
//! The 1 000-concept case runs in the ordinary suite so the harness cannot rot unnoticed, and it
//! asserts the **shape of the fixture and the answer of every query** — a benchmark whose queries
//! silently return nothing measures an empty loop very quickly.
//!
//! The real sizes are `#[ignore]`d, because a 1M-concept load is minutes of work and CI is not
//! where that belongs. They must be run in release: a debug-built RocksDB and SPARQL evaluator
//! produce timings that are not about our code at all.
//!
//! ```text
//! cargo test --release -p openbiz-store -- --ignored --nocapture --test-threads=1
//! ```
//!
//! The numbers this produced are in `docs/adr/0013-oxigraph-query-scale.md`.

use std::io;
use std::time::{Duration, Instant};

use oxigraph::model::{Literal, NamedNode, Term};

use crate::{GraphId, QueryFormats, QueryLimits, Store};

/// How many concepts sit at the top of the tree, with no `skos:broader`.
const TOP_CONCEPTS: usize = 10;

/// How many children each non-leaf concept has.
///
/// Ten, so depth grows as log₁₀ of the size: 10k is four levels deep, 1M is six. That is the
/// shape of a real classification scheme rather than of a linked list, and it keeps the
/// transitive-closure queries measuring breadth rather than recursion depth.
const BRANCHING: usize = 10;

/// Concepts loaded per write transaction.
///
/// Batched rather than loaded in one transaction because one transaction holds its whole write
/// batch in memory, and 1M concepts is 7M quads. Batching is also what makes the load path here
/// the *same* path an import will use — [`crate::Transaction::insert`], the choke point — rather
/// than the backend's bulk loader, which would measure a load we do not actually perform.
const CONCEPTS_PER_TRANSACTION: usize = 25_000;

const SKOS: &str = "http://www.w3.org/2004/02/skos/core#";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// Adjectives and nouns, so labels are lexically diverse rather than all sharing a prefix.
///
/// Seventeen and nineteen: coprime with each other and with the branching factor, so the label
/// vocabulary does not correlate with a concept's position in the tree. A prefix search for one
/// adjective therefore matches about one seventeenth of the concepts, scattered — which is what a
/// real label search does, and what makes the search timings mean anything.
const ADJECTIVES: [&str; 17] = [
    "Alluvial",
    "Basal",
    "Cardinal",
    "Distal",
    "Ephemeral",
    "Ferric",
    "Glacial",
    "Hydraulic",
    "Isobaric",
    "Juridical",
    "Karstic",
    "Littoral",
    "Meridional",
    "Nominal",
    "Orbital",
    "Palustrine",
    "Zeta",
];

const NOUNS: [&str; 19] = [
    "abutment",
    "basin",
    "cadastre",
    "datum",
    "easement",
    "fascia",
    "gradient",
    "horizon",
    "inlet",
    "junction",
    "keystone",
    "lintel",
    "manifold",
    "nodule",
    "outcrop",
    "parapet",
    "quadrant",
    "revetment",
    "spillway",
];

/// The adjective a label search looks for. Its index decides how many concepts match.
const SEARCH_ADJECTIVE: &str = ADJECTIVES[16];

/// A generated SKOS vocabulary of a known size and known shape.
///
/// Holds the facts the assertions need — how many concepts, where they are — so the small-size
/// test can prove the fixture is what it claims before anything is timed against it.
struct SyntheticVocabulary {
    graph: GraphId,
    scheme: String,
    size: usize,
}

impl SyntheticVocabulary {
    /// The IRI of concept `index`.
    fn concept(&self, index: usize) -> String {
        format!("{}/c{index}", self.graph.iri())
    }

    /// The index of `index`'s parent, or `None` if it is a top concept.
    const fn parent(index: usize) -> Option<usize> {
        if index < TOP_CONCEPTS {
            None
        } else {
            Some((index - TOP_CONCEPTS) / BRANCHING)
        }
    }

    /// The English preferred label of concept `index`.
    fn pref_label_en(index: usize) -> String {
        format!(
            "{} {} {index}",
            ADJECTIVES[index % ADJECTIVES.len()],
            NOUNS[(index / ADJECTIVES.len()) % NOUNS.len()]
        )
    }

    /// How many ancestors concept `index` has — the length of its `skos:broader` chain.
    fn ancestor_count(index: usize) -> usize {
        let mut depth = 0;
        let mut at = index;
        while let Some(parent) = Self::parent(at) {
            depth += 1;
            at = parent;
        }
        depth
    }

    /// How many concepts have `index` somewhere above them in the tree.
    ///
    /// Counted by walking every concept's chain rather than by a closed form, because the closed
    /// form would be a second implementation of the tree and the two could disagree. This is test
    /// support at sizes where an O(n log n) count is free relative to the load it checks.
    fn descendant_count(&self, index: usize) -> usize {
        (0..self.size)
            .filter(|&candidate| {
                let mut at = candidate;
                while let Some(parent) = Self::parent(at) {
                    if parent == index {
                        return true;
                    }
                    at = parent;
                }
                false
            })
            .count()
    }

    /// How many concepts a search for [`SEARCH_ADJECTIVE`] should match.
    fn search_matches(&self) -> usize {
        (0..self.size)
            .filter(|index| index % ADJECTIVES.len() == 16)
            .count()
    }

    /// How many quads the vocabulary holds, from the generator's own rules.
    fn quad_count(&self) -> usize {
        // Six per concept — type, scheme, two labels, an alternative label, a definition — plus
        // one `skos:broader` for every concept that is not a top concept, plus the scheme's own
        // type and its `skos:hasTopConcept` to each top concept.
        self.size * 6 + self.size.saturating_sub(TOP_CONCEPTS) + 1 + TOP_CONCEPTS.min(self.size)
    }
}

/// What loading a vocabulary cost.
struct LoadReport {
    quads: usize,
    elapsed: Duration,
    /// Bytes the store occupies on disk once the load has settled.
    ///
    /// Measured because "how much disk does 1M concepts need" is a procurement question that the
    /// incumbents answer by naming a triplestore and shrugging, and because a single-binary
    /// product that quietly needs a hundred gigabytes has broken `CLAUDE.md` §1.5 in a way no
    /// timing would show.
    on_disk: u64,
}

/// Generate a SKOS vocabulary of `size` concepts and write it through the store's write path.
///
/// Returns the fixture and what the load cost. The load number is not incidental: *"how long does
/// it take to get my 400 000 concepts in"* is the first question a migration asks, and it is a
/// question the incumbents answer with a services engagement.
fn load(store: &Store, size: usize) -> (SyntheticVocabulary, LoadReport) {
    let graph = GraphId::vocabulary(format!("https://example.org/openbiz/scale/{size}"))
        .expect("a valid absolute IRI outside the reserved namespace");
    let vocabulary = SyntheticVocabulary {
        scheme: format!("{}/scheme", graph.iri()),
        graph: graph.clone(),
        size,
    };

    store
        .create_vocabulary_graph(&graph)
        .expect("a fresh vocabulary graph");

    let type_of = NamedNode::new_unchecked(RDF_TYPE);
    let in_scheme = NamedNode::new_unchecked(format!("{SKOS}inScheme"));
    let pref_label = NamedNode::new_unchecked(format!("{SKOS}prefLabel"));
    let alt_label = NamedNode::new_unchecked(format!("{SKOS}altLabel"));
    let definition = NamedNode::new_unchecked(format!("{SKOS}definition"));
    let broader = NamedNode::new_unchecked(format!("{SKOS}broader"));
    let has_top_concept = NamedNode::new_unchecked(format!("{SKOS}hasTopConcept"));
    let concept_class: Term = NamedNode::new_unchecked(format!("{SKOS}Concept")).into();
    let scheme_class: Term = NamedNode::new_unchecked(format!("{SKOS}ConceptScheme")).into();
    let scheme_node = NamedNode::new_unchecked(vocabulary.scheme.clone());
    let scheme: Term = scheme_node.clone().into();

    let started = Instant::now();
    let mut written = 0;

    // The scheme states its own top concepts, as a real SKOS vocabulary does. This is not
    // decoration: `top_concepts_stated` below exists to measure what that one modelling choice is
    // worth against `top_concepts_derived`, which has to find them by negation.
    let mut scheme_triples = vec![(scheme_node.clone(), type_of.clone(), scheme_class)];
    for index in 0..TOP_CONCEPTS.min(size) {
        scheme_triples.push((
            scheme_node.clone(),
            has_top_concept.clone(),
            NamedNode::new_unchecked(vocabulary.concept(index)).into(),
        ));
    }
    written += scheme_triples.len();
    store
        .transaction(|transaction| transaction.insert(&graph, scheme_triples))
        .expect("a vocabulary graph accepts its concept scheme");

    for batch in (0..size).step_by(CONCEPTS_PER_TRANSACTION) {
        let end = (batch + CONCEPTS_PER_TRANSACTION).min(size);
        let mut triples = Vec::with_capacity((end - batch) * 7);

        for index in batch..end {
            let subject = NamedNode::new_unchecked(vocabulary.concept(index));
            let label = SyntheticVocabulary::pref_label_en(index);

            triples.push((subject.clone(), type_of.clone(), concept_class.clone()));
            triples.push((subject.clone(), in_scheme.clone(), scheme.clone()));
            triples.push((
                subject.clone(),
                pref_label.clone(),
                Literal::new_language_tagged_literal_unchecked(&label, "en").into(),
            ));
            triples.push((
                subject.clone(),
                pref_label.clone(),
                Literal::new_language_tagged_literal_unchecked(format!("Begriff {index}"), "de")
                    .into(),
            ));
            triples.push((
                subject.clone(),
                alt_label.clone(),
                Literal::new_language_tagged_literal_unchecked(format!("{label} (variant)"), "en")
                    .into(),
            ));
            triples.push((
                subject.clone(),
                definition.clone(),
                Literal::new_language_tagged_literal_unchecked(
                    format!("A synthetic concept, number {index} of {size}, for scale testing."),
                    "en",
                )
                .into(),
            ));

            if let Some(parent) = SyntheticVocabulary::parent(index) {
                triples.push((
                    subject,
                    broader.clone(),
                    NamedNode::new_unchecked(vocabulary.concept(parent)).into(),
                ));
            }
        }

        written += triples.len();
        store
            .transaction(|transaction| transaction.insert(&graph, triples))
            .expect("a vocabulary graph accepts a batch of concepts");
    }

    let elapsed = started.elapsed();
    let on_disk = bytes_on_disk(store.path());

    (
        vocabulary,
        LoadReport {
            quads: written,
            elapsed,
            on_disk,
        },
    )
}

/// Total size of every file under `path`, recursively.
///
/// Read from the filesystem rather than asked of the backend: what an operator provisions is what
/// `du` reports, not what RocksDB believes it is using.
fn bytes_on_disk(path: &std::path::Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| match entry.metadata() {
            Ok(metadata) if metadata.is_dir() => bytes_on_disk(&entry.path()),
            Ok(metadata) => metadata.len(),
            Err(_) => 0,
        })
        .sum()
}

/// One query the interface will issue, and what it should answer.
struct Probe {
    /// Short name, used as the row label in the table.
    name: &'static str,
    /// What a reader of the table needs to know about why this query is here.
    intent: &'static str,
    /// The SPARQL text, exactly as it is sent.
    sparql: String,
    /// How many answers it must produce, derived from the generator's rules.
    expected_answers: u64,
}

/// The queries the concept tree, the search box, and the concept page need.
///
/// Derived from the fixture so each one's expected answer count is a fact about the generator
/// rather than a number copied out of a previous run. A benchmark that does not check its answers
/// will happily report that a query returning zero rows is very fast.
fn probes(vocabulary: &SyntheticVocabulary) -> Vec<Probe> {
    let prefix = format!("PREFIX skos: <{SKOS}>\n");
    let size = vocabulary.size;
    // A leaf at the deepest point of the tree, and a top concept with a large subtree beneath it.
    let deepest = vocabulary.concept(size - 1);
    let root = vocabulary.concept(0);
    let middle = vocabulary.concept(size / 2);
    let middle_label = SyntheticVocabulary::pref_label_en(size / 2);
    let search = SEARCH_ADJECTIVE.to_lowercase();

    vec![
        Probe {
            name: "count_concepts",
            intent: "the header count every vocabulary page shows",
            sparql: format!("{prefix}SELECT (COUNT(?c) AS ?n) WHERE {{ ?c a skos:Concept }}"),
            expected_answers: 1,
        },
        Probe {
            name: "top_concepts_derived",
            intent:
                "the first thing the concept tree draws, found by negation over the whole graph",
            sparql: format!(
                "{prefix}SELECT ?c ?label WHERE {{\n  \
                 ?c a skos:Concept ; skos:prefLabel ?label .\n  \
                 FILTER(lang(?label) = \"en\")\n  \
                 FILTER NOT EXISTS {{ ?c skos:broader ?parent }}\n\
                 }}"
            ),
            expected_answers: TOP_CONCEPTS as u64,
        },
        Probe {
            name: "top_concepts_stated",
            intent: "the same answer, from a scheme that states its top concepts — the mitigation",
            sparql: format!(
                "{prefix}SELECT ?c ?label WHERE {{\n  \
                 <{scheme}> skos:hasTopConcept ?c .\n  \
                 ?c skos:prefLabel ?label .\n  \
                 FILTER(lang(?label) = \"en\")\n\
                 }}",
                scheme = vocabulary.scheme
            ),
            expected_answers: TOP_CONCEPTS.min(size) as u64,
        },
        Probe {
            name: "children",
            intent: "expanding one node of the tree — a lookup in the object position",
            sparql: format!(
                "{prefix}SELECT ?c ?label WHERE {{\n  \
                 ?c skos:broader <{root}> ; skos:prefLabel ?label .\n  \
                 FILTER(lang(?label) = \"en\")\n\
                 }}"
            ),
            expected_answers: BRANCHING.min(size.saturating_sub(TOP_CONCEPTS)) as u64,
        },
        Probe {
            name: "concept_detail",
            intent: "opening one concept — everything stated about a bound subject",
            sparql: format!("{prefix}SELECT ?p ?o WHERE {{ <{middle}> ?p ?o }}"),
            expected_answers: 7,
        },
        Probe {
            name: "label_exact",
            intent: "resolving a label a user pasted in — a bound object literal",
            sparql: format!(
                "{prefix}SELECT ?c WHERE {{ ?c skos:prefLabel \"{middle_label}\"@en }}"
            ),
            expected_answers: 1,
        },
        Probe {
            name: "search_prefix_first_page",
            intent: "what the search box actually sends: a prefix match, one page of results",
            sparql: format!(
                "{prefix}SELECT ?c ?label WHERE {{\n  \
                 ?c skos:prefLabel ?label .\n  \
                 FILTER(STRSTARTS(LCASE(STR(?label)), \"{search}\"))\n\
                 }} LIMIT 50"
            ),
            expected_answers: 50u64.min(vocabulary.search_matches() as u64),
        },
        Probe {
            name: "search_prefix_all",
            intent: "the same search unpaged — the cost the LIMIT is hiding",
            sparql: format!(
                "{prefix}SELECT ?c ?label WHERE {{\n  \
                 ?c skos:prefLabel ?label .\n  \
                 FILTER(STRSTARTS(LCASE(STR(?label)), \"{search}\"))\n\
                 }}"
            ),
            expected_answers: vocabulary.search_matches() as u64,
        },
        Probe {
            name: "ancestors",
            intent: "the breadcrumb above a concept — a transitive path upwards",
            sparql: format!(
                "{prefix}SELECT ?ancestor WHERE {{ <{deepest}> skos:broader+ ?ancestor }}"
            ),
            expected_answers: SyntheticVocabulary::ancestor_count(size - 1) as u64,
        },
        Probe {
            name: "descendants",
            intent: "everything under one branch — the query a bulk edit or a report needs",
            sparql: format!(
                "{prefix}SELECT ?descendant WHERE {{ ?descendant skos:broader+ <{root}> }}"
            ),
            expected_answers: vocabulary.descendant_count(0) as u64,
        },
    ]
}

/// What one probe cost, over several runs.
struct Timing {
    name: &'static str,
    intent: &'static str,
    answers: u64,
    median: Duration,
    slowest: Duration,
}

impl Timing {
    /// Whether the limits this build actually ships would refuse this query outright.
    ///
    /// The distinction the table exists to make. A query that takes four seconds is a design
    /// problem; a query the shipped defaults **refuse** is a broken feature, and the two are
    /// invisible to each other in a plain timings table.
    fn refused_by_shipped_defaults(&self) -> bool {
        self.answers > QueryLimits::DEFAULT_MAX_ANSWERS
            || self.median > QueryLimits::DEFAULT_TIMEOUT
    }
}

/// How many timed runs each probe gets, after one untimed run to warm the page cache.
const RUNS: usize = 3;

/// Time every probe against `store`, checking each one's answer before believing its timing.
fn time_probes(store: &Store, vocabulary: &SyntheticVocabulary) -> Vec<Timing> {
    // Generous on purpose: this measures what the engine can do, and the shipped defaults are
    // then reported *against* those numbers rather than silently truncating them.
    let limits = QueryLimits::new(u64::MAX, Duration::from_secs(600));

    probes(vocabulary)
        .into_iter()
        .map(|probe| {
            let run = || {
                let started = Instant::now();
                let report = store
                    .query(&probe.sparql, QueryFormats::default(), limits, io::sink())
                    .unwrap_or_else(|error| {
                        panic!("{} must evaluate, but failed: {error}", probe.name)
                    });
                (report.answers(), started.elapsed())
            };

            let (answers, _) = run();
            assert_eq!(
                answers, probe.expected_answers,
                "{} answered with {answers} rows, not the {} the fixture guarantees — a timing \
                 for a query that is not doing what it says is worse than no timing at all",
                probe.name, probe.expected_answers
            );

            let mut elapsed: Vec<Duration> = (0..RUNS).map(|_| run().1).collect();
            elapsed.sort_unstable();

            Timing {
                name: probe.name,
                intent: probe.intent,
                answers,
                median: elapsed[RUNS / 2],
                slowest: elapsed[RUNS - 1],
            }
        })
        .collect()
}

/// Load a vocabulary of `size` concepts, time every probe against it, and print the table.
fn measure(size: usize) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let store = Store::open(dir.path()).expect("a fresh store");

    let (vocabulary, load_report) = load(&store, size);
    assert_eq!(
        load_report.quads,
        vocabulary.quad_count(),
        "the loader and the generator's own arithmetic must agree about how much was written"
    );

    let timings = time_probes(&store, &vocabulary);

    println!("\n### {size} concepts\n");
    println!(
        "Loaded {} quads through the transactional write path in {:.1} s ({:.0} quads/s), \
         occupying {:.0} MB on disk.\n",
        load_report.quads,
        load_report.elapsed.as_secs_f64(),
        load_report.quads as f64 / load_report.elapsed.as_secs_f64(),
        load_report.on_disk as f64 / (1024.0 * 1024.0)
    );
    println!("| Query | Answers | Median | Slowest of {RUNS} | Shipped defaults |");
    println!("|---|---:|---:|---:|---|");
    for timing in &timings {
        println!(
            "| `{}` | {} | {:.1} ms | {:.1} ms | {} |",
            timing.name,
            timing.answers,
            timing.median.as_secs_f64() * 1000.0,
            timing.slowest.as_secs_f64() * 1000.0,
            if timing.refused_by_shipped_defaults() {
                "**refused**"
            } else {
                "served"
            }
        );
    }
    println!();
    for timing in &timings {
        println!("- `{}` — {}", timing.name, timing.intent);
    }

    store.close().expect("a clean close");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The harness itself, at a size that costs the ordinary suite a moment.
    ///
    /// This is not a performance assertion — timings on a CI runner are noise. It asserts the
    /// things a timing depends on being true: that the fixture has the shape the generator
    /// claims, and that **every probe returns the rows it is supposed to**. Without this the
    /// ignored benchmarks below could quietly measure nine queries that match nothing.
    #[test]
    fn the_harness_measures_a_vocabulary_whose_shape_it_can_prove() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path()).expect("a fresh store");

        let (vocabulary, report) = load(&store, 1_000);

        assert_eq!(report.quads, 1_000 * 6 + 990 + 1 + TOP_CONCEPTS);
        assert_eq!(report.quads, vocabulary.quad_count());
        assert!(
            report.on_disk > 0,
            "a loaded store must occupy some disk, or the measurement is reading the wrong path"
        );

        // The tree is the shape the depth-sensitive probes assume: ten roots, ten-way branching,
        // and deep enough that `skos:broader+` is walking more than one hop.
        assert_eq!(SyntheticVocabulary::parent(0), None);
        assert_eq!(SyntheticVocabulary::parent(TOP_CONCEPTS - 1), None);
        assert_eq!(SyntheticVocabulary::parent(TOP_CONCEPTS), Some(0));
        assert_eq!(SyntheticVocabulary::ancestor_count(0), 0);
        assert_eq!(
            SyntheticVocabulary::ancestor_count(999),
            2,
            "a thousand concepts under ten-way branching is two levels below the top concepts"
        );

        // `time_probes` asserts every probe's answer count against the generator, so reaching
        // here at all is the check. Keeping the result proves the run was not empty.
        let timings = time_probes(&store, &vocabulary);
        assert_eq!(timings.len(), 10);
        assert!(
            timings.iter().all(|timing| timing.answers > 0),
            "every probe must find something, or its timing means nothing"
        );
        assert!(
            !timings.iter().any(Timing::refused_by_shipped_defaults),
            "nothing at a thousand concepts should come near the shipped limits, so this \
             assertion firing means either the limits moved or the fixture did"
        );

        store.close().expect("a clean close");
    }

    /// The label generator must actually scatter, or the search probes measure a prefix that
    /// matches everything or nothing.
    #[test]
    fn labels_are_lexically_diverse_and_the_search_prefix_matches_a_known_fraction() {
        let graph = GraphId::vocabulary("https://example.org/openbiz/scale/labels").expect("valid");
        let vocabulary = SyntheticVocabulary {
            scheme: format!("{}/scheme", graph.iri()),
            graph,
            size: 1_700,
        };

        assert_eq!(
            vocabulary.search_matches(),
            100,
            "one in seventeen concepts carries the searched adjective"
        );
        assert!(SyntheticVocabulary::pref_label_en(16).starts_with(SEARCH_ADJECTIVE));
        assert!(!SyntheticVocabulary::pref_label_en(17).starts_with(SEARCH_ADJECTIVE));
        assert_ne!(
            SyntheticVocabulary::pref_label_en(0),
            SyntheticVocabulary::pref_label_en(17),
            "labels seventeen apart share an adjective and must still differ"
        );
    }

    #[test]
    #[ignore = "minutes of work; run deliberately in release, see the module documentation"]
    fn scale_10k() {
        measure(10_000);
    }

    #[test]
    #[ignore = "minutes of work; run deliberately in release, see the module documentation"]
    fn scale_100k() {
        measure(100_000);
    }

    #[test]
    #[ignore = "minutes of work; run deliberately in release, see the module documentation"]
    fn scale_1m() {
        measure(1_000_000);
    }
}
