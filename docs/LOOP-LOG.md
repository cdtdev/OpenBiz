# Loop log

One entry per iteration, newest at the bottom. Terse. This is the loop's working memory across
context boundaries — the next iteration starts by reading the last few entries.

The **"still uncertain"** line is mandatory and must not be padding. It is the input to the
monitor's non-convergence detector: if the same doubt recurs across several iterations, the loop is
circling rather than learning, and a human needs to break the cycle. Writing "nothing uncertain" to
look competent disables the one signal that catches a stuck loop.

## Entry format

```
## Iteration N — <date>
- **Took:** <backlog item>
- **Did:** <what changed, in behaviour terms>
- **Tests:** <added/changed; suite result>
- **Learned:** <something true now that was not known before>
- **Recorded:** <UNTESTED/BLOCKED/PROPOSED entries opened or closed>
- **Still uncertain:** <the honest open question — required>
```

---

## Iteration 1 — 2026-08-18
- **Took:** Phase 0 — "Embed the built UI into the binary via `rust-embed` and serve it from the
  server" plus its paired test item. Taken together because the second is the first one's proof.
- **Did:** `ui/dist` is now compiled into the binary and served from the router's fallback, so the
  single-binary non-negotiable (`CLAUDE.md` §1.2) is a fact rather than an intention. The fallback
  discriminates rather than blanket-serving `index.html`: unmatched `/api/…` and missing
  `/assets/…` are 404, non-`GET` to unknown paths is 405, everything else is the SPA shell so deep
  links work. Fingerprinted assets get `immutable` caching, the shell gets `no-cache` with a
  SHA-256 `ETag` and 304 on `If-None-Match`. `build.rs` fails a *release* build when `ui/dist` is
  missing or holds a stale placeholder, and synthesises a marked placeholder in debug so the Rust
  suite still runs on a machine with no Node.
- **Tests:** 15 → 28. Thirteen unit tests in `src/ui.rs` covering the shell, deep links, every
  embedded asset's content type, both cache policies, both `ETag` branches, and all three 404/405
  paths; one end-to-end test over a real TCP socket (`tests/serves_embedded_ui.rs`, hand-rolled
  client — no HTTP client dependency added). New `Single binary` CI job builds for release, deletes
  `ui/dist` **and** `ui/node_modules`, then starts the binary and curls it. `cargo fmt`, `clippy -D
  warnings`, `cargo deny check licenses bans sources`, and the UI typecheck/build all green.
  Verified by hand that the release binary serves the real 318-byte Vite shell and the 194 087-byte
  bundle with `ui/dist` moved off disk, and that a release build with a placeholder refuses to
  compile.
- **Learned:** three things worth carrying. (1) `rust-embed` reads from the *filesystem* in debug
  builds unless `debug-embed` is set — without it every test here would have proven the disk works,
  not that the binary is self-contained; the default quietly makes the test worthless for the thing
  we care about. (2) The honest version of this item required inverting the check: asserting the
  binary serves the UI is weak, *deleting the source and then asserting* is the only form that
  distinguishes embedded from merely present. (3) The Rust CI job now depends on the Node toolchain,
  which is real coupling — the alternative was testing `build.rs`'s placeholder and calling it the
  product.
- **Recorded:** Closed the "UI is built but nothing serves it" gap in `UNTESTED.md` — the largest
  open Phase 0 gap. Opened three new ones, all narrower: no UI test runner exists at all (`npm test`
  in the driver is a silently-passing no-op), nothing has rendered the bundle in a browser, and
  asset serving is unproven above two files with no compression negotiated. `adr/0004` records the
  decision and the measurements. Two proposals opened (UI test runner; headless-browser smoke test)
  — not promoted. Drained the feedback inbox: added the Phase 2 candidate seam as that phase's first
  item on product-owner instruction, made "record LLM opportunities as you go" a standing
  instruction in the plan, opened the section for it in `PROPOSED.md`, and logged all of it to the
  new `docs/FEEDBACK-LOG.md`. Nil LLM opportunity recorded for this iteration rather than invented.
  `promote-queue.json` was already `[]`.
- **Still uncertain:** whether `debug-embed` is the right long-term call. It buys honest tests and
  costs a full `cargo build` to see any UI change through the binary. Right now the Vite dev server
  absorbs that, but its proxy only forwards `/healthz` and `/api` — once Phase 3 adds routes the
  proxy does not know about, dev and production diverge in exactly the way `adr/0004` congratulates
  itself for avoiding, and I do not yet know whether that bites at Phase 3 or much later. The
  related unknown is binary size: two files embed for free, and I have no measurement of what a real
  design system's `ui/dist` does to size, compile time, or the "cold start under a few seconds"
  commitment in §1.5.
