//! `openbiz merge` — make two concepts one, and repoint everything that pointed at the duplicate.
//!
//! The second of `docs/BUILD-PLAN.md`'s bulk operations. Like `openbiz move` it raises **one**
//! candidate carrying both halves of the change, because approving the removals without the
//! additions would delete a concept and leave every reference to it dangling — a state nobody
//! proposed and one no SKOS integrity condition reports, since a statement whose object is an IRI
//! nothing else describes is perfectly well-formed RDF.
//!
//! Nothing reaches the vocabulary here. The merge is computed, staged as a candidate, and printed;
//! `openbiz approve` applies it inside one transaction. That is `CLAUDE.md` §3.
//!
//! # Why it reads the graph twice and the model once
//!
//! The claim on the plan item is "every reference repointed", and the interpreted SKOS model
//! cannot support it: it holds what SKOS has a reading of, and an enterprise vocabulary is full of
//! statements it does not — `dcterms:creator`, an internal approval property, a reference from a
//! collection nobody typed. So the raw statements stream past an [`MergeScan`], which keeps only
//! those mentioning the two concepts, and the model is read for the SKOS questions the scan cannot
//! answer: what is a concept, what is a label, and where the hierarchy runs.
//!
//! # What it cannot repoint, and says so
//!
//! A reference from **another vocabulary** is a statement in another named graph, and changing it
//! is a change to that vocabulary — a different candidate, reviewed by whoever owns it. This
//! command does not silently reach across the boundary; it counts what it found and names where.
//! The same goes for a change already staged against this vocabulary and still waiting: approving
//! it after this merge would put the reference back.
//!
//! # Why a command and not an endpoint
//!
//! The same objection every writing path in this build records: there is no authentication yet,
//! and `POST /api/merge` would be an unauthenticated way to delete a concept out of somebody's
//! thesaurus. The candidate seam over HTTP is its own plan item and lands with the identity.

use openbiz_skos::{
    newly_violated, ConditionOutcome, CoreModel, Demotion, Merge, MergeScan, Node,
    PropertyRefinements, Statement, WalkBound,
};
use std::collections::BTreeSet;

use openbiz_store::{
    Candidate, CandidateSource, CandidateState, GraphId, GraphKind, Provenance, StatementRef,
    StatementTerm, Store,
};

use crate::cli::{actor, CommandError};

/// Propose merging `source` into `target`, repointing every reference in `graph`.
///
/// Reads the vocabulary, computes the change, and stages it as a candidate. **Nothing is written
/// to the vocabulary.**
pub fn merge(
    store: &Store,
    graph: &str,
    source: &str,
    target: &str,
) -> Result<String, CommandError> {
    let vocabulary = GraphId::vocabulary(graph)?;
    let model = crate::inspect::read(store, graph)?;

    let source = Node::iri(source);
    let target = Node::iri(target);
    let mut scan = MergeScan::builder(source.clone(), target.clone());
    store.for_each_statement(graph, |statement| {
        scan.push(crate::inspect::convert(statement))
    })?;
    let merge = model
        .merge(&scan.build(), WalkBound::DEFAULT)
        .map_err(CommandError::Merge)?;

    // The change is computed; now check what it would leave behind. See `would_break` for why
    // this is the whole condition set and not the two conditions a merge obviously risks.
    let broken = would_break(store, graph, &model, &merge)?;
    if !broken.is_empty() {
        return Err(CommandError::MergeBreaksIntegrity(Box::new(
            BrokenConditions {
                graph: graph.to_owned(),
                source: merge.source().clone(),
                target: merge.target().clone(),
                broken,
            },
        )));
    }

    let elsewhere = elsewhere(store, graph, &source)?;

    let provenance = Provenance {
        source: CandidateSource::BulkEdit,
        agent: format!("{} (openbiz merge)", actor()?),
        note: format!(
            "merged {} into {}, repointing {} statements",
            merge.source(),
            merge.target(),
            merge.removals().len()
        ),
        // A computed merge is not a guess; see the same note on `openbiz move`.
        confidence: None,
    };

    let additions = borrowed(merge.additions());
    let removals = borrowed(merge.removals());
    let candidate = store.propose_edit(&vocabulary, &additions, &removals, &provenance)?;

    Ok(report(graph, &model, &merge, &elsewhere, &candidate))
}

/// The SKOS integrity conditions this merge would break, having read the vocabulary as it would
/// be afterwards.
///
/// **The whole condition set, not a subset.** A hand-written check for the conditions a merge is
/// *expected* to risk would have caught S14 — both concepts have a preferred label, so the
/// collision is obvious — and missed S27 entirely, which a merge breaks whenever one concept is
/// `skos:related` to something above the other. That was found by running the command against a
/// store on disk, not by reasoning about it, and it is exactly the argument for asking the model
/// the question it already answers rather than predicting the answer.
///
/// **The cost is real and stated.** This reads the vocabulary a second time and builds a second
/// model, so a merge is four passes over the graph rather than two. That is the price of checking
/// a proposal against the whole specification instead of against an author's expectations, and it
/// is paid by a bulk operation nobody runs in a loop. It is unmeasured on a large vocabulary and
/// recorded as such in `docs/UNTESTED.md`.
fn would_break(
    store: &Store,
    graph: &str,
    before: &CoreModel,
    merge: &Merge,
) -> Result<Vec<ConditionOutcome>, CommandError> {
    let removed: BTreeSet<Statement> = merge.removals().iter().cloned().collect();

    // The same two passes `crate::inspect::read` makes, and for the same reason: a refinement
    // declaration may sit after every statement that uses it.
    let mut refinements = PropertyRefinements::builder();
    read_as_merged(store, graph, &removed, merge, |statement| {
        refinements.push(statement)
    })?;
    let mut builder = CoreModel::builder().with_refinements(refinements.build());
    read_as_merged(store, graph, &removed, merge, |statement| {
        builder.push(statement)
    })?;

    Ok(newly_violated(before, &builder.build()))
}

/// Stream the vocabulary as it would be after the merge: without the removals, with the additions.
fn read_as_merged(
    store: &Store,
    graph: &str,
    removed: &BTreeSet<Statement>,
    merge: &Merge,
    mut visit: impl FnMut(Statement),
) -> Result<(), CommandError> {
    store.for_each_statement(graph, |statement| {
        let statement = crate::inspect::convert(statement);
        if !removed.contains(&statement) {
            visit(statement);
        }
    })?;
    for statement in merge.additions() {
        visit(statement.clone());
    }
    Ok(())
}

/// A merge refused because of what it would leave behind, and the conditions that say so.
#[derive(Debug)]
pub struct BrokenConditions {
    /// The vocabulary, which is what `openbiz integrity` takes — not either concept.
    pub graph: String,
    /// The concept that would have been merged away.
    pub source: Node,
    /// The concept that would have survived.
    pub target: Node,
    /// The conditions that hold now and would not afterwards.
    pub broken: Vec<ConditionOutcome>,
}

impl std::fmt::Display for BrokenConditions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "merging {} into {} would leave a graph that is not a SKOS vocabulary. {} that {} \
             now would not afterwards:",
            self.source,
            self.target,
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
             thing after the merge — but by then the change would be in the vocabulary",
            self.graph
        )
    }
}

/// Where else in the store the merged concept is mentioned, and how often.
///
/// Other vocabularies and the changes still waiting for a decision. Neither is touched: the first
/// belongs to somebody else's graph, and the second has not been agreed to yet.
fn elsewhere(
    store: &Store,
    graph: &str,
    source: &Node,
) -> Result<Vec<(String, usize)>, CommandError> {
    let mut found = Vec::new();

    for other in store.graphs()? {
        if other.kind() != GraphKind::Vocabulary || other.iri() == graph {
            continue;
        }
        let count = count_in(store, other.iri(), source)?;
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
        let count = count_in(store, payload.iri(), source)?;
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
/// `docs/adr/0019` records. Unlike a move, a merge really does carry literals — a repointed label
/// is one — so both arms of this are exercised by the ordinary path.
fn borrowed(statements: &[openbiz_skos::Statement]) -> Vec<StatementRef<'_>> {
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

/// What the operator reads back, in the order they need it.
///
/// A merge's diff can be long and its *effect* is one sentence: this IRI stops existing. So the
/// sentence comes first, then the things the operator did not ask for — a demoted label, a link
/// that went, a reference this command cannot reach — and the statements last.
fn report(
    graph: &str,
    model: &CoreModel,
    merge: &Merge,
    elsewhere: &[(String, usize)],
    candidate: &Candidate,
) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "{}{}\n",
        merge.source(),
        named_in(model, merge.source())
    ));
    out.push_str(&format!(
        "merged into {}{}\n",
        merge.target(),
        named_in(model, merge.target())
    ));
    out.push_str(&format!("in {graph}\n"));

    out.push_str(&format!(
        "\n{} would stop existing in this vocabulary: {} and {} it. Nothing else in {graph} would \
         mention it.\n",
        merge.source(),
        statements_about(merge.subjects()),
        statements_at(merge.objects()),
    ));
    out.push_str(
        "The candidate is the record that it existed. A merge does not leave a tombstone behind \
         in the vocabulary; deprecating a concept in place is a different change.\n",
    );

    if !merge.demotions().is_empty() {
        out.push_str(
            "\nSKOS S14 allows one preferred label per language, so these become alternative \
             labels on the surviving concept rather than being dropped:\n",
        );
        for Demotion {
            label,
            in_favour_of,
        } in merge.demotions()
        {
            out.push_str(&format!("  {label} yields to {in_favour_of}\n"));
        }
    }

    if !merge.self_links().is_empty() {
        out.push_str(
            "\nThese two concepts are linked to each other, so the link goes rather than becoming \
             a link from the survivor to itself:\n",
        );
        for statement in merge.self_links() {
            out.push_str(&format!("  not written: {statement}\n"));
        }
    }

    if !merge.already_said().is_empty() {
        out.push_str(&format!(
            "\n{} the vocabulary already carries, so {} not proposed again:\n",
            match merge.already_said().len() {
                1 => "1 statement".to_owned(),
                many => format!("{many} statements"),
            },
            match merge.already_said().len() {
                1 => "it is",
                _ => "they are",
            }
        ));
        for statement in merge.already_said() {
            out.push_str(&format!("  already there: {statement}\n"));
        }
    }

    if !elsewhere.is_empty() {
        out.push_str(&format!(
            "\nwarning: {} is also mentioned outside this vocabulary, and this change does not \
             touch it — a statement in another graph is a change to that graph, reviewed by \
             whoever owns it:\n",
            merge.source()
        ));
        for (source, count) in elsewhere {
            out.push_str(&format!("  {count} in {source}\n"));
        }
    }

    out.push_str("\nit would remove:\n");
    for statement in merge.removals() {
        out.push_str(&format!("  {statement}\n"));
    }
    match merge.additions().is_empty() {
        true => out.push_str(
            "and add nothing: everything the duplicate said, the surviving concept already says.\n",
        ),
        false => {
            out.push_str("and add:\n");
            for statement in merge.additions() {
                out.push_str(&format!("  {statement}\n"));
            }
        }
    }

    out.push_str(&format!(
        "\nproposed candidate {} against {}. Nothing has been written to the vocabulary. Review \
         it with `openbiz candidate {}`, then `openbiz approve {}` or `openbiz reject {}`.\n",
        candidate.id(),
        candidate.target(),
        candidate.id(),
        candidate.id(),
        candidate.id(),
    ));

    out
}

/// "3 statements are about" / "1 statement is about".
fn statements_about(count: usize) -> String {
    match count {
        1 => "1 statement is about it".to_owned(),
        many => format!("{many} statements are about it"),
    }
}

/// "2 point at" / "1 points at".
fn statements_at(count: usize) -> String {
    match count {
        0 => "nothing points at".to_owned(),
        1 => "1 points at".to_owned(),
        many => format!("{many} point at"),
    }
}

/// A concept's preferred label in parentheses, or nothing when it has none.
fn named_in(model: &CoreModel, node: &Node) -> String {
    model
        .resource(node)
        .and_then(|resource| resource.display_label())
        .map(|label| format!(" ({label})"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use openbiz_skos::MergeError;
    use openbiz_store::{
        CandidateId, CandidateSource, Decision, GraphId, GraphIdError, Provenance, RdfSyntax, Store,
    };

    use super::merge;
    use crate::cli::CommandError;

    const VOCABULARY: &str = "http://example.org/thesaurus";
    const OTHER: &str = "http://example.org/other";

    /// A store holding `turtle` in one registered vocabulary, through the seam data really uses.
    fn store_with(turtle: &str) -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(directory.path()).expect("an openable store");
        load(&store, VOCABULARY, turtle);
        (directory, store)
    }

    fn load(store: &Store, graph: &str, turtle: &str) {
        let target = GraphId::vocabulary(graph).expect("a valid vocabulary IRI");
        store
            .create_vocabulary_graph(&target)
            .expect("a fresh registration");
        let candidate = store
            .propose_import(
                &target,
                RdfSyntax::Turtle,
                turtle.as_bytes(),
                &Provenance {
                    source: CandidateSource::Import,
                    agent: "test".to_owned(),
                    note: "fixture".to_owned(),
                    confidence: None,
                },
            )
            .expect("a well-formed proposal");
        store
            .decide(candidate.id(), Decision::Approve, "test")
            .expect("an approvable candidate");
    }

    const DUPLICATES: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix ex: <http://example.org/> .
        ex:animals a skos:Concept ; skos:prefLabel "Animals"@en .
        ex:cats a skos:Concept ; skos:prefLabel "Cats"@en ; skos:broader ex:animals .
        ex:felines a skos:Concept ; skos:prefLabel "Felines"@en ; skos:broader ex:animals .
        ex:tabby a skos:Concept ; skos:prefLabel "Tabby"@en ; skos:broader ex:felines .
        ex:policy ex:approvedBy ex:felines .
    "#;

    /// The whole shape of the command: the sentence first, then what the operator did not ask
    /// for, then the diff, then the candidate to review.
    #[test]
    fn a_merge_reports_what_stops_existing_before_it_shows_the_statements() {
        let (_directory, store) = store_with(DUPLICATES);
        let report = merge(
            &store,
            VOCABULARY,
            "http://example.org/felines",
            "http://example.org/cats",
        )
        .expect("an ordinary merge");

        assert!(report.contains("(\"Felines\"@en)"), "{report}");
        assert!(
            report.contains("would stop existing in this vocabulary"),
            "{report}"
        );
        assert!(
            report.contains("3 statements are about it and 2 point at it"),
            "{report}"
        );
        assert!(
            report.contains("\"Felines\"@en yields to \"Cats\"@en"),
            "the demotion is the one thing here the operator did not ask for: {report}"
        );
        assert!(
            report.contains("<http://example.org/tabby> skos:broader <http://example.org/cats>"),
            "{report}"
        );
        assert!(
            report.contains(
                "<http://example.org/policy> <http://example.org/approvedBy> \
                 <http://example.org/cats>"
            ),
            "a reference SKOS has no reading of is still a reference: {report}"
        );
        assert!(report.contains("proposed candidate"), "{report}");
    }

    /// The claim the item is checked off on: after approval, nothing in the vocabulary mentions
    /// the merged IRI. Read off the store rather than off the report.
    #[test]
    fn after_approval_the_merged_concept_is_mentioned_by_nothing() {
        let (_directory, store) = store_with(DUPLICATES);
        let report = merge(
            &store,
            VOCABULARY,
            "http://example.org/felines",
            "http://example.org/cats",
        )
        .expect("an ordinary merge");
        let id = CandidateId::parse(
            report
                .split("proposed candidate ")
                .nth(1)
                .and_then(|rest| rest.split(' ').next())
                .expect("a candidate id in the report"),
        )
        .expect("the report names the candidate the way the command line takes it");
        store
            .decide(id, Decision::Approve, "test")
            .expect("an approvable candidate");

        let mut mentions = 0;
        let mut repointed = 0;
        store
            .for_each_statement(VOCABULARY, |statement| {
                let rendered = format!("{statement:?}");
                if rendered.contains("example.org/felines") {
                    mentions += 1;
                }
                if rendered.contains("approvedBy") && rendered.contains("example.org/cats") {
                    repointed += 1;
                }
            })
            .expect("a readable vocabulary");

        assert_eq!(mentions, 0, "the merged IRI survived the merge");
        assert_eq!(repointed, 1, "and the reference to it did not follow");
    }

    /// A reference from another vocabulary is a change to that vocabulary. It is counted and
    /// named, never silently rewritten — and never silently left out either.
    #[test]
    fn a_reference_from_another_vocabulary_is_reported_and_not_touched() {
        let (_directory, store) = store_with(DUPLICATES);
        load(
            &store,
            OTHER,
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:mousers a skos:Concept ; skos:closeMatch ex:felines .
            "#,
        );

        let report = merge(
            &store,
            VOCABULARY,
            "http://example.org/felines",
            "http://example.org/cats",
        )
        .expect("a merge");

        assert!(
            report.contains("also mentioned outside this vocabulary"),
            "{report}"
        );
        assert!(
            report.contains("1 in the vocabulary http://example.org/other"),
            "{report}"
        );

        let mut survives = 0;
        store
            .for_each_statement(OTHER, |statement| {
                if format!("{statement:?}").contains("example.org/felines") {
                    survives += 1;
                }
            })
            .expect("a readable vocabulary");
        assert_eq!(survives, 1, "the other vocabulary must be untouched");
    }

    /// A duplicate whose every statement the survivor already carries adds nothing, and the
    /// report says so rather than printing an empty heading.
    #[test]
    fn a_merge_that_adds_nothing_says_so() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:animals a skos:Concept .
            ex:cats a skos:Concept ; skos:prefLabel "Cats"@en ; skos:broader ex:animals .
            ex:felines a skos:Concept ; skos:broader ex:animals .
            "#,
        );

        let report = merge(
            &store,
            VOCABULARY,
            "http://example.org/felines",
            "http://example.org/cats",
        )
        .expect("a merge");

        assert!(
            report.contains("add nothing: everything the duplicate said"),
            "{report}"
        );
        assert!(report.contains("nothing points at it"), "{report}");
    }

    /// The refusal reaches the command line as a refusal, with the route named.
    #[test]
    fn a_merge_that_would_close_a_cycle_is_refused_by_the_command() {
        let (_directory, store) = store_with(DUPLICATES);
        let error = merge(
            &store,
            VOCABULARY,
            "http://example.org/tabby",
            "http://example.org/animals",
        )
        .expect_err("tabby is two links below animals");

        assert!(
            matches!(error, CommandError::Merge(MergeError::WouldCycle { .. })),
            "{error}"
        );
        assert!(error.to_string().contains("would make a cycle"), "{error}");
    }

    /// The case that made this check exist, found by running the command against a store on disk
    /// rather than by reasoning about it. S27 is nowhere in a merge's obvious risk surface: it is
    /// broken because the survivor is `skos:related` to something the duplicate was *below*, and
    /// no author writing a merge would predict it. Refused, with the counter-example.
    #[test]
    fn a_merge_that_would_break_an_integrity_condition_is_refused_and_stages_nothing() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:animals a skos:Concept .
            ex:cats a skos:Concept ; skos:related ex:animals .
            ex:felines a skos:Concept ; skos:broader ex:animals .
            "#,
        );

        let error = merge(
            &store,
            VOCABULARY,
            "http://example.org/felines",
            "http://example.org/cats",
        )
        .expect_err("the survivor would be related to a concept above it");

        let said = error.to_string();
        assert!(
            matches!(error, CommandError::MergeBreaksIntegrity(_)),
            "{said}"
        );
        assert!(said.contains("not a SKOS vocabulary"), "{said}");
        assert!(said.contains("S27"), "{said}");
        assert!(
            said.contains(&format!("openbiz integrity {VOCABULARY}")),
            "the next command has to name the vocabulary, not either concept: {said}"
        );
        assert_eq!(
            store.candidates().expect("a readable list").len(),
            1,
            "a refused merge must leave no candidate behind — only the fixture import"
        );
    }

    /// A vocabulary that already violates a condition must stay editable: refusing every merge for
    /// a fault the merge did not introduce would make the tool unable to fix it.
    #[test]
    fn a_condition_the_vocabulary_already_violates_does_not_refuse_the_merge() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:animals a skos:Concept .
            ex:cats a skos:Concept ; skos:related ex:animals ; skos:broader ex:animals .
            ex:felines a skos:Concept .
            "#,
        );

        merge(
            &store,
            VOCABULARY,
            "http://example.org/felines",
            "http://example.org/cats",
        )
        .expect("S27 was already violated before this merge, and the merge did not make it worse");
    }

    /// The merge is against a vocabulary, and a graph that is not one is refused before any read.
    #[test]
    fn a_merge_against_a_graph_that_is_not_a_vocabulary_is_refused() {
        let (_directory, store) = store_with(DUPLICATES);
        let error = merge(
            &store,
            "urn:openbiz:graph:system",
            "http://example.org/felines",
            "http://example.org/cats",
        )
        .expect_err("OpenBiz's own graphs are not authored");
        assert!(
            matches!(error, CommandError::Graph(GraphIdError::Reserved { .. })),
            "{error}"
        );
    }
}
