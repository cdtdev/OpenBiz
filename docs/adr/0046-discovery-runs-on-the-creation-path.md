# ADR 0046 — Discovery runs on the creation path, and a source that cannot answer is reported

**Status:** accepted (2026-08-19) · **Phase:** 2 · **Supersedes nothing** · Implements `adr/0003`
§§1, 2, 3 and 7 for the local store.

## Context

`CLAUDE.md` §1.7 and `adr/0003` name the failure this product exists to prevent: the enterprise
that owns nine overlapping taxonomies and cannot tell. The mechanism is always the same — creating
something new is one command, and finding the nine that already exist is a research project.

`openbiz mint` is this build's creation path. There is no "create concept" command; a concept is
created by staging a change through the candidate seam, and to write that change somebody has to
decide the new concept's IRI. `mint` is where that decision is made, so it is where discovery has
to run.

Before this ADR, `mint` ran **one exact-label lookup in the target vocabulary** and its own report
said so — "that is one exact lookup in one vocabulary and not a discovery pass". Honest, and not
enough: the concept a curator is about to duplicate is usually in the vocabulary they are *not*
looking at.

## Decision

**1. Discovery is on the creation path, not beside it.** `openbiz mint <graph> <label>` runs a
discovery pass before it answers, and prints what it found **above** the IRI. There is no flag to
turn it on. A discovery feature a user has to remember to invoke prevents nothing, because the
person who forgets is exactly the person creating the duplicate.

**2. The IRI is still offered.** Two concepts can legitimately share a label, and a tool that
refuses on a lexical match is a tool people work around. The report's job is to make the choice
informed, not to make it for the curator. What it does refuse — unchanged from before — is minting
a *disambiguating suffix* onto a slug that is already taken.

**3. Sources sit behind `DiscoveryProvider`, in a crate that cannot reach one.**
`openbiz-discovery` depends on `openbiz-skos` and `thiserror` and nothing else: no store, no HTTP
client, no engine. A source is reached either through a caller's own `DiscoveryProvider` or
through `LocalCorpus`, which the composition root implements over the real store. So the crate
that will one day hold catalog connectors *cannot today open a connection or a database by
accident*, and an air-gapped deployment (`CLAUDE.md` §1.1) is unaffected by anything added here
until somebody adds a dependency and writes the ADR that justifies it.

**4. A source that cannot answer is reported, never fatal** (`adr/0003` §7). A provider returns
`SourceAnswer` or `Unavailable`; `Discovery::across` records the unavailable ones with their
reasons and carries on. Creation is never blocked by a catalog that is down. The local provider
degrades as a *whole source* rather than skipping the part it could not read, because a partial
answer offered as a complete one is precisely the failure this is defending against.

**5. Nothing found is never printed as nothing exists.** Every pass reports what was consulted,
what each source actually looked at, how many labels it read, and — always, even when everything
went well — the sentence naming what was *not* asked: no peer, no data catalog, no public
registry, because this build has no connector for one. A bounded list says how many matches it
withheld.

**6. Matching is the forgiving default, ranked.** Any label kind (§5.1 defines `skos:hiddenLabel`
for exactly this), any language, anywhere inside the label. Exact matches are reported under a
`STOP` heading and counted **by concept, not by label**; partial ones are shown separately as
"may be the concept meant under another name". A match in the vocabulary being authored ranks
above the same match elsewhere: one is a duplicate about to be created, the other is something to
map to.

**7. The reuse ladder is printed only when something was found.** `adr/0003` §3's five rungs, in
the words of what this build can do about each. A ladder printed over an empty list teaches the
reader to skip the paragraph on the day it matters.

## What this does *not* do

- **No source but the local store.** Peers, SPARQL endpoints, public registries and enterprise
  catalogs are Phase 12. The report says they were not consulted rather than leaving it to be
  assumed.
- **No recorded justification for creating anyway.** `adr/0003` §3 requires one, and the only
  place this build has for it is the note on the change that creates the concept. Recording it as
  a first-class, auditable object needs the candidate seam over HTTP, which is blocked on
  authentication (`BLOCKED.md`). The report says this out loud rather than implying the
  justification was captured.
- **No structural or fuzzy matching.** Lexical only: case-insensitive, and *not* accent-,
  spelling- or normalisation-insensitive. `adr/0003` §6 wants more; the report states the limit in
  the same sentence it states the reach.
- **Nothing measured at scale.** Discovery reads every vocabulary in the store, one at a time, on
  a command a person is waiting for. `docs/UNTESTED.md` carries it.

## Consequences

- New crate `openbiz-discovery`, and a new `discovery` module in the server that is the store
  adapter and nothing else.
- `openbiz mint` now reads every vocabulary in the store **twice** on each run: once for the IRI
  collision scan, once for discovery. Both were already whole-store reads for the namespace scan;
  this doubles a cost nobody has measured. Recorded, not fixed — merging the two passes would tie
  discovery's corpus to the minter's scan, and the two ask different questions.
- The `mint` report is longer. That is the trade the item makes: the section a reader can skip is
  the one that stops them creating the tenth overlapping vocabulary.
