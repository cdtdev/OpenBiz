//! The OpenBiz server: routing, configuration, and composition of the engine crates.
//!
//! This is the composition root. It will be touched by nearly every feature, which is expected —
//! `/openbiz-status` treats frequent commits here as healthy growth so long as they add distinct
//! things rather than repair the same mechanism.

use axum::{routing::get, Json, Router};
use openbiz_api::Health;

mod config;
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
