# ADR 0016 — Store-format migrations are a versioned chain, and the first one is a self-heal we deleted

**Status:** accepted (2026-08-18) · **Phase:** 1

## Context

Phase 1's last item reads *"store-format migration framework — versioned, forward-only, tested on a
populated store"*.

Before this iteration the store's version handling was half a mechanism. A store stamped **newer**
than the build was refused, correctly and with a message. A store stamped **older** was accepted
without comment: `stamp_or_check_format_version` returned `Ok((found, false))` and everything above
it carried on as if the store were current. With `FORMAT_VERSION = 1` no such store could exist, so
the branch was harmless — and it was also the exact line that would have lost data on the day the
version was first bumped, because the build would have read a version-1 store while believing
version-2 invariants.

The plan named the concrete first customer: `openbiz restore` reads a format stamp out of a file on
a customer's disk and, until now, **refused** an older one with "migrating an older backup is not
implemented yet". That refusal was honest. It was also a capability gap with a name, and it is the
one that bites hardest — the file an operator restores is by definition the one written by the
build they are leaving.

## The problem with building a migration engine before there is a migration

An engine with an empty registry is code with no production caller, which `CLAUDE.md` §4.1 says is
not done — it is an entry in `UNTESTED.md`. Manufacturing a format change to give the engine
something to do would be worse: a version bump that records no real difference teaches the next
person that versions are decorative.

So the question was whether a *genuine* first migration already existed, unnamed. It did.
`Store::open` contained this:

```rust
// The system graph is registered on **every** open, not only when the store is created, so a
// store written before the registry existed acquires one by being opened. That is additive, so
// it needs no format bump and no migration […]
let registered = txn.ensure_registered(&GraphId::system())?;
```

That is a migration wearing a self-heal's clothes. It ran unconditionally, forever, on the startup
path, for the benefit of stores that needed it once. It could not say which stores had needed it.
It left no record that anything had happened. And it set the precedent every future additive change
would follow, until the open path was a pile of idempotent fixups with no history between them.

## Decision 1 — a store's version records which invariants hold, and version 2 means one more does

`FORMAT_VERSION` is **2**, and the difference between 1 and 2 is not a change to the shape of the
data. Both versions serialise identically. The difference is what the build *guarantees*: at
version 2, the system graph is in the graph registry. At version 1 it might not have been.

That is what a format version is for. It is not a schema hash; it is a claim about what the reader
may assume. Making the claim explicit is what let the unconditional write become a one-off, and
what lets the check that replaced it be a **refusal** rather than a repair:

```rust
if !txn.contains_graph(SYSTEM_GRAPH_IRI)? {
    return Err(StoreError::Corrupt { … });
}
```

A store at version 2 that violates version 2's invariant is a store something outside our code has
written to. A governance product should say that, not mend it in silence and leave the operator
with no idea their store was ever wrong. The cost is real and stated: a case the old code papered
over now stops a deployment. That is the intended direction of the trade.

## Decision 2 — forward-only, one version at a time, no `revert`

A `Migration` declares the version it applies at and always ends one higher. Multi-version jumps
are not expressible: an upgrade across four versions is four reviewable, individually testable
steps, not one function that has to know every intermediate shape.

There is no downgrade path and there will not be one. The honest way back is the backup taken
before the upgrade, and a half-working downgrade is precisely what invites an operator to skip
taking it. `openbiz backup` is the answer, and it is a command that either works or fails loudly
rather than a promise.

**A gap in the chain refuses.** If a build ever ships without the step for a version it claims to
read, the store is refused with the *missing* version named — because that number identifies the
release the operator needs. Skipping ahead, running 2→3 against a version-1 store because 1→2 is
absent, would apply a transformation to a shape it was never written for. The chain is also checked
to be unbroken from 1 to `FORMAT_VERSION` by a test, so bumping the constant without adding the
migration fails the build rather than a customer's store.

## Decision 3 — one transaction for the whole chain

Every step, every migration record, and the new stamp commit together or not at all. A store
stamped 3 that only got as far as migration 2 opens, looks fine, and has an unknown subset of
itself in the old shape — the state nobody can reason about. `adr/0015` made the same argument
about half-restored stores; it applies with more force here, because there is no file to try again
from.

This inherits `adr/0015`'s unmeasured memory ceiling: a migration that rewrites a large fraction of
a store holds that rewrite in the backend's write batch. The first migration writes five quads, so
the ceiling is not reached today. It is recorded in `UNTESTED.md` rather than left to be discovered.

Proven by a **synthetic chain**: the engine takes the migration list and the target version as
parameters, so a test can run a two-step chain whose second step fails and assert that the first
step's write is gone and the stamp has not moved. The real chain has no failing migration and
should not acquire one for a test's benefit.

## Decision 4 — a migration explains itself, in the log *and* on disk

`CLAUDE.md` §3 requires an auto-applied change to answer *"why?"*. A store upgrade is the most
invisible auto-applied change in the product: it happens during startup, to a customer's data,
without anybody asking for it. So:

- **The caller gets a `MigrationReport`** — which steps ran, from and to which version, and each
  one's own sentence about what it did. `main.rs` logs it at `warn` on startup and `openbiz restore`
  appends it to the line a script reads, because "restored 12 000 statements" looks identical
  whether or not the file was migrated on the way in.
- **The store gets a record.** Five quads per migration in the system graph: what ran, from, to,
  why, and when as an `xsd:dateTime`. The log line scrolls away; the record is still there when an
  auditor asks in a year. It is ordinary RDF, so the answer comes back from a SPARQL query naming
  `FROM <urn:openbiz:graph:system>` through the endpoint that already exists, not from a
  proprietary log format. That query is the record's production reader and there is a test that
  runs it.

## Decision 5 — restore migrates the file, inside the restoring transaction

`Store::restore` no longer refuses an older backup. It reads the file's stamp, restores the
content, and runs the same chain over it in the same transaction that wrote it — so a backup that
cannot be migrated restores **nothing**, rather than restoring a store this build misreads.

Two details worth stating. The migration acts on the *file's* version, not the target store's: the
target stamped itself at the current version when it was opened, and the shape needing to be
brought forward is the one that just arrived from disk. And the registry read-back that `adr/0015`
introduced now runs *after* the migration, so the question it asks — "would this build open the
store I am about to commit?" — is asked about the store that will actually exist.

A backup from a build **newer** than this one is still refused. There is nothing to be done with a
shape we have never seen.

## What was measured

- A version-1 store, populated with a vocabulary and a concept, opens, migrates, keeps its content,
  reports the step by name, and is at version 2 afterwards. Opening it again reports no migration.
- A version-2 store missing the system-graph registration is refused as corrupt with a message
  naming the action, not repaired.
- A store at a version with no migration out of it is refused naming the missing version.
- A two-step synthetic chain applies in chain order rather than list order and writes exactly one
  stamp, at the end.
- A synthetic chain whose second step fails leaves no trace of the first and does not move the
  stamp.
- End to end through the real binary: a hand-written version-1 backup restores into a fresh data
  directory, the command says it migrated and why, the server serves the vocabulary *and* the
  system-graph registration the file did not carry, and a backup of the result contains the
  migration record, its timestamp, and a stamp of 2.

273 Rust tests and 29 UI tests green; `cargo fmt`, `cargo clippy -D warnings`, and `cargo deny`
clean.

## Consequences

- The first real format bump has a tested mechanism behind it instead of being the change that
  discovers it needs one.
- `Store::format_version()` is now always `FORMAT_VERSION` for a store that opened. Every other
  outcome is an error. Callers no longer have to remember to check.
- One dependency added: `oxsdatatypes` (MIT OR Apache-2.0), already in the tree beneath Oxigraph,
  for the `xsd:dateTime` on a migration record. Formatting a timestamp by hand would have meant
  hand-rolling civil-date arithmetic to produce a lexical form the store then has to agree with;
  using the library the store's own literals go through means it agrees by construction.
- The unconditional per-open registry write is gone. The saving is trivial; the point is that the
  next additive change has somewhere to go that is not the open path.
- **What this does not prove.** Nothing has ever migrated a *large* store, no migration has ever
  rewritten content rather than metadata, and the version-1 stores these tests migrate were made by
  degrading a version-2 store rather than by a version-1 build, which no longer exists. See
  `UNTESTED.md`.
