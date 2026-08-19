# ADR 0029 — Mapping links are lifted into the hierarchy, not kept in a section of their own

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 32
- **Extended by `adr/0030`**, which took the S45 deferral recorded below and closed it. Where
  this document says the closure is not computed and that the report says so, read `adr/0030`:
  both stopped being true at iteration 33, and the sentence it quotes from `openbiz inspect` has
  been replaced.
- **Supersedes nothing.** Extends `adr/0023` (semantic relations) and `adr/0025` (transitive
  ancestry by walking) with SKOS Reference §10, and defers one part of §10 to the rule `adr/0025`
  set.

## Context

§10 gives SKOS five mapping properties — `skos:broadMatch`, `skos:narrowMatch`,
`skos:relatedMatch`, `skos:closeMatch`, `skos:exactMatch` — and one super-property,
`skos:mappingRelation`. They are how one vocabulary says something about another's concepts without
merging the two, which is the whole of `CLAUDE.md` §1.7: an enterprise with nine overlapping
thesauri needs to see which of them are joined to anything before it authorises a tenth.

The tempting implementation is a parallel structure: mappings in their own map, their own section
of the report, their own rules. It is tempting because §10 reads like a self-contained section, and
because "external links" feels like a different kind of thing from a vocabulary's internal shape.

It is wrong, and §10.6.1 says why in its own words: there is "an intimate connection between the
SKOS semantic relation properties and the SKOS mapping properties", and it prints the sub-property
tree to prove it. S41 makes `skos:broadMatch` a sub-property of `skos:broader`. A mapping link *is*
a hierarchical link, in the model's own terms, and a build that keeps them apart is not reading the
specification — it is reading the section headings.

## Decision

### 1. §10 is closed before §8, and its links are lifted into §8's

`close_mappings` runs first: it closes S43's inverse pair and S44's three symmetric properties,
lifts every `skos:exactMatch` to a `skos:closeMatch` under S42, and then records what S41 lifts —
`broadMatch → broader`, `narrowMatch → narrower`, `relatedMatch → related` — for
`close_semantic_relations` to consume. §8's pass then closes the lifted links exactly as it closes
the graph's own, under S22, S23, S25 and S26.

Three consequences, and all three are the point:

- **`openbiz ancestors` walks through a mapping.** A concept whose parent is in somebody else's
  vocabulary has that parent in its ancestry, with the path printed. Proven end to end against the
  binary.
- **§8.4's S27 catches §10.6.2's clashes with no §10 rule at all.** Examples 59, 60 and 61 are
  marked "not consistent" by the specification and §10 states no condition that makes them so —
  they are inconsistent *because* of the sub-property tree, and Example 61's clash is two
  `skos:broadMatch` steps and a `skos:relatedMatch` between the ends, which only the transitive
  walk finds.
- **The report's hierarchy counts include mapped links**, which is what stops a heavily-mapped
  vocabulary reading as a flat list of concepts.

**This decision has a cost, and it is paid in the report rather than hidden.** Running §10 first
means a `skos:broader` link can now be entailed under S41 as well as under S25, and the existing
line in `openbiz inspect` counted *any* entailed `skos:broader` as one "stated as `skos:narrower`".
That sentence became false the moment mappings were lifted, and it was found by reading the report
the binary printed, not by any test — the whole suite was green. The two origins are now counted
apart and the lifted ones get their own line. This is the fifth iteration running to find a comment
or a line of prose that was true when it was written and was falsified by the commit that read it.

### 2. The route to `skos:Concept` runs through `skos:mappingRelation`, in printed steps

Both ends of every mapping link are `skos:Concept` (§10's Examples 54–57). The citation is not S19
quoted flatly: S19 constrains `skos:semanticRelation`, a property no author writes. So the chain is
printed — S40 up to `skos:mappingRelation`, S39 up to `skos:semanticRelation`, then S19 and S20 —
and for `skos:exactMatch` the S42 step is printed too, because **S40 does not name it**: it reaches
the super-property through `skos:closeMatch`. This is the same discipline `adr/0023` recorded for
S22-then-S21, and the reason is the same: a citation that skips a step names a statement that does
not mention the property the author used.

Two routes reach `skos:Concept` for a hierarchical mapping — the two-step mapping one and the
three-step S41/S22/S21 one — and both are true. The mapping pass runs first so the shorter citation
wins. Where the two routes reach a *link* rather than a class, the same rule applies: the converse
of a `skos:broadMatch` is cited as S41 from the converse mapping rather than as S25 from the lifted
relation, because that premise is a link the report shows in its mapping section.

### 3. S45 is not applied, and the report says so

`skos:exactMatch` is an `owl:TransitiveProperty` (S45). We do not close it. Under `adr/0025`'s
rule — materialise what is bounded by the schema, walk what is bounded by the data — a transitive
closure is a walk, and the walk for this one is a different shape from `ancestry`:

- An exact-match cluster is **undirected**. `skos:exactMatch` is symmetric *and* transitive, so the
  closure is a connected component, not a path upwards, and the derivation for "these two are
  equivalent" is a path through that component.
- §10.6.6 warns outright that "applications must be able to cope with cycles in `skos:exactMatch`
  and `skos:closeMatch`", and shows the entailment that produces one from a single statement.
- S46 then has to be checked across the closure as well as across the stated links, which is a
  second sweep with a shared budget — `adr/0027`'s finding applies to it before it is written.

That is an item, not a footnote, so it is part 2 in `docs/BUILD-PLAN.md`. What matters here is that
the gap is **stated in the product**: every `openbiz inspect` report containing a mapping prints
"S45 makes skos:exactMatch transitive and this build does not close it, so a chain of exact matches
is reported as the links it states". A test pins the absence, and its doc comment says it is to be
replaced by its opposite when the walk lands — never deleted to make a build pass.

### 4. S46 is one finding per pair, and it distinguishes the two arguments

S46 makes `skos:exactMatch` disjoint with `skos:broadMatch` and `skos:relatedMatch`. §10.4's note
extends it to `skos:narrowMatch` by symmetry and inversion. The finding carries which of the two
arguments applies, because quoting S46 flatly at a `skos:narrowMatch` clash would cite a statement
that does not mention the property in front of the reader.

The clash is visible at both ends after the closure and is **one** violation, so it is reported
from the lexicographically first end, with the origin of every link — most clashes are half
written, and an author needs to see which statement is theirs. This is the opposite choice to
S27's, where two findings mean two genuinely different violating paths, and the difference is
recorded on both.

`skos:closeMatch` is never checked against `skos:exactMatch`: S42 makes every exact match a close
match, so the two holding together is the entailment. A disjointness table that included it would
report every exact mapping in every vocabulary as a violation of the statement that produced it.

### 5. What §10 permits, we do not report

Each of these has a test asserting our silence:

- A mapping **inside one concept scheme** (Example 58). §10.6.1 calls cross-scheme use a
  *convention* and says there are no formal integrity conditions against the other case. The report
  says so in the mapping section, so a count is never read as a complaint.
- A **reflexive** mapping (Example 66). None of the five is irreflexive.
- **Cycles and alternate paths** in `skos:broadMatch` (Examples 67, 68) — which, after S41, are
  cycles in `skos:broader` that the ancestry walk must terminate on and stay quiet about.

## Consequences

- A mapped vocabulary is navigable: `openbiz ancestors` crosses the boundary, and the hierarchy
  counts include what the mapping contributed, separated from what the graph stated.
- `openbiz inspect` gains a mapping section that a vocabulary with no mappings never sees, so the
  section's presence is itself the answer to "is this thesaurus joined to anything?".
- §10 is **partially** implemented and the build says which part: S38–S44 and S46 applied, S45 not.
- Memory: a stated mapping link now costs more than a stated relation, because it produces both
  mapping-side and relation-side entries plus their derivations. `crates/openbiz-skos/src/scale.rs`
  generates no mapping links at all, so this is arithmetic and not a measurement —
  `docs/UNTESTED.md` records it as open.

## What was measured

Nothing, and that is worth saying plainly rather than leaving a heading empty. This iteration added
no bounded walk and no sweep; the work is proportional to the mapping statements read, in the same
way §8's closure is proportional to the relations read. The measurement this decision *does* need
is the one `UNTESTED.md` now carries: what a mapping costs per link at 10k and 100k, against a
generator that does not yet produce one.
