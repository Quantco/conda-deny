use anyhow::{Context, Result};
use clap::Parser;
use conda_deny::cli::{Cli, CondaDenyCliConfig};
use conda_deny::conda_deny_config::{CondaDeny, CondaDenyTomlConfig};
use conda_deny::expression_utils::parse_expression;
use conda_deny::license_whitelist::build_license_whitelist;
use conda_deny::{check_license_infos, format_check_output, list, CondaDenyListConfig};
use conda_deny::{CondaDenyCheckConfig, CondaDenyConfig};
use log::{debug, info, trace};

fn get_config_options(cli: Cli) -> Result<CondaDenyConfig> {
    let toml_config = if let Some(config_path) = cli.config {
        CondaDenyTomlConfig::from_path(config_path.as_str())
            .with_context(|| format!("Failed to parse config file {}", config_path))?
    } else {
        match CondaDenyTomlConfig::from_path("pixi.toml")
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
                CondaDenyTomlConfig::from_path("pyproject.toml")
                    .context(e)
                    .with_context(|| "Failed to parse config file pyproject.toml")?
            }
        }
    };

    debug!("Parsed TOML config: {:?}", toml_config);

    let config = match cli.command {
        CondaDenyCliConfig::Check {
            lockfile,
            prefix,
            platform,
            environment,
            include_safe,
            osi,
            ignore_pypi,
        } => {
            // cli overrides toml configuration
            let lockfile = lockfile.unwrap_or(toml_config.get_lockfile_spec());
            let prefix = prefix.unwrap_or_default();
            if lockfile.is_empty() && prefix.is_empty() {
                return Err(anyhow::anyhow!("No lockfiles or conda prefixes provided"));
            }

            let platform = if platform.is_some() {
                platform
            } else {
                toml_config.get_platform_spec()
            };
            if platform.is_some() && !prefix.is_empty() {
                return Err(anyhow::anyhow!(
                    "Cannot specify platforms and conda prefixes at the same time"
                ));
            }

            let environment = if environment.is_some() {
                environment
            } else {
                toml_config.get_environment_spec()
            };
            if environment.is_some() && !prefix.is_empty() {
                return Err(anyhow::anyhow!(
                    "Cannot specify environments and conda prefixes at the same time"
                ));
            }

            let license_whitelist = toml_config.get_license_whitelists();

            // defaults to false
            let osi = if osi.is_some() {
                osi
            } else {
                toml_config.get_osi()
            }
            .unwrap_or(false);
            if osi && !license_whitelist.is_empty() {
                return Err(anyhow::anyhow!(
                    "Cannot use OSI mode and license whitelists at the same time"
                ));
            }

            if !osi && license_whitelist.is_empty() {
                return Err(anyhow::anyhow!("No license whitelist provided"));
            }

            // defaults to false
            let ignore_pypi = if ignore_pypi.is_some() {
                ignore_pypi
            } else {
                toml_config.get_ignore_pypi()
            }
            .unwrap_or(false);

            // licenses
            let license_whitelists = toml_config.get_license_whitelists();

            // todo: make prettier
            let safe_licenses_from_toml = toml_config
                .tool
                .conda_deny
                .safe_licenses
                .unwrap_or_default();
            let ignore_packages_from_toml = toml_config
                .tool
                .conda_deny
                .ignore_packages
                .unwrap_or_default();

            let license_whitelist_urls = toml_config.get_license_whitelists();
            let license_whitelists_remote = build_license_whitelist(&license_whitelist_urls)?;
            let safe_licenses = safe_licenses_from_toml
                .iter()
                .map(|license_str| parse_expression(license_str)?)
                .chain(license_whitelists_remote.safe_licenses)
                .collect::<Vec<_>>();
            let ignore_packages = ignore_packages_from_toml
                .iter()
                .chain(license_whitelists_remote.ignore_packages)
                .collect::<Vec<_>>();

            CondaDenyConfig::Check(CondaDenyCheckConfig {
                prefix,
                lockfile,
                platform,
                environment,
                include_safe,
                osi,
                ignore_pypi,
                safe_licenses: safe_licenes,
                ignore_packages,
            })
        }
        CondaDenyCliConfig::List {
            lockfile,
            prefix,
            platform,
            environment,
        } => {
            // todo: refactor with check
            // cli overrides toml configuration
            let lockfile = lockfile.unwrap_or(toml_config.get_lockfile_spec());
            let prefix = prefix.unwrap_or_default();
            if lockfile.is_empty() && prefix.is_empty() {
                return Err(anyhow::anyhow!("No lockfiles or conda prefixes provided"));
            }

            let platform = if platform.is_some() {
                platform
            } else {
                toml_config.get_platform_spec()
            };
            if platform.is_some() && !prefix.is_empty() {
                return Err(anyhow::anyhow!(
                    "Cannot specify platforms and conda prefixes at the same time"
                ));
            }

            let environment = if environment.is_some() {
                environment
            } else {
                toml_config.get_environment_spec()
            };
            if environment.is_some() && !prefix.is_empty() {
                return Err(anyhow::anyhow!(
                    "Cannot specify environments and conda prefixes at the same time"
                ));
            }

            CondaDenyConfig::List(CondaDenyListConfig {
                prefix,
                lockfile,
                platform,
                environment,
            })
        }
    };

    Ok(config)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    debug!("Parsed CLI config: {:?}", cli);

    let config = get_config_options(cli)?;

    info!("Parsed config: {:?}", config);

    match config {
        CondaDenyConfig::Check(check_config) => {
            let (safe_dependencies, unsafe_dependencies) = check_license_infos(&check_config)?;

            print!(
                "{}",
                format_check_output(
                    safe_dependencies,
                    unsafe_dependencies.clone(),
                    check_config.include_safe,
                )
            );

            if !unsafe_dependencies.is_empty() {
                std::process::exit(1);
            };
            Ok(())
        }
        CondaDenyConfig::List(list_config) => {
            list(&list_config)?;
            Ok(())
        }
    }
}
