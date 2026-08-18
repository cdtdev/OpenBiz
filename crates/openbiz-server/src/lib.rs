//! The OpenBiz server: routing, configuration, and composition of the engine crates.
//!
//! This is the composition root. It will be touched by nearly every feature, which is expected —
//! `/openbiz-status` treats frequent commits here as healthy growth so long as they add distinct
//! things rather than repair the same mechanism.

use axum::{routing::get, Json, Router};
use openbiz_api::Health;

mod config;
mod shutdown;
mod ui;

pub use config::{Config, ConfigError, Setting, Source};
pub use shutdown::shutdown_signal;

/// Build the application router.
///
/// Everything the API does not claim falls through to the embedded UI (see [`ui`]), which is what
/// makes the single binary in `CLAUDE.md` §1 a fact rather than an intention. Using a
/// `MethodRouter` as the fallback service — rather than a bare handler — keeps writes to unknown
/// paths a 405 instead of quietly answering a POST with the HTML shell.
pub fn app() -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .fallback_service(get(ui::serve))
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
    use tower::ServiceExt;

    #[tokio::test]
    async fn healthz_reports_ok() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let health: Health = serde_json::from_slice(&body).unwrap();
        assert_eq!(health.status, "ok");
        assert!(!health.version.is_empty());
    }

    /// Routes the API does not claim now fall through to the embedded UI rather than 404ing —
    /// the SPA owns its own URL space. The narrower 404 contract that still holds (unmatched
    /// `/api/…` and missing `/assets/…`) is asserted in [`crate::ui`]'s tests.
    #[tokio::test]
    async fn unknown_routes_fall_through_to_the_ui() {
        let response = app()
            .oneshot(Request::builder().uri("/nope").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
