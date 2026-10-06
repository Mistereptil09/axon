use axon_shared::paths::PathsConfig;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeskConfig {
    #[serde(default)]
    pub paths: PathsConfig,
}
