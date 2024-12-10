use crate::expression_utils::parse_expression;
use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::license_info::LicenseState;

pub struct CondaMetaEntry {
    pub name: String,
    pub version: String,
    pub license: LicenseState,
    #[allow(dead_code)]
    pub sha256: String,
    pub timestamp: u64,
    #[allow(dead_code)]
    pub platform: String,
    pub build: String,
}

impl CondaMetaEntry {
    pub fn from_filepath(filepath: &str) -> Result<Self> {
        let contents = Self::read_file(filepath)
            .with_context(|| format!("Failed to read file: {}", filepath))?;

        let v: Value = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse JSON from file: {}", filepath))?;

        let license_str = v["license"].as_str().unwrap_or_default();
        let license = match parse_expression(license_str) {
            Ok(parsed_license) => LicenseState::Valid(parsed_license),
            Err(_) => LicenseState::Invalid(license_str.to_owned()),
        };

        Ok(CondaMetaEntry {
            name: v["name"].as_str().unwrap_or_default().to_owned(),
            version: v["version"].as_str().unwrap_or_default().to_owned(),
            license,
            sha256: v["sha256"].as_str().unwrap_or_default().to_owned(),
            timestamp: v["timestamp"].as_u64().unwrap_or_default(),
            platform: v["subdir"].as_str().unwrap_or_default().to_owned(),
            build: v["build"].as_str().unwrap_or_default().to_owned(),
        })
    }

    fn read_file(filepath: &str) -> Result<String> {
        let path = Path::new(filepath);

        if !path.exists() {
            anyhow::bail!("Error: The file {} does not exist.", filepath);
        }

        let mut file = File::open(filepath)
            .with_context(|| format!("Failed to open the file: {}", filepath))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .with_context(|| format!("Failed to read the file: {}", filepath))?;

        Ok(contents)
    }
}

pub struct CondaMetaEntries {
    pub entries: Vec<CondaMetaEntry>,
}

impl CondaMetaEntries {
    pub fn from_dir(conda_meta_path: &PathBuf) -> Result<Self> {
        let mut conda_meta_entries: Vec<CondaMetaEntry> = Vec::new();

        for entry in fs::read_dir(conda_meta_path)? {
            let entry = entry?;
            let file_name = entry
                .path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();

            // Skip file if it is not a .json file
            if !file_name.ends_with(".json") {
                continue;
            }
            let conda_meta_entry =
                CondaMetaEntry::from_filepath(entry.path().to_str().unwrap_or_else(|| {
                    panic!("Failed to convert path to string: {:?}", entry.path())
                }))?;
            conda_meta_entries.push(conda_meta_entry);
        }

        Ok(CondaMetaEntries {
            entries: conda_meta_entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    pub fn test_non_json_in_conda_meta() {
        let conda_meta_entries =
            CondaMetaEntries::from_dir(&PathBuf::from_str("tests/test_conda_metas/non-json-in-conda-meta").unwrap()).unwrap();
        assert_eq!(conda_meta_entries.entries.len(), 1);
        assert_eq!(conda_meta_entries.entries[0].name, "xz");
        assert_eq!(conda_meta_entries.entries[0].version, "5.2.6");
    }

    #[test]
    pub fn test_non_existent_conda_meta() {
        assert!(
            CondaMetaEntries::from_dir(&PathBuf::from_str("tests/test_conda_metas/non-existent-conda-meta").unwrap()).is_err()
        );
    }

    #[test]
    pub fn test_from_filepath_nonexistent_file() {
        assert!(CondaMetaEntry::from_filepath("non-existent-file.json").is_err());
    }

    #[test]
    pub fn test_from_filepath_invalid_json() {
        let entries =
            CondaMetaEntry::from_filepath("tests/test_conda_metas/invalid_json/invalid.json");
        assert!(entries.is_err());
    }

    #[test]
    pub fn test_from_dir_nonexistent_dir() {
        let entries = CondaMetaEntries::from_dir(&PathBuf::from_str("non-existent-dir").unwrap());
        assert!(entries.is_err());
        assert!(entries
            .err()
            .unwrap()
            .to_string()
            .contains("No such file or directory"));
    }
}
