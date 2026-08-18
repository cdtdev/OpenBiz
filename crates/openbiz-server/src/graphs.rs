//! `GET /api/graphs` — the store's graph registry, over HTTP.
//!
//! This is the **read** half of the named-graph model (`openbiz_store::graph`). Creation is
//! deliberately absent: `CLAUDE.md` §1.7 requires discovery to run before creation and to record a
//! justification when something new is made anyway, and `DiscoveryProvider` does not exist until
//! Phase 2. A `POST /api/graphs` added now would be a charter violation wearing the costume of
//! progress, so the registry is readable and not yet writable.
//!
//! # Why the response carries OpenBiz's own graphs
//!
//! Every registered graph is listed, including the system graph. The registry is an account of
//! what the store holds, and an endpoint that quietly omitted rows would make "what is in my
//! store?" unanswerable from the API — which is precisely the opacity `CLAUDE.md` attacks.
//!
//! What the incumbents get wrong is not *exposing* their support graphs but **presenting them as
//! the user's**: VocBench puts the triplestore's own graphs in front of a subject-matter expert,
//! who is then asked "which graph does this go in?" and cannot answer. The separation that fixes
//! that lives one layer up — `kind` is on the wire so the UI can keep our bookkeeping out of the
//! places a taxonomist works, without the API having to lie.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use openbiz_api::{ApiError, GraphKind, GraphList, GraphSummary};
use openbiz_store::{Store, StoreError};

/// What the HTTP layer needs from the rest of the process.
///
/// The store is shared rather than owned because axum clones the state per connection. `main`
/// keeps its own handle so it can [`Store::close`] after the server has drained — see the comment
/// there, which is the only place that ordering is enforced.
#[derive(Clone)]
pub struct AppState {
    store: Arc<Store>,
}

impl AppState {
    /// State backed by an already-open store.
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

/// A failed API response.
///
/// One type, so every error the API produces has the same body shape and the same decision about
/// what a client is told.
#[derive(Debug)]
pub(crate) struct Failure {
    status: StatusCode,
    message: String,
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        (self.status, Json(ApiError::new(self.message))).into_response()
    }
}

impl From<StoreError> for Failure {
    /// A store failure is ours, not the caller's, so it is a 500 — and the store's own words go to
    /// the log rather than into the response.
    ///
    /// That split is deliberate and costs something. `StoreError` is written for an operator: it
    /// names the store's path and, for a corrupt registry, the IRI of the offending graph. Both are
    /// facts about the customer's deployment and their vocabularies, and this endpoint has no
    /// authentication in front of it yet (Phase 7). The operator loses nothing — the full error is
    /// in the log, at the moment it happened — while an unauthenticated caller learns only that the
    /// registry could not be read. Revisit when there is an authenticated administrative role to
    /// return the detail to.
    fn from(error: StoreError) -> Self {
        tracing::error!(%error, "the graph registry could not be read");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "the graph registry could not be read; the server log records why".to_owned(),
        }
    }
}

/// `GET /api/graphs` — every graph the store knows about, ordered by IRI.
///
/// Reads the registry on each request rather than caching it. The registry is small, it is the
/// store's own metadata, and a cache would have to be invalidated by every future creation path —
/// a stale "your vocabulary does not exist" is a worse failure than a scan that has not yet been
/// measured. That it is *unmeasured* is recorded in `docs/UNTESTED.md`.
pub(crate) async fn list(State(state): State<AppState>) -> Result<Json<GraphList>, Failure> {
    let graphs = state.store.graphs()?;

    Ok(Json(GraphList {
        graphs: graphs
            .into_iter()
            .map(|graph| GraphSummary {
                kind: on_the_wire(graph.kind()),
                iri: graph.iri().to_owned(),
            })
            .collect(),
    }))
}

/// Translate the store's notion of a graph kind into the published one.
///
/// Exhaustive on purpose. A fourth kind in the store fails this build until somebody decides what
/// it is called on the wire, which is the whole reason `openbiz_api::GraphKind` is a separate type
/// rather than a re-export.
fn on_the_wire(kind: openbiz_store::GraphKind) -> GraphKind {
    match kind {
        openbiz_store::GraphKind::Vocabulary => GraphKind::Vocabulary,
        openbiz_store::GraphKind::System => GraphKind::System,
        openbiz_store::GraphKind::Inferred => GraphKind::Inferred,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn every_store_kind_has_a_distinct_name_on_the_wire() {
        assert_eq!(
            on_the_wire(openbiz_store::GraphKind::Vocabulary),
            GraphKind::Vocabulary
        );
        assert_eq!(
            on_the_wire(openbiz_store::GraphKind::System),
            GraphKind::System
        );
        assert_eq!(
            on_the_wire(openbiz_store::GraphKind::Inferred),
            GraphKind::Inferred
        );
    }

    /// The error path this endpoint can actually take. It is hard to reach through a real store —
    /// `main` refuses to start against a registry it cannot read — so the mapping is asserted
    /// directly rather than left as the one branch nothing exercises.
    #[tokio::test]
    async fn a_store_failure_is_a_500_that_does_not_repeat_the_stores_own_words() {
        let error = StoreError::Corrupt {
            path: PathBuf::from("/srv/customer-deployment/data/store"),
            detail: "graph http://acme.example/secret-project has kind \"ontology\"".to_owned(),
        };
        let detail = error.to_string();

        let response = Failure::from(error).into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("a body");
        let reported: ApiError = serde_json::from_slice(&body).expect("an ApiError body");

        assert_eq!(
            reported.message,
            "the graph registry could not be read; the server log records why"
        );
        assert!(
            !detail.contains(&reported.message),
            "the test is worthless if the store's error already says this"
        );
        assert!(
            !reported.message.contains("acme.example")
                && !reported.message.contains("customer-deployment"),
            "neither the customer's IRIs nor their paths belong in an unauthenticated response: {}",
            reported.message
        );
    }
}
