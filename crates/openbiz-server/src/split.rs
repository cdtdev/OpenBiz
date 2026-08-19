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

use std::collections::BTreeSet;

use openbiz_discovery::{Discovered, Discovery, LocalVocabularies, Match};
use openbiz_skos::{
    mint as mint_iri, CoreModel, LabelQuery, Node, Part, PartRequest, Placement, SlugBound, Split,
};
use openbiz_store::{Candidate, CandidateSource, GraphId, Provenance, Store};

use crate::cli::{actor, CommandError};
use crate::discovery::StoreCorpus;
use crate::mint::LADDER;
use crate::mint::{consulted_entries, convention_of, line, pattern_for, scan_for, PatternSource};
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

    // §1.7 before anything is named, on the creation path `openbiz mint` is not. Every part is a
    // new concept, so every part's name gets its own pass — across the whole store, not the one
    // vocabulary being edited, because the concept a part duplicates is usually in the vocabulary
    // the curator is not looking at. Discovery cannot fail the command: a source that will not
    // answer is reported and the split goes ahead, so there is no `?` here and there must never
    // be one.
    let already = already_here(store, graph, labels);

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
        graph, &model, &split, &source, &elsewhere, &candidate, &already,
    ))
}

/// What discovery found for each part's name, in the order the parts were given.
///
/// One entry per label whatever happened, so the report can never quietly skip a name: a part
/// nothing could be asked about says so, in the place the reader is looking for it.
fn already_here(
    store: &Store,
    graph: &str,
    labels: &[String],
) -> Vec<(String, Option<Discovered>)> {
    let corpus = StoreCorpus::authoring(store, graph);
    let local = LocalVocabularies::named("this store", &corpus);

    // Compacted, because a label discovery cannot be asked about must not shift the answers of
    // the labels after it. The index carried here is what puts each answer back on its own name.
    let mut asked = Vec::new();
    let mut queries = Vec::new();
    for (index, label) in labels.iter().enumerate() {
        if let Ok(query) = LabelQuery::new(label) {
            asked.push(index);
            queries.push(query);
        }
    }

    // One pass over the store for every part, rather than one per part: the reading is the
    // expensive half and it does not depend on the label (`DiscoveryProvider::search_each`).
    let passes = match queries.is_empty() {
        true => Vec::new(),
        false => Discovery::new().across_each(&[&local], &queries),
    };

    let mut per_label: Vec<(String, Option<Discovered>)> =
        labels.iter().map(|label| (label.clone(), None)).collect();
    for (index, pass) in asked.into_iter().zip(passes) {
        if let Some(entry) = per_label.get_mut(index) {
            entry.1 = Some(pass);
        }
    }
    per_label
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
    already: &[(String, Option<Discovered>)],
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

    out.push_str(&discovered(already, split.parts(), split.concept()));

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

/// What already exists that these part names might already name — the §1.7 pass, run before the
/// parts are named and printed above them.
///
/// The whole difference between splitting a concept and quietly minting duplicates of concepts
/// somebody else already has. A split is *several* creations at once, so the question is asked
/// once per part and answered under that part's own name: a report that merged the answers would
/// tell a curator "something here already exists" without saying which of their three parts it
/// was, which is an answer nobody can act on.
///
/// The three things this must never do are the ones `openbiz mint` names: report a bounded list
/// as a complete one, report an unavailable source as an absent match, or print a bare "nothing
/// found" without saying how far the looking went.
fn discovered(already: &[(String, Option<Discovered>)], parts: &[Part], concept: &Node) -> String {
    let mut out = String::from(
        "\nwhat already exists, asked once per part before any of them was named (CLAUDE.md \
         §1.7):\n",
    );

    let mut anything = false;
    for (label, found) in already {
        out.push_str(&format!("  {label:?}\n"));
        let Some(found) = found else {
            // Only a label with no text at all reaches here, and the split refuses one a few
            // lines later, so today nothing prints this. Said anyway, and kept rather than
            // dropped: the alternative is either a second copy of the domain's blank-label rule
            // or a list whose entries no longer line up with the parts, and a match shown under
            // the wrong part name is the one failure this section exists to prevent.
            out.push_str(
                "    nothing was looked for: an empty name matches every label there is\n",
            );
            continue;
        };

        // The concept being split is not a concept to reuse — it is the one being divided — so a
        // part that carries one of its labels is a label to apportion, not a duplicate to avoid.
        // Found by running the command: splitting "Banks" into a part called "Bank" reported the
        // original as an existing concept and offered the reuse ladder over it, which reads as
        // "do not split this", the opposite of the right advice.
        let is_original = |hit: &&Match| hit.resource == *concept;
        let elsewhere: BTreeSet<_> = found
            .exact()
            .filter(|hit| !is_original(hit))
            .map(|hit| &hit.resource)
            .collect();
        let related_elsewhere = found.related().filter(|hit| !is_original(hit)).count();
        anything |= !elsewhere.is_empty() || related_elsewhere > 0;

        match (elsewhere.len(), found.exact().count()) {
            (0, 0) => out.push_str("    nothing discovery reached is called this\n"),
            (0, _) => out.push_str(
                "    nothing else discovery reached is called this, and the concept being split \
                 already carries it:\n",
            ),
            (concepts, _) => out.push_str(&format!(
                "    STOP — already a label on {concepts} other concept(s) discovery reached:\n"
            )),
        }
        for hit in found.exact() {
            out.push_str(&hit_line(hit, concept));
        }

        if found.related().count() > 0 {
            out.push_str(&format!(
                "    {} {} label(s) contain it, which may be the concept meant under another \
                 name:\n",
                match elsewhere.is_empty() {
                    true => "but",
                    false => "and",
                },
                found.related().count()
            ));
            for hit in found.related() {
                out.push_str(&hit_line(hit, concept));
            }
        }
        if !found.is_complete() {
            out.push_str(&format!(
                "    {} more match(es) are not listed: {} matched and this report stops at {}\n",
                found.withheld(),
                found.matched(),
                found.bound().max_matches
            ));
        }
    }

    // The vocabulary being edited was read into a model before any of this, so what it already
    // calls things is known whether or not discovery could be asked. Printed only when discovery
    // could not answer, where it is the last thing standing between a curator and a duplicate —
    // and never otherwise, because it says less than the pass above and repeating it would teach
    // the reader that this section says everything twice.
    if already.iter().any(|(_, found)| {
        found
            .as_ref()
            .is_none_or(|found| found.unavailable().next().is_some())
    }) {
        let reused: Vec<&Part> = parts
            .iter()
            .filter(|part| !part.already_called_that.is_empty())
            .collect();
        out.push_str(
            "\n  discovery could not read everything, so the vocabulary being edited was also \
             checked directly — this is one vocabulary and not the store:\n",
        );
        match reused.is_empty() {
            true => out.push_str("    nothing here is already called any of these\n"),
            false => {
                anything = true;
                for part in reused {
                    for existing in &part.already_called_that {
                        out.push_str(&format!(
                            "    {} is already called {}\n",
                            existing, part.label
                        ));
                    }
                }
            }
        }
    }

    if anything {
        out.push_str(LADDER);
        out.push_str(STILL_PROPOSED);
    }

    // Said before the counts, because a reader who takes "18 label(s) read" for the size of the
    // store will conclude the search was wider than it was. Every source was asked about every
    // part name in one pass, so the totals below are across all of them.
    out.push_str(&format!(
        "\nevery source was asked about all {} part name(s) in one pass; the counts below are the \
         totals across them\n",
        already.len()
    ));
    out.push_str(&consulted_entries(&Discovered::consulted_across(
        &already
            .iter()
            .filter_map(|(_, found)| found.clone())
            .collect::<Vec<_>>(),
    )));
    out
}

/// One match, with a word about the one match that is not a concept to reuse.
fn hit_line(hit: &Match, concept: &Node) -> String {
    let mut out = format!("    {}", line(hit));
    if hit.resource == *concept {
        out.push_str(
            "        this is the concept being split, so the part would take a label the \
             original already carries: apportion that label rather than reuse the concept\n",
        );
    }
    out
}

/// The one thing a split can say about the ladder's last rung.
///
/// Different from a mint's, and the difference matters: a mint offers an IRI and writes nothing,
/// so its reader can simply not use it. A split has already staged a change, so its reader has
/// something to *undo*, and the cheapest correct action — reject it — has to be named here rather
/// than left to be found at the bottom of the report.
const STILL_PROPOSED: &str = concat!(
    "The parts are still proposed below, because two concepts can legitimately share a label — ",
    "but if one of these is the concept a part means, approving this change creates the ",
    "duplicate. Nothing has been written to the vocabulary yet: `openbiz reject` costs nothing.\n",
    "nothing here records a justification for creating these instead: adr/0003 §3 requires that ",
    "record, and the note on the candidate below says what this command did rather than why the ",
    "concepts above did not fit.\n",
);

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

    /// A source that cannot answer, so the section can be driven through a branch no real store
    /// produces: the local corpus is the store the command has already read successfully.
    struct Refusing;

    impl openbiz_discovery::DiscoveryProvider for Refusing {
        fn name(&self) -> &str {
            "a catalog"
        }
        fn search(
            &self,
            _query: &openbiz_skos::LabelQuery,
        ) -> Result<openbiz_discovery::SourceAnswer, openbiz_discovery::Unavailable> {
            Err(openbiz_discovery::Unavailable::because(
                "the catalog is unreachable",
            ))
        }
    }

    fn refused(label: &str) -> (String, Option<openbiz_discovery::Discovered>) {
        let query = openbiz_skos::LabelQuery::new(label).expect("a query");
        (
            label.to_owned(),
            Some(openbiz_discovery::Discovery::new().across(&[&Refusing], &query)),
        )
    }

    /// **`adr/0003` §7, on this path.** A source that could not be read never blocks the split —
    /// and the vocabulary being edited is checked directly anyway, because that much is known
    /// from the model already in hand and is the last thing standing between a curator and a
    /// duplicate.
    #[test]
    fn when_discovery_could_not_read_everything_the_vocabulary_is_still_checked_directly() {
        let part = openbiz_skos::Part {
            iri: openbiz_skos::Node::iri("http://example.org/c_1"),
            label: openbiz_skos::LexicalLabel {
                language: Some("en".to_owned()),
                text: "Money".to_owned(),
            },
            already_called_that: vec![openbiz_skos::Node::iri("http://example.org/money")],
        };

        let report = super::discovered(
            &[refused("Money")],
            &[part],
            &openbiz_skos::Node::iri(BANKS),
        );

        assert!(
            report.contains(
                "discovery could not read everything, so the vocabulary being edited was also \
                 checked directly — this is one vocabulary and not the store:"
            ),
            "{report}"
        );
        assert!(
            report.contains("<http://example.org/money> is already called \"Money\"@en"),
            "{report}"
        );
        assert!(
            report.contains("a catalog — UNAVAILABLE: the catalog is unreachable"),
            "an unavailable source is never an absent match: {report}"
        );
        assert!(report.contains("reuse outranks creation"), "{report}");
    }

    /// The same, with nothing found by the direct check either: the reader is told the narrower
    /// check ran and came back empty, rather than being left to read silence as safety.
    #[test]
    fn a_direct_check_that_finds_nothing_says_so_rather_than_saying_nothing() {
        let report = super::discovered(&[refused("Vaults")], &[], &openbiz_skos::Node::iri(BANKS));

        assert!(
            report.contains("nothing here is already called any of these"),
            "{report}"
        );
        assert!(
            !report.contains("reuse outranks creation"),
            "nothing was found, so there is no ladder to climb: {report}"
        );
    }

    /// A name discovery could not be asked about is said so under its own heading. The split
    /// refuses such a label a few lines later, so nothing prints this today — it is here because
    /// the alternative to the branch is a list whose entries stop lining up with the parts.
    #[test]
    fn a_name_that_could_not_be_asked_about_is_not_silently_clean() {
        let report = super::discovered(
            &[("".to_owned(), None)],
            &[],
            &openbiz_skos::Node::iri(BANKS),
        );

        assert!(
            report.contains("nothing was looked for: an empty name matches every label there is"),
            "{report}"
        );
        assert!(
            report.contains("discovery consulted 0 source(s):"),
            "no pass ran, and the report says so rather than implying one did: {report}"
        );
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
            .find("STOP — already a label on 1 other concept(s)")
            .expect("the warning");
        let parts = report.find("the parts, named under").expect("the parts");
        assert!(warning < parts, "reuse comes before creation: {report}");
        assert!(
            report.contains("<http://example.org/money>  (Money)  skos:prefLabel \"Money\"@en"),
            "{report}"
        );
    }

    /// **The item this discovery pass exists for.** The concept a part duplicates is usually in
    /// the vocabulary the curator is *not* looking at, and until now `openbiz split` asked only
    /// the one it was editing. A vocabulary-local check cannot make this match at all.
    #[test]
    fn a_part_that_duplicates_a_concept_in_another_vocabulary_is_named() {
        let (_directory, store) = store_with(POLYSEMOUS);
        load(
            &store,
            OTHER,
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix other: <http://example.org/other/> .
            other:rivers a skos:Concept ; skos:prefLabel "Rivers"@en .
            "#,
        );

        let report = split(
            &store,
            VOCABULARY,
            BANKS,
            &["Rivers".to_owned(), "Vaults".to_owned()],
            Placement::Below,
            None,
            Some("http://example.org/c_{n}"),
        )
        .expect("a match elsewhere is a warning, not a wall");

        assert!(
            report.contains(
                "<http://example.org/other/rivers>  (Rivers)  skos:prefLabel \"Rivers\"@en, in \
                 the vocabulary http://example.org/other"
            ),
            "the duplicate is elsewhere and has to be named: {report}"
        );
        assert!(report.contains("reuse outranks creation"), "{report}");
        let found = report
            .find("Rivers\"@en, in the vocabulary")
            .expect("the match");
        let parts = report.find("the parts, named under").expect("the parts");
        assert!(found < parts, "discovery precedes creation: {report}");
    }

    /// A label that exists only in a change nobody has approved is still a label somebody chose.
    /// Splitting into it produces the duplicate the moment both are approved.
    #[test]
    fn a_part_that_duplicates_a_pending_change_is_named_as_pending() {
        let (_directory, store) = store_with(POLYSEMOUS);
        store
            .propose_import(
                &GraphId::vocabulary(VOCABULARY).expect("a valid IRI"),
                RdfSyntax::Turtle,
                r#"
                @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
                <http://example.org/vaults> a skos:Concept ; skos:prefLabel "Vaults"@en .
                "#
                .as_bytes(),
                &Provenance {
                    source: CandidateSource::Import,
                    agent: "test".to_owned(),
                    note: "not yet approved".to_owned(),
                    confidence: None,
                },
            )
            .expect("a well-formed proposal");

        let report = split(
            &store,
            VOCABULARY,
            BANKS,
            &["Vaults".to_owned(), "Tellers".to_owned()],
            Placement::Below,
            None,
            Some("http://example.org/c_{n}"),
        )
        .expect("a match in a pending change is a warning, not a wall");

        assert!(
            report.contains("in candidate 2, which is waiting for a decision"),
            "{report}"
        );
    }

    /// The concept being split is **not** a concept to reuse — it is the one being divided. A
    /// part carrying one of its labels is a label to apportion, and offering the reuse ladder
    /// over it reads as "do not split this", which is the opposite of the right advice.
    ///
    /// Found by running the command against a real store, not by reasoning about it.
    #[test]
    fn the_concept_being_split_is_not_offered_as_a_concept_to_reuse() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let report = split(
            &store,
            VOCABULARY,
            BANKS,
            &["Banks".to_owned(), "Vaults".to_owned()],
            Placement::Below,
            None,
            Some("http://example.org/c_{n}"),
        )
        .expect("a part named after the original is legitimate");

        assert!(
            report.contains(
                "nothing else discovery reached is called this, and the concept being split \
                 already carries it"
            ),
            "{report}"
        );
        assert!(
            report.contains(
                "this is the concept being split, so the part would take a label the original \
                 already carries: apportion that label rather than reuse the concept"
            ),
            "{report}"
        );
        assert!(
            !report.contains("reuse outranks creation"),
            "the ladder must not be offered over the concept being divided: {report}"
        );
        assert!(!report.contains("STOP"), "{report}");
    }

    /// **A bare "nothing found" is never printed.** Every pass says which sources answered, how
    /// far each looked, and which were never asked — the sentence that stops "nothing found"
    /// being read as "nothing exists", which is how the tenth overlapping vocabulary is created.
    #[test]
    fn a_split_that_finds_nothing_still_says_how_far_the_looking_went() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let report = split(
            &store,
            VOCABULARY,
            BANKS,
            &["Vaults".to_owned(), "Tellers of tales".to_owned()],
            Placement::Below,
            None,
            Some("http://example.org/c_{n}"),
        )
        .expect("a split that duplicates nothing");

        assert!(
            report.contains("nothing discovery reached is called this"),
            "{report}"
        );
        assert!(
            report.contains(
                "what already exists, asked once per part before any of them was named \
                 (CLAUDE.md §1.7):"
            ),
            "{report}"
        );
        assert!(
            report.contains("discovery consulted 1 source(s):"),
            "{report}"
        );
        assert!(
            report.contains("no peer, no data catalog, and no public registry was consulted"),
            "{report}"
        );
        assert!(
            !report.contains("reuse outranks creation"),
            "a ladder over an empty list teaches the reader to skip it: {report}"
        );
    }

    /// A related match — the query inside a longer label — says what it is, in one sentence
    /// whose wording is pinned because a line continuation has twice eaten a space in this build.
    #[test]
    fn a_label_that_merely_contains_a_part_name_is_offered_as_a_possible_meaning() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let report = split(
            &store,
            VOCABULARY,
            BANKS,
            &["Tell".to_owned(), "Vaults".to_owned()],
            Placement::Below,
            None,
            Some("http://example.org/c_{n}"),
        )
        .expect("a split");

        assert!(
            report.contains(
                "but 1 label(s) contain it, which may be the concept meant under another name:"
            ),
            "{report}"
        );
        assert!(report.contains("(Tellers)"), "{report}");
    }

    /// Two part names are two questions and one consultation. The sources are named once, for the
    /// command, with the counts across every label — three copies of the same paragraph is how a
    /// report stops being read.
    #[test]
    fn several_part_names_report_their_sources_once() {
        let (_directory, store) = store_with(POLYSEMOUS);
        let report = split(
            &store,
            VOCABULARY,
            BANKS,
            &["Vaults".to_owned(), "Tellers of tales".to_owned()],
            Placement::Below,
            None,
            Some("http://example.org/c_{n}"),
        )
        .expect("a split");

        assert_eq!(
            report.matches("discovery consulted").count(),
            1,
            "one consultation record for the command: {report}"
        );
        assert!(
            report.contains("every source was asked about all 2 part name(s) in one pass"),
            "{report}"
        );
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
