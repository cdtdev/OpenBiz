//! `openbiz mint` — the IRI a new concept would be given, and everything behind that choice.
//!
//! # Why minting is a command of its own, and why it writes nothing
//!
//! There is no "create concept" command in this build, on purpose: what exists is the candidate
//! seam — a change is proposed in a file, staged, read, and approved — and to write that file
//! somebody has to decide what the new concept's IRI will be. Without this command they decide it
//! by copying an existing IRI and editing the end of it, which is how a vocabulary ends up with
//! `c_00123` beside `c_124` and two concepts sharing one IRI.
//!
//! Which makes this the creation path, and `CLAUDE.md` §1.7 puts discovery *before* creation. So
//! this command runs a discovery pass over every source the deployment has and prints what it
//! found **above** the IRI. The IRI is still offered — two concepts can legitimately share a
//! label, and a tool that refuses is a tool people work around — but nobody mints one here
//! without first being shown what is already there.
//!
//! So this command answers exactly that question and does nothing else. It stages nothing and
//! **it reserves nothing**. Run it twice and it returns the same IRI both times, and the report
//! says so in as many words, because a minter that looks like an allocator is worse than no
//! minter at all. The IRI becomes taken when a candidate carrying it is staged — and the next
//! mint sees it, because the scan reads staged changes as well as vocabularies.
//!
//! # The one thing it writes, and only when asked
//!
//! `--because` records the justification `adr/0003` §3 requires: an auditable record naming what
//! discovery found and why none of it fitted, keyed to the IRI about to be created. It goes to
//! OpenBiz's own system graph, not to the vocabulary, so none of the three sentences above stops
//! being true — nothing is staged, nothing is reserved, and the IRI does not move.
//!
//! It is captured *here*, at the moment the ladder is printed, rather than in a command of its
//! own, because `adr/0003` §4 is a requirement with teeth: reuse must be less work than
//! recreating. A justification that costs a second command is one that gets skipped, and the ADR
//! is explicit that a mechanism people route around has failed rather than been ignored.
//!
//! **`--because` without a label is refused.** §3 asks for a reason *naming what was found*, and
//! with no label there was no discovery pass, so there is nothing found to name. Recording one
//! anyway would file the appearance of diligence as evidence of it.
//!
//! # Which pattern is used, and why the order matters
//!
//! Three answers, and the first one that exists wins:
//!
//! 1. **`--pattern`**, for this one invocation.
//! 2. **The pattern recorded for the vocabulary** (`openbiz policy`). This is what a deployment
//!    should be on: an import, a discovery match, and an agent proposal all read the same recorded
//!    policy, so they mint the same way as the curator does.
//! 3. **The convention read off the vocabulary's own concepts**, when nothing is recorded.
//!
//! The third is a good suggestion and a poor policy, and the report says so where it is used.
//! Inference answers "what do most of these concepts look like *now*", so it moves when the
//! vocabulary does: a vocabulary whose first ten concepts are in one namespace and whose next ten
//! arrive in another silently changes its own convention part-way through, and by the time anybody
//! notices the IRIs are permanent. A recorded policy is the answer to that, and it is why `openbiz
//! policy` exists.
//!
//! A recorded pattern this build cannot parse is **refused**, not quietly replaced by inference:
//! falling back would mint into a namespace nobody chose while the vocabulary has a written
//! decision saying otherwise.
//!
//! # What is read
//!
//! Three things, and the report names all three:
//!
//! 1. **The vocabulary's own convention.** The namespace most of its concepts are already in, and
//!    whether their local names are numbered or worded, is the evidence for the default pattern.
//!    A vocabulary that has no majority namespace gets no suggestion rather than a confident
//!    wrong one, and a pattern — recorded or given — is then required.
//! 2. **Every IRI under that pattern's prefix, anywhere in the store.** Not just the target
//!    vocabulary: an IRI is a global identifier, and a deployment where two vocabularies extend
//!    the same namespace is the ordinary case in an enterprise, not an exotic one. Only IRIs
//!    under the prefix are kept, so the memory this costs is the size of the namespace and not
//!    the size of the store.
//! 3. **Everything discovery can reach that is already called this.** Not one vocabulary: every
//!    vocabulary in the store and every change staged against one, matched anywhere inside a
//!    label of any kind in any language, through `openbiz-discovery`'s own trait. What answered,
//!    what it read, and what was never asked are all printed, because a quiet "nothing found"
//!    read as "nothing exists" is what creates the tenth overlapping vocabulary. Sources beyond
//!    the local store — peers, catalogs, public registries — are Phase 12, and the report says
//!    they were not consulted rather than leaving the reader to assume they were.

use std::collections::BTreeSet;

use openbiz_discovery::{Discovered, Discovery, LocalVocabularies, Match, Outcome};
use openbiz_skos::{
    mint as mint_iri, CoreModel, IriConvention, LabelQuery, MintDerivation, MintPattern, MintScan,
    Minted, SkosClass, SlugBound, Suggestion,
};
use openbiz_store::{CandidateState, GraphId, GraphKind, IriPolicy, Store};

use crate::cli::actor;

use crate::cli::CommandError;
use crate::discovery::StoreCorpus;
use crate::inspect::convert;

/// Report the IRI a new concept in `graph` would be minted with, and record why, if asked.
///
/// Without `because` this reads and writes nothing. With it, one justification record is written
/// to the system graph after the IRI is computed — see the module documentation for why it is
/// captured here and why it is refused when no label was given.
pub fn mint(
    store: &Store,
    graph: &str,
    label: Option<&str>,
    pattern: Option<&str>,
    because: Option<&str>,
) -> Result<String, CommandError> {
    // Refused before anything is read, so the operator is told what is wrong with the command
    // rather than handed a report with a note buried in it.
    if because.is_some() && label.map(str::trim).unwrap_or_default().is_empty() {
        return Err(CommandError::JustifyingWithoutLooking);
    }

    let mut builder = CoreModel::builder();
    store.for_each_statement(graph, |statement| builder.push(convert(statement)))?;
    let model = builder.build();

    // §1.7 before anything else: what already exists, asked across the whole store rather than
    // the one vocabulary. Discovery cannot fail the command — a source that will not answer is
    // reported as unavailable and the mint goes ahead — so there is no `?` here and there must
    // never be one.
    let corpus = StoreCorpus::authoring(store, graph);
    let local = LocalVocabularies::named("this store", &corpus);
    let found = label
        .map(LabelQuery::new)
        .transpose()
        .ok()
        .flatten()
        .map(|query| Discovery::new().across(&[&local], &query));

    let convention = convention_of(&model);

    // What this vocabulary has *decided*, which outranks what its concepts happen to look like.
    let recorded = store.iri_policy(&GraphId::vocabulary(graph)?)?;

    // Whichever pattern wins, the suggestion is still computed, so the report can say what the
    // vocabulary's own concepts would have chosen — a pattern that disagrees with them is exactly
    // the thing worth showing somebody before they use it.
    let suggested = convention.suggest();
    let (chosen, source) = pattern_for(graph, &suggested, &recorded, pattern)?;

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

    // After the IRI, because a justification names the IRI it justifies: there is nothing to
    // attach one to until minting has actually produced something.
    let recorded = match (because, label, &minted, &found) {
        (Some(reason), Some(label), Ok(minted), Some(found)) => Some(record_justification(
            store,
            graph,
            &minted.iri,
            label,
            reason,
            found,
        )?),
        _ => None,
    };

    Ok(report(
        graph, label, &chosen, &source, &suggested, &scan, &minted, &found, because, &recorded,
    ))
}

/// Write down what was found and why the operator is creating anyway.
///
/// Everything discovery reached is recorded, exact matches and containing ones alike: the question
/// an auditor asks is "what did this person see and pass over", and a near match a curator looked
/// at and rejected is exactly as much a part of that as an identical label.
///
/// The evidence is marked incomplete when any source could not be reached or the match list stopped
/// at its bound, because a record that does not say so invites a reader to take a search that was
/// cut short for one that finished.
fn record_justification(
    store: &Store,
    graph: &str,
    concept: &str,
    label: &str,
    reason: &str,
    found: &Discovered,
) -> Result<openbiz_store::Justification, CommandError> {
    let mut considered: BTreeSet<String> = BTreeSet::new();
    for hit in found.matches() {
        // A blank node cannot be named in a record an auditor will look things up in. It is
        // counted in the report rather than dropped silently — see `unnameable`.
        if let Some(iri) = hit.resource.as_iri() {
            considered.insert(iri.to_owned());
        }
    }
    let complete = found.is_complete() && found.unavailable().next().is_none();

    Ok(store.record_justification(
        &GraphId::vocabulary(graph)?,
        concept,
        label,
        &considered.into_iter().collect::<Vec<_>>(),
        reason,
        complete,
        &actor()?,
    )?)
}

/// How many matches could not be named in a justification record, because they are blank nodes.
fn unnameable(found: &Discovered) -> usize {
    found
        .matches()
        .iter()
        .filter(|hit| hit.resource.as_iri().is_none())
        .count()
}

/// Which pattern a new IRI in `graph` is minted under, and where that pattern came from.
///
/// Three answers, and the first one that exists wins: `--pattern`, then the vocabulary's recorded
/// policy, then the convention read off its own concepts. Shared with `openbiz split`, which mints
/// under the same three and must not resolve them a second, subtly different way — the whole point
/// of `openbiz policy` is that every producer mints the same.
pub(crate) fn pattern_for<'a>(
    graph: &str,
    suggested: &Result<Suggestion, openbiz_skos::NoConvention>,
    recorded: &'a Option<IriPolicy>,
    given: Option<&str>,
) -> Result<(MintPattern, PatternSource<'a>), CommandError> {
    Ok(match given {
        Some(text) => (
            MintPattern::parse(text)?,
            PatternSource::Given {
                recorded: recorded.as_ref(),
            },
        ),
        None => match recorded {
            // Refused rather than fallen back from. A vocabulary with a recorded policy has made a
            // decision, and minting under a different pattern because we could not read that
            // decision is worse than not minting at all.
            Some(policy) => (
                MintPattern::parse(policy.pattern()).map_err(|source| {
                    CommandError::RecordedPatternUnusable {
                        graph: graph.to_owned(),
                        pattern: policy.pattern().to_owned(),
                        recorded_by: policy.recorded_by().to_owned(),
                        source,
                    }
                })?,
                PatternSource::Recorded(policy),
            ),
            None => match suggested {
                Ok(suggestion) => (suggestion.pattern.clone(), PatternSource::Inferred),
                Err(error) => return Err(CommandError::NoConvention(error.clone())),
            },
        },
    })
}

/// Every IRI in the store that begins with `prefix`, and where each was found.
///
/// Vocabularies first, then the staged changes, because [`MintScan`] keeps the first source to
/// mention an IRI: a collision with a vocabulary must not be reported as a collision with a
/// candidate that merely repeats what the vocabulary already says.
pub(crate) fn scan_for(
    store: &Store,
    target: &str,
    prefix: &str,
) -> Result<MintScan, CommandError> {
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

/// Where the pattern being minted under came from — the three answers, in the order they win.
///
/// Kept as a type rather than a pair of booleans because the report says something different about
/// each, and "given" and "inferred" being distinguishable by accident is how a report ends up
/// telling somebody their deployment has a policy when it has a coincidence.
pub(crate) enum PatternSource<'a> {
    /// Given with `--pattern`, for this invocation only.
    ///
    /// Carries whatever the vocabulary records, because overriding a *written decision* is a louder
    /// thing than overriding a guess and the report has to name the decision it is stepping over.
    Given {
        /// The recorded policy this command is ignoring for one mint, if there is one.
        recorded: Option<&'a IriPolicy>,
    },
    /// Recorded for the vocabulary, which is what every producer reads.
    Recorded(&'a IriPolicy),
    /// Read off the vocabulary's own concepts, because nothing is recorded.
    Inferred,
}

/// What a vocabulary's own concepts say about how it names things.
///
/// Shared with `openbiz policy`, which needs exactly the same evidence to tell an operator whether
/// the pattern they are about to record agrees with what the vocabulary already does. Two copies of
/// this would eventually disagree, and the report that said "your concepts suggest X" would stop
/// matching the one that mints.
pub(crate) fn convention_of(model: &CoreModel) -> IriConvention {
    let mut convention = IriConvention::new();
    for (node, _) in model.instances_of(SkosClass::Concept) {
        match node.as_iri() {
            Some(iri) => convention.push(iri),
            None => convention.push_blank(),
        }
    }
    convention
}

/// The trade a pattern's policy makes, in the words the operator needs before choosing it.
pub(crate) fn policy_line(pattern: &MintPattern) -> &'static str {
    match pattern.policy() {
        openbiz_skos::MintPolicy::Opaque => {
            "  an opaque IRI: the local name means nothing, so nothing about the concept can \
             make it wrong\n"
        }
        openbiz_skos::MintPolicy::Readable => {
            "  a readable IRI: the local name comes from the label, and is never revised when \
             the label changes\n"
        }
    }
}

/// Whether the pattern being compared is one chosen now or one the vocabulary already records.
///
/// The two need different sentences, and getting that wrong was found by reading the command's own
/// output: a *recorded* pattern that disagrees with the concepts is not somebody "minting under a
/// different pattern" — nobody is doing anything, the vocabulary's written policy and its existing
/// IRIs simply differ — and telling the reader they are taking a risky action they are not taking is
/// how a report stops being read.
pub(crate) enum PatternStanding {
    /// Chosen for this command: given with `--pattern`, or being recorded right now.
    Chosen,
    /// Already recorded for the vocabulary, and being reported.
    Recorded,
}

/// How a pattern stands against what the vocabulary's own concepts suggest.
///
/// Said out loud wherever a pattern is reported, because a disagreement is legitimate — it is how a
/// convention gets changed on purpose — and is also exactly how somebody mints into the wrong
/// namespace without noticing. Only the reader can tell which of the two it is, so neither command
/// refuses and both say what they saw.
pub(crate) fn compared_with_convention(
    pattern: &MintPattern,
    suggested: &Result<Suggestion, openbiz_skos::NoConvention>,
    standing: PatternStanding,
) -> String {
    match suggested {
        Ok(suggestion) if suggestion.pattern == *pattern => {
            "  which is also what this vocabulary's own concepts suggest\n".to_owned()
        }
        Ok(suggestion) => match standing {
            PatternStanding::Chosen => format!(
                "  this vocabulary's own concepts suggest {} instead; minting under a different \
                 pattern is legitimate and it is also how a concept ends up in the wrong \
                 namespace\n",
                suggestion.pattern
            ),
            PatternStanding::Recorded => format!(
                "  this vocabulary's own concepts suggest {} instead, so its recorded policy and \
                 the IRIs it already holds disagree: either the policy was recorded to change the \
                 convention, or it names a namespace nobody meant. Nothing already minted is \
                 affected either way\n",
                suggestion.pattern
            ),
        },
        Err(error) => format!("  this vocabulary suggests nothing to compare it with: {error}\n"),
    }
}

/// The report, kept apart from the store so it can be tested against parts in hand.
#[allow(clippy::too_many_arguments)]
fn report(
    graph: &str,
    label: Option<&str>,
    pattern: &MintPattern,
    source: &PatternSource<'_>,
    suggested: &Result<Suggestion, openbiz_skos::NoConvention>,
    scan: &MintScan,
    minted: &Result<Minted, openbiz_skos::MintError>,
    found: &Option<Discovered>,
    because: Option<&str>,
    recorded: &Option<openbiz_store::Justification>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("an IRI for a new concept in {graph}\n"));
    match label {
        Some(label) => out.push_str(&format!("to be called {label:?}\n")),
        None => out.push_str("with no label given\n"),
    }

    // §1.7 first, before the IRI: if the vocabulary already calls something this, the next step is
    // not to mint anything.
    out.push_str(&already_here(label, found));

    out.push_str(&format!("\npattern: {pattern}\n"));
    out.push_str(policy_line(pattern));
    out.push_str(&source_of_pattern(pattern, source, suggested));

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

    out.push_str(&justification_line(because, recorded, found));

    // Last, and never omitted. A reader who takes this for an allocator will mint twice and
    // create two concepts on one IRI, which is the exact failure the command exists to prevent.
    //
    // Two wordings, because one of them would have to be the weaker of the two truths. Without
    // `--because` this command writes nothing whatsoever, and saying only "nothing is staged"
    // would understate that; with it, one record was written, and saying "nothing was written"
    // would be false. A closing claim a reader is meant to rely on has to be the true one.
    out.push_str(match recorded {
        None => concat!(
            "\nnothing was written and nothing is reserved: run this again and it answers the ",
            "same. The IRI becomes taken when a change carrying it is staged — `openbiz import` ",
            "— and the next mint sees it there.\n",
        ),
        Some(_) => concat!(
            "\nnothing is staged and nothing is reserved: the justification above is the only ",
            "thing written, and it is in OpenBiz's own graph rather than in the vocabulary. Run ",
            "this again and it answers the same IRI. The IRI becomes taken when a change ",
            "carrying it is staged — `openbiz import` — and the next mint sees it there.\n",
        ),
    });
    out
}

/// What was recorded about creating rather than reusing, or that nothing was.
///
/// Three cases, and the middle one is the one that must never be silent: `--because` was given and
/// no record could be written, because nothing was minted for it to be about. Saying nothing there
/// would leave an operator believing they had filed a justification they had not.
fn justification_line(
    because: Option<&str>,
    recorded: &Option<openbiz_store::Justification>,
    found: &Option<Discovered>,
) -> String {
    match (because, recorded) {
        (Some(_), Some(record)) => {
            let mut out = format!(
                concat!(
                    "\njustification {} recorded, in OpenBiz's own graph and not in the ",
                    "vocabulary:\n  {:?}\n  recorded by {} at {}\n",
                ),
                record.id(),
                record.reason(),
                record.recorded_by(),
                record.recorded_at()
            );
            match record.considered().len() {
                0 => out.push_str(concat!(
                    "  naming nothing passed over, because discovery found nothing under this ",
                    "label\n",
                )),
                count => {
                    out.push_str(&format!(
                        concat!(
                            "  naming {} existing resource(s) passed over, each queryable as ",
                            "<urn:openbiz:justificationConsidered>\n",
                        ),
                        count
                    ));
                    for resource in record.considered() {
                        out.push_str(&format!("    {resource}\n"));
                    }
                }
            }
            if !record.search_was_complete() {
                out.push_str(concat!(
                    "  the search behind it did not finish — a source was unreachable or the ",
                    "match list stopped at its bound — and the record says so\n",
                ));
            }
            let blank = found.as_ref().map(unnameable).unwrap_or_default();
            if blank > 0 {
                out.push_str(&format!(
                    concat!(
                        "  {} match(es) are not named in the record: they are blank nodes, which ",
                        "nothing can look up later\n",
                    ),
                    blank
                ));
            }
            out
        }
        (Some(_), None) => {
            "\nno justification was recorded: nothing was minted for one to be about\n".to_owned()
        }
        (None, _) => String::new(),
    }
}

/// Where the pattern came from: the operator, the vocabulary's recorded policy, or its concepts.
fn source_of_pattern(
    pattern: &MintPattern,
    source: &PatternSource<'_>,
    suggested: &Result<Suggestion, openbiz_skos::NoConvention>,
) -> String {
    let mut out = String::new();
    match source {
        PatternSource::Given { recorded } => {
            out.push_str("  given with --pattern, for this one command\n");
            match recorded {
                Some(policy) if policy.pattern() == pattern.to_string() => out.push_str(&format!(
                    "  which is also what this vocabulary records, set by {} at {}\n",
                    policy.recorded_by(),
                    policy.recorded_at()
                )),
                // The loud case. A recorded policy is what every other producer will mint under, so
                // a command that steps over it has to say whose decision it stepped over.
                Some(policy) => out.push_str(&format!(
                    "  this vocabulary records {:?} instead, set by {} at {}; that record is \
                     unchanged and every other producer still mints under it. Use `openbiz policy` \
                     if the recorded pattern is the one that should change\n",
                    policy.pattern(),
                    policy.recorded_by(),
                    policy.recorded_at()
                )),
                None => out.push_str(
                    "  nothing is recorded for this vocabulary, so this pattern applies to this \
                     command and to nothing else\n",
                ),
            }
            out.push_str(&compared_with_convention(
                pattern,
                suggested,
                PatternStanding::Chosen,
            ));
        }
        PatternSource::Recorded(policy) => {
            out.push_str(&format!(
                "  recorded for this vocabulary by {} at {}, so every producer mints under it — \
                 an import, a match against another vocabulary, and a curator at this command \
                 line all get the same answer\n",
                policy.recorded_by(),
                policy.recorded_at()
            ));
            out.push_str(&compared_with_convention(
                pattern,
                suggested,
                PatternStanding::Recorded,
            ));
            out.push_str(
                "  give --pattern to mint once under a different one, or `openbiz policy` to \
                 change what is recorded\n",
            );
        }
        PatternSource::Inferred => {
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
            }
            // The point of the item this paragraph exists for. An inferred pattern is a reading of
            // the vocabulary as it stands, so it moves when the vocabulary does, and nothing says
            // so at the moment the IRI it produced becomes permanent.
            out.push_str(
                "  nothing is recorded for this vocabulary, so this was inferred from its own \
                 concepts and is a reading of them as they stand now: it changes when they do, \
                 and two producers reading it at different times can disagree\n",
            );
            out.push_str(
                "  record it with `openbiz policy` to fix it for every producer, or give \
                 --pattern to override it once\n",
            );
        }
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

/// What already exists that this label might already name — the §1.7 pass, run before anything
/// is minted and printed before the IRI.
///
/// This is the whole difference between a minter and a silo generator. The question is not "does
/// this vocabulary already use this string" but "does the organisation already have this concept",
/// and the second one is answered by asking every source discovery has — today the local store's
/// vocabularies and the changes staged against them, tomorrow a peer, a catalog, a registry
/// (`adr/0003` §2, Phase 12).
///
/// Three things this section must never do, each of which is how a duplicate gets created:
/// report a bounded list as a complete one; report an unavailable source as an absent match; or
/// print a bare "nothing found" without saying how far the looking went.
fn already_here(label: Option<&str>, found: &Option<Discovered>) -> String {
    let Some(label) = label else {
        return "\nno label was given, so nothing was checked for an existing concept of the same \
                name; give one and this looks across every vocabulary in the store first\n"
            .to_owned();
    };
    // The only way a query fails to build is an empty one, which matches every label there is and
    // would report the whole store as a duplicate of nothing.
    let Some(found) = found else {
        return "\nthe label given is empty, so nothing was looked for: an empty query matches \
                every label there is\n"
            .to_owned();
    };

    let mut out = String::new();
    let exact = found.exact().count();
    let related = found.related().count();

    if exact > 0 {
        // Concepts, not labels. Two of a concept's labels reading the same string is one concept
        // to reuse, and a count of labels would report it as two — an over-count in the one
        // sentence whose whole job is to be believed.
        let concepts: BTreeSet<_> = found.exact().map(|hit| &hit.resource).collect();
        out.push_str(&format!(
            "\nSTOP — {label:?} is already a label on {} concept(s) discovery reached:\n",
            concepts.len()
        ));
        for hit in found.exact() {
            out.push_str(&line(hit));
        }
    } else {
        out.push_str(&format!(
            "\nnothing discovery reached is called {label:?}\n"
        ));
    }

    if related > 0 {
        out.push_str(&format!(
            "\n{} {related} label(s) contain it, which may be the concept meant under another \
             name:\n",
            match exact {
                0 => "but",
                _ => "and",
            }
        ));
        for hit in found.related() {
            out.push_str(&line(hit));
        }
    }

    if !found.is_complete() {
        out.push_str(&format!(
            "  {} more match(es) are not listed: {} matched and this report stops at {}\n",
            found.withheld(),
            found.matched(),
            found.bound().max_matches
        ));
    }

    if exact > 0 || related > 0 {
        out.push_str(LADDER);
        out.push_str(STILL_MINTED);
    }

    out.push_str(&consulted(found));
    out
}

/// The reuse ladder, `adr/0003` §3, in the words of what this build can actually do about it.
///
/// Printed only when something was found, because a ladder offered over an empty list is noise
/// that teaches the reader to skip the paragraph on the day it matters.
///
/// **Shared with `openbiz split`**, the other creation path. Two copies would drift, and a ladder
/// whose rungs depend on which command you happened to use is one nobody can be held to. Each
/// command adds its own closing sentence, because what happens next differs: a mint offers an IRI
/// and writes nothing, a split stages a change somebody can still reject.
pub(crate) const LADDER: &str = concat!(
    "\nreuse outranks creation (CLAUDE.md §1.7, adr/0003 §3), in this order: use one of these ",
    "concepts as it stands; map to it with skos:exactMatch or skos:closeMatch; extend it with a ",
    "narrower concept of your own; and only then create a new one. ",
);

/// The one thing a mint can say about the ladder's last rung.
const STILL_MINTED: &str = concat!(
    "An IRI is still minted below, because two concepts can legitimately share a label — but if ",
    "one of these is the concept you mean, minting a second one is how a vocabulary becomes a ",
    "silo.\n",
    "if you create one anyway, §3 asks for a recorded reason naming what was found and why none ",
    "of it fitted: give --because \"…\" and this command files one, which `openbiz ",
    "justifications` and any SPARQL query over the system graph can then be asked about.\n",
);

/// One match: what it is, what it is called, how it matched, and where it lives.
///
/// Shared with `openbiz split` for the same reason as [`LADDER`]: a match a curator reads on one
/// creation path must not be laid out differently on the other.
pub(crate) fn line(hit: &Match) -> String {
    format!(
        "  {}{}  {}{}, in {}\n",
        hit.resource,
        match &hit.display {
            Some(display) => format!("  ({display})"),
            None => String::new(),
        },
        match hit.kind {
            Some(kind) => kind.to_string(),
            None => "labelled".to_owned(),
        },
        match &hit.label.language {
            Some(tag) => format!(" {:?}@{tag}", hit.label.text),
            None => format!(" {:?}, untagged", hit.label.text),
        },
        hit.within
    )
}

/// Which sources answered, what each looked at, and which could not be reached.
///
/// Never omitted, and never shortened when everything went well. A reader has to be able to tell
/// "this term is not in the organisation" from "one store was read and nothing else was asked",
/// and the second is what this build actually does.
fn consulted(found: &Discovered) -> String {
    consulted_entries(found.consulted())
}

/// The same, over a consultation record merged across several labels — `openbiz split` asks about
/// one name per part and reports the sources once, for the command.
pub(crate) fn consulted_entries(entries: &[openbiz_discovery::Consulted]) -> String {
    let mut out = format!("\ndiscovery consulted {} source(s):\n", entries.len());
    for entry in entries {
        match &entry.outcome {
            Outcome::Answered {
                matched,
                searched,
                labels_read,
            } => out.push_str(&format!(
                "  {} — {searched}, {labels_read} label(s) read, {matched} match(es)\n",
                entry.source
            )),
            // The case the whole design turns on. An unavailable source is not an absent match,
            // and a report that let the two look alike would have a broken connector quietly
            // telling somebody to create a concept the organisation already has.
            Outcome::Unavailable { reason } => out.push_str(&format!(
                "  {} — UNAVAILABLE: {reason}. Nothing above says this term is not there; it says \
                 it was not looked for\n",
                entry.source
            )),
        }
    }
    out.push_str(
        "matched over every label of every kind, in any language, anywhere inside the label, \
         ignoring case but not accents, spelling, or Unicode normalisation\n",
    );
    out.push_str(
        "no peer, no data catalog, and no public registry was consulted: this build has no \
         connector for one (adr/0003 §2, Phase 12), so a concept that exists only outside this \
         store has not been seen\n",
    );
    out
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
        let report = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");

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
        let report = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");

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
        let report = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");

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
        let report = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");

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

        let report = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");

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

        let report = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");

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
        let report = mint(&store, VOCABULARY, Some("Solar power"), None, None).expect("a mint");

        let stop = report.find("STOP").expect("the §1.7 warning");
        let minted = report.find("minted:").expect("an IRI");
        assert!(stop < minted, "the warning comes first: {report}");
        assert!(
            report.contains("https://example.org/energy/c_3"),
            "the concept that already holds the label is named: {report}"
        );
        assert!(report.contains("reuse outranks creation"), "{report}");
    }

    /// A clean answer has to say how far it looked. A quiet "nothing found" that reads as
    /// "nothing exists" is the report that creates duplicates, and the sentence that stops it
    /// being read that way is the one naming the sources nobody asked.
    #[test]
    fn a_clean_discovery_pass_says_what_it_consulted_and_what_it_did_not() {
        let (_directory, store) = store_with(&[(VOCABULARY, NUMBERED), (OTHER, "")]);
        let report = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");

        assert!(
            report.contains("nothing discovery reached is called \"Tidal power\""),
            "{report}"
        );
        assert!(
            report.contains("this store — 2 vocabularies"),
            "the answer says what was read, not just that it read: {report}"
        );
        assert!(
            report.contains("no peer, no data catalog, and no public registry was consulted"),
            "what was *not* asked is the sentence that stops this reading as \"nothing \
             exists\": {report}"
        );
        assert!(
            !report.contains("reuse outranks creation"),
            "a ladder over an empty list teaches the reader to skip the paragraph: {report}"
        );
    }

    /// **The item, in one report.** A concept in a *different* vocabulary in the same store is
    /// found — the match `openbiz mint` could not make when it looked in one vocabulary — and it
    /// is found before the IRI, because the next step is to reuse it, not to mint.
    #[test]
    fn a_concept_in_another_vocabulary_is_discovered_before_the_iri() {
        let (_directory, store) = store_with(&[
            (VOCABULARY, NUMBERED),
            (
                OTHER,
                r#"@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
                   <https://example.org/materials/c_7> a skos:Concept ;
                     skos:prefLabel "Tidal power"@en ."#,
            ),
        ]);

        let report = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");

        let stop = report.find("STOP").expect("the §1.7 warning");
        let minted = report.find("minted:").expect("an IRI");
        assert!(stop < minted, "the warning comes first: {report}");
        assert!(
            report.contains("https://example.org/materials/c_7"),
            "the concept already holding the term is named: {report}"
        );
        assert!(
            report.contains("in the vocabulary https://example.org/materials"),
            "and so is the vocabulary it is in, which is the one the curator is not looking at: \
             {report}"
        );
        assert!(report.contains("reuse outranks creation"), "{report}");
        assert!(
            report.contains("skos:exactMatch"),
            "the rung above creating is named, not just the warning: {report}"
        );
    }

    /// A term that contains the query is not the same term, and the report must not say STOP
    /// about it — but it must still be shown, because "Tidal power generation" existing is the
    /// reason not to mint "Tidal power" without thinking.
    #[test]
    fn a_partial_match_is_shown_as_related_and_does_not_stop_the_report() {
        let (_directory, store) = store_with(&[(
            VOCABULARY,
            r#"@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
               <https://example.org/energy/c_1> a skos:Concept ;
                 skos:prefLabel "Tidal power generation"@en ."#,
        )]);

        let report = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");

        assert!(
            !report.contains("STOP"),
            "it is not the same term: {report}"
        );
        assert!(
            report.contains("label(s) contain it"),
            "and it is still shown: {report}"
        );
        assert!(
            report.contains("https://example.org/energy/c_1"),
            "{report}"
        );
    }

    /// A hit on an alternative label is a hit — SKOS §5.1 defines the other two label properties
    /// for exactly this — and the concept is shown under the label it is *displayed* by, which
    /// §5.1 says is never a hidden one.
    #[test]
    fn a_match_on_an_alternative_label_names_the_concepts_preferred_one() {
        let (_directory, store) = store_with(&[(
            VOCABULARY,
            r#"@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
               <https://example.org/energy/c_1> a skos:Concept ;
                 skos:prefLabel "Tidal stream generation"@en ;
                 skos:altLabel "Tidal power"@en ."#,
        )]);

        let report = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");

        assert!(report.contains("STOP"), "{report}");
        assert!(
            report.contains("(Tidal stream generation)"),
            "shown under its preferred label: {report}"
        );
        assert!(
            report.contains("skos:altLabel"),
            "and honest about which label matched: {report}"
        );
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

        let report = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");

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
        let report = mint(&store, VOCABULARY, None, None, None).expect("a mint");

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
        let report = mint(&store, VOCABULARY, Some("Solar power"), None, None).expect("a report");

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
        let first = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");
        let second = mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect("a mint");

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

        let error =
            mint(&store, VOCABULARY, Some("Tidal power"), None, None).expect_err("no convention");

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
            None,
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
            None,
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
            None,
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
            None,
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
        let error = mint(
            &store,
            "https://example.org/absent",
            Some("Tidal"),
            None,
            None,
        )
        .expect_err("no such vocabulary");

        assert!(
            error.to_string().contains("no graph is registered"),
            "{error}"
        );
    }
}
