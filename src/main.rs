use std::{
    io::{self, Write},
    process::ExitCode,
};

use anyhow::Result;
use clap::{CommandFactory, Parser};
use conda_deny::bundle::bundle;
use conda_deny::check::check;
use conda_deny::cli::{Cli, CondaDenyCliConfig};
use conda_deny::get_config_options;
use conda_deny::list::list;
use conda_deny::CondaDenyConfig;
use log::{debug, info};

fn print_completions(shell: clap_complete::Shell, stdout: &mut dyn Write) -> Result<()> {
    let mut command = Cli::command();

    if shell == clap_complete::Shell::Bash {
        command = command.name("conda_deny");
        let mut completions = Vec::new();
        clap_complete::generate(shell, &mut command, "conda_deny", &mut completions);
        let completions = String::from_utf8(completions)?;
        write!(
            stdout,
            "{}",
            completions.replace(" conda_deny\n", " conda-deny\n")
        )?;
    } else {
        clap_complete::generate(shell, &mut command, "conda-deny", stdout);
    }

    Ok(())
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    debug!("Parsed CLI config: {cli:?}");

    if let CondaDenyCliConfig::Completion { shell } = cli.command {
        return print_completions(shell, &mut io::stdout());
    }

    let config = get_config_options(cli.config, cli.command)?;

    info!("Parsed config: {config:?}");

    let stdout = io::stdout();

    match config {
        CondaDenyConfig::Check(check_config) => check(check_config, stdout),
        CondaDenyConfig::List(list_config) => list(list_config, stdout),
        CondaDenyConfig::Bundle(bundle_config) => bundle(bundle_config, stdout),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
