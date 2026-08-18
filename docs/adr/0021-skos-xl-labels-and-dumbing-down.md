# ADR 0021 — SKOS-XL labels, and where the dumbed-down label lives

- **Status:** accepted
- **Date:** 2026-08-18 (iteration 22)
- **Supersedes / amends:** extends `adr/0019` (the SKOS core model) and `adr/0020` (lexical
  labels). Changes neither decision; changes one type, noted below.

## Context

In plain SKOS a label is a literal hanging off a concept. There is nowhere to record who created
it, when it was approved, that it is deprecated, or that this acronym stands for that term.
ISO 25964 needs all of those, which is why `CLAUDE.md` §2 lists SKOS-XL as part of the authoring
model rather than as an optional extra: **plain SKOS cannot faithfully represent an ISO 25964
thesaurus**, and our buyers have ISO 25964 in their requirements.

SKOS-XL is Appendix B of the SKOS Reference, statements S47–S62. It makes the label a resource
with an IRI of its own, gives it a single `skosxl:literalForm`, and provides three labelling
properties analogous to the plain ones. The item was taken after the plain labels rather than
before them — the reason is in `adr/0020` and in iteration 21's log — because S55–S57 make the XL
properties *dumb down* to the plain ones, so the derived thing needs the thing it derives to.

`skosxl:labelRelation` (B.4, S59–S62) is **not** in this decision. It is an extension point the
specification says is "not intended to be used directly", nothing in the authoring path depends on
it, and it was split into its own build-plan item at this iteration rather than bolted on here.

## Decision 1 — a dumbed-down label goes in the same place as an asserted one, with an origin

S55–S57 make the property chain `(skosxl:prefLabel, skosxl:literalForm)` a sub-property of
`skos:prefLabel`, and the same for alternative and hidden. Example 83 states the entailment
outright. So a concept labelled only through SKOS-XL still has plain SKOS labels — but only if
something performs the entailment.

There were two places the resulting labels could go: a separate `xl_labels` view a caller opts
into, or the same `Resource::labels` map the asserted ones are in. **They go in the same map**,
each carrying a `LabelOrigin` of `Asserted` or `DumbedDown(rule)`.

That is not a convenience. B.3.4.2 says Examples 84–87 are inconsistent *because of* S13 and S14 —
conditions defined on the plain labelling properties — and those checks read that map. A build
that kept dumbed-down labels somewhere else would report Example 84 as a clean vocabulary, and
would report an XL-authored thesaurus as having no labels in any language, which is the "reports
zero where a real vocabulary has thousands" failure `adr/0019` exists to avoid.

**A type changed.** `Resource::labels()` was `BTreeMap<LexicalLabel, BTreeSet<LabelKind>>` and is
now `BTreeMap<LexicalLabel, BTreeMap<LabelKind, LabelOrigin>>`. Every caller is in this workspace
and every one was updated; the alternative — a parallel map of origins keyed the same way — has
two things to keep in step and no compiler to notice when they drift.

An asserted label is **never** overwritten by a dumbed-down one, exactly as `entail_class` never
overwrites an asserted class. The graph said it; claiming to have deduced it would be a derivation
nobody needed. A test pins this, and it caught the bug where it did not hold.

## Decision 2 — only a well-formed label dumbs down

A label dumbs down when it has exactly one literal form and that form is a plain literal. Two
forms, no form, or a form that is not a plain literal all produce nothing.

- **Two forms** — there is no principled way to choose which of them the concept is really called,
  and picking one would put a label in front of an author that nobody wrote. It is already
  reported under S52.
- **A non-plain form** — dumbing it down would produce exactly the non-plain `skos:prefLabel` that
  S12 is about, so one fault would be reported twice under two rules and the concept would appear
  to have a preferred label it cannot display.

## Decision 3 — the classification of Appendix B's statements, and whose judgement each is

§1.7 sets out the structure every section of the SKOS Reference follows, and *"Integrity
Conditions — if there are any integrity conditions, those are given"* is one of its parts. §4.4,
§5.4, §8.4, §9.4 and §10.4 each have one. **Appendix B has none**: B.2.2, B.3.2 and B.4.2 are all
headed "Class and Property Definitions". So the severity of every SKOS-XL finding is a decision
this ADR has to make and record, rather than one the specification's headings make for us.

| What | Severity | Whose judgement |
|---|---|---|
| Two different `skosxl:literalForm` values (S52) | inconsistent | **the specification's** — Examples 76, 77, 78 and 79 are each marked "(not consistent)" for exactly this |
| `skosxl:Label` also a Concept, ConceptScheme or Collection (S48) | inconsistent | ours — no example marks it, but a resource in two disjoint classes is a contradiction |
| One label resource under two XL labelling properties (S58) | inconsistent | ours — S58 is worded identically to S13, which §5.4 *does* call an integrity condition |
| A node as a `skosxl:literalForm` (S49) | inconsistent | ours — the values of an `owl:DatatypeProperty` are literals by definition |
| A literal on an XL labelling property (S53) | inconsistent | ours — the mirror of S3 and S30, already treated this way |
| A `skosxl:Label` with **no** literal form (S52) | ill-formed | ours, and deliberately not inconsistent — see below |
| A `skosxl:literalForm` that is not a plain literal (S51) | ill-formed | ours, by analogy with S12 — the analogy is ours, §5.6.2 is said about §5 and is not restated in Appendix B |

`Severity::IllFormed` means "SKOS permits it and we think it is a mistake". For a violated
disjointness that would be false, which is why the two disjointness rows are inconsistent even
though we are the ones classifying them.

### Why "no literal form" is not an inconsistency

S52 says `skosxl:Label` is a sub-class of a restriction on `skosxl:literalForm` **cardinality
exactly 1**, and it is tempting to read that as "a label with none is broken". Under OWL's
open-world assumption it is not: the restriction *entails that a form exists*, it does not require
the graph to state one. A partial export, a federated query, or a half-finished import all produce
labels whose forms are elsewhere, and calling those inconsistent would refuse valid data.

So it is reported, with the reason printed in the finding, and the vocabulary still stands. Two
forms is the other half of "exactly 1" and *is* a contradiction, because both cannot be the one
value. Getting these two halves of one axiom the same way round would be a bug; a test asserts
they are not.

### Why an IRI is a contradiction here and not under `skos:prefLabel`

This asymmetry is the specification's and not ours. **S10** makes `skos:prefLabel` an
`owl:AnnotationProperty`, and OWL 2 annotation properties take IRIs as values quite legally — so
`skos:prefLabel <http://…>` is odd, reportable, and not a contradiction, which is what `adr/0020`
already decided. **S49** makes `skosxl:literalForm` an `owl:DatatypeProperty`, whose values are
literals by definition. Same shape of mistake, two different answers, both correct.

## Decision 4 — `skosxl:Label` is a fifth `SkosClass`, not a model of its own

`SkosClass` was documented as "one of the four classes of the SKOS core model", with a note saying
`skosxl:Label` was deliberately absent. It is now a fifth variant. The reason is S48: a
disjointness check can only be run over classes that share a map, and keeping the label class
elsewhere would mean either a second disjointness pass with its own bugs or no S48 at all.

The cost is that `SkosClass::iri()` can no longer assume the SKOS namespace, so it asks the class
(`SkosClass::namespace`). A test asserts the two namespaces are not interchangeable in either
direction — `skos:Label` is not a class and `skosxl:Concept` is not one either — because reading
one as the other would put a label into the SKOS core model silently.

`skos:OrderedCollection` is absent from S48's disjointness rows on purpose. S29 has already made
every ordered collection a collection by the time the check runs, so the `skos:Collection` row
catches it, with the citation the specification actually states. A test asserts the reported
finding says `skos:Collection (inferred, S29)`.

## Consequences

`openbiz inspect` grew a `skos-xl labels:` section — how many label resources there are, how many
have exactly one literal form, how many resources are labelled through SKOS-XL, and how many plain
labels were inferred from them. It is **omitted entirely** for a vocabulary that does not use
SKOS-XL, so its presence is itself the answer to "is this thesaurus using SKOS-XL?", which is the
first question a migration asks. The `skosxl:Label` row in the class counts still prints `0`, so a
reader can see it was looked for.

The dumbed-down labels flow into the existing `languages:` coverage and into `Resource::
display_label`, so an XL-authored thesaurus is named and counted like any other. That is the
decision above, visible.

**Memory.** `adr/0020` recorded that the model holds the labels rather than counting and dropping
them, so peak memory is proportional to the label count. SKOS-XL adds one entry per label resource
(its forms) and one per (resource, label) link. For a thesaurus authored entirely in SKOS-XL that
is roughly double what plain SKOS costs, because each label exists twice — once as a resource with
a form, once as the dumbed-down plain label on the concept. Unmeasured at scale, as everything in
`adr/0013`'s range is; recorded in `docs/UNTESTED.md`.

## Evidence

Thirteen of Appendix B's own numbered examples are asserted to be what the specification marks
them: 75 (consistent), 76–79 (not consistent), 80 (non-entailment), 81 (consistent), 82 and 83
(consistent, and the entailment), and 84–87 (not consistent). Examples 88 and 89 are B.4's and
belong to the `skosxl:labelRelation` item.

The suite was shown to **discriminate** before it was trusted. Five mutations each turned it red:
the dumbing-down disabled (6 unit tests and 2 end-to-end tests), S52's multiple-form check
disabled (2), S48's disjointness rows broken (10), a non-plain literal form accepted as a label
(1), and a missing literal form reclassified as inconsistent (1).

The production caller is `openbiz inspect`, exercised against the real binary on disk with a
thesaurus that arrived through the candidate seam and that states **no plain label anywhere** —
every label in its report is entailed.
