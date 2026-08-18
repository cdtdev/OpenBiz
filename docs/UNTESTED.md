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

### ~~The UI has no test suite at all~~ — CLOSED, iteration 4
- **What is now proven:** Vitest + Testing Library run 10 assertions over `App` in CI (`npm test` in
  the `UI` job). All three `Probe` states are exercised: the loading text and the single `/healthz`
  call on mount; the success line naming status and version; the `role="alert"` branch for an HTTP
  refusal, a transport rejection, and a non-`Error` rejection; the unmount abort; the `AbortError`
  swallow; and a StrictMode double-mount that must not paint a spurious alert. Each was shown to
  fail against a deliberately broken `App.tsx` before being trusted — see the plan item for the
  seven mutants.
- **Correction to the original entry:** it said the driver's `npm test` "silently passes". It did
  not — there was no `test` script, so it exited 1 with `Missing script: "test"`, and the `UI` CI
  job never called it. Zero UI assertions had ever run either way.
- **Opened:** iteration 1 · **Closed:** iteration 4

### The UI suite asserts on jsdom, and covers one component
- **Kind:** narrowly-proven
- **What is proven:** `App`'s render output and probe lifecycle, under jsdom, via accessible
  queries (`getByRole`) rather than test IDs — so the assertions are about what a user or a screen
  reader perceives, not about markup shape.
- **What is not:** three things. (1) **jsdom is not a browser** — no layout, no real paint, no CSS,
  no focus semantics beyond jsdom's approximation. The Phase 3 Playwright item is what closes that;
  see "Nothing renders the UI in a browser". (2) **`main.tsx` is untested** — the "root element
  missing" throw and the `createRoot` call have no assertion; only `App` is mounted, by the test's
  own `render`. (3) **`CLAUDE.md` §4.4's keyboard-navigability clause is still unenforced.** Nothing
  in `App` is interactive yet, so there is nothing to tab to and no test would be meaningful — but
  the moment Phase 3 adds a control, a keyboard test has to arrive with it, and no mechanism
  currently forces that.
- **Amended, iteration 6:** it now covers *two* components — `App` and `Vocabularies` — plus the
  `useProbe` hook they share, at 22 assertions. Every point above still stands unchanged, and (3)
  has now survived a second component without being tested: `Vocabularies` renders a heading, a
  list, and paragraphs, so there is still nothing to tab to and `CLAUDE.md` §4.4 is still satisfied
  **vacuously**. Two iterations of UI have now been added without the clause ever being exercised.
- **What would close it:** for (1) the Playwright item; for (2) a test that mounts `main.tsx`
  against a document with and without `#root`; for (3) a Phase 3 convention, ideally a lint or a
  shared test helper, that makes an interactive component without a keyboard test fail.
- **Opened:** iteration 4 · **Amended:** iteration 6

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

### ~~The named-graph model has no production caller~~ — CLOSED, iteration 5
- **Closed by:** `insert_into` in `crates/openbiz-store/src/lib.rs` — the single function through
  which every write to the store passes, including the format stamp. It requires a `&GraphId` and
  refuses a target that is not directly writable, so `is_directly_writable()` is a rule the store
  enforces rather than a comment a caller may forget, and `StoreError::NotWritable` is now returned
  by a public method (`create_vocabulary_graph`). `Store::open` registers the system graph in the
  graph registry on every open, and `main.rs` reads that registry **before it binds**, failing
  startup if it cannot be described. `GraphId`'s fields are private, so the pairing of IRI and kind
  is an invariant the type enforces: the mutation that assembled one by struct literal from a
  registry row would not compile. 36 store tests (was 12); twelve mutants killed. See `adr/0007`.
- **Kind:** no-production-caller
- **Opened:** iteration 3 · **Closed:** iteration 5

### `create_vocabulary_graph` has no production caller
- **Kind:** no-production-caller
- **What is proven:** it registers a graph, refuses an IRI that is already registered with a message
  pointing at the reuse ladder, refuses a graph that is not directly writable, leaves the registry
  unchanged when it refuses, and survives close and reopen. Nine tests, and the mutants that drop
  either guard are killed.
- **What is not:** **nothing calls it outside tests.** No HTTP route, no UI, no import path creates a
  vocabulary graph. This is deliberate rather than an oversight, and the distinction matters:
  `CLAUDE.md` §1.7 requires discovery to run *before* creation and requires a recorded justification
  when something new is created anyway, and `DiscoveryProvider` does not exist until Phase 2. Adding
  a create endpoint now would be a charter violation dressed up as progress, so the honest position
  is a recorded gap.
- **What would close it:** the Phase 2 authoring path with its local discovery hook. The read half —
  `GET /api/graphs` and the registry in the UI — is the next Phase 1 item and does not depend on it.
- **Opened:** iteration 5

### A corrupt registry is proven to stop the store, not proven to stop the server
- **Kind:** partial-coverage
- **What is proven:** `Store::graphs()` returns `StoreError::Corrupt` for an unrecognised kind
  token, for a registry entry that breaks the namespace rule, and for a graph registered twice with
  different kinds — each asserted against a store doctored through the backend, and each mutant
  killed. `main.rs` propagates that error with `?` before the listener binds.
- **What is not:** the *propagation* is inspected-only. `tests/graceful_shutdown.rs` proves the
  registry is read at startup (the mutants that stop reading it and that read-but-do-not-report it
  both turn it red), but no test starts the binary against a **doctored** store and asserts it
  refuses to serve. Building that fixture needs raw quad access from outside the crate, which today
  means either making the backend reachable or adding `oxigraph` as a dev-dependency of
  `openbiz-server` — and a test-only direct dependency on the engine cuts against `CLAUDE.md` §3,
  so it is not a cost worth paying for one assertion.
- **Also, since iteration 6:** `GET /api/graphs` has the same shape of gap. `From<StoreError> for
  Failure` is asserted directly — a constructed `StoreError::Corrupt` becomes a 500 whose body
  contains neither the customer's IRIs nor their store path, and both mutants (200 instead of 500,
  and echoing the store's own words) are killed. What is *not* proven is that a real store can
  actually drive the handler down that branch, for the same fixture reason. In practice it is close
  to unreachable: `main` refuses to start against a registry it cannot read, so the endpoint only
  sees one that went bad while the process was running.
- **What would close it:** a store-layer test helper that writes an arbitrary quad, gated behind a
  cargo feature the server's tests can enable. Now wanted by two callers rather than one, which
  changes the cost/benefit recorded above — it is worth doing the next time either gap is touched.
- **Opened:** iteration 5 · **Amended:** iteration 6

### Registry reads are unmeasured above a handful of graphs
- **Kind:** partial-coverage
- **What is proven:** `Store::graphs()` returns the right graphs in a stable order with up to four
  registered, and `contains_graph` answers without scanning the store's contents. Listing asks the
  registry rather than the backend precisely so it does not become a whole-store scan.
- **What is not:** the registry read is a pattern scan bounded by the number of *graphs*, which is
  fine at four and unmeasured at four thousand — and an enterprise with a vocabulary per business
  domain per jurisdiction gets there. It also runs on the startup path, so if it is slow it is slow
  in the cold-start budget `CLAUDE.md` §1.5 sets. Nothing has measured it, and `graphs.sort()` is an
  additional O(n log n) on top.
- **Widened, iteration 6:** `GET /api/graphs` reads the registry **on every request**, deliberately
  and with the reasoning recorded in `adr/0008` §6 — a cache would need invalidating by every future
  creation, import, and restore path, and a stale "your vocabulary does not exist" is worse than an
  unmeasured scan. But the consequence is that an unmeasured scan is now on a *hot* path rather than
  a once-per-process one, and it also serialises the whole registry into a JSON body with no paging.
  The 4 000-graph deployment gets a 4 000-element response on every page load.
- **What would close it:** the Phase 1 benchmark spike should register 10k graphs and time
  `graphs()`, startup, **and the endpoint**, alongside the query evaluation and `close()` numbers it
  already owes. If the number is bad, the answer is paging or a `?kind=` filter — both API changes,
  which is why the spike should land before Phase 3 builds an interface on top of this shape.
- **Opened:** iteration 5 · **Widened:** iteration 6

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
  extracting `libclang1-20` and `libclang-common-20-dev` from downloaded `.deb`s into
  `~/.local/libclang`. **That workaround is not in the repo and does not survive a machine reset** —
  a future iteration that starts with an unexplained `bindgen` panic should read this entry rather
  than conclude the store is broken.
- **The full incantation, because `LIBCLANG_PATH` alone is not enough** (iteration 4 lost time to
  this): with only `LIBCLANG_PATH` set, libclang loads but fails to locate its own builtin headers
  and `bindgen` dies on `rocksdb/c.h:65:10: fatal error: 'stdbool.h' file not found` — a *different*
  message from the `Unable to find libclang` this entry originally described, and easy to misread as
  a real build break. Both variables are needed:
  ```
  export LIBCLANG_PATH="$HOME/.local/libclang/usr/lib/llvm-20/lib"
  export BINDGEN_EXTRA_CLANG_ARGS="-resource-dir=$HOME/.local/libclang/usr/lib/llvm-20/lib/clang/20"
  ```
- **What would close it:** a human running `sudo apt install clang libclang-dev` on the loop
  machine. Out of loop scope (`CLAUDE.md` §8 — needs root).
- **Opened:** iteration 3 · **Amended:** iteration 4

### The UI dependency tree is outside the §5 licence gate
- **Kind:** unenforced-policy
- **What is proven:** `cargo deny check licenses` enforces `CLAUDE.md` §5 over the whole Rust tree
  in CI, and it is green. The four packages added this iteration were checked by hand before being
  committed: `vitest` (MIT), `jsdom` (MIT), `@testing-library/react` (MIT), `@testing-library/dom`
  (MIT). A sweep of all 153 installed npm packages found 134 MIT, 8 ISC, 5 Apache-2.0, 2 BSD-2,
  2 BSD-3, 1 MIT-0 — all permitted — and exactly one unlisted licence.
- **What is not:** **nothing enforces this.** §5 says "every new dependency gets a licence check",
  and for `ui/` that check is a human remembering to run one. The sweep above was ad hoc and is not
  in the repo, so the next npm install is unchecked by default. The one unlisted licence is
  `caniuse-lite` under **CC-BY-4.0** — pre-existing, pulled in transitively by Vite via
  `browserslist`, build-time only, and a *data* package rather than code, so none of it reaches the
  shipped binary. CC-BY-4.0 is attribution-only and not copyleft, so this is the "unlisted but
  permissive in substance" case of §5 rather than the forbidden one — but §5 requires an ADR for
  that judgement and there is no ADR, because there is no allow list for it to widen.
- **What would close it:** the `Audit the UI dependency tree for licences` proposal in
  `PROPOSED.md`. Deliberately not done here: adding an unrequested CI gate is scope creep, and the
  CC-BY-4.0 call deserves its own ADR rather than a footnote in a test-runner commit.
- **Opened:** iteration 4

### The JSON API has no authentication, and the error path is shaped around that
- **Kind:** partial-coverage
- **What is proven:** `GET /api/graphs` returns 200 with the registry, `POST` returns 405, an
  unmatched `/api/…` returns 404, and a store failure returns a 500 whose body carries neither the
  customer's graph IRIs nor their store path — asserted directly, and both mutants killed.
- **What is not:** **nothing authenticates or authorises this endpoint.** Anyone who can reach the
  port can enumerate every vocabulary IRI in the deployment. That is the expected state for Phase 1
  (auth is Phase 7) and it is not a bug, but it is a fact the loop should not lose track of, because
  a second decision has already been taken *because* of it: `adr/0008` §3 deliberately withholds the
  store's own error text from the response, which costs real diagnostic value and cuts against
  `CLAUDE.md` §3's explainability commitment. When Phase 7 lands, that trade should be revisited
  rather than inherited — an authenticated administrator should get the detail.
- **What would close it:** Phase 7's authorisation, plus a test that an authenticated administrative
  caller receives the full store error and an unauthenticated one does not.
- **Opened:** iteration 6

### `main`'s refusal to close a still-shared store is inspected-only
- **Kind:** inspected-only
- **What is proven:** the happy path, and it is proven where it can actually fail.
  `the_graph_registry_is_served_over_http_and_the_store_still_closes_cleanly` serves a real HTTP
  request from the real binary — which is what makes a connection clone the shared state — and then
  signals it, asserting exit zero and `store closed cleanly`. Before this iteration every shutdown
  test signalled a process that had never served a request, so the reclaim was trivially safe.
- **What is not:** the *failure* branch. If `Arc::into_inner` returns `None`, `main` bails with an
  error saying the store could not be closed cleanly, and nothing exercises that: producing it needs
  a deliberately leaked handle, which we have no way to inject without adding a seam whose only user
  is the test. So the message an operator would see in the one situation where their writes might
  not have reached disk has never been printed by a running process.
- **What would close it:** honestly, little worth its cost today. Revisit if a future feature holds
  a store handle outside the router — a background materialisation pass, a scheduled backup — since
  that is when the branch stops being theoretical.
- **Opened:** iteration 6

### `useProbe` re-fetches on a URL change that no caller makes
- **Kind:** no-production-caller
- **What is proven:** the hook fetches once on mount, checks the status before parsing the body,
  aborts on unmount, and stays silent on an abort — 22 assertions across two components, and the
  four mutants against those behaviours are killed.
- **What is not:** its effect depends on `[url]`, so changing the `url` prop would cancel the first
  request and issue a second. **Nothing changes it** — both call sites pass a string literal — so
  that branch has no production caller and no test. It is one line and removing it would be worse
  (a stale response for the old URL is a nastier bug than an unused code path), but it should not be
  counted as proven behaviour.
- **What would close it:** the first component that reads a parameterised URL — Phase 2's
  per-vocabulary views are the likely first — should arrive with a test that changing the URL
  abandons the first response.
- **Opened:** iteration 6
