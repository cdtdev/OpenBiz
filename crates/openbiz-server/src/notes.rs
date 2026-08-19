//! `openbiz notes` — print everything a vocabulary documents one resource with, and why.
//!
//! This is SKOS Reference §7 made reachable. `openbiz inspect` reports documentation as *coverage*
//! — how many concepts carry a definition, a scope note, an example — because the notes are
//! bounded by the size of the vocabulary and every other answer in that report is bounded by its
//! structure. This is the command that shows the notes themselves, for one resource at a time.
//!
//! # Why one resource and not the vocabulary
//!
//! The definition is the longest text a thesaurus holds and there is one per concept. Printing
//! them all is `openbiz export`'s job, in a standard syntax, and it already does it. What no
//! standard syntax gives you is the S17 entailment made visible: a Turtle export of a vocabulary
//! shows the `skos:definition` the author wrote and never shows the `skos:note` it entails, so an
//! operator asking "why does the count say 400 notes when I wrote 120 definitions?" has nowhere to
//! look. Here the entailed note is printed beside the asserted one, carrying the statement it came
//! from and the rule that licensed it, which is `CLAUDE.md` §3's requirement.
//!
//! # It is not only for concepts
//!
//! §7's own Example 24 puts a `skos:definition` on an `owl:Class`, and the specification marks it
//! consistent. So this command takes a resource IRI, not a concept IRI, and reports whatever the
//! vocabulary documents — a concept, a scheme, a collection, or something SKOS has no opinion
//! about at all.
//!
//! # Why a command and not an endpoint
//!
//! The same reason `openbiz inspect` and `openbiz ancestors` are commands, and it is not the
//! authentication objection: this only reads. The concept editor that will show a definition
//! beside the label it belongs to is Phase 3's item, and shipping an endpoint now with no
//! interface behind it would be a caller with nothing behind it.

use openbiz_skos::{CoreModel, Node, NoteKind, NoteOrigin, Resource, Term};
use openbiz_store::Store;

use crate::cli::CommandError;
use crate::inspect::convert;

/// Report what the vocabulary at `graph` documents `resource` with, and why.
///
/// Reads and nothing else.
///
/// A resource the vocabulary says nothing about *in SKOS terms* is **refused**, not reported as an
/// undocumented one. The two read identically and mean opposite things — one is a concept nobody
/// has got round to defining, the other is a mistyped IRI — and at a command line the second is
/// the likelier.
pub fn notes(store: &Store, graph: &str, resource: &str) -> Result<String, CommandError> {
    let mut builder = CoreModel::builder();
    store.for_each_statement(graph, |statement| builder.push(convert(statement)))?;
    let model = builder.build();

    let node = Node::iri(resource);
    let Some(held) = model.resource(&node) else {
        return Err(CommandError::NoSuchResource {
            resource: resource.to_owned(),
            graph: graph.to_owned(),
        });
    };

    Ok(report(graph, &node, held))
}

/// The report itself, kept apart from the store so it can be tested against a model in hand.
fn report(graph: &str, node: &Node, resource: &Resource) -> String {
    let mut out = String::new();
    out.push_str(&format!("{node}{}\n", named(resource)));
    out.push_str(&format!("in {graph}\n"));

    if resource.notes().is_empty() {
        // Deliberately not phrased as a defect. §7 states no integrity condition at all, so a
        // resource with no documentation is consistent SKOS and this command must not imply
        // otherwise. Whether a concept *ought* to have a definition is a Z39.19 or ISO 25964
        // question, which is a rule pack in `openbiz-validate` and not a sentence here.
        out.push_str(
            "\nno SKOS documentation property carries anything for it. SKOS states no condition \
             requiring one.\n",
        );
        return out;
    }

    // Grouped by property, in `NoteKind::ALL` order — what it means, where it stops, what it looks
    // like, then the historical notes, then the editor's, then the general one. A property with
    // nothing under it is skipped: unlike the coverage table in `openbiz inspect`, which answers
    // "how documented is this vocabulary" and needs its zeroes, this answers "what does it say
    // about this one thing" and a screen of empty headings would bury the answer.
    for kind in NoteKind::ALL {
        let values: Vec<&Term> = resource.notes_of(kind).collect();
        if values.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{kind}\n"));
        for value in values {
            out.push_str(&format!("  {value}\n"));
            // Only the entailed ones explain themselves. An asserted note needs no derivation —
            // the graph says it — and printing "asserted" against every line would bury the S17
            // entailments this command exists to make visible.
            if let Some(NoteOrigin::Entailed(rule)) = resource.note_origin(value, kind) {
                out.push_str(&format!("    inferred, not stated under {kind}\n"));
                out.push_str(&format!("    because {}\n", stated_under(resource, value)));
                out.push_str(&format!("    and {rule}\n"));
            }
        }
    }

    out
}

/// The properties that *did* state this value, rendered as the premise of an S17 lift.
///
/// Plural because more than one can have: a value stated as both a `skos:definition` and a
/// `skos:scopeNote` licenses one `skos:note` between them, and naming only the first would be a
/// premise that is true but incomplete — a reader removing that one statement would expect the
/// conclusion to go away, and it would not.
fn stated_under(resource: &Resource, value: &Term) -> String {
    let stated: Vec<String> = NoteKind::ALL
        .into_iter()
        .filter(|kind| resource.note_origin(value, *kind) == Some(NoteOrigin::Asserted))
        .map(|kind| format!("{kind} {value}"))
        .collect();
    if stated.is_empty() {
        // Unreachable: S17 is the only rule that entails a note, and it only fires from an
        // asserted one. Rendered rather than panicked on, for the reason `inspect` gives — a
        // report that admits a gap in itself beats one that aborts on a customer's vocabulary.
        return "no asserted note was recorded, which is a defect in this report".to_owned();
    }
    stated.join(", and ")
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

    use super::notes;
    use crate::cli::CommandError;

    /// The vocabulary every fixture below is loaded into.
    const VOCABULARY: &str = "https://example.org/chemistry";

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
@prefix ex:   <https://example.org/chemistry/> .
";

    /// The whole point of the command: the asserted note and the one S17 supplied, side by side,
    /// with the premise and the rule for the second.
    #[test]
    fn an_entailed_note_is_printed_with_the_statement_that_licensed_it() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Chemistry a skos:Concept ;
  skos:prefLabel \"Chemistry\"@en ;
  skos:definition \"The study of matter.\"@en ."
        ));

        let report = match notes(
            &store,
            VOCABULARY,
            "https://example.org/chemistry/Chemistry",
        ) {
            Ok(report) => report,
            Err(error) => unreachable!("the concept is in the vocabulary: {error}"),
        };

        assert!(report.contains("(\"Chemistry\"@en)"), "{report}");
        assert!(report.contains("skos:definition"), "{report}");
        assert!(report.contains("skos:note"), "{report}");
        assert!(
            report.contains("inferred, not stated under skos:note"),
            "{report}"
        );
        assert!(
            report.contains("because skos:definition \"The study of matter.\"@en"),
            "{report}"
        );
        assert!(
            report.contains("sub-properties of skos:note"),
            "the rule must be quoted, not merely numbered: {report}"
        );
    }

    /// A resource with no documentation is reported, not refused — and the report says the
    /// specification asks for none, so an operator does not read a legal vocabulary as a broken
    /// one.
    #[test]
    fn an_undocumented_concept_is_reported_and_not_called_a_defect() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Physics a skos:Concept ; skos:prefLabel \"Physics\"@en ."
        ));

        let report = match notes(&store, VOCABULARY, "https://example.org/chemistry/Physics") {
            Ok(report) => report,
            Err(error) => unreachable!("the concept is in the vocabulary: {error}"),
        };

        assert!(
            report.contains("no SKOS documentation property"),
            "{report}"
        );
        assert!(
            report.contains("SKOS states no condition requiring one"),
            "{report}"
        );
    }

    /// §7's Example 24 through the command: the subject is an `owl:Class`, not a concept, and it
    /// is documented all the same.
    #[test]
    fn a_definition_on_something_that_is_not_a_concept_is_reported() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
@prefix owl: <http://www.w3.org/2002/07/owl#> .
ex:Protein a owl:Class ;
  skos:definition \"A physical entity consisting of a sequence of amino-acids.\"@en ."
        ));

        let report = match notes(&store, VOCABULARY, "https://example.org/chemistry/Protein") {
            Ok(report) => report,
            Err(error) => unreachable!("the class carries a note: {error}"),
        };
        assert!(report.contains("sequence of amino-acids"), "{report}");
    }

    /// Example 23's `<MyNote>` — the *object* of a note — is not a resource of ours, because §7
    /// gives `skos:note` no range and typing its object would add a member to the customer's
    /// vocabulary that nobody wrote. So asking about it is refused rather than answered with an
    /// empty report.
    #[test]
    fn the_object_of_a_note_is_not_itself_a_documented_resource() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Chemistry a skos:Concept ; skos:note ex:MyNote ."
        ));

        let error = match notes(&store, VOCABULARY, "https://example.org/chemistry/MyNote") {
            Ok(report) => unreachable!("the object of a note is not in the model: {report}"),
            Err(error) => error,
        };
        assert!(
            matches!(error, CommandError::NoSuchResource { .. }),
            "{error}"
        );
        assert!(error.to_string().contains("in SKOS terms"), "{error}");
    }

    /// A value stated under two properties licenses one `skos:note`, and the premise names both.
    /// Naming only one would be a premise a reader could falsify by deleting that statement and
    /// finding the conclusion still standing.
    #[test]
    fn a_value_under_two_properties_names_both_in_the_premise() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Chemistry a skos:Concept ;
  skos:definition \"Matter.\"@en ;
  skos:scopeNote \"Matter.\"@en ."
        ));

        let report = match notes(
            &store,
            VOCABULARY,
            "https://example.org/chemistry/Chemistry",
        ) {
            Ok(report) => report,
            Err(error) => unreachable!("the concept is in the vocabulary: {error}"),
        };
        assert!(
            report.contains(
                "because skos:definition \"Matter.\"@en, and skos:scopeNote \
                             \"Matter.\"@en"
            ),
            "{report}"
        );
    }

    /// An unregistered graph is refused by the store, exactly as `openbiz inspect` and
    /// `openbiz ancestors` are — a typo in the vocabulary IRI must not read as an empty one.
    #[test]
    fn an_unregistered_vocabulary_is_refused() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Chemistry a skos:Concept ."
        ));
        let error = notes(
            &store,
            "https://example.org/nothing",
            "https://example.org/x",
        )
        .expect_err("an unregistered graph is refused");
        assert!(matches!(error, CommandError::Store(_)), "{error}");
    }
}
