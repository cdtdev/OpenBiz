//! The record `adr/0003` §3 requires, end to end, against the real binary as a child process.
//!
//! The claim only a child process can make is the one the item is about: a justification written
//! by one invocation is **on disk**, and a later, separate invocation — and a SPARQL query that
//! knows nothing about either command — can be asked which concepts were created despite an
//! existing match. A record that lived in memory would pass every unit test in this repository and
//! answer nothing to an auditor.

use std::path::Path;
use std::process::{Command, Output};

/// A store holding one empty vocabulary, as an operator could type it.
const BACKUP: &str = concat!(
    "<urn:openbiz:store> <urn:openbiz:storeFormatVersion> ",
    "\"4\"^^<http://www.w3.org/2001/XMLSchema#integer> <urn:openbiz:graph:system> .\n",
    "<urn:openbiz:graph:system> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<urn:openbiz:graph:system> <urn:openbiz:graphKind> \"system\" <urn:openbiz:graph:system> .\n",
    "<https://example.org/energy> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<https://example.org/energy> <urn:openbiz:graphKind> \"vocabulary\" ",
    "<urn:openbiz:graph:system> .\n",
    "<https://example.org/materials> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<https://example.org/materials> <urn:openbiz:graphKind> \"vocabulary\" ",
    "<urn:openbiz:graph:system> .\n",
);

/// A second registered vocabulary, so the narrowing test has somewhere with nothing recorded.
const MATERIALS: &str = "https://example.org/materials";

/// The vocabulary the fixture registers.
const ENERGY: &str = "https://example.org/energy";

/// The IRI of the concept already called "Solar power", which every test below passes over.
const EXISTING: &str = "https://example.org/energy/c_3";

/// A vocabulary that already has a concept called "Solar power".
const CONCEPTS: &str = r#"
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex: <https://example.org/energy/> .

ex:c_1 a skos:Concept ; skos:prefLabel "Renewable energy"@en .
ex:c_3 a skos:Concept ; skos:prefLabel "Solar power"@en .
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

/// Succeed, check the report is laid out, and hand back stdout.
fn ok(dir: &Path, args: &[&str]) -> String {
    let output = run(dir, args);
    assert!(
        output.status.success(),
        "`openbiz {}` failed: {}",
        args.join(" "),
        stderr(&output)
    );
    let report = stdout(&output);
    well_spaced(&report, &args.join(" "));
    report
}

/// No line whose spacing is a swallowed line continuation rather than a layout.
///
/// This is a crude assertion and it is here because the defect it catches has now reached
/// user-facing output **three iterations running** (54, 55, and this one), each time the same way:
/// a Rust line continuation eaten by the tool the source was written through, leaving a wall of
/// spaces where a single space belonged. It is invisible to every assertion that checks for a
/// substring, which is why noticing it three times was luck rather than testing.
///
/// Two rules, because the defect lands in two places and the first version of this guard only
/// caught one of them — a continuation swallowed at the *start* of a line looks exactly like
/// indentation until you bound how deep indentation is allowed to go.
///
/// 1. **Indentation is at most six spaces.** These reports nest three levels, two spaces each. A
///    swallowed continuation carries the Rust source's own indentation, which is far deeper — the
///    three real occurrences were 19, 22, and 26.
/// 2. **No run of three or more spaces after that.** Three and not two, because a match line
///    legitimately separates its columns with two.
fn well_spaced(report: &str, what: &str) {
    for line in report.lines() {
        let indent = line.len() - line.trim_start().len();
        assert!(
            indent <= 6,
            "`openbiz {what}` printed a line indented {indent} spaces, which is deeper than any \
             layout in these reports and is what a swallowed line continuation looks like: \
             {line:?}"
        );
        assert!(
            !line.trim_start().contains("   "),
            "`openbiz {what}` printed a line with a run of spaces in it, which is a swallowed \
             line continuation and not a layout: {line:?}"
        );
    }
}

/// A store whose vocabulary holds [`CONCEPTS`], having arrived through the seam and been approved.
fn authored() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("seed.nq"), BACKUP).expect("write the fixture");
    let restored = run(dir.path(), &["restore", "seed.nq"]);
    assert!(
        restored.status.success(),
        "the fixture did not restore: {}",
        stderr(&restored)
    );

    std::fs::write(dir.path().join("concepts.ttl"), CONCEPTS).expect("write the import");
    let imported = run(dir.path(), &["import", ENERGY, "concepts.ttl"]);
    assert!(
        imported.status.success(),
        "the import failed: {}",
        stderr(&imported)
    );
    let id = stdout(&imported)
        .split_whitespace()
        .nth(2)
        .expect("the import names the candidate it raised")
        .to_owned();
    let approved = run(dir.path(), &["approve", &id]);
    assert!(
        approved.status.success(),
        "the approval failed: {}",
        stderr(&approved)
    );
    dir
}

/// **The item's whole point.** One process records; a second, separate process reads it back and
/// says what was passed over, by whom, and why.
#[test]
fn a_justification_recorded_by_one_invocation_is_read_by_the_next() {
    let dir = authored();

    let minted = ok(
        dir.path(),
        &[
            "mint",
            ENERGY,
            "Solar power",
            "--because",
            "the existing concept is a funding programme, not the technology",
        ],
    );
    assert!(
        minted.contains("STOP"),
        "discovery still runs first: {minted}"
    );
    assert!(minted.contains("justification 1 recorded"), "{minted}");
    assert!(minted.contains(EXISTING), "{minted}");
    assert!(
        minted.contains("the justification above is the only thing written"),
        "with the flag the report must not still claim nothing was written: {minted}"
    );

    let read = ok(dir.path(), &["justifications"]);
    assert!(
        read.contains("1 record(s), of which 1 created something despite an existing match"),
        "{read}"
    );
    assert!(read.contains(EXISTING), "what was passed over: {read}");
    assert!(
        read.contains("the existing concept is a funding programme, not the technology"),
        "the reason: {read}"
    );
    assert!(read.contains("ada@example.org"), "who decided: {read}");
}

/// The auditor's question, in the shape it has to be in to be askable. The store's own tests run
/// the SPARQL join; what a child process proves is that the join's raw material is **on disk**,
/// with the resource passed over in the object position as an IRI rather than as prose. This is
/// the difference between the record and the note `adr/0003` §3 rules out, and if it regresses the
/// item has quietly become the thing it replaced.
#[test]
fn what_was_passed_over_is_on_disk_as_a_joinable_iri() {
    let dir = authored();

    ok(
        dir.path(),
        &[
            "mint",
            ENERGY,
            "Solar power",
            "--because",
            "a different sense of the term",
        ],
    );
    ok(
        dir.path(),
        &[
            "mint",
            ENERGY,
            "Tidal power",
            "--because",
            "nothing was found under this name",
        ],
    );

    ok(dir.path(), &["backup", "out.nq"]);
    let written = std::fs::read_to_string(dir.path().join("out.nq")).expect("the backup is there");

    let considered: Vec<&str> = written
        .lines()
        .filter(|line| line.contains("<urn:openbiz:justificationConsidered>"))
        .collect();
    assert_eq!(
        considered.len(),
        1,
        "only the creation that passed something over records one: {considered:?}"
    );
    assert!(
        considered[0].contains(&format!(
            "<urn:openbiz:justificationConsidered> <{EXISTING}>"
        )),
        "an IRI in the object position, not a literal: {}",
        considered[0]
    );
    assert!(
        written.contains("<urn:openbiz:graph:system> .")
            && considered[0].ends_with("<urn:openbiz:graph:system> ."),
        "the record is in OpenBiz's own graph and not in the vocabulary: {}",
        considered[0]
    );
}

/// A creation with nothing found is still recorded, and the report says which kind it is. An
/// auditor reading "nothing existing was found" is being told somebody looked — which is a
/// different fact from no record at all.
#[test]
fn a_creation_with_nothing_found_is_recorded_and_distinguished() {
    let dir = authored();

    let minted = ok(
        dir.path(),
        &[
            "mint",
            ENERGY,
            "Tidal power",
            "--because",
            "no existing concept covers this",
        ],
    );
    assert!(
        minted.contains("naming nothing passed over"),
        "the report distinguishes the two: {minted}"
    );

    let read = ok(dir.path(), &["justifications"]);
    assert!(
        read.contains("1 record(s), of which 0 created something despite an existing match"),
        "{read}"
    );
    assert!(read.contains("nothing existing was found"), "{read}");
}

/// Without the flag, `openbiz mint` writes nothing — which is what it did before this existed, and
/// what every existing script depends on.
#[test]
fn minting_without_the_flag_still_writes_nothing() {
    let dir = authored();

    let minted = ok(dir.path(), &["mint", ENERGY, "Solar power"]);
    assert!(
        minted.contains("--because"),
        "the ladder names the flag that records the reason: {minted}"
    );
    assert!(!minted.contains("justification 1 recorded"), "{minted}");
    assert!(
        minted.contains("nothing was written and nothing is reserved"),
        "without the flag the report claims the stronger of the two truths: {minted}"
    );

    let read = ok(dir.path(), &["justifications"]);
    assert!(read.contains("nothing is recorded"), "{read}");
}

/// The empty report must not read as a clean bill of health, because nothing enforces the record.
#[test]
fn an_empty_report_says_what_it_cannot_tell_apart() {
    let dir = authored();

    let read = ok(dir.path(), &["justifications"]);
    assert!(
        read.contains("look identical from here"),
        "an empty governance report that reads as reassurance is the failure: {read}"
    );
    assert!(
        read.contains("nothing in this build refuses a creation that has no justification"),
        "{read}"
    );
}

/// `--because` with no label is refused: there was no discovery pass, so there is nothing found to
/// name, and a record written anyway would file the appearance of diligence as evidence of it.
#[test]
fn a_justification_with_nothing_looked_for_is_refused() {
    let dir = authored();

    let refused = run(
        dir.path(),
        &["mint", ENERGY, "--because", "we need a new one"],
    );
    assert!(!refused.status.success(), "{}", stdout(&refused));
    assert!(
        stderr(&refused).contains("with no label nothing was looked for"),
        "{}",
        stderr(&refused)
    );

    let read = ok(dir.path(), &["justifications"]);
    assert!(
        read.contains("nothing is recorded"),
        "the refusal wrote nothing: {read}"
    );
}

/// A record whose search stopped at its bound says so, in the record and in both reports.
///
/// The bound is the reachable half of "the search did not finish" — a source that will not answer
/// is the other half, and this build has only one source, which always answers. Without this the
/// flag would be a field nothing could ever set to `false`, which is a field that lies by
/// construction: every record would claim a complete search because no code path could produce an
/// incomplete one.
#[test]
fn a_search_that_stopped_at_its_bound_is_recorded_as_unfinished() {
    let dir = authored();

    // More concepts sharing one label than the discovery bound will list.
    let mut crowd = String::from(
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/energy/> .\n",
    );
    for n in 100..130 {
        crowd.push_str(&format!(
            "ex:c_{n} a skos:Concept ; skos:prefLabel \"Solar power {n}\"@en .\n"
        ));
    }
    std::fs::write(dir.path().join("crowd.ttl"), crowd).expect("write the import");
    let imported = ok(dir.path(), &["import", ENERGY, "crowd.ttl"]);
    let id = imported
        .split_whitespace()
        .nth(2)
        .expect("the import names its candidate")
        .to_owned();
    ok(dir.path(), &["approve", &id]);

    let minted = ok(
        dir.path(),
        &[
            "mint",
            ENERGY,
            "Solar power",
            "--because",
            "none of the thirty is the one meant",
        ],
    );
    assert!(
        minted.contains("the search behind it did not finish"),
        "the mint report must not present a truncated search as a finished one: {minted}"
    );

    let read = ok(dir.path(), &["justifications"]);
    assert!(
        read.contains("the search behind this record did not finish"),
        "{read}"
    );
    assert!(
        read.contains("rest on a search that did not finish"),
        "the summary counts them, so a reader sees it without reading every entry: {read}"
    );
}

/// Narrowing to a vocabulary answers the smaller question without hiding that the store holds
/// more — an empty per-vocabulary report beside a store full of records is exactly where somebody
/// concludes the wrong thing.
#[test]
fn narrowing_to_one_vocabulary_still_says_what_the_store_holds() {
    let dir = authored();

    ok(
        dir.path(),
        &[
            "mint",
            ENERGY,
            "Solar power",
            "--because",
            "a distinct sense",
        ],
    );

    let mine = ok(dir.path(), &["justifications", ENERGY]);
    assert!(mine.contains("1 record(s)"), "{mine}");

    let theirs = ok(dir.path(), &["justifications", MATERIALS]);
    assert!(
        theirs.contains("the store holds 1 record(s) for others"),
        "{theirs}"
    );
}

/// Recording twice appends rather than replacing: a justification is a statement made at a time,
/// and an audit trail that overwrites its own entries is not one.
#[test]
fn a_second_justification_does_not_replace_the_first() {
    let dir = authored();

    for reason in ["a distinct sense", "and on reflection, still distinct"] {
        ok(
            dir.path(),
            &["mint", ENERGY, "Solar power", "--because", reason],
        );
    }

    let read = ok(dir.path(), &["justifications"]);
    assert!(read.contains("2 record(s)"), "{read}");
    assert!(read.contains("a distinct sense"), "{read}");
    assert!(read.contains("and on reflection, still distinct"), "{read}");
}
