//! The store as a discovery corpus: the adapter, and deliberately nothing else.
//!
//! `openbiz-discovery` owns the trait, the ranking, and the rule that a source which cannot
//! answer is reported rather than fatal. What it does not own — and by `CLAUDE.md` §3 must not —
//! is the store. This module is the composition root's ten lines that let the one meet the other:
//! it lists what is searchable, reads one part at a time, and turns a store failure into the
//! [`Unavailable`] outcome discovery already knows how to report.
//!
//! # What counts as searchable, and why the pending changes are in
//!
//! Every vocabulary in the store, and every change staged against one and still waiting for a
//! decision. The second half matters on the creation path: a curator who proposed "Tidal power"
//! this morning and has not had it approved must not be told this afternoon that nothing is
//! called that. `openbiz mint` already reads the pending changes when it checks whether an IRI is
//! taken, so a label check that did not read them would print "nothing is already called this"
//! directly above "the IRI is taken by candidate 2" — two true sentences that read as a
//! contradiction, and one this build already fixed once.

use openbiz_discovery::{CorpusPart, LocalCorpus, Unavailable};
use openbiz_skos::CoreModel;
use openbiz_store::{CandidateState, GraphKind, Store};

use crate::inspect::convert;

/// The local store, seen as the parts discovery can search.
pub(crate) struct StoreCorpus<'a> {
    store: &'a Store,
    /// The vocabulary a new concept would go into, which is the one part reported as home.
    target: String,
}

impl<'a> StoreCorpus<'a> {
    /// The store, with `target` as the vocabulary being authored.
    pub(crate) fn authoring(store: &'a Store, target: &str) -> Self {
        StoreCorpus {
            store,
            target: target.to_owned(),
        }
    }
}

impl LocalCorpus for StoreCorpus<'_> {
    fn parts(&self) -> Result<Vec<CorpusPart>, Unavailable> {
        let mut parts = Vec::new();

        // The vocabulary being authored first, so a duplicate about to be created inside it is
        // the first thing read as well as the first thing ranked.
        let graphs = self.store.graphs().map_err(|error| {
            Unavailable::because(format!("the store could not be read: {error}"))
        })?;
        for graph in graphs {
            if graph.kind() != GraphKind::Vocabulary {
                continue;
            }
            parts.push(match graph.iri() == self.target {
                true => CorpusPart::home(graph.iri(), "this vocabulary"),
                false => {
                    CorpusPart::elsewhere(graph.iri(), format!("the vocabulary {}", graph.iri()))
                }
            });
        }
        // Deterministic, and home first whatever order the store lists its graphs in: the report
        // is read top to bottom by somebody deciding whether to create a concept.
        parts.sort_by_key(|part| (!part.is_home(), part.at.clone()));

        let candidates = self.store.candidates().map_err(|error| {
            Unavailable::because(format!("the staged changes could not be read: {error}"))
        })?;
        for candidate in candidates {
            // Only the ones still waiting. An approved candidate's statements are in the
            // vocabulary and would be found there twice; a rejected one's are the record of
            // something refused, and reporting a refused label as an existing concept is worse
            // than not reporting it.
            if candidate.state() != CandidateState::Proposed {
                continue;
            }
            let Some(payload) = candidate.payload() else {
                continue;
            };
            parts.push(CorpusPart::pending(
                payload.iri(),
                format!(
                    "candidate {}, which is waiting for a decision",
                    candidate.id()
                ),
            ));
        }

        Ok(parts)
    }

    fn model(&self, part: &CorpusPart) -> Result<CoreModel, Unavailable> {
        let mut builder = CoreModel::builder();
        self.store
            .for_each_statement(&part.at, |statement| builder.push(convert(statement)))
            .map_err(|error| {
                Unavailable::because(format!("{} could not be read: {error}", part.within))
            })?;
        Ok(builder.build())
    }
}
