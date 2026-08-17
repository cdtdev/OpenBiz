//! Validation for OpenBiz: SHACL plus the governance rule packs built on it.
//!
//! # The boundary this crate exists to defend
//!
//! Per `CLAUDE.md` §3, **no third-party SHACL engine may be called from application code**.
//! Everything goes through [`Validator`]. The Rust SHACL crates are young and none has yet been
//! measured against the W3C test suite for our purposes (Phase 4 opens with that spike), so we must
//! be able to swap or replace the engine without touching callers.

use thiserror::Error;

/// Severity of a validation result, mirroring SHACL's `sh:severity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// `sh:Info` — advisory only.
    Info,
    /// `sh:Warning` — a likely problem that does not block.
    Warning,
    /// `sh:Violation` — a genuine constraint violation.
    Violation,
}

/// A single validation result.
///
/// `explanation` is not optional decoration. Per `CLAUDE.md` §3 every violation must be able to say
/// why it fired and how to fix it — governance teams have to defend these decisions to auditors,
/// and "constraint failed" is not a defence.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// IRI of the node that failed.
    pub focus_node: String,
    /// IRI of the shape that reported the failure.
    pub source_shape: String,
    /// How serious this is.
    pub severity: Severity,
    /// Human-readable account of what failed and how to fix it.
    pub explanation: String,
}

/// The outcome of validating a graph against a shapes graph.
#[derive(Debug, Clone, Default)]
pub struct ValidationReport {
    /// Every result produced, in no guaranteed order.
    pub results: Vec<ValidationResult>,
}

impl ValidationReport {
    /// True when nothing at [`Severity::Violation`] was reported.
    ///
    /// Warnings and info results do not make a graph non-conformant.
    pub fn conforms(&self) -> bool {
        !self
            .results
            .iter()
            .any(|r| r.severity == Severity::Violation)
    }
}

/// Errors raised while validating.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// The shapes graph could not be parsed or understood.
    #[error("invalid shapes graph: {0}")]
    InvalidShapes(String),
    /// The underlying engine failed.
    #[error("validation engine failed: {0}")]
    Engine(String),
}

/// A SHACL validation engine.
///
/// Implementations wrap a concrete engine. Application code depends on this trait, never on the
/// engine directly.
pub trait Validator {
    /// Validate `data_graph` against `shapes_graph`, both as named-graph IRIs.
    fn validate(
        &self,
        data_graph: &str,
        shapes_graph: &str,
    ) -> Result<ValidationReport, ValidationError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result_at(severity: Severity) -> ValidationResult {
        ValidationResult {
            focus_node: "http://example.org/c1".to_owned(),
            source_shape: "http://example.org/shapes#ConceptShape".to_owned(),
            severity,
            explanation: "test".to_owned(),
        }
    }

    #[test]
    fn empty_report_conforms() {
        assert!(ValidationReport::default().conforms());
    }

    #[test]
    fn violations_break_conformance() {
        let report = ValidationReport {
            results: vec![result_at(Severity::Violation)],
        };
        assert!(!report.conforms());
    }

    #[test]
    fn warnings_and_info_do_not_break_conformance() {
        let report = ValidationReport {
            results: vec![result_at(Severity::Warning), result_at(Severity::Info)],
        };
        assert!(
            report.conforms(),
            "only sh:Violation makes a graph non-conformant"
        );
    }
}
