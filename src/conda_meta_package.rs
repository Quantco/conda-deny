use core::panic;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[allow(dead_code)]
pub struct CondaMetaPackage {
    name: String,
    version: String,
    build: String,
    platform: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CondaMetaFile {
    build: String,
    build_number: i32,
    depends: Vec<String>,
    license: String,
    license_family: Option<String>,
    md5: String,
    name: String,
    noarch: Option<String>,
    sha256: String,
    size: i32,
    subdir: String,
    version: String,
}

impl CondaMetaPackage {
    #[allow(dead_code)]
    fn from_lock_string(lock_string: &str) -> Option<Self> {
        // Strip leading "-" and any spaces
        let trimmed = lock_string.trim_start_matches('-').trim();

        // Split by "::" to separate the source/platform part from the rest
        let parts: Vec<&str> = trimmed.split("::").collect();
        if parts.len() != 2 {
            panic!("Invalid package format: {}", lock_string);
        }

        // Extract platform from the first part, split by "/"
        let arch_parts: Vec<&str> = parts[0].split('/').collect();
        if arch_parts.len() != 2 {
            panic!("Invalid package architecture: {}", parts[0]);
        }
        let platform = arch_parts[1].to_string();

        // Split the second part by "=" to separate name, version, and hash
        let details: Vec<&str> = parts[1].split('=').collect();
        if details.len() != 3 {
            panic!("Invalid package details: {}", parts[1]);
        }
        let name = details[0].to_string();
        let version = details[1].to_string();
        let build = details[2].to_string();

        Some(CondaMetaPackage {
            name,
            version,
            build,
            platform,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conda_meta_package_creation() {
        let conda_meta_package = CondaMetaPackage {
            name: "test".to_string(),
            version: "1.0".to_string(),
            build: "pyhd8ed1ab_0".to_string(),
            platform: "noarch".to_string(),
        };

        assert_eq!(conda_meta_package.name, "test");
        assert_eq!(conda_meta_package.version, "1.0");
        assert_eq!(conda_meta_package.build, "pyhd8ed1ab_0");
        assert_eq!(conda_meta_package.platform, "noarch");
    }

    #[test]
    fn test_conda_meta_file_creation() {
        let conda_meta_file = CondaMetaFile {
            build: "pyhd8ed1ab_0".to_string(),
            build_number: 0,
            depends: vec![
                "python >=3.8,<3.9.0a0".to_string(),
                "python_abi 3.8.* *_cp38".to_string(),
            ],
            license: "BSD-3-Clause".to_string(),
            license_family: Some("BSD".to_string()),
            md5: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
            name: "alabaster".to_string(),
            noarch: Some("python".to_string()),
            sha256: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
            size: 0,
            subdir: "noarch".to_string(),
            version: "0.7.16".to_string(),
        };

        assert_eq!(conda_meta_file.build, "pyhd8ed1ab_0");
        assert_eq!(conda_meta_file.build_number, 0);
        assert_eq!(conda_meta_file.depends.len(), 2);
        assert_eq!(conda_meta_file.license, "BSD-3-Clause");
        assert_eq!(conda_meta_file.license_family.unwrap(), "BSD");
        assert_eq!(conda_meta_file.md5, "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(conda_meta_file.name, "alabaster");
        assert_eq!(conda_meta_file.noarch.unwrap(), "python");
        assert_eq!(conda_meta_file.sha256, "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(conda_meta_file.size, 0);
        assert_eq!(conda_meta_file.subdir, "noarch");
        assert_eq!(conda_meta_file.version, "0.7.16");
    }

    #[test]
    fn test_from_conda_lock_string() {
        let test_conda_lock_entries: Vec<(&str, &str, &str, &str, &str)> = vec![
            (
                "- conda-forge/linux-64::_libgcc_mutex=0.1=conda_forge",
                "linux-64",
                "_libgcc_mutex",
                "0.1",
                "conda_forge",
            ),
            (
                "- conda-forge/linux-64::_openmp_mutex=4.5=2_gnu",
                "linux-64",
                "_openmp_mutex",
                "4.5",
                "2_gnu",
            ),
            (
                "- conda-forge/noarch::alabaster=0.7.16=pyhd8ed1ab_0",
                "noarch",
                "alabaster",
                "0.7.16",
                "pyhd8ed1ab_0",
            ),
            (
                "- conda-forge/linux-64::alsa-lib=1.2.11=hd590300_1",
                "linux-64",
                "alsa-lib",
                "1.2.11",
                "hd590300_1",
            ),
            (
                "- conda-forge/noarch::anyio=4.3.0=pyhd8ed1ab_0",
                "noarch",
                "anyio",
                "4.3.0",
                "pyhd8ed1ab_0",
            ),
            (
                "- conda-forge/noarch::argon2-cffi=23.1.0=pyhd8ed1ab_0",
                "noarch",
                "argon2-cffi",
                "23.1.0",
                "pyhd8ed1ab_0",
            ),
            (
                "- conda-forge/noarch::exceptiongroup=1.2.0=pyhd8ed1ab_2",
                "noarch",
                "exceptiongroup",
                "1.2.0",
                "pyhd8ed1ab_2",
            ),
            (
                "- conda-forge/noarch::executing=2.0.1=pyhd8ed1ab_0",
                "noarch",
                "executing",
                "2.0.1",
                "pyhd8ed1ab_0",
            ),
            (
                "- conda-forge/linux-64::expat=2.6.2=h59595ed_0",
                "linux-64",
                "expat",
                "2.6.2",
                "h59595ed_0",
            ),
            (
                "- conda-forge/linux-64::fastavro=1.9.4=py312h98912ed_0",
                "linux-64",
                "fastavro",
                "1.9.4",
                "py312h98912ed_0",
            ),
            (
                "- conda-forge/noarch::filelock=3.13.1=pyhd8ed1ab_0",
                "noarch",
                "filelock",
                "3.13.1",
                "pyhd8ed1ab_0",
            ),
            (
                "- conda-forge/noarch::font-ttf-dejavu-sans-mono=2.37=hab24e00_0",
                "noarch",
                "font-ttf-dejavu-sans-mono",
                "2.37",
                "hab24e00_0",
            ),
            (
                "- conda-forge/noarch::font-ttf-inconsolata=3.000=h77eed37_0",
                "noarch",
                "font-ttf-inconsolata",
                "3.000",
                "h77eed37_0",
            ),
            (
                "- conda-forge/noarch::font-ttf-source-code-pro=2.038=h77eed37_0",
                "noarch",
                "font-ttf-source-code-pro",
                "2.038",
                "h77eed37_0",
            ),
        ];

        for (lock_string, expected_platform, expected_name, expected_version, expected_build) in
            test_conda_lock_entries.iter()
        {
            let package = CondaMetaPackage::from_lock_string(lock_string).unwrap();
            assert_eq!(package.platform, *expected_platform);
            assert_eq!(package.name, *expected_name);
            assert_eq!(package.version, *expected_version);
            assert_eq!(package.build, *expected_build);
        }
    }
}
