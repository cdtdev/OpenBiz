# ADR 0030 — S45's closure is a walk over an undirected cluster, and S46 is re-checked across it

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 33
- **Supersedes nothing.** Completes `adr/0029` (mapping properties, part 1) by taking the one part
  of §10 it deferred, and applies `adr/0025`'s rule — materialise what the schema bounds, walk what
  the data bounds — to `skos:exactMatch`.

## Context

§10.2 of the SKOS Reference makes `skos:exactMatch` an `owl:TransitiveProperty` (**S45**), so a
chain of exact mapping links entails a link between its ends. §10.6.3's Example 62 is the
specification's own statement of it. `adr/0029` closed the rest of §10 and left this out, on the
ground that a transitive closure is walked rather than stored and the walk was an item of its own.

That deferral had a cost sharper than a missing conclusion, and `docs/UNTESTED.md` recorded it as
such: **S46 was checked only over the links the model held**. A vocabulary stating
`<A> exactMatch <B>`, `<B> exactMatch <C>` and `<A> broadMatch <C>` violates §10.4 — but no
statement in it names an exact match and a hierarchical one between the same pair, so the direct
check saw nothing and the report said "no SKOS integrity condition is violated". A false negative
on an integrity condition is worse than a missing entailment, because the operator has been told
something.

And the shape is not contrived. It is the **hub mapping**: a house vocabulary is mapped to an
industry hub, the hub is mapped onwards to a regulator's list, and nothing in the house vocabulary
mentions the regulator at all. That is the ordinary enterprise artefact this product exists to
serve.

## Decision

### 1. The closure is walked, never stored — and the argument is stronger here than for S24

`adr/0024` measured what materialising S24's closure would cost and `adr/0025` refused it: a legal
100 000-link chain closes to 5 000 050 000 pairs, so the bound on what may be stored is not "large"
but *unbounded on permitted input*.

`skos:exactMatch` is worse, and the reason is worth stating because it is not the same reason.
`skos:broaderTransitive` is directed, so a chain of *n* concepts closes to *n(n−1)/2* pairs.
`skos:exactMatch` is **symmetric as well as transitive** — S44 and S45 together — so its closure
over the same chain is all *n²* ordered pairs, every one of which S44 then requires in both
directions. A hub with a thousand vocabularies mapped onto it is **one cluster**, and storing its
closure is a million links produced from two thousand statements.

`CoreModel::exact_match_cluster` is therefore a bounded breadth-first walk, and
`Resource::mappings_of` keeps meaning "one-step links under this property" permanently. The
`example_62_closes_by_walking_and_not_by_storing` test asserts *both* halves, so a later build that
quietly materialises the closure fails it.

### 2. The walk is over a **cluster**, not a path, and the difference is visible in the output

`Ancestry` answers "what is above this concept". `ExactMatchCluster` answers "what is this concept
interchangeable with", and because S44 has already put every link at both ends, that is a connected
component of an undirected graph. Two consequences, both asserted rather than left to inference:

- **Cycles are ordinary rather than pathological.** §10.6.6 warns outright that "applications must
  be able to cope with cycles in skos:exactMatch and skos:closeMatch", and after S44 *every* link
  is one. A walk that did not expand each concept exactly once would not terminate on a
  two-statement vocabulary.
- **The origin is a member of its own cluster** whenever it has any exact match at all:
  `<A> exactMatch <B>` gives `<B> exactMatch <A>` under S44, and those two give `<A> exactMatch <A>`
  under S45. §10.6.6's Example 66 marks a reflexive mapping consistent, so this is a conclusion and
  never a defect. It is **printed**, because a conclusion the build draws and then hides is one an
  operator cannot check — but printed apart from the concepts the author asked about, under its own
  heading and with §10.6.6 quoted beside it. See §5 below for how that was found.

### 3. `skos:closeMatch` is not closed, and never will be

§10.1 says `skos:closeMatch` is not transitive precisely so that chaining it across schemes does
not compound errors. The mirror walk is one line of code and it is a charter violation dressed as
symmetry: it would state what the author declined to state. `a_close_match_chain_is_not_walked`
pins the absence.

### 4. S46 is checked twice, over two different things, and reports each pair once

- `check_mapping_disjointness` — the pre-existing pass, over the links the model **holds**. Cheap,
  exhaustive, and it has the statement the author actually wrote.
- `check_exact_match_closure_disjointness` — the new pass, over what S45 adds. One walk per concept
  holding an exact match, and a vocabulary with none pays nothing.

They cannot double-report: `ExactMatchCluster::entailed` yields only chains of two links or more,
so a pair joined *both* directly and through a chain is reported once, by the first pass, which has
the better citation. `a_pair_clashing_both_directly_and_through_a_chain_is_reported_once` asserts
it, and mutating the threshold from `>= 3` to `>= 2` fails that test.

The new finding carries the **chain**, not the endpoints, for the reason S27's carries a path: an
author told "these two clash" for a link they never wrote has been given a verdict; one shown the
chain has been given the edit to make.

**What is deliberately not checked:** S46 against the *transitive* hierarchy. §10.4 states the
disjointness on `skos:broadMatch`, `skos:narrowMatch` (through §10.4's own note) and
`skos:relatedMatch` — the one-step mapping properties. It does not state it against
`skos:broaderTransitive`. The lifted hierarchy is covered instead by S27, which §8.4 *does* state
transitively, and `adr/0029` is why those lifts are there to be walked.

### 5. The sweep's budget is shared, and the first test of that was wrong

`adr/0027` recorded, at real cost, that a per-walk budget multiplied by one walk per concept is not
a bound. The sweep here was written with a shared budget from the start, and
`the_closure_budget_is_shared_across_the_sweep` was written to protect it.

**That test's first draft did not.** It used a single three-concept chain and a budget one walk
exhausts — and a per-walk budget passes it, because either reading reports the sweep giving up when
the first walk is the one that runs out. The mutant survived. The test now uses **five separate
two-concept clusters** and a budget for two and a half walks, which is the only arrangement the two
readings disagree about: sharing stops the sweep partway with concepts left unwalked, a fresh copy
per walk finishes all five and reports nothing.

This is recorded rather than quietly fixed because the lesson is not "S45's sweep needed a better
test". It is that **a test written against a known failure mode can be shaped so it cannot observe
that failure mode**, and the only thing that caught it was mutating the code the test exists to
protect. The equivalent S27 test was mutated the same way and is sound.

### 6. Two new `Unchecked` findings, not one

`ExactMatchClusterBoundReached` says *one concept* sits in a cluster too large for one walk, and the
sweep goes on. `ExactMatchSweepExhausted` says the **check itself stopped**, and the concepts it
never reached are unchecked without anything being wrong with them individually. Collapsing them
would name one concept and stay silent about the rest, which reads exactly like "the others were
checked and were fine". Both are `Severity::Unchecked`, so `checks_are_complete` is false and the
report will not claim a check it did not finish. This mirrors §8.4's pair exactly, which is the
point: the same failure deserves the same shape.

## What was measured

**The dense case, and it contradicted the arithmetic that was about to go into this ADR.**

The first draft of this section said nothing was measured and gave the reasoning instead: a
concept-for-concept mapping gives clusters of two, two links per concept, and reaches the
million-link default at about 500 000 mapped concepts — comfortable. That is true and it is the
easy shape.

The sweep walks once per *member*, and every member of a cluster has the same cluster. So a **hub**
— *n* vocabularies all declaring their concept equivalent to one central concept — is one cluster
walked *n* times, and the cost is quadratic in the cluster rather than linear in the vocabulary.
Measured: **220 links for 10 members, 20 200 for 100, 321 200 for 400** — about 2n².
`the_sweep_cost_is_quadratic_in_a_cluster_and_not_linear_in_the_vocabulary` pins the complexity as
a band, so a change of traversal order is not a failure but a change of complexity is.

A **1 000-member cluster therefore exhausts `EquivalenceBound::DEFAULT` on a vocabulary of a
thousand concepts**, and the report would say S46 is unchecked. That is honest and useless.

**What was not done about it, and why.** The fix is to walk each connected *component* once rather
than each member, which makes the sweep linear — *n* walks recompute one answer *n* times. It
changes what `ExactMatchClusterBoundReached` means (there is no longer one walk per concept to
attribute it to), so it is a design change rather than an optimisation, and this item was already
the whole of §10's part 2. It is recorded in `docs/UNTESTED.md` as the obvious next move.

**What is still unmeasured** is whether it matters: no fixture in this repository has a cluster
larger than four, `crates/openbiz-skos/src/scale.rs` generates no mapping links at all, and this is
the second iteration running to say so. The honest claim is that §10's examples pass, the dense
case has a number, and no *real* mapped vocabulary has ever been read by this build.

## Consequences

- Example 62 is entailed, and `openbiz mappings <graph> <resource>` is its production caller
  alongside the S46 sweep.
- The `openbiz inspect` sentence claiming S45 is unimplemented was **false the moment this landed**
  and is replaced rather than deleted: the counts are still one-step links, and the report now says
  so and names the command that resolves them.
- `docs/UNTESTED.md`'s S45 entry is closed. Its per-concept-view entry is closed. Its
  never-measured entry is not, and is widened.
- A fifth `Rule` is now reachable from a report — S45 — and every place that renders one already
  quotes the statement rather than the number.
