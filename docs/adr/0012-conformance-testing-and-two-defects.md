# 0012 — Conformance testing against the specifications' own text, and the two defects it found

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 10 (blind-spot pass)
- **Supersedes:** nothing. **Amends:** `0010` (serialisation and export), whose conformance claim
  this ADR narrows.

## Context

`CLAUDE.md` §4.5 requires a standards claim to be backed by a test against the specification's own
examples or test suite. Until this iteration, every test of the six serialisations was a **round
trip**: a graph serialised by Oxigraph and re-read by Oxigraph, with the statements required to
come back the same. That is a real property — it has killed real bugs, and four mutants of the
serialiser were confirmed to break it in iteration 8 — but it is **self-consistency, not
conformance**. A writer and a reader sharing a misreading of a grammar agree with each other
perfectly and hand a third party a file it cannot read.

Iterations 8 and 9 both closed with this as their "still uncertain" line. Two consecutive
iterations naming the same doubt is what the loop log's non-convergence signal exists to surface,
so this pass took it as the item.

## Decision

Test the exports against a reader **we wrote, from the published grammars**, sharing no code with
the writer under test.

Two of the six syntaxes are covered: **N-Triples and N-Quads**. They were chosen because their
grammars are small enough to transcribe faithfully and to be *reviewed against the specification*
by a human — seven productions and eight terminals for the pair. Turtle, TriG, RDF/XML and JSON-LD
are not covered and their round trip remains self-consistency only.

The checker lives in `crates/openbiz-store/src/spec_conformance.rs`, is `#[cfg(test)]`-only, and
reports two independent things:

- **grammar conformance** — is this a document of the language, per the EBNF in
  [N-Triples §7](https://www.w3.org/TR/n-triples/#sec-grammar) and
  [N-Quads §4](https://www.w3.org/TR/n-quads/#sec-grammar), plus the normative absolute-IRI
  requirement of [N-Triples §2.2](https://www.w3.org/TR/n-triples/#sec-iri) which is stated in prose
  rather than in the EBNF;
- **canonical-form conformance** — the five layout constraints of
  [Canonical N-Triples §4](https://www.w3.org/TR/n-triples/#canonical-ntriples), reported separately
  because a non-canonical document is still perfectly legal and conflating the two would let a
  layout complaint masquerade as a syntax error.

The fixtures are the specification's own. [N-Triples Example 3](https://www.w3.org/TR/n-triples/#sec-literals)
is transcribed verbatim and its published lines are compared against our bytes.

**A checker nobody has broken proves nothing**, so twenty-one documents each violating exactly one
named production or one named §4 constraint are required to be rejected, and a canonical document
is required to be accepted. Those negative tests are the reason to believe the positive ones.

## What was measured

Both findings below are **defects in what OpenBiz ships**, found by this method and invisible to the
round trip. Neither is fixed here; both are pinned as executable assertions that turn red if the
behaviour changes in either direction, owned in `docs/UNTESTED.md`, with the work in
`docs/PROPOSED.md`.

### 1. The store rewrites the lexical form of literals it can interpret

[RDF 1.1](https://www.w3.org/TR/rdf11-concepts/#section-Graph-Literal) defines a literal as the pair
(lexical form, datatype IRI). The store decodes the lexical form into a value on the way in and
re-renders it on the way out, so **a different term comes back from the one that went in**:

| written | read back |
|---|---|
| `"1.663E-4"^^xsd:double` | `"0.0001663"^^xsd:double` |
| `"1.0E1"^^xsd:float` | `"10"^^xsd:float` |
| `"007"^^xsd:integer` | `"7"^^xsd:integer` |
| `"+7"^^xsd:integer` | `"7"^^xsd:integer` |
| `"007"^^xsd:nonNegativeInteger` | `"7"^^xsd:nonNegativeInteger` |
| `"4.00"^^xsd:decimal` | `"4"^^xsd:decimal` |
| `"1"^^xsd:boolean` | `"true"^^xsd:boolean` |
| `"2026-08-19T00:00:00+00:00"^^xsd:dateTime` | `"2026-08-19T00:00:00Z"^^xsd:dateTime` |

Measured to survive untouched: `xsd:string`, a datatype the engine has never heard of
(`http://acme.example/datatype/ProductCode`), a lexical form already in canonical form, and — the
perverse one — **a value that is invalid for its datatype**. `"abc"^^xsd:nonNegativeInteger` comes
back byte-for-byte while `"007"^^xsd:integer` does not. The store is faithful to what it cannot
interpret and lossy with what it can.

Three things make this worse than "the store normalises numbers".

1. **It loses statements, not just spellings.** Two triples differing only in their object's
   lexical form are two distinct triples in RDF. Written together, one comes back. The graph a user
   gets out is smaller than the one they put in.
2. **It is silent.** Nothing in the API, the export, or the interface says it happened — the exact
   failure `RdfSyntax::records_graph_names` exists to prevent for a different kind of loss, and the
   thing `adr/0010` built its wedge on.
3. **Zero-padded codes are the normal case in this market.** A `skos:notation` of
   `"007"^^xsd:integer` is what an enterprise classification scheme actually carries.

### 2. Our N-Triples is one constraint short of canonical

Canonical N-Triples §4: "ECHAR MUST NOT be used for characters that are allowed directly in
`STRING_LITERAL_QUOTE`." A tab is allowed directly. Our writer emits `\t`.

Nothing is lost — the document is valid N-Triples and any reader recovers the same term, which is
why the round trip never saw it. What is lost is the ability to claim canonical form, and with it
the guarantee that two tools serialising one graph produce the same bytes. That guarantee is what
makes a vocabulary diffable in git, which is one of the charter's pillars.

Carriage return and line feed are handled correctly, and no `UCHAR` is emitted for non-ASCII —
accented Latin, CJK, and an emoji are all written raw, as §4 requires. The violation is the tab
alone.

## Consequences

- The claim OpenBiz may make for N-Triples and N-Quads is now **grammar conformance, tested against
  the published EBNF**, plus canonical form for N-Triples *except* the escaped tab. That is a
  stronger and more precise claim than `adr/0010` made, and it is narrower in one place.
- The claim for Turtle, TriG, RDF/XML and JSON-LD is **unchanged and remains round-trip fidelity
  only**. `docs/UNTESTED.md` says so.
- **Oxigraph is not yet load-bearing without qualification.** `CLAUDE.md` §3 requires a spike and an
  ADR recording what was measured before adopting an engine as load-bearing; this is the first
  measurement that counts against it. Finding 1 is not a bug we can fix from outside — it is how
  the engine encodes terms — so the options are upstream work, a term encoding of our own, or
  accepting the loss and disclosing it. That is a decision with a cost, and per §7 the loop does not
  take it alone: it is in `docs/PROPOSED.md` for a human.
- The trait boundary earned its keep. Because `RdfSyntax` is ours and `oxigraph::io` never crosses
  the crate boundary, both findings are statements about *our* published behaviour that a future
  engine swap would have to preserve or explicitly change.

## Alternatives considered

- **Run the W3C rdf-tests suites.** The right long-term answer and still open. Rejected for this
  iteration because it means vendoring a test corpus with its own licence question and a manifest
  format to interpret, which is an item of its own rather than a blind-spot pass. Recorded in
  `docs/PROPOSED.md`.
- **Compare against `rapper` or `riot`.** An independent reader for all six syntaxes at once, and
  much cheaper to write. Rejected because it makes the test suite depend on a tool that is not in
  the repo and not in CI, so it would pass locally and skip silently on the runner — the "green
  because it found nothing to run" failure that `passWithNoTests: false` exists to prevent.
- **Write independent readers for all six.** A Turtle or RDF/XML reader written to check our writer
  would in practice be written to accept whatever our writer emits, which is the tautology this ADR
  exists to escape, at ten times the cost.
