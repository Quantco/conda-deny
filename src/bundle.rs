use anyhow::{anyhow, Context, Result};
use rattler_lock::{CondaPackageData, UrlOrPath};
use rattler_package_streaming::{
    read::stream_tar_bz2,
    seek::{stream_conda_content, stream_conda_info},
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::io::{copy, Cursor, Read, Seek, SeekFrom, Write};
use tempfile::NamedTempFile;

use indicatif::{ProgressBar, ProgressStyle};

use crate::{pixi_lock::get_conda_packages_for_pixi_lock, CondaDenyBundleConfig, LockfileOrPrefix};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LicenseFile {
    filename: String,
    license_text: String,
}

pub fn bundle<W: Write>(config: CondaDenyBundleConfig, mut out: W) -> Result<()> {
    let lockfile_or_prefix = config.lockfile_or_prefix.clone();

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

            let license_files: Vec<LicenseFile> = conda_packages
                .par_iter()
                .map(|conda_package| {
                    bar.inc(1);
                    bar.set_message(format!(
                        "📦 Bundling licenses for: {}",
                        conda_package.record().name.as_source()
                    ));
                    process_conda_package(conda_package)
                })
                .collect::<Result<Vec<Vec<LicenseFile>>, _>>()?
                .into_iter()
                .flatten()
                .collect();

            bar.finish_with_message("✅ Bundling licenses complete!");
        }
        LockfileOrPrefix::Prefix(prefix_path) => {
            println!("Prefix path: {:?}", prefix_path);
            // TODO: Implement logic for handling prefix paths
        }
    }
    Ok(())
}

fn process_conda_package(conda_package: &CondaPackageData) -> Result<Vec<LicenseFile>> {
    let url = match conda_package.location() {
        UrlOrPath::Url(url) => url,
        UrlOrPath::Path(_path) => return Err(anyhow!("Local paths not supported yet")),
    };

    let mut temp_file = download_conda_package_as_tempfile(url.clone())
        .with_context(|| format!("Downloading failed: {}", url))?;
    temp_file.seek(SeekFrom::Start(0))?;

    if url.to_string().ends_with(".tar.bz2") {
        license_files_from_tarbz2(temp_file)
    } else if url.to_string().ends_with(".conda") {
        license_files_from_dot_conda(temp_file)
    } else {
        Err(anyhow!("Unsupported package format: {}", url))
    }
}

fn download_conda_package_as_tempfile(url: Url) -> Result<tempfile::NamedTempFile> {
    let mut response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to download the conda package"));
    }

    let mut temp_file =
        NamedTempFile::new().with_context(|| "Creation of new NamedTempFile failed.")?;
    copy(&mut response, &mut temp_file)
        .with_context(|| "Copying downloaded .conda package to NamedTempFile failed.")?;
    Ok(temp_file)
}

fn extract_license_files<R: Read>(archive: &mut tar::Archive<R>) -> Result<Vec<LicenseFile>> {
    let mut licenses = Vec::new();

    for entry in archive.entries()? {
        let mut file = entry?;
        let path = file.path()?.to_path_buf();
        if file.header().entry_type().is_file()
            && path.components().any(|c| c.as_os_str() == "licenses")
        {
            let mut content = String::new();
            file.read_to_string(&mut content)?;

            let filename = path
                .file_name()
                .ok_or_else(|| anyhow!("Invalid license file path {:?}", path))?
                .to_string_lossy()
                .to_string();

            licenses.push(LicenseFile {
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
    use std::{fs::File, io::SeekFrom, path::Path};

    use super::*;

    #[test]
    fn test_download_conda_package() {
        let url = Url::parse(
            "https://conda.anaconda.org/conda-forge/linux-64/conda-deny-0.4.1-h53e704d_0.conda",
        )
        .unwrap();
        let mut temp_file = download_conda_package_as_tempfile(url).unwrap();
        assert!(temp_file.path().exists());

        // Store permanently for testing
        // let mut temp_file = temp_file.reopen().unwrap();
        // let mut file = File::create("tests/ruamel.yaml-0.18.6-py312h98912ed_0.conda").unwrap();
        // copy(&mut temp_file, &mut file).unwrap();

        temp_file.seek(SeekFrom::Start(0)).unwrap();

        license_files_from_dot_conda(temp_file).unwrap();
    }

    #[test]
    fn test_streaming() {
        let path = Path::new("tests/cached-property-1.5.2-hd8ed1ab_1.tar.bz2");
        // license_files_from_conda_package(File::open(path).unwrap()).unwrap();
        // let content = stream_conda_info(File::open(path).unwrap()).unwrap();
        // println!("Content: {:?}", content.entries().unwrap().);
        // let file = read_package_file::<LicenseFile>(path).unwrap();

        let license_files = license_files_from_tarbz2(File::open(path).unwrap()).unwrap();
        println!("License files: {:?}", license_files);
        println!("This is the test");
    }
}
