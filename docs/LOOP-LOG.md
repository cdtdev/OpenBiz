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
