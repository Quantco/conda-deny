use crate::{fetch_license_infos, license_info::LicenseInfo, CheckOutput, CondaDenyCheckConfig};
use anyhow::{Context, Result};
use colored::Colorize;
use log::debug;
use std::io::Write;

fn check_license_infos(config: &CondaDenyCheckConfig) -> Result<CheckOutput> {
    let license_infos = fetch_license_infos(config.lockfile_or_prefix.clone())
        .with_context(|| "Fetching license information failed.")?;

    if config.osi {
        debug!("Checking licenses for OSI compliance");
        Ok(license_infos.osi_check())
    } else {
        debug!("Checking licenses against specified whitelist");
        license_infos.check(config)
    }
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
            output.push_str(&license_info.pretty_print())
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
            output.push_str(&license_info.pretty_print())
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
