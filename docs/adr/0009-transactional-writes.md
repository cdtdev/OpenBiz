# ADR 0009 — Transactional writes: a closure, a write lock, and why the backend's transaction is not enough

**Status:** accepted (2026-08-18) · **Phase:** 1

## Context

Phase 1's fifth item is "transactional write API with rollback; concurrent-reader safety under
test". Before this iteration the store's only write path was a private `insert_into` calling the
backend's `Store::extend`, which is atomic for one batch of quads and nothing more. There was no
way for a caller to make two writes land together, and no way to abandon a change part-way without
leaving the earlier part behind.

That gap was not theoretical. `Store::create_vocabulary_graph` read "is this IRI registered?" and
then wrote, as two separate operations against the backend. A test written for this iteration —
eight threads creating the same IRI — showed **all eight succeeding**. The damage is not a
duplicate row. A graph registered twice is a `StoreError::Corrupt`, and `Store::graphs` refuses the
*whole* registry when it finds one, so one user's mistimed second click takes the entire vocabulary
list down for everyone. `CLAUDE.md` §1.7 says the store must never make "create another one" the
path of least resistance; it was making it the path of least resistance *and* corrupting itself
doing so.

Phase 1's preamble says this is the substrate and every later phase inherits its mistakes. Import
(Phase 11), materialisation (Phase 8), and agent proposals (Phase 10) are all multi-write
operations that can fail half-way, and all three are downstream of this decision.

## What was measured

Oxigraph 0.5.9's `Store::start_transaction` returns a handle over a **RocksDB snapshot plus an
in-memory write-batch-with-index**. Reading `rocksdb_wrapper.rs` shows `commit()` is an
unconditional `rocksdb_write_writebatch_wi` of that batch: there is no conflict detection, no
validation that the snapshot is still current, and no serialisation between transactions.

So the backend's transaction gives us:

- **atomicity** — the batch applies whole or not at all; and
- **repeatable-read isolation** — a reader outside sees nothing until commit, and a reader inside
  sees the opening snapshot plus its own pending writes.

It does **not** give serialisability. Two transactions that both read "this IRI is free" both
commit, and the second silently overwrites the first's decision. That is a lost update, and in a
governance product a lost update is an approval that vanishes.

This was confirmed empirically rather than only by reading the source: with the write lock removed
but the backend transaction in place, the eight-racer test still fails. The lock is load-bearing,
not belt-and-braces.

## Decision

**1. The public API is a closure, not a `begin`/`commit` pair.**
`Store::transaction(|txn| ...)` commits when the closure returns `Ok` and discards everything when
it returns `Err` or panics. A handle-based API makes *silently never committed* the outcome a
forgetful caller gets; a closure makes *rolled back* the outcome a failing caller gets, which is
the safe one. It also keeps `oxigraph::store::Transaction` out of our public API, which `CLAUDE.md`
§3 requires so the engine stays swappable.

**2. Writers are serialised by a mutex we own; readers are not.**
This is what converts the backend's atomicity into the serialisability a read-modify-write needs.
It is cheap here in a way it would not be for a shared server: `CLAUDE.md` §1.2's single-binary
rule means exactly one process owns the store, and the backend's exclusive file lock enforces it,
so there is no distributed case to handle. Readers never take the lock, so concurrent reads stay
concurrent — asserted by a test in which four readers complete while a writer is parked mid
transaction, which would deadlock rather than fail if reads ever started taking the lock.

**3. The existence check moves inside the transaction.**
`create_vocabulary_graph` is now `transaction(|txn| txn.create_vocabulary_graph(g))`, and the check
and the write are one atomic step. This is the actual fix for the race above.

**4. Mutex poisoning is recovered from, not propagated.**
The mutex guards no data — it is a serialisation token — and a panic inside a transaction leaves
the store untouched, because unwinding drops the transaction and discards its writes. Propagating
the poison would turn one rolled-back edit into a store that has silently gone read-only for the
rest of the process's life. That is a worse failure than the one poisoning exists to prevent.

**5. A nested transaction is refused, not deadlocked on.**
The write lock is not reentrant, so a transaction opened inside another on the same store would
block forever against itself: no error, no log line, a request that never returns. `StoreError::
NestedTransaction` turns that into something a caller can read. The reentrancy mark is keyed by
store address rather than being a per-thread flag, so a process holding two stores over two data
directories is not falsely refused.

**6. Opening the store is one transaction, and it is the production caller.**
The format stamp and the system graph's registry entry now commit together. They were two
independent writes, and a kill in the gap left a store that was stamped but had no system graph in
its registry — which this build reports as inconsistent. A first start is the likeliest moment for
a container to be killed, so the gap was not hypothetical. Every deployment executes this path on
every start, which is what satisfies `CLAUDE.md` §4.1.

**7. The write choke point keeps its runtime refusal.**
Moving the choke point inside `Transaction` made it tempting to demote `is_directly_writable` to a
`debug_assert`, since every current caller checks first. It stays a runtime refusal. A caller that
has already checked makes it redundant, and that is the point: the rule must hold for the caller
who has not, including one added in a later phase by someone reading only the method's signature.

## Consequences

- A transaction holds its whole change set in memory (upstream says so explicitly), and it is the
  one thing other writers wait behind. Bulk import must not be one transaction per file; Phase 11
  needs a batching decision, and Phase 13's benchmark harness should measure the lock's contention
  before we assume it is free.
- Serialised writers mean write throughput is single-threaded by construction. That is a deliberate
  trade of throughput for correctness at a phase where correctness compounds and throughput does
  not. It is recorded in `UNTESTED.md` as unmeasured, because it is.
- The public write vocabulary is still only "register a graph". Raw triple writing stays private
  because the triple type is the backend's, and §3 keeps that out of our API. It becomes public
  when Phase 1's parse/serialise item gives us a term model of our own.
- Rollback is proven against a returned `Err` and against a panic. It is **not** proven against
  process death mid-transaction — see `UNTESTED.md`.

## Alternatives rejected

- **Rely on the backend's transaction alone.** Measured; it does not serialise. Rejected on
  evidence, not on preference.
- **Optimistic concurrency with a retry loop.** The backend exposes no conflict signal to retry on,
  so this would mean building version counters in the system graph — real complexity to buy back
  parallelism we have no evidence we need in a single-process store.
- **A reentrant lock.** It would make nesting work rather than refusing it, but nesting inside one
  logical write is a design mistake we would then be silently supporting; and reentrancy would let
  an inner transaction commit against an outer one's snapshot, which is a subtler bug than the
  deadlock it removes.
