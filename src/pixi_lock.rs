use std::path::Path;

use anyhow::{Context, Result};

use log::warn;
use rattler_conda_types::Platform;
use rattler_lock::{CondaPackageData, LockFile, LockedPackageRef};

fn _get_environment_names(lock_file: &LockFile) -> Vec<String> {
    lock_file
        .environments()
        .map(|env| env.0.to_string())
        .collect::<Vec<_>>()
}

pub fn get_conda_packages_for_pixi_lock(
    pixi_lock_path: &Path,
    environment_spec: &Option<Vec<String>>,
    platform_spec: &Option<Vec<Platform>>,
    ignore_pypi: bool,
) -> Result<Vec<CondaPackageData>> {
    let lock_file = LockFile::from_path(pixi_lock_path)
        .with_context(|| format!("Failed to read pixi.lock file: {:?}", pixi_lock_path))?;
    let environment_spec = environment_spec
        .clone()
        .unwrap_or_else(|| _get_environment_names(&lock_file));
    let mut package_records = Vec::new();

    for environment_name in environment_spec {
        match lock_file.environment(&environment_name) {
            Some(environment) => {
                for platform in environment.platforms() {
                    if platform_spec
                        .as_ref()
                        .map(|all| all.contains(&platform))
                        .unwrap_or(true)
                    {
                        let packages = match environment.packages(platform) {
                            Some(pkgs) => pkgs,
                            None => continue,
                        };
                        for package in packages {
                            match package {
                                LockedPackageRef::Conda(conda_package) => {
                                    package_records.push(conda_package.to_owned());
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
            None => {
                return Err(anyhow::anyhow!(
                    "Environment not found in lock file: {}",
                    environment_name
                ));
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
    fn test_get_packages_for_pixi_lock() {
        let path = Path::new("tests/default_pixi.lock");
        let package_records = get_conda_packages_for_pixi_lock(path, &None, &None, false);
        assert_eq!(package_records.unwrap().len(), 758);

        let package_records =
            get_conda_packages_for_pixi_lock(path, &Some(vec!["lint".to_string()]), &None, false);
        assert_eq!(package_records.unwrap().len(), 219);

        let package_records = get_conda_packages_for_pixi_lock(
            path,
            &Some(vec!["lint".to_string()]),
            &Some(vec![Platform::Linux64]),
            false,
        );
        assert_eq!(package_records.unwrap().len(), 48);
    }
}
