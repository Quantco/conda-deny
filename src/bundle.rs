use scopeguard::defer;
use std::fs::{self, create_dir_all, File};
use std::io::{self, copy, BufReader};
use std::path::{Path, PathBuf};

use bzip2::read::BzDecoder;

use anyhow::{Context, Result};
use log::{debug, warn};
use reqwest::blocking::get;
use tar::Archive;
use zip::ZipArchive;
use zstd::Decoder;

type LicenseFileName = String;
type LicenseText = String;

pub fn get_license_contents_for_package_url(url: &str) -> Result<Vec<(String, String)>> {
    let file_name = Path::new(url)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to extract file name from URL"))?;

    let output_dir = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get file stem as str"))?;

    download_file(url, file_name)?;
    defer! {
        let _ = fs::remove_file(file_name);
    }
    unpack_conda_file(file_name)?;
    defer! {
        let _ = fs::remove_dir_all(output_dir);
    }
    let license_strings = get_licenses_from_unpacked_conda_package(output_dir)?;

    std::fs::remove_file(file_name)
        .with_context(|| format!("Failed to delete file {}", file_name))?;
    std::fs::remove_dir_all(output_dir)
        .with_context(|| format!("Failed to remove directory {}", output_dir))?;

    Ok(license_strings)
}

fn find_all_licenses_directories(root: &Path) -> Result<Vec<PathBuf>> {
    let mut licenses_dirs = Vec::new();
    visit_dir(root, &mut licenses_dirs)?;
    Ok(licenses_dirs)
}

fn visit_dir(path: &Path, licenses_dirs: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_dir() {
            if entry_path.file_name().unwrap() == "licenses" {
                licenses_dirs.push(entry_path.clone());
            } else {
                visit_dir(&entry_path, licenses_dirs)?;
            }
        }
    }
    Ok(())
}

fn get_licenses_from_unpacked_conda_package(
    unpacked_conda_package_dir: &str,
) -> Result<Vec<(LicenseFileName, LicenseText)>> {
    let mut license_strings = Vec::new();

    let licenses_dirs = find_all_licenses_directories(Path::new(unpacked_conda_package_dir))?;

    if !licenses_dirs.is_empty() {
        for licenses_dir in licenses_dirs {
            get_license_texts_for_dir(&licenses_dir, &mut license_strings).with_context(|| {
            format!(
                "Failed to get license content from {}. Does the licenses directory exist within the package?",
                licenses_dir.display()
            )
        })?;
        }
        if license_strings.is_empty() {
            warn!(
                "Warning: No license files found in {}. Adding default license message.",
                unpacked_conda_package_dir
            );
            license_strings.push((
                "NO LICENSE FOUND".to_string(),
                "THE LICENSE OF THIS PACKAGE IS NOT PACKAGED!".to_string(),
            ));
        }
    } else {
        warn!(
            "Warning: No 'info/licenses' directory found in {}. Adding default license message.",
            unpacked_conda_package_dir
        );
        license_strings.push((
            "NO LICENSE FOUND".to_string(),
            "THE LICENSE OF THIS PACKAGE IS NOT PACKAGED!".to_string(),
        ));
    }

    license_strings.sort();
    license_strings.dedup();

    Ok(license_strings)
}

fn get_license_texts_for_dir(
    path: &Path,
    license_strings: &mut Vec<(LicenseFileName, LicenseText)>,
) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_dir() {
            get_license_texts_for_dir(&entry_path, license_strings)?;
        } else {
            let entry_file_name = entry.file_name().to_string_lossy().to_string();
            let content = fs::read_to_string(&entry_path)
                .with_context(|| format!("Failed to read {:?}", entry_path))?;
            license_strings.push((entry_file_name, content));
        }
    }
    Ok(())
}

fn download_file(url: &str, file_path: &str) -> Result<()> {
    let response = get(url).with_context(|| format!("Failed to download {}", file_path))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download file: {}",
            response.status()
        ));
    }

    let mut dest = File::create(file_path)
        .with_context(|| format!("File at {} could not be created", file_path))?;
    let content = response.bytes()?;

    copy(&mut content.as_ref(), &mut dest)?;

    debug!("File downloaded successfully to {}", file_path);
    Ok(())
}

fn unpack_conda_file(file_path: &str) -> Result<()> {
    let output_dir = Path::new(file_path)
        .file_stem()
        .map(PathBuf::from)
        .expect("Failed to get file stem");

    let file_extension = Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    match file_extension {
        "conda" => unpack_conda_archive(file_path, &output_dir),
        "bz2" => unpack_tar_bz2_archive(file_path, &output_dir),
        other => Err(anyhow::anyhow!(format!(
            "Unsupported file extension: {}",
            other
        ))),
    }
}

fn unpack_conda_archive(file_path: &str, output_dir: &Path) -> Result<()> {
    let zip_file =
        File::open(file_path).with_context(|| format!("Failed to open {}", file_path))?;
    let mut zip = ZipArchive::new(BufReader::new(zip_file))
        .with_context(|| "Failed to create zip archive")?;

    for i in 0..zip.len() {
        let mut zip_file = zip.by_index(i)?;
        if zip_file.name().ends_with(".tar.zst") {
            let mut tar_zst_data = Vec::new();
            io::copy(&mut zip_file, &mut tar_zst_data)?;

            let mut decoder = Decoder::new(&tar_zst_data[..])?;
            let mut tar_data = Vec::new();
            io::copy(&mut decoder, &mut tar_data)?;

            let mut tar = Archive::new(&tar_data[..]);
            create_dir_all(output_dir).with_context(|| {
                format!(
                    "Failed to create directory {}",
                    output_dir.to_string_lossy()
                )
            })?;
            tar.unpack(output_dir)
                .with_context(|| format!("Failed to unpack {}", output_dir.to_string_lossy()))?;
            debug!("Successfully unpacked to {:?}", output_dir);
        }
    }
    Ok(())
}

fn unpack_tar_bz2_archive(file_path: &str, output_dir: &Path) -> Result<()> {
    let tar_bz2_file =
        File::open(file_path).with_context(|| format!("Failed to open {}", file_path))?;
    let bz2_decoder = BzDecoder::new(tar_bz2_file);
    let mut tar = Archive::new(bz2_decoder);

    create_dir_all(output_dir).with_context(|| {
        format!(
            "Failed to create directory {}",
            output_dir.to_string_lossy()
        )
    })?;
    tar.unpack(output_dir)
        .with_context(|| format!("Failed to unpack {}", output_dir.to_string_lossy()))?;
    debug!("Successfully unpacked .tar.bz2 to {:?}", output_dir);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_all_licenses_directories() {
        let root = Path::new("tests/test_bundle_data/polarify-0.2.0-pyhd8ed1ab_0.conda");
        let result = find_all_licenses_directories(root).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains(&PathBuf::from("tests/test_bundle_data/polarify-0.2.0-pyhd8ed1ab_0.conda/pkg-polarify-0.2.0-pyhd8ed1ab_0/site-packages/polarify-0.2.0.dist-info/licenses")));
        assert!(result.contains(&PathBuf::from("tests/test_bundle_data/polarify-0.2.0-pyhd8ed1ab_0.conda/pkg-polarify-0.2.0-pyhd8ed1ab_0/info/licenses")));

        let root = Path::new("tests/test_bundle_data/_libgcc_mutex-0.1-free");
        let result = find_all_licenses_directories(root).unwrap();

        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_get_licenses_from_unpacked_conda_package_with_license_files() {
        let unpacked_conda_dir =
            Path::new("tests/test_bundle_data/polarify-0.2.0-pyhd8ed1ab_0.conda");

        let result =
            get_licenses_from_unpacked_conda_package(unpacked_conda_dir.to_str().unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.contains(&(
            String::from("LICENSE"),
            String::from("This is the license.")
        )));
    }

    #[test]
    fn test_get_licenses_from_unpacked_conda_package_without_licenses_directory() {
        let unpacked_conda_dir = Path::new("tests/test_bundle_data/_libgcc_mutex-0.1-free");

        let result =
            get_licenses_from_unpacked_conda_package(unpacked_conda_dir.to_str().unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.contains(&(
            "NO LICENSE FOUND".to_string(),
            "THE LICENSE OF THIS PACKAGE IS NOT PACKAGED!".to_string()
        )));
    }

    #[test]
    fn test_get_licenses_from_unpacked_conda_package_empty_liceses_dir() {
        let unpacked_conda_dir = Path::new("tests/test_bundle_data/empty_licenses_dir");

        let result =
            get_licenses_from_unpacked_conda_package(unpacked_conda_dir.to_str().unwrap()).unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.contains(&(
            "NO LICENSE FOUND".to_string(),
            "THE LICENSE OF THIS PACKAGE IS NOT PACKAGED!".to_string()
        )));
    }
}
