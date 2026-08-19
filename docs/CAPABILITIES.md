# What OpenBiz does today

This is the honest answer to *"what does this actually do right now?"* — written for a person
evaluating OpenBiz, not as a log of how it got here.

**It is prose, not a changelog.** When a capability changes, the paragraph describing it is
*rewritten*, not appended to. If you want the history, `docs/LOOP-LOG.md` has one entry per
iteration and `docs/adr/` has the decisions.

> **Pre-alpha.** OpenBiz is 55 of 221 backlog items — Phase 0 and most of Phases 1 and 2. There is
> a working command line over a real embedded RDF store and a minimal web interface. There is **no
> authentication**, no validation rule packs, no reasoner, no governance workflow beyond
> approve/reject, and no LLM assistance. Do not put a vocabulary you care about in it yet.
>
> Everything below is checked off in [`BUILD-PLAN.md`](BUILD-PLAN.md) and its known gaps are in
> [`UNTESTED.md`](UNTESTED.md). Anything not described here is not built.

---

## The shape of it

One executable. It carries the HTTP server, the web interface, and an embedded
[Oxigraph](https://github.com/oxigraph/oxigraph) RDF store — no JVM, no external triplestore, no
application server. A CI job proves this the only way that means anything: it deletes `ui/dist`
from disk and asserts the release binary still serves the whole interface.

It runs air-gapped **structurally**, not by promise: the store is built with the embedded engine's
HTTP client disabled, so the binary contains no code path that can open an outbound connection.
That is also why SPARQL `SERVICE` federation is absent rather than merely unimplemented.

A store holds **one named graph per vocabulary**, plus a system graph
(`<urn:openbiz:graph:system>`) for OpenBiz's own bookkeeping. The `urn:openbiz:` prefix is reserved
against user authoring, there is a single write choke point every write passes through, and writes
are transactional and serialised. Stores carry a **format version** with forward-only migrations
that record in the store itself what was done to it and when — so a SPARQL query answers *"what has
been done to this store?"* long after the log line has scrolled away.

---

## Getting data in and out

```sh
openbiz import <graph> <file>     # propose the file's statements as additions
openbiz retract <graph> <file>    # propose the file's statements as removals
openbiz backup <file>             # write the whole store to one N-Quads file
openbiz restore <file>            # rebuild an empty store from one
```

All six serialisations OpenBiz commits to are readable and writable: **Turtle, N-Triples, N-Quads,
TriG, RDF/XML, JSON-LD**. The syntax of an import is taken from the file extension. N-Triples and
N-Quads are checked against a reader written from the published EBNF rather than against the
library that wrote the bytes — which found two real defects nobody had seen ([`adr/0012`](adr/0012-conformance-testing-and-two-defects.md)).

A backup is the whole store as N-Quads — every vocabulary *and* the graph registry — so it is
readable by any conforming tool, line-based enough to `grep` and `diff`, and independent of what
OpenBiz stores data in. It refuses to overwrite an existing file, because the file most likely to
be in the way is the last good backup. A restore needs an **empty** store, runs in one transaction,
refuses anything that would not open afterwards, and migrates an older backup forward as it reads —
saying so, because *"restored 12 000 statements"* looks identical whether or not your data was
changed on the way in.

Both need the store to themselves, so **stop the server first**. There is no online backup yet.

Over HTTP, `GET /api/export` serialises any registered graph to any of the six syntaxes, and
`GET`/`POST /api/sparql` evaluates SPARQL 1.1 **queries** in all four result formats over a default
dataset that is the user's vocabularies and none of OpenBiz's own graphs, bounded by limits that
refuse rather than truncate.

---

## Reading a vocabulary

```sh
openbiz inspect <graph>      # what does this hold, in SKOS terms, and why?
openbiz integrity <graph>    # which SKOS integrity conditions does it satisfy?
openbiz notes <graph> <resource>     # what does it document this with?
openbiz mappings <graph> <resource>  # what is this joined to?
```

**Everything inferred is printed with its derivation.** Not "the tool says so" — the statement it
followed from and the clause of the SKOS Reference that licensed it:

```
<…/scheme> rdf:type skos:ConceptScheme
  because <…/scheme> skos:hasTopConcept <…/apac>
  and S5: The rdfs:domain of skos:hasTopConcept is the class skos:ConceptScheme.
```

That is the feature, not decoration. A governance team defending a decision to an auditor needs to
show *why* a concept is in a scheme.

What the model covers today:

- **SKOS core** — concepts, schemes and collections, including the ones no statement typed, because
  SKOS entails them. A graph saying `<C> skos:inScheme <S>` has a concept scheme whether or not
  anyone said so, and a tool counting only `rdf:type` reports zero schemes for a large share of real
  thesauri ([`adr/0019`](adr/0019-skos-core-model.md)).
- **Lexical labels**, per language, with S13 and S14 enforced, and a report of which languages a
  thesaurus is actually in and how far behind each one is ([`adr/0020`](adr/0020-lexical-labels-and-plain-literals.md)).
- **SKOS-XL**, both halves — a label with an IRI you can date, attribute and version, plus the
  S55–S57 chains that dumb it down to a plain SKOS label, so a concept labelled *only* through
  SKOS-XL still counts towards language coverage ([`adr/0021`](adr/0021-skos-xl-labels-and-dumbing-down.md)).
  `skosxl:labelRelation` is read, because it is where ISO 25964 puts an acronym relationship and
  plain SKOS cannot express it ([`adr/0022`](adr/0022-skos-xl-label-relations.md)).
- **Semantic relations** (§8) read and closed, so a hierarchy written in one direction reads in
  both, and polyhierarchy is counted rather than complained about ([`adr/0023`](adr/0023-semantic-relations-and-the-super-property-citation.md)).
- **Documentation properties** (§7), with S17 lifting the six specific ones onto `skos:note` — the
  one view a Turtle export of the same vocabulary cannot show. A vocabulary's *own* refinements
  reach the report too: `ex:usageNote rdfs:subPropertyOf skos:scopeNote` is read, because §7.1 calls
  the seven "a set of extension points" and enterprise thesauri use them
  ([`adr/0026`](adr/0026-documentation-properties.md), [`adr/0028`](adr/0028-reading-a-vocabularys-own-note-refinements.md)).
- **Mapping properties** (§10) read and closed (S38–S46), including S45's transitivity — so
  `<A> exactMatch <B> exactMatch <C>` with `<A> broadMatch <C>` is caught, which is the hub shape an
  enterprise actually produces and which no statement in the graph names directly
  ([`adr/0029`](adr/0029-mapping-links-are-lifted-into-the-hierarchy.md), [`adr/0030`](adr/0030-the-exact-match-closure-is-a-walk-over-a-cluster.md)).

`openbiz integrity` is the roll-call: every condition whose violation makes this build call a graph
inconsistent, one row each — the specification's six under the heading SKOS gives them, and the ten
this build classifies itself printed apart and **labelled as ours**. Each row is **held, violated,
or unchecked**, and the third is not a weaker first: a bounded walk that stopped leaves a condition
genuinely unanswered and the report says which and why. A check that gave up is not a check that
passed ([`adr/0031`](adr/0031-the-integrity-conditions-are-a-roll-call.md)).

Where a judgement is ours rather than the specification's, it says so. Two `skos:memberList` values
on one resource looks like an S35 violation and is consistent SKOS: we report it and name the
judgement as ours. A concept with **no definition** is likewise consistent SKOS — requiring one is
ANSI/NISO Z39.19's rule or ISO 25964's, and belongs in a rule pack you can name and switch off
(Phase 4), not in a finding citing a statement nobody made.

All four commands only read. A test asserts the store is byte-for-byte unchanged.

---

## Navigating it

```sh
openbiz ancestors <graph> <concept>   # what is above it, and by what path
openbiz paths <graph> <concept>       # every route up to a root, and the cycles they hit
openbiz tree <graph> <concept>        # what is below it and beside it
openbiz search <graph> <text>         # find concepts by a word, not by an IRI

# and any of the four with --current, to leave the retired concepts out and be told what that cost
openbiz ancestors <graph> <concept> --current
openbiz paths <graph> <concept> --current
openbiz tree <graph> <concept> --current
openbiz search <graph> <text> --current
```

**The transitive closure is never stored, at any vocabulary size.** A legal 100 000-link SKOS chain
licenses five thousand million pairs, and a stored pair could cite the rule but not name the path it
took — which the charter's explainability requirement forbids. So it is walked on demand, the path
*is* the derivation, and a walk that hit its bound says so ([`adr/0024`](adr/0024-semantic-relation-closure-scale.md),
[`adr/0025`](adr/0025-transitive-ancestry-by-walking.md)).

Three distinctions the reports keep that most tools blur:

- **A route to a concept with no broader concept is not a route to a scheme's top concept.** SKOS
  relates the two nowhere at all, so both are reported and kept apart ([`adr/0033`](adr/0033-every-route-to-a-root-and-the-cycle-it-runs-into.md)).
- **A child is not a descendant one step down.** S22's entailment runs one way, so a stated
  `skos:narrowerTransitive` link makes a concept a descendant with no place in the stated tree, and
  the report names S22 when a vocabulary shows the difference. "Sibling" is our word, labelled as
  ours ([`adr/0032`](adr/0032-the-concept-tree-read-downwards.md)).
- **A tree gives each concept one parent**, so the routes it could not show are counted and named
  after it rather than left to a reader to not notice.

`openbiz search` is the only command that starts from a *word* rather than an IRI a person already
holds. It matches preferred, alternative **and hidden** labels — the last because SKOS §5.1 justifies
that property in terms of text search — with RFC 4647 basic language filtering, and every default set
to the forgiving one: a search that finds nothing is precisely how a duplicate concept gets created.
Narrowing is explicit, and two options that narrow the same thing are refused rather than resolved
last-wins ([`adr/0034`](adr/0034-searching-labels-is-forgiving-by-default-and-says-what-it-did-not-do.md)).

**A retired concept reads as retired everywhere** — `tree`, `ancestors`, `paths`, `search` and
`inspect` all know what `owl:deprecated` means. The decision is **show and mark, never hide**:
hiding a retired concept leaves its live children hanging off nothing, and hiding a search hit
reports a term the vocabulary holds as one it has never heard of. Each report also says what its
marks add up to — the current concepts a retirement left below it, the retired concepts a breadcrumb
runs through, the vocabulary's whole retirement backlog as counts and never as findings
([`adr/0041`](adr/0041-a-retired-concept-is-shown-and-marked-never-hidden.md)).

**Leaving them out is a request, not a default.** `openbiz search <graph> <text> --current` gives a
curator drafting a new branch the list without the obsolete terms in it — and always closes with how
many labels it withheld, on how many retired concepts, and how to see them. Especially when they were
*everything* that matched: a report that said "nothing matched" about a term the vocabulary holds is
the false negative the default exists to prevent. The exclusion runs inside the scan, so the result
limit is spent on hits you will actually see
([`adr/0043`](adr/0043-current-only-hides-the-hits-and-never-the-count.md)).

**`openbiz tree --current` narrows a hierarchy, which is a different question with a different
answer.** Because a deprecation touches nothing below it, a retired concept with current concepts
under it is the *commonest* outcome of a retirement — so a narrowed tree drops a branch **only when
the whole branch is retired**, and keeps a retired concept that current ones hang off, marked as the
route to them. Nothing is lifted and nothing is re-parented: every concept the narrowed tree shows
keeps the depth, the parent and the derivation the full tree gave it, so narrowing can never make
the tree state a link the vocabulary does not. The counts close the report either way, including the
case where every descendant is retired and the tree would otherwise read as a leaf
([`adr/0044`](adr/0044-a-branch-goes-only-when-the-whole-branch-is-retired.md)).

**Looking *up*, the flag splits again, and the two halves disagree on purpose.** `ancestors` asks
which concepts are above one: a concept reachable only *through* a retired one is still above it —
retiring removes no link — so the retired concept leaves the list and everything above it stays. Its
**path is printed whole**, because the path is the derivation and cutting a concept out of
`A → B → C` would state that `C` is directly above `A`. `paths` asks by what *routes*, and a route
is atomic: it is offered only if every concept on it is current, and one that is not is withheld
entire rather than shortened past. The cycles are never narrowed — a cycle is why a route reaches no
summit, so leaving one out deletes the explanation and keeps the problem
([`adr/0045`](adr/0045-current-concepts-on-routes.md)).

So `--current` is on all four commands under three different rules, because they answer three
different questions — and every one of them obeys the same single rule: **hide the concepts, never
the fact that there were concepts.** The case that proves it in each is the one where the flag
withholds *everything*. An emptied ancestor list would otherwise say a concept has no broader
concept when the vocabulary puts two over it; an emptied route list would blame a cycle that need
not exist. Both are a false negative about the hierarchy above a concept, which is how the wrong
parent gets chosen for the next one.

---

## Creating a concept — what already exists comes first

```sh
openbiz mint <graph> [<label>] [--pattern <p>]   # what already exists, then what IRI a new one gets
openbiz policy <graph> [--pattern <p>]           # show, or record, the pattern
```

There is no "create concept" command. A concept is created by staging a change, and to write that
change somebody has to decide its IRI — which makes `mint` the creation path, and **discovery runs
on it, before the IRI and with no flag to enable**. Given a label, `mint` searches every vocabulary
in the store and every change waiting for a decision — every label kind, any language, anywhere
inside the label — and prints what it found above the IRI, each match with the vocabulary it lives
in. A term that already exists in the vocabulary you are *not* looking at is the concept you were
about to duplicate.

The IRI is still offered: two concepts can legitimately share a label, and a tool that refuses on a
lexical match is one people work around. What the report will not do is let "nothing found" read as
"nothing exists" — it always names what answered, what each source read, and what was never asked.
No peer, data catalog, or public registry is consulted, because this build has no connector for one
(Phase 12), and the report says so on every run. A source that cannot answer is reported as
unavailable and never blocks the mint
([`adr/0046`](adr/0046-discovery-runs-on-the-creation-path.md)).

Matching is lexical: case-insensitive, and **not** insensitive to accents, spelling, or Unicode
normalisation. What `adr/0003` §3 calls the reuse ladder is printed when something is found, and
the report is honest that this build has nowhere to record a justification for creating a new
concept anyway, except the note on the change that creates it.

**`openbiz split` is the other creation path, and the same pass runs on it** — under *every* part
name, because a split names several concepts at once and each one of them is a creation. The check
it had before looked only in the vocabulary being edited, which is the check §1.7 exists to say is
not enough: a term is divided because it meant two things, and one of the two very often already
exists elsewhere under that name. The answers are given under each part's own name, the sources are
named once for the command, and the whole thing costs **one** reading of the store however many
parts are named. A part named after one of the original's own labels is shown and annotated as the
concept being divided rather than offered as a concept to reuse
([`adr/0048`](adr/0048-discovery-on-every-name-a-split-creates.md)).

`mint` **reads and reserves nothing** — run it twice, get the same answer, and it says so. A number
goes above the highest in use and never fills a gap; a slug already taken is *refused* rather than
given a disambiguating suffix. Collisions are checked across every vocabulary in the store and every
change staged against one ([`adr/0035`](adr/0035-an-iri-is-minted-from-what-the-vocabulary-already-does.md)).

`policy` is where the pattern stops being a guess about the vocabulary and becomes a recorded,
attributed decision — written to the system graph and **never into the vocabulary**, so it does not
travel as content. `mint` takes the first of `--pattern`, the record, then inference, and refuses a
recorded pattern it cannot parse rather than falling back to a namespace nobody chose
([`adr/0036`](adr/0036-the-minting-pattern-is-a-recorded-decision.md)).

---

## Changing a vocabulary

**Nothing writes to a vocabulary directly.** Every change — an import, a bulk operation, and one day
a discovery match or an LLM proposal — is a **candidate**: statements staged in a named graph of
their own (`<urn:openbiz:graph:candidate:<id>>`), plus a record of who proposed what and why.

```sh
openbiz candidates          # what is waiting for a decision
openbiz candidate <id>      # one proposed change and the statements it would add or remove
openbiz approve <id>        # apply it — recording who approved it, in the same transaction
openbiz reject <id>         # refuse it
```

`openbiz help` prints all 24 commands. Exit status is `0` on success, `1` if the operation
failed, and `2` if the arguments were not understood — so a wrapper script can tell *retry this*
from *you typed it wrong*. Approving and rejecting need a name to record: `OPENBIZ_ACTOR` if it is
set, otherwise `USER` or `LOGNAME`.

Because a candidate is a real named graph, a pending change is **exportable in any of the six
syntaxes and queryable over SPARQL on the day it exists**. Approval is a copy between two graphs
*inside the transaction that records the approver*, so the store can never hold statements in a
vocabulary with no record of who let them in. Provenance is mandatory and its source is a closed
token, so *"show me everything an assistant proposed"* is answerable before there is an assistant
([`adr/0017`](adr/0017-candidate-seam.md)).

A candidate has two halves: statements to add and statements to remove. A removal names statements
that must already exist, checked at proposal **and again inside the transaction that applies it**, so
an approval the vocabulary has outgrown is refused rather than partly applied
([`adr/0018`](adr/0018-candidate-removals.md)).

The editorial operations all produce candidates:

| | |
|---|---|
| `openbiz move <graph> <concept> <to>` | Re-parents a concept **and everything below it** as *one* candidate that both removes and adds — approving half of a move would leave a branch hanging off nothing ([`adr/0037`](adr/0037-a-move-is-one-candidate-with-two-halves.md)). |
| `openbiz merge <graph> <duplicate> <survivor>` | Repoints every reference in the vocabulary, **including statements SKOS has no reading of** (which is why it reads the raw graph, not the model), demotes a colliding preferred label rather than dropping it, and **refuses any change that would leave the graph failing an integrity condition that holds now** ([`adr/0038`](adr/0038-a-merge-is-checked-against-the-vocabulary-it-would-leave.md)). |
| `openbiz split <graph> <concept> --into … --into …` | Asks what already exists under every part name first, across the store ([`adr/0048`](adr/0048-discovery-on-every-name-a-split-creates.md)), then creates the parts under the vocabulary's own minting policy, records with `prov:wasDerivedFrom` where each came from, and **removes nothing** — then reports every label, child, link and note still hanging off the original that only a person can apportion ([`adr/0039`](adr/0039-a-split-creates-the-parts-and-refuses-to-apportion.md)). |
| `openbiz deprecate <graph> <concept> [--replaced-by …]` | Retires a term **in place**: marks it `owl:deprecated`, records the successor with `dcterms:isReplacedBy`, and **deletes nothing at all**, so the IRI keeps resolving — the one thing a merge cannot offer. SKOS has no deprecation term, so both come from OWL 2 and Dublin Core rather than from anything invented here ([`adr/0040`](adr/0040-a-deprecation-retires-a-concept-and-strands-what-it-cannot-decide.md)). |
| `openbiz reinstate <graph> <resource>` | Takes a retirement back, removing the marker and the recorded successor **together** — a current concept that records a successor is a contradiction — and keeping every `skos:changeNote`, because the retirement happened and a history tidied until it never appears is the opaque change log this product exists to replace ([`adr/0042`](adr/0042-a-reinstatement-removes-the-status-and-keeps-the-history.md)). |

**Every record in the trail says which clock it is on.** A candidate raised, a candidate decided,
an IRI policy recorded and a migration applied are each stamped in **UTC**, as a typed
`xsd:dateTime` — so the trail is not merely readable but *orderable*, by an ordinary SPARQL query
over `<urn:openbiz:graph:system>` rather than by trusting our own rendering of it. A record whose
timestamp names no timezone cannot be placed against any other record, so the store refuses one on
the way back in rather than showing a reviewer a date nobody can order
([`adr/0047`](adr/0047-the-audit-trail-says-which-clock.md)).

The refusals matter as much as the operations. `merge`'s first working version produced a vocabulary
violating S14 and S27 from ordinary input, so the check is now the *whole* condition set run against
the vocabulary the change would leave — not the subset an author would have predicted. And every one
of these reports, **before** the diff, what it could not decide: the children, links, mappings and
collection memberships a change stranded, which only a person can apportion.

---

## The web interface

Minimal and honest about it. It lists the vocabularies in the store — keeping OpenBiz's own graphs
out of the user's list, counted rather than hidden — and offers a per-vocabulary export with a format
chooser read from the server.

That is all of it. The interface is **Phase 3, and Phase 3 has not started**: 0 of 12 items. None of
the command line above is reachable from a browser yet.

---

## What is not built

Stated plainly, because a roadmap you cannot trust is one of the things we are attacking the
incumbents for.

- **No authentication or authorisation**, anywhere. This is why SPARQL Update and the Graph Store
  Protocol are deliberately absent (an unauthenticated write endpoint is not a feature),
  `POST /api/graphs` answers **405**, and the candidate seam has no HTTP or UI half. It is the single
  largest thing standing between OpenBiz and being usable by more than one person.
- **No validation rule packs.** SHACL, ISO 25964 and Z39.19 checks are Phase 4, 0 of 11.
- **No reasoning.** OWL 2 EL/RL is Phase 5, 0 of 9. Phase 9's OWL model and IO is **blocked**: the
  obvious Rust crate, `horned-owl`, is LGPL-3.0, which the licence policy forbids in the core, and
  the replacement is a commercial decision a human has to take ([`BLOCKED.md`](BLOCKED.md)).
- **No governance workflow** beyond a candidate and one approve/reject. No review assignment, no
  roles, no signoff chain. Phase 6, 0 of 9.
- **No lifecycle or methodology packs and no Solution Advisor.** Phase 7, 0 of 21.
- **No GitHub integration.** Vocabularies are not yet branches and pull requests. Phase 8, 0 of 10.
- **No LLM assistance and no provider.** The default is, and will remain, none. Phase 10, 0 of 21.
  The *seam* it will plug into — candidates carrying provenance and reviewed before they land —
  exists today and is what everything above already writes through.
- **No discovery source but the local store.** `DiscoveryProvider` exists and runs on both creation
  paths, over every vocabulary in the store and every change waiting for a decision. No peer, data
  catalog, or public registry has a connector — Phase 12 — and every report says so on every run
  rather than letting "nothing found" read as "nothing exists".
- **No SPARQL console** in the interface, though the endpoint is there.
- **No online backup** — taking one means stopping the server.

## What is proven narrowly

The gaps we know about are individually written down in [`UNTESTED.md`](UNTESTED.md) rather than
summarised into comfort. The ones that would most affect an evaluation:

- **Memory.** A stated semantic relation costs **3.9 KiB resident** — 43× the size of the fact — and
  a million-link vocabulary with no labels at all held **4.4 GiB**; a realistic million-concept
  polyhierarchy peaked at **8.2 GiB**. That is measured, recorded, and not yet fixed.
- **Search does not scale.** Every search is a linear scan of a model rebuilt per request. Nothing
  indexes anything.
- **Label matching ignores case but not accents, spelling, or Unicode normalisation**, so real
  thesaurus labels can be unfindable.
- **Several bounds are judgements, not measurements.** The constants that stop a walk, a search, or
  a slug search are named in `UNTESTED.md` with what has and has not been measured against each.
- **No fixture here is a real extended thesaurus.** Everything is generated or hand-written.

---

## Where the detail is

| | |
|---|---|
| [`BUILD-PLAN.md`](BUILD-PLAN.md) | The backlog and the burn-down, phase by phase. |
| [`adr/`](adr/) | Every architectural decision, with what was measured, not just what was chosen. |
| [`UNTESTED.md`](UNTESTED.md) | Built but unproven, or proven only narrowly. |
| [`BLOCKED.md`](BLOCKED.md) | What cannot proceed, and exactly what would unblock it. |
| [`PROPOSED.md`](PROPOSED.md) | Work believed necessary but not yet authorised. |
| [`LOOP-LOG.md`](LOOP-LOG.md) | How it got here, one entry per iteration. |
| [`COMPETITIVE.md`](COMPETITIVE.md) | The incumbent research the product positioning rests on. |
