# ADR 0011 — The SPARQL 1.1 Query endpoint, its dataset, and its bounds

**Status:** accepted (2026-08-18) · **Phase:** 1

## Context

Phase 1's item reads "SPARQL 1.1 Query endpoint with all four result formats (JSON, XML, CSV,
TSV)". `CLAUDE.md` §2 commits us to SPARQL 1.1 Query as a conformance target, and every competitor
in this market has an endpoint, so the interesting decisions are not *whether* but *what it answers
with by default*, *what stops it*, and *what it refuses to guess*.

Three constraints shaped every decision below.

- **Nothing in an OpenBiz store is in the default graph.** `adr/0007` puts every quad in a named
  graph, so the specification's own default — "the store's default graph" — matches nothing.
- **Oxigraph's query evaluation is explicitly not yet optimised upstream** (`CLAUDE.md` §3), and
  §1.5 commits us to modest memory at rest. An unbounded endpoint in front of an unoptimised
  evaluator is a way for one caller to take the server down with one line of valid SPARQL.
- **The binary is compiled without an HTTP client** (`adr/0006`), so §1.1's air-gapped operation is
  a property of what is linked in rather than a promise.

## Decision 1 — the default dataset is the registered vocabulary graphs, and nothing else

A query that carries no `FROM`/`FROM NAMED` is evaluated over the union of the graphs the registry
records as `kind: "vocabulary"`. The system graph and any inferred graph are outside it.

Both alternatives are worse, and in opposite directions.

*The store's default graph* is what the specification says and it would return **zero rows for
every query against a populated store**. That is not a subtlety a user recovers from; it reads as a
broken product.

*The union of every graph* is what the incumbents do, and it is the trap this decision exists to
avoid. Point a taxonomist at PoolParty's or GraphDB's endpoint, run `SELECT * WHERE { ?s ?p ?o }`,
and the answer interleaves the tool's own bookkeeping with their vocabulary, unlabelled. That is
precisely the failure `adr/0007` was built to prevent and that `adr/0008` describes VocBench
committing: our metadata presented as the user's data.

The rule is **not hidden**. The graphs it covers are exactly what `GET /api/graphs` already reports
as vocabularies, which is already in the interface. And a query that names its own dataset with
`FROM` or `FROM NAMED` is honoured **verbatim**, including when it names one of ours — that is
SPARQL 1.1's own rule about dataset specification, and it is the escape hatch an operator needs to
ask "what is actually in my store?", the question §1 exists to keep answerable. Nothing is hidden;
the default is chosen.

The implementation rests on `prepared.dataset().is_default_dataset()` being true exactly when the
query carried no dataset clause. A dataset clause always names at least one IRI, so it can never
produce the store's own default graph — `the_system_graph_is_reachable_when_a_query_asks_for_it_by_name`
pins that down, because the whole escape hatch rests on it.

## Decision 2 — both bounds refuse; neither truncates

`QueryLimits` carries a cap on answers (100 000) and a wall-clock deadline (30 s). Exceeding either
is an error with a status code and a message. **Neither truncates.**

This is the decision with the most product content in it. The common alternative — return the first
N rows and stop — produces a document that is well-formed, complete-looking, and wrong, and in a
governance tool it is wrong in the specific direction of "the row you were looking for is not
here". A governance team cannot sign off rows they were never told were missing. A refusal is
recoverable; a silent truncation is discovered downstream or never.

The deadline is implemented with Oxigraph's `CancellationToken` and a watchdog thread, because
evaluation is synchronous and holds the calling thread: the only way to interrupt it is from
somewhere else. The watchdog exits the moment the query finishes and drops its sender, so a server
answering many queries does not accumulate one sleeping thread per query for the length of the
timeout. `Disconnected` is deliberately **not** treated as a timeout — that is the query having
already produced its answer.

The limits are a parameter type rather than constants so they can come from configuration without
touching a caller. They are **not configurable in this build**, which is recorded in
`docs/UNTESTED.md` rather than implied by the type.

## Decision 3 — `ResultsSyntax` is ours, for a sharper reason than `RdfSyntax` was

Same §3 rule as `adr/0010`: no third-party type in our API. Here the membership matches the
engine's, so the reason is a different one — **shape**. `sparesults` reports CSV's media type as
`text/csv; charset=utf-8`, a media type *with a parameter*, which is not something you can compare
against an entry in a caller's `Accept` header without stripping it first. Ours are bare, and the
charset is added once by whoever writes the response header. A test asserts the engine still maps
each of our bare media types and extensions back to the right format, so a backend swap that
disagreed with our published contract is a failing test rather than a client that can no longer
read what we wrote — and a second test asserts the engine's strings *still* carry parameters, so
the duplication can be deleted the day that stops being true.

`parse` is deliberately **narrower** than the engine's table, which also answers to
`application/xml`, `application/json`, and `text/plain`. `application/xml` in particular arrives in
every browser's `Accept` header and is historically how RDF/XML was served, so reading it as a
request for SPARQL Results XML would hand a person who typed the endpoint into an address bar a
results document they never asked for.

**`preserves_term_detail` is the field that earns the type.** CSV writes every value as bare text:
`"1"` the string and `1` the integer come out identical, an IRI is indistinguishable from a literal
that looks like one, and a **language tag is simply gone**. For a multilingual thesaurus that is
not a technicality — it is the difference between a label and which language the label is in. The
shape of the mistake is a governance team exporting a review spreadsheet as CSV, editing it, and
re-importing a vocabulary whose language tags have all quietly become the default. The
specification says so; no tool in this market says so at the point of choosing. The claim is
asserted against what the serialiser actually writes, probed with a language tag rather than a
datatype — TSV writes `"1"^^xsd:integer` as `1`, which is abbreviation rather than loss, and would
have made the test claim a loss that had not happened.

`GET /api/sparql/formats` serves this list, mirroring `/api/export/formats` from `adr/0010`. Two
lists rather than one merged list, because which family applies is decided by the query, and a
caller who cannot tell them apart is exactly the caller who sends `?format=csv` with a `CONSTRUCT`.
The endpoint also exists so that `preserves_term_detail` has a **production reader**: without it the
constant is a well-argued fact that nothing in the product ever tells a user, which is the "built
but no production caller" failure `CLAUDE.md` §4.1 names. A test asserts every token the list
advertises is one the query endpoint actually accepts, which is the drift the list exists to
prevent.

## Decision 4 — one `Accept` header, two families, and a 406 rather than a substitution

A SPARQL query answers in one of two families and **which one is a property of the query, not of
the request**: `SELECT` and `ASK` produce results documents, `CONSTRUCT` and `DESCRIBE` produce
RDF. A caller negotiating content has to state its preferences before anything has been parsed.

So `Acceptable` holds an `Option` per family, both read from the one header, and the check that the
answer's shape is one the caller can read happens **after** evaluation — the shape is not known
until the query has run. The cost is doing work for a response that is then refused, which happens
only when a caller's `Accept` and their own query disagree.

The alternative is what the market does: ask a typical endpoint for `text/turtle`, send it a
`SELECT`, and get JSON with a `Content-Type` that says JSON and no acknowledgement that the
negotiation failed. Here that is a **406 naming what the query actually produced** and the formats
that shape can be written in.

`Accept` parsing was factored into `crate::accept` and the export endpoint now shares it. Two
endpoints negotiating content must not disagree about what `q=0` means or which of two
equally-weighted entries a client meant first. `q=0` is a refusal and is dropped, not treated as a
weak preference — honouring it is the entire purpose of the weight.

## Decision 5 — unrecognised parameters are refused by name, including the protocol's own

`default-graph-uri` and `named-graph-uri` are SPARQL 1.1 Protocol parameters and they are
**refused**, not ignored. Implementing them means deciding how a protocol-supplied dataset
interacts with Decision 1's vocabulary-graph default *and* with a query's own `FROM` — a decision
worth making deliberately rather than in passing. Until then, a named refusal.

Every other unknown parameter is refused the same way, as `?format=turtel` already is on the export
endpoint. An ignored `?formt=csv` is a document in the wrong format carrying a correct-looking
`Content-Type`; an ignored `?default-graph-uri=` is a plausible answer about the wrong graphs. Both
are wrong in the way a caller checks for last. A repeated parameter is refused rather than resolved,
because "first wins" and "last wins" are both defensible and the caller cannot tell which they got.

The URL query string and the form body are decoded by the **same decoder** (`serde_urlencoded`,
already in the tree beneath axum's own extractors). Two decoders would eventually disagree about a
`+`, a `%`, or a repeated key, and the protocol defines both carriers.

## Decision 6 — the endpoint never writes, and says so by name

There is no update endpoint in this build. Text that parses as a SPARQL Update is refused as **an
update**, recognised by parsing it as one rather than by sniffing for a keyword — so
`SELECT ?insert WHERE …` is still a query, and a real update gets "that is a SPARQL Update, not a
query" rather than a syntax error at token three that sends someone hunting a typo in text that has
none.

Federation is likewise a *named* refusal: a `SERVICE` clause is a 501 stating that this build is
compiled without an HTTP client so it can run air-gapped, and that **nothing was sent** to the named
endpoint. It is matched on `QueryEvaluationError::UnsupportedService`, the variant, not the message
text. Until that arm existed a hand-run against the real binary returned a bare 500, which tells a
caller something broke rather than that this deployment deliberately has no federation.

## Decision 7 — the timeout's status code is the least-wrong of a bad set

A cancelled query is answered **503**, and this is recorded as a compromise rather than a fit.
RFC 9110 has no code for "the server cancelled a valid request against its own resource policy".
408 is about a slow *request*; 504 is about an upstream server, and there is none; 500 would claim
something went wrong when nothing did. 503 is the least-wrong, and the body carries the part that is
actually actionable.

**The cost is real:** a load balancer reading 503 may take the instance out of rotation over one
expensive query. That is the known downside of this choice, it is not hypothetical, and it is
recorded in `docs/UNTESTED.md` because no deployment has yet met it.

## Consequences

- A user's first query returns their vocabulary and nothing of ours, and the rule is written down
  and escapable rather than hidden.
- No single query can exhaust the server, and no caller is handed a partial answer that looks whole.
- The answer is buffered in the HTTP layer and only becomes a response on `Ok`, because
  `Store::query` may leave a partial document in the writer when it refuses. That makes "never send
  a truncated results document" a property of the code's shape rather than a rule to remember —
  and it means the endpoint does **not** stream, so a large answer is held in memory twice. Recorded
  in `docs/UNTESTED.md`.
- Evaluation runs on `spawn_blocking`. A blocking RocksDB scan on an async worker would stall every
  other request the runtime has.
- **There is no query console in the interface.** The item is scoped to the endpoint, and its
  production caller is HTTP. That is a real §4.4 gap for a user-facing capability and it is recorded
  in `docs/UNTESTED.md` rather than quietly folded into this item.
- The default dataset is computed per query from the registry, which is one extra registry read on
  every query. Unmeasured, and folded into the Phase 1 benchmark spike.
