# Untested & unproven

Things built but not proven, or proven only narrowly. **Write here the moment you notice, not at
the end of the iteration** — the gap you defer recording is the one you forget.

This file existing is not a failure. A loop that never records a gap is not a loop with no gaps; it
is a loop that has stopped looking. What *is* a failure is a gap that never closes.

What belongs here:
- **Built but no production caller.** Code that exists and is unit-tested but nothing invokes.
  This is the most important category and the easiest to rationalise away — record it every time.
- Happy path tested, failure paths not.
- A standard partially implemented, where we must not claim full support.
- Anything needing hardware, an account, or infrastructure this machine lacks — mark these
  **environment-limited** so the monitor can tell a permanent floor from real rot.
- Behaviour verified by inspection rather than by a test.

Strike an entry through (`~~like this~~`) when it closes, with a one-line note on what closed it.
Do not delete it — the record of what took how long to close is the signal.

## Entry format

```
### <what is unproven>
- **Kind:** no-production-caller | partial-coverage | partial-standard | environment-limited | inspected-only
- **What is proven:** …
- **What is not:** …
- **What would close it:** …
- **Opened:** iteration N
```

### The ISO 25964-1 revision date is second-hand — the ISO registry itself was never read
- **Kind:** environment-limited
- **What is proven:** nothing, by test. This is a research finding, and it is recorded here rather
  than only in `docs/COMPETITIVE.md` because we are about to make a standards claim that rests on
  it. Two independent secondary sources (a 2024 peer-reviewed article by the revision's own
  participant, and NISO's ISO 25964 committee page) agree that ISO 25964-1 went to comment and vote
  on 2024-07-30, that TC 46's work is complete, and that publication is expected in 2026.
- **What is not:** the primary source. The ISO catalogue returns **HTTP 403** to automated fetching
  — confirmed again at iteration 50 against the *revision's* own entry,
  `https://www.iso.org/standard/86713.html` (iteration 25 tried `53657`, which is the 2011 edition).
  So the exact **stage code** (50.00 vs 50.20 vs 60.00) and any **publication date** remain
  unverified.
- **Narrowed at iteration 50, not closed.** Two things previously unknown are now known from the
  catalogue entry's own title as surfaced in search metadata — still not the page itself: the
  revision is at **FDIS** (`ISO/FDIS 25964-1`), the approval stage before publication, and it is
  **Edition 2** with a changed title ("...for information retrieval, **management and use**"). That
  is stronger than "expected in 2026", and it is still second-hand.
- **What would close it:** a human opening the ISO catalogue page in a browser and reading the
  stage. One minute of work that this environment cannot do. Until then, no OpenBiz document may
  say a 2026 edition of ISO 25964-1 **exists** — only that the revision is at FDIS and unpublished.
- **Opened:** iteration 25 (product-owner pass) · **narrowed:** iteration 50

### Every competitor-weakness claim we lean on comes from one source
- **Kind:** partial-coverage
- **What is proven:** the *structural* claims are well sourced — the Graphwise merger is confirmed by
  all three parties' own announcements, and VocBench's deployment weight is confirmed by its own
  release notes (RDF4J 4.3.15, GraphDB 10.6.2, an FTS plugin deployed by hand into `/lib/plugins`).
  Those are facts about what the products *require*, and we can stand behind them.
- **What is not:** the four PoolParty weaknesses in `docs/COMPETITIVE.md` — steep learning curve,
  very high consulting fees, blocking bugs with incomplete responses, and no public roadmap — all
  come from **Gartner Peer Insights practitioner reviews and nothing else.** The last of those is
  the single most load-bearing claim in our positioning: the entire "the roadmap is the repo" wedge
  row rests on it. One review aggregator is thin evidence for a permanent differentiator, and this
  pass found no second source either confirming or contradicting it.
- **What would close it:** a second independent practitioner source per claim — a G2 or TrustRadius
  corpus, a public procurement evaluation, or a conference talk by a customer. Or, for the roadmap
  claim specifically, the simpler and more direct check: does Graphwise publish a public roadmap or
  changelog for PoolParty today? That is answerable from their own site and was not attempted.
- **Opened:** iteration 25 (product-owner pass)

### The dev LLM shim has never been compared against a real provider
- **Kind:** environment-limited
- **What is proven:** nothing. `adr/0002` §3 makes the shim's fidelity load-bearing by design —
  development and production are meant to exercise the same `OpenAiCompatibleProvider` code path,
  differing only in base URL — and the ADR names divergence as a risk in its own consequences
  section. The shim is not yet built (Phase 10), so there is no code to be wrong.
- **What is not:** the standing check the ADR implies. This pass reviewed `adr/0002`'s provider
  *decisions* and confirmed they hold, but did not and could not exercise the shim.
- **What would close it:** Phase 10 running the agent evaluation sets against a real provider as
  well as the shim, and recording the diff. Recorded now so it is a known obligation entering the
  phase rather than a discovery inside it.
- **Opened:** iteration 25 (product-owner pass)

### ~~S24 and S27 are not implemented, so `skos:broaderTransitive` is one step and §8.4 is unchecked~~
- **Kind:** partial-standard
- **Closed, iteration 28.** S24 is applied by walking (`CoreModel::ancestry`) and S27 is read off
  the walk at build time. §8.5's Examples 25–29 each come out to the consistency the
  specification prints beside it, in one test that asserts all five; Examples 33, 36 and 37 are
  consistent as marked; and `openbiz ancestors <graph> <concept>` is the production caller, proven
  end to end against the binary on disk. See `docs/adr/0025`.
- **What replaced it, and it is not nothing:** the entry below on the *cost* of not storing the
  closure, and the entry below that on what the report still does not enumerate. The false green
  this entry named — "no SKOS integrity condition is violated by this graph" reading as a
  complete check — is now a three-way sentence, so a check that gave up says so; but the report
  still does not list *which* conditions it checked, so the sentence remains narrower than it
  reads for every condition this build has not implemented at all.
- **Opened:** iteration 24

### ~~The walk's cost is unmeasured, which is the other half of `adr/0024`'s question~~
- **Kind:** unproven-at-scale
- **What is proven:** the walk is bounded and terminates on a cyclic hierarchy, and the bound is
  reported rather than silently truncating. Correctness is covered by §8.5's and §8.6's own
  examples.
- **What is not:** anything about time or memory at size. `adr/0024` measured what *storing* the
  closure costs and this build stores nothing, so the open number is now the cost of **not**
  storing it: the S27 pass is one walk per concept with a `skos:related`, run inside every
  `openbiz inspect`, and `scale.rs` does not exercise it. A hierarchy that is deep and densely
  cross-linked associatively is the shape that would hurt, and no fixture in the tree is one.
- **The risk while it is open:** a vocabulary where `openbiz inspect` becomes slow or hits the
  bound, discovered by a customer rather than by us. The bound makes the failure honest — the
  report says the check was abandoned — but a validator that declines to answer is still one
  that declines to answer.
- **What would close it:** extending `crates/openbiz-skos/src/scale.rs` to the walk, at the same
  10k/100k/1M sizes and across the same four hierarchy shapes, plus a shape it does not yet have:
  a deep hierarchy with associative links on every concept. Proposed rather than done, because
  iteration 26 established that measuring a traversal that does not exist yet produces a number
  that agrees with you, and the same argument says the measurement belongs in its own iteration
  rather than in the one that built the thing.
- **Closed, iteration 30**, and it found a defect rather than a number. `scale.rs` gained an
  associative dimension and the shape this entry asked for — a deep hierarchy with a `skos:related`
  on every concept — and measured a legal 10 001-concept chain at **30.63 s against 62 ms** for the
  same vocabulary without the associative links. The cause was that `AncestryBound::max_links`
  bounded one walk while the pass makes one walk per concept, so the bound bounded nothing. Fixed
  by sharing the budget across the sweep: **530 ms**, with the abandonment reported. See
  `docs/adr/0027`. The 1M-concept row is still not run — see the entry that replaces this one below.
- **Opened:** iteration 28

### ~~The default ancestry bound has never been hit outside a test that lowered it~~
- **Kind:** partial-coverage
- **What is proven:** that hitting a bound is reported as `Severity::Unchecked` rather than read as
  a pass, and that the same graph with room comes out inconsistent — so the difference is
  demonstrably the bound and not the data. `AncestryBound::new(1, 8)` is how the test reaches it.
- **What is not:** the actual default, `AncestryBound::DEFAULT` — 100 000 ancestors and
  1 000 000 links. Nothing in the repository comes within three orders of magnitude of either, so
  the numbers are a judgement about vocabularies we have not seen. If they are too low a real
  thesaurus is refused an answer it should get; if they are too high the walk is the reason a
  request hangs, which is what the bound exists to prevent.
- **What would close it:** the scale measurement above, which would say what a walk of each size
  actually costs and turn the two numbers from a guess into a budget.
- **Closed, iteration 30.** `AncestryBound::DEFAULT` is now hit for real, in release, by
  `the_s27_pass_at_each_shape_with_an_associative_link_on_every_concept` — a 10 001-concept chain
  with one `skos:related` per concept owes about 50 million links against a budget of one million —
  and end to end through the binary by
  `inspect_says_the_disjointness_check_was_abandoned_rather_than_claiming_it_passed`, on a
  vocabulary of three thousand triples. What the numbers say about the *value*: a chain 1 000 deep
  is checked completely and one 1 500 deep is not, so the million is roughly "any hierarchy under a
  thousand levels, fully checked". `max_ancestors` (100 000) is still untouched by anything.
- **Opened:** iteration 28

### The disjointness sweep's budget is now a product limit, and nothing has been sized against a real thesaurus
- **Kind:** measured-and-over-budget
- **What is proven:** that the sweep stops rather than hanging, that it says how many concepts it
  never reached, and that `openbiz inspect`'s closing sentence hedges accordingly — all end to end
  through the binary. And the cost of stopping, measured: a 10 001-concept chain with a genuine S27
  violation on every concept reports **1 413 of 9 999 violations** before the budget runs out,
  against 999 of 999 at a thousand concepts.
- **What is not:** that a million links is the right number for anyone. It was chosen in iteration
  28 as a backstop against a pathological graph and `adr/0027` has now turned it into a limit an
  ordinary customer can reach — the check is complete for a hierarchy about a thousand levels deep
  and partial past that. No real thesaurus has been measured, so nobody knows whether real
  vocabularies sit at ten levels (in which case this never fires) or whether some enterprise
  hierarchy with a dense associative layer sits past it.
- **The risk while it is open:** a governance team reads "1 413 violations, check abandoned" and
  cannot tell whether the remaining 8 586 exist. That is honest and it is worse than an answer.
- **What would close it:** either the algorithmic fix or the scaling budget in `docs/PROPOSED.md`,
  or a measurement against a real published thesaurus (AGROVOC, EuroVoc, MeSH) — which needs a
  licence check on the data before it can enter the repository, and so is a decision rather than a
  task.
- **Opened:** iteration 30

### A long S27 violation path is unmeasured, because the harness only generates short ones
- **Kind:** partial-coverage
- **What is proven:** what a *short* violation costs. `Associativity::EveryConceptInHierarchy`
  relates each concept to its grandparent, and `Ancestry::path_to` is breadth-first, so the path
  carried by `Finding::RelatedAndBroaderTransitive` is three nodes however deep the hierarchy is.
- **What is not:** the case that would hurt. A vocabulary relating every concept to its *root*
  produces a path per finding proportional to the depth, so the findings alone hold memory
  quadratic in the vocabulary — and unlike the walk, nothing bounds what a finding holds. The
  sweep budget limits how many findings there can be, which caps it indirectly and by accident
  rather than by design.
- **What would close it:** a fourth associativity shape relating every concept to concept 0, run at
  the same sizes, and a decision about whether a finding's path needs a bound of its own.
- **Opened:** iteration 30

### ~~The semantic relation model holds four entries per stated link, and the ceiling is unmeasured~~
- **Kind:** partial-coverage
- **Closed, iteration 26.** The measurement it asked for exists: `crates/openbiz-skos/src/scale.rs`
  reports time, resident memory, held entries, derivations, report size and the S24 closure's size
  at 10k, 100k and 1M links across four hierarchy shapes, and the numbers are in
  `docs/adr/0024-semantic-relation-closure-scale.md`. The decision it asked for was taken: **S24's
  closure is not materialised**, because a legal 100 000-link chain licenses five thousand million
  pairs and because a stored entry cannot name the path it took.
- **What it found, and what has replaced it below:** the ceiling is not comfortable. A stated link
  costs **3.9 KiB resident**, 43× the size of the fact; a million-link vocabulary with **no labels
  at all** measured **4.4 GiB and a 62-second build**. That is a live problem with what is already
  shipped, not with what comes next, and it is now the two entries that follow.

### A stated semantic relation costs 3.9 KiB of memory, and a million-link vocabulary does not fit
- **Kind:** measured-and-over-budget
- **What is proven:** the number, three ways. `docs/adr/0024` measures a marginal
  **3.86 KiB per stated `skos:broader`** at a million links and **3.85 KiB** at a hundred thousand,
  against **0.70 KiB** for a typed concept that states nothing — so the relations are five times
  the rest of the model at one link per concept. A 1M-link tree held **4 376 MiB** (peak 5 081 MiB)
  and took **62.66 s** to build, of which 54.7 s was system time: it was paging, not computing.
- **What is not:** any judgement that this is acceptable. `CLAUDE.md` §1.5 asks for "modest memory
  at rest", and 4.4 GiB before a single `skos:prefLabel` is loaded is not that. The ADR decomposes
  it — roughly 900 B of eagerly-rendered derivation text, 390 B of cloned IRIs, and about 1 KiB of
  `BTreeMap`'s eleven-slot allocation floor paid for maps that hold one entry — but **fixes none of
  it**, because each fix changes a shipped public type and belongs to its own item.
- **What would close it:** the three reductions in `docs/PROPOSED.md` — derivations reconstructed
  on demand rather than pre-rendered, one direction stored per inverse pair, and a container
  without the eleven-slot floor — measured against the same harness, with a target stated in the
  plan rather than assumed.
- **Do not read this as "1M links is unsupported".** It completes and answers correctly. It is the
  memory that is wrong, and nothing in the product currently refuses a vocabulary for being large,
  which is the honest shape of the risk: a customer meets this as a slow machine, not as an error.
- **Opened:** iteration 26

### `openbiz inspect` renders a 1 GiB report from a million-link vocabulary, in one `String`
- **Kind:** measured-and-over-budget
- **What is proven:** the size. A derivation renders on **three** lines carrying the full text of
  the SKOS statement that licensed it, and there are three derivations per stated link, so the
  `why:` section alone is **10.3 MiB at 10k links, 103 MiB at 100k, and 1 033.8 MiB at 1M** —
  measured by `scale.rs` in the same format `inspect` uses, with a test pinning the two together.
- **What is not:** what the command does with it. `inspect` builds its whole report into a single
  `String` and prints it at the end, so at a million links the report is a gigabyte held *on top of*
  the 4.4 GiB model. Nothing measures that and no test runs `inspect` at that size.
- **Why it is not simply capped:** the module argues, at length and correctly, that a silent cap in
  an *inference* report is the one thing such a report must never do — "that is all there was" is
  precisely what a truncated explanation implies and precisely what is false. The fix is therefore
  a product decision (stream rather than buffer? a cap that names itself and the flag that lifts
  it?) and not a constant. In `docs/PROPOSED.md`.
- **Opened:** iteration 26

### A candidate's evidence is kept forever, and nobody has decided for how long
- **Kind:** partial-coverage
- **What is proven:** an applied candidate's staging graph survives the decision, and so does a
  rejected one, so "what exactly was approved" stays answerable after the vocabulary has moved on.
  Tests assert both.
- **What is not:** the cost. An approved import is stored **twice** — once staged, once in the
  vocabulary — forever, and a 100k-concept import doubles the store on disk for the life of the
  deployment. Nothing measures that, nothing prunes it, and there is no retention policy because
  *how long to keep the evidence* is a compliance decision rather than an engineering one. The
  behaviour is the conservative default (never delete evidence) taken deliberately, not an oversight,
  but a customer with a monthly bulk import will notice.
- **What would close it:** a retention policy a deployment can configure, and a measurement of what
  duplication actually costs at the scale `adr/0013` used. The policy is in `PROPOSED.md`.
- **Opened:** iteration 17

### ~~A candidate cannot propose a removal, so nothing that needs one is reachable~~ — CLOSED, iteration 18
- Closed by part 2 of the seam: `Store::propose_retraction`, `openbiz retract`, and an apply path
  that removes inside the transaction recording the decision. A removal is refused if it names
  statements the vocabulary does not hold, and refused again at approval if the vocabulary has moved
  since. See `adr/0018`. The narrower gap that remains — nothing raises a candidate carrying *both*
  halves — is its own entry below.

### ~~Nothing produces a candidate that both adds and removes~~ — CLOSED, iteration 42
- **Kind:** no-production-caller
- **Closed by the first bulk operation.** `openbiz move` raises a candidate with both halves
  through `Store::propose_edit`, and it is proven the way this entry asked. Both halves land:
  asserted end to end against the real binary, by taking a backup after the approval and reading
  the two statements off it rather than out of the code that wrote them. The
  removals-before-additions order `apply_payload` documents is pinned by a store test staging **one
  statement in both halves** — the only shape that can observe the order, and one no producer in
  this build computes, so it is a promise the record makes rather than a path anything walks.
  See `adr/0037`.
- **What replaced it, and it is not nothing:** the three entries below on what `openbiz move` does
  not do. The seam is exercised; the *operation* has a bounded scope.
- **Opened:** iteration 18

### The addition and removal counts count parsed statements, not distinct ones
- **Kind:** partial-coverage
- **What is proven:** the counts a candidate reports match the number of statements in its file for
  every fixture in the suite, and the staleness refusal's arithmetic is computed against the
  *staged graph* rather than against the recorded count, so it is unaffected by this.
- **What is not:** a file naming the same statement twice reports two. RDF graphs are sets, so only
  one is staged, and "removes 2 statements" would then be a number one greater than what the
  reviewer's diff shows. Pre-existing on the additions side since iteration 17 and inherited by
  removals. Nothing tests it either way.
- **Why not fixed here:** counting distinct statements means either a second full read of the
  staging graph or holding a set of every quad in memory, and the import path is already documented
  as holding the whole file in the write batch. Making its memory ceiling worse to fix a count that
  is wrong only for a malformed file is the wrong trade to take silently.
- **What would close it:** a decision on whether to count distinct or to report both numbers, plus
  a test with a duplicated statement in the file.
- **Opened:** iteration 18

### An approved removal's staging graph is the only copy of what was taken away
- **Kind:** partial-coverage
- **What is proven:** the removals graph survives approval — asserted end to end, from a backup of
  the store after the statements have left the vocabulary.
- **What is not:** the consequence for retention. For an *addition*, deleting candidate evidence
  loses the provenance of statements that are still in the vocabulary; for a **removal** it loses
  the statements themselves, permanently, with no other copy anywhere in the store. That makes the
  retention policy proposed at iteration 17 a materially bigger decision than it was, and it is
  still undecided, so the conservative default (keep everything) is the only safe one.
- **What would close it:** the retention policy in `PROPOSED.md`, which must now treat the two
  halves differently or say explicitly why it does not.
- **Opened:** iteration 18

### The blank-node round trip depends on Oxigraph's choice of labels
- **Kind:** partial-coverage
- **What is proven:** measured rather than assumed, and pinned by a test. An N-Triples export of a
  vocabulary retracts from that vocabulary with a blank node in it, because our serialiser writes
  labels our parser reads back as the same node. A hand-written `_:note` is refused by the presence
  check rather than removing something adjacent. Both directions are asserted.
- **What is not:** anything about *why* that holds. No RDF specification requires a serialiser to
  emit labels that a parser will map back to the same nodes — it is a property of this Oxigraph
  version, and the test is what would catch it changing. It is also only proven for N-Triples: the
  round-trip test over all six syntaxes uses a fixture with no blank nodes in it.
- **What would close it:** extending the six-syntax round-trip fixture to include a blank node, and
  a note in `adr/0018` if any syntax turns out to behave differently.
- **Opened:** iteration 18

### An import holds the whole file in the backend's write batch, and the ceiling is unmeasured
- **Kind:** partial-coverage
- **What is proven:** an import is one transaction, so a file that fails to parse two thirds of the
  way through stages nothing — proven by a test that checks no candidate is left behind after a
  syntax error.
- **What is not:** the memory that costs. This is the same unmeasured ceiling `Store::restore`
  carries (see the restore entry below) and it arrives sooner, because an import is something a user
  does routinely rather than once after a disaster. `adr/0013` measured a million concepts loading
  through the transactional write path in five minutes and ~6 GB; an import of that size has never
  been run through the seam and would additionally hold the staged copy.
- **What would close it:** importing a large file and measuring resident memory, then deciding
  whether a bounded-memory import is worth the half-staged state it would cost.
- **Opened:** iteration 17

### The candidate list and record reads are unmeasured above a handful
- **Kind:** partial-coverage
- **What is proven:** listing works and is ordered by identifier; reading one candidate reads its
  whole record in a single subject scan.
- **What is not:** `Store::candidates` reads every candidate record in the system graph and then
  re-reads each one, so it is linear in the number of candidates ever raised — and nothing is ever
  deleted, so that number only grows. Minting an identifier scans the same set. A deployment doing a
  bulk import a day would have thousands of records within a few years and nobody has measured what
  that does to `openbiz candidates` or to the *proposal* path, which pays the scan on every write.
  Same class as the registry-read entry below, with a worse growth curve.
- **What would close it:** a benchmark in the `scale` harness at 1k / 10k / 100k candidates, and, if
  it bites, a sequence counter in the system graph instead of a max-scan.
- **Opened:** iteration 17

### The migration that rewrites nothing is proven to run, not proven to be unnecessary
- **Kind:** inspected-only
- **What is proven:** migration `0003-allow-candidate-graphs` runs on open and on restore, reports
  itself, records itself, and moves the stamp — end to end, through the real binary, from a
  hand-written version-2 backup.
- **What is not:** the claim that it *needs to write nothing* is a claim that every version-2 store
  is already a valid version-3 store, and that is reasoning from the diff rather than a measurement.
  It is true for every store this build can produce; it rests on the change being purely additive,
  which is exactly the kind of belief that is right until somebody adds a non-additive detail to a
  "purely additive" version.
- **What would close it:** nothing cheap. The honest mitigation is that the next migration to claim
  it writes nothing should be viewed with more suspicion than this one was.
- **Opened:** iteration 17 · **Amended:** iteration 18 — `0004-allow-candidate-removals` is the
  second migration to write nothing, and the same doubt applies to it unchanged. It was viewed with
  the suspicion this entry asked for, which produced one thing the 2 → 3 step does not have: a
  **one-step** end-to-end test from a hand-written version-3 backup, because a chain test passes
  whether the last step ran or was skipped by an off-by-one — every earlier step having run is
  enough to make the content assertions hold. Version 3's step still has no such test.

### An approval is attributed to whoever the operating system says ran the command
- **Kind:** partial-coverage
- **What is proven:** a decision with nobody to record is refused, by the store and by the command
  line, and the refusal names the variable to set. The recorded string says the decision came from
  the command line.
- **What is not:** it is not an *authenticated* identity and cannot be. `OPENBIZ_ACTOR` is an
  environment variable anybody with shell access can set to anything, and `USER` is barely better.
  For an audit trail whose whole purpose is attribution, that is a real limitation and not a
  cosmetic one — the trail records a claim, not a verified fact. It is honest today because the
  product has no authentication at all and does not pretend to; it stops being honest the moment
  somebody reads the trail as proof.
- **What would close it:** authentication, which part 3 of the candidate seam waits on.
- **Opened:** iteration 17

### No migration has ever run against a store an older build actually wrote
- **Kind:** partial-coverage
- **What is proven:** the 1 → 2 migration runs on open and on restore, keeps a populated store's
  content, records itself in the system graph, reports itself to the caller, and does not repeat on
  the next open. A synthetic two-step chain proves the engine applies steps in chain order, stamps
  once at the end, rolls back whole when a step fails, and refuses a chain with a gap. End to end,
  the real binary restores a hand-written version-1 backup and serves the result.
- **What is not:** **every version-1 store in these tests was made by degrading a version-2 store** —
  `clear_graph` on the system graph, then a stamp of 1 — because the build that wrote real version-1
  stores no longer exists and never shipped. That fixture is our belief about what version 1 looked
  like, not a version-1 store. If a real one differs in any way we have forgotten, the migration
  meets it for the first time on a customer's disk. The end-to-end backup fixture is hand-written
  from the specification rather than degraded, which is a partial answer and the better one.
- **What would close it:** keeping a byte-exact store directory (or backup file) from each released
  version as a fixture, from the first release onwards. That is a release-process decision, so it is
  in `PROPOSED.md` rather than done here.
- **Opened:** iteration 16 · **Amended:** iteration 18 — the version-3 fixture added this iteration
  is hand-written from the specification too, so the end-to-end suite now carries three
  authored-not-degraded older-format backups (1, 2, 3). The store's *unit* fixtures are still
  produced by degrading a current-format store, so the substance of this entry is unchanged.

### A migration holds its whole rewrite in the backend's write batch, and no migration has rewritten content
- **Kind:** partial-coverage
- **What is proven:** the chain runs in one transaction, so a failed step leaves the store exactly
  as it was. The only real migration writes five quads and touches no content.
- **What is not:** the memory ceiling `adr/0015` records for restore applies here too and is
  measured no better. A future migration that rewrites, say, every literal in a million-concept
  store would hold that rewrite in memory, and there is no chunking, no progress reporting, and no
  estimate a build could refuse on. A migration that OOMs part way is safe — nothing commits — but
  the deployment is then unstartable with no way forward except restoring a backup into an older
  build, which is exactly the situation the operator was upgrading to leave.
- **What would close it:** the same measurement `adr/0015`'s entry asks for, plus a decision about
  whether a content-rewriting migration must stream. Deferred until a migration actually needs to
  rewrite content — deciding it now would be guessing at the shape of the problem.
- **Opened:** iteration 16

### A store whose format version has no migration is now refused, and nothing has ever hit that path in anger
- **Kind:** partial-coverage
- **What is proven:** version 0 — which never existed — is refused on open and on restore, with the
  missing version named and an action stated. The chain-integrity test fails the build if
  `FORMAT_VERSION` is bumped without its migration.
- **What is not:** the refusal exists for a build that has *dropped* a migration it once had, which
  is a thing no release has done because there has been one release lineage and no removals. The
  message tells an operator to "upgrade one release at a time"; nobody has ever had to, so whether
  that instruction is actionable — whether the intermediate build is obtainable — is a release-and-
  distribution question the loop cannot answer (`CLAUDE.md` §8).
- **What would close it:** a documented support policy stating how many format versions back a
  build migrates from, and an archive of builds that makes stepping through possible. Commercial and
  release-process decisions; see `PROPOSED.md`.
- **Opened:** iteration 16

### CI's toolchain install is bounded now, but the retry and the timeout have never fired
- **Kind:** partial-coverage
- **What is proven:** the `have_toolchain` detection was tested locally in all three states that
  matter — no clang, clang present but `libclang` absent, and both present — and returns the right
  answer in each without tripping `set -e`. The middle case is the load-bearing one: it is the state
  that otherwise produces a `bindgen` panic several steps later with no named cause. Both jobs are
  generated from an identical step (checked by `diff`), and the YAML parses.
- **What is not:** on `ubuntu-latest` the detection short-circuits to "already present", so **the
  apt branch does not execute in CI at all** — the three-attempt retry, the `::error::` after a
  third failure, and the post-install assertion are all unexercised by any real run. Equally, no
  build has yet hit the 6-minute step timeout or a 45-minute job timeout, so what a timeout actually
  *reports* on the PR is inferred from GitHub's documented behaviour, not observed. The whole change
  is a fix for a failure mode I can no longer reproduce on demand.
- **What would close it:** a `workflow_dispatch` input that forces the apt branch (and one that
  forces a sleep past the step timeout), run once deliberately to see both report red with a
  readable cause. Worth doing the next time CI is touched for another reason.
- **Opened:** iteration 15

### A restore holds the whole file in the backend's write batch, and the ceiling is unmeasured
- **Kind:** partial-coverage
- **What is proven:** a restore is one transaction, so a failure anywhere rolls the whole thing
  back — tested at 10 001 quads, which is deliberately more than the 10 000-quad batch our own
  buffer flushes at, so the test proves rollback across *committed-to-the-backend-but-uncommitted*
  batches rather than within a single one. `adr/0015` records why chunked commits were refused.
- **What is not:** how much memory a large restore actually needs. The backend holds a
  transaction's write batch in memory until commit, so a store of the size `adr/0013` measured —
  a million concepts, ~6 GB on disk — may need an amount of RAM nobody has measured, and the
  failure mode is an OOM kill part way through a disaster recovery. Note the asymmetry: **backup
  streams and restore does not**, so a deployment can produce a file it cannot read back on the
  same machine. That is the shape of the problem worth testing first.
- **What would close it:** restore a generated 1M-concept backup — the `scale` module already
  builds one — under a measured memory ceiling, and record the number in `adr/0015`. If it will
  not fit, the decision to reopen is chunked commits *plus* a marker that makes a half-restored
  store refuse to open, which is a different design rather than a weakening of this one.
- **Opened:** iteration 14

### There is no online backup: taking one means stopping the server
- **Kind:** partial-coverage
- **What is proven:** `openbiz backup` and `openbiz restore` work against a stopped deployment, and
  the embedded store's exclusive lock makes that a refusal rather than a race — a backup attempted
  while the server runs fails with "already in use by another OpenBiz process".
- **What is not:** anything about backing up a *running* deployment, because there is no way to.
  For a self-hosted product whose pitch is that it is one process, "stop the service to back it up"
  is a real operational cost that the JVM incumbents, with their separate triplestore, do not all
  pay. It is also the reason the backup's two snapshots (below) cannot yet bite.
- **What would close it:** the authenticated `GET /api/backup` in `PROPOSED.md`, which needs the
  authentication model that does not exist yet. Until then this is a documented limitation, and
  `README.md` says so rather than leaving an operator to discover it from a lock error.
- **Opened:** iteration 14

### A backup's registry read and its scan are two snapshots
- **Kind:** inspected-only
- **What is proven:** the quad scan is a single backend snapshot, so the *statements* in a backup
  are internally consistent. This is the same property `Store::export_graph` has, and it is read
  from the backend's source rather than assumed.
- **What is not:** that the graph count in `BackupReport` describes the same instant as the
  statements, because the registry is read first, on its own snapshot. It cannot matter today —
  the only process that can write is the one taking the backup, and it is not serving — but it
  becomes real the moment an online backup exists, and by then the code will look correct.
- **What would close it:** a read transaction spanning both, which the backend supports; do it in
  the same change that adds an online backup, not before, because a read transaction held for the
  length of a full scan has its own cost.
- **Opened:** iteration 14

### The backup round trip is proven only on content written through the crate's own transaction
- **Kind:** partial-coverage
- **What is proven:** a store holding two vocabularies with SKOS-shaped statements backs up,
  restores into a fresh store, and comes back byte-identical line-for-line — checked through
  `Store::export_graph` and, end to end, through a running server's `GET /api/export`. A
  created-but-empty vocabulary survives too, which is the case a content-based backup could most
  easily lose.
- **What is not:** anything about content an *author* wrote, because there is no authoring path
  yet (Phase 2). The statements in these tests are written through `Transaction::insert`, which is
  crate-private. So the round trip is proven over the terms those tests chose: IRIs, a
  language-tagged literal, a plain literal. **Not** proven: blank nodes (N-Quads labels are
  preserved by the parser's default, which is inspected and not tested), RDF-star terms, hostile
  Unicode in a label, a literal past the boundaries `adr/0014` measured, or a vocabulary large
  enough to matter.
- **What would close it:** a round-trip test over a hostile fixture — one deliberately built from
  every term shape the store can hold — added when Phase 2 gives us a real authoring path to build
  it through. A blind-spot iteration is the natural home for it.
- **Opened:** iteration 14

### ~~Restore reads one syntax of six, and reads a store rather than importing into a vocabulary~~ — CLOSED, iteration 17
- **Kind:** partial-standard
- **What is proven:** all six syntaxes now parse through a real production caller — `openbiz
  import` — and each is round-tripped against the serialiser: a vocabulary is exported, re-proposed
  as a candidate, and the staged statements compared to the source graph statement for statement.
  Malformed input is refused with a one-based line number, and an extension we do not know is
  refused naming the six we do rather than guessed at.
- **What was not, and now is:** an *import* — statements landing in somebody's existing
  vocabulary — exists, behind the candidate seam that `CLAUDE.md` §3 required it to wait for.
- **What remains open, separately:** the round trip is proven against *our own* reader, which is
  fidelity rather than conformance. That is the pre-existing entry further down this file and it is
  unchanged. And there is still no direct-write import, deliberately.
- **Opened:** iteration 14 · **Closed:** iteration 17

### A SHACL `sh:datatype` constraint over a derived integer type can never be satisfied
- **Kind:** partial-standard
- **What is proven:** the store replaces the datatype IRI of every derived integer type — `xsd:int`,
  `xsd:short`, `xsd:byte`, `xsd:long`, `xsd:unsignedLong`, `xsd:nonNegativeInteger`,
  `xsd:positiveInteger` — with `xsd:integer` whenever the value fits `i64`. Measured through both
  `export_graph` and `DATATYPE()` over `Store::query`, so it is the stored term and not a
  serialisation artefact. Pinned by
  `literal_precision::tests::a_derived_integer_datatype_is_replaced_by_xsd_integer`.
- **What is not:** the consequence for Phase 4. A SHACL shape asserting `sh:datatype xsd:int`
  cannot match anything in this store, because the datatype the shape names is not the datatype the
  store returns. Nothing has been built that would notice, because the SHACL engine does not exist
  yet. The same applies to an OWL 2 datatype range over a derived type in Phase 5.
- **What would close it:** the Phase 4 spike evaluating SHACL engines must include a
  `sh:datatype xsd:int` case against real stored data, and record the answer rather than assume the
  engine is at fault. If the substitution is still present then, the rule packs must be written in
  terms of datatypes the store preserves, and that constraint must be *stated* rather than silently
  worked around.
- **Opened:** iteration 13

### Nobody has looked upstream at whether the datatype substitution is fixable
- **Kind:** inspected-only
- **What is proven:** that the substitution happens, and that it happens in the term encoding rather
  than in the serialiser.
- **What is not:** *why*. `adr/0014` reasons that arbitrary-precision integers are a property of the
  value representation and therefore expensive to change, and that reasoning is sound for the
  **range** boundary. It does not follow for the **datatype substitution**, which may be a much
  cheaper thing — a `Literal` construction detail, a configuration, or an upstream bug — and no one
  has read Oxigraph's source or issue tracker to find out. The ADR records it as unexamined rather
  than as hard, deliberately, because assuming it is hard is how a cheap fix goes unfound.
- **What would close it:** half an iteration reading `oxigraph::model::Literal` and the term encoder,
  and searching upstream issues for the derived-datatype case. That is a bounded, concrete task and
  it should happen before the Phase 4 spike, not after.
- **Opened:** iteration 13

### The literal boundaries are measured on this backend version only
- **Kind:** partial-coverage
- **What is proven:** the exact thresholds for `xsd:integer` (`i64`), `xsd:decimal` (128-bit fixed
  point, 18 fraction digits), `xsd:float`/`xsd:double` (IEEE 754 saturation), and the XSD-validity
  test for calendar and duration types, against the pinned Oxigraph version.
- **What is not:** that these are *stable*. They are properties of an internal representation that
  upstream has never documented as a contract, so a patch release could move them. The tests fail if
  that happens, which is the mitigation, but the failure will read as "our test broke" rather than
  "the store's numeric range changed" unless the reader gets to `adr/0014`. The assertion messages
  name the ADR for that reason.
- **What would close it:** nothing available to the loop — it is a dependency-stability question, not
  a coverage one. Recorded so a version bump is met with a re-read rather than a re-baseline.
- **Opened:** iteration 13

### What an import should do with a literal that will not survive is undecided
- **Kind:** partial-coverage
- **What is proven:** that some literals lose their value, some lose their datatype, and some
  collapse into an existing statement — all silently, on the write path that already exists.
- **What is not:** what the *parser* should do about it, because the parser is deliberately deferred
  behind the candidate seam. Refuse the literal, accept it with a recorded warning, or accept it
  silently are three different products, and the current answer is the third by default rather than
  by decision.
- **What would close it:** the Phase 2 candidate seam item must answer it explicitly. A candidate
  already carries provenance and is already reviewed by a human, so "this literal will be stored
  differently from how you wrote it" is a fact the seam is shaped to carry — but only if someone
  puts it there.
- **Opened:** iteration 13

---

### The service-defined default dataset is believed spec-permitted, not verified against the text
- **Kind:** partial-standard
- **What is proven:** the behaviour itself is thoroughly tested — a query naming no dataset sees the
  vocabulary graphs and nothing else, a query naming its own `FROM` reaches ours verbatim, and a
  store with no vocabularies answers nothing rather than everything.
- **What is not:** that this is *conformant*. `adr/0011` chooses a default dataset that is not "the
  store's default graph", and the justification rests on SPARQL 1.1 permitting a service to define
  its own default dataset when a query specifies none. That reading was **not checked against the
  specification's own words** this iteration — it is recalled, not cited, and `CLAUDE.md` §4.5 says
  a standards claim needs the spec behind it. If the reading is wrong, this is a documented
  deviation rather than a conformant choice, and the honest word for it in user-facing text changes.
- **A second, narrower gap even if the reading is right:** query *portability*. The same query text
  returns different answers here and against a standards-configured endpoint over the same data.
  That is a property of every service-defined default dataset, and nothing warns a user copying a
  query in or out. A SPARQL Service Description at the endpoint would be the standard way to make
  the dataset self-describing rather than documented-elsewhere; we do not serve one.
- **What would close it:** read SPARQL 1.1 Query §13 and SPARQL 1.1 Protocol §2.1.4, cite the
  clause in `adr/0011` or correct the ADR, and consider a Service Description.
- **Opened:** iteration 9

### SPARQL 1.1 Protocol is implemented for query only, and two of its parameters are refused
- **Kind:** partial-standard
- **What is proven:** all three of the protocol's request forms for a *query* — `GET ?query=`,
  `POST application/sparql-query`, and `POST` form-encoded — are tested end to end against the
  router, and all four result formats are tested for both `SELECT` and `ASK`.
- **What is not:** the protocol's `default-graph-uri` and `named-graph-uri` parameters are a named
  400, not an implementation. There is no Update endpoint (its own plan item) and no Graph Store
  Protocol (likewise). So the honest claim this build supports is **SPARQL 1.1 Query over the
  protocol's three query forms**, not "SPARQL 1.1 Protocol". Do not write the latter anywhere
  user-facing.
- **What would close it:** deciding how a protocol-supplied dataset composes with the
  vocabulary-graph default of `adr/0011` *and* with a query's own `FROM` — the three-way
  interaction is the reason it was deferred rather than guessed — then implementing it with the
  spec's own examples as tests.
- **Opened:** iteration 9

### The SPARQL endpoint has no query console in the interface
- **Kind:** no-production-caller
- **What is proven:** the endpoint's production caller is HTTP: routed at `/api/sparql`, tested
  through the real router, and usable from `curl` or any SPARQL client.
- **What is not:** nothing in the UI calls it. `CLAUDE.md` §4.4 requires anything user-facing to be
  reachable in the interface and keyboard-navigable, and a query console is plainly user-facing.
  The plan item is scoped to the endpoint, so this is recorded as an open gap rather than folded
  into that item and quietly called done — but a taxonomist cannot run a query in OpenBiz today.
- **What would close it:** a console in the interface — an editor, a format chooser reading the
  server's own list, a results table, and the refusals rendered as text rather than as a status
  code. It is a UI item and it is not in the plan; recorded in `PROPOSED.md`.
- **Note, same iteration:** `GET /api/sparql/formats` was added so the list a console needs is
  already served, and so that `preserves_term_detail` has a production reader rather than being a
  constant only its own test consults. It has an HTTP caller and **no UI caller** — the warning
  that CSV silently drops language tags is now *available* to an interface and still not *shown* to
  a user, which is the half of this gap that closing the endpoint did not close.
- **Opened:** iteration 9

### The endpoint buffers a whole answer in memory, twice
- **Kind:** partial-coverage
- **What is proven:** the buffer is *deliberate* and load-bearing. `Store::query` may leave a
  partial document in its writer when it refuses, and a truncated results document is syntactically
  valid and semantically wrong, so the HTTP layer buffers into a `Vec` and only builds a response
  on `Ok`. Tests assert a refused query's body carries the refusal and none of the partial answer.
- **What is not:** the cost. A `SELECT` answering at the 100 000-row cap holds the serialised
  document in a `Vec` and then again in the response body, and no test measures either. The same
  gap as the export endpoint's, one layer up, and with a larger worst case because the cap is
  bigger than any vocabulary we have.
- **What would close it:** a real measurement at the cap, then either a bounded streaming body that
  can still discard a partial document, or a documented maximum response size.
- **Still open after iteration 11.** `adr/0013` measured time, not memory, so it did not touch this.
  It did narrow the worst case worth measuring: the largest answer any probe produced was 111 110
  rows, and it was refused by the row cap before the body was built — so the practical worst case is
  an answer just under 100 000 rows, not an unbounded one.
- **Opened:** iteration 9

### The query limits are hard-coded — ~~and the defaults are chosen rather than measured~~
- **Kind:** inspected-only
- **What is proven:** both bounds work and both refuse rather than truncate — the row cap is tested
  for solutions and for constructed triples, and the deadline is tested to cancel a runaway join.
  `QueryLimits` is a parameter type, so wiring it to configuration touches no caller.
  **Half closed, iteration 11.** The numbers are no longer reasoned: `adr/0013` measures them, and
  both were wrong in a way reasoning would not have found. The **100 000-answer cap refuses a
  legitimate query at a million concepts** — "everything under this branch" answers with 111 110
  rows in 1.6 s and is refused, which is the designed behaviour and still a capability the customer
  does not have. And the **30 s deadline does not protect interactivity**: the concept tree's own
  first query takes 21.6 s at 1M and is *served*, because 21.6 s is inside 30 s.
- **What is not:** nothing reads configuration into it; every production call still uses
  `QueryLimits::default()`, so a deployment whose largest subtree exceeds 100 000 concepts cannot
  raise the cap. And there is still no second, smaller bound for queries a human is waiting on —
  the deadline is a runaway guard and nothing else.
- **What would close it:** config keys with the provenance `adr/0005` requires, and a decision on
  an interactivity budget. Both are in `PROPOSED.md`.
- **Opened:** iteration 9 · **half closed:** iteration 11

### What the scale spike did not measure: concurrency, memory, a cold cache, and a lumpy vocabulary
- **Kind:** partial-coverage
- **What is proven:** `adr/0013`'s timings, load rate, and disk figures, at 10k / 100k / 1M
  concepts, each probe's answer count asserted against the generator before its timing was
  believed, median of three runs after a warm-up, on the machine the ADR names.
- **What is not**, and each of these could move the numbers materially:
  - **Concurrency.** One process, one query at a time, no writer running. What a 21-second query
    does to nine other users, and what the `adr/0009` write lock costs under a concurrent load, is
    unmeasured. Partly environment-limited (`CLAUDE.md` §8).
  - **Memory.** Timings only. The endpoint buffers a whole answer twice (entry below) and this run
    did not weigh it, so the §1.5 "modest memory at rest" commitment is still unevidenced under
    load.
  - **A cold cache.** The page cache is warm because the load had just written the data. The
    first query after a restart, against a store larger than RAM, is a different number and is the
    one an operator actually meets in the morning.
  - **Disk after compaction.** ~840 bytes per quad was measured immediately after loading, with no
    compaction run. It is an upper bound; the settled size is unknown.
  - **A realistic vocabulary shape.** The fixture is a balanced ten-way tree with uniform label
    lengths. Real thesauri have concepts with thousands of children and label lengths spanning two
    orders of magnitude, and a regular shape flatters an index.
- **What would close it:** for the first two, Phase 13's benchmark harness with a concurrent driver
  and RSS sampling. For the cold cache, a restart between load and probe — cheap, and deliberately
  not folded into this iteration. For the shape, a fixture derived from a real published
  vocabulary, which needs the licence question `CLAUDE.md` §6 raises about fixture data settled
  first.
- **Opened:** iteration 11

### The timeout answers 503, which is the least-wrong code rather than a right one
- **Kind:** inspected-only
- **What is proven:** the mapping is deliberate and argued in `adr/0011`. RFC 9110 has no code for
  "the server cancelled a valid request against its own resource policy"; 408, 504, and 500 each
  claim something untrue.
- **What is not:** the consequence. A load balancer or a service mesh reading 503 may take the
  instance out of rotation over one expensive query, which would turn a bounded refusal into an
  availability incident. No deployment has met this and nothing tests it.
- **What would close it:** running behind a real proxy with health checking and watching what one
  timed-out query does to rotation. Environment-limited in part — a realistic answer needs a
  deployment topology this machine does not have.
- **Opened:** iteration 9

### The query tests put statements in a vocabulary through the backend, not through an authoring path
- **Kind:** partial-coverage
- **What is proven:** the fixture is honest about itself and could not be otherwise today — no
  public API can put a statement into a vocabulary graph, because the store creates the container
  and Phase 2's candidate seam is what fills it.
- **What is not:** that a query sees what the *real* authoring path will actually write. The
  fixture inserts through `store.backend` directly, so it bypasses the write choke point, the
  transaction, and whatever shape the candidate seam settles on. If authoring later writes a
  different shape — reified statements, provenance quads alongside content — these tests keep
  passing against a shape production never produces.
- **What would close it:** rewriting the fixture onto the real authoring API the moment one exists.
  This is a deliberate debt with a named trigger, not an oversight.
- **Opened:** iteration 9

### Two query tests are timing-sensitive and could flake on a loaded machine
- **Kind:** partial-coverage
- **What is proven:** they pass repeatedly on this machine, and both are testing something real —
  that the deadline actually cancels, and that a watchdog never cancels a query that already
  finished (run 40 times over, because a watchdog cancelling a *later* query would otherwise show
  up as an intermittent failure and nothing else).
- **What is not:** their behaviour under contention. `a_quick_query_is_never_cancelled_by_its_own_watchdog`
  gives a one-statement query a 30 ms deadline; on a heavily loaded CI runner that query could
  genuinely exceed 30 ms and the test would fail for a reason that is not the bug it hunts. The
  tight deadline is *why* the test discriminates, so widening it to remove the flake would also
  remove most of its power — that trade is recorded rather than taken.
- **What would close it:** an injectable clock, or a deterministic cancellation hook, so the race
  is exercised without depending on wall-clock timing at all.
- **Opened:** iteration 9

---

### ~~The round trip is proven against our own reader~~ — HALF CLOSED, iteration 10
- **Kind:** partial-standard
- **What is proven:** every one of the six syntaxes survives serialise → parse → compare, over
  content chosen to be hostile: two language tags in non-Latin and accented scripts, an
  `xsd:integer`, a literal carrying a quote, a newline, a backslash and an emoji, an IRI with a
  percent-encoded space, and a blank node. Four mutants of the serialiser were confirmed to break
  it. Empty graphs are proven to produce *readable empty documents* in all six, which is the case
  that separates RDF/XML and JSON-LD from the line-based syntaxes.
- **What is not:** the reader in that round trip is the same library as the writer. Self-consistency
  is what is proven; **conformance is not**. If Oxigraph's Turtle writer and Turtle reader shared a
  misreading of the grammar, this test would pass and a third-party consumer would still choke.
  `CLAUDE.md` §4.5 requires a standards claim to be backed by the spec's own examples, so until
  that exists the claim this build makes is *round-trip fidelity*, not "we implement Turtle".
  Nothing yet reads an OpenBiz export with a tool we did not write.
- **What would close it:** run the W3C RDF test suites (rdf-tests) for each syntax against our
  export path, and — cheaper and worth doing first — assert a handful of exports byte-for-byte
  against fixtures produced by an independent tool (`rapper`, `riot`) so a divergence surfaces as a
  diff rather than as a customer's failed import.
- **Opened:** iteration 8
- **Half closed, iteration 10.** Two of the six are now checked against a reader **we wrote from
  the published EBNF**, sharing no code with the writer: `crates/openbiz-store/src/spec_conformance.rs`
  reads our N-Triples and N-Quads exports against [N-Triples §7] and [N-Quads §4], enforces the
  absolute-IRI requirement of [N-Triples §2.2], checks the five layout constraints of
  [Canonical N-Triples §4], and compares our bytes against [N-Triples Example 3] as published. The
  checker is itself proven to discriminate: twenty-one documents each violating exactly one named
  production or constraint are required to be rejected. See `adr/0012`.
  **It found two defects** — both now their own entries below, and both invisible to the round
  trip that preceded it. That is the answer to the question this entry was opened to ask.
  **Still open for the other four.** Turtle, TriG, RDF/XML, and JSON-LD remain proven only against
  our own reader, and the wording above applies to them unchanged. The W3C rdf-tests suites remain
  the thing that would close it properly for all six; that is now a `PROPOSED.md` item rather than
  a line in this entry, because folding it in here is what made it feel handled for two iterations.

  [N-Triples §7]: https://www.w3.org/TR/n-triples/#sec-grammar
  [N-Quads §4]: https://www.w3.org/TR/n-quads/#sec-grammar
  [N-Triples §2.2]: https://www.w3.org/TR/n-triples/#sec-iri
  [Canonical N-Triples §4]: https://www.w3.org/TR/n-triples/#canonical-ntriples
  [N-Triples Example 3]: https://www.w3.org/TR/n-triples/#sec-literals

### The store silently rewrites the lexical form of any literal it can interpret
- **Kind:** partial-standard
- **What is proven:** measured exactly, and pinned by
  `the_store_rewrites_the_lexical_form_of_the_datatypes_it_models_natively`. Written in, read back
  out: `"1.663E-4"^^xsd:double` → `"0.0001663"`; `"1.0E1"^^xsd:float` → `"10"`;
  `"007"^^xsd:integer` → `"7"`; `"+7"^^xsd:integer` → `"7"`;
  `"007"^^xsd:nonNegativeInteger` → `"7"`; `"4.00"^^xsd:decimal` → `"4"`;
  `"1"^^xsd:boolean` → `"true"`; `"2026-08-19T00:00:00+00:00"^^xsd:dateTime` →
  `"2026-08-19T00:00:00Z"`. Untouched: `xsd:string`, a datatype the engine does not know, an
  already-canonical lexical form, and — the perverse case — a value that is *invalid* for its
  datatype (`"abc"^^xsd:nonNegativeInteger` survives byte-for-byte). `RDF 1.1` defines a literal as
  the pair (lexical form, datatype IRI), so these are **different terms**, not different spellings.
  `two_triples_that_differ_only_in_lexical_form_collapse_into_one` proves the sharper harm: two
  distinct triples go in and one comes out.
- **What is not:** **nothing about this is disclosed to a user.** No API field, no export header, no
  interface warning. `RdfSyntax::records_graph_names` exists precisely so a different kind of
  silent loss is stated before a download, and this one is larger and unstated. **The loss is the
  store's, not the export's** — `the_rewrite_is_the_stores_and_not_the_exports` runs a `CONSTRUCT`
  that never touches `export_graph` and gets `"7"` back too, so every reader inherits it and a fix
  has to touch stored data rather than a serialiser. Still unmeasured: whether the rewrite happens
  at insert or at read (it is the term encoding either way, but which one decides whether existing
  stores can be repaired in place), and the full set of affected datatypes — the eight above are
  the ones tried, and the rule is the engine's, not ours.
- **What would close it:** the fix is a decision, not a patch, so it is in `PROPOSED.md` — upstream
  work, a term encoding of our own, or accepting the loss and *disclosing* it. Disclosure is the
  cheapest and is not a fix; a governance team cannot sign off a vocabulary whose notations
  changed. Until one is chosen, no OpenBiz surface may claim that an export round-trips.
- **Opened:** iteration 10

### Our N-Triples is one constraint short of Canonical N-Triples
- **Kind:** partial-standard
- **What is proven:** [Canonical N-Triples §4] requires that `ECHAR` be used only for U+0022,
  U+005C, U+000A and U+000D — "ECHAR MUST NOT be used for characters that are allowed directly in
  `STRING_LITERAL_QUOTE`". A tab is allowed directly and our writer emits `\t`. Pinned by
  `our_n_triples_export_is_canonical_n_triples_but_for_one_known_violation`, which requires exactly
  this one violation and no other, so a second one appearing is a failure. The other four
  constraints hold: single-space separators, no comments, no `UCHAR` (accented Latin, CJK, and an
  emoji are all written raw), and the carriage return and line feed correctly escaped.
- **What is not:** nothing is *lost* — the document is valid N-Triples and any conforming reader
  recovers the same term, which is why iteration 8's round trip could not see it. What is not true
  is that two tools serialising one graph produce the same bytes, which is what makes a vocabulary
  diffable in git. Whether the other five syntaxes have an equivalent layout-level divergence is
  unknown; only N-Triples has a canonical form defined, so for the others the question does not
  even have a spec-shaped answer.
- **What would close it:** either upstream, or an N-Triples writer of our own — it is the simplest
  of the six and the one where writing it is genuinely cheap. In `PROPOSED.md`; not taken here
  because replacing a serialiser is a decision about the engine boundary, not a blind-spot fix.
- **Opened:** iteration 10

  [Canonical N-Triples §4]: https://www.w3.org/TR/n-triples/#canonical-ntriples

### The HTTP export buffers the whole graph in memory
- **Kind:** partial-coverage
- **What is proven:** `Store::export_graph` genuinely streams — quads go to the writer as they are
  read, and the backend's iterator holds one snapshot for the whole scan, so peak memory in the
  *store* is one quad and a concurrent commit cannot tear the file. It takes no write lock, proven
  by exporting from inside an open write transaction.
- **What is not:** the HTTP layer collects that stream into a `Vec<u8>` to build the response body,
  so a single request is bounded by memory rather than by graph size, and N concurrent exports of a
  large vocabulary are bounded by N times that. Nothing has exported more than nine quads. The
  serialisation itself has never been timed. The work runs on `spawn_blocking`, so it does not stall
  the async runtime — that much is by construction, and also untested.
- **What would close it:** the Phase 1 benchmark spike, which now owes a **fifth** number: export
  wall-clock and peak RSS at 10k / 100k / 1M concepts, per syntax. If the number is bad the fix is a
  streaming body (`Body::from_stream` over a channel fed by the blocking task), which is a change to
  this handler and to nothing above it.
- **Opened:** iteration 8
- **Amended iteration 9:** the SPARQL endpoint has the same shape and a larger worst case, so the
  spike now owes a **sixth** number — query wall-clock and peak RSS at the 100 000-answer cap. The
  two handlers should be fixed together or not at all; a streaming export beside a buffering query
  endpoint is the inconsistency a reviewer would have to re-derive.

### The interface's download path has never run against a real store with content
- **Kind:** partial-coverage
- **What is proven:** under jsdom, the chooser renders exactly the formats the server advertises in
  the order it advertises them, every link's `href` is the escaped export URL for the chosen format,
  changing the format rewrites every link, and the lossy-syntax warning appears and disappears with
  the choice. Three mutations of the component were confirmed to break it. Against the real binary,
  `GET /api/export` was exercised by hand for TriG, N-Quads via `Accept`, and a 404.
- **What is not:** the two halves have never met. A vocabulary cannot be created over HTTP (§1.7
  holds that until `DiscoveryProvider` exists) and the store's public API creates only *empty*
  vocabularies, so **no download link has ever been clicked against a graph with statements in it**.
  The `Content-Disposition` filename, the browser's save behaviour, and what a 404 looks like to a
  user who clicked a link for a vocabulary deleted in another tab are all unobserved.
- **What would close it:** either Phase 2's authoring path, which will put content in a vocabulary,
  or the browser-driven test recorded in "Nothing renders the UI in a browser". Whichever lands
  first should exercise this.
- **Opened:** iteration 8

### `X-OpenBiz-Graph` is our own header, and nothing reads it
- **Kind:** no-production-caller
- **What is proven:** the header is on every export response, carries the graph's IRI, and is
  percent-escaped to ASCII so a non-ASCII IRI cannot make it an invalid header value. The escaping
  is unit-tested including the escaping of `%` itself.
- **What is not:** no client parses it — not our own interface, which already knows which graph it
  asked for. It exists so that an export in a triple syntax still *states* which graph it is, which
  is a real gap, but the value of an answer nobody reads is an assumption. Nor is it a registered
  header name or a standard: a consumer would have to be told about it. `Link: <iri>; rel="canonical"`
  or an RDF-level provenance statement in the payload may be better answers.
- **What would close it:** the import path (the next plan item) using it to default the target graph
  when a Turtle file is re-uploaded — which is the use case that would prove the header earns its
  place, or show that it does not.
- **Opened:** iteration 8

### An export's registry check and its scan are two snapshots
- **Kind:** inspected-only
- **What is proven:** by reading Oxigraph's `Store::quads_for_pattern`, the *scan* takes one
  snapshot and holds it for the whole iteration, so an export is internally consistent.
- **What is not:** `contains_graph` runs first, on its own earlier snapshot. A graph deregistered in
  the gap would be exported anyway; a graph registered in the gap would 404 despite existing. Both
  are unreachable today because **nothing in this build deregisters a graph**, so the window is
  argued from the code rather than closed by it — the same shape as the kill-window argument in
  iteration 7 and equally untested.
- **What would close it:** a read transaction spanning the check and the scan, added when the first
  deletion path arrives. Doing it now would put a lock-shaped API in front of a race that cannot
  happen, and the test for it could not be written.
- **Opened:** iteration 8

### ~~The UI is built but nothing serves it~~ — CLOSED, iteration 1
- **Closed by:** `rust-embed` embedding `ui/dist`, a router fallback serving it, 13 tests in
  `crates/openbiz-server/src/ui.rs`, an end-to-end test over a real socket in
  `tests/serves_embedded_ui.rs`, and a `Single binary` CI job that deletes `ui/dist` before asking
  the release binary to serve the interface. Verified by hand: the release binary served the real
  318-byte Vite `index.html` and the 194 087-byte bundle with `ui/dist` moved off disk. See
  `adr/0004`.
- **Kind:** no-production-caller
- **What was proven:** `ui/` typechecks under TS strict and builds to `ui/dist` (28 modules, ~194 kB
  raw / ~61 kB gzipped). The Rust server compiles, serves `/healthz`, and returns 404 elsewhere.
- **What was not:** the two halves were not connected. No `rust-embed`, no static-file route, no
  test that a built asset was reachable from the binary.
- **Opened:** Phase 0 hand-build (pre-iteration-1) · **Closed:** iteration 1

### ~~The UI has no test suite at all~~ — CLOSED, iteration 4
- **What is now proven:** Vitest + Testing Library run 10 assertions over `App` in CI (`npm test` in
  the `UI` job). All three `Probe` states are exercised: the loading text and the single `/healthz`
  call on mount; the success line naming status and version; the `role="alert"` branch for an HTTP
  refusal, a transport rejection, and a non-`Error` rejection; the unmount abort; the `AbortError`
  swallow; and a StrictMode double-mount that must not paint a spurious alert. Each was shown to
  fail against a deliberately broken `App.tsx` before being trusted — see the plan item for the
  seven mutants.
- **Correction to the original entry:** it said the driver's `npm test` "silently passes". It did
  not — there was no `test` script, so it exited 1 with `Missing script: "test"`, and the `UI` CI
  job never called it. Zero UI assertions had ever run either way.
- **Opened:** iteration 1 · **Closed:** iteration 4

### Write throughput under a serialised writer is unmeasured
- **Kind:** partial-coverage
- **What is proven:** writes are correct under concurrency — eight threads racing on one IRI leave
  exactly one registration, eight threads on distinct IRIs all land, and readers are never blocked
  by an open transaction. Correctness under contention is tested; **cost** under contention is not.
- **What is not:** `adr/0009` trades write parallelism for serialisability by taking a lock we own,
  and no number anywhere says what that costs. Nothing measures how long a transaction holds the
  lock, how deep the queue gets, or where the knee is. Upstream also states a transaction holds its
  entire change set in memory, so a naive "one transaction per import file" in Phase 11 is both a
  memory risk and a long lock hold, and nothing currently stops someone writing it.
- **What would close it:** the Phase 1 Oxigraph benchmark spike, which now owes a fourth number —
  concurrent write throughput and lock wait time at the 10k/100k/1M sizes it already covers. Phase
  13 then addresses whatever it finds.
- **Opened:** iteration 7

### Rollback is proven against errors and panics, but not against process death
- **Kind:** partial-coverage
- **What is proven:** a transaction that returns `Err` writes nothing, a transaction that panics
  writes nothing and does not leave the store read-only, and a rolled-back transaction leaves the
  store byte-identical (asserted on quad count, not just on absence). Six mutants confirmed each
  assertion is load-bearing.
- **What is not:** nothing kills a process mid-transaction and reopens the store. That is the
  case the backend's crash recovery handles rather than our code, so it is testing Oxigraph rather
  than testing us — but it is also the case an operator actually hits, and "the backend handles it"
  is an assumption we have not verified. Relatedly, `Store::open` now commits the format stamp and
  the system-graph registration in one transaction *specifically* to close a kill-in-the-gap
  window, and the proof that the window is closed is the code's shape, not a test.
- **What would close it:** a harness that spawns the real binary, `SIGKILL`s it during a write, and
  reopens the store asserting it is either fully changed or fully unchanged. `tests/graceful_shutdown.rs`
  already spawns real binaries, so the machinery exists; what is missing is a way to make the
  binary write on demand, which needs an authoring endpoint that §1.7 says cannot exist until
  `DiscoveryProvider` does.
- **Opened:** iteration 7

### The nested-transaction guard fails by hanging if it regresses
- **Kind:** inspected-only
- **What is proven:** `a_nested_transaction_is_refused_rather_than_deadlocking` passes, and a
  mutant that stops keying the reentrancy mark by store address kills it.
- **What is not:** if the guard were removed entirely, that test would **hang** rather than fail,
  because the bug it guards against is itself a deadlock. A hanging test in CI reads as a flaky
  runner or a timeout, not as a regression, so the signal is real but badly shaped.
- **What would close it:** run the nested call on a spawned thread and join it with a timeout, so
  the absence of the guard reports as a failed assertion rather than as a stuck job. Not done here
  because `std` has no `join_timeout`; it needs a channel-with-timeout dance that is more test
  machinery than the one assertion justifies today.
- **Amended, iteration 8:** a **second** test now has the same shape.
  `exporting_does_not_block_on_the_write_lock` exports from inside an open write transaction; if
  `export_graph` ever took the write lock, that test would hang rather than fail. The property is
  worth asserting — an export must never be able to block an author — but two hanging tests is now
  a pattern rather than a one-off, and the timeout helper this entry describes would pay for itself
  across both.
- **Opened:** iteration 7 · **Amended:** iteration 8

### The UI suite asserts on jsdom, and covers one component
- **Kind:** narrowly-proven
- **What is proven:** `App`'s render output and probe lifecycle, under jsdom, via accessible
  queries (`getByRole`) rather than test IDs — so the assertions are about what a user or a screen
  reader perceives, not about markup shape.
- **What is not:** three things. (1) **jsdom is not a browser** — no layout, no real paint, no CSS,
  no focus semantics beyond jsdom's approximation. The Phase 3 Playwright item is what closes that;
  see "Nothing renders the UI in a browser". (2) **`main.tsx` is untested** — the "root element
  missing" throw and the `createRoot` call have no assertion; only `App` is mounted, by the test's
  own `render`. (3) **`CLAUDE.md` §4.4's keyboard-navigability clause is still unenforced.** Nothing
  in `App` is interactive yet, so there is nothing to tab to and no test would be meaningful — but
  the moment Phase 3 adds a control, a keyboard test has to arrive with it, and no mechanism
  currently forces that.
- **Amended, iteration 6:** it now covers *two* components — `App` and `Vocabularies` — plus the
  `useProbe` hook they share, at 22 assertions. Every point above still stands unchanged, and (3)
  has now survived a second component without being tested: `Vocabularies` renders a heading, a
  list, and paragraphs, so there is still nothing to tab to and `CLAUDE.md` §4.4 is still satisfied
  **vacuously**. Two iterations of UI have now been added without the clause ever being exercised.
- **Amended, iteration 8:** the interface has its **first interactive control** — the export format
  chooser — so (3) is no longer vacuous. It is now *narrowly* satisfied: a test asserts the control
  is a native `<select>` with an associated `<label>` rather than a `div` with a click handler, that
  the download is a real `<a href>`, and that both accept focus. That is the thing which *makes* a
  tab order, but it is not the tab order: jsdom has no real focus semantics, so nothing proves the
  sequence a keyboard user actually walks, that the chooser is reachable before the links it
  governs, or that the `role="alert"` states are announced. Point (3) has therefore changed from
  "untested because untestable" to "tested at the only level jsdom permits", which is progress and
  is not closure. Coverage is now 29 assertions across the same two components.
- **What would close it:** for (1) the Playwright item; for (2) a test that mounts `main.tsx`
  against a document with and without `#root`; for (3) the same Playwright item, which is the only
  place a real tab order can be walked — plus the Phase 3 convention, ideally a lint or a shared
  test helper, that makes an interactive component without a keyboard test fail.
- **Opened:** iteration 4 · **Amended:** iterations 6, 8

### Nothing renders the UI in a browser
- **Kind:** environment-limited
- **What is proven:** the served bytes are correct — status, content type, `ETag`/304, cache
  headers, and the exact Vite `index.html` and bundle, asserted over a real socket.
- **What is not:** no browser has ever executed the bundle. React could throw on mount and every
  test here would still pass, because they assert on transport, not on rendering. Content-Security-
  Policy, `crossorigin` on the module script, and MIME strictness are unexercised for the same
  reason.
- **What would close it:** a headless-browser smoke test (Playwright) that loads `/` from the real
  binary and asserts the health line appears. This machine can run one; it is a deliberate scope
  call not to add a browser toolchain in a Phase 0 iteration.
- **Opened:** iteration 1

### Asset serving is unproven above trivial size and count
- **Kind:** partial-coverage
- **What is proven:** one 194 kB bundle and one 318-byte shell, served whole, in a two-file
  `ui/dist`.
- **What is not:** no compression (`Content-Encoding`) is negotiated or emitted, so the 194 kB
  bundle goes over the wire uncompressed — an incumbent-grade UI will be several times that.
  Range requests are not handled. `Assets::get` is a lookup over a generated match arm per file; its
  behaviour at the hundreds-of-files scale a real design system produces is unmeasured, as is the
  effect on binary size and compile time.
- **What would close it:** revisit when Phase 3's design system makes `ui/dist` realistic —
  measure binary size, cold start, and first-paint transfer size, and decide on compression then.
- **Opened:** iteration 1

### ~~Config is env-only; the file path is unimplemented~~ — HALF CLOSED, iteration 2
- **Closed by:** `crates/openbiz-server/src/config.rs` — defaults → TOML file → environment, with
  documented precedence, `deny_unknown_fields`, blank-value rejection, and a `Source` on every
  setting. 16 tests; `Config::load` is called by `main.rs`, which logs each setting's provenance
  and names it in the bind-failure message. Four failure paths verified by hand against the real
  binary. See `adr/0005` and `docs/CONFIGURATION.md`.
- **Still open — see the entry below:** the `data_dir` half. Nothing creates or opens that
  directory yet.
- **Kind:** partial-coverage
- **Opened:** Phase 0 hand-build (pre-iteration-1) · **Config-file half closed:** iteration 2

### ~~`data_dir` is configured, logged, and used by nothing~~ — CLOSED, iteration 3
- **Closed by:** `Store::open(config.data_dir.value())` in `main.rs`, which runs *before* the
  listener binds. A `data_dir` that is a file, is unwritable, or is already locked by another
  OpenBiz now fails startup with a message naming both the path and the configuration layer that
  chose it. Covered by `a_data_directory_that_is_a_file_is_reported_as_such`,
  `an_unwritable_data_directory_is_reported_before_the_backend_sees_it`, and — across real
  processes — `a_second_instance_refuses_to_share_the_data_directory`. See `adr/0006`.
- **Kind:** no-production-caller
- **What is proven:** `data_dir` is read from the defaults, a file, or `OPENBIZ_DATA_DIR`, is
  rejected when blank, carries its provenance, and is logged at startup.
- **What is not:** **no code creates, opens, or writes that directory.** A user who sets it to an
  unwritable path, a path that does not exist, or a file rather than a directory gets a clean
  startup and no warning — the setting is inert until Phase 1 wires the store. This is the honest
  reading of `CLAUDE.md` §4 clause 1: a value with no consumer is not a feature.
- **What would close it:** the first Phase 1 item (`openbiz-store` Oxigraph lifecycle — open, close,
  durable path, graceful shutdown), which is the next item in the plan.
- **Opened:** iteration 2 (carved out of the entry above)

### Configuration is validated for shape, not for meaning
- **Kind:** partial-coverage
- **What is proven:** unknown keys, malformed TOML, wrongly-typed values, blank values, and a
  missing explicitly-named file all fail with a message naming the source. Precedence across all
  three layers is tested per setting, including that one key in a file does not change the
  provenance of another.
- **What is not:** `bind` is only checked for being non-blank. `OPENBIZ_BIND=not-an-address` is
  accepted by the loader and fails later at `TcpListener::bind` — a clear error, but later than it
  needs to be, and it is the only setting where the bind attempt happens to be a validator. No
  setting has a range, enum, or path check, and there is no mechanism for one. A non-UTF-8
  `OPENBIZ_CONFIG` path is treated as unset by `std::env::var().ok()` rather than reported, which is
  a silent ignore of exactly the kind the rest of this module refuses.
- **What would close it:** a per-setting validation hook run during `Config::resolve`, so the error
  carries the `Source` like the blank-value one does. Worth doing when the settings count justifies
  it — with two settings a hook is more machinery than the problem.
- **Opened:** iteration 2

### ~~Configuration precedence is untested against the real process environment~~ — CLOSED, iteration 3
- **Closed by:** `crates/openbiz-server/tests/graceful_shutdown.rs`'s
  `the_process_environment_reaches_the_configuration_with_its_provenance`, which spawns the real
  binary with a controlled environment and asserts on its startup log — that `data_dir` and `bind`
  both report `$OPENBIZ_*` as their source, and that requesting port 0 logs the port actually
  allocated rather than the request. `Config::load` now has a regression test; a typo in a variable
  name inside it fails CI. This was also the promoted plan item of the same name.
- **Kind:** partial-coverage
- **What is proven:** `Config::resolve` is tested exhaustively with an injected environment lookup,
  and the four headline failure paths were run by hand against the real binary with real env vars
  and real files.
- **What is not:** `Config::load` — the ten-line function that supplies `std::env::var` and the
  default path — has no automated test. It cannot easily have one: `std::env::set_var` mutates
  state shared by every thread in the test binary, and Rust 2024 marks it `unsafe` for that reason.
  So the wiring between the tested core and the real environment is inspected-only, and a typo in a
  variable name inside `load` would pass CI.
- **What would close it:** an integration test that spawns the binary as a subprocess with a
  controlled environment and asserts on its startup log — the same shape as
  `tests/serves_embedded_ui.rs`. Cheap; deferred only to keep this iteration to one item.
- **Opened:** iteration 2

### ~~The named-graph model has no production caller~~ — CLOSED, iteration 5
- **Closed by:** `insert_into` in `crates/openbiz-store/src/lib.rs` — the single function through
  which every write to the store passes, including the format stamp. It requires a `&GraphId` and
  refuses a target that is not directly writable, so `is_directly_writable()` is a rule the store
  enforces rather than a comment a caller may forget, and `StoreError::NotWritable` is now returned
  by a public method (`create_vocabulary_graph`). `Store::open` registers the system graph in the
  graph registry on every open, and `main.rs` reads that registry **before it binds**, failing
  startup if it cannot be described. `GraphId`'s fields are private, so the pairing of IRI and kind
  is an invariant the type enforces: the mutation that assembled one by struct literal from a
  registry row would not compile. 36 store tests (was 12); twelve mutants killed. See `adr/0007`.
- **Kind:** no-production-caller
- **Opened:** iteration 3 · **Closed:** iteration 5

### `create_vocabulary_graph` has no production caller
- **Kind:** no-production-caller
- **What is proven:** it registers a graph, refuses an IRI that is already registered with a message
  pointing at the reuse ladder, refuses a graph that is not directly writable, leaves the registry
  unchanged when it refuses, and survives close and reopen. Nine tests, and the mutants that drop
  either guard are killed.
- **What is not:** **nothing calls it outside tests.** No HTTP route, no UI, no import path creates a
  vocabulary graph. This is deliberate rather than an oversight, and the distinction matters:
  `CLAUDE.md` §1.7 requires discovery to run *before* creation and requires a recorded justification
  when something new is created anyway, and `DiscoveryProvider` does not exist until Phase 2. Adding
  a create endpoint now would be a charter violation dressed up as progress, so the honest position
  is a recorded gap.
- **What would close it:** the Phase 2 authoring path with its local discovery hook. The read half —
  `GET /api/graphs` and the registry in the UI — is the next Phase 1 item and does not depend on it.
- **Amended, iteration 7:** still open, and the entry understated the cost of leaving it open.
  Having no production caller is *why* nobody had hit the race in it: the check and the write were
  two separate operations, and eight threads creating one IRI all succeeded, leaving a registry
  that `Store::graphs` then refuses wholesale as `Corrupt`. That is now fixed — the check and the
  write are one transaction (`adr/0009`) — but the lesson is about this ledger rather than about
  the bug. **A no-production-caller entry is not a dormant risk; it is an untested one**, and the
  concurrency defect sat in a method with nine passing tests. The `transaction` API this method now
  delegates to *does* have a production caller (store startup), so the seam is exercised on every
  start even though this method is not.
- **Opened:** iteration 5 · **Amended:** iteration 7

### A corrupt registry is proven to stop the store, not proven to stop the server
- **Kind:** partial-coverage
- **What is proven:** `Store::graphs()` returns `StoreError::Corrupt` for an unrecognised kind
  token, for a registry entry that breaks the namespace rule, and for a graph registered twice with
  different kinds — each asserted against a store doctored through the backend, and each mutant
  killed. `main.rs` propagates that error with `?` before the listener binds.
- **What is not:** the *propagation* is inspected-only. `tests/graceful_shutdown.rs` proves the
  registry is read at startup (the mutants that stop reading it and that read-but-do-not-report it
  both turn it red), but no test starts the binary against a **doctored** store and asserts it
  refuses to serve. Building that fixture needs raw quad access from outside the crate, which today
  means either making the backend reachable or adding `oxigraph` as a dev-dependency of
  `openbiz-server` — and a test-only direct dependency on the engine cuts against `CLAUDE.md` §3,
  so it is not a cost worth paying for one assertion.
- **Also, since iteration 6:** `GET /api/graphs` has the same shape of gap. `From<StoreError> for
  Failure` is asserted directly — a constructed `StoreError::Corrupt` becomes a 500 whose body
  contains neither the customer's IRIs nor their store path, and both mutants (200 instead of 500,
  and echoing the store's own words) are killed. What is *not* proven is that a real store can
  actually drive the handler down that branch, for the same fixture reason. In practice it is close
  to unreachable: `main` refuses to start against a registry it cannot read, so the endpoint only
  sees one that went bad while the process was running.
- **What would close it:** a store-layer test helper that writes an arbitrary quad, gated behind a
  cargo feature the server's tests can enable. Now wanted by two callers rather than one, which
  changes the cost/benefit recorded above — it is worth doing the next time either gap is touched.
- **Opened:** iteration 5 · **Amended:** iteration 6

### Registry reads are unmeasured above a handful of graphs
- **Kind:** partial-coverage
- **What is proven:** `Store::graphs()` returns the right graphs in a stable order with up to four
  registered, and `contains_graph` answers without scanning the store's contents. Listing asks the
  registry rather than the backend precisely so it does not become a whole-store scan.
- **What is not:** the registry read is a pattern scan bounded by the number of *graphs*, which is
  fine at four and unmeasured at four thousand — and an enterprise with a vocabulary per business
  domain per jurisdiction gets there. It also runs on the startup path, so if it is slow it is slow
  in the cold-start budget `CLAUDE.md` §1.5 sets. Nothing has measured it, and `graphs.sort()` is an
  additional O(n log n) on top.
- **Widened, iteration 6:** `GET /api/graphs` reads the registry **on every request**, deliberately
  and with the reasoning recorded in `adr/0008` §6 — a cache would need invalidating by every future
  creation, import, and restore path, and a stale "your vocabulary does not exist" is worse than an
  unmeasured scan. But the consequence is that an unmeasured scan is now on a *hot* path rather than
  a once-per-process one, and it also serialises the whole registry into a JSON body with no paging.
  The 4 000-graph deployment gets a 4 000-element response on every page load.
- **Widened again, iteration 8:** the interface now reads the registry *and* `/api/export/formats`
  on mount, so a page load is two requests rather than one — the second is a constant-size response
  built from a six-element array and is not a scan, so it does not widen the measurement problem,
  but it does mean the vocabulary list's time-to-first-paint now waits on two round trips.
- **What would close it:** the Phase 1 benchmark spike should register 10k graphs and time
  `graphs()`, startup, **and the endpoint**, alongside the query evaluation and `close()` numbers it
  already owes. If the number is bad, the answer is paging or a `?kind=` filter — both API changes,
  which is why the spike should land before Phase 3 builds an interface on top of this shape.
- **Opened:** iteration 5 · **Widened:** iterations 6, 8

### Durability is proven for one quad, not for a vocabulary
- **Kind:** partial-coverage
- **What is proven:** what one open commits, the next open reads back — `the_format_stamp_survives_
  close_and_reopen` closes the store and reopens it, and the cross-process test restarts a real
  binary against the same directory. The stamp is flushed immediately on first open.
- **What is not:** the store has never held more than a single quad. Nothing has written a
  vocabulary, so nothing has measured write throughput, flush cost at size, or how long `close()`
  takes when there is real data to flush — which is the number that decides whether a `docker stop`
  grace period of 10 s is enough or whether we are hard-killed anyway. The two upstream Oxigraph
  risks (unoptimised query evaluation, literal precision) are **untouched** by this iteration and
  keep their Phase 1 spike items; `adr/0006` must not be read as clearing them.
- **What would close it:** the Phase 1 benchmark spike, which should measure `close()` at 10k /
  100k / 1M concepts and not only query evaluation.
- **Amended, iteration 7:** the store has now held up to nine quads rather than three, which does
  not move this entry at all — the point stands unchanged. The spike now owes a *fourth* number as
  well; see "Write throughput under a serialised writer is unmeasured".
- **Opened:** iteration 3 · **Amended:** iteration 7

### Graceful shutdown is proven to exit cleanly, not to drain
- **Kind:** partial-coverage
- **What is proven:** `SIGTERM` to a real process exits zero, logs which signal stopped it, logs
  `store closed cleanly`, and releases the lock so the next process can open the same directory.
  `shutdown_signal()` is proven not to resolve on its own — which matters, because `axum::serve`
  returns `Ok(())` on graceful shutdown, so a signal future that resolved immediately would produce
  a binary that exits zero while serving nothing.
- **What is not:** **no test has an in-flight request when the signal arrives.** The ordering the
  module documents — stop accepting, drain, *then* flush — is asserted only in its doc comment. A
  regression that closed the store before the last response was written would pass every test here.
  `SIGKILL` recovery was checked **by hand only**: a hard kill exits 137 and writes no
  `store closed cleanly` line (so the assertion discriminates rather than always passing), and the
  next start reopened the store and read its stamp back. That was a near-empty store with no write
  in flight — the case that actually matters, a hard kill *during* a write, is unmeasured, and none
  of it is a regression test.
- **What would close it:** a test that opens a slow request, sends `SIGTERM` mid-flight, and asserts
  the response completes before the process exits; and a `SIGKILL` test asserting the store reopens.
- **Opened:** iteration 3

### The lock classification depends on a RocksDB message string
- **Kind:** partial-coverage
- **What is proven:** both wordings RocksDB actually emits are pinned by tests — the same-process
  one in `openbiz-store`, the cross-process one in `tests/graceful_shutdown.rs`. Discovering they
  differ cost this iteration a red test, and a unit test alone would have shipped a classifier that
  never fired in production.
- **What is not:** `classify_open` matches on the substring `LOCK:`, and RocksDB's wording is not an
  API. A version bump could change it. The mitigation is that the tests go red rather than the
  classification degrading silently — but that is a promise about *noticing*, not about working, and
  a user hitting the fallback branch gets a true but much less useful error.
- **What would close it:** nothing available today; the backend exposes no typed lock error. Revisit
  if Oxigraph gains one. Recorded so a future upgrade knows to look here first.
- **Opened:** iteration 3

### Shutdown is Unix-only in test, and by implication in practice
- **Kind:** environment-limited
- **What is proven:** on Unix, `SIGINT` and `SIGTERM` both stop the server gracefully.
- **What is not:** `tests/graceful_shutdown.rs` is `#![cfg(unix)]` — `SIGTERM` does not exist
  elsewhere, and on Windows the `Ctrl-C` branch is the entire contract, unexercised by any test.
  Nothing has ever run this binary on Windows. We do not currently claim Windows support; this entry
  exists so that claim is not made accidentally.
- **What would close it:** a Windows CI runner exercising `Ctrl-C` shutdown, if and when Windows
  becomes a supported target.
- **Opened:** iteration 3

### The store's build toolchain is unavailable on the loop machine without a workaround
- **Kind:** environment-limited
- **What is proven:** the workspace builds and its tests pass once `libclang` is present. CI
  installs `clang` and `libclang-dev` explicitly rather than relying on the runner image.
- **What is not:** this machine has no `clang`, no `libclang`, and no passwordless `sudo`, so
  `cargo test --workspace` fails at `bindgen` on a clean checkout. The loop works around it by
  extracting `libclang1-20` and `libclang-common-20-dev` from downloaded `.deb`s into
  `~/.local/libclang`. **That workaround is not in the repo and does not survive a machine reset** —
  a future iteration that starts with an unexplained `bindgen` panic should read this entry rather
  than conclude the store is broken.
- **The full incantation, because `LIBCLANG_PATH` alone is not enough** (iteration 4 lost time to
  this): with only `LIBCLANG_PATH` set, libclang loads but fails to locate its own builtin headers
  and `bindgen` dies on `rocksdb/c.h:65:10: fatal error: 'stdbool.h' file not found` — a *different*
  message from the `Unable to find libclang` this entry originally described, and easy to misread as
  a real build break. Both variables are needed:
  ```
  export LIBCLANG_PATH="$HOME/.local/libclang/usr/lib/llvm-20/lib"
  export BINDGEN_EXTRA_CLANG_ARGS="-resource-dir=$HOME/.local/libclang/usr/lib/llvm-20/lib/clang/20"
  ```
- **What would close it:** a human running `sudo apt install clang libclang-dev` on the loop
  machine. Out of loop scope (`CLAUDE.md` §8 — needs root).
- **Opened:** iteration 3 · **Amended:** iteration 4

### The UI dependency tree is outside the §5 licence gate
- **Kind:** unenforced-policy
- **What is proven:** `cargo deny check licenses` enforces `CLAUDE.md` §5 over the whole Rust tree
  in CI, and it is green. The four packages added this iteration were checked by hand before being
  committed: `vitest` (MIT), `jsdom` (MIT), `@testing-library/react` (MIT), `@testing-library/dom`
  (MIT). A sweep of all 153 installed npm packages found 134 MIT, 8 ISC, 5 Apache-2.0, 2 BSD-2,
  2 BSD-3, 1 MIT-0 — all permitted — and exactly one unlisted licence.
- **What is not:** **nothing enforces this.** §5 says "every new dependency gets a licence check",
  and for `ui/` that check is a human remembering to run one. The sweep above was ad hoc and is not
  in the repo, so the next npm install is unchecked by default. The one unlisted licence is
  `caniuse-lite` under **CC-BY-4.0** — pre-existing, pulled in transitively by Vite via
  `browserslist`, build-time only, and a *data* package rather than code, so none of it reaches the
  shipped binary. CC-BY-4.0 is attribution-only and not copyleft, so this is the "unlisted but
  permissive in substance" case of §5 rather than the forbidden one — but §5 requires an ADR for
  that judgement and there is no ADR, because there is no allow list for it to widen.
- **What would close it:** the `Audit the UI dependency tree for licences` proposal in
  `PROPOSED.md`. Deliberately not done here: adding an unrequested CI gate is scope creep, and the
  CC-BY-4.0 call deserves its own ADR rather than a footnote in a test-runner commit.
- **Opened:** iteration 4

### The JSON API has no authentication, and the error path is shaped around that
- **Kind:** partial-coverage
- **What is proven:** `GET /api/graphs` returns 200 with the registry, `POST` returns 405, an
  unmatched `/api/…` returns 404, and a store failure returns a 500 whose body carries neither the
  customer's graph IRIs nor their store path — asserted directly, and both mutants killed.
- **What is not:** **nothing authenticates or authorises this endpoint.** Anyone who can reach the
  port can enumerate every vocabulary IRI in the deployment. That is the expected state for Phase 1
  (auth is Phase 7) and it is not a bug, but it is a fact the loop should not lose track of, because
  a second decision has already been taken *because* of it: `adr/0008` §3 deliberately withholds the
  store's own error text from the response, which costs real diagnostic value and cuts against
  `CLAUDE.md` §3's explainability commitment. When Phase 7 lands, that trade should be revisited
  rather than inherited — an authenticated administrator should get the detail.
- **What would close it:** Phase 7's authorisation, plus a test that an authenticated administrative
  caller receives the full store error and an unauthenticated one does not.
- **Opened:** iteration 6

### `main`'s refusal to close a still-shared store is inspected-only
- **Kind:** inspected-only
- **What is proven:** the happy path, and it is proven where it can actually fail.
  `the_graph_registry_is_served_over_http_and_the_store_still_closes_cleanly` serves a real HTTP
  request from the real binary — which is what makes a connection clone the shared state — and then
  signals it, asserting exit zero and `store closed cleanly`. Before this iteration every shutdown
  test signalled a process that had never served a request, so the reclaim was trivially safe.
- **What is not:** the *failure* branch. If `Arc::into_inner` returns `None`, `main` bails with an
  error saying the store could not be closed cleanly, and nothing exercises that: producing it needs
  a deliberately leaked handle, which we have no way to inject without adding a seam whose only user
  is the test. So the message an operator would see in the one situation where their writes might
  not have reached disk has never been printed by a running process.
- **What would close it:** honestly, little worth its cost today. Revisit if a future feature holds
  a store handle outside the router — a background materialisation pass, a scheduled backup — since
  that is when the branch stops being theoretical.
- **Opened:** iteration 6

### `useProbe` re-fetches on a URL change that no caller makes
- **Kind:** no-production-caller
- **What is proven:** the hook fetches once on mount, checks the status before parsing the body,
  aborts on unmount, and stays silent on an abort — 22 assertions across two components, and the
  four mutants against those behaviours are killed.
- **What is not:** its effect depends on `[url]`, so changing the `url` prop would cancel the first
  request and issue a second. **Nothing changes it** — both call sites pass a string literal — so
  that branch has no production caller and no test. It is one line and removing it would be worse
  (a stale response for the old URL is a nastier bug than an unused code path), but it should not be
  counted as proven behaviour.
- **What would close it:** the first component that reads a parameterised URL — Phase 2's
  per-vocabulary views are the likely first — should arrive with a test that changing the URL
  abandons the first response.
- **Opened:** iteration 6

### The SKOS core model is not measured at scale
- **Kind:** partial-coverage
- **What is proven:** `CoreModel` classifies the four core classes, applies nine axioms, and walks
  `skos:memberList` correctly on graphs of a few dozen statements — 31 unit tests keyed to the
  SKOS Reference's own examples, plus an end-to-end run through the binary. Three mutations turned
  the suite red before it was trusted (the registry check dropped, S29 entailment disabled, the
  disjointness checks disabled).
- **What is not:** it holds one `Resource` per subject **in memory**, and nothing has run it over
  the 100k- or 1M-concept stores `adr/0013` built. The statements stream, so the peak is the model
  rather than the graph, but the model over a million concepts is an unmeasured number and
  `openbiz inspect` on such a store is an unmeasured wall-clock. `adr/0013` found one 21-second
  cliff by measuring rather than reasoning, which is the reason not to assume this one is fine.
  **Amended at iteration 21, and the number got worse.** The model used to count labels and drop
  them; it now *keeps* them, because S13 and S14 are per-resource conditions and no statement
  order tells you a resource is finished. Labels are most of a thesaurus's statements, so peak
  memory has gone from "proportional to the structure" to "proportional to the size" — the exact
  claim `adr/0019` made and `adr/0020` withdraws. What is held is minimal (one entry per distinct
  label per resource, shared across the three properties) and the *report* is still bounded by
  the language count rather than the label count, but the model is not, and the measurement is
  now more overdue than it was.
  **Amended again at iteration 22, and it got worse a second time.** A vocabulary authored in
  SKOS-XL holds each label roughly twice — once as a label resource with its literal form, once as
  the dumbed-down plain label on the concept — plus one entry per (resource, label) link. For an
  ISO 25964 thesaurus, which is exactly the customer that authors in SKOS-XL, that is the common
  case rather than the corner. Still unmeasured.
- **What would close it:** run `inspect` against the generated stores from `adr/0013`'s harness and
  record the memory and the time, as that ADR did for the tree and the label search. The harness
  generates plain-SKOS stores, so closing this now also means generating a SKOS-XL one.
- **Opened:** iteration 20

### A derivation chain is a list, not a tree
- **Kind:** partial-coverage
- **What is proven:** every derived fact carries the statement it followed from and the
  specification statement that licensed it, and the list is emitted in the order the rules ran, so
  an S8 → S5 chain reads downwards. A test asserts every derivation names both.
- **What is not:** a premise that was *itself* derived is printed as a statement, not as a link to
  the derivation that produced it. Reading "because `<S>` `skos:hasTopConcept` `<C>`" requires the
  reader to scan upwards for where that came from. On the fixture there are eight derivations and
  that is easy; on a real vocabulary with thousands it is not a explanation a person can follow.
- **What would close it:** give `Derivation` a reference to its premise's derivation where one
  exists, and render the chain indented. Cheap now, and it becomes the shape the interface's "why?"
  panel needs — which is the point at which it stops being cosmetic.
- **Opened:** iteration 20

### `inspect` has no machine-readable output and no exit status for "inconsistent"
- **Kind:** partial-coverage
- **What is proven:** the report is readable by a person, exits 0 on a successful read, and
  non-zero when the vocabulary does not exist.
- **What is not:** a graph that violates S9 or S37 still exits **0** — the inconsistency is in the
  text, not the status — so `openbiz inspect … && deploy` does not gate on it. There is also no
  JSON form, so nothing but a human can consume the answer. Neither is a defect of the model; both
  are decisions deferred rather than made, and they are recorded so the deferral is visible.
- **What would close it:** Phase 4's SHACL validation is where a *gate* belongs, with an exit
  status and a machine-readable report. If `inspect` grows one first it should be the same shape,
  not a second one.
- **Opened:** iteration 20

### A literal in subject position is mapped rather than refused
- **Kind:** inspected-only
- **What is proven:** `openbiz-server`'s `node()` maps the store's three term kinds onto the
  domain's two. Oxigraph's subject type cannot hold a literal, so the third arm is unreachable
  through `Store::for_each_statement`.
- **What is not:** that unreachability is a property of *this* engine's type, verified by reading
  it, not by a test — nothing could construct the input to write one. The arm maps a literal to a
  blank node labelled with its lexical form, which is visibly wrong rather than silently wrong, but
  if a future source of `StatementRef` (a parser, a discovery connector) is less careful than
  Oxigraph, this is where a malformed statement would enter the model looking well-formed.
- **What would close it:** make `StatementRef::subject` a type that cannot hold a literal, as
  Oxigraph's does. Worth doing when a second producer of `StatementRef` exists; premature now.
- **Opened:** iteration 20

### `Resource::preferred_label_in` does no BCP 47 language fallback
- **Kind:** partial-standard
- **What is proven:** a preferred label is found by exact language tag, lower-cased, and Example 18
  is asserted: a request for `en` is not answered with `en-GB`, because §5.6.5 makes them different
  tags. `Resource::display_label` falls back from preferred to alternative and is deterministic
  whichever order the statements arrive in.
- **What is not:** §5.6.5 *suggests* an application implement BCP 47's "lookup" algorithm so that a
  request for `en-GB` can be answered by `en`, and we do not. A vocabulary labelled only in `en`
  therefore looks entirely unlabelled to a caller asking for `en-GB`. `display_label` picks the
  first label in language-tag order, which is deterministic but arbitrary across languages —
  every caller in this build prints the tag beside it so the arbitrariness is visible, which is a
  mitigation and not a fix.
- **What would close it:** a configured display-language preference order, with lookup fallback
  applied against it, and a test per §5.6.5. It needs a place to configure it, so it belongs with
  the interface rather than ahead of it.
- **Opened:** iteration 21

### S11 is not entailed — `rdfs:label` is not derived from a SKOS label
- **Kind:** partial-standard
- **What is proven:** S12, S13 and S14 are implemented and tested against the specification's own
  Examples 10–19. S10 and S11 are quoted in `labels.rs` and deliberately not applied, with the
  reason recorded in `adr/0020`.
- **What is not:** S11 says the three properties are sub-properties of `rdfs:label`, so every SKOS
  label entails an `rdfs:label`. We do not derive it. Nothing in this build reads `rdfs:label`, and
  entailing it would add one derivation per label to a report that prints every derivation — the
  cost is a report the size of the vocabulary for a fact no caller consumes. **A tool that
  round-trips our export and expects `rdfs:label` will not find one**, which is a real interop
  gap even though it is a sound omission.
- **What would close it:** derive it when something reads it — most likely the interface's display
  layer or a DCAT export — and suppress it from the derivation listing, which is a change to how
  derivations are reported and not just to the rule set.
- **Opened:** iteration 21

### An S12-refused label removes the resource from the model when it is the only thing said about it
- **Kind:** partial-coverage
- **What is proven:** a label that is not an RDF plain literal raises S12's finding, is discarded,
  and takes no part in S13 or S14 — tested both ways, including that the same typed literal under
  two properties is two findings and no clash. A resource that is *also* typed keeps everything
  else it has, which is asserted.
- **What is not:** a resource whose only SKOS statements are refused labels does not appear in
  `CoreModel::resources()` at all. The findings name it, so nothing is silently lost, but a caller
  iterating `resources()` to build a concept list will not see it. That is the documented meaning
  of `resources()` and it is asserted — but it is a sharper edge than it looks, and the first
  caller that treats `resources()` as "everything the graph mentions" will be wrong.
- **What would close it:** decide whether `resources()` means "mentioned" or "understood" once,
  when a second caller exists. Today `openbiz inspect` is the only one and it reads findings too.
- **Opened:** iteration 21

### ~~SKOS-XL is implemented as far as the labels, and `skosxl:labelRelation` is not read at all~~
- **Kind:** partial-standard
- **What is proven:** Appendix B.2, B.3 **and now B.4** — S48–S62. Fifteen of the appendix's own
  numbered examples (75–89) are asserted to be what the specification marks them, the S55–S57
  chains are entailed and feed S13 and S14, S59–S62 are applied, and ten mutations across two
  iterations turned the suite red before it was trusted. `openbiz inspect` reports both against a
  real store.
- **~~What is not~~:** ~~B.4 — `skosxl:labelRelation`, S59–S62 — is not read.~~ **Closed at
  iteration 23** (`adr/0022`). All four statements are applied: a literal under S59, both ends
  entailed as `skosxl:Label` under S60 and S61, and the symmetric closure under S62 with the
  converse carrying its origin. What is *left* of the gap is narrower and is its own entry below,
  because it is about `rdfs:subPropertyOf` and not about B.4.
  **Still not applied**, and this half is not closed: S47 (`skosxl:Label` is an `owl:Class`) and
  S52's *sub-class* reading — the cardinality restriction is checked as a count, not modelled as
  a class expression, so nothing would notice a graph that restated the restriction incorrectly.
- **Opened:** iteration 22. **Partly closed:** iteration 23 — B.4 is in; S47 and S52's sub-class
  reading remain.

### A refinement of `skosxl:labelRelation` reaches nothing, because we read no `rdfs:subPropertyOf`
- **Kind:** partial-standard
- **What is proven:** the *unsafe* inference is not made, and a test asserts it. Appendix B.4.4.1
  warns that "a sub-property of a symmetric property is not necessarily symmetric", so Example
  89's `ex:acronym` must never be closed — "FAO" is an acronym for "Food and Agriculture
  Organization" and the converse is false. The Example 89 test states the `rdfs:subPropertyOf`
  axiom, uses the refined property, and asserts that no `ex:acronym` statement is invented in
  either direction.
- **What is not:** the **sound** inference is not made either. `<B> ex:acronym <A>` entails
  `<B> skosxl:labelRelation <A>` under RDFS, which S62 then closes to `<A> skosxl:labelRelation
  <B>` — the super-property is symmetric even though the refinement is not. We make neither step,
  because this crate reads no `rdfs:subPropertyOf` anywhere. **B.4.1 says the property "is not
  intended to be used directly, but rather as an extension point"**, so a refinement is the
  *ordinary* way B.4 is used, not the exotic one — which makes this gap larger than the four
  statements suggest. A thesaurus whose ISO 25964 label relationships are expressed through
  `ex:acronym` and its siblings reads to us as a thesaurus with no label relationships at all, and
  reports "0 links" rather than "links we did not understand". That is the same shape as a
  silently broken discovery connector, which `CLAUDE.md` §8 of the driver calls out by name.
- **How to tell:** `openbiz inspect` omits the link line entirely for such a vocabulary. There is
  no signal distinguishing "no links" from "links expressed in a vocabulary we do not read".
- **What would close it:** RDFS sub-property reasoning, which is not a Phase 2 item and does not
  belong in `openbiz-skos` as a special case for one property. The honest home for it is the
  reasoner (`openbiz-owl`, Phase 5) or a SHACL rule pack (Phase 4). Raised in `docs/PROPOSED.md`
  rather than taken, because deciding where entailment lives is exactly the standing question
  iterations 18, 20, 21 and 22 kept ending on.
- **Opened:** iteration 23

### An entailed label is in our answers and not in our exports
- **Kind:** partial-coverage
- **What is proven:** a concept labelled only through SKOS-XL is reported by `openbiz inspect` as
  having plain SKOS labels, counted in the language coverage, and named by them — asserted end to
  end through the binary against a fixture that states no plain label anywhere.
- **What is not:** `openbiz export` hands out the **asserted** graph. So the same vocabulary,
  exported, contains no `skos:prefLabel` at all, and a generic RDF browser, a DCAT catalogue or a
  SPARQL query written against `skos:prefLabel` sees an unlabelled thesaurus. Nothing in the build
  says so to the person downloading the file. This is iteration 21's recorded doubt about S11 with
  a concrete instance attached, and it now applies to S7, S8, S29, S36 and S55–S57 as well.
  The opposite choice — materialising entailments into the vocabulary — was rejected at iteration
  20 and that reasoning still holds: a materialised statement is indistinguishable from an asserted
  one to every other reader, so the user would own statements they never wrote and cannot delete.
  Both cannot be right, and neither has been decided.
- **What would close it:** a decision on entailment materialisation, recorded as an ADR. The
  shapes worth comparing are an `--entailed` flag on `export`, a separate entailed named graph the
  user can choose to serve, and a cached model invalidated by the candidate seam's apply step.
  Raised in `docs/PROPOSED.md` rather than taken here, because it changes what a customer's data
  *is* and that is not a decision to make inside an item about labels.
- **Opened:** iteration 22

### The window before the stop signals are registered is closed as far as a program can close it
- **Kind:** partial-coverage
- **What is proven:** `StopSignals::install()` registers `SIGINT` and `SIGTERM` synchronously, at
  the top of `serve()`, before the graph registry is read and before the listener binds. Two tests
  assert it: that the registration is logged before the port is, and that a `SIGTERM` sent as soon
  as the registration line appears still exits zero and still closes the store. Both were shown to
  discriminate — moving the registration after the bind fails one, never registering `SIGTERM`
  fails five.
- **What is not:** the window from `exec` to that call. Process start, argument parsing, config
  load and **the store open** all happen first, and a `SIGTERM` during any of them is still a hard
  kill under the default disposition. No program can register a handler before it runs, so this
  cannot be closed entirely — but the store open is the slow part and it is on the wrong side of
  the line. Moving the registration into `main` before the store opens would shrink it further and
  has not been done, because `install()` would then also run for `openbiz backup` and the other
  one-shot commands, where a stop *should* be abrupt.
- **What would close it:** decide whether the one-shot commands want graceful stops too. If they
  do, `install()` moves to the top of `main` and this shrinks to argument parsing. If they do not,
  this entry is the floor and should be marked environment-limited instead.
- **Opened:** iteration 22

### `the_graph_registry_is_read_at_startup` failed twice on CI and has not been reproduced
- **Kind:** partial-coverage
- **What is proven:** the test passes on the loop machine — 27 consecutive runs, 12 of them with
  every core saturated, at eight test threads. It has passed on every CI run since, including the
  two after the changes below.
- **What is not:** *why it failed*. It failed on iteration 22's branch and again on `main` after
  the merge, both times on `assert!(server.wait_for_exit().success())` with no status, no child
  log, and no way to tell a hard kill from a non-zero exit out of `main`. Two real defects were
  found while looking, and either could explain it, but **neither is confirmed as the cause**:
  1. **The stop signals were registered lazily**, on the first poll of the future handed to
     `axum::serve(..).with_graceful_shutdown(..)` — which happens after the port is logged. A
     `SIGTERM` in that window is a hard kill under the default disposition. Fixed at iteration 22
     (`StopSignals::install`), with two tests and two mutations. This explains the first failure
     cleanly and **cannot** explain the second, which ran with the fix in place.
  2. **The readiness probe only proved the port was bound**, not that the server answered. The
     listener is bound and logged before `axum::serve` is entered, so a TCP connect succeeds out
     of the accept backlog — the probe could return while the process was still short of serving,
     and it left an accepted-but-never-answered connection behind for the graceful drain. Fixed at
     iteration 22 in both `graceful_shutdown.rs` and `backup_restore.rs`, which had the same
     harness copied.
- **What would close it:** a reproduction, or enough green CI runs to say the two fixes above
  covered it. The assertions now print the child's exit status and its whole log, so the next
  failure — if there is one — arrives with its evidence attached. That instrumentation is the
  real deliverable here: the previous two failures were unusable.
- **Watch for:** a third failure. If one comes with a non-zero exit *and* an `anyhow` message in
  the log, the cause is one of `serve`'s two post-drain refusals — "the store was still in use
  after the server drained" or "the store did not close cleanly" — and that is a product defect
  under load, not a test one.
- **Opened:** iteration 22

## The retired-claims sweep was done by hand, and nothing stops the next one being missed

- **What was done:** iteration 27 grepped the repository for the retired absolute claim "there is
  no OWL 2 DL reasoner in Rust" and for `horned-owl`, and corrected `README.md`, `CLAUDE.md` (crate
  map, candidate list, and the §5 licence example), `crates/openbiz-owl/src/lib.rs` (module docs and
  `Profile::Dl`), and the superseded paragraph in `docs/COMPETITIVE.md`. The rule that produced the
  sweep is written down at the top of `COMPETITIVE.md`, with a table of what has been retired.
- **What is unproven:** that the sweep was **complete**. It searched for the phrasings the loop
  happened to think of — `no OWL 2 DL reasoner`, `no DL reasoner`, `DL reasoner`, `HermiT`,
  `horned-owl` — across `.md`, `.rs`, `.ts` and `.tsx`. A paraphrase that shares no distinctive
  substring with any of those would have been missed silently, and a grep that finds nothing looks
  exactly like a repository that says nothing wrong.
- **What is worse:** the rule itself has **no enforcement**. It is a convention in a research file,
  guarding against the loop forgetting a convention. The proposal to make it a CI check with a
  machine-readable ledger is in `docs/PROPOSED.md`, unpromoted, so today the guarantee is only as
  good as the next iteration's memory — which is the thing that already failed once.
- **What would close it:** that CI check, or a periodic human read of `README.md` against
  `COMPETITIVE.md`'s corrections. Only the first is cheap enough to happen reliably.
- **Opened:** iteration 27

### What a note costs in the model is unmeasured, and notes are the bulk text of a vocabulary
- **Kind:** partial-coverage
- **What is proven:** the model reads and keeps every SKOS documentation property, and S17 adds at
  most one entry per stated note. Correctness is tested against §7's Examples 22, 23 and 24 and
  against both statement orders.
- **What is not:** the **cost**. `adr/0024` measured the semantic relation model and produced a
  hard number — about 3.9 KiB of resident memory per stated `skos:broader`, 4.4 GiB at a million
  links. There is no equivalent number for a note, and a note is the *longest* text a vocabulary
  holds: a 100 000-concept thesaurus with a paragraph of definition on each is tens of megabytes of
  text before any `BTreeMap` overhead, and `openbiz inspect` holds all of it at once. The lifted
  S17 entries roughly double the per-value map entries without duplicating the string.
- **Why it matters more than it looks:** `CoreModelBuilder`'s doc comment already had to be
  corrected once, at iteration 26, because it claimed the model was proportional to structure and
  the relations had made that untrue. This is the second thing that makes it untrue, and it is
  recorded before the claim is made rather than after it is caught.
- **What would close it:** a `scale` harness case that generates a vocabulary with notes of a
  realistic length at 10k / 100k / 1M concepts and reports resident bytes per note, the way
  `scale::tests` already does for relations. The harness exists; the case does not.
- **Opened:** iteration 29

### ~~A vocabulary's own note refinements are invisible, because we still read no `rdfs:subPropertyOf`~~ — CLOSED, iteration 31
- **Closed by** `adr/0028`. A first pass reads `rdfs:subPropertyOf` and resolves it against the
  seven; `ex:usageNote` reaches the report as a `skos:scopeNote` citing `rdfs7`, S17 lifts it to
  `skos:note`, and `openbiz inspect` names the declared properties rather than only counting them.
  Proven against the binary on disk. **The `skosxl:labelRelation` half of this entry is still
  open** — see the entry above it, which is unchanged.
- **Kind:** partial-standard
- **What is proven:** the seven properties SKOS names are read, and S17 lifts the six onto
  `skos:note`.
- **What is not:** §7.1 says the seven "provide a set of extension points for defining more
  specific types of note". A vocabulary declaring `ex:usageNote rdfs:subPropertyOf skos:scopeNote`
  and then using `ex:usageNote` gets **nothing** — no scope note, no note, and no row in the
  coverage table. The statements are counted among the non-SKOS ones and dropped. An enterprise
  thesaurus that has been extended this way will read as less documented than it is, and the report
  gives no hint that it is looking past something.
- **This is the same gap as the `skosxl:labelRelation` refinement entry above**, and one mechanism
  should close both: reading `rdfs:subPropertyOf` out of the graph needs either buffering or a
  second pass, because a declaration can arrive after the statement that uses it, and the chain is
  graph-controlled so it needs a cycle guard and a bound. The `skosxl` case is *harder*, because
  B.4.4.1 says a refinement of a symmetric property must not be closed even once you can see it;
  this one is not, because a sub-property of an annotation property is just a sub-property.
- **What would close it:** Phase 2's "Documentation properties, part 2" item, which is in the plan
  unchecked and was split out for exactly this reason.
- **Opened:** iteration 29

### Every read of a vocabulary now scans the store twice, and the second scan is unmeasured
- **Kind:** unmeasured-cost
- **What is proven:** `openbiz inspect` and `openbiz notes` both go through
  `inspect::read`, which runs `Store::for_each_statement` twice — once for `rdfs:subPropertyOf`,
  once for everything else (`adr/0028`). It is correct, and the tests cover the behaviour.
- **What is not:** what it costs. Every vocabulary pays the first pass, including the large
  majority that declare no refinements at all, because the pass cannot know that until it has
  finished. `scale.rs` generates no `rdfs:subPropertyOf` and does not go through the store, so
  there is no number for the second scan at 10k, 100k or 1M statements, and no comparison against
  the one-pass build it replaced. `adr/0024` has a hard number for the option it rejected;
  this ADR has none for the option it shipped, which is the same criticism iteration 28 made of
  `adr/0025`.
- **How to tell:** it will show as `inspect` taking roughly twice as long on a large import, with
  no report line explaining why.
- **What would close it:** a timing case in `scale.rs` that reads through a real `Store` rather
  than a statement vector, at the three sizes, with and without the first pass. That harness does
  not exist — every `scale.rs` case builds the model from an in-memory iterator.
- **Opened:** iteration 31

### `RefinementBound::DEFAULT` is a judgement about vocabularies nobody here has seen
- **Kind:** unproven-default
- **What is proven:** the bound works and is shared across the resolution rather than per property
  — `the_step_budget_is_shared_across_every_property` asserts it and was proven to fail against a
  per-property mutant. `an_exhausted_resolution_reports_unchecked_and_not_inconsistent` proves the
  exhausted path reports at `Severity::Unchecked` and leaves `is_consistent()` true.
- **What is not:** the two numbers. 10 000 declared properties and 100 000 edges were chosen by
  reasoning about what a schema plausibly holds, not by measuring one. **Neither has been reached
  outside a test that lowered them** — the same gap iteration 28 opened for `AncestryBound::DEFAULT`
  and iteration 30 closed by hitting it for real. There is no fixture here where the real default
  fires, so an operator hitting it would be the first.
- **How to tell:** the report says the resolution stopped and how much was left. That part is
  proven; what is unproven is whether it can ever legitimately happen.
- **What would close it:** an end-to-end case through the binary whose declared property graph
  reaches the shipped default, the way iteration 30's 1 500-deep chain did for ancestry.
- **Opened:** iteration 31

### No fixture here is a real extended thesaurus
- **Kind:** unproven-against-real-data
- **What is proven:** §7.1's extension point works on graphs this loop wrote — a single
  declaration, a two-step chain, a cycle, a refinement of a non-note property, one property
  refining two of the seven, and a graph carrying its own copy of S17.
- **What is not:** that those shapes are the ones enterprise thesauri actually use. `CLAUDE.md` §6
  forbids real vocabulary data in fixtures without a clear licence, so every case here was invented
  by the same process that wrote the code — which means a shape neither of us thought of is
  invisible to both. The specific unknown is whether refinements in the wild are declared in the
  vocabulary graph at all, or in a separate ontology graph the vocabulary imports; if the latter,
  the first pass reads the wrong graph and finds nothing, and reports "no declared refinements"
  rather than "the declarations are somewhere I did not look".
- **How to tell:** it cannot be told from inside this repository.
- **Partly told at iteration 37, from outside this repository, and the answer was not the one this
  entry guessed.** AGROVOC's public SPARQL endpoint says its 21 `rdfs:subPropertyOf` declarations
  into SKOS are **in the same graph as the concepts** (`http://aims.fao.org/aos/agrovoc/`), so the
  "declarations are in an imported ontology graph we never look at" fear named above **does not
  hold there** and our first pass reads the right graph. What it found instead is worse for our
  coverage: **not one of the 21 refines a documentation property.** Eight refine `skos:notation`,
  twelve refine `skos:related`, and one refines **`skos:broader`** — a hierarchy link invisible to a
  reader that does not entail from `rdfs:subPropertyOf`. Every fixture here refines a note property.
  Separately, only **2 of the 21 are used on any statement**, so a report listing declarations says
  nothing about which are live. See the iteration-37 proposal in `PROPOSED.md` for the method and
  the numbers; this remains open because one vocabulary is not a population and none of it is a test.
- **What would close it:** a permissively-licensed published SKOS thesaurus that uses §7.1's
  extension point, read end to end. Recorded as a proposal rather than taken, because finding one
  is research and not engineering.
- **Opened:** iteration 31

### ~~S45 is not applied, so `skos:exactMatch` is a link and not an equivalence class~~

**Closed at iteration 33.** `CoreModel::exact_match_cluster` walks the closure, Example 62 is
entailed, and S46 is checked across it by `check_exact_match_closure_disjointness` — so the
`<A> exactMatch <B> exactMatch <C>` with `<A> broadMatch <C>` vocabulary this entry named is now
reported as inconsistent, with the chain as its derivation, end to end through the binary. The
pinning test was replaced by its opposite rather than deleted. See `adr/0030`.

**What replaced it, narrower:** S42's lift is not applied *across* the closure. A concept reached
only by chaining is reported as an `skos:exactMatch` and not also as the `skos:closeMatch` S42
entails from it, so `openbiz mappings` lists the chained concepts under one heading and not two.
The conclusion is one step from what the report already prints, under a heading it already prints,
and the cost of stating it would be a second walk per report — but it is a conclusion SKOS licenses
and this build does not draw, which is what this ledger is for. Closing it means deciding whether
the close-match section lists walked members or stays the one-step section it is today, and that is
a report-design question rather than a rule.

- **Opened:** iteration 32, closed iteration 33; the narrower entry above opened iteration 33

### A mapping link's cost has never been measured, and the scale harness cannot produce one

- **Kind:** partial-coverage
- **What is proven:** nothing about cost. The arithmetic is that a stated `skos:broadMatch` now
  produces a mapping entry, its S43 converse, a lifted `skos:broader`, that link's converse, both
  transitive variants, and a derivation for each — more per statement than the 3.9 KiB per stated
  `skos:broader` that `adr/0024` measured, which was already the largest per-statement cost in the
  model.
- **What is not:** any of it. `crates/openbiz-skos/src/scale.rs` generates concepts, hierarchies
  and associative links, and **no mapping links at all**, so every shape it measures is a
  vocabulary with no outward links. A thesaurus mapped concept-for-concept to a second one is an
  ordinary enterprise artefact and is exactly the shape nothing here has ever been run against.
  This is the same finding iteration 31 recorded about labels and notes, on a third axis: the
  generator has one dimension and the model now has four.
- **What would close it:** a mapping row in the scale harness — a vocabulary of N concepts each
  carrying one `skos:broadMatch` and one `skos:exactMatch` to a second namespace — measured at 10k
  and 100k, with the per-link cost compared against `adr/0024`'s number for a stated relation.
- **Widened at iteration 33, and this is now the second iteration running to record it.** The
  closure sweep added a *second* unmeasured cost, of a different kind: S46-across-S45 walks once
  per concept holding a `skos:exactMatch`. The arithmetic for the concept-for-concept case — every
  cluster has two members, so two links per concept — reaches `EquivalenceBound::DEFAULT`'s million
  at about 500 000 mapped concepts, and that half is still arithmetic and still unmeasured, because
  `scale.rs` generates no mapping links.
- **Iteration 37 measured the density a real mapped thesaurus has, though not our cost of it.**
  AGROVOC carries **36,402 `skos:exactMatch`, 13,888 `closeMatch`, 261 `broadMatch`, 72
  `narrowMatch` and 13 `relatedMatch`** across 41,825 concepts — about 1.2 mapping links per
  concept, and the overwhelming majority of them the `exactMatch` that drives the quadratic sweep
  above. So the concept-for-concept shape this entry calls "an ordinary enterprise artefact" is
  confirmed ordinary, and the input that would exercise it exists and is CC BY 4.0. What is still
  unmeasured is ours: no mapping link has ever been through our model.
- **The dense case is measured, and it is worse than the arithmetic suggested.** I wrote the
  paragraph above as reasoning, then measured it, and the number disagreed with the shape I had
  assumed. A **hub** — *n* vocabularies all declaring their concept equivalent to one central
  concept — is a single cluster walked once per member, so the sweep costs about **2n²**: measured
  at 220 links for 10 members, 20 200 for 100, and 321 200 for 400.
  `the_sweep_cost_is_quadratic_in_a_cluster_and_not_linear_in_the_vocabulary` now pins it. So a
  **1 000-member cluster exhausts the default budget on its own**, on a vocabulary of a thousand
  concepts, and the report would truthfully say S46 is unchecked — which is honest, useless, and
  would look like a defect to the customer whose vocabulary is the most carefully mapped one we
  have ever been handed.
- **What would close the dense half:** walking each *component* once instead of each *member*,
  which makes the sweep linear — every member of a cluster has the same cluster, so *n* walks
  recompute one answer *n* times. It was not done here because it changes what the per-concept
  bound findings mean and the item was already the whole of §10's part 2. It is the obvious next
  move if any real vocabulary turns out to have dense clusters, **and nothing here can tell us
  whether one does**: no fixture in this repository has a cluster larger than four.
- **Opened:** iteration 32, widened and half-measured iteration 33

### ~~There is no per-concept view of what a concept is mapped to~~

**Closed at iteration 33.** `openbiz mappings <graph> <resource>` prints the five properties, the
origin and quoted rule for every link the graph did not state, S41's lift per section, and the
concepts reachable only by chaining exact matches with the chain that reached each. Proven in the
module's own tests and end to end against the real binary reading a store off disk.

- **Opened:** iteration 32, closed iteration 33

### `rdfs:subPropertyOf` and `rdfs:subClassOf` are reported but not entailed from, so a vocabulary using SKOS's own extension point reads as substantially unchecked
- **Kind:** partial-standard
- **What is proven:** that the gap is *reported*. `openbiz integrity` scans every RDFS declaration
  a graph makes, walks each declared term up to the SKOS and SKOS-XL terms it reaches, and marks
  every integrity condition checked over one of those terms `Verdict::Unchecked` rather than held —
  with the declaration named and the chain printed. Proven against a one-step declaration, a
  two-step chain, a cycle, `rdfs:subClassOf` as well as `rdfs:subPropertyOf`, and end to end
  against the binary reading a store off disk. See `adr/0031`.
- **What is not:** the entailment itself. `ex:seeAlso rdfs:subPropertyOf skos:related` should give
  every `ex:seeAlso` statement a `skos:related`, under RDFS rule rdfs7, and it does not — the
  statements are still read as non-SKOS and dropped. `rdfs:subClassOf` is read **only** to raise the
  caveat; nothing is inferred from it at all. So on a thesaurus that uses §7.1's extension point
  over a SKOS property — which is ordinary enterprise practice — five of the sixteen conditions come
  back unchecked, and that is the true state of the build rather than a presentational problem.
- **Why it was not simply done here:** it is a decision about closure, not a missing line. The
  `refinement` module resolves note properties precisely because §7 states no integrity condition,
  so a wrong entailment there cannot make a graph inconsistent. Over `skos:related` it can, in both
  directions — and B.4.4.1's warning that a sub-property of a symmetric property need not be
  symmetric is the standing evidence that this reasoning is not uniform across SKOS's properties.
  It needs its own item.
- **What would close it:** resolving declared refinements against the semantic-relation, mapping and
  labelling properties as well as the seven note properties, entailing the SKOS statement with a
  derivation citing rdfs7 and the chain, and replacing each `Caveat::UnreadRefinement` test with its
  opposite. The tests that pin the current behaviour carry that instruction in a comment and must
  not be deleted to make a build pass.
- **Opened:** iteration 34

### The RDFS declaration scan has never been run against a vocabulary with more than a handful of declarations
- **Kind:** partial-coverage
- **What is proven:** the bound behaves. The step budget is shared across the whole scan rather than
  spent per term — pinned by a test using five separate clusters and a budget for two, which is the
  only shape that tells a shared budget from a per-walk one — and the ceiling on distinct terms
  refuses rather than grows. An exhausted scan makes every condition unchecked and says so.
- **What is not:** any cost measurement. `crates/openbiz-skos/src/scale.rs` generates concepts,
  hierarchies and associative links, and **no `rdfs:subPropertyOf` at all**. This is the fourth
  dimension of the model the generator does not produce — after labels and notes (iteration 31),
  mapping links (32) and cluster density (33) — and it is the third iteration running in which the
  honest closing line has been "the generator only ever produces the easy shape".
- **What would close it:** a declarations row in the scale harness — a vocabulary of N concepts
  whose schema declares M refinements at chain depths 1, 2 and 10 — measured for build time and
  peak memory at the sizes the other rows use.
- **Opened:** iteration 34

### `CAPABILITIES.md` is hand-written prose with nothing checking it against the build
- **Kind:** partial-coverage
- **What is proven:** every claim in it at iteration 48 was written from a primary source in this
  repository — the `USAGE` constant, the route table in `crates/openbiz-server/src/lib.rs`, the
  checked boxes in `BUILD-PLAN.md`, and the ADR each paragraph cites. The counts (55 done, 166 open,
  221 total) were derived by counting boxes, not remembered, and every `adr/` link was resolved
  against `ls docs/adr/` after a first pass got **twenty of them wrong** by inventing plausible
  filenames.
- **What is not:** that it stays true. Nothing fails when a command is added, a bound changes, or a
  phase completes — the file is prose and the build has no opinion about prose. **This has already
  happened once in the file it replaces**: `README.md`'s capability sections stopped at
  `adr/0026` and were missing `search`, `tree`, `paths`, `mint`, `policy`, `move`, `merge`, `split`,
  `deprecate`, `reinstate`, `integrity` and `mappings` — twelve commands, roughly fifteen
  iterations of drift, and nothing reported it. The surface that can now drift is larger than the
  README's was, not smaller.
- **What is worse:** a stale capability document is the specific failure `CLAUDE.md` §4 calls worse
  than lacking the capability, and this one is written *for someone evaluating the product*. It is
  the same defect as the CLI usage list below, one level up and with no test at all rather than a
  one-directional one.
- **What would close it:** a CI check that reads the command words out of `USAGE` and the phase
  counts out of `BUILD-PLAN.md` and fails when `CAPABILITIES.md` does not mention one — the cheap
  half of it, since prose about *behaviour* cannot be checked this way. Proposed rather than built,
  because deciding what a docs check may fail on is a judgement the loop should not make alone.
- **Opened:** iteration 48

### The CLI usage list is hand-maintained in one direction only
- **Kind:** partial-coverage
- **What is proven:** every command word the test names appears in `USAGE` **and** parses without
  `ArgsError::UnknownCommand`, which is new at iteration 34 and catches a command documented but
  never wired.
- **What is not:** the reverse. A command the parser accepts and the list omits is invisible to the
  test, which is how `mappings` went undocumented in it for a whole iteration — the second time
  that list has drifted, after `inspect` and `ancestors` at iteration 26. The test's own docstring
  warns about exactly this failure and the test still could not catch it.
- **What would close it:** parsing the command words out of `USAGE` itself, or dispatching the
  parser through a table both it and the test read, so neither side can be extended alone.
- **Opened:** iteration 34

### The downward walk's cost has never been measured, and the generator makes it look cheap
- **Kind:** partial-coverage
- **What is proven:** the walk is bounded on both axes and an abandoned walk is distinguishable from
  a finished one, going down as well as up — pinned by a test that hits each bound separately. The
  two directions agree over every ordered pair of a four-concept polyhierarchy with a cycle in it,
  so a defect in one cannot survive in the other.
- **What is not:** any number. `adr/0024` measured the *upward* walk at 10k, 100k and 1M links, and
  the downward one has never been run at any size. The two are not symmetric in cost even though
  they are symmetric in code: the concepts above a leaf are a handful, and the concepts below a top
  concept are most of the vocabulary. `openbiz tree` on the root of a large thesaurus is therefore
  the first path in this build whose *ordinary* answer is the size of the vocabulary, and its
  rendering allocates one line per descendant.
- **And the same generator gap as the last four iterations:** `crates/openbiz-skos/src/scale.rs`
  builds a **chain**, which is the shape in which a subtree is small at every concept but the top.
  A broad shallow thesaurus — the ordinary shape — is the one that makes `tree` expensive, and no
  fixture here has one.
- **What would close it:** a breadth row in the scale harness (N concepts, branching factor B,
  depth D) measured for `descent` from the root and for the rendered report's size, at the sizes
  `adr/0024` used.
- **Half of that is done at iteration 40 and the half that matters is not.** The generator now
  builds breadth — `Shape::Tree` was always there and `Shape::Polytree` and `Shape::Lattice` join
  it — so the *input* this entry asks for exists at 10k, 100k and 1M. **Nothing measures `descent`
  from the root on any of them.** The route column added this iteration measures the walk *upwards*,
  which is the cheap direction and the one `adr/0024` already covered, so this entry's central
  claim is untouched: the first path in this build whose ordinary answer is the size of the
  vocabulary has still never been run at any size. Do not read the new shapes as closing it.
- **Opened:** iteration 35 · **Amended:** iteration 40

### `WalkBound::DEFAULT` going down is a ceiling an ordinary vocabulary reaches, and nobody here has reached it
- **Kind:** untested-boundary
- **What is proven:** hitting either bound is reported rather than silently truncated, with a
  closing sentence that differs from the complete one, and the tests hit both bounds with a custom
  `WalkBound`.
- **What is not:** the default. 100 000 nodes was chosen in `adr/0024` as a backstop for the *upward*
  walk, where an ordinary vocabulary is nowhere near it. Downwards, a walk from a top concept covers
  most of the vocabulary, so a thesaurus larger than the bound reaches it **because it is large** —
  and what that report looks like, how long it takes to produce, and whether a customer reads
  "this tree is a lower bound and not the answer" as an honest limit or as a defect in us, are all
  unknown. It is deliberately not raised without a measurement.
- **What would close it:** running `openbiz tree` from the root of a generated 150 000-concept
  vocabulary and recording the time, the memory and the output size, then deciding the default
  against numbers rather than against `adr/0024`'s upward reasoning.
- **Opened:** iteration 35

### ~~Paths to a root, and naming a cycle, are not implemented~~ — CLOSED, iteration 36
- **Kind:** not-built
- **What is proven:** nothing, and this entry exists so that the backlog item's own wording does not
  read as done. `openbiz tree` shows *one* route to each descendant — the breadth-first shortest —
  and names the routes it could not show, but it does not enumerate every path, and it detects a
  cycle only in the one case where the cycle runs through the concept asked about.
- **What is not:** every path from a concept to a top concept, which is what a breadcrumb needs and
  what part 2 of the split item covers. It is not a trivial extension of the walk: the number of
  ancestors is linear in the hierarchy and the number of *paths* is not, so it needs a bound of its
  own with a different failure mode — and a cycle makes the number of paths infinite rather than
  merely large.
- **What would close it:** part 2 of the concept tree item.
- **Opened:** iteration 35
- **Closed:** iteration 36. `CoreModel::paths_to_root` enumerates every simple route up under its
  own `PathBound`, `HierarchyCycle` names each loop with the way up that ran into it, and
  `openbiz paths <graph> <concept>` is the production caller. The two readings of "root" are kept
  apart rather than collapsed — see the entry below, which is what this one turned into.

### ~~`PathBound::DEFAULT` is a judgement about polyhierarchies nobody here has measured~~ — CLOSED, iteration 40
- **Kind:** untested-boundary
- **What is proven:** each of the three ceilings refuses rather than truncating, and an incomplete
  enumeration is distinguishable from a complete one — pinned by a test that hits the route ceiling
  and the step ceiling separately on a lattice of sixteen routes, and by one that hits the cycle
  ceiling on a hierarchy with two loops and no routes at all. The same test asserts the thing the
  bound exists for: on that lattice the *ancestry* is complete at eight concepts while the route
  list is not, from the same hierarchy at the same moment.
- **What is not:** the numbers. 10 000 routes, 10 000 cycles and 1 000 000 steps were chosen by
  reasoning — an ISO 25964 thesaurus is a handful of levels deep and a concept in one has one to
  three broader concepts, which puts an ordinary worst case in the low thousands — and **not by
  measurement**. That reasoning puts a real vocabulary uncomfortably near the route ceiling rather
  than safely below it, which is the opposite of `WalkBound::DEFAULT`'s position going up. Nothing
  here has ever enumerated routes on a vocabulary large enough to find out, and the step ceiling
  was taken from `adr/0024`'s link measurement, which measured a *walk* and not an enumeration.
- **And the generator still cannot produce the shape:** `crates/openbiz-skos/src/scale.rs` builds a
  chain, in which every concept has exactly one broader concept and therefore exactly one route up.
  It cannot generate a polyhierarchy at all, so the one input shape that would exercise this bound
  is the one shape the harness has never been able to make. That is the sixth consecutive iteration
  to record a gap that is really a gap in the generator.
- **What would close it:** a branching row in the scale harness — N concepts, B broader links each,
  depth D — with `paths_to_root` timed from a leaf at the sizes `adr/0024` used, and the default
  set against those numbers.
- **Two real vocabularies were counted at iteration 37 and both sit far below the ceiling, in the
  opposite direction to this entry's reasoning.** LC Genre/Form Terms — 2,685 concepts, **25.8% of
  them with more than one broader concept**, so a genuine polyhierarchy — has a worst case of
  **7 routes to a summit, at depth 3**. AGROVOC's 41,825 concepts have **at most 2** broader links
  each and none has three. The reasoning recorded above assumed branching and depth compound; in a
  real thesaurus **depth is 3–4 and stops them compounding**. That does not close this entry — two
  vocabularies are not a population, neither is the deep faceted kind, and neither number came from
  a test — but "uncomfortably near the ceiling" is no longer the honest way to state the doubt, and
  the branching generator this entry asks for is still the thing that would close it.
- **Closed, iteration 40, by building the generator this entry has asked for four times.**
  `crates/openbiz-skos/src/scale.rs` gained two branching shapes: `Polytree`, a balanced tree in
  which a share of the concepts state extra broader links — calibrated to LC Genre/Form Terms'
  measured 25.8% and its measured maximum of four — and `Lattice`, levels of *w* concepts each
  linked to the whole level above, whose routes multiply by *w* per level. Both are asserted
  against arithmetic that can be done by hand before any timing is read off them.
- **The number, and it settles the doubt in the direction iteration 37 pointed.** A realistic
  polyhierarchy at 10 001 / 100 001 / 1 000 001 concepts, with a quarter of the concepts carrying
  the measured maximum of four broader links, enumerates **10, 13 and 16 routes** from a widened
  concept. Not thousands. `max_paths` is 10 000, so a million-concept thesaurus of the shape a real
  one has sits **three orders of magnitude below the ceiling**, and the entry's original fear —
  "uncomfortably near" — was wrong in the safe direction.
- **What the ceiling *is* reachable by is a vocabulary of thirty concepts.** A binary lattice whose
  deepest concept sits on level 15 has 2¹⁴ routes from **56 links**, and
  `a_thirty_concept_lattice_exhausts_the_default_route_bound` pins both sides of the boundary: 29
  concepts enumerate 8 192 routes completely, 30 exhaust the bound and report an incomplete answer,
  and the step budget is nowhere near spent when it happens. So `max_paths` is not a size limit at
  all — it is a *shape* limit, and the shapes that reach it are legal SKOS graphs that no thesaurus
  practice produces. That is the right kind of backstop and it is now measured rather than argued.
- **Opened:** iteration 36 · **Closed:** iteration 40

### Nothing measures what an enumeration costs when it is abandoned rather than completed
- **Kind:** partial-coverage
- **What is proven:** the step ceiling stops the enumeration and reports it, and the tests hit it.
- **What is not:** the cost of the case the ceiling is for. A hierarchy in which every way up runs
  into a cycle records **no routes at all** while still spending steps building and abandoning them,
  so `max_paths` never fires and only `max_steps` can stop it. Every cyclic fixture here is three or
  four concepts, so the abandoned-work path has been proven correct and never proven affordable —
  and a million steps of it is a million `BTreeSet` insertions and removals on the route, which is
  the one part of this code whose constant factor nobody has looked at.
- **What would close it:** the same branching row as the entry above, with a cycle planted near the
  top so that the enumeration completes nothing, timed against the same vocabulary without it.
- **Still open after iteration 40, and the missing half is the cycle.** The branching row now
  exists, so the first half of what would close this is built — but every shape the generator makes
  is **acyclic by construction**: `Polytree`'s extra parents always point at an earlier index and
  `Lattice` links strictly one level up, which is exactly what makes them safe to generate and
  exactly what makes them useless here. The abandoned-work path is still proven correct on four
  concepts and never proven affordable.
- **Opened:** iteration 36 · **Amended:** iteration 40

### A route names its transitive-only steps and no export or endpoint carries that distinction
- **Kind:** partial-coverage
- **What is proven:** `RouteStep::is_stated` tells a stated `skos:broader` step from one licensed
  only by `skos:broaderTransitive`, `openbiz paths` draws the two differently and explains the
  difference, and a mutation collapsing them fails two tests.
- **What is not:** anything outside that one report. The distinction is the difference between "this
  concept is somewhere above that one" and "this concept is its parent", which is exactly what a
  breadcrumb in the interface will have to render — and Phase 3 has no way to ask for it, because
  there is no endpoint. The risk is that the interface reimplements the walk over `skos:broader`
  alone, silently drops every route through a transitive-only link, and shows a shorter breadcrumb
  than the vocabulary supports.
- **What would close it:** the concept-tree endpoint of Phase 3, carrying the flag, with a test that
  a route through a transitive-only step survives the JSON round trip.
- **Opened:** iteration 36

### Label matching neither case-folds nor normalises, so real thesaurus labels are unfindable
- **Kind:** partial-standard
- **What is proven:** matching is full Unicode lowercasing on both sides (`str::to_lowercase`), not
  ASCII, and tests cover French, Greek, and the context-sensitive final sigma. Both gaps are pinned
  by tests that **assert the miss**, so the day either is fixed the test fails rather than the
  ledger going stale silently.
- **What is not:** the two misses themselves are defects for a real user. (1) No case folding:
  `"Straße"` lowercases to itself, so `strasse` finds nothing — and German authoring conventions
  make that the likelier spelling to be typed. In Greek, `οδόσ` does not find `ΟΔΌΣ`, because a
  final sigma lowercases to `ς`. (2) No Unicode normalisation: composed `é` (U+00E9) and decomposed
  `e` + U+0301 are different strings that render identically, and iteration 37's measurement of
  AGROVOC's 1.25M SKOS-XL labels means multilingual corpora of exactly this kind are in scope. Both
  produce a report that says "nothing matched" for a concept that exists, which is `CLAUDE.md`
  §1.7's silo-generating failure precisely.
- **What would close it:** a dependency decision, which is why it was not taken inside a feature —
  `unicode-normalization` (MIT/Apache-2.0) for NFC, and a case-folding table for the other, against
  §1.5's dependency budget. Then the two pinning tests invert.
- **Opened:** iteration 38

### RFC 4647 extended filtering is not implemented, so a narrowed language range loses script tags
- **Kind:** partial-standard
- **What is proven:** basic filtering (§3.3.1) against the RFC's own example shapes — `en` selects
  `en-GB` and not `enm`; `de-DE` selects `de-DE-1996`; the wildcard selects any tag and never an
  untagged label; a malformed range is refused rather than silently matching nothing.
- **What is not:** extended filtering (§3.3.2). `--lang de-DE` does **not** match the tag
  `de-Latn-DE`, because basic filtering will not skip an intermediate subtag. A user narrowing to a
  region therefore silently loses script-tagged labels, and script subtags are ordinary in exactly
  the multilingual vocabularies this option exists for.
- **What would close it:** implementing §3.3.2's matching over subtag lists, and a test built from
  the RFC's own `de-DE` / `de-Latn-DE` example.
- **Opened:** iteration 38

### `SearchBound::DEFAULT` is a judgement about a reader, measured against nothing
- **Kind:** partial-coverage
- **What is proven:** the bound is enforced during the scan rather than after it, so memory is
  bounded by the ceiling and not by the match count; a test asserts the truncated list is exactly
  the head of the list a global sort gives; and the report distinguishes what matched from what is
  shown — including the zero case, which this iteration found reporting "nothing matched" when
  eight labels had.
- **What is not:** the number 200. It is reasoning about how much a person will read, not a
  measurement of what a real query against a real thesaurus returns. Iteration 37 measured AGROVOC
  at 41,825 concepts and 1.25M SKOS-XL labels; a two-letter infix query against that returns far
  more than 200 and the user is told to narrow, which may be right or may be the tool refusing to
  answer. This is the third unmeasured constant of its kind after `WalkBound::DEFAULT` and
  `PathBound::DEFAULT`.
- **What would close it:** the LCGFT fixture proposed at iteration 37, queried with the terms a
  cataloguer would actually type, with the match counts recorded.
- **Opened:** iteration 38

### Every search is a linear scan of a model rebuilt per request, and nothing indexes anything
- **Kind:** partial-coverage
- **What is proven:** correctness at fixture scale, and that the search reports how many labels and
  resources it read, so its cost is at least legible in its own output.
- **What is not:** that this is affordable. `openbiz search` builds the whole `CoreModel` from the
  store on every invocation and then walks every label of every resource — so the cost is the size
  of the vocabulary, per query, with no index and no reuse. At AGROVOC's measured 10M triples that
  is a full scan per keystroke for Phase 3's as-you-type search, which is the item that will make
  this untenable rather than merely wasteful.
- **What would close it:** a measurement first — the scan timed against a 100k-concept vocabulary —
  and only then a decision about indexing, which is a Phase 13 concern and should not be
  pre-empted here.
- **Opened:** iteration 38

### The engine-free IRI check is a subset of RFC 3987, and the store's parser is the real gate
- **Kind:** partial-standard
- **What is proven:** `openbiz-skos` will not mint anything that is not absolute (RFC 3986 §3.1's
  scheme grammar) or that carries a character an IRI may not, and `ucschar`'s ranges are
  transcribed from RFC 3987 §2.2 with the boundaries pinned by test (U+009F out, U+00A0 in,
  U+D7FF in, U+E000 out). Everything minted is then put to `openbiz_store::accepts_iri`, which is
  Oxigraph's own `NamedNode` parser, before it is shown to anybody — a pattern with a broken
  percent-escape is refused there and a test proves it. A minted non-ASCII IRI is imported into a
  real store and read back out unchanged (`tests/mint_iri.rs`).
- **What is not:** `plausible_iri` is not the RFC 3987 grammar and cannot be, in a crate that will
  not depend on an engine. It does not check percent-encoding, `iprivate` placement, the authority
  or path grammar, or IPv6 host literals. A caller of `openbiz-skos` that is *not* `openbiz-server`
  would get a weaker guarantee than the command line does, and there is no such caller today —
  which is exactly the sort of thing that stops being true quietly.
- **What would close it:** either an IRI grammar in `openbiz-skos` tested against RFC 3987's own
  examples, or a trait boundary that makes the engine's parser a required collaborator of minting
  rather than a courtesy the server happens to perform.
- **Opened:** iteration 39

### Every mint scans every vocabulary in the store, and that is unmeasured
- **Kind:** partial-coverage
- **What is proven:** correctness at fixture scale, across several vocabularies and staged
  candidates, and that memory is bounded by the *namespace* rather than the store — only IRIs
  under the pattern's prefix are kept, and a test pins that an IRI outside it is counted and
  dropped. The report prints both numbers, so the scan's cost is legible in its own output.
- **What is not:** the time. `openbiz mint` reads every statement of every registered vocabulary
  graph plus every pending candidate's additions, once per invocation, and builds the full
  `CoreModel` of the target on top of that. At iteration 37's measured AGROVOC scale (10M triples)
  that is a full store scan to answer one question, and nothing has been timed. The narrower
  alternative — scan only the target vocabulary — was rejected deliberately, because an IRI is a
  global identifier and two vocabularies extending one namespace is an ordinary enterprise case;
  the cost of being right about that has not been paid in measurement.
- **What would close it:** the scan timed against the 100k- and 1M-concept generators
  `openbiz-store`'s `scale` module already has, with the number recorded beside `adr/0013`'s.
- **Opened:** iteration 39

### `SlugBound::DEFAULT` is the fourth unmeasured constant
- **Kind:** partial-coverage
- **What is proven:** the bound cuts at a word boundary rather than mid-word, the result says it
  was truncated, and the derivation printed to the user says the IRI no longer carries the whole
  term.
- **What is not:** the number 96. No corpus of enterprise labels was consulted, and the label
  lengths of a real thesaurus are exactly the thing iteration 37's proposed LCGFT fixture would
  answer. This is the fourth constant of its kind after `WalkBound::DEFAULT`, `PathBound::DEFAULT`
  and `SearchBound::DEFAULT`, and the pattern of writing a plausible number and recording it here
  is now four iterations old.
- **What would close it:** label-length percentiles from a real published thesaurus.
- **Opened:** iteration 39

### The scale generator has branching now, and still generates no labels, notes, mappings or refinements
- **Kind:** partial-coverage
- **What is proven:** the hierarchy's *shape*, on five shapes rather than three.
  `crates/openbiz-skos/src/scale.rs` builds a detached baseline, a tree, a star, a chain, a
  polytree calibrated to LC Genre/Form Terms' measured 25.8% share and maximum of four broader
  concepts, and a lattice whose routes multiply per level. Each is asserted against hand-checkable
  arithmetic before any timing is taken from it.
- **What is not: four of the five axes six previous iterations recorded.** Branching was the axis
  iterations 31 to 36 kept deferring and iteration 40 built. The other four are untouched and this
  entry exists so that closing one does not read as closing them:
  - **no labels and no notes** (iteration 31) — so `openbiz search`, which scans every label of
    every concept linearly with nothing indexing anything, is measured here against **zero labels**;
  - **no mapping links** (iteration 32) — so the S43/S45/S46 passes and `EquivalenceBound` are
    measured against a vocabulary with no outward links at all;
  - **no dense `skos:related` clusters** (iteration 33) — the associative shapes are one link per
    concept, and the quadratic case is a hub;
  - **no `rdfs:subPropertyOf`** (iteration 34) — the §7.1 extension point that AGROVOC uses 21
    times in the wild.
  Each has its own entry above with its own "what would close it"; this one is the index, so that
  a reader who sees "the generator was widened at iteration 40" does not conclude more than that.
- **And every shape is acyclic by construction**, which is deliberate — the extra links always
  point at an earlier index — and which means the cyclic paths through `paths_to_root` and the
  §8.4 sweep are still measured on nothing.
- **What would close it:** one axis per blind-spot pass, labels first, because `search` is the
  command whose cost model is least understood and whose §1.7 justification is strongest.
- **Opened:** iteration 40

### A realistic million-concept polyhierarchy peaks at 8.2 GiB, which is more than `adr/0024` recorded and near this machine's ceiling
- **Kind:** untested-boundary
- **What is proven:** the number, measured this iteration in release on the loop machine.
  1 000 001 concepts with a quarter of them carrying LC Genre/Form Terms' measured maximum of four
  broader concepts is 1 749 986 stated links, 6 999 944 held entries, 5 249 958 derivations, a
  **1 809 MiB** `inspect` report, and a **peak RSS of 8 178 MiB** against a 3 144 MiB delta. The
  same size with one extra link instead of three peaks at 6 413 MiB.
- **Why it matters:** `adr/0024` measured the monohierarchic tree at the same size at **5 081 MiB**
  peak and sized every judgement in this build against it. Branching a quarter of the concepts adds
  **61%** to that peak, and the machine this loop runs on has 11 GiB. A vocabulary of that size and
  shape is not exotic — it is AGROVOC's shape at twenty-four times AGROVOC's size.
- **What is not:** whether it is a wall or a slope. Nothing was measured **above** a million
  concepts, on a machine with different memory, or with the store's own footprint alongside — every
  row here builds the model from an in-memory iterator in a process doing nothing else, and the
  real caller is a server holding an Oxigraph instance at the same time. `CLAUDE.md` §8 puts the
  hardware side of that outside the loop.
- **What would close it:** the same table taken with a `Store` open, and a stated policy for what
  the server does when a vocabulary will not fit — which is a product decision, not a measurement.
- **Opened:** iteration 40

### "Every producer mints under the recorded policy" has exactly one producer
- **Kind:** built-but-narrow
- **What is proven:** `openbiz policy` records a pattern and `openbiz mint`, in a **separate
  process**, mints under it rather than under what the vocabulary's own concepts suggest
  (`crates/openbiz-server/tests/iri_policy.rs`). The precedence — `--pattern`, then the record, then
  inference — is pinned in all three directions, and a recorded pattern this build cannot parse is
  refused rather than fallen back from.
- **What is not:** the claim the item exists for. `adr/0036`'s value is that an import, a discovery
  match, and an agent proposal all mint the same way as the curator, and **none of those mint at
  all today** — `openbiz import` takes IRIs already written in the file, `DiscoveryProvider` is
  still ahead of us in Phase 2, and agents are Phase 10. So the seam is in place and one caller uses
  it, and the sentence the reports print — "every producer mints under this" — is a statement about
  a build with one producer.
- **Why it is recorded rather than softened:** the sentence is the right thing to print, because it
  is what the record *means* and it is what the next producer will have to honour. But a reader of
  `BUILD-PLAN.md` should not conclude that several code paths were made consistent with each other,
  because there is only one.
- **What would close it:** a mint from each of the paths `adr/0036` names — import, discovery,
  agents — since those are the ones the record was written for.
- **Narrowed at iteration 44:** there is now a **second** producer. `openbiz split` mints one IRI
  per part through the same `pattern_for` resolution, offering each back to the scan before the
  next, and `crates/openbiz-server/tests/split_concept.rs` proves three parts get three numbers
  under a recorded pattern in a separate process. So "several code paths were made consistent with
  each other" is true now, of two of them. The entry stays open because the three paths `adr/0036`
  was actually written for still do not mint.
- **Opened:** iteration 41

### A replaced minting policy is not kept, so there is no history of the decision
- **Kind:** untested-boundary
- **What is proven:** recording a second pattern removes the first, leaves exactly one recorded (a
  test counts the quads, because a replacement that only *added* would make the next read refuse the
  whole record as corrupt), and prints the displaced pattern with its author and timestamp at the
  moment it stops being in force.
- **What is not:** any answer to "what policy was in force in March". The printed line is the only
  notice; nothing is stored. Today's honest answer is "read the IRIs minted in March" — the
  vocabulary's own contents are the record — which is true and is not an audit trail.
- **Why it matters:** `CLAUDE.md` §1 makes governance the substrate and PROV-O the audit model, and
  this is a governance decision with a deliberate hole in its history. It is a small feature and a
  real one: it wants an ordering, a retention answer (the same question `UNTESTED.md` already records
  unanswered for a candidate's evidence), and a place in the provenance model rather than three more
  quads invented beside it.
- **What would close it:** a versioned policy record, decided together with the retention question,
  when PROV-O arrives rather than before.
- **Opened:** iteration 41

### A recorded policy travels with a whole-store backup and not with a vocabulary export
- **Kind:** untested-boundary
- **What is proven:** the backup half. A policy recorded in one store, backed up, and restored into a
  fresh data directory is read back with its pattern and its author intact
  (`tests/iri_policy.rs`) — which follows from its living in the system graph, and is tested rather
  than reasoned about because the alternative placement would have passed every other test in that
  file.
- **What is not:** anything about the export half, which is a **known absence** rather than an
  untested claim. `openbiz export` and `GET /api/export` write one vocabulary's own statements, and
  the policy is deliberately not one of them (`adr/0036` §3). So moving a vocabulary between two
  OpenBiz deployments by export rather than by backup arrives with no policy and an inferred
  default, silently.
- **Why it is not simply fixed:** putting the policy in the vocabulary would fix the export and
  break the thing the placement exists for — a SKOS export carrying an OpenBiz configuration
  statement no standard defines. The real answer is probably an export mode that carries OpenBiz's
  own facts *alongside* the graph rather than inside it, which is a Phase 3 API question.
- **What would close it:** either that export mode, or a warning on import that the arriving
  vocabulary has no recorded policy — the second being cheap and worth considering first.
- **Opened:** iteration 41

### `openbiz move` cannot give a concept its first parent, so it cannot demote a top concept
- **Kind:** known-absence
- **What is proven:** the move refuses a concept with no broader concept, in as many words, and
  points at `openbiz import` as the way to do it today. A concept that is *both* a top concept and
  has a broader concept is moved and the report names the schemes it is a top concept of, asserted
  in `relocate.rs`'s own tests.
- **What is not:** there is no operation that gives a concept its first broader concept, and
  therefore no place where "and it stops being a top concept" could be implemented. A curator
  building a hierarchy out of a flat imported vocabulary — which is the ordinary way a first
  thesaurus arrives — cannot use `openbiz move` for the first level at all.
- **Why it was not built here:** two reasons, and the second is the harder one. It is a different
  operation with different refusals, and the item was already split four ways. And the core model
  closes S8 into both `Resource::top_concept_of` and `Resource::has_top_concept` **without
  recording which direction the graph asserted**, so a demotion cannot compute a removal that is
  guaranteed to be present — `propose_retraction`'s presence check would refuse half of them. That
  is a model gap, not a command gap, and fixing it touches every report that reads top concepts.
- **What would close it:** origin tracking on the top-concept sets, the way `RelationOrigin` does
  it for semantic relations, and then a "set the broader concept" operation that removes the
  top-concept statements the graph actually carries.
- **Opened:** iteration 42

### A directly-stated transitive link to a *non-adjacent* ancestor survives a move unexamined
- **Kind:** partial-coverage
- **What is proven:** a `skos:broaderTransitive` or `skos:narrowerTransitive` link stated directly
  between the concept and the parent it is leaving is refused, in both directions, and the entailed
  transitive link S22 lifts from `skos:broader` is correctly *not* refused — all three asserted.
- **What is not:** a graph stating `<concept> skos:broaderTransitive <grandparent>` directly. The
  move does not look at it, does not remove it, and does not mention it; after the move the concept
  is still under its old grandparent by S24 while the report says it moved. This is the same defect
  the adjacent-link refusal exists to prevent, one step further up, and nothing tests either way.
- **Why it was not handled:** it is genuinely ambiguous what the author meant. A direct transitive
  link to a distant ancestor may be a deliberate assertion about a relationship the one-step links
  do not carry, in which case removing it is wrong; or it may be stale, in which case leaving it is
  wrong. The adjacent case has no such ambiguity, which is why that one is refused.
- **What would close it:** a decision on what a direct transitive link to a non-adjacent ancestor
  means, and then either a refusal or a line in the report naming the ones that will survive.
  Reporting them is the cheaper half and would close the *silence*, which is the worse part.
- **Opened:** iteration 42

### The subtree count a move quotes has never been run against a large subtree
- **Kind:** unproven-at-scale
- **What is proven:** the count is the length of the same bounded downward walk that does the cycle
  check, so the two cannot disagree; that the walk stopping at its bound refuses the move rather
  than reporting an unchecked one, asserted with a bound of one node; and the count itself on
  subtrees of nought and one concept.
- **What is not:** the number in the sentence a reviewer actually reads before approving. Every
  fixture here has a handful of concepts. `WalkBound::DEFAULT` going down is a ceiling an ordinary
  large vocabulary *reaches* — `tree.rs`'s own module note says so, and there is a separate entry
  above about exactly that — so a move of a top concept in a 100 000-concept thesaurus is likely to
  be **refused** by decision 6 of `adr/0037` rather than reported. That is the honest behaviour and
  it may also be a product limit nobody has measured: the operator most likely to want a subtree
  move is the one with the largest subtree.
- **Why it was not measured here:** `openbiz-skos`'s scale harness generates hierarchies and would
  answer this, and the item was already at its size. The measurement is a separate, cheap piece of
  work.
- **What would close it:** running the existing generator at 10k/100k/1M and recording where the
  refusal starts, then deciding whether the downward bound a move uses should be its own number
  rather than `WalkBound::DEFAULT`.
- **Opened:** iteration 42

### `openbiz move` does not run the integrity check a merge now runs, and leaves an S27 violation
- **Kind:** defect in a checked-off item, reproduced by hand
- **What is proven:** it is real, and it was measured rather than reasoned about. At iteration 43,
  against the debug binary and a store on disk:
  ```
  ex:a a skos:Concept .
  ex:b a skos:Concept .
  ex:c a skos:Concept ; skos:broader ex:a ; skos:related ex:b .
  ```
  `openbiz move <v> ex:c ex:b` is **accepted**, the candidate approves, and
  `openbiz integrity <v>` afterwards reports `S27 VIOLATED (1)` where it held before. The move
  wrote `<c> skos:broader <b>` beside an existing `<c> skos:related <b>`, and §8.4 makes those
  disjoint. Nothing refused it and nothing warned.
- **What is not:** there is no test for this. `adr/0037`'s refusals are all about the *hierarchy*
  the move leaves — cycles, ambiguity, a stale transitive link — and none of them asks whether the
  resulting graph is still a SKOS vocabulary.
- **Why it was not fixed here:** the mechanism to fix it landed in this iteration
  (`openbiz_skos::newly_violated`, `adr/0038` decision 5) and the fix is one call plus a test. It
  was not taken because the loop's standing rule is one item per iteration and this is a second
  one; promoting my own scope is the brake `CLAUDE.md` §7 exists to keep. It is in `PROPOSED.md`
  with the reproduction above so the next iteration can act on it in one step.
- **What would close it:** `crates/openbiz-server/src/relocate.rs` calling `would_break` the way
  `merge.rs` does, a `CommandError` variant for it, and this reproduction as the failing test
  first. The same question should then be asked of `openbiz import` and `openbiz retract`, which
  can also leave a graph that is not a SKOS vocabulary and which say nothing either.
- **Opened:** iteration 43

### A merge cannot join two concepts whose labels are SKOS-XL, and refuses rather than reconciling
- **Kind:** product limit, proven to be a refusal rather than a defect
- **What is proven:** that the failure is loud. `merge_concepts.rs` imports two concepts each with
  a `skosxl:prefLabel` pointing at a label resource with an English literal form, and the merge is
  **refused** with `S14` named, because S55 would dump both down to preferred labels in one
  language on the survivor. The vocabulary is untouched and no candidate is staged.
- **What is not:** that an operator can do anything about it except retract a label by hand first.
  The label reconciliation in `adr/0038` decision 2 — demote the colliding preferred label to an
  alternative one — works on plain `skos:prefLabel` statements and has no SKOS-XL equivalent. The
  same hole applies to a label written with a *refinement* of `skos:prefLabel`, which is repointed
  as written; that one is untested, and it is only the integrity check standing behind it.
- **Why it was not handled:** the SKOS-XL analogue is not the same operation. Demoting a plain
  label rewrites a predicate; demoting an XL label means changing which SKOS-XL property points at
  the label resource, and B.3's S55–S57 chains then have to be re-read to know whether the result
  says what was meant. That is a decision about SKOS-XL semantics, not a branch in a rewrite.
- **What would close it:** an XL arm on `CoreModel::reconcile` that moves a `skosxl:prefLabel` to
  `skosxl:altLabel` when the survivor already has a preferred literal form in that language, plus
  Appendix B examples as the test. `ISO 25964 fidelity depends on SKOS-XL` (`lib.rs`'s own module
  note), so an enterprise thesaurus is more likely to hit this path than the plain one.
- **Opened:** iteration 43

### `ReferenceBound::DEFAULT` is the fifth unmeasured constant
- **Kind:** unmeasured judgement
- **What is proven:** that hitting it refuses rather than truncates, asserted with the bound set to
  four; and that a truncated scan cannot answer, because "the vocabulary does not already say this"
  is exactly the question the survivor's statements answer and a half-kept set answers it wrongly.
- **What is not:** the number. 100 000 statements about one concept was chosen by the argument that
  a hub concept is the one least likely to be merged into anything, which is plausible and
  untested. It joins `WalkBound::DEFAULT`, `PathBound::DEFAULT`, `SearchBound::DEFAULT`,
  `SlugBound::DEFAULT` and `RefinementBound::DEFAULT` — six constants now, each with an entry here
  saying the same thing, which is itself the finding: this build has a systematic habit of choosing
  a ceiling by argument and never returning to it.
- **What would close it:** the scale generator producing a hub concept and the bound measured
  against it — or, better, one piece of work that measures all six together, since they are the
  same question asked six times.
- **Opened:** iteration 43

### A merge is four passes over the graph and two models, and that is unmeasured
- **Kind:** unproven-at-scale
- **What is proven:** correctness. The integrity check reads the vocabulary as it *would be* — the
  graph without the removals, with the additions — and that is what catches S14 and S27.
- **What is not:** the cost. `crate::inspect::read` is two passes and one model; a merge adds a
  scan for the references and a second two-pass model read, so it is four passes and two models.
  Every fixture here has under a dozen statements. `adr/0013` measured that reading a large
  vocabulary is the expensive thing this build does, and a merge now does it twice. The
  cross-vocabulary reference count adds one more full pass **per other vocabulary in the store**,
  which is unbounded in the number of vocabularies rather than in their size.
- **Why it was not measured here:** the item was already carrying a decision that was not in the
  plan when it started (`adr/0038` decision 5), and measurement is separable work.
- **What would close it:** the existing scale generator at 10k/100k/1M, timing `openbiz merge`
  against a leaf duplicate, and a decision on whether the integrity check should be optional — with
  the honest note that making it optional is how a governance product ends up shipping the default
  that skips it.
- **Opened:** iteration 43

### A refined `prov:wasDerivedFrom` makes a split entail a violation this build cannot see
- **Kind:** partial-coverage
- **What is proven:** `openbiz split` writes `prov:wasDerivedFrom` into the **vocabulary** graph, and
  the whole SKOS condition set is run against the vocabulary the change would leave (`adr/0038`'s
  check, generalised into `crate::staging::newly_broken`). No input could be constructed that trips
  it, which is stated in `adr/0039` rather than presented as a guarantee.
- **What is not:** a vocabulary that declares `prov:wasDerivedFrom rdfs:subPropertyOf skos:related`
  makes a `--place below` split entail `skos:related` **and** `skos:broader` between the same pair,
  which S27 forbids. The check does not catch it, and the reason is honest behaviour elsewhere: this
  build reports S27 as **unchecked** in such a vocabulary — "this build entails nothing from it" —
  rather than falsely held, and a condition with no verdict on either side cannot be *newly*
  violated. Reproduced by hand at iteration 44 against a store on disk; the split succeeded, was
  approved, and `openbiz integrity` afterwards said `unchecked`, not `VIOLATED`.
- **Why it matters beyond this command:** it is a general property of the guard, not of splits. Any
  change whose statements interact with a refinement this build cannot read is invisible to it. The
  guard protects a vocabulary whose declarations we understand, and is silent about one whose
  declarations we have already admitted we do not.
- **What would close it:** either entailing through declared refinements of the SKOS properties, or
  refusing a change that touches a property whose refinements leave a condition unchecked. The
  second is cheap and may be too blunt; neither was attempted.
- **Opened:** iteration 44

### A part named what something here is already named behaves differently under the two minting policies
- **Kind:** partial-coverage
- **What is proven:** both behaviours, in
  `crates/openbiz-server/src/split.rs`. Under an **opaque** pattern (`{n}`) the report warns —
  "this vocabulary already has a concept by that name" — before it lists the parts, and stages the
  candidate anyway, because a large vocabulary has legitimate homonyms. Under a **readable** pattern
  (`{slug}`) the label becomes the local name, the IRI is therefore taken, and `openbiz mint`
  refuses rather than suffixing it, which is `CLAUDE.md` §1.7 working as designed.
- **What is not:** that the operator can act on the second one. The refusal they get is about an
  **IRI** — "already in use ... a new concept must not take an IRI something else denotes" — when
  their actual problem is that they tried to create a second concept with an existing name. The two
  are the same event and the message names the wrong one. Found by a test failing, not by design.
- **What would close it:** `CommandError::CannotMint` carrying whether the vocabulary also already
  carries that *label*, and saying so first. One field and one lookup, deliberately not folded into
  the item that discovered it.
- **Opened:** iteration 44

### A split propagates `skos:topConceptOf` in a direction it chose rather than one it read
- **Kind:** partial-coverage
- **What is proven:** under `--place beside`, a part of a split of a top concept becomes
  `<part> skos:topConceptOf <scheme>`, and under `--place below` it does not, which is right because
  a part below a top concept is not one. Both are tested in `openbiz-skos`.
- **What is not:** that the direction matches the vocabulary. `CoreModel` closes S8 on read, so it
  cannot say whether the graph asserted `skos:topConceptOf` or `skos:hasTopConcept`, and the
  subject-first form was chosen so a part reads the way `skos:broader` does everywhere else here.
  The same split gets the **broader** direction right, by reading `stated_directions`, which makes
  the inconsistency visible in one command's output: a downward-authored thesaurus gets
  `<parent> skos:narrower <part>` and `<part> skos:topConceptOf <scheme>` in the same diff.
- **Note:** this is the same core-model gap iteration 42 recorded against `openbiz move`, reached
  from a different direction. It is one gap, not two.
- **What would close it:** `Resource` recording the asserted direction of S8 the way it records the
  asserted direction of S25.
- **Opened:** iteration 44

### Nothing about `openbiz split` is measured on a large vocabulary
- **Kind:** partial-coverage
- **What is proven:** correctness on fixtures of a handful of concepts, in-process and against the
  real binary on disk.
- **What is not:** cost. A split reads the vocabulary once for the model, once more for the mint
  scan across **every** vocabulary in the store, and twice more for `newly_broken` — so it is at
  least four passes and one of them is store-wide. That is the same unmeasured claim `adr/0038`
  made about a merge, now made by a second command, and the recurrence is the finding: this is the
  seventh unmeasured cost or constant in this crate.
- **What would close it:** the split run against the 100k and 1M generated vocabularies `adr/0013`
  and `adr/0024` already build, with wall-clock and peak memory recorded.
- **Opened:** iteration 44

### ~~A retired concept reads exactly like a current one in every command that browses~~ — CLOSED, iteration 46
- **Kind:** partial-coverage
- **Closed by:** `openbiz_skos::Retirements`, read beside `CoreModel` in the pass
  `inspect::read_with_retirements` already makes, and consulted by `openbiz tree`,
  `openbiz ancestors`, `openbiz paths`, `openbiz search` and `openbiz inspect` — the five commands
  this entry named. The decision is show and mark, never hide, per `adr/0041`, and each report also
  states what its marks add up to rather than leaving that to the reader. Proven by five tests
  against the real binary in separate processes: retire a term through `openbiz deprecate` and
  `openbiz approve`, then read it back through each of the five commands.
- **What is still not proven**, and is recorded separately below rather than left inside a closed
  entry: `openbiz notes` and `openbiz mappings` do not carry the mark, there is no way to ask for
  current concepts only, there is no way to un-retire, and none of this is measured at scale.
- **Opened:** iteration 45 · **Closed:** iteration 46

### A deprecation's date and author live in the candidate and do not survive a vocabulary export
- **Kind:** partial-coverage
- **What is proven:** the candidate records who proposed the retirement, who approved it, and when,
  in `xsd:dateTime`. `openbiz backup` carries all of it.
- **What is not:** anything at the vocabulary level. Export one vocabulary as Turtle and it says the
  concept is deprecated and what replaces it, and nothing about when or by whom. An auditor handed
  the exported file — which is how a vocabulary usually leaves this system — cannot date the
  retirement. `adr/0040` chose this over inventing a date predicate (`prov:invalidatedAtTime` says
  the entity ceased to exist, which is false of a retired concept), but choosing it does not make
  the gap smaller.
- **Note:** the same shape as the recorded-minting-policy entry from iteration 41. Two features now
  keep governance facts in OpenBiz's graphs that a standards-compliant reader of the exported
  vocabulary cannot see; that recurrence is itself the finding.
- **What would close it:** a decision about whether provenance belongs in the vocabulary or in an
  export sidecar, taken once for both.
- **Widened at iteration 47.** `openbiz reinstate` has the same shape and makes it worse in one
  specific way: after a retirement is taken back, the *only* thing left in the exported vocabulary
  saying the retirement ever happened is a free-text `skos:changeNote` — which `adr/0042`
  deliberately keeps, and which carries no date, no author, and no machine-readable indication
  that it is about a retirement at all. The fact survives the export; every governance attribute
  of it does not.
- **Opened:** iteration 45

### `openbiz split` counts mapping statements where it means mapped resources, and reports one link as two
- **Kind:** defect, in an item already checked off
- **What is proven:** `openbiz deprecate` counts the distinct resources a concept is mapped to,
  because a test written against all five mapping properties failed and was right to: SKOS §10.2
  (S42) makes `skos:exactMatch` a sub-property of `skos:closeMatch`, so the model holds two links
  for one stated `skos:exactMatch`.
- **What is not:** the same fix in `openbiz split`, whose `Unapportioned::mappings` still sums
  `BTreeMap::len` over the properties. A concept with one `skos:exactMatch` and nothing else is
  reported by a split as carrying "2 mapping links into other vocabularies". Reproduced by
  inspection of `crates/openbiz-skos/src/split.rs`, not yet by a test.
- **What would close it:** the two-line change `deprecate` already carries, plus the failing test
  first. It is not folded in here because fixing an already-checked item while passing through is
  what the one-item rule refuses.
- **Opened:** iteration 45

### `StatusBound::DEFAULT` is the sixth unmeasured constant in this crate
- **Kind:** unmeasured judgement
- **What is proven:** the bound stops an unbounded set, and hitting it produces a refusal rather
  than a wrong answer — a truncated scan cannot establish that a concept is *not* already retired,
  and every refusal in `CoreModel::deprecate` rests on that absence. Tested by driving the bound to
  one.
- **What is not:** that 1 000 is a number about anything. The reasoning in the doc comment is
  honest and thin: a concept superseded by a thousand others is a corrupt graph, and the constant
  exists to stop one exhausting memory rather than to describe a real vocabulary.
- **Note:** `WalkBound`, `PathBound`, `SearchBound`, `SlugBound`, `ReferenceBound`, and now this.
  Six constants, one measured (`PathBound`, iteration 40). The recurrence is the finding.
- **Widened at iteration 47.** `ReinstatementScan` reuses the same constant as a cap on **every**
  status statement it holds about one resource — markers, unreadable markers and replacements —
  and not only the replacements the field is named for. So one unmeasured number now guards two
  scans with different contents, and the second one holds statements rather than counting them,
  which is the more expensive of the two. Recorded here rather than as a seventh entry, because
  duplicating the entry would hide that it is the *same* number doing more work.
- **Opened:** iteration 45

### Nothing about `openbiz deprecate` is measured on a large vocabulary
- **Kind:** partial-coverage
- **What is proven:** correctness on fixtures of a handful of concepts, in-process and against the
  real binary on disk.
- **What is not:** cost. A deprecation reads the vocabulary once for the model, once more for the
  status scan, and twice more for `newly_broken` — four passes — and then walks every collection in
  the model looking for one that lists the concept, which is linear in collections and in their
  members. It also runs `elsewhere` across every vocabulary in the store, twice when the
  replacement is external.
- **Note:** the eighth unmeasured cost in this crate. Recorded again rather than merged into the
  others because each command's shape differs; the *pattern* is proposed as one measurement task.
- **What would close it:** the command run against the 100k and 1M generated vocabularies
  `adr/0013` and `adr/0024` already build, with wall-clock and peak memory recorded.
- **Opened:** iteration 45

### `openbiz notes` and `openbiz mappings` do not say a resource is retired
- **Kind:** partial-coverage
- **What is proven:** the five commands that browse or search a vocabulary — `tree`, `ancestors`,
  `paths`, `search`, `inspect` — all read `owl:deprecated` and mark what they print, each with a
  test against the real binary.
- **What is not:** the two commands that report one named resource. `openbiz notes <graph>
  <resource>` prints a retired concept's documentation, and `openbiz mappings <graph> <resource>`
  prints what it is joined to, with nothing to say either resource is obsolete. `openbiz mappings`
  is the sharper of the two: it is the command that answers "what does this concept correspond to
  elsewhere", and a mapping *to* a retired concept in another vocabulary is exactly the thing a
  governance function needs told.
- **Note:** not an oversight of principle — both go through `crate::inspect::read`, and the seam
  that carries the index is `read_with_retirements` beside it. Switching them is small. It was left
  out because `adr/0041`'s decision was taken for *browse* paths, and what a per-resource report
  should say about a retired resource it was asked about directly is a slightly different question
  from what a list should say about one it happens to contain.
- **What would close it:** both commands on the shared seam, with the full account at the top —
  they are per-resource reports, so the focus rule in `adr/0041` §4 applies rather than the list one.
- **Opened:** iteration 46

### No read command can be asked for current concepts only
- **Kind:** partial-coverage
- **What is proven:** every read command shows a retired concept and marks it, and a test asserts
  that a vocabulary retiring nothing reads exactly as it did.
- **What is not:** the other half of the need. `adr/0041` argues at length that hiding must not be
  the *default*; it does not argue that hiding should be impossible, and a curator building a new
  branch of a large thesaurus has a real reason to want a tree or a search with the obsolete terms
  out of the way. Today the only way to get one is to read past the marks.
- **Note:** it is the plan item below the one this iteration closed, so this entry exists to keep
  the gap visible rather than to add work. The hard part is not the flag: it is what a *tree* does
  when a retired concept has current concepts below it, where dropping the branch would lose them
  and keeping the node contradicts the flag.
- **What would close it:** the plan item, with the tree case decided explicitly rather than by
  whatever falls out of a filter.
- **Opened:** iteration 46

### Nothing reads the retirement marker at scale, and `inspect`'s section walks the hierarchy again
- **Kind:** partial-coverage
- **What is proven:** correctness on fixtures of a handful of concepts, in-process and against the
  real binary on disk. The index itself is cheap by construction: one `BTreeMap` insertion per
  `owl:deprecated` or `dcterms:isReplacedBy` statement, in a pass over the store that already ran.
- **What is not:** cost, in two places. `openbiz inspect`'s retirements section calls
  `CoreModel::children` once per retired concept, so a vocabulary that has retired a large fraction
  of itself — a migration is exactly that — pays a walk per retired concept on every inspect. And
  `openbiz search` now calls `status::explain` per hit, which follows every recorded replacement.
  Both are bounded by the model, and neither has a number against it.
- **Note:** the ninth unmeasured cost in this crate, and the second one in a *read* path rather
  than a write. The proposal to replace the whole run of them with measurements is in
  `docs/PROPOSED.md` from iteration 45 and is unpromoted.
- **What would close it:** `openbiz inspect` and `openbiz search` run against the 100k and 1M
  generated vocabularies `adr/0013` and `adr/0024` already build, with a large fraction of the
  concepts retired, and wall-clock recorded.
- **Opened:** iteration 46

### A retired concept is still a scheme's top concept and this build only counts it
- **Kind:** partial-coverage
- **What is proven:** `openbiz inspect` counts how many retired concepts are still a scheme's top
  concept, and the count is tested end to end against the real binary.
- **What is not:** anything a browse would do about it. A `skos:hasTopConcept` from a live scheme to
  a retired concept means the scheme's entry point — the first thing a user of that vocabulary
  sees — is a term nobody should use. `openbiz tree` marks it if you ask about it; nothing walks
  *down from a scheme*, because this build has no "show me the scheme" command at all, so the case
  is counted where it can be counted and displayed nowhere.
- **Note:** it is the same shape as the entry from iteration 45 about `skos:topConceptOf` being
  propagated in a direction chosen rather than read: both are the absence of a command that starts
  at a scheme. One gap, arrived at from a third side.
- **What would close it:** a scheme-level read path, which Phase 3's concept tree needs anyway.
- **Opened:** iteration 46


### Nothing about `openbiz reinstate` is measured on a large vocabulary
- **Kind:** partial-coverage
- **What is proven:** correctness on fixtures of a handful of concepts, in-process and against the
  real binary on disk — including the whole-graph claim that a reinstatement restores the
  vocabulary letter for letter except the change notes, checked by comparing three `openbiz backup`
  outputs rather than by asking the code what it did.
- **What is not:** cost. It reads the vocabulary once for the model *and its retirements*, once
  more for the status scan, and twice more for `newly_broken` — four passes, the same shape as a
  deprecation — and then runs `elsewhere` across every vocabulary in the store. The surroundings
  section additionally scans the whole `Retirements` index for resources naming this one as their
  replacement, which is linear in the retirements the vocabulary holds and is therefore most
  expensive in exactly the migration case where it matters.
- **Note:** the tenth unmeasured cost in this crate. The proposal to replace the whole run of them
  with measurements has been unpromoted since iteration 45.
- **What would close it:** the command run against the 100k and 1M generated vocabularies
  `adr/0013` and `adr/0024` already build, with a large fraction of the concepts retired, and
  wall-clock and peak memory recorded.
- **Opened:** iteration 47

### An `owl:deprecated` this build cannot read is handled only in a unit test
- **Kind:** partial-coverage
- **What is proven:** `CoreModel::reinstate` leaves an `owl:deprecated` whose object is not read as
  `true` — `"false"`, an IRI, a language-tagged literal — exactly where it is, and reports it in
  `Reinstatement::unread` so the report can say what was not touched and why (`adr/0042` §5).
  Tested in the domain crate against a hand-built statement.
- **What is not:** that the path survives a real import. Nothing here has pushed
  `owl:deprecated "false"^^xsd:boolean` through Oxigraph's Turtle parser and out again, so the
  lexical form the store hands back — and therefore whether `says_true` reads it the same way
  after a round trip through the store as it does from a fixture — is untested. The same doubt
  applies to `says_true`'s leniency about a plain `"true"`, which has always been tested from
  fixtures only.
- **What would close it:** a binary-level test that imports a vocabulary carrying both spellings of
  the marker and an unreadable one, and asserts what `openbiz reinstate` removes and what it names.
- **Opened:** iteration 47

### Nothing takes back a retirement in bulk, and a reversed migration is one command per concept
- **Kind:** partial-coverage
- **What is proven:** one resource, one command, one candidate — and the report tells the operator
  which of its neighbours are still retired, so the next command is at least discoverable.
- **What is not:** the case the deprecation lifecycle was built for. A migration retires a large
  fraction of a scheme; abandoning that migration means taking back every one of those retirements,
  and there is no way to do it but to run the command once per concept, each producing its own
  candidate for a reviewer to approve separately. `openbiz move` has the same shape and solved it
  by acting on a subtree; nothing here does.
- **Note:** this is the mirror of iteration 46's "still uncertain" about whether *show and mark*
  survives a vocabulary that has retired most of itself. Both are the sparse-case assumption
  showing through, from opposite ends of the lifecycle.
- **What would close it:** either a subtree or query-scoped form of the command, or a recorded
  decision that one-at-a-time is right because each retirement was its own decision. Proposed in
  `docs/PROPOSED.md`, unpromoted.
- **Opened:** iteration 47

### data.world's catalog may be built on our own standards surface, and we could not read the source
- **Kind:** environment-limited
- **What is proven:** nothing, by test — a research finding, recorded here because it would change a
  competitive conclusion and an `adr/0003` connector design if true.
- **What is claimed:** that data.world's catalog knowledge graph is built from **DCAT** (catalog),
  **Dublin Core** (metadata), **SKOS** (glossaries and thesauri) and **PROV** (provenance and
  lineage). If that holds it is very nearly `CLAUDE.md` §2's own standards surface, which would make
  data.world the one catalog vendor that is a **standards-story competitor** rather than a discovery
  target — the opposite of how `adr/0003` currently frames it.
- **What is not:** any primary source. The claim traces to a search-engine summary of a data.world
  blog post (Juan Sequeda, Principal Scientist, dated 2022-02-04). **Two fetch attempts at
  iteration 50 returned the page's headline, byline and footer but not the article body**, and a
  second data.world article fetched in full contains no such statement. So we have an attribution
  and no readable text behind it.
- **Why it is filed rather than dropped:** the same pass found a secondary source asserting Collibra
  supports SKOS, which Collibra's own documentation contradicts. That is direct evidence that
  aggregated summaries about this vendor class are unreliable in **both** directions, so this one
  should neither be repeated nor assumed false.
- **What would close it:** a human — or any fetch that renders the post body — reading the article
  and confirming or refuting the four-vocabulary composition. Alternatively data.world's own public
  ontology documentation, which was not located this pass.
- **Opened:** iteration 50 (product-owner pass)
