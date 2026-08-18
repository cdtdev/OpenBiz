# ADR 0017 — A proposed change is a named graph plus a record, and approval is a copy

**Status:** accepted (2026-08-18) · **Phase:** 2

## Context

`CLAUDE.md` §3 states the rule this decision implements:

> **Any path that changes a vocabulary takes *candidates*, not just direct writes.** A candidate is
> a proposed change carrying its provenance, its source, and a confidence where one is meaningful,
> which a human reviews before it lands.

and it says why it has to be built *before* the paths that need it: "build direct writes now and
every import, discovery, and agent path has to be retrofitted later."

That instruction has been load-bearing for six iterations without existing. Three Phase 1 items —
RDF parsing, SPARQL Update, and the Graph Store Protocol — are all deferred on it, each with the
same reasoning written against it: their production caller mutates a vocabulary, and there was no
shape for a mutation to arrive in. `POST /api/graphs` answers 405 for a related reason. So the seam
is not one feature among the Phase 2 list; it is the dependency the phase is ordered around.

The item as written is larger than one iteration, so it is split in the plan. This ADR records the
first slice: the model, its persistence, its review, and its first producer.

## Decision

### A candidate's payload is a named graph, not a serialised blob

The obvious implementation is a record in the system graph carrying the proposed statements as a
literal — a chunk of Turtle in a string. It is less code. It was rejected because it makes the
proposed statements **opaque to every tool in the product**: a reviewer cannot export them, cannot
query them, cannot diff them, and the interface would need a parser of its own before it could show
anything but a wall of text.

Instead the statements are staged as ordinary quads in a graph of their own,
`urn:openbiz:graph:candidate:<id>`, derived from the candidate's identifier rather than chosen.
Three things follow for free, and each is a capability we would otherwise have had to build:

- `GET /api/export?graph=urn:openbiz:graph:candidate:7&format=…` already serialises a pending change
  into any of the six syntaxes. `openbiz candidate 7` uses exactly that call to print the diff.
- A SPARQL query naming `FROM <urn:openbiz:graph:candidate:7>` asks anything else about it, through
  the endpoint that already exists.
- **Approval is a copy between two graphs inside one transaction**, so a half-applied candidate is
  not a state the store can be in. The alternative would have been parse-then-write at approval
  time, which means a syntax error can surface at the moment of approval rather than at the moment
  of proposal — the worst possible time to discover it.

The staging graph is **registered**, as `GraphKind::Candidate`. Registering it costs a format
version (below) and buys the thing `CLAUDE.md` §1 is about: an operator asking "what is actually in
my store?" gets the whole answer. Statements the store holds and cannot describe are precisely the
opacity we are attacking, and an unregistered graph would also have broken backup and restore —
`GraphId::classify` judges every graph name in a backup file, and a name it cannot place is refused.

The kind is what keeps it out of the way. `GET /api/graphs` reports it; the interface filters on
`kind === "vocabulary"` and *counts* what it holds back; the SPARQL endpoint's default dataset is
the registered vocabulary graphs and nothing else. So a pending proposal is visible to anyone
looking for it and invisible to everyone querying their vocabularies, which is what "not yet
approved" has to mean.

### Format version 3 exists although the migration writes nothing

Adding a fourth `GraphKind` changes what may be on disk. A build without the candidate seam reads
`candidate` out of a registry, finds a kind it does not know, and reports **the whole registry as
corrupt metadata** — which is a correct refusal reached by a wrong route, and it sends an operator
who has merely downgraded off to disaster recovery.

`FORMAT_VERSION` therefore moves to 3 and migration `0003-allow-candidate-graphs` joins the chain.
It rewrites nothing: every version-2 store is already a valid version-3 store, because the change
is additive. That deliberately sits against iteration 16's warning that *"a version that records no
real difference teaches the next person that versions are decorative"*, so the difference is stated
rather than assumed. The difference is real and it is one-directional: a version-3 store may hold
something a version-2 build cannot describe. The stamp is the gate that turns that into "upgrade"
instead of "corrupt". Inventing a write so the step looked substantial would have been the actual
dishonesty.

The step that writes nothing is the one that can silently not run, so it has an end-to-end test of
its own against the real binary — `a_version_two_backup_is_brought_forward_by_a_migration_that_rewrites_nothing` —
restoring a hand-written version-2 file and checking the stamp moved, the report said so, and the
record is in the store.

### Provenance is mandatory; confidence is not

A candidate carries the source kind (`import`, `discovery`, `manual`, `bulk-edit`, `assistant`), who
or what raised it, a one-line note on why, when, and optionally a confidence between 0 and 1. Agent,
note, and source are **required and validated**; a proposal that cannot say who raised it or why is
one no reviewer can weigh, and a reviewer forced to infer the intent from the statements is doing
the producer's job.

The confidence is optional because it is only meaningful for a producer that computes one. A file
import has none, and stamping `1.0` on it would put a number a reviewer could sort by beside numbers
that mean something. A confidence outside 0–1 is refused: a scale nobody has stated is worse than no
confidence at all.

`source` is a **closed set written as a token**, not free text, because the first question a
governance team will ask is "show me everything an assistant proposed", and free text cannot answer
it. `CandidateSource::parse` refuses a token it does not know rather than defaulting, so a store
written by a build that knew a sixth producer is refused rather than misread.

### The states are `proposed`, `applied`, `rejected`

Not `approved`. Approval applies inside the same transaction that records it, so an "approved but
not yet applied" state is one nothing can produce — and naming a state today that nothing produces
is a claim about a capability we do not have. When Phase 6 gives approval a workflow, that limbo
becomes real and gets its own state.

A decided candidate is refused a second decision, naming the state it is in. Deciding twice would
either duplicate statements or silently do nothing, and both are worse than being told.

The record and the vocabulary change **commit together**. A store can therefore never hold a
candidate marked applied whose statements are absent, or statements in a vocabulary with no record
of who let them in. That pairing is the whole value of the seam to an auditor, and it is the reason
approval is not two operations.

### The payload survives the decision

An applied candidate keeps its staging graph, and so does a rejected one. Deleting the evidence of
what was approved is not a default a governance product may take — if the statements are later
edited, the record of what was actually agreed is the only thing that can settle the argument.

It costs storage: an approved import is held twice. That is stated in `docs/UNTESTED.md` and a
retention policy is in `docs/PROPOSED.md` for a human, because *how long to keep the evidence* is a
compliance decision and not one the loop should take.

### The first producer is `openbiz import`, and it is a command rather than an endpoint

`openbiz import <graph> <file>` parses a file and proposes it. `openbiz candidates`, `openbiz
candidate <id>`, `openbiz approve <id>`, and `openbiz reject <id>` review and decide.

Not HTTP, for `adr/0015`'s reason applied to a sharper case: there is still no authentication, and
`openbiz approve` can write to a customer's vocabulary. An unauthenticated "apply this change" is
the same objection that has SPARQL Update deferred. Unlike backup and restore, these do not need the
store to themselves *in principle*; they need it today only because the embedded store takes an
exclusive lock. The HTTP and UI half lands with authentication, not before it.

Two smaller decisions inside that:

- **The syntax comes from the file extension**, resolved through the same table `?format=` and
  `Accept` go through — so what `openbiz export` writes is what `openbiz import` reads back. An
  extension we do not know is refused naming the six we do, rather than guessed at: reading a file as
  the wrong syntax produces either a syntax error two hundred lines in or, far worse, a successful
  import of something else.
- **A decision needs somebody to record.** The store refuses an unattributed decision, so the
  command line resolves `OPENBIZ_ACTOR`, then `USER`, then `LOGNAME`, and *fails* if it finds none,
  telling the operator how to satisfy it. The recorded string says it came from the command line
  rather than dressing the operating system's account of who ran it up as an identity we verified.

### What the import refuses

- **A target that is not a registered vocabulary.** Importing into a graph that does not exist would
  create a vocabulary as a side effect, and creation is the path `CLAUDE.md` §1.7 requires to run
  through discovery with a recorded justification. An import is not a way around that rule.
- **Statements naming a graph other than the target.** A quad syntax carries graph names and an
  import goes to one vocabulary. Dropping the names silently would land a multi-graph file in one
  place and report success; honouring them silently would let one import write to vocabularies the
  operator never named. Statements in the default graph and statements naming the target itself are
  both accepted, which is exactly what makes an export of a vocabulary re-import into it in all six
  syntaxes.
- **A file with no statements**, which is almost always a file in a different syntax from the one the
  extension named.

Blank node labels are renamed as they are read, so two imports that both say `_:b1` do not silently
merge into one node.

## What was measured

- **302 Rust tests** (from 273) and **30 UI tests** (from 29); `fmt`, `clippy -D warnings`, `deny`,
  and the UI build green.
- The round trip is proven **for all six syntaxes**: a vocabulary is exported, re-proposed, and the
  staged statements are compared to the source graph statement for statement. This is what closes
  Phase 1's parsing item.
- The suite was proven to **discriminate** before it was trusted. Three mutations were each confirmed
  to turn it red: approval no longer copying the payload (4 tests fail), the graph-name rule dropped
  so a file naming another vocabulary is flattened (1), and blank-node renaming removed so two
  imports merge (1).
- End to end against the real binary: a hand-written backup seeds a vocabulary, an import proposes
  five statements, a backup of the store shows **zero** statements in the vocabulary and five in the
  staging graph, the review prints the provenance and the statements, approval moves them, a second
  approval is refused, and an anonymous approval is refused with the variable to set.

## Consequences

- Phase 1's **RDF parsing** item is met and checked off: six syntaxes, round-tripped against the
  serialiser, with a real production caller. The parser's only entry point is the candidate seam,
  which is the point — there is no direct-write import to retrofit later.
- SPARQL Update and the Graph Store Protocol remain deferred. They now have half their dependency;
  the other half is the authorisation, and the version worth having is one where an update means
  "propose this change set, show me the diff, and let a human approve it".
- Every later producer — discovery matches, bulk edits, Phase 10 agents — has an existing seam to
  arrive through, and `CandidateSource` already names them.
- **Removals are not yet expressible.** A candidate proposes additions. A merge, a deprecation, and a
  corrective agent all need removals, and that is the next slice of the seam rather than a gap in
  this one; the record shape leaves room for it.
