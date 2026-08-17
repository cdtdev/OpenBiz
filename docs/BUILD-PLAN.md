# OpenBiz build plan

The backlog and the burn-down. One `- [ ]` per item; check it off only when it meets the
**definition of done** in `CLAUDE.md` §4 — including having a real production caller.

**Status:** Phase 0 nearly complete — workspace, server, UI, and harness landed. 15 tests passing;
`cargo fmt`, `cargo clippy -D warnings`, and the UI typecheck/build are green.

**Current position:** Phase 0 (Harness & ground). Two items remain: embedding the built UI into the
binary, and branch protection. Phase 1 has not started.

**How to work this plan.** Take the next unchecked item in the current phase. If it turns out to be
much larger than it reads, split it in place into smaller `- [ ]` items and do the first — do not
silently half-do it. If you find work that *should* exist but is not here, it goes in
`docs/PROPOSED.md` for a human to promote. You do not add items to this file yourself.

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
- [x] Cargo workspace with the seven crates from `CLAUDE.md` §3, each compiling
- [x] Axum server with `/healthz`, structured `tracing`, and config from env
- [x] React + TS + Vite UI skeleton that typechecks and builds
- [ ] Embed the built UI into the binary via `rust-embed` and serve it from the server
      > The UI builds to `ui/dist` but **nothing serves it** — the single-binary promise in
      > `CLAUDE.md` §1 is not yet met. Recorded in `UNTESTED.md`.
- [ ] Test that the server serves the embedded UI at `/`
- [ ] Config from a file as well as the environment (only `OPENBIZ_*` env vars are read today)
- [x] GitHub Actions CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, UI build
- [x] `cargo deny` licence policy enforcing `CLAUDE.md` §5, wired into CI
- [ ] Branch protection on `main` so the loop *cannot* merge red — **BLOCKED**, needs GitHub Pro or
      a public repo. See `BLOCKED.md`. The loop watches checks itself as a workaround, which is a
      convention rather than an enforced rule
- [x] Author the iteration driver prompt and the `/openbiz-status` + `/openbiz-control` skills

---

## Phase 1 — RDF core & store

> Enables: everything above it. This is the substrate; get it wrong and every later phase inherits
> the mistake.

- [ ] `openbiz-store`: embedded Oxigraph lifecycle — open, close, durable path, graceful shutdown
- [ ] Named-graph model: one graph per vocabulary, plus a system graph for OpenBiz's own metadata
- [ ] Transactional write API with rollback; concurrent-reader safety under test
- [ ] Parse and serialise Turtle, N-Triples, N-Quads, TriG, RDF/XML, JSON-LD — round-trip tested
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
- [ ] Onboarding: a new user reaches their first correct edit without documentation

---

## Phase 4 — Validation & rule packs

> Enables: governance that is machine-checked rather than hoped for. This is where editorial
> best practice stops being a PDF nobody reads.

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

> Enables: the reason an enterprise buys this rather than using Protégé for free.

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

## Phase 7 — GitHub-native vocabulary-as-code

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

## Phase 8 — Ontology (OWL 2) authoring

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

## Phase 9 — Interop & migration

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

## Phase 10 — Scale & performance

> Enables: surviving procurement. Enterprise vocabularies are large and evaluations are adversarial.

- [ ] Benchmark harness with published, reproducible numbers
- [ ] 1M+ concept vocabulary: tree navigation and search stay interactive
- [ ] Query result streaming and pagination throughout
- [ ] Caching for hot paths — concept tree, search, materialised inferences
- [ ] Cold start under a few seconds with a large store attached
- [ ] Memory ceiling documented and enforced under load
- [ ] Address whatever the Phase 1 Oxigraph benchmark spike found

---

## Phase 11 — Enterprise hardening

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

---

## Phase 12 — Out of loop scope

> Requires a human, real infrastructure, or a commercial decision. `CLAUDE.md` §8. **The loop does
> not attempt these** — they are recorded here so the plan stays honest about total remaining work.

- [ ] Validate SAML and SCIM against a real enterprise IdP
- [ ] Third-party penetration test
- [ ] Load testing on representative server hardware
- [ ] Design review by a professional designer against the Phase 3 goal
- [ ] Usability testing with practising taxonomists
- [ ] Pricing, packaging, and the open-core boundary
- [ ] Trademark and brand
- [ ] Public release and distribution
