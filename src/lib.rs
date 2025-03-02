pub mod check;
pub mod cli;
pub mod conda_deny_config;
mod conda_meta_entry;
mod conda_meta_package;
pub mod expression_utils;
mod license_info;
pub mod license_whitelist;
pub mod list;
mod pixi_lock;

use std::{env, path::PathBuf};

use cli::CondaDenyCliConfig;
use conda_deny_config::CondaDenyTomlConfig;
use license_info::LicenseInfo;
use license_whitelist::{get_license_information_from_toml_config, IgnorePackage};

use anyhow::{Context, Result};
use log::debug;
use rattler_conda_types::Platform;
use serde::Deserialize;
use spdx::Expression;

use crate::license_info::LicenseInfos;

#[derive(Debug)]
pub enum CondaDenyConfig {
    Check(CondaDenyCheckConfig),
    List(CondaDenyListConfig),
}

#[derive(Debug, Clone, clap::ValueEnum, Default, Deserialize, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    #[default]
    Default,
    Json,
    JsonPretty,
    Csv,
}

/// Configuration for the check command
#[derive(Debug)]
pub struct CondaDenyCheckConfig {
    pub lockfile_or_prefix: LockfileOrPrefix,
    pub osi: bool,
    pub safe_licenses: Vec<Expression>,
    pub ignore_packages: Vec<IgnorePackage>,
    pub output_format: OutputFormat,
}

/// Shared configuration between check and list commands
#[derive(Debug)]
pub struct CondaDenyListConfig {
    pub lockfile_or_prefix: LockfileOrPrefix,
    pub output_format: OutputFormat,
}

#[derive(Debug, Clone)]
pub struct LockfileSpec {
    lockfiles: Vec<PathBuf>,
    platforms: Option<Vec<Platform>>,
    environments: Option<Vec<String>>,
    ignore_pypi: bool,
}

#[derive(Debug, Clone)]
pub enum LockfileOrPrefix {
    Lockfile(LockfileSpec),
    Prefix(Vec<PathBuf>),
}

pub type CheckOutput = (Vec<LicenseInfo>, Vec<LicenseInfo>);

pub fn fetch_license_infos(lockfile_or_prefix: LockfileOrPrefix) -> Result<LicenseInfos> {
    match lockfile_or_prefix {
        LockfileOrPrefix::Lockfile(lockfile_spec) => {
            LicenseInfos::from_pixi_lockfiles(lockfile_spec)
                .with_context(|| "Getting license information from config file failed.")
        }
        LockfileOrPrefix::Prefix(prefixes) => LicenseInfos::from_conda_prefixes(&prefixes)
            .with_context(|| "Getting license information from conda prefixes failed."),
    }
}

const IGNORE_PYPI_DEFAULT: bool = false;

fn get_lockfile_or_prefix(
    cli_config: CondaDenyCliConfig,
    toml_config: CondaDenyTomlConfig,
) -> Result<LockfileOrPrefix> {
    // cli overrides toml configuration

    Ok(LockfileOrPrefix::Prefix(vec!["adwd".into()]))
    // if lockfile.is_empty() && prefix.is_empty() {
    //     // test if pixi.lock exists next to config file, otherwise error
    //     let default_lockfile_path = env::current_dir()?.join("pixi.lock");
    //     if !default_lockfile_path.is_file() {
    //         Err(anyhow::anyhow!("No lockfiles or conda prefixes provided"))
    //     } else {
    //         Ok(LockfileOrPrefix::Lockfile(LockfileSpec {
    //             lockfiles: vec![default_lockfile_path],
    //             platforms,
    //             environments,
    //             ignore_pypi: ignore_pypi.unwrap_or(IGNORE_PYPI_DEFAULT),
    //         }))
    //     }
    // } else if !lockfile.is_empty() && !prefix.is_empty() {
    //     // TODO: Specified prefixes override lockfiles
    //     Err(anyhow::anyhow!(
    //         "Both lockfiles and conda prefixes provided. Please only provide either or."
    //     ))
    // } else if !lockfile.is_empty() {
    //     Ok(LockfileOrPrefix::Lockfile(LockfileSpec {
    //         lockfiles: lockfile.iter().map(|s| s.into()).collect(),
    //         platforms,
    //         environments,
    //         ignore_pypi: ignore_pypi.unwrap_or(IGNORE_PYPI_DEFAULT),
    //     }))
    // } else {
    //     assert!(!prefix.is_empty());

    //     if platforms.is_some() {
    //         Err(anyhow::anyhow!(
    //             "Cannot specify platforms and conda prefixes at the same time"
    //         ))
    //     } else if environments.is_some() {
    //         Err(anyhow::anyhow!(
    //             "Cannot specify pixi environments and conda prefixes at the same time"
    //         ))
    //     } else if ignore_pypi.is_some() {
    //         Err(anyhow::anyhow!(
    //             "Cannot specify ignore-pypi and conda prefixes at the same time"
    //         ))
    //     } else {
    //         Ok(LockfileOrPrefix::Prefix(
    //             prefix.iter().map(|s| s.into()).collect(),
    //         ))
    //     }
    // }
}

pub fn get_config_options(
    config: Option<PathBuf>,
    cli_config: CondaDenyCliConfig,
) -> Result<CondaDenyConfig> {
    // if config provided, use config
    // else, try to load pixi.toml, then pyproject.toml and if nothing helps, use empty config
    let toml_config = if let Some(config_path) = config {
        CondaDenyTomlConfig::from_path(config_path.clone())
            .with_context(|| format!("Failed to parse config file {:?}", config_path))?
    } else {
        match CondaDenyTomlConfig::from_path("pixi.toml".into())
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
                match CondaDenyTomlConfig::from_path("pyproject.toml".into())
                    .context(e)
                    .with_context(|| "Failed to parse config file pyproject.toml")
                {
                    Ok(config) => config,
                    Err(e) => {
                        debug!(
                            "Error parsing config file: pyproject.toml: {}. Using empty config instead...",
                            e
                        );
                        CondaDenyTomlConfig::empty()
                    }
                }
            }
        }
    };

    debug!("Parsed TOML config: {:?}", toml_config);

    // cli overrides toml configuration
    // let lockfile = cli_config
    //     .lockfile()
    //     .unwrap_or(toml_config.get_lockfile_spec());
    // let prefix = cli_config.prefix().unwrap_or_default();

    // let platforms = if cli_config.platform().is_some() {
    //     cli_config.platform()
    // } else {
    //     toml_config.get_platform_spec()
    // };
    // if platforms.is_some() && !prefix.is_empty() {
    //     return Err(anyhow::anyhow!(
    //         "Cannot specify platforms and conda prefixes at the same time"
    //     ));
    // }

    // let environments = if cli_config.environment().is_some() {
    //     cli_config.environment()
    // } else {
    //     toml_config.get_environment_spec()
    // };
    // if environments.is_some() && !prefix.is_empty() {
    //     return Err(anyhow::anyhow!(
    //         "Cannot specify environments and conda prefixes at the same time"
    //     ));
    // }

    // let ignore_pypi = if cli_config.ignore_pypi().is_some() {
    //     cli_config.ignore_pypi()
    // } else {
    //     toml_config.get_ignore_pypi()
    // };

    // let output_format = cli_config.output_format().unwrap_or_default();

    let lockfile_or_prefix = get_lockfile_or_prefix(cli_config, toml_config)?;

    let config = match cli_config {
        CondaDenyCliConfig::Check { osi, .. } => {
            // defaults to false
            let osi = if osi.is_some() {
                osi
            } else {
                toml_config.get_osi()
            }
            .unwrap_or(false);

            let (safe_licenses, ignore_packages) =
                get_license_information_from_toml_config(&toml_config)?;
            if osi && !safe_licenses.is_empty() {
                return Err(anyhow::anyhow!(
                    "Cannot use OSI mode and safe-licenses at the same time"
                ));
            }

            if !osi && safe_licenses.is_empty() {
                return Err(anyhow::anyhow!("No license whitelist provided"));
            }

            CondaDenyConfig::Check(CondaDenyCheckConfig {
                lockfile_or_prefix,
                osi,
                safe_licenses,
                ignore_packages,
                output_format,
            })
        }
        CondaDenyCliConfig::List { .. } => CondaDenyConfig::List(CondaDenyListConfig {
            lockfile_or_prefix,
            output_format,
        }),
    };

    Ok(config)
}
