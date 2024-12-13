use std::io::Write;

use crate::{fetch_license_infos, CondaDenyListConfig};
use anyhow::{Context, Result};

pub fn list<W: Write>(config: CondaDenyListConfig, mut out: W) -> Result<()> {
    let license_infos = fetch_license_infos(config.lockfile_or_prefix.clone())
    .with_context(|| "Fetching license information failed.")?;
    let mut output = String::new();
    for license_info in &license_infos.license_infos {
        output.push_str(&license_info.pretty_print());
    }
    out.write_all(output.as_bytes())?;
    Ok(())
}
