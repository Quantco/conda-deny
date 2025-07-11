use std::io::Write;

use crate::{fetch_license_infos, CondaDenyListConfig, OutputFormat};
use anyhow::{Context, Result};

pub fn list<W: Write>(config: CondaDenyListConfig, mut out: W) -> Result<()> {
    let license_infos = fetch_license_infos(config.lockfile_or_prefix.clone())
        .with_context(|| "Fetching license information failed.")?;

    match config.output_format {
        OutputFormat::Default => {
            let mut output = String::new();
            for license_info in &license_infos.license_infos {
                output.push_str(&license_info.pretty_print());
            }
            writeln!(out, "{output}")?;
        }
        OutputFormat::Json => {
            serde_json::to_writer(&mut out, &license_infos)?;
        }
        OutputFormat::JsonPretty => {
            serde_json::to_writer_pretty(&mut out, &license_infos)?;
        }
        OutputFormat::Csv => {
            let mut writer = csv::WriterBuilder::new().from_writer(vec![]);

            for license_info in &license_infos.license_infos {
                writer.serialize(license_info).with_context(|| {
                    format!(
                        "Failed to serialize the following license info: {license_info:?}"
                    )
                })?;
            }

            out.write_all(&writer.into_inner()?)?;
        }
    }
    out.flush()?;
    Ok(())
}
