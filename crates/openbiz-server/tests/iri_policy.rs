//! `openbiz policy` end to end, against the real binary as a child process.
//!
//! The crate's own tests prove the reports against parts held in hand, and the store's prove the
//! record survives a reopen in one process. The claim only a child process can make is the one the
//! item is actually about: **a pattern recorded by one invocation is what a later, separate
//! invocation of `openbiz mint` mints under.** A policy that lived in memory would pass every unit
//! test in this repository and fail the first thing a deployment does with it.
//!
//! Each `run` below is a fresh process opening the store from disk, so every assertion that crosses
//! two of them is an assertion about what was written down.

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

/// A vocabulary that numbers its concepts, so inference and a readable policy disagree.
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

/// Succeed and hand back stdout.
fn ok(dir: &Path, args: &[&str]) -> String {
    let output = run(dir, args);
    assert!(
        output.status.success(),
        "`openbiz {}` failed: {}",
        args.join(" "),
        stderr(&output)
    );
    stdout(&output)
}

/// The state every existing store is in: nothing recorded, and mint inferring each time.
#[test]
fn a_vocabulary_with_no_recorded_policy_says_so_and_says_what_it_costs() {
    let dir = authored();

    let report = ok(dir.path(), &["policy", ENERGY]);
    assert!(report.contains("nothing is recorded"), "{report}");
    assert!(
        report.contains("its own concepts suggest https://example.org/energy/c_{n}"),
        "{report}"
    );
    assert!(report.contains("moves as they grow"), "{report}");

    // And mint says the same thing where the person minting will see it.
    let minted = ok(dir.path(), &["mint", ENERGY, "Tidal power"]);
    assert!(
        minted.contains("nothing is recorded for this vocabulary"),
        "{minted}"
    );
    assert!(
        minted.contains("minted: https://example.org/energy/c_4"),
        "{minted}"
    );
}

/// The item, end to end and across three processes: record a pattern that *disagrees* with what the
/// vocabulary's concepts suggest, and watch a later, separate `openbiz mint` obey the record rather
/// than the concepts. If the recorded policy were ignored this would mint `c_4`.
#[test]
fn a_recorded_pattern_is_what_a_later_mint_uses() {
    let dir = authored();

    let recorded = ok(
        dir.path(),
        &[
            "policy",
            ENERGY,
            "--pattern",
            "https://example.org/energy/{slug}",
        ],
    );
    assert!(
        recorded.contains("pattern: https://example.org/energy/{slug}"),
        "{recorded}"
    );
    assert!(
        recorded.contains("recorded by ada@example.org at"),
        "{recorded}"
    );
    assert!(
        recorded.contains("suggest https://example.org/energy/c_{n} instead"),
        "the disagreement with the vocabulary is not hidden: {recorded}"
    );
    assert!(
        recorded.contains("nothing already minted changed"),
        "{recorded}"
    );

    let shown = ok(dir.path(), &["policy", ENERGY]);
    assert!(
        shown.contains("recorded: https://example.org/energy/{slug}"),
        "a separate process reads what was written: {shown}"
    );

    let minted = ok(dir.path(), &["mint", ENERGY, "Tidal power"]);
    assert!(
        minted.contains("minted: https://example.org/energy/tidal-power"),
        "the recorded pattern outranks what the concepts suggest: {minted}"
    );
    assert!(
        minted.contains("recorded for this vocabulary by ada@example.org"),
        "and the report says where the pattern came from: {minted}"
    );
}

/// `--pattern` is still a one-off override, and overriding a *recorded* policy is louder than
/// overriding a guess — so the report has to name the recorded one it is stepping over.
#[test]
fn a_pattern_on_the_command_line_overrides_the_recorded_one_for_that_command_only() {
    let dir = authored();
    ok(
        dir.path(),
        &[
            "policy",
            ENERGY,
            "--pattern",
            "https://example.org/energy/{slug}",
        ],
    );

    let minted = ok(
        dir.path(),
        &[
            "mint",
            ENERGY,
            "Tidal power",
            "--pattern",
            "https://example.org/energy/x_{n}",
        ],
    );
    assert!(
        minted.contains("minted: https://example.org/energy/x_1"),
        "{minted}"
    );
    assert!(
        minted.contains("given with --pattern, for this one command"),
        "{minted}"
    );
    // The report has to name the decision it stepped over. Without this it read exactly the same as
    // an override of a vocabulary that had recorded nothing at all, which is the case where nothing
    // is being contradicted — found by running the two side by side.
    assert!(
        minted.contains(
            "records \"https://example.org/energy/{slug}\" instead, set by ada@example.org"
        ),
        "the recorded policy being overridden is named: {minted}"
    );
    assert!(
        minted.contains("that record is unchanged and every other producer still mints under it"),
        "and the override is scoped to this command: {minted}"
    );

    let still = ok(dir.path(), &["policy", ENERGY]);
    assert!(
        still.contains("recorded: https://example.org/energy/{slug}"),
        "a one-off mint changed nothing that was recorded: {still}"
    );
}

/// The counterpart of the test above: with nothing recorded, `--pattern` contradicts no decision,
/// and the report must not imply it did. These two reports being indistinguishable was the defect.
#[test]
fn a_pattern_given_where_nothing_is_recorded_says_it_contradicts_no_decision() {
    let dir = authored();

    let minted = ok(
        dir.path(),
        &[
            "mint",
            ENERGY,
            "Tidal power",
            "--pattern",
            "https://example.org/energy/x_{n}",
        ],
    );
    assert!(
        minted.contains(
            "nothing is recorded for this vocabulary, so this pattern applies to this command"
        ),
        "{minted}"
    );
    assert!(
        !minted.contains("that record is unchanged"),
        "there is no record to leave unchanged: {minted}"
    );
}

/// Changing a convention on purpose. The previous pattern is named, once, at the moment it stops
/// being in force — which is the only place it is ever visible again.
#[test]
fn recording_a_second_pattern_says_what_it_replaced() {
    let dir = authored();
    ok(
        dir.path(),
        &[
            "policy",
            ENERGY,
            "--pattern",
            "https://example.org/energy/c_{n}",
        ],
    );

    let replaced = ok(
        dir.path(),
        &[
            "policy",
            ENERGY,
            "--pattern",
            "https://example.org/energy/{slug}",
        ],
    );
    assert!(
        replaced.contains("this replaced \"https://example.org/energy/c_{n}\""),
        "{replaced}"
    );
    assert!(replaced.contains("no longer kept anywhere"), "{replaced}");
}

/// A policy is OpenBiz's own record about a vocabulary, so it lives in the system graph — which
/// means a whole-store backup carries it and a restore brings it back. Worth proving rather than
/// reasoning about: the alternative placement (in the vocabulary) would also pass every other test
/// in this file, and would publish an OpenBiz configuration statement into a SKOS export.
#[test]
fn a_recorded_policy_survives_a_backup_and_restore() {
    let dir = authored();
    ok(
        dir.path(),
        &[
            "policy",
            ENERGY,
            "--pattern",
            "https://example.org/energy/{slug}",
        ],
    );
    ok(dir.path(), &["backup", "whole.nq"]);

    let restored_dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::copy(
        dir.path().join("whole.nq"),
        restored_dir.path().join("whole.nq"),
    )
    .expect("copy the backup");
    ok(restored_dir.path(), &["restore", "whole.nq"]);

    let shown = ok(restored_dir.path(), &["policy", ENERGY]);
    assert!(
        shown.contains("recorded: https://example.org/energy/{slug}"),
        "the restored store records the same policy: {shown}"
    );
    assert!(shown.contains("recorded by ada@example.org"), "{shown}");
}

/// The pattern is validated where the two crates meet, before anything is written. A store holding
/// a pattern nothing can parse is a vocabulary that cannot mint at all.
#[test]
fn a_pattern_this_build_cannot_mint_under_is_never_recorded() {
    let dir = authored();

    let refused = run(
        dir.path(),
        &["policy", ENERGY, "--pattern", "no placeholder"],
    );
    assert!(
        !refused.status.success(),
        "a pattern with no placeholder was accepted: {}",
        stdout(&refused)
    );

    let shown = ok(dir.path(), &["policy", ENERGY]);
    assert!(
        shown.contains("nothing is recorded"),
        "the refusal left the vocabulary as it was: {shown}"
    );
}

/// Recording a policy is a governance decision, so it is attributed by the same rule an approval is.
#[test]
fn recording_a_policy_needs_a_name_to_record() {
    let dir = authored();

    let refused = Command::new(env!("CARGO_BIN_EXE_openbiz"))
        .args([
            "policy",
            ENERGY,
            "--pattern",
            "https://example.org/energy/c_{n}",
        ])
        .current_dir(dir.path())
        .env("OPENBIZ_DATA_DIR", dir.path())
        .env("OPENBIZ_LOG", "warn")
        .env_remove("OPENBIZ_CONFIG")
        .env_remove("OPENBIZ_ACTOR")
        .env_remove("USER")
        .env_remove("LOGNAME")
        .output()
        .expect("run the openbiz binary");

    assert!(
        !refused.status.success(),
        "an unattributed policy was recorded: {}",
        stdout(&refused)
    );
    assert!(
        stderr(&refused).contains("OPENBIZ_ACTOR"),
        "the refusal says how to name somebody: {}",
        stderr(&refused)
    );

    // Showing needs nobody, because it decides nothing.
    let shown = ok(dir.path(), &["policy", ENERGY]);
    assert!(shown.contains("nothing is recorded"), "{shown}");
}

/// A vocabulary that is not there is news, and OpenBiz's own graphs have no minting policy.
#[test]
fn a_policy_is_refused_for_anything_that_is_not_a_vocabulary() {
    let dir = authored();

    for graph in ["https://example.org/nothing", "urn:openbiz:graph:system"] {
        let refused = run(dir.path(), &["policy", graph]);
        assert!(
            !refused.status.success(),
            "`openbiz policy {graph}` answered: {}",
            stdout(&refused)
        );
    }
}
