# OpenBiz build plan

The backlog and the burn-down. One `- [ ]` per item; check it off only when it meets the
**definition of done** in `CLAUDE.md` §4 — including having a real production caller.

**Status:** Phase 0 is complete — verified by counting the unchecked boxes in the phase, not from
memory of what was left (a product-owner correction after iteration 4; see `FEEDBACK-LOG.md`).
Phase 1 is five items in. The embedded store opens, stamps, and closes an Oxigraph instance inside
the binary; it has a **named-graph model with a real enforcement point** — one graph per
vocabulary, a system graph for OpenBiz's own metadata, `urn:openbiz:` reserved against user
authoring, and a single write choke point that every write passes through — and **that model is now
visible to a user**: `GET /api/graphs` serves the registry, and the interface lists the
vocabularies in it while keeping OpenBiz's own graphs out of the user's list and counted rather
than hidden. **Writes are transactional and serialised**, which closed a real corruption race in
the creation path. **A graph can be got back out**: `GET /api/export` serialises any registered
graph to any of the six syntaxes §2 commits to, the interface offers it per vocabulary with a
format chooser read from the server, and the export carries none of OpenBiz's own bookkeeping.
143 Rust tests and 29 UI tests passing; `cargo fmt`, `cargo clippy -D warnings`,
`cargo deny`, and the UI typecheck/test/build are green. **The single binary is real:** a
`Single binary` CI job deletes `ui/dist` from disk and the release binary still serves the full
interface. **The roadmap is the repo, publicly:** this plan, the ADRs, and the honest gaps in
`UNTESTED.md` are readable by anyone.

**Current position:** Phase 1 (RDF core & store), 6 of 12 items done (the serialisation item
was split in two — see the split note below). Serialisation
landed this iteration: `RdfSyntax` is our own enum over the six syntaxes §2 names, `Store::export_graph`
streams one graph into any of them, and `GET /api/export` serves it with content negotiation, a
404 for a graph that is not registered, and headers that say what the file is. **Next: parse those
same six syntaxes** — deliberately deferred behind the candidate seam or backup/restore, because a
parser's caller is an import and an import mutates a vocabulary (see the split note). Vocabulary
*creation* over HTTP remains deliberately absent — §1.7 requires discovery to run before creation and
`DiscoveryProvider` does not exist until Phase 2 — so `POST /api/graphs` answers 405 rather than
being quietly added.

**How to work this plan.** Take the next unchecked `- [ ]` item in the current phase. If it turns
out to be much larger than it reads, split it in place into smaller items and do the first — do not
silently half-do it. If you find work that *should* exist but is not here, it goes in
`docs/PROPOSED.md` for a human to promote. You do not add items to this file yourself.

**Standing instruction from the product owner** (`FEEDBACK-LOG.md`, 2026-08-18): in *every* phase,
when you notice a place where LLM assistance would materially help — a tedious editorial task, a
judgement needing recall across thousands of concepts, a translation, a mapping, a definition to
draft — write it to `docs/PROPOSED.md` under "LLM assistance opportunities" with the concrete user
problem it solves. By the time Phase 10 arrives its agent list should reflect what was learned
building Phases 1–9, not the guesses made on day one. **Do not pull Phase 10 forward** to service
these notes; recording the opportunity is the whole task.

**Standing instruction from the product owner** (`FEEDBACK-LOG.md`, 2026-08-18): **parity is
failure.** Before building any item, answer *"what do the incumbents do badly here, and what would
be materially better?"* — not "does the incumbent have this" — and write the answer into the item or
the commit. Working a competitor's feature list as a checklist is the specific failure mode; the
question is always what the *user* is trying to accomplish. If the honest answer is "here we can
only match", say so in `docs/PROPOSED.md` rather than shipping parity quietly. This never licenses
scope creep and never overrides `CLAUDE.md` §1 or §4. The every-25th-iteration product-owner pass
re-reads the charter's wedge table row by row and asks whether what we built is *better* yet, or
merely present.

Phases are ordered by dependency, not importance. Phase 3 (the interface) is deliberately early:
the interface is a core differentiator, and building it late means retrofitting every API to it.

---

## Phase 0 — Harness & ground

> Enables: everything. An autonomous loop with no green baseline and no ledgers cannot tell
> progress from damage.

- [x] Create the private GitHub repo and wire the local remote
- [x] Write the product charter (`CLAUDE.md`)
- [x] Research and record the competitive and standards landscape (`docs/COMPETITIVE.md`)
- [x] Seed this build plan
- [x] Create the loop ledgers: `UNTESTED.md`, `BLOCKED.md`, `PROPOSED.md`, `LOOP-LOG.md`
- [x] Apache-2.0 `LICENSE` and `README.md`
- [x] Cargo workspace with the initial seven crates, each compiling
- [x] Axum server with `/healthz`, structured `tracing`, and config from env
- [x] React + TS + Vite UI skeleton that typechecks and builds
- [x] Research KOS development methodologies and design the methodology engine, LLM integration, and
      enterprise awareness (`docs/METHODOLOGY.md`, ADRs 0001–0003)
- [x] Embed the built UI into the binary via `rust-embed` and serve it from the server
- [x] Test that the server serves the embedded UI at `/`
      > Proven the hard way: a `Single binary` CI job deletes `ui/dist` and `ui/node_modules`, then
      > starts the release binary and curls it. See `adr/0004`.
- [x] Config from a file as well as the environment
      > **Better, not parity:** the incumbents' weakness here is not the file format, it is that a
      > deployment's *effective* configuration is unknowable — spread across layers, with a
      > misspelled key silently ignored. So an unrecognised key is a hard error naming the line and
      > the keys we accept, and every setting carries its provenance: the startup log and the bind
      > failure both say which of the default, the file, or the variable won. See `adr/0005` and
      > `docs/CONFIGURATION.md`.
- [x] GitHub Actions CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, UI build
- [x] `cargo deny` licence policy enforcing `CLAUDE.md` §5, wired into CI
- [x] Branch protection on `main` so the loop *cannot* merge red
      > Unblocked 2026-08-18 by the product owner making the repository **public** — the commercial
      > decision the blocker deferred to a human, taken rather than worked around. Ruleset
      > `main-protection` is active on `main` with `Rust`, `Licence policy`, `UI`, and `Single
      > binary` as required checks, force-push and deletion blocked, and no bypass actors, so the
      > rule binds the owner too. Merging red is now refused by the server rather than only by the
      > loop's own discipline. See the Resolved entry in `BLOCKED.md`.
- [x] Author the iteration driver prompt and the `/openbiz-status` + `/openbiz-control` skills
- [x] UI test runner (Vitest + Testing Library) with a test per `Probe` state, wired into CI
      > Promoted from `PROPOSED.md` by the product owner. **Correction to the promoted text:**
      > `npm test` was not "a no-op that passes silently" — there was no `test` script at all, so it
      > exited 1 with `Missing script: "test"`, and the `UI` CI job never invoked it. The effect was
      > the same (zero UI assertions ever ran) but the mechanism was different, and the loop's own
      > report of a green UI suite was the actual falsehood.
      > **Better, not parity:** a test step that goes green because it found nothing to run is the
      > failure this item exists to prevent, so `passWithNoTests: false` is stated explicitly in
      > `vite.config.ts` and deleting the suite now turns CI red. And the suite was proven to
      > *discriminate* before it was trusted: seven mutations of `App.tsx` — dropping the
      > `response.ok` guard, dropping the `AbortError` swallow, dropping the unmount abort, never
      > leaving the loading state, removing `role="alert"`, probing the wrong endpoint, and blanking
      > the non-`Error` message — were each confirmed to turn the suite red. The first draft of the
      > suite let the `AbortError` mutant live, because the `fetch` stub ignored its `AbortSignal`;
      > that is fixed and is why the stub now rejects on abort the way a real `fetch` does.

---

## Phase 1 — RDF core & store

> Enables: everything above it. This is the substrate; get it wrong and every later phase inherits
> the mistake.

- [x] `openbiz-store`: embedded Oxigraph lifecycle — open, close, durable path, graceful shutdown
      > **Better, not parity:** every incumbent has a triplestore; what they do badly is that it
      > sits in a *separate lifecycle from the application*. Four failure modes follow, and one
      > lifecycle answers all four. The store opens **before** the listener binds, so a store that
      > will not open is a process that never starts rather than one that is "up but useless". A
      > second instance over one data directory is refused in our words — "already in use by another
      > OpenBiz process", naming the configuration layer that chose the path — not a RocksDB `LOCK`
      > errno. `SIGTERM` drains, *then* flushes, and logs `store closed cleanly`, so an operator can
      > tell a graceful stop from a kill. A store from a newer build is refused, never misread.
      > See `adr/0006`.
- [x] Test `Config::load` against a real process environment via a subprocess
      > Closed as a by-product of the store lifecycle item above, which needed a real-process
      > harness anyway: `tests/graceful_shutdown.rs` spawns the binary with a controlled
      > environment and asserts `data_dir` and `bind` both report `$OPENBIZ_*` as their source.
      > No separate iteration was spent on it.
      > Promoted from `PROPOSED.md` by the product owner. `Config::resolve` is tested with an
      > injected environment because `std::env::set_var` is not thread-safe; that leaves the wiring
      > to the real environment provable only from outside the process.
- [x] Named-graph model: `GraphId` and `GraphKind`, the reserved `urn:openbiz:` namespace, the
      system graph, and the graph registry — with every write routed through one guarded choke point
      > **Better, not parity:** every RDF tool has named graphs; what the incumbents do with them is
      > the problem. PoolParty and TopBraid EDG keep project metadata in the same store as the
      > content, so exports carry tool-specific bookkeeping a standards-compliant consumer has to be
      > told to ignore — which is what breaks the round-trip §1.3 requires. VocBench exposes the
      > triplestore's own support graphs to the user, so "which graph does this go in" becomes a
      > question a subject-matter expert is asked and cannot answer. And across all of them inferred
      > triples are commonly materialised where a human can also write, after which "did a person
      > state this or did a reasoner derive it" is unanswerable. Here: one graph per vocabulary,
      > `urn:openbiz:` reserved so a user cannot author into our bookkeeping, an inferred graph's
      > IRI *derived* from its vocabulary rather than chosen, and a single write choke point that
      > refuses a graph the rules say is not directly writable. The registry lives in the system
      > graph and is re-validated on read, so a doctored backup is refused rather than trusted.
      > See `adr/0007`.
- [x] Expose the graph registry over HTTP (`GET /api/graphs`) and in the UI — the **read** half.
      The create half waits on §1.7's discovery-first path, because a "create new" that skips
      `DiscoveryProvider` or records no justification is a charter violation rather than a shortcut
      > Split out of the item above, which was two items wearing one hat: the store model, and
      > exposing it.
      > **Better, not parity:** the endpoint returns the *whole* registry, including OpenBiz's own
      > graphs, and the **UI** is what keeps our bookkeeping out of the user's vocabulary list.
      > Both halves matter and the incumbents get one or the other wrong. VocBench puts the
      > triplestore's support graphs in the same list as the user's content, so a subject-matter
      > expert is asked "which graph does this go in?" and cannot answer. Hiding them in the API
      > instead would make "what is actually in my store?" unanswerable to an operator — the
      > opacity §1 exists to attack. So: `kind` is on the wire, the API never omits a row, the
      > interface shows vocabularies only, and the graphs it holds back are **counted** rather than
      > silently dropped. The empty state — what every new deployment sees — says that reuse
      > outranks creation (§1.7) instead of offering a "New vocabulary" button.
      > `POST /api/graphs` is a 405, not a 404: the registry is deliberately read-only until
      > `DiscoveryProvider` exists. See `adr/0008`.
- [x] Transactional write API with rollback; concurrent-reader safety under test
      > **Better, not parity:** the finding that shaped this item is that **the backend's own
      > transaction does not serialise writers**. Oxigraph 0.5.9's transaction is a snapshot plus a
      > write batch, and commit is an unconditional write of that batch — no conflict detection.
      > Two callers that both read "this IRI is free" both commit. A test written first proved it:
      > eight threads creating one IRI, **all eight succeeded**, and because a graph registered
      > twice makes `Store::graphs` refuse the *whole* registry, one user's mistimed second click
      > took the entire vocabulary list down. The incumbents inherit their triplestore's isolation
      > level and mostly do not say what it is; here it is measured, named in `adr/0009`, and the
      > gap is closed by a write lock we own — proven load-bearing by removing it and watching the
      > race return. Rollback is a **closure**, so the safe outcome is the one a failing caller
      > gets by default rather than one they must remember to ask for; a panic rolls back too, and
      > does not leave the store silently read-only. Nesting is refused rather than deadlocked on.
      > The production caller is store startup: the format stamp and the system graph's registry
      > entry now commit **together**, closing a window where a kill left a stamped store this
      > build reports as inconsistent.
- [x] Serialise a named graph to Turtle, N-Triples, N-Quads, TriG, RDF/XML, and JSON-LD, and
      export it over HTTP and from the interface — proven faithful by re-parsing every syntax and
      comparing the statements back
      > **Better, not parity.** Every incumbent exports RDF; three things they do badly are what
      > this item is actually about. (1) **The export is not what you saw.** PoolParty and TopBraid
      > EDG keep project bookkeeping in the same store as the content, so a consumer has to be told
      > which parts to ignore — the round trip `CLAUDE.md` §1.3 requires. Here a vocabulary export
      > cannot contain our metadata because our metadata was never in the vocabulary; the
      > named-graph model (`adr/0007`) pays for itself, and a test asserts `urn:openbiz:` appears in
      > no export. (2) **Silent lossiness.** Turtle, N-Triples, and RDF/XML have nowhere to record a
      > graph name, so an export in one of them cannot say which vocabulary it is — universally
      > true and universally unmentioned, so users discover it from a re-import that lands in the
      > wrong place. Here `recordsGraphNames` comes from the constant the serialiser branches on, is
      > served to the interface, and is stated before the download; `X-OpenBiz-Graph` carries the
      > identity the payload cannot. (3) **Export is a wizard or a job you come back for**, so it
      > cannot be scripted, scheduled, or diffed in CI. Here it is `GET /api/export?graph=…&format=…`
      > and the interface uses the same URL a runbook would. Also: a graph that does not exist is a
      > 404, never an empty file, and a format we do not have is a 400 naming the ones we do —
      > silently substituting the default is how a caller finds out from their own parser.
      > **Scope, honestly:** this is the serialise half. Parsing is the item below, and the round
      > trip is proven against our own reader, which is fidelity rather than conformance — see
      > `docs/UNTESTED.md`.
- [ ] Parse those same six syntaxes into the store, round-tripped against the serialiser above
      > **Split note (iteration 8).** One item wearing two hats, and the seam between them is a
      > charter constraint rather than convenience. A parser's production caller is an *import*, an
      > import mutates a vocabulary, and `CLAUDE.md` §3 says a change to a vocabulary arrives as a
      > reviewable **candidate** — the seam that is Phase 2's first item. Landing the parser now
      > would mean either code with no caller (§4.1) or a direct-write import to retrofit later,
      > which is the exact failure §3 warns about. It lands with whichever comes first: backup and
      > restore below, which parses N-Quads and touches no vocabulary, or Phase 2's candidate seam.
      > Serialisation has no such dependency — an export is a read.
- [ ] SPARQL 1.1 Query endpoint with all four result formats (JSON, XML, CSV, TSV)
- [ ] SPARQL 1.1 Update endpoint, guarded by authorisation
- [ ] SPARQL Graph Store Protocol
- [ ] **Spike:** benchmark Oxigraph query evaluation at 10k / 100k / 1M concepts. Record real
      numbers in an ADR. Upstream states evaluation is unoptimised — find where that bites us
      *before* we build the UI on top of it
- [ ] **Spike:** characterise Oxigraph's numeric/calendar/duration literal precision limits and
      decide our documented behaviour at the boundary
- [ ] Backup and restore to a single portable file; restore verified against a live store
- [ ] Store-format migration framework — versioned, forward-only, tested on a populated store

---

## Phase 2 — SKOS authoring model

> Enables: the product's core noun. Everything a taxonomist does lands here.

- [ ] **Candidate seam:** every path that mutates a vocabulary takes a *candidate* — a proposed
      change carrying provenance, source, and a confidence where one is meaningful — reviewed before
      it lands. One shape for a CSV import, a discovery match, a bulk edit, and a Phase 10 agent.
      > Added on product-owner instruction (`FEEDBACK-LOG.md`, 2026-08-18), which names it the
      > highest-value near-term work for LLM integration. It is `CLAUDE.md` §3 "design for
      > assistability" made concrete, and it is **interface shape, not new functionality** — do not
      > build agents or an `LlmProvider` behind it. Build this **before** the mutation items below,
      > or every one of them needs retrofitting.
- [ ] SKOS core model: `Concept`, `ConceptScheme`, `Collection`, `OrderedCollection`
- [ ] SKOS-XL labels as first-class resources (required for ISO 25964 fidelity — not optional)
- [ ] Semantic relations: `broader`/`narrower`/`related`, transitive variants, polyhierarchy
- [ ] Labels: `prefLabel`, `altLabel`, `hiddenLabel`, per-language, with the one-preferred-label-
      per-language rule enforced
- [ ] Documentation properties: `definition`, `scopeNote`, `example`, `historyNote`, `editorialNote`
- [ ] Mapping properties: `exactMatch`, `closeMatch`, `broadMatch`, `narrowMatch`, `relatedMatch`
- [ ] All SKOS integrity conditions from the specification, each with a test citing its S-number
- [ ] Concept tree query API: children, ancestors, siblings, paths-to-root, with cycle detection
- [ ] Full-text search across labels with language filtering and prefix/infix matching
- [ ] Concept IRI minting: configurable patterns, collision detection, opaque-vs-readable policy
- [ ] Bulk operations: merge concepts, split a concept, move a subtree, deprecate with replacement
- [ ] Deprecation lifecycle preserving history rather than deleting — auditors need the trail
- [ ] `DiscoveryProvider` trait plus a local-store implementation, wired into concept creation
      > The hook lands here so the creation path is **built around discovery** rather than
      > retrofitted. Enterprise and public sources arrive in Phase 12 (`adr/0003`).

---

## Phase 3 — The interface

> Enables: the differentiator we cannot retrofit. Beauty and clarity here are the product.
> Measured against: could a subject-matter expert with no RDF training make a correct first edit
> unaided?

- [ ] Design system: type scale, spacing, colour with verified contrast, motion, dark and light
- [ ] Application shell: navigation, command palette, keyboard-first interaction throughout
- [ ] Concept tree: virtualised for 100k+ nodes, drag-to-reparent, polyhierarchy made legible
- [ ] Concept detail: inline editing, optimistic updates, conflict detection on concurrent edit
- [ ] Search: as-you-type, ranked, language-aware, keyboard-navigable results
- [ ] Multilingual editing: side-by-side languages, per-language completeness indicators
- [ ] Relationship editor with live cycle and integrity warnings shown *as you type*, not on save
- [ ] Graph visualisation of a concept neighbourhood — readable at 100+ nodes, not a hairball
- [ ] Accessibility: WCAG 2.2 AA, full keyboard operation, screen-reader tested, focus management
- [ ] Empty, loading, and error states designed rather than defaulted
- [ ] Headless-browser smoke test (Playwright) loading `/` from the release binary
      > Promoted from `PROPOSED.md` by the product owner. Transport tests prove the right bytes are
      > served; they cannot prove the app mounts. Shares a harness with the accessibility item above.
- [ ] Onboarding: a new user reaches their first correct edit without documentation

---

## Phase 4 — Validation & rule packs

> Enables: governance that is machine-checked rather than hoped for — **and Phase 7's gates**, which
> are SHACL shapes (`adr/0001`).

- [ ] **Spike:** evaluate `oxirs-shacl` vs `shacl_validation` vs in-house against the W3C SHACL
      test suite. Record coverage, performance, and licence in an ADR before choosing
- [ ] `Validator` trait owned by us; the chosen engine sits behind it (`CLAUDE.md` §3)
- [ ] SHACL Core constraint components, conformance-tested
- [ ] SHACL-SPARQL constraints
- [ ] Validation report model with severity levels, surfaced in the UI at the offending field
- [ ] Rule pack: SKOS integrity conditions
- [ ] Rule pack: ISO 25964 thesaurus conformance
- [ ] Rule pack: ANSI/NISO Z39.19 editorial best practice
- [ ] Custom organisation rule packs, authored in the UI without hand-writing SHACL
- [ ] Validation on write (blocking) and scheduled full-vocabulary sweeps (reporting)
- [ ] **Every violation explains itself**: what failed, which shape, why, and how to fix it

---

## Phase 5 — Reasoning & explanation

> Enables: the "why?" that governance teams must answer to auditors — the incumbents' weakest flank.

- [ ] `Reasoner` trait owned by us, with a null implementation as the default
- [ ] RDFS entailment via forward-chaining materialisation
- [ ] OWL 2 RL rule engine, incremental where possible
- [ ] OWL 2 EL classification via `whelk-rs`, behind our trait
- [ ] Consistency checking with a human-readable account of the inconsistency
- [ ] **Explanation**: every inferred triple can produce its full derivation chain, rendered for a
      non-logician. No inference path may ship without this (`CLAUDE.md` §3)
- [ ] Incremental re-reasoning on edit, fast enough for interactive use
- [ ] Materialised inferences visibly distinguished from asserted facts everywhere in the UI
- [ ] Document the DL gap honestly in user-facing docs — we support EL and RL, not full DL

---

## Phase 6 — Governance & workflow

> Enables: the reason an enterprise buys this rather than using Protégé for free. Also the approval
> path every LLM proposal flows through (Phase 10).

- [ ] Identity and RBAC: roles modelled on VocBench's editorial separation (see `COMPETITIVE.md`)
- [ ] Per-vocabulary and per-scheme permissions
- [ ] Change requests: propose, review, approve, reject, with threaded discussion
- [ ] Editorial workflow states: draft → review → approved → published → deprecated
- [ ] PROV-O audit trail — the real provenance model, not a side log
- [ ] Immutable, exportable audit export for compliance
- [ ] Versioning and named releases of a vocabulary, with diff between any two versions
- [ ] Human-readable concept-level diff: "3 concepts added, 1 relabelled, 1 deprecated"
- [ ] Notifications for review requests and assignments

---

## Phase 7 — Lifecycle & methodology

> Enables: "where am I, what next, what is blocking me" — and routing a newcomer to the right
> artifact type before they build the wrong one. Design in `docs/METHODOLOGY.md` and `adr/0001`.
> Depends on Phase 4 (gates are SHACL) and Phase 6 (roles, states, audit).

- [ ] `openbiz-lifecycle` crate: methodology pack model as a **scenario graph**, not a linear phase
      list — NeOn's nine scenarios branch and recombine, and a linear model cannot express them
- [ ] Pack RDF vocabulary, Turtle loader, and validation *of packs themselves*
- [ ] Project state: bound pack, current phase/scenario, activity completion, history
- [ ] Gate evaluation via SHACL — exit criteria as shapes, failures linked to the **specific
      offending concepts**, never an opaque "not ready"
- [ ] Gate override, role-gated, with a recorded reason surfaced in the audit trail and the Compass
- [ ] Project Compass UI: phase ribbon, next action, blocking criteria, honest progress, and a
      visible marker when a phase was passed by override rather than satisfied
- [ ] Pack: `z39-19-taxonomy` (authority lists and taxonomies)
- [ ] Pack: `iso-25964-thesaurus`
- [ ] Pack: `noy-mcguinness-101` (the seven steps; gentlest on-ramp)
- [ ] Pack: `methontology` (staged, with continuous support activities)
- [ ] Pack: `neon` (nine scenarios; the enterprise default because it is built around reuse)
- [ ] Pack: `lot` (requirements → implementation → publication → maintenance)
- [ ] Pack: `samod` (milestones, modelet, bag of tests — its test cases map onto our SHACL shapes
      and SPARQL competency-question checks)
- [ ] Competency questions as first-class requirements: define, attach, verify via SPARQL
- [ ] Solution Advisor: the diagnostic interview, phrased so a subject-matter expert can answer
      every question without knowing what SKOS is
- [ ] Advisor routing to artifact type + pack + starting template, with stated reasoning
- [ ] Advisor consults discovery **first** and can recommend building nothing (`adr/0003`)
- [ ] Solution Brief: versioned, revisable, diffable record of the decision and rejected alternatives
- [ ] Escalation: authority list → taxonomy → thesaurus, guided migration
- [ ] Escalation: thesaurus → ontology as **guided reinterpretation, never one-click** — a per-concept
      decision that keeps the SKOS vocabulary alongside the ontology rather than replacing it
- [ ] Custom pack authoring so an organisation can encode its own governance standard

---

## Phase 8 — GitHub-native vocabulary-as-code

> Enables: the structural answer to "no visible roadmap, no reviewable history". This is the pillar
> the incumbents cannot copy without changing how they build.

- [ ] Deterministic, diff-friendly Turtle serialisation — stable ordering, canonical formatting
- [ ] Vocabulary ↔ git working tree mapping with a documented file layout
- [ ] Commit on approved change, carrying author attribution and the change request reference
- [ ] Branch per change request; open a PR against the vocabulary repo
- [ ] Render the concept-level diff into the PR body so a reviewer reads meaning, not triples
- [ ] Pull and reconcile external commits; three-way merge with conflict surfacing in the UI
- [ ] Ship a GitHub Action that runs OpenBiz validation on PRs in a vocabulary repo
- [ ] Webhook receiver: external push triggers reimport and revalidation
- [ ] Work against a self-hosted GitHub Enterprise Server, not just github.com
- [ ] Degrade cleanly to plain git with no GitHub — air-gapped customers still get versioning

---

## Phase 9 — Ontology (OWL 2) authoring

> Enables: the "ontologies" half of the pitch. Measured against Protégé, which is the benchmark
> users arrive with.

- [ ] OWL 2 model and IO via `horned-owl`, behind our own boundary
- [ ] Class hierarchy editor with inferred-vs-asserted clearly distinguished
- [ ] Object, data, and annotation property editors with characteristics
- [ ] Class expression builder usable without Manchester syntax fluency — but Manchester available
- [ ] Axiom editor with live consistency feedback
- [ ] Import closure management: `owl:imports`, version IRIs, resolution, and caching
- [ ] Punning and OWL 2 profile validation with a plain-language explanation of profile violations
- [ ] SKOS ↔ OWL bridge: use a taxonomy as an ontology's class scaffold without conflating them

---

## Phase 10 — LLM & agent assistance

> Enables: the acceleration, without becoming a dependency. Design in `adr/0002`.
> **Every item here must degrade cleanly to `NullProvider`.** Depends on Phase 6 for the approval
> path proposals flow through.

- [ ] `openbiz-llm` crate: `LlmProvider` trait and `NullProvider` as the **default**
- [ ] `AnthropicProvider` — Anthropic Messages API
- [ ] `OpenAiCompatibleProvider` — one implementation covering Azure OpenAI, vLLM, Ollama, LiteLLM,
      and gateway-fronted Bedrock, including **local models for air-gapped sites**
- [ ] `tools/openbiz-llm-shim`: dev-only OpenAI-compatible HTTP facade over `claude -p`, excluded
      from release builds and never present in the product binary
- [ ] Prove dev and production exercise the **same code path** — only the base URL differs
- [ ] Proposal model: an agent run emits suggested changes that a human reviews, edits, and approves
      through the Phase 6 workflow. **No path from model output to committed vocabulary.**
- [ ] PROV-O provenance on every proposal: model, prompt template version, timestamp, requesting
      user, inputs, cited sources
- [ ] Per-vocabulary LLM policy: off · local-only · named external provider
- [ ] Egress audit log, plus disclosure in the UI **before the first call**, not buried in settings
- [ ] Prompt templates as versioned git artifacts, not string literals
- [ ] Golden evaluation sets per agent, and a harness the loop can run to catch regressions
- [ ] Agent: **note consolidation** — unstructured notes, glossaries, and spreadsheets into candidate
      concepts with labels, definitions, and proposed relations
- [ ] Agent: candidate term extraction from a document corpus
- [ ] Agent: definition drafting in house style, with sources
- [ ] Agent: near-synonym and duplicate detection
- [ ] Agent: mapping suggestion between vocabularies (feeds Phase 12)
- [ ] Agent: translation drafting for multilingual vocabularies
- [ ] Agent: competency question generation and gap-spotting
- [ ] Agent: change-request impact summary for reviewers
- [ ] Agent awareness of lifecycle position — suggest what the *current phase* actually needs
- [ ] Verify every LLM-assisted path has a working manual path, with `NullProvider` in CI

---

## Phase 11 — Interop & migration

> Enables: displacing an incumbent. Nobody starts empty — the migration path *is* the sale.

- [ ] SKOS import with a dry-run diff before anything is written
- [ ] ISO 25964-1 XML import and export
- [ ] MADS/RDF import and export
- [ ] CSV and Excel import with column mapping and a preview of what will change
- [ ] **Migration importers for PoolParty, TopBraid EDG, and VocBench exports** — the switching path
- [ ] DCAT 3 catalogue export
- [ ] Content negotiation and dereferenceable concept IRIs for publishing
- [ ] SPARQL federation against external endpoints
- [ ] OntoLex-Lemon support (VocBench parity)

---

## Phase 12 — Enterprise awareness & anti-silo

> Enables: the reason a CDO buys this for the *organisation* rather than for one team. Design in
> `adr/0003`. Depends on Phase 2 (mappings) and Phase 11 (import machinery for connectors).

- [ ] `openbiz-discovery` crate: full `DiscoveryProvider` implementations beyond the Phase 2 local hook
- [ ] Discovery on the creation path for **both** vocabulary creation and concept creation —
      asynchronous, never blocking typing
- [ ] Reuse ladder: use · map · extend · fork · create-new, with a recorded justification naming
      what was found and why nothing fitted
- [ ] **Measure that reuse is fewer interactions than creating new.** If it is not, the ladder is
      decoration
- [ ] Federated OpenBiz peer discovery
- [ ] Arbitrary SPARQL endpoint provider
- [ ] Public registry providers: EuroVoc, AGROVOC, LCSH, schema.org, IPTC
- [ ] Connector: SharePoint managed-metadata term store (a major real-world silo source)
- [ ] Connector: Microsoft Purview
- [ ] Connector: Collibra
- [ ] Connector: Alation
- [ ] Connector: DataHub / OpenMetadata
- [ ] Connector: Confluence and wiki glossaries
- [ ] Enterprise vocabulary registry: catalog every KOS in the organisation **including ones OpenBiz
      does not manage** — you cannot de-silo what you cannot see
- [ ] Standing overlap and duplication report across all known vocabularies
- [ ] Consolidation workflow for detected overlaps
- [ ] Lexical and structural matching baseline that works with **no LLM** (recall improves with one)
- [ ] An unavailable source degrades to "source unavailable" and never blocks creation
- [ ] Air-gapped mode: local and peer discovery only, no external calls

---

## Phase 13 — Scale & performance

> Enables: surviving procurement. Enterprise vocabularies are large and evaluations are adversarial.

- [ ] Benchmark harness with published, reproducible numbers
- [ ] 1M+ concept vocabulary: tree navigation and search stay interactive
- [ ] Query result streaming and pagination throughout
- [ ] Caching for hot paths — concept tree, search, materialised inferences
- [ ] Cold start under a few seconds with a large store attached
- [ ] Memory ceiling documented and enforced under load
- [ ] Address whatever the Phase 1 Oxigraph benchmark spike found
- [ ] Discovery and gate evaluation stay interactive on large vocabularies

---

## Phase 14 — Enterprise hardening

> Enables: passing security review, which is where good products die.

- [ ] OIDC authentication
- [ ] SAML 2.0 authentication
- [ ] SCIM user and group provisioning
- [ ] Air-gapped install: one binary, no network calls, documented and verified offline
- [ ] TLS configuration, security headers, CSRF protection
- [ ] Rate limiting and request-size limits
- [ ] Prometheus metrics, structured logs, health and readiness endpoints
- [ ] Automated backup scheduling with restore rehearsal
- [ ] Upgrade path with tested store migrations across versions
- [ ] Threat model documented; dependency and container scanning in CI
- [ ] Admin console: users, roles, backups, system health
- [ ] Show the effective configuration and its provenance in the admin console
      > Promoted from `PROPOSED.md` by the product owner. Depends on Phase 6 authentication —
      > effective configuration is not public. Must show a credential's *source* without its
      > *value* (`CLAUDE.md` §6, secrets).
- [ ] Secrets handling for LLM and connector credentials — never in the store, never in logs

---

## Phase 15 — Out of loop scope

> Requires a human, real infrastructure, or a commercial decision. `CLAUDE.md` §8. **The loop does
> not attempt these** — they are recorded here so the plan stays honest about total remaining work.

- [ ] Validate SAML and SCIM against a real enterprise IdP
- [ ] Enterprise connector credentials and test tenants (SharePoint, Purview, Collibra, Alation)
- [ ] Commercial terms and data-processing agreements for hosted LLM providers
- [ ] Third-party penetration test
- [ ] Load testing on representative server hardware
- [ ] Design review by a professional designer against the Phase 3 goal
- [ ] Usability testing with practising taxonomists — **including whether the reuse ladder's
      justification prompt is genuinely read or merely clicked through** (`adr/0003`)
- [ ] Validation of the methodology packs by practitioners who use those methodologies
- [ ] Pricing, packaging, and the open-core boundary
- [ ] Trademark and brand
- [ ] Public release and distribution
