# Feedback log

Human direction that entered the loop through `~/.claude/openbiz/feedback.md`, dated and kept
verbatim. Feedback is the one input that may enter `BUILD-PLAN.md` without passing through
`PROPOSED.md` — a human already authorised it — so this file is the audit trail for why plan items
exist that the loop did not propose.

Each entry records what was received, what the loop did about it, and anything it declined to do
and why. The inbox is truncated after processing so the same feedback is never acted on twice.

---

## 2026-08-18 — from the product owner

> Incorporate LLMs wherever they materially improve the experience or the outcome. Three specific
> instructions, in priority order:
>
> 1. **Build the assistability seam now.** `CLAUDE.md` §3 "Design for assistability" was added for
>    this. Every path that changes a vocabulary must take *candidates* carrying provenance, source,
>    and confidence, reviewed before they land — the same shape for a CSV import, a discovery match,
>    a bulk edit, or a Phase 10 agent. Do this as you build Phase 2 and Phase 3, not as a retrofit.
>    This is the single highest-value thing you can do for LLM integration in the near term, and it
>    is interface shape, not new functionality.
>
> 2. **Record the opportunities as you go.** In every phase, when you notice a place where LLM
>    assistance would materially help — a tedious editorial task, a judgement that needs recall
>    across thousands of concepts, a translation, a mapping, a definition to draft — write it to
>    `docs/PROPOSED.md` with the concrete user problem it solves. By the time Phase 10 arrives, its
>    agent list should be informed by everything you learned building Phases 1–9, not by the guesses
>    made on day one. Do not promote these yourself.
>
> 3. **Do not pull Phase 10 forward.** Building `LlmProvider` or the shim before there is a caller
>    is exactly the "built but no production caller" failure in `CLAUDE.md` §4, and the approval path
>    agents need does not exist until Phase 6. Resist it. The constraint that nothing core may
>    require an LLM (§1.6) stands regardless.
>
> Expect excellent progress. Prioritise depth and honesty over checkbox count — a phase done properly
> with its gaps recorded is worth more than three phases half-done and reported green.

**Processed:** iteration 1.

**Action taken:**

1. **Candidate seam** — added to `BUILD-PLAN.md` as the **first** item of Phase 2, ahead of the SKOS
   core model, with a note that the mutation items below it depend on its existing first. Placed
   first deliberately: the instruction is explicitly "not as a retrofit", and every item after it in
   Phase 2 mutates a vocabulary. Not started this iteration — Phase 0 is not finished and phases are
   ordered by dependency (`CLAUDE.md` §3, the plan's own preamble).
2. **Record opportunities as you go** — written into `BUILD-PLAN.md`'s "How to work this plan" as a
   standing instruction, and an "LLM assistance opportunities" section opened in `PROPOSED.md`. Made
   standing rather than a one-off item because it is a per-phase habit, not a task that completes.
3. **Do not pull Phase 10 forward** — no action needed; it agrees with `CLAUDE.md` §4 and with the
   plan's phase ordering. Recorded here so a later iteration tempted by it finds the ruling.

**No charter conflict.** All three instructions restate or sharpen `CLAUDE.md` §1.6, §3, and §4
rather than contending with them.

**Honest note on this iteration's contribution to it:** iteration 1 built the embedded-UI item,
which mutates nothing and therefore had no candidate seam to build and surfaced no genuine LLM
opportunity. Recorded as "none found" in `PROPOSED.md` rather than inventing one to look responsive.
