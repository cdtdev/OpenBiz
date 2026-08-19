//! `openbiz tree` — print what is below one concept and what sits beside it, and say why.
//!
//! `openbiz ancestors` walks the hierarchy upwards; this is the other two directions a tree view
//! reads. It is the half of the concept tree an author spends their day in: a taxonomist opening
//! a vocabulary looks *down* from a top concept far more often than up from a leaf, and until
//! this existed the build could not answer "what is under Animals" at all.
//!
//! # Three questions, and the second is not the specification's
//!
//! - **Children** — one `skos:narrower` link down. What the graph *states* is directly below.
//! - **Siblings** — the concepts sharing a broader concept. Not a SKOS notion; the definition and
//!   its three deliberate limits are in [`openbiz_skos::Siblings`], and this report labels it as
//!   ours rather than letting it read as a specification term.
//! - **Everything below** — `skos:narrowerTransitive` walked under S24, printed as an indented
//!   tree.
//!
//! # Why the indentation is the derivation
//!
//! `CLAUDE.md` §3 requires every inference to explain itself, and for a transitive conclusion the
//! explanation is the path — which is what `openbiz ancestors` prints per ancestor. A subtree is
//! different in shape rather than in kind: printing the whole path against every one of a
//! thousand descendants repeats each prefix once per leaf under it, and the result is unreadable
//! for exactly the reason the tree is readable. So the path is printed **once, as structure**, and
//! each concept that is a transitive conclusion rather than a stated link is marked. Nothing is
//! withheld: [`Descent::derivation_to`] still renders the full chain for any single descendant,
//! and `openbiz ancestors` prints it that way from the other end.
//!
//! # Why a command and not an endpoint
//!
//! As with `openbiz inspect`, `ancestors`, `notes` and `mappings`, and not the authentication
//! objection: this only reads. The interface's concept tree is Phase 3's item, and an endpoint
//! now would be a caller with nothing behind it.

use std::collections::{BTreeMap, BTreeSet};

use openbiz_skos::{
    CoreModel, Descent, Node, RelationOrigin, Resource, SemanticRelation, Siblings, WalkBound,
};
use openbiz_store::Store;

use crate::cli::CommandError;

/// Report what is below `concept` in the vocabulary at `graph`, what is beside it, and why.
///
/// Reads and nothing else.
///
/// A concept the vocabulary never mentions is **refused**, exactly as `openbiz ancestors` refuses
/// one. A leaf and a typo produce the same empty answer and mean opposite things, and at a
/// command line the typo is the likelier of the two.
pub fn tree(store: &Store, graph: &str, concept: &str) -> Result<String, CommandError> {
    let model = crate::inspect::read(store, graph)?;

    let node = Node::iri(concept);
    if model.resource(&node).is_none() {
        return Err(CommandError::NoSuchConcept {
            concept: concept.to_owned(),
            graph: graph.to_owned(),
        });
    }

    let below = model.descent(&node, WalkBound::DEFAULT);
    let beside = model.siblings(&node, WalkBound::DEFAULT);
    Ok(report(graph, &node, &model, &below, &beside))
}

/// The report itself, kept apart from the store so it can be tested against a model in hand.
fn report(
    graph: &str,
    concept: &Node,
    model: &CoreModel,
    below: &Descent,
    beside: &Siblings,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("{concept}{}\n", named_in(model, concept)));
    out.push_str(&format!("in {graph}\n"));

    children_section(&mut out, model, concept, below);
    siblings_section(&mut out, model, beside);
    descent_section(&mut out, model, concept, below);

    out
}

/// What the graph states is directly below — and what is below it that this is *not*.
fn children_section(out: &mut String, model: &CoreModel, concept: &Node, below: &Descent) {
    let children: Vec<_> = model.children(concept).collect();
    if children.is_empty() {
        out.push_str(
            "\nno concept is a child of it: nothing is one skos:narrower link below. SKOS states \
             no condition requiring one.\n",
        );
    } else {
        out.push_str(&format!(
            "\n{} child concept(s), one skos:narrower link below:\n",
            children.len()
        ));
        for (child, origin) in &children {
            out.push_str(&format!("  {child}{}\n", named_in(model, child)));
            // Only an entailed link explains itself; the graph speaks for the ones it states, and
            // a line of "asserted" against every child would bury the ones it did not.
            if let RelationOrigin::Entailed(rule) = origin {
                out.push_str("    inferred, not stated\n");
                out.push_str(&format!("    and {rule}\n"));
            }
        }
    }

    // The module's central asymmetry, reported only when this vocabulary actually shows it.
    // S22 makes skos:narrower a sub-property of skos:narrowerTransitive and not the reverse, so a
    // concept the graph puts below this one *transitively* is a descendant with no stated place
    // in the tree — and a reader counting the first level of the tree against the children above
    // would otherwise find two different numbers and no explanation.
    let stated: BTreeSet<&Node> = children.iter().map(|(child, _)| *child).collect();
    let unstated: Vec<&Node> = below
        .steps()
        .filter(|(_, from)| *from == concept)
        .map(|(node, _)| node)
        .filter(|node| !stated.contains(node))
        .collect();
    if !unstated.is_empty() {
        out.push_str(&format!(
            "\n{} concept(s) are one skos:narrowerTransitive link below it without being \
             children:\n",
            unstated.len()
        ));
        for node in unstated {
            out.push_str(&format!("  {node}{}\n", named_in(model, node)));
        }
        out.push_str(
            "  S22 makes skos:narrower a sub-property of skos:narrowerTransitive and not the \
             other way round, so the graph placing a concept below this one transitively does \
             not state that it is a child.\n",
        );
    }
}

/// What sits beside it — our query, labelled as ours.
fn siblings_section(out: &mut String, model: &CoreModel, beside: &Siblings) {
    if beside.is_empty() {
        out.push_str(if beside.parents() == 0 {
            "\nit has no broader concept, so it has no siblings: a sibling here means a concept \
             sharing a broader concept, and two concepts sharing none are not related by it.\n"
        } else {
            "\nno other concept shares a broader concept with it.\n"
        });
    } else {
        out.push_str(&format!(
            "\n{} concept(s) share a broader concept with it — which is our term and not one SKOS \
             states:\n",
            beside.len()
        ));
        for sibling in beside.siblings() {
            out.push_str(&format!("  {sibling}{}\n", named_in(model, sibling)));
            if let Some(shared) = beside.through(sibling) {
                for parent in shared {
                    out.push_str(&format!("    both are under {parent}\n"));
                }
            }
        }
    }

    if !beside.is_complete() {
        out.push_str(
            "  the search for siblings stopped at its bound, so this list is a lower bound and a \
             concept missing from it may still share a broader concept.\n",
        );
    }
}

/// Everything below, as the indented tree the walk already produced.
fn descent_section(out: &mut String, model: &CoreModel, concept: &Node, below: &Descent) {
    if below.is_empty() {
        out.push_str(if below.is_complete() {
            "\nnothing is below it: it has no narrower concept, directly or transitively.\n"
        } else {
            "\nnothing was reached before the walk hit its bound, so this is not an answer.\n"
        });
        return;
    }

    out.push_str(&format!(
        "\n{} concept(s) are below it, by {} link(s) walked:\n",
        below.len(),
        below.links_walked()
    ));

    // The predecessor list turned round. Each concept is reached from exactly one other, so this
    // is a tree — with one exception, handled below: a cycle puts the origin back among the
    // descendants, and the origin is also the root.
    let mut branches: BTreeMap<&Node, Vec<&Node>> = BTreeMap::new();
    for (node, from) in below.steps() {
        branches.entry(from).or_default().push(node);
    }

    // An explicit stack rather than recursion. A 100 000-link chain is a legal SKOS graph (§8
    // states no condition on depth) and recursing down one would overflow the stack, which is a
    // crash where the bound is meant to produce an honest incomplete answer.
    let mut printed: BTreeSet<&Node> = BTreeSet::from([concept]);
    let mut stack: Vec<(&Node, usize)> = Vec::new();
    let mut transitive = false;
    push_branch(&mut stack, branches.get(concept), 1);

    while let Some((node, depth)) = stack.pop() {
        let indent = "  ".repeat(depth);
        if !printed.insert(node) {
            // Only the origin can arrive twice, and only through a cycle — which §8.6.8 says is
            // consistent. Printed rather than skipped: a tree that silently stopped there would
            // hide the one structural fact an author most needs to see.
            out.push_str(&format!(
                "{indent}{node}{} — the hierarchy comes back round to the concept asked about\n",
                named_in(model, node)
            ));
            continue;
        }
        transitive |= depth > 1;
        out.push_str(&format!(
            "{indent}{node}{}{}\n",
            named_in(model, node),
            if depth > 1 { "  [S24]" } else { "" }
        ));
        push_branch(&mut stack, branches.get(node), depth + 1);
    }

    out.push_str(
        "\nthe indentation is the path: a concept shown under another is skos:narrowerTransitive \
         to it",
    );
    out.push_str(if transitive {
        ", and one marked [S24] is a conclusion that transitivity licensed rather than a \
         link the graph states.\n"
    } else {
        ".\n"
    });

    polyhierarchy_note(out, model, concept, below);

    if below.is_complete() {
        out.push_str("that is every concept below it.\n");
    } else {
        out.push_str(
            "the walk stopped at its bound before reaching the bottom, so this tree is a lower \
             bound and not the answer.\n",
        );
    }
}

/// Name the concepts the tree shape cannot show, rather than letting the shape imply they are not
/// there.
///
/// A tree gives each concept one parent and the walk is breadth-first, so a concept below the
/// origin by two routes is printed once, under the shorter. **That is a property of the rendering
/// and not of the vocabulary**, and it is the one place where this report's shape says something
/// the graph does not: a reader seeing Buildings under Property alone would conclude it is not
/// also under Vehicles.
///
/// Polyhierarchy is ordinary in a thesaurus, §8 states nothing against it, and ISO 25964 relies on
/// it — so this is not a finding and is not phrased as one. It is the count and the missing links,
/// stated where the tree would otherwise quietly drop them.
fn polyhierarchy_note(out: &mut String, model: &CoreModel, concept: &Node, below: &Descent) {
    // The tree's edges run down `skos:narrowerTransitive`, so the routes into a concept are its
    // one-step `skos:broaderTransitive` links — restricted to what this subtree actually holds,
    // because a broader concept outside it is not a route the tree could have shown.
    let within = |node: &Node| below.contains(node) || node == concept;

    let mut elsewhere: Vec<(&Node, Vec<&Node>)> = Vec::new();
    for (node, shown) in below.steps() {
        let Some(routes) = model
            .resource(node)
            .and_then(|resource| resource.relations(SemanticRelation::BroaderTransitive))
        else {
            continue;
        };
        let others: Vec<&Node> = routes
            .keys()
            .filter(|route| *route != shown && within(route))
            .collect();
        if !others.is_empty() {
            elsewhere.push((node, others));
        }
    }

    if elsewhere.is_empty() {
        return;
    }

    out.push_str(&format!(
        "\n{} concept(s) below it are under more than one concept in this subtree, and a tree can \
         show each once. Also below:\n",
        elsewhere.len()
    ));
    for (node, others) in elsewhere {
        for other in others {
            out.push_str(&format!(
                "  {node}{} is also below {other}{}\n",
                named_in(model, node),
                named_in(model, other)
            ));
        }
    }
    out.push_str(
        "  polyhierarchy is ordinary in a thesaurus and SKOS states no condition against it; this \
         is what the tree's shape cannot show, not a defect in the vocabulary.\n",
    );
}

/// Push one concept's branch onto the stack so it pops in the order the model holds it.
fn push_branch<'a>(
    stack: &mut Vec<(&'a Node, usize)>,
    branch: Option<&Vec<&'a Node>>,
    depth: usize,
) {
    let Some(branch) = branch else {
        return;
    };
    for node in branch.iter().rev() {
        stack.push((node, depth));
    }
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

    use super::tree;
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

    const ANIMALS: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix ex: <http://example.org/> .
        ex:animals a skos:Concept ; skos:prefLabel "Animals"@en .
        ex:mammals a skos:Concept ; skos:prefLabel "Mammals"@en ; skos:broader ex:animals .
        ex:birds a skos:Concept ; skos:prefLabel "Birds"@en ; skos:broader ex:animals .
        ex:cats a skos:Concept ; skos:prefLabel "Cats"@en ; skos:broader ex:mammals .
    "#;

    /// The whole point of the command: the subtree, indented, with the concepts that are
    /// transitive conclusions marked as such and the one-step ones not.
    #[test]
    fn the_tree_prints_the_subtree_and_marks_what_transitivity_licensed() {
        let (_directory, store) = store_with(ANIMALS);
        let report =
            tree(&store, VOCABULARY, "http://example.org/animals").expect("animals is a concept");

        assert!(report.contains("2 child concept(s)"), "{report}");
        assert!(report.contains("3 concept(s) are below it"), "{report}");
        // Cats is two steps down, so it is indented twice and is S24's conclusion.
        assert!(
            report.contains("    <http://example.org/cats>  (\"Cats\"@en)  [S24]"),
            "{report}"
        );
        // Mammals is one step down, which is S22's conclusion or the graph's own — never S24's.
        assert!(
            report.contains("  <http://example.org/mammals>  (\"Mammals\"@en)\n"),
            "{report}"
        );
        assert!(
            !report.contains("<http://example.org/mammals>  (\"Mammals\"@en)  [S24]"),
            "a one-step link is not a transitive conclusion: {report}"
        );
        assert!(
            report.contains("that is every concept below it."),
            "{report}"
        );
    }

    /// Children are the stated hierarchy. The ones S25 turned round from `skos:broader` say so,
    /// rather than reading as something the author wrote.
    #[test]
    fn a_child_reached_through_the_inverse_says_which_rule_reached_it() {
        let (_directory, store) = store_with(ANIMALS);
        let report =
            tree(&store, VOCABULARY, "http://example.org/animals").expect("animals is a concept");

        assert!(report.contains("inferred, not stated"), "{report}");
        assert!(
            report.contains("S25: skos:narrower is owl:inverseOf the property skos:broader."),
            "the specification statement is quoted, not just cited: {report}"
        );
    }

    /// Siblings, labelled as our term and reduced to the concept they are shared through.
    #[test]
    fn siblings_name_the_broader_concept_they_are_shared_through() {
        let (_directory, store) = store_with(ANIMALS);
        let report =
            tree(&store, VOCABULARY, "http://example.org/birds").expect("birds is a concept");

        assert!(
            report.contains("which is our term and not one SKOS states"),
            "{report}"
        );
        assert!(report.contains("<http://example.org/mammals>"), "{report}");
        assert!(
            report.contains("    both are under <http://example.org/animals>"),
            "{report}"
        );
    }

    /// A leaf has nothing below it and a top concept has no siblings. Neither is a defect — SKOS
    /// states no condition requiring either — and the report says so rather than printing an
    /// empty heading a reader would read as a gap.
    #[test]
    fn a_leaf_and_a_top_concept_are_both_legal_and_reported_as_such() {
        let (_directory, store) = store_with(ANIMALS);

        let leaf = tree(&store, VOCABULARY, "http://example.org/cats").expect("cats is a concept");
        assert!(leaf.contains("nothing is below it"), "{leaf}");
        assert!(leaf.contains("no concept is a child of it"), "{leaf}");
        assert!(
            leaf.contains("no other concept shares a broader concept with it"),
            "cats is its parent's only child, and having none is not having no parent: {leaf}"
        );

        let top =
            tree(&store, VOCABULARY, "http://example.org/animals").expect("animals is a concept");
        assert!(
            top.contains("it has no broader concept, so it has no siblings"),
            "{top}"
        );
    }

    /// §8.6.8's Example 37 — a cycle is consistent SKOS. The report must terminate and must show
    /// that the hierarchy comes back to where it started rather than quietly stopping there.
    #[test]
    fn a_cycle_is_printed_rather_than_silently_cut() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:a a skos:Concept ; skos:broader ex:b .
            ex:b a skos:Concept ; skos:broader ex:a .
            "#,
        );

        let report = tree(&store, VOCABULARY, "http://example.org/a").expect("a is a concept");
        assert!(
            report.contains("the hierarchy comes back round to the concept asked about"),
            "{report}"
        );
        assert!(report.contains("2 concept(s) are below it"), "{report}");
    }

    /// A concept placed below another only transitively is a descendant and **not** a child, and
    /// the report names the rule that makes it so rather than leaving two counts to disagree in
    /// silence.
    #[test]
    fn a_transitive_link_is_reported_as_below_but_not_as_a_child() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:a a skos:Concept ; skos:narrowerTransitive ex:b .
            ex:b a skos:Concept .
            "#,
        );

        let report = tree(&store, VOCABULARY, "http://example.org/a").expect("a is a concept");
        assert!(report.contains("no concept is a child of it"), "{report}");
        assert!(
            report.contains("1 concept(s) are one skos:narrowerTransitive link below it"),
            "{report}"
        );
        assert!(
            report.contains("S22 makes skos:narrower a sub-property"),
            "{report}"
        );
        assert!(report.contains("1 concept(s) are below it"), "{report}");
    }

    /// A concept below the origin by two routes is printed once, and the route the tree could not
    /// show is named rather than dropped. The shape of a tree is the one place this report can
    /// imply something the graph does not, so it says what it left out.
    #[test]
    fn a_second_route_into_a_concept_is_named_rather_than_dropped() {
        let (_directory, store) = store_with(
            r#"
            @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
            @prefix ex: <http://example.org/> .
            ex:assets a skos:Concept ; skos:prefLabel "Assets"@en .
            ex:property a skos:Concept ; skos:prefLabel "Property"@en ; skos:broader ex:assets .
            ex:vehicles a skos:Concept ; skos:prefLabel "Vehicles"@en ; skos:broader ex:assets .
            ex:buildings a skos:Concept ; skos:prefLabel "Buildings"@en ;
                skos:broader ex:property , ex:vehicles .
            "#,
        );

        let report =
            tree(&store, VOCABULARY, "http://example.org/assets").expect("assets is a concept");

        // Once in the tree, under one of its two parents.
        assert_eq!(
            report.matches("<http://example.org/buildings>").count(),
            2,
            "once in the tree and once in the note that names the other route: {report}"
        );
        assert!(
            report
                .contains("1 concept(s) below it are under more than one concept in this subtree"),
            "{report}"
        );
        assert!(
            report.contains(
                "<http://example.org/buildings>  (\"Buildings\"@en) is also below \
                 <http://example.org/vehicles>  (\"Vehicles\"@en)"
            ),
            "{report}"
        );
        assert!(
            report.contains("polyhierarchy is ordinary in a thesaurus"),
            "it is not phrased as a defect: {report}"
        );
    }

    /// The [S24] legend is printed only when something is marked with it. A subtree one level deep
    /// has no transitive conclusion in it, and explaining a marker that is not on the page reads
    /// as though the reader missed one.
    #[test]
    fn the_transitivity_legend_is_printed_only_when_something_carries_the_mark() {
        let (_directory, store) = store_with(ANIMALS);

        let deep =
            tree(&store, VOCABULARY, "http://example.org/animals").expect("animals is a concept");
        assert!(
            deep.contains("[S24] is a conclusion that transitivity licensed"),
            "{deep}"
        );

        let shallow =
            tree(&store, VOCABULARY, "http://example.org/mammals").expect("mammals is a concept");
        assert!(shallow.contains("the indentation is the path"), "{shallow}");
        assert!(
            !shallow.contains("[S24]"),
            "nothing in this subtree is a transitive conclusion: {shallow}"
        );
    }

    /// A concept the vocabulary never mentions is refused, not answered with an empty tree. A
    /// leaf and a typo look identical and mean opposite things.
    #[test]
    fn a_concept_the_vocabulary_never_mentions_is_refused() {
        let (_directory, store) = store_with(ANIMALS);

        assert!(matches!(
            tree(&store, VOCABULARY, "http://example.org/typo"),
            Err(CommandError::NoSuchConcept { .. })
        ));
    }
}
