//! The OpenBiz server: routing, configuration, and composition of the engine crates.
//!
//! This is the composition root. It will be touched by nearly every feature, which is expected —
//! `/openbiz-status` treats frequent commits here as healthy growth so long as they add distinct
//! things rather than repair the same mechanism.

use axum::{routing::get, Json, Router};
use openbiz_api::Health;

mod config;
mod export;
mod graphs;
mod shutdown;
mod ui;

pub use config::{Config, ConfigError, Setting, Source};
pub use graphs::AppState;
pub use shutdown::shutdown_signal;

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
