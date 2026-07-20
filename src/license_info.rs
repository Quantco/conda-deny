use std::{collections::BTreeSet, path::PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use rattler_conda_types::prefix_record::PrefixRecord;
use rattler_conda_types::PackageRecord;
use rattler_lock::{CondaPackageData, CondaSourceData, SourceIdentifier};
use rayon::prelude::*;
use serde::Serialize;
use spdx::Expression;

use crate::{
    expression_utils::{check_expression_safety, extract_license_texts, parse_expression},
    license_allowlist::IgnorePackage,
    license_allowlist::{is_package_ignored, is_package_ignored_by_name_only},
    pixi_lock::get_conda_packages_for_pixi_lock,
    CheckOutput, CondaDenyCheckConfig, LockfileSpec,
};

#[derive(Debug, Clone, Serialize)]
pub struct LicenseInfo {
    pub package_name: String,
    pub version: Option<String>,
    pub license: LicenseState,
    pub platform: Option<String>,
    pub build: Option<String>,
    #[serde(skip_serializing)]
    pub source_identifier: Option<String>,
}

impl LicenseInfo {
    pub fn from_package_record(package_record: PackageRecord) -> Self {
        LicenseInfo {
            package_name: package_record.name.as_source().to_string(),
            version: Some(package_record.version.version().to_string()),
            license: license_state_from_optional_str(package_record.license.as_deref()),
            platform: Some(package_record.subdir),
            build: Some(package_record.build),
            source_identifier: None,
        }
    }

    pub fn from_partial_source(source_data: &CondaSourceData) -> Option<Self> {
        let metadata = source_data.metadata.as_partial()?;

        Some(LicenseInfo {
            package_name: metadata.name.as_source().to_string(),
            version: None,
            license: license_state_from_optional_str(metadata.license.as_deref()),
            platform: None,
            build: None,
            source_identifier: Some(SourceIdentifier::from_source_data(source_data).to_string()),
        })
    }

    pub fn pretty_print(&self) -> String {
        let license_str = match &self.license {
            LicenseState::Valid(license) => license.to_string(),
            LicenseState::Invalid(license) => license.to_string(),
            LicenseState::NoLicense => "no license".to_string(),
        };

        let comment = match &self.license {
            LicenseState::Valid(_) => None,
            LicenseState::Invalid(_) => Some("(Non-SPDX)"),
            LicenseState::NoLicense => None,
        };
        let version = self.version.as_deref().unwrap_or("unknown-source");
        let build = self.build.as_deref().unwrap_or("unknown-source");
        let platform = self.platform.as_deref().unwrap_or("unknown-source");

        if let Some(source_identifier) = &self.source_identifier {
            return if let Some(comment) = comment {
                format!(
                    "{} ({}): {} {}\n",
                    source_identifier.blue(),
                    "source".bright_purple(),
                    license_str.yellow(),
                    comment.bright_black(),
                )
            } else {
                format!(
                    "{} ({}): {}\n",
                    source_identifier.blue(),
                    "source".bright_purple(),
                    license_str.yellow(),
                )
            };
        }

        if let Some(comment) = comment {
            format!(
                "{} {}-{} ({}): {} {}\n",
                self.package_name.blue(),
                version.cyan(),
                build.bright_cyan().italic(),
                platform.bright_purple(),
                license_str.yellow(),
                comment.bright_black(),
            )
        } else {
            format!(
                "{} {}-{} ({}): {}\n",
                self.package_name.blue(),
                version.cyan(),
                build.bright_cyan().italic(),
                platform.bright_purple(),
                license_str.yellow(),
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
            && self.source_identifier == other.source_identifier
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
            .then_with(|| self.build.cmp(&other.build))
            .then_with(|| self.platform.cmp(&other.platform))
            .then_with(|| self.source_identifier.cmp(&other.source_identifier))
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

    pub fn from_pixi_lockfiles(
        lockfile_spec: LockfileSpec,
        ignore_packages: &[IgnorePackage],
    ) -> Result<LicenseInfos> {
        anyhow::ensure!(
            !lockfile_spec.lockfiles.is_empty(),
            "No lockfiles provided in LockfileSpec"
        );

        let conda_packages: Vec<CondaPackageData> = lockfile_spec
            .lockfiles
            .par_iter()
            .map(|lockfile| {
                get_conda_packages_for_pixi_lock(
                    lockfile,
                    &lockfile_spec.environments,
                    &lockfile_spec.platforms,
                    lockfile_spec.ignore_pypi,
                    ignore_packages,
                )
                .with_context(|| {
                    format!(
                        "Failed to get package records from lockfile: {}",
                        lockfile.display()
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

        let mut license_infos = BTreeSet::new();
        for package in conda_packages {
            let package_name = package.name().as_source();

            if let Some(record) = package.record().cloned() {
                let package_version = record.version.version().to_string();
                if is_package_ignored(ignore_packages, package_name, &package_version)? {
                    continue;
                }

                license_infos.insert(LicenseInfo::from_package_record(record));
            } else {
                if is_package_ignored_by_name_only(ignore_packages, package_name) {
                    continue;
                }

                let Some(source) = package.as_source() else {
                    return Err(anyhow::anyhow!(
                        "Package record missing in lockfile for {package_name}. \
                         Add a name-only entry for this package to ignore-packages to ignore it."
                    ));
                };

                let Some(license_info) = LicenseInfo::from_partial_source(source) else {
                    return Err(anyhow::anyhow!(
                        "Package record missing in lockfile for {package_name}. \
                         Add a name-only entry for this package to ignore-packages to ignore it."
                    ));
                };

                license_infos.insert(license_info);
            }
        }

        Ok(LicenseInfos {
            license_infos: license_infos.into_iter().collect(),
        })
    }

    pub fn from_conda_prefixes(
        prefixes: &[PathBuf],
        ignore_packages: &[IgnorePackage],
    ) -> Result<LicenseInfos> {
        let mut license_infos: BTreeSet<_> = BTreeSet::new();
        anyhow::ensure!(!prefixes.is_empty(), "No conda prefixes provided");

        for conda_prefix in prefixes {
            // This is needed because collect_from_prefix silently ignores non-existing prefixes
            let meta_path = conda_prefix.join("conda-meta");
            anyhow::ensure!(
                meta_path.exists(),
                "The conda prefix {:?} is invalid: {:?} directory is missing",
                conda_prefix,
                meta_path
            );
            let prefix_records: Vec<PrefixRecord> = PrefixRecord::collect_from_prefix(conda_prefix)
                .with_context(|| {
                    format!(
                        "Failed to collect prefix records from {}",
                        conda_prefix.display()
                    )
                })?;

            for record in prefix_records {
                let package_record = record.repodata_record.package_record;
                let package_name = package_record.name.as_source();
                let package_version = package_record.version.version().to_string();
                if is_package_ignored(ignore_packages, package_name, &package_version)? {
                    continue;
                }

                license_infos.insert(LicenseInfo::from_package_record(package_record));
            }
        }

        Ok(LicenseInfos {
            license_infos: license_infos.into_iter().collect(),
        })
    }

    pub fn check(&self, config: &CondaDenyCheckConfig) -> Result<CheckOutput> {
        let mut safe_dependencies = Vec::new();
        let mut unsafe_dependencies = Vec::new();

        for license_info in &self.license_infos {
            match &license_info.license {
                LicenseState::Valid(license) => {
                    if check_expression_safety(license, &config.safe_licenses) {
                        safe_dependencies.push(license_info.clone());
                    } else {
                        unsafe_dependencies.push(license_info.clone());
                    }
                }
                LicenseState::Invalid(_) | LicenseState::NoLicense => {
                    unsafe_dependencies.push(license_info.clone());
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
                LicenseState::Invalid(_) | LicenseState::NoLicense => {
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
    NoLicense,
}

fn license_state_from_optional_str(license: Option<&str>) -> LicenseState {
    let Some(license) = license else {
        return LicenseState::NoLicense;
    };
    match parse_expression(license) {
        Ok(parsed_license) => LicenseState::Valid(parsed_license),
        Err(_) => LicenseState::Invalid(license.to_owned()),
    }
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
            version: Some("0.1.0".to_string()),
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: Some("py_0".to_string()),
            source_identifier: None,
        };
        let safe_license_info = LicenseInfo {
            package_name: "test".to_string(),
            version: Some("0.1.0".to_string()),
            license: LicenseState::Valid(Expression::parse("MIT").unwrap()),
            platform: Some("linux-64".to_string()),
            build: Some("py_0".to_string()),
            source_identifier: None,
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
            version: Some("0.1.0".to_string()),
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: Some("py_0".to_string()),
            source_identifier: None,
        };
        let license_info2 = LicenseInfo {
            package_name: "test2".to_string(),
            version: Some("0.1.0".to_string()),
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: Some("py_0".to_string()),
            source_identifier: None,
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
            version: Some("0.1.0".to_string()),
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: Some("py_0".to_string()),
            source_identifier: None,
        };
        let license_info2 = LicenseInfo {
            package_name: "test".to_string(),
            version: Some("0.1.0".to_string()),
            license: LicenseState::Invalid("Invalid-MIT".to_string()),
            platform: Some("linux-64".to_string()),
            build: Some("py_0".to_string()),
            source_identifier: None,
        };

        let mut license_infos = LicenseInfos {
            license_infos: vec![license_info1, license_info2],
        };

        license_infos.dedup();

        assert_eq!(license_infos.license_infos.len(), 1);
    }
}
