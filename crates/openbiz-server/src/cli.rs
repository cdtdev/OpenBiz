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
    /// Report what is above one concept in the hierarchy, and why. Reads and nothing else.
    Ancestors {
        /// The IRI of the vocabulary graph to read.
        graph: String,
        /// The IRI of the concept to walk up from.
        concept: String,
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
  openbiz ancestors <graph> <concept>
                             report what is above <concept> in the hierarchy, and why
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

Inspect only reads. It reports the concepts, concept schemes, and collections a vocabulary holds,
including the ones no statement typed — SKOS itself says a resource with concepts in it is a
concept scheme — and it names the specification statement behind every fact it inferred. It
separates a violated SKOS integrity condition, which makes a graph not a SKOS vocabulary, from
something merely ill-formed, which is our judgement and says so.

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
fn actor() -> Result<String, CommandError> {
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

    #[test]
    fn the_usage_names_every_command_it_can_parse() {
        for command in [
            "backup",
            "restore",
            "import",
            "retract",
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
        }
    }
}
