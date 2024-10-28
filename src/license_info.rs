use std::path::Path;

use anyhow::{Context, Result};
use rattler_conda_types::PackageRecord;
use spdx::Expression;

use colored::*;

use crate::{
    conda_deny_config::CondaDenyConfig,
    conda_meta_entry::{CondaMetaEntries, CondaMetaEntry},
    expression_utils::{check_expression_safety, extract_license_ids, parse_expression},
    license_whitelist::ParsedLicenseWhitelist,
    list,
    pixi_lock::get_package_records_for_pixi_lock,
    CheckOutput,
};

#[derive(Debug, Clone)]
pub struct LicenseInfo {
    pub package_name: String,
    pub version: String,
    #[allow(dead_code)]
    pub timestamp: Option<u64>,
    pub license: LicenseState,
    #[allow(dead_code)]
    pub platform: Option<String>,
    pub build: String,
}

impl LicenseInfo {
    pub fn from_conda_meta_entry(entry: &CondaMetaEntry) -> Self {
        LicenseInfo {
            package_name: entry.name.clone(),
            version: entry.version.clone(),
            timestamp: Some(entry.timestamp),
            license: entry.license.clone(),
            platform: Some(entry.platform.clone()),
            build: entry.build.clone(),
        }
    }

    pub fn from_package_record(package_record: PackageRecord) -> Self {
        let license_str = match &package_record.license {
            Some(license) => license.clone(),
            None => "None".to_string(),
        };
        let license_for_package = match parse_expression(&license_str) {
            Ok(parsed_license) => LicenseState::Valid(parsed_license),
            Err(_) => LicenseState::Invalid(license_str.to_owned()),
        };

        LicenseInfo {
            package_name: package_record.name.as_source().to_string(),
            version: package_record.version.version().to_string(),
            timestamp: None,
            license: license_for_package,
            platform: Some(package_record.subdir),
            build: package_record.build,
        }
    }

    pub fn pretty_print(&self, colored: bool) -> String {
        let license_str = match &self.license {
            LicenseState::Valid(license) => license.to_string(),
            LicenseState::Invalid(license) => license.to_string(),
        };

        let recognized = match &self.license {
            LicenseState::Valid(_) => "",
            LicenseState::Invalid(_) => "(Non-SPDX)",
        };
        if colored {
            format!(
                "{} {}-{} ({}): {} {}\n",
                &self.package_name.blue(),
                &self.version.cyan(),
                &self.build.bright_cyan().italic(),
                &self
                    .platform
                    .as_ref()
                    .unwrap_or(&"Unknown".to_string())
                    .bright_purple(),
                license_str.yellow(),
                recognized.bright_black(),
            )
        } else {
            format!(
                "{} {}-{} ({}): {} {}\n",
                &self.package_name,
                &self.version,
                &self.build.italic(),
                &self.platform.as_ref().unwrap_or(&"Unknown".to_string()),
                license_str,
                recognized,
            )
        }
    }
}

use std::cmp::Ordering;

impl PartialEq for LicenseInfo {
    fn eq(&self, other: &Self) -> bool {
        self.package_name == other.package_name
            && self.version == other.version
            && self.build == other.build
            && self.platform == other.platform
    }
}

impl Eq for LicenseInfo {}

impl PartialOrd for LicenseInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LicenseInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.package_name
            .cmp(&other.package_name)
            .then_with(|| self.version.cmp(&other.version))
    }
}

pub struct LicenseInfos {
    pub license_infos: Vec<LicenseInfo>,
}

impl LicenseInfos {
    pub fn sort(&mut self) {
        self.license_infos.sort();
    }

    pub fn dedup(&mut self) {
        self.license_infos.dedup();
    }

    pub fn from_pixi_lockfiles(
        lockfiles: Vec<String>,
        platforms: Vec<String>,
        environment_specs: Vec<String>,
    ) -> Result<LicenseInfos> {
        let mut license_infos = Vec::new();

        let mut package_records = Vec::new();

        if lockfiles.is_empty() {
            let package_records_for_lockfile = get_package_records_for_pixi_lock(
                None,
                environment_specs.clone(),
                platforms.clone(),
            )
            .with_context(|| "Failed to get package records for pixi.lock")?;

            package_records.extend(package_records_for_lockfile);
        } else {
            for lockfile in lockfiles {
                let path = Path::new(&lockfile);
                let package_records_for_lockfile = get_package_records_for_pixi_lock(
                    Some(path),
                    environment_specs.clone(),
                    platforms.clone(),
                )
                .with_context(|| {
                    format!("Failed to get package records from lockfile: {}", &lockfile)
                })?;

                package_records.extend(package_records_for_lockfile);
            }
        }
        for package_record in package_records {
            let license_info = LicenseInfo::from_package_record(package_record);
            license_infos.push(license_info);
        }

        license_infos.sort();
        license_infos.dedup();

        Ok(LicenseInfos { license_infos })
    }

    pub fn from_conda_prefixes(conda_prefixes: &Vec<String>) -> Result<LicenseInfos> {
        let mut license_infos = Vec::new();

        for conda_prefix in conda_prefixes {
            let conda_meta_path = format!("{}/conda-meta", conda_prefix);
            let conda_meta_entries =
                CondaMetaEntries::from_dir(&conda_meta_path).with_context(|| {
                    format!(
                        "Failed to parse conda meta entries from conda-meta: {}",
                        conda_meta_path
                    )
                })?;

            let license_infos_for_meta = LicenseInfos::from_conda_meta_entries(conda_meta_entries);

            license_infos.extend(license_infos_for_meta.license_infos);
        }

        license_infos.sort();
        license_infos.dedup();

        Ok(LicenseInfos { license_infos })
    }

    pub fn from_conda_meta_entries(conda_meta_entries: CondaMetaEntries) -> Self {
        let license_infos = conda_meta_entries
            .entries
            .iter()
            .map(LicenseInfo::from_conda_meta_entry)
            .collect();
        LicenseInfos { license_infos }
    }

    pub fn get_license_infos_from_config(
        config: &CondaDenyConfig,
        cli_lockfiles: &Vec<String>,
        cli_platforms: &Vec<String>,
        cli_environments: &Vec<String>,
    ) -> Result<LicenseInfos> {
        let mut platforms = config.get_platform_spec().map_or(vec![], |p| p);
        let mut lockfiles = config.get_lockfile_spec();
        let mut environment_specs = config.get_environment_spec().map_or(vec![], |e| e);

        platforms.extend(cli_platforms.clone());
        lockfiles.extend(cli_lockfiles.clone());
        environment_specs.extend(cli_environments.clone());

        LicenseInfos::from_pixi_lockfiles(lockfiles, platforms, environment_specs)
    }

    pub fn check(&self, license_whitelist: &ParsedLicenseWhitelist) -> CheckOutput {
        let mut safe_dependencies = Vec::new();
        let mut unsafe_dependencies = Vec::new();

        for license_info in &self.license_infos {
            match &license_info.license {
                LicenseState::Valid(license) => {
                    if check_expression_safety(license, &license_whitelist.safe_licenses)
                        || license_whitelist
                            .is_package_ignored(&license_info.package_name, &license_info.version)
                            .unwrap()
                    {
                        safe_dependencies.push(license_info.clone());
                    } else {
                        unsafe_dependencies.push(license_info.clone());
                    }
                }
                LicenseState::Invalid(_) => {
                    if license_whitelist
                        .is_package_ignored(&license_info.package_name, &license_info.version)
                        .unwrap()
                    {
                        safe_dependencies.push(license_info.clone());
                    } else {
                        unsafe_dependencies.push(license_info.clone());
                    }
                }
            }
        }

        (safe_dependencies, unsafe_dependencies)
    }

    pub fn osi_check(&self) -> CheckOutput {
        let mut safe_dependencies = Vec::new();
        let mut unsafe_dependencies = Vec::new();

        for license_info in &self.license_infos {
            match &license_info.license {
                LicenseState::Valid(license) => {
                    let license_ids = extract_license_ids(license);
                    let is_safe = license_ids.iter().all(|license_id_str| {
                        if let Some(license_id) = spdx::license_id(license_id_str) {
                            license_id.is_osi_approved()
                        } else {
                            false
                        }
                    });

                    if is_safe {
                        safe_dependencies.push(license_info.clone());
                    } else {
                        unsafe_dependencies.push(license_info.clone());
                    }
                }
                LicenseState::Invalid(_) => {
                    unsafe_dependencies.push(license_info.clone());
                }
            }
        }

        (safe_dependencies, unsafe_dependencies)
    }

    pub fn list(&self) {
        let output = list::list_license_infos(self, true);
        println!("{}", output);
    }
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum LicenseState {
    Valid(Expression),
    Invalid(String),
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::conda_meta_entry::CondaMetaEntry;
    use spdx::Expression;

    #[test]
    fn test_license_info_creation() {
        let license_info = LicenseInfo {
            package_name: "test".to_string(),
            version: "1.0".to_string(),
            timestamp: Some(1234567890),
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: "py_0".to_string(),
        };

        assert_eq!(license_info.package_name, "test");
        assert_eq!(license_info.version, "1.0");
        assert_eq!(license_info.timestamp, Some(1234567890));
        assert_eq!(
            license_info.license,
            LicenseState::Invalid("Invalid-MIT".to_string())
        );
        assert_eq!(license_info.platform, Some("linux-64".to_string()));
        assert_eq!(license_info.build, "py_0".to_string());
    }

    #[test]
    fn test_license_info_from_conda_meta_entry() {
        let entry = CondaMetaEntry {
            name: "test".to_string(),
            version: "1.0".to_string(),
            timestamp: 1234567890,
            license: super::LicenseState::Invalid("Invalid-MIT".to_string()),
            sha256: "123456".to_string(),
            platform: "linux-64".to_string(),
            build: "py_0".to_string(),
        };

        let license_info = LicenseInfo::from_conda_meta_entry(&entry);

        assert_eq!(license_info.package_name, "test");
        assert_eq!(license_info.version, "1.0");
        assert_eq!(license_info.timestamp, Some(1234567890));
        assert_eq!(
            license_info.license,
            super::LicenseState::Invalid("Invalid-MIT".to_string())
        );
        assert_eq!(license_info.platform, Some("linux-64".to_string()));
        assert_eq!(license_info.build, "py_0".to_string());
    }

    #[test]
    fn test_exit_code_for_safe_and_unsafe_dependencies() {
        // Create license infos without unsafe dependencies
        let unsafe_license_info = LicenseInfo {
            package_name: "test".to_string(),
            version: "0.1.0".to_string(),
            timestamp: Some(1234567890),
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: "py_0".to_string(),
        };
        let safe_license_info = LicenseInfo {
            package_name: "test".to_string(),
            version: "0.1.0".to_string(),
            timestamp: Some(1234567890),
            license: LicenseState::Valid(Expression::parse("MIT").unwrap()),
            platform: Some("linux-64".to_string()),
            build: "py_0".to_string(),
        };

        let unsafe_license_infos = LicenseInfos {
            license_infos: vec![unsafe_license_info, safe_license_info.clone()],
        };

        let safe_license_infos = LicenseInfos {
            license_infos: vec![safe_license_info.clone(), safe_license_info.clone()],
        };

        let license_whitelist = ParsedLicenseWhitelist {
            safe_licenses: vec![Expression::parse("MIT").unwrap()],
            ignore_packages: vec![],
        };

        let (_safe_dependencies, _unsafe_dependencies) =
            unsafe_license_infos.check(&license_whitelist);
        assert!(!_unsafe_dependencies.is_empty());

        let (_safe_dependencies, _unsafe_dependencies) =
            safe_license_infos.check(&license_whitelist);
        assert!(_unsafe_dependencies.is_empty());
    }

    #[test]
    fn test_sort_license_infos() {
        let license_info1 = LicenseInfo {
            package_name: "test".to_string(),
            version: "0.1.0".to_string(),
            timestamp: Some(1234567890),
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: "py_0".to_string(),
        };
        let license_info2 = LicenseInfo {
            package_name: "test2".to_string(),
            version: "0.1.0".to_string(),
            timestamp: Some(1234567890),
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: "py_0".to_string(),
        };

        let mut license_infos = LicenseInfos {
            license_infos: vec![license_info2, license_info1],
        };

        license_infos.sort();

        assert_eq!(license_infos.license_infos[0].package_name, "test");
        assert_eq!(license_infos.license_infos[1].package_name, "test2");
    }

    #[test]
    fn test_dedub_license_infos() {
        let license_info1 = LicenseInfo {
            package_name: "test".to_string(),
            version: "0.1.0".to_string(),
            timestamp: Some(1234567890),
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: "py_0".to_string(),
        };
        let license_info2 = LicenseInfo {
            package_name: "test".to_string(),
            version: "0.1.0".to_string(),
            timestamp: Some(1234567890),
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: "py_0".to_string(),
        };

        let mut license_infos = LicenseInfos {
            license_infos: vec![license_info1, license_info2],
        };

        license_infos.dedup();

        assert_eq!(license_infos.license_infos.len(), 1);
    }

    #[test]
    fn test_license_infos_from_config() {
        let test_file_path = format!(
            "{}test_config_for_license_infos.toml",
            "tests/test_pyproject_toml_files/"
        );
        let config = CondaDenyConfig::from_path(&test_file_path).expect("Failed to read config");
        let license_infos =
            LicenseInfos::get_license_infos_from_config(&config, &vec![], &vec![], &vec![]);
        assert_eq!(license_infos.unwrap().license_infos.len(), 396);
    }
}
