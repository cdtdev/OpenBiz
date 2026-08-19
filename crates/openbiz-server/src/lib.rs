//! The OpenBiz server: routing, configuration, and composition of the engine crates.
//!
//! This is the composition root. It will be touched by nearly every feature, which is expected —
//! `/openbiz-status` treats frequent commits here as healthy growth so long as they add distinct
//! things rather than repair the same mechanism.

use axum::{routing::get, Json, Router};
use openbiz_api::Health;

mod accept;
mod ancestors;
mod cli;
mod config;
mod export;
mod graphs;
mod inspect;
mod notes;
mod shutdown;
mod sparql;
mod ui;

pub use ancestors::ancestors;
pub use cli::{
    back_up, candidates, decide, import, restore, retract, show, ArgsError, Command, CommandError,
    ACTOR_VARIABLE, USAGE,
};
pub use config::{Config, ConfigError, Setting, Source};
pub use graphs::AppState;
pub use inspect::inspect;
pub use notes::notes;
pub use shutdown::StopSignals;

/// Build the application router.
///
/// Takes the state it needs rather than reaching for a global: the store is opened by `main`
/// before the listener binds, so a router that could be built without one would be a router that
/// can exist before the thing it serves.
///
/// Everything the API does not claim falls through to the embedded UI (see [`ui`]), which is what
/// makes the single binary in `CLAUDE.md` §1 a fact rather than an intention. Using a
/// `MethodRouter` as the fallback service — rather than a bare handler — keeps writes to unknown
/// paths a 405 instead of quietly answering a POST with the HTML shell.
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/graphs", get(graphs::list))
        .route("/api/export", get(export::export))
        .route("/api/export/formats", get(export::formats))
        .route(
            "/api/sparql",
            get(sparql::query_get).post(sparql::query_post),
        )
        .route("/api/sparql/formats", get(sparql::formats))
        .fallback_service(get(ui::serve))
        .with_state(state)
}

/// A router over a store that lives for the whole test binary.
///
/// Shared, because opening a RocksDB store costs real time and the tests that use this — `ui`'s —
/// never touch it; they assert on the *fallback*, and what they need is the real [`app`] router
/// rather than a hand-built imitation that could drift from it. Tests that write to the registry
/// build their own store instead, so nothing here depends on test ordering.
#[cfg(test)]
pub(crate) fn test_app() -> Router {
    use std::sync::{Arc, OnceLock};

    // Never dropped, which is the point: dropping the `TempDir` would delete the store while a
    // later test is still using it.
    static SHARED: OnceLock<(tempfile::TempDir, Arc<openbiz_store::Store>)> = OnceLock::new();

    let (_dir, store) = SHARED.get_or_init(|| {
        let dir = tempfile::TempDir::new().expect("a temporary data directory");
        let store = Arc::new(openbiz_store::Store::open(dir.path()).expect("a fresh store opens"));
        (dir, store)
    });

    app(AppState::new(Arc::clone(store)))
}

/// Liveness and readiness probe.
async fn healthz() -> Json<Health> {
    Json(Health::ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use openbiz_api::{GraphKind, GraphList};
    use openbiz_store::{GraphId, Store, SYSTEM_GRAPH_IRI};
    use std::sync::Arc;
    use tempfile::TempDir;
    use tower::ServiceExt;

    /// A router over a real store in a throwaway directory. The store is real rather than a
    /// double because the thing under test is the seam between HTTP and the registry, and a fake
    /// registry would assert that our own mock behaves.
    fn with_store() -> (TempDir, Arc<Store>, Router) {
        let dir = TempDir::new().expect("a temporary data directory");
        let store = Arc::new(Store::open(dir.path()).expect("a fresh store opens"));
        let router = app(AppState::new(Arc::clone(&store)));
        (dir, store, router)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(
        router: Router,
        uri: &str,
    ) -> (StatusCode, T) {
        let response = router
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&body).expect("a JSON body of the expected shape"),
        )
    }

    #[tokio::test]
    async fn healthz_reports_ok() {
        let (_dir, _store, router) = with_store();
        let (status, health) = get_json::<Health>(router, "/healthz").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(health.status, "ok");
        assert!(!health.version.is_empty());
    }

    /// A store that has only ever been opened still holds one graph — its own. Reporting an empty
    /// registry here would be a plausible-looking lie, and it is the reading that a "hide our
    /// bookkeeping" filter in the wrong layer would produce.
    #[tokio::test]
    async fn a_fresh_store_reports_the_system_graph_and_nothing_else() {
        let (_dir, _store, router) = with_store();
        let (status, list) = get_json::<GraphList>(router, "/api/graphs").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(list.graphs.len(), 1, "expected only the system graph");
        assert_eq!(list.graphs[0].iri, SYSTEM_GRAPH_IRI);
        assert_eq!(list.graphs[0].kind, GraphKind::System);
    }

    #[tokio::test]
    async fn a_registered_vocabulary_is_reported_with_its_kind_and_in_iri_order() {
        let (_dir, store, router) = with_store();
        for iri in [
            "http://example.org/v/zebra",
            "http://example.org/v/aardvark",
        ] {
            store
                .create_vocabulary_graph(&GraphId::vocabulary(iri).expect("a valid IRI"))
                .expect("a fresh IRI is registrable");
        }

        let (status, list) = get_json::<GraphList>(router, "/api/graphs").await;

        assert_eq!(status, StatusCode::OK);
        let reported: Vec<_> = list
            .graphs
            .iter()
            .map(|graph| (graph.iri.as_str(), graph.kind))
            .collect();
        assert_eq!(
            reported,
            vec![
                ("http://example.org/v/aardvark", GraphKind::Vocabulary),
                ("http://example.org/v/zebra", GraphKind::Vocabulary),
                (SYSTEM_GRAPH_IRI, GraphKind::System),
            ],
            "the order must be the store's stable IRI order, not the backend's iteration order"
        );
    }

    /// The endpoint is read-only until the discovery-first creation path exists (`CLAUDE.md` §1.7).
    /// A 405 rather than a fall-through to the HTML shell is what tells an API client that.
    #[tokio::test]
    async fn the_registry_cannot_be_written_through_this_endpoint() {
        let (_dir, _store, router) = with_store();
        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/graphs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    /// The end-to-end shape of an export: the right bytes, and headers that say what they are.
    /// The system graph is used because it is the one graph a fresh store has content in — and
    /// exporting it is a real operator answer to "what is actually in my store?", which is the
    /// opacity `CLAUDE.md` §1 exists to attack.
    #[tokio::test]
    async fn a_graph_is_exported_with_headers_that_describe_it() {
        let (_dir, store, router) = with_store();
        store
            .create_vocabulary_graph(&GraphId::vocabulary("http://acme.example/v/finance").unwrap())
            .expect("a fresh IRI registers");

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/export?graph=urn%3Aopenbiz%3Agraph%3Asystem&format=nquads")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[axum::http::header::CONTENT_TYPE],
            "application/n-quads; charset=utf-8"
        );
        assert_eq!(
            response.headers()[axum::http::header::CONTENT_DISPOSITION],
            "attachment; filename=\"system.nq\""
        );
        assert_eq!(response.headers()["x-openbiz-graph"], SYSTEM_GRAPH_IRI);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(body.to_vec()).expect("UTF-8");
        assert!(
            text.contains("http://acme.example/v/finance"),
            "the registry entry for the vocabulary belongs in the system graph: {text}"
        );
        assert!(
            text.lines()
                .all(|line| line.ends_with(&format!("<{SYSTEM_GRAPH_IRI}> ."))),
            "every N-Quads line must name the graph it came from: {text}"
        );
    }

    /// A vocabulary that exists and holds nothing exports as nothing — and that is a 200, not a
    /// 404. It is the state every vocabulary is in between being created and its first concept.
    #[tokio::test]
    async fn an_empty_vocabulary_exports_as_an_empty_document() {
        let (_dir, store, router) = with_store();
        store
            .create_vocabulary_graph(&GraphId::vocabulary("http://acme.example/v/empty").unwrap())
            .expect("a fresh IRI registers");

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/export?graph=http%3A%2F%2Facme.example%2Fv%2Fempty")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[axum::http::header::CONTENT_TYPE],
            "text/turtle; charset=utf-8",
            "no format and no Accept means the readable default"
        );
    }

    /// The difference between "there is no such vocabulary" and "here is your vocabulary, it is
    /// empty" is one a caller cannot recover from being told wrongly.
    #[tokio::test]
    async fn exporting_a_graph_that_does_not_exist_is_a_404_not_an_empty_file() {
        let (_dir, _store, router) = with_store();
        let (status, error) = get_json::<openbiz_api::ApiError>(
            router,
            "/api/export?graph=http%3A%2F%2Facme.example%2Fv%2Fnope",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            error.message.contains("http://acme.example/v/nope"),
            "the refusal must name what was asked for: {}",
            error.message
        );
    }

    #[tokio::test]
    async fn an_export_must_name_a_graph() {
        let (_dir, _store, router) = with_store();
        let (status, error) = get_json::<openbiz_api::ApiError>(router, "/api/export").await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            error.message.contains("/api/graphs"),
            "point the caller at the list of graphs: {}",
            error.message
        );
    }

    /// A misspelled format is refused rather than served as the default, because a caller who
    /// asked for JSON-LD and silently received Turtle finds out from their parser.
    #[tokio::test]
    async fn a_format_we_do_not_have_is_refused_and_the_alternatives_are_named() {
        let (_dir, _store, router) = with_store();
        let (status, error) = get_json::<openbiz_api::ApiError>(
            router,
            "/api/export?graph=urn%3Aopenbiz%3Agraph%3Asystem&format=turtel",
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        for token in ["turtle", "ntriples", "nquads", "trig", "rdfxml", "jsonld"] {
            assert!(error.message.contains(token), "{token} must be offered");
        }
    }

    #[tokio::test]
    async fn the_accept_header_chooses_the_syntax_when_no_format_is_given() {
        let (_dir, _store, router) = with_store();
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/export?graph=urn%3Aopenbiz%3Agraph%3Asystem")
                    .header("accept", "application/ld+json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[axum::http::header::CONTENT_TYPE],
            "application/ld+json; charset=utf-8"
        );
    }

    #[tokio::test]
    async fn an_unsatisfiable_accept_is_a_406() {
        let (_dir, _store, router) = with_store();
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/export?graph=urn%3Aopenbiz%3Agraph%3Asystem")
                    .header("accept", "text/csv")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);
    }

    /// The interface renders whatever this returns, so it is the thing that stops the format
    /// chooser drifting from what the serialiser can actually produce.
    #[tokio::test]
    async fn the_export_formats_are_advertised_for_the_interface_to_render() {
        let (_dir, _store, router) = with_store();
        let (status, formats) =
            get_json::<openbiz_api::ExportFormats>(router, "/api/export/formats").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(formats.formats.len(), 6);
        assert_eq!(formats.formats[0].token, "turtle");
        assert!(
            formats.formats.iter().any(|it| it.records_graph_names)
                && formats.formats.iter().any(|it| !it.records_graph_names),
            "the interface can only warn about lossy syntaxes if both kinds are advertised"
        );
    }

    /// The results-format list is what a query console would render, and — more importantly — it
    /// is the only production reader of `preserves_term_detail`. Without this endpoint that
    /// constant is a well-argued fact nothing in the product ever tells a user, which is the
    /// "built but no production caller" failure `CLAUDE.md` §4.1 names.
    #[tokio::test]
    async fn the_results_formats_are_advertised_with_their_lossiness() {
        let (_dir, _store, router) = with_store();
        let (status, formats) =
            get_json::<openbiz_api::ResultsFormats>(router, "/api/sparql/formats").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            formats.formats.len(),
            4,
            "SPARQL 1.1 defines four results formats"
        );
        assert_eq!(
            formats.formats[0].token, "json",
            "the default is offered first"
        );
        assert!(
            formats.formats.iter().any(|it| it.preserves_term_detail)
                && formats.formats.iter().any(|it| !it.preserves_term_detail),
            "an interface can only warn about the lossy one if both kinds are advertised"
        );

        // The specific warning this endpoint exists to make possible, pinned to the format it is
        // actually about — an assertion that both kinds merely *exist* would still pass if CSV and
        // JSON swapped claims.
        let csv = formats
            .formats
            .iter()
            .find(|it| it.token == "csv")
            .expect("CSV is one of the four");
        assert!(
            !csv.preserves_term_detail,
            "CSV writes bare text, so a language tag is lost and the interface must be able to say so"
        );
        assert_eq!(
            csv.media_type, "text/csv",
            "advertised bare, for Accept comparison"
        );
    }

    /// Every advertised token must actually be accepted by the endpoint that advertises it. This
    /// is the drift this list exists to prevent, so it is asserted rather than assumed.
    #[tokio::test]
    async fn every_advertised_results_format_is_one_the_endpoint_accepts() {
        let (_dir, _store, router) = with_store();
        let (_, formats) =
            get_json::<openbiz_api::ResultsFormats>(router.clone(), "/api/sparql/formats").await;

        for format in formats.formats {
            let (_dir, _store, router) = with_store();
            let (status, content_type, _) = respond(
                router,
                Request::builder()
                    .uri(format!(
                        "/api/sparql?query=ASK%20%7B%7D&format={}",
                        format.token
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;

            assert_eq!(
                status,
                StatusCode::OK,
                "?format={} was advertised",
                format.token
            );
            assert!(
                content_type.starts_with(&format.media_type),
                "?format={} answered as {content_type:?}, not the advertised {:?}",
                format.token,
                format.media_type
            );
        }
    }

    /// Routes the API does not claim now fall through to the embedded UI rather than 404ing —
    /// the SPA owns its own URL space. The narrower 404 contract that still holds (unmatched
    /// `/api/…` and missing `/assets/…`) is asserted in [`crate::ui`]'s tests.
    #[tokio::test]
    async fn unknown_routes_fall_through_to_the_ui() {
        let (_dir, _store, router) = with_store();
        let response = router
            .oneshot(Request::builder().uri("/nope").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    /// A response's status, content type, and body — the three things every SPARQL protocol test
    /// below asserts on.
    async fn respond(router: Router, request: Request<Body>) -> (StatusCode, String, String) {
        let response = router.oneshot(request).await.unwrap();
        let status = response.status();
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .map(|value| value.to_str().expect("an ASCII content type").to_owned())
            .unwrap_or_default();
        let body = response.into_body().collect().await.unwrap().to_bytes();

        (
            status,
            content_type,
            String::from_utf8(body.to_vec()).expect("a UTF-8 body"),
        )
    }

    /// SPARQL 1.1 Protocol defines three ways to send a query. All three are implemented, so all
    /// three are tested — a protocol claim backed by one of its forms is a claim about that form.
    #[tokio::test]
    async fn a_query_can_arrive_by_any_of_the_three_protocol_forms() {
        for (name, request) in [
            (
                "GET with ?query=",
                Request::builder()
                    .uri("/api/sparql?query=ASK%20%7B%7D")
                    .body(Body::empty())
                    .unwrap(),
            ),
            (
                "POST application/sparql-query",
                Request::builder()
                    .method("POST")
                    .uri("/api/sparql")
                    .header("content-type", "application/sparql-query")
                    .body(Body::from("ASK { }"))
                    .unwrap(),
            ),
            (
                "POST form-encoded",
                Request::builder()
                    .method("POST")
                    .uri("/api/sparql")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("query=ASK+%7B%7D"))
                    .unwrap(),
            ),
        ] {
            let (_dir, _store, router) = with_store();
            let (status, content_type, body) = respond(router, request).await;

            assert_eq!(status, StatusCode::OK, "{name}");
            assert_eq!(
                content_type, "application/sparql-results+json; charset=utf-8",
                "{name}"
            );
            // The exact document SPARQL 1.1 Query Results JSON defines for an `ASK`, asserted
            // literally rather than by parsing it — the bytes are the contract a client reads.
            assert_eq!(body, r#"{"head":{},"boolean":true}"#, "{name}");
        }
    }

    /// The end-to-end face of the dataset rule. A fresh store holds OpenBiz's own quads — the
    /// format stamp and the registry — and a query that did not ask for them must not see them.
    #[tokio::test]
    async fn a_query_never_returns_our_own_bookkeeping_to_a_caller_who_did_not_ask() {
        let (_dir, store, router) = with_store();
        store
            .create_vocabulary_graph(&GraphId::vocabulary("http://acme.example/v/dataset").unwrap())
            .expect("a fresh IRI registers");

        let (status, _, body) = respond(
            router,
            Request::builder()
                .uri("/api/sparql?query=SELECT%20*%20WHERE%20%7B%3Fs%20%3Fp%20%3Fo%7D")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(!body.contains("urn:openbiz:"), "{body}");
        assert!(
            body.contains(r#""bindings":[]"#),
            "an empty vocabulary answers with no rows, not with our graphs: {body}"
        );
    }

    #[tokio::test]
    async fn a_named_format_decides_the_serialisation_and_the_content_type() {
        let (_dir, _store, router) = with_store();
        let (status, content_type, body) = respond(
            router,
            Request::builder()
                .uri("/api/sparql?query=ASK%20%7B%7D&format=csv")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, "text/csv; charset=utf-8");
        assert_eq!(body, "true");
    }

    /// A `CONSTRUCT` answers with RDF, so the RDF family is what `Accept` is read against — the
    /// same header that would have been unsatisfiable for a `SELECT`.
    #[tokio::test]
    async fn a_construct_is_negotiated_against_the_rdf_family() {
        let (_dir, _store, router) = with_store();
        let (status, content_type, _) = respond(
            router,
            Request::builder()
                .uri("/api/sparql?query=CONSTRUCT%20%7B%3Fs%20%3Fp%20%3Fo%7D%20WHERE%20%7B%3Fs%20%3Fp%20%3Fo%7D")
                .header("accept", "application/trig")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(content_type, "application/trig; charset=utf-8");
    }

    /// Asking for Turtle and sending a `SELECT` is a negotiation that cannot be satisfied. Every
    /// other endpoint in this market answers it with JSON and a `Content-Type` that contradicts
    /// the request; here it is a 406 that says what the query actually produced.
    #[tokio::test]
    async fn an_accept_that_the_answers_shape_cannot_satisfy_is_a_406() {
        let (_dir, _store, router) = with_store();
        let (status, _, body) = respond(
            router,
            Request::builder()
                .uri("/api/sparql?query=SELECT%20*%20WHERE%20%7B%3Fs%20%3Fp%20%3Fo%7D")
                .header("accept", "text/turtle")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_ACCEPTABLE);
        assert!(body.contains("table of solutions"), "{body}");
    }

    /// The endpoint never writes, and it says which of the two things the caller sent.
    #[tokio::test]
    async fn an_update_is_refused_as_an_update() {
        let (_dir, _store, router) = with_store();
        let (status, _, body) = respond(
            router,
            Request::builder()
                .method("POST")
                .uri("/api/sparql")
                .header("content-type", "application/sparql-query")
                .body(Body::from("CLEAR ALL"))
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("SPARQL Update"), "{body}");
        assert!(body.contains("never writes"), "{body}");
    }

    /// Refused rather than ignored. An ignored `default-graph-uri` answers a question about a
    /// different dataset from the one the caller asked about, which is the class of wrongness a
    /// caller has no way to detect.
    #[tokio::test]
    async fn an_unimplemented_protocol_parameter_is_refused_rather_than_ignored() {
        let (_dir, _store, router) = with_store();
        let (status, _, body) = respond(
            router,
            Request::builder()
                .uri("/api/sparql?query=ASK%20%7B%7D&default-graph-uri=http%3A%2F%2Fa.example%2Fg")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("default-graph-uri"), "{body}");
    }

    #[tokio::test]
    async fn a_query_must_actually_be_sent() {
        let (_dir, _store, router) = with_store();
        let (status, _, body) = respond(
            router,
            Request::builder()
                .uri("/api/sparql")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.contains("query="), "{body}");
    }

    #[tokio::test]
    async fn a_posted_query_needs_a_content_type_the_protocol_defines() {
        let (_dir, _store, router) = with_store();
        let (status, _, body) = respond(
            router,
            Request::builder()
                .method("POST")
                .uri("/api/sparql")
                .header("content-type", "text/plain")
                .body(Body::from("ASK { }"))
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert!(body.contains("application/sparql-query"), "{body}");
    }

    /// A form-encoded request carries its own `format` field, and that field is the one that
    /// decides — including when the URL carries a different one. Found by a mutant that swapped
    /// the precedence and survived, because the first version of this test only sent `format` in
    /// one of the two places, where either precedence gives the same answer.
    #[tokio::test]
    async fn a_form_encoded_request_chooses_its_format_from_its_own_body() {
        let (_dir, _store, router) = with_store();
        let (status, content_type, body) = respond(
            router,
            Request::builder()
                .method("POST")
                .uri("/api/sparql?format=xml")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("query=ASK+%7B%7D&format=tsv"))
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            content_type, "text/tab-separated-values; charset=utf-8",
            "the form body is the request's own payload, so its format wins over a leftover in \
             the URL"
        );
        assert_eq!(body, "true");
    }

    /// A `POST` with no `Content-Type` at all must be refused rather than guessed at. Guessing
    /// means reading an arbitrary body as SPARQL, which turns a client bug into a parser error
    /// about text the caller never meant as a query. Also found by a surviving mutant.
    #[tokio::test]
    async fn a_posted_query_with_no_content_type_is_refused_rather_than_guessed_at() {
        let (_dir, _store, router) = with_store();
        let (status, _, body) = respond(
            router,
            Request::builder()
                .method("POST")
                .uri("/api/sparql")
                .body(Body::from("ASK { }"))
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert!(body.contains("Content-Type"), "{body}");
    }

    /// `CLAUDE.md` §1.1 promises air-gapped operation, and `docs/adr/0006` makes it true by
    /// building the store without an HTTP client rather than by asking callers not to federate. A
    /// `SERVICE` clause is therefore a **capability this build does not have** — a 501 that says
    /// so, not the bare 500 a hand-run against the real binary found it giving.
    #[tokio::test]
    async fn a_federated_query_is_a_documented_absence_rather_than_a_failure() {
        let (_dir, _store, router) = with_store();
        let (status, _, body) = respond(
            router,
            Request::builder()
                .uri("/api/sparql?query=SELECT%20*%20WHERE%20%7BSERVICE%20%3Chttp%3A%2F%2Fa.invalid%2F%3E%20%7B%3Fs%20%3Fp%20%3Fo%7D%7D")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert!(body.contains("air-gapped"), "{body}");
        assert!(
            body.contains("nothing was sent"),
            "the caller's first question is whether the remote endpoint heard from us: {body}"
        );
    }

    /// The endpoint claims two methods. A third must be a 405 rather than a fall-through to the
    /// HTML shell, which is what tells an API client that the path exists and the verb does not.
    #[tokio::test]
    async fn the_sparql_endpoint_answers_to_get_and_post_and_nothing_else() {
        let (_dir, _store, router) = with_store();
        let (status, _, _) = respond(
            router,
            Request::builder()
                .method("PUT")
                .uri("/api/sparql")
                .body(Body::from("ASK { }"))
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }

    /// The fall-through must not swallow an API path the router does not claim — a client asking
    /// for `/api/typo` needs a 404, not a page of HTML.
    #[tokio::test]
    async fn an_unmatched_api_route_is_still_a_404() {
        let (_dir, _store, router) = with_store();
        let response = router
            .oneshot(
                Request::builder()
                    .uri("/api/graphss")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
