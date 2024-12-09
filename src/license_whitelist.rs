use std::{env, fs, str::FromStr};

use anyhow::{Context, Result};
use async_trait::async_trait;
use log::debug;
use rattler_conda_types::{ParseStrictness, Version, VersionSpec};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use serde::Deserialize;
use spdx::Expression;

use crate::{
    conda_deny_config::CondaDenyTomlConfig, expression_utils::parse_expression,
    CondaDenyCheckConfig,
};

#[derive(Debug, Deserialize)]
pub struct LicenseWhitelistConfig {
    tool: RemoteWhitelistTool,
}

#[derive(Debug, Deserialize)]
struct RemoteWhitelistTool {
    #[serde(rename = "conda-deny")]
    conda_deny: LicenseWhitelist,
}

#[derive(Debug)]
pub struct ParsedLicenseWhitelist {
    pub safe_licenses: Vec<Expression>,
    pub ignore_packages: Vec<IgnorePackage>,
}

#[derive(Debug, Deserialize)]
pub struct IgnorePackage {
    package: String,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LicenseWhitelist {
    #[serde(rename = "safe-licenses")]
    safe_licenses: Option<Vec<String>>,
    #[serde(rename = "ignore-packages")]
    ignore_packages: Option<Vec<IgnorePackage>>,
}

pub fn is_package_ignored_2(safe_licenses: Vec<Expression>, package_name: &str, package_version: &str) -> Result<bool> {
    let parsed_package_version = Version::from_str(package_version).with_context(|| {
        format!(
            "Error parsing package version: {} for package: {}",
            package_version, package_name
        )
    })?;

    for ignore_package in &self.ignore_packages {
        if ignore_package.package == package_name {
            match &ignore_package.version {
                Some(version_req_str) => {
                    let version_req =
                        VersionSpec::from_str(version_req_str, ParseStrictness::Strict)
                            .with_context(|| {
                                format!(
                                    "Error parsing version requirement: {} for package: {}",
                                    version_req_str, package_name
                                )
                            })?;

                    if version_req.matches(&parsed_package_version) {
                        return Ok(true);
                    }
                }
                None => {
                    return Ok(true);
                }
            }
        }
    }

    // If no matches were found, the package is not ignored
    Ok(false)
}

impl ParsedLicenseWhitelist {
    pub fn from_toml_file(toml_file: &str) -> Result<ParsedLicenseWhitelist> {
        let config_content = fs::read_to_string(toml_file)
            .with_context(|| format!("Failed to read TOML file: {}", toml_file))?;

        let config: LicenseWhitelistConfig = toml::from_str(&config_content)
            .with_context(|| format!("Failed to parse TOML content from file: {}", toml_file))?;

        let mut expressions = Vec::new();

        if let Some(safe_licenses) = config.tool.conda_deny.safe_licenses {
            for license in safe_licenses {
                let expr = parse_expression(&license)
                    .with_context(|| format!("Failed to parse license expression: {}", license))?;
                expressions.push(expr);
            }
        }

        let ignore_packages = config.tool.conda_deny.ignore_packages.unwrap_or_default();

        Ok(ParsedLicenseWhitelist {
            safe_licenses: expressions,
            ignore_packages,
        })
    }

    pub fn is_package_ignored(&self, package_name: &str, package_version: &str) -> Result<bool> {
        let parsed_package_version = Version::from_str(package_version).with_context(|| {
            format!(
                "Error parsing package version: {} for package: {}",
                package_version, package_name
            )
        })?;

        for ignore_package in &self.ignore_packages {
            if ignore_package.package == package_name {
                match &ignore_package.version {
                    Some(version_req_str) => {
                        let version_req =
                            VersionSpec::from_str(version_req_str, ParseStrictness::Strict)
                                .with_context(|| {
                                    format!(
                                        "Error parsing version requirement: {} for package: {}",
                                        version_req_str, package_name
                                    )
                                })?;

                        if version_req.matches(&parsed_package_version) {
                            return Ok(true);
                        }
                    }
                    None => {
                        return Ok(true);
                    }
                }
            }
        }

        // If no matches were found, the package is not ignored
        Ok(false)
    }
    pub fn empty() -> ParsedLicenseWhitelist {
        ParsedLicenseWhitelist {
            safe_licenses: Vec::new(),
            ignore_packages: Vec::new(),
        }
    }

    pub fn extend(&mut self, other: ParsedLicenseWhitelist) {
        self.safe_licenses.extend(other.safe_licenses);
        self.ignore_packages.extend(other.ignore_packages);
    }
}

#[async_trait]
pub trait ReadRemoteConfig {
    async fn read(&self, url: &str) -> Result<LicenseWhitelistConfig>;
}

pub struct RealRemoteConfigReader;

#[async_trait]
impl ReadRemoteConfig for RealRemoteConfigReader {
    async fn read(&self, url: &str) -> Result<LicenseWhitelistConfig> {
        let client = reqwest::Client::new();

        // Add GitHub specific headers if GITHUB_TOKEN exists
        let mut headers = HeaderMap::new();
        if let Ok(token) = env::var("GITHUB_TOKEN") {
            headers.insert(
                ACCEPT,
                HeaderValue::from_static("application/vnd.github.v3.raw"),
            );
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token))
                    .with_context(|| "Invalid Header value for AUTHORIZATION")?,
            );
        }

        let remote_config = client
            .get(url)
            .headers(headers)
            .send()
            .await?
            .text()
            .await?;

        let value: LicenseWhitelistConfig = toml::from_str(&remote_config).with_context(|| {
            format!(
                "Failed to parse license whitelist to TOML for whitelist URL: {}",
                url
            )
        })?;

        Ok(value)
    }
}

pub fn fetch_safe_licenses(
    remote_config: &str,
    reader: &dyn ReadRemoteConfig,
) -> Result<ParsedLicenseWhitelist> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let url = remote_config;
    let read_config_task = reader.read(url);

    match runtime.block_on(read_config_task) {
        Ok(config) => {
            let mut expressions = Vec::new();
            if config.tool.conda_deny.safe_licenses.is_some() {
                for license in config.tool.conda_deny.safe_licenses.unwrap() {
                    let expr = parse_expression(&license).with_context(|| {
                        format!("Failed to parse license expression: {}", license)
                    })?;
                    expressions.push(expr);
                }
            }
            let mut ignore_packages = Vec::new();
            if config.tool.conda_deny.ignore_packages.is_some() {
                ignore_packages = config.tool.conda_deny.ignore_packages.unwrap();
            }

            Ok(ParsedLicenseWhitelist {
                safe_licenses: expressions,
                ignore_packages,
            })
        }
        Err(_) => Err(anyhow::anyhow!("Failed to read remote license whitelist.\nPlease check the URL. If you need a GITHUB_TOKEN, please set it in your environment.")),
    }
}

pub fn build_license_whitelist(license_whitelist: &Vec<String>) -> Result<ParsedLicenseWhitelist> {
    // TODO: license whitelist from config as well
    // let license_whitelist = ParsedLicenseWhitelist::from_toml_file(&conda_deny_config.license_whitelist)
    //     .with_context(|| {
    //         format!(
    //             "Failed to read the TOML file at path: {}",
    //             conda_deny_config.path
    //         )
    //     })?;

    // final_license_whitelist.extend(license_whitelist);
    let mut final_license_whitelist = ParsedLicenseWhitelist::empty();

    for license_whitelist_path in license_whitelist.iter() {
        if license_whitelist_path.starts_with("http") {
            let reader = RealRemoteConfigReader;

            match fetch_safe_licenses(&license_whitelist_path, &reader) {
                Ok(safe_licenses_for_url) => final_license_whitelist.extend(safe_licenses_for_url),
                Err(e) => {
                    return Err(e).with_context(|| {
                        format!(
                            "Failed to fetch safe licenses from URL: {}",
                            license_whitelist_path
                        )
                    });
                }
            }
        } else {
            match ParsedLicenseWhitelist::from_toml_file(&license_whitelist_path) {
                Ok(license_whitelist) => final_license_whitelist.extend(license_whitelist),
                Err(e) => {
                    return Err(e).with_context(|| {
                        format!(
                            "Failed to parse TOML file at path: {}",
                            license_whitelist_path
                        )
                    });
                }
            }
        }
    }

    if final_license_whitelist.safe_licenses.is_empty() {
        anyhow::bail!("Your license whitelist is empty.\nIf you want to use the OSI license whitelist, use the --osi flag.");
    } else {
        debug!("License whitelist built successfully.");
    }
    Ok(final_license_whitelist)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::error::Error;

    struct MockReader;

    #[async_trait]
    impl ReadRemoteConfig for MockReader {
        async fn read(&self, _url: &str) -> Result<LicenseWhitelistConfig> {
            Ok(LicenseWhitelistConfig {
                tool: RemoteWhitelistTool {
                    conda_deny: LicenseWhitelist {
                        safe_licenses: Some(vec!["MIT".to_string(), "Apache-2.0".to_string()]),
                        ignore_packages: Some(vec![]),
                    },
                },
            })
        }
    }

    #[test]
    fn test_fetch_safe_licenses_success() {
        let reader = MockReader;
        // TODO: Change this to "https://raw.githubusercontent.com/QuantCo/conda-deny/main/tests/test_remote_base_configs/conda-deny-license_whitelist.toml"
        let result = fetch_safe_licenses("https://raw.githubusercontent.com/PaulKMueller/conda-deny-test/main/conda-deny-license_whitelist.toml", &reader)
            .unwrap();

        // Assert the result
        assert_eq!(result.safe_licenses.len(), 2);
        assert!(result.safe_licenses.iter().any(|e| e.to_string() == "MIT"));
        assert!(result
            .safe_licenses
            .iter()
            .any(|e| e.to_string() == "Apache-2.0"));
    }

    #[test]
    fn test_valid_remote_base_config() {
        let parsed_config = super::ParsedLicenseWhitelist::from_toml_file(
            "tests/test_remote_base_configs/valid_config.toml",
        )
        .unwrap();
        assert_eq!(parsed_config.safe_licenses.len(), 2);
        assert_eq!(parsed_config.ignore_packages.len(), 1);
    }

    #[test]
    fn test_invalid_remote_base_config() {
        let parsed_config = super::ParsedLicenseWhitelist::from_toml_file(
            "tests/test_remote_base_configs/invalid_config.toml",
        );
        assert!(parsed_config.is_err());
    }

    #[test]
    fn test_different_versions_in_remote_base_config() {
        let parsed_config = super::ParsedLicenseWhitelist::from_toml_file(
            "tests/test_remote_base_configs/version_test_config.toml",
        )
        .unwrap();
        assert_eq!(parsed_config.safe_licenses.len(), 2);
        assert_eq!(parsed_config.ignore_packages.len(), 3);

        assert!(parsed_config
            .ignore_packages
            .iter()
            .any(|x| x.package == "package1" && x.version == Some("=4.2.1".to_string())));
        assert!(parsed_config
            .ignore_packages
            .iter()
            .any(|x| x.package == "package2" && x.version == Some("<=4.2.1".to_string())));
        assert!(parsed_config
            .ignore_packages
            .iter()
            .any(|x| x.package == "package3" && x.version == Some(">4.2.1".to_string())));
    }

    #[test]
    fn test_semver_matching() {
        let version1 = Version::from_str("4.2.1").unwrap();
        let version2 = Version::from_str("4.2.2").unwrap();
        let version3 = Version::from_str("4.2.0").unwrap();
        let version_req = VersionSpec::from_str("=4.2.1", ParseStrictness::Strict).unwrap();

        assert!(version_req.matches(&version1));
        assert!(!version_req.matches(&version2));
        assert!(!version_req.matches(&version3));
    }

    #[test]
    fn test_is_package_ignored() {
        let parsed_config = super::ParsedLicenseWhitelist::from_toml_file(
            "tests/test_remote_base_configs/version_test_config.toml",
        )
        .unwrap();
        assert!(parsed_config
            .is_package_ignored("package1", "4.2.1")
            .unwrap());
        assert!(!parsed_config
            .is_package_ignored("package1", "4.3.0")
            .unwrap());
        assert!(!parsed_config
            .is_package_ignored("package1", "4.3.2")
            .unwrap());
    }

    #[test]
    fn test_empty_parsed_license_whitelist() {
        let empty = super::ParsedLicenseWhitelist::empty();
        assert_eq!(empty.safe_licenses.len(), 0);
        assert_eq!(empty.ignore_packages.len(), 0);
    }

    #[test]
    fn test_extend_parsed_license_whitelist() {
        let mut empty = super::ParsedLicenseWhitelist::empty();
        let parsed_config = super::ParsedLicenseWhitelist::from_toml_file(
            "tests/test_remote_base_configs/version_test_config.toml",
        )
        .unwrap();
        empty.extend(parsed_config);
        assert_eq!(empty.safe_licenses.len(), 2);
        assert_eq!(empty.ignore_packages.len(), 3);
    }

    // Mock the read_remote_config function
    async fn _mock_read_remote_config(
        _url: &str,
    ) -> Result<LicenseWhitelistConfig, Box<dyn Error>> {
        Ok(LicenseWhitelistConfig {
            tool: RemoteWhitelistTool {
                conda_deny: LicenseWhitelist {
                    safe_licenses: Some(vec!["MIT".to_string(), "Apache-2.0".to_string()]),
                    ignore_packages: Some(vec![]),
                },
            },
        })
    }

    #[test]
    fn test_get_safe_licenses_local() {
        let conda_deny_config =
            CondaDenyTomlConfig::from_path("tests/test_remote_base_configs/valid_config.toml")
                .unwrap();
        panic!("TODO");
        // let safe_licenses_whitelist = super::build_license_whitelist(&conda_deny_config).unwrap();
        // assert_eq!(safe_licenses_whitelist.safe_licenses.len(), 5);
        // assert_eq!(
        //     safe_licenses_whitelist.safe_licenses,
        //     vec![
        //         parse_expression("MIT").unwrap(),
        //         parse_expression("PSF-2.0").unwrap(),
        //         parse_expression("Apache-2.0").unwrap(),
        //         parse_expression("Unlicense").unwrap(),
        //         parse_expression("WTFPL").unwrap()
        //     ]
        // );
        // assert_eq!(safe_licenses_whitelist.ignore_packages.len(), 2);
        // assert_eq!(safe_licenses_whitelist.ignore_packages[0].version, None);
    }
}
