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
//! # The fourth and fifth shapes, and the six iterations that asked for them
//!
//! Every shape above is a **monohierarchy**: one parent per concept, so exactly one route from any
//! concept to the summit. That was never stated as a limitation and it silently decided six
//! measurements. Iterations 31 to 36 each closed one rule and each recorded, in a different rule's
//! words, the same finding — the generator only ever produces the easy shape — and iteration 36
//! reached the case where it is not merely unhelpful: [`CoreModel::paths_to_root`] enumerates every
//! route to a root, [`PathBound::max_paths`] exists to stop that enumeration from being
//! exponential, and **a hierarchy with one parent per concept has exactly one route**, so the whole
//! module's central bound was unmeasurable in principle from anything this harness could build.
//!
//! - [`Shape::Polytree`] — a balanced tree in which a **share** of the concepts state more than one
//!   `skos:broader`. This is what a real thesaurus is: iteration 37 measured LC Genre/Form Terms at
//!   **25.8% of concepts with more than one broader concept, maximum 4**, and AGROVOC at 1.1% with
//!   two and none with three. The share and the width are parameters so both can be built, and
//!   [`LCGFT_SHARE`] and [`LCGFT_WIDEST`] name the measured ones rather than leaving them as
//!   numerals in a test.
//! - [`Shape::Lattice`] — levels of `width` concepts, every concept linked to **every** concept in
//!   the level above. The **route ceiling**, and the counterpart of [`Shape::Chain`]: `width`
//!   routes multiply at every level, so a concept `L` levels down has `width^(L-1)` routes to the
//!   summit while having only `width * L` ancestors. This is the shape [`PathBound::max_paths`]'s
//!   own documentation describes and nothing here could generate — and it is how a vocabulary that
//!   is *tiny* by every other measure exhausts a bound sized for a hundred thousand concepts.
//!
//! # The honest limits of what this measures
//!
//! **The vocabulary is synthetic and regular.** Uniform IRIs of a fixed length, no labels, no
//! notes, no schemes, every concept typed. That isolates the relation cost, which is the point,
//! but it means these numbers are the relation model's cost and *not* a whole vocabulary's. A real
//! thesaurus carries labels in six languages and the memory those cost is measured nowhere here.
//!
//! **Branching is the axis this module gained at iteration 40, and it is one of six.** Labels,
//! notes, mapping properties, `rdfs:subPropertyOf` refinements, and dense `skos:related` clusters
//! are still generated by nothing, so every command that reads those — `openbiz search` above all,
//! which scans every label linearly — is measured here at exactly zero of them. Those gaps are in
//! `docs/UNTESTED.md` under their own names and are not implied to be covered by this one.
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
use crate::paths::PathBound;
use crate::relations::{SemanticRelation, SKOS_BROADER, SKOS_RELATED};

/// One concept in every four states a second `skos:broader`.
///
/// Iteration 37 counted LC Genre/Form Terms — 2 685 concepts, fetched and counted rather than
/// cited — and found **25.8%** of them with more than one broader concept. This is that share,
/// rounded to a period the generator can express exactly. It is the *ordinary* case: a real
/// thesaurus is polyhierarchic and every number this harness printed before iteration 40 was
/// measured on a graph that was not.
const LCGFT_SHARE: usize = 4;

/// Three additional `skos:broader` links on every concept the share selects.
///
/// LCGFT's **maximum** was 4 broader concepts on one concept; this puts that maximum on the whole
/// quarter rather than on the one concept that had it. Deliberately worse than the measured
/// vocabulary, because a bound that survives the exaggeration is a bound, and one that only
/// survives the average is a coincidence.
const LCGFT_WIDEST: usize = 3;

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
            let parent = shape.primary_parent_of(index)?;
            let grandparent = shape.primary_parent_of(parent)?;
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
    /// A balanced tree in which every `period`-th concept states `extra` further broader concepts.
    ///
    /// **Realistic, which none of the shapes above is.** The extra parents are always at a
    /// *strictly smaller index* than the primary one, which is what keeps the graph acyclic
    /// without a check: every link points at a concept generated earlier. A concept too near the
    /// root to have `extra` distinct earlier concepts to point at simply gets fewer, so the share
    /// the model ends up holding is a little below `1/period` — [`Measurement::polyhierarchic`]
    /// reports what was built rather than what was asked for, because those are different numbers
    /// and only one of them is evidence.
    Polytree {
        branching: usize,
        period: usize,
        extra: usize,
    },
    /// Levels of `width` concepts, each linked to **every** concept in the level above.
    ///
    /// The route ceiling. A concept `L` levels below the summit has `width^(L-1)` routes to it and
    /// only `width * (L-1) + 1` ancestors, so this is the one shape where the number of routes and
    /// every other measure of size disagree by orders of magnitude. That disagreement is the whole
    /// reason [`PathBound::max_paths`] is a separate ceiling from [`crate::WalkBound::max_nodes`].
    Lattice { width: usize },
}

impl Shape {
    fn name(self) -> &'static str {
        match self {
            Shape::Detached => "none",
            Shape::Tree { .. } => "tree",
            Shape::Star => "star",
            Shape::Chain => "chain",
            Shape::Polytree { .. } => "polytree",
            Shape::Lattice { .. } => "lattice",
        }
    }

    /// The level `index` sits on in a lattice of `width`, counting the summit as level 0.
    fn lattice_level(width: usize, index: usize) -> usize {
        if index == 0 || width == 0 {
            0
        } else {
            (index - 1) / width + 1
        }
    }

    /// The parent the associative rules follow, or `None` for a root.
    ///
    /// One parent even where the shape states several, because [`associate_of`]'s "grandparent"
    /// has to stay the single, hand-checkable concept it was before branching existed. A shape
    /// with several parents has several grandparents, and quietly relating a concept to all of
    /// them would change what the S27 rows mean without changing their name.
    fn primary_parent_of(self, index: usize) -> Option<usize> {
        self.parents_of(index).first().copied()
    }

    /// The concept the route column is measured from.
    ///
    /// The last-generated concept in every shape but one, because it is the deepest and so the
    /// most expensive origin. **The polytree is the exception and the exception is the point:** its
    /// last concept is an arbitrary leaf which the share may not have widened, and a narrow leaf
    /// has exactly one route however polyhierarchic the vocabulary above it is. Measuring there
    /// would have printed `routes 1` for a vocabulary built specifically to have more than one,
    /// which is the wrong measurement wearing a right-looking number. So the polytree is measured
    /// from the last concept it actually widened.
    ///
    /// This is one concept and not the vocabulary's worst case, which would cost an enumeration
    /// per concept. The column is a representative, and `docs/UNTESTED.md` says so rather than
    /// letting the table read as a maximum.
    fn route_origin(self, concepts: usize) -> usize {
        let last = concepts.saturating_sub(1);
        match self {
            Shape::Polytree { period, .. } if period > 0 => (last / period) * period,
            _ => last,
        }
    }

    /// Every concept `index` states `skos:broader` to, in ascending order and without repeats.
    ///
    /// Concept 0 is the root in every shape. In the three monohierarchic ones this yields exactly
    /// `concepts - 1` links in total, which is what keeps their rows comparable; the two branching
    /// shapes state more by construction, and [`Measurement::stated_links`] counts what was
    /// written rather than assuming the difference.
    fn parents_of(self, index: usize) -> Vec<usize> {
        if index == 0 {
            return Vec::new();
        }
        match self {
            Shape::Detached => Vec::new(),
            Shape::Tree { branching } => vec![(index - 1) / branching],
            Shape::Star => vec![0],
            Shape::Chain => vec![index - 1],
            Shape::Polytree {
                branching,
                period,
                extra,
            } => {
                let primary = (index - 1) / branching;
                let mut parents = vec![primary];
                if period > 0 && index.is_multiple_of(period) {
                    // Downwards from the primary parent: each is a distinct earlier concept, so
                    // the result needs no deduplication and can close no loop.
                    parents.extend((1..=extra).filter_map(|step| primary.checked_sub(step)));
                }
                parents.sort_unstable();
                parents
            }
            Shape::Lattice { width } => {
                let level = Shape::lattice_level(width, index);
                if level <= 1 {
                    vec![0]
                } else {
                    let first = (level - 2) * width + 1;
                    (first..first + width).collect()
                }
            }
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
    /// Concepts the **model** holds with more than one `skos:broader`.
    ///
    /// Read off the built model rather than off the generator's intent, because a shape that asks
    /// for a share it cannot reach near the root would otherwise report the share it wanted.
    polyhierarchic: usize,
    /// The most `skos:broader` links any one concept states.
    widest_concept: usize,
    /// Every route to a summit from the concept [`Shape::route_origin`] names, under
    /// [`PathBound::DEFAULT`] — and whether the enumeration ran out of hierarchy or out of budget.
    ///
    /// `routes_complete` is the column that matters: `false` means the default bound refused to
    /// finish, and the route count beside it is a lower bound rather than an answer.
    routes: usize,
    routes_complete: bool,
    route_steps: usize,
    route_time: Duration,
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
            "{shape:<8} assoc {assoc:<8} {concepts:>9} concepts {links:>9} links \
             {related:>9} related | poly {poly:>9} widest {widest:>3} | build {build:>8.2?} | \
             rss +{held} MiB peak {peak} MiB | held {entries:>10} | deriv {deriv:>10} | \
             findings {findings:>9} | \
             report {report:>8.1} MiB | closure {closure:>14} ({multiple}) in {count:>8.2?} | \
             routes {routes:>7}{routes_mark} in {route_steps:>9} steps, {route_time:>8.2?}",
            shape = self.shape.name(),
            assoc = self.associativity.name(),
            concepts = self.concepts,
            links = self.stated_links,
            related = self.related_links,
            poly = self.polyhierarchic,
            widest = self.widest_concept,
            routes = self.routes,
            // A route count from an enumeration that ran out of budget is a lower bound, and a
            // table that printed it as a number would say the small one is the true one.
            routes_mark = if self.routes_complete { " " } else { "+" },
            route_steps = self.route_steps,
            route_time = self.route_time,
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
        for parent in shape.parents_of(index) {
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

    let broader_counts = |model: &CoreModel| {
        model
            .resources()
            .filter_map(|(_, resource)| resource.relations(SemanticRelation::Broader))
            .map(BTreeMap::len)
            .fold((0usize, 0usize), |(many, widest), count| {
                (many + usize::from(count > 1), widest.max(count))
            })
    };
    let (polyhierarchic, widest_concept) = broader_counts(&model);

    // From the last concept generated, which is the deepest in every shape here. Under the bound
    // production uses, so an incomplete answer in this column is an incomplete answer in
    // `openbiz paths`.
    let route_started = Instant::now();
    let route_origin = shape.route_origin(concepts);
    let routes = model.paths_to_root(&Node::iri(concept_iri(route_origin)), PathBound::DEFAULT);
    let route_time = route_started.elapsed();

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
        polyhierarchic,
        widest_concept,
        routes: routes.len(),
        routes_complete: routes.is_complete(),
        route_steps: routes.steps_walked(),
        route_time,
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
        assert_eq!(tree.parents_of(0), Vec::<usize>::new());
        assert_eq!(tree.parents_of(1), vec![0]);
        assert_eq!(tree.parents_of(2), vec![0]);
        assert_eq!(tree.parents_of(5), vec![2]);
        assert_eq!(tree.parents_of(6), vec![2]);

        for index in 0..8 {
            assert!(
                Shape::Detached.parents_of(index).is_empty(),
                "the baseline has no links"
            );
        }

        assert!(Shape::Star.parents_of(0).is_empty());
        for index in 1..8 {
            assert_eq!(Shape::Star.parents_of(index), vec![0]);
        }

        assert!(Shape::Chain.parents_of(0).is_empty());
        for index in 1..8 {
            assert_eq!(Shape::Chain.parents_of(index), vec![index - 1]);
        }
    }

    /// The polytree must widen the concepts it says it widens and no others, must point every
    /// extra link at an **earlier** concept — the property that makes it acyclic without a check —
    /// and must never state the same broader concept twice, which would inflate every per-link
    /// number in the table by counting one link as two.
    #[test]
    fn a_polytree_widens_only_the_share_it_names_and_only_upwards() {
        let shape = Shape::Polytree {
            branching: 2,
            period: 4,
            extra: 3,
        };

        // Branching 2: the primary parent of `index` is `(index - 1) / 2`, unchanged.
        assert_eq!(shape.parents_of(1), vec![0], "1 is not a multiple of 4");
        assert_eq!(shape.parents_of(5), vec![2]);

        // 4 is selected: primary parent 1, then 0. There is no concept below 0, so it gets one
        // extra rather than three — fewer, never a wrap-around into a larger index.
        assert_eq!(shape.parents_of(4), vec![0, 1]);
        // 8 is selected: primary parent 3, then 2, 1, 0 — the full three.
        assert_eq!(shape.parents_of(8), vec![0, 1, 2, 3]);

        for index in 1..200 {
            let parents = shape.parents_of(index);
            let mut sorted = parents.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(parents, sorted, "concept {index} states a broader twice");
            for parent in parents {
                assert!(
                    parent < index,
                    "concept {index} points at {parent}, which is not earlier"
                );
            }
            if !index.is_multiple_of(4) {
                assert_eq!(shape.parents_of(index).len(), 1, "{index} is not selected");
            }
        }
    }

    /// The lattice must place every concept on the level its arithmetic claims and link it to the
    /// whole level above, because the route count this shape exists to produce is `width` to the
    /// power of the depth and a single missing link changes it by a factor of the width.
    #[test]
    fn a_lattice_links_each_level_to_the_whole_level_above() {
        let shape = Shape::Lattice { width: 2 };

        assert!(shape.parents_of(0).is_empty(), "0 is the summit");
        // Level 1 is concepts 1 and 2, both under the summit alone.
        assert_eq!(shape.parents_of(1), vec![0]);
        assert_eq!(shape.parents_of(2), vec![0]);
        // Level 2 is concepts 3 and 4, each under both of level 1.
        assert_eq!(shape.parents_of(3), vec![1, 2]);
        assert_eq!(shape.parents_of(4), vec![1, 2]);
        // Level 3 is concepts 5 and 6, each under both of level 2.
        assert_eq!(shape.parents_of(5), vec![3, 4]);
        assert_eq!(shape.parents_of(6), vec![3, 4]);

        for index in 1..60 {
            for parent in shape.parents_of(index) {
                assert!(parent < index, "{index} points at {parent}");
                assert_eq!(
                    Shape::lattice_level(2, parent) + 1,
                    Shape::lattice_level(2, index),
                    "{index} must link one level up, not further"
                );
            }
        }

        // A width of three moves the levels, and the boundary is where an off-by-one would hide.
        let wide = Shape::Lattice { width: 3 };
        assert_eq!(wide.parents_of(3), vec![0], "3 is still level 1");
        assert_eq!(wide.parents_of(4), vec![1, 2, 3], "4 opens level 2");
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

        // A lattice of width 2 and seven concepts is 0 | 1,2 | 3,4 | 5,6. A concept on level L
        // reaches every concept above it, which is 1 + 2(L−1): level 1 reaches 1, level 2 reaches
        // 3, level 3 reaches 5. So 2(1) + 2(3) + 2(5) = 18 — far fewer pairs than the *routes*,
        // which is the distinction this shape exists to make visible.
        let lattice = measure(
            Shape::Lattice { width: 2 },
            7,
            Associativity::None,
            UNBOUNDED,
        );
        assert_eq!(lattice.closure_pairs, Some(18));
        assert_eq!(lattice.stated_links, 10, "1 + 1 + 2 + 2 + 2 + 2");
    }

    /// **The route count is exponential in the depth and nothing else in the model is.**
    ///
    /// A lattice of width `w` gives a concept on level `L` exactly `w^(L−1)` routes to the summit,
    /// because every step down multiplies the ways of getting there by the width of the level
    /// above. Checked against arithmetic that can be done in the head at three depths, so the
    /// bound test below is measuring the shape it claims rather than an accident.
    #[test]
    fn a_lattice_multiplies_the_routes_to_the_summit_at_every_level() {
        let routes_from_the_deepest = |concepts: usize| {
            measure(
                Shape::Lattice { width: 2 },
                concepts,
                Associativity::None,
                UNBOUNDED,
            )
            .routes
        };

        // 0 | 1,2 — the deepest concept is on level 1 and has the one route every monohierarchy
        // has. This row is the control: it is what every shape before iteration 40 could produce.
        assert_eq!(routes_from_the_deepest(3), 1);
        // 0 | 1,2 | 3,4 — level 2, two routes.
        assert_eq!(routes_from_the_deepest(5), 2);
        // 0 | 1,2 | 3,4 | 5,6 — level 3, four.
        assert_eq!(routes_from_the_deepest(7), 4);
        // 0 | … | 9,10 — level 5, sixteen.
        assert_eq!(routes_from_the_deepest(11), 16);

        // Width three multiplies by three, which is the check that the exponent is the level and
        // the base is the width rather than both being two by coincidence.
        let wide = measure(
            Shape::Lattice { width: 3 },
            10,
            Associativity::None,
            UNBOUNDED,
        );
        assert_eq!(wide.routes, 9, "level 3 of a width-3 lattice");
    }

    /// **Thirty concepts exhaust `PathBound::DEFAULT`, and twenty-nine do not.**
    ///
    /// This is the measurement the last six iterations could not take. `max_paths` is 10 000 and
    /// its own documentation justifies the number by an ordinary thesaurus reaching "the low
    /// thousands" — iteration 37 then counted the only real polyhierarchy available, LC Genre/Form
    /// Terms, and found a worst case of **7 routes**. So the ceiling is three orders of magnitude
    /// above the one measured vocabulary, and the graph that reaches it is not a large thesaurus
    /// at all: it is thirty concepts, which fits on a screen.
    ///
    /// Both halves are asserted on purpose. A test that only showed the bound being hit would pass
    /// just as well against a bound of zero.
    #[test]
    fn a_thirty_concept_lattice_exhausts_the_default_route_bound() {
        // 1 + 2 * 14 concepts puts the last one on level 14, with 2^13 = 8 192 routes.
        let under = measure(
            Shape::Lattice { width: 2 },
            29,
            Associativity::None,
            UNBOUNDED,
        );
        assert_eq!(under.routes, 8_192);
        assert!(
            under.routes_complete,
            "8 192 routes is inside a ceiling of {}",
            PathBound::DEFAULT.max_paths
        );

        // One concept further puts the deepest one on level 15 with 2^14 = 16 384 routes, which
        // the ceiling refuses. One concept — this is the whole distance between an answer and a
        // lower bound.
        let over = measure(
            Shape::Lattice { width: 2 },
            30,
            Associativity::None,
            UNBOUNDED,
        );
        assert_eq!(over.routes, PathBound::DEFAULT.max_paths);
        assert!(
            !over.routes_complete,
            "the enumeration must report a lower bound, not a truncated answer presented as whole"
        );

        // And it is the *route* ceiling that stopped it, not the step budget: this graph has 59
        // links and the walk never came near a million of them.
        assert!(
            over.route_steps < PathBound::DEFAULT.max_steps,
            "{} steps",
            over.route_steps
        );
        assert_eq!(
            over.stated_links, 56,
            "two on level 1, then two each for levels 2..15"
        );
        under.print();
        over.print();
    }

    /// The polytree must build the share it names, and the model must hold it — the share the
    /// generator *asks* for and the share the model *holds* differ near the root, and the column
    /// in the table is the second one.
    #[test]
    fn a_polytree_holds_the_share_of_polyhierarchic_concepts_the_table_reports() {
        let measured = measure(
            Shape::Polytree {
                branching: 10,
                period: LCGFT_SHARE,
                extra: 1,
            },
            1_000,
            Associativity::None,
            UNBOUNDED,
        );

        // 249 concepts in 1..1000 are multiples of 4, and two of them get no extra link: with a
        // branching of 10 both concept 4 and concept 8 have concept 0 as their primary parent, and
        // there is nothing below 0 to step down to. So the model holds 247, which is the number
        // the table prints — the generator asking for a quarter and the model holding slightly
        // under it is exactly why this column is read off the model.
        assert_eq!(measured.polyhierarchic, 247);
        assert_eq!(measured.widest_concept, 2);
        assert_eq!(measured.stated_links, 999 + 247);
        // Measured from concept 996, the last one the share widened: its two broader concepts are
        // 98 and 99, which rejoin at 9 and run up to the summit by two distinct routes. A leaf the
        // share did not widen has one route no matter what the vocabulary looks like, which is
        // what `route_origin` exists to avoid printing.
        assert_eq!(measured.routes, 2);
        assert!(measured.routes_complete);

        // The measured maximum of LC Genre/Form Terms, applied to the whole quarter rather than to
        // the one concept that had it.
        let widest = measure(
            Shape::Polytree {
                branching: 10,
                period: LCGFT_SHARE,
                extra: LCGFT_WIDEST,
            },
            1_000,
            Associativity::None,
            UNBOUNDED,
        );
        assert_eq!(widest.widest_concept, 4, "LCGFT's maximum is four broader");
        assert!(widest.stated_links > measured.stated_links);
        assert!(widest.routes > measured.routes);
        measured.print();
        widest.print();
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
            rule: SkosRule::S25.into(),
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
            assert_eq!(
                measured.polyhierarchic,
                0,
                "{} is a monohierarchy and every number it ever printed was measured on one",
                shape.name()
            );
            assert_eq!(measured.routes, 1, "{}", shape.name());
            measured.print();
        }

        // The two branching shapes state more than one link per concept, so the per-link
        // arithmetic is asserted against the links they actually wrote rather than against
        // `concepts - 1`. That the ratio survives several parents is the thing worth pinning: it
        // is what makes every row of the release table comparable across shapes.
        for shape in [
            Shape::Polytree {
                branching: 10,
                period: LCGFT_SHARE,
                extra: LCGFT_WIDEST,
            },
            Shape::Lattice { width: 2 },
        ] {
            let measured = measure(shape, 500, Associativity::None, COUNT_BUDGET);
            assert!(
                measured.stated_links > 499,
                "{} must state more links than a monohierarchy of the same size",
                shape.name()
            );
            assert_eq!(measured.held_entries, measured.stated_links * 4);
            assert_eq!(measured.derivations, measured.stated_links * 3);
            assert!(measured.polyhierarchic > 0, "{}", shape.name());
            assert!(measured.widest_concept > 1, "{}", shape.name());
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
        for shape in [
            Shape::Star,
            Shape::Tree { branching: 10 },
            Shape::Polytree {
                branching: 10,
                period: LCGFT_SHARE,
                extra: 1,
            },
            Shape::Polytree {
                branching: 10,
                period: LCGFT_SHARE,
                extra: LCGFT_WIDEST,
            },
            Shape::Chain,
        ] {
            measure(shape, 10_001, Associativity::None, COUNT_BUDGET).print();
        }
    }

    /// **What a realistic vocabulary costs — which is to say, the first row in this module that a
    /// customer's thesaurus resembles.**
    ///
    /// Read against `the_relation_model_at_…`, whose only difference is that a quarter of the
    /// concepts state one fewer broader concept. That difference is the whole of what six previous
    /// iterations' numbers assumed away: the closure of a polyhierarchy is not the closure of the
    /// tree underneath it, because a second parent adds that parent's entire ancestry.
    #[test]
    #[ignore = "a million concepts; run in release with --ignored"]
    fn a_realistic_polyhierarchy_at_each_order_of_magnitude() {
        for concepts in [10_001, 100_001, 1_000_001] {
            for extra in [1, LCGFT_WIDEST] {
                measure(
                    Shape::Polytree {
                        branching: 10,
                        period: LCGFT_SHARE,
                        extra,
                    },
                    concepts,
                    Associativity::None,
                    COUNT_BUDGET,
                )
                .print();
            }
        }
    }

    /// What §8.4's disjointness pass costs once the ancestry it walks is a polyhierarchy.
    ///
    /// `check_semantic_relation_disjointness` walks a concept's whole ancestry per associative
    /// link, and every measurement of that pass so far was taken where a concept's ancestry is a
    /// single line up to the root. A second parent means a second line, and the two do not merge
    /// until they happen to meet — so the pass's per-concept work is a property of the branching
    /// and not only of the depth, and no row in this module has ever said what it is.
    #[test]
    #[ignore = "one ancestry walk per concept over a branching hierarchy; run in release"]
    fn the_s27_pass_on_a_polyhierarchy_rather_than_a_tree() {
        for shape in [
            Shape::Tree { branching: 10 },
            Shape::Polytree {
                branching: 10,
                period: LCGFT_SHARE,
                extra: LCGFT_WIDEST,
            },
        ] {
            for associativity in [Associativity::None, Associativity::EveryConceptDetached] {
                measure(shape, 10_001, associativity, COUNT_BUDGET).print();
            }
        }
    }

    /// **How small a legal vocabulary has to be to defeat the route bound, at each width.**
    ///
    /// Not `#[ignore]`d and deliberately so: the answer is tens of concepts, so it costs
    /// milliseconds, and it is the one measurement in this module a reader is likeliest to
    /// disbelieve. Every other ceiling here is reached by making a vocabulary enormous.
    #[test]
    fn the_route_bound_is_reached_by_a_vocabulary_of_tens_of_concepts() {
        // The smallest vocabulary of each width whose deepest concept has more routes than the
        // ceiling: 2^14, 3^9 and 4^7 respectively. Note that widening does **not** monotonically
        // shrink the graph needed — a wider level reaches the ceiling in fewer levels but costs
        // more concepts to build each one.
        for (width, concepts) in [(2, 30), (3, 29), (4, 30)] {
            let measured = measure(
                Shape::Lattice { width },
                concepts,
                Associativity::None,
                COUNT_BUDGET,
            );
            assert!(
                !measured.routes_complete,
                "width {width} at {concepts} concepts should exhaust the route bound"
            );
            assert!(
                measured.concepts < 50,
                "the point is that it is small: {} concepts",
                measured.concepts
            );
            measured.print();
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
