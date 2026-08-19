//! Merging two concepts into one: the statements that make every reference to one of them a
//! reference to the other.
//!
//! The second of `docs/BUILD-PLAN.md`'s bulk operations, and the second producer of a candidate
//! carrying **both** halves of a change (`CLAUDE.md` §3). Like [`relocate`](crate::relocate),
//! nothing here writes: a [`Merge`] is an *answer* — the statements a merge would remove and the
//! statements it would add — computed against a [`CoreModel`] and a [`MergeScan`] read a moment
//! ago. The caller stages them as a candidate; a human approves them.
//!
//! # Why a merge needs the graph and not only the model
//!
//! A move touches two statements and both of them are `skos:broader` or `skos:narrower`, so the
//! interpreted model holds everything it needs. A merge is the opposite: its promise is that
//! **every** reference to the merged concept is repointed, and the model is an interpretation of a
//! vocabulary rather than a copy of it. `<X> ex:approvedBy <A>` is a reference SKOS has no opinion
//! about, and a merge that repointed only the statements the model recognised would leave the
//! vocabulary pointing at a concept that no longer exists — silently, because nothing downstream
//! reads `ex:approvedBy` either.
//!
//! So the caller streams the *raw* graph past a [`MergeScan`], which keeps two things and nothing
//! else: the statements that mention the concept being merged away, and the statements that
//! mention the one that survives. The first is what gets rewritten; the second is what tells the
//! rewrite whether the vocabulary already says it. Peak memory is the degree of two concepts, not
//! the graph — which is why this is a scan rather than "read the whole vocabulary".
//!
//! # What happens to each reference
//!
//! Every statement mentioning the source is removed, and its rewrite — the same statement with the
//! source replaced by the target, in either position — is added, **except** in three cases:
//!
//! - **It would link the target to itself.** `<A> skos:broader <B>` merged into `<B>` rewrites to
//!   `<B> skos:broader <B>`, which §8.6.7's Example 36 marks *consistent* and which is a concept
//!   with no route to a root. Dropped, and reported: absorbing a concept into its own parent is an
//!   ordinary merge and the reviewer should see that the link between them is what went.
//! - **The vocabulary already says it.** Two siblings under one parent both state
//!   `skos:broader <parent>`; after the merge that is one statement, not two.
//! - **It is a label the target already carries**, under any of the three kinds. Adding it again
//!   under a different kind would violate S13, which forbids a resource carrying the same literal
//!   as two different kinds of label.
//!
//! # Preferred labels, and the one place this operation makes a choice for you
//!
//! S14 allows a resource at most one preferred label per language tag. Two concepts being merged
//! almost always both have one in the same language — that is frequently *why* they are being
//! merged — so a merge that repointed both would produce a vocabulary that is not SKOS.
//!
//! The source's preferred label becomes an **alternative** label on the target. Nothing is lost,
//! S14 holds, and the target keeps the name it was already known by. The alternative — refusing
//! the merge and making the operator retract a label first — would refuse nearly every real merge,
//! and the alternative to *that* — dropping the label — loses the search term that made the
//! duplicate findable in the first place. Every demotion is named in the report, because this is
//! the one thing here the operator did not ask for.
//!
//! # What it refuses
//!
//! - **Merging a concept into itself**, which is not a change.
//! - **Either side not being a `skos:Concept`** the vocabulary knows about. Merging a collection
//!   into a concept would produce statements SKOS has no reading of.
//! - **A merge that would make a cycle.** Identifying two concepts turns any hierarchy path
//!   between them of length two or more into a cycle: with `<A> broader <X> broader <B>`, merging
//!   `A` into `B` leaves `B broader X` and `X broader B`. §8.6.8 calls that consistent, so nothing
//!   downstream would report it, and the result is a branch with no route to a root. The check is
//!   an *upward* walk from each parent, never a downward one: everything below a concept is most
//!   of the vocabulary, and the direction that answers the question cheaply is the one that asks
//!   "is the survivor above this parent?" rather than "is this parent below the survivor?".
//! - **A walk or a scan that hit its bound**, because an incomplete answer cannot establish an
//!   absence, and "no cycle found within the budget" is not "no cycle".

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::hierarchy::WalkBound;
use crate::labels::{LabelKind, LexicalLabel};
use crate::model::{CoreModel, Node, SkosClass, Statement, Term};
use crate::relations::SemanticRelation;

/// How many statements about the two concepts a [`MergeScan`] will hold before it gives up.
///
/// A merge is bounded like every other enumeration in this crate, and for the same reason: the
/// input is a customer's vocabulary and nothing in RDF stops one concept being the object of a
/// million statements. What is different here is that the bound is on *retained* statements rather
/// than on a walk, so hitting it means the scan cannot answer, not that it answered partially.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceBound {
    /// The most statements about either concept that will be kept.
    pub max_statements: usize,
}

impl ReferenceBound {
    /// The bound every caller in this build uses unless it says otherwise.
    ///
    /// 100 000 statements about one concept. **This is a judgement measured against nothing**, and
    /// it is recorded as such in `docs/UNTESTED.md` alongside the four constants before it. The
    /// reasoning: a concept with a hundred thousand statements about it is a hub, and the concept
    /// most likely to be one is a top concept in a large polyhierarchy — which is also the concept
    /// least likely to be merged into anything. A leaf duplicate, which is what merges are
    /// actually for, has a handful.
    pub const DEFAULT: ReferenceBound = ReferenceBound {
        max_statements: 100_000,
    };
}

impl Default for ReferenceBound {
    fn default() -> Self {
        ReferenceBound::DEFAULT
    }
}

/// Every statement in a vocabulary that mentions either of two concepts.
///
/// Built by streaming the whole graph past [`MergeScanBuilder::push`]; see the module note for why
/// a merge needs the raw statements and not the interpreted model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeScan {
    source: Node,
    target: Node,
    about_source: BTreeSet<Statement>,
    about_target: BTreeSet<Statement>,
    complete: bool,
}

impl MergeScan {
    /// Start a scan for the concept being merged away and the one that survives.
    pub fn builder(source: Node, target: Node) -> MergeScanBuilder {
        MergeScanBuilder {
            scan: MergeScan {
                source,
                target,
                about_source: BTreeSet::new(),
                about_target: BTreeSet::new(),
                complete: true,
            },
            bound: ReferenceBound::DEFAULT,
        }
    }

    /// The concept being merged away.
    pub fn source(&self) -> &Node {
        &self.source
    }

    /// The concept that survives.
    pub fn target(&self) -> &Node {
        &self.target
    }

    /// How many statements mention the concept being merged away.
    ///
    /// A lower bound when [`MergeScan::is_complete`] is `false`.
    pub fn references(&self) -> usize {
        self.about_source.len()
    }

    /// Whether the scan kept everything it saw rather than stopping at its bound.
    pub fn is_complete(&self) -> bool {
        self.complete
    }
}

/// Collects the statements a merge needs while the graph streams past.
#[derive(Debug, Clone)]
pub struct MergeScanBuilder {
    scan: MergeScan,
    bound: ReferenceBound,
}

impl MergeScanBuilder {
    /// Use a different bound. See [`ReferenceBound::DEFAULT`] for what the standing one is.
    pub fn with_bound(mut self, bound: ReferenceBound) -> Self {
        self.bound = bound;
        self
    }

    /// Offer one statement of the vocabulary. Statements mentioning neither concept are dropped.
    pub fn push(&mut self, statement: Statement) {
        let mentions_source = mentions(&statement, &self.scan.source);
        let mentions_target = mentions(&statement, &self.scan.target);
        if !mentions_source && !mentions_target {
            return;
        }
        // A bound that truncated would make an absence unreadable: "the vocabulary does not
        // already say this" is exactly the question the target's statements answer, and a
        // half-kept set answers it wrongly rather than not at all.
        if self
            .scan
            .about_source
            .len()
            .max(self.scan.about_target.len())
            >= self.bound.max_statements
        {
            self.scan.complete = false;
            return;
        }
        if mentions_source {
            self.scan.about_source.insert(statement.clone());
        }
        if mentions_target {
            self.scan.about_target.insert(statement);
        }
    }

    /// The finished scan.
    pub fn build(self) -> MergeScan {
        self.scan
    }
}

/// Whether a statement names `node` in subject or object position.
fn mentions(statement: &Statement, node: &Node) -> bool {
    &statement.subject == node || statement.object.as_node() == Some(node)
}

/// A preferred label that a merge turns into an alternative one, and the label it yields to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Demotion {
    /// The source's preferred label, which becomes a `skos:altLabel` on the target.
    pub label: LexicalLabel,
    /// The target's preferred label in the same language, which is why S14 leaves room for one.
    pub in_favour_of: LexicalLabel,
}

/// What merging one concept into another would change.
///
/// Produced by [`CoreModel::merge`] and applied by nobody: the statements are a proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merge {
    source: Node,
    target: Node,
    removals: Vec<Statement>,
    additions: Vec<Statement>,
    self_links: Vec<Statement>,
    already_said: Vec<Statement>,
    demotions: Vec<Demotion>,
    subjects: usize,
    objects: usize,
}

impl Merge {
    /// The concept being merged away. After the change, nothing in the vocabulary mentions it.
    pub fn source(&self) -> &Node {
        &self.source
    }

    /// The concept that survives.
    pub fn target(&self) -> &Node {
        &self.target
    }

    /// The statements the merge takes out of the vocabulary: every one that mentions the source.
    ///
    /// Never empty — a concept the vocabulary knows about is the subject of at least the statement
    /// that typed it, or of the hierarchy link that placed it.
    pub fn removals(&self) -> &[Statement] {
        &self.removals
    }

    /// The statements the merge puts into the vocabulary.
    ///
    /// May be **empty**, and that is the ordinary result of merging a concept whose every
    /// statement the target already carries — a duplicate created by two imports of one thesaurus.
    pub fn additions(&self) -> &[Statement] {
        &self.additions
    }

    /// The rewritten statements dropped because they would link the target to itself.
    ///
    /// Non-empty exactly when the two concepts were hierarchically or associatively linked. The
    /// statements here are the *rewrites*, so they read as what would have been written.
    pub fn self_links(&self) -> &[Statement] {
        &self.self_links
    }

    /// The rewritten statements dropped because the vocabulary already carries them.
    pub fn already_said(&self) -> &[Statement] {
        &self.already_said
    }

    /// The preferred labels this merge turns into alternative ones, to keep S14.
    pub fn demotions(&self) -> &[Demotion] {
        &self.demotions
    }

    /// How many removed statements have the source as their subject.
    pub fn subjects(&self) -> usize {
        self.subjects
    }

    /// How many removed statements point *at* the source. These are the references a merge exists
    /// to repoint, and the ones an operator deleting a concept by hand would leave dangling.
    pub fn objects(&self) -> usize {
        self.objects
    }
}

/// Why a merge was refused. Every one of these is a question the operator has to answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeError {
    /// The vocabulary says nothing about one of the two concepts.
    NoSuchConcept {
        /// The IRI that was asked for.
        concept: Node,
        /// Which side of the merge named it.
        role: &'static str,
    },
    /// The vocabulary knows the resource, and it is not a `skos:Concept`.
    NotAConcept {
        /// The resource that was named.
        resource: Node,
        /// Which side of the merge named it.
        role: &'static str,
    },
    /// The two IRIs are the same.
    IntoItself {
        /// The concept.
        concept: Node,
    },
    /// The scan hit its bound, so what mentions the concepts is not fully known.
    TooManyReferences {
        /// How many statements about one concept were kept before it stopped.
        reached: usize,
    },
    /// Identifying the two concepts would close a hierarchy cycle.
    WouldCycle {
        /// The concept being merged away.
        source: Node,
        /// The concept that survives.
        target: Node,
        /// The route that becomes a cycle, source first and target last.
        route: Vec<Node>,
    },
    /// An upward walk hit its bound, so the absence of a cycle could not be established.
    HierarchyTooLarge {
        /// The concept the walk started from.
        concept: Node,
        /// How many ancestors it reached before stopping.
        reached: usize,
    },
}

impl fmt::Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeError::NoSuchConcept { concept, role } => write!(
                f,
                "the vocabulary says nothing about {concept}, and it is the {role} of this merge; \
                 a merge joins two concepts that are both already in it"
            ),
            MergeError::NotAConcept { resource, role } => write!(
                f,
                "{resource} is not a skos:Concept, and the {role} of a merge is a concept: \
                 repointing a collection's or a scheme's references onto a concept would state \
                 things SKOS has no reading of"
            ),
            MergeError::IntoItself { concept } => write!(
                f,
                "{concept} is both sides of this merge, so there is nothing to merge; name the \
                 concept to keep second and the duplicate first"
            ),
            MergeError::TooManyReferences { reached } => write!(
                f,
                "one of these concepts is mentioned by more than {reached} statements and the scan \
                 stopped there, so what points at it is not fully known; a merge that repointed \
                 part of it would leave the rest pointing at a concept that no longer exists"
            ),
            MergeError::WouldCycle {
                source,
                target,
                route,
            } => write!(
                f,
                "merging {source} into {target} would make a cycle, because the hierarchy already \
                 runs between them through {}: {}. SKOS §8.6.8 calls a cycle consistent, so \
                 nothing downstream would report it — it would simply be a branch with no route \
                 to a root",
                (route.len() - 2),
                list(route)
            ),
            MergeError::HierarchyTooLarge { concept, reached } => write!(
                f,
                "the walk above {concept} reached {reached} concepts and stopped at its bound, so \
                 whether this merge would close a cycle could not be established; a merge that \
                 skipped that check would be reporting a check it did not run"
            ),
        }
    }
}

impl std::error::Error for MergeError {}

/// Nodes as a readable list, which is what the cycle refusal ends with.
fn list(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(|node| node.to_string())
        .collect::<Vec<_>>()
        .join(" -> ")
}

impl CoreModel {
    /// The statements that would merge one concept into another, every reference repointed.
    ///
    /// `scan` supplies the raw statements — see the module note for why the interpreted model is
    /// not enough — and this supplies the SKOS reading of them: what a label collision means, what
    /// a link between the two concepts becomes, and whether the result would still be a hierarchy.
    ///
    /// Nothing is written. The answer is a [`Merge`] holding two statement lists for a caller to
    /// stage as a candidate.
    pub fn merge(&self, scan: &MergeScan, bound: WalkBound) -> Result<Merge, MergeError> {
        let (source, target) = (&scan.source, &scan.target);
        if source == target {
            return Err(MergeError::IntoItself {
                concept: source.clone(),
            });
        }
        for (node, role) in [(source, "duplicate"), (target, "surviving concept")] {
            let Some(resource) = self.resource(node) else {
                return Err(MergeError::NoSuchConcept {
                    concept: node.clone(),
                    role,
                });
            };
            if !resource.is_a(SkosClass::Concept) {
                return Err(MergeError::NotAConcept {
                    resource: node.clone(),
                    role,
                });
            }
        }
        if !scan.is_complete() {
            return Err(MergeError::TooManyReferences {
                reached: scan.about_source.len().max(scan.about_target.len()),
            });
        }

        self.refuse_a_cycle(source, target, bound)?;

        let mut removals: Vec<Statement> = scan.about_source.iter().cloned().collect();
        removals.sort();
        let (subjects, objects) = counts(&removals, source);

        let mut additions = Vec::new();
        let mut self_links = Vec::new();
        let mut already_said = Vec::new();
        let mut demotions = Vec::new();
        // Labels the additions have already committed to putting on the target, so that two
        // preferred labels in one language on the *source* — itself an S14 violation, which a
        // merge neither creates nor is required to repair — cannot both arrive as preferred ones.
        let mut arriving: BTreeMap<LexicalLabel, LabelKind> = BTreeMap::new();

        for statement in &removals {
            let rewritten = rewrite(statement, source, target);
            if rewritten.subject == *target && rewritten.object.as_node() == Some(target) {
                self_links.push(rewritten);
                continue;
            }
            if scan.about_target.contains(&rewritten) {
                already_said.push(rewritten);
                continue;
            }
            match self.reconcile(&rewritten, target, &mut arriving) {
                Reconciled::Add(statement) => additions.push(statement),
                Reconciled::Demote(statement, demotion) => {
                    demotions.push(demotion);
                    additions.push(statement);
                }
                Reconciled::AlreadyCarried => already_said.push(rewritten),
            }
        }
        additions.sort();
        additions.dedup();
        self_links.sort();
        already_said.sort();
        already_said.dedup();

        Ok(Merge {
            source: source.clone(),
            target: target.clone(),
            removals,
            additions,
            self_links,
            already_said,
            demotions,
            subjects,
            objects,
        })
    }

    /// Refuse a merge that would close a hierarchy cycle.
    ///
    /// Identifying two concepts turns every path between them of length two or more into a cycle;
    /// a path of length one becomes a self-link, which the merge drops. So the question is whether
    /// some parent of one concept — other than the other concept itself — has the other concept
    /// above it. That is an **upward** walk from each parent, which is the cheap direction: see
    /// the module note, and `docs/UNTESTED.md` on what a downward walk costs on a real thesaurus.
    fn refuse_a_cycle(
        &self,
        source: &Node,
        target: &Node,
        bound: WalkBound,
    ) -> Result<(), MergeError> {
        for (below, above) in [(source, target), (target, source)] {
            for parent in self.broader_concepts(below) {
                if &parent == above {
                    continue;
                }
                let ancestry = self.ancestry(&parent, bound);
                if !ancestry.is_complete() {
                    return Err(MergeError::HierarchyTooLarge {
                        concept: parent,
                        reached: ancestry.len(),
                    });
                }
                if let Some(path) = ancestry.path_to(above) {
                    let mut route = vec![below.clone()];
                    route.extend(path);
                    // Always reported source-first, whichever side of the merge found it: the
                    // operator asked to merge source into target and the route reads as an answer
                    // to that, not to its mirror.
                    if below != source {
                        route.reverse();
                    }
                    return Err(MergeError::WouldCycle {
                        source: source.clone(),
                        target: target.clone(),
                        route,
                    });
                }
            }
        }
        Ok(())
    }

    /// Every concept the graph places directly above `concept`, in a stable order.
    fn broader_concepts(&self, concept: &Node) -> Vec<Node> {
        self.resource(concept)
            .and_then(|resource| resource.relations(SemanticRelation::Broader))
            .map(|links| links.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// What to do with one rewritten statement whose subject is the surviving concept.
    ///
    /// Only labels need a decision; everything else is added as written. See the module note on
    /// preferred labels for why S14 forces one and what it is.
    fn reconcile(
        &self,
        rewritten: &Statement,
        target: &Node,
        arriving: &mut BTreeMap<LexicalLabel, LabelKind>,
    ) -> Reconciled {
        let Some(kind) = LabelKind::ALL
            .into_iter()
            .find(|kind| rewritten.predicate == kind.property_iri())
        else {
            return Reconciled::Add(rewritten.clone());
        };
        if rewritten.subject != *target {
            // A label *of something else* that happened to mention the source cannot arise —
            // a label's object is a literal — but a statement whose object is the target and
            // whose predicate is a label property is malformed rather than a label, and it is
            // repointed as written rather than reinterpreted.
            return Reconciled::Add(rewritten.clone());
        }
        let Some(label) = LexicalLabel::of(&rewritten.object) else {
            // S12's case: not a plain literal, so not a label SKOS recognises. Repointed as
            // written — the merge is not the place a malformed label gets repaired.
            return Reconciled::Add(rewritten.clone());
        };

        // S13: the same literal may not be two kinds of label on one resource. If the target
        // already carries it at all, the label is not lost by leaving it alone.
        let carried = self
            .resource(target)
            .map(|resource| resource.labels().contains_key(&label))
            .unwrap_or(false);
        if carried || arriving.contains_key(&label) {
            return Reconciled::AlreadyCarried;
        }

        if kind != LabelKind::Preferred {
            arriving.insert(label, kind);
            return Reconciled::Add(rewritten.clone());
        }

        // S14: at most one preferred label per language. `preferred_label_in` matches the tag
        // exactly and lower-cased, which is how the model groups labels, so this asks the same
        // question the integrity check asks.
        let held = match &label.language {
            Some(tag) => self
                .resource(target)
                .and_then(|resource| resource.preferred_label_in(tag))
                .cloned(),
            None => self
                .resource(target)
                .and_then(|resource| {
                    resource
                        .labels_of(LabelKind::Preferred)
                        .find(|held| held.language.is_none())
                })
                .cloned(),
        }
        .or_else(|| {
            arriving
                .iter()
                .find(|(held, kind)| {
                    **kind == LabelKind::Preferred && held.language == label.language
                })
                .map(|(held, _)| held.clone())
        });

        match held {
            Some(in_favour_of) => {
                arriving.insert(label.clone(), LabelKind::Alternative);
                Reconciled::Demote(
                    Statement::new(
                        rewritten.subject.clone(),
                        LabelKind::Alternative.property_iri(),
                        rewritten.object.clone(),
                    ),
                    Demotion {
                        label,
                        in_favour_of,
                    },
                )
            }
            None => {
                arriving.insert(label, LabelKind::Preferred);
                Reconciled::Add(rewritten.clone())
            }
        }
    }
}

/// What [`CoreModel::reconcile`] decided about one rewritten statement.
enum Reconciled {
    /// Add it as it stands.
    Add(Statement),
    /// Add it under a different label property, because S14 leaves room for only one.
    Demote(Statement, Demotion),
    /// Do not add it: the target already carries this label.
    AlreadyCarried,
}

/// The same statement with every mention of `source` replaced by `target`.
fn rewrite(statement: &Statement, source: &Node, target: &Node) -> Statement {
    Statement {
        subject: if statement.subject == *source {
            target.clone()
        } else {
            statement.subject.clone()
        },
        predicate: statement.predicate.clone(),
        object: match &statement.object {
            Term::Node(node) if node == source => Term::Node(target.clone()),
            other => other.clone(),
        },
    }
}

/// How many of the removed statements have the concept as subject, and how many point at it.
///
/// A statement doing both is counted in both, which is why the two do not add up to the total and
/// why the report prints them as two facts rather than as a split.
fn counts(removals: &[Statement], concept: &Node) -> (usize, usize) {
    let subjects = removals
        .iter()
        .filter(|statement| statement.subject == *concept)
        .count();
    let objects = removals
        .iter()
        .filter(|statement| statement.object.as_node() == Some(concept))
        .count();
    (subjects, objects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Literal;
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

    /// `<subject> <predicate> "text"@lang`.
    fn label(subject: &Node, predicate: &str, text: &str, language: &str) -> Statement {
        Statement::new(
            subject.clone(),
            predicate.to_owned(),
            Term::Literal(Literal {
                value: text.to_owned(),
                language: Some(language.to_owned()),
                datatype: crate::labels::RDF_LANG_STRING.to_owned(),
            }),
        )
    }

    fn concept(name: &Node) -> Statement {
        Statement::new(
            name.clone(),
            format!("{}type", ns::RDF),
            Node::iri(format!("{}Concept", ns::SKOS)),
        )
    }

    /// Build both the model and the scan from one list of statements, which is what the server
    /// does with two passes over the same store.
    fn merge_of(
        statements: &[Statement],
        source: &Node,
        target: &Node,
    ) -> Result<Merge, MergeError> {
        let model = CoreModel::from_statements(statements.iter().cloned());
        let mut scan = MergeScan::builder(source.clone(), target.clone());
        for statement in statements {
            scan.push(statement.clone());
        }
        model.merge(&scan.build(), WalkBound::DEFAULT)
    }

    fn rendered(statements: &[Statement]) -> Vec<String> {
        statements.iter().map(Statement::to_string).collect()
    }

    /// The ordinary case: two imports of one thesaurus produced two concepts for one thing, and
    /// something else already points at the one being merged away.
    #[test]
    fn every_reference_to_the_merged_concept_is_repointed() {
        let (animals, cats, felines, tabby) =
            (ex("animals"), ex("cats"), ex("felines"), ex("tabby"));
        let statements = [
            concept(&animals),
            concept(&cats),
            concept(&felines),
            concept(&tabby),
            s(&cats, &skos("broader"), &animals),
            s(&felines, &skos("broader"), &animals),
            s(&tabby, &skos("broader"), &felines),
            label(&cats, &skos("prefLabel"), "Cats", "en"),
            label(&felines, &skos("prefLabel"), "Felines", "en"),
            Statement::new(
                ex("policy"),
                "http://example.org/approvedBy".to_owned(),
                felines.clone(),
            ),
        ];

        let merge = merge_of(&statements, &felines, &cats).expect("an ordinary merge");

        // Every statement mentioning the duplicate goes, including the one SKOS has no reading of.
        assert_eq!(
            rendered(merge.removals()),
            vec![
                "<http://example.org/felines> rdf:type <http://www.w3.org/2004/02/skos/core#Concept>",
                "<http://example.org/felines> skos:broader <http://example.org/animals>",
                "<http://example.org/felines> skos:prefLabel \"Felines\"@en",
                "<http://example.org/policy> <http://example.org/approvedBy> <http://example.org/felines>",
                "<http://example.org/tabby> skos:broader <http://example.org/felines>",
            ]
        );
        // And arrives repointed, minus what the survivor already says.
        assert_eq!(
            rendered(merge.additions()),
            vec![
                "<http://example.org/cats> skos:altLabel \"Felines\"@en",
                "<http://example.org/policy> <http://example.org/approvedBy> <http://example.org/cats>",
                "<http://example.org/tabby> skos:broader <http://example.org/cats>",
            ]
        );
        assert_eq!(
            rendered(merge.already_said()),
            vec![
                "<http://example.org/cats> rdf:type <http://www.w3.org/2004/02/skos/core#Concept>",
                "<http://example.org/cats> skos:broader <http://example.org/animals>",
            ],
            "the type and the shared parent are already there, so they are not proposed twice"
        );
        assert_eq!(merge.subjects(), 3);
        assert_eq!(merge.objects(), 2);
    }

    /// S14 allows one preferred label per language, so the duplicate's becomes an alternative.
    /// Nothing is lost: the term that made the duplicate findable is still a search term.
    #[test]
    fn a_colliding_preferred_label_is_demoted_rather_than_dropped_or_refused() {
        let (cats, felines) = (ex("cats"), ex("felines"));
        let statements = [
            concept(&cats),
            concept(&felines),
            label(&cats, &skos("prefLabel"), "Cats", "en"),
            label(&felines, &skos("prefLabel"), "Felines", "en"),
            label(&felines, &skos("prefLabel"), "Chats", "fr"),
        ];

        let merge = merge_of(&statements, &felines, &cats).expect("a merge");

        assert_eq!(
            rendered(merge.additions()),
            vec![
                "<http://example.org/cats> skos:altLabel \"Felines\"@en",
                "<http://example.org/cats> skos:prefLabel \"Chats\"@fr",
            ],
            "French collides with nothing, so it stays preferred"
        );
        assert_eq!(merge.demotions().len(), 1);
        assert_eq!(merge.demotions()[0].label.text, "Felines");
        assert_eq!(merge.demotions()[0].in_favour_of.text, "Cats");
    }

    /// S13 forbids one literal being two kinds of label on one resource. A label the survivor
    /// already carries — under any kind — is left alone rather than added under another.
    #[test]
    fn a_label_the_survivor_already_carries_is_not_added_under_a_second_kind() {
        let (cats, felines) = (ex("cats"), ex("felines"));
        let statements = [
            concept(&cats),
            concept(&felines),
            label(&cats, &skos("prefLabel"), "Cats", "en"),
            label(&cats, &skos("altLabel"), "Felines", "en"),
            label(&felines, &skos("prefLabel"), "Felines", "en"),
        ];

        let merge = merge_of(&statements, &felines, &cats).expect("a merge");

        assert!(
            merge.additions().is_empty(),
            "nothing arrives: {:?}",
            rendered(merge.additions())
        );
        assert_eq!(
            rendered(merge.already_said()),
            vec![
                "<http://example.org/cats> rdf:type <http://www.w3.org/2004/02/skos/core#Concept>",
                "<http://example.org/cats> skos:prefLabel \"Felines\"@en",
            ],
            "the rewritten preferred label is reported as already carried, not demoted into an \
             altLabel the survivor already has"
        );
        assert!(merge.demotions().is_empty());
    }

    /// Absorbing a concept into its own parent is an ordinary merge, and the link between them is
    /// what goes. Adding it back would be `<B> skos:broader <B>`, which §8.6.7 calls consistent.
    #[test]
    fn a_link_between_the_two_concepts_becomes_a_self_link_and_is_dropped() {
        let (cats, tabby, spot) = (ex("cats"), ex("tabby"), ex("spot"));
        let statements = [
            concept(&cats),
            concept(&tabby),
            concept(&spot),
            s(&tabby, &skos("broader"), &cats),
            s(&spot, &skos("broader"), &tabby),
        ];

        let merge = merge_of(&statements, &tabby, &cats).expect("absorbing a child into a parent");

        assert_eq!(
            rendered(merge.self_links()),
            vec!["<http://example.org/cats> skos:broader <http://example.org/cats>"]
        );
        assert_eq!(
            rendered(merge.additions()),
            vec!["<http://example.org/spot> skos:broader <http://example.org/cats>"],
            "the grandchild becomes a child"
        );
    }

    /// The direction the graph stated the link in makes no difference: `skos:narrower` from the
    /// parent is the same self-link once the two are identified.
    #[test]
    fn a_narrower_link_between_the_two_is_a_self_link_too() {
        let (cats, tabby) = (ex("cats"), ex("tabby"));
        let statements = [
            concept(&cats),
            concept(&tabby),
            s(&cats, &skos("narrower"), &tabby),
        ];

        let merge = merge_of(&statements, &tabby, &cats).expect("a merge");

        assert_eq!(
            rendered(merge.self_links()),
            vec!["<http://example.org/cats> skos:narrower <http://example.org/cats>"]
        );
        assert!(merge.additions().is_empty());
    }

    /// Identifying two concepts with a two-step path between them closes a cycle. §8.6.8 calls a
    /// cycle consistent, so nothing downstream reports it, and the branch has no route to a root.
    #[test]
    fn a_merge_that_would_close_a_cycle_is_refused_and_names_the_route() {
        let (a, x, b) = (ex("a"), ex("x"), ex("b"));
        let statements = [
            concept(&a),
            concept(&x),
            concept(&b),
            s(&a, &skos("broader"), &x),
            s(&x, &skos("broader"), &b),
        ];

        let error = merge_of(&statements, &a, &b).expect_err("that closes a cycle");

        assert_eq!(
            error,
            MergeError::WouldCycle {
                source: a.clone(),
                target: b.clone(),
                route: vec![a, x, b],
            }
        );
        assert!(error.to_string().contains("would make a cycle"), "{error}");
    }

    /// The same path in the other direction, found by walking up from the survivor's parent. The
    /// route is still printed with the concept the operator named first.
    #[test]
    fn a_cycle_found_from_the_surviving_side_is_still_reported_source_first() {
        let (a, x, b) = (ex("a"), ex("x"), ex("b"));
        let statements = [
            concept(&a),
            concept(&x),
            concept(&b),
            s(&b, &skos("broader"), &x),
            s(&x, &skos("broader"), &a),
        ];

        let error = merge_of(&statements, &a, &b).expect_err("that closes a cycle");

        assert_eq!(
            error,
            MergeError::WouldCycle {
                source: a.clone(),
                target: b.clone(),
                route: vec![a, x, b],
            }
        );
    }

    /// A direct link is *not* a cycle: it becomes a self-link, which the merge drops. This is the
    /// boundary the cycle check has to get right, because refusing here would refuse the commonest
    /// merge there is.
    #[test]
    fn a_direct_link_between_the_two_is_not_treated_as_a_cycle() {
        let (child, parent) = (ex("child"), ex("parent"));
        let statements = [
            concept(&child),
            concept(&parent),
            s(&child, &skos("broader"), &parent),
        ];

        merge_of(&statements, &child, &parent).expect("absorbing a child into its parent");
        merge_of(&statements, &parent, &child).expect("and the same merge stated the other way");
    }

    /// A concept the vocabulary has never heard of is refused rather than merged into existence.
    #[test]
    fn an_unknown_concept_on_either_side_is_refused() {
        let cats = ex("cats");
        let statements = [concept(&cats)];

        assert_eq!(
            merge_of(&statements, &ex("ghost"), &cats).expect_err("no such duplicate"),
            MergeError::NoSuchConcept {
                concept: ex("ghost"),
                role: "duplicate",
            }
        );
        assert_eq!(
            merge_of(&statements, &cats, &ex("ghost")).expect_err("no such survivor"),
            MergeError::NoSuchConcept {
                concept: ex("ghost"),
                role: "surviving concept",
            }
        );
    }

    /// A collection is not a concept, and repointing its members onto a concept would state
    /// something SKOS has no reading of.
    #[test]
    fn a_resource_that_is_not_a_concept_is_refused_on_either_side() {
        let (cats, group) = (ex("cats"), ex("group"));
        let statements = [
            concept(&cats),
            Statement::new(
                group.clone(),
                format!("{}type", ns::RDF),
                Node::iri(format!("{}Collection", ns::SKOS)),
            ),
        ];

        assert_eq!(
            merge_of(&statements, &group, &cats).expect_err("a collection is not a concept"),
            MergeError::NotAConcept {
                resource: group.clone(),
                role: "duplicate",
            }
        );
        assert_eq!(
            merge_of(&statements, &cats, &group).expect_err("nor on the other side"),
            MergeError::NotAConcept {
                resource: group,
                role: "surviving concept",
            }
        );
    }

    /// Merging a concept into itself is not a change, and the message says which order to name
    /// them in — the mistake it most likely came from.
    #[test]
    fn merging_a_concept_into_itself_is_refused() {
        let cats = ex("cats");
        let error = merge_of(&[concept(&cats)], &cats, &cats).expect_err("not a change");
        assert_eq!(error, MergeError::IntoItself { concept: cats });
        assert!(error
            .to_string()
            .contains("name the concept to keep second"));
    }

    /// A scan that stopped at its bound cannot say what points at the concept, and a merge that
    /// repointed part of a vocabulary would leave the rest dangling. Refused, not truncated.
    #[test]
    fn a_scan_that_hit_its_bound_refuses_rather_than_repointing_part_of_the_graph() {
        let (cats, felines) = (ex("cats"), ex("felines"));
        let statements: Vec<Statement> = [concept(&cats), concept(&felines)]
            .into_iter()
            .chain((0..8).map(|n| s(&ex(&format!("c{n}")), &skos("broader"), &felines)))
            .collect();
        let model = CoreModel::from_statements(statements.iter().cloned());
        let mut scan = MergeScan::builder(felines.clone(), cats.clone())
            .with_bound(ReferenceBound { max_statements: 4 });
        for statement in &statements {
            scan.push(statement.clone());
        }

        let error = model
            .merge(&scan.build(), WalkBound::DEFAULT)
            .expect_err("an incomplete scan cannot answer");
        assert_eq!(error, MergeError::TooManyReferences { reached: 4 });
        assert!(error.to_string().contains("no longer exists"), "{error}");
    }

    /// The scan keeps the two concepts' statements and nothing else, which is what makes peak
    /// memory the degree of two concepts rather than the size of the vocabulary.
    #[test]
    fn the_scan_keeps_only_what_mentions_the_two_concepts() {
        let (cats, felines, other) = (ex("cats"), ex("felines"), ex("other"));
        let mut scan = MergeScan::builder(felines.clone(), cats.clone());
        scan.push(concept(&other));
        scan.push(s(&other, &skos("broader"), &ex("unrelated")));
        scan.push(concept(&felines));
        let scan = scan.build();

        assert_eq!(scan.references(), 1);
        assert!(scan.is_complete());
        assert_eq!(scan.source(), &felines);
        assert_eq!(scan.target(), &cats);
    }

    /// Two preferred labels in one language on the duplicate are an S14 violation the merge did
    /// not create. It must not carry it across as two preferred labels on the survivor either.
    #[test]
    fn two_preferred_labels_in_one_language_do_not_both_arrive_preferred() {
        let (cats, felines) = (ex("cats"), ex("felines"));
        let statements = [
            concept(&cats),
            concept(&felines),
            label(&felines, &skos("prefLabel"), "Felines", "en"),
            label(&felines, &skos("prefLabel"), "Feline", "en"),
        ];

        let merge = merge_of(&statements, &felines, &cats).expect("a merge");

        let preferred = merge
            .additions()
            .iter()
            .filter(|statement| statement.predicate == skos("prefLabel"))
            .count();
        assert_eq!(
            preferred,
            1,
            "{:?} would violate S14 on the survivor",
            rendered(merge.additions())
        );
        assert_eq!(merge.demotions().len(), 1);
    }

    /// A statement that mentions the duplicate in both positions rewrites to a self-link and goes
    /// nowhere, rather than arriving as `<B> skos:related <B>`.
    #[test]
    fn a_statement_naming_the_duplicate_twice_becomes_a_self_link() {
        let (cats, felines) = (ex("cats"), ex("felines"));
        let statements = [
            concept(&cats),
            concept(&felines),
            s(&felines, &skos("related"), &felines),
        ];

        let merge = merge_of(&statements, &felines, &cats).expect("a merge");

        assert_eq!(
            rendered(merge.self_links()),
            vec!["<http://example.org/cats> skos:related <http://example.org/cats>"]
        );
        assert!(merge.additions().is_empty());
    }
}
