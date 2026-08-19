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

use std::collections::BTreeSet;

use openbiz_skos::{
    CoreModel, Node, Offered, PathBound, Resource, Retirements, RootPath, RootPaths, RouteStep,
    SkosRule,
};
use openbiz_store::Store;

use crate::cli::CommandError;
use crate::status;

/// Report every route from `concept` up to a summit in the vocabulary at `graph`, and why.
///
/// Reads and nothing else.
///
/// A concept the vocabulary never mentions is **refused**, exactly as `openbiz ancestors` and
/// `openbiz tree` refuse one. Here the confusion it prevents is sharper than elsewhere: a concept
/// the graph has never heard of has no broader concept, so it would otherwise be reported as its
/// own root — a confident answer about a typo.
pub fn paths(
    store: &Store,
    graph: &str,
    concept: &str,
    current_only: bool,
) -> Result<String, CommandError> {
    let (model, retirements) = crate::inspect::read_with_retirements(store, graph)?;

    let node = Node::iri(concept);
    if model.resource(&node).is_none() {
        return Err(CommandError::NoSuchConcept {
            concept: concept.to_owned(),
            graph: graph.to_owned(),
        });
    }

    let found = model.paths_to_root(&node, PathBound::DEFAULT);
    Ok(report(
        graph,
        &node,
        &model,
        &retirements,
        &found,
        current_only,
    ))
}

/// The report itself, kept apart from the store so it can be tested against a model in hand.
fn report(
    graph: &str,
    concept: &Node,
    model: &CoreModel,
    retirements: &Retirements,
    found: &RootPaths,
    current_only: bool,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{concept}{}{}\n",
        named_in(model, concept),
        status::mark(retirements, concept)
    ));
    out.push_str(&format!("in {graph}\n"));
    // Said before any count, because every number under it is a number about a narrowed report.
    // The second sentence is this flag's whole rule: a route is atomic, so it is offered whole or
    // withheld whole and never repaired (`docs/adr/0045` §2).
    if current_only {
        out.push_str(
            "current concepts only: a route is offered only if every concept on it is current. A \
             route through a retired concept is withheld entire and counted — no route is \
             shortened past one, because a shortened route would claim its two neighbours are \
             directly linked and this vocabulary does not say that.\n",
        );
    }
    status::explain(&mut out, "", retirements, model, concept);
    // The concept the report is *about* is never the thing being filtered: the reader named it.
    if current_only && retirements.is_retired(concept) {
        out.push_str(
            "it is shown whatever its status because you asked about it by name; --current \
             applies to the routes above it.\n",
        );
    }

    // Built here and handed over as a set of nodes: `owl:deprecated` is not SKOS, so which
    // resources to leave out is a status question answered beside the model (`docs/adr/0041`).
    // Empty when the flag is off, which makes the narrowing offer everything.
    let skip = match current_only {
        true => status::retired_in(retirements),
        false => BTreeSet::new(),
    };
    let offered = found.excluding(&skip);

    routes_section(&mut out, model, found, &offered, current_only);
    match current_only {
        // Vacuous by construction: no offered route touches a retired concept. What replaces it
        // is the count of the routes that do, which is the thing a narrowed report owes its
        // reader.
        true => withheld_note(&mut out, &offered),
        false => retired_on_routes(&mut out, model, retirements, found),
    }
    summits_section(&mut out, model, retirements, &offered);
    cycles_section(&mut out, model, found, current_only);

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
fn routes_section(
    out: &mut String,
    model: &CoreModel,
    found: &RootPaths,
    offered: &Offered<'_>,
    current_only: bool,
) {
    if offered.is_empty() {
        // The case `--current` has to be safe in, and the reason `Offered` counts what it
        // withheld rather than letting a caller subtract. Unsaid, an emptied list prints the
        // sentence reserved for a hierarchy whose every way up runs into a loop — blaming a cycle
        // that need not exist, about a vocabulary whose routes are all intact and all obsolete.
        if offered.withheld() > 0 {
            out.push_str(&format!(
                "\nno route from it is current the whole way up: all {} route(s) it has run \
                 through a retired concept, and --current was asked for. Those routes still hold \
                 — retiring a concept removes no link — so run the same command without --current \
                 to see them and what each retired concept says to use instead.\n",
                offered.withheld()
            ));
            if !found.is_complete() {
                out.push_str(
                    "\nthe enumeration also stopped at its bound, so a route that is current the \
                     whole way up may simply not have been built.\n",
                );
            }
            return;
        }
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
        "\n{} {}route(s) up to a concept with no broader concept, by {} link(s) followed:\n",
        offered.len(),
        match current_only {
            true => "current ",
            false => "",
        },
        found.steps_walked()
    ));

    let mut any_transitive = false;
    let mut any_inferred = false;
    for (index, route) in offered.routes().enumerate() {
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

/// The retired concepts the routes pass through, listed once rather than marked in every chain.
///
/// A route is printed as one arrow chain on one line, and hanging `[retired]` off each concept in
/// it would make the chain — the thing the report exists to show — unreadable. So the marker is
/// lifted out: the routes stay legible, and the fact that a breadcrumb trail runs through obsolete
/// concepts is stated once, in full, with each of them named.
///
/// It is stated at all because a route through a retired concept is still a true route. Nothing
/// was removed when the concept was retired, so the hierarchy is exactly as it was, and a reader
/// given the chain alone would have no way to tell the difference.
fn retired_on_routes(
    out: &mut String,
    model: &CoreModel,
    retirements: &Retirements,
    found: &RootPaths,
) {
    let mut retired: Vec<&Node> = Vec::new();
    for route in found.paths() {
        for node in
            std::iter::once(route.origin()).chain(route.steps().iter().map(RouteStep::concept))
        {
            if retirements.is_retired(node) && !retired.contains(&node) {
                retired.push(node);
            }
        }
    }
    if retired.is_empty() {
        return;
    }

    out.push_str(&format!(
        "\n{} concept(s) on these routes are retired:\n",
        retired.len()
    ));
    for node in retired {
        out.push_str(&format!("  {node}{}\n", named_in(model, node)));
    }
    out.push_str(
        "  the routes above still hold: retiring a concept removes nothing, so every link they \
         follow is still stated. A breadcrumb built from one of them would show a reader a term \
         the vocabulary no longer wants used.\n",
    );
}

/// What `--current` withheld, always said, and never said as nothing.
///
/// The rule `docs/adr/0043` set for `openbiz search` and every browse command has inherited:
/// **the routes go, the fact that there were routes stays.** A reader asking for the routes above
/// a concept is drawing a breadcrumb or deciding where a new concept belongs, and a list that
/// quietly lost every obsolete way up reads as a vocabulary with fewer ways up than it has.
///
/// Nothing is printed when nothing was withheld, which is every enumeration in the overwhelming
/// majority of vocabularies: they retire nothing.
fn withheld_note(out: &mut String, offered: &Offered<'_>) {
    if offered.withheld() == 0 {
        return;
    }
    out.push_str(&format!(
        "\n{} more route(s) up from it run through a retired concept and are not shown because \
         --current was asked for. They still hold — retiring a concept removes no link — so run \
         the same command without --current to see them and which concepts on them are obsolete.\n",
        offered.withheld()
    ));
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
fn summits_section(
    out: &mut String,
    model: &CoreModel,
    retirements: &Retirements,
    offered: &Offered<'_>,
) {
    let summits = offered.summits();
    if summits.is_empty() {
        return;
    }

    out.push_str(&format!(
        "\n{} concept(s) the routes stop at, each having no broader concept:\n",
        summits.len()
    ));
    for summit in &summits {
        out.push_str(&format!(
            "  {summit}{}{}\n",
            named_in(model, summit),
            status::mark(retirements, summit)
        ));
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
    for route in offered.routes() {
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
fn cycles_section(out: &mut String, model: &CoreModel, found: &RootPaths, current_only: bool) {
    if found.cycle_count() == 0 {
        return;
    }

    out.push_str(&format!(
        "\n{} cycle(s) in the hierarchy at or above it, each one a way up that reaches no \
         summit:\n",
        found.cycle_count()
    ));
    // `docs/adr/0045` §3. A cycle is not a route offered to anyone; it is why a route reaches no
    // summit, and §8.6.8 makes it consistent SKOS rather than a defect. Withholding one because a
    // retired concept is in it would leave an empty route list with its explanation deleted, so
    // this section is never narrowed — and under the flag it says so, because a reader told the
    // retired concepts are out would otherwise read a retired concept here as the flag failing.
    if current_only {
        out.push_str(
            "  --current does not narrow this list. A cycle is not a route on offer, it is why a \
             route reaches no summit, and leaving one out because a retired concept lies in it \
             would remove the explanation and not the problem.\n",
        );
    }
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
        let report = paths(&store, VOCABULARY, "http://example.org/poodles", false)
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
        let error = paths(&store, VOCABULARY, "http://example.org/poodlez", false)
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
            false,
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
        let report = paths(&store, VOCABULARY, "http://example.org/a", false)
            .expect("a is in the vocabulary");

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
        let report = paths(&store, VOCABULARY, "http://example.org/leaf", false)
            .expect("leaf is in the store");

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
        let walk = crate::ancestors(&store, VOCABULARY, "http://example.org/leaf", false)
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
        let report = paths(&store, VOCABULARY, "http://example.org/leaf", false)
            .expect("leaf is in the store");

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
        let report = paths(&store, VOCABULARY, "http://example.org/leaf", false)
            .expect("leaf is in the store");

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

    /// The same diamond with `Dogs` retired, so one of the two routes runs through an obsolete
    /// concept and the other does not.
    const RETIRED_ROUTE: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix owl: <http://www.w3.org/2002/07/owl#> .
        @prefix ex: <http://example.org/> .
        ex:scheme a skos:ConceptScheme ; skos:hasTopConcept ex:animals .
        ex:poodles a skos:Concept ; skos:prefLabel "Poodles"@en ;
            skos:broader ex:dogs, ex:pets .
        ex:dogs a skos:Concept ; skos:prefLabel "Dogs"@en ; skos:broader ex:animals ;
            owl:deprecated true .
        ex:pets a skos:Concept ; skos:prefLabel "Pets"@en ; skos:broader ex:animals .
        ex:animals a skos:Concept ; skos:prefLabel "Animals"@en .
    "#;

    /// Lifted out of the chains rather than hung off every concept in them: a route is one line
    /// and it is the thing this report exists to show.
    #[test]
    fn a_retired_concept_on_a_route_is_named_once_and_not_marked_in_the_chain() {
        let (_directory, store) = store_with(RETIRED_ROUTE);
        let report = paths(&store, VOCABULARY, "http://example.org/poodles", false)
            .expect("poodles is in the vocabulary");

        assert!(report.contains("2 route(s) up to a concept"), "{report}");
        assert!(
            report.contains("1 concept(s) on these routes are retired:"),
            "{report}"
        );
        assert!(
            report.contains("  <http://example.org/dogs>  (\"Dogs\"@en)\n"),
            "{report}"
        );
        assert!(
            report.contains("the routes above still hold"),
            "a route through a retired concept is still a true route: {report}"
        );
        // The chain itself is untouched, so it stays readable.
        assert!(
            report.contains(
                "<http://example.org/poodles>  (\"Poodles\"@en) → <http://example.org/dogs>  \
                 (\"Dogs\"@en) → <http://example.org/animals>  (\"Animals\"@en)"
            ),
            "{report}"
        );
    }

    /// A retired concept asked about directly gets the full account at the top.
    #[test]
    fn a_retired_concept_asked_about_is_marked_and_explained() {
        let (_directory, store) = store_with(RETIRED_ROUTE);
        let report = paths(&store, VOCABULARY, "http://example.org/dogs", false)
            .expect("dogs is in the vocabulary");

        assert!(report.contains("[retired]"), "{report}");
        assert!(
            report.contains("the vocabulary marks it owl:deprecated"),
            "{report}"
        );
    }

    /// A vocabulary that retires nothing reads exactly as it did.
    #[test]
    fn a_vocabulary_with_no_retirements_says_nothing_about_them() {
        let (_directory, store) = store_with(DIAMOND);
        let report = paths(&store, VOCABULARY, "http://example.org/poodles", false)
            .expect("poodles is in the vocabulary");

        assert!(!report.contains("retired"), "{report}");
    }

    /// `docs/adr/0045` §2, and the decision the item turned on. One route runs through retired
    /// `Dogs` and goes whole; the other is untouched. Neither is repaired into the other.
    #[test]
    fn current_only_withholds_the_whole_route_and_shortens_none() {
        let (_directory, store) = store_with(RETIRED_ROUTE);
        let report = paths(&store, VOCABULARY, "http://example.org/poodles", true)
            .expect("poodles is in the vocabulary");

        assert!(
            report.contains("1 current route(s) up to a concept"),
            "{report}"
        );
        assert!(
            report.contains(
                "<http://example.org/poodles>  (\"Poodles\"@en) → <http://example.org/pets>  \
                 (\"Pets\"@en) → <http://example.org/animals>  (\"Animals\"@en)"
            ),
            "the current route is offered whole: {report}"
        );
        assert!(
            !report.contains("<http://example.org/dogs>"),
            "the withheld route is not printed, in whole or in part: {report}"
        );
        // The failure the rule exists to prevent: `Poodles → Animals` is not a link this
        // vocabulary states, so no repaired route may appear.
        assert!(
            !report.contains(
                "<http://example.org/poodles>  (\"Poodles\"@en) → <http://example.org/animals>"
            ),
            "a route is never shortened past the concept that was left out: {report}"
        );
    }

    /// The narrowing says what it cost, which `docs/adr/0043` made the whole safety of the flag.
    #[test]
    fn current_only_says_how_many_routes_it_withheld() {
        let (_directory, store) = store_with(RETIRED_ROUTE);
        let report = paths(&store, VOCABULARY, "http://example.org/poodles", true)
            .expect("poodles is in the vocabulary");

        assert!(
            report.contains(
                "1 more route(s) up from it run through a retired concept and are not shown"
            ),
            "{report}"
        );
        assert!(
            report.contains("without --current to see them"),
            "the sentence that gets them back: {report}"
        );
        // The unnarrowed report's own section counts the retired concepts *on the routes shown*,
        // and under the flag no shown route has one, so it must not also be printed.
        assert!(
            !report.contains("concept(s) on these routes are retired:"),
            "{report}"
        );
    }

    /// A diamond whose every way up runs through a retired concept.
    const EVERY_ROUTE_RETIRED: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix owl: <http://www.w3.org/2002/07/owl#> .
        @prefix ex: <http://example.org/> .
        ex:poodles a skos:Concept ; skos:prefLabel "Poodles"@en ;
            skos:broader ex:dogs, ex:pets .
        ex:dogs a skos:Concept ; skos:prefLabel "Dogs"@en ; skos:broader ex:animals ;
            owl:deprecated true .
        ex:pets a skos:Concept ; skos:prefLabel "Pets"@en ; skos:broader ex:animals ;
            owl:deprecated true .
        ex:animals a skos:Concept ; skos:prefLabel "Animals"@en .
    "#;

    /// **The case the counts exist for** (`docs/adr/0045` §4). Every route is withheld, so the
    /// list is empty — and the sentence an empty list otherwise gets blames a cycle. There is no
    /// cycle in this vocabulary; every way up is intact and obsolete, which is the opposite
    /// diagnosis and the opposite remedy.
    #[test]
    fn paths_current_only_never_blames_a_cycle_for_a_withheld_route() {
        let (_directory, store) = store_with(EVERY_ROUTE_RETIRED);
        let report = paths(&store, VOCABULARY, "http://example.org/poodles", true)
            .expect("poodles is in the vocabulary");

        assert!(
            report.contains("no route from it is current the whole way up: all 2 route(s)"),
            "{report}"
        );
        assert!(
            !report.contains("every way up runs into a cycle"),
            "there is no cycle here and the report must not invent one: {report}"
        );
        assert!(report.contains("without --current to see them"), "{report}");
        assert!(
            !report.contains("concept(s) the routes stop at"),
            "no route is offered, so nothing is where one stops: {report}"
        );
    }

    /// A hierarchy whose only way up is a loop, with a retired concept in the loop.
    const RETIRED_IN_A_CYCLE: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix owl: <http://www.w3.org/2002/07/owl#> .
        @prefix ex: <http://example.org/> .
        ex:leaf a skos:Concept ; skos:prefLabel "Leaf"@en ; skos:broader ex:a .
        ex:a a skos:Concept ; skos:prefLabel "A"@en ; skos:broader ex:b ; owl:deprecated true .
        ex:b a skos:Concept ; skos:prefLabel "B"@en ; skos:broader ex:a .
    "#;

    /// `docs/adr/0045` §3. A cycle is not a route on offer; it is why a route reaches no summit.
    /// Withholding it because a retired concept lies in it would delete the explanation and leave
    /// the problem, so the section is never narrowed — and says so, because a reader told the
    /// retired concepts are out would otherwise read one here as the flag failing.
    #[test]
    fn current_only_never_narrows_the_cycles() {
        let (_directory, store) = store_with(RETIRED_IN_A_CYCLE);
        let report = paths(&store, VOCABULARY, "http://example.org/leaf", true)
            .expect("leaf is in the vocabulary");

        assert!(report.contains("1 cycle(s) in the hierarchy"), "{report}");
        assert!(
            report.contains("<http://example.org/a>"),
            "the retired concept in the loop is named, because the loop runs through it: {report}"
        );
        assert!(
            report.contains("--current does not narrow this list"),
            "{report}"
        );
    }

    /// The concept the report is *about* is never filtered — the reader named it — and its own
    /// status says nothing about the routes above it.
    #[test]
    fn the_concept_asked_about_is_shown_whatever_its_status() {
        let (_directory, store) = store_with(RETIRED_ROUTE);
        let report = paths(&store, VOCABULARY, "http://example.org/dogs", true)
            .expect("dogs is in the vocabulary");

        assert!(
            report.contains("it is shown whatever its status because you asked about it by name"),
            "{report}"
        );
        assert!(
            report.contains("1 current route(s) up to a concept"),
            "its own retirement withholds no route above it: {report}"
        );
    }

    /// A vocabulary that retires nothing reads the same either way, so nobody pays for a feature
    /// their vocabulary does not use.
    ///
    /// Asserted as *the whole report below the banner*, not as the absence of a phrase: the
    /// banner is printed whenever the flag is typed, and nothing else may move.
    #[test]
    fn current_only_on_a_vocabulary_with_no_retirements_reads_identically() {
        let (_directory, store) = store_with(DIAMOND);
        let narrowed = paths(&store, VOCABULARY, "http://example.org/poodles", true)
            .expect("poodles is in the vocabulary");
        let full = paths(&store, VOCABULARY, "http://example.org/poodles", false)
            .expect("poodles is in the vocabulary");

        let banner = narrowed
            .lines()
            .find(|line| line.starts_with("current concepts only:"))
            .expect("the flag announces itself");
        assert_eq!(
            narrowed.replace(&format!("{banner}\n"), ""),
            full.replace(
                "route(s) up to a concept",
                "current route(s) up to a concept"
            ),
            "when nothing is retired only the banner and the word in the count may differ"
        );
        assert!(!narrowed.contains("not shown"), "{narrowed}");
    }
}
