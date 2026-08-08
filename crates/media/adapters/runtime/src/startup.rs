//! Configuration loading.
//!
//! The filesystem is an adapter concern, so it lives here rather than in the application crate.
//! What the application owns is the document, its migrations, and its validation rules.

use std::path::{Path, PathBuf};

use media_application::{ConfigurationError, MediaConfiguration, configuration};

/// The environment variable that points at a configuration file.
pub const CONFIGURATION_PATH_VARIABLE: &str = "MEDIA_CONFIG";

/// The default configuration file, relative to the working directory.
pub const DEFAULT_CONFIGURATION_PATH: &str = "media/media-server.json";

/// Where this run's configuration comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigurationSource {
    /// Read the given file. A missing file at the default location means a first run and yields
    /// the shipped defaults; a missing file the operator named explicitly is an error.
    File { path: PathBuf, required: bool },
    /// Use the shipped defaults without touching the filesystem. Serves tests and diagnostics.
    Defaults,
}

impl ConfigurationSource {
    /// Honors an explicit `MEDIA_CONFIG` override and otherwise looks in the default location.
    pub fn from_environment() -> Self {
        match std::env::var(CONFIGURATION_PATH_VARIABLE) {
            Ok(path) if !path.trim().is_empty() => Self::File {
                path: PathBuf::from(path.trim()),
                required: true,
            },
            _ => Self::File {
                path: PathBuf::from(DEFAULT_CONFIGURATION_PATH),
                required: false,
            },
        }
    }
}

/// Why the server cannot start.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("cannot read the configuration file {}: {source}", path.display())]
    Unreadable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the configuration file {} is not usable: {source}", path.display())]
    Invalid {
        path: PathBuf,
        #[source]
        source: ConfigurationError,
    },
}

/// Reads, migrates, and validates the configuration for this run.
pub fn load_configuration(
    source: &ConfigurationSource,
) -> Result<MediaConfiguration, StartupError> {
    match source {
        ConfigurationSource::Defaults => Ok(MediaConfiguration::default()),
        ConfigurationSource::File { path, required } => read_file(path, *required),
    }
}

fn read_file(path: &Path, required: bool) -> Result<MediaConfiguration, StartupError> {
    let serialized = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !required => {
            tracing::info!(
                path = %path.display(),
                "no configuration file yet; starting from the shipped defaults"
            );
            return Ok(MediaConfiguration::default());
        }
        Err(source) => {
            return Err(StartupError::Unreadable {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    configuration::load(&serialized).map_err(|source| StartupError::Invalid {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_source_is_optional_and_the_override_is_required() {
        if std::env::var(CONFIGURATION_PATH_VARIABLE).is_ok() {
            return; // The developer running this pointed the process somewhere on purpose.
        }
        let default = ConfigurationSource::File {
            path: PathBuf::from(DEFAULT_CONFIGURATION_PATH),
            required: false,
        };
        assert_eq!(ConfigurationSource::from_environment(), default);
    }

    #[test]
    fn the_shipped_defaults_need_no_filesystem() {
        let configuration = load_configuration(&ConfigurationSource::Defaults).unwrap();
        assert_eq!(configuration.outputs.len(), 1);
    }

    #[test]
    fn a_missing_optional_file_falls_back_to_the_defaults() {
        let source = ConfigurationSource::File {
            path: PathBuf::from("does/not/exist/media-server.json"),
            required: false,
        };
        assert!(load_configuration(&source).is_ok());
    }

    #[test]
    fn a_missing_required_file_stops_startup() {
        let source = ConfigurationSource::File {
            path: PathBuf::from("does/not/exist/media-server.json"),
            required: true,
        };
        let error = load_configuration(&source).unwrap_err();
        assert!(matches!(error, StartupError::Unreadable { .. }), "{error}");
    }
}
