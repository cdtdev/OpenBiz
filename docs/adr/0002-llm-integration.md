# ADR 0002 — LLM and agent integration

**Status:** accepted (2026-08-18) · **Phase:** 10

## Context

LLM assistance is genuinely valuable for knowledge organisation: consolidating unstructured notes
into candidate concepts, extracting terms from a corpus, drafting definitions, spotting near
synonyms, proposing mappings between vocabularies, drafting translations, generating competency
questions.

It also collides with two of our non-negotiables. `CLAUDE.md` §1 requires air-gapped operation and
forbids required external services. And our buyer is a governance function in a regulated industry,
for whom **sending vocabulary content to a third-party API is a data-egress event**, not a feature.

## Decision

**1. LLM features are strictly optional. The default provider is none.** Nothing in the core may
require an LLM. Every LLM-assisted path has a working manual path. Air-gapped deployments lose
assistance and nothing else. A feature that cannot degrade this way does not ship.

**2. Providers sit behind an `LlmProvider` trait we own** — the same discipline as `Reasoner` and
`Validator` (`CLAUDE.md` §3). Shipped implementations:
- `AnthropicProvider` — Anthropic Messages API.
- `OpenAiCompatibleProvider` — covers Azure OpenAI, vLLM, Ollama, LiteLLM, and gateway-fronted
  Bedrock. One implementation, most of the market, including **local models for air-gapped sites**.
- `NullProvider` — the default. Reports no capabilities; callers degrade rather than fail.

**3. Development uses the Claude CLI behind an OpenAI-compatible shim.** A dev-only binary
(`openbiz-llm-shim`) serves `/v1/chat/completions` and executes `claude -p` underneath. Development
and production therefore exercise **the same code path** through `OpenAiCompatibleProvider`; the
only difference is a base URL. The shim is never shipped in the product binary and is excluded from
release builds.

**4. Agents produce proposals, never writes.** An agent run emits a `Proposal` — a set of suggested
changes a human reviews, edits, and approves through the ordinary governance workflow (Phase 6).
There is no path from model output to committed vocabulary that skips a human. This is not
timidity: it is the only design compatible with governance-as-substrate, and it is the honest
answer to "AI-assisted" competitors whose provenance for a definition is nobody knows.

**5. Every proposal carries full provenance in PROV-O** — model, version, prompt template version,
timestamp, requesting user, inputs, and cited sources. A definition's origin must be answerable
years later.

**6. Every call is an audited egress event.** Per-vocabulary LLM policy (off · local only · named
external provider), an audit log of what was sent where and by whom, and the ability to forbid
egress entirely for a classified vocabulary. Surfaced in the UI before the first call, not buried
in settings.

**7. Prompt templates are versioned artifacts in git**, not string literals, and each agent has a
golden evaluation set so output quality is measurable and regressions are visible.

## Consequences

- New crate `openbiz-llm` (trait, providers, agents, proposal model) and dev-only
  `openbiz-llm-shim`.
- Depends on Phase 6 for the review workflow proposals flow into. Benefits from Phase 7 — an agent
  that knows the current lifecycle phase can suggest what that phase actually needs.
- Adds an outbound network dependency **when enabled**, which must never become load-bearing.
- **Risk:** LLM assistance quietly becoming required, e.g. a UI where the manual path rots into
  something unusable. The degradation-watch check in `/openbiz-status` should treat "core path only
  reachable via an agent" as charter drift.
- **Risk:** the shim diverging from real OpenAI semantics and hiding bugs until production. Keep it
  thin, and run the agent evaluation sets against a real provider before any release claim.
