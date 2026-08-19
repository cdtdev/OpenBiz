//! `openbiz policy` — the pattern a vocabulary's new IRIs are minted under, written down.
//!
//! # Why this command exists
//!
//! `openbiz mint` can read a pattern off a vocabulary's own concepts, and that is a genuinely good
//! *suggestion*: it is evidence rather than a preference, and it is more than every incumbent
//! offers, all of which make you configure a URI pattern against nothing. It is a poor *policy*,
//! for one reason. Inference answers "what do most of these concepts look like now", so its answer
//! moves as the vocabulary grows. A vocabulary whose first two hundred concepts are in one
//! namespace and whose next hundred arrive in another will change its own convention part-way
//! through, and each IRI it produced was permanent the moment it was used.
//!
//! A recorded policy is one decision, in one place, that every producer reads: the curator here, an
//! import, a discovery match, and — when Phase 10 arrives — an agent proposal. That is the whole
//! point. A pattern that lives in one operator's shell history is not a policy.
//!
//! # What it writes, and what it deliberately does not
//!
//! It writes three statements to OpenBiz's own system graph: the pattern, who recorded it, and
//! when. It writes **nothing to the vocabulary** — the policy is a fact about a vocabulary rather
//! than one in it, so it does not travel to another tool as a statement no standard defines — and
//! it changes **nothing already minted**. A policy governs the next mint. The IRIs a vocabulary
//! already holds are the record of what it did before, and rewriting them is not something a policy
//! change may quietly do, because an IRI that changes denotes a different concept.
//!
//! Because it writes to the system graph rather than to a vocabulary, it does not go through the
//! candidate seam (`CLAUDE.md` §3, which is about proposed changes to a vocabulary). Recording it
//! is still attributed, by the same rule an approval is: the pattern is a governance decision and
//! an unattributed one is not one.
//!
//! # Why it says whether the vocabulary agrees
//!
//! Recording a pattern the vocabulary's own concepts contradict is legitimate — it is exactly how a
//! convention is changed on purpose, and refusing it would make this command useless for the case
//! it is most needed in. It is also how somebody starts minting into the wrong namespace and does
//! not find out for a year. Only the operator can tell those apart, so both reports say what the
//! concepts suggest and whether it matches, and neither refuses.

use openbiz_skos::{CoreModel, MintPattern};
use openbiz_store::{GraphId, IriPolicy, Store};

use crate::cli::{actor, CommandError};
use crate::inspect::convert;
use crate::mint::{compared_with_convention, convention_of, policy_line, PatternStanding};

/// Show the recorded IRI-minting policy for `graph`, or record `pattern` as it.
///
/// With no pattern this reads and writes nothing. With one it validates the pattern *here* — the
/// store deliberately does not know what a pattern means (see `openbiz_store::policy`) — and
/// records it only if this build could actually mint under it. A store holding a pattern nothing
/// can parse is a vocabulary that cannot mint at all, which is a worse outcome than a rejected
/// argument.
pub fn policy(store: &Store, graph: &str, pattern: Option<&str>) -> Result<String, CommandError> {
    let target = GraphId::vocabulary(graph)?;

    // The vocabulary's own concepts, for the agreement line. Read before the write so that a
    // failure to read the vocabulary is not a policy half-recorded.
    let mut builder = CoreModel::builder();
    store.for_each_statement(graph, |statement| builder.push(convert(statement)))?;
    let suggested = convention_of(&builder.build()).suggest();

    match pattern {
        None => {
            let recorded = store.iri_policy(&target)?;
            Ok(show(graph, recorded.as_ref(), &suggested))
        }
        Some(text) => {
            let parsed = MintPattern::parse(text)?;
            let recorded = store.record_iri_policy(&target, text, &actor()?)?;
            Ok(recorded_report(
                graph,
                &parsed,
                &recorded.policy,
                recorded.replaced.as_ref(),
                &suggested,
            ))
        }
    }
}

/// What is recorded, or that nothing is and what that costs.
fn show(
    graph: &str,
    recorded: Option<&IriPolicy>,
    suggested: &Result<openbiz_skos::Suggestion, openbiz_skos::NoConvention>,
) -> String {
    let mut out = format!("the IRI-minting policy for {graph}\n");

    match recorded {
        Some(policy) => {
            // A recorded pattern this build cannot parse is shown as what it is rather than hidden.
            // `openbiz mint` refuses outright on this, and an operator sent here to find out why
            // must be able to see the recorded text and the reason together.
            match MintPattern::parse(policy.pattern()) {
                Ok(parsed) => {
                    out.push_str(&format!("\nrecorded: {parsed}\n"));
                    out.push_str(policy_line(&parsed));
                    out.push_str(&format!(
                        "  recorded by {} at {}\n",
                        policy.recorded_by(),
                        policy.recorded_at()
                    ));
                    out.push_str(&compared_with_convention(
                        &parsed,
                        suggested,
                        PatternStanding::Recorded,
                    ));
                    out.push_str(
                        "  every producer mints under this: an import, a match against another \
                         vocabulary, and a curator at this command line all get the same answer\n",
                    );
                }
                Err(error) => {
                    out.push_str(&format!(
                        "\nrecorded, and unusable: {:?}\n  this build cannot read it: {error}\n  \
                         recorded by {} at {}\n  nothing can be minted for this vocabulary until a \
                         pattern this build accepts is recorded\n",
                        policy.pattern(),
                        policy.recorded_by(),
                        policy.recorded_at()
                    ));
                }
            }
        }
        None => {
            out.push_str(
                "\nnothing is recorded, so `openbiz mint` reads a pattern off this vocabulary's \
                 own concepts each time it runs\n",
            );
            match suggested {
                Ok(suggestion) => {
                    out.push_str(&format!(
                        "  its own concepts suggest {}\n",
                        suggestion.pattern
                    ));
                    out.push_str(policy_line(&suggestion.pattern));
                }
                Err(error) => out.push_str(&format!(
                    "  and they suggest nothing: {error}. Nothing can be minted here until a \
                     pattern is recorded or given with --pattern\n"
                )),
            }
            // The reason the item exists, stated where somebody who has not recorded one will read
            // it.
            out.push_str(
                "  an inferred pattern is a reading of the concepts as they stand, so it moves as \
                 they grow: a vocabulary that acquires concepts in a second namespace can change \
                 its own convention part-way through, and the IRIs on both sides of that are \
                 already permanent\n",
            );
            out.push_str(&format!(
                "  record one with: openbiz policy {graph} --pattern <p>\n"
            ));
        }
    }

    out
}

/// What recording one did, including what it displaced.
fn recorded_report(
    graph: &str,
    parsed: &MintPattern,
    policy: &IriPolicy,
    replaced: Option<&IriPolicy>,
    suggested: &Result<openbiz_skos::Suggestion, openbiz_skos::NoConvention>,
) -> String {
    let mut out = format!("recorded the IRI-minting policy for {graph}\n");
    out.push_str(&format!("\npattern: {parsed}\n"));
    out.push_str(policy_line(parsed));
    out.push_str(&format!(
        "  recorded by {} at {}\n",
        policy.recorded_by(),
        policy.recorded_at()
    ));
    out.push_str(&compared_with_convention(
        parsed,
        suggested,
        PatternStanding::Chosen,
    ));

    match replaced {
        // The one moment the previous policy is visible: recording overwrites it. Said plainly,
        // because a convention that changed with nobody told is how a vocabulary ends up with two
        // generations of IRI and no record of the decision that divided them.
        Some(previous) => out.push_str(&format!(
            "\nthis replaced {:?}, recorded by {} at {}. That earlier pattern is no longer kept \
             anywhere: the IRIs already minted under it are the record of it\n",
            previous.pattern(),
            previous.recorded_by(),
            previous.recorded_at()
        )),
        None => out.push_str("\nthis vocabulary had no recorded pattern before now\n"),
    }

    out.push_str(
        "\nnothing already minted changed: a policy governs the next mint, and an IRI that changes \
         is a different concept. Every producer reads this one — an import, a match against \
         another vocabulary, and a curator running `openbiz mint`\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbiz_skos::IriConvention;

    /// A convention built from IRIs, as a vocabulary's concepts would give it.
    fn convention(iris: &[&str]) -> Result<openbiz_skos::Suggestion, openbiz_skos::NoConvention> {
        let mut convention = IriConvention::new();
        for iri in iris {
            convention.push(iri);
        }
        convention.suggest()
    }

    fn policy_record(pattern: &str) -> IriPolicy {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path()).expect("a fresh store opens");
        let graph = GraphId::vocabulary("https://example.org/energy").expect("a vocabulary IRI");
        store
            .create_vocabulary_graph(&graph)
            .expect("the vocabulary is created");
        store
            .record_iri_policy(&graph, pattern, "ada@example.org")
            .expect("the policy is recorded")
            .policy
    }

    /// The report an operator reads before they have decided anything. It must name the drift, or
    /// nobody has any reason to record a policy at all.
    #[test]
    fn nothing_recorded_says_what_inference_costs_and_how_to_stop_it() {
        let report = show(
            "https://example.org/energy",
            None,
            &convention(&[
                "https://example.org/energy/c_1",
                "https://example.org/energy/c_2",
            ]),
        );

        assert!(report.contains("nothing is recorded"), "{report}");
        assert!(
            report.contains("its own concepts suggest https://example.org/energy/c_{n}"),
            "{report}"
        );
        assert!(
            report.contains("moves as they grow"),
            "the cost of inference is stated: {report}"
        );
        assert!(
            report.contains("openbiz policy https://example.org/energy --pattern"),
            "and the way to stop it is spelled out: {report}"
        );
    }

    /// A vocabulary with no majority namespace suggests nothing, and that is worse without a policy
    /// than with one — so the report says minting is blocked rather than merely unsuggested.
    #[test]
    fn nothing_recorded_and_nothing_suggested_says_minting_is_blocked() {
        let report = show(
            "https://example.org/energy",
            None,
            &convention(&["https://a.example/c_1", "https://b.example/d_1"]),
        );

        assert!(report.contains("they suggest nothing"), "{report}");
        assert!(report.contains("Nothing can be minted here"), "{report}");
    }

    /// The recorded case, agreeing with the vocabulary.
    #[test]
    fn a_recorded_policy_names_its_pattern_its_author_and_its_reach() {
        let policy = policy_record("https://example.org/energy/c_{n}");
        let report = show(
            "https://example.org/energy",
            Some(&policy),
            &convention(&[
                "https://example.org/energy/c_1",
                "https://example.org/energy/c_2",
            ]),
        );

        assert!(
            report.contains("recorded: https://example.org/energy/c_{n}"),
            "{report}"
        );
        assert!(report.contains("an opaque IRI"), "{report}");
        assert!(
            report.contains("recorded by ada@example.org at"),
            "{report}"
        );
        assert!(
            report.contains("which is also what this vocabulary's own concepts suggest"),
            "{report}"
        );
        assert!(
            report.contains("every producer mints under this"),
            "{report}"
        );
    }

    /// A recorded policy the vocabulary's concepts contradict. Not refused — that is how a
    /// convention is changed — and not silent either.
    ///
    /// The second assertion is the one worth reading. Showing a recorded policy is *nobody doing
    /// anything*: the vocabulary's written decision and its existing IRIs differ, and that is a
    /// state, not an act. The first version of this report reused the sentence written for
    /// `--pattern` — "minting under a different pattern is legitimate and it is also how a concept
    /// ends up in the wrong namespace" — which told the reader they were taking a risk they were
    /// not taking. Found by reading the command's own output; the assertion pins that it is gone.
    #[test]
    fn a_recorded_policy_that_disagrees_with_the_concepts_says_so() {
        let policy = policy_record("https://example.org/energy/{slug}");
        let report = show(
            "https://example.org/energy",
            Some(&policy),
            &convention(&[
                "https://example.org/energy/c_1",
                "https://example.org/energy/c_2",
            ]),
        );

        assert!(report.contains("a readable IRI"), "{report}");
        assert!(
            report.contains("suggest https://example.org/energy/c_{n} instead"),
            "the disagreement is named: {report}"
        );
        assert!(
            report.contains("its recorded policy and the IRIs it already holds disagree"),
            "and named as a state of the vocabulary: {report}"
        );
        assert!(
            !report.contains("minting under a different pattern"),
            "nobody is minting anything here: {report}"
        );
    }

    /// A pattern recorded by a build that accepted something this one does not. The report has to
    /// show the text and the reason together, because `openbiz mint` sends people here.
    #[test]
    fn a_recorded_pattern_this_build_cannot_read_is_shown_as_unusable() {
        let policy = policy_record("no placeholder at all");
        let report = show(
            "https://example.org/energy",
            Some(&policy),
            &convention(&["https://example.org/energy/c_1"]),
        );

        assert!(report.contains("recorded, and unusable"), "{report}");
        assert!(report.contains("no placeholder at all"), "{report}");
        assert!(
            report.contains("nothing can be minted for this vocabulary"),
            "{report}"
        );
    }

    /// The first recording, and the paragraph that keeps somebody from thinking their existing IRIs
    /// have just been rewritten.
    #[test]
    fn a_first_recording_says_nothing_already_minted_changed() {
        let policy = policy_record("https://example.org/energy/c_{n}");
        let parsed = MintPattern::parse(policy.pattern()).expect("a pattern");
        let report = recorded_report(
            "https://example.org/energy",
            &parsed,
            &policy,
            None,
            &convention(&["https://example.org/energy/c_1"]),
        );

        assert!(
            report.contains("recorded the IRI-minting policy for https://example.org/energy"),
            "{report}"
        );
        assert!(
            report.contains("had no recorded pattern before now"),
            "{report}"
        );
        assert!(
            report.contains("nothing already minted changed"),
            "{report}"
        );
    }

    /// Replacing one. The previous pattern is named, and so is the fact that this is the last time
    /// it will be.
    #[test]
    fn replacing_a_policy_names_what_it_displaced() {
        let previous = policy_record("https://example.org/energy/c_{n}");
        let policy = policy_record("https://example.org/energy/{slug}");
        let parsed = MintPattern::parse(policy.pattern()).expect("a pattern");
        let report = recorded_report(
            "https://example.org/energy",
            &parsed,
            &policy,
            Some(&previous),
            &convention(&["https://example.org/energy/c_1"]),
        );

        assert!(
            report.contains("this replaced \"https://example.org/energy/c_{n}\""),
            "{report}"
        );
        assert!(report.contains("recorded by ada@example.org"), "{report}");
        assert!(
            report.contains("no longer kept anywhere"),
            "the cost of replacing is stated: {report}"
        );
    }
}
