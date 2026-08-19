//! Putting back a concept that was retired in error, and keeping the record that it was.
//!
//! [`deprecate`](crate::deprecate) writes the retirement and [`status`](crate::status) reads it
//! back. This is the third and last part of the lifecycle, and the first operation in this build
//! whose whole purpose is to **remove** statements: everything before it either added only, or
//! removed as a side effect of repointing something. A retirement is a claim a vocabulary makes
//! about itself — *this term is no longer current* — and a claim made in error is retracted, not
//! annotated.
//!
//! Like every other operation in this crate it writes nothing: a [`Reinstatement`] is an *answer*
//! — the statements it would remove and the one it would add — computed against a
//! [`CoreModel`] and a [`ReinstatementScan`] read a moment ago. The caller stages them as a
//! candidate; a human approves them.
//!
//! # What comes out, and why the replacement comes out with the marker
//!
//! Both halves of what [`deprecate`](crate::deprecate) writes:
//!
//! - **`owl:deprecated`** — every statement about the resource whose object this build reads as
//!   `true`. *Every* one, not the first: a vocabulary that arrived from two tools can carry the
//!   typed literal and the plain one, and leaving either behind means the resource is still
//!   retired everywhere while this command reports that it is not.
//! - **`dcterms:isReplacedBy`** — every statement about the resource, whatever its object.
//!
//! The second is the decision worth arguing. Removing only the marker would leave a resource that
//! is current and records a successor, which is exactly the half-retirement
//! [`Retirement::is_unmarked`](crate::Retirement::is_unmarked) exists to report and which
//! `openbiz inspect` calls out as the most likely way a retirement goes wrong. DCMI defines
//! `dcterms:isReplacedBy` as "a related resource that supplants, displaces, or supersedes the
//! described resource" — a current concept that is superseded is a contradiction, not a nuance —
//! so the two statements go together in both directions. A vocabulary that wants to record a
//! relationship to the concept it was nearly replaced by has `skos:related` and `skos:closeMatch`
//! and can say so with `openbiz import`; this does not decide that for it.
//!
//! # What stays, and this is the decision the item turns on
//!
//! **The `skos:changeNote` explaining the retirement stays.** Every one of them, untouched.
//!
//! Two reasons, and the first is on its own sufficient. Nothing links a change note to the marker.
//! [`deprecate`](crate::deprecate) writes an ordinary `skos:changeNote` with no statement joining
//! it to the `owl:deprecated` it was written beside, so identifying "the note that explained the
//! retirement" means matching on its text or on its position in a stream, which is a guess that
//! removes a curator's prose when it is wrong. The second reason is that even a note this could
//! identify should stay: SKOS §7 defines `skos:changeNote` as documenting a *modification*, and
//! the modification happened. A vocabulary whose history reads "retired, then reinstated" is
//! telling the truth; one whose history has been tidied until the retirement never appears is the
//! opaque change history `CLAUDE.md` names as a reason this product exists. So the note that
//! explained the retirement becomes the record of a retirement that was undone, `--note` adds the
//! sentence explaining why it was undone, and [`Reinstatement::kept_notes`] names the ones left in
//! place so the report can show the operator the history they now have.
//!
//! # It is defined by the statements, not by the model
//!
//! Every other operation here starts by asking [`CoreModel`] for a resource and refusing if it is
//! not a `skos:Concept`. This one does not, and the difference is what it is for: it removes
//! statements that exist, and whether they can be removed does not depend on what else the graph
//! says about their subject. A stray `owl:deprecated` imported about an IRI this vocabulary types
//! as nothing at all is exactly the case where a person needs the marker gone and the model has
//! never heard of the subject. The model is still read — for the labels the report needs, for the
//! notes that stay, and for the integrity check the caller runs — but it is not the gate.
//!
//! # What it refuses
//!
//! - **A resource this vocabulary says nothing about the status of.** There is nothing to remove,
//!   and a candidate that changes nothing wastes a reviewer's attention.
//! - **A scan that hit its bound**, because a scan that stopped collecting cannot promise it found
//!   every marker, and a reinstatement that leaves one behind reports success and changes nothing
//!   a reader would notice.
//! - **A change note with nothing in it**, as everywhere else.
//!
//! # What it will not read, and says so
//!
//! An `owl:deprecated` whose object is neither `"true"^^xsd:boolean` nor the plain `"true"` this
//! build is lenient about — `"false"`, an IRI, a language-tagged literal — is left exactly where
//! it is and reported in [`Reinstatement::unread`]. Nothing in this build reads it as a
//! retirement, so removing it would be acting on a meaning nobody here has established, and
//! silently leaving it would hide a status statement from the one command whose subject is status.

use std::collections::BTreeSet;
use std::fmt;

use crate::deprecate::{literal, says_true, StatusBound, DCTERMS_IS_REPLACED_BY, OWL_DEPRECATED};
use crate::labels::LexicalLabel;
use crate::model::{CoreModel, Node, Statement, Term};
use crate::notes::{NoteKind, SKOS_CHANGE_NOTE};

/// Every statement one vocabulary makes about the status of one resource.
///
/// Built by streaming the whole graph past [`ReinstatementScanBuilder::push`]. Unlike
/// [`DeprecationScan`](crate::DeprecationScan) it keeps the **statements themselves** rather than
/// counts and a set of successors, because what it produces is a list of removals and a removal
/// has to match the statement in the store exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstatementScan {
    resource: Node,
    markers: Vec<Statement>,
    unread: Vec<Statement>,
    replacements: Vec<Statement>,
    complete: bool,
}

impl ReinstatementScan {
    /// Start collecting what a vocabulary says about the status of `resource`.
    pub fn builder(resource: Node) -> ReinstatementScanBuilder {
        ReinstatementScanBuilder {
            scan: ReinstatementScan {
                resource,
                markers: Vec::new(),
                unread: Vec::new(),
                replacements: Vec::new(),
                complete: true,
            },
            bound: StatusBound::DEFAULT,
        }
    }

    /// The resource the scan is about.
    pub fn resource(&self) -> &Node {
        &self.resource
    }

    /// The `owl:deprecated` statements this build reads as retiring it.
    pub fn markers(&self) -> &[Statement] {
        &self.markers
    }

    /// The `owl:deprecated` statements it does not read as retiring anything.
    pub fn unread(&self) -> &[Statement] {
        &self.unread
    }

    /// The `dcterms:isReplacedBy` statements about it.
    pub fn replacements(&self) -> &[Statement] {
        &self.replacements
    }

    /// Whether every status statement about the resource was held.
    ///
    /// False once the bound was hit. [`CoreModel::reinstate`] refuses rather than proceeding: see
    /// the module documentation for why a partial answer is worse here than no answer.
    pub fn is_complete(&self) -> bool {
        self.complete
    }
}

/// Collects the status statements while the vocabulary streams past.
#[derive(Debug, Clone)]
pub struct ReinstatementScanBuilder {
    scan: ReinstatementScan,
    bound: StatusBound,
}

impl ReinstatementScanBuilder {
    /// Use a different bound. See [`StatusBound::DEFAULT`] for what the standing one is.
    ///
    /// The bound counts **every** status statement this holds about the resource, and not only the
    /// replacements its field is named for. The two scans hold the same kind of thing and the one
    /// constant governs both; this one holds statements where a deprecation holds a count, which
    /// is a reason to bound it and not a reason to bound it differently.
    pub fn with_bound(mut self, bound: StatusBound) -> Self {
        self.bound = bound;
        self
    }

    /// Offer one statement of the vocabulary.
    pub fn push(&mut self, statement: Statement) {
        if statement.subject != self.scan.resource {
            return;
        }
        let held = self.scan.markers.len() + self.scan.unread.len() + self.scan.replacements.len();
        if held >= self.bound.max_replacements {
            // Only once it would have kept something. A graph under the bound is complete even if
            // it streamed a million statements about other subjects past this.
            if statement.predicate == OWL_DEPRECATED
                || statement.predicate == DCTERMS_IS_REPLACED_BY
            {
                self.scan.complete = false;
            }
            return;
        }

        if statement.predicate == OWL_DEPRECATED {
            match says_true(&statement.object) {
                true => self.scan.markers.push(statement),
                false => self.scan.unread.push(statement),
            }
            return;
        }
        if statement.predicate == DCTERMS_IS_REPLACED_BY {
            self.scan.replacements.push(statement);
        }
    }

    /// The finished scan.
    pub fn build(self) -> ReinstatementScan {
        self.scan
    }
}

/// What putting one resource back would remove from a vocabulary, and what it would leave.
///
/// Produced by [`CoreModel::reinstate`] and applied by nobody: the statements are a proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reinstatement {
    resource: Node,
    removals: Vec<Statement>,
    additions: Vec<Statement>,
    note: Option<LexicalLabel>,
    was_marked: bool,
    replacements: Vec<Node>,
    unread: Vec<Statement>,
    kept_notes: Vec<Term>,
}

impl Reinstatement {
    /// The resource being put back.
    pub fn resource(&self) -> &Node {
        &self.resource
    }

    /// The statements it would remove: every marker, then every recorded replacement.
    pub fn removals(&self) -> &[Statement] {
        &self.removals
    }

    /// The statements it would add: the operator's change note, when they gave one.
    pub fn additions(&self) -> &[Statement] {
        &self.additions
    }

    /// The change note recording why it was put back, if one was given.
    pub fn note(&self) -> Option<&LexicalLabel> {
        self.note.as_ref()
    }

    /// Whether the vocabulary actually marked it `owl:deprecated`.
    ///
    /// False for the half-retirement: something recorded as replaced and never marked, which
    /// [`deprecate`](crate::deprecate) cannot produce and every browse command reads as current.
    /// It is still put right here, because the statement that has to come out is the same one.
    pub fn was_marked(&self) -> bool {
        self.was_marked
    }

    /// The successors it stops naming, in IRI order.
    ///
    /// The node objects of the `dcterms:isReplacedBy` statements being removed. A statement whose
    /// object is a literal is still removed and simply names no successor to list here.
    pub fn replacements(&self) -> &[Node] {
        &self.replacements
    }

    /// The `owl:deprecated` statements left in place because nothing here reads them.
    ///
    /// See the module documentation. Empty for every vocabulary this build wrote.
    pub fn unread(&self) -> &[Statement] {
        &self.unread
    }

    /// The change notes the resource keeps, in a stable order.
    ///
    /// Including the one that explained the retirement, which is the point: the modification
    /// happened, and `skos:changeNote` is the property that documents a modification.
    pub fn kept_notes(&self) -> &[Term] {
        &self.kept_notes
    }
}

/// Nothing could be put back, and this says exactly what stopped it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReinstatementError {
    /// The vocabulary says nothing about the resource's status.
    NotRetired {
        /// The IRI that was asked for.
        resource: Node,
        /// Whether the vocabulary says anything about the resource at all.
        known: bool,
    },
    /// A change note with nothing in it.
    EmptyNote,
    /// The scan hit its bound, so the statements to remove cannot all have been found.
    ScanTruncated {
        /// The resource.
        resource: Node,
    },
}

impl fmt::Display for ReinstatementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReinstatementError::NotRetired { resource, known } => {
                write!(
                    f,
                    "this vocabulary does not say {resource} is retired or superseded, \
                     so there is nothing to take back"
                )?;
                match known {
                    true => Ok(()),
                    false => write!(
                        f,
                        ". It says nothing about {resource} at all — check the IRI, and check \
                         which vocabulary retired it, because a retirement is per-vocabulary"
                    ),
                }
            }
            ReinstatementError::EmptyNote => {
                write!(f, "the change note given has nothing in it")
            }
            ReinstatementError::ScanTruncated { resource } => write!(
                f,
                "there are more statements about the status of {resource} than this can hold, \
                 so the ones to remove cannot all have been found — and one marker left behind \
                 would leave it retired while this reported that it is not"
            ),
        }
    }
}

impl std::error::Error for ReinstatementError {}

impl CoreModel {
    /// The statements that would put `resource` back, and the ones that would stay.
    ///
    /// `scan` carries what the raw graph says about its status, which this model cannot know:
    /// `owl:deprecated` and `dcterms:isReplacedBy` are not SKOS. `note` is the operator's own
    /// sentence about why it was put back, written as a `skos:changeNote`; `language` overrides
    /// the tag it is given.
    ///
    /// Nothing is written. The answer is a [`Reinstatement`] holding the removals, and the caller
    /// stages them for a person to approve.
    pub fn reinstate(
        &self,
        scan: &ReinstatementScan,
        note: Option<&str>,
        language: Option<&str>,
    ) -> Result<Reinstatement, ReinstatementError> {
        let resource = scan.resource();
        if !scan.is_complete() {
            return Err(ReinstatementError::ScanTruncated {
                resource: resource.clone(),
            });
        }

        if scan.markers().is_empty() && scan.replacements().is_empty() {
            return Err(ReinstatementError::NotRetired {
                resource: resource.clone(),
                known: self.resource(resource).is_some(),
            });
        }

        let described = self.resource(resource);
        let note = match note {
            Some(text) if text.trim().is_empty() => return Err(ReinstatementError::EmptyNote),
            Some(text) => Some(LexicalLabel {
                language: match described {
                    Some(described) => self.note_language(described, language),
                    // Nothing to take a language from, and this is the one operation whose subject
                    // need not be in the model at all. The caller's tag or untagged.
                    None => language
                        .map(str::trim)
                        .filter(|given| !given.is_empty())
                        .map(str::to_ascii_lowercase),
                },
                text: text.to_owned(),
            }),
            None => None,
        };

        let mut removals = scan.markers().to_vec();
        removals.extend_from_slice(scan.replacements());

        let additions = match &note {
            Some(note) => vec![Statement::new(
                resource.clone(),
                SKOS_CHANGE_NOTE.to_owned(),
                Term::Literal(literal(note)),
            )],
            None => Vec::new(),
        };

        let replacements: BTreeSet<Node> = scan
            .replacements()
            .iter()
            .filter_map(|statement| statement.object.as_node())
            .filter(|node| *node != resource)
            .cloned()
            .collect();

        let kept_notes = described
            .map(|described| described.notes_of(NoteKind::ChangeNote).cloned().collect())
            .unwrap_or_default();

        Ok(Reinstatement {
            resource: resource.clone(),
            removals,
            additions,
            note,
            was_marked: !scan.markers().is_empty(),
            replacements: replacements.into_iter().collect(),
            unread: scan.unread().to_vec(),
            kept_notes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deprecate::XSD_BOOLEAN;
    use crate::labels::{RDF_LANG_STRING, XSD_STRING};
    use crate::model::{Literal, SkosClass, RDF_TYPE};
    use crate::ns;

    fn ex(name: &str) -> Node {
        Node::iri(format!("http://example.org/{name}"))
    }

    fn concept(name: &str, label: &str) -> Vec<Statement> {
        vec![
            Statement::new(
                ex(name),
                RDF_TYPE.to_owned(),
                Node::iri(SkosClass::Concept.iri()),
            ),
            Statement::new(
                ex(name),
                format!("{}prefLabel", ns::SKOS),
                Term::Literal(Literal {
                    value: label.to_owned(),
                    language: Some("en".to_owned()),
                    datatype: RDF_LANG_STRING.to_owned(),
                }),
            ),
        ]
    }

    /// `owl:deprecated "true"^^xsd:boolean`, exactly as `CoreModel::deprecate` writes it.
    fn marked(name: &str) -> Statement {
        Statement::new(
            ex(name),
            OWL_DEPRECATED.to_owned(),
            Term::Literal(Literal {
                value: "true".to_owned(),
                language: None,
                datatype: XSD_BOOLEAN.to_owned(),
            }),
        )
    }

    fn replaced(name: &str, by: &str) -> Statement {
        Statement::new(ex(name), DCTERMS_IS_REPLACED_BY.to_owned(), ex(by))
    }

    fn change_note(name: &str, text: &str) -> Statement {
        Statement::new(
            ex(name),
            SKOS_CHANGE_NOTE.to_owned(),
            Term::Literal(Literal {
                value: text.to_owned(),
                language: Some("en".to_owned()),
                datatype: RDF_LANG_STRING.to_owned(),
            }),
        )
    }

    /// A model and a scan built from the same statements, which is what a caller does.
    fn read(statements: &[Statement], resource: Node) -> (CoreModel, ReinstatementScan) {
        let model = CoreModel::from_statements(statements.iter().cloned());
        let mut scan = ReinstatementScan::builder(resource);
        for statement in statements {
            scan.push(statement.clone());
        }
        (model, scan.build())
    }

    #[test]
    fn putting_a_concept_back_removes_the_marker_and_nothing_else() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(marked("wireless"));
        let (model, scan) = read(&statements, ex("wireless"));

        let back = model.reinstate(&scan, None, None).expect("a reinstatement");

        assert!(back.was_marked());
        assert_eq!(back.removals(), &[marked("wireless")]);
        assert_eq!(back.additions(), &[]);
        assert_eq!(back.replacements(), &[]);
    }

    /// The decision `docs/adr/0042` turns on: a current concept that records a successor is the
    /// half-retirement `openbiz inspect` reports, so removing only the marker would create one.
    #[test]
    fn the_replacement_comes_out_with_the_marker() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.extend(concept("radio", "Radio"));
        statements.push(marked("wireless"));
        statements.push(replaced("wireless", "radio"));
        let (model, scan) = read(&statements, ex("wireless"));

        let back = model.reinstate(&scan, None, None).expect("a reinstatement");

        assert_eq!(
            back.removals(),
            &[marked("wireless"), replaced("wireless", "radio")]
        );
        assert_eq!(back.replacements(), &[ex("radio")]);
        // The replacement is a live concept and nothing about it is touched, in either direction.
        assert!(!back
            .removals()
            .iter()
            .any(|statement| statement.subject == ex("radio")));
    }

    /// Round trip: what `CoreModel::deprecate` writes is exactly what this takes back out.
    #[test]
    fn it_removes_exactly_what_a_deprecation_added_except_the_note() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.extend(concept("radio", "Radio"));
        let model = CoreModel::from_statements(statements.iter().cloned());
        let mut scan = crate::DeprecationScan::builder(ex("wireless"), Some(ex("radio")));
        for statement in &statements {
            scan.push(statement.clone());
        }
        let deprecation = model
            .deprecate(&scan.build(), Some("superseded by Radio"), None)
            .expect("a retirement");
        statements.extend(deprecation.additions().iter().cloned());

        let (model, scan) = read(&statements, ex("wireless"));
        let back = model.reinstate(&scan, None, None).expect("a reinstatement");

        let taken: BTreeSet<&Statement> = back.removals().iter().collect();
        let added: BTreeSet<&Statement> = deprecation.additions().iter().collect();
        let note = Statement::new(
            ex("wireless"),
            SKOS_CHANGE_NOTE.to_owned(),
            Term::Literal(Literal {
                value: "superseded by Radio".to_owned(),
                language: Some("en".to_owned()),
                datatype: RDF_LANG_STRING.to_owned(),
            }),
        );
        assert!(
            added.contains(&note),
            "the retirement wrote its change note"
        );
        assert_eq!(
            taken,
            added.iter().copied().filter(|s| **s != note).collect(),
            "everything the retirement added comes out, except the note explaining it"
        );
        assert_eq!(back.kept_notes(), std::slice::from_ref(&note.object));
    }

    /// The second half of the same decision: the note stays *and* is reported, so a report can
    /// show the operator the history they are left with rather than leaving it invisible.
    #[test]
    fn every_change_note_stays_including_the_one_explaining_the_retirement() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(marked("wireless"));
        statements.push(change_note("wireless", "retired: superseded by Radio"));
        statements.push(change_note("wireless", "moved under Communications, 1998"));
        let (model, scan) = read(&statements, ex("wireless"));

        let back = model.reinstate(&scan, None, None).expect("a reinstatement");

        assert_eq!(back.removals(), &[marked("wireless")]);
        assert_eq!(back.kept_notes().len(), 2);
        assert!(!back
            .removals()
            .iter()
            .any(|statement| statement.predicate == SKOS_CHANGE_NOTE));
    }

    #[test]
    fn a_note_records_why_it_was_put_back_and_takes_the_concepts_language() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(marked("wireless"));
        let (model, scan) = read(&statements, ex("wireless"));

        let back = model
            .reinstate(
                &scan,
                Some("retired in error, still in use in the archive"),
                None,
            )
            .expect("a reinstatement");

        assert_eq!(
            back.additions(),
            &[change_note(
                "wireless",
                "retired in error, still in use in the archive"
            )]
        );
        assert_eq!(
            back.note().and_then(|note| note.language.clone()),
            Some("en".to_owned())
        );
    }

    /// Every marker, not the first: two tools can each have written one, and one left behind
    /// leaves the concept retired everywhere while the command reports that it is not.
    #[test]
    fn both_spellings_of_the_marker_come_out() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(marked("wireless"));
        statements.push(Statement::new(
            ex("wireless"),
            OWL_DEPRECATED.to_owned(),
            Term::Literal(Literal {
                value: "true".to_owned(),
                language: None,
                datatype: XSD_STRING.to_owned(),
            }),
        ));
        let (model, scan) = read(&statements, ex("wireless"));

        let back = model.reinstate(&scan, None, None).expect("a reinstatement");

        assert_eq!(back.removals().len(), 2);
        assert!(back.unread().is_empty());
    }

    /// A status statement this build does not read as a retirement is left alone and named,
    /// because removing it would act on a meaning nobody here has established.
    #[test]
    fn an_unreadable_status_statement_is_left_in_place_and_reported() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(marked("wireless"));
        let unreadable = Statement::new(
            ex("wireless"),
            OWL_DEPRECATED.to_owned(),
            Term::Literal(Literal {
                value: "false".to_owned(),
                language: None,
                datatype: XSD_BOOLEAN.to_owned(),
            }),
        );
        statements.push(unreadable.clone());
        let (model, scan) = read(&statements, ex("wireless"));

        let back = model.reinstate(&scan, None, None).expect("a reinstatement");

        assert_eq!(back.removals(), &[marked("wireless")]);
        assert_eq!(back.unread(), &[unreadable]);
    }

    /// The half-retirement `openbiz deprecate` cannot produce and every browse command reads as
    /// current. The statement that has to come out is the same one, so this puts it right.
    #[test]
    fn a_replacement_with_no_marker_is_still_taken_back() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.extend(concept("radio", "Radio"));
        statements.push(replaced("wireless", "radio"));
        let (model, scan) = read(&statements, ex("wireless"));

        let back = model.reinstate(&scan, None, None).expect("a reinstatement");

        assert!(!back.was_marked());
        assert_eq!(back.removals(), &[replaced("wireless", "radio")]);
    }

    /// It is defined by the statements and not by the model: a stray marker about an IRI this
    /// vocabulary types as nothing at all is exactly where a person needs it gone.
    #[test]
    fn a_marker_about_something_the_model_never_heard_of_still_comes_out() {
        let statements = vec![marked("stray")];
        let (model, scan) = read(&statements, ex("stray"));

        let back = model
            .reinstate(&scan, Some("imported by mistake"), None)
            .expect("a reinstatement");

        assert_eq!(back.removals(), &[marked("stray")]);
        assert!(back.kept_notes().is_empty());
        // Nothing to take a language from, and nothing was guessed.
        assert_eq!(back.note().and_then(|note| note.language.clone()), None);
    }

    #[test]
    fn a_concept_nothing_says_is_retired_is_refused() {
        let statements = concept("wireless", "Wireless telegraphy");
        let (model, scan) = read(&statements, ex("wireless"));

        assert_eq!(
            model.reinstate(&scan, None, None),
            Err(ReinstatementError::NotRetired {
                resource: ex("wireless"),
                known: true,
            })
        );
    }

    /// The same refusal says something different when the vocabulary has never heard of the IRI,
    /// because a retirement is per-vocabulary and the likeliest mistake is the wrong graph.
    #[test]
    fn an_iri_this_vocabulary_does_not_hold_is_refused_and_told_so() {
        let statements = concept("wireless", "Wireless telegraphy");
        let (model, scan) = read(&statements, ex("radio"));

        let refusal = model.reinstate(&scan, None, None).expect_err("a refusal");

        assert_eq!(
            refusal,
            ReinstatementError::NotRetired {
                resource: ex("radio"),
                known: false,
            }
        );
        assert!(refusal.to_string().contains("says nothing about"));
    }

    #[test]
    fn an_empty_note_is_refused_rather_than_written() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(marked("wireless"));
        let (model, scan) = read(&statements, ex("wireless"));

        assert_eq!(
            model.reinstate(&scan, Some("   "), None),
            Err(ReinstatementError::EmptyNote)
        );
    }

    /// A truncated scan cannot promise it found every marker, and one left behind means the
    /// concept is still retired while this reports that it is not.
    #[test]
    fn a_scan_that_hit_its_bound_is_refused_rather_than_half_applied() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(marked("wireless"));
        statements.push(replaced("wireless", "radio"));
        statements.push(replaced("wireless", "broadcasting"));
        let model = CoreModel::from_statements(statements.iter().cloned());
        let mut scan = ReinstatementScan::builder(ex("wireless")).with_bound(StatusBound {
            max_replacements: 2,
        });
        for statement in &statements {
            scan.push(statement.clone());
        }
        let scan = scan.build();

        assert!(!scan.is_complete());
        assert_eq!(
            model.reinstate(&scan, None, None),
            Err(ReinstatementError::ScanTruncated {
                resource: ex("wireless"),
            })
        );
    }

    /// The bound counts what is held about the resource, not what streamed past it.
    #[test]
    fn statements_about_everything_else_do_not_count_against_the_bound() {
        let mut statements = concept("wireless", "Wireless telegraphy");
        statements.push(marked("wireless"));
        for index in 0..50 {
            statements.extend(concept(&format!("other{index}"), "Other"));
            statements.push(marked(&format!("other{index}")));
        }
        let mut scan = ReinstatementScan::builder(ex("wireless")).with_bound(StatusBound {
            max_replacements: 2,
        });
        for statement in &statements {
            scan.push(statement.clone());
        }

        assert!(scan.build().is_complete());
    }
}
