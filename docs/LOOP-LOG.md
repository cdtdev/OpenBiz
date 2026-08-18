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

## Iteration 2 — 2026-08-18
- **Took:** Phase 0 — "Config from a file as well as the environment (only `OPENBIZ_*` env vars are
  read today)". The last unblocked item in Phase 0.
- **Did:** configuration is now three documented layers — defaults → TOML file → environment — and
  every effective value carries **where it came from**. `OPENBIZ_CONFIG` names a file; absent that,
  `openbiz.toml` in the working directory, whose absence is normal and silent. An explicitly named
  file that is missing is a hard error, because an explicit request must never degrade to the
  defaults. Applied the new standing direction ("parity is failure") before building: the
  incumbents' weakness here is not the file format, it is that a deployment's *effective*
  configuration is unknowable — spread across an app-server descriptor, properties files, and a
  triplestore config, with a misspelled key silently ignored. So two things beat parity rather than
  matching it. (1) An unrecognised key **fails the load**, with TOML's span naming the line and
  serde naming the keys we accept. (2) Every setting is a `Setting<T>` carrying a `Source`
  (`Default` / `File(path)` / `Env(name)`); `main.rs` logs one line of provenance per setting before
  it binds, and the bind failure now reads `failed to bind 0.0.0.0:80, from
  /etc/openbiz/openbiz.toml` rather than naming only the address that did not work. A blank value is
  an error naming its source, not an absence. `Deref`/`Display` forward, so provenance costs call
  sites nothing.
- **Tests:** 28 → 45. Sixteen in `crates/openbiz-server/src/config.rs`: precedence per layer and
  per setting, partial files, `OPENBIZ_CONFIG` selection against a decoy default file, missing
  default vs missing explicit file, unknown key, malformed TOML, wrong type, blank value from each
  of the three sources, comments, and the provenance report. `cargo fmt`, `clippy -D warnings`,
  `cargo test --workspace`, and `cargo deny check licenses bans sources` all green; `toml` and
  `tempfile` needed **no** widening of the §5 allow list. The UI did not change, so its build was
  not re-run. Beyond the suite: verified all four failure paths by hand against the real binary and
  corrected `docs/CONFIGURATION.md` where my documented log line did not match what it actually
  emits.
- **Learned:** two things. (1) The unknown-key test passed the first time it ran, which is not
  evidence — so I deleted `deny_unknown_fields` and confirmed it goes red, then restored it. Every
  one of these sixteen tests passed on first run because the code was written alongside them; that
  makes "prove the load-bearing assertion can fail" a step worth doing explicitly rather than a
  formality, and I should do it for the *most load-bearing* assertion of every item, not only when
  test-first happens to be natural. (2) Injecting the environment lookup into `Config::resolve` was
  forced rather than chosen: `std::env::set_var` mutates state shared by every thread in the test
  binary, so environment-mutating tests are flaky against each other by construction. The good
  design came out of a constraint, and it left a real hole — `Config::load` itself is now the one
  untested line of wiring, recorded rather than glossed.
- **Recorded:** `adr/0005` (precedence, TOML over JSON/YAML, `deny_unknown_fields`, the provenance
  model, and why `figment`/`config-rs` were rejected on §1.5) and a new `docs/CONFIGURATION.md`.
  Drained the feedback inbox **first**, before starting work: logged the "parity is failure"
  standing direction verbatim to `FEEDBACK-LOG.md`, truncated the inbox immediately, then wrote it
  into `BUILD-PLAN.md` as a standing instruction and opened a "Parity findings" section in
  `PROPOSED.md` for the honest "we can only match here" cases. `promote-queue.json` was already
  `[]`. In `UNTESTED.md`: closed the config-file half of the env-only entry and **split out three
  narrower gaps** rather than letting the closure imply more than it earned — `data_dir` is
  configured and consumed by nothing, `bind` is validated for shape but not meaning, and
  `Config::load`'s wiring to the real environment is inspected-only. Two proposals opened
  (subprocess test for `load`; effective-configuration screen in the Phase 14 admin console). Nil
  LLM opportunity again, with the reasoning: refusing to interpret a misspelled key is the point of
  this change, and an LLM guessing at it would be the opposite. One note carried forward — `Source`
  is the same provenance shape `adr/0002` requires of every LLM proposal, and Phase 10 should not
  invent a second vocabulary for it.
- **Still uncertain:** whether "blank is an error" survives contact with real deployments. It is
  right on principle — an empty `OPENBIZ_BIND=` is a silent ignore wearing a different hat — but
  `docker compose` interpolating an unset variable to empty is extremely common, and the failure
  mode is a container that will not start where the incumbents would have shrugged and used the
  default. I think refusing is correct and I have no evidence either way; the first person to
  deploy this in a compose file is the test, and there is nobody to ask yet. The narrower version
  of the same doubt: this iteration decided per-setting validation is "more machinery than the
  problem" at two settings, and I do not know where that stops being true — Phase 1's `data_dir`
  alone wants an is-this-a-writable-directory check, so it may be one item away rather than ten.

## Iteration 3 — 2026-08-18
- **Took:** Phase 1 — "`openbiz-store`: embedded Oxigraph lifecycle — open, close, durable path,
  graceful shutdown". **The tree was dirty at the start:** a previous iteration had built this item
  essentially to completion and was interrupted before landing it. I inspected it rather than
  building on it blind, found it complete and charter-compliant, and finished it — the ADR it
  referenced three times did not exist, the README carried no build prerequisite, CI had no clang
  step, and none of it had ever compiled. So the honest description of this iteration is: *verify,
  complete, and land someone else's interrupted work*, not "write a store".
- **Did:** the store is now real and load-bearing. `Store::open` creates the directory, takes the
  backend's exclusive lock, and stamps or checks a `FORMAT_VERSION`; `close()` consumes the store,
  flushes, and reports whether that worked. `main.rs` opens it **before** `TcpListener::bind` and
  closes it **after** `axum`'s graceful shutdown has drained, so the ordering is stop-accepting →
  drain → flush → log `store closed cleanly`. `shutdown_signal()` handles `SIGINT` and `SIGTERM`;
  `SIGTERM` is the one that matters, because `docker stop`, a Kubernetes eviction, and `systemctl
  stop` all send it and a `Ctrl-C`-only service is hard-killed on every routine restart. Applied
  "parity is failure" before building: every incumbent has a triplestore, and what they do badly is
  keep it in a *separate lifecycle from the application* — from which "up but useless", two
  instances on one directory, unordered shutdown, and silent version downgrade all follow. One
  lifecycle answers all four, and each answer is a test.
- **Tests:** 45 → 59, all green, plus `cargo fmt`, `clippy -D warnings`, and `cargo deny check
  licenses bans sources`. Twelve in `openbiz-store` (fresh open, stamp survives close/reopen,
  nested paths created, second open refused, store-from-the-future refused, two stamps refused,
  non-numeric stamp refused, close releases the lock, data dir that is a file, unwritable parent);
  three in `crates/openbiz-server/tests/graceful_shutdown.rs`, which spawns **real binaries** —
  `SIGTERM` exits zero and leaves a reusable directory, a second instance refuses to share the data
  directory, and the process environment reaches `Config::load` with its provenance intact. Beyond
  the suite I checked the load-bearing assertion *discriminates*: a `SIGKILL` exits 137 and writes
  **no** `store closed cleanly` line, while `SIGTERM` writes exactly one — and the next start
  reopened the store after the hard kill. Recorded as a hand check, not a test.
- **Learned:** three things. (1) **The pipe masked the failure.** My first `cargo test --workspace |
  tail -40` reported exit 0 while the build had panicked in `bindgen` — the driver warns about this
  for `gh pr checks` and it bit here instead, in the same shape. I now redirect to a file and test
  `$?`. Had I trusted it, I would have pushed an unbuilt branch and called it green. (2) **This
  machine cannot build the store from a clean checkout** — no `clang`, no `libclang`, no
  passwordless `sudo`. Rather than substitute Oxigraph's in-memory backend (which would have been
  exactly the "silently substituting a weaker implementation" the charter forbids), I extracted
  `libclang1-20` and `libclang-common-20-dev` from downloaded `.deb`s into `~/.local/libclang`. It
  works, it is outside the repo, and it will not survive a machine reset — recorded in `UNTESTED.md`
  so a future iteration reads the entry instead of concluding the store is broken. (3) **The §5
  licence escalation was not needed.** `CLAUDE.md` anticipated Oxigraph's tree as the first case
  forcing an allow-list decision; it resolves entirely within the existing list and `cargo deny`
  passes unchanged. Worth stating plainly so nobody later reads `adr/0006` as having used that path.
- **Recorded:** `adr/0006` (adopting Oxigraph as load-bearing; the four incumbent failure modes; why
  `default-features = false` keeps `http-client` out of an air-gapped build; why `len()` is O(n) and
  there is deliberately no public `quad_count()`; why `trait RdfStore` is deferred rather than
  guessed at with one implementation). Closed two `UNTESTED.md` entries — `data_dir` has a consumer
  for the first time, and `Config::load` now has a real-process regression test. Opened **six**
  narrower ones, and the largest is the honest one: **the named-graph model has no production
  caller** — `GraphId`, `GraphKind`, and `is_directly_writable()` are unit-tested rules that nothing
  invokes, and `StoreError::NotWritable` is a variant never returned. The store has held exactly one
  quad, ever. Also opened: durability proven for one quad not a vocabulary, shutdown proven to exit
  cleanly but never with a request in flight, the lock classifier's dependence on a RocksDB message
  string, Unix-only shutdown coverage, and the libclang gap above. Drained both human inboxes
  first: `feedback.md` was empty (nothing to log), and `promote-queue.json` held four entries, all
  applied — a UI test runner into Phase 0, the `Config::load` subprocess test into Phase 1, a
  headless-browser smoke test into Phase 3, and the effective-configuration screen into Phase 14 —
  with each proposal marked `promoted (→ Phase N)` and the queue emptied to `[]`. The Phase 1 one
  was closed by this item's own harness, so it is checked off with a note saying so rather than
  claimed as separate work. Nil LLM assistance opportunity, and not for lack of looking: process
  lifecycle and lock arbitration are exactly the kind of thing that must be deterministic and
  explainable, and an LLM in that path would be a liability rather than a help.
- **Still uncertain:** whether opening the store before binding is right when the store is *large*.
  It is unambiguously right today — a store that will not open should never become a server that
  accepts requests — but `Store::open` is currently a few milliseconds on an empty directory and I
  have measured nothing else. If RocksDB recovery after an unclean stop takes tens of seconds on a
  1M-concept store, then "cold start under a few seconds" (§1.5) and "never up but useless" become
  a genuine conflict rather than a happy coincidence, and I do not know which way to resolve it —
  a readiness endpoint that reports "opening" is the obvious answer and is also precisely the
  "up but useless" ambiguity I just congratulated us for eliminating. The Phase 1 benchmark spike
  should measure open and close times, not only query evaluation, and it currently says neither.

## Iteration 3 (addendum) — 2026-08-18 — feedback drained after the item landed
- **Took:** product-owner feedback that arrived in `feedback.md` **after** iteration 3's drain and
  while the store item was in CI. Acted on immediately rather than deferred, because it is ledger
  bookkeeping plus a standing direction, not build work.
- **Did:** verified the claims against the API before recording them, rather than taking them on
  trust — the repository is `"visibility": "public"`, and ruleset `main-protection` is
  `enforcement: active` on `main` with rules `required_status_checks`, `non_fast_forward`, and
  `deletion`, requiring all four CI jobs (`Rust`, `Licence policy`, `UI`, `Single binary`) and with
  an **empty** `bypass_actors`, so it binds the owner too. Moved the branch-protection entry in
  `BLOCKED.md` to Resolved (keeping the original text in a `<details>` block — the record of what
  took how long to close is the signal), checked off the Phase 0 item, and rewrote the plan's
  `**Status:**` line: **Phase 0 is now complete with no open items.** `BLOCKED.md`'s Open section is
  empty for the first time.
- **Learned:** the blocker was closed by a human taking the commercial decision it deferred to them,
  not by the loop finding a way around it — which is the outcome `PROPOSED.md` and `BLOCKED.md`
  exist to produce, and the first time the brake has demonstrably worked rather than merely been
  obeyed. Also worth stating: `gh pr checks --watch --fail-fast` **stays** in the driver. The server
  now refuses a red merge, but the watch is how the loop *finds out* in time to fix it on the
  branch. What has gone is the silent-failure mode, not the reason for the step.
- **Recorded:** the feedback verbatim in `FEEDBACK-LOG.md`, with a note that it arrived
  mid-iteration and was drained at the end of the same one. On the "the README is now a public shop
  front" direction I found one real discrepancy and fixed it, which this iteration had itself
  created: `default-features = false` drops the embedded engine's HTTP client, so SPARQL
  **Federated Query** is not compiled in — while the README listed it in the standards line without
  qualification. It now says so plainly, alongside the existing OWL DL admission. The rest of the
  README's claims sit under its pre-alpha disclaimer and match the plan.
- **Still uncertain:** whether "no outbound code path at all" is a position we can actually hold.
  It is unambiguously right for the LLM providers, where `adr/0002` makes egress opt-in and
  auditable. Federated query is different in kind: `SERVICE` is a *query author's* construct, so the
  egress decision is taken per-query by whoever writes the SPARQL, not per-deployment by an
  operator — and I do not know whether a per-deployment allow-list of federation endpoints is
  genuine control or security theatre that an author routes around with a literal IRI. That question
  needs answering before the Phase 1 SPARQL items, not when federation is finally implemented.

## Iteration 4 — 2026-08-18
- **Took:** Phase 0 — "UI test runner (Vitest + Testing Library) with a test per `Probe` state,
  wired into CI", the last unchecked Phase 0 item. **First, though, the iteration landed dangling
  work:** the previous iteration's addendum sat on `item/phase0-close-branch-protection` as **open
  PR #7**, pushed but never merged, so `main` did not contain the branch-protection close, the
  README's federation correction, or the addendum log entry. All four checks were already green; it
  merged as `423b1e3` before anything new was started. Worth naming as a loop failure mode: the
  driver's step 7 ends with a merge, and an iteration that dies between `push` and `merge` leaves no
  trace anywhere except an open PR that nothing reads. `git status` was clean and `main`'s last run
  was `success`, so both of step 1's tripwires said "fine".
- **Did:** `ui/` has a test runner and CI runs it. Ten assertions over `App` in `src/App.test.tsx`,
  covering all three `Probe` states — the single `/healthz` call and loading text on mount, the
  success line naming status and version, `role="alert"` for an HTTP refusal / a transport rejection
  / a non-`Error` rejection, the unmount abort, the `AbortError` swallow, and a StrictMode
  double-mount that must not paint a spurious alert. Queries are by accessible role and visible
  text, never test IDs, so the assertions are about what a user perceives. `passWithNoTests: false`
  is stated explicitly in `vite.config.ts`, and `npm test` is a step in the `UI` job.
- **Tests:** 59 Rust (unchanged) + **10 UI (new)**. `cargo fmt`, `cargo clippy -D warnings`,
  `cargo test --workspace`, `cargo deny check licenses bans sources`, `npm run typecheck`,
  `npm test`, `npm run build` all green. **The suite was proven to discriminate before it was
  trusted:** seven mutations of `App.tsx` — dropping the `response.ok` guard, dropping the
  `AbortError` swallow, dropping the unmount abort, never leaving the loading state, removing
  `role="alert"`, probing `/health` instead of `/healthz`, and blanking the non-`Error` message —
  were each run against the suite and each turned it red.
- **Learned:** three things, and the first is the real one. (1) **My first draft was a false green.**
  The `AbortError` mutant *survived*: the suite passed against an `App` that reports every aborted
  probe as "Cannot reach server". The cause was that my `fetch` stub ignored the `AbortSignal` it
  was handed, so the branch it claimed to cover was never entered — and the test I had written for
  it, the StrictMode one, passed for an unrelated reason. Had I stopped at "10 tests green" I would
  have checked off an item whose headline assertion was decorative. The stub now rejects with a real
  `AbortError` when its signal fires, and a second, direct test targets the branch without relying
  on StrictMode's timing. **Mutation testing is what turned a plausible suite into an evidenced
  one**, and the only reason it happened is that the charter demands a failing test be proven to
  catch the bug. (2) **The item's own promoted description was wrong**, in the direction that
  flatters us less: `npm test` was not "a no-op that passes silently" — there was no `test` script,
  so it exited 1, and the `UI` CI job never invoked it at all. Same effect, different mechanism, and
  the falsehood was in the loop's reporting rather than in npm's behaviour. Corrected in the plan
  and in `UNTESTED.md` rather than quietly fixed. (3) **`LIBCLANG_PATH` alone does not rebuild the
  store on this machine.** `bindgen` got past "unable to find libclang" and died on
  `'stdbool.h' file not found`, because libclang could not locate its own resource dir; it needs
  `BINDGEN_EXTRA_CLANG_ARGS=-resource-dir=.../clang/20` as well. The `UNTESTED.md` entry described
  only the first symptom, so it read as "already known" while describing a different failure. Both
  variables are now written down verbatim.
- **Recorded:** closed the `UNTESTED.md` entry "The UI has no test suite at all" and opened its
  honest successor, "The UI suite asserts on jsdom, and covers one component" — jsdom is not a
  browser, `main.tsx` is untested, and §4.4's keyboard-navigability clause is still unenforced
  because `App` has nothing to tab to. Amended the store-toolchain entry with the full incantation.
  Opened a third: **the UI dependency tree is outside the §5 licence gate.** `cargo deny` covers the
  Rust tree; nothing covers npm, so §5's "every new dependency gets a licence check" is done by hand
  for `ui/`. I did it by hand — the four packages added are MIT, and a sweep of all 153 installed
  packages found one unlisted licence, `caniuse-lite` under **CC-BY-4.0**: pre-existing, transitive
  via Vite's `browserslist`, build-time only, a data package that never reaches the binary, and
  attribution-only rather than copyleft. On substance that is §5's "unlisted but permissive" case,
  not the forbidden one — but §5 requires an ADR for that judgement and I did not write one, because
  bundling a licensing decision into a test-runner commit is how allow lists get widened quietly.
  It is a proposal instead, covering the gate *and* the ADR, for a human to promote. Also recorded a
  **parity finding**: a UI test runner is hygiene, not a wedge — we were behind and have caught up —
  and on the accessibility half of the "joyless UI" row we currently match the incumbents'
  intentions and have shipped nothing better. Nil LLM opportunity, with reasoning: generating tests
  is a fact about our process, not something a taxonomist would ever invoke.
- **Still uncertain:** whether the loop can actually keep §4.4's keyboard clause once Phase 3 starts
  producing components faster than one per iteration. Today the clause is satisfied vacuously — no
  interactive element exists — so nothing has ever tested whether we honour it under pressure, and
  the harness that would let us is now in place with no rule pointing at it. I do not know what the
  forcing mechanism should be: a lint cannot tell an interactive component from a static one, a
  review checklist is exactly the convention-not-enforcement thing branch protection replaced, and a
  blanket "every component needs a keyboard test" produces meaningless tests for a heading. The
  narrower and more troubling version: this iteration only discovered its own false green because
  mutation testing is cheap on one 45-line component, and I have no idea whether that practice
  survives contact with a design system — if it does not, "the UI suite is green" degrades back into
  the same unexamined claim it was this morning, just with more files.

## Iteration 5 — 2026-08-18
- **Took:** Phase 1 — "Named-graph model: one graph per vocabulary, plus a system graph for
  OpenBiz's own metadata", the next unchecked item. **Split in place**, because it was two items
  wearing one hat: the store model, and exposing it. This iteration is the model; the read half
  (`GET /api/graphs` plus the UI) is now its own item, and the *create* half is deliberately not
  next — §1.7 requires discovery before creation and `DiscoveryProvider` does not exist until
  Phase 2, so an endpoint that creates vocabularies now would be a charter violation dressed up as
  progress.
- **Drained first, as the driver requires:** `promote-queue.json` was `[]`. `feedback.md` held a
  product-owner correction, copied into `FEEDBACK-LOG.md` and truncated to empty *before* any work
  started. Its factual claim — the plan saying "Phase 0 is complete" while a Phase 0 item was
  unchecked — had already been closed by iteration 4, and I verified that by counting rather than
  assuming: 18 items, 0 unchecked. The **mechanism** it corrects stands and is the part that
  matters, so the counting rule is now written into the plan's header and a proposal is open for a
  CI job that checks the falsifiable half of the `**Status:**` line. This is a process error caught
  externally, not a doc fix: the loop described a phase from memory of what was in it before its own
  promotion, and it took a human reading the public repo to notice.
- **Did:** `crates/openbiz-store/src/graph.rs` is new — `GraphId` with **private fields**, so the
  pairing of IRI and kind is an invariant the type enforces rather than a convention callers follow.
  A vocabulary graph must be a valid absolute IRI (validated by the backend's own parser, so what we
  accept is exactly what the store, the serialisers, and SPARQL accept) and must be **outside**
  `urn:openbiz:`. An inferred graph's IRI is *derived* from its vocabulary, not chosen, so two
  vocabularies cannot share one and materialisation cannot be aimed at a graph a human authored. In
  `lib.rs`, `insert_into` is now the single function that writes anything: it takes a `&GraphId`,
  refuses one that is not directly writable, and the format stamp goes through it too. A graph
  registry lives in the system graph, `Store::open` registers the system graph into it on every
  open, and `main.rs` reads the registry **before it binds** and refuses to start if it cannot
  describe it. Registry writes are atomic via `Store::extend`, because a registry entry is two quads
  and a half-existing graph is worse than an absent one. See `adr/0007`.
- **Tests:** 84 Rust (was 59) + 10 UI (unchanged). `cargo fmt`, `cargo clippy -D warnings`,
  `cargo test --workspace`, `cargo deny check licenses` green. **Twelve mutants, all killed**, and a
  thirteenth rejected by the compiler: dropping the writability guard in `insert_into`; dropping it
  in `create_vocabulary_graph`; dropping the already-exists check; not registering the system graph
  at open; removing the sort from `graphs()`; making `from_registry` trust what it reads; defaulting
  an unknown kind token to `Vocabulary`; not enforcing the reserved namespace; not validating IRI
  syntax; matching the namespace with `contains("openbiz")` instead of a prefix; and two against
  `main.rs` — never reading the registry, and reading it without reporting it. The thirteenth,
  assembling a `GraphId` from a registry row by struct literal, **would not compile**, which is the
  private-fields decision paying for itself on the day it was made.
- **Learned:** three things. (1) **The registry did not need a format bump, and noticing that was
  the useful part.** A store written before it existed acquires one by being opened, because the
  quads are additive and an older build never looks for them. The reflex was to bump
  `FORMAT_VERSION` and write a migration; had I followed it, every future piece of system metadata
  would have cost a migration it did not need, and the migration framework — a Phase 1 item that
  does not exist yet — would have been forced early to service a change that required nothing from
  it. (2) **Reading the registry has to re-apply the same invariants as writing it.** I built the
  read path first as a plain deserialise and only then asked what a doctored backup does: a registry
  row claiming the system graph's IRI is a `vocabulary` would have handed a user write access to our
  bookkeeping through the ordinary authoring path. It is now `Corrupt`, and the mutant that trusts
  what it reads is killed. The general form is that anything durable is an attack surface, and this
  crate now has two examples — the format stamp and the registry. (3) **Oxigraph 0.5.9 has no
  closure-based `transaction()`;** `Store::extend` is the atomic multi-quad write, and it was enough
  here. Worth knowing before the next item, which is the public transactional write API and *will*
  need `start_transaction`.
- **Recorded:** closed the `UNTESTED.md` entry "The named-graph model has no production caller" —
  the largest no-production-caller gap the store had — and opened three honest successors rather
  than banking a clean sheet. `create_vocabulary_graph` has no production caller and will not until
  Phase 2's discovery hook, and I would rather say so than add a placeholder endpoint that violates
  §1.7. A corrupt registry is proven to stop the *store* but its refusal to start the *server* is
  inspected-only, because building that fixture needs raw quad access from outside the crate and the
  only cheap route is a test-only `oxigraph` dependency in `openbiz-server`, which cuts against §3
  for one assertion. And registry reads are unmeasured above four graphs while sitting on the
  startup path, so the Phase 1 benchmark spike now owes a third number. No parity finding: this is a
  row where we are genuinely ahead of the incumbents rather than catching up, and the plan item says
  why.
- **Still uncertain:** whether `urn:openbiz:` is a defensible reservation or one we will regret.
  It is unambiguously right *inside* the store — a user must not be able to author into our
  bookkeeping — but the IRIs it protects will leak outward the moment anything is exported, and I do
  not know what an inferred graph's IRI should look like to a consumer who is not us. Today it is
  `urn:openbiz:graph:inferred:<the vocabulary's own IRI>`, which is honest and machine-readable and
  also announces our product name inside a customer's exported data. A customer publishing that to a
  partner is publishing a fact about their tooling, and "we do not own a domain" is a reason not to
  mint `http:` IRIs, not a reason this particular shape is right. The narrower version I cannot yet
  answer: when Phase 11 exports a vocabulary with materialised inferences, does the inferred graph go
  in the export at all, and under whose namespace — because if the answer is "not ours", then this
  IRI scheme is an internal detail that must never be serialised, and nothing currently stops it
  being serialised.

## Iteration 6 — 2026-08-18
- **Took:** Phase 1 — "Expose the graph registry over HTTP (`GET /api/graphs`) and in the UI — the
  **read** half", the next unchecked item, and the one iteration 5 split out for exactly this.
- **Drained first, as the driver requires:** `promote-queue.json` was `[]` and `feedback.md` was
  empty, so nothing to log or truncate. `main` was mid-run when the iteration started — the check
  was watched to completion (`success`) rather than assumed from iteration 5's report, which is the
  whole point of that step.
- **Did:** the store is reachable from a browser for the first time. `app()` now takes an
  `AppState`; `main` wraps the open store in an `Arc`, hands the router a clone, and reclaims it
  with `Arc::into_inner` after the drain so `Store::close` still runs — failing loudly if it cannot,
  rather than skipping the flush quietly. `GET /api/graphs` serves the whole registry as JSON;
  `POST` is a **405**, because §1.7 has no honest creation path until `DiscoveryProvider` exists and
  a documented refusal beats a silence. `ui/src/Vocabularies.tsx` lists the vocabularies, and
  `ui/src/useProbe.ts` now holds the fetch-once-on-mount logic that was inline in `App`. See
  `adr/0008`.
- **The design decision worth naming:** the API returns **every** graph including OpenBiz's own, and
  the **UI** is what keeps them out of the user's list. The reflex was to filter in the endpoint so
  no client could show them by accident. That is the wrong layer. VocBench's failure is putting
  support graphs in front of a subject-matter expert who is then asked which graph to author into —
  a UI failure. Filtering in the API would swap it for an operator asking "what is in my store?" and
  getting a silently short answer, which is the opacity §1 exists to attack. So `kind` is on the
  wire, the endpoint never omits a row, and the graphs the interface holds back are **counted**
  ("1 further graph is held for OpenBiz's own use") rather than dropped.
- **Tests:** 97 Rust (was 84) + 22 UI (was 10). `cargo fmt`, `cargo clippy -D warnings`,
  `cargo test --workspace`, `cargo deny check licenses`, and the UI typecheck/build/test all green.
  **Thirteen mutants, all killed** — five Rust (inferred graphs reported as vocabularies; 200
  instead of 500; echoing the store's own error text to the client; filtering our graphs out of the
  *API*; not mounting the route) and eight UI (show every graph as a vocabulary; count all graphs as
  held back; announce "0 further graphs"; invert singular/plural; render an empty list instead of
  the empty state; trust a non-2xx body; report an abort as a failure; never abort on unmount).
  Also verified by hand against the running binary: 200 with the system graph, 404 on `/api/graphss`,
  405 on `POST`, `store closed cleanly` on `SIGTERM`.
- **Learned:** three things. (1) **A mutant is what told me a test was missing, not a review.** The
  "announce zero held-back graphs" mutation initially killed nothing — every registry fixture I had
  written contained the system graph, so the `internal > 0` guard was never exercised at zero. The
  suite looked thorough and had a hole shaped exactly like the case a real deployment reaches the
  moment it has one vocabulary and nothing else. Writing the mutants **before** declaring the suite
  done is now twice-proven as the thing that catches a plausible green. (2) **Sharing the store with
  the router quietly weakened an existing test.** `Store::close` consumes the store, so the `Arc`
  reclaim is a new way for shutdown to fail — and every existing shutdown test signalled a process
  that had never served a request, so no connection had ever cloned the state and the reclaim was
  trivially safe in all of them. The new integration test serves a real request *first*. The general
  form: adding a sharer to an owned resource can turn a proven path into an unproven one without
  changing a line of the tested code. (3) **Extracting `useProbe` was safe to do in the same
  iteration as the feature only because `App`'s ten tests already existed** — they passed unchanged
  across the extraction, which is the evidence that the refactor preserved behaviour. Iteration 4's
  test runner paid for itself here, one iteration later than it was justified on.
- **Recorded:** three new `UNTESTED.md` entries and three amendments. New: the JSON API has **no
  authentication**, and `adr/0008` §3 already took a decision *because* of that — withholding the
  store's own error text from the 500 — which costs diagnostic value and should be revisited at
  Phase 7 rather than inherited; `main`'s refusal to close a still-shared store is inspected-only;
  and `useProbe`'s re-fetch-on-URL-change branch has no production caller. Amended: the registry
  scan is now on a **hot** path as well as the startup path and serialises with no paging, so the
  Phase 1 benchmark spike owes the endpoint a number *before* Phase 3 builds on this shape; the
  corrupt-registry fixture gap now has two callers wanting it rather than one; and the jsdom entry
  now covers two components, with §4.4's keyboard clause **still satisfied vacuously** after a
  second iteration of UI. One LLM opportunity recorded, and a genuine one: the list shows raw IRIs
  until Phase 2 gives graphs labels, which at enterprise scale makes an existing vocabulary
  unfindable and creating a new one the rational move — an agent that drafts a "what this covers"
  line for review pushes in §1.7's direction rather than against it.
- **Still uncertain:** whether returning the whole registry survives contact with scale, and I have
  now built a UI on the assumption that it does. The reasoning in `adr/0008` §1 is about *honesty* —
  an inventory endpoint that omits rows cannot be trusted about the rows it shows — and I still
  think it is right. But honesty and shape are different questions, and I answered the second by
  reflex from the first: an enterprise with a vocabulary per domain per jurisdiction gets a
  four-thousand-element JSON body on every page load, read from an unmeasured pattern scan, filtered
  client-side. The narrower version I cannot answer: if the spike says the scan is too slow, the fix
  is paging or a `?kind=` filter, and **a paged registry is one that omits rows** — at which point
  the argument that the API must never omit a row has to become an argument about *why* it omitted
  them, which is a different and much weaker claim. I do not know whether I have designed the
  contract or merely deferred it, and the honest answer is that the benchmark spike should have
  come first.

## Iteration 7 — 2026-08-18
- **Took:** Phase 1 — "Transactional write API with rollback; concurrent-reader safety under test",
  the next unchecked item.
- **Drained first, as the driver requires:** `promote-queue.json` was `[]` and `feedback.md` was
  empty, so nothing to log or truncate. `main`'s CI run was **in progress** when the iteration
  started; it was watched to completion (`success`) rather than read as the previous iteration's
  reported green, which is the whole point of that step.
- **The finding that shaped the item:** I wrote the race test before the fix, as the driver requires
  for a bug, and it did not fail the way I expected — **all eight racers succeeded**, not two. The
  check-then-write in `create_vocabulary_graph` was two separate backend operations, so every racer
  read "free" and every racer wrote. The consequence is worse than a duplicate row: a graph
  registered twice makes `Store::graphs` refuse the *whole* registry as `Corrupt`, so one user's
  mistimed second click takes the entire vocabulary list down for everyone.
- **Then the more important finding.** I read Oxigraph's storage layer before designing, and its
  transaction is a RocksDB snapshot plus a write-batch-with-index whose `commit()` is an
  unconditional batch write — **no conflict detection, no snapshot validation, no serialisation
  between transactions**. So wrapping the check and the write in a backend transaction would fix
  nothing: both racers still read the same snapshot and both still commit. I did not take that on
  my own reading. The first mutant removes our write lock while keeping the backend transaction,
  and the race comes straight back. The lock is load-bearing, and the claim in `adr/0009` is
  measured rather than argued.
- **Did:** `Store::transaction(|txn| ...)` — commits on `Ok`, discards on `Err` **and on panic**.
  A closure rather than a `begin`/`commit` pair, so the safe outcome is what a failing caller gets
  by default instead of what they must remember to ask for, and so `oxigraph::store::Transaction`
  stays out of our API (§3). Writers serialise on a mutex we own; readers never take it. The
  existence check moved *inside* the transaction, which is the actual fix. Nesting is refused with
  `StoreError::NestedTransaction` rather than deadlocking, keyed by store address so two stores over
  two data directories are not falsely refused. Mutex poisoning is recovered from, not propagated —
  a panic rolls the store back, so refusing every later write would turn one abandoned edit into a
  store that had silently gone read-only. See `adr/0009`.
- **The production caller, and it is a real behaviour change:** `Store::open` now commits the format
  stamp and the system graph's registry entry in **one** transaction. They were two independent
  writes, and a kill in the gap left a store that was stamped but had no system graph in its
  registry — a state this build reports as inconsistent, reached at the likeliest moment for a
  container to be killed. Every deployment runs this on every start.
- **The thing I nearly got wrong.** Moving the write choke point inside `Transaction` made it
  tempting to demote its `is_directly_writable` refusal to a `debug_assert`, since every current
  caller checks first — and the compiler pushed that way, because the refactor made `insert`
  infallible and the borrow checker was happier for it. That would have quietly converted a rule
  the store *enforces* into a rule callers are *trusted* to follow, which is exactly the silent
  weakening §4 warns about. It stays a runtime refusal. The existing test
  `no_write_reaches_an_inferred_graph` is what caught it: it failed to compile, which is the only
  reason I looked.
- **Tests:** 105 Rust (was 97) + 22 UI unchanged. `cargo fmt`, `cargo clippy -D warnings`,
  `cargo test --workspace`, and `cargo deny check licenses` all green. **Six mutants, all killed** —
  write lock removed; existence check moved back outside the transaction; commit even when the
  closure failed; mutex poisoning propagated; choke point stops refusing inferred graphs;
  reentrancy mark not keyed by store. Also verified by hand against the real binary: first start
  serves the system graph, `SIGTERM` logs `store closed cleanly`, and a reopen of the same data
  directory serves the same registry without re-stamping.
- **Learned, about the ledger rather than the code:** `UNTESTED.md` already recorded
  `create_vocabulary_graph` as having no production caller, and I had read that entry. What the
  entry framed as a *dormant* risk was an *untested* one — the concurrency defect sat inside a
  method with nine passing tests, and it survived precisely because nothing exercised it under
  contention. The entry has been amended to say so. Second, smaller: the toolchain entry for this
  machine's missing `libclang` was already in `UNTESTED.md`, complete with the exact two-variable
  incantation, and I re-derived it from the error messages instead of reading it. The ledger was
  right and I did not consult it; that is a cost of a long file, not of a wrong file.
- **Recorded:** three new `UNTESTED.md` entries and two amendments. New: write throughput under a
  serialised writer is entirely unmeasured, so the Phase 1 benchmark spike now owes a **fourth**
  number; rollback is proven against `Err` and panic but **not** against process death, and the
  kill-in-the-gap window `open()` was restructured to close is argued from the code's shape rather
  than from a test; and the nested-transaction test **hangs rather than fails** if the guard
  regresses, which is a real signal in a bad shape. One LLM opportunity recorded, and a genuine one
  created by this change rather than adjacent to it: serialising writers means a second author's
  work can be refused, and the manual path is a triple-level diff — which is exactly the thing
  governance teams say is not an answer to "does this affect what I was doing?".
- **Still uncertain:** whether serialising every writer is a decision I am entitled to make at this
  phase, or one I have made on this phase's evidence and will be unable to revisit. It is correct
  today and correct for the reason `adr/0009` gives — one process owns the store, so the lock is
  nearly free and it buys serialisability the backend does not offer. What I cannot answer is
  whether it stays nearly free. Upstream says a transaction holds its whole change set in memory,
  so the natural Phase 11 shape — one transaction per imported file — is both a long lock hold and
  a memory risk, and by then every write path above will have been written against an API that
  never made a caller think about how long it holds the lock. The narrower version: I have measured
  that the lock is *necessary* and not measured what it *costs*, and those are the two halves of the
  same decision. I built on the half I measured. If the spike comes back saying a 100k-concept
  import holds the write lock for minutes, the fix is finer-grained locking or optimistic retry with
  version counters, and both are much harder to retrofit under callers than they would be to have
  designed for now — which is the same failure mode as iteration 6's uncertainty about paging the
  registry, one layer down. Two iterations running, the benchmark spike is the thing I keep wishing
  had already happened, and it is still four items away in a plan I am not allowed to reorder.

## Iteration 8 — 2026-08-18
- **Took:** Phase 1 — "Parse and serialise Turtle, N-Triples, N-Quads, TriG, RDF/XML, JSON-LD —
  round-trip tested", the next unchecked item. **Split it in place** and did the first half.
- **Drained first, as the driver requires:** `promote-queue.json` was `[]` and `feedback.md` was
  empty, so nothing to log or truncate. `main`'s last CI run was `success`, checked rather than
  inherited from iteration 7's report; the tree was clean.
- **The split, and it is a charter constraint rather than convenience.** One item wearing two hats.
  A serialiser's production caller is an export, which is a read. A parser's production caller is an
  **import**, which mutates a vocabulary — and `CLAUDE.md` §3 says a change to a vocabulary arrives
  as a reviewable *candidate*, which is Phase 2's first item. Building the parser now had exactly
  two outcomes and both are the failures the charter names: code with no production caller (§4.1),
  or a direct-write `POST /api/import` to be retrofitted later, which is verbatim what §3 warns
  about. So the parser lands with backup/restore (parses N-Quads, touches no vocabulary) or with the
  candidate seam, whichever comes first. Round-tripping did not have to wait: serialise, re-read
  with the engine's parser, compare the statement set.
- **Did:** `RdfSyntax` — our own six-variant enum owning the media type, extension, and `?format=`
  token for each syntax. `Store::export_graph(iri, syntax, writer)` streams one graph out.
  `GET /api/export?graph=…&format=…` serves it with `Accept` negotiation, and
  `GET /api/export/formats` advertises what this build can write. The interface gained its **first
  interactive control**: a format chooser, with a per-vocabulary download link. See `adr/0010`.
- **The finding I did not expect, and it changed the type.** `oxrdfio::RdfFormat` carries **N3**,
  which is not a W3C Recommendation and is not on `CLAUDE.md` §2's standards surface. Re-exporting
  the engine's enum — the obvious thing, and one line — would have published a seventh serialisation
  we have never tested, documented, or committed to. That is a standards claim made by accident,
  which §4.5 forbids, and §3's "no third-party type in our API" would have caught it only as a style
  rule. The rule turned out to have a second, better reason than the one it is written for. A test
  now asserts the gap is deliberate: the engine still recognises `text/n3`; we return `None`.
- **The thing I got wrong and caught.** I wrote in `export_graph`'s doc comment that reads are "a
  snapshot per quad rather than one snapshot for the whole scan", and recorded it as a consistency
  gap. Then I read Oxigraph's `Store::quads_for_pattern` and it takes `self.storage.snapshot()` once
  and holds it for the iterator's life — so an export **cannot** be torn by a concurrent commit, and
  I had written a false weakness into the code and was about to write it into `UNTESTED.md`. Both
  are corrected. The real, narrower gap is that `contains_graph` runs on its own earlier snapshot,
  which is unreachable today because nothing deregisters a graph; that one is recorded, honestly, as
  argued-from-code rather than tested.
- **Better, not parity** (the standing instruction, answered before building): three things the
  incumbents do badly here. Their export is *not what you saw* — PoolParty and TopBraid EDG keep
  project bookkeeping in the content store, so a consumer must be told which parts to ignore; ours
  cannot, because `adr/0007` never put our metadata in a vocabulary, and a test asserts
  `urn:openbiz:` appears in no export. Their export is *silently lossy* — three of six syntaxes
  cannot record a graph name, universally true and universally unmentioned; ours states it, from the
  same constant the serialiser branches on. And their export is *a wizard or a job*, so it cannot be
  scripted or diffed in CI; ours is one URL the interface and `curl` share.
- **Tests:** 143 Rust (was 105) + 29 UI (was 22). `cargo fmt`, `cargo clippy -D warnings`,
  `cargo test --workspace`, `cargo deny check licenses`, and the UI typecheck/test/build all green.
  **Twelve mutants, all killed** — registry check removed, graph name never written, every graph
  exported instead of one, document never finished (this one only dies on RDF/XML and JSON-LD, which
  is why the empty-document test exists), `Accept` weights ignored, unsatisfiable `Accept` silently
  defaulted, filename left unsanitised, unknown `?format=` defaulted, the graph-name claim made to
  disagree with the engine, and three of the component: link ignoring the chosen format, warning
  removed, IRI spliced into the URL unescaped. Also verified by hand against the real binary:
  TriG and N-Quads exports with correct headers, `Accept:` negotiation, and a 404 for a graph that
  is not registered.
- **Learned, about test design.** The round-trip test passed on the *first* run, which for a
  seven-statement fixture across six syntaxes is exactly the result a vacuous test gives. Two
  assertions were added before I would believe it — that the expected set has as many members as the
  fixture has statements, and that the bytes are non-empty — and then four mutants were run against
  it. That is the second time this loop has learned that "green immediately" is a prompt to attack
  the test rather than to move on. Related: blank-node labels are document-scoped by specification,
  so comparing them would have tested the engine's label generator; they are collapsed to one
  placeholder, and the fixture has exactly one blank node so nothing can be conflated by it.
- **Recorded:** five new `UNTESTED.md` entries and three amendments. New: the round trip is proven
  against **our own reader**, so it is fidelity and not conformance — if the engine's writer and
  reader shared a misreading we would pass and a third-party consumer would choke, and §4.5 means
  the claim this build makes is round-trip fidelity, not "we implement Turtle"; the HTTP layer
  buffers a whole graph even though the store streams; the interface's download path has never met a
  graph with statements in it, because nothing can put statements in one yet; `X-OpenBiz-Graph` has
  no reader; and the check-then-scan snapshot window above. Amended: the benchmark spike now owes a
  **fifth** number; a second test now hangs-rather-than-fails if its property regresses, which makes
  that a pattern; and §4.4's keyboard clause is no longer satisfied *vacuously* — there is a real
  control now, asserted to be a native `<select>` with a label that accepts focus, which is the
  thing that makes a tab order but is not the tab order. One LLM opportunity recorded, and the note
  worth carrying is that it is the **third** iteration to describe the same capability from a
  different seat — Phase 10 should build "explain a set of RDF changes in the vocabulary's own
  terms" once and route three questions to it.
- **Still uncertain:** whether "round-trip tested" is a test at all, or a tautology I have dressed as
  one. The suite serialises with Oxigraph and re-reads with Oxigraph, so what it proves is that the
  library agrees with itself. Every mutant I killed was a mutant of *my* code — the branch that
  picks quad-versus-triple, the registry check, the graph filter, the call to `finish()` — and those
  are genuinely proven. What is not proven, and cannot be by this method, is the layer underneath:
  a shared misreading of Turtle's escaping rules, or an RDF/XML writer that emits something its own
  reader accepts and `rapper` does not, passes silently. I know the fix (the W3C rdf-tests suites,
  or cheaper, a handful of fixtures produced by an independent tool) and I did not do it, and the
  reason is not good: I judged it out of scope for one item, having already spent the item's budget
  on a split I think was right. So the position I have landed is that OpenBiz can now hand a
  customer a file, and the only evidence that the file is correct is that OpenBiz can read it back.
  That is exactly the shape of assurance `CLAUDE.md` §4.5 exists to reject, and the fact that I wrote
  the honest caveat into `UNTESTED.md` does not make the export any more interoperable — it makes
  the gap *visible*, which is a different and lesser thing. The narrower question I cannot answer:
  whether "prove it against the spec's own test suite" is a task the next parsing item should carry,
  or a Phase 1 item of its own that nobody will schedule because the ledger entry makes it feel
  already handled.

