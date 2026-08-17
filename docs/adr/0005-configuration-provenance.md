# ADR 0005 — Layered configuration, and every setting carries its provenance

**Status:** accepted (2026-08-18) · **Phase:** 0

## Context

Phase 0 called for "config from a file as well as the environment". Until this iteration `Config`
read two `OPENBIZ_*` variables with `unwrap_or` and nothing else.

The standing product-owner direction of 2026-08-18 (`FEEDBACK-LOG.md`) says parity is failure: for
every item, ask what the incumbents do badly and what would be materially better. Configuration is
an unglamorous place to ask that, but the honest answer is specific and actionable.

What the incumbents do badly here is not the *format*. It is that a deployment's effective
configuration is unknowable. PoolParty, TopBraid EDG, metaphactory, and VocBench are each spread
across an application-server descriptor, one or more `.properties` files, and a triplestore
connection configuration, layered by rules that are documented in prose if at all. Two consequences
follow, and both are routinely reported by people standing them up:

- **A key you misspell is silently ignored.** The operator is then certain they configured
  something they did not, and the symptom shows up somewhere unrelated.
- **When a value is not what you expected, there is no way to ask why.** You go and read every
  layer by hand and reason about precedence.

Matching them means shipping a config file. Beating them means making the effective configuration
*self-explaining* — which is the same commitment `CLAUDE.md` §3 makes about inference, applied at a
much smaller scale.

## Decision

**1. Three layers, documented precedence: default → file → environment.** The environment wins
because it is the layer a container runtime or systemd unit can reach without editing a file. A
missing file at the *default* path is normal and silent; a missing file that `OPENBIZ_CONFIG`
explicitly named is a hard error, because an explicit request must never degrade to the defaults.

**2. TOML.** Over JSON because a deployment's configuration file is where operators leave notes for
the next operator, and JSON has no comments. Over YAML because significant whitespace is a footgun
in a file hand-edited under pressure, and because the YAML ecosystem's implicit typing ("no" as a
boolean) is a class of bug we would rather not import. `toml` v1.1.4 is MIT OR Apache-2.0; its
tree (`toml_parser`, `toml_datetime`, `toml_writer`, `serde_spanned`, `winnow`) is uniformly
MIT/Apache-2.0, and `cargo deny check licenses bans sources` passes **unchanged** — no widening of
the `CLAUDE.md` §5 allow list, so this ADR records no licence exception. It is a format parser at
the process boundary, not an engine, so the §3 "own the trait" rule does not apply; the seam that
matters is `Config::resolve`, which takes its file path and environment lookup as parameters.

**3. `deny_unknown_fields`.** An unrecognised key fails the load, with TOML's span pointing at the
line and serde naming the keys we accept. This is the single highest-value line in the change.

**4. Every value is a `Setting<T>` carrying a `Source`** — `Default`, `File(path)`, or
`Env(name)`. `Deref` and `Display` forward to the value, so provenance costs call sites nothing.
The composition root logs one line per setting at startup, before it binds, and the bind failure
message names the source rather than only the address:

```
Error: failed to bind 0.0.0.0:80, from /etc/openbiz/openbiz.toml
```

**5. A blank value is an error, not an absence.** `OPENBIZ_BIND=` and `bind = ""` both fail,
naming the source. An unset shell variable, an empty `docker compose` interpolation, and a systemd
`Environment=` line with nothing behind it all collapse to empty; treating empty as "unset" would
reintroduce the silent-ignore failure that decision 3 exists to prevent.

## What was measured

- Test count 28 → 45; 16 of the new tests are in `crates/openbiz-server/src/config.rs`.
- The unknown-key test was **verified to fail** with `deny_unknown_fields` removed, rather than
  assumed to be catching something. A test that has never been red is not evidence.
- `cargo deny check licenses bans sources`: `bans ok, licenses ok, sources ok`. The two `winnow`
  major versions in the tree are a `multiple-versions = "warn"` duplicate, not a policy failure.
- No new required external service; startup remains a single binary with no file present.

## Consequences

- Settings added in later phases must be added in three places — the `Config` struct, `FileConfig`,
  and the environment merge — and any that is forgotten fails a test rather than silently
  disappearing, because `Config::settings()` is what the startup log and its test both iterate.
- `Config::resolve` takes an injected environment lookup so the merge is tested without mutating
  process-global state, which the test binary shares across threads. `Config::load` is the only
  place that touches the real environment.
- Provenance is currently only reachable through the startup log and error messages. An
  administrative UI that shows the effective configuration and where each value came from is the
  natural home for it, and belongs with the Phase 14 admin console rather than here — recorded in
  `PROPOSED.md`, not built.

## Alternatives rejected

- **`figment` or `config-rs`.** Both do layered configuration well, and `figment` even tracks
  metadata about which provider supplied a value. Rejected on `CLAUDE.md` §1.5: they bring a
  multi-format dependency tree to serve two settings, and the merge we need is fifteen lines. The
  decision is reversible — `Config::resolve` is the only thing that would change — and should be
  revisited if the settings count grows past roughly a dozen or profiles/`includes` are wanted.
- **Environment-only, no file.** Rejected: the plan item asks for a file, and air-gapped operators
  hand-editing a unit file for every setting is a worse experience than one commented TOML file.
- **File wins over environment.** Rejected: it makes a container image's environment unable to
  override a file baked into it, which is the common deployment shape.
