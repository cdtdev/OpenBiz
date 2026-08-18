//! `openbiz inspect` — read a vocabulary and say what is in it, in SKOS terms.
//!
//! This is the composition root for the SKOS core model: the one place where the store's
//! engine-free statement type is mapped onto the domain crate's, because neither crate depends on
//! the other and something has to join them. That join is three lines and it is the whole cost of
//! the layering; see `docs/adr/0019`.
//!
//! # Why a command and not an endpoint
//!
//! For once, not the authentication objection — this only reads. It is a command because it is the
//! *first* caller of the core model and its job is to make the model's answers reachable and
//! checkable now, from a shell, against a real store on disk. The interface will want the same
//! answers rendered as a tree with counts beside each scheme, and that is Phase 2's concept-tree
//! item, not this one. Shipping a half-tree behind HTTP to look further along would be the
//! "built but no production caller" failure in reverse: a caller with nothing behind it.
//!
//! # Why it prints every derivation
//!
//! `CLAUDE.md` §3 requires every inference to explain itself, and an explanation nobody can read
//! is not one. The report therefore prints each derived fact with its premise and the
//! specification statement that licensed it, however many there are, exactly as
//! `openbiz candidate <id>` prints a whole staging graph. A silent cap would read as "that is all
//! there was" — the one thing a report about inference must never imply. An operator with a large
//! vocabulary redirects to a file; an operator with a truncated report has no way to know.

use openbiz_skos::{ClassOrigin, CoreModel, Literal, Node, SkosClass, Statement, Term};
use openbiz_store::{StatementRef, StatementTerm, Store};

use crate::cli::CommandError;

/// Read the vocabulary at `graph` and report what it holds.
///
/// Reads and nothing else. The statements stream out of the store one at a time and into the
/// model, so peak memory is the model rather than the graph.
///
/// An IRI with no registry entry is refused rather than reported as an empty vocabulary — the
/// store draws that distinction (see [`Store::for_each_statement`]) and losing it here would turn
/// a typo into a report of a well-formed empty thesaurus.
pub fn inspect(store: &Store, graph: &str) -> Result<String, CommandError> {
    let mut builder = CoreModel::builder();
    store.for_each_statement(graph, |statement| builder.push(convert(statement)))?;
    Ok(report(graph, &builder.build()))
}

/// The store's borrowed statement as the domain crate's owned one.
///
/// The two types exist separately so that neither crate depends on the other, which is the
/// decision `docs/adr/0019` records. This is where that decision is paid for.
fn convert(statement: StatementRef<'_>) -> Statement {
    Statement {
        subject: node(statement.subject),
        predicate: statement.predicate.to_owned(),
        object: term(statement.object),
    }
}

/// A term in subject position, which RDF guarantees is never a literal.
///
/// A literal subject cannot come out of the store — Oxigraph's own subject type cannot hold one —
/// so this is a translation, not a decision about malformed data.
fn node(term: StatementTerm<'_>) -> Node {
    match term {
        StatementTerm::Iri(iri) => Node::iri(iri),
        StatementTerm::Blank(label) => Node::blank(label),
        // Unreachable through the store; mapped rather than panicked on, because a `todo!()` here
        // would be an `unwrap()` wearing a different hat (`CLAUDE.md` §6). A blank node labelled
        // with the lexical form is wrong in an obvious way rather than fatal in a silent one.
        StatementTerm::Literal { value, .. } => Node::blank(value),
    }
}

/// A term in object position, which may be any of the three kinds.
fn term(value: StatementTerm<'_>) -> Term {
    match value {
        StatementTerm::Iri(iri) => Term::Node(Node::iri(iri)),
        StatementTerm::Blank(label) => Term::Node(Node::blank(label)),
        StatementTerm::Literal {
            value,
            language,
            datatype,
        } => Term::Literal(Literal {
            value: value.to_owned(),
            language: language.map(str::to_owned),
            datatype: datatype.to_owned(),
        }),
    }
}

/// Render the model as the report an operator reads.
///
/// Four sections, in the order somebody asking "what is this vocabulary?" wants them: what is in
/// it, how it is organised, what was inferred rather than stated, and what is wrong with it. A
/// section with nothing to say is left out rather than printed empty, except the last — "no
/// findings" is the answer to a question that was asked, and its absence would be indistinguishable
/// from a report that does not check.
fn report(graph: &str, model: &CoreModel) -> String {
    let mut out = format!(
        "<{graph}>\n  {} statement(s) read\n\n",
        model.statements_read()
    );

    for class in SkosClass::ALL {
        let total = model.count_of(class);
        let inferred = model
            .instances_of(class)
            .filter(|(_, resource)| {
                matches!(
                    resource.classes().get(&class),
                    Some(ClassOrigin::Entailed(_))
                )
            })
            .count();
        out.push_str(&format!("  {:<24}{total}", class.to_string()));
        if inferred > 0 {
            out.push_str(&format!("  ({inferred} inferred)"));
        }
        out.push('\n');
    }

    let schemes: Vec<_> = model.instances_of(SkosClass::ConceptScheme).collect();
    if !schemes.is_empty() {
        out.push_str("\nconcept schemes:\n");
        for (node, resource) in schemes {
            out.push_str(&format!(
                "  {node}  {} top concept(s)\n",
                resource.has_top_concept().len()
            ));
        }
    }

    // An ordered collection is also a collection under S29, so listing both classes would list it
    // twice. The order it is in is the thing worth saying about it, so that is what is said.
    let collections: Vec<_> = model.instances_of(SkosClass::Collection).collect();
    if !collections.is_empty() {
        out.push_str("\ncollections:\n");
        for (node, resource) in collections {
            out.push_str(&format!("  {node}  {} member(s)", resource.members().len()));
            if resource.is_a(SkosClass::OrderedCollection) {
                let ordered = resource
                    .member_lists()
                    .iter()
                    .filter(|list| list.is_well_formed())
                    .count();
                out.push_str(&format!(", ordered by {ordered} well-formed list(s)"));
            }
            out.push('\n');
        }
    }

    let derivations = model.derivations();
    if !derivations.is_empty() {
        out.push_str(&format!(
            "\nwhy: {} fact(s) were inferred rather than stated\n",
            derivations.len()
        ));
        for derivation in derivations {
            out.push_str(&format!("  {derivation}\n"));
        }
    }

    let findings = model.findings();
    out.push_str(&format!("\nfindings: {}\n", findings.len()));
    for finding in findings {
        out.push_str(&format!("  [{}] {finding}\n", finding.severity()));
    }

    out.push_str(if model.is_consistent() {
        "\nno SKOS integrity condition is violated by this graph.\n"
    } else {
        "\nthis graph violates a SKOS integrity condition and is not a SKOS vocabulary.\n"
    });

    out
}

#[cfg(test)]
mod tests {
    use openbiz_store::{GraphId, RdfSyntax, Store};

    use super::inspect;
    use crate::cli::CommandError;

    const VOCABULARY: &str = "http://example.org/thesaurus";

    /// A store holding `turtle` in one registered vocabulary, ready to inspect.
    fn store_with(turtle: &str) -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(directory.path()).expect("an openable store");
        let target = GraphId::vocabulary(VOCABULARY).expect("a valid vocabulary IRI");
        store
            .create_vocabulary_graph(&target)
            .expect("a fresh registration");

        // Through the seam, exactly as a user's data arrives: proposed, then approved. Writing
        // directly would test the report against statements no production path can produce.
        let candidate = store
            .propose_import(
                &target,
                RdfSyntax::Turtle,
                turtle.as_bytes(),
                &openbiz_store::Provenance {
                    source: openbiz_store::CandidateSource::Import,
                    agent: "test".to_owned(),
                    note: "fixture".to_owned(),
                    confidence: None,
                },
            )
            .expect("a well-formed proposal");
        store
            .decide(candidate.id(), openbiz_store::Decision::Approve, "test")
            .expect("an approvable candidate");
        (directory, store)
    }

    const PREFIXES: &str = "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
                            @prefix ex: <http://example.org/> .\n";

    #[test]
    fn a_vocabulary_is_reported_in_skos_terms() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             ex:scheme a skos:ConceptScheme ; skos:hasTopConcept ex:animals .
             ex:animals a skos:Concept ; skos:inScheme ex:scheme .
             ex:cat a skos:Concept ; skos:inScheme ex:scheme .
             ex:dog a skos:Concept ; skos:inScheme ex:scheme .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("skos:Concept"), "{report}");
        assert!(report.contains("<http://example.org/scheme>"), "{report}");
        assert!(report.contains("1 top concept(s)"), "{report}");
        assert!(
            report.contains("no SKOS integrity condition is violated"),
            "{report}"
        );
    }

    /// The report's whole reason for existing: an answer a user did not state, with its reason.
    #[test]
    fn an_inferred_fact_is_reported_with_the_rule_that_licensed_it() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             ex:cat a skos:Concept ; skos:topConceptOf ex:scheme .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        // Nothing typed ex:scheme, and nothing said skos:inScheme or skos:hasTopConcept.
        assert!(
            report.contains("were inferred rather than stated"),
            "{report}"
        );
        assert!(report.contains("S5"), "{report}");
        assert!(report.contains("S7"), "{report}");
        assert!(report.contains("S8"), "{report}");
        assert!(report.contains("1 top concept(s)"), "{report}");
    }

    #[test]
    fn a_violated_integrity_condition_is_named_and_the_graph_is_not_called_consistent() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             ex:muddle a skos:Concept, skos:ConceptScheme .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("[inconsistent]"), "{report}");
        assert!(report.contains("S9"), "{report}");
        assert!(
            report.contains("violates a SKOS integrity condition"),
            "{report}"
        );
    }

    /// Ill-formed is not inconsistent, and conflating them is how a tool refuses valid data.
    #[test]
    fn an_ill_formed_member_list_is_reported_without_calling_the_graph_inconsistent() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             @prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
             ex:group a skos:OrderedCollection ; skos:memberList ex:cell .
             ex:cell rdf:first ex:cat ; rdf:rest ex:cell .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("[ill-formed]"), "{report}");
        assert!(
            report.contains("no SKOS integrity condition is violated"),
            "{report}"
        );
    }

    /// A vocabulary created and not yet authored into. The seam refuses an empty *import*, so this
    /// state is reached by creating the graph and stopping — which is exactly what a user who has
    /// just made a vocabulary has, and the first thing they are likely to inspect.
    #[test]
    fn a_registered_but_empty_vocabulary_reports_nothing_rather_than_failing() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(directory.path()).expect("an openable store");
        store
            .create_vocabulary_graph(&GraphId::vocabulary(VOCABULARY).expect("a valid IRI"))
            .expect("a fresh registration");

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("0 statement(s) read"), "{report}");
        assert!(report.contains("findings: 0"), "{report}");
    }

    /// A typo must not read as "this vocabulary is empty and fine".
    #[test]
    fn an_unregistered_iri_is_refused_rather_than_reported_as_empty() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(directory.path()).expect("an openable store");

        let error = inspect(&store, "http://example.org/never-registered")
            .expect_err("an unregistered vocabulary");

        assert!(
            matches!(error, CommandError::Store(_)),
            "expected the store's refusal, got {error}"
        );
    }
}
