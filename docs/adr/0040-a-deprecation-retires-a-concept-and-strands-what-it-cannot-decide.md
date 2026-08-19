# 0040 — A deprecation retires a concept and strands what it cannot decide

- **Status:** accepted
- **Date:** 2026-08-19
- **Phase:** 2 — SKOS authoring model
- **Item:** Bulk operations, part 4 — deprecate with replacement

## Context

The other three bulk operations kept pointing at a change none of them could make. `openbiz merge`
ends its report with "a merge does not leave a tombstone behind in the vocabulary; deprecating a
concept in place is a different change". `openbiz split` ends its with "retiring it is a
deprecation, which keeps the trail an auditor needs". Both are true, and until now neither was
possible.

The change they were pointing at is the one a governance team actually needs. A term goes out of
use. Its IRI has been published; other systems have stored it; a dataset catalogued under it three
years ago still says so. Deleting it — which is what a merge amounts to for the concept that does
not survive — breaks every one of those. What is wanted is the concept still being there, still
meaning what it meant, and saying plainly that it is no longer current and what to use instead.

The two plan lines "Bulk operations, part 4 — deprecate with replacement" and "Deprecation
lifecycle preserving history rather than deleting" describe the same operation from two sides. This
ADR records the first: the operation and what it writes. The lifecycle around it — a retired
concept's treatment in `openbiz tree`, `openbiz search` and `openbiz ancestors`, and an
un-deprecation — stays as the separate plan item it already is.

## Decision

`openbiz deprecate <graph> <concept> [--replaced-by <iri>] [--note <text>] [--language <tag>]`
computes the change, stages it as **one additions-only candidate**, and prints it. `openbiz
approve` applies it. Nothing reaches the vocabulary without a human reading the report.

### 1. Three statements, none of them ours, and none of them SKOS

**SKOS defines no deprecation term.** That is a fact about the 2009 Recommendation, not a gap in
this build: it has no status vocabulary at all. `CLAUDE.md` §2 forbids inventing a proprietary
substitute for something already standardised, so the statements come from where published SKOS
vocabularies get them:

| Statement | Why this one |
|---|---|
| `owl:deprecated "true"^^xsd:boolean` | OWL 2 §5.5 defines it as an annotation property with **no logical consequences** — exactly right for a status marker. The concept means what it always meant; every inference drawn from it before is still sound. What changed is whether anyone should use it again. |
| `dcterms:isReplacedBy <iri>` | DCMI: "a related resource that supplants, displaces, or supersedes the described resource". Optional — a term can go out of use with nothing taking its place. |
| `skos:changeNote "…"` | The operator's own sentence about why. SKOS §7 separates a note about a *modification* from `skos:historyNote`'s note about a past state; retiring is the modification. |

**Only one direction of the replacement is written.** DCMI describes `dcterms:replaces` as the
converse in prose but declares no `owl:inverseOf` between the two, so asserting both would be two
claims where the standard licenses one — and the second would be a statement about the
*replacement*, which is a live concept this change has no business editing.

**Who retired it and when is not in the vocabulary.** It is in the candidate, where every other
command in this build records it. The consequence is stated rather than hidden: an export of the
vocabulary carries the fact of the deprecation and its replacement, and not its date or its author.
That is the same gap `docs/UNTESTED.md` already records for a recorded minting policy, and it is
recorded again here rather than solved by inventing a date predicate.

### 2. It removes nothing, and that is the whole operation

Not "removes nothing yet". A deprecation that retracted anything would be a deletion with extra
steps, and the reason to run one instead of a merge is precisely that every statement survives.

The consequence is a second call that works: a concept retired when it went out of use, given a
replacement months later when one is agreed. `owl:deprecated` is already there, so only the
`dcterms:isReplacedBy` is proposed, and the report says which of the two it is doing.

The same property is why a **different** replacement is refused. Changing one means retracting a
published statement; that is `openbiz retract`, run deliberately, not a side effect of typing
`--replaced-by` twice.

### 3. A replacement is a signpost, not a rewrite

`dcterms:isReplacedBy` repoints nothing. Every reference in the vocabulary still points at the
retired concept and still resolves. Rewriting them all is `openbiz merge`, and it does it by making
the old IRI stop existing — the thing a deprecation exists to avoid.

There is no operation that does both: repoint every reference at the replacement *and* keep the
retired concept. That is a real gap, it is in `docs/PROPOSED.md`, and it is not folded quietly into
this item.

### 4. What it strands is most of the report

Retiring a concept is three statements and takes a second. What it *leaves* is the work:

- **concepts still below it** — live children under a parent nobody should use again, which is the
  consequential one and is invisible in every tree view that does not check a parent's status;
- **schemes it is a top concept of** — where a retired concept heads the browse tree;
- **collections that still list it** — through `skos:member` *and* through an ordered collection's
  `skos:memberList`, because checking one property alone would miss exactly the vocabularies that
  took the trouble to order theirs;
- **concepts above it and beside it**, and **the resources it is mapped to** in other vocabularies;
- **every statement in this vocabulary pointing at it**, counted from the raw graph, including the
  ones SKOS has no reading of.

None of these is wrong afterwards, and none can be decided from the graph: a live child may want
re-parenting under the replacement or may want retiring too, and nothing in the vocabulary says
which. So the report counts and names them, and it does so **before** the diff — the order
`adr/0039` settled on, for the reason it gives: a reader who stops at "retired, replaced by X"
believes the job is finished.

### 5. What it refuses

- A concept the vocabulary says nothing about, or one that is not a `skos:Concept`.
- Retiring what is already retired with nothing new to record — a candidate that changes nothing
  spends a reviewer's attention for no decision.
- A concept as its own replacement; a replacement this vocabulary holds that is **not** a concept;
  a replacement that is **itself deprecated**, which is a trail leading nowhere.
- A second, different replacement (§2).
- A scan that hit its bound, because an incomplete scan cannot establish that a concept is *not*
  already retired, and every refusal above rests on that absence.

It does **not** refuse a replacement this vocabulary has never heard of. A term retired here in
favour of one in the corporate vocabulary next door is ordinary governance. What the operator gets
instead is the distinction only the store can draw: if some other graph in the store knows the IRI,
the report says which and how often; if **nothing anywhere** does, it warns that a mistyped IRI and
a genuine external replacement are identical statements from here.

### 6. The whole condition set is run, again

None of the three statements is SKOS, so it is hard to see how one could break a SKOS integrity
condition. That is exactly the reasoning `adr/0038` found to be wrong about a merge, so
`crate::staging::newly_broken` runs the whole set against the vocabulary the change would leave.
No input was found that trips it. The same blind spot `adr/0039` recorded applies unchanged: a
vocabulary that refines `dcterms:isReplacedBy` under a SKOS property makes the check unable to see
a violation it *causes*, because a condition this build reports as unchecked cannot become newly
violated. It is in `docs/UNTESTED.md`.

## What was measured, and what was not

A test that failed and was right to: `Stranded` counted mapping **statements** and reported a
concept mapped once as mapped twice, because SKOS §10.2 (S42) makes `skos:exactMatch` a
sub-property of `skos:closeMatch` and the model holds both. The count is now of distinct
**resources** the concept is mapped to, which is what a reviewer has to decide about. `openbiz
split`'s equivalent count has the same defect and still has it — it is in `docs/UNTESTED.md` rather
than fixed here, because fixing an already-checked-off item because you are passing through is what
the one-item rule refuses.

`Statement`'s human-readable form learned three prefixes — `prov`, `owl`, `dcterms` — because this
command's diff otherwise printed `skos:changeNote` beside a forty-character IRI for the statement
next to it. It changes `openbiz split`'s printed diff too, in the same direction.

Nothing here is measured on a large vocabulary. `StatusBound::DEFAULT` is the sixth constant in
this crate that is a judgement against nothing.

## Alternatives rejected

- **A `skos:historyNote` rather than a `skos:changeNote`.** Defensible: retirement is arguably a
  statement about a past state. The note this command writes is the operator's account of *the
  modification*, which is what §7 gives `skos:changeNote` for. A vocabulary that prefers history
  notes can write one with `openbiz import`; nothing here stops it.
- **A date predicate in the vocabulary.** `prov:invalidatedAtTime` was the candidate and it
  overreaches: PROV-O invalidation is the cessation of an entity, and a retired concept has not
  ceased to exist — that is the point. Rather than pick a predicate that says something false, the
  date stays in the candidate and the gap is recorded.
- **Removing `skos:topConceptOf` or `skos:inScheme` so a retired concept drops out of browse
  trees.** It would improve every reader immediately and it is a retraction, which this operation
  does not do. Making retired concepts fall out of ordinary reads is the *lifecycle* item, and the
  right place for it is the read paths, not a silent removal at write time.
- **Repointing references to the replacement.** See §3. It is a different operation and it is
  proposed rather than assumed.

## Consequences

- A concept can be retired without anything that references it breaking, which is the first
  operation in this build that a published vocabulary can run safely.
- `openbiz tree`, `openbiz search` and `openbiz ancestors` do **not** yet know what
  `owl:deprecated` means: a retired concept is reported exactly like a current one. That is the
  lifecycle item, it is the thing most likely to surprise an operator who has just retired
  something, and it is stated in `docs/UNTESTED.md` rather than left to be discovered.
- The fourth producer of candidates, and the third that mints nothing.
