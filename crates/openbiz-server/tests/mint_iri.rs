//! `openbiz mint` end to end, against the real binary as a child process.
//!
//! The crate's own tests prove the rules against models built in-process. This file proves the
//! only thing a curator actually has: a program, with arguments and an exit status, reading a
//! vocabulary off disk — and the one claim that no in-process test can make, which is that a
//! minted IRI survives being written back through `openbiz import` and comes out of the store the
//! same IRI it went in as. That matters here more than it would elsewhere, because this build
//! mints IRIs with non-ASCII characters in them on purpose (RFC 3987 §2.2), and "it is a legal
//! IRI" and "this store round-trips it" are two different claims.

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
);

/// The vocabulary IRI the fixture registers.
const ENERGY: &str = "https://example.org/energy";

/// A vocabulary that numbers its concepts, with a gap at 2 that must never be filled.
const CONCEPTS: &str = r#"
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex: <https://example.org/energy/> .

ex:c_1 a skos:Concept ; skos:prefLabel "Renewable energy"@en .
ex:c_3 a skos:Concept ; skos:prefLabel "Solar power"@en .
ex:c_12 a skos:Concept ; skos:prefLabel "Wind power"@en .
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
    import_and_approve(dir.path(), "concepts.ttl", CONCEPTS);
    dir
}

/// Propose `turtle` and approve it, and answer with the candidate's identifier.
fn import_and_approve(data_dir: &Path, file: &str, turtle: &str) -> String {
    let id = import(data_dir, file, turtle);
    let approved = run(data_dir, &["approve", &id]);
    assert!(
        approved.status.success(),
        "the approval failed: {}",
        stderr(&approved)
    );
    id
}

/// Propose `turtle` and leave it waiting for a decision.
fn import(data_dir: &Path, file: &str, turtle: &str) -> String {
    std::fs::write(data_dir.join(file), turtle).expect("write the import");
    let imported = run(data_dir, &["import", ENERGY, file]);
    assert!(
        imported.status.success(),
        "the import failed: {}",
        stderr(&imported)
    );
    stdout(&imported)
        .split_whitespace()
        .nth(2)
        .expect("the import names the candidate it raised")
        .to_owned()
}

/// The command as a curator runs it: no pattern, no configuration, one term.
#[test]
fn mint_reads_the_pattern_off_the_vocabulary_and_goes_above_the_highest_number() {
    let dir = authored();

    let output = run(dir.path(), &["mint", ENERGY, "Tidal power"]);
    assert!(output.status.success(), "mint failed: {}", stderr(&output));
    let report = stdout(&output);

    assert!(
        report.contains("pattern: https://example.org/energy/c_{n}"),
        "{report}"
    );
    assert!(
        report.contains("minted: https://example.org/energy/c_13"),
        "the gap at 2 is not filled: {report}"
    );
    assert!(
        report.contains("nothing was written and nothing is reserved"),
        "{report}"
    );
}

/// **The claim only the real store can settle.** A readable IRI minted from an accented label is
/// a legal IRI under RFC 3987 §2.2, and this proves the store takes it, keeps it, and gives it
/// back — and that minting it a second time then finds it taken.
#[test]
fn a_minted_iri_survives_being_imported_and_is_taken_afterwards() {
    let dir = authored();

    let minted = run(
        dir.path(),
        &[
            "mint",
            ENERGY,
            "Énergie marémotrice",
            "--pattern",
            "https://example.org/energy/{slug}",
        ],
    );
    assert!(minted.status.success(), "mint failed: {}", stderr(&minted));
    let iri = "https://example.org/energy/énergie-marémotrice";
    assert!(
        stdout(&minted).contains(&format!("minted: {iri}")),
        "{}",
        stdout(&minted)
    );

    import_and_approve(
        dir.path(),
        "new.ttl",
        &format!(
            "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
             <{iri}> a skos:Concept ; skos:prefLabel \"Énergie marémotrice\"@fr .\n"
        ),
    );

    // Out of the store again, byte for byte the IRI that went in.
    let exported = run(dir.path(), &["search", ENERGY, "marémotrice"]);
    assert!(
        exported.status.success(),
        "search failed: {}",
        stderr(&exported)
    );
    assert!(stdout(&exported).contains(iri), "{}", stdout(&exported));

    // And minting it again now refuses, rather than handing out the IRI a concept holds.
    let again = run(
        dir.path(),
        &[
            "mint",
            ENERGY,
            "Énergie marémotrice",
            "--pattern",
            "https://example.org/energy/{slug}",
        ],
    );
    let report = stdout(&again);
    assert!(report.contains("nothing was minted"), "{report}");
    assert!(report.contains("already in use"), "{report}");
    assert!(
        report.contains("no disambiguating suffix is offered"),
        "{report}"
    );
}

/// A change staged and not yet decided holds its IRIs, so two curators preparing imports on the
/// same day cannot mint the same IRI and silently merge two concepts on approval.
#[test]
fn a_change_waiting_for_a_decision_holds_its_iris() {
    let dir = authored();
    import(
        dir.path(),
        "staged.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         <https://example.org/energy/c_13> a skos:Concept ; skos:prefLabel \"Tidal power\"@en .\n",
    );

    let output = run(dir.path(), &["mint", ENERGY, "Tidal power"]);
    assert!(output.status.success(), "mint failed: {}", stderr(&output));
    let report = stdout(&output);

    assert!(
        report.contains("minted: https://example.org/energy/c_14"),
        "{report}"
    );
    assert!(
        report.contains("waiting for a decision"),
        "the report says where the taken IRI was found: {report}"
    );
    // Both halves of the report read the staged change, so they cannot contradict each other.
    assert!(report.contains("STOP"), "{report}");
    assert!(report.contains("in candidate 2"), "{report}");
}

/// The store is untouched. A command that only reads must leave the bytes alone, and the only
/// honest way to claim that is to compare the store before and after.
#[test]
fn mint_writes_nothing() {
    let dir = authored();

    let before = run(dir.path(), &["backup", "before.nq"]);
    assert!(
        before.status.success(),
        "backup failed: {}",
        stderr(&before)
    );

    let output = run(dir.path(), &["mint", ENERGY, "Tidal power"]);
    assert!(output.status.success(), "mint failed: {}", stderr(&output));

    let after = run(dir.path(), &["backup", "after.nq"]);
    assert!(after.status.success(), "backup failed: {}", stderr(&after));

    let one = std::fs::read_to_string(dir.path().join("before.nq")).expect("the first backup");
    let two = std::fs::read_to_string(dir.path().join("after.nq")).expect("the second backup");
    let mut one: Vec<&str> = one.lines().collect();
    let mut two: Vec<&str> = two.lines().collect();
    one.sort_unstable();
    two.sort_unstable();
    assert_eq!(one, two, "mint changed the store");
}

/// A vocabulary that does not say what its IRIs look like is refused with a non-zero status, so a
/// script cannot mistake "I could not decide" for an IRI.
#[test]
fn a_vocabulary_with_no_convention_fails_rather_than_guessing() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("seed.nq"), BACKUP).expect("write the fixture");
    run(dir.path(), &["restore", "seed.nq"]);
    import_and_approve(
        dir.path(),
        "spread.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         <https://a.example/one> a skos:Concept .\n\
         <https://b.example/two> a skos:Concept .\n\
         <https://c.example/three> a skos:Concept .\n",
    );

    let output = run(dir.path(), &["mint", ENERGY, "Tidal power"]);
    assert!(!output.status.success(), "{}", stdout(&output));
    assert!(
        stderr(&output).contains("give one with --pattern"),
        "{}",
        stderr(&output)
    );
}
