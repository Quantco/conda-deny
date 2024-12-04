pub mod cli;
pub mod conda_deny_config;
mod conda_meta_entry;
mod conda_meta_package;
mod expression_utils;
mod license_info;
pub mod license_whitelist;
mod list;
mod pixi_lock;
mod read_remote;

use colored::Colorize;
use license_info::LicenseInfo;
use license_whitelist::build_license_whitelist;

use anyhow::{Context, Result};
use log::debug;

use crate::license_info::LicenseInfos;

#[derive(Debug)]
pub enum CondaDenyConfig {
    Check(CondaDenyCheckConfig),
    List(CondaDenyListConfig),
}

/// Configuration for the check command
#[derive(Debug)]
pub struct CondaDenyCheckConfig {
    pub prefix: Vec<String>,
    pub lockfile: Vec<String>,
    pub platform: Option<Vec<String>>,
    pub environment: Option<Vec<String>>,
    pub include_safe: bool,
    pub osi: bool,
    pub ignore_pypi: bool,
    pub license_whitelist: Vec<String>,
}

/// Shared configuration between check and list commands
#[derive(Debug)]
pub struct CondaDenyListConfig {
    pub prefix: Vec<String>,
    pub lockfile: Vec<String>,
    pub platform: Option<Vec<String>>,
    pub environment: Option<Vec<String>>,
}

pub type CheckOutput = (Vec<LicenseInfo>, Vec<LicenseInfo>);

pub fn fetch_license_infos(config: &CondaDenyCheckConfig) -> Result<LicenseInfos> {
    // TODO: what when both prefix and lockfiles are not empty?
    if config.prefix.is_empty() {
        LicenseInfos::from_pixi_lockfiles(config)
            .with_context(|| "Getting license information from config file failed.")
    } else {
        LicenseInfos::from_conda_prefixes(config)
            .with_context(|| "Getting license information from conda prefixes failed.")
    }
}

pub fn list(config: &CondaDenyListConfig) -> Result<()> {
    panic!("TODO");
    // let license_infos =
    //     fetch_license_infos(config).with_context(|| "Fetching license information failed.")?;
    // license_infos.list();
    Ok(())
}

pub fn check_license_infos(config: &CondaDenyCheckConfig) -> Result<CheckOutput> {
    let license_infos =
        fetch_license_infos(config).with_context(|| "Fetching license information failed.")?;

    if config.osi {
        debug!("Checking licenses for OSI compliance");
        Ok(license_infos.osi_check())
    } else {
        let license_whitelist = build_license_whitelist(config)
            .with_context(|| "Building the license whitelist failed.")?;
        debug!("Checking licenses against specified whitelist");
        Ok(license_infos.check(&license_whitelist))
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
