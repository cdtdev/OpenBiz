# 0044 — A narrowed tree drops a branch only when the whole branch is retired

- **Status:** accepted
- **Date:** 2026-08-19
- **Phase:** 2 — SKOS authoring model
- **Item:** Asking for current concepts only — `openbiz tree`

## Context

`adr/0043` gave `openbiz search` a `--current` flag under one rule: **it hides the hits and never
the fact that there were hits.** It also said, explicitly, that the browse commands are separate
items, because what "leave the retired ones out" means in a *hierarchy* is a different question
with a different answer.

This is that question, and the reason it is different is `adr/0040`. `openbiz deprecate` retires a
concept and deliberately touches nothing below it — the children may want re-parenting under the
replacement, or retiring too, and nothing in the graph says which. So **a retired concept with
current concepts below it is the commonest outcome of a retirement**, not an edge case. A flat list
can drop a hit and lose nothing else. A tree cannot: dropping that concept takes its live children
with it.

Three candidates, all of them defensible on the face of it:

1. **Lift the children** to the nearest surviving ancestor. Sound with respect to the closure —
   S24 does entail `Telegraphy skos:narrowerTransitive Morse` across a retired `Wireless` — but the
   tree's own legend says a concept at depth 1 is a *stated* child and one deeper carries `[S24]`.
   Lifting makes that legend false unless the marking rule is rewritten, and even then it hides the
   thing the curator most needs: the route to those children runs through an obsolete concept, and
   they are the person who has to decide what to do about it.
2. **Refuse the combination** — error when `--current` meets a retired interior concept. A command
   that disobeys the arguments it was given, and a flag whose availability depends on the data.
3. **Keep the retired concept as a structural line.** Costs one visible retired concept in a report
   that asked for none.

## Decision

### 1. A branch goes only when the whole branch is retired

`--current` removes a concept from the tree exactly when removing it changes nothing about any
concept that stays. A retired concept lying on the tree's path to a surviving one is **kept as a
route** and marked `[retired, kept as the route to what is below]`; a retired concept with nothing
current under it is dropped, taking its subtree — which is retired all the way down — with it.

The consequence is the property that makes this safe to reason about: **nothing moves.** Every
concept the narrowed tree shows keeps the depth, the parent, and the derivation the unnarrowed tree
gave it, so the pruning can never make the tree state a link the graph does not.
`the_flag_removes_concepts_and_never_moves_them` compares the two reports' indentation and fails if
anyone changes that.

Option 1 was rejected for the reason above; option 2 because a flag that is an error on some
vocabularies and not others is worse than a flag that is honest about what it kept.

### 2. A list is not a tree, and is narrowed as a list

The children, the "below without being a child" list, and the siblings are lists: nothing in them
is structural, so a retired concept in one is simply dropped and counted. That is `adr/0043`'s
answer applied unchanged, including its hard case — when *every* child is retired the report says
so, because an empty children list under a concept that has children reads as a leaf.

### 3. The counts are the whole safety of the flag, as they were for `search`

Every list and the tree end with what was withheld: how many were dropped, how many are shown as
routes, and the sentence that gets them back — run the same command without `--current`. The case
that matters is the one where every descendant is retired: the tree is empty, and without the count
that report says a concept is a leaf when the vocabulary holds a subtree under it. That is the same
false negative `adr/0041` refused to ship, and
`tree_current_only_still_admits_that_a_retired_subtree_is_there` pins it end to end rather than
pinning the happy path.

The concept the report is **about** is never filtered — the reader named it — and it says so.

### 4. The walk is not narrowed; the rendering is

This is where a hierarchy genuinely departs from `adr/0043` §3, which put the exclusion *inside*
the scan so the bound was spent on hits the caller would see. Here the excluded concept may be the
only route to the concepts the caller does want, so it **has to be walked through**. `--current`
therefore spends the walk's bound exactly as the unnarrowed command does, and a tree that hit its
bound reports the same lower bound either way.

So the seam is `Descent::excluding(skip) -> Pruned`, over a finished descent: it is handed a
`BTreeSet<Node>` and never told why those nodes are in it, which keeps `owl:deprecated` out of a
SKOS crate (`adr/0041` §1). `Pruned` answers `shows`, `is_route`, and the three counts, and
`every_descendant_is_kept_a_route_or_dropped_and_nothing_else` asserts they account for the whole
descent — counted rather than derived by subtraction, so the numbers cannot silently disagree.

### 5. What counts as current is unchanged

`status::retired_in` — resources carrying `owl:deprecated`, and deliberately not a resource naming
a successor without the marker. That is `adr/0043` §5 verbatim, and it is now one function shared
by both commands rather than the same paragraph written twice.

## Consequences

- `openbiz tree <graph> <concept> --current` shows the current concepts below and beside one
  concept, keeps the retired ones that current concepts hang off, and closes with the counts.
- `openbiz-skos` gains `Descent::excluding` and `Pruned`. `CoreModel` is unchanged.
- `openbiz ancestors` and `openbiz paths` still have no such flag; that is the next item, and it is
  a third question again — not which concepts to drop but what to say about a *route* that runs
  through a retired one.
- Nothing about any default moves. `adr/0041` stands.
