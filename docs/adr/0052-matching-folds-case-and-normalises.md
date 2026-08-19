# 0052 — Label matching folds case and normalises, and stops there

**Date:** 2026-08-20 (NZST, UTC+12)
**Status:** accepted
**Implements:** `CLAUDE.md` §1.7 (reuse outranks creation), §2 (standards-first), §5 (licence
policy), §1.5 (the dependency budget). Closes two `docs/UNTESTED.md` entries opened at iterations
38 and 54.

## Context

Two gaps had been recorded against label matching for twenty-two iterations, both with the same
shape and the same consequence:

- **No case folding.** `str::to_lowercase` is a case *mapping*. `ß` lowercases to itself, so a
  search for `STRASSE` did not find `Straße` — and `ss` is the ASCII convention German authoring
  uses, so it is the *likelier* spelling to be typed. In Greek a word-final `Σ` lowercases to `ς`
  and a medial one to `σ`, so `οδόσ` did not find `ΟΔΌΣ`.
- **No normalisation.** A composed `é` (U+00E9) and a decomposed `e` + U+0301 render identically
  and are different strings. Neither the cataloguer who stored one nor the curator who typed the
  other can see which form their editor produced, so this miss is undiagnosable from outside the
  program.

Both were pinned by tests that asserted the miss, so neither could go stale silently. What made
them worth taking now rather than later is where the matching is *used*. `openbiz search` and
`openbiz mint`'s discovery pass share one matcher, and on the creation path a miss is not an empty
result the user retries — it is the report saying "nothing discovery reached is called this",
followed by a freshly minted IRI. That is `CLAUDE.md` §1.7's silo-generating failure exactly, and
the anti-silo feature was producing it.

Both entries recorded the same blocker: *"a dependency decision, which is why it was not taken
inside a feature"*. Taking that decision is this ADR.

## Decision 1 — `caseless` + `unicode-normalization`, and not ICU4X

Rust's standard library has no case folding, so this is a dependency or it is a hand-rolled table.

**Rejected: hand-rolling.** A case-folding table is 1,500-odd mappings that change with each
Unicode release. Copying one into the tree is taking on a maintenance burden in exchange for a
manifest line.

**Rejected: ICU4X (`icu_casemap` + `icu_normalizer`).** It is the standards-native answer —
maintained by the Unicode Consortium itself, and `Unicode-3.0` is already on our allow list. It was
rejected on §1.5. It requires Rust 1.88 and brings `icu_provider`, `icu_collections`, `icu_properties`,
`zerovec`, `potential_utf` and baked data crates for one string operation. Worth revisiting if we
ever need collation, locale-tailored casing, or segmentation — at which point ICU4X earns its
weight and this decision should be reopened rather than defended.

**Chosen: `caseless` 0.2.2 (MIT) and `unicode-normalization` 0.1.25 (MIT OR Apache-2.0).** Both
from `unicode-rs`. What was measured:

- **Weight: four crates total** — `caseless`, `unicode-normalization`, `tinyvec`, `tinyvec_macros`.
  `cargo tree -p openbiz-skos` confirms nothing else arrives.
- **Licences: both already permitted**, so `deny.toml` is unchanged. `cargo deny check licenses`
  passes without an allow-list widening — which per §5 is the branch that needs no commercial
  decision.
- **Data currency: CaseFolding-16.0.0**, dated 2024-04-30, shipped in the crate and checkable
  (`caseless::UNICODE_VERSION` is `(16, 0, 0)`). One Unicode release behind current. A stale fold
  table was the specific risk in a crate last released at 0.2.x, and it is not stale.

The residual risk is maintenance, not correctness: `caseless` is a small crate at 0.2.2. It is
mitigated by Decision 2 — the whole of it is used from one function, so replacing it is an edit to
one file.

## Decision 2 — one seam, enforced by a test that reads our own source

`openbiz_skos::fold` is the only place either crate is called. `CLAUDE.md` §3 puts third-party
*engines* behind our own traits; a text library is not an engine, and a trait with one
implementation and no prospect of a second would be ceremony. A single function is the honest form
of the same rule.

But a convention is what the wall-clock seam already is, and `UNTESTED.md` records that its
enforcement is "a convention, not an enforced rule" — the next call site written elsewhere compiles
and passes clippy. So this seam is enforced: `nothing_outside_this_module_reaches_for_unicode_case_
or_normalisation` reads the crate's own `src` directory and fails if `caseless::`,
`unicode_normalization`, `default_case_fold` or an `.nfd()`/`.nfc()`/`.nfkd()`/`.nfkc()` call
appears in any file but `fold.rs`.

That guard is not hypothetical hygiene. This crate already shipped two spellings of
case-insensitivity that were each correct in their own place — `to_lowercase` for matching,
`to_ascii_lowercase` for BCP 47 language tags, which are ASCII by definition — and a third one
introduced casually is a matcher that disagrees with the first about whether two labels are the
same term.

## Decision 3 — canonical caseless (D145), not compatibility (D146)

The implemented definition is the Unicode Standard §3.13, D145:

> NFD(toCasefold(NFD(X))) = NFD(toCasefold(NFD(Y)))

Compatibility caseless matching would additionally equate a full-width `Ａ` with `A` and — much
worse — `m²` with `m2`. A thesaurus is precisely a place where a superscript can be the whole
distinction, so canonical equivalence ("the same characters") is right and compatibility
equivalence ("related characters") is not.

One consequence is worth naming because it looks like a contradiction: the standard's own folding
table gives the `ﬁ` ligature (U+FB01) a full mapping to `fi`, so some of what looks like
compatibility folding happens regardless. That is the standard's decision, and there is a test
saying so, so the next reader does not file it as a bug.

**The outer NFD is kept because it is the definition, and it is not proven necessary.** A search
was run for an input where dropping it changes the answer: all 1,112,064 Unicode scalar values
alone, and each followed by a combining mark drawn from ten combining classes — 11M sequences.
**Zero counterexamples.** A mutation that deletes that pass survives the whole suite. The code
implements D145 as written rather than as reduced by a bounded search, and `UNTESTED.md` records
that one of the three passes has no test that fails without it. Deleting a specification's step
because a two-character search could not distinguish it is how subtle bugs are introduced.

## Decision 4 — accents are a difference, and this is where matching stops

`ecole` still does not find `École`, `okologie` does not find `Ökologie`, and `color` does not find
`colour`. Each is pinned by a test that asserts the *non*-match.

Stripping diacritics is not a Unicode operation; it is a language-specific editorial guess. `slug`
already refuses to make it, and for the same reason: German and Swedish disagree about `ö`, and
folding the difference away manufactures matches between terms that are not the same word. On the
creation path a false positive is worse than the false negative it replaces — a STOP that says
`Energie` and `Énergie` are the same term invites a merge nobody asked for, and merges are hard to
undo. There is a test asserting that `Energie` does *not* stop the mint.

Spelling correction has an answer already in the specification: `skos:hiddenLabel` exists so a
cataloguer can author the misspellings, and search reads hidden labels by default.

The risk this project now carries has flipped direction. It used to be "matching is too literal";
it is now "the next plausible improvement merges distinct terms". The pinning tests were rewritten
to pin the new direction rather than deleted.

## What it cost

Measured by `fold_cost_against_lowercasing` (`#[ignore]`d, in `fold.rs`, release build):

| corpus | fold | `to_lowercase` | ratio |
|---|---|---|---|
| ASCII | 21.5 ns/label | 23.4 ns/label | **0.92×** |
| accented Latin | 1419.3 ns/label | 123.7 ns/label | **11.5×** |
| Greek | 1515.6 ns/label | 497.6 ns/label | **3.05×** |

ASCII is free — slightly *faster* than the Unicode-aware `to_lowercase` it replaced, because
`fold` takes an ASCII fast path whose equivalence to the general path is checked character by
character over the whole range rather than argued.

Non-ASCII is not free, and the honest reading is that this made an already-recorded problem worse
for exactly the vocabularies the feature is for. Search is a linear scan of a model rebuilt per
request (`UNTESTED.md`, iteration 38), and every label read is now folded. At 1419 ns, a scan of
AGROVOC's measured 1.25M labels spends roughly **1.8 seconds folding alone**, per search. The fix
is to fold each label once when the model is built rather than once per query — that is an
indexing change, it belongs with the indexing work already recorded, and it is not this item.

## Consequences

- `openbiz search` and `openbiz mint` both stop missing on case and on encoding, through one
  matcher, with the end-to-end proof at the level a curator meets it: three tests in
  `openbiz-server` assert that a mint for `Strasse` now reaches STOP.
- Three report texts that claimed "ignores case but not accents, spelling, or Unicode
  normalisation" were wrong the moment this landed and are rewritten. A tool that overstates its
  reach on the no-results path is how a duplicate gets created.
- `slug` deliberately keeps `to_lowercase`, and its doc comment now says why rather than pointing
  at a gap that has closed: a slug becomes a published IRI, and folding would mint `strasse` for
  `Straße`, changing the identifier a cataloguer typed into a different word.
- Reopening this is cheap: one function, four crates, one enforced seam.
