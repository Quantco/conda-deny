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

use crate::conda_deny_config::CondaDenyConfig;
use crate::license_info::LicenseInfos;

// todo: refactor this
pub type CliInput<'a> = (
    &'a CondaDenyConfig,
    &'a Vec<String>,
    &'a Vec<String>,
    &'a Vec<String>,
    &'a Vec<String>,
    bool,
    bool,
);
pub type CheckOutput = (Vec<LicenseInfo>, Vec<LicenseInfo>);

pub fn fetch_license_infos(cli_input: CliInput) -> Result<LicenseInfos> {
    let (conda_deny_config, cli_lockfiles, cli_platforms, cli_environments, conda_prefixes, _, cli_ignore_pypi) =
        cli_input;

    if conda_prefixes.is_empty() {
        LicenseInfos::get_license_infos_from_config(
            conda_deny_config,
            cli_lockfiles,
            cli_platforms,
            cli_environments, 
            cli_ignore_pypi,
        )
        .with_context(|| "Getting license information from config file failed.")
    } else {
        LicenseInfos::from_conda_prefixes(conda_prefixes)
            .with_context(|| "Getting license information from conda prefixes failed.")
    }
}

pub fn list(cli_input: CliInput) -> Result<()> {
    let license_infos =
        fetch_license_infos(cli_input).with_context(|| "Fetching license information failed.")?;
    license_infos.list();
    Ok(())
}

pub fn check_license_infos(cli_input: CliInput) -> Result<CheckOutput> {
    let (conda_deny_config, _, _, _, _, osi, _) = cli_input;

    let license_infos =
        fetch_license_infos(cli_input).with_context(|| "Fetching license information failed.")?;

    if osi {
        debug!("Checking licenses for OSI compliance");
        Ok(license_infos.osi_check())
    } else {
        let license_whitelist = build_license_whitelist(conda_deny_config)
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
