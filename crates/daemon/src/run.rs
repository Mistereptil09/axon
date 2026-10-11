use crate::config::{Config, RawConfig};
use axon_shared::config::load;
use axon_shared::paths::{self, App};
use std::error::Error;
use std::path::PathBuf;

/// Loads and checks the config, then (later) starts the daemon.
/// # Errors
/// Any error that stops the daemon from starting.
pub fn run(config_override: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
    let config_file = paths::config_file(App::Daemon, config_override)?;
    let raw_config: RawConfig = load(&config_file)?;
    let _config: Config = raw_config.resolve()?;
    Ok(())
}
