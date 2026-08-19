//! What every command that stages a change to a vocabulary has to do, done once.
//!
//! Three things belong here rather than in one command's module, because the second command that
//! needed them proved they were not about merging:
//!
//! - **[`newly_broken`]** — the SKOS integrity conditions a proposed change would leave a
//!   vocabulary failing. Iteration 43 built this inside `openbiz merge` after the first working
//!   merge produced, from ordinary input, a vocabulary violating S14 and S27. The lesson was not
//!   "a merge risks those two conditions"; it was that **predicting which conditions an operation
//!   risks is unreliable**, and the only honest check is to run the whole set against the
//!   vocabulary the change would leave. That reasoning is not about merges, so neither is this.
//! - **[`elsewhere`]** — the references to a concept that a change to *this* vocabulary cannot
//!   reach: other vocabularies, and changes still waiting for a decision.
//! - **[`borrowed`]** — the domain crate's owned statements as the store's borrowed ones, which is
//!   the cost of the layering `docs/adr/0019` records.
//!
//! `openbiz move` still does **not** call [`newly_broken`], and can leave a vocabulary with an S27
//! violation. That is a real defect, it is reproduced in `docs/UNTESTED.md`, and fixing it is
//! work a human authorises through `docs/PROPOSED.md` rather than something to fold quietly into
//! the item that happened to make it easy.

use std::collections::BTreeSet;

use openbiz_skos::{
    newly_violated, ConditionOutcome, CoreModel, Node, PropertyRefinements, Statement,
};
use openbiz_store::{CandidateState, GraphKind, StatementRef, StatementTerm, Store};

use crate::cli::CommandError;

/// The SKOS integrity conditions a proposed change would break, having read the vocabulary as it
/// would be afterwards.
///
/// `before` is the model of the vocabulary as it stands; `additions` and `removals` are the change
/// being proposed. The answer is the conditions that hold **now** and would not afterwards — never
/// the ones already failing, or a vocabulary that is in trouble could never be edited to fix it.
///
/// **The cost is real and stated.** This reads the vocabulary a second time and builds a second
/// model, so a checked operation is four passes over the graph rather than two. That is the price
/// of checking a proposal against the whole specification instead of against an author's
/// expectations, and it is paid by a bulk operation nobody runs in a loop. It is unmeasured on a
/// large vocabulary and recorded as such in `docs/UNTESTED.md`.
pub(crate) fn newly_broken(
    store: &Store,
    graph: &str,
    before: &CoreModel,
    additions: &[Statement],
    removals: &[Statement],
) -> Result<Vec<ConditionOutcome>, CommandError> {
    let removed: BTreeSet<Statement> = removals.iter().cloned().collect();

    // The same two passes `crate::inspect::read` makes, and for the same reason: a refinement
    // declaration may sit after every statement that uses it.
    let mut refinements = PropertyRefinements::builder();
    read_as_changed(store, graph, &removed, additions, |statement| {
        refinements.push(statement)
    })?;
    let mut builder = CoreModel::builder().with_refinements(refinements.build());
    read_as_changed(store, graph, &removed, additions, |statement| {
        builder.push(statement)
    })?;

    Ok(newly_violated(before, &builder.build()))
}

/// Stream the vocabulary as the change would leave it: without the removals, with the additions.
fn read_as_changed(
    store: &Store,
    graph: &str,
    removed: &BTreeSet<Statement>,
    additions: &[Statement],
    mut visit: impl FnMut(Statement),
) -> Result<(), CommandError> {
    store.for_each_statement(graph, |statement| {
        let statement = crate::inspect::convert(statement);
        if !removed.contains(&statement) {
            visit(statement);
        }
    })?;
    for statement in additions {
        visit(statement.clone());
    }
    Ok(())
}

/// Where else in the store a concept is mentioned, and how often.
///
/// Other vocabularies and the changes still waiting for a decision. Neither is touched: the first
/// belongs to somebody else's graph, and the second has not been agreed to yet.
pub(crate) fn elsewhere(
    store: &Store,
    graph: &str,
    concept: &Node,
) -> Result<Vec<(String, usize)>, CommandError> {
    let mut found = Vec::new();

    for other in store.graphs()? {
        if other.kind() != GraphKind::Vocabulary || other.iri() == graph {
            continue;
        }
        let count = count_in(store, other.iri(), concept)?;
        if count > 0 {
            found.push((format!("the vocabulary {}", other.iri()), count));
        }
    }

    for candidate in store.candidates()? {
        // Only the ones still waiting. An applied candidate's statements are in a vocabulary and
        // were counted there; a rejected one's are the record of what was refused and will never
        // be written.
        if candidate.state() != CandidateState::Proposed {
            continue;
        }
        let Some(payload) = candidate.payload() else {
            continue;
        };
        let count = count_in(store, payload.iri(), concept)?;
        if count > 0 {
            found.push((
                format!(
                    "candidate {}, which is waiting for a decision",
                    candidate.id()
                ),
                count,
            ));
        }
    }

    Ok(found)
}

/// How many statements in one graph mention `concept`.
fn count_in(store: &Store, graph: &str, concept: &Node) -> Result<usize, CommandError> {
    let iri = concept.as_iri().unwrap_or_default();
    let mut count = 0;
    store.for_each_statement(graph, |statement| {
        let names = matches!(statement.subject, StatementTerm::Iri(subject) if subject == iri)
            || matches!(statement.object, StatementTerm::Iri(object) if object == iri);
        if names {
            count += 1;
        }
    })?;
    Ok(count)
}

/// The domain crate's owned statements as the store's borrowed ones.
///
/// The other direction of `crate::inspect::convert`, and the same cost of the layering that
/// `docs/adr/0019` records. Both arms are exercised by the ordinary path: a merge repoints a
/// label and a split writes one.
pub(crate) fn borrowed(statements: &[Statement]) -> Vec<StatementRef<'_>> {
    statements
        .iter()
        .map(|statement| StatementRef {
            subject: term(&statement.subject),
            predicate: &statement.predicate,
            object: match &statement.object {
                openbiz_skos::Term::Node(node) => term(node),
                openbiz_skos::Term::Literal(literal) => StatementTerm::Literal {
                    value: &literal.value,
                    language: literal.language.as_deref(),
                    datatype: &literal.datatype,
                },
            },
        })
        .collect()
}

/// One node as the store's borrowed term.
fn term(node: &Node) -> StatementTerm<'_> {
    match node {
        Node::Iri(iri) => StatementTerm::Iri(iri),
        Node::Blank(label) => StatementTerm::Blank(label),
    }
}

/// A change refused because of what it would leave behind, and the conditions that say so.
#[derive(Debug)]
pub struct BrokenConditions {
    /// The vocabulary, which is what `openbiz integrity` takes — not the concepts involved.
    pub graph: String,
    /// What was being proposed, in the words the refusal opens with: "merging X into Y".
    pub change: String,
    /// The conditions that hold now and would not afterwards.
    pub broken: Vec<ConditionOutcome>,
}

impl std::fmt::Display for BrokenConditions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} would leave a graph that is not a SKOS vocabulary. {} that {} \
             now would not afterwards:",
            self.change,
            match self.broken.len() {
                1 => "One integrity condition".to_owned(),
                many => format!("{many} integrity conditions"),
            },
            match self.broken.len() {
                1 => "holds",
                _ => "hold",
            },
        )?;
        for outcome in &self.broken {
            // `forbids`, not the rule's full statement: each finding below prints the statement
            // as part of its own derivation, and printing it here as well said it twice.
            write!(
                f,
                "\n  {} ({}) — {}",
                outcome.condition.rule().number(),
                outcome.condition.section(),
                outcome.condition.forbids(),
            )?;
            for finding in outcome.violations.iter().take(3) {
                write!(f, "\n    {finding}")?;
            }
        }
        write!(
            f,
            "\nRetract what causes it first. `openbiz integrity {}` would have reported the same \
             thing afterwards — but by then the change would be in the vocabulary",
            self.graph
        )
    }
}
