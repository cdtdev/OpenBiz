//! `openbiz deprecate` — retire a concept without deleting it, and say what still points at it.
//!
//! The fourth of `docs/BUILD-PLAN.md`'s bulk operations, and the one the other three kept pointing
//! at: a merge's report says "deprecating a concept in place is a different change", and a split's
//! says "retiring the original is a deprecation, which keeps the trail an auditor needs". This is
//! that change.
//!
//! Nothing reaches the vocabulary here: the statements are computed, staged as one candidate, and
//! printed; `openbiz approve` applies them inside one transaction. That is `CLAUDE.md` §3.
//!
//! # Why the report is mostly about what it did not do
//!
//! Retiring a concept is three statements and takes a second. What it *leaves* is the work: a live
//! child still under a retired parent, a scheme whose browse tree is headed by something nobody
//! should use, a collection that still lists it, and every mapping another vocabulary made to it.
//! None of those is wrong — the concept still means what it meant — and none of them can be
//! decided from the graph. So the report counts and names them, and it says so **before** the
//! diff, which is the same order `openbiz split` settled on for the same reason.
//!
//! # Why a replacement repoints nothing
//!
//! `dcterms:isReplacedBy` is a signpost. The vocabulary still says everything it said, and every
//! system holding the old IRI keeps resolving it — which is the entire point of retiring rather
//! than deleting. Rewriting every reference is `openbiz merge`, and it does it by making the old
//! IRI stop existing. There is no operation here that does both, and there deliberately is not
//! one yet: it is in `docs/PROPOSED.md` rather than folded quietly into this item.
//!
//! # Why a command and not an endpoint
//!
//! The same objection every writing path in this build records: there is no authentication yet,
//! and `POST /api/deprecate` would be an unauthenticated way to retire concepts in somebody's
//! thesaurus. The candidate seam over HTTP is its own plan item and lands with the identity.

use openbiz_skos::{CoreModel, Deprecation, DeprecationScan, Node, Stranded};
use openbiz_store::{Candidate, CandidateSource, GraphId, Provenance, Store};

use crate::cli::{actor, CommandError};
use crate::staging::{borrowed, elsewhere, newly_broken};

/// Propose retiring `concept`, optionally recording what replaces it and why.
///
/// Reads the vocabulary twice — once as SKOS, once as the raw statements that carry a status
/// `CoreModel` has no reading of — computes the change, and stages it as a candidate. **Nothing is
/// written to the vocabulary, and nothing is removed.**
pub fn deprecate(
    store: &Store,
    graph: &str,
    concept: &str,
    replaced_by: Option<&str>,
    note: Option<&str>,
    language: Option<&str>,
) -> Result<String, CommandError> {
    let vocabulary = GraphId::vocabulary(graph)?;
    let model = crate::inspect::read(store, graph)?;

    let concept = Node::iri(concept);
    let replacement = replaced_by.map(Node::iri);
    let mut scan = DeprecationScan::builder(concept.clone(), replacement.clone());
    store.for_each_statement(graph, |statement| {
        scan.push(crate::inspect::convert(statement))
    })?;
    let deprecation = model
        .deprecate(&scan.build(), note, language)
        .map_err(CommandError::Deprecate)?;

    // The change is computed; now check what it would leave behind. None of these three
    // statements is SKOS, so it is hard to see how one could break a SKOS condition — and that is
    // exactly the reasoning iteration 43 found to be wrong about a merge. The whole condition set
    // is run, and the argument for it is in `crate::staging`.
    let broken = newly_broken(store, graph, &model, deprecation.additions(), &[])?;
    if !broken.is_empty() {
        return Err(CommandError::BreaksIntegrity {
            operation: "deprecate",
            conditions: Box::new(crate::staging::BrokenConditions {
                graph: graph.to_owned(),
                change: format!("retiring {concept}"),
                broken,
            }),
        });
    }

    let mentioned = elsewhere(store, graph, &concept)?;
    // A replacement this vocabulary has never heard of is ordinary — a term retired in favour of
    // one next door — so the domain model allows it. What the operator needs to know is whether it
    // is that, or a typo, and the store is the only place that can tell them apart.
    let replacement_known = match deprecation.replacement() {
        Some(replacement) if model.resource(replacement).is_none() => {
            Some(elsewhere(store, graph, replacement)?)
        }
        _ => None,
    };

    let provenance = Provenance {
        source: CandidateSource::BulkEdit,
        agent: format!("{} (openbiz deprecate)", actor()?),
        note: match deprecation.replacement() {
            Some(replacement) => format!("deprecated {concept}, replaced by {replacement}"),
            None => format!("deprecated {concept}"),
        },
        // A computed deprecation is not a guess; see the same note on `openbiz move`.
        confidence: None,
    };

    let additions = borrowed(deprecation.additions());
    let candidate = store.propose_edit(&vocabulary, &additions, &[], &provenance)?;

    Ok(report(
        graph,
        &model,
        &deprecation,
        &mentioned,
        replacement_known.as_deref(),
        &candidate,
    ))
}

/// What the operator reads back, in the order they need it.
///
/// The retirement itself is one line, because it is one decision. Everything after it is what the
/// operator now has to decide about, and it comes **before** the diff: a reader who stops at
/// "retired, replaced by X" believes the job is finished, and the concept still has children.
fn report(
    graph: &str,
    model: &CoreModel,
    deprecation: &Deprecation,
    elsewhere: &[(String, usize)],
    replacement_elsewhere: Option<&[(String, usize)]>,
    candidate: &Candidate,
) -> String {
    let concept = deprecation.concept();
    let mut out = String::new();

    out.push_str(&format!("{concept}{}\n", named_in(model, concept)));
    out.push_str(match deprecation.marks() {
        true => "deprecated — kept in the vocabulary, marked no longer current\n",
        false => "already deprecated — this only records what it is replaced by\n",
    });
    out.push_str(&format!("in {graph}\n"));

    match deprecation.replacement() {
        Some(replacement) => {
            out.push_str(&format!(
                "\nreplaced by {replacement}{}\n",
                named_in(model, replacement)
            ));
            out.push_str(
                "  a signpost and not a rewrite: nothing in this vocabulary is repointed, and \
                 every reference to the retired concept still resolves to it. Repointing them all \
                 is `openbiz merge`, which makes the retired IRI stop existing.\n",
            );
        }
        None => out.push_str(
            "\nwith nothing recorded as replacing it. A term can go out of use with nothing \
             taking its place; if something does, `openbiz deprecate <graph> <concept> \
             --replaced-by <iri>` records it later.\n",
        ),
    }

    if let Some(found) = replacement_elsewhere {
        let replacement = deprecation
            .replacement()
            .map(Node::to_string)
            .unwrap_or_default();
        match found.is_empty() {
            true => out.push_str(&format!(
                "\nwarning: nothing anywhere in this store says anything about {replacement}. \
                 dcterms:isReplacedBy may point at a concept in another system, which is ordinary \
                 — but so is a mistyped IRI, and they look identical from here. Check it before \
                 approving.\n"
            )),
            false => {
                out.push_str(&format!(
                    "\n{replacement} is not in this vocabulary. It is known here:\n"
                ));
                for (source, count) in found {
                    out.push_str(&format!("  {count} statements in {source}\n"));
                }
            }
        }
    }

    if let Some(note) = deprecation.note() {
        out.push_str(&format!(
            "\nrecorded as a skos:changeNote{}: {}\n",
            match &note.language {
                Some(tag) => format!(" in {tag}"),
                None => ", untagged — this concept has no one preferred-language label to take a \
                        tag from, and --language says which"
                    .to_owned(),
            },
            note.text
        ));
    }

    out.push_str(&stranded(model, deprecation.stranded(), concept));

    if !elsewhere.is_empty() {
        out.push_str(&format!(
            "\n{concept} is also mentioned outside this vocabulary. Nothing there changes and \
             nothing there breaks — but whoever owns it is pointing at a concept that is no longer \
             current, and only this graph says so:\n"
        ));
        for (source, count) in elsewhere {
            out.push_str(&format!("  {count} in {source}\n"));
        }
    }

    out.push_str("\nit would add:\n");
    for statement in deprecation.additions() {
        out.push_str(&format!("  {statement}\n"));
    }
    out.push_str("and remove nothing — that is what makes this a retirement and not a deletion.\n");

    out.push_str(&format!(
        "\nproposed candidate {} against {}. Nothing has been written to the vocabulary. Review \
         it with `openbiz candidate {}`, then `openbiz approve {}` or `openbiz reject {}`.\n",
        candidate.id(),
        candidate.target(),
        candidate.id(),
        candidate.id(),
        candidate.id(),
    ));

    out
}

/// What is still attached to the retired concept, which nothing here decided.
fn stranded(model: &CoreModel, left: &Stranded, concept: &Node) -> String {
    if left.is_empty() {
        return format!(
            "\nNothing in this vocabulary points at {concept} and it points at nothing, so \
             retiring it strands no one.\n"
        );
    }

    let mut out = String::from(
        "\nstill attached to it, and untouched — a deprecation retires a concept, it does not \
         decide what to do about everything joined to it:\n",
    );

    if !left.narrower.is_empty() {
        out.push_str(&format!(
            "  {} still below it, under a concept nobody should use again: re-parent {} with \
             `openbiz move`, or retire {} too\n",
            concepts(left.narrower.len()),
            match left.narrower.len() {
                1 => "it",
                _ => "each",
            },
            match left.narrower.len() {
                1 => "it",
                _ => "them",
            }
        ));
        for child in left.narrower.iter().take(5) {
            out.push_str(&format!("    {child}{}\n", named_in(model, child)));
        }
        if left.narrower.len() > 5 {
            out.push_str(&format!("    and {} more\n", left.narrower.len() - 5));
        }
    }
    if !left.top_concept_of.is_empty() {
        out.push_str(&format!(
            "  it heads the browse tree of {} scheme{}, where a retired concept is the first \
             thing a reader sees:\n",
            left.top_concept_of.len(),
            match left.top_concept_of.len() {
                1 => "",
                _ => "s",
            }
        ));
        for scheme in &left.top_concept_of {
            out.push_str(&format!("    {scheme}{}\n", named_in(model, scheme)));
        }
    }
    if !left.collections.is_empty() {
        out.push_str(&format!(
            "  {} still list{} it as a member:\n",
            match left.collections.len() {
                1 => "1 collection".to_owned(),
                many => format!("{many} collections"),
            },
            match left.collections.len() {
                1 => "s",
                _ => "",
            }
        ));
        for collection in &left.collections {
            out.push_str(&format!(
                "    {collection}{}\n",
                named_in(model, collection)
            ));
        }
    }
    if !left.broader.is_empty() {
        out.push_str(&format!(
            "  {} above it, which keeps a retired narrower concept\n",
            concepts(left.broader.len())
        ));
    }
    if !left.related.is_empty() {
        out.push_str(&format!(
            "  {} associatively linked to it\n",
            concepts(left.related.len())
        ));
    }
    if left.mapped_to > 0 {
        out.push_str(&format!(
            "  {} in other vocabularies it is mapped to, which now join a live concept to a \
             retired one\n",
            match left.mapped_to {
                1 => "1 resource".to_owned(),
                many => format!("{many} resources"),
            }
        ));
    }
    if left.incoming > 0 {
        out.push_str(&format!(
            "  {} in this vocabulary point at it in all, counting the statements SKOS has no \
             reading of\n",
            match left.incoming {
                1 => "1 statement".to_owned(),
                many => format!("{many} statements"),
            }
        ));
    }

    out
}

/// "3 concepts are" / "1 concept is".
fn concepts(count: usize) -> String {
    match count {
        1 => "1 concept is".to_owned(),
        many => format!("{many} concepts are"),
    }
}

/// A concept's preferred label in parentheses, or nothing when it has none.
fn named_in(model: &CoreModel, node: &Node) -> String {
    model
        .resource(node)
        .and_then(|resource| resource.display_label())
        .map(|label| format!(" ({label})"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use openbiz_skos::DeprecationError;
    use openbiz_store::{
        CandidateSource, CandidateState, Decision, GraphId, Provenance, RdfSyntax, Store,
    };

    use super::deprecate;
    use crate::cli::CommandError;

    const VOCABULARY: &str = "http://example.org/thesaurus";
    const OTHER: &str = "http://example.org/other";
    const WIRELESS: &str = "http://example.org/wireless";
    const RADIO: &str = "http://example.org/radio";

    /// A store holding `turtle` in one registered vocabulary, through the seam data really uses.
    fn store_with(turtle: &str) -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(directory.path()).expect("an openable store");
        load(&store, VOCABULARY, turtle);
        (directory, store)
    }

    fn load(store: &Store, graph: &str, turtle: &str) {
        let target = GraphId::vocabulary(graph).expect("a valid vocabulary IRI");
        store
            .create_vocabulary_graph(&target)
            .expect("a fresh registration");
        let candidate = store
            .propose_import(
                &target,
                RdfSyntax::Turtle,
                turtle.as_bytes(),
                &Provenance {
                    source: CandidateSource::Import,
                    agent: "test".to_owned(),
                    note: "fixture".to_owned(),
                    confidence: None,
                },
            )
            .expect("a well-formed proposal");
        store
            .decide(candidate.id(), Decision::Approve, "test")
            .expect("an approvable candidate");
    }

    /// A term that went out of use, with everything a retirement strands: a live child, a parent,
    /// an associative link, a mapping, a scheme it heads and a collection that lists it.
    const OBSOLETE: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix ex: <http://example.org/> .
        ex:scheme a skos:ConceptScheme .
        ex:telegraphy a skos:Concept ; skos:prefLabel "Telegraphy"@en ; skos:inScheme ex:scheme .
        ex:wireless a skos:Concept ; skos:prefLabel "Wireless telegraphy"@en ;
            skos:inScheme ex:scheme ; skos:topConceptOf ex:scheme ;
            skos:broader ex:telegraphy ; skos:related ex:signals ;
            skos:closeMatch <https://other.example/wireless> .
        ex:radio a skos:Concept ; skos:prefLabel "Radio"@en ; skos:inScheme ex:scheme .
        ex:signals a skos:Concept ; skos:prefLabel "Signals"@en ; skos:inScheme ex:scheme .
        ex:morse a skos:Concept ; skos:prefLabel "Morse code"@en ; skos:broader ex:wireless .
        ex:obsolete a skos:Collection ; skos:member ex:wireless .
    "#;

    fn retire(store: &Store, replaced_by: Option<&str>) -> Result<String, CommandError> {
        deprecate(store, VOCABULARY, WIRELESS, replaced_by, None, None)
    }

    /// The order is the argument of the command: what is *not* done comes before the diff, because
    /// a reader who stops at "retired, replaced by Radio" thinks the job is finished.
    #[test]
    fn a_retirement_says_what_it_stranded_before_it_shows_the_statements() {
        let (_directory, store) = store_with(OBSOLETE);
        let report = retire(&store, Some(RADIO)).expect("a retirement");

        let stranded = report
            .find("still attached to it")
            .expect("the report must say what it did not decide");
        let diff = report.find("it would add:").expect("and then the diff");
        assert!(
            stranded < diff,
            "the work still to do comes first: {report}"
        );

        assert!(
            report.contains("1 concept is still below it"),
            "the live child under a retired parent is the consequential one: {report}"
        );
        assert!(report.contains("<http://example.org/morse> (\"Morse code\"@en)"));
        assert!(report.contains("it heads the browse tree of 1 scheme"));
        assert!(report.contains("1 collection still lists it as a member"));
        assert!(report.contains("1 concept is above it"));
        assert!(report.contains("1 concept is associatively linked to it"));
        assert!(report.contains("1 resource in other vocabularies it is mapped to"));
    }

    #[test]
    fn a_retirement_stages_one_candidate_that_removes_nothing() {
        let (_directory, store) = store_with(OBSOLETE);
        let report = retire(&store, Some(RADIO)).expect("a retirement");
        assert!(
            report.contains("and remove nothing — that is what makes this a retirement"),
            "{report}"
        );

        let candidates = store.candidates().expect("the store's candidates");
        let staged = candidates
            .iter()
            .find(|candidate| candidate.state() == CandidateState::Proposed)
            .expect("one candidate waiting");
        assert_eq!(staged.target().iri(), VOCABULARY);
        assert_eq!(staged.provenance().source, CandidateSource::BulkEdit);
        assert!(
            staged.provenance().agent.ends_with("(openbiz deprecate)"),
            "the command that raised it is part of the trail: {}",
            staged.provenance().agent
        );
        assert!(
            staged.provenance().note.contains("replaced by"),
            "and so is the replacement: {}",
            staged.provenance().note
        );
    }

    /// The three statements, and no fourth one about the replacement.
    #[test]
    fn the_diff_is_the_marker_the_replacement_and_nothing_about_the_replacement_itself() {
        let (_directory, store) = store_with(OBSOLETE);
        let report = deprecate(
            &store,
            VOCABULARY,
            WIRELESS,
            Some(RADIO),
            Some("Superseded by broadcasting terms."),
            None,
        )
        .expect("a retirement");

        assert!(
            report.contains("owl:deprecated \"true\"^^<http://www.w3.org/2001/XMLSchema#boolean>"),
            "{report}"
        );
        assert!(report.contains("dcterms:isReplacedBy <http://example.org/radio>"));
        assert!(report.contains("skos:changeNote \"Superseded by broadcasting terms.\"@en"));
        assert!(
            !report.contains("<http://example.org/radio> dcterms:"),
            "nothing is said about the replacement itself: {report}"
        );
    }

    /// The signpost is the thing operators most often expect to be a rewrite, so the report says
    /// it is not, in the same breath as the replacement.
    #[test]
    fn the_report_says_a_replacement_repoints_nothing() {
        let (_directory, store) = store_with(OBSOLETE);
        let report = retire(&store, Some(RADIO)).expect("a retirement");

        assert!(report.contains("a signpost and not a rewrite"), "{report}");
        assert!(report.contains("`openbiz merge`"));
    }

    #[test]
    fn a_retirement_with_no_replacement_says_how_to_record_one_later() {
        let (_directory, store) = store_with(OBSOLETE);
        let report = retire(&store, None).expect("a retirement");

        assert!(
            report.contains("with nothing recorded as replacing it"),
            "{report}"
        );
        assert!(report.contains("--replaced-by"));
    }

    /// Ordinary governance: the replacement lives in the corporate vocabulary next door. The store
    /// can see that it exists, so the report says where rather than warning.
    #[test]
    fn a_replacement_in_another_vocabulary_in_this_store_is_reported_not_warned_about() {
        let (_directory, store) = store_with(OBSOLETE);
        load(
            &store,
            OTHER,
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            <http://example.org/other/radio> a skos:Concept ; skos:prefLabel "Radio"@en .
            "#,
        );

        let report = retire(&store, Some("http://example.org/other/radio")).expect("a retirement");

        assert!(
            report.contains("is not in this vocabulary. It is known here:"),
            "{report}"
        );
        assert!(report.contains("the vocabulary http://example.org/other"));
        assert!(!report.contains("nothing anywhere in this store"));
    }

    /// A mistyped IRI and a genuine cross-vocabulary replacement are identical statements. The
    /// store is the only thing that can tell them apart, and it says so rather than refusing.
    #[test]
    fn a_replacement_nothing_in_the_store_has_heard_of_is_warned_about() {
        let (_directory, store) = store_with(OBSOLETE);
        let report = retire(&store, Some("http://example.org/radioo")).expect("a retirement");

        assert!(
            report.contains("nothing anywhere in this store says anything about"),
            "{report}"
        );
        assert!(report.contains("so is a mistyped IRI"));
    }

    #[test]
    fn retiring_what_is_already_retired_is_refused_rather_than_proposing_nothing() {
        let (_directory, store) = store_with(OBSOLETE);
        retire(&store, None).expect("a first retirement");
        let candidate = store
            .candidates()
            .expect("the store's candidates")
            .into_iter()
            .find(|candidate| candidate.state() == CandidateState::Proposed)
            .expect("one waiting");
        store
            .decide(candidate.id(), Decision::Approve, "test")
            .expect("an approvable candidate");

        let refused = retire(&store, None).expect_err("a second retirement");
        assert!(
            matches!(
                refused,
                CommandError::Deprecate(DeprecationError::AlreadyDeprecated { .. })
            ),
            "{refused}"
        );
    }

    /// The workflow the second call exists for: retired when it went out of use, replacement
    /// agreed on later.
    #[test]
    fn a_replacement_can_be_recorded_against_a_concept_already_retired() {
        let (_directory, store) = store_with(OBSOLETE);
        retire(&store, None).expect("a first retirement");
        let candidate = store
            .candidates()
            .expect("the store's candidates")
            .into_iter()
            .find(|candidate| candidate.state() == CandidateState::Proposed)
            .expect("one waiting");
        store
            .decide(candidate.id(), Decision::Approve, "test")
            .expect("an approvable candidate");

        let report = retire(&store, Some(RADIO)).expect("a replacement recorded later");

        assert!(
            report.contains("already deprecated — this only records what it is replaced by"),
            "{report}"
        );
        assert!(report.contains("dcterms:isReplacedBy <http://example.org/radio>"));
        assert!(
            !report.contains("owl:deprecated"),
            "the marker is already there and is not proposed twice: {report}"
        );
    }

    #[test]
    fn a_concept_the_vocabulary_has_never_heard_of_is_refused() {
        let (_directory, store) = store_with(OBSOLETE);
        let refused = deprecate(
            &store,
            VOCABULARY,
            "http://example.org/nothing",
            None,
            None,
            None,
        )
        .expect_err("nothing to retire");

        assert!(
            matches!(
                refused,
                CommandError::Deprecate(DeprecationError::NoSuchConcept { .. })
            ),
            "{refused}"
        );
    }

    /// A concept with nothing hanging off it is the case where the report must not invent work.
    #[test]
    fn a_concept_nothing_points_at_strands_no_one() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            <http://example.org/wireless> a skos:Concept ; skos:prefLabel "Wireless"@en .
            "#,
        );
        let report = retire(&store, None).expect("a retirement");

        assert!(report.contains("strands no one"), "{report}");
        assert!(!report.contains("still attached to it"));
    }
}
