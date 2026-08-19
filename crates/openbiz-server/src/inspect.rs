//! `openbiz inspect` — read a vocabulary and say what is in it, in SKOS terms.
//!
//! This is the composition root for the SKOS core model: the one place where the store's
//! engine-free statement type is mapped onto the domain crate's, because neither crate depends on
//! the other and something has to join them. That join is three lines and it is the whole cost of
//! the layering; see `docs/adr/0019`.
//!
//! # Why a command and not an endpoint
//!
//! For once, not the authentication objection — this only reads. It is a command because it is the
//! *first* caller of the core model and its job is to make the model's answers reachable and
//! checkable now, from a shell, against a real store on disk. The interface will want the same
//! answers rendered as a tree with counts beside each scheme, and that is Phase 2's concept-tree
//! item, not this one. Shipping a half-tree behind HTTP to look further along would be the
//! "built but no production caller" failure in reverse: a caller with nothing behind it.
//!
//! # Why it prints every derivation
//!
//! `CLAUDE.md` §3 requires every inference to explain itself, and an explanation nobody can read
//! is not one. The report therefore prints each derived fact with its premise and the
//! specification statement that licensed it, however many there are, exactly as
//! `openbiz candidate <id>` prints a whole staging graph. A silent cap would read as "that is all
//! there was" — the one thing a report about inference must never imply. An operator with a large
//! vocabulary redirects to a file; an operator with a truncated report has no way to know.

use openbiz_skos::{
    ClassOrigin, CoreModel, LabelKind, LabelOrigin, Literal, MappingProperty, Node, NoteKind,
    PropertyRefinements, RelationOrigin, Resource, Retirement, Retirements, SemanticRelation,
    SkosClass, SkosRule, Statement, Term,
};
use openbiz_store::{StatementRef, StatementTerm, Store};

use crate::cli::CommandError;

/// Read the vocabulary at `graph` and report what it holds.
///
/// Reads and nothing else. The statements stream out of the store one at a time and into the
/// model, so peak memory is the model rather than the graph.
///
/// An IRI with no registry entry is refused rather than reported as an empty vocabulary — the
/// store draws that distinction (see [`Store::for_each_statement`]) and losing it here would turn
/// a typo into a report of a well-formed empty thesaurus.
pub fn inspect(store: &Store, graph: &str) -> Result<String, CommandError> {
    let (model, retirements) = read_with_retirements(store, graph)?;
    Ok(report(graph, &model, &retirements))
}

/// Read a vocabulary into the core model — **two passes over the store, not one**.
///
/// The first pass reads only `rdfs:subPropertyOf` and keeps the property graph; the second builds
/// the model knowing what the vocabulary's own note properties refine. The order is forced rather
/// than chosen: a declaration may sit after every statement that uses it, and RDF has no document
/// order for a single pass to rely on. `docs/adr/0028` records why the alternative — buffering the
/// unrecognised statements until the declarations arrive — was refused: it would trade a second
/// scan of the store for holding most of the graph in memory, and "peak memory is the model rather
/// than the graph" is a promise this command makes two paragraphs above.
///
/// Shared with `openbiz notes` so the two commands cannot disagree about what a vocabulary says.
pub(crate) fn read(store: &Store, graph: &str) -> Result<CoreModel, CommandError> {
    Ok(read_with_retirements(store, graph)?.0)
}

/// The same two passes, also collecting what the vocabulary says is no longer current.
///
/// `owl:deprecated` is not SKOS, so [`CoreModel`] has nothing to say about it and a
/// [`Retirements`] index is built beside it — from the **same** second pass, so a browse command
/// that marks its retired concepts costs no extra scan of the store. Every command that shows a
/// concept to a person goes through this one rather than through [`read`], which is what stops a
/// new read path from quietly forgetting that some of what it prints is obsolete.
pub(crate) fn read_with_retirements(
    store: &Store,
    graph: &str,
) -> Result<(CoreModel, Retirements), CommandError> {
    let mut refinements = PropertyRefinements::builder();
    store.for_each_statement(graph, |statement| refinements.push(convert(statement)))?;

    let mut builder = CoreModel::builder().with_refinements(refinements.build());
    let mut retirements = Retirements::builder();
    store.for_each_statement(graph, |statement| {
        let statement = convert(statement);
        retirements.push(statement.clone());
        builder.push(statement);
    })?;
    Ok((builder.build(), retirements.build()))
}

/// The store's borrowed statement as the domain crate's owned one.
///
/// The two types exist separately so that neither crate depends on the other, which is the
/// decision `docs/adr/0019` records. This is where that decision is paid for.
pub(crate) fn convert(statement: StatementRef<'_>) -> Statement {
    Statement {
        subject: node(statement.subject),
        predicate: statement.predicate.to_owned(),
        object: term(statement.object),
    }
}

/// A term in subject position, which RDF guarantees is never a literal.
///
/// A literal subject cannot come out of the store — Oxigraph's own subject type cannot hold one —
/// so this is a translation, not a decision about malformed data.
fn node(term: StatementTerm<'_>) -> Node {
    match term {
        StatementTerm::Iri(iri) => Node::iri(iri),
        StatementTerm::Blank(label) => Node::blank(label),
        // Unreachable through the store; mapped rather than panicked on, because a `todo!()` here
        // would be an `unwrap()` wearing a different hat (`CLAUDE.md` §6). A blank node labelled
        // with the lexical form is wrong in an obvious way rather than fatal in a silent one.
        StatementTerm::Literal { value, .. } => Node::blank(value),
    }
}

/// A term in object position, which may be any of the three kinds.
fn term(value: StatementTerm<'_>) -> Term {
    match value {
        StatementTerm::Iri(iri) => Term::Node(Node::iri(iri)),
        StatementTerm::Blank(label) => Term::Node(Node::blank(label)),
        StatementTerm::Literal {
            value,
            language,
            datatype,
        } => Term::Literal(Literal {
            value: value.to_owned(),
            language: language.map(str::to_owned),
            datatype: datatype.to_owned(),
        }),
    }
}

/// Render the model as the report an operator reads.
///
/// Sections in the order somebody asking "what is this vocabulary?" wants them: what is in it,
/// what languages it is in, whether it is authored in SKOS-XL, how it is organised, what was
/// inferred rather than stated, and what is wrong with it. A section with nothing to say is left
/// out rather than printed empty, except the last — "no findings" is the answer to a question that
/// was asked, and its absence would be indistinguishable from a report that does not check.
///
/// # Why the languages section is counts and not labels
///
/// Every other section is bounded by the *structure* of the vocabulary — its schemes, its
/// collections, its inferences. The labels are bounded by its size, and a hundred-thousand-concept
/// thesaurus would drown every other answer in this report. So the labels appear as coverage per
/// language plus the one number a governance team asks for first: how many concepts have no
/// preferred label at all. Listing the labels themselves is the concept tree's job, and that is
/// its own item.
fn report(graph: &str, model: &CoreModel, retirements: &Retirements) -> String {
    let mut out = format!(
        "<{graph}>\n  {} statement(s) read\n\n",
        model.statements_read()
    );

    for class in SkosClass::ALL {
        let total = model.count_of(class);
        let inferred = model
            .instances_of(class)
            .filter(|(_, resource)| {
                matches!(
                    resource.classes().get(&class),
                    Some(ClassOrigin::Entailed(_))
                )
            })
            .count();
        out.push_str(&format!("  {:<24}{total}", class.to_string()));
        if inferred > 0 {
            out.push_str(&format!("  ({inferred} inferred)"));
        }
        out.push('\n');
    }

    let coverage = model.label_coverage();
    let concepts = model.count_of(SkosClass::Concept);
    if !coverage.is_empty() || concepts > 0 {
        out.push_str("\nlanguages:\n");
        if coverage.is_empty() {
            // The section is printed empty rather than skipped, because the case that reaches
            // here is a vocabulary whose labels were *all* refused under S12 — the one time the
            // count below matters most, and the one time a missing section would read as "there
            // was nothing to say about labels".
            out.push_str("  none — nothing in this vocabulary carries a SKOS lexical label\n");
        }
        for language in &coverage {
            out.push_str(&format!("  {language}\n"));
        }
        let unlabelled = model
            .instances_of(SkosClass::Concept)
            .filter(|(_, resource)| resource.labels_of(LabelKind::Preferred).next().is_none())
            .count();
        // Consistent with SKOS — §5.6.4 says so outright — and still the first thing anybody
        // responsible for the vocabulary wants to know, so it is a count and not a finding.
        out.push_str(&format!(
            "  {unlabelled} concept(s) have no skos:prefLabel in any language\n"
        ));
    }

    // Left out entirely for a vocabulary authored in plain SKOS, which is most of them. A section
    // reading "0 labels" on every report would be noise; here its presence is the answer to "is
    // this thesaurus using SKOS-XL at all?", which is the first thing a migration asks.
    let labels: Vec<_> = model.instances_of(SkosClass::Label).collect();
    let labelled: Vec<_> = model
        .resources()
        .filter(|(_, resource)| !resource.xl_labels().is_empty())
        .collect();
    if !labels.is_empty() || !labelled.is_empty() {
        // Counted as *links* and not as statements. S62 closes every link into a pair, so summing
        // the relations a resource holds would report twice what an author wrote and read as a
        // vocabulary with twice the structure it has. A link stated in one direction and a link
        // stated in both are the same one link, which is what symmetry means; the second number
        // is the one that says how much of it we supplied.
        let links: usize = model
            .resources()
            .map(|(node, resource)| {
                resource
                    .label_relations()
                    .keys()
                    .filter(|other| *other >= node)
                    .count()
            })
            .sum();
        let converses: usize = model
            .resources()
            .map(|(_, resource)| {
                resource
                    .label_relations()
                    .values()
                    .filter(|origin| matches!(origin, RelationOrigin::Entailed(_)))
                    .count()
            })
            .sum();
        let with_form = labels
            .iter()
            .filter(|(_, resource)| resource.literal_forms().len() == 1)
            .count();
        // Counted from the labels themselves rather than from the derivation list, because the
        // two can differ: a resource that also states the plain label outright keeps the asserted
        // one and no derivation is recorded, which is the correct answer to both questions.
        let dumbed_down: usize = model
            .resources()
            .map(|(_, resource)| {
                resource
                    .labels()
                    .values()
                    .flat_map(|kinds| kinds.values())
                    .filter(|origin| matches!(origin, LabelOrigin::DumbedDown(_)))
                    .count()
            })
            .sum();
        out.push_str("\nskos-xl labels:\n");
        out.push_str(&format!(
            "  {} skosxl:Label resource(s), {with_form} with exactly one literal form\n",
            labels.len()
        ));
        out.push_str(&format!(
            "  {} resource(s) labelled through SKOS-XL, {dumbed_down} plain SKOS label(s) \
             inferred from them\n",
            labelled.len()
        ));
        // Printed only when there are any. B.4 is an extension point with no built-in
        // refinements, so most SKOS-XL thesauri use none of it, and a line reading "0 links" on
        // every one of them would be the noise this whole section is omitted to avoid.
        if links > 0 {
            out.push_str(&format!(
                "  {links} link(s) between labels, {converses} converse(s) inferred under S62\n"
            ));
        }
    }

    // Documentation — SKOS Reference §7. Printed for any vocabulary that has concepts, including
    // one that documents none of them, because "0 definitions" is the answer to the question a
    // governance team asks first and a missing section would read as "we did not look".
    //
    // **Coverage and not content.** The notes are the longest text a thesaurus holds and there is
    // roughly one definition per concept, so printing them would drown every other answer here —
    // the same reason the labels appear as coverage. `openbiz notes <graph> <resource>` prints
    // the notes themselves, one resource at a time.
    if concepts > 0 {
        out.push_str("\ndocumentation:\n");
        for row in model.documentation_coverage() {
            out.push_str(&format!("  {row}\n"));
        }
        // Neither a finding nor a complaint, and the line says so outright. §7 has no "Integrity
        // Conditions" subsection at all — §5.4 does — so a concept with no definition is
        // consistent SKOS. Whether it *ought* to have one is ANSI/NISO Z39.19 and ISO 25964,
        // which are rule packs in `openbiz-validate` and are not built yet. Saying which document
        // would ask the question is what stops an operator reading the zero as our verdict.
        out.push_str(
            "  §7 states no integrity condition, so an undocumented concept is consistent SKOS; \
             requiring a definition is a Z39.19 / ISO 25964 rule pack\n",
        );

        // Which properties the vocabulary declared for itself, named rather than merely counted.
        // §7.1 calls the seven "a set of extension points", and a thesaurus that has used them has
        // note properties whose names do not appear anywhere else in this report — so an author
        // checking "12 through a declared refinement" against their own file has nothing to search
        // for unless this line prints it.
        out.push_str(&refinements(model.refinements()));
    }

    let schemes: Vec<_> = model.instances_of(SkosClass::ConceptScheme).collect();
    if !schemes.is_empty() {
        out.push_str("\nconcept schemes:\n");
        for (node, resource) in schemes {
            out.push_str(&format!(
                "  {node}{}  {} top concept(s)\n",
                named(resource),
                resource.has_top_concept().len()
            ));
        }
    }

    // An ordered collection is also a collection under S29, so listing both classes would list it
    // twice. The order it is in is the thing worth saying about it, so that is what is said.
    let collections: Vec<_> = model.instances_of(SkosClass::Collection).collect();
    if !collections.is_empty() {
        out.push_str("\ncollections:\n");
        for (node, resource) in collections {
            out.push_str(&format!(
                "  {node}{}  {} member(s)",
                named(resource),
                resource.members().len()
            ));
            if resource.is_a(SkosClass::OrderedCollection) {
                let ordered = resource
                    .member_lists()
                    .iter()
                    .filter(|list| list.is_well_formed())
                    .count();
                out.push_str(&format!(", ordered by {ordered} well-formed list(s)"));
            }
            out.push('\n');
        }
    }

    // The hierarchy, which is what a thesaurus is bought for. Left out entirely for a vocabulary
    // that has none — a flat list of concepts is a legitimate thing to hold, and printing "0
    // links" on every one would be the noise the SKOS-XL section above is omitted to avoid.
    //
    // **Counted as links and not as statements**, for the reason that section gives: S25 closes
    // every hierarchical link into a pair, so summing what each resource holds would report twice
    // the structure the author wrote. `skos:broader` is counted and `skos:narrower` is not,
    // because after the closure they are the same links seen from the two ends.
    let hierarchical: usize = model
        .resources()
        .map(|(_, resource)| resource.broader_count())
        .sum();
    // Entailed **under S25** and not merely entailed. Now that section 10 is read, a
    // `skos:broader` link can also arrive under S41 from a `skos:broadMatch`, and calling that one
    // "stated as skos:narrower" would tell an author their file says something it does not. The
    // two are counted apart and the second gets its own line.
    let from_narrower: usize = model
        .resources()
        .filter_map(|(_, resource)| resource.relations(SemanticRelation::Broader))
        .flat_map(|links| links.values())
        .filter(|origin| **origin == RelationOrigin::Entailed(SkosRule::S25))
        .count();
    let from_mapping: usize = model
        .resources()
        .filter_map(|(_, resource)| resource.relations(SemanticRelation::Broader))
        .flat_map(|links| links.values())
        .filter(|origin| **origin == RelationOrigin::Entailed(SkosRule::S41))
        .count();
    let associative: usize = model
        .resources()
        .map(|(node, resource)| {
            resource
                .relations(SemanticRelation::Related)
                .map_or(0, |links| {
                    links.keys().filter(|other| *other >= node).count()
                })
        })
        .sum();
    let associative_converses: usize = model
        .resources()
        .filter_map(|(_, resource)| resource.relations(SemanticRelation::Related))
        .flat_map(|links| links.values())
        .filter(|origin| **origin == RelationOrigin::Entailed(SkosRule::S23))
        .count();
    // As above: an associative link lifted from a `skos:relatedMatch` under S41 is not a converse
    // that S23 supplied.
    let associative_from_mapping: usize = model
        .resources()
        .filter_map(|(_, resource)| resource.relations(SemanticRelation::Related))
        .flat_map(|links| links.values())
        .filter(|origin| **origin == RelationOrigin::Entailed(SkosRule::S41))
        .count();
    // A link the graph stated with `skos:broaderTransitive` itself rather than with
    // `skos:broader`. It is not in the count above and never will be: sub-property entailment
    // runs upwards, so nothing lifts a transitive link down to `skos:broader`. Reporting it
    // separately is what stops such a vocabulary reading as one with no hierarchy at all.
    let stated_transitive: usize = model
        .resources()
        .map(|(_, resource)| {
            resource
                .relations(SemanticRelation::BroaderTransitive)
                .map_or(0, |links| {
                    links
                        .values()
                        .filter(|origin| **origin == RelationOrigin::Asserted)
                        .count()
                })
        })
        .sum();
    if hierarchical > 0 || associative > 0 || stated_transitive > 0 {
        out.push_str("\nsemantic relations:\n");
        out.push_str(&format!(
            "  {hierarchical} hierarchical link(s), {from_narrower} of them stated as \
             skos:narrower\n"
        ));
        out.push_str(&format!(
            "  {associative} associative link(s), {associative_converses} converse(s) inferred \
             under S23\n"
        ));
        if from_mapping > 0 || associative_from_mapping > 0 {
            out.push_str(&format!(
                "  {from_mapping} hierarchical and {associative_from_mapping} associative \
                 link(s) were lifted from mapping links under S41\n"
            ));
        }
        // Polyhierarchy. §8 states nothing against it and ISO 25964 relies on it, so it is a
        // number and never a finding — but it is the number a migration from a strictly
        // single-parent source asks for first, so it is printed whenever there is any.
        let polyhierarchical = model
            .resources()
            .filter(|(_, resource)| resource.broader_count() > 1)
            .count();
        if polyhierarchical > 0 {
            out.push_str(&format!(
                "  {polyhierarchical} concept(s) have more than one broader concept \
                 (polyhierarchy)\n"
            ));
        }
        if stated_transitive > 0 {
            out.push_str(&format!(
                "  {stated_transitive} link(s) stated with skos:broaderTransitive or \
                 skos:narrowerTransitive rather than with skos:broader\n"
            ));
        }
        // S24 is answered by walking, not by storing, so the counts above are the *links* and
        // never the closure. Saying which is not pedantry: an operator who read "1 200
        // hierarchical links" as "1 200 ancestor relationships" would under-count a deep
        // vocabulary badly, and `openbiz ancestors` is the command that answers the other
        // question. See `docs/adr/0025`.
        out.push_str(
            "  counted as stated links; S24's transitive closure is not stored — \
             `openbiz ancestors <graph> <concept>` walks it\n",
        );
    }

    // Section 10 — the outward links, and the reason `CLAUDE.md` §1.7 exists: an enterprise with
    // nine overlapping vocabularies needs to see which of them are joined to anything. Left out
    // entirely for a vocabulary with no mappings, as the sections above are.
    //
    // **Counted as links and not as statements.** S43 pairs every `skos:broadMatch` with a
    // `skos:narrowMatch` and S44 makes the other three symmetric, so summing what each resource
    // holds would report twice the reach the author wrote. `skos:broadMatch` is counted and
    // `skos:narrowMatch` is not, because after the closure they are the same links seen from the
    // two ends — the rule the hierarchy above is counted by.
    let hierarchical_mappings: usize = model
        .resources()
        .map(|(_, resource)| {
            resource
                .mappings_of(MappingProperty::BroadMatch)
                .map_or(0, |links| links.len())
        })
        .sum();
    let from_narrow_match: usize = model
        .resources()
        .filter_map(|(_, resource)| resource.mappings_of(MappingProperty::BroadMatch))
        .flat_map(|links| links.values())
        .filter(|origin| matches!(origin, RelationOrigin::Entailed(_)))
        .count();
    let symmetric = |property: MappingProperty| -> usize {
        model
            .resources()
            .map(|(node, resource)| {
                resource.mappings_of(property).map_or(0, |links| {
                    links.keys().filter(|other| *other >= node).count()
                })
            })
            .sum()
    };
    let exact = symmetric(MappingProperty::ExactMatch);
    let close = symmetric(MappingProperty::CloseMatch);
    let associative_mappings = symmetric(MappingProperty::RelatedMatch);
    let close_from_exact: usize = model
        .resources()
        .map(|(node, resource)| {
            resource
                .mappings_of(MappingProperty::CloseMatch)
                .map_or(0, |links| {
                    links
                        .iter()
                        .filter(|(other, origin)| {
                            *other >= node && matches!(origin, RelationOrigin::Entailed(_))
                        })
                        .count()
                })
        })
        .sum();
    let mapped = model
        .resources()
        .filter(|(_, resource)| !resource.mappings().is_empty())
        .count();
    if mapped > 0 {
        out.push_str("\nmappings:\n");
        out.push_str(&format!(
            "  {hierarchical_mappings} hierarchical mapping link(s), {from_narrow_match} of them \
             stated as skos:narrowMatch\n"
        ));
        out.push_str(&format!(
            "  {associative_mappings} associative mapping link(s)\n"
        ));
        out.push_str(&format!(
            "  {exact} exact and {close} close equivalence mapping link(s), {close_from_exact} \
             of the close ones inferred under S42\n"
        ));
        out.push_str(&format!(
            "  {mapped} resource(s) in this graph carry at least one mapping link\n"
        ));
        // Said in every report that has a mapping in it, because a count of one-step links and a
        // count of equivalence classes are different numbers and an operator who read the first
        // as the second would under-count a chained vocabulary. Until iteration 33 this sentence
        // reported S45 as unimplemented; the closure is now walked rather than counted, so what
        // it says has changed and the count above has not.
        out.push_str(
            "  counted as the links held, one step each; S45 makes skos:exactMatch transitive \
             and its closure is walked rather than stored, so a chain of exact matches is \
             counted here as the links it states and resolved per concept by openbiz mappings\n",
        );
        // §10.6.1: using the mapping properties only across concept schemes is a convention, and a
        // mapping inside one scheme is consistent. Said out loud so that the count above is never
        // read as a complaint about a vocabulary that maps within itself.
        out.push_str(
            "  \u{a7}10.6.1 makes mapping across schemes a convention rather than a rule, so a \
             mapping inside one scheme is counted here and is never a finding\n",
        );
    }

    retirements_section(&mut out, model, retirements);

    let derivations = model.derivations();
    if !derivations.is_empty() {
        out.push_str(&format!(
            "\nwhy: {} fact(s) were inferred rather than stated\n",
            derivations.len()
        ));
        for derivation in derivations {
            out.push_str(&format!("  {derivation}\n"));
        }
    }

    let findings = model.findings();
    out.push_str(&format!("\nfindings: {}\n", findings.len()));
    for finding in findings {
        out.push_str(&format!("  [{}] {finding}\n", finding.severity()));
    }

    // Three sentences and not two. A check that gave up is not a check that passed, and until
    // this build had a bounded walk in it there was no way for the report to say so — which
    // `docs/UNTESTED.md` recorded as the sharpest false green in the tree.
    out.push_str(match (model.is_consistent(), model.checks_are_complete()) {
        (false, _) => {
            "\nthis graph violates a SKOS integrity condition and is not a SKOS vocabulary.\n"
        }
        (true, true) => "\nno SKOS integrity condition is violated by this graph.\n",
        (true, false) => {
            "\nno SKOS integrity condition is violated by the part of this graph that was \
             checked — one or more checks above were abandoned and say so.\n"
        }
    });

    // And the sentence above is a summary, so it names the command that takes it apart. "No
    // integrity condition is violated" does not say which conditions were *checked*, and a reader
    // who takes it for "all of them held" has read more into it than it says — which for a
    // vocabulary using its own extension points is wrong. `openbiz integrity` is per condition.
    out.push_str(
        "for the verdict on each SKOS integrity condition separately, and which of them this \
         build\ncould check over this vocabulary, run `openbiz integrity <graph>`.\n",
    );

    out
}

/// What the vocabulary says is no longer current, and what a retirement left behind.
///
/// Left out entirely for a vocabulary that retires nothing, which is most of them and the whole of
/// one that has never used `openbiz deprecate`. A row of zeroes on every report would be the noise
/// the SKOS-XL section above is omitted to avoid.
///
/// It is **not** phrased as findings. A retired concept with current children is the ordinary and
/// intended aftermath of a retirement — `openbiz deprecate` moves nothing, deliberately, because
/// whether each child should follow the replacement, be retired too, or stay where it is is a
/// decision only a person can take (`docs/adr/0040`). So these are counts of work outstanding,
/// which is what a governance function opens this report for, and not complaints about a
/// defective vocabulary.
fn retirements_section(out: &mut String, model: &CoreModel, retirements: &Retirements) {
    if retirements.is_empty() {
        return;
    }

    out.push_str("\nretirements:\n");

    let concepts = model.count_of(SkosClass::Concept);
    let retired: Vec<(&Node, &Retirement)> = retirements.retired().collect();
    let as_concepts = retired
        .iter()
        .filter(|(node, _)| {
            model
                .resource(node)
                .is_some_and(|resource| resource.is_a(SkosClass::Concept))
        })
        .count();
    out.push_str(&format!(
        "  {} resource(s) marked owl:deprecated, {as_concepts} of them concepts, out of \
         {concepts} concept(s)\n",
        retired.len()
    ));
    // OWL 2 §5.5 gives owl:deprecated no logical consequences, and a reader meeting a status
    // count in a SKOS report is entitled to think the marker changed something. It changed
    // nothing — that is what makes a retirement safe, and it is why every number below it is not
    // zero.
    out.push_str(
        "  owl:deprecated is an OWL 2 annotation with no logical consequences (\u{a7}5.5), and \
         SKOS defines no status term of its own; nothing about what these resources mean has \
         changed\n",
    );

    let dead_ends = retired
        .iter()
        .filter(|(_, retirement)| retirement.is_dead_end())
        .count();
    out.push_str(&format!(
        "  {} record what supersedes them with dcterms:isReplacedBy; {dead_ends} record nothing, \
         which is a term gone out of use with no successor and not an omission\n",
        retired.len() - dead_ends
    ));

    // The two things a retirement leaves standing, asked of the whole vocabulary at once. The
    // write half reports them one concept at a time, at the moment of retiring it, which is the
    // one moment nobody is looking at the backlog they add up to.
    let mut with_current_children = 0usize;
    let mut heading_schemes = 0usize;
    for (node, _) in &retired {
        if model
            .children(node)
            .any(|(child, _)| !retirements.is_retired(child))
        {
            with_current_children += 1;
        }
        if model
            .resource(node)
            .is_some_and(|resource| !resource.top_concept_of().is_empty())
        {
            heading_schemes += 1;
        }
    }
    if with_current_children > 0 {
        out.push_str(&format!(
            "  {with_current_children} of them still have concepts directly below them that are \
             not retired\n"
        ));
    }
    if heading_schemes > 0 {
        out.push_str(&format!(
            "  {heading_schemes} of them are still a scheme's top concept, so a browse of that \
             scheme still starts at a retired concept\n"
        ));
    }

    let unmarked: Vec<&Node> = retirements.unmarked().map(|(node, _)| node).collect();
    if !unmarked.is_empty() {
        out.push_str(&format!(
            "  {} resource(s) record a replacement without being marked owl:deprecated, so every \
             command here reads them as current:\n",
            unmarked.len()
        ));
        for node in unmarked {
            out.push_str(&format!("    {node}{}\n", named_of(model, node)));
        }
        out.push_str(
            "    openbiz deprecate writes both statements or neither, so this came from \
             elsewhere; it is the half of a retirement that has no visible effect\n",
        );
    }
}

/// A resource's label in parentheses, or nothing — for a node that may not be in the model at all.
fn named_of(model: &CoreModel, node: &Node) -> String {
    model.resource(node).map(named).unwrap_or_default()
}

/// The note properties this vocabulary declared for itself, one line each, or nothing.
///
/// Silent when the vocabulary declared none, which is the common case — a section that printed
/// "0 declared refinements" for every ordinary thesaurus would be noise, and unlike the coverage
/// table above there is no zero here anybody is asking about.
///
/// The arrow is `→` and not `rdfs:subPropertyOf` because the target may be several declarations
/// away: the line states what was *entailed*, and `openbiz notes` prints the chain that reached
/// it for any concept carrying one.
fn refinements(refinements: &PropertyRefinements) -> String {
    if refinements.is_empty() {
        return String::new();
    }
    let mut out = String::from("  this vocabulary declares note properties of its own:\n");
    for (property, kinds) in refinements.iter() {
        let targets: Vec<String> = kinds.keys().map(NoteKind::to_string).collect();
        out.push_str(&format!("    <{property}> → {}\n", targets.join(", ")));
    }
    out
}

/// A resource's label in parentheses, or nothing if it has none.
///
/// The language tag is printed with it. [`Resource::display_label`] picks deterministically but
/// arbitrarily across languages, so showing the tag is what keeps the report honest: a reader can
/// see they were given the German one rather than assuming the vocabulary has no English.
fn named(resource: &Resource) -> String {
    match resource.display_label() {
        Some(label) => format!("  ({label})"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use openbiz_store::{GraphId, RdfSyntax, Store};

    use super::inspect;
    use crate::cli::CommandError;

    const VOCABULARY: &str = "http://example.org/thesaurus";

    /// A store holding `turtle` in one registered vocabulary, ready to inspect.
    fn store_with(turtle: &str) -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(directory.path()).expect("an openable store");
        let target = GraphId::vocabulary(VOCABULARY).expect("a valid vocabulary IRI");
        store
            .create_vocabulary_graph(&target)
            .expect("a fresh registration");

        // Through the seam, exactly as a user's data arrives: proposed, then approved. Writing
        // directly would test the report against statements no production path can produce.
        let candidate = store
            .propose_import(
                &target,
                RdfSyntax::Turtle,
                turtle.as_bytes(),
                &openbiz_store::Provenance {
                    source: openbiz_store::CandidateSource::Import,
                    agent: "test".to_owned(),
                    note: "fixture".to_owned(),
                    confidence: None,
                },
            )
            .expect("a well-formed proposal");
        store
            .decide(candidate.id(), openbiz_store::Decision::Approve, "test")
            .expect("an approvable candidate");
        (directory, store)
    }

    const PREFIXES: &str = "@prefix skos: <http://www.w3.org/2004/02/skos/core#> .\n\
                            @prefix ex: <http://example.org/> .\n";

    #[test]
    fn a_vocabulary_is_reported_in_skos_terms() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             ex:scheme a skos:ConceptScheme ; skos:hasTopConcept ex:animals .
             ex:animals a skos:Concept ; skos:inScheme ex:scheme .
             ex:cat a skos:Concept ; skos:inScheme ex:scheme .
             ex:dog a skos:Concept ; skos:inScheme ex:scheme .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("skos:Concept"), "{report}");
        assert!(report.contains("<http://example.org/scheme>"), "{report}");
        assert!(report.contains("1 top concept(s)"), "{report}");
        assert!(
            report.contains("no SKOS integrity condition is violated"),
            "{report}"
        );
    }

    /// The report's whole reason for existing: an answer a user did not state, with its reason.
    #[test]
    fn an_inferred_fact_is_reported_with_the_rule_that_licensed_it() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             ex:cat a skos:Concept ; skos:topConceptOf ex:scheme .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        // Nothing typed ex:scheme, and nothing said skos:inScheme or skos:hasTopConcept.
        assert!(
            report.contains("were inferred rather than stated"),
            "{report}"
        );
        assert!(report.contains("S5"), "{report}");
        assert!(report.contains("S7"), "{report}");
        assert!(report.contains("S8"), "{report}");
        assert!(report.contains("1 top concept(s)"), "{report}");
    }

    #[test]
    fn a_violated_integrity_condition_is_named_and_the_graph_is_not_called_consistent() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             ex:muddle a skos:Concept, skos:ConceptScheme .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("[inconsistent]"), "{report}");
        assert!(report.contains("S9"), "{report}");
        assert!(
            report.contains("violates a SKOS integrity condition"),
            "{report}"
        );
    }

    /// Ill-formed is not inconsistent, and conflating them is how a tool refuses valid data.
    #[test]
    fn an_ill_formed_member_list_is_reported_without_calling_the_graph_inconsistent() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             @prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
             ex:group a skos:OrderedCollection ; skos:memberList ex:cell .
             ex:cell rdf:first ex:cat ; rdf:rest ex:cell .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("[ill-formed]"), "{report}");
        assert!(
            report.contains("no SKOS integrity condition is violated"),
            "{report}"
        );
    }

    /// A vocabulary created and not yet authored into. The seam refuses an empty *import*, so this
    /// state is reached by creating the graph and stopping — which is exactly what a user who has
    /// just made a vocabulary has, and the first thing they are likely to inspect.
    #[test]
    fn a_registered_but_empty_vocabulary_reports_nothing_rather_than_failing() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(directory.path()).expect("an openable store");
        store
            .create_vocabulary_graph(&GraphId::vocabulary(VOCABULARY).expect("a valid IRI"))
            .expect("a fresh registration");

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("0 statement(s) read"), "{report}");
        assert!(report.contains("findings: 0"), "{report}");
    }

    /// A typo must not read as "this vocabulary is empty and fine".
    #[test]
    fn an_unregistered_iri_is_refused_rather_than_reported_as_empty() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(directory.path()).expect("an openable store");

        let error = inspect(&store, "http://example.org/never-registered")
            .expect_err("an unregistered vocabulary");

        assert!(
            matches!(error, CommandError::Store(_)),
            "expected the store's refusal, got {error}"
        );
    }

    /// The labels are what a person recognises a vocabulary by, so the report leads with them.
    #[test]
    fn the_report_names_a_scheme_and_a_collection_by_their_labels() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             ex:scheme a skos:ConceptScheme ;
                 skos:prefLabel \"Regions\"@en ;
                 skos:hasTopConcept ex:emea .
             ex:emea a skos:Concept ; skos:prefLabel \"Europe\"@en .
             ex:group a skos:Collection ;
                 skos:prefLabel \"Reporting groups\"@en ;
                 skos:member ex:emea .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("(\"Regions\"@en)"), "{report}");
        assert!(report.contains("(\"Reporting groups\"@en)"), "{report}");
    }

    /// The multilingual gap, which is the number a translation programme is actually managing.
    #[test]
    fn the_report_shows_coverage_per_language_and_what_is_unlabelled() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             ex:cat a skos:Concept ; skos:prefLabel \"cat\"@en ; skos:prefLabel \"chat\"@fr .
             ex:dog a skos:Concept ; skos:prefLabel \"dog\"@en ; skos:altLabel \"hound\"@en .
             ex:fish a skos:Concept .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("languages:"), "{report}");
        assert!(
            report.contains("@en  2 preferred on 2 resource(s), 1 alternative, 0 hidden"),
            "{report}"
        );
        assert!(
            report.contains("@fr  1 preferred on 1 resource(s), 0 alternative, 0 hidden"),
            "{report}"
        );
        assert!(
            report.contains("1 concept(s) have no skos:prefLabel in any language"),
            "{report}"
        );
        // Unlabelled is a fact about the vocabulary, not a violation of SKOS — §5.6.4.
        assert!(
            report.contains("no SKOS integrity condition is violated"),
            "{report}"
        );
    }

    /// S14 through the real store: the commonest defect in a thesaurus merged from two sources.
    #[test]
    fn two_preferred_labels_in_one_language_make_the_vocabulary_inconsistent() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             ex:cat a skos:Concept ; skos:prefLabel \"cat\"@en ; skos:prefLabel \"feline\"@en .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("[inconsistent]"), "{report}");
        assert!(report.contains("S14"), "{report}");
        assert!(
            report.contains("violates a SKOS integrity condition"),
            "{report}"
        );
    }

    /// S13, and the tag comparison that decides whether it fires.
    #[test]
    fn one_label_under_two_properties_is_inconsistent_but_two_tags_are_not() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             ex:cat a skos:Concept ; skos:prefLabel \"cat\"@en ; skos:altLabel \"cat\"@en .
            "
        ));
        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");
        assert!(report.contains("S13"), "{report}");
        assert!(report.contains("[inconsistent]"), "{report}");

        // Example 19 of the SKOS Reference: the same text in two tags is consistent.
        let (_other_directory, other) = store_with(&format!(
            "{PREFIXES}
             ex:cat a skos:Concept ; skos:prefLabel \"cat\"@en ; skos:altLabel \"cat\"@en-GB .
            "
        ));
        let report = inspect(&other, VOCABULARY).expect("a readable vocabulary");
        assert!(
            report.contains("no SKOS integrity condition is violated"),
            "{report}"
        );
    }

    /// S12 is a usage convention, so a typed label is reported and the vocabulary still stands.
    #[test]
    fn a_label_that_is_not_a_plain_literal_is_ill_formed_rather_than_inconsistent() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
             ex:cat a skos:Concept ; skos:prefLabel \"4\"^^xsd:integer .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("[ill-formed]"), "{report}");
        assert!(report.contains("S12"), "{report}");
        assert!(
            report.contains("no SKOS integrity condition is violated"),
            "{report}"
        );
        // The refused label left the concept unlabelled, and the report says so. This is the
        // case that made the languages section print when it has nothing to list: skipping it
        // would hide the count exactly when every label in the vocabulary had been refused.
        assert!(report.contains("languages:"), "{report}");
        assert!(
            report.contains("nothing in this vocabulary carries a SKOS lexical label"),
            "{report}"
        );
        assert!(
            report.contains("1 concept(s) have no skos:prefLabel in any language"),
            "{report}"
        );
    }

    /// The coverage table counts a refined note, and the report **names the property** it came
    /// from — a number an author cannot check against their own file is not a report.
    #[test]
    fn the_coverage_table_names_the_vocabularys_own_note_properties() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

ex:usageNote rdfs:subPropertyOf skos:scopeNote .

ex:Chemistry a skos:Concept ;
  skos:prefLabel \"Chemistry\"@en ;
  ex:usageNote \"Use for the discipline.\"@en ."
        ));

        let report = match inspect(&store, VOCABULARY) {
            Ok(report) => report,
            Err(error) => unreachable!("the vocabulary is registered: {error}"),
        };

        assert!(report.contains("documentation:"), "{report}");
        assert!(
            report.contains("through a declared refinement"),
            "the scope-note row must say the note was not written under skos:scopeNote: {report}"
        );
        assert!(
            report.contains("this vocabulary declares note properties of its own"),
            "{report}"
        );
        assert!(
            report.contains("<http://example.org/usageNote> → skos:scopeNote"),
            "{report}"
        );
        // And S17 still runs on top of it, so `skos:note` is 1 rather than 0.
        assert!(report.contains("inferred under S17"), "{report}");
    }

    /// An ordinary thesaurus declares nothing, and the section stays silent. A "0 declared
    /// refinements" line on every vocabulary would be noise, unlike the coverage zeroes above it
    /// which are the answer to a question somebody asked.
    #[test]
    fn a_vocabulary_that_declares_no_refinements_gets_no_refinement_section() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
ex:Chemistry a skos:Concept ;
  skos:prefLabel \"Chemistry\"@en ;
  skos:definition \"The study of matter.\"@en ."
        ));

        let report = match inspect(&store, VOCABULARY) {
            Ok(report) => report,
            Err(error) => unreachable!("the vocabulary is registered: {error}"),
        };

        assert!(report.contains("documentation:"), "{report}");
        assert!(
            !report.contains("declares note properties of its own"),
            "{report}"
        );
        assert!(
            !report.contains("through a declared refinement"),
            "{report}"
        );
    }

    /// The mapping section: what a vocabulary is joined to, counted as links rather than as the
    /// statements S43 and S44 double.
    #[test]
    fn the_report_counts_mapping_links_once_and_says_what_it_did_not_close() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             @prefix theirs: <http://other.example.org/> .
             ex:cat a skos:Concept ;
                 skos:prefLabel \"Cat\"@en ;
                 skos:exactMatch theirs:feline ;
                 skos:broadMatch theirs:mammal ;
                 skos:relatedMatch theirs:pet .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("\nmappings:\n"), "{report}");
        assert!(
            report.contains("1 hierarchical mapping link(s), 0 of them stated as skos:narrowMatch"),
            "{report}"
        );
        assert!(report.contains("1 associative mapping link(s)"), "{report}");
        assert!(
            report.contains(
                "1 exact and 1 close equivalence mapping link(s), 1 of the close ones inferred \
                 under S42"
            ),
            "{report}"
        );
        // Four resources: the concept, and the three it maps to, each of which S20 typed.
        assert!(
            report.contains("4 resource(s) in this graph carry at least one mapping link"),
            "{report}"
        );
        // The counts are one-step links and the report says so, because a reader who took
        // "1 exact" for "1 equivalence class" would under-count a chained vocabulary. What the
        // sentence claims changed at iteration 33 — the closure is now walked — and the count
        // it qualifies did not.
        assert!(
            report.contains("counted as the links held, one step each"),
            "{report}"
        );
        assert!(
            report.contains("resolved per concept by openbiz mappings"),
            "the report must name the command that answers the question it declines: {report}"
        );
        assert!(
            report.contains("no SKOS integrity condition is violated"),
            "{report}"
        );
    }

    /// A vocabulary that maps nothing gets no mapping section at all — the rule the SKOS-XL
    /// section follows, and the reason the section's presence is itself an answer.
    #[test]
    fn a_vocabulary_with_no_mappings_gets_no_mapping_section() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             ex:cat a skos:Concept ; skos:prefLabel \"Cat\"@en .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");
        assert!(!report.contains("mappings:"), "{report}");
    }

    /// S46 through the report: the clash is named, the statement is quoted, and the closing
    /// sentence refuses to call the vocabulary consistent.
    #[test]
    fn an_exact_match_clashing_with_a_hierarchical_one_is_reported_as_inconsistent() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             @prefix theirs: <http://other.example.org/> .
             ex:cat a skos:Concept ;
                 skos:exactMatch theirs:feline ;
                 skos:broadMatch theirs:feline .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(report.contains("skos:broadMatch (asserted)"), "{report}");
        assert!(
            report.contains(
                "S46: skos:exactMatch is disjoint with each of the properties skos:broadMatch"
            ),
            "{report}"
        );
        assert!(
            report.contains("violates a SKOS integrity condition"),
            "{report}"
        );
    }

    /// A mapped hierarchy is a hierarchy: S41 puts `skos:broadMatch` under `skos:broader`, so the
    /// semantic relation counts include it and `openbiz ancestors` can walk through it. A build
    /// that kept mappings in a section of their own would report this vocabulary as flat.
    ///
    /// And the lifted link is **not** counted as one "stated as skos:narrower", which is what that
    /// line said before section 10 was read and would now be a report of a statement nobody wrote.
    #[test]
    fn a_mapped_hierarchy_is_counted_as_hierarchy_and_cites_s41() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}
             @prefix theirs: <http://other.example.org/> .
             ex:cat a skos:Concept ; skos:broadMatch theirs:mammal .
            "
        ));

        let report = inspect(&store, VOCABULARY).expect("a readable vocabulary");

        assert!(
            report.contains("1 hierarchical link(s), 0 of them stated as skos:narrower"),
            "{report}"
        );
        assert!(
            report.contains(
                "1 hierarchical and 0 associative link(s) were lifted from mapping links under S41"
            ),
            "{report}"
        );
        assert!(
            report.contains(
                "S41: skos:broadMatch is a sub-property of skos:broader, skos:narrowMatch is a"
            ),
            "the lift must explain itself with the statement that licensed it: {report}"
        );
    }

    /// A vocabulary mid-retirement: one concept retired with a successor and current concepts
    /// still under it, one retired with no successor while heading a scheme, and one carrying the
    /// half-retirement `openbiz deprecate` cannot produce.
    const RETIRING: &str = r#"
        @prefix skos: <http://www.w3.org/2004/02/skos/core#> .
        @prefix owl: <http://www.w3.org/2002/07/owl#> .
        @prefix dcterms: <http://purl.org/dc/terms/> .
        @prefix ex: <http://example.org/> .

        ex:scheme a skos:ConceptScheme ; skos:hasTopConcept ex:wireless .

        ex:wireless a skos:Concept ; skos:prefLabel "Wireless"@en ;
            skos:topConceptOf ex:scheme ;
            owl:deprecated true .
        ex:aerials a skos:Concept ; skos:prefLabel "Aerials"@en ; skos:broader ex:wireless ;
            owl:deprecated true ; dcterms:isReplacedBy ex:antennas .
        ex:masts a skos:Concept ; skos:prefLabel "Masts"@en ; skos:broader ex:aerials .
        ex:antennas a skos:Concept ; skos:prefLabel "Antennas"@en .
        ex:telegraphy a skos:Concept ; skos:prefLabel "Telegraphy"@en ;
            dcterms:isReplacedBy ex:antennas .
    "#;

    /// The whole-vocabulary view of a retirement backlog, which is the thing `openbiz deprecate`
    /// reports one concept at a time and nobody sees added up.
    #[test]
    fn the_report_counts_what_the_vocabulary_has_retired() {
        let (_directory, store) = store_with(RETIRING);
        let report = inspect(&store, VOCABULARY).expect("a registered vocabulary");

        assert!(report.contains("\nretirements:\n"), "{report}");
        assert!(
            report.contains(
                "2 resource(s) marked owl:deprecated, 2 of them concepts, out of 5 \
                             concept(s)"
            ),
            "{report}"
        );
        assert!(
            report.contains(
                "1 record what supersedes them with dcterms:isReplacedBy; 1 record \
                             nothing"
            ),
            "{report}"
        );
    }

    /// OWL 2 §5.5 gives the marker no logical consequences, and a count in a SKOS report is
    /// exactly where a reader would assume otherwise.
    #[test]
    fn the_report_says_the_marker_changed_no_meaning() {
        let (_directory, store) = store_with(RETIRING);
        let report = inspect(&store, VOCABULARY).expect("a registered vocabulary");

        assert!(
            report.contains("an OWL 2 annotation with no logical consequences"),
            "{report}"
        );
        assert!(
            report.contains("SKOS defines no status term of its own"),
            "{report}"
        );
    }

    /// The two things a retirement leaves standing. Counted, not phrased as findings: leaving
    /// them is the deliberate decision `docs/adr/0040` records, not a defect.
    #[test]
    fn the_report_counts_what_the_retirements_left_standing() {
        let (_directory, store) = store_with(RETIRING);
        let report = inspect(&store, VOCABULARY).expect("a registered vocabulary");

        // Wireless keeps Aerials, which is retired; Aerials keeps Masts, which is not.
        assert!(
            report.contains(
                "1 of them still have concepts directly below them that are not \
                             retired"
            ),
            "{report}"
        );
        assert!(
            report.contains("1 of them are still a scheme's top concept"),
            "{report}"
        );
        // And they are counts, not complaints: nothing here becomes a finding.
        assert!(report.contains("\nfindings: 0\n"), "{report}");
    }

    /// The half-retirement that reads as a perfectly current concept everywhere else.
    #[test]
    fn a_replacement_recorded_without_the_marker_is_named() {
        let (_directory, store) = store_with(RETIRING);
        let report = inspect(&store, VOCABULARY).expect("a registered vocabulary");

        assert!(
            report.contains(
                "1 resource(s) record a replacement without being marked \
                             owl:deprecated"
            ),
            "{report}"
        );
        assert!(
            report.contains("<http://example.org/telegraphy>  (\"Telegraphy\"@en)"),
            "named rather than counted, because a count of one is not actionable: {report}"
        );
    }

    /// The section is absent from every vocabulary that has never retired anything, which is most
    /// of them.
    #[test]
    fn a_vocabulary_that_retires_nothing_has_no_retirements_section() {
        let (_directory, store) = store_with(&format!(
            "{PREFIXES}ex:cats a skos:Concept ; skos:prefLabel \"Cats\"@en .\n"
        ));
        let report = inspect(&store, VOCABULARY).expect("a registered vocabulary");

        assert!(!report.contains("retirements:"), "{report}");
    }
}
