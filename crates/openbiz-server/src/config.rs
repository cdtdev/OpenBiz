//! Configuration: built-in defaults, a TOML file, the environment — and the **provenance** of
//! every effective value.
//!
//! Precedence, lowest to highest: default → file → environment. The environment wins because it is
//! what a container runtime, a systemd unit, or a one-off `OPENBIZ_BIND=… ./openbiz` can reach
//! without editing a file on disk.
//!
//! Two deliberate choices distinguish this from how the incumbents handle configuration, where a
//! deployment is spread across an app-server descriptor, a properties file, and a triplestore
//! connection string, and a key you misspell is silently ignored:
//!
//! 1. **An unrecognised key is an error, not a shrug.** `deny_unknown_fields` plus TOML's spans
//!    turn `bnd = "0.0.0.0:80"` into a message naming the line and the keys we do accept. The
//!    failure mode we refuse to have is "I set it and nothing happened".
//! 2. **Every value knows where it came from.** A [`Setting`] carries its [`Source`], so the
//!    startup log says which of the three layers won, and a failure to bind can name the file or
//!    the variable the operator must edit rather than just the address that did not work. This is
//!    the configuration-scale form of the explainability commitment in `CLAUDE.md` §3.
//!
//! Zero configuration remains a working configuration: with no file and no environment, the
//! defaults bind loopback. A missing *default-path* file is normal; a missing file that
//! `OPENBIZ_CONFIG` explicitly named is an error, because an explicit request must never
//! silently degrade to the defaults.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Environment variable naming the configuration file to read.
pub const ENV_CONFIG: &str = "OPENBIZ_CONFIG";
/// Environment variable overriding [`Config::bind`].
pub const ENV_BIND: &str = "OPENBIZ_BIND";
/// Environment variable overriding [`Config::data_dir`].
pub const ENV_DATA_DIR: &str = "OPENBIZ_DATA_DIR";

/// The file consulted when [`ENV_CONFIG`] is unset. Its absence is not an error.
pub const DEFAULT_CONFIG_FILE: &str = "openbiz.toml";

/// Where an effective configuration value came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Nothing set it; it is the built-in default.
    Default,
    /// A configuration file, at this path.
    File(PathBuf),
    /// An environment variable, with this name.
    Env(&'static str),
}

impl fmt::Display for Source {
    /// Phrased to complete the sentence "…, from {source}", because that is how it is read in a
    /// log line or an error message.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Default => f.write_str("the built-in default"),
            Source::File(path) => write!(f, "{}", path.display()),
            Source::Env(key) => write!(f, "${key}"),
        }
    }
}

/// A configuration value together with the reason it has that value.
///
/// [`Deref`](std::ops::Deref) and [`Display`](fmt::Display) forward to the value, so a caller that
/// does not care about provenance reads exactly as it would with a bare field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting<T> {
    value: T,
    source: Source,
}

impl<T> Setting<T> {
    fn new(value: T, source: Source) -> Self {
        Self { value, source }
    }

    /// The effective value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Which layer supplied [`Setting::value`].
    pub fn source(&self) -> &Source {
        &self.source
    }
}

impl<T> std::ops::Deref for Setting<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: fmt::Display> fmt::Display for Setting<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

/// Server configuration.
///
/// Deliberately minimal: a self-hosted product must start with no configuration at all, and every
/// required setting is one more step between download and a running server.
#[derive(Debug, Clone)]
pub struct Config {
    /// Address to bind, e.g. `127.0.0.1:8080`.
    pub bind: Setting<String>,
    /// Directory holding the RDF store and backups.
    pub data_dir: Setting<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: Setting::new("127.0.0.1:8080".to_owned(), Source::Default),
            data_dir: Setting::new("./data".to_owned(), Source::Default),
        }
    }
}

/// Everything that can go wrong turning the three layers into a [`Config`].
///
/// Each variant names the thing an operator must go and edit. That is the whole point of the
/// [`Source`] carried through: an error that says "invalid address" is a puzzle, and one that says
/// "invalid address, from $OPENBIZ_BIND" is an instruction.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// `OPENBIZ_CONFIG` named a file that is not there. Never falls back to the defaults.
    #[error("OPENBIZ_CONFIG names {path}, but no file exists there")]
    NoSuchFile { path: PathBuf },

    /// The file exists but could not be read — permissions, a directory, a broken symlink.
    #[error("the configuration file {path} could not be read")]
    Unreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Malformed TOML, or a key we do not recognise. The inner error carries the line and column.
    #[error("{path} is not a valid OpenBiz configuration file")]
    Invalid {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    /// A value was supplied but is blank. Silently substituting the default here would be the
    /// same "I set it and nothing happened" failure that `deny_unknown_fields` exists to prevent.
    #[error("{key} is set to a blank value, from {origin}; remove it to use the default instead")]
    Blank { key: &'static str, origin: Source },
}

/// The file layer, before merging. Every field is optional: a file may set one key and leave the
/// rest to the defaults.
///
/// `deny_unknown_fields` is the load-bearing attribute — it is what makes a typo loud.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    bind: Option<String>,
    data_dir: Option<String>,
}

/// How the environment is read. Injected so the merge logic is testable without mutating
/// process-global state, which is shared across the test binary's threads and would make these
/// tests flaky against each other.
type EnvLookup<'a> = &'a dyn Fn(&str) -> Option<String>;

impl Config {
    /// Load configuration from the defaults, the configuration file, and the environment.
    ///
    /// This is the production entry point; [`Config::resolve`] holds the logic so tests can drive
    /// it with an explicit environment.
    pub fn load() -> Result<Self, ConfigError> {
        Self::resolve(
            &|key| std::env::var(key).ok(),
            Path::new(DEFAULT_CONFIG_FILE),
        )
    }

    /// The settings in a stable order, for logging and for display.
    ///
    /// Returns the key as it is written *in a configuration file*; the [`Source`] already names the
    /// environment variable when that is what won.
    pub fn settings(&self) -> [(&'static str, &Setting<String>); 2] {
        [("bind", &self.bind), ("data_dir", &self.data_dir)]
    }

    fn resolve(env: EnvLookup, default_path: &Path) -> Result<Self, ConfigError> {
        let (path, explicit) = match env(ENV_CONFIG) {
            Some(named) if named.trim().is_empty() => {
                return Err(ConfigError::Blank {
                    key: ENV_CONFIG,
                    origin: Source::Env(ENV_CONFIG),
                })
            }
            Some(named) => (PathBuf::from(named), true),
            None => (default_path.to_owned(), false),
        };

        let mut config = Self::default();

        if let Some(file) = read_file(&path, explicit)? {
            if let Some(bind) = file.bind {
                config.bind = Setting::new(bind, Source::File(path.clone()));
            }
            if let Some(data_dir) = file.data_dir {
                config.data_dir = Setting::new(data_dir, Source::File(path));
            }
        }

        if let Some(bind) = env(ENV_BIND) {
            config.bind = Setting::new(bind, Source::Env(ENV_BIND));
        }
        if let Some(data_dir) = env(ENV_DATA_DIR) {
            config.data_dir = Setting::new(data_dir, Source::Env(ENV_DATA_DIR));
        }

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        for (key, setting) in self.settings() {
            if setting.value.trim().is_empty() {
                return Err(ConfigError::Blank {
                    // Name the thing the operator has to go and edit: the variable if the
                    // environment won, otherwise the file key.
                    key: match setting.source {
                        Source::Env(variable) => variable,
                        _ => key,
                    },
                    origin: setting.source.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Read and parse the configuration file, if there is one to read.
///
/// `explicit` distinguishes the two absences: a missing `openbiz.toml` in the working directory is
/// the ordinary zero-configuration case, while a missing file that `OPENBIZ_CONFIG` named is an
/// operator error we must not paper over.
fn read_file(path: &Path, explicit: bool) -> Result<Option<FileConfig>, ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return if explicit {
                Err(ConfigError::NoSuchFile {
                    path: path.to_owned(),
                })
            } else {
                Ok(None)
            };
        }
        Err(source) => {
            return Err(ConfigError::Unreadable {
                path: path.to_owned(),
                source,
            })
        }
    };

    toml::from_str(&text)
        .map(Some)
        .map_err(|source| ConfigError::Invalid {
            path: path.to_owned(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// An environment with nothing in it.
    fn empty_env() -> impl Fn(&str) -> Option<String> {
        |_| None
    }

    /// An environment holding exactly these variables.
    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |key| map.get(key).cloned()
    }

    /// Write `contents` to a uniquely-named file in a temporary directory and return both, so the
    /// directory outlives the test body and is cleaned up on drop.
    fn config_file(contents: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("openbiz.toml");
        std::fs::write(&path, contents).expect("write config file");
        (dir, path)
    }

    /// A path inside a real temporary directory that deliberately has no file at it.
    fn absent_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("openbiz.toml");
        (dir, path)
    }

    #[test]
    fn config_defaults_to_loopback() {
        // A self-hosted server must not default to a public interface.
        assert!(Config::default().bind.starts_with("127.0.0.1"));
    }

    #[test]
    fn no_file_and_no_environment_yields_the_defaults() {
        let (_dir, absent) = absent_path();

        let config = Config::resolve(&empty_env(), &absent).expect("zero configuration must work");

        assert_eq!(config.bind.value(), "127.0.0.1:8080");
        assert_eq!(config.data_dir.value(), "./data");
        assert_eq!(config.bind.source(), &Source::Default);
        assert_eq!(config.data_dir.source(), &Source::Default);
    }

    #[test]
    fn a_file_overrides_the_defaults_and_records_its_path() {
        let (_dir, path) = config_file("bind = \"0.0.0.0:9000\"\ndata_dir = \"/srv/openbiz\"\n");

        let config = Config::resolve(&empty_env(), &path).expect("valid file");

        assert_eq!(config.bind.value(), "0.0.0.0:9000");
        assert_eq!(config.data_dir.value(), "/srv/openbiz");
        assert_eq!(config.bind.source(), &Source::File(path.clone()));
        assert_eq!(config.data_dir.source(), &Source::File(path));
    }

    #[test]
    fn a_partial_file_leaves_the_rest_at_their_defaults() {
        let (_dir, path) = config_file("bind = \"0.0.0.0:9000\"\n");

        let config = Config::resolve(&empty_env(), &path).expect("valid file");

        assert_eq!(config.bind.value(), "0.0.0.0:9000");
        assert_eq!(config.data_dir.value(), "./data");
        // The provenance must stay per-setting: one key in a file does not make the whole
        // configuration "from the file".
        assert_eq!(config.data_dir.source(), &Source::Default);
    }

    #[test]
    fn the_environment_beats_the_file() {
        let (_dir, path) = config_file("bind = \"0.0.0.0:9000\"\ndata_dir = \"/srv/openbiz\"\n");
        let env = env_of(&[
            (ENV_CONFIG, path.to_str().expect("utf-8 path")),
            (ENV_BIND, "127.0.0.1:1234"),
        ]);

        let config = Config::resolve(&env, Path::new("unused")).expect("valid file");

        assert_eq!(config.bind.value(), "127.0.0.1:1234");
        assert_eq!(config.bind.source(), &Source::Env(ENV_BIND));
        // …and the key the environment did *not* override still comes from the file.
        assert_eq!(config.data_dir.value(), "/srv/openbiz");
        assert_eq!(config.data_dir.source(), &Source::File(path));
    }

    #[test]
    fn openbiz_config_selects_the_file_to_read() {
        let (_dir, path) = config_file("data_dir = \"/var/lib/openbiz\"\n");
        let env = env_of(&[(ENV_CONFIG, path.to_str().expect("utf-8 path"))]);

        // The default path is a file that does exist and says something different: proving
        // OPENBIZ_CONFIG is genuinely consulted rather than merely tolerated.
        let (_other, decoy) = config_file("data_dir = \"/wrong\"\n");
        let config = Config::resolve(&env, &decoy).expect("valid file");

        assert_eq!(config.data_dir.value(), "/var/lib/openbiz");
    }

    #[test]
    fn a_missing_default_file_is_not_an_error() {
        let (_dir, absent) = absent_path();

        let config = Config::resolve(&empty_env(), &absent);

        assert!(config.is_ok(), "zero configuration must remain valid");
    }

    #[test]
    fn a_missing_explicitly_named_file_is_an_error() {
        let (_dir, absent) = absent_path();
        let env = env_of(&[(ENV_CONFIG, absent.to_str().expect("utf-8 path"))]);

        let error = Config::resolve(&env, Path::new("unused")).expect_err("must not fall back");

        assert!(
            matches!(error, ConfigError::NoSuchFile { .. }),
            "an explicitly named file that is absent must not silently degrade to the defaults, \
             got {error:?}"
        );
    }

    #[test]
    fn an_unknown_key_is_rejected_and_names_the_keys_we_accept() {
        // The failure this exists to prevent: a typo that is silently ignored, leaving the
        // operator certain they configured something they did not.
        let (_dir, path) = config_file("bnd = \"0.0.0.0:9000\"\n");

        let error = Config::resolve(&empty_env(), &path).expect_err("typo must be loud");

        let ConfigError::Invalid { source, .. } = &error else {
            panic!("expected an Invalid error, got {error:?}");
        };
        let message = source.to_string();
        assert!(
            message.contains("bnd"),
            "must quote the offending key: {message}"
        );
        assert!(
            message.contains("bind") && message.contains("data_dir"),
            "must name the keys we do accept: {message}"
        );
        assert!(
            message.contains("line 1"),
            "must locate the offending key: {message}"
        );
    }

    #[test]
    fn malformed_toml_is_rejected_with_its_position() {
        let (_dir, path) = config_file("bind = \n");

        let error = Config::resolve(&empty_env(), &path).expect_err("malformed file must fail");

        let ConfigError::Invalid { source, .. } = &error else {
            panic!("expected an Invalid error, got {error:?}");
        };
        assert!(
            source.to_string().contains("line 1"),
            "must locate the problem: {source}"
        );
    }

    #[test]
    fn a_wrongly_typed_value_is_rejected() {
        let (_dir, path) = config_file("bind = 8080\n");

        let error = Config::resolve(&empty_env(), &path).expect_err("a port is not an address");

        assert!(
            matches!(error, ConfigError::Invalid { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn a_blank_environment_value_is_an_error_naming_the_variable() {
        // Empty-but-set is what an unset shell variable, a systemd `Environment=` line, or a
        // docker-compose interpolation collapses to. Falling back to the default here would be a
        // silent ignore; naming it is the whole point of tracking provenance.
        let (_dir, absent) = absent_path();
        let env = env_of(&[(ENV_BIND, "   ")]);

        let error = Config::resolve(&env, &absent).expect_err("blank must not be accepted");

        let ConfigError::Blank { key, origin } = &error else {
            panic!("expected a Blank error, got {error:?}");
        };
        assert_eq!(*key, ENV_BIND);
        assert_eq!(origin, &Source::Env(ENV_BIND));
        assert!(
            error.to_string().contains("$OPENBIZ_BIND"),
            "must point at the variable: {error}"
        );
    }

    #[test]
    fn a_blank_file_value_is_an_error_naming_the_file() {
        let (_dir, path) = config_file("data_dir = \"\"\n");

        let error = Config::resolve(&empty_env(), &path).expect_err("blank must not be accepted");

        let ConfigError::Blank { key, origin } = &error else {
            panic!("expected a Blank error, got {error:?}");
        };
        assert_eq!(*key, "data_dir");
        assert_eq!(origin, &Source::File(path.clone()));
        assert!(
            error.to_string().contains(&path.display().to_string()),
            "must point at the file to edit: {error}"
        );
    }

    #[test]
    fn a_blank_openbiz_config_is_an_error_rather_than_a_missing_file() {
        let (_dir, absent) = absent_path();
        let env = env_of(&[(ENV_CONFIG, "")]);

        let error = Config::resolve(&env, &absent).expect_err("blank must not be accepted");

        assert!(
            matches!(error, ConfigError::Blank { key, .. } if key == ENV_CONFIG),
            "got {error:?}"
        );
    }

    #[test]
    fn comments_and_blank_lines_are_accepted() {
        // The reason the format is TOML and not JSON: a deployment's configuration is where
        // operators leave notes for the next operator.
        let (_dir, path) = config_file("# why we bind wide\nbind = \"0.0.0.0:9000\"\n\n");

        let config = Config::resolve(&empty_env(), &path).expect("comments are legal");

        assert_eq!(config.bind.value(), "0.0.0.0:9000");
    }

    #[test]
    fn settings_are_reported_with_their_sources() {
        let (_dir, path) = config_file("bind = \"0.0.0.0:9000\"\n");

        let config = Config::resolve(&empty_env(), &path).expect("valid file");
        let reported: Vec<_> = config
            .settings()
            .into_iter()
            .map(|(key, setting)| (key, setting.value().clone(), setting.source().clone()))
            .collect();

        assert_eq!(
            reported,
            vec![
                (
                    "bind",
                    "0.0.0.0:9000".to_owned(),
                    Source::File(path.clone())
                ),
                ("data_dir", "./data".to_owned(), Source::Default),
            ],
            "the startup log must be able to show every setting and where it came from"
        );
    }

    #[test]
    fn a_source_reads_as_the_place_to_go_and_edit() {
        assert_eq!(Source::Default.to_string(), "the built-in default");
        assert_eq!(Source::Env(ENV_BIND).to_string(), "$OPENBIZ_BIND");
        assert_eq!(
            Source::File(PathBuf::from("/etc/openbiz.toml")).to_string(),
            "/etc/openbiz.toml"
        );
    }

    #[test]
    fn a_setting_reads_as_its_value() {
        // Deref and Display forward, so provenance costs call sites nothing.
        let setting = Setting::new("127.0.0.1:8080".to_owned(), Source::Default);

        assert_eq!(setting.to_string(), "127.0.0.1:8080");
        assert!(setting.starts_with("127."));
    }
}
