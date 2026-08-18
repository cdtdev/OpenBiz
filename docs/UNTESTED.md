# Untested & unproven

Things built but not proven, or proven only narrowly. **Write here the moment you notice, not at
the end of the iteration** — the gap you defer recording is the one you forget.

This file existing is not a failure. A loop that never records a gap is not a loop with no gaps; it
is a loop that has stopped looking. What *is* a failure is a gap that never closes.

What belongs here:
- **Built but no production caller.** Code that exists and is unit-tested but nothing invokes.
  This is the most important category and the easiest to rationalise away — record it every time.
- Happy path tested, failure paths not.
- A standard partially implemented, where we must not claim full support.
- Anything needing hardware, an account, or infrastructure this machine lacks — mark these
  **environment-limited** so the monitor can tell a permanent floor from real rot.
- Behaviour verified by inspection rather than by a test.

Strike an entry through (`~~like this~~`) when it closes, with a one-line note on what closed it.
Do not delete it — the record of what took how long to close is the signal.

## Entry format

```
### <what is unproven>
- **Kind:** no-production-caller | partial-coverage | partial-standard | environment-limited | inspected-only
- **What is proven:** …
- **What is not:** …
- **What would close it:** …
- **Opened:** iteration N
```

---

### ~~The UI is built but nothing serves it~~ — CLOSED, iteration 1
- **Closed by:** `rust-embed` embedding `ui/dist`, a router fallback serving it, 13 tests in
  `crates/openbiz-server/src/ui.rs`, an end-to-end test over a real socket in
  `tests/serves_embedded_ui.rs`, and a `Single binary` CI job that deletes `ui/dist` before asking
  the release binary to serve the interface. Verified by hand: the release binary served the real
  318-byte Vite `index.html` and the 194 087-byte bundle with `ui/dist` moved off disk. See
  `adr/0004`.
- **Kind:** no-production-caller
- **What was proven:** `ui/` typechecks under TS strict and builds to `ui/dist` (28 modules, ~194 kB
  raw / ~61 kB gzipped). The Rust server compiles, serves `/healthz`, and returns 404 elsewhere.
- **What was not:** the two halves were not connected. No `rust-embed`, no static-file route, no
  test that a built asset was reachable from the binary.
- **Opened:** Phase 0 hand-build (pre-iteration-1) · **Closed:** iteration 1

### The UI has no test suite at all
- **Kind:** no-production-caller
- **What is proven:** `ui/` typechecks under `tsc --noEmit` and builds. The `App` component's
  `fetch("/healthz")` is now reachable from the binary — `tests/serves_embedded_ui.rs` proves the
  same origin answers `/healthz` with `{"status":"ok"}`, so the request the component makes will
  succeed.
- **What is not:** **no test runner is installed** (`ui/package.json` has no `test` script), so no
  assertion exists that `App` renders the health report, renders the `role="alert"` error branch on
  a failed fetch, or aborts cleanly on unmount. The three `Probe` states are written and never
  exercised. The iteration driver's `npm test` step is currently a no-op that silently passes.
- **What would close it:** Vitest plus Testing Library, and a test per `Probe` state. This wants
  doing before Phase 3 adds real components, not after.
- **Opened:** iteration 1

### Nothing renders the UI in a browser
- **Kind:** environment-limited
- **What is proven:** the served bytes are correct — status, content type, `ETag`/304, cache
  headers, and the exact Vite `index.html` and bundle, asserted over a real socket.
- **What is not:** no browser has ever executed the bundle. React could throw on mount and every
  test here would still pass, because they assert on transport, not on rendering. Content-Security-
  Policy, `crossorigin` on the module script, and MIME strictness are unexercised for the same
  reason.
- **What would close it:** a headless-browser smoke test (Playwright) that loads `/` from the real
  binary and asserts the health line appears. This machine can run one; it is a deliberate scope
  call not to add a browser toolchain in a Phase 0 iteration.
- **Opened:** iteration 1

### Asset serving is unproven above trivial size and count
- **Kind:** partial-coverage
- **What is proven:** one 194 kB bundle and one 318-byte shell, served whole, in a two-file
  `ui/dist`.
- **What is not:** no compression (`Content-Encoding`) is negotiated or emitted, so the 194 kB
  bundle goes over the wire uncompressed — an incumbent-grade UI will be several times that.
  Range requests are not handled. `Assets::get` is a lookup over a generated match arm per file; its
  behaviour at the hundreds-of-files scale a real design system produces is unmeasured, as is the
  effect on binary size and compile time.
- **What would close it:** revisit when Phase 3's design system makes `ui/dist` realistic —
  measure binary size, cold start, and first-paint transfer size, and decide on compression then.
- **Opened:** iteration 1

### ~~Config is env-only; the file path is unimplemented~~ — HALF CLOSED, iteration 2
- **Closed by:** `crates/openbiz-server/src/config.rs` — defaults → TOML file → environment, with
  documented precedence, `deny_unknown_fields`, blank-value rejection, and a `Source` on every
  setting. 16 tests; `Config::load` is called by `main.rs`, which logs each setting's provenance
  and names it in the bind-failure message. Four failure paths verified by hand against the real
  binary. See `adr/0005` and `docs/CONFIGURATION.md`.
- **Still open — see the entry below:** the `data_dir` half. Nothing creates or opens that
  directory yet.
- **Kind:** partial-coverage
- **Opened:** Phase 0 hand-build (pre-iteration-1) · **Config-file half closed:** iteration 2

### ~~`data_dir` is configured, logged, and used by nothing~~ — CLOSED, iteration 3
- **Closed by:** `Store::open(config.data_dir.value())` in `main.rs`, which runs *before* the
  listener binds. A `data_dir` that is a file, is unwritable, or is already locked by another
  OpenBiz now fails startup with a message naming both the path and the configuration layer that
  chose it. Covered by `a_data_directory_that_is_a_file_is_reported_as_such`,
  `an_unwritable_data_directory_is_reported_before_the_backend_sees_it`, and — across real
  processes — `a_second_instance_refuses_to_share_the_data_directory`. See `adr/0006`.
- **Kind:** no-production-caller
- **What is proven:** `data_dir` is read from the defaults, a file, or `OPENBIZ_DATA_DIR`, is
  rejected when blank, carries its provenance, and is logged at startup.
- **What is not:** **no code creates, opens, or writes that directory.** A user who sets it to an
  unwritable path, a path that does not exist, or a file rather than a directory gets a clean
  startup and no warning — the setting is inert until Phase 1 wires the store. This is the honest
  reading of `CLAUDE.md` §4 clause 1: a value with no consumer is not a feature.
- **What would close it:** the first Phase 1 item (`openbiz-store` Oxigraph lifecycle — open, close,
  durable path, graceful shutdown), which is the next item in the plan.
- **Opened:** iteration 2 (carved out of the entry above)

### Configuration is validated for shape, not for meaning
- **Kind:** partial-coverage
- **What is proven:** unknown keys, malformed TOML, wrongly-typed values, blank values, and a
  missing explicitly-named file all fail with a message naming the source. Precedence across all
  three layers is tested per setting, including that one key in a file does not change the
  provenance of another.
- **What is not:** `bind` is only checked for being non-blank. `OPENBIZ_BIND=not-an-address` is
  accepted by the loader and fails later at `TcpListener::bind` — a clear error, but later than it
  needs to be, and it is the only setting where the bind attempt happens to be a validator. No
  setting has a range, enum, or path check, and there is no mechanism for one. A non-UTF-8
  `OPENBIZ_CONFIG` path is treated as unset by `std::env::var().ok()` rather than reported, which is
  a silent ignore of exactly the kind the rest of this module refuses.
- **What would close it:** a per-setting validation hook run during `Config::resolve`, so the error
  carries the `Source` like the blank-value one does. Worth doing when the settings count justifies
  it — with two settings a hook is more machinery than the problem.
- **Opened:** iteration 2

### ~~Configuration precedence is untested against the real process environment~~ — CLOSED, iteration 3
- **Closed by:** `crates/openbiz-server/tests/graceful_shutdown.rs`'s
  `the_process_environment_reaches_the_configuration_with_its_provenance`, which spawns the real
  binary with a controlled environment and asserts on its startup log — that `data_dir` and `bind`
  both report `$OPENBIZ_*` as their source, and that requesting port 0 logs the port actually
  allocated rather than the request. `Config::load` now has a regression test; a typo in a variable
  name inside it fails CI. This was also the promoted plan item of the same name.
- **Kind:** partial-coverage
- **What is proven:** `Config::resolve` is tested exhaustively with an injected environment lookup,
  and the four headline failure paths were run by hand against the real binary with real env vars
  and real files.
- **What is not:** `Config::load` — the ten-line function that supplies `std::env::var` and the
  default path — has no automated test. It cannot easily have one: `std::env::set_var` mutates
  state shared by every thread in the test binary, and Rust 2024 marks it `unsafe` for that reason.
  So the wiring between the tested core and the real environment is inspected-only, and a typo in a
  variable name inside `load` would pass CI.
- **What would close it:** an integration test that spawns the binary as a subprocess with a
  controlled environment and asserts on its startup log — the same shape as
  `tests/serves_embedded_ui.rs`. Cheap; deferred only to keep this iteration to one item.
- **Opened:** iteration 2

### The named-graph model has no production caller
- **Kind:** no-production-caller
- **What is proven:** `GraphId`, `GraphKind`, `SYSTEM_GRAPH_IRI`, and `is_directly_writable()`
  compile and are unit-tested — a vocabulary graph is directly writable, an inferred one is not.
- **What is not:** **nothing constructs a `GraphId` outside tests, and no code writes to a named
  graph.** `is_directly_writable()` is a rule with no enforcement point: `StoreError::NotWritable`
  exists as a variant that is never returned. The only quad the store has ever held is its own
  format stamp, written by `stamp_or_check_format_version` against a hardcoded IRI. This is the
  honest reading of `CLAUDE.md` §4.1 — the model is designed, not delivered.
- **What would close it:** the next plan item, "Named-graph model: one graph per vocabulary, plus a
  system graph", which must route writes through `is_directly_writable()` so the check has a caller.
- **Opened:** iteration 3

### Durability is proven for one quad, not for a vocabulary
- **Kind:** partial-coverage
- **What is proven:** what one open commits, the next open reads back — `the_format_stamp_survives_
  close_and_reopen` closes the store and reopens it, and the cross-process test restarts a real
  binary against the same directory. The stamp is flushed immediately on first open.
- **What is not:** the store has never held more than a single quad. Nothing has written a
  vocabulary, so nothing has measured write throughput, flush cost at size, or how long `close()`
  takes when there is real data to flush — which is the number that decides whether a `docker stop`
  grace period of 10 s is enough or whether we are hard-killed anyway. The two upstream Oxigraph
  risks (unoptimised query evaluation, literal precision) are **untouched** by this iteration and
  keep their Phase 1 spike items; `adr/0006` must not be read as clearing them.
- **What would close it:** the Phase 1 benchmark spike, which should measure `close()` at 10k /
  100k / 1M concepts and not only query evaluation.
- **Opened:** iteration 3

### Graceful shutdown is proven to exit cleanly, not to drain
- **Kind:** partial-coverage
- **What is proven:** `SIGTERM` to a real process exits zero, logs which signal stopped it, logs
  `store closed cleanly`, and releases the lock so the next process can open the same directory.
  `shutdown_signal()` is proven not to resolve on its own — which matters, because `axum::serve`
  returns `Ok(())` on graceful shutdown, so a signal future that resolved immediately would produce
  a binary that exits zero while serving nothing.
- **What is not:** **no test has an in-flight request when the signal arrives.** The ordering the
  module documents — stop accepting, drain, *then* flush — is asserted only in its doc comment. A
  regression that closed the store before the last response was written would pass every test here.
  `SIGKILL` recovery was checked **by hand only**: a hard kill exits 137 and writes no
  `store closed cleanly` line (so the assertion discriminates rather than always passing), and the
  next start reopened the store and read its stamp back. That was a near-empty store with no write
  in flight — the case that actually matters, a hard kill *during* a write, is unmeasured, and none
  of it is a regression test.
- **What would close it:** a test that opens a slow request, sends `SIGTERM` mid-flight, and asserts
  the response completes before the process exits; and a `SIGKILL` test asserting the store reopens.
- **Opened:** iteration 3

### The lock classification depends on a RocksDB message string
- **Kind:** partial-coverage
- **What is proven:** both wordings RocksDB actually emits are pinned by tests — the same-process
  one in `openbiz-store`, the cross-process one in `tests/graceful_shutdown.rs`. Discovering they
  differ cost this iteration a red test, and a unit test alone would have shipped a classifier that
  never fired in production.
- **What is not:** `classify_open` matches on the substring `LOCK:`, and RocksDB's wording is not an
  API. A version bump could change it. The mitigation is that the tests go red rather than the
  classification degrading silently — but that is a promise about *noticing*, not about working, and
  a user hitting the fallback branch gets a true but much less useful error.
- **What would close it:** nothing available today; the backend exposes no typed lock error. Revisit
  if Oxigraph gains one. Recorded so a future upgrade knows to look here first.
- **Opened:** iteration 3

### Shutdown is Unix-only in test, and by implication in practice
- **Kind:** environment-limited
- **What is proven:** on Unix, `SIGINT` and `SIGTERM` both stop the server gracefully.
- **What is not:** `tests/graceful_shutdown.rs` is `#![cfg(unix)]` — `SIGTERM` does not exist
  elsewhere, and on Windows the `Ctrl-C` branch is the entire contract, unexercised by any test.
  Nothing has ever run this binary on Windows. We do not currently claim Windows support; this entry
  exists so that claim is not made accidentally.
- **What would close it:** a Windows CI runner exercising `Ctrl-C` shutdown, if and when Windows
  becomes a supported target.
- **Opened:** iteration 3

### The store's build toolchain is unavailable on the loop machine without a workaround
- **Kind:** environment-limited
- **What is proven:** the workspace builds and its tests pass once `libclang` is present. CI
  installs `clang` and `libclang-dev` explicitly rather than relying on the runner image.
- **What is not:** this machine has no `clang`, no `libclang`, and no passwordless `sudo`, so
  `cargo test --workspace` fails at `bindgen` on a clean checkout. The loop works around it by
  extracting `libclang1-20` from a downloaded `.deb` into `~/.local/libclang` and exporting
  `LIBCLANG_PATH`. **That workaround is not in the repo and does not survive a machine reset** — a
  future iteration that starts with an unexplained `Unable to find libclang` panic should read this
  entry rather than conclude the store is broken.
- **What would close it:** a human running `sudo apt install clang libclang-dev` on the loop
  machine. Out of loop scope (`CLAUDE.md` §8 — needs root).
- **Opened:** iteration 3
