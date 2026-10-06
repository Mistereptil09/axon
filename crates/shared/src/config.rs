use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot read config file {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid config file {path}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

/// Default function to load a config of axon
/// # Errors
/// `ConfigError::Read` if the Path cannot be read
/// `ConfigError::Parse` if the file cannot be parsed by toml
pub fn load<T: DeserializeOwned>(path: &Path) -> Result<T, ConfigError> {
    let text: String = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    let config = toml::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Sample {
        name: String,
        port: u16,
    }

    fn write_config(content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, content).unwrap();
        (dir, path)
    }

    #[test]
    fn valid_file() {
        let text: &str = "name = \"axon\"\nport = 5";
        let (_dir, path): (tempfile::TempDir, PathBuf) = write_config(text);
        let result = load::<Sample>(&path).unwrap();
        assert_eq!(
            result,
            Sample {
                name: "axon".to_string(),
                port: 5
            }
        );
    }

    #[test]
    fn missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.toml");
        assert!(matches!(
            load::<Sample>(&path),
            Err(ConfigError::Read { .. })
        ));
    }

    #[test]
    fn broken_toml_file() {
        let (_dir, path): (tempfile::TempDir, PathBuf) = write_config("name = \"axon");
        assert!(matches!(
            load::<Sample>(&path),
            Err(ConfigError::Parse { .. })
        ));
    }

    #[test]
    fn wrong_type() {
        let (_dir, path): (tempfile::TempDir, PathBuf) =
            write_config("name = \"axon\"\nport = \"not a number\"");
        assert!(matches!(
            load::<Sample>(&path),
            Err(ConfigError::Parse { .. })
        ));
    }

    #[test]
    fn unknown_field() {
        let (_dir, path): (tempfile::TempDir, PathBuf) =
            write_config("name = \"axon\"\nport = 5\nprot = 1");
        assert!(matches!(
            load::<Sample>(&path),
            Err(ConfigError::Parse { .. })
        ));
    }

    #[test]
    fn error_names_the_file() {
        let (_dir, path): (tempfile::TempDir, PathBuf) = write_config("name = \"axon");
        assert!(matches!(
            load::<Sample>(&path),
            Err(ConfigError::Parse { path: p, .. }) if p == path
        ));
    }
}
