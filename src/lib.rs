pub mod cli;
pub mod conda_deny_config;
mod conda_meta_entry;
mod conda_meta_package;
mod expression_utils;
mod license_info;
pub mod license_whitelist;
mod list;
mod pixi_lock;
mod read_remote;

use std::fs::{self, create_dir_all, File};
use std::io::{self, copy, BufReader};
use std::path::{Path, PathBuf};

use bzip2::read::BzDecoder;
use cli::{combine_cli_and_config_input, Cli, Commands};
use colored::Colorize;
use license_info::LicenseInfo;
use license_whitelist::build_license_whitelist;

use anyhow::{Context, Result};
use log::{debug, warn};
use pixi_lock::get_conda_packages_for_pixi_lock;
use rattler_lock::CondaPackage;
use reqwest::blocking::get;
use tar::Archive;
use zip::ZipArchive;
use zstd::Decoder;

use crate::conda_deny_config::CondaDenyConfig;
use crate::license_info::LicenseInfos;

pub type CheckOutput = (Vec<LicenseInfo>, Vec<LicenseInfo>);

#[derive(Clone)]
pub struct CondaDenyInput {
    pub config: CondaDenyConfig,
    pub cli_lockfiles: Vec<String>,
    pub cli_platforms: Vec<String>,
    pub cli_environments: Vec<String>,
    pub conda_prefixes: Vec<String>,
    pub osi: bool,
}

pub fn collect_license_infos(conda_deny_input: CondaDenyInput) -> Result<LicenseInfos> {
    if conda_deny_input.conda_prefixes.is_empty() {
        let (lockfiles, platforms, environment_specs) = combine_cli_and_config_input(
            &conda_deny_input.config,
            &conda_deny_input.cli_lockfiles,
            &conda_deny_input.cli_platforms,
            &conda_deny_input.cli_environments,
        );

        LicenseInfos::from_pixi_lockfiles(lockfiles, platforms, environment_specs)
            .with_context(|| "Getting license information from config file failed.")
    } else {
        LicenseInfos::from_conda_prefixes(&conda_deny_input.conda_prefixes)
            .with_context(|| "Getting license information from conda prefixes failed.")
    }
}

pub fn list(cli_input: CondaDenyInput) -> Result<()> {
    let license_infos =
        collect_license_infos(cli_input).with_context(|| "Fetching license information failed.")?;
    license_infos.list();
    Ok(())
}

pub fn check_license_infos(conda_deny_input: CondaDenyInput) -> Result<CheckOutput> {
    let license_infos = collect_license_infos(conda_deny_input.clone())
        .with_context(|| "Fetching license information failed.")?;

    if conda_deny_input.clone().osi.to_owned() {
        debug!("Checking licenses for OSI compliance");
        Ok(license_infos.osi_check())
    } else {
        let license_whitelist = build_license_whitelist(&conda_deny_input.config)
            .with_context(|| "Building the license whitelist failed.")?;
        debug!("Checking licenses against specified whitelist");
        Ok(license_infos.check(&license_whitelist))
    }
}

pub fn format_check_output(
    safe_dependencies: Vec<LicenseInfo>,
    unsafe_dependencies: Vec<LicenseInfo>,
    include_safe_dependencies: bool,
) -> String {
    let mut output = String::new();

    if include_safe_dependencies && !safe_dependencies.is_empty() {
        output.push_str(
            format!(
                "\n✅ {}:\n\n",
                "The following dependencies are safe".green()
            )
            .as_str(),
        );
        for license_info in &safe_dependencies {
            output.push_str(&license_info.pretty_print(true))
        }
    }

    if !unsafe_dependencies.is_empty() {
        output.push_str(
            format!(
                "\n❌ {}:\n\n",
                "The following dependencies are unsafe".red()
            )
            .as_str(),
        );
        for license_info in &unsafe_dependencies {
            output.push_str(&license_info.pretty_print(true))
        }
    }

    if unsafe_dependencies.is_empty() {
        output.push_str(&format!(
            "\n{}",
            "✅ No unsafe licenses found! ✅".to_string().green()
        ));
    } else {
        output.push_str(&format!(
            "\n{}",
            "❌ Unsafe licenses found! ❌".to_string().red()
        ));
    }

    output.push_str(&format!(
        "\nThere were {} safe licenses and {} unsafe licenses.\n",
        safe_dependencies.len().to_string().green(),
        unsafe_dependencies.len().to_string().red()
    ));

    output.push('\n');

    output
}

pub fn parse_cli_input(cli: Cli) -> Result<CondaDenyInput> {
    let osi = match cli.command {
        Commands::Check { osi, .. } => osi,
        _ => false,
    };

    let mut config = CondaDenyConfig::empty();

    if !osi {
        config = if let Some(config_path) = cli.config {
            CondaDenyConfig::from_path(config_path.as_str())
                .with_context(|| format!("Failed to parse config file {}", config_path))?
        } else {
            match CondaDenyConfig::from_path("pixi.toml")
                .with_context(|| "Failed to parse config file pixi.toml")
            {
                Ok(config) => {
                    debug!("Successfully loaded config from pixi.toml");
                    config
                }
                Err(e) => {
                    debug!(
                    "Error parsing config file: pixi.toml: {}. Attempting to use pyproject.toml instead...",
                    e
                );
                    CondaDenyConfig::from_path("pyproject.toml")
                        .context(e)
                        .with_context(|| "Failed to parse config file pyproject.toml")?
                }
            }
        };
    } else {
        debug!("Skipping config file parsing for OSI compliance check. Your {} section will be ignored.", "[tool.conda-deny]".yellow());
    }

    let conda_prefixes = cli.prefix.unwrap_or_default();
    let cli_lockfiles = cli.lockfile.unwrap_or_default();
    let cli_platforms = cli.platform.unwrap_or_default();
    let cli_environments = cli.environment.unwrap_or_default();

    debug!("CLI input for platforms: {:?}", cli_platforms);
    debug!("CLI input for environments: {:?}", cli_environments);
    debug!("CLI input for conda prefixes: {:?}", conda_prefixes);
    let mut locks_to_check = cli_lockfiles.clone();
    locks_to_check.push("pixi.lock".to_string());
    debug!("CLI input for pixi lockfiles: {:?}", locks_to_check);
    debug!("CLI input for OSI compliance: {}", osi);

    let cli_input = CondaDenyInput {
        config: config.clone(),
        cli_lockfiles: locks_to_check.clone(),
        cli_platforms: cli_platforms.clone(),
        cli_environments: cli_environments.clone(),
        conda_prefixes: conda_prefixes.clone(),
        osi,
    };

    Ok(cli_input)
}

pub fn bundle(conda_deny_input: CondaDenyInput) -> Result<()> {
    let (lockfiles, platforms, environment_specs) = combine_cli_and_config_input(
        &conda_deny_input.config,
        &conda_deny_input.cli_lockfiles,
        &conda_deny_input.cli_platforms,
        &conda_deny_input.cli_environments,
    );
    let mut conda_packages = Vec::new();

    for lockfile in lockfiles {
        let lockfile_path = Path::new(&lockfile);
        let packages_for_lockfile = get_conda_packages_for_pixi_lock(
            Some(lockfile_path),
            environment_specs.clone(),
            platforms.clone(),
        );
        conda_packages.extend(packages_for_lockfile?);
    }

    std::fs::create_dir_all("licenses").with_context(|| "Failed to create licenses directory")?;
    std::fs::remove_dir_all("licenses").with_context(|| "Failed to create licenses directory")?;
    std::fs::create_dir_all("licenses").with_context(|| "Failed to create licenses directory")?;

    debug!("Bundling licenses...");
    for (conda_package, environment) in conda_packages {
        let conda_package_path = conda_package.file_name().unwrap();

        let conda_package_dir = Path::new(conda_package_path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("Failed to get file stem as str");
        let license_files =
            get_license_contents_for_package(&conda_package).with_context(|| {
                format!(
                    "Failed to get license contents for package {}",
                    conda_package.url()
                )
            })?;

        for (license_file_name, license_file_contents) in license_files {
            let license_file_path = format!(
                "licenses/{}/{}/{}",
                environment.clone().unwrap(),
                conda_package_dir,
                license_file_name
            );

            if !Path::new(&license_file_path).exists() {
                std::fs::create_dir_all(Path::new(&license_file_path).parent().unwrap())?;
                std::fs::write(license_file_path.clone(), license_file_contents.clone())
                    .with_context(|| format!("Writing the file {}", license_file_path))?;
                debug!("License file written to {}", license_file_path);
            } else {
                let existing_license_file_contents = std::fs::read_to_string(&license_file_path)?;
                if existing_license_file_contents != license_file_contents {
                    debug!("License file {} already exists and is different from the current license. Appending the new license to the file.", license_file_path);
                    std::fs::write(
                        license_file_path.clone(),
                        format!(
                            "{}\n\n{}",
                            existing_license_file_contents, license_file_contents
                        ),
                    )
                    .with_context(|| format!("Writing the file {}", license_file_path))?;
                }
            }
        }
    }
    Ok(())
}

fn get_license_contents_for_package(conda_package: &CondaPackage) -> Result<Vec<(String, String)>> {
    let file_path = match conda_package.file_name() {
        Some(file_path) => file_path,
        None => {
            return Err(anyhow::anyhow!(
                "Failed to get file path for {}",
                conda_package.url().to_string()
            ))
        }
    };
    let output_dir = Path::new(file_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("Failed to get file stem as str");

    let url = format!("{}", conda_package.url());
    download_file(&url, conda_package.file_name().unwrap())?;
    unpack_conda_file(conda_package.file_name().unwrap())?;

    let license_strings = get_licenses_from_unpacked_conda_package(output_dir)?;

    std::fs::remove_file(file_path)
        .with_context(|| format!("Failed to delete file {}", file_path))?;
    std::fs::remove_dir_all(output_dir)
        .with_context(|| format!("Failed to remove directory {}", output_dir))?;

    Ok(license_strings)
}

fn get_licenses_from_unpacked_conda_package(output_dir: &str) -> Result<Vec<(String, String)>> {
    let mut license_strings = Vec::new();
    let licenses_dir = format!("{}/info/licenses", output_dir);

    let licenses_path = Path::new(&licenses_dir);
    if !licenses_path.exists() {
        warn!(
            "Warning: No 'info/licenses' directory found in {}. Adding default license message.",
            output_dir
        );
        license_strings.push((
            "NO LICENSE FOUND".to_string(),
            "THE LICENSE OF THIS PACKAGE IS NOT PACKAGED!".to_string(),
        ));
        return Ok(license_strings);
    }

    fn visit_dir(path: &Path, license_strings: &mut Vec<(String, String)>) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();

            if entry_path.is_dir() {
                visit_dir(&entry_path, license_strings)?;
            } else {
                let entry_file_name = entry.file_name().to_string_lossy().to_string();
                let content = fs::read_to_string(&entry_path)
                    .with_context(|| format!("Failed to read {:?}", entry_path))?;
                license_strings.push((entry_file_name, content));
            }
        }
        Ok(())
    }

    visit_dir(licenses_path, &mut license_strings).with_context(|| {
        format!(
            "Failed to get license content from {}. Does the licenses directory exist within the package?",
            licenses_dir
        )
    })?;

    Ok(license_strings)
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
        _ => Err(anyhow::anyhow!("Unsupported file extension")),
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
            break;
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
