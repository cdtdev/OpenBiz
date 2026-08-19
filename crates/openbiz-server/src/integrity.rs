//! `openbiz integrity` — the roll-call: every condition, named, with its verdict.
//!
//! `openbiz inspect` closes with one sentence about the whole graph: it violates an integrity
//! condition, or it does not. That sentence is what an author needs and it is not what an auditor
//! needs. "No integrity condition is violated" does not say **which** conditions were checked, and
//! a governance function defending a decision has to be able to answer that in the specific:
//! *S14 was checked over 41 000 concepts and held; S27 was not checked at all, because your
//! vocabulary refines `skos:related` and this build draws no conclusions from that.*
//!
//! # The three verdicts, and why the third is the point
//!
//! Held, violated, unchecked. Collapsing *unchecked* into *held* is the false green this build
//! spends most of its defensive effort on, and the roll-call is where it is finally attributed per
//! condition rather than for the model as a whole: a bounded ancestry walk costs §8.4's check and
//! says nothing whatever about §5.4's, but `CoreModel::checks_are_complete` — the only answer
//! available before this command — answers `false` for the model and reads as though everything
//! were in doubt.
//!
//! # Two groups, because ten of the sixteen are our reading and not the document's
//!
//! Six conditions sit under a heading called "Integrity Conditions" — §4.4, §5.4, §8.4, §9.4,
//! §10.4. Ten further statements can make this build call a graph inconsistent, and none of them
//! does. Printing all sixteen under one heading would put words in the specification's mouth;
//! printing only six would report every condition held on a vocabulary this build calls
//! inconsistent. So they are printed apart, and the second group says whose judgement it is.
//!
//! # Why a command and not an endpoint
//!
//! As with `inspect`, `ancestors`, `notes` and `mappings`, and not the authentication objection:
//! this only reads. The compliance view that will render this beside a vocabulary is Phase 3's.

use std::collections::BTreeMap;

use openbiz_skos::{Authority, Caveat, ConditionOutcome, CoreModel, Verdict};
use openbiz_store::Store;

use crate::cli::CommandError;

/// Report which integrity conditions the vocabulary at `graph` satisfies, and which were checked.
///
/// Reads and nothing else. An IRI with no registry entry is refused rather than reported as a
/// vocabulary that satisfies every condition vacuously — which it would, and which is exactly the
/// answer a mistyped IRI must not produce here.
pub fn integrity(store: &Store, graph: &str) -> Result<String, CommandError> {
    let model = crate::inspect::read(store, graph)?;
    Ok(report(graph, &model))
}

/// The report itself, kept apart from the store so it can be tested against a model in hand.
fn report(graph: &str, model: &CoreModel) -> String {
    let outcomes = model.integrity();
    let mut out = String::new();

    out.push_str(&format!("integrity of {graph}\n"));
    out.push_str(&format!("{} statement(s) read\n", model.statements_read()));

    out.push_str(&format!("\n{}\n", verdict_line(&outcomes)));

    out.push_str(
        "\nthe SKOS Reference's own integrity conditions\n  \u{a7}4.4, \u{a7}5.4, \u{a7}8.4, \
         \u{a7}9.4 and \u{a7}10.4 are the five sections headed \"Integrity Conditions\", and \
         these six are all\n  they state. A graph violating one is not a SKOS vocabulary, and \
         that is the specification's word rather than ours.\n",
    );
    for outcome in outcomes
        .iter()
        .filter(|outcome| outcome.condition.authority() == Authority::Specification)
    {
        out.push_str(&summary(outcome));
    }

    out.push_str(
        "\nstatements this build treats as contradictions, by our reading\n  \u{a7}1.7 sets out \
         the structure every section follows, and Appendix B has no \"Integrity Conditions\" \
         heading at\n  all. These ten are axioms rather than conditions, and a graph breaking one \
         holds a logical contradiction —\n  a resource in two disjoint classes, a property given \
         a value of the wrong kind. Calling that an\n  inconsistency is our judgement, said as \
         ours.\n",
    );
    for outcome in outcomes
        .iter()
        .filter(|outcome| outcome.condition.authority() == Authority::OurReading)
    {
        out.push_str(&summary(outcome));
    }

    // Everything that is not simply "held" gets the specification's own words and the evidence.
    // A one-line verdict an operator cannot check is a verdict they have to take on trust, which
    // is the position `docs/COMPETITIVE.md` records the incumbents putting a governance team in.
    let interesting: Vec<&ConditionOutcome> = outcomes
        .iter()
        .filter(|outcome| outcome.verdict() != Verdict::Held)
        .collect();
    if !interesting.is_empty() {
        out.push_str("\nin detail\n");
        for outcome in interesting {
            out.push_str(&detail(outcome));
        }
    }

    out.push_str(&read_past(&outcomes));
    out.push_str(&closing(&outcomes));
    out
}

/// The headline: what this vocabulary is, in one sentence.
fn verdict_line(outcomes: &[ConditionOutcome]) -> String {
    let specified = |verdict: Verdict| {
        outcomes
            .iter()
            .filter(|outcome| {
                outcome.condition.authority() == Authority::Specification
                    && outcome.verdict() == verdict
            })
            .count()
    };
    let violated = outcomes
        .iter()
        .filter(|outcome| outcome.verdict() == Verdict::Violated)
        .count();
    let unchecked = outcomes
        .iter()
        .filter(|outcome| outcome.verdict() == Verdict::Unchecked)
        .count();

    if violated > 0 {
        return format!(
            "VIOLATED — {violated} of {} condition(s) checked here, {} of the specification's \
             six, are violated.\nthis graph is not a SKOS vocabulary.",
            outcomes.len(),
            specified(Verdict::Violated)
        );
    }
    if unchecked > 0 {
        return format!(
            "UNCHECKED — no violation was found, and {unchecked} of {} condition(s) were not \
             checked over the whole\nvocabulary. That is not the same as holding: see the detail \
             below for what each one is missing.",
            outcomes.len()
        );
    }
    format!(
        "HELD — all {} condition(s) were checked over the whole vocabulary and none is violated.",
        outcomes.len()
    )
}

/// One line per condition: the number, the section, the verdict, and what it forbids.
fn summary(outcome: &ConditionOutcome) -> String {
    let verdict = match outcome.verdict() {
        Verdict::Violated => format!("VIOLATED ({})", outcome.violations.len()),
        Verdict::Held => "held".to_owned(),
        Verdict::Unchecked => "unchecked".to_owned(),
    };
    format!(
        "  {:<4} {:<14} {:<13} {}\n",
        outcome.condition.rule().number(),
        outcome.condition.section(),
        verdict,
        outcome.condition.forbids()
    )
}

/// The specification's own words, the counter-examples, and what stopped the check.
fn detail(outcome: &ConditionOutcome) -> String {
    let mut out = format!(
        "\n  {} — {}\n",
        outcome.condition.rule().number(),
        outcome.condition.rule().statement()
    );
    out.push_str(&format!(
        "  {}, {}\n",
        outcome.condition.section(),
        outcome.condition.authority()
    ));

    if !outcome.violations.is_empty() {
        out.push_str(&format!(
            "  violated — {} counter-example(s):\n",
            outcome.violations.len()
        ));
        for violation in &outcome.violations {
            out.push_str(&format!("    {violation}\n"));
        }
    }

    if !outcome.caveats.is_empty() {
        // The wording differs by verdict on purpose. "There may be more" after a violation and
        // "this is not a pass" after none are different warnings, and printing one sentence for
        // both would blunt whichever one mattered.
        out.push_str(if outcome.violations.is_empty() {
            "  not checked over the whole vocabulary, so there is no verdict:\n"
        } else {
            "  and the check did not cover the whole vocabulary, so there may be more:\n"
        });
        for caveat in &outcome.caveats {
            out.push_str(&format!("    {caveat}\n"));
        }
    }

    out
}

/// The declarations this build read past, said **once** with the reason, and the conditions each
/// one leaves without a verdict.
///
/// It is a section of its own because one ordinary declaration clouds several conditions at once:
/// SKOS entails class membership from its own properties, so an unread `rdfs:subPropertyOf
/// skos:related` reaches S9, S18, S27, S37 and S48 together. Running the command against a store
/// on disk printed the same four-line explanation five times over, which buries the one fact the
/// reader needs — that this vocabulary uses an extension point and this build does not follow it.
fn read_past(outcomes: &[ConditionOutcome]) -> String {
    // Kept in `CONDITIONS`' order rather than sorted, because sorting the S-numbers as text puts
    // S9 after S48 — which reads as a mistake on a line whose whole job is to be checkable
    // against the table above it.
    let mut clouded: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
    for outcome in outcomes {
        for caveat in &outcome.caveats {
            if let Caveat::UnreadRefinement(refinement) = caveat {
                clouded
                    .entry(refinement.to_string())
                    .or_default()
                    .push(outcome.condition.rule().number());
            }
        }
    }
    if clouded.is_empty() {
        return String::new();
    }

    let mut out = String::from(
        "\nwhat this build read past\n  \u{a7}7.1 offers rdfs:subPropertyOf as an extension \
         point, and this build resolves it for the seven\n  documentation properties only. \
         rdfs:subClassOf is not read at all. Statements made with the terms\n  below were \
         therefore read as non-SKOS, and the conditions beside each were checked over a graph\n  \
         missing them. Nothing here is a defect in the vocabulary \u{2014} it is the boundary of \
         what this build\n  entails, and it is the difference between \"unchecked\" and \
         \"held\".\n",
    );
    for (refinement, conditions) in clouded {
        out.push_str(&format!("    {refinement}\n"));
        out.push_str(&format!(
            "      leaves unchecked: {}\n",
            conditions.join(", ")
        ));
    }
    out
}

/// What "held" means, said once, at the end where a reader stops.
fn closing(outcomes: &[ConditionOutcome]) -> String {
    let held = outcomes
        .iter()
        .filter(|outcome| outcome.verdict() == Verdict::Held)
        .count();
    format!(
        "\n{held} condition(s) held. \"Held\" means the check ran over this whole vocabulary and \
         found no\ncounter-example — it is a statement about what is in the graph, not about what \
         a later edit might add.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbiz_skos::{CoreModel, Node, Statement, Term, SKOS_BROADER, SKOS_RELATED};

    const GRAPH: &str = "https://example.org/regions";

    fn node(local: &str) -> Node {
        Node::iri(format!("https://example.org/{local}"))
    }

    fn stated(subject: &str, predicate: &str, object: &str) -> Statement {
        Statement::new(node(subject), predicate.to_owned(), node(object))
    }

    /// Every condition is on the page whatever the vocabulary says, because a condition that
    /// appears only when it fails is one an operator cannot tell was checked.
    #[test]
    fn every_condition_is_named_in_the_report() {
        let model = CoreModel::from_statements([stated("a", SKOS_BROADER, "b")]);
        let report = report(GRAPH, &model);

        for condition in openbiz_skos::CONDITIONS {
            assert!(
                report.contains(condition.rule().number()),
                "{} is missing from the report:\n{report}",
                condition.rule()
            );
        }
        assert!(report.contains("HELD"), "{report}");
    }

    /// The two groups are printed apart, and the second says whose judgement it is.
    ///
    /// Putting S48 under a heading that says "the SKOS Reference's own integrity conditions"
    /// would be citing the specification for a classification it does not make — the failure
    /// `docs/COMPETITIVE.md` records of the incumbents, in the one place we claim to be better.
    #[test]
    fn our_own_classifications_are_printed_apart_from_the_specifications() {
        let model = CoreModel::from_statements([stated("a", SKOS_BROADER, "b")]);
        let report = report(GRAPH, &model);

        let specified = report
            .find("the SKOS Reference's own integrity conditions")
            .expect("the first heading");
        let ours = report
            .find("statements this build treats as contradictions, by our reading")
            .expect("the second heading");
        let s9 = report.find("  S9  ").expect("S9's line");
        let s48 = report.find("  S48 ").expect("S48's line");

        assert!(specified < s9 && s9 < ours, "{report}");
        assert!(ours < s48, "{report}");
    }

    /// A violation is named by S-number, quoted, and shown.
    #[test]
    fn a_violated_condition_is_quoted_and_evidenced() {
        let model = CoreModel::from_statements([
            stated("a", SKOS_BROADER, "b"),
            stated("a", SKOS_RELATED, "b"),
        ]);
        let report = report(GRAPH, &model);

        assert!(report.contains("VIOLATED"), "{report}");
        assert!(report.contains("not a SKOS vocabulary"), "{report}");
        assert!(
            report.contains("skos:related is disjoint with the property skos:broaderTransitive."),
            "{report}"
        );
        assert!(report.contains("counter-example(s)"), "{report}");
    }

    /// An unchecked condition does not read as a pass, and says what it is missing.
    ///
    /// This is the report's reason for existing. The same vocabulary through `openbiz inspect`
    /// prints "no SKOS integrity condition is violated", which is true, and which an operator
    /// reads as "S27 held".
    #[test]
    fn an_unchecked_condition_does_not_read_as_held() {
        let model = CoreModel::from_statements([
            Statement::new(
                Node::iri("https://example.org/seeAlso"),
                openbiz_skos::RDFS_SUB_PROPERTY_OF.to_owned(),
                Term::Node(Node::iri(SKOS_RELATED)),
            ),
            stated("a", "https://example.org/seeAlso", "b"),
            stated("a", SKOS_BROADER, "b"),
        ]);
        let report = report(GRAPH, &model);

        assert!(report.contains("UNCHECKED"), "{report}");
        assert!(report.contains("S27  \u{a7}8.4"), "{report}");
        assert!(
            report.contains("not checked over the whole vocabulary, so there is no verdict"),
            "{report}"
        );
        assert!(
            report.contains("this build entails nothing from it"),
            "{report}"
        );
        assert!(
            report.contains("documentation properties only"),
            "the report must say why it read past the declaration:\n{report}"
        );
        // Said once, not once per condition it clouds. The first draft repeated the whole
        // explanation under every affected condition, which was five times over on a two-line
        // vocabulary.
        assert_eq!(
            report.matches("documentation properties only").count(),
            1,
            "{report}"
        );
        assert!(
            report.contains("leaves unchecked: S9, S27, S37, S18, S48"),
            "one declaration clouds five conditions, listed in the table's order:\n{report}"
        );
    }

    /// A vocabulary with nothing in it holds every condition vacuously, and the closing sentence
    /// says what that does and does not mean.
    #[test]
    fn an_empty_vocabulary_holds_every_condition_and_the_report_qualifies_it() {
        let report = report(GRAPH, &CoreModel::from_statements([]));

        assert!(report.contains("HELD"), "{report}");
        assert!(
            report.contains("found no\ncounter-example"),
            "the closing qualification is missing:\n{report}"
        );
    }
}
