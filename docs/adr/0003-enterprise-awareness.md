# ADR 0003 — Enterprise awareness and the anti-silo posture

**Status:** accepted (2026-08-18) · **Phase:** 12

## Context

The failure mode this product exists to prevent is **the enterprise that owns nine overlapping
taxonomies and cannot tell**. Finance maintains a customer classification in a spreadsheet, the data
team has one in their catalog, marketing has a SharePoint term store, and a public standard already
covers most of it.

A vocabulary tool that makes it easy to create a new vocabulary and hard to find an existing one is
a **silo generator**. Every incumbent is one, because a new-vocabulary wizard is easy to build and
cross-enterprise discovery is not.

## Decision

**1. Discovery precedes creation, structurally.** Starting a vocabulary, and adding a concept to
one, both trigger a search across every configured source first. Discovery is not a feature the user
must remember to invoke; it is on the creation path.

**2. Sources sit behind a `DiscoveryProvider` trait we own.** Implementations: the local store;
federated OpenBiz peers; arbitrary SPARQL endpoints; public vocabulary registries (EuroVoc,
AGROVOC, LCSH, SNOMED CT, schema.org, IPTC and similar); and enterprise connectors — data catalogs
(Collibra, Alation, Microsoft Purview, DataHub, OpenMetadata, Unity Catalog), the SharePoint
managed-metadata term store, and Confluence or wiki glossaries.

**3. The reuse ladder. "Create new" is the last rung and requires a recorded justification.**
1. **Use** the existing concept directly.
2. **Map** to it (`skos:exactMatch` / `closeMatch` / `broadMatch`).
3. **Extend** it — import and specialise.
4. **Fork** it, with a recorded reason.
5. **Create new**, with a recorded reason naming what was found and why nothing fitted.

The justification is the mechanism. Not a warning dialog — those get clicked through — but an
auditable record that makes proliferation visible to the people accountable for it.

**4. Reuse must be less work than recreating.** If mapping to an existing concept takes more clicks
than typing a new one, the ladder is decoration. This is a usability requirement with teeth, and it
is the item to test in usability sessions.

**5. Register vocabularies OpenBiz does not manage.** An enterprise vocabulary registry that catalogs
every KOS in the organisation — including the spreadsheets and term stores nobody governs. **You
cannot de-silo what you cannot see**, and registration is a far lower bar than migration. This is
also the wedge into an account: catalog the mess first, migrate later.

**6. Overlap detection is a standing report, not a one-off scan.** Cross-vocabulary duplicate and
near-duplicate detection with a consolidation workflow. LLM assistance improves recall here
(`adr/0002`) but must not be required — lexical and structural matching carries the baseline.

**7. Federation degrades to nothing.** Air-gapped deployments lose external and public sources and
keep local and peer discovery. No discovery source may be load-bearing.

## Consequences

- New crate `openbiz-discovery`: trait, providers, connectors, matching, registry, overlap reports.
- Depends on Phase 2 (mapping properties) and Phase 11 (import machinery for connectors). The
  *hook* — the trait plus a local-only implementation — lands earlier so the creation path is built
  around discovery from the start rather than retrofitted.
- Each connector is an integration with its own auth, rate limits, and breakage. Connectors are
  individually optional and individually tested; a broken connector must degrade to "source
  unavailable", never block creation.
- **Risk:** discovery latency on the creation path making the product feel slow. Search
  asynchronously and never block typing.
- **Risk:** justification prompts becoming click-through noise. If telemetry or usability testing
  shows they are ignored, the mechanism has failed and needs redesign, not louder wording.
