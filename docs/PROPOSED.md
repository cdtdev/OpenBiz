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

_None yet — Phase 0 has not completed its first iteration._
