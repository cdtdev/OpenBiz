//! S24's transitive closure, answered by walking rather than by storing.
//!
//! §8 of the SKOS Reference (W3C Recommendation, 18 August 2009) makes
//! `skos:broaderTransitive` and `skos:narrowerTransitive` `owl:TransitiveProperty` (S24), so a
//! chain of `skos:broader` links entails a link from each concept to every concept above it.
//! §8.6.6's Example 35 is the specification's own statement of that entailment.
//!
//! # Why this is a walk and not a table
//!
//! `docs/adr/0025` and the measurement behind it, `docs/adr/0024`, settle it: the closure is
//! **never materialised**. A chain of 100 000 `skos:broader` links is a legal SKOS graph — §8
//! states no condition against depth, and §8.6.8 says a cycle is legal too — and its closure is
//! 5 000 050 000 pairs. The bound on what we may store is therefore not "large", it is
//! *unbounded on permitted input*, and no vocabulary size makes it safe.
//!
//! The second reason is the one that would still apply if memory were free. A stored
//! `(Node, RelationOrigin)` can cite S24 but cannot name the path it took, and `CLAUDE.md` §3
//! requires every inference to explain itself. A walk produces the path **as a by-product of
//! finding the answer at all**, so [`Ancestry::path_to`] costs nothing extra and
//! [`Ancestry::derivation_to`] renders it as the specification statement plus the chain that
//! licensed it.
//!
//! # The bound, and why an abandoned walk must not look like a finished one
//!
//! A walk is bounded ([`AncestryBound`]). Without one, asking §8.4's question of every concept in
//! a million-link vocabulary is a million traversals of the whole hierarchy, and the honest
//! failure mode of an unbounded walk is a server that stops answering rather than one that says
//! it does not know.
//!
//! [`Ancestry::is_complete`] is therefore not a nicety. A walk that gave up after two ancestors
//! and a concept that genuinely has two ancestors produce the same [`Ancestry::len`], and reading
//! the second answer off the first is exactly how a validator reports "consistent" for a graph it
//! never finished checking. Every caller in this crate branches on it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::model::{CoreModel, Derivation, Node, SkosRule};
use crate::relations::SemanticRelation;

/// How much of a hierarchy one walk may cover before it gives up and says so.
///
/// Two numbers rather than one, because they fail differently. `max_ancestors` bounds a *deep*
/// hierarchy — the 100 000-link chain above. `max_links` bounds a *wide* one: a concept with a
/// million `skos:broader` values has one ancestor per link and reaching them costs a million
/// steps, so a walk bounded only by ancestors would still take a million of them before stopping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AncestryBound {
    /// The most distinct ancestors a walk may reach.
    pub max_ancestors: usize,
    /// The most links **one check** may follow, reached or not.
    ///
    /// One check is one walk when a caller asks about one concept — `openbiz ancestors` — and it
    /// is the *whole sweep* when a caller walks once per concept, which is what §8.4's
    /// disjointness pass does. A sweep hands each walk what is left of this budget rather than a
    /// fresh copy of it, and that is not a refinement: **a per-walk budget times one walk per
    /// concept is not a bound.** Iteration 30 measured a legal 10 001-concept chain with one
    /// `skos:related` on each concept building in **30.6 seconds** against 62 ms for the same
    /// vocabulary without them, with this ceiling at a million and no single walk coming within
    /// two orders of magnitude of it. See `docs/adr/0027`.
    pub max_links: usize,
}

impl AncestryBound {
    /// The bound every caller in this build uses unless it says otherwise.
    ///
    /// 100 000 ancestors and 1 000 000 links. Chosen to be far above any hierarchy a thesaurus
    /// has and far below the point where the walk is the reason a request is slow: ISO 25964
    /// thesauri are conventionally a handful of levels deep, and `docs/adr/0024` measured a
    /// million *links* as already past what this build holds comfortably in memory. It is a
    /// backstop against a pathological graph, not a product limit — a vocabulary that hits it has
    /// a problem the report should name, which is why hitting it is a [`Finding`](crate::Finding)
    /// rather than a silent truncation.
    pub const DEFAULT: AncestryBound = AncestryBound {
        max_ancestors: 100_000,
        max_links: 1_000_000,
    };

    /// A bound of your own. Used by the tests to hit it without generating 100 000 concepts.
    pub fn new(max_ancestors: usize, max_links: usize) -> Self {
        AncestryBound {
            max_ancestors,
            max_links,
        }
    }
}

impl Default for AncestryBound {
    fn default() -> Self {
        AncestryBound::DEFAULT
    }
}

/// Everything above one concept in the hierarchy, and the path that reached each of them.
///
/// "Above" is `skos:broaderTransitive`, which after the closure in
/// [`relations`](crate::relations) holds the links S22 lifted from `skos:broader`, the ones S25
/// and S26 turned round from `skos:narrower`, and the ones the graph stated itself. This walks
/// those one-step links; what it adds is S24.
///
/// The origin is **not** an ancestor of itself unless the graph makes it one. §8.6.7's Example 36
/// (`<A> skos:broader <A>`) and §8.6.8's Example 37 (a two-concept cycle) are both marked
/// consistent by the specification, and in both the origin does come back as its own ancestor —
/// legitimately, with a path that names the cycle. Nothing here treats that as a defect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ancestry {
    origin: Node,
    /// Each ancestor, and the concept the walk stepped from to reach it — one step closer to the
    /// origin. A predecessor map rather than a stored path per ancestor: the paths of *n*
    /// ancestors share their prefixes, so keeping one node each is the difference between memory
    /// proportional to the hierarchy and memory proportional to the hierarchy times its depth.
    reached: BTreeMap<Node, Node>,
    links_walked: usize,
    complete: bool,
}

impl Ancestry {
    /// The concept the walk started from.
    pub fn origin(&self) -> &Node {
        &self.origin
    }

    /// Whether the walk ran out of ancestors rather than out of budget.
    ///
    /// `false` means the answer is a lower bound and nothing may be concluded from an *absence*
    /// in it. Never ignore this: see the module note.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// How many ancestors were reached.
    pub fn len(&self) -> usize {
        self.reached.len()
    }

    /// Whether nothing above the origin was reached.
    pub fn is_empty(&self) -> bool {
        self.reached.is_empty()
    }

    /// How many links the walk followed. Reported so a bound that was hit says which one.
    pub fn links_walked(&self) -> usize {
        self.links_walked
    }

    /// Whether `node` is above the origin.
    ///
    /// A `false` from an incomplete walk means "not found within the bound", not "not an
    /// ancestor".
    pub fn contains(&self, node: &Node) -> bool {
        self.reached.contains_key(node)
    }

    /// Every ancestor reached, in a stable order.
    pub fn ancestors(&self) -> impl Iterator<Item = &Node> {
        self.reached.keys()
    }

    /// The path the walk took from the origin to `node`, origin first and `node` last.
    ///
    /// Breadth-first, so it is a *shortest* path — the one an author is likeliest to recognise as
    /// the route through their hierarchy. A concept reachable by two routes (§8.6.9's Examples 38
    /// and 39, both consistent) gets one of them, and the model does not claim it is the only one.
    pub fn path_to(&self, node: &Node) -> Option<Vec<Node>> {
        if !self.reached.contains_key(node) {
            return None;
        }
        let mut path = vec![node.clone()];
        let mut current = node;
        // Bounded by the number of reached nodes: the predecessor map is a breadth-first tree
        // rooted at the origin, so walking it back terminates. The counter is belt and braces —
        // a loop here would hang a report rather than print a wrong one, and that is worse.
        for _ in 0..=self.reached.len() {
            let previous = self.reached.get(current)?;
            path.push(previous.clone());
            if *previous == self.origin {
                path.reverse();
                return Some(path);
            }
            current = previous;
        }
        None
    }

    /// Why `node` is above the origin, as the derivation `CLAUDE.md` §3 requires.
    ///
    /// `None` when `node` is not an ancestor, and **also** when the path is a single link: a
    /// direct `skos:broaderTransitive` is S22's or the graph's own, both of which are already in
    /// [`CoreModel::derivations`], and repeating them here would credit S24 with a conclusion it
    /// did not add. What this returns is precisely what the transitivity licensed.
    pub fn derivation_to(&self, node: &Node) -> Option<Derivation> {
        let path = self.path_to(node)?;
        if path.len() < 3 {
            return None;
        }
        let premise = path
            .windows(2)
            .map(|step| format!("{} skos:broaderTransitive {}", step[0], step[1]))
            .collect::<Vec<_>>()
            .join(", ");
        Some(Derivation {
            conclusion: format!("{} skos:broaderTransitive {node}", self.origin),
            premise,
            rule: SkosRule::S24,
        })
    }
}

impl CoreModel {
    /// Walk `skos:broaderTransitive` upwards from `concept` and report what is above it.
    ///
    /// This is S24 and it is the only place in the build that applies it. Nothing is written back
    /// into the model: [`Resource::relations`](crate::Resource::relations) keeps meaning "links
    /// under this property" and never "ancestors", permanently and by design
    /// (`docs/adr/0024`, `docs/adr/0025`).
    ///
    /// Terminates on a cyclic hierarchy. §8.6.8 is explicit that a cycle is consistent with the
    /// SKOS data model, so a walk that hung on one would refuse to read a legal vocabulary; the
    /// cycle comes back as the origin being its own ancestor, with a path that names it.
    ///
    /// The mirror walk — descendants, down `skos:narrowerTransitive` — is deliberately **not**
    /// here. It is the same function with the inverse property and it has no caller, which
    /// `CLAUDE.md` §4 calls not-done rather than ahead. It arrives with the concept-tree item
    /// that needs it.
    pub fn ancestry(&self, concept: &Node, bound: AncestryBound) -> Ancestry {
        let mut ancestry = Ancestry {
            origin: concept.clone(),
            reached: BTreeMap::new(),
            links_walked: 0,
            complete: true,
        };

        let mut expanded: BTreeSet<Node> = BTreeSet::new();
        let mut queue: VecDeque<Node> = VecDeque::new();
        queue.push_back(concept.clone());

        while let Some(current) = queue.pop_front() {
            // The origin can be reached again through a cycle. It is recorded as an ancestor when
            // that happens — §8.6.8 says the graph is consistent and the fact is true — but its
            // links are followed once, or the walk would go round for ever.
            if !expanded.insert(current.clone()) {
                continue;
            }
            let Some(links) = self
                .resource(&current)
                .and_then(|resource| resource.relations(SemanticRelation::BroaderTransitive))
            else {
                continue;
            };
            for above in links.keys() {
                if ancestry.links_walked >= bound.max_links {
                    ancestry.complete = false;
                    return ancestry;
                }
                ancestry.links_walked += 1;
                if ancestry.reached.contains_key(above) {
                    continue;
                }
                if ancestry.reached.len() >= bound.max_ancestors {
                    ancestry.complete = false;
                    return ancestry;
                }
                ancestry.reached.insert(above.clone(), current.clone());
                queue.push_back(above.clone());
            }
        }

        ancestry
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

    /// A chain of `skos:broader`, walked. §8.6.6's Example 35 is the specification's own statement
    /// that `<A> broader <B> . <B> broader <C> .` entails `<A> broaderTransitive <C>`.
    #[test]
    fn example_35_entails_the_link_across_two_steps() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        let above = model.ancestry(&a, AncestryBound::DEFAULT);
        assert!(above.is_complete());
        assert_eq!(above.len(), 2);
        assert!(above.contains(&b) && above.contains(&c));
        assert_eq!(
            above.path_to(&c),
            Some(vec![a.clone(), b.clone(), c.clone()])
        );

        // The one-step link is S22's, already in the model's derivations; S24 adds only the far
        // one, and the derivation names the path rather than asserting the endpoint.
        assert_eq!(above.derivation_to(&b), None);
        let why = above.derivation_to(&c).expect("S24 licensed the far link");
        assert_eq!(why.rule, SkosRule::S24);
        assert_eq!(
            why.premise,
            format!("{a} skos:broaderTransitive {b}, {b} skos:broaderTransitive {c}")
        );
        assert_eq!(why.conclusion, format!("{a} skos:broaderTransitive {c}"));
    }

    /// The walk goes up and not down: `<C>` is above `<A>`, so `<A>` is not above `<C>`.
    #[test]
    fn the_walk_has_a_direction() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        assert!(model.ancestry(&c, AncestryBound::DEFAULT).is_empty());
        assert_eq!(model.ancestry(&b, AncestryBound::DEFAULT).len(), 1);
    }

    /// §8.6.8, Example 37 — a cycle is **consistent** with the SKOS data model. The walk must
    /// terminate, must report the origin as its own ancestor, and must be able to say why.
    #[test]
    fn example_37_terminates_and_names_the_cycle() {
        let (a, b) = (ex("A"), ex("B"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &a)]);

        let above = model.ancestry(&a, AncestryBound::DEFAULT);
        assert!(above.is_complete(), "a cycle is not a bound being hit");
        assert!(above.contains(&a), "the cycle makes A its own ancestor");
        assert!(above.contains(&b));
        assert_eq!(
            above.path_to(&a),
            Some(vec![a.clone(), b.clone(), a.clone()])
        );
        assert_eq!(
            above
                .derivation_to(&a)
                .expect("the cycle is an S24 conclusion")
                .rule,
            SkosRule::S24
        );
        assert!(model.findings().is_empty(), "{:?}", model.findings());
    }

    /// §8.6.7, Example 36 — `<A> skos:broader <A>` on its own is consistent. One step, so it is
    /// S22's conclusion and not S24's, and `derivation_to` must not claim it.
    #[test]
    fn example_36_is_a_one_step_link_and_not_a_transitive_conclusion() {
        let a = ex("A");
        let model = CoreModel::from_statements([s(&a, &skos("broader"), &a)]);

        let above = model.ancestry(&a, AncestryBound::DEFAULT);
        assert!(above.contains(&a));
        assert_eq!(above.path_to(&a), Some(vec![a.clone(), a.clone()]));
        assert_eq!(above.derivation_to(&a), None);
    }

    /// §8.6.9, Examples 38 and 39 — two paths to the same concept, both consistent. The ancestor
    /// is reported once, by the shorter route, and the polyhierarchy is not a finding.
    #[test]
    fn example_38_reports_one_shortest_path_for_two_routes() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([
            s(&a, &skos("broader"), &b),
            s(&a, &skos("broader"), &c),
            s(&b, &skos("broader"), &c),
        ]);

        let above = model.ancestry(&a, AncestryBound::DEFAULT);
        assert_eq!(above.len(), 2);
        assert_eq!(above.path_to(&c), Some(vec![a.clone(), c.clone()]));
        assert!(model.findings().is_empty(), "{:?}", model.findings());
    }

    /// A walk that gave up says so, and `contains` from it proves nothing. This is the single
    /// most dangerous confusion in the module, so it is tested directly rather than inferred.
    #[test]
    fn a_bounded_walk_is_distinguishable_from_a_finished_one() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        let bounded = model.ancestry(&a, AncestryBound::new(1, usize::MAX));
        assert!(!bounded.is_complete());
        assert_eq!(bounded.len(), 1);
        assert!(!bounded.contains(&c), "the walk never got there");

        let by_links = model.ancestry(&a, AncestryBound::new(usize::MAX, 1));
        assert!(!by_links.is_complete());
        assert_eq!(by_links.links_walked(), 1);

        // And the same walk with room finishes, so the difference is the bound and not the graph.
        assert!(model.ancestry(&a, AncestryBound::new(2, 2)).is_complete());
    }

    /// A concept nothing in the graph mentions has no ancestors, and asking is not an error.
    #[test]
    fn a_concept_the_graph_never_mentions_has_no_ancestors() {
        let model = CoreModel::from_statements([]);
        let above = model.ancestry(&ex("A"), AncestryBound::DEFAULT);
        assert!(above.is_complete() && above.is_empty());
        assert_eq!(above.path_to(&ex("B")), None);
        assert_eq!(above.derivation_to(&ex("B")), None);
    }

    /// The walk reads `skos:broaderTransitive`, which S22 fills from `skos:broader` and S25/S26
    /// fill from `skos:narrower` — so a hierarchy written downwards walks upwards, and one stated
    /// with the transitive property itself is walked too.
    #[test]
    fn a_hierarchy_written_downwards_or_transitively_walks_the_same() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let downwards = CoreModel::from_statements([
            s(&c, &skos("narrower"), &b),
            s(&b, &skos("narrower"), &a),
        ]);
        assert_eq!(downwards.ancestry(&a, AncestryBound::DEFAULT).len(), 2);

        let transitively = CoreModel::from_statements([
            s(&a, &skos("broaderTransitive"), &b),
            s(&b, &skos("broaderTransitive"), &c),
        ]);
        let above = transitively.ancestry(&a, AncestryBound::DEFAULT);
        assert!(above.contains(&c), "S24 closes stated transitive links too");
    }

    /// A blank node is a perfectly good concept and the walk must not lose one.
    #[test]
    fn a_chain_of_blank_nodes_walks_like_any_other() {
        let (a, b, c) = (Node::blank("a"), Node::blank("b"), Node::blank("c"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        assert_eq!(
            model.ancestry(&a, AncestryBound::DEFAULT).path_to(&c),
            Some(vec![a, b, c])
        );
    }

    /// `skos:related` is not a hierarchy and must never be walked as one — §8.6.4's Example 32
    /// says the associative relation is not even transitive.
    #[test]
    fn example_32_is_not_walked_as_a_hierarchy() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("related"), &b), s(&b, &skos("related"), &c)]);

        assert!(model.ancestry(&a, AncestryBound::DEFAULT).is_empty());
    }
}
