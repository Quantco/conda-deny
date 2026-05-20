use std::{
    io::{self, Write},
    process::ExitCode,
};

use anyhow::Result;
use clap::{CommandFactory, Parser};
use clap_complete::{
    env::{Bash, Elvish, EnvCompleter, Fish, Powershell, Zsh},
    CompleteEnv,
};
use conda_deny::bundle::bundle;
use conda_deny::check::check;
use conda_deny::cli::{Cli, CondaDenyCliConfig};
use conda_deny::get_config_options;
use conda_deny::list::list;
use conda_deny::CondaDenyConfig;
use log::{debug, info};

fn print_completions(shell: clap_complete::Shell, stdout: &mut dyn Write) -> Result<()> {
    let command = Cli::command();
    // We are using the (unstable) Rust-Native completion engine.
    // It addresses some bugs that would otherwise need to be fixed here
    // (see https://github.com/clap-rs/clap/issues/3166)
    let completer: &dyn EnvCompleter = match shell {
        clap_complete::Shell::Bash => &Bash,
        clap_complete::Shell::Elvish => &Elvish,
        clap_complete::Shell::Fish => &Fish,
        clap_complete::Shell::PowerShell => &Powershell,
        clap_complete::Shell::Zsh => &Zsh,
        _ => unreachable!("clap_complete::Shell is non-exhaustive"),
    };

    completer.write_registration(
        "COMPLETE",
        command.get_name(),
        "conda-deny",
        "conda-deny",
        stdout,
    )?;
    Ok(())
}

fn run() -> Result<()> {
    CompleteEnv::with_factory(Cli::command)
        .bin("conda-deny")
        .complete();

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
