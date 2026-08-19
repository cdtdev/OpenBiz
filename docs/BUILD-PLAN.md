# OpenBiz build plan

The backlog and the burn-down. One `- [ ]` per item; check it off only when it meets the
**definition of done** in `CLAUDE.md` §4 — including having a real production caller.

**Status:** Phase 0 is complete — verified by counting the unchecked boxes in the phase, not from
memory of what was left (a product-owner correction after iteration 4; see `FEEDBACK-LOG.md`).
Phase 1 is **12 of 14**, and the two that remain are deliberately deferred — see below. The embedded store opens, stamps, and closes an Oxigraph instance inside
the binary; it has a **named-graph model with a real enforcement point** — one graph per
vocabulary, a system graph for OpenBiz's own metadata, `urn:openbiz:` reserved against user
authoring, and a single write choke point that every write passes through — and **that model is now
visible to a user**: `GET /api/graphs` serves the registry, and the interface lists the
vocabularies in it while keeping OpenBiz's own graphs out of the user's list and counted rather
than hidden. **Writes are transactional and serialised**, which closed a real corruption race in
the creation path. **A graph can be got back out**: `GET /api/export` serialises any registered
graph to any of the six syntaxes §2 commits to, the interface offers it per vocabulary with a
format chooser read from the server, and the export carries none of OpenBiz's own bookkeeping.
**And it can be asked questions**: `GET`/`POST /api/sparql` evaluates SPARQL 1.1 queries in all
four results formats and all six RDF syntaxes, over a default dataset that is the user's
vocabularies and none of OpenBiz's own graphs, bounded by limits that refuse rather than truncate.
402 Rust tests and 30 UI tests passing; `cargo fmt`, `cargo clippy -D warnings`,
`cargo deny`, and the UI typecheck/test/build are green. **And the serialisation claim is now
narrower and better evidenced:** N-Triples and N-Quads are checked against a reader written from
the published EBNF rather than against the library that wrote the bytes, which found two real
defects nobody had seen (see `adr/0012`). **And the store's literal handling is characterised rather than assumed:** the boundaries at which
`xsd:integer`, `xsd:decimal`, `xsd:double`, `xsd:dateTime`, and `xsd:duration` stop being
interpreted are measured and pinned, and the finding is that a literal past the boundary
round-trips perfectly while silently ceasing to be a value — so a filter over it omits rows rather
than failing. A derived integer datatype is silently replaced by `xsd:integer`, which loses
statements (see `adr/0014`). **And the engine is now measured rather than trusted:**
the charter's standing warning that Oxigraph's query evaluation is unoptimised has a number against
it at 10k, 100k, and 1M concepts, taken through our own query entry point against the queries the
interface will issue — navigation is flat and fast, the tree's *first* query is a 21-second cliff
with a 0.6 ms fix, and label search does not scale (see `adr/0013`).
**And a deployment can now be got out and put back:** `openbiz backup <file>` writes the whole
store — every vocabulary and OpenBiz's own registry — as N-Quads rather than as a snapshot of the
storage engine, and `openbiz restore <file>` rebuilds an empty store from one, refusing anything
that would not open afterwards (see `adr/0015`), and its production caller is the command line
rather than an unauthenticated endpoint. **The single binary is real:**
a `Single binary` CI job deletes `ui/dist` from disk and the release binary still serves the full
interface. **The roadmap is the repo, publicly:** this plan, the ADRs, and the honest gaps in
`UNTESTED.md` are readable by anyone.

**Current position:** Phase 2 (SKOS authoring model), 13 of 24 items done — 24 and not 23 because the mapping-properties item was split in two at iteration 32. **The build now knows
what a concept is, what it is called, and how to read a thesaurus that calls things the ISO 25964
way.** A vocabulary's lexical labels are modelled per language, both of the integrity conditions
SKOS states on them are enforced (S13, S14), and `openbiz inspect` reports which languages a
thesaurus is actually in and how far behind each one is — see `adr/0020`, which also records what
"RDF plain literal" has to mean now that RDF 1.1 has abolished the term. **SKOS-XL is in, both
halves** (Appendix B.2, B.3 and B.4): a label is a resource with an IRI you can date, attribute and
version, and the S55–S57 chains dumb it down to a plain SKOS label so the same two integrity
conditions catch Appendix B's own Examples 84–87 — see `adr/0021`, which records why Appendix B
having no "Integrity Conditions" heading made every severity in it a decision to write down.
**And labels can be linked to each other**, which is where ISO 25964 puts an acronym relationship
and what plain SKOS cannot express: S59–S62 are applied, a link entails its converse because the
property is symmetric, and a refinement of it is deliberately *not* closed, because B.4.4.1 warns
that a sub-property of a symmetric property is not necessarily symmetric — see `adr/0022`.
**And the vocabulary now has a shape**: §8's semantic relations are read and closed, so a
hierarchy an author wrote in one direction reads in both, polyhierarchy is counted rather than
complained about, and a `skos:broader` pointing at a collection is caught by a domain rule on a
property nobody writes — see `adr/0023`. **And the hierarchy can now be read all the way up**: S24's
transitive closure is applied by a bounded walk rather than stored — `openbiz ancestors <graph>
<concept>` prints every concept above one with the path that reached it, which for a link nobody
wrote *is* the derivation — and §8.4's integrity condition S27 is read off that walk, so §8.5's
Examples 25–29 all come out to the consistency the specification prints beside them. Example 27's
clash, between two concepts the author never linked directly, was reported as a clean vocabulary
until iteration 28. A cycle stays consistent, terminates, and is named rather than complained
about. See `adr/0025`.
**And a check that gave up no longer reads as a check that passed:** `Severity` gained `Unchecked`
and `openbiz inspect` closes with one of three sentences instead of two.
**And the vocabulary can now say what it means**: §7's seven documentation properties are read,
S17 lifts each of the six specific ones onto `skos:note`, and `openbiz notes <graph> <resource>`
prints a concept's definition, scope note and examples with the statement behind every note SKOS
entailed — the one thing a Turtle export of the same vocabulary cannot show. **§7 states no
integrity condition and we invent none**: a concept with no definition is consistent SKOS, the
report says so beside the count, and the check every incumbent runs there is named as the Z39.19 /
ISO 25964 rule pack it actually is. See `adr/0026`.
**And a vocabulary's own note properties are read too.** §7.1 calls the seven "a set of extension
points", and an enterprise thesaurus uses them: `ex:usageNote rdfs:subPropertyOf skos:scopeNote`
plus a statement written with `ex:usageNote` now reaches the report as a scope note citing RDFS
`rdfs7`, which S17 then lifts to a `skos:note`. Reading it takes **two passes over the source** —
a declaration can arrive after every statement that uses it, and buffering the alternative would
cost the graph in memory. `openbiz inspect` names the declared properties rather than only counting
them, because a number an author cannot check against their own file is not a report. See
`adr/0028`, and note that `Derivation` can now cite RDFS as well as SKOS, because citing an
S-number for something SKOS does not state would be a guess wearing a citation.

**And a vocabulary can now say what it is joined to.** §10's five mapping properties are read and
closed (S38–S44), and the load-bearing decision is that a mapping is **not** a section of its own:
S41 lifts `skos:broadMatch`, `skos:narrowMatch` and `skos:relatedMatch` into §8's relations before
those are closed, so `openbiz ancestors` climbs through a mapping into another vocabulary's concept
and §8.4's S27 catches §10.6.2's Examples 59–61 with no rule of §10's own. S46, §10's only
integrity condition, is reported once per pair and cites §10.4's note where the specification
argues by inversion rather than naming the property. See `adr/0029`.
**And S45's closure is now walked too**, which closed a false negative rather than only adding a
conclusion: `skos:exactMatch` is transitive, so `<A> exactMatch <B> exactMatch <C>` with
`<A> broadMatch <C>` violates §10.4 — and no statement in that vocabulary names both properties for
one pair, so it was reported as *consistent* until iteration 33. That is the **hub** shape an
enterprise actually produces. The walk is over an undirected **cluster** rather than a path
upwards, because S44 puts every link at both ends, so cycles are ordinary (§10.6.6 requires coping
with them) and a mapped concept is its own exact match (Example 66, consistent, printed rather than
hidden). `openbiz mappings <graph> <resource>` is the per-concept view, printing what one concept
is joined to with the rule behind every link the graph did not state. See `adr/0030`.

**And we now know what that costs, which changed the design of the next item before it was
written.** Iteration 26 measured the relation model at 10k, 100k and 1M links across four
hierarchy shapes and decided, in `adr/0024`, that **S24's closure is never materialised** — a legal
100 000-link chain licenses five thousand million pairs, and a stored entry can cite the rule but
cannot name the path, which `CLAUDE.md` §3 requires. Ancestry will be a bounded traversal answered
on read. The measurement also produced the first hard number that contradicts a non-negotiable:
a stated link costs **3.9 KiB of resident memory**, 43× the size of the fact, and a million-link
vocabulary with no labels at all held **4.4 GiB**. That is about what is already shipped, it is
recorded rather than fixed, and it is in `UNTESTED.md` and three proposals awaiting a human.

**Iteration 30 was a blind-spot pass, so no plan item moved and the count above is unchanged.** It
audited `ancestry.rs`, which iteration 28 had asked the next blind-spot pass to treat as
inherited-and-unaudited, and found that **the bound protecting §8.4's disjointness check bounded
nothing**. `AncestryBound::max_links` was per *walk*; the check makes one walk per concept with a
`skos:related`, so its cost is concepts × depth and the ceiling was never consulted at that level.
A legal 10 001-concept chain with one associative link per concept built in **30.63 seconds against
62 ms** without them — 490× the whole rest of the model — and the report said the check had
**finished**. Nothing caught it because no fixture in the repository stated a `skos:related` at
scale: `scale.rs` had been measuring the data the pass reads and never the pass. The budget is now
shared across the sweep (**530 ms**, abandonment reported and counted), and the trade it makes —
a partial answer where there was a slow complete one — is in `adr/0027` and `UNTESTED.md` with two
proposals for the real fix.

**Iteration 25 was the product-owner pass, so no plan item moved and the count above is unchanged.**
It landed one thing that changes the shape of a later phase: **`horned-owl` is LGPL-3.0**, which
`CLAUDE.md` §5 forbids, so the first Phase 9 item is now blocked on a licence decision a human has
to take. Two other items — the Phase 4 SHACL spike and Phase 5's `whelk-rs` line — carry notes
about assumptions in them that no longer hold. The research and its sources are in
`docs/COMPETITIVE.md`; eight proposals are in `docs/PROPOSED.md` awaiting promotion, and none was
promoted by the loop.

`openbiz inspect <graph>` reads a vocabulary and reports its concepts, concept
schemes, and collections — including the ones no statement typed, because SKOS itself entails
them — and names the specification statement behind every fact it inferred. It separates a violated
SKOS integrity condition from something merely ill-formed and says which judgement is ours (see
`adr/0019`). The candidate seam is
complete except for its HTTP and UI half, which is blocked on authentication — so **every remaining
Phase 2 item now has the shape it needs to be built against**, which was not true two iterations
ago. Phase 1 is 12 of 14 and the two that remain — SPARQL Update and the Graph Store Protocol — are
blocked on **authorisation**, not on the seam: the seam they were waiting for now exists, and what
is left is that neither may be an unauthenticated write.

**Iteration 18 took the seam's removing half.** A candidate carries two staging graphs, so a merge,
a split, a move, and a deprecation are expressible for the first time. The decision that took the
work is the **precondition**: a removal names statements that must already exist, so it is checked
against the vocabulary at proposal *and* again inside the transaction that applies it, and an
approval the vocabulary has outgrown is refused rather than partly applied. Store format version 4.
See `adr/0018`.

**Iteration 17 took Phase 2's candidate seam**, which is the dependency the whole phase is ordered
around and which three Phase 1 items had been deferred on for six iterations. A proposed change is
now a **named graph plus a record**: the statements are staged in
`urn:openbiz:graph:candidate:<id>`, so a pending change is exportable in any of the six syntaxes and
queryable over SPARQL on the day it exists, and approval is a copy between two graphs *inside the
transaction that records who approved it* — so the store can never hold statements in a vocabulary
with no record of who let them in. Provenance is mandatory and its source is a closed token, so
"show me everything an assistant proposed" is answerable before there is an assistant. The first
producer is `openbiz import`, and **that closed Phase 1's RDF parsing item**: all six syntaxes,
round-tripped against the serialiser, with a real production caller. See `adr/0017`.

**Iteration 16 took the store-format migration framework**, whose first migration turned out to be
code that already existed: an unconditional per-open self-heal that re-registered the system graph forever, for the
benefit of stores that needed it once and with no record that it had ever happened. It is now a
versioned, one-off, self-explaining step, and `openbiz restore` migrates an older backup instead of
refusing it. See `adr/0016`. **Iteration 14 took backup and restore**, which is the
first item since the two spikes that ships a capability rather than a measurement. A backup is a
single N-Quads file carrying the whole store including the registry, so it is readable by any
conforming tool and hand-authorable — the end-to-end test's fixture was written from the
specification rather than produced by our own writer. A restore is one transaction that re-reads
the registry it wrote, inside that transaction, and refuses to commit a store this build could not
open. Its production caller is the command line, because a backup script needs an exit status and
because an unauthenticated `POST /api/restore` would be the same defect SPARQL Update is deferred
over. **This also gave the N-Quads parser its first production caller**, which is the condition
`adr/0010` set — but the parsing item stays open, because one syntax of six reading a whole store
into an empty one is not an import — a gap iteration 17 closed with the candidate seam's import.

**Iteration 15 took no plan item.** Iteration 14 built, tested, and pushed backup/restore but ended
without merging it, so PR #17 sat open with a required check wedged in an unbounded `apt-get` and
`main` did not contain the capability this plan already described as done. Iteration 15 bounded
CI's toolchain install and every job with `timeout-minutes` — so a stalled network call now fails
the check instead of leaving it pending forever, which branch protection cannot distinguish from
still-running — and then landed PR #17. Iteration 16 landed the store-format migration framework,
which closed that gap: `openbiz restore` no longer refuses a backup written by an older build, it
migrates it as it reads it (see `adr/0016`).

**Iteration 20 took the SKOS core model**, and the decision that took the work was the
*layering*: `openbiz-skos` depends on neither Oxigraph nor `openbiz-store`, so the model is
testable from a literal array of statements and the same code will classify a candidate's staging
graph, a parsed file, or a discovery result. The price is a duplicated statement type mapped in
three lines at the composition root, and `adr/0019` records why that was the cheapest of the three
options. **Next is SKOS-XL labels.**

Phase 1's two open items are no longer waiting on the seam: SPARQL Update and the Graph Store Protocol
each wait on authorisation, which is what part 3 of the seam waits on too. **Phase 1 is therefore
as complete as it can be without an identity model.** Vocabulary *creation* over HTTP remains
deliberately absent for the same §1.7 reason, so `POST /api/graphs` answers 405. **There is still
no SPARQL console in the interface**; the endpoint's caller is HTTP, and the console is an open
§4.4 gap in `UNTESTED.md` and a proposal. **And there is no online backup** — both new commands
need the store to themselves, so a backup means stopping the server; the authenticated endpoint
that would remove that cost is in `PROPOSED.md`.

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
- [x] Parse those same six syntaxes into the store, round-tripped against the serialiser above
      > **Split note (iteration 8).** One item wearing two hats, and the seam between them is a
      > charter constraint rather than convenience. A parser's production caller is an *import*, an
      > import mutates a vocabulary, and `CLAUDE.md` §3 says a change to a vocabulary arrives as a
      > reviewable **candidate** — the seam that is Phase 2's first item. Landing the parser now
      > would mean either code with no caller (§4.1) or a direct-write import to retrofit later,
      > which is the exact failure §3 warns about. It lands with whichever comes first: backup and
      > restore below, which parses N-Quads and touches no vocabulary, or Phase 2's candidate seam.
      > Serialisation has no such dependency — an export is a read.
      >
      > **Half met at iteration 14.** Backup and restore landed, so the **N-Quads** parser now has
      > a real production caller. This item stays open and the reason is not bookkeeping: restore
      > reads *one* syntax of six, and it reads a **whole store** — registry included — into an
      > empty one, which is not an import. An import lands statements in somebody's existing
      > vocabulary, and that is the mutation still waiting on Phase 2's candidate seam. Checking
      > this box now would claim five parsers we do not have and an import path we deliberately do
      > not.
      >
      > **Closed at iteration 17, by the seam it was waiting for.** Phase 2's candidate seam landed
      > and brought its first producer with it: `openbiz import <graph> <file>` parses **all six**
      > syntaxes and proposes the statements as a reviewable candidate. The round trip is proven per
      > syntax — a vocabulary is exported, re-proposed, and the staged statements compared to the
      > source graph statement for statement — and the production caller is the command line.
      > **Scope, honestly:** the parser's *only* entry point is the candidate seam. There is no
      > direct-write import and there is not going to be one, which is the whole reason this item
      > waited. The round trip is still against our own serialiser, so it is fidelity rather than
      > conformance; that gap is unchanged and stays in `docs/UNTESTED.md`. See `adr/0017`.
- [x] SPARQL 1.1 Query endpoint with all four result formats (JSON, XML, CSV, TSV)
      > **Better, not parity.** Every tool in this market has a SPARQL endpoint, so the question is
      > not whether but what it answers with by default, what stops it, and what it refuses to
      > guess. Three things they do badly are what this item is about. (1) **The default dataset is
      > a trap.** Point a taxonomist at PoolParty's or GraphDB's endpoint and
      > `SELECT * WHERE { ?s ?p ?o }` returns the union of everything, tool bookkeeping interleaved
      > with their vocabulary and unlabelled. Here the default dataset is the registered
      > *vocabulary* graphs and nothing else — `adr/0007`'s named-graph model paying for itself a
      > second time — the rule is written down, the graphs it covers are exactly what
      > `GET /api/graphs` already reports as `kind: "vocabulary"`, and a query naming its own `FROM`
      > is honoured verbatim so an operator can still ask "what is actually in my store?". The
      > default is *chosen*, not imposed. (2) **A runaway query is the caller's problem to notice.**
      > Endpoints either run until something falls over or truncate at a row cap and hand back the
      > truncation as if it were the answer. Here both bounds **refuse**: a governance team cannot
      > sign off rows they were never told were missing. (3) **`Accept` is advisory.** Ask a typical
      > endpoint for `text/turtle`, send it a `SELECT`, and you get JSON labelled JSON with no
      > acknowledgement that negotiation failed — here that is a 406 naming what the query actually
      > produced. Also: an update is refused *as an update*, recognised by parsing it rather than by
      > sniffing for a keyword, and a `SERVICE` clause is a 501 stating that this build has no HTTP
      > client so nothing was sent to the named endpoint. `GET /api/sparql/formats` advertises the
      > four results formats and **which of them loses term detail** — CSV writes bare text, so a
      > language tag is simply gone, which for a multilingual thesaurus is the difference between a
      > label and which language it is in. That constant is read from the same place the serialiser
      > branches on, and serving it is what gives it a production reader instead of leaving it a
      > fact only its own test consults.
      > **Scope, honestly.** This is SPARQL 1.1 **Query** over the protocol's three query forms, not
      > "SPARQL 1.1 Protocol": `default-graph-uri` and `named-graph-uri` are a named 400 rather than
      > an implementation, because how a protocol-supplied dataset composes with the
      > vocabulary-graph default *and* with a query's own `FROM` is a three-way decision worth
      > taking deliberately. Update and Graph Store Protocol are the two items below. And **there is
      > no query console in the interface** — the item is scoped to the endpoint and its production
      > caller is HTTP, so the console is recorded as an open §4.4 gap in `UNTESTED.md` and proposed
      > rather than quietly folded in here. See `adr/0011`.
- [ ] SPARQL 1.1 Update endpoint, guarded by authorisation
      > **Deferred at iteration 11, and the reason is a charter constraint rather than effort.**
      > An *applying* Update endpoint fails three of `CLAUDE.md`'s clauses at once, and only one of
      > them is the authorisation the item's own title names. (1) **§3:** "any path that changes a
      > vocabulary takes *candidates*, not just direct writes" — `INSERT DATA` is the most direct
      > write there is, and landing it now means retrofitting a review seam onto an endpoint whose
      > entire contract is *"this has already happened"*. (2) **§1.7:**
      > `INSERT DATA { GRAPH <new> … }` brings a vocabulary into existence, and a creation path
      > that skips `DiscoveryProvider` and records no justification is exactly what
      > `POST /api/graphs` answers 405 to prevent — adding the same capability under a different
      > verb is routing around our own rule, not implementing a standard. (3) **The guard:** there
      > is no authentication at all yet, and an unauthenticated arbitrary-write endpoint on a
      > governance product is not a partial feature, it is a defect.
      > The evaluation half is genuinely cheap — Oxigraph parses and applies updates already, and
      > `Store::query` has recognised an update *as* an update since iteration 9 — and that is
      > precisely why the split was refused rather than taken. The cheap half is the half that does
      > the damage; splitting here would land the write and defer the control.
      > **It lands with Phase 2's candidate seam and Phase 6's approval path**, where an update
      > stops meaning "apply this" and starts meaning "propose this change set, show me the diff,
      > and let a human approve it". That is the version worth having, it is the one no incumbent
      > offers, and it is not reachable from Phase 1. Nothing is checked off and nothing is claimed
      > in the meantime.
- [ ] SPARQL Graph Store Protocol
      > **Deferred at iteration 11, on the item above's dependency and on the parser's.** `PUT`,
      > `POST`, `DELETE`, and `PATCH` *are* this protocol, and every one of them is a direct write
      > to a vocabulary: they need the RDF parser (deferred above) and the candidate seam, the same
      > two things. What would be left is `GET` and `HEAD`, which `GET /api/export` already serves
      > under a different URL — so shipping the read half alone would be a second spelling of an
      > existing capability plus four 405s, and calling that "Graph Store Protocol" would misreport
      > a standard. `CLAUDE.md` §4 minds that considerably more than it minds an unchecked box.
- [x] **Spike:** benchmark Oxigraph query evaluation at 10k / 100k / 1M concepts. Record real
      numbers in an ADR. Upstream states evaluation is unoptimised — find where that bites us
      *before* we build the UI on top of it
      > **Better, not parity.** Every vendor in this market publishes a benchmark, and two habits
      > make almost all of them useless to the person asking *"will this hold my vocabulary?"*.
      > They measure a **benchmark suite** — BSBM, LUBM — which has no concept tree in it and so
      > cannot answer the only question a taxonomist has; and they are **unreproducible**, being
      > numbers from hardware you do not have over a dataset you cannot generate. Here every probe
      > is one *interaction* — draw the tree's first level, expand a node, open a concept, type in
      > the search box, show a breadcrumb, list a subtree — timed through `Store::query`, the same
      > call `/api/sparql` makes, so the measured time includes serialising the answer because that
      > is what a caller actually waits for. And the generator, the queries, and the harness are in
      > the repo: anyone who doubts the numbers can produce their own. The harness asserts **every
      > probe's answer count against the generator's own arithmetic before believing its timing**,
      > because a benchmark whose queries match nothing measures an empty loop very quickly.
      > **It found where it bites, and the answer was not where the charter's warning pointed.**
      > Every bound-term lookup — expand, open, resolve a label, walk a breadcrumb — is 0.2–0.6 ms
      > *flat* from 10k to 1M. What falls over is the **first query the interface issues**: finding
      > top concepts by `FILTER NOT EXISTS { ?c skos:broader ?p }` costs 89 ms at 10k, 1.16 s at
      > 100k, and **21.6 s at 1M** — and it is *served*, not refused, because 21.6 s fits inside
      > the 30 s deadline. A scheme that states its top concepts with `skos:hasTopConcept` answers
      > the identical question in **0.6 ms, flat**. Three more findings: `LIMIT 50` does not bound
      > the work, so type-ahead search is linear in the graph and costs ~0.5 s per keystroke at 1M;
      > our *own* 100 000-answer cap refuses "everything under this branch" at 1M (111 110 rows);
      > and a million concepts loads through the transactional write path in five minutes and
      > occupies ~6 GB. See `adr/0013`, which also lists what it did **not** measure — concurrency,
      > memory, a cold cache, and a realistically lumpy vocabulary.
- [x] **Spike:** characterise Oxigraph's numeric/calendar/duration literal precision limits and
      decide our documented behaviour at the boundary
      > **Better, not parity.** Every store in this market has limits here; not one of them
      > publishes where they are. The incumbents' JVM stores use `BigInteger`/`BigDecimal`, so on
      > raw range they beat us and saying otherwise would be dishonest — that is written up as a
      > parity finding rather than glossed. What none of them tell you is *what happens at the
      > edge*, and that is where this spike went. The answer is not "large values round": it is
      > that a literal outside the range the backend models **stops being a value** while still
      > round-tripping byte-for-byte, so `FILTER(?value > 1000)` silently omits the rows that
      > crossed the line and a short answer reads exactly like "there were no such rows". One digit
      > separates `…105727` (a number) from `…105728` (not one), and nothing in the data
      > distinguishes them. The boundaries for integer, decimal, float/double, calendar, and
      > duration are now measured, pinned as tests that fail if either side moves, and published in
      > `adr/0014` — which is the thing the incumbents do not do.
      > **And it found a defect the item did not ask about**, which is worse than the boundary: the
      > datatype IRI of a derived integer type is **not preserved**. `"5"^^xsd:int` is stored and
      > returned as `"5"^^xsd:integer`, so four distinct RDF terms written against one subject come
      > back as **two statements** — silent triple loss on ordinary input. It also means a SHACL
      > `sh:datatype xsd:int` constraint (Phase 4) can never be satisfied and an OWL 2 datatype
      > range over a derived type (Phase 5) is untestable. Both are recorded against those phases in
      > `UNTESTED.md` rather than left to be met as a surprise.
      > **Scope, honestly.** The spike *characterises and documents*; it does not fix. The remedy —
      > the store telling a caller what it rewrote at the moment it rewrote it — is a user-facing
      > capability belonging to Phase 2's candidate seam, and it is in `PROPOSED.md` for a human
      > rather than self-authorised (`CLAUDE.md` §7).
- [x] Backup and restore to a single portable file; restore verified against a live store
      > **Better, not parity.** Every incumbent has a backup story and most of them have the same
      > one: stop the app, snapshot the triplestore, hope the versions line up on the way back.
      > Three things are different here. (1) **The file is RDF, not our storage engine's.** A
      > backup is N-Quads — every vocabulary *and* OpenBiz's own registry, in a W3C
      > Recommendation any conforming tool can read. A RocksDB checkpoint would have been three
      > lines instead of four hundred, and it would have made the customer's disaster recovery a
      > function of a dependency `CLAUDE.md` §3 explicitly reserves the right to replace. The
      > end-to-end test's fixture is **hand-written from the specification**, not produced by
      > `openbiz backup`, because a portability claim nothing but us can satisfy is not a claim.
      > (2) **A restore checks that it is about to produce a store we can open** — it re-reads the
      > registry it just wrote, through the same code `Store::open` uses, *inside the transaction
      > that wrote it*, and rolls the whole thing back if the answer is no. Restoring an unopenable
      > store is the worst outcome available, because the operator has already lost the original.
      > The whole file is one transaction for the same reason: half-restored is the state nobody
      > can reason about. (3) **The refusals name the operator's next action.** An *export* handed
      > to restore is refused for having no format stamp — with a message saying it is not a
      > backup — rather than by a syntax error in a file that is perfectly valid RDF; an older
      > stamp says "this needs a migration"; a newer one says "upgrade"; a non-empty store says
      > "restore into a fresh data directory". A backup never overwrites an existing file, because
      > the file most likely to be in the way is the last good backup.
      > **Its production caller is the command line**, not HTTP: `openbiz backup <file>` and
      > `openbiz restore <file>`, which is what cron, a systemd timer, and a pre-stop hook can
      > actually use — and what keeps an unauthenticated "replace the whole store" endpoint from
      > existing while there is still no authentication. The end-to-end test restores a backup
      > through the real binary, starts the real server on the result, and finds the vocabulary in
      > `GET /api/graphs` and its statements in `GET /api/export`. See `adr/0015`.
      > **Scope, honestly:** there is **no online backup** — both commands need the store to
      > themselves, so taking one means stopping the server, and the authenticated endpoint that
      > would fix that is proposed rather than built. The round trip is proven on small stores; the
      > memory a large restore needs is unmeasured, and both are in `UNTESTED.md`.
- [x] Store-format migration framework — versioned, forward-only, tested on a populated store
      > **Better, not parity.** Every product in this market migrates its store; what they do
      > badly is tell you about it. Four things this does differently. (1) **The first migration
      > was already in the code, unnamed.** `Store::open` re-registered the system graph on every
      > single open so that a store written before the registry existed would acquire one — an
      > unconditional idempotent write on the startup path, running forever for stores that needed
      > it once, leaving no record and knowing nothing about which stores had needed it. It is now
      > migration 1 → 2, it runs once, and what replaced it on the open path is a **check that
      > refuses**: a store claiming version 2 that violates version 2's invariant is reported, not
      > silently mended. A version therefore records *which invariants hold*, which is what makes
      > the refusal legitimate. (2) **It explains itself twice.** `CLAUDE.md` §3 requires an
      > auto-applied change to answer "why?", and a store upgrade is the one change nobody asked
      > for: it happens at startup, to a customer's data. So the caller gets a `MigrationReport`
      > that the server logs and `openbiz restore` prints — "restored 12 000 statements" looks
      > identical whether or not the file was migrated — **and the store gets a record**: what ran,
      > from, to, why, and when, as ordinary RDF in the system graph. The log scrolls away; the
      > record answers the auditor a year later, through a SPARQL query naming
      > `FROM <urn:openbiz:graph:system>` rather than through a proprietary log. (3) **A gap
      > refuses rather than skips**, naming the *missing* version, because that number identifies
      > the release the operator needs; and the chain is checked unbroken from 1 to
      > `FORMAT_VERSION` by a test, so bumping the constant without its migration fails the build
      > instead of a customer's store. (4) **Restore migrates the file** — `openbiz restore` no
      > longer refuses an older backup, it brings it forward inside the transaction that wrote it,
      > so an unmigratable backup restores nothing rather than something misread. One transaction
      > for the whole chain, for `adr/0015`'s reason: half-migrated is the state nobody can reason
      > about.
      > **Its production callers are `Store::open` and `Store::restore`**, both of which run the
      > chain on every invocation and one of which actually migrates in the end-to-end test
      > against the real binary. The engine takes its chain and target as parameters, so the
      > roll-back of a *failing* step is proven on a synthetic chain rather than by adding a
      > failing migration to the real one. See `adr/0016`.
      > **Scope, honestly:** every version-1 store in these tests was made by **degrading a
      > version-2 store**, because no version-1 build exists — that fixture is our belief about
      > version 1, not version 1. No migration has ever rewritten content rather than metadata, so
      > the memory ceiling `adr/0015` records is untested here too, and how far back a build should
      > migrate from is a support policy nobody has written. All three are in `UNTESTED.md`, and
      > the fixture corpus that would fix the first is in `PROPOSED.md`.

---

## Phase 2 — SKOS authoring model

> Enables: the product's core noun. Everything a taxonomist does lands here.

- [x] **Candidate seam, part 1 — additions:** a proposed change to one vocabulary, carrying
      provenance, source, and a confidence where one is meaningful, staged where a human can read it
      and applied only on approval. First producer: `openbiz import`.
      > Added on product-owner instruction (`FEEDBACK-LOG.md`, 2026-08-18), which names it the
      > highest-value near-term work for LLM integration. It is `CLAUDE.md` §3 "design for
      > assistability" made concrete, and it is **interface shape, not new functionality** — do not
      > build agents or an `LlmProvider` behind it. Build this **before** the mutation items below,
      > or every one of them needs retrofitting.
      > **Split at iteration 17** into three: additions (this item), removals, and the HTTP/UI half.
      > The split is not convenience — the second waits on nothing and the third waits on
      > authentication, so bundling them would have meant holding the seam back behind a blocker
      > that has nothing to do with it.
      > **Better, not parity.** Every incumbent has *some* review step and three things they do
      > badly are what this item is about. (1) **The proposal is opaque until you accept it.** A
      > pending import in PoolParty or EDG is a job in a queue, not a thing you can read; the diff
      > you are approving is rendered by one screen that has to exist for the purpose. Here the
      > payload is an ordinary named graph, so a pending change is exportable in any of the six
      > syntaxes and queryable over SPARQL the day it exists, and `openbiz candidate 7` prints the
      > statements through the same `GET /api/export` a runbook would use. (2) **Provenance is
      > whatever the producer felt like writing.** Here the source is a closed token, so "show me
      > everything an assistant proposed" is answerable, and a proposal that cannot say who raised
      > it or why is refused at the point of proposal rather than discovered at review. (3) **The
      > approval and the change are two events.** Here they are one transaction, so the store can
      > never hold statements in a vocabulary with no record of who let them in — which is the only
      > version of this an auditor can use.
      > **Its production caller is the command line**, not HTTP: `openbiz import`, `openbiz
      > candidates`, `openbiz candidate <id>`, `openbiz approve <id>`, `openbiz reject <id>`. An
      > unauthenticated "apply this change to a vocabulary" is the same objection that has SPARQL
      > Update deferred. This also gave Phase 1's RDF parsing item its production caller and closed
      > it. See `adr/0017`.
      > **Scope, honestly:** additions only — a candidate cannot yet propose a removal, so nothing
      > that needs one (merge, split, deprecate) is reachable through the seam. The evidence is kept
      > forever, so an approved import is stored twice and there is no retention policy. Both are in
      > `docs/UNTESTED.md`.
- [x] **Candidate seam, part 2 — removals:** a candidate proposes statements to *remove* as well as
      to add, so a merge, a split, a move, and a deprecation are all one shape.
      > Split out of the item above at iteration 17. Every bulk operation below needs this, and none
      > of them should be built before it exists — that is the same argument the seam itself was
      > built on, one level down. The record shape already leaves room for it.
      > **Done at iteration 18.** A candidate now has two staging graphs — additions at
      > `urn:openbiz:graph:candidate:<id>` and removals at `…:<id>:removals` — under one prefix, so
      > either half exports and queries through the paths that already exist. Production caller:
      > `openbiz retract <graph> <file>`, the mirror of `openbiz import`, which is a real workflow
      > on its own (export the vocabulary, cut it down to what should go, hand it back) and not a
      > command that exists for the test's benefit.
      > **The precondition is the substance of the item, not a detail.** A removal names statements
      > that must already be there, and the vocabulary can change between the proposal and the
      > approval. It is checked twice — at proposal, refusing a file whose statements are not in the
      > vocabulary; and again *inside the applying transaction*, refusing an approval the vocabulary
      > has outgrown. A stale candidate stays open so it can still be rejected. Applying what still
      > matches was rejected as an option: it removes less than the reviewer agreed to and reports
      > success, and nothing afterwards says the change differed from the one reviewed. This answers
      > the doubt iteration 17 recorded when it deferred this item.
      > **Store format version 4**, with a migration that writes nothing — because a version-3 build
      > would read a removing candidate as removing nothing, show half a diff, and apply half a
      > change while recording that it applied all of it. See `adr/0018`.
      > **Scope, honestly:** nothing produces a candidate carrying *both* halves yet. The record and
      > the apply path support one; the producers are the bulk operations below. In
      > `docs/UNTESTED.md`.
- [ ] **Candidate seam, part 3 — over HTTP and in the interface:** propose, list, review, approve,
      and reject from the API and the UI.
      > Split out at iteration 17 and **blocked on authentication**, which does not exist yet. The
      > CLI half is deliberately complete without it, so a deployment can use the seam today; what
      > is missing is that a reviewer has to be on the server's console. `POST /api/candidates` and
      > an approve endpoint are unauthenticated arbitrary writes to a customer's vocabulary until
      > there is an identity behind them, which is a defect rather than a partial feature.
- [x] SKOS core model: `Concept`, `ConceptScheme`, `Collection`, `OrderedCollection`
      > A graph can be asked what it holds in SKOS terms, and the answer includes what nobody
      > stated. The model is **engine-free** — `openbiz-skos` depends on neither Oxigraph nor
      > `openbiz-store`, so it classifies a literal array of statements, and the store grew a
      > second exit (`Store::for_each_statement`) so a caller can reason about a graph without
      > serialising and re-parsing it. It applies the nine SKOS axioms that bear on class
      > membership (S4–S8, S29, S31, S33, S36), quotes each in full, and **every derived fact
      > carries its premise and its rule** — `CLAUDE.md` §3's explainability requirement, not a
      > nicety. S32 is deliberately not applied: its range is a union, which entails neither
      > disjunct, and inferring one would be a guess wearing a citation.
      > **A violated integrity condition (S9, S37) and something merely ill-formed are different
      > words and are not blurred** — two `skos:memberList` values on one resource look like an
      > S35 violation and are consistent with SKOS (§9.6.2, Example 43), so they are reported as
      > our judgement rather than the specification's. A defective `rdf:List` entails nothing and
      > says how many items were read before the defect.
      > Production caller: `openbiz inspect <graph>`, proven end to end against the real binary
      > on disk — including that the store is byte-for-byte unchanged afterwards. See `adr/0019`.
      > **Scope, honestly:** the four core classes only. Labels, semantic relations, mapping
      > properties, and SKOS-XL are the items below and are not modelled here.
- [x] Labels: `prefLabel`, `altLabel`, `hiddenLabel`, per-language, with the one-preferred-label-
      per-language rule enforced
      > **Taken before SKOS-XL at iteration 21, deliberately.** SKOS-XL was listed first and it
      > depends on this: S55–S57 make the property chain (`skosxl:prefLabel`,
      > `skosxl:literalForm`) a sub-property of `skos:prefLabel`, so an XL label's whole point is
      > that it "dumbs down" to a plain SKOS label — and Appendix B.3.4.2 says the SKOS+XL
      > inconsistencies (Examples 84–87) are inconsistencies *because of* S13 and S14, which are
      > this item. Building XL first would have meant building the derived thing before the thing
      > it derives to. The order in the list was not a dependency, and now it is.
      > **The two integrity conditions the specification states are implemented and no others.**
      > §5.4 lists exactly two — S13, pairwise disjointness, and S14, at most one `skos:prefLabel`
      > per language *tag* — and both are enforced. S12's "range is the class of RDF plain
      > literals" is **not** one: §5.6.2 says an application "may reject such data but is not
      > required to", so a typed or IRI-valued label is reported as ill-formed and the vocabulary
      > still stands. Getting that the wrong way round would refuse valid enterprise data.
      > **The decision that needed the specification open is what "plain literal" means now.**
      > RDF 1.1 abolished the term; §3.3 of RDF 1.1 Concepts splits it into a language-tagged
      > string (`rdf:langString`) and a simple literal (`xsd:string`), and that pair is what we
      > accept. Language tags are compared lower-cased *in this crate*, not relied upon from the
      > engine, because `openbiz-skos` is engine-free and a parsed file or an agent's proposal can
      > hand us `@EN`. See `adr/0020`.
      > **Production caller:** `openbiz inspect` gained a `languages:` section — coverage per
      > language plus how many concepts have no preferred label in any of them, which is the
      > translation gap a multilingual programme manages — and now names schemes and collections
      > by their label rather than by IRI alone. Counts, never a list: every other section of that
      > report is bounded by the vocabulary's structure and labels are bounded by its size.
      > **Scope, honestly:** S11 (`rdfs:label` as a super-property) is quoted and not entailed, and
      > there is no BCP 47 lookup fallback when asking for a language. Both in `docs/UNTESTED.md`,
      > along with the memory claim `adr/0019` made and this item withdrew.
- [x] SKOS-XL labels as first-class resources (required for ISO 25964 fidelity — not optional)
      > **Done at iteration 22**, and **split in place**: this item is Appendix B.2 and B.3 — the
      > `skosxl:Label` class, `skosxl:literalForm`, the three XL labelling properties, and the
      > dumbing-down. B.4's `skosxl:labelRelation` is the item below.
      > **The dumbing-down is the point.** S55–S57 make the property chain (`skosxl:prefLabel`,
      > `skosxl:literalForm`) a sub-property of `skos:prefLabel`, so a concept labelled only
      > through SKOS-XL still has plain SKOS labels — by entailment, if somebody performs it. The
      > derived labels go into the **same map** as the asserted ones, each carrying a
      > `LabelOrigin`, because B.3.4.2 says Examples 84–87 are inconsistent *because of* S13 and
      > S14, which are conditions on that map. Keeping them elsewhere would report Example 84 as
      > a clean vocabulary. See `adr/0021`.
      > **Appendix B states no integrity conditions at all** — B.2.2, B.3.2 and B.4.2 are headed
      > "Class and Property Definitions", and §1.7 makes that heading meaningful. So the severity
      > of every SKOS-XL finding is ours to decide and the ADR records whose judgement each is: the
      > specification's for two literal forms (Examples 76–79 are marked "not consistent"), ours
      > for the disjointness rules and the datatype-property rule, ours for the two ill-formed
      > ones. A label with **no** literal form is deliberately *not* inconsistent — "cardinality
      > exactly 1" entails a form exists, it does not require the graph to state one, and calling
      > a partial export broken would refuse valid data.
      > **Production caller:** `openbiz inspect` gained a `skos-xl labels:` section, omitted
      > entirely for a vocabulary that does not use SKOS-XL so that its presence answers "is this
      > thesaurus using SKOS-XL?". Proven end to end against the real binary with a fixture that
      > states **no plain label anywhere**.
      > **Scope, honestly:** S59–S62 were not read at all, so a `skosxl:labelRelation` was
      > silently ignored — **closed by the item below at iteration 23**. Still open: the memory cost (an XL thesaurus holds each
      > label roughly twice) and the export gap, which SKOS-XL makes materially worse: our export
      > of an XL-authored thesaurus carries no plain labels at all.
- [x] `skosxl:labelRelation` — links between labels, and the ISO 25964 extension point (B.4,
      S59–S62)
      > Split out of the item above at iteration 22 and **done at iteration 23**. All four
      > statements are applied. S59 makes it an object property, so a literal there reuses the
      > finding S3 and S30 already raise. S60 and S61 make both ends a `skosxl:Label` — which
      > reports nothing by itself and is what makes a mistake visible: a link pointing at a
      > `skos:Concept` is caught by **S48**, and without the domain and range rules the same graph
      > reads as clean. S62 makes it symmetric, so a link entails its converse, and the converse
      > goes into the same map as the asserted direction carrying a `RelationOrigin` — the third
      > origin type in this crate, for the third time the same reason. A graph that states both
      > directions gets two asserted links and no derivation.
      > **The trap is B.4.4.1's last sentence** — "a sub-property of a symmetric property is not
      > necessarily symmetric" — so Example 89's `ex:acronym` must never be closed, because "FAO"
      > is an acronym for "Food and Agriculture Organization" and the converse is false. We read
      > no `rdfs:subPropertyOf` at all, so a refinement is invisible rather than mis-inferred, and
      > a test asserts that. See `adr/0022`, which also records why a label linked to **itself** is
      > deliberately not a finding: Appendix B does not forbid it, and inventing an integrity
      > condition the specification does not state is the incumbents' failure.
      > **Production caller:** `openbiz inspect` gained one line in the existing `skos-xl labels:`
      > section, counting **links** and not statements — S62 closes every link into a pair — and
      > printed only when there are any. Proven end to end against the real binary.
      > **Scope, honestly:** the *sound* half of sub-property reasoning is also not done — a
      > refinement's statement does not reach `skosxl:labelRelation` either, so a thesaurus whose
      > ISO 25964 label relationships are expressed through refinements, which is the ordinary way
      > B.4 is used, reads to us as having none. In `docs/UNTESTED.md`.
- [x] Semantic relations, part 1 — the links themselves and what they entail (§8, S18–S23, S25,
      S26)
      > Split in place at iteration 24 and **done at the same iteration**. The original item read
      > as one line and is ten specification statements, a closure, a domain-and-range pass and an
      > integrity condition that none of the others can be tested without; the split is at the
      > seam where the work changes shape, not at a convenient point in it.
      > Eight statements applied. **S18** makes all six object properties, so a literal reuses the
      > finding S3, S30 and S59 already raise. **S25, S26 and S23** close the inverses — a
      > hierarchy written downwards reads upwards and back, which is what makes two merged sources
      > comparable — and **S22** lifts both directions into the transitive variants. The inverse
      > pass runs *first*, and a test compares the whole model of a graph written with
      > `skos:broader` against the same graph written with `skos:narrower`: the two are identical,
      > and would not be if the lift ran first.
      > **S19 and S20 are the domain and range, and they constrain `skos:semanticRelation`** — a
      > property no author writes. So the citation for a concept typed out of a `skos:broader`
      > runs S22 → S21 → S19/S20 and the report prints every step, because citing S19 against the
      > `skos:broader` statement itself would name a statement that does not mention the property
      > the author used. The S21 step is recorded only when a class actually follows from it, so
      > a vocabulary that types its own concepts sees none of it.
      > Polyhierarchy is **counted and never reported**: §8 states nothing against a concept with
      > two parents and ISO 25964 relies on it. See `adr/0023`, which also records why a concept
      > related to itself is not a finding, and why the S48-style fan-out iteration 23 worried
      > about does not occur here — it came from entailing a *constrained* class, and
      > `skos:Concept` is not one. That is now an assertion in the end-to-end test, not a claim.
      > **Production caller:** `openbiz inspect` gained a `semantic relations:` section, omitted
      > entirely for a vocabulary with no links so that its presence answers "does this vocabulary
      > have a hierarchy at all?". Proven end to end against the real binary.
      > **Scope, honestly:** S24 and S27 are the item below and are *not* done, so
      > `skos:broaderTransitive` holds one-step links only and §8.4 is unchecked — four of §8.6's
      > five examples are marked "not consistent" by the specification and read as clean to us
      > today. A test pins that rather than leaving it to be found. Also new: the model now holds
      > something that scales with a vocabulary's *size* rather than its structure, four entries
      > per stated link, and the ceiling is unmeasured. Both in `docs/UNTESTED.md`.
- [x] Semantic relations, part 2a — measure what the model holds, and decide where the closure
      lives before it is built
      > Split out of the item below at iteration 26, because that item's own text made the
      > measurement a prerequisite of its design and iteration 24's loop log said in as many words
      > that starting S24 without a number in front of it should stop and get one first.
      > **Done:** `crates/openbiz-skos/src/scale.rs` reports build time, resident memory, held
      > entries, derivations, report size and the S24 closure's size at 10k, 100k and 1M links
      > across four hierarchy shapes — no links, a star, a balanced tree, and a chain. The closure
      > is **counted by traversal and never held**, so its size is knowable without paying for it,
      > and a count past its budget is recorded as a refusal rather than as a zero.
      > **Decided, `adr/0024`:** S24's closure is **never materialised**. A legal 100 000-link
      > chain licenses five thousand million pairs, and a stored `(Node, RelationOrigin)` can cite
      > S24 but cannot name the path it took, which `CLAUDE.md` §3 requires of every inference.
      > Ancestry is a bounded traversal answered on read, and a bounded answer says that it was.
      > **Production caller:** none, and by design — this is a measurement spike in the shape of
      > `adr/0013` and `adr/0014`, and its output is a decision the next item is bound by. Its small
      > case runs in the ordinary suite and asserts the arithmetic of every shape, so the harness
      > cannot rot into measuring nothing.
      > **What it found that nobody asked for:** a stated link costs **3.9 KiB resident**, 43× the
      > size of the fact, and a million-link vocabulary with no labels at all held **4.4 GiB** and
      > took **62.66 s** to build. That is against §1.5 and it is about what is already shipped,
      > not about what comes next. Two `UNTESTED.md` entries and three proposals; **not** fixed
      > here, because each fix changes a shipped public type and is its own item.
- [x] Semantic relations, part 2b — the transitive traversal and §8.4's integrity condition (S24,
      S27)
      > Split out of the item above at iteration 24, and split again at iteration 26. S24 makes `skos:broaderTransitive` and
      > `skos:narrowerTransitive` `owl:TransitiveProperty`; S27 makes `skos:related` disjoint with
      > `skos:broaderTransitive`. They are one item because **S27 cannot be tested without S24**:
      > §8.6's Examples 27 and 29 are inconsistent only through the closure, and a build that
      > applied S27 to the one-step links would report Examples 26 and 28 and pass 27 and 29 —
      > a validator that answers "consistent" for a graph the specification marks otherwise, which
      > is worse than one that says nothing.
      > Needs: cycle containment (a vocabulary with `<A> broader <B> broader <A>` must terminate
      > and must not be reported as broken — §8 states no condition against a cycle), and a
      > derivation that names each step of the path rather than asserting the endpoint.
      > **Bound by `adr/0024`:** build a traversal, not a closure. Nothing is added to
      > `Resource::relations`, which keeps meaning "links under this property" and never
      > "ancestors" — permanently, and by design. The traversal is bounded and an answer that hit
      > its bound is distinguishable from one that ran out of ancestors, because `Some(0)` from an
      > abandoned walk reads as "this concept has no ancestors" for the concept that has most.
      > Acceptance: §8.5's Examples 25–29, all five, each asserted to the consistency the
      > specification prints beside it.
      > **Done at iteration 28**, and the acceptance test is one test asserting all five, because
      > the point of the set is the contrast: 25 is consistent and 26–29 are not, and a build that
      > got them all wrong in the same direction would pass four of five split tests. §8.6's
      > Examples 33, 36 and 37 are covered too — a concept related to itself, broader than itself,
      > and a cycle are each consistent and none is a finding.
      > **The closure is a walk and is never stored**, exactly as `adr/0024` bound it:
      > `CoreModel::ancestry` is a bounded breadth-first traversal, `Resource::relations` still
      > means "links under this property", and the path falls out of the walk so a transitive
      > conclusion cites the chain rather than asserting the endpoint. See `adr/0025`.
      > **A bound that was hit is now sayable.** `Severity` gained `Unchecked` and
      > `CoreModel::checks_are_complete()` sits beside `is_consistent()`, so `openbiz inspect`
      > closes with one of three sentences instead of two — a check that gave up no longer reads
      > as a check that passed. That closes half of the `UNTESTED.md` entry iteration 24 opened;
      > the report still does not enumerate which conditions it checked, and that half is still
      > open.
      > **Production caller:** `openbiz ancestors <graph> <concept>`, which prints every concept
      > above one and the path that reached it, proven end to end against the binary on disk. S27
      > reaches the operator through `openbiz inspect`, which now reports Example 27's indirect
      > clash — a graph it called clean until this iteration.
      > **Scope, honestly:** the walk goes **up** only. Descendants are the same function with the
      > inverse property and have no caller, so they arrive with the concept-tree item. Nothing
      > measures what the walk costs at size — `adr/0024` measured storing and this stores nothing
      > — which is `UNTESTED.md`'s replacement entry and the reason the default bound is a
      > judgement rather than a budget.
- [x] Documentation properties, part 1 — the seven properties and S17: `note`, `definition`,
      `scopeNote`, `example`, `historyNote`, `changeNote`, `editorialNote`
      > **Split in place at iteration 29.** The item as written named five properties; SKOS §7 has
      > **seven**, and it has an extension point besides. This part is the seven and the one
      > inference §7 licenses. Part 2 below is the extension point, split out because it needs a
      > second pass over a stream the builder reads once — an architectural change, not a
      > continuation.
      > **The load-bearing fact about §7 is a negative one: it states no integrity condition.**
      > §5.4 has an "Integrity Conditions" heading and §7 has no such subsection at all, so nothing
      > here can make a graph inconsistent and nothing here raises a `Finding` of any severity.
      > That is a deliberate refusal, not an omission: **a concept with no `skos:definition` is
      > consistent SKOS**, every incumbent flags it, and the check they are running is ANSI/NISO
      > Z39.19 or ISO 25964 — a rule pack in `openbiz-validate`, where it can be named and switched
      > off, not a SKOS finding citing a statement the specification never made. `openbiz inspect`
      > says so in the report, in the same breath as the count, so a zero is not read as our
      > verdict. A test asserts the absence, so nobody can add one later without deleting it.
      > **S16 constrains the value not at all**, so a note is a bare `Term`: Examples 22 (a literal)
      > and 23 (an IRI) are both marked consistent, and the two node-shaped usage patterns §7.1
      > names are indistinguishable from the statement alone, so we do not guess between them.
      > **And the object of a note is not typed** — §7 has no range, unlike S19/S20 on
      > `skos:semanticRelation` — so Example 23's `<MyNote>` joins no vocabulary.
      > **S17 is materialised, not walked**, which is the opposite of `adr/0025`'s decision for S24
      > and for a stated reason: the lift is one step deep, cannot chain, and adds at most one
      > entry per stated note. It runs upwards only, so a bare `skos:note` never acquires a more
      > specific kind, and an asserted note is never overwritten by an entailed one.
      > **Production callers: two.** `openbiz notes <graph> <resource>` prints everything a
      > vocabulary documents one resource with, and beside each note SKOS entailed, the statement
      > it came from and the quoted rule — which is the one thing a Turtle export cannot show.
      > It takes a *resource*, not a concept, because §7's own Example 24 documents an `owl:Class`.
      > `openbiz inspect` gains a documentation coverage table, counts rather than content for the
      > reason the languages section is counts. Both proven against the binary on disk.
      > **Acceptance:** §7's Examples 22, 23 and 24, all three, plus a test asserting §7's silence.
      > See `adr/0026`.
- [x] Documentation properties, part 2 — the extension point: a vocabulary's own
      `rdfs:subPropertyOf` refinements of the seven
      > Split out at iteration 29, landed at iteration 31. §7.1's "set of extension points for
      > defining more specific types of note" is read: `ex:usageNote rdfs:subPropertyOf
      > skos:scopeNote` plus a statement made with `ex:usageNote` entails a `skos:scopeNote` under
      > RDFS `rdfs7`, and S17 then entails a `skos:note`. A chain the graph controls is resolved
      > with a cycle guard and a bound, and its composition is derived once for the vocabulary
      > citing `rdfs5`.
      > **Two passes over the source, not a buffer.** A declaration can arrive after every
      > statement that uses it, so a single pass would have to hold every unrecognised statement
      > until the declarations were in — which breaks `inspect`'s own promise that peak memory is
      > the model rather than the graph. The first pass reads `rdfs:subPropertyOf` and keeps the
      > *property* graph, which is schema-sized. See `adr/0028`.
      > **The budget is shared across the resolution**, not per property — `adr/0027`'s finding
      > applied before the fact rather than after it, with a test proven to fail against a
      > per-property mutant.
      > **`Derivation.rule` is now `Rule`**, either a SKOS statement or an RDFS entailment pattern,
      > because citing an S-number for something SKOS does not state would be a guess wearing a
      > citation.
      > **Production callers: both existing ones.** `openbiz notes` prints the property the
      > vocabulary actually used, the declaration, and the rule; `openbiz inspect` counts refined
      > notes apart from S17 lifts and **names** the declared properties. Both proven against the
      > binary on disk — which is how the S17-premise defect in `openbiz notes` was found, with the
      > whole suite green.
      > **Scope, honestly:** `skosxl:labelRelation`'s refinement is still not read. It was meant to
      > be the same mechanism and the resolution is written to accept it, but B.4.4.1 forbids
      > closing a refinement of a symmetric property, which is a decision this item does not make.
      > `UNTESTED.md`'s iteration-23 entry stays open and it is a proposal.
- [x] Mapping properties, part 1 — the five properties, the sub-property lattice, and §10's
      only integrity condition: `exactMatch`, `closeMatch`, `broadMatch`, `narrowMatch`,
      `relatedMatch`
      > Split from one item at iteration 32 and the first half landed there. §10's S38–S44 and S46
      > are applied; **S45 is not** and is part 2 below, so the split is visible rather than
      > implied.
      > **The load-bearing decision is that a mapping is not a section of its own.** S41 makes
      > `skos:broadMatch` a sub-property of `skos:broader`, so the mapping links are closed
      > *before* §8's pass and lifted into it — which means `openbiz ancestors` climbs through a
      > mapped concept into another vocabulary's, and §8.4's S27 catches Examples 59, 60 and 61
      > with no rule of §10's own. A build that kept them apart would report every mapped
      > thesaurus as flat and every mapped vocabulary as an island, which is the silo
      > `CLAUDE.md` §1.7 exists to prevent. See `adr/0029`.
      > **S43 and S44 are closed, S42 lifts every exact match to a close match, and S40 then S39
      > carry both ends up to `skos:Concept`** — through `skos:mappingRelation`, in two printed
      > steps, because S19 constrains a property no author writes. `skos:exactMatch` reaches the
      > super-property through S42 rather than directly, since S40 does not name it, and the
      > derivation prints that step rather than skipping it. Examples 54–57.
      > **S46 is one finding per pair, not one per end**, and it distinguishes the two arguments
      > the specification makes: S46 names `skos:broadMatch` and `skos:relatedMatch` outright, and
      > §10.4's note reaches `skos:narrowMatch` through symmetry and inversion. Quoting S46 flatly
      > at a `skos:narrowMatch` clash would cite a statement that does not mention the property in
      > front of the reader.
      > **What §10 permits, we do not report**, each with a test asserting the silence: a mapping
      > inside one concept scheme (Example 58), a reflexive mapping (Example 66), and cycles and
      > alternate paths in `skos:broadMatch` (Examples 67, 68) — which after S41 are cycles in
      > `skos:broader` that the ancestry walk has to survive rather than complain about.
      > **Production callers: two, and one of them found a defect.** `openbiz inspect` gains a
      > mapping section, and `openbiz ancestors` now walks through a mapping link — both proven
      > against the binary on disk. Running the report showed the *existing* semantic-relations
      > line calling a link lifted under S41 "stated as skos:narrower", which is a statement the
      > author never wrote; the two origins are now counted apart.
      > **Acceptance:** §10's Examples 49–61 and 63–68, plus a test pinning S45's absence.
- [x] Mapping properties, part 2 — S45's transitivity, walked rather than stored, and a
      per-concept view of what a vocabulary is mapped to
      > **Done at iteration 33.** `CoreModel::exact_match_cluster` walks the closure and nothing
      > stores it, so Example 62 is entailed and `Resource::mappings_of` still means "one-step
      > links" — both halves are asserted, so a later build that materialises the closure fails a
      > test. The walk's shape is a **connected component and not a path upwards**, because S44 has
      > already put every link at both ends: cycles are ordinary rather than pathological (§10.6.6
      > requires an application to cope with them), and a concept with any exact match is its own
      > exact match, which §10.6.6's Example 66 marks consistent and the report prints rather than
      > hides.
      > **The sharpest half is S46 across that closure.** A vocabulary stating
      > `<A> exactMatch <B>`, `<B> exactMatch <C>` and `<A> broadMatch <C>` violates §10.4, and no
      > statement in it names both properties for one pair — so it was reported as *consistent*
      > until this landed. That is the hub shape an enterprise actually produces, and a false "no
      > violation" is worse than a missing conclusion. The two S46 passes cannot double-report,
      > because only chains of two links or more reach the second.
      > **Production caller:** `openbiz mappings <graph> <resource>` for the walk, and the model's
      > own build for the sweep. The command prints the five properties, the origin and quoted rule
      > for every link the graph did not state, S41's lift per section, and the chained concepts
      > with the chain that reached each.
      > `openbiz inspect`'s sentence claiming S45 was unimplemented became false with this commit
      > and was replaced rather than deleted: the counts are still one-step links, and the report
      > says so and names the command that resolves them.
      > **Not done, and in `UNTESTED.md`:** S42's lift is not applied across the closure, so a
      > chained concept is listed as an exact match and not also as the close match S42 entails;
      > and the sweep's cost is unmeasured on a mapped vocabulary of any size, which is now the
      > second iteration running to record that the scale harness generates no mapping links.
      > See `adr/0030`.
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
      > Amended by iteration 25's research, and the amendment is **not** authorised into the item —
      > it sits in `docs/PROPOSED.md` awaiting promotion. Two things changed: this line does not say
      > **which SHACL** (1.0 is the only Recommendation; SHACL 1.2 Core has been in Working Draft
      > since 2026-08-03 with a materially larger surface), and the engine list is now incomplete —
      > `purrdf-shapes` is new, and `shacl_validation` has 20× `oxirs-shacl`'s adoption.
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
      > Note from iteration 25: **`whelk-rs` is not published to crates.io.** MIT-licensed and the
      > repo is alive, so this is a supply question rather than a licence one — but `deny.toml`
      > sets `unknown-git = "deny"`, so a git dependency fails CI by policy today. The options are
      > in `docs/PROPOSED.md`; the question should be settled before this item is picked up.
- [ ] Consistency checking with a human-readable account of the inconsistency
- [ ] **Explanation**: every inferred triple can produce its full derivation chain, rendered for a
      non-logician. No inference path may ship without this (`CLAUDE.md` §3)
- [ ] Incremental re-reasoning on edit, fast enough for interactive use
- [ ] Materialised inferences visibly distinguished from asserted facts everywhere in the UI
- [ ] Document the DL gap honestly in user-facing docs — we support EL and RL, not full DL
      > Note from iteration 27: `README.md` already carries a first statement of this gap, and its
      > wording was **corrected on 2026-08-19** — the absolute "there is no Rust DL reasoner" is
      > retired in favour of "none mature enough to depend on". Start from that phrasing; do not
      > reintroduce the absolute. See the retired-claims table in `docs/COMPETITIVE.md`.

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
      > **BLOCKED on a licence, since iteration 25.** `horned-owl` is **LGPL-3.0** (verified three
      > ways on 2026-08-18), which `CLAUDE.md` §5 forbids in the core. §5 routes copyleft to
      > `docs/BLOCKED.md` and stops, because it is a commercial decision a human takes — see that
      > entry for the four options and their costs, and `docs/PROPOSED.md` for the request to pick
      > one. **Do not start this item, and do not substitute a weaker dependency to get round it.**
      > The dependency named in this line may not survive the decision, so the line itself is
      > provisional.
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
