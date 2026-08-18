# ADR 0015 — A backup is RDF, not a snapshot of our storage engine

**Status:** accepted (2026-08-19) · **Phase:** 1

## Context

Phase 1's eleventh item reads *"backup and restore to a single portable file; restore verified
against a live store"*. Until this iteration the only way to copy an OpenBiz deployment was to copy
the RocksDB directory, and the only way to read one was to run OpenBiz.

That is the shape of a product a customer cannot leave, which is the opposite of what `CLAUDE.md`
§1.1 promises when it says the customer hosts and the customer owns the data. It is also the
practical thing a data-governance team asks about in the first hour of an evaluation, because
somebody has to sign the disaster-recovery runbook.

## Decision 1 — the file is N-Quads, not a storage-engine checkpoint

Oxigraph exposes RocksDB's checkpoint API, and using it would have been three lines against
roughly four hundred. It was refused.

A checkpoint is a directory of SST files readable by one version of one storage engine. It makes
the durability of a governance system a function of our *implementation choice* — and `CLAUDE.md`
§3 is explicit that the engine is swappable, naming Oxigraph as a **known risk** to be replaced if
it stalls. A backup that a backend swap silently invalidates is not a backup; it is a copy. The
customer would discover this at exactly the moment they could least afford to.

So a backup is [N-Quads]: the statements themselves, in a W3C Recommendation, readable by any
conforming tool on any platform, forever. Four properties follow and each is worth having:

- **Line-based**, so it streams out of a scan with one quad in memory, `grep`s, and produces a
  reviewable `diff` between two days.
- **Graph-carrying**, which the three triple syntaxes are not — a whole-store backup written in
  Turtle would collapse every vocabulary into one indistinguishable pile.
- **Hand-authorable.** The end-to-end test's fixture is seven lines a person wrote from the
  specification, not a file `openbiz backup` produced. If the only thing that can make a backup is
  us, the portability claim is untested and the format is free to drift into something private.
- **Self-describing.** The store's format version is already a statement in the system graph, so
  the file carries its own version stamp and we did not have to invent a header.

The price is honest: it is larger than a checkpoint and slower to write, because it is text and it
is a full scan. Neither is measured beyond the small stores in the tests (`docs/UNTESTED.md`).

## Decision 2 — the backup contains OpenBiz's own metadata, and an export does not

`GET /api/export` deliberately hands back one vocabulary and nothing of ours (`adr/0010`). A backup
is the opposite job, so it takes the opposite rule: the system graph goes in the file, because the
registry — which graphs exist, and what kind each is — *is* what turns a pile of statements back
into a store.

This makes the two files distinguishable, which matters more than it sounds. The likeliest wrong
file to hand `openbiz restore` is an export, and restoring one would produce a store full of
content that nothing describes. It is refused by what it **lacks** — no format stamp — with a
message that says it is not a backup, rather than by a syntax error that would send the operator
hunting through a file that is perfectly valid RDF.

## Decision 3 — restore is all-or-nothing, and pays for it in memory

The whole file is one transaction. A restore that fails anywhere — a syntax error two thirds
through, a graph we will not accept, a registry that does not read back — leaves the target store
exactly as it was.

The alternative, committing in chunks, buys a bounded memory footprint with a half-restored store:
one that looks like a store, opens like a store, and is missing an unknown subset of a vocabulary.
That is the one state an operator cannot reason about, and they are restoring precisely because
they have already lost the original. The trade is refused in that direction.

The cost is real and unmeasured: the backend holds a transaction's write batch in memory until it
commits, so a very large restore needs room for it. `adr/0013` measured a million concepts at ~6 GB
on disk, which is the order of magnitude to worry about. Recorded in `docs/UNTESTED.md` with what
would close it.

## Decision 4 — restore verifies that it is about to produce a store we can open

Five refusals, all raised inside the transaction:

| Refused | Why it is not merely pedantic |
|---|---|
| A store that is not empty | Merging two stores interleaves two histories with no way to separate them afterwards. |
| No format stamp | It is an export, or somebody else's RDF — see Decision 2. |
| A stamp this build does not read | Newer says "upgrade"; older says "this needs a migration and there is not one yet" — two different actions, so two different errors. |
| A statement in no graph, in a blank-node graph, or in a graph IRI that breaks the `GraphId` invariants | A file that invents `urn:openbiz:graph:whatever` was written against a build that is not this one, and guessing what it meant is how a restore produces a store nobody can explain. |
| Content in a graph the file's own registry does not list, or a registry this build cannot read back | This is the load-bearing one. The registry is *data in the file*, so the restore re-reads it through the same code `Store::open` uses, **inside the transaction that just wrote it**, and asks: would this build open the store I am about to commit? A restore that commits an unopenable store is the worst outcome available here. |

The format stamp in the file is **checked and not written**: the target store stamped itself when
it was opened, and two stamps is a store `Store::open` reports as corrupt from then on. So a
restore reports one fewer statement than the backup contained, which the report's documentation
states rather than leaves to be noticed.

## Decision 5 — these are commands, not endpoints

Everything else in this build is reachable over HTTP. Backup and restore are not, for three
reasons in increasing order of how much they bind.

1. **A backup script needs a command anyway.** Backups run from cron, a systemd timer, a
   container's pre-stop hook. Those understand an exit status and a line on stdout; an HTTP
   endpoint needs a credential and a client before it is useful to any of them.
2. **A restore needs the store to itself.** It refuses a store that already holds anything, so it
   is by construction an operation on a stopped deployment — and the embedded store's exclusive
   lock enforces that rather than trusting an operator to remember.
3. **There is no authentication yet.** `POST /api/restore` would be an unauthenticated way to
   replace an entire customer's data; `GET /api/backup` an unauthenticated way to take a copy of
   all of it. That is the objection that already has SPARQL Update deferred, and it is not weaker
   here.

The limitation this creates is stated rather than hidden: **there is no online backup.** Taking one
means stopping the server. For a single-binary product that is a real operational cost, and the
authenticated online endpoint that would remove it is in `docs/PROPOSED.md` rather than
self-authorised.

Two smaller decisions inside the command surface, both about not destroying things:

- A backup **never overwrites**. The file most likely to be in the way is the last good backup, and
  replacing it with a partial one turns a bad day into an unrecoverable one.
- A mistyped command exits **2**, distinct from the 1 an operation failure returns, so a wrapper
  script can tell "retry this" from "you typed it wrong" — and so that `openbiz backupp today.nq`
  can never start a server while the operator believes a backup is being taken.

The argument parser is hand-written: four forms, no flags. `clap` would be a dependency and a
build-time cost for less code than the table it replaces, and `CLAUDE.md` §1.5 makes every
dependency something to justify. If the surface grows past one screen that judgement should be
revisited — it is a size judgement, not a principle.

## Consequences

- **The N-Quads *parser* now has a production caller**, which is the condition `adr/0010` set for
  it landing. That does **not** close Phase 1's parsing item: one syntax of six, reading a whole
  store rather than importing into a vocabulary, and an import still waits on Phase 2's candidate
  seam for the reasons `adr/0010` gave.
- **Restore is a second reader of the registry format.** The registry's shape is now a
  compatibility surface between OpenBiz and files on customers' disks, not just an internal
  detail. A change to it needs a format-version bump and the migration framework — which is the
  next item in the plan, and now has a concrete first customer.
- **The store's format version has become load-bearing in a second place.** It was a guard against
  an older build misreading a newer store; it is now also what tells a backup apart from an export.

[N-Quads]: https://www.w3.org/TR/n-quads/
