# ADR 0004 — Embed the UI with `rust-embed`, and prove it by deleting the source

**Status:** accepted (2026-08-18) · **Phase:** 0

## Context

`CLAUDE.md` §1.2 makes "single binary" a non-negotiable: the server, the UI assets, and eventually
the store ship as one executable plus a data directory. Until this iteration the two halves were
not connected — `ui/` built to `ui/dist` and nothing served it, recorded in `UNTESTED.md` as the
most load-bearing open gap in Phase 0.

The complication is that `ui/dist` is a build artefact. It is gitignored, it needs a Node toolchain
to produce, and it does not exist in a fresh clone. Whatever we choose has to hold two things at
once: the Rust test suite must not become hostage to the frontend build, and a release binary must
never be able to ship without the real interface inside it.

## Decision

**1. `rust-embed` compiles `ui/dist` into the binary.** It is MIT, has a small transitive tree
(`globset`, `bstr`, `sha2`, `mime_guess`), and `cargo deny check licenses bans sources` passes
unchanged — no widening of the §5 allow list was needed, so this ADR records no licence exception.

**2. The `debug-embed` feature is on.** Without it `rust-embed` reads from the filesystem in debug
builds. The tests would then prove that the disk still has a copy of the UI, which is precisely the
thing we are trying not to rely on. With it, debug and release behave identically and the tests
assert against bytes that are actually in the binary.

**3. `build.rs` refuses a release build with no UI, and synthesises a placeholder otherwise.**
A debug build with no `ui/dist` gets a clearly-marked stand-in page, a loud `cargo::warning`, and a
`openbiz_placeholder_ui` cfg that compiles out the one test which needs real Vite output. A release
build with no UI — or with a leftover placeholder, detected by a `.openbiz-placeholder` sentinel —
fails the compile with an actionable message. `cargo test` works on a machine with no Node; a
release binary with a stub UI inside it cannot be produced.

**4. The router's fallback is not a blanket "return index.html".** Three cases, because conflating
them produces bad failure modes:

| Request | Response | Why |
|---|---|---|
| unmatched `/api/…` | 404 | An API client must not receive HTML that parses as neither JSON nor an error. |
| missing `/assets/…` | 404 | Returning the shell for a mistyped bundle surfaces in the browser as an opaque MIME-type error instead of a clear 404. |
| anything else | `index.html`, 200 | Client-side routes must deep-link. |

Non-`GET` methods to unknown paths get 405 rather than the shell, which is why the fallback is
registered with `fallback_service(get(…))` rather than as a bare handler.

**5. Caching is content-derived.** Vite fingerprints everything under `/assets/`, so those get
`max-age=31536000, immutable`. The shell gets `no-cache`, which means revalidate, not "do not
cache" — so it needs a validator to be worth anything. `ETag` is the SHA-256 `rust-embed` has
already computed, and `If-None-Match` is answered with 304.

## What was measured

Verified by running the release binary with `ui/dist` moved off disk entirely:

| Check | Result |
|---|---|
| `GET /` with no `ui/dist` on disk | 200, the real Vite `index.html`, 318 bytes |
| `GET /assets/index-<hash>.js` | 200, 194 087 bytes, `content-type: text/javascript`, `immutable` |
| `GET /vocabularies/acme` (deep link) | 200, the shell |
| `GET /assets/nope.js` | 404 |
| `GET /api/nope` | 404 |
| `GET /healthz` from the same origin | 200 `{"status":"ok","version":"0.0.1"}` |
| release build with the placeholder present | compile fails with the actionable message |
| `cargo test` with `ui/dist` absent | 14 of 15 pass; the Vite-output test compiles out |

Release binary: ~1m31s to build, `lto = "thin"`, `codegen-units = 1`, stripped.

## Consequences

- **The Rust CI job now needs Node.** `npm ci && npm run build` runs in `ui/` before the cargo
  steps, or the suite tests the placeholder instead of the product. This is a real coupling and it
  will slow the Rust job; the alternative — asserting nothing about the real UI — is worse.
- **A new `Single binary` CI job** builds for release, **deletes `ui/dist` and `ui/node_modules`**,
  then starts the binary and curls it. This is the only check that distinguishes "self-contained"
  from "the filesystem still has a copy", and it is the one that keeps §1.2 honest as the binary
  grows.
- **Development still uses the Vite dev server**, whose proxy forwards `/healthz` and `/api` to the
  Rust server on 8080. Changing the UI now requires a `cargo build` to see it *through the binary* —
  that is the cost of `debug-embed`, and the dev server is the answer to it.
- **The store is not yet in the binary.** §1.2's promise covers the store too; Oxigraph arrives in
  Phase 1 and this ADR does not speak for it.
