# 0051 — Colour is arithmetic over the file that ships

**Date:** 2026-08-20 (NZST, UTC+12)
**Status:** accepted
**Implements:** `CLAUDE.md` §3 (WCAG 2.2 AA as a conformance target), §4 (a real production caller),
the Phase 3 design-system item.

## Context

`CLAUDE.md` names a *"dated, dense, joyless UI"* as one of the incumbents' structural weaknesses and
answers it with *"visually stunning and modern. Design is a feature, not a coat of paint."* The
Phase 3 item that follows from that asks for a design system with **"colour with verified
contrast"**.

"Verified" is the whole difficulty. Every design system in the world claims AA. The claim is
normally made once, by a person, against a palette in a design tool, and it survives exactly until
someone nudges a grey. Nothing in a repository usually knows the difference between a palette that
meets 4.5:1 and one that used to.

Before this iteration the interface had **no stylesheet at all** — unstyled browser defaults, with
`ui/src/App.tsx` and `Vocabularies.tsx` emitting bare `<p>` and `<ul>`.

## Decision 1 — the CSS is the source of truth, and the test reads it

Tokens live in `ui/src/design/tokens.css` as custom properties. The test reads that file off disk,
parses it, and computes over the values.

**The alternative was to declare tokens in TypeScript and generate the CSS.** That is the more usual
arrangement and it is the wrong one here: it lets the suite verify a source of truth the browser
never loads. Two artefacts drift, and the one that drifts silently is always the generated one.

`ui/src/design/tokens.ts` is therefore a small, deliberately narrow reader — custom-property
declarations, `:root`, and the one `@media (prefers-color-scheme: dark)` block — that **refuses**
what it does not understand instead of skipping it. A parser that silently finds nothing turns every
assertion downstream into a test that passed because it checked nothing.

**What was measured, and it changed the implementation.** The obvious way to read the file is
`import css from "./tokens.css?raw"`. It returns the **empty string**: Vitest stubs CSS imports out.
A probe test printed `LENGTH 0`. Had that shipped, every colour assertion below would have iterated
an empty token set and reported green. Reading the bytes with `node:fs` is worth one dev-only
dependency (`@types/node`, MIT) to avoid precisely that.

## Decision 2 — two layers, and the split is load-bearing

1. `--palette-*` — raw sRGB hex, the only literal colours in the codebase, meaningless on their own.
2. semantic roles — written **only** as `var(--palette-…)`.

A rule names a role, never a swatch, so the dark theme is a re-pointing of layer 2 and no component
knows. Both halves are enforced: a palette entry that is not hex fails, and a role containing a `#`
fails.

Hex, not `oklch()`. `oklch()` is a nicer authoring space, but the contrast check computes sRGB
relative luminance from these exact bytes, and a notation it had to approximate would turn a
verified number into a plausible one. Translucent colours are **refused** for the same reason: a
foreground with alpha has no single contrast ratio, so accepting one would let a token pass a check
that means nothing.

## Decision 3 — the pairings are derived from the names, not from a list

One naming rule:

```
--color-<base>                    a background
--color-[<qualifier>-]on-<base>   text on it       — SC 1.4.3, >= 4.5:1
--border-on-<base>                a boundary on it — SC 1.4.11, >= 3:1
--focus-on-<base>                 a focus ring     — SC 1.4.11, >= 3:1
```

The check enumerates every token matching the pattern and looks up `--color-<base>`. **A list of
"legal pairings" kept beside the tokens was rejected**: a list is a second place to update, so a
surface added without a foreground — or a foreground naming a surface nobody defined — would pass a
check written against it. Here the same mistake is an unresolvable partner and the suite fails.

The naming rule is what makes the design system self-policing rather than self-describing.

## Decision 4 — three rules that keep it a system

Each is the mechanical form of something `CLAUDE.md` already requires.

- **No literal colour outside the palette layer**, in any stylesheet. Without it the tokens are a
  suggestion, and the first component in a hurry writes its own grey.
- **No palette swatch that no role names.** §4 counts a value nothing invokes as undone. A palette
  is where that rots invisibly, because nobody ever deletes a colour. This is why the neutral ramp
  has gaps at 200, 300, 700 and 800 — the numbering keeps its meaning, so they fill in unrenumbered.
- **No colour role that no rule uses.** The production-caller clause, mechanised.

The same reasoning is why there is no `info` role, no filled-accent surface, and no motion token:
nothing is yet informational, pressable, or animated.

**The type and spacing scales are the one exception, deliberately.** A scale is the closed set of
values a rule may choose from, and a scale with its unpopular steps removed is no longer a scale.
What is tested there instead are its invariants: ascending in declared order, every type step a
whole pixel at a 16px root, every space step on the 4px grid.

## Decision 5 — dark is layered over light, and only `--color-` roles must move

The dark block overrides only what differs. A token added to light and forgotten in dark still
*resolves* — to the light value — and the contrast check catches it there. A full independent second
set would turn the same slip into a **missing** token, which is easier to walk past.

A separate check requires every `--color-` role to be re-pointed in dark. It is deliberately not
extended to borders and focus rings: no colour clears 4.5:1 against both a near-white and a
near-black canvas, so an inherited *text* role cannot survive the arithmetic anyway, whereas 3:1
against both is achievable and the status borders share one value on purpose. Demanding they differ
would mean inventing a second near-identical swatch to satisfy a rule nobody could state.

## What this does not prove, and it is the larger half

The arithmetic is over **values**. Nothing here renders. jsdom evaluates no media query and no
cascade, so no test has seen the dark theme applied to anything, and the pairing check reads token
*names* rather than which background a rule actually draws a foreground on. That second gap is not
hypothetical: a zebra-striped list was written and then removed during this iteration precisely
because it put a surface foreground on the canvas colour, a pairing the convention cannot check.

Both are in `docs/UNTESTED.md`. The Phase 3 Playwright item is what closes the first.

## Mutation results

Every check was reverted in turn and the suite re-run, because a check that has never failed is a
check nobody has verified. All of these fail as they should: a border swatch lightened below 3:1
(1 failure), a dark override deleted (2), a swatch nudged out of ramp order (1), an unreferenced
swatch added (1), a colour role's only rule removed (1), a literal colour written into a rule (1),
the token `@import` commented out (1), the stylesheet import commented out (1), an off-pixel type
step (1), an off-grid space step (1), a type step that breaks the scale ratio (1), and the luminance
coefficients transposed (3).

**Two of those found real defects in the checks themselves**, which is the argument for running
them at all:

- the literal-colour scan used `/#[0-9a-fA-F]{3}\b/`. `#666666` ends on a word character, so the
  word boundary matched the three-digit form and **missed every six-digit one**. `color: #666666`
  passed.
- the two "is it actually imported?" assertions matched the source text without stripping comments,
  so commenting the import out left both passing.
