# ADR 0010 — Serialising a graph, and exporting it over one URL

**Status:** accepted (2026-08-18) · **Phase:** 1

## Context

Phase 1's sixth item read "parse and serialise Turtle, N-Triples, N-Quads, TriG, RDF/XML, JSON-LD —
round-trip tested". It was split in two and only the serialise half was built. This ADR records
that split, the shape of the syntax type, and the contract the HTTP endpoint publishes.

Until this iteration nothing could get data *out* of an OpenBiz store except by reading the
RocksDB directory. That is the shape of a product a customer cannot leave, which is the opposite of
what `CLAUDE.md` §1.1 promises when it says the customer owns the data.

## Decision 1 — parsing is deferred, and the reason is a charter constraint

A serialiser's production caller is an export, which is a read. A parser's production caller is an
import, which **mutates a vocabulary** — and `CLAUDE.md` §3 says a change to a vocabulary arrives as
a reviewable *candidate*, carrying its provenance and its source, reviewed before it lands. The
candidate seam is the first item of Phase 2.

So building the parser now had two possible outcomes and both are bad. Either it lands with no
production caller, which `CLAUDE.md` §4.1 says is not done and belongs in `UNTESTED.md`; or it lands
with a direct-write `POST /api/import`, which is exactly the retrofit §3 exists to prevent —
"build direct writes now and every import, discovery, and agent path has to be retrofitted later".

The parser therefore lands with whichever comes first: **backup and restore** (later in Phase 1,
which parses N-Quads and touches no vocabulary) or **Phase 2's candidate seam**. Round-trip testing
did not have to wait for it: the test serialises, re-reads with the engine's parser, and compares
the statement set. What that proves is fidelity, not conformance, and `UNTESTED.md` says so.

## Decision 2 — `RdfSyntax` is ours, and it is narrower than the engine's

`oxrdfio::RdfFormat` is not re-exported, for the usual §3 reason and for one specific to this case:
the engine's list is **wider than our commitment**. It carries N3, which is not a W3C
Recommendation and is not on `CLAUDE.md` §2's standards surface. A re-export would have published a
seventh format we have never tested, documented, or committed to — a standards claim made by
accident, which §4.5 forbids. A test asserts the gap is deliberate: the engine still recognises
`text/n3` and `RdfSyntax::parse("text/n3")` is `None`.

Owning the type also means owning the contract: the media type, the file extension, and the
`?format=` token are ours, not whatever the parser happens to answer to. Two tests keep those from
drifting from the engine (`from_media_type` and `from_extension` must map each of ours back to the
right backend format), so a backend swap that disagreed with our published contract is a failing
test rather than a client that stops being able to re-read what we wrote.

**`records_graph_names` is the field that earns the type.** Turtle, N-Triples, and RDF/XML are
triple syntaxes and have nowhere to record which graph a statement belongs to. This is true of every
tool in this market and mentioned by none of them, so users find out from a re-import that lands in
the wrong place. Here the constant the serialiser branches on is the same one the API advertises and
the interface warns from, and a test asserts it agrees with the engine's own `supports_datasets`.

## Decision 3 — the export is exactly one graph, and nothing of ours

`Store::export_graph` reads one graph's quads and writes them. It does not filter our metadata out
on the way — our metadata was never in the vocabulary. That is `adr/0007`'s named-graph model
paying for itself: OpenBiz's bookkeeping lives in the system graph, inferences live in a derived
graph, so a vocabulary export cannot contain either.

This is the round trip §1.3 requires, and it is where the incumbents fail. PoolParty and TopBraid
EDG keep project metadata in the same store as the content, so an export carries tool-specific
bookkeeping a standards-compliant consumer has to be told to ignore. A test asserts that
`urn:openbiz:` appears in no vocabulary export, in any of the six syntaxes.

The system graph is itself exportable, deliberately. Making it unreachable would be the opacity §1
attacks; the rule is that our bookkeeping is never *mixed into* the user's work, not that an
operator cannot ask what is in their store.

## Decision 4 — a missing graph is a 404, and a missing format is a 400

An unregistered IRI is `StoreError::NoSuchGraph`, never an empty file. An empty export for a
vocabulary that does not exist is a valid, well-formed, entirely wrong document with nothing in it
to warn the caller. A *registered but empty* graph is an empty document, which is the correct and
different answer — and is proven to be a **readable** empty document in all six syntaxes, because
RDF/XML and JSON-LD both need a wrapper to parse at all.

Existence is decided by the **registry**, not by whether any quad names the graph. Deciding from
the data would report a created-but-empty vocabulary as absent, which is precisely the vocabulary a
user is most likely to be looking for.

Likewise a `?format=` we do not recognise is a 400 naming the six we have, and an `Accept` we cannot
satisfy is a 406. Falling back to the default would hand somebody who typed `?format=turtel` a file
in a format they did not ask for, and they would find out from their own parser.

## Decision 5 — one URL, negotiated, with headers that say what the file is

`GET /api/export?graph=<iri>&format=<token>`. A query parameter rather than a path segment because
a graph IRI contains `/`, and `%2F` in a path is normalised by enough proxies to make the URL
fragile. `/api/export` rather than `/api/graphs/export` so a future `/api/graphs/{id}` cannot
shadow it.

Exporting from the incumbents means a modal, a wizard, or a job to come back for. That is not a UI
complaint: it means the export cannot be scripted, scheduled, put in a runbook, or diffed in CI,
which is most of what a governance team wants from one. Here the interface and `curl` use the same
URL and neither is privileged.

`?format=` wins; otherwise `Accept` is negotiated with `q=` weights honoured, `q=0` treated as a
refusal, and `*/*` meaning the default. Subtype wildcards are not matched — guessing which of two
`text/…` syntaxes was meant is the silent substitution refused above. A browser's
`text/html,…,application/xml;q=0.9,*/*;q=0.8` gets Turtle, which is asserted, because
`application/xml` is in the engine's media-type table as RDF/XML and would otherwise win on weight.

Responses carry `Content-Type` with the media type, `Content-Disposition` with a filename derived
from the IRI's last segment and sanitised to a conservative ASCII set, and `X-OpenBiz-Graph` naming
the graph — percent-escaped, because an IRI may hold bytes a header value may not. That last header
exists so an export in a triple syntax still *states* which graph it is. Nothing reads it yet; that
is recorded in `UNTESTED.md` rather than presented as a feature.

`GET /api/export/formats` advertises the list. The UI and the server ship in one binary (§1.2), so
an interface offering a format the serialiser does not have would not be caught by a type check or
a deployment — only by a user picking it. Serving the list makes that divergence impossible, and it
is where the interface's lossy-syntax warning gets its facts.

## Decision 6 — the store streams; the HTTP layer buffers, and says so

`export_graph` writes each quad to the caller's writer as it is read, and the backend's iterator
holds **one snapshot for the whole scan** — read from Oxigraph's source, not assumed — so a commit
landing mid-export cannot tear the file. It takes no write lock, so an export never blocks an
author; that is asserted by exporting from inside an open write transaction.

The HTTP handler runs the export on `spawn_blocking`, because a whole-graph RocksDB scan on an
async worker thread would stall every other request. It then **buffers** the result into the
response body. That bounds a request by memory rather than by graph size, and it is the honest cost
of not building a streaming body today. `UNTESTED.md` records it and the Phase 1 benchmark spike now
owes a fifth number. The fix, if the number is bad, is `Body::from_stream` over a channel fed by the
blocking task — a change to one handler and to nothing above it.

## Consequences

- A customer can get their vocabulary out of OpenBiz with `curl`, in six syntaxes, with no export
  job and no consulting engagement. §1.1's "the customer owns the data" has a mechanism.
- The store still has **no term model of its own**. `Transaction::insert` remains private over the
  backend's triple type, because export needed no term to cross our boundary — only bytes. The term
  model arrives with the public write API it is actually for.
- Twelve mutants were confirmed to break these tests: four of the serialiser (registry check
  removed, graph name never written, every graph exported, document never finished), three of the
  endpoint (weights ignored, unsatisfiable `Accept` silently defaulted, filename left unsanitised),
  two of the syntax type (unknown format defaulted, graph-name claim disagreeing with the engine),
  and three of the interface (link ignoring the chosen format, warning removed, IRI spliced in
  unescaped).
