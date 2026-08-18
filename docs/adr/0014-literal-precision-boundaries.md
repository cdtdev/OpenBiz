# 0014 — Where a typed literal stops being a value, and what we say about it

- **Status:** accepted
- **Date:** 2026-08-18
- **Phase:** 1 (RDF core & store)
- **Supersedes / amends:** nothing. Extends `adr/0012`, which found the lexical rewriting this
  measures the edge of.

## Context

`docs/BUILD-PLAN.md` Phase 1 asks for a spike: *"characterise Oxigraph's numeric/calendar/duration
literal precision limits and decide our documented behaviour at the boundary"*.

The question was not idle. `adr/0012` had already established that the store rewrites the lexical
form of any literal it can interpret — `"007"^^xsd:integer` comes back `"7"` — and that the rewrite
is the *store's*, not the export's, so every reader inherits it. That finding leaves an obvious
follow-up unanswered: **what happens to a literal it cannot interpret?** A store that normalises
numbers is a nuisance. A store whose behaviour changes character at an undocumented threshold is a
correctness problem, and which one we have is exactly what a governance buyer is entitled to know
before they put a controlled vocabulary in it.

The measurements are in `crates/openbiz-store/src/literal_precision.rs`, which is the harness and
the record together, in the same style as `adr/0013`'s benchmark. Every claim below is an assertion
in that module, so it fails if the behaviour changes in either direction.

## What was measured

A battery of typed literals written through `Store::transaction` — the real write path — and read
back two ways: serialised through `Store::export_graph`, and interrogated through `Store::query`,
the entry point `/api/sparql` calls. Reading it both ways is what separates a serialisation
artefact from a stored-term one.

### The rule

The backend's term encoding is a **value-space** encoding for the datatypes it models natively and
a **lexical-space** encoding for everything else.

- Where it **interprets** a literal, it stores the value and renders a canonical lexical form on the
  way out. The term that comes back is not the term that went in.
- Where it **fails to interpret** one — the value is outside the range of the Rust type behind the
  datatype, or the literal is ill-typed — it keeps the bytes exactly and stores no value. The
  literal round-trips perfectly and is no longer a number, a date, or a duration to the query
  engine.

### The boundaries

| family | interpreted while | at the edge |
|---|---|---|
| `xsd:integer` and derived types | the value fits `i64` | ±2^63; beyond it the bytes are kept and the value is gone |
| `xsd:decimal` | 128-bit fixed point, 18 fraction digits | `170141183460469231731.687303715884105727` is a number; `…728` is not. `0.000000000000000001` is a number; a 19th fraction digit is not |
| `xsd:float` / `xsd:double` | always | saturates to `INF` / `0` per IEEE 754 — **this is what XSD 1.1 specifies**, not a limitation |
| `xsd:dateTime`, `xsd:date`, `xsd:time` | the lexical form is XSD-valid | a leap second (`23:59:60`) or a timezone past ±14:00 is kept verbatim and is not a date |
| `xsd:duration` and its two subtypes | the lexical form is XSD-valid | normalises to the XSD canonical form: `PT25H` → `P1DT1H`, `P13M` → `P1Y1M` |

### Why this is the finding, rather than "large values lose precision"

They do not lose precision. `"123456789012345678901234567890"^^xsd:integer` round-trips
byte-for-byte. It loses its **value**: `isNumeric` is false, `FILTER(?o > 1)` does not match it,
and `?o - 1` is unbound. An export, a diff, and a git history all show it as a well-formed
`xsd:integer` indistinguishable from one the engine can add to.

So the practical harm is not a wrong number. It is a **filter that silently omits rows**. A
governance team asking `FILTER(?value > 1000)` over a column that crosses the boundary gets an
answer missing exactly the rows that crossed it, and a short answer reads precisely like "there
were no such rows". That is the failure mode `adr/0011` designed the query limits to refuse rather
than commit, arriving here through a different door.

### The defect found on the way, which is worse than the boundary

**The datatype IRI of a derived integer type is not preserved.** `xsd:int`, `xsd:short`,
`xsd:byte`, `xsd:long`, `xsd:unsignedLong`, `xsd:nonNegativeInteger`, and `xsd:positiveInteger`
all come back as `xsd:integer` when the value fits `i64`. The lexical form survives; the datatype
does not.

The same asymmetry `adr/0012` found holds here and is still backwards: `"9223372036854775808"^^xsd:long`
is *out of range for `long`*, is therefore not interpreted, and therefore **keeps** its datatype.
The well-typed value loses its type; the ill-typed value keeps it.

Two consequences, both landing outside Phase 1:

1. **Statements are silently lost.** Under [RDF 1.1] a literal is the pair (lexical form, datatype
   IRI), so `"5"^^xsd:int` and `"5"^^xsd:integer` are distinct terms. Written against one subject
   and one predicate, four distinct terms — `xsd:int`, `xsd:integer`, `xsd:byte`, `xsd:string` —
   come back as **two statements**. Nothing in the API, the export, or the transaction's result
   says three became one. A vocabulary that went in with four assertions comes out with two, and
   the diff a reviewer approves is against the two.
2. **It breaks two later phases at the root.** A SHACL `sh:datatype xsd:int` constraint (Phase 4)
   can never be satisfied by data in this store, because the datatype the shape names is not the
   datatype the store returns. An OWL 2 datatype range over a derived type (Phase 5) is untestable
   for the same reason. Both are recorded in `docs/UNTESTED.md` against those phases so they are met
   as a known constraint rather than as a mystery.

### Two smaller things, recorded because they will otherwise be rediscovered

- **A large double is rendered in positional notation.** `"1.0E308"^^xsd:double` exports as 309
  characters of digits. That is a valid `xsd:double` lexical form and it is not the canonical one
  XSD's canonical mapping specifies. Harmless to a parser, ugly in an export, and surprising in a
  diff.
- **Arithmetic overflow and an uninterpreted operand are indistinguishable in an answer.**
  `int_max + 1` is unbound because the sum has nowhere to go; `int_over - 1` is unbound because the
  operand was never a number. Both appear as the same missing cell.

## Decision

**We state the boundary rather than move it.**

Moving it means replacing the term encoding of the store the entire product rests on — arbitrary
precision integers and decimals are not a configuration flag in Oxigraph, they are a different
value representation throughout the encoder, the comparator, and the index. `CLAUDE.md` §1
commits us to one binary with an embedded store; trading that for `xsd:int` fidelity is not a
trade this spike is authorised to make, and it is not obviously the right one at any price.

So, concretely, for this build:

1. **The boundaries above are the documented behaviour**, pinned by
   `crates/openbiz-store/src/literal_precision.rs`. They are not "current behaviour we happen to
   have"; they are what we claim, and the tests fail if either side of the claim moves.
2. **We do not silently widen the numeric range** by pre-parsing literals into a bignum of our own.
   That would make the store's answers and our own disagree, which is worse than one honest limit.
3. **We claim no more than we do.** Nothing in the docs, the API, or the UI may describe the store
   as preserving typed literals. It preserves the ones it cannot interpret and canonicalises the
   ones it can, and the datatype substitution above means it does not even preserve those faithfully.
4. **Disclosure is the fix, and it is a human's to authorise.** The obvious remedy — the store
   telling a caller "this literal was rewritten" or "this datatype was substituted" at the moment
   it happens, so an import can surface it and a reviewer can see it — is a user-facing capability
   with a real design cost. It belongs to the candidate seam (Phase 2), where a proposed change
   already carries provenance and a human already reviews it. It is written up in
   `docs/PROPOSED.md` and is **not** built here, per `CLAUDE.md` §7.

## What this does not settle

- **Whether the collapse is acceptable at all.** A store that turns four statements into two is
  arguably not a conformant RDF store, and "we documented it" may not be an adequate answer for a
  regulated buyer. That is a commercial judgement, and it is the first of the proposals.
- **Whether the derived-datatype substitution is fixable cheaply.** It may be an Oxigraph
  configuration or a `Literal` construction detail rather than a property of the encoding; nobody
  has looked upstream. Recorded in `docs/UNTESTED.md` as unexamined rather than assumed hard.
- **What an import should do** when it meets a literal that will not survive. Refuse it, accept it
  with a recorded warning, or accept it silently — a decision that belongs with the parser, which
  is deliberately deferred behind the candidate seam.

[RDF 1.1]: https://www.w3.org/TR/rdf11-concepts/#section-Graph-Literal
