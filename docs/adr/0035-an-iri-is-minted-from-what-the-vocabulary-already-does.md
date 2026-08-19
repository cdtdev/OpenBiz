# 0035 — An IRI is minted from what the vocabulary already does, and it reserves nothing

- **Status:** accepted
- **Date:** 2026-08-19 (iteration 39)
- **Item:** Phase 2 — "Concept IRI minting, part 1 — the pattern, the two policies, and collision
  detection"

## Context

A concept's IRI is the one thing about it that can never be corrected. Labels are translated,
definitions rewritten, a concept moves in the hierarchy and is deprecated and replaced — and
through all of it the IRI is what every downstream system, published dataset and citation holds.
Two mistakes are therefore permanent:

1. **Minting an IRI something else already uses.** RDF does not refuse it and cannot: statements
   about the same IRI are statements about the same thing. Two concepts silently become one, found
   out later by whoever reads a concept with two preferred labels in one language.
2. **Minting an IRI that encodes something mutable.** A readable IRI derived from a label promises
   the label will not change. Nobody can keep that promise. It is still often the right trade — a
   legible IRI is worth a great deal in a SPARQL query and in a published dataset — but it is a
   trade, and a tool that makes it silently has decided something on the user's behalf.

There is no "create concept" command in this build, on purpose: `CLAUDE.md` §1.7 puts discovery
before creation and `DiscoveryProvider` is still ahead of us in the plan. What exists is the
candidate seam — propose a change in a file, stage it, read it, approve it — and to write that file
somebody has to decide the new concept's IRI. Today they decide it by copying an existing IRI and
editing the end, which is how `c_00123` ends up beside `c_124`.

## Decisions

### 1. The command reads and reserves nothing, and says so

`openbiz mint <graph> [<label>] [--pattern <p>]` writes nothing, stages nothing, and allocates
nothing. Run it twice and it answers the same both times.

This was the sharpest design risk in the item. A minter that *looks* like an allocator is worse
than no minter: somebody mints twice, believes they hold two identifiers, and creates two concepts
on one IRI — the exact failure the command exists to prevent, reintroduced by the command itself.
Making it reserve would have meant a write path, a lease, an expiry, and a reconciliation with the
candidate seam, all before there is authentication to attribute the reservation to.

So the closing paragraph of every report states it in as many words, and an integration test takes
a backup before and after and compares the N-Quads. The seam that makes this coherent already
exists: an IRI becomes taken when a change carrying it is *staged*, and the next mint sees it.

### 2. The default pattern is read off the vocabulary, and refuses to guess

Every incumbent has a configurable URI pattern, and every one makes you configure it against
nothing. Here the default is evidence: the namespace most of the vocabulary's concepts are already
in, and whether their local names are numbered (`c_1234`, as AGROVOC and LCSH do) or worded. The
counts are printed, so the suggestion can be checked rather than believed.

Two majority rules, and the refusals are the point:

- If no namespace holds *most* of the concepts, there is **no suggestion** and `--pattern` is
  required. A leading namespace that is a plurality is a coincidence; minting into it would produce
  official-looking IRIs belonging to nothing.
- If most local names in that namespace are not numbered, the readable pattern is suggested rather
  than a numbered one with a made-up fixed part.

`--pattern` overrides, and when it disagrees with what the vocabulary suggests the report says so
loudly. Minting under a new pattern is legitimate — it is how a convention changes — and it is also
how a concept lands in the wrong namespace unnoticed.

### 3. The two collision rules deliberately differ

- A **numbered** collision goes *above the highest number in use* and never fills a gap. A gap is
  evidence that something was there. The deprecation lifecycle (still ahead in the plan) keeps
  history rather than deleting, but an IRI that left a vocabulary by any route must not come back
  attached to a different concept.
- A **worded** collision is **refused**. Appending `-2` is what the incumbents do, and
  `renewable-energy-2` is a silo with a suffix: it means the vocabulary already holds a concept with
  this label, and §1.7 says reuse outranks creation. Where the two really are homographs — Java the
  island, Java the language — the answer thesaurus practice has used for decades is a qualifier in
  the term itself, which the caller supplies by minting from the qualified label. The report names
  that way out rather than leaving a dead end.

Zero padding is kept, and only when the vocabulary really pads — see "What running it found".

### 4. Collisions are checked across the whole store, and against staged changes

An IRI is a global identifier. A deployment where two vocabularies extend one namespace is an
ordinary enterprise case, so the scan reads **every registered vocabulary graph**, not only the
target, and **every pending candidate's additions**. The last is the case no incumbent covers: two
curators preparing imports on the same day, each minting `c_13`, discovering on approval that they
have one concept.

A candidate that was *rejected* does not hold its IRIs. Its statements stay staged forever as the
record of what was refused, and an IRI that was refused never denoted anything.

The cost is bounded by keeping only IRIs under the pattern's prefix, so memory is the size of the
namespace and not of the store. The *time* is a full store scan per invocation and is unmeasured —
`docs/UNTESTED.md`.

### 5. Nothing is transliterated

RFC 3987 §2.2 puts essentially the whole of assigned Unicode in `ucschar`, so `Ökologie` needs no
transliteration to be a legal IRI local name, and this does not perform one. Mapping `ö` to `o` is
a lossy, language-specific guess — Swedish and German disagree — and it manufactures collisions
between terms that are not the same word.

A character is kept when it is alphanumeric **and** `iunreserved`; whitespace and other punctuation
become a single `-`; apostrophes are elided rather than split on (`Müller's cheese` →
`müllers-cheese`). Emoji are inside `ucschar` and are dropped anyway, because they are not
alphanumeric — and a label made only of such characters cannot mint a readable IRI at all and says
so, rather than minting the bare namespace.

The `ucschar` ranges are transcribed range by range and pinned by test at their boundaries.

### 6. `openbiz-skos` mints; the store's parser has the last word

`openbiz-skos` is engine-free (`CLAUDE.md` §3) and can therefore only apply a *subset* of RFC 3987:
absolute per RFC 3986 §3.1's scheme grammar, and made only of characters an IRI may carry. That
subset misses things a real parser catches — a broken percent-escape being the likeliest. So
`openbiz-server` puts every minted IRI to `openbiz_store::accepts_iri`, which is Oxigraph's own
`NamedNode` parser, before showing it to anybody. The pattern `https://example.org/%zz/{slug}` is
refused there.

This is a deliberate asymmetry and it is recorded in `docs/UNTESTED.md`: a caller of
`openbiz-skos` that is not `openbiz-server` gets the weaker guarantee, and there is no such caller
today.

### 7. Discovery runs before creation, at the scale honestly available

Before it reports an IRI, the report answers "is this already here?" — one exact-label lookup, over
whole labels of every kind in every language, in the target vocabulary **and** in the changes
staged against it. A match prints `STOP` and names the concept before the IRI is shown.

It is emphatically **not** the discovery pass §1.7 promises and Phase 12 builds, and the report
says which it is: "one exact lookup in one vocabulary and not a discovery pass: a differently
spelled or accented term here will not have been seen." A quiet "nothing found" that reads as
"nothing exists" is precisely how a silo gets created.

## What running it found

Two defects and one contradiction, none of which any unit test had caught. The tenth consecutive
iteration in which running the product found what testing it did not.

1. **A false claim about the vocabulary.** Of a vocabulary holding `c_1`, `c_3` and `c_12`, the
   report said "written with 2 digits, which is how this vocabulary writes them". It writes them
   with one, one, and two. The width of the highest number is not evidence of a padding convention;
   a leading zero is. `HighestInUse::pads()` now asks that question, and the sentence appears only
   when it is true and cites the IRI it was read from. The output was accidentally correct in this
   case — `format!` pads to a *minimum* width — which is what made it a claim worth catching rather
   than a crash.
2. **The report contradicted itself.** The IRI half read the changes staged against the vocabulary
   and the label half did not, so a report could say "nothing is already called that" directly above
   "the IRI is taken by candidate 2". Both sentences were true and together they were nonsense. The
   label check now reads the staged changes too, and names which one a label is in.
3. **"a opaque IRI".** Trivial, and it was on the first line of every numbered mint.

## Alternatives rejected

- **A UUID or hash placeholder.** It would need `uuid` or `rand` against §1.5's dependency budget,
  and a sequential number is what the two largest published SKOS thesauri actually use. A counter
  is opaque enough, deterministic, and testable without a seed.
- **Filling numeric gaps.** Cheaper to explain and permanently wrong: see decision 3.
- **A `-2` suffix on a slug collision.** Decision 3. It is the incumbents' behaviour and it
  manufactures the duplicate this product exists to prevent.
- **Scanning only the target vocabulary.** Cheaper, and wrong about what an IRI is: decision 4.
- **Reserving the minted IRI.** Decision 1.
- **Persisting the policy per vocabulary now.** Split into part 2 of the plan item. It needs a home
  for per-vocabulary settings, and the system graph has nothing writing to it outside the registry.
  A per-invocation pattern is right for one curator at a command line and *insufficient* for a
  deployment where imports, discovery matches and agent proposals must all mint the same way —
  which is the whole of part 2's argument.

## Consequences

- A curator writing an import file has a defensible answer to "what IRI?" that cites the
  vocabulary, and one that refuses when the vocabulary cannot answer.
- The candidate seam gains a second reason to stage early: staging is what makes an IRI taken.
- Part 2 is now a named, scoped item rather than an implication.
- Three `UNTESTED.md` entries: the subset IRI check, the unmeasured store-wide scan, and
  `SlugBound::DEFAULT` — the fourth unmeasured constant in as many iterations.
