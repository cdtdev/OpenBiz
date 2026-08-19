//! `openbiz mappings` — print what one concept is joined to in other vocabularies, and why.
//!
//! This is SKOS Reference §10 made reachable per concept, and it is the anti-silo half of the
//! model at the scale a person works at. `openbiz inspect` answers "how mapped is this
//! vocabulary" — so many hierarchical links, so many equivalence ones — because those counts are
//! what a governance function asks of a whole thesaurus. An author asks a different question
//! about one concept, and until this command existed the build had no answer to it: reading
//! "4 exact mapping link(s)" in a 100 000-concept vocabulary told you nothing about *which* four.
//!
//! # Why it is worth a command rather than an export
//!
//! The same argument `openbiz notes` makes for §7, and §10 makes it more sharply. A Turtle export
//! shows the `skos:broadMatch` the author wrote. It does not show the `skos:narrowMatch` S43
//! entails, the `skos:closeMatch` S42 lifts out of every exact match, or — the one nothing in any
//! serialisation can show — the concepts reached only by **chaining** exact matches under S45.
//! That last one is the ordinary enterprise shape: a house vocabulary is mapped to a hub, the hub
//! is mapped to a regulator's list, and the question "is our Client their Counterparty?" is
//! answered by two statements neither of which mentions both ends.
//!
//! # The closure is walked here, not stored
//!
//! [`CoreModel::exact_match_cluster`] is the walk and `docs/adr/0030` records why S45's closure is
//! never materialised. What that costs this command is a bound: a cluster too large to walk is
//! reported as an incomplete answer rather than as a complete short one, because an absence from
//! an abandoned walk proves nothing and reading one as proof is how a report claims a check it
//! never finished.
//!
//! # Why a command and not an endpoint
//!
//! As with `openbiz inspect`, `openbiz ancestors` and `openbiz notes`, and not the authentication
//! objection: this only reads. The mapping editor that will show these links beside the concept
//! they belong to is Phase 3's, and an endpoint now would be a caller with nothing behind it.

use openbiz_skos::{EquivalenceBound, MappingProperty, Node, RelationOrigin, Resource, SkosRule};
use openbiz_store::Store;

use crate::cli::CommandError;

/// Report what the vocabulary at `graph` joins `resource` to, and why.
///
/// Reads and nothing else.
///
/// A resource the vocabulary says nothing about *in SKOS terms* is **refused**, not reported as
/// an unmapped one — the same distinction `openbiz notes` draws, and for the same reason: the two
/// read identically and mean opposite things, and at a command line a mistyped IRI is the
/// likelier.
pub fn mappings(store: &Store, graph: &str, resource: &str) -> Result<String, CommandError> {
    let model = crate::inspect::read(store, graph)?;

    let node = Node::iri(resource);
    let Some(held) = model.resource(&node) else {
        return Err(CommandError::NoSuchResource {
            resource: resource.to_owned(),
            graph: graph.to_owned(),
        });
    };

    let cluster = model.exact_match_cluster(&node, EquivalenceBound::DEFAULT);
    Ok(report(graph, &node, held, &cluster))
}

/// The report itself, kept apart from the store so it can be tested against a model in hand.
fn report(
    graph: &str,
    node: &Node,
    resource: &Resource,
    cluster: &openbiz_skos::ExactMatchCluster,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("{node}{}\n", named(resource)));
    out.push_str(&format!("in {graph}\n"));

    if resource.mappings().is_empty() {
        // Deliberately not phrased as a defect. §10 states no condition requiring a concept to be
        // mapped to anything, and a vocabulary that maps nothing is perfectly good SKOS. Whether
        // a concept *ought* to be mapped is `CLAUDE.md` §1.7's question, answered by discovery
        // and not by a sentence here.
        out.push_str(
            "\nno SKOS mapping property links it to anything. SKOS states no condition requiring \
             one.\n",
        );
        return out;
    }

    // Grouped by property, in `MappingProperty::ALL` order — the two hierarchical directions, the
    // associative one, then the two equivalence ones from weaker to stronger, which is how
    // `openbiz inspect` orders them too. A property with nothing under it is skipped: this
    // answers "what is *this* concept joined to" and a screen of empty headings would bury it.
    for property in MappingProperty::ALL {
        let Some(links) = resource.mappings_of(property) else {
            continue;
        };
        if links.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{property}\n"));
        for (other, origin) in links {
            out.push_str(&format!("  {other}\n"));
            // Only the entailed ones explain themselves. An asserted link needs no derivation —
            // the graph says it — and printing "asserted" against every line would bury the
            // entailments this command exists to make visible.
            if let RelationOrigin::Entailed(rule) = origin {
                out.push_str("    inferred, not stated\n");
                out.push_str(&format!("    and {rule}\n"));
            }
        }
        // Said once per section rather than per link, because it is a property of the property.
        // Without it a reader has no way to know that the mapping they just read is also in the
        // hierarchy `openbiz ancestors` walks and the one §8.4's disjointness is checked over.
        if let Some((relation, rule)) = property.semantic_counterpart() {
            out.push_str(&format!(
                "  each of these is also a {relation} link\n    and {rule}\n"
            ));
        }
    }

    out.push_str(&exact_match_chains(node, cluster));
    out
}

/// The concepts reached only by chaining exact matches — S45's conclusions and nothing else.
///
/// The one-step members are the graph's own links and S44's converses, and they are printed above
/// under `skos:exactMatch`. Repeating them here would credit S45 with a conclusion it did not
/// add, which is the same line [`openbiz_skos::ExactMatchCluster::derivation_to`] draws.
///
/// # The origin is printed apart from the rest, and printed
///
/// A concept with any exact match at all is its own exact match — S44 gives the converse, S45
/// composes the two — so the origin is in every non-trivial cluster and its chain is the least
/// interesting one in the report. Found by running the binary: listed among the others it sorts
/// by IRI and lands wherever the alphabet puts it, so the answer an author came for ("what are we
/// equivalent to?") reads as though it began with "yourself".
///
/// It is moved rather than dropped. It is a conclusion this build draws, it is what makes
/// `<A> skos:exactMatch <B>` plus `<A> skos:broadMatch <A>` an S46 violation, and a conclusion
/// hidden from the report is one an operator cannot check. §10.6.6's Example 66 marks a reflexive
/// mapping consistent, and the line says so, because an author who reads "it is its own exact
/// match" without that sentence will look for the mistake that caused it.
fn exact_match_chains(node: &Node, cluster: &openbiz_skos::ExactMatchCluster) -> String {
    let mut out = String::new();
    let (reflexive, chains): (Vec<_>, Vec<_>) =
        cluster.entailed().partition(|(other, _)| *other == node);

    if !chains.is_empty() {
        out.push_str("\njoined through a chain of exact matches\n");
        for (other, chain) in chains {
            out.push_str(&format!("  {other}\n"));
            out.push_str("    inferred, not stated\n");
            out.push_str(&format!("    because {}\n", steps(&chain)));
            out.push_str(&format!("    and {}\n", SkosRule::S45));
        }
    }

    for (_, chain) in reflexive {
        out.push_str("\nand it is its own exact match\n");
        out.push_str(&format!("  because {}\n", steps(&chain)));
        out.push_str(&format!("  and {}\n", SkosRule::S45));
        out.push_str(
            "  \u{a7}10.6.6 marks a reflexive mapping consistent, so this is a conclusion and \
             never a defect\n",
        );
    }

    // An abandoned walk must never read as a finished one. Said whether or not any chain was
    // found, and said last so it qualifies everything above it: what is printed is a floor.
    if !cluster.is_complete() {
        out.push_str(&format!(
            "\nthe walk of {node}'s exact-match cluster stopped after {} concept(s) and {} \
             link(s) without closing\n  so the chains above are what was reached and not all \
             there are\n  and {}\n",
            cluster.len(),
            cluster.links_walked(),
            SkosRule::S45,
        ));
    }

    out
}

/// An exact-match chain rendered as the statements it is made of, which is its premise.
fn steps(chain: &[Node]) -> String {
    chain
        .windows(2)
        .map(|step| format!("{} skos:exactMatch {}", step[0], step[1]))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A resource's label in parentheses, or nothing if it has none. As `openbiz notes` prints it.
fn named(resource: &Resource) -> String {
    match resource.display_label() {
        Some(label) => format!("  ({label})"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use openbiz_store::{GraphId, RdfSyntax, Store};

    use super::mappings;
    use crate::cli::CommandError;

    /// The vocabulary every fixture below is loaded into.
    const VOCABULARY: &str = "https://example.org/house";

    /// A store holding `turtle` in one registered vocabulary, ready to read.
    ///
    /// The statements go in through `propose_import` and `decide`, which is the seam every write
    /// passes through — a test that reached past it would be testing a path no operator can use.
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

    const PREFIXES: &str = "\
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex:   <https://example.org/house/> .
@prefix hub:  <https://hub.example/> .
@prefix reg:  <https://regulator.example/> .
";

    fn report_for(store: &Store, resource: &str) -> String {
        match mappings(store, VOCABULARY, resource) {
            Ok(report) => report,
            Err(error) => unreachable!("the concept is in the vocabulary: {error}"),
        }
    }

    /// The hub shape, which is what this command exists for: the house concept is mapped to a hub
    /// and the hub onwards, and the regulator's concept is reachable only by chaining. The chain
    /// and S45 are both printed, because a conclusion with no derivation is not one.
    #[test]
    fn a_concept_reached_only_by_chaining_exact_matches_is_printed_with_its_chain() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Client a skos:Concept ;
  skos:prefLabel \"Client\"@en ;
  skos:exactMatch hub:Party .
hub:Party skos:exactMatch reg:Counterparty ."
        ));

        let report = report_for(&store, "https://example.org/house/Client");

        assert!(report.contains("(\"Client\"@en)"), "{report}");
        assert!(report.contains("\nskos:exactMatch\n"), "{report}");
        assert!(report.contains("<https://hub.example/Party>"), "{report}");
        assert!(
            report.contains("joined through a chain of exact matches"),
            "{report}"
        );
        assert!(
            report.contains("<https://regulator.example/Counterparty>"),
            "the whole point of the walk: {report}"
        );
        assert!(
            report.contains(
                "because <https://example.org/house/Client> skos:exactMatch \
                 <https://hub.example/Party>, <https://hub.example/Party> skos:exactMatch \
                 <https://regulator.example/Counterparty>"
            ),
            "{report}"
        );
        assert!(
            report.contains("S45: skos:exactMatch is an instance of owl:TransitiveProperty."),
            "the rule must be quoted, not merely numbered: {report}"
        );
    }

    /// A link the graph wrote from the other end reaches the report, carrying the statement that
    /// licensed it — S43 for the hierarchical pair, S44 for the symmetric ones. Without the
    /// origin an author searching their own Turtle for `skos:narrowMatch` finds nothing and
    /// concludes the tool invented the link.
    #[test]
    fn a_link_the_author_wrote_from_the_other_end_is_printed_with_the_rule_that_turned_it_round() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Client a skos:Concept ; skos:prefLabel \"Client\"@en .
hub:Party skos:narrowMatch ex:Client ."
        ));

        let report = report_for(&store, "https://example.org/house/Client");

        assert!(report.contains("\nskos:broadMatch\n"), "{report}");
        assert!(report.contains("inferred, not stated"), "{report}");
        assert!(report.contains("S43"), "{report}");
    }

    /// S42's lift is visible per concept: an exact match is also a close match, and the report
    /// says which statement made it one.
    #[test]
    fn an_exact_match_is_reported_as_a_close_match_too_and_says_why() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Client a skos:Concept ; skos:exactMatch hub:Party ."
        ));

        let report = report_for(&store, "https://example.org/house/Client");
        assert!(report.contains("\nskos:closeMatch\n"), "{report}");
        assert!(report.contains("S42"), "{report}");
    }

    /// S41 is stated once per section, because an author needs to know the mapping they just read
    /// is in the hierarchy `openbiz ancestors` walks. The equivalence properties lift into
    /// nothing and must not claim to — §10 gives them no counterpart in §8, and inventing one
    /// would put every exact match into `skos:related` and start reporting S27 clashes SKOS does
    /// not state.
    #[test]
    fn the_hierarchical_and_associative_mappings_say_they_are_also_semantic_relations() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Client a skos:Concept ;
  skos:broadMatch hub:Party ;
  skos:relatedMatch hub:Account ;
  skos:exactMatch reg:Counterparty ."
        ));

        let report = report_for(&store, "https://example.org/house/Client");
        assert!(
            report.contains("each of these is also a skos:broader link"),
            "{report}"
        );
        assert!(
            report.contains("each of these is also a skos:related link"),
            "{report}"
        );
        assert_eq!(
            report.matches("each of these is also").count(),
            2,
            "one for each of the two sections S41 names, and neither equivalence property: \
             {report}"
        );
    }

    /// A concept the vocabulary maps nowhere is reported, not refused — and the report says the
    /// specification asks for no mapping, so an operator does not read a legal vocabulary as a
    /// broken one.
    #[test]
    fn an_unmapped_concept_is_reported_and_not_called_a_defect() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Client a skos:Concept ; skos:prefLabel \"Client\"@en ."
        ));

        let report = report_for(&store, "https://example.org/house/Client");
        assert!(report.contains("no SKOS mapping property"), "{report}");
        assert!(
            report.contains("SKOS states no condition requiring one"),
            "{report}"
        );
    }

    /// A concept the vocabulary says nothing about in SKOS terms is refused, so a mistyped IRI
    /// does not read as a well-formed unmapped concept.
    #[test]
    fn a_resource_the_vocabulary_never_mentions_is_refused() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Client a skos:Concept ."
        ));

        let error = match mappings(&store, VOCABULARY, "https://example.org/house/Nothing") {
            Ok(report) => unreachable!("nothing in the graph mentions it: {report}"),
            Err(error) => error,
        };
        assert!(
            matches!(error, CommandError::NoSuchResource { .. }),
            "{error}"
        );
    }

    /// An unregistered graph is refused by the store, exactly as every other reading command is —
    /// a typo in the vocabulary IRI must not read as an empty one.
    #[test]
    fn an_unregistered_vocabulary_is_refused() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Client a skos:Concept ."
        ));
        let error = mappings(
            &store,
            "https://example.org/nothing",
            "https://example.org/x",
        )
        .expect_err("an unregistered graph is refused");
        assert!(matches!(error, CommandError::Store(_)), "{error}");
    }

    /// §10.6.6's Example 66 through the command: a concept with any exact match is its own exact
    /// match, and the report prints the chain rather than hiding a conclusion the build drew —
    /// but prints it *apart* from the concepts the author actually wants, and says the
    /// specification calls it consistent.
    #[test]
    fn a_mapped_concept_is_its_own_exact_match_and_the_report_says_why() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Client a skos:Concept ; skos:exactMatch hub:Party ."
        ));

        let report = report_for(&store, "https://example.org/house/Client");
        assert!(
            report.contains("and it is its own exact match"),
            "the reflexive conclusion is printed, not hidden: {report}"
        );
        assert!(
            report.contains("\u{a7}10.6.6 marks a reflexive mapping consistent"),
            "an unexplained self-link reads as a defect: {report}"
        );
        assert!(
            !report.contains("joined through a chain of exact matches"),
            "there is nothing else in the cluster, so there is no chain section: {report}"
        );
    }

    /// The reflexive conclusion never lands in the middle of the list an author came for. Found
    /// by running the binary against a three-vocabulary chain, where sorting by IRI put the
    /// concept's own IRI first and the report opened by telling the author about themselves.
    #[test]
    fn the_reflexive_conclusion_is_printed_after_the_concepts_the_author_asked_about() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Client a skos:Concept ; skos:exactMatch hub:Party .
hub:Party skos:exactMatch reg:Counterparty ."
        ));

        let report = report_for(&store, "https://example.org/house/Client");
        let chains = report
            .find("joined through a chain of exact matches")
            .expect("the far concept is reached by chaining");
        let reflexive = report
            .find("and it is its own exact match")
            .expect("the reflexive conclusion is still printed");
        assert!(chains < reflexive, "{report}");
        // And the far concept is inside the first section, not the second.
        let far = report
            .find("<https://regulator.example/Counterparty>\n    inferred")
            .expect("the far concept carries its derivation");
        assert!(chains < far && far < reflexive, "{report}");
    }
}
