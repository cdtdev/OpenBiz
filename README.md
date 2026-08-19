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

**A backup from an older build is migrated as it is restored**, in the same transaction that reads
it — so a file this build cannot bring forward restores *nothing* rather than something it would
misread. When that happens the command says so and says why, because "restored 12 000 statements"
looks identical whether or not your data was changed on the way in:

```
restored 4 statements into 2 graphs, from last-year.nq; migrated the store format from version 1
to 2: 0002-register-system-graph (1 → 2): registered the system graph in the graph registry…
```

The same facts are written into the store itself, so a SPARQL query over
`<urn:openbiz:graph:system>` still answers *"what has been done to this store, and when?"* long
after the log line has gone. Upgrades are **forward-only**: there is no downgrade, and the honest
way back is the backup you took before upgrading. A store the server opens is always at the
current format version — anything else is an error rather than something you have to remember to
check. See [`docs/adr/0016`](docs/adr/0016-store-format-migrations.md).

Exit status is `0` on success, `1` if the operation failed, and `2` if the arguments were not
understood — so a wrapper script can tell "retry this" from "you typed it wrong". `openbiz help`
prints the full usage.

There is **no online backup yet**: see [`docs/adr/0015`](docs/adr/0015-backup-and-restore.md) for
why these are commands rather than HTTP endpoints, and `docs/PROPOSED.md` for the authenticated
endpoint that would remove the need to stop the server.

## Reading a vocabulary

```sh
openbiz inspect https://example.org/regions   # what does this vocabulary hold, in SKOS terms?
```

It reports the concepts, concept schemes, and collections a vocabulary holds — **including the ones
no statement typed**, because SKOS entails them: a graph saying `<C> skos:inScheme <S>` has a
concept scheme whether or not anyone said so, and a tool that counted only `rdf:type` would report
zero schemes for a large share of real thesauri.

Every fact it inferred is printed with the statement it followed from and the statement of the SKOS
Reference that licensed it:

```
<…/scheme> rdf:type skos:ConceptScheme
  because <…/scheme> skos:hasTopConcept <…/apac>
  and S5: The rdfs:domain of skos:hasTopConcept is the class skos:ConceptScheme.
```

That is not decoration. A governance team defending a decision to an auditor needs to show why a
concept is in a scheme, and "the tool says so" is not an answer.

It also separates a violated **integrity condition** — S9 and S37, which make a graph not a SKOS
vocabulary — from something merely **ill-formed**, which SKOS permits and we think is a mistake.
Two `skos:memberList` values on one resource are the case that catches tools out: it looks like a
violation of S35 and is consistent with SKOS, so we report it and say the judgement is ours. See
[`docs/adr/0019`](docs/adr/0019-skos-core-model.md).

### SKOS-XL

A thesaurus that follows ISO 25964 gives each label an IRI of its own, so it can record who
created the label, when it was approved, and what it stands for — none of which plain SKOS has
anywhere to put. `inspect` reads that too, and applies the SKOS Reference's sub-property chains
(S55–S57), so a concept labelled **only** through SKOS-XL still reports the plain SKOS labels it
entails:

```
skos-xl labels:
  2 skosxl:Label resource(s), 2 with exactly one literal form
  1 resource(s) labelled through SKOS-XL, 2 plain SKOS label(s) inferred from them
```

Those inferred labels count towards the per-language coverage and name the concept in the report,
because to the person asking "how much of this is in French?" a SKOS-XL label is a French label.
Each one is printed with its chain, like any other inference.

Appendix B of the SKOS Reference states no integrity conditions, so the severity of every SKOS-XL
finding is a judgement — and [`docs/adr/0021`](docs/adr/0021-skos-xl-labels-and-dumbing-down.md)
records, rule by rule, whose. Two literal forms on one label is the specification's own
"not consistent"; a label with *no* literal form is deliberately **not** an inconsistency, because
"cardinality exactly 1" entails that a form exists without requiring the graph to state it, and
refusing a partial export would be refusing valid data.

`skosxl:labelRelation` **is** read — it is where ISO 25964 puts an acronym relationship, which
plain SKOS cannot express — and a link entails its converse because S62 makes the property
symmetric. A *refinement* of it is deliberately not closed, because Appendix B.4.4.1 warns that a
sub-property of a symmetric property is not necessarily symmetric. See
[`docs/adr/0022`](docs/adr/0022-skos-xl-label-relations.md).

`inspect` only reads. It writes nothing, and a test asserts the store is byte-for-byte unchanged.

### Reading the hierarchy

```sh
openbiz ancestors https://example.org/regions https://example.org/regions/japan
```

`skos:broader` records one step. §8 of the SKOS Reference makes `skos:broaderTransitive` an
`owl:TransitiveProperty` (S24), so a chain of one-step links entails a link from each concept to
every concept above it — and that closure is **never stored**, at any vocabulary size. A chain of
100 000 links is a legal SKOS graph and its closure is five thousand million pairs, and a stored
pair could cite the rule but not name the path it took. So it is walked on demand:

```
<…/japan>  ("Japan"@en)
in https://example.org/regions

2 concept(s) are above it, by 2 link(s) walked:
  <…/apac>  ("Asia-Pacific"@en)
    <…/japan> → <…/eastasia> → <…/apac>
    because <…/japan> skos:broaderTransitive <…/eastasia>, <…/eastasia> skos:broaderTransitive <…/apac>
    and S24: skos:broaderTransitive and skos:narrowerTransitive are each instances of owl:TransitiveProperty.
```

The path is the derivation. For a link nobody wrote, showing the chain is the difference between a
verdict and an explanation.

That walk is also what makes §8.4's integrity condition checkable. **S27** makes `skos:related`
disjoint with `skos:broaderTransitive` — the SKOS Reference treats hierarchical and associative
links as "fundamentally distinct in nature" and takes the stronger position that the disjointness
reaches *indirect* hierarchical links too — so a vocabulary saying `<A> broader <B>`, `<B> broader
<C>` and `<A> related <C>` is not a SKOS vocabulary, even though nobody wrote a link from `<A>` to
`<C>`. `inspect` reports it, with the chain that makes it actionable.

The walk is bounded, and a walk that hit its bound says so rather than reporting what it managed
to reach as the answer — a check that gave up is not a check that passed. See
[`docs/adr/0025`](docs/adr/0025-transitive-ancestry-by-walking.md).

### Reading what a concept means

```sh
openbiz notes https://example.org/regions https://example.org/regions/apac
```

§7 of the SKOS Reference gives seven documentation properties — `definition`, `scopeNote`,
`example`, `historyNote`, `changeNote`, `editorialNote`, and the general `note` — and **S17** makes
the first six sub-properties of the last. So writing a definition entails a note, and this is the
one place that shows it: a Turtle export of the same vocabulary shows the `skos:definition` the
author wrote and never shows the `skos:note` it entails.

```
skos:note
  "The Asia-Pacific region."@en
    inferred, not stated under skos:note
    because skos:definition "The Asia-Pacific region."@en
    and S17: skos:changeNote, skos:definition, … are each sub-properties of skos:note.
```

It takes a **resource**, not a concept, because §7's own Example 24 documents an `owl:Class`.

`inspect` counts the same thing per property, and prints one sentence beside the counts that most
tools get wrong:

> §7 states no integrity condition, so an undocumented concept is consistent SKOS; requiring a
> definition is a Z39.19 / ISO 25964 rule pack

**A concept with no definition is not a defect in SKOS.** §5.4 has an "Integrity Conditions"
heading and §7 has none at all. Asking every concept to carry a definition is a real and reasonable
governance rule — it is just ANSI/NISO Z39.19's rule or ISO 25964's, not the SKOS Reference's, and
it belongs in a rule pack you can name and switch off rather than in a finding that cites a
statement nobody made. Those packs are Phase 4. Until then we report the number and say who is
asking. See [`docs/adr/0026`](docs/adr/0026-documentation-properties.md).

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
