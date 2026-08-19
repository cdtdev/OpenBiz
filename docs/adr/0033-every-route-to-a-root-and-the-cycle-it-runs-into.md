# ADR 0033 — Every route to a root: "root" is two things, a route is simple, and a cycle is named with the way into it

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 36
- **Supersedes nothing.** Completes the split `adr/0032` made: that ADR took the concept tree
  downwards and sideways and deferred *routes*, which is this one.

## Context

`openbiz ancestors` (`adr/0025`) answers **which** concepts are above one concept. `openbiz tree`
(`adr/0032`) answers what is below and beside it. Nothing answered **by what routes** — the
question a breadcrumb is made of, and the question an author asks when an unexpected roll-up
appears and they want to know which way up produced it.

That is not a rendering of the ancestry. In a polyhierarchy the number of ancestors is linear in
the hierarchy and the number of routes is not: a lattice of four levels each offering two broader
concepts has eight ancestors and sixteen routes, and the disproportion grows. A concept with a
complete ancestry can therefore have an incomplete route list, from the same hierarchy at the same
moment.

Three things had to be decided rather than assumed, and the backlog item said so.

## Decision

### 1. "Root" is two notions, they are not the same set, and both are reported

§8 of the SKOS Reference states the hierarchy; §4.6 states concept schemes. **Nothing in either
relates them.** The specification's numbered statements about `skos:hasTopConcept` are S5 (its
domain), S6 (its range), S7 (`skos:topConceptOf` is a sub-property of `skos:inScheme`) and S8 (the
two are inverses). Not one mentions `skos:broader`, and §8 states no condition mentioning a top
concept.

So a route runs to a **summit** — a concept with no broader concept, which is where the hierarchy
stops — and every **top concept** the route passes through is marked *where it passes through it*,
including one part-way up. A top concept with a broader concept is legal SKOS; so is a summit that
is a top concept of nothing. Both come out of a real vocabulary and both are printed.

Collapsing them was the shorter option and it was rejected: stopping a route at a top concept
hides whatever the graph puts above it, and calling every summit a top concept invents a condition
the specification does not state. The report names the disagreement explicitly when it occurs —
"N concept(s) on these routes are a scheme's top concept without being where a route stops" — with
the S5-to-S8 reasoning attached, because a reader who found their scheme's entry point half-way up
a route will otherwise conclude the report lost it.

A mutation that stops a route at a top concept fails two tests.

### 2. A route is simple, and that is the only terminating reading

§8.6.8 marks a cycle **consistent** with the SKOS data model. A cycle makes the number of walks to
a root infinite rather than merely large, so "every path to a root" has no answer at all unless a
route is forbidden to visit a concept twice.

So the enumeration is depth-first over simple paths, with an explicit stack rather than recursion —
a 100 000-link chain is legal SKOS and recursing down one turns the bound's honest incomplete
answer into a crash, which is `adr/0032`'s reasoning for the tree renderer applied to the walker.

A vocabulary in which every way up runs into a loop therefore reports **no routes**, which is a
real and correct answer rather than a failure to find one, and the cycles are its explanation.

### 3. A cycle is named, rotated, and carries the way into it

This is the half `openbiz ancestors` cannot do. A walk from one concept reports a cycle only when
the cycle runs back through *that* concept; a loop two levels above it is invisible from there and
is still the reason that concept has no route to a root.

Three consequences, each tested:

- **Rotated to its lowest concept.** A loop has no first concept — it is wherever the enumeration
  entered it — so two routes into one loop produce two rotations of one sequence. Without the
  rotation a count of cycles is a count of ways in wearing the wrong name. A mutation dropping it
  fails two tests.
- **Carrying its approach:** the concepts before the loop on the route that ran into it. Empty when
  the loop runs through the concept asked about, which is the only case `ancestors` can report.
  Without it a reader sees routes that do reach a summit and a loop somewhere, and cannot tell that
  a whole branch above them ends nowhere. One representative approach, not all of them: the loop is
  one fact about the vocabulary however many ways there are in, and listing every approach would be
  a second exponential inside the first.
- **Explaining itself.** A loop of two or more concepts puts each of them above itself, which is an
  S24 conclusion, and the derivation names the chain. A loop of **one** — §8.6.7's Example 36,
  `<A> skos:broader <A>` — gets none, because that conclusion is S22's or the graph's own and
  crediting transitivity with it would be a citation for a step nothing took. A mutation that lets
  the one-concept loop claim S24 fails its test.

### 4. A bound of its own, with three numbers

`WalkBound` bounds a set: a breadth-first walk visits each concept once, so its cost is the size of
the hierarchy. This enumeration's cost is the number of routes. Borrowing the walk's bound would
have meant one ceiling governing two quantities that differ exponentially.

`PathBound` is three numbers because three things fail differently:

| | bounds | the case it is for |
|---|---|---|
| `max_paths` | routes recorded | the exponential one — a small hierarchy can have very many routes |
| `max_cycles` | distinct loops named | a hierarchy that records **no** routes can still find many loops |
| `max_steps` | links followed | the work rather than the answer — an abandoned route still cost the steps that built it |

The defaults are 10 000 / 10 000 / 1 000 000. The step ceiling is `WalkBound::DEFAULT`'s link
ceiling, because it bounds the same thing and `adr/0024` measured a million links as already past
what this build holds comfortably. **The route ceiling is reasoning and not measurement** — an ISO
25964 thesaurus is a handful of levels deep and a concept in one has one to three broader concepts,
which puts an ordinary worst case in the low thousands. That is uncomfortably near the ceiling
rather than safely below it, which is the opposite of `WalkBound`'s position going up, and it is in
`docs/UNTESTED.md` with the measurement that would settle it. It is not raised without one.

Hitting any of the three sets the answer incomplete, and an incomplete enumeration is
distinguishable from a complete one at every level of the API and in the report's closing sentence.
A mutation that reports an abandoned enumeration as complete fails its test.

### 5. A step says whether it is stated as a parent link

S22 makes `skos:broader` a sub-property of `skos:broaderTransitive` and not the reverse — the same
asymmetry `adr/0032` made its headline, met one level up. So a step licensed only by
`skos:broaderTransitive` says the upper concept is somewhere above the lower one and **does not say
it is directly above it**: there may be levels between them the vocabulary does not name.

A breadcrumb drawn from such a step is a true statement of containment and a false statement of
adjacency. `RouteStep::is_stated` is what tells them apart, and `openbiz paths` draws the two with
different arrows and prints the legend only when one appears — the iteration-35 lesson that a
legend printed where nothing carries the mark reads as though the reader missed one.

The enumeration itself walks `skos:broaderTransitive`, as `ancestry` does. The closure is not
stored (`adr/0025`), so a step is always a link the vocabulary holds and never a shortcut this
build invented. A mutation that walks `skos:broader` instead fails two tests: it loses every route
through a transitive-only link entirely.

## Consequences

- **The production caller is `openbiz paths <graph> <concept>`**, not an endpoint. Same reasoning as
  `inspect`, `ancestors`, `tree`, `notes` and `mappings`, and not the authentication objection: it
  only reads. The interface's breadcrumb is Phase 3's item and an endpoint now would be a caller
  with nothing behind it.
- **A concept the vocabulary does not hold is refused**, as the other reading commands refuse one.
  The confusion is sharper here than anywhere else: an unknown concept has no broader concept, so
  without the refusal the answer to a typo would be a confident "it is its own root".
- **Nothing is written back into the model**, and `Resource::relations` still means "links under
  this property" and never "routes". `adr/0025` permanently.
- **The generator still cannot make the shape this needs.** `scale.rs` builds a chain, in which
  every concept has one broader concept and therefore exactly one route up. It cannot generate a
  polyhierarchy at all, so the one input shape that exercises `PathBound` is the one shape the
  harness has never been able to produce. That is the sixth consecutive iteration recording a gap
  that is really a gap in the generator, and it is now the largest single hole in this crate's
  evidence.

## What was measured

Nothing was timed. Eight mutations were applied and all eight were killed:

| mutation | tests that failed |
|---|---|
| cycle rotation dropped, so one loop dedups by way in | 2 |
| walk `skos:broader` instead of `skos:broaderTransitive` | 2 |
| stop a route at a top concept, collapsing the two roots | 2 |
| record only the first route to each summit | 4 |
| report an abandoned enumeration as complete | 1 |
| the approach includes the loop's first concept | 3 |
| a one-concept loop claims an S24 derivation | 1 |
| a concept the vocabulary lacks is reported as its own root | 2 |

The first attempt at this pass reverted each mutation with `git checkout --`, which **fails
silently on an untracked file** — both `paths.rs` files were new — so the first three mutations
accumulated and the results were about a file with three defects in it rather than one. Caught by
reading the command's error output rather than its verdicts. The pass was redone against a file
copy, and the suite was re-run green before and after. This is iteration 33's lesson in a third
costume: a mutation you did not verify was reverted is as worthless as one you did not verify was
applied.

The runs also had to be redone with `--no-fail-fast`: cargo stops after the first failing test
binary, so the first pass never ran the `openbiz-skos` unit tests at all and reported one killing
test where there were two.
