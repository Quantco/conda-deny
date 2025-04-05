use anyhow::{anyhow, Context, Result};
use rattler_lock::UrlOrPath;
use rattler_package_streaming::{
    read::stream_tar_bz2,
    seek::{stream_conda_content, stream_conda_info},
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::io::{copy, Cursor, Read, Seek, SeekFrom, Write};
use tempfile::NamedTempFile;

use crate::{pixi_lock::get_conda_packages_for_pixi_lock, CondaDenyBundleConfig, LockfileOrPrefix};

pub fn bundle<W: Write>(config: CondaDenyBundleConfig, mut out: W) -> Result<()> {
    let lockfile_or_prefix = config.lockfile_or_prefix.clone();

    let mut license_files: Vec<LicenseFile> = Vec::new();

    match lockfile_or_prefix {
        LockfileOrPrefix::Lockfile(lockfile_spec) => {
            let lockfiles = lockfile_spec.lockfiles.clone();
            for lockfile in lockfiles {
                let conda_packages = get_conda_packages_for_pixi_lock(
                    &lockfile,
                    &lockfile_spec.environments,
                    &lockfile_spec.platforms,
                    lockfile_spec.ignore_pypi,
                )?;

                // println!("conda_packages: {:?}", conda_packages);

                for conda_package in conda_packages {
                    let package_path = conda_package.location();
                    println!(
                        "Package: {:?}",
                        conda_package.location().as_url().unwrap().to_string()
                    );
                    match package_path {
                        UrlOrPath::Url(url) => {
                            let mut temp_file = download_conda_package_as_tempfile(url.clone())
                                .with_context(|| {
                                    format!(
                                        "Downloading .conda package to tempfile failed: {}",
                                        url
                                    )
                                })?;
                            temp_file
                                .seek(SeekFrom::Start(0))
                                .with_context(|| "Resetting temp file cursor failed.")?;

                            if url.to_string().ends_with(".tar.bz2") {
                                let current_package_license_files =
                                    license_files_from_tarbz2(temp_file).with_context(|| format!(
                                        "Getting license files from the following tar.bz2 package failed: {}", conda_package.record().name.as_source())
                                    )?;

                                for license_file in current_package_license_files.clone() {
                                    println!(
                                        "Package: {:?} Filename: {}",
                                        conda_package.record().name,
                                        license_file.filename
                                    );
                                }
                                license_files.extend(current_package_license_files);
                            } else if url.to_string().ends_with(".conda") {
                                let current_package_license_files =
                                    license_files_from_conda_package(temp_file).with_context(|| format!(
                                        "Getting license files from the following conda package failed: {}", conda_package.record().name.as_source())
                                    )?;

                                for license_file in current_package_license_files.clone() {
                                    println!(
                                        "Package: {:?} Filename: {}",
                                        conda_package.record().name,
                                        license_file.filename
                                    );
                                }
                                license_files.extend(current_package_license_files);
                            } else {
                                return Err(anyhow!(
                                    "Unsupported package type format (not .conda or .tar.bz2)"
                                ));
                            }
                        }
                        UrlOrPath::Path(path) => {
                            todo!("Handle local path: {:?}", path);
                            // TODO: Implement logic for handling local paths
                        }
                    }
                }
            }
        }
        LockfileOrPrefix::Prefix(prefix_path) => {
            println!("Prefix path: {:?}", prefix_path);
            // TODO: Implement logic for handling prefix paths
        }
    }
    println!("Number of license files: {:?}", license_files.len());

    println!("The end of this function is reached.");
    Ok(())
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

fn license_files_from_conda_package<R: Read + Seek>(mut reader: R) -> Result<Vec<LicenseFile>> {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LicenseFile {
    filename: String,
    license_text: String,
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

        license_files_from_conda_package(temp_file).unwrap();
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
