use assert_cmd::prelude::*;
use conda_deny::bundle::bundle;
use conda_deny::cli::CondaDenyCliConfig;
use conda_deny::{
    check::check, get_config_options, list::list, CondaDenyCheckConfig, CondaDenyConfig,
    CondaDenyListConfig,
};
use conda_deny::{CondaDenyBundleConfig, OutputFormat};
use rattler_conda_types::Platform;
use rstest::{fixture, rstest};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;
use walkdir::WalkDir;

#[fixture]
fn list_config(
    #[default(None)] config: Option<PathBuf>,
    #[default(None)] lockfile: Option<Vec<PathBuf>>,
    #[default(None)] prefix: Option<Vec<PathBuf>>,
    #[default(None)] platform: Option<Vec<Platform>>,
    #[default(None)] environment: Option<Vec<String>>,
    #[default(None)] ignore_pypi: Option<bool>,
    #[default(Some(OutputFormat::Default))] output: Option<OutputFormat>,
) -> CondaDenyListConfig {
    let cli = CondaDenyCliConfig::List {
        lockfile,
        prefix,
        platform,
        environment,
        ignore_pypi,
        output,
    };

    let config = get_config_options(config, cli).unwrap();

    match config {
        CondaDenyConfig::List(list_config) => list_config,
        _ => panic!(),
    }
}

#[fixture]
fn bundle_config(
    #[default(None)] config: Option<PathBuf>,
    #[default(None)] lockfile: Option<Vec<PathBuf>>,
    #[default(None)] prefix: Option<Vec<PathBuf>>,
    #[default(None)] platform: Option<Vec<Platform>>,
    #[default(None)] environment: Option<Vec<String>>,
    #[default(None)] ignore_pypi: Option<bool>,
    #[default(None)] directory: Option<PathBuf>,
) -> CondaDenyBundleConfig {
    let cli = CondaDenyCliConfig::Bundle {
        lockfile,
        prefix,
        platform,
        environment,
        ignore_pypi,
        directory,
    };

    let config = get_config_options(config, cli).unwrap();

    match config {
        CondaDenyConfig::Bundle(bundle_config) => bundle_config,
        _ => panic!(),
    }
}

#[fixture]
fn check_config(
    #[default(None)] config: Option<PathBuf>,
    #[default(None)] lockfile: Option<Vec<PathBuf>>,
    #[default(None)] prefix: Option<Vec<PathBuf>>,
    #[default(None)] platform: Option<Vec<Platform>>,
    #[default(None)] environment: Option<Vec<String>>,
    #[default(None)] osi: Option<bool>,
    #[default(None)] ignore_pypi: Option<bool>,
    #[default(Some(OutputFormat::Default))] output: Option<OutputFormat>,
) -> CondaDenyCheckConfig {
    let cli = CondaDenyCliConfig::Check {
        lockfile,
        prefix,
        platform,
        environment,
        osi,
        ignore_pypi,
        output,
    };

    let config = get_config_options(config, cli);
    let config = config.unwrap();

    match config {
        CondaDenyConfig::Check(check_config) => check_config,
        _ => panic!(),
    }
}

#[fixture]
fn out() -> Vec<u8> {
    Vec::new()
}

#[rstest]
#[case("check", "test_default_use_case")]
#[case("list", "test_default_use_case")]
#[case("check", "test_default_use_case_pyproject")]
#[case("list", "test_default_use_case_pyproject")]
fn test_default_use_case(#[case] subcommand: &str, #[case] test_name: &str) {
    use core::str;

    let path_string = format!("tests/{}", test_name);
    let test_dir = Path::new(path_string.as_str());

    let output = Command::cargo_bin("conda-deny")
        .unwrap()
        .arg(subcommand)
        .current_dir(test_dir)
        .env("CLICOLOR_FORCE", "1")
        .output()
        .expect("Failed to execute command");

    let stdout = str::from_utf8(&output.stdout).unwrap();
    if subcommand == "check" {
        assert!(stdout.contains("There were \u{1b}[32m247\u{1b}[0m safe licenses and \u{1b}[31m301\u{1b}[0m unsafe licenses."), "{stdout}");
        output.assert().failure();
    } else {
        assert!(stdout.contains("\u{1b}[34mzstandard\u{1b}[0m \u{1b}[36m0.22.0\u{1b}[0m-\u{1b}[3;96mpy312h721a963_1\u{1b}[0m (\u{1b}[95mosx-arm64\u{1b}[0m): \u{1b}[33mBSD-3-Clause"));
        assert!(stdout.contains("\u{1b}[34mzlib\u{1b}[0m \u{1b}[36m1.3.1\u{1b}[0m-\u{1b}[3;96mh4ab18f5_1\u{1b}[0m (\u{1b}[95mlinux-64\u{1b}[0m): \u{1b}[33mZlib"));
        assert!(stdout.contains("\u{1b}[34mxz\u{1b}[0m \u{1b}[36m5.2.6\u{1b}[0m-\u{1b}[3;96mh166bdaf_0\u{1b}[0m (\u{1b}[95mlinux-64\u{1b}[0m): \u{1b}[33mLGPL-2.1 and GPL-2.0"));
        output.assert().success();
    }
}

#[test]
fn test_remote_whitelist_check() {
    // Create a temporary file for the license_whitelist.toml
    let mut temp_config_file = NamedTempFile::new().unwrap();
    let file_content = r#"[tool.conda-deny]
license-whitelist = "https://raw.githubusercontent.com/Quantco/conda-deny/refs/heads/main/tests/default_license_whitelist.toml""#;

    temp_config_file
        .as_file_mut()
        .write_all(file_content.as_bytes())
        .unwrap();

    let temp_config_file_path = temp_config_file.path().to_path_buf();

    let mut out = out();
    let check_config = check_config(
        Some(temp_config_file_path),
        Some(vec!["tests/default_pixi.lock".into()]),
        None,
        None,
        None,
        None,
        None,
        None,
    );

    let result = check(check_config, &mut out);
    let output = String::from_utf8(out).unwrap();

    assert!(output.contains(
        "There were \u{1b}[32m242\u{1b}[0m safe licenses and \u{1b}[31m299\u{1b}[0m unsafe licenses."
    ));
    assert!(result.is_err());
}

#[test]
fn test_multiple_whitelists_check() {
    // Create a temporary file for the license_whitelist.toml
    let mut temp_license_whitelist = NamedTempFile::new().unwrap();
    let file_content = r#"[tool.conda-deny]
                                safe-licenses = ["BSD-3-Clause"]"#;
    temp_license_whitelist
        .as_file_mut()
        .write_all(file_content.as_bytes())
        .unwrap();

    let temp_whitelist_path = temp_license_whitelist.path().to_path_buf();

    // Create a temporary file for pixi.toml
    let mut temp_pixi_toml = NamedTempFile::new().unwrap();
    let file_content = "[tool.conda-deny]
        license-whitelist = [
        \"https://raw.githubusercontent.com/Quantco/conda-deny/refs/heads/main/tests/default_license_whitelist.toml\",
        \"".to_string() + temp_whitelist_path.to_str().unwrap() + "\"]";

    temp_pixi_toml
        .as_file_mut()
        .write_all(file_content.as_bytes())
        .unwrap();

    let mut out = out();
    // Inject the temporary file's path into check_config
    let temp_path = Some(temp_pixi_toml.path().to_path_buf());
    let check_config = check_config(
        temp_path,
        Some(vec!["tests/default_pixi.lock".into()]),
        None,
        None,
        None,
        None,
        None,
        None,
    );

    let result = check(check_config, &mut out);
    let output = String::from_utf8(out).unwrap();

    assert!(output.contains(
        "There were \u{1b}[32m343\u{1b}[0m safe licenses and \u{1b}[31m198\u{1b}[0m unsafe licenses."
    ));
    assert!(result.is_err());
}

#[test]
fn test_platform_env_restrictions_check() {
    // Create a temporary file for pixi.toml
    let mut temp_pixi_toml = NamedTempFile::new().unwrap();
    let file_content = r#"[tool.conda-deny]
license-whitelist = "tests/default_license_whitelist.toml"
lockfile = "tests/default_pixi.lock"
platform = "linux-64"
environment = "lint""#;

    temp_pixi_toml
        .as_file_mut()
        .write_all(file_content.as_bytes())
        .unwrap();

    let mut out = out();
    // Inject the temporary file's path into check_config
    let temp_path = Some(temp_pixi_toml.path().to_path_buf());
    let check_config = check_config(
        temp_path,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    let result = check(check_config, &mut out);
    let output = String::from_utf8(out).unwrap();

    assert!(output.contains(
        "There were \u{1b}[32m27\u{1b}[0m safe licenses and \u{1b}[31m21\u{1b}[0m unsafe licenses."
    ), "{output:?}");
    assert!(result.is_err());
}

#[test]
fn test_safe_licenses_in_config_check() {
    // Create a temporary file for pixi.toml
    let mut temp_pixi_toml = NamedTempFile::new().unwrap();
    let file_content = r#"[tool.conda-deny]
license-whitelist = "tests/default_license_whitelist.toml"
safe-licenses = ["BSD-3-Clause"]"#;

    temp_pixi_toml
        .as_file_mut()
        .write_all(file_content.as_bytes())
        .unwrap();

    let mut out = out();
    // Inject the temporary file's path into check_config
    let temp_path = Some(temp_pixi_toml.path().to_path_buf());
    let check_config = check_config(
        temp_path,
        Some(vec!["tests/default_pixi.lock".into()]),
        None,
        None,
        None,
        None,
        None,
        None,
    );

    let result = check(check_config, &mut out);
    let output = String::from_utf8(out).unwrap();

    assert!(output.contains(
        "There were \u{1b}[32m343\u{1b}[0m safe licenses and \u{1b}[31m198\u{1b}[0m unsafe licenses."
    ));
    assert!(result.is_err());
}

#[rstest]
fn test_osi_check(
    #[with(
        // CONFIG PATH
        None,
        // LOCKFILE PATHS
        Some(vec!["tests/default_pixi.lock".into()]),
        // PREFIXES
        None,
        // PLATFORM
        None,
        // ENVIRONMENT
        None,
        // OSI FLAG
        Some(true)
    )]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);

    let output = String::from_utf8(out).unwrap();

    assert!(output.contains(
        "There were \u{1b}[32m457\u{1b}[0m safe licenses and \u{1b}[31m84\u{1b}[0m unsafe licenses."
    ));
    assert!(result.is_err());
}

#[rstest]
fn test_prefix_list(
    #[with(
        // CONFIG PATH
        None,
        // LOCKFILE PATHS
        None,
        // PREFIXES
        Some(vec!["tests/test_conda_prefixes/test-env".into()])
)]
    list_config: CondaDenyListConfig,
    mut out: Vec<u8>,
) {
    // When --prefix is specified, only the license information for the conda-meta directory in the specified prefix should be listed
    // License information from pixi.lock should not be listed
    let result = list(list_config, &mut out);
    assert!(result.is_ok(), "{:?}", result.unwrap_err());
    let output = String::from_utf8(out).unwrap();
    let line_count = output.split("\n").count();
    let expected_line_count = 51;
    assert_eq!(
        line_count, expected_line_count,
        "Unexpected number of output lines"
    );
}

#[rstest]
fn test_exception_check(
    #[with(Some(PathBuf::from(
        // CONFIG PATH
        "tests/test_exception_use_case/config_with_exception.toml")),
        // LOCKFILE PATHS
        Some(vec!["tests/default_pixi.lock".into()])
    )]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);

    let output = String::from_utf8(out).unwrap();

    assert!(output.contains(
        "There were \u{1b}[32m527\u{1b}[0m safe licenses and \u{1b}[31m14\u{1b}[0m unsafe licenses."
    ), "{output:?}");
    assert!(result.is_err());
}

#[rstest]
fn test_pypi_ignore_check(
    #[with(
        // CONFIG PATH
        Some(PathBuf::from("tests/test_pypi_ignore/pixi.toml")),
        // LOCKFILE PATHS
        Some(vec!["tests/test_pypi_ignore/lockfile_with_pypi_packages.lock".into()]),
        // PREFIXES
        None,
        // PLATFORM
        None,
        // ENVIRONMENT
        None,
        // OSI FLAG
        None,
        // IGNORE PYPI
        Some(true)
    )]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);

    let output = String::from_utf8(out).unwrap();

    assert!(output.contains(
        "There were \u{1b}[32m5\u{1b}[0m safe licenses and \u{1b}[31m17\u{1b}[0m unsafe licenses."
    ));
    assert!(result.is_err());
}

#[rstest]
fn test_pypi_ignore_error(
    #[with(
        // CONFIG PATH
        Some(PathBuf::from("tests/test_pypi_ignore/pixi.toml")),
        // LOCKFILE PATHS
        Some(vec!["tests/test_pypi_ignore/lockfile_with_pypi_packages.lock".into()]),
        // PREFIXES
        None,
        // PLATFORM
        None,
        // ENVIRONMENT
        None,
        // OSI FLAG
        None,
        // IGNORE PYPI
        Some(false)
    )]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);
    if let Err(e) = &result {
        println!("Actual error: {}", e);
    }
    assert!(result.is_err());
    assert!(format!("{:?}", result).contains("Pypi packages are not supported: beautifulsoup4"));
    assert_eq!(out, b"");
}

#[rstest]
fn test_bundle_prefix() {
    let mut out = out();
    let temp_dir = tempfile::tempdir().unwrap();
    let bundle_config = bundle_config(
        // CONFIG PATH
        None,
        // LOCKFILE PATHS
        None,
        // PREFIXES
        Some(vec!["tests/test_conda_prefixes/test-env".into()]),
        // PLATFORM
        None,
        // ENVIRONMENT
        None,
        // IGNORE PYPI
        None,
        // DIRECTORY
        Some(temp_dir.path().join(Path::new("test_bundle"))),
    );

    // Suppress progress bar
    std::env::set_var("NO_PROGRESS", "1");

    bundle(bundle_config.clone(), &mut out).unwrap();
    let bundle_dir = bundle_config.directory.unwrap();

    let mut files = Vec::new();
    let mut dirs = Vec::new();

    for e in WalkDir::new(&bundle_dir).into_iter().filter_map(|e| e.ok()) {
        if e.metadata().unwrap().is_file() {
            files.push(e.path().to_path_buf());
        }
        if e.metadata().unwrap().is_dir() {
            dirs.push(e.path().to_path_buf());
        }
    }
    assert_eq!(files.len(), 48, "{files:?}");
    assert_eq!(dirs.len(), 52, "{dirs:?}");

    let top_level_entries = std::fs::read_dir(bundle_dir).unwrap();
    let top_level_entry_count = top_level_entries.count();
    assert_eq!(top_level_entry_count, 47);
    let output = String::from_utf8(out).unwrap();
    assert!(output.contains("License files written to:"));
}

#[rstest]
fn test_bundle_lockfile() {
    let mut out = out();
    let temp_dir = tempfile::tempdir().unwrap();
    let bundle_config = bundle_config(
        // CONFIG PATH
        None,
        // LOCKFILE PATHS
        Some(vec!["tests/test_default_use_case/pixi.lock".into()]),
        // PREFIXES
        None,
        // PLATFORM
        None,
        // ENVIRONMENT
        None,
        // IGNORE PYPI
        None,
        // DIRECTORY
        Some(temp_dir.path().join(Path::new("test_bundle"))),
    );

    // Suppress progress bar
    std::env::set_var("NO_PROGRESS", "1");

    bundle(bundle_config.clone(), &mut out).unwrap();
    let bundle_dir = bundle_config.directory.unwrap();

    let mut files = Vec::new();
    let mut dirs = Vec::new();

    for e in WalkDir::new(&bundle_dir).into_iter().filter_map(|e| e.ok()) {
        if e.metadata().unwrap().is_file() {
            files.push(e.path().to_path_buf());
        }
        if e.metadata().unwrap().is_dir() {
            dirs.push(e.path().to_path_buf());
        }
    }
    assert_eq!(files.len(), 6649);
    assert_eq!(dirs.len(), 4724);

    let top_level_entries = std::fs::read_dir(bundle_dir).unwrap();
    let top_level_entry_count = top_level_entries.count();
    assert_eq!(top_level_entry_count, 356);
    let output = String::from_utf8(out).unwrap();
    assert!(output.contains("License files written to:"));
}
