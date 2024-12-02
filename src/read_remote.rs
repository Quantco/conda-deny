use crate::license_whitelist::LicenseWhitelistConfig;
use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use std::env;

use core::str;

pub async fn _read_remote_config(url: &str) -> Result<LicenseWhitelistConfig> {
    let client = reqwest::Client::new();

    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/vnd.github.v3.raw"),
    );

    if let Ok(token) = env::var("GITHUB_TOKEN") {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token))
                .with_context(|| "Failed to create authorization header")?,
        );
    }

    let response = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .with_context(|| "Failed to send request to the remote server")?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
                "Failed to download config from {}. Status: {}.\nPlease make sure you have set the GITHUB_TOKEN environment variable.",
                url,
                response.status()
            ));
    }

    let remote_config = response
        .text()
        .await
        .with_context(|| "Failed to read response text")?;

    let value: LicenseWhitelistConfig = toml::from_str(&remote_config)
        .with_context(|| "Failed to parse TOML from the downloaded config")?;

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_remote_config() {
        let url = "https://raw.githubusercontent.com/QuantCo/conda-deny/main/tests/test_remote_base_configs/conda-deny-license_whitelist.toml";
        let whitelist = _read_remote_config(url).await;

        assert!(whitelist.is_ok());
    }

    #[tokio::test]
    async fn test_read_remote_non_existent_config() {
        let url = "https://raw.githubusercontent.com/QuantCo/conda-deny/main/tests/test_remote_base_configs/this-config-does-not-exist.toml";
        let whitelist = _read_remote_config(url).await;

        assert!(whitelist.is_err());
    }
}
