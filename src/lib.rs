pub mod bundle;
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

use std::{collections::HashSet, env, path::PathBuf};

use cli::CondaDenyCliConfig;
use conda_deny_config::CondaDenyTomlConfig;
use license_info::LicenseInfo;
use license_whitelist::{get_license_information_from_toml_config, IgnorePackage};

use anyhow::{anyhow, Context, Result};
use glob::glob_with;
use log::{debug, warn};
use rattler_conda_types::Platform;
use serde::Deserialize;
use spdx::Expression;

use crate::license_info::LicenseInfos;

#[derive(Debug)]
pub enum CondaDenyConfig {
    Check(CondaDenyCheckConfig),
    List(CondaDenyListConfig),
    Bundle(CondaDenyBundleConfig),
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

/// Shared configuration between check, list, and bundle commands
#[derive(Debug)]
pub struct CondaDenyListConfig {
    pub lockfile_or_prefix: LockfileOrPrefix,
    pub output_format: OutputFormat,
}

/// Shared configuration between check, list, and bundle commands
#[derive(Debug, Clone)]
pub struct CondaDenyBundleConfig {
    pub lockfile_or_prefix: LockfileOrPrefix,
    pub directory: Option<PathBuf>,
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

fn resolve_glob_patterns(patterns: &[String]) -> Result<Vec<PathBuf>> {
    let mut set = HashSet::new();

    // otherwise, we recurse into .pixi directories
    let glob_options = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: true,
    };

    for pattern in patterns {
        let paths = match glob_with(pattern, glob_options) {
            Ok(paths) => paths,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to resolve glob pattern {}: {}",
                    pattern,
                    e
                ))
            }
        };

        for entry in paths {
            match entry {
                Ok(path) => set.insert(path),
                Err(e) => {
                    return Err(anyhow!(
                        "Error while resolving glob pattern {}: {}",
                        pattern,
                        e
                    ))
                }
            };
        }
    }

    Ok(set.into_iter().collect())
}

fn get_lockfile_or_prefix(
    cli_config: &CondaDenyCliConfig,
    toml_config: &CondaDenyTomlConfig,
) -> Result<LockfileOrPrefix> {
    // cli overrides toml config
    if let Some(prefix) = cli_config.prefix() {
        debug!("Ignoring toml config in favor of CLI config");
        assert!(!prefix.is_empty());
        Ok(LockfileOrPrefix::Prefix(prefix))
    } else if let Some(lockfile_patterns) = cli_config.lockfile() {
        // ignore lockfile spec from toml config, only look at cli config
        debug!("Ignoring toml config in favor of CLI config");
        assert!(!lockfile_patterns.is_empty());
        let lockfile_spec = LockfileSpec {
            environments: cli_config.environment(),
            lockfiles: resolve_glob_patterns(&lockfile_patterns)?,
            platforms: cli_config.platform(),
            ignore_pypi: cli_config.ignore_pypi().unwrap_or(IGNORE_PYPI_DEFAULT),
        };
        Ok(LockfileOrPrefix::Lockfile(lockfile_spec))
    } else {
        let lockfile_patterns = toml_config.get_lockfile_spec();
        let lockfiles = if lockfile_patterns.is_empty() {
            let default_lockfile_path = env::current_dir()?.join("pixi.lock");
            if !default_lockfile_path.is_file() {
                return Err(anyhow::anyhow!("No lockfiles or conda prefixes provided"));
            }
            vec![default_lockfile_path]
        } else {
            resolve_glob_patterns(&lockfile_patterns)?
        };
        if lockfiles.is_empty() {
            warn!("Your lockfile glob patterns did not match any files. THis will do nothing.");
        }
        let platforms = cli_config.platform().or(toml_config.get_platform_spec());
        let environments = cli_config
            .environment()
            .or(toml_config.get_environment_spec());
        let ignore_pypi = cli_config
            .ignore_pypi()
            .or(toml_config.get_ignore_pypi())
            .unwrap_or(IGNORE_PYPI_DEFAULT);
        Ok(LockfileOrPrefix::Lockfile(LockfileSpec {
            lockfiles,
            platforms,
            environments,
            ignore_pypi,
        }))
    }
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

    let output_format = cli_config.output().unwrap_or_default();
    let lockfile_or_prefix = get_lockfile_or_prefix(&cli_config, &toml_config)?;

    let config = match cli_config {
        CondaDenyCliConfig::Check { osi, .. } => {
            let osi = osi.or(toml_config.get_osi()).unwrap_or(false);

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
        CondaDenyCliConfig::Bundle { directory, .. } => {
            CondaDenyConfig::Bundle(CondaDenyBundleConfig {
                lockfile_or_prefix,
                directory,
            })
        }
    };

    Ok(config)
}
