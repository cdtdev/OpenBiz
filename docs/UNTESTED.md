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

### The UI is built but nothing serves it
- **Kind:** no-production-caller
- **What is proven:** `ui/` typechecks under TS strict and builds to `ui/dist` (28 modules, ~194 kB
  raw / ~61 kB gzipped). The Rust server compiles, serves `/healthz`, and returns 404 elsewhere.
- **What is not:** the two halves are not connected. No `rust-embed`, no static-file route, no test
  that a built asset is reachable from the binary. The `App` component's `fetch("/healthz")` has
  never run against the real server — only the Vite dev proxy would route it, and that is untested
  too. **Until this closes, the single-binary promise in `CLAUDE.md` §1 is a claim, not a fact.**
- **What would close it:** the two open Phase 0 items — embed `ui/dist` via `rust-embed`, serve it
  from `/`, and add a test asserting the server returns the embedded `index.html`.
- **Opened:** Phase 0 hand-build (pre-iteration-1)

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
