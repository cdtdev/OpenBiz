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

_No iterations yet. Phase 0 is being laid by hand before the loop takes over._
