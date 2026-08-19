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

use openbiz_skos::{Ancestry, CoreModel, Node, Resource, Retirements, WalkBound};
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
pub fn ancestors(store: &Store, graph: &str, concept: &str) -> Result<String, CommandError> {
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
    ))
}

/// The report itself, kept apart from the store so it can be tested against a model in hand.
fn report(
    graph: &str,
    concept: &Node,
    resource: &Resource,
    model: &CoreModel,
    retirements: &Retirements,
    above: Ancestry,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{concept}{}{}\n",
        named(resource),
        status::mark(retirements, concept)
    ));
    out.push_str(&format!("in {graph}\n"));
    status::explain(&mut out, "", retirements, model, concept);

    if above.is_empty() {
        out.push_str(if above.is_complete() {
            "\nnothing is above it: it has no broader concept, directly or transitively.\n"
        } else {
            "\nnothing was reached before the walk hit its bound, so this is not an answer.\n"
        });
        return out;
    }

    out.push_str(&format!(
        "\n{} concept(s) are above it, by {} link(s) walked:\n",
        above.len(),
        above.links_walked()
    ));
    for ancestor in above.ancestors() {
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

    // A current concept whose ancestors are not. `openbiz deprecate` retires a concept and moves
    // nothing below it, so this is the ordinary aftermath of a retirement seen from underneath —
    // and it is invisible in a report that marks the ancestors and says nothing about what the
    // marks together mean for the concept the reader actually asked about.
    if !retirements.is_retired(concept) {
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
        out.push_str("\nthat is all of them.\n");
    }

    out
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
        let report = ancestors(&store, VOCABULARY, "http://example.org/cats")
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
        let report = ancestors(&store, VOCABULARY, "http://example.org/animals")
            .expect("animals is in the vocabulary");

        assert!(report.contains("nothing is above it"), "{report}");
        assert!(!report.contains("bound"), "{report}");
    }

    /// A typo must not read as a root concept. This is the whole reason the command refuses
    /// rather than reporting an empty answer.
    #[test]
    fn a_concept_the_vocabulary_does_not_hold_is_refused() {
        let (_directory, store) = store_with(CHAIN);
        let error = ancestors(&store, VOCABULARY, "http://example.org/catz")
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
        let error = ancestors(&store, "http://example.org/nope", "http://example.org/cats")
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
        let report =
            ancestors(&store, VOCABULARY, "http://example.org/a").expect("a is in the vocabulary");

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
        let report = ancestors(&store, VOCABULARY, "http://example.org/cats")
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
        let report = ancestors(&store, VOCABULARY, "http://example.org/cats")
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
        let report = ancestors(&store, VOCABULARY, "http://example.org/mammals")
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
        let report = ancestors(&store, VOCABULARY, "http://example.org/cats")
            .expect("cats is in the vocabulary");

        assert!(!report.contains("retired"), "{report}");
    }
}
