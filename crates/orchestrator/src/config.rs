use axon_shared::paths::PathsConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestratorConfig {
    #[serde(default)]
    pub paths: PathsConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_field_is_refused() {
        let result = toml::from_str::<OrchestratorConfig>("[path]\nstate_dir = \"/x\"");
        assert!(result.is_err());
    }
}
