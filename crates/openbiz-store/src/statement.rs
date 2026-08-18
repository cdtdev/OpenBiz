//! Reading one graph's statements out, without an engine type crossing the boundary.
//!
//! [`Store::export_graph`](crate::Store::export_graph) already gets a graph out, but it gets it out
//! as *bytes in a syntax*. A caller that wants to reason about the graph — count its concepts,
//! find its schemes, walk a collection's list — would have to serialise it and parse it back,
//! which means shipping a parser to read our own store.
//!
//! So this is the other exit: the same streaming scan, handing each statement to a closure as
//! borrowed strings. `CLAUDE.md` §3 forbids a third-party engine's types in our public API, so
//! [`StatementTerm`] and [`StatementRef`] are ours. They are deliberately thin — a term is an IRI,
//! a blank node label, or a literal with its language and datatype, and nothing else — because
//! their only job is to be translated at the boundary by whichever crate is asking.
//!
//! **The duplication is real and it is the price of the layering.** `openbiz-skos` has its own
//! owned statement type, and a caller maps between them. The alternative is the domain crate
//! depending on the storage crate or vice versa, and either direction makes one of them
//! untestable without the other. See `docs/adr/0019`.

use oxigraph::model::{NamedNodeRef, NamedOrBlankNodeRef, TermRef};
use oxigraph::store::Store as Backend;

use crate::StoreError;

/// One term of a statement, borrowed from the scan that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementTerm<'a> {
    /// An IRI.
    Iri(&'a str),
    /// A blank node's label, as the store holds it.
    Blank(&'a str),
    /// A literal.
    Literal {
        /// The lexical form.
        value: &'a str,
        /// The BCP 47 language tag, for a language-tagged string.
        language: Option<&'a str>,
        /// The datatype IRI. `rdf:langString` when `language` is set.
        datatype: &'a str,
    },
}

/// One statement of a graph, borrowed for the duration of the callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatementRef<'a> {
    /// What the statement is about. Never a literal.
    pub subject: StatementTerm<'a>,
    /// The predicate IRI.
    pub predicate: &'a str,
    /// What it says.
    pub object: StatementTerm<'a>,
}

impl<'a> From<NamedOrBlankNodeRef<'a>> for StatementTerm<'a> {
    fn from(node: NamedOrBlankNodeRef<'a>) -> Self {
        match node {
            NamedOrBlankNodeRef::NamedNode(iri) => StatementTerm::Iri(iri.as_str()),
            NamedOrBlankNodeRef::BlankNode(label) => StatementTerm::Blank(label.as_str()),
        }
    }
}

impl<'a> From<TermRef<'a>> for StatementTerm<'a> {
    fn from(term: TermRef<'a>) -> Self {
        match term {
            TermRef::NamedNode(iri) => StatementTerm::Iri(iri.as_str()),
            TermRef::BlankNode(label) => StatementTerm::Blank(label.as_str()),
            TermRef::Literal(literal) => StatementTerm::Literal {
                value: literal.value(),
                language: literal.language(),
                datatype: literal.datatype().as_str(),
            },
        }
    }
}

/// Hand every statement of `graph` to `visit`, and return how many there were.
pub(crate) fn for_each_statement(
    backend: &Backend,
    graph: NamedNodeRef<'_>,
    mut visit: impl FnMut(StatementRef<'_>),
) -> Result<usize, StoreError> {
    let mut seen = 0;
    for quad in backend.quads_for_pattern(None, None, None, Some(graph.into())) {
        let quad = quad.map_err(|error| StoreError::Backend(error.to_string()))?;
        visit(StatementRef {
            subject: quad.subject.as_ref().into(),
            predicate: quad.predicate.as_str(),
            object: quad.object.as_ref().into(),
        });
        seen += 1;
    }
    Ok(seen)
}

#[cfg(test)]
mod tests {
    use oxigraph::model::{Literal, NamedNode, Term};

    use crate::{
        CandidateSource, Decision, GraphId, Provenance, RdfSyntax, StatementTerm, Store, StoreError,
    };

    const VOCABULARY: &str = "http://example.org/vocabulary";
    const OTHER: &str = "http://example.org/other";

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("a temporary directory")
    }

    fn vocabulary(iri: &str) -> GraphId {
        GraphId::vocabulary(iri).expect("a valid absolute IRI outside the reserved namespace")
    }

    fn iri(iri: &str) -> NamedNode {
        NamedNode::new(iri).expect("a valid absolute IRI")
    }

    /// Every statement of `graph`, rendered so a test can assert on the whole set at once.
    fn read(store: &Store, graph: &str) -> Result<Vec<String>, StoreError> {
        let mut out = Vec::new();
        store.for_each_statement(graph, |statement| {
            out.push(format!(
                "{:?} {} {:?}",
                statement.subject, statement.predicate, statement.object
            ));
        })?;
        out.sort();
        Ok(out)
    }

    #[test]
    fn every_statement_of_the_graph_comes_out_and_nothing_of_any_other() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = vocabulary(VOCABULARY);
        let other = vocabulary(OTHER);
        store
            .transaction(|txn| {
                txn.create_vocabulary_graph(&graph)?;
                txn.create_vocabulary_graph(&other)?;
                txn.insert(
                    &graph,
                    vec![(
                        iri("http://example.org/a"),
                        iri("http://example.org/p"),
                        iri("http://example.org/b").into(),
                    )],
                )?;
                txn.insert(
                    &other,
                    vec![(
                        iri("http://example.org/x"),
                        iri("http://example.org/p"),
                        iri("http://example.org/y").into(),
                    )],
                )
            })
            .expect("two fresh vocabularies take their statements");

        let seen = read(&store, VOCABULARY).expect("the graph is readable");

        assert_eq!(seen.len(), 1);
        assert!(seen[0].contains("http://example.org/a"), "{seen:?}");
        // The other vocabulary's statement is not in it, and neither is the registry entry that
        // OpenBiz wrote about this graph — that lives in the system graph and never was here.
        assert!(!seen[0].contains("http://example.org/x"), "{seen:?}");
        assert!(!seen[0].contains("urn:openbiz"), "{seen:?}");
    }

    #[test]
    fn the_count_returned_is_the_number_of_statements_handed_over() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = vocabulary(VOCABULARY);
        store
            .transaction(|txn| {
                txn.create_vocabulary_graph(&graph)?;
                txn.insert(
                    &graph,
                    (0..7)
                        .map(|index| {
                            (
                                iri(&format!("http://example.org/{index}")),
                                iri("http://example.org/p"),
                                Literal::new_simple_literal("v").into(),
                            )
                        })
                        .collect(),
                )
            })
            .expect("a fresh vocabulary takes its statements");

        let mut handed_over = 0;
        let counted = store
            .for_each_statement(VOCABULARY, |_| handed_over += 1)
            .expect("readable");

        assert_eq!(counted, 7);
        assert_eq!(handed_over, counted);
    }

    #[test]
    fn a_literal_keeps_its_language_and_its_datatype() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = vocabulary(VOCABULARY);
        store
            .transaction(|txn| {
                txn.create_vocabulary_graph(&graph)?;
                txn.insert(
                    &graph,
                    vec![
                        (
                            iri("http://example.org/a"),
                            iri("http://www.w3.org/2004/02/skos/core#prefLabel"),
                            Literal::new_language_tagged_literal("Chemie", "de")
                                .expect("a valid language tag")
                                .into(),
                        ),
                        (
                            iri("http://example.org/a"),
                            iri("http://example.org/count"),
                            Literal::new_typed_literal(
                                "4",
                                iri("http://www.w3.org/2001/XMLSchema#integer"),
                            )
                            .into(),
                        ),
                    ],
                )
            })
            .expect("a fresh vocabulary takes its statements");

        let mut objects = Vec::new();
        store
            .for_each_statement(VOCABULARY, |statement| {
                if let StatementTerm::Literal {
                    value,
                    language,
                    datatype,
                } = statement.object
                {
                    objects.push((
                        value.to_owned(),
                        language.map(str::to_owned),
                        datatype.to_owned(),
                    ));
                }
            })
            .expect("readable");
        objects.sort();

        assert_eq!(
            objects,
            vec![
                (
                    "4".to_owned(),
                    None,
                    "http://www.w3.org/2001/XMLSchema#integer".to_owned()
                ),
                (
                    "Chemie".to_owned(),
                    Some("de".to_owned()),
                    "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString".to_owned()
                ),
            ],
            "a label's language and a value's datatype are what the SKOS model reads next"
        );
    }

    /// Blank nodes come out as blank nodes, with the label the store holds.
    ///
    /// This matters more than it looks: an `rdf:List` written in Turtle is a chain of blank nodes,
    /// so a `skos:memberList` is unreadable if the scan flattens them. There is no way to write a
    /// blank-node subject through the transaction API, which is deliberate, so this goes in the
    /// way a user would — through an import and an approval.
    #[test]
    fn a_blank_node_comes_out_as_a_blank_node() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = vocabulary(VOCABULARY);
        store
            .create_vocabulary_graph(&graph)
            .expect("a fresh vocabulary");

        let turtle = b"<http://example.org/c> \
             <http://www.w3.org/2004/02/skos/core#memberList> _:cell . \
             _:cell <http://www.w3.org/1999/02/22-rdf-syntax-ns#first> <http://example.org/x> .";
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                &turtle[..],
                &Provenance {
                    source: CandidateSource::Import,
                    agent: "the test".to_owned(),
                    note: "a list cell, which is necessarily a blank node".to_owned(),
                    confidence: None,
                },
            )
            .expect("the file is proposed");
        store
            .decide(candidate.id(), Decision::Approve, "the test")
            .expect("the candidate applies");

        let mut blanks = Vec::new();
        store
            .for_each_statement(VOCABULARY, |statement| {
                for term in [statement.subject, statement.object] {
                    if let StatementTerm::Blank(label) = term {
                        blanks.push(label.to_owned());
                    }
                }
            })
            .expect("readable");

        assert_eq!(blanks.len(), 2, "one as an object, one as a subject");
        assert_eq!(
            blanks[0], blanks[1],
            "the same blank node has the same label in both positions, which is the whole \
             requirement — an rdf:List cannot be walked otherwise"
        );
    }

    #[test]
    fn an_unregistered_graph_is_refused_rather_than_read_as_empty() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");

        let error = store
            .for_each_statement("http://example.org/never-created", |_| {})
            .expect_err("an unregistered graph is not readable");

        assert!(
            matches!(error, StoreError::NoSuchGraph { .. }),
            "a vocabulary that does not exist and one that is empty are different answers: {error}"
        );
    }

    #[test]
    fn a_registered_but_empty_graph_reads_as_empty() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        store
            .create_vocabulary_graph(&vocabulary(VOCABULARY))
            .expect("a fresh vocabulary");

        assert_eq!(
            read(&store, VOCABULARY).expect("readable"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_malformed_iri_is_refused_the_same_way_as_an_unregistered_one() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");

        let error = store
            .for_each_statement("not an iri", |_| {})
            .expect_err("a string that is not an IRI names no graph");

        assert!(matches!(error, StoreError::NoSuchGraph { .. }), "{error}");
    }

    /// A candidate's staging graph is a registered graph, so it reads like any other.
    ///
    /// Not a curiosity: it is what lets a reviewer be shown what a proposed change would do to the
    /// SKOS structure *before* approving it, using the same reader as the vocabulary itself.
    #[test]
    fn a_candidates_staging_graph_is_readable_before_it_is_approved() {
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = vocabulary(VOCABULARY);
        store
            .create_vocabulary_graph(&graph)
            .expect("a fresh vocabulary");

        let turtle = b"<http://example.org/a> \
             <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> \
             <http://www.w3.org/2004/02/skos/core#Concept> .";
        let candidate = store
            .propose_import(
                &graph,
                RdfSyntax::Turtle,
                &turtle[..],
                &Provenance {
                    source: CandidateSource::Import,
                    agent: "the test".to_owned(),
                    note: "one concept".to_owned(),
                    confidence: None,
                },
            )
            .expect("the file is proposed");
        let staged = candidate
            .payload()
            .expect("additions are staged")
            .iri()
            .to_owned();

        assert_eq!(read(&store, &staged).expect("readable").len(), 1);
        assert_eq!(
            read(&store, VOCABULARY).expect("readable"),
            Vec::<String>::new(),
            "nothing reached the vocabulary, which is the point of the seam"
        );
    }

    #[test]
    fn the_object_of_a_statement_may_be_any_of_the_three_kinds_of_term() {
        // A characterisation, so that a change to the term model shows up here rather than as a
        // silently dropped statement in the SKOS reader.
        let dir = temp_dir();
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = vocabulary(VOCABULARY);
        store
            .transaction(|txn| {
                txn.create_vocabulary_graph(&graph)?;
                txn.insert(
                    &graph,
                    vec![
                        (
                            iri("http://example.org/a"),
                            iri("http://example.org/p"),
                            Term::from(iri("http://example.org/b")),
                        ),
                        (
                            iri("http://example.org/a"),
                            iri("http://example.org/q"),
                            Literal::new_simple_literal("v").into(),
                        ),
                    ],
                )
            })
            .expect("a fresh vocabulary takes its statements");

        let mut kinds = Vec::new();
        store
            .for_each_statement(VOCABULARY, |statement| {
                kinds.push(match statement.object {
                    StatementTerm::Iri(_) => "iri",
                    StatementTerm::Blank(_) => "blank",
                    StatementTerm::Literal { .. } => "literal",
                });
                // A subject is never a literal, which is what makes the SKOS reader's `Node` type
                // able to be narrower than its `Term`.
                assert!(!matches!(statement.subject, StatementTerm::Literal { .. }));
            })
            .expect("readable");
        kinds.sort_unstable();

        assert_eq!(kinds, vec!["iri", "literal"]);
    }
}
