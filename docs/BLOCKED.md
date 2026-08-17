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

_None._

## Resolved

_None._
