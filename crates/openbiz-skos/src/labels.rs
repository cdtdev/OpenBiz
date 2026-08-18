//! SKOS lexical labels — `skos:prefLabel`, `skos:altLabel`, and `skos:hiddenLabel`.
//!
//! A label is the thing a taxonomist actually types, and it is the only part of a vocabulary a
//! subject-matter expert sees. It is also where SKOS puts two of its integrity conditions, so it
//! is the first place a real thesaurus is found to be inconsistent.
//!
//! # What the specification says, and where
//!
//! SKOS Reference §5.3 gives three class-and-property definitions (S10, S11, and
//! [`S12`](crate::SkosRule::S12)) and §5.4 gives **exactly two integrity conditions**
//! ([`S13`](crate::SkosRule::S13), [`S14`](crate::SkosRule::S14)). That heading is load-bearing:
//! it is the specification, not us, drawing the line this module reports along. Appendix B.3.4.2
//! restates the same two and calls them "the two integrity conditions … defined on the basic SKOS
//! labeling properties", which is the confirmation that there is no third one hiding elsewhere.
//!
//! - **S13** — the three properties are *pairwise disjoint*, so one resource may not carry the
//!   same label under two of them (Examples 13, 14, 15).
//! - **S14** — a resource has at most one `skos:prefLabel` **per language tag** (Example 12).
//!   Per *tag*, not per language: `"color"@en`, `"color"@en-US` and `"colour"@en-GB` on one
//!   resource is consistent, and §5.6.5 with Example 18 says so outright.
//!
//! S12 — the range is "the class of RDF plain literals" — is **not** an integrity condition, and
//! §5.6.2 is explicit about the consequence: "If a graph does not follow this usage convention an
//! application may reject such data but is not required to." So a label that is not a plain
//! literal is reported as [`Severity::IllFormed`](crate::Severity) and the vocabulary is still a
//! SKOS vocabulary. Refusing it would be the exact failure `docs/COMPETITIVE.md` records against
//! the incumbents: valid enterprise data turned away by a tool being stricter than the standard.
//!
//! # "RDF plain literal" in an RDF 1.1 world
//!
//! SKOS was published against RDF 1.0, which had a *plain literal* — a lexical form with an
//! optional language tag. **RDF 1.1 abolished the term**; the string "plain literal" does not
//! appear in RDF 1.1 Concepts at all. §3.3 of that document defines the two things it was split
//! into: a *language-tagged string*, whose datatype is always `rdf:langString`, and a *simple
//! literal*, which is "syntactic sugar for abstract syntax literals with the datatype IRI
//! `xsd:string`".
//!
//! So the RDF 1.0 class of plain literals is, in RDF 1.1, exactly those two — and that is what
//! [`LexicalLabel::of`] accepts. Nothing else is a label: `"4"^^xsd:integer` is not, an IRI is
//! not, and neither participates in S13 or S14, because a term with no language tag *and* no
//! claim to be a string cannot be placed in the per-language buckets those conditions are about.
//! See `docs/adr/0020`.
//!
//! # Language tags are compared lower-cased
//!
//! RDF 1.1 Concepts §3.3: "The value space of language tags is always in lower case." Two labels
//! tagged `@EN` and `@en` are therefore the same label and clash under S13, and two preferred
//! labels so tagged violate S14. The store's engine happens to normalise on the way in, but this
//! crate does not depend on any engine (`docs/adr/0019`) and so must not depend on that either —
//! it lower-cases here, and a test pins it.
//!
//! Tags are ASCII by [BCP47], so `to_ascii_lowercase` is the correct operation and not merely the
//! cheap one: `to_lowercase` is locale-shaped in ways that would mangle a tag containing `I`.
//!
//! [BCP47]: https://www.rfc-editor.org/info/bcp47

use std::fmt;

use crate::model::{Literal, Term};
use crate::ns;

/// `skos:prefLabel`.
pub const SKOS_PREF_LABEL: &str = "http://www.w3.org/2004/02/skos/core#prefLabel";
/// `skos:altLabel`.
pub const SKOS_ALT_LABEL: &str = "http://www.w3.org/2004/02/skos/core#altLabel";
/// `skos:hiddenLabel`.
pub const SKOS_HIDDEN_LABEL: &str = "http://www.w3.org/2004/02/skos/core#hiddenLabel";

/// `rdf:langString` — the datatype of every language-tagged string in RDF 1.1.
pub const RDF_LANG_STRING: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#langString";
/// `xsd:string` — the datatype an RDF 1.1 simple literal carries.
pub const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// Which of the three SKOS labelling properties a label was given under.
///
/// The order of the variants is the order a report shows them in, which is the order a person
/// asks for them: what it is called, what else it is called, what will find it in a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LabelKind {
    /// `skos:prefLabel` — at most one per language tag, per resource. S14.
    Preferred,
    /// `skos:altLabel` — synonyms and variants. Any number.
    Alternative,
    /// `skos:hiddenLabel` — matched by search, never displayed. Any number.
    Hidden,
}

impl LabelKind {
    /// Every kind, in the order a report shows them.
    pub const ALL: [LabelKind; 3] = [
        LabelKind::Preferred,
        LabelKind::Alternative,
        LabelKind::Hidden,
    ];

    /// The SKOS property IRI for this kind.
    pub fn property_iri(self) -> String {
        format!("{}{}", ns::SKOS, self.local_name())
    }

    /// The local name within the SKOS namespace.
    pub fn local_name(self) -> &'static str {
        match self {
            LabelKind::Preferred => "prefLabel",
            LabelKind::Alternative => "altLabel",
            LabelKind::Hidden => "hiddenLabel",
        }
    }

    /// The kind a predicate IRI names, or `None` if it is not a SKOS labelling property.
    pub fn from_iri(iri: &str) -> Option<Self> {
        match iri {
            SKOS_PREF_LABEL => Some(LabelKind::Preferred),
            SKOS_ALT_LABEL => Some(LabelKind::Alternative),
            SKOS_HIDDEN_LABEL => Some(LabelKind::Hidden),
            _ => None,
        }
    }

    /// The SKOS-XL property IRI for this kind — `skosxl:prefLabel` and its two siblings. B.3.
    ///
    /// The local names are the same as the plain properties', which is what B.3.1 means by
    /// "analogous to the properties of the same local name". Only the namespace differs.
    pub fn xl_property_iri(self) -> String {
        format!("{}{}", ns::SKOSXL, self.local_name())
    }

    /// The kind an SKOS-XL predicate IRI names, or `None` if it is not one of the three.
    pub fn from_xl_iri(iri: &str) -> Option<Self> {
        match iri {
            crate::xl::SKOSXL_PREF_LABEL => Some(LabelKind::Preferred),
            crate::xl::SKOSXL_ALT_LABEL => Some(LabelKind::Alternative),
            crate::xl::SKOSXL_HIDDEN_LABEL => Some(LabelKind::Hidden),
            _ => None,
        }
    }

    /// The sub-property chain axiom that dumbs an XL label of this kind down to a plain one.
    ///
    /// S55 for preferred, S56 for alternative, S57 for hidden — the three are stated separately
    /// in B.3.2 and each names one property, so the rule is a property of the kind rather than a
    /// single citation covering all three.
    pub fn dumbing_down_rule(self) -> crate::SkosRule {
        match self {
            LabelKind::Preferred => crate::SkosRule::S55,
            LabelKind::Alternative => crate::SkosRule::S56,
            LabelKind::Hidden => crate::SkosRule::S57,
        }
    }

    /// Whether SKOS permits at most one of these per language tag on one resource.
    ///
    /// Integrity condition [`S14`](crate::SkosRule::S14). Only the preferred label is limited;
    /// §5.6.4 is explicit that a resource may have alternatives and no preferred label at all.
    pub fn is_unique_per_language(self) -> bool {
        matches!(self, LabelKind::Preferred)
    }
}

impl fmt::Display for LabelKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "skos:{}", self.local_name())
    }
}

/// A label: a lexical form and the language tag it is in, if it has one.
///
/// Constructed only through [`LexicalLabel::of`], so a value of this type is by construction
/// something SKOS accepts as a label. The language tag is held lower-cased, which is the value
/// space RDF 1.1 Concepts §3.3 defines, so two labels are equal exactly when the RDF literals
/// behind them are the same term.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LexicalLabel {
    /// The BCP 47 language tag, lower-cased. `None` for a simple literal.
    ///
    /// Ordered before the text so that iterating a resource's labels groups them by language,
    /// which is how both the report and a translator want to read them.
    pub language: Option<String>,
    /// The lexical form, exactly as it was written.
    pub text: String,
}

impl LexicalLabel {
    /// The label a term carries, or `None` if the term is not something SKOS accepts as one.
    ///
    /// `None` is not an error here — it is the input to [`S12`](crate::SkosRule::S12)'s finding,
    /// and the caller raises it. See the module documentation for what RDF 1.1 makes of a
    /// "plain literal".
    pub fn of(term: &Term) -> Option<Self> {
        let Term::Literal(literal) = term else {
            // An IRI or a blank node. Not a literal at all, so not a plain one.
            return None;
        };
        Self::of_literal(literal)
    }

    /// The label a literal carries, or `None` if it is not a plain literal.
    pub fn of_literal(literal: &Literal) -> Option<Self> {
        match &literal.language {
            // A language-tagged string. RDF 1.1 requires the tag to be non-empty and the datatype
            // to be `rdf:langString`; anything else is a literal we do not recognise rather than a
            // label we should silently repair.
            Some(tag) if !tag.is_empty() && literal.datatype == RDF_LANG_STRING => {
                Some(LexicalLabel {
                    language: Some(tag.to_ascii_lowercase()),
                    text: literal.value.clone(),
                })
            }
            // A simple literal, which RDF 1.1 §3.3 gives the datatype `xsd:string`.
            None if literal.datatype == XSD_STRING => Some(LexicalLabel {
                language: None,
                text: literal.value.clone(),
            }),
            _ => None,
        }
    }

    /// Whether the label is in `tag`, compared as RDF 1.1 compares language tags.
    pub fn is_in(&self, tag: &str) -> bool {
        self.language.as_deref() == Some(&tag.to_ascii_lowercase())
    }
}

impl fmt::Display for LexicalLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.text)?;
        match &self.language {
            Some(language) => write!(f, "@{language}"),
            None => Ok(()),
        }
    }
}

/// How many labels of each kind a graph carries in one language, and how many resources have one.
///
/// This is the answer to the question a multilingual programme actually asks — *how far behind is
/// the French?* — and it is a count, not a list, so it stays the same size whether the vocabulary
/// has ten concepts or a million.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageCoverage {
    /// The language tag, lower-cased, or `None` for labels with no tag.
    pub language: Option<String>,
    /// How many (resource, label) pairs this language has under `skos:prefLabel`.
    ///
    /// A pair, not a distinct string: two concepts both preferring `"Bank"@en` count twice,
    /// because the question this answers is how much of the vocabulary is labelled in this
    /// language, not how large its lexicon is.
    pub preferred: usize,
    /// The same, under `skos:altLabel`.
    pub alternative: usize,
    /// The same, under `skos:hiddenLabel`.
    pub hidden: usize,
    /// How many resources have at least one preferred label in this language.
    pub resources_with_preferred: usize,
}

impl LanguageCoverage {
    /// How many labels of `kind` this language has.
    pub fn count_of(&self, kind: LabelKind) -> usize {
        match kind {
            LabelKind::Preferred => self.preferred,
            LabelKind::Alternative => self.alternative,
            LabelKind::Hidden => self.hidden,
        }
    }

    /// Every label in this language, of any kind.
    pub fn total(&self) -> usize {
        self.preferred + self.alternative + self.hidden
    }
}

impl fmt::Display for LanguageCoverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.language {
            Some(language) => write!(f, "@{language}")?,
            None => write!(f, "(no language tag)")?,
        }
        write!(
            f,
            "  {} preferred on {} resource(s), {} alternative, {} hidden",
            self.preferred, self.resources_with_preferred, self.alternative, self.hidden
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn literal(value: &str, language: Option<&str>, datatype: &str) -> Literal {
        Literal {
            value: value.to_owned(),
            language: language.map(str::to_owned),
            datatype: datatype.to_owned(),
        }
    }

    #[test]
    fn label_kinds_map_to_the_skos_properties_both_ways() {
        for kind in LabelKind::ALL {
            assert_eq!(LabelKind::from_iri(&kind.property_iri()), Some(kind));
        }
        assert_eq!(
            LabelKind::Preferred.property_iri(),
            "http://www.w3.org/2004/02/skos/core#prefLabel"
        );
        assert_eq!(
            LabelKind::from_iri("http://www.w3.org/2000/01/rdf-schema#label"),
            None
        );
    }

    #[test]
    fn only_preferred_labels_are_unique_per_language() {
        // SKOS integrity condition S14, and §5.6.4 for the other two.
        assert!(LabelKind::Preferred.is_unique_per_language());
        assert!(!LabelKind::Alternative.is_unique_per_language());
        assert!(!LabelKind::Hidden.is_unique_per_language());
    }

    /// The RDF 1.1 reading of "plain literal": a language-tagged string, or an `xsd:string`.
    #[test]
    fn a_language_tagged_string_and_a_simple_literal_are_both_labels() {
        assert_eq!(
            LexicalLabel::of_literal(&literal("love", Some("en"), RDF_LANG_STRING)),
            Some(LexicalLabel {
                language: Some("en".to_owned()),
                text: "love".to_owned()
            })
        );
        assert_eq!(
            LexicalLabel::of_literal(&literal("love", None, XSD_STRING)),
            Some(LexicalLabel {
                language: None,
                text: "love".to_owned()
            })
        );
    }

    #[test]
    fn a_typed_literal_that_is_not_a_string_is_not_a_label() {
        assert_eq!(
            LexicalLabel::of_literal(&literal(
                "4",
                None,
                "http://www.w3.org/2001/XMLSchema#integer"
            )),
            None
        );
    }

    /// Not a curiosity: an IRI here is the mistake behind every "the label shows as a URL" bug.
    #[test]
    fn an_iri_is_not_a_label() {
        use crate::model::Node;
        assert_eq!(
            LexicalLabel::of(&Term::Node(Node::iri("http://example.org/love"))),
            None
        );
    }

    /// RDF 1.1 Concepts §3.3 — "The value space of language tags is always in lower case."
    #[test]
    fn a_language_tag_is_held_lower_cased_so_two_spellings_are_one_label() {
        let upper = LexicalLabel::of_literal(&literal("love", Some("EN-gb"), RDF_LANG_STRING));
        let lower = LexicalLabel::of_literal(&literal("love", Some("en-GB"), RDF_LANG_STRING));
        assert_eq!(upper, lower);
        assert_eq!(upper.expect("a label").language.as_deref(), Some("en-gb"));
    }

    #[test]
    fn a_language_tag_is_matched_case_insensitively() {
        let label = LexicalLabel::of_literal(&literal("love", Some("en"), RDF_LANG_STRING))
            .expect("a label");
        assert!(label.is_in("EN"));
        assert!(label.is_in("en"));
        assert!(
            !label.is_in("en-GB"),
            "en and en-GB are different tags — §5.6.5"
        );
    }

    /// A language tag and an `rdf:langString` datatype come as a pair or not at all.
    #[test]
    fn a_mismatched_language_and_datatype_is_not_a_label() {
        assert_eq!(
            LexicalLabel::of_literal(&literal("love", Some("en"), XSD_STRING)),
            None
        );
        assert_eq!(
            LexicalLabel::of_literal(&literal("love", None, RDF_LANG_STRING)),
            None
        );
        assert_eq!(
            LexicalLabel::of_literal(&literal("love", Some(""), RDF_LANG_STRING)),
            None,
            "RDF 1.1 requires a non-empty tag with rdf:langString"
        );
    }

    #[test]
    fn labels_are_ordered_by_language_then_text() {
        let mut labels = [
            LexicalLabel {
                language: Some("fr".to_owned()),
                text: "amour".to_owned(),
            },
            LexicalLabel {
                language: Some("en".to_owned()),
                text: "love".to_owned(),
            },
            LexicalLabel {
                language: Some("en".to_owned()),
                text: "adoration".to_owned(),
            },
            LexicalLabel {
                language: None,
                text: "untagged".to_owned(),
            },
        ];
        labels.sort();
        let rendered: Vec<String> = labels.iter().map(ToString::to_string).collect();
        assert_eq!(
            rendered,
            vec![
                "\"untagged\"",
                "\"adoration\"@en",
                "\"love\"@en",
                "\"amour\"@fr"
            ]
        );
    }

    #[test]
    fn coverage_totals_every_kind() {
        let coverage = LanguageCoverage {
            language: Some("en".to_owned()),
            preferred: 3,
            alternative: 2,
            hidden: 1,
            resources_with_preferred: 3,
        };
        assert_eq!(coverage.total(), 6);
        assert_eq!(coverage.count_of(LabelKind::Preferred), 3);
        assert_eq!(coverage.count_of(LabelKind::Hidden), 1);
        assert!(coverage.to_string().contains("@en"));
    }
}
