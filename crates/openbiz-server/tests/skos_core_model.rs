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

/// SKOS-XL as an enterprise thesaurus actually uses it: the label has an IRI of its own, so the
/// thesaurus can record who created it and when — which is the whole reason ISO 25964 needs
/// SKOS-XL and plain SKOS will not do (`CLAUDE.md` §2).
///
/// Note what is **not** here, as in [`CONCEPTS`]: no `skos:prefLabel` statement anywhere. Every
/// plain label in the report below is entailed from a chain, and a vocabulary that reported none
/// would be one an ordinary RDF tool sees as unlabelled.
const XL_CONCEPTS: &str = r#"
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix skosxl: <http://www.w3.org/2008/05/skos-xl#> .
@prefix dcterms: <http://purl.org/dc/terms/> .
@prefix ex: <https://example.org/regions/> .

ex:latam a skos:Concept ;
    skosxl:prefLabel ex:label-latam ;
    skosxl:altLabel ex:label-latam-short ;
    skos:topConceptOf ex:scheme .

ex:label-latam a skosxl:Label ;
    skosxl:literalForm "Latin America"@en ;
    dcterms:created "2026-01-14"^^<http://www.w3.org/2001/XMLSchema#date> ;
    dcterms:creator "ada@example.org" .

ex:label-latam-short a skosxl:Label ;
    skosxl:literalForm "LATAM"@en .
"#;

/// The dumbing-down, end to end: an XL-labelled concept reads as a plain SKOS one.
#[test]
fn inspect_infers_plain_skos_labels_from_skos_xl_and_says_which_chain_licensed_each() {
    let dir = authored();
    import_and_approve(dir.path(), "xl.ttl", XL_CONCEPTS);

    let output = run(dir.path(), &["inspect", REGIONS]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);

    assert!(
        report.contains("skosxl:Label"),
        "the class must be counted: {report}"
    );
    assert!(
        report.contains("2 skosxl:Label resource(s), 2 with exactly one literal form"),
        "{report}"
    );
    assert!(
        report.contains(
            "1 resource(s) labelled through SKOS-XL, 2 plain SKOS label(s) inferred from them"
        ),
        "{report}"
    );

    // The chains, quoted. S55 and S56 are separate statements of the specification and each names
    // one property, so a report citing S55 for the alternative label would be citing the wrong one.
    assert!(
        report.contains(
            "The property chain (skosxl:prefLabel, skosxl:literalForm) is a sub-property of \
             skos:prefLabel."
        ),
        "{report}"
    );
    assert!(
        report.contains(
            "The property chain (skosxl:altLabel, skosxl:literalForm) is a sub-property of \
             skos:altLabel."
        ),
        "{report}"
    );
    assert!(
        report.contains("skos:prefLabel \"Latin America\"@en"),
        "the inferred plain label must be shown, not merely counted: {report}"
    );

    // And the inferred labels answer the question a translation programme asks. Before this
    // import the English column read "2 preferred on 2 resource(s), 1 alternative"; the XL
    // concept moves both, which is only true if a dumbed-down label counts as a label.
    assert!(
        report.contains("@en  3 preferred on 3 resource(s), 2 alternative, 0 hidden"),
        "{report}"
    );
    assert!(
        report.contains("0 concept(s) have no skos:prefLabel in any language"),
        "an XL-labelled concept is not an unlabelled concept: {report}"
    );

    assert!(report.contains("findings: 0"), "{report}");
    assert!(
        report.contains("no SKOS integrity condition is violated"),
        "{report}"
    );
}

/// Appendix B.3.4.2's Example 84, through the binary: two preferred XL labels in one language.
///
/// The point is *where* the fault is caught. Nothing in the XL data model alone forbids this —
/// B.3.4.2 says so outright — and it is inconsistent only because the chains produce two
/// `skos:prefLabel` values that S14 then forbids. A build that dumbed labels down into a separate
/// bucket would report this vocabulary as clean.
#[test]
fn inspect_finds_a_duplicate_preferred_label_that_exists_only_by_dumbing_down() {
    let dir = authored();
    import_and_approve(dir.path(), "xl.ttl", XL_CONCEPTS);
    assert!(
        stdout(&run(dir.path(), &["inspect", REGIONS])).contains("findings: 0"),
        "the fixture must start clean"
    );

    import_and_approve(
        dir.path(),
        "second-label.ttl",
        "@prefix skosxl: <http://www.w3.org/2008/05/skos-xl#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         ex:latam skosxl:prefLabel ex:label-latam-alt .\n\
         ex:label-latam-alt a skosxl:Label ; skosxl:literalForm \"LatAm\"@en .\n",
    );

    let output = run(dir.path(), &["inspect", REGIONS]);
    let report = stdout(&output);

    assert!(report.contains("S14"), "{report}");
    assert!(
        report.contains("\"LatAm\"@en") && report.contains("\"Latin America\"@en"),
        "the finding must name both labels even though neither is stated as a plain label: \
         {report}"
    );
    assert!(
        report.contains("violates a SKOS integrity condition"),
        "{report}"
    );
    assert!(output.status.success(), "{}", stderr(&output));
}

/// Appendix B.4 through the binary: a link between two labels, and the converse we supplied.
///
/// This is the shape ISO 25964's label relationships take in SKOS-XL — "LATAM" stands in a
/// recorded relationship to "Latin America", and the relationship hangs off the *labels* rather
/// than off the concept, which is precisely what plain SKOS cannot express. The link is stated in
/// one direction only, as an author would state it, and S62 supplies the other.
#[test]
fn inspect_reports_links_between_labels_and_the_converse_it_inferred() {
    let dir = authored();
    import_and_approve(dir.path(), "xl.ttl", XL_CONCEPTS);

    import_and_approve(
        dir.path(),
        "acronym.ttl",
        "@prefix skosxl: <http://www.w3.org/2008/05/skos-xl#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         ex:label-latam-short skosxl:labelRelation ex:label-latam .\n",
    );

    let output = run(dir.path(), &["inspect", REGIONS]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);

    // One link, not two. The graph carries one statement and S62 makes it hold both ways; a
    // report saying "2 links" would be counting our own inference as the author's work.
    assert!(
        report.contains("1 link(s) between labels, 1 converse(s) inferred under S62"),
        "{report}"
    );
    assert!(
        report.contains(
            "<https://example.org/regions/label-latam> skosxl:labelRelation \
             <https://example.org/regions/label-latam-short>"
        ),
        "the inferred converse must be shown with its direction, not merely counted: {report}"
    );
    assert!(
        report.contains("skosxl:labelRelation is an instance of owl:SymmetricProperty."),
        "the derivation must quote the statement that licensed it: {report}"
    );

    // The vocabulary is unchanged in every other respect: a link entails no label, no class it
    // did not already have, and no finding.
    assert!(report.contains("findings: 0"), "{report}");
    assert!(
        report.contains("2 skosxl:Label resource(s), 2 with exactly one literal form"),
        "{report}"
    );
    assert!(
        report.contains("no SKOS integrity condition is violated"),
        "{report}"
    );
}

/// A `skosxl:labelRelation` pointing at a concept is caught, and the report says which statement
/// caught it — the case that makes S60 worth applying rather than merely quoting.
#[test]
fn inspect_refuses_a_label_relation_that_points_at_a_concept() {
    let dir = authored();
    import_and_approve(dir.path(), "xl.ttl", XL_CONCEPTS);

    import_and_approve(
        dir.path(),
        "mislinked.ttl",
        "@prefix skosxl: <http://www.w3.org/2008/05/skos-xl#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         ex:label-latam skosxl:labelRelation ex:latam .\n",
    );

    let report = stdout(&run(dir.path(), &["inspect", REGIONS]));

    assert!(
        report.contains("The rdfs:range of skosxl:labelRelation is the class skosxl:Label."),
        "the range rule is what makes the concept a label: {report}"
    );
    assert!(
        report.contains(
            "skosxl:Label is disjoint with each of skos:Concept, skos:ConceptScheme and \
             skos:Collection."
        ),
        "and S48 is what makes that a contradiction: {report}"
    );
    assert!(
        report.contains("violates a SKOS integrity condition"),
        "{report}"
    );
}

/// A plain-SKOS vocabulary gains no SKOS-XL section, because "0 labels" on every report is noise
/// and the section's presence is itself the answer to "does this thesaurus use SKOS-XL?".
#[test]
fn inspect_says_nothing_about_skos_xl_for_a_vocabulary_that_does_not_use_it() {
    let dir = authored();

    let report = stdout(&run(dir.path(), &["inspect", REGIONS]));

    assert!(!report.contains("skos-xl labels:"), "{report}");
    assert!(
        report.contains("skosxl:Label            0\n"),
        "the class is still counted, so the reader can see it was looked for: {report}"
    );
}

/// §8 through the binary: a hierarchy an author wrote in one direction, read in both.
///
/// This is what a thesaurus is bought for. `ex:world` and `ex:eurasia` are never typed and never
/// mentioned except as the far end of a link, and they come out as concepts — which is S19 and S20
/// doing the work that makes the *next* test's mistake visible. Note that the file states three
/// `skos:broader` and one `skos:narrower`, mixing the directions exactly as two merged sources do.
#[test]
fn inspect_reports_a_hierarchy_and_the_direction_each_link_was_written_in() {
    let dir = authored();
    import_and_approve(
        dir.path(),
        "hierarchy.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         ex:apac skos:broader ex:world .\n\
         ex:emea skos:broader ex:world .\n\
         ex:emea skos:broader ex:eurasia .\n\
         ex:world skos:narrower ex:latam .\n\
         ex:apac skos:related ex:emea .\n",
    );

    let output = run(dir.path(), &["inspect", REGIONS]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);

    // Four links, not eight. S25 closes every one into a pair, so counting what each concept
    // holds would report twice the hierarchy the author wrote.
    assert!(
        report.contains("4 hierarchical link(s), 1 of them stated as skos:narrower"),
        "{report}"
    );
    assert!(
        report.contains("1 associative link(s), 1 converse(s) inferred under S23"),
        "{report}"
    );
    // Polyhierarchy: `ex:emea` sits under two parents. Ordinary in a thesaurus, never a finding.
    assert!(
        report.contains("1 concept(s) have more than one broader concept (polyhierarchy)"),
        "{report}"
    );

    // The inverse the author did not write, shown with its direction and its statement.
    assert!(
        report.contains(
            "<https://example.org/regions/latam> skos:broader <https://example.org/regions/world>"
        ),
        "{report}"
    );
    assert!(
        report.contains("skos:narrower is owl:inverseOf the property skos:broader."),
        "{report}"
    );
    // And the S22 lift, which is what the transitive closure will read when it arrives.
    assert!(
        report.contains(
            "<https://example.org/regions/emea> skos:broaderTransitive \
             <https://example.org/regions/eurasia>"
        ),
        "{report}"
    );

    // The chain that types an untyped concept: S22 to the variant, S21 to the super-property,
    // then S19 and S20. A report citing S20 against the `skos:broader` statement itself would
    // name a statement that does not mention the property the author wrote.
    assert!(
        report.contains(
            "<https://example.org/regions/emea> skos:semanticRelation \
             <https://example.org/regions/eurasia>"
        ),
        "{report}"
    );
    assert!(
        report.contains(
            "skos:broaderTransitive, skos:narrowerTransitive and skos:related are each \
             sub-properties of skos:semanticRelation."
        ),
        "{report}"
    );
    assert!(
        report.contains("The rdfs:range of skos:semanticRelation is the class skos:Concept."),
        "{report}"
    );

    // Two concepts were typed in the file; three more are here only because they are at the end
    // of a link.
    assert!(
        report.contains("skos:Concept            5  (3 inferred)"),
        "{report}"
    );
    assert!(report.contains("findings: 0"), "{report}");
    assert!(
        report.contains("no SKOS integrity condition is violated"),
        "{report}"
    );
}

/// The mistake S19 and S20 exist to catch, end to end: a `skos:broader` pointing at a collection.
///
/// Without the domain and range this graph reads as clean — nothing else in it would ever type
/// `ex:reporting` as a concept — so the report would show a tidy hierarchy with a collection
/// quietly sitting in it. The two statements that catch it are both printed, because a governance
/// team defending the refusal needs to cite them.
#[test]
fn inspect_refuses_a_broader_link_that_points_at_a_collection() {
    let dir = authored();
    import_and_approve(
        dir.path(),
        "mislinked.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         ex:apac skos:broader ex:reporting .\n",
    );

    let report = stdout(&run(dir.path(), &["inspect", REGIONS]));

    assert!(
        report.contains("The rdfs:range of skos:semanticRelation is the class skos:Concept."),
        "the range rule is what makes the collection a concept: {report}"
    );
    assert!(
        report.contains(
            "skos:Collection is disjoint with each of skos:Concept and \
                         skos:ConceptScheme."
        ),
        "and S37 is what makes that a contradiction: {report}"
    );
    assert!(
        report.contains("violates a SKOS integrity condition"),
        "{report}"
    );
    // **One** finding, not several. Iteration 23 closed on the worry that a domain or range rule
    // entailing a class nobody wanted fans one authoring error out into a list nobody reads. It
    // does not here, and the reason is structural rather than lucky: S48's fan-out came from
    // `skosxl:Label` also being *constrained* — S52 wants a literal form, so a concept made a
    // label picks up a second complaint — and `skos:Concept` carries no such constraint. This
    // assertion is what would notice if a later item gave it one. See `adr/0023`.
    assert!(report.contains("findings: 1"), "{report}");
}

/// A vocabulary with no links gains no section, for the reason the SKOS-XL one is omitted: "0
/// links" on every report is noise, and the section's presence is itself the answer to "does this
/// vocabulary have a hierarchy at all?".
#[test]
fn inspect_says_nothing_about_semantic_relations_for_a_flat_vocabulary() {
    let dir = authored();

    let report = stdout(&run(dir.path(), &["inspect", REGIONS]));

    assert!(!report.contains("semantic relations:"), "{report}");
}

/// S24 end to end: a chain three deep, walked by the real binary against a store on disk, with
/// the derivation printed for the link nobody wrote. Everything in between — the file, the
/// proposal, the approval, the store, the model, the walk — has to work for this to read.
#[test]
fn ancestors_walks_a_chain_off_disk_and_names_s24_for_the_step_nobody_stated() {
    let dir = authored();
    import_and_approve(
        dir.path(),
        "hierarchy.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         ex:japan a skos:Concept ; skos:prefLabel \"Japan\"@en ; skos:broader ex:eastasia .\n\
         ex:eastasia a skos:Concept ; skos:prefLabel \"East Asia\"@en ; skos:broader ex:apac .\n",
    );

    let output = run(
        dir.path(),
        &["ancestors", REGIONS, "https://example.org/regions/japan"],
    );
    let report = stdout(&output);
    assert!(output.status.success(), "{}", stderr(&output));

    assert!(report.contains("2 concept(s) are above it"), "{report}");
    assert!(
        report.contains("(\"East Asia\"@en)") && report.contains("(\"Asia-Pacific\"@en)"),
        "an operator reads labels, not IRIs: {report}"
    );
    assert!(
        report.contains(
            "<https://example.org/regions/japan> → <https://example.org/regions/eastasia> → \
             <https://example.org/regions/apac>"
        ),
        "the path is the derivation for a transitive conclusion: {report}"
    );
    assert!(
        report.contains(
            "S24: skos:broaderTransitive and skos:narrowerTransitive are each instances of \
             owl:TransitiveProperty."
        ),
        "the report must quote the statement, not merely cite it: {report}"
    );
    assert!(report.contains("that is all of them."), "{report}");

    // And the direction is real: nothing is above the top of the chain.
    let top = stdout(&run(
        dir.path(),
        &["ancestors", REGIONS, "https://example.org/regions/apac"],
    ));
    assert!(top.contains("nothing is above it"), "{top}");
}

/// §7 end to end: the definition an author wrote, and the `skos:note` S17 entailed from it,
/// printed by the real binary against a store on disk with the premise and the quoted rule.
///
/// The entailed note is the whole reason this command exists. A Turtle export of this vocabulary
/// shows the `skos:definition` and never shows the `skos:note`, so an operator who read
/// "4 note(s)" in `openbiz inspect` after writing three definitions has nowhere else to look.
#[test]
fn notes_prints_the_documentation_off_disk_and_names_s17_for_the_note_nobody_stated() {
    let dir = authored();
    import_and_approve(
        dir.path(),
        "documented.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         ex:apac skos:definition \"The Asia-Pacific region.\"@en ;\n\
             skos:scopeNote \"Excludes Central Asia.\"@en .\n",
    );

    let output = run(
        dir.path(),
        &["notes", REGIONS, "https://example.org/regions/apac"],
    );
    let report = stdout(&output);
    assert!(output.status.success(), "{}", stderr(&output));

    assert!(report.contains("(\"Asia-Pacific\"@en)"), "{report}");
    assert!(
        report.contains("skos:definition") && report.contains("\"The Asia-Pacific region.\"@en"),
        "{report}"
    );
    assert!(
        report.contains("skos:scopeNote") && report.contains("\"Excludes Central Asia.\"@en"),
        "{report}"
    );
    assert!(
        report.contains("inferred, not stated under skos:note"),
        "the entailed note is what an export cannot show: {report}"
    );
    assert!(
        report.contains("because skos:definition \"The Asia-Pacific region.\"@en"),
        "the premise must name the statement it came from: {report}"
    );
    assert!(
        report.contains(
            "S17: skos:changeNote, skos:definition, skos:editorialNote, skos:example, \
             skos:historyNote and skos:scopeNote are each sub-properties of skos:note."
        ),
        "the report must quote the statement, not merely cite it: {report}"
    );
}

/// A concept nobody has documented is reported as such and exits 0, and the report says the
/// specification asks for no documentation — so an operator does not read legal SKOS as broken.
///
/// §7 has no "Integrity Conditions" subsection at all, unlike §5.4. Every incumbent flags a
/// missing definition; that check is ANSI/NISO Z39.19 and ISO 25964, which are rule packs we have
/// not built, and reporting it as a SKOS finding would be citing a statement the specification
/// never made.
#[test]
fn notes_reports_an_undocumented_concept_without_calling_it_a_defect() {
    let dir = authored();

    let output = run(
        dir.path(),
        &["notes", REGIONS, "https://example.org/regions/emea"],
    );
    let report = stdout(&output);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        report.contains("no SKOS documentation property carries anything for it"),
        "{report}"
    );
    assert!(
        report.contains("SKOS states no condition requiring one"),
        "{report}"
    );
}

/// A resource the vocabulary says nothing about in SKOS terms must not read as an undocumented
/// one, and must not exit 0. The two reports are opposite in meaning and a mistyped IRI is the
/// likelier of the two at a command line.
#[test]
fn notes_refuses_a_resource_the_vocabulary_does_not_hold() {
    let dir = authored();

    let output = run(
        dir.path(),
        &["notes", REGIONS, "https://example.org/regions/atlantis"],
    );

    assert!(
        !output.status.success(),
        "a resource that is not there must fail: {}",
        stdout(&output)
    );
    assert!(stderr(&output).contains("atlantis"), "{}", stderr(&output));
}

/// `openbiz inspect` counts the documentation, including what S17 supplied, and says in the same
/// breath that a zero is not a verdict.
#[test]
fn inspect_reports_documentation_coverage_and_who_would_ask_for_more() {
    let dir = authored();

    let bare = stdout(&run(dir.path(), &["inspect", REGIONS]));
    assert!(bare.contains("documentation:"), "{bare}");
    assert!(
        bare.contains("skos:definition") && bare.contains("0 concept(s), 0 note(s)"),
        "a zero is an answer and must be printed: {bare}"
    );
    assert!(
        bare.contains("§7 states no integrity condition")
            && bare.contains("Z39.19 / ISO 25964 rule pack"),
        "the report must name who would ask for a definition, since SKOS does not: {bare}"
    );

    import_and_approve(
        dir.path(),
        "documented.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         ex:apac skos:definition \"The Asia-Pacific region.\"@en .\n",
    );

    let report = stdout(&run(dir.path(), &["inspect", REGIONS]));
    assert!(
        report.contains("skos:definition           1 concept(s), 1 note(s)"),
        "{report}"
    );
    assert!(
        report.contains("1 inferred under S17"),
        "the note S17 supplied has to be distinguishable from one the author wrote: {report}"
    );
}

/// A concept the vocabulary does not hold must not read as a root concept, and must not exit 0.
#[test]
fn ancestors_refuses_a_concept_the_vocabulary_does_not_hold() {
    let dir = authored();

    let output = run(
        dir.path(),
        &["ancestors", REGIONS, "https://example.org/regions/atlantis"],
    );

    assert!(
        !output.status.success(),
        "a concept that is not there must fail: {}",
        stdout(&output)
    );
    assert!(stderr(&output).contains("atlantis"), "{}", stderr(&output));
}

/// §8.5's Example 27 through the whole product: the clash is between concepts the author never
/// linked directly, so a build without S24 reports this vocabulary as clean. That is the false
/// green `docs/UNTESTED.md` recorded from iteration 24 until this one.
#[test]
fn inspect_reports_the_indirect_s27_clash_of_example_27() {
    let dir = authored();

    let clean = stdout(&run(dir.path(), &["inspect", REGIONS]));
    assert!(
        clean.contains("no SKOS integrity condition is violated"),
        "{clean}"
    );

    import_and_approve(
        dir.path(),
        "clash.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         ex:japan a skos:Concept ; skos:broader ex:eastasia ; skos:related ex:apac .\n\
         ex:eastasia a skos:Concept ; skos:broader ex:apac .\n",
    );

    let output = run(dir.path(), &["inspect", REGIONS]);
    let report = stdout(&output);

    assert!(
        report.contains("violates a SKOS integrity condition"),
        "{report}"
    );
    assert!(
        report.contains("skos:related is disjoint with the property skos:broaderTransitive."),
        "the finding must quote S27, not merely cite it: {report}"
    );
    assert!(
        report.contains(
            "<https://example.org/regions/japan> skos:broaderTransitive \
             <https://example.org/regions/eastasia>"
        ),
        "the chain is what makes the clash actionable, because nobody wrote the link: {report}"
    );
}

/// **The §8.4 sweep's budget, hit through the binary on a vocabulary of three thousand triples.**
///
/// This is the end-to-end half of `docs/adr/0027`. The disjointness check makes one walk per
/// concept that has a `skos:related`, so its cost is the number of associated concepts times the
/// depth of the hierarchy — and a 1 500-deep chain with one associative link on each concept owes
/// 1 124 250 links, past `WalkBound::DEFAULT`'s million, in a file an operator could plausibly
/// import. Before iteration 30 the budget was per walk: no single walk came within three orders of
/// magnitude of it, so nothing stopped and the report said the check had **finished**.
///
/// What this asserts is the sentence an operator reads. Not that the check is fast — that it
/// declines to claim a result it did not compute.
#[test]
fn inspect_says_the_disjointness_check_was_abandoned_rather_than_claiming_it_passed() {
    const DEPTH: usize = 1_500;

    let mut turtle = String::from(
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/regions/> .\n",
    );
    for index in 0..DEPTH {
        if index > 0 {
            turtle.push_str(&format!(
                "ex:c{index:04} skos:broader ex:c{:04} .\n",
                index - 1
            ));
        }
        // One associate, outside the hierarchy and the same one for every concept, so nothing here
        // violates S27 and every walk runs all the way to the top. That is the expensive shape, and
        // it is perfectly ordinary SKOS.
        turtle.push_str(&format!("ex:c{index:04} skos:related ex:topic .\n"));
    }

    let dir = authored();
    import_and_approve(dir.path(), "deep.ttl", &turtle);

    let output = run(dir.path(), &["inspect", REGIONS]);
    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);

    assert!(
        report.contains("the disjointness check stopped after"),
        "the sweep ran out of budget and the report must say so: {report}"
    );
    assert!(
        report.contains("concept(s) unwalked"),
        "and must say how many it never reached: {report}"
    );
    assert!(
        report.contains("[unchecked]"),
        "abandoning a check is `Severity::Unchecked`, not a violation: {report}"
    );
    assert!(
        report.contains(
            "no SKOS integrity condition is violated by the part of this graph that was checked"
        ),
        "the closing sentence must be the hedged one, not the clean one: {report}"
    );
    // And the hedge must be earned: this vocabulary really does violate nothing.
    assert!(!report.contains("is not a SKOS vocabulary"), "{report}");
}

/// §10 end to end: a vocabulary whose hierarchy leaves the building. `skos:broadMatch` is a
/// sub-property of `skos:broader` under S41, so the walk climbs *through* the mapping into
/// somebody else's concept — which is the whole point of a mapping and the thing an operator
/// cannot see from a Turtle export, where the two properties look unrelated.
///
/// A build that kept the mapping properties in a section of their own would report this thesaurus
/// as one concept deep and every mapped vocabulary as an island.
#[test]
fn ancestors_climbs_through_a_mapping_link_off_disk_and_names_s41() {
    let dir = authored();
    import_and_approve(
        dir.path(),
        "mapped.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         @prefix theirs: <https://other.example.org/geo/> .\n\
         ex:japan a skos:Concept ; skos:prefLabel \"Japan\"@en ; skos:broader ex:eastasia .\n\
         ex:eastasia a skos:Concept ; skos:prefLabel \"East Asia\"@en ;\n\
             skos:broadMatch theirs:asia .\n",
    );

    let output = run(
        dir.path(),
        &["ancestors", REGIONS, "https://example.org/regions/japan"],
    );
    let report = stdout(&output);
    assert!(output.status.success(), "{}", stderr(&output));

    assert!(report.contains("2 concept(s) are above it"), "{report}");
    assert!(
        report.contains(
            "<https://example.org/regions/japan> → <https://example.org/regions/eastasia> → \
             <https://other.example.org/geo/asia>"
        ),
        "the walk must cross the mapping: {report}"
    );

    // And the report of the vocabulary says where that link came from, with the statement that
    // licensed the lift rather than a bare "inferred".
    let inspected = stdout(&run(dir.path(), &["inspect", REGIONS]));
    assert!(
        inspected.contains(
            "S41: skos:broadMatch is a sub-property of skos:broader, skos:narrowMatch is a"
        ),
        "{inspected}"
    );
    assert!(
        inspected.contains("1 hierarchical mapping link(s), 0 of them stated as skos:narrowMatch"),
        "{inspected}"
    );
    assert!(
        inspected.contains("its closure is walked rather than stored"),
        "the report says what its counts are and are not: {inspected}"
    );
    assert!(
        inspected.contains("no SKOS integrity condition is violated"),
        "{inspected}"
    );
}

/// S46 end to end — §10's only integrity condition, read by the real binary off disk. The clash
/// is between an exact mapping and a hierarchical one, which is Example 52, and the vocabulary is
/// refused the word "consistent".
#[test]
fn inspect_reports_an_exact_and_hierarchical_mapping_clash_off_disk() {
    let dir = authored();
    import_and_approve(
        dir.path(),
        "clash.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         @prefix theirs: <https://other.example.org/geo/> .\n\
         ex:japan a skos:Concept ; skos:prefLabel \"Japan\"@en ;\n\
             skos:exactMatch theirs:nippon ; skos:broadMatch theirs:nippon .\n",
    );

    let report = stdout(&run(dir.path(), &["inspect", REGIONS]));

    assert!(
        report.contains("skos:exactMatch <https://other.example.org/geo/nippon> (asserted)"),
        "{report}"
    );
    assert!(
        report.contains(
            "S46: skos:exactMatch is disjoint with each of the properties skos:broadMatch and \
             skos:relatedMatch."
        ),
        "the report must quote the statement, not merely cite it: {report}"
    );
    assert!(
        report.contains("violates a SKOS integrity condition"),
        "{report}"
    );
}

/// `openbiz mappings` end to end — the hub shape, read by the real binary off disk.
///
/// The house vocabulary maps a concept to a hub, and the hub maps onwards to a third party.
/// Nothing in the file joins the house concept to the third party, and the operator asks the one
/// question no serialisation can answer: what is this concept actually equivalent to?
#[test]
fn mappings_walks_a_chain_of_exact_matches_and_names_the_statement_that_licensed_each_step() {
    let dir = authored();
    import_and_approve(
        dir.path(),
        "hub.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         @prefix hub: <https://hub.example.org/geo/> .\n\
         @prefix reg: <https://regulator.example.org/geo/> .\n\
         ex:japan a skos:Concept ; skos:prefLabel \"Japan\"@en ;\n\
             skos:exactMatch hub:jp ; skos:broadMatch hub:eastasia .\n\
         hub:jp skos:exactMatch reg:JPN .\n",
    );

    let output = run(
        dir.path(),
        &["mappings", REGIONS, "https://example.org/regions/japan"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let report = stdout(&output);

    assert!(report.contains("(\"Japan\"@en)"), "{report}");
    assert!(
        report.contains("<https://hub.example.org/geo/jp>"),
        "{report}"
    );
    // The link the walk found, which the file states from neither end.
    assert!(
        report.contains("joined through a chain of exact matches"),
        "{report}"
    );
    assert!(
        report.contains("<https://regulator.example.org/geo/JPN>"),
        "the third party is reachable only by chaining: {report}"
    );
    assert!(
        report.contains("S45: skos:exactMatch is an instance of owl:TransitiveProperty."),
        "the rule must be quoted, not merely cited: {report}"
    );
    // And the hierarchical mapping says it is in the hierarchy `openbiz ancestors` walks.
    assert!(
        report.contains("each of these is also a skos:broader link"),
        "{report}"
    );
}

/// The false negative S45's absence used to produce, end to end. The house concept is mapped to a
/// hub, the hub onwards, and the house concept broad-matches the far end — a violation of S46 in
/// which **no statement in the file names an exact match and a hierarchical one between the same
/// pair**. Before the closure was walked this vocabulary was reported as consistent.
#[test]
fn inspect_finds_the_clash_that_only_the_exact_match_closure_reveals() {
    let dir = authored();
    import_and_approve(
        dir.path(),
        "chain-clash.ttl",
        "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
         @prefix ex: <https://example.org/regions/> .\n\
         @prefix hub: <https://hub.example.org/geo/> .\n\
         @prefix reg: <https://regulator.example.org/geo/> .\n\
         ex:japan a skos:Concept ; skos:prefLabel \"Japan\"@en ;\n\
             skos:exactMatch hub:jp ; skos:broadMatch reg:JPN .\n\
         hub:jp skos:exactMatch reg:JPN .\n",
    );

    let report = stdout(&run(dir.path(), &["inspect", REGIONS]));

    assert!(
        report.contains("S45: skos:exactMatch is an instance of owl:TransitiveProperty."),
        "the chain is the derivation and the report must print it: {report}"
    );
    assert!(
        report.contains(
            "S46: skos:exactMatch is disjoint with each of the properties skos:broadMatch and \
             skos:relatedMatch."
        ),
        "{report}"
    );
    assert!(
        report.contains("violates a SKOS integrity condition"),
        "a clash visible only through the closure is still a clash: {report}"
    );
}

/// The roll-call against a vocabulary on disk: every condition named, and the one this build
/// cannot check on this graph reported as unchecked rather than as held.
///
/// The fixture is deliberately the awkward shape rather than a broken one. It is a *consistent*
/// vocabulary — nothing in it violates anything — that also declares `ex:seeAlso` a sub-property
/// of `skos:related`. `openbiz inspect` says "no SKOS integrity condition is violated", which is
/// true and which an operator reads as "all of them were checked". They were not: statements made
/// with `ex:seeAlso` were read as non-SKOS, so §8.4's check ran over a graph missing the author's
/// own associative links, and S27 has no verdict on this vocabulary at all.
#[test]
fn integrity_names_every_condition_and_says_which_one_it_could_not_check() {
    let dir = authored();
    import_and_approve(
        dir.path(),
        "extension.ttl",
        r#"
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex: <https://example.org/regions/> .

ex:seeAlso rdfs:subPropertyOf skos:related .
ex:emea ex:seeAlso ex:apac .
"#,
    );

    let output = run(dir.path(), &["integrity", REGIONS]);
    assert!(
        output.status.success(),
        "integrity failed: {}",
        stderr(&output)
    );
    let report = stdout(&output);

    // All sixteen rows, whatever the vocabulary says.
    for number in [
        "S9", "S13", "S14", "S27", "S37", "S46", "S3", "S18", "S30", "S38", "S48", "S49", "S52",
        "S53", "S58", "S59",
    ] {
        assert!(report.contains(number), "{number} is missing:\n{report}");
    }

    // The specification's six are separated from the ten this build classifies itself.
    assert!(
        report.contains("the SKOS Reference's own integrity conditions"),
        "{report}"
    );
    assert!(
        report.contains("statements this build treats as contradictions, by our reading"),
        "{report}"
    );

    // S27 has no verdict, and the report says exactly why.
    assert!(report.contains("UNCHECKED"), "{report}");
    assert!(
        report.contains("<https://example.org/regions/seeAlso>"),
        "the report must name the declaration it read past:\n{report}"
    );
    assert!(
        report.contains("skos:related"),
        "and the SKOS property it reaches:\n{report}"
    );

    // And `inspect` on the same store still reports the vocabulary as violating nothing — which
    // is the sentence this command exists to take apart.
    let inspected = stdout(&run(dir.path(), &["inspect", REGIONS]));
    assert!(
        inspected.contains("no SKOS integrity condition is violated"),
        "{inspected}"
    );
    assert!(
        inspected.contains("openbiz integrity"),
        "inspect's summary must name the command that takes it apart:\n{inspected}"
    );
}

/// A vocabulary that violates §5.4 is named as such by S-number, with the specification's words.
#[test]
fn integrity_quotes_the_condition_a_vocabulary_violates() {
    let dir = authored();
    import_and_approve(
        dir.path(),
        "clash.ttl",
        r#"
@prefix skos: <http://www.w3.org/2004/02/skos/core#> .
@prefix ex: <https://example.org/regions/> .

ex:emea skos:prefLabel "EMEA region"@en .
"#,
    );

    let report = stdout(&run(dir.path(), &["integrity", REGIONS]));

    assert!(report.contains("VIOLATED"), "{report}");
    assert!(report.contains("not a SKOS vocabulary"), "{report}");
    assert!(
        report
            .contains("A resource has no more than one value of skos:prefLabel per language tag."),
        "the specification's own words are the point:\n{report}"
    );
}

/// An IRI with no registry entry is refused rather than reported as a vocabulary that satisfies
/// every condition — which it would, vacuously, and which is what a mistyped IRI produces.
#[test]
fn integrity_refuses_a_graph_the_store_does_not_hold() {
    let dir = authored();

    let output = run(dir.path(), &["integrity", "https://example.org/typo"]);
    assert!(!output.status.success(), "{}", stdout(&output));
    assert!(
        stderr(&output).contains("https://example.org/typo"),
        "{}",
        stderr(&output)
    );
}
