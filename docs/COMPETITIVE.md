# Competitive & standards research

Living document. The loop appends findings; it does not delete prior entries — a superseded finding
gets a dated correction beneath it, so we can see when we were wrong and why.

**Rule for this file:** every claim carries a source and a date. An unsourced assertion about a
competitor is a liability, not research. Vendor marketing pages describe intent; practitioner
reviews describe reality. Weight them accordingly, and say which one a claim came from.

---

## The market, as of 2026-08

Consolidation is the defining fact. **Graphwise** is the merger of **Ontotext** (GraphDB
triplestore) and **Semantic Web Company** (PoolParty), and **metaphacts** (metaphactory) sits under
the same umbrella. Three of our four commercial competitors are now one vendor with one roadmap.

That is a genuine opening. Consolidation means integration work absorbing roadmap capacity, a
single commercial posture to price against, and enterprises newly nervous about concentration risk
in a governance-critical system. "Second source" is a real buying motive in regulated industries.

---

## Competitors

### PoolParty Semantic Suite (Graphwise)
Enterprise semantic platform: taxonomy and ontology management, graph-based text mining, concept
tagging, semantic search, GraphRAG. The most feature-complete incumbent, and the leader in text
mining and auto-tagging.

**Documented weaknesses** — from Gartner Peer Insights practitioner reviews:
- Steep learning curve; stakeholders from a SQL background or without formal KM training struggle.
- "Very high consulting fee" for vendor implementation advice.
- Bugs that block implementation, with incomplete vendor responses.
- **No clear release roadmap; limited visibility into how and when issues get fixed.**

That last one is the most exploitable. It is a *structural* consequence of closed development, so
they cannot fix it without changing how they operate. Our roadmap being a public git repo is a
permanent, non-copyable differentiator.

### metaphactory (Graphwise)
Enterprise knowledge-graph platform: visual semantic modelling, vocabulary and taxonomy management,
data-catalog integration, collaboration, asset governance, AI-assisted modelling. Strong on
end-user-facing knowledge-graph *applications* — a genuine strength we should not pretend away.
Heavy deployment (JVM plus a backing triplestore).

### TopBraid EDG (TopQuadrant)
The governance-first incumbent and the closest to our positioning. Deep SHACL heritage — SHACL's
principal authors came from TopQuadrant. Strongest on data governance and reference-data workflows.
Expect them to be the hardest to beat on governance depth; we beat them on deployment weight,
price, and interface.

### Protégé / WebProtégé (Stanford)
Free, academic, enormous installed base, the default for ontology *authoring*. Desktop Protégé is
the OWL editing benchmark. WebProtégé is collaborative but dated. Not an enterprise governance
product and does not try to be — but it sets user expectations for what OWL editing feels like, so
our ontology editor gets measured against it.

### VocBench 3 (University of Rome Tor Vergata / EU Publications Office)
The open-source competitor and the one to study hardest. MPL-licensed collaborative platform for
OWL ontologies, SKOS/SKOS-XL thesauri, OntoLex-Lemon lexicons, generic RDF. Used by the EU
Publications Office for production vocabularies.

**Genuinely strong at:** collaboration with workflow-managed content validation and publication;
dedicated user roles separating management from vertical editing competences; tabbed structures per
modelling vocabulary (concept tree, schemes, collections, class/property trees, lexicons); direct
SPARQL 1.1 query/update for power users.

**Where we differentiate:** deployment weight (Java/Tomcat plus a separate RDF4J or GraphDB backend
versus our one binary), interface modernity, and GitHub-native change management. Note that VB3
users reportedly wanted *more* modelling freedom than plain SKOS/SKOS-XL — evidence that
extensibility beyond the base model is a real requirement, not a nice-to-have.

**Do not underestimate their role model.** Their editorial roles are the product of a decade of
real thesaurus practice at a large publisher. Ours should learn from it rather than reinvent it.

---

## Standards notes

- **ISO 25964-1/-2** is the thesaurus standard, with a documented mapping to SKOS and SKOS-XL.
  Where the base SKOS model lacks a construct for an ISO 25964 feature, the mapping uses SKOS-XL,
  supplemented by additional proposals. **Implication: SKOS-XL is not optional for us** — plain
  SKOS cannot faithfully represent an ISO 25964 thesaurus, and our enterprise buyers will have
  ISO 25964 in their requirements.
- **ISO 25964-2** covers mapping *between* vocabularies — thesauri to classification schemes,
  taxonomies, subject headings, ontologies, name authority lists, terminologies, synonym rings.
  This is exactly the "centralise and govern many vocabularies" problem our buyer has.
- **ANSI/NISO Z39.19** and ISO 25964 are best-practice guidelines, not just data models. A
  SKOS-valid vocabulary can still be a badly designed one. Encoding these as **SHACL rule packs**
  turns editorial best practice into something machine-checkable — a feature the incumbents largely
  leave to human reviewers.

## Rust ecosystem assessment

Recorded because the single-binary commitment costs us the JVM's mature semantic-web stack, and we
should be honest about the size of that bill.

- **Oxigraph** — SPARQL 1.1 Query, Update, and Federated Query; Graph Store Protocol; records empty
  graphs and supports CREATE/DROP properly. RDF 1.2 and SPARQL 1.2 behind the `rdf-12` feature.
  **Stated limitation: SPARQL query evaluation is not yet optimised.** Also has precision limits on
  numeric/calendar/duration XSD literal encodings, with arithmetic undefined outside them. Both need
  benchmarking before we depend on them; the second one matters for any vocabulary carrying
  quantitative or temporal metadata.
- **horned-owl** — OWL 2 data model and IO (RDF/XML, OWL functional syntax), designed for
  performance and pluggability rather than GUI support, which suits a server. Has an ecosystem:
  `py-horned-owl`, `horned-bin`.
- **whelk-rs** (INCATools) — a Rust port of the Whelk reasoner. **EL profile only.**
- **rustdl** (MaastrichtU-IDS) — Rust OWL DL reasoner targeting HermiT/Konclude parity. Early;
  watch, do not depend.
- **SHACL**: `oxirs-shacl` claims a production release with core constraints, property paths, and
  logical constraints; `shacl_validation` and `shacl-rust` are alternatives. **None yet verified by
  us — a spike must compare them against the W3C SHACL test suite before one becomes load-bearing.**

**Honest conclusion:** there is **no OWL 2 DL reasoner in Rust**. Our realistic reasoning target is
**EL + RL**, which covers the large majority of enterprise ontologies (SNOMED CT and the Gene
Ontology are both EL) but is a genuine capability gap against Protégé-plus-HermiT for expressive DL
ontologies. This must be stated plainly in our documentation rather than glossed. If DL reasoning
becomes a deal-blocker in real evaluations, the escalation path is an optional external reasoner
sidecar — which would trade against the single-binary rule and therefore needs an ADR and human
sign-off.
