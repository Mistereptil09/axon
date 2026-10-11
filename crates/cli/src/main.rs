use axon_cli::run::run;
use axon_shared::report::report;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    /// Path to the config file (default: `~/.config/axon/cli/config.toml`)
    #[arg(long)]
    config: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();
    if let Err(error) = run(args.config) {
        report(error.as_ref());
        std::process::exit(1);
    }
}
