use std::io::Write;

use crate::{fetch_license_infos, license_info::LicenseInfos, CondaDenyListConfig};
use anyhow::{Context, Result};

pub fn list_license_infos(license_infos: &LicenseInfos, colored: bool) -> String {
    let mut output = String::new();

    for license_info in &license_infos.license_infos {
        output.push_str(&license_info.pretty_print(colored));
    }
    output
}

pub fn list<W: Write>(config: &CondaDenyListConfig, mut out: W) -> Result<()> {
    let license_infos = fetch_license_infos(config.lockfile_or_prefix.clone())
        .with_context(|| "Fetching license information failed.")?;
    license_infos.list(&mut out)
}
