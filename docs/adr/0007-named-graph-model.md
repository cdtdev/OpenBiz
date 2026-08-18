# ADR 0007 — The named-graph model: one graph per vocabulary, a reserved namespace, and one write choke point

**Status:** accepted (2026-08-18) · **Phase:** 1

## Context

Phase 1's third item is "named-graph model: one graph per vocabulary, plus a system graph for
OpenBiz's own metadata". Before this iteration, `GraphId`, `GraphKind`, and
`GraphId::is_directly_writable()` existed in `openbiz-store` and **nothing constructed one outside
a test** — `UNTESTED.md` recorded it as the store's largest no-production-caller gap, and
`StoreError::NotWritable` was a variant nothing could return. The model was designed, not
delivered.

The decisions below are cheap now and very expensive later, because every phase above Phase 1
inherits them: an export boundary (Phase 11), a permission boundary (Phase 6), a diff boundary
(Phase 7), and the asserted-versus-inferred boundary every "why?" explanation rests on
(`CLAUDE.md` §3).

### What the incumbents do badly here

Parity is not the target. Every RDF tool has named graphs; the question is what they do with them.

- **PoolParty** and **TopBraid EDG** both keep project metadata in the same store as the content,
  and both are known for exports that carry tool-specific bookkeeping a standards-compliant
  consumer then has to be told to ignore. The round-trip in `CLAUDE.md` §1.3 is exactly what that
  breaks.
- **VocBench 3** exposes the underlying triplestore's graph structure to the user, including its
  own support graphs, so "which graph do I put this in" becomes a question a subject-matter expert
  is asked and cannot answer.
- Across all of them, **inferred triples are commonly materialised into the same graph as asserted
  ones**, or into a graph the user can also write to. Once that happens, "is this fact something a
  human stated or something a reasoner derived" is unanswerable, and every audit conversation that
  depends on the distinction is lost.

## Decision

**1. Every quad lives in a named graph. Nothing is written to the default graph, ever.**
A quad with no graph cannot be exported, versioned, permissioned, or attributed, and "which
vocabulary is this statement part of?" is the first question every later phase asks. The rule is
enforced structurally: the only write path takes a `&GraphId`.

**2. Three kinds, and the distinction is load-bearing rather than descriptive.**
`Vocabulary` (one user-authored artefact per graph), `System` (OpenBiz's own bookkeeping — the
format stamp, the graph registry, later workflow and provenance), and `Inferred` (materialised
entailments, derived and never asserted).

**3. `urn:openbiz:` is reserved, and a vocabulary graph may not be inside it.**
Without this a user can register a vocabulary at the system graph's own IRI — or at an inferred
graph's IRI — and acquire write access to our bookkeeping through a path that looks like ordinary
authoring. `GraphId`'s fields are private, so the pairing of IRI and kind is an invariant the type
enforces rather than a convention callers follow. `urn:` and not `http:` because we do not own a
domain, and minting under someone else's namespace — or at an IRI that 404s — is worse than being
honestly non-dereferenceable.

**4. An inferred graph's IRI is derived, not chosen:** `urn:openbiz:graph:inferred:<vocabulary
IRI>`. Two vocabularies therefore cannot share an inferred graph, materialisation cannot be aimed
at a graph a human authored, and the derivation is readable straight off the IRI.

**5. One registry, in the system graph, and it is the source of truth for "what graphs exist".**
Two quads per graph: `rdf:type urn:openbiz:Graph` and `urn:openbiz:graphKind "<kind>"`. Listing
asks the registry rather than the backend, for two reasons — asking the backend which graphs
contain quads is a whole-store scan, and it would also miss a vocabulary that has been created but
not yet populated, which is precisely the vocabulary a user is looking at when they wonder where it
went.

**6. Reading the registry re-applies every invariant that writing did.** The registry is data on
disk: a hand-edited store, a doctored backup, or a build with a bug must be *refused*, not trusted.
An unrecognised kind token is `Corrupt`, not a silent downgrade to "vocabulary" — the same class of
mistake as misreading a format version, and the same answer.

**7. One write choke point, and the writability rule is enforced there.** `insert_into` is the only
function in the crate that writes, it requires a `&GraphId`, and it refuses a target that is not
directly writable. The format stamp goes through it too. Today every production write targets the
system graph, so the refusal branch does not fire in production — the point of putting the choke
point in now is that the first import, materialisation, or agent proposal to arrive **cannot route
around it**. This is `CLAUDE.md` §3's "design for assistability from the first phase" applied to
the write path.

**8. Registry writes are atomic.** A registry entry is two quads; a process that died between them
would leave a graph that half exists, which is worse than one that does not exist at all. Oxigraph
0.5's `Store::extend` commits a set of quads in one transaction, so this needed no new machinery —
and deliberately does not pre-empt the *public* transactional write API, which is the next plan
item.

**9. The registry is additive, so acquiring one is not a format change.** A store written before
the registry existed gains one by being opened; `FORMAT_VERSION` stays at 1. An older build reading
such a store simply does not look for the quads. Bumping the format for every additive piece of
system metadata would make the store far harder to evolve than it has to be, and would spend a
migration on something that needs none. Tested:
`a_store_without_a_registry_gains_one_on_open_without_a_format_bump`.

**10. Creating a graph at an IRI that is already registered is refused, and the message points at
the reuse ladder.** This is the store-level face of `CLAUDE.md` §1.7. Quietly adopting an existing
graph is how two vocabularies end up believing they own one, and it makes "create another" the
cheapest action available — the exact behaviour this product exists to attack.

## What was measured

- 36 tests in `openbiz-store` (was 12) and one new end-to-end test against the real binary.
- **Twelve mutants, all killed.** Dropping the writability guard in `insert_into`; dropping it in
  `create_vocabulary_graph`; dropping the already-exists check; not registering the system graph at
  open; removing the sort from `graphs()`; making `from_registry` trust what it reads; defaulting
  an unknown kind token to `Vocabulary`; not enforcing the reserved namespace; not validating IRI
  syntax; matching the reserved namespace with `contains("openbiz")` instead of a prefix; and two
  against `main.rs` — never reading the registry, and reading it but not reporting it. A thirteenth
  mutation, assembling a `GraphId` from a registry row by struct literal, **would not compile**,
  because the fields are private. That is the invariant working as designed rather than as
  documentation.
- IRI validation is delegated to the backend's parser, so what we accept is exactly what the store,
  the serialisers, and SPARQL accept. A second, hand-rolled notion of validity would drift, and the
  drift would surface as an export that will not re-import.

## Consequences

- `GraphId::vocabulary` now returns `Result`. Callers validate at construction, so a bad IRI is
  rejected while the user still has the form open rather than surfacing as a backend error three
  layers down.
- `main.rs` reads the registry before binding and fails startup if it cannot be described. Same
  footing as a store that will not open: better never up than up and wrong. The count is logged at
  `info`; the IRIs at `debug`, because vocabulary IRIs are customer metadata.
- `create_vocabulary_graph` has **no production caller yet** and will not get one until the
  discovery-first creation path exists (`CLAUDE.md` §1.7 — a creation path that skips
  `DiscoveryProvider` or records no justification is a charter violation, not a shortcut).
  Recorded in `UNTESTED.md` rather than papered over with a placeholder endpoint.
- The registry's *external* representation is a separate question. This is internal bookkeeping in
  our own namespace; the catalogue interop surface is DCAT 3, which is a Phase 11 item, and nothing
  here should be read as the shape we publish.
