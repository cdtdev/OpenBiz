# 0043 — Asking for current concepts only hides the hits, never the count

- **Status:** accepted
- **Date:** 2026-08-19
- **Phase:** 2 — SKOS authoring model
- **Item:** Asking a read command for current concepts only, opt-in per command

## Context

`adr/0041` decided that **no command hides a retired concept by default**, and the argument it
gave for `openbiz search` was the strongest of the five: a silo is created when someone looks for
a term, does not find it, and makes a new one, so a search that omits a retired concept reports a
term this vocabulary *holds* as one it has never heard of. That is the single reading most likely
to produce a second, worse copy of a term that already exists (`CLAUDE.md` §1.7).

The same ADR recorded that filtering them out **on request** is a real need and a separate item.
It is: a curator building a new branch, or drafting a candidate list for review, wants the
obsolete terms out of the way, and telling them to read past twenty `[retired]` markers is telling
them to use a worse tool. This ADR is that item, for `search`. The other read commands —
`tree`, `ancestors`, `paths` — are separate items, because what "leave the retired ones out" means
in a *hierarchy* is a different question with a different answer, and `openbiz inspect` gets no
such flag at all: its retirement section is a report *about* the retirements.

## Decision

### 1. `--current`, opt-in, per command, never a default

One flag, off unless typed, refused if typed twice (the parser's existing rule for options that
narrow the same thing). It composes with every other narrowing option rather than replacing them.
`adr/0041` stands unchanged: nothing about the default behaviour of any command moves.

### 2. The hits go; the fact that there were hits stays

This is the rule that makes the flag safe, and it is the whole of this ADR.

The report always ends with the count of what the flag withheld — how many labels matched, and on
how many retired concepts — together with the one sentence that gets them back: *run the same
search without `--current`*. Nothing is printed when nothing was withheld, which is every search
on the overwhelming majority of vocabularies, because they retire nothing.

**The case this exists for is the one where everything that matched was retired.** Then the list is
empty, the report says "nothing matched", and without the count that report is exactly the false
negative `adr/0041` refused to ship — worse, in fact, because the user asked for a narrowing and
would reasonably read "nothing matched" as "nothing matched, narrowed or not". Both the unit test
and the end-to-end test pin that sentence rather than the happy path.

A rejected alternative: naming the withheld concepts, or listing them under a "retired" heading.
That is a flag that does not do what it says, and a curator who asked for a clean list would get
the same list with a subheading. The count is the honest minimum: it tells them the vocabulary has
something without putting it back in front of them.

### 3. The exclusion happens inside the scan, before the bound

`CoreModel::search_excluding(query, skip)` takes a set of resources to leave out and applies it
during the scan, so the bound (`SearchBound`, 200 by default) is spent entirely on hits the caller
will see. The obvious implementation — run the search, filter its answer — is wrong in a way that
is invisible on a small vocabulary: 200 retired matches sorting ahead of the current ones would
crowd every current hit out of the bounded answer, and `is_complete()` would then report the empty
result as the whole truth. That is a false negative in the one command whose false negatives make
people create duplicate concepts, and it is the failure that
`the_bound_is_spent_on_the_hits_that_survive_the_exclusion` in `openbiz-skos` exists to catch.

`is_complete()` stays a statement about the **bound** and not about the exclusion. A search that
withheld matches reported everything it was asked for; `withheld()` is where the rest is accounted.

### 4. The model is told *which* resources, never *why*

`search_excluding` takes a `BTreeSet<Node>`. It does not know about `owl:deprecated`, and it must
not: `adr/0041` §1 keeps the retirement status *beside* `CoreModel` rather than inside it, because
`owl:deprecated` is not SKOS and a SKOS model that reads a non-SKOS status turns that boundary
from a rule into a matter of taste. The server builds the set from `Retirements` — which it has
already, from the same single pass over the store — and hands over nodes.

The consequence is that the seam is reusable by the later items without widening: a `--current`
on `tree` will hand the same set to a different walk.

### 5. What counts as current: the marker, and only the marker

The set is `Retirements::retired()` — resources carrying `owl:deprecated` — and deliberately not
everything the index knows about. The other state it records is a resource naming a successor with
**no** marker, the half-retirement that arrives by import and that `openbiz deprecate` cannot
produce. Every command in this build reads that resource as current, `openbiz inspect` reports it
as the commonest way a retirement goes wrong, and dropping it from a search for *current* concepts
would hide a term the vocabulary has not retired on the strength of a statement that does not
retire it. It stays, and it keeps its `[replaced, but not marked retired]` mark, which is the only
thing telling the reader the vocabulary is of two minds about it.

## Consequences

- `openbiz search <graph> <text> --current` leaves the retired hits out, states the narrowing in
  the header before any count that it changed, and closes with what it withheld.
- `LabelSearch` gains `withheld()` and `withheld_resources()`, both zero from `CoreModel::search`.
- The other read commands are unchanged and still show every retired concept, marked. Until their
  own items land, `--current` on `tree`, `ancestors` or `paths` is an unknown-option error rather
  than a silent no-op — which is the right failure, but it does mean the flag is inconsistent
  across the command set for as long as that takes. `docs/UNTESTED.md` carries it.
- Nothing about the write half moves, and no default changes anywhere.
