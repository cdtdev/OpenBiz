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

## Resolved

_None._
