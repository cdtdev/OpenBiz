<div align="center">

# OpenBiz

**Centralise, author, and govern your taxonomies, ontologies, and thesauri — in one binary.**

Standards-native knowledge organisation for the enterprise. Self-hosted. No JVM. No external
database. No consultants.

</div>

---

> **Status: pre-alpha, under active development.** The architecture and backlog are real; the
> product is not built yet. See [`docs/BUILD-PLAN.md`](docs/BUILD-PLAN.md) for exactly where things
> stand — including what is missing. Nothing below is claimed as working unless it is checked off
> there.

## Why

Enterprise vocabulary management today means standing up a JVM application server, a separate
triplestore, and a consulting engagement — then training specialists to operate an interface built
in a different decade. The tools are capable and genuinely deep. They are also heavy, opaque, and
expensive to live with.

OpenBiz takes the opposite position on every one of those:

- **One binary.** The server, the interface, and the RDF store ship as a single executable plus a
  data directory. Download it, run it. It works air-gapped.
- **Standards-first.** SKOS and SKOS-XL, OWL 2, SHACL, SPARQL 1.1, ISO 25964, PROV-O, DCAT. We
  implement the specifications rather than inventing proprietary substitutes.
- **Governance as substrate.** Review, approval, provenance, and versioning are in the core model,
  not bolted on. Every inference and every validation failure explains *why*.
- **GitHub-native.** Vocabularies are code. Branches, pull requests, reviewable concept-level
  diffs, and CI validation — with a clean fallback to plain git for air-gapped deployments.
- **The method is in the product.** A guided lifecycle always answers *where am I, what next, what
  is blocking me* — with methodology packs for Z39.19, ISO 25964, NeOn, LOT, SAMOD, METHONTOLOGY,
  and Noy & McGuinness. A Solution Advisor routes you to the right artifact type *before* you build
  the wrong one, and is willing to tell you to build nothing at all.
- **Discovery before creation.** Before you make a new vocabulary or concept, OpenBiz searches what
  your organisation and the public standards bodies already have. Reuse, mapping, and extension rank
  above creating new — which requires a recorded justification. A tool that makes new vocabularies
  cheap and existing ones invisible is a silo generator.
- **Optional, auditable assistance.** LLM agents consolidate notes, draft definitions, and propose
  mappings — but they emit *proposals a human approves*, never writes. Every call is a logged data-
  egress event you can refuse per vocabulary. The default provider is none, and nothing in the core
  requires one.
- **The roadmap is the repo.** The backlog, the architecture decisions, and the known gaps are all
  in git, in the open.

## Standards

RDF 1.1 · SPARQL 1.1 (Query, Update, Federated, Graph Store Protocol) · SKOS · SKOS-XL · OWL 2
(EL and RL profiles) · SHACL · ISO 25964-1/-2 · ANSI/NISO Z39.19 · PROV-O · DCAT 3

Serialisations: Turtle · N-Triples · N-Quads · TriG · RDF/XML · JSON-LD

**Stated plainly:** there is no OWL 2 **DL** reasoner in the Rust ecosystem. Our reasoning target is
EL and RL, which covers the large majority of enterprise ontologies — SNOMED CT and the Gene
Ontology are both EL — but is a real gap against Protégé with HermiT for expressively DL
ontologies. We would rather say so here than have you discover it during an evaluation.

## Architecture

Rust backend (Axum) with an **embedded Oxigraph** RDF store. React + TypeScript frontend, compiled
and embedded into the binary. Third-party reasoners and validators sit behind traits we own, so an
immature dependency can be swapped without touching application code.

See [`CLAUDE.md`](CLAUDE.md) for the full charter and [`docs/`](docs/) for architecture decisions.

## Development

Requires a recent stable Rust toolchain and Node.js.

```bash
cargo test --workspace
```

Contributions follow the conventions in [`CLAUDE.md`](CLAUDE.md) §6.

## Licence

Apache-2.0. See [`LICENSE`](LICENSE).
