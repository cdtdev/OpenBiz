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

## Backup and restore

```sh
openbiz backup /backups/openbiz-$(date +%F).nq   # write the whole store to one file
openbiz restore /backups/openbiz-2026-08-19.nq   # rebuild an empty store from one
```

A backup is **N-Quads**: every vocabulary plus OpenBiz's own graph registry, in a W3C
Recommendation that any conforming RDF tool can read. It is not a snapshot of our storage engine,
so it stays readable if OpenBiz changes what it stores data in — and it is line-based, so it
`grep`s, `diff`s between two days, and compresses well.

Both commands need the store to themselves, so **stop the server first**. The store's location
comes from the same configuration the server uses (`OPENBIZ_DATA_DIR`, or `data_dir` in
`openbiz.toml`), and the commands log which one they used.

What they refuse, and why:

- **A backup never overwrites an existing file.** The file most likely to be in the way is the last
  good backup.
- **A restore needs an empty store** — restore into a fresh data directory and point the server at
  that. A restore replaces a store; merging one into a populated store would interleave two
  histories with no way to separate them afterwards.
- **A restore refuses anything that would not open afterwards**: a file with no store format stamp
  (most often an *export* of one vocabulary rather than a backup of a store), a stamp from a newer
  build, statements in a graph the file's own registry does not list. All of it is checked inside
  one transaction, so a refused restore leaves the target store exactly as it was.

Exit status is `0` on success, `1` if the operation failed, and `2` if the arguments were not
understood — so a wrapper script can tell "retry this" from "you typed it wrong". `openbiz help`
prints the full usage.

There is **no online backup yet**: see [`docs/adr/0015`](docs/adr/0015-backup-and-restore.md) for
why these are commands rather than HTTP endpoints, and `docs/PROPOSED.md` for the authenticated
endpoint that would remove the need to stop the server.

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
