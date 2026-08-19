# 0041 — A retired concept is shown and marked, never hidden

- **Status:** accepted
- **Date:** 2026-08-19
- **Phase:** 2 — SKOS authoring model
- **Item:** Deprecation lifecycle, read half — every browse and search path knows the marker

## Context

`adr/0040` retires a concept by adding three statements and **removing nothing**. That is what
makes a retirement safe: the IRI keeps resolving, every reference to it keeps working, and an
auditor asking in three years what the term meant still gets an answer.

It is also why the operation had, until now, no visible effect anywhere a person looks. A retired
concept is byte-for-byte the concept it was — same `rdf:type`, same labels, same `skos:broader`,
same place in every scheme — so `openbiz tree`, `openbiz ancestors`, `openbiz paths`,
`openbiz search` and `openbiz inspect` showed it exactly as they showed it the day before. An
operator could retire a term and be offered it by the next search, with nothing to tell them.
`adr/0040` recorded that as the lifecycle item and `docs/UNTESTED.md` recorded it as the largest of
its five gaps. This ADR is that item's read half.

The write half of the lifecycle — un-retiring a concept — is not here. It is a change to a
vocabulary, it goes through the candidate seam, and it belongs with the other write operations.

## Decision

### 1. The status is read beside the model, not inside it

`owl:deprecated` is **not SKOS**. SKOS 2009 has no status vocabulary at all, which is why
`adr/0040` had to borrow OWL 2's annotation property and Dublin Core's replacement predicate.
`CoreModel` reads a graph *as SKOS* — its resources, its classes, its integrity conditions are all
SKOS's — and putting a non-SKOS status inside it would turn that boundary from a rule into a matter
of taste.

So `openbiz_skos::Retirements` is a separate index, built from the same stream of statements, in
the same pass, exactly as `DeprecationScan` already is for one named concept. The difference in
scope is why it is a second type rather than a loop over the first: a scan answers about a concept
named in advance, and a browse command does not know which of the concepts it is about to print are
retired until it has printed them.

`crate::inspect::read_with_retirements` is the single seam. It makes the two passes `read` already
made and returns the model and the index together, so a browse command that marks its retired
concepts costs **no extra scan of the store** — and so a read path added later cannot quietly
forget that some of what it prints is obsolete.

### 2. Nothing is bounded here, and that is a decision rather than an oversight

Every other enumeration in `openbiz-skos` carries a bound, and six of them are recorded in
`docs/UNTESTED.md` as constants measured against nothing. This one has none.

The argument is containment, not optimism: the retired resources are a **subset** of the resources
`CoreModel` already holds unbounded, and the replacements recorded about them are a subset of the
statements it already read. A caller that can hold the model can hold this. A seventh unmeasured
constant guarding something strictly smaller than an unguarded thing would be a ritual.

### 3. Show and mark. Never hide — in any command

Each read path admitted three options: show, mark, or hide by default. Every one takes the same
one, and the uniformity is deliberate: a retired concept that disappears from one command and
appears in the next teaches an operator that the tool is unreliable rather than that the concept is.

- **Hiding breaks the hierarchy.** A retired concept with current concepts below it is the
  *commonest* outcome of a retirement, because `openbiz deprecate` deliberately does not touch the
  children (`adr/0040` §4). Dropping it from `openbiz tree` would leave those children hanging off
  nothing and silently misreport the shape of the vocabulary.
- **Hiding a search hit manufactures the silo.** `CLAUDE.md` §1.7 and `openbiz search`'s own module
  documentation say the same thing: a silo is created when someone looks for a term, does not find
  it, and makes a new one. A retired concept omitted from search results reads as "this vocabulary
  has never heard of it" — the single conclusion most likely to produce a second, worse copy of a
  term that already exists. Told "it exists, it is retired, use this instead", the same person does
  the right thing.
- **Showing without marking was the defect**, and it is what shipped in `adr/0040`.

Filtering retired concepts out **on request** is a real need and a separate plan item. It is opt-in
per command, it is not a default, and it is not built here.

### 4. Marked in a list, explained at the focus — except in search

A subtree of a thousand descendants would be unreadable with a three-line retirement notice against
each one, and `openbiz tree` already prints its derivation as structure for that exact reason. So a
concept in a **list** carries `[retired]` and nothing more, and the concept the command was *asked
about* carries the full account: that it is still there, what supersedes it, and — where nothing
does — that the absence is the vocabulary's answer rather than the report's omission.

**`openbiz search` is the deliberate exception**: every hit gets the full account. Search is where a
term is chosen for reuse. A person told only `[retired]`, with no successor named, has been given a
dead end and will either use the retired term or create a duplicate. That is the failure this whole
feature exists to prevent, so the successor is named at the point of choosing.

### 5. What the marks add up to is stated, not left to be inferred

A report that marks concepts and says nothing else has moved the work to the reader. So each
command draws the one conclusion its own shape makes visible:

| Command | What it says beyond the marks |
|---|---|
| `tree` | how many concepts below a retired one are **not** retired, and that whether each moves, retires, or stays is a person's decision |
| `ancestors` | that a **current** concept sits under retired ones, and that the hierarchy did not change when they were retired |
| `paths` | which retired concepts the routes run through, listed once rather than marked in every chain — a breadcrumb built from one would show a reader an obsolete term |
| `search` | how many of the hits shown are retired, and why they are shown rather than hidden |
| `inspect` | the whole-vocabulary backlog: how many are retired, how many record a successor, how many still have current children, how many still head a scheme |

`openbiz inspect`'s section is **counts, not findings**. Leaving children under a retired parent is
`adr/0040`'s deliberate decision, not a defect, and a vocabulary mid-retirement must not be reported
as broken. The test asserts `findings: 0` for exactly that reason.

### 6. Two things the read half can see that the write half cannot

- **A replacement that is itself retired.** `openbiz deprecate` refuses to *create* that trail and
  cannot refuse to *find* it: the replacement may have been retired long after it was named. The
  signpost is followed for the reader and called out.
- **A resource carrying `dcterms:isReplacedBy` with no `owl:deprecated`.** `openbiz deprecate`
  writes both or neither, so this arrived by import or by hand. It reads as a perfectly current
  concept everywhere. It gets its own mark, and `openbiz inspect` names them, because the most
  likely way a retirement goes wrong should not be the one thing nothing looks at.

### 7. Lenient on read, strict on write

OWL 2 §5.5 requires `"true"^^xsd:boolean`, which is what this build writes. A vocabulary that
arrived from another tool carrying a plain `"true"` is still saying the concept is retired, and
reading that as "current" is the same false negative from the other direction. `owl:deprecated
"false"` marks nothing. This is `openbiz deprecate`'s existing leniency, from the same function.

## What was measured, and what was not

A test failed and was right to: the fixture's `"true"@en` was correctly read as **not** retired. A
language-tagged literal is not the boolean OWL 2 asks for and is not the untyped string the
leniency above admits, so the fixture — not the rule — was wrong. The test now writes the typed
form the production path writes.

**Nothing here is measured on a large vocabulary.** The index adds one `BTreeMap` insertion per
`owl:deprecated` or `dcterms:isReplacedBy` statement to a pass that already happens, and
`inspect`'s section walks the retired concepts once calling `CoreModel::children` on each — which
is the ninth entry in this crate's run of unmeasured costs. It is in `docs/UNTESTED.md` with the
other eight.

## Alternatives rejected

- **Hiding retired concepts by default, with a flag to show them.** The three arguments in §3. The
  short version: the default that loses information is the wrong default for a governance tool, and
  every one of the three failure modes is silent.
- **Putting the status on `Resource` inside `CoreModel`.** Convenient, and it would erase the one
  rule that keeps the SKOS model a SKOS model (§1). The cost of the separate index is one clone per
  statement in a pass that already runs.
- **A `Status` enum on every printed line — `current` / `retired`.** Symmetrical and much noisier:
  every line of every report on every vocabulary would carry a word, to say something true of
  almost all of them. A mark that appears only when it means something is the one that gets read.
- **Deriving the mark from `DeprecationScan` per printed concept.** Correct and quadratic: one pass
  over the graph per concept in a subtree.
- **Doing the un-retirement in the same item.** It is a write, it goes through the candidate seam,
  and folding it in would have meant designing a write while deciding show-versus-hide for five
  read paths.

## Consequences

- The largest gap `adr/0040` left is closed: a term retired through `openbiz deprecate` now reads
  as retired in every command that browses or searches the vocabulary.
- **There is still no way to un-retire a concept.** It is the remaining half of the lifecycle item
  and it is on the plan.
- **There is no way to ask for only current concepts.** Every command shows everything and marks
  what is obsolete; a `--current-only` filter is a separate plan item.
- `openbiz notes` and `openbiz mappings` — which report one named resource rather than browsing —
  do not carry the mark. In `docs/UNTESTED.md`.
- A vocabulary that has never retired anything reads exactly as it did. Every one of the five
  commands has a test asserting it, so this feature is not a tax on the other 99%.
