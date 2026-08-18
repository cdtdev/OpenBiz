# Proposed work — awaiting human promotion

Work the loop believes is needed but **did not authorise itself**. This file is the brake on
autonomous scope creep, and it only works if the loop actually uses it.

**The loop writes here. The loop does not promote from here.** A human promotes items into
`BUILD-PLAN.md` via `/openbiz-status` → `promote <n>`. Never move your own proposal into the plan
and then build it — that defeats the entire point of the file.

## Entry format

Copy this shape exactly; `/openbiz-status` parses the `Status:` line semantically.

```
### <short imperative title>
- **Status:** proposed.
- **Gap:** what is missing or wrong today, concretely.
- **Why load-bearing:** what this unlocks, or what breaks without it. If it is a nice-to-have,
  say so plainly — an inflated proposal wastes a human decision.
- **Cost & impact:** rough iterations, any phase it depends on, runtime or UX impact.
- **Suggested phase:** Phase N.
```

`Status:` values: `proposed` (open), `promote-requested` (queued by a human, not yet applied),
`promoted (→ Phase N)`, `deferred`, `rejected`.

---

## Open proposals

### Add a UI test runner before Phase 3 begins
- **Status:** promoted (→ Phase 0) · **done, iteration 4.**
- **Gap:** `ui/package.json` has no `test` script, so there is no UI test runner at all. The
  iteration driver's `npm test` step is a no-op that passes silently, which is worse than having no
  step — it reads as a green suite. `App.tsx` has three `Probe` states (loading, ok, error) and none
  is asserted; React could throw on mount and every check we run today would still pass, because
  they assert on HTTP transport rather than on rendering.
- **Why load-bearing:** Phase 3 is the differentiator and is eleven items of UI. Retrofitting a test
  runner onto a built design system is far more expensive than installing one against two
  components, and `CLAUDE.md` §4.2 requires the UI suite to be green — a claim we cannot currently
  make truthfully. Also blocks §4.4's keyboard-navigability requirement from being checkable.
- **Cost & impact:** ~1 iteration. Vitest plus Testing Library, both MIT. Adds a CI step. No runtime
  or binary-size impact — dev dependencies only, never embedded.
- **Suggested phase:** Phase 0, or as the first item of Phase 3.

### Bring the UI dependency tree under the §5 licence gate
- **Gap:** `cargo deny check licenses` enforces `CLAUDE.md` §5 across the Rust tree in CI. Nothing
  equivalent exists for `ui/`, so §5's "every new dependency gets a licence check" is, for npm, a
  convention the loop follows by hand — which is precisely the enforcement gap branch protection
  was opened for on the git side. Adding four dev dependencies this iteration made it concrete: the
  check happened, but only because the iteration chose to do it.
- **Why load-bearing:** Phase 3 is eleven items of UI and will pull in a component library, an
  icon set, and a router. That is the moment an npm subtree acquires something copyleft or
  source-available, and the moment it is most expensive to discover late. An open-core product
  whose licence policy covers one of its two dependency trees does not have a licence policy.
- **The concrete finding that prompted this:** a hand sweep of all 153 installed packages found one
  unlisted licence — `caniuse-lite` under **CC-BY-4.0**, pre-existing, transitive via Vite's
  `browserslist`, build-time only, and a data package whose contents never reach the binary.
  CC-BY-4.0 is attribution-only, not copyleft, so on substance this is a §5 "unlisted but
  permissive" call rather than a wall. **It should be recorded in an ADR, and it is not** — the
  proposal should cover writing that ADR alongside the gate, not just the gate.
- **Cost & impact:** ~1 iteration. Candidate tools are `license-checker-rseidelsohn` (BSD-3) or a
  ~30-line `node` script over `node_modules` with no new dependency at all — the sweep run this
  iteration was the latter and worked fine, which argues for checking it in rather than adding a
  tool. Adds one CI step to the `UI` job. No runtime or binary-size impact.
- **Suggested phase:** Phase 0, or as the first item of Phase 3 — before the component library
  lands, not after.

### Headless-browser smoke test against the release binary
- **Status:** promoted (→ Phase 3).
- **Gap:** nothing has ever executed the bundle in a browser. `adr/0004` proves the right bytes are
  served; it cannot prove the app mounts. The gap will widen once Phase 3 adds routing, and the
  failure mode — binary serves 200, page is blank — is exactly the one transport tests cannot see.
- **Why load-bearing:** moderately. It is the only check that covers the whole promise end to end
  ("download, run, done"), and it would also catch CSP, `crossorigin`, and MIME-strictness problems
  that only appear in a real browser. Honestly a nice-to-have *until* Phase 3, and close to
  essential after it.
- **Cost & impact:** ~1 iteration. Playwright is Apache-2.0; it downloads browser binaries, which
  makes CI slower and adds a network dependency to the *build*, never to the product.
- **Suggested phase:** Phase 3, alongside the accessibility item it would share a harness with.

### Test `Config::load` against a real process environment via a subprocess
- **Status:** promoted (→ Phase 1).
- **Gap:** `Config::resolve` is tested exhaustively with an injected environment, but `Config::load`
  — the wiring that supplies `std::env::var` and the default path — has no automated test, because
  `std::env::set_var` mutates state shared across the test binary's threads. A typo in a variable
  name inside `load` would pass CI. The four failure paths were verified by hand this iteration,
  which is not a regression test.
- **Why load-bearing:** modestly, but it grows. Every phase adds settings, and each one widens the
  untested gap between the tested merge and the real environment. The same harness would let us
  assert on the startup provenance log, which is now a user-facing contract documented in
  `docs/CONFIGURATION.md` and asserted nowhere.
- **Cost & impact:** well under an iteration. Spawn the release binary as a subprocess with a
  controlled environment and a temporary directory, assert on its stderr — the same shape as
  `tests/serves_embedded_ui.rs`. No new dependency.
- **Suggested phase:** Phase 0, or folded into the first Phase 1 item that makes `data_dir` real.

### Show the effective configuration and its provenance in the admin console
- **Status:** promoted (→ Phase 14).
- **Gap:** `Setting`/`Source` know where every value came from, but that answer is only reachable in
  the startup log and in error messages. An operator debugging a running server has to find the log
  from process start, which in a container may be long gone.
- **Why load-bearing:** nice-to-have on its own; genuinely valuable as part of the Phase 14 admin
  console, where it is the difference between "the settings screen" the incumbents ship and a screen
  that answers *why* a value is what it is. Needs authentication first — effective configuration is
  not public information, so this cannot land before Phase 6.
- **Cost & impact:** small once the console exists; a read-only endpoint and a table. Must redact:
  by Phase 10 the config will hold LLM provider credentials, which `CLAUDE.md` §14 says never go in
  logs — so this screen must show a credential's *source* without its *value*.
- **Suggested phase:** Phase 14, with the admin console.

---

### Make the plan's `**Status:**` line checkable rather than hand-written
- **Status:** proposed.
- **Gap:** `BUILD-PLAN.md`'s `**Status:**` and `**Current position:**` lines are prose the loop
  writes from memory at the end of an iteration. After iteration 4 the product owner caught them
  claiming "Phase 0 is complete — no open items" while Phase 0 still held an unchecked box, because
  the loop had promoted an item into that phase minutes earlier and then described the phase from
  its recollection of what was there before. The factual error is fixed and the counting discipline
  is now written into the plan's header, but the discipline is a convention, and this repository has
  already learned once — with branch protection — what a convention is worth compared with a check.
- **Why load-bearing:** the repository is public. A plan that declares a phase complete while an item
  in it is open is exactly the "roadmap you cannot trust" failure we attack the incumbents for, and
  `CLAUDE.md` §4's "honesty over green" makes misreporting worse than the gap being reported. It is
  small now and corrosive as a habit — and it is the kind of claim a script can verify absolutely,
  since both the claim and the evidence are in one file.
- **What it would be:** a CI job that parses `BUILD-PLAN.md`, counts `- [ ]` per phase, and fails if
  the `**Status:**` or `**Current position:**` lines name a phase as complete that still has
  unchecked items, or state an item count that does not match. Roughly the same shape as the `Single
  binary` job: cheap, mechanical, and it fails on the branch rather than being noticed by a human
  three iterations later.
- **Cost & impact:** well under one iteration. A script plus a CI step; no runtime or binary impact.
  The honest risk is that it invites writing the status line to satisfy the parser rather than the
  reader, so it should check the *falsifiable* claims only and leave the prose alone.
- **Suggested phase:** Phase 0, as a harness item.

## Parity findings

Items where the honest answer to *"what would be materially better than the incumbents?"* is **"here
we can only match"** — recorded per the product owner's standing direction (`FEEDBACK-LOG.md`,
2026-08-18) rather than shipped quietly as parity.

_Iteration 2 (layered configuration) found a real "better": the incumbents' weakness is not the file
format but that a deployment's effective configuration is unknowable, so provenance on every setting
and a hard error on an unrecognised key beat parity rather than matching it. See `adr/0005`._

### A UI test runner is table stakes, and we are behind on the thing it enables
- **Iteration 4 (UI test runner).** The honest answer here is not even "we can only match" — it is
  **"we were behind and have now caught up"**. Every incumbent's UI is tested; ours had zero
  assertions. Installing Vitest is hygiene, not a differentiator, and framing it as a wedge would be
  dishonest. The one genuinely better thing in it is small and worth naming precisely: the suite was
  **proven to discriminate** before it was trusted, by killing seven mutations of the component. A
  green suite nobody has tried to break is a claim, not evidence.
- **The wedge this touches, and where we are still behind:** the charter's row is "dated, dense,
  joyless UI" → "visually stunning and modern", and `CLAUDE.md` §4.4 requires anything user-facing
  to be **keyboard-navigable**. The incumbents are genuinely weak here — dense, mouse-driven,
  specialist-facing screens. We now have the harness to assert keyboard behaviour and **no rule that
  forces anyone to use it**; `App` has no interactive element, so the gap is invisible today and
  will not be on the first Phase 3 item. Recorded in `UNTESTED.md`. This is a parity finding in the
  strict sense: on accessibility we currently match the incumbents' *intentions* and have shipped
  nothing that beats them.

---

## LLM assistance opportunities

Places found **while building** where LLM assistance would materially help, recorded per the product
owner's standing instruction (`FEEDBACK-LOG.md`, 2026-08-18). These inform Phase 10's agent list so
it reflects what Phases 1–9 actually taught us rather than day-one guesses.

**These are notes, not authorisation.** The loop does not pull Phase 10 forward to service them, and
does not promote them.

_Iteration 1 (embedded UI): none found. Serving static assets is deterministic transport work with
no judgement in it, no natural-language content, and nothing that mutates a vocabulary — so there is
no candidate seam here either. Recorded as a genuine nil return rather than padded._

_Iteration 2 (layered configuration): none found. Parsing a two-key TOML file is deterministic, and
an LLM guessing what a misspelled key "probably meant" is the opposite of what this change is for —
the value here comes from refusing to interpret. Configuration mutates no vocabulary, so there is no
candidate seam. One adjacent observation worth carrying to Phase 10 rather than a proposal in itself:
`Source` is a small, concrete instance of the provenance shape that `adr/0002` requires on every LLM
proposal — "which layer produced this value" and "which model, prompt version, and inputs produced
this suggestion" are the same question, and the Phase 10 proposal model should not invent a second
vocabulary for it._

_Iteration 4 (UI test runner): none found, and this one is worth being blunt about. Writing tests is
something an LLM is good at, but that is a fact about **our** development process, not a product
capability — Phase 10's agent list is for things an OpenBiz **user** would invoke, and no taxonomist
asks their vocabulary tool to generate Vitest specs. The mutation sweep points the other way if
anything: the one mutant that survived the first draft survived because the test's `fetch` stub was
subtly unfaithful to the real API's abort semantics, which is exactly the class of plausible-looking
wrongness an LLM produces most readily and a human reviewer least reliably catches. Whatever Phase
10 builds, it should assume its output needs an adversarial check that is not itself generated._

_Iteration 5 (named-graph model): none in the built path, and one worth writing down for later. The
model itself is rules — a reserved namespace, a derived IRI, a writability check — and every one of
them must be deterministic and explainable, so an LLM anywhere in it would be a liability. The
opportunity is one layer up and belongs to Phase 12 rather than Phase 10's agent list: the reason
`create_vocabulary_graph` refuses an IRI that is already registered is that IRI collision is the
**only** duplication the store can detect, and it is the least interesting kind. Two vocabularies
that overlap by 80% of their concepts under entirely different IRIs are invisible to it, and that is
the actual silo. Lexical and structural matching is the no-LLM baseline `adr/0003` already requires;
what an LLM adds is recall on near-synonyms and definitional overlap that string matching misses.
Recording it here so the Phase 12 overlap report is not designed as if IRI equality were the
problem.

_Iteration 6 (graph registry over HTTP and in the UI): **one, and it is a real one.** The interface
now lists vocabularies **by raw IRI**, because that is all a graph has until Phase 2 gives it SKOS
labels — `http://example.org/v/animals` and nothing else. At three vocabularies that is merely ugly;
at the two hundred an enterprise actually has, "which of these is the one I want?" becomes
unanswerable from the list, and the user's rational response is to create a two-hundred-and-first.
That is §1.7's silo generator arriving through a UI affordance rather than a missing feature.
The manual path is a curator writing `dcterms:description` on each vocabulary, and it must stay the
path — but it is exactly the tedious editorial backlog nobody completes for vocabularies that
already exist. **The candidate:** an agent that reads a vocabulary's top concepts and drafts a
one-line "what this covers" summary, emitted as a proposal a curator accepts, edits, or rejects,
never written directly. Provenance is straightforward (the graph is the input, and it is already
named). This is a strong Phase 10 candidate precisely because it makes an **existing** vocabulary
findable rather than making a new one easier to write, which is the direction §1.7 wants assistance
to push. Worth pairing with the Phase 2 discovery work, since the same summary is what a discovery
result needs to show. Note the degradation is clean: with `NullProvider` the list shows IRIs and
whatever descriptions a human wrote, which is exactly today's behaviour._
