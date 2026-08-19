# OpenBiz — product charter & engineering constitution

This file is re-read at the start of **every** autonomous iteration. It is the standing brief.
If a decision here conflicts with something you infer from the code, **this file wins** — or you
raise an ADR to change it. Never silently diverge.

---

## 1. What we are building

**OpenBiz** is a self-hosted, standards-native platform for enterprises to **centralise, author,
and govern** taxonomies, ontologies, and thesauri.

The buyer is a large enterprise — typically a data-governance, knowledge-management, or
architecture function in a regulated industry (finance, healthcare, pharma, defence, insurance,
public sector). They already know what SKOS is. They are not the audience for a toy.

We are going directly at **PoolParty** and **metaphactory** (both now under Graphwise, alongside
Ontotext GraphDB), **TopBraid EDG**, and **Protégé / WebProtégé**. The open-source incumbent is
**VocBench 3**.

### The wedge — why anyone switches

Research on the incumbents (see `docs/COMPETITIVE.md`) shows their weaknesses are consistent and
structural, not cosmetic. Each is one of our pillars:

| Their weakness | Our commitment |
|---|---|
| Heavyweight JVM + external triplestore + app server to stand up | **One binary.** No JVM, no external DB, no app server. Download, run, done. |
| Steep learning curve; needs trained specialists | **Intuitive enough for a subject-matter expert** with no RDF training to make their first correct edit unaided. |
| Very high mandatory consulting fees to implement | **Zero-consultant install.** If a deployment needs a services engagement, we have failed. |
| Bugs with no public roadmap, no visibility into fixes | **The roadmap is the repo.** Backlog, decisions, and known gaps are public and in git. |
| Dated, dense, joyless UI | **Visually stunning and modern.** Design is a feature, not a coat of paint. |
| Governance bolted on after the fact | **Governance is the substrate** — review, approval, provenance, and versioning are in the core model. |
| Proprietary, opaque change history | **GitHub-native.** Vocabularies are code: branches, PRs, reviewable diffs, CI validation. |
| No guidance — the tool assumes you already know how to build an ontology | **The method is in the product.** A guided lifecycle always answers "where am I, what next, what is blocking me", and a Solution Advisor routes you to the right artifact type before you start. See `docs/METHODOLOGY.md`. |
| "AI features" with no provenance and no way to refuse them | **Assistance is optional, auditable, and never writes.** Agents emit proposals a human approves; every call is a logged egress event; the default provider is none. See `adr/0002`. |
| Easy to create a tenth overlapping vocabulary, hard to find the nine that exist | **Discovery precedes creation.** Reuse, mapping, and extension rank above creating new, which requires a recorded justification. See `adr/0003`. |

### Non-negotiables

These are not up for iterative renegotiation. Changing one requires an ADR **and** human sign-off
via `docs/PROPOSED.md`.

1. **Self-hosted only.** No cloud dependency, no phone-home, no license server. Must run fully
   air-gapped. The customer hosts, the customer owns the data.
2. **Single binary.** The server, the UI assets, and the store ship as one executable plus a data
   directory. Adding a required external service is a charter violation.
3. **Standards-first.** We implement W3C/ISO specs; we do not invent proprietary substitutes for
   things that are already standardised. Where we extend, we extend *additively* and the artefact
   must still round-trip through a standards-compliant tool.
4. **Apache-2.0 open core.** See §5 — the licence rules bind every dependency you add.
5. **Lightweight.** Cold start under a few seconds, modest memory at rest. Every dependency is a
   liability; justify it.
6. **Assistance is optional and never authoritative.** No core capability may require an LLM, and no
   model output may reach a vocabulary without human approval. Every LLM-assisted path has a working
   manual path; an air-gapped deployment loses assistance and nothing else. Sending vocabulary
   content to an external provider is a **data-egress event** — it is opt-in, per-vocabulary,
   audited, and refusable.
7. **Reuse outranks creation.** Discovery runs before creation, not as a feature the user must
   remember. Creating something new when something existing would serve requires a recorded
   justification. A tool that makes new vocabularies cheap and existing ones invisible is a silo
   generator, which is the problem we exist to solve.

---

## 2. Standards surface

Treat this as the conformance target. Items marked *(target)* are not yet built — they are the
backlog, not a claim.

- **RDF 1.1** core; **RDF 1.2** where Oxigraph's `rdf-12` feature allows.
- **SPARQL 1.1** Query, Update, Federated Query, Graph Store Protocol.
- **SKOS** and **SKOS-XL** — the primary authoring model, including the SKOS integrity conditions.
- **OWL 2** — DL where we can, **EL** and **RL** profiles as the practical reasoning targets.
- **SHACL** — Core and SPARQL-based constraints; our validation and governance-rule substrate.
- **ISO 25964-1/-2** thesaurus model and its documented SKOS/SKOS-XL mapping; **ANSI/NISO Z39.19**
  best-practice checks. These ship as *rule packs*, expressed in SHACL.
- **PROV-O** — the vocabulary for our audit trail. Not a bolt-on log; the actual provenance model.
- **DCAT 3** — catalogue interop.
- **OntoLex-Lemon** *(target)* — richer lexical modelling, VocBench parity.

Serialisations: Turtle, N-Triples, N-Quads, TriG, RDF/XML, JSON-LD. Plus pragmatic enterprise
import/export: ISO 25964 XML, MADS/RDF, SKOS-shaped CSV/Excel.

---

## 3. Architecture

**Backend** — Rust. Axum HTTP. **Oxigraph embedded** as the RDF store (no external triplestore).
**Frontend** — React + TypeScript + Vite, compiled and **embedded into the binary** via `rust-embed`.

```
Cargo.toml                 workspace root
crates/
  openbiz-server/          binary entrypoint; Axum, routing, config, embedded UI assets
  openbiz-store/           Oxigraph wrapper: named-graph model, transactions, backup/restore
  openbiz-skos/            SKOS + SKOS-XL domain model, concept tree, integrity conditions
  openbiz-owl/             OWL 2 model and IO; Reasoner trait; EL + RL engines
                           (the model/IO dependency is undecided — see BLOCKED.md)
  openbiz-validate/        SHACL: Validator trait, rule packs (ISO 25964, Z39.19)
  openbiz-lifecycle/       methodology packs, project state, gate evaluation, Solution Advisor
  openbiz-llm/             LlmProvider trait, providers, agents, proposal model
  openbiz-discovery/       DiscoveryProvider trait, connectors, matching, vocabulary registry
  openbiz-git/             vocabulary-as-code: serialise to Turtle, branch/PR, GitHub API
  openbiz-api/             shared HTTP/JSON types, OpenAPI generation
tools/
  openbiz-llm-shim/        DEV ONLY: OpenAI-compatible HTTP facade over the Claude CLI.
                           Never shipped in the product binary; excluded from release builds.
ui/                        React + TS + Vite frontend
docs/                      charter satellites, ADRs, and the loop's ledgers
```

### Engine dependencies are behind our own traits

The Rust semantic-web ecosystem is younger than the JVM's. That is an accepted, deliberate cost of
the single-binary commitment — but it must not become a trap. **Never call a third-party engine
directly from application code.** Every one sits behind a trait we own — `Reasoner`, `Validator`,
`LlmProvider`, `DiscoveryProvider` — so a crate or vendor that stalls, changes terms, or proves
wrong can be swapped without touching callers. The rule applies with most force to `LlmProvider`,
where the vendor landscape moves fastest and lock-in is most expensive.

Current candidates, none yet load-bearing:
- `oxigraph` — store and SPARQL. **Known risk:** query evaluation is explicitly not yet optimised
  upstream. Benchmark before depending on it for large-vocabulary paths.
- ~~`horned-owl` — OWL 2 data model and IO (RDF/XML, functional syntax).~~ **Ruled out: LGPL-3.0**,
  which §5 forbids in the core. The replacement is an open commercial decision, not a spike — see
  `docs/BLOCKED.md`.
- `whelk-rs` — OWL **EL** reasoner. No Rust OWL 2 **DL** reasoner is mature enough to depend on
  (`rustdl` exists and is Apache-2.0, but is far below the adoption we would need); EL + RL is our
  realistic target. Also **not published to crates.io**, which `deny.toml` refuses today.
- `oxirs-shacl` / `shacl_validation` — SHACL. **Unproven for our purposes — spike before adopting.**

Adopting any of these as load-bearing requires a spike task and an ADR recording what was measured.

### Design for assistability, from the first phase

LLM agents arrive in Phase 10, but the shape they need must exist long before that — and it costs
nothing to build it correctly the first time.

**Any path that changes a vocabulary takes *candidates*, not just direct writes.** A candidate is a
proposed change carrying its provenance, its source, and a confidence where one is meaningful, which
a human reviews before it lands. This is the same shape whether the candidate came from a CSV
import, a discovery match against another vocabulary, a bulk edit, or an LLM agent.

Build it once, in Phase 2, and Phase 10 slots in behind an existing seam. Build direct writes now
and every import, discovery, and agent path has to be retrofitted later. So: when you implement
something that mutates a vocabulary, ask whether a machine might one day propose that change, and
if so, put the candidate-and-review seam in now.

This is not a licence to build LLM code early with no caller — that is exactly the "built but no
production caller" failure of §4. It is an instruction about **interface shape**, not about adding
functionality.

### Explainability is a first-class feature

Every inference, validation failure, and auto-applied rule must be able to answer **"why?"** with a
human-readable derivation. The incumbents are weak here and it is the single most requested thing
from governance teams defending a decision to an auditor. Never add an inference path that cannot
explain itself.

---

## 4. Definition of done

An item is done when **all** hold. No exceptions, no "will wire it up next iteration".

1. It works end to end and has a **real production caller** — not just a tested function. Code that
   exists but nothing invokes is *not* done; it is an entry in `docs/UNTESTED.md`.
2. Tests cover the behaviour, including the failure paths. `cargo test` and the UI suite are green.
3. `cargo clippy -- -D warnings` and `cargo fmt --check` pass.
4. Anything user-facing is reachable in the UI and keyboard-navigable.
5. Any standards claim is backed by a test against the spec's own examples or test suite.
6. Docs updated when behaviour changed.

**Honesty over green.** A truthful red is worth more than a green you engineered by weakening the
test. Never delete or loosen a failing assertion to make a build pass — fix the code, or record the
gap in `docs/UNTESTED.md` and move on. Never claim a standard is supported when only the happy path
is. Partial support is fine and normal; *misreporting* it is not.

---

## 5. Licensing — binding constraint on dependencies

Open core, **Apache-2.0**. The core is Apache-2.0; enterprise features (SSO, advanced governance,
audit) may later be a separately-licensed layer, so the core must stay cleanly relicensable.

- **Permitted:** MIT, Apache-2.0, BSD-2/3, ISC, MPL-2.0 (file-level copyleft, isolated in its own
  crate), Unicode, Zlib.
- **Forbidden in the core:** GPL, LGPL, AGPL, SSPL, and any non-commercial or source-available
  licence. This rules out Blazegraph and Virtuoso OSS — no real loss, since Oxigraph, RDF4J, and
  Jena are all permissive.
- Every new dependency gets a licence check. `cargo deny` enforces this in CI. If you add an
  *optional* dep and CI fails on licence, **remove the dep** — do not weaken the policy.
- **If a dependency we genuinely cannot avoid (Oxigraph and its transitive tree is the standing
  case) carries a licence that is merely *unlisted* rather than forbidden**, that is a decision, not
  a wall. Judge it: if it is permissive in substance (`Unicode-DFS-2016`, `BSD-*`, `OpenSSL`,
  `BSL-1.0`, `Zlib` and similar), add it to `deny.toml` **in the same commit as an ADR recording
  what it is and why it is compatible with open core**. If it is copyleft — GPL, LGPL, AGPL, SSPL,
  or source-available — the answer is still no: record it in `BLOCKED.md` and stop, because that
  one is a commercial decision a human has to make.
  Never add a licence to the allow list without the ADR. A silent widening is how this policy dies.
  **The worked example of the second branch is `horned-owl`**, which this file named as the OWL 2
  candidate until iteration 25 found it is LGPL-3.0. It is in `BLOCKED.md`, unresolved, and it is
  there rather than in `deny.toml` because that is what the rule above requires.

---

## 6. Conventions

**Git.** One branch per backlog item: `item/<phase>-<short-slug>`. Open a PR, let CI run,
auto-merge on green. Never commit directly to `main`. Never force-push `main`. Never merge red.

**Commits.** Imperative subject under ~72 chars, describing behaviour change, not file churn.

**Rust.** Edition 2021+. `thiserror` for library errors, `anyhow` at the binary boundary. No
`unwrap()`/`expect()` outside tests and startup. `tracing` for logs, never `println!`.

**TypeScript.** Strict mode. No `any` without a comment justifying it.

**Testing.** Unit tests beside the code; integration tests in `tests/`. Every bug fix starts with a
failing test that reproduces it. Standards conformance uses the spec's published test suites where
they exist.

**Secrets.** Never commit tokens, keys, or customer data. No real vocabulary data in fixtures
without a clear licence.

---

## 7. The ledgers — how the loop keeps itself honest

Four files in `docs/`. They exist because an autonomous loop that only ever reports success
degrades silently; these are where inconvenient truths go.

- **`BUILD-PLAN.md`** — the phased backlog and the burn-down. `- [ ]` / `- [x]`.
- **`UNTESTED.md`** — things built but not proven, or proven only narrowly. Write here **the moment**
  you notice, not later. "Built but no production caller" belongs here.
- **`BLOCKED.md`** — work that cannot proceed and precisely what would unblock it.
- **`PROPOSED.md`** — work you believe is needed but did not authorise yourself. **You do not
  promote your own proposals into the plan.** A human does, via `/openbiz-status`. This is the
  brake on autonomous scope creep, and it only works if you actually use it.
- **`LOOP-LOG.md`** — one entry per iteration: what changed, what you learned, and an explicit
  **"still uncertain"** line. That last line is not optional and must not be padding.

**Date every ledger entry with an explicit offset**, e.g. `2026-08-20 (NZST, UTC+12)` or a plain
UTC date said to be UTC. This loop runs across midnight in a UTC+12 zone, so a bare date is
ambiguous by up to thirty-six hours between two entries labelled the same day — and an entry a
reader must put in order is exactly the thing that must say which clock it means. Product-owner
instruction, `FEEDBACK-LOG.md` 2026-08-20; the same rule binds provenance timestamps in the
product, where `adr/0047` implements it.

---

## 8. Out of scope for the loop

Do not attempt these autonomously; they need a human or infrastructure the loop does not have.

- Anything requiring a real enterprise IdP, a purchased certificate, or a paid third-party account.
- Publishing releases, pushing to registries, or any outward-facing distribution.
- Pricing, licensing text changes, trademark, or other commercial decisions.
- Load testing that needs hardware beyond this machine — record the intent in `UNTESTED.md`.
- Anything that would require weakening §1's non-negotiables or §5's licence policy.

When you hit one of these, write it to `BLOCKED.md` or `PROPOSED.md` and move to the next item.
Never work around a constraint by lowering it.
