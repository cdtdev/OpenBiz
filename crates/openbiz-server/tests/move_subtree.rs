//! `openbiz move` end to end, against the real `openbiz` binary as a child process.
//!
//! The first bulk operation, and the first thing in this build that raises a candidate carrying
//! **both** halves of a change. `docs/UNTESTED.md` recorded that combination as untested from
//! iteration 18 until a producer existed; this file is what closes it, and it closes it the way
//! the entry asked — a candidate whose two halves both land, proved by reading the store back off
//! disk rather than by asking the code that wrote it.
//!
//! The store is seeded from a hand-written backup, as `candidate_seam.rs` explains: there is no
//! "create vocabulary" command, because creating one runs through discovery with a recorded
//! justification (`CLAUDE.md` §1.7).

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

const REGIONS: &str = "https://example.org/regions";
const WORLD: &str = "https://example.org/regions/world";
const EMEA: &str = "https://example.org/regions/emea";
const APAC: &str = "https://example.org/regions/apac";
const FRANCE: &str = "https://example.org/regions/france";
const PARIS: &str = "https://example.org/regions/paris";
const BROADER: &str = "http://www.w3.org/2004/02/skos/core#broader";

/// A small hierarchy with a subtree in it: world > emea > france > paris, and world > apac.
///
/// `apac` states its link the other way round, so one fixture covers both directions SKOS S25
/// makes equivalent and a move that quietly converted one into the other would be visible.
const CONCEPTS: &str = r#"
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex: <https://example.org/regions/> .

ex:world a skos:Concept ; skos:prefLabel "World"@en ; skos:narrower ex:apac .
ex:emea a skos:Concept ; skos:prefLabel "EMEA"@en ; skos:broader ex:world .
ex:apac a skos:Concept ; skos:prefLabel "APAC"@en .
ex:france a skos:Concept ; skos:prefLabel "France"@en ; skos:broader ex:emea .
ex:paris a skos:Concept ; skos:prefLabel "Paris"@en ; skos:broader ex:france .
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

/// A store with the hierarchy actually in the vocabulary, through the seam that puts it there.
fn populated() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("seed.nq"), BACKUP).expect("write the fixture");
    let restored = run(dir.path(), &["restore", "seed.nq"]);
    assert!(restored.status.success(), "{}", stderr(&restored));

    std::fs::write(dir.path().join("concepts.ttl"), CONCEPTS).expect("write the import");
    let imported = run(dir.path(), &["import", REGIONS, "concepts.ttl"]);
    assert!(imported.status.success(), "{}", stderr(&imported));
    let approved = run(dir.path(), &["approve", "1"]);
    assert!(approved.status.success(), "{}", stderr(&approved));
    dir
}

/// The vocabulary as it is on disk, one N-Quads line per statement.
fn vocabulary(dir: &Path) -> Vec<String> {
    let name = "check.nq";
    let output = run(dir, &["backup", name]);
    assert!(output.status.success(), "{}", stderr(&output));
    let backup = std::fs::read_to_string(dir.join(name)).expect("read the backup back");
    std::fs::remove_file(dir.join(name)).expect("so the next backup can be taken");
    backup
        .lines()
        .filter(|line| line.ends_with(&format!("<{REGIONS}> .")))
        .map(str::to_owned)
        .collect()
}

fn holds(statements: &[String], subject: &str, predicate: &str, object: &str) -> bool {
    let wanted = format!("<{subject}> <{predicate}> <{object}> <{REGIONS}> .");
    statements.contains(&wanted)
}

/// The whole item, in the order an operator walks it: propose, read, approve, and check the disk.
#[test]
fn a_move_is_proposed_as_one_candidate_and_both_halves_land_together() {
    let dir = populated();

    let proposed = run(dir.path(), &["move", REGIONS, FRANCE, APAC]);
    assert!(
        proposed.status.success(),
        "the move failed: {}",
        stderr(&proposed)
    );
    let said = stdout(&proposed);
    assert!(
        said.contains("1 concept is below it and moves with it"),
        "the count of what moves has to come before the two-statement diff: {said}"
    );
    assert!(
        said.contains("nothing has been written to the vocabulary")
            || said.contains("Nothing has been written to the vocabulary"),
        "an operator must be told the vocabulary is untouched, not left to infer it: {said}"
    );
    assert!(
        said.contains("candidate 2"),
        "the report has to name the candidate to review: {said}"
    );

    // Proposed and not applied: the vocabulary still says what it said.
    let before = vocabulary(dir.path());
    assert!(holds(&before, FRANCE, BROADER, EMEA));
    assert!(!holds(&before, FRANCE, BROADER, APAC));

    // One candidate, two halves, and the list says so in one row.
    let listed = stdout(&run(dir.path(), &["candidates"]));
    assert!(
        listed.contains("adds 1 statement and removes 1 statement"),
        "a move is one decision and its row has to show both halves: {listed}"
    );
    let shown = stdout(&run(dir.path(), &["candidate", "2"]));
    assert!(
        shown.contains("would remove") && shown.contains("would add"),
        "a reviewer has to be able to read both halves before deciding: {shown}"
    );
    assert!(
        shown.contains("bulk-edit"),
        "the source has to say what kind of producer raised it: {shown}"
    );

    let approved = run(dir.path(), &["approve", "2"]);
    assert!(approved.status.success(), "{}", stderr(&approved));

    // The claim the whole item rests on, read off the disk rather than out of the writer.
    let after = vocabulary(dir.path());
    assert!(
        !holds(&after, FRANCE, BROADER, EMEA),
        "the removal half did not land: {after:?}"
    );
    assert!(
        holds(&after, FRANCE, BROADER, APAC),
        "the addition half did not land: {after:?}"
    );
    assert_eq!(
        after.len(),
        before.len(),
        "one statement out and one in leaves the vocabulary the same size"
    );

    // And the subtree came with it without a statement about it being rewritten.
    assert!(
        holds(&after, PARIS, BROADER, FRANCE),
        "paris is still under france, by its own link: {after:?}"
    );
    let above = stdout(&run(dir.path(), &["ancestors", REGIONS, PARIS]));
    assert!(
        above.contains(APAC) && !above.contains(EMEA),
        "paris moved with france, which is what makes this a subtree move: {above}"
    );
}

/// S25 lets a vocabulary state its hierarchy either way round; a move must not convert it.
#[test]
fn a_link_stated_as_narrower_is_moved_as_narrower() {
    let dir = populated();

    let proposed = run(dir.path(), &["move", REGIONS, APAC, EMEA]);
    assert!(proposed.status.success(), "{}", stderr(&proposed));
    let said = stdout(&proposed);
    assert!(
        said.contains("skos:narrower"),
        "the graph states this link as skos:narrower, so the diff must too: {said}"
    );

    assert!(run(dir.path(), &["approve", "2"]).status.success());
    let after = vocabulary(dir.path());
    assert!(
        holds(
            &after,
            EMEA,
            "http://www.w3.org/2004/02/skos/core#narrower",
            APAC
        ),
        "the vocabulary stays authored the way it was written: {after:?}"
    );
    assert!(
        !holds(&after, APAC, BROADER, EMEA),
        "and the inverse S25 already entails is not written down as a fact: {after:?}"
    );
    assert!(!holds(
        &after,
        WORLD,
        "http://www.w3.org/2004/02/skos/core#narrower",
        APAC
    ));
}

/// A cycle is *consistent* SKOS (§8.6.8), so this refusal is the only thing that catches it.
#[test]
fn moving_a_concept_under_its_own_descendant_is_refused_and_stages_nothing() {
    let dir = populated();

    let refused = run(dir.path(), &["move", REGIONS, EMEA, PARIS]);
    assert!(!refused.status.success(), "{}", stdout(&refused));
    let complaint = stderr(&refused);
    assert!(
        complaint.contains("cycle") && complaint.contains(PARIS),
        "the refusal has to name the route it would have made: {complaint}"
    );

    let listed = stdout(&run(dir.path(), &["candidates"]));
    assert!(
        listed.contains("0 proposed"),
        "a refused move must not leave a candidate behind: {listed}"
    );
}

/// A polyhierarchic concept has several parents and a move replaces one, so it must be named.
#[test]
fn a_concept_with_two_broader_concepts_needs_from_and_then_moves_one_link() {
    let dir = populated();

    // Give france a second parent, so it is polyhierarchic.
    std::fs::write(
        dir.path().join("second.nt"),
        format!("<{FRANCE}> <{BROADER}> <{APAC}> .\n"),
    )
    .expect("write the import");
    assert!(run(dir.path(), &["import", REGIONS, "second.nt"])
        .status
        .success());
    assert!(run(dir.path(), &["approve", "2"]).status.success());

    let refused = run(dir.path(), &["move", REGIONS, FRANCE, WORLD]);
    assert!(!refused.status.success(), "{}", stdout(&refused));
    let complaint = stderr(&refused);
    assert!(
        complaint.contains("--from") && complaint.contains(EMEA) && complaint.contains(APAC),
        "the refusal has to name both parents and how to choose: {complaint}"
    );

    let proposed = run(
        dir.path(),
        &["move", REGIONS, FRANCE, WORLD, "--from", EMEA],
    );
    assert!(proposed.status.success(), "{}", stderr(&proposed));
    assert!(run(dir.path(), &["approve", "3"]).status.success());

    let after = vocabulary(dir.path());
    assert!(!holds(&after, FRANCE, BROADER, EMEA), "{after:?}");
    assert!(holds(&after, FRANCE, BROADER, WORLD), "{after:?}");
    assert!(
        holds(&after, FRANCE, BROADER, APAC),
        "the parent that was not named is untouched, which is what replacing exactly one means: \
         {after:?}"
    );
}

/// The vocabulary can move underneath a pending move, and approving it anyway would lie.
#[test]
fn approving_a_move_the_vocabulary_has_outgrown_is_refused_rather_than_half_applied() {
    let dir = populated();

    let proposed = run(dir.path(), &["move", REGIONS, FRANCE, APAC]);
    assert!(proposed.status.success(), "{}", stderr(&proposed));

    // Somebody else removes the very link the move was going to remove.
    std::fs::write(
        dir.path().join("gone.nt"),
        format!("<{FRANCE}> <{BROADER}> <{EMEA}> .\n"),
    )
    .expect("write the retraction");
    assert!(run(dir.path(), &["retract", REGIONS, "gone.nt"])
        .status
        .success());
    assert!(run(dir.path(), &["approve", "3"]).status.success());

    let refused = run(dir.path(), &["approve", "2"]);
    assert!(
        !refused.status.success(),
        "a stale move must not be applied: {}",
        stdout(&refused)
    );

    // And nothing of it landed: the addition half is not in the vocabulary either.
    let after = vocabulary(dir.path());
    assert!(
        !holds(&after, FRANCE, BROADER, APAC),
        "the two halves are one decision, so a refused one applies neither: {after:?}"
    );
    assert!(run(dir.path(), &["reject", "2"]).status.success());
}

/// `openbiz move` has to be discoverable, or nobody will find it.
#[test]
fn the_usage_names_the_move_command_and_its_option() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let help = stdout(&run(dir.path(), &["help"]));
    assert!(help.contains("openbiz move"), "{help}");
    assert!(help.contains("--from"), "{help}");
}
