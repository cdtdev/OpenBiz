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
    CoreModel, Descent, Node, Pruned, RelationOrigin, Resource, Retirements, SemanticRelation,
    Siblings, WalkBound,
};
use openbiz_store::Store;

use crate::cli::CommandError;
use crate::status;

/// Report what is below `concept` in the vocabulary at `graph`, what is beside it, and why.
///
/// Reads and nothing else.
///
/// A concept the vocabulary never mentions is **refused**, exactly as `openbiz ancestors` refuses
/// one. A leaf and a typo produce the same empty answer and mean opposite things, and at a
/// command line the typo is the likelier of the two.
pub fn tree(
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

    let below = model.descent(&node, WalkBound::DEFAULT);
    let beside = model.siblings(&node, WalkBound::DEFAULT);
    Ok(report(
        graph,
        &node,
        &model,
        &retirements,
        &below,
        &beside,
        current_only,
    ))
}

/// The report itself, kept apart from the store so it can be tested against a model in hand.
fn report(
    graph: &str,
    concept: &Node,
    model: &CoreModel,
    retirements: &Retirements,
    below: &Descent,
    beside: &Siblings,
    current_only: bool,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{concept}{}{}\n",
        named_in(model, concept),
        status::mark(retirements, concept)
    ));
    out.push_str(&format!("in {graph}\n"));
    // Said before any count, because every number under it is a number about a narrowed report
    // and a reader who learns that at the bottom has already read them as totals. The second
    // sentence is the whole difference between this flag and the one on `openbiz search`.
    if current_only {
        out.push_str(
            "current concepts only: the vocabulary's retired concepts are left out. A retired \
             concept with current concepts below it is kept and marked instead of dropped — \
             taking it out would take them with it, and nothing here is lifted or re-parented.\n",
        );
    }
    status::explain(&mut out, "", retirements, model, concept);
    // The concept the report is *about* is never the thing being filtered: the reader named it.
    if current_only && retirements.is_retired(concept) {
        out.push_str(
            "it is shown whatever its status because you asked about it by name; --current \
             applies to what is below and beside it.\n",
        );
    }
    retirement_below(&mut out, retirements, concept, below);

    // Built here and handed over as a set of nodes: `owl:deprecated` is not SKOS, so which
    // resources to leave out is a status question answered beside the model (`docs/adr/0041`).
    // Empty when the flag is off, which makes the pruning show everything.
    let skip = match current_only {
        true => status::retired_in(retirements),
        false => BTreeSet::new(),
    };
    let pruned = below.excluding(&skip);

    children_section(&mut out, model, retirements, concept, below, current_only);
    siblings_section(&mut out, model, retirements, beside, current_only);
    descent_section(
        &mut out,
        model,
        retirements,
        concept,
        below,
        &pruned,
        current_only,
    );

    out
}

/// What a retirement left standing underneath it, said before the tree rather than after.
///
/// `openbiz deprecate` retires a concept and deliberately does not touch what is below it — the
/// children may want re-parenting under the replacement, or retiring too, and nothing in the graph
/// says which (`docs/adr/0040`). That decision leaves the vocabulary in a state a tree view is the
/// first place anyone would notice and the last place anyone would look: current concepts hanging
/// under an obsolete parent. So it is counted here, at the top, and not left for the reader to
/// work out by scanning a subtree for the absence of a marker.
///
/// The count is of concepts **not** retired, because a retired branch retired all the way down is
/// a finished job and saying nothing about it is right.
fn retirement_below(out: &mut String, retirements: &Retirements, concept: &Node, below: &Descent) {
    if !retirements.is_retired(concept) || below.is_empty() {
        return;
    }

    let current = below
        .steps()
        .filter(|(node, _)| !retirements.is_retired(node))
        .count();
    if current == 0 {
        out.push_str(&format!(
            "every one of the {} concept(s) below it is retired too.\n",
            below.len()
        ));
        return;
    }

    out.push_str(&format!(
        "{current} of the {} concept(s) below it are not retired, and are now under a concept \
         that is. Retiring a concept moves nothing: whether each of them should move under the \
         replacement, be retired too, or stay is a decision only a person can take.\n",
        below.len()
    ));
}

/// What the graph states is directly below — and what is below it that this is *not*.
fn children_section(
    out: &mut String,
    model: &CoreModel,
    retirements: &Retirements,
    concept: &Node,
    below: &Descent,
    current_only: bool,
) {
    let stated_children: Vec<(&Node, &RelationOrigin)> = model.children(concept).collect();
    let children: Vec<(&Node, &RelationOrigin)> = stated_children
        .iter()
        .copied()
        .filter(|(child, _)| !(current_only && retirements.is_retired(child)))
        .collect();
    let left_out = stated_children.len() - children.len();

    if stated_children.is_empty() {
        out.push_str(
            "\nno concept is a child of it: nothing is one skos:narrower link below. SKOS states \
             no condition requiring one.\n",
        );
    } else if children.is_empty() {
        // The list is a list and not a tree, so nothing is kept for structure — and an empty one
        // under a concept that *has* children would read as a leaf. `docs/adr/0043`'s rule: the
        // hits go, the fact that there were hits stays.
        out.push_str(&format!(
            "\nnone of its {} child concept(s) are listed: every one of them is retired and \
             --current was asked for.\n",
            stated_children.len()
        ));
    } else {
        out.push_str(&match left_out {
            0 => format!(
                "\n{} child concept(s), one skos:narrower link below:\n",
                children.len()
            ),
            _ => format!(
                "\n{} of its {} child concept(s), one skos:narrower link below:\n",
                children.len(),
                stated_children.len()
            ),
        });
        for (child, origin) in &children {
            out.push_str(&format!(
                "  {child}{}{}\n",
                named_in(model, child),
                status::mark(retirements, child)
            ));
            // Only an entailed link explains itself; the graph speaks for the ones it states, and
            // a line of "asserted" against every child would bury the ones it did not.
            if let RelationOrigin::Entailed(rule) = origin {
                out.push_str("    inferred, not stated\n");
                out.push_str(&format!("    and {rule}\n"));
            }
        }
        // Only under a list that showed something: the empty case above already gave the count,
        // and saying it twice reads as two different numbers.
        if left_out > 0 {
            out.push_str(&format!(
                "  {left_out} more child concept(s) are retired and not listed because --current \
                 was asked for.\n"
            ));
        }
    }

    // The module's central asymmetry, reported only when this vocabulary actually shows it.
    // S22 makes skos:narrower a sub-property of skos:narrowerTransitive and not the reverse, so a
    // concept the graph puts below this one *transitively* is a descendant with no stated place
    // in the tree — and a reader counting the first level of the tree against the children above
    // would otherwise find two different numbers and no explanation.
    let stated: BTreeSet<&Node> = stated_children.iter().map(|(child, _)| *child).collect();
    let all_unstated: Vec<&Node> = below
        .steps()
        .filter(|(_, from)| *from == concept)
        .map(|(node, _)| node)
        .filter(|node| !stated.contains(node))
        .collect();
    let unstated: Vec<&Node> = all_unstated
        .iter()
        .copied()
        .filter(|node| !(current_only && retirements.is_retired(node)))
        .collect();
    let unstated_left_out = all_unstated.len() - unstated.len();
    if !unstated.is_empty() {
        out.push_str(&format!(
            "\n{} concept(s) are one skos:narrowerTransitive link below it without being \
             children:\n",
            unstated.len()
        ));
        for node in unstated {
            out.push_str(&format!(
                "  {node}{}{}\n",
                named_in(model, node),
                status::mark(retirements, node)
            ));
        }
        out.push_str(
            "  S22 makes skos:narrower a sub-property of skos:narrowerTransitive and not the \
             other way round, so the graph placing a concept below this one transitively does \
             not state that it is a child.\n",
        );
    }
    if unstated_left_out > 0 {
        out.push_str(&format!(
            "\n{unstated_left_out} concept(s) one skos:narrowerTransitive link below it without \
             being children are retired and not listed because --current was asked for.\n"
        ));
    }
}

/// What sits beside it — our query, labelled as ours.
fn siblings_section(
    out: &mut String,
    model: &CoreModel,
    retirements: &Retirements,
    beside: &Siblings,
    current_only: bool,
) {
    let shown: Vec<&Node> = beside
        .siblings()
        .filter(|sibling| !(current_only && retirements.is_retired(sibling)))
        .collect();
    let left_out = beside.len() - shown.len();

    if beside.is_empty() {
        out.push_str(if beside.parents() == 0 {
            "\nit has no broader concept, so it has no siblings: a sibling here means a concept \
             sharing a broader concept, and two concepts sharing none are not related by it.\n"
        } else {
            "\nno other concept shares a broader concept with it.\n"
        });
    } else if shown.is_empty() {
        // As with the children: a sibling list is a list, so an empty one under a concept that
        // *has* siblings has to say which of the two it is.
        out.push_str(&format!(
            "\nnone of the {} concept(s) sharing a broader concept with it are listed: every one \
             of them is retired and --current was asked for.\n",
            beside.len()
        ));
    } else {
        out.push_str(&format!(
            "\n{} concept(s) share a broader concept with it — which is our term and not one SKOS \
             states:\n",
            shown.len()
        ));
        for sibling in shown {
            out.push_str(&format!(
                "  {sibling}{}{}\n",
                named_in(model, sibling),
                status::mark(retirements, sibling)
            ));
            if let Some(shared) = beside.through(sibling) {
                for parent in shared {
                    out.push_str(&format!("    both are under {parent}\n"));
                }
            }
        }
        if left_out > 0 {
            out.push_str(&format!(
                "  {left_out} more concept(s) sharing a broader concept with it are retired and \
                 not listed because --current was asked for.\n"
            ));
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
fn descent_section(
    out: &mut String,
    model: &CoreModel,
    retirements: &Retirements,
    concept: &Node,
    below: &Descent,
    pruned: &Pruned<'_>,
    current_only: bool,
) {
    if below.is_empty() {
        out.push_str(if below.is_complete() {
            "\nnothing is below it: it has no narrower concept, directly or transitively.\n"
        } else {
            "\nnothing was reached before the walk hit its bound, so this is not an answer.\n"
        });
        return;
    }

    // The case `--current` exists to be safe in. Everything below is retired, so the tree is
    // empty — and an empty tree under a concept that *has* a subtree reads as a leaf, which is
    // the false negative `docs/adr/0041` refused to ship. The count, and the way back, instead.
    if pruned.kept() == 0 {
        out.push_str(&format!(
            "\nnothing below it is current: all {} concept(s) below it are retired, and \
             --current was asked for. They are in this vocabulary: run the same command without \
             --current to see them and what each one says to use instead.\n",
            below.len()
        ));
        if !below.is_complete() {
            out.push_str(
                "the walk stopped at its bound before reaching the bottom, so a current concept \
                 may still be below it.\n",
            );
        }
        return;
    }

    out.push_str(&format!(
        "\n{} {}concept(s) are below it, by {} link(s) walked{}:\n",
        pruned.kept(),
        match current_only {
            true => "current ",
            false => "",
        },
        below.links_walked(),
        match pruned.routes() {
            0 => String::new(),
            routes => format!(", under {routes} retired concept(s) kept as the route to them"),
        }
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
        // A concept the pruning dropped takes its whole branch with it, and that is not a
        // rendering shortcut: `Descent::excluding` keeps as a route every excluded concept lying
        // between the origin and a survivor, so anything dropped has nothing kept below it.
        if !pruned.shows(node) {
            continue;
        }
        let indent = "  ".repeat(depth);
        if !printed.insert(node) {
            // Only the origin can arrive twice, and only through a cycle — which §8.6.8 says is
            // consistent. Printed rather than skipped: a tree that silently stopped there would
            // hide the one structural fact an author most needs to see.
            out.push_str(&format!(
                "{indent}{node}{}{} — the hierarchy comes back round to the concept asked about\n",
                named_in(model, node),
                mark_in_tree(retirements, pruned, node)
            ));
            continue;
        }
        transitive |= depth > 1;
        out.push_str(&format!(
            "{indent}{node}{}{}{}\n",
            named_in(model, node),
            mark_in_tree(retirements, pruned, node),
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

    polyhierarchy_note(out, model, concept, below, pruned);

    if below.is_complete() {
        out.push_str(match current_only {
            true => "that is every current concept below it.\n",
            false => "that is every concept below it.\n",
        });
    } else {
        out.push_str(
            "the walk stopped at its bound before reaching the bottom, so this tree is a lower \
             bound and not the answer.\n",
        );
    }
    withheld_note(out, pruned);
}

/// A concept's mark inside the tree, which is the ordinary one unless the pruning kept it.
///
/// An excluded concept appearing unremarked in a report that was asked to leave those out reads
/// as a defect in the flag. `[retired, kept as the route to what is below]` says why it is there,
/// and in saying it shows the decision `docs/adr/0040` leaves pending.
fn mark_in_tree(retirements: &Retirements, pruned: &Pruned<'_>, node: &Node) -> &'static str {
    match pruned.is_route(node) {
        true => status::ROUTE,
        false => status::mark(retirements, node),
    }
}

/// What `--current` left out of the tree, always said, and never said as nothing.
///
/// The rule `docs/adr/0043` set for `openbiz search` and this command inherits: **the concepts go,
/// the fact that there were concepts stays.** A curator narrowing a tree to plan a new branch is
/// the person most likely to create a duplicate of a term the vocabulary already holds, so the
/// report ends by saying how many it did not show and how to get them back.
///
/// Nothing is printed when nothing was withheld, which is every tree in the overwhelming majority
/// of vocabularies: they retire nothing.
fn withheld_note(out: &mut String, pruned: &Pruned<'_>) {
    if pruned.routes() > 0 {
        out.push_str(&format!(
            "\n{} retired concept(s) are shown, marked, because current concepts sit below them. \
             Retiring a concept moves nothing: whether each of those should move under the \
             replacement, be retired too, or stay is a decision only a person can take.\n",
            pruned.routes()
        ));
    }
    if pruned.dropped() > 0 {
        out.push_str(&format!(
            "\n{} retired concept(s) below it are not shown, with nothing current under them. \
             They are in this vocabulary: run the same command without --current to see them and \
             what each one says to use instead.\n",
            pruned.dropped()
        ));
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
fn polyhierarchy_note(
    out: &mut String,
    model: &CoreModel,
    concept: &Node,
    below: &Descent,
    pruned: &Pruned<'_>,
) {
    // The tree's edges run down `skos:narrowerTransitive`, so the routes into a concept are its
    // one-step `skos:broaderTransitive` links — restricted to what this subtree actually holds,
    // because a broader concept outside it is not a route the tree could have shown. Under
    // `--current` that restriction tightens to what the tree actually *printed*: naming a route
    // through a concept the report just left out would put it back, one line further down.
    let within = |node: &Node| (pruned.shows(node) && below.contains(node)) || node == concept;

    let mut elsewhere: Vec<(&Node, Vec<&Node>)> = Vec::new();
    for (node, shown) in below.steps() {
        if !pruned.shows(node) {
            continue;
        }
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

    /// A thesaurus with a retirement in the middle of the hierarchy — the commonest outcome of a
    /// retirement (`docs/adr/0040`), and the case `--current` had to be designed around. `Wireless`
    /// is retired with a live child under it; `Spark` is retired with nothing under it at all.
    const RETIRED: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix owl: <http://www.w3.org/2002/07/owl#> .
        @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
        @prefix ex: <http://example.org/> .
        ex:telegraphy a skos:Concept ; skos:prefLabel "Telegraphy"@en .
        ex:radio a skos:Concept ; skos:prefLabel "Radio"@en ; skos:broader ex:telegraphy .
        ex:wireless a skos:Concept ; skos:prefLabel "Wireless"@en ; skos:broader ex:telegraphy ;
            owl:deprecated "true"^^xsd:boolean .
        ex:morse a skos:Concept ; skos:prefLabel "Morse"@en ; skos:broader ex:wireless .
        ex:spark a skos:Concept ; skos:prefLabel "Spark"@en ; skos:broader ex:wireless ;
            owl:deprecated "true"^^xsd:boolean .
    "#;

    /// **The decision the item turned on.** A retired concept with a current concept below it is
    /// kept and marked rather than dropped, because dropping it would take the child with it and
    /// lifting the child would put a link in the tree the vocabulary does not state. A retired
    /// concept with nothing current below it goes, and is counted.
    #[test]
    fn a_retired_concept_on_the_route_to_a_current_one_is_kept_and_marked() {
        let (_directory, store) = store_with(RETIRED);
        let report = tree(&store, VOCABULARY, "http://example.org/telegraphy", true)
            .expect("telegraphy is a concept");

        assert!(
            report.contains(
                "<http://example.org/wireless>  (\"Wireless\"@en)  [retired, kept as \
                             the route to what is below]"
            ),
            "{report}"
        );
        assert!(
            report.contains("    <http://example.org/morse>  (\"Morse\"@en)  [S24]"),
            "the child keeps the depth and the derivation the unpruned tree gave it: {report}"
        );
        assert!(
            !report.contains("<http://example.org/spark>"),
            "retired with nothing current below it, so it goes: {report}"
        );
        assert!(
            report.contains("2 current concept(s) are below it"),
            "the count is of what was asked for: {report}"
        );
        assert!(
            report.contains("1 retired concept(s) below it are not shown"),
            "and the fact that there were more stays: {report}"
        );
    }

    /// Nothing is lifted. The same tree without the flag has to place `Morse` in exactly the same
    /// position, which is what says the pruning removed concepts rather than rearranging them.
    #[test]
    fn the_flag_removes_concepts_and_never_moves_them() {
        let (_directory, store) = store_with(RETIRED);
        let shown = tree(&store, VOCABULARY, "http://example.org/telegraphy", false)
            .expect("telegraphy is a concept");
        let narrowed = tree(&store, VOCABULARY, "http://example.org/telegraphy", true)
            .expect("telegraphy is a concept");

        let indent_of = |report: &str, iri: &str| -> usize {
            report
                .lines()
                .find(|line| line.trim_start().starts_with(iri))
                .map(|line| line.len() - line.trim_start().len())
                .expect("the concept is in the tree")
        };
        assert_eq!(
            indent_of(&shown, "<http://example.org/morse>"),
            indent_of(&narrowed, "<http://example.org/morse>"),
            "narrowed:\n{narrowed}"
        );
    }

    /// **The failure the flag would otherwise reintroduce**, and the tree's version of the one
    /// `docs/adr/0043` pins for `openbiz search`. Everything below is retired, so the tree is
    /// empty — and an empty tree under a concept that has a subtree reads as a leaf, which is a
    /// term the vocabulary holds reported as one it has never heard of.
    #[test]
    fn a_subtree_that_is_retired_all_the_way_down_is_still_counted() {
        let (_directory, store) = store_with(RETIRED);
        let report = tree(&store, VOCABULARY, "http://example.org/wireless", true)
            .expect("wireless is a concept");
        let all_retired =
            tree(&store, VOCABULARY, "http://example.org/spark", true).expect("spark is a concept");

        // Wireless still has Morse under it, so it is not the empty case.
        assert!(
            report.contains("1 current concept(s) are below it"),
            "{report}"
        );
        assert!(
            report.contains("1 retired concept(s) below it are not shown"),
            "{report}"
        );
        // Spark has nothing below it at all, which is a different sentence from a subtree the
        // flag emptied — and the two must not be blurred.
        assert!(
            all_retired.contains("nothing is below it: it has no narrower concept"),
            "{all_retired}"
        );
        assert!(
            !all_retired.contains("nothing below it is current"),
            "a leaf is not an emptied subtree: {all_retired}"
        );
    }

    /// The concept the report is *about* is never filtered: the reader named it. Hiding it, or
    /// refusing the combination, would be a command that disobeys the arguments it was given.
    #[test]
    fn the_concept_asked_about_is_shown_whatever_its_status() {
        let (_directory, store) = store_with(RETIRED);
        let report =
            tree(&store, VOCABULARY, "http://example.org/spark", true).expect("spark is a concept");

        assert!(report.contains("<http://example.org/spark>"), "{report}");
        assert!(
            report.contains("you asked about it by name"),
            "and the report says why it is there: {report}"
        );
        assert!(
            report.contains("the vocabulary marks it owl:deprecated"),
            "with the full account, as every other command gives it: {report}"
        );
    }

    /// A children list is a list and not a tree, so nothing in it is kept for structure — which
    /// means an empty one under a concept that *has* children would read as a leaf. The count is
    /// what stops that, and the report must not then give the same count twice.
    #[test]
    fn a_list_emptied_by_the_flag_says_so_once() {
        let (_directory, store) = store_with(RETIRED);
        let report =
            tree(&store, VOCABULARY, "http://example.org/morse", true).expect("morse is a concept");

        assert!(
            report.contains(
                "none of the 1 concept(s) sharing a broader concept with it are listed: every one \
                 of them is retired"
            ),
            "{report}"
        );
        assert_eq!(
            report.matches("--current was asked for").count(),
            1,
            "the same withholding is stated once, not twice: {report}"
        );
    }

    /// Without the flag nothing changes, which is `docs/adr/0041` standing: no default moves.
    #[test]
    fn without_the_flag_every_retired_concept_is_still_shown() {
        let (_directory, store) = store_with(RETIRED);
        let report = tree(&store, VOCABULARY, "http://example.org/telegraphy", false)
            .expect("telegraphy is a concept");

        assert!(report.contains("<http://example.org/spark>"), "{report}");
        assert!(report.contains("<http://example.org/wireless>"), "{report}");
        assert!(report.contains("[retired]"), "{report}");
        assert!(!report.contains("current concepts only"), "{report}");
        assert!(!report.contains("kept as the route"), "{report}");
    }

    /// The whole point of the command: the subtree, indented, with the concepts that are
    /// transitive conclusions marked as such and the one-step ones not.
    #[test]
    fn the_tree_prints_the_subtree_and_marks_what_transitivity_licensed() {
        let (_directory, store) = store_with(ANIMALS);
        let report = tree(&store, VOCABULARY, "http://example.org/animals", false)
            .expect("animals is a concept");

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
        let report = tree(&store, VOCABULARY, "http://example.org/animals", false)
            .expect("animals is a concept");

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
        let report = tree(&store, VOCABULARY, "http://example.org/birds", false)
            .expect("birds is a concept");

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

        let leaf =
            tree(&store, VOCABULARY, "http://example.org/cats", false).expect("cats is a concept");
        assert!(leaf.contains("nothing is below it"), "{leaf}");
        assert!(leaf.contains("no concept is a child of it"), "{leaf}");
        assert!(
            leaf.contains("no other concept shares a broader concept with it"),
            "cats is its parent's only child, and having none is not having no parent: {leaf}"
        );

        let top = tree(&store, VOCABULARY, "http://example.org/animals", false)
            .expect("animals is a concept");
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

        let report =
            tree(&store, VOCABULARY, "http://example.org/a", false).expect("a is a concept");
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

        let report =
            tree(&store, VOCABULARY, "http://example.org/a", false).expect("a is a concept");
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

        let report = tree(&store, VOCABULARY, "http://example.org/assets", false)
            .expect("assets is a concept");

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

        let deep = tree(&store, VOCABULARY, "http://example.org/animals", false)
            .expect("animals is a concept");
        assert!(
            deep.contains("[S24] is a conclusion that transitivity licensed"),
            "{deep}"
        );

        let shallow = tree(&store, VOCABULARY, "http://example.org/mammals", false)
            .expect("mammals is a concept");
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
            tree(&store, VOCABULARY, "http://example.org/typo", false),
            Err(CommandError::NoSuchConcept { .. })
        ));
    }

    /// `Mammals` retired, with `Cats` still under it — the ordinary aftermath of a retirement,
    /// because `openbiz deprecate` deliberately moves nothing.
    const RETIRED_MAMMALS: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix owl: <http://www.w3.org/2002/07/owl#> .
        @prefix dcterms: <http://purl.org/dc/terms/> .
        @prefix ex: <http://example.org/> .
        ex:animals a skos:Concept ; skos:prefLabel "Animals"@en .
        ex:mammals a skos:Concept ; skos:prefLabel "Mammals"@en ; skos:broader ex:animals ;
            owl:deprecated true ; dcterms:isReplacedBy ex:vertebrates .
        ex:vertebrates a skos:Concept ; skos:prefLabel "Vertebrates"@en ; skos:broader ex:animals .
        ex:cats a skos:Concept ; skos:prefLabel "Cats"@en ; skos:broader ex:mammals .
    "#;

    /// The concept the tree is *about*, marked and explained rather than merely marked. Before
    /// this, `openbiz deprecate` could retire a concept and `openbiz tree` would show it exactly
    /// as it showed it the day before.
    #[test]
    fn a_retired_concept_asked_about_is_marked_and_explained() {
        let (_directory, store) = store_with(RETIRED_MAMMALS);
        let report = tree(&store, VOCABULARY, "http://example.org/mammals", false)
            .expect("mammals is a concept");

        assert!(report.contains("[retired]"), "{report}");
        assert!(
            report.contains("the vocabulary marks it owl:deprecated"),
            "{report}"
        );
        assert!(
            report.contains("a deprecation removes nothing"),
            "the reader has to be told the concept is still there: {report}"
        );
        assert!(
            report.contains("<http://example.org/vertebrates>  (\"Vertebrates\"@en)"),
            "the signpost is followed for them: {report}"
        );
    }

    /// The count that makes the retirement's outstanding work visible from the tree, which is the
    /// first place anyone would notice it and the last place anyone would look.
    #[test]
    fn a_retired_concept_says_how_many_current_concepts_are_below_it() {
        let (_directory, store) = store_with(RETIRED_MAMMALS);
        let report = tree(&store, VOCABULARY, "http://example.org/mammals", false)
            .expect("mammals is a concept");

        assert!(
            report.contains("1 of the 1 concept(s) below it are not retired"),
            "{report}"
        );
        assert!(
            report.contains("only a person can take"),
            "the decision is named as a person's, not implied to be automatic: {report}"
        );
    }

    /// A retired concept in a *list* carries the marker and nothing more — the tree stays a tree.
    #[test]
    fn a_retired_child_is_marked_in_the_list_and_not_explained_there() {
        let (_directory, store) = store_with(RETIRED_MAMMALS);
        let report = tree(&store, VOCABULARY, "http://example.org/animals", false)
            .expect("animals is a concept");

        assert!(
            report.contains("  <http://example.org/mammals>  (\"Mammals\"@en)  [retired]\n"),
            "{report}"
        );
        assert!(
            !report.contains("a deprecation removes nothing"),
            "the explanation belongs to the concept asked about, not to every line: {report}"
        );
        // And the current concepts around it are not marked, which is what makes the mark mean
        // something.
        assert!(
            report.contains("  <http://example.org/vertebrates>  (\"Vertebrates\"@en)\n"),
            "{report}"
        );
    }

    /// Nothing is hidden. The retired concept is still in the subtree, in its place, because
    /// dropping it would leave `Cats` hanging off nothing and misreport the vocabulary's shape.
    #[test]
    fn a_retired_concept_is_still_in_the_subtree_below_a_current_one() {
        let (_directory, store) = store_with(RETIRED_MAMMALS);
        let report = tree(&store, VOCABULARY, "http://example.org/animals", false)
            .expect("animals is a concept");

        assert!(report.contains("3 concept(s) are below it"), "{report}");
        assert!(
            report.contains("    <http://example.org/cats>  (\"Cats\"@en)  [S24]"),
            "the concept under the retired one is still reached: {report}"
        );
    }

    /// A vocabulary that retires nothing reads exactly as it did, which is what stops this feature
    /// from being a tax on every other report.
    #[test]
    fn a_vocabulary_with_no_retirements_says_nothing_about_them() {
        let (_directory, store) = store_with(ANIMALS);
        let report = tree(&store, VOCABULARY, "http://example.org/animals", false)
            .expect("animals is a concept");

        assert!(!report.contains("retired"), "{report}");
        assert!(!report.contains("owl:deprecated"), "{report}");
    }
}
