# Untested & unproven

Things built but not proven, or proven only narrowly. **Write here the moment you notice, not at
the end of the iteration** — the gap you defer recording is the one you forget.

This file existing is not a failure. A loop that never records a gap is not a loop with no gaps; it
is a loop that has stopped looking. What *is* a failure is a gap that never closes.

What belongs here:
- **Built but no production caller.** Code that exists and is unit-tested but nothing invokes.
  This is the most important category and the easiest to rationalise away — record it every time.
- Happy path tested, failure paths not.
- A standard partially implemented, where we must not claim full support.
- Anything needing hardware, an account, or infrastructure this machine lacks — mark these
  **environment-limited** so the monitor can tell a permanent floor from real rot.
- Behaviour verified by inspection rather than by a test.

Strike an entry through (`~~like this~~`) when it closes, with a one-line note on what closed it.
Do not delete it — the record of what took how long to close is the signal.

## Entry format

```
### <what is unproven>
- **Kind:** no-production-caller | partial-coverage | partial-standard | environment-limited | inspected-only
- **What is proven:** …
- **What is not:** …
- **What would close it:** …
- **Opened:** iteration N
```

---

### The service-defined default dataset is believed spec-permitted, not verified against the text
- **Kind:** partial-standard
- **What is proven:** the behaviour itself is thoroughly tested — a query naming no dataset sees the
  vocabulary graphs and nothing else, a query naming its own `FROM` reaches ours verbatim, and a
  store with no vocabularies answers nothing rather than everything.
- **What is not:** that this is *conformant*. `adr/0011` chooses a default dataset that is not "the
  store's default graph", and the justification rests on SPARQL 1.1 permitting a service to define
  its own default dataset when a query specifies none. That reading was **not checked against the
  specification's own words** this iteration — it is recalled, not cited, and `CLAUDE.md` §4.5 says
  a standards claim needs the spec behind it. If the reading is wrong, this is a documented
  deviation rather than a conformant choice, and the honest word for it in user-facing text changes.
- **A second, narrower gap even if the reading is right:** query *portability*. The same query text
  returns different answers here and against a standards-configured endpoint over the same data.
  That is a property of every service-defined default dataset, and nothing warns a user copying a
  query in or out. A SPARQL Service Description at the endpoint would be the standard way to make
  the dataset self-describing rather than documented-elsewhere; we do not serve one.
- **What would close it:** read SPARQL 1.1 Query §13 and SPARQL 1.1 Protocol §2.1.4, cite the
  clause in `adr/0011` or correct the ADR, and consider a Service Description.
- **Opened:** iteration 9

### SPARQL 1.1 Protocol is implemented for query only, and two of its parameters are refused
- **Kind:** partial-standard
- **What is proven:** all three of the protocol's request forms for a *query* — `GET ?query=`,
  `POST application/sparql-query`, and `POST` form-encoded — are tested end to end against the
  router, and all four result formats are tested for both `SELECT` and `ASK`.
- **What is not:** the protocol's `default-graph-uri` and `named-graph-uri` parameters are a named
  400, not an implementation. There is no Update endpoint (its own plan item) and no Graph Store
  Protocol (likewise). So the honest claim this build supports is **SPARQL 1.1 Query over the
  protocol's three query forms**, not "SPARQL 1.1 Protocol". Do not write the latter anywhere
  user-facing.
- **What would close it:** deciding how a protocol-supplied dataset composes with the
  vocabulary-graph default of `adr/0011` *and* with a query's own `FROM` — the three-way
  interaction is the reason it was deferred rather than guessed — then implementing it with the
  spec's own examples as tests.
- **Opened:** iteration 9

### The SPARQL endpoint has no query console in the interface
- **Kind:** no-production-caller
- **What is proven:** the endpoint's production caller is HTTP: routed at `/api/sparql`, tested
  through the real router, and usable from `curl` or any SPARQL client.
- **What is not:** nothing in the UI calls it. `CLAUDE.md` §4.4 requires anything user-facing to be
  reachable in the interface and keyboard-navigable, and a query console is plainly user-facing.
  The plan item is scoped to the endpoint, so this is recorded as an open gap rather than folded
  into that item and quietly called done — but a taxonomist cannot run a query in OpenBiz today.
- **What would close it:** a console in the interface — an editor, a format chooser reading the
  server's own list, a results table, and the refusals rendered as text rather than as a status
  code. It is a UI item and it is not in the plan; recorded in `PROPOSED.md`.
- **Note, same iteration:** `GET /api/sparql/formats` was added so the list a console needs is
  already served, and so that `preserves_term_detail` has a production reader rather than being a
  constant only its own test consults. It has an HTTP caller and **no UI caller** — the warning
  that CSV silently drops language tags is now *available* to an interface and still not *shown* to
  a user, which is the half of this gap that closing the endpoint did not close.
- **Opened:** iteration 9

### The endpoint buffers a whole answer in memory, twice
- **Kind:** partial-coverage
- **What is proven:** the buffer is *deliberate* and load-bearing. `Store::query` may leave a
  partial document in its writer when it refuses, and a truncated results document is syntactically
  valid and semantically wrong, so the HTTP layer buffers into a `Vec` and only builds a response
  on `Ok`. Tests assert a refused query's body carries the refusal and none of the partial answer.
- **What is not:** the cost. A `SELECT` answering at the 100 000-row cap holds the serialised
  document in a `Vec` and then again in the response body, and no test measures either. The same
  gap as the export endpoint's, one layer up, and with a larger worst case because the cap is
  bigger than any vocabulary we have.
- **What would close it:** a real measurement at the cap, then either a bounded streaming body that
  can still discard a partial document, or a documented maximum response size.
- **Opened:** iteration 9

### The query limits are hard-coded, and the defaults are chosen rather than measured
- **Kind:** inspected-only
- **What is proven:** both bounds work and both refuse rather than truncate — the row cap is tested
  for solutions and for constructed triples, and the deadline is tested to cancel a runaway join.
  `QueryLimits` is a parameter type, so wiring it to configuration touches no caller.
- **What is not:** nothing reads configuration into it; every production call uses
  `QueryLimits::default()`. And the numbers themselves — 100 000 answers, 30 seconds — are
  reasoned, not measured. Neither has been checked against how long an unoptimised evaluator
  actually takes over a real vocabulary, so 30 s may refuse legitimate work or 100 000 rows may be
  far more memory than the §1.5 commitment tolerates.
- **What would close it:** the Phase 1 benchmark spike, then config keys with the provenance
  `adr/0005` requires.
- **Opened:** iteration 9

### The timeout answers 503, which is the least-wrong code rather than a right one
- **Kind:** inspected-only
- **What is proven:** the mapping is deliberate and argued in `adr/0011`. RFC 9110 has no code for
  "the server cancelled a valid request against its own resource policy"; 408, 504, and 500 each
  claim something untrue.
- **What is not:** the consequence. A load balancer or a service mesh reading 503 may take the
  instance out of rotation over one expensive query, which would turn a bounded refusal into an
  availability incident. No deployment has met this and nothing tests it.
- **What would close it:** running behind a real proxy with health checking and watching what one
  timed-out query does to rotation. Environment-limited in part — a realistic answer needs a
  deployment topology this machine does not have.
- **Opened:** iteration 9

### The query tests put statements in a vocabulary through the backend, not through an authoring path
- **Kind:** partial-coverage
- **What is proven:** the fixture is honest about itself and could not be otherwise today — no
  public API can put a statement into a vocabulary graph, because the store creates the container
  and Phase 2's candidate seam is what fills it.
- **What is not:** that a query sees what the *real* authoring path will actually write. The
  fixture inserts through `store.backend` directly, so it bypasses the write choke point, the
  transaction, and whatever shape the candidate seam settles on. If authoring later writes a
  different shape — reified statements, provenance quads alongside content — these tests keep
  passing against a shape production never produces.
- **What would close it:** rewriting the fixture onto the real authoring API the moment one exists.
  This is a deliberate debt with a named trigger, not an oversight.
- **Opened:** iteration 9

### Two query tests are timing-sensitive and could flake on a loaded machine
- **Kind:** partial-coverage
- **What is proven:** they pass repeatedly on this machine, and both are testing something real —
  that the deadline actually cancels, and that a watchdog never cancels a query that already
  finished (run 40 times over, because a watchdog cancelling a *later* query would otherwise show
  up as an intermittent failure and nothing else).
- **What is not:** their behaviour under contention. `a_quick_query_is_never_cancelled_by_its_own_watchdog`
  gives a one-statement query a 30 ms deadline; on a heavily loaded CI runner that query could
  genuinely exceed 30 ms and the test would fail for a reason that is not the bug it hunts. The
  tight deadline is *why* the test discriminates, so widening it to remove the flake would also
  remove most of its power — that trade is recorded rather than taken.
- **What would close it:** an injectable clock, or a deterministic cancellation hook, so the race
  is exercised without depending on wall-clock timing at all.
- **Opened:** iteration 9

---

### ~~The round trip is proven against our own reader~~ — HALF CLOSED, iteration 10
- **Kind:** partial-standard
- **What is proven:** every one of the six syntaxes survives serialise → parse → compare, over
  content chosen to be hostile: two language tags in non-Latin and accented scripts, an
  `xsd:integer`, a literal carrying a quote, a newline, a backslash and an emoji, an IRI with a
  percent-encoded space, and a blank node. Four mutants of the serialiser were confirmed to break
  it. Empty graphs are proven to produce *readable empty documents* in all six, which is the case
  that separates RDF/XML and JSON-LD from the line-based syntaxes.
- **What is not:** the reader in that round trip is the same library as the writer. Self-consistency
  is what is proven; **conformance is not**. If Oxigraph's Turtle writer and Turtle reader shared a
  misreading of the grammar, this test would pass and a third-party consumer would still choke.
  `CLAUDE.md` §4.5 requires a standards claim to be backed by the spec's own examples, so until
  that exists the claim this build makes is *round-trip fidelity*, not "we implement Turtle".
  Nothing yet reads an OpenBiz export with a tool we did not write.
- **What would close it:** run the W3C RDF test suites (rdf-tests) for each syntax against our
  export path, and — cheaper and worth doing first — assert a handful of exports byte-for-byte
  against fixtures produced by an independent tool (`rapper`, `riot`) so a divergence surfaces as a
  diff rather than as a customer's failed import.
- **Opened:** iteration 8
- **Half closed, iteration 10.** Two of the six are now checked against a reader **we wrote from
  the published EBNF**, sharing no code with the writer: `crates/openbiz-store/src/spec_conformance.rs`
  reads our N-Triples and N-Quads exports against [N-Triples §7] and [N-Quads §4], enforces the
  absolute-IRI requirement of [N-Triples §2.2], checks the five layout constraints of
  [Canonical N-Triples §4], and compares our bytes against [N-Triples Example 3] as published. The
  checker is itself proven to discriminate: twenty-one documents each violating exactly one named
  production or constraint are required to be rejected. See `adr/0012`.
  **It found two defects** — both now their own entries below, and both invisible to the round
  trip that preceded it. That is the answer to the question this entry was opened to ask.
  **Still open for the other four.** Turtle, TriG, RDF/XML, and JSON-LD remain proven only against
  our own reader, and the wording above applies to them unchanged. The W3C rdf-tests suites remain
  the thing that would close it properly for all six; that is now a `PROPOSED.md` item rather than
  a line in this entry, because folding it in here is what made it feel handled for two iterations.

  [N-Triples §7]: https://www.w3.org/TR/n-triples/#sec-grammar
  [N-Quads §4]: https://www.w3.org/TR/n-quads/#sec-grammar
  [N-Triples §2.2]: https://www.w3.org/TR/n-triples/#sec-iri
  [Canonical N-Triples §4]: https://www.w3.org/TR/n-triples/#canonical-ntriples
  [N-Triples Example 3]: https://www.w3.org/TR/n-triples/#sec-literals

### The store silently rewrites the lexical form of any literal it can interpret
- **Kind:** partial-standard
- **What is proven:** measured exactly, and pinned by
  `the_store_rewrites_the_lexical_form_of_the_datatypes_it_models_natively`. Written in, read back
  out: `"1.663E-4"^^xsd:double` → `"0.0001663"`; `"1.0E1"^^xsd:float` → `"10"`;
  `"007"^^xsd:integer` → `"7"`; `"+7"^^xsd:integer` → `"7"`;
  `"007"^^xsd:nonNegativeInteger` → `"7"`; `"4.00"^^xsd:decimal` → `"4"`;
  `"1"^^xsd:boolean` → `"true"`; `"2026-08-19T00:00:00+00:00"^^xsd:dateTime` →
  `"2026-08-19T00:00:00Z"`. Untouched: `xsd:string`, a datatype the engine does not know, an
  already-canonical lexical form, and — the perverse case — a value that is *invalid* for its
  datatype (`"abc"^^xsd:nonNegativeInteger` survives byte-for-byte). `RDF 1.1` defines a literal as
  the pair (lexical form, datatype IRI), so these are **different terms**, not different spellings.
  `two_triples_that_differ_only_in_lexical_form_collapse_into_one` proves the sharper harm: two
  distinct triples go in and one comes out.
- **What is not:** **nothing about this is disclosed to a user.** No API field, no export header, no
  interface warning. `RdfSyntax::records_graph_names` exists precisely so a different kind of
  silent loss is stated before a download, and this one is larger and unstated. **The loss is the
  store's, not the export's** — `the_rewrite_is_the_stores_and_not_the_exports` runs a `CONSTRUCT`
  that never touches `export_graph` and gets `"7"` back too, so every reader inherits it and a fix
  has to touch stored data rather than a serialiser. Still unmeasured: whether the rewrite happens
  at insert or at read (it is the term encoding either way, but which one decides whether existing
  stores can be repaired in place), and the full set of affected datatypes — the eight above are
  the ones tried, and the rule is the engine's, not ours.
- **What would close it:** the fix is a decision, not a patch, so it is in `PROPOSED.md` — upstream
  work, a term encoding of our own, or accepting the loss and *disclosing* it. Disclosure is the
  cheapest and is not a fix; a governance team cannot sign off a vocabulary whose notations
  changed. Until one is chosen, no OpenBiz surface may claim that an export round-trips.
- **Opened:** iteration 10

### Our N-Triples is one constraint short of Canonical N-Triples
- **Kind:** partial-standard
- **What is proven:** [Canonical N-Triples §4] requires that `ECHAR` be used only for U+0022,
  U+005C, U+000A and U+000D — "ECHAR MUST NOT be used for characters that are allowed directly in
  `STRING_LITERAL_QUOTE`". A tab is allowed directly and our writer emits `\t`. Pinned by
  `our_n_triples_export_is_canonical_n_triples_but_for_one_known_violation`, which requires exactly
  this one violation and no other, so a second one appearing is a failure. The other four
  constraints hold: single-space separators, no comments, no `UCHAR` (accented Latin, CJK, and an
  emoji are all written raw), and the carriage return and line feed correctly escaped.
- **What is not:** nothing is *lost* — the document is valid N-Triples and any conforming reader
  recovers the same term, which is why iteration 8's round trip could not see it. What is not true
  is that two tools serialising one graph produce the same bytes, which is what makes a vocabulary
  diffable in git. Whether the other five syntaxes have an equivalent layout-level divergence is
  unknown; only N-Triples has a canonical form defined, so for the others the question does not
  even have a spec-shaped answer.
- **What would close it:** either upstream, or an N-Triples writer of our own — it is the simplest
  of the six and the one where writing it is genuinely cheap. In `PROPOSED.md`; not taken here
  because replacing a serialiser is a decision about the engine boundary, not a blind-spot fix.
- **Opened:** iteration 10

  [Canonical N-Triples §4]: https://www.w3.org/TR/n-triples/#canonical-ntriples

### The HTTP export buffers the whole graph in memory
- **Kind:** partial-coverage
- **What is proven:** `Store::export_graph` genuinely streams — quads go to the writer as they are
  read, and the backend's iterator holds one snapshot for the whole scan, so peak memory in the
  *store* is one quad and a concurrent commit cannot tear the file. It takes no write lock, proven
  by exporting from inside an open write transaction.
- **What is not:** the HTTP layer collects that stream into a `Vec<u8>` to build the response body,
  so a single request is bounded by memory rather than by graph size, and N concurrent exports of a
  large vocabulary are bounded by N times that. Nothing has exported more than nine quads. The
  serialisation itself has never been timed. The work runs on `spawn_blocking`, so it does not stall
  the async runtime — that much is by construction, and also untested.
- **What would close it:** the Phase 1 benchmark spike, which now owes a **fifth** number: export
  wall-clock and peak RSS at 10k / 100k / 1M concepts, per syntax. If the number is bad the fix is a
  streaming body (`Body::from_stream` over a channel fed by the blocking task), which is a change to
  this handler and to nothing above it.
- **Opened:** iteration 8
- **Amended iteration 9:** the SPARQL endpoint has the same shape and a larger worst case, so the
  spike now owes a **sixth** number — query wall-clock and peak RSS at the 100 000-answer cap. The
  two handlers should be fixed together or not at all; a streaming export beside a buffering query
  endpoint is the inconsistency a reviewer would have to re-derive.

### The interface's download path has never run against a real store with content
- **Kind:** partial-coverage
- **What is proven:** under jsdom, the chooser renders exactly the formats the server advertises in
  the order it advertises them, every link's `href` is the escaped export URL for the chosen format,
  changing the format rewrites every link, and the lossy-syntax warning appears and disappears with
  the choice. Three mutations of the component were confirmed to break it. Against the real binary,
  `GET /api/export` was exercised by hand for TriG, N-Quads via `Accept`, and a 404.
- **What is not:** the two halves have never met. A vocabulary cannot be created over HTTP (§1.7
  holds that until `DiscoveryProvider` exists) and the store's public API creates only *empty*
  vocabularies, so **no download link has ever been clicked against a graph with statements in it**.
  The `Content-Disposition` filename, the browser's save behaviour, and what a 404 looks like to a
  user who clicked a link for a vocabulary deleted in another tab are all unobserved.
- **What would close it:** either Phase 2's authoring path, which will put content in a vocabulary,
  or the browser-driven test recorded in "Nothing renders the UI in a browser". Whichever lands
  first should exercise this.
- **Opened:** iteration 8

### `X-OpenBiz-Graph` is our own header, and nothing reads it
- **Kind:** no-production-caller
- **What is proven:** the header is on every export response, carries the graph's IRI, and is
  percent-escaped to ASCII so a non-ASCII IRI cannot make it an invalid header value. The escaping
  is unit-tested including the escaping of `%` itself.
- **What is not:** no client parses it — not our own interface, which already knows which graph it
  asked for. It exists so that an export in a triple syntax still *states* which graph it is, which
  is a real gap, but the value of an answer nobody reads is an assumption. Nor is it a registered
  header name or a standard: a consumer would have to be told about it. `Link: <iri>; rel="canonical"`
  or an RDF-level provenance statement in the payload may be better answers.
- **What would close it:** the import path (the next plan item) using it to default the target graph
  when a Turtle file is re-uploaded — which is the use case that would prove the header earns its
  place, or show that it does not.
- **Opened:** iteration 8

### An export's registry check and its scan are two snapshots
- **Kind:** inspected-only
- **What is proven:** by reading Oxigraph's `Store::quads_for_pattern`, the *scan* takes one
  snapshot and holds it for the whole iteration, so an export is internally consistent.
- **What is not:** `contains_graph` runs first, on its own earlier snapshot. A graph deregistered in
  the gap would be exported anyway; a graph registered in the gap would 404 despite existing. Both
  are unreachable today because **nothing in this build deregisters a graph**, so the window is
  argued from the code rather than closed by it — the same shape as the kill-window argument in
  iteration 7 and equally untested.
- **What would close it:** a read transaction spanning the check and the scan, added when the first
  deletion path arrives. Doing it now would put a lock-shaped API in front of a race that cannot
  happen, and the test for it could not be written.
- **Opened:** iteration 8

### ~~The UI is built but nothing serves it~~ — CLOSED, iteration 1
- **Closed by:** `rust-embed` embedding `ui/dist`, a router fallback serving it, 13 tests in
  `crates/openbiz-server/src/ui.rs`, an end-to-end test over a real socket in
  `tests/serves_embedded_ui.rs`, and a `Single binary` CI job that deletes `ui/dist` before asking
  the release binary to serve the interface. Verified by hand: the release binary served the real
  318-byte Vite `index.html` and the 194 087-byte bundle with `ui/dist` moved off disk. See
  `adr/0004`.
- **Kind:** no-production-caller
- **What was proven:** `ui/` typechecks under TS strict and builds to `ui/dist` (28 modules, ~194 kB
  raw / ~61 kB gzipped). The Rust server compiles, serves `/healthz`, and returns 404 elsewhere.
- **What was not:** the two halves were not connected. No `rust-embed`, no static-file route, no
  test that a built asset was reachable from the binary.
- **Opened:** Phase 0 hand-build (pre-iteration-1) · **Closed:** iteration 1

### ~~The UI has no test suite at all~~ — CLOSED, iteration 4
- **What is now proven:** Vitest + Testing Library run 10 assertions over `App` in CI (`npm test` in
  the `UI` job). All three `Probe` states are exercised: the loading text and the single `/healthz`
  call on mount; the success line naming status and version; the `role="alert"` branch for an HTTP
  refusal, a transport rejection, and a non-`Error` rejection; the unmount abort; the `AbortError`
  swallow; and a StrictMode double-mount that must not paint a spurious alert. Each was shown to
  fail against a deliberately broken `App.tsx` before being trusted — see the plan item for the
  seven mutants.
- **Correction to the original entry:** it said the driver's `npm test` "silently passes". It did
  not — there was no `test` script, so it exited 1 with `Missing script: "test"`, and the `UI` CI
  job never called it. Zero UI assertions had ever run either way.
- **Opened:** iteration 1 · **Closed:** iteration 4

### Write throughput under a serialised writer is unmeasured
- **Kind:** partial-coverage
- **What is proven:** writes are correct under concurrency — eight threads racing on one IRI leave
  exactly one registration, eight threads on distinct IRIs all land, and readers are never blocked
  by an open transaction. Correctness under contention is tested; **cost** under contention is not.
- **What is not:** `adr/0009` trades write parallelism for serialisability by taking a lock we own,
  and no number anywhere says what that costs. Nothing measures how long a transaction holds the
  lock, how deep the queue gets, or where the knee is. Upstream also states a transaction holds its
  entire change set in memory, so a naive "one transaction per import file" in Phase 11 is both a
  memory risk and a long lock hold, and nothing currently stops someone writing it.
- **What would close it:** the Phase 1 Oxigraph benchmark spike, which now owes a fourth number —
  concurrent write throughput and lock wait time at the 10k/100k/1M sizes it already covers. Phase
  13 then addresses whatever it finds.
- **Opened:** iteration 7

### Rollback is proven against errors and panics, but not against process death
- **Kind:** partial-coverage
- **What is proven:** a transaction that returns `Err` writes nothing, a transaction that panics
  writes nothing and does not leave the store read-only, and a rolled-back transaction leaves the
  store byte-identical (asserted on quad count, not just on absence). Six mutants confirmed each
  assertion is load-bearing.
- **What is not:** nothing kills a process mid-transaction and reopens the store. That is the
  case the backend's crash recovery handles rather than our code, so it is testing Oxigraph rather
  than testing us — but it is also the case an operator actually hits, and "the backend handles it"
  is an assumption we have not verified. Relatedly, `Store::open` now commits the format stamp and
  the system-graph registration in one transaction *specifically* to close a kill-in-the-gap
  window, and the proof that the window is closed is the code's shape, not a test.
- **What would close it:** a harness that spawns the real binary, `SIGKILL`s it during a write, and
  reopens the store asserting it is either fully changed or fully unchanged. `tests/graceful_shutdown.rs`
  already spawns real binaries, so the machinery exists; what is missing is a way to make the
  binary write on demand, which needs an authoring endpoint that §1.7 says cannot exist until
  `DiscoveryProvider` does.
- **Opened:** iteration 7

### The nested-transaction guard fails by hanging if it regresses
- **Kind:** inspected-only
- **What is proven:** `a_nested_transaction_is_refused_rather_than_deadlocking` passes, and a
  mutant that stops keying the reentrancy mark by store address kills it.
- **What is not:** if the guard were removed entirely, that test would **hang** rather than fail,
  because the bug it guards against is itself a deadlock. A hanging test in CI reads as a flaky
  runner or a timeout, not as a regression, so the signal is real but badly shaped.
- **What would close it:** run the nested call on a spawned thread and join it with a timeout, so
  the absence of the guard reports as a failed assertion rather than as a stuck job. Not done here
  because `std` has no `join_timeout`; it needs a channel-with-timeout dance that is more test
  machinery than the one assertion justifies today.
- **Amended, iteration 8:** a **second** test now has the same shape.
  `exporting_does_not_block_on_the_write_lock` exports from inside an open write transaction; if
  `export_graph` ever took the write lock, that test would hang rather than fail. The property is
  worth asserting — an export must never be able to block an author — but two hanging tests is now
  a pattern rather than a one-off, and the timeout helper this entry describes would pay for itself
  across both.
- **Opened:** iteration 7 · **Amended:** iteration 8

### The UI suite asserts on jsdom, and covers one component
- **Kind:** narrowly-proven
- **What is proven:** `App`'s render output and probe lifecycle, under jsdom, via accessible
  queries (`getByRole`) rather than test IDs — so the assertions are about what a user or a screen
  reader perceives, not about markup shape.
- **What is not:** three things. (1) **jsdom is not a browser** — no layout, no real paint, no CSS,
  no focus semantics beyond jsdom's approximation. The Phase 3 Playwright item is what closes that;
  see "Nothing renders the UI in a browser". (2) **`main.tsx` is untested** — the "root element
  missing" throw and the `createRoot` call have no assertion; only `App` is mounted, by the test's
  own `render`. (3) **`CLAUDE.md` §4.4's keyboard-navigability clause is still unenforced.** Nothing
  in `App` is interactive yet, so there is nothing to tab to and no test would be meaningful — but
  the moment Phase 3 adds a control, a keyboard test has to arrive with it, and no mechanism
  currently forces that.
- **Amended, iteration 6:** it now covers *two* components — `App` and `Vocabularies` — plus the
  `useProbe` hook they share, at 22 assertions. Every point above still stands unchanged, and (3)
  has now survived a second component without being tested: `Vocabularies` renders a heading, a
  list, and paragraphs, so there is still nothing to tab to and `CLAUDE.md` §4.4 is still satisfied
  **vacuously**. Two iterations of UI have now been added without the clause ever being exercised.
- **Amended, iteration 8:** the interface has its **first interactive control** — the export format
  chooser — so (3) is no longer vacuous. It is now *narrowly* satisfied: a test asserts the control
  is a native `<select>` with an associated `<label>` rather than a `div` with a click handler, that
  the download is a real `<a href>`, and that both accept focus. That is the thing which *makes* a
  tab order, but it is not the tab order: jsdom has no real focus semantics, so nothing proves the
  sequence a keyboard user actually walks, that the chooser is reachable before the links it
  governs, or that the `role="alert"` states are announced. Point (3) has therefore changed from
  "untested because untestable" to "tested at the only level jsdom permits", which is progress and
  is not closure. Coverage is now 29 assertions across the same two components.
- **What would close it:** for (1) the Playwright item; for (2) a test that mounts `main.tsx`
  against a document with and without `#root`; for (3) the same Playwright item, which is the only
  place a real tab order can be walked — plus the Phase 3 convention, ideally a lint or a shared
  test helper, that makes an interactive component without a keyboard test fail.
- **Opened:** iteration 4 · **Amended:** iterations 6, 8

### Nothing renders the UI in a browser
- **Kind:** environment-limited
- **What is proven:** the served bytes are correct — status, content type, `ETag`/304, cache
  headers, and the exact Vite `index.html` and bundle, asserted over a real socket.
- **What is not:** no browser has ever executed the bundle. React could throw on mount and every
  test here would still pass, because they assert on transport, not on rendering. Content-Security-
  Policy, `crossorigin` on the module script, and MIME strictness are unexercised for the same
  reason.
- **What would close it:** a headless-browser smoke test (Playwright) that loads `/` from the real
  binary and asserts the health line appears. This machine can run one; it is a deliberate scope
  call not to add a browser toolchain in a Phase 0 iteration.
- **Opened:** iteration 1

### Asset serving is unproven above trivial size and count
- **Kind:** partial-coverage
- **What is proven:** one 194 kB bundle and one 318-byte shell, served whole, in a two-file
  `ui/dist`.
- **What is not:** no compression (`Content-Encoding`) is negotiated or emitted, so the 194 kB
  bundle goes over the wire uncompressed — an incumbent-grade UI will be several times that.
  Range requests are not handled. `Assets::get` is a lookup over a generated match arm per file; its
  behaviour at the hundreds-of-files scale a real design system produces is unmeasured, as is the
  effect on binary size and compile time.
- **What would close it:** revisit when Phase 3's design system makes `ui/dist` realistic —
  measure binary size, cold start, and first-paint transfer size, and decide on compression then.
- **Opened:** iteration 1

### ~~Config is env-only; the file path is unimplemented~~ — HALF CLOSED, iteration 2
- **Closed by:** `crates/openbiz-server/src/config.rs` — defaults → TOML file → environment, with
  documented precedence, `deny_unknown_fields`, blank-value rejection, and a `Source` on every
  setting. 16 tests; `Config::load` is called by `main.rs`, which logs each setting's provenance
  and names it in the bind-failure message. Four failure paths verified by hand against the real
  binary. See `adr/0005` and `docs/CONFIGURATION.md`.
- **Still open — see the entry below:** the `data_dir` half. Nothing creates or opens that
  directory yet.
- **Kind:** partial-coverage
- **Opened:** Phase 0 hand-build (pre-iteration-1) · **Config-file half closed:** iteration 2

### ~~`data_dir` is configured, logged, and used by nothing~~ — CLOSED, iteration 3
- **Closed by:** `Store::open(config.data_dir.value())` in `main.rs`, which runs *before* the
  listener binds. A `data_dir` that is a file, is unwritable, or is already locked by another
  OpenBiz now fails startup with a message naming both the path and the configuration layer that
  chose it. Covered by `a_data_directory_that_is_a_file_is_reported_as_such`,
  `an_unwritable_data_directory_is_reported_before_the_backend_sees_it`, and — across real
  processes — `a_second_instance_refuses_to_share_the_data_directory`. See `adr/0006`.
- **Kind:** no-production-caller
- **What is proven:** `data_dir` is read from the defaults, a file, or `OPENBIZ_DATA_DIR`, is
  rejected when blank, carries its provenance, and is logged at startup.
- **What is not:** **no code creates, opens, or writes that directory.** A user who sets it to an
  unwritable path, a path that does not exist, or a file rather than a directory gets a clean
  startup and no warning — the setting is inert until Phase 1 wires the store. This is the honest
  reading of `CLAUDE.md` §4 clause 1: a value with no consumer is not a feature.
- **What would close it:** the first Phase 1 item (`openbiz-store` Oxigraph lifecycle — open, close,
  durable path, graceful shutdown), which is the next item in the plan.
- **Opened:** iteration 2 (carved out of the entry above)

### Configuration is validated for shape, not for meaning
- **Kind:** partial-coverage
- **What is proven:** unknown keys, malformed TOML, wrongly-typed values, blank values, and a
  missing explicitly-named file all fail with a message naming the source. Precedence across all
  three layers is tested per setting, including that one key in a file does not change the
  provenance of another.
- **What is not:** `bind` is only checked for being non-blank. `OPENBIZ_BIND=not-an-address` is
  accepted by the loader and fails later at `TcpListener::bind` — a clear error, but later than it
  needs to be, and it is the only setting where the bind attempt happens to be a validator. No
  setting has a range, enum, or path check, and there is no mechanism for one. A non-UTF-8
  `OPENBIZ_CONFIG` path is treated as unset by `std::env::var().ok()` rather than reported, which is
  a silent ignore of exactly the kind the rest of this module refuses.
- **What would close it:** a per-setting validation hook run during `Config::resolve`, so the error
  carries the `Source` like the blank-value one does. Worth doing when the settings count justifies
  it — with two settings a hook is more machinery than the problem.
- **Opened:** iteration 2

### ~~Configuration precedence is untested against the real process environment~~ — CLOSED, iteration 3
- **Closed by:** `crates/openbiz-server/tests/graceful_shutdown.rs`'s
  `the_process_environment_reaches_the_configuration_with_its_provenance`, which spawns the real
  binary with a controlled environment and asserts on its startup log — that `data_dir` and `bind`
  both report `$OPENBIZ_*` as their source, and that requesting port 0 logs the port actually
  allocated rather than the request. `Config::load` now has a regression test; a typo in a variable
  name inside it fails CI. This was also the promoted plan item of the same name.
- **Kind:** partial-coverage
- **What is proven:** `Config::resolve` is tested exhaustively with an injected environment lookup,
  and the four headline failure paths were run by hand against the real binary with real env vars
  and real files.
- **What is not:** `Config::load` — the ten-line function that supplies `std::env::var` and the
  default path — has no automated test. It cannot easily have one: `std::env::set_var` mutates
  state shared by every thread in the test binary, and Rust 2024 marks it `unsafe` for that reason.
  So the wiring between the tested core and the real environment is inspected-only, and a typo in a
  variable name inside `load` would pass CI.
- **What would close it:** an integration test that spawns the binary as a subprocess with a
  controlled environment and asserts on its startup log — the same shape as
  `tests/serves_embedded_ui.rs`. Cheap; deferred only to keep this iteration to one item.
- **Opened:** iteration 2

### ~~The named-graph model has no production caller~~ — CLOSED, iteration 5
- **Closed by:** `insert_into` in `crates/openbiz-store/src/lib.rs` — the single function through
  which every write to the store passes, including the format stamp. It requires a `&GraphId` and
  refuses a target that is not directly writable, so `is_directly_writable()` is a rule the store
  enforces rather than a comment a caller may forget, and `StoreError::NotWritable` is now returned
  by a public method (`create_vocabulary_graph`). `Store::open` registers the system graph in the
  graph registry on every open, and `main.rs` reads that registry **before it binds**, failing
  startup if it cannot be described. `GraphId`'s fields are private, so the pairing of IRI and kind
  is an invariant the type enforces: the mutation that assembled one by struct literal from a
  registry row would not compile. 36 store tests (was 12); twelve mutants killed. See `adr/0007`.
- **Kind:** no-production-caller
- **Opened:** iteration 3 · **Closed:** iteration 5

### `create_vocabulary_graph` has no production caller
- **Kind:** no-production-caller
- **What is proven:** it registers a graph, refuses an IRI that is already registered with a message
  pointing at the reuse ladder, refuses a graph that is not directly writable, leaves the registry
  unchanged when it refuses, and survives close and reopen. Nine tests, and the mutants that drop
  either guard are killed.
- **What is not:** **nothing calls it outside tests.** No HTTP route, no UI, no import path creates a
  vocabulary graph. This is deliberate rather than an oversight, and the distinction matters:
  `CLAUDE.md` §1.7 requires discovery to run *before* creation and requires a recorded justification
  when something new is created anyway, and `DiscoveryProvider` does not exist until Phase 2. Adding
  a create endpoint now would be a charter violation dressed up as progress, so the honest position
  is a recorded gap.
- **What would close it:** the Phase 2 authoring path with its local discovery hook. The read half —
  `GET /api/graphs` and the registry in the UI — is the next Phase 1 item and does not depend on it.
- **Amended, iteration 7:** still open, and the entry understated the cost of leaving it open.
  Having no production caller is *why* nobody had hit the race in it: the check and the write were
  two separate operations, and eight threads creating one IRI all succeeded, leaving a registry
  that `Store::graphs` then refuses wholesale as `Corrupt`. That is now fixed — the check and the
  write are one transaction (`adr/0009`) — but the lesson is about this ledger rather than about
  the bug. **A no-production-caller entry is not a dormant risk; it is an untested one**, and the
  concurrency defect sat in a method with nine passing tests. The `transaction` API this method now
  delegates to *does* have a production caller (store startup), so the seam is exercised on every
  start even though this method is not.
- **Opened:** iteration 5 · **Amended:** iteration 7

### A corrupt registry is proven to stop the store, not proven to stop the server
- **Kind:** partial-coverage
- **What is proven:** `Store::graphs()` returns `StoreError::Corrupt` for an unrecognised kind
  token, for a registry entry that breaks the namespace rule, and for a graph registered twice with
  different kinds — each asserted against a store doctored through the backend, and each mutant
  killed. `main.rs` propagates that error with `?` before the listener binds.
- **What is not:** the *propagation* is inspected-only. `tests/graceful_shutdown.rs` proves the
  registry is read at startup (the mutants that stop reading it and that read-but-do-not-report it
  both turn it red), but no test starts the binary against a **doctored** store and asserts it
  refuses to serve. Building that fixture needs raw quad access from outside the crate, which today
  means either making the backend reachable or adding `oxigraph` as a dev-dependency of
  `openbiz-server` — and a test-only direct dependency on the engine cuts against `CLAUDE.md` §3,
  so it is not a cost worth paying for one assertion.
- **Also, since iteration 6:** `GET /api/graphs` has the same shape of gap. `From<StoreError> for
  Failure` is asserted directly — a constructed `StoreError::Corrupt` becomes a 500 whose body
  contains neither the customer's IRIs nor their store path, and both mutants (200 instead of 500,
  and echoing the store's own words) are killed. What is *not* proven is that a real store can
  actually drive the handler down that branch, for the same fixture reason. In practice it is close
  to unreachable: `main` refuses to start against a registry it cannot read, so the endpoint only
  sees one that went bad while the process was running.
- **What would close it:** a store-layer test helper that writes an arbitrary quad, gated behind a
  cargo feature the server's tests can enable. Now wanted by two callers rather than one, which
  changes the cost/benefit recorded above — it is worth doing the next time either gap is touched.
- **Opened:** iteration 5 · **Amended:** iteration 6

### Registry reads are unmeasured above a handful of graphs
- **Kind:** partial-coverage
- **What is proven:** `Store::graphs()` returns the right graphs in a stable order with up to four
  registered, and `contains_graph` answers without scanning the store's contents. Listing asks the
  registry rather than the backend precisely so it does not become a whole-store scan.
- **What is not:** the registry read is a pattern scan bounded by the number of *graphs*, which is
  fine at four and unmeasured at four thousand — and an enterprise with a vocabulary per business
  domain per jurisdiction gets there. It also runs on the startup path, so if it is slow it is slow
  in the cold-start budget `CLAUDE.md` §1.5 sets. Nothing has measured it, and `graphs.sort()` is an
  additional O(n log n) on top.
- **Widened, iteration 6:** `GET /api/graphs` reads the registry **on every request**, deliberately
  and with the reasoning recorded in `adr/0008` §6 — a cache would need invalidating by every future
  creation, import, and restore path, and a stale "your vocabulary does not exist" is worse than an
  unmeasured scan. But the consequence is that an unmeasured scan is now on a *hot* path rather than
  a once-per-process one, and it also serialises the whole registry into a JSON body with no paging.
  The 4 000-graph deployment gets a 4 000-element response on every page load.
- **Widened again, iteration 8:** the interface now reads the registry *and* `/api/export/formats`
  on mount, so a page load is two requests rather than one — the second is a constant-size response
  built from a six-element array and is not a scan, so it does not widen the measurement problem,
  but it does mean the vocabulary list's time-to-first-paint now waits on two round trips.
- **What would close it:** the Phase 1 benchmark spike should register 10k graphs and time
  `graphs()`, startup, **and the endpoint**, alongside the query evaluation and `close()` numbers it
  already owes. If the number is bad, the answer is paging or a `?kind=` filter — both API changes,
  which is why the spike should land before Phase 3 builds an interface on top of this shape.
- **Opened:** iteration 5 · **Widened:** iterations 6, 8

### Durability is proven for one quad, not for a vocabulary
- **Kind:** partial-coverage
- **What is proven:** what one open commits, the next open reads back — `the_format_stamp_survives_
  close_and_reopen` closes the store and reopens it, and the cross-process test restarts a real
  binary against the same directory. The stamp is flushed immediately on first open.
- **What is not:** the store has never held more than a single quad. Nothing has written a
  vocabulary, so nothing has measured write throughput, flush cost at size, or how long `close()`
  takes when there is real data to flush — which is the number that decides whether a `docker stop`
  grace period of 10 s is enough or whether we are hard-killed anyway. The two upstream Oxigraph
  risks (unoptimised query evaluation, literal precision) are **untouched** by this iteration and
  keep their Phase 1 spike items; `adr/0006` must not be read as clearing them.
- **What would close it:** the Phase 1 benchmark spike, which should measure `close()` at 10k /
  100k / 1M concepts and not only query evaluation.
- **Amended, iteration 7:** the store has now held up to nine quads rather than three, which does
  not move this entry at all — the point stands unchanged. The spike now owes a *fourth* number as
  well; see "Write throughput under a serialised writer is unmeasured".
- **Opened:** iteration 3 · **Amended:** iteration 7

### Graceful shutdown is proven to exit cleanly, not to drain
- **Kind:** partial-coverage
- **What is proven:** `SIGTERM` to a real process exits zero, logs which signal stopped it, logs
  `store closed cleanly`, and releases the lock so the next process can open the same directory.
  `shutdown_signal()` is proven not to resolve on its own — which matters, because `axum::serve`
  returns `Ok(())` on graceful shutdown, so a signal future that resolved immediately would produce
  a binary that exits zero while serving nothing.
- **What is not:** **no test has an in-flight request when the signal arrives.** The ordering the
  module documents — stop accepting, drain, *then* flush — is asserted only in its doc comment. A
  regression that closed the store before the last response was written would pass every test here.
  `SIGKILL` recovery was checked **by hand only**: a hard kill exits 137 and writes no
  `store closed cleanly` line (so the assertion discriminates rather than always passing), and the
  next start reopened the store and read its stamp back. That was a near-empty store with no write
  in flight — the case that actually matters, a hard kill *during* a write, is unmeasured, and none
  of it is a regression test.
- **What would close it:** a test that opens a slow request, sends `SIGTERM` mid-flight, and asserts
  the response completes before the process exits; and a `SIGKILL` test asserting the store reopens.
- **Opened:** iteration 3

### The lock classification depends on a RocksDB message string
- **Kind:** partial-coverage
- **What is proven:** both wordings RocksDB actually emits are pinned by tests — the same-process
  one in `openbiz-store`, the cross-process one in `tests/graceful_shutdown.rs`. Discovering they
  differ cost this iteration a red test, and a unit test alone would have shipped a classifier that
  never fired in production.
- **What is not:** `classify_open` matches on the substring `LOCK:`, and RocksDB's wording is not an
  API. A version bump could change it. The mitigation is that the tests go red rather than the
  classification degrading silently — but that is a promise about *noticing*, not about working, and
  a user hitting the fallback branch gets a true but much less useful error.
- **What would close it:** nothing available today; the backend exposes no typed lock error. Revisit
  if Oxigraph gains one. Recorded so a future upgrade knows to look here first.
- **Opened:** iteration 3

### Shutdown is Unix-only in test, and by implication in practice
- **Kind:** environment-limited
- **What is proven:** on Unix, `SIGINT` and `SIGTERM` both stop the server gracefully.
- **What is not:** `tests/graceful_shutdown.rs` is `#![cfg(unix)]` — `SIGTERM` does not exist
  elsewhere, and on Windows the `Ctrl-C` branch is the entire contract, unexercised by any test.
  Nothing has ever run this binary on Windows. We do not currently claim Windows support; this entry
  exists so that claim is not made accidentally.
- **What would close it:** a Windows CI runner exercising `Ctrl-C` shutdown, if and when Windows
  becomes a supported target.
- **Opened:** iteration 3

### The store's build toolchain is unavailable on the loop machine without a workaround
- **Kind:** environment-limited
- **What is proven:** the workspace builds and its tests pass once `libclang` is present. CI
  installs `clang` and `libclang-dev` explicitly rather than relying on the runner image.
- **What is not:** this machine has no `clang`, no `libclang`, and no passwordless `sudo`, so
  `cargo test --workspace` fails at `bindgen` on a clean checkout. The loop works around it by
  extracting `libclang1-20` and `libclang-common-20-dev` from downloaded `.deb`s into
  `~/.local/libclang`. **That workaround is not in the repo and does not survive a machine reset** —
  a future iteration that starts with an unexplained `bindgen` panic should read this entry rather
  than conclude the store is broken.
- **The full incantation, because `LIBCLANG_PATH` alone is not enough** (iteration 4 lost time to
  this): with only `LIBCLANG_PATH` set, libclang loads but fails to locate its own builtin headers
  and `bindgen` dies on `rocksdb/c.h:65:10: fatal error: 'stdbool.h' file not found` — a *different*
  message from the `Unable to find libclang` this entry originally described, and easy to misread as
  a real build break. Both variables are needed:
  ```
  export LIBCLANG_PATH="$HOME/.local/libclang/usr/lib/llvm-20/lib"
  export BINDGEN_EXTRA_CLANG_ARGS="-resource-dir=$HOME/.local/libclang/usr/lib/llvm-20/lib/clang/20"
  ```
- **What would close it:** a human running `sudo apt install clang libclang-dev` on the loop
  machine. Out of loop scope (`CLAUDE.md` §8 — needs root).
- **Opened:** iteration 3 · **Amended:** iteration 4

### The UI dependency tree is outside the §5 licence gate
- **Kind:** unenforced-policy
- **What is proven:** `cargo deny check licenses` enforces `CLAUDE.md` §5 over the whole Rust tree
  in CI, and it is green. The four packages added this iteration were checked by hand before being
  committed: `vitest` (MIT), `jsdom` (MIT), `@testing-library/react` (MIT), `@testing-library/dom`
  (MIT). A sweep of all 153 installed npm packages found 134 MIT, 8 ISC, 5 Apache-2.0, 2 BSD-2,
  2 BSD-3, 1 MIT-0 — all permitted — and exactly one unlisted licence.
- **What is not:** **nothing enforces this.** §5 says "every new dependency gets a licence check",
  and for `ui/` that check is a human remembering to run one. The sweep above was ad hoc and is not
  in the repo, so the next npm install is unchecked by default. The one unlisted licence is
  `caniuse-lite` under **CC-BY-4.0** — pre-existing, pulled in transitively by Vite via
  `browserslist`, build-time only, and a *data* package rather than code, so none of it reaches the
  shipped binary. CC-BY-4.0 is attribution-only and not copyleft, so this is the "unlisted but
  permissive in substance" case of §5 rather than the forbidden one — but §5 requires an ADR for
  that judgement and there is no ADR, because there is no allow list for it to widen.
- **What would close it:** the `Audit the UI dependency tree for licences` proposal in
  `PROPOSED.md`. Deliberately not done here: adding an unrequested CI gate is scope creep, and the
  CC-BY-4.0 call deserves its own ADR rather than a footnote in a test-runner commit.
- **Opened:** iteration 4

### The JSON API has no authentication, and the error path is shaped around that
- **Kind:** partial-coverage
- **What is proven:** `GET /api/graphs` returns 200 with the registry, `POST` returns 405, an
  unmatched `/api/…` returns 404, and a store failure returns a 500 whose body carries neither the
  customer's graph IRIs nor their store path — asserted directly, and both mutants killed.
- **What is not:** **nothing authenticates or authorises this endpoint.** Anyone who can reach the
  port can enumerate every vocabulary IRI in the deployment. That is the expected state for Phase 1
  (auth is Phase 7) and it is not a bug, but it is a fact the loop should not lose track of, because
  a second decision has already been taken *because* of it: `adr/0008` §3 deliberately withholds the
  store's own error text from the response, which costs real diagnostic value and cuts against
  `CLAUDE.md` §3's explainability commitment. When Phase 7 lands, that trade should be revisited
  rather than inherited — an authenticated administrator should get the detail.
- **What would close it:** Phase 7's authorisation, plus a test that an authenticated administrative
  caller receives the full store error and an unauthenticated one does not.
- **Opened:** iteration 6

### `main`'s refusal to close a still-shared store is inspected-only
- **Kind:** inspected-only
- **What is proven:** the happy path, and it is proven where it can actually fail.
  `the_graph_registry_is_served_over_http_and_the_store_still_closes_cleanly` serves a real HTTP
  request from the real binary — which is what makes a connection clone the shared state — and then
  signals it, asserting exit zero and `store closed cleanly`. Before this iteration every shutdown
  test signalled a process that had never served a request, so the reclaim was trivially safe.
- **What is not:** the *failure* branch. If `Arc::into_inner` returns `None`, `main` bails with an
  error saying the store could not be closed cleanly, and nothing exercises that: producing it needs
  a deliberately leaked handle, which we have no way to inject without adding a seam whose only user
  is the test. So the message an operator would see in the one situation where their writes might
  not have reached disk has never been printed by a running process.
- **What would close it:** honestly, little worth its cost today. Revisit if a future feature holds
  a store handle outside the router — a background materialisation pass, a scheduled backup — since
  that is when the branch stops being theoretical.
- **Opened:** iteration 6

### `useProbe` re-fetches on a URL change that no caller makes
- **Kind:** no-production-caller
- **What is proven:** the hook fetches once on mount, checks the status before parsing the body,
  aborts on unmount, and stays silent on an abort — 22 assertions across two components, and the
  four mutants against those behaviours are killed.
- **What is not:** its effect depends on `[url]`, so changing the `url` prop would cancel the first
  request and issue a second. **Nothing changes it** — both call sites pass a string literal — so
  that branch has no production caller and no test. It is one line and removing it would be worse
  (a stale response for the old URL is a nastier bug than an unused code path), but it should not be
  counted as proven behaviour.
- **What would close it:** the first component that reads a parameterised URL — Phase 2's
  per-vocabulary views are the likely first — should arrive with a test that changing the URL
  abandons the first response.
- **Opened:** iteration 6
