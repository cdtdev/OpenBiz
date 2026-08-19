//! How a retired concept is shown, in every command that reads a vocabulary.
//!
//! `openbiz deprecate` marks a concept `owl:deprecated` and **removes nothing** — the decision
//! `docs/adr/0040` records, and the reason a retirement is safe: the IRI keeps resolving, every
//! reference to it keeps working, and an auditor asking in three years what the term meant still
//! gets an answer. The cost of that decision is paid here. Nothing about a retired concept changes
//! in the graph, so unless a read path looks for the marker it shows a retired concept exactly as
//! it shows a current one, and the operator who has just retired a term is offered it again by the
//! next search.
//!
//! # The decision, taken once for every command: show and mark
//!
//! There were three options per command — show, mark, or hide by default — and this build takes
//! the same one everywhere. **Nothing is hidden.**
//!
//! - **Hiding breaks the hierarchy.** A retired concept with current concepts below it is the
//!   commonest outcome of a retirement, because `openbiz deprecate` deliberately does not touch
//!   the children. Dropping it from `openbiz tree` would leave those children hanging off nothing
//!   and would silently misreport the shape of the vocabulary.
//! - **Hiding a search hit manufactures the silo.** `CLAUDE.md` §1.7 and `openbiz search`'s own
//!   documentation say the same thing: someone looks for a term, does not find it, and creates a
//!   duplicate. A retired concept omitted from search results reads as "this vocabulary has never
//!   heard of it", which is the one conclusion most likely to produce a second, worse copy of a
//!   term that already exists. Told "it exists and it is retired, use this instead", the same
//!   person does the right thing.
//! - **Showing without marking is the status quo, and it is the defect.**
//!
//! So a retired concept appears wherever it appeared before, carrying [`MARKER`], and the concept
//! the report is *about* gets the full sentence from [`explain`]. Filtering retired concepts out
//! on request is a real need and a separate plan item: it is an opt-in per command, not a default,
//! and it is not built here.
//!
//! # Marked in a list, explained at the focus
//!
//! A subtree of a thousand descendants would be unreadable with a three-line retirement notice
//! against each one, and `openbiz tree` prints its derivation as structure for exactly that
//! reason. So a concept in a *list* carries the marker only, and the concept the command was
//! asked about carries the explanation and the signpost. Nothing is withheld: `openbiz deprecate`
//! and `openbiz inspect` both report the whole picture, and asking about the marked concept
//! directly gives the full account of it.

use openbiz_skos::{CoreModel, Node, Resource, Retirements};

/// What a retired concept carries in a list, appended after its IRI and label.
///
/// Short by design: it goes on lines that already carry an IRI and a label, and it appears once
/// per retired concept in a subtree that may hold hundreds.
pub(crate) const MARKER: &str = "  [retired]";

/// What a concept recording a replacement without the marker carries.
///
/// A different mark for a different state, and not silence: this is the half-retirement
/// `openbiz deprecate` cannot produce, so it arrived by import or by hand, and it reads as fully
/// current to every other line of every report.
pub(crate) const UNMARKED: &str = "  [replaced, but not marked retired]";

/// The mark for one resource in a list, or nothing when the vocabulary says nothing about it.
pub(crate) fn mark(retirements: &Retirements, node: &Node) -> &'static str {
    match retirements.get(node) {
        Some(retirement) if retirement.is_retired() => MARKER,
        Some(retirement) if retirement.is_unmarked() => UNMARKED,
        _ => "",
    }
}

/// The full account of one resource's status, indented to `indent`, or nothing when it is current.
///
/// For the concept a report is *about*. It says three things a marker cannot: that the concept is
/// still there, what to use instead, and — when there is nothing to use instead — that the absence
/// is the vocabulary's answer rather than this report's omission.
pub(crate) fn explain(
    out: &mut String,
    indent: &str,
    retirements: &Retirements,
    model: &CoreModel,
    node: &Node,
) {
    let Some(retirement) = retirements.get(node) else {
        return;
    };

    if retirement.is_retired() {
        out.push_str(&format!(
            "{indent}retired: the vocabulary marks it owl:deprecated. it still exists and every \
             reference to it still resolves — a deprecation removes nothing.\n"
        ));
    } else if retirement.is_unmarked() {
        out.push_str(&format!(
            "{indent}a replacement is recorded for it and it is not marked owl:deprecated, so \
             every command here reads it as current. one of the two statements a retirement is \
             made of is missing.\n"
        ));
    } else {
        return;
    }

    if retirement.replaced_by().is_empty() {
        out.push_str(&format!(
            "{indent}nothing is recorded as replacing it: the vocabulary says the term is out of \
             use and offers no successor.\n"
        ));
        return;
    }

    // Plural because dcterms:isReplacedBy states no cardinality. A concept superseded by several
    // is how a vocabulary records a division that has already happened, and picking one of them
    // for the reader would be this report inventing an answer.
    out.push_str(&format!(
        "{indent}use instead, by dcterms:isReplacedBy ({} recorded):\n",
        retirement.replaced_by().len()
    ));
    for replacement in retirement.replaced_by() {
        out.push_str(&format!(
            "{indent}  {replacement}{}{}\n",
            named_in(model, replacement),
            mark(retirements, replacement)
        ));
        // A signpost pointing at a retired concept leads nowhere, and the reader has to be told
        // rather than following it. This is the trail `openbiz deprecate` refuses to *create* and
        // cannot refuse to *find*: the replacement may have been retired long afterwards.
        if retirements.is_retired(replacement) {
            out.push_str(&format!(
                "{indent}    which is itself retired, so this trail does not end at a current \
                 concept\n"
            ));
        }
        // A replacement in another vocabulary is ordinary governance, and this graph cannot say
        // anything about its status. Saying so is the difference between "not retired" and
        // "not known here".
        if model.resource(replacement).is_none() {
            out.push_str(&format!(
                "{indent}    which this vocabulary does not describe, so nothing here says what \
                 it is or whether it is current\n"
            ));
        }
    }
}

/// A resource's preferred label in parentheses, or nothing if it has none.
fn named_in(model: &CoreModel, node: &Node) -> String {
    match model.resource(node).and_then(Resource::display_label) {
        Some(label) => format!("  ({label})"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use openbiz_skos::{CoreModel, Node, Retirements, Statement, XSD_STRING};

    use super::{explain, mark, MARKER, UNMARKED};

    /// A model and a retirement index from the same Turtle-shaped statements, which is what every
    /// caller builds.
    fn read(turtle: &str) -> (CoreModel, Retirements) {
        let statements: Vec<Statement> = parse(turtle);
        (
            CoreModel::from_statements(statements.iter().cloned()),
            Retirements::from_statements(statements),
        )
    }

    /// A deliberately tiny N-Triples reader: `<s> <p> <o> .`, `<s> <p> "lit"@en .`, or
    /// `<s> <p> "lit"^^<iri> .`, one per line. The store is not in this test's way, so neither is
    /// a parser — but the typed form is read, because `owl:deprecated` is written typed and a
    /// fixture that could only express the lenient form would test the leniency and not the rule.
    fn parse(text: &str) -> Vec<Statement> {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| {
                let line = line.trim_end_matches(" .");
                let (subject, rest) = line.split_once(' ').expect("a subject");
                let (predicate, object) = rest.split_once(' ').expect("a predicate");
                let subject = Node::iri(subject.trim_matches(|c| c == '<' || c == '>'));
                let predicate = predicate.trim_matches(|c| c == '<' || c == '>').to_owned();
                match object.strip_prefix('"') {
                    Some(literal) => {
                        let (value, suffix) = literal.split_once('"').expect("a closing quote");
                        let (language, datatype) = match suffix.split_at(2) {
                            ("^^", iri) => {
                                (None, iri.trim_matches(|c| c == '<' || c == '>').to_owned())
                            }
                            _ if suffix.is_empty() => (None, XSD_STRING.to_owned()),
                            _ => (
                                Some(suffix.trim_start_matches('@').to_owned()),
                                openbiz_skos::RDF_LANG_STRING.to_owned(),
                            ),
                        };
                        Statement::new(
                            subject,
                            predicate,
                            openbiz_skos::Term::Literal(openbiz_skos::Literal {
                                value: value.to_owned(),
                                language,
                                datatype,
                            }),
                        )
                    }
                    None => Statement::new(
                        subject,
                        predicate,
                        Node::iri(object.trim_matches(|c| c == '<' || c == '>')),
                    ),
                }
            })
            .collect()
    }

    const CONCEPT: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
    const PREF: &str = "http://www.w3.org/2004/02/skos/core#prefLabel";
    const DEPRECATED: &str = "http://www.w3.org/2002/07/owl#deprecated";
    const REPLACED_BY: &str = "http://purl.org/dc/terms/isReplacedBy";
    const BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";

    #[test]
    fn a_current_resource_carries_no_mark_and_no_explanation() {
        let (model, retirements) = read(&format!(
            "<http://example.org/a> <{CONCEPT}> <http://www.w3.org/2004/02/skos/core#Concept> .\n"
        ));
        let node = Node::iri("http://example.org/a");

        assert_eq!(mark(&retirements, &node), "");
        let mut out = String::new();
        explain(&mut out, "", &retirements, &model, &node);
        assert!(out.is_empty(), "{out}");
    }

    #[test]
    fn the_two_states_carry_different_marks() {
        let (_model, retirements) = read(&format!(
            "<http://example.org/a> <{DEPRECATED}> \"true\"^^<{BOOLEAN}> .\n\
             <http://example.org/b> <{REPLACED_BY}> <http://example.org/c> .\n"
        ));

        assert_eq!(
            mark(&retirements, &Node::iri("http://example.org/a")),
            MARKER
        );
        assert_eq!(
            mark(&retirements, &Node::iri("http://example.org/b")),
            UNMARKED
        );
    }

    /// The trail `openbiz deprecate` refuses to create and cannot refuse to find: the replacement
    /// may have been retired long after it was named. A reader following the signpost has to be
    /// told it leads somewhere obsolete rather than discovering it one command later.
    #[test]
    fn a_replacement_that_is_itself_retired_is_called_out() {
        let (model, retirements) = read(&format!(
            "<http://example.org/old> <{DEPRECATED}> \"true\"^^<{BOOLEAN}> .\n\
             <http://example.org/old> <{REPLACED_BY}> <http://example.org/newer> .\n\
             <http://example.org/newer> <{CONCEPT}> \
             <http://www.w3.org/2004/02/skos/core#Concept> .\n\
             <http://example.org/newer> <{PREF}> \"Newer\"@en .\n\
             <http://example.org/newer> <{DEPRECATED}> \"true\"^^<{BOOLEAN}> .\n"
        ));

        let mut out = String::new();
        explain(
            &mut out,
            "",
            &retirements,
            &model,
            &Node::iri("http://example.org/old"),
        );
        assert!(
            out.contains("<http://example.org/newer>  (\"Newer\"@en)  [retired]"),
            "{out}"
        );
        assert!(
            out.contains("this trail does not end at a current concept"),
            "{out}"
        );
    }

    /// A replacement in the corporate vocabulary next door is ordinary governance, and this graph
    /// cannot say whether it is current. "Not retired" and "not known here" are different answers.
    #[test]
    fn a_replacement_this_vocabulary_does_not_describe_says_so() {
        let (model, retirements) = read(&format!(
            "<http://example.org/old> <{DEPRECATED}> \"true\"^^<{BOOLEAN}> .\n\
             <http://example.org/old> <{REPLACED_BY}> <http://elsewhere.example/term> .\n"
        ));

        let mut out = String::new();
        explain(
            &mut out,
            "",
            &retirements,
            &model,
            &Node::iri("http://example.org/old"),
        );
        assert!(
            out.contains("which this vocabulary does not describe"),
            "{out}"
        );
        assert!(!out.contains("this trail does not end"), "{out}");
    }

    /// The indent is the caller's, because a hit in a search result is indented under it and a
    /// concept a report is about is not.
    #[test]
    fn the_explanation_is_indented_where_the_caller_asks() {
        let (model, retirements) = read(&format!(
            "<http://example.org/old> <{DEPRECATED}> \"true\"^^<{BOOLEAN}> .\n"
        ));

        let mut out = String::new();
        explain(
            &mut out,
            "    ",
            &retirements,
            &model,
            &Node::iri("http://example.org/old"),
        );
        assert!(out.lines().all(|line| line.starts_with("    ")), "{out}");
    }
}
