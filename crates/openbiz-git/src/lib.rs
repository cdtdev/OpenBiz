//! Vocabulary-as-code: git and GitHub integration.
//!
//! This is the pillar the incumbents cannot copy without changing how they build — practitioner
//! reviews of PoolParty cite no visible roadmap and no insight into fixes, which is a structural
//! consequence of closed development (`docs/COMPETITIVE.md`).
//!
//! Two constraints shape this crate:
//!
//! 1. **Serialisation must be deterministic.** A diff is only reviewable if re-serialising an
//!    unchanged vocabulary produces a byte-identical file. Stable ordering is a correctness
//!    requirement here, not a nicety.
//! 2. **GitHub is optional.** Air-gapped customers are a first-class deployment target
//!    (`CLAUDE.md` §1), so everything must degrade to plain local git with no API access.

use thiserror::Error;

/// Where a vocabulary's git remote lives, if anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteKind {
    /// No remote — local git only. The air-gapped case.
    None,
    /// github.com.
    GitHubCloud,
    /// A self-hosted GitHub Enterprise Server at the given base URL.
    GitHubEnterprise(String),
}

impl RemoteKind {
    /// Whether pull-request workflows are available.
    ///
    /// Without a remote we still get versioning, history, and diffs from local git — just not PRs.
    pub fn supports_pull_requests(&self) -> bool {
        !matches!(self, RemoteKind::None)
    }
}

/// A concept-level change, for rendering a human-readable diff.
///
/// Reviewers approve *meaning*, not triples. A PR body listing raw statement changes is not a
/// review artefact a taxonomist can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConceptChange {
    /// A concept was added.
    Added {
        /// The concept's IRI.
        iri: String,
        /// Its preferred label at the time of the change.
        pref_label: String,
    },
    /// A concept's preferred label changed.
    Relabelled {
        /// The concept's IRI.
        iri: String,
        /// The previous preferred label.
        from: String,
        /// The new preferred label.
        to: String,
    },
    /// A concept moved in the hierarchy.
    Moved {
        /// The concept's IRI.
        iri: String,
        /// The previous broader concept, if any.
        from_broader: Option<String>,
        /// The new broader concept, if any.
        to_broader: Option<String>,
    },
    /// A concept was deprecated. Never deleted — auditors need the trail.
    Deprecated {
        /// The concept's IRI.
        iri: String,
        /// The concept superseding it, if one was named.
        replaced_by: Option<String>,
    },
}

/// Errors raised by git integration.
#[derive(Debug, Error)]
pub enum GitError {
    /// The working tree is not a git repository.
    #[error("not a git repository: {0}")]
    NotARepository(String),
    /// A merge conflict needs human resolution.
    #[error("merge conflict in {0}")]
    Conflict(String),
    /// The remote API rejected the request or was unreachable.
    #[error("remote operation failed: {0}")]
    Remote(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn airgapped_deployments_have_no_pull_requests() {
        assert!(!RemoteKind::None.supports_pull_requests());
    }

    #[test]
    fn both_github_flavours_support_pull_requests() {
        assert!(RemoteKind::GitHubCloud.supports_pull_requests());
        assert!(
            RemoteKind::GitHubEnterprise("https://ghe.example.com".to_owned())
                .supports_pull_requests()
        );
    }
}
