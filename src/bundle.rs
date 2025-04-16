use anyhow::{anyhow, Context, Result};
use log::{debug, trace, warn};
use rattler_conda_types::PrefixRecord;
use rattler_lock::CondaPackageData;
use rattler_package_streaming::{
    read::stream_tar_bz2,
    seek::stream_conda_info,
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use reqwest::Url;
use std::{
    io::{Cursor, Read, Seek, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use indicatif::{ProgressBar, ProgressStyle};

use rattler_networking::AuthenticationMiddleware;

use crate::{pixi_lock::get_conda_packages_for_pixi_lock, CondaDenyBundleConfig, LockfileOrPrefix};

type LicenseContents = (String, Vec<u8>);

#[derive(Debug, Clone)]
struct LicenseFile {
    package_name: String,
    filename: String,
    license_text: Vec<u8>,
}

pub fn bundle<W: Write>(config: CondaDenyBundleConfig, mut out: W) -> Result<()> {
    let lockfile_or_prefix = config.lockfile_or_prefix.clone();

    let license_files: Vec<LicenseFile> = match lockfile_or_prefix {
        LockfileOrPrefix::Lockfile(lockfile_spec) => {
            let lockfiles = lockfile_spec.lockfiles.clone();
            let mut conda_packages = vec![];
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

            bundle_license_files(
                conda_packages,
                |pkg: &CondaPackageData| {
                    pkg.location()
                        .as_url()
                        .ok_or_else(|| {
                            anyhow!(format!("URL for package could not be resolved: {:?}", pkg))
                        })
                        .cloned()
                },
                |pkg| pkg.record().name.as_source().to_string(),
                |pkg| {
                    format!(
                        "{}-{}-{}",
                        pkg.record().name.as_source(),
                        pkg.record().version,
                        pkg.record().build
                    )
                },
            )?
        }
        LockfileOrPrefix::Prefix(prefix_paths) => {
            let mut prefix_records = vec![];
            for prefix in prefix_paths {
                let recs = PrefixRecord::collect_from_prefix(prefix.as_path())
                    .with_context(|| format!("Failed to collect from: {:?}", prefix))?;
                prefix_records.extend(recs);
            }

            bundle_license_files(
                prefix_records,
                |rec: &PrefixRecord| Ok(rec.repodata_record.url.clone()),
                |rec: &PrefixRecord| rec.file_name(),
                |rec| {
                    Path::new(&rec.file_name())
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                },
            )?
        }
    };

    let path = config.directory.unwrap_or(PathBuf::from("bundle"));
    let path = Path::new(&path);
    create_license_file_directory(path, license_files)
        .with_context(|| format!("Failed to create license file directory: {:?}", path))?;
    writeln!(out, "License files written to: {:?}", path)
        .with_context(|| format!("Failed to write license file: {:?}", path))?;

    Ok(())
}

fn bundle_license_files<I>(
    items: I,
    get_url: impl Fn(&I::Item) -> Result<Url> + Sync,
    get_name: impl Fn(&I::Item) -> String + Sync,
    get_filename: impl Fn(&I::Item) -> String + Sync,
) -> Result<Vec<LicenseFile>>
where
    I: IntoIterator + Send + Sync,
    I::Item: Sync,
{
    let items: Vec<_> = items.into_iter().collect();
    let bar = setup_bundle_bar(items.len() as u64);

    let license_files = items
        .par_iter()
        .map(|item| -> Result<Vec<LicenseFile>> {
            bar.inc(1);
            bar.set_message(format!("📦 Bundling licenses for: {}", get_name(item)));

            let rt = tokio::runtime::Runtime::new()?;

            let url = get_url(item)?;
            let files = rt
                .block_on(get_license_files_from_url(url))
                .with_context(|| format!("Failed to process conda package: {}", get_name(item)))?;

            warn!("Received {} license files for {}", files.len(), get_name(item));
            trace!("License files: {:?}", files);

            let package_name = get_filename(item);
            Ok(files
                .into_iter()
                .map(|(filename, license_text)| LicenseFile {
                    package_name: package_name.clone(),
                    filename,
                    license_text,
                })
                .collect())
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();

    bar.finish_with_message("✅ Bundling licenses complete!");
    Ok(license_files)
}

fn create_license_file_directory(path: &Path, license_files: Vec<LicenseFile>) -> Result<()> {
    if path.exists() {
        warn!(
            "Output directory already exists. Removing and creating a new one: {:?}",
            path
        );
        std::fs::remove_dir_all(path)
            .with_context(|| format!("Failed to remove existing directory: {:?}", path))?;
    }
    std::fs::create_dir_all(path)
        .with_context(|| format!("Failed to create output directory: {:?}", path))?;
    for license_file in &license_files {
        let package_name = license_file.package_name.to_string();
        let package_path = path.join(package_name);
        std::fs::create_dir_all(&package_path)
            .with_context(|| format!("Failed to create package directory: {:?}", package_path))?;

        let relative_path = Path::new(&license_file.filename)
            .strip_prefix("info/licenses")
            .expect("All relative paths have info/licenses prefix");

        let file_path = package_path.join(relative_path);

        let parent = file_path.parent().expect("Parent dir always exists");
        std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory for: {:?}", file_path))?;

        let mut file = std::fs::File::create(&file_path)
            .with_context(|| format!("Failed to create license file: {:?}", file_path))?;
        file.write_all(
            &license_file
                .license_text
                .clone()
                .into_iter()
                .collect::<Vec<u8>>(),
        )
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

async fn get_license_files_from_url(url: Url) -> Result<Vec<LicenseContents>> {
    let cursor = download_conda_package_as_cursor(url.clone()).await?;
    if url.path().ends_with(".conda") {
        license_files_from_dot_conda(cursor)
    } else {
        license_files_from_tarbz2(cursor)
    }
}

fn extract_license_files<R: Read>(archive: &mut tar::Archive<R>) -> Result<Vec<LicenseContents>> {
    let mut files_and_license_text = Vec::new();

    for entry in archive.entries()? {
        let mut file = entry?;
        let path = file.path()?.to_path_buf();
        if file.header().entry_type().is_file() && path.starts_with("info/licenses") {
            let mut content: Vec<u8> = Vec::new();
            file.read_to_end(&mut content)?;

            let filename = path.to_string_lossy().to_string();

            files_and_license_text.push((filename.clone(), content.clone()));
        }
    }

    Ok(files_and_license_text)
}

fn license_files_from_tarbz2<R: Read + Seek>(mut reader: R) -> Result<Vec<LicenseContents>> {
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    let reader = Cursor::new(buffer);

    let mut archive = stream_tar_bz2(reader);

    extract_license_files(&mut archive)
}

fn license_files_from_dot_conda<R: Read + Seek>(mut reader: R) -> Result<Vec<LicenseContents>> {
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    let reader = Cursor::new(buffer);

    let mut info_archive = stream_conda_info(reader)
        .with_context(|| "Streaming .conda package info from archive failed.")?;

    extract_license_files(&mut info_archive)
}

fn setup_bundle_bar(steps: u64) -> ProgressBar {
    if std::env::var("NO_PROGRESS").is_ok() {
        let pb = ProgressBar::hidden();
        pb.set_length(steps);
        return pb;
    }
    let bar = ProgressBar::new(steps);

    bar.set_style(
        ProgressStyle::with_template(
            "{msg}\n{spinner:.green} [{elapsed_precise}] {bar:40.yellow} {pos:>7}/{len:7}",
        )
        .expect("Failed to set progress bar template")
        .progress_chars("##-"),
    );

    bar
}
