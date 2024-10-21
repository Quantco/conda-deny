use anyhow::{Context, Result};
use clap::Parser;
use conda_deny::cli::{Cli, Commands};
use conda_deny::conda_deny_config::CondaDenyConfig;
use conda_deny::{check_license_infos, format_check_output, list};
use log::{debug, info};

fn main() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    let config = if let Some(config_path) = cli.config {
        CondaDenyConfig::from_path(config_path.as_str())
            .with_context(|| format!("Failed to parse config file {}", config_path))?
    } else {
        match CondaDenyConfig::from_path("pixi.toml")
            .with_context(|| "Failed to parse config file pixi.toml")
        {
            Ok(config) => {
                debug!("Successfully loaded config from pixi.toml");
                config
            }
            Err(e) => {
                debug!(
                    "Error parsing config file: pixi.toml: {}. Attempting to use pyproject.toml instead...",
                    e
                );
                CondaDenyConfig::from_path("pyproject.toml")
                    .context(e)
                    .with_context(|| "Failed to parse config file pyproject.toml")?
            }
        }
    };

    let conda_prefixes = cli.prefix.unwrap_or_default();

    match cli.command {
        Commands::Check {
            include_safe,
            osi,
            lockfile,
            platform,
            environment,
        } => {
            let cli_lockfiles = lockfile.unwrap_or_default();
            let cli_platforms = platform.unwrap_or_default();
            let cli_environment = environment.unwrap_or_default();

            debug!("Check command called.");
            debug!("Checking platforms: {:?}", cli_platforms);
            debug!("Checking environments: {:?}", cli_environment);
            debug!("Checking conda prefixes: {:?}", conda_prefixes);
            let mut locks_to_check = cli_lockfiles.clone();
            locks_to_check.push("pixi.lock".to_string());
            debug!("Checking pixi lockfiles: {:?}", locks_to_check);

            if include_safe {
                info!("Including safe dependencies in output");
            }

            let check_input = (
                &config,
                cli_lockfiles,
                cli_platforms,
                cli_environment,
                conda_prefixes,
                osi,
            );

            let check_output = check_license_infos(check_input)?;

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
            list(&config)
        }
    }
}
