//! The SKOS core model end to end, against the real `openbiz` binary as a child process.
//!
//! The crate's own tests prove the rules against arrays of statements, and `inspect.rs`'s tests
//! prove the report against a store built in-process. This file proves the only thing an operator
//! actually has: a program, with arguments and an exit status, reading a vocabulary **off disk**
//! that arrived through the candidate seam like any other change.
//!
//! The claim that most needs proving here is the inference one. A model that only counted
//! `rdf:type` statements would pass every unit test written against graphs that type everything,
//! and then report zero concept schemes for a real thesaurus. So the fixture deliberately leaves
//! the scheme untyped and the report has to find it anyway — and say which statement of the
//! specification let it.
//!
//! The store is seeded from a hand-written backup, as `candidate_seam.rs` explains: there is no
//! "create vocabulary" command, because creating one runs through discovery with a recorded
//! justification (`CLAUDE.md` §1.7).

use std::path::Path;
use std::process::{Command, Output};

/// A store holding one empty vocabulary, as an operator could type it.
const BACKUP: &str = concat!(
    "<urn:openbiz:store> <urn:openbiz:storeFormatVersion> ",
    "\"4\"^^<http://www.w3.org/2001/XMLSchema#integer> <urn:openbiz:graph:system> .\n",
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

/// A thesaurus of the shape enterprise data actually arrives in.
///
/// Note what is **not** here: nothing types `ex:scheme` as a `skos:ConceptScheme`, and nothing
/// says `ex:apac skos:inScheme ex:scheme`. Both are entailed, and a report that misses them is
/// the failure this fixture exists to catch.
const CONCEPTS: &str = r#"
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex: <https://example.org/regions/> .

ex:emea a skos:Concept ;
    skos:prefLabel "Europe, Middle East and Africa"@en ;
    skos:altLabel "EMEA"@en ;
    skos:topConceptOf ex:scheme .

ex:apac a skos:Concept ;
    skos:prefLabel "Asia-Pacific"@en ;
    skos:prefLabel "Asie-Pacifique"@fr ;
    skos:topConceptOf ex:scheme .

ex:reporting a skos:OrderedCollection ;
    skos:memberList ( ex:emea ex:apac ) .
"#;

/// Run `openbiz <args>` against `data_dir` and wait for it to finish.
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

/// Put `turtle` into the vocabulary the way a user does: propose it, then approve it.
fn import_and_approve(data_dir: &Path, file: &str, turtle: &str) {
    std::fs::write(data_dir.join(file), turtle).expect("write the import");
    let imported = run(data_dir, &["import", REGIONS, file]);
    assert!(
        imported.status.success(),
        "the import failed: {}",
        stderr(&imported)
    );

    // The candidate identifier is the last whitespace-separated word of "proposed candidate N
    // against ...", which is what an operator reads off the same line.
    let announced = stdout(&imported);
    let id = announced
        .split_whitespace()
        .nth(2)
        .expect("the import names the candidate it raised")
        .to_owned();
    let approved = run(data_dir, &["approve", &id]);
    assert!(
        approved.status.success(),
        "the approval failed: {}",
        stderr(&approved)
    );
}

#[test]
fn inspect_reports_a_vocabulary_in_skos_terms_and_names_the_rule_behind_every_inference() {
    let dir = authored();

    let output = run(dir.path(), &["inspect", REGIONS]);
    assert!(
        output.status.success(),
        "inspect failed: {}",
        stderr(&output)
    );
    let report = stdout(&output);

    // Asserted: two concepts and an ordered collection.
    assert!(report.contains("skos:Concept"), "{report}");
    assert!(report.contains("skos:OrderedCollection"), "{report}");

    // Entailed: nothing in the file typed the scheme, and the report finds it anyway.
    assert!(
        report.contains("<https://example.org/regions/scheme>"),
        "the untyped concept scheme must still be reported: {report}"
    );
    assert!(
        report.contains("2 top concept(s)"),
        "skos:hasTopConcept must be read back from skos:topConceptOf under S8: {report}"
    );
    assert!(
        report.contains("(1 inferred)"),
        "the scheme must be marked as inferred rather than passed off as stated: {report}"
    );

    // The explanation, which is the requirement rather than a nicety (`CLAUDE.md` §3).
    assert!(
        report.contains("were inferred rather than stated"),
        "{report}"
    );
    assert!(
        report.contains("The rdfs:domain of skos:hasTopConcept is the class skos:ConceptScheme."),
        "a derivation must quote the specification, not merely cite it: {report}"
    );
    assert!(
        report.contains("because"),
        "a derivation must name the statement it followed from: {report}"
    );

    // The ordered collection's members come from walking the rdf:List, which nothing asserted.
    assert!(
        report.contains("2 member(s)"),
        "S36 must infer the list's items as members: {report}"
    );
    assert!(
        report.contains("ordered by 1 well-formed list(s)"),
        "{report}"
    );

    // The labels, which are what a person recognises any of this by. Note that the scheme is
    // named by a label it does not have — it has none — so it is listed by IRI alone, while the
    // collection and the concepts carry theirs.
    assert!(report.contains("languages:"), "{report}");
    assert!(
        report.contains("@en  2 preferred on 2 resource(s), 1 alternative, 0 hidden"),
        "{report}"
    );
    assert!(
        report.contains("@fr  1 preferred on 1 resource(s), 0 alternative, 0 hidden"),
        "the French half of the thesaurus is one concept behind, and the report is where a \
         translation programme sees that: {report}"
    );
    assert!(
        report.contains("0 concept(s) have no skos:prefLabel in any language"),
        "{report}"
    );

    assert!(
        report.contains("findings: 0"),
        "a well-formed thesaurus must produce no findings: {report}"
    );
    assert!(
        report.contains("no SKOS integrity condition is violated"),
        "{report}"
    );
}

/// The commonest real defect: two sources merged, and one concept ends up with two preferred
/// labels in the same language. It has to survive the whole path — a file, an import, an
/// approval, a store on disk, and a report — to be worth anything to an operator.
#[test]
fn inspect_finds_a_duplicate_preferred_label_after_a_second_import_lands() {
    let dir = authored();

    let clean = stdout(&run(dir.path(), &["inspect", REGIONS]));
    assert!(
        clean.contains("no SKOS integrity condition is violated"),
        "{clean}"
    );

    import_and_approve(
        dir.path(),
        "merged.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         ex:apac skos:prefLabel \"Asia Pacific\"@en .\n",
    );

    let output = run(dir.path(), &["inspect", REGIONS]);
    let report = stdout(&output);

    assert!(report.contains("S14"), "{report}");
    assert!(
        report.contains(
            "A resource has no more than one value of skos:prefLabel per language \
                         tag."
        ),
        "the finding must quote the specification, not merely cite it: {report}"
    );
    assert!(
        report.contains("\"Asia Pacific\"@en") && report.contains("\"Asia-Pacific\"@en"),
        "the finding must name both labels, because fixing it means choosing between them: \
         {report}"
    );
    assert!(
        report.contains("violates a SKOS integrity condition"),
        "{report}"
    );
    // Still exits 0 — `inspect` reports, it does not gate. Recorded in `docs/UNTESTED.md`.
    assert!(output.status.success(), "{}", stderr(&output));
}

/// A typo in a vocabulary IRI must not read as "that vocabulary is empty and fine".
#[test]
fn inspect_refuses_an_unregistered_vocabulary_with_a_non_zero_status() {
    let dir = authored();

    let output = run(dir.path(), &["inspect", "https://example.org/regoins"]);

    assert!(
        !output.status.success(),
        "an unregistered vocabulary must fail: {}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("regoins"),
        "the message must name what was not found: {}",
        stderr(&output)
    );
}

/// Inspect reads. If it ever writes, this is the test that says so.
#[test]
fn inspect_changes_nothing_in_the_store() {
    let dir = authored();

    let before = run(dir.path(), &["backup", "before.nq"]);
    assert!(before.status.success(), "{}", stderr(&before));
    let before = std::fs::read_to_string(dir.path().join("before.nq")).expect("the backup");

    let inspected = run(dir.path(), &["inspect", REGIONS]);
    assert!(inspected.status.success(), "{}", stderr(&inspected));

    let after = run(dir.path(), &["backup", "after.nq"]);
    assert!(after.status.success(), "{}", stderr(&after));
    let after = std::fs::read_to_string(dir.path().join("after.nq")).expect("the backup");

    let mut before: Vec<_> = before.lines().collect();
    let mut after: Vec<_> = after.lines().collect();
    before.sort_unstable();
    after.sort_unstable();
    assert_eq!(
        before, after,
        "inspect must leave the store byte-for-byte as it found it"
    );
}
