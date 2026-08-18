# ADR 0018 — A candidate has two halves, and a removal is checked against the vocabulary twice

**Status:** accepted (2026-08-18) · **Phase:** 2

## Context

`adr/0017` built the candidate seam as an **additive** shape: a candidate proposes statements to
add, and approval copies them into the target vocabulary. It said so explicitly, and it named the
gap: "proposing removals — which a merge, a deprecation, or a corrective agent all need — is the
next slice of the seam."

That gap is not cosmetic. A merge, a split, a move, and a deprecation are each "these statements
go, those arrive". None of them is expressible against an additive seam, and every bulk operation
in Phase 2's list is one of them. The plan says none of those should be built before removals
exist, for the same reason nothing should have been built before the seam existed.

Iteration 17's loop-log entry also recorded a doubt about this slice, before it was taken:

> a removal has to name statements that *already exist* in the target, and a candidate raised on
> Monday against a vocabulary edited on Tuesday may name statements that are gone by the time
> somebody approves it on Wednesday. Applying it would then silently remove fewer statements than
> the reviewer agreed to, and my apply path has no concept of a precondition.

This ADR records what was decided about both.

## Decision

### Two staging graphs, under one prefix

A candidate's removals are staged in `urn:openbiz:graph:candidate:<id>:removals`, beside the
additions in `urn:openbiz:graph:candidate:<id>`.

Two graphs rather than one graph with a per-statement marker, because a marker would have to be
carried *in* the RDF — which means either reifying every proposed statement or inventing a
container vocabulary. Both make the payload unreadable by the export path that already exists, and
that path is the whole reason `adr/0017` chose a named graph over a blob. With two graphs,
`openbiz candidate 7` prints "would remove … / would add …" as two ordinary Turtle exports, and a
SPARQL query naming `FROM <urn:openbiz:graph:candidate:7:removals>` asks anything else about it.

One prefix rather than two, because `GraphId::classify` decides what every quad of a restore is
allowed to be with a single `starts_with`, and a second prefix would have been a second place for
that rule to be incomplete. A graph named `urn:openbiz:graph:candidate-removals:7` would have
classified as a *vocabulary*, silently, which is exactly the class of mistake the reserved
namespace exists to prevent.

The additions graph keeps the IRI it already had. Rewriting the staging IRIs of proposals a
customer has already reviewed would rewrite their audit trail to make our naming tidier.

### A removal is refused if it names statements that are not there — twice

**At proposal.** `Store::propose_retraction` checks every parsed statement against the target and
refuses the whole file if any is missing, naming the count and showing one. It does not stage the
matching subset: a candidate whose diff is a subset of what the operator asked for, and which does
not say so, is worse than a refusal. The likeliest cause of a mismatch is a producer working from a
stale copy of the vocabulary, and that is worth interrupting.

**At approval.** `Store::decide` checks again, inside the applying transaction, and refuses with
`CandidateStale` if the vocabulary no longer holds every statement. This is the check that answers
iteration 17's doubt, and it is the one that matters — the first only catches a producer that was
already out of date, whereas this catches the vocabulary moving underneath a pending review, which
is the ordinary case in a governance product where review takes days.

The refusal is deliberately not a repair. Three alternatives were considered and rejected:

- **Apply what still matches.** This is the silent-lie option: the reviewer approved twelve
  statements, ten are removed, and the command reports success. There is no artefact anywhere
  afterwards that says the change differed from the one that was reviewed.
- **Re-derive the candidate against the current vocabulary.** That is a *new* proposal wearing an
  old proposal's approval, and the approval is the thing an auditor is relying on.
- **Lock the vocabulary while a candidate is open.** One pending proposal would block all authoring
  on a vocabulary, which in a product whose review cycle is measured in days is not a trade anyone
  would accept.

A stale candidate stays in `proposed`, so it can still be **rejected**. A proposal that can no
longer be applied is exactly the one somebody wants to close.

### Both halves apply in one transaction, removals first

`apply_payload` removes, then adds, inside the transaction that records the decision — so the
pairing `adr/0017` established (statements never reach a vocabulary without a record of who let
them in) holds for statements *leaving* it too, which is where it matters more: an approved removal
is the one change the vocabulary itself no longer evidences.

The order is only observable for a statement staged in *both* halves, which no producer can raise
today. It is fixed now rather than left to whichever half happens to be written first: removing
then adding means such a statement survives, which is what "replace this with itself" has to mean.

### Blank nodes are not renamed on the removing side

An import renames blank node labels so that two files both using `_:b1` do not merge into one node.
A retraction cannot: a renamed label matches nothing, so every removal naming a blank node would
fail the presence check.

That leaves the export-edit-retract workflow resting on whether our serialiser writes labels our
parser reads back as the same node, which no RDF specification promises. It was **measured rather
than assumed**, and it holds: an N-Triples export of a vocabulary retracts from that vocabulary,
blank nodes included. A *hand-written* `_:note` is a different node however it is spelled, and is
refused by the presence check rather than removing something adjacent. Both halves of that are
pinned by a test, because both could change under us with an Oxigraph upgrade.

### Format version 4

The store format goes to 4, with a migration that brings nothing forward.

`adr/0016` set the rule this appears to break — "a version that records no real difference teaches
the next person that versions are decorative" — and the difference here is real and one-directional.
A version-3 build reading a version-4 store does not *fail*: `read_record` looks up the predicates
it knows and ignores the ones it does not, so it would read a candidate that removes twelve
statements as a candidate that removes nothing, show a reviewer a diff missing half its content,
and on approval **apply only the additions while recording that the whole candidate was applied**.
Every step of that succeeds and nothing anywhere says the vocabulary now differs from what was
approved.

That is precisely the failure a version stamp exists to convert into "upgrade". Nothing is written
because nothing needs to be: a version-3 store's candidates are additions-only, and an absent
removal count means zero by construction rather than by a default the migration writes in.

The one-step 3 → 4 migration has an end-to-end test of its own against a hand-written version-3
backup, beside the existing version-1 and version-2 ones. A chain test alone would pass whether the
final step ran or was skipped by an off-by-one, because every earlier step having run is enough to
make the content assertions hold.

## Consequences

**What this makes possible.** Merge, split, move, and deprecate are now expressible against the
seam, and can be built as producers rather than as retrofits. `openbiz retract <graph> <file>` is
the first one, and it is a real workflow on its own: export the vocabulary, cut it down to what
should go, hand it back.

**What is still missing.** No producer raises a candidate carrying *both* halves. The record and
the apply path both support one; nothing makes one. That is recorded in `docs/UNTESTED.md` rather
than claimed as working, and the producers that need it are the bulk operations later in Phase 2.

**Evidence grows.** An approved removal is stored in the removals graph forever, which is now the
*only* copy of statements the vocabulary no longer holds. That makes the missing retention policy
(proposed at iteration 17) more consequential rather than less: a policy that deletes candidate
evidence would, for a removal, destroy the only record of what was taken away.

**The counts are of parsed statements, not distinct ones.** A file naming the same statement twice
reports two. This is pre-existing behaviour on the additions side and is recorded in
`docs/UNTESTED.md`; the staleness check counts against the staged graph rather than against the
record, so the refusal's arithmetic is unaffected.

**No HTTP surface.** `openbiz retract` is a command for the same reason `openbiz import` is: an
unauthenticated "remove these statements from a customer's vocabulary" is a defect, not a partial
feature. Part 3 of the seam lands with authentication.
