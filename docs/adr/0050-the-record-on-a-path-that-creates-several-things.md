# 0050 — One justification per part, and a rejected creation says so

**Date:** 2026-08-20 (NZST, UTC+12)
**Status:** accepted
**Extends:** `adr/0049`. **Implements:** `adr/0003` §3, `CLAUDE.md` §1.7. **Carries through:** `adr/0048`.

## Context

`adr/0049` made the justification a first-class record and wired it to `openbiz mint`. It closed
with the sentence this ADR exists to answer: *"`openbiz split`, the other creation path, does not
yet record one. It is the next item."*

A split is not a second mint. It is **N creations from one command**, and it stages a candidate —
so it raises two questions the mint path never had to answer, and both are governance decisions
rather than coding ones.

## Decision 1 — one `--because` covers every part; the *findings* are per part

`openbiz split … --because "…"` files **one justification per part**. Each names the part's own IRI,
its own label, and what discovery found under **that name** and passed over. The reason is one
sentence, given once, and recorded against all of them.

**Why not a reason per part.** It would have to be aligned to the `--into` labels positionally, and
a list that drifted by one would file every reason against the wrong part with nothing in the
output to signal it — a governance record that is confidently wrong is worse than one that is
coarse. And `adr/0003` §4 binds here as it did at `adr/0049`: reuse must be *less* work than
recreating, so a three-part split that demands three separate reasons is a mechanism people route
around, which §3 counts as failure rather than as the user's fault.

What genuinely differs per part is what already existed under that part's name, and that is the
thing an auditor queries. It is per part in the record, which is exactly why `adr/0049` refused to
put the justification on the candidate: one field on a candidate could never have said which of
three parts had a match.

The cost is real and recorded: a curator whose reasons for the three parts genuinely differ has one
sentence to say all three things in. `docs/UNTESTED.md`.

## Decision 2 — a rejected candidate keeps its justifications, and the report says nothing was created

The records are written when the split is **proposed**, and they stand whatever the reviewer
decides. Deleting them on rejection was rejected outright: a justification is a statement somebody
made at a time, and an audit trail that erases entries when a change is refused is not one — the
interesting governance question is often precisely *what did we propose and then not do*.

But standing unqualified would be dishonest in the other direction. A report listing a rejected
split's parts as concepts created despite existing matches would count proliferation that never
happened, and `docs/UNTESTED.md` has carried exactly that gap since `adr/0049`: *"a justification
survives its creation being abandoned, and nothing notices"*.

So each record **names the candidate it arose from**, as an IRI in the object position — the same
representational decision as the considered resources, and for the same reason. `?j
<justificationCandidate> ?c . ?c <candidateState> "rejected"` is a query. `openbiz justifications`
reads each record's candidate back and reports one of four fates:

- **nothing was proposed** — the mint path, which stages nothing at all, so the record says somebody
  looked and *not* that the concept exists;
- proposed, **undecided**;
- proposed and **approved**;
- proposed and **refused** — "the concept was never created", counted in a summary line as well as
  marked per record.

The unknown-state branch is named rather than folded into one of the others, because `CandidateState`
is `#[non_exhaustive]` and a fate reported wrongly is worse than one reported as unknown.

The report's headline also changed wording: "of which N **passed over something that already
existed**", not "created something". With the fates under it, the old wording contradicted its own
entries.

## Decision 3 — the concept being divided is never "passed over"

`adr/0048` established that a part taking a label the original already carries is a *label to
apportion*, not a duplicate to avoid; the match is shown, annotated, and kept out of the reuse
ladder. That rule carries into the record: the concept being split never appears in any part's
considered list. Recording it would file the original as a duplicate of its own parts, which is the
opposite of what the operation means. The split report says so — but only where a part actually did
match the original, because a sentence printed under every split is one readers learn to skip.

## Decision 4 — a split's records are written in one transaction

`Store::record_justifications` takes a batch and refuses all of it if any one request is refusable.
A split that recorded two parts out of three would read as though the third had been reused, and
nothing would distinguish that from a record lost halfway through. The identifiers of one split are
consecutive.

The singular `record_justification` is now a wrapper over the batch, and both take a
`NewJustification` struct rather than eight positional arguments — several of them strings, where
two could be swapped without the compiler noticing and the record would name the wrong concept.

## Decision 5 — still no store format bump

Same reasoning as `adr/0049`. `justificationCandidate` is a new, optional predicate on a subject type
no earlier build reads. Nothing already on disk changes meaning, and an older build ignores what it
cannot see rather than misreading it. Absent rather than written as "none", so "no candidate" is
`FILTER NOT EXISTS` and a build that never writes one asserts nothing about a candidate it does not
have.

A record naming a candidate the store cannot look up is **corrupt**, not a record reported without
one: "nothing was proposed" is itself a claim, and making it falsely is how a report says a creation
never happened when it did.

## Consequences

- Both creation paths in this build now file the record `adr/0003` §3 requires, and the reading side
  can tell a creation that happened from one that was refused.
- **Nothing is still refused.** No path in this build makes anybody file a justification; these are
  what people chose to write down. Both reports keep saying so.
- A rejected split's records are correctly excluded from the proliferation reading, and an *approved*
  candidate is reported as approved — not as "the concept exists", because a later change this
  record knows nothing about may have removed it.
- Reading the report now costs one candidate lookup per record on top of reading every record.
  Unmeasured, in `docs/UNTESTED.md` with the rest of that family.
