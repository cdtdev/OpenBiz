//! What the semantic relation model costs at the sizes an enterprise thesaurus reaches — and what
//! S24's closure *would* cost on top of it, counted before it is built rather than after.
//!
//! # Why this module exists before the closure does
//!
//! Iteration 24 landed S22, S23, S25 and S26 and opened an entry in `docs/UNTESTED.md` saying, in
//! substance: this is the first thing the core model holds that grows with a vocabulary's **size**
//! rather than with its structure, four `(Node, RelationOrigin)` entries and three derivations per
//! stated link, and nobody has measured the ceiling. The build plan's next item is S24, whose
//! closure is superlinear in exactly the same data. Measuring afterwards would mean choosing the
//! architecture and then discovering the number, which is the wrong order.
//!
//! So this harness answers two separate questions:
//!
//! 1. **What does the model already hold?** Time, resident memory, held link entries, derivations,
//!    and the size of the report `openbiz inspect` renders from them.
//! 2. **What would S24 add?** The size of the `skos:broaderTransitive` closure, **counted by
//!    traversal without materialising it**, so the number can be known without first building the
//!    thing the number might forbid.
//!
//! The decision those numbers produced is in `docs/adr/0024-semantic-relation-closure-scale.md`.
//!
//! # The shapes, and why three of them
//!
//! A single "realistic" shape would answer the easy question and miss the one that matters. The
//! closure's size is a property of the hierarchy's *shape*, not of its link count, and the three
//! here span the range SKOS permits:
//!
//! - [`Shape::Tree`] — a balanced ten-way tree. What a real thesaurus mostly looks like: broad and
//!   shallow, depth growing with the logarithm of the size. The closure is a small multiple of the
//!   stated links.
//! - [`Shape::Star`] — one root, every other concept directly beneath it. The **floor**: depth 1,
//!   so the closure adds nothing at all. Included because a measurement with no floor cannot say
//!   how much of the cost is the shape and how much is the size.
//! - [`Shape::Chain`] — `<c1> broader <c2> broader <c3> …`, one long ladder. The **ceiling**:
//!   n(n−1)/2 closure pairs from n−1 stated links. This is not a realistic thesaurus and is not
//!   meant to be. It is a *legal* SKOS graph — §8 states no condition against depth — and a model
//!   that cannot survive a legal input has a defect, not an edge case.
//!
//! # The honest limits of what this measures
//!
//! **The vocabulary is synthetic and regular.** Uniform IRIs of a fixed length, no labels, no
//! notes, no schemes, every concept typed. That isolates the relation cost, which is the point,
//! but it means these numbers are the relation model's cost and *not* a whole vocabulary's. A real
//! thesaurus carries labels in six languages and the memory those cost is measured nowhere here.
//!
//! **Resident memory is a Linux number and an allocator's opinion.** [`resident_bytes`] reads
//! `VmRSS` from `/proc/self/status`; on any other platform it returns `None` and the memory column
//! is absent rather than wrong. Even on Linux, glibc need not return freed pages to the kernel, so
//! the delta across a build is an *upper* bound on what the model holds and a *lower* bound on what
//! the process needed. Both are recorded, and neither is presented as the other.
//!
//! **It is one process on one machine with no concurrent load**, exactly as
//! `openbiz-store`'s `scale` module says of itself. `CLAUDE.md` §8 puts hardware-bound load testing
//! outside the loop.
//!
//! # Running it
//!
//! The small case runs in the ordinary suite, so the harness cannot rot unnoticed, and it asserts
//! the *shape* of the fixture and the *arithmetic* of the model — a benchmark whose generator
//! silently produced an empty graph would measure nothing very quickly indeed.
//!
//! The real sizes are `#[ignore]`d and must be run in release; a debug-built `BTreeMap` measures
//! the profile, not the design.
//!
//! ```text
//! cargo test --release -p openbiz-skos -- --ignored --nocapture --test-threads=1
//! ```

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::time::{Duration, Instant};

use crate::model::{CoreModel, Node, SkosClass, Statement, RDF_TYPE};
use crate::relations::{SemanticRelation, SKOS_BROADER, SKOS_RELATED};

/// Whether a generated vocabulary states associative links — and so whether §8.4's disjointness
/// pass runs at all.
///
/// **Every row measured before iteration 30 was [`Associativity::None`]**, which is why the S27
/// pass cost nothing in any of them: `check_semantic_relation_disjointness` walks once per concept
/// that has a `skos:related`, and no fixture in the repository had one. The harness was measuring
/// the model the pass reads and never the pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Associativity {
    /// No `skos:related` anywhere. The baseline that makes the S27 column subtractable.
    None,
    /// Every concept `skos:related` to a resource **outside** the hierarchy.
    ///
    /// Detached on purpose. The pass walks a concept's whole ancestry before it looks at a single
    /// associate, so the walk costs the same whether the associate clashes or not; what differs is
    /// how many `Finding`s come back. An in-hierarchy associate in a chain violates S27 every time,
    /// and each violation holds its entire path — a second and quite separate cost, measured by
    /// [`Associativity::EveryConceptInHierarchy`] rather than confounded with this one.
    EveryConceptDetached,
    /// Every concept `skos:related` to its own grandparent, which is an S27 violation by
    /// construction: the grandparent is `skos:broaderTransitive` of the concept.
    ///
    /// This measures what a *violation* costs on top of the walk, but note what it does **not**
    /// measure: `path_to` is breadth-first, so the path a grandparent clash carries is three nodes
    /// however deep the hierarchy is. A clash against a concept far above — every concept related
    /// to the root — would hold paths quadratic in the depth, and no shape here generates one.
    /// That gap is recorded in `docs/UNTESTED.md` rather than implied to be covered.
    EveryConceptInHierarchy,
}

impl Associativity {
    fn name(self) -> &'static str {
        match self {
            Associativity::None => "none",
            Associativity::EveryConceptDetached => "detached",
            Associativity::EveryConceptInHierarchy => "in-tree",
        }
    }
}

/// The resource concept `index` is associated with, outside the hierarchy.
fn associate_iri(index: usize) -> String {
    format!("http://example.org/vocabulary/associate-{index:09}")
}

/// What concept `index` states `skos:related` to, or `None` for no associative link.
///
/// The in-hierarchy case picks the concept's **grandparent**, which is an S27 violation in every
/// shape that has one — and `None` where the shape has no grandparent to reach, so a star (depth 1)
/// and the baseline generate no violations at all. That is the correct answer rather than a gap:
/// a shallow hierarchy genuinely cannot violate S27 two steps up.
fn associate_of(shape: Shape, associativity: Associativity, index: usize) -> Option<Node> {
    match associativity {
        Associativity::None => None,
        Associativity::EveryConceptDetached => Some(Node::iri(associate_iri(index))),
        Associativity::EveryConceptInHierarchy => {
            let parent = shape.parent_of(index)?;
            let grandparent = shape.parent_of(parent)?;
            Some(Node::iri(concept_iri(grandparent)))
        }
    }
}

/// The hierarchy a generated vocabulary has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// No hierarchy at all: every concept typed, not one link between them. The **baseline**,
    /// which is what makes the other rows subtractable — without it the table says what a
    /// vocabulary costs and cannot say what the *relations* cost, and those are the two different
    /// numbers the decision needs.
    Detached,
    /// A balanced tree of the given branching factor. Realistic.
    Tree { branching: usize },
    /// One root with every other concept directly beneath it. The shallowest legal hierarchy.
    Star,
    /// A single ladder. The deepest legal hierarchy, and the closure's worst case.
    Chain,
}

impl Shape {
    fn name(self) -> &'static str {
        match self {
            Shape::Detached => "none",
            Shape::Tree { .. } => "tree",
            Shape::Star => "star",
            Shape::Chain => "chain",
        }
    }

    /// The parent of concept `index`, or `None` for the root.
    ///
    /// Concept 0 is the root in every shape, so every generated vocabulary is a single connected
    /// hierarchy with exactly `concepts - 1` stated links. That keeps the link count comparable
    /// across shapes: what differs between the rows of the table is the depth and nothing else.
    fn parent_of(self, index: usize) -> Option<usize> {
        if index == 0 {
            return None;
        }
        match self {
            Shape::Detached => None,
            Shape::Tree { branching } => Some((index - 1) / branching),
            Shape::Star => Some(0),
            Shape::Chain => Some(index - 1),
        }
    }
}

/// One row of the table.
#[derive(Debug)]
struct Measurement {
    shape: Shape,
    associativity: Associativity,
    concepts: usize,
    /// `skos:broader` statements the generator wrote.
    stated_links: usize,
    /// `skos:related` statements the generator wrote. One per concept, or none.
    related_links: usize,
    /// Findings the build produced. For an in-hierarchy associate every one of them is an S27
    /// violation carrying its whole path, so this column and the memory column move together.
    findings: usize,
    build: Duration,
    /// `VmRSS` before the build, and after it. `None` off Linux.
    rss_before: Option<u64>,
    rss_after: Option<u64>,
    /// Peak `VmHWM` for the process at the end of the run. `None` off Linux.
    rss_peak: Option<u64>,
    /// Every `(Node, RelationOrigin)` the model holds, summed over resources and relations.
    held_entries: usize,
    derivations: usize,
    /// Bytes the `why:` section of `openbiz inspect` would render from those derivations.
    report_bytes: usize,
    /// The number of `<x> skos:broaderTransitive <y>` pairs S24 would license, counted by
    /// traversal. `None` when the count itself was refused as too expensive — see
    /// [`count_closure`].
    closure_pairs: Option<u64>,
    closure_count: Duration,
}

impl Measurement {
    /// The multiple of the stated links S24 would add. The number the decision turns on.
    fn closure_multiple(&self) -> Option<f64> {
        let pairs = self.closure_pairs?;
        (self.stated_links > 0).then(|| pairs as f64 / self.stated_links as f64)
    }

    fn print(&self) {
        let mib = |bytes: Option<u64>| {
            bytes.map_or_else(
                || "     n/a".to_string(),
                |value| format!("{:8.1}", value as f64 / (1024.0 * 1024.0)),
            )
        };
        let held = match (self.rss_before, self.rss_after) {
            (Some(before), Some(after)) => {
                format!(
                    "{:8.1}",
                    after.saturating_sub(before) as f64 / (1024.0 * 1024.0)
                )
            }
            _ => "     n/a".to_string(),
        };
        println!(
            "{shape:<6} assoc {assoc:<8} {concepts:>9} concepts {links:>9} links \
             {related:>9} related | build {build:>8.2?} | \
             rss +{held} MiB peak {peak} MiB | held {entries:>10} | deriv {deriv:>10} | \
             findings {findings:>9} | \
             report {report:>8.1} MiB | closure {closure:>14} ({multiple}) in {count:>8.2?}",
            shape = self.shape.name(),
            assoc = self.associativity.name(),
            concepts = self.concepts,
            links = self.stated_links,
            related = self.related_links,
            findings = self.findings,
            build = self.build,
            held = held,
            peak = mib(self.rss_peak),
            entries = self.held_entries,
            deriv = self.derivations,
            report = self.report_bytes as f64 / (1024.0 * 1024.0),
            closure = self
                .closure_pairs
                .map_or_else(|| "not counted".to_string(), |pairs| pairs.to_string()),
            multiple = self
                .closure_multiple()
                .map_or_else(|| "-".to_string(), |multiple| format!("{multiple:.1}x")),
            count = self.closure_count,
        );
    }
}

/// The process's current resident set size in bytes, or `None` where the kernel does not say.
///
/// Reads `VmRSS` from `/proc/self/status`. Deliberately not a dependency: a benchmark that pulls a
/// crate into the workspace to weigh itself has made the thing it measures heavier, and
/// `CLAUDE.md` §1.5 calls every dependency a liability.
fn resident_bytes() -> Option<u64> {
    proc_status_field("VmRSS:")
}

/// The process's peak resident set size in bytes — `VmHWM`, which the kernel never lowers.
fn peak_resident_bytes() -> Option<u64> {
    proc_status_field("VmHWM:")
}

fn proc_status_field(field: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with(field))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

/// A concept's IRI. Fixed-width so the memory a row reports is not an artefact of the numbering.
fn concept_iri(index: usize) -> String {
    format!("http://example.org/vocabulary/concept-{index:09}")
}

/// Build the model for `concepts` concepts in `shape`, measuring as it goes.
///
/// The statements are pushed as they are generated rather than collected into a `Vec` first, so
/// the resident-memory delta is the *model* and not the model plus a copy of its input. That is
/// also how `openbiz inspect` feeds the builder — it streams out of the store one statement at a
/// time — so the harness and the production caller hold the same thing.
fn measure(
    shape: Shape,
    concepts: usize,
    associativity: Associativity,
    closure_budget: u64,
) -> Measurement {
    let rss_before = resident_bytes();
    let started = Instant::now();

    let mut builder = CoreModel::builder();
    let mut stated_links = 0;
    let mut related_links = 0;
    for index in 0..concepts {
        let node = Node::iri(concept_iri(index));
        builder.push(Statement::new(
            node.clone(),
            RDF_TYPE,
            Node::iri(SkosClass::Concept.iri()),
        ));
        if let Some(parent) = shape.parent_of(index) {
            builder.push(Statement::new(
                node.clone(),
                SKOS_BROADER,
                Node::iri(concept_iri(parent)),
            ));
            stated_links += 1;
        }
        if let Some(associate) = associate_of(shape, associativity, index) {
            builder.push(Statement::new(node, SKOS_RELATED, associate));
            related_links += 1;
        }
    }
    let model = builder.build();

    let build = started.elapsed();
    let rss_after = resident_bytes();

    let held_entries: usize = model
        .resources()
        .map(|(_, resource)| {
            resource
                .semantic_relations()
                .values()
                .map(BTreeMap::len)
                .sum::<usize>()
        })
        .sum();

    let report_bytes = derivation_report_bytes(&model);

    let closure_started = Instant::now();
    let closure_pairs = count_closure(&model, closure_budget);
    let closure_count = closure_started.elapsed();

    Measurement {
        shape,
        associativity,
        concepts,
        stated_links,
        related_links,
        findings: model.findings().len(),
        build,
        rss_before,
        rss_after,
        rss_peak: peak_resident_bytes(),
        held_entries,
        derivations: model.derivations().len(),
        report_bytes,
        closure_pairs,
        closure_count,
    }
}

/// The bytes `openbiz inspect` would emit for its `why:` section.
///
/// Rendered the way `inspect` renders it — `"  {derivation}\n"` — because the question this
/// answers is about the operator's terminal, not about an abstraction. The crates do not depend on
/// each other, so the format is duplicated in one line here and pinned by
/// [`the_report_estimate_matches_how_inspect_renders_a_derivation`].
fn derivation_report_bytes(model: &CoreModel) -> usize {
    let mut line = String::new();
    let mut total = 0;
    for derivation in model.derivations() {
        line.clear();
        // Writing into a `String` cannot fail; the result is discarded rather than unwrapped.
        let _ = writeln!(line, "  {derivation}");
        total += line.len();
    }
    total
}

/// How many `<x> skos:broaderTransitive <y>` pairs S24 would license — counted, never held.
///
/// One breadth-first walk up `skos:broader` per concept, counting the distinct concepts reached.
/// The visited set is per-concept and dropped between walks, so the peak memory of the *count* is
/// one concept's ancestor set rather than the whole closure. That is the entire trick: it makes the
/// size of a structure knowable without paying for the structure.
///
/// **Cycles terminate.** A vocabulary with `<A> broader <B> broader <A>` is legal SKOS — §8 states
/// no condition against a cycle — and the visited set is what stops the walk, not a depth limit.
/// A concept in a cycle reaches every other concept in it, which is the correct answer.
///
/// Returns `None` once the running total passes `budget`. The count is itself proportional to the
/// closure, so a shape whose closure is quadratic has a quadratic *count*, and running it to
/// completion at every size would make the harness the slowest thing in the repository. A refusal
/// with the budget printed beside it is a more useful measurement than a number that took an hour,
/// and it is recorded as a refusal rather than as a zero.
fn count_closure(model: &CoreModel, budget: u64) -> Option<u64> {
    let mut broader: BTreeMap<&Node, Vec<&Node>> = BTreeMap::new();
    for (node, resource) in model.resources() {
        if let Some(links) = resource.relations(SemanticRelation::Broader) {
            broader.insert(node, links.keys().collect());
        }
    }

    let mut total: u64 = 0;
    let mut seen: BTreeSet<&Node> = BTreeSet::new();
    let mut queue: VecDeque<&Node> = VecDeque::new();
    for start in model.resources().map(|(node, _)| node) {
        seen.clear();
        queue.clear();
        queue.push_back(start);
        while let Some(node) = queue.pop_front() {
            for &parent in broader.get(node).into_iter().flatten() {
                if seen.insert(parent) {
                    total += 1;
                    if total > budget {
                        return None;
                    }
                    queue.push_back(parent);
                }
            }
        }
    }
    Some(total)
}

/// No budget: used where the shape's closure is known to be small.
const UNBOUNDED: u64 = u64::MAX;

/// The largest closure this harness will count before refusing. Twenty million pairs is a few
/// seconds in release and is far past anything the model could hold, so a shape that exceeds it
/// has already answered the question.
const COUNT_BUDGET: u64 = 20_000_000;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Derivation, SkosRule};
    use crate::relations::RelationOrigin;

    /// The generator must produce the hierarchy it claims to, or every row of the table is a
    /// measurement of the wrong graph. Checked at a size small enough to reason about by hand.
    #[test]
    fn each_shape_generates_the_hierarchy_it_names() {
        // Tree, branching 2, seven concepts: 0 over 1 and 2, 1 over 3 and 4, 2 over 5 and 6.
        let tree = Shape::Tree { branching: 2 };
        assert_eq!(tree.parent_of(0), None);
        assert_eq!(tree.parent_of(1), Some(0));
        assert_eq!(tree.parent_of(2), Some(0));
        assert_eq!(tree.parent_of(5), Some(2));
        assert_eq!(tree.parent_of(6), Some(2));

        for index in 0..8 {
            assert_eq!(
                Shape::Detached.parent_of(index),
                None,
                "the baseline has no links"
            );
        }

        assert_eq!(Shape::Star.parent_of(0), None);
        for index in 1..8 {
            assert_eq!(Shape::Star.parent_of(index), Some(0));
        }

        assert_eq!(Shape::Chain.parent_of(0), None);
        for index in 1..8 {
            assert_eq!(Shape::Chain.parent_of(index), Some(index - 1));
        }
    }

    /// The closure count must be exact, not approximate, or the decision rests on an estimate.
    /// Three shapes with closures that can be computed in the head:
    ///
    /// - a chain of n concepts has n(n−1)/2 ancestor pairs;
    /// - a star of n concepts has n−1, one per leaf;
    /// - a balanced binary tree of seven concepts has 0 + 1 + 1 + 2 + 2 + 2 + 2 = 10.
    #[test]
    fn the_closure_count_matches_the_arithmetic_of_each_shape() {
        let chain = measure(Shape::Chain, 10, Associativity::None, UNBOUNDED);
        assert_eq!(chain.closure_pairs, Some(45), "10 * 9 / 2");

        let star = measure(Shape::Star, 10, Associativity::None, UNBOUNDED);
        assert_eq!(star.closure_pairs, Some(9), "one ancestor per leaf");

        let tree = measure(
            Shape::Tree { branching: 2 },
            7,
            Associativity::None,
            UNBOUNDED,
        );
        assert_eq!(tree.closure_pairs, Some(10));
    }

    /// A cycle is legal SKOS and the count must terminate on one, returning the answer rather than
    /// a depth-limited guess. Three concepts in a ring each reach the other two, so the closure is
    /// nine pairs — every concept reaches every concept, itself included.
    #[test]
    fn the_closure_count_terminates_on_a_cycle_and_counts_it() {
        let ring: Vec<Statement> = (0..3)
            .map(|index| {
                Statement::new(
                    Node::iri(concept_iri(index)),
                    SKOS_BROADER,
                    Node::iri(concept_iri((index + 1) % 3)),
                )
            })
            .collect();
        let model = CoreModel::from_statements(ring);
        assert_eq!(count_closure(&model, UNBOUNDED), Some(9));
    }

    /// The budget is a refusal, not a zero. A caller that read `Some(0)` from an abandoned count
    /// would record "the closure is empty" for the shape whose closure is largest.
    #[test]
    fn a_closure_larger_than_the_budget_is_refused_rather_than_truncated() {
        let model = CoreModel::from_statements((1..50).map(|index| {
            Statement::new(
                Node::iri(concept_iri(index)),
                SKOS_BROADER,
                Node::iri(concept_iri(index - 1)),
            )
        }));
        assert_eq!(count_closure(&model, 10), None);
        assert_eq!(count_closure(&model, UNBOUNDED), Some(50 * 49 / 2));
    }

    /// The arithmetic `docs/UNTESTED.md` states — four held entries and three derivations per
    /// stated `skos:broader` link — pinned at a size the ignored runs then multiply. If the
    /// closure passes ever change this ratio, the table's units change with it and this is where
    /// that shows up.
    #[test]
    fn the_model_holds_four_entries_and_three_derivations_per_stated_link() {
        let measured = measure(
            Shape::Tree { branching: 10 },
            1_000,
            Associativity::None,
            UNBOUNDED,
        );
        assert_eq!(measured.stated_links, 999);
        assert_eq!(measured.held_entries, 999 * 4);
        assert_eq!(measured.derivations, 999 * 3);
        assert!(
            measured.report_bytes > 999 * 3 * 100,
            "each derivation renders on three lines with the statement's own text: {} bytes",
            measured.report_bytes
        );
        measured.print();
    }

    /// The report estimate must render a derivation exactly as `openbiz inspect` does, or the
    /// megabytes in the ADR are about a format nobody sees. `inspect` writes `"  {derivation}\n"`;
    /// so does [`derivation_report_bytes`], and this is the assertion that keeps them equal.
    #[test]
    fn the_report_estimate_matches_how_inspect_renders_a_derivation() {
        let derivation = Derivation {
            conclusion: "<a> skos:narrower <b>".to_string(),
            premise: "<b> skos:broader <a>".to_string(),
            rule: SkosRule::S25,
        };
        let rendered = format!("  {derivation}\n");
        let model = CoreModel::from_statements([Statement::new(
            Node::iri(concept_iri(1)),
            SKOS_BROADER,
            Node::iri(concept_iri(0)),
        )]);
        let one: usize = model
            .derivations()
            .iter()
            .map(|derivation| format!("  {derivation}\n").len())
            .sum();
        assert_eq!(derivation_report_bytes(&model), one);
        assert!(
            rendered.lines().count() == 3,
            "a derivation is three lines, which is why the report is larger than it reads"
        );
    }

    /// The small case of the real harness, in the ordinary suite so the generator, the closure
    /// count, and the printer cannot rot between release runs. It asserts the answer of every
    /// shape rather than merely running them: a benchmark that silently measured an empty model
    /// would report excellent numbers.
    #[test]
    fn the_harness_measures_every_shape_at_a_size_the_suite_can_afford() {
        for shape in [Shape::Tree { branching: 10 }, Shape::Star, Shape::Chain] {
            let measured = measure(shape, 500, Associativity::None, COUNT_BUDGET);
            assert_eq!(measured.stated_links, 499);
            assert_eq!(measured.held_entries, 499 * 4);
            assert_eq!(measured.derivations, 499 * 3);
            assert!(measured.closure_pairs.is_some(), "{}", shape.name());
            measured.print();
        }

        // The baseline states nothing, so it must hold nothing: no links, no entailments, no
        // closure. A generator that quietly linked them anyway would make every subtraction in
        // the ADR wrong in the same direction.
        let baseline = measure(Shape::Detached, 500, Associativity::None, UNBOUNDED);
        assert_eq!(baseline.stated_links, 0);
        assert_eq!(baseline.held_entries, 0);
        assert_eq!(baseline.derivations, 0);
        assert_eq!(baseline.closure_pairs, Some(0));
        assert_eq!(baseline.report_bytes, 0);
        baseline.print();
    }

    /// One asserted link, one entailed under each of S25 and S22 twice over — the four the
    /// `UNTESTED.md` entry names, each with the origin that says which it is. Held here rather
    /// than only in `model.rs` because [`Measurement::held_entries`] counts them without looking
    /// at what they are, and a count that included an asserted link twice would still be four.
    #[test]
    fn the_four_held_entries_are_one_asserted_link_and_three_entailments() {
        let model = CoreModel::from_statements([Statement::new(
            Node::iri(concept_iri(1)),
            SKOS_BROADER,
            Node::iri(concept_iri(0)),
        )]);
        let child = Node::iri(concept_iri(1));
        let parent = Node::iri(concept_iri(0));
        let origin = |node: &Node, relation, other: &Node| {
            model
                .resource(node)
                .and_then(|resource| resource.relations(relation))
                .and_then(|links| links.get(other))
                .copied()
        };
        assert_eq!(
            origin(&child, SemanticRelation::Broader, &parent),
            Some(RelationOrigin::Asserted)
        );
        assert_eq!(
            origin(&parent, SemanticRelation::Narrower, &child),
            Some(RelationOrigin::Entailed(SkosRule::S25))
        );
        assert_eq!(
            origin(&child, SemanticRelation::BroaderTransitive, &parent),
            Some(RelationOrigin::Entailed(SkosRule::S22))
        );
        assert_eq!(
            origin(&parent, SemanticRelation::NarrowerTransitive, &child),
            Some(RelationOrigin::Entailed(SkosRule::S22))
        );
    }

    /// The baseline row: what a typed concept costs before a single link is stated. Subtracting
    /// it from the tree rows is the only way to say how much of the model's memory the semantic
    /// relations are actually responsible for, which is the question the decision turns on.
    #[test]
    #[ignore = "a million concepts; run in release with --ignored"]
    fn what_a_vocabulary_costs_with_no_relations_at_all() {
        for concepts in [100_001, 1_000_001] {
            let measured = measure(Shape::Detached, concepts, Associativity::None, UNBOUNDED);
            assert_eq!(measured.stated_links, 0);
            assert_eq!(measured.held_entries, 0);
            assert_eq!(measured.derivations, 0);
            assert_eq!(measured.closure_pairs, Some(0));
            measured.print();
        }
    }

    /// The real table. Release only, and `#[ignore]`d because a million concepts is minutes of
    /// work and CI is not where that belongs. The numbers it printed are in
    /// `docs/adr/0024-semantic-relation-closure-scale.md`.
    #[test]
    #[ignore = "minutes of work and gigabytes of memory; run in release with --ignored"]
    fn the_relation_model_at_ten_thousand_a_hundred_thousand_and_a_million_links() {
        for concepts in [10_001, 100_001, 1_000_001] {
            measure(
                Shape::Tree { branching: 10 },
                concepts,
                Associativity::None,
                COUNT_BUDGET,
            )
            .print();
        }
    }

    /// What the shape costs, at one size, across the whole range SKOS permits. The chain row is
    /// the one the decision turns on.
    #[test]
    #[ignore = "the chain's closure is quadratic; run in release with --ignored"]
    fn the_closure_at_ten_thousand_links_from_the_shallowest_shape_to_the_deepest() {
        for shape in [Shape::Star, Shape::Tree { branching: 10 }, Shape::Chain] {
            measure(shape, 10_001, Associativity::None, COUNT_BUDGET).print();
        }
    }

    /// How fast the deepest legal shape leaves the affordable range. Each step multiplies the
    /// stated links by ten and the closure by a hundred.
    #[test]
    #[ignore = "quadratic by construction; run in release with --ignored"]
    fn the_deepest_legal_shape_at_each_order_of_magnitude() {
        for concepts in [1_001, 10_001, 100_001] {
            measure(Shape::Chain, concepts, Associativity::None, COUNT_BUDGET).print();
        }
    }

    /// The associative generator must state the links it claims, or the S27 column measures a pass
    /// that never ran — which is precisely the gap this whole dimension exists to close.
    #[test]
    fn the_associative_generator_states_a_related_link_per_concept() {
        let detached = measure(
            Shape::Chain,
            50,
            Associativity::EveryConceptDetached,
            UNBOUNDED,
        );
        assert_eq!(detached.stated_links, 49);
        assert_eq!(
            detached.related_links, 50,
            "one per concept, hierarchy or not"
        );
        assert_eq!(
            detached.findings, 0,
            "an associate outside the hierarchy cannot be above the concept: {:?}",
            detached.findings
        );

        // The in-hierarchy case relates each concept to its grandparent, so every concept with a
        // grandparent violates S27. In a 50-long chain that is concepts 2..50.
        let in_tree = measure(
            Shape::Chain,
            50,
            Associativity::EveryConceptInHierarchy,
            UNBOUNDED,
        );
        assert_eq!(in_tree.related_links, 48);
        assert_eq!(
            in_tree.findings, 48,
            "every grandparent link is a clash S27 forbids"
        );

        // A star has depth 1, so nothing in it has a grandparent and nothing can violate S27 this
        // way. The generator must produce no associative links at all rather than silently
        // relating a concept to itself.
        let star = measure(
            Shape::Star,
            50,
            Associativity::EveryConceptInHierarchy,
            UNBOUNDED,
        );
        assert_eq!(star.related_links, 0);
        assert_eq!(star.findings, 0);
    }

    /// **What §8.4's pass costs, which nothing measured until iteration 30.**
    ///
    /// `check_semantic_relation_disjointness` runs one full ancestry walk per concept that has a
    /// `skos:related`. The walk is bounded; the *pass* is one walk per concept and the bound does
    /// not compose, so the pass's cost is the number of associated concepts times the depth of the
    /// hierarchy. In a tree that is `n log n` and invisible. In a chain it is quadratic, and the
    /// row for the chain is the one this test exists to print.
    ///
    /// Read it against the `none` row of the same shape and size: the difference is the pass.
    #[test]
    #[ignore = "quadratic in the chain row by construction; run in release with --ignored"]
    fn the_s27_pass_at_each_shape_with_an_associative_link_on_every_concept() {
        for shape in [Shape::Star, Shape::Tree { branching: 10 }, Shape::Chain] {
            for associativity in [Associativity::None, Associativity::EveryConceptDetached] {
                measure(shape, 10_001, associativity, COUNT_BUDGET).print();
            }
        }
    }

    /// How fast the pass leaves the affordable range as the deepest legal shape grows. Each step
    /// multiplies the concepts by ten and the pass's work by a hundred.
    #[test]
    #[ignore = "quadratic by construction; run in release with --ignored"]
    fn the_s27_pass_on_the_deepest_legal_shape_at_each_order_of_magnitude() {
        for concepts in [1_001, 10_001, 100_001] {
            measure(
                Shape::Chain,
                concepts,
                Associativity::EveryConceptDetached,
                COUNT_BUDGET,
            )
            .print();
        }
    }

    /// What a *violation* costs on top of the walk: every finding carries the whole path that
    /// proved it, so a deep hierarchy with a clash on every concept holds paths quadratic in its
    /// depth. Printed beside the detached row of the same size, which walks identically and holds
    /// no paths at all.
    #[test]
    #[ignore = "holds a path per finding; run in release with --ignored"]
    fn what_an_s27_violation_costs_when_every_concept_has_one() {
        for concepts in [1_001, 10_001] {
            measure(
                Shape::Chain,
                concepts,
                Associativity::EveryConceptInHierarchy,
                COUNT_BUDGET,
            )
            .print();
        }
    }
}
