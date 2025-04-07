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
                    // Set package name of all returned license files

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

                    let package_name = format!("{}-{}-{}", conda_package.record().name.as_source(), conda_package.record().version, conda_package.record().build);
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
            serde_json::to_writer_pretty(&mut out, &license_files)
                .with_context(|| "Failed to write license files to JSON")?;
        }
        OutputFormat::JsonPretty => {
            serde_json::to_writer(&mut out, &license_files)
                .with_context(|| "Failed to write license files to pretty JSON")?;
        }
        OutputFormat::Csv => {
            let mut wtr = csv::Writer::from_writer(out);
            for license_file in &license_files {
                wtr.serialize(license_file)
                    .with_context(|| "Failed to serialize license file to CSV")?;
            }
            wtr.flush().with_context(|| "Failed to flush CSV writer")?;
        }
        OutputFormat::Default => {
            for license_file in &license_files {
                writeln!(out, "{:?}", license_files).with_context(|| {
                    format!("Failed to write license file: {:?}", license_file.filename)
                })?;
            }
        }
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
        if file.header().entry_type().is_file()
            && path.components().any(|c| {
                c.as_os_str() == "licenses"
                    || c.as_os_str() == "LICENSE"
                    || c.as_os_str() == "copying"
            })
        {
            let mut content = String::new();
            file.read_to_string(&mut content)?;

            let filename = path
                .file_name()
                .ok_or_else(|| anyhow!("Invalid license file path {:?}", path))?
                .to_string_lossy()
                .to_string();

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
