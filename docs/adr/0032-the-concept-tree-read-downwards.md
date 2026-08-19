# ADR 0032 — The concept tree read downwards: a child is not a descendant one step down, and "sibling" is our word

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 35
- **Supersedes nothing.** Completes the other half of `adr/0025`'s decision — that S24's closure is
  a walk and not a table — by walking it in the second direction, and renames the bound that
  decision introduced.

## Context

`openbiz ancestors` (`adr/0025`) answers what is *above* a concept. Nothing answered what is below
it or beside it, and those are the questions a concept tree is made of: a taxonomist opening a
vocabulary looks down from a top concept far more often than up from a leaf.

The backlog item is "Concept tree query API: children, ancestors, siblings, paths-to-root, with
cycle detection". Ancestors was already done. The item was **split in place** into two, and this
ADR records the first: children, descendants and siblings — the downward and sideways reading.
Every path to a root, and naming the cycle a path runs through, is part 2, because enumerating
*routes* is a different problem from reaching *nodes*: the number of ancestors is linear in the
hierarchy and the number of paths to a root is not.

## Decision

### 1. One walk, two directions

The upward and downward walks are the same breadth-first traversal over inverse properties. That is
not a convenience: S25 and S26 make the model close each direction into the other, so
`<A> skos:narrowerTransitive <B>` is present exactly when `<B> skos:broaderTransitive <A>` is.

`hierarchy.rs` therefore holds the traversal, the bound and the predecessor map, and `Ancestry` and
`Descent` are thin readings of it that know which property they walked — which is what lets each
cite the statement that licensed its conclusions, as `CLAUDE.md` §3 requires. A bare walk could not.

A test asserts the two directions agree over every ordered pair of a four-concept polyhierarchy
with a cycle in it, so a defect found in one direction cannot survive in the other.

### 2. `AncestryBound` is now `WalkBound`, and its `max_ancestors` is `max_nodes`

A mechanical rename with one substantive reason: the same bound now governs a walk that has no
ancestors in it. Leaving the old name would have meant a downward walk bounded by a field whose name
says the opposite of what it counts.

**And the numbers mean different things in the two directions, which is the part worth recording.**
`WalkBound::DEFAULT` is 100 000 nodes and 1 000 000 links, chosen in `adr/0024` for the upward walk,
where they are a backstop against a pathological graph — an ISO 25964 thesaurus is conventionally a
handful of levels deep, so an ordinary vocabulary is nowhere near them.

Downwards they are **reachable by an ordinary vocabulary**. Everything below a top concept is most
of the vocabulary, so a walk down from the root of a 100 000-concept thesaurus hits the ceiling
*because the vocabulary is large*, not because it is hostile. That is not a reason to remove the
bound and not a reason to raise it silently. It is the reason `Descent::is_complete` matters more
going down than going up, and why the report's closing sentence is different in the two cases rather
than decorative. Earlier ADRs name the old type; they are historical and are not rewritten.

### 3. A child is not a descendant one step down

This is the substance of the item and it comes straight out of S22.

S22 makes `skos:narrower` a **sub-property of** `skos:narrowerTransitive`. Entailment runs from
sub-property to super-property and not back, so:

- `<A> skos:narrower <B>` entails `<A> skos:narrowerTransitive <B>`. B is a child **and** a
  descendant.
- `<A> skos:narrowerTransitive <B>` entails **nothing** about `skos:narrower`. B is a descendant and
  **not** a child, and A has no children at all.

Both are legal SKOS and the second is not exotic — it is what a vocabulary states when it knows one
concept is somewhere under another without claiming to know the levels between. So
`CoreModel::children` reads `skos:narrower` and `CoreModel::descent` walks `skos:narrowerTransitive`,
and the concepts reachable by following children are a subset, sometimes a strict one, of the
descendants.

Collapsing the two would have been less code and would have put statements in the graph's mouth.
`openbiz tree` reports the difference **when a vocabulary actually shows it**, naming S22, rather
than leaving two counts to disagree in silence.

### 4. "Sibling" is our word, and it is labelled as ours

SKOS has no sibling property and §8 states nothing about one; ISO 25964's relationships are BT, NT
and RT. `Siblings` is therefore a query over the model and not an entailment, defined here:

> A sibling of a concept is another concept that has at least one `skos:broader` concept in common
> with it.

Three consequences follow from that sentence, each tested and each a decision:

1. **One step up and one step down, not transitive.** A concept under the same *grandparent* is not
   a sibling. Widening it to the transitive properties would make every concept under a large top
   concept a sibling of every other — a true statement about the closure and a useless answer.
   The same asymmetry as §3 applies upwards: a concept whose only upward link is a stated
   `skos:broaderTransitive` has no broader concept and therefore no siblings. That case is pinned by
   a test, because on every ordinary vocabulary — where S22 fills the transitive property from
   `skos:broader` anyway — walking the wrong one of the two would be invisible. **It was invisible:
   the mutant that swaps them survived the first suite and is what the test was written for.**
2. **A concept is never its own sibling.** §8.6.7's Example 36 (`<A> skos:broader <A>`) is
   consistent and makes A its own parent and its own child, from which the definition would make A
   its own sibling. Excluded, because "another concept" is what the word means.
3. **Two top concepts are not siblings.** They share no broader concept, so nothing here relates
   them. What makes them belong together is `skos:hasTopConcept` from a shared scheme — a different
   question — and inventing a relation from the *absence* of a link would claim something the graph
   does not say.

Because no statement licenses a sibling, nothing emits a `Derivation`: a fabricated rule number is
worse than no citation. What is returned instead is the concept **shared**, so any sibling reduces
to two `skos:broader` links the model already explains individually.

### 5. In a tree, the indentation is the derivation — and what the tree cannot show is printed

`openbiz ancestors` prints the full path against each ancestor, because for a transitive conclusion
the path *is* the derivation. A subtree is different in shape rather than in kind: printing the whole
path against each of a thousand descendants repeats every prefix once per leaf under it, and the
result is unreadable for exactly the reason the tree is readable. So the path is printed once, as
structure, and each concept that is a transitive conclusion rather than a stated link is marked
`[S24]` — with the legend printed only when something on the page carries the mark. Nothing is
withheld: `Descent::derivation_to` still renders the full chain for any single descendant.

**A tree gives each concept one parent, and that is the one place this report's shape can say
something the graph does not.** A concept below the origin by two routes is printed once, under the
shorter; a reader seeing Buildings under Property alone would conclude it is not also under
Vehicles. So the routes the tree could not show are counted and named after it. Polyhierarchy is
ordinary in a thesaurus, §8 states nothing against it, and ISO 25964 relies on it — so this is not a
finding and is not phrased as one. **It was found by running the command against a store on disk,
not by a test**, which is the seventh iteration running that doing so changed the output.

### 6. Rendering uses an explicit stack, and a cycle is printed rather than cut

A 100 000-link chain is legal SKOS — §8 states no condition on depth — so recursing down a subtree
would overflow the stack, turning the bound's honest incomplete answer into a crash.

§8.6.8's Example 37 says a cycle is consistent, and a cycle puts the origin back among its own
descendants. The renderer marks what it has printed, and the origin arriving a second time is
printed as *the hierarchy comes back round to the concept asked about* rather than silently
dropped — because that is the one structural fact an author most needs to see. Full cycle detection,
which names every cycle rather than the one through the origin, is part 2.

## Consequences

- `openbiz tree <graph> <concept>` is the production caller. It only reads, so it is a command for
  the same reason `inspect`, `ancestors`, `notes` and `mappings` are: the interface's concept tree is
  Phase 3's item, and an endpoint now would be a caller with nothing behind it.
- 690 Rust tests pass, up from 661. No new dependency.
- Six mutants were run. Five were killed by the suite as written. The sixth — swapping the sibling
  search's upward step from `skos:broader` to `skos:broaderTransitive` — **survived**, and the test
  that now kills it is §4.1's.
- **Not done and recorded in `docs/UNTESTED.md`:** the downward walk's cost is unmeasured at any
  scale, and `scale.rs` still generates the one hierarchy shape that makes it look cheap; and the
  default bound is a number nobody here has hit going down, on a vocabulary nobody here has seen.
