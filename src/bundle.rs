use anyhow::{anyhow, Context, Result};
use rattler_conda_types::PrefixRecord;
use rattler_package_streaming::{
    read::stream_tar_bz2,
    seek::{stream_conda_content, stream_conda_info},
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::{
    io::{Cursor, Read, Seek, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use indicatif::{ProgressBar, ProgressStyle};

use rattler_networking::AuthenticationMiddleware;

use crate::{
    pixi_lock::get_conda_packages_for_pixi_lock, CondaDenyBundleConfig, LockfileOrPrefix,
    OutputFormat,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LicenseFile {
    package_name: Option<String>,
    filename: String,
    // TODO: Change to bytes
    license_text: String,
}

pub fn bundle<W: Write>(config: CondaDenyBundleConfig, mut out: W) -> Result<()> {
    let lockfile_or_prefix = config.lockfile_or_prefix.clone();
    let license_files: Vec<LicenseFile>;

    match lockfile_or_prefix {
        LockfileOrPrefix::Lockfile(lockfile_spec) => {
            let lockfiles = lockfile_spec.lockfiles.clone();
            let mut conda_packages = Vec::new();
            for lockfile in lockfiles.clone() {
                conda_packages.extend(
                    get_conda_packages_for_pixi_lock(
                        &lockfile,
                        &lockfile_spec.environments,
                        &lockfile_spec.platforms,
                        lockfile_spec.ignore_pypi,
                    )?
                    .into_iter(),
                );
            }

            let bar = setup_bundle_bar(conda_packages.len() as u64);

            license_files = conda_packages
                .par_iter()
                .map(|conda_package| -> Result<Vec<LicenseFile>, anyhow::Error> {
                    bar.inc(1);
                    bar.set_message(format!(
                        "📦 Bundling licenses for: {}",
                        conda_package.record().name.as_source()
                    ));

                    let rt = tokio::runtime::Runtime::new()?;

                    let mut files = rt
                        .block_on(get_license_files_from_url(
                            conda_package.location().as_url().unwrap().to_owned(),
                        ))
                        .with_context(|| {
                            format!(
                                "Failed to process conda package: {}",
                                conda_package.record().name.as_source()
                            )
                        })?;

                    let package_name = format!(
                        "{}-{}-{}",
                        conda_package.record().name.as_source(),
                        conda_package.record().version,
                        conda_package.record().build
                    );
                    for file in &mut files {
                        file.package_name = Some(package_name.clone());
                    }

                    Ok(files)
                })
                .collect::<Result<Vec<Vec<LicenseFile>>, _>>()?
                .into_iter()
                .flatten()
                .collect();

            bar.finish_with_message("✅ Bundling licenses complete!");
        }
        LockfileOrPrefix::Prefix(prefix_paths) => {
            let mut prefix_records: Vec<PrefixRecord> = Vec::new();
            for prefix in prefix_paths {
                let current_prefix_records =
                    rattler_conda_types::PrefixRecord::collect_from_prefix(prefix.as_path())
                        .with_context(|| {
                            format!("Failed to collect prefix records from: {:?}", prefix)
                        })?;
                prefix_records.extend(current_prefix_records);
            }

            let bar = setup_bundle_bar(prefix_records.len() as u64);

            license_files = prefix_records
                .par_iter()
                .map(|record| -> Result<Vec<LicenseFile>, anyhow::Error> {
                    bar.inc(1);
                    bar.set_message(format!("📦 Bundling licenses for: {}", record.file_name()));
                    let rt = tokio::runtime::Runtime::new()?;
                    let url = record.repodata_record.url.clone();
                    let mut files = rt
                        .block_on(get_license_files_from_url(url.clone()))
                        .with_context(|| format!("Failed to process conda package: {}", url))?;

                    let package_name = record.file_name().to_string();
                    for file in &mut files {
                        file.package_name = Some(package_name.clone());
                    }
                    Ok(files)
                })
                .collect::<Result<Vec<Vec<LicenseFile>>, _>>()?
                .into_iter()
                .flatten()
                .collect();
            bar.finish_with_message("✅ Bundling licenses complete!");
        }
    }

    match config.output_format {
        OutputFormat::Json => {
            if config.directory.is_some() {
                return Err(anyhow::anyhow!(
                    "Cannot use CSV output format with a directory specified"
                ));
            }
            serde_json::to_writer(&mut out, &license_files)
                .with_context(|| "Failed to write license files to JSON")?;
        }
        OutputFormat::JsonPretty => {
            if config.directory.is_some() {
                return Err(anyhow::anyhow!(
                    "Cannot use CSV output format with a directory specified"
                ));
            }
            serde_json::to_writer_pretty(&mut out, &license_files)
                .with_context(|| "Failed to write license files to pretty JSON")?;
        }
        OutputFormat::Csv => {
            // Throw error. CSV output is not supported.
            return Err(anyhow::anyhow!("Bundling to CSV format is not supported."));
        }
        OutputFormat::Default => {
            let path = config.directory.unwrap_or(PathBuf::from(".conda-deny"));
            let path = Path::new(&path);
            create_license_file_directory(path, license_files)
                .with_context(|| format!("Failed to create license file directory: {:?}", path))?;
            writeln!(out, "License files written to: {:?}", path)
                .with_context(|| format!("Failed to write license file: {:?}", path))?;
        }
    }

    Ok(())
}

fn create_license_file_directory(path: &Path, license_files: Vec<LicenseFile>) -> Result<()> {
    if path.exists() {
        std::fs::remove_dir_all(path)
            .with_context(|| format!("Failed to remove existing directory: {:?}", path))?;
    }
    std::fs::create_dir_all(path)
        .with_context(|| format!("Failed to create output directory: {:?}", path))?;
    for license_file in &license_files {
        let package_name = license_file
            .package_name
            .as_ref()
            .ok_or_else(|| anyhow!("Package name is not specified"))?;
        let package_path = path.join(package_name);
        std::fs::create_dir_all(&package_path)
            .with_context(|| format!("Failed to create package directory: {:?}", package_path))?;

        let relative_path = Path::new(&license_file.filename)
            .strip_prefix("info/licenses")
            .with_context(|| format!("Unexpected license path: {}", license_file.filename))?;

        let file_path = package_path.join(relative_path);

        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory for: {:?}", file_path))?;
        }

        let mut file = std::fs::File::create(&file_path)
            .with_context(|| format!("Failed to create license file: {:?}", file_path))?;
        file.write_all(license_file.license_text.as_bytes())
            .with_context(|| format!("Failed to write license text to file: {:?}", file_path))?;
    }
    Ok(())
}

async fn download_conda_package_as_cursor(url: Url) -> Result<std::io::Cursor<Vec<u8>>> {
    let auth_middleware = AuthenticationMiddleware::from_env_and_defaults()
        .with_context(|| "Failed to set up authentication middleware.")?;
    let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::default())
        .with_arc(Arc::new(auth_middleware))
        .build();

    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!(format!(
            "Failed to download the conda package: {}",
            response.status()
        )));
    }

    let bytes = response.bytes().await?;
    Ok(std::io::Cursor::new(bytes.to_vec()))
}

async fn get_license_files_from_url(url: Url) -> Result<Vec<LicenseFile>> {
    let cursor = download_conda_package_as_cursor(url.clone()).await?;
    if url.path().ends_with(".conda") {
        license_files_from_dot_conda(cursor)
    } else {
        license_files_from_tarbz2(cursor)
    }
}

fn extract_license_files<R: Read>(archive: &mut tar::Archive<R>) -> Result<Vec<LicenseFile>> {
    let mut licenses = Vec::new();

    for entry in archive.entries()? {
        let mut file = entry?;
        let path = file.path()?.to_path_buf();
        if file.header().entry_type().is_file() && path.starts_with("info/licenses") {
            let mut content = String::new();
            file.read_to_string(&mut content)?;

            let filename = path.to_string_lossy().to_string();

            licenses.push(LicenseFile {
                package_name: None,
                filename,
                license_text: content,
            });
        }
    }

    Ok(licenses)
}

fn license_files_from_tarbz2<R: Read + Seek>(mut reader: R) -> Result<Vec<LicenseFile>> {
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    let reader = Cursor::new(buffer);

    let mut archive = stream_tar_bz2(reader);

    extract_license_files(&mut archive)
}

fn license_files_from_dot_conda<R: Read + Seek>(mut reader: R) -> Result<Vec<LicenseFile>> {
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    let reader = Cursor::new(buffer);

    let mut content_archive = stream_conda_content(reader.clone())
        .with_context(|| "Streaming .conda package content from archive failed.")?;
    let mut info_archive = stream_conda_info(reader)
        .with_context(|| "Streaming .conda package info from archive failed.")?;

    let mut license_files = extract_license_files(&mut content_archive)?;
    license_files.extend(extract_license_files(&mut info_archive)?);

    Ok(license_files)
}

fn setup_bundle_bar(steps: u64) -> ProgressBar {
    let bar = ProgressBar::new(steps);

    bar.set_style(
        ProgressStyle::with_template(
            "{msg}\n{spinner:.green} [{elapsed_precise}] {bar:40.yellow} {pos:>7}/{len:7}",
        )
        .unwrap()
        .progress_chars("##-"),
    );

    bar
}

#[cfg(test)]
mod tests {
    use std::{fs::File, path::Path};

    use super::*;

    #[tokio::test]
    async fn test_in_memory_license_file_extraction() {
        let url = Url::parse(
            "https://conda.anaconda.org/conda-forge/linux-64/pysocks-1.7.1-py39hf3d152e_5.tar.bz2",
        )
        .unwrap();
        let license_files = get_license_files_from_url(url).await.unwrap();

        println!("This test is running");

        for license_file in &license_files {
            println!("Filename: {}", license_file.filename);
            println!("License Text: {}", license_file.license_text);
        }
        assert!(!license_files.is_empty());
    }

    #[test]
    fn test_bundle_conda_prefix() {
        let path = Path::new("/Users/pkm/micromamba/envs/test-bundle");
        let file = File::open(path).unwrap();
        let license_files = license_files_from_dot_conda(file).unwrap();
        for license_file in &license_files {
            println!("Filename: {}", license_file.filename);
            println!("License Text: {}", license_file.license_text);
        }
    }
}
