use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;
use conda_deny::cli::{Cli, Commands};
use conda_deny::conda_deny_config::CondaDenyConfig;
use conda_deny::{check_license_infos, format_check_output, list};
use log::debug;

fn main() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    let osi = match cli.command {
        Commands::Check { osi, .. } => osi,
        _ => false,
    };

    let mut config = CondaDenyConfig::empty();

    if !osi {
        config = if let Some(config_path) = cli.config {
            CondaDenyConfig::from_path(config_path.as_str())
                .context(format!("Failed to parse config file {}", config_path))?
        } else {
            match CondaDenyConfig::from_path("pixi.toml")
                .context("Failed to parse config file pixi.toml")
            {
                Ok(config) => {
                    debug!("Successfully loaded config from pixi.toml");
                    config
                }
                Err(pixi_err) => {
                    debug!(
                        "Error parsing pixi.toml: {}. Attempting pyproject.toml...",
                        pixi_err
                    );
                    for cause in pixi_err.chain() {
                        debug!("Caused by: {}", cause);
                    }

                    CondaDenyConfig::from_path("pyproject.toml")
                        .context("Failed to parse config file pyproject.toml")
                        .map_err(|pyproject_err| {
                            anyhow::anyhow!(
                                "Failed to parse both pixi.toml and pyproject.toml:\n  pixi.toml: {}\n  pyproject.toml: {}",
                                pixi_err,
                                pyproject_err
                            )
                        })?
                }
            }
        };
    } else {
        debug!("Skipping config file parsing for OSI compliance check. Your {} section will be ignored.", "[tool.conda-deny]".yellow());
    }

    let conda_prefixes = cli.prefix.unwrap_or_default();
    let cli_lockfiles = cli.lockfile.unwrap_or_default();
    let cli_platforms = cli.platform.unwrap_or_default();
    let cli_environments = cli.environment.unwrap_or_default();

    let cli_input = (
        &config,
        &cli_lockfiles,
        &cli_platforms,
        &cli_environments,
        &conda_prefixes,
        osi,
    );

    debug!("CLI input for platforms: {:?}", cli_platforms);
    debug!("CLI input for environments: {:?}", cli_environments);
    debug!("CLI input for conda prefixes: {:?}", conda_prefixes);
    let mut locks_to_check = cli_lockfiles.clone();
    locks_to_check.push("pixi.lock".to_string());
    debug!("CLI input for pixi lockfiles: {:?}", locks_to_check);
    debug!("CLI input for OSI compliance: {}", osi);

    match cli.command {
        Commands::Check {
            include_safe,
            osi: _,
        } => {
            debug!("Check command called.");

            if include_safe {
                debug!("Including safe dependencies in output");
            }

            let check_output = check_license_infos(cli_input)?;

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
            list(cli_input)?;
            Ok(())
        }
    }
}
