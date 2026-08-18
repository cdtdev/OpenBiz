# 0013 — What Oxigraph's query evaluation costs at 10k, 100k, and 1M concepts

- **Status:** accepted
- **Date:** 2026-08-18
- **Iteration:** 11
- **Amends:** `0006` (embedded store), which adopted Oxigraph with the evaluation risk noted but
  unmeasured, and `0011` (SPARQL query endpoint), whose limits were chosen rather than measured.

## Context

`CLAUDE.md` §3 carries a standing warning about the engine the whole product rests on:

> `oxigraph` — store and SPARQL. **Known risk:** query evaluation is explicitly not yet optimised
> upstream. Benchmark before depending on it for large-vocabulary paths.

Phase 3 builds the interface, and the interface *is* a set of large-vocabulary paths. The point of
doing this now is that the concept tree has not been written yet: a number that arrives after the
tree is built tells us how much to rewrite, and a number that arrives before it tells us what to
write.

Two entries in `docs/UNTESTED.md` were waiting on exactly this: the query limits are hard-coded and
their values — 100 000 answers, 30 seconds — were **reasoned, not measured**, and the endpoint
buffers a whole answer in memory with no measurement of what that costs at the cap.

## Decision

Measure the queries **our own interface will issue**, through **our own entry point**, at three
sizes, and publish both the harness and the numbers.

`crates/openbiz-store/src/scale.rs` generates a synthetic SKOS vocabulary, loads it through
[`Store::transaction`] — the real write choke point, not the backend's bulk loader — and times ten
probes through [`Store::query`], the same call `/api/sparql` makes. The measured time therefore
includes parsing, evaluation, **and serialising the answer**, because that is the whole of what a
caller waits for.

Two things about this differ from how this market normally reports performance, and both are
deliberate.

**It measures the product, not a benchmark suite.** BSBM and LUBM report aggregate query throughput
over a synthetic e-commerce or university dataset. Neither answers *"does the concept tree open"*,
which is the only question a taxonomist evaluating this tool actually has. Each probe here is one
interaction: draw the tree's first level, expand a node, open a concept, type in the search box,
show a breadcrumb, list a subtree.

**It is reproducible by the person who doubts it.** The generator, the queries, and the harness are
in the repo; the numbers below are what this machine produced and anyone can produce their own.
A performance number from vendor hardware you do not have, over a dataset you cannot generate, is
marketing. `CLAUDE.md` §1 says the roadmap is the repo — so is the benchmark.

### The fixture

A balanced tree: ten top concepts, ten-way branching, so depth grows as log₁₀ of the size. Each
concept carries `rdf:type`, `skos:inScheme`, an English and a German `skos:prefLabel`, an English
`skos:altLabel`, a `skos:definition`, and — except at the top — one `skos:broader`. The concept
scheme states its own top concepts with `skos:hasTopConcept`. Seven quads per concept: 1M concepts
is 7M quads.

Labels are built from seventeen adjectives and nineteen nouns, both coprime with the branching
factor, so the label vocabulary does not correlate with position in the tree and a prefix search
matches roughly one concept in seventeen, scattered.

### How the harness is kept honest

A benchmark whose queries quietly match nothing measures an empty loop very quickly. So **every
probe's answer count is asserted against the generator's own arithmetic before its timing is
believed** — ten top concepts, seven statements about a concept, 111 110 descendants of a root at
1M — and a mismatch fails the run rather than producing a fast, meaningless row. The 1 000-concept
case runs in the ordinary test suite for the same reason: the harness cannot rot unnoticed.

## What was measured

Machine: Intel Core i5-10400F (6 cores / 12 threads, 2.90 GHz), 11.7 GiB RAM, ext4 on NVMe, WSL2
on Linux 5.15.153.1. `rustc` 1.97.1, release profile (`lto = "thin"`, `codegen-units = 1`),
Oxigraph 0.5.9 with the RocksDB backend. One process, no concurrent load. Median of three timed
runs after one untimed warm-up.

### 10 000 concepts — 70 001 quads, loaded in 1.9 s (36 657 quads/s), 50 MB on disk

| Query | Answers | Median | Slowest of 3 | Shipped defaults |
|---|---:|---:|---:|---|
| `count_concepts` | 1 | 3.3 ms | 3.7 ms | served |
| `top_concepts_derived` | 10 | 89.6 ms | 97.4 ms | served |
| `top_concepts_stated` | 10 | 0.4 ms | 0.4 ms | served |
| `children` | 10 | 0.4 ms | 0.4 ms | served |
| `concept_detail` | 7 | 0.3 ms | 0.3 ms | served |
| `label_exact` | 1 | 0.2 ms | 0.2 ms | served |
| `search_prefix_first_page` | 50 | 6.4 ms | 6.7 ms | served |
| `search_prefix_all` | 588 | 23.3 ms | 23.5 ms | served |
| `ancestors` | 3 | 0.2 ms | 0.3 ms | served |
| `descendants` | 1 110 | 4.5 ms | 4.7 ms | served |

### 100 000 concepts — 700 001 quads, loaded in 26.2 s (26 685 quads/s), 503 MB on disk

| Query | Answers | Median | Slowest of 3 | Shipped defaults |
|---|---:|---:|---:|---|
| `count_concepts` | 1 | 38.6 ms | 38.7 ms | served |
| `top_concepts_derived` | 10 | 1 162.2 ms | 1 197.2 ms | served |
| `top_concepts_stated` | 10 | 0.4 ms | 0.4 ms | served |
| `children` | 10 | 0.4 ms | 0.4 ms | served |
| `concept_detail` | 7 | 0.3 ms | 0.3 ms | served |
| `label_exact` | 1 | 0.2 ms | 0.2 ms | served |
| `search_prefix_first_page` | 50 | 51.8 ms | 53.7 ms | served |
| `search_prefix_all` | 5 882 | 294.1 ms | 316.2 ms | served |
| `ancestors` | 4 | 0.3 ms | 0.3 ms | served |
| `descendants` | 11 110 | 71.2 ms | 71.8 ms | served |

### 1 000 000 concepts — 7 000 001 quads, loaded in 297.7 s (23 514 quads/s), 5 872 MB on disk

| Query | Answers | Median | Slowest of 3 | Shipped defaults |
|---|---:|---:|---:|---|
| `count_concepts` | 1 | 344.9 ms | 347.2 ms | served |
| `top_concepts_derived` | 10 | **21 589.8 ms** | 21 857.9 ms | served |
| `top_concepts_stated` | 10 | 0.6 ms | 0.6 ms | served |
| `children` | 10 | 0.5 ms | 0.6 ms | served |
| `concept_detail` | 7 | 0.3 ms | 0.3 ms | served |
| `label_exact` | 1 | 0.3 ms | 0.3 ms | served |
| `search_prefix_first_page` | 50 | 479.6 ms | 486.8 ms | served |
| `search_prefix_all` | 58 823 | 4 262.5 ms | 4 270.9 ms | served |
| `ancestors` | 5 | 0.4 ms | 0.4 ms | served |
| `descendants` | 111 110 | 1 559.8 ms | 1 568.1 ms | **refused** |

What each probe is:

| Probe | The interaction it stands for |
|---|---|
| `count_concepts` | the header count every vocabulary page shows |
| `top_concepts_derived` | the tree's first level, found by `FILTER NOT EXISTS { ?c skos:broader ?p }` |
| `top_concepts_stated` | the same answer, read from the scheme's `skos:hasTopConcept` |
| `children` | expanding one node — a lookup in the object position |
| `concept_detail` | opening one concept — everything stated about a bound subject |
| `label_exact` | resolving a label a user pasted in — a bound object literal |
| `search_prefix_first_page` | what the search box sends: `STRSTARTS` over labels, `LIMIT 50` |
| `search_prefix_all` | the same search unpaged — the cost the `LIMIT` is hiding |
| `ancestors` | the breadcrumb above a concept — `skos:broader+` upwards |
| `descendants` | everything under one branch — what a bulk edit or a report needs |

### How each one grows

Exponent fitted across the full two orders of magnitude (`t ∝ nᵏ`):

| Query | 10k → 1M | k |
|---|---:|---:|
| `children`, `concept_detail`, `label_exact`, `ancestors`, `top_concepts_stated` | ~×1.5 | ≈ 0.09 — **flat** |
| `count_concepts` | ×104 | 1.01 |
| `search_prefix_first_page` | ×75 | 0.94 |
| `search_prefix_all` | ×183 | 1.13 |
| `top_concepts_derived` | ×241 | 1.19 |
| `descendants` | ×347 | 1.27 |

## Consequences

### 1. Oxigraph is fit for the navigation paths, and that is now measured rather than hoped

Every query that binds a term — expand a node, open a concept, resolve a label, walk a breadcrumb —
is **0.2–0.6 ms at every size**, from 10 000 concepts to a million. Two orders of magnitude of data
for a factor of about 1.5 in time. The indexes do what indexes are for, and `skos:broader+` is not
the problem anyone assumes it is: five hops upward through a million-concept tree is 0.4 ms.

`CLAUDE.md` §3 requires a spike and an ADR before adopting an engine as load-bearing. This is that
ADR, and for the tree-navigation paths of Phase 3 the answer is **yes**, with the four caveats
below. The single-binary commitment does not have to be paid for with an unusable tree.

### 2. The concept tree must not find its top concepts by negation — 21.6 s versus 0.6 ms

`top_concepts_derived` is 89.6 ms at 10k, 1.16 s at 100k, and **21.6 s at a million**. It is the
single worst number here and it is the *first* query the interface issues, before anything else is
on screen.

What makes it worse than a slow query is that it **succeeds**. 21.6 s is inside the 30 s deadline
`adr/0011` set, so the endpoint does not refuse it — it returns the right ten rows after twenty-one
seconds, by which time every user has concluded the product is broken and most have reloaded, which
issues it again. A refusal would at least be legible.

The mitigation is a modelling decision rather than a tuning one, and it is total: a scheme that
**states** its top concepts with `skos:hasTopConcept` answers the identical question in **0.6 ms,
flat at every size** — around 36 000× faster at 1M. SKOS already has the predicate; the incumbents
mostly treat it as optional metadata.

Two consequences follow, and the second is the one that is easy to miss:

- Phase 2's authoring model should **maintain** `skos:hasTopConcept` as concepts are created and
  re-parented, so the assertion is always true rather than usually true. That is a Phase 2 design
  decision, so it is written to `docs/PROPOSED.md` rather than taken here (`CLAUDE.md` §7).
- Phase 3 **cannot assume the assertion exists.** A vocabulary imported from an incumbent may not
  state its top concepts at all, and a tree that silently falls back to the derived query has
  merely moved the 21 s from every vocabulary to the imported ones — which are exactly the
  vocabularies a migrating customer opens first. The interface needs to know which case it is in
  and say so, rather than discover it by hanging.

### 3. `LIMIT` does not bound the work, so type-ahead search does not scale

`search_prefix_first_page` returns fifty rows at every size and takes 6.4 ms, 51.8 ms, and
479.6 ms — **linear in the size of the graph** (k = 0.94), not in the size of the answer. The
`LIMIT` bounds what is returned and nothing else, because `FILTER(STRSTARTS(LCASE(STR(?label)), …))`
cannot use an index: every `skos:prefLabel` in the vocabulary is read, decoded, lower-cased, and
tested.

Half a second per keystroke at a million concepts, before any network. There is no SPARQL-level fix;
this needs a real text index over labels, which we do not have and which SPARQL 1.1 does not
standardise. Recorded in `docs/PROPOSED.md` and it belongs with Phase 13's search work.

### 4. Our own default row cap refuses a legitimate query at 1M — and the refusal is correct

`descendants` answers with 111 110 rows at a million concepts, against a
`QueryLimits::DEFAULT_MAX_ANSWERS` of 100 000. It is **not slow** — 1.6 s — it is **refused**.

The refusal is `adr/0011`'s design working as intended: refuse rather than truncate, because a
governance team cannot sign off rows they were never told were missing. It is still a capability a
customer does not have: at a million concepts, *"show me everything under this branch"* does not
work. The cap has to exceed the largest subtree a customer actually has, and no reasoned number can
know that. This converts the open `UNTESTED.md` entry — "the limits are hard-coded, and the defaults
are chosen rather than measured" — from a worry into a requirement with a number attached.

### 5. The 30 s deadline is a runaway guard, not an interactivity guard, and the two are not the same

Nothing measured here came within a factor of ten of exhausting 30 s except
`top_concepts_derived`, which reached 72 % of it and was served. So the deadline is defensible as
what it is — a stop on an accidental cartesian product — and useless as a promise that the
interface stays responsive. A query the user is *waiting on* needs a budget in the region of a
second, and that is a different bound applied at a different layer. Recorded in `docs/PROPOSED.md`.

### 6. Import and disk, since procurement asks

Through the transactional write path, with no bulk loader and no tuning: **23 500–36 700 quads per
second**, so a million-concept vocabulary loads in about five minutes. That is a migration number
worth having, and it is measured on the path an import will actually use rather than on a fast path
an import cannot take.

Disk is **~840 bytes per quad, linear**: 50 MB, 503 MB, and 5 872 MB. A million-concept vocabulary
therefore needs roughly 6 GB. Measured immediately after loading with **no compaction**, so it is
an upper bound and the settled figure is unmeasured — recorded in `docs/UNTESTED.md`. Nobody should
read "single binary" as "single small directory".

### 7. What this ADR does not measure

- **Concurrency.** One process, one query at a time. What the numbers do under ten simultaneous
  users is unmeasured and is Phase 13's problem; `CLAUDE.md` §8 puts hardware-bound load testing
  outside the loop.
- **Memory.** Timings only. `adr/0011` records that the endpoint buffers a whole answer twice, and
  this run did not weigh it.
- **A realistic vocabulary shape.** The fixture is a balanced ten-way tree with uniform labels.
  Real thesauri are lumpy — a handful of concepts with thousands of children, label lengths across
  two orders of magnitude, sparse translations — and a regular shape flatters an index. Read these
  as *"no worse than this, on this machine"*.
- **A cold store.** The page cache is warm by the time the probes run, because the load just wrote
  the data. First-query-after-restart is a different and unmeasured number.

## Alternatives considered

- **Run BSBM or LUBM.** Comparable to published figures for other engines, which is the one thing
  they are good for. Rejected as the *primary* measurement: neither dataset has a concept tree, and
  "how does Oxigraph compare to GraphDB on BSBM" is not a question anyone evaluating OpenBiz asks.
  Worth doing later purely as a cross-check on this machine's numbers.
- **Benchmark `oxigraph::store::Store` directly.** Isolates the engine from our serialisation and
  our default-dataset rewriting. Rejected because the number a user experiences includes both, and
  because a measurement that skips `Store::query` would not have caught the `descendants` refusal
  at all — that is our limit, not Oxigraph's.
- **Use Oxigraph's bulk loader for the fixture.** Several times faster to build the 1M case.
  Rejected because the load figure would then describe a path no import in this product takes, and
  a load-rate number nobody can reproduce through the product's own API is the kind of benchmark
  this ADR exists not to publish.
- **Assert timing thresholds in CI.** Tempting, and it would catch a regression. Rejected: a CI
  runner's timings are noise, so the assertion would either be so loose it catches nothing or so
  tight it fails randomly — and a randomly-failing performance test gets deleted or, worse,
  loosened until it is decoration. The 1 000-concept case in CI asserts *correctness* of the
  harness instead, which is the part that can silently rot.
