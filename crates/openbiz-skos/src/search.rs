//! Finding a concept by what it is called — matching over the lexical labels of §5.
//!
//! This is the first question a subject-matter expert asks a vocabulary, and it is asked in the
//! only terms they have: a word they think the concept is called. Everything else this crate does
//! starts from an IRI the asker already knows.
//!
//! # The specification designed a label for this
//!
//! SKOS Reference §5.1 is unusually direct about why the third labelling property exists:
//!
//! > The hidden labels are useful when a user is interacting with a knowledge organization system
//! > via a text-based search function. The user may, for example, enter mis-spelled words when
//! > trying to find a relevant concept. If the mis-spelled query can be matched against a hidden
//! > label, the user will be able to find the relevant concept, but the hidden label won't
//! > otherwise be visible to the user (so further mistakes aren't encouraged).
//!
//! So `skos:hiddenLabel` is searched **by default**, together with the preferred and the
//! alternative labels — a search that skipped it would defeat the one property the specification
//! defines in terms of search. The second half of that sentence is a *display* rule and it binds
//! elsewhere: [`Resource::display_label`](crate::Resource::display_label) never chooses a hidden
//! label, so a hit on one is shown under the concept's preferred label. What a report may say is
//! *which* label matched — see `docs/adr/0034` for why an authoring tool answers that question
//! where a public search front-end would not.
//!
//! # Case, and the accent that is still a difference
//!
//! §5.1 also says a lexical label is "a string of UNICODE characters", so matching is done over
//! Unicode and not over bytes. Both sides go through [`fold`](crate::fold), which is the Unicode
//! Standard's §3.13 **canonical caseless** form — case folding and canonical normalisation, not
//! the lowercasing this used to do. That module documents the definition; what matters here is the
//! two misses it removes, both of which produced "no results" for a concept the vocabulary held:
//!
//! - `"Straße"` lowercases to itself, so a search for `STRASSE` found nothing. It folds to
//!   `strasse`, and now it does. Likewise a Greek word-final `ς` against a medial `σ`.
//! - `"é"` written as one code point (U+00E9) and as two (`e` + U+0301) render identically and
//!   are different strings. They now normalise together. Both forms occur in real multilingual
//!   thesauri.
//!
//! **An accent is still a difference.** `ecole` does not find `École`, and that is deliberate
//! rather than pending: stripping diacritics is a language-specific editorial guess, not a Unicode
//! operation, and it manufactures matches between terms that are not the same word. So is
//! stemming, and so is spelling correction — `skos:hiddenLabel` is the specification's own answer
//! to the last of those. Each of these non-matches is pinned by a test, because the failure mode
//! of an unrecorded gap here is a search that quietly reports "no results" for a concept that
//! exists. `docs/UNTESTED.md` carries what remains.
//!
//! # No match offset is reported, and that is deliberate
//!
//! Folding is not length-preserving — `ß` folds to two characters, `İ` to two — so an offset into
//! the folded form is not an offset into the label the author wrote. A caller that highlighted the
//! matched characters using such an offset would highlight the wrong ones on exactly the labels
//! that most need care. [`MatchQuality`] therefore says *how* a label matched, which is what
//! ranking and explanation need, and no index is exposed.
//!
//! # Language filtering is RFC 4647 basic filtering
//!
//! A user asking for English wants `en`, `en-GB` and `en-US`, and does not want `enm` (Middle
//! English). That is exactly [RFC 4647] §3.3.1 *basic filtering*:
//!
//! > A language range matches a particular language tag if, in a case-insensitive comparison, it
//! > exactly equals the tag, or if it exactly equals a prefix of the tag such that the first
//! > character following the prefix is "-".
//!
//! and its wildcard: "The special range `*` in a language priority list matches any tag."
//!
//! Read that wildcard exactly: it matches any *tag*. A label with no language tag at all — an RDF
//! 1.1 simple literal, which [`LexicalLabel`] carries as `language: None` — has no tag to match,
//! so `*` does not select it. That is why [`LanguageFilter`] has three cases and not two:
//! [`Any`](LanguageFilter::Any) is *no filter*, `Range("*")` is every tagged label, and
//! [`Untagged`](LanguageFilter::Untagged) is the labels a multilingual programme is usually
//! hunting for when it audits its own data.
//!
//! **Extended filtering (§3.3.2) is not implemented.** The consequence is worth stating rather
//! than burying: under basic filtering the range `de-DE` does *not* match the tag `de-Latn-DE`,
//! because the character after the prefix `de` is `-` but the range is longer than that. A user
//! who narrows to `de-DE` therefore silently loses script-tagged labels. `docs/UNTESTED.md`
//! carries it.
//!
//! [RFC 4647]: https://www.rfc-editor.org/rfc/rfc4647

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use thiserror::Error;

use crate::fold::fold;
use crate::labels::{LabelKind, LexicalLabel};
use crate::model::{CoreModel, Node};
use crate::xl::LabelOrigin;

/// How a query text is compared with a label.
///
/// The three are ordered as a report ranks them, best first, and that ordering is load-bearing:
/// [`LabelSearch`] sorts by it, so an exact hit is never pushed off the end of a bounded result
/// list by an incidental substring match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchMode {
    /// The whole label, and nothing less.
    Exact,
    /// The label begins with the query. What a type-ahead wants.
    Prefix,
    /// The query occurs anywhere in the label. The forgiving default.
    Infix,
}

impl fmt::Display for MatchMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchMode::Exact => write!(f, "exact"),
            MatchMode::Prefix => write!(f, "prefix"),
            MatchMode::Infix => write!(f, "infix"),
        }
    }
}

/// How well a label matched — the same three cases, seen from the label's end.
///
/// A hit found under [`MatchMode::Infix`] still records that it happened to be exact, which is how
/// ranking puts "Bank" above "Investment banking" for the query `bank` without the user having to
/// ask twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchQuality {
    /// The label is the query.
    Exact,
    /// The label begins with the query but is longer.
    Prefix,
    /// The query is inside the label, not at its start.
    Infix,
}

impl fmt::Display for MatchQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchQuality::Exact => write!(f, "exact match"),
            MatchQuality::Prefix => write!(f, "matches at the start"),
            MatchQuality::Infix => write!(f, "matches inside the label"),
        }
    }
}

/// Which labels a query will look at, by language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageFilter {
    /// Every label, tagged or not. The default: a filter nobody asked for.
    Any,
    /// Only labels whose tag the range selects under RFC 4647 basic filtering.
    ///
    /// Never selects an untagged label, including for the wildcard range — see the module
    /// documentation.
    Range(LanguageRange),
    /// Only labels with no language tag at all.
    Untagged,
}

impl fmt::Display for LanguageFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LanguageFilter::Any => write!(f, "any language"),
            LanguageFilter::Range(range) => write!(f, "language range {range}"),
            LanguageFilter::Untagged => write!(f, "labels with no language tag"),
        }
    }
}

/// A basic language range, as RFC 4647 §2.1 defines one.
///
/// > `language-range = (1*8ALPHA *("-" 1*8alphanum)) / "*"`
///
/// Validated on construction and refused if malformed, rather than kept and matched against
/// nothing: a range with a typo in it that quietly selects no labels is indistinguishable, in the
/// report, from a vocabulary that has none in that language.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LanguageRange(String);

impl LanguageRange {
    /// Read a range, lower-cased, or refuse it.
    pub fn parse(range: &str) -> Result<Self, QueryError> {
        if range == "*" {
            return Ok(LanguageRange("*".to_owned()));
        }
        let malformed = || QueryError::MalformedLanguageRange {
            range: range.to_owned(),
        };
        let mut parts = range.split('-');
        let primary = parts.next().ok_or_else(malformed)?;
        if primary.is_empty()
            || primary.len() > 8
            || !primary.chars().all(|c| c.is_ascii_alphabetic())
        {
            return Err(malformed());
        }
        for part in parts {
            if part.is_empty() || part.len() > 8 || !part.chars().all(|c| c.is_ascii_alphanumeric())
            {
                return Err(malformed());
            }
        }
        // ASCII by BCP 47, so `to_ascii_lowercase` and not `to_lowercase`, for the same reason
        // `labels.rs` gives: the Unicode mapping is locale-shaped in ways that mangle an `I`.
        Ok(LanguageRange(range.to_ascii_lowercase()))
    }

    /// Whether this range selects `tag`, under RFC 4647 §3.3.1 basic filtering.
    ///
    /// `tag` is expected lower-cased, which is how [`LexicalLabel`] keeps one.
    pub fn selects(&self, tag: &str) -> bool {
        if self.0 == "*" {
            return true;
        }
        let tag = tag.to_ascii_lowercase();
        // "it exactly equals the tag, or … a prefix of the tag such that the first character
        // following the prefix is '-'". The second clause is what makes `en` select `en-GB` and
        // not `enm`.
        tag == self.0
            || (tag.starts_with(&self.0) && tag.as_bytes().get(self.0.len()) == Some(&b'-'))
    }

    /// The range as written, lower-cased.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LanguageRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// How many hits a search will hold and report.
///
/// A search over labels differs from every other bounded walk in this crate: the *cost* is the
/// size of the vocabulary, but the *answer* can be too. A one-letter infix query against a real
/// thesaurus matches essentially every label it has, and a report is not the place to put a
/// million of them.
///
/// The bound is enforced during the scan and not after it, so the memory a search holds is
/// bounded by the ceiling rather than by the number of matches. Because the ordering is total,
/// discarding the tail of an over-full buffer mid-scan gives exactly the list that sorting every
/// match and truncating would — see [`LabelSearch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchBound {
    /// The most hits to report. Matches beyond it are counted, not kept.
    pub max_hits: usize,
}

impl SearchBound {
    /// 200 hits.
    ///
    /// Chosen for a person reading a report, not measured against a corpus: each hit is several
    /// lines once its kind and its derivation are printed, so 200 is already a long screenful, and
    /// a query that matches more than that is one the user should narrow rather than one the tool
    /// should dump. `docs/UNTESTED.md` records that this is reasoning and not measurement.
    pub const DEFAULT: SearchBound = SearchBound { max_hits: 200 };
}

/// The query could not be read.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum QueryError {
    /// The text to look for is empty.
    ///
    /// Refused rather than answered: an empty infix query matches every label in the vocabulary,
    /// and a list of everything presented as *search results* tells the reader their query
    /// succeeded when what happened is that it said nothing.
    #[error("a search needs something to look for; the query is empty")]
    EmptyQuery,
    /// Every label kind was filtered out, so nothing could ever match.
    #[error("a search over no label kinds can never match; ask for at least one")]
    NoKinds,
    /// The language range is not one RFC 4647 §2.1 admits.
    #[error(
        "{range:?} is not a language range: RFC 4647 §2.1 allows 1-8 letters, then \
         hyphen-separated groups of 1-8 letters or digits, or the wildcard \"*\""
    )]
    MalformedLanguageRange {
        /// What was given.
        range: String,
    },
}

/// What to look for, where, and how much of it to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelQuery {
    text: String,
    folded: String,
    mode: MatchMode,
    language: LanguageFilter,
    kinds: BTreeSet<LabelKind>,
    bound: SearchBound,
}

impl LabelQuery {
    /// A query for `text`, matched anywhere in a label, in any language, over all three kinds.
    ///
    /// The defaults are the forgiving ones on purpose: this is the first thing a subject-matter
    /// expert types, and a search that finds too much is recoverable where one that silently finds
    /// nothing sends them off to create a duplicate concept — which is the failure `CLAUDE.md`
    /// §1.7 exists to prevent.
    pub fn new(text: &str) -> Result<Self, QueryError> {
        if text.is_empty() {
            return Err(QueryError::EmptyQuery);
        }
        Ok(LabelQuery {
            text: text.to_owned(),
            folded: fold(text),
            mode: MatchMode::Infix,
            language: LanguageFilter::Any,
            kinds: LabelKind::ALL.into_iter().collect(),
            bound: SearchBound::DEFAULT,
        })
    }

    /// Match this way instead of anywhere in the label.
    pub fn with_mode(mut self, mode: MatchMode) -> Self {
        self.mode = mode;
        self
    }

    /// Look only at labels this filter selects.
    pub fn with_language(mut self, language: LanguageFilter) -> Self {
        self.language = language;
        self
    }

    /// Look only at labels given under these kinds.
    pub fn with_kinds(
        mut self,
        kinds: impl IntoIterator<Item = LabelKind>,
    ) -> Result<Self, QueryError> {
        let kinds: BTreeSet<LabelKind> = kinds.into_iter().collect();
        if kinds.is_empty() {
            return Err(QueryError::NoKinds);
        }
        self.kinds = kinds;
        Ok(self)
    }

    /// Report at most this many hits.
    pub fn with_bound(mut self, bound: SearchBound) -> Self {
        self.bound = bound;
        self
    }

    /// The text as the user typed it.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// How the text is compared with a label.
    pub fn mode(&self) -> MatchMode {
        self.mode
    }

    /// Which labels are looked at, by language.
    pub fn language(&self) -> &LanguageFilter {
        &self.language
    }

    /// Which kinds are looked at.
    pub fn kinds(&self) -> &BTreeSet<LabelKind> {
        &self.kinds
    }

    /// How many hits will be reported.
    pub fn bound(&self) -> SearchBound {
        self.bound
    }

    /// Whether this query selects the label's language.
    pub fn selects_language(&self, label: &LexicalLabel) -> bool {
        match (&self.language, &label.language) {
            (LanguageFilter::Any, _) => true,
            (LanguageFilter::Range(range), Some(tag)) => range.selects(tag),
            // No tag, so nothing for a range to match — including the wildcard, which RFC 4647
            // defines as matching any *tag*.
            (LanguageFilter::Range(_), None) => false,
            (LanguageFilter::Untagged, tag) => tag.is_none(),
        }
    }

    /// How this query matches the label's text, or `None` if it does not.
    ///
    /// The language and the kinds are not consulted here; this is the text comparison alone.
    pub fn matches_text(&self, label: &LexicalLabel) -> Option<MatchQuality> {
        let folded = fold(&label.text);
        let quality = if folded == self.folded {
            MatchQuality::Exact
        } else if folded.starts_with(&self.folded) {
            MatchQuality::Prefix
        } else if folded.contains(&self.folded) {
            MatchQuality::Infix
        } else {
            return None;
        };
        // A mode admits every quality at least as good as itself: an exact hit is a prefix hit is
        // an infix hit, and the ranking below is what keeps the good ones at the top.
        if quality <= self.quality_floor() {
            Some(quality)
        } else {
            None
        }
    }

    /// The worst quality this mode admits.
    fn quality_floor(&self) -> MatchQuality {
        match self.mode {
            MatchMode::Exact => MatchQuality::Exact,
            MatchMode::Prefix => MatchQuality::Prefix,
            MatchMode::Infix => MatchQuality::Infix,
        }
    }
}

/// One label of one resource that the query matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelHit {
    /// The resource the label belongs to.
    pub resource: Node,
    /// The label itself, exactly as the vocabulary carries it.
    pub label: LexicalLabel,
    /// How it matched.
    pub quality: MatchQuality,
    /// The kinds this resource gives this label under, and where each came from.
    ///
    /// More than one only where a graph violates S13 by giving one string under two of the three
    /// properties — which happens, which is why this is a map and not a single kind, and which the
    /// integrity report names separately.
    pub kinds: BTreeMap<LabelKind, LabelOrigin>,
}

impl LabelHit {
    /// The best kind this hit was found under — preferred over alternative over hidden.
    ///
    /// Never `None` in a hit a search produced: a hit exists because at least one kind survived
    /// the query's filter. Returned as an `Option` anyway so a hand-built hit cannot panic a
    /// report.
    pub fn best_kind(&self) -> Option<LabelKind> {
        self.kinds.keys().next().copied()
    }

    /// The total order the report and the bound both use.
    ///
    /// Total, not merely consistent: no two distinct hits compare equal, because a hit is
    /// identified by its resource and its label and both are in the key. That is what lets the
    /// bound be applied during the scan — see [`LabelSearch`].
    fn key(&self) -> (MatchQuality, Option<LabelKind>, &str, Option<&str>, &Node) {
        (
            self.quality,
            self.best_kind(),
            self.label.text.as_str(),
            self.label.language.as_deref(),
            &self.resource,
        )
    }
}

/// What a query found, what it did not look at, and what it was told to leave out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelSearch {
    hits: Vec<LabelHit>,
    matched: usize,
    labels_read: usize,
    resources_read: usize,
    withheld: usize,
    withheld_resources: usize,
    bound: SearchBound,
}

impl LabelSearch {
    /// The hits, best first.
    pub fn hits(&self) -> &[LabelHit] {
        &self.hits
    }

    /// How many hits are reported.
    pub fn len(&self) -> usize {
        self.hits.len()
    }

    /// Whether nothing is reported.
    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    /// How many (resource, label) pairs matched, including the ones the bound discarded.
    pub fn matched(&self) -> usize {
        self.matched
    }

    /// How many (resource, label) pairs were considered.
    pub fn labels_read(&self) -> usize {
        self.labels_read
    }

    /// How many resources were considered.
    pub fn resources_read(&self) -> usize {
        self.resources_read
    }

    /// How many (resource, label) pairs matched on a resource the caller asked to leave out.
    ///
    /// Zero from [`CoreModel::search`], which leaves nothing out. Counted rather than discarded:
    /// a caller that withholds hits is still obliged to say there were some, and the count is the
    /// only thing standing between "you asked not to see them" and "the vocabulary has never
    /// heard of it" — which is the reading that makes someone create a duplicate.
    pub fn withheld(&self) -> usize {
        self.withheld
    }

    /// How many distinct resources those withheld matches were on.
    pub fn withheld_resources(&self) -> usize {
        self.withheld_resources
    }

    /// The bound this search ran under.
    pub fn bound(&self) -> SearchBound {
        self.bound
    }

    /// Whether every match is reported.
    ///
    /// About the bound and not about the exclusion: a search that withheld matches on excluded
    /// resources reported everything it was asked for, and [`withheld`](Self::withheld) is where
    /// the rest is accounted for.
    pub fn is_complete(&self) -> bool {
        self.matched == self.hits.len()
    }
}

impl CoreModel {
    /// Find every label in the vocabulary that the query matches.
    ///
    /// Reads the whole model: unlike a hierarchy walk, which starts from one concept, a search has
    /// nowhere to start from but everything. The cost is therefore the number of labels the
    /// vocabulary holds, and the *answer* is bounded separately by [`SearchBound`].
    ///
    /// Every label the model holds is searched, which includes the plain labels SKOS-XL entails
    /// under S55–S57: a vocabulary that keeps its terms as `skosxl:Label` resources is searchable
    /// on exactly the same terms as one that does not, and each hit carries the
    /// [`LabelOrigin`] that says which it was.
    pub fn search(&self, query: &LabelQuery) -> LabelSearch {
        self.search_excluding(query, &BTreeSet::new())
    }

    /// The same search, with the labels of `skip`'s resources matched, counted, and then left out.
    ///
    /// The exclusion set is a set of resources and nothing more: this method does not know why a
    /// caller wants them gone, which is what keeps a status the model does not model — `owl:`
    /// `deprecated` is not SKOS (`docs/adr/0041`) — from leaking into a SKOS query. The caller
    /// reads the status beside the model and hands over the nodes.
    ///
    /// **The exclusion is applied before the bound, and that is the whole reason it lives here.**
    /// Filtering the hits a bounded search returned would let 200 retired matches crowd out the
    /// current ones a caller asked for and report the result as complete — a false negative in
    /// the one command whose false negatives make people create duplicate concepts. Excluding
    /// during the scan means the bound is spent entirely on hits the caller will see.
    ///
    /// What is excluded is still *matched*, and [`LabelSearch::withheld`] carries the count. A
    /// caller may hide a hit; nothing here lets it hide that there was one.
    pub fn search_excluding(&self, query: &LabelQuery, skip: &BTreeSet<Node>) -> LabelSearch {
        let cap = query.bound.max_hits;
        // Sorting and truncating whenever the buffer reaches twice the ceiling keeps the memory a
        // search holds bounded by the ceiling, however many matches the vocabulary has. The result
        // is the same list a global sort would give because the ordering is total.
        let flush_at = cap.saturating_mul(2).max(1);
        let mut hits: Vec<LabelHit> = Vec::new();
        let mut matched = 0;
        let mut labels_read = 0;
        let mut resources_read = 0;
        let mut withheld = 0;
        let mut withheld_resources = 0;

        for (node, resource) in self.resources() {
            resources_read += 1;
            let excluded = skip.contains(node);
            let mut counted_this_resource = false;
            for (label, origins) in resource.labels() {
                labels_read += 1;
                if !query.selects_language(label) {
                    continue;
                }
                let kinds: BTreeMap<LabelKind, LabelOrigin> = origins
                    .iter()
                    .filter(|(kind, _)| query.kinds.contains(kind))
                    .map(|(kind, origin)| (*kind, *origin))
                    .collect();
                if kinds.is_empty() {
                    continue;
                }
                let Some(quality) = query.matches_text(label) else {
                    continue;
                };
                // Counted before the bound is consulted, so a caller is told what it asked not to
                // see whether or not the answer was also truncated.
                if excluded {
                    withheld += 1;
                    if !counted_this_resource {
                        counted_this_resource = true;
                        withheld_resources += 1;
                    }
                    continue;
                }
                matched += 1;
                if cap == 0 {
                    continue;
                }
                hits.push(LabelHit {
                    resource: node.clone(),
                    label: label.clone(),
                    quality,
                    kinds,
                });
                if hits.len() >= flush_at {
                    hits.sort_by(|left, right| left.key().cmp(&right.key()));
                    hits.truncate(cap);
                }
            }
        }

        hits.sort_by(|left, right| left.key().cmp(&right.key()));
        hits.truncate(cap);

        LabelSearch {
            hits,
            matched,
            labels_read,
            resources_read,
            withheld,
            withheld_resources,
            bound: query.bound,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::{RDF_LANG_STRING, XSD_STRING};
    use crate::model::{Literal, SkosRule, Statement, Term};
    use crate::ns;
    use crate::xl::{SKOSXL_LITERAL_FORM, SKOSXL_PREF_LABEL};

    fn ex(name: &str) -> Node {
        Node::iri(format!("http://example.org/{name}"))
    }

    fn skos(local: &str) -> String {
        format!("{}{local}", ns::SKOS)
    }

    /// `"text"@tag`, or a simple literal when the tag is `None`.
    fn text(value: &str, language: Option<&str>) -> Term {
        Term::Literal(Literal {
            value: value.to_owned(),
            language: language.map(str::to_owned),
            datatype: match language {
                Some(_) => RDF_LANG_STRING.to_owned(),
                None => XSD_STRING.to_owned(),
            },
        })
    }

    fn labelled(subject: &Node, kind: LabelKind, value: &str, language: Option<&str>) -> Statement {
        Statement::new(subject.clone(), kind.property_iri(), text(value, language))
    }

    /// A small multilingual vocabulary with all three label kinds in it.
    fn vocabulary() -> CoreModel {
        CoreModel::from_statements([
            Statement::new(ex("bank"), skos("prefLabel"), text("Bank", Some("en"))),
            Statement::new(
                ex("bank"),
                skos("altLabel"),
                text("Banking house", Some("en")),
            ),
            Statement::new(ex("bank"), skos("hiddenLabel"), text("banc", Some("en"))),
            Statement::new(ex("bank"), skos("prefLabel"), text("Banque", Some("fr"))),
            Statement::new(
                ex("riverbank"),
                skos("prefLabel"),
                text("River bank", Some("en-GB")),
            ),
            Statement::new(
                ex("investment"),
                skos("prefLabel"),
                text("Investment banking", Some("en")),
            ),
            Statement::new(ex("code"), skos("prefLabel"), text("BANK CODE", None)),
        ])
    }

    fn found(search: &LabelSearch) -> Vec<String> {
        search
            .hits()
            .iter()
            .map(|hit| format!("{} {}", hit.resource, hit.label))
            .collect()
    }

    /// The default query is forgiving: anywhere in the label, any language, all three kinds.
    #[test]
    fn an_infix_query_finds_the_word_wherever_it_sits_in_the_label() {
        let model = vocabulary();
        let search = model.search(&LabelQuery::new("bank").expect("a query"));

        assert_eq!(search.matched(), 5, "{:#?}", found(&search));
        assert!(search.is_complete());
        // Best first: the exact match, then the two that start with it, then the ones that
        // contain it — and within a rank, the preferred label before the alternative.
        assert_eq!(
            found(&search),
            vec![
                "<http://example.org/bank> \"Bank\"@en".to_owned(),
                "<http://example.org/code> \"BANK CODE\"".to_owned(),
                "<http://example.org/bank> \"Banking house\"@en".to_owned(),
                "<http://example.org/investment> \"Investment banking\"@en".to_owned(),
                "<http://example.org/riverbank> \"River bank\"@en-gb".to_owned(),
            ]
        );
        // "Banque"@fr is not here either: the French for bank does not contain the English word,
        // and this is a string match and not a translation.
        assert!(!found(&search).iter().any(|hit| hit.contains("Banque")));
        // "banc"@en, the hidden label, is not here and should not be: it is what finds the
        // *mis-spelling*, not what a correctly spelled query matches.
        assert!(!found(&search).iter().any(|hit| hit.contains("banc")));
    }

    /// §5.1: the hidden label exists for search, so it is searched — and it is found by the
    /// mis-spelling the specification's own example is about.
    #[test]
    fn a_hidden_label_is_searched_because_that_is_what_the_specification_defines_it_for() {
        let model = vocabulary();
        let search = model.search(&LabelQuery::new("banc").expect("a query"));

        assert_eq!(search.len(), 1);
        let hit = &search.hits()[0];
        assert_eq!(hit.resource, ex("bank"));
        assert_eq!(hit.best_kind(), Some(LabelKind::Hidden));
        assert_eq!(hit.quality, MatchQuality::Exact);
    }

    /// And it can be excluded, for a caller that is showing results to the public rather than to
    /// the person curating the vocabulary.
    #[test]
    fn a_query_can_be_narrowed_to_the_kinds_a_caller_will_display() {
        let model = vocabulary();
        let query = LabelQuery::new("banc")
            .expect("a query")
            .with_kinds([LabelKind::Preferred, LabelKind::Alternative])
            .expect("two kinds");

        assert!(model.search(&query).is_empty());
        assert_eq!(
            LabelQuery::new("banc").expect("a query").with_kinds([]),
            Err(QueryError::NoKinds)
        );
    }

    #[test]
    fn prefix_and_exact_modes_narrow_what_infix_admits() {
        let model = vocabulary();
        let prefix = model.search(
            &LabelQuery::new("bank")
                .expect("a query")
                .with_mode(MatchMode::Prefix),
        );
        assert_eq!(
            found(&prefix),
            vec![
                "<http://example.org/bank> \"Bank\"@en".to_owned(),
                "<http://example.org/code> \"BANK CODE\"".to_owned(),
                "<http://example.org/bank> \"Banking house\"@en".to_owned(),
            ]
        );

        let exact = model.search(
            &LabelQuery::new("bank")
                .expect("a query")
                .with_mode(MatchMode::Exact),
        );
        assert_eq!(
            found(&exact),
            vec!["<http://example.org/bank> \"Bank\"@en".to_owned()]
        );
        assert_eq!(exact.hits()[0].quality, MatchQuality::Exact);
    }

    /// Matching is over Unicode characters, as §5.1 says a label is — not over ASCII bytes.
    #[test]
    fn case_is_ignored_beyond_ascii() {
        let model = CoreModel::from_statements([
            Statement::new(ex("school"), skos("prefLabel"), text("École", Some("fr"))),
            Statement::new(ex("street"), skos("prefLabel"), text("ΟΔΌΣ", Some("el"))),
        ]);

        assert_eq!(model.search(&LabelQuery::new("école").expect("q")).len(), 1);
        assert_eq!(model.search(&LabelQuery::new("ÉCOLE").expect("q")).len(), 1);
        // Greek, where lowercasing is context-sensitive: a final Σ lowercases to ς, and both
        // sides of the comparison get the same treatment, so the uppercase query still matches.
        assert_eq!(model.search(&LabelQuery::new("ΟΔΌΣ").expect("q")).len(), 1);
        assert_eq!(model.search(&LabelQuery::new("οδός").expect("q")).len(), 1);
    }

    /// **The gap this test used to pin, now inverted.** Until iteration 60 matching lowercased,
    /// and `ß` lowercases to itself — so the German convention of typing it `ss` found nothing.
    /// Matching now folds case (Unicode §3.13), so it does.
    #[test]
    fn case_folding_finds_a_sharp_s_typed_as_ss() {
        let model = CoreModel::from_statements([Statement::new(
            ex("street"),
            skos("prefLabel"),
            text("Straße", Some("de")),
        )]);

        assert_eq!(
            model.search(&LabelQuery::new("straße").expect("q")).len(),
            1
        );
        for typed in ["strasse", "STRASSE", "Strasse", "StRaSsE"] {
            let found = model.search(&LabelQuery::new(typed).expect("q"));
            assert_eq!(found.len(), 1, "{typed} should find Straße");
            // And it is an *exact* match, not a partial one: the two strings are the same term.
            assert_eq!(found.hits()[0].quality, MatchQuality::Exact, "{typed}");
        }

        // The same gap in Greek, and the likelier one to be hit: a user typing a medial σ where
        // the label ends in ς found nothing. Case folding maps both to σ; lowercasing does not.
        let greek = CoreModel::from_statements([Statement::new(
            ex("street"),
            skos("prefLabel"),
            text("ΟΔΌΣ", Some("el")),
        )]);
        assert_eq!(greek.search(&LabelQuery::new("οδόσ").expect("q")).len(), 1);
        assert_eq!(greek.search(&LabelQuery::new("οδός").expect("q")).len(), 1);
    }

    /// **The other gap, also inverted.** A composed `é` and a decomposed `e` + U+0301 look
    /// identical and are different strings. Matching now normalises, so either spelling of the
    /// query finds either spelling of the label — and both forms occur in real thesauri.
    #[test]
    fn normalisation_finds_either_spelling_of_an_accented_label() {
        for stored in ["E\u{301}cole", "\u{c9}cole"] {
            let model = CoreModel::from_statements([Statement::new(
                ex("school"),
                skos("prefLabel"),
                text(stored, Some("fr")),
            )]);

            for typed in ["e\u{301}cole", "\u{e9}cole", "\u{c9}COLE", "E\u{301}COLE"] {
                let found = model.search(&LabelQuery::new(typed).expect("q"));
                assert_eq!(
                    found.len(),
                    1,
                    "a label stored as {stored:?} should be found by {typed:?}"
                );
                assert_eq!(found.hits()[0].quality, MatchQuality::Exact);
            }
        }
    }

    /// **What matching still will not do, pinned in the direction that now matters.** With folding
    /// and normalisation in, the risk has flipped: the next plausible step is stripping accents,
    /// which would silently merge terms that are not the same word. Each of these must stay a
    /// miss, and each is the kind of miss an authored `skos:hiddenLabel` is the specification's
    /// own answer to.
    #[test]
    fn matching_still_does_not_strip_accents_or_correct_spelling() {
        let model = CoreModel::from_statements([
            Statement::new(ex("school"), skos("prefLabel"), text("École", Some("fr"))),
            Statement::new(
                ex("ecology"),
                skos("prefLabel"),
                text("Ökologie", Some("de")),
            ),
            Statement::new(ex("colour"), skos("prefLabel"), text("colour", Some("en"))),
        ]);

        for typed in ["ecole", "okologie", "color", "schools"] {
            assert!(
                model.search(&LabelQuery::new(typed).expect("q")).is_empty(),
                "{typed} must not match: stripping accents or correcting spelling would merge \
                 terms that are not the same word (docs/UNTESTED.md)"
            );
        }
    }

    /// RFC 4647 §3.3.1, with the specification's own shape of example: the range selects the tag
    /// it equals and the tags it prefixes at a hyphen, and nothing else.
    #[test]
    fn a_language_range_selects_by_rfc_4647_basic_filtering() {
        let en = LanguageRange::parse("en").expect("a range");
        assert!(en.selects("en"));
        assert!(en.selects("en-gb"));
        assert!(en.selects("EN-GB"), "the comparison is case-insensitive");
        assert!(!en.selects("enm"), "Middle English is not English");

        let de_de = LanguageRange::parse("de-DE").expect("a range");
        assert!(de_de.selects("de-de-1996"), "RFC 4647 §3.3.1's own example");
        assert!(
            !de_de.selects("de-latn-de"),
            "basic filtering does not skip an intermediate subtag; extended filtering would"
        );

        assert!(LanguageRange::parse("*").expect("a range").selects("zxx"));
    }

    #[test]
    fn a_language_filter_narrows_a_search_and_the_wildcard_is_not_the_same_as_no_filter() {
        let model = vocabulary();
        let english =
            LabelQuery::new("bank")
                .expect("a query")
                .with_language(LanguageFilter::Range(
                    LanguageRange::parse("en").expect("a range"),
                ));
        // "River bank"@en-GB is selected by the range `en`; "Banque"@fr and the untagged one are
        // not — and "River bank" is an infix hit, so it comes after the exact and prefix ones.
        assert_eq!(
            found(&model.search(&english)),
            vec![
                "<http://example.org/bank> \"Bank\"@en".to_owned(),
                "<http://example.org/bank> \"Banking house\"@en".to_owned(),
                "<http://example.org/investment> \"Investment banking\"@en".to_owned(),
                "<http://example.org/riverbank> \"River bank\"@en-gb".to_owned(),
            ]
        );

        let tagged =
            LabelQuery::new("bank")
                .expect("a query")
                .with_language(LanguageFilter::Range(
                    LanguageRange::parse("*").expect("a range"),
                ));
        assert_eq!(
            model.search(&tagged).matched(),
            4,
            "the wildcard matches any tag, and an untagged label has no tag to match"
        );

        let untagged = LabelQuery::new("bank")
            .expect("a query")
            .with_language(LanguageFilter::Untagged);
        assert_eq!(
            found(&model.search(&untagged)),
            vec!["<http://example.org/code> \"BANK CODE\"".to_owned()]
        );
    }

    /// A malformed range is refused rather than kept and matched against nothing: silently
    /// selecting no labels reads, in a report, exactly like a vocabulary that has none.
    #[test]
    fn a_malformed_language_range_is_refused() {
        for bad in ["", "e n", "en-", "-en", "abcdefghi", "en_GB", "**"] {
            assert!(
                matches!(
                    LanguageRange::parse(bad),
                    Err(QueryError::MalformedLanguageRange { .. })
                ),
                "{bad:?} was accepted"
            );
        }
        for good in [
            "en",
            "en-GB",
            "de-DE-1996",
            "zh-Hant",
            "*",
            "x",
            "qaa-Qaaa-QM",
        ] {
            assert!(LanguageRange::parse(good).is_ok(), "{good:?} was refused");
        }
    }

    /// An empty query would match every label in the vocabulary. That is not a search result.
    #[test]
    fn an_empty_query_is_refused() {
        assert_eq!(LabelQuery::new(""), Err(QueryError::EmptyQuery));
    }

    /// A label the vocabulary only holds as a SKOS-XL literal form is found, and the hit says
    /// which chain put it there. Without this a thesaurus modelled the ISO 25964 way — which is
    /// SKOS-XL, per this crate's own preamble — would be unsearchable.
    #[test]
    fn a_label_that_exists_only_by_dumbing_down_from_skos_xl_is_found_and_explains_itself() {
        let model = CoreModel::from_statements([
            Statement::new(ex("bank"), SKOSXL_PREF_LABEL.to_owned(), ex("label-1")),
            Statement::new(
                ex("label-1"),
                SKOSXL_LITERAL_FORM.to_owned(),
                text("Bank", Some("en")),
            ),
        ]);

        let search = model.search(&LabelQuery::new("bank").expect("a query"));
        let hit = search
            .hits()
            .iter()
            .find(|hit| hit.resource == ex("bank"))
            .expect("the concept is found through its XL label");
        assert_eq!(
            hit.kinds.get(&LabelKind::Preferred),
            Some(&LabelOrigin::DumbedDown(SkosRule::S55))
        );
    }

    /// One string under two properties violates S13; the search reports one hit carrying both,
    /// rather than the same label twice.
    #[test]
    fn a_label_given_under_two_kinds_is_one_hit_with_both() {
        let model = CoreModel::from_statements([
            labelled(&ex("bank"), LabelKind::Preferred, "Bank", Some("en")),
            labelled(&ex("bank"), LabelKind::Alternative, "Bank", Some("en")),
        ]);

        let search = model.search(&LabelQuery::new("bank").expect("a query"));
        assert_eq!(search.len(), 1);
        assert_eq!(
            search.hits()[0].kinds.keys().copied().collect::<Vec<_>>(),
            vec![LabelKind::Preferred, LabelKind::Alternative]
        );
        assert_eq!(search.hits()[0].best_kind(), Some(LabelKind::Preferred));
    }

    /// The bound truncates the *answer*, says so, and still counts what it dropped.
    #[test]
    fn the_bound_reports_the_best_hits_and_admits_that_it_stopped() {
        let mut statements = Vec::new();
        for index in 0..50 {
            statements.push(labelled(
                &ex(&format!("c{index:02}")),
                LabelKind::Alternative,
                &format!("Banking {index:02}"),
                Some("en"),
            ));
        }
        // One exact hit, pushed in last, so a bound that kept the first three it saw would miss
        // it. It is the one that must survive.
        statements.push(labelled(
            &ex("zzz"),
            LabelKind::Preferred,
            "Bank",
            Some("en"),
        ));
        let model = CoreModel::from_statements(statements);

        let query = LabelQuery::new("bank")
            .expect("a query")
            .with_bound(SearchBound { max_hits: 3 });
        let search = model.search(&query);

        assert_eq!(search.matched(), 51);
        assert_eq!(search.len(), 3);
        assert!(!search.is_complete());
        assert_eq!(
            found(&search),
            vec![
                "<http://example.org/zzz> \"Bank\"@en".to_owned(),
                "<http://example.org/c00> \"Banking 00\"@en".to_owned(),
                "<http://example.org/c01> \"Banking 01\"@en".to_owned(),
            ]
        );
    }

    /// The bound is applied during the scan to keep memory bounded, so this is the claim that
    /// makes that safe: the truncated list is exactly the head of the list a global sort gives.
    #[test]
    fn truncating_during_the_scan_gives_the_same_answer_as_sorting_everything() {
        let mut statements = Vec::new();
        for index in 0..200 {
            // Deliberately not in label order, so a scan that kept what it saw first would differ.
            let value = format!("Bank {:03}", (index * 137) % 200);
            statements.push(labelled(
                &ex(&format!("c{index:03}")),
                LabelKind::Alternative,
                &value,
                Some("en"),
            ));
        }
        let model = CoreModel::from_statements(statements);

        let unbounded = model.search(&LabelQuery::new("bank").expect("a query").with_bound(
            SearchBound {
                max_hits: usize::MAX,
            },
        ));
        let bounded = model.search(
            &LabelQuery::new("bank")
                .expect("a query")
                .with_bound(SearchBound { max_hits: 7 }),
        );

        assert_eq!(unbounded.len(), 200);
        assert_eq!(bounded.hits(), &unbounded.hits()[..7]);
    }

    /// An excluded resource contributes no hit, and the count of what it would have contributed
    /// is the thing that stops the exclusion reading as an absence.
    #[test]
    fn excluding_a_resource_withholds_its_hits_and_counts_them() {
        let model = vocabulary();
        let query = LabelQuery::new("bank").expect("a query");

        let skip = BTreeSet::from([ex("bank")]);
        let search = model.search_excluding(&query, &skip);

        assert!(
            !found(&search).iter().any(|hit| hit.contains("/bank> ")),
            "{:?}",
            found(&search)
        );
        // Two labels on the one excluded resource matched: "Bank"@en and "Banking house"@en.
        assert_eq!(search.withheld(), 2);
        assert_eq!(search.withheld_resources(), 1);
        assert!(
            search.is_complete(),
            "everything the caller asked for is shown; is_complete is about the bound"
        );
    }

    /// The claim that makes the exclusion belong here rather than in the caller: the bound is
    /// spent entirely on hits the caller will see. Filtering a bounded answer afterwards would
    /// return nothing at all from this vocabulary and call it complete.
    #[test]
    fn the_bound_is_spent_on_the_hits_that_survive_the_exclusion() {
        let mut statements = Vec::new();
        let mut skip = BTreeSet::new();
        // Fifty excluded resources whose labels sort ahead of every current one, so a bound
        // applied first would be exhausted before a single current hit was kept.
        for index in 0..50 {
            let node = ex(&format!("old{index:02}"));
            statements.push(labelled(
                &node,
                LabelKind::Preferred,
                &format!("Bank AAA {index:02}"),
                Some("en"),
            ));
            skip.insert(node);
        }
        for index in 0..3 {
            statements.push(labelled(
                &ex(&format!("new{index:02}")),
                LabelKind::Preferred,
                &format!("Bank ZZZ {index:02}"),
                Some("en"),
            ));
        }
        let model = CoreModel::from_statements(statements);

        let query = LabelQuery::new("bank")
            .expect("a query")
            .with_bound(SearchBound { max_hits: 3 });
        let search = model.search_excluding(&query, &skip);

        assert_eq!(search.len(), 3, "the bound went to the current concepts");
        assert!(search.is_complete());
        assert_eq!(search.withheld(), 50);
        assert_eq!(search.withheld_resources(), 50);
        assert!(
            found(&search).iter().all(|hit| hit.contains("ZZZ")),
            "{:?}",
            found(&search)
        );
    }

    /// A search that excludes nothing is the search that was there before, withheld count and all.
    #[test]
    fn an_ordinary_search_withholds_nothing() {
        let search = vocabulary().search(&LabelQuery::new("bank").expect("a query"));

        assert_eq!(search.withheld(), 0);
        assert_eq!(search.withheld_resources(), 0);
        assert_eq!(
            search,
            vocabulary()
                .search_excluding(&LabelQuery::new("bank").expect("a query"), &BTreeSet::new())
        );
    }

    /// What the search looked at, which is what makes its cost legible in a report.
    #[test]
    fn a_search_says_how_much_of_the_vocabulary_it_read() {
        let model = vocabulary();
        let search = model.search(&LabelQuery::new("nothing at all").expect("a query"));

        assert!(search.is_empty());
        assert!(
            search.is_complete(),
            "nothing matched, so nothing was dropped"
        );
        assert_eq!(search.resources_read(), 4);
        assert_eq!(search.labels_read(), 7);
    }
}
