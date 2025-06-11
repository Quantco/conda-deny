use crate::{
    fetch_license_infos,
    license_info::{LicenseInfo, LicenseState},
    CheckOutput, CondaDenyCheckConfig, OutputFormat,
};
use anyhow::{Context, Result};
use colored::Colorize;
use log::debug;
use serde::Serialize;
use serde_json::json;
use std::io::Write;

fn check_license_infos(config: &CondaDenyCheckConfig) -> Result<CheckOutput> {
    let license_infos = fetch_license_infos(config.lockfile_or_prefix.clone())
        .with_context(|| "Fetching license information failed.")?;

    if config.osi {
        debug!("Checking licenses for OSI compliance");
        Ok(license_infos.osi_check())
    } else {
        debug!("Checking licenses against specified allowlist");
        license_infos.check(config)
    }
}

pub fn check<W: Write>(check_config: CondaDenyCheckConfig, mut out: W) -> Result<()> {
    let (safe_dependencies, unsafe_dependencies) = check_license_infos(&check_config)?;

    match check_config.output_format {
        OutputFormat::Default => {
            writeln!(
                out,
                "{}",
                format_check_output(safe_dependencies, unsafe_dependencies.clone(),)
            )?;
        }
        OutputFormat::Json => {
            let json_output = json!({
                "safe": safe_dependencies,
                "unsafe": unsafe_dependencies,
            });
            writeln!(out, "{}", json_output)?;
        }
        OutputFormat::JsonPretty => {
            let json_output = json!({
                "safe": safe_dependencies,
                "unsafe": unsafe_dependencies,
            });
            writeln!(out, "{}", serde_json::to_string_pretty(&json_output)?)?;
        }
        OutputFormat::Csv => {
            #[derive(Debug, Clone, Serialize)]
            struct LicenseInfoWithSafety {
                package_name: String,
                version: String,
                license: LicenseState,
                platform: Option<String>,
                build: String,
                safe: bool,
            }

            let mut writer = csv::WriterBuilder::new().from_writer(vec![]);

            for (license_info, is_safe) in unsafe_dependencies
                .iter()
                .map(|x: &LicenseInfo| (x, false))
                .chain(safe_dependencies.iter().map(|x: &LicenseInfo| (x, true)))
            {
                let extended_info = LicenseInfoWithSafety {
                    package_name: license_info.package_name.clone(),
                    version: license_info.version.clone(),
                    license: license_info.license.clone(),
                    platform: license_info.platform.clone(),
                    build: license_info.build.clone(),
                    safe: is_safe,
                };
                writer.serialize(&extended_info).with_context(|| {
                    format!(
                        "Failed to serialize the following LicenseInfo to CSV: {:?}",
                        extended_info
                    )
                })?;
            }

            out.write_all(&writer.into_inner()?)?;
        }
    }

    if !unsafe_dependencies.is_empty() {
        Err(anyhow::anyhow!("Unsafe licenses found"))
    } else {
        Ok(())
    }
}

pub fn format_check_output(
    safe_dependencies: Vec<LicenseInfo>,
    unsafe_dependencies: Vec<LicenseInfo>,
) -> String {
    let mut output = String::new();

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
