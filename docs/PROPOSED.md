# Proposed work — awaiting human promotion

Work the loop believes is needed but **did not authorise itself**. This file is the brake on
autonomous scope creep, and it only works if the loop actually uses it.

**The loop writes here. The loop does not promote from here.** A human promotes items into
`BUILD-PLAN.md` via `/openbiz-status` → `promote <n>`. Never move your own proposal into the plan
and then build it — that defeats the entire point of the file.

## Entry format

Copy this shape exactly; `/openbiz-status` parses the `Status:` line semantically.

```
### <short imperative title>
- **Status:** proposed.
- **Gap:** what is missing or wrong today, concretely.
- **Why load-bearing:** what this unlocks, or what breaks without it. If it is a nice-to-have,
  say so plainly — an inflated proposal wastes a human decision.
- **Cost & impact:** rough iterations, any phase it depends on, runtime or UX impact.
- **Suggested phase:** Phase N.
```

`Status:` values: `proposed` (open), `promote-requested` (queued by a human, not yet applied),
`promoted (→ Phase N)`, `deferred`, `rejected`.

---

## Open proposals

### Decide how long a decided candidate's evidence is kept
- **Status:** proposed.
- **Gap:** an approved candidate's staged statements are kept forever, and so are a rejected one's.
  That is the deliberate conservative default — deleting the evidence of what was approved is not a
  choice a governance product should make silently — but it means an approved import is stored
  **twice**, permanently, and a deployment doing a monthly bulk import grows its store without bound
  for reasons nobody explained to them. There is no policy, no configuration, and no pruning.
- **Why load-bearing:** *how long to keep the evidence* is a compliance question with a different
  answer in pharma, finance, and a research library, and the loop should not invent one. What is
  needed is a decision — keep forever, keep for N years, keep the record but drop the payload after
  approval, or let the deployment choose — and then a small amount of code. Without it the honest
  position is the one in `UNTESTED.md`: the default is safe and the cost is unbounded.
- **Cost & impact:** one iteration once the policy is chosen. Storage impact is proportional to
  import volume; no runtime cost on any read path.
- **Suggested phase:** Phase 6 (governance & workflow), where retention sits beside the rest of the
  audit model.

### LLM assistance opportunities — the candidate seam is where they all arrive
- **Status:** proposed. Recorded under the product owner's standing instruction
  (`FEEDBACK-LOG.md`, 2026-08-18) to note assistance opportunities as they are discovered, **not** a
  request to pull Phase 10 forward.
- **Gap:** three concrete user problems surfaced while building the seam, and each is now a
  `CandidateSource` away from being buildable rather than a redesign away.
  1. **An import arrives with no idea what it is.** `openbiz import` proposes five statements and
     tells a reviewer nothing about whether they duplicate concepts the vocabulary already has,
     contradict its existing labels, or introduce a second preferred label in a language. A reviewer
     facing a 10 000-statement import has no realistic way to check by hand, so they will approve it.
     An assistant that reads the staged graph against the target and *annotates the candidate* —
     "42 of these concepts already exist under different IRIs" — is the difference between a review
     and a rubber stamp. It writes nothing; it adds to the record a human is already reading.
  2. **The mandatory note is the weakest field in the record.** `openbiz import` fills it with
     "imported from animals.ttl as Turtle", which is provenance rather than intent. For a
     human-raised candidate the note is the one thing an auditor will actually read in five years,
     and people write "update" in it. Drafting a note *from the diff* — with the human free to
     reject or rewrite it — is a small, high-frequency editorial task with a manual path already in
     place.
  3. **Rejections teach nothing.** A rejected candidate records who and when but not *why*, so the
     same bad import arrives again next month. Summarising what a vocabulary's reviewers keep
     refusing, across hundreds of candidates, is exactly the recall-across-thousands-of-records
     judgement no human does well and no report answers.
- **Why load-bearing:** not load-bearing now, by design. The point of recording it is that Phase
  10's agent list should reflect what was learned building Phases 1–9. All three sit behind the
  existing seam and none of them writes to a vocabulary.
- **Cost & impact:** none until Phase 10. Each is one agent emitting an annotation on an existing
  record.
- **Suggested phase:** Phase 10.

### Add a UI test runner before Phase 3 begins
- **Status:** promoted (→ Phase 0) · **done, iteration 4.**
- **Gap:** `ui/package.json` has no `test` script, so there is no UI test runner at all. The
  iteration driver's `npm test` step is a no-op that passes silently, which is worse than having no
  step — it reads as a green suite. `App.tsx` has three `Probe` states (loading, ok, error) and none
  is asserted; React could throw on mount and every check we run today would still pass, because
  they assert on HTTP transport rather than on rendering.
- **Why load-bearing:** Phase 3 is the differentiator and is eleven items of UI. Retrofitting a test
  runner onto a built design system is far more expensive than installing one against two
  components, and `CLAUDE.md` §4.2 requires the UI suite to be green — a claim we cannot currently
  make truthfully. Also blocks §4.4's keyboard-navigability requirement from being checkable.
- **Cost & impact:** ~1 iteration. Vitest plus Testing Library, both MIT. Adds a CI step. No runtime
  or binary-size impact — dev dependencies only, never embedded.
- **Suggested phase:** Phase 0, or as the first item of Phase 3.

### Bring the UI dependency tree under the §5 licence gate
- **Gap:** `cargo deny check licenses` enforces `CLAUDE.md` §5 across the Rust tree in CI. Nothing
  equivalent exists for `ui/`, so §5's "every new dependency gets a licence check" is, for npm, a
  convention the loop follows by hand — which is precisely the enforcement gap branch protection
  was opened for on the git side. Adding four dev dependencies this iteration made it concrete: the
  check happened, but only because the iteration chose to do it.
- **Why load-bearing:** Phase 3 is eleven items of UI and will pull in a component library, an
  icon set, and a router. That is the moment an npm subtree acquires something copyleft or
  source-available, and the moment it is most expensive to discover late. An open-core product
  whose licence policy covers one of its two dependency trees does not have a licence policy.
- **The concrete finding that prompted this:** a hand sweep of all 153 installed packages found one
  unlisted licence — `caniuse-lite` under **CC-BY-4.0**, pre-existing, transitive via Vite's
  `browserslist`, build-time only, and a data package whose contents never reach the binary.
  CC-BY-4.0 is attribution-only, not copyleft, so on substance this is a §5 "unlisted but
  permissive" call rather than a wall. **It should be recorded in an ADR, and it is not** — the
  proposal should cover writing that ADR alongside the gate, not just the gate.
- **Cost & impact:** ~1 iteration. Candidate tools are `license-checker-rseidelsohn` (BSD-3) or a
  ~30-line `node` script over `node_modules` with no new dependency at all — the sweep run this
  iteration was the latter and worked fine, which argues for checking it in rather than adding a
  tool. Adds one CI step to the `UI` job. No runtime or binary-size impact.
- **Suggested phase:** Phase 0, or as the first item of Phase 3 — before the component library
  lands, not after.

### Headless-browser smoke test against the release binary
- **Status:** promoted (→ Phase 3).
- **Gap:** nothing has ever executed the bundle in a browser. `adr/0004` proves the right bytes are
  served; it cannot prove the app mounts. The gap will widen once Phase 3 adds routing, and the
  failure mode — binary serves 200, page is blank — is exactly the one transport tests cannot see.
- **Why load-bearing:** moderately. It is the only check that covers the whole promise end to end
  ("download, run, done"), and it would also catch CSP, `crossorigin`, and MIME-strictness problems
  that only appear in a real browser. Honestly a nice-to-have *until* Phase 3, and close to
  essential after it.
- **Cost & impact:** ~1 iteration. Playwright is Apache-2.0; it downloads browser binaries, which
  makes CI slower and adds a network dependency to the *build*, never to the product.
- **Suggested phase:** Phase 3, alongside the accessibility item it would share a harness with.

### Test `Config::load` against a real process environment via a subprocess
- **Status:** promoted (→ Phase 1).
- **Gap:** `Config::resolve` is tested exhaustively with an injected environment, but `Config::load`
  — the wiring that supplies `std::env::var` and the default path — has no automated test, because
  `std::env::set_var` mutates state shared across the test binary's threads. A typo in a variable
  name inside `load` would pass CI. The four failure paths were verified by hand this iteration,
  which is not a regression test.
- **Why load-bearing:** modestly, but it grows. Every phase adds settings, and each one widens the
  untested gap between the tested merge and the real environment. The same harness would let us
  assert on the startup provenance log, which is now a user-facing contract documented in
  `docs/CONFIGURATION.md` and asserted nowhere.
- **Cost & impact:** well under an iteration. Spawn the release binary as a subprocess with a
  controlled environment and a temporary directory, assert on its stderr — the same shape as
  `tests/serves_embedded_ui.rs`. No new dependency.
- **Suggested phase:** Phase 0, or folded into the first Phase 1 item that makes `data_dir` real.

### Show the effective configuration and its provenance in the admin console
- **Status:** promoted (→ Phase 14).
- **Gap:** `Setting`/`Source` know where every value came from, but that answer is only reachable in
  the startup log and in error messages. An operator debugging a running server has to find the log
  from process start, which in a container may be long gone.
- **Why load-bearing:** nice-to-have on its own; genuinely valuable as part of the Phase 14 admin
  console, where it is the difference between "the settings screen" the incumbents ship and a screen
  that answers *why* a value is what it is. Needs authentication first — effective configuration is
  not public information, so this cannot land before Phase 6.
- **Cost & impact:** small once the console exists; a read-only endpoint and a table. Must redact:
  by Phase 10 the config will hold LLM provider credentials, which `CLAUDE.md` §14 says never go in
  logs — so this screen must show a credential's *source* without its *value*.
- **Suggested phase:** Phase 14, with the admin console.

---

### Make the plan's `**Status:**` line checkable rather than hand-written
- **Status:** proposed.
- **Gap:** `BUILD-PLAN.md`'s `**Status:**` and `**Current position:**` lines are prose the loop
  writes from memory at the end of an iteration. After iteration 4 the product owner caught them
  claiming "Phase 0 is complete — no open items" while Phase 0 still held an unchecked box, because
  the loop had promoted an item into that phase minutes earlier and then described the phase from
  its recollection of what was there before. The factual error is fixed and the counting discipline
  is now written into the plan's header, but the discipline is a convention, and this repository has
  already learned once — with branch protection — what a convention is worth compared with a check.
- **Why load-bearing:** the repository is public. A plan that declares a phase complete while an item
  in it is open is exactly the "roadmap you cannot trust" failure we attack the incumbents for, and
  `CLAUDE.md` §4's "honesty over green" makes misreporting worse than the gap being reported. It is
  small now and corrosive as a habit — and it is the kind of claim a script can verify absolutely,
  since both the claim and the evidence are in one file.
- **What it would be:** a CI job that parses `BUILD-PLAN.md`, counts `- [ ]` per phase, and fails if
  the `**Status:**` or `**Current position:**` lines name a phase as complete that still has
  unchecked items, or state an item count that does not match. Roughly the same shape as the `Single
  binary` job: cheap, mechanical, and it fails on the branch rather than being noticed by a human
  three iterations later.
- **Cost & impact:** well under one iteration. A script plus a CI step; no runtime or binary impact.
  The honest risk is that it invites writing the status line to satisfy the parser rather than the
  reader, so it should check the *falsifiable* claims only and leave the prose alone.
- **Widened at iteration 48, with evidence that the convention has now failed twice.** The product
  owner's 2026-08-19 feedback moved the capability narrative out of these two fields into
  `docs/CAPABILITIES.md`, which makes the fields short enough to check — but it also created a
  second hand-written document making falsifiable claims about the build. Writing it surfaced that
  `README.md`'s capability sections had gone stale without anyone noticing: they stopped at
  `adr/0026` and omitted twelve shipped commands across roughly fifteen iterations. So the check
  should cover three things, all mechanical: the phase counts in `BUILD-PLAN.md`'s `**Status:**`
  line, that no field names a phase complete while a box in it is unchecked, and that every command
  word in `openbiz-server`'s `USAGE` constant appears somewhere in `CAPABILITIES.md`. The third is
  the same one-directional gap `UNTESTED.md` records against the CLI usage test, closed at the
  documentation level instead. Prose about *behaviour* still cannot be checked and should not be
  attempted.
- **Suggested phase:** Phase 0, as a harness item.

### Bring the Oxigraph benchmark spike forward, ahead of the remaining Phase 1 items

- **Status:** deferred — **overtaken by events, iteration 11.** The reordering this asked for never
  happened and is now moot: the spike ran at its own scheduled place in the plan, because the two
  items ahead of it (SPARQL Update, Graph Store Protocol) turned out to be deferred on charter
  grounds rather than merely hard. Worth recording that the proposal's own argument was therefore
  never tested — the spike did not have to be pulled forward, so we do not know whether it should
  have been. **One of the five numbers it had accumulated is now measured** (query evaluation at
  10k/100k/1M, `adr/0013`, plus load rate and disk, which were not on the list). **Four are not** —
  see the re-proposal below rather than treating this entry as closed.
- **Why:** this is the same doubt for the second iteration running, which is the signal the loop log
  exists to surface. Iteration 6 shipped `GET /api/graphs` returning the *whole* registry from an
  unmeasured pattern scan and closed with "I do not know whether I have designed the contract or
  merely deferred it". Iteration 7 shipped a write lock that serialises every writer and closed with
  "I have measured that the lock is necessary and not measured what it costs". Both are load-bearing
  shapes, both are now underneath callers, and both were decided on reasoning rather than numbers
  because the spike that would produce the numbers is item 9 of 11.
- **What it changes:** the spike currently owes four numbers rather than the one it was written for —
  query evaluation at 10k/100k/1M (its original scope), `close()` flush cost at those sizes,
  registry scan cost on the `GET /api/graphs` hot path, and concurrent write throughput plus lock
  wait time under the `adr/0009` lock. Three of those four were added by items built *after* the
  spike was scheduled, which is itself the argument: each further item adds a number the spike owes
  and a caller written against an unmeasured assumption.
- **Amended, iteration 8:** five numbers now, not four — export wall-clock and peak RSS per syntax
  joins the list, because `GET /api/export` buffers a whole graph into a response body. And the cost
  argument below has **weakened by half**: it said the spike should wait because serialisation would
  make a large store easy to populate, and serialisation landed this iteration — but only the
  *write* half of it. The parser is deliberately deferred behind the candidate seam, so a fixture
  still cannot be loaded from a Turtle file. The honest position is that the reason to wait is now
  one item (the parser) rather than four, and that item is itself blocked on Phase 2.
- **What it costs:** the remaining Phase 1 items before it (parsing, SPARQL query, SPARQL update,
  Graph Store Protocol) are the ones that make a large store *easy to populate*, so running the
  spike first means building its fixtures by hand instead of loading a Turtle file. That is real
  work and it is the honest reason the spike was placed where it is. The counter-argument is that
  SPARQL query evaluation is the specific thing upstream documents as unoptimised, so building the
  query endpoint before measuring it is the least reversible order available.
- **Why this is not the loop's call:** phases are ordered by dependency and `CLAUDE.md` §7 says the
  loop does not promote its own proposals or reorder the plan. Reordering on the strength of the
  loop's own recurring unease is exactly the scope creep that brake exists to stop.
- **Opened:** iteration 7

### Measure the four store costs the benchmark spike did not cover

- **Status:** proposed.
- **Gap:** `adr/0013` measured SPARQL query evaluation, which was the spike item's own scope. Four
  other numbers had accumulated against that spike between iterations 6 and 8, each one a
  load-bearing shape that went underneath a caller on reasoning rather than measurement, and none
  of them is a query: **(1)** `Store::close()` flush cost at 100k and 1M — an operator's
  `SIGTERM`-to-exit time, which `adr/0006` promises is graceful; **(2)** the registry pattern scan
  on the `GET /api/graphs` hot path, which runs on every page load and was never measured against
  a store with many graphs; **(3)** concurrent write throughput and lock wait time under the
  `adr/0009` write lock, which serialises every writer and whose cost iteration 7 explicitly closed
  as unmeasured; **(4)** export wall-clock and peak RSS per syntax, since `GET /api/export` buffers
  a whole graph into a response body.
- **Why load-bearing:** three of the four are on paths a user hits constantly rather than
  occasionally, and (3) is the one that decides whether a second author is a colleague or a queue.
  The spike's harness now exists and generates stores at all three sizes, so the marginal cost of
  each of these is small in a way it was not before — which is the main reason to do them now and
  not to let them accumulate against a later spike the same way they accumulated against this one.
- **Cost & impact:** ~1 iteration for all four, reusing `crates/openbiz-store/src/scale.rs`. (3)
  needs a concurrent driver, which is genuinely new. No runtime impact — measurement only.
- **Suggested phase:** Phase 1, immediately after the literal-precision spike, or Phase 13.

### Maintain `skos:hasTopConcept` in the SKOS authoring model

- **Status:** proposed.
- **Gap:** `adr/0013` measured the concept tree's first query. Finding its top concepts by
  `FILTER NOT EXISTS { ?c skos:broader ?p }` costs **21.6 s at a million concepts**, and — worse —
  is **served rather than refused**, because 21.6 s fits inside the 30 s deadline. The same
  question asked of a scheme that *states* its top concepts with `skos:hasTopConcept` is **0.6 ms,
  flat at every size**. That is not a tuning difference; it is a modelling one.
- **Why load-bearing:** it decides what Phase 2's authoring model writes and what Phase 3's tree is
  allowed to assume, and both are cheap to get right now and expensive to retrofit. Concretely the
  proposal is two things: **(a)** the authoring path *maintains* `skos:hasTopConcept` as concepts
  are created, deleted, and re-parented, so the assertion is always true rather than usually true —
  which makes it an integrity condition with a validation rule, not an optimisation; and **(b)**
  Phase 3 must **not** silently fall back to the derived query when the assertion is missing,
  because a vocabulary imported from an incumbent very often will not state its top concepts, and
  those are exactly the vocabularies a migrating customer opens first. A silent fallback moves the
  21 s from every vocabulary to the imported ones and makes the migration path the slow path.
- **Cost & impact:** small inside Phase 2 if taken with the model; a rewrite of the tree if taken
  after Phase 3. The maintenance rule needs care where a concept has more than one parent or where
  `skos:broader` is asserted only in one direction.
- **Suggested phase:** Phase 2, with the SKOS integrity conditions.

### Give queries a human is waiting on their own budget, separate from the runaway guard

- **Status:** proposed.
- **Gap:** `QueryLimits::DEFAULT_TIMEOUT` is 30 s and `adr/0013` shows why one bound cannot do both
  jobs. Nothing measured came within a factor of ten of 30 s except the tree's first query, which
  reached 72 % of it and was **served**. So the deadline works as what it is — a stop on an
  accidental cartesian product — and is no protection at all against an interface that takes
  twenty-one seconds to draw its first screen.
- **Why load-bearing:** without a second, much smaller bound applied to interface-issued queries,
  "the product feels broken" is a state the server will happily sustain rather than surface. It also
  needs to *refuse legibly* — the honesty rule that governs the row cap applies here too: a user who
  waits should be told the query was abandoned, not left guessing.
- **Cost & impact:** small — `QueryLimits` is already a parameter, so this is a second constant and
  a caller that picks between them. The judgement is in the number and in what the interface does
  with the refusal, not in the plumbing.
- **Suggested phase:** Phase 3, with the first screen that issues a query.

### Index labels, because `LIMIT` does not bound a `STRSTARTS` search

- **Status:** proposed.
- **Gap:** the search box's query — `FILTER(STRSTARTS(LCASE(STR(?label)), …))` with `LIMIT 50` —
  costs 6.4 ms at 10k, 51.8 ms at 100k, and **479.6 ms at 1M** (`adr/0013`). It returns fifty rows
  at every size, so the cost is **linear in the graph, not in the answer**: the `LIMIT` bounds what
  comes back and nothing else, because a string function in a `FILTER` cannot use an index. Every
  `skos:prefLabel` in the vocabulary is read, decoded, lower-cased, and tested. That is half a
  second per keystroke at a million concepts, before any network.
- **Why load-bearing:** search is how anyone finds anything in a large vocabulary, and it is also
  §1.7's load-bearing feature — a user who cannot find the existing concept creates a duplicate,
  which is the silo this product exists to attack. There is no SPARQL-level fix and SPARQL 1.1 does
  not standardise full-text search, so this is a real component decision (an embedded index such as
  `tantivy`, kept in step with the store) rather than a query rewrite.
- **Cost & impact:** several iterations, and it adds a dependency with a licence check and an
  index-consistency problem of its own — a second copy of the labels that can drift from the store.
  Not a nice-to-have: at 1M concepts type-ahead is unusable without it. Must degrade to the scan
  when no index exists, so a fresh or restored store still searches.
- **Suggested phase:** Phase 13, or earlier if Phase 3's search screen needs it to be honest.

### Build a SPARQL query console in the interface
- **Status:** proposed.
- **Gap:** `/api/sparql` is live, tested, and answers in all four results formats and all six RDF
  syntaxes — and nothing in the interface calls it. A taxonomist cannot run a query in OpenBiz; they
  need `curl` or an external SPARQL client. `CLAUDE.md` §4.4 requires anything user-facing to be
  reachable in the UI and keyboard-navigable, and a query console is plainly user-facing. The
  endpoint item was scoped to the endpoint, so this is recorded rather than folded into it.
- **Why load-bearing:** it is the first screen in the product where a user *does* something rather
  than reads a list, and it is the only way to see vocabulary content at all until Phase 2's
  authoring path exists. It also exercises three server behaviours that currently have no human
  reader: the format list, the refusals (a 406, a 413, a 503 with its reason), and the
  `preserves_term_detail` warning that says CSV silently drops language tags — which is exactly the
  warning that is worthless in an API and valuable at the point of choosing a download format.
  Without it the endpoint's careful error messages are read only by tests.
- **Cost & impact:** one iteration. Depends on nothing not already built — the endpoint, the format
  list, and the graph registry are all served. Should reuse the export item's format chooser rather
  than growing a second one.
- **Suggested phase:** Phase 1 (alongside the endpoint) or early Phase 3 (the interface phase), and
  the loop has no view on which — that is the judgement being asked for.

### Decide what to do about the store rewriting literal lexical forms
- **Status:** proposed.
- **Gap:** the store returns a *different RDF term* from the one written, for every literal whose
  datatype the engine models natively. `"007"^^xsd:integer` comes back `"7"`;
  `"1.663E-4"^^xsd:double` comes back `"0.0001663"`; `"1"^^xsd:boolean` comes back `"true"`. Two
  triples differing only in an object's lexical form collapse to one, so a graph loses statements.
  Nothing tells the user. Measured and pinned in iteration 10 — see `adr/0012` and `UNTESTED.md`.
- **Why load-bearing:** it breaks `CLAUDE.md` §1.3 — an artefact that does not survive a write and a
  read has not round-tripped, never mind round-tripped through somebody else's tool. Zero-padded
  notation codes are ordinary in enterprise classification schemes, so this is not an exotic edge
  case. And silence is the specific thing the charter's wedge attacks: we say the incumbents' export
  is lossy in ways they do not disclose, and here we have a larger undisclosed loss. **The loop
  cannot decide this alone** — the three options have very different costs and one of them is a
  judgement about a dependency the whole product rests on:
  1. **Upstream.** Get Oxigraph to preserve the lexical form. Cheapest for us, slowest and least
     certain, and it is a dependency on somebody else's roadmap.
  2. **Our own term encoding.** Keep the original lexical form beside the value. Real work in
     `openbiz-store`, a store format bump, and it puts us on a fork of the engine's data model —
     which is the "swap the engine later" cost `CLAUDE.md` §3 was written to avoid paying. Note
     this is the *expensive* option: `the_rewrite_is_the_stores_and_not_the_exports` shows the loss
     is in the term encoding rather than in the serialiser, so a fix touches stored data and every
     existing store needs a migration or a rebuild.
  3. **Accept and disclose.** Say it in the API, the export, and the interface, the way
     `records_graph_names` already says its smaller thing. Cheap, honest, and **not a fix**: a
     governance team cannot sign off a vocabulary whose notations silently changed.
  There is a fourth that is not on the table without a much larger decision — a different store —
  and naming it is part of what makes 1–3 a real choice rather than a foregone one.
- **Cost & impact:** option 3 is one iteration. Option 2 is several and touches the format version.
  Option 1 is unbounded. Whichever is chosen, the disclosure work in option 3 is worth doing first
  because it is a prerequisite for being honest while the rest is decided.
- **Suggested phase:** Phase 1. It is a property of the substrate and every later phase inherits it.
- **Amended, iteration 13:** the spike below found a **larger** instance of the same loss — the
  datatype IRI of a derived integer type is dropped, not just the lexical form — so whichever option
  is chosen here has to cover that too. Read the two together; they are one decision.

### Measure the core model's size before the transitive closure is built on top of it
- **Status:** deferred — **overtaken by events, iteration 26.** The measurement was taken and the
  decision recorded, without this proposal being promoted, because the build-plan item it was a
  prerequisite of said so itself: *"a decision on the closure's size taken against the measurement
  `docs/UNTESTED.md` now asks for"* is a clause of "Semantic relations, part 2", so taking it was
  splitting an in-plan item rather than promoting a proposal. The numbers and the decision are in
  `docs/adr/0024-semantic-relation-closure-scale.md`; the harness is `openbiz-skos`'s `scale`
  module. **The decision went the way this proposal guessed it might:** S24's closure is answered
  on read and never stored. What the measurement additionally found — that the *existing*
  materialisation costs 3.9 KiB per link and 4.4 GiB at a million — is genuinely new and is the
  three proposals immediately below, which are *not* self-promoted and want a human.
- **Superseded status:** proposed.
- **Gap:** iteration 24 put semantic relations into `openbiz-skos`, and they are the first thing
  in the crate that scales with a vocabulary's **size** rather than with its structure. The
  closure materialises every stated link under four properties — the one written, its inverse
  under S25, and both transitive variants under S22 — so a 100k-link thesaurus holds roughly 400k
  `(Node, RelationOrigin)` entries with the IRIs cloned into each, plus three derivations per link
  in a `Vec<String>`-shaped list that `openbiz inspect` prints in full and without a cap.
  `CoreModelBuilder`'s doc comment still says what is kept is "proportional to the resources the
  model has something to say about rather than to the size of the graph". That was true when
  labels and notes were counted and dropped. It is now narrower than it reads.
- **Why load-bearing:** the very next build-plan item is S24's transitive closure, which is
  superlinear in exactly this data — quadratic in the worst case for a deep hierarchy — and it
  will be built on whatever shape is there when it arrives. If the right answer turns out to be
  "store one direction and answer the other on read", that is a much cheaper change to make
  *before* the closure than after, and it cannot be decided without a number. The scale spike in
  `adr/0013` measured the store; nothing has ever measured the model.
- **What is being asked for:** peak resident memory and wall clock for `openbiz inspect` at 10k,
  100k and 1M links, on both a shallow-and-wide and a deep-and-narrow hierarchy, plus the size of
  the report it prints. Then a recorded decision, in an ADR, on whether to keep materialising.
- **Why the loop is not deciding it:** the loop can take the measurement — it is one item — but
  the decision it feeds is an architectural one about the core model's shape, and picking a new
  shape from an unmeasured fear is the other way to get this wrong. It is also arguably a
  re-ordering of Phase 2, which is a plan change and not the loop's to make.
- **Cost & impact:** one iteration for the measurement. The change it might imply is larger, and
  cheaper now than in three items' time. No new dependency.
- **Suggested phase:** Phase 2, immediately before "Semantic relations, part 2".

### Decide where entailments live, because our answers and our exports now disagree
- **Status:** proposed.
- **Gap:** `openbiz inspect` reports facts nobody stated — a concept scheme found under S5, a
  `skos:member` found under S36, `skos:inScheme` found under S7. `openbiz export` and
  `GET /api/export` hand out the **asserted** graph, so none of those statements are in the file a
  customer downloads. Both behaviours are individually defensible and were individually argued for
  (`adr/0019`, and iteration 20's log). Together they mean the same vocabulary answers two
  different questions two different ways, and **nothing in the build tells the person holding the
  export that this is so**. S11 — every SKOS label entails an `rdfs:label` — is the newest case
  and the most visible one: a generic RDF browser, a DCAT catalogue, or a SPARQL query written
  against `rdfs:label` finds nothing in our export, silently.
- **Why load-bearing:** this is not a tidy-up. "Report zero where a real vocabulary has thousands"
  is the exact failure iteration 20 built the entailment path to prevent, and the export path has
  it, aimed outwards. It is also a decision with no default that is merely conservative:
  materialising entailments puts statements into a customer's vocabulary that they never wrote and
  cannot delete, which iteration 20 argued against and this proposal does *not* ask to reverse.
  The likely answer is a third option neither iteration has designed — an entailed *view*, selected
  at export and at query time, so a caller says which one they want and the file says which one it
  is. That needs deciding before Phase 4's SHACL validation, which will otherwise have to pick one
  silently, and before Phase 8's Git export, where the diff a reviewer reads depends on it.
- **Cost & impact:** one iteration to decide and write the ADR; probably two to three to implement
  across export, SPARQL, and the report, and it touches the candidate seam's apply step — a place
  now touched four times for four reasons, which is itself a signal. No new dependency. The runtime
  cost is whatever the chosen shape is: recomputing per request, a cache invalidated at the seam,
  or a materialised graph kept separate from the user's.
- **Why the loop is not deciding it:** it has been the "still uncertain" line of iterations 18, 20
  and 21, which by the loop's own rule (iteration 18) makes it a design change rather than a
  nuisance — and the cost of getting it wrong falls on a customer's exported data, not on us.
- **Amended at iteration 22, and the case is now much stronger.** SKOS-XL landed, and S55–S57 mean
  a thesaurus authored in SKOS-XL — which is the ISO 25964 customer, the one this product is aimed
  at — has **no plain SKOS labels at all** in the file we export. Not a missing `rdfs:label` on a
  concept that already has a `skos:prefLabel`, as S11 was: no labels. Every generic RDF tool a
  customer points at that export sees an unlabelled thesaurus, and the dumbing-down that Appendix
  B.3.4.1 exists to provide is a thing only OpenBiz performs. It is no longer plausible to call
  this a tidy-up, and it should be decided before the concept tree rather than at the latest
  before Phase 4.
- **Amended at iteration 23 — a third instance, and no new argument.** S62's symmetric closure
  produces label links that are in our answers and not in our exports, exactly as S11's and
  S55–S57's entailments are. Recorded so the count is honest; the case above is unchanged and the
  urgency is not raised again, because raising it a second time would be padding rather than
  evidence.
- **Suggested phase:** Phase 2, **before** the concept tree.

### Read `rdfs:subPropertyOf`, so a refinement of `skosxl:labelRelation` is not invisible
- **Status:** proposed.
- **Gap:** Appendix B.4.1 says `skosxl:labelRelation` "is not intended to be used directly, but
  rather as an extension point which can be refined for more specific labeling scenarios", and
  Example 89 refines it to `ex:acronym`. That is how ISO 25964's label relationships actually
  reach SKOS-XL — an acronym, a spelling variant, a translation pairing — so **the ordinary use of
  B.4 is the one we cannot read**. Iteration 23 applied S59–S62 to the property itself; a
  vocabulary that uses a refinement instead gets no links at all, and `openbiz inspect` omits the
  link line rather than saying "there are links here in a vocabulary I do not read".
- **Why load-bearing:** it is the "reports zero where a real vocabulary has thousands" failure
  again, and this time it is aimed at exactly the customer the product is for. A thesaurus
  migrated from ISO 25964 will express its label relationships through refinements, and our report
  will say the labels are unlinked. That is indistinguishable from a correct answer, which is what
  makes it worse than an error.
- **What is *not* being asked for:** closing the refinement itself. B.4.4.1 is explicit that "a
  sub-property of a symmetric property is not necessarily symmetric" — "FAO" is an acronym for
  "Food and Agriculture Organization" and the converse is false — and a test already asserts we do
  not. The sound step is only that a refinement's statement entails the *super*-property's, which
  S62 may then close.
- **Why the loop is not deciding it:** because the honest implementation is not a special case for
  one property in `openbiz-skos`. RDFS sub-property reasoning is either the reasoner's job
  (`openbiz-owl`, Phase 5) or a SHACL rule pack's (Phase 4), and choosing between them is the same
  standing question as "where do entailments live" above — which is a human's to settle, not a
  fifth iteration's to guess at. Building it into the SKOS crate to close the gap quickly would
  put an inference path somewhere it will have to be moved from.
- **Cost & impact:** small if it lands in the reasoner alongside other RDFS entailments; wrong at
  any price if it lands as a hard-coded arm in the SKOS builder. No new dependency either way.
- **Suggested phase:** Phase 4 or Phase 5, decided together with the entailment-location proposal
  above rather than separately.

### Decide what to do about the store dropping derived integer datatypes
- **Status:** proposed.
- **Gap:** `"5"^^xsd:int` is stored, and returned, as `"5"^^xsd:integer`. So are `xsd:short`,
  `xsd:byte`, `xsd:long`, `xsd:unsignedLong`, `xsd:nonNegativeInteger`, and `xsd:positiveInteger`
  whenever the value fits `i64`. Under RDF 1.1 a literal is the pair (lexical form, datatype IRI),
  so these are different terms being conflated: four distinct terms written against one subject and
  one predicate come back as **two statements**. Measured and pinned in iteration 13 — see
  `adr/0014` and `UNTESTED.md`.
- **Why load-bearing:** this is strictly worse than the lexical rewriting above, for two reasons.
  First, a datatype IRI is what every downstream constraint language *names*: a SHACL
  `sh:datatype xsd:int` shape (Phase 4) can never be satisfied by data in this store, and an OWL 2
  datatype range over a derived type (Phase 5) is untestable. Those are two whole phases inheriting
  a defect from the substrate. Second, it loses **statements** rather than merely altering them, on
  input a taxonomist would call unremarkable, with nothing anywhere reporting it — so a vocabulary
  that went in with four assertions comes out with two and the diff a reviewer approves is against
  the two. `CLAUDE.md` §1.3 requires an artefact to round-trip through a standards-compliant tool;
  this does not round-trip through *ours*.
- **The one thing to do before choosing:** `UNTESTED.md` records that **nobody has looked upstream**.
  `adr/0014` argues the *range* boundary is expensive to move because it is a property of the value
  representation, and that argument is sound — but it does not transfer to the datatype
  substitution, which may be a `Literal` construction detail or an upstream bug and therefore cheap.
  Half an iteration reading `oxigraph::model::Literal` and the upstream issue tracker would turn
  this from a three-option commercial judgement into, possibly, a patch. **That reading should
  happen before this proposal is ruled on**, and before the Phase 4 SHACL spike, which will
  otherwise blame the SHACL engine for it.
- **Cost & impact:** unknown until the reading above is done, which is why the reading is the
  recommendation rather than an option. If it is not cheap, the options are the same three as the
  proposal above and should be decided together.
- **Suggested phase:** Phase 1, ahead of Phase 4.

### Write our own N-Triples serialiser, or get the escaped tab fixed upstream
- **Status:** proposed.
- **Gap:** our N-Triples output violates one of Canonical N-Triples §4's five constraints — it
  writes `\t` for a tab, where §4 requires characters that `STRING_LITERAL_QUOTE` admits directly to
  be written directly. Valid N-Triples, not canonical N-Triples. Measured in iteration 10; see
  `adr/0012`.
- **Why load-bearing:** honestly, **less than the entry above**, and saying so is the point of this
  file. Nothing is lost and no consumer breaks. What it costs is the claim that two tools serialising
  one graph produce identical bytes, which is what makes a vocabulary reviewable as a git diff —
  a charter pillar, but one nothing in the product depends on yet, because there is no git
  integration until Phase 8. Promoting this ahead of the lexical-form decision would be the wrong
  order.
- **Cost & impact:** N-Triples is the simplest of the six syntaxes and writing a conforming
  serialiser is perhaps half an iteration, with the conformance checker from iteration 10 already in
  place to prove it. But it means one of the six no longer goes through the engine's writer, which
  is an inconsistency a reader would have to re-derive — so it is probably only worth doing as part
  of the decision above, or not at all.
- **Suggested phase:** Phase 1, or deferred to Phase 8 when git integration makes the diff matter.

### Run the W3C rdf-tests suites against all six serialisations
- **Status:** proposed.
- **Gap:** iteration 10 covered two of the six syntaxes with a reader written from the published
  EBNF. The other four — Turtle, TriG, RDF/XML, JSON-LD — are still proven only by being re-read by
  the library that wrote them, which is self-consistency rather than conformance. Their grammars are
  far too large to transcribe the way N-Triples' was.
- **Why load-bearing:** `CLAUDE.md` §4.5 requires a standards claim to rest on the spec's own tests,
  and Turtle is our **default** export format — the one most users will accept without reading
  further. The two defects found the moment a genuinely independent check existed for N-Triples are
  the argument: neither was visible to a round trip, and there is no reason to think the other four
  are cleaner, only that nothing has looked.
- **Cost & impact:** one to two iterations. The corpus is dual-licensed W3C Test Suite / BSD-3-Clause
  — BSD-3-Clause is permitted by §5 outright, so vendoring a subset is a licence question with a
  known answer, but it should still be recorded per §6's fixture rule. The manifests are RDF, which
  we can already read. Wiring it into CI is the part with real cost, since it should not be
  a download at test time (§1.1, air-gapped).
- **Suggested phase:** Phase 1.

### Serve an authenticated online backup, so a deployment need not stop to be copied
- **Status:** proposed.
- **Gap:** `openbiz backup` needs the store to itself, because the embedded store takes an
  exclusive lock. Backing up a deployment therefore means stopping it. For a product whose pitch is
  "one binary, one process", that turns a routine nightly job into a service window — and it is the
  one place where the incumbents' separate triplestore is genuinely an advantage, because theirs
  can be snapshotted underneath the running app.
- **Why load-bearing:** load-bearing for a production deployment, not for the build. A
  data-governance team writes the disaster-recovery runbook before they sign, and "stop the service
  nightly" is a real objection in a regulated shop. The store already supports what is needed —
  the scan is a single snapshot — so the work is an authenticated `GET /api/backup` that streams
  N-Quads, plus the read transaction spanning the registry read and the scan (`UNTESTED.md`).
- **Cost & impact:** roughly one iteration for the endpoint once authentication exists, which it
  does not. **Depends on Phase 6's authorisation model**; proposing it now so that model is
  designed knowing a whole-store read is one of its subjects. Doing it *before* authentication
  would ship an unauthenticated way to exfiltrate the entire customer's data, which is why the loop
  did not.
- **Suggested phase:** Phase 6.

### Verify a backup without restoring it
- **Status:** proposed.
- **Gap:** the only way to find out whether a backup file is good is to restore it into a fresh
  data directory, which needs disk and a stopped moment. An operator's nightly job can therefore
  report "backup written" for months while writing something that will not restore — truncated by
  a full disk, corrupted in transit to object storage, or an export somebody put in the wrong
  place. Every refusal `Store::restore` makes is already computable from the file alone.
- **Why load-bearing:** an untested backup is not a backup, and this is the cheapest way there is
  to make "we test our backups" a true sentence in a runbook. It is a small feature — a
  `openbiz verify <file>` that runs the parse, the stamp check, the graph classification, and the
  registry read-back against an in-memory store, and prints what it would have restored.
- **Cost & impact:** well under one iteration; the logic exists and needs the transaction swapped
  for a dry run. Note the honest limit: verifying without writing proves the file is *acceptable*,
  not that the target disk can hold it.
- **Suggested phase:** Phase 1 or Phase 6.

### Decide whether a backup should be compressed, and by what
- **Status:** proposed.
- **Gap:** a backup is uncompressed N-Quads, which is the most verbose sensible form of RDF — every
  statement repeats its full IRIs. A million-concept vocabulary is a large text file, and nobody
  measured how large. Operators will pipe it through `gzip` themselves, at which point the product
  has an undocumented convention instead of a decision.
- **Why load-bearing:** nice-to-have, and say so plainly. The reason to raise it as a decision
  rather than just doing it is that compressing *inside* the product costs the property the format
  was chosen for: a `.nq.gz` is no longer something an operator can `grep`, `head`, or eyeball, and
  a compression dependency is a dependency in the `CLAUDE.md` §1.5 sense. The alternative — read a
  `.gz` on restore but never write one — is a smaller commitment that solves the practical half.
- **Cost & impact:** under one iteration either way. Wants the size measurement from the 1M-concept
  restore in `UNTESTED.md` first, so the decision is taken against a number.
- **Suggested phase:** Phase 1.

### Keep a store fixture from every released format version, from the first release on
- **Status:** proposed.
- **Gap:** the migration tests build their "version 1" store by degrading a version-2 one — clearing
  the system graph and rewriting the stamp. That is our *belief* about what version 1 looked like,
  written by the same build that reads it. Version 1 never shipped, so today the gap is theoretical.
  From the first release it stops being theoretical: every later migration will be tested against a
  fixture the current build invented rather than against a store a real build wrote.
- **Why load-bearing:** a migration is the one operation that runs on data we did not write, at the
  moment an operator is least able to recover, and it is the operation most likely to be tested only
  against its own assumptions. A corpus of real per-version fixtures is the only thing that makes
  "we migrate from 1.2" a claim rather than a hope. It is cheap to start and impossible to backfill —
  once a release is gone, so is the store it wrote.
- **Cost & impact:** near zero per release (one `openbiz backup` of a small seeded store, committed
  as a fixture), and it wants a decision now because the cost of starting late is total. Needs a
  release process, which does not exist yet — hence a proposal, not an item.
- **Suggested phase:** Phase 1.

### Decide and document how many store-format versions back a build migrates from
- **Status:** proposed.
- **Gap:** the migration chain refuses a store it has no path for and tells the operator to "upgrade
  one release at a time". Nothing says how far back the chain will ever reach, and nothing guarantees
  the intermediate builds are obtainable — which is what makes that instruction actionable or empty.
- **Why load-bearing:** an enterprise buyer running an air-gapped, self-hosted deployment upgrades
  in jumps, sometimes years apart. "Which versions can I upgrade from?" is a question they ask
  before signing, not after. It is also the constraint that decides whether an old migration may
  ever be deleted, which is a code-lifetime question the loop will otherwise answer by accident.
- **Cost & impact:** a paragraph of policy plus, possibly, a `docs/` page. The policy is a
  commercial and support commitment, so it is a human's call (`CLAUDE.md` §8), not the loop's.
- **Suggested phase:** Phase 1.

### Decide the OWL 2 model dependency, and amend the charter text that names `horned-owl`
- **Status:** proposed.
- **Gap:** `horned-owl` is LGPL-3.0 (verified 2026-08-18, three ways — see `docs/BLOCKED.md`), which
  `CLAUDE.md` §5 forbids in the core. Yet §3 names it as the OWL 2 candidate, §5 uses it as the
  example of a licence that is *merely unlisted*, and `docs/BUILD-PLAN.md`'s Phase 9 item specifies
  it by name. Three places in the standing brief now point at a dependency the standing brief bans.
- **Why load-bearing:** §5's rule is that a copyleft dependency is a human's decision, and §8 puts
  licensing decisions out of loop scope, so the loop **cannot** resolve this — but it also cannot
  start Phase 9 without it resolved, and it cannot leave the charter saying two contradictory
  things. The four options and their costs are enumerated in the `BLOCKED.md` entry; this proposal
  is the request to pick one and to correct §3, §5, and the Phase 9 item in the same commit as the
  ADR. It is not urgent — Phase 9 is six phases out and nothing depends on the crate today — but it
  is the kind of decision that gets expensive the day someone has already written the plan against
  the banned dependency.
- **Cost & impact:** the decision itself is a human sitting with the four options for an hour, plus
  possibly legal input on option 1. The *engineering* cost ranges from near-zero (option 1) to
  several phases (option 2, writing our own OWL 2 structural model and IO). Whichever is chosen, an
  ADR and a three-file charter amendment follow. No runtime impact today.
- **Suggested phase:** Phase 9 — but the decision should be taken well before Phase 9 begins.

### Say which SHACL version the Phase 4 spike is testing, and add two engines to its list
- **Status:** proposed.
- **Gap:** the Phase 4 spike item says "evaluate `oxirs-shacl` vs `shacl_validation` vs in-house
  against the W3C SHACL test suite". It does not say **which SHACL**. SHACL 1.0 (2017) is the only
  Recommendation, but the Data Shapes WG now has SHACL 1.2 Core in Working Draft (2026-08-03) with
  node expressions, `sh:values`, `sh:defaultValue`, per-constraint severity via reification,
  `sh:targetWhere` and list constraints — a materially larger surface. A spike that measures three
  engines against an unnamed target produces a number nobody can interpret later. Separately, the
  candidate list is now out of date: `purrdf-shapes` (MIT/Apache, 2026-08-02, claims native RDF 1.2
  SHACL) did not exist when the item was written, and the adoption gap between the two named
  engines is 20× (`shacl_validation` 44k downloads, `oxirs-shacl` 2.0k) which is a fact the spike
  should weigh rather than discover.
- **Why load-bearing:** SHACL is not one feature among many for us — `docs/METHODOLOGY.md` makes
  **gate criteria SHACL shapes**, so the Methodology Engine, the rule packs, and validation-on-write
  all sit on whichever engine this spike picks. Choosing it against a target we did not write down
  is how we end up unable to say what we support. **This is emphatically not a proposal to chase a
  Working Draft** — the recommendation is to conform to SHACL 1.0 and to record 1.2 divergence as a
  known-forward-compatibility note, so that when 1.2 reaches Recommendation we know what we owe.
- **Cost & impact:** amends an existing plan item rather than adding one; perhaps half an iteration
  of extra spike work to run the fourth engine and to write the version statement.
- **Suggested phase:** Phase 4.

### Look at SHACL 1.2 UI (`shui:`) before Phase 3 invents a form-description vocabulary
- **Status:** proposed.
- **Gap:** Phase 3 will need to decide, for every property of a concept, what widget renders it,
  in what order, under what group, with what label in which language. Every tool in this market
  solves that, and TopBraid solves it by driving forms from shapes. W3C now has a standards-track
  answer: **SHACL 1.2 User Interfaces**, First Public Working Draft 2026-05-26, defining a `shui:`
  vocabulary with widget selection by scoring, grouping and ordering, cross-language label
  resolution, property roles, and 16 built-in editors plus 10 viewers. Nothing in our plan mentions
  it, so the default outcome is that Phase 3 invents a private vocabulary for the same job.
- **Why load-bearing:** `CLAUDE.md` §1.3 says we implement standards rather than inventing
  proprietary substitutes for things already standardised, and this is precisely a thing being
  standardised. It also pays off twice — the same annotations that render our editor render a
  customer's own rule pack's forms, which is the "custom organisation rule packs, authored in the UI
  without hand-writing SHACL" item in Phase 4. **The honest caveat:** an FPWD is early, it will
  change, and building the UI on it would be a mistake. The proposal is narrower — *read it, and
  where we need a vocabulary for something it covers, use its terms rather than minting ours*, so
  that a later migration is a version bump rather than a rewrite.
- **Cost & impact:** roughly one iteration to read the FPWD and write an ADR mapping our Phase 3
  form needs onto `shui:` terms, marking what it does not cover. Blocks nothing.
- **Suggested phase:** Phase 3, before the concept-detail item.

### Re-cite the ISO 25964 rule pack and methodology pack against the 2026 revision
- **Status:** proposed.
- **Gap:** **ISO 25964-1 is being revised and publication is expected in 2026.** The revision went
  to comment and vote on 2024-07-30 and TC 46's work is reported complete; announced changes include
  GUIDs, expanded non-Latin-script examples, DEI guideline references, the addition of "concept" and
  "concept term", and substantial annexe updates. We cite the 2011 edition in `CLAUDE.md` §2, in
  `docs/METHODOLOGY.md`'s `iso-25964-thesaurus` pack, and in the Phase 4 rule-pack item.
  (ISO 25964-2:2013 was confirmed in 2023 and is unaffected.)
- **Why load-bearing:** `CLAUDE.md` §7's review rule says in as many words that **a pack which
  misrepresents its source methodology is worse than no pack**, and a rule pack sold as ISO 25964
  conformance while checking a superseded edition is that failure exactly — in front of the buyer
  who cares most, since ISO 25964 in a requirements document is why they are evaluating us. It is
  also the one finding in this pass with a real deadline attached to somebody else's calendar.
- **Cost & impact:** the standard is paywalled, so acquiring it is a purchase decision and therefore
  a human's (`CLAUDE.md` §8). Until then the honest move is cheap and should happen regardless:
  make every citation say **"ISO 25964-1:2011"** rather than "ISO 25964", so the edition we actually
  implement is stated rather than implied. That part is an hour. Re-basing the pack on the new
  edition is a separate, larger item that cannot start without the text.
- **Suggested phase:** Phase 4 for the rule pack; the citation tightening could ride any iteration.

### Use Oxigraph's RDFC 1.0 canonicalization for vocabulary-as-code diffs
- **Status:** proposed.
- **Gap:** Phase 8 promises reviewable diffs of vocabularies in git. Serialising a graph naively
  gives a diff dominated by statement reordering and renamed blank nodes — noise that makes a PR
  unreviewable, which would fail the wedge row it exists to deliver. Nothing in the Phase 8 items
  addresses canonicalisation. Meanwhile **Oxigraph 0.5.4 added RDFC 1.0**, the W3C canonicalization
  algorithm, and we already ship Oxigraph and are locked at 0.5.9.
- **Why load-bearing:** "GitHub-native — vocabularies are code: branches, PRs, **reviewable diffs**"
  is one of the seven wedge rows in `CLAUDE.md` §1. A diff a governance reviewer cannot read is the
  feature not working. The unusual thing here is the cost: the capability is already inside a
  dependency we ship, so this is closer to *noticing* than to building — which is exactly why it
  would otherwise be missed until someone opened an ugly PR.
- **Cost & impact:** needs a measurement first — RDFC 1.0's blank-node canonicalisation is
  worst-case expensive on adversarial graphs, and a SKOS vocabulary's blank-node count is usually
  near zero, so the honest guess is that it is nearly free for us and that guess should be checked
  rather than assumed. Perhaps one iteration to measure and one to wire. Note it interacts with the
  open question in "Decide where entailments live" above: canonicalising the *asserted* graph and
  canonicalising the *entailed* one give different files, and Phase 8 must say which is committed.
- **Suggested phase:** Phase 8.

### Add a `SkosmosProvider`, because one connector covers a class of public registries
- **Status:** proposed.
- **Gap:** `adr/0003` lists public registries individually — EuroVoc, AGROVOC, LCSH, SNOMED CT,
  schema.org, IPTC — which implies one connector, one auth story, and one breakage surface each.
  But **Skosmos is the common front end for a large part of that class**: AGROVOC, Finto, and many
  national and institutional thesauri all publish the same REST API. AGROVOC has now retired its
  legacy SOAP services and states it will add no further web services, leaving SPARQL and the
  Skosmos REST API as the only two machine routes.
- **Why load-bearing:** `adr/0003`'s own consequences section names the risk — "each connector is an
  integration with its own auth, rate limits, and breakage" — and a `SkosmosProvider` plus a base
  URL collapses many of those into one implementation with one test. It also lets a customer point
  at *their own* Skosmos instance, which is a real deployment pattern in the public sector. This is
  a nice-to-have in the sense that nothing breaks without it; it is load-bearing in the sense that
  the alternative is N connectors we will not all maintain, and an unmaintained connector reports
  "nothing found", which reads as "nothing exists" — the anti-silo feature failing silently.
- **Cost & impact:** one connector's work for several sources' coverage. Depends on the
  `DiscoveryProvider` trait landing in Phase 2. No connector exists yet, so nothing is broken today.
- **Suggested phase:** Phase 12, with the trait's Phase 2 hook unchanged.

### Decide how `whelk-rs` can be a dependency at all, given it is not published
- **Status:** proposed.
- **Gap:** Phase 5 names `whelk-rs` as the OWL EL reasoner behind our `Reasoner` trait.
  **`whelk-rs` is not on crates.io** (checked 2026-08-18; the repo is alive, MIT-licensed, last
  pushed 2026-06-29, 20 stars). Our own `deny.toml` sets `unknown-git = "deny"`, so a git dependency
  fails CI by policy. This is not a licence problem — MIT is fine — it is a supply problem, and it
  is a *smaller* one than the `horned-owl` entry but it has the same shape: a plan item naming a
  dependency the policy will not accept.
- **Why load-bearing:** it is cheap to resolve and expensive to discover late. The options are ask
  upstream to publish, vendor it with a recorded justification, carve a narrow `[sources]` exception
  for one known git URL, or use one of the newer permissive EL implementations
  (`owl-dl-saturation`, `ontologos-el`) — all of which are much less proven. Recording it now means
  Phase 5 opens with the question already asked.
- **Cost & impact:** the decision is minutes; the consequence is which crate Phase 5 spikes against.
  Nothing depends on it today.
- **Suggested phase:** Phase 5.

### Record the catalog vendors as competitors, not only as discovery connectors
- **Status:** proposed.
- **Gap:** `docs/COMPETITIVE.md` covers PoolParty, metaphactory, TopBraid EDG, Protégé and VocBench
  — the semantic-web-native tools. It says nothing about Collibra, Alation, Microsoft Purview,
  data.world, and their business-glossary modules. `adr/0003` names them, but only as **discovery
  connectors** — sources to read from. They are also where a data-governance buyer's budget usually
  already sits, and "we already have a glossary in Collibra" is the objection our positioning most
  has to answer.
- **Why load-bearing:** it changes what the product has to prove. Against PoolParty we argue
  deployment weight and price; against an incumbent catalog we argue that a glossary of flat terms
  with no `broader`, no scheme, no SKOS export and no integrity conditions is not a vocabulary, and
  that theirs and ours should be **connected** rather than one replacing the other — which is the
  `adr/0003` posture and a genuinely stronger sales position than displacement. The research to
  support that argument does not exist in our files, so today it is an assertion.
- **Cost & impact:** research and writing, no code. Half an iteration. The risk of *not* doing it is
  that a Phase 12 connector gets built on guesses about what those products actually expose.
- **Suggested phase:** not a build phase — a research task for a future product-owner pass, listed
  here so the next one does not have to rediscover the gap.

### Adopt a fetch-on-demand real-thesaurus fixture, and stop calling the shape questions unanswerable

- **Status:** proposed. Written at the product owner's direct request (`FEEDBACK-LOG.md`,
  2026-08-19: *"What I am asking for is a proposal, not a fetch"*), against their four stated
  criteria — licence, size and location, which questions it answers, air-gapped honesty. Nothing was
  added to the repository and nothing was built. **The recommendation is yes, and it is smaller than
  either of us expected.**
- **The framing correction lands, and it was worse than the note says.** Six consecutive iterations
  wrote a doubt whose closing line was some variant of *"it cannot be told from inside this
  repository"*. That sentence was true and it was also a place to stop looking. This iteration spent
  roughly forty minutes with `curl` and a public SPARQL endpoint and came back with **numbers** for
  four of the six. `CLAUDE.md` §8 does not list public test data, and it was never the constraint —
  the constraint was that "I cannot know" reads like diligence and is much cheaper than finding out.

#### What was actually measured, before proposing anything

Two real published vocabularies, no download into the repository (one 763 KB file to `/tmp`, deleted):

| | AGROVOC | LC Genre/Form Terms (LCGFT) |
|---|---|---|
| Concepts | 41,825 | 2,685 |
| Licence | **CC BY 4.0**, asserted by the dataset's own VoID (`dct:license`) | **public domain**, asserted by the publisher |
| Distribution | `agrovoc_core.nt.zip` **69.8 MiB** / `agrovoc_lod.nt.zip` **91.2 MiB**, 10,089,090 triples | `genreForms.skosrdf.nt.gz` **745 KiB** → **10.8 MiB** N-Triples, 90,890 lines |
| Concepts with >1 `skos:broader` | **474 (1.1%)**, maximum 2 | **693 (25.8%)**, maximum 4 |
| Worst-case routes to a summit | not enumerated (endpoint) | **7**, at depth 3 |
| Mapping links | 36,402 `exactMatch`, 13,888 `closeMatch`, 261 `broadMatch`, 72 `narrowMatch`, 13 `relatedMatch` | none |
| SKOS-XL | yes — 1,251,722 `skosxl:Label` | yes, **alt labels only**: 6,121 `skosxl:altLabel`, 6,314 `literalForm`, and plain `skos:prefLabel` |
| `rdfs:subPropertyOf` into SKOS | **21 declared**, of which **2 used** | none |

Method: AGROVOC by SPARQL against `https://agrovoc.fao.org/sparql` and by its VoID descriptor at
`http://aims.fao.org/aos/agrovoc/void.ttl`; LCGFT by fetching the dump and counting it with a
throwaway script. Sizes by `curl -I`, read from `content-length`. None of this is a test and none of
it is in CI — that is the proposal, not the finding.

#### Four findings that change what we believe, independent of whether this is promoted

1. **`PathBound::DEFAULT` is not close to the wire — it is three orders of magnitude away.** The
   iteration-36 entry reasoned that an ordinary thesaurus sits *near* the 10,000-route ceiling. On
   the one real polyhierarchy measured — and LCGFT is genuinely polyhierarchical, a quarter of its
   concepts have more than one parent — the worst concept in the vocabulary has **7 routes to a
   summit**. AGROVOC cannot exceed 2 broader links on any concept at all. The reasoning was not
   merely unmeasured; its direction was wrong, because it assumed branching and depth compound and
   in a real thesaurus **depth is 3–4 and stops them compounding**. This does not close the entry —
   two vocabularies are not a population, and neither is the deep faceted kind — but it moves the
   default from "uncomfortably near" to "unmeasured, with the two available measurements far below".
2. **The SKOS extension point is used, and not where we tested it.** §7.1 work assumed refinements
   of the *documentation* properties. AGROVOC declares 21 refinements and **not one is a note
   property**: 8 refine `skos:notation`, 12 refine `skos:related`, and one refines **`skos:broader`**
   (`agrovoc:IndigenousPeoples-broader`). A refinement of `skos:broader` is a hierarchy link that a
   reader which does not entail from `rdfs:subPropertyOf` **does not see** — which is exactly the
   open `UNTESTED.md` entry, now with a real vocabulary behind it instead of an invented one.
   It also answers the specific unknown that entry named: the declarations are in the **same graph**
   as the concepts (`http://aims.fao.org/aos/agrovoc/`), not a separate imported ontology. Our first
   pass looks in the right place.
3. **A real thesaurus declares far more than it uses.** Of AGROVOC's 21 refinements, **2** appear on
   any statement (`m49Code`, 284 uses; `hasCodeISO3Country`, 220). `openbiz inspect` would report 21
   declarations with no hint that 19 are dead, which is a report-design question we have never had
   the input to ask.
4. **Publisher checksums exist and one of them lies about what it hashes.** LC publishes a PREMIS
   SHA-1 beside each dump. Two independent fetches of `genreForms.skosrdf.nt.gz` agree with each
   other and **disagree** with the published hash — because the hash is attached to the `.gz` URI in
   the JSON-LD but is in fact the hash of the **decompressed** bytes (`sha1sum lcgft.nt` matches
   exactly). Usable, then, but only if you decompress first; a fixture harness that verified the
   `.gz` against the publisher's stated fixity would fail every time and look like tampering.

#### The four criteria, answered

- **Licence.** Three clear, one not.
  - **AGROVOC — CC BY 4.0**, and this is a real check rather than a recollection: the licence is
    asserted *by the dataset itself* in its VoID descriptor. **Caveat that matters:** FAO holds
    copyright for the six FAO languages; content in the other ~34 languages rests with the
    contributing institutions, so a full-language dump is of mixed provenance and only the FAO-language
    subset is cleanly CC BY.
  - **LCSH / LCGFT / LCDGT — public domain.** *"The Library of Congress has prepared this linked data
    system and is making it available as a public domain data set."*
  - **Getty AAT/TGN/ULAN — ODC-BY 1.0**, with a prescribed attribution string. Permissive in
    substance; an attribution obligation is a thing to honour, not a blocker.
  - **EuroVoc — does not pass a licence check today, and this inverts the note's guess.** The
    Publications Office copyright page licenses *"the editorial content of this website"* CC BY 4.0
    and then **explicitly routes the specialised databases (CELLAR, EU Vocabularies) to
    `op-copyright@publications.europa.eu`**. Secondary sources say CC BY 4.0 under Decision
    2011/833/EU and they are probably right; *probably right* is what the note itself refused to
    accept. Resolving it means a human sending an email, which is out of scope for the loop. **Do
    not use EuroVoc until someone does.**
- **Size, and where it lives — and the disk figure in the note is off by a factor of thirty-five.**
  C: is at 26 GB free (95% full) and that is where the *loop state directory* lives; the **repo is
  on G: with 355 GB free**, and `CARGO_TARGET_DIR`, `~/.cache` and `/tmp` are on the WSL ext4 root
  with **929 GB free**. So the constraint is real but it is a *placement* constraint: put the cache
  under `~/.cache/openbiz-fixtures` on the ext4 root and never under the Windows profile. Nothing is
  committed either way — recommended layout is a cache directory outside the tree, a manifest of
  URL + expected SHA-256 + expected triple count in the repo, and every fixture test skipping (not
  failing) when the cache is cold.
  - **But there is a real problem with pinning, and it is the reason for the recommendation below.**
    AGROVOC publishes **only a moving URL** (`.../latestAgrovoc/agrovoc_core.nt.zip`); older releases
    are *"available upon request"* by email, and the dated path guessed from the release date 404s.
    LC regenerates its dumps **daily** (`dcterms:modified` was today's date on every file checked).
    Neither publisher offers an immutable, content-addressed URL. A pinned SHA-256 against either
    therefore breaks on the publisher's schedule, not ours — the fixture would go red for reasons
    that are not defects, which is the fastest way to teach a team to ignore a red.
- **Which `UNTESTED.md` questions it actually answers.** Named, and honestly ranked:
  - *`PathBound::DEFAULT` is a judgement about polyhierarchies nobody here has measured* — **yes,
    already partly answered above.** LCGFT alone gives a real route enumeration.
  - *A mapping link's cost has never been measured, and the scale harness cannot produce one* —
    **yes, AGROVOC only.** 50,636 mapping links over 41,825 concepts, and 36,402 of them
    `exactMatch`, which is also the input the exact-match closure work has never had.
  - *No fixture here is a real extended thesaurus* — **yes, AGROVOC only**, and it has already
    changed the answer (finding 2).
  - *The SKOS core model is not measured at scale* / *what a note costs is unmeasured* — **partly.**
    AGROVOC is 10M triples; LCGFT carries 5,933 `changeNote`s with reified `changeset:` provenance,
    which is a note shape we have never modelled.
  - *`WalkBound::DEFAULT` going down* and *what an abandoned enumeration costs* — **no.** Neither
    vocabulary has a cycle or a wide enough subtree, and the second needs a *planted* cycle. **These
    two still need the branching generator**, and a real fixture does not substitute for it.
  - So: **four helped, two not**, and the two it does not help are the two the last two iterations
    said the generator should do. That is an argument for doing *both*, in that order.
- **Air-gapped honesty.** A test that needs the network fails in the deployments §1.1 exists to
  serve, so the network must never be on the path of `cargo test`. The rule that keeps it honest:
  **a fixture test that cannot find its cache skips with a printed reason and does not fetch.**
  Fetching is a separate, explicit developer command. CI stays hermetic — it will simply skip these
  — which means they are a *local measurement instrument*, not a gate, and calling them a gate would
  be the dishonest version. Every question above still has to be answerable without them, and the
  answer to "what does an air-gapped deployment lose" is "nothing; this is our test harness, not the
  product".

#### The recommendation, which is narrower than the ask

**Take LCGFT and only LCGFT, as one pinned, decompressed, byte-checked file — and do not build a
fetch harness for AGROVOC yet.**

- It is **745 KiB compressed**, public domain with no attribution string to thread through, a real
  ISO-25964-shaped thesaurus with 25.8% polyhierarchy, real SKOS-XL, and real reified change notes.
  It answers the polyhierarchy question *outright* and it costs nothing.
- AGROVOC answers more, but it is 70 MiB behind a moving URL with no archival copy and mixed-provenance
  multilingual content, and every one of those is a reason for a human to decide rather than a loop.
- The generic "fetch-on-demand with a checksum" machinery is the part to **not** build first: with no
  immutable URL anywhere, the manifest is the hard problem and the downloader is the easy one.

- **Cost & impact:** one iteration for LCGFT (fetch script, manifest, skip-when-absent harness, and
  the first two measurements). AGROVOC is a second iteration and should be a separate decision. No
  runtime impact — nothing here ships in the binary. No new dependency: we already read N-Triples.
- **Suggested phase:** Phase 2, immediately before or after the branching generator, which this
  does not replace.
- **What to decide:** (a) LCGFT yes/no; (b) whether anyone will email `op-copyright@` about EuroVoc;
  (c) whether a fixture that skips in CI is worth having at all, or whether a measurement nobody is
  forced to run will simply rot — that last one is a genuine risk and the loop should not pretend
  otherwise.

## Parity findings

Items where the honest answer to *"what would be materially better than the incumbents?"* is **"here
we can only match"** — recorded per the product owner's standing direction (`FEEDBACK-LOG.md`,
2026-08-18) rather than shipped quietly as parity.

_Iteration 2 (layered configuration) found a real "better": the incumbents' weakness is not the file
format but that a deployment's effective configuration is unknowable, so provenance on every setting
and a hard error on an unrecognised key beat parity rather than matching it. See `adr/0005`._

### A UI test runner is table stakes, and we are behind on the thing it enables
- **Iteration 4 (UI test runner).** The honest answer here is not even "we can only match" — it is
  **"we were behind and have now caught up"**. Every incumbent's UI is tested; ours had zero
  assertions. Installing Vitest is hygiene, not a differentiator, and framing it as a wedge would be
  dishonest. The one genuinely better thing in it is small and worth naming precisely: the suite was
  **proven to discriminate** before it was trusted, by killing seven mutations of the component. A
  green suite nobody has tried to break is a claim, not evidence.
- **The wedge this touches, and where we are still behind:** the charter's row is "dated, dense,
  joyless UI" → "visually stunning and modern", and `CLAUDE.md` §4.4 requires anything user-facing
  to be **keyboard-navigable**. The incumbents are genuinely weak here — dense, mouse-driven,
  specialist-facing screens. We now have the harness to assert keyboard behaviour and **no rule that
  forces anyone to use it**; `App` has no interactive element, so the gap is invisible today and
  will not be on the first Phase 3 item. Recorded in `UNTESTED.md`. This is a parity finding in the
  strict sense: on accessibility we currently match the incumbents' *intentions* and have shipped
  nothing that beats them.

### On raw numeric range the JVM incumbents beat us, and saying otherwise would be dishonest
- **Iteration 13 (literal-precision spike).** PoolParty, GraphDB, TopBraid EDG, and VocBench all sit
  on JVM stores that model `xsd:integer` as `BigInteger` and `xsd:decimal` as `BigDecimal`. They have
  no range boundary to characterise. We have one at `i64` and another at a 128-bit fixed point, and
  that is a direct, unwinnable consequence of `CLAUDE.md` §1.2 — the single-binary commitment picked
  an embedded Rust store, and this is part of its bill. **We cannot beat them on range and should
  never imply we do.**
- **Where we *are* better, stated narrowly so it is not overclaiming:** none of them publish where
  their edges are, what their store does to a lexical form, or which of their normalisations are
  XSD-specified and which are theirs. We now do, in `adr/0014`, with a test suite that fails if the
  claim drifts. That is a real difference in *kind* — the wedge row is "proprietary, opaque change
  history" → "the roadmap is the repo" — but it is a difference in **disclosure**, not in
  capability, and a customer with a 25-digit identifier is better served by an incumbent today.
- **What would turn this from a parity finding into a wedge:** the disclosure proposal above, built
  into the candidate seam so the product *tells* an author at the moment their literal will not
  survive. An incumbent that quietly stores everything and an OpenBiz that quietly stores most
  things are both opaque; an OpenBiz that says "this notation will not round-trip, here is why" is
  the only one of the three a governance team can actually sign off. That is the thing worth
  building, and it is a human's to authorise.

---

## LLM assistance opportunities

Places found **while building** where LLM assistance would materially help, recorded per the product
owner's standing instruction (`FEEDBACK-LOG.md`, 2026-08-18). These inform Phase 10's agent list so
it reflects what Phases 1–9 actually taught us rather than day-one guesses.

**These are notes, not authorisation.** The loop does not pull Phase 10 forward to service them, and
does not promote them.

_Iteration 1 (embedded UI): none found. Serving static assets is deterministic transport work with
no judgement in it, no natural-language content, and nothing that mutates a vocabulary — so there is
no candidate seam here either. Recorded as a genuine nil return rather than padded._

_Iteration 2 (layered configuration): none found. Parsing a two-key TOML file is deterministic, and
an LLM guessing what a misspelled key "probably meant" is the opposite of what this change is for —
the value here comes from refusing to interpret. Configuration mutates no vocabulary, so there is no
candidate seam. One adjacent observation worth carrying to Phase 10 rather than a proposal in itself:
`Source` is a small, concrete instance of the provenance shape that `adr/0002` requires on every LLM
proposal — "which layer produced this value" and "which model, prompt version, and inputs produced
this suggestion" are the same question, and the Phase 10 proposal model should not invent a second
vocabulary for it._

_Iteration 4 (UI test runner): none found, and this one is worth being blunt about. Writing tests is
something an LLM is good at, but that is a fact about **our** development process, not a product
capability — Phase 10's agent list is for things an OpenBiz **user** would invoke, and no taxonomist
asks their vocabulary tool to generate Vitest specs. The mutation sweep points the other way if
anything: the one mutant that survived the first draft survived because the test's `fetch` stub was
subtly unfaithful to the real API's abort semantics, which is exactly the class of plausible-looking
wrongness an LLM produces most readily and a human reviewer least reliably catches. Whatever Phase
10 builds, it should assume its output needs an adversarial check that is not itself generated._

_Iteration 5 (named-graph model): none in the built path, and one worth writing down for later. The
model itself is rules — a reserved namespace, a derived IRI, a writability check — and every one of
them must be deterministic and explainable, so an LLM anywhere in it would be a liability. The
opportunity is one layer up and belongs to Phase 12 rather than Phase 10's agent list: the reason
`create_vocabulary_graph` refuses an IRI that is already registered is that IRI collision is the
**only** duplication the store can detect, and it is the least interesting kind. Two vocabularies
that overlap by 80% of their concepts under entirely different IRIs are invisible to it, and that is
the actual silo. Lexical and structural matching is the no-LLM baseline `adr/0003` already requires;
what an LLM adds is recall on near-synonyms and definitional overlap that string matching misses.
Recording it here so the Phase 12 overlap report is not designed as if IRI equality were the
problem.

_Iteration 6 (graph registry over HTTP and in the UI): **one, and it is a real one.** The interface
now lists vocabularies **by raw IRI**, because that is all a graph has until Phase 2 gives it SKOS
labels — `http://example.org/v/animals` and nothing else. At three vocabularies that is merely ugly;
at the two hundred an enterprise actually has, "which of these is the one I want?" becomes
unanswerable from the list, and the user's rational response is to create a two-hundred-and-first.
That is §1.7's silo generator arriving through a UI affordance rather than a missing feature.
The manual path is a curator writing `dcterms:description` on each vocabulary, and it must stay the
path — but it is exactly the tedious editorial backlog nobody completes for vocabularies that
already exist. **The candidate:** an agent that reads a vocabulary's top concepts and drafts a
one-line "what this covers" summary, emitted as a proposal a curator accepts, edits, or rejects,
never written directly. Provenance is straightforward (the graph is the input, and it is already
named). This is a strong Phase 10 candidate precisely because it makes an **existing** vocabulary
findable rather than making a new one easier to write, which is the direction §1.7 wants assistance
to push. Worth pairing with the Phase 2 discovery work, since the same summary is what a discovery
result needs to show. Note the degradation is clean: with `NullProvider` the list shows IRIs and
whatever descriptions a human wrote, which is exactly today's behaviour._

_Iteration 7 (transactional writes): none in the built path, and one genuine one it creates. The
transaction machinery is the last place an LLM belongs — atomicity, lock ordering, and rollback are
exactly the properties that must be deterministic and provable, and "the model thought the write
should commit" is not a sentence anyone should be able to say about a governance store. The
opportunity is the **consequence** of serialising writers. Once writes are serialised, a second
author's work can be refused because the store moved underneath them, and the manual path — which
must always exist under §1.6 — is showing them a triple-level diff. That is precisely what the
incumbents do and what governance teams complain about: a diff of RDF statements is not an answer to
"what changed and does it affect what I was doing?". An agent that reads the intervening change set
and says "while you were editing, someone added three narrower concepts under Loan and deprecated
Bridging Loan — your edit touches neither" is a recall-and-summarise task over data we already hold,
it emits no writes, and it degrades to the raw diff with `NullProvider`. This is the same shape as
the planned "change-request impact summary for reviewers" agent, and the two should probably be one
agent serving two moments rather than two implementations: the reviewer's question and the
conflicted author's question are the same question asked from different seats._

_Iteration 8 (serialising a graph and exporting it): **one, and it is downstream of the item rather
than in it.** Serialisation is byte-level transport work with a specification for an answer; an LLM
has nothing to contribute and everything to lose there, and the round-trip test is the kind of
assertion a model is most likely to make plausibly wrong. The opportunity is what happens **after**
someone downloads a file. An export exists to be given to somebody — a regulator, a partner, a
downstream system — and the question that follows it is never "is this valid Turtle", it is "what
changed since the copy I sent last quarter, and does any of it affect the mappings my system relies
on?". Today the manual path is diffing two Turtle files, which is a diff of statements: a renamed
prefix or a reordered block reads as hundreds of changes, and a genuinely significant deprecation
reads as one line among them. That is the same complaint governance teams make about the incumbents'
version history. **The candidate:** an agent that reads two exports of the same vocabulary and
produces a change summary in the vocabulary's own terms — "eleven concepts added under Derivatives,
one deprecated with a replacement, no mapping targets removed" — emitted as a document a human
approves before it is attached to the export, never written into the vocabulary. It is recall over
data we already hold, it writes nothing, and with `NullProvider` the user gets exactly today's
behaviour, which is the raw diff. Note this is the **third** iteration to record a variant of the
same shape: iteration 6 wanted a vocabulary summarised for findability, iteration 7 wanted an
intervening change set summarised for a conflicted author, and this wants a change set summarised
for an external recipient. Three seats, one capability — Phase 10 should build "explain a set of RDF
changes to a human in the vocabulary's own terms" once and route three questions to it, rather than
discovering that convergence after building three agents._

_Iteration 9 (SPARQL query endpoint): one found, and it is a different shape from the previous
three. **Not in the evaluator** — SPARQL evaluation is specified, deterministic, and the last place
a probabilistic component belongs; and not in query *generation* either, at least not as the
headline, because "natural language to SPARQL" is the demo every vendor in this market gives and
none of them will say what happens when the generated query is subtly wrong. The opportunity is the
one this iteration's own design kept running into: **the endpoint refuses well, and a refusal is
only as good as the reader's ability to act on it.** A 413 says "more than 100 000 results, add a
LIMIT or narrow the query" and a 503 says "ran longer than 30 seconds" — correct, honest, and
useless to a subject-matter expert who did not write the query by hand and has no idea which of its
five triple patterns is the expensive one. The manual path is reading a query plan, which is a skill
the charter's target user explicitly does not have. **The candidate:** an agent that reads a refused
query together with the shape of the vocabulary it ran against and proposes a narrowed rewrite —
"this pattern is unconstrained on all three positions; adding `?s a skos:Concept` reduces it to the
12 000 concepts in Finance" — emitted as a **proposal the user reads, edits, and runs themselves**,
never auto-executed. It writes nothing to any vocabulary, so it needs no candidate seam; with
`NullProvider` the user gets exactly today's behaviour, which is the refusal text. Worth noting
what this shares with the other three: it is again "explain something to a human in the
vocabulary's own terms", but the input is a *query* rather than a change set, so it is a genuinely
fourth capability rather than the fourth seat on the same one — and if Phase 10 builds the
change-explanation agent first, this one should be checked against it before being built separately._

_Iteration 10 (conformance blind-spot pass): one, and it is the **fourth** time the same capability
has been described from a different seat. The finding this pass produced is a sentence a user needs
to be told — "the notation you wrote as `007` will come back as `7`, and a second notation that
differed only in padding has been merged into it" — and producing that sentence means diffing two
RDF graphs and describing the difference in the vocabulary's own terms rather than in triples.
Iterations 7, 8 and 9 each reached the same place from validation, from export lossiness, and from
query results. Phase 10 should build **"explain a set of RDF changes in the vocabulary's own
terms"** once, with the graph diff computed deterministically and the LLM used only to narrate it —
which keeps it inside `adr/0002`'s rule that a model never establishes a fact, only phrases one.
Note the manual path already exists and must keep existing: the difference is computable and
printable without any model, and an air-gapped deployment gets the triples and loses only the
prose._

_Iteration 11 (Oxigraph scale spike): **one, and it is adjacent to iteration 5's rather than new** —
saying so is the point of writing it down. Finding 3 of `adr/0013` is that label search is
`FILTER(STRSTARTS(…))` over every label in the graph, and the fix is a lexical index. But the
measurement made the *other* half of the problem visible: even at 0.5 ms, a lexical index answers
"which labels start with what I typed", and a taxonomist searching `car` will not be shown
`automobile`. At a million concepts that is the §1.7 failure in its purest form — the existing
concept is there, the search does not surface it, and the rational response is to create a
duplicate. **The candidate:** semantic similarity over labels and definitions, offered *alongside*
the lexical hits and visibly labelled as suggestions, at the moment of search and again at the
moment of creation. This is the same underlying capability as the near-synonym recall recorded at
iteration 5 for Phase 12's overlap report — one is similarity *between* vocabularies, this is
similarity *within* one, and Phase 10 should build it once rather than twice. The manual path is
the lexical index and must stay the whole product: an air-gapped deployment searches exactly as
well as a lexical index searches, and loses only the recall. Worth flagging a cost the other
opportunities in this file do not have — embeddings mean a **second index that can drift from the
store**, and a stale suggestion index is a silent wrong answer rather than a visible failure._

_Also iteration 11, recorded as a deliberate **nil** so it is not re-found: the obvious candidate
here — "explain why this query was slow and propose a rewrite" — is already written up at iteration
9 from the query endpoint's seat, and nothing measured this iteration changes its shape. And the
structural gaps the spike's fixture made vivid (a scheme that does not state its top concepts,
concepts with no `skos:inScheme`, orphans with no `skos:broader`) are **computable exactly** and
must not be handed to a model: finding them is a SPARQL query, and `adr/0002`'s rule that a model
never establishes a fact rules it out on its own._

_Iteration 13 (literal-precision spike): **one, and it is a disclosure task rather than a
generation one.** When the store will not preserve a literal — a padded notation, an out-of-range
identifier, a derived datatype it substitutes — the *fact* is computable exactly and must be
computed exactly (`adr/0002`: a model never establishes a fact). What is not computable is the
**sentence a taxonomist needs**: not `"5"^^xsd:int → "5"^^xsd:integer`, which means nothing to a
subject-matter expert, but "the code `007` will be stored as `7`; if the leading zeros are part of
the code, record it as text instead". **The candidate:** at review time in the candidate seam,
given the exact machine-computed list of literals that will not survive, draft the plain-language
warning and the concrete remedy in the vocabulary's own terms. The manual path is the exact list
itself, which is the whole of the correctness — an air-gapped deployment sees every affected
literal and loses only the plain English. This is the **fifth** iteration to describe "explain a set
of RDF changes in the vocabulary's own terms" from a different seat; at this point Phase 10 should
treat it as one named agent with five callers rather than five notes._

_Iteration 14 (backup and restore): **one, and it is narrow — a refusal that cannot say what is
wrong.** When `openbiz restore` refuses a file, it names the rule that was broken exactly, because
every one of those rules is computable and must be (`adr/0002`). But the operator is holding a file
in a disaster, and the question they actually have is "what **is** this file, and where is my real
backup?". Nothing in the product answers that. **The candidate:** given a refused file's shape —
which graphs it names, whether it carries a registry, how many statements, what vocabularies the
IRIs suggest, what stamp it has — draft the plain sentence that identifies it: "this looks like an
export of the *Regions* vocabulary taken from a store, not a whole-store backup; a backup of that
store would also contain `urn:openbiz:graph:system`." The manual path is the refusal message we
already emit, which carries the whole of the correctness — an air-gapped deployment loses only the
identification. Note the guard rails this one needs: the input is a file the operator handed us,
so it is a **data-egress event** in the §1.6 sense and must be refusable, and the output must never
be phrased as a recommendation to retry — a model that says "this is probably fine" over a
restore is the one place its confidence is most expensive.

_Also iteration 14, a **deliberate nil** on the tempting one: "generate the disaster-recovery
runbook". It is tempting because it is prose, and it is wrong for the same reason as the others —
the runbook's correctness is a property of the commands, the exit statuses, and the refusals, all
of which we own and can document exactly. An LLM-written runbook is a plausible document that
nobody verified against the binary, and it would be discovered wrong on the worst day the customer
has. `README.md` is the right home for it and a human wrote it._

### Cut what a semantic relation costs, starting with the derivation text

- **Status:** proposed.
- **Gap:** `docs/adr/0024` measures **3.86 KiB of resident memory per stated `skos:broader`** —
  43× the 92 bytes of the fact itself — against 0.70 KiB for a typed concept that states nothing.
  A million-link vocabulary carrying **no labels at all** held 4 376 MiB and took 62.66 s to
  build, 54.7 s of it system time, which is a machine paging rather than computing. `CLAUDE.md`
  §1.5 asks for "modest memory at rest". This is not that.
- **Why load-bearing:** it is the first hard number that contradicts a non-negotiable, and it is
  in the crate every later phase reads through. Phase 3's concept tree, Phase 4's exports and
  Phase 6's validation all build on `CoreModel`, so the cost is paid again by everything
  downstream. It is also cheapest to change now: the ADR has just decided that S24 adds *nothing*
  to this structure, so the structure is momentarily stable.
- **What is being asked for**, in the order the ADR's decomposition ranks them, each measured
  against the existing `scale` harness rather than argued:
  1. **Stop pre-rendering derivations** (~900 B/link, the largest single share). A `Derivation`
     holds two eagerly-`format!`ed `String`s of about 120 characters. The same facts are already
     in the model as structured links; the text could be reconstructed from
     `(relation, from, to, rule)` at the moment something reads it, which for most vocabularies is
     never. This changes a public type with callers in `openbiz-server`.
  2. **Store one direction of each inverse pair** (~200 B/link). S25 and S26 make the converse
     recoverable exactly; holding both is a read-time convenience paid for in permanent memory.
     The cost is that `Resource::relations` becomes a computed answer for one direction, which
     needs care to keep `RelationOrigin` truthful about which direction the graph actually stated.
  3. **Escape `BTreeMap`'s allocation floor** (~1 KiB/link). An eleven-slot node is allocated for
     a map holding one entry, once per relation per resource, and most of those maps will never
     hold a second. A small-vector-of-pairs representation below a threshold would reclaim most of
     it. This is the largest share and the least interesting change, which is a good sign for it.
- **Why the loop is not deciding it:** all three change shipped public types with production
  callers, and (1) in particular trades memory against the latency of an explanation, which is the
  feature `CLAUDE.md` §3 calls first-class. Ranking memory above explanation latency is a product
  judgement. A target should also be *stated* — "a million-link vocabulary in under N GiB" — and
  choosing N is not the loop's to do.
- **Cost & impact:** roughly one iteration each, independently landable in the order above. No new
  dependency for (1) and (2); (3) is implementable with `Vec` and needs none either.
- **Suggested phase:** Phase 2, after "Semantic relations, part 2" — the traversal that item builds
  should be measured on the current shape before the shape moves.

### Decide what `openbiz inspect` does when the explanation is a gigabyte

- **Status:** proposed.
- **Gap:** a derivation renders on **three** lines carrying the full text of the SKOS statement
  that licensed it, and there are three derivations per stated link. `docs/adr/0024` measures the
  `why:` section at **10.3 MiB at 10k links, 103 MiB at 100k, and 1 033.8 MiB at 1M** — built into
  a single `String` and printed at the end, so at a million links it is a gigabyte held *on top of*
  the 4.4 GiB model. No test runs `inspect` at that size.
- **Why load-bearing:** it is the only place the inference model is reachable by a user today, so
  it is where a real operator meets this first, and they meet it as a machine that stops
  responding. It is also the template for every later explanation surface — Phase 3's "why is this
  concept here?" panel and Phase 6's validation report have the same problem in a smaller font.
- **Why the loop is not deciding it:** `inspect`'s own module documentation argues, at length and
  correctly, that a silent cap in an *inference* report is the one thing such a report must never
  do — a truncated explanation implies "that is all there was", which is exactly what is false, and
  `docs/COMPETITIVE.md` records opaque incumbent reporting as a thing we sell against. Overturning
  a documented product argument on memory grounds is a product decision. The alternatives are not
  equivalent and want choosing between: **stream** the report as it is built rather than buffering
  it (keeps every derivation, costs the summary counts that come last); **cap with a named flag**,
  printing "n of m shown, `--all` for the rest" (never silent, but a default that hides most of the
  answer); or **group** derivations by rule and print each rule once with a count and a sample
  (readable at any size, and no longer a per-fact audit trail).
- **Cost & impact:** one iteration once the shape is chosen. Streaming is the largest change and
  touches how `inspect` composes its whole report, not only this section.
- **Suggested phase:** Phase 2.

### Give the scale harnesses a home and a schedule, because nothing runs them

- **Status:** proposed.
- **Gap:** there are now three `#[ignore]`d measurement harnesses — `openbiz-store`'s `scale` and
  `literal_precision`, and `openbiz-skos`'s `scale` — feeding `adr/0013`, `adr/0014` and
  `adr/0024`. **Nothing runs any of them.** CI runs the ordinary suite, which by design skips
  every row that matters. Each has a small in-suite case asserting the fixture's arithmetic so the
  harness cannot rot into measuring nothing, and that is a real guard, but it does not notice a
  change that makes the model twice as large — it only notices one that changes the *ratio*.
- **Why load-bearing:** a performance number in an ADR with nothing re-measuring it is a claim with
  an expiry date nobody wrote down. `adr/0013`'s numbers are from iteration 11 and the store has
  had a format migration since. The failure mode is quiet and familiar: the ADR still reads true,
  and the machine no longer agrees with it.
- **What is being asked for:** one of — a `cargo xtask bench` that runs all three and prints the
  tables together, so a human can run one command before a release; **or** a scheduled CI job on a
  runner large enough for the 1M rows, which is arguably the hardware-bound testing `CLAUDE.md` §8
  puts outside the loop; **or** an explicit decision that these are run by hand at named moments
  (before a phase closes, before a release) and that the ADRs carry the date they were last run.
  The third costs nothing and would be a real improvement on today.
- **Why the loop is not deciding it:** two of the three options are about CI infrastructure and
  runner cost, which is a spending decision, and the third is a process commitment about when
  humans do things.
- **Cost & impact:** the third option is under an iteration. The first is one. The second is not
  the loop's to arrange.
- **Suggested phase:** Phase 14, or sooner if a release is contemplated.

### Enforce the retired-claims rule in CI instead of trusting the loop to grep

- **Status:** proposed.
- **Gap:** iteration 25 retired the claim "there is no OWL 2 DL reasoner in Rust" in
  `docs/COMPETITIVE.md` and left it standing in `README.md`, `CLAUDE.md`, and
  `crates/openbiz-owl/src/lib.rs`. A human found it two iterations later and corrected it by hand
  (`docs/FEEDBACK-LOG.md`, 2026-08-19). Iteration 27 fixed every instance and wrote the rule down —
  *retire a claim, grep the repo, fix it everywhere in the same iteration* — as a convention at the
  top of `COMPETITIVE.md`, plus a table of what has been retired and where it was published.
- **Why load-bearing:** the convention is a rule the loop has to remember, and the failure it
  guards against is precisely the loop failing to remember. The repository is public and
  `CLAUDE.md` §4 makes misreporting worse than lacking, so the cost of the next miss is a false
  public claim, not an internal inconsistency. This is also the second time a research finding has
  had to be applied by hand after being recorded correctly.
- **What is being asked for:** a machine-readable retired-claims ledger (the table in
  `COMPETITIVE.md`, or a small file beside it) giving each retired claim a phrase to search for,
  and a CI check that fails if that phrase appears in a **live** document — `README.md`,
  `CLAUDE.md`, `BUILD-PLAN.md`, `UNTESTED.md`, `BLOCKED.md`, `PROPOSED.md`, and all source doc
  comments. Append-only history — `LOOP-LOG.md`, `FEEDBACK-LOG.md`, the dated ADRs, and
  `COMPETITIVE.md`'s own superseded paragraphs — is exempt, and the exemptions must be listed
  explicitly in the ledger rather than implied by the script, or the check quietly stops covering
  new files.
- **Why the loop is not deciding it:** it is a new CI gate and a new repo-wide artefact, neither of
  which any plan item asks for, and the human's correction asked the loop to *follow* the mechanism
  rather than to build one. Building an unrequested doc-linting framework off the back of a
  one-paragraph correction is the scope creep `CLAUDE.md` §7 puts this file in the way of.
- **Cost & impact:** small — roughly an afternoon for the check, the ledger format, and its own
  failing-then-passing test. The design risk is a check so noisy it gets exemptions added to
  silence it, which would be worse than no check; the mitigation is that a phrase only enters the
  ledger when a pass actually retires a claim, so the list stays short.
- **Suggested phase:** Phase 14, or alongside the next product-owner pass.

### Measure the ancestry walk at scale, the way `adr/0024` measured storing

- **Status:** proposed.
- **Gap:** `adr/0024` measured what materialising S24's closure would cost and ruled it out.
  Iteration 28 built the alternative — a bounded walk, answered on read — and measured nothing
  about it. So the repository has a hard number for the option it rejected and none for the one it
  shipped. `crates/openbiz-skos/src/scale.rs` covers the relation *model* at 10k, 100k and 1M
  links across four hierarchy shapes and does not touch `CoreModel::ancestry`.
- **Why load-bearing:** the S27 pass runs one walk per concept with a `skos:related`, inside every
  `openbiz inspect`, and Phase 3's concept tree will ask the same question of every visible node.
  That is the quadratic iteration 26's loop-log flagged as the open risk of answering on read, and
  it is now shipped rather than hypothetical. It also decides `AncestryBound::DEFAULT`: 100 000
  ancestors and 1 000 000 links are a judgement about vocabularies nobody here has seen, and too
  low refuses a real thesaurus an answer while too high makes the walk the reason a request hangs.
- **What is being asked for:** extend `scale.rs` to the walk, at the same sizes and shapes, plus
  one shape it does not have — a deep hierarchy with an associative link on every concept, which
  is the shape that makes the S27 pass expensive. Report per-walk time and the whole-vocabulary
  S27 pass separately, because they are different questions: one is what a concept-tree click
  costs and the other is what an inspect costs. Then either confirm the default bound or move it,
  in an ADR that records the number.
- **Why the loop is not deciding it:** it is a second measurement iteration in the same area, and
  `CLAUDE.md` §3's "stop at one item" plus iteration 26's own finding — that measuring a traversal
  which does not exist yet produces a number that agrees with you — both say it belongs in its own
  iteration rather than bolted onto the one that built the thing. It is also the shape of work a
  human may reasonably want to sequence against Phase 3 instead.
- **Cost & impact:** one iteration. No new dependency: the existing harness reads
  `/proc/self/status` rather than pulling a crate in. Runtime impact is nil — the harness is
  `#[cfg(test)]` and the expensive sizes are `#[ignore]`d.
- **Suggested phase:** Phase 2, before the concept-tree query API item; or Phase 3 alongside it.

### Make `openbiz inspect` say which integrity conditions it checked

- **Status:** proposed.
- **Gap:** the report closes with "no SKOS integrity condition is violated by this graph". That
  sentence is true of every condition this build implements and silent about every one it does
  not. Iteration 28 fixed the sharpest case — a check that started and hit its bound now reports
  `Severity::Unchecked` and the closing sentence changes — but a condition that was never
  implemented at all still produces the confident sentence, and nothing in the output distinguishes
  "checked and clean" from "not looked for".
- **Why load-bearing:** the whole pitch of the report is that a governance team can take it to an
  auditor. A clean bill of health that does not say what was examined is the incumbents' failure
  `docs/COMPETITIVE.md` records, reproduced by us. Phase 2 has "all SKOS integrity conditions, each
  with a test citing its S-number" still open, which means the gap between what the sentence
  implies and what is true is at its widest right now.
- **What is being asked for:** an enumeration the report can print — the S-numbers this build
  evaluates, and for each the number of resources it was evaluated over — so the closing sentence
  becomes a claim with a scope. It probably wants to be a method on `CoreModel` rather than a list
  maintained in the report, or it will drift out of date exactly like the sentence it replaces.
- **Why the loop is not deciding it:** it changes the shape of the model's public answer and it
  overlaps the open "all SKOS integrity conditions" plan item, so doing it now would half-build
  that item out of order.
- **Cost & impact:** one iteration on its own; close to free if done as part of the all-conditions
  item, which is the better sequencing.
- **Suggested phase:** Phase 2, as part of "All SKOS integrity conditions from the specification".

### Put SKOS §6 (`skos:notation`) in the plan — it has no item anywhere
- **Status:** proposed.
- **Gap:** SKOS Reference §6 defines `skos:notation` and statement **S15** ("skos:notation is an
  instance of owl:DatatypeProperty"). Phase 2's backlog has items for lexical labels (§5),
  documentation properties (§7), semantic relations (§8), collections (§9) and mapping properties
  (§10). **There is no item for §6 at all** — not unchecked, not deferred, not blocked. It was
  noticed while reading §7, because the two sections are adjacent and the model's own test names
  `skos:notation` as the example of a statement it drops.
- **Why load-bearing:** a notation is the *classification code* — `621.3`, `E11.9`, `ISIC C10` —
  and in the regulated industries `CLAUDE.md` names as the buyer, it is frequently the primary key
  the rest of the enterprise joins on. A thesaurus imported from a library or statistical
  classification is mostly notations. Today they are read from the file, stored in the graph, and
  exported again correctly — the store is faithful — but the SKOS model does not see them, so
  `openbiz inspect` reports nothing about them and nothing can search or sort by one. It is also
  the one place SKOS uses a *typed* literal deliberately (§6.5.1 recommends a datatype per
  notation scheme), which interacts with `adr/0014`'s finding that a literal past Oxigraph's
  precision boundary round-trips perfectly while ceasing to be a value.
- **Cost & impact:** small — one property, one statement, no integrity condition, no closure. A
  day's item at the size Phase 2's others have run. It should probably sit immediately after the
  documentation properties, since both are per-concept scalars the concept editor will want.
- **Suggested phase:** Phase 2.

### Say which of §7's properties a concept is *missing*, as a rule pack rather than a finding
- **Status:** proposed.
- **Gap:** `openbiz inspect` now counts documentation coverage and says explicitly that SKOS
  requires none, naming ANSI/NISO Z39.19 and ISO 25964 as the documents that would ask. That is
  honest, and it is also an admission that we cannot yet answer the question a governance team
  actually has: *which* concepts lack a definition, and does that matter for this vocabulary.
- **Why load-bearing:** it is the single most common check a taxonomy team runs, every incumbent
  ships it, and `docs/COMPETITIVE.md` records that our differentiator is not having the check but
  being able to say **which document asks for it and why** — which needs the check to exist, cited,
  and refusable. The right home is `openbiz-validate`'s rule-pack substrate, expressed in SHACL as
  `CLAUDE.md` §2 requires, not a hard-coded `Finding` in `openbiz-skos`. Building it in the SKOS
  crate would be quick and would be the exact mistake this iteration refused to make.
- **Cost & impact:** blocked on Phase 4's SHACL substrate existing, so this is a *placement*
  decision more than a work item: it should be one of the first rules the Z39.19 pack carries, and
  it should be listed on that pack's build-plan item so it is not rediscovered later.
- **Suggested phase:** Phase 4.

### Make §8.4's disjointness check cost less than concepts × depth
- **Status:** proposed.
- **Gap:** `adr/0027` stopped the S27 pass hanging on a legal vocabulary by sharing one link budget
  across the whole sweep. That is a backstop, not an answer. The pass still walks the entire
  ancestry of every concept that has a `skos:related`, so its cost is the number of associated
  concepts times the depth of the hierarchy, and the budget converts "slow" into "partially
  checked" rather than into "checked". Measured: a 10 001-concept chain with a genuine violation on
  every concept reports 1 413 of 9 999 before it stops.
- **Why load-bearing:** S27 is the **only** integrity condition §8 states, and it is the one a
  thesaurus actually violates in practice — an editor writes `skos:related` between two concepts
  that a third editor later puts in the same branch, and nothing shows it until a validator finds
  it. A validator that checks the first 14% of a deep vocabulary is not the differentiator
  `docs/COMPETITIVE.md` claims.
- **Options, none costed:** (a) walk **down** from each associate instead of up from each concept
  when the associate count is smaller — same answer, different multiplier, and free to choose per
  concept; (b) order the sweep so a concept's walk reuses its parent's result, which is a partial
  closure and needs `adr/0025`'s memory argument re-run against a *bounded* cache rather than the
  full closure it rejected; (c) an interval or level labelling of the hierarchy, computed once per
  build, that answers "is X above Y" in constant time for a DAG — the standard answer, and it needs
  measuring against the polyhierarchy SKOS permits before anyone believes it.
- **Cost & impact:** medium. (c) is the real fix and is a genuine piece of design work, including
  what it does about the cycles §8.6.8 makes legal. It should not be started without the
  measurement harness `scale.rs` now has, which is the thing that would say whether it worked.
- **Suggested phase:** Phase 2, after the remaining §8 items.

### Decide whether the sweep budget should scale with the vocabulary
- **Status:** proposed.
- **Gap:** `AncestryBound::DEFAULT`'s million links was chosen in iteration 28 as a backstop against
  a pathological graph and `adr/0027` has made it a limit an ordinary customer can reach: a
  hierarchy about a thousand levels deep is checked completely and a deeper one is not. A fixed
  constant is the wrong shape for a limit whose right value depends on how much the operator is
  willing to wait, and there is no way to raise it — `with_ancestry_bound` has no production caller
  and no configuration path.
- **Why load-bearing:** it is the difference between "OpenBiz cannot check my vocabulary" and
  "OpenBiz asked me whether I wanted to wait". `CLAUDE.md` §1.5 wants a lightweight default; it does
  not want a validator that silently declines on a vocabulary the customer considers ordinary.
- **Options:** a budget derived from the graph size (so the pass is linear-ish in the vocabulary
  rather than constant); a configuration key with the default documented; or a `--thorough` flag on
  `openbiz inspect` that lifts it and says what it will cost. The third is the most honest and the
  most work.
- **Cost & impact:** small once the decision is made; the decision is the item. It interacts with
  the proposal above — an algorithmic fix might remove the need entirely, so this should not be
  built first.
- **Suggested phase:** Phase 2.

### Find a permissively-licensed extended thesaurus and read it end to end
- **Status:** proposed.
- **Gap:** §7.1's extension point now works (`adr/0028`), and every fixture proving it was invented
  by this loop. `CLAUDE.md` §6 forbids real vocabulary data in fixtures without a clear licence,
  which is right — and the consequence is that a shape neither the code nor the tests thought of is
  invisible to both. The specific unknown that worries me: whether enterprise vocabularies declare
  `ex:usageNote rdfs:subPropertyOf skos:scopeNote` **in the vocabulary graph**, or in a separate
  ontology graph the vocabulary imports. If it is the second, the first pass reads the wrong graph,
  finds nothing, and reports "no declared refinements" — which reads exactly like "there are none".
  That is the silent-broken-connector failure the driver names by name.
- **Why load-bearing:** it is the difference between a feature that works on our examples and one
  that works on a customer's file, and the failure mode is a false negative that looks like a clean
  report. It also generalises past this item: the same question applies to every §2 standard we
  claim, and nothing in the repository has ever been read against a published vocabulary.
- **Options:** a licence-cleared corpus checked into `tests/fixtures` (best, and the licence review
  is the work); a documented manual procedure an operator can run against their own file and report
  on (cheap, and proves nothing here); or a `--follow-imports` option on `inspect` that reads
  `owl:imports`, which is a different item and would need the graph registry to hold the imported
  ontology at all.
- **Cost & impact:** the search is research, not engineering, and it is the reason this is a
  proposal. Reading one once found is small.
- **Suggested phase:** Phase 2, or Phase 11 (interop & migration) if it turns into a corpus.

### Close `skosxl:labelRelation`'s refinement, now that the mechanism exists
- **Status:** proposed.
- **Gap:** `UNTESTED.md`'s iteration-23 entry — a refinement of `skosxl:labelRelation` reaches
  nothing — was written as the same gap as the note one, to be closed by one mechanism. The
  mechanism now exists (`crate::refinement`, `adr/0028`) and is written against a target property
  set rather than hard-wired to notes, so attaching B.4's property to it is small. What is *not*
  small is the decision it needs: B.4.4.1 says "a sub-property of a symmetric property is not
  necessarily symmetric", so reading `ex:acronym` means entailing the `skosxl:labelRelation` and
  then **declining** to close S62 over the refined property while still closing it over the
  super-property. That is a rule about which conclusions a derivation may licence, not a wiring job.
- **Why load-bearing:** B.4.1 says the property "is not intended to be used directly, but rather as
  an extension point", so a refinement is the *ordinary* way B.4 is used. A thesaurus whose ISO
  25964 label relationships are expressed through `ex:acronym` currently reads to us as one with no
  label relationships at all.
- **Options:** extend `PropertyRefinements` to resolve B.4's property and record, per entailed link,
  whether it arrived through a refinement — then have S62 close only the ones that did not. Or
  defer the whole thing to the reasoner (Phase 5), which is where iterations 18–23 kept concluding
  entailment belongs.
- **Cost & impact:** moderate, and it is the second half of a gap this iteration closed the first
  half of. Not taken because deciding where entailment lives is a standing question and I should
  not settle it inside a documentation-properties item.
- **Suggested phase:** Phase 2.

### Check every writing path against the vocabulary it would leave, starting with `openbiz move`
- **Status:** proposed.
- **Gap:** iteration 43 built `openbiz_skos::newly_violated` — build the model of the vocabulary a
  proposed change would leave, run every SKOS integrity condition, and refuse any that is violated
  afterwards and was not before (`adr/0038` decision 5). It exists because the first working
  version of `openbiz merge` produced, from ordinary input, a graph violating **two** of the
  specification's own conditions, one of which (S27) no hand-written check would have predicted.
  **`openbiz move` has the same hole and is already checked off.** Reproduced by hand at iteration
  43 and recorded in `UNTESTED.md`: moving a concept under something it is `skos:related` to is
  accepted, approves cleanly, and leaves `S27 VIOLATED` where it held before. `openbiz import` and
  `openbiz retract` are the same shape — a file of statements can break any condition — and they
  say nothing either.
- **Why load-bearing:** "governance is the substrate" is one of the eight rows in `CLAUDE.md` §1's
  wedge table, and an approval path that writes a graph the product's own `openbiz integrity` then
  calls not-a-SKOS-vocabulary is the plainest possible contradiction of it. It is worse for an
  import than for a move, because an import is the path a customer's first day runs through.
- **Options:** (a) wire `would_break` into `relocate.rs`, then into the import and retract paths —
  smallest, and the move half is one call plus a test. (b) Move the check into `Store::propose_edit`
  and `propose_import`, so *no* candidate can be staged that breaks a condition — stronger, and
  wrong as stated, because the store is engine-free by `adr/0019` and cannot read SKOS. (c) Check
  at **approval** rather than at proposal, which is where the vocabulary actually changes and which
  would cover a candidate raised before an unrelated edit broke something. (c) is probably the
  right long-term answer and (a) is the right next step; they are not exclusive.
- **Cost & impact:** (a) is small — one call, one error variant, one failing test per path. (c) is a
  design decision about what a candidate means, and needs its own ADR: a proposal that was valid
  when raised and is not when approved is a state the seam has never had to represent.
- **Suggested phase:** Phase 2 for (a), because it repairs items already in it. Phase 4 (SHACL and
  governance rules) for (c), where the same question arises for rule packs.

### Repoint a merged concept's references from the vocabularies that hold them
- **Status:** proposed.
- **Gap:** `openbiz merge` repoints every reference **inside one vocabulary** and counts the ones
  outside it, naming each graph and how many (`adr/0038` decision 6). It does not change them,
  deliberately: a statement in another named graph is a change to that vocabulary, which somebody
  else reviews. The result is honest and incomplete — after a merge, another vocabulary's
  `skos:closeMatch` still points at an IRI that no longer denotes anything, and the only thing that
  said so was one line of a report that has scrolled past.
- **Why load-bearing:** `CLAUDE.md` §1.7 makes reuse and mapping outrank creation, so the product
  actively encourages the cross-vocabulary links this then leaves dangling. The more successfully a
  customer maps their vocabularies together, the more broken references a merge leaves behind.
- **Options:** (a) raise one additional candidate **per affected vocabulary**, staged and
  unapproved, so each owner reviews their own — matches the governance model exactly, and means one
  merge can produce five candidates. (b) Record the merge in the system graph as a redirect the
  other vocabularies' reads consult, which is a tombstone by another name and collides with the
  deprecation-lifecycle item. (c) Report only, as now, and leave it to the deprecation lifecycle to
  give the merged IRI a forwarding address. (a) is the one that fits the seam; (c) may be the right
  sequencing, since a merge and a deprecation want the same answer.
- **Cost & impact:** (a) is moderate and needs a decision the seam has not needed before — a single
  operator action producing candidates against graphs the operator may not own.
- **Suggested phase:** Phase 2, or alongside the deprecation-lifecycle item, which it overlaps.

### Decide what the integrity guard should do when a vocabulary's own refinements make a condition unreadable
- **Status:** proposed.
- **Gap:** `crate::staging::newly_broken` — `adr/0038`'s check, generalised at iteration 44 — asks
  whether a condition **holds now and would not afterwards**. A vocabulary that declares a
  sub-property this build cannot entail through leaves the affected conditions **unchecked**, which
  is the honest verdict and is exactly why the guard cannot fire: a condition with no verdict on
  either side was never "held". So the change goes through and the violation lands unseen.
  Reproduced at iteration 44 with `prov:wasDerivedFrom rdfs:subPropertyOf skos:related`, which makes
  `openbiz split --place below` entail an S27 violation the guard passes and `openbiz integrity`
  afterwards reports as `unchecked` rather than `VIOLATED` (`docs/UNTESTED.md`).
- **Why load-bearing:** the guard is currently the only thing standing between a computed bulk edit
  and a vocabulary that stops being SKOS, and it is **weakest on exactly the vocabularies most
  likely to need it** — the ones elaborate enough to declare their own refinements, which is what an
  ISO 25964 thesaurus with a house extension looks like. It also compounds the doubt iteration 43
  closed on: the guard already protects a clean vocabulary better than a dirty one, and this is a
  second axis of the same asymmetry.
- **Options:** (a) entail through declared refinements of the SKOS semantic properties, which is the
  real fix and is a reasoning change, not a check change — it belongs with the `RefinementScan` work
  and is the largest of the three. (b) Refuse a change whose statements use a property whose
  refinements leave a condition unchecked — cheap, sound, and probably too blunt: it would refuse
  ordinary edits to any vocabulary with one awkward declaration. (c) Do not refuse, but **say so**:
  have every staging command report "N conditions could not be checked here, so this change was not
  measured against them", which is the explainability commitment applied to the guard's own limits
  and costs almost nothing. (c) is worth doing whichever of (a) and (b) is chosen.
- **Cost & impact:** (c) is small. (b) is small and risky. (a) is a substantial piece of reasoning
  work and should be scoped against the specification rather than against this one reproduction.
- **Suggested phase:** (c) in Phase 2, alongside the "check every writing path" item above, which it
  belongs with. (a) in Phase 4, where SHACL and the rule packs raise the same question about what a
  rule that cannot be evaluated should say.

### Repoint every reference at a replacement without making the retired IRI stop existing
- **Status:** proposed.
- **Gap:** there are two operations and neither is the one a curator usually wants. `openbiz merge`
  repoints every reference in the vocabulary and the duplicate stops existing. `openbiz deprecate`
  keeps the concept and repoints nothing — `dcterms:isReplacedBy` is a signpost, so after a
  retirement every `skos:broader`, `skos:related` and collection membership still points at the
  retired concept (`adr/0040` decision 3). The realistic editorial act is **both**: the term is
  obsolete, the IRI must keep resolving for everything outside this system, and everything *inside*
  the vocabulary should now go through the replacement.
- **Why load-bearing:** without it, the second half of a retirement is manual and unbounded — one
  `openbiz move` per child, and nothing at all for a `skos:related` link. The report names the work
  precisely and then leaves an operator to do it a statement at a time, which is exactly the "the
  tool assumes you already know how to build an ontology" complaint `CLAUDE.md` makes about the
  incumbents. It is also the third item to arrive at the same missing primitive: `openbiz split`
  leaves narrower concepts unapportioned and points at `openbiz move`; a merge leaves other
  vocabularies' references and points at nothing.
- **Options:** (a) a `--repoint` flag on `openbiz deprecate`, staging one candidate that both adds
  the retirement and rewrites the internal references — one decision, one review, and the diff is
  large in exactly the cases where it should be. (b) A separate `openbiz supersede` that assumes the
  retirement has already happened and reads `dcterms:isReplacedBy` to know where to point, which
  composes better and is one more command to discover. (c) Leave it manual and improve the report,
  which is today's behaviour. (b) keeps each command's claim about what it removes intact, which is
  the property `adr/0040` leans on hardest; (a) is fewer steps for the operator.
- **Cost & impact:** moderate. Most of the machinery exists — `MergeScan` already computes exactly
  this rewrite — but the decision about *which* references should move is not obvious: a
  `skos:historyNote` mentioning the retired concept should stay, and a `prov:wasDerivedFrom`
  pointing at it certainly should.
- **Suggested phase:** Phase 2, with the deprecation-lifecycle item, which asks the same question
  from the read side.

### Measure the bulk operations at scale, once, rather than recording an eighth unmeasured cost
- **Status:** proposed.
- **Gap:** `docs/UNTESTED.md` now holds eight entries that say the same thing about a different
  command: correctness proven on fixtures of a handful of concepts, cost unmeasured. Move, merge,
  split and deprecate are each four passes over the vocabulary, and three of them additionally scan
  **every** vocabulary in the store. The generators `adr/0013` and `adr/0024` already build 100k and
  1M-concept vocabularies, so the missing thing is not a harness — it is one task that uses it.
- **Why load-bearing:** `CLAUDE.md` §1.5 makes lightweight a non-negotiable, and Oxigraph's own
  documentation warns that query evaluation is not yet optimised. Four passes over a 1M-concept
  vocabulary to retire one term is either fine or a product defect, and eight ledger entries in a
  row have declined to find out. The `adr/0024` measurement, when it was finally taken, moved a
  decision; these might too.
- **Options:** (a) one benchmark task covering all four operations against the existing generated
  vocabularies, recording wall-clock and peak memory per command and per pass, and closing all eight
  entries or replacing them with numbers. (b) Instrument each command to report its own pass count
  and elapsed time, which turns the question into something a customer can answer on their own data.
  (a) first; (b) is worth doing anyway and is nearly free once `tracing` spans are in place.
- **Cost & impact:** (a) is a day's work at most and needs no new dependency. The 1M runs are
  `#[ignore]`d, as the existing scale tests are, so CI is unaffected.
- **Suggested phase:** Phase 2, after the last bulk operation lands, so it measures a settled set.

### Tell an author when a concept they are *mapped to* has been retired elsewhere
- **Status:** proposed.
- **Gap:** `adr/0041` makes a retired concept visible in the vocabulary it lives in. It says nothing
  about the direction that matters most for a federated estate: this vocabulary's
  `skos:exactMatch` to a concept in the corporate vocabulary next door, which that vocabulary
  retired last month. Nothing here reads it, nothing warns, and the mapping goes on looking correct.
  `openbiz deprecate` already draws the distinction from the other side — it warns when a
  replacement is an IRI *nothing in the store* describes — so the store-wide read exists; what does
  not exist is anything that runs it over a vocabulary's mapping links.
- **Why load-bearing:** `CLAUDE.md` §1.7 makes reuse outrank creation, and a mapping is the cheapest
  form of reuse there is. A tool that encourages mapping and then never tells you a mapping has gone
  stale is quietly making reuse more expensive than duplication, which is the failure mode the whole
  anti-silo commitment exists to avoid. It is also the sharpest version of a complaint
  `docs/COMPETITIVE.md` records against the incumbents: a vocabulary looks healthy right up until an
  auditor follows a link.
- **Options:** (a) a section in `openbiz inspect` that resolves every mapping target against the
  other vocabularies in the store and names the retired ones — cheap, local, and bounded by what one
  deployment holds. (b) The same check at write time, warning when a *new* mapping targets something
  already retired, which is the version that prevents rather than reports. (c) Both, with (a) as the
  sweep and (b) as the guard. It is deliberately **not** proposed as anything that reaches outside
  the store: a mapping to a resource on the public web is Phase 12's discovery problem and carries a
  data-egress question this does not.
- **Cost & impact:** small for (a) — it is `openbiz mappings` crossed with `Retirements`, both of
  which now exist. (b) needs a decision about whether a mapping to a retired concept is a warning or
  a refusal; it should be a warning, because mapping to a superseded term is sometimes exactly what
  a historical dataset needs.
- **Suggested phase:** Phase 2 for (a) if it is judged part of the lifecycle; otherwise Phase 12,
  where cross-vocabulary awareness lives.

### Take back a retirement over a subtree, so an abandoned migration is one decision
- **Status:** proposed.
- **Gap:** `openbiz reinstate` (`adr/0042`) takes back the retirement of **one** resource. The case
  the deprecation lifecycle exists for is a migration — import a legacy scheme, retire a large part
  of it in favour of the new one — and a migration that is abandoned, or that retired one branch
  too many, has to be reversed one concept at a time, each producing its own candidate for a
  reviewer to approve separately. `openbiz move` had exactly this shape and answered it by acting on
  a subtree; nothing in the deprecation lifecycle does.
- **Why load-bearing:** it is the same sparse-case assumption iteration 46 flagged as its own
  "still uncertain" about *show and mark*, arriving from the other end. Every part of this
  lifecycle has been designed against a thesaurus with a handful of obsolete terms, and every part
  of it becomes awkward in the migration case that a governance function actually runs. A reviewer
  handed two hundred one-line candidates will approve them without reading them, which turns the
  candidate seam — the thing `CLAUDE.md` §3 makes the substrate of the whole product — into a
  formality.
- **Options:** (a) `--subtree`, mirroring `openbiz move`: take back the retirement of a concept and
  everything below it, in one candidate. Simple, and wrong in one common case — a retired concept
  can have children retired for their own unrelated reasons. (b) Scope it by the retirement instead
  of by the tree: take back every retirement that named *this* concept as its replacement, which is
  precisely "undo that migration" and reads the graph rather than guessing. (c) Take back every
  retirement in a candidate that was applied, by candidate id, which is the most honest of the
  three — it reverses a decision that was actually made — and needs the store to record which
  statements an applied candidate wrote, which it already does.
- **Cost & impact:** (a) is small and reuses the walk `openbiz move` has. (c) is the most valuable
  and is really a general "revert a candidate" capability that every write path in this build would
  want, which is an argument for proposing it as that rather than as a flag on this command.
- **Suggested phase:** Phase 2 for (a) or (b); (c) belongs with the candidate seam and should not be
  smuggled in as a deprecation feature.

### Make an unlanded iteration visible, by checking the log against `main`'s history

- **Status:** proposed.
- **Gap:** iteration 48 completed every step of the loop except the last. It wrote
  `docs/CAPABILITIES.md`, cut `BUILD-PLAN.md`'s preamble, rewrote the stale README sections, ran the
  full verification green, and appended a loop-log entry describing all of it as done — then exited
  without `git commit`. Iteration 49 found the work uncommitted on a branch with zero commits,
  re-verified it, and landed it. Nothing was lost, but for one iteration `LOOP-LOG.md`'s newest
  entry described a commit that did not exist.
- **Why load-bearing:** `LOOP-LOG.md` is explicitly the loop's working memory across context
  boundaries (`CLAUDE.md` §7), and the next iteration reads it as history. Every ledger is written
  by the iteration reporting on itself, so an iteration that dies between "record the truth" and
  "land it" leaves files that are internally consistent, individually accurate, and collectively
  wrong about what shipped — there is no contradiction *inside* the repository to notice. Iteration
  49 caught it only because the tree happened to be dirty **and** the newest entry named the same
  files; had 48 committed but not pushed, or pushed but not merged, the tree would have been clean
  and the entry would very likely have been read as history and built upon.
- **What is being asked for:** a check that compares `LOOP-LOG.md`'s newest `## Iteration N` heading
  against `main`'s history — if the newest entry is not reachable in a commit on `main`, say so
  loudly. Natural homes are `/openbiz-status`, so a human sees it, and the loop's own orientation
  step, so the next iteration is told rather than left to infer it from a dirty tree. The stronger
  variant is ordering: write the log entry as a follow-up commit *after* the merge, which
  `CLAUDE.md` §8 already permits, so the entry cannot exist before the thing it describes.
- **Why the loop is not deciding it:** it changes the loop's own operating procedure and the
  `/openbiz-status` surface, neither of which is a plan item, and the ordering variant edits the
  prompt's documented step sequence. A self-directed rewrite of the harness that supervises it is
  exactly what `CLAUDE.md` §7 puts this file in the way of.
- **Cost & impact:** small — the check is a few lines of `git log --grep` against one heading, and
  the ordering change is free. The risk is a false alarm on the legitimate case where the entry is
  committed with the item in the same PR and `main` has not yet been updated locally, so it should
  compare against the remote and treat "unpushed" and "unlanded" as distinct.
- **Suggested phase:** Phase 14, or alongside the next product-owner pass.
