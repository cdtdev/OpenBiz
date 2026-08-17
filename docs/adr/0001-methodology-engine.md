# ADR 0001 — Methodology engine, packs, and gates as SHACL

**Status:** accepted (2026-08-18) · **Phase:** 7

## Context

Practitioners need to know where they are in a development lifecycle and what to do next, and
newcomers need routing to the right artifact type before they start. The field has several
established methodologies — Noy & McGuinness 101, METHONTOLOGY, NeOn, LOT, SAMOD — plus the
standards-derived workflows of ISO 25964 and ANSI/NISO Z39.19. They disagree in shape: NeOn is
explicitly scenario-based with no fixed lifecycle, SAMOD is a short iterative loop with milestones,
METHONTOLOGY is staged. See `docs/METHODOLOGY.md`.

Hard-coding one lifecycle would pick a fight with every organisation that uses another, and would
make us wrong for most artifact types.

## Decision

**1. Methodologies are declarative data, not code.** A *methodology pack* defines phases,
activities, roles, deliverables, and gate criteria. Packs are RDF, versioned in git, forkable, and
customisable by the customer. Shipping a pack is content work, not a release.

**2. The pack model is a scenario graph, not a linear phase list.** Forced by NeOn, whose nine
scenarios branch and recombine. A linear model cannot express it, and NeOn is our enterprise default
because it is the only mainstream methodology organised around reuse — the same posture as
`adr/0003`.

**3. Gate criteria are SHACL shapes.** Exit criteria are constraints over the project graph, which
is what SHACL already expresses. Consequences: no bespoke rule language; gates are as inspectable
and customisable as any other rule pack; a failing gate names the *specific offending concepts*
rather than reporting an opaque "not ready". This makes Phase 7 depend on Phase 4.

**4. Gates guide, they do not block.** Any gate is overridable by an authorised role **with a
recorded reason**, surfaced in the audit trail and the Compass. Enterprises reject tools that block
them; governance teams need deviations logged, not prevented. A project running on overridden gates
must not be able to present itself as compliant.

**5. The Solution Advisor is separate from the Engine and runs first.** It routes a stated problem
to an artifact type and pack, and emits a **Solution Brief** — a versioned, revisable record of the
answers, the recommendation, the reasoning, and the rejected alternatives. Its first action is a
discovery sweep (`adr/0003`), and it must be able to recommend building nothing.

**6. Escalation between artifact types is guided migration, never one-click.**
Authority list → taxonomy → thesaurus is largely mechanical. **Thesaurus → ontology is not, and we
will not automate it.** A `skos:Concept` is an individual; an `owl:Class` is a set. Mechanically
rewriting one into the other is the most common modelling error in this field, and a tool that
offers the button is responsible for the mistake.

## Consequences

- New crate `openbiz-lifecycle`: pack model, project state, gate evaluation, Advisor.
- Depends on Phase 4 (SHACL) and Phase 6 (roles, workflow states, audit).
- Pack authoring needs its own editing surface eventually; until then packs are hand-written Turtle.
- Every pack must cite its source, and mark any step that is an OpenBiz addition rather than
  misattributing it to the published methodology.
- **Risk:** the pack model could grow into a general workflow engine. It must stay scoped to
  knowledge-organisation lifecycles. If a pack needs arbitrary code execution, that is a signal we
  modelled the wrong thing — raise it rather than adding a scripting hook.
