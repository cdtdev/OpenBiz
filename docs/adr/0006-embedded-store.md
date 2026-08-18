# ADR 0006 — Oxigraph as the embedded store, and what "open" has to guarantee

**Status:** accepted (2026-08-18) · **Phase:** 1

## Context

`CLAUDE.md` §1.2 makes the single binary a non-negotiable: the server, the UI assets, and the store
ship as one executable plus a data directory. Adding a required external service is a charter
violation. Phase 1's first item — "embedded Oxigraph lifecycle: open, close, durable path, graceful
shutdown" — is where that stops being an architecture diagram and becomes a process that either
does or does not hold a customer's data safely.

This ADR records adopting `oxigraph` as **load-bearing**, which `CLAUDE.md` §3 requires a spike and
an ADR for, and records the lifecycle guarantees the wrapper has to make.

### Parity is failure — what the incumbents do badly here

The standing product-owner direction says to ask what the incumbents do *badly*, not whether they
have the feature. Every incumbent has a triplestore. What they do badly is that the store is in a
**separate lifecycle from the application**, and four failure modes follow from that one fact. They
are not exotic; they are what people report while standing these products up.

1. **The app starts against a store that is not ready.** PoolParty, TopBraid EDG, metaphactory, and
   VocBench all connect to a triplestore over HTTP at or after startup. The app comes up, the store
   is not there yet, and the operator gets a running service that fails every request. "Up but
   useless" is unresolvable from the outside — it looks identical to a permissions problem.
2. **Two instances share one data directory.** A second container scheduled before the first is
   fully drained, or an operator who forgot one is running. In a split deployment nothing detects
   it; in an embedded one it must.
3. **An unclean stop.** The application's shutdown and the store's flush are separate events with no
   ordering between them, so a container stop can kill the app mid-write. The recovery procedure
   then lands on the operator.
4. **A silent downgrade.** An older build opens a newer store and misreads it rather than refusing.

Embedding the store is what lets us answer all four, because there is exactly one lifecycle. That is
the material improvement — not "we also have a triplestore".

## Decision

**1. Adopt `oxigraph` 0.5 as the store, `default-features = false`, features `["rocksdb"]`.**

Turning default features off is deliberate. It drops the `http-client` family from the tree, so the
product carries **no code path that can open an outbound connection** — §1.1's air-gapped
requirement enforced by the dependency graph rather than by intention. SPARQL federated query
(`SERVICE`) is in the standards surface (§2) and is therefore a *known deferred* capability, not an
oversight: when it lands it must arrive with an explicit, per-deployment egress control of the kind
`adr/0002` requires of LLM providers, not as a feature flag flipped on by default.

**2. No `oxigraph::` type crosses this crate's public API.** §3 requires every third-party engine to
sit behind a trait we own. `StoreError` carries `std::io::Error` and strings; `Store`, `GraphId`, and
`GraphKind` are ours. Today the wrapper is a struct rather than a trait, because a trait with one
implementation and no second candidate is speculative generality — but the *boundary* is real and
tested, so introducing a trait later changes this file and nothing above it. The upstream risks in
`docs/COMPETITIVE.md` (unoptimised query evaluation; literal precision limits) are precisely why
this boundary is worth paying for before we know whether we will need to cross it.

**3. Open before bind; close after drain.** `main.rs` opens the store *before* `TcpListener::bind`,
so a store that will not open is a process that never starts rather than one that accepts requests
and fails each of them. Shutdown runs the other way: `SIGTERM`/`SIGINT` → stop accepting → drain
in-flight requests → flush and close the store → log `store closed cleanly`. `SIGTERM` is the one
that matters; `docker stop`, a Kubernetes eviction, and `systemctl stop` all send it, and a service
that handles only `Ctrl-C` is hard-killed on every routine restart.

**4. `close()` consumes the store and reports whether the flush worked.** Dropping releases the lock
silently, which cannot distinguish a clean stop from a kill in a log an operator reads after the
fact.

**5. Refuse rather than guess.** A store stamped with a *higher* `FORMAT_VERSION` is refused with an
instruction ("Upgrade, or restore a backup"). A store with two stamps, or a non-numeric one, is
refused as corrupt. Guessing which stamp is right is how a migration destroys data.

**6. The lock error is classified into our own words.** A second instance gets *"already in use by
another OpenBiz process"* plus the configuration layer that chose the path, not a RocksDB errno
about a `LOCK` file.

## What was measured

- **The lock message has two wordings, and only one of them is the one that matters.** Two opens
  from the *same* process report `lock hold by current process … LOCK: No locks available`; two
  *separate* processes — the real operator failure — report `While lock file: …/LOCK: Resource
  temporarily unavailable`. A unit test alone would have shipped a classifier that never fired in
  production. The common substring is the lock file, so that is what `classify_open` matches, and
  **both** wordings are pinned by tests: the same-process one in the store crate, the cross-process
  one in `crates/openbiz-server/tests/graceful_shutdown.rs`, which spawns two real binaries.
  This is deliberately fragile — RocksDB's wording is not an API. If it changes, those tests go red
  rather than the classification degrading silently, and the fallback still reports a true error.
- **`Store::len()` is a full iteration, not a counter read** — established by reading the RocksDB
  backend's implementation, not assumed. So there is no public `quad_count()`: a method that reads
  as O(1) and is O(n) would put a whole-store scan in the cold-start path the first time somebody
  logged it, breaking §1.5. It exists as `#[cfg(test)]` only. When something genuinely needs a
  count it should ask for a *scoped* one and be honest about the cost.
- **Durability is asserted by read-back, not by naming a directory.** The format stamp is flushed
  immediately on first open — it must survive a hard kill in the seconds after a first start, or the
  next open sees an unstamped store that already holds data — and the test closes and reopens to
  read what the previous open committed.
- **The licence policy needed no widening.** `CLAUDE.md` §5 anticipated Oxigraph's tree as the
  likely first case forcing an allow-list decision. It did not: `oxigraph` 0.5.9 and its whole
  transitive tree (`oxrdf`, `oxttl`, `oxrdfxml`, `oxsdatatypes`, `sparesults`, `spareval`,
  `spargebra`, `sparopt`, `oxrdfio`, and the `librocksdb-sys` chain) resolve within the existing
  allow list, and `cargo deny check licenses bans sources` passes **unchanged**. So this ADR
  records **no licence exception** — the §5 escalation path was not needed and must not be read as
  having been used.
- **Build cost is real and one-directional.** RocksDB is compiled from source, so the first build
  needs a C++20 compiler and `libclang`. This is a build-time requirement; the shipped artefact is
  still one self-contained executable. Recorded in `README.md`.

## Consequences

- `data_dir` has a consumer for the first time. A path that is a file, or is unwritable, is now
  reported at startup with our message and the configuration layer that chose it, closing the
  `UNTESTED.md` entry that called the setting inert.
- The workspace now depends on a C++ toolchain to build. CI already has one; a contributor on a
  bare machine does not, hence the README change.
- We have **not** validated the two upstream risks. Query evaluation performance and literal
  precision keep their spike items in Phase 1; nothing here measured either, and this ADR must not
  be read as clearing them.
- Named graphs are modelled (`GraphId`, `GraphKind`) but **nothing reads them yet** — that is the
  next plan item, and it is recorded in `UNTESTED.md` rather than implied to be done.

## Alternatives considered

- **An external triplestore (GraphDB, Fuseki, RDF4J).** Charter violation under §1.2, and the
  source of all four failure modes above. Not a candidate.
- **Sled or a hand-rolled quad store over an embedded KV.** Means writing a SPARQL engine, which is
  the one part of this problem that is genuinely hard and thoroughly standardised. No.
- **Oxigraph's in-memory backend.** Rejected: the product is a system of record. Durability is the
  requirement, not an optimisation.
- **Defining `trait RdfStore` now.** Deferred, not rejected — see decision 2. One implementation and
  no second candidate makes the trait's shape a guess. The boundary is enforced and tested today,
  which is what makes deferring it safe.
