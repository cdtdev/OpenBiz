# ADR 0028 — Reading a vocabulary's own note refinements, in a second pass

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 31
- **Supersedes nothing.** Extends `adr/0026` (documentation properties) with §7.1's extension
  point, and is the third answer in the sequence `adr/0024` / `adr/0025` started: *materialise or
  walk?*

## Context

SKOS Reference §7.1 says the seven documentation properties "provide a set of extension points for
defining more specific types of note". An enterprise thesaurus takes that offer routinely, because
ISO 25964's *scope note* and a house style guide's *usage note* are different things that both need
to survive a round trip:

```turtle
ex:usageNote rdfs:subPropertyOf skos:scopeNote .

ex:Chemistry a skos:Concept ;
  ex:usageNote "Use for the discipline, not the school subject."@en .
```

Under RDFS entailment pattern `rdfs7` that entails `ex:Chemistry skos:scopeNote "…"`, and S17 then
entails a `skos:note`. Until this iteration we read **no `rdfs:subPropertyOf` at all**, so the
statement was counted among the non-SKOS ones and dropped: the concept reported as undocumented,
the coverage table showed zero scope notes, and nothing in the report hinted that it had looked past
something. `UNTESTED.md` recorded it from iteration 29.

## Decision

### 1. Two passes over the source, not one pass with a buffer

`CoreModelBuilder` is a one-pass stream and a declaration may arrive after every statement that uses
it — RDF has no document order and a store has none at all. So a single pass can only do this by
buffering every statement whose predicate it does not recognise until the declarations are in.

On a graph carrying `dct:created`, `foaf:name`, and an organisation's own metadata, that means
holding most of the graph to find the handful of statements that turn out to matter. `openbiz
inspect`'s own documentation promises "the statements stream out of the store one at a time and into
the model, so peak memory is the model rather than the graph", and buffering breaks exactly that
promise.

The first pass reads `rdfs:subPropertyOf` and discards everything else, so what it holds is the
**property graph**: the number of properties a vocabulary declares, not the number of statements it
makes with them. `crates/openbiz-server/src/inspect.rs::read` is the single place both `openbiz
inspect` and `openbiz notes` go through, so the two commands cannot disagree about what a vocabulary
says.

**What it costs:** a second full scan of the graph. That is a real price and it is paid on every
`inspect` and every `notes`, including on the vast majority of vocabularies that declare no
refinements at all — the first pass cannot know that until it has finished. It is unmeasured, and
`UNTESTED.md` says so.

### 2. Materialised, where `adr/0025` walks — and the two are the same arithmetic

`adr/0025` refused to materialise S24's transitive closure and walks the hierarchy on demand,
because the closure is unbounded, graph-controlled, and its derivation *is* a path. This
materialises the refinement resolution into a map. The rule behind both:

> **Materialise what is bounded by the schema. Walk what is bounded by the data.**

A concept hierarchy is data — a million concepts is ordinary. A property hierarchy is schema — a
thesaurus declaring ten of its own note properties is normal and one declaring ten thousand has a
modelling problem this tool cannot fix. The resolution runs once per vocabulary read, not once per
concept, so the cost does not multiply by anything.

### 3. The budget is shared across the resolution

`RefinementBound { max_properties: 10_000, max_steps: 100_000 }`, and **`max_steps` is spent across
the whole resolution rather than per property**. This is `adr/0027`'s finding applied before the
fact rather than after it: a per-item budget applied to a sweep over items is not a bound, and the
defect iteration 30 found had a prose comment describing a limit the code did not impose.

`refinement::tests::the_step_budget_is_shared_across_every_property` asserts it directly — twenty
properties, a budget of five, five resolved and fifteen reported unresolved — and it was proven to
fail against a per-property mutant before it was trusted.

When the bound is reached, `Finding::RefinementBoundReached` is raised at `Severity::Unchecked`.
It is explicitly **not** a statement about consistency: §7 states no integrity condition, so a
refinement can never make a graph inconsistent. What it says is that the documentation counts are a
**floor rather than a total**, because statements made with the unresolved properties were read as
non-SKOS and dropped. `is_consistent()` stays true; `checks_are_complete()` goes false.

### 4. The derivation cites RDFS, not SKOS

`Derivation.rule` was `SkosRule`. It is now `Rule`, which is `Rule::Skos(SkosRule)` or
`Rule::Rdfs(RdfsRule)`. Citing an S-number for an entailment SKOS does not state would be a guess
wearing a citation — the failure mode this crate spends most of its comments avoiding. §7.1 offers
the extension point and RDF 1.1 Semantics §9.2.1 says what follows from using it, so the derivation
names `rdfs7` and quotes it.

A chain longer than one declaration gets a **second** derivation, citing `rdfs5`, for the composed
sub-property statement:

```
<…/houseNote> rdfs:subPropertyOf skos:scopeNote
    because <…/houseNote> rdfs:subPropertyOf <…/usageNote>,
        and <…/usageNote> rdfs:subPropertyOf skos:scopeNote
    and rdfs5: …
```

Without it, a reader checking the explanation against their own Turtle would look for a statement
the file does not contain. It is emitted **once for the vocabulary** rather than once per note — the
conclusion is about two properties and mentions no concept — and only for a refinement some
statement actually used.

### 5. Three things deliberately not done

- **A graph's own copy of S17 is read, counted, and not used.** A vocabulary that imports the SKOS
  ontology carries `skos:definition rdfs:subPropertyOf skos:note` as a statement. Deriving from that
  copy would make a conclusion's explanation depend on whether the customer imported the ontology.
  S17 answers for those edges; the edge is skipped.
- **`skosxl:labelRelation`'s refinement is still not read.** It was meant to be the same mechanism,
  and the resolution here is written against a target set rather than hard-wired so it has somewhere
  to land — but B.4.4.1 warns that "a sub-property of a symmetric property is not necessarily
  symmetric", so that case needs a decision about closure this item does not make. The `UNTESTED.md`
  entry from iteration 23 stays open.
- **Refinements are opt-in at the call site.** A builder given none behaves exactly as it did
  before this ADR, and `model::tests::without_the_first_pass_a_refinement_entails_nothing` asserts
  it. Without that test, a build that read refinements unconditionally would pass every other test
  here.

## What was measured

Nothing was benchmarked, and this ADR is weaker than `adr/0024` and `adr/0027` for it. What exists
is arithmetic and a proof by mutation:

- The resolution is `O(edges)` in the declared property graph, once per vocabulary read, with a
  shared ceiling of 100 000 edges.
- Two mutants were run and both were caught: disabling the refinement arm in `push` fails five tests
  across two crates, and making the step budget per-property fails two.
- **The second scan of the store is unmeasured.** So is the resolution against a real extended
  thesaurus, because there is no such fixture here. Both are in `UNTESTED.md`.

## What the binary found that the tests did not

The unit and integration tests were all green when the feature was first run against the binary on
disk, and the report printed this against the entailed `skos:note`:

```
    because no asserted note was recorded, which is a defect in this report
```

`stated_under` in `openbiz notes` rendered the premise of an S17 lift by looking for asserted notes
only, and S17 had just acquired a second way to fire. The fallback string was written as
unreachable, with a comment explaining why — and the comment's reasoning had been made false by the
same commit that was reading it. Fixed with a failing test first, and recorded here because it is
the second data point in three iterations for "a correct comment becomes a wrong one when the code
under it grows a case", after iterations 28, 29 and 30 each found one.

## Consequences

- An extended thesaurus reads as documented. `openbiz notes` shows the refined property, the
  declaration, and the rule; `openbiz inspect` counts refined notes apart from S17 lifts and
  **names** the properties, because a number an author cannot check against their own file is not a
  report.
- Every read of a vocabulary now scans it twice.
- `Derivation.rule` is a wider type. Existing comparisons still read `derivation.rule ==
  SkosRule::S8`, via `PartialEq<SkosRule> for Rule`.
- The mechanism generalises to any target property set, so `skosxl:labelRelation` — and Phase 4's
  rule packs, if they want it — have somewhere to attach.
