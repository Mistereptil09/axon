use axon_desk::config::{Config, RawConfig};
use axon_shared::config::load;
use axon_shared::paths::{self, App};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    /// Path to the config file (default: `~/.config/axon/desk/config.toml`)
    #[arg(long)]
    config: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    let config_file = match paths::config_file(App::Desk, args.config) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    };

    let raw_config: RawConfig = match load(&config_file) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    };

    let _config: Config = match raw_config.resolve() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    };
}
