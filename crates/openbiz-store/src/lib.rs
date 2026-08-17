//! Embedded RDF store and named-graph model.
//!
//! Phase 1 wires this to an embedded Oxigraph instance. The single-binary rule (`CLAUDE.md` §1)
//! means the store is a library inside our process, never an external service — adding a required
//! triplestore would be a charter violation, not an optimisation.
//!
//! Two known Oxigraph risks are recorded in `docs/COMPETITIVE.md` and must be benchmarked before we
//! depend on them: SPARQL query evaluation is upstream-documented as unoptimised, and numeric,
//! calendar, and duration literal encodings have precision limits outside which arithmetic is
//! undefined.

use thiserror::Error;

/// How a named graph is used.
///
/// Vocabularies are isolated per graph so they can be versioned, exported, and permissioned
/// independently; OpenBiz's own bookkeeping lives in [`GraphKind::System`] so it never leaks into a
/// customer's exported vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphKind {
    /// A user-authored vocabulary.
    Vocabulary,
    /// OpenBiz's own metadata: workflow state, provenance, configuration.
    System,
    /// Materialised inferences, kept separate so they are never confused with asserted facts.
    Inferred,
}

/// Identifies a named graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphId {
    /// The graph's IRI.
    pub iri: String,
    /// What the graph holds.
    pub kind: GraphKind,
}

impl GraphId {
    /// A vocabulary graph.
    pub fn vocabulary(iri: impl Into<String>) -> Self {
        Self {
            iri: iri.into(),
            kind: GraphKind::Vocabulary,
        }
    }

    /// Whether callers may write to this graph directly.
    ///
    /// Inferred graphs are written only by a reasoner-driven materialisation pass; letting
    /// application code assert into them would destroy the asserted-versus-inferred distinction the
    /// UI depends on.
    pub fn is_directly_writable(&self) -> bool {
        !matches!(self.kind, GraphKind::Inferred)
    }
}

/// Errors raised by the store.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The store could not be opened at the configured path.
    #[error("could not open store at {path}: {source}")]
    Open {
        /// The path that failed.
        path: String,
        /// The underlying cause.
        source: std::io::Error,
    },
    /// A write targeted a graph that is not directly writable.
    #[error("graph {0} is not directly writable")]
    NotWritable(String),
    /// The backend failed.
    #[error("store backend failed: {0}")]
    Backend(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vocabulary_graphs_are_writable() {
        assert!(GraphId::vocabulary("http://example.org/v/1").is_directly_writable());
    }

    #[test]
    fn inferred_graphs_are_not_directly_writable() {
        let inferred = GraphId {
            iri: "http://example.org/v/1/inferred".to_owned(),
            kind: GraphKind::Inferred,
        };
        assert!(
            !inferred.is_directly_writable(),
            "only materialisation may write inferred graphs"
        );
    }
}
