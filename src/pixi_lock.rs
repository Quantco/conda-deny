use std::path::Path;

use anyhow::{Context, Result};

use log::warn;
use rattler_conda_types::Platform;
use rattler_lock::{CondaPackageData, LockFile, LockedPackage, PypiPackageData};

use crate::license_allowlist::{
    is_package_ignored, is_package_ignored_by_name_only, IgnorePackage,
};

pub fn get_conda_packages_for_pixi_lock(
    pixi_lock_path: &Path,
    environment_spec: &Option<Vec<String>>,
    platform_spec: &Option<Vec<Platform>>,
    ignore_pypi: bool,
    ignore_packages: &[IgnorePackage],
) -> Result<Vec<CondaPackageData>> {
    let lock_file = LockFile::from_path(pixi_lock_path)
        .with_context(|| format!("Failed to read pixi.lock file: {pixi_lock_path:?}"))?;
    let environment_spec = environment_spec
        .clone()
        .unwrap_or_else(|| _get_environment_names(&lock_file));
    let mut package_records = Vec::new();

    for environment_name in environment_spec {
        package_records.extend(collect_conda_packages_for_environment(
            &lock_file,
            &environment_name,
            platform_spec,
            ignore_pypi,
            ignore_packages,
        )?);
    }

    Ok(package_records)
}

fn collect_conda_packages_for_environment(
    lock_file: &LockFile,
    environment_name: &str,
    platform_spec: &Option<Vec<Platform>>,
    ignore_pypi: bool,
    ignore_packages: &[IgnorePackage],
) -> Result<Vec<CondaPackageData>> {
    let environment = lock_file.environment(environment_name).ok_or_else(|| {
        anyhow::anyhow!("Environment not found in lock file: {}", environment_name)
    })?;
    let mut package_records = Vec::new();

    for platform in environment.platforms() {
        if !platform_matches(platform.subdir(), platform_spec) {
            continue;
        }

        let Some(packages) = environment.packages(platform) else {
            continue;
        };

        for package in packages {
            match package {
                LockedPackage::Conda(conda_package) => {
                    package_records.push(conda_package.to_owned());
                }
                LockedPackage::Pypi(package_data) => {
                    ignore_or_reject_pypi_package(package_data, ignore_pypi, ignore_packages)?;
                }
            }
        }
    }

    Ok(package_records)
}

fn ignore_or_reject_pypi_package(
    package_data: &PypiPackageData,
    ignore_pypi: bool,
    ignore_packages: &[IgnorePackage],
) -> Result<()> {
    let package_name = package_data.name().to_string();

    if is_pypi_package_ignored(package_data, ignore_packages)? {
        warn!("Ignoring pypi package: {}", package_name);
        return Ok(());
    }

    if !ignore_pypi {
        return Err(anyhow::anyhow!(
            "Pypi packages are not supported: {}",
            package_name
        ));
    }

    warn!("Ignoring pypi package: {}", package_name);

    Ok(())
}

fn is_pypi_package_ignored(
    package_data: &PypiPackageData,
    ignore_packages: &[IgnorePackage],
) -> Result<bool> {
    let package_name = package_data.name().to_string();

    match package_data {
        PypiPackageData::Distribution(distribution_data) => is_package_ignored(
            ignore_packages,
            &package_name,
            &distribution_data.version.to_string(),
        ),
        PypiPackageData::Source(_) => Ok(is_package_ignored_by_name_only(
            ignore_packages,
            &package_name,
        )),
    }
}

fn platform_matches(platform: Platform, platform_spec: &Option<Vec<Platform>>) -> bool {
    platform_spec
        .as_ref()
        .map(|all| all.contains(&platform))
        .unwrap_or(true)
}

fn _get_environment_names(lock_file: &LockFile) -> Vec<String> {
    lock_file
        .environments()
        .map(|env| env.0.to_string())
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_get_packages_for_pixi_lock() {
        let path = Path::new("tests/default_pixi.lock");
        let package_records = get_conda_packages_for_pixi_lock(path, &None, &None, false, &[]);
        assert_eq!(package_records.unwrap().len(), 758);

        let package_records = get_conda_packages_for_pixi_lock(
            path,
            &Some(vec!["lint".to_string()]),
            &None,
            false,
            &[],
        );
        assert_eq!(package_records.unwrap().len(), 219);

        let package_records = get_conda_packages_for_pixi_lock(
            path,
            &Some(vec!["lint".to_string()]),
            &Some(vec![Platform::Linux64]),
            false,
            &[],
        );
        assert_eq!(package_records.unwrap().len(), 48);

        let path = Path::new("tests/pixi-build/pixi.lock");
        let package_records = get_conda_packages_for_pixi_lock(path, &None, &None, false, &[]);
        assert_eq!(package_records.unwrap().len(), 89);

        let package_records = get_conda_packages_for_pixi_lock(
            path,
            &None,
            &Some(vec![Platform::Linux64]),
            false,
            &[],
        );
        assert_eq!(package_records.unwrap().len(), 22);
    }
}
