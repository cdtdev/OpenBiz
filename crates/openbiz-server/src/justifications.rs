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

use openbiz_store::{GraphId, Justification, Store};

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

    Ok(report(only.as_ref(), &shown, all.len()))
}

/// The report itself.
fn report(only: Option<&GraphId>, shown: &[&Justification], held: usize) -> String {
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
    out.push_str(&format!(
        "\n{} record(s), of which {despite} created something despite an existing match\n",
        shown.len()
    ));

    for record in shown {
        out.push_str(&entry(record, only.is_none()));
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

    out.push_str(NOT_ENFORCED);
    out
}

/// One record, laid out so the thing being justified is read before the justification.
fn entry(record: &Justification, name_the_graph: bool) -> String {
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
    if !record.search_was_complete() {
        out.push_str("    the search behind this record did not finish\n");
    }
    out
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
