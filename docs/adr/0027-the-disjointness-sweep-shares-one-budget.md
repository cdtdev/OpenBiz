# 0027 — The §8.4 disjointness sweep shares one budget across every walk it makes

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 30 (blind-spot pass)
- **Supersedes nothing. Corrects a claim made in** `adr/0025` and in `ancestry.rs`'s own module
  documentation.

## The claim that was wrong

`adr/0025` decided that S24's transitive closure is answered by **walking** rather than by storing,
and bounded the walk so that a pathological graph could not hang a request. `ancestry.rs` states the
reasoning:

> A walk is bounded (`AncestryBound`). Without one, asking §8.4's question of every concept in a
> million-link vocabulary is a million traversals of the whole hierarchy, and the honest failure
> mode of an unbounded walk is a server that stops answering rather than one that says it does not
> know.

The diagnosis is exactly right and the remedy did not implement it. `AncestryBound::max_links` was
**per walk**. §8.4's check makes one walk per concept that has a `skos:related`. A per-walk budget
multiplied by one walk per concept is not a bound on anything — the pass's cost is the number of
associated concepts times the depth of the hierarchy, and the ceiling that was supposed to stop it
is never consulted at that level at all.

Iteration 28 wrote the sentence, iteration 28 wrote the code, and the sentence and the code
disagreed. Nothing caught it because **no fixture in the repository stated a single `skos:related`
at scale**: `scale.rs` measured the model the pass reads, four hierarchy shapes and three sizes, and
never once made the pass run.

## What it costs, measured

`crates/openbiz-skos/src/scale.rs`, release, this machine. Each shape at 10 001 concepts, with and
without one `skos:related` per concept. The associate is **outside** the hierarchy, so nothing here
is a violation and every walk runs to the top — which is the expensive case and is perfectly
ordinary SKOS.

| shape | no associative links | one per concept | the pass |
|---|---|---|---|
| star (depth 1) | 76.6 ms | 130.1 ms | 54 ms |
| tree (branching 10) | 60.5 ms | 130.8 ms | 70 ms |
| **chain (depth 10 000)** | **62.6 ms** | **30.63 s** | **30.6 s** |

The chain row is **490 times the whole rest of the model build**, and `AncestryBound::DEFAULT`'s
million-link ceiling never fired once, because no *single* walk came within two orders of magnitude
of it. The report said the check had finished.

That number is quadratic, so it is the small one. The same shape at 100 001 concepts is a hundred
times the work — tens of minutes — and at a million it is days.

**None of this needs a hostile input.** §8 states no condition against depth, `skos:related` is the
second-most-used property in a thesaurus after the labels, and the vocabulary that produced the
30 seconds is 20 001 triples with no labels in it.

## The decision

**`AncestryBound::max_links` is the budget for one *check*, not for one *walk*.** A sweep hands each
walk what is left of the budget rather than a fresh copy of it. When it runs out, the sweep stops.

Two consequences, both deliberate:

1. **A new finding, not a reused one.** `Finding::DisjointnessSweepExhausted { checked, unchecked,
   links_walked }`, at `Severity::Unchecked`. Reusing `AncestryBoundReached` would have named the
   one concept the sweep happened to be walking and said nothing about the thousands it never
   reached — which reads precisely like "those were checked and were fine". The two are different
   failures: `AncestryBoundReached` says one concept sits under more hierarchy than a walk may
   cover; this says **the check itself stopped**.
2. **A clash found on the way out is still reported.** What a partial walk *did* reach is a real
   answer. It is only the **absence** that may not be read from an abandoned check, which is the
   distinction `Severity::Unchecked` and `checks_are_complete()` already exist to carry.

`CoreModel::ancestry` is unchanged: a caller asking about one concept — `openbiz ancestors` — makes
one walk, and for that caller "per check" and "per walk" are the same sentence.

### After

| shape, 10 001 concepts | before | after |
|---|---|---|
| chain, one associate per concept | 30.63 s, check reported **complete** | **530 ms**, check reported **abandoned** |
| star | 130.1 ms | 138.1 ms |
| tree | 130.8 ms | 133.9 ms |

The chain at each order of magnitude, after: 1 001 concepts **195 ms and the check completes**
(1 124 250 owed links is over the budget at 1 500, so a thousand-deep chain is still fully checked);
10 001 **620 ms**; 100 001 **2.61 s**. All bounded, all honest about which.

## What this decision costs, stated plainly

**A deep vocabulary now gets a partial S27 answer where it previously got a complete one very
slowly.** Measured: a 10 001-concept chain with a real violation on every concept reported **1 413
of 9 999 violations** before the budget ran out, against 999 of 999 at a thousand concepts. The
report says so — `unchecked` counts what was skipped and the closing sentence hedges — but a
governance team reading "1 413 violations, check abandoned" has less than they had before.

That is the right trade under `CLAUDE.md` §3 and §4 (a validator that declines to answer beats one
that stops answering, and honesty over green), and it is **not a good outcome**. It converts
`AncestryBound::DEFAULT`'s million into a product-visible limit rather than the backstop against a
pathological graph it was introduced as. The real fix is an algorithm whose cost is not
concepts × depth, or a budget that scales with the vocabulary. Both are in `docs/PROPOSED.md`,
neither is promoted, and this ADR is not pretending the budget is the answer — it is the thing that
stops a legal vocabulary hanging the report while a better answer is designed.

## What was measured and what was not

- **Measured:** the pass at three shapes × 10 001 concepts; the chain at 1 001 / 10 001 / 100 001;
  the cost of a violation on every concept at 1 001 and 10 001. One process, one machine, no
  concurrent load, synthetic vocabularies with no labels — the same limits `scale.rs` already
  states of itself.
- **Not measured:** the pass at a million concepts, and the memory a *long* violation path costs.
  `path_to` is breadth-first, so the grandparent clash the harness generates carries a three-node
  path however deep the hierarchy is. A vocabulary relating every concept to its root would hold
  paths quadratic in the depth, and no shape here generates one. Recorded in `docs/UNTESTED.md`
  rather than implied to be covered.

## Why this is in an ADR and not just a bug fix

Because the meaning of a shipped public field changed, and because the failure was not a typo. The
code was written by an iteration that had **correctly diagnosed the aggregate problem in prose** and
then bounded the wrong thing. The general lesson is in `docs/LOOP-LOG.md`: a harness that measures
the data structure a pass reads is not measuring the pass, and the gap between the two is invisible
in a green suite.
