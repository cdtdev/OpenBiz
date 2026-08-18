# ADR 0022 — links between SKOS-XL labels, and the refinement we deliberately do not close

- **Status:** accepted
- **Date:** 2026-08-18 (iteration 23)
- **Supersedes / amends:** completes `adr/0021`, which explicitly left Appendix B.4 out. Changes
  no decision in `adr/0019`, `adr/0020` or `adr/0021`; adds one type and one report line.

## Context

Appendix B.4 of the SKOS Reference defines `skosxl:labelRelation` in four statements:

- **S59** — `skosxl:labelRelation` is an instance of `owl:ObjectProperty`.
- **S60** — the `rdfs:domain` of `skosxl:labelRelation` is the class `skosxl:Label`.
- **S61** — the `rdfs:range` of `skosxl:labelRelation` is the class `skosxl:Label`.
- **S62** — `skosxl:labelRelation` is an instance of `owl:SymmetricProperty`.

It matters to us for one reason. ISO 25964 has *label* relationships — an acronym stands for a
term, a spelling variant stands beside another — and they hang off the labels rather than off the
concept, which is precisely what plain SKOS cannot express. B.4 is where the documented ISO
25964-to-SKOS mapping puts them, and `CLAUDE.md` §2 commits us to that mapping.

B.4.1 says the property "is not intended to be used directly, but rather as an extension point
which can be refined for more specific labeling scenarios", and Example 89 refines it to
`ex:acronym`. B.4.4.1 closes the appendix with one sentence that is the whole difficulty:

> Note that a sub-property of a symmetric property is not necessarily symmetric.

Appendix B states **no integrity conditions** — B.4.2, like B.2.2 and B.3.2, is headed "Class and
Property Definitions", and §1.7 makes that heading meaningful. `adr/0021` records the consequence
and it applies unchanged here: every severity in this appendix is a decision we take and write
down, and the table below says whose judgement each one is.

## Decision 1 — all four statements are applied, and none of them creates a new finding kind

S59 reuses `Finding::LiteralOnObjectProperty`, exactly as S3 and S30 do for `skos:inScheme` and
`skos:member`: an object property given a literal is a contradiction, and the value is dropped
rather than kept as a link to a literal.

S60 and S61 entail `skosxl:Label` at both ends of a link, through the same `entail_class` path
S50 and S54 use. Nothing new is reported *by* them — but they are what makes a mistake visible.
A `skosxl:labelRelation` pointing at a `skos:Concept` is caught by **S48**, because the concept is
now also a label and those classes are disjoint. Without S60/S61 the same graph reads as clean,
since nothing else in it types the concept as a label. That cascade is the reason to apply a
domain and range rule rather than merely quote it, and an end-to-end test asserts the report
quotes both statements: S61 for why the concept became a label, S48 for why that is a
contradiction.

The severities, and whose judgement each is:

| Case | Severity | Whose judgement |
|---|---|---|
| A literal as the object of `skosxl:labelRelation` (S59) | `Inconsistent` | **Ours**, by the same reading S3 and S30 already have — the values of an `owl:ObjectProperty` are not literals |
| A link that puts a resource in `skosxl:Label` and a disjoint class (S60/S61 then S48) | `Inconsistent` | **Ours** for the classification, as `adr/0021` records for S48; the contradiction is not |
| A label established only by a link, carrying no literal form (S60/S61 then S52) | `IllFormed` | **Ours**, unchanged from `adr/0021` — "cardinality exactly 1" entails a form exists, it does not require the graph to state one |
| A link from a label to **itself** | *not reported* | **Ours.** `owl:SymmetricProperty` says nothing against a reflexive pair and neither do we |

That last row is a decision and not an oversight. A self-link is very likely an authoring mistake,
and it would have been easy to report it — but Appendix B does not forbid it, and inventing an
integrity condition the specification does not state is the failure `docs/COMPETITIVE.md` records
against the incumbents. If it turns out to matter, it belongs in a SHACL rule pack in Phase 4,
where a customer can switch it off, not in the model where they cannot.

## Decision 2 — the symmetric converse is stored beside the asserted link, carrying its origin

S62 makes the property symmetric, so `<A> skosxl:labelRelation <B>` entails the converse. The
converse goes into the **same map** as the asserted direction, keyed by the other label, carrying
a `RelationOrigin` — the third origin type in this crate, after `ClassOrigin` and `LabelOrigin`,
and for the third time the same reason: a caller that cannot tell the graph's statement from ours
has an answer and not an audit trail.

Two consequences, both tested:

- A graph that states **both** directions gets two asserted links and **no** derivation. It said
  so; claiming to have deduced it would be a derivation nobody needed. This is the same rule
  `entail_class` and the dumbing-down pass already hold, and iteration 22's log records breaking it
  once, so it is asserted rather than assumed.
- A link from a label to itself **is** its own converse, so it is inserted once and entails
  nothing.

The alternative — a separate "inferred links" view — was rejected for the reason `adr/0021` gives
for the dumbed-down labels: a caller asking "what is this label related to?" wants one answer, and
a model that makes them consult two places will have callers that consult one.

## Decision 3 — the class rules read the graph's own statements, before the closure runs

`apply_xl_class_rules` reads `label_relations` as the graph stated them and runs *before*
`close_label_relations`. So each end is classified by a statement that is actually in the file —
the subject under S60, the object under S61 — rather than half of them resting on a converse we
inferred a moment earlier. Both citations would be sound; the one-step one is the one a person can
check against the specification. This is the same choice `apply_scheme_rules` records for S5
against the S8-then-S7-then-S4 route, and it is recorded again because the ordering looks
arbitrary and is not.

Ordering also matters in the other direction: the class rules must precede `resolve_literal_forms`,
because a label established *only* by a link is still a label with no literal form and the report
should say so.

## Decision 4 — a refinement of `skosxl:labelRelation` is invisible to us, not mis-inferred

This is the trap B.4.4.1 warns about. "FAO" is an acronym for "Food and Agriculture Organization";
the converse is false. So Example 89's `ex:acronym` must **never** be closed, even though the
property it refines is symmetric.

We read no `rdfs:subPropertyOf` anywhere in this crate, so a refinement is simply not seen — which
is the safe half of the answer, and it is asserted rather than left to chance: the Example 89 test
states the `rdfs:subPropertyOf` axiom, uses the refined property, and asserts that no `ex:acronym`
statement is invented in either direction. The day sub-property reasoning arrives, it arrives
against an assertion that already says what it must not do.

The **unsafe** half is what we do not yet do, and it is recorded in `docs/UNTESTED.md` rather than
quietly omitted. The sound inference from Example 89 is that `<B> ex:acronym <A>` entails
`<B> skosxl:labelRelation <A>`, which S62 then closes to `<A> skosxl:labelRelation <B>` — the
*super*-property is symmetric even though the refinement is not. We make neither step, so a
thesaurus whose ISO 25964 label relationships are expressed through refinements reads to us as a
thesaurus with no label relationships at all. That is the ordinary way B.4 is used in practice, so
the gap is larger than the four statements suggest and the ledger says so in those words.

## What was measured

- 14 new tests: 11 unit and 3 end-to-end through the real binary against a store on disk. Two of
  them are the appendix's own numbered examples — 88 asserted consistent and closing under S62,
  89 asserted consistent and **not** closing.
- The suite was proven to discriminate before it was trusted. Five mutations, each turned it red:
  the S62 closure disabled (3 failures), S60 and S61 not entailing (3), the "already stated"
  guard removed so an asserted link is restated as an inference (2), `skosxl:labelRelation` not
  read at all — the iteration-22 behaviour — (9), and links counted as ordered pairs rather than
  as links (1).
- 446 Rust tests and 30 UI tests pass; `fmt`, `clippy -D warnings` and `cargo deny` are green.
- No new dependency.

## Consequences

- `Resource::label_relations()` is a new public accessor returning a map from the other label to a
  `RelationOrigin`. The store format is unchanged: this is derived at read time, nothing is
  persisted, and no migration is needed.
- `openbiz inspect` gains one line inside the existing `skos-xl labels:` section, printed only
  when a vocabulary has links. It counts **links**, not statements — S62 closes every link into a
  pair, so summing the relations each resource holds would report twice what an author wrote.
- The export gap `adr/0021` opened widens by one more entailment: an inferred converse is in our
  answers and not in the file `openbiz export` hands out. It is the same defect, already raised in
  `docs/PROPOSED.md` with its urgency noted, and this is a third instance rather than a new one.
