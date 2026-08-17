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

### Config is env-only; the file path is unimplemented
- **Kind:** partial-coverage
- **What is proven:** `Config::from_env` reads `OPENBIZ_BIND` and `OPENBIZ_DATA_DIR`, and
  `Config::default` binds loopback (tested).
- **What is not:** there is no config-file support despite the Phase 0 item naming it. The
  `data_dir` field is carried and logged but **no code creates or opens that directory** — it is
  inert until Phase 1 wires the store.
- **What would close it:** implement file config with documented precedence, and have the store
  actually use `data_dir`.
- **Opened:** Phase 0 hand-build (pre-iteration-1)
