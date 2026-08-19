//! Every route from one concept up to a root, and the cycles a route runs into.
//!
//! [`ancestry`](crate::ancestry) answers *which* concepts are above one concept. This answers a
//! different question with a different shape: **by what routes**. A breadcrumb needs the routes,
//! not the set, and in a polyhierarchy the two are not interchangeable — the set is linear in the
//! hierarchy and the routes are not.
//!
//! # "Root" is decided here, and the decision is that there are two notions and they are not the
//! same set
//!
//! §8 of the SKOS Reference (W3C Recommendation, 18 August 2009) states the hierarchy; §4.6 states
//! concept schemes. **Nothing in either relates them.** The specification's numbered statements
//! about `skos:hasTopConcept` are S5 (its domain), S6 (its range), S7 (`skos:topConceptOf` is a
//! sub-property of `skos:inScheme`) and S8 (the two are inverses). Not one of them mentions
//! `skos:broader`, and §8 states no condition mentioning a top concept. So:
//!
//! - A concept with **no broader concept** is where an upward route stops. That is a fact about
//!   the hierarchy, and it is what this module calls a **summit**.
//! - A **top concept** of a scheme is a fact about that scheme's entry points. A top concept with
//!   a broader concept is legal SKOS, and so is a summit that is a top concept of nothing.
//!
//! Collapsing the two would have been shorter and it would have invented a condition the
//! specification does not state. Instead a route runs to a summit, and **every top concept it
//! passes through is marked where it passes through it** — including one part-way up, which is
//! exactly the case where the two notions disagree and the only one where a caller can see that
//! they do.
//!
//! # Why routes are enumerated simple, and what that does to a cycle
//!
//! A route here never visits a concept twice. That is not a simplification, it is the only
//! terminating reading of the question: §8.6.8 says a cycle is **consistent** with the SKOS data
//! model, and a cycle makes the number of walks to a root infinite rather than merely large.
//!
//! So when the enumeration steps onto a concept already on the route it is building, it does not
//! follow it. It **records the cycle by name** — [`RootPaths::cycles`] — and carries on. That is
//! the second half of this module's item and the thing [`ancestry`](crate::ancestry) cannot do: a
//! walk from one concept reports a cycle only when the cycle runs through *that* concept, and a
//! cycle two levels above it is invisible from there while still being the reason a breadcrumb
//! has no route to a root.
//!
//! A cycle is stored **rotated so that its lowest concept comes first**, so the same loop entered
//! from two different routes is one entry rather than two spellings of one.
//!
//! # The bound is a different bound, with a different failure mode
//!
//! [`WalkBound`](crate::WalkBound) bounds a *set*: a breadth-first walk visits each concept once,
//! so its cost is the size of the hierarchy. This enumeration's cost is the number of **routes**,
//! which is exponential in the depth of a polyhierarchy — a lattice of *n* levels each offering
//! two parents has 2ⁿ routes through it and 2*n* ancestors. A concept with a complete ancestry can
//! therefore have an incomplete route list, which is why this has a [`PathBound`] of its own
//! rather than borrowing the walk's.
//!
//! As everywhere else in this crate, an answer that ran out of budget is
//! [distinguishable](RootPaths::is_complete) from one that ran out of hierarchy, and nothing may
//! be concluded from an absence in the first.

use std::collections::{BTreeMap, BTreeSet};
use std::iter;

use crate::model::{CoreModel, Derivation, Node, SkosRule};
use crate::relations::SemanticRelation;

/// How much of the route space one enumeration may cover before it gives up and says so.
///
/// Three numbers rather than one, because they bound three different things and a graph can
/// exhaust any of them without coming near the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathBound {
    /// The most complete routes to a summit that may be recorded.
    ///
    /// This is the exponential one. A polyhierarchy of *n* levels each offering two broader
    /// concepts has 2ⁿ routes and only 2*n* ancestors, so this ceiling is reachable by a hierarchy
    /// that is small by every other measure.
    pub max_paths: usize,
    /// The most distinct cycles that may be named.
    ///
    /// Separate from `max_paths` because a cycle is found *instead of* a route rather than as part
    /// of one: a hierarchy whose every route runs into a loop records no routes at all and can
    /// still find more cycles than this build should hold.
    pub max_cycles: usize,
    /// The most links the enumeration may follow, recorded or not.
    ///
    /// The backstop that bounds the work rather than the answer. A route abandoned at a cycle
    /// still cost the steps that built it, and a graph can spend this whole budget without
    /// completing a single route.
    pub max_steps: usize,
}

impl PathBound {
    /// The bound every caller in this build uses unless it says otherwise.
    ///
    /// 10 000 routes, 10 000 cycles, 1 000 000 steps. The step ceiling is
    /// [`WalkBound::DEFAULT`](crate::WalkBound::DEFAULT)'s link ceiling, because it bounds the
    /// same thing — links followed — and `docs/adr/0024` measured a million of them as already
    /// past what this build holds comfortably.
    ///
    /// The route ceiling is a judgement and is recorded as one in `docs/UNTESTED.md`. An ISO
    /// 25964 thesaurus is conventionally a handful of levels deep and a concept in one has one to
    /// three broader concepts, which puts an ordinary worst case in the low thousands — near
    /// enough to this ceiling that a real vocabulary could meet it, which is why hitting it
    /// reports an incomplete answer rather than a truncated one presented as whole.
    pub const DEFAULT: PathBound = PathBound {
        max_paths: 10_000,
        max_cycles: 10_000,
        max_steps: 1_000_000,
    };

    /// A bound of your own. Used by the tests to hit it without generating 10 000 routes.
    pub fn new(max_paths: usize, max_cycles: usize, max_steps: usize) -> Self {
        PathBound {
            max_paths,
            max_cycles,
            max_steps,
        }
    }
}

impl Default for PathBound {
    fn default() -> Self {
        PathBound::DEFAULT
    }
}

/// One step up a route: the concept it reaches, and whether the graph states it as a parent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RouteStep {
    concept: Node,
    stated: bool,
}

impl RouteStep {
    /// The concept this step reaches.
    pub fn concept(&self) -> &Node {
        &self.concept
    }

    /// Whether the graph states this as a **parent link** — `skos:broader` in one direction or
    /// the other — rather than only as a `skos:broaderTransitive` one.
    ///
    /// `false` is not a detail. S22 makes `skos:broader` a sub-property of
    /// `skos:broaderTransitive` and not the reverse, so a transitive-only step says the upper
    /// concept is somewhere above the lower one **without stating that it is directly above it**:
    /// there may be levels between them the vocabulary does not name. A breadcrumb drawn from
    /// such a step is a true statement of containment and a false statement of adjacency, and
    /// this is what tells them apart.
    pub fn is_stated(&self) -> bool {
        self.stated
    }
}

/// One route from a concept up to a summit, and what the route passes through on the way.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RootPath {
    origin: Node,
    steps: Vec<RouteStep>,
    top_concepts: BTreeMap<Node, BTreeSet<Node>>,
}

impl RootPath {
    /// The concept the route runs from.
    pub fn origin(&self) -> &Node {
        &self.origin
    }

    /// The steps up, in order. Empty when the concept asked about is its own summit.
    pub fn steps(&self) -> &[RouteStep] {
        &self.steps
    }

    /// The whole route as concepts, the concept asked about first and the summit last.
    ///
    /// Never empty: a concept with no broader concept is its own summit and its route is the one
    /// concept. That is a different answer from having no route at all, which is what a cycle
    /// produces, and the two must not be confused.
    pub fn concepts(&self) -> impl Iterator<Item = &Node> {
        iter::once(&self.origin).chain(self.steps.iter().map(RouteStep::concept))
    }

    /// Where the route stops: a concept with no broader concept.
    ///
    /// A summit is *not* by itself a top concept of any scheme — see the module note. Ask
    /// [`RootPath::top_concept_of`] whether it is one.
    pub fn summit(&self) -> &Node {
        match self.steps.last() {
            Some(step) => &step.concept,
            None => &self.origin,
        }
    }

    /// The schemes `concept` is a top concept of, if it is on this route and is one.
    ///
    /// Both directions under S8, so `skos:hasTopConcept` from the scheme answers this as well as
    /// `skos:topConceptOf` from the concept.
    pub fn top_concept_of(&self, concept: &Node) -> Option<&BTreeSet<Node>> {
        self.top_concepts.get(concept)
    }

    /// Every concept on this route that is a top concept of some scheme, with those schemes.
    ///
    /// In route order rather than in the map's order, because **where** on the route a scheme is
    /// entered is the point of asking.
    pub fn top_concepts(&self) -> impl Iterator<Item = (&Node, &BTreeSet<Node>)> {
        self.concepts()
            .filter_map(|concept| Some((concept, self.top_concepts.get(concept)?)))
    }

    /// Why the summit is above the concept asked about, as `CLAUDE.md` §3 requires.
    ///
    /// `None` when the route is a single link or has no links at all: one step is S22's conclusion
    /// or the graph's own, both already in [`CoreModel::derivations`], and a route of no steps
    /// concludes nothing. What this returns is precisely what S24's transitivity licensed.
    pub fn derivation(&self) -> Option<Derivation> {
        if self.steps.len() < 2 {
            return None;
        }
        Some(Derivation {
            conclusion: format!("{} skos:broaderTransitive {}", self.origin, self.summit()),
            premise: chain(&self.concepts().cloned().collect::<Vec<_>>()),
            rule: SkosRule::S24.into(),
        })
    }
}

/// A loop in the hierarchy: a run of concepts each above the last, closing back on the first.
///
/// §8.6.8 marks a cycle **consistent** with the SKOS data model, so this is not a
/// [`Finding`](crate::Finding) and is not phrased as one. What it is, is the reason a route
/// stopped without reaching a summit, which is a thing a breadcrumb has to be able to say.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HierarchyCycle {
    concepts: Vec<Node>,
    approach: Vec<Node>,
}

impl HierarchyCycle {
    /// The concepts in the loop, each above the one before it, and the last above the first.
    ///
    /// The first concept is **not** repeated at the end; the loop closes implicitly. Rotated so
    /// the lowest concept comes first, so the same loop met from two routes is one cycle rather
    /// than two spellings of it.
    pub fn concepts(&self) -> &[Node] {
        &self.concepts
    }

    /// The way up from the concept asked about to the loop: every concept before it on the route
    /// that ran into it, the concept asked about first.
    ///
    /// **Empty when the loop runs through the concept asked about itself**, which is the only
    /// case an upward walk can report on its own. A non-empty approach is the answer to "which of
    /// my ways up leads nowhere", and it is one representative route rather than all of them: the
    /// loop is one fact about the vocabulary however many ways there are into it, and listing
    /// every approach would be a second exponential inside the first.
    pub fn approach(&self) -> &[Node] {
        &self.approach
    }

    /// How many concepts are in the loop. One is §8.6.7's Example 36, a concept above itself.
    pub fn len(&self) -> usize {
        self.concepts.len()
    }

    /// Whether the loop holds no concepts, which by construction it never does. Present because
    /// clippy asks for it beside [`HierarchyCycle::len`].
    pub fn is_empty(&self) -> bool {
        self.concepts.is_empty()
    }

    /// Why every concept in the loop is its own broader concept, as `CLAUDE.md` §3 requires.
    ///
    /// `None` for a loop of one concept: `<A> skos:broader <A>` is §8.6.7's Example 36, whose
    /// conclusion is S22's and the graph's own rather than S24's, and crediting transitivity with
    /// it would be a citation for a step nothing took.
    pub fn derivation(&self) -> Option<Derivation> {
        let (first, rest) = self.concepts.split_first()?;
        if rest.is_empty() {
            return None;
        }
        let round: Vec<Node> = self
            .concepts
            .iter()
            .cloned()
            .chain(iter::once(first.clone()))
            .collect();
        Some(Derivation {
            conclusion: format!("{first} skos:broaderTransitive {first}"),
            premise: chain(&round),
            rule: SkosRule::S24.into(),
        })
    }
}

/// Every route from one concept to a summit, and every cycle the enumeration ran into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootPaths {
    origin: Node,
    paths: Vec<RootPath>,
    /// Keyed by the rotated loop so one loop is one entry, whichever way in was found first —
    /// the value carries that first way in, which the key deliberately does not.
    cycles: BTreeMap<Vec<Node>, HierarchyCycle>,
    steps: usize,
    complete: bool,
}

impl RootPaths {
    /// The concept the routes run from.
    pub fn origin(&self) -> &Node {
        &self.origin
    }

    /// Whether the enumeration ran out of hierarchy rather than out of budget.
    ///
    /// `false` means both lists are lower bounds: there may be routes not shown and cycles not
    /// named, and **an absence proves nothing**. In particular an empty route list from an
    /// incomplete enumeration does not mean the concept has no route to a summit.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// How many routes to a summit were found.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Whether no route reached a summit.
    ///
    /// From a complete enumeration this is a real and unusual answer: every route out of the
    /// concept runs into a cycle, so nothing reachable above it lacks a broader concept. The
    /// cycles are in [`RootPaths::cycles`] and they are the explanation.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Every route found, in the order the model holds the concepts they branch at.
    pub fn paths(&self) -> impl Iterator<Item = &RootPath> {
        self.paths.iter()
    }

    /// Every distinct cycle the enumeration ran into, whether or not it runs through the origin.
    pub fn cycles(&self) -> impl Iterator<Item = &HierarchyCycle> {
        self.cycles.values()
    }

    /// How many distinct cycles were named.
    pub fn cycle_count(&self) -> usize {
        self.cycles.len()
    }

    /// How many links the enumeration followed. Reported so a bound that was hit says which one.
    pub fn steps_walked(&self) -> usize {
        self.steps
    }

    /// The distinct concepts the routes end at.
    ///
    /// Fewer than the routes whenever a polyhierarchy offers two ways to the same summit, which
    /// is the ordinary case and the reason a breadcrumb needs the routes rather than this.
    pub fn summits(&self) -> BTreeSet<&Node> {
        self.paths.iter().map(RootPath::summit).collect()
    }
}

/// Render a run of concepts as the chain of one-step links that licensed it.
fn chain(concepts: &[Node]) -> String {
    concepts
        .windows(2)
        .map(|step| format!("{} skos:broaderTransitive {}", step[0], step[1]))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Rotate a loop so its lowest concept comes first, keeping the order of the links.
///
/// A loop has no first concept — it is wherever the enumeration happened to enter it — so two
/// routes into the same loop produce two rotations of one sequence. Rotating both to the same one
/// is what makes a count of cycles a count of cycles rather than of ways in.
fn rotate_to_lowest(cycle: &[Node]) -> Vec<Node> {
    let Some((lowest, _)) = cycle
        .iter()
        .enumerate()
        .min_by(|left, right| left.1.cmp(right.1))
    else {
        return Vec::new();
    };
    cycle[lowest..]
        .iter()
        .chain(&cycle[..lowest])
        .cloned()
        .collect()
}

impl CoreModel {
    /// Enumerate every route from `concept` up to a concept with no broader concept.
    ///
    /// Walks `skos:broaderTransitive`, which is what [`CoreModel::ancestry`] walks and which holds
    /// the one-step links S22 lifted from `skos:broader`, the ones S25 and S26 turned round from
    /// the narrower properties, and the ones the graph stated itself. The closure is not stored
    /// (see `docs/adr/0025`), so a step here is always a link the vocabulary holds and never a
    /// shortcut this build invented.
    ///
    /// Terminates on a cyclic hierarchy, which §8.6.8 says is consistent: a route that steps onto
    /// a concept already on it stops there and the loop is recorded in [`RootPaths::cycles`].
    ///
    /// A concept the graph never mentions gets one route — itself — because it has no broader
    /// concept. A caller that must tell that from a genuine root has to ask the model whether it
    /// holds the resource at all; `openbiz paths` refuses the first case for exactly that reason.
    pub fn paths_to_root(&self, concept: &Node, bound: PathBound) -> RootPaths {
        let mut found = RootPaths {
            origin: concept.clone(),
            paths: Vec::new(),
            cycles: BTreeMap::new(),
            steps: 0,
            complete: true,
        };

        // The route being built, and its concepts as a set so "already on this route" is a lookup
        // rather than a scan — a legal SKOS hierarchy can be 100 000 links deep and scanning the
        // route at every step would make the enumeration quadratic in that depth.
        let mut route: Vec<RouteStep> = Vec::new();
        let mut on_route: BTreeSet<Node> = BTreeSet::from([concept.clone()]);

        // An explicit stack rather than recursion, for the same reason `openbiz tree` renders with
        // one: a 100 000-link chain is legal SKOS and recursing down it turns the bound's honest
        // incomplete answer into a crash.
        let mut frames: Vec<(Vec<Node>, usize)> = Vec::new();

        let first = self.broader_of(concept);
        if first.is_empty() {
            // The concept asked about is its own summit. One route, of no steps.
            self.record(&mut found, concept, &route, bound);
        }
        frames.push((first, 0));

        while let Some((parents, next)) = frames.last_mut() {
            let Some(parent) = parents.get(*next).cloned() else {
                frames.pop();
                if let Some(left) = route.pop() {
                    on_route.remove(&left.concept);
                }
                continue;
            };
            *next += 1;

            if found.steps >= bound.max_steps {
                found.complete = false;
                return found;
            }
            found.steps += 1;

            if on_route.contains(&parent) {
                if !self.record_cycle(&mut found, concept, &route, &parent, bound) {
                    return found;
                }
                continue;
            }

            let from = route.last().map_or(concept, RouteStep::concept);
            let stated = self
                .resource(from)
                .and_then(|resource| resource.relations(SemanticRelation::Broader))
                .is_some_and(|links| links.contains_key(&parent));

            route.push(RouteStep {
                concept: parent.clone(),
                stated,
            });
            on_route.insert(parent.clone());

            let above = self.broader_of(&parent);
            if above.is_empty() {
                if !self.record(&mut found, concept, &route, bound) {
                    return found;
                }
                route.pop();
                on_route.remove(&parent);
            } else {
                frames.push((above, 0));
            }
        }

        found
    }

    /// The concepts one `skos:broaderTransitive` link above `concept`, in the model's order.
    fn broader_of(&self, concept: &Node) -> Vec<Node> {
        self.resource(concept)
            .and_then(|resource| resource.relations(SemanticRelation::BroaderTransitive))
            .into_iter()
            .flatten()
            .map(|(node, _)| node.clone())
            .collect()
    }

    /// Record the route as reaching a summit. `false` when the bound refused it.
    fn record(
        &self,
        found: &mut RootPaths,
        origin: &Node,
        route: &[RouteStep],
        bound: PathBound,
    ) -> bool {
        if found.paths.len() >= bound.max_paths {
            found.complete = false;
            return false;
        }
        let top_concepts = iter::once(origin)
            .chain(route.iter().map(RouteStep::concept))
            .filter_map(|concept| {
                let schemes = self.resource(concept)?.top_concept_of();
                (!schemes.is_empty()).then(|| (concept.clone(), schemes.clone()))
            })
            .collect();
        found.paths.push(RootPath {
            origin: origin.clone(),
            steps: route.to_vec(),
            top_concepts,
        });
        true
    }

    /// Record the loop closed by stepping from the end of `route` onto `parent`, which is already
    /// on it. `false` when the bound refused it.
    fn record_cycle(
        &self,
        found: &mut RootPaths,
        origin: &Node,
        route: &[RouteStep],
        parent: &Node,
        bound: PathBound,
    ) -> bool {
        let concepts: Vec<Node> = iter::once(origin.clone())
            .chain(route.iter().map(|step| step.concept.clone()))
            .collect();
        // The loop is the tail of the route from where `parent` first appears. Unreachable
        // otherwise — the caller checked membership — but computed rather than unwrapped, because
        // a report that panics on a legal vocabulary is worse than one that misses a cycle.
        let Some(from) = concepts.iter().position(|node| node == parent) else {
            return true;
        };
        let cycle = HierarchyCycle {
            concepts: rotate_to_lowest(&concepts[from..]),
            approach: concepts[..from].to_vec(),
        };
        if !found.cycles.contains_key(&cycle.concepts) && found.cycles.len() >= bound.max_cycles {
            found.complete = false;
            return false;
        }
        // The first way in wins. A later route into the same loop is the same fact about the
        // vocabulary, and replacing the approach would make which one is reported depend on the
        // order the model happens to hold the links in.
        found.cycles.entry(cycle.concepts.clone()).or_insert(cycle);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Statement;
    use crate::ns;

    fn ex(name: &str) -> Node {
        Node::iri(format!("http://example.org/{name}"))
    }

    fn skos(local: &str) -> String {
        format!("{}{local}", ns::SKOS)
    }

    fn s(subject: &Node, predicate: &str, object: &Node) -> Statement {
        Statement::new(subject.clone(), predicate.to_owned(), object.clone())
    }

    fn routes(found: &RootPaths) -> Vec<Vec<Node>> {
        found
            .paths()
            .map(|path| path.concepts().cloned().collect())
            .collect()
    }

    /// The item's substance. A diamond gives **two** routes to one summit, where the ancestry
    /// gives three ancestors and one shortest path each — which is why the routes are a separate
    /// question and not a rendering of the walk.
    #[test]
    fn a_diamond_has_two_routes_to_one_summit() {
        let (a, b, c, d) = (ex("A"), ex("B"), ex("C"), ex("D"));
        let model = CoreModel::from_statements([
            s(&a, &skos("broader"), &b),
            s(&a, &skos("broader"), &c),
            s(&b, &skos("broader"), &d),
            s(&c, &skos("broader"), &d),
        ]);

        let found = model.paths_to_root(&a, PathBound::DEFAULT);
        assert!(found.is_complete());
        assert_eq!(
            routes(&found),
            vec![
                vec![a.clone(), b.clone(), d.clone()],
                vec![a.clone(), c.clone(), d.clone()],
            ]
        );
        assert_eq!(found.summits(), BTreeSet::from([&d]));
        assert_eq!(found.cycle_count(), 0);

        // And the ancestry of the same concept is three concepts and says nothing about routes.
        assert_eq!(model.ancestry(&a, crate::WalkBound::DEFAULT).len(), 3);
    }

    /// A concept with no broader concept is its own summit, and its route is the one concept.
    /// That is a *different answer* from having no route at all — see the cycle tests below.
    #[test]
    fn a_concept_with_no_broader_concept_is_its_own_summit() {
        let a = ex("A");
        let model = CoreModel::from_statements([s(&a, &skos("broader"), &ex("B"))]);

        let found = model.paths_to_root(&ex("B"), PathBound::DEFAULT);
        assert!(found.is_complete());
        assert_eq!(routes(&found), vec![vec![ex("B")]]);
        let route = found.paths().next().expect("one route");
        assert_eq!(route.steps().len(), 0);
        assert_eq!(route.summit(), &ex("B"));
        assert_eq!(route.derivation(), None);
    }

    /// §8.6.8's Example 37 with no way out: every route runs into the cycle, so **no route
    /// reaches a summit** — and that is the answer, with the cycle as its explanation, rather
    /// than a hang or an empty list with nothing to say for itself.
    #[test]
    fn example_37_yields_no_route_and_one_named_cycle() {
        let (a, b) = (ex("A"), ex("B"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &a)]);

        let found = model.paths_to_root(&a, PathBound::DEFAULT);
        assert!(found.is_complete(), "a cycle is not a bound being hit");
        assert!(found.is_empty(), "nothing above A lacks a broader concept");
        assert_eq!(found.cycle_count(), 1);

        let cycle = found.cycles().next().expect("the cycle is named");
        assert_eq!(cycle.concepts(), &[a.clone(), b.clone()]);
        let why = cycle.derivation().expect("S24 licensed the loop");
        assert_eq!(why.rule, SkosRule::S24);
        assert_eq!(why.conclusion, format!("{a} skos:broaderTransitive {a}"));
        assert_eq!(
            why.premise,
            format!("{a} skos:broaderTransitive {b}, {b} skos:broaderTransitive {a}")
        );
        assert!(model.findings().is_empty(), "{:?}", model.findings());
    }

    /// §8.6.7's Example 36 — a concept above itself. One cycle of one concept, and it is S22's
    /// conclusion and not S24's, so it must not claim a transitive derivation.
    #[test]
    fn example_36_is_a_cycle_of_one_and_claims_no_transitive_derivation() {
        let a = ex("A");
        let model = CoreModel::from_statements([s(&a, &skos("broader"), &a)]);

        let found = model.paths_to_root(&a, PathBound::DEFAULT);
        assert!(found.is_empty());
        assert_eq!(found.cycle_count(), 1);
        let cycle = found.cycles().next().expect("the cycle is named");
        assert_eq!(cycle.len(), 1);
        assert_eq!(cycle.concepts(), std::slice::from_ref(&a));
        assert_eq!(cycle.derivation(), None);
    }

    /// **The thing `ancestry` cannot do.** A cycle two levels above the concept asked about does
    /// not run through it, so a walk from it reports no cycle at all — while still being the
    /// reason its routes have no summit. Naming it is half of this module's item.
    #[test]
    fn a_cycle_above_the_concept_is_named_even_though_it_does_not_run_through_it() {
        let (a, b, c, d) = (ex("A"), ex("B"), ex("C"), ex("D"));
        let model = CoreModel::from_statements([
            s(&a, &skos("broader"), &b),
            s(&b, &skos("broader"), &c),
            s(&c, &skos("broader"), &d),
            s(&d, &skos("broader"), &b),
        ]);

        // The walk upwards from A never comes back to A: the loop does not run through it.
        let above = model.ancestry(&a, crate::WalkBound::DEFAULT);
        assert!(!above.contains(&a), "the cycle does not run through A");

        let found = model.paths_to_root(&a, PathBound::DEFAULT);
        assert!(found.is_complete());
        assert!(found.is_empty(), "every route out of A runs into the loop");
        assert_eq!(found.cycle_count(), 1);
        let cycle = found.cycles().next().expect("the loop");
        assert_eq!(
            cycle.concepts(),
            &[b.clone(), c.clone(), d.clone()],
            "rotated to its lowest concept"
        );
        assert_eq!(
            cycle.approach(),
            std::slice::from_ref(&a),
            "and the way up that runs into it is named, which is what says which branch of the \
             hierarchy ends nowhere"
        );
    }

    /// A loop the concept asked about is itself part of has **no** approach, which is what tells
    /// it apart from one above the concept — and it is the only case an upward walk can report.
    #[test]
    fn a_loop_through_the_concept_asked_about_has_no_approach() {
        let (a, b) = (ex("A"), ex("B"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &a)]);

        let cycle = model
            .paths_to_root(&a, PathBound::DEFAULT)
            .cycles()
            .next()
            .expect("the loop")
            .clone();
        assert_eq!(cycle.concepts(), &[a, b]);
        assert!(cycle.approach().is_empty());
    }

    /// One loop entered by two routes is one cycle, not two spellings of it. Without the rotation
    /// this reports two, which is a count of ways in wearing the name of a count of cycles.
    #[test]
    fn one_loop_reached_two_ways_is_counted_once() {
        let (a, b, c, d) = (ex("A"), ex("B"), ex("C"), ex("D"));
        let model = CoreModel::from_statements([
            // Two routes from A into the loop, entering it at different concepts.
            s(&a, &skos("broader"), &b),
            s(&a, &skos("broader"), &c),
            s(&b, &skos("broader"), &c),
            s(&c, &skos("broader"), &d),
            s(&d, &skos("broader"), &b),
        ]);

        let found = model.paths_to_root(&a, PathBound::DEFAULT);
        assert_eq!(found.cycle_count(), 1, "{:?}", found.cycles);
        assert_eq!(
            found.cycles().next().expect("the loop").concepts(),
            &[b.clone(), c.clone(), d.clone()]
        );
    }

    /// A route reaching a summit is unaffected by a cycle on a *different* route out of the same
    /// concept. Both halves of the answer are reported and neither suppresses the other.
    #[test]
    fn a_route_to_a_summit_and_a_route_into_a_cycle_are_both_reported() {
        let (a, b, c, top) = (ex("A"), ex("B"), ex("C"), ex("Top"));
        let model = CoreModel::from_statements([
            s(&a, &skos("broader"), &top),
            s(&a, &skos("broader"), &b),
            s(&b, &skos("broader"), &c),
            s(&c, &skos("broader"), &b),
        ]);

        let found = model.paths_to_root(&a, PathBound::DEFAULT);
        assert!(found.is_complete());
        assert_eq!(routes(&found), vec![vec![a.clone(), top.clone()]]);
        assert_eq!(found.cycle_count(), 1);
        assert_eq!(
            found.cycles().next().expect("the loop").concepts(),
            &[b.clone(), c.clone()]
        );
    }

    /// **The two notions of "root", side by side.** A top concept part-way up the route is marked
    /// where it sits, and the summit above it is a summit that is a top concept of nothing. SKOS
    /// states no condition relating the two, so both are reported and neither is a defect.
    #[test]
    fn a_top_concept_part_way_up_is_marked_and_is_not_the_summit() {
        let (leaf, middle, above) = (ex("Leaf"), ex("Middle"), ex("Above"));
        let scheme = ex("Scheme");
        let model = CoreModel::from_statements([
            s(&leaf, &skos("broader"), &middle),
            s(&middle, &skos("broader"), &above),
            s(&scheme, &skos("hasTopConcept"), &middle),
        ]);

        let found = model.paths_to_root(&leaf, PathBound::DEFAULT);
        let route = found.paths().next().expect("one route");
        assert_eq!(
            route.summit(),
            &above,
            "the hierarchy stops above the scheme's entry point"
        );
        assert_eq!(route.top_concept_of(&above), None);
        assert_eq!(
            route.top_concept_of(&middle),
            Some(&BTreeSet::from([scheme.clone()])),
            "S8 answers this from the scheme's side of the link"
        );
        assert_eq!(
            route.top_concepts().collect::<Vec<_>>(),
            vec![(&middle, &BTreeSet::from([scheme.clone()]))]
        );
        assert!(model.findings().is_empty(), "{:?}", model.findings());
    }

    /// The S22 asymmetry, one route long. A step licensed only by `skos:broaderTransitive` states
    /// containment and **not** adjacency, and the route says which of its steps are which.
    #[test]
    fn a_transitive_only_step_is_distinguished_from_a_stated_parent_link() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([
            s(&a, &skos("broader"), &b),
            s(&b, &skos("broaderTransitive"), &c),
        ]);

        let found = model.paths_to_root(&a, PathBound::DEFAULT);
        let route = found.paths().next().expect("one route");
        assert_eq!(
            route.concepts().cloned().collect::<Vec<_>>(),
            vec![a.clone(), b.clone(), c.clone()]
        );
        assert_eq!(route.steps().len(), 2);
        assert!(route.steps()[0].is_stated(), "A skos:broader B");
        assert!(
            !route.steps()[1].is_stated(),
            "S22 runs one way: a transitive link does not state that C is B's parent"
        );
    }

    /// A hierarchy written downwards walks upwards, because S25 and S26 close each direction into
    /// the other — and the inverted link is a stated parent link, not a transitive-only one.
    #[test]
    fn a_hierarchy_written_downwards_produces_the_same_routes() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements([s(&b, &skos("narrower"), &a)]);

        let found = model.paths_to_root(&a, PathBound::DEFAULT);
        assert_eq!(routes(&found), vec![vec![a.clone(), b.clone()]]);
        assert!(found.paths().next().expect("one route").steps()[0].is_stated());
    }

    /// **The bound, and the whole reason it is a bound of its own.** Two parents at each of four
    /// levels is sixteen routes and eight ancestors: the ancestry is complete and the route list
    /// is not, from the same hierarchy at the same moment.
    #[test]
    fn an_incomplete_route_list_can_sit_beside_a_complete_ancestry() {
        // A lattice: level n has two concepts, each linked to both concepts of level n + 1.
        let mut statements = Vec::new();
        let origin = ex("origin");
        for side in ["L", "R"] {
            statements.push(s(&origin, &skos("broader"), &ex(&format!("{side}0"))));
        }
        for level in 0..3 {
            for from in ["L", "R"] {
                for to in ["L", "R"] {
                    statements.push(s(
                        &ex(&format!("{from}{level}")),
                        &skos("broader"),
                        &ex(&format!("{to}{}", level + 1)),
                    ));
                }
            }
        }
        let model = CoreModel::from_statements(statements);

        let all = model.paths_to_root(&origin, PathBound::DEFAULT);
        assert!(all.is_complete());
        assert_eq!(all.len(), 16, "two choices at each of four levels");
        assert_eq!(all.summits().len(), 2);

        // The ancestry of the same concept is eight concepts, and complete under its own bound.
        let above = model.ancestry(&origin, crate::WalkBound::DEFAULT);
        assert!(above.is_complete() && above.len() == 8);

        let bounded = model.paths_to_root(&origin, PathBound::new(4, 10, usize::MAX));
        assert!(
            !bounded.is_complete(),
            "four routes of sixteen is not the answer"
        );
        assert_eq!(bounded.len(), 4);

        let by_steps = model.paths_to_root(&origin, PathBound::new(usize::MAX, 10, 3));
        assert!(!by_steps.is_complete());
        assert_eq!(by_steps.steps_walked(), 3);

        // And with room it finishes, so the difference is the bound and not the graph.
        assert!(model
            .paths_to_root(&origin, PathBound::new(16, 1, usize::MAX))
            .is_complete());
    }

    /// The cycle ceiling refuses rather than truncating, and it is separate from the route
    /// ceiling: a hierarchy can exhaust it while recording no routes at all.
    #[test]
    fn the_cycle_ceiling_refuses_rather_than_dropping_a_cycle_in_silence() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([
            // Two loops through A, so two cycles and no route to a summit.
            s(&a, &skos("broader"), &b),
            s(&b, &skos("broader"), &a),
            s(&a, &skos("broader"), &c),
            s(&c, &skos("broader"), &a),
        ]);

        let all = model.paths_to_root(&a, PathBound::DEFAULT);
        assert!(all.is_complete() && all.is_empty());
        assert_eq!(all.cycle_count(), 2);

        let bounded = model.paths_to_root(&a, PathBound::new(usize::MAX, 1, usize::MAX));
        assert!(!bounded.is_complete());
        assert_eq!(bounded.cycle_count(), 1);
    }

    /// `skos:related` is not a hierarchy and must never be walked as one — §8.6.4's Example 32
    /// says the associative relation is not even transitive.
    #[test]
    fn example_32_is_not_walked_as_a_hierarchy() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements([s(&a, &skos("related"), &b)]);

        let found = model.paths_to_root(&a, PathBound::DEFAULT);
        assert_eq!(routes(&found), vec![vec![a]], "A is its own summit");
    }

    /// A blank node is a perfectly good concept and the enumeration must not lose one.
    #[test]
    fn a_route_through_blank_nodes_enumerates_like_any_other() {
        let (a, b) = (Node::blank("a"), Node::blank("b"));
        let model = CoreModel::from_statements([s(&a, &skos("broader"), &b)]);

        let found = model.paths_to_root(&a, PathBound::DEFAULT);
        assert_eq!(routes(&found), vec![vec![a, b]]);
    }

    /// Every route reported is a real chain of one-step links in the model, no concept appears on
    /// one twice, its summit really has no broader concept, and every step's stated-or-not flag
    /// agrees with the model. Asserted structurally rather than against a fixture's shape, so a
    /// defect in the frame bookkeeping cannot pass by matching an expected list.
    #[test]
    fn every_reported_route_is_a_real_chain_with_no_repeated_concept() {
        let (a, b, c, d, e) = (ex("A"), ex("B"), ex("C"), ex("D"), ex("E"));
        let model = CoreModel::from_statements([
            s(&a, &skos("broader"), &b),
            s(&a, &skos("broader"), &c),
            s(&b, &skos("broader"), &d),
            s(&c, &skos("broaderTransitive"), &d),
            s(&c, &skos("broader"), &e),
            s(&d, &skos("broader"), &e),
        ]);

        let found = model.paths_to_root(&a, PathBound::DEFAULT);
        assert!(!found.is_empty() && found.is_complete());
        for route in found.paths() {
            let concepts: Vec<&Node> = route.concepts().collect();
            assert_eq!(
                concepts.iter().collect::<BTreeSet<_>>().len(),
                concepts.len(),
                "a concept appears twice on {concepts:?}"
            );
            assert_eq!(route.steps().len(), concepts.len() - 1);
            for (index, pair) in concepts.windows(2).enumerate() {
                let links = model
                    .resource(pair[0])
                    .and_then(|resource| resource.relations(SemanticRelation::BroaderTransitive))
                    .expect("a concept a route steps from has broader concepts");
                assert!(
                    links.contains_key(pair[1]),
                    "{} is not one link above {}",
                    pair[1],
                    pair[0]
                );
                let stated = model
                    .resource(pair[0])
                    .and_then(|resource| resource.relations(SemanticRelation::Broader))
                    .is_some_and(|stated| stated.contains_key(pair[1]));
                assert_eq!(route.steps()[index].is_stated(), stated);
            }
            assert!(
                model
                    .resource(route.summit())
                    .and_then(|resource| resource.relations(SemanticRelation::BroaderTransitive))
                    .is_none(),
                "a summit has no broader concept"
            );
        }
    }
}
