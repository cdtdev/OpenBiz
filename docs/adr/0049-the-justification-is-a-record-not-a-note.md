# 0049 — The justification for creating is a record, not a note

**Date:** 2026-08-20 (NZST, UTC+12)
**Status:** accepted
**Supersedes:** nothing. **Implements:** `adr/0003` §3, `CLAUDE.md` §1.7.

## Context

`adr/0003` §3 puts "create new" at the bottom of the reuse ladder and requires a recorded reason
naming what was found and why nothing fitted. It is explicit about what that record is *not*:

> The justification is the mechanism. Not a warning dialog — those get clicked through — but an
> auditable record that makes proliferation visible to the people accountable for it.

Iterations 55 and 56 put discovery on both creation paths — `openbiz mint` and `openbiz split` —
and both print the ladder. Neither recorded anything, and both said so. Two iterations running, the
loop's own "still uncertain" line asked whether printing a ladder with no record behind it had
built precisely the clickable warning dialog §3 rules out.

"Visible to the people accountable" is the whole specification, and it is a *query*: which concepts
were created despite something already existing under that name? That is the question this ADR is
answering, and every option below is judged against whether it can be asked.

## Decision

**A justification is a first-class record in the system graph, keyed to the IRI that was created.**

It carries: the created IRI; the vocabulary; the label it was created under; each existing resource
that was found and passed over, **as an IRI in the object position**; the reason; whether the search
behind it finished; and who recorded it, when, on the UTC clock (`adr/0047`).

It is written by `openbiz mint --because "…"` and read by `openbiz justifications [<graph>]`.

### Why not a field on the candidate's provenance

The build plan named this as the alternative, and it loses on two counts.

A candidate can create several concepts at once — `openbiz split` divides one concept into three —
and a single field could not say which of the three had a match and which did not. The unit being
justified is a *creation*, not a proposal.

More decisively: `openbiz mint` has no candidate at all. It computes an IRI and stages nothing, by
design. A field on the candidate would cover the half of the creation surface that already has a
reviewer looking at it and miss the half that has nobody.

### Why not prose in the note

Because prose cannot be asked the question. This is the entire distinction §3 draws, and it is why
the resources passed over are named nodes rather than text: a query joining a justification to the
concept it passed over is only possible with an IRI in the object position. Written as text the
record would read the same to a human and answer nothing to an auditor. There is a test that joins
`?passed` to the vocabulary's own `skos:prefLabel` for exactly that reason — it fails the moment the
representation weakens, which was checked by weakening it.

### Why it is captured on `mint` rather than in a command of its own

`adr/0003` §4 is a usability requirement with teeth: reuse must be less work than recreating. A
justification that costs a second command is one that gets skipped, and §3 is clear that a mechanism
people route around has failed rather than been ignored. `mint` is where the ladder is printed, so
it is where the reason is cheapest to capture.

This changes `mint` from a command that writes nothing to one that writes when asked. The three
properties that mattered are unchanged and are still asserted: it stages nothing, it reserves
nothing, and run twice it answers the same IRI. The record goes to OpenBiz's own system graph, so —
like the IRI-minting policy (`adr/0036`) — it is a fact *about* a vocabulary rather than one *in*
it, and it does not travel to another tool as a statement no standard defines. It is therefore not
a change to a vocabulary and does not go through the candidate seam (`CLAUDE.md` §3).

### Why `--because` with no label is refused

§3 asks for a reason *naming what was found*. With no label there was no discovery pass, so there is
nothing found to name, and a record written anyway would file the appearance of diligence as
evidence of it.

### Why the record says whether the search finished

A justification is evidence that somebody looked. Evidence from a search that could not reach a
source, or that stopped at its bound, is weaker — and a record that does not say so invites a
reader to take a truncated search for a finished one. So `searchWasComplete` is required rather than
optional, and both reports mark and count the records that rest on an unfinished search.

### No store format bump

The record is a new subject type in the system graph under predicates no earlier build reads. Unlike
format versions 3 and 4 — which were additive but changed how an *existing* structure had to be
read, and so existed to make an older build refuse rather than misread — nothing here changes the
meaning of anything already on disk. An older build opening a store with justifications ignores
them, which is the correct behaviour: it cannot report a record it does not know about, and it will
not misreport one. The IRI-minting policy was added on the same reasoning.

## Consequences

- The auditor's question is answerable, in SPARQL over the system graph and through
  `openbiz justifications`, across every vocabulary at once because proliferation happens *between*
  vocabularies.
- **Nothing is refused.** There is no single-step create in this build to attach a refusal to, so
  these records are what people chose to write down rather than everything that happened. Both
  reports say so, in the full and the empty case, because an empty governance report reads as a
  clean bill of health and this one is not one.
- `openbiz split`, the other creation path, does not yet record one. It is the next item.
- A match that is a blank node cannot be named in the record. It is counted in the report rather
  than dropped silently. In `docs/UNTESTED.md`.
- Recording twice appends rather than replacing. A justification is a statement made at a time, and
  an audit trail that overwrites its own entries is not one.
