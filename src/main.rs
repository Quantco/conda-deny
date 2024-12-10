use std::io;

use anyhow::Result;
use clap::Parser;
use conda_deny::cli::Cli;
use conda_deny::CondaDenyConfig;
use conda_deny::{check, get_config_options, list};
use log::{debug, info};

fn main() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    debug!("Parsed CLI config: {:?}", cli);

    let config = get_config_options(cli.config, cli.command)?;

    info!("Parsed config: {:?}", config);

    let stdout = io::stdout();

    match config {
        CondaDenyConfig::Check(check_config) => check(check_config, stdout),
        CondaDenyConfig::List(list_config) => list(&list_config, stdout),
    }
}
