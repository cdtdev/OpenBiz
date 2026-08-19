# ADR 0048 — Discovery runs on every name a split creates, in one pass

**Status:** accepted (2026-08-20, NZST UTC+12) · **Phase:** 2

## Context

`adr/0046` put discovery on the creation path and named `openbiz mint` as that path. It is not the
only one. `openbiz split` names two or more new concepts in a single command, mints an IRI for each
under the vocabulary's own policy, and stages them — and until this decision it asked nothing about
any of those names beyond the vocabulary it was editing.

The vocabulary-local check it did have is `Part::already_called_that`: concepts in *this* graph
carrying that exact preferred label. That is the check `CLAUDE.md` §1.7 exists to say is not
enough. The concept a part duplicates is usually in the vocabulary the curator is not looking at,
and a split is precisely where a duplicate arrives under a new name — a term is divided because it
meant two things, and one of the two very often already exists somewhere else under that name.

So §1.7 held on one creation path and not the other, and the one it did not hold on creates
several concepts at a time.

## The problem the obvious implementation has

`Discovery::across` takes one query. Three parts is three passes, and a pass over the local source
reads **every vocabulary in the store and every change waiting for a decision**, one model at a
time, from disk. Three parts would have paid for the whole store three times, on the creation path,
in front of somebody who is waiting. `docs/UNTESTED.md` already records that `mint` reads the store
twice per run with no measurement behind it; multiplying that by the number of parts is not a cost
to add quietly.

The reading is the expensive half and it does not depend on the query.

## Decision

**1. `DiscoveryProvider::search_each(&[LabelQuery])` — several questions, one pass.** A default
method that loops over `search`, which is exactly right for a remote source with one request per
query, and which `LocalVocabularies` overrides: it lists the corpus once, and for each part builds
the model once and runs every query against it while it is in hand. Three part names now cost one
reading of the store rather than three, and the memory ceiling is unchanged — still one model at a
time, whatever the number of labels. `Discovery::across_each` returns one `Discovered` per query,
in the order given, and `across` is now that function with one query.

**2. A source whose answers cannot be lined up is unavailable, to every query.** An implementation
that returns a different number of answers than it was asked questions is treated as a source that
could not answer, rather than having its answers aligned by position and hope. A match shown under
the wrong part name is the one failure this whole crate exists to prevent. The same rule covers a
source that simply fails: it is recorded as unavailable against **every** label of the pass, so one
part cannot report a complete search while its neighbour reports a partial one, from one pass.

**3. The question is asked once per part and answered under that part's own name.** A report that
merged the answers would tell a curator "something here already exists" without saying which of
their three parts it was, which is an answer nobody can act on. The consultation record — which
sources answered, how far each looked, which were never asked — is merged and printed **once**, for
the command, with the counts summed across the labels. Three copies of the same paragraph is how a
report stops being read; and the report says out loud that the counts are totals across all the
part names, so "18 labels read" is not taken for the size of the store.

**4. The concept being split is not a concept to reuse.** A part named after one of the original's
own labels matches it, and the reuse ladder offered over that match reads as "use this concept as
it stands" — which is "do not split this", the opposite of the right advice. Such a match is still
shown, because a part taking a label the original carries is a real thing to notice, but it is
annotated as the concept being divided, it does not count toward "something was found", and the
ladder is not printed over it. **This was found by running the command against a store, not by
reasoning about it.**

**5. Nothing is refused.** As on the mint path, and for the same reason: two concepts can
legitimately share a label, and a tool that refuses on a lexical match is one people work around.
The split is still staged; it is a proposal somebody can reject, and the report says so where the
reader is looking rather than at the bottom.

**6. When discovery could not read everything, the vocabulary being edited is checked directly.**
The model of the graph being split is already in hand, so what it calls things is known whether or
not discovery could be asked. That narrower check is printed **only** when a source was
unavailable, where it is the last thing standing between a curator and a duplicate — and never
otherwise, because it says less than the pass above and repeating it teaches the reader that this
section says everything twice.

## What was rejected

**Refusing a split whose part duplicates something.** Same argument as `adr/0046` §5: homonyms are
legitimate, and a wall here is a wall around the operation that exists to *reduce* ambiguity.

**Dropping `already_called_that` now that discovery subsumes it.** Discovery's exact matches are a
superset — every label kind, any language, every vocabulary — so printing both routinely would say
the same thing twice. But it survives as the degraded path, because the case where discovery cannot
answer is exactly the case where the curator most needs whatever check is still possible.

**Keeping one `Discovered` for the whole command.** Cheaper to render and useless to act on: the
curator's next question after "something already exists" is always "which of my parts".

## Consequences

- `openbiz split` reads the store **twice more** than before: once for `scan_for` and the model as
  it already did, and once for the discovery pass. That is one more full read on a path where a
  person is waiting, and it is unmeasured — `docs/UNTESTED.md`.
- The justification `adr/0003` §3 requires is still not recorded anywhere first-class. The report
  says so in as many words rather than implying the ladder has teeth it does not have. That is the
  next plan item, and it is a store-format decision.
- Matching is lexical, as everywhere else in this build: case-insensitive and **not** insensitive
  to accents, spelling, or Unicode normalisation. On a creation path a miss is a duplicate rather
  than a retry.
