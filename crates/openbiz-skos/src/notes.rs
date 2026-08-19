//! Documentation properties — what a vocabulary says *about* its own concepts.
//!
//! §7 of the SKOS Reference (W3C Recommendation, 18 August 2009), statements
//! [`S16`](crate::SkosRule::S16) and [`S17`](crate::SkosRule::S17). A definition and a scope note
//! are what turn a list of terms into something a subject-matter expert can use without asking the
//! person who built it: they are the difference between "Chemistry" appearing under "Science" and
//! anybody being able to tell whether a new record belongs there.
//!
//! # Seven properties, and §7 states no integrity condition on any of them
//!
//! That absence is the single most important fact in this module and it is deliberate on the
//! specification's part. §7.1 opens: "Notes are used to provide information relating to SKOS
//! concepts. There is no restriction on the nature of this information, e.g., it could be plain
//! text, hypertext, or an image; it could be a definition, information about the scope of a
//! concept, editorial information, or any other type of information."
//!
//! So nothing here can make a graph inconsistent, and nothing here produces a
//! [`Finding`](crate::Finding) of any severity. In particular:
//!
//! - **A concept with no `skos:definition` is not a defect.** It is very often a *finding a
//!   governance team wants*, and every incumbent reports it — but it is a best-practice check from
//!   ANSI/NISO Z39.19 or ISO 25964, not something SKOS states, and it belongs in a rule pack in
//!   `openbiz-validate` where the pack can be named and switched off. Reporting it from here would
//!   be inventing an integrity condition and citing the SKOS Reference for it, which is the exact
//!   dishonesty `CLAUDE.md` §4 forbids.
//! - **Two definitions in the same language are not a defect either.** S14's one-per-language-tag
//!   rule is about `skos:prefLabel` and §7 has no counterpart. A concept may carry three
//!   definitions in English from three sources, and a thesaurus being merged from three sources
//!   routinely does.
//! - **A note whose value is an IRI is not a defect.** See the next section: it is one of the
//!   three patterns the specification designed these properties around.
//!
//! # Three usage patterns, two term shapes, and no way to tell the last two apart
//!
//! §7.1: "Three different usage patterns are recommended in the [SKOS-PRIMER] for the SKOS
//! documentation properties — 'documentation as an RDF literal', 'documentation as a related
//! resource description' and 'documentation as a document reference'. The data model defined in
//! this section is intended to accommodate all three design patterns."
//!
//! S16 makes all seven `owl:AnnotationProperty`, which constrains the value not at all — no
//! domain, no range. Example 22 is a literal, Example 23 is an IRI, and both are marked
//! *consistent*. So a note's value is simply a [`Term`](crate::Term), and this module deliberately
//! does not have its own value type: inventing one would be inventing a distinction the
//! specification refuses to draw.
//!
//! The two node-shaped patterns — a related resource with its own description, and a reference to
//! a document — are **indistinguishable from the statement alone**. `<A> skos:note <B>` is
//! Example 23 whether `<B>` is a `foaf:Document` or a blank node carrying an `rdf:value` and a
//! `dct:creator`. We therefore report a note's value as a node and say nothing about which pattern
//! it is, rather than guessing from the shape of the surrounding graph.
//!
//! **And a note's object is not typed by us.** A `skos:broader` gives both ends a class, because
//! S19 and S20 are a domain and a range and say so. §7 has neither, so `<MyNote>` in Example 23
//! enters no model as a resource, acquires no class, and is not counted as anything. An annotation
//! property tells you nothing about what it points at, and a report that quietly registered
//! `<MyNote>` as a vocabulary resource would be adding a member to the customer's vocabulary that
//! nobody wrote.
//!
//! # What *is* inferred: S17, one step upwards
//!
//! S17 makes the six specific properties sub-properties of `skos:note`, so a stated
//! `skos:definition` entails a `skos:note` with the same value. That is materialised here — unlike
//! S24's transitive closure, which `docs/adr/0025` answers by walking — because it is bounded: one
//! extra entry per stated note, never more, and never a chain. `skos:note` has no super-property
//! and none of the six is a sub-property of another, so the lift is exactly one step deep and
//! terminates by construction rather than by a bound.
//!
//! The entailment runs **upwards only**. A stated `skos:note` entails nothing about which of the
//! six it might have been, and inferring one would be a guess wearing a citation — the same
//! reasoning that keeps `skos:semanticRelation` out of [`SemanticRelation`](crate::SemanticRelation).
//! So `openbiz` will tell you a vocabulary has 400 notes of which 120 are definitions, and will
//! never tell you an undifferentiated note is a definition.
//!
//! An asserted note is never overwritten by an entailed one, so a resource stating both
//! `skos:definition "X"` and `skos:note "X"` keeps the assertion and records no derivation —
//! consistent with how an asserted class and an asserted label are treated.
//!
//! # Sub-properties a *vocabulary* declares are read, by a second pass
//!
//! **Corrected at iteration 31; this section said "we do not read it" until then.**
//!
//! §7.1 also says the seven "provide a set of extension points for defining more specific types of
//! note", and an enterprise thesaurus routinely declares `ex:usageNote rdfs:subPropertyOf
//! skos:scopeNote`. Under RDFS that entails a `skos:scopeNote`, and then S17 entails a
//! `skos:note`. Both now happen — see [`crate::refinement`] for the mechanism and
//! `docs/adr/0028` for why it is a second pass over the source rather than a buffer.
//!
//! Two things follow that this module has to be careful about. The lift below is still exactly one
//! step deep and still needs no cycle guard, because **S17 is not what chains** — a refinement
//! chain is resolved entirely in [`crate::refinement`], which has its own guard and its own bound,
//! and what reaches [`NoteKind`] is always one of the seven. And a caller that does not run the
//! first pass gets [`PropertyRefinements::default()`](crate::PropertyRefinements), which entails
//! nothing, so refinements are opt-in at the call site rather than silently on.

use std::fmt;

use crate::model::SkosRule;
use crate::ns;

/// `skos:note` — the general property, and the super-property of the other six under S17.
pub const SKOS_NOTE: &str = "http://www.w3.org/2004/02/skos/core#note";
/// `skos:changeNote` — a fine-grained record of a modification.
pub const SKOS_CHANGE_NOTE: &str = "http://www.w3.org/2004/02/skos/core#changeNote";
/// `skos:definition` — a complete explanation of the concept's intended meaning.
pub const SKOS_DEFINITION: &str = "http://www.w3.org/2004/02/skos/core#definition";
/// `skos:editorialNote` — for an editor, translator or maintainer, not for the reader.
pub const SKOS_EDITORIAL_NOTE: &str = "http://www.w3.org/2004/02/skos/core#editorialNote";
/// `skos:example` — an example of the concept's use.
pub const SKOS_EXAMPLE: &str = "http://www.w3.org/2004/02/skos/core#example";
/// `skos:historyNote` — what the concept used to mean, or used to be called.
pub const SKOS_HISTORY_NOTE: &str = "http://www.w3.org/2004/02/skos/core#historyNote";
/// `skos:scopeNote` — guidance on the boundary: what belongs here and what does not.
pub const SKOS_SCOPE_NOTE: &str = "http://www.w3.org/2004/02/skos/core#scopeNote";

/// One of the seven SKOS documentation properties a note can be given under.
///
/// Ordered so a report reads the way somebody meeting a concept for the first time asks: what does
/// it mean, where does it stop, what does it look like — then the two historical notes, then the
/// one that is addressed to the editor rather than the reader, and finally the undifferentiated
/// general property that says only "there is a note here".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoteKind {
    /// `skos:definition`.
    Definition,
    /// `skos:scopeNote`.
    ScopeNote,
    /// `skos:example`.
    Example,
    /// `skos:historyNote`.
    HistoryNote,
    /// `skos:changeNote`.
    ChangeNote,
    /// `skos:editorialNote`.
    EditorialNote,
    /// `skos:note` — the general property. Every other kind entails one of these under S17.
    Note,
}

impl NoteKind {
    /// Every kind, in the order a report shows them.
    pub const ALL: [NoteKind; 7] = [
        NoteKind::Definition,
        NoteKind::ScopeNote,
        NoteKind::Example,
        NoteKind::HistoryNote,
        NoteKind::ChangeNote,
        NoteKind::EditorialNote,
        NoteKind::Note,
    ];

    /// The property's local name within the SKOS namespace.
    pub fn local_name(self) -> &'static str {
        match self {
            NoteKind::Definition => "definition",
            NoteKind::ScopeNote => "scopeNote",
            NoteKind::Example => "example",
            NoteKind::HistoryNote => "historyNote",
            NoteKind::ChangeNote => "changeNote",
            NoteKind::EditorialNote => "editorialNote",
            NoteKind::Note => "note",
        }
    }

    /// The property's IRI.
    pub fn iri(self) -> String {
        format!("{}{}", ns::SKOS, self.local_name())
    }

    /// The kind an IRI names, or `None` for anything else — which is most predicates.
    pub fn from_iri(iri: &str) -> Option<Self> {
        let local = iri.strip_prefix(ns::SKOS)?;
        NoteKind::ALL
            .into_iter()
            .find(|kind| kind.local_name() == local)
    }

    /// The property this one is a sub-property of, and the statement that says so.
    ///
    /// `None` for [`NoteKind::Note`] itself, which is the top of a hierarchy exactly one step
    /// deep. That shallowness is not an assumption about the graph: S17 names the six explicitly
    /// and SKOS declares no sub-property relationship among them, so the lift can never chain and
    /// needs no cycle guard. A vocabulary's *own* refinements do chain, and are resolved into one
    /// of the seven by [`crate::refinement`] before they ever reach here — which is why this stays
    /// one step and why the guard lives there rather than in this file.
    pub fn super_property(self) -> Option<(NoteKind, SkosRule)> {
        match self {
            NoteKind::Note => None,
            _ => Some((NoteKind::Note, SkosRule::S17)),
        }
    }
}

impl fmt::Display for NoteKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "skos:{}", self.local_name())
    }
}

/// Whether a note was stated by the graph, lifted onto a super-property by S17, or reached
/// through a refinement the vocabulary declared itself.
///
/// Not [`Copy`], unlike the other origin types in this crate, because [`NoteOrigin::Refined`]
/// names an arbitrary property IRI. The alternative was keeping the refining property somewhere
/// other than the origin, which would have meant a report could say a note was inferred without
/// being able to say from what.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoteOrigin {
    /// The graph carries the statement under this property.
    Asserted,
    /// We concluded it, under this rule — in practice always S17.
    Entailed(SkosRule),
    /// The graph stated it under its own property, which it declares a sub-property of this one.
    ///
    /// Licensed by RDFS, not by SKOS: §7.1 offers the extension point and RDF 1.1 Semantics says
    /// what follows from using it. The IRI is the property the graph actually used, so a reader
    /// can find the statement in the source file — which is the whole point, because the source
    /// file does not contain the conclusion.
    Refined {
        /// The property the statement was made with.
        property: String,
    },
}

impl fmt::Display for NoteOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NoteOrigin::Asserted => write!(f, "asserted"),
            NoteOrigin::Entailed(rule) => write!(f, "inferred, {}", rule.number()),
            NoteOrigin::Refined { property } => write!(f, "inferred from {property}, rdfs7"),
        }
    }
}

/// How much of a vocabulary carries documentation of one kind.
///
/// One row per [`NoteKind`], counted over `skos:Concept` instances only — a note on a concept
/// scheme or on an `owl:Class` is read and reported, but it is not what "how documented is this
/// thesaurus?" is asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentationCoverage {
    /// The property counted.
    pub kind: NoteKind,
    /// How many concepts carry at least one note under it.
    pub concepts: usize,
    /// How many notes there are under it, across those concepts.
    pub notes: usize,
    /// How many of those notes we inferred rather than read — always under S17, so always zero
    /// for every kind except [`NoteKind::Note`].
    pub inferred: usize,
    /// How many reached this property through an `rdfs:subPropertyOf` the vocabulary declared
    /// itself, rather than through S17 or through being stated.
    ///
    /// Counted apart from `inferred` because the two answer different questions. An S17 lift is
    /// the specification's own arithmetic and needs no explaining; a refinement means the
    /// vocabulary has note properties of its own, and an author reading a coverage table wants to
    /// know that the number in front of them includes statements written with a property whose
    /// name does not appear in it.
    pub refined: usize,
}

impl fmt::Display for DocumentationCoverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<20}{:>7} concept(s), {} note(s)",
            self.kind.to_string(),
            self.concepts,
            self.notes
        )?;
        if self.inferred > 0 {
            write!(f, ", {} inferred under S17", self.inferred)?;
        }
        if self.refined > 0 {
            write!(f, ", {} through a declared refinement", self.refined)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_round_trips_through_its_iri() {
        for kind in NoteKind::ALL {
            assert_eq!(NoteKind::from_iri(&kind.iri()), Some(kind));
        }
    }

    /// The constants and the enum must not drift apart: the builder matches on the constants and
    /// the model reports the enum, so a typo in one would silently stop reading a property.
    #[test]
    fn the_constants_are_the_enums_iris() {
        assert_eq!(NoteKind::Note.iri(), SKOS_NOTE);
        assert_eq!(NoteKind::ChangeNote.iri(), SKOS_CHANGE_NOTE);
        assert_eq!(NoteKind::Definition.iri(), SKOS_DEFINITION);
        assert_eq!(NoteKind::EditorialNote.iri(), SKOS_EDITORIAL_NOTE);
        assert_eq!(NoteKind::Example.iri(), SKOS_EXAMPLE);
        assert_eq!(NoteKind::HistoryNote.iri(), SKOS_HISTORY_NOTE);
        assert_eq!(NoteKind::ScopeNote.iri(), SKOS_SCOPE_NOTE);
    }

    /// A SKOS IRI that is not one of the seven is not a note, and neither is a lookalike from
    /// another namespace. `skos:notation` is the trap: it is §6, it is next to these in the
    /// specification, and it is not a documentation property.
    #[test]
    fn nothing_else_is_a_documentation_property() {
        assert_eq!(
            NoteKind::from_iri("http://www.w3.org/2004/02/skos/core#notation"),
            None
        );
        assert_eq!(
            NoteKind::from_iri("http://www.w3.org/2004/02/skos/core#prefLabel"),
            None
        );
        assert_eq!(
            NoteKind::from_iri("http://purl.org/dc/terms/description"),
            None
        );
        assert_eq!(NoteKind::from_iri("http://example.org/ns#definition"), None);
    }

    /// S17 names six sub-properties, not seven, and the hierarchy is one step deep. Both halves
    /// matter: the first is the specification's own list, and the second is why the lift in
    /// `CoreModelBuilder` needs no cycle guard.
    #[test]
    fn six_kinds_are_sub_properties_of_note_and_note_is_not_one() {
        let lifted: Vec<_> = NoteKind::ALL
            .into_iter()
            .filter(|kind| kind.super_property().is_some())
            .collect();
        assert_eq!(lifted.len(), 6);
        assert_eq!(NoteKind::Note.super_property(), None);
        for kind in lifted {
            let (parent, rule) = match kind.super_property() {
                Some(pair) => pair,
                None => unreachable!("filtered above"),
            };
            assert_eq!(parent, NoteKind::Note);
            assert_eq!(rule, SkosRule::S17);
            assert_eq!(parent.super_property(), None, "the lift must not chain");
        }
    }

    #[test]
    fn an_origin_says_which_rule_it_came_from() {
        assert_eq!(NoteOrigin::Asserted.to_string(), "asserted");
        assert_eq!(
            NoteOrigin::Entailed(SkosRule::S17).to_string(),
            "inferred, S17"
        );
        assert_eq!(
            NoteOrigin::Refined {
                property: "ex:usageNote".to_owned()
            }
            .to_string(),
            "inferred from ex:usageNote, rdfs7"
        );
    }
}
