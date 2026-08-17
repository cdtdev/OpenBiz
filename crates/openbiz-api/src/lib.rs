//! Shared HTTP/JSON types for the OpenBiz API.
//!
//! These types are the contract between the Rust backend and the TypeScript frontend. Keep them
//! free of storage and reasoning concerns so the contract stays stable while engines behind it
//! change.

use serde::{Deserialize, Serialize};

/// Health report returned by `GET /healthz`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    /// `"ok"` when the server is able to serve requests.
    pub status: String,
    /// The server's crate version.
    pub version: String,
}

impl Health {
    /// A healthy report stamped with the compiled-in crate version.
    pub fn ok() -> Self {
        Self {
            status: "ok".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_health_reports_status_and_version() {
        let health = Health::ok();
        assert_eq!(health.status, "ok");
        assert!(
            !health.version.is_empty(),
            "version must be stamped from the crate metadata"
        );
    }
}
