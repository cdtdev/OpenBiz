//! `openbiz search` — find a concept by what it is called.
//!
//! Every other read command in this build starts from an IRI. This is the one that starts from a
//! word, which is the only thing a subject-matter expert has when they sit down: they know the
//! organisation calls it a *carrier bag* and they do not know whether the thesaurus does.
//!
//! # Why this is the anti-silo command
//!
//! `CLAUDE.md` §1.7 says reuse outranks creation, and the mechanism by which a silo is actually
//! created is mundane: someone looks for a term, does not find it, and makes a new one. A search
//! that is too strict — case-sensitive, whole-label-only, preferred-labels-only, one language —
//! manufactures exactly that outcome and reports it as "no results". So the defaults are the
//! forgiving ones: anywhere in the label, any language, and all three label kinds including
//! `skos:hiddenLabel`, which SKOS Reference §5.1 defines *in terms of* text search.
//!
//! # What the report shows, and the one thing §5.1 says not to
//!
//! §5.1 ends "the hidden label won't otherwise be visible to the user (so further mistakes aren't
//! encouraged)". That is a rule about *display*, and it binds a public-facing search front-end
//! built on this data. It does not bind a report addressed to the person curating the vocabulary,
//! who has to be able to see which of their hidden labels matched in order to maintain them — and
//! `CLAUDE.md` §3 requires every answer to say why it holds. So the matched label is printed with
//! its kind, and the concept is still *named* by its preferred label, never by the hidden one.
//! See `docs/adr/0034`.
//!
//! # `--current`, and the hole it has to not re-open
//!
//! `docs/adr/0041` shows every retired concept and marks it, in this command above all: told
//! "it exists, it is retired, use this instead", someone reusing a term does the right thing.
//! A curator building a new branch still has a real need for a list without them, and `--current`
//! is that request. It is opt-in, it is never the default, and it is bounded by one rule recorded
//! in `docs/adr/0043`: **it hides the hits and never the fact that there were hits.** The retired
//! matches are counted during the scan and the count closes the report, including — especially —
//! when it is the only thing that matched.

use openbiz_skos::{
    CoreModel, LabelKind, LabelOrigin, LabelQuery, LabelSearch, MatchMode, Node, Resource,
    Retirements, SkosClass,
};
use openbiz_store::Store;

use crate::cli::CommandError;
use crate::inspect::convert;
use crate::status;

/// Report every label in the vocabulary at `graph` that `query` matches.
///
/// With `current_only`, the hits on concepts the vocabulary marks retired are left out of the
/// list and counted into the sentence that closes the report. Reads and nothing else.
pub fn search(
    store: &Store,
    graph: &str,
    query: &LabelQuery,
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

    // The set is built here and not inside the search because `owl:deprecated` is not SKOS
    // (`docs/adr/0041`): the model is asked to leave out some resources, and which ones is a
    // status question answered beside it.
    let found = match current_only {
        true => model.search_excluding(query, &status::retired_in(&retirements)),
        false => model.search(query),
    };
    Ok(report(
        graph,
        query,
        &model,
        &retirements,
        current_only,
        found,
    ))
}

/// The report itself, kept apart from the store so it can be tested against a model in hand.
fn report(
    graph: &str,
    query: &LabelQuery,
    model: &CoreModel,
    retirements: &Retirements,
    current_only: bool,
    found: LabelSearch,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("{:?} in {graph}\n", query.text()));
    out.push_str(&format!(
        "{}, {}, over {}\n",
        match query.mode() {
            MatchMode::Exact => "matched against the whole label",
            MatchMode::Prefix => "matched at the start of the label",
            MatchMode::Infix => "matched anywhere in the label",
        },
        query.language(),
        kinds_asked_for(query),
    ));
    // Said at the top, before any count, because every number below it is a number about a
    // narrowed search and a reader who learns that at the bottom has already read them as totals.
    if current_only {
        out.push_str(
            "current concepts only: the vocabulary's retired concepts were matched and left out.\n",
        );
    }

    // Deliberately `matched()` and not `is_empty()`. They differ exactly when the bound
    // suppressed every hit, and reporting that as "nothing matched" is a false negative in the one
    // command whose false negatives make people create duplicate concepts. Found by running the
    // command with `--limit 0`.
    if found.matched() == 0 {
        // Said in the words that stop the reader concluding the concept does not exist. That
        // conclusion is how a duplicate gets created, which is the thing this command exists to
        // prevent, so the sentence names the two things that most often cause a miss.
        out.push_str(&format!(
            "\nnothing matched, out of {} label(s) on {} resource(s) read.\n",
            found.labels_read(),
            found.resources_read()
        ));
        out.push_str(
            "matching ignores case but not accents or spelling: an unaccented or differently \
             spelled query will not find an accented or differently spelled label.\n",
        );
        // Last, because it is the sentence that changes what the reader does next. This is the one
        // outcome that makes `--current` dangerous: every match this vocabulary had was on a
        // retired concept, so the flag has turned "it exists, and here is what replaced it" into
        // "no results" — the reading that gets a duplicate created (`CLAUDE.md` §1.7).
        withheld_note(&mut out, &found);
        return out;
    }

    out.push_str(&format!(
        "\n{} label(s) matched, out of {} read on {} resource(s){}\n",
        found.matched(),
        found.labels_read(),
        found.resources_read(),
        match found.is_empty() {
            true => ".",
            false => ":",
        }
    ));

    if found.is_empty() {
        // The bound is zero. Something matched, nothing is shown, and the report says both rather
        // than letting the second be read as the first.
        out.push_str(&format!(
            "\nnone of them are listed: the limit is {}.\n",
            found.bound().max_hits
        ));
        return out;
    }

    let mut retired = 0usize;
    for hit in found.hits() {
        out.push_str(&format!(
            "\n  {}{}{}\n",
            hit.resource,
            named(model, &hit.resource),
            status::mark(retirements, &hit.resource)
        ));
        // The full account and not just the marker, which is a departure from every other read
        // command and is the point of this one. Search is where a term is chosen for reuse: a
        // person who has found the right concept and is not told it is obsolete will use it, and a
        // person told only "[retired]" with no successor named will conclude the vocabulary has
        // nothing and create the duplicate this command exists to prevent (`CLAUDE.md` §1.7).
        status::explain(&mut out, "    ", retirements, model, &hit.resource);
        if retirements.is_retired(&hit.resource) {
            retired += 1;
        }
        out.push_str(&format!("    {}  {}\n", hit.label, hit.quality));
        for (kind, origin) in &hit.kinds {
            out.push_str(&format!("    under {}", property(*kind)));
            match origin {
                LabelOrigin::Asserted => out.push('\n'),
                // The explainability requirement: a label the vocabulary never states plainly is
                // still searchable, and the reader has to be told why it was found at all — they
                // will not find it by searching their own Turtle for `skos:prefLabel`.
                LabelOrigin::DumbedDown(rule) => out.push_str(&format!(
                    ", which the vocabulary does not state: {origin}, {}\n",
                    rule.statement()
                )),
            }
            if *kind == LabelKind::Hidden {
                out.push_str(
                    "      a hidden label: SKOS §5.1 gives it for search and says it should not \
                     otherwise be shown\n",
                );
            }
        }
        if let Some(what) = not_a_concept(model, &hit.resource) {
            out.push_str(&format!("    {what}\n"));
        }
    }

    if retired > 0 {
        out.push_str(&format!(
            "\n{retired} of the {} concept(s) shown are retired. They are shown rather than \
             hidden: a search that omitted them would report a term this vocabulary holds as one \
             it has never heard of, which is how a duplicate gets created.\n",
            found.len()
        ));
    }

    if found.is_complete() {
        out.push_str("\nthat is all of them.\n");
    } else {
        out.push_str(&format!(
            "\nthe best {} are shown; {} more matched and are not listed, so this is not the \
             whole answer.\n",
            found.len(),
            found.matched() - found.len()
        ));
    }
    // After the completeness line and not before it: "that is all of them" is about the bound, and
    // a reader who meets the withheld count first reads the two as contradicting each other.
    withheld_note(&mut out, &found);

    out
}

/// What `--current` left out, always said, and never said as nothing.
///
/// This is the whole safety of the flag. `docs/adr/0041` refuses to hide a retired concept by
/// default because a search that omits one reports a term the vocabulary *holds* as one it has
/// never heard of, and that reading is how a duplicate concept gets created. Asking for current
/// concepts only is a legitimate request and it re-opens exactly that hole — so the hits go, the
/// count stays, and the reader is told in one line how to get them back.
///
/// Nothing is printed when nothing was withheld, which is every search on the overwhelming
/// majority of vocabularies: they have no retired concept at all.
fn withheld_note(out: &mut String, found: &LabelSearch) {
    if found.withheld() == 0 {
        return;
    }
    out.push_str(&format!(
        "\n{} more label(s) matched, on {} retired concept(s), and are not listed because \
         --current was asked for. They are in this vocabulary: run the same search without \
         --current to see them and what each one says to use instead.\n",
        found.withheld(),
        found.withheld_resources(),
    ));
}

/// The label kinds a query asked for, as a phrase.
fn kinds_asked_for(query: &LabelQuery) -> String {
    let names: Vec<&str> = query
        .kinds()
        .iter()
        .map(|kind| match kind {
            LabelKind::Preferred => "preferred",
            LabelKind::Alternative => "alternative",
            LabelKind::Hidden => "hidden",
        })
        .collect();
    format!("{} label(s)", names.join(", "))
}

/// `skos:prefLabel` and its two siblings, written as an author writes them.
fn property(kind: LabelKind) -> String {
    format!("skos:{}", kind.local_name())
}

/// A resource's preferred label in parentheses, or nothing if it has none.
///
/// Deliberately [`Resource::display_label`], which never chooses a hidden label: §5.1 says a
/// hidden label is not otherwise shown, and the *name* of a concept is the "otherwise".
fn named(model: &CoreModel, node: &Node) -> String {
    match model.resource(node).and_then(Resource::display_label) {
        Some(label) => format!("  ({label})"),
        None => String::new(),
    }
}

/// What the matched resource is, when it is not a concept.
///
/// §5 puts labels on resources of any type, and a scheme or a collection carrying the word being
/// searched for is a legitimate hit — but a reader scanning a list of results will read every line
/// as a concept unless told otherwise.
fn not_a_concept(model: &CoreModel, node: &Node) -> Option<String> {
    let resource = model.resource(node)?;
    if resource.is_a(SkosClass::Concept) {
        return None;
    }
    let classes: Vec<String> = resource
        .classes()
        .keys()
        .map(|class| format!("skos:{}", class.local_name()))
        .collect();
    Some(match classes.is_empty() {
        true => "not a concept: nothing in the vocabulary types it, in SKOS terms".to_owned(),
        false => format!("not a concept: {}", classes.join(", ")),
    })
}

#[cfg(test)]
mod tests {
    use openbiz_skos::SearchBound;
    use openbiz_store::{GraphId, RdfSyntax, Store};

    use super::search;
    use openbiz_skos::{LabelKind, LabelQuery, LanguageFilter, LanguageRange, MatchMode};

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

    /// Multilingual, all three label kinds, one SKOS-XL label, one untagged label, and two
    /// resources that carry a matching label without being concepts.
    const PACKAGING: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix skosxl: <http://www.w3.org/2008/05/skos-xl#> .
        @prefix ex: <http://example.org/> .

        ex:scheme a skos:ConceptScheme ; skos:prefLabel "Retail packaging thesaurus"@en .

        ex:bag a skos:Concept ;
          skos:prefLabel "Carrier bag"@en ;
          skos:prefLabel "Sac de transport"@fr ;
          skos:altLabel "Shopping bag"@en-GB ;
          skos:hiddenLabel "carrier-bag"@en .

        ex:paper a skos:Concept ; skos:prefLabel "Paper bag"@en ; skos:altLabel "Paper sack"@en .

        ex:baggage a skos:Collection ; skos:prefLabel "Baggage and bags"@en .

        ex:tote skosxl:prefLabel ex:tote-label .
        ex:tote-label a skosxl:Label ; skosxl:literalForm "Tote bag"@en .

        ex:untagged a skos:Concept ; skos:prefLabel "BAG (uncontrolled)" .
    "#;

    fn query(text: &str) -> LabelQuery {
        LabelQuery::new(text).expect("a non-empty query")
    }

    /// The command's whole purpose: a word finds the concepts the vocabulary calls by it, in every
    /// language and under every label kind, without the asker knowing an IRI.
    #[test]
    fn a_word_finds_every_concept_labelled_with_it_however_it_is_labelled() {
        let (_directory, store) = store_with(PACKAGING);
        let report =
            search(&store, VOCABULARY, &query("bag"), false).expect("a readable vocabulary");

        assert!(report.contains("<http://example.org/bag>"), "{report}");
        assert!(report.contains("<http://example.org/paper>"), "{report}");
        // Under an alternative label, which is a hit a preferred-labels-only search would miss.
        assert!(report.contains("\"Shopping bag\"@en-gb"), "{report}");
        assert!(report.contains("that is all of them."), "{report}");
    }

    /// §5.1 defines the hidden label *for* text search, so it is searched — and the report names
    /// the concept by its preferred label rather than by the hidden one it matched.
    #[test]
    fn a_hidden_label_is_matched_and_never_becomes_the_concepts_name() {
        let (_directory, store) = store_with(PACKAGING);
        let report = search(&store, VOCABULARY, &query("carrier-bag"), false)
            .expect("a readable vocabulary");

        assert!(report.contains("\"carrier-bag\"@en"), "{report}");
        assert!(report.contains("under skos:hiddenLabel"), "{report}");
        assert!(
            report.contains(
                "SKOS §5.1 gives it for search and says it should not otherwise be shown"
            ),
            "{report}"
        );
        assert!(
            report.contains("<http://example.org/bag>  (\"Carrier bag\"@en)"),
            "the concept is named by its preferred label, never by the hidden one: {report}"
        );
    }

    /// A label the vocabulary holds only as SKOS-XL is searchable, and the hit says which rule put
    /// it within reach — `CLAUDE.md` §3: nothing inferred without a derivation.
    #[test]
    fn a_skos_xl_label_is_found_and_says_why_it_was_reachable() {
        let (_directory, store) = store_with(PACKAGING);
        let report =
            search(&store, VOCABULARY, &query("tote"), false).expect("a readable vocabulary");

        assert!(report.contains("<http://example.org/tote>"), "{report}");
        assert!(
            report.contains("which the vocabulary does not state"),
            "{report}"
        );
        assert!(
            report.contains("skosxl:prefLabel, skosxl:literalForm"),
            "the S55 chain is quoted, not merely cited: {report}"
        );
    }

    /// §5 puts labels on resources of any type. A scheme or a collection that matches is a real
    /// hit and is said not to be a concept, because a reader scans the list as if it were one.
    #[test]
    fn a_hit_that_is_not_a_concept_says_so() {
        let (_directory, store) = store_with(PACKAGING);
        let report =
            search(&store, VOCABULARY, &query("Baggage"), false).expect("a readable vocabulary");

        assert!(
            report.contains("not a concept: skos:Collection"),
            "{report}"
        );

        let scheme =
            search(&store, VOCABULARY, &query("thesaurus"), false).expect("a readable vocabulary");
        assert!(
            scheme.contains("not a concept: skos:ConceptScheme"),
            "{scheme}"
        );
    }

    /// Nothing found is the report that matters most, because it is the one that sends a user off
    /// to create a duplicate. It says what it read and names the two reasons a match is missed.
    #[test]
    fn nothing_found_says_what_was_read_and_why_a_match_might_have_been_missed() {
        let (_directory, store) = store_with(PACKAGING);
        let report =
            search(&store, VOCABULARY, &query("kayak"), false).expect("a readable vocabulary");

        assert!(report.contains("nothing matched"), "{report}");
        assert!(report.contains("label(s) on"), "{report}");
        assert!(
            report.contains("ignores case but not accents or spelling"),
            "{report}"
        );
    }

    /// **The regression this test was written for.** A bound of zero suppressed every hit and the
    /// report then said "nothing matched" — a false negative in the one command whose false
    /// negatives cause duplicate concepts. What matched and what is shown are two numbers.
    #[test]
    fn a_bound_that_shows_nothing_does_not_claim_nothing_matched() {
        let (_directory, store) = store_with(PACKAGING);
        let report = search(
            &store,
            VOCABULARY,
            &query("bag").with_bound(SearchBound { max_hits: 0 }),
            false,
        )
        .expect("a readable vocabulary");

        assert!(
            !report.contains("nothing matched"),
            "labels matched; only the bound stopped them being shown: {report}"
        );
        assert!(
            report.contains("matched") && report.contains("none of them are listed"),
            "{report}"
        );
    }

    /// The bound truncates the answer and admits it, rather than presenting a partial list as the
    /// whole one.
    #[test]
    fn a_truncated_answer_says_it_is_not_the_whole_answer() {
        let (_directory, store) = store_with(PACKAGING);
        let report = search(
            &store,
            VOCABULARY,
            &query("bag").with_bound(SearchBound { max_hits: 2 }),
            false,
        )
        .expect("a readable vocabulary");

        assert!(report.contains("this is not the whole answer"), "{report}");
        assert!(!report.contains("that is all of them."), "{report}");
    }

    /// The header states the query that actually ran, so a report read later — or pasted into a
    /// ticket — cannot be mistaken for a broader search than it was.
    #[test]
    fn the_header_states_the_narrowing_that_was_applied() {
        let (_directory, store) = store_with(PACKAGING);
        let report = search(
            &store,
            VOCABULARY,
            &query("bag")
                .with_mode(MatchMode::Prefix)
                .with_language(LanguageFilter::Range(
                    LanguageRange::parse("en").expect("a range"),
                ))
                .with_kinds([LabelKind::Preferred])
                .expect("one kind"),
            false,
        )
        .expect("a readable vocabulary");

        assert!(
            report.contains("matched at the start of the label"),
            "{report}"
        );
        assert!(report.contains("language range en"), "{report}");
        assert!(report.contains("preferred label(s)"), "{report}");
        assert!(
            !report.contains("hidden"),
            "hidden labels were excluded, so nothing about them belongs in the report: {report}"
        );
    }

    /// The untagged label is the one a multilingual programme audits for, and the wildcard range
    /// does not select it — RFC 4647's wildcard matches any *tag*, and it has none.
    #[test]
    fn untagged_labels_are_their_own_filter() {
        let (_directory, store) = store_with(PACKAGING);
        let report = search(
            &store,
            VOCABULARY,
            &query("bag").with_language(LanguageFilter::Untagged),
            false,
        )
        .expect("a readable vocabulary");

        assert!(report.contains("<http://example.org/untagged>"), "{report}");
        assert!(!report.contains("<http://example.org/paper>"), "{report}");
        assert!(report.contains("labels with no language tag"), "{report}");
    }

    /// A vocabulary that is not registered is refused rather than answered with an empty search,
    /// which would read as "the term does not exist" for a graph nobody has loaded.
    #[test]
    fn an_unregistered_vocabulary_is_refused_rather_than_reported_empty() {
        let (_directory, store) = store_with(PACKAGING);
        let error = search(&store, "http://example.org/absent", &query("bag"), false)
            .expect_err("no such vocabulary");

        assert!(
            error.to_string().contains("no graph is registered"),
            "{error}"
        );
    }

    /// A retired concept with a successor, and a retired concept with none — the two states a
    /// searcher has to be able to tell apart, because only the first gives them somewhere to go.
    const RETIRED_BAGS: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix owl: <http://www.w3.org/2002/07/owl#> .
        @prefix dcterms: <http://purl.org/dc/terms/> .
        @prefix ex: <http://example.org/> .

        ex:bag a skos:Concept ; skos:prefLabel "Carrier bag"@en ;
            owl:deprecated true ; dcterms:isReplacedBy ex:reusable .
        ex:reusable a skos:Concept ; skos:prefLabel "Reusable bag"@en .
        ex:sack a skos:Concept ; skos:prefLabel "Paper bag"@en ; owl:deprecated true .
    "#;

    /// The command this matters most in, and the one place a marker alone is not enough. Someone
    /// searching for a term is choosing one to use: told only "[retired]" with no successor named
    /// they conclude the vocabulary has nothing, and create the duplicate §1.7 exists to prevent.
    #[test]
    fn a_retired_hit_carries_the_full_account_and_the_successor() {
        let (_directory, store) = store_with(RETIRED_BAGS);
        let report =
            search(&store, VOCABULARY, &query("carrier"), false).expect("a readable vocabulary");

        assert!(report.contains("<http://example.org/bag>"), "{report}");
        assert!(report.contains("[retired]"), "{report}");
        assert!(
            report.contains("use instead, by dcterms:isReplacedBy"),
            "{report}"
        );
        assert!(
            report.contains("<http://example.org/reusable>  (\"Reusable bag\"@en)"),
            "the successor is named, not merely said to exist: {report}"
        );
    }

    /// Nothing is hidden, and the report says why rather than leaving it as an unexplained choice.
    #[test]
    fn retired_hits_are_counted_and_shown_rather_than_omitted() {
        let (_directory, store) = store_with(RETIRED_BAGS);
        let report =
            search(&store, VOCABULARY, &query("bag"), false).expect("a readable vocabulary");

        assert!(report.contains("3 label(s) matched"), "{report}");
        assert!(
            report.contains("2 of the 3 concept(s) shown are retired"),
            "{report}"
        );
        assert!(
            report.contains("which is how a duplicate gets created"),
            "{report}"
        );
        // The current concept is still there and still unmarked.
        assert!(
            report.contains("<http://example.org/reusable>  (\"Reusable bag\"@en)\n"),
            "{report}"
        );
    }

    /// A term gone out of use with no successor is an ordinary editorial act, and the searcher is
    /// told that the absence is the vocabulary's answer rather than this report's omission.
    #[test]
    fn a_retired_hit_with_no_successor_says_so_plainly() {
        let (_directory, store) = store_with(RETIRED_BAGS);
        let report =
            search(&store, VOCABULARY, &query("paper"), false).expect("a readable vocabulary");

        assert!(report.contains("<http://example.org/sack>"), "{report}");
        assert!(
            report.contains("nothing is recorded as replacing it"),
            "{report}"
        );
        assert!(!report.contains("use instead"), "{report}");
    }

    /// A vocabulary that retires nothing reads exactly as it did.
    #[test]
    fn a_vocabulary_with_no_retirements_says_nothing_about_them() {
        let (_directory, store) = store_with(PACKAGING);
        let report =
            search(&store, VOCABULARY, &query("bag"), false).expect("a readable vocabulary");

        assert!(!report.contains("retired"), "{report}");
    }

    /// The request the flag exists for: a curator building a new branch wants the list without the
    /// obsolete terms in it. They get it, and they are told what it cost them.
    #[test]
    fn current_only_leaves_the_retired_hits_out_and_says_how_many() {
        let (_directory, store) = store_with(RETIRED_BAGS);
        let report =
            search(&store, VOCABULARY, &query("bag"), true).expect("a readable vocabulary");

        assert!(
            report.contains("<http://example.org/reusable>  (\"Reusable bag\"@en)"),
            "{report}"
        );
        assert!(!report.contains("<http://example.org/bag>"), "{report}");
        assert!(!report.contains("<http://example.org/sack>"), "{report}");
        assert!(report.contains("current concepts only"), "{report}");
        assert!(
            report.contains("2 more label(s) matched, on 2 retired concept(s)"),
            "{report}"
        );
        assert!(
            report.contains("run the same search without --current"),
            "the way back is one line away, not a thing to work out: {report}"
        );
    }

    /// **The outcome that makes the flag dangerous, and the test that pins the mitigation.** Every
    /// match was on a retired concept, so without the withheld count this report would read
    /// "nothing matched" about a term the vocabulary holds — which is how a duplicate gets
    /// created (`CLAUDE.md` §1.7). `docs/adr/0043`.
    #[test]
    fn current_only_never_reports_an_empty_search_when_retired_concepts_matched() {
        let (_directory, store) = store_with(RETIRED_BAGS);
        let report =
            search(&store, VOCABULARY, &query("carrier"), true).expect("a readable vocabulary");

        assert!(report.contains("nothing matched"), "{report}");
        assert!(
            report.contains("1 more label(s) matched, on 1 retired concept(s)"),
            "the absence is explained rather than left to be read as non-existence: {report}"
        );
        assert!(report.contains("They are in this vocabulary"), "{report}");
    }

    /// The bound is spent on the hits that survive the flag, which is why the exclusion happens
    /// inside the search and not over its answer. Filtering afterwards would show nothing here and
    /// call it the whole answer.
    #[test]
    fn current_only_does_not_let_retired_hits_use_up_the_limit() {
        let (_directory, store) = store_with(RETIRED_BAGS);
        let report = search(
            &store,
            VOCABULARY,
            &query("bag").with_bound(SearchBound { max_hits: 1 }),
            true,
        )
        .expect("a readable vocabulary");

        assert!(
            report.contains("<http://example.org/reusable>"),
            "the one slot went to the one current concept: {report}"
        );
        assert!(report.contains("that is all of them."), "{report}");
    }

    /// A concept naming a successor without carrying the marker is one every command here reads as
    /// **current**, so `--current` keeps it — and keeps its mark, which is the only thing telling
    /// the reader the vocabulary is of two minds about it.
    #[test]
    fn current_only_keeps_a_concept_that_is_replaced_but_not_marked_retired() {
        const HALF: &str = r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix dcterms: <http://purl.org/dc/terms/> .
            @prefix ex: <http://example.org/> .

            ex:bag a skos:Concept ; skos:prefLabel "Carrier bag"@en ;
                dcterms:isReplacedBy ex:reusable .
            ex:reusable a skos:Concept ; skos:prefLabel "Reusable bag"@en .
        "#;
        let (_directory, store) = store_with(HALF);
        let report =
            search(&store, VOCABULARY, &query("carrier"), true).expect("a readable vocabulary");

        assert!(report.contains("<http://example.org/bag>"), "{report}");
        assert!(
            report.contains("[replaced, but not marked retired]"),
            "{report}"
        );
        assert!(
            !report.contains("more label(s) matched"),
            "nothing was withheld, so nothing is claimed to have been: {report}"
        );
    }

    /// Asking for current concepts only in a vocabulary that has retired nothing changes no word
    /// of the answer except the line saying what was asked for.
    #[test]
    fn current_only_over_a_vocabulary_with_no_retirements_withholds_nothing() {
        let (_directory, store) = store_with(PACKAGING);
        let asked = search(&store, VOCABULARY, &query("bag"), true).expect("a readable vocabulary");
        let plain =
            search(&store, VOCABULARY, &query("bag"), false).expect("a readable vocabulary");

        assert!(!asked.contains("more label(s) matched"), "{asked}");
        assert_eq!(
            asked.replace(
                "current concepts only: the vocabulary's retired concepts were matched and left \
                 out.\n",
                ""
            ),
            plain
        );
    }
}
