# ADR 0020 — SKOS lexical labels, and what "plain literal" means in RDF 1.1

- **Status:** accepted
- **Date:** 2026-08-18 (iteration 21)
- **Supersedes / amends:** extends `adr/0019` (the SKOS core model); does not change it.

## Context

SKOS Reference §5 defines three lexical-labelling properties — `skos:prefLabel`, `skos:altLabel`,
`skos:hiddenLabel` — and puts two of the specification's integrity conditions on them. Labels are
the part of a vocabulary a subject-matter expert actually sees, and they are where a thesaurus
merged from two sources is first found to be inconsistent. `openbiz-skos` modelled the four core
classes and nothing lexical; `openbiz inspect` could tell you a vocabulary had 40,000 concepts and
not what a single one of them was called.

Three decisions had to be made, and only the first is obvious.

## Decision 1 — report exactly the two integrity conditions the specification states, and no more

§5.3 gives three class-and-property definitions (S10, S11, S12). §5.4 — a heading that says
**"Integrity Conditions"** — gives two: **S13**, that the three properties are pairwise disjoint,
and **S14**, that a resource has at most one `skos:prefLabel` per language tag. Appendix B.3.4.2
independently restates the same two and calls them "the two integrity conditions … defined on the
basic SKOS labeling properties", which is the confirmation that there is no third hiding elsewhere
in the document.

So `Severity::Inconsistent` is used for S13 and S14 and for nothing else in §5. In particular:

- **S12 — the range is "the class of RDF plain literals" — is not an integrity condition.** §5.6.2
  says so with unusual directness: *"If a graph does not follow this usage convention an
  application may reject such data but is not required to."* We report it as `IllFormed` and read
  on. Refusing it would be the failure `docs/COMPETITIVE.md` records against the incumbents:
  valid enterprise data turned away by a tool stricter than the standard it claims to implement.
- **A resource with no preferred label is not a finding at all.** §5.6.4 states outright that
  alternatives without a preferred label are consistent. It is still the first number a governance
  team asks for, so `openbiz inspect` prints it as a *count* — a fact about the vocabulary, in the
  section about labels, not in the section about what is wrong with it. Whether Z39.19 or ISO
  25964 want it as a rule is Phase 5's question, and those packs are allowed an opinion SKOS does
  not have.
- **S11 — the three properties are sub-properties of `rdfs:label` — is deliberately not applied.**
  It is sound, and entailing it would emit one derivation per label in a report that already
  prints every derivation. Nothing in this build reads `rdfs:label`, so the entailment would cost
  a report the size of the vocabulary to produce a fact no caller consumes. If something ever
  reads `rdfs:label`, this is the paragraph to come back to.

## Decision 2 — "RDF plain literal" is read as RDF 1.1's two replacements for it

This is the one that needed the specification open rather than remembered.

SKOS was published in 2009 against RDF 1.0, whose *plain literal* was a lexical form with an
optional language tag. **RDF 1.1 abolished the term.** The string "plain literal" does not appear
anywhere in RDF 1.1 Concepts. §3.3 of that document defines the two things it became:

- a **language-tagged string**, whose datatype IRI is always `rdf:langString` and which carries a
  non-empty BCP 47 tag; and
- a **simple literal**, which is *"syntactic sugar for abstract syntax literals with the datatype
  IRI `xsd:string`"*.

Since Oxigraph implements RDF 1.1, every "plain literal" reaching us is one of those two, and
nothing else is one. `LexicalLabel::of` accepts exactly that pair. Consequences worth stating
because they are decisions and not accidents:

- `"4"^^xsd:integer` under `skos:prefLabel` is **not** a label. It raises S12's finding and is
  then **discarded** rather than kept in some other bucket. S13 asks whether two properties carry
  the same label and S14 asks how many preferred labels a language has; a term that is neither a
  string nor language-tagged has no answer to either, and inventing a bucket for it would mean
  reporting clashes the specification does not describe. A test pins this: the same typed literal
  under two properties is two S12 findings and **no** S13 clash.
- A resource whose *only* SKOS statements are refused labels does not appear in
  `CoreModel::resources()` at all. The findings name it; `resources()` keeps its documented
  meaning of "what the model has something to say about". Both halves are asserted, because
  together they are the honest answer — the graph mentioned it, and we learned nothing about it.
- An IRI or blank node under a labelling property takes the same path. It is the mistake behind
  every "the label displays as a URL" bug, and it is `IllFormed`, not fatal.

The alternative — treating any literal's lexical form as the label and ignoring the datatype —
was rejected. It would silently make `"4"^^xsd:integer` and `"4"@en` the same label, which is a
term-equality claim RDF does not make, and it would produce S13 and S14 findings that no reading
of the specification supports.

## Decision 3 — language tags are compared lower-cased, in this crate, not upstream

RDF 1.1 Concepts §3.3: *"Lexical representations of language tags MAY be converted to lower case.
The value space of language tags is always in lower case."* So `@EN` and `@en` are the same tag,
two labels so tagged are the same RDF term, and a resource carrying both as preferred labels
violates S14.

Oxigraph normalises tags on the way in — `openbiz-store`'s own tests record that. **We do not rely
on it.** `openbiz-skos` depends on no engine by `adr/0019`, precisely so that a parsed file, a
discovery match, or an agent's proposal can be classified without a store existing. Any of those
can hand us `@EN`. The lower-casing therefore happens in `LexicalLabel::of`, and a mutation test
confirms it: reverting it to `clone()` turns three tests red.

`to_ascii_lowercase` and not `to_lowercase`: BCP 47 tags are ASCII, and `to_lowercase` is
locale-shaped in ways that would mangle a tag containing `I`.

The comparison is on the **whole tag**, and this is what most of the specification's own examples
are about:

- Example 18 — `"color"@en`, `"color"@en-US`, `"colour"@en-GB` on one resource is **consistent**.
  A model comparing primary subtags would refuse it.
- Example 19 — `skos:prefLabel "love"@en` beside `skos:altLabel "love"@en-GB` is **consistent**.
  A model comparing lexical forms alone would refuse it.
- Example 11 — four Japanese script tags, four preferred labels, consistent.

§5.6.5 *suggests* an application implement BCP 47's "lookup" fallback when matching a request for
a language. `Resource::preferred_label_in` deliberately does not: it is a presentation policy, it
needs a configured preference order to be useful, and doing it silently would mean a caller asking
for French sometimes getting English with no way to tell. Recorded in `docs/UNTESTED.md`.

## What this costs, measured against what `adr/0019` promised

`adr/0019` said the model keeps "what is proportional to the resources the model has something to
say about rather than to the size of the graph — a vocabulary's labels and notes, which are most
of its statements, are counted and dropped". **That is no longer true of labels, and this ADR is
where that is admitted.** Keeping them is not optional: S13 and S14 are per resource, statements
arrive in whatever order the scan produces, and there is no point at which a resource is known to
be complete, so every label has to be held until the build.

What is held is the minimum the two conditions need — per resource, a map from the distinct label
to the set of properties carrying it. A label repeated under two properties is stored once. No
lexical form is duplicated across the kinds.

The report was shaped around the same constraint. Every other section of `openbiz inspect` is
bounded by the *structure* of the vocabulary; labels are bounded by its *size*. So labels appear
as **coverage counts per language** plus one number, and never as a list. The section is the same
size for a ten-concept glossary and a million-concept thesaurus.

**Unmeasured:** peak memory for `openbiz inspect` on `adr/0013`'s 100k and 1M stores now scales
with the label count, and no run has been made. In `docs/UNTESTED.md`.

## Consequences

- `openbiz inspect` gained a `languages:` section and now names schemes and collections by their
  label. That section prints even when it has nothing to list, because the case that reaches it
  empty is a vocabulary whose labels were *all* refused under S12 — the one time the unlabelled
  count matters most, and the one time a missing section would read as "nothing to say".
- `LabelKind` and a `Label` struct had been sitting in `openbiz-skos/src/lib.rs` since Phase 0
  with no caller. `LabelKind` is now real and lives in `labels.rs`; the `Label` struct is deleted.
- The display label chosen when no language is asked for is deterministic but arbitrary across
  languages, so every caller prints the tag beside it. A configured display-language order is a
  separate decision and is not guessed at here.

## Evidence

Eighteen tests in `openbiz-skos`, of which ten are the SKOS Reference's own numbered examples
(10, 11, 12, 13, 14, 15, 16, 17, 18, 19) asserted to be what the specification says they are; five
in `openbiz-server`'s `inspect` against a real store; and one end to end against the binary on
disk, where a second import lands a duplicate preferred label and the report finds it.

Four mutations each turned the suite red before it was trusted: S13's check disabled (3 tests),
S14's check disabled (3), the language tag left un-lower-cased (3), and every literal accepted as
a label (5).
