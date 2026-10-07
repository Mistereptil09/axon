use axon_shared::paths::{App, PathError, Paths, RawPaths};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConfig {
    #[serde(default)]
    paths: RawPaths,
}

pub struct Config {
    pub paths: Paths,
}

impl RawConfig {
    /// Resolves the raw config into a checked `Config`.
    /// # Errors
    /// The first `PathError` from any directory.
    pub fn resolve(self) -> Result<Config, PathError> {
        Ok(Config {
            paths: self.paths.resolve(App::Orchestrator)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_field_is_refused() {
        let result = toml::from_str::<RawConfig>("[path]\nstate_dir = \"/x\"");
        assert!(result.is_err());
    }
}
