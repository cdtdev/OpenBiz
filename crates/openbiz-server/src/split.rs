//! `openbiz split` — divide one concept into several, and say plainly what it did not divide.
//!
//! The third of `docs/BUILD-PLAN.md`'s bulk operations. Nothing reaches the vocabulary here: the
//! parts are computed, staged as one candidate, and printed; `openbiz approve` applies them inside
//! one transaction. That is `CLAUDE.md` §3.
//!
//! # The half this does, and the half it refuses to guess
//!
//! A merge has one right answer for every statement it touches. A split has none: the concept is
//! being divided *because* its labels, its narrower concepts, its `skos:related` links and its
//! notes belong to different things, and which part each belongs to is the editorial judgement
//! nobody has made yet. So this creates the parts — IRI, preferred label, position, and a
//! `prov:wasDerivedFrom` back to the concept — leaves the original untouched, and ends its report
//! with everything still hanging off it and the command that apportions each kind.
//!
//! A report that stopped after "3 parts proposed" would read as if the job were done. It is not:
//! the split is the easy half.
//!
//! # Why it mints rather than taking IRIs
//!
//! The parts are new concepts, and `openbiz mint` exists precisely so nobody names a new concept
//! by copying an existing IRI and editing the end of it. This resolves the pattern exactly as
//! `openbiz mint` does — `--pattern`, then the vocabulary's recorded policy, then the convention
//! read off its own concepts — through the same function, so a deployment that has recorded a
//! policy gets parts named the way its curators name everything else. Each minted IRI is offered
//! back to the scan before the next is minted, so a numbered pattern gives three parts three
//! numbers rather than the same one three times.
//!
//! This is the second producer to mint under a recorded policy, which is the `docs/UNTESTED.md`
//! entry the minting item opened when it had only one.
//!
//! Minting also decides what happens when a part is named what something here is already named.
//! Under an **opaque** pattern the report warns and carries on, because a large vocabulary has
//! legitimate homonyms. Under a **readable** one the label *is* the local name, so the IRI is
//! already taken and `openbiz mint` refuses it rather than suffixing it — which is `CLAUDE.md`
//! §1.7 working as designed, but arrives as a message about an IRI when the operator's problem is
//! a name. That asymmetry is real, it is tested both ways, and it is in `docs/UNTESTED.md`.
//!
//! # Why a command and not an endpoint
//!
//! The same objection every writing path in this build records: there is no authentication yet,
//! and `POST /api/split` would be an unauthenticated way to add concepts to somebody's thesaurus.
//! The candidate seam over HTTP is its own plan item and lands with the identity.

use openbiz_skos::{
    mint as mint_iri, CoreModel, Node, Part, PartRequest, Placement, SlugBound, Split,
};
use openbiz_store::{Candidate, CandidateSource, GraphId, Provenance, Store};

use crate::cli::{actor, CommandError};
use crate::mint::{convention_of, pattern_for, scan_for, PatternSource};
use crate::staging::{borrowed, elsewhere, newly_broken, BrokenConditions};

/// Propose dividing `concept` into one new concept per label in `labels`.
///
/// Reads the vocabulary, mints an IRI for each part, computes the change, and stages it as a
/// candidate. **Nothing is written to the vocabulary, and nothing about `concept` is removed.**
pub fn split(
    store: &Store,
    graph: &str,
    concept: &str,
    labels: &[String],
    placement: Placement,
    language: Option<&str>,
    pattern: Option<&str>,
) -> Result<String, CommandError> {
    let vocabulary = GraphId::vocabulary(graph)?;
    let model = crate::inspect::read(store, graph)?;
    let concept = Node::iri(concept);

    let recorded = store.iri_policy(&vocabulary)?;
    let suggested = convention_of(&model).suggest();
    let (chosen, source) = pattern_for(graph, &suggested, &recorded, pattern)?;

    let mut scan = scan_for(store, graph, chosen.prefix())?;
    let mut parts = Vec::with_capacity(labels.len());
    for label in labels {
        let minted =
            mint_iri(&chosen, Some(label), SlugBound::DEFAULT, &scan).map_err(|source| {
                CommandError::CannotMint {
                    label: label.clone(),
                    source,
                }
            })?;
        // `openbiz-skos` is engine-free and can only apply a subset of RFC 3987. The parser that
        // will actually store this IRI is entitled to the last word.
        if !openbiz_store::accepts_iri(&minted.iri) {
            return Err(CommandError::NotAnIri { iri: minted.iri });
        }
        // Offered back before the next mint, so the parts of one split do not collide with each
        // other. Nothing is reserved anywhere else: this scan is thrown away when the command ends,
        // and the IRIs become taken when the candidate below is staged.
        scan.push(&minted.iri, "another part of this split");
        parts.push(PartRequest {
            iri: Node::iri(minted.iri),
            label: label.clone(),
        });
    }

    let split = model
        .split(&concept, &parts, placement, language)
        .map_err(CommandError::Split)?;

    // The change is computed; now check what it would leave behind. A split adds statements about
    // IRIs nothing has ever mentioned, so it is hard to see how it could break a condition that
    // holds — and that is exactly the reasoning iteration 43 found to be wrong about a merge. The
    // whole condition set is run, and the argument for it is in `crate::staging`.
    let broken = newly_broken(store, graph, &model, split.additions(), &[])?;
    if !broken.is_empty() {
        return Err(CommandError::BreaksIntegrity {
            operation: "split",
            conditions: Box::new(BrokenConditions {
                graph: graph.to_owned(),
                change: format!("splitting {concept} into {} parts", split.parts().len()),
                broken,
            }),
        });
    }

    let elsewhere = elsewhere(store, graph, &concept)?;

    let provenance = Provenance {
        source: CandidateSource::BulkEdit,
        agent: format!("{} (openbiz split)", actor()?),
        note: format!(
            "split {} into {} concepts, placed {} it",
            concept,
            split.parts().len(),
            placement
        ),
        // A computed split is not a guess; see the same note on `openbiz move`.
        confidence: None,
    };

    let additions = borrowed(split.additions());
    let candidate = store.propose_edit(&vocabulary, &additions, &[], &provenance)?;

    Ok(report(
        graph, &model, &split, &source, &elsewhere, &candidate,
    ))
}

/// What the operator reads back, in the order they need it.
///
/// The order is the argument of this whole command. A reused label comes first, because
/// `CLAUDE.md` §1.7 says the right answer may be not to create anything. The parts come next,
/// because that is what was asked for. **What is still on the original comes before the diff**,
/// because it is the work this command deliberately did not do and the reader has to see it before
/// they decide the change is finished.
fn report(
    graph: &str,
    model: &CoreModel,
    split: &Split,
    source: &PatternSource<'_>,
    elsewhere: &[(String, usize)],
    candidate: &Candidate,
) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "{}{}\n",
        split.concept(),
        named_in(model, split.concept())
    ));
    out.push_str(&format!(
        "split into {} concepts, placed {} it\n",
        split.parts().len(),
        split.placement()
    ));
    out.push_str(&format!("in {graph}\n"));

    let reused: Vec<&Part> = split
        .parts()
        .iter()
        .filter(|part| !part.already_called_that.is_empty())
        .collect();
    if !reused.is_empty() {
        out.push_str(
            "\nwarning: this vocabulary already has a concept by that name. Reusing one outranks \
             creating another — check these before approving:\n",
        );
        for part in reused {
            for existing in &part.already_called_that {
                out.push_str(&format!(
                    "  {} is already called {}\n",
                    existing, part.label
                ));
            }
        }
    }

    out.push_str(&format!(
        "\nthe parts, named under {}:\n",
        pattern_line(source)
    ));
    for part in split.parts() {
        out.push_str(&format!("  {} {}\n", part.iri, part.label));
    }

    out.push_str("\nplaced:\n");
    match split.placement() {
        Placement::Beside => match split.parents().is_empty() {
            true => out.push_str(
                "  under nothing — the concept has no broader concept, so neither do the parts\n",
            ),
            false => {
                for parent in split.parents() {
                    out.push_str(&format!("  under {}{}\n", parent, named_in(model, parent)));
                }
            }
        },
        Placement::Below => out.push_str(&format!(
            "  under {}, which becomes their broader concept\n",
            split.concept()
        )),
    }
    for scheme in split.top_concept_of() {
        out.push_str(&format!("  as a top concept of {scheme}\n"));
    }
    for scheme in split.schemes() {
        out.push_str(&format!("  in the scheme {scheme}\n"));
    }

    let left = split.unapportioned();
    match left.is_empty() {
        true => out.push_str(&format!(
            "\n{} carries nothing else, so there is nothing left to apportion.\n",
            split.concept()
        )),
        false => {
            out.push_str(&format!(
                "\nstill on {} and not apportioned — a split cannot know which part each of these \
                 belongs to, and this command has not guessed:\n",
                split.concept()
            ));
            if !left.narrower.is_empty() {
                out.push_str(&format!(
                    "  {} below it: move {} under the right part with `openbiz move`\n",
                    concepts(left.narrower.len()),
                    match left.narrower.len() {
                        1 => "it",
                        _ => "each",
                    }
                ));
                for child in left.narrower.iter().take(5) {
                    out.push_str(&format!("    {}{}\n", child, named_in(model, child)));
                }
                if left.narrower.len() > 5 {
                    out.push_str(&format!("    and {} more\n", left.narrower.len() - 5));
                }
            }
            if !left.related.is_empty() {
                out.push_str(&format!(
                    "  {} associatively linked to it\n",
                    concepts(left.related.len())
                ));
            }
            if left.mappings > 0 {
                out.push_str(&format!(
                    "  {} mapping link{} into other vocabularies, which now point at a concept \
                     that means less than it did\n",
                    left.mappings,
                    match left.mappings {
                        1 => "",
                        _ => "s",
                    }
                ));
            }
            if left.notes > 0 {
                out.push_str(&format!(
                    "  {} documentation note{}\n",
                    left.notes,
                    match left.notes {
                        1 => "",
                        _ => "s",
                    }
                ));
            }
            if left.labels > 0 {
                // Not "including the one that named both senses", which this said until it was
                // read against a granularity split, where no label ever named two senses.
                out.push_str(&format!(
                    "  {} label{} of its own\n",
                    left.labels,
                    match left.labels {
                        1 => "",
                        _ => "s",
                    }
                ));
            }
            out.push_str(&format!(
                "{} itself stays exactly as it is. Retiring it is a deprecation, which is a \
                 different change and keeps the trail an auditor needs.\n",
                split.concept()
            ));
        }
    }

    if !elsewhere.is_empty() {
        out.push_str(&format!(
            "\n{} is also mentioned outside this vocabulary. Nothing there changes — it still \
             denotes what it denoted — but whoever owns it may want the parts instead:\n",
            split.concept()
        ));
        for (source, count) in elsewhere {
            out.push_str(&format!("  {count} in {source}\n"));
        }
    }

    out.push_str("\nit would add:\n");
    for statement in split.additions() {
        out.push_str(&format!("  {statement}\n"));
    }
    out.push_str("and remove nothing.\n");

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

/// Where the minting pattern came from, in one clause.
fn pattern_line(source: &PatternSource<'_>) -> String {
    match source {
        PatternSource::Given { .. } => "the pattern given with --pattern".to_owned(),
        PatternSource::Recorded(policy) => format!(
            "this vocabulary's recorded pattern {:?}, set by {}",
            policy.pattern(),
            policy.recorded_by()
        ),
        PatternSource::Inferred => "the convention this vocabulary's own concepts suggest, which \
                                    is a guess — record one with `openbiz policy`"
            .to_owned(),
    }
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
    use openbiz_skos::{Placement, SplitError};
    use openbiz_store::{
        CandidateSource, CandidateState, Decision, GraphId, Provenance, RdfSyntax, Store,
    };

    use super::split;
    use crate::cli::CommandError;

    const VOCABULARY: &str = "http://example.org/thesaurus";
    const OTHER: &str = "http://example.org/other";
    const BANKS: &str = "http://example.org/banks";

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

    /// One term meaning two things, with a child, an associative link, a note and a second label —
    /// every kind of thing a split has to decline to apportion.
    const POLYSEMOUS: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix ex: <http://example.org/> .
        ex:scheme a skos:ConceptScheme .
        ex:institutions a skos:Concept ; skos:prefLabel "Institutions"@en ; skos:inScheme ex:scheme .
        ex:banks a skos:Concept ; skos:prefLabel "Banks"@en ; skos:altLabel "Bank"@en ;
            skos:broader ex:institutions ; skos:inScheme ex:scheme ;
            skos:related ex:money ; skos:scopeNote "Both senses, wrongly."@en .
        ex:money a skos:Concept ; skos:prefLabel "Money"@en ; skos:inScheme ex:scheme .
        ex:tellers a skos:Concept ; skos:prefLabel "Tellers"@en ; skos:broader ex:banks .
    "#;

    fn senses() -> Vec<String> {
        vec!["Banks (financial)".to_owned(), "Banks (river)".to_owned()]
    }

    fn beside(store: &Store) -> Result<String, CommandError> {
        split(
            store,
            VOCABULARY,
            BANKS,
            &senses(),
            Placement::Beside,
            None,
            Some("http://example.org/{slug}"),
        )
    }

    /// The order is the argument of the command: what is *not* done comes before the diff, because
    /// a reader who stops at "2 parts proposed" thinks the job is finished.
    #[test]
    fn a_split_says_what_is_left_unapportioned_before_it_shows_the_statements() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let report = beside(&store).expect("a two-sense split");

        let unapportioned = report
            .find("still on")
            .expect("the report must say what it did not decide");
        let diff = report.find("it would add:").expect("and then the diff");
        assert!(
            unapportioned < diff,
            "the work still to do comes first: {report}"
        );

        assert!(
            report.contains("1 concept is below it: move it under the right part"),
            "the child has to be named, with the command that apportions it: {report}"
        );
        assert!(report.contains("1 concept is associatively linked to it"));
        assert!(report.contains("1 documentation note"));
        assert!(report.contains("2 labels of its own"));
        assert!(
            report.contains("stays exactly as it is"),
            "and the original surviving has to be said out loud: {report}"
        );
    }

    /// A split is the one bulk operation with no removals at all, and the candidate has to say so.
    #[test]
    fn a_split_stages_one_candidate_that_removes_nothing() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let report = beside(&store).expect("a two-sense split");
        assert!(report.contains("and remove nothing."), "{report}");

        let candidates = store.candidates().expect("the store's candidates");
        let staged = candidates
            .iter()
            .find(|candidate| candidate.state() == CandidateState::Proposed)
            .expect("one candidate waiting");
        assert_eq!(staged.target().iri(), VOCABULARY);
        assert_eq!(staged.provenance().source, CandidateSource::BulkEdit);
        assert!(
            staged.provenance().agent.ends_with("(openbiz split)"),
            "the command that raised it is part of the trail: {}",
            staged.provenance().agent
        );
    }

    /// The parts take their place; the concept keeps its own, and gains nothing.
    #[test]
    fn beside_puts_the_parts_where_the_concept_is_and_says_nothing_new_about_it() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let report = beside(&store).expect("a two-sense split");

        assert!(report.contains("under <http://example.org/institutions> (\"Institutions\"@en)"));
        assert!(report.contains("in the scheme <http://example.org/scheme>"));
        assert!(
            report.contains(
                "<http://example.org/banks-financial> skos:broader \
                             <http://example.org/institutions>"
            ),
            "{report}"
        );
        assert!(
            !report.contains("<http://example.org/banks> skos:"),
            "no addition is a statement about the concept being split: {report}"
        );
    }

    #[test]
    fn below_makes_the_concept_the_broader_concept_of_every_part() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let report = split(
            &store,
            VOCABULARY,
            BANKS,
            &senses(),
            Placement::Below,
            None,
            Some("http://example.org/{slug}"),
        )
        .expect("a granularity split");

        assert!(
            report.contains("which becomes their broader concept"),
            "{report}"
        );
        assert!(report.contains(
            "<http://example.org/banks-financial> skos:broader <http://example.org/banks>"
        ));
    }

    /// Every part is minted before the next, so a numbered pattern does not hand out one number
    /// three times. The scan is in memory and reserves nothing outside this command.
    #[test]
    fn the_parts_of_one_split_are_minted_against_each_other() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let report = split(
            &store,
            VOCABULARY,
            BANKS,
            &["One".to_owned(), "Two".to_owned(), "Three".to_owned()],
            Placement::Below,
            None,
            Some("http://example.org/c_{n}"),
        )
        .expect("a three-part split under a numbered pattern");

        for iri in [
            "<http://example.org/c_1>",
            "<http://example.org/c_2>",
            "<http://example.org/c_3>",
        ] {
            assert!(
                report.contains(iri),
                "{iri} should be one of the parts: {report}"
            );
        }
    }

    /// `CLAUDE.md` §1.7: reuse outranks creation, so this is the first thing said — and under an
    /// opaque pattern it is a warning rather than a refusal, because a large vocabulary has
    /// legitimate homonyms and refusing would make them unauthorable.
    #[test]
    fn a_part_named_what_something_here_is_named_is_warned_about_before_the_parts() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let report = split(
            &store,
            VOCABULARY,
            BANKS,
            &["Money".to_owned(), "Rivers".to_owned()],
            Placement::Below,
            None,
            Some("http://example.org/c_{n}"),
        )
        .expect("a colliding label is a warning, not a wall");

        let warning = report
            .find("already has a concept by that name")
            .expect("the warning");
        let parts = report.find("the parts, named under").expect("the parts");
        assert!(warning < parts, "reuse comes before creation: {report}");
        assert!(report.contains("<http://example.org/money> is already called \"Money\"@en"));
    }

    /// The same collision under a **readable** pattern is a wall, and not one this command puts
    /// up: the label becomes the local name, the IRI is therefore taken, and `openbiz mint`
    /// refuses a taken IRI rather than suffixing it — which `CLAUDE.md` §1.7 is the reason for.
    ///
    /// Found by writing the test above against a `{slug}` pattern and reading why it failed. The
    /// cost is that the operator is told about an IRI when their problem is a name, and that is
    /// recorded in `docs/UNTESTED.md` rather than papered over here.
    #[test]
    fn under_a_readable_pattern_the_same_collision_is_the_minter_refusing() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let error = split(
            &store,
            VOCABULARY,
            BANKS,
            &["Money".to_owned(), "Rivers".to_owned()],
            Placement::Below,
            None,
            Some("http://example.org/{slug}"),
        )
        .expect_err("the slug of an existing label is an existing IRI");

        match error {
            CommandError::CannotMint { label, .. } => assert_eq!(label, "Money"),
            other => panic!("expected a mint refusal, got {other}"),
        }
    }

    /// The IRI is minted, and a mint that cannot answer stops the split rather than being reported
    /// and carried on from — a part with no IRI is not a part.
    #[test]
    fn a_part_whose_iri_is_already_taken_names_the_part_that_could_not_be_minted() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let error = split(
            &store,
            VOCABULARY,
            BANKS,
            &["Tellers".to_owned(), "Rivers".to_owned()],
            Placement::Below,
            None,
            Some("http://example.org/{slug}"),
        )
        .expect_err("`tellers` is already an IRI in this vocabulary");

        match error {
            CommandError::CannotMint { label, .. } => assert_eq!(label, "Tellers"),
            other => panic!("expected a mint refusal, got {other}"),
        }
    }

    /// The check is "violated after, and not before". A vocabulary that already fails a condition
    /// must stay editable, or the tool cannot be used to repair the mess it is pointed at.
    #[test]
    fn a_vocabulary_that_already_fails_a_condition_can_still_be_split() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:bad a skos:Concept ; skos:prefLabel "One"@en ; skos:prefLabel "Two"@en .
            ex:banks a skos:Concept ; skos:prefLabel "Banks"@en .
            "#,
        );
        let report = beside(&store).expect("S14 was already violated, and not by this change");
        assert!(report.contains("proposed candidate"), "{report}");
    }

    #[test]
    fn a_reference_from_another_vocabulary_is_named_and_not_touched() {
        let (_directory, store) = store_with(POLYSEMOUS);
        load(
            &store,
            OTHER,
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/other/> .
            ex:branch a skos:Concept ; skos:closeMatch <http://example.org/banks> .
            "#,
        );
        let report = beside(&store).expect("a two-sense split");

        assert!(
            report.contains("is also mentioned outside this vocabulary"),
            "{report}"
        );
        assert!(
            report.contains(&format!("1 in the vocabulary {OTHER}")),
            "{report}"
        );
    }

    #[test]
    fn the_domain_refusals_reach_the_operator_whole() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let error = split(
            &store,
            VOCABULARY,
            "http://example.org/scheme",
            &senses(),
            Placement::Beside,
            None,
            Some("http://example.org/{slug}"),
        )
        .expect_err("a concept scheme is not a concept");

        match error {
            CommandError::Split(SplitError::NotAConcept { .. }) => {}
            other => panic!("expected a split refusal, got {other}"),
        }
    }
}
