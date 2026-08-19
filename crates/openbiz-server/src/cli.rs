//! The command line: what `openbiz` does when it is not serving.
//!
//! # Why backup and restore are commands and not endpoints
//!
//! Every other capability in this build is reachable over HTTP, and these two deliberately are
//! not. Three reasons, in order of how much they bind.
//!
//! 1. **There is no authentication yet.** A `POST /api/restore` would be an unauthenticated way to
//!    replace an entire store's contents, and `GET /api/backup` an unauthenticated way to take a
//!    copy of the whole customer's data including our own metadata. That is not a partial feature;
//!    it is the same objection that has SPARQL Update deferred (see `docs/BUILD-PLAN.md`).
//! 2. **A restore needs the store to itself.** It refuses a store that already holds anything, so
//!    it is by construction an operation on a *stopped* deployment — and the embedded store's
//!    exclusive lock enforces that rather than trusting an operator to remember it.
//! 3. **This is what a backup script needs anyway.** Backups run from cron, from a systemd timer,
//!    from a container's pre-stop hook. A command with an exit status is what those understand;
//!    an HTTP endpoint needs a credential and a client before it is useful to any of them.
//!
//! An *online* backup over HTTP, taken while the server runs, is a real capability and it is
//! proposed rather than assumed (`docs/PROPOSED.md`).
//!
//! # Why import and review are commands too
//!
//! The same three reasons, and the second one differently. `openbiz import` and `openbiz retract`
//! propose a change and `openbiz approve` applies one, so between them they can write to a
//! customer's vocabulary —
//! which is precisely why they are not endpoints yet. There is still no authentication, and an
//! unauthenticated "apply this change to a vocabulary" is the objection that has SPARQL Update
//! deferred. What is different from backup and restore is that these do **not** need the store to
//! themselves in principle; they need it today only because the embedded store takes an exclusive
//! lock. Putting the candidate seam behind HTTP is the next slice, and it lands with the
//! authentication, not before it.
//!
//! # Why the parser is hand-written
//!
//! Four forms, no flags, no subcommand tree. `clap` would be a dependency and a build-time cost
//! for less code than the argument table below, and `CLAUDE.md` §1.5 makes every dependency
//! something to justify rather than reach for. If the surface grows past what fits on one screen
//! here, that judgement should be revisited — it is a size judgement, not a principle.

use std::ffi::OsString;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use openbiz_skos::{
    LabelKind, LabelQuery, LanguageFilter, LanguageRange, MatchMode, NoConvention, PatternError,
    SearchBound,
};
use openbiz_store::{
    Candidate, CandidateId, CandidateIdError, CandidatePart, CandidateSource, CandidateState,
    Decision, GraphId, GraphIdError, Provenance, RdfSyntax, Store, StoreError, BACKUP_SYNTAX,
};
use thiserror::Error;

/// What the operator asked `openbiz` to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Run the server. What happens with no arguments at all, which is the common case.
    Serve,
    /// Write a backup of the configured store to a file.
    Backup {
        /// Where to write it. Must not already exist.
        file: PathBuf,
    },
    /// Reconstruct the configured store from a backup file.
    Restore {
        /// The backup to read.
        file: PathBuf,
    },
    /// Propose the statements in a file as a change to a vocabulary. Nothing is written to it.
    Import {
        /// The IRI of the vocabulary graph the change is proposed against.
        graph: String,
        /// The RDF file to read.
        file: PathBuf,
    },
    /// Propose the statements in a file as a removal from a vocabulary. Nothing is written to it.
    Retract {
        /// The IRI of the vocabulary graph the removal is proposed against.
        graph: String,
        /// The RDF file naming the statements to remove.
        file: PathBuf,
    },
    /// Report what a vocabulary holds, in SKOS terms. Reads and nothing else.
    Inspect {
        /// The IRI of the vocabulary graph to read.
        graph: String,
    },
    /// Report which SKOS integrity conditions the vocabulary satisfies. Reads and nothing else.
    Integrity {
        /// The IRI of the vocabulary graph to read.
        graph: String,
    },
    /// Report what is above one concept in the hierarchy, and why. Reads and nothing else.
    Ancestors {
        /// The IRI of the vocabulary graph to read.
        graph: String,
        /// The IRI of the concept to walk up from.
        concept: String,
    },
    /// Report every route from one concept up to a root, and why. Reads and nothing else.
    Paths {
        /// The IRI of the vocabulary graph to read.
        graph: String,
        /// The IRI of the concept to enumerate the routes above.
        concept: String,
    },
    /// Report what is below one concept and beside it, and why. Reads and nothing else.
    Tree {
        /// The IRI of the vocabulary graph to read.
        graph: String,
        /// The IRI of the concept to walk down from.
        concept: String,
    },
    /// Find the concepts a vocabulary labels with some text. Reads and nothing else.
    Search {
        /// The IRI of the vocabulary graph to read.
        graph: String,
        /// What to look for, and how.
        query: Box<LabelQuery>,
    },
    /// Report the IRI a new concept in a vocabulary would be given. Reads and nothing else.
    Mint {
        /// The IRI of the vocabulary graph the concept would go in.
        graph: String,
        /// What the new concept would be called. Required by a `{slug}` pattern.
        label: Option<String>,
        /// The pattern to mint under, overriding the one the vocabulary suggests.
        pattern: Option<String>,
    },
    /// Show, or record, the IRI-minting pattern a vocabulary's new concepts are given.
    Policy {
        /// The IRI of the vocabulary the policy belongs to.
        graph: String,
        /// The pattern to record. Without it, this shows what is recorded and writes nothing.
        pattern: Option<String>,
    },
    /// Report what a vocabulary documents one resource with, and why. Reads and nothing else.
    Notes {
        /// The IRI of the vocabulary graph to read.
        graph: String,
        /// The IRI of the resource to report the documentation of.
        resource: String,
    },
    /// Report what a vocabulary joins one resource to, and why. Reads and nothing else.
    Mappings {
        /// The IRI of the vocabulary graph to read.
        graph: String,
        /// The IRI of the resource to report the mapping links of.
        resource: String,
    },
    /// List every proposed change the store holds.
    Candidates,
    /// Show one proposed change, with the statements it would add.
    Show {
        /// The candidate's identifier.
        id: String,
    },
    /// Apply a proposed change to its target vocabulary.
    Approve {
        /// The candidate's identifier.
        id: String,
    },
    /// Refuse a proposed change. Its statements stay staged and the vocabulary is untouched.
    Reject {
        /// The candidate's identifier.
        id: String,
    },
    /// Print [`USAGE`] and exit successfully.
    Help,
}

/// What to tell an operator who got the arguments wrong — and what `openbiz help` prints.
pub const USAGE: &str = "\
openbiz — self-hosted taxonomy, ontology, and thesaurus management

Usage:
  openbiz                    start the server
  openbiz backup <file>      write a backup of the store to <file>
  openbiz restore <file>     rebuild an empty store from a backup
  openbiz import <graph> <file>
                             propose the file's statements as additions to <graph>
  openbiz retract <graph> <file>
                             propose the file's statements as removals from <graph>
  openbiz inspect <graph>    report what <graph> holds in SKOS terms, and why
  openbiz integrity <graph>  report which SKOS integrity conditions <graph> satisfies
  openbiz ancestors <graph> <concept>
                             report what is above <concept> in the hierarchy, and why
  openbiz paths <graph> <concept>
                             report every route from <concept> up to a root, and why
  openbiz tree <graph> <concept>
                             report what is below <concept> and beside it, and why
  openbiz search <graph> <text> [options]
                             find the concepts <graph> labels with <text>
  openbiz mint <graph> [<label>] [--pattern <p>]
                             report the IRI a new concept in <graph> would be given
  openbiz policy <graph> [--pattern <p>]
                             show, or record, the pattern <graph> mints new IRIs under
  openbiz notes <graph> <resource>
                             print what <graph> documents <resource> with, and why
  openbiz mappings <graph> <resource>
                             print what <graph> joins <resource> to, and why
  openbiz candidates         list the proposed changes waiting for a decision
  openbiz candidate <id>     show one proposed change and the statements it would add
  openbiz approve <id>       apply a proposed change to its vocabulary
  openbiz reject <id>        refuse a proposed change
  openbiz help               show this

A backup is the whole store as N-Quads: every vocabulary and OpenBiz's own registry, in a
W3C-standard syntax any conforming tool can read. Restore refuses a store that is not empty, so
restore into a fresh data directory and point the server at that.

Neither import nor retract writes to a vocabulary. Each reads the file — the syntax is taken from
the file extension — stages the statements where you can read them, and records who proposed what
and why. `openbiz approve` is what applies them, and it records who approved them. Approving and
rejecting need a name to record: OPENBIZ_ACTOR if it is set, otherwise USER or LOGNAME.

Ancestors only reads. It walks `skos:broaderTransitive` up from one concept and prints every
concept above it with the path that reached it, which for a link nobody stated is the derivation
S24 licensed. The closure is never stored — a legal SKOS hierarchy can be arbitrarily deep, and a
cycle in it is legal too — so this walks it on demand and says so if it stops at its bound.

Paths only reads. It is the other half of ancestors: not which concepts are above one, but by
what routes. In a polyhierarchy the number of ancestors is linear and the number of routes is not,
so this has a bound of its own and says when it hit it. A route stops at a concept with no broader
concept, which is not the same thing as a scheme's top concept — SKOS relates neither to the other
— so both are reported and kept apart. A route that runs into a cycle stops there and the cycle is
named, including one that does not run through the concept asked about, which is the case an
upward walk cannot see and is still the reason a breadcrumb has no root to reach.

Tree only reads. It is ancestors turned round: the concepts one skos:narrower link below, the
ones sharing a broader concept — our term, not one SKOS states — and everything below under
skos:narrowerTransitive, printed as an indented tree in which the indentation is the path S24
licensed. A concept the graph places below another only transitively is a descendant and not a
child, and the report says so rather than letting two counts disagree.

Search only reads. It matches <text> against every label the vocabulary holds — preferred,
alternative, and hidden, which SKOS §5.1 defines for exactly this — ignoring case, anywhere in the
label, in any language. Those defaults are deliberate: a search that finds nothing is how a
duplicate concept gets created. Narrow it with:
  --exact                    the whole label and nothing less
  --prefix                   the label begins with the text
  --lang <range>             only labels whose tag the range selects (RFC 4647 basic filtering,
                             so `en` selects `en-GB`; `*` selects every tagged label)
  --untagged                 only labels with no language tag
  --kind pref|alt|hidden     only this kind; repeat for more than one
  --limit <n>                report at most n hits (default 200)
Matching ignores case but not accents, spelling, or Unicode normalisation.

Mint only reads, and reserves nothing: run it twice and it answers the same. It reports the IRI a
new concept would be given, under a pattern with one placeholder — {n} for an opaque IRI, {slug}
for one read from the label. With no --pattern the pattern is read off the vocabulary's own
concepts, and a vocabulary with no majority namespace gets no suggestion rather than a guess. A
number goes above the highest in use and never fills a gap; a slug that is already taken is
refused rather than given a disambiguating suffix. Every IRI under the namespace is checked,
across every vocabulary in the store and every change staged against one.

Policy is where that pattern stops being a guess. With no --pattern it shows what <graph> records
and writes nothing; with one it records it, replacing any pattern already recorded and saying what
it replaced. A recorded pattern is what every producer mints under — an import, a match against
another vocabulary, a curator here — so the same vocabulary cannot be given IRIs two ways. Without
one, mint infers a pattern from the vocabulary's own concepts, which is a reading of them as they
stand and therefore moves as they grow. Nothing already minted changes either way: a policy governs
the next mint. Recording one needs a name, from the same place approving does.

Inspect only reads. It reports the concepts, concept schemes, and collections a vocabulary holds,
including the ones no statement typed — SKOS itself says a resource with concepts in it is a
concept scheme — and it names the specification statement behind every fact it inferred. It
separates a violated SKOS integrity condition, which makes a graph not a SKOS vocabulary, from
something merely ill-formed, which is our judgement and says so.

Integrity only reads. It is inspect's closing sentence taken apart: every condition whose
violation makes this build call a graph inconsistent, one row each, with the specification's own
words and the counter-examples. A condition is held, violated, or **unchecked** — and unchecked is
not a weaker held. A bounded walk that gave up, or a vocabulary that refines a SKOS property this
build reads past, leaves a condition genuinely unanswered, and the report says which and why. Six
of the conditions are the specification's own; the other ten are our reading, printed apart and
labelled as ours.

Notes only reads. It prints every SKOS documentation property carrying anything for one resource —
the definition, the scope note, the examples and the rest — and beside each note that SKOS itself
entailed, the statement it came from and the rule. §7 states no integrity condition, so a resource
with no documentation is legal SKOS and this says so rather than reporting a defect. It takes any
resource, not only a concept: §7's own example documents an `owl:Class`.

Retract refuses statements the vocabulary does not already hold, and approving a retraction is
refused if the vocabulary has changed underneath it — a removal that quietly takes away less than
was reviewed is worse than one that fails.

Every command needs the store to itself; stop the server first.

The store's location comes from the same configuration the server uses:
  OPENBIZ_DATA_DIR, or data_dir in openbiz.toml.";

/// The operator's arguments did not name something this build can do.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ArgsError {
    /// The first argument is not one of our commands.
    ///
    /// Refused rather than treated as "serve, and ignore the rest": someone typing
    /// `openbiz backupp /backups/today.nq` must not get a running server and the belief that they
    /// have a backup.
    #[error("{0:?} is not an openbiz command")]
    UnknownCommand(String),
    /// A command that needs a file was given none.
    #[error("`openbiz {command}` needs a file to {verb}")]
    MissingFile {
        /// The command that was named.
        command: &'static str,
        /// What it would have done with the file, for the message.
        verb: &'static str,
    },
    /// More arguments than the command takes.
    ///
    /// Named because the likeliest cause is an unquoted path with a space in it, and half a path
    /// silently becoming the filename is how a backup ends up somewhere nobody looks for it.
    #[error("`openbiz {command}` takes one file, and {extra} more argument(s) were given")]
    TooManyArguments {
        /// The command that was named.
        command: &'static str,
        /// How many arguments were left over.
        extra: usize,
    },
    /// A command that needs an argument was given none.
    #[error("`openbiz {command}` needs {what}")]
    MissingArgument {
        /// The command that was named.
        command: &'static str,
        /// What was missing, phrased to complete the sentence above.
        what: &'static str,
    },
    /// An argument was not valid text.
    #[error("an argument is not valid Unicode, so it cannot be a command")]
    NotUnicode,
    /// An option was given that the command does not have.
    #[error("`openbiz {command}` has no option {option:?}")]
    UnknownOption {
        /// The command it was given to.
        command: &'static str,
        /// What was given.
        option: String,
    },
    /// An option that takes a value was the last thing on the line.
    #[error("{option} needs a value after it")]
    MissingOptionValue {
        /// The option.
        option: &'static str,
    },
    /// An option's value is not one the command accepts.
    #[error("{value:?} is not a value for {option}; expected {expected}")]
    BadOptionValue {
        /// The option.
        option: &'static str,
        /// What was given.
        value: String,
        /// What would have been accepted.
        expected: &'static str,
    },
    /// The same narrowing was asked for twice, in two ways that disagree.
    #[error("{option} was given after another option that contradicts it; give one or the other")]
    ConflictingOptions {
        /// The second of the two.
        option: &'static str,
    },
    /// The options did not make a query this build can run.
    #[error(transparent)]
    BadQuery(
        /// Why not.
        #[from]
        openbiz_skos::QueryError,
    ),
}

impl Command {
    /// Read a command out of the process arguments, *excluding* the program name.
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, ArgsError> {
        let mut args = args.into_iter();

        let Some(first) = args.next() else {
            return Ok(Self::Serve);
        };
        let first = first.into_string().map_err(|_| ArgsError::NotUnicode)?;

        let (name, command) = match first.as_str() {
            "help" | "--help" | "-h" => return Self::no_more(Self::Help, "help", args),
            "backup" => (
                "backup",
                Self::Backup {
                    file: Self::one_file("backup", "write", &mut args)?,
                },
            ),
            "restore" => (
                "restore",
                Self::Restore {
                    file: Self::one_file("restore", "read", &mut args)?,
                },
            ),
            "import" => (
                "import",
                Self::Import {
                    graph: Self::text(
                        "import",
                        "the IRI of the vocabulary to propose against",
                        &mut args,
                    )?,
                    file: Self::one_file("import", "read", &mut args)?,
                },
            ),
            "retract" => (
                "retract",
                Self::Retract {
                    graph: Self::text(
                        "retract",
                        "the IRI of the vocabulary to propose against",
                        &mut args,
                    )?,
                    file: Self::one_file("retract", "read", &mut args)?,
                },
            ),
            "inspect" => (
                "inspect",
                Self::Inspect {
                    graph: Self::text("inspect", "the IRI of a vocabulary to read", &mut args)?,
                },
            ),
            "ancestors" => (
                "ancestors",
                Self::Ancestors {
                    graph: Self::text("ancestors", "the IRI of a vocabulary to read", &mut args)?,
                    concept: Self::text(
                        "ancestors",
                        "the IRI of a concept to walk up from",
                        &mut args,
                    )?,
                },
            ),
            "paths" => (
                "paths",
                Self::Paths {
                    graph: Self::text("paths", "the IRI of a vocabulary to read", &mut args)?,
                    concept: Self::text(
                        "paths",
                        "the IRI of a concept to enumerate the routes above",
                        &mut args,
                    )?,
                },
            ),
            "tree" => (
                "tree",
                Self::Tree {
                    graph: Self::text("tree", "the IRI of a vocabulary to read", &mut args)?,
                    concept: Self::text(
                        "tree",
                        "the IRI of a concept to walk down from",
                        &mut args,
                    )?,
                },
            ),
            "search" => {
                let graph = Self::text("search", "the IRI of a vocabulary to read", &mut args)?;
                let text = Self::text("search", "the text to look for", &mut args)?;
                // The two positionals are taken before any option is read, so a search for a term
                // that begins with a hyphen needs no escaping and is never mistaken for a flag.
                return Ok(Self::Search {
                    graph,
                    query: Box::new(Self::search_query(&text, args)?),
                });
            }
            "mint" => {
                let graph = Self::text("mint", "the IRI of a vocabulary to read", &mut args)?;
                return Self::mint_command(graph, args);
            }
            "policy" => {
                let graph = Self::text("policy", "the IRI of a vocabulary", &mut args)?;
                return Self::policy_command(graph, args);
            }
            "integrity" => (
                "integrity",
                Self::Integrity {
                    graph: Self::text("integrity", "the IRI of a vocabulary to read", &mut args)?,
                },
            ),
            "notes" => (
                "notes",
                Self::Notes {
                    graph: Self::text("notes", "the IRI of a vocabulary to read", &mut args)?,
                    resource: Self::text(
                        "notes",
                        "the IRI of a resource to report the documentation of",
                        &mut args,
                    )?,
                },
            ),
            "mappings" => (
                "mappings",
                Self::Mappings {
                    graph: Self::text("mappings", "the IRI of a vocabulary to read", &mut args)?,
                    resource: Self::text(
                        "mappings",
                        "the IRI of a resource to report the mapping links of",
                        &mut args,
                    )?,
                },
            ),
            "candidates" => ("candidates", Self::Candidates),
            "candidate" => (
                "candidate",
                Self::Show {
                    id: Self::text("candidate", "a candidate to show", &mut args)?,
                },
            ),
            "approve" => (
                "approve",
                Self::Approve {
                    id: Self::text("approve", "a candidate to approve", &mut args)?,
                },
            ),
            "reject" => (
                "reject",
                Self::Reject {
                    id: Self::text("reject", "a candidate to reject", &mut args)?,
                },
            ),
            other => return Err(ArgsError::UnknownCommand(other.to_owned())),
        };

        Self::no_more(command, name, args)
    }

    /// Take the single file argument a command requires.
    fn one_file(
        command: &'static str,
        verb: &'static str,
        args: &mut impl Iterator<Item = OsString>,
    ) -> Result<PathBuf, ArgsError> {
        args.next()
            .map(PathBuf::from)
            .ok_or(ArgsError::MissingFile { command, verb })
    }

    /// Take a textual argument a command requires.
    fn text(
        command: &'static str,
        what: &'static str,
        args: &mut impl Iterator<Item = OsString>,
    ) -> Result<String, ArgsError> {
        args.next()
            .ok_or(ArgsError::MissingArgument { command, what })?
            .into_string()
            .map_err(|_| ArgsError::NotUnicode)
    }

    /// Read what `openbiz mint` takes after the vocabulary.
    ///
    /// The label is positional, because `openbiz mint <graph> "Renewable energy"` is how anyone
    /// would write it — but it is optional, since a `{n}` pattern needs no label, and an optional
    /// positional followed by options is ambiguous the moment a label begins with a hyphen. So the
    /// rule is stated rather than guessed at: the first argument after the vocabulary is the label
    /// unless it begins with `--`, and `--label` is there for the term that does.
    fn mint_command(
        graph: String,
        args: impl Iterator<Item = OsString>,
    ) -> Result<Self, ArgsError> {
        let mut label: Option<String> = None;
        let mut pattern: Option<String> = None;

        let mut args = args.map(|arg| arg.into_string()).peekable();
        // The positional, taken only when it cannot be an option.
        if let Some(Ok(first)) = args.peek() {
            if !first.starts_with("--") {
                label = args.next().transpose().map_err(|_| ArgsError::NotUnicode)?;
            }
        }

        while let Some(arg) = args.next() {
            let arg = arg.map_err(|_| ArgsError::NotUnicode)?;
            let mut value = |option: &'static str| -> Result<String, ArgsError> {
                args.next()
                    .ok_or(ArgsError::MissingOptionValue { option })?
                    .map_err(|_| ArgsError::NotUnicode)
            };
            match arg.as_str() {
                "--label" => {
                    let given = value("--label")?;
                    set(&mut label, given, "--label")?;
                }
                "--pattern" => {
                    let given = value("--pattern")?;
                    set(&mut pattern, given, "--pattern")?;
                }
                other => {
                    return Err(ArgsError::UnknownOption {
                        command: "mint",
                        option: other.to_owned(),
                    })
                }
            }
        }

        Ok(Self::Mint {
            graph,
            label,
            pattern,
        })
    }

    /// Read `openbiz policy`'s one option.
    ///
    /// The pattern is only ever a `--pattern` value and never a positional, which is deliberate:
    /// `openbiz policy <graph>` shows and writes nothing, so a typo in the pattern must not be the
    /// difference between reading and recording. Recording is something you ask for by name.
    fn policy_command(
        graph: String,
        args: impl Iterator<Item = OsString>,
    ) -> Result<Self, ArgsError> {
        let mut pattern: Option<String> = None;
        let mut args = args.map(|arg| arg.into_string());

        while let Some(arg) = args.next() {
            let arg = arg.map_err(|_| ArgsError::NotUnicode)?;
            match arg.as_str() {
                "--pattern" => {
                    let given = args
                        .next()
                        .ok_or(ArgsError::MissingOptionValue {
                            option: "--pattern",
                        })?
                        .map_err(|_| ArgsError::NotUnicode)?;
                    set(&mut pattern, given, "--pattern")?;
                }
                other => {
                    return Err(ArgsError::UnknownOption {
                        command: "policy",
                        option: other.to_owned(),
                    })
                }
            }
        }

        Ok(Self::Policy { graph, pattern })
    }

    /// Read the options `openbiz search` accepts, refusing anything it does not.
    ///
    /// Every option that narrows the search is refused twice over rather than taken last-wins: a
    /// user who typed `--exact --prefix` meant one of them, and quietly obeying the second is how
    /// a report comes back narrower than the person who ran it believes.
    fn search_query(
        text: &str,
        args: impl Iterator<Item = OsString>,
    ) -> Result<LabelQuery, ArgsError> {
        let mut query = LabelQuery::new(text).map_err(ArgsError::BadQuery)?;
        let mut mode: Option<MatchMode> = None;
        let mut language: Option<LanguageFilter> = None;
        let mut kinds: Vec<LabelKind> = Vec::new();
        let mut limit: Option<usize> = None;

        let mut args = args.map(|arg| arg.into_string()).peekable();
        while let Some(arg) = args.next() {
            let arg = arg.map_err(|_| ArgsError::NotUnicode)?;
            let mut value = |option: &'static str| -> Result<String, ArgsError> {
                args.next()
                    .ok_or(ArgsError::MissingOptionValue { option })?
                    .map_err(|_| ArgsError::NotUnicode)
            };
            match arg.as_str() {
                "--exact" => set(&mut mode, MatchMode::Exact, "--exact")?,
                "--prefix" => set(&mut mode, MatchMode::Prefix, "--prefix")?,
                "--infix" => set(&mut mode, MatchMode::Infix, "--infix")?,
                "--lang" => {
                    let range = value("--lang")?;
                    let range = LanguageRange::parse(&range).map_err(ArgsError::BadQuery)?;
                    set(&mut language, LanguageFilter::Range(range), "--lang")?;
                }
                "--untagged" => set(&mut language, LanguageFilter::Untagged, "--untagged")?,
                "--kind" => {
                    let kind = value("--kind")?;
                    kinds.push(match kind.as_str() {
                        "pref" | "preferred" | "prefLabel" => LabelKind::Preferred,
                        "alt" | "alternative" | "altLabel" => LabelKind::Alternative,
                        "hidden" | "hiddenLabel" => LabelKind::Hidden,
                        other => {
                            return Err(ArgsError::BadOptionValue {
                                option: "--kind",
                                value: other.to_owned(),
                                expected: "pref, alt, or hidden",
                            })
                        }
                    });
                }
                "--limit" => {
                    let given = value("--limit")?;
                    let parsed = given
                        .parse::<usize>()
                        .map_err(|_| ArgsError::BadOptionValue {
                            option: "--limit",
                            value: given.clone(),
                            expected: "a whole number of hits",
                        })?;
                    set(&mut limit, parsed, "--limit")?;
                }
                other => {
                    return Err(ArgsError::UnknownOption {
                        command: "search",
                        option: other.to_owned(),
                    })
                }
            }
        }

        if let Some(mode) = mode {
            query = query.with_mode(mode);
        }
        if let Some(language) = language {
            query = query.with_language(language);
        }
        if !kinds.is_empty() {
            query = query.with_kinds(kinds).map_err(ArgsError::BadQuery)?;
        }
        if let Some(max_hits) = limit {
            query = query.with_bound(SearchBound { max_hits });
        }
        Ok(query)
    }

    /// Refuse anything left over rather than ignoring it.
    fn no_more(
        command: Self,
        name: &'static str,
        args: impl Iterator<Item = OsString>,
    ) -> Result<Self, ArgsError> {
        let extra = args.count();
        if extra > 0 {
            return Err(ArgsError::TooManyArguments {
                command: name,
                extra,
            });
        }
        Ok(command)
    }
}

/// Record an option's value, refusing a second one that narrows the same thing.
///
/// Last-wins is the usual convention and is wrong here. `--exact --prefix` is not a user changing
/// their mind mid-line; it is a user who does not know which of the two they asked for, and a
/// report that silently obeys the second comes back narrower than the person reading it believes.
/// The same applies to `--lang fr --untagged`, which asks for two disjoint sets of labels.
fn set<T>(slot: &mut Option<T>, value: T, option: &'static str) -> Result<(), ArgsError> {
    if slot.is_some() {
        return Err(ArgsError::ConflictingOptions { option });
    }
    *slot = Some(value);
    Ok(())
}

/// A backup or restore could not be carried out.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CommandError {
    /// The backup file already exists.
    ///
    /// A backup never overwrites, because the file most likely to be in the way is the last good
    /// backup — and overwriting that with a partial one is the failure that turns a bad day into
    /// an unrecoverable one.
    #[error(
        "{} already exists, and a backup never overwrites a file; \
         choose another name or move the existing one",
        path.display()
    )]
    WouldOverwrite {
        /// The file that is in the way.
        path: PathBuf,
    },
    /// The backup file could not be created or written.
    #[error("could not write the backup to {}: {source}", path.display())]
    Write {
        /// The file that failed.
        path: PathBuf,
        /// The underlying cause.
        #[source]
        source: std::io::Error,
    },
    /// The backup file could not be opened for reading.
    #[error("could not read the backup at {}: {source}", path.display())]
    Read {
        /// The file that failed.
        path: PathBuf,
        /// The underlying cause.
        #[source]
        source: std::io::Error,
    },
    /// The file's extension does not name a syntax we read.
    #[error(
        "the syntax of {} is taken from its extension, and {extension:?} is not one OpenBiz \
         reads; rename it to one of: {known}",
        path.display()
    )]
    UnknownSyntax {
        /// The file that could not be classified.
        path: PathBuf,
        /// The extension it had, or the empty string if it had none.
        extension: String,
        /// The extensions we do read, for the message.
        known: String,
    },
    /// A decision was asked for and nothing named who was taking it.
    ///
    /// Not a nuisance and not a placeholder: the store refuses an unattributed decision, because
    /// an approval nobody can be traced to is the one record an audit cannot do without. Until
    /// there is authentication, the command line's honest answer is to ask.
    #[error(
        "there is nobody to record as having taken this decision; set OPENBIZ_ACTOR to the person \
         or system responsible, or run this where USER or LOGNAME is set"
    )]
    NoActor,
    /// The graph IRI given on the command line is not one.
    #[error(transparent)]
    Graph(#[from] GraphIdError),
    /// The candidate identifier given on the command line is not one.
    #[error(transparent)]
    CandidateId(#[from] CandidateIdError),
    /// The store refused the operation, or failed during it.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The vocabulary does not mention the concept that was asked about.
    ///
    /// Refused rather than answered with "nothing is above it", which is what a root concept
    /// says. The two are indistinguishable in the output and opposite in meaning, and at a
    /// command line a mistyped IRI is far likelier than a genuine root.
    #[error("{graph} says nothing about {concept}, so there is nothing to walk up from")]
    NoSuchConcept {
        /// The concept IRI that was asked about.
        concept: String,
        /// The vocabulary that was read.
        graph: String,
    },
    /// The pattern given with `--pattern` is not one this build mints under.
    #[error(transparent)]
    Pattern(#[from] PatternError),
    /// No pattern was given, nothing is recorded, and the vocabulary does not suggest one.
    ///
    /// Refused rather than defaulted. A pattern invented here would mint IRIs in a namespace
    /// nothing else in the deployment uses, and they would look every bit as official as the real
    /// ones — which is worse than being asked to type one.
    #[error("{0}; give one with --pattern, or record one with `openbiz policy`")]
    NoConvention(NoConvention),
    /// The vocabulary records a minting pattern this build cannot read.
    ///
    /// Refused rather than fallen back from. The vocabulary has a written decision about how its
    /// IRIs are named; minting under something else because we could not parse that decision would
    /// put concepts in a namespace nobody chose, and the IRIs would be permanent before anyone
    /// looked. Recording a correct one is a single command, and the refusal names it.
    #[error(
        "{graph} records the minting pattern {pattern:?} — set by {recorded_by} — and this build \
         cannot read it: {source}. Record a pattern this build accepts with `openbiz policy`, or \
         mint once with --pattern"
    )]
    RecordedPatternUnusable {
        /// The vocabulary whose policy could not be read.
        graph: String,
        /// The pattern as it is recorded.
        pattern: String,
        /// Who recorded it, so the person to ask is named.
        recorded_by: String,
        /// Why it could not be read.
        #[source]
        source: PatternError,
    },
    /// The minted IRI was rejected by the parser that would have stored it.
    #[error(
        "{iri:?} would be minted and the store's own RDF parser will not accept it as an IRI; \
         the pattern is at fault"
    )]
    NotAnIri {
        /// What would have been minted.
        iri: String,
    },
    /// The vocabulary says nothing in SKOS terms about the resource that was asked about.
    ///
    /// Distinct from [`CommandError::NoSuchConcept`] because the question is different: `openbiz
    /// notes` takes any resource, not a concept, since §7's own Example 24 documents an
    /// `owl:Class`. Refused rather than answered with "it carries no documentation", which is
    /// what a real but undocumented concept says — and which is a legal, consistent state that a
    /// mistyped IRI must not be confused with.
    #[error("{graph} says nothing about {resource} in SKOS terms, so there is nothing to report")]
    NoSuchResource {
        /// The resource IRI that was asked about.
        resource: String,
        /// The vocabulary that was read.
        graph: String,
    },
}

/// Environment variable naming who is responsible for a decision taken on the command line.
pub const ACTOR_VARIABLE: &str = "OPENBIZ_ACTOR";

/// Propose the statements in `file` as a change to the vocabulary at `graph`.
///
/// Nothing reaches the vocabulary. The syntax comes from the file's extension — the same table
/// `?format=` and `Accept` are resolved against, so what `openbiz export` writes is what this
/// reads back — and an extension we do not know is refused rather than guessed at, because
/// reading a file as the wrong syntax produces either a syntax error two hundred lines in or, far
/// worse, a successful import of something else.
pub fn import(store: &Store, graph: &str, file: &Path) -> Result<String, CommandError> {
    propose(store, graph, file, CandidatePart::Additions)
}

/// Propose the statements in `file` as a removal from the vocabulary at `graph`.
///
/// The mirror of [`import`], and the command that makes a correction, a merge, or a deprecation
/// expressible at all. Nothing reaches the vocabulary here either. It refuses a file naming
/// statements the vocabulary does not hold, because a removal that matches nothing looks
/// successful and changes nothing — see [`Store::propose_retraction`].
pub fn retract(store: &Store, graph: &str, file: &Path) -> Result<String, CommandError> {
    propose(store, graph, file, CandidatePart::Removals)
}

/// Raise a candidate from a file, for whichever half of a change it carries.
fn propose(
    store: &Store,
    graph: &str,
    file: &Path,
    part: CandidatePart,
) -> Result<String, CommandError> {
    let target = GraphId::vocabulary(graph)?;

    let extension = file
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let Some(syntax) = RdfSyntax::parse(extension) else {
        return Err(CommandError::UnknownSyntax {
            path: file.to_path_buf(),
            extension: extension.to_owned(),
            known: RdfSyntax::ALL
                .iter()
                .map(|syntax| format!(".{}", syntax.file_extension()))
                .collect::<Vec<_>>()
                .join(", "),
        });
    };

    let handle = File::open(file).map_err(|source| CommandError::Read {
        path: file.to_path_buf(),
        source,
    })?;

    let command = match part {
        CandidatePart::Additions => "import",
        CandidatePart::Removals => "retract",
    };
    let provenance = Provenance {
        source: CandidateSource::Import,
        agent: format!("{} (openbiz {command})", actor()?),
        note: format!(
            "{} from {} as {syntax}",
            match part {
                CandidatePart::Additions => "imported",
                CandidatePart::Removals => "retraction read",
            },
            file.display()
        ),
        // A file has no confidence to state. Inventing one — 1.0, say — would put a number a
        // reviewer could sort by next to numbers that mean something.
        confidence: None,
    };

    let reader = BufReader::new(handle);
    let candidate = match part {
        CandidatePart::Additions => store.propose_import(&target, syntax, reader, &provenance)?,
        CandidatePart::Removals => {
            store.propose_retraction(&target, syntax, reader, &provenance)?
        }
    };

    Ok(format!(
        "proposed candidate {} against {}: {} from {}, read as {syntax}\n\
         nothing has been written to the vocabulary. Review it with `openbiz candidate {}`, then \
         `openbiz approve {}` or `openbiz reject {}`.",
        candidate.id(),
        candidate.target(),
        effect(&candidate),
        file.display(),
        candidate.id(),
        candidate.id(),
        candidate.id(),
    ))
}

/// What a candidate would do to its target, in the words a list or a report uses.
///
/// Both halves are always named when both are present, and a half that is empty is left out
/// rather than printed as a zero: "adds 4 statements, removes 0" invites the reader to wonder
/// which of the two numbers is the interesting one.
fn effect(candidate: &Candidate) -> String {
    let statements = |count: u64| {
        if count == 1 {
            "1 statement".to_owned()
        } else {
            format!("{count} statements")
        }
    };
    match (candidate.additions(), candidate.removals()) {
        (adds, 0) => format!("adds {}", statements(adds)),
        (0, removes) => format!("removes {}", statements(removes)),
        (adds, removes) => format!(
            "adds {} and removes {}",
            statements(adds),
            statements(removes)
        ),
    }
}

/// List every proposed change the store holds, oldest first.
pub fn candidates(store: &Store) -> Result<String, CommandError> {
    let candidates = store.candidates()?;

    if candidates.is_empty() {
        return Ok("no changes have been proposed".to_owned());
    }

    let mut out = String::new();
    for candidate in &candidates {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            candidate.id(),
            candidate.state(),
            effect(candidate),
            candidate.target(),
            candidate.provenance().note,
        ));
    }
    out.push_str(&format!(
        "{} proposed, {} applied, {} rejected",
        count(&candidates, CandidateState::Proposed),
        count(&candidates, CandidateState::Applied),
        count(&candidates, CandidateState::Rejected),
    ));
    Ok(out)
}

/// How many of `candidates` are in `state`.
fn count(candidates: &[Candidate], state: CandidateState) -> usize {
    candidates
        .iter()
        .filter(|candidate| candidate.state() == state)
        .count()
}

/// Show one proposed change, provenance first and then the statements themselves.
///
/// The statements are the reason this command exists. An approval taken without reading them is
/// not a review, and "go and write a SPARQL query" is not an answer for the person whose job is to
/// decide. They come out as Turtle, which is the readable one of the six.
pub fn show(store: &Store, id: &str) -> Result<String, CommandError> {
    let candidate = store.candidate(CandidateId::parse(id)?)?;
    let provenance = candidate.provenance();

    let mut out = format!(
        "candidate {}\n  state:      {}\n  target:     {}\n  source:     {}\n           proposed by: {}\n  proposed at: {}\n  why:        {}\n  effect:     {}\n",
        candidate.id(),
        candidate.state(),
        candidate.target(),
        provenance.source,
        provenance.agent,
        candidate.proposed_at(),
        provenance.note,
        effect(&candidate),
    );
    if let Some(confidence) = provenance.confidence {
        out.push_str(&format!("  confidence: {confidence}\n"));
    }
    if let (Some(by), Some(at)) = (candidate.decided_by(), candidate.decided_at()) {
        out.push_str(&format!("  decided by: {by}\n  decided at: {at}\n"));
    }
    // Removals first, which is the order they are applied in and the order somebody reading a
    // change wants them: what goes, then what arrives.
    for (heading, graph) in [
        ("would remove", candidate.removal_payload()),
        ("would add", candidate.payload()),
    ] {
        let Some(graph) = graph else {
            continue;
        };
        out.push_str(&format!("\n{heading}, staged in {graph}:\n\n"));

        let mut statements = Vec::new();
        store.export_graph(graph.iri(), RdfSyntax::Turtle, &mut statements)?;
        out.push_str(&String::from_utf8(statements).map_err(|error| {
            CommandError::Store(StoreError::Backend(format!(
                "the staged statements are not valid UTF-8, which no serialiser of ours writes: \
                 {error}"
            )))
        })?);
    }

    Ok(out)
}

/// Approve or reject a proposed change.
pub fn decide(store: &Store, id: &str, decision: Decision) -> Result<String, CommandError> {
    let candidate = store.decide(CandidateId::parse(id)?, decision, &actor()?)?;

    Ok(match decision {
        Decision::Approve => format!(
            "approved candidate {}: it {} in {}, recorded as approved by {}",
            candidate.id(),
            effect(&candidate),
            candidate.target(),
            candidate.decided_by().unwrap_or_default(),
        ),
        Decision::Reject => format!(
            "rejected candidate {}: {} is unchanged. The statements stay staged in {}, so what \
             was refused is still readable",
            candidate.id(),
            candidate.target(),
            staged_in(&candidate),
        ),
    })
}

/// The staging graphs a candidate's statements are kept in, for a message that names them.
fn staged_in(candidate: &Candidate) -> String {
    [candidate.removal_payload(), candidate.payload()]
        .into_iter()
        .flatten()
        .map(GraphId::to_string)
        .collect::<Vec<_>>()
        .join(" and ")
}

/// Who to record as responsible for a decision taken from the command line.
///
/// There is no authentication yet, so this is the operating system's account of who ran the
/// command — and it says so in the recorded string rather than dressing it up as an identity the
/// product verified. [`ACTOR_VARIABLE`] overrides it, because the account a cron job runs under is
/// rarely the team answerable for what it did.
pub(crate) fn actor() -> Result<String, CommandError> {
    [ACTOR_VARIABLE, "USER", "LOGNAME"]
        .into_iter()
        .find_map(|variable| {
            std::env::var(variable)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .ok_or(CommandError::NoActor)
}

/// Write `store` out to `file`, and report what was written.
///
/// The file is created exclusively — an existing one is [`CommandError::WouldOverwrite`], never a
/// truncation — and is fsynced before this returns. A backup that is only in the page cache is
/// exactly as durable as the machine you are backing up, which is to say: not a backup.
pub fn back_up(store: &Store, file: &Path) -> Result<String, CommandError> {
    let handle = File::create_new(file).map_err(|source| {
        if source.kind() == std::io::ErrorKind::AlreadyExists {
            CommandError::WouldOverwrite {
                path: file.to_path_buf(),
            }
        } else {
            CommandError::Write {
                path: file.to_path_buf(),
                source,
            }
        }
    })?;

    let write_failed = |source| CommandError::Write {
        path: file.to_path_buf(),
        source,
    };

    let mut writer = BufWriter::new(handle);
    let report = store.backup(&mut writer)?;
    writer.flush().map_err(write_failed)?;
    writer
        .into_inner()
        .map_err(|error| write_failed(error.into_error()))?
        .sync_all()
        .map_err(write_failed)?;

    Ok(format!(
        "backed up {} statements from {} graphs to {} ({})",
        report.quads(),
        report.graphs(),
        file.display(),
        BACKUP_SYNTAX,
    ))
}

/// Rebuild `store` from `file`, and report what was restored.
///
/// `store` must be empty; see [`Store::restore`] for every refusal and why each exists. A refusal
/// leaves the store untouched, so a failed restore is safe to diagnose and retry.
pub fn restore(store: &Store, file: &Path) -> Result<String, CommandError> {
    let handle = File::open(file).map_err(|source| CommandError::Read {
        path: file.to_path_buf(),
        source,
    })?;

    let report = store.restore(BufReader::new(handle))?;

    // An operator restoring a backup taken by an older release needs to be told that the file was
    // brought forward, and by what. Saying only "restored 12 000 statements" would hide a change
    // to their data behind a count that looks identical either way.
    let migrated = if report.migrations().migrated() {
        format!("; {}", report.migrations())
    } else {
        String::new()
    };

    Ok(format!(
        "restored {} statements into {} graphs, from {}{migrated}",
        report.quads(),
        report.graphs(),
        file.display(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, ArgsError> {
        Command::parse(args.iter().map(OsString::from))
    }

    #[test]
    fn no_arguments_means_serve() {
        assert_eq!(parse(&[]), Ok(Command::Serve));
    }

    #[test]
    fn backup_and_restore_take_one_file() {
        assert_eq!(
            parse(&["backup", "/backups/today.nq"]),
            Ok(Command::Backup {
                file: PathBuf::from("/backups/today.nq")
            })
        );
        assert_eq!(
            parse(&["restore", "/backups/today.nq"]),
            Ok(Command::Restore {
                file: PathBuf::from("/backups/today.nq")
            })
        );
    }

    #[test]
    fn help_is_spelled_three_ways() {
        for spelling in ["help", "--help", "-h"] {
            assert_eq!(parse(&[spelling]), Ok(Command::Help), "spelled {spelling}");
        }
    }

    /// The two positionals, and the defaults that make the command forgiving by design.
    #[test]
    fn search_defaults_to_the_forgiving_query() {
        let Ok(Command::Search { graph, query }) = parse(&["search", "http://e.org/v", "bag"])
        else {
            panic!("search takes a graph and some text");
        };
        assert_eq!(graph, "http://e.org/v");
        assert_eq!(query.text(), "bag");
        assert_eq!(query.mode(), MatchMode::Infix);
        assert_eq!(query.language(), &LanguageFilter::Any);
        assert_eq!(query.kinds().len(), 3, "all three kinds, hidden included");
        assert_eq!(query.bound(), SearchBound::DEFAULT);
    }

    /// The ordinary form: a vocabulary and the term the new concept will be called.
    #[test]
    fn mint_takes_a_vocabulary_and_an_optional_label() {
        assert_eq!(
            parse(&["mint", "http://e.org/v", "Renewable energy"]),
            Ok(Command::Mint {
                graph: "http://e.org/v".to_owned(),
                label: Some("Renewable energy".to_owned()),
                pattern: None,
            })
        );
        assert_eq!(
            parse(&["mint", "http://e.org/v"]),
            Ok(Command::Mint {
                graph: "http://e.org/v".to_owned(),
                label: None,
                pattern: None,
            })
        );
    }

    /// The positional label is optional, so an argument that begins with `--` is read as an option
    /// and never silently swallowed as the label. `--label` is how the awkward term is given.
    #[test]
    fn a_label_beginning_with_two_hyphens_needs_the_option() {
        let error = parse(&["mint", "http://e.org/v", "--peculiar"]).expect_err("not an option");
        assert_eq!(
            error,
            ArgsError::UnknownOption {
                command: "mint",
                option: "--peculiar".to_owned()
            }
        );

        assert_eq!(
            parse(&["mint", "http://e.org/v", "--label", "--peculiar"]),
            Ok(Command::Mint {
                graph: "http://e.org/v".to_owned(),
                label: Some("--peculiar".to_owned()),
                pattern: None,
            })
        );
    }

    #[test]
    fn mint_takes_a_pattern() {
        assert_eq!(
            parse(&["mint", "http://e.org/v", "--pattern", "http://e.org/v/{n}"]),
            Ok(Command::Mint {
                graph: "http://e.org/v".to_owned(),
                label: None,
                pattern: Some("http://e.org/v/{n}".to_owned()),
            })
        );
    }

    #[test]
    fn policy_shows_by_default_and_records_only_when_asked() {
        assert_eq!(
            parse(&["policy", "http://e.org/v"]),
            Ok(Command::Policy {
                graph: "http://e.org/v".to_owned(),
                pattern: None,
            }),
            "no pattern means show, which writes nothing"
        );
        assert_eq!(
            parse(&[
                "policy",
                "http://e.org/v",
                "--pattern",
                "http://e.org/v/c_{n}"
            ]),
            Ok(Command::Policy {
                graph: "http://e.org/v".to_owned(),
                pattern: Some("http://e.org/v/c_{n}".to_owned()),
            })
        );
    }

    /// Recording is asked for by name and never by position. A pattern taken as a positional would
    /// make a typo in a vocabulary IRI the difference between reading and writing.
    #[test]
    fn policy_refuses_a_pattern_given_as_a_positional() {
        assert_eq!(
            parse(&["policy", "http://e.org/v", "http://e.org/v/c_{n}"]),
            Err(ArgsError::UnknownOption {
                command: "policy",
                option: "http://e.org/v/c_{n}".to_owned()
            })
        );
    }

    /// The same refusal `mint` makes, for the same reason: two patterns is somebody who does not
    /// know which they asked for, and the second one is the one that gets written down.
    #[test]
    fn policy_refuses_two_patterns() {
        assert_eq!(
            parse(&[
                "policy",
                "http://e.org/v",
                "--pattern",
                "http://e.org/v/c_{n}",
                "--pattern",
                "http://e.org/v/{slug}"
            ]),
            Err(ArgsError::ConflictingOptions {
                option: "--pattern"
            })
        );
    }

    #[test]
    fn policy_needs_a_vocabulary() {
        assert!(parse(&["policy"]).is_err(), "the graph is not optional");
    }

    /// Two patterns is somebody who does not know which they asked for, and obeying the second
    /// mints into a namespace they did not read.
    #[test]
    fn two_patterns_are_refused_rather_than_taken_last_wins() {
        let error = parse(&[
            "mint",
            "http://e.org/v",
            "--pattern",
            "http://e.org/a/{n}",
            "--pattern",
            "http://e.org/b/{n}",
        ])
        .expect_err("two patterns");

        assert_eq!(
            error,
            ArgsError::ConflictingOptions {
                option: "--pattern"
            }
        );
    }

    #[test]
    fn a_pattern_with_no_value_is_refused() {
        assert_eq!(
            parse(&["mint", "http://e.org/v", "--pattern"]),
            Err(ArgsError::MissingOptionValue {
                option: "--pattern"
            })
        );
    }

    /// The options are read after both positionals, so a term beginning with a hyphen needs no
    /// escaping — `--` is a legitimate thing to look for in a notation-heavy vocabulary.
    #[test]
    fn a_search_term_that_looks_like_a_flag_is_still_the_term() {
        let Ok(Command::Search { query, .. }) = parse(&["search", "http://e.org/v", "--exact"])
        else {
            panic!("the second positional is the text, whatever it looks like");
        };
        assert_eq!(query.text(), "--exact");
        assert_eq!(
            query.mode(),
            MatchMode::Infix,
            "it was the term, so it did not also set the mode"
        );
    }

    #[test]
    fn every_narrowing_option_is_read() {
        let Ok(Command::Search { query, .. }) = parse(&[
            "search",
            "http://e.org/v",
            "bag",
            "--prefix",
            "--lang",
            "en-GB",
            "--kind",
            "pref",
            "--kind",
            "alt",
            "--limit",
            "5",
        ]) else {
            panic!("all of these are search options");
        };
        assert_eq!(query.mode(), MatchMode::Prefix);
        assert_eq!(
            query.language(),
            &LanguageFilter::Range(LanguageRange::parse("en-GB").expect("a range"))
        );
        assert_eq!(
            query.kinds().iter().copied().collect::<Vec<_>>(),
            vec![LabelKind::Preferred, LabelKind::Alternative]
        );
        assert_eq!(query.bound(), SearchBound { max_hits: 5 });
    }

    /// Two options that narrow the same thing are refused rather than resolved last-wins. A user
    /// who typed both does not know which they asked for, and a report that quietly obeys the
    /// second is narrower than the person reading it believes.
    #[test]
    fn two_options_that_contradict_each_other_are_refused() {
        for line in [
            vec!["search", "http://e.org/v", "bag", "--exact", "--prefix"],
            vec![
                "search",
                "http://e.org/v",
                "bag",
                "--lang",
                "fr",
                "--untagged",
            ],
            vec![
                "search",
                "http://e.org/v",
                "bag",
                "--limit",
                "5",
                "--limit",
                "6",
            ],
        ] {
            let error = parse(&line).expect_err("contradictory narrowing must be refused");
            assert!(
                matches!(error, ArgsError::ConflictingOptions { .. }),
                "{line:?} gave {error}"
            );
        }
    }

    /// A search cannot be narrowed to no label kinds at all, and a malformed language range is
    /// refused rather than kept and matched against nothing — either would report an empty search
    /// that reads exactly like a vocabulary with no such term.
    #[test]
    fn a_query_that_could_never_match_is_refused_at_the_command_line() {
        assert!(matches!(
            parse(&["search", "http://e.org/v", ""]),
            Err(ArgsError::BadQuery(openbiz_skos::QueryError::EmptyQuery))
        ));
        assert!(matches!(
            parse(&["search", "http://e.org/v", "bag", "--lang", "en_GB"]),
            Err(ArgsError::BadQuery(
                openbiz_skos::QueryError::MalformedLanguageRange { .. }
            ))
        ));
    }

    #[test]
    fn an_option_search_does_not_have_is_refused_rather_than_ignored() {
        let error = parse(&["search", "http://e.org/v", "bag", "--fuzzy"])
            .expect_err("an unknown option must be refused");
        assert!(
            matches!(&error, ArgsError::UnknownOption { option, .. } if option == "--fuzzy"),
            "{error}"
        );

        for option in ["--lang", "--kind", "--limit"] {
            let error = parse(&["search", "http://e.org/v", "bag", option])
                .expect_err("an option with no value must be refused");
            assert!(
                matches!(error, ArgsError::MissingOptionValue { .. }),
                "{option} with nothing after it gave {error}"
            );
        }

        let error = parse(&["search", "http://e.org/v", "bag", "--kind", "preffered"])
            .expect_err("a misspelt kind must be refused");
        assert!(matches!(error, ArgsError::BadOptionValue { .. }), "{error}");
    }

    /// The failure that matters most: a typo must never silently start a server while the
    /// operator believes a backup is being taken.
    #[test]
    fn a_mistyped_command_is_refused_rather_than_treated_as_serve() {
        let error = parse(&["backupp", "/backups/today.nq"]).expect_err("a typo must be refused");
        assert_eq!(error, ArgsError::UnknownCommand("backupp".to_owned()));
        assert!(error.to_string().contains("backupp"));
    }

    #[test]
    fn a_command_with_no_file_says_which_command_needed_one() {
        for command in ["backup", "restore"] {
            let error = parse(&[command]).expect_err("a missing file must be refused");
            assert!(
                error.to_string().contains(command),
                "the message must name the command: {error}"
            );
        }
    }

    /// The likeliest cause is an unquoted path with a space in it, where taking the first word
    /// would write the backup somewhere nobody will look for it.
    #[test]
    fn extra_arguments_are_refused_rather_than_ignored() {
        let error = parse(&["backup", "/backups/last", "week.nq"])
            .expect_err("extra arguments must be refused");
        assert_eq!(
            error,
            ArgsError::TooManyArguments {
                command: "backup",
                extra: 1
            }
        );
        assert_eq!(
            parse(&["help", "me"]),
            Err(ArgsError::TooManyArguments {
                command: "help",
                extra: 1
            })
        );
    }

    #[test]
    fn import_takes_the_vocabulary_it_proposes_against_and_then_the_file() {
        assert_eq!(
            parse(&["import", "https://example.org/regions", "concepts.ttl"]),
            Ok(Command::Import {
                graph: "https://example.org/regions".to_owned(),
                file: PathBuf::from("concepts.ttl"),
            })
        );
    }

    #[test]
    fn retract_takes_the_vocabulary_it_proposes_against_and_then_the_file() {
        assert_eq!(
            parse(&["retract", "https://example.org/regions", "wrong.nt"]),
            Ok(Command::Retract {
                graph: "https://example.org/regions".to_owned(),
                file: PathBuf::from("wrong.nt"),
            })
        );
        let error =
            parse(&["retract", "https://example.org/regions"]).expect_err("a file is required");
        assert!(
            error.to_string().contains("retract"),
            "the message must name the command: {error}"
        );
        let error = parse(&["retract"]).expect_err("a graph is required");
        assert!(
            error.to_string().contains("retract") && error.to_string().contains("IRI"),
            "the message must say what was missing: {error}"
        );
    }

    #[test]
    fn the_review_commands_take_a_candidate_and_the_list_takes_nothing() {
        assert_eq!(parse(&["candidates"]), Ok(Command::Candidates));
        assert_eq!(
            parse(&["candidate", "7"]),
            Ok(Command::Show { id: "7".to_owned() })
        );
        assert_eq!(
            parse(&["approve", "7"]),
            Ok(Command::Approve { id: "7".to_owned() })
        );
        assert_eq!(
            parse(&["reject", "7"]),
            Ok(Command::Reject { id: "7".to_owned() })
        );
        assert_eq!(
            parse(&["candidates", "7"]),
            Err(ArgsError::TooManyArguments {
                command: "candidates",
                extra: 1
            })
        );
    }

    /// Two arguments, both required, in that order — and a third refused, because an unquoted
    /// IRI is not the mistake here but a swapped pair would be silently wrong.
    #[test]
    fn ancestors_takes_a_vocabulary_and_a_concept() {
        assert_eq!(
            parse(&[
                "ancestors",
                "https://example.org/regions",
                "https://example.org/regions/japan"
            ]),
            Ok(Command::Ancestors {
                graph: "https://example.org/regions".to_owned(),
                concept: "https://example.org/regions/japan".to_owned(),
            })
        );
        let error = parse(&["ancestors", "https://example.org/regions"])
            .expect_err("a concept is required");
        assert!(
            error.to_string().contains("concept"),
            "the message must say what was missing: {error}"
        );
        assert_eq!(
            parse(&["ancestors", "a", "b", "c"]),
            Err(ArgsError::TooManyArguments {
                command: "ancestors",
                extra: 1
            })
        );
    }

    /// Two arguments, both required, and the second is a *resource* rather than a concept —
    /// §7's own Example 24 documents an `owl:Class`, so the parameter must not promise a concept
    /// it does not require.
    #[test]
    fn notes_takes_a_vocabulary_and_a_resource() {
        assert_eq!(
            parse(&[
                "notes",
                "https://example.org/regions",
                "https://example.org/regions/apac"
            ]),
            Ok(Command::Notes {
                graph: "https://example.org/regions".to_owned(),
                resource: "https://example.org/regions/apac".to_owned(),
            })
        );
        let error =
            parse(&["notes", "https://example.org/regions"]).expect_err("a resource is required");
        assert!(
            error.to_string().contains("resource"),
            "the message must say what was missing: {error}"
        );
        assert_eq!(
            parse(&["notes", "a", "b", "c"]),
            Err(ArgsError::TooManyArguments {
                command: "notes",
                extra: 1
            })
        );
    }

    /// Two arguments, both required, and the second is a *resource* for the same reason
    /// `notes` takes one: §10 puts no domain on the mapping properties beyond `skos:Concept`
    /// through S39, and a report is worth printing for anything the model holds.
    #[test]
    fn mappings_takes_a_vocabulary_and_a_resource() {
        assert_eq!(
            parse(&[
                "mappings",
                "https://example.org/regions",
                "https://example.org/regions/apac"
            ]),
            Ok(Command::Mappings {
                graph: "https://example.org/regions".to_owned(),
                resource: "https://example.org/regions/apac".to_owned(),
            })
        );
        let error = parse(&["mappings", "https://example.org/regions"])
            .expect_err("a resource is required");
        assert!(
            error.to_string().contains("resource"),
            "the message must say what was missing: {error}"
        );
        assert_eq!(
            parse(&["mappings", "a", "b", "c"]),
            Err(ArgsError::TooManyArguments {
                command: "mappings",
                extra: 1
            })
        );
    }

    #[test]
    fn inspect_takes_one_vocabulary_and_refuses_a_second_argument() {
        assert_eq!(
            parse(&["inspect", "https://example.org/regions"]),
            Ok(Command::Inspect {
                graph: "https://example.org/regions".to_owned(),
            })
        );
        assert_eq!(
            parse(&["inspect", "https://example.org/regions", "extra"]),
            Err(ArgsError::TooManyArguments {
                command: "inspect",
                extra: 1
            })
        );
        let error = parse(&["inspect"]).expect_err("a vocabulary is required");
        assert!(
            error.to_string().contains("inspect") && error.to_string().contains("IRI"),
            "the message must say what was missing: {error}"
        );
    }

    /// The identifier is not validated here on purpose: `openbiz approve banana` must fail with
    /// the store's account of what a candidate identifier is, not with a usage error that says
    /// only that something was wrong.
    #[test]
    fn a_command_with_no_candidate_says_which_command_needed_one() {
        for command in ["candidate", "approve", "reject"] {
            let error = parse(&[command]).expect_err("a missing identifier must be refused");
            assert!(
                error.to_string().contains(command),
                "the message must name the command: {error}"
            );
        }
        let error = parse(&["import"]).expect_err("a missing graph must be refused");
        assert!(
            error.to_string().contains("import") && error.to_string().contains("IRI"),
            "the message must say what was missing: {error}"
        );
        let error = parse(&["import", "https://example.org/regions"])
            .expect_err("a missing file must be refused");
        assert!(
            error.to_string().contains("import"),
            "the message must name the command: {error}"
        );
    }

    /// The list is hand-maintained, and it had **drifted**: `inspect` and `ancestors` were
    /// missing from it, so the test's own name was untrue for two iterations. Found while adding
    /// `notes`, and corrected here rather than only extended — a completeness test that is
    /// quietly incomplete is worse than none, because it reports coverage it does not have.
    #[test]
    fn the_usage_names_every_command_it_can_parse() {
        for command in [
            "backup",
            "restore",
            "import",
            "retract",
            "inspect",
            "integrity",
            "ancestors",
            "paths",
            "tree",
            "search",
            "notes",
            "mappings",
            "candidates",
            "candidate",
            "approve",
            "reject",
            "help",
        ] {
            assert!(
                USAGE.contains(command),
                "usage does not mention {command}, so nobody can discover it"
            );
            assert!(
                !matches!(
                    parse(&[command, "a", "b"]),
                    Err(ArgsError::UnknownCommand(_))
                ),
                "usage documents {command} and the parser does not know it"
            );
        }
    }
}
