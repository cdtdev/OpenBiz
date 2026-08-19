//! `openbiz deprecate` end to end, against the real `openbiz` binary as a child process.
//!
//! The fourth bulk operation, and the one whose whole claim is about what survives. Two things can
//! only be checked against a real store:
//!
//! - **Nothing is removed.** That is a statement about a whole graph, so the test reads the graph
//!   off disk with `openbiz backup` before and after and asserts that every line present before is
//!   still present after — rather than asking the code that computed the change whether it removed
//!   anything.
//! - **The concept is still a concept afterwards**, still labelled, still in its scheme, and now
//!   carrying `owl:deprecated` and `dcterms:isReplacedBy`. A retirement that quietly cost the
//!   concept its type or its labels would break every system holding the IRI, which is the exact
//!   failure retiring rather than deleting exists to prevent.
//!
//! The store is seeded from a hand-written backup, as `candidate_seam.rs` explains: there is no
//! "create vocabulary" command, because creating one runs through discovery with a recorded
//! justification (`CLAUDE.md` §1.7).

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Output};

/// A store holding one empty vocabulary, as an operator could type it.
const BACKUP: &str = concat!(
    "<urn:openbiz:store> <urn:openbiz:storeFormatVersion> ",
    "\"3\"^^<http://www.w3.org/2001/XMLSchema#integer> <urn:openbiz:graph:system> .\n",
    "<urn:openbiz:graph:system> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<urn:openbiz:graph:system> <urn:openbiz:graphKind> \"system\" <urn:openbiz:graph:system> .\n",
    "<https://example.org/thesaurus> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<https://example.org/thesaurus> <urn:openbiz:graphKind> \"vocabulary\" ",
    "<urn:openbiz:graph:system> .\n",
);

const THESAURUS: &str = "https://example.org/thesaurus";
const WIRELESS: &str = "https://example.org/thesaurus/wireless";
const RADIO: &str = "https://example.org/thesaurus/radio";
const DEPRECATED: &str = "http://www.w3.org/2002/07/owl#deprecated";
const IS_REPLACED_BY: &str = "http://purl.org/dc/terms/isReplacedBy";
const CHANGE_NOTE: &str = "http://www.w3.org/2004/02/skos/core#changeNote";
const CONCEPT: &str = "http://www.w3.org/2004/02/skos/core#Concept";

/// A term that went out of use, with a live child, a parent, a scheme it heads and a collection
/// that lists it — everything a retirement leaves for a person to decide about.
const CONCEPTS: &str = r#"
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex: <https://example.org/thesaurus/> .

ex:scheme a skos:ConceptScheme .
ex:telegraphy a skos:Concept ; skos:prefLabel "Telegraphy"@en ; skos:inScheme ex:scheme .
ex:wireless a skos:Concept ; skos:prefLabel "Wireless telegraphy"@en ;
    skos:altLabel "Radiotelegraphy"@en ; skos:inScheme ex:scheme ; skos:topConceptOf ex:scheme ;
    skos:broader ex:telegraphy ; skos:scopeNote "Pre-1930 usage."@en .
ex:radio a skos:Concept ; skos:prefLabel "Radio"@en ; skos:inScheme ex:scheme .
ex:morse a skos:Concept ; skos:prefLabel "Morse code"@en ; skos:broader ex:wireless ;
    skos:inScheme ex:scheme .
ex:obsolete a skos:Collection ; skos:member ex:wireless .
"#;

fn run(data_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openbiz"))
        .args(args)
        .current_dir(data_dir)
        .env("OPENBIZ_DATA_DIR", data_dir)
        .env("OPENBIZ_ACTOR", "ada@example.org")
        .env("OPENBIZ_LOG", "warn")
        .env_remove("OPENBIZ_CONFIG")
        .output()
        .expect("run the openbiz binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A store with the thesaurus populated, through the seam that puts statements there.
fn populated() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("seed.nq"), BACKUP).expect("write the fixture");
    let restored = run(dir.path(), &["restore", "seed.nq"]);
    assert!(restored.status.success(), "{}", stderr(&restored));

    std::fs::write(dir.path().join("import.ttl"), CONCEPTS).expect("write the import");
    let imported = run(dir.path(), &["import", THESAURUS, "import.ttl"]);
    assert!(imported.status.success(), "{}", stderr(&imported));
    let approved = run(dir.path(), &["approve", "1"]);
    assert!(approved.status.success(), "{}", stderr(&approved));
    dir
}

/// Every statement in the store, as the N-Quads lines `openbiz backup` writes.
///
/// The graph as it is on disk, which is the only thing that can answer "was anything removed?".
fn quads(dir: &Path, name: &str) -> BTreeSet<String> {
    let backed_up = run(dir, &["backup", name]);
    assert!(backed_up.status.success(), "{}", stderr(&backed_up));
    std::fs::read_to_string(dir.join(name))
        .expect("read the backup")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

/// The lines of a backup that mention one IRI **in the vocabulary graph**.
///
/// The graph has to be part of the filter. A backup is every quad in the store, and an approved
/// candidate keeps its own copy of the statements it proposed — so counting lines that merely
/// mention the concept counts each one twice and reports a marker written once as written twice.
fn about<'a>(quads: &'a BTreeSet<String>, iri: &str) -> BTreeSet<&'a str> {
    quads
        .iter()
        .filter(|line| line.contains(iri) && line.ends_with(&format!("<{THESAURUS}> .")))
        .map(String::as_str)
        .collect()
}

/// Retire the concept and approve the candidate, returning the report.
fn retire_and_approve(dir: &Path, args: &[&str], candidate: &str) -> String {
    let proposed = run(dir, args);
    assert!(proposed.status.success(), "{}", stderr(&proposed));
    let approved = run(dir, &["approve", candidate]);
    assert!(approved.status.success(), "{}", stderr(&approved));
    stdout(&proposed)
}

/// The claim the whole operation rests on: a retirement is not a deletion. Every statement the
/// vocabulary held about the concept before is still there afterwards, letter for letter.
#[test]
fn a_retirement_adds_three_statements_and_removes_none() {
    let dir = populated();
    let before = quads(dir.path(), "before.nq");

    retire_and_approve(
        dir.path(),
        &[
            "deprecate",
            THESAURUS,
            WIRELESS,
            "--replaced-by",
            RADIO,
            "--note",
            "Superseded by broadcasting terms.",
        ],
        "2",
    );

    let after = quads(dir.path(), "after.nq");
    let vocabulary_before: BTreeSet<&str> = before
        .iter()
        .filter(|line| line.ends_with(&format!("<{THESAURUS}> .")))
        .map(String::as_str)
        .collect();
    for line in &vocabulary_before {
        assert!(
            after.contains(*line),
            "a retirement removed a statement it should have kept:\n{line}"
        );
    }

    let now = about(&after, WIRELESS);
    let added: Vec<&&str> = now.iter().filter(|line| !before.contains(**line)).collect();
    assert_eq!(
        added.len(),
        3,
        "the marker, the replacement and the note, and nothing else: {added:#?}"
    );
    assert!(added.iter().any(|line| line.contains(DEPRECATED)));
    assert!(added.iter().any(|line| line.contains(IS_REPLACED_BY)));
    assert!(added.iter().any(|line| line.contains(CHANGE_NOTE)));
}

/// A retired concept that lost its type or its labels would break every system holding the IRI,
/// which is the failure retiring rather than deleting exists to prevent.
#[test]
fn a_retired_concept_is_still_a_concept_with_its_labels_and_its_place() {
    let dir = populated();
    retire_and_approve(
        dir.path(),
        &["deprecate", THESAURUS, WIRELESS, "--replaced-by", RADIO],
        "2",
    );

    let inspected = run(dir.path(), &["inspect", THESAURUS]);
    assert!(inspected.status.success(), "{}", stderr(&inspected));

    let after = quads(dir.path(), "after.nq");
    let now = about(&after, WIRELESS);
    assert!(
        now.iter().any(|line| line.contains(CONCEPT)),
        "still a skos:Concept: {now:#?}"
    );
    assert!(now.iter().any(|line| line.contains("Wireless telegraphy")));
    assert!(now.iter().any(|line| line.contains("Radiotelegraphy")));
    assert!(now.iter().any(|line| line.contains("Pre-1930 usage")));
    // And the child is still under it, untouched, which is what the report said it was leaving.
    assert!(now.iter().any(|line| line.contains("morse")));
}

/// The workflow the second call exists for: retired when it went out of use, replacement agreed on
/// months later. It needs the marker written by the first call to be visible to the second, which
/// is a round trip through the store.
#[test]
fn a_replacement_can_be_recorded_against_a_concept_retired_earlier() {
    let dir = populated();
    retire_and_approve(dir.path(), &["deprecate", THESAURUS, WIRELESS], "2");

    let report = retire_and_approve(
        dir.path(),
        &["deprecate", THESAURUS, WIRELESS, "--replaced-by", RADIO],
        "3",
    );
    assert!(
        report.contains("already deprecated — this only records what it is replaced by"),
        "{report}"
    );

    let after = quads(dir.path(), "after.nq");
    let now = about(&after, WIRELESS);
    let markers = now.iter().filter(|line| line.contains(DEPRECATED)).count();
    assert_eq!(markers, 1, "the marker is not written twice: {now:#?}");
    assert!(now.iter().any(|line| line.contains(IS_REPLACED_BY)));
}

/// A candidate that changes nothing spends a reviewer's attention for no decision, so the second
/// identical retirement is refused rather than staged.
#[test]
fn retiring_a_concept_twice_over_is_refused_and_stages_nothing() {
    let dir = populated();
    retire_and_approve(dir.path(), &["deprecate", THESAURUS, WIRELESS], "2");

    let again = run(dir.path(), &["deprecate", THESAURUS, WIRELESS]);
    assert!(!again.status.success(), "{}", stdout(&again));
    assert!(
        stderr(&again).contains("is already deprecated"),
        "{}",
        stderr(&again)
    );

    let candidates = run(dir.path(), &["candidates"]);
    assert!(candidates.status.success(), "{}", stderr(&candidates));
    assert!(
        stdout(&candidates).contains("0 proposed"),
        "nothing is left waiting: {}",
        stdout(&candidates)
    );
}

/// The report's own claim about the work it is leaving, read from the real binary rather than from
/// the function that wrote it.
#[test]
fn the_report_names_what_the_retirement_stranded() {
    let dir = populated();
    let proposed = run(
        dir.path(),
        &["deprecate", THESAURUS, WIRELESS, "--replaced-by", RADIO],
    );
    assert!(proposed.status.success(), "{}", stderr(&proposed));
    let report = stdout(&proposed);

    assert!(report.contains("1 concept is still below it"), "{report}");
    assert!(report.contains("it heads the browse tree of 1 scheme"));
    assert!(report.contains("1 collection still lists it as a member"));
    assert!(
        report.contains("a signpost and not a rewrite"),
        "the thing an operator is most likely to assume otherwise: {report}"
    );
    assert!(
        report.contains("and remove nothing"),
        "and the claim the operation rests on: {report}"
    );
}
