//! `openbiz split` end to end, against the real `openbiz` binary as a child process.
//!
//! The third bulk operation, and the one whose claims are about what it **did not** do. Two of
//! them can only be checked against a real store:
//!
//! - **The concept being split is left exactly as it was.** That is a statement about a whole
//!   graph, so the test reads the graph off disk with `openbiz backup` before and after and
//!   compares every line mentioning the concept, rather than asking the code that computed the
//!   change whether it changed anything.
//! - **The parts are real concepts afterwards**, in the hierarchy, in the scheme, and carrying the
//!   `prov:wasDerivedFrom` that says where they came from.
//!
//! The store is seeded from a hand-written backup, as `candidate_seam.rs` explains: there is no
//! "create vocabulary" command, because creating one runs through discovery with a recorded
//! justification (`CLAUDE.md` §1.7).

use std::path::Path;
use std::process::{Command, Output};

/// A store holding two empty vocabularies, as an operator could type it.
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
const BANKS: &str = "https://example.org/thesaurus/banks";
const INSTITUTIONS: &str = "https://example.org/thesaurus/institutions";
const SCHEME: &str = "https://example.org/thesaurus/scheme";
const TELLERS: &str = "https://example.org/thesaurus/tellers";
const BROADER: &str = "http://www.w3.org/2004/02/skos/core#broader";
const IN_SCHEME: &str = "http://www.w3.org/2004/02/skos/core#inScheme";
const DERIVED_FROM: &str = "http://www.w3.org/ns/prov#wasDerivedFrom";

/// One term meaning two things — the case a split exists for — with a child, an associative link,
/// a second label and a note, none of which a split can apportion.
const CONCEPTS: &str = r#"
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex: <https://example.org/thesaurus/> .

ex:scheme a skos:ConceptScheme .
ex:institutions a skos:Concept ; skos:prefLabel "Institutions"@en ; skos:inScheme ex:scheme .
ex:banks a skos:Concept ; skos:prefLabel "Banks"@en ; skos:altLabel "Bank"@en ;
    skos:broader ex:institutions ; skos:inScheme ex:scheme ;
    skos:related ex:money ; skos:scopeNote "Both senses, wrongly."@en .
ex:money a skos:Concept ; skos:prefLabel "Money"@en ; skos:inScheme ex:scheme .
ex:tellers a skos:Concept ; skos:prefLabel "Tellers"@en ; skos:broader ex:banks ;
    skos:inScheme ex:scheme .
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

/// One graph as it is on disk, one N-Quads line per statement.
fn graph(dir: &Path, iri: &str) -> Vec<String> {
    let name = "check.nq";
    let output = run(dir, &["backup", name]);
    assert!(output.status.success(), "{}", stderr(&output));
    let backup = std::fs::read_to_string(dir.join(name)).expect("read the backup back");
    std::fs::remove_file(dir.join(name)).expect("so the next backup can be taken");
    backup
        .lines()
        .filter(|line| line.ends_with(&format!("<{iri}> .")))
        .map(str::to_owned)
        .collect()
}

/// Every line of the graph that mentions `iri` at all, sorted, so two readings can be compared.
fn mentioning(statements: &[String], iri: &str) -> Vec<String> {
    let mut found: Vec<String> = statements
        .iter()
        .filter(|line| line.contains(&format!("<{iri}>")))
        .cloned()
        .collect();
    found.sort();
    found
}

fn holds(statements: &[String], subject: &str, predicate: &str, object: &str) -> bool {
    statements
        .iter()
        .any(|line| line.starts_with(&format!("<{subject}> <{predicate}> <{object}>")))
}

/// The whole item, in the order an operator walks it: propose, read, approve, and check the disk.
///
/// The assertions the item is checked off on are the last two: the parts are really there, and
/// **the concept that was split is byte-for-byte what it was**, which is the claim that makes the
/// unapportioned list honest rather than a disclaimer.
#[test]
fn a_split_creates_the_parts_and_leaves_the_concept_exactly_as_it_was() {
    let dir = populated();
    let before = mentioning(&graph(dir.path(), THESAURUS), BANKS);

    let proposed = run(
        dir.path(),
        &[
            "split",
            THESAURUS,
            BANKS,
            "--place",
            "beside",
            "--into",
            "Banks (financial)",
            "--into",
            "Banks (river)",
        ],
    );
    assert!(
        proposed.status.success(),
        "the split failed: {}",
        stderr(&proposed)
    );
    let said = stdout(&proposed);

    assert!(
        said.contains("still on") && said.contains("1 concept is below it"),
        "the work it did not do has to be in the report: {said}"
    );
    assert!(
        said.find("still on").expect("the unapportioned section")
            < said.find("it would add:").expect("the diff"),
        "and it has to come before the diff: {said}"
    );
    assert!(said.contains("and remove nothing."), "{said}");

    let approved = run(dir.path(), &["approve", "2"]);
    assert!(approved.status.success(), "{}", stderr(&approved));

    let after = graph(dir.path(), THESAURUS);
    let financial = "https://example.org/thesaurus/banks-financial";
    let river = "https://example.org/thesaurus/banks-river";

    for part in [financial, river] {
        assert!(
            holds(&after, part, BROADER, INSTITUTIONS),
            "{part} should stand where the concept stood: {after:#?}"
        );
        assert!(holds(&after, part, IN_SCHEME, SCHEME));
        assert!(
            holds(&after, part, DERIVED_FROM, BANKS),
            "{part} should record where it came from"
        );
    }

    assert_eq!(
        mentioning(&after, BANKS),
        {
            // The concept keeps every statement it had, and gains only the two derivations that
            // name it as their source — which are statements about the *parts*.
            let mut expected = before.clone();
            expected.push(format!(
                "<{financial}> <{DERIVED_FROM}> <{BANKS}> <{THESAURUS}> ."
            ));
            expected.push(format!(
                "<{river}> <{DERIVED_FROM}> <{BANKS}> <{THESAURUS}> ."
            ));
            expected.sort();
            expected
        },
        "a split removes nothing and says nothing new about the concept it divides"
    );

    // And the vocabulary is still a SKOS vocabulary afterwards, checked by the command an
    // operator would run rather than by the code that proposed the change.
    let integrity = run(dir.path(), &["integrity", THESAURUS]);
    assert!(integrity.status.success(), "{}", stderr(&integrity));
    assert!(
        stdout(&integrity).contains("none is violated"),
        "{}",
        stdout(&integrity)
    );
}

/// The other placement, and the one the report has to describe differently: the concept becomes
/// the parts' broader concept rather than their sibling.
#[test]
fn a_split_below_puts_the_parts_under_the_concept_and_leaves_its_child_alone() {
    let dir = populated();

    let proposed = run(
        dir.path(),
        &[
            "split",
            THESAURUS,
            BANKS,
            "--place",
            "below",
            "--into",
            "Retail banks",
            "--into",
            "Investment banks",
        ],
    );
    assert!(proposed.status.success(), "{}", stderr(&proposed));
    let approved = run(dir.path(), &["approve", "2"]);
    assert!(approved.status.success(), "{}", stderr(&approved));

    let after = graph(dir.path(), THESAURUS);
    let retail = "https://example.org/thesaurus/retail-banks";
    assert!(holds(&after, retail, BROADER, BANKS));
    assert!(
        !holds(&after, retail, BROADER, INSTITUTIONS),
        "a part below the concept does not also stand beside it"
    );
    assert!(
        holds(&after, TELLERS, BROADER, BANKS),
        "the concept's own child is not re-parented: which part it belongs under is the \
         judgement this command refuses to make"
    );
}

/// `--place` is required, and the message says what the two words mean rather than listing them.
#[test]
fn a_split_with_no_placement_is_refused_before_anything_is_staged() {
    let dir = populated();
    let refused = run(
        dir.path(),
        &["split", THESAURUS, BANKS, "--into", "One", "--into", "Two"],
    );
    assert!(!refused.status.success());
    assert!(
        stderr(&refused).contains("--place beside or --place below"),
        "{}",
        stderr(&refused)
    );

    let candidates = run(dir.path(), &["candidates"]);
    assert!(
        !stdout(&candidates).contains("bulk-edit"),
        "nothing was staged: {}",
        stdout(&candidates)
    );
}

/// A split into one part is not a split, and saying so is cheaper than letting somebody approve a
/// candidate that adds a synonym-by-accident.
#[test]
fn a_split_into_one_part_is_refused() {
    let dir = populated();
    let refused = run(
        dir.path(),
        &[
            "split", THESAURUS, BANKS, "--place", "below", "--into", "One",
        ],
    );
    assert!(!refused.status.success());
    assert!(
        stderr(&refused).contains("a split needs at least two parts"),
        "{}",
        stderr(&refused)
    );
}

/// Every part is minted before the next, against a scan that has seen the ones already minted, so
/// a numbered pattern gives three parts three numbers.
#[test]
fn the_parts_are_minted_under_the_vocabularys_recorded_pattern_and_do_not_collide() {
    let dir = populated();
    let recorded = run(
        dir.path(),
        &[
            "policy",
            THESAURUS,
            "--pattern",
            "https://example.org/thesaurus/c_{n}",
        ],
    );
    assert!(recorded.status.success(), "{}", stderr(&recorded));

    let proposed = run(
        dir.path(),
        &[
            "split", THESAURUS, BANKS, "--place", "below", "--into", "One", "--into", "Two",
            "--into", "Three",
        ],
    );
    assert!(proposed.status.success(), "{}", stderr(&proposed));
    let said = stdout(&proposed);
    assert!(
        said.contains("recorded pattern") && said.contains("ada@example.org"),
        "the report names the decision it minted under: {said}"
    );

    let approved = run(dir.path(), &["approve", "2"]);
    assert!(approved.status.success(), "{}", stderr(&approved));

    let after = graph(dir.path(), THESAURUS);
    for number in 1..=3 {
        let iri = format!("https://example.org/thesaurus/c_{number}");
        assert!(
            holds(&after, &iri, BROADER, BANKS),
            "{iri} should be one of the three parts: {after:#?}"
        );
    }
}
