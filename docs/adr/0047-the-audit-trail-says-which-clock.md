# ADR 0047 — The audit trail says which clock it is on, and its stamps are values

**Status:** accepted (2026-08-20 NZST, UTC+12) · **Phase:** 2 · **Supersedes nothing** ·
Raised by product-owner feedback (`FEEDBACK-LOG.md`, 2026-08-20).

## Context

The product owner resolved a discrepancy three iterations had escalated: the harness reports local
time, `date -u` reports UTC, this host is `Pacific/Auckland` at UTC+12, and for half of every day
the two disagree by a calendar day. Nothing was broken. The instruction that came with the
resolution was the substantive part:

> an offset is not a discrepancy until you have compared like with like. Anywhere the build reasons
> about time, be explicit about which clock it means. […] For anything a reader must order — a
> ledger entry, a provenance timestamp, a `dcterms:created` on a candidate — use an explicit offset
> or UTC, never a bare date. That one is worth a proposal if the audit trail is affected.

So the first question was whether it *is* affected. Four things in this build write a wall-clock
time, all in `openbiz-store`: a candidate raised, a candidate decided, an IRI policy recorded, a
migration applied. Every one used `oxsdatatypes::DateTime::now()`, which is built on the Unix epoch
with `TimezoneOffset::UTC` and therefore always emits `…Z`.

**The instants were right. Two things around them were not.**

**A recorded IRI policy's timestamp was a plain literal.** A candidate's `proposed_at` and a
migration's `migrationAt` were written as typed `xsd:dateTime` from the day each shipped;
`urn:openbiz:iriPatternRecordedAt` was written with `Literal::new_simple_literal`, holding the
identical lexical form as an `xsd:string`. It read correctly everywhere a person printed it and was
invisible to everywhere a machine compared it. `SELECT ?at … FILTER (?at > "…"^^xsd:dateTime)` over
the system graph returned **zero** rows for a policy whose stamp was plainly in range — measured,
not assumed, and the same query now returns one. The question the field exists to answer — *which
minting convention was in force when that concept was created* — is a comparison between a policy's
stamp and a candidate's, and it silently could not be asked. `CLAUDE.md` §3 makes the trail ordinary
RDF answerable by SPARQL precisely so an auditor need not take our word for it; a field that is only
correct when a human reads it is not that.

**The stamps were the one field of a record the store did not re-validate on the way back in.**
`candidate::read_record` opens with "every field is re-validated rather than trusted", and it means
it — the target IRI, both payload graphs, the source token, the state, the counts, the
addition/removal invariant, the decided-by pairing. `proposed_at` and `decided_at` came back through
the same reader that returns an agent's name. A doctored store saying a candidate was raised at
`"last Tuesday"`, or at `2026-08-19T14:17:03` with no timezone, was read, kept, and printed to a
reviewer as evidence.

## Decision

**1. One seam, `openbiz_store::RecordedAt`, and every wall-clock stamp goes through it.** Four call
sites, no others; the crate has no other time source and no `chrono` or `time` dependency.

**2. What we write is UTC. What we read must carry an explicit offset — any offset.** These are
deliberately different rules. `RecordedAt::now()` stamps `…Z` and nothing else. `RecordedAt::parse`
refuses a lexical form that is not an `xsd:dateTime` and one that names no timezone, but accepts
`+12:00` — because a store may hold records this build did not write, and an offset we would not
have chosen is still perfectly orderable. What is refused is the *absence* of an answer. A bare
`2026-08-19T14:17:03` is valid XSD and an unusable audit record: XSD leaves it to the reader's
implicit timezone, so two such stamps from servers in different zones cannot be ordered against each
other at all.

**3. The seam is tighter than the engine, and that is checked rather than assumed.** `adr/0014`
recorded two lexical forms Oxigraph keeps verbatim and will not compare — a leap second, and a
timezone past ±14:00. Both are outside what XSD admits, and `RecordedAt::parse` refuses both, so
**everything this build accepts is something the engine can order**. `24:00:00` goes the other way:
XSD admits it as the next day's midnight, we accept it, and the engine normalises and compares it.
Tests pin both directions. Refusing a form the specification allows, to suit one engine's quirk,
would be `CLAUDE.md` §2's standards-first commitment inverted.

**4. `iriPatternRecordedAt` is a typed `xsd:dateTime`, and format version 5 brings existing stores
forward.** `RetypeIriPolicyStamps` is the first migration in the chain that rewrites data.

**5. A value the migration cannot read is left exactly as found.** Not retyped — labelling prose as
a date and time asserts something the record does not say. Not fatal — refusing at open turns one
unreadable field in one vocabulary into a store that will not start, and sends an operator to
disaster recovery for a record whose pattern they could have read by hand. `policy::read_policy`
already refuses it at the read, naming the vocabulary, which is where a per-vocabulary problem
belongs.

**6. No comparison is implemented in Rust.** `RecordedAt` exposes `now`, `parse`, and the lexical
form. Ordering the trail is a SPARQL question over `xsd:dateTime`, which is the whole reason for
decision 4; a second implementation in Rust would be a second answer to a question the datatype's
own semantics already answer correctly.

## What was measured

- The untyped stamp: `FILTER (?at > "2000-01-01T00:00:00Z"^^xsd:dateTime)` over a recorded policy
  returned 0 rows before and 1 after. Reverting `quads_of` to `new_simple_literal` fails that test
  and the datatype assertion beside it.
- Removing the timezone requirement from `RecordedAt::parse` fails four tests across three modules.
- A real version-4 backup — the stamp rolled back and the literal untyped — restored through the
  built binary: migrated to 5, retyped, pattern and attribution and instant intact, migration record
  written, and `openbiz policy` still reads it.
- `2026-08-19T00:00:00+15:00` and `2016-12-31T23:59:60Z` are refused by the seam; `24:00:00Z` is
  accepted and compares.

## Consequences

Store format version **5**. An operator upgrading gets a one-step migration that reports what it
did and leaves a record of itself. A build at version 4 refuses a version-5 store as too new, which
is the correct refusal.

Nothing yet *orders* the trail in the product — no `openbiz history`, no sorted candidate list. This
ADR makes the trail orderable and pins that it is; using it is later work, and it is proposed rather
than self-authorised.

The seam is not enforced. A future call to `DateTime::now().to_string()` in the store would compile
and would write a stamp nothing had validated. Recorded in `UNTESTED.md`.
