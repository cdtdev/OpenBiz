//! `rdfs:subPropertyOf` — the extension point §7.1 hands a vocabulary, and what it costs to read.
//!
//! §7.1 of the SKOS Reference: the seven documentation properties "provide a set of extension
//! points for defining more specific types of note". An enterprise thesaurus takes that offer
//! routinely — `ex:usageNote rdfs:subPropertyOf skos:scopeNote` is how ISO 25964's *scope note*
//! and a house style guide's *usage note* coexist without either pretending to be the other.
//!
//! Under RDFS that declaration entails a `skos:scopeNote` for every statement made with
//! `ex:usageNote`, and [`S17`](crate::SkosRule::S17) then entails a `skos:note`. Until this module
//! existed we read no `rdfs:subPropertyOf` at all, so an extended thesaurus reported as *less*
//! documented than it is, and the report gave no hint it was looking past something.
//!
//! # Why this is a second pass and not a bigger `match` arm
//!
//! [`CoreModelBuilder`](crate::CoreModelBuilder) is a one-pass stream, and a declaration may
//! arrive after every statement that uses it — RDF has no document order and a store has no
//! order at all. So a single pass can only do this by **buffering every statement it does not
//! recognise** until the declarations are in, which on a graph carrying `dct:created`,
//! `foaf:name` and an organisation's own metadata means buffering most of the graph to find the
//! handful of statements that turn out to matter. `openbiz inspect`'s promise is that "peak
//! memory is the model rather than the graph", and that is the promise buffering breaks.
//!
//! A second pass over the *source* buffers nothing: it reads `rdfs:subPropertyOf` and discards
//! everything else, so what it holds is the property graph — the number of properties a
//! vocabulary declares, not the number of statements it makes with them. That is smaller by
//! orders of magnitude on every real thesaurus, and it is the reason this is materialised into a
//! map while `docs/adr/0025` walks the concept hierarchy on demand instead. The two answers look
//! opposite and are the same arithmetic: materialise what is small and bounded by the schema,
//! walk what is large and bounded by the data.
//!
//! # The budget is shared across the resolution, which is iteration 30's lesson
//!
//! The chain is graph-controlled: `ex:a rdfs:subPropertyOf ex:b rdfs:subPropertyOf
//! skos:definition` is legal, so is a cycle, and so is a chain a thousand long. [`RefinementBound`]
//! guards both, and `max_steps` is spent **across the whole resolution** rather than per property.
//! A per-property budget times one walk per property is not a bound — that is exactly the defect
//! `docs/adr/0027` found in the disjointness sweep, where a prose comment described a limit the
//! code did not impose. Stated here so the next reader does not have to rediscover it.
//!
//! When the bound is reached, resolution stops and says so
//! ([`Finding::RefinementBoundReached`](crate::Finding::RefinementBoundReached), at
//! [`Severity::Unchecked`](crate::Severity::Unchecked)): the properties it never resolved read as
//! undocumented, and a report that stayed silent about that would be claiming a completeness it
//! does not have.
//!
//! # What this deliberately does not do
//!
//! - **It does not re-derive S17 from a graph's own copy of the SKOS ontology.** A vocabulary that
//!   imports SKOS carries `skos:definition rdfs:subPropertyOf skos:note` as a statement, and
//!   citing that copy rather than the specification would make the same conclusion's explanation
//!   depend on whether the customer happened to import the ontology. Those edges are skipped and
//!   S17 answers for them.
//! - **It resolves note properties only.** `skosxl:labelRelation`'s refinement is the same gap and
//!   was meant to be closed by the same mechanism, but B.4.4.1 warns that a sub-property of a
//!   symmetric property is not necessarily symmetric, so that one cannot simply reuse this
//!   resolution — it needs a decision about closure that this item does not make. `docs/UNTESTED.md`
//!   records that it is still open, and the resolution here is written against a target set rather
//!   than hard-wired to notes so the decision has somewhere to land.
//! - **It does not read `owl:subPropertyOf` or an inverse.** There is no such property; RDFS is
//!   the whole of the extension mechanism §7.1 points at.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::model::{RdfsRule, Statement, Term};
use crate::notes::NoteKind;
use crate::ns;

/// `rdfs:subPropertyOf` — the one predicate this pass reads.
pub const RDFS_SUB_PROPERTY_OF: &str = "http://www.w3.org/2000/01/rdf-schema#subPropertyOf";

/// How far the resolution of a vocabulary's own property refinements may go before it stops.
///
/// Both halves are a backstop against a graph that declares a pathological property hierarchy,
/// not a product limit — a real thesaurus declares tens of refinements, not thousands. See the
/// module documentation for why `max_steps` is spent across the whole resolution rather than per
/// property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefinementBound {
    /// How many distinct properties may be declared as sub-properties of something.
    pub max_properties: usize,
    /// How many `rdfs:subPropertyOf` edges the resolution may follow **in total**.
    pub max_steps: usize,
}

impl RefinementBound {
    /// The default: 10 000 declared properties and 100 000 edges followed across the resolution.
    ///
    /// Chosen against the schema rather than the data. A vocabulary declaring ten thousand of its
    /// own note properties has a modelling problem this tool cannot fix, and the edge budget is
    /// ten per property so a deep chain is affordable and a dense one is not.
    pub const DEFAULT: Self = RefinementBound {
        max_properties: 10_000,
        max_steps: 100_000,
    };
}

impl Default for RefinementBound {
    fn default() -> Self {
        RefinementBound::DEFAULT
    }
}

/// Why the resolution stopped early, and what was left unresolved when it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefinementExhaustion {
    /// How many declared properties were resolved before it stopped.
    pub resolved: usize,
    /// How many were left. Statements made with these read as non-SKOS and are dropped.
    pub unresolved: usize,
    /// How many `rdfs:subPropertyOf` edges had been followed when it stopped.
    pub steps_walked: usize,
}

/// What a vocabulary's own `rdfs:subPropertyOf` declarations entail about its notes.
///
/// Build one with [`PropertyRefinements::builder`], pushing the same statements you will push into
/// [`CoreModelBuilder`](crate::CoreModelBuilder) — everything that is not an `rdfs:subPropertyOf`
/// declaration between two IRIs is discarded immediately, so this pass holds the property graph
/// and nothing else.
///
/// [`PropertyRefinements::default()`] is the empty one: it entails nothing, which is what every
/// caller that does not care about refinements gets and is exactly the behaviour that shipped
/// before this module existed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyRefinements {
    /// Property IRI → the note kinds it entails, each with the chain of properties that reached
    /// it. The chain starts with the property itself and ends with the SKOS property.
    entailed: BTreeMap<String, BTreeMap<NoteKind, Vec<String>>>,
    /// How many `rdfs:subPropertyOf` statements were read, including ones that entail nothing.
    declarations: usize,
    /// Set when [`RefinementBound`] stopped the resolution.
    exhaustion: Option<RefinementExhaustion>,
}

impl PropertyRefinements {
    /// Start reading a graph's property declarations.
    pub fn builder() -> PropertyRefinementsBuilder {
        PropertyRefinementsBuilder {
            edges: BTreeMap::new(),
            declarations: 0,
            bound: RefinementBound::DEFAULT,
            overflowed: 0,
        }
    }

    /// Read a graph's property declarations from statements already in hand.
    pub fn from_statements(statements: impl IntoIterator<Item = Statement>) -> Self {
        let mut builder = PropertyRefinements::builder();
        for statement in statements {
            builder.push(statement);
        }
        builder.build()
    }

    /// The note kinds a predicate entails, with the chain of properties that reached each.
    ///
    /// Empty for every predicate a vocabulary has not refined, which is almost all of them.
    pub fn note_kinds(&self, predicate: &str) -> Option<&BTreeMap<NoteKind, Vec<String>>> {
        self.entailed.get(predicate)
    }

    /// Whether anything at all was entailed. A vocabulary that declares no refinements — the
    /// common case — resolves to an empty set and costs the model nothing.
    pub fn is_empty(&self) -> bool {
        self.entailed.is_empty()
    }

    /// How many properties were found to refine a documentation property.
    pub fn refined_properties(&self) -> usize {
        self.entailed.len()
    }

    /// How many `rdfs:subPropertyOf` statements were read, whatever they turned out to entail.
    pub fn declarations(&self) -> usize {
        self.declarations
    }

    /// Set when [`RefinementBound`] stopped the resolution before it finished.
    pub fn exhaustion(&self) -> Option<RefinementExhaustion> {
        self.exhaustion
    }

    /// Every refined property and what it entails, in a stable order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &BTreeMap<NoteKind, Vec<String>>)> {
        self.entailed.iter()
    }

    /// The chain that reached `kind` from `predicate`, rendered as the `rdfs5` steps that compose
    /// it — or `None` when the chain is a single declared step, which needs no composing.
    ///
    /// This is the explanation of *why* `ex:a rdfs:subPropertyOf skos:definition` holds when the
    /// graph only ever said `ex:a rdfs:subPropertyOf ex:b` and `ex:b rdfs:subPropertyOf
    /// skos:definition`. Without it a reader looking for the premise in the source file would not
    /// find it, which is the failure mode `docs/adr/0025` calls a verdict without an explanation.
    pub fn composition(&self, predicate: &str, kind: NoteKind) -> Option<(String, String)> {
        let chain = self.entailed.get(predicate)?.get(&kind)?;
        if chain.len() < 3 {
            return None;
        }
        let steps: Vec<String> = chain
            .windows(2)
            .map(|pair| format!("{} rdfs:subPropertyOf {}", short(&pair[0]), short(&pair[1])))
            .collect();
        Some((
            format!("{} rdfs:subPropertyOf {}", short(predicate), kind),
            steps.join(", and "),
        ))
    }
}

/// Reads `rdfs:subPropertyOf` out of a statement stream and resolves what it entails.
#[derive(Debug, Clone)]
pub struct PropertyRefinementsBuilder {
    edges: BTreeMap<String, BTreeSet<String>>,
    declarations: usize,
    bound: RefinementBound,
    /// Declarations refused because `max_properties` was already reached.
    overflowed: usize,
}

impl PropertyRefinementsBuilder {
    /// Use a different bound. The default is [`RefinementBound::DEFAULT`]; a test lowers it to
    /// reach the exhausted path without generating ten thousand properties to do it.
    pub fn with_bound(mut self, bound: RefinementBound) -> Self {
        self.bound = bound;
        self
    }

    /// Offer one statement. Anything that is not an `rdfs:subPropertyOf` between two IRIs is
    /// discarded without being counted against the bound.
    ///
    /// A blank node in subject position is dropped rather than recorded: RDF requires a predicate
    /// to be an IRI, so a blank node can never *be* the property being refined and an edge from
    /// one can never license anything. Dropping it is not leniency — there is no conclusion to
    /// draw.
    pub fn push(&mut self, statement: Statement) {
        if statement.predicate != RDFS_SUB_PROPERTY_OF {
            return;
        }
        let Some(sub) = statement.subject.as_iri() else {
            return;
        };
        let Term::Node(object) = &statement.object else {
            return;
        };
        let Some(sup) = object.as_iri() else {
            return;
        };

        self.declarations += 1;

        // S17 already licenses a specific note property being a sub-property of `skos:note`, and
        // a graph that imports the SKOS ontology carries its own copy of that statement. Citing
        // the copy would make the explanation of a conclusion depend on whether the customer
        // imported the ontology, so the edge is read, counted, and then not used.
        if NoteKind::from_iri(sub).is_some() && sup == NoteKind::Note.iri() {
            return;
        }

        if !self.edges.contains_key(sub) && self.edges.len() >= self.bound.max_properties {
            self.overflowed += 1;
            return;
        }
        self.edges
            .entry(sub.to_owned())
            .or_default()
            .insert(sup.to_owned());
    }

    /// Resolve every declared property against the seven documentation properties.
    pub fn build(self) -> PropertyRefinements {
        let mut entailed: BTreeMap<String, BTreeMap<NoteKind, Vec<String>>> = BTreeMap::new();
        let mut steps = 0usize;
        let mut resolved = 0usize;
        let mut exhaustion = None;

        let starts: Vec<&String> = self.edges.keys().collect();
        for (index, start) in starts.iter().enumerate() {
            if steps >= self.bound.max_steps {
                exhaustion = Some(RefinementExhaustion {
                    resolved,
                    unresolved: starts.len() - index + self.overflowed,
                    steps_walked: steps,
                });
                break;
            }
            // A property that is itself one of the seven is not resolved *from*: a statement made
            // with `skos:definition` is read directly, and anything it refines beyond `skos:note`
            // is still reached, because the walk below starts from every declared property
            // including those. What is skipped is only the S17 edge, above.
            let found = self.walk(start, &mut steps);
            resolved = index + 1;
            if !found.is_empty() {
                entailed.insert((*start).clone(), found);
            }
        }

        PropertyRefinements {
            entailed,
            declarations: self.declarations,
            exhaustion,
        }
    }

    /// Breadth-first upward from one property, spending `steps` from the shared budget.
    ///
    /// Breadth-first so the chain a derivation shows is the shortest one that reaches the
    /// conclusion — a reader checking the explanation against the source file should not be handed
    /// a nine-step detour when a two-step path exists. The visited set is the cycle guard:
    /// `ex:a rdfs:subPropertyOf ex:b rdfs:subPropertyOf ex:a` terminates and entails nothing.
    fn walk(&self, start: &str, steps: &mut usize) -> BTreeMap<NoteKind, Vec<String>> {
        let mut found: BTreeMap<NoteKind, Vec<String>> = BTreeMap::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut came_from: BTreeMap<&str, &str> = BTreeMap::new();
        let mut queue: VecDeque<&str> = VecDeque::new();

        seen.insert(start);
        queue.push_back(start);

        while let Some(current) = queue.pop_front() {
            let Some(supers) = self.edges.get(current) else {
                continue;
            };
            for sup in supers {
                if *steps >= self.bound.max_steps {
                    return found;
                }
                *steps += 1;
                if !seen.insert(sup.as_str()) {
                    continue;
                }
                came_from.insert(sup.as_str(), current);
                if let Some(kind) = NoteKind::from_iri(sup) {
                    found
                        .entry(kind)
                        .or_insert_with(|| chain(start, sup.as_str(), &came_from));
                }
                queue.push_back(sup.as_str());
            }
        }

        found
    }
}

/// Reconstruct the path from `start` to `end`, `start` first.
fn chain(start: &str, end: &str, came_from: &BTreeMap<&str, &str>) -> Vec<String> {
    let mut path = vec![end.to_owned()];
    let mut cursor = end;
    while cursor != start {
        let Some(previous) = came_from.get(cursor) else {
            // Unreachable: every node in `came_from` was reached from `start`. Returning the
            // partial path rather than panicking, for the reason `CLAUDE.md` §6 gives — a broken
            // explanation beats an aborted report on a customer's vocabulary.
            break;
        };
        cursor = previous;
        path.push(cursor.to_owned());
    }
    path.reverse();
    path
}

/// A property IRI as a CURIE where we know the prefix, angle-bracketed otherwise.
///
/// Duplicated from `model::curie` rather than shared because that one is private to the model and
/// this module must not depend on it; the two are asserted equal by a test in `model`.
fn short(iri: &str) -> String {
    for (prefix, namespace) in [
        ("skos", ns::SKOS),
        ("skosxl", ns::SKOSXL),
        ("rdf", ns::RDF),
        ("rdfs", ns::RDFS),
    ] {
        if let Some(local) = iri.strip_prefix(namespace) {
            return format!("{prefix}:{local}");
        }
    }
    format!("<{iri}>")
}

/// The rule that licenses a refined statement, for a derivation to cite.
pub const REFINEMENT_RULE: RdfsRule = RdfsRule::Rdfs7;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Node;

    fn sub(sub: &str, sup: &str) -> Statement {
        Statement {
            subject: Node::iri(sub),
            predicate: RDFS_SUB_PROPERTY_OF.to_owned(),
            object: Term::Node(Node::iri(sup)),
        }
    }

    const USAGE: &str = "http://example.org/ns#usageNote";
    const HOUSE: &str = "http://example.org/ns#houseNote";

    /// §7.1's own worked shape: one declaration, one entailed kind, chain of two.
    #[test]
    fn one_declaration_entails_the_property_it_names() {
        let refinements =
            PropertyRefinements::from_statements([sub(USAGE, &NoteKind::ScopeNote.iri())]);
        let kinds = refinements.note_kinds(USAGE).expect("usageNote is refined");
        assert_eq!(kinds.len(), 1);
        assert_eq!(
            kinds.get(&NoteKind::ScopeNote),
            Some(&vec![USAGE.to_owned(), NoteKind::ScopeNote.iri()])
        );
        assert_eq!(refinements.declarations(), 1);
        assert_eq!(refinements.refined_properties(), 1);
        assert!(refinements.exhaustion().is_none());
    }

    /// A chain the graph controls: `ex:house → ex:usage → skos:scopeNote`. RDFS makes
    /// `rdfs:subPropertyOf` transitive (rdfs5), so the far end is entailed too.
    #[test]
    fn a_chain_reaches_the_far_end_and_records_the_whole_path() {
        let refinements = PropertyRefinements::from_statements([
            sub(HOUSE, USAGE),
            sub(USAGE, &NoteKind::ScopeNote.iri()),
        ]);
        let chain = refinements
            .note_kinds(HOUSE)
            .and_then(|kinds| kinds.get(&NoteKind::ScopeNote))
            .expect("the chain reaches scopeNote");
        assert_eq!(
            chain,
            &vec![
                HOUSE.to_owned(),
                USAGE.to_owned(),
                NoteKind::ScopeNote.iri()
            ]
        );
        let (conclusion, premise) = refinements
            .composition(HOUSE, NoteKind::ScopeNote)
            .expect("a two-step chain composes");
        assert_eq!(
            conclusion,
            "<http://example.org/ns#houseNote> rdfs:subPropertyOf skos:scopeNote"
        );
        assert!(
            premise.contains("rdfs:subPropertyOf <http://example.org/ns#usageNote>"),
            "{premise}"
        );
        assert!(
            premise.contains("rdfs:subPropertyOf skos:scopeNote"),
            "{premise}"
        );
    }

    /// A one-step declaration is already in the graph, so there is nothing to compose and nothing
    /// to explain. Printing a composition for it would be noise pretending to be provenance.
    #[test]
    fn a_single_step_declaration_needs_no_composition() {
        let refinements =
            PropertyRefinements::from_statements([sub(USAGE, &NoteKind::ScopeNote.iri())]);
        assert_eq!(refinements.composition(USAGE, NoteKind::ScopeNote), None);
    }

    /// One property may refine two of the seven. RDFS permits it and both conclusions follow.
    #[test]
    fn a_property_may_refine_more_than_one_of_the_seven() {
        let refinements = PropertyRefinements::from_statements([
            sub(USAGE, &NoteKind::ScopeNote.iri()),
            sub(USAGE, &NoteKind::EditorialNote.iri()),
        ]);
        let kinds = refinements.note_kinds(USAGE).expect("refined");
        assert_eq!(
            kinds.keys().copied().collect::<Vec<_>>(),
            vec![NoteKind::ScopeNote, NoteKind::EditorialNote]
        );
    }

    /// A cycle terminates and entails nothing. The graph controls the chain, so this is reachable
    /// from a customer's file and not merely a hostile test.
    #[test]
    fn a_cycle_terminates_and_entails_nothing() {
        let refinements =
            PropertyRefinements::from_statements([sub(HOUSE, USAGE), sub(USAGE, HOUSE)]);
        assert!(refinements.is_empty());
        assert_eq!(refinements.declarations(), 2);
    }

    /// A cycle that also reaches out to a note property still entails the note. Terminating is
    /// not the same as giving up.
    #[test]
    fn a_cycle_that_also_reaches_a_note_property_still_entails_it() {
        let refinements = PropertyRefinements::from_statements([
            sub(HOUSE, USAGE),
            sub(USAGE, HOUSE),
            sub(USAGE, &NoteKind::Definition.iri()),
        ]);
        assert!(refinements
            .note_kinds(HOUSE)
            .and_then(|kinds| kinds.get(&NoteKind::Definition))
            .is_some());
    }

    /// A graph that imports the SKOS ontology carries S17 as statements. We read them, count them,
    /// and do not use them — the specification is the citation, not the customer's copy of it.
    #[test]
    fn the_graphs_own_copy_of_s17_is_read_and_not_re_derived() {
        let statements: Vec<Statement> = NoteKind::ALL
            .into_iter()
            .filter(|kind| *kind != NoteKind::Note)
            .map(|kind| sub(&kind.iri(), &NoteKind::Note.iri()))
            .collect();
        let refinements = PropertyRefinements::from_statements(statements);
        assert!(
            refinements.is_empty(),
            "S17's own edges must not be re-derived"
        );
        assert_eq!(refinements.declarations(), 6);
    }

    /// But a graph that genuinely extends SKOS's own structure — one of the seven refining
    /// another — is not S17 and is read. It is unusual and it is legal RDFS.
    #[test]
    fn one_of_the_seven_refining_another_is_not_s17_and_is_read() {
        let refinements = PropertyRefinements::from_statements([sub(
            &NoteKind::Example.iri(),
            &NoteKind::ScopeNote.iri(),
        )]);
        assert!(refinements
            .note_kinds(&NoteKind::Example.iri())
            .and_then(|kinds| kinds.get(&NoteKind::ScopeNote))
            .is_some());
    }

    /// A declaration whose ends are not both IRIs licenses nothing. A blank node can never be a
    /// predicate, so there is no statement it could ever apply to.
    #[test]
    fn a_declaration_that_is_not_between_two_iris_is_dropped() {
        let literal = Statement {
            subject: Node::iri(USAGE),
            predicate: RDFS_SUB_PROPERTY_OF.to_owned(),
            object: Term::Literal(crate::model::Literal {
                value: "skos:scopeNote".to_owned(),
                language: None,
                datatype: crate::XSD_STRING.to_owned(),
            }),
        };
        let blank = Statement {
            subject: Node::blank("b0"),
            predicate: RDFS_SUB_PROPERTY_OF.to_owned(),
            object: Term::Node(Node::iri(NoteKind::ScopeNote.iri())),
        };
        let refinements = PropertyRefinements::from_statements([literal, blank]);
        assert!(refinements.is_empty());
        assert_eq!(refinements.declarations(), 0);
    }

    /// A refinement of something that is not a documentation property entails nothing here. Most
    /// `rdfs:subPropertyOf` in the wild is of this shape and must cost the model nothing.
    #[test]
    fn a_refinement_of_a_non_note_property_entails_nothing() {
        let refinements = PropertyRefinements::from_statements([sub(
            USAGE,
            "http://purl.org/dc/terms/description",
        )]);
        assert!(refinements.is_empty());
        assert_eq!(refinements.declarations(), 1);
    }

    /// The step budget is spent across the whole resolution, not per property. This is the shape
    /// `docs/adr/0027` found broken elsewhere, so it is asserted rather than assumed: twenty
    /// properties each one step from a note property, with a budget of five, must leave most of
    /// them unresolved and **say so**.
    #[test]
    fn the_step_budget_is_shared_across_every_property() {
        let statements: Vec<Statement> = (0..20)
            .map(|n| {
                sub(
                    &format!("http://example.org/ns#p{n}"),
                    &NoteKind::Definition.iri(),
                )
            })
            .collect();
        let mut builder = PropertyRefinements::builder().with_bound(RefinementBound {
            max_properties: 100,
            max_steps: 5,
        });
        for statement in statements {
            builder.push(statement);
        }
        let refinements = builder.build();

        let exhaustion = refinements
            .exhaustion()
            .expect("a shared budget of five must not resolve twenty properties");
        assert_eq!(exhaustion.resolved, 5);
        assert_eq!(exhaustion.unresolved, 15);
        assert_eq!(exhaustion.steps_walked, 5);
        assert_eq!(refinements.refined_properties(), 5);
    }

    /// The property ceiling refuses new properties and counts them into what was left unresolved,
    /// so the report's "unresolved" number is the truth and not just the walk's share of it.
    #[test]
    fn the_property_ceiling_counts_what_it_refused() {
        let mut builder = PropertyRefinements::builder().with_bound(RefinementBound {
            max_properties: 2,
            max_steps: 0,
        });
        for n in 0..6 {
            builder.push(sub(
                &format!("http://example.org/ns#p{n}"),
                &NoteKind::Definition.iri(),
            ));
        }
        let refinements = builder.build();
        let exhaustion = refinements.exhaustion().expect("both bounds are reached");
        assert_eq!(
            exhaustion.unresolved,
            2 + 4,
            "two unwalked and four refused"
        );
        assert_eq!(refinements.declarations(), 6, "all six were read");
    }

    /// The empty set is the default, and it is what every caller that does not do the second pass
    /// gets. It must be free rather than merely cheap.
    #[test]
    fn the_default_entails_nothing() {
        let refinements = PropertyRefinements::default();
        assert!(refinements.is_empty());
        assert_eq!(refinements.declarations(), 0);
        assert_eq!(refinements.note_kinds(USAGE), None);
        assert!(refinements.exhaustion().is_none());
    }

    /// The CURIE renderer must agree with the model's, or a derivation's premise and its
    /// conclusion would spell the same property two different ways.
    #[test]
    fn skos_and_rdfs_iris_render_as_curies() {
        assert_eq!(short(&NoteKind::ScopeNote.iri()), "skos:scopeNote");
        assert_eq!(short(RDFS_SUB_PROPERTY_OF), "rdfs:subPropertyOf");
        assert_eq!(short(USAGE), "<http://example.org/ns#usageNote>");
    }
}
