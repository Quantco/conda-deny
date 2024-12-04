use anyhow::{Context, Result};
use log::debug;
use serde::Deserialize;
use std::vec;
use std::{fs::File, io::Read};

#[derive(Debug, Deserialize)]
pub struct CondaDenyTomlConfig {
    tool: Tool,
    #[serde(skip)]
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct Tool {
    #[serde(rename = "conda-deny")]
    conda_deny: CondaDeny,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum LicenseWhitelist {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformSpec {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum EnviromentSpec {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum LockfileSpec {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize)]
struct PixiEnvironmentEntry {
    _file: String,
    _environments: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CondaDeny {
    #[serde(rename = "license-whitelist")]
    license_whitelist: Option<LicenseWhitelist>,
    #[serde(rename = "platform")]
    platform_spec: Option<PlatformSpec>,
    #[serde(rename = "environment")]
    environment_spec: Option<EnviromentSpec>,
    #[serde(rename = "lockfile")]
    lockfile_spec: Option<LockfileSpec>,
    #[serde(rename = "osi")]
    osi: Option<bool>,
    #[serde(rename = "ignore-pypi")]
    ignore_pypi: Option<bool>,
}

impl CondaDenyTomlConfig {
    pub fn from_path(filepath: &str) -> Result<Self> {
        let mut file =
            File::open(filepath).with_context(|| format!("Failed to open file: {}", filepath))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .with_context(|| format!("Failed to read file contents: {}", filepath))?;

        let mut config: CondaDenyTomlConfig = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse TOML from the file: {}", filepath))?;

        config.path = filepath.to_string();

        debug!("Loaded config from file: {}", filepath);

        Ok(config)
    }

    pub fn get_license_whitelists(&self) -> Vec<String> {
        if self.tool.conda_deny.license_whitelist.is_none() {
            Vec::<String>::new()
        } else {
            match &self.tool.conda_deny.license_whitelist.as_ref().unwrap() {
                LicenseWhitelist::Single(path) => vec![path.clone()],
                LicenseWhitelist::Multiple(path) => path.clone(),
            }
        }
    }

    pub fn get_platform_spec(&self) -> Option<Vec<String>> {
        match &self.tool.conda_deny.platform_spec {
            Some(PlatformSpec::Single(name)) => Some(vec![name.clone()]),
            Some(PlatformSpec::Multiple(names)) => Some(names.clone()),
            None => None,
        }
    }

    pub fn get_environment_spec(&self) -> Option<Vec<String>> {
        match &self.tool.conda_deny.environment_spec {
            Some(EnviromentSpec::Single(name)) => Some(vec![name.clone()]),
            Some(EnviromentSpec::Multiple(names)) => Some(names.clone()),
            None => None,
        }
    }

    pub fn get_lockfile_spec(&self) -> Vec<String> {
        match &self.tool.conda_deny.lockfile_spec {
            Some(LockfileSpec::Single(name)) => vec![name.clone()],
            Some(LockfileSpec::Multiple(names)) => names.clone(),
            None => vec![],
        }
    }

    pub fn get_osi(&self) -> Option<bool> {
        self.tool.conda_deny.osi
    }

    pub fn get_ignore_pypi(&self) -> Option<bool> {
        self.tool.conda_deny.ignore_pypi
    }

    pub fn empty() -> Self {
        CondaDenyTomlConfig {
            tool: Tool {
                conda_deny: CondaDeny {
                    license_whitelist: None,
                    platform_spec: None,
                    environment_spec: None,
                    lockfile_spec: None,
                    osi: None,
                    ignore_pypi: None,
                },
            },
            path: "".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    const TEST_FILES: &str = "tests/test_pyproject_toml_files/";

    #[test]
    fn test_valid_config_multiple_urls() {
        let test_file_path = format!("{}valid_config_multiple_urls.toml", TEST_FILES);
        let config =
            CondaDenyTomlConfig::from_path(&test_file_path).expect("Failed to read config");

        let license_config_paths = config.get_license_whitelists();
        assert_eq!(
            &license_config_paths,
            &vec![
                "https://example.org/conda-deny/base_config1.toml".to_string(),
                "https://example.org/conda-deny/base_config2.toml".to_string()
            ]
        );
    }

    #[test]
    fn test_missing_optional_fields() {
        let test_file_path = format!("{}missing_optional_fields.toml", TEST_FILES);
        let config =
            CondaDenyTomlConfig::from_path(&test_file_path).expect("Failed to read config");

        let license_config_paths = config.get_license_whitelists();
        assert_eq!(
            license_config_paths,
            vec!("https://example.org/conda-deny/base_config.toml")
        );
    }

    #[test]
    fn test_invalid_toml() {
        let test_file_path = format!("{}invalid.toml", TEST_FILES);
        let result = CondaDenyTomlConfig::from_path(&test_file_path);

        assert!(result.is_err());
    }

    #[test]
    fn test_get_license_config_paths() {
        let test_file_path = format!("{}/valid_config_single_url.toml", TEST_FILES);
        let config =
            CondaDenyTomlConfig::from_path(&test_file_path).expect("Failed to read config");

        assert_eq!(
            config.get_license_whitelists(),
            vec!["https://example.org/conda-deny/base_config.toml".to_string()]
        );

        let test_file_path = format!("{}/valid_config_multiple_urls.toml", TEST_FILES);
        let config =
            CondaDenyTomlConfig::from_path(&test_file_path).expect("Failed to read config");

        assert_eq!(
            config.get_license_whitelists(),
            vec![
                "https://example.org/conda-deny/base_config1.toml".to_string(),
                "https://example.org/conda-deny/base_config2.toml".to_string()
            ]
        );
    }
}
