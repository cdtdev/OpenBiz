//! `openbiz mint` — the IRI a new concept would be given, and everything behind that choice.
//!
//! # Why minting is a command of its own, and why it writes nothing
//!
//! There is no "create concept" command in this build, on purpose: `CLAUDE.md` §1.7 puts
//! discovery before creation and the `DiscoveryProvider` item is still ahead of us in the plan.
//! What exists today is the candidate seam — a change is proposed in a file, staged, read, and
//! approved — and to write that file somebody has to decide what the new concept's IRI will be.
//! Today they decide it by copying an existing IRI and editing the end of it, which is how a
//! vocabulary ends up with `c_00123` beside `c_124` and two concepts sharing one IRI.
//!
//! So this command answers exactly that question and does nothing else. It reads; it stages
//! nothing; **it reserves nothing**. Run it twice and it returns the same IRI both times, and the
//! report says so in as many words, because a minter that looks like an allocator is worse than
//! no minter at all. The IRI becomes taken when a candidate carrying it is staged — and the next
//! mint sees it, because the scan reads staged changes as well as vocabularies.
//!
//! # What is read
//!
//! Three things, and the report names all three:
//!
//! 1. **The vocabulary's own convention.** The namespace most of its concepts are already in, and
//!    whether their local names are numbered or worded, is the evidence for the default pattern.
//!    A vocabulary that has no majority namespace gets no suggestion rather than a confident
//!    wrong one, and `--pattern` is then required.
//! 2. **Every IRI under that pattern's prefix, anywhere in the store.** Not just the target
//!    vocabulary: an IRI is a global identifier, and a deployment where two vocabularies extend
//!    the same namespace is the ordinary case in an enterprise, not an exotic one. Only IRIs
//!    under the prefix are kept, so the memory this costs is the size of the namespace and not
//!    the size of the store.
//! 3. **The labels the vocabulary already carries.** §1.7 again: if something here is already
//!    called what you are about to call the new concept, minting an IRI is the wrong next step,
//!    and the report says so before it says anything else. This is not the discovery pass that
//!    Phase 12 will bring — it is one exact-label lookup in one vocabulary — and it says which it
//!    is rather than letting a quiet "nothing found" be read as "nothing exists".

use openbiz_skos::{
    mint as mint_iri, CoreModel, IriConvention, LabelKind, LabelQuery, MatchMode, MintDerivation,
    MintPattern, MintScan, Minted, Node, Resource, SkosClass, SlugBound, Suggestion,
};
use openbiz_store::{CandidateState, GraphKind, Store};

use crate::cli::CommandError;
use crate::inspect::convert;

/// Report the IRI a new concept in `graph` would be minted with. Reads and nothing else.
pub fn mint(
    store: &Store,
    graph: &str,
    label: Option<&str>,
    pattern: Option<&str>,
) -> Result<String, CommandError> {
    let mut builder = CoreModel::builder();
    store.for_each_statement(graph, |statement| builder.push(convert(statement)))?;
    let model = builder.build();

    // The same question asked of the changes waiting for a decision. The IRI half of this report
    // reads them, so a label half that does not would have the report saying "nothing here is
    // called that" directly above "the IRI is taken by candidate 2" — two true sentences that read
    // as a contradiction. Found by running the command.
    let staged = staged_models(store, graph)?;

    let mut convention = IriConvention::new();
    for (node, _) in model.instances_of(SkosClass::Concept) {
        match node.as_iri() {
            Some(iri) => convention.push(iri),
            None => convention.push_blank(),
        }
    }

    // An explicit pattern wins and the suggestion is still computed, so the report can say what
    // the vocabulary would have chosen — a `--pattern` that disagrees with the vocabulary's own
    // convention is exactly the thing worth showing somebody before they use it.
    let suggested = convention.suggest();
    let chosen = match pattern {
        Some(text) => MintPattern::parse(text)?,
        None => match &suggested {
            Ok(suggestion) => suggestion.pattern.clone(),
            Err(error) => return Err(CommandError::NoConvention(error.clone())),
        },
    };

    let scan = scan_for(store, graph, chosen.prefix())?;
    let minted = mint_iri(&chosen, label, SlugBound::DEFAULT, &scan);

    // `openbiz-skos` is engine-free and can only apply a subset of RFC 3987. The parser that will
    // actually store this IRI is entitled to the last word, and asking it here is the difference
    // between "we think this is an IRI" and "the store accepts it".
    if let Ok(minted) = &minted {
        if !openbiz_store::accepts_iri(&minted.iri) {
            return Err(CommandError::NotAnIri {
                iri: minted.iri.clone(),
            });
        }
    }

    Ok(report(
        graph,
        label,
        &chosen,
        pattern.is_some(),
        &suggested,
        &scan,
        &minted,
        &model,
        &staged,
    ))
}

/// A model of each change staged against `graph` and still waiting for a decision.
///
/// Only this vocabulary's own pending changes. A label in *another* vocabulary is a real and
/// useful thing to know about and it is a different question — that is the discovery pass §1.7
/// promises and Phase 12 builds, and answering a fraction of it here under the same heading would
/// misreport how far this looked.
fn staged_models(store: &Store, graph: &str) -> Result<Vec<(String, CoreModel)>, CommandError> {
    let mut staged = Vec::new();
    for candidate in store.candidates()? {
        if candidate.state() != CandidateState::Proposed || candidate.target().iri() != graph {
            continue;
        }
        let Some(payload) = candidate.payload() else {
            continue;
        };
        let mut builder = CoreModel::builder();
        store.for_each_statement(payload.iri(), |statement| builder.push(convert(statement)))?;
        staged.push((format!("candidate {}", candidate.id()), builder.build()));
    }
    Ok(staged)
}

/// Every IRI in the store that begins with `prefix`, and where each was found.
///
/// Vocabularies first, then the staged changes, because [`MintScan`] keeps the first source to
/// mention an IRI: a collision with a vocabulary must not be reported as a collision with a
/// candidate that merely repeats what the vocabulary already says.
fn scan_for(store: &Store, target: &str, prefix: &str) -> Result<MintScan, CommandError> {
    let mut scan = MintScan::under(prefix);

    for graph in store.graphs()? {
        if graph.kind() != GraphKind::Vocabulary {
            continue;
        }
        let source = match graph.iri() == target {
            true => "this vocabulary".to_owned(),
            false => format!("the vocabulary {}", graph.iri()),
        };
        store.for_each_statement(graph.iri(), |statement| {
            push_iris(&mut scan, statement, &source)
        })?;
    }

    for candidate in store.candidates()? {
        // Only the ones still waiting. An applied candidate's statements are in the vocabulary
        // and were counted there; a rejected one's stay staged forever as the record of what was
        // refused, and an IRI that was refused never denoted anything, so it is free.
        if candidate.state() != CandidateState::Proposed {
            continue;
        }
        let Some(payload) = candidate.payload() else {
            continue;
        };
        let source = format!(
            "candidate {}, which is waiting for a decision",
            candidate.id()
        );
        store.for_each_statement(payload.iri(), |statement| {
            push_iris(&mut scan, statement, &source)
        })?;
    }

    Ok(scan)
}

/// Offer every IRI a statement mentions, in all three positions.
///
/// The predicate counts. A concept minted onto an IRI a property already uses is a different
/// silent collision from the concept-on-concept one, and it is no less permanent.
fn push_iris(scan: &mut MintScan, statement: openbiz_store::StatementRef<'_>, source: &str) {
    for term in [statement.subject, statement.object] {
        if let openbiz_store::StatementTerm::Iri(iri) = term {
            scan.push(iri, source);
        }
    }
    scan.push(statement.predicate, source);
}

/// The report, kept apart from the store so it can be tested against parts in hand.
#[allow(clippy::too_many_arguments)]
fn report(
    graph: &str,
    label: Option<&str>,
    pattern: &MintPattern,
    pattern_was_given: bool,
    suggested: &Result<Suggestion, openbiz_skos::NoConvention>,
    scan: &MintScan,
    minted: &Result<Minted, openbiz_skos::MintError>,
    model: &CoreModel,
    staged: &[(String, CoreModel)],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("an IRI for a new concept in {graph}\n"));
    match label {
        Some(label) => out.push_str(&format!("to be called {label:?}\n")),
        None => out.push_str("with no label given\n"),
    }

    // §1.7 first, before the IRI: if the vocabulary already calls something this, the next step is
    // not to mint anything.
    out.push_str(&already_called(model, staged, label));

    out.push_str(&format!("\npattern: {pattern}\n"));
    out.push_str(match pattern.policy() {
        openbiz_skos::MintPolicy::Opaque => {
            "  an opaque IRI: the local name means nothing, so nothing about the concept can \
             make it wrong\n"
        }
        openbiz_skos::MintPolicy::Readable => {
            "  a readable IRI: the local name comes from the label, and is never revised when \
             the label changes\n"
        }
    });
    out.push_str(&source_of_pattern(pattern, pattern_was_given, suggested));

    out.push_str(&format!(
        "\nchecked against {} IRI(s) under {}, out of {} read:\n",
        scan.len(),
        pattern.prefix(),
        scan.offered()
    ));
    match scan.is_empty() {
        true => out.push_str("  nothing in this store uses this namespace yet\n"),
        false => {
            for (source, count) in scan.sources() {
                out.push_str(&format!("  {count} in {source}\n"));
            }
        }
    }

    match minted {
        Ok(minted) => {
            out.push_str(&format!("\nminted: {}\n", minted.iri));
            out.push_str(&derivation(&minted.derivation));
        }
        Err(error) => {
            out.push_str(&format!("\nnothing was minted: {error}\n"));
            if let openbiz_skos::MintError::Taken { .. } = error {
                out.push_str(
                    "  no disambiguating suffix is offered: a second concept with the same label \
                     is the duplicate this build exists to prevent (CLAUDE.md §1.7). Reuse the \
                     concept that holds the IRI, or qualify the term — mint from \"Java \
                     (programming language)\" rather than from \"Java\".\n",
                );
            }
        }
    }

    // Last, and never omitted. A reader who takes this for an allocator will mint twice and
    // create two concepts on one IRI, which is the exact failure the command exists to prevent.
    out.push_str(
        "\nnothing was written and nothing is reserved: run this again and it answers the same. \
         The IRI becomes taken when a change carrying it is staged — `openbiz import` — and the \
         next mint sees it there.\n",
    );
    out
}

/// Where the pattern came from: the operator, or the vocabulary's own concepts.
fn source_of_pattern(
    pattern: &MintPattern,
    pattern_was_given: bool,
    suggested: &Result<Suggestion, openbiz_skos::NoConvention>,
) -> String {
    let mut out = String::new();
    if !pattern_was_given {
        if let Ok(suggestion) = suggested {
            let evidence = &suggestion.evidence;
            out.push_str(&format!(
                "  read off this vocabulary: {} of its {} concept(s) are in {}",
                evidence.namespace_count, evidence.concepts, evidence.namespace
            ));
            match &evidence.fixed_part {
                Some((fixed, count)) => out.push_str(&format!(
                    ", and {count} of those have a number after {fixed:?}\n"
                )),
                None => out.push_str(&format!(
                    ", and {} of those have a numbered local name, which is not most of them\n",
                    evidence.numbered
                )),
            }
            if evidence.namespaces > 1 {
                out.push_str(&format!(
                    "  {} namespace(s) are in use here; the others are not minted into\n",
                    evidence.namespaces
                ));
            }
            out.push_str("  give --pattern to override it\n");
        }
        return out;
    }

    out.push_str("  given with --pattern\n");
    match suggested {
        Ok(suggestion) if suggestion.pattern == *pattern => {
            out.push_str("  which is also what this vocabulary's own concepts suggest\n")
        }
        // Worth saying loudly. A pattern that disagrees with the vocabulary is legitimate — it is
        // how a convention gets changed — and it is also how somebody mints into the wrong
        // namespace without noticing.
        Ok(suggestion) => out.push_str(&format!(
            "  this vocabulary's own concepts suggest {} instead; minting under a different \
             pattern is legitimate and it is also how a concept ends up in the wrong namespace\n",
            suggestion.pattern
        )),
        Err(error) => out.push_str(&format!(
            "  this vocabulary suggests nothing to compare it with: {error}\n"
        )),
    }
    out
}

/// The derivation, which `CLAUDE.md` §3 requires of every answer this build gives.
fn derivation(derivation: &MintDerivation) -> String {
    match derivation {
        MintDerivation::FromLabel { label, slug } => {
            let mut out = format!("  from the label {label:?}, reduced to {:?}\n", slug.text());
            if slug.truncated() {
                out.push_str(
                    "  the label was longer than a local name should be, so it was cut at a word \
                     boundary: the IRI no longer says the whole term\n",
                );
            }
            out.push_str(
                "  accented and non-Latin characters are kept rather than transliterated: RFC \
                 3987 §2.2 allows them in an IRI, and mapping them to ASCII is a guess that \
                 differs by language\n",
            );
            out.push_str(
                "  this is the trade a readable IRI makes: if the label is later corrected, the \
                 IRI stays as it is, because an IRI that changes is a different concept\n",
            );
            out
        }
        MintDerivation::Numbered {
            number,
            width,
            above,
        } => {
            let mut out = String::new();
            match above {
                Some(highest) => {
                    out.push_str(&format!(
                        "  the highest number in use under this pattern is {}, in {}\n",
                        highest.number, highest.iri
                    ));
                    out.push_str(&format!(
                        "  {number} is above it, not the lowest free number: a gap is evidence \
                         that something was once there, and an IRI must never come back attached \
                         to a different concept\n"
                    ));
                    // Only when the vocabulary really pads. Two digits is not a two-digit
                    // convention, and saying it of `c_12` states something untrue about the
                    // vocabulary — which running the command against a store is what caught.
                    if highest.pads() {
                        out.push_str(&format!(
                            "  written with {width} digits, which is how this vocabulary writes \
                             them: {}\n",
                            highest.iri
                        ));
                    }
                }
                None => out.push_str(
                    "  nothing in this store uses this pattern yet, so this is the first\n",
                ),
            }
            out
        }
    }
}

/// What the vocabulary already calls by this label — the §1.7 check, run before anything is
/// minted.
fn already_called(
    model: &CoreModel,
    staged: &[(String, CoreModel)],
    label: Option<&str>,
) -> String {
    let Some(label) = label else {
        return "\nno label was given, so nothing was checked for an existing concept of the same \
                name; give one and this looks first\n"
            .to_owned();
    };
    let Ok(query) = LabelQuery::new(label).map(|query| query.with_mode(MatchMode::Exact)) else {
        return String::new();
    };
    let mut lines = String::new();
    let mut matched = 0;
    for (where_, model) in std::iter::once((&"this vocabulary".to_owned(), model))
        .chain(staged.iter().map(|(source, model)| (source, model)))
    {
        let found = model.search(&query);
        matched += found.matched();
        for hit in found.hits() {
            lines.push_str(&format!(
                "  {}{}  {}, in {where_}\n",
                hit.resource,
                named(model, &hit.resource),
                match hit.best_kind() {
                    Some(LabelKind::Preferred) => "skos:prefLabel",
                    Some(LabelKind::Alternative) => "skos:altLabel",
                    Some(LabelKind::Hidden) => "skos:hiddenLabel",
                    None => "labelled",
                }
            ));
        }
    }

    if matched == 0 {
        return format!(
            "\nnothing is already called {label:?}, in this vocabulary or in the {} change(s) \
             staged against it — matched against whole labels of every kind, in every language. \
             That is one exact lookup in one vocabulary and not a discovery pass: a differently \
             spelled or accented term here will not have been seen.\n",
            staged.len()
        );
    }

    let mut out = format!("\nSTOP — {matched} label(s) already match {label:?} exactly:\n");
    out.push_str(&lines);
    out.push_str(
        "reuse outranks creation (CLAUDE.md §1.7). An IRI may still be minted below, because two \
         concepts can legitimately share a label — but if one of these is the concept you mean, \
         minting a second one is how a vocabulary becomes a silo.\n",
    );
    out
}

/// A resource's preferred label in parentheses, never a hidden one — SKOS §5.1.
fn named(model: &CoreModel, node: &Node) -> String {
    match model.resource(node).and_then(Resource::display_label) {
        Some(label) => format!("  ({label})"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use openbiz_store::{CandidateSource, Decision, GraphId, Provenance, RdfSyntax, Store};

    use super::mint;

    const VOCABULARY: &str = "https://example.org/energy";
    const OTHER: &str = "https://example.org/materials";

    /// A store with one registered vocabulary per entry, each loaded through the candidate seam
    /// exactly as a user's data arrives.
    fn store_with(graphs: &[(&str, &str)]) -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(directory.path()).expect("an openable store");
        for (iri, turtle) in graphs {
            let target = GraphId::vocabulary(*iri).expect("a valid vocabulary IRI");
            store
                .create_vocabulary_graph(&target)
                .expect("a fresh registration");
            // An empty entry registers the vocabulary and loads nothing: the store refuses an
            // import with no statements in it, and a vocabulary on its first day is exactly the
            // case worth testing.
            if !turtle.trim().is_empty() {
                let candidate = propose(&store, &target, turtle);
                store
                    .decide(candidate, Decision::Approve, "test")
                    .expect("an approvable candidate");
            }
        }
        (directory, store)
    }

    /// Stage a change without deciding it, which is what makes its IRIs taken.
    fn propose(store: &Store, target: &GraphId, turtle: &str) -> openbiz_store::CandidateId {
        store
            .propose_import(
                target,
                RdfSyntax::Turtle,
                turtle.as_bytes(),
                &Provenance {
                    source: CandidateSource::Import,
                    agent: "test".to_owned(),
                    note: "fixture".to_owned(),
                    confidence: None,
                },
            )
            .expect("a well-formed proposal")
            .id()
    }

    /// Numbered local names with a gap at 2, which the mint must not fill.
    const NUMBERED: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix ex: <https://example.org/energy/> .

        ex:c_1 a skos:Concept ; skos:prefLabel "Renewable energy"@en .
        ex:c_3 a skos:Concept ; skos:prefLabel "Solar power"@en .
        ex:c_12 a skos:Concept ; skos:prefLabel "Wind power"@en .
    "#;

    /// Worded local names, which suggest a readable pattern instead.
    const WORDED: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix ex: <https://example.org/energy/> .

        ex:renewable-energy a skos:Concept ; skos:prefLabel "Renewable energy"@en .
        ex:solar-power a skos:Concept ; skos:prefLabel "Solar power"@en .
    "#;

    /// **The command's whole point.** The pattern is evidence from the vocabulary, and the number
    /// goes above the highest in use rather than filling the gap at 2.
    #[test]
    fn the_pattern_is_read_off_the_vocabulary_and_the_number_goes_above_the_highest() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED)]);
        let report = mint(&store, VOCABULARY, Some("Tidal power"), None).expect("a mint");

        assert!(
            report.contains("pattern: https://example.org/energy/c_{n}"),
            "{report}"
        );
        assert!(report.contains("read off this vocabulary"), "{report}");
        assert!(
            report.contains("minted: https://example.org/energy/c_13"),
            "{report}"
        );
        assert!(
            report.contains("not the lowest free number"),
            "the gap at 2 is left alone, and the report says why: {report}"
        );
    }

    /// **The defect running the command against a store found.** `c_1`, `c_3` and `c_12` are
    /// written with one, one and two digits and pad nothing, and the report claimed two digits
    /// was "how this vocabulary writes them". A number's width is not evidence of a convention.
    #[test]
    fn an_unpadded_vocabulary_is_not_described_as_padded() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED)]);
        let report = mint(&store, VOCABULARY, Some("Tidal power"), None).expect("a mint");

        assert!(
            !report.contains("how this vocabulary writes them"),
            "nothing here pads: {report}"
        );
    }

    /// The other half: a vocabulary that does pad keeps the padding and names the IRI it read it
    /// from, so the claim can be checked.
    #[test]
    fn a_padded_vocabulary_keeps_its_padding_and_cites_it() {
        let (_directory, store) = store_with(&[(
            VOCABULARY,
            r#"@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
               <https://example.org/energy/c_0912> a skos:Concept ."#,
        )]);
        let report = mint(&store, VOCABULARY, Some("Tidal power"), None).expect("a mint");

        assert!(
            report.contains("minted: https://example.org/energy/c_0913"),
            "{report}"
        );
        assert!(
            report.contains(
                "written with 4 digits, which is how this vocabulary writes them: \
                             https://example.org/energy/c_0912"
            ),
            "{report}"
        );
    }

    /// A vocabulary whose local names are words gets a readable pattern, and the report states the
    /// trade that makes rather than leaving it to be inferred from the shape of the IRI.
    #[test]
    fn a_worded_vocabulary_mints_a_readable_iri_and_names_the_trade() {
        let (_directory, store) = store_with(&[(VOCABULARY, WORDED)]);
        let report = mint(&store, VOCABULARY, Some("Tidal power"), None).expect("a mint");

        assert!(
            report.contains("minted: https://example.org/energy/tidal-power"),
            "{report}"
        );
        assert!(report.contains("a readable IRI"), "{report}");
        assert!(
            report.contains("the IRI stays as it is"),
            "the promise a readable IRI cannot keep is stated: {report}"
        );
    }

    /// **The collision nobody else checks.** A change that is staged and not yet approved holds
    /// its IRIs against the next mint, so two curators preparing imports on the same day cannot
    /// mint the same IRI and silently merge two concepts on approval.
    #[test]
    fn a_staged_change_holds_its_iris_against_the_next_mint() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED)]);
        let target = GraphId::vocabulary(VOCABULARY).expect("a valid IRI");
        propose(
            &store,
            &target,
            r#"@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
               <https://example.org/energy/c_13> a skos:Concept ."#,
        );

        let report = mint(&store, VOCABULARY, Some("Tidal power"), None).expect("a mint");

        assert!(
            report.contains("minted: https://example.org/energy/c_14"),
            "{report}"
        );
        assert!(
            report.contains("candidate 2, which is waiting for a decision"),
            "the report names where the taken IRI was found: {report}"
        );
    }

    /// An IRI is a global identifier, so a second vocabulary in the same store extending the same
    /// namespace is a real collision and not somebody else's problem.
    #[test]
    fn an_iri_another_vocabulary_uses_is_not_minted_again() {
        let (_directory, store) = store_with(&[
            (VOCABULARY, NUMBERED),
            (
                OTHER,
                r#"@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
                   <https://example.org/energy/c_20> a skos:Concept ;
                     skos:prefLabel "Borrowed"@en ."#,
            ),
        ]);

        let report = mint(&store, VOCABULARY, Some("Tidal power"), None).expect("a mint");

        assert!(
            report.contains("minted: https://example.org/energy/c_21"),
            "{report}"
        );
        assert!(
            report.contains("the vocabulary https://example.org/materials"),
            "{report}"
        );
    }

    /// **`CLAUDE.md` §1.7 in one report.** Something here is already called this, and that is said
    /// before the IRI, because minting one is the wrong next step.
    #[test]
    fn a_label_the_vocabulary_already_uses_stops_the_report_first() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED)]);
        let report = mint(&store, VOCABULARY, Some("Solar power"), None).expect("a mint");

        let stop = report.find("STOP").expect("the §1.7 warning");
        let minted = report.find("minted:").expect("an IRI");
        assert!(stop < minted, "the warning comes first: {report}");
        assert!(
            report.contains("https://example.org/energy/c_3"),
            "the concept that already holds the label is named: {report}"
        );
        assert!(report.contains("reuse outranks creation"), "{report}");
    }

    /// The §1.7 check is one exact lookup, and says so. A quiet "nothing found" that reads as
    /// "nothing exists" is the report that creates duplicates.
    #[test]
    fn a_clean_label_check_says_how_narrow_it_was() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED)]);
        let report = mint(&store, VOCABULARY, Some("Tidal power"), None).expect("a mint");

        assert!(report.contains("nothing is already called"), "{report}");
        assert!(report.contains("not a discovery pass"), "{report}");
    }

    /// **The contradiction running the command produced.** The IRI half of the report reads the
    /// changes staged against a vocabulary, so a label half that read only the vocabulary printed
    /// "nothing is already called that" directly above "the IRI is taken by candidate 2".
    #[test]
    fn a_label_only_a_staged_change_carries_is_still_found() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED)]);
        let target = GraphId::vocabulary(VOCABULARY).expect("a valid IRI");
        propose(
            &store,
            &target,
            r#"@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
               <https://example.org/energy/c_13> a skos:Concept ;
                 skos:prefLabel "Tidal power"@en ."#,
        );

        let report = mint(&store, VOCABULARY, Some("Tidal power"), None).expect("a mint");

        assert!(report.contains("STOP"), "{report}");
        assert!(
            report.contains("in candidate 2"),
            "the report says where the label is, and it is not in the vocabulary yet: {report}"
        );
        assert!(
            !report.contains("nothing is already called"),
            "the two halves of the report must not contradict each other: {report}"
        );
    }

    /// With no label there is nothing to look up, and the report says that rather than printing a
    /// reassuring silence.
    #[test]
    fn no_label_says_the_duplicate_check_did_not_run() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED)]);
        let report = mint(&store, VOCABULARY, None, None).expect("a mint");

        assert!(
            report.contains("nothing was checked for an existing concept"),
            "{report}"
        );
        assert!(
            report.contains("minted: https://example.org/energy/c_13"),
            "{report}"
        );
    }

    /// **The §1.7 refusal.** A `-2` suffix is a duplicate concept with a tidier IRI.
    #[test]
    fn a_taken_slug_is_refused_and_offers_no_suffix() {
        let (_directory, store) = store_with(&[(VOCABULARY, WORDED)]);
        let report = mint(&store, VOCABULARY, Some("Solar power"), None).expect("a report");

        assert!(report.contains("nothing was minted"), "{report}");
        assert!(report.contains("already in use"), "{report}");
        assert!(
            report.contains("no disambiguating suffix is offered"),
            "{report}"
        );
        assert!(
            report.contains("qualify the term"),
            "the way out is named: {report}"
        );
    }

    /// The trap a minter that looks like an allocator sets: mint twice, get two concepts on one
    /// IRI. The answer is the same both times and the report says why.
    #[test]
    fn minting_twice_answers_the_same_and_says_nothing_is_reserved() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED)]);
        let first = mint(&store, VOCABULARY, Some("Tidal power"), None).expect("a mint");
        let second = mint(&store, VOCABULARY, Some("Tidal power"), None).expect("a mint");

        assert_eq!(first, second);
        assert!(
            first.contains("nothing was written and nothing is reserved"),
            "{first}"
        );
        assert!(first.contains("`openbiz import`"), "{first}");
    }

    /// A vocabulary spread over namespaces has no convention to read, and a guess would mint
    /// official-looking IRIs belonging to nothing.
    #[test]
    fn a_vocabulary_with_no_convention_is_refused_rather_than_guessed_at() {
        let (_directory, store) = store_with(&[(
            VOCABULARY,
            r#"@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
               <https://a.example/one> a skos:Concept .
               <https://b.example/two> a skos:Concept .
               <https://c.example/three> a skos:Concept ."#,
        )]);

        let error = mint(&store, VOCABULARY, Some("Tidal power"), None).expect_err("no convention");

        assert!(
            error.to_string().contains("give one with --pattern"),
            "{error}"
        );
        assert!(error.to_string().contains("3 namespaces"), "{error}");
    }

    /// An empty vocabulary is the first-day case, and `--pattern` is the answer.
    #[test]
    fn an_empty_vocabulary_mints_under_a_given_pattern() {
        let (_directory, store) = store_with(&[(VOCABULARY, "")]);
        let report = mint(
            &store,
            VOCABULARY,
            Some("Renewable energy"),
            Some("https://example.org/energy/{slug}"),
        )
        .expect("a mint");

        assert!(
            report.contains("minted: https://example.org/energy/renewable-energy"),
            "{report}"
        );
        assert!(report.contains("given with --pattern"), "{report}");
        assert!(
            report.contains("nothing in this store uses this namespace yet"),
            "{report}"
        );
    }

    /// Minting under a pattern the vocabulary does not use is legitimate — it is how a convention
    /// changes — and it is also how a concept lands in the wrong namespace unnoticed.
    #[test]
    fn a_pattern_that_disagrees_with_the_vocabulary_is_allowed_and_said_out_loud() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED)]);
        let report = mint(
            &store,
            VOCABULARY,
            Some("Tidal power"),
            Some("https://example.org/energy/{slug}"),
        )
        .expect("a mint");

        assert!(
            report.contains("suggest https://example.org/energy/c_{n} instead"),
            "{report}"
        );
        assert!(report.contains("wrong namespace"), "{report}");
    }

    /// A pattern the store's own parser will not accept is refused before it is offered to
    /// anybody, rather than being minted and failing at import.
    #[test]
    fn a_pattern_the_store_would_reject_is_refused() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED)]);
        let error = mint(
            &store,
            VOCABULARY,
            Some("Tidal power"),
            Some("https://example.org/%zz/{slug}"),
        )
        .expect_err("a broken escape");

        assert!(
            error.to_string().contains("will not accept it as an IRI"),
            "{error}"
        );
    }

    /// A pattern with nothing to fill in would mint the same IRI every time.
    #[test]
    fn a_pattern_with_no_placeholder_is_refused() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED)]);
        let error = mint(
            &store,
            VOCABULARY,
            Some("Tidal power"),
            Some("https://example.org/energy/tidal"),
        )
        .expect_err("no placeholder");

        assert!(
            error.to_string().contains("the same IRI every time"),
            "{error}"
        );
    }

    /// An unregistered vocabulary is refused rather than answered with a first mint, which would
    /// invite somebody to import into a graph that does not exist.
    #[test]
    fn an_unregistered_vocabulary_is_refused() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED)]);
        let error = mint(&store, "https://example.org/absent", Some("Tidal"), None)
            .expect_err("no such vocabulary");

        assert!(
            error.to_string().contains("no graph is registered"),
            "{error}"
        );
    }
}
