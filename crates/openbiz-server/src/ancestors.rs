//! `openbiz ancestors` — walk the hierarchy above one concept and say why each step holds.
//!
//! This is S24 made reachable. `openbiz inspect` reports the *links* a vocabulary states, which
//! after §8's closure is what an author wrote seen from both ends; it deliberately does not report
//! the transitive closure, because `docs/adr/0024` established that the closure of a legal SKOS
//! graph is unbounded and `docs/adr/0025` therefore answers it by walking on read. This is that
//! walk, with a command in front of it.
//!
//! # Why the whole path and not just the answer
//!
//! An operator asking "is Cats under Animals?" can be given `yes`. `CLAUDE.md` §3 requires more
//! than that: every inference must be able to answer *why* with a human-readable derivation, and
//! for a transitive conclusion the derivation **is** the path. A governance team defending a
//! roll-up to an auditor needs the chain, not the verdict — that is the gap `docs/COMPETITIVE.md`
//! records as the incumbents' weakest ground.
//!
//! # Why a command and not an endpoint
//!
//! The same reason `openbiz inspect` is one, and it is not the authentication objection: this only
//! reads. It is a command because the concept tree that will ask this question over HTTP is
//! Phase 3's item, and shipping an endpoint now with no interface behind it would be a caller
//! with nothing behind it.

use std::collections::BTreeSet;

use openbiz_skos::{Above, Ancestry, CoreModel, Node, Resource, Retirements, WalkBound};
use openbiz_store::Store;

use crate::cli::CommandError;
use crate::inspect::convert;
use crate::status;

/// Report what is above `concept` in the vocabulary at `graph`, and why.
///
/// Reads and nothing else.
///
/// A concept the vocabulary never mentions is **refused**, not reported as a concept with no
/// ancestors. The two answers look identical and mean opposite things — one is a root concept,
/// the other is a typo — and the second is the likelier of the two at a command line.
pub fn ancestors(
    store: &Store,
    graph: &str,
    concept: &str,
    current_only: bool,
) -> Result<String, CommandError> {
    let mut builder = CoreModel::builder();
    let mut retirements = Retirements::builder();
    store.for_each_statement(graph, |statement| {
        let statement = convert(statement);
        retirements.push(statement.clone());
        builder.push(statement);
    })?;
    let model = builder.build();
    let retirements = retirements.build();

    let node = Node::iri(concept);
    let Some(resource) = model.resource(&node) else {
        return Err(CommandError::NoSuchConcept {
            concept: concept.to_owned(),
            graph: graph.to_owned(),
        });
    };

    Ok(report(
        graph,
        &node,
        resource,
        &model,
        &retirements,
        model.ancestry(&node, WalkBound::DEFAULT),
        current_only,
    ))
}

/// The report itself, kept apart from the store so it can be tested against a model in hand.
#[allow(clippy::too_many_arguments)]
fn report(
    graph: &str,
    concept: &Node,
    resource: &Resource,
    model: &CoreModel,
    retirements: &Retirements,
    above: Ancestry,
    current_only: bool,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{concept}{}{}\n",
        named(resource),
        status::mark(retirements, concept)
    ));
    out.push_str(&format!("in {graph}\n"));
    // Said before any count, because every number under it is a number about a narrowed report
    // and a reader who learns that at the bottom has already read them as totals. The second
    // sentence is what separates this flag from the one on `openbiz tree`: looking up, the paths
    // are the derivation and are never edited (`docs/adr/0045` §1).
    if current_only {
        out.push_str(
            "current concepts only: the vocabulary's retired concepts are left out of the list. \
             The paths are printed whole, retired concepts and all — a path is why a concept is \
             above this one, and cutting a concept out of the middle of one would claim a link \
             the vocabulary does not state.\n",
        );
    }
    status::explain(&mut out, "", retirements, model, concept);
    // The concept the report is *about* is never the thing being filtered: the reader named it.
    if current_only && retirements.is_retired(concept) {
        out.push_str(
            "it is shown whatever its status because you asked about it by name; --current \
             applies to what is above it.\n",
        );
    }

    // Built here and handed over as a set of nodes: `owl:deprecated` is not SKOS, so which
    // resources to leave out is a status question answered beside the model (`docs/adr/0041`).
    // Empty when the flag is off, which makes the narrowing list everything.
    let skip = match current_only {
        true => status::retired_in(retirements),
        false => BTreeSet::new(),
    };
    let narrowed = above.excluding(&skip);

    if narrowed.is_empty() {
        // The case `--current` has to be safe in. Everything above is retired, so an unqualified
        // "nothing is above it" would say this concept is a root of the hierarchy when the
        // vocabulary puts concepts over it — the false negative `docs/adr/0041` refused to ship,
        // seen from underneath, and the one that gets the wrong parent chosen for a new concept.
        if narrowed.dropped() > 0 {
            out.push_str(&format!(
                "\nnothing above it is current: all {} concept(s) above it are retired, and \
                 --current was asked for. They are in this vocabulary and it is still under them \
                 — retiring a concept moves nothing. Run the same command without --current to \
                 see them and what each one says to use instead.\n",
                narrowed.dropped()
            ));
            if !above.is_complete() {
                out.push_str(
                    "\nthe walk also stopped at its bound before reaching the top, so a current \
                     concept above it may simply not have been reached.\n",
                );
            }
            return out;
        }
        out.push_str(if above.is_complete() {
            "\nnothing is above it: it has no broader concept, directly or transitively.\n"
        } else {
            "\nnothing was reached before the walk hit its bound, so this is not an answer.\n"
        });
        return out;
    }

    out.push_str(&format!(
        "\n{} {}concept(s) are above it, by {} link(s) walked:\n",
        narrowed.len(),
        match current_only {
            true => "current ",
            false => "",
        },
        above.links_walked()
    ));
    for ancestor in narrowed.listed() {
        let label = model.resource(ancestor).map(named).unwrap_or_default();
        out.push_str(&format!(
            "  {ancestor}{label}{}\n",
            status::mark(retirements, ancestor)
        ));
        // The path is the derivation for a transitive conclusion, so it is printed for every
        // ancestor and not only for the far ones. A one-step ancestor gets its path too — it is
        // one line and it is what makes the list readable as a hierarchy rather than as a set.
        out.push_str(&format!(
            "    {}\n",
            match above.path_to(ancestor) {
                Some(path) => path
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" → "),
                // Unreachable: `ancestors()` yields exactly the keys `path_to` resolves. Printed
                // rather than unwrapped, because a report that panics on a vocabulary is worse
                // than one that admits a gap in itself.
                None => "no path was recorded, which is a defect in this report".to_owned(),
            }
        ));
        if let Some(derivation) = above.derivation_to(ancestor) {
            out.push_str(&format!("    because {}\n", derivation.premise));
            out.push_str(&format!("    and {}\n", derivation.rule));
        }
    }

    withheld_note(&mut out, &narrowed);
    on_the_way(&mut out, model, &narrowed);

    // A current concept whose ancestors are not. `openbiz deprecate` retires a concept and moves
    // nothing below it, so this is the ordinary aftermath of a retirement seen from underneath —
    // and it is invisible in a report that marks the ancestors and says nothing about what the
    // marks together mean for the concept the reader actually asked about.
    //
    // Not said under `--current`: there the retired ancestors are not in the list to be counted
    // off it, and `withheld_note` has already said the same thing with the numbers that report's
    // own reader needs.
    if !current_only && !retirements.is_retired(concept) {
        let retired = above
            .ancestors()
            .filter(|ancestor| retirements.is_retired(ancestor))
            .count();
        if retired > 0 {
            out.push_str(&format!(
                "\n{retired} of the concept(s) above it are retired, so this concept — which is \
                 not — sits under something no longer current. Nothing was removed when they were \
                 retired, so it is still where it was.\n"
            ));
        }
    }

    // The one sentence that stops a truncated answer reading as a complete one.
    if !above.is_complete() {
        out.push_str(
            "\nthe walk stopped at its bound before reaching the top, so this list is a lower \
             bound and not the answer.\n",
        );
    } else {
        out.push_str(match current_only {
            true => "\nthat is every current concept above it.\n",
            false => "\nthat is all of them.\n",
        });
    }

    out
}

/// The retired concepts the printed paths still run through, named once rather than marked in
/// every chain.
///
/// `docs/adr/0045` §1: the path is the derivation, so it is printed whole and a concept excluded
/// from the *list* can still appear in one. Under a flag that says the retired concepts are out,
/// one turning up in a printed path unremarked reads as the flag not working — so it is named,
/// with what it is doing there.
///
/// Nothing is printed when nothing was kept, which is every ancestry in the overwhelming majority
/// of vocabularies: they retire nothing.
fn on_the_way(out: &mut String, model: &CoreModel, narrowed: &Above<'_>) {
    let kept: Vec<&Node> = narrowed.on_the_way().collect();
    if kept.is_empty() {
        return;
    }

    // "of them", never "more". These are a subset of the concepts `withheld_note` has just
    // counted, and a second bare number about the same concepts tells a reader there are more
    // retired concepts above them than there are — the duplicated-count defect iteration 52 found
    // on `openbiz tree` and fixed there.
    out.push_str(&format!(
        "\n{} of them appear in the paths above, which are printed whole:\n",
        kept.len()
    ));
    for node in kept {
        let label = model.resource(node).map(named).unwrap_or_default();
        out.push_str(&format!("  {node}{label}\n"));
    }
    out.push_str(
        "  each is in a path because taking it out would make that path claim its two neighbours \
         are directly linked, which this vocabulary does not say. The concepts above them are \
         listed because a retirement removes no link: they are still above this concept.\n",
    );
}

/// What `--current` left out of the list, always said, and never said as nothing.
///
/// The rule `docs/adr/0043` set for `openbiz search` and every browse command has inherited:
/// **the concepts go, the fact that there were concepts stays.** Someone reading upwards is
/// choosing where a new concept belongs, so a list that quietly lost an ancestor is how it gets
/// filed under the wrong one.
fn withheld_note(out: &mut String, narrowed: &Above<'_>) {
    if narrowed.dropped() == 0 {
        return;
    }
    out.push_str(&format!(
        "\n{} concept(s) above it are retired and not listed because --current was asked for. \
         They are in this vocabulary and it is still under them: run the same command without \
         --current to see them and what each one says to use instead.\n",
        narrowed.dropped()
    ));
}

/// A resource's label in parentheses, or nothing if it has none. As `openbiz inspect` prints it.
fn named(resource: &Resource) -> String {
    match resource.display_label() {
        Some(label) => format!("  ({label})"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use openbiz_store::{GraphId, RdfSyntax, Store};

    use super::ancestors;
    use crate::cli::CommandError;

    const VOCABULARY: &str = "http://example.org/thesaurus";

    /// A store holding `turtle` in one registered vocabulary. Through the candidate seam, exactly
    /// as a user's data arrives, for the reason `inspect`'s tests give.
    fn store_with(turtle: &str) -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(directory.path()).expect("an openable store");
        let target = GraphId::vocabulary(VOCABULARY).expect("a valid vocabulary IRI");
        store
            .create_vocabulary_graph(&target)
            .expect("a fresh registration");
        let candidate = store
            .propose_import(
                &target,
                RdfSyntax::Turtle,
                turtle.as_bytes(),
                &openbiz_store::Provenance {
                    source: openbiz_store::CandidateSource::Import,
                    agent: "test".to_owned(),
                    note: "fixture".to_owned(),
                    confidence: None,
                },
            )
            .expect("a well-formed proposal");
        store
            .decide(candidate.id(), openbiz_store::Decision::Approve, "test")
            .expect("an approvable candidate");
        (directory, store)
    }

    const CHAIN: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix ex: <http://example.org/> .
        ex:cats a skos:Concept ; skos:prefLabel "Cats"@en ; skos:broader ex:mammals .
        ex:mammals a skos:Concept ; skos:prefLabel "Mammals"@en ; skos:broader ex:animals .
        ex:animals a skos:Concept ; skos:prefLabel "Animals"@en .
    "#;

    /// The whole point of the command: a concept two steps down reports both ancestors, and the
    /// far one carries the S24 derivation that names the path.
    #[test]
    fn a_two_step_chain_reports_both_ancestors_and_explains_the_far_one() {
        let (_directory, store) = store_with(CHAIN);
        let report = ancestors(&store, VOCABULARY, "http://example.org/cats", false)
            .expect("cats is in the vocabulary");

        assert!(report.contains("2 concept(s) are above it"), "{report}");
        assert!(report.contains("http://example.org/mammals"), "{report}");
        assert!(report.contains("http://example.org/animals"), "{report}");
        assert!(report.contains("(\"Mammals\"@en)"), "{report}");
        assert!(
            report.contains(
                "<http://example.org/cats> → <http://example.org/mammals> → \
                 <http://example.org/animals>"
            ),
            "the path is the derivation and it must be printed: {report}"
        );
        assert!(
            report.contains("S24: skos:broaderTransitive and skos:narrowerTransitive are each"),
            "the specification statement is quoted, not just cited: {report}"
        );
        assert!(report.contains("that is all of them."), "{report}");
    }

    /// A root concept says so, and does not say it hit a bound.
    #[test]
    fn a_root_concept_reports_that_nothing_is_above_it() {
        let (_directory, store) = store_with(CHAIN);
        let report = ancestors(&store, VOCABULARY, "http://example.org/animals", false)
            .expect("animals is in the vocabulary");

        assert!(report.contains("nothing is above it"), "{report}");
        assert!(!report.contains("bound"), "{report}");
    }

    /// A typo must not read as a root concept. This is the whole reason the command refuses
    /// rather than reporting an empty answer.
    #[test]
    fn a_concept_the_vocabulary_does_not_hold_is_refused() {
        let (_directory, store) = store_with(CHAIN);
        let error = ancestors(&store, VOCABULARY, "http://example.org/catz", false)
            .expect_err("catz is not in the vocabulary");

        assert!(
            matches!(error, CommandError::NoSuchConcept { .. }),
            "{error:?}"
        );
        assert!(
            error.to_string().contains("catz") && error.to_string().contains(VOCABULARY),
            "{error}"
        );
    }

    /// An unregistered graph is refused by the store, exactly as `openbiz inspect` is — a typo in
    /// the vocabulary IRI must not read as an empty thesaurus either.
    #[test]
    fn an_unregistered_vocabulary_is_refused() {
        let (_directory, store) = store_with(CHAIN);
        let error = ancestors(
            &store,
            "http://example.org/nope",
            "http://example.org/cats",
            false,
        )
        .expect_err("that vocabulary is not registered");

        assert!(matches!(error, CommandError::Store(_)), "{error:?}");
    }

    /// §8.6.8's Example 37 through the real store and the real command: a cycle is consistent, so
    /// the walk must terminate and report the concept as its own ancestor rather than hang.
    #[test]
    fn a_cyclic_hierarchy_terminates_and_names_the_cycle() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:a a skos:Concept ; skos:broader ex:b .
            ex:b a skos:Concept ; skos:broader ex:a .
            "#,
        );
        let report = ancestors(&store, VOCABULARY, "http://example.org/a", false)
            .expect("a is in the vocabulary");

        assert!(report.contains("2 concept(s) are above it"), "{report}");
        assert!(
            report.contains(
                "<http://example.org/a> → <http://example.org/b> → <http://example.org/a>"
            ),
            "the cycle is named rather than hidden: {report}"
        );
    }

    /// The same chain with `Mammals` retired — the state a vocabulary is in after an ordinary
    /// retirement, because `openbiz deprecate` moves nothing below the concept it retires.
    const RETIRED_MIDDLE: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix owl: <http://www.w3.org/2002/07/owl#> .
        @prefix ex: <http://example.org/> .
        ex:cats a skos:Concept ; skos:prefLabel "Cats"@en ; skos:broader ex:mammals .
        ex:mammals a skos:Concept ; skos:prefLabel "Mammals"@en ; skos:broader ex:animals ;
            owl:deprecated true .
        ex:animals a skos:Concept ; skos:prefLabel "Animals"@en .
    "#;

    /// A retired ancestor is marked where it is listed, and is still listed: it is still above
    /// this concept, because retiring it removed nothing.
    #[test]
    fn a_retired_ancestor_is_marked_and_still_reported() {
        let (_directory, store) = store_with(RETIRED_MIDDLE);
        let report = ancestors(&store, VOCABULARY, "http://example.org/cats", false)
            .expect("cats is in the vocabulary");

        assert!(report.contains("2 concept(s) are above it"), "{report}");
        assert!(
            report.contains("  <http://example.org/mammals>  (\"Mammals\"@en)  [retired]\n"),
            "{report}"
        );
        assert!(
            report.contains("  <http://example.org/animals>  (\"Animals\"@en)\n"),
            "a current ancestor carries no mark, which is what makes the mark mean something: \
             {report}"
        );
    }

    /// The marks add up to something about the concept the reader actually asked about, and a
    /// report that printed them and said nothing would leave that to be worked out.
    #[test]
    fn a_current_concept_under_a_retired_one_is_told_so() {
        let (_directory, store) = store_with(RETIRED_MIDDLE);
        let report = ancestors(&store, VOCABULARY, "http://example.org/cats", false)
            .expect("cats is in the vocabulary");

        assert!(
            report.contains("1 of the concept(s) above it are retired"),
            "{report}"
        );
        assert!(
            report.contains("it is still where it was"),
            "the reader is told the hierarchy did not change: {report}"
        );
    }

    /// A retired concept asked about directly gets the full account, not the marker alone.
    #[test]
    fn a_retired_concept_asked_about_is_explained() {
        let (_directory, store) = store_with(RETIRED_MIDDLE);
        let report = ancestors(&store, VOCABULARY, "http://example.org/mammals", false)
            .expect("mammals is in the vocabulary");

        assert!(
            report.contains("the vocabulary marks it owl:deprecated"),
            "{report}"
        );
        assert!(
            report.contains("nothing is recorded as replacing it"),
            "a retirement with no successor says so rather than staying silent: {report}"
        );
        // And it does not tell a retired concept that its ancestors are current.
        assert!(
            !report.contains("of the concept(s) above it are retired"),
            "{report}"
        );
    }

    /// A vocabulary that retires nothing reads exactly as it did.
    #[test]
    fn a_vocabulary_with_no_retirements_says_nothing_about_them() {
        let (_directory, store) = store_with(CHAIN);
        let report = ancestors(&store, VOCABULARY, "http://example.org/cats", false)
            .expect("cats is in the vocabulary");

        assert!(!report.contains("retired"), "{report}");
    }

    /// `docs/adr/0045` §1, and the decision the item turned on. `Animals` is above `Cats` whatever
    /// `Mammals`'s status: retiring a concept removes no link. So the retired ancestor leaves the
    /// *list* and the current one above it stays — anything else suppresses a current concept on
    /// the strength of another concept's status.
    #[test]
    fn current_only_drops_the_retired_ancestor_and_keeps_the_one_above_it() {
        let (_directory, store) = store_with(RETIRED_MIDDLE);
        let report = ancestors(&store, VOCABULARY, "http://example.org/cats", true)
            .expect("cats is in the vocabulary");

        assert!(
            report.contains("1 current concept(s) are above it"),
            "{report}"
        );
        assert!(
            report.contains("  <http://example.org/animals>  (\"Animals\"@en)\n"),
            "the concept above the retired one is still above this one: {report}"
        );
        assert!(
            !report.contains("  <http://example.org/mammals>  (\"Mammals\"@en)  [retired]\n"),
            "the retired ancestor is not listed as an ancestor: {report}"
        );
        assert!(
            report.contains("that is every current concept above it."),
            "{report}"
        );
    }

    /// The other half of §1: the path is the derivation and is printed whole. Cutting `Mammals`
    /// out of `Cats → Mammals → Animals` would state that `Animals` is directly above `Cats`,
    /// which this vocabulary does not say — so the path keeps it, and the report names it rather
    /// than leaving a retired concept in a report that was asked for none.
    #[test]
    fn current_only_never_edits_a_path_and_names_what_is_still_in_one() {
        let (_directory, store) = store_with(RETIRED_MIDDLE);
        let narrowed = ancestors(&store, VOCABULARY, "http://example.org/cats", true)
            .expect("cats is in the vocabulary");
        let full = ancestors(&store, VOCABULARY, "http://example.org/cats", false)
            .expect("cats is in the vocabulary");

        let path = "    <http://example.org/cats> → <http://example.org/mammals> → \
                    <http://example.org/animals>";
        assert!(narrowed.contains(path), "{narrowed}");
        assert!(
            full.contains(path),
            "the narrowed path is the one the unnarrowed report printed, unchanged: {full}"
        );
        assert!(
            narrowed.contains("1 of them appear in the paths above, which are printed whole:"),
            "{narrowed}"
        );
        assert!(
            narrowed.contains("  <http://example.org/mammals>  (\"Mammals\"@en)\n"),
            "the concept in the path is named: {narrowed}"
        );
    }

    /// The narrowing says what it cost, which `docs/adr/0043` made the whole safety of the flag.
    #[test]
    fn current_only_says_how_many_ancestors_it_left_out() {
        let (_directory, store) = store_with(RETIRED_MIDDLE);
        let report = ancestors(&store, VOCABULARY, "http://example.org/cats", true)
            .expect("cats is in the vocabulary");

        assert!(
            report.contains(
                "1 concept(s) above it are retired and not listed because --current was asked \
                 for."
            ),
            "{report}"
        );
        assert!(
            report.contains("run the same command without --current"),
            "the sentence that gets them back: {report}"
        );
        // The unnarrowed report's own sentence about retired ancestors is not also printed: it
        // counts them off a list they are no longer on, so it would be a second number about the
        // same thing.
        assert!(
            !report.contains("of the concept(s) above it are retired, so this concept"),
            "{report}"
        );
    }

    /// A vocabulary whose whole hierarchy above one concept is retired.
    const ALL_ABOVE_RETIRED: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix owl: <http://www.w3.org/2002/07/owl#> .
        @prefix ex: <http://example.org/> .
        ex:cats a skos:Concept ; skos:prefLabel "Cats"@en ; skos:broader ex:mammals .
        ex:mammals a skos:Concept ; skos:prefLabel "Mammals"@en ; skos:broader ex:animals ;
            owl:deprecated true .
        ex:animals a skos:Concept ; skos:prefLabel "Animals"@en ; owl:deprecated true .
    "#;

    /// **The case the counts exist for** (`docs/adr/0045` §4). Every concept above `Cats` is
    /// retired, so the list is empty — and an unqualified "nothing is above it" would say this
    /// concept is a root of the hierarchy when the vocabulary puts two concepts over it. That is
    /// how the wrong parent gets chosen for a new concept.
    #[test]
    fn ancestors_current_only_still_admits_the_retired_concepts_above() {
        let (_directory, store) = store_with(ALL_ABOVE_RETIRED);
        let report = ancestors(&store, VOCABULARY, "http://example.org/cats", true)
            .expect("cats is in the vocabulary");

        assert!(
            report.contains("nothing above it is current: all 2 concept(s) above it are retired"),
            "{report}"
        );
        assert!(
            !report.contains("it has no broader concept"),
            "the sentence for a genuine root must not be printed about a concept with two \
             concepts above it: {report}"
        );
        assert!(
            report.contains("without --current to see them"),
            "the sentence that gets them back: {report}"
        );
    }

    /// A genuine root and an emptied list produce different reports, which is the distinction the
    /// test above is about — pinned from the other side so neither sentence can drift into the
    /// other's case.
    #[test]
    fn a_genuine_root_is_not_told_its_ancestors_were_withheld() {
        let (_directory, store) = store_with(ALL_ABOVE_RETIRED);
        let report = ancestors(&store, VOCABULARY, "http://example.org/animals", true)
            .expect("animals is in the vocabulary");

        assert!(report.contains("it has no broader concept"), "{report}");
        assert!(!report.contains("nothing above it is current"), "{report}");
    }

    /// The concept the report is *about* is never filtered — the reader named it — and it says so
    /// rather than leaving a `[retired]` marker under a flag that said there would be none.
    #[test]
    fn the_concept_asked_about_is_shown_whatever_its_status() {
        let (_directory, store) = store_with(RETIRED_MIDDLE);
        let report = ancestors(&store, VOCABULARY, "http://example.org/mammals", true)
            .expect("mammals is in the vocabulary");

        assert!(
            report.contains("it is shown whatever its status because you asked about it by name"),
            "{report}"
        );
        assert!(
            report.contains("1 current concept(s) are above it"),
            "and the flag still applies above it: {report}"
        );
    }

    /// A vocabulary that retires nothing reads the same either way, so nobody pays for a feature
    /// their vocabulary does not use.
    ///
    /// Asserted as *the whole report below the banner*, not as the absence of a phrase. The
    /// banner is printed whenever the flag is typed — a reader has to be told the numbers under
    /// it are narrowed ones before they read them — but nothing else may move.
    #[test]
    fn current_only_on_a_vocabulary_with_no_retirements_reads_identically() {
        let (_directory, store) = store_with(CHAIN);
        let narrowed = ancestors(&store, VOCABULARY, "http://example.org/cats", true)
            .expect("cats is in the vocabulary");
        let full = ancestors(&store, VOCABULARY, "http://example.org/cats", false)
            .expect("cats is in the vocabulary");

        let banner = narrowed
            .lines()
            .find(|line| line.starts_with("current concepts only:"))
            .expect("the flag announces itself");
        assert_eq!(
            narrowed.replace(&format!("{banner}\n"), ""),
            full.replace("concept(s) are above it", "current concept(s) are above it")
                .replace(
                    "that is all of them.",
                    "that is every current concept above it."
                ),
            "when nothing is retired only the three lines that say the flag is on may differ: \
             the banner, the word in the count, and the closing line"
        );
        assert!(!narrowed.contains("not listed"), "{narrowed}");
    }

    /// Two counts about one concept must not read as two concepts. `Mammals` is the only retired
    /// thing above `Cats`; a report that says "1 retired concept is in the paths" and then "1 more
    /// concept is not listed" has told the reader there are two, which is the same duplicated-count
    /// defect iteration 52 found on `openbiz tree`.
    #[test]
    fn the_retired_ancestor_is_counted_once_and_not_twice() {
        let (_directory, store) = store_with(RETIRED_MIDDLE);
        let report = ancestors(&store, VOCABULARY, "http://example.org/cats", true)
            .expect("cats is in the vocabulary");

        assert!(
            !report.contains("1 more concept(s)"),
            "\"more\" says there is another one, and there is not: {report}"
        );
        assert!(
            report.contains("1 of them appear in the paths above"),
            "the second count names itself as part of the first: {report}"
        );
    }

    /// The concept asked about is not "a retired concept in a path that is not listed as an
    /// ancestor" — it is the concept the report is about, and the report already says so three
    /// lines higher. Naming it again both contradicts that and gives a false reason: it is the
    /// start of every path, so it has no two neighbours to be claimed adjacent.
    #[test]
    fn the_concept_asked_about_is_not_also_reported_as_being_in_the_way() {
        let (_directory, store) = store_with(RETIRED_MIDDLE);
        let report = ancestors(&store, VOCABULARY, "http://example.org/mammals", true)
            .expect("mammals is in the vocabulary");

        assert!(
            !report.contains("appear in the paths above without being listed"),
            "nothing was withheld from this report, so nothing may be reported as withheld: \
             {report}"
        );
        assert!(
            report.contains("it is shown whatever its status because you asked about it by name"),
            "{report}"
        );
    }
}
