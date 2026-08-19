# 0036 — The minting pattern is a recorded decision, kept in our graph and not in the vocabulary

- **Status:** accepted
- **Date:** 2026-08-19 (iteration 41)
- **Item:** Phase 2 — "Concept IRI minting, part 2 — the policy persisted per vocabulary, so every
  producer mints the same way"

## Context

`adr/0035` gave this build a minter whose default pattern is **read off the vocabulary's own
concepts**: the namespace most of them are already in, and whether their local names are numbered or
worded. That is a real improvement on every incumbent, all of which make you configure a URI pattern
against nothing at all. It is also, as that ADR's own closing section and iteration 39's loop entry
both said, a good *suggestion* and a poor *policy*.

The reason is in what inference answers. It answers "what do most of these concepts look like
**now**", so the answer is a function of the vocabulary's current contents. A vocabulary whose first
two hundred concepts are in one namespace and whose next hundred arrive in another passes a tipping
point, after which every mint disagrees with every mint before it. Nothing announces the crossing,
and each IRI was permanent the moment it was used. Worse, the drift is invisible in exactly the
deployment this product is for: an import, a discovery match, and — from Phase 10 — an agent
proposal all have to mint the same way as the curator, and a default computed independently by each
of them at different moments is not one policy, it is several that happen to agree today.

So the question this ADR settles is not "should the pattern be configurable" — it always was, with
`--pattern`. It is **where a decision about a vocabulary lives, who it is attributed to, and what
happens when it disagrees with the vocabulary's own history.**

## Decisions

### 1. Three sources, in a fixed order, and the report always says which one was used

`openbiz mint` takes the first that exists:

1. `--pattern`, for that one command.
2. **The pattern recorded for the vocabulary.**
3. The convention inferred from the vocabulary's own concepts.

The order is the only defensible one — an explicit argument beats a written decision beats a reading
of the data — but the *reporting* is the part that took work. Each of the three gets a different
paragraph, because they are different kinds of fact and a reader who cannot tell them apart cannot
tell a deployment that has a policy from one that has a coincidence. In particular, the inferred
case now says out loud that its answer moves as the vocabulary grows, and names the command that
stops it. A weakness we know about and do not mention is a weakness we are hiding.

### 2. A recorded pattern this build cannot parse is refused, not fallen back from

If a vocabulary records a pattern `MintPattern::parse` rejects, `openbiz mint` fails and names the
pattern, who recorded it, and why it could not be read.

The tempting alternative — fall back to inference — is wrong in a way worth stating. The vocabulary
has a *written decision* about how its identifiers are named. Minting under something else because
we could not read that decision produces IRIs in a namespace nobody chose, which look exactly as
official as the real ones and are permanent before anybody investigates. Refusing costs one command;
falling back costs the vocabulary. `openbiz policy <graph>` shows the unusable text and the parse
error together, so the operator sent there by the refusal can see both at once.

### 3. The record lives in the system graph, on the vocabulary's registry subject

Three statements — the pattern, who recorded it, when — hanging off the vocabulary's own subject in
`urn:openbiz:graph:system`, beside the `urn:openbiz:graphKind` entry that is the other thing OpenBiz
records *about* a vocabulary rather than *in* it.

The alternative was a statement in the vocabulary itself, on its `skos:ConceptScheme`. Rejected:
that publishes it. An export of the vocabulary to another tool would carry an OpenBiz configuration
statement that no standard defines, which the receiving tool either drops or preserves as noise, and
`CLAUDE.md` §1.3's round-trip requirement is about *standards* content surviving, not about our
settings hitching a ride. `adr/0007` already settled the general form of this — our metadata was
never in the graph — and this is the same rule applied to a new fact.

The consequence, which the module documentation and `UNTESTED.md` both state: a **whole-store backup
carries the policy** (proven, `tests/iri_policy.rs`) and a **single-vocabulary export does not**.
That is the correct trade and it is not free, and somebody moving a vocabulary between two OpenBiz
deployments by export rather than backup will arrive with no policy and an inferred default.

### 4. Recording is attributed, and is not a candidate

It is refused without a name to record, by the same rule and the same resolution order as
`openbiz approve` (`OPENBIZ_ACTOR`, then `USER`, then `LOGNAME`). The pattern a vocabulary mints
under is a governance decision, and an unattributed decision is not one.

It does **not** go through the candidate seam. `CLAUDE.md` §3 requires that of a change to a
*vocabulary*, and this changes no statement in one: no concept is touched, and no IRI already minted
is affected, because a policy governs the next mint and an IRI that changes is a different concept.
What it changes is OpenBiz's own record about the vocabulary, which is the same category as the
registry entry created when the vocabulary was made — also a direct write. Both reports say the
"nothing already minted changed" part explicitly, because "you have just changed how this vocabulary
names things" is a sentence a reader can easily hear as "you have just renamed things".

### 5. A disagreement with the vocabulary's own concepts is reported and never refused

Recording `…/{slug}` for a vocabulary whose two hundred concepts are all `…/c_{n}` is legitimate: it
is precisely how a convention gets changed on purpose, and refusing it would make the command
useless for the case it is most needed in. It is also exactly how somebody starts minting into the
wrong namespace and does not find out for a year. Only the operator can tell those apart, so both
`openbiz policy` and `openbiz mint` print what the concepts suggest and whether it matches, and
neither refuses.

The two readings need **different sentences**, which was found by reading the command's own output
rather than by reasoning. A pattern *chosen now* — given with `--pattern`, or being recorded — gets
"minting under a different pattern is legitimate and it is also how a concept ends up in the wrong
namespace". A pattern *already recorded* and merely being reported gets "its recorded policy and the
IRIs it already holds disagree: either the policy was recorded to change the convention, or it names
a namespace nobody meant". The first version reused the former for both, which told a reader looking
at a stored fact that they were taking a risky action they were not taking.

Relatedly, `mint --pattern` over a recorded policy now names the record it is stepping over, its
author, and the fact that the record is unchanged. Before that it read identically to an override of
a vocabulary that had recorded nothing — which is the case where nothing is being contradicted.

### 6. Replacing a policy overwrites it, and the report is the only notice

Recording a second pattern removes the first and hands it back so the command can print it. There is
no history of policy changes.

This is a deliberate, recorded limitation rather than a decision we are comfortable with. A
governance product should be able to answer "what policy was in force in March", and today the
honest answer is "look at the IRIs minted in March" — the vocabulary's own contents are the record.
Keeping a versioned policy history is a small feature and a real one (it wants an ordering, a
retention answer, and a place in the audit trail `PROV-O` will eventually shape), and inventing it
inside this item would have been scope this ADR could not justify. It is in `UNTESTED.md`.

## What was measured

Nothing that needs a number. This item is a persistence and precedence decision, not a performance
one: reading a policy is a pattern scan of the system graph narrowed to one subject and one
predicate, the same shape as the registry read `UNTESTED.md` already records as bounded by the number
of graphs rather than by content.

What was *verified* is the claim the item exists for, and it needs separate processes to be a claim
at all: `tests/iri_policy.rs` records a pattern in one invocation of the real binary and reads it
back from a later, separate invocation of `openbiz mint`, with the recorded pattern deliberately
disagreeing with what the vocabulary's concepts suggest — so an implementation that ignored the
record would mint `c_4` and the test would say so. It also proves the record survives a backup and
restore, that a refused pattern is never written, and that an unattributed recording is refused.

## Consequences

- A deployment can now be *configured* rather than merely consistent by luck, and the configuration
  is one fact in one place with a name and a timestamp on it.
- "Every producer mints under it" has exactly one producer today, because nothing else in this build
  mints. The value of this item is almost entirely in front of it — the import path, discovery, and
  agent proposals — which is the shape `CLAUDE.md` §3 asks for and is also why the claim is written
  down in `UNTESTED.md` rather than left implied.
- `openbiz policy` is command-line only. The API and interface half arrives with the rest of Phase
  3, on the same basis every other Phase 2 item was closed.
- The system graph now holds a fact that is neither the registry nor a candidate, which is the first
  of what will be several: a per-vocabulary *setting*. It was deliberately not generalised into a
  settings map — one named thing that a reader can find is worth more than a framework for facts
  that do not exist yet — but the second such setting should look hard at whether it wants to.
