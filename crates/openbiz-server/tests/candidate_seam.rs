//! The candidate seam end to end, against the real `openbiz` binary as a child process.
//!
//! `CLAUDE.md` §3 says a change to a vocabulary arrives as a *candidate* a human reviews before it
//! lands. The store's own tests prove the transaction; this file proves the thing an operator
//! actually has: a program, with arguments, exit statuses, and output they can read before
//! deciding. The distinction matters here more than it does for backup — a review path that is
//! technically correct and unreadable is one nobody will use, and an approval taken without
//! reading the statements is not a review.
//!
//! The store is seeded from a hand-written backup rather than by a "create vocabulary" command,
//! because there is not one: creating a vocabulary runs through discovery with a recorded
//! justification (§1.7) and that path belongs to Phase 2's later items. The fixture is written
//! from the specification, as `backup_restore.rs` explains.

use std::path::Path;
use std::process::{Command, Output};

/// A store holding one empty vocabulary, as an operator could type it.
const BACKUP: &str = concat!(
    "<urn:openbiz:store> <urn:openbiz:storeFormatVersion> ",
    "\"3\"^^<http://www.w3.org/2001/XMLSchema#integer> <urn:openbiz:graph:system> .\n",
    "<urn:openbiz:graph:system> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<urn:openbiz:graph:system> <urn:openbiz:graphKind> \"system\" <urn:openbiz:graph:system> .\n",
    "<https://example.org/regions> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<https://example.org/regions> <urn:openbiz:graphKind> \"vocabulary\" ",
    "<urn:openbiz:graph:system> .\n",
);

/// The vocabulary IRI the fixture registers.
const REGIONS: &str = "https://example.org/regions";

/// What an operator would be importing: two concepts, in the readable syntax.
const CONCEPTS: &str = r#"
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex: <https://example.org/regions/> .

ex:emea a skos:Concept ;
    skos:prefLabel "Europe, Middle East and Africa"@en .

ex:apac a skos:Concept ;
    skos:prefLabel "Asia-Pacific"@en ;
    skos:broader ex:emea .
"#;

/// Run `openbiz <args>` against `data_dir`, with a named actor, and wait for it to finish.
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

/// A store with one registered, empty vocabulary in it.
fn seeded() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("seed.nq"), BACKUP).expect("write the fixture");
    let restored = run(dir.path(), &["restore", "seed.nq"]);
    assert!(
        restored.status.success(),
        "the fixture did not restore: {}",
        stderr(&restored)
    );
    std::fs::write(dir.path().join("concepts.ttl"), CONCEPTS).expect("write the import");
    dir
}

/// Back the store up and return the file, so a claim about what landed is a claim about disk.
fn backup(dir: &Path, name: &str) -> String {
    let output = run(dir, &["backup", name]);
    assert!(
        output.status.success(),
        "the backup failed: {}",
        stderr(&output)
    );
    std::fs::read_to_string(dir.join(name)).expect("read the backup back")
}

/// How many statements the backup holds in `graph`.
fn statements_in(backup: &str, graph: &str) -> usize {
    backup
        .lines()
        .filter(|line| line.ends_with(&format!("<{graph}> .")))
        .count()
}

/// The whole review loop, in the order an operator would walk it.
#[test]
fn an_import_is_proposed_reviewed_and_only_then_applied() {
    let dir = seeded();

    let imported = run(dir.path(), &["import", REGIONS, "concepts.ttl"]);
    assert!(
        imported.status.success(),
        "the import failed: {}",
        stderr(&imported)
    );
    let said = stdout(&imported);
    assert!(
        said.contains("candidate 1") && said.contains("5 statements"),
        "the import must say what it proposed and under what number: {said}"
    );
    assert!(
        said.contains("nothing has been written to the vocabulary"),
        "an operator must be told the vocabulary is untouched, not left to infer it: {said}"
    );

    // Nothing reached the vocabulary, and what was proposed is on disk where it can be reviewed.
    let after_import = backup(dir.path(), "after-import.nq");
    assert_eq!(
        statements_in(&after_import, REGIONS),
        0,
        "an unapproved import must not have reached the vocabulary"
    );
    assert_eq!(
        statements_in(&after_import, "urn:openbiz:graph:candidate:1"),
        5,
        "the proposed statements must be staged"
    );

    // The list is what tells somebody there is work waiting.
    let listed = stdout(&run(dir.path(), &["candidates"]));
    assert!(
        listed.contains("proposed") && listed.contains(REGIONS) && listed.contains("concepts.ttl"),
        "the list must say what is waiting, against what, and why: {listed}"
    );

    // The review itself: provenance *and* the statements. Either alone is not a review.
    let shown = stdout(&run(dir.path(), &["candidate", "1"]));
    for expected in [
        "ada@example.org",
        "openbiz import",
        "Asia-Pacific",
        "Europe, Middle East and Africa",
        "urn:openbiz:graph:candidate:1",
    ] {
        assert!(
            shown.contains(expected),
            "a reviewer needs {expected:?} to decide, and it is not in: {shown}"
        );
    }

    let approved = run(dir.path(), &["approve", "1"]);
    assert!(
        approved.status.success(),
        "the approval failed: {}",
        stderr(&approved)
    );
    assert!(
        stdout(&approved).contains("ada@example.org"),
        "an approval must record who took it: {}",
        stdout(&approved)
    );

    let after_approval = backup(dir.path(), "after-approval.nq");
    assert_eq!(
        statements_in(&after_approval, REGIONS),
        5,
        "approval is what puts the statements in the vocabulary"
    );
    assert!(
        after_approval.contains("urn:openbiz:candidateDecidedBy"),
        "the store, not the log, is where the decision is recorded"
    );

    // Deciding twice is refused rather than repeated.
    let again = run(dir.path(), &["approve", "1"]);
    assert!(!again.status.success(), "a second approval must fail");
    assert!(
        stderr(&again).contains("already"),
        "and must say why: {}",
        stderr(&again)
    );
}

#[test]
fn a_rejected_import_leaves_the_vocabulary_alone_and_the_evidence_readable() {
    let dir = seeded();
    run(dir.path(), &["import", REGIONS, "concepts.ttl"]);

    let rejected = run(dir.path(), &["reject", "1"]);
    assert!(
        rejected.status.success(),
        "the rejection failed: {}",
        stderr(&rejected)
    );

    let after = backup(dir.path(), "after-rejection.nq");
    assert_eq!(statements_in(&after, REGIONS), 0);
    assert_eq!(
        statements_in(&after, "urn:openbiz:graph:candidate:1"),
        5,
        "what was refused must stay readable"
    );

    let shown = stdout(&run(dir.path(), &["candidate", "1"]));
    assert!(
        shown.contains("rejected") && shown.contains("ada@example.org"),
        "the record must say it was rejected and by whom: {shown}"
    );
}

#[test]
fn importing_into_a_vocabulary_that_does_not_exist_is_refused() {
    let dir = seeded();
    let output = run(
        dir.path(),
        &[
            "import",
            "https://example.org/never-created",
            "concepts.ttl",
        ],
    );
    assert!(
        !output.status.success(),
        "an import must not create a vocabulary as a side effect (CLAUDE.md §1.7)"
    );
    assert!(
        stderr(&output).contains("https://example.org/never-created"),
        "the refusal must name the graph: {}",
        stderr(&output)
    );
}

#[test]
fn a_file_whose_extension_names_no_syntax_is_refused_rather_than_guessed_at() {
    let dir = seeded();
    std::fs::write(dir.path().join("concepts.txt"), CONCEPTS).expect("write it");

    let output = run(dir.path(), &["import", REGIONS, "concepts.txt"]);
    assert!(!output.status.success(), "an unknown extension must fail");
    let said = stderr(&output);
    assert!(
        said.contains(".ttl") && said.contains(".jsonld"),
        "the refusal must list what we do read: {said}"
    );
}

/// The store refuses an unattributed decision, so the command line has to be able to name someone.
#[test]
fn a_decision_with_nobody_to_record_is_refused_and_says_how_to_fix_it() {
    let dir = seeded();
    run(dir.path(), &["import", REGIONS, "concepts.ttl"]);

    let output = Command::new(env!("CARGO_BIN_EXE_openbiz"))
        .args(["approve", "1"])
        .current_dir(dir.path())
        .env_clear()
        .env("OPENBIZ_DATA_DIR", dir.path())
        .output()
        .expect("run the openbiz binary");

    assert!(
        !output.status.success(),
        "an anonymous approval must not be recorded"
    );
    assert!(
        stderr(&output).contains("OPENBIZ_ACTOR"),
        "the refusal must say how to satisfy it: {}",
        stderr(&output)
    );

    let still_pending = stdout(&run(dir.path(), &["candidate", "1"]));
    assert!(
        still_pending.contains("proposed"),
        "and it must have changed nothing: {still_pending}"
    );
}

#[test]
fn a_candidate_that_never_existed_is_named_rather_than_guessed_at() {
    let dir = seeded();
    for (args, expected) in [
        (["candidate", "9"], "no candidate 9"),
        (["approve", "007"], "007"),
    ] {
        let output = run(dir.path(), &args);
        assert!(!output.status.success(), "{args:?} must fail");
        assert!(
            stderr(&output).contains(expected),
            "{args:?} must say {expected:?}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn the_usage_names_every_command_it_can_parse() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let help = stdout(&run(dir.path(), &["help"]));
    for command in ["import", "candidates", "candidate", "approve", "reject"] {
        assert!(
            help.contains(command),
            "usage does not mention {command}, so nobody can discover it"
        );
    }
}
