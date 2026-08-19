# 0026 — Documentation properties: seven of them, one entailment, and no integrity condition

- **Status:** accepted
- **Date:** 2026-08-19
- **Iteration:** 29
- **Supersedes:** nothing
- **Relates to:** `adr/0019` (the engine-free core model), `adr/0020` (lexical labels),
  `adr/0022` (why a refinement of `skosxl:labelRelation` is not closed), `adr/0025` (why S24 is a
  walk and not a table)

## Context

SKOS Reference §7 defines seven documentation properties — `skos:note` and the six specific ones
— and two statements about them:

- **S16.** "skos:note, skos:changeNote, skos:definition, skos:editorialNote, skos:example,
  skos:historyNote and skos:scopeNote are each instances of owl:AnnotationProperty."
- **S17.** "skos:changeNote, skos:definition, skos:editorialNote, skos:example, skos:historyNote
  and skos:scopeNote are each sub-properties of skos:note."

§7 carries three examples, all marked *consistent*: **22** a note as a literal, **23** a note as an
IRI, **24** a `skos:definition` on an `owl:Class`.

The build-plan item named five properties. §7 has seven, and §7.1 also designates them as extension
points for a vocabulary's own refinements. The item was split in place: this ADR covers the seven
properties and S17; the extension point is Phase 2's next item.

## Decisions

### 1. §7 states no integrity condition, and we invent none

This is the decision the rest follows from, and it was the one worth writing down.

§5.4 has a subsection headed "Integrity Conditions" and states exactly two, S13 and S14. **§7 has
no such subsection at all.** So nothing in this module can make a graph inconsistent, and nothing
in it raises a `Finding` of any severity — not `Inconsistent`, and not `IllFormed` either.

Three things a tool built on habit rather than on the specification would flag, and which we
deliberately do not:

| Shape | Why it is not a finding |
|---|---|
| A concept with no `skos:definition` | Consistent SKOS. The check every incumbent runs here is ANSI/NISO Z39.19 or ISO 25964 — a *rule pack*, in `openbiz-validate`, where it can be named, cited, and switched off. |
| Two `skos:definition`s in the same language | S14's one-per-language-tag rule is about `skos:prefLabel`. §7 has no counterpart, and a thesaurus merged from three sources routinely carries three definitions. |
| A note whose value is an IRI | Example 23, marked consistent. See decision 2. |

The first is the one that matters commercially, because "which concepts lack a definition" is a
real and frequently asked governance question — and answering it with a SKOS citation would be
citing a statement the specification never made. `openbiz inspect` therefore prints the count and,
on the same screen, the sentence naming which document *would* ask for a definition. A zero must
not read as our verdict.

`section_7_states_no_integrity_condition_and_this_build_invents_none` asserts the absence, so a
later iteration cannot add one without deleting a test that says why it is wrong to.

### 2. A note's value is a bare `Term`, and we do not guess which usage pattern it is

§7.1 names three patterns the data model must accommodate: *documentation as an RDF literal*,
*as a related resource description*, and *as a document reference*. S16 makes all seven properties
`owl:AnnotationProperty`, with no domain and no range, so the value is unconstrained.

We therefore reuse the existing `Term` — node or literal — rather than introducing a value type of
our own. Three patterns collapse into two term shapes, and the second and third are
**indistinguishable from the statement alone**: `<A> skos:note <B>` is Example 23 whether `<B>` is a
`foaf:Document` or a blank node carrying an `rdf:value`. Inferring which from the shape of the
surrounding graph would be a distinction the specification refuses to draw, reported as though it
had.

**Consequence, recorded rather than hidden:** a caller cannot ask "is this note inline text or a
pointer?" beyond "is it a literal?". That is what the specification supports.

### 3. The object of a note acquires nothing

A `skos:broader` types both of its ends, because S19 and S20 are a domain and a range and say so.
§7 has neither. So Example 23's `<MyNote>` enters the model as nothing: no class, no resource
entry, no count. `openbiz notes <graph> <MyNote>` is refused rather than answered with an empty
report.

The alternative — registering it so the report can mention it — would add a member to the
customer's vocabulary that nobody wrote, which is the failure mode `adr/0019` established the
model must not have.

### 4. S17 is materialised; S24 is walked. The difference is arithmetic, not taste

`adr/0025` answers S24's transitive closure by walking, because a legal chain of 100 000 links
licenses five thousand million pairs and the conclusion cannot cite a path it did not keep.

S17 is the opposite case and gets the opposite answer:

| | S24 (`broaderTransitive`) | S17 (note sub-properties) |
|---|---|---|
| Depth | unbounded, graph-controlled | exactly one; `skos:note` has no super-property and none of the six is under another |
| Can chain | yes | no, by the specification's own list |
| Cost | quadratic in depth | at most one entry per stated note |
| Needs a cycle guard | yes | no — it terminates by construction |
| Derivation | is the *path*, so it must be walked | is one premise and one rule |

So the lift is applied in `CoreModelBuilder::attach_notes`, with no bound and no guard, and every
lifted note carries a `Derivation` naming the statement it came from and quoting S17 in full.

**Upwards only.** A stated `skos:note` entails nothing about which of the six it might have been.
Inferring one would be the same error as inferring a `skos:broader` from a
`skos:semanticRelation`, which `adr/0023` refused for the same reason.

**An assertion beats an entailment.** A resource stating both `skos:definition "X"` and
`skos:note "X"` keeps the asserted note and records no derivation — consistent with an asserted
class (S29) and an asserted label dumbed down from SKOS-XL (S55). Tested in both statement orders,
because a pass whose answer depends on the order the author happened to write in is a defect that
only shows up on somebody else's file.

### 5. Two production callers, and why the content one is per-resource

`openbiz inspect` gains a **documentation coverage** table: one row per property, counts and not
content, with the rows that are zero kept. Counts, for the reason the languages section is counts —
every other answer in that report is bounded by the vocabulary's *structure* and the notes are
bounded by its *size*, so a hundred-thousand-concept thesaurus would drown the report.

`openbiz notes <graph> <resource>` prints the notes themselves, one resource at a time. It exists
because of one thing no export can do: **a Turtle export shows the `skos:definition` the author
wrote and never shows the `skos:note` it entails.** An operator reading "4 note(s)" after writing
three definitions has nowhere else to look. Here the entailed note is printed beside the asserted
one with its premise and the quoted rule.

It takes a **resource**, not a concept, because Example 24 documents an `owl:Class`. The CLI
parameter is named `resource` accordingly, and a test asserts the error message says "in SKOS
terms" rather than promising a concept it does not require.

Both are commands and not endpoints for the reason `openbiz inspect` and `openbiz ancestors` are:
they only read, and the interface that will show a definition beside its label is Phase 3's item.
Shipping an endpoint now would be a caller with nothing behind it.

## What was measured

Nothing was measured, and that is a gap rather than an omission. The notes are the longest text a
vocabulary holds and the model now keeps all of them, keyed by value, plus one lifted entry per
stated note. `adr/0024` measured what a semantic relation costs (about 3.9 KiB per stated
`skos:broader`); the equivalent number for a note is not known, and `docs/UNTESTED.md` records it.
The `scale` module is where it would go.

## Consequences

- `skos:scopeNote` moved from "counted and dropped" into the core model. The test that drew that
  boundary was updated rather than deleted, and now asserts both halves: `skos:notation` is still
  dropped (§6 is not built), and the scope note is not.
- `SkosRule` gained `S16` and `S17`, each quoting the specification in full, so every report that
  prints a derivation prints the statement rather than the number.
- Nothing in the UI changed. The documentation properties are reachable only from the command line
  until Phase 3's concept editor.
- §6 (`skos:notation`, S15) has **no build-plan item anywhere**, which was noticed while reading §7
  and is in `docs/PROPOSED.md`. It is a gap in the plan, not in the build.
