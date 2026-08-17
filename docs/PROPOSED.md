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
- **Status:** proposed.
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

### Headless-browser smoke test against the release binary
- **Status:** proposed.
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
