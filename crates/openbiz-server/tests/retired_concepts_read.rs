//! What a retired concept looks like to the commands that browse a vocabulary, end to end.
//!
//! `deprecate_concept.rs` proves the write half: the marker lands, and nothing is removed. This is
//! the consequence of that second fact, which is the whole reason a read half was needed. Because
//! a deprecation retracts nothing (`docs/adr/0040`), a retired concept is byte-for-byte the
//! concept it was — same type, same labels, same place in the hierarchy — so every command that
//! reads the vocabulary showed it exactly as before until something looked for the marker.
//!
//! The test runs the real binary as a child process, retires a term through the candidate seam
//! exactly as an operator would, and then asks the five browse commands what they now say about
//! it. Against the real binary rather than the report functions, because the thing being checked
//! is that the marker survives the round trip through the store and reaches a person's terminal.

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
const MORSE: &str = "https://example.org/thesaurus/morse";
const TELEGRAPHY: &str = "https://example.org/thesaurus/telegraphy";

/// The same thesaurus `deprecate_concept.rs` uses: a term with a parent, a live child, a scheme it
/// heads and a collection listing it — everything a retirement leaves standing.
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

/// The report of a command that must succeed.
fn read(dir: &Path, args: &[&str]) -> String {
    let output = run(dir, args);
    assert!(output.status.success(), "{}", stderr(&output));
    stdout(&output)
}

/// A store with the thesaurus populated and `Wireless telegraphy` retired in favour of `Radio` —
/// through `openbiz deprecate` and `openbiz approve`, exactly as an operator would.
fn retired() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("seed.nq"), BACKUP).expect("write the fixture");
    let restored = run(dir.path(), &["restore", "seed.nq"]);
    assert!(restored.status.success(), "{}", stderr(&restored));

    std::fs::write(dir.path().join("import.ttl"), CONCEPTS).expect("write the import");
    let imported = run(dir.path(), &["import", THESAURUS, "import.ttl"]);
    assert!(imported.status.success(), "{}", stderr(&imported));
    let approved = run(dir.path(), &["approve", "1"]);
    assert!(approved.status.success(), "{}", stderr(&approved));

    let proposed = run(
        dir.path(),
        &["deprecate", THESAURUS, WIRELESS, "--replaced-by", RADIO],
    );
    assert!(proposed.status.success(), "{}", stderr(&proposed));
    let approved = run(dir.path(), &["approve", "2"]);
    assert!(approved.status.success(), "{}", stderr(&approved));
    dir
}

/// The gap this closes, stated as the test that would have failed before it: a term retired
/// through the command still reads as current in the command an author browses with.
#[test]
fn the_tree_marks_the_retired_concept_and_names_what_replaces_it() {
    let dir = retired();
    let report = read(dir.path(), &["tree", THESAURUS, WIRELESS]);

    assert!(report.contains("[retired]"), "{report}");
    assert!(
        report.contains("the vocabulary marks it owl:deprecated"),
        "{report}"
    );
    assert!(report.contains(RADIO), "the successor is named: {report}");
    assert!(
        report.contains("1 of the 1 concept(s) below it are not retired"),
        "the live child the retirement left behind is counted: {report}"
    );
}

/// The retired concept is still the child's parent, because nothing was removed — and the child is
/// now told, which is the fact `openbiz deprecate` could report only at the moment of retiring.
#[test]
fn ancestors_of_a_live_child_report_the_retired_parent() {
    let dir = retired();
    let report = read(dir.path(), &["ancestors", THESAURUS, MORSE]);

    assert!(report.contains(WIRELESS), "{report}");
    assert!(report.contains("[retired]"), "{report}");
    assert!(
        report.contains("of the concept(s) above it are retired"),
        "{report}"
    );
    // Telegraphy is above Morse too and is current, so the mark distinguishes rather than decorates.
    assert!(report.contains(TELEGRAPHY), "{report}");
}

/// A breadcrumb built from these routes would offer a reader a term the vocabulary has retired.
#[test]
fn paths_name_the_retired_concept_the_routes_run_through() {
    let dir = retired();
    let report = read(dir.path(), &["paths", THESAURUS, MORSE]);

    assert!(
        report.contains("1 concept(s) on these routes are retired:"),
        "{report}"
    );
    assert!(report.contains("the routes above still hold"), "{report}");
}

/// The command where this matters most: someone looking for a term to reuse is told the one they
/// found is obsolete, and which to use instead — rather than reusing it, or being shown nothing
/// and creating a duplicate.
#[test]
fn search_shows_the_retired_concept_and_says_what_to_use_instead() {
    let dir = retired();
    let report = read(dir.path(), &["search", THESAURUS, "telegraphy"]);

    assert!(
        report.contains(WIRELESS),
        "it is shown, not hidden: {report}"
    );
    assert!(report.contains("[retired]"), "{report}");
    assert!(
        report.contains("use instead, by dcterms:isReplacedBy"),
        "{report}"
    );
    assert!(report.contains(RADIO), "{report}");
    assert!(report.contains("concept(s) shown are retired"), "{report}");
}

/// The whole-vocabulary view: what has been retired, and what each retirement left for a person.
#[test]
fn inspect_reports_the_retirement_and_what_it_left_standing() {
    let dir = retired();
    let report = read(dir.path(), &["inspect", THESAURUS]);

    assert!(report.contains("\nretirements:\n"), "{report}");
    assert!(
        report.contains("1 resource(s) marked owl:deprecated"),
        "{report}"
    );
    assert!(
        report.contains("1 of them still have concepts directly below them that are not retired"),
        "{report}"
    );
    assert!(
        report.contains("1 of them are still a scheme's top concept"),
        "the retired concept still heads the scheme's browse tree: {report}"
    );
    // A count and not a complaint: leaving them is the write half's deliberate decision.
    assert!(report.contains("\nfindings: 0\n"), "{report}");
}

/// The opt-in half of `docs/adr/0041`, end to end: a curator who asks for current concepts only
/// gets a list without the obsolete terms in it.
#[test]
fn search_current_only_leaves_the_retired_concept_out() {
    let dir = retired();
    let report = read(
        dir.path(),
        &["search", THESAURUS, "telegraphy", "--current"],
    );

    assert!(
        report.contains("current concepts only"),
        "the narrowing is stated before the counts it changes: {report}"
    );
    assert!(
        report.contains(TELEGRAPHY),
        "the current concept is still found: {report}"
    );
    assert!(
        !report.contains(WIRELESS),
        "the retired concept is the one thing that was asked to go: {report}"
    );
    // "Wireless telegraphy"@en and "Radiotelegraphy"@en, both on the one retired concept.
    assert!(
        report.contains("2 more label(s) matched, on 1 retired concept(s)"),
        "{report}"
    );
}

/// **The failure the flag would otherwise reintroduce.** Everything that matched was retired, so
/// without the withheld count this report would tell someone looking for a term to reuse that the
/// vocabulary has never heard of it — which is how the duplicate gets created (`CLAUDE.md` §1.7).
#[test]
fn search_current_only_still_admits_that_a_retired_concept_matched() {
    let dir = retired();
    let report = read(
        dir.path(),
        &["search", THESAURUS, "radiotelegraphy", "--current"],
    );

    assert!(report.contains("nothing matched"), "{report}");
    assert!(
        report.contains("1 more label(s) matched, on 1 retired concept(s)"),
        "{report}"
    );
    assert!(
        report.contains("run the same search without --current"),
        "the way to see them is in the report, not in the manual: {report}"
    );

    // And the way back works, from this same store, without any other change.
    let shown = read(dir.path(), &["search", THESAURUS, "radiotelegraphy"]);
    assert!(shown.contains(WIRELESS), "{shown}");
    assert!(shown.contains(RADIO), "the successor is named: {shown}");
}
