# ADR 0034 — Searching labels: forgiving by default, hidden labels included, and honest about what it did not match

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 38
- **Supersedes nothing.** It is the first command in this build that starts from a *word* rather
  than from an IRI.

## Context

`openbiz ancestors`, `paths`, `tree`, `notes` and `mappings` all begin with a concept IRI the asker
already has. A subject-matter expert sitting down in front of a thesaurus has no IRI. They have a
word — the organisation calls the thing a *carrier bag* — and they do not know whether the
thesaurus calls it that, calls it something else, or does not have it at all.

`CLAUDE.md` §1.7 states the commercial reason this matters more here than in a general search box:
**reuse outranks creation, and the mechanism by which a silo is actually created is a failed
search.** Somebody looks for a term, does not find it, concludes it does not exist, and creates the
tenth overlapping concept. A search that is too strict manufactures exactly that outcome and
reports it as "no results" — which is indistinguishable, on screen, from the truth.

Four things had to be decided rather than assumed.

## Decision

### 1. Every default is the forgiving one, and `skos:hiddenLabel` is searched

The default query matches **anywhere in the label**, in **any language**, over **all three** SKOS
lexical labelling properties. Narrowing is available (`--exact`, `--prefix`, `--lang`,
`--untagged`, `--kind`) and is never the default.

Hidden labels are not an optional extra here. SKOS Reference §5.1 defines the property *in terms of
search*:

> The hidden labels are useful when a user is interacting with a knowledge organization system via
> a text-based search function. The user may, for example, enter mis-spelled words when trying to
> find a relevant concept. If the mis-spelled query can be matched against a hidden label, the user
> will be able to find the relevant concept, but the hidden label won't otherwise be visible to the
> user (so further mistakes aren't encouraged).

A search that skipped `skos:hiddenLabel` would defeat the one labelling property the specification
justifies by search. So it is searched by default, and a caller who is showing results to the
public rather than to a curator narrows with `--kind`.

### 2. The second half of that sentence is a **display** rule, and it binds elsewhere

"the hidden label won't otherwise be visible" governs how a concept is *named*, not whether a match
on one may be reported. `Resource::display_label` already never chooses a hidden label, so the
report **names the concept by its preferred label** and prints the matched hidden label beside it,
annotated with what §5.1 says about it.

That is the right reading for this audience: the report is addressed to the person curating the
vocabulary, who cannot maintain a hidden label they are not allowed to see matched. A public-facing
search front-end built on this API should pass `--kind pref --kind alt`, and the option exists so
that it can.

### 3. Matching is Unicode lowercasing — which is neither case folding nor normalisation

§5.1 says a lexical label is "a string of UNICODE characters", so both sides are compared after
`str::to_lowercase`, the full Unicode mapping, not ASCII. Two things that deliberately are **not**
done, because the standard library offers neither:

- **Case folding.** `"Straße"` lowercases to itself, so `strasse` does not find it. In Greek a
  final `Σ` lowercases to `ς` while a medial one lowercases to `σ`, so a user typing `οδόσ` does
  not find `ΟΔΌΣ`.
- **Normalisation.** A composed `é` (U+00E9) and a decomposed `e` + U+0301 look identical and are
  different strings. Both occur in real multilingual thesauri.

Both are **pinned by tests that assert the miss**, so the day either arrives the test fails and
`docs/UNTESTED.md` cannot go stale silently. Adding either means a dependency
(`unicode-normalization`, or a case-folding table), which is a §1.5 decision and not one to take
inside a feature.

The report says so in its own words when nothing matched: *"matching ignores case but not accents
or spelling"*. A miss that explains its own limits is a miss the reader can act on.

### 4. No match offset is reported

Lowercasing is not length-preserving — `İ` (U+0130) lowercases to two code points — so an offset
into the folded form is not an offset into the label the author wrote. A caller highlighting the
matched characters from such an offset would highlight the wrong ones on exactly the labels that
most need care. `MatchQuality` therefore records *how* a label matched (exact, prefix, infix),
which is what ranking and explanation need, and no index is exposed. Phase 3's as-you-type search
will need highlighting; it will need it computed on the label as written, and the absence of an
offset here is what will force that.

### 5. Language filtering is RFC 4647 **basic** filtering, and the wildcard is not "everything"

`--lang en` selects `en`, `en-GB`, `en-US` and not `enm`, which is RFC 4647 §3.3.1 exactly. A
malformed range is **refused at the command line** rather than kept and matched against nothing: a
range with a typo that quietly selects no labels reads, in a report, precisely like a vocabulary
that has none in that language.

`LanguageFilter` has three cases and not two, because RFC 4647's wildcard matches any *tag*, and an
RDF 1.1 simple literal has no tag at all. So `--lang '*'` is every tagged label and `--untagged` is
the labels with no tag — the set a multilingual programme is usually hunting for when it audits its
own data. "No filter" is a third thing again, and is the default.

**Extended filtering (§3.3.2) is not implemented**, so `de-DE` does not match `de-Latn-DE`. In
`docs/UNTESTED.md`.

### 6. What matched and what is shown are two numbers, and conflating them is a false negative

The bound (`--limit`, default 200) truncates the **answer**, not the search. The report always
states how many labels matched, how many were read, and over how many resources — and when the
bound cut the list it says the answer is not the whole one.

**This is where running the command found a defect the tests did not.** The report chose its
"nothing matched" branch on whether the *shown list* was empty. With `--limit 0`, eight labels
matched, none were shown, and the report said **"nothing matched"** — a false negative in the one
command whose false negatives cause duplicate concepts. The branch is now taken on the match count,
the zero-bound case says both numbers, and a test asserts it.

### 7. A hit that is not a concept says so

§5 puts lexical labels on resources of any type, so a concept scheme or a collection carrying the
searched word is a legitimate hit and is reported. But a reader scanning results reads every line
as a concept, so a non-concept hit carries what it actually is. Where a label is only reachable
because SKOS-XL was dumbed down (S55–S57), the hit quotes the rule — `CLAUDE.md` §3: no fact
without its derivation.

## Consequences

- A thesaurus modelled the ISO 25964 way, as SKOS-XL, is searchable on the same terms as one that
  is not, and each hit says which it was.
- The search reads the whole model, because unlike a hierarchy walk it has nowhere to start from
  but everything. The cost is the number of labels; the answer is bounded separately.
- `SearchBound::DEFAULT` (200) is **reasoning about a person reading a report, not a measurement**
  against a corpus. Recorded in `docs/UNTESTED.md`, and it is the same class of unmeasured constant
  as `WalkBound::DEFAULT` and `PathBound::DEFAULT`.
- Nothing indexes anything. Every search is a linear scan of the model built for the request. That
  is honest at Phase 2's scale and will not survive Phase 13; it is recorded rather than
  pre-optimised.
- The engine-free rule holds: `openbiz-skos` gained the whole search model without depending on
  Oxigraph or `openbiz-store`.

## Alternatives rejected

- **Preferred labels only, hidden labels behind a flag.** Reverses §5.1's own justification for the
  property and makes the default the silo-generating one.
- **Last-wins for contradictory options.** `--exact --prefix` is not someone changing their mind
  mid-line; it is someone who does not know what they asked for. Quietly obeying the second returns
  a narrower report than the reader believes they are holding. Refused instead.
- **An empty query answered rather than refused.** It matches every label in the vocabulary, and a
  list of everything presented as *search results* tells the reader their query succeeded when what
  happened is that it said nothing.
- **Adding a normalisation or case-folding dependency now.** Either would make this materially
  better and both are §1.5 decisions about the dependency budget, taken deliberately and not inside
  a feature. Recorded as a gap with tests that pin the current behaviour.
