# ADR 0019 — The SKOS core model is engine-free, entails only what it can quote, and says which findings are ours

**Status:** accepted (2026-08-19) · **Phase:** 2

## Context

Everything Phase 2 builds — labels, semantic relations, the concept tree, search, bulk operations —
answers a question *about concepts*, and until now nothing in the build knew what a concept was.
The store holds quads; `openbiz-skos` held a label-kind enum and two integrity helpers. Between
them there was no answer to "what is in this vocabulary?".

Three decisions were open, and each of them is the kind that is expensive to reverse once four
other items are built on top.

## Decision

### 1. The model is engine-free, and the duplication that costs is accepted

`openbiz-skos` does **not** depend on `openbiz-store`, and `openbiz-store` does not depend on
`openbiz-skos`. The model reads its own owned `Statement` type; the store hands out its own
borrowed `StatementRef`. `openbiz-server` — the composition root — maps between them in three
lines (`crates/openbiz-server/src/inspect.rs`).

That is a duplicated statement type and it was chosen over the two alternatives:

- **Domain depends on storage.** Then classifying a graph requires opening a RocksDB store, and the
  model's tests become integration tests measured in seconds. Worse, a discovery match or a parsed
  file — neither of which is in a store — could not be classified at all without first writing it
  to one.
- **Storage depends on domain.** Then the store knows about SKOS, and the next domain (OWL, DCAT,
  SKOS-XL) either goes in the same crate or gets the same treatment.

`CLAUDE.md` §3 says a third-party engine never crosses into application code. This is that rule
applied downwards as well: our *own* storage engine does not cross into the domain either. The
practical test is that `CoreModel::from_statements` takes a literal array, so all 31 of the model's
tests are pure and run in under a millisecond — and the same code will classify a candidate's
staging graph, a parsed file, and a discovery result without any of them being a vocabulary yet.

The store's side of this is a new exit: `Store::for_each_statement` streams one graph's statements
to a closure as borrowed strings. Without it a caller wanting to *reason* about a graph would have
to `export_graph` to bytes and parse them back — carrying a parser to read our own store.

### 2. It entails the axioms it can quote, and refuses the one it cannot

A graph that says `<C> skos:inScheme <S>` and never types `<S>` still has a concept scheme; S4 says
so. A reader that counted only `rdf:type` would report **zero** schemes for a large fraction of
real vocabularies. That is not a conservative answer, it is a wrong one, and it is the answer that
makes a tool look broken on the customer's first import.

So the model applies exactly those SKOS axioms that bear on class membership or define a property
in terms of another: S4, S5, S6, S7, S8, S29, S31, S33, S36. Each is quoted in full in the source,
and **every derived fact carries its premise and its rule** — `CoreModel::derivations()` is the
answer to §3's "never add an inference path that cannot explain itself", and `openbiz inspect`
prints it:

```
<…/scheme> rdf:type skos:ConceptScheme
  because <…/scheme> skos:hasTopConcept <…/apac>
  and S5: The rdfs:domain of skos:hasTopConcept is the class skos:ConceptScheme.
```

**S32 is deliberately not applied.** It gives `skos:member` a range that is the *union* of
`skos:Concept` and `skos:Collection`, and a union entails membership of neither disjunct. Inferring
either would be a guess wearing a citation, which is worse than not inferring at all — because the
derivation would make it look checked.

### 3. "Inconsistent" and "ill-formed" are different words and are not blurred

The SKOS Reference puts a handful of statements under the heading **integrity condition**. Among
the core classes only S9 (`skos:Concept` disjoint with `skos:ConceptScheme`) and S37
(`skos:Collection` disjoint with both) are among them. A graph violating one is not a SKOS
vocabulary, and we say so.

Everything else we report is **our judgement**, labelled as ours. A cyclic or truncated `rdf:List`
behind `skos:memberList` is the case that matters: [RDF-SEMANTICS] §3.3.3, cited by SKOS §9.6.2,
explicitly permits a semantic extension to impose well-formedness restrictions on the collection
vocabulary. We impose one, and report it as `Severity::IllFormed` rather than dressing it up as the
specification's ruling.

The sharpest case is **two `skos:memberList` values on one resource**. S35 makes `skos:memberList`
an `owl:FunctionalProperty`, so this looks like a violation and is not: §9.6.2 and Example 43
explain that without also asserting the two lists are different objects, OWL merely concludes they
are the same one. We report it, and we say in the same breath that SKOS permits it. Getting this
backwards is how a tool ends up refusing valid enterprise data — which is precisely the complaint
`docs/COMPETITIVE.md` records against the incumbents.

A defective list also **entails nothing**. S36 is about "every item in the list", and a list that
could not be read to the end has no known set of items. The report says how many items were read
before the defect and that no `skos:member` was inferred from them: an honest half-answer rather
than a confident wrong one.

## Consequences

- `openbiz inspect <graph>` is the production caller. It reads and nothing else — there is no
  authentication objection to answer, unlike `import`, `approve`, and the deferred SPARQL Update —
  and an end-to-end test asserts the store is byte-for-byte unchanged afterwards.
- It prints **every** derivation, with no cap. A cap would read as "that is all there was", which
  is the one thing a report about inference must not imply.
- An unregistered IRI is refused rather than reported as an empty vocabulary, so a typo cannot read
  as "that thesaurus exists and is fine".
- The model holds one `Resource` per subject in memory. That is fine for the vocabulary sizes
  `adr/0013` measured for navigation and is **not** measured for the 1M-concept case; recorded in
  `docs/UNTESTED.md` rather than assumed.
- Labels, semantic relations, mapping properties, and SKOS-XL are **not** here. Each is its own
  build-plan item, and a placeholder for any of them would make `CoreModel` report on something it
  does not understand.

[RDF-SEMANTICS]: https://www.w3.org/TR/rdf-mt/
