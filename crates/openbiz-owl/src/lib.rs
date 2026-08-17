//! OWL 2 modelling and reasoning for OpenBiz.
//!
//! # Scope, stated honestly
//!
//! There is **no OWL 2 DL reasoner in the Rust ecosystem**. Our realistic targets are the **EL**
//! and **RL** profiles, which cover the large majority of enterprise ontologies (SNOMED CT and the
//! Gene Ontology are both EL) but leave a genuine gap against Protégé with HermiT for expressive
//! DL ontologies. That gap is documented in the README rather than glossed over, and
//! [`Profile::Dl`] exists so we can *reject* work we cannot do rather than silently under-reason.
//!
//! Per `CLAUDE.md` §3, concrete reasoners sit behind [`Reasoner`] and are never called directly
//! from application code.

use thiserror::Error;

/// An OWL 2 profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// RDFS entailment only.
    Rdfs,
    /// OWL 2 EL — tractable, covers large biomedical terminologies.
    El,
    /// OWL 2 RL — rule-based, suits forward-chaining materialisation.
    Rl,
    /// Full OWL 2 DL. **Not currently supported by any Rust reasoner.**
    Dl,
}

/// A single step in the derivation of an inferred statement.
#[derive(Debug, Clone)]
pub struct DerivationStep {
    /// The rule or axiom that licensed this step.
    pub rule: String,
    /// The statements this step consumed.
    pub premises: Vec<String>,
    /// The statement this step produced.
    pub conclusion: String,
}

/// Why an inference holds — the full derivation chain.
///
/// `CLAUDE.md` §3 forbids shipping an inference path that cannot explain itself. This type is that
/// commitment made structural: a [`Reasoner`] cannot report an inference without being able to
/// produce its [`Explanation`].
#[derive(Debug, Clone)]
pub struct Explanation {
    /// The statement being explained.
    pub conclusion: String,
    /// The derivation, ordered from premises to conclusion.
    pub steps: Vec<DerivationStep>,
}

/// Errors raised while reasoning.
#[derive(Debug, Error)]
pub enum ReasoningError {
    /// The ontology is outside the profile this reasoner supports.
    #[error("ontology is not in the {0:?} profile: {1}")]
    OutOfProfile(Profile, String),
    /// The ontology is logically inconsistent.
    #[error("ontology is inconsistent: {0}")]
    Inconsistent(String),
    /// The underlying engine failed.
    #[error("reasoning engine failed: {0}")]
    Engine(String),
}

/// A reasoning engine over an ontology graph.
pub trait Reasoner {
    /// The profile this engine reasons in.
    fn profile(&self) -> Profile;

    /// Materialise entailed statements for the named graph, returning their count.
    fn materialise(&self, graph: &str) -> Result<usize, ReasoningError>;

    /// Explain why `statement` is entailed in the named graph.
    ///
    /// Returns `Ok(None)` when the statement is not entailed at all. Returning an inference from
    /// [`Self::materialise`] that this method cannot explain is a bug, not a limitation.
    fn explain(&self, graph: &str, statement: &str) -> Result<Option<Explanation>, ReasoningError>;
}

/// A reasoner that entails nothing. The default until Phase 5 lands a real engine.
///
/// Deliberately honest: it reports [`Profile::Rdfs`] and derives nothing, so a caller that depends
/// on inference sees zero results rather than silently wrong ones.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullReasoner;

impl Reasoner for NullReasoner {
    fn profile(&self) -> Profile {
        Profile::Rdfs
    }

    fn materialise(&self, _graph: &str) -> Result<usize, ReasoningError> {
        Ok(0)
    }

    fn explain(
        &self,
        _graph: &str,
        _statement: &str,
    ) -> Result<Option<Explanation>, ReasoningError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_reasoner_derives_nothing() {
        let r = NullReasoner;
        assert_eq!(r.materialise("http://example.org/g").unwrap(), 0);
    }

    #[test]
    fn null_reasoner_explains_nothing_it_did_not_derive() {
        let r = NullReasoner;
        let explanation = r.explain("http://example.org/g", "?s ?p ?o").unwrap();
        assert!(
            explanation.is_none(),
            "a reasoner that derives nothing must not claim an explanation"
        );
    }
}
