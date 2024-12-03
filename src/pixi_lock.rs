use std::path::Path;

use anyhow::{Context, Result};

use rattler_conda_types::{PackageRecord, Platform};
use rattler_lock::LockFile;

fn _get_environment_names(pixi_lock_path: &Path) -> Vec<String> {
    let lock = LockFile::from_path(pixi_lock_path).unwrap();
    let environment_names = lock
        .environments()
        .map(|env| env.0.to_string())
        .collect::<Vec<_>>();
    environment_names
}

pub fn get_package_records_for_pixi_lock(
    pixi_lock_path: Option<&Path>,
    mut environment_spec: Vec<String>,
    platform_spec: Vec<String>,
) -> Result<Vec<PackageRecord>> {
    let pixi_lock_path = pixi_lock_path.unwrap_or(Path::new("pixi.lock"));

    let lock = LockFile::from_path(pixi_lock_path)
        .with_context(|| format!("Failed to read pixi.lock file: {:?}", pixi_lock_path))?;

    if environment_spec.is_empty() {
        environment_spec = _get_environment_names(pixi_lock_path);
    }

    let mut package_records = Vec::new();

    if platform_spec.is_empty() {
        for environment_name in environment_spec {
            if let Some(environment) = lock.environment(&environment_name) {
                for platform in environment.platforms() {
                    if let Some(packages) = environment.packages(platform) {
                        for package in packages {
                            if let Some(conda_package) = package.as_conda() {
                                let package_record = conda_package.record();
                                package_records.push(package_record.to_owned());
                            }
                        }
                    }
                }
            }
        }
    } else {
        for platform_name in &platform_spec {
            if let Ok(platform) = platform_name.parse::<Platform>() {
                for environment_name in environment_spec.clone() {
                    if let Some(environment) = lock.environment(&environment_name) {
                        if let Some(packages) = environment.packages(platform) {
                            for package in packages {
                                if let Some(conda_package) = package.as_conda() {
                                    let package_record = conda_package.record();
                                    package_records.push(package_record.to_owned());
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
        let environment_names =
            _get_environment_names(Path::new("tests/test_pixi_lock_files/valid1_pixi.lock"));
        assert_eq!(environment_names, vec!["default", "demo", "lint"]);
    }

    #[test]
    fn test_get_packages_for_pixi_lock() {
        let path = Path::new("tests/test_pixi_lock_files/valid1_pixi.lock");
        let package_records = get_package_records_for_pixi_lock(Some(path), vec![], vec![]);
        assert_eq!(package_records.unwrap().len(), 758);

        let package_records =
            get_package_records_for_pixi_lock(Some(path), vec!["lint".to_string()], vec![]);
        assert_eq!(package_records.unwrap().len(), 219);

        let package_records = get_package_records_for_pixi_lock(
            Some(path),
            vec!["lint".to_string()],
            vec!["linux-64".to_string()],
        );
        assert_eq!(package_records.unwrap().len(), 48);
    }
}
