use std::path::Path;

use anyhow::{Context, Result};

use log::warn;
use rattler_conda_types::{PackageRecord, Platform};
use rattler_lock::{LockFile, LockedPackageRef};

fn _get_environment_names(lock_file: &LockFile) -> Vec<String> {
    lock_file
        .environments()
        .map(|env| env.0.to_string())
        .collect::<Vec<_>>()
}

pub fn get_conda_packages_for_pixi_lock(
    pixi_lock_path: &Path,
    environment_spec: &Option<Vec<String>>,
    platform_spec: &Option<Vec<String>>,
    ignore_pypi: bool,
) -> Result<Vec<PackageRecord>> {
    let lock_file = LockFile::from_path(pixi_lock_path)
        .with_context(|| format!("Failed to read pixi.lock file: {:?}", pixi_lock_path))?;
    let environment_spec = environment_spec
        .clone()
        .unwrap_or_else(|| _get_environment_names(&lock_file));
    let mut package_records = Vec::new();

    if platform_spec.is_none() {
        for environment_name in environment_spec {
            if let Some(environment) = lock_file.environment(&environment_name) {
                for platform in environment.platforms() {
                    if let Some(packages) = environment.packages(platform) {
                        for package in packages {
                            match package {
                                LockedPackageRef::Conda(conda_package) => {
                                    let package_record = conda_package.record();
                                    package_records.push(package_record.to_owned());
                                }
                                LockedPackageRef::Pypi(package_data, _) => {
                                    if !ignore_pypi {
                                        return Err(anyhow::anyhow!(
                                            "Pypi packages are not supported: {}",
                                            package_data.name
                                        ));
                                    } else {
                                        warn!("Ignoring pypi package: {}", package_data.name);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        for platform_name in &platform_spec.clone().unwrap() {
            if let Ok(platform) = platform_name.parse::<Platform>() {
                for environment_name in environment_spec.clone() {
                    if let Some(environment) = lock_file.environment(&environment_name) {
                        if let Some(packages) = environment.packages(platform) {
                            for package in packages {
                                match package {
                                    LockedPackageRef::Conda(conda_package) => {
                                        let package_record = conda_package.record();
                                        package_records.push(package_record.to_owned());
                                    }
                                    LockedPackageRef::Pypi(package_data, _) => {
                                        if !ignore_pypi {
                                            return Err(anyhow::anyhow!(
                                                "Pypi packages are not supported: {}",
                                                package_data.name
                                            ));
                                        } else {
                                            warn!("Ignoring pypi package: {}", package_data.name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(package_records)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_pixi_lock_read_out() {
        let lock_file =
            LockFile::from_path(Path::new("tests/test_pixi_lock_files/valid1_pixi.lock")).unwrap();
        let environment_names = _get_environment_names(&lock_file);
        assert_eq!(environment_names, vec!["default", "demo", "lint"]);
    }

    #[test]
    fn test_get_packages_for_pixi_lock() {
        let path = Path::new("tests/test_pixi_lock_files/valid1_pixi.lock");
        let package_records = get_conda_packages_for_pixi_lock(&path, &None, &None, false);
        assert_eq!(package_records.unwrap().len(), 758);

        let package_records =
            get_conda_packages_for_pixi_lock(path, &Some(vec!["lint".to_string()]), &None, false);
        assert_eq!(package_records.unwrap().len(), 219);

        let package_records = get_conda_packages_for_pixi_lock(
            path,
            &Some(vec!["lint".to_string()]),
            &Some(vec!["linux-64".to_string()]),
            false,
        );
        assert_eq!(package_records.unwrap().len(), 48);
    }
}
