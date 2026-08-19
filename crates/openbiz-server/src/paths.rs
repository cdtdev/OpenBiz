//! `openbiz paths` — every route from one concept up to a root, and the cycles a route hits.
//!
//! `openbiz ancestors` answers *which* concepts are above one concept, by walking breadth-first
//! and printing one shortest path to each. This answers the other half of the same question and
//! it is not a rendering of the first: in a polyhierarchy the number of ancestors is linear and
//! the number of **routes** is not, so a concept with three ancestors can have two routes or
//! sixteen, and a breadcrumb needs the routes.
//!
//! # What this prints that `ancestors` cannot
//!
//! 1. **Every route, not the shortest one per ancestor.** A concept under two parents that meet
//!    again higher up appears once in `ancestors`, by the shorter way. Here both ways are shown,
//!    because "which way up" is the question an author asks when an unexpected roll-up appears.
//! 2. **A cycle that does not run through the concept asked about.** `ancestors` reports a cycle
//!    only when the walk comes back to its own origin. A loop two levels above a concept is
//!    invisible from there and is still the reason that concept has no route to a root, and this
//!    names it. See [`openbiz_skos::HierarchyCycle`].
//! 3. **Which steps are stated as parent links and which are only transitive.** S22 runs one way,
//!    so a step licensed only by `skos:broaderTransitive` states containment without stating
//!    adjacency: there may be levels in between that the vocabulary does not name, and a
//!    breadcrumb drawn from such a step would claim they are neighbours.
//!
//! # "Root" is two things and this report keeps them apart
//!
//! A **summit** is a concept with no broader concept — where a route stops. A **top concept** is
//! a scheme's declared entry point. SKOS relates neither to the other: its statements about
//! `skos:hasTopConcept` are S5 to S8, which are its domain, its range, its sub-property of
//! `skos:inScheme` and its inverse, and none of them mentions the hierarchy. So a top concept can
//! have a broader concept and a summit can be a top concept of nothing — both legal, both
//! reported, neither a finding. [`openbiz_skos::RootPath`] has the reasoning.
//!
//! # Why a command and not an endpoint
//!
//! As with `inspect`, `ancestors`, `tree`, `notes` and `mappings`, and not the authentication
//! objection: this only reads. The interface's breadcrumb is Phase 3's item, and an endpoint now
//! would be a caller with nothing behind it.

use openbiz_skos::{CoreModel, Node, PathBound, Resource, RootPath, RootPaths, SkosRule};
use openbiz_store::Store;

use crate::cli::CommandError;

/// Report every route from `concept` up to a summit in the vocabulary at `graph`, and why.
///
/// Reads and nothing else.
///
/// A concept the vocabulary never mentions is **refused**, exactly as `openbiz ancestors` and
/// `openbiz tree` refuse one. Here the confusion it prevents is sharper than elsewhere: a concept
/// the graph has never heard of has no broader concept, so it would otherwise be reported as its
/// own root — a confident answer about a typo.
pub fn paths(store: &Store, graph: &str, concept: &str) -> Result<String, CommandError> {
    let model = crate::inspect::read(store, graph)?;

    let node = Node::iri(concept);
    if model.resource(&node).is_none() {
        return Err(CommandError::NoSuchConcept {
            concept: concept.to_owned(),
            graph: graph.to_owned(),
        });
    }

    let found = model.paths_to_root(&node, PathBound::DEFAULT);
    Ok(report(graph, &node, &model, &found))
}

/// The report itself, kept apart from the store so it can be tested against a model in hand.
fn report(graph: &str, concept: &Node, model: &CoreModel, found: &RootPaths) -> String {
    let mut out = String::new();
    out.push_str(&format!("{concept}{}\n", named_in(model, concept)));
    out.push_str(&format!("in {graph}\n"));

    routes_section(&mut out, model, found);
    summits_section(&mut out, model, found);
    cycles_section(&mut out, model, found);

    if !found.is_complete() {
        out.push_str(
            "\nthe enumeration stopped at its bound, so the routes and the cycles above are both \
             lower bounds: there may be routes it never built and loops it never met, and nothing \
             follows from something being absent from either list.\n",
        );
    }

    out
}

/// Every route, as an arrow chain, with the two things a chain alone would not say.
fn routes_section(out: &mut String, model: &CoreModel, found: &RootPaths) {
    if found.is_empty() {
        out.push_str(if found.is_complete() {
            "\nno route from it reaches a concept with no broader concept: every way up runs into \
             a cycle. The cycles are below, and they are the answer rather than a failure to find \
             one.\n"
        } else {
            "\nno route was completed before the enumeration hit its bound, so this is not an \
             answer.\n"
        });
        return;
    }

    out.push_str(&format!(
        "\n{} route(s) up to a concept with no broader concept, by {} link(s) followed:\n",
        found.len(),
        found.steps_walked()
    ));

    let mut any_transitive = false;
    let mut any_inferred = false;
    for (index, route) in found.paths().enumerate() {
        let mut line = format!("  {}. {}", index + 1, chain(model, route));
        if route.steps().len() > 1 {
            any_inferred = true;
            line.push_str("  [S24]");
        }
        out.push_str(&line);
        out.push('\n');
        any_transitive |= route.steps().iter().any(|step| !step.is_stated());
    }

    if any_inferred {
        out.push_str(&format!(
            "  a route marked [S24] puts its last concept above its first by a conclusion \
             transitivity licensed rather than by a link the graph states: {}\n",
            SkosRule::S24.statement()
        ));
    }
    if any_transitive {
        out.push_str(
            "  a step drawn ⇢ is stated only as skos:broaderTransitive. S22 makes skos:broader a \
             sub-property of skos:broaderTransitive and not the other way round, so such a step \
             says the upper concept is somewhere above the lower one and does not say it is \
             directly above it — there may be levels between them this vocabulary does not \
             name.\n",
        );
    }
}

/// One route as `A → B ⇢ C`, with each concept's label and the arrow that says what licensed it.
fn chain(model: &CoreModel, route: &RootPath) -> String {
    let mut out = format!("{}{}", route.origin(), named_in(model, route.origin()));
    for step in route.steps() {
        out.push_str(if step.is_stated() { " → " } else { " ⇢ " });
        out.push_str(&format!(
            "{}{}",
            step.concept(),
            named_in(model, step.concept())
        ));
    }
    out
}

/// Where the routes stop, and where they pass a scheme's declared entry point — kept apart,
/// because SKOS relates neither to the other.
fn summits_section(out: &mut String, model: &CoreModel, found: &RootPaths) {
    let summits = found.summits();
    if summits.is_empty() {
        return;
    }

    out.push_str(&format!(
        "\n{} concept(s) the routes stop at, each having no broader concept:\n",
        summits.len()
    ));
    for summit in &summits {
        out.push_str(&format!("  {summit}{}\n", named_in(model, summit)));
        match model.resource(summit).map(Resource::top_concept_of) {
            Some(schemes) if !schemes.is_empty() => {
                for scheme in schemes {
                    out.push_str(&format!(
                        "    and it is a top concept of {scheme}{}\n",
                        named_in(model, scheme)
                    ));
                }
            }
            _ => out.push_str(
                "    and it is a top concept of no scheme here, which SKOS permits: nothing in it \
                 makes a concept with no broader concept a scheme's entry point.\n",
            ),
        }
    }

    // The case where the two readings of "root" disagree, named where it happens. A reader
    // looking for their scheme's top concept at the end of a route needs to be told it is
    // half-way up rather than left to conclude the vocabulary lost it.
    let mut midway: Vec<String> = Vec::new();
    for route in found.paths() {
        for (concept, schemes) in route.top_concepts() {
            if summits.contains(concept) {
                continue;
            }
            for scheme in schemes {
                let line = format!(
                    "  {concept}{} is a top concept of {scheme}{}, and this vocabulary puts \
                     concepts above it\n",
                    named_in(model, concept),
                    named_in(model, scheme)
                );
                if !midway.contains(&line) {
                    midway.push(line);
                }
            }
        }
    }
    if !midway.is_empty() {
        out.push_str(&format!(
            "\n{} concept(s) on these routes are a scheme's top concept without being where a \
             route stops:\n",
            midway.len()
        ));
        for line in midway {
            out.push_str(&line);
        }
        out.push_str(
            "  SKOS states nothing relating the two. Its statements about skos:hasTopConcept are \
             S5 to S8 — its domain, its range, its sub-property of skos:inScheme, and its inverse \
             — and none of them mentions skos:broader, so a top concept with a broader concept is \
             legal SKOS and not a defect this report has found.\n",
        );
    }
}

/// The loops, named — including the ones no walk from this concept would ever come back through.
fn cycles_section(out: &mut String, model: &CoreModel, found: &RootPaths) {
    if found.cycle_count() == 0 {
        return;
    }

    out.push_str(&format!(
        "\n{} cycle(s) in the hierarchy at or above it, each one a way up that reaches no \
         summit:\n",
        found.cycle_count()
    ));
    for cycle in found.cycles() {
        let round: Vec<String> = cycle
            .concepts()
            .iter()
            .chain(cycle.concepts().first())
            .map(|node| format!("{node}{}", named_in(model, node)))
            .collect();
        out.push_str(&format!("  {}\n", round.join(" → ")));
        // Which of the ways up runs into it. Without this a reader sees a list of routes that do
        // reach a summit and a loop somewhere, and cannot tell that a whole branch above them
        // ends nowhere — which is the question they opened the report with.
        if cycle.approach().is_empty() {
            out.push_str("    reached from the concept asked about, which is in the loop\n");
        } else {
            out.push_str(&format!(
                "    reached from {}\n",
                cycle
                    .approach()
                    .iter()
                    .map(|node| format!("{node}{}", named_in(model, node)))
                    .collect::<Vec<_>>()
                    .join(" → ")
            ));
        }
        if let Some(derivation) = cycle.derivation() {
            out.push_str(&format!("    {}\n", derivation.conclusion));
            out.push_str(&format!("    because {}\n", derivation.premise));
            out.push_str(&format!("    and {}\n", derivation.rule));
        }
    }
    out.push_str(
        "  §8.6.8 of the SKOS Reference marks a cycle consistent with the SKOS data model, so \
         this is what the hierarchy says and not a defect this report has found. It is printed \
         because a route that runs into one has no root to reach, and a breadcrumb that simply \
         stopped there would not say why.\n",
    );
}

/// A concept's label in parentheses, or nothing if the vocabulary gives it none.
fn named_in(model: &CoreModel, node: &Node) -> String {
    match model.resource(node).and_then(Resource::display_label) {
        Some(label) => format!("  ({label})"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use openbiz_store::{GraphId, RdfSyntax, Store};

    use super::paths;
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

    /// A diamond: Poodles is under Dogs and under Pets, both of which are under Animals.
    const DIAMOND: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix ex: <http://example.org/> .
        ex:scheme a skos:ConceptScheme ; skos:hasTopConcept ex:animals .
        ex:poodles a skos:Concept ; skos:prefLabel "Poodles"@en ;
            skos:broader ex:dogs, ex:pets .
        ex:dogs a skos:Concept ; skos:prefLabel "Dogs"@en ; skos:broader ex:animals .
        ex:pets a skos:Concept ; skos:prefLabel "Pets"@en ; skos:broader ex:animals .
        ex:animals a skos:Concept ; skos:prefLabel "Animals"@en .
    "#;

    /// The command's reason for existing: two routes to one summit, both printed. `ancestors`
    /// reports Animals once, by whichever way is shorter, and cannot say there are two.
    #[test]
    fn a_diamond_prints_both_routes_to_one_summit() {
        let (_directory, store) = store_with(DIAMOND);
        let report = paths(&store, VOCABULARY, "http://example.org/poodles")
            .expect("poodles is in the vocabulary");

        assert!(report.contains("2 route(s) up to a concept"), "{report}");
        assert!(
            report.contains(
                "<http://example.org/poodles>  (\"Poodles\"@en) → <http://example.org/dogs>  \
                 (\"Dogs\"@en) → <http://example.org/animals>  (\"Animals\"@en)"
            ),
            "the route through Dogs is printed with its labels: {report}"
        );
        assert!(
            report.contains("<http://example.org/pets>  (\"Pets\"@en) → "),
            "the route through Pets is printed too: {report}"
        );
        assert!(
            report.contains("1 concept(s) the routes stop at"),
            "two routes, one summit: {report}"
        );
        assert!(
            report.contains("it is a top concept of <http://example.org/scheme>"),
            "the summit is the scheme's entry point and the report says so: {report}"
        );
        assert!(report.contains("[S24]"), "{report}");
        assert!(
            !report.contains("⇢"),
            "every step here is a stated skos:broader link: {report}"
        );
        assert!(!report.contains("cycle"), "{report}");
    }

    /// A typo must not read as a root concept. A concept the graph never mentions has no broader
    /// concept, so without the refusal this command would confidently call it its own root.
    #[test]
    fn a_concept_the_vocabulary_does_not_hold_is_refused() {
        let (_directory, store) = store_with(DIAMOND);
        let error = paths(&store, VOCABULARY, "http://example.org/poodlez")
            .expect_err("poodlez is not in the vocabulary");

        assert!(
            matches!(error, CommandError::NoSuchConcept { .. }),
            "{error:?}"
        );
    }

    /// An unregistered graph is refused by the store, as every other reading command refuses one.
    #[test]
    fn an_unregistered_vocabulary_is_refused() {
        let (_directory, store) = store_with(DIAMOND);
        let error = paths(
            &store,
            "http://example.org/nope",
            "http://example.org/poodles",
        )
        .expect_err("that vocabulary is not registered");

        assert!(matches!(error, CommandError::Store(_)), "{error:?}");
    }

    /// The summit says so plainly, and does not claim a scheme it is not in.
    #[test]
    fn a_summit_that_is_no_scheme_s_top_concept_says_so() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:a a skos:Concept ; skos:broader ex:b .
            ex:b a skos:Concept .
            "#,
        );
        let report =
            paths(&store, VOCABULARY, "http://example.org/a").expect("a is in the vocabulary");

        assert!(report.contains("1 route(s) up to a concept"), "{report}");
        assert!(
            report.contains("it is a top concept of no scheme here"),
            "{report}"
        );
        assert!(
            !report.contains("[S24]"),
            "one step is S22's conclusion, not S24's: {report}"
        );
    }

    /// **The half `ancestors` cannot reach.** The loop is above the concept and does not run
    /// through it, so the upward walk from it never comes back and reports nothing — while the
    /// loop is exactly why this concept has no route to a root.
    #[test]
    fn a_cycle_above_the_concept_is_named_and_explains_the_missing_route() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:leaf a skos:Concept ; skos:prefLabel "Leaf"@en ; skos:broader ex:b .
            ex:b a skos:Concept ; skos:broader ex:c .
            ex:c a skos:Concept ; skos:broader ex:b .
            "#,
        );
        let report =
            paths(&store, VOCABULARY, "http://example.org/leaf").expect("leaf is in the store");

        assert!(
            report.contains("no route from it reaches a concept with no broader concept"),
            "{report}"
        );
        assert!(report.contains("1 cycle(s) in the hierarchy"), "{report}");
        assert!(
            report.contains(
                "<http://example.org/b> → <http://example.org/c> → <http://example.org/b>"
            ),
            "the loop is printed closing on itself: {report}"
        );
        assert!(
            report.contains("§8.6.8 of the SKOS Reference marks a cycle consistent"),
            "a legal cycle must not be reported as a defect: {report}"
        );
        assert!(
            report.contains("reached from <http://example.org/leaf>  (\"Leaf\"@en)"),
            "the way up that runs into the loop is named, so the reader knows which branch of \
             their hierarchy ends nowhere: {report}"
        );
        assert!(
            report.contains("S24: skos:broaderTransitive and skos:narrowerTransitive are each"),
            "the specification statement is quoted, not just cited: {report}"
        );

        // And the upward walk, from the same store, has nothing to say about the loop.
        let walk = crate::ancestors(&store, VOCABULARY, "http://example.org/leaf")
            .expect("leaf is in the store");
        assert!(
            !walk.contains(
                "http://example.org/leaf> → <http://example.org/b> → \
                            <http://example.org/c> → <http://example.org/b>"
            ),
            "ancestors reports a cycle only through its own origin: {walk}"
        );
    }

    /// A step stated only as `skos:broaderTransitive` is drawn differently and the legend says
    /// what the difference means. Levels may be missing between the two concepts, and a
    /// breadcrumb that drew them adjacent would be claiming something the graph does not say.
    #[test]
    fn a_transitive_only_step_is_drawn_and_labelled_differently() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:leaf a skos:Concept ; skos:broaderTransitive ex:top .
            ex:top a skos:Concept .
            "#,
        );
        let report =
            paths(&store, VOCABULARY, "http://example.org/leaf").expect("leaf is in the store");

        assert!(
            report.contains("<http://example.org/leaf> ⇢ <http://example.org/top>"),
            "{report}"
        );
        assert!(
            report.contains("stated only as skos:broaderTransitive"),
            "{report}"
        );
        assert!(
            report.contains("does not say it is directly above it"),
            "the difference is adjacency and the report must say so: {report}"
        );
    }

    /// **The two readings of "root", disagreeing.** A scheme's top concept with a broader concept
    /// is legal SKOS, and a reader who found their entry point half-way up a route must be told
    /// that rather than left to conclude the report lost it.
    #[test]
    fn a_top_concept_that_is_not_where_a_route_stops_is_named_as_such() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:scheme a skos:ConceptScheme ; skos:hasTopConcept ex:middle .
            ex:leaf a skos:Concept ; skos:broader ex:middle .
            ex:middle a skos:Concept ; skos:prefLabel "Middle"@en ; skos:broader ex:above .
            ex:above a skos:Concept .
            "#,
        );
        let report =
            paths(&store, VOCABULARY, "http://example.org/leaf").expect("leaf is in the store");

        assert!(
            report.contains(
                "1 concept(s) on these routes are a scheme's top concept without \
                             being where a route stops"
            ),
            "{report}"
        );
        assert!(
            report.contains(
                "<http://example.org/middle>  (\"Middle\"@en) is a top concept of \
                 <http://example.org/scheme>"
            ),
            "{report}"
        );
        assert!(
            report.contains("none of them mentions skos:broader"),
            "the negative claim about the specification is stated, not implied: {report}"
        );
        assert!(
            report.contains("<http://example.org/above>"),
            "the route still stops where the hierarchy does: {report}"
        );
    }
}
