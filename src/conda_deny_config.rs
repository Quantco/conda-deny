use anyhow::{Context, Result};
use log::debug;
use serde::Deserialize;
use std::vec;
use std::{fs::File, io::Read};

#[derive(Debug, Deserialize)]
pub struct CondaDenyConfig {
    tool: Tool,
    #[serde(skip)]
    pub path: String,
}

#[derive(Debug, Deserialize)]
struct Tool {
    #[serde(rename = "conda-deny")]
    conda_deny: CondaDeny,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum LicenseWhitelist {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum PlatformSpec {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum EnviromentSpec {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum ExcludeEnviromentSpec {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum LockfileSpec {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize)]
struct CondaDeny {
    #[serde(rename = "license-whitelist")]
    license_whitelist: Option<LicenseWhitelist>,
    #[serde(rename = "platform")]
    platform_spec: Option<PlatformSpec>,
    #[serde(rename = "environment")]
    environment_spec: Option<EnviromentSpec>,
    #[serde(rename = "exclude-environment")]
    exclude_environment_spec: Option<ExcludeEnviromentSpec>,
    #[serde(rename = "lockfile")]
    lockfile_spec: Option<LockfileSpec>,
}

impl CondaDenyConfig {
    pub fn from_path(filepath: &str) -> Result<Self> {
        let mut file =
            File::open(filepath).with_context(|| format!("Failed to open file: {}", filepath))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .with_context(|| format!("Failed to read file contents: {}", filepath))?;

        let mut config: CondaDenyConfig = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse TOML from the file: {}", filepath))?;

        config.path = filepath.to_string();

        if config.tool.conda_deny.environment_spec.is_some()
            && config.tool.conda_deny.exclude_environment_spec.is_some()
        {
            return Err(anyhow::anyhow!(
                "Config cannot have both environment and exclude-environment fields"
            ));
        }

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

    pub fn get_exclude_environment_spec(&self) -> Option<Vec<String>> {
        match &self.tool.conda_deny.exclude_environment_spec {
            Some(ExcludeEnviromentSpec::Single(name)) => Some(vec![name.clone()]),
            Some(ExcludeEnviromentSpec::Multiple(names)) => Some(names.clone()),
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

    pub fn empty() -> Self {
        CondaDenyConfig {
            tool: Tool {
                conda_deny: CondaDeny {
                    license_whitelist: None,
                    platform_spec: None,
                    environment_spec: None,
                    exclude_environment_spec: None,
                    lockfile_spec: None,
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

    const TEST_FILES: &str = "tests/test_config_setups/";

    #[test]
    fn test_exclude_environment_parsing() {
        let test_file_path = format!(
            "{}/test_exclude_environment/exclude_environment.toml",
            TEST_FILES
        );
        let config = CondaDenyConfig::from_path(&test_file_path);
        assert!(config.is_ok());

        let exclude_environment = config
            .unwrap()
            .tool
            .conda_deny
            .exclude_environment_spec
            .unwrap();

        let _correct_exclude_environment = ExcludeEnviromentSpec::Single("lint".to_string());
        assert!(matches!(exclude_environment, _correct_exclude_environment));

        let test_file_path = format!(
            "{}/test_exclude_environment/multiple_exclude_environment.toml",
            TEST_FILES
        );
        let config = CondaDenyConfig::from_path(&test_file_path);
        assert!(config.is_ok());

        let exclude_environments = config
            .unwrap()
            .tool
            .conda_deny
            .exclude_environment_spec
            .unwrap();

        let _correct_exclude_environments =
            ExcludeEnviromentSpec::Multiple(vec!["lint".to_string(), "demo".to_string()]);
        assert!(matches!(
            exclude_environments,
            _correct_exclude_environments
        ));

        let test_file_path = format!(
            "{}/test_exclude_environment/error_exclude_environment.toml",
            TEST_FILES
        );
        let config = CondaDenyConfig::from_path(&test_file_path);
        assert!(config.is_err());
    }

    #[test]
    fn test_valid_config_multiple_urls() {
        let test_file_path = format!("{}valid_config_multiple_urls.toml", TEST_FILES);
        let config = CondaDenyConfig::from_path(&test_file_path).expect("Failed to read config");

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
        let config = CondaDenyConfig::from_path(&test_file_path).expect("Failed to read config");

        let license_config_paths = config.get_license_whitelists();
        assert_eq!(
            license_config_paths,
            vec!("https://example.org/conda-deny/base_config.toml")
        );
    }

    #[test]
    fn test_invalid_toml() {
        let test_file_path = format!("{}invalid.toml", TEST_FILES);
        let result = CondaDenyConfig::from_path(&test_file_path);

        assert!(result.is_err());
    }

    #[test]
    fn test_get_license_config_paths() {
        let test_file_path = format!("{}/valid_config_single_url.toml", TEST_FILES);
        let config = CondaDenyConfig::from_path(&test_file_path).expect("Failed to read config");

        assert_eq!(
            config.get_license_whitelists(),
            vec!["https://example.org/conda-deny/base_config.toml".to_string()]
        );

        let test_file_path = format!("{}/valid_config_multiple_urls.toml", TEST_FILES);
        let config = CondaDenyConfig::from_path(&test_file_path).expect("Failed to read config");

        assert_eq!(
            config.get_license_whitelists(),
            vec![
                "https://example.org/conda-deny/base_config1.toml".to_string(),
                "https://example.org/conda-deny/base_config2.toml".to_string()
            ]
        );
    }
}
