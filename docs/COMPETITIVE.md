# Competitive & standards research

Living document. The loop appends findings; it does not delete prior entries — a superseded finding
gets a dated correction beneath it, so we can see when we were wrong and why.

**Rule for this file:** every claim carries a source and a date. An unsourced assertion about a
competitor is a liability, not research. Vendor marketing pages describe intent; practitioner
reviews describe reality. Weight them accordingly, and say which one a claim came from.

**Second rule, added 2026-08-19 after a product-owner correction (`docs/FEEDBACK-LOG.md`):
retiring a claim here does not retire it anywhere else.** This file is research; the README,
`CLAUDE.md`, the plan, and source doc comments are where the claim is *published*. A correction
recorded only here leaves the repository asserting, in public, something its own research no longer
supports — which `CLAUDE.md` §4 counts as misreporting, and which `/openbiz-status` reads as charter
drift. So: **when a pass retires a claim, grep the whole repo for it and fix every instance in the
same iteration**, and add a row to the table below saying where it was fixed. Append-only records —
`LOOP-LOG.md`, `FEEDBACK-LOG.md`, the dated ADRs, and this file's own superseded paragraphs — are
history and are annotated in place rather than edited.

### Retired claims and where they were published

| Claim, as it used to read | Retired | What is true instead | Fixed in |
|---|---|---|---|
| "There is no OWL 2 **DL** reasoner in Rust" (absolute existence claim) | 2026-08-18, iteration 25 | No Rust OWL 2 DL reasoner is *mature enough for us to depend on*. `rustdl` (Apache-2.0, MaastrichtU-IDS) exists and is active. **The practical conclusion is unchanged: EL + RL remains our target.** | 2026-08-19, iteration 27 — `README.md` §Standards, `CLAUDE.md` §3 candidate list, `crates/openbiz-owl/src/lib.rs` module docs and `Profile::Dl`. This file's own §"Rust ecosystem assessment" conclusion annotated in place. |
| "`horned-owl` is our OWL 2 model/IO candidate" | 2026-08-18, iteration 25 | `horned-owl` is **LGPL-3.0**, forbidden in the core by `CLAUDE.md` §5. The replacement is an open commercial decision, recorded in `BLOCKED.md`, not a spike the loop may take. | 2026-08-19, iteration 27 — `CLAUDE.md` §3 crate map, §3 candidate list, and the §5 example that used it to illustrate a *merely unlisted* licence. |
| "ISO 25964-2:2013 was confirmed in 2023 and is **unchanged**" | 2026-08-19, iteration 50 | The 2023 confirmation is correct, but **revision work on Part 2 has now recently started** (Taxonomy Strategies, 2026-04-15). Part 2 is the vocabulary-*mapping* standard our mapping features implement against. | 2026-08-19, iteration 50 — this file's iteration-25 paragraph annotated in place; `docs/PROPOSED.md`'s ISO re-citation proposal updated. No other file asserted it. |
| "ISO 25964-1's revision is **expected to publish in 2026**" (a forecast, repeated by us) | 2026-08-19, iteration 50 | It is at **FDIS** — `ISO/FDIS 25964-1`, Edition 2, the approval stage before publication — replacing ISO 25964-1:2011, and **the title changes** to "...for information retrieval, **management and use**". Not retired as wrong, retired as **too vague to act on**; the ISO stage code and publication date are still unverified behind a 403. | 2026-08-19, iteration 50 — `docs/UNTESTED.md`'s entry narrowed, `docs/PROPOSED.md`'s ISO proposal updated, this file annotated in place. Our citations still say "ISO 25964-1:2011", which remains **correct** and needed no change. |

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

> **Superseded on 2026-08-18 — read the correction below before quoting this paragraph.** The
> absolute existence claim is too strong; `rustdl` exists and is Apache-2.0. The practical
> conclusion (EL + RL) is unchanged. See "Corrections to the assessment above".

**Honest conclusion:** there is **no OWL 2 DL reasoner in Rust**. Our realistic reasoning target is
**EL + RL**, which covers the large majority of enterprise ontologies (SNOMED CT and the Gene
Ontology are both EL) but is a genuine capability gap against Protégé-plus-HermiT for expressive DL
ontologies. This must be stated plainly in our documentation rather than glossed. If DL reasoning
becomes a deal-blocker in real evaluations, the escalation path is an optional external reasoner
sidecar — which would trade against the single-binary rule and therefore needs an ADR and human
sign-off.

---

# Product-owner pass — 2026-08-18 (iteration 25)

Re-run of the market, standards, and ecosystem research per `CLAUDE.md` §7 and the loop's
every-25th-iteration rule. Prior findings above are left standing; corrections and updates are
below, dated, per this file's own rule.

## The headline: our named OWL dependency is LGPL-3.0

**`horned-owl` is licensed LGPL-3.0.** Verified three ways on 2026-08-18: the crates.io metadata for
every published version including the current `3.0.0` (`license = "LGPL-3.0"`), the `license` field
in the repository's own `Cargo.toml`, and the presence of `COPYING` plus `COPYING.lesser` — GPLv3
and LGPLv3 — at the repository root. There is no permissive dual-licence option offered.

`CLAUDE.md` §5 forbids LGPL in the core, without qualification, and routes exactly this case to
`BLOCKED.md`. `CLAUDE.md` §3 and §5 both name `horned-owl` as the OWL 2 candidate, and §5 offers it
as an example of a dependency whose licence might be *merely unlisted*. It is not merely unlisted;
it is on the forbidden list. **That is a collision inside the charter itself**, and the charter's own
answer is that a copyleft dependency is a commercial decision a human takes. Recorded in
`docs/BLOCKED.md`; `docs/PROPOSED.md` carries the options.

Sources: <https://crates.io/api/v1/crates/horned-owl>,
<https://github.com/phillord/horned-owl> (repository root, 2026-08-18).

## Rust ecosystem — measured, not recalled

Every figure below is from the crates.io API or the GitHub API on 2026-08-18.

| Crate | Version | Licence | Last publish | Downloads (all / 90d) | Read |
|---|---|---|---|---|---|
| `oxigraph` | 0.5.9 | Apache-2.0 | 2026-06-18 | 682k / 486k | Healthy. We pin `0.5`, lock at 0.5.9. |
| `horned-owl` | 3.0.0 | **LGPL-3.0** | 2026-08-10 | 63k / 19k | **Blocked on licence.** Also two majors in a month (2.0.0 and 3.0.0 both 2026-07-17 / 2026-08-10) — a fast-moving API. |
| `whelk-rs` | — | MIT | — | — | **Not published to crates.io.** A git dependency only, which `deny.toml`'s `unknown-git = "deny"` refuses today. Repo alive (pushed 2026-06-29) but 20 stars. |
| `owlish` | 0.28.0 | MIT OR Apache-2.0 | 2023-07-05 | 81k | Permissive OWL 2 model with a Turtle parser. **Three years stale.** |
| `owl-dl-saturation` | 0.3.0 | Apache-2.0 OR MIT | 2026 | 202 | Part of `rustdl`. EL saturation kernel. Very early. |
| `ontologos-*` (9 crates) | 1.1.4 | MIT OR Apache-2.0 | 2026-07-13 | ~3.5k each | EL/RL/RDFS/DL facade. **Treat with suspicion:** repo created 2026-06-11, 0 stars, one author, nine crates to v1.1.4 in seven weeks. Version number is not evidence. |
| `oxirs-shacl` | 0.4.1 | Apache-2.0 | 2026-07-28 | 2.0k | Low adoption. |
| `shacl_validation` | 0.2.12 | MIT OR Apache-2.0 | 2026-04-22 | 44k | The `rudof` project. 20× the adoption of `oxirs-shacl`. |
| `purrdf-shapes` | 0.12.0 | MIT OR Apache-2.0 | 2026-08-02 | 2.8k | New; claims native RDF 1.2 SHACL. Add to the Phase 4 spike's list. |

**Corrections to the assessment above.** The earlier "**there is no OWL 2 DL reasoner in Rust**" is
now too strong as a flat statement and should be read as "none we would depend on": `rustdl`
(Apache-2.0, MaastrichtU-IDS) is actively developed and publishes `owl-dl-saturation`, and
`ontologos-dl` claims DL. Both are far below the adoption and age at which we would make one
load-bearing, so **the practical conclusion is unchanged — EL + RL remains our target** — but the
absolute claim is not one we should print in customer-facing documentation.

The earlier note that `whelk-rs` is a candidate omitted that **it is not on crates.io at all**. That
is a harder constraint than immaturity: `deny.toml` sets `unknown-git = "deny"`, so adopting it
means either a published release upstream or a policy exception. Neither is a spike; it is a
decision.

## Standards — three things moved

**SHACL 1.2 exists and is in active development.** The W3C **Data Shapes Working Group** is
producing four specifications, none yet at Candidate Recommendation:

- **SHACL 1.2 Core** — Working Draft, **2026-08-03**. Adds node expressions (computed targets and
  values), `sh:values` and `sh:defaultValue`, per-constraint severity and message via reification,
  `sh:targetWhere`, `sh:shape` for data-graph nodes declaring their own shapes, and list
  constraints (`sh:memberShape`, `sh:minListLength`). Appendix G is "Changes between the original
  SHACL Core and SHACL 1.2 Core".
- **SHACL 1.2 Rules** — Working Draft.
- **SHACL 1.2 User Interfaces** — First Public Working Draft, **2026-05-26**. Defines a `shui:`
  vocabulary for generating forms and viewers from shapes: widget selection by scoring, grouping and
  ordering, label resolution across languages, 16 built-in editors and 10 viewers, property roles.
- **SHACL 1.2 Profiling** — First Public Working Draft, 2026.

`CLAUDE.md` §2 commits to "SHACL — Core and SPARQL-based constraints", which is SHACL 1.0 (2017) and
remains the only Recommendation. **This is not a reason to chase a Working Draft.** It is a reason
the Phase 4 spike must say which version it is testing against, and a reason to look hard at SHACL
1.2 UI before Phase 3 invents a private form-description vocabulary — TopBraid drives its editing
forms from shapes today, and `shui:` is the standards-track version of that idea. Written up in
`docs/PROPOSED.md`.

**RDF 1.2 reached Candidate Recommendation.** W3C is inviting implementations of *RDF 1.2 Concepts
and Abstract Syntax* and *RDF 1.2 Semantics* (comments were due 2026-05-05). SPARQL 1.2's twelve
documents remain Working Drafts. Oxigraph tracks this closely — 0.5.0 (2025-09-13) replaced RDF-star
with RDF 1.2 and broke its API doing so; 0.5.3 added the SPARQL 1.2 `VERSION` declaration; 0.5.4
added RDF/XML 1.2 draft support **and RDFC 1.0 canonicalization**. Our §2 wording ("RDF 1.2 where
Oxigraph's `rdf-12` feature allows") is still accurate and is now less hedged than it needs to be.

**RDFC 1.0 in Oxigraph 0.5.4 is a Phase 8 gift we had not planned for.** Canonical RDF
serialisation is the difference between a vocabulary diff a reviewer can read and a diff dominated
by blank-node and statement-order churn. Vocabulary-as-code is one of our seven wedge rows, and the
capability is already in a dependency we ship. Proposal filed.

> **Annotated 2026-08-19 (iteration 50): the paragraph below is superseded in two places — the
> revision is at **FDIS**, which is sharper than "expected in 2026", and **ISO 25964-2's revision
> has now started**, so "unchanged" no longer holds. See §"ISO 25964 — one finding sharpened, one
> corrected" below. The rest of the paragraph stands.

**ISO 25964-1 is being revised, with publication expected in 2026.** The revision went out for
comment and vote on 2024-07-30; TC 46's work is reported complete. Announced changes include GUIDs,
a list of connected standards, expanded non-Latin-script examples, DEI guideline references, the
addition of "concept" and "concept term", and substantial annexe updates. ISO 25964-2:2013 was
reviewed and confirmed in 2023 and is unchanged. **We currently cite the 2011 edition** in
`CLAUDE.md` §2, `docs/METHODOLOGY.md`'s `iso-25964-thesaurus` pack, and the Phase 4 rule pack. A
rule pack that claims ISO 25964 conformance against a superseded edition is exactly the
"misrepresents its source methodology" failure `CLAUDE.md` §7 warns about. Proposal filed; the ISO
catalogue page itself returns 403 to automated fetching, so the publication date has **not** been
verified against the registry and that gap is recorded in `docs/UNTESTED.md`.

**ANSI/NISO Z39.19 is unchanged:** Z39.19-2005 (R2010) remains current, and no revision was found
in progress. The `z39-19-taxonomy` pack's citation is accurate.

Sources: <https://www.w3.org/TR/shacl12-core/>, <https://www.w3.org/TR/shacl12-ui/>,
<https://www.w3.org/news/2026/first-public-working-draft-shacl-1-2-profiling/>,
<https://www.w3.org/news/2026/w3c-invites-implementations-of-rdf-1-2-concepts-and-abstract-data-model-and-rdf-1-2-semantics/>,
<https://github.com/oxigraph/oxigraph/blob/main/CHANGELOG.md>,
<https://journals.sagepub.com/doi/10.1177/18758789241299011>,
<https://www.niso.org/standards-committees/iso-25964>,
<https://www.niso.org/publications/ansiniso-z3919-2005-r2010>.

## Competitors — one correction and one gap in our own file

**Correction: "VocBench 3" is our label, not theirs.** The product line has been at
**VocBench 14.0 / Semantic Turkey 14.0 / ShowVoc 5.0** since **2025-03-22**. "VocBench 3" is the
major generation, and using it as a version number makes our competitive file read as three years
out of date. The 14.0 release was a client-technology rewrite; new user-facing features were faceted
dataset search in ShowVoc, an improved Resource View with navigation between lists, trees and
resource views, publicly accessible projects, and a reworked project-users section. Deployment
requirements are unchanged in kind and confirm our wedge: **RDF4J 4.3.15, external stores at 4.0+,
GraphDB tested at 10.6.2, and the GraphDB FTS plugin now shipped separately and deployed by hand
into `/lib/plugins`.** That last detail is our "zero-consultant install" row, written by the
competitor.

**Graphwise:** no change of substance found since the last pass. The merger of Semantic Web Company
and Ontotext is confirmed by both parties' own announcements; PoolParty and metaphactory are both
under it; the combined event is now the Graphwise AI Summit. Nothing found that supersedes the
Gartner Peer Insights weaknesses recorded above — but note those reviews are the *only* practitioner
source in this file, and a single source is thin evidence for the claim we lean on hardest.

**Gap in our own file, recorded rather than fixed:** we have no entry for the *catalog* vendors
(Collibra, Alation, Microsoft Purview, data.world) whose business glossary modules are where a
governance buyer's budget usually already sits. `adr/0003` names them as discovery *connectors*,
which is right, but they compete with us for the same line item and the file does not say so.

Sources: <https://groups.google.com/g/vocbench-user/c/hf57kp_DMBo>,
<https://graphwise.ai/blog/graphwise-merger-swc-ontotext/>,
<https://kmeducationhub.de/graphwise-ai-summit-poolparty-summit-knowledge-graph-forum/>.

## `adr/0003` — discovery sources

**AGROVOC retired its legacy SOAP web services** and now offers exactly two machine routes: the
SPARQL endpoint at `https://agrovoc.fao.org/sparql` and the **Skosmos REST API**. FAO states it does
not plan to add further web services. Nothing found suggesting EuroVoc has changed.

The useful part is the shape, not the fact. **Skosmos is the common front end for a large class of
public SKOS registries** — AGROVOC, Finto, and many national and institutional thesauri all expose
the same REST surface. `adr/0003` lists registries one by one, which implies one connector each.
One `SkosmosProvider` plus a base URL covers the class. Proposal filed.

No connector is implemented yet, so nothing is silently broken today — the failure mode `CLAUDE.md`
§3 warns about (a dead connector reporting "nothing found", which reads as "nothing exists") is
still ahead of us, not behind.

Source: <https://aims.fao.org/node/121113>.

## `adr/0002` — LLM providers

The two-provider decision (`AnthropicProvider` + `OpenAiCompatibleProvider`, `NullProvider` default)
**still holds**: Anthropic publishes no OpenAI-compatible endpoint, so one implementation cannot
cover both, and the OpenAI-compatible surface still covers Azure OpenAI, vLLM, Ollama, LiteLLM and
gateway-fronted Bedrock — including the local models an air-gapped site needs.

What has moved is the Anthropic request surface, and it has moved enough that Phase 10 written from
memory would be wrong. Current model IDs are `claude-opus-5`, `claude-sonnet-5`, `claude-fable-5`,
`claude-haiku-4-5` — undated strings. Notable changes against a 2025 prior: `budget_tokens` is
removed and returns 400 on current models, replaced by `thinking: {type: "adaptive"}` plus
`output_config.effort`; assistant prefill is removed and returns 400; structured output moved to
`output_config.format`, with `output_format` deprecated; and there is a new `stop_reason:
"refusal"` a caller must handle before reading `content`.

Two of these matter beyond keeping up. **Structured outputs are directly load-bearing for
`adr/0002` §4** — a `Proposal` has to be machine-parseable to be reviewable, and constrained
decoding is a better answer than parsing prose. And **zero-data-retention constraints now vary by
model** — at least one current model is unavailable under zero data retention — which is a fact our
per-vocabulary egress policy (`adr/0002` §6) should be able to *state to the user*, not merely
enforce. Neither changes the ADR's decisions; both belong in the notes Phase 10 reads.

The dev shim (`adr/0002` §3) was not exercised this pass. Whether it still matches real provider
semantics is unverified and is recorded in `docs/UNTESTED.md`, not asserted here.

---

# Product-owner pass — 2026-08-19 (iteration 50)

Second scheduled pass, per `CLAUDE.md` §7 and the loop's every-25th-iteration rule.

**Read the interval before reading the findings.** The previous pass was **2026-08-18 — one
calendar day ago.** Twenty-five iterations of this loop cost the world about twenty-four hours, so
almost nothing in the market *can* have moved, and a pass that reported a fresh crop of competitor
news would be manufacturing it. This pass therefore does three things rather than re-running the
survey: it **closes the gap iteration 25 recorded but did not fix**, it **sharpens one finding and
corrects another** where a better source existed, and it says plainly which re-checks came back
unchanged instead of re-narrating them.

That interval is itself the most useful finding for whoever schedules these. The every-25th rule is
indexed to iterations, and iterations are cheap; the things it asks us to review — ISO revisions,
vendor roadmaps, crate maturity — move on a scale of months. Two passes a day apart is the rule
firing on the wrong clock. Recorded as a proposal rather than acted on, because changing the loop's
own cadence is not a change the loop should make for itself.

## Closing the recorded gap: the catalog and glossary vendors

Iteration 25 ended with *"Gap in our own file, recorded rather than fixed: we have no entry for the
catalog vendors (Collibra, Alation, Microsoft Purview, data.world) whose business glossary modules
are where a governance buyer's budget usually already sits."* The corresponding proposal scoped it
as **a research task for a future product-owner pass**. This is that pass; here is the entry.

### Why they belong in this file at all

They are not semantic-web tools and they do not market themselves against PoolParty. They belong
here because of where the money is: a data-governance function evaluating us usually **already owns
one of these**, and the objection our positioning most has to answer is not "why not PoolParty" but
*"we already have a glossary in Collibra."* An entry that only lists the tools we resemble leaves
the loop building against the competitor we do not actually meet in the room.

### Collibra

The governance-orchestration incumbent, and the one whose glossary is most often already bought.

**What its Business Glossary actually is, from Collibra's own current product documentation:** four
core asset types — **Business Term**, **Acronym**, **Measure**, and **KPI** — organised through
**Domains** (those whose domain type is `Glossary`) and **Communities**, with relations that run
from a business term to a data attribute and from that attribute to a column.

**The load-bearing observation is what the page does not contain.** That documentation page
**mentions no W3C standard at all** — not SKOS, not RDF, not OWL, not SHACL. This matters because
secondary comparison sites assert the opposite. A search summary consulted during this pass stated
Collibra "may support standards such as SKOS, RDF, OWL, and SHACL"; that claim traces to aggregator
listicles, not to Collibra, and this file's first rule is that vendor claims need a vendor source.
**We should not repeat it, and we should not build a connector on it.**

So the honest characterisation is: a proprietary asset-type model, bound to physical data assets,
with governance workflow around it. There is no concept scheme, no `broader`/`narrower` semantics,
no mapping relations, and no evidence of a standards-native export. That is a different artefact
from a thesaurus — and *the difference is our argument*, provided we state it as a difference in
purpose rather than as a deficiency, because on its own ground the model is well-matched to it.

### Alation, Microsoft Purview, data.world

Recorded more thinly and deliberately so, because this pass verified less about them.

- **Alation** — positions on adoption and stewardship: search UI, active catalog, out-of-the-box
  stewardship workflows. Its centre of gravity is analyst behaviour and self-service BI, not
  vocabulary structure.
- **Microsoft Purview** — ties glossary alignment to governed data assets and security controls.
  The reported failure mode is fragmentation of taxonomy modelling across domains when access
  controls are not aligned. Fastest to first value of the three (reported 2–4 weeks, against
  Collibra's 3–6 months) — which is a **caution for us**, not a comfort: "one binary, no
  consultants" competes with "already in the tenant", and the second is a very short install too.
- **data.world** — the one whose model may genuinely overlap ours. A secondary source states its
  catalog knowledge graph is built from **DCAT** (catalog), **Dublin Core** (metadata), **SKOS**
  (glossaries and thesauri) and **PROV** (provenance and lineage) — which is, almost exactly,
  `CLAUDE.md` §2's own standards surface. **This is flagged, not asserted.** Two attempts to
  retrieve the primary post (Juan Sequeda, data.world Principal Scientist, 2022-02-04) returned the
  page furniture without the article body, so the composition claim is **unverified at source** and
  is recorded in `docs/UNTESTED.md`. If it holds, data.world is the catalog vendor least available
  to us as a mere discovery source and most available as a competitor telling the same standards
  story — and that is worth an hour of somebody's verified reading before Phase 12 designs a
  connector around an assumption.

### What this changes about our positioning

It does not change the plan, and no item moved. It changes what the product has to prove, in one
direction worth writing down: against PoolParty we argue deployment weight and price, but against
an incumbent catalog we argue that **a glossary of business terms bound to columns is not a
vocabulary** — no scheme, no hierarchy semantics, no mappings, no integrity conditions — and that
the two should be **connected rather than one replacing the other**. That is the `adr/0003` posture
already, and it is a stronger commercial position than displacement, because it does not ask the
buyer to write off the system they already run.

Sources: <https://productresources.collibra.com/docs/collibra/latest/Content/BusinessGlossary/to_business-glossary.htm>
(fetched 2026-08-19), <https://promethium.ai/guides/data-governance-tools-comparison-collibra-alation-atlan-purview/>,
<https://data.world/blog/3-ways-to-confirm-your-data-catalog-is-really-powered-by-a-knowledge-graph/>
(body not retrievable 2026-08-19).

## ISO 25964 — one finding sharpened, one corrected

Iteration 25 reported that ISO 25964-1's revision was expected to publish in 2026 and could not
verify it, because the ISO catalogue returns 403 to automated fetching. **It still does**, verified
again this pass. But the catalogue entry is visible through search metadata, and it carries more
than a date.

**Sharpened: the revision is at FDIS, and the title has changed.** The entry is
**`ISO/FDIS 25964-1`** — Final Draft International Standard, the approval stage, the last one before
publication — for **Edition 2**, titled *"Information and documentation — Thesauri and
interoperability with other vocabularies — Part 1: Thesauri for information retrieval, **management
and use**"*, explicitly replacing ISO 25964-1:2011. The 2011 Part 1 is titled *"Thesauri for
information retrieval"*; the added **"management and use"** is a scope change on the face of the
title, in a standard whose §Guidelines-for-thesaurus-management-software clauses are exactly what a
vocabulary tool gets measured against.

"At FDIS" is materially more actionable than "expected in 2026": FDIS text is essentially final, and
the remaining stages are ballot and publication. The deadline iteration 25 noted as "attached to
somebody else's calendar" is nearer than it read.

**Corrected: Part 2 is no longer static.** Iteration 25 recorded *"ISO 25964-2:2013 was reviewed and
confirmed in 2023 and is unchanged."* The confirmation is right; the implication is now stale.
**Revision work on Part 2 has recently started** (Taxonomy Strategies, presentation dated
2026-04-15). Part 2 is the *mapping between vocabularies* standard — thesauri to classification
schemes, taxonomies, subject headings, ontologies, name authority lists — which this file already
identifies as "exactly the 'centralise and govern many vocabularies' problem our buyer has", and
which our mapping features implement against. A revision starting there is worth watching, though
starting is early enough that nothing is actionable yet.

**Still unverified, and stated as such:** the exact ISO stage code (50.00 vs 50.20 vs 60.00) and any
publication date. Both sit behind the 403. The FDIS designation comes from the catalogue entry's own
title as surfaced in search metadata, not from the page. `docs/UNTESTED.md` carries the gap.

No repository text changed as a result. Our citations still say ISO 25964-1:2011, which remains the
published edition and therefore remains **correct** — the open proposal to make every citation name
the edition explicitly is unaffected and is now better motivated, since an Edition 2 with a different
title is precisely the situation in which a bare "ISO 25964" citation becomes ambiguous.

Sources: <https://www.iso.org/standard/86713.html> (403 to automated fetch; title and scope read
from search metadata, 2026-08-19), <https://taxonomystrategies.com/thesaurus-standards-for-taxonomies/>
(presentation dated 2026-04-15).

## Re-checked and unchanged — stated briefly, on purpose

Each of these was checked this pass and found where iteration 25 left it. They are one line each
because one line is all a one-day interval earns.

- **Oxigraph** — still `0.5.9`, last published 2026-06-18, read from the crates.io API. We remain
  locked at the version iteration 25 measured. No action.
- **`horned-owl` / LGPL-3.0** — unchanged, and still the right call. `BLOCKED.md`'s entry stands
  untouched; nothing in the tree depends on it and Phase 9 is still six phases out.
- **ANSI/NISO Z39.19** — no revision found in progress; Z39.19-2005 (R2010) remains current, so the
  `z39-19-taxonomy` pack's citation is still accurate.
- **`adr/0002` (LLM providers)** — no re-verification attempted this pass. Iteration 25's notes on
  the Anthropic request surface are one day old and there is no honest way to improve on them in
  that time. The dev shim remains unexercised, which `docs/UNTESTED.md` already records.
- **`adr/0003` (discovery sources)** — no connector is implemented, so there is still nothing that
  can be silently broken. The AGROVOC/Skosmos finding and its proposal stand.
- **Methodology packs** (`docs/METHODOLOGY.md`) — no cited methodology found revised beyond the
  ISO 25964 movement recorded above.

## Charter-drift audit

Folded in because iteration 50 is also a blind-spot boundary. Mechanical checks, not impressions.

- **No new dependency and no new required external service.** The workspace dependency list is
  unchanged, and Oxigraph is still built `default-features = false` with only `rocksdb`, which keeps
  the `http-client` feature family out of the tree — the thing that makes `CLAUDE.md` §1.1's
  air-gapped commitment true by construction rather than by intent.
- **No forbidden licence.** `cargo deny check licenses` passes, `rc=0`.
- **`unwrap()`/`expect()` outside tests and startup** (§6): a raw grep reports 975 hits, which is
  the wrong number — it counts inline `#[cfg(test)]` modules. Parsing test modules out leaves
  **seven**, all in `crates/openbiz-store/src/scale.rs`, which is the scale harness. That harness
  having no scheduled runner is already an open proposal; its `expect()`s are test-shaped code in a
  non-test file, which is a known and recorded consequence of that, not new drift.
- **Nothing found that requires an LLM**, and `NullProvider` remains the default.

The audit found no drift. Per the loop's own rule that a blind-spot pass finding nothing must say so
rather than report a comfortable green: **this one found nothing, and the checks above are what it
ran** — a reader can judge whether they were the right checks, which is the point of listing them.
