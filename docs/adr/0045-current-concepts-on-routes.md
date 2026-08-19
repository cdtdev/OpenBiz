# 0045 — A route is offered whole or not at all; a concept above is above whatever the way there

- **Status:** accepted
- **Date:** 2026-08-19
- **Phase:** 2 — SKOS authoring model
- **Item:** Asking for current concepts only — `openbiz ancestors` and `openbiz paths`

## Context

`adr/0043` gave `openbiz search` a `--current` flag under one rule — **it hides the hits and never
the fact that there were hits** — and said the browse commands were separate questions.
`adr/0044` answered the first of them for `openbiz tree`: a branch goes only when the whole branch
is retired, because `openbiz deprecate` retires a concept and touches nothing below it
(`adr/0040`), so a retired concept with current concepts under it is the *commonest* outcome of a
retirement rather than an edge case.

`ancestors` and `paths` are the third question, and the plan named why: both answer about
**routes**. Looking down, the flag decides which concepts to drop. Looking up, the concepts above
are rarely the problem — a route *to* them is. A route that runs through a retired concept is not
a route a breadcrumb should offer, and is also not a route that has stopped existing, because
retiring removes nothing.

The trap is answering both commands the same way. They ask different questions and the same rule
gives one of them a false negative.

## Decision

### 1. `openbiz ancestors --current` narrows the list and never the derivation

`ancestors` answers **which concepts are above one concept**. A concept reachable only *through* a
retired concept is still above it: S24's conclusion holds on links that a deprecation left exactly
where they were. Hiding it would suppress a current concept on the strength of another concept's
status — the false negative `adr/0041` refused to ship, seen from underneath.

So the *list* is narrowed as a list, which is `adr/0043` applied unchanged: a retired ancestor is
dropped and counted. But every surviving ancestor keeps the path that reached it, **printed
unchanged, retired concepts and all**. The path is the derivation `CLAUDE.md` §3 requires; editing
a concept out of the middle would make the report state a link the graph does not, which is
`adr/0044`'s "nothing moves" property in its other direction.

The retired concepts left standing on those paths are lifted into one section and named, following
the shape `paths` already uses for the same reason: a chain with `[retired]` hung off every third
concept stops being readable as a chain. Naming them is not optional — under `--current` a reader
has been told the retired concepts are out, so one appearing in a printed path unremarked reads as
a bug in the flag.

### 2. `openbiz paths --current` withholds whole routes and never edits one

`paths` answers **by what routes**, and a route is atomic. Removing a concept from the middle of
`A → B → C` yields `A → C`, which asserts an adjacency the vocabulary does not state — the exact
failure `adr/0044` rejected option 1 (lifting children) to avoid. There is no partial answer here
to give.

So: **a route is offered only if every concept on it is current**, and it is offered whole. A route
touching a retired concept anywhere is withheld and counted. Nothing is trimmed, merged, or
shortened.

The origin is never filtered — the reader named it — and the report says so, as `tree` does.

### 3. Cycles are never narrowed, and the report says that too

A cycle is not a route offered to a reader; it is the *reason* a route reaches no summit, and
§8.6.8 makes it consistent SKOS rather than a finding. Withholding a cycle because it runs through
a retired concept would suppress a real structural fact about the hierarchy on grounds that have
nothing to do with it, and would leave a report saying "no route reaches a summit" with no
explanation printed.

This is the same exemption `openbiz inspect` has for the opposite reason: a section that is *about*
the hierarchy's shape is not narrowed by a flag about concept status.

### 4. The counts are the whole safety of the flag, and the empty case is what they are for

As in `adr/0043` §3 and `adr/0044` §3, every narrowed section closes with what it withheld and the
sentence that gets it back. Two cases carry the weight, and both are the case where the flag
withholds *everything*:

- **Every ancestor is retired.** Unsaid, the report prints "nothing is above it: it has no broader
  concept" — which says a concept is a root of the hierarchy when the vocabulary puts things above
  it. `ancestors_current_only_still_admits_the_retired_concepts_above` pins the replacement
  sentence.
- **Every route runs through a retired concept.** Unsaid, the report prints "no route from it
  reaches a concept with no broader concept: every way up runs into a cycle" — which blames a cycle
  that may not exist, and reads as a broken hierarchy rather than an obsolete one.
  `paths_current_only_never_blames_a_cycle_for_a_withheld_route` pins that this sentence cannot
  appear under the flag when routes were withheld.

Both are a false negative about the hierarchy above a concept, which is how the *wrong* parent gets
chosen for a new one.

### 5. The walk is not narrowed; the rendering is

`adr/0044` §4 unchanged, and for the same reason twice over. Upwards, a retired concept has to be
walked **through** to reach the current concepts above it; and a route has to be enumerated in full
before anything can tell whether every concept on it is current. So both bounds — `WalkBound` and
`PathBound` — are spent exactly as the unnarrowed commands spend them, and a truncated answer says
so identically either way.

The seams are `Ancestry::excluding(skip) -> Above` and `RootPaths::excluding(skip) -> Offered`,
each over a finished walk, each handed a `BTreeSet<Node>` and never told why those nodes are in it.
That is `adr/0041` §1: `owl:deprecated` is not SKOS and does not enter `openbiz-skos`. `Above` and
`Offered` count what they withheld rather than deriving it by subtraction, so the numbers cannot
silently disagree with the walk they came from.

### 6. What counts as current is unchanged

`status::retired_in` — resources carrying `owl:deprecated`, and deliberately not a resource naming
a successor without the marker. `adr/0043` §5 and `adr/0044` §5 verbatim, now one function shared
by four commands.

## Consequences

- `openbiz ancestors <graph> <concept> --current` lists the current concepts above one concept,
  keeps their derivations intact, names the retired concepts those derivations run through, and
  closes with what it left out.
- `openbiz paths <graph> <concept> --current` offers only the routes that are current the whole way
  up, and says how many it withheld.
- `--current` now exists on all four browse and search commands, with three different rules,
  because they answer three different questions. Every one of them obeys `adr/0043`'s single rule:
  hide the concepts, never the fact that there were concepts.
- `openbiz-skos` gains `Ancestry::excluding`/`Above` and `RootPaths::excluding`/`Offered`.
  `CoreModel` is unchanged.
- `openbiz inspect` still gets no such flag, for the reason the plan records: its retirement
  section is a report *about* the retirements.
- Nothing about any default moves. `adr/0041` stands.
