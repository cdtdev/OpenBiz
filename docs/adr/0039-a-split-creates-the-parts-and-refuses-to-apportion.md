# 0039 — A split creates the parts and refuses to apportion the original

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 44
- **Supersedes:** nothing
- **Related:** `adr/0035` (minting), `adr/0036` (the recorded pattern), `adr/0037` (a move is one
  candidate with two halves), `adr/0038` (a merge is checked against the vocabulary it would leave)

## Context

`docs/BUILD-PLAN.md`'s third bulk operation is "split one concept into several". The two before it
had a determinate answer for every statement they touched: a move re-parents two links, and a merge
sends every reference to one place, because there is only one place for it to go.

A split has no such answer, and this is not a gap in the design — it is the operation. A concept is
being divided **because** its labels, its narrower concepts, its `skos:related` links and its notes
belong to different things, and which part each belongs to is the editorial judgement nobody has
made yet. That judgement is exactly what the operator is being asked for. A tool that apportioned
them automatically would be inventing meaning and putting a person's name on it.

Two shapes of split are both ordinary in thesaurus practice, and they are not the same change:

- **Polysemy.** `Banks` was two senses under one term. `Banks (financial)` and `Banks (river)`
  belong where `Banks` was.
- **Granularity.** `Vehicles` was too coarse. `Cars` and `Trucks` belong *under* it.

## Decision

`openbiz split <graph> <concept> --place beside|below --into <label> --into <label> …` computes one
candidate and stages it. Four decisions are worth recording.

### 1. A split adds and removes nothing else — the original survives untouched

The candidate is additions only, and it is the first of the three bulk operations that removes
nothing. Every statement about the concept stays exactly where it was. The command then reports
what is still attached to it — the concepts below it, the associative links, the mapping links, the
notes, the labels — with the command that apportions each kind (`openbiz move` for a child).

The alternative, deleting the original, would leave every reference to it dangling; that state is
well-formed RDF that nothing reports, and avoiding it is what the *next* plan item, deprecation
with replacement, exists for. Splitting and retiring are two changes, reviewed separately, and an
auditor gets both in the trail.

**The report's order is the argument.** What was *not* done comes before the diff, because a reader
who stops at "2 parts proposed" believes the job is finished. It is not: the split is the easy half.

### 2. `--place` is required and has no default

Choosing wrongly produces a vocabulary that is consistent SKOS and says something false.
`Banks (river)` is not narrower than `Banks` — homonymy is not hierarchy, and §8.1's `skos:broader`
relates *concepts* rather than terms — but §8.6.7 makes the graph perfectly consistent, so nothing
downstream reports it. That is the same argument `adr/0037` makes about a move into a cycle, and it
is why this asks rather than inherits a guess.

Under `beside`, each part takes the concept's own broader concepts — stated in whichever direction
the vocabulary states them, so a downward-authored thesaurus stays downward — its schemes, and its
place as a top concept where it is one. Under `below`, each part is `skos:broader` the concept, and
is deliberately **not** made a top concept, because a part below a top concept is not one.

### 3. Each part records where it came from, in the vocabulary, with `prov:wasDerivedFrom`

PROV-O is the vocabulary `CLAUDE.md` §2 commits to for provenance, and the derivation is the one
thing this operation knows that no later reader could reconstruct: these concepts exist because that
one was divided. It goes in the **vocabulary graph**, not in OpenBiz's own graphs, because it is a
statement about the concept rather than bookkeeping about the edit — so it survives an export and
answers "why does this concept exist?" in a tool that has never heard of OpenBiz. It is also the
recorded justification `CLAUDE.md` §1.7 asks of anything that creates rather than reuses.

**This has a consequence, and it was found by running the command rather than by reasoning.** Our
statement now sits in the user's graph and interacts with the user's own declarations. A vocabulary
that declares `prov:wasDerivedFrom rdfs:subPropertyOf skos:related` makes a `below` split entail an
S27 violation — and this build does not catch it, because it correctly reports S27 as **unchecked**
in such a vocabulary ("this build entails nothing from it") rather than falsely held, and a
condition with no verdict either side cannot be newly violated. That is honest behaviour producing a
blind spot, and it is in `docs/UNTESTED.md` with the reproduction.

### 4. The parts are minted, not named by hand, and the whole condition set is still run

The IRIs come from the same `pattern_for` that `openbiz mint` uses — `--pattern`, then the
vocabulary's recorded policy, then the convention read off its own concepts — so a deployment that
has recorded a policy (`adr/0036`) gets parts named the way its curators name everything else. Each
minted IRI is offered back to the scan before the next is minted, so three parts get three numbers.
This is the **second producer** to mint under a recorded policy, which is what the minting item's
`UNTESTED.md` entry asked for when it had only one.

`adr/0038`'s check runs here too, over the whole condition set. A split adds statements about IRIs
nothing has ever mentioned, so it is hard to see how it could break a condition that holds — and
"hard to see how" is precisely the reasoning iteration 43 found to be wrong about a merge, where a
hand-written subset would have caught S14 and missed S27. **We could not construct an input that
trips it**, and it is kept anyway, because the cost is one function call and the alternative is
trusting an argument of exactly the shape that has already failed once. It was generalised out of
`openbiz merge` into `crate::staging` in the same commit, unchanged.

## What was measured

- The command was run by hand against a store on disk before any test was written, for the
  fourteenth iteration running, and two things in the report were wrong until they were read:
  "1 concept is below it: move **each** under the right part", and a label count that claimed
  "including the one that named both senses" — presumptuous for a polysemy split and simply false
  for a granularity one, where no label ever named two senses.
- **A collision between a part's label and an existing concept behaves differently under the two
  minting policies**, which was found by a test failing. Under an *opaque* pattern the report warns
  and carries on, because a large vocabulary has legitimate homonyms. Under a *readable* one the
  label becomes the local name, so the IRI is already taken and `openbiz mint` refuses it rather
  than suffixing it — `CLAUDE.md` §1.7 working as designed, but arriving as a message about an IRI
  when the operator's problem is a name. Both are tested; the asymmetry is recorded rather than
  papered over.
- 44 tests: 24 in `openbiz-skos` for the computation and every refusal, 11 in the server for the
  report and the store, 3 in argument parsing, and 5 against the real binary on disk in separate
  processes — where the item's actual claim lives, because "the concept is left exactly as it was"
  is a statement about a whole graph and is asserted by reading it back off disk with
  `openbiz backup` before and after and comparing every line that mentions it.
- No new dependency.

## Consequences

- A split is not a complete edit and the product must never present it as one. The unapportioned
  list is the honest half of the feature; a UI that shows the parts and hides the list would be
  worse than no split at all.
- A reference from another vocabulary still denotes what it denoted, so nothing there is wrong and
  nothing is rewritten — it is counted and named, as a merge does.
- The parts inherit `skos:topConceptOf` in the subject-first direction under `beside`, because the
  model closes S8 on read and cannot say which of the two directions the graph asserted. That is a
  real model gap, it predates this item, and it is in `docs/UNTESTED.md`.
