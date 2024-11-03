mod bundle;
pub mod cli;
pub mod conda_deny_config;
mod conda_meta_entry;
mod conda_meta_package;
mod expression_utils;
mod license_info;
pub mod license_whitelist;
mod list;
mod pixi_lock;
mod prefix;
mod read_remote;

use rayon::prelude::*;
use std::path::Path;

use bundle::get_license_contents_for_package_url;
use cli::{combine_cli_and_config_input, Cli, Commands};
use colored::Colorize;
use license_info::LicenseInfo;
use license_whitelist::build_license_whitelist;

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use log::debug;
use pixi_lock::get_conda_packages_for_pixi_lock;

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

pub fn parse_cli_input(cli: &Cli) -> Result<CondaDenyInput> {
    let osi = match cli.command {
        Commands::Check { osi, .. } => osi,
        _ => false,
    };

    let mut config = CondaDenyConfig::empty();

    if !osi {
        config = if let Some(config_path) = cli.config.clone() {
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

    let conda_prefixes = cli.prefix.clone().unwrap_or_default();
    let cli_lockfiles = cli.lockfile.clone().unwrap_or_default();
    let cli_platforms = cli.platform.clone().unwrap_or_default();
    let cli_environments = cli.environment.clone().unwrap_or_default();

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

pub fn bundle(conda_deny_input: CondaDenyInput, output_dir: Option<String>) -> Result<()> {
    let (lockfiles, platforms, environment_specs) = combine_cli_and_config_input(
        &conda_deny_input.config,
        &conda_deny_input.cli_lockfiles,
        &conda_deny_input.cli_platforms,
        &conda_deny_input.cli_environments,
    );

    let conda_prefixes = conda_deny_input.conda_prefixes.clone();

    let bundle_dir = output_dir.unwrap_or_else(|| "licenses".to_string());

    std::fs::create_dir_all(bundle_dir.clone())
        .with_context(|| "Failed to create licenses directory")?;
    std::fs::remove_dir_all(bundle_dir.clone())
        .with_context(|| "Failed to create licenses directory")?;
    std::fs::create_dir_all(bundle_dir.clone())
        .with_context(|| "Failed to create licenses directory")?;

    if conda_prefixes.is_empty() {
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

        let bar = setup_bundle_bar(conda_packages.len() as u64);

        let _bundling_result: Result<()> = conda_packages.par_iter().try_for_each(|(conda_package, environment)| {
            bar.inc(1);

            let conda_package_path = conda_package.file_name().unwrap();

            let conda_package_dir = Path::new(conda_package_path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("Failed to get file stem as str");

            let license_files = get_license_contents_for_package_url(conda_package.url().as_str())
                .with_context(|| {
                    format!(
                        "Failed to get license contents for package {}",
                        conda_package.url()
                    )
                })?;

            license_files.into_par_iter().try_for_each(|(license_file_name, license_file_contents)| {
                let license_file_path = format!(
                    "{}/{}/{}/{}",
                    bundle_dir,
                    environment.clone().unwrap(),
                    conda_package_dir,
                    license_file_name
                );

                if !Path::new(&license_file_path).exists() {
                    let parent_dir = Path::new(&license_file_path)
                        .parent()
                        .with_context(|| format!("Failed to get parent directory of {}", license_file_path))?;

                    std::fs::create_dir_all(parent_dir).with_context(|| format!("Failed to create directory: {:?}", parent_dir))?;

                    std::fs::write(license_file_path.clone(), license_file_contents.clone())
                        .with_context(|| format!("Writing the file {}", license_file_path))?;
                    debug!("License file written to {}", license_file_path);
                } else {
                    let existing_license_file_contents = std::fs::read_to_string(&license_file_path).with_context(|| format!("Failed to read file: {}", license_file_path))?;
                    if existing_license_file_contents != *license_file_contents {
                        debug!(
                            "License file {} already exists and is different from the current license. Appending the new license to the file.",
                            license_file_path
                        );
                        std::fs::write(
                            license_file_path.clone(),
                            format!("{}\n\n{}", existing_license_file_contents, license_file_contents)
                        )
                        .with_context(|| format!("Writing the file {}", license_file_path))?;
                    }
                }
                Ok::<(), anyhow::Error>(())
            })?;
            Ok(())
        });

        bar.finish_with_message("✅ Bundling licenses complete!");
        bar.finish();
    } else {
        let package_urls_for_prefix = prefix::get_package_urls_for_prefixes(conda_prefixes)?;
        let bar = setup_bundle_bar(package_urls_for_prefix.len() as u64);

        let _bundling_result: Result<()> = package_urls_for_prefix.par_iter().map(|package_url| {
            bar.inc(1);

            let conda_package_path = Path::new(&package_url)
                .file_name()
                .expect("Failed to get file name")
                .to_str()
                .expect("Failed to convert file name to str");

            let conda_package_dir = Path::new(conda_package_path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("Failed to get file stem as str");
            let license_files =
                get_license_contents_for_package_url(package_url).with_context(|| {
                    format!("Failed to get license contents for package {}", package_url)
                })?;

            for (license_file_name, license_file_contents) in license_files {
                let license_file_path =
                    format!("{}/{}/{}", bundle_dir, conda_package_dir, license_file_name);

                if !Path::new(&license_file_path).exists() {
                    std::fs::create_dir_all(Path::new(&license_file_path).parent().unwrap())?;
                    std::fs::write(license_file_path.clone(), license_file_contents.clone())
                        .with_context(|| format!("Writing the file {}", license_file_path))?;
                    debug!("License file written to {}", license_file_path);
                } else {
                    let existing_license_file_contents =
                        std::fs::read_to_string(&license_file_path)?;
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
            Ok(())
        }).collect();
        bar.finish_with_message("✅ Bundling licenses complete!");
        bar.finish();
    }
    Ok(())
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

    bar.set_message("📦 Bundling licenses...");
    bar
}
