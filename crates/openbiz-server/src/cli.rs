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

use openbiz_store::{Store, StoreError, BACKUP_SYNTAX};
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
  openbiz help               show this

A backup is the whole store as N-Quads: every vocabulary and OpenBiz's own registry, in a
W3C-standard syntax any conforming tool can read. Restore refuses a store that is not empty, so
restore into a fresh data directory and point the server at that.

Both commands need the store to themselves; stop the server first.

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

        let command = match first.as_str() {
            "help" | "--help" | "-h" => return Self::no_more(Self::Help, "help", args),
            "backup" => Self::Backup {
                file: Self::one_file("backup", "write", &mut args)?,
            },
            "restore" => Self::Restore {
                file: Self::one_file("restore", "read", &mut args)?,
            },
            other => return Err(ArgsError::UnknownCommand(other.to_owned())),
        };

        let name = match command {
            Self::Backup { .. } => "backup",
            _ => "restore",
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
    /// The store refused the operation, or failed during it.
    #[error(transparent)]
    Store(#[from] StoreError),
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
    fn the_usage_names_every_command_it_can_parse() {
        for command in ["backup", "restore", "help"] {
            assert!(
                USAGE.contains(command),
                "usage does not mention {command}, so nobody can discover it"
            );
        }
    }
}
