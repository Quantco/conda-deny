use std::{
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use rattler_conda_types::PackageRecord;
use spdx::Expression;

use colored::*;

use crate::{
    conda_meta_entry::{CondaMetaEntries, CondaMetaEntry},
    expression_utils::{check_expression_safety, extract_license_texts, parse_expression},
    license_whitelist::is_package_ignored,
    list,
    pixi_lock::get_conda_packages_for_pixi_lock,
    CheckOutput, CondaDenyCheckConfig, LockfileSpec,
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

    pub fn from_pixi_lockfiles(lockfile_spec: LockfileSpec) -> Result<LicenseInfos> {
        let mut license_infos = Vec::new();
        let mut package_records = Vec::new();

        assert!(!lockfile_spec.lockfiles.is_empty());
        for lockfile in lockfile_spec.lockfiles {
            let path: &Path = Path::new(&lockfile);
            let package_records_for_lockfile = get_conda_packages_for_pixi_lock(
                path,
                &lockfile_spec.environments,
                &lockfile_spec.platforms,
                lockfile_spec.ignore_pypi,
            )
            .with_context(|| {
                format!(
                    "Failed to get package records from lockfile: {:?}",
                    &lockfile.to_str()
                )
            })?;
            package_records.extend(package_records_for_lockfile);
        }

        for package_record in package_records {
            let license_info = LicenseInfo::from_package_record(package_record);
            license_infos.push(license_info);
        }

        license_infos.sort();
        license_infos.dedup();

        Ok(LicenseInfos { license_infos })
    }

    pub fn from_conda_prefixes(prefixes: &[PathBuf]) -> Result<LicenseInfos> {
        let mut license_infos = Vec::new();
        assert!(!prefixes.is_empty());
        for conda_prefix in prefixes {
            let conda_meta_path = conda_prefix.join("conda-meta");
            let conda_meta_entries =
                CondaMetaEntries::from_dir(&conda_meta_path).with_context(|| {
                    format!(
                        "Failed to parse conda meta entries from conda-meta: {:?}",
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

    pub fn check(&self, config: &CondaDenyCheckConfig) -> Result<CheckOutput> {
        let mut safe_dependencies = Vec::new();
        let mut unsafe_dependencies = Vec::new();

        for license_info in &self.license_infos {
            match &license_info.license {
                LicenseState::Valid(license) => {
                    if check_expression_safety(license, &config.safe_licenses)
                        || is_package_ignored(
                            &config.ignore_packages,
                            &license_info.package_name,
                            &license_info.version,
                        )?
                    {
                        safe_dependencies.push(license_info.clone());
                    } else {
                        unsafe_dependencies.push(license_info.clone());
                    }
                }
                LicenseState::Invalid(_) => {
                    if is_package_ignored(
                        &config.ignore_packages,
                        &license_info.package_name,
                        &license_info.version,
                    )? {
                        safe_dependencies.push(license_info.clone());
                    } else {
                        unsafe_dependencies.push(license_info.clone());
                    }
                }
            }
        }

        Ok((safe_dependencies, unsafe_dependencies))
    }

    pub fn osi_check(&self) -> CheckOutput {
        let mut safe_dependencies = Vec::new();
        let mut unsafe_dependencies = Vec::new();

        for license_info in &self.license_infos {
            match &license_info.license {
                LicenseState::Valid(license) => {
                    let license_ids = extract_license_texts(license);
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

    pub fn list<W: Write>(&self, mut out: W) -> Result<()> {
        let output = list::list_license_infos(self, true);
        out.write_all(output.as_bytes())?;
        Ok(())
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
    use crate::{conda_meta_entry::CondaMetaEntry, LockfileOrPrefix};
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

        let safe_licenses = vec![Expression::parse("MIT").unwrap()];
        let ignore_packages = vec![];

        let config = CondaDenyCheckConfig {
            lockfile_or_prefix: LockfileOrPrefix::Lockfile(LockfileSpec {
                lockfiles: vec!["pixi.lock".into()],
                platforms: None,
                environments: None,
                ignore_pypi: false,
            }),
            include_safe: false,
            osi: false,
            safe_licenses,
            ignore_packages,
        };

        let (_, unsafe_dependencies) = unsafe_license_infos.check(&config).unwrap();
        assert!(!unsafe_dependencies.is_empty());

        let (_, unsafe_dependencies) = safe_license_infos.check(&config).unwrap();
        assert!(unsafe_dependencies.is_empty());
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
}
