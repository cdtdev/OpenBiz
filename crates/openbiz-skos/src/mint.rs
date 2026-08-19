//! Minting the IRI a new concept will be known by, for as long as the vocabulary exists.
//!
//! # Why this is not a formatting problem
//!
//! An IRI is the one thing about a concept that can never be corrected. Labels are translated,
//! definitions are rewritten, a concept moves in the hierarchy and is deprecated and replaced —
//! and through all of it the IRI is what every downstream system, every published dataset and
//! every citation is holding. So the two ways to get this wrong are both permanent:
//!
//! 1. **Minting an IRI something else already uses.** The store does not refuse it, because RDF
//!    has no such notion: statements about the same IRI are statements about the same thing. Two
//!    concepts silently become one, and the merge is discovered later by someone reading a concept
//!    with two preferred labels in the same language.
//! 2. **Minting an IRI that encodes something mutable.** A readable IRI derived from a label is a
//!    promise that the label will not change, which nobody can keep. It is still frequently the
//!    right choice — it is legible in a SPARQL query and in a published dataset — but it is a
//!    trade, and a tool that makes it silently has decided something on the user's behalf.
//!
//! This module is deliberately engine-free, like the rest of `openbiz-skos`: it mints from a
//! pattern, a label and a set of IRIs already in use, and every refusal names what it saw.
//!
//! # The two policies, which are the same mechanism
//!
//! `CLAUDE.md`'s backlog calls this "opaque-vs-readable policy", and both are one pattern with one
//! placeholder:
//!
//! - `https://example.org/thesaurus/c_{n}` — **opaque**. The local name carries no meaning, so it
//!   cannot become wrong. This is what AGROVOC (`c_1234`) and LCSH (`sh85001234`) do.
//! - `https://example.org/thesaurus/{slug}` — **readable**. The local name is derived from the
//!   label at the moment of minting and is never revised afterwards.
//!
//! # What we do that the incumbents do not
//!
//! Every tool in this market has a configurable URI pattern, and every one of them makes you
//! configure it against nothing. Here the default pattern is **read off the vocabulary itself**
//! ([`IriConvention`]) — the namespace most of its concepts are already in, and whether their
//! local names are numbered or worded — so the suggestion is evidence rather than a preference,
//! and a vocabulary whose concepts are spread over three namespaces gets no suggestion at all
//! instead of a confident wrong one.
//!
//! And the two collision rules differ, on purpose:
//!
//! - A **numbered** collision is resolved by going *above the highest number in use*, never by
//!   filling a gap. A gap is evidence that something was there — the deprecation lifecycle keeps
//!   history rather than deleting, but an IRI that left this vocabulary by any route must not come
//!   back attached to a different concept.
//! - A **worded** collision is refused outright. Appending `-2` is what the incumbents do, and
//!   `renewable-energy-2` is a silo with a suffix: it means the vocabulary already has a concept
//!   with this label and `CLAUDE.md` §1.7 says reuse outranks creation. If the two really are
//!   homographs — Java the island, Java the language — the answer thesaurus practice has used for
//!   decades is a qualifier in the term itself, which the caller supplies by minting from the
//!   qualified label.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use thiserror::Error;

/// What a pattern leaves for the minter to fill in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placeholder {
    /// `{n}` — a decimal number above every number already in use.
    Number,
    /// `{slug}` — the label, reduced to characters an IRI may carry.
    Slug,
}

impl Placeholder {
    /// The token as it is written in a pattern, including the braces.
    pub const fn token(self) -> &'static str {
        match self {
            Placeholder::Number => "{n}",
            Placeholder::Slug => "{slug}",
        }
    }

    /// Which of the two policies this placeholder is.
    pub const fn policy(self) -> MintPolicy {
        match self {
            Placeholder::Number => MintPolicy::Opaque,
            Placeholder::Slug => MintPolicy::Readable,
        }
    }
}

/// Whether the minted local name means anything.
///
/// This is the decision the backlog item calls "opaque-vs-readable", and it is recorded on every
/// result so a report can state which trade was made rather than leaving the reader to infer it
/// from the shape of the IRI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MintPolicy {
    /// The local name carries no meaning, so nothing about the concept can make it wrong.
    Opaque,
    /// The local name is derived from the label, and is not revised when the label changes.
    Readable,
}

impl fmt::Display for MintPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MintPolicy::Opaque => f.write_str("opaque"),
            MintPolicy::Readable => f.write_str("readable"),
        }
    }
}

/// A pattern with exactly one placeholder: literal text, the placeholder, literal text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintPattern {
    /// Everything before the placeholder.
    prefix: String,
    /// The one placeholder.
    placeholder: Placeholder,
    /// Everything after it. Usually empty; a pattern like `…/{slug}#concept` is legal.
    suffix: String,
}

impl MintPattern {
    /// Read a pattern.
    ///
    /// Exactly one placeholder is required. Neither bound is arbitrary: a pattern with none mints
    /// the same IRI every time, which is the silent-merge failure this module exists to prevent,
    /// and a pattern with two has no reading that is obviously right — `{slug}-{n}` looks like
    /// disambiguation and is the `-2` suffix under another name.
    pub fn parse(pattern: &str) -> Result<Self, PatternError> {
        let mut found: Option<(usize, Placeholder)> = None;
        let mut rest = pattern;
        let mut at = 0;

        while let Some(open) = rest.find('{') {
            let Some(close) = rest[open..].find('}') else {
                return Err(PatternError::UnclosedPlaceholder {
                    at: at + open,
                    pattern: pattern.to_owned(),
                });
            };
            let close = open + close;
            let token = &rest[open..=close];
            let placeholder = match token {
                "{n}" => Placeholder::Number,
                "{slug}" => Placeholder::Slug,
                other => {
                    return Err(PatternError::UnknownPlaceholder {
                        token: other.to_owned(),
                    })
                }
            };
            if let Some((first, _)) = found {
                return Err(PatternError::TwoPlaceholders {
                    first,
                    second: at + open,
                });
            }
            found = Some((at + open, placeholder));
            at += close + 1;
            rest = &rest[close + 1..];
        }

        let Some((position, placeholder)) = found else {
            return Err(PatternError::NoPlaceholder {
                pattern: pattern.to_owned(),
            });
        };

        let prefix = pattern[..position].to_owned();
        let suffix = pattern[position + placeholder.token().len()..].to_owned();

        // The literal halves have to be IRI text themselves. Checked here rather than after
        // filling, because "the pattern is wrong" and "this label produced something odd" are
        // different mistakes and telling them apart is most of the value of the message.
        for (part, which) in [(&prefix, "before"), (&suffix, "after")] {
            if let Some(bad) = part.chars().find(|c| !iri_character(*c)) {
                return Err(PatternError::NotIriText {
                    character: bad,
                    which,
                });
            }
        }
        if scheme_of(&prefix).is_none() {
            return Err(PatternError::NoScheme {
                prefix: prefix.clone(),
            });
        }

        Ok(MintPattern {
            prefix,
            placeholder,
            suffix,
        })
    }

    /// Build a pattern from its parts, without going through the text form.
    ///
    /// Used by [`IriConvention::suggest`], whose parts come from IRIs the vocabulary already
    /// holds; it validates them the same way, because a namespace read out of a store is still
    /// text somebody typed once.
    pub fn new(prefix: &str, placeholder: Placeholder, suffix: &str) -> Result<Self, PatternError> {
        MintPattern::parse(&format!("{prefix}{}{suffix}", placeholder.token()))
    }

    /// Everything before the placeholder — the namespace, and any fixed part of the local name.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Everything after the placeholder.
    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    /// Which placeholder this pattern has.
    pub fn placeholder(&self) -> Placeholder {
        self.placeholder
    }

    /// Which policy minting under this pattern applies.
    pub fn policy(&self) -> MintPolicy {
        self.placeholder.policy()
    }

    /// The IRI this pattern makes of `filling`.
    pub fn fill(&self, filling: &str) -> String {
        format!("{}{filling}{}", self.prefix, self.suffix)
    }

    /// If `iri` was minted from this pattern, what went in the placeholder.
    pub fn local_of<'a>(&self, iri: &'a str) -> Option<&'a str> {
        iri.strip_prefix(&self.prefix)?.strip_suffix(&self.suffix)
    }
}

impl fmt::Display for MintPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{}",
            self.prefix,
            self.placeholder.token(),
            self.suffix
        )
    }
}

/// The pattern does not describe an IRI this build will mint.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum PatternError {
    /// No placeholder, so every mint would return the same IRI.
    #[error(
        "{pattern:?} has no placeholder, so it would mint the same IRI every time; \
         use {{n}} for an opaque IRI or {{slug}} for a readable one"
    )]
    NoPlaceholder {
        /// What was given.
        pattern: String,
    },
    /// Two placeholders, which has no reading we are willing to guess at.
    #[error(
        "a pattern takes one placeholder and this has two, at {first} and {second}; \
         a pattern like {{slug}}-{{n}} is a disambiguating suffix, which this build refuses \
         on purpose"
    )]
    TwoPlaceholders {
        /// Byte offset of the first.
        first: usize,
        /// Byte offset of the second.
        second: usize,
    },
    /// A `{` with no `}` after it.
    #[error("{pattern:?} opens a placeholder at {at} and never closes it")]
    UnclosedPlaceholder {
        /// Byte offset of the `{`.
        at: usize,
        /// What was given.
        pattern: String,
    },
    /// A placeholder we do not have.
    #[error("{token} is not a placeholder; this build has {{n}} and {{slug}}")]
    UnknownPlaceholder {
        /// What was written, braces included.
        token: String,
    },
    /// The literal text around the placeholder could not appear in an IRI.
    #[error("the literal text {which} the placeholder holds {character:?}, which an IRI may not")]
    NotIriText {
        /// The offending character.
        character: char,
        /// `before` or `after`, for the message.
        which: &'static str,
    },
    /// The pattern is relative, and a concept IRI must not be.
    #[error(
        "{prefix:?} has no scheme, so the pattern would mint a relative IRI; \
         a concept's IRI is what every other system holds and must be absolute"
    )]
    NoScheme {
        /// The literal text before the placeholder.
        prefix: String,
    },
}

/// How long a slug is allowed to get.
///
/// A 400-character label is ordinary in a definition-heavy vocabulary and makes an IRI nobody can
/// use. The bound cuts at a word boundary and the result says that it did, because a truncated
/// slug is a local name that no longer says what the caller thinks it says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlugBound {
    /// The most characters — not bytes — a slug may carry.
    pub max_chars: usize,
}

impl SlugBound {
    /// Long enough for a multi-word term, short enough to read in a URL bar.
    ///
    /// **Unmeasured.** No corpus of enterprise labels was consulted; see `docs/UNTESTED.md`.
    pub const DEFAULT: SlugBound = SlugBound { max_chars: 96 };
}

impl Default for SlugBound {
    fn default() -> Self {
        SlugBound::DEFAULT
    }
}

/// A label reduced to something an IRI can carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slug {
    /// The slug.
    text: String,
    /// Whether [`SlugBound`] cut it short.
    truncated: bool,
}

impl Slug {
    /// The slug text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether the bound cut it short.
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

/// Reduce a label to a slug.
///
/// # What is kept, and why nothing is transliterated
///
/// RFC 3987 §2.2 allows a great deal more than ASCII in an IRI: `iunreserved` is
/// `ALPHA / DIGIT / "-" / "." / "_" / "~" / ucschar`, and `ucschar` covers essentially the whole
/// of assigned Unicode above U+00A0. So `Ökologie` needs no transliteration to be a legal IRI, and
/// this does not perform one. That is a deliberate departure from the ASCII-slug habit: mapping
/// `ö` to `o` is a lossy, language-specific guess — Swedish and German disagree about it — and it
/// manufactures collisions between terms that are not the same word.
///
/// A character is kept when it is alphanumeric **and** `iunreserved`; whitespace and everything
/// else become a single `-`; apostrophes are elided rather than split on, so `Müller's cheese`
/// is `müllers-cheese` and not `müller-s-cheese`. Case is lowered with Rust's Unicode-aware
/// `to_lowercase`, which is a *mapping* and not a full case fold — and unlike label matching,
/// which moved to folding at iteration 60, that is the right operation here. A slug becomes a
/// local name in an IRI that is then published, cited, and compared byte for byte; folding would
/// mint `strasse` for `Straße`, silently changing the identifier a German cataloguer typed into a
/// different word. Folding is for deciding whether two strings are the same term ([`fold`]);
/// lowercasing is for deriving a stable identifier from one of them.
///
/// [`fold`]: crate::fold
pub fn slug(label: &str, bound: SlugBound) -> Result<Slug, SlugError> {
    let mut out = String::new();
    let mut pending_boundary = false;
    let mut truncated = false;
    let mut kept = 0usize;

    for character in label.to_lowercase().chars() {
        if character == '\'' || character == '\u{2019}' {
            // Word-internal by convention in every language that writes it, so it joins rather
            // than separates. Dropped outright: an IRI may carry `'`, but a local name that ends
            // in one reads as a typo.
            continue;
        }
        if character.is_alphanumeric() && iunreserved(character) {
            if pending_boundary && !out.is_empty() {
                out.push('-');
                kept += 1;
                pending_boundary = false;
            }
            if kept >= bound.max_chars {
                truncated = true;
                break;
            }
            out.push(character);
            kept += 1;
        } else {
            pending_boundary = true;
        }
    }

    // Cut back to a word boundary. `renewable-energy-sourc` is worse than `renewable-energy` —
    // it looks like a typo rather than an abbreviation — so a truncation that landed inside a
    // word gives that word up, unless the whole slug is one word and there is nothing to give.
    if truncated {
        if let Some(boundary) = out.rfind('-') {
            out.truncate(boundary);
        }
    }
    while out.ends_with('-') {
        out.pop();
    }

    if out.is_empty() {
        return Err(SlugError::NothingUsable {
            label: label.to_owned(),
        });
    }

    Ok(Slug {
        text: out,
        truncated,
    })
}

/// The label could not become a slug.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SlugError {
    /// Every character was dropped — a label of punctuation, symbols, or emoji.
    #[error(
        "{label:?} holds no character an IRI can carry, so it cannot mint a readable IRI; \
         mint an opaque one, or give a pattern with {{n}}"
    )]
    NothingUsable {
        /// The label as given.
        label: String,
    },
}

/// The IRIs a mint must not collide with, and where each was found.
///
/// Only IRIs under the pattern's prefix are worth holding, so this is built with that prefix and
/// ignores everything else. That is what keeps the scan proportional to the namespace rather than
/// to the store: a deployment with a million concepts in six vocabularies still only holds the
/// handful that could possibly collide.
#[derive(Debug, Clone, Default)]
pub struct MintScan {
    /// The pattern prefix an IRI must start with to be worth keeping.
    prefix: String,
    /// IRI to the caller's phrase for where it was seen. First writer wins, so the vocabulary
    /// itself is reported in preference to a staged change that also mentions the IRI.
    seen: HashMap<String, String>,
    /// How many IRIs each source contributed, for the report.
    sources: BTreeMap<String, usize>,
    /// How many IRIs were offered in total, including the ones outside the prefix.
    offered: usize,
}

impl MintScan {
    /// A scan that keeps IRIs beginning with `prefix`.
    pub fn under(prefix: &str) -> Self {
        MintScan {
            prefix: prefix.to_owned(),
            ..MintScan::default()
        }
    }

    /// Offer an IRI, said to have been found in `source`.
    ///
    /// `source` is the caller's words — "the vocabulary", "candidate 7" — because this crate has
    /// no idea what a candidate or a named graph is and should not learn.
    pub fn push(&mut self, iri: &str, source: &str) {
        self.offered += 1;
        if !iri.starts_with(&self.prefix) {
            return;
        }
        if self.seen.contains_key(iri) {
            return;
        }
        self.seen.insert(iri.to_owned(), source.to_owned());
        *self.sources.entry(source.to_owned()).or_default() += 1;
    }

    /// Where `iri` was seen, if it was.
    pub fn source_of(&self, iri: &str) -> Option<&str> {
        self.seen.get(iri).map(String::as_str)
    }

    /// How many IRIs under the prefix are held.
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// Whether nothing under the prefix was found.
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    /// How many IRIs were offered altogether, including those outside the prefix.
    pub fn offered(&self) -> usize {
        self.offered
    }

    /// What each source contributed, in name order.
    pub fn sources(&self) -> impl Iterator<Item = (&str, usize)> {
        self.sources
            .iter()
            .map(|(name, count)| (name.as_str(), *count))
    }
}

/// The number a numbered mint went above, and how the vocabulary writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighestInUse {
    /// The number itself.
    pub number: u64,
    /// The IRI carrying it.
    pub iri: String,
    /// How many digits it was written with, so `c_0912` does not become `c_913`.
    pub width: usize,
}

impl HighestInUse {
    /// Whether this vocabulary writes its numbers with leading zeros.
    ///
    /// The width alone does not answer it: `c_12` is two digits and pads nothing, and treating it
    /// as a two-digit convention makes the mint *claim* something about the vocabulary that is not
    /// true. Found by running the command against a store — the report said "written with 2
    /// digits, which is how this vocabulary writes them" of a vocabulary holding `c_1`, `c_3` and
    /// `c_12`, which writes them with one, one, and two.
    pub fn pads(&self) -> bool {
        self.width > self.number.to_string().len()
    }
}

/// How a minted IRI was arrived at. `CLAUDE.md` §3: nothing without a derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MintDerivation {
    /// From a label, through [`slug`].
    FromLabel {
        /// The label as given.
        label: String,
        /// What it reduced to.
        slug: Slug,
    },
    /// From the numbers already in use.
    Numbered {
        /// The number chosen.
        number: u64,
        /// How it was written, in digits.
        width: usize,
        /// The highest number found under the pattern, if any was.
        above: Option<HighestInUse>,
    },
}

/// A minted IRI, and everything that went into it.
///
/// **Nothing about this is reserved.** No store was written, so minting twice in a row returns the
/// same IRI both times. It becomes taken when a change carrying it is staged, and the next mint
/// sees it because [`MintScan`] reads staged changes as well as the vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Minted {
    /// The IRI.
    pub iri: String,
    /// Which policy it was minted under.
    pub policy: MintPolicy,
    /// How it was arrived at.
    pub derivation: MintDerivation,
    /// How many IRIs under the prefix were checked against.
    pub checked: usize,
}

/// Mint an IRI under `pattern`.
///
/// `label` is required by `{slug}` and optional for `{n}`.
pub fn mint(
    pattern: &MintPattern,
    label: Option<&str>,
    bound: SlugBound,
    scan: &MintScan,
) -> Result<Minted, MintError> {
    let (iri, derivation) = match pattern.placeholder() {
        Placeholder::Slug => {
            let Some(label) = label else {
                return Err(MintError::LabelRequired);
            };
            let slug = slug(label, bound)?;
            let iri = pattern.fill(slug.text());
            // Refused, never disambiguated. See the module header: a `-2` suffix is a duplicate
            // concept with a tidier IRI, and `CLAUDE.md` §1.7 says the answer is to reuse what is
            // there or to qualify the term.
            if let Some(found_in) = scan.source_of(&iri) {
                return Err(MintError::Taken {
                    iri,
                    found_in: found_in.to_owned(),
                });
            }
            (
                iri,
                MintDerivation::FromLabel {
                    label: label.to_owned(),
                    slug,
                },
            )
        }
        Placeholder::Number => {
            let above = highest_in_use(pattern, scan);
            let number = match &above {
                Some(highest) => highest.number.checked_add(1).ok_or(MintError::Exhausted {
                    highest: highest.number,
                })?,
                None => 1,
            };
            // Keep the vocabulary's own zero padding, and only when there is some. `c_0912`
            // followed by `c_913` sorts wrongly in every tool that sorts these as strings, which is
            // most of them — but `c_12` is not a padded number and a vocabulary that writes `c_1`
            // is not writing two digits.
            let width = above
                .as_ref()
                .filter(|highest| highest.pads())
                .map_or(1, |highest| highest.width);
            let written = format!("{number:0width$}");
            let iri = pattern.fill(&written);
            // Cannot normally happen — the number is above every number in use — but a pattern
            // whose prefix ends in a digit makes `c_1` and `c_11` overlap in ways worth refusing
            // rather than reasoning about.
            if let Some(found_in) = scan.source_of(&iri) {
                return Err(MintError::Taken {
                    iri,
                    found_in: found_in.to_owned(),
                });
            }
            (
                iri,
                MintDerivation::Numbered {
                    number,
                    width: written.len(),
                    above,
                },
            )
        }
    };

    if !plausible_iri(&iri) {
        return Err(MintError::NotAnIri { iri });
    }

    Ok(Minted {
        iri,
        policy: pattern.policy(),
        derivation,
        checked: scan.len(),
    })
}

/// The largest number written in the placeholder of any IRI the scan holds.
fn highest_in_use(pattern: &MintPattern, scan: &MintScan) -> Option<HighestInUse> {
    let mut best: Option<HighestInUse> = None;
    for iri in scan.seen.keys() {
        let Some(local) = pattern.local_of(iri) else {
            continue;
        };
        if local.is_empty() || !local.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(number) = local.parse::<u64>() else {
            // More than twenty digits. Not a number this build mints, and not a reason to fail:
            // it is simply not one of ours.
            continue;
        };
        if best.as_ref().is_none_or(|held| number > held.number) {
            best = Some(HighestInUse {
                number,
                iri: iri.clone(),
                width: local.len(),
            });
        }
    }
    best
}

/// Nothing could be minted, and this says exactly what stopped it.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MintError {
    /// A `{slug}` pattern with no label.
    #[error("this pattern mints from a label ({{slug}}) and no label was given")]
    LabelRequired,
    /// The label held nothing an IRI can carry.
    #[error(transparent)]
    Unusable(#[from] SlugError),
    /// The IRI is already in use.
    #[error(
        "{iri} is already in use, in {found_in}; \
         a new concept must not take an IRI something else already denotes"
    )]
    Taken {
        /// The IRI that would have been minted.
        iri: String,
        /// Where it was found, in the caller's words. Not named `source`: to `thiserror` that
        /// word means the error underneath this one, and this is a place in a store.
        found_in: String,
    },
    /// The numbering ran out, which takes a vocabulary numbered to `u64::MAX`.
    #[error("the highest number in use is {highest}, and there is no number above it")]
    Exhausted {
        /// The number that could not be exceeded.
        highest: u64,
    },
    /// The filled pattern is not IRI text.
    #[error("{iri:?} is not an IRI, so it will not be minted")]
    NotAnIri {
        /// What would have been minted.
        iri: String,
    },
}

/// What a vocabulary's existing concept IRIs already look like.
///
/// This is the evidence a default pattern is read off. It counts the namespace of every concept
/// IRI — split at the last `#`, or failing that the last `/`, which is the split every RDF tool
/// uses to abbreviate an IRI — and, within the leading namespace, how many local names are a
/// number with an optional fixed prefix.
#[derive(Debug, Clone, Default)]
pub struct IriConvention {
    /// Namespace to how many concepts are in it.
    namespaces: HashMap<String, usize>,
    /// Local names seen per namespace, kept only for the counts below.
    numbered: HashMap<String, BTreeMap<String, usize>>,
    /// How many concept IRIs were read.
    concepts: usize,
    /// How many concepts had no IRI at all — a blank node, which cannot suggest anything.
    without_iri: usize,
}

impl IriConvention {
    /// An empty convention, to push concept IRIs into.
    pub fn new() -> Self {
        IriConvention::default()
    }

    /// Record one concept that is named by an IRI.
    pub fn push(&mut self, iri: &str) {
        self.concepts += 1;
        let Some((namespace, local)) = split_namespace(iri) else {
            return;
        };
        *self.namespaces.entry(namespace.to_owned()).or_default() += 1;
        if let Some(fixed) = numbered_local(local) {
            *self
                .numbered
                .entry(namespace.to_owned())
                .or_default()
                .entry(fixed.to_owned())
                .or_default() += 1;
        }
    }

    /// Record one concept named by a blank node, which no pattern can be read off.
    pub fn push_blank(&mut self) {
        self.concepts += 1;
        self.without_iri += 1;
    }

    /// How many concepts were read.
    pub fn concepts(&self) -> usize {
        self.concepts
    }

    /// How many of them were blank nodes.
    pub fn without_iri(&self) -> usize {
        self.without_iri
    }

    /// Every namespace seen, most concepts first, then by name so the report is stable.
    pub fn namespaces(&self) -> Vec<(&str, usize)> {
        let mut all: Vec<(&str, usize)> = self
            .namespaces
            .iter()
            .map(|(namespace, count)| (namespace.as_str(), *count))
            .collect();
        all.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
        all
    }

    /// The pattern this vocabulary's own concepts justify, or why there is none.
    ///
    /// Deliberately refuses to guess. A vocabulary whose concepts are spread evenly over several
    /// namespaces has no convention to read, and inventing one would produce IRIs that look
    /// official and belong to nothing.
    pub fn suggest(&self) -> Result<Suggestion, NoConvention> {
        let namespaces = self.namespaces();
        let Some(&(namespace, count)) = namespaces.first() else {
            return Err(match self.concepts {
                0 => NoConvention::NoConcepts,
                _ => NoConvention::NoIris {
                    concepts: self.concepts,
                },
            });
        };
        // A leading namespace that is not a majority is a coincidence, not a convention.
        if count * 2 <= self.concepts {
            return Err(NoConvention::NoMajority {
                leading: namespace.to_owned(),
                leading_count: count,
                concepts: self.concepts,
                namespaces: namespaces.len(),
            });
        }

        let numbered = self.numbered.get(namespace);
        let leading_fixed = numbered.and_then(|by_fixed| {
            by_fixed
                .iter()
                .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
                .map(|(fixed, count)| (fixed.clone(), *count))
        });
        let numbered_count: usize = numbered.map_or(0, |by_fixed| by_fixed.values().sum());

        // Majority again, and for the same reason: a handful of numbered local names among
        // hundreds of worded ones is not what the vocabulary does.
        let (placeholder, prefix, evidence) = match leading_fixed {
            Some((fixed, fixed_count)) if numbered_count * 2 > count => (
                Placeholder::Number,
                format!("{namespace}{fixed}"),
                Evidence {
                    namespace: namespace.to_owned(),
                    namespace_count: count,
                    concepts: self.concepts,
                    namespaces: namespaces.len(),
                    numbered: numbered_count,
                    fixed_part: Some((fixed, fixed_count)),
                },
            ),
            _ => (
                Placeholder::Slug,
                namespace.to_owned(),
                Evidence {
                    namespace: namespace.to_owned(),
                    namespace_count: count,
                    concepts: self.concepts,
                    namespaces: namespaces.len(),
                    numbered: numbered_count,
                    fixed_part: None,
                },
            ),
        };

        let pattern = MintPattern::new(&prefix, placeholder, "")
            .map_err(|error| NoConvention::Unusable { prefix, error })?;
        Ok(Suggestion { pattern, evidence })
    }
}

/// A pattern read off the vocabulary, and the counts that justify it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// The pattern.
    pub pattern: MintPattern,
    /// Why.
    pub evidence: Evidence,
}

/// What was counted to arrive at a suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// The namespace most concepts are in.
    pub namespace: String,
    /// How many are in it.
    pub namespace_count: usize,
    /// How many concepts were read altogether.
    pub concepts: usize,
    /// How many distinct namespaces they are spread over.
    pub namespaces: usize,
    /// How many local names in the leading namespace are numbered.
    pub numbered: usize,
    /// The fixed part shared by the numbered local names, and how many carry it.
    pub fixed_part: Option<(String, usize)>,
}

/// The vocabulary does not say what its IRIs should look like.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum NoConvention {
    /// Nothing to read.
    #[error("this vocabulary holds no concepts, so it cannot say what a new IRI should look like")]
    NoConcepts,
    /// Concepts, but none of them named by an IRI.
    #[error("all {concepts} concept(s) here are blank nodes, which name no pattern")]
    NoIris {
        /// How many concepts were read.
        concepts: usize,
    },
    /// No namespace holds most of the concepts.
    #[error(
        "these {concepts} concept(s) are spread over {namespaces} namespaces and none holds most \
         of them — the largest, {leading}, holds {leading_count}; \
         say which to mint in with a pattern"
    )]
    NoMajority {
        /// The namespace with the most concepts.
        leading: String,
        /// How many it holds.
        leading_count: usize,
        /// How many concepts were read.
        concepts: usize,
        /// How many namespaces they are spread over.
        namespaces: usize,
    },
    /// The namespace read off the vocabulary is not something we would mint into.
    #[error("the namespace {prefix:?} read from this vocabulary cannot be a pattern: {error}")]
    Unusable {
        /// What was read.
        prefix: String,
        /// Why it was refused.
        error: PatternError,
    },
}

/// Split an IRI into a namespace and a local name, the way every RDF tool abbreviates one.
///
/// The `#` wins over `/` because a hash IRI's fragment is the local name however many slashes came
/// before it. Returns `None` when there is no separator at all, which a concept IRI in practice
/// never is.
fn split_namespace(iri: &str) -> Option<(&str, &str)> {
    let at = match iri.rfind('#') {
        Some(hash) => hash,
        None => iri.rfind('/')?,
    };
    let (namespace, local) = iri.split_at(at + 1);
    match local.is_empty() {
        true => None,
        false => Some((namespace, local)),
    }
}

/// If `local` is a fixed part followed by digits, the fixed part. `c_1234` gives `c_`, `1234`
/// gives the empty string, and `renewable-energy` gives `None`.
fn numbered_local(local: &str) -> Option<&str> {
    let digits = local.len() - local.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    match digits {
        0 => None,
        _ => Some(&local[..local.len() - digits]),
    }
}

/// `iunreserved` from RFC 3987 §2.2, less the ASCII punctuation that is in it.
///
/// Used to decide what a slug may keep, so the question asked of every character is "could this
/// stand unescaped in an IRI", and the alphanumeric test is applied separately by the caller.
fn iunreserved(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '-' | '.' | '_' | '~')
        || ucschar(character)
}

/// `ucschar` from RFC 3987 §2.2, transcribed range by range.
fn ucschar(character: char) -> bool {
    matches!(character as u32,
        0xA0..=0xD7FF
        | 0xF900..=0xFDCF
        | 0xFDF0..=0xFFEF
        | 0x10000..=0x1FFFD
        | 0x20000..=0x2FFFD
        | 0x30000..=0x3FFFD
        | 0x40000..=0x4FFFD
        | 0x50000..=0x5FFFD
        | 0x60000..=0x6FFFD
        | 0x70000..=0x7FFFD
        | 0x80000..=0x8FFFD
        | 0x90000..=0x9FFFD
        | 0xA0000..=0xAFFFD
        | 0xB0000..=0xBFFFD
        | 0xC0000..=0xCFFFD
        | 0xD0000..=0xDFFFD
        | 0xE1000..=0xEFFFD)
}

/// Whether `character` may stand in an IRI at all.
///
/// The ASCII half of this is RFC 3987's `iunreserved` plus the sub-delimiters and the generic
/// delimiters — which is every printable ASCII character except space and the five RFC 3986 §2
/// excludes outright (`<`, `>`, `"`, `` ` ``, `\`) plus `{`, `}`, `|`, `^`, which RFC 3986 §7.3
/// names as the ones that get corrupted in transit.
fn iri_character(character: char) -> bool {
    if character.is_ascii() {
        return character.is_ascii_graphic()
            && !matches!(
                character,
                '<' | '>' | '"' | '{' | '}' | '|' | '\\' | '^' | '`'
            );
    }
    ucschar(character) || matches!(character as u32, 0xE000..=0xF8FF)
}

/// The scheme of an absolute IRI: `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." ) ":"`, RFC 3986 §3.1.
fn scheme_of(iri: &str) -> Option<&str> {
    let colon = iri.find(':')?;
    let scheme = &iri[..colon];
    let mut characters = scheme.chars();
    if !characters.next()?.is_ascii_alphabetic() {
        return None;
    }
    characters
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        .then_some(scheme)
}

/// A deliberately partial IRI check: absolute, and made only of characters an IRI may carry.
///
/// It is **not** the RFC 3987 grammar. It cannot be, in a crate that will not depend on an engine
/// — and it does not have to be, because everything this module mints is built from a pattern that
/// was checked the same way plus a slug built only of `iunreserved` characters. The caller that
/// puts a minted IRI in front of a user is expected to check it against the parser that will
/// actually store it; `openbiz-server` does. The gap is recorded in `docs/UNTESTED.md`.
fn plausible_iri(iri: &str) -> bool {
    scheme_of(iri).is_some() && iri.chars().all(iri_character)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(prefix: &str, iris: &[&str]) -> MintScan {
        let mut scan = MintScan::under(prefix);
        for iri in iris {
            scan.push(iri, "the vocabulary");
        }
        scan
    }

    /// A pattern with nothing to fill in mints the same IRI every time, which is the silent merge
    /// this module exists to prevent. Refused at the pattern, not discovered at the second mint.
    #[test]
    fn a_pattern_with_no_placeholder_is_refused() {
        let error = MintPattern::parse("https://example.org/thesaurus/concept")
            .expect_err("no placeholder");

        assert!(
            matches!(error, PatternError::NoPlaceholder { .. }),
            "{error}"
        );
        assert!(
            error.to_string().contains("the same IRI every time"),
            "{error}"
        );
    }

    /// `{slug}-{n}` is the disambiguating suffix under another name, and this build refuses that
    /// deliberately rather than by accident.
    #[test]
    fn a_pattern_with_two_placeholders_is_refused() {
        let error =
            MintPattern::parse("https://example.org/t/{slug}-{n}").expect_err("two placeholders");

        assert!(
            matches!(error, PatternError::TwoPlaceholders { .. }),
            "{error}"
        );
    }

    #[test]
    fn an_unknown_placeholder_names_the_ones_that_exist() {
        let error = MintPattern::parse("https://example.org/t/{uuid}").expect_err("no such thing");

        assert!(
            error.to_string().contains("{n}") && error.to_string().contains("{slug}"),
            "{error}"
        );
    }

    /// A concept IRI is what every other system holds, so a relative one is not a partial answer.
    #[test]
    fn a_relative_pattern_is_refused() {
        let error = MintPattern::parse("/thesaurus/{slug}").expect_err("relative");

        assert!(matches!(error, PatternError::NoScheme { .. }), "{error}");
    }

    #[test]
    fn a_pattern_holding_a_character_an_iri_cannot_carry_is_refused() {
        let error =
            MintPattern::parse("https://example.org/my thesaurus/{slug}").expect_err("a space");

        assert!(
            matches!(error, PatternError::NotIriText { character: ' ', .. }),
            "{error}"
        );
    }

    /// RFC 3987 §2.2 puts the whole of assigned Unicode in `ucschar`, so `Ökologie` is already a
    /// legal IRI local name. Transliterating it to `okologie` would be a language-specific guess
    /// that manufactures collisions between words that are not the same word.
    #[test]
    fn a_slug_keeps_unicode_rather_than_transliterating_it() {
        assert_eq!(
            slug("Ökologie", SlugBound::DEFAULT)
                .expect("a usable label")
                .text(),
            "ökologie"
        );
        assert_eq!(
            slug("Straße", SlugBound::DEFAULT)
                .expect("a usable label")
                .text(),
            "straße"
        );
        assert_eq!(
            slug("生物多様性", SlugBound::DEFAULT)
                .expect("a usable label")
                .text(),
            "生物多様性"
        );
    }

    #[test]
    fn a_slug_joins_words_and_elides_apostrophes() {
        assert_eq!(
            slug("Renewable energy sources", SlugBound::DEFAULT)
                .expect("a usable label")
                .text(),
            "renewable-energy-sources"
        );
        assert_eq!(
            slug("Müller's cheese", SlugBound::DEFAULT)
                .expect("a usable label")
                .text(),
            "müllers-cheese"
        );
        // A slash separates rather than disappearing: `bagsack` is a word nobody wrote.
        assert_eq!(
            slug("Bags / sacks", SlugBound::DEFAULT)
                .expect("a usable label")
                .text(),
            "bags-sacks"
        );
    }

    /// An emoji is inside `ucschar` and would make a legal IRI, and it is still not a local name.
    /// A label made only of such characters cannot mint a readable IRI at all, and says so instead
    /// of minting the bare namespace.
    #[test]
    fn a_label_with_nothing_an_iri_can_carry_is_refused() {
        let error = slug("🎉 !!! ?", SlugBound::DEFAULT).expect_err("nothing usable");

        assert!(matches!(error, SlugError::NothingUsable { .. }), "{error}");
        assert!(
            error.to_string().contains("{n}"),
            "the way out is named: {error}"
        );
    }

    #[test]
    fn a_long_label_is_cut_at_a_word_boundary_and_says_so() {
        let long = "Policies concerning the management of surface water drainage in urban areas";
        let cut = slug(long, SlugBound { max_chars: 24 }).expect("a usable label");

        assert!(cut.truncated(), "{cut:?}");
        assert_eq!(cut.text(), "policies-concerning-the");
    }

    /// The mint the whole module exists for: nothing in use, so the first number is 1.
    #[test]
    fn an_empty_namespace_mints_the_first_number() {
        let pattern = MintPattern::parse("https://example.org/t/c_{n}").expect("a pattern");
        let minted = mint(
            &pattern,
            None,
            SlugBound::DEFAULT,
            &scan("https://example.org/t/c_", &[]),
        )
        .expect("a mint");

        assert_eq!(minted.iri, "https://example.org/t/c_1");
        assert_eq!(minted.policy, MintPolicy::Opaque);
    }

    /// **The rule a gap-filling minter gets wrong.** `c_2` is free because something was there;
    /// an IRI that has been used must never come back attached to a different concept.
    #[test]
    fn a_numbered_mint_goes_above_the_highest_and_never_fills_a_gap() {
        let pattern = MintPattern::parse("https://example.org/t/c_{n}").expect("a pattern");
        let held = scan(
            "https://example.org/t/c_",
            &[
                "https://example.org/t/c_1",
                "https://example.org/t/c_3",
                "https://example.org/t/c_12",
            ],
        );
        let minted = mint(&pattern, None, SlugBound::DEFAULT, &held).expect("a mint");

        assert_eq!(minted.iri, "https://example.org/t/c_13");
        let MintDerivation::Numbered {
            above: Some(above), ..
        } = &minted.derivation
        else {
            panic!("the highest in use is the derivation: {minted:?}");
        };
        assert_eq!(above.number, 12);
        assert_eq!(above.iri, "https://example.org/t/c_12");
    }

    /// `c_0912` followed by `c_913` sorts wrongly in every tool that sorts these as strings.
    #[test]
    fn zero_padding_is_kept() {
        let pattern = MintPattern::parse("https://example.org/t/c_{n}").expect("a pattern");
        let held = scan(
            "https://example.org/t/c_",
            &["https://example.org/t/c_0912"],
        );

        assert_eq!(
            mint(&pattern, None, SlugBound::DEFAULT, &held)
                .expect("a mint")
                .iri,
            "https://example.org/t/c_0913"
        );
    }

    /// **The §1.7 rule.** `renewable-energy-2` is a duplicate concept with a tidier IRI, so a
    /// worded collision is refused and names what holds the IRI.
    #[test]
    fn a_worded_collision_is_refused_rather_than_suffixed() {
        let pattern = MintPattern::parse("https://example.org/t/{slug}").expect("a pattern");
        let mut held = MintScan::under("https://example.org/t/");
        held.push("https://example.org/t/renewable-energy", "candidate 7");

        let error = mint(
            &pattern,
            Some("Renewable energy"),
            SlugBound::DEFAULT,
            &held,
        )
        .expect_err("taken");

        assert!(matches!(error, MintError::Taken { .. }), "{error}");
        assert!(error.to_string().contains("candidate 7"), "{error}");
        assert!(
            !error.to_string().contains("-2"),
            "no suffix is offered: {error}"
        );
    }

    #[test]
    fn a_slug_pattern_with_no_label_is_refused() {
        let pattern = MintPattern::parse("https://example.org/t/{slug}").expect("a pattern");
        let error = mint(
            &pattern,
            None,
            SlugBound::DEFAULT,
            &MintScan::under("https://example.org/t/"),
        )
        .expect_err("no label");

        assert!(matches!(error, MintError::LabelRequired), "{error}");
    }

    /// The scan holds only what could collide, which is what keeps it proportional to the
    /// namespace rather than to the store.
    #[test]
    fn a_scan_keeps_only_what_could_collide() {
        let mut held = MintScan::under("https://example.org/t/");
        held.push("https://example.org/t/c_1", "the vocabulary");
        held.push("https://elsewhere.example/other", "another vocabulary");
        held.push(
            "http://www.w3.org/2004/02/skos/core#Concept",
            "the vocabulary",
        );

        assert_eq!(held.len(), 1);
        assert_eq!(held.offered(), 3);
        assert_eq!(
            held.source_of("https://example.org/t/c_1"),
            Some("the vocabulary")
        );
    }

    /// The first source to mention an IRI is the one reported, so a collision with the vocabulary
    /// is never described as a collision with a staged change that merely repeats it.
    #[test]
    fn the_first_source_to_mention_an_iri_is_the_one_reported() {
        let mut held = MintScan::under("https://example.org/t/");
        held.push("https://example.org/t/c_1", "the vocabulary");
        held.push("https://example.org/t/c_1", "candidate 7");

        assert_eq!(
            held.source_of("https://example.org/t/c_1"),
            Some("the vocabulary")
        );
        assert_eq!(held.len(), 1);
    }

    #[test]
    fn a_numbered_vocabulary_suggests_its_own_shape() {
        let mut convention = IriConvention::new();
        for iri in [
            "https://example.org/t/c_1",
            "https://example.org/t/c_2",
            "https://example.org/t/c_30",
        ] {
            convention.push(iri);
        }

        let suggestion = convention.suggest().expect("a convention");
        assert_eq!(
            suggestion.pattern.to_string(),
            "https://example.org/t/c_{n}"
        );
        assert_eq!(suggestion.evidence.numbered, 3);
        assert_eq!(suggestion.evidence.fixed_part, Some(("c_".to_owned(), 3)));
    }

    #[test]
    fn a_worded_vocabulary_suggests_a_readable_pattern() {
        let mut convention = IriConvention::new();
        for iri in [
            "https://example.org/t/renewable-energy",
            "https://example.org/t/solar-power",
            "https://example.org/t/wind-power",
        ] {
            convention.push(iri);
        }

        let suggestion = convention.suggest().expect("a convention");
        assert_eq!(
            suggestion.pattern.to_string(),
            "https://example.org/t/{slug}"
        );
        assert_eq!(suggestion.pattern.policy(), MintPolicy::Readable);
    }

    /// A hash namespace splits at the `#`, however many slashes came before it.
    #[test]
    fn a_hash_namespace_splits_at_the_hash() {
        let mut convention = IriConvention::new();
        convention.push("https://example.org/t/vocab#renewable-energy");
        convention.push("https://example.org/t/vocab#solar-power");

        let suggestion = convention.suggest().expect("a convention");
        assert_eq!(
            suggestion.pattern.to_string(),
            "https://example.org/t/vocab#{slug}"
        );
    }

    /// **The refusal that matters.** Concepts spread evenly over namespaces have no convention to
    /// read, and a confident guess would mint official-looking IRIs belonging to nothing.
    #[test]
    fn no_majority_namespace_yields_no_suggestion() {
        let mut convention = IriConvention::new();
        convention.push("https://a.example/t/one");
        convention.push("https://b.example/t/two");
        convention.push("https://c.example/t/three");

        let error = convention.suggest().expect_err("no convention");
        assert!(matches!(error, NoConvention::NoMajority { .. }), "{error}");
        assert!(error.to_string().contains("3 namespaces"), "{error}");
    }

    #[test]
    fn an_empty_vocabulary_yields_no_suggestion() {
        assert!(matches!(
            IriConvention::new().suggest().expect_err("nothing to read"),
            NoConvention::NoConcepts
        ));
    }

    /// A vocabulary whose concepts are all blank nodes is a different answer from an empty one:
    /// there are concepts, and none of them can say what an IRI here looks like.
    #[test]
    fn blank_nodes_are_counted_apart_from_an_empty_vocabulary() {
        let mut convention = IriConvention::new();
        convention.push_blank();
        convention.push_blank();

        let error = convention.suggest().expect_err("nothing to read");
        assert!(
            matches!(error, NoConvention::NoIris { concepts: 2 }),
            "{error}"
        );
        assert_eq!(convention.without_iri(), 2);
    }

    /// The `ucschar` ranges are transcribed from RFC 3987 §2.2, so the boundaries are worth
    /// pinning: U+009F is below the first range and U+00A0 opens it.
    #[test]
    fn the_ucschar_boundaries_are_the_ones_rfc_3987_states() {
        assert!(!ucschar('\u{9F}'));
        assert!(ucschar('\u{A0}'));
        assert!(ucschar('\u{D7FF}'));
        assert!(!ucschar('\u{E000}'));
        assert!(ucschar('\u{10000}'));
        assert!(!ucschar('\u{1FFFE}'));
    }

    #[test]
    fn a_minted_iri_is_absolute_and_carries_only_iri_characters() {
        let pattern = MintPattern::parse("https://example.org/t/{slug}").expect("a pattern");
        let minted = mint(
            &pattern,
            Some("Ökologie und Umwelt"),
            SlugBound::DEFAULT,
            &MintScan::under("https://example.org/t/"),
        )
        .expect("a mint");

        assert_eq!(minted.iri, "https://example.org/t/ökologie-und-umwelt");
        assert!(plausible_iri(&minted.iri), "{minted:?}");
    }
}

#[cfg(test)]
mod padding_tests {
    use super::*;

    fn scan(prefix: &str, iris: &[&str]) -> MintScan {
        let mut scan = MintScan::under(prefix);
        for iri in iris {
            scan.push(iri, "the vocabulary");
        }
        scan
    }

    /// **The defect running the command found.** A vocabulary holding `c_1`, `c_3` and `c_12`
    /// writes its numbers with one, one, and two digits and pads none of them, and the report said
    /// "written with 2 digits, which is how this vocabulary writes them". The width of the highest
    /// number is not evidence of a convention; a leading zero is.
    #[test]
    fn a_two_digit_number_is_not_a_two_digit_convention() {
        let pattern = MintPattern::parse("https://example.org/t/c_{n}").expect("a pattern");
        let held = scan(
            "https://example.org/t/c_",
            &[
                "https://example.org/t/c_1",
                "https://example.org/t/c_3",
                "https://example.org/t/c_12",
            ],
        );
        let minted = mint(&pattern, None, SlugBound::DEFAULT, &held).expect("a mint");

        let MintDerivation::Numbered {
            above: Some(above), ..
        } = &minted.derivation
        else {
            panic!("a numbered mint: {minted:?}");
        };
        assert!(!above.pads(), "c_12 pads nothing: {above:?}");
    }

    /// The other half: a vocabulary that really does pad says so, and the mint keeps the padding.
    #[test]
    fn a_padded_vocabulary_pads_and_says_so() {
        let pattern = MintPattern::parse("https://example.org/t/c_{n}").expect("a pattern");
        let held = scan(
            "https://example.org/t/c_",
            &["https://example.org/t/c_0912"],
        );
        let minted = mint(&pattern, None, SlugBound::DEFAULT, &held).expect("a mint");

        assert_eq!(minted.iri, "https://example.org/t/c_0913");
        let MintDerivation::Numbered {
            above: Some(above), ..
        } = &minted.derivation
        else {
            panic!("a numbered mint: {minted:?}");
        };
        assert!(above.pads(), "{above:?}");
    }

    /// A number that overflows its padding keeps every digit rather than being cut to fit.
    #[test]
    fn a_number_that_outgrows_its_padding_keeps_its_digits() {
        let pattern = MintPattern::parse("https://example.org/t/c_{n}").expect("a pattern");
        let held = scan("https://example.org/t/c_", &["https://example.org/t/c_099"]);

        assert_eq!(
            mint(&pattern, None, SlugBound::DEFAULT, &held)
                .expect("a mint")
                .iri,
            "https://example.org/t/c_100"
        );
    }
}
