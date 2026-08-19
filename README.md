<div align="center">

# OpenBiz

**Centralise, author, and govern your taxonomies, ontologies, and thesauri — in one binary.**

Standards-native knowledge organisation for the enterprise. Self-hosted. No JVM. No external
database. No consultants.

</div>

---

> **Status: pre-alpha, under active development.** The architecture and backlog are real; the
> product is not built yet. [`docs/CAPABILITIES.md`](docs/CAPABILITIES.md) is what it does today,
> including what it does not; [`docs/BUILD-PLAN.md`](docs/BUILD-PLAN.md) is the burn-down. Nothing
> below is claimed as working unless it is checked off there.

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

**Stated plainly:** no Rust OWL 2 **DL** reasoner is mature enough for us to depend on. Work does
exist — `rustdl` (Apache-2.0, MaastrichtU-IDS) is actively developed — but nothing in that space is
near the adoption, age, or stability at which we would make it load-bearing, so our reasoning target
is **EL and RL**. That covers the large majority of enterprise ontologies — SNOMED CT and the Gene
Ontology are both EL — but it is a real gap against Protégé with HermiT for expressively DL
ontologies. We would rather say so here than have you discover it during an evaluation. The survey
behind this, with versions and download counts, is in
[`docs/COMPETITIVE.md`](docs/COMPETITIVE.md).

**Also stated plainly:** SPARQL **Federated Query** (`SERVICE`) is listed above as a conformance
target and is **deliberately not compiled in today**. The store is built with the embedded engine's
HTTP client disabled, so the binary carries no code path that can open an outbound connection —
which is what makes the air-gapped claim structural rather than a promise. Federation will return
with an explicit, per-deployment egress control, the same shape the LLM providers get. Everything
else in that list is a target tracked in [`docs/BUILD-PLAN.md`](docs/BUILD-PLAN.md), not a claim;
see [`docs/adr/0006`](docs/adr/0006-embedded-store.md).

## Architecture

Rust backend (Axum) with an **embedded Oxigraph** RDF store. React + TypeScript frontend, compiled
and embedded into the binary. Third-party reasoners and validators sit behind traits we own, so an
immature dependency can be swapped without touching application code.

See [`CLAUDE.md`](CLAUDE.md) for the full charter and [`docs/`](docs/) for architecture decisions.

## What works today

OpenBiz is pre-alpha. What exists is a working command line over a real embedded RDF store, and a
minimal web interface that lists your vocabularies and exports them.

```sh
openbiz inspect  <graph>            # what does this vocabulary hold, in SKOS terms, and why?
openbiz integrity <graph>           # which SKOS integrity conditions does it satisfy?
openbiz search   <graph> <text>     # find concepts by a word, not by an IRI
openbiz tree     <graph> <concept>  # what is below it, and beside it
openbiz merge    <graph> <dup> <survivor>   # propose merging two concepts, references and all
openbiz approve  <id>               # apply a proposed change, recording who approved it
openbiz backup   <file>             # write the whole store to one N-Quads file
openbiz help                        # all 24 commands
```

Two things run through all of it. **Every inference is printed with its derivation** — the statement
it followed from and the clause of the SKOS Reference that licensed it — because a governance team
defending a decision to an auditor cannot say "the tool says so". And **nothing writes to a
vocabulary directly**: every change is a *candidate* staged in a named graph of its own, exportable
and queryable before it lands, approved inside the transaction that records the approver. That is
the same seam a discovery match or an LLM proposal will use.

**The honest, complete answer to "what does this actually do?" — including everything it does not —
is [`docs/CAPABILITIES.md`](docs/CAPABILITIES.md).** It is kept as prose and rewritten as the
product changes, so it does not drift into a changelog. The known gaps are in
[`docs/UNTESTED.md`](docs/UNTESTED.md), and the burn-down is
[`docs/BUILD-PLAN.md`](docs/BUILD-PLAN.md).

There is **no authentication yet**, which is why there is no write endpoint over HTTP and why you
should not put a vocabulary you care about in it.

## Development

Requires a recent stable Rust toolchain and Node.js.

The embedded store builds RocksDB from source, so the **first** build also needs a C++20 compiler
and `libclang` (for `bindgen`). On Debian/Ubuntu:

```bash
sudo apt install build-essential clang libclang-dev
```

This is a *build*-time requirement only. The resulting binary is still a single self-contained
executable with no external service — see [`docs/adr/0006`](docs/adr/0006-embedded-store.md).

```bash
cargo test --workspace
```

Contributions follow the conventions in [`CLAUDE.md`](CLAUDE.md) §6.

## Licence

Apache-2.0. See [`LICENSE`](LICENSE).
