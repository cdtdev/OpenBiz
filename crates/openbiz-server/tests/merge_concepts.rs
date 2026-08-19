//! `openbiz merge` end to end, against the real `openbiz` binary as a child process.
//!
//! The second bulk operation, and the one whose claim can only be checked against a real store:
//! **after the merge, nothing in the vocabulary mentions the duplicate**. That is a statement
//! about a whole graph, so the test reads the whole graph back off disk with `openbiz backup` and
//! greps it, rather than asking the code that computed the change whether it computed it.
//!
//! The store is seeded from a hand-written backup, as `candidate_seam.rs` explains: there is no
//! "create vocabulary" command, because creating one runs through discovery with a recorded
//! justification (`CLAUDE.md` §1.7).

use std::path::Path;
use std::process::{Command, Output};

/// A store holding two empty vocabularies, as an operator could type it.
///
/// Two, because a merge has to prove it does *not* reach into the second one.
const BACKUP: &str = concat!(
    "<urn:openbiz:store> <urn:openbiz:storeFormatVersion> ",
    "\"3\"^^<http://www.w3.org/2001/XMLSchema#integer> <urn:openbiz:graph:system> .\n",
    "<urn:openbiz:graph:system> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<urn:openbiz:graph:system> <urn:openbiz:graphKind> \"system\" <urn:openbiz:graph:system> .\n",
    "<https://example.org/animals> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<https://example.org/animals> <urn:openbiz:graphKind> \"vocabulary\" ",
    "<urn:openbiz:graph:system> .\n",
    "<https://example.org/facilities> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> ",
    "<urn:openbiz:Graph> <urn:openbiz:graph:system> .\n",
    "<https://example.org/facilities> <urn:openbiz:graphKind> \"vocabulary\" ",
    "<urn:openbiz:graph:system> .\n",
);

const ANIMALS: &str = "https://example.org/animals";
const FACILITIES: &str = "https://example.org/facilities";
const ROOT: &str = "https://example.org/animals/animals";
const CATS: &str = "https://example.org/animals/cats";
const FELINES: &str = "https://example.org/animals/felines";
const TABBY: &str = "https://example.org/animals/tabby";
const BROADER: &str = "http://www.w3.org/2004/02/skos/core#broader";
const HOUSED: &str = "https://example.org/schema/housedAs";

/// Two imports of one thesaurus produced two concepts for one thing, which is the case merges
/// exist for. `felines` is the duplicate: it has a child, a parent, a preferred label that
/// collides in English, one that does not collide in French, and a reference from a property SKOS
/// has never heard of.
const CONCEPTS: &str = r#"
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex: <https://example.org/animals/> .
@prefix schema: <https://example.org/schema/> .

ex:animals a skos:Concept ; skos:prefLabel "Animals"@en .
ex:cats a skos:Concept ; skos:prefLabel "Cats"@en ; skos:broader ex:animals .
ex:felines a skos:Concept ; skos:prefLabel "Felines"@en ; skos:prefLabel "Félins"@fr ;
    skos:broader ex:animals .
ex:tabby a skos:Concept ; skos:prefLabel "Tabby"@en ; skos:broader ex:felines .
ex:enclosure schema:housedAs ex:felines .
"#;

/// A second vocabulary that maps into the first. Merging in the first must not touch it.
const MAPPING: &str = r#"
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex: <https://example.org/facilities/> .

ex:catHouse a skos:Concept ; skos:prefLabel "Cat house"@en ;
    skos:closeMatch <https://example.org/animals/felines> .
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

/// A store with both vocabularies populated, through the seam that puts statements there.
fn populated() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("seed.nq"), BACKUP).expect("write the fixture");
    let restored = run(dir.path(), &["restore", "seed.nq"]);
    assert!(restored.status.success(), "{}", stderr(&restored));

    for (index, (graph, turtle)) in [(ANIMALS, CONCEPTS), (FACILITIES, MAPPING)]
        .into_iter()
        .enumerate()
    {
        let file = format!("import{index}.ttl");
        std::fs::write(dir.path().join(&file), turtle).expect("write the import");
        let imported = run(dir.path(), &["import", graph, &file]);
        assert!(imported.status.success(), "{}", stderr(&imported));
        let approved = run(dir.path(), &["approve", &(index + 1).to_string()]);
        assert!(approved.status.success(), "{}", stderr(&approved));
    }
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

fn holds(statements: &[String], subject: &str, predicate: &str, object: &str) -> bool {
    statements
        .iter()
        .any(|line| line.starts_with(&format!("<{subject}> <{predicate}> <{object}>")))
}

/// The whole item, in the order an operator walks it: propose, read, approve, and check the disk.
///
/// The assertion the item is checked off on is the last one: **no line of the vocabulary mentions
/// the merged IRI**, which is a claim about the graph and not about the diff.
#[test]
fn a_merge_is_proposed_as_one_candidate_and_leaves_nothing_pointing_at_the_duplicate() {
    let dir = populated();

    let proposed = run(dir.path(), &["merge", ANIMALS, FELINES, CATS]);
    assert!(
        proposed.status.success(),
        "the merge failed: {}",
        stderr(&proposed)
    );
    let said = stdout(&proposed);
    assert!(
        said.contains("would stop existing in this vocabulary"),
        "the effect has to come before the diff: {said}"
    );
    assert!(
        said.contains("4 statements are about it and 2 point at it"),
        "the two counts are what tell an operator this is not a small change: {said}"
    );
    assert!(
        said.contains("yields to"),
        "a demoted preferred label is the one thing here the operator did not ask for: {said}"
    );
    assert!(
        said.contains("Nothing has been written to the vocabulary"),
        "an operator must be told the vocabulary is untouched, not left to infer it: {said}"
    );
    assert!(
        said.contains("also mentioned outside this vocabulary"),
        "a reference this command cannot reach must be named, not silently left: {said}"
    );

    // Proposed and not applied: the vocabulary still says what it said.
    let before = graph(dir.path(), ANIMALS);
    assert!(holds(&before, TABBY, BROADER, FELINES));
    assert!(!holds(&before, TABBY, BROADER, CATS));

    // One candidate, both halves, in one row of the list.
    let listed = stdout(&run(dir.path(), &["candidates"]));
    assert!(
        listed.contains("adds ") && listed.contains("removes "),
        "a merge is one decision that both removes and adds: {listed}"
    );

    let approved = run(dir.path(), &["approve", "3"]);
    assert!(approved.status.success(), "{}", stderr(&approved));

    let after = graph(dir.path(), ANIMALS);
    assert!(
        !after.iter().any(|line| line.contains(FELINES)),
        "the merged IRI survived somewhere in the vocabulary: {after:#?}"
    );
    assert!(
        holds(&after, TABBY, BROADER, CATS),
        "the child did not follow: {after:#?}"
    );
    assert!(
        holds(
            &after,
            "https://example.org/animals/enclosure",
            HOUSED,
            CATS
        ),
        "a reference through a property SKOS has no reading of did not follow: {after:#?}"
    );
    assert!(
        holds(&after, CATS, BROADER, ROOT),
        "the shared parent link must survive, not be removed as part of the duplicate: {after:#?}"
    );
    assert!(
        after
            .iter()
            .any(|line| line.contains("skos/core#altLabel> \"Felines\"@en")),
        "the colliding preferred label had to survive as an alternative one: {after:#?}"
    );
    assert!(
        after.iter().any(
            |line| line.contains("skos/core#prefLabel> \"F\\u00E9lins\"@fr")
                || line.contains("skos/core#prefLabel> \"Félins\"@fr")
        ),
        "the label that collided with nothing had to stay preferred: {after:#?}"
    );

    // The other vocabulary is a different graph and a different decision. Untouched.
    let elsewhere = graph(dir.path(), FACILITIES);
    assert!(
        elsewhere.iter().any(|line| line.contains(FELINES)),
        "the merge reached across a vocabulary boundary: {elsewhere:#?}"
    );

    // And the vocabulary that remains is still SKOS: one preferred label per language.
    let integrity = run(dir.path(), &["integrity", ANIMALS]);
    assert!(integrity.status.success(), "{}", stderr(&integrity));
    let checked = stdout(&integrity);
    assert!(
        checked.contains("none is violated"),
        "a merge must not leave a vocabulary that fails an integrity condition: {checked}"
    );
    assert!(
        !checked
            .lines()
            .any(|line| line.split_whitespace().any(|word| word == "violated")),
        "no condition row may read `violated`, whatever the closing sentence says: {checked}"
    );
}

/// The refusals reach the operator as refusals, with a non-zero exit and one sentence each.
#[test]
fn a_merge_that_would_close_a_cycle_is_refused_once_and_exits_non_zero() {
    let dir = populated();

    let refused = run(dir.path(), &["merge", ANIMALS, TABBY, ROOT]);
    assert!(
        !refused.status.success(),
        "a refused merge must not exit 0: {}",
        stdout(&refused)
    );
    let said = stderr(&refused);
    assert!(said.contains("would make a cycle"), "{said}");
    assert_eq!(
        said.matches("would make a cycle").count(),
        1,
        "the sentence must be printed once, not once as the message and once as its cause: {said}"
    );

    // And nothing was staged: a refused merge leaves no candidate behind.
    let listed = stdout(&run(dir.path(), &["candidates"]));
    assert!(
        !listed.contains("merge"),
        "a refusal must not leave a candidate: {listed}"
    );
}

/// The SKOS-XL case, which is the reason the integrity check reads the whole condition set rather
/// than the two conditions a merge obviously risks.
///
/// A merge reconciles *plain* SKOS labels: it demotes a colliding `skos:prefLabel` to an
/// alternative one. It does not reconcile SKOS-XL labels, because a `skosxl:prefLabel` points at a
/// label **resource** and repointing it is not a label decision — so after the merge S55 dumbs both
/// resources down to plain preferred labels in one language and S14 is violated. Nothing about the
/// merge computation knows that. The check on the resulting vocabulary catches it anyway, which is
/// the whole argument for asking the model rather than predicting the answer.
#[test]
fn a_merge_that_would_violate_s14_through_skos_xl_labels_is_refused() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("seed.nq"), BACKUP).expect("write the fixture");
    assert!(run(dir.path(), &["restore", "seed.nq"]).status.success());
    std::fs::write(
        dir.path().join("xl.ttl"),
        r#"
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix skosxl: <http://www.w3.org/2008/05/skos-xl#> .
@prefix ex: <https://example.org/animals/> .

ex:cats a skos:Concept ; skosxl:prefLabel ex:l1 .
ex:felines a skos:Concept ; skosxl:prefLabel ex:l2 .
ex:l1 a skosxl:Label ; skosxl:literalForm "Cats"@en .
ex:l2 a skosxl:Label ; skosxl:literalForm "Felines"@en .
"#,
    )
    .expect("write the import");
    assert!(run(dir.path(), &["import", ANIMALS, "xl.ttl"])
        .status
        .success());
    assert!(run(dir.path(), &["approve", "1"]).status.success());

    let refused = run(dir.path(), &["merge", ANIMALS, FELINES, CATS]);
    assert!(
        !refused.status.success(),
        "a merge that leaves a graph failing S14 must not exit 0: {}",
        stdout(&refused)
    );
    let said = stderr(&refused);
    assert!(said.contains("not a SKOS vocabulary"), "{said}");
    assert!(said.contains("S14"), "{said}");

    // And the vocabulary is untouched: no candidate, and both concepts still there.
    let after = graph(dir.path(), ANIMALS);
    assert!(
        after.iter().any(|line| line.contains(FELINES)),
        "{after:#?}"
    );
    assert!(
        !stdout(&run(dir.path(), &["candidates"])).contains("proposed\t"),
        "a refusal must not leave a candidate"
    );
}

/// Merging a concept into itself is not a change, and the message names the mistake it came from.
#[test]
fn merging_a_concept_into_itself_is_refused() {
    let dir = populated();

    let refused = run(dir.path(), &["merge", ANIMALS, CATS, CATS]);
    assert!(!refused.status.success(), "{}", stdout(&refused));
    assert!(
        stderr(&refused).contains("name the concept to keep second"),
        "{}",
        stderr(&refused)
    );
}

/// The command needs all three of its IRIs; two of them is not a merge of something unnamed.
#[test]
fn merge_needs_all_three_of_its_positionals() {
    let dir = populated();

    let refused = run(dir.path(), &["merge", ANIMALS, FELINES]);
    assert!(!refused.status.success(), "{}", stdout(&refused));
    assert!(
        stderr(&refused).contains("the IRI of the concept that survives"),
        "{}",
        stderr(&refused)
    );
}
