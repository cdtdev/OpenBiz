//! S45's transitive closure of `skos:exactMatch`, answered by walking rather than by storing.
//!
//! §10.2 of the SKOS Reference (W3C Recommendation, 18 August 2009) makes `skos:exactMatch` an
//! `owl:TransitiveProperty` (S45), so a chain of exact mapping links entails a link between its
//! ends. §10.6.3's Example 62 is the specification's own statement of that entailment, and it is
//! the shape an enterprise actually produces: a house vocabulary is mapped to a hub, the hub is
//! mapped to a regulator's list, and the question "is our `Client` their `Counterparty`?" is
//! answered by two statements neither of which mentions both ends.
//!
//! # Why this is a walk and not a table
//!
//! `docs/adr/0025` states the rule the whole build follows — *materialise what is bounded by the
//! schema, walk what is bounded by the data* — and a transitive closure is bounded by the data.
//! `docs/adr/0030` records why it applies here with more force than it did to S24, not less:
//! `skos:exactMatch` is **symmetric as well as transitive** (S44 and S45 together), so its
//! closure over a chain of *n* concepts is not the *n(n−1)/2* pairs a hierarchy would give but
//! all *n²* of them, every one of which S44 then requires in both directions. A hub with a
//! thousand vocabularies mapped onto it is one cluster, and storing its closure is a million
//! links produced from two thousand statements.
//!
//! # Why the shape differs from [`Ancestry`](crate::Ancestry)
//!
//! Both are bounded breadth-first walks with a predecessor map, and there the resemblance stops.
//! `skos:broaderTransitive` is directed, so an ancestry is a set of concepts *above* one; an
//! exact-match closure is **undirected**, because S44 has already put every link at both ends. So
//! what this walks is a connected component — a cluster of concepts SKOS says are interchangeable
//! — and not a path upwards.
//!
//! Two consequences, and both are in the tests rather than left to a reader's inference:
//!
//! - **The origin comes back as a member of its own cluster** whenever it has any exact match at
//!   all. That is not an artefact: `<A> skos:exactMatch <B>` entails `<B> skos:exactMatch <A>`
//!   under S44, and those two entail `<A> skos:exactMatch <A>` under S45. §10.6.6's Example 66
//!   marks a reflexive mapping consistent, so this is a conclusion and never a defect. It is the
//!   same treatment [`Ancestry`](crate::Ancestry) gives a hierarchy cycle, arrived at for a
//!   different reason.
//! - **Cycles are ordinary rather than pathological.** §10.6.6 warns outright that "applications
//!   must be able to cope with cycles in skos:exactMatch and skos:closeMatch", and after S44
//!   every single link is one. A walk that did not expand each concept exactly once would not
//!   terminate on a two-statement vocabulary.
//!
//! # The bound, and why an abandoned walk must not look like a finished one
//!
//! [`ExactMatchCluster::is_complete`] carries the same warning [`Ancestry`](crate::Ancestry)
//! does, and for the same reason: a walk that gave up after two members and a cluster that
//! genuinely has two members produce the same [`ExactMatchCluster::len`]. An *absence* from an
//! incomplete walk proves nothing, and reading one as a proof is how a validator reports
//! "consistent" for a graph it never finished checking. Every caller in this crate branches on
//! it, and the S46 sweep turns a `false` into a [`Finding`](crate::Finding) rather than swallowing
//! it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::mapping::MappingProperty;
use crate::model::{CoreModel, Derivation, Node, SkosRule};

/// How much of an exact-match cluster one walk may cover before it gives up and says so.
///
/// Two numbers rather than one, because they fail differently — the split
/// [`AncestryBound`](crate::AncestryBound) documents, with the sizes reasoned from §10 rather than
/// from §8. `max_members` bounds a *long* chain of hub mappings; `max_links` bounds a *dense*
/// cluster, where a hub concept carrying a thousand exact matches costs a thousand steps to leave
/// however few distinct concepts are behind them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EquivalenceBound {
    /// The most distinct concepts one cluster walk may reach.
    pub max_members: usize,
    /// The most links **one check** may follow, reached or not.
    ///
    /// As with [`AncestryBound::max_links`](crate::AncestryBound::max_links), one check is one
    /// walk when a caller asks about one concept — `openbiz mappings` — and it is the *whole
    /// sweep* when S46 walks once per concept holding an exact match. The sweep hands each walk
    /// what is **left** of this budget rather than a fresh copy of it, because a per-walk budget
    /// multiplied by one walk per concept is not a bound. `docs/adr/0027` has the measurement
    /// that made that lesson expensive the first time.
    pub max_links: usize,
}

impl EquivalenceBound {
    /// The bound every caller in this build uses unless it says otherwise.
    ///
    /// 100 000 members and 1 000 000 links, which are [`AncestryBound::DEFAULT`]'s numbers and
    /// are chosen the same way: far above any cluster a real mapping produces, far below the
    /// point where the walk is the reason a report is slow. A backstop against a pathological
    /// graph rather than a product limit — a vocabulary that hits it has a problem the report
    /// should name, which is why hitting it is a [`Finding`](crate::Finding) and never a silent
    /// truncation.
    ///
    /// [`AncestryBound::DEFAULT`]: crate::AncestryBound::DEFAULT
    pub const DEFAULT: EquivalenceBound = EquivalenceBound {
        max_members: 100_000,
        max_links: 1_000_000,
    };

    /// A bound of your own. Used by the tests to hit it without generating 100 000 concepts.
    pub fn new(max_members: usize, max_links: usize) -> Self {
        EquivalenceBound {
            max_members,
            max_links,
        }
    }
}

impl Default for EquivalenceBound {
    fn default() -> Self {
        EquivalenceBound::DEFAULT
    }
}

/// Every concept one concept is joined to through `skos:exactMatch`, and the chain that reached
/// each of them.
///
/// "Joined to" is the closure S44 and S45 license together over the one-step links
/// [`Resource::mappings_of`](crate::Resource::mappings_of) holds — the graph's own plus the
/// converses S44 supplied. This walks those; what it adds is S45.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactMatchCluster {
    origin: Node,
    /// Each member, and the concept the walk stepped from to reach it. A predecessor map rather
    /// than a stored chain per member, for the reason [`Ancestry`](crate::Ancestry) gives: the
    /// chains share their prefixes.
    reached: BTreeMap<Node, Node>,
    links_walked: usize,
    complete: bool,
}

impl ExactMatchCluster {
    /// The concept the walk started from.
    pub fn origin(&self) -> &Node {
        &self.origin
    }

    /// Whether the walk ran out of concepts rather than out of budget.
    ///
    /// `false` means the answer is a lower bound and nothing may be concluded from an *absence*
    /// in it. Never ignore this: see the module note.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// How many concepts were reached.
    pub fn len(&self) -> usize {
        self.reached.len()
    }

    /// Whether nothing was reached — the origin holds no exact match at all.
    pub fn is_empty(&self) -> bool {
        self.reached.is_empty()
    }

    /// How many links the walk followed. Reported so a bound that was hit says which one.
    pub fn links_walked(&self) -> usize {
        self.links_walked
    }

    /// Whether `node` is in the origin's cluster.
    ///
    /// A `false` from an incomplete walk means "not found within the bound", not "not equivalent".
    pub fn contains(&self, node: &Node) -> bool {
        self.reached.contains_key(node)
    }

    /// Every concept reached, in a stable order.
    pub fn members(&self) -> impl Iterator<Item = &Node> {
        self.reached.keys()
    }

    /// The chain the walk followed from the origin to `node`, origin first and `node` last.
    ///
    /// Breadth-first, so it is a *shortest* chain — the one an author is likeliest to recognise
    /// as the route through their mappings. A concept reachable two ways gets one of them, and
    /// the model does not claim it is the only one.
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

    /// Why `node` is an exact match of the origin, as the derivation `CLAUDE.md` §3 requires.
    ///
    /// `None` when `node` is not in the cluster, and **also** when the chain is a single link: a
    /// direct `skos:exactMatch` is the graph's own or S44's, both of which are already in
    /// [`CoreModel::derivations`], and repeating them here would credit S45 with a conclusion it
    /// did not add. What this returns is precisely what the transitivity licensed.
    pub fn derivation_to(&self, node: &Node) -> Option<Derivation> {
        let path = self.path_to(node)?;
        if path.len() < 3 {
            return None;
        }
        let premise = path
            .windows(2)
            .map(|step| format!("{} skos:exactMatch {}", step[0], step[1]))
            .collect::<Vec<_>>()
            .join(", ");
        Some(Derivation {
            conclusion: format!("{} skos:exactMatch {node}", self.origin),
            premise,
            rule: SkosRule::S45.into(),
        })
    }

    /// Every member S45 supplied — those reached by a chain of two links or more.
    ///
    /// The one-step members are the graph's own links and S44's converses; they are already in
    /// the model and already checked against S46. This is the part the walk added, which is what
    /// both callers want: the report prints it as its own section, and the S46 sweep checks it
    /// without re-reporting a clash the direct pass has already found.
    pub fn entailed(&self) -> impl Iterator<Item = (&Node, Vec<Node>)> + '_ {
        self.reached.keys().filter_map(move |node| {
            let path = self.path_to(node)?;
            (path.len() >= 3).then_some((node, path))
        })
    }
}

impl CoreModel {
    /// Walk `skos:exactMatch` outwards from `concept` and report the cluster it sits in.
    ///
    /// This is S45 and it is the only place in the build that applies it. Nothing is written back
    /// into the model: [`Resource::mappings_of`](crate::Resource::mappings_of) keeps meaning
    /// "one-step links under this property" and never "equivalence class", permanently and by
    /// design (`docs/adr/0025`, `docs/adr/0030`).
    ///
    /// Terminates on a cyclic cluster, which after S44 is every cluster with a link in it — see
    /// the module note. §10.6.6 requires exactly this of an application.
    ///
    /// The mirror walk for `skos:closeMatch` is deliberately **not** here and never will be: §10.1
    /// says `skos:closeMatch` is not transitive precisely so that chaining it across schemes does
    /// not compound errors, and closing it would state what its author declined to.
    pub fn exact_match_cluster(
        &self,
        concept: &Node,
        bound: EquivalenceBound,
    ) -> ExactMatchCluster {
        let mut cluster = ExactMatchCluster {
            origin: concept.clone(),
            reached: BTreeMap::new(),
            links_walked: 0,
            complete: true,
        };

        let mut expanded: BTreeSet<Node> = BTreeSet::new();
        let mut queue: VecDeque<Node> = VecDeque::new();
        queue.push_back(concept.clone());

        while let Some(current) = queue.pop_front() {
            // The origin is reached again through S44's converse whenever it has any link at all.
            // It is recorded as a member when that happens — S45 entails it and §10.6.6's
            // Example 66 says the graph is consistent — but its links are followed once, or the
            // walk would go round for ever on a two-statement vocabulary.
            if !expanded.insert(current.clone()) {
                continue;
            }
            let Some(links) = self
                .resource(&current)
                .and_then(|resource| resource.mappings_of(MappingProperty::ExactMatch))
            else {
                continue;
            };
            for other in links.keys() {
                if cluster.links_walked >= bound.max_links {
                    cluster.complete = false;
                    return cluster;
                }
                cluster.links_walked += 1;
                if cluster.reached.contains_key(other) {
                    continue;
                }
                if cluster.reached.len() >= bound.max_members {
                    cluster.complete = false;
                    return cluster;
                }
                cluster.reached.insert(other.clone(), current.clone());
                queue.push_back(other.clone());
            }
        }

        cluster
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Statement;

    fn ex(name: &str) -> Node {
        Node::iri(format!("http://example.org/{name}"))
    }

    fn exact(from: &Node, to: &Node) -> Statement {
        Statement::new(from.clone(), MappingProperty::ExactMatch.iri(), to.clone())
    }

    /// §10.6.3's **Example 62** — the specification's own statement that
    /// `<A> exactMatch <B> . <B> exactMatch <C> .` entails `<A> skos:exactMatch <C>`.
    ///
    /// This is the assertion that replaced `s45_is_not_applied_so_an_exact_match_chain_does_not
    /// _close` in `model.rs`, by hand and deliberately, when the walk landed.
    #[test]
    fn example_62_entails_the_link_across_two_steps() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([exact(&a, &b), exact(&b, &c)]);

        let cluster = model.exact_match_cluster(&a, EquivalenceBound::DEFAULT);
        assert!(cluster.is_complete());
        assert!(cluster.contains(&c), "S45 licenses the far link");
        assert_eq!(
            cluster.path_to(&c),
            Some(vec![a.clone(), b.clone(), c.clone()])
        );

        // The one-step link is the graph's own, already in the model's derivations; S45 adds only
        // the far one, and the derivation names the chain rather than asserting the endpoint.
        assert_eq!(cluster.derivation_to(&b), None);
        let why = cluster
            .derivation_to(&c)
            .expect("S45 licensed the far link");
        assert_eq!(why.rule, SkosRule::S45);
        assert_eq!(
            why.premise,
            format!("{a} skos:exactMatch {b}, {b} skos:exactMatch {c}")
        );
        assert_eq!(why.conclusion, format!("{a} skos:exactMatch {c}"));
    }

    /// The cluster is **undirected**, which is the whole difference from an ancestry: the far end
    /// of the chain reaches the near one, because S44 put every link at both ends.
    #[test]
    fn the_walk_reaches_both_ways_because_the_property_is_symmetric() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([exact(&a, &b), exact(&b, &c)]);

        assert!(model
            .exact_match_cluster(&c, EquivalenceBound::DEFAULT)
            .contains(&a));
        assert!(model
            .exact_match_cluster(&b, EquivalenceBound::DEFAULT)
            .contains(&a));
    }

    /// The origin is its own exact match once it has any link at all — S44 then S45 — and the
    /// walk says why rather than hiding a conclusion it drew. §10.6.6's Example 66 marks a
    /// reflexive mapping consistent, so this is never a defect.
    #[test]
    fn a_linked_concept_is_its_own_exact_match_and_the_chain_says_why() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements([exact(&a, &b)]);

        let cluster = model.exact_match_cluster(&a, EquivalenceBound::DEFAULT);
        assert!(
            cluster.contains(&a),
            "S44 and S45 entail the reflexive link"
        );
        assert_eq!(
            cluster.path_to(&a),
            Some(vec![a.clone(), b.clone(), a.clone()])
        );
        assert_eq!(
            cluster
                .derivation_to(&a)
                .expect("the reflexive link is an S45 conclusion")
                .rule,
            SkosRule::S45
        );
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    /// §10.6.6's **Example 66** on its own — a stated reflexive mapping is one link, not a
    /// transitive conclusion, so `derivation_to` must not claim S45 for it.
    #[test]
    fn example_66_is_a_one_step_link_and_not_a_transitive_conclusion() {
        let a = ex("A");
        let model = CoreModel::from_statements([exact(&a, &a)]);

        let cluster = model.exact_match_cluster(&a, EquivalenceBound::DEFAULT);
        assert!(cluster.contains(&a));
        assert_eq!(cluster.path_to(&a), Some(vec![a.clone(), a.clone()]));
        assert_eq!(cluster.derivation_to(&a), None);
    }

    /// §10.6.6 — "applications must be able to cope with cycles in skos:exactMatch and
    /// skos:closeMatch". A stated cycle terminates and is consistent.
    #[test]
    fn a_stated_cycle_terminates_and_is_consistent() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([exact(&a, &b), exact(&b, &c), exact(&c, &a)]);

        let cluster = model.exact_match_cluster(&a, EquivalenceBound::DEFAULT);
        assert!(cluster.is_complete(), "a cycle is not a bound being hit");
        assert_eq!(cluster.len(), 3, "A, B and C, the first of them itself");
        assert!(model.is_consistent(), "{:?}", model.findings());
    }

    /// §10.1 — `skos:closeMatch` is deliberately **not** transitive, "in order to avoid the
    /// possibility of compound errors when key mappings are combined across more than two
    /// schemes". A chain of close matches must not become a cluster.
    #[test]
    fn a_close_match_chain_is_not_walked() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([
            Statement::new(a.clone(), MappingProperty::CloseMatch.iri(), b.clone()),
            Statement::new(b.clone(), MappingProperty::CloseMatch.iri(), c.clone()),
        ]);

        assert!(model
            .exact_match_cluster(&a, EquivalenceBound::DEFAULT)
            .is_empty());
    }

    /// A walk that gave up says so, and `contains` from it proves nothing. This is the single
    /// most dangerous confusion in the module, so it is tested directly rather than inferred.
    #[test]
    fn a_bounded_walk_is_distinguishable_from_a_finished_one() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([exact(&a, &b), exact(&b, &c)]);

        let bounded = model.exact_match_cluster(&a, EquivalenceBound::new(1, usize::MAX));
        assert!(!bounded.is_complete());
        assert_eq!(bounded.len(), 1);
        assert!(!bounded.contains(&c), "the walk never got there");

        let by_links = model.exact_match_cluster(&a, EquivalenceBound::new(usize::MAX, 1));
        assert!(!by_links.is_complete());
        assert_eq!(by_links.links_walked(), 1);

        // And the same walk with room finishes, so the difference is the bound and not the graph.
        assert!(model
            .exact_match_cluster(&a, EquivalenceBound::DEFAULT)
            .is_complete());
    }

    /// A concept nothing in the graph maps has an empty cluster, and asking is not an error.
    #[test]
    fn a_concept_with_no_exact_match_has_an_empty_cluster() {
        let model = CoreModel::from_statements([]);
        let cluster = model.exact_match_cluster(&ex("A"), EquivalenceBound::DEFAULT);
        assert!(cluster.is_complete() && cluster.is_empty());
        assert_eq!(cluster.path_to(&ex("B")), None);
        assert_eq!(cluster.derivation_to(&ex("B")), None);
    }

    /// `entailed` is what the walk added and not what the graph said. Three concepts in a chain:
    /// the middle one is one step away and is excluded; the far one and the origin itself are
    /// S45's.
    #[test]
    fn only_the_chains_of_two_steps_or_more_are_reported_as_entailed() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([exact(&a, &b), exact(&b, &c)]);

        let cluster = model.exact_match_cluster(&a, EquivalenceBound::DEFAULT);
        let entailed: Vec<&Node> = cluster.entailed().map(|(node, _)| node).collect();
        assert_eq!(
            entailed,
            vec![&a, &c],
            "B is one step and is the graph's own"
        );
    }

    /// **What the sweep costs on a dense cluster, measured rather than reasoned about.**
    ///
    /// The S46 closure sweep walks once per concept holding an exact match. When the mapping is
    /// concept-for-concept — the common case, where every cluster has two members — that is two
    /// links per concept and the [`EquivalenceBound::DEFAULT`] million is reached at about half a
    /// million mapped concepts, which is comfortable.
    ///
    /// A **hub** is the other shape: *n* vocabularies all declaring their concept equivalent to
    /// one central concept. That is a single cluster of *n*, walked once per member, and this test
    /// pins what falls out of it — the sweep costs about **2n²**, so it is quadratic in the
    /// cluster and not linear in the vocabulary. A 400-member cluster spends a third of the
    /// default budget on its own; a 1 000-member one exhausts it and the report says S46 is
    /// unchecked.
    ///
    /// It is a test rather than a comment because the number is the input to a decision nobody has
    /// taken yet: whether the sweep should walk each *component* once instead of each *member*,
    /// which would make it linear. `docs/UNTESTED.md` carries that, and this test is what a future
    /// iteration measures against.
    #[test]
    fn the_sweep_cost_is_quadratic_in_a_cluster_and_not_linear_in_the_vocabulary() {
        let cost_of_a_hub = |members: usize| -> usize {
            let centre = ex("hub");
            let model = CoreModel::from_statements(
                (0..members).map(|i| exact(&ex(&format!("m{i}")), &centre)),
            );
            (0..members)
                .map(|i| ex(&format!("m{i}")))
                .chain(std::iter::once(centre))
                .map(|node| {
                    model
                        .exact_match_cluster(&node, EquivalenceBound::DEFAULT)
                        .links_walked()
                })
                .sum()
        };

        // Measured on 2026-08-19: 220, 20 200 and 321 200 links for 10, 100 and 400 members.
        // Asserted as a band rather than exactly, so a change in traversal order is not a failure
        // but a change in *complexity* is. Each size is walked once, because this test is the
        // slowest in the crate and walking 400 members twice would double that for nothing.
        let sizes = [10usize, 100, 400];
        let costs: Vec<usize> = sizes.into_iter().map(cost_of_a_hub).collect();
        for (members, cost) in sizes.into_iter().zip(costs.iter().copied()) {
            let square = members * members;
            assert!(
                cost >= square && cost <= 3 * square + 4 * members,
                "{members} members cost {cost} links, which is not the ~2n^2 this test pins"
            );
        }
        assert!(
            costs[2] > EquivalenceBound::DEFAULT.max_links / 4,
            "a 400-member cluster spends a serious fraction of the default budget, and a build \
             that made this cheap should say so here rather than leave the claim standing"
        );
    }

    /// A blank node is a perfectly good concept and the walk must not lose one.
    #[test]
    fn a_chain_of_blank_nodes_walks_like_any_other() {
        let (a, b, c) = (Node::blank("a"), Node::blank("b"), Node::blank("c"));
        let model = CoreModel::from_statements([exact(&a, &b), exact(&b, &c)]);

        assert_eq!(
            model
                .exact_match_cluster(&a, EquivalenceBound::DEFAULT)
                .path_to(&c),
            Some(vec![a, b, c])
        );
    }
}
