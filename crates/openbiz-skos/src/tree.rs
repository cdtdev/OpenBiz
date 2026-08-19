//! The concept tree read downwards and sideways: children, descendants, and siblings.
//!
//! [`ancestry`](crate::ancestry) answers what is *above* a concept. This answers the other two
//! questions a tree view asks — what is directly below it, everything below it, and what sits
//! beside it — over §8 of the SKOS Reference (W3C Recommendation, 18 August 2009).
//!
//! # A child is not the same thing as a descendant one step down
//!
//! This is the subtlety in the module and it comes straight out of S22, so it is worth stating
//! before anything else.
//!
//! S22 makes `skos:narrower` a **sub-property of** `skos:narrowerTransitive`. A sub-property
//! entails its super-property and not the other way round, so:
//!
//! - `<A> skos:narrower <B>` entails `<A> skos:narrowerTransitive <B>`. B is a child of A **and**
//!   a descendant of A.
//! - `<A> skos:narrowerTransitive <B>` entails **nothing** about `skos:narrower`. B is a
//!   descendant of A and **not** a child of A, and A has no children at all.
//!
//! Both are legal SKOS and the second is not exotic: it is what a vocabulary states when it knows
//! one concept is somewhere under another without claiming to know the intervening levels.
//!
//! So [`CoreModel::children`] reads `skos:narrower` and [`CoreModel::descent`] walks
//! `skos:narrowerTransitive`, and the set of concepts reachable by following children is a subset
//! — sometimes a strict one — of the descendants. A tree view built only from children is
//! therefore an honest picture of the *stated* hierarchy and not of everything the vocabulary
//! entails, and a caller that needs the second must ask for it. Collapsing the two would be the
//! easier code and it would put statements in the graph's mouth.
//!
//! # "Sibling" is our word, not the specification's
//!
//! SKOS has no sibling property and §8 states nothing about one; neither does ISO 25964, whose
//! relationships are BT, NT and RT. [`Siblings`] is therefore a **query over the model, not an
//! entailment**, and it is defined here rather than cited:
//!
//! > A sibling of a concept is another concept that has at least one `skos:broader` concept in
//! > common with it.
//!
//! Three consequences that follow from that sentence and are each tested:
//!
//! 1. **It is one step up and one step down, not transitive.** A concept sharing a *grandparent*
//!    is not a sibling. Widening it to the transitive properties would make every concept under a
//!    large top concept a sibling of every other, which is a true statement about the closure and
//!    a useless answer to the question.
//! 2. **A concept is never its own sibling.** §8.6.7's Example 36 (`<A> skos:broader <A>`) is
//!    consistent, and it makes A its own parent and its own child — from which the definition
//!    above would make A its own sibling. Excluded, because "another concept" is what the word
//!    means and a tree view listing a concept beside itself is a defect however defensible its
//!    derivation.
//! 3. **Concepts with no broader concept are not siblings of each other.** Two top concepts share
//!    no parent, so nothing here relates them. That is a real gap and it is deliberate: what makes
//!    two top concepts belong together is `skos:hasTopConcept` from a shared scheme, which is a
//!    different question with a different answer, and inventing a "root sibling" from the absence
//!    of a link would be a claim the graph does not make.
//!
//! Because no statement licenses a sibling, nothing here emits a [`Derivation`](crate::Derivation)
//! — a fabricated rule number would be worse than no citation. What [`Siblings::through`] returns
//! instead is the concepts shared, so the answer can always be reduced to the two `skos:broader`
//! links behind it, each of which the model already explains.

use std::collections::{BTreeMap, BTreeSet};

use crate::hierarchy::{Walk, WalkBound};
use crate::model::{CoreModel, Derivation, Node};
use crate::relations::{RelationOrigin, SemanticRelation};

/// Everything below one concept in the hierarchy, and the path that reached each of them.
///
/// "Below" is `skos:narrowerTransitive`, which after the closure in
/// [`relations`](crate::relations) holds the links S22 lifted from `skos:narrower`, the ones S25
/// and S26 turned round from `skos:broader`, and the ones the graph stated itself. This walks
/// those one-step links; what it adds is S24.
///
/// The mirror of [`Ancestry`](crate::Ancestry) in every respect, including the ones that are
/// uncomfortable: the origin is a descendant of itself when a cycle makes it one (§8.6.8's
/// Example 37), a concept reachable two ways is reported once by the shorter route (§8.6.9), and
/// an abandoned walk is distinguishable from a finished one. Read
/// [`WalkBound`](crate::WalkBound)'s note before assuming the default bound is as roomy going
/// down as it is going up — it is not, because everything under a top concept is most of the
/// vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descent(Walk);

impl Descent {
    /// The concept the walk started from.
    pub fn origin(&self) -> &Node {
        self.0.origin()
    }

    /// Whether the walk ran out of hierarchy rather than out of budget.
    ///
    /// `false` means the answer is a lower bound and nothing may be concluded from an *absence*
    /// in it. On a large vocabulary walked from a top concept this is the expected answer, not an
    /// exceptional one.
    pub fn is_complete(&self) -> bool {
        self.0.is_complete()
    }

    /// How many descendants were reached.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether nothing below the origin was reached.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// How many links the walk followed. Reported so a bound that was hit says which one.
    pub fn links_walked(&self) -> usize {
        self.0.links_walked()
    }

    /// Whether `node` is below the origin.
    ///
    /// A `false` from an incomplete walk means "not found within the bound", not "not a
    /// descendant".
    pub fn contains(&self, node: &Node) -> bool {
        self.0.contains(node)
    }

    /// Every descendant reached, in a stable order.
    pub fn descendants(&self) -> impl Iterator<Item = &Node> {
        self.0.reached()
    }

    /// Every descendant and the concept the walk stepped from to reach it.
    ///
    /// The breadth-first tree as a predecessor list, which is what a caller rendering an indented
    /// tree needs. Such a caller **must** mark what it has already printed: a cycle is legal SKOS
    /// and puts the origin back among the descendants, so following the list forwards without a
    /// guard does not terminate.
    pub fn steps(&self) -> impl Iterator<Item = (&Node, &Node)> {
        self.0.steps()
    }

    /// The path the walk took from the origin down to `node`, origin first and `node` last.
    ///
    /// Breadth-first, so it is a shortest path. A concept below the origin by two routes gets one
    /// of them, and the model does not claim it is the only one.
    pub fn path_to(&self, node: &Node) -> Option<Vec<Node>> {
        self.0.path_to(node)
    }

    /// Why `node` is below the origin, as the derivation `CLAUDE.md` §3 requires.
    ///
    /// `None` when `node` is not a descendant, and **also** when the path is a single link, which
    /// is S22's conclusion or the graph's own rather than S24's. What this returns is precisely
    /// what the transitivity licensed.
    pub fn derivation_to(&self, node: &Node) -> Option<Derivation> {
        self.0
            .derivation_to(node, SemanticRelation::NarrowerTransitive)
    }

    /// Which descendants survive when the caller wants none of `skip`, and which of `skip` cannot
    /// be taken out without taking a survivor with it.
    ///
    /// The walk itself is **not** narrowed and this takes no part in it: a concept in `skip` may
    /// be the only route to concepts the caller does want, so it has to be walked *through*
    /// whatever the caller thinks of it. This is therefore a decision about rendering a finished
    /// descent, and it is the one place where a hierarchy differs from a flat list — see
    /// [`CoreModel::search_excluding`], where the exclusion runs inside the scan because there is
    /// no structure to preserve.
    ///
    /// The rule is a single sentence: **a branch goes only when the whole branch is in `skip`.**
    /// A member of `skip` lying on the tree's path to a survivor is retained as a *route*
    /// ([`Pruned::is_route`]) rather than dropped. Nothing is lifted and nothing is re-parented,
    /// so every concept shown keeps the depth, the parent, and the derivation the unpruned tree
    /// gave it, and the pruning can never make the tree state a link the graph does not.
    ///
    /// As everywhere else in this crate, the set is of *nodes*: this is told which resources to
    /// leave out and never why.
    pub fn excluding<'a>(&'a self, skip: &BTreeSet<Node>) -> Pruned<'a> {
        let mut from: BTreeMap<&Node, &Node> = BTreeMap::new();
        for (node, predecessor) in self.steps() {
            from.insert(node, predecessor);
        }

        let mut shown: BTreeSet<&Node> = BTreeSet::new();
        let mut routes: BTreeSet<&Node> = BTreeSet::new();
        let mut kept = 0usize;
        for node in self.descendants() {
            if skip.contains(node) {
                continue;
            }
            kept += 1;
            if !shown.insert(node) {
                continue;
            }
            // Up the tree's own predecessor list, marking everything between this survivor and
            // the origin. Every node reached is shown; the ones in `skip` are shown as routes.
            let mut at = node;
            while let Some(predecessor) = from.get(at).copied() {
                // The origin is the root of the report and is never pruned, and stopping here is
                // also what terminates a cyclic hierarchy, which §8.6.8 says is consistent.
                if predecessor == self.origin() {
                    break;
                }
                if skip.contains(predecessor) {
                    routes.insert(predecessor);
                }
                // Anything already shown has already had its own chain walked, so there is
                // nothing above it left to mark.
                if !shown.insert(predecessor) {
                    break;
                }
                at = predecessor;
            }
        }

        // Counted rather than subtracted: the three numbers have to add up to the descent's own
        // length and a report that derives one of them from the other two cannot notice when they
        // do not.
        let dropped = self
            .descendants()
            .filter(|node| skip.contains(*node) && !routes.contains(node))
            .count();
        Pruned {
            shown,
            routes,
            kept,
            dropped,
        }
    }
}

/// What a [`Descent`] shows once a set of concepts is asked to be left out of it.
///
/// Three numbers and two questions, and the numbers are the point: [`Pruned::dropped`] and
/// [`Pruned::routes`] are what a report has to state so that narrowing a tree never reads as a
/// vocabulary that is smaller than it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pruned<'a> {
    shown: BTreeSet<&'a Node>,
    routes: BTreeSet<&'a Node>,
    kept: usize,
    dropped: usize,
}

impl Pruned<'_> {
    /// Whether the tree still shows `node`, either on its own account or as a route.
    pub fn shows(&self, node: &Node) -> bool {
        self.shown.contains(node)
    }

    /// Whether `node` is one of the excluded concepts, kept only because survivors sit below it.
    ///
    /// A caller rendering the tree must say so against the line: an excluded concept appearing
    /// unremarked in a report that was asked to leave those out is worse than not narrowing at
    /// all.
    pub fn is_route(&self, node: &Node) -> bool {
        self.routes.contains(node)
    }

    /// How many descendants were not in the excluded set. The size of the answer asked for.
    pub fn kept(&self) -> usize {
        self.kept
    }

    /// How many excluded concepts are still shown, because something kept sits below them.
    pub fn routes(&self) -> usize {
        self.routes.len()
    }

    /// How many excluded concepts went, taking nothing kept with them.
    pub fn dropped(&self) -> usize {
        self.dropped
    }
}

/// The concepts sharing a broader concept with one concept, and which concepts they share.
///
/// Our query and not a SKOS entailment — see the module note for the definition and for the three
/// things it deliberately does not do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Siblings {
    origin: Node,
    /// Each sibling, and every concept it and the origin both have as a broader concept. A set
    /// rather than one node because polyhierarchy is ordinary: two concepts can be siblings twice
    /// over, and an author looking at an unexpected sibling wants to know which parent put it
    /// there.
    shared: BTreeMap<Node, BTreeSet<Node>>,
    parents: usize,
    links_walked: usize,
    complete: bool,
}

impl Siblings {
    /// The concept the siblings are siblings of.
    pub fn origin(&self) -> &Node {
        &self.origin
    }

    /// Whether the search ran out of hierarchy rather than out of budget.
    ///
    /// `false` means the list is a lower bound: a concept absent from it may still be a sibling.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// How many siblings were found.
    pub fn len(&self) -> usize {
        self.shared.len()
    }

    /// Whether the concept has no siblings — or none the search reached.
    pub fn is_empty(&self) -> bool {
        self.shared.is_empty()
    }

    /// How many broader concepts of the origin were looked through.
    ///
    /// A concept with no broader concept at all has no siblings under this definition, and this
    /// is what tells that apart from a concept whose parents simply have no other children.
    pub fn parents(&self) -> usize {
        self.parents
    }

    /// How many links the search followed, up and down together.
    pub fn links_walked(&self) -> usize {
        self.links_walked
    }

    /// Every sibling found, in a stable order.
    pub fn siblings(&self) -> impl Iterator<Item = &Node> {
        self.shared.keys()
    }

    /// The concepts `node` and the origin both have as broader concepts, if `node` is a sibling.
    ///
    /// This is the answer to "why is this beside me": each concept named here stands for a pair of
    /// `skos:broader` links the model can explain individually.
    pub fn through(&self, node: &Node) -> Option<&BTreeSet<Node>> {
        self.shared.get(node)
    }
}

impl CoreModel {
    /// The concepts one `skos:narrower` link below `concept`, and how each link was established.
    ///
    /// One step and stated: this reads `skos:narrower`, which holds what the graph stated and what
    /// S25 turned round from `skos:broader`, and it does **not** read `skos:narrowerTransitive`.
    /// A concept below this one by a stated transitive link is a descendant and not a child — see
    /// the module note, which is the one thing to read before using this.
    ///
    /// Empty for a concept the graph never mentions, and empty for one with no narrower concepts.
    /// A caller that needs to tell those apart must ask the model whether it has the resource at
    /// all; a leaf and a typo look identical here and mean opposite things.
    pub fn children(&self, concept: &Node) -> impl Iterator<Item = (&Node, &RelationOrigin)> {
        self.resource(concept)
            .and_then(|resource| resource.relations(SemanticRelation::Narrower))
            .into_iter()
            .flatten()
    }

    /// Walk `skos:narrowerTransitive` downwards from `concept` and report what is below it.
    ///
    /// The mirror of [`CoreModel::ancestry`], over the same bounded walk. Terminates on a cyclic
    /// hierarchy, which §8.6.8 says is consistent; the cycle comes back as the origin being its
    /// own descendant, with a path that names it.
    pub fn descent(&self, concept: &Node, bound: WalkBound) -> Descent {
        Descent(self.walk(concept, SemanticRelation::NarrowerTransitive, bound))
    }

    /// The concepts sharing a broader concept with `concept`.
    ///
    /// One step up `skos:broader` and one step back down `skos:narrower`, with the origin removed
    /// from its own answer. Not transitive, and not a SKOS entailment — the module note has the
    /// definition and the reasoning.
    ///
    /// Bounded the same way the walks are, and for the same reason: a concept under a top concept
    /// with a hundred thousand children has a hundred thousand siblings, and a report that
    /// silently returned the first few of them would be indistinguishable from one where that was
    /// all there were.
    pub fn siblings(&self, concept: &Node, bound: WalkBound) -> Siblings {
        let mut siblings = Siblings {
            origin: concept.clone(),
            shared: BTreeMap::new(),
            parents: 0,
            links_walked: 0,
            complete: true,
        };

        let Some(resource) = self.resource(concept) else {
            return siblings;
        };
        let Some(parents) = resource.relations(SemanticRelation::Broader) else {
            return siblings;
        };

        for parent in parents.keys() {
            if siblings.links_walked >= bound.max_links {
                siblings.complete = false;
                return siblings;
            }
            siblings.links_walked += 1;
            siblings.parents += 1;

            let Some(children) = self
                .resource(parent)
                .and_then(|resource| resource.relations(SemanticRelation::Narrower))
            else {
                continue;
            };
            for child in children.keys() {
                if siblings.links_walked >= bound.max_links {
                    siblings.complete = false;
                    return siblings;
                }
                siblings.links_walked += 1;
                // A concept is not its own sibling, even where §8.6.7's Example 36 makes it its
                // own parent and its own child. See the module note.
                if child == concept {
                    continue;
                }
                if !siblings.shared.contains_key(child) && siblings.shared.len() >= bound.max_nodes
                {
                    siblings.complete = false;
                    return siblings;
                }
                siblings
                    .shared
                    .entry(child.clone())
                    .or_default()
                    .insert(parent.clone());
            }
        }

        siblings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SkosRule, Statement};
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

    fn children_of(model: &CoreModel, concept: &Node) -> Vec<Node> {
        model
            .children(concept)
            .map(|(child, _)| child.clone())
            .collect()
    }

    /// The rule the whole pruning is: a branch goes only when the whole branch is excluded. A
    /// leaf that nobody wants is dropped; the same concept with a survivor under it is kept as a
    /// route, and the survivor keeps the depth and the parent the unpruned tree gave it.
    #[test]
    fn an_excluded_concept_with_a_survivor_below_it_is_kept_as_a_route() {
        let (top, gone, kept, alone) = (ex("Top"), ex("Gone"), ex("Kept"), ex("Alone"));
        let model = CoreModel::from_statements([
            s(&top, &skos("narrower"), &gone),
            s(&gone, &skos("narrower"), &kept),
            s(&top, &skos("narrower"), &alone),
        ]);

        let below = model.descent(&top, WalkBound::DEFAULT);
        let pruned = below.excluding(&BTreeSet::from([gone.clone(), alone.clone()]));

        assert!(pruned.shows(&kept));
        assert!(pruned.shows(&gone), "the only route to Kept");
        assert!(pruned.is_route(&gone));
        assert!(
            !pruned.shows(&alone),
            "excluded, and nothing kept is below it"
        );
        assert_eq!(
            (pruned.kept(), pruned.routes(), pruned.dropped()),
            (1, 1, 1)
        );
        assert_eq!(
            below.path_to(&kept),
            Some(vec![top.clone(), gone.clone(), kept.clone()]),
            "nothing is lifted: the path through the excluded concept is unchanged"
        );
    }

    /// The three numbers are the report's whole honesty and they must account for every
    /// descendant. A subtree several levels deep, excluded all the way down, is the case where a
    /// count derived by subtraction from a partial walk would quietly disagree.
    #[test]
    fn every_descendant_is_kept_a_route_or_dropped_and_nothing_else() {
        let (top, a, b, c, d) = (ex("Top"), ex("A"), ex("B"), ex("C"), ex("D"));
        let model = CoreModel::from_statements([
            s(&top, &skos("narrower"), &a),
            s(&a, &skos("narrower"), &b),
            s(&b, &skos("narrower"), &c),
            s(&top, &skos("narrower"), &d),
        ]);

        let below = model.descent(&top, WalkBound::DEFAULT);
        let pruned = below.excluding(&BTreeSet::from([a.clone(), b.clone(), c.clone()]));

        assert_eq!(
            pruned.kept() + pruned.routes() + pruned.dropped(),
            below.len()
        );
        assert_eq!(
            (pruned.kept(), pruned.routes(), pruned.dropped()),
            (1, 0, 3)
        );
        assert!(pruned.shows(&d) && !pruned.shows(&a));
    }

    /// Excluding everything must leave an empty tree and a number saying so, not an empty tree
    /// that reads as a concept with nothing below it. This is the case the report's wording is
    /// built around.
    #[test]
    fn excluding_every_descendant_leaves_the_count_behind() {
        let (top, a, b) = (ex("Top"), ex("A"), ex("B"));
        let model = CoreModel::from_statements([
            s(&top, &skos("narrower"), &a),
            s(&a, &skos("narrower"), &b),
        ]);

        let below = model.descent(&top, WalkBound::DEFAULT);
        let pruned = below.excluding(&BTreeSet::from([a.clone(), b.clone()]));

        assert_eq!(
            (pruned.kept(), pruned.routes(), pruned.dropped()),
            (0, 0, 2)
        );
        assert!(!pruned.shows(&a) && !pruned.shows(&b));
    }

    /// An empty exclusion changes nothing, which is what makes the flag safe to leave off.
    #[test]
    fn excluding_nothing_shows_every_descendant() {
        let (top, a, b) = (ex("Top"), ex("A"), ex("B"));
        let model = CoreModel::from_statements([
            s(&top, &skos("narrower"), &a),
            s(&a, &skos("narrower"), &b),
        ]);

        let below = model.descent(&top, WalkBound::DEFAULT);
        let pruned = below.excluding(&BTreeSet::new());

        assert_eq!(
            (pruned.kept(), pruned.routes(), pruned.dropped()),
            (2, 0, 0)
        );
        assert!(pruned.shows(&a) && pruned.shows(&b));
    }

    /// §8.6.8 says a cyclic hierarchy is consistent, and a cycle puts the origin back among its
    /// own descendants. Walking each survivor's chain upwards must terminate on it rather than
    /// going round for ever.
    #[test]
    fn a_cycle_terminates_the_walk_back_up() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([
            s(&a, &skos("narrower"), &b),
            s(&b, &skos("narrower"), &c),
            s(&c, &skos("narrower"), &a),
        ]);

        let below = model.descent(&a, WalkBound::DEFAULT);
        let pruned = below.excluding(&BTreeSet::from([b.clone()]));

        assert!(pruned.shows(&c) && pruned.is_route(&b));
        assert_eq!(
            pruned.kept() + pruned.routes() + pruned.dropped(),
            below.len()
        );
    }

    /// A concept below the origin by two routes is printed once, under the shorter — so an
    /// exclusion on the *other* route must not resurrect that route as a structural line.
    #[test]
    fn only_the_route_the_tree_actually_took_is_kept() {
        let (top, long, short, leaf) = (ex("Top"), ex("Long"), ex("Short"), ex("Leaf"));
        let model = CoreModel::from_statements([
            s(&top, &skos("narrower"), &short),
            s(&short, &skos("narrower"), &leaf),
            s(&top, &skos("narrower"), &long),
            s(&long, &skos("narrower"), &leaf),
        ]);

        let below = model.descent(&top, WalkBound::DEFAULT);
        let pruned = below.excluding(&BTreeSet::from([short.clone(), long.clone()]));

        let reached = below.path_to(&leaf).expect("Leaf is below Top");
        let through = &reached[1];
        assert!(pruned.is_route(through), "the route the tree took is kept");
        let other = match through == &short {
            true => &long,
            false => &short,
        };
        assert!(
            !pruned.shows(other),
            "the route the tree did not take is not resurrected by the pruning"
        );
    }

    /// §8.6.6's Example 35 read downwards. `<A> broader <B> . <B> broader <C> .` entails
    /// `<C> narrowerTransitive <A>` by S25, S22 and S24 together, and the derivation names the
    /// path rather than asserting the endpoint.
    #[test]
    fn example_35_entails_the_downward_link_across_two_steps() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        let below = model.descent(&c, WalkBound::DEFAULT);
        assert!(below.is_complete());
        assert_eq!(below.len(), 2);
        assert!(below.contains(&a) && below.contains(&b));
        assert_eq!(
            below.path_to(&a),
            Some(vec![c.clone(), b.clone(), a.clone()])
        );

        assert_eq!(
            below.derivation_to(&b),
            None,
            "one step is S22's, not S24's"
        );
        let why = below.derivation_to(&a).expect("S24 licensed the far link");
        assert_eq!(why.rule, SkosRule::S24);
        assert_eq!(
            why.premise,
            format!("{c} skos:narrowerTransitive {b}, {b} skos:narrowerTransitive {a}")
        );
        assert_eq!(why.conclusion, format!("{c} skos:narrowerTransitive {a}"));
    }

    /// The walk goes down and not up, which is the defect a mirrored implementation would hide.
    #[test]
    fn the_downward_walk_has_a_direction() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        assert!(model.descent(&a, WalkBound::DEFAULT).is_empty());
        assert_eq!(model.descent(&b, WalkBound::DEFAULT).len(), 1);
    }

    /// **The module's central distinction.** S22 runs one way: a stated `skos:narrowerTransitive`
    /// makes B a descendant of A and leaves A with no children at all, while a stated
    /// `skos:narrower` makes B both. A tree built from children is the stated hierarchy; a tree
    /// built from descendants is everything the vocabulary entails, and they are not the same
    /// tree.
    #[test]
    fn a_stated_transitive_link_is_a_descendant_and_not_a_child() {
        let (a, b) = (ex("A"), ex("B"));

        let transitively = CoreModel::from_statements([s(&a, &skos("narrowerTransitive"), &b)]);
        assert!(transitively.descent(&a, WalkBound::DEFAULT).contains(&b));
        assert_eq!(
            children_of(&transitively, &a),
            Vec::<Node>::new(),
            "S22 does not run downwards, so nothing entails skos:narrower here"
        );

        let directly = CoreModel::from_statements([s(&a, &skos("narrower"), &b)]);
        assert!(directly.descent(&a, WalkBound::DEFAULT).contains(&b));
        assert_eq!(children_of(&directly, &a), vec![b]);
    }

    /// A child written the other way round is still a child: S25 turns `<B> broader <A>` into
    /// `<A> narrower <B>`, and the origin says which statement did it.
    #[test]
    fn a_hierarchy_written_upwards_still_has_children() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements([s(&b, &skos("broader"), &a)]);

        let children: Vec<_> = model.children(&a).collect();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].0, &b);
        assert_eq!(children[0].1, &RelationOrigin::Entailed(SkosRule::S25));

        // And the direction the graph did state is asserted, not inferred.
        let stated: Vec<_> = model.children(&b).collect();
        assert!(stated.is_empty());
        assert_eq!(
            model
                .resource(&b)
                .and_then(|r| r.relations(SemanticRelation::Broader))
                .and_then(|links| links.get(&a)),
            Some(&RelationOrigin::Asserted)
        );
    }

    /// A concept the graph never mentions has no children and no descendants, and asking is not
    /// an error. It is also indistinguishable from a leaf, which is why the doc comment says so.
    #[test]
    fn a_concept_the_graph_never_mentions_has_nothing_below_it() {
        let model = CoreModel::from_statements([]);
        let below = model.descent(&ex("A"), WalkBound::DEFAULT);
        assert!(below.is_complete() && below.is_empty());
        assert_eq!(children_of(&model, &ex("A")), Vec::<Node>::new());
        assert!(model.siblings(&ex("A"), WalkBound::DEFAULT).is_empty());
    }

    /// §8.6.8, Example 37 — a two-concept cycle is **consistent**. The downward walk must
    /// terminate, must report the origin as its own descendant, and must be able to say why.
    #[test]
    fn example_37_terminates_going_down_too() {
        let (a, b) = (ex("A"), ex("B"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &a)]);

        let below = model.descent(&a, WalkBound::DEFAULT);
        assert!(below.is_complete(), "a cycle is not a bound being hit");
        assert!(below.contains(&a), "the cycle makes A its own descendant");
        assert!(below.contains(&b));
        assert_eq!(
            below.path_to(&a),
            Some(vec![a.clone(), b.clone(), a.clone()])
        );
        assert_eq!(
            below
                .derivation_to(&a)
                .expect("the cycle is an S24 conclusion")
                .rule,
            SkosRule::S24
        );
        assert!(model.findings().is_empty(), "{:?}", model.findings());
    }

    /// §8.6.9, Examples 38 and 39 — two routes to the same concept, both consistent. The
    /// descendant is reported once, by the shorter route, and polyhierarchy is not a finding.
    #[test]
    fn example_38_reports_one_shortest_path_downwards() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([
            s(&a, &skos("broader"), &b),
            s(&a, &skos("broader"), &c),
            s(&b, &skos("broader"), &c),
        ]);

        let below = model.descent(&c, WalkBound::DEFAULT);
        assert_eq!(below.len(), 2);
        assert_eq!(below.path_to(&a), Some(vec![c.clone(), a.clone()]));
        assert!(model.findings().is_empty(), "{:?}", model.findings());
    }

    /// A walk down that gave up says so, and an absence from it proves nothing. The same
    /// confusion as upwards, and more likely to be met: the default bound going down is a size an
    /// ordinary vocabulary reaches.
    #[test]
    fn a_bounded_descent_is_distinguishable_from_a_finished_one() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model = CoreModel::from_statements([
            s(&a, &skos("narrower"), &b),
            s(&b, &skos("narrower"), &c),
        ]);

        let bounded = model.descent(&a, WalkBound::new(1, usize::MAX));
        assert!(!bounded.is_complete());
        assert_eq!(bounded.len(), 1);
        assert!(!bounded.contains(&c), "the walk never got there");

        let by_links = model.descent(&a, WalkBound::new(usize::MAX, 1));
        assert!(!by_links.is_complete());
        assert_eq!(by_links.links_walked(), 1);

        assert!(model.descent(&a, WalkBound::new(2, 2)).is_complete());
    }

    /// A blank node is a perfectly good concept and the walk must not lose one.
    #[test]
    fn a_chain_of_blank_nodes_walks_downwards_like_any_other() {
        let (a, b, c) = (Node::blank("a"), Node::blank("b"), Node::blank("c"));
        let model = CoreModel::from_statements([
            s(&a, &skos("narrower"), &b),
            s(&b, &skos("narrower"), &c),
        ]);

        assert_eq!(
            model.descent(&a, WalkBound::DEFAULT).path_to(&c),
            Some(vec![a, b, c])
        );
    }

    /// §8.6.4's Example 32 — the associative relation is not transitive and is not a hierarchy.
    /// It must never be walked as one in either direction, and it must never make a sibling.
    #[test]
    fn example_32_is_not_walked_as_a_hierarchy_downwards() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("related"), &b), s(&b, &skos("related"), &c)]);

        assert!(model.descent(&a, WalkBound::DEFAULT).is_empty());
        assert_eq!(children_of(&model, &a), Vec::<Node>::new());
        assert!(model.siblings(&a, WalkBound::DEFAULT).is_empty());
    }

    /// The definition, at its simplest: two concepts under one parent are each other's siblings,
    /// and the answer names the parent they share.
    #[test]
    fn two_concepts_under_one_parent_are_siblings_through_it() {
        let (a, b, p) = (ex("A"), ex("B"), ex("P"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &p), s(&b, &skos("broader"), &p)]);

        let beside = model.siblings(&a, WalkBound::DEFAULT);
        assert!(beside.is_complete());
        assert_eq!(beside.origin(), &a);
        assert_eq!(beside.parents(), 1);
        assert_eq!(beside.siblings().collect::<Vec<_>>(), vec![&b]);
        assert_eq!(beside.through(&b), Some(&BTreeSet::from([p.clone()])));

        // Symmetric, because the definition is.
        let other = model.siblings(&b, WalkBound::DEFAULT);
        assert_eq!(other.siblings().collect::<Vec<_>>(), vec![&a]);
    }

    /// Polyhierarchy makes two concepts siblings twice over, and the answer says through which
    /// parents rather than reporting the sibling twice.
    #[test]
    fn siblings_through_two_parents_are_named_once_with_both() {
        let (a, b, p, q) = (ex("A"), ex("B"), ex("P"), ex("Q"));
        let model = CoreModel::from_statements([
            s(&a, &skos("broader"), &p),
            s(&a, &skos("broader"), &q),
            s(&b, &skos("broader"), &p),
            s(&b, &skos("broader"), &q),
        ]);

        let beside = model.siblings(&a, WalkBound::DEFAULT);
        assert_eq!(beside.parents(), 2);
        assert_eq!(beside.len(), 1);
        assert_eq!(beside.through(&b), Some(&BTreeSet::from([p, q])));
    }

    /// **A concept is never its own sibling**, even where §8.6.7's Example 36 makes it its own
    /// parent and therefore its own child. The graph is consistent and the exclusion is ours.
    #[test]
    fn example_36_does_not_make_a_concept_its_own_sibling() {
        let (a, b) = (ex("A"), ex("B"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &a), s(&b, &skos("broader"), &a)]);

        let beside = model.siblings(&a, WalkBound::DEFAULT);
        assert!(!beside.siblings().any(|node| node == &a));
        // B is under A, and A is under A, so B *is* a sibling of A through A. That follows from
        // the definition and the graph, and it is not softened.
        assert_eq!(beside.through(&b), Some(&BTreeSet::from([a.clone()])));
        assert!(model.findings().is_empty(), "{:?}", model.findings());
    }

    /// Siblings are one step, not the closure: a concept under the origin's grandparent is not a
    /// sibling of it. Widening this to the transitive properties is the mistake the module note
    /// argues against, so it is pinned.
    #[test]
    fn a_concept_under_the_grandparent_is_not_a_sibling() {
        let (a, uncle, parent, grandparent) = (ex("A"), ex("U"), ex("P"), ex("G"));
        let model = CoreModel::from_statements([
            s(&a, &skos("broader"), &parent),
            s(&parent, &skos("broader"), &grandparent),
            s(&uncle, &skos("broader"), &grandparent),
        ]);

        let beside = model.siblings(&a, WalkBound::DEFAULT);
        assert!(
            beside.is_empty(),
            "{:?}",
            beside.siblings().collect::<Vec<_>>()
        );
        // And the concept that *is* a sibling of the parent is found when asked about the parent.
        assert_eq!(
            model
                .siblings(&parent, WalkBound::DEFAULT)
                .siblings()
                .collect::<Vec<_>>(),
            vec![&uncle]
        );
    }

    /// Two top concepts share no broader concept, so nothing here relates them. Deliberate: what
    /// makes them belong together is `skos:hasTopConcept`, a different question.
    #[test]
    fn two_top_concepts_are_not_siblings_of_each_other() {
        let (a, b, scheme) = (ex("A"), ex("B"), ex("Scheme"));
        let model = CoreModel::from_statements([
            s(&scheme, &skos("hasTopConcept"), &a),
            s(&scheme, &skos("hasTopConcept"), &b),
        ]);

        let beside = model.siblings(&a, WalkBound::DEFAULT);
        assert!(beside.is_empty());
        assert_eq!(beside.parents(), 0, "a top concept has no broader concept");
    }

    /// A sibling search that gave up says so, on either of the two bounds, and a concept absent
    /// from an incomplete answer may still be a sibling.
    #[test]
    fn a_bounded_sibling_search_is_distinguishable_from_a_finished_one() {
        let (a, b, c, p) = (ex("A"), ex("B"), ex("C"), ex("P"));
        let model = CoreModel::from_statements([
            s(&a, &skos("broader"), &p),
            s(&b, &skos("broader"), &p),
            s(&c, &skos("broader"), &p),
        ]);

        let by_nodes = model.siblings(&a, WalkBound::new(1, usize::MAX));
        assert!(!by_nodes.is_complete());
        assert_eq!(by_nodes.len(), 1);

        let by_links = model.siblings(&a, WalkBound::new(usize::MAX, 1));
        assert!(!by_links.is_complete());
        assert!(by_links.is_empty(), "the budget went on the step upwards");

        let complete = model.siblings(&a, WalkBound::DEFAULT);
        assert!(complete.is_complete());
        assert_eq!(complete.len(), 2);
    }

    /// **The same asymmetry, on the step upwards.** A concept whose only upward link is a stated
    /// `skos:broaderTransitive` has no `skos:broader` concept, so under the definition in the
    /// module note it has no siblings — and it is not merely that none were found: it has no
    /// parent to share. Pinned because walking the transitive property here instead would be
    /// invisible on every ordinary vocabulary, where S22 fills it from `skos:broader` anyway, and
    /// would silently invent siblings on the one shape that tells them apart.
    #[test]
    fn a_concept_placed_under_another_only_transitively_has_no_siblings() {
        let (a, b, p) = (ex("A"), ex("B"), ex("P"));
        let model = CoreModel::from_statements([
            s(&a, &skos("broaderTransitive"), &p),
            s(&b, &skos("broader"), &p),
        ]);

        let beside = model.siblings(&a, WalkBound::DEFAULT);
        assert!(beside.is_complete());
        assert!(
            beside.is_empty(),
            "{:?}",
            beside.siblings().collect::<Vec<_>>()
        );
        assert_eq!(
            beside.parents(),
            0,
            "a transitive link upwards is not a broader concept"
        );

        // And B, which *is* under P by skos:broader, is not given A as a sibling either — the
        // definition is symmetric and it excludes A at both ends.
        assert!(model.siblings(&b, WalkBound::DEFAULT).is_empty());
    }

    /// A sibling reached by a stated `skos:narrower` on the parent counts, because that is a
    /// child of the parent however it was written. The step down is `skos:narrower` and not
    /// `skos:narrowerTransitive`, for the reason the module note gives about children.
    #[test]
    fn a_sibling_stated_downwards_from_the_parent_is_found() {
        let (a, b, p) = (ex("A"), ex("B"), ex("P"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &p), s(&p, &skos("narrower"), &b)]);

        assert_eq!(
            model
                .siblings(&a, WalkBound::DEFAULT)
                .siblings()
                .collect::<Vec<_>>(),
            vec![&b]
        );

        // Whereas a merely transitive link from the parent is not a child of it and so makes no
        // sibling — the same asymmetry, one level up.
        let transitive = CoreModel::from_statements([
            s(&a, &skos("broader"), &p),
            s(&p, &skos("narrowerTransitive"), &b),
        ]);
        assert!(transitive.siblings(&a, WalkBound::DEFAULT).is_empty());
    }
}
