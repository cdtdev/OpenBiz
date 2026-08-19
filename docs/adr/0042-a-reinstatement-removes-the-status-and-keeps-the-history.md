# 0042 — A reinstatement removes the status and keeps the history

- **Status:** accepted
- **Date:** 2026-08-19
- **Phase:** 2 — SKOS authoring model
- **Item:** Deprecation lifecycle, un-retiring — a concept retired in error, put back

## Context

`adr/0040` writes a retirement and removes nothing; `adr/0041` reads it back so that every browse
and search path marks it. Both deferred the third part, and both named it: a concept can be retired
in error, or retired and then found still to be in use, and until now the only way back was to
hand-write the statements into a file and retract them with `openbiz retract`. That is a real
manual path and it is why this was not urgent — but it requires an operator to know the exact
lexical form of the marker this build wrote, including the datatype, and to get it byte-for-byte
right or the retraction silently matches nothing.

This is also the **first operation in this build whose whole purpose is to remove statements**.
Every write before it either added only (`import`, `split`, `deprecate`) or removed as a side
effect of repointing something (`merge`, `move`). That makes the removal seam — the half of the
candidate model `adr/0004` added and which `openbiz retract` has been the only ordinary user of —
load-bearing for a computed operation for the first time.

## Decision

`openbiz reinstate <graph> <resource> [--note <text>] [--language <tag>]`.

### 1. It removes the marker **and** the recorded successor, always, and offers no way to keep one

Both halves of what `openbiz deprecate` writes come out together.

Removing only `owl:deprecated` would leave a resource that is current and records a successor. That
state has a name in this build already: `Retirement::is_unmarked`, the half-retirement `adr/0041`
added because it is *the most likely way a retirement goes wrong* and `openbiz inspect` reports it
as such. A command that produced one deliberately, as its normal outcome, would be manufacturing
the defect the previous iteration built a report for.

The standard says the same thing. DCMI defines `dcterms:isReplacedBy` as "a related resource that
supplants, displaces, or supersedes the described resource". A current concept that is superseded
is a contradiction rather than a nuance. If the two concepts are still related after the
reinstatement, SKOS has `skos:related` and `skos:closeMatch` for saying so, and the operator can
say it with `openbiz import` — this does not decide it for them.

So there is no `--keep-replacement` and no `--marker-only`. The refusal in the argument parser is
deliberate rather than an omission.

### 2. Every marker comes out, not the first one found

`says_true` is lenient on read: `"true"^^xsd:boolean` and a plain `"true"` are both read as a
retirement, because a vocabulary that arrived from another tool may carry either. A resource that
has been through two tools can carry **both**. Leaving one behind leaves the concept retired
everywhere while this command reports that it is not — a false green in the product itself — so the
scan holds every status statement about the resource and the reinstatement removes all of them.

This is why the scan holds **statements** where `DeprecationScan` holds counts and a set: a removal
has to match what is in the store exactly, and `Store::decide` refuses to apply a candidate whose
removals are no longer all present.

### 3. The `skos:changeNote` explaining the retirement stays. Every note stays.

This is the decision the item turns on, and it was posed on the plan item as an open question.

The sufficient reason is mechanical: **nothing links a change note to the marker.**
`CoreModel::deprecate` writes an ordinary `skos:changeNote` with no statement joining it to the
`owl:deprecated` it was written beside. Identifying "the note that explained the retirement" would
mean matching on its text or on its position in a statement stream, which is a guess, and a wrong
guess deletes a curator's prose.

The better reason is that even a note this *could* identify should stay. SKOS §7 defines
`skos:changeNote` as documenting a **modification**, and the modification happened. A vocabulary
whose history reads "retired 2026-03; reinstated 2026-08, retired in error" is telling the truth. A
vocabulary tidied until the retirement never appears is the "proprietary, opaque change history"
`CLAUDE.md` §1 names as a reason this product exists — and it is worse here than in a proprietary
tool, because the tidying would have been done automatically by a command the operator ran for a
different purpose.

So: the notes stay, `--note` adds the sentence explaining why the retirement was taken back, and
`Reinstatement::kept_notes` names the ones left in place so the **report shows the operator the
history they now have** rather than leaving them to find it in an export.

### 4. It is defined by the statements, not by the model

Every other operation here starts by asking `CoreModel` for the resource and refuses if it is not a
`skos:Concept`. This one does not. It removes statements that exist, and whether they can be
removed does not depend on what else the graph says about their subject. The case that decides it:
a stray `owl:deprecated` imported about an IRI this vocabulary types as nothing at all is exactly
where a person needs the marker gone and exactly where the model has never heard of the subject.

The model is still read — for the labels the report prints, for the notes that stay, and for the
integrity check the caller runs — but it is not the gate. The refusal is *the vocabulary says
nothing about this resource's status*, and it says something different when the vocabulary has
never heard of the IRI at all, because a retirement is per-vocabulary and the likeliest mistake is
naming the wrong graph.

### 5. An `owl:deprecated` it cannot read is left alone and named

`owl:deprecated "false"`, an IRI object, a language-tagged literal: none of these is read by this
build as a retirement. Removing them would be acting on a meaning nobody here has established, and
silently leaving them would hide a status statement from the one command whose subject is status.
So they stay, and `Reinstatement::unread` reports them with the reason. This is the explainability
commitment (`CLAUDE.md` §3) applied to an absence: the command says what it did **not** do and why.

### 6. It reports what is still retired around it, and puts none of it right

The mirror of `adr/0040`'s stranding report, from the other side. A reinstated concept whose broader
concept is still retired is a current concept under one nobody should use; its children were retired
by their own decisions and stay retired. Each is a separate decision and none is inferable from the
graph, so the report counts and names them.

One thing does get better on its own and is reported as such: a retired concept that named **this**
one as its replacement was a trail leading to another retired concept — the defect `adr/0041` added
a report for — and after the reinstatement it leads somewhere current.

### 7. The whole integrity condition set runs, as on every other computed write

Removing two non-SKOS statements cannot break a SKOS condition. That is precisely the reasoning
iteration 43 found to be wrong about a merge and iteration 45 declined to repeat about a
deprecation, so it is not repeated here either. `newly_broken` runs the whole set against the
vocabulary the change would leave.

## Consequences

- The deprecation lifecycle is complete in the model and on the command line: retire, read, take
  back. Nothing in it is reachable from the HTTP API or the UI, which is the candidate seam's own
  blocked item and not this one.
- A reinstatement is **not** a return to the graph's earlier state, and the report says so. It is
  the earlier state plus the change notes. An operator diffing two exports will see one statement
  they did not expect, and the report tells them in advance which one and why.
- The removal half of the candidate model now carries a computed operation. `openbiz retract`'s
  file-driven path is no longer its only ordinary user.
- `ReinstatementScan` reuses `StatusBound::max_replacements` as a cap on **every** status statement
  it holds about one resource, not only the replacements the field is named for. That constant was
  already recorded in `docs/UNTESTED.md` as measured against nothing; it now governs a second thing
  and the entry is updated rather than duplicated.
- Nothing measures this on a large vocabulary. It is two passes over the graph plus the two
  `newly_broken` adds, exactly like a deprecation, and exactly as unmeasured. Recorded.

## Alternatives rejected

- **A `--reinstate` flag on `openbiz deprecate`.** The two operations share a subject and nothing
  else: one adds and one removes, one has a replacement and a note to reason about and the other
  refuses to take a replacement at all. Folding them would give one command two argument sets and
  two refusal sets that never overlap.
- **Removing the change note that explained the retirement.** §3. The mechanical objection alone
  settles it, and the honesty objection would settle it even if the mechanics were easy.
- **Refusing a resource that is not a `skos:Concept`.** §4. Symmetry with `openbiz deprecate` is not
  worth a command that cannot remove a marker somebody's import put on a collection.
- **Removing an unreadable `owl:deprecated` along with the readable ones.** §5. It would be this
  build inventing a reading of a statement it elsewhere declines to read.
