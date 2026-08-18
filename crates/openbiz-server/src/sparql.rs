//! `/api/sparql` — the SPARQL 1.1 Query endpoint.
//!
//! # What this implements
//!
//! All three of [SPARQL 1.1 Protocol]'s request forms for a query:
//!
//! | Form | How the query arrives |
//! |---|---|
//! | `GET /api/sparql?query=…` | URL query string |
//! | `POST` with `application/sparql-query` | the request body *is* the query |
//! | `POST` with `application/x-www-form-urlencoded` | a `query=` field in the body |
//!
//! All four result formats SPARQL 1.1 defines for `SELECT` and `ASK` — JSON, XML, CSV, TSV — and
//! all six RDF serialisations for `CONSTRUCT` and `DESCRIBE`, negotiated from one `Accept` header
//! or named outright with `?format=`.
//!
//! # What it deliberately does not implement, and says so
//!
//! The protocol's `default-graph-uri` and `named-graph-uri` parameters are **refused**, not
//! ignored. A caller who sends one and is silently given a different dataset from the one they
//! asked for gets an answer that is wrong in the way that is hardest to notice — it is well-formed,
//! plausible, and about the wrong graphs. So is any other unrecognised parameter: `?formt=csv` is
//! a 400 naming the parameter, rather than a Turtle document the caller did not ask for. This is
//! the same rule the export endpoint applies to `?format=turtel`.
//!
//! # Read-only, by name
//!
//! There is no update endpoint in this build. Text that parses as a SPARQL Update is refused as
//! *an update*, not as a syntax error — see [`openbiz_store::Store::query`], which recognises it by
//! parsing it rather than by sniffing for a keyword.
//!
//! # Better, not parity
//!
//! Every tool in this market has a SPARQL endpoint. Three things they do badly are what this one is
//! actually about.
//!
//! 1. **The default dataset is a trap.** Point a taxonomist at PoolParty's or GraphDB's endpoint and
//!    `SELECT * WHERE { ?s ?p ?o }` returns the union of everything, including the tool's own
//!    bookkeeping graphs, unlabelled and interleaved with their vocabulary. Here the default dataset
//!    is the registered *vocabulary* graphs and nothing else, the rule is written down, and the
//!    graphs it covers are exactly what `GET /api/graphs` already reports as `kind: "vocabulary"`.
//!    A query that names its own `FROM` is honoured verbatim, so nothing is hidden — the default is
//!    chosen rather than imposed.
//! 2. **A runaway query is the caller's problem to notice.** Endpoints either run until something
//!    falls over or truncate at a row cap and return the truncation as if it were the answer. Here
//!    both bounds **refuse**: exceeding them is a status code and a message, never a short document
//!    that looks complete. A governance team cannot sign off rows they were never told were missing.
//! 3. **`Accept` is advisory.** Ask a typical endpoint for `text/turtle` and send it a `SELECT` and
//!    you will get JSON with a `Content-Type` that says so and no acknowledgement that the
//!    negotiation failed. Here that is a 406 naming what the query actually produced.
//!
//! [SPARQL 1.1 Protocol]: https://www.w3.org/TR/sparql11-protocol/

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{RawQuery, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use openbiz_api::{ResultsFormat, ResultsFormats};
use openbiz_store::{
    QueryFormats, QueryLimits, QueryReport, QueryShape, RdfSyntax, ResultsSyntax, Store, StoreError,
};

use crate::accept;
use crate::graphs::{AppState, Failure};

/// The media type a SPARQL 1.1 Protocol direct `POST` carries.
const SPARQL_QUERY: &str = "application/sparql-query";

/// The media type a SPARQL 1.1 Protocol form-encoded `POST` carries.
const FORM_ENCODED: &str = "application/x-www-form-urlencoded";

/// Protocol parameters this build understands well enough to refuse precisely.
///
/// They select the dataset a query runs over. Implementing them means deciding how a
/// protocol-supplied dataset interacts with the vocabulary-graph default and with a query's own
/// `FROM` — a decision worth making deliberately rather than in passing, so until then they are a
/// named refusal. Recorded in `docs/UNTESTED.md`.
const DATASET_PARAMETERS: [&str; 2] = ["default-graph-uri", "named-graph-uri"];

/// `GET /api/sparql/formats` — every query-results serialisation this build can write.
///
/// The sibling of `GET /api/export/formats`, and served rather than duplicated in the frontend for
/// the same reason: the UI and the server ship in one binary, so a divergence between what the
/// interface offers and what the server writes would be caught by a user picking a format and
/// getting a 400, not by a type check.
///
/// This lists only the **results** formats — what a `SELECT` or an `ASK` answers in. A `CONSTRUCT`
/// or `DESCRIBE` answers with RDF, and those formats are already served by `/api/export/formats`
/// from the same constants. Two lists rather than one merged list, because which one applies is
/// decided by the query, and a caller that cannot tell them apart is exactly the caller who sends
/// `?format=csv` with a `CONSTRUCT`.
///
/// `preservesTermDetail` is why this endpoint exists rather than being a hard-coded array in the
/// frontend: it comes from the same constant the serialiser branches on, so the interface cannot
/// warn about CSV's silent loss of language tags while the writer does something else.
pub(crate) async fn formats() -> Json<ResultsFormats> {
    Json(ResultsFormats {
        formats: ResultsSyntax::ALL
            .into_iter()
            .map(|syntax| ResultsFormat {
                token: syntax.token().to_owned(),
                label: syntax.label().to_owned(),
                media_type: syntax.media_type().to_owned(),
                file_extension: syntax.file_extension().to_owned(),
                preserves_term_detail: syntax.preserves_term_detail(),
            })
            .collect(),
    })
}

/// `GET /api/sparql?query=…` — evaluate a query named in the URL.
pub(crate) async fn query_get(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Failure> {
    let parameters = Parameters::parse(raw.as_deref().unwrap_or_default())?;
    let Some(query) = parameters.query else {
        return Err(Failure::new(
            StatusCode::BAD_REQUEST,
            "name the query to evaluate: /api/sparql?query=<SPARQL>. A query may also be POSTed, \
             as application/sparql-query or as a form field",
        ));
    };

    answer(state.store(), query, parameters.format, &headers).await
}

/// `POST /api/sparql` — evaluate a query carried in the body, in either protocol form.
pub(crate) async fn query_post(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, Failure> {
    let from_url = Parameters::parse(raw.as_deref().unwrap_or_default())?;

    let (query, format) = match media_type(&headers) {
        Some(SPARQL_QUERY) => {
            if from_url.query.is_some() {
                return Err(Failure::new(
                    StatusCode::BAD_REQUEST,
                    "the query is in the body of an application/sparql-query request, so ?query= \
                     in the URL is a second, contradictory query; send one or the other",
                ));
            }
            (text(&body)?, from_url.format)
        }
        Some(FORM_ENCODED) => {
            let from_body = Parameters::parse(&text(&body)?)?;
            let Some(query) = from_body.query else {
                return Err(Failure::new(
                    StatusCode::BAD_REQUEST,
                    "a form-encoded SPARQL request carries the query in a query= field",
                ));
            };
            (query, from_body.format.or(from_url.format))
        }
        Some(other) => {
            return Err(Failure::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!(
                    "{other:?} is not a way to send a SPARQL query; use {SPARQL_QUERY} with the \
                     query as the body, or {FORM_ENCODED} with a query= field"
                ),
            ))
        }
        None => {
            return Err(Failure::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!(
                    "a POSTed SPARQL query needs a Content-Type: {SPARQL_QUERY} with the query as \
                     the body, or {FORM_ENCODED} with a query= field"
                ),
            ))
        }
    };

    answer(state.store(), query, format, &headers).await
}

/// Evaluate `query` and render whatever it answered with.
async fn answer(
    store: Arc<Store>,
    query: String,
    format: Option<String>,
    headers: &HeaderMap,
) -> Result<Response, Failure> {
    let wanted = match &format {
        Some(named) => Acceptable::named(named)?,
        None => Acceptable::negotiated(headers)?,
    };
    let limits = QueryLimits::default();

    let (report, body) = evaluate(store, query, wanted.formats(), limits).await?;

    // Checked after evaluation rather than before, because the shape of an answer is a property of
    // the query and is not known until it has been parsed and run. The cost is doing work for a
    // response that is then refused, which happens only when a caller's `Accept` and their query
    // disagree — rare, and much cheaper than the alternative of answering in a format they told us
    // they cannot read.
    if !wanted.accepts(report.shape()) {
        return Err(Failure::new(
            StatusCode::NOT_ACCEPTABLE,
            wanted.mismatch(report.shape()),
        ));
    }

    tracing::debug!(
        shape = ?report.shape(),
        answers = report.answers(),
        media_type = report.media_type(),
        "a SPARQL query was answered"
    );

    Ok((
        [(
            header::CONTENT_TYPE,
            format!("{}; charset=utf-8", report.media_type()),
        )],
        body,
    )
        .into_response())
}

/// Run the query off the async runtime, and buffer its answer.
///
/// Evaluating SPARQL is a blocking RocksDB scan: on an async worker it would stall every other
/// request the runtime has. The answer is buffered — and *only* turned into a response on `Ok` —
/// because [`Store::query`] may leave a partial document behind when it refuses. A partial results
/// document is syntactically valid and semantically wrong, so it must never reach a client; that
/// is the whole reason this is a `Vec` rather than a streaming body.
async fn evaluate(
    store: Arc<Store>,
    query: String,
    formats: QueryFormats,
    limits: QueryLimits,
) -> Result<(QueryReport, Vec<u8>), Failure> {
    let answered = tokio::task::spawn_blocking(move || {
        let mut bytes = Vec::new();
        store
            .query(&query, formats, limits, &mut bytes)
            .map(|report| (report, bytes))
    })
    .await;

    match answered {
        Ok(Ok(answer)) => Ok(answer),
        Ok(Err(error)) => Err(refused(error, limits)),
        Err(error) => {
            tracing::error!(%error, "the query task did not finish");
            Err(Failure::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the query could not be evaluated; the server log records why",
            ))
        }
    }
}

/// Turn a store refusal into the status and the words a caller can act on.
///
/// The three refusals a well-formed request can meet are separated, because the fix differs. A
/// syntax error is a typo; an update is the wrong endpoint; a limit is a query that has to be made
/// cheaper. Collapsing them into one 400 would leave a caller guessing which.
///
/// **On the timeout's status code, honestly:** RFC 9110 has no code for "the server cancelled a
/// valid request against its own resource policy". 408 is about a slow *request*, 504 is about an
/// upstream server, and 500 would claim something went wrong when nothing did. 503 is the
/// least-wrong of a bad set, and the body carries the part that is actually actionable. The cost
/// is real and recorded in `docs/adr/0011`: a load balancer reading 503 may take the instance out
/// of rotation over one expensive query.
fn refused(error: StoreError, limits: QueryLimits) -> Failure {
    match error {
        StoreError::QuerySyntax { detail } => Failure::new(
            StatusCode::BAD_REQUEST,
            // The detail is about the caller's own query text — not the customer's data and not
            // their deployment — so it goes in the response. Withholding it here would leave
            // somebody hunting a typo they cannot see.
            format!("that is not a valid SPARQL 1.1 query: {detail}"),
        ),
        StoreError::QueryIsUpdate => Failure::new(
            StatusCode::BAD_REQUEST,
            "that is a SPARQL Update, not a query. /api/sparql evaluates queries and never writes; \
             this build exposes no update endpoint",
        ),
        StoreError::QueryTooLarge { limit } => Failure::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "the query answered with more than {limit} results, so it was refused rather than \
                 truncated — a partial answer that looks complete is worse than no answer. Add a \
                 LIMIT, or narrow the query"
            ),
        ),
        StoreError::QueryNeedsFederation => Failure::new(
            StatusCode::NOT_IMPLEMENTED,
            "this build has no SPARQL 1.1 Federated Query: it is compiled without an HTTP client \
             so that it can run air-gapped, so a SERVICE clause cannot be evaluated and nothing \
             was sent to the named endpoint",
        ),
        StoreError::QueryTimedOut => Failure::new(
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "the query ran for longer than {} seconds and was cancelled. Nothing was written; \
                 narrow the query or add a LIMIT",
                limits.timeout().as_secs()
            ),
        ),
        error => {
            tracing::error!(%error, "a SPARQL query failed");
            Failure::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the query could not be evaluated; the server log records why",
            )
        }
    }
}

/// The syntaxes a caller will accept, in each of the two families.
///
/// Two families because one `Accept` header has to answer two different questions — how to write a
/// table of solutions, and how to write RDF — and which of them applies is decided by the query,
/// not by the caller. Keeping both as `Option` is what lets the endpoint tell a caller *which*
/// preference it could not meet, instead of substituting silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Acceptable {
    solutions: Option<ResultsSyntax>,
    graph: Option<RdfSyntax>,
    /// The caller accepts anything, so an unmatched family falls back to its default.
    anything: bool,
}

impl Acceptable {
    /// No preference expressed: both families get their default.
    const ANY: Self = Self {
        solutions: None,
        graph: None,
        anything: true,
    };

    /// A syntax named outright with `?format=`.
    ///
    /// Binding for its own family and unacceptable for the other, which is the honest reading:
    /// somebody who wrote `?format=csv` and sent a `CONSTRUCT` asked for something this query
    /// cannot produce, and handing them Turtle labelled as Turtle would still not be what they
    /// asked for.
    fn named(requested: &str) -> Result<Self, Failure> {
        if let Some(solutions) = ResultsSyntax::parse(requested) {
            return Ok(Self {
                solutions: Some(solutions),
                graph: None,
                anything: false,
            });
        }
        if let Some(graph) = RdfSyntax::parse(requested) {
            return Ok(Self {
                solutions: None,
                graph: Some(graph),
                anything: false,
            });
        }

        Err(Failure::new(
            StatusCode::BAD_REQUEST,
            format!(
                "{requested:?} is not a format OpenBiz can write; {}",
                offered()
            ),
        ))
    }

    /// The best match in each family, read from `Accept`.
    fn negotiated(headers: &HeaderMap) -> Result<Self, Failure> {
        let Some(accept) = accept::header(headers) else {
            return Ok(Self::ANY);
        };
        let ranked = accept::preferences(accept);
        if ranked.is_empty() {
            return Ok(Self::ANY);
        }

        let mut wanted = Self {
            solutions: None,
            graph: None,
            anything: false,
        };
        for range in ranked {
            if range == accept::ANYTHING {
                // Ranked above everything still unmatched, so whatever is left takes its default.
                wanted.anything = true;
                break;
            }
            wanted.solutions = wanted.solutions.or_else(|| ResultsSyntax::parse(range));
            wanted.graph = wanted.graph.or_else(|| RdfSyntax::parse(range));
        }

        if wanted.solutions.is_none() && wanted.graph.is_none() && !wanted.anything {
            return Err(Failure::new(
                StatusCode::NOT_ACCEPTABLE,
                format!("OpenBiz cannot write {accept:?}; {}", offered()),
            ));
        }

        Ok(wanted)
    }

    /// What to hand [`Store::query`]: the caller's choice where they made one, the default where
    /// they did not.
    fn formats(self) -> QueryFormats {
        QueryFormats::new(
            self.solutions.unwrap_or(ResultsSyntax::DEFAULT),
            self.graph.unwrap_or(RdfSyntax::DEFAULT),
        )
    }

    /// Whether an answer of this shape is one the caller said they could read.
    fn accepts(self, shape: QueryShape) -> bool {
        if self.anything {
            return true;
        }
        if shape.is_results() {
            self.solutions.is_some()
        } else {
            self.graph.is_some()
        }
    }

    /// Why an answer could not be given in a form the caller accepts.
    fn mismatch(self, shape: QueryShape) -> String {
        let produced = match shape {
            QueryShape::Solutions => "a table of solutions",
            QueryShape::Boolean => "a boolean",
            QueryShape::Graph => "RDF",
        };
        let family = if shape.is_results() {
            names(ResultsSyntax::ALL.iter().map(|it| it.media_type()))
        } else {
            names(RdfSyntax::ALL.iter().map(|it| it.media_type()))
        };

        format!(
            "the query answered with {produced}, which OpenBiz writes as one of {family} — and \
             none of those is a format this request accepts"
        )
    }
}

/// Every format either family can be asked for, for a refusal that says what to do instead.
fn offered() -> String {
    format!(
        "SELECT and ASK answer in one of {}, and CONSTRUCT and DESCRIBE in one of {}",
        names(ResultsSyntax::ALL.iter().map(|it| it.token())),
        names(RdfSyntax::ALL.iter().map(|it| it.token())),
    )
}

fn names<'a>(items: impl Iterator<Item = &'a str>) -> String {
    items.collect::<Vec<_>>().join(", ")
}

/// The request's media type, without parameters and lower-cased.
fn media_type(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::CONTENT_TYPE)?.to_str().ok()?;
    let value = value.split(';').next().unwrap_or(value).trim();
    // Matched case-sensitively against our two constants after this, so only the exact spellings
    // the protocol defines are accepted. `to_str` already guarantees ASCII.
    Some(value)
}

/// A request body as text.
fn text(body: &Bytes) -> Result<String, Failure> {
    String::from_utf8(body.to_vec()).map_err(|_| {
        Failure::new(
            StatusCode::BAD_REQUEST,
            "the request body is not valid UTF-8; SPARQL is defined over Unicode text",
        )
    })
}

/// The parameters this endpoint understands, read from a URL query string or a form body.
///
/// Both are decoded by the same decoder, so a `+`, a `%`, or a repeated key cannot mean one thing
/// in a URL and another in a form.
#[derive(Debug, Default, PartialEq, Eq)]
struct Parameters {
    query: Option<String>,
    format: Option<String>,
}

impl Parameters {
    /// Read `raw`, refusing anything that is not a parameter this endpoint acts on.
    ///
    /// Refusing rather than ignoring is the whole point. An ignored `?formt=csv` is a document in
    /// the wrong format with a correct-looking `Content-Type`; an ignored `?default-graph-uri=` is
    /// an answer about the wrong graphs. Both are wrong in the way a caller checks for last.
    fn parse(raw: &str) -> Result<Self, Failure> {
        let pairs: Vec<(String, String)> = serde_urlencoded::from_str(raw).map_err(|error| {
            Failure::new(
                StatusCode::BAD_REQUEST,
                format!("the parameters could not be decoded: {error}"),
            )
        })?;

        let mut parameters = Self::default();
        for (key, value) in pairs {
            let slot =
                match key.as_str() {
                    "query" => &mut parameters.query,
                    "format" => &mut parameters.format,
                    "update" => return Err(Failure::new(
                        StatusCode::BAD_REQUEST,
                        "/api/sparql evaluates queries and never writes; this build exposes no \
                         update endpoint",
                    )),
                    other if DATASET_PARAMETERS.contains(&other) => {
                        return Err(Failure::new(
                            StatusCode::BAD_REQUEST,
                            format!(
                            "{other:?} is not implemented, and is refused rather than ignored so \
                             that no answer is ever about a different dataset from the one asked \
                             for. A query names its own dataset with FROM and FROM NAMED; \
                             otherwise it runs over the registered vocabulary graphs, which GET \
                             /api/graphs lists"
                        ),
                        ))
                    }
                    other => {
                        return Err(Failure::new(
                            StatusCode::BAD_REQUEST,
                            format!(
                            "{other:?} is not a parameter /api/sparql understands; it takes query \
                             and format"
                        ),
                        ))
                    }
                };

            if slot.is_some() {
                return Err(Failure::new(
                    StatusCode::BAD_REQUEST,
                    format!(
                        "{key:?} was given more than once, and guessing which one was meant is \
                         how a caller gets an answer to a question they did not ask"
                    ),
                ));
            }
            *slot = Some(value);
        }

        Ok(parameters)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn accepting(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCEPT,
            HeaderValue::from_str(value).expect("a valid header value"),
        );
        headers
    }

    #[test]
    fn no_accept_header_takes_the_defaults_for_both_families() {
        let wanted = Acceptable::negotiated(&HeaderMap::new()).expect("no preference");

        assert_eq!(wanted, Acceptable::ANY);
        assert_eq!(wanted.formats(), QueryFormats::default());
        for shape in [
            QueryShape::Solutions,
            QueryShape::Boolean,
            QueryShape::Graph,
        ] {
            assert!(wanted.accepts(shape));
        }
    }

    #[test]
    fn each_family_is_negotiated_independently_from_one_header() {
        let wanted = Acceptable::negotiated(&accepting(
            "application/sparql-results+xml, application/trig",
        ))
        .expect("both families matched");

        assert_eq!(
            wanted.formats(),
            QueryFormats::new(ResultsSyntax::Xml, RdfSyntax::TriG)
        );
        assert!(wanted.accepts(QueryShape::Solutions));
        assert!(wanted.accepts(QueryShape::Graph));
    }

    /// A caller who asked only for RDF and sent a `SELECT` is told so, rather than handed JSON
    /// under a `Content-Type` that says JSON and an `Accept` that said otherwise.
    #[test]
    fn a_family_the_caller_did_not_ask_for_is_not_acceptable() {
        let wanted = Acceptable::negotiated(&accepting("text/turtle")).expect("one family matched");

        assert!(wanted.accepts(QueryShape::Graph));
        assert!(!wanted.accepts(QueryShape::Solutions));
        assert!(!wanted.accepts(QueryShape::Boolean));
        assert!(
            wanted.mismatch(QueryShape::Solutions).contains("csv"),
            "a refusal must say what it could have written: {}",
            wanted.mismatch(QueryShape::Solutions)
        );
    }

    /// A browser's header. The wildcard is what makes it satisfiable, and it must not be read as a
    /// request for SPARQL Results XML just because `application/xml` is in the list.
    #[test]
    fn a_browsers_accept_header_takes_the_defaults() {
        let wanted =
            Acceptable::negotiated(&accepting("text/html,application/xml;q=0.9,*/*;q=0.8"))
                .expect("the wildcard makes it satisfiable");

        assert!(wanted.anything);
        assert_eq!(wanted.formats(), QueryFormats::default());
        assert!(wanted.accepts(QueryShape::Solutions));
    }

    #[test]
    fn an_accept_matching_neither_family_is_refused_rather_than_substituted() {
        let failure = Acceptable::negotiated(&accepting("text/html")).expect_err("406");

        assert_eq!(failure.status(), StatusCode::NOT_ACCEPTABLE);
        assert!(
            failure.message().contains("json") && failure.message().contains("turtle"),
            "a refusal must name both families: {}",
            failure.message()
        );
    }

    #[test]
    fn an_explicit_format_binds_its_own_family_and_no_other() {
        let csv = Acceptable::named("csv").expect("a results syntax");
        assert_eq!(csv.formats().solutions(), ResultsSyntax::Csv);
        assert!(csv.accepts(QueryShape::Solutions));
        assert!(!csv.accepts(QueryShape::Graph));

        let trig = Acceptable::named("trig").expect("an RDF syntax");
        assert_eq!(trig.formats().graph(), RdfSyntax::TriG);
        assert!(trig.accepts(QueryShape::Graph));
        assert!(!trig.accepts(QueryShape::Solutions));
    }

    #[test]
    fn an_unknown_format_is_refused_and_the_refusal_lists_both_families() {
        let failure = Acceptable::named("jsno").expect_err("400");

        assert_eq!(failure.status(), StatusCode::BAD_REQUEST);
        assert!(failure.message().contains("json"));
        assert!(failure.message().contains("turtle"));
    }

    #[test]
    fn parameters_are_decoded_the_same_way_a_form_body_is() {
        let parsed = Parameters::parse("query=SELECT+%2A+WHERE+%7B%3Fs+%3Fp+%3Fo%7D&format=tsv")
            .expect("valid parameters");

        assert_eq!(
            parsed,
            Parameters {
                query: Some("SELECT * WHERE {?s ?p ?o}".to_owned()),
                format: Some("tsv".to_owned()),
            }
        );
    }

    /// The failure this guards is the one nobody checks for: a mistyped parameter silently
    /// dropped, and an answer in a format the caller did not choose.
    #[test]
    fn an_unrecognised_parameter_is_refused_by_name() {
        let failure = Parameters::parse("query=ASK+%7B%7D&formt=csv").expect_err("400");

        assert_eq!(failure.status(), StatusCode::BAD_REQUEST);
        assert!(
            failure.message().contains("formt"),
            "the refusal must name the parameter: {}",
            failure.message()
        );
    }

    /// Refused rather than ignored, because ignoring it answers a question about the wrong graphs.
    #[test]
    fn a_dataset_parameter_is_refused_and_the_refusal_explains_the_dataset() {
        for parameter in DATASET_PARAMETERS {
            let failure =
                Parameters::parse(&format!("query=ASK+%7B%7D&{parameter}=http%3A%2F%2Fa"))
                    .expect_err("400");

            assert_eq!(failure.status(), StatusCode::BAD_REQUEST);
            assert!(failure.message().contains(parameter));
            assert!(
                failure.message().contains("FROM"),
                "the refusal must say how to name a dataset instead: {}",
                failure.message()
            );
        }
    }

    #[test]
    fn an_update_parameter_is_refused_as_a_write() {
        let failure = Parameters::parse("update=CLEAR+ALL").expect_err("400");

        assert_eq!(failure.status(), StatusCode::BAD_REQUEST);
        assert!(failure.message().contains("never writes"));
    }

    #[test]
    fn a_repeated_parameter_is_refused_rather_than_resolved() {
        let failure = Parameters::parse("query=ASK+%7B%7D&query=ASK+%7B%7D").expect_err("400");

        assert_eq!(failure.status(), StatusCode::BAD_REQUEST);
        assert!(failure.message().contains("more than once"));
    }

    #[test]
    fn an_empty_query_string_is_no_parameters_rather_than_an_error() {
        assert_eq!(
            Parameters::parse("").expect("nothing to read"),
            Parameters::default()
        );
    }

    #[test]
    fn a_content_type_is_read_without_its_parameters() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/sparql-query; charset=utf-8"),
        );

        assert_eq!(media_type(&headers), Some(SPARQL_QUERY));
        assert_eq!(media_type(&HeaderMap::new()), None);
    }

    /// SPARQL is defined over Unicode text, so a body that is not UTF-8 is a client error rather
    /// than something to lossily decode into a query that means something else.
    #[test]
    fn a_body_that_is_not_utf8_is_refused() {
        let failure = text(&Bytes::from_static(&[0xff, 0xfe])).expect_err("400");

        assert_eq!(failure.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn a_timeout_refusal_names_the_deadline_and_says_nothing_was_written() {
        let limits = QueryLimits::new(10, std::time::Duration::from_secs(7));
        let failure = refused(StoreError::QueryTimedOut, limits);

        assert_eq!(failure.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(failure.message().contains('7'));
        assert!(failure.message().contains("Nothing was written"));
    }

    #[test]
    fn a_too_large_refusal_names_the_limit_and_refuses_to_truncate() {
        let failure = refused(
            StoreError::QueryTooLarge { limit: 42 },
            QueryLimits::default(),
        );

        assert_eq!(failure.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert!(failure.message().contains("42"));
        assert!(failure.message().contains("truncated"));
    }

    /// A store failure that is ours rather than the caller's must not put the store's own words —
    /// which name the customer's paths and IRIs — into an unauthenticated response.
    #[test]
    fn an_internal_failure_says_nothing_about_the_deployment() {
        let failure = refused(
            StoreError::QueryFailed {
                detail: "graph http://acme.example/secret-project is unreadable".to_owned(),
            },
            QueryLimits::default(),
        );

        assert_eq!(failure.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!failure.message().contains("acme.example"));
    }
}
