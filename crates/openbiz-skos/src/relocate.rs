//! Moving a subtree: the statements that put one concept under a different broader concept.
//!
//! This is the first of `docs/BUILD-PLAN.md`'s bulk operations and the first producer of a
//! candidate carrying **both** halves of a change — `CLAUDE.md` §3's seam is what makes "these
//! statements go, those arrive" one decision a reviewer takes once rather than two proposals that
//! leave the vocabulary in a state nobody asked for if only the first lands.
//!
//! Nothing here writes. A [`Relocation`] is an *answer*: the statements a move would remove and
//! the statements it would add, computed against a [`CoreModel`] read a moment ago. The caller
//! stages them as a candidate; a human approves them.
//!
//! # Moving a subtree is re-parenting its root, and that is not a shortcut
//!
//! Everything below the concept is below it by its *own* `skos:broader` links, and none of those
//! mention the concept's parent. So a move touches the links between the moved concept and the
//! parent it is leaving, and nothing else — a hundred thousand descendants move because the graph
//! already says they are below it, not because a hundred thousand statements were rewritten. The
//! descendants are counted anyway, because "this moves 40 000 concepts" is the thing a reviewer
//! most needs to be told before approving a two-statement diff.
//!
//! # The direction the vocabulary states a link in is preserved
//!
//! S25 makes `skos:broader` and `skos:narrower` inverses, so a vocabulary may state either, or
//! both, and mean the same hierarchy. A move that always wrote `skos:broader` would quietly
//! convert a vocabulary authored in `skos:narrower` — an export would come back different from
//! what went in, for a reason no one chose. So each direction the graph actually states between
//! the concept and its old parent is removed, and the *same* directions are added between the
//! concept and its new one.
//!
//! What is **not** added is the inverse of what was: a move that stated both directions when the
//! graph stated one would be writing an entailment down as a fact. S25 already gives the reader
//! the other direction, and [`RelationOrigin`] is how a report says which of the two the graph
//! actually carries.
//!
//! # What it refuses, and why each refusal is not a nuisance
//!
//! A move is cheap to propose and expensive to get wrong: it changes where a whole branch of a
//! thesaurus hangs, and the vocabulary is still perfectly consistent afterwards, so nothing
//! downstream will notice. Each refusal below is a case where the operator almost certainly meant
//! something else.
//!
//! - **Into itself, or into its own descendant.** `<A> skos:broader <A>` and a cycle are both
//!   *consistent* SKOS — §8.6.8 is explicit that a cyclic hierarchy is not an inconsistency — so
//!   no integrity condition would catch this and no report would flag it. What it produces is a
//!   branch with no route to a root, which every one of our own hierarchy walks then reports as a
//!   cycle rather than as a thesaurus. The refusal names the route.
//! - **A concept with more than one broader concept, when the operator did not say which link they
//!   meant.** A polyhierarchic concept has several parents and a move replaces exactly one of
//!   them; picking one would be a coin toss whose result is permanent. Naming them and stopping
//!   costs one command.
//! - **A concept with no broader concept at all.** That is not a move, it is *giving* a concept a
//!   parent, and the two differ in a way that matters: a top concept that gains a broader concept
//!   should stop being a top concept, and this operation does not do that. See the note on
//!   [`Relocation::top_concept_of`].
//! - **A `skos:broaderTransitive` or `skos:narrowerTransitive` link stated directly between the
//!   two.** S22 lifts every `skos:broader` into `skos:broaderTransitive`, so the transitive link
//!   is normally an entailment and disappears with the statement that licensed it. A graph that
//!   states it *directly* has said something the move does not remove, and the concept would still
//!   be under its old parent by S24 while every report said it had moved. Refused rather than
//!   quietly removed, because a directly-stated transitive link is unusual enough that the author
//!   meant something by it.
//! - **A hierarchy too large to walk within the bound.** The cycle check is a bounded downward
//!   walk, and an incomplete walk cannot prove the new parent is *not* below the concept. A
//!   refusal that says "could not check" is the only honest answer; proceeding would be a check
//!   that reports success when it did not run.

use std::fmt;

use crate::hierarchy::WalkBound;
use crate::model::{CoreModel, Node, SkosClass, Statement};
use crate::relations::{RelationOrigin, SemanticRelation};

/// Which of the two inverse directions a hierarchy link is stated in.
///
/// Both may be, and then both are removed and both are added. Neither being stated is impossible
/// here: a parent is only a parent because one of them is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StatedDirections {
    /// The graph states `<concept> skos:broader <parent>`.
    broader: bool,
    /// The graph states `<parent> skos:narrower <concept>`.
    narrower: bool,
}

/// What moving one concept under a different broader concept would change.
///
/// Produced by [`CoreModel::relocate`] and applied by nobody: the statements are a proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relocation {
    concept: Node,
    from: Node,
    to: Node,
    removals: Vec<Statement>,
    additions: Vec<Statement>,
    moved_with_it: usize,
    subtree_complete: bool,
    top_concept_of: Vec<Node>,
}

impl Relocation {
    /// The concept being moved.
    pub fn concept(&self) -> &Node {
        &self.concept
    }

    /// The broader concept it is being moved out from under.
    pub fn from(&self) -> &Node {
        &self.from
    }

    /// The broader concept it is being moved under.
    pub fn to(&self) -> &Node {
        &self.to
    }

    /// The statements the move takes out of the vocabulary. Never empty.
    pub fn removals(&self) -> &[Statement] {
        &self.removals
    }

    /// The statements the move puts into the vocabulary. Never empty, and the same length as
    /// [`Relocation::removals`] — a move replaces each stated direction with the same direction.
    pub fn additions(&self) -> &[Statement] {
        &self.additions
    }

    /// How many concepts are below the moved one, and therefore move with it.
    ///
    /// Zero for a leaf. Read [`Relocation::subtree_complete`] before quoting this: a walk that hit
    /// its bound reports what it reached, which is a lower bound and not a count.
    pub fn moved_with_it(&self) -> usize {
        self.moved_with_it
    }

    /// Whether the downward walk that counted the subtree finished.
    ///
    /// Always `true` today, because [`CoreModel::relocate`] refuses a move it could not walk — the
    /// same walk is the cycle check, and an incomplete one cannot prove there is no cycle. Exposed
    /// so a future caller that separates the two cannot silently inherit the assumption.
    pub fn subtree_complete(&self) -> bool {
        self.subtree_complete
    }

    /// The concept schemes that record the moved concept as a **top** concept.
    ///
    /// Reported rather than corrected. A concept that is both a top concept and has a broader
    /// concept is odd, and it was already odd before the move — this operation requires an
    /// existing broader concept, so it can neither create that state nor make it worse. Demoting a
    /// top concept belongs to the operation that gives a concept its *first* parent, which this is
    /// not, and which would need to know which direction of S8 the graph stated. Empty for the
    /// ordinary case.
    pub fn top_concept_of(&self) -> &[Node] {
        &self.top_concept_of
    }
}

/// Why a move was refused. Every one of these is a question the operator has to answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationError {
    /// The vocabulary says nothing about the concept being moved.
    NoSuchConcept {
        /// The IRI that was asked for.
        concept: Node,
    },
    /// The vocabulary knows the resource, and it is not a `skos:Concept`.
    NotAConcept {
        /// The resource that was named.
        resource: Node,
        /// Which side of the move named it.
        role: &'static str,
    },
    /// The vocabulary says nothing about the proposed new broader concept.
    NoSuchParent {
        /// The IRI that was asked for.
        parent: Node,
    },
    /// The concept was asked to be moved under itself.
    IntoItself {
        /// The concept.
        concept: Node,
    },
    /// The proposed new broader concept is already one of the concept's broader concepts.
    AlreadyThere {
        /// The concept.
        concept: Node,
        /// The parent it already has.
        parent: Node,
    },
    /// The concept has no broader concept, so there is nothing to move it out from under.
    NoBroaderConcept {
        /// The concept.
        concept: Node,
    },
    /// The concept has several broader concepts and the operator did not say which to replace.
    AmbiguousParent {
        /// The concept.
        concept: Node,
        /// Every broader concept it has, in a stable order.
        parents: Vec<Node>,
    },
    /// The named old parent is not one of the concept's broader concepts.
    NotAParent {
        /// The concept.
        concept: Node,
        /// What the operator named.
        named: Node,
        /// Every broader concept it actually has, in a stable order.
        parents: Vec<Node>,
    },
    /// The proposed new broader concept is below the concept being moved.
    IntoItsOwnDescendant {
        /// The concept.
        concept: Node,
        /// The proposed new parent, which is below it.
        parent: Node,
        /// The route down from the concept to that parent, the concept first.
        route: Vec<Node>,
    },
    /// A transitive hierarchy link between the concept and its old parent is stated directly.
    ///
    /// Boxed because it is far the largest thing any of these carries, and every other refusal
    /// would pay for it on the stack of the common path.
    TransitiveLinkStated {
        /// The statement that would survive the move.
        statement: Box<Statement>,
    },
    /// The downward walk hit its bound, so the absence of a cycle could not be established.
    SubtreeTooLarge {
        /// The concept.
        concept: Node,
        /// How many concepts below it the walk reached before stopping.
        reached: usize,
    },
}

impl fmt::Display for RelocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RelocationError::NoSuchConcept { concept } => write!(
                f,
                "the vocabulary says nothing about {concept}, so there is nothing to move"
            ),
            RelocationError::NotAConcept { resource, role } => write!(
                f,
                "{resource} is not a skos:Concept, and the {role} of a move is a concept: \
                 skos:broader relates concepts, and a collection or a scheme in either position \
                 would state something SKOS does not have"
            ),
            RelocationError::NoSuchParent { parent } => write!(
                f,
                "the vocabulary says nothing about {parent}, so nothing can be moved under it; a \
                 move re-parents within one vocabulary and does not create the parent"
            ),
            RelocationError::IntoItself { concept } => write!(
                f,
                "{concept} cannot be moved under itself: SKOS §8.6.8 says a cyclic hierarchy is \
                 consistent, so nothing downstream would report this — it would simply be a \
                 concept with no route to a root"
            ),
            RelocationError::AlreadyThere { concept, parent } => write!(
                f,
                "{parent} is already a broader concept of {concept}, so this move would change \
                 nothing"
            ),
            RelocationError::NoBroaderConcept { concept } => write!(
                f,
                "{concept} has no broader concept, so there is nothing to move it out from under. \
                 Giving a concept its first parent is a different change from moving it: a top \
                 concept that gains one should stop being a top concept, and a move does not do \
                 that. Propose it with `openbiz import` until that operation exists"
            ),
            RelocationError::AmbiguousParent { concept, parents } => write!(
                f,
                "{concept} has {} broader concepts and a move replaces exactly one of them, so \
                 name the one to replace with --from: {}",
                parents.len(),
                list(parents)
            ),
            RelocationError::NotAParent {
                concept,
                named,
                parents,
            } => write!(
                f,
                "{named} is not a broader concept of {concept}, so there is no link to move; it \
                 has {}",
                list(parents)
            ),
            RelocationError::IntoItsOwnDescendant {
                concept,
                parent,
                route,
            } => write!(
                f,
                "{parent} is below {concept} already, so moving {concept} under it would make a \
                 cycle with no route to a root: {}",
                list(route)
            ),
            RelocationError::TransitiveLinkStated { statement } => write!(
                f,
                "the vocabulary states {statement} directly, and a move does not remove it, so \
                 the concept would still be under its old parent by S24 while every report said \
                 it had moved. Retract that statement first if it is stale"
            ),
            RelocationError::SubtreeTooLarge { concept, reached } => write!(
                f,
                "the walk below {concept} reached {reached} concepts and stopped at its bound, so \
                 whether the new parent is below it could not be established; a move that skipped \
                 that check would be reporting a check it did not run"
            ),
        }
    }
}

impl std::error::Error for RelocationError {}

/// Nodes as a readable list, which is what every one of these refusals ends with.
fn list(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|node| node.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

impl CoreModel {
    /// The statements that would move `concept` out from under one broader concept and under `to`.
    ///
    /// `from` names which broader concept is being left, and may be omitted when the concept has
    /// exactly one. Nothing is written: the answer is a [`Relocation`] holding two statement lists
    /// for a caller to stage as a candidate.
    ///
    /// Read the module note for what is refused and why. The short version is that a wrong move
    /// leaves a perfectly consistent vocabulary, so the checks here are the only ones there are.
    pub fn relocate(
        &self,
        concept: &Node,
        to: &Node,
        from: Option<&Node>,
        bound: WalkBound,
    ) -> Result<Relocation, RelocationError> {
        let Some(resource) = self.resource(concept) else {
            return Err(RelocationError::NoSuchConcept {
                concept: concept.clone(),
            });
        };
        if !resource.is_a(SkosClass::Concept) {
            return Err(RelocationError::NotAConcept {
                resource: concept.clone(),
                role: "subject",
            });
        }
        let Some(new_parent) = self.resource(to) else {
            return Err(RelocationError::NoSuchParent { parent: to.clone() });
        };
        if !new_parent.is_a(SkosClass::Concept) {
            return Err(RelocationError::NotAConcept {
                resource: to.clone(),
                role: "new broader concept",
            });
        }
        if concept == to {
            return Err(RelocationError::IntoItself {
                concept: concept.clone(),
            });
        }

        // Every broader concept the graph gives it, whichever direction each was stated in: the
        // model has already turned `<parent> skos:narrower <concept>` round under S25, so one map
        // holds both and a polyhierarchy authored in either direction reads the same here.
        let parents: Vec<Node> = resource
            .relations(SemanticRelation::Broader)
            .map(|links| links.keys().cloned().collect())
            .unwrap_or_default();
        if parents.iter().any(|parent| parent == to) {
            return Err(RelocationError::AlreadyThere {
                concept: concept.clone(),
                parent: to.clone(),
            });
        }

        let old = match from {
            Some(named) if parents.iter().any(|parent| parent == named) => named.clone(),
            Some(named) => {
                return Err(RelocationError::NotAParent {
                    concept: concept.clone(),
                    named: named.clone(),
                    parents,
                })
            }
            None => match parents.len() {
                0 => {
                    return Err(RelocationError::NoBroaderConcept {
                        concept: concept.clone(),
                    })
                }
                1 => parents[0].clone(),
                _ => {
                    return Err(RelocationError::AmbiguousParent {
                        concept: concept.clone(),
                        parents,
                    })
                }
            },
        };

        // A transitive link stated directly is the one thing a move cannot express its way out of.
        // Both directions are asked, and the first one found is the refusal: a graph carrying both
        // is refused by either, and naming one statement is enough to send the operator to it.
        if let Some(statement) = [
            self.stated_directly(concept, SemanticRelation::BroaderTransitive, &old),
            self.stated_directly(&old, SemanticRelation::NarrowerTransitive, concept),
        ]
        .into_iter()
        .flatten()
        .next()
        {
            return Err(RelocationError::TransitiveLinkStated {
                statement: Box::new(statement),
            });
        }

        // The cycle check and the subtree count are the same walk, so a report that names how many
        // concepts move cannot disagree with the check that let the move happen.
        let below = self.descent(concept, bound);
        if !below.is_complete() {
            return Err(RelocationError::SubtreeTooLarge {
                concept: concept.clone(),
                reached: below.len(),
            });
        }
        if below.contains(to) {
            return Err(RelocationError::IntoItsOwnDescendant {
                concept: concept.clone(),
                parent: to.clone(),
                route: below.path_to(to).unwrap_or_else(|| vec![concept.clone()]),
            });
        }

        let stated = self.stated_directions(concept, &old);
        let mut removals = Vec::new();
        let mut additions = Vec::new();
        if stated.broader {
            removals.push(broader(concept, &old));
            additions.push(broader(concept, to));
        }
        if stated.narrower {
            removals.push(narrower(&old, concept));
            additions.push(narrower(to, concept));
        }

        Ok(Relocation {
            concept: concept.clone(),
            from: old,
            to: to.clone(),
            removals,
            additions,
            moved_with_it: below.len(),
            subtree_complete: below.is_complete(),
            top_concept_of: resource.top_concept_of().iter().cloned().collect(),
        })
    }

    /// The statement `<subject> <relation> <object>`, if the graph carries it *as written*.
    ///
    /// [`RelationOrigin::Asserted`] is the whole point: an entailed link is not a statement in the
    /// graph, so proposing to remove it would name something that is not there.
    fn stated_directly(
        &self,
        subject: &Node,
        relation: SemanticRelation,
        object: &Node,
    ) -> Option<Statement> {
        match self.resource(subject)?.relations(relation)?.get(object)? {
            RelationOrigin::Asserted => Some(Statement::new(
                subject.clone(),
                relation.iri(),
                object.clone(),
            )),
            RelationOrigin::Entailed(_) => None,
        }
    }

    /// Which of `skos:broader` and `skos:narrower` the graph states between a concept and a parent.
    fn stated_directions(&self, concept: &Node, parent: &Node) -> StatedDirections {
        StatedDirections {
            broader: self
                .stated_directly(concept, SemanticRelation::Broader, parent)
                .is_some(),
            narrower: self
                .stated_directly(parent, SemanticRelation::Narrower, concept)
                .is_some(),
        }
    }
}

/// `<concept> skos:broader <parent>`.
fn broader(concept: &Node, parent: &Node) -> Statement {
    Statement::new(
        concept.clone(),
        SemanticRelation::Broader.iri(),
        parent.clone(),
    )
}

/// `<parent> skos:narrower <concept>`.
fn narrower(parent: &Node, concept: &Node) -> Statement {
    Statement::new(
        parent.clone(),
        SemanticRelation::Narrower.iri(),
        concept.clone(),
    )
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

    /// `<name> a skos:Concept`, so the model types the resource without a hierarchy link doing it.
    fn concept(name: &Node) -> Statement {
        Statement::new(
            name.clone(),
            format!("{}type", ns::RDF),
            Node::iri(format!("{}Concept", ns::SKOS)),
        )
    }

    fn rendered(statements: &[Statement]) -> Vec<String> {
        statements.iter().map(Statement::to_string).collect()
    }

    /// The ordinary case, and the shape of the whole operation: one link out, one link in.
    #[test]
    fn a_move_replaces_the_one_link_and_leaves_the_subtree_alone() {
        let (world, emea, apac, france, paris) = (
            ex("world"),
            ex("emea"),
            ex("apac"),
            ex("france"),
            ex("paris"),
        );
        let model = CoreModel::from_statements([
            concept(&world),
            s(&emea, &skos("broader"), &world),
            s(&apac, &skos("broader"), &world),
            s(&france, &skos("broader"), &emea),
            s(&paris, &skos("broader"), &france),
        ]);

        let moved = model
            .relocate(&france, &apac, None, WalkBound::DEFAULT)
            .expect("france has exactly one broader concept");

        assert_eq!(moved.concept(), &france);
        assert_eq!(moved.from(), &emea);
        assert_eq!(moved.to(), &apac);
        assert_eq!(
            rendered(moved.removals()),
            vec![format!("{france} skos:broader {emea}")]
        );
        assert_eq!(
            rendered(moved.additions()),
            vec![format!("{france} skos:broader {apac}")]
        );

        // The one number a reviewer needs that the diff does not show.
        assert_eq!(moved.moved_with_it(), 1, "paris moves with it");
        assert!(moved.subtree_complete());
        assert!(moved.top_concept_of().is_empty());
    }

    /// S25 lets a vocabulary state the hierarchy either way round, and a move must not convert it.
    #[test]
    fn a_link_stated_only_as_narrower_is_replaced_as_narrower() {
        let (a, b, c) = (ex("a"), ex("b"), ex("c"));
        let model =
            CoreModel::from_statements([concept(&b), concept(&c), s(&a, &skos("narrower"), &c)]);

        let moved = model
            .relocate(&c, &b, None, WalkBound::DEFAULT)
            .expect("the model turned the narrower link round, so c has a broader concept");

        assert_eq!(moved.from(), &a);
        assert_eq!(
            rendered(moved.removals()),
            vec![format!("{a} skos:narrower {c}")],
            "removing <c> skos:broader <a> would name a statement the graph does not carry"
        );
        assert_eq!(
            rendered(moved.additions()),
            vec![format!("{b} skos:narrower {c}")],
            "and the vocabulary stays authored the way it was written"
        );
    }

    /// Both directions stated is both directions moved — and *only* those two.
    #[test]
    fn a_link_stated_both_ways_round_is_replaced_both_ways_round() {
        let (p, q, b) = (ex("p"), ex("q"), ex("b"));
        let model = CoreModel::from_statements([
            concept(&b),
            s(&q, &skos("broader"), &p),
            s(&p, &skos("narrower"), &q),
        ]);

        let moved = model
            .relocate(&q, &b, None, WalkBound::DEFAULT)
            .expect("one broader concept, stated twice");

        assert_eq!(
            rendered(moved.removals()),
            vec![
                format!("{q} skos:broader {p}"),
                format!("{p} skos:narrower {q}"),
            ]
        );
        assert_eq!(
            rendered(moved.additions()),
            vec![
                format!("{q} skos:broader {b}"),
                format!("{b} skos:narrower {q}"),
            ]
        );
        assert_eq!(
            moved.additions().len(),
            moved.removals().len(),
            "a move replaces each stated direction with the same direction and invents none"
        );
    }

    /// The refusal that matters most: §8.6.8 makes a cycle *consistent*, so nothing else catches it.
    #[test]
    fn a_move_under_its_own_descendant_is_refused_with_the_route() {
        let (a, b, c) = (ex("a"), ex("b"), ex("c"));
        let model = CoreModel::from_statements([
            concept(&a),
            s(&b, &skos("broader"), &a),
            s(&c, &skos("broader"), &b),
        ]);

        let error = model
            .relocate(&b, &c, None, WalkBound::DEFAULT)
            .expect_err("c is below b");
        match error {
            RelocationError::IntoItsOwnDescendant { route, .. } => {
                assert_eq!(route, vec![b.clone(), c.clone()])
            }
            other => panic!("{other}"),
        }

        // And the degenerate one step of the same thing.
        assert!(matches!(
            model.relocate(&b, &b, None, WalkBound::DEFAULT),
            Err(RelocationError::IntoItself { .. })
        ));
    }

    /// A polyhierarchic concept has several parents and a move replaces one, so it must be named.
    #[test]
    fn a_concept_with_two_broader_concepts_needs_the_one_being_left_named() {
        let (a, b, c, r) = (ex("a"), ex("b"), ex("c"), ex("r"));
        let model = CoreModel::from_statements([
            concept(&c),
            s(&r, &skos("broader"), &a),
            s(&r, &skos("broader"), &b),
        ]);

        match model.relocate(&r, &c, None, WalkBound::DEFAULT) {
            Err(RelocationError::AmbiguousParent { parents, .. }) => {
                assert_eq!(parents, vec![a.clone(), b.clone()])
            }
            other => panic!("{other:?}"),
        }

        // Named, and the other parent is untouched — which is what "replaces exactly one" means.
        let moved = model
            .relocate(&r, &c, Some(&a), WalkBound::DEFAULT)
            .expect("--from names a real parent");
        assert_eq!(moved.from(), &a);
        assert_eq!(
            rendered(moved.removals()),
            vec![format!("{r} skos:broader {a}")]
        );

        // And a parent it does not have is refused rather than silently added.
        match model.relocate(&r, &c, Some(&c), WalkBound::DEFAULT) {
            Err(RelocationError::NotAParent { parents, named, .. }) => {
                assert_eq!(named, c);
                assert_eq!(parents, vec![a.clone(), b.clone()]);
            }
            other => panic!("{other:?}"),
        }
    }

    /// A statement a move does not remove would leave the concept under its old parent by S24.
    #[test]
    fn a_directly_stated_transitive_link_to_the_old_parent_is_refused() {
        let (a, b, s_) = (ex("a"), ex("b"), ex("s"));
        let model = CoreModel::from_statements([
            concept(&b),
            s(&s_, &skos("broader"), &a),
            s(&s_, &skos("broaderTransitive"), &a),
        ]);

        match model.relocate(&s_, &b, None, WalkBound::DEFAULT) {
            Err(RelocationError::TransitiveLinkStated { statement }) => assert_eq!(
                statement.to_string(),
                format!("{s_} skos:broaderTransitive {a}")
            ),
            other => panic!("{other:?}"),
        }

        // The inverse direction of the same objection: `<a> skos:narrowerTransitive <s>`.
        let model = CoreModel::from_statements([
            concept(&b),
            s(&s_, &skos("broader"), &a),
            s(&a, &skos("narrowerTransitive"), &s_),
        ]);
        match model.relocate(&s_, &b, None, WalkBound::DEFAULT) {
            Err(RelocationError::TransitiveLinkStated { statement }) => assert_eq!(
                statement.to_string(),
                format!("{a} skos:narrowerTransitive {s_}")
            ),
            other => panic!("{other:?}"),
        }

        // But the *entailed* transitive link S22 lifts from `skos:broader` is not a statement in
        // the graph and must not be refused, or every ordinary move would be.
        let model = CoreModel::from_statements([concept(&b), s(&s_, &skos("broader"), &a)]);
        assert!(model.relocate(&s_, &b, None, WalkBound::DEFAULT).is_ok());
    }

    /// Giving a concept its first parent is a different change, and it is not this one.
    #[test]
    fn a_concept_with_no_broader_concept_is_refused_rather_than_given_one() {
        let (a, b) = (ex("a"), ex("b"));
        let model = CoreModel::from_statements([concept(&a), concept(&b)]);

        assert!(matches!(
            model.relocate(&a, &b, None, WalkBound::DEFAULT),
            Err(RelocationError::NoBroaderConcept { .. })
        ));
    }

    /// The three ways a move can name something the vocabulary cannot move.
    #[test]
    fn a_resource_that_is_not_a_concept_in_this_vocabulary_is_refused() {
        let (a, b, collection) = (ex("a"), ex("b"), ex("collection"));
        let model = CoreModel::from_statements([
            concept(&b),
            s(&a, &skos("broader"), &b),
            Statement::new(
                collection.clone(),
                format!("{}type", ns::RDF),
                Node::iri(format!("{}Collection", ns::SKOS)),
            ),
        ]);

        assert!(matches!(
            model.relocate(&ex("absent"), &b, None, WalkBound::DEFAULT),
            Err(RelocationError::NoSuchConcept { .. })
        ));
        assert!(matches!(
            model.relocate(&a, &ex("absent"), None, WalkBound::DEFAULT),
            Err(RelocationError::NoSuchParent { .. })
        ));
        assert!(
            matches!(
                model.relocate(&collection, &b, None, WalkBound::DEFAULT),
                Err(RelocationError::NotAConcept { .. })
            ),
            "S28 makes skos:Collection disjoint from skos:Concept, and skos:broader relates \
             concepts"
        );
        assert!(matches!(
            model.relocate(&a, &collection, None, WalkBound::DEFAULT),
            Err(RelocationError::NotAConcept { .. })
        ));
    }

    /// A move that would change nothing is told so rather than staged as an empty proposal.
    #[test]
    fn a_move_under_a_broader_concept_it_already_has_is_refused() {
        let (a, b, r) = (ex("a"), ex("b"), ex("r"));
        let model =
            CoreModel::from_statements([s(&r, &skos("broader"), &a), s(&r, &skos("broader"), &b)]);

        assert!(matches!(
            model.relocate(&r, &b, Some(&a), WalkBound::DEFAULT),
            Err(RelocationError::AlreadyThere { .. })
        ));
    }

    /// An incomplete walk cannot prove there is no cycle, and saying so is the only honest answer.
    #[test]
    fn a_subtree_too_large_to_walk_is_refused_rather_than_moved_unchecked() {
        let (root, a, b, c, d) = (ex("root"), ex("a"), ex("b"), ex("c"), ex("d"));
        let model = CoreModel::from_statements([
            concept(&d),
            s(&a, &skos("broader"), &root),
            s(&b, &skos("broader"), &a),
            s(&c, &skos("broader"), &b),
        ]);

        // Two concepts below `a`, and a walk allowed to reach one of them.
        match model.relocate(&a, &d, None, WalkBound::new(1, 100)) {
            Err(RelocationError::SubtreeTooLarge { .. }) => {}
            other => panic!("{other:?}"),
        }
    }

    /// Reported, not corrected, and not a refusal: the state predates the move.
    #[test]
    fn a_top_concept_being_moved_is_reported_and_not_refused() {
        let (scheme, a, b, t) = (ex("scheme"), ex("a"), ex("b"), ex("t"));
        let model = CoreModel::from_statements([
            concept(&b),
            s(&t, &skos("broader"), &a),
            s(&t, &skos("topConceptOf"), &scheme),
        ]);

        let moved = model
            .relocate(&t, &b, None, WalkBound::DEFAULT)
            .expect("odd is not the same as refusable");
        assert_eq!(moved.top_concept_of(), std::slice::from_ref(&scheme));
    }
}
