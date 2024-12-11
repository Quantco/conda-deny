pub mod cli;
pub mod conda_deny_config;
mod conda_meta_entry;
mod conda_meta_package;
pub mod expression_utils;
mod license_info;
pub mod license_whitelist;
mod list;
mod pixi_lock;
mod read_remote;

use std::{env, io::Write, path::PathBuf};

use cli::CondaDenyCliConfig;
use colored::Colorize;
use conda_deny_config::CondaDenyTomlConfig;
use license_info::LicenseInfo;
use license_whitelist::{get_license_information_from_toml_config, IgnorePackage};

use anyhow::{Context, Result};
use log::debug;
use rattler_conda_types::Platform;
use spdx::Expression;

use crate::license_info::LicenseInfos;

#[derive(Debug)]
pub enum CondaDenyConfig {
    Check(CondaDenyCheckConfig),
    List(CondaDenyListConfig),
}

/// Configuration for the check command
#[derive(Debug)]
pub struct CondaDenyCheckConfig {
    pub lockfile_or_prefix: LockfileOrPrefix,
    pub include_safe: bool,
    pub osi: bool,
    pub ignore_pypi: bool,
    pub safe_licenses: Vec<Expression>,
    pub ignore_packages: Vec<IgnorePackage>,
}

/// Shared configuration between check and list commands
#[derive(Debug)]
pub struct CondaDenyListConfig {
    pub lockfile_or_prefix: LockfileOrPrefix,
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

fn get_lockfile_or_prefix(
    lockfile: Vec<String>,
    prefix: Vec<String>,
    platforms: Option<Vec<Platform>>,
    environments: Option<Vec<String>>,
    ignore_pypi: bool,
) -> Result<LockfileOrPrefix> {
    if lockfile.is_empty() && prefix.is_empty() {
        // test if pixi.lock exists next to config file, otherwise error
        let default_lockfile_path = env::current_dir()?.join("pixi.lock");
        if !default_lockfile_path.is_file() {
            Err(anyhow::anyhow!("No lockfiles or conda prefixes provided"))
        } else {
            Ok(LockfileOrPrefix::Lockfile(LockfileSpec {
                lockfiles: vec![default_lockfile_path],
                platforms,
                environments,
                ignore_pypi,
            }))
        }
    } else if !lockfile.is_empty() && !prefix.is_empty() {
        Err(anyhow::anyhow!(
            "Both lockfiles and conda prefixes provided. Please only provide either or."
        ))
    } else if !lockfile.is_empty() {
        Ok(LockfileOrPrefix::Lockfile(LockfileSpec {
            lockfiles: lockfile.iter().map(|s| s.into()).collect(),
            platforms,
            environments,
            ignore_pypi,
        }))
    } else {
        // todo: throw errors instead
        assert!(!prefix.is_empty());
        assert!(platforms.is_none());
        assert!(environments.is_none());
        Ok(LockfileOrPrefix::Prefix(
            prefix.iter().map(|s| s.into()).collect(),
        ))
    }
}

pub fn get_config_options(
    config: Option<String>,
    cli_config: CondaDenyCliConfig,
) -> Result<CondaDenyConfig> {
    // if config provided, use config
    // else, try to load pixi.toml, then pyproject.toml and if nothing helps, use empty config
    let toml_config = if let Some(config_path) = config {
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
                match CondaDenyTomlConfig::from_path("pyproject.toml")
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

    let config = match cli_config {
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
            // todo: fix unwrap
            let platform = platform.map(|p| p.iter().map(|s| s.parse().unwrap()).collect());

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

            // defaults to false
            // todo: fail if used in combination with prefix
            let ignore_pypi = if ignore_pypi.is_some() {
                ignore_pypi
            } else {
                toml_config.get_ignore_pypi()
            }
            .unwrap_or(false);

            let lockfile_or_prefix = get_lockfile_or_prefix(
                lockfile,
                prefix.clone(),
                platform,
                environment,
                ignore_pypi,
            )?;

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

            // osi and safe-licenses are mutually exclusive
            // todo: throw an error here instead
            let (safe_licenses, ignore_packages) = if osi {
                (vec![], vec![])
            } else {
                get_license_information_from_toml_config(toml_config)?
            };

            CondaDenyConfig::Check(CondaDenyCheckConfig {
                lockfile_or_prefix,
                include_safe,
                osi,
                ignore_pypi,
                safe_licenses,
                ignore_packages,
            })
        }
        CondaDenyCliConfig::List {
            lockfile,
            prefix,
            platform,
            environment,
            ignore_pypi,
        } => {
            // todo: refactor with check
            // cli overrides toml configuration
            let lockfile = lockfile.unwrap_or(toml_config.get_lockfile_spec());
            let prefix = prefix.unwrap_or_default();

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
            // todo: fix unwrap
            let platform = platform.map(|p| p.iter().map(|s| s.parse().unwrap()).collect());

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

            // defaults to false
            // todo: fail if used in combination with prefix
            let ignore_pypi = if ignore_pypi.is_some() {
                ignore_pypi
            } else {
                toml_config.get_ignore_pypi()
            }
            .unwrap_or(false);

            let lockfile_or_prefix = get_lockfile_or_prefix(
                lockfile,
                prefix.clone(),
                platform,
                environment,
                ignore_pypi,
            )?;

            CondaDenyConfig::List(CondaDenyListConfig { lockfile_or_prefix })
        }
    };

    Ok(config)
}

pub fn check<W: Write>(check_config: CondaDenyCheckConfig, mut out: W) -> Result<()> {
    let (safe_dependencies, unsafe_dependencies) = check_license_infos(&check_config)?;

    writeln!(
        out,
        "{}",
        format_check_output(
            safe_dependencies,
            unsafe_dependencies.clone(),
            check_config.include_safe,
        )
    )?;

    if !unsafe_dependencies.is_empty() {
        Err(anyhow::anyhow!("Unsafe licenses found"))
    } else {
        Ok(())
    }
}

pub fn list<W: Write>(config: &CondaDenyListConfig, mut out: W) -> Result<()> {
    let license_infos = fetch_license_infos(config.lockfile_or_prefix.clone())
        .with_context(|| "Fetching license information failed.")?;
    license_infos.list(&mut out)
}

pub fn check_license_infos(config: &CondaDenyCheckConfig) -> Result<CheckOutput> {
    let license_infos = fetch_license_infos(config.lockfile_or_prefix.clone())
        .with_context(|| "Fetching license information failed.")?;

    if config.osi {
        debug!("Checking licenses for OSI compliance");
        Ok(license_infos.osi_check())
    } else {
        debug!("Checking licenses against specified whitelist");
        Ok(license_infos.check(config))
    }
}

pub fn format_check_output(
    safe_dependencies: Vec<LicenseInfo>,
    unsafe_dependencies: Vec<LicenseInfo>,
    include_safe_dependencies: bool,
) -> String {
    let mut output = String::new();

    if include_safe_dependencies && !safe_dependencies.is_empty() {
        output.push_str(
            format!(
                "\n✅ {}:\n\n",
                "The following dependencies are safe".green()
            )
            .as_str(),
        );
        for license_info in &safe_dependencies {
            output.push_str(&license_info.pretty_print(true))
        }
    }

    if !unsafe_dependencies.is_empty() {
        output.push_str(
            format!(
                "\n❌ {}:\n\n",
                "The following dependencies are unsafe".red()
            )
            .as_str(),
        );
        for license_info in &unsafe_dependencies {
            output.push_str(&license_info.pretty_print(true))
        }
    }

    if unsafe_dependencies.is_empty() {
        output.push_str(&format!(
            "\n{}",
            "✅ No unsafe licenses found! ✅".to_string().green()
        ));
    } else {
        output.push_str(&format!(
            "\n{}",
            "❌ Unsafe licenses found! ❌".to_string().red()
        ));
    }

    output.push_str(&format!(
        "\nThere were {} safe licenses and {} unsafe licenses.\n",
        safe_dependencies.len().to_string().green(),
        unsafe_dependencies.len().to_string().red()
    ));

    output.push('\n');

    output
}
