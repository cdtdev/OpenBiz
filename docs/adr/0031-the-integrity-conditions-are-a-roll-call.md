# ADR 0031 — The integrity conditions are a roll-call, and "unchecked" is a verdict of its own

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 34
- **Supersedes nothing.** Completes the SKOS integrity work begun in `adr/0017`'s wake and carried
  by `adr/0020` (§5.4), `adr/0025` (§8.4) and `adr/0030` (§10.4), by making the *coverage* claim
  itself checkable rather than implied.

## Context

By iteration 33 every integrity condition the SKOS Reference states was implemented, each by the
item that owned its section, and each with a test citing its S-number. The backlog item "all SKOS
integrity conditions from the specification, each with a test citing its S-number" was therefore
almost closed by accretion — which is exactly the situation in which a build starts claiming more
than it can show.

Two things were missing, and neither is a rule.

**First, nothing enumerated them.** "Which conditions does this build check?" could only be answered
by reading the source. A specification-conformance claim that lives in prose is one nobody can
falsify, and `CLAUDE.md` §4 forbids exactly that: *never claim a standard is supported when only
the happy path is.*

**Second, and much sharper: the product could not say what it had failed to check on a given
vocabulary.** `openbiz inspect` ends in one sentence — the graph violates an integrity condition, or
it does not — with a caveat appended when any bounded walk gave up. That sentence is true and it is
read as something stronger. "No SKOS integrity condition is violated" is heard as "all of them were
checked and all of them held", and on two kinds of vocabulary that reading is false:

1. **A bounded walk stopped.** `Severity::Unchecked` and `CoreModel::checks_are_complete` already
   exist for this (`adr/0027`), but they answer *for the whole model*. One exhausted ancestry walk
   makes the model incomplete, which reads as though every condition were in doubt — when what the
   bound actually cost is §8.4's check and nothing else.
2. **The vocabulary uses an extension point this build reads past.** §7.1 offers
   `rdfs:subPropertyOf`, and `adr/0028` resolves it — **for the seven documentation properties
   only**. `rdfs:subClassOf` is not read at all. So a thesaurus declaring
   `ex:seeAlso rdfs:subPropertyOf skos:related` has its own associative links read as non-SKOS, and
   §8.4 is then checked over a graph missing them. Nothing anywhere said so. This is the same class
   of defect `adr/0030` found in S46 — a false "no violation" produced by an entailment we chose not
   to perform — one level up.

## Decision

### 1. Sixteen conditions, in two groups, as data

`openbiz-skos/src/integrity.rs` holds `CONDITIONS`: every statement whose violation makes this build
call a graph inconsistent, each with its S-number, the section that states it, what it forbids in
one clause, and the SKOS terms it is checked over.

**Six are the specification's.** §4.4, §5.4, §8.4, §9.4 and §10.4 are the five sections headed
"Integrity Conditions", and between them they state S9, S13, S14, S27, S37 and S46. That is all of
them; a test asserts the list is exactly those six and that only they cite a `§N.4`.

**Ten are ours.** S3, S18, S30, S38 (a literal where an object property takes a resource), S49 (the
mirror), S52 (two literal forms), S48 and S58 (SKOS-XL disjointness), S53 and S59. None sits under
an "Integrity Conditions" heading — Appendix B has none at all, which the `xl` module already
records — and a violation of each is nonetheless a logical contradiction. They are in the table
under `Authority::OurReading`, printed apart, and labelled as our judgement.

Leaving them out was considered and rejected. A report saying "all six of the specification's
integrity conditions held" about a vocabulary this build calls inconsistent is worse than a longer
table, and it is the failure mode `docs/COMPETITIVE.md` records of the incumbents in the one place
we claim to be better than them.

That split buys a property worth more than the table: **every `Severity::Inconsistent` finding is
attributed to a row**, asserted by a test over one of every `Finding` variant, so a graph is
consistent exactly when no row is violated. A finding added later that forgets to register fails
that test and fails to compile in `violated_by`, whose match is exhaustive by name rather than by
wildcard.

### 2. Held, violated, unchecked — and the third is not a weaker second

`Verdict::Unchecked` means the check did not run over the whole vocabulary, so the condition has no
verdict. Two things produce it, and both are attributed **per condition** rather than for the model:

- A bounded walk that stopped. `AncestryBoundReached` and `DisjointnessSweepExhausted` leave S27
  unanswered and say nothing about anything else; `ExactMatchClusterBoundReached` and
  `ExactMatchSweepExhausted` leave S46. `RefinementBoundReached` leaves **nothing** unanswered, and
  that is a claim rather than an oversight: the refinement pass resolves note properties only and §7
  states no integrity condition, so a resolution that gave up cannot hide a violation of anything in
  the table. It makes the documentation counts a floor, which is a different thing and one `inspect`
  already says. There is a test for the negative.
- A declared refinement this build reads past. See §3.

A violation **outranks** a caveat: a counter-example that was found is found whether or not the
search was exhaustive. The caveats stay on the outcome, because in that case they mean "and there
may be more", which the report still prints.

### 3. `rdfs:subPropertyOf` and `rdfs:subClassOf` are scanned — to say we did not use them

The model now collects every RDFS declaration as it reads a graph and, at build time, walks each
declared term upward to the SKOS and SKOS-XL terms it reaches. A term a condition is checked over
makes that condition unchecked, and the report names the declaration and prints the chain.

Four decisions inside that, each taken deliberately:

- **It walks rather than matching objects.** `ex:seeAlso → ex:linkedTo → skos:related` is legal and a
  one-step check would miss it, which is a false "held" produced by a shortcut. Breadth-first, so
  the chain shown is the shortest one that reaches the conclusion; the visited set is the cycle
  guard.
- **It stops at the first SKOS term.** What lies above `skos:related` is the specification's business
  and S21 already applies it.
- **A declaration whose subject is itself a SKOS or SKOS-XL term is ignored.** A vocabulary that
  imports the SKOS ontology carries S22 and S42 as ordinary statements; reading its copy as an
  unread refinement would make every importing vocabulary's entire roll-call unchecked — a report so
  cautious it says nothing. Those axioms are applied from the specification, where the citation
  belongs.
- **The budget is shared across the whole scan**, not per term, and the ceiling on distinct terms
  refuses rather than grows. `adr/0027`'s lesson, and iteration 33's: a per-walk budget times one
  walk per term is not a bound. The test uses five small clusters and a budget for two, because one
  long chain cannot tell the two readings apart. An exhausted scan makes **every** condition
  unchecked — a declaration never read could have reached any term at all.

**Nothing is entailed from any of this.** Performing the entailment is a decision about closure with
the same shape as the one B.4.4.1 blocks for `skosxl:labelRelation`, and it belongs to an item of
its own. What this ADR decides is only that the gap is *reported* rather than silently folded into a
pass.

### 4. The conservative direction is named, and it is one direction

The term list for a class-based condition is the whole class-bearing set — every property from which
S4–S8, S19–S21, S31, S33 and S39–S41 can entail a class membership — rather than three narrower
lists. So one declaration of `ex:seeAlso rdfs:subPropertyOf skos:related` leaves five conditions
unchecked: S9, S27, S37, S18 and S48.

That is right rather than lazy, and it is the reason: SKOS entails class membership *from its own
properties*, so an unread link can produce a class several steps from the property that was written.
A caveat naming one condition too many costs a reader a sentence. One naming a condition too few is
the false negative this whole module exists to prevent. The error is deliberately one-directional.

### 5. Production caller

`openbiz integrity <graph>` — the roll-call, with a headline verdict, the two groups a line each,
the specification's own words and the counter-examples for anything not simply held, and a single
section naming what the build read past.

`openbiz inspect`'s closing sentence now names it. That sentence is a summary and it is read as more
than it says; pointing at the command that takes it apart is the cheapest correction available and
it is a real behaviour change rather than a doc edit.

## Consequences

- The specification-conformance claim is now falsifiable. "Which integrity conditions do you check?"
  has an answer in code with a test behind it, and "which did you check on *my* vocabulary" has one
  per vocabulary.
- A vocabulary using §7.1's extension point over a SKOS property reads as substantially unchecked,
  because it is. Enterprise thesauri do this routinely, so the commonest real vocabulary will show
  five unchecked rows until sub-property entailment lands. That is uncomfortable and it is true; the
  alternative is the report the incumbents give.
- The cost per statement is one comparison against two predicate IRIs and, for a declaration, an
  entry in a map bounded by the vocabulary's schema rather than its data — the same argument
  `adr/0028` makes for the refinement pass.
- **`rdfs:subClassOf` is now read for the first time**, and only to report that it is not used. A
  build that later entails from it has somewhere to land and a set of tests already saying what it
  must not report once it does.

## What was measured, and what was not

Three mutants run, all caught: collapsing `Unchecked` into `Held` fails five tests across two
crates; dropping the `rdfs:subClassOf` half of the scan fails two; stopping the walk after one step
fails two. A fourth — removing S46's attribution of its bound findings — **appeared to survive**,
and did not: the edit had silently failed to apply because the string had been reformatted since it
was copied. Re-applied with an assertion that the replacement matched, it fails its test. The lesson
is the one iteration 33 recorded about `git checkout`: a mutation you did not verify was applied is
not a mutation, and "the suite stayed green" is then a statement about nothing.

**Not measured: the scan's cost on a real vocabulary.** No fixture here declares more than a handful
of refinements, and `scale.rs` generates none at all — the third dimension of the model the
generator does not produce, after labels and notes (iteration 31) and mapping links (32, 33). The
bound is a backstop against a pathological schema, not a number anybody has taken. In
`docs/UNTESTED.md`.
