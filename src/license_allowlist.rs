use std::{env, fs, str::FromStr};

use anyhow::{Context, Result};
use async_trait::async_trait;
use log::debug;
use rattler_conda_types::{ParseStrictness, Version, VersionSpec};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use serde::Deserialize;
use spdx::Expression;

use crate::{conda_deny_config::CondaDenyTomlConfig, expression_utils::parse_expression};

#[derive(Debug, Deserialize)]
pub struct LicenseAllowlistConfig {
    tool: RemoteAllowlistTool,
}

#[derive(Debug, Deserialize)]
struct RemoteAllowlistTool {
    #[serde(rename = "conda-deny")]
    conda_deny: LicenseAllowlist,
}

#[derive(Debug, Deserialize, Clone)]
pub struct IgnorePackage {
    package: String,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LicenseAllowlist {
    #[serde(rename = "safe-licenses")]
    safe_licenses: Option<Vec<String>>,
    #[serde(rename = "ignore-packages")]
    ignore_packages: Option<Vec<IgnorePackage>>,
}

pub fn is_package_ignored(
    ignore_packages: &Vec<IgnorePackage>,
    package_name: &str,
    package_version: &str,
) -> Result<bool> {
    let parsed_package_version = Version::from_str(package_version).with_context(|| {
        format!(
            "Error parsing package version: {} for package: {}",
            package_version, package_name
        )
    })?;

    for ignore_package in ignore_packages {
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

pub fn license_config_from_toml_str(
    toml_file: &str,
) -> Result<(Vec<Expression>, Vec<IgnorePackage>)> {
    let config_content = fs::read_to_string(toml_file)
        .with_context(|| format!("Failed to read TOML file: {}", toml_file))?;

    let config: LicenseAllowlistConfig = toml::from_str(&config_content)
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

    Ok((expressions, ignore_packages))
}

#[async_trait]
pub trait ReadRemoteConfig {
    async fn read(&self, url: &str) -> Result<String>;
}

pub struct RealRemoteConfigReader;

#[async_trait]
impl ReadRemoteConfig for RealRemoteConfigReader {
    async fn read(&self, url: &str) -> Result<String> {
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

        let result = client
            .get(url)
            .headers(headers)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(result)
    }
}

pub fn fetch_safe_licenses(
    remote_config: &str,
    reader: &dyn ReadRemoteConfig,
) -> Result<(Vec<Expression>, Vec<IgnorePackage>)> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let url = remote_config;
    let read_config_task = reader.read(url);
    let config_str = runtime.block_on(read_config_task).map_err(|e| {
        anyhow::anyhow!(
            "Failed to read remote license allowlist.\nPlease check the URL. If you need a GITHUB_TOKEN, please set it in your environment.\nError: {}",
            e
        )
    })?;

    let config: LicenseAllowlistConfig = toml::from_str(&config_str).with_context(|| {
        format!(
            "Failed to parse license allowlist to TOML for allowlist URL: {}",
            url
        )
    })?;
    let mut expressions = Vec::new();
    for license in config.tool.conda_deny.safe_licenses.unwrap_or_default() {
        let expr = parse_expression(&license)
            .with_context(|| format!("Failed to parse license expression: {}", license))?;
        expressions.push(expr);
    }
    let ignore_packages = config.tool.conda_deny.ignore_packages.unwrap_or_default();
    Ok((expressions, ignore_packages))
}

pub fn build_license_allowlist(
    license_allowlist: &[String],
) -> Result<(Vec<Expression>, Vec<IgnorePackage>)> {
    let mut all_safe_licenses = Vec::new();
    let mut all_ignore_packages = Vec::new();

    for license_allowlist_path in license_allowlist.iter() {
        // todo: use Url (or Path)
        if license_allowlist_path.starts_with("http") {
            let reader = RealRemoteConfigReader;

            match fetch_safe_licenses(license_allowlist_path, &reader) {
                Ok((safe_licenses, ignore_packages)) => {
                    all_safe_licenses.extend(safe_licenses);
                    all_ignore_packages.extend(ignore_packages);
                }
                Err(e) => {
                    return Err(e).with_context(|| {
                        format!(
                            "Failed to fetch safe licenses from URL: {}",
                            license_allowlist_path
                        )
                    });
                }
            }
        } else {
            match license_config_from_toml_str(license_allowlist_path) {
                Ok((safe_licenses, ignore_packages)) => {
                    all_safe_licenses.extend(safe_licenses);
                    all_ignore_packages.extend(ignore_packages);
                }
                Err(e) => {
                    return Err(e).with_context(|| {
                        format!(
                            "Failed to parse TOML file at path: {}",
                            license_allowlist_path
                        )
                    });
                }
            }
        }
    }

    debug!("License allowlist built successfully.");
    Ok((all_safe_licenses, all_ignore_packages))
}

pub fn get_license_information_from_toml_config(
    toml_config: &CondaDenyTomlConfig,
) -> Result<(Vec<Expression>, Vec<IgnorePackage>)> {
    let safe_licenses_from_toml = toml_config
        .tool
        .conda_deny
        .safe_licenses
        .clone()
        .unwrap_or_default();
    let ignore_packages_from_toml = toml_config
        .tool
        .conda_deny
        .ignore_packages
        .clone()
        .unwrap_or_default();

    let license_allowlist_urls = toml_config.get_license_allowlists().clone();
    let (safe_licenses, ignore_packages) = build_license_allowlist(&license_allowlist_urls)?;

    // TODO: Remove duplicates
    let safe_licenses = safe_licenses_from_toml
        .iter()
        .map(|license_str| parse_expression(license_str))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .chain(safe_licenses)
        .collect::<Vec<_>>();

    // TODO: Remove duplicates
    let ignore_packages = ignore_packages_from_toml
        .iter()
        .cloned()
        .chain(ignore_packages)
        .collect::<Vec<_>>();
    Ok((safe_licenses, ignore_packages))
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use crate::conda_deny_config::CondaDenyTomlConfig;

    use super::*;
    use std::{error::Error, io::Write};

    #[test]
    fn test_fetch_safe_licenses_success() {
        let reader = RealRemoteConfigReader;
        let (safe_licenses, ignore_packages) = fetch_safe_licenses("https://raw.githubusercontent.com/quantco/conda-deny/main/tests/default_license_allowlist.toml", &reader)
            .unwrap();

        // Assert the result
        assert_eq!(safe_licenses.len(), 5);
        assert!(safe_licenses.iter().any(|e| e.to_string() == "MIT"));
        assert!(safe_licenses.iter().any(|e| e.to_string() == "Apache-2.0"));
        assert_eq!(ignore_packages.len(), 1);
    }

    #[test]
    fn test_valid_remote_base_config() {
        // Create a temporary file for the pixi.toml
        let mut temp_config_file = NamedTempFile::new().unwrap();
        let file_content = r#"[tool.conda-deny]
license-allowlist = "tests/default_license_allowlist.toml"
safe-licenses = [
    # Licenses by their SPDX identifier, see https://spdx.org/licenses/
    "MIT",
    "PSF-2.0",
]
ignore-packages = [
    {package="make"},
]"#;
        temp_config_file
            .as_file_mut()
            .write_all(file_content.as_bytes())
            .unwrap();

        let temp_config_path = temp_config_file.path().to_str().unwrap();

        let (safe_licenses, ignored_packages) =
            license_config_from_toml_str(temp_config_path).unwrap();
        assert_eq!(safe_licenses.len(), 2);
        assert_eq!(ignored_packages.len(), 1);
    }

    #[test]
    fn test_is_package_ignored() {
        let ignored_packages = vec![
            IgnorePackage {
                package: "package1".to_string(),
                version: Some("=4.2.1".to_string()),
            },
            IgnorePackage {
                package: "package2".to_string(),
                version: Some("<=4.2.1".to_string()),
            },
            IgnorePackage {
                package: "package3".to_string(),
                version: Some(">4.2.1".to_string()),
            },
        ];
        assert!(is_package_ignored(&ignored_packages, "package1", "4.2.1").unwrap());
        assert!(!is_package_ignored(&ignored_packages, "package1", "4.3.0").unwrap());
        assert!(!is_package_ignored(&ignored_packages, "package1", "4.3.2").unwrap());
    }

    // Mock the read_remote_config function
    async fn _mock_read_remote_config(
        _url: &str,
    ) -> Result<LicenseAllowlistConfig, Box<dyn Error>> {
        Ok(LicenseAllowlistConfig {
            tool: RemoteAllowlistTool {
                conda_deny: LicenseAllowlist {
                    safe_licenses: Some(vec!["MIT".to_string(), "Apache-2.0".to_string()]),
                    ignore_packages: Some(vec![]),
                },
            },
        })
    }

    #[test]
    fn test_get_safe_licenses_local() {
        // Create a temporary file for the pixi.toml
        let mut temp_config_file = NamedTempFile::new().unwrap();
        let file_content = r#"[tool.conda-deny]
        license-allowlist = "tests/default_license_allowlist.toml"
        safe-licenses = [
            # Licenses by their SPDX identifier, see https://spdx.org/licenses/
            "MIT",
            "PSF-2.0",
        ]
        ignore-packages = [
            {package="make"},
        ]"#;
        temp_config_file
            .as_file_mut()
            .write_all(file_content.as_bytes())
            .unwrap();

        let temp_config_path = temp_config_file.path().to_str().unwrap();

        let toml_config = CondaDenyTomlConfig::from_path(temp_config_path.into()).unwrap();
        let (safe_licenses, ignored_packages) =
            get_license_information_from_toml_config(&toml_config).unwrap();
        assert_eq!(safe_licenses.len(), 7);
        assert_eq!(
            safe_licenses,
            vec![
                parse_expression("MIT").unwrap(),
                parse_expression("PSF-2.0").unwrap(),
                parse_expression("Apache-2.0").unwrap(),
                parse_expression("Unlicense").unwrap(),
                parse_expression("WTFPL").unwrap(),
                // Currently, duplicates are still possible
                parse_expression("MIT").unwrap(),
                parse_expression("PSF-2.0").unwrap(),
            ]
        );
        assert_eq!(ignored_packages.len(), 2);
        assert_eq!(ignored_packages[0].version, None);
    }
}
