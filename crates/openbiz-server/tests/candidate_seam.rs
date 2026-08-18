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

/// One statement out of [`CONCEPTS`], as the file an operator would hand to `openbiz retract`:
/// the hierarchy link they have decided is wrong.
const WRONG_LINK: &str = concat!(
    "<https://example.org/regions/apac> ",
    "<http://www.w3.org/2004/02/skos/core#broader> ",
    "<https://example.org/regions/emea> .\n",
);

/// Seed a store and get the concepts *into* the vocabulary, so there is something to remove.
fn seeded_and_populated() -> tempfile::TempDir {
    let dir = seeded();
    let imported = run(dir.path(), &["import", REGIONS, "concepts.ttl"]);
    assert!(imported.status.success(), "{}", stderr(&imported));
    let approved = run(dir.path(), &["approve", "1"]);
    assert!(approved.status.success(), "{}", stderr(&approved));
    std::fs::write(dir.path().join("wrong-link.nt"), WRONG_LINK).expect("write the retraction");
    dir
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

/// The removing half of the seam, walked the way the adding half is: propose, read, decide.
#[test]
fn a_retraction_is_proposed_reviewed_and_only_then_applied() {
    let dir = seeded_and_populated();

    let proposed = run(dir.path(), &["retract", REGIONS, "wrong-link.nt"]);
    assert!(
        proposed.status.success(),
        "the retraction failed: {}",
        stderr(&proposed)
    );
    let said = stdout(&proposed);
    assert!(
        said.contains("candidate 2") && said.contains("removes 1 statement"),
        "the retraction must say what it proposed and under what number: {said}"
    );
    assert!(
        said.contains("nothing has been written to the vocabulary"),
        "an operator must be told the vocabulary is untouched: {said}"
    );

    // Proposing removed nothing: all five statements are still there.
    let after_proposal = backup(dir.path(), "after-proposal.nq");
    assert_eq!(
        statements_in(&after_proposal, REGIONS),
        5,
        "proposing a removal must not remove anything"
    );
    assert_eq!(
        statements_in(&after_proposal, "urn:openbiz:graph:candidate:2:removals"),
        1,
        "the statement to be removed must be staged where a reviewer can read it"
    );

    // The review says which way the change runs. "1 statement" without "remove" is not a review.
    let shown = stdout(&run(dir.path(), &["candidate", "2"]));
    for expected in [
        "removes 1 statement",
        "would remove",
        "urn:openbiz:graph:candidate:2:removals",
        "broader",
        "ada@example.org",
    ] {
        assert!(
            shown.contains(expected),
            "a reviewer needs {expected:?} to decide, and it is not in: {shown}"
        );
    }
    assert!(
        !shown.contains("would add"),
        "a candidate that adds nothing must not offer an empty additions section: {shown}"
    );

    let listed = stdout(&run(dir.path(), &["candidates"]));
    assert!(
        listed.contains("removes 1 statement"),
        "the list must distinguish a removal from an addition: {listed}"
    );

    let approved = run(dir.path(), &["approve", "2"]);
    assert!(
        approved.status.success(),
        "the approval failed: {}",
        stderr(&approved)
    );
    assert!(
        stdout(&approved).contains("removes 1 statement")
            && stdout(&approved).contains("ada@example.org"),
        "an approval must say what it did and who took it: {}",
        stdout(&approved)
    );

    let after_approval = backup(dir.path(), "after-approval.nq");
    assert_eq!(
        statements_in(&after_approval, REGIONS),
        4,
        "approval is what takes the statement out of the vocabulary"
    );
    assert!(
        !after_approval.contains(&format!(
            "skos/core#broader> <https://example.org/regions/emea> <{REGIONS}>"
        )),
        "and it is the proposed statement that went: {after_approval}"
    );
    assert_eq!(
        statements_in(&after_approval, "urn:openbiz:graph:candidate:2:removals"),
        1,
        "an approved removal is the one change the vocabulary no longer records, so the staged \
         evidence must outlive it"
    );
}

/// A removal the vocabulary cannot satisfy is refused where the operator is standing, with a
/// non-zero status a script can act on.
#[test]
fn retracting_a_statement_the_vocabulary_does_not_hold_is_refused() {
    let dir = seeded_and_populated();
    std::fs::write(
        dir.path().join("absent.nt"),
        "<https://example.org/regions/apac> \
         <http://www.w3.org/2004/02/skos/core#prefLabel> \"Asia Pacific\"@en .\n",
    )
    .expect("write the file");

    let refused = run(dir.path(), &["retract", REGIONS, "absent.nt"]);
    assert!(
        !refused.status.success(),
        "a removal that matches nothing must fail: {}",
        stdout(&refused)
    );
    let said = stderr(&refused);
    assert!(
        said.contains("are not in") && said.contains("Asia Pacific"),
        "and must show what was missing: {said}"
    );

    let after = backup(dir.path(), "after-refusal.nq");
    assert_eq!(statements_in(&after, REGIONS), 5, "nothing may have moved");
    assert!(
        !stdout(&run(dir.path(), &["candidates"])).contains("candidate 2"),
        "and no candidate may have been left behind"
    );
}

/// The stale-approval refusal, end to end: the vocabulary moves under a pending review.
#[test]
fn approving_a_retraction_the_vocabulary_has_outgrown_is_refused_rather_than_half_applied() {
    let dir = seeded_and_populated();

    // Two reviewers raise the same removal before either is decided.
    for _ in 0..2 {
        let proposed = run(dir.path(), &["retract", REGIONS, "wrong-link.nt"]);
        assert!(proposed.status.success(), "{}", stderr(&proposed));
    }
    let applied = run(dir.path(), &["approve", "2"]);
    assert!(applied.status.success(), "{}", stderr(&applied));

    let refused = run(dir.path(), &["approve", "3"]);
    assert!(
        !refused.status.success(),
        "approving a removal the vocabulary has outgrown must fail: {}",
        stdout(&refused)
    );
    let said = stderr(&refused);
    assert!(
        said.contains("no longer there") && said.contains("propose the removal again"),
        "and must say what happened and what to do about it: {said}"
    );

    // The candidate is still open, so it can be closed deliberately rather than left half-decided.
    let shown = stdout(&run(dir.path(), &["candidate", "3"]));
    assert!(
        shown.contains("proposed"),
        "a refused approval must leave the candidate open: {shown}"
    );
    let rejected = run(dir.path(), &["reject", "3"]);
    assert!(
        rejected.status.success(),
        "and a candidate that can no longer be applied must still be closeable: {}",
        stderr(&rejected)
    );

    let after = backup(dir.path(), "after.nq");
    assert_eq!(
        statements_in(&after, REGIONS),
        4,
        "the first approval stands and the second changed nothing"
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
    for command in [
        "import",
        "retract",
        "candidates",
        "candidate",
        "approve",
        "reject",
    ] {
        assert!(
            help.contains(command),
            "usage does not mention {command}, so nobody can discover it"
        );
    }
}
