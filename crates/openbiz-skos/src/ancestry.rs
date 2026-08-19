//! S24's transitive closure upwards, answered by walking rather than by storing.
//!
//! §8 of the SKOS Reference (W3C Recommendation, 18 August 2009) makes
//! `skos:broaderTransitive` and `skos:narrowerTransitive` `owl:TransitiveProperty` (S24), so a
//! chain of `skos:broader` links entails a link from each concept to every concept above it.
//! §8.6.6's Example 35 is the specification's own statement of that entailment.
//!
//! The walk itself lives in [`hierarchy`](crate::hierarchy) and is shared with the downward one
//! in [`tree`](crate::tree); what is here is the upward reading of it.
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
//! A walk is bounded ([`WalkBound`]). Without one, asking §8.4's question of every concept in
//! a million-link vocabulary is a million traversals of the whole hierarchy, and the honest
//! failure mode of an unbounded walk is a server that stops answering rather than one that says
//! it does not know.
//!
//! [`Ancestry::is_complete`] is therefore not a nicety. A walk that gave up after two ancestors
//! and a concept that genuinely has two ancestors produce the same [`Ancestry::len`], and reading
//! the second answer off the first is exactly how a validator reports "consistent" for a graph it
//! never finished checking. Every caller in this crate branches on it.

use std::collections::BTreeSet;

use crate::hierarchy::{Walk, WalkBound};
use crate::model::{CoreModel, Derivation, Node};
use crate::relations::SemanticRelation;

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
pub struct Ancestry(Walk);

impl Ancestry {
    /// The concept the walk started from.
    pub fn origin(&self) -> &Node {
        self.0.origin()
    }

    /// Whether the walk ran out of ancestors rather than out of budget.
    ///
    /// `false` means the answer is a lower bound and nothing may be concluded from an *absence*
    /// in it. Never ignore this: see the module note.
    pub fn is_complete(&self) -> bool {
        self.0.is_complete()
    }

    /// How many ancestors were reached.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether nothing above the origin was reached.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// How many links the walk followed. Reported so a bound that was hit says which one.
    pub fn links_walked(&self) -> usize {
        self.0.links_walked()
    }

    /// Whether `node` is above the origin.
    ///
    /// A `false` from an incomplete walk means "not found within the bound", not "not an
    /// ancestor".
    pub fn contains(&self, node: &Node) -> bool {
        self.0.contains(node)
    }

    /// Every ancestor reached, in a stable order.
    pub fn ancestors(&self) -> impl Iterator<Item = &Node> {
        self.0.reached()
    }

    /// The path the walk took from the origin to `node`, origin first and `node` last.
    ///
    /// Breadth-first, so it is a *shortest* path — the one an author is likeliest to recognise as
    /// the route through their hierarchy. A concept reachable by two routes (§8.6.9's Examples 38
    /// and 39, both consistent) gets one of them, and the model does not claim it is the only one.
    pub fn path_to(&self, node: &Node) -> Option<Vec<Node>> {
        self.0.path_to(node)
    }

    /// Why `node` is above the origin, as the derivation `CLAUDE.md` §3 requires.
    ///
    /// `None` when `node` is not an ancestor, and **also** when the path is a single link: a
    /// direct `skos:broaderTransitive` is S22's or the graph's own, both of which are already in
    /// [`CoreModel::derivations`], and repeating them here would credit S24 with a conclusion it
    /// did not add. What this returns is precisely what the transitivity licensed.
    pub fn derivation_to(&self, node: &Node) -> Option<Derivation> {
        self.0
            .derivation_to(node, SemanticRelation::BroaderTransitive)
    }

    /// Which ancestors survive when the caller wants none of `skip`, and which of `skip` are still
    /// printed because they lie on a survivor's path.
    ///
    /// The walk itself is **not** narrowed and this takes no part in it: a concept in `skip` may
    /// be the only route to ancestors the caller does want, so it has to be walked *through*
    /// whatever the caller thinks of it. This is a decision about rendering a finished walk.
    ///
    /// The rule differs from [`Descent::excluding`](crate::Descent::excluding) and the difference
    /// is the whole point of having two. Downwards, dropping a concept takes its subtree with it,
    /// so a concept in `skip` is *kept* when survivors hang off it. Upwards nothing hangs off
    /// anything: an ancestor is in the list on its own account, so every member of `skip` leaves
    /// the list. What it cannot leave is the **path**, which is the derivation — editing a concept
    /// out of the middle of `A → B → C` would make the report state that `C` is directly above
    /// `A`, which the graph does not say. So the paths are untouched and
    /// [`Above::on_the_way`] names what stayed in them.
    ///
    /// As everywhere else in this crate, the set is of *nodes*: this is told which resources to
    /// leave out and never why.
    pub fn excluding<'a>(&'a self, skip: &BTreeSet<Node>) -> Above<'a> {
        let mut listed: Vec<&Node> = Vec::new();
        let mut dropped = 0usize;
        for ancestor in self.ancestors() {
            if skip.contains(ancestor) {
                dropped += 1;
                continue;
            }
            listed.push(ancestor);
        }

        // Only the paths of the ancestors that survived: a concept in `skip` reached solely by way
        // of another concept in `skip` is gone from the report entirely, and naming it as "on the
        // way" to something the report does not show would be naming it for no reason.
        let mut on_the_way: BTreeSet<Node> = BTreeSet::new();
        for ancestor in &listed {
            let Some(path) = self.path_to(ancestor) else {
                continue;
            };
            for node in path {
                // Never the origin. It is the start of every path rather than a concept between
                // two others, so it neither can be edited out nor is one of the ancestors this
                // narrowing is about — and a caller that named it here would be contradicting its
                // own statement that the concept asked about is exempt.
                if &node != self.origin() && skip.contains(&node) {
                    on_the_way.insert(node);
                }
            }
        }

        Above {
            listed,
            on_the_way,
            dropped,
        }
    }
}

/// What an [`Ancestry`] lists once a set of concepts is asked to be left out of it.
///
/// Two counts and they are the point: [`Above::dropped`] is what a report has to state so that a
/// narrowed list of ancestors never reads as a concept with nothing above it, and
/// [`Above::on_the_way`] is what it has to state so that an excluded concept appearing in a
/// printed derivation does not read as a bug in the narrowing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Above<'a> {
    listed: Vec<&'a Node>,
    on_the_way: BTreeSet<Node>,
    dropped: usize,
}

impl Above<'_> {
    /// The surviving ancestors, in the order the walk reached them.
    pub fn listed(&self) -> impl Iterator<Item = &Node> {
        self.listed.iter().copied()
    }

    /// How many ancestors were not in the excluded set. The size of the answer asked for.
    pub fn len(&self) -> usize {
        self.listed.len()
    }

    /// Whether nothing above the origin survived the exclusion.
    ///
    /// **Not** the same as having no ancestors, and a report that confuses the two says a concept
    /// is a root of the hierarchy when the vocabulary puts concepts above it. Ask
    /// [`Above::dropped`] which of the two it is.
    pub fn is_empty(&self) -> bool {
        self.listed.is_empty()
    }

    /// How many ancestors were in the excluded set and are therefore not listed.
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    /// The excluded concepts still printed, because a listed ancestor's path runs through them.
    ///
    /// A caller must name these. Under a flag that says the excluded concepts are out, one turning
    /// up unremarked in a derivation reads as the flag not working.
    ///
    /// Always a subset of what [`Above::dropped`] counted, and never the origin — so a caller may
    /// state these as *part of* the concepts it withheld rather than as a second set, which would
    /// tell a reader there are more retired concepts above than there are.
    pub fn on_the_way(&self) -> impl Iterator<Item = &Node> {
        self.on_the_way.iter()
    }
}

impl CoreModel {
    /// Walk `skos:broaderTransitive` upwards from `concept` and report what is above it.
    ///
    /// Nothing is written back into the model; the walk and its bound are
    /// [`hierarchy`](crate::hierarchy)'s, and the mirror of this — everything *below* a concept —
    /// is [`CoreModel::descent`].
    ///
    /// Terminates on a cyclic hierarchy. §8.6.8 is explicit that a cycle is consistent with the
    /// SKOS data model, so a walk that hung on one would refuse to read a legal vocabulary; the
    /// cycle comes back as the origin being its own ancestor, with a path that names it.
    pub fn ancestry(&self, concept: &Node, bound: WalkBound) -> Ancestry {
        Ancestry(self.walk(concept, SemanticRelation::BroaderTransitive, bound))
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

    /// A chain of `skos:broader`, walked. §8.6.6's Example 35 is the specification's own statement
    /// that `<A> broader <B> . <B> broader <C> .` entails `<A> broaderTransitive <C>`.
    #[test]
    fn example_35_entails_the_link_across_two_steps() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        let above = model.ancestry(&a, WalkBound::DEFAULT);
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

        assert!(model.ancestry(&c, WalkBound::DEFAULT).is_empty());
        assert_eq!(model.ancestry(&b, WalkBound::DEFAULT).len(), 1);
    }

    /// §8.6.8, Example 37 — a cycle is **consistent** with the SKOS data model. The walk must
    /// terminate, must report the origin as its own ancestor, and must be able to say why.
    #[test]
    fn example_37_terminates_and_names_the_cycle() {
        let (a, b) = (ex("A"), ex("B"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &a)]);

        let above = model.ancestry(&a, WalkBound::DEFAULT);
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

        let above = model.ancestry(&a, WalkBound::DEFAULT);
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

        let above = model.ancestry(&a, WalkBound::DEFAULT);
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

        let bounded = model.ancestry(&a, WalkBound::new(1, usize::MAX));
        assert!(!bounded.is_complete());
        assert_eq!(bounded.len(), 1);
        assert!(!bounded.contains(&c), "the walk never got there");

        let by_links = model.ancestry(&a, WalkBound::new(usize::MAX, 1));
        assert!(!by_links.is_complete());
        assert_eq!(by_links.links_walked(), 1);

        // And the same walk with room finishes, so the difference is the bound and not the graph.
        assert!(model.ancestry(&a, WalkBound::new(2, 2)).is_complete());
    }

    /// A concept nothing in the graph mentions has no ancestors, and asking is not an error.
    #[test]
    fn a_concept_the_graph_never_mentions_has_no_ancestors() {
        let model = CoreModel::from_statements([]);
        let above = model.ancestry(&ex("A"), WalkBound::DEFAULT);
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
        assert_eq!(downwards.ancestry(&a, WalkBound::DEFAULT).len(), 2);

        let transitively = CoreModel::from_statements([
            s(&a, &skos("broaderTransitive"), &b),
            s(&b, &skos("broaderTransitive"), &c),
        ]);
        let above = transitively.ancestry(&a, WalkBound::DEFAULT);
        assert!(above.contains(&c), "S24 closes stated transitive links too");
    }

    /// A blank node is a perfectly good concept and the walk must not lose one.
    #[test]
    fn a_chain_of_blank_nodes_walks_like_any_other() {
        let (a, b, c) = (Node::blank("a"), Node::blank("b"), Node::blank("c"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        assert_eq!(
            model.ancestry(&a, WalkBound::DEFAULT).path_to(&c),
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

        assert!(model.ancestry(&a, WalkBound::DEFAULT).is_empty());
    }

    /// `docs/adr/0045` §1. The excluded concept leaves the *list* — nothing hangs off an ancestor,
    /// so taking it out costs nothing — but the ancestors above it stay, because a deprecation
    /// removed no link and they are still above the origin.
    #[test]
    fn excluding_drops_the_ancestor_and_keeps_what_is_above_it() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        let above = model.ancestry(&a, WalkBound::DEFAULT);
        let narrowed = above.excluding(&BTreeSet::from([b.clone()]));

        assert_eq!(narrowed.len(), 1);
        assert_eq!(narrowed.listed().collect::<Vec<_>>(), vec![&c]);
        assert_eq!(narrowed.dropped(), 1);
    }

    /// The other half of the same rule, and the reason `on_the_way` exists: the derivation for the
    /// surviving ancestor still runs through the excluded one, and the caller has to be able to
    /// say so. Editing `B` out of `A → B → C` would state that `C` is directly above `A`.
    #[test]
    fn excluding_never_edits_a_path_and_names_what_stayed_in_one() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        let above = model.ancestry(&a, WalkBound::DEFAULT);
        let narrowed = above.excluding(&BTreeSet::from([b.clone()]));

        assert_eq!(narrowed.on_the_way().collect::<Vec<_>>(), vec![&b]);
        assert_eq!(
            above.path_to(&c),
            Some(vec![a.clone(), b.clone(), c.clone()]),
            "the walk's own paths are untouched by the exclusion"
        );
    }

    /// An excluded concept reached only by way of another excluded concept is gone from the report
    /// entirely, so naming it as "on the way" to something nothing shows would be noise.
    #[test]
    fn an_excluded_concept_on_no_surviving_path_is_not_named() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        let above = model.ancestry(&a, WalkBound::DEFAULT);
        let narrowed = above.excluding(&BTreeSet::from([b.clone(), c.clone()]));

        assert!(narrowed.is_empty());
        assert_eq!(narrowed.dropped(), 2);
        assert_eq!(narrowed.on_the_way().count(), 0);
    }

    /// The case the counts exist for. Empty is not the same answer as "nothing is above it", and
    /// only `dropped` tells the two apart.
    #[test]
    fn an_emptied_list_is_not_a_concept_with_nothing_above_it() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements([s(&a, &skos("broader"), &b)]);

        let above_a = model.ancestry(&a, WalkBound::DEFAULT);
        let emptied = above_a.excluding(&BTreeSet::from([b.clone()]));
        assert!(emptied.is_empty() && emptied.dropped() == 1);

        let above_b = model.ancestry(&b, WalkBound::DEFAULT);
        let root = above_b.excluding(&BTreeSet::from([b.clone()]));
        assert!(root.is_empty() && root.dropped() == 0);
    }

    /// Excluding nothing is the unnarrowed answer, which is what makes one code path serve both.
    #[test]
    fn excluding_nothing_lists_every_ancestor() {
        let (a, b, c) = (ex("A"), ex("B"), ex("C"));
        let model =
            CoreModel::from_statements([s(&a, &skos("broader"), &b), s(&b, &skos("broader"), &c)]);

        let above = model.ancestry(&a, WalkBound::DEFAULT);
        let narrowed = above.excluding(&BTreeSet::new());

        assert_eq!(narrowed.len(), above.len());
        assert_eq!(narrowed.dropped(), 0);
        assert_eq!(narrowed.on_the_way().count(), 0);
    }

    /// The origin is never "on the way" to anything. It is the start of every path, not a concept
    /// between two others, and a caller naming it as an excluded concept still turning up in a
    /// derivation would contradict its own statement that the concept asked about is exempt.
    #[test]
    fn the_origin_is_never_reported_as_on_the_way() {
        let (a, b) = (ex("A"), ex("B"));
        let model = CoreModel::from_statements([s(&a, &skos("broader"), &b)]);

        let above = model.ancestry(&a, WalkBound::DEFAULT);
        let narrowed = above.excluding(&BTreeSet::from([a.clone()]));

        assert_eq!(narrowed.len(), 1, "excluding the origin lists B as before");
        assert_eq!(narrowed.dropped(), 0);
        assert_eq!(
            narrowed.on_the_way().count(),
            0,
            "the origin is the start of the path, not a concept in the middle of one"
        );
    }
}
