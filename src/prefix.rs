use anyhow::{Context, Result};

use crate::conda_meta_entry::CondaMetaEntries;

pub fn get_package_urls_for_prefixes(conda_prefixes: Vec<String>) -> Result<Vec<String>> {
    let mut package_urls_for_prefix = Vec::new();

    for conda_prefix in conda_prefixes {
        let conda_meta_path = format!("{}/conda-meta", conda_prefix);
        let conda_meta_entries =
            CondaMetaEntries::from_dir(&conda_meta_path).with_context(|| {
                format!(
                    "Failed to parse conda meta entries from conda-meta: {}",
                    conda_meta_path
                )
            })?;

        for entry in conda_meta_entries.entries {
            package_urls_for_prefix.push(entry.url);
        }
    }
    package_urls_for_prefix.sort();
    package_urls_for_prefix.dedup();

    Ok(package_urls_for_prefix)
}
