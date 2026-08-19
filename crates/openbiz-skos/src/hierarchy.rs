//! The bounded breadth-first walk both directions of the hierarchy are answered by.
//!
//! §8 of the SKOS Reference (W3C Recommendation, 18 August 2009) makes `skos:broaderTransitive`
//! and `skos:narrowerTransitive` `owl:TransitiveProperty` (S24). Neither closure is stored — see
//! [`ancestry`](crate::ancestry) for why, and `docs/adr/0024` and `docs/adr/0025` for the
//! measurement behind it — so both are answered by walking the one-step links on read.
//!
//! Going up and going down are the *same walk over the inverse property*, which is not a
//! convenience but a consequence of S25 and S26: the model closes each direction into the other,
//! so `<A> skos:narrowerTransitive <B>` is present exactly when `<B> skos:broaderTransitive <A>`
//! is. One implementation therefore answers both, and a defect found in one direction cannot
//! survive in the other.
//!
//! # The bound means two different things in the two directions, and that is the point
//!
//! [`WalkBound::DEFAULT`] is 100 000 nodes and 1 000 000 links, and those numbers were chosen for
//! the upward walk, where they are a backstop against a pathological graph: a thesaurus is
//! conventionally a handful of levels deep, so an ordinary vocabulary is nowhere near them.
//!
//! **Downwards they are reachable by an ordinary vocabulary.** Everything below a top concept is
//! most of the vocabulary, so a walk down from the root of a 100 000-concept thesaurus reaches
//! the bound *because the vocabulary is large*, not because it is pathological. That is not a
//! reason to remove the bound — an unbounded walk on a hostile graph is a server that stops
//! answering — and it is not a reason to raise it silently either. It is a reason the incomplete
//! answer must be **impossible to mistake for a complete one**, which is what
//! [`Walk::is_complete`] exists for and what every caller in this crate branches on.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::model::{CoreModel, Derivation, Node, SkosRule};
use crate::relations::SemanticRelation;

/// How much of a hierarchy one walk may cover before it gives up and says so.
///
/// Two numbers rather than one, because they fail differently. `max_nodes` bounds a *deep*
/// hierarchy — a 100 000-link chain is legal SKOS. `max_links` bounds a *wide* one: a concept with
/// a million `skos:broader` values has one ancestor per link and reaching them costs a million
/// steps, so a walk bounded only by nodes would still take a million of them before stopping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WalkBound {
    /// The most distinct concepts a walk may reach.
    ///
    /// Named for nodes and not for ancestors because the same walk runs downwards, where the
    /// reached concepts are descendants and where — see the module note — this ceiling is
    /// something an ordinary large vocabulary meets rather than a backstop it never approaches.
    pub max_nodes: usize,
    /// The most links **one check** may follow, reached or not.
    ///
    /// One check is one walk when a caller asks about one concept — `openbiz ancestors`,
    /// `openbiz tree` — and it is the *whole sweep* when a caller walks once per concept, which is
    /// what §8.4's disjointness pass does. A sweep hands each walk what is left of this budget
    /// rather than a fresh copy of it, and that is not a refinement: **a per-walk budget times one
    /// walk per concept is not a bound.** Iteration 30 measured a legal 10 001-concept chain with
    /// one `skos:related` on each concept building in **30.6 seconds** against 62 ms for the same
    /// vocabulary without them, with this ceiling at a million and no single walk coming within
    /// two orders of magnitude of it. See `docs/adr/0027`.
    pub max_links: usize,
}

impl WalkBound {
    /// The bound every caller in this build uses unless it says otherwise.
    ///
    /// 100 000 concepts and 1 000 000 links. Chosen to be far above any hierarchy a thesaurus has
    /// *upwards* and far below the point where the walk is the reason a request is slow: ISO 25964
    /// thesauri are conventionally a handful of levels deep, and `docs/adr/0024` measured a
    /// million *links* as already past what this build holds comfortably in memory. It is a
    /// backstop against a pathological graph, not a product limit — a vocabulary that hits it has
    /// a problem the report should name, which is why hitting it is a [`Finding`](crate::Finding)
    /// rather than a silent truncation.
    ///
    /// Read the module note before assuming the same is true downwards. It is not.
    pub const DEFAULT: WalkBound = WalkBound {
        max_nodes: 100_000,
        max_links: 1_000_000,
    };

    /// A bound of your own. Used by the tests to hit it without generating 100 000 concepts.
    pub fn new(max_nodes: usize, max_links: usize) -> Self {
        WalkBound {
            max_nodes,
            max_links,
        }
    }
}

impl Default for WalkBound {
    fn default() -> Self {
        WalkBound::DEFAULT
    }
}

/// What one bounded traversal of the hierarchy reached, and how it got to each concept.
///
/// Crate-internal on purpose: a caller is given [`Ancestry`](crate::Ancestry) or
/// [`Descent`](crate::Descent), which know which property they walked and can therefore say which
/// statement licensed each conclusion. A bare `Walk` could not, and `CLAUDE.md` §3 requires every
/// inference to explain itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Walk {
    origin: Node,
    /// Each concept reached, and the concept the walk stepped from to reach it — one step closer
    /// to the origin. A predecessor map rather than a stored path per concept: the paths of *n*
    /// reached concepts share their prefixes, so keeping one node each is the difference between
    /// memory proportional to the hierarchy and memory proportional to the hierarchy times its
    /// depth.
    reached: BTreeMap<Node, Node>,
    links_walked: usize,
    complete: bool,
}

impl Walk {
    /// The concept the walk started from.
    pub(crate) fn origin(&self) -> &Node {
        &self.origin
    }

    /// Whether the walk ran out of hierarchy rather than out of budget.
    pub(crate) fn is_complete(&self) -> bool {
        self.complete
    }

    /// How many concepts were reached.
    pub(crate) fn len(&self) -> usize {
        self.reached.len()
    }

    /// Whether nothing was reached.
    pub(crate) fn is_empty(&self) -> bool {
        self.reached.is_empty()
    }

    /// How many links the walk followed. Reported so a bound that was hit says which one.
    pub(crate) fn links_walked(&self) -> usize {
        self.links_walked
    }

    /// Whether `node` was reached.
    pub(crate) fn contains(&self, node: &Node) -> bool {
        self.reached.contains_key(node)
    }

    /// Every concept reached, in a stable order.
    pub(crate) fn reached(&self) -> impl Iterator<Item = &Node> {
        self.reached.keys()
    }

    /// Every concept reached and the concept the walk stepped from to reach it.
    ///
    /// This is the breadth-first tree as a predecessor list. A caller rendering it as a tree must
    /// mark what it has already printed: a cycle is legal SKOS (§8.6.8) and puts the origin back
    /// among the reached concepts, so following predecessors forwards without a guard does not
    /// terminate.
    pub(crate) fn steps(&self) -> impl Iterator<Item = (&Node, &Node)> {
        self.reached.iter()
    }

    /// The path the walk took from the origin to `node`, origin first and `node` last.
    ///
    /// Breadth-first, so it is a *shortest* path — the one an author is likeliest to recognise as
    /// the route through their hierarchy. A concept reachable by two routes (§8.6.9's Examples 38
    /// and 39, both consistent) gets one of them, and the model does not claim it is the only one.
    pub(crate) fn path_to(&self, node: &Node) -> Option<Vec<Node>> {
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

    /// Why `node` was reached, as the derivation `CLAUDE.md` §3 requires.
    ///
    /// `None` when `node` was not reached, and **also** when the path is a single link: a direct
    /// link under `via` is S22's or the graph's own, both of which are already in
    /// [`CoreModel::derivations`], and repeating them here would credit S24 with a conclusion it
    /// did not add. What this returns is precisely what the transitivity licensed.
    pub(crate) fn derivation_to(&self, node: &Node, via: SemanticRelation) -> Option<Derivation> {
        let path = self.path_to(node)?;
        if path.len() < 3 {
            return None;
        }
        let premise = path
            .windows(2)
            .map(|step| format!("{} {via} {}", step[0], step[1]))
            .collect::<Vec<_>>()
            .join(", ");
        Some(Derivation {
            conclusion: format!("{} {via} {node}", self.origin),
            premise,
            rule: SkosRule::S24.into(),
        })
    }
}

impl CoreModel {
    /// Walk `via` outwards from `concept`, breadth-first, until the hierarchy or the bound runs
    /// out.
    ///
    /// This is S24 and it is the only place in the build that applies it. Nothing is written back
    /// into the model: [`Resource::relations`](crate::Resource::relations) keeps meaning "links
    /// under this property" and never "ancestors" or "descendants", permanently and by design
    /// (`docs/adr/0024`, `docs/adr/0025`).
    ///
    /// Terminates on a cyclic hierarchy. §8.6.8 is explicit that a cycle is consistent with the
    /// SKOS data model, so a walk that hung on one would refuse to read a legal vocabulary; the
    /// cycle comes back as the origin being reached from itself, with a path that names it.
    pub(crate) fn walk(&self, concept: &Node, via: SemanticRelation, bound: WalkBound) -> Walk {
        let mut walk = Walk {
            origin: concept.clone(),
            reached: BTreeMap::new(),
            links_walked: 0,
            complete: true,
        };

        let mut expanded: BTreeSet<Node> = BTreeSet::new();
        let mut queue: VecDeque<Node> = VecDeque::new();
        queue.push_back(concept.clone());

        while let Some(current) = queue.pop_front() {
            // The origin can be reached again through a cycle. It is recorded when that happens —
            // §8.6.8 says the graph is consistent and the fact is true — but its links are
            // followed once, or the walk would go round for ever.
            if !expanded.insert(current.clone()) {
                continue;
            }
            let Some(links) = self
                .resource(&current)
                .and_then(|resource| resource.relations(via))
            else {
                continue;
            };
            for next in links.keys() {
                if walk.links_walked >= bound.max_links {
                    walk.complete = false;
                    return walk;
                }
                walk.links_walked += 1;
                if walk.reached.contains_key(next) {
                    continue;
                }
                if walk.reached.len() >= bound.max_nodes {
                    walk.complete = false;
                    return walk;
                }
                walk.reached.insert(next.clone(), current.clone());
                queue.push_back(next.clone());
            }
        }

        walk
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

    fn s(subject: &Node, predicate: &str, object: &Node) -> Statement {
        Statement::new(
            subject.clone(),
            format!("{}{predicate}", ns::SKOS),
            object.clone(),
        )
    }

    /// The two directions are one walk over inverse properties, and S25/S26 close each into the
    /// other — so what one direction reaches, the other reaches from the far end. Asserted over
    /// every pair in a polyhierarchy with a cycle in it, because this is the property that lets
    /// one implementation answer both questions.
    #[test]
    fn what_the_upward_walk_reaches_the_downward_walk_reaches_from_the_other_end() {
        let (a, b, c, d) = (ex("A"), ex("B"), ex("C"), ex("D"));
        let model = CoreModel::from_statements([
            s(&a, "broader", &b),
            s(&a, "broader", &c),
            s(&b, "broader", &d),
            s(&c, "narrower", &a),
            s(&d, "broader", &b),
        ]);

        for from in [&a, &b, &c, &d] {
            for to in [&a, &b, &c, &d] {
                let up = model
                    .walk(
                        from,
                        SemanticRelation::BroaderTransitive,
                        WalkBound::DEFAULT,
                    )
                    .contains(to);
                let down = model
                    .walk(to, SemanticRelation::NarrowerTransitive, WalkBound::DEFAULT)
                    .contains(from);
                assert_eq!(
                    up, down,
                    "{from} above {to} disagrees with {to} below {from}"
                );
            }
        }
    }

    /// The predecessor list is a tree over the reached concepts and nothing else: every step's
    /// source is either the origin or itself reached, so a caller rendering it never meets a
    /// parent it has not seen.
    #[test]
    fn every_step_starts_from_the_origin_or_from_something_reached() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([s(&a, "narrower", &b), s(&b, "narrower", &c)]);

        let walk = model.walk(&a, SemanticRelation::NarrowerTransitive, WalkBound::DEFAULT);
        for (node, from) in walk.steps() {
            assert!(
                from == walk.origin() || walk.contains(from),
                "{node} was reached from {from}, which is neither the origin nor reached"
            );
        }
    }
}
