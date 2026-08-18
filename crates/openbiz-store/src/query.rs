//! SPARQL 1.1 Query evaluation, bounded and read-only.
//!
//! # The dataset a query sees, and why it is not the whole store
//!
//! Every quad in an OpenBiz store lives in a named graph — nothing is ever written to the default
//! graph (see [`crate::graph`]). A SPARQL query whose default graph is "the store's default graph"
//! would therefore match *nothing at all*, and `SELECT * WHERE { ?s ?p ?o }` returning zero rows
//! against a populated store is not a subtlety a user recovers from; it reads as a broken product.
//!
//! The opposite default is worse in a way that is harder to see. Making the default graph the
//! union of **every** graph puts OpenBiz's own bookkeeping — the format stamp, the graph registry,
//! later the workflow and provenance records — into the results of a taxonomist's first query,
//! mixed in with their vocabulary and with nothing to distinguish it. That is precisely the
//! failure `crate::graph` was built to prevent and that `openbiz_server::graphs` describes
//! VocBench committing: our metadata presented as the user's data.
//!
//! So the rule is: **a query that names no dataset of its own is evaluated over the union of the
//! registered vocabulary graphs, and over those graphs alone.** The system graph and any inferred
//! graph are outside it. That rule is not a secret the user has to infer — the graphs it covers
//! are exactly the entries `GET /api/graphs` reports with `kind: "vocabulary"`, which is already
//! served, already in the interface, and already the answer to "which vocabularies do I have".
//!
//! A query that *does* name its own dataset with `FROM` or `FROM NAMED` is honoured verbatim,
//! including when it names one of ours. That is SPARQL 1.1's own rule about dataset specification
//! and it is the escape hatch an operator needs to ask "what is actually in my store?" — the
//! question `CLAUDE.md` §1 exists to keep answerable. Nothing is hidden; the default is chosen.
//!
//! # Why a query is bounded
//!
//! `CLAUDE.md` §3 records that Oxigraph's query evaluation is explicitly not yet optimised
//! upstream, and §1.5 commits us to modest memory at rest. An unbounded SPARQL endpoint in front
//! of an unoptimised evaluator is a way for one caller to take the server down with one line of
//! valid, well-meant SPARQL — the accidental cartesian product every SPARQL user has written.
//!
//! Two bounds, both in [`QueryLimits`]: a wall-clock deadline and a cap on the number of answers.
//! Both **refuse** rather than truncate. A truncated result set that is presented as a complete
//! one is the single worst thing this endpoint could do: a governance team would export it, sign
//! it off, and never learn that the rows they needed were the ones past the cap.
//!
//! # Read-only, and it says so by name
//!
//! This evaluates queries. A SPARQL Update sent here is refused as [`StoreError::QueryIsUpdate`] —
//! recognised by *parsing it as an update*, not by sniffing for a keyword, so the refusal says
//! what the text actually is rather than complaining about a syntax error at token three.
//!
//! # Federation
//!
//! `SERVICE` is not available. The store links Oxigraph without its `http-client` feature
//! (`docs/adr/0006`), so a federated query fails inside the evaluator rather than opening a
//! socket. That is the air-gap commitment of `CLAUDE.md` §1.1 enforced by what is compiled in,
//! and `a_federated_query_cannot_reach_the_network` is the test that keeps it honest.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use oxigraph::io::RdfSerializer;
use oxigraph::model::{GraphName, NamedNode, NamedOrBlankNode};
use oxigraph::sparql::results::QueryResultsSerializer;
use oxigraph::sparql::{CancellationToken, QueryEvaluationError, QueryResults, SparqlEvaluator};

use crate::{GraphKind, RdfSyntax, ResultsSyntax, Store, StoreError};

/// What shape of answer a query produced.
///
/// The three the specification defines. Which one a query yields decides which family of
/// serialisation the answer is written in, so a caller negotiating content has to know it —
/// exhaustive on purpose, so a fourth shape would fail every caller's build rather than fall into
/// a wildcard arm that guesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryShape {
    /// A `SELECT`: a table of variable bindings, written in a [`ResultsSyntax`].
    Solutions,
    /// An `ASK`: a boolean, written in a [`ResultsSyntax`].
    Boolean,
    /// A `CONSTRUCT` or `DESCRIBE`: RDF, written in an [`RdfSyntax`].
    Graph,
}

impl QueryShape {
    /// Whether this shape is written as SPARQL results rather than as RDF.
    pub const fn is_results(self) -> bool {
        match self {
            Self::Solutions | Self::Boolean => true,
            Self::Graph => false,
        }
    }
}

/// The serialisation to use for each answer shape.
///
/// Both are supplied up front because the shape of a query's answer is only known once it has been
/// parsed, while a caller negotiating an HTTP `Accept` header has to state its preferences before
/// anything is parsed. Carrying both costs nothing and keeps the store from having to expose a
/// parse-then-execute pair purely so a caller can look at the shape in between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueryFormats {
    solutions: ResultsSyntax,
    graph: RdfSyntax,
}

impl QueryFormats {
    /// Write solutions and booleans as `solutions`, and constructed RDF as `graph`.
    pub const fn new(solutions: ResultsSyntax, graph: RdfSyntax) -> Self {
        Self { solutions, graph }
    }

    /// The syntax `SELECT` and `ASK` answers are written in.
    pub const fn solutions(self) -> ResultsSyntax {
        self.solutions
    }

    /// The syntax `CONSTRUCT` and `DESCRIBE` answers are written in.
    pub const fn graph(self) -> RdfSyntax {
        self.graph
    }
}

impl Default for QueryFormats {
    fn default() -> Self {
        Self::new(ResultsSyntax::DEFAULT, RdfSyntax::DEFAULT)
    }
}

/// What a single query is allowed to consume.
///
/// A parameter rather than a constant so the values can come from configuration later without
/// touching a caller. They are not configurable in this build, which is recorded in
/// `docs/UNTESTED.md` rather than implied by the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueryLimits {
    max_answers: u64,
    timeout: Duration,
}

impl QueryLimits {
    /// The default cap on answers: enough for a real analytical query over a large vocabulary,
    /// small enough that the buffered response cannot exhaust a modest server.
    pub const DEFAULT_MAX_ANSWERS: u64 = 100_000;

    /// The default wall-clock deadline. Chosen to be longer than any interactive query should
    /// need and shorter than a human will wait before assuming the server has hung.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

    /// Limits of `max_answers` rows and `timeout` of wall-clock time.
    pub const fn new(max_answers: u64, timeout: Duration) -> Self {
        Self {
            max_answers,
            timeout,
        }
    }

    /// The most solutions, or constructed triples, a query may answer with.
    pub const fn max_answers(self) -> u64 {
        self.max_answers
    }

    /// How long a query may run before it is cancelled.
    pub const fn timeout(self) -> Duration {
        self.timeout
    }
}

impl Default for QueryLimits {
    fn default() -> Self {
        Self::new(Self::DEFAULT_MAX_ANSWERS, Self::DEFAULT_TIMEOUT)
    }
}

/// What a completed query produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QueryReport {
    shape: QueryShape,
    media_type: &'static str,
    answers: u64,
}

impl QueryReport {
    /// The shape of the answer, and so which family of syntax it was written in.
    pub const fn shape(self) -> QueryShape {
        self.shape
    }

    /// The media type of what was written, ready for a `Content-Type` header.
    ///
    /// Comes from the syntax actually used rather than from the caller's request, so a response
    /// cannot be labelled as something other than what it holds.
    pub const fn media_type(self) -> &'static str {
        self.media_type
    }

    /// How many solutions, or constructed triples, were written. `ASK` answers with one.
    pub const fn answers(self) -> u64 {
        self.answers
    }
}

impl Store {
    /// Evaluate a SPARQL 1.1 query and write its answer to `writer`.
    ///
    /// The dataset, the bounds, and the read-only guarantee are the module documentation's
    /// subject; the short version is that a query naming no dataset sees the registered vocabulary
    /// graphs and nothing else, that it is refused rather than truncated when it exceeds
    /// [`QueryLimits`], and that a SPARQL Update sent here is named as such and refused.
    ///
    /// # Partial output
    ///
    /// On `Err`, `writer` may already hold part of a document. A caller must discard it rather
    /// than send it: a truncated results document is syntactically plausible and semantically
    /// wrong, which is the failure mode this whole module is arranged against. Buffering into a
    /// `Vec` and only responding on `Ok` — which is what the HTTP layer does — makes that
    /// impossible to get wrong.
    ///
    /// # Cost
    ///
    /// Blocks, and for a large vocabulary blocks for a while: run it off an async runtime's
    /// worker. Takes no write lock, so a query never blocks an author and an author never blocks a
    /// query. The backend evaluates against a single snapshot taken when the query starts, so a
    /// commit landing mid-query cannot tear the answer.
    pub fn query(
        &self,
        query: &str,
        formats: QueryFormats,
        limits: QueryLimits,
        writer: impl std::io::Write,
    ) -> Result<QueryReport, StoreError> {
        let token = CancellationToken::new();
        let mut prepared = SparqlEvaluator::new()
            .with_cancellation_token(token.clone())
            .parse_query(query)
            .map_err(|error| {
                // Parsing it a second time, as an update, is what turns "syntax error at line 1"
                // into "that is an update, and this endpoint does not write". It only ever runs on
                // the failure path, so the common case pays nothing for it.
                if SparqlEvaluator::new().parse_update(query).is_ok() {
                    StoreError::QueryIsUpdate
                } else {
                    StoreError::QuerySyntax {
                        detail: error.to_string(),
                    }
                }
            })?;

        // `is_default_dataset` is true exactly when the query carried no `FROM`/`FROM NAMED`: a
        // dataset clause always names at least one IRI, so it can never produce the store's own
        // default graph. `the_system_graph_is_reachable_when_a_query_asks_for_it_by_name` pins
        // that down, because the whole escape hatch rests on it.
        if prepared.dataset().is_default_dataset() {
            let vocabularies = self.vocabulary_graphs()?;
            prepared.dataset_mut().set_default_graph(
                vocabularies
                    .iter()
                    .cloned()
                    .map(GraphName::NamedNode)
                    .collect(),
            );
            prepared.dataset_mut().set_available_named_graphs(
                vocabularies
                    .into_iter()
                    .map(NamedOrBlankNode::NamedNode)
                    .collect(),
            );
        }

        under_deadline(limits.timeout(), &token, || {
            let results = prepared
                .on_store(&self.backend)
                .execute()
                .map_err(evaluation_failed)?;
            write_answer(results, formats, limits, writer)
        })
    }

    /// The IRIs of the registered vocabulary graphs, as backend nodes.
    ///
    /// Read from the registry rather than from the backend's graph list, for the reason
    /// [`Store::graphs`] gives: a created-but-empty vocabulary is still a vocabulary, and a user
    /// querying one and getting "no such graph" would reasonably conclude it had been lost.
    fn vocabulary_graphs(&self) -> Result<Vec<NamedNode>, StoreError> {
        self.graphs()?
            .into_iter()
            .filter(|graph| graph.kind() == GraphKind::Vocabulary)
            .map(|graph| {
                NamedNode::new(graph.iri()).map_err(|error| {
                    self.corrupt(format!(
                        "the registry holds {} as a vocabulary graph, which is not a valid IRI: \
                         {error}",
                        graph.iri()
                    ))
                })
            })
            .collect()
    }
}

/// Run `work`, cancelling it through `token` if it has not finished within `timeout`.
///
/// A thread rather than a timer because the work is synchronous and holds the calling thread: the
/// only way to interrupt it is from somewhere else. The thread costs one `recv_timeout` and exits
/// the moment `work` returns and the sender is dropped, so it is not a background task that
/// outlives the query — a detail that matters because a server answering many queries would
/// otherwise accumulate one sleeping thread per query for the length of the timeout.
fn under_deadline<T>(timeout: Duration, token: &CancellationToken, work: impl FnOnce() -> T) -> T {
    let (finished, wait) = mpsc::channel::<()>();
    let deadline = token.clone();
    let watchdog = std::thread::spawn(move || {
        // Only a timeout cancels. `Disconnected` means the query finished and dropped its end,
        // which must not be read as "time is up" — the query has already produced its answer.
        if matches!(wait.recv_timeout(timeout), Err(RecvTimeoutError::Timeout)) {
            deadline.cancel();
        }
    });

    let outcome = work();

    drop(finished);
    // A watchdog that panicked would mean the deadline never fired, which is a bug in this
    // function rather than a fact about the query; there is nothing useful the caller could do
    // with it, and the query's own answer is what they asked for.
    let _ = watchdog.join();
    outcome
}

/// Serialise whichever shape of answer came back.
fn write_answer(
    results: QueryResults<'_>,
    formats: QueryFormats,
    limits: QueryLimits,
    writer: impl std::io::Write,
) -> Result<QueryReport, StoreError> {
    match results {
        QueryResults::Boolean(answer) => {
            QueryResultsSerializer::from_format(formats.solutions().backend())
                .serialize_boolean_to_writer(writer, answer)
                .map_err(|source| StoreError::AnswerWrite { source })?;

            Ok(QueryReport {
                shape: QueryShape::Boolean,
                media_type: formats.solutions().media_type(),
                answers: 1,
            })
        }
        QueryResults::Solutions(solutions) => {
            // Read before the iterator is consumed: the header of every results format is the
            // variable list, and it has to be written before the first row.
            let variables = solutions.variables().to_vec();
            let mut sink = QueryResultsSerializer::from_format(formats.solutions().backend())
                .serialize_solutions_to_writer(writer, variables)
                .map_err(|source| StoreError::AnswerWrite { source })?;

            let mut answers = 0;
            for solution in solutions {
                let solution = solution.map_err(evaluation_failed)?;
                answers += 1;
                if answers > limits.max_answers() {
                    return Err(StoreError::QueryTooLarge {
                        limit: limits.max_answers(),
                    });
                }
                sink.serialize(&solution)
                    .map_err(|source| StoreError::AnswerWrite { source })?;
            }
            sink.finish()
                .map_err(|source| StoreError::AnswerWrite { source })?;

            Ok(QueryReport {
                shape: QueryShape::Solutions,
                media_type: formats.solutions().media_type(),
                answers,
            })
        }
        QueryResults::Graph(triples) => {
            let mut sink = RdfSerializer::from_format(formats.graph().backend()).for_writer(writer);

            let mut answers = 0;
            for triple in triples {
                let triple = triple.map_err(evaluation_failed)?;
                answers += 1;
                if answers > limits.max_answers() {
                    return Err(StoreError::QueryTooLarge {
                        limit: limits.max_answers(),
                    });
                }
                // A constructed triple belongs to no graph, so it is written as a triple in every
                // syntax — including the three that could have carried a graph name. There is no
                // name to carry: `CONSTRUCT` builds a graph that did not previously exist.
                sink.serialize_triple(&triple)
                    .map_err(|source| StoreError::AnswerWrite { source })?;
            }
            sink.finish()
                .map_err(|source| StoreError::AnswerWrite { source })?;

            Ok(QueryReport {
                shape: QueryShape::Graph,
                media_type: formats.graph().media_type(),
                answers,
            })
        }
    }
}

/// Translate an evaluation failure, keeping the deadline distinguishable from everything else.
///
/// The only thing that cancels a query in this build is [`under_deadline`], so `Cancelled` means
/// "it ran out of time" and must be reported as that. Folding it into a generic failure would tell
/// an operator whose query timed out that something went wrong in the store.
fn evaluation_failed(error: QueryEvaluationError) -> StoreError {
    match error {
        QueryEvaluationError::Cancelled => StoreError::QueryTimedOut,
        // Matched on the *variant*, not on the message text. This is the only way a `SERVICE`
        // clause can fail in a build without an HTTP client, and reporting it as a generic
        // evaluation failure — which is what it looked like until a hand-run against the real
        // binary showed a bare 500 — tells a caller that something broke rather than that this
        // deployment has deliberately no federation.
        QueryEvaluationError::UnsupportedService(_) => StoreError::QueryNeedsFederation,
        other => StoreError::QueryFailed {
            detail: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GraphId, SYSTEM_GRAPH_IRI};
    use oxigraph::model::{Literal, Quad};
    use std::time::Instant;
    use tempfile::TempDir;

    const VOCABULARY: &str = "http://acme.example/v/finance";
    const EVERYTHING: &str = "SELECT * WHERE { ?s ?p ?o }";

    /// A store holding a registered vocabulary graph with `statements` quads in it.
    ///
    /// The quads are written through the **backend**, not through a public API, because no public
    /// API can yet put a statement into a vocabulary graph: the store creates the container and
    /// Phase 2's candidate seam fills it. That makes this a fixture rather than a shortcut — but it
    /// is a fixture that must be replaced by the real authoring path the moment one exists, or
    /// these tests will keep passing against a shape production never produces. Recorded in
    /// `docs/UNTESTED.md`.
    fn with_vocabulary(statements: usize) -> (TempDir, Store) {
        let dir = TempDir::new().expect("a temporary data directory");
        let store = Store::open(dir.path()).expect("a fresh store opens");
        store
            .create_vocabulary_graph(&GraphId::vocabulary(VOCABULARY).expect("a valid IRI"))
            .expect("a fresh IRI registers");

        let graph = NamedNode::new(VOCABULARY).expect("a valid IRI");
        let predicate =
            NamedNode::new("http://www.w3.org/2004/02/skos/core#prefLabel").expect("a valid IRI");
        for index in 0..statements {
            let subject = NamedNode::new(format!("{VOCABULARY}/concept/{index}"))
                .expect("a valid concept IRI");
            let quad = Quad::new(
                subject,
                predicate.clone(),
                Literal::new_language_tagged_literal(format!("Concept {index}"), "en")
                    .expect("a valid language tag"),
                graph.clone(),
            );
            store.backend.insert(&quad).expect("a quad is insertable");
        }

        (dir, store)
    }

    /// Evaluate `query` with the defaults and hand back what was written.
    fn answered(store: &Store, query: &str) -> (QueryReport, String) {
        answered_as(
            store,
            query,
            QueryFormats::default(),
            QueryLimits::default(),
        )
    }

    fn answered_as(
        store: &Store,
        query: &str,
        formats: QueryFormats,
        limits: QueryLimits,
    ) -> (QueryReport, String) {
        let mut written = Vec::new();
        let report = store
            .query(query, formats, limits, &mut written)
            .unwrap_or_else(|error| panic!("{query:?} should have been answered: {error}"));
        (
            report,
            String::from_utf8(written).expect("results are UTF-8"),
        )
    }

    fn refused(store: &Store, query: &str, limits: QueryLimits) -> StoreError {
        store
            .query(query, QueryFormats::default(), limits, &mut Vec::new())
            .expect_err("this query should have been refused")
    }

    #[test]
    fn a_select_answers_with_the_vocabularys_own_statements() {
        let (_dir, store) = with_vocabulary(3);
        let (report, written) = answered(&store, EVERYTHING);

        assert_eq!(report.shape(), QueryShape::Solutions);
        assert_eq!(report.answers(), 3);
        assert_eq!(report.media_type(), ResultsSyntax::DEFAULT.media_type());
        assert!(written.contains("Concept 0"), "{written}");
        assert!(written.contains("skos/core#prefLabel"), "{written}");
    }

    /// The whole point of the dataset rule. A store *always* holds OpenBiz's own quads — the
    /// format stamp and the registry — so "zero rows" here is a claim about what was excluded, not
    /// about an empty store. `the_system_graph_is_reachable_when_a_query_asks_for_it_by_name` is
    /// what makes this assertion non-vacuous: it proves those quads are there to be found.
    #[test]
    fn our_own_bookkeeping_never_appears_in_a_query_that_did_not_ask_for_it() {
        let (_dir, store) = with_vocabulary(2);

        for query in [
            EVERYTHING,
            "SELECT * WHERE { GRAPH ?g { ?s ?p ?o } }",
            "SELECT * WHERE { ?s <urn:openbiz:storeFormatVersion> ?o }",
        ] {
            let (_, written) = answered(&store, query);
            assert!(
                !written.contains("urn:openbiz:"),
                "{query:?} leaked OpenBiz's own graph into a user's results: {written}"
            );
        }

        let (report, _) = answered(&store, "SELECT * WHERE { GRAPH ?g { ?s ?p ?o } }");
        assert_eq!(
            report.answers(),
            2,
            "GRAPH ?g must enumerate the vocabulary graphs and only those"
        );
    }

    /// The escape hatch, and the proof that the exclusion above is real rather than an empty store.
    /// SPARQL 1.1 says a query's own dataset clause decides the dataset; we honour that even when
    /// it names one of ours, because an operator asking "what is actually in my store?" is a
    /// question `CLAUDE.md` §1 exists to keep answerable.
    #[test]
    fn the_system_graph_is_reachable_when_a_query_asks_for_it_by_name() {
        let (_dir, store) = with_vocabulary(1);
        let (report, written) = answered(
            &store,
            &format!("SELECT * FROM <{SYSTEM_GRAPH_IRI}> WHERE {{ ?s ?p ?o }}"),
        );

        assert!(
            report.answers() > 0,
            "the system graph holds the format stamp and the registry, so it is never empty"
        );
        assert!(
            written.contains("urn:openbiz:storeFormatVersion"),
            "{written}"
        );
    }

    /// A store with no vocabularies yet answers *nothing*, rather than falling back to everything.
    /// The failure this guards is the one that only appears on a fresh install: an empty default
    /// dataset quietly becoming the union of all graphs, so a brand-new deployment's first query
    /// returns OpenBiz's internals.
    #[test]
    fn a_store_with_no_vocabularies_answers_nothing_rather_than_everything() {
        let dir = TempDir::new().expect("a temporary data directory");
        let store = Store::open(dir.path()).expect("a fresh store opens");

        let (report, written) = answered(&store, EVERYTHING);

        assert_eq!(report.answers(), 0);
        assert!(!written.contains("urn:openbiz:"), "{written}");
    }

    #[test]
    fn an_ask_answers_with_a_boolean_in_every_results_syntax() {
        let (_dir, store) = with_vocabulary(1);

        for syntax in ResultsSyntax::ALL {
            let formats = QueryFormats::new(syntax, RdfSyntax::DEFAULT);
            let (report, written) =
                answered_as(&store, "ASK { ?s ?p ?o }", formats, QueryLimits::default());

            assert_eq!(report.shape(), QueryShape::Boolean);
            assert_eq!(report.media_type(), syntax.media_type());
            assert!(
                written.to_ascii_lowercase().contains("true"),
                "{syntax} wrote {written:?} for an ASK that is true"
            );
        }
    }

    #[test]
    fn a_select_can_be_written_in_every_results_syntax() {
        let (_dir, store) = with_vocabulary(1);

        for syntax in ResultsSyntax::ALL {
            let (report, written) = answered_as(
                &store,
                EVERYTHING,
                QueryFormats::new(syntax, RdfSyntax::DEFAULT),
                QueryLimits::default(),
            );

            assert_eq!(report.shape(), QueryShape::Solutions);
            assert_eq!(report.answers(), 1);
            assert_eq!(report.media_type(), syntax.media_type());
            assert!(
                written.contains("Concept 0"),
                "{syntax} wrote {written:?}, which does not hold the binding"
            );
        }
    }

    #[test]
    fn a_construct_answers_with_rdf_in_every_serialisation() {
        let (_dir, store) = with_vocabulary(2);

        for syntax in RdfSyntax::ALL {
            let (report, written) = answered_as(
                &store,
                "CONSTRUCT { ?s <http://example.org/constructedLabel> ?o } WHERE { ?s ?p ?o }",
                QueryFormats::new(ResultsSyntax::DEFAULT, syntax),
                QueryLimits::default(),
            );

            assert_eq!(report.shape(), QueryShape::Graph);
            assert_eq!(report.answers(), 2);
            assert_eq!(report.media_type(), syntax.media_type());
            // The *local name* rather than the whole IRI, because RDF/XML splits a predicate into
            // a namespace declaration and an element name and so never writes it in one piece.
            assert!(
                written.contains("constructedLabel"),
                "{syntax} wrote {written:?}, which does not hold the constructed predicate"
            );
            assert!(
                !written.contains("skos"),
                "{syntax} wrote the source predicate rather than the constructed one: {written:?}"
            );
        }
    }

    /// A `DESCRIBE` is RDF too, and it is the one query form whose shape is easy to get wrong —
    /// it has no `WHERE` in its simplest form and produces a graph rather than a table.
    #[test]
    fn a_describe_answers_with_rdf() {
        let (_dir, store) = with_vocabulary(2);
        let (report, written) = answered(&store, &format!("DESCRIBE <{VOCABULARY}/concept/0>"));

        assert_eq!(report.shape(), QueryShape::Graph);
        assert_eq!(report.media_type(), RdfSyntax::DEFAULT.media_type());
        assert!(written.contains("Concept 0"), "{written}");
    }

    /// An update refused as a *syntax error* sends somebody hunting for a typo in text that has
    /// none. Recognised by parsing it as an update rather than by sniffing for a keyword, so a
    /// query with the word `INSERT` in a literal is not caught by it.
    #[test]
    fn a_sparql_update_is_refused_as_an_update_rather_than_as_a_syntax_error() {
        let (_dir, store) = with_vocabulary(0);

        for update in [
            "INSERT DATA { <http://a.example/s> <http://a.example/p> \"o\" }",
            "DELETE WHERE { ?s ?p ?o }",
            "CLEAR ALL",
            &format!("DROP GRAPH <{SYSTEM_GRAPH_IRI}>"),
        ] {
            assert!(
                matches!(
                    refused(&store, update, QueryLimits::default()),
                    StoreError::QueryIsUpdate
                ),
                "{update:?} was not recognised as an update"
            );
        }
    }

    /// The keyword-sniffing failure the parse-it-twice approach avoids: a query that merely
    /// *mentions* an update keyword is an ordinary query and must be answered.
    #[test]
    fn a_query_that_merely_mentions_an_update_keyword_is_still_a_query() {
        let (_dir, store) = with_vocabulary(1);
        let (report, _) = answered(
            &store,
            "SELECT * WHERE { ?s ?p ?o . FILTER(?o != \"INSERT DATA { }\") }",
        );

        assert_eq!(report.shape(), QueryShape::Solutions);
        assert_eq!(report.answers(), 1);
    }

    #[test]
    fn malformed_sparql_is_refused_with_the_parsers_own_words() {
        let (_dir, store) = with_vocabulary(0);

        let StoreError::QuerySyntax { detail } =
            refused(&store, "SELECT ?s WHERE {", QueryLimits::default())
        else {
            panic!("an unterminated query is a syntax error");
        };
        assert!(
            !detail.is_empty(),
            "a syntax refusal with no detail leaves the caller guessing"
        );
    }

    /// Refusing rather than truncating is the point. A caller handed the first `limit` rows of a
    /// larger answer has a document that is valid, complete-looking, and missing exactly the rows
    /// they have no way to know about.
    #[test]
    fn an_answer_past_the_cap_is_refused_rather_than_truncated() {
        let (_dir, store) = with_vocabulary(20);
        let limits = QueryLimits::new(5, QueryLimits::DEFAULT_TIMEOUT);

        assert!(
            matches!(
                refused(&store, EVERYTHING, limits),
                StoreError::QueryTooLarge { limit: 5 }
            ),
            "20 statements past a cap of 5 must be a refusal"
        );

        // Exactly at the cap is not past it.
        let (report, _) = answered_as(
            &store,
            "SELECT * WHERE { ?s ?p ?o } LIMIT 5",
            QueryFormats::default(),
            limits,
        );
        assert_eq!(report.answers(), 5);
    }

    /// The cap counts constructed triples too, not only solutions — a `CONSTRUCT` is just as
    /// capable of producing an unbounded answer.
    #[test]
    fn the_cap_applies_to_constructed_triples_as_well() {
        let (_dir, store) = with_vocabulary(20);

        assert!(matches!(
            refused(
                &store,
                "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }",
                QueryLimits::new(5, QueryLimits::DEFAULT_TIMEOUT),
            ),
            StoreError::QueryTooLarge { limit: 5 }
        ));
    }

    /// A runaway query must be stopped by the *deadline*, not by anything else.
    ///
    /// Shaped so a regression **fails rather than hangs**, which two earlier tests in this
    /// codebase do not manage: the query is an aggregate, so it produces exactly one solution and
    /// the answer cap can never fire, and the work it does is finite — a three-way join over 60
    /// statements, 216,000 tuples. If cancellation stopped working this test would grind through
    /// them and then fail the assertion in a few seconds, rather than blocking the suite forever.
    #[test]
    fn a_runaway_query_is_stopped_by_the_deadline() {
        let (_dir, store) = with_vocabulary(60);
        let limits = QueryLimits::new(u64::MAX, Duration::from_millis(50));

        let started = Instant::now();
        let error = refused(
            &store,
            "SELECT (COUNT(*) AS ?n) WHERE { ?a ?b ?c . ?d ?e ?f . ?g ?h ?i }",
            limits,
        );
        let elapsed = started.elapsed();

        assert!(
            matches!(error, StoreError::QueryTimedOut),
            "the deadline must be the thing that stopped it, not {error}"
        );
        assert!(
            elapsed < Duration::from_secs(10),
            "cancellation took {elapsed:?}, which is not a deadline"
        );
    }

    /// A query that finishes well inside its deadline must not be reported as having timed out,
    /// and the watchdog must not outlive it. Run many times over, because a watchdog that
    /// cancelled a *later* query would show up as an intermittent failure and nothing else.
    #[test]
    fn a_quick_query_is_never_cancelled_by_its_own_watchdog() {
        let (_dir, store) = with_vocabulary(1);
        let limits = QueryLimits::new(u64::MAX, Duration::from_millis(30));

        for _ in 0..40 {
            let (report, _) = answered_as(&store, EVERYTHING, QueryFormats::default(), limits);
            assert_eq!(report.answers(), 1);
        }
    }

    /// `CLAUDE.md` §1.1: the product must run air-gapped, and `docs/adr/0006` makes that true by
    /// building Oxigraph without its HTTP client rather than by asking callers not to federate. A
    /// `SERVICE` clause therefore fails *inside the evaluator* — it never resolves a host, opens a
    /// socket, or leaks the fact that this deployment exists to whoever owns the endpoint IRI.
    #[test]
    fn a_federated_query_cannot_reach_the_network() {
        let (_dir, store) = with_vocabulary(1);

        let error = refused(
            &store,
            "SELECT * WHERE { SERVICE <http://sparql.example.invalid/> { ?s ?p ?o } }",
            QueryLimits::default(),
        );

        assert!(
            matches!(error, StoreError::QueryNeedsFederation),
            "a SERVICE clause must be reported as a capability this build does not have, not as \
             {error}"
        );
        assert!(
            error.to_string().contains("air-gapped"),
            "the refusal must say why federation is absent: {error}"
        );
    }

    #[test]
    fn the_limits_have_the_documented_defaults() {
        let limits = QueryLimits::default();

        assert_eq!(limits.max_answers(), QueryLimits::DEFAULT_MAX_ANSWERS);
        assert_eq!(limits.timeout(), QueryLimits::DEFAULT_TIMEOUT);
        assert_eq!(
            QueryFormats::default(),
            QueryFormats::new(ResultsSyntax::DEFAULT, RdfSyntax::DEFAULT)
        );
    }

    #[test]
    fn a_shape_knows_which_family_of_syntax_writes_it() {
        assert!(QueryShape::Solutions.is_results());
        assert!(QueryShape::Boolean.is_results());
        assert!(!QueryShape::Graph.is_results());
    }
}
