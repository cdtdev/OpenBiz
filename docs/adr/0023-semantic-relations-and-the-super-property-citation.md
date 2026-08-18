# 0023 — Semantic relations: what we close, and why the citation runs through a property nobody writes

- **Status:** accepted
- **Date:** 2026-08-18 (iteration 24)
- **Supersedes:** nothing. Extends `0019` (SKOS core model) and follows `0022`'s pattern for
  entailed links.

## Context

§8 of the SKOS Reference is the section a thesaurus is bought for. `skos:broader`/`skos:narrower`
is ISO 25964's BT/NT and `skos:related` is its RT; a vocabulary without them is a word list.

Ten statements, S18–S27. This ADR covers the eight we apply. **S24 (transitivity) and S27 (the
one integrity condition) are the next build-plan item** and are not claimed here — see
`docs/UNTESTED.md`, which also records that four of §8.6's five examples therefore read as clean
today.

The build-plan item was split in place because the two halves have different shapes: this one is
a set of one-step closures over what the graph states, and the other is a transitive closure with
cycle containment plus a condition that reads it. Landing them together would have meant one
iteration long enough to be rushed at the end, which is where a wrong entailment gets written.

## Decisions

### 1. Five relations are stored; `skos:semanticRelation` is read and stored under nothing

`SemanticRelation` has five variants: `Broader`, `Narrower`, `BroaderTransitive`,
`NarrowerTransitive`, `Related`. `skos:semanticRelation` is deliberately not one.

S21 makes the other five its sub-properties, so every link entails one — but the entailment runs
*upwards*. From `<A> skos:semanticRelation <B>` nothing follows about which of the five holds, so
there is no bucket to file it in, and materialising a sixth set would be another copy of every
link in the vocabulary answering a question the five already answer.

A graph that states it outright is still read: S18 refuses a literal on it and S19/S20 type both
ends. It then stops. A test asserts that none of the five gains an entry from it, because
inventing `skos:broader` out of a super-property statement would be a hierarchy we made up.

### 2. The inverses run before the sub-property lift, and the order is load-bearing

Two passes:

1. **S25, S26, S23** — the inverse of every stated link. S25 pairs `skos:broader` with
   `skos:narrower`, S26 pairs the two transitive variants, and S23 makes `skos:related` its own
   inverse (`owl:SymmetricProperty` is exactly that, so one pass closes all five while still
   citing the statement the specification prints for each).
2. **S22** — the lift into the transitive variants.

Running the lift first would leave a hierarchy written with `skos:narrower` short of its
`skos:broaderTransitive` links and one written with `skos:broader` short of its
`skos:narrowerTransitive` ones. The model's answer would then depend on which direction the
author happened to type. `the_direction_the_author_typed_does_not_change_the_answer` compares the
two whole models and is the test that would catch a reordering.

The lift iterates every relation and asks `transitive_variant()`, rather than iterating the two
that have one. That is not style: with the two hard-coded, a table that wrongly lifted
`skos:related` into `skos:broaderTransitive` would be wrong in the table and right in the closure,
and only the table's own unit test would notice. Verified by mutation — with the loop over all
five, the same mutation fails a model-level test as well.

### 3. A direction the graph states is never restated as an inference

The third time this decision is made — after `ClassOrigin` and `LabelOrigin` — and for the third
time the same reason: a report that cannot distinguish what the graph said from what we concluded
is not an audit trail. `RelationOrigin` is now shared between `skosxl:labelRelation` and the five
semantic relations rather than duplicated, because the question is identical in both places.

### 4. The class citation runs through `skos:semanticRelation`, and the S21 step is recorded only when a class follows

This is the decision that took the longest and it is worth stating plainly.

S19 and S20 are the domain and range, and they constrain **`skos:semanticRelation`** — not
`skos:broader`. So a report that printed "`<B>` is a `skos:Concept` because `<A> skos:broader <B>`
and S20" would cite a statement that does not mention the property the author wrote. The chain is
therefore printed in full: S22 to the transitive variant, S21 to the super-property, then S19 and
S20 conclude from that.

Three steps per link is a lot to print, and printing it for every link in a vocabulary would
double the derivation list. So **the S21 step is recorded only when a class actually follows from
it.** A vocabulary that types its own concepts — the ordinary case — produces no S21 step and no
S19/S20 conclusion, and the pass is silent. The alternative of recording none would leave the
class entailment citing a premise that appears nowhere in the report, which is worse than
verbose.

The S22 step is *always* recorded, because it is a link the caller can read back, not a step in a
citation.

### 5. Applying the domain and range — and the fan-out it causes, deliberately

Like S60 and S61 before them, S19 and S20 usually report nothing. What they do is make a mistake
visible: a `skos:broader` pointing at a `skos:Collection` types that collection as a concept, and
S37's disjointness then says so. Without them the same graph reads as clean, because nothing else
in it would ever type the collection.

Iteration 23 closed on an open worry about exactly this — that a domain or range rule entailing a
class nobody wanted makes one authoring error fan out into several findings about rules the author
never engaged with — and asked the next iteration that touched a domain rule to stop treating it
as a curiosity. It was measured here rather than assumed. **A `skos:broader` into a collection
produces one finding, not several.** The reason is structural and not luck: S48's fan-out came
from `skosxl:Label` also being *constrained* (S52 requires a literal form, so a concept made a
label acquires a second complaint), whereas `skos:Concept` carries no such constraint — nothing in
SKOS requires a concept to have anything. So the fan-out is a property of entailing a *constrained*
class, not of domain and range rules in general. That is a narrower worry than iteration 23 left,
and the remaining Phase 2 domain-and-range items — the mapping and documentation properties — all
entail `skos:Concept`. The concern is recorded as understood rather than carried forward as an
open doubt.

### 6. Polyhierarchy is counted, never reported

A concept with two broader concepts is ordinary in a thesaurus and §8 states nothing against it;
ISO 25964 relies on it. It is a number in the report — the number a migration from a
strictly-single-parent source asks for first — and never a `Finding`. Reporting it as one would be
inventing an integrity condition the specification does not state, which is the incumbents'
failure `docs/COMPETITIVE.md` records.

The same reasoning covers a concept broader than itself, or related to itself: §8 states no
condition against either. S27 is about `skos:related` and `skos:broaderTransitive` *together*, and
it is not implemented yet in any case. `adr/0022` made the identical call for a label linked to
itself.

### 7. Links are counted, not statements

`openbiz inspect` reports `skos:broader` entries and not `skos:narrower` ones, because after the
closure they are the same links seen from the two ends; summing both would report twice the
hierarchy the author wrote. `skos:related` is counted once per unordered pair for the same reason
S62's links were. The report also says how many of the hierarchical links were written the other
way round, which is the one thing the count alone hides.

A link the graph stated with `skos:broaderTransitive` itself is reported on its own line and is
*not* in the hierarchical count — sub-property entailment runs upwards, so nothing lifts it down
to `skos:broader`, and a vocabulary authored that way would otherwise read as one with no
hierarchy at all. That line also says, in the product and not only in a ledger, that the closure
S24 licenses is not taken.

## Consequences

- A hierarchy written in either direction reads the same way, which is what makes two merged
  sources comparable.
- `Resource::relations(BroaderTransitive)` is **not** an ancestor set. It is named for the
  property and not for "ancestors" precisely so a caller cannot mistake it for one, and
  `docs/UNTESTED.md` records the gap.
- The core model now holds something that scales with the *size* of a vocabulary rather than with
  its structure, for the first time — four entries per stated link. `CoreModelBuilder`'s doc
  comment claiming otherwise is now narrower than it reads, and the measurement is recorded as
  owed before S24's closure lands on top of it.
- `openbiz inspect`'s closing sentence — "no SKOS integrity condition is violated by this graph" —
  is true of the conditions we check and silent about S27, which we do not. Recorded.

## What was measured

Seven mutations, each turning the suite red:

| Mutation | Failing tests |
|---|---|
| S22 lift disabled | 6 |
| Inverse closure disabled | 6 |
| S19/S20 domain and range not applied | 6 |
| `skos:related` lifted into `skos:broaderTransitive` | 2 |
| `skos:related`'s inverse pointed at `skos:broader` | 3 |
| Hierarchical links counted as ordered pairs | 1 |
| `skos:semanticRelation` not read at all | 1 |
