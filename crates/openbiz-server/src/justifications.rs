//! `openbiz justifications` — what was created despite something already existing.
//!
//! # The question this command exists to answer
//!
//! `adr/0003` §3 does not ask for a justification because writing one is improving. It asks for
//! "an auditable record that makes proliferation visible to the people accountable for it", and
//! *visible* is the operative word: a record nobody can read back is a note, and a note is the
//! click-through the same paragraph rules out.
//!
//! So this is the reading half, and it is what makes the recording half more than ceremony. It
//! answers, across the whole store: which concepts were created when discovery had already found
//! something under that name, who decided that, when, and on what grounds.
//!
//! # Why it reads across every vocabulary by default
//!
//! Proliferation is not a property of one vocabulary. The failure `CLAUDE.md` §1.7 describes — "a
//! tenth overlapping vocabulary" — happens *between* vocabularies, so a report that made you name
//! one at a time would hide exactly the pattern it exists to surface. A vocabulary can still be
//! named, for the narrower question.
//!
//! # What it is honest about
//!
//! Two things, both stated in the report rather than left for the reader to work out.
//!
//! **Absence of a record is not evidence of reuse.** Nothing in this build forces a justification —
//! there is no single-step create to hang the requirement off, and `openbiz mint` will still mint
//! without `--because`. So an empty report means "nobody recorded one", which is consistent with
//! a store where everything was properly reused *and* with one where nobody used the flag. The
//! report says which of those it cannot tell apart.
//!
//! **A record whose search did not finish is weaker evidence**, and is marked, because a
//! justification produced by a pass that could not reach a source says less about what exists than
//! one produced by a pass that could.

use openbiz_store::{CandidateState, GraphId, Justification, Store, StoreError};

use crate::cli::CommandError;

/// Report every recorded justification, or only those for `graph`.
pub fn justifications(store: &Store, graph: Option<&str>) -> Result<String, CommandError> {
    // Classified before the read so that a graph that is not a vocabulary is refused as the
    // category error it is, rather than reported as a vocabulary with no justifications.
    let only = graph.map(GraphId::vocabulary).transpose()?;
    let all = store.justifications()?;
    let shown: Vec<&Justification> = all
        .iter()
        .filter(|record| only.as_ref().is_none_or(|id| record.graph() == id))
        .collect();
    let fates: Vec<Fate> = shown
        .iter()
        .map(|record| fate(store, record))
        .collect::<Result<_, _>>()?;

    Ok(report(only.as_ref(), &shown, &fates, all.len()))
}

/// What became of the creation a record justifies.
///
/// The distinction the whole thing turns on: a record is evidence that somebody looked before
/// naming something, and that is *not* the same as evidence that the thing exists. Reporting the
/// two as one would count a rejected split as proliferation and would let a mint — which stages
/// nothing at all — read as a creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    /// No candidate: nothing was ever proposed, so nothing here says a concept was created.
    NothingProposed,
    /// Proposed as a candidate, in the state that candidate is now in.
    Proposed(CandidateState),
}

/// Read the fate of one record's creation, by looking up the candidate it named.
///
/// One candidate read per record, which is the cost of answering the question at all; it is
/// unmeasured at scale and recorded in `docs/UNTESTED.md` with the rest of that family.
fn fate(store: &Store, record: &Justification) -> Result<Fate, CommandError> {
    let Some(id) = record.arising_from() else {
        return Ok(Fate::NothingProposed);
    };
    match store.candidate(id) {
        Ok(candidate) => Ok(Fate::Proposed(candidate.state())),
        // A record naming a candidate the store no longer holds is a hole in the trail, not a
        // record to report without one. It stops the report rather than being glossed, because a
        // fate quietly omitted reads as a fate of "created".
        Err(error) => Err(CommandError::Store(match error {
            StoreError::NoSuchCandidate { .. } => StoreError::Corrupt {
                path: store.path().to_path_buf(),
                detail: format!(
                    "justification {} says it arose from candidate {id}, and this store does not \
                     hold that candidate; whether the concept it justifies was ever created \
                     cannot be answered",
                    record.id()
                ),
            },
            other => other,
        })),
    }
}

/// The report itself.
fn report(only: Option<&GraphId>, shown: &[&Justification], fates: &[Fate], held: usize) -> String {
    let mut out = match only {
        Some(graph) => format!("recorded justifications for creating new concepts in {graph}\n"),
        None => "recorded justifications for creating new concepts, across every vocabulary\n"
            .to_owned(),
    };

    if shown.is_empty() {
        out.push_str(&empty(only, held));
        return out;
    }

    let despite = shown
        .iter()
        .filter(|record| !record.considered().is_empty())
        .count();
    // "passed over something" rather than "created something": since each record says what became
    // of its candidate, a headline that called every record a creation would contradict the entries
    // under it, where a refused change is reported as a concept that was never created.
    out.push_str(&format!(
        "\n{} record(s), of which {despite} passed over something that already existed\n",
        shown.len()
    ));

    for (record, fate) in shown.iter().zip(fates) {
        out.push_str(&entry(record, *fate, only.is_none()));
    }

    let partial = shown
        .iter()
        .filter(|record| !record.search_was_complete())
        .count();
    if partial > 0 {
        out.push_str(&format!(
            "\n{partial} of these rest on a search that did not finish, and are marked; a source \
             that could not be reached says nothing about what exists behind it\n"
        ));
    }

    // Said as a count as well as per record, because the headline above counts records and not
    // creations, and a reader who stopped at the headline would take a refused change for a
    // vocabulary that grew.
    let refused = fates
        .iter()
        .filter(|fate| matches!(fate, Fate::Proposed(CandidateState::Rejected)))
        .count();
    if refused > 0 {
        out.push_str(&format!(
            "\n{refused} of these justify a creation that was then refused, so the concept named \
             was never created; the record stands because it is a statement somebody made at a \
             time\n"
        ));
    }

    out.push_str(NOT_ENFORCED);
    out
}

/// One record, laid out so the thing being justified is read before the justification.
fn entry(record: &Justification, fate: Fate, name_the_graph: bool) -> String {
    let mut out = format!("\n  {} — {}\n", record.id(), record.concept());
    out.push_str(&format!("    created under {:?}\n", record.label()));
    if name_the_graph {
        out.push_str(&format!("    in {}\n", record.graph()));
    }
    match record.considered().len() {
        0 => out.push_str("    nothing existing was found under that label\n"),
        count => {
            out.push_str(&format!("    {count} existing resource(s) passed over:\n"));
            for resource in record.considered() {
                out.push_str(&format!("      {resource}\n"));
            }
        }
    }
    out.push_str(&format!("    because {:?}\n", record.reason()));
    out.push_str(&format!(
        "    recorded by {} at {}\n",
        record.recorded_by(),
        record.recorded_at()
    ));
    out.push_str(&became(record, fate));
    if !record.search_was_complete() {
        out.push_str("    the search behind this record did not finish\n");
    }
    out
}

/// Whether the concept this record justifies was ever actually created.
///
/// Four answers and none of them is "created", because nothing in this build can say that: what it
/// can say is what was proposed and what a reviewer did with it. A candidate that was applied put
/// the statements in the vocabulary; anything since is a later change this record knows nothing
/// about.
fn became(record: &Justification, fate: Fate) -> String {
    match fate {
        Fate::NothingProposed => format!(
            "    nothing was proposed: {} was minted and staged nowhere, so this record says \
             somebody looked, not that the concept exists\n",
            record.concept()
        ),
        Fate::Proposed(CandidateState::Proposed) => format!(
            "    proposed as candidate {}, which nobody has decided yet\n",
            display(record)
        ),
        Fate::Proposed(CandidateState::Applied) => format!(
            "    proposed as candidate {}, which was approved\n",
            display(record)
        ),
        Fate::Proposed(CandidateState::Rejected) => format!(
            "    proposed as candidate {}, which was refused — the concept was never created\n",
            display(record)
        ),
        // `CandidateState` is `#[non_exhaustive]`: a state a later build adds is named rather than
        // folded into one of the others, because a fate reported wrongly is worse than one
        // reported as unknown.
        Fate::Proposed(other) => format!(
            "    proposed as candidate {}, which is in a state this build does not know ({other})\n",
            display(record)
        ),
    }
}

/// The candidate a record names, for a line that has already established there is one.
fn display(record: &Justification) -> String {
    match record.arising_from() {
        Some(id) => id.to_string(),
        None => "(none)".to_owned(),
    }
}

/// Nothing to show, and what that does and does not mean.
///
/// The distinction matters more here than anywhere else in the report: an empty governance report
/// reads as a clean bill of health, and this one is not one.
fn empty(only: Option<&GraphId>, held: usize) -> String {
    let mut out = match (only, held) {
        (Some(_), 0) => "\nnothing is recorded, here or anywhere in this store\n".to_owned(),
        (Some(_), held) => format!(
            "\nnothing is recorded for this vocabulary; the store holds {held} record(s) for \
             others\n"
        ),
        (None, _) => "\nnothing is recorded\n".to_owned(),
    };
    out.push_str(
        "  which is not the same as \"nothing was created despite a match\": a justification is \
         recorded when somebody asks for one, so an empty report and a store where nobody used \
         --because look identical from here\n",
    );
    out.push_str(NOT_ENFORCED);
    out
}

/// Said in both the full and the empty report, because it is the limit of what either proves.
///
/// Stating it is not modesty. A governance report read as stronger than it is does more damage
/// than one nobody reads, and `CLAUDE.md` §4 puts the rule plainly: partial support is normal,
/// misreporting it is not.
const NOT_ENFORCED: &str = concat!(
    "\nnothing in this build refuses a creation that has no justification: there is no ",
    "single-step create to attach that refusal to, so these records are what people chose to ",
    "write down rather than everything that happened.\n",
);
