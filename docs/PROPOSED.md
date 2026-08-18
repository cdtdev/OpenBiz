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
- **Suggested phase:** Phase 2, **before** the concept tree.

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
