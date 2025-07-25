use anyhow::{Context, Result};
use log::debug;
use rattler_conda_types::Platform;
use serde::Deserialize;
use std::path::PathBuf;
use std::vec;
use std::{fs::File, io::Read};

use crate::license_allowlist::IgnorePackage;

#[derive(Debug, Deserialize)]
pub struct CondaDenyTomlConfig {
    pub tool: Tool,
}

#[derive(Debug, Deserialize)]
pub struct Tool {
    #[serde(rename = "conda-deny")]
    pub conda_deny: CondaDeny,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum LicenseAllowlist {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PlatformSpec {
    Single(Platform),
    Multiple(Vec<Platform>),
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
#[serde(rename_all = "kebab-case")]
pub struct CondaDeny {
    #[serde(alias = "license-whitelist")]
    license_allowlist: Option<LicenseAllowlist>,
    #[serde(rename = "platform")]
    platform_spec: Option<PlatformSpec>,
    #[serde(rename = "environment")]
    environment_spec: Option<EnviromentSpec>,
    #[serde(rename = "lockfile")]
    lockfile_spec: Option<LockfileSpec>,
    osi: Option<bool>,
    ignore_pypi: Option<bool>,
    pub safe_licenses: Option<Vec<String>>,
    pub ignore_packages: Option<Vec<IgnorePackage>>,
}

impl CondaDenyTomlConfig {
    pub fn from_path(filepath: PathBuf) -> Result<Self> {
        let mut file = File::open(filepath.clone())?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let config: CondaDenyTomlConfig = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse TOML from the file: {filepath:?}"))?;

        debug!("Loaded config from file: {:?}", filepath.clone());

        Ok(config)
    }

    pub fn get_license_allowlists(&self) -> Vec<String> {
        if self.tool.conda_deny.license_allowlist.is_none() {
            Vec::<String>::new()
        } else {
            match &self.tool.conda_deny.license_allowlist {
                None => vec![],
                Some(LicenseAllowlist::Single(path)) => vec![path.clone()],
                Some(LicenseAllowlist::Multiple(path)) => path.clone(),
            }
        }
    }

    pub fn get_platform_spec(&self) -> Option<Vec<Platform>> {
        match &self.tool.conda_deny.platform_spec {
            Some(PlatformSpec::Single(name)) => Some(vec![*name]),
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
                    license_allowlist: None,
                    platform_spec: None,
                    environment_spec: None,
                    lockfile_spec: None,
                    osi: None,
                    ignore_pypi: None,
                    safe_licenses: None,
                    ignore_packages: None,
                },
            },
        }
    }
}
