# Methodology — what OpenBiz offers its users

> **Scope.** This is about the development methodologies OpenBiz *provides to its users* for building
> taxonomies, thesauri, and ontologies. It is not about how OpenBiz itself is engineered — that is
> `CLAUDE.md`.

Two problems, and they are different:

1. **"Where am I and what do I do next?"** — a practitioner mid-project needs to know their position
   in a lifecycle, what remains in this phase, and what is blocking the next one.
2. **"What should I even be building?"** — a newcomer does not know whether they need a taxonomy, a
   thesaurus, or an ontology. Getting this wrong is the most expensive mistake in the field, and it
   is usually made on day one by someone who did not know a choice was being made.

Problem 1 is the **Methodology Engine**. Problem 2 is the **Solution Advisor**. The Advisor selects
a pack; the Engine runs it.

---

## Part 1 — the Solution Advisor (meta-methodology)

A structured diagnostic that routes a stated problem to an artifact type, a methodology pack, and a
starting template — and **records why**, so the choice is auditable and revisable.

### The discriminating questions

Deliberately about the user's *job*, not about RDF. A subject-matter expert must be able to answer
every one without knowing what SKOS is.

1. What decision or task does this need to support? (find content · classify content · agree
   terminology · integrate data across systems · machine inference · regulatory reporting)
2. Do users need to **browse a hierarchy**, or only pick from a list?
3. Does one thing go by **several names**? Do different departments call it different things?
4. Do you need **more than one language**?
5. Do you need the computer to **derive facts you did not state** (classification, consistency)?
6. Do you need to describe **individual things** (this specific contract, this specific asset), or
   only the *kinds* of things?
7. Is there an **existing standard** in your domain you must align to or report against?
8. Who maintains this, how often, and who signs off?
9. Roughly how many terms — hundreds, thousands, or hundreds of thousands?

### Routing

| Signal | Artifact | Model |
|---|---|---|
| Fixed set of permitted values, no structure | **Authority list** | SKOS `ConceptScheme`, flat |
| + hierarchy for browsing or classification | **Taxonomy** | SKOS, `broader`/`narrower` |
| + synonyms, scope notes, associative relations, multilingual | **Thesaurus** | SKOS-XL, ISO 25964 |
| + formal class/property semantics, constraints, inference | **Ontology** | OWL 2 |
| + individuals and integration at scale | **Knowledge graph** | OWL 2 + instance data |
| Constrain or validate a data structure | **Application profile** | SHACL + DCAP |
| Connect two vocabularies that both already exist | **Crosswalk** | SKOS mappings, ISO 25964-2 |

Question 7 can override the result outright: if the domain mandates a scheme, the answer is usually
**adopt or extend it**, not build. Question 8 is the reality check — an artifact nobody is resourced
to maintain should be scoped down, and the Advisor says so rather than cheerfully specifying an
ontology that will be abandoned in a year.

### The Advisor must be able to say "build nothing"

Its **first** action is a discovery sweep (see `adr/0003`). If something in the enterprise, or a
public standard, already covers the domain, the recommendation is *reuse, extend, or map* — with
"create new" ranked last and requiring a recorded justification. An advisor that always recommends
building is a silo generator, and this one is deliberately biased against its own product.

### Output: the Solution Brief

A recorded, versioned artifact in the project: the answers given, the recommendation, the reasoning,
the alternatives rejected and why, and the discovery results that informed it. It is revisable —
re-running the Advisor produces a new version and a diff, not a silent overwrite. Governance teams
have to justify these decisions later; a recommendation that leaves no trace is worthless to them.

---

## Part 2 — methodology packs

A pack is a **declarative, versioned definition** of a lifecycle: phases, activities, roles,
deliverables, and gate criteria. Packs are RDF, stored in git, and forkable — an organisation with
its own governance standard writes its own pack rather than begging us for a feature.

**Gate criteria are SHACL shapes.** This is the load-bearing design decision. "You may not leave
Conceptualisation until every concept has a definition and a preferred label in the primary
language" is a constraint over the graph, which is exactly what SHACL expresses. So the Methodology
Engine needs no bespoke rule language, gates are as inspectable and customisable as any other rule
pack, and a failing gate points at the *specific offending concepts* rather than saying "not ready".
This is why Phase 7 depends on Phase 4.

### Packs to ship

Each cites its source. Where a methodology is silent on something OpenBiz needs, the pack marks that
step as an OpenBiz addition rather than misattributing it.

**`z39-19-taxonomy`** — authority lists and taxonomies, from ANSI/NISO Z39.19.
Scope & warrant → term collection → hierarchy construction → validation → publication → maintenance.

**`iso-25964-thesaurus`** — thesauri, from ISO 25964-1.
Adds equivalence relations (USE/UF), associative relations (RT), scope notes, and multilingual
management. Gates enforce the standard's structural rules. ISO 25964-2 governs the crosswalk pack.

**`noy-mcguinness-101`** — the classic seven steps, the gentlest on-ramp to ontology work:
determine domain & scope → consider reuse → enumerate terms → define classes & hierarchy → define
properties → define facets → create instances. Best default for a first ontology.

**`methontology`** — staged lifecycle with continuous support activities: specification →
conceptualisation → formalisation → implementation → maintenance, alongside knowledge acquisition,
evaluation, and documentation throughout. Suits organisations that want document-heavy stage gates.

**`neon`** — **scenario-based, not a single lifecycle.** NeOn's nine scenarios cover: (1) from
specification to implementation, (2) reusing and re-engineering non-ontological resources,
(3) reusing ontological resources, (4) reusing and re-engineering ontological resources, (5) reusing
and merging ontological resources, (6) reusing, merging and re-engineering ontological resources,
(7) reusing ontology design patterns, (8) restructuring ontological resources, (9) localising
ontological resources.
This is the enterprise default, because it is the only mainstream methodology built around *reuse*
as the normal case rather than the exception — which is precisely the anti-silo posture in
`adr/0003`. NeOn deliberately prescribes no fixed lifecycle, so our pack model must support a
**scenario graph**, not just a linear phase list. That requirement comes from NeOn and nothing else,
and it is why the pack schema allows branching.

**`lot`** — Linked Open Terms: requirements specification → implementation → publication →
maintenance. Requirements are captured as **competency questions tied to user stories**, and the
methodology emphasises reusing terms from already-published vocabularies and publishing per Linked
Data principles. The pack for anything intended for external publication.

**`samod`** — agile and test-first. Three iterative steps, each ending in a released milestone,
built around a *modelet* (a small model for one exemplar domain description) that is merged into the
current model, with a growing *bag of tests* re-run against every milestone. Model, data, and query
tests must all pass before a milestone is released. Inspired by test-driven development and
eXtreme Design.
**SAMOD maps onto OpenBiz unusually well**: its test cases are our SHACL shapes and SPARQL
competency-question checks, so its milestones are literally gate evaluations. Best pack for a small
team iterating quickly.

### Choosing an ontology pack

| Context | Pack |
|---|---|
| First ontology, learning | `noy-mcguinness-101` |
| Reuse-heavy, networked, enterprise | `neon` |
| For external publication as Linked Data | `lot` |
| Small team, fast iteration, test-first | `samod` |
| Document-heavy staged governance | `methontology` |

---

## Part 3 — escalation between artifact types

Needs grow. The common failure is discovering in year two that a taxonomy needed to be a thesaurus,
and starting over. OpenBiz treats escalation as a **first-class, guided migration**, with the cost
stated honestly up front.

**Authority list → Taxonomy.** Add hierarchy. Cheap and safe; existing concepts are untouched.

**Taxonomy → Thesaurus.** Reify labels as SKOS-XL resources, add equivalence and associative
relations, add scope notes. Mechanically automatable; the real work is editorial, deciding which
alternative labels are genuine synonyms.

**Thesaurus → Ontology.** **Expensive, and not automatable — deliberately so.** A `skos:Concept` is
an *individual* representing a unit of thought; an `owl:Class` is a *set of things*. "Invoice" as a
concept in a thesaurus and "Invoice" as a class whose members are actual invoices are different
assertions, and mechanically rewriting one into the other is the single most common modelling error
in this field. OpenBiz will not offer a one-click conversion. It offers a guided reinterpretation
that forces an explicit decision per concept, keeps the SKOS vocabulary intact alongside the
ontology, and links them rather than replacing one with the other.

**Ontology → Knowledge graph.** Add instance data and integration. Mostly an infrastructure step.

Every escalation records a decision in the project's Solution Brief history, so "why is this a
thesaurus?" always has an answer.

---

## Part 4 — the Project Compass

The user-facing answer to "where am I?". Always visible, never a modal that gets dismissed:

- **Phase ribbon** — the pack's phases with the current one marked, in the style practitioners
  already recognise from methodology diagrams.
- **What to do next** — the next incomplete required activity, as an action, not a description.
- **What is blocking the gate** — failing exit criteria, each linked to the *specific concepts* that
  fail, because a gate that says "not ready" without saying which items are wrong is an obstacle
  rather than guidance.
- **Progress** — activities complete in this phase; phases complete overall. Honest about the
  distinction between "done" and "skipped with a reason".

### Guidance, not a straitjacket

Any gate can be overridden by a user with the right role, **with a recorded reason**. Enterprises
reject tools that block them, and governance teams need deviations logged rather than prevented.
Overrides appear in the audit trail and in the Compass, so a project running on three overridden
gates cannot quietly present itself as compliant.
