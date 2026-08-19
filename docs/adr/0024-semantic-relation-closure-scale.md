# 0024 — What the semantic relation model costs, and why S24's closure is never materialised

- **Status:** accepted
- **Date:** 2026-08-18
- **Iteration:** 26
- **Amends:** `0023` (semantic relations and the super-property citation), which materialised four
  links and three derivations per stated link without measuring what that costs.
- **Binds:** the next build-plan item, S24 and S27. It settles *where* the transitive closure
  lives before that item chooses.
- **Extended:** iteration 40, which re-ran the whole table on a **polyhierarchy** — the shape every
  row below was measured without and which a real thesaurus has. The decision is unchanged; the
  numbers and what they now cover are in the section at the end.

## Context

`0023` closed the semantic relations under S22, S23, S25 and S26. It also made the core model hold,
for the first time, something proportional to a vocabulary's **size** rather than to its structure:
four `(Node, RelationOrigin)` entries and three [`Derivation`]s for every stated `skos:broader`.
The iteration that landed it opened an entry in `docs/UNTESTED.md` saying so, and its loop-log
"still uncertain" line asked the next iteration not to start S24 without a number in front of it:

> There is a real chance the right answer is to stop materialising and answer on read, and that is
> a decision better taken before the closure is built on top of the current shape than after.

S24 makes `skos:broaderTransitive` and `skos:narrowerTransitive` `owl:TransitiveProperty`. Its
closure is superlinear in exactly the data `0023` already multiplies by four. Measuring afterwards
would mean choosing the architecture and then discovering the number.

## What was measured

`crates/openbiz-skos/src/scale.rs`. Synthetic vocabularies of typed `skos:Concept`s in four
shapes, statements streamed into [`CoreModelBuilder`] one at a time exactly as `openbiz inspect`
streams them out of the store, so the resident-memory figure is the model and not the model plus a
copy of its input.

The shapes span the range SKOS permits, because the closure's size is a property of the
hierarchy's **shape** and not of its link count:

- **none** — every concept typed, no links. The baseline, which is what makes the other rows
  subtractable.
- **star** — one root, everything directly beneath it. Depth 1: the closure adds nothing.
- **tree** — balanced, branching 10. What a real thesaurus mostly looks like.
- **chain** — one ladder. Depth *n*: n(n−1)/2 closure pairs from n−1 links. Not realistic, and
  not meant to be — it is a **legal** SKOS graph, because §8 states no condition against depth.

The `skos:broaderTransitive` closure S24 would license is **counted by traversal and never held**,
one breadth-first walk per concept with a per-concept visited set. That is the whole trick: the
size of a structure is knowable without paying for the structure. The count is refused above
20 000 000 pairs and recorded as a refusal, never as a zero.

Release build, one process, no concurrent load, WSL2 on 12 GiB. `VmRSS` and `VmHWM` from
`/proc/self/status`; no dependency was added to weigh the thing whose weight is the concern.

## The numbers

| shape | concepts | stated links | build | RSS delta | peak RSS | held entries | derivations | `inspect` report | S24 closure |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| none  | 100 001 | 0 | 0.11 s | 72 MiB | 73 MiB | 0 | 0 | 0 | 0 |
| none  | 1 000 001 | 0 | 1.45 s | 699 MiB | 720 MiB | 0 | 0 | 0 | 0 |
| star  | 10 001 | 10 000 | 0.09 s | — | 53 MiB | 40 000 | 30 000 | 10.3 MiB | 10 000 (1.0×) |
| tree  | 10 001 | 10 000 | 0.08 s | — | 52 MiB | 40 000 | 30 000 | 10.3 MiB | 38 770 (3.9×) |
| tree  | 100 001 | 100 000 | 1.03 s | 448 MiB | 512 MiB | 400 000 | 300 000 | 103.4 MiB | 487 660 (4.9×) |
| tree  | 1 000 001 | 1 000 000 | **62.66 s** | **4 376 MiB** | **5 081 MiB** | 4 000 000 | 3 000 000 | **1 033.8 MiB** | 5 876 550 (5.9×) |
| chain | 1 001 | 1 000 | 0.01 s | — | 7 MiB | 4 000 | 3 000 | 1.0 MiB | 500 500 (**500.5×**) |
| chain | 10 001 | 10 000 | 0.08 s | — | 59 MiB | 40 000 | 30 000 | 10.3 MiB | 50 005 000 (**5 000×**) † |
| chain | 100 001 | 100 000 | 1.18 s | 514 MiB | 578 MiB | 400 000 | 300 000 | 103.4 MiB | 5 000 050 000 (**50 000×**) † |

† The count was refused above the 20 000 000 budget. The figure printed here is the arithmetic for
a chain, n(n−1)/2, which the harness verifies exactly at n = 10, n = 50 and n = 1 001.

**The RSS-delta column is blank below 100 000 links on purpose.** At 10 000 the same measurement
read +48.4 MiB in one run and +14.7 MiB in another, differing only in where it sat in the sequence:
at that size the delta is allocator warm-up, not the model. Only the rows large enough for the
model to dominate are quoted, and the peak column is given beside it so neither is mistaken for the
other. glibc need not return freed pages to the kernel, so a delta is an upper bound on what the
model holds and a lower bound on what the process needed.

### Three things the table says

**1. A stated link costs about 3.9 KiB of resident memory.** Subtracting the baseline: at 1M,
(4 376 − 699) MiB over 1 000 000 links is **3.86 KiB per link**; at 100k, (448 − 72) MiB over
100 000 is **3.85 KiB**. The two agree, so it is a marginal cost and not an artefact of one size.
The link itself is two 46-character IRIs — 92 bytes. **We spend roughly 43× the size of the fact to
record the fact.** A typed concept with no links costs 0.70 KiB, so at one link per concept the
relations are **five times the rest of the model put together**.

Where it goes, in falling order: three `Derivation`s per link, each holding two eagerly-`format!`ed
`String`s of about 120 characters (≈900 B); four `BTreeMap` entries with the IRI cloned into each
(≈390 B); and the `BTreeMap` allocation floor, which reserves an eleven-slot node for a map holding
one entry, paid once per relation per resource (≈1 KiB). The last of those is the uncomfortable
one: most of it is empty space in maps that will never hold a second entry.

**2. The realistic multiple is not a constant — it is the average depth.** The tree's closure runs
3.9× the stated links at 10k, 4.9× at 100k, 5.9× at 1M. It rises by one per decade because it *is*
log₁₀ of the size. Anyone sizing a materialised closure from a measurement at one size will
under-provision at the next.

**3. The chain settles it.** 1 000 links produce 500 500 closure pairs. 10 000 produce 50 005 000.
100 000 produce five thousand million. At the 3.9 KiB per link this model already demonstrates —
and even at a tenth of it, since a closure entry is one map entry rather than four — the last of
those is hundreds of gigabytes for a vocabulary a taxonomist could type. This is not "expensive".
It is **unbounded on an input SKOS permits**, and a graph that deep is exactly what a naïve
import from a hierarchical source produces.

## Decision

### 1. S24's closure is not materialised. It is answered on read.

`skos:broaderTransitive` and `skos:narrowerTransitive` keep holding the one-step links S22 lifted
and the ones the graph stated. Ancestry and descent are computed by traversal at the moment a
caller asks, and the model gains no per-pair storage.

Two independent reasons, and either alone would be sufficient:

- **The chain.** A design that holds only for well-shaped input is not a design. The traversal's
  cost is bounded by the answer the caller asked for; the materialised closure's cost is bounded by
  the worst hierarchy anyone can draw.
- **Explainability.** `CLAUDE.md` §3 requires every inference to answer *"why?"* with a derivation,
  and the build-plan item requires *"a derivation that names each step of the path rather than
  asserting the endpoint"*. A materialised entry is a `(Node, RelationOrigin)` pair: one rule, no
  path. It **cannot** say why `<A> skos:broaderTransitive <F>` — it can only say S24. A traversal
  produces A→B→C→…→F as a by-product of finding the answer at all. The constraint that forces the
  traversal is the same one that makes it explainable, which is the strongest form this kind of
  argument takes.

This is not a deferral of materialisation to a later optimisation. Materialising is rejected.

### 2. The traversal is bounded, and a bounded answer says so.

The measured refusal is the model. `count_closure` stops at a budget and returns "not counted"
rather than a truncated number, because `Some(0)` from an abandoned walk would read as *"this
concept has no ancestors"* for the concept with the most. The read path inherits that shape: an
answer that hit its bound is distinguishable from an answer that ran out of ancestors. A silent
truncation in an ancestry query is the same defect as a silent cap in an inference report, and
`docs/COMPETITIVE.md` records it as an incumbent's.

### 3. Cycles terminate, and are not a finding.

§8 states no integrity condition against `<A> skos:broader <B> skos:broader <A>`, so we state none
either. Termination comes from the visited set, not from a depth limit — a depth limit would give a
wrong answer on a deep legal hierarchy in order to survive a cyclic one. A concept in a cycle
reaches every concept in the cycle, which is the correct answer and is asserted as such.

## What this does not decide

The 3.9 KiB per link is about `0023`'s materialisation, **not** about S24, and it is over budget
against `CLAUDE.md` §1.5 at a million links before a single label is loaded. Three separable
reductions are visible in the decomposition above — reconstructing derivations on demand instead of
pre-rendering their text, storing one direction of each inverse pair instead of both, and using a
container without `BTreeMap`'s eleven-slot floor for maps that hold one entry. Each is a change to
a shipped public type with callers, and each is its own item.

They are recorded in `docs/PROPOSED.md` and `docs/UNTESTED.md` and are **not** taken here.
Rewriting the representation in the same change that decides where the closure lives would mean
neither decision was measured against the other, and the loop does not promote its own proposals.

The same applies to `openbiz inspect`'s 1 GiB report at a million links. That module argues at
length, and correctly, that a silent cap in an inference report is the one thing such a report must
never do. Changing it is a product decision about a different failure than the one this ADR
measures.

## Consequences

- The next item, S24 and S27, builds a traversal and not a closure. §8.6's Examples 27 and 29 are
  inconsistent only through the closure, so S27's check runs against the traversal's answer.
- `Resource::relations` continues to mean *"links under this property"* and never *"ancestors"*.
  The accessor is already named for the property rather than for the question, which was deliberate
  in `0023` and is now load-bearing.
- The harness stays. Its small case runs in the ordinary suite and asserts the arithmetic of every
  shape, so a future change to the closure passes that alters the four-entries-and-three-
  derivations ratio fails a test rather than silently changing the units of this table.
- The numbers are reproducible by anyone: `cargo test --release -p openbiz-skos -- --ignored
  --nocapture --test-threads=1`.

---

## Confirmed at iteration 40 on a shape this ADR never measured

Every row above is a **monohierarchy** — one broader concept per concept. That was not stated as a
limitation and it silently decided the decision: the closure's size is a property of the hierarchy's
shape, and a shape with one parent per concept is the cheapest branching a hierarchy can have. Six
subsequent iterations each recorded, in a different rule's words, that the generator could produce
nothing else. Iteration 40 built two branching shapes and re-ran the table.

The realistic row is a **polytree**: a balanced tree of branching 10 in which one concept in four
states extra broader links, calibrated to iteration 37's count of LC Genre/Form Terms — 25.8% of
concepts with more than one broader concept, maximum 4.

| shape | concepts | stated links | build | peak RSS | held entries | `inspect` report | S24 closure |
|---|---|---|---|---|---|---|---|
| tree (above) | 1 000 001 | 1 000 000 | 62.66 s | 5 081 MiB | 4 000 000 | 1 033.8 MiB | 5 876 550 (5.9×) |
| polytree, +1 broader on ¼ | 1 000 001 | 1 249 998 | 28.57 s | 6 413 MiB | 4 999 992 | 1 292.2 MiB | 7 595 226 (6.1×) |
| polytree, +3 broader on ¼ | 1 000 001 | 1 749 986 | 23.57 s | **8 178 MiB** | 6 999 944 | 1 809.1 MiB | 13 789 093 (**7.9×**) |

**Finding 2 survives, and branching behaves like extra depth.** The polytree's multiple runs 4.1×
at 10k, 5.9× at 100k, 7.9× at 1M — the same one-per-decade rise the tree shows, displaced upwards
by about two. So "the realistic multiple is the average depth" is still the rule; a second parent
adds to the average depth rather than doing something new to it. That is the reassuring answer, and
it is the *opposite* of what iterations 33 and 36 assumed when they reasoned that branching and
depth would compound.

**Finding 1's per-link cost is unchanged**, which is what makes the rows comparable: the model still
holds four entries and three derivations per stated link with several parents, and the ordinary
suite now asserts that against the links actually written rather than against `concepts - 1`.

**What did get worse is the absolute memory.** Quadrupling the broader links on a quarter of a
million-concept vocabulary adds 61% to the peak RSS this ADR sized its judgements against — 8.2 GiB
on a machine with 11. That is recorded in `docs/UNTESTED.md` rather than acted on here, because it
changes no decision this ADR took: the closure is still never materialised, and materialising the
13.8 million pairs the polytree licenses would be worse by exactly the margin the decision avoids.

**The decision therefore stands unchanged**, now measured against a hierarchy shaped like a real
thesaurus rather than only against one that was easy to generate.
