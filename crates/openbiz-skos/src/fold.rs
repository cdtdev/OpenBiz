//! The one place a label is reduced to the form matching compares — Unicode caseless matching.
//!
//! Everything that asks "is this the same term?" over a label's text goes through [`fold`]. That
//! is a deliberate single seam and not a convenience: matching that is done in two places drifts,
//! and the way it drifts is that one of them keeps finding a concept the other has stopped
//! finding. `CLAUDE.md` §1.7 makes that failure expensive — a miss on the creation path is not an
//! empty result the user retries, it is a duplicate concept.
//!
//! # What the Unicode Standard calls this
//!
//! §3.13 *Default Case Algorithms* defines caseless matching in terms of **case folding**, which
//! is a different operation from the lowercasing this used to do:
//!
//! > Case folding is primarily used for caseless comparison of text, such as identifiers in a
//! > computer program, rather than actual text transformation. Case folding in Unicode is based on
//! > the simple case mappings […] but includes additional changes to the source text to help make
//! > it more useful for caseless matching.
//!
//! The concrete difference is the one German authoring conventions hit constantly: `ß` *lowercases
//! to itself*, so `str::to_lowercase` leaves `Straße` alone and a search for `STRASSE` finds
//! nothing. Full case folding maps it to `ss` and the search succeeds. The same shape appears in
//! Greek, where a word-final `Σ` lowercases to `ς` but folds to `σ`, so `οδόσ` and `οδός` are the
//! same term to a matcher and two different strings to a lowercaser.
//!
//! The definition this implements is **D145, canonical caseless match**:
//!
//! > X and Y are canonical caseless equivalents if and only if
//! > NFD(toCasefold(NFD(X))) = NFD(toCasefold(NFD(Y)))
//!
//! Hence the shape of [`fold`]: decompose, fold, decompose again.
//!
//! **The outer NFD is kept because it is the definition, not because it is proven necessary.**
//! Iteration 60 searched for an input where dropping it changes the answer — every one of the
//! 1,112,064 Unicode scalar values alone, and each of them followed by a combining mark drawn
//! from ten combining classes, 11M sequences in all — and found none. The standard's own note
//! says normalisation is not required *before* folding except for U+0345 and the characters that
//! decompose to it, which is what the inner NFD is for; it does not say the outer one is
//! redundant, and the search was over one- and two-character sequences rather than a proof. So
//! the code implements D145 as written and `docs/UNTESTED.md` records that one of its three
//! passes has no test that fails without it.
//!
//! Normalisation is the other half of the same user-visible problem. A composed `é` (U+00E9) and a
//! decomposed `e` + U+0301 render identically on screen, are different strings to `==`, and both
//! occur in real multilingual thesauri — iteration 37 measured AGROVOC at 1.25M SKOS-XL labels, so
//! corpora of exactly this kind are in scope.
//!
//! # What this deliberately does *not* do
//!
//! **It does not strip accents.** `ecole` still does not find `École`. Removing diacritics is not
//! a Unicode operation but a language-specific editorial guess — the same guess `slug` refuses to
//! make, for the same reason: German and Swedish disagree about `ö`, and folding the difference
//! away manufactures matches between terms that are not the same word. What is fixed here is the
//! case where the two strings *are* the same word and only their encoding differs.
//!
//! **It is canonical, not compatibility, folding.** `compatibility_caseless_match` (D146) would
//! also equate a full-width `Ａ` with `A`, and — much less desirably — `2⁵` with `25`. Canonical
//! equivalence is the relation that means "the same characters"; compatibility equivalence means
//! "related characters", and a thesaurus is a place where a superscript can be the distinction
//! that matters.
//!
//! One consequence is worth naming because it is not obvious from the definition: the standard's
//! case folding table *does* map the `ﬁ` ligature (U+FB01) to `fi`, so that much of what looks
//! like compatibility folding happens anyway. That is the standard's choice, not ours.
//!
//! # The folded form is not for display, and holds no offsets
//!
//! Folding is not length-preserving in either direction — one character can fold to three — so an
//! index into a folded string is not an index into the label an author wrote. A caller that
//! highlighted a match using such an offset would highlight the wrong characters on exactly the
//! labels that most need care. Nothing here exposes an offset, and
//! [`LabelQuery`](crate::LabelQuery) reports *how* a label matched rather than where.

use caseless::Caseless;
use unicode_normalization::UnicodeNormalization;

/// Reduce `text` to the form caseless matching compares — Unicode §3.13's canonical caseless form.
///
/// Two strings fold to the same value exactly when they are canonical caseless equivalents (D145).
/// The result is for comparison only: it is not a display form, and no index into it is an index
/// into `text`.
///
/// ```
/// # use openbiz_skos::fold;
/// // Case folding, which lowercasing is not: `ß` lowercases to itself.
/// assert_eq!(fold("Straße"), fold("STRASSE"));
/// // Normalisation: one code point and two, rendering identically.
/// assert_eq!(fold("\u{c9}cole"), fold("E\u{301}cole"));
/// // And what it does not do: an accent is a difference, not a decoration.
/// assert_ne!(fold("École"), fold("Ecole"));
/// ```
pub fn fold(text: &str) -> String {
    // ASCII is already in NFD, and the only folding the standard defines over it is A–Z to a–z,
    // so the general path provably cannot do anything else here — `ascii_folds_exactly_as_the
    // _general_path_does` checks that character by character rather than taking it on trust.
    // Worth having: an enterprise vocabulary's labels are overwhelmingly ASCII, and every search
    // folds every label it reads.
    if text.is_ascii() {
        return text.to_ascii_lowercase();
    }
    canonical_caseless(text)
}

/// D145's normal form, computed the long way, for every input the fast path does not take.
///
/// One `collect`: `nfd`, `default_case_fold` and the outer `nfd` are all lazy over `char`, so the
/// three passes the definition describes cost one allocation rather than three. The outer `nfd`
/// is the pass no test distinguishes — see the module docs and `docs/UNTESTED.md`.
fn canonical_caseless(text: &str) -> String {
    text.nfd().default_case_fold().nfd().collect()
}

#[cfg(test)]
mod tests {
    use super::{canonical_caseless, fold};

    /// The fast path is an optimisation and must be indistinguishable from the thing it skips.
    /// Checked over the whole of ASCII rather than over a handful of letters, because the claim
    /// being made is about the range and not about the examples.
    #[test]
    fn ascii_folds_exactly_as_the_general_path_does() {
        for byte in 0u8..=127 {
            let character = byte as char;
            let text = character.to_string();
            assert_eq!(
                fold(&text),
                canonical_caseless(&text),
                "U+{:04X} takes the ASCII fast path to a different answer",
                byte
            );
        }
        // And in combination, so the check is not only over single characters.
        for text in ["Bank", "BANK CODE", "van der Waals", "ISO-25964", "a1_b2"] {
            assert_eq!(fold(text), canonical_caseless(text), "{text}");
        }
    }

    /// §3.13's own worked distinction between lowercasing and folding. Each of these is a term a
    /// German or Greek cataloguer would plausibly type, and each was a miss before folding.
    #[test]
    fn folding_equates_what_lowercasing_leaves_apart() {
        // `ß` lowercases to itself; it folds to `ss`.
        assert_eq!(fold("Straße"), fold("STRASSE"));
        assert_eq!(fold("Straße"), fold("strasse"));
        assert_eq!(fold("Straße"), "strasse");
        // A word-final sigma lowercases to `ς` and folds to `σ`, so the two spellings of the same
        // Greek word meet only under folding.
        assert_eq!(fold("ΟΔΌΣ"), fold("οδόσ"));
        assert_eq!(fold("ΟΔΌΣ"), fold("οδός"));
        // Titlecase, which is a third case and not a variant of the other two.
        assert_eq!(fold("ǅungla"), fold("ǄUNGLA"));
        assert_eq!(fold("ǅungla"), fold("ǆungla"));
    }

    /// The normalisation half: canonically equivalent spellings are the same term.
    #[test]
    fn canonically_equivalent_spellings_fold_together() {
        // Composed and decomposed acute.
        assert_eq!(fold("\u{c9}cole"), fold("E\u{301}cole"));
        // A character with two combining marks, where the order of the marks differs. NFD
        // reorders by combining class, so these are equivalent and a naive comparison misses it.
        assert_eq!(fold("q\u{307}\u{323}"), fold("q\u{323}\u{307}"));
        // The angstrom sign and A-with-ring-above are canonically equivalent by decomposition.
        assert_eq!(fold("\u{212b}ngström"), fold("Ångström"));
    }

    /// The line we do not cross, pinned so that "matching got cleverer" cannot happen silently.
    /// Each of these is a *deliberate* non-match, and each would be a false positive that merged
    /// two distinct terms.
    #[test]
    fn folding_is_not_transliteration_stemming_or_compatibility() {
        // An accent is part of the word, not a decoration on it.
        assert_ne!(fold("École"), fold("Ecole"));
        assert_ne!(fold("Ökologie"), fold("Okologie"));
        // No stemming and no spelling correction.
        assert_ne!(fold("banks"), fold("bank"));
        assert_ne!(fold("colour"), fold("color"));
        // Compatibility equivalence (D146) is not applied: a superscript can be the whole
        // distinction in a scientific thesaurus.
        assert_ne!(fold("m\u{b2}"), fold("m2"));
        assert_ne!(fold("\u{ff21}"), fold("A")); // full-width A
    }

    /// The one case folding does that *looks* like compatibility folding, recorded because a
    /// reader who has just read the test above will otherwise think it is a bug. CaseFolding.txt
    /// gives U+FB01 a full (`F`) mapping; this is the standard's decision, not ours.
    #[test]
    fn the_standards_own_table_decomposes_the_fi_ligature() {
        assert_eq!(fold("\u{fb01}le"), fold("file"));
    }

    /// The empty string and lone combining marks are inputs a query builder can produce and a
    /// label can hold; folding must not panic or invent characters.
    #[test]
    fn degenerate_input_folds_to_itself() {
        assert_eq!(fold(""), "");
        assert_eq!(fold("\u{301}"), "\u{301}");
        assert_eq!(fold("   "), "   ");
        // Unpaired-looking sequences and non-characters still round-trip as text.
        assert_eq!(fold("\u{200b}"), "\u{200b}"); // zero-width space
    }

    /// What folding costs against the lowercasing it replaced, because every search folds every
    /// label it reads and `docs/UNTESTED.md` already records that search is a linear scan.
    /// `#[ignore]`d: it is a measurement, not an assertion, and the suite should not fail on a
    /// loaded machine. Run with `cargo test -p openbiz-skos -- --ignored --nocapture fold_cost`.
    #[test]
    #[ignore = "a measurement, not an assertion"]
    fn fold_cost_against_lowercasing() {
        use std::time::Instant;

        const ROUNDS: usize = 200_000;
        let corpora = [
            ("ascii", "Renewable energy generation"),
            ("latin-1 accents", "Génération d'énergie renouvelable"),
            ("greek", "Παραγωγή ανανεώσιμης ενέργειας"),
        ];

        for (name, label) in corpora {
            let started = Instant::now();
            let mut sink = 0usize;
            for _ in 0..ROUNDS {
                sink += fold(label).len();
            }
            let folding = started.elapsed();

            let started = Instant::now();
            let mut other = 0usize;
            for _ in 0..ROUNDS {
                other += label.to_lowercase().len();
            }
            let lowercasing = started.elapsed();

            println!(
                "{name:<16} fold {:>8.1} ns/label   lowercase {:>8.1} ns/label   ratio {:.2}x  \
                 (sinks {sink}/{other})",
                folding.as_nanos() as f64 / ROUNDS as f64,
                lowercasing.as_nanos() as f64 / ROUNDS as f64,
                folding.as_secs_f64() / lowercasing.as_secs_f64(),
            );
        }
    }

    /// **The seam, enforced rather than intended.** A second place that folds — or worse, one that
    /// normalises without folding — is not a compile error and not a test failure; it is a matcher
    /// that quietly disagrees with this one about whether two labels are the same term. So the
    /// crate's own source is read and the Unicode libraries are required to appear in this file
    /// alone.
    ///
    /// This is the guard `docs/UNTESTED.md` proposed for the wall-clock seam and it is written
    /// here for the same reason: the defect class has already shipped once, as two spellings of
    /// case-insensitivity (`to_lowercase` for matching, `to_ascii_lowercase` for language tags)
    /// that were each right in their own place and would have been wrong in the other.
    #[test]
    fn nothing_outside_this_module_reaches_for_unicode_case_or_normalisation() {
        let source_directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        // Every way of naming the two crates' operations that would compile.
        let reaches = [
            "caseless::",
            "unicode_normalization",
            "default_case_fold",
            ".nfd()",
            ".nfc()",
            ".nfkd()",
            ".nfkc()",
        ];

        let mut offenders = Vec::new();
        let entries = std::fs::read_dir(&source_directory).expect("the crate's own src directory");
        for entry in entries {
            let path = entry.expect("a readable directory entry").path();
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            if path.file_name().is_some_and(|name| name == "fold.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a readable source file");
            for needle in reaches {
                if text.contains(needle) {
                    offenders.push(format!("{} uses {needle}", path.display()));
                }
            }
        }

        assert!(
            offenders.is_empty(),
            "case folding and normalisation belong in fold.rs, so that one answer to \"is this the \
             same term?\" exists rather than two that drift: {offenders:?}"
        );
    }

    /// Folding is idempotent — the folded form is a normal form, so folding it again changes
    /// nothing. Not decorative: `search` folds a query once and every label once, and if that were
    /// untrue a folded query compared against a twice-folded label would silently stop matching.
    #[test]
    fn folding_is_idempotent() {
        for text in [
            "Straße",
            "ΟΔΌΣ",
            "E\u{301}cole",
            "\u{c9}cole",
            "\u{fb01}le",
            "İstanbul",
            "ǅungla",
            "Bank",
        ] {
            let once = fold(text);
            assert_eq!(fold(&once), once, "{text}");
        }
    }
}
