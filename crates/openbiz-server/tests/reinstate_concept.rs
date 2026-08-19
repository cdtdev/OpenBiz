//! `openbiz reinstate` end to end, against the real `openbiz` binary as a child process.
//!
//! The last part of the deprecation lifecycle, and the only place its central claim can be
//! checked. `docs/adr/0042` says a reinstatement puts the vocabulary back exactly as it was
//! before the retirement **except** for the change notes, which stay because the retirement
//! happened. That is a statement about a whole graph over time, so the test reads the graph off
//! disk with `openbiz backup` three times — before the retirement, after it, and after taking it
//! back — and compares the sets, rather than asking the code that computed the change what it
//! thinks it did.
//!
//! The second thing only a real store can show is that the read half agrees: `openbiz tree` and
//! `openbiz search` stop printing `[retired]` for a concept that has been put back. The write
//! half and the read half are separate indexes over the same statements (`docs/adr/0041`), and
//! nothing inside either one can prove they agree.
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

/// Run a command that stages a candidate, then approve it.
fn stage_and_approve(dir: &Path, args: &[&str], candidate: &str) -> String {
    let proposed = run(dir, args);
    assert!(proposed.status.success(), "{}", stderr(&proposed));
    let approved = run(dir, &["approve", candidate]);
    assert!(approved.status.success(), "{}", stderr(&approved));
    stdout(&proposed)
}

/// Only the vocabulary's own quads. A backup is the whole store, and an approved candidate keeps
/// its own copy of what it proposed, so the comparison has to be graph by graph.
fn vocabulary(quads: &BTreeSet<String>) -> BTreeSet<&str> {
    quads
        .iter()
        .filter(|line| line.ends_with(&format!("<{THESAURUS}> .")))
        .map(String::as_str)
        .collect()
}

/// `docs/adr/0042`'s claim, checked against the graph on disk rather than against the code that
/// computed it: everything the retirement added comes back out, the vocabulary is otherwise
/// letter for letter what it was, and the change note explaining the retirement is the one thing
/// that stays.
#[test]
fn taking_a_retirement_back_restores_the_graph_except_the_change_note() {
    let dir = populated();
    let before = quads(dir.path(), "before.nq");

    stage_and_approve(
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
    let retired = quads(dir.path(), "retired.nq");
    assert_eq!(
        vocabulary(&retired).len(),
        vocabulary(&before).len() + 3,
        "the marker, the replacement and the note"
    );

    stage_and_approve(dir.path(), &["reinstate", THESAURUS, WIRELESS], "3");
    let after = quads(dir.path(), "after.nq");

    let before_lines = vocabulary(&before);
    let after_lines = vocabulary(&after);
    for line in &before_lines {
        assert!(
            after_lines.contains(*line),
            "taking a retirement back removed a statement that predates it:\n{line}"
        );
    }
    let kept: Vec<&&str> = after_lines
        .iter()
        .filter(|line| !before_lines.contains(**line))
        .collect();
    assert_eq!(
        kept.len(),
        1,
        "exactly one statement outlives the retirement: {kept:#?}"
    );
    assert!(kept[0].contains(CHANGE_NOTE), "and it is the change note");
    assert!(kept[0].contains("Superseded by broadcasting terms."));

    let now = about(&after, WIRELESS);
    assert!(
        !now.iter().any(|line| line.contains(DEPRECATED)),
        "the marker is gone: {now:#?}"
    );
    assert!(
        !now.iter().any(|line| line.contains(IS_REPLACED_BY)),
        "and so is the recorded successor: {now:#?}"
    );
    assert!(
        now.iter().any(|line| line.contains(CONCEPT)),
        "and it is still a concept: {now:#?}"
    );
}

/// The read half and the write half are separate indexes over the same statements, and nothing
/// inside either can show they agree. This runs the commands a person actually looks at.
#[test]
fn the_browse_commands_stop_calling_it_retired() {
    let dir = populated();
    stage_and_approve(
        dir.path(),
        &["deprecate", THESAURUS, WIRELESS, "--replaced-by", RADIO],
        "2",
    );

    let tree = stdout(&run(dir.path(), &["tree", THESAURUS, WIRELESS]));
    assert!(tree.contains("[retired]"), "retired first: {tree}");
    let found = stdout(&run(dir.path(), &["search", THESAURUS, "wireless"]));
    assert!(found.contains("[retired]"), "and in search: {found}");

    stage_and_approve(dir.path(), &["reinstate", THESAURUS, WIRELESS], "3");

    let tree = run(dir.path(), &["tree", THESAURUS, WIRELESS]);
    assert!(tree.status.success(), "{}", stderr(&tree));
    assert!(
        !stdout(&tree).contains("[retired]"),
        "the tree agrees it is current again: {}",
        stdout(&tree)
    );
    let found = run(dir.path(), &["search", THESAURUS, "wireless"]);
    assert!(found.status.success(), "{}", stderr(&found));
    assert!(
        stdout(&found).contains("Wireless telegraphy"),
        "it is still findable: {}",
        stdout(&found)
    );
    assert!(
        !stdout(&found).contains("[retired]"),
        "and search agrees: {}",
        stdout(&found)
    );
    let inspected = stdout(&run(dir.path(), &["inspect", THESAURUS]));
    assert!(
        !inspected.contains("retired"),
        "and the whole-vocabulary backlog is empty: {inspected}"
    );
}

/// A refusal exits non-zero, says why, and stages nothing — the same contract every other
/// refusing command in this build keeps.
#[test]
fn reinstating_a_concept_that_is_not_retired_is_refused_and_stages_nothing() {
    let dir = populated();

    let refused = run(dir.path(), &["reinstate", THESAURUS, WIRELESS]);
    assert!(!refused.status.success(), "{}", stdout(&refused));
    assert!(
        stderr(&refused).contains("nothing to take back"),
        "{}",
        stderr(&refused)
    );

    let waiting = stdout(&run(dir.path(), &["candidates"]));
    assert!(
        waiting.contains("0 proposed"),
        "a refusal proposes nothing: {waiting}"
    );
}

/// The command is discoverable, which `CLAUDE.md` §4.4 asks of anything user-facing.
#[test]
fn the_help_names_the_command_and_says_what_it_keeps() {
    let dir = populated();
    let help = stdout(&run(dir.path(), &["help"]));
    assert!(
        help.contains("openbiz reinstate <graph> <resource>"),
        "{help}"
    );
    assert!(
        help.contains("the skos:changeNote explaining the retirement"),
        "{help}"
    );
}
