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

pub type CheckInput<'a> = (
    &'a CondaDenyConfig,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    Vec<String>,
    bool,
);
pub type CheckOutput = (Vec<LicenseInfo>, Vec<LicenseInfo>);

pub fn list(conda_deny_config: &CondaDenyConfig) -> Result<()> {
    let mut license_infos =
        LicenseInfos::get_license_infos_from_config(conda_deny_config, vec![], vec![], vec![])
            .with_context(|| "Getting license information from config file failed.")?;

    license_infos.sort();
    license_infos.dedup();

    license_infos.list();
    Ok(())
}

pub fn check_license_infos(check_input: CheckInput) -> Result<CheckOutput> {
    let (conda_deny_config, cli_lockfiles, cli_platforms, cli_environment, conda_prefixes, osi) =
        check_input;

    if conda_prefixes.is_empty() {
        let mut license_infos = LicenseInfos::get_license_infos_from_config(
            conda_deny_config,
            cli_lockfiles,
            cli_platforms,
            cli_environment,
        )
        .with_context(|| "Getting license information from config file failed.")?;

        license_infos.sort();
        license_infos.dedup();

        if osi {
            debug!("Checking licenses for OSI compliance");
            Ok(license_infos.osi_check())
        } else {
            let license_whitelist = build_license_whitelist(conda_deny_config)
            .with_context(|| "Building the license whitelist failed.")?;
            debug!("Checking licenses against specified whitelist");
            Ok(license_infos.check(&license_whitelist))
        }
    } else {
        let mut conda_prefixes_license_infos = LicenseInfos::from_conda_prefixes(&conda_prefixes)?;
        conda_prefixes_license_infos.sort();
        conda_prefixes_license_infos.dedup();
        if osi {
            debug!("Checking for OSI licenses");
            Ok(conda_prefixes_license_infos.osi_check())
        } else {
            let license_whitelist = build_license_whitelist(conda_deny_config)
            .with_context(|| "Building the license whitelist failed.")?;
            debug!("Checking licenses against specified whitelist");
            Ok(conda_prefixes_license_infos.check(&license_whitelist))
        }
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
