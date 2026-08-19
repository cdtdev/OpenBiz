# 0037 — A move is one candidate with two halves, and it re-parents rather than rewrites

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 42
- **Supersedes:** nothing. Extends `adr/0019` (the two statement types), and is the first
  production caller of the removal half `adr/0018`-era work put behind `Store::propose_retraction`.

## Context

`docs/BUILD-PLAN.md` carried one Phase 2 item reading *"Bulk operations: merge concepts, split a
concept, move a subtree, deprecate with replacement"*. Four operations that share a producer and
share nothing else; it was split in place and this records the first.

The candidate seam has carried two halves — additions and removals — since iteration 18, and
`docs/UNTESTED.md` has carried an entry since then saying **nothing produces a candidate with
both**. `openbiz import` raises additions because a file is additions; `openbiz retract` raises
removals for the same reason. The combination existed in the record, in `read_record`'s invariants,
and in `apply_payload`, and had never once been built. That is the "built but no production caller"
shape `CLAUDE.md` §4 names, held deliberately open because the caller was a real plan item.

A move is the smallest of the four operations that needs both halves, so it is what closes it.

## Decisions

### 1. One candidate, not two

A move could have been expressed as a retraction plus an import. It is not, because a reviewer
would then be able to approve one and reject the other, and approving only the retraction leaves a
branch of the thesaurus hanging off nothing — a state nobody proposed, that violates no SKOS
integrity condition, and that no report in this build would call wrong. `Store::propose_edit` takes
both halves and `Store::decide` applies both inside one transaction, so the intermediate state is
not one the store can hold.

### 2. The producer computes statements; it does not serialise and re-parse them

`propose_import` and `propose_retraction` take a reader and a syntax because their input is a file.
A bulk operation's input is the vocabulary's own model, so `propose_edit` takes
`&[StatementRef<'_>]` — the borrowed statement type `adr/0019` already defined for reads, used in
the other direction. The alternative was for the operation to write Turtle into a buffer and hand
that to `propose_import`, which would mean shipping a serialiser and a parser to move one statement
between two of our own modules, and would put a syntax error on a path where no syntax exists.

The cost of the layering `adr/0019` chose is paid a second time here: `openbiz-server` converts
`openbiz_skos::Statement` into `openbiz_store::StatementRef` in the same file that converts the
other way. That is the price of neither crate depending on the other and it is still the right
trade.

**Consequence:** a computed statement has had no parser look at it. So `propose_edit` checks what a
parser would have: a literal in subject position, an IRI that is not one, a language tag that is
not one. Each is refused with the detail rather than mapped to something adjacent, because the
adjacent statement would be about a different resource and would land in a vocabulary looking
deliberate.

**And the counts are of distinct statements**, unlike the file paths. The whole change is already
in memory, so a seen-set costs nothing; `UNTESTED.md` records that the file paths count parsed
statements and cannot cheaply do the same. The producer's order is preserved for the diff.

### 3. Moving a subtree is re-parenting its root

Everything below the moved concept is below it by its **own** `skos:broader` links, none of which
mention the concept's parent. So a move touches the links between the concept and the parent it is
leaving, and nothing else. A forty-thousand-concept branch moves because the graph already says it
is below, not because forty thousand statements were rewritten.

Measured on nothing: this is a property of the model, not a benchmark result, and the subtree count
is the only thing that scales with the branch. It is a bounded walk that is refused when incomplete
(decision 6), so the cost is the ancestry walk's cost, which `adr/0024` and `adr/0025` measured.

**Consequence:** the diff is two statements and the effect can be the whole thesaurus. So the
report prints the count of what moves **before** the diff. A report that showed only the diff would
be accurate and useless.

### 4. The direction the vocabulary states a link in is preserved

S25 makes `skos:broader` and `skos:narrower` inverses, so a vocabulary may state either and mean
the same hierarchy. A move that always wrote `skos:broader` would silently convert a vocabulary
authored in `skos:narrower`: an export would come back different from what went in, for a reason
nobody chose.

So each direction the graph *asserts* between the concept and its old parent is removed, and the
same directions are added between the concept and its new one. `RelationOrigin::Asserted` is what
makes this answerable — an entailed link is not a statement in the graph, and proposing to remove
one would name something that is not there, which `propose_retraction` would refuse.

What is **not** added is the inverse of what was. Writing both directions when the graph states one
would be recording an entailment as a fact.

### 5. Which link is being replaced must be unambiguous

A polyhierarchic concept has several broader concepts and a move replaces exactly one. Choosing for
the operator would be a coin toss whose result is permanent, so a concept with more than one parent
requires `--from`. `--from` is an option and not a fourth positional because it is the minority
case, and four IRIs on a command line to say "move this under that" is a worse default than one
extra flag for the concepts that need it.

A concept with **no** broader concept is refused rather than given one. Giving a concept its first
parent is a different operation: a top concept that gains one should stop being a top concept, and
this does not do that (see the gap below).

### 6. Everything the operation refuses is consistent SKOS

This is the reason the checks are here at all. §8.6.8 says a cyclic hierarchy is *consistent*, so a
move into the concept's own descendant produces a vocabulary that passes every integrity condition
in `openbiz integrity` and has a branch with no route to a root. Nothing downstream catches it.

The cycle check is a bounded downward walk, and it is the **same walk** that counts the subtree, so
the number the report quotes cannot disagree with the check that allowed the move. An incomplete
walk cannot prove the new parent is not below the concept, so the move is refused rather than
proceeding on an unrun check.

A `skos:broaderTransitive` or `skos:narrowerTransitive` link stated *directly* between the concept
and the parent it is leaving is also refused. S22 lifts every `skos:broader` into
`skos:broaderTransitive`, so the transitive link is normally an entailment that disappears with the
statement licensing it; a graph stating it directly has said something the move does not remove,
and the concept would still be under its old parent by S24 while every report said it had moved.
Refused rather than quietly removed as well, because a directly-stated transitive link is unusual
enough that the author meant something by it.

## What was rejected

- **Removing the top-concept statements as part of the move.** A concept that gains a broader
  concept should arguably stop being a `skos:topConceptOf` its scheme. Rejected on two grounds.
  First, this operation *requires* an existing broader concept, so a concept it can move was
  already both — the oddity predates the move and is neither created nor worsened by it. Second,
  the core model closes S8 into both `top_concept_of` and `has_top_concept` without recording which
  direction the graph asserted, so we could not compute a removal that is guaranteed to be present.
  It is **reported** in the move's own report instead, and the model gap is in `UNTESTED.md`.
- **Refusing to move a top concept.** Same reasoning: refusing a state we did not create, and which
  is legal SKOS, is a nuisance rather than a guard.
- **Rewriting the subtree.** Nothing to rewrite; see decision 3.
- **Making `move` an HTTP endpoint.** The same objection `openbiz import` records: there is no
  authentication, and `POST /api/move` would be an unauthenticated way to re-hang a branch of
  somebody's thesaurus. `BLOCKED.md` already holds the seam-over-HTTP item.

## Consequences

- `docs/UNTESTED.md`'s "nothing produces a candidate that both adds and removes" is **closed**, the
  way the entry asked: both halves land, proved against the real binary by reading the store back
  off disk, and the removals-before-additions order is pinned by a store test staging one statement
  in both halves — the only shape that can observe it, and one no producer here computes.
- `CandidateSource::BulkEdit` has its first producer.
- The next three bulk operations have a producer to slot into rather than a seam to build.
- Three gaps opened, all in `UNTESTED.md`: no first-parent operation and therefore no top-concept
  demotion; a directly-stated transitive link to a *non-adjacent* ancestor is not examined; and the
  subtree count has never been run against anything larger than a handful of concepts.
