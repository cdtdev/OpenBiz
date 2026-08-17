//! The OpenBiz server: routing, configuration, and composition of the engine crates.
//!
//! This is the composition root. It will be touched by nearly every feature, which is expected —
//! `/openbiz-status` treats frequent commits here as healthy growth so long as they add distinct
//! things rather than repair the same mechanism.

use axum::{routing::get, Json, Router};
use openbiz_api::Health;

/// Server configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Address to bind, e.g. `127.0.0.1:8080`.
    pub bind: String,
    /// Directory holding the RDF store and backups.
    pub data_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".to_owned(),
            data_dir: "./data".to_owned(),
        }
    }
}

impl Config {
    /// Read configuration from the environment, falling back to [`Default`].
    ///
    /// Deliberately minimal: a self-hosted product must start with no configuration at all, and
    /// every required setting is one more step between download and a running server.
    pub fn from_env() -> Self {
        let default = Self::default();
        Self {
            bind: std::env::var("OPENBIZ_BIND").unwrap_or(default.bind),
            data_dir: std::env::var("OPENBIZ_DATA_DIR").unwrap_or(default.data_dir),
        }
    }
}

/// Build the application router.
pub fn app() -> Router {
    Router::new().route("/healthz", get(healthz))
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

    #[tokio::test]
    async fn unknown_routes_are_not_found() {
        let response = app()
            .oneshot(Request::builder().uri("/nope").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn config_defaults_to_loopback() {
        // A self-hosted server must not default to a public interface.
        assert!(Config::default().bind.starts_with("127.0.0.1"));
    }
}
