//! `openbiz move` — put a concept, and everything below it, under a different broader concept.
//!
//! The first of `docs/BUILD-PLAN.md`'s bulk operations, and the first thing in this build that
//! raises a candidate carrying **both** halves of a change. `openbiz import` and `openbiz retract`
//! each carry one, because a file is one; a move is "this link goes, that link arrives" and the
//! two are one decision. Splitting it across two candidates would mean a reviewer could approve
//! the removal and reject the addition, leaving a branch of the thesaurus hanging off nothing —
//! a state nobody proposed and no report would call wrong.
//!
//! Nothing reaches the vocabulary here. The move is computed, staged as a candidate, and printed;
//! `openbiz approve` is what applies it, inside one transaction, as it does for every other
//! change. That is `CLAUDE.md` §3, and a bulk operation is exactly the producer the seam was built
//! in Phase 2 to receive.
//!
//! # Why it is a command and not an endpoint
//!
//! The same objection `openbiz import` records: there is no authentication yet, and
//! `POST /api/move` would be an unauthenticated way to re-hang a branch of somebody's thesaurus.
//! The candidate seam over HTTP is its own plan item and it lands with the identity, not before.
//!
//! # What the report has to say, and why
//!
//! A move's diff is two statements and its effect can be forty thousand concepts. A report that
//! showed only the diff would be accurate and useless — the reviewer would be approving a
//! two-line change without being told what moved. So the count of what moves with it comes
//! *first*, and the statements come after.

use openbiz_skos::{CoreModel, Node, Relocation, WalkBound};
use openbiz_store::{
    Candidate, CandidateSource, GraphId, Provenance, StatementRef, StatementTerm, Store,
};

use crate::cli::{actor, CommandError};

/// Propose moving `concept` under `to`, replacing the link to `from`.
///
/// `from` may be omitted when the concept has exactly one broader concept; with more than one it
/// is required, because a move replaces exactly one link and guessing which would be permanent.
///
/// Reads the vocabulary, computes the change, and stages it as a candidate. **Nothing is written
/// to the vocabulary.**
pub fn relocate(
    store: &Store,
    graph: &str,
    concept: &str,
    to: &str,
    from: Option<&str>,
) -> Result<String, CommandError> {
    let target = GraphId::vocabulary(graph)?;
    let model = crate::inspect::read(store, graph)?;

    let concept = Node::iri(concept);
    let to = Node::iri(to);
    let from = from.map(Node::iri);
    let move_ = model
        .relocate(&concept, &to, from.as_ref(), WalkBound::DEFAULT)
        .map_err(CommandError::Relocation)?;

    let provenance = Provenance {
        source: CandidateSource::BulkEdit,
        agent: format!("{} (openbiz move)", actor()?),
        note: format!(
            "moved {} out from under {} and under {}",
            move_.concept(),
            move_.from(),
            move_.to()
        ),
        // A computed move is not a guess. A confidence would be 1.0 or nothing, and 1.0 next to a
        // discovery match's 0.72 in the same list invites a reviewer to compare two numbers that
        // do not mean the same thing.
        confidence: None,
    };

    let additions = borrowed(move_.additions());
    let removals = borrowed(move_.removals());
    let candidate = store.propose_edit(&target, &additions, &removals, &provenance)?;

    Ok(report(graph, &model, &move_, &candidate))
}

/// The domain crate's owned statements as the store's borrowed ones.
///
/// The other direction of `crate::inspect::convert`, and the same cost of the layering that
/// `docs/adr/0019` records. A move only ever computes IRI-to-IRI statements, so the literal arm
/// cannot arise; it is mapped rather than panicked on because a `todo!()` here would be an
/// `unwrap()` wearing a different hat, and the store refuses a literal subject anyway.
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
/// Kept apart from the store so it can be tested against a relocation in hand.
fn report(graph: &str, model: &CoreModel, move_: &Relocation, candidate: &Candidate) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "{}{}\n",
        move_.concept(),
        named_in(model, move_.concept())
    ));
    out.push_str(&format!("in {graph}\n\n"));
    out.push_str(&format!(
        "out from under {}{}\n",
        move_.from(),
        named_in(model, move_.from())
    ));
    out.push_str(&format!(
        "         under {}{}\n",
        move_.to(),
        named_in(model, move_.to())
    ));

    // First, because it is the number that decides whether this is a small change or a large one,
    // and the diff below is two statements either way.
    out.push_str(&match move_.moved_with_it() {
        0 => "\nnothing is below it, so it moves alone.\n".to_owned(),
        1 => "\n1 concept is below it and moves with it.\n".to_owned(),
        many => format!("\n{many} concepts are below it and move with it.\n"),
    });
    out.push_str(
        "The concepts below it are not rewritten: they are below it by their own skos:broader \
         links, which do not mention the concept it is leaving.\n",
    );

    if !move_.top_concept_of().is_empty() {
        out.push_str(&format!(
            "\nnote: {} is recorded as a top concept of {}. That is unchanged by this move — it \
             already had a broader concept — and it is unusual enough to be worth seeing before \
             you approve.\n",
            move_.concept(),
            move_
                .top_concept_of()
                .iter()
                .map(|scheme| scheme.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    out.push_str("\nit would remove:\n");
    for statement in move_.removals() {
        out.push_str(&format!("  {statement}\n"));
    }
    out.push_str("and add:\n");
    for statement in move_.additions() {
        out.push_str(&format!("  {statement}\n"));
    }
    out.push_str(
        "\nThe direction each link is stated in is kept: SKOS S25 makes skos:broader and \
         skos:narrower inverses, so a vocabulary authored in one of them stays authored in it.\n",
    );

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
    use openbiz_store::{
        CandidateSource, Decision, GraphId, GraphIdError, Provenance, RdfSyntax, Store,
    };

    use super::relocate;
    use crate::cli::CommandError;

    const VOCABULARY: &str = "http://example.org/thesaurus";

    /// A store holding `turtle` in one registered vocabulary, through the seam data really uses.
    fn store_with(turtle: &str) -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(directory.path()).expect("an openable store");
        let target = GraphId::vocabulary(VOCABULARY).expect("a valid vocabulary IRI");
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
        (directory, store)
    }

    const ANIMALS: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix ex: <http://example.org/> .
        ex:scheme a skos:ConceptScheme .
        ex:animals a skos:Concept ; skos:prefLabel "Animals"@en .
        ex:mammals a skos:Concept ; skos:prefLabel "Mammals"@en ; skos:broader ex:animals ;
            skos:topConceptOf ex:scheme .
        ex:birds a skos:Concept ; skos:prefLabel "Birds"@en ; skos:broader ex:animals .
        ex:cats a skos:Concept ; skos:prefLabel "Cats"@en ; skos:broader ex:mammals .
    "#;

    /// A concept that is a top concept *and* has a broader concept is odd, and the move neither
    /// makes it so nor fixes it — it requires an existing parent, so the state predates it. The
    /// reviewer is told, because approving a re-parenting of a top concept without noticing that
    /// is exactly the mistake this report exists to prevent.
    #[test]
    fn a_top_concept_being_moved_is_named_in_the_report_and_not_refused() {
        let (_directory, store) = store_with(ANIMALS);
        let report = relocate(
            &store,
            VOCABULARY,
            "http://example.org/mammals",
            "http://example.org/birds",
            None,
        )
        .expect("odd is not the same as refusable");

        assert!(
            report.contains("recorded as a top concept of <http://example.org/scheme>"),
            "{report}"
        );
        assert!(
            report.contains("unchanged by this move"),
            "and it must be clear the move is not what caused it: {report}"
        );
        assert!(
            report.contains("1 concept is below it and moves with it"),
            "{report}"
        );
        assert!(report.contains("(\"Mammals\"@en)"), "{report}");
    }

    /// The move is against a vocabulary, and a graph that is not one is refused before any read.
    #[test]
    fn a_move_against_a_graph_that_is_not_a_vocabulary_is_refused() {
        let (_directory, store) = store_with(ANIMALS);
        let error = relocate(
            &store,
            "urn:openbiz:graph:system",
            "http://example.org/mammals",
            "http://example.org/birds",
            None,
        )
        .expect_err("OpenBiz's own graphs are not authored");
        assert!(
            matches!(error, CommandError::Graph(GraphIdError::Reserved { .. })),
            "{error}"
        );
    }
}
