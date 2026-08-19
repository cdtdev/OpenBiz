# 0025 — S24 answered by walking, and S27 read off the walk

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 28
- **Implements:** the decision `0024` took. `0024` measured the cost and ruled out materialising
  S24's closure; this is what was built instead, and what that shape then made possible and
  impossible.
- **Amends:** `0023` (semantic relations and the super-property citation), which left S24 and S27
  unimplemented and said so.

## Context

§8 of the SKOS Reference makes `skos:broaderTransitive` and `skos:narrowerTransitive`
`owl:TransitiveProperty` (**S24**), and §8.4 states the section's only integrity condition:
`skos:related` is disjoint with `skos:broaderTransitive` (**S27**).

The two are one item because S27 cannot be tested without S24. §8.5's Examples 27 and 29 are each
marked *not consistent* by the specification, and in each the clash is between two concepts the
author never linked directly — Example 27's own prose says "the clash is not immediately obvious"
and that it "becomes apparent when inferences are drawn". A build applying S27 to the one-step
links would report Examples 26 and 28 and pass 27 and 29: a validator answering "consistent" for a
graph the specification marks otherwise, which is worse than one that says nothing.

`0024` had already settled where the closure may live: **nowhere**. A chain of 100 000
`skos:broader` links is a legal SKOS graph — §8 states no condition against depth, and §8.6.8 says
a cycle is legal too — and its closure is 5 000 050 000 pairs. And a stored `(Node,
RelationOrigin)` can cite S24 but cannot name the path it took, which `CLAUDE.md` §3 requires of
every inference.

## Decision

**S24 is a bounded breadth-first walk, computed on read and never stored.**
`CoreModel::ancestry(concept, bound)` returns an `Ancestry`; `Resource::relations` is untouched and
keeps meaning "links under this property", permanently. `crates/openbiz-skos/src/ancestry.rs`.

**S27 is read off that walk at build time**, one walk per concept that has a `skos:related`. A
vocabulary with no associative links pays nothing; a vocabulary with no hierarchy pays walks that
find nothing.

Four things fell out of that shape, and three of them were not obvious before it was built.

### 1. The path is free, and it is the derivation

A walk knows how it got somewhere because that is how it got there. `Ancestry` keeps a predecessor
map — one node per ancestor, not one path per ancestor, so its memory is the hierarchy and not the
hierarchy times its depth — and `path_to` reconstructs the route on demand. For a transitive
conclusion **the path is the derivation**, so `derivation_to` renders it as premise plus S24.

`derivation_to` returns `None` for a one-step ancestor, deliberately. That link is S22's or the
graph's own and is already in `CoreModel::derivations`; crediting S24 with it would be a citation
for a conclusion it did not add.

### 2. One direction of walk is enough, and that is not an optimisation

S27's finding is raised by walking **up** from each concept with an associative link. Example 29
states its hierarchy with `skos:narrower` and is still caught, because S25 has already turned those
links round and S23 has already put the associative link at both ends — so the clash is found from
whichever end the hierarchy climbs from. §8.4's own note is this argument:

> because skos:related is a symmetric property, and skos:broaderTransitive and
> skos:narrowerTransitive are inverses, skos:related is therefore also disjoint with
> skos:narrowerTransitive

A second downward walk would find the same pairs and report them twice.

### 3. A bound is required, and an abandoned walk must not look like a finished one

The walk is bounded by ancestors reached and links followed (`AncestryBound::DEFAULT` — 100 000 and
1 000 000). Without a bound, asking §8.4's question of every concept in a million-link vocabulary
is a million traversals of the whole hierarchy, and the honest failure mode of an unbounded walk is
a server that stops answering rather than one that says it does not know.

But a bound introduces a worse failure than the one it prevents, unless it is reported. A walk that
gave up after two ancestors and a concept that genuinely has two ancestors produce the same count,
and reading the second off the first is exactly how a validator reports "consistent" for a graph it
never finished checking.

So **`Severity` gained a third variant, `Unchecked`**, and `Finding::AncestryBoundReached` carries
it. `CoreModel::is_consistent()` deliberately still answers `true` in its presence — we have not
found a violation — and `CoreModel::checks_are_complete()` is the question a report must ask beside
it. `openbiz inspect` now closes with one of **three** sentences rather than two.

This closes the risk `docs/UNTESTED.md` recorded at iteration 24: that "no SKOS integrity condition
is violated by this graph" is true of every condition we implement and silent about the ones we do
not. It does not close it in general — the report still does not enumerate which conditions were
checked — but the one case where the answer is "we started and gave up" now says so.

### 4. A reflexive pair violates S27, and that is our reading

§8.6.5's Example 33 (`<A> skos:related <A>`) and §8.6.7's Example 36 (`<A> skos:broader <A>`) are
each marked **consistent**, and this build agrees: neither on its own is a finding.

A graph with **both** is Example 26 with `<B>` substituted by `<A>`, and this build reports it as
inconsistent. Disjoint properties is a condition on pairs and nothing excludes the pair `(A, A)`.
The alternative reading — exempt reflexive pairs from S27 — would have to invent an exception the
specification does not state, which is the larger liberty of the two. It is recorded here because
the specification prints no example either way, so this is a decision and not a citation.

## What was rejected

- **Materialising the closure, at any threshold.** `0024`. The input that breaks it is legal SKOS,
  so there is no vocabulary size that makes it safe.
- **A downward walk as well as an upward one for S27.** §8.4's note makes it redundant; see above.
- **Building the descendants walk alongside the ancestors one.** It is the same function with the
  inverse property and it has no caller, which `CLAUDE.md` §4 calls not-done rather than ahead. It
  arrives with the concept-tree item that needs it.
- **Reporting a cycle as a finding.** §8.6.8 marks Example 37 consistent and says only that a cycle
  "represents a potential problem" for "many applications". Inventing the condition would be the
  incumbents' failure `docs/COMPETITIVE.md` records. The walk terminates on one and reports the
  concept as its own ancestor with the path that names the cycle, which is the fact rather than a
  verdict.
- **Making `is_consistent()` false when a check was abandoned.** That claims an inconsistency we
  have not found. The two questions are separate and the report asks both.

## What this does not settle

- **Cost at scale is unmeasured.** `0024`'s harness measured storing; this stores nothing, and the
  cost of *not* storing is now the open number — n walks of average depth d, once per validation
  run. The bound turns the pathological case into a refusal rather than a hang, which is honest,
  but a validator that declines to answer is still a validator that declines to answer. Recorded in
  `docs/UNTESTED.md`; extending `scale.rs` to the walk is a proposal, not a claim.
- **The S27 pass walks at build time**, so every `openbiz inspect` of a vocabulary with associative
  links pays for it whether the operator asked about consistency or not. That is the right default
  for a report whose job is to find problems; it may be the wrong default for the concept-tree
  endpoint that reads the same model in Phase 3.
- **The default bound is a guess.** 100 000 ancestors and 1 000 000 links are chosen to be far
  above any thesaurus and far below the point where the walk is why a request is slow. No
  vocabulary in this repository comes near either, so neither has been observed being hit outside a
  test that lowered it deliberately.
