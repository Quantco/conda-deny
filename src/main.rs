use anyhow::Result;
use clap::Parser;
use conda_deny::cli::{Cli, Commands};
use conda_deny::{bundle, check_license_infos, format_check_output, list, parse_cli_input};

use log::debug;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let conda_deny_input = parse_cli_input(&cli)?;

    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    match cli.command {
        Commands::Check {
            include_safe,
            osi: _,
        } => {
            debug!("Check command called.");

            if include_safe {
                debug!("Including safe dependencies in output");
            }

            let check_output = check_license_infos(conda_deny_input)?;

            let (safe_dependencies, unsafe_dependencies) = check_output;

            print!(
                "{}",
                format_check_output(safe_dependencies, unsafe_dependencies.clone(), include_safe,)
            );

            if !unsafe_dependencies.is_empty() {
                std::process::exit(1);
            };
            Ok(())
        }
        Commands::List {} => {
            debug!("List command called");
            list(conda_deny_input)?;
            Ok(())
        }
        Commands::Bundle { output } => {
            debug!("Bundle command called");
            bundle(conda_deny_input, output.clone())?;
            Ok(())
        }
    }
}
