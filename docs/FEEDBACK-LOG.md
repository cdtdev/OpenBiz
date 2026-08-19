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

---

## 2026-08-19 — from the product owner: you are on a wall clock, and nobody told you

**Information you have been missing.** Every iteration runs under a hard timeout and is **killed**
when it expires — currently **60 minutes**, rising to 90 when the loop is next restarted. There is
no warning and no grace: the process is terminated mid-work, the branch is left uncommitted, and the
next iteration inherits the mess. This has now cost three iterations (7, 10, and one earlier), which
is roughly three hours of nothing.

That was my omission, not your error. Act on it:

1. **Budget the iteration.** Aim to land — committed, PR opened, checks watched, merged — within
   about **45 minutes**. Treat the remainder as reserve for CI, which currently runs 10–16 minutes
   on its own and is *inside* your budget, not outside it.

2. **Split by cost, not just by scope.** The driver already tells you to split an item that is
   bigger than it reads. Add this: split an item that is *slower* than it reads. The Oxigraph scale
   spike is the clearest case — "10k / 100k / 1M concepts" is three items wearing one hat, and the
   1M leg alone may exceed a whole iteration. Do the small legs, land them with real numbers, and
   leave the expensive leg as its own `- [ ]`. Partial measurements that are committed beat complete
   measurements that were killed.

3. **Checkpoint long-running work.** When an item is dominated by measurement rather than coding,
   write results into the ADR **as each one completes**, and commit. A benchmark that dies at minute
   58 with nothing on disk has produced nothing; the same benchmark writing each result as it lands
   has produced most of its value. This applies to any item where the machine is working and you are
   waiting.

4. **If you genuinely cannot fit an item even split** — say so in `BLOCKED.md` with the measured
   reason, rather than starting it each iteration and being killed each time. A repeating silent
   failure is far worse than a recorded one.

**Do not let this compromise the work itself.** `CLAUDE.md` §4 still governs: no weakened
assertions, no unearned checkmarks, no "I ran out of time so I claimed it." If the honest outcome of
an iteration is one small landed item plus a recorded reason the rest did not fit, that is a
successful iteration. Speed never buys a dishonest green.

---

## 2026-08-19 — from the product owner (drained at iteration 27)

> ## 2026-08-19 — correction from the product owner: the README contradicts our own research
>
> `README.md` line 58 still states, under a heading that reads **"Stated plainly"**:
>
> > there is no OWL 2 **DL** reasoner in the Rust ecosystem
>
> Your own product-owner pass (iteration 25, `docs/COMPETITIVE.md`) retired that claim as too strong:
> `rustdl` is Apache-2.0 and actively developed, and publishes `owl-dl-saturation`. You corrected the
> research file and left the public claim standing.
>
> **Fix the README to match.** The *practical* conclusion has not changed and should not be softened —
> EL + RL remains our target, because nothing in that space is near load-bearing. What must go is the
> absolute existence claim. Say what is true: no Rust OWL 2 DL reasoner is mature enough for us to
> depend on, name the gap against Protégé with HermiT, and keep the honesty that makes that paragraph
> worth having.
>
> **The mechanism, which matters more than the line.** A correction recorded in a research document is
> not applied until every place that repeats the claim is updated. `CLAUDE.md` §4 makes misreporting
> worse than lacking, the repository is public, and `/openbiz-status`'s degradation watch treats
> "README claims something `BUILD-PLAN` or the research does not support" as charter drift for exactly
> this reason. **When a pass retires a claim, grep the repo for it and fix every instance in the same
> iteration** — README, CLAUDE.md, ADRs, and docs.
>
> While you are there: `CLAUDE.md` §5 still offers `horned-owl` as an example of a dependency whose
> licence might be *merely unlisted*. It is on the forbidden list, as your own research established.
> That example is now actively misleading and should be replaced — the `horned-owl` decision itself
> stays blocked and is not yours to take.

**What the loop did (iteration 27, 2026-08-19).** Took this as the whole iteration rather than a
Phase 2 item, because it is a false claim standing in public on a repository whose pitch is honesty.

*The line.* `README.md`'s "Stated plainly" paragraph now reads that **no Rust OWL 2 DL reasoner is
mature enough for us to depend on**, names `rustdl` (Apache-2.0, MaastrichtU-IDS) as the work that
exists, keeps EL + RL as the target and the Protégé-with-HermiT gap unsoftened, and points at
`docs/COMPETITIVE.md` for the survey. The practical conclusion was not weakened, as instructed.

*Every other instance, per the mechanism.* Grepped for `no OWL 2 DL reasoner`, `no DL reasoner`,
`DL reasoner`, `HermiT` and `horned-owl` across `.md`, `.rs`, `.ts`, `.tsx`. Five live places said
it; all five are corrected — `README.md`; `CLAUDE.md` §3's candidate list; `crates/openbiz-owl/src/lib.rs`
module docs and the `Profile::Dl` doc comment ("Not currently supported by any Rust reasoner", the
strongest phrasing in the repo and the one furthest from the research); and `COMPETITIVE.md`'s own
superseded conclusion, which is annotated in place rather than edited because that file leaves prior
findings standing on purpose. `BUILD-PLAN.md`'s Phase 5 "document the DL gap" item now carries a
note to start from the corrected wording. `LOOP-LOG.md` and this file are append-only history and
were left alone.

*`horned-owl` in §5.* The example is replaced: §5's "merely unlisted licence" branch now cites
Oxigraph and its transitive tree, and `horned-owl` appears instead as the **worked example of the
other branch** — the copyleft one that goes to `BLOCKED.md` and stops. §3's crate map no longer
names it as the OWL 2 dependency, and §3's candidate list strikes it through with the reason. The
decision itself was not taken; it remains blocked.

*The mechanism, written down.* `COMPETITIVE.md` gained a second file rule — retiring a claim there
does not retire it anywhere else, so grep the repo and fix every instance in the same iteration —
and a **retired-claims table** recording each retired claim, what is true instead, and which files
were corrected. Two rows so far: this claim and the `horned-owl` candidacy.

*What was declined, and why.* The loop did **not** build a CI check to enforce the rule. That is a
new repo-wide gate off the back of a one-paragraph correction, and the instruction was to follow the
mechanism, not to build one. The design is in `docs/PROPOSED.md` for a human to promote, and
`docs/UNTESTED.md` records the honest gap in the meantime: this sweep was manual, its completeness
rests on the phrasings the loop thought to search for, and nothing prevents a recurrence.

## Received 2026-08-19 (iteration 37)


## 2026-08-19 — from the product owner: you can go and get a real vocabulary

Five consecutive iterations have recorded the same doubt — note-property placement, mapping density,
exact-match cluster size, unchecked-condition frequency, and now downward-walk cost — and you named
it yourself at iteration 34 as "the same blind spot for the fourth time." Every one reduces to: *no
fixture in this repository resembles a real thesaurus.*

**The framing to correct.** You have written "I cannot find out from inside this repository" more
than once. That is true and it is not the constraint you think it is. `CLAUDE.md` §8 puts real IdPs,
paid accounts, releases, pricing, and hardware-bound load testing out of scope. **Public test data
is not on that list.** EuroVoc, AGROVOC, LCSH, and the Getty vocabularies are published, freely
downloadable, real ISO 25964-shaped thesauri in the tens to hundreds of thousands of concepts. Using
one as a fixture is ordinary engineering, not a scope decision, and it would answer four or five of
your open questions with measurements instead of caveats.

**What I am asking for is a proposal, not a fetch.** Do not download half a gigabyte into the repo
on the strength of this note. Evaluate it properly and write it up in `docs/PROPOSED.md`:

- **Licence.** Each source has its own terms and §5 governs what enters this repository. EuroVoc and
  AGROVOC are permissively licensed as far as I know, but *as far as I know* is not a licence check.
- **Size and where it lives.** A real thesaurus should almost certainly not be committed. A
  fetch-on-demand fixture with a checksum, cached outside the tree and skipped when absent, keeps CI
  hermetic and the clone small — and the loop machine is at 26 GB free, so size is not free.
- **Which questions it actually answers.** Name them against the `UNTESTED.md` entries they would
  close. If a source answers only one, say so — that changes whether it is worth the machinery.
- **Air-gapped honesty.** A test that needs the network is a test that fails in the deployments we
  claim to serve. Say how it degrades.

If after that the answer is "not worth it yet", record that with the reasoning and stop writing the
doubt as though it were unanswerable. An open question you have decided not to answer is a different
thing from one you cannot.

**What the loop did (iteration 37, 2026-08-19).** Took this as the whole iteration. Nothing was
downloaded into the repository and nothing was built; the deliverable is the proposal
*"Adopt a fetch-on-demand real-thesaurus fixture, and stop calling the shape questions
unanswerable"* in `PROPOSED.md`, which answers all four criteria with measurements rather than
recollection.

Four things in the note came back different from how it was written, and they are recorded here
because a human made a decision on each premise:

- **EuroVoc does not pass a licence check.** Not because it is closed — it is probably CC BY 4.0
  under Decision 2011/833/EU — but because the Publications Office's own copyright page licenses
  *"the editorial content of this website"* and then routes the specialised databases (CELLAR, EU
  Vocabularies) to `op-copyright@publications.europa.eu`. Only secondary sources say CC BY, which is
  the *"as far as I know is not a licence check"* standard this note itself set. **AGROVOC does
  pass**, and by the strongest available evidence: `dct:license <…/by/4.0/>` in the dataset's own
  VoID descriptor. LC's data is public domain by the publisher's own statement. Getty is ODC-BY 1.0.
- **The disk figure is off by placement, not by amount.** C: is at 26 GB free and holds the loop
  *state* directory; the repo is on G: with 355 GB free and the build cache, `~/.cache` and `/tmp`
  are on the WSL ext4 root with **929 GB free**. The constraint is real but it is about *where* the
  cache goes, not whether it fits.
- **"Fetch-on-demand with a checksum" has a problem neither of us saw.** Neither AGROVOC nor LC
  publishes an immutable URL: AGROVOC serves only `.../latestAgrovoc/…` and keeps older releases
  behind an email request; LC regenerates every dump daily. A pinned checksum therefore goes stale
  on the publisher's schedule. And LC's own published SHA-1, which looks like the answer, is
  attached to the `.gz` URI but is actually the hash of the *decompressed* bytes — verified by
  fetching the file twice and hashing both forms.
- **The recommendation is smaller than the ask, and the note's own test is the reason.** *"If a
  source answers only one, say so"* — so: LCGFT (745 KiB, public domain, 25.8% polyhierarchy) is
  recommended and AGROVOC (70 MiB, moving URL, mixed-provenance multilingual content) is deliberately
  **not**, as a separate human decision.

The framing correction is accepted without reservation and its effect is already in the ledger:
three `UNTESTED.md` entries have had *"it cannot be told from inside this repository"* replaced by
measurements, and one of them — `PathBound::DEFAULT` — turned out to be wrong in its direction, not
merely unproven. Roughly forty minutes of `curl` and a public SPARQL endpoint did what six
iterations had called unanswerable.
