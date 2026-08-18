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

---

## 2026-08-18 — from the product owner (second entry, standing direction)

> ## 2026-08-18 — from the product owner (STANDING DIRECTION, not a one-off)
>
> **Parity is failure.** The goal is not to do what PoolParty, metaphactory, TopBraid EDG, Protégé,
> and VocBench do. It is to do it *better*. Matching them produces a worse copy, because you also
> inherit their framing of the problem.
>
> Make this operational, not aspirational:
>
> 1. **For every item, ask two questions before you build.** Not "does the incumbent have this" but
>    *"what do they do badly here, and what would be materially better?"* Write the answer into the
>    item or the commit. If the honest answer is "we can only match", say so explicitly in
>    `docs/PROPOSED.md` rather than shipping parity quietly — that is a finding worth a human's
>    attention, not a failure to hide.
>
> 2. **Beware parity creep.** Working a competitor's feature list as a checklist is the specific
>    failure mode. It feels productive and produces a second-rate imitation. The question is always
>    what the *user* is trying to accomplish, and whether we can serve that better — sometimes by
>    building something the incumbents do not have at all.
>
> 3. **Where "better" is concretely reachable** — these are the fronts, and they are already in the
>    charter's wedge table:
>    - They *show* an inference; we **explain the derivation** to someone who is not a logician.
>    - They *flag* a validation error; we **name the offending concepts and offer the fix**.
>    - Their diffs are triples; ours are **meaning** — "3 concepts added, 1 relabelled, 1 deprecated".
>    - They make creating a vocabulary easy; we make **reuse easier than creation**.
>    - They assume you already know how to build an ontology; we **guide you**, and route you away
>      from building the wrong artifact entirely.
>    - They need training and consultants; we need **neither**.
>    - They are JVM-heavy; we are **one fast binary**.
>
> 4. **This does not license scope creep or shortcuts.** Better means better on the thing the item is
>    already about. It never means adding unrequested features, and it never overrides the
>    non-negotiables in `CLAUDE.md` §1 or the definition of done in §4. A beautifully-conceived
>    feature with a dishonest test is worse than an ordinary one with an honest gap recorded.
>
> Carry this into the every-25th-iteration product-owner pass: re-read the wedge table and ask, per
> row, whether what we have actually built is *better* yet — or merely present.

**Processed:** iteration 2.

**Action taken:** this is standing direction, so it becomes a rule the loop re-reads rather than a
backlog item.

1. Written into `BUILD-PLAN.md`'s "How to work this plan" as a standing instruction: before building
   any item, answer *"what do the incumbents do badly here, and what would be materially better?"*
   in the commit or the item, and record an honest "we can only match here" in `PROPOSED.md` rather
   than shipping parity silently.
2. Added the per-row wedge-table audit to the every-25th-iteration product-owner pass instruction in
   the same file, so the review the feedback asks for has a place to happen.
3. Applied to **this** iteration's item (file config) — see the "better than parity" note in
   `PROPOSED.md` and the loop log.

**No charter conflict.** It sharpens the wedge table in `CLAUDE.md` §1 rather than contending with
it, and §4's clause 4 is explicitly preserved.

## 2026-08-18 — repository made public; branch protection active (drained iteration 3)

Arrived mid-iteration 3, after that iteration's drain. Logged and acted on at the end of the
same iteration because it is ledger bookkeeping plus a standing direction, not build work.

Verbatim:

> 
> ## 2026-08-18 — from the product owner
> 
> **The repository is now PUBLIC, and branch protection is active.** Two consequences to act on:
> 
> 1. **Close the branch-protection blocker.** The `main-protection` ruleset is live: `Rust`,
>    `Licence policy`, `UI`, and `Single binary` are required status checks, force-push and deletion
>    of `main` are blocked, and bypass is disabled for everyone including the owner. Move the
>    "Branch protection on `main`" entry in `docs/BLOCKED.md` to Resolved, check off the corresponding
>    Phase 0 item in `docs/BUILD-PLAN.md`, and update the `**Status:**` line — **Phase 0 is now
>    complete with no open items.** Note in the entry that it was unblocked by making the repo public
>    rather than by upgrading the plan.
> 
> 2. **Everything you write is now published the moment it lands.** This makes the charter's "the
>    roadmap is the repo" claim literally true — the backlog, the ADRs, and the honest gaps in
>    `UNTESTED.md` are the differentiator against incumbents whose roadmaps are invisible, so keep
>    writing them exactly as candidly as you have been. Do not start softening `UNTESTED.md` or
>    `COMPETITIVE.md` because strangers can read them; the honesty *is* the product claim.
> 
>    Two things that now matter more: never commit a secret, credential, or token — there is no
>    private window to catch it in. And `README.md` is now a public shop front, so the rule in
>    `CLAUDE.md` §4 against claiming unearned support is no longer an internal discipline but a public
>    one. Check it still matches the plan before any iteration that touches it.

## Drained 2026-08-18 (iteration 5)


## 2026-08-18 — correction from the product owner

`docs/BUILD-PLAN.md` currently states **"Phase 0 is complete — no open items"** while Phase 0 still
contains an unchecked item: *"UI test runner (Vitest + Testing Library) with a test per `Probe`
state, wired into CI"* — promoted from `PROPOSED.md` in the same iteration that closed branch
protection. Fix the `**Status:**` and `**Current position:**` lines to match reality.

The mechanism to fix, not just the line: you closed the last *original* Phase 0 item and concluded
the phase was done, without re-reading the phase after the promotion you had applied minutes
earlier. **Derive that claim by counting `- [ ]` in the phase, never from memory of what was left.**

Why this is worth an explicit correction over a one-line typo: `CLAUDE.md` §4 says misreporting
support is worse than lacking it, and the repository is now public — a plan that declares a phase
complete while an item sits open is precisely the "roadmap you cannot trust" failure we are
attacking the incumbents for. It also feeds the degradation watch's charter-drift signal. This is
small now and corrosive if it becomes habit.

Note this in `LOOP-LOG.md` as a process error caught externally, not just a doc fix.
