use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use rattler_conda_types::prefix_record::PrefixRecord;
use rattler_conda_types::PackageRecord;
use serde::Serialize;
use spdx::Expression;

use crate::{
    expression_utils::{check_expression_safety, extract_license_texts, parse_expression},
    license_allowlist::is_package_ignored,
    pixi_lock::get_conda_packages_for_pixi_lock,
    CheckOutput, CondaDenyCheckConfig, LockfileSpec,
};

#[derive(Debug, Clone, Serialize)]
pub struct LicenseInfo {
    pub package_name: String,
    pub version: String,
    pub license: LicenseState,
    pub platform: Option<String>,
    pub build: String,
}

impl LicenseInfo {
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
            license: license_for_package,
            platform: Some(package_record.subdir),
            build: package_record.build,
        }
    }

    pub fn pretty_print(&self) -> String {
        let license_str = match &self.license {
            LicenseState::Valid(license) => license.to_string(),
            LicenseState::Invalid(license) => license.to_string(),
        };

        let recognized = match &self.license {
            LicenseState::Valid(_) => "",
            LicenseState::Invalid(_) => "(Non-SPDX)",
        };
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

#[derive(Debug, Clone, Serialize)]
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
        let mut conda_packages = Vec::new();

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
            conda_packages.extend(package_records_for_lockfile);
        }

        for conda_package in conda_packages {
            let license_info = LicenseInfo::from_package_record(conda_package.record().to_owned());
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
            // This is needed because collect_from_prefix silently ignores non-existing prefixes
            if !conda_prefix.join("conda-meta").exists() {
                anyhow::bail!("Error: The prefix path {:?} does not exist.", conda_prefix);
            }

            let prefix_records: Vec<PrefixRecord> = PrefixRecord::collect_from_prefix(conda_prefix)
                .with_context(|| {
                    format!("Failed to collect prefix records from: {conda_prefix:?}")
                })?;

            let package_records: Vec<PackageRecord> = prefix_records
                .iter()
                .map(|record| record.repodata_record.package_record.clone())
                .collect();

            for package_record in package_records {
                let license_info = LicenseInfo::from_package_record(package_record.to_owned());
                license_infos.push(license_info);
            }
        }

        license_infos.sort();
        license_infos.dedup();

        Ok(LicenseInfos { license_infos })
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
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[allow(clippy::large_enum_variant)]
pub enum LicenseState {
    #[serde(serialize_with = "serialize_expression")]
    Valid(Expression),
    Invalid(String),
}

fn serialize_expression<S>(expr: &Expression, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&format!("{expr:?}"))
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{LockfileOrPrefix, OutputFormat};
    use spdx::Expression;

    #[test]
    fn test_exit_code_for_safe_and_unsafe_dependencies() {
        // Create license infos without unsafe dependencies
        let unsafe_license_info = LicenseInfo {
            package_name: "test".to_string(),
            version: "0.1.0".to_string(),
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: "py_0".to_string(),
        };
        let safe_license_info = LicenseInfo {
            package_name: "test".to_string(),
            version: "0.1.0".to_string(),
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
            osi: false,
            safe_licenses,
            ignore_packages,
            output_format: OutputFormat::Default,
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
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: "py_0".to_string(),
        };
        let license_info2 = LicenseInfo {
            package_name: "test2".to_string(),
            version: "0.1.0".to_string(),
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
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: "py_0".to_string(),
        };
        let license_info2 = LicenseInfo {
            package_name: "test".to_string(),
            version: "0.1.0".to_string(),
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
