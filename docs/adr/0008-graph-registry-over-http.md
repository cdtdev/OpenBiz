# ADR 0008 — The graph registry over HTTP, and who decides what a user sees

- **Status:** accepted
- **Date:** 2026-08-18 (iteration 6)
- **Supersedes:** nothing. Extends `adr/0007` (the named-graph model) with its read surface.

## Context

`adr/0007` built the named-graph model inside the store: one graph per vocabulary, a system graph
for OpenBiz's own metadata, `urn:openbiz:` reserved against user authoring, and a registry in the
system graph that is re-validated on read. Its only production caller was a log line — `main` reads
the registry before binding and refuses to start if it cannot describe it. Nothing a user could
reach knew that vocabularies existed.

This ADR records the decisions taken exposing it: `GET /api/graphs` and a `Vocabularies` component
in the interface. It is the **read** half only.

## Decisions

### 1. The API returns the whole registry; the UI decides what a taxonomist sees

The obvious design is to filter OpenBiz's own graphs out of the endpoint, so that a client cannot
show them by accident. We did the opposite: `GET /api/graphs` returns every registered graph with
its `kind`, and `ui/src/Vocabularies.tsx` shows only the vocabularies.

Both halves of that are load-bearing, and the incumbents get one or the other wrong.

- **VocBench** exposes the triplestore's own support graphs in the same list as the user's content.
  A subject-matter expert is then asked "which graph does this go in?" — a question about our
  implementation that they have no way to answer. That is the failure the UI filter prevents.
- **Filtering in the API** would trade that failure for a different one: an operator asking "what is
  actually in my store?" would get an answer that omits rows, with nothing saying so. A governance
  tool whose inventory endpoint quietly under-reports is exactly the opacity `CLAUDE.md` §1 exists
  to attack, and a `?kind=` filter added later could not undo a client that had learned to trust the
  short list.

So the separation lives in the layer that has the user in front of it, and `kind` is on the wire so
that layer can do its job without the API having to lie.

**The graphs the UI holds back are counted, not hidden.** The interface says "1 further graph is
held for OpenBiz's own use and is not shown here." A list that silently drops rows invites the
question of what else it dropped; a list that states the count answers it. This is the same instinct
as `adr/0007`'s refusal to skip an unreadable registry row.

### 2. `openbiz_api::GraphKind` is a separate type from `openbiz_store::GraphKind`

A re-export would have been fewer lines. It would also have made the wire format a shadow of an
internal model: renaming a registry token — an on-disk change needing a migration — would silently
have become an API break, and adding a fourth kind to the store would have started appearing in
JSON with no decision taken.

They are separate types with an exhaustive `match` between them (`graphs::on_the_wire`). A new store
kind now **fails this build** until somebody decides what it is called on the wire. The tokens are
asserted against literal strings in `openbiz-api`'s tests, so a rename is a visible break rather
than a refactor.

### 3. Store errors are logged in full and reported in outline

`StoreError` is written for an operator: it names the store's path, and a corrupt registry names the
IRI of the offending graph. Both are facts about a customer's deployment and their vocabularies, and
this endpoint has no authentication in front of it (Phase 7). So `From<StoreError> for Failure`
logs the error at `error` level with its own words, and returns a 500 carrying only *"the graph
registry could not be read; the server log records why"*.

This is a real cost, and it cuts against the explainability commitment in `CLAUDE.md` §3. The
mitigation is that the operator loses nothing — the full error is in the log at the moment it
happened — and the decision should be revisited when there is an authenticated administrative role
to return the detail to. It is recorded here rather than left as an unexplained string.

### 4. `POST /api/graphs` is a 405, not an absent route

`CLAUDE.md` §1.7 requires discovery to run before creation and a recorded justification when
something new is created anyway. `DiscoveryProvider` does not exist until Phase 2, so there is no
honest creation path to expose, and adding one would be a charter violation wearing the costume of
progress.

Mounting the path with only `GET` means a write attempt gets a 405 — "this resource exists and does
not accept that" — rather than a 404 that reads as "wrong URL". The distinction is asserted by a
test, so the day creation arrives it replaces a documented refusal rather than filling a silence.

The interface makes the same statement in the empty state a new deployment sees: *"No vocabularies
yet. Before creating one, OpenBiz will look for an existing vocabulary that already serves — reuse
outranks creation."* No "New vocabulary" button, because §1.7's whole point is that creating must
not be the cheapest thing on the page.

### 5. The store is shared with the router through an `Arc`, and reclaimed before close

`Store::close` consumes the store, so `main` cannot both hand it to the router and close it. It
wraps it in an `Arc`, gives the router a clone via `AppState`, and after `axum::serve` has drained
calls `Arc::into_inner`.

If that reclaim fails, something still holds a clone — a leaked task, a connection that outlived the
drain — and `main` **fails with an error saying so** rather than skipping the flush. A silent skip is
precisely the failure `Store::close` was built to make impossible: an operator reading a clean
shutdown log while the last writes never reached disk.

`tests/graceful_shutdown.rs::the_graph_registry_is_served_over_http_and_the_store_still_closes_cleanly`
serves a real request from the real binary *before* signalling it, because a connection is what
clones the state — it is the only test in which the reclaim can actually fail.

### 6. The registry is read per request, not cached

The registry is small and it is the store's own metadata. A cache would need invalidating by every
future creation, import, and restore path, and a stale *"your vocabulary does not exist"* is a worse
failure than a scan nobody has measured. That it is unmeasured above a handful of graphs — and that
it now sits on the request path as well as the startup path — is recorded in `docs/UNTESTED.md` and
owed a number by the Phase 1 benchmark spike.

## What was measured

- 97 Rust tests (was 84) and 22 UI tests (was 10); `cargo fmt`, `cargo clippy -D warnings`,
  `cargo test --workspace`, `cargo deny check licenses`, and the UI typecheck/build/test all green.
- **Thirteen mutants, all killed.** Rust: reporting inferred graphs as vocabularies; answering 200
  instead of 500 on a store failure; putting the store's own error text in the response body;
  filtering OpenBiz's graphs out of the *API*; not mounting the route at all. UI: showing every
  graph as a vocabulary; counting all graphs as held back; announcing "0 further graphs"; inverting
  the singular/plural; rendering an empty list instead of the empty state; trusting a non-2xx body;
  reporting an abort as a failure; never aborting on unmount.
- Verified by hand against the running binary: `GET /api/graphs` → `200`
  `{"graphs":[{"iri":"urn:openbiz:graph:system","kind":"system"}]}`; `GET /api/graphss` → `404`;
  `POST /api/graphs` → `405`; `SIGTERM` → `store closed cleanly`.

## Consequences

- `openbiz-api` now has a dependency on `serde_json` — dev-only, for the tests that pin the wire
  format. Nothing is added to the shipped binary.
- `app()` takes an `AppState`. Every future test that builds a router needs a store; `ui`'s tests
  share one opened once per test binary, because they assert on the fallback and never touch it.
- `ui/src/useProbe.ts` now holds the fetch-once-on-mount logic that was inline in `App`. It was
  extracted rather than copied because the parts that are easy to get wrong — checking the status
  before parsing the body, aborting on unmount, staying silent on an abort — are exactly the parts a
  second copy would let rot. `App`'s ten existing tests passed unchanged across the extraction,
  which is what made it safe to do in the same iteration as the feature.
