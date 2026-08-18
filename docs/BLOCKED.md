# Blocked

Work that cannot proceed, and **precisely** what would unblock it.

An entry here must name the unblocking action specifically enough that a human can act on it in one
sitting. "Needs infrastructure" is not a blocker entry; "needs an Okta tenant with SAML configured,
to verify the assertion parsing in `openbiz-server/src/auth/saml.rs`" is.

**Do not work around a blocker by lowering a requirement.** If an item is blocked, record it and
take the next item. Silently substituting a weaker implementation is how a charter erodes.

## Entry format

```
### <blocked item> (Phase N)
- **Blocked on:** the specific missing thing.
- **Unblocked by:** the specific action a human would take.
- **Tried:** what you attempted before concluding it was blocked.
- **Workaround in place:** none | <what, and what it costs>
- **Opened:** iteration N
```

---

## Open

### OWL 2 model and IO via `horned-owl` (Phase 9)
- **Blocked on:** **`horned-owl` is LGPL-3.0**, and `CLAUDE.md` §5 forbids LGPL in the core without
  qualification. There is no permissive dual-licence option. This is not a spike that failed and not
  a maturity judgement — the crate is healthy and actively released. It is a licence wall, and §5
  says a copyleft dependency is recorded here and **stopped on**, because it is a commercial
  decision a human takes.
- **Why it is worse than an ordinary licence mismatch, stated plainly:** LGPL's relinking obligation
  is usually satisfied by dynamic linking. Rust statically links, and `CLAUDE.md` §1.2 commits us to
  **one binary**. A statically-linked LGPL dependency puts the obligation on the whole executable —
  the conventional discharge is shipping object files or an equivalent that lets a user relink
  against a modified `horned-owl`. That collides with §5's other requirement, that the core stay
  **cleanly relicensable** for a separately-licensed enterprise layer. So the two non-negotiables
  this touches are §1.2 and §5 at the same time, which is exactly why the loop must not decide it.
- **Unblocked by:** a human choosing one of four, and recording it in an ADR:
  1. **Accept LGPL-3.0 for `openbiz-owl` only**, isolated in its own crate the way §5 already
     permits for MPL-2.0, and accept the static-linking obligation for the shipped binary. Needs
     legal input on whether the open-core plan survives it. This is the cheapest engineering path
     and the most expensive commercial one.
  2. **Write our own OWL 2 structural model and IO.** Large — the OWL 2 Structural Specification is
     roughly 60 axiom types, plus RDF/XML and Functional-Syntax mapping in both directions — but it
     is a *known* quantity, it is squarely inside §3's "engine dependencies sit behind our own
     trait" discipline (there would be no engine), and Phase 9 needs the boundary anyway.
  3. **Adopt `owlish`** (MIT OR Apache-2.0) as the model and accept it is three years stale
     (last publish 2023-07-05), which in practice means adopting an unmaintained crate and
     maintaining it ourselves. Permissive, and smaller than option 2, but the staleness is the whole
     risk and it should be measured against a real ontology before anyone believes it.
  4. **Ask upstream for a permissive relicence or a dual licence.** Free to ask, slow, and outside
     our control; not a plan on its own, but it costs nothing to send alongside 1–3.
- **Tried:** verification, not implementation — nothing was built. `horned-owl`'s licence was
  confirmed three independent ways on 2026-08-18: crates.io metadata for `3.0.0`, the `license`
  field in the upstream `Cargo.toml`, and `COPYING` + `COPYING.lesser` (GPLv3 + LGPLv3) at the
  repository root. The permissive alternatives were then enumerated from a crates.io search and each
  one's licence, publication date, and download count read from the API rather than recalled.
- **Workaround in place:** none, and none is needed **yet** — Phase 9 is six phases away and nothing
  in the tree depends on `horned-owl`. That is the good news in this entry and the reason it is a
  decision rather than an emergency. It is filed now precisely so the decision is not discovered on
  the first day of Phase 9 with an implementation plan already written against it.
- **Also affects the charter text itself.** `CLAUDE.md` §3 lists `horned-owl` as the OWL 2
  candidate, §5 offers it as the example of a dependency whose licence is "merely *unlisted* rather
  than forbidden", and `docs/BUILD-PLAN.md` names it in the Phase 9 item. All three are now wrong
  and cannot be corrected by the loop, because correcting them means choosing between the options
  above. `docs/PROPOSED.md` carries the amendment.
- **Opened:** iteration 25 (product-owner pass)

### Candidate seam, part 3 — over HTTP and in the interface (Phase 2)
- **Blocked on:** there is no authentication. `POST /api/candidates` and an approve endpoint are
  an unauthenticated "apply this arbitrary change to a customer's vocabulary" until an identity
  sits behind them, and the seam's whole value is that an approval is recorded against a person.
- **Unblocked by:** Phase 6's authentication item landing — specifically, a request-scoped actor
  the handler can pass to `Store::decide` in place of today's `OPENBIZ_ACTOR` environment
  variable, and a way to refuse an unauthenticated caller. It does not need an enterprise IdP;
  local accounts are enough to unblock this entry.
- **Tried:** nothing. It was split out at iteration 17 *because* it is blocked, so that the two
  halves that are not blocked could land. The reasoning is recorded on the item in
  `docs/BUILD-PLAN.md` and has not changed.
- **Workaround in place:** the command line. `openbiz import`, `retract`, `candidates`,
  `candidate <id>`, `approve`, `reject` are complete, so a deployment can use the seam today.
  **The cost is real:** a reviewer has to be on the server's console, which rules out the
  distributed review a governance function actually runs on.
- **Opened:** iteration 17 · **recorded here at iteration 21**, which is late — it was named on
  the plan item from the start but never entered this file, so the loop's own "do not re-attempt
  something already recorded as blocked" check could not see it.

## Resolved

### ~~Branch protection on `main`~~ (Phase 0) — RESOLVED 2026-08-18
- **Unblocked by:** the product owner making the repository **public**, which is one of the two
  actions this entry named. Protected branches are unavailable on private repositories under the
  free plan; they are available on public ones. Note that it was unblocked by taking the commercial
  decision the entry deferred to a human — *not* by weakening the requirement, and not by the loop
  finding a way around it. That distinction is the whole reason the entry existed.
- **Now in force,** verified against the API rather than taken on trust: ruleset `main-protection`,
  `enforcement: active`, targeting `main`, with rules `required_status_checks`, `non_fast_forward`,
  and `deletion`. The required contexts are `Rust`, `Licence policy`, `UI`, and `Single binary` —
  all four of CI's jobs. `bypass_actors` is empty, so the rule binds the owner too.
- **What changes for the loop:** merging red is now refused by the server, not merely by the loop's
  own discipline. The `gh pr checks --watch --fail-fast` step in the iteration driver **stays** —
  it is how the loop finds out a check failed in time to fix it on the branch, and it is what keeps
  the loop from sitting on an unmergeable PR. What has gone is the silent-failure mode where
  skipping that step would have merged failing code anyway.
- **Cost that remains:** `gh pr merge --auto` is now usable in principle, since a merge requirement
  finally exists. The loop has not switched to it and should not without a reason — watching the
  checks gives the loop the failure output, which auto-merge does not.
- **Opened:** Phase 0 hand-build (pre-iteration-1) · **Resolved:** iteration 3

<details>
<summary>The original entry, kept for the record</summary>

### Branch protection on `main` (Phase 0)
- **Blocked on:** GitHub returns `403 Upgrade to GitHub Pro or make this repository public` for both
  the rulesets API and classic branch protection. Protected branches are not available on private
  repositories under the free plan.
- **Unblocked by:** a human either (a) upgrading `cdtdev` to GitHub Pro, or (b) deciding the repo
  should be public — plausible given the Apache-2.0 open-core model in `CLAUDE.md` §5, but that is a
  commercial decision and explicitly out of loop scope (§8).
- **Tried:** `POST /repos/cdtdev/OpenBiz/rulesets` with required status checks for the `Rust`,
  `Licence policy`, and `UI` jobs. Rejected 403.
- **Workaround in place:** the loop runs `gh pr checks --watch --fail-fast` before merging, so it
  refuses to merge a red PR by its own discipline. **The cost is real:** this is a convention the
  loop follows, not a rule the server enforces. Nothing prevents a merge of failing code if the
  loop skips that step, and `gh pr merge --auto` cannot be used at all — auto-merge only arms when
  a merge requirement exists. `/openbiz-status` Section C exists partly to catch this.
- **Opened:** Phase 0 hand-build (pre-iteration-1)

</details>
