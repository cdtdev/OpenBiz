# 0038 — A merge repoints every reference, and is checked against the vocabulary it would leave

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 43
- **Supersedes:** nothing. Extends `adr/0037` (a move is one candidate with two halves).

## Context

`docs/BUILD-PLAN.md` Phase 2, "Bulk operations, part 2 — merge two concepts into one, with every
reference repointed". Two imports of one thesaurus, or two curators working a week apart, produce
two concepts for one thing. Merging them is the operation that undoes it, and it is the operation
an operator is most likely to run **wrong**: it deletes an IRI, and every incumbent in
`docs/COMPETITIVE.md` treats that as a form fill.

`adr/0037` built the producer this needs — one candidate carrying both halves of a change. What a
merge adds is that its promise is about a *whole graph* rather than about two statements.

## Decisions

### 1. A merge reads the raw graph, not the interpreted model

A move touches two statements and both are `skos:broader` or `skos:narrower`, so `CoreModel` holds
everything it needs. "Every reference repointed" cannot be answered from `CoreModel` at all: the
model is an interpretation of a vocabulary, and an enterprise vocabulary is full of statements it
has no reading of. `<X> ex:approvedBy <A>` is a reference. A merge that repointed only what SKOS
recognises would leave the vocabulary pointing at a concept that no longer exists — silently,
because nothing downstream reads `ex:approvedBy` either.

So `MergeScan` streams the raw graph and keeps exactly two things: the statements mentioning the
concept being merged away, and the statements mentioning the one that survives. The first is what
gets rewritten; the second is what tells the rewrite whether the vocabulary already says it. Peak
memory is the degree of two concepts, not the graph.

**Measured, not asserted:** the end-to-end test imports `ex:enclosure schema:housedAs ex:felines`,
approves the merge, reads the vocabulary back off disk with `openbiz backup`, and asserts that no
line mentions the merged IRI and that `housedAs` now points at the survivor.

### 2. A colliding preferred label is demoted, not dropped and not refused

S14 allows one `skos:prefLabel` per language tag. Two concepts being merged almost always both
have one in the same language — that is frequently *why* they are being merged — so repointing
both produces a graph that is not SKOS.

Three options, and the reason for the choice:

- **Refuse and make the operator retract a label first.** Refuses nearly every real merge. The
  operation would be technically correct and practically unusable.
- **Drop the duplicate's label.** Loses the search term that made the duplicate findable in the
  first place, which is the term a user will type again next month.
- **Demote it to `skos:altLabel`.** Nothing is lost, S14 holds, the survivor keeps the name it was
  known by, and the demoted term still answers `openbiz search`.

The third. It is the only place this operation makes a choice the operator did not ask for, so
every demotion is named in the report.

S13 is the constraint that stops the demotion being naive: one literal may not be two kinds of
label on one resource. A label the survivor already carries — under **any** of the three kinds —
is left alone rather than added under another.

### 3. A link between the two concepts goes, rather than becoming a self-link

Absorbing a concept into its own parent is an ordinary merge. `<A> skos:broader <B>` rewrites to
`<B> skos:broader <B>`, which §8.6.7's Example 36 marks *consistent* and which is a concept with no
route to a root. Dropped, and reported: the reviewer should see that the link between them is what
went, because that is the one statement a merge destroys rather than moves.

### 4. A merge that would close a cycle is refused, and the check walks upwards

Identifying two concepts turns every hierarchy path between them of length two or more into a
cycle: with `<A> broader <X> broader <B>`, merging `A` into `B` leaves `B broader X` and
`X broader B`. §8.6.8 calls that consistent, so nothing downstream reports it.

The check asks, of each parent of one concept, whether the *other* concept is above it — an
**upward** walk. The equivalent downward formulation ("is this parent below the survivor?") is the
one `adr/0037` used for a move, and iteration 42's own closing doubt was that `WalkBound::DEFAULT`
going down is a ceiling an ordinary large vocabulary reaches. Upward it is not: ISO 25964 thesauri
are conventionally a handful of levels deep. Same question, cheaper direction, and it is a direct
answer to that doubt rather than a repetition of it.

A path of length **one** is not a cycle — it becomes a self-link, which decision 3 drops — and
refusing it would refuse the commonest merge there is. That boundary has its own test.

### 5. The change is checked against the vocabulary it would leave — the whole condition set

**This is the decision this ADR exists for, and it was not in the plan when the item started.**

The first working version of this command produced, from perfectly ordinary input, a vocabulary
violating **two** of the SKOS Reference's own integrity conditions. Verified by hand against a
real store before it was written down:

- **S14**, through SKOS-XL. The label reconciliation in decision 2 works on plain `skos:prefLabel`
  statements. A `skosxl:prefLabel` points at a label **resource**, so repointing it is not a label
  decision and the reconciliation never sees it — and then S55 dumbs both resources down to plain
  preferred labels in one language.
- **S27**, through `skos:related`. Broken whenever the survivor is associatively linked to
  something the duplicate was below. Nothing in a merge's obvious risk surface predicts this.

A hand-written check for the conditions a merge is *expected* to break would have caught S14 and
missed S27 entirely. So the check is not hand-written: `newly_violated(before, after)` builds the
model of the vocabulary as it would be — the graph without the removals, with the additions — and
runs **every** integrity condition, using the code already tested against the specification's own
examples. Any condition violated afterwards that was not violated before refuses the merge, naming
the condition and its counter-examples.

**Only newly broken conditions count.** A vocabulary that already violates a condition must stay
editable, or the tool cannot be used to fix it. That has its own test.

**The cost is stated rather than hidden:** this reads the vocabulary a second time and builds a
second model, so a merge is four passes over the graph rather than two. That is the price of
checking a proposal against the whole specification instead of against an author's expectations,
paid by a bulk operation nobody runs in a loop. It is unmeasured at scale and recorded in
`UNTESTED.md`.

### 6. A reference from another vocabulary is counted and named, never rewritten

A statement in another named graph is a change to that vocabulary, reviewed by whoever owns it —
a different candidate. The command counts what it found and names where, including changes staged
against this vocabulary and still waiting, because approving one after this merge would put the
reference back. Silently leaving them would make "every reference repointed" a claim the report
does not support; silently rewriting them would reach across a governance boundary.

### 7. No tombstone

The merged IRI simply stops existing in the vocabulary, and the candidate record is what says it
existed. `dcterms:isReplacedBy`, `owl:deprecated`, and a redirect are the deprecation lifecycle,
which is its own plan item; inventing half of it here would produce a second, incompatible answer
to a question that item has to settle. The report says so in a sentence rather than leaving the
operator to discover it.

## Consequences

- `openbiz merge <graph> <duplicate> <survivor>` is the production caller. There is no endpoint,
  for the authentication reason every writing path in this build records (`BLOCKED.md`).
- `newly_violated` is a general mechanism and `openbiz move` does **not** use it. A move can leave
  an S27 violation, verified by hand at iteration 43 and recorded in `UNTESTED.md` with the
  reproduction. Fixing it is one call and a test, and it is in `PROPOSED.md` rather than
  self-authorised.
- 30 new tests, 899 in total. `cargo fmt --check`, `clippy --workspace --all-targets -D warnings`,
  `cargo test --workspace`, `cargo deny check licenses` all `rc=0`, read from exit status.
- No new dependency.
