use assert_cmd::prelude::*;
use conda_deny::cli::CondaDenyCliConfig;
use conda_deny::{
    check::check, get_config_options, list::list, CondaDenyCheckConfig, CondaDenyConfig,
    CondaDenyListConfig,
};
use rattler_conda_types::Platform;
use rstest::{fixture, rstest};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;

#[fixture]
fn list_config(
    #[default(None)] config: Option<PathBuf>,
    #[default(None)] lockfile: Option<Vec<PathBuf>>,
    #[default(None)] prefix: Option<Vec<PathBuf>>,
    #[default(None)] platform: Option<Vec<Platform>>,
    #[default(None)] environment: Option<Vec<String>>,
    #[default(None)] ignore_pypi: Option<bool>,
) -> CondaDenyListConfig {
    let cli = CondaDenyCliConfig::List {
        lockfile,
        prefix,
        platform,
        environment,
        ignore_pypi,
    };

    let config = get_config_options(config, cli).unwrap();

    match config {
        CondaDenyConfig::List(list_config) => list_config,
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
    #[default(None)] exclude_environment: Option<Vec<String>>,
) -> CondaDenyCheckConfig {
    let cli = CondaDenyCliConfig::Check {
        lockfile,
        prefix,
        platform,
        environment,
        osi,
        ignore_pypi,
        exclude_environment,
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
        assert!(stdout.contains("There were \u{1b}[32m242\u{1b}[0m safe licenses and \u{1b}[31m300\u{1b}[0m unsafe licenses."));
        output.assert().failure();
    } else {
        assert!(stdout.contains("\u{1b}[34mzstandard\u{1b}[0m \u{1b}[36m0.22.0\u{1b}[0m-\u{1b}[3;96mpy312h721a963_1\u{1b}[0m (\u{1b}[95mosx-arm64\u{1b}[0m): \u{1b}[33mBSD-3-Clause"));
        assert!(stdout.contains("\u{1b}[34mzlib\u{1b}[0m \u{1b}[36m1.3.1\u{1b}[0m-\u{1b}[3;96mh4ab18f5_1\u{1b}[0m (\u{1b}[95mlinux-64\u{1b}[0m): \u{1b}[33mZlib"));
        assert!(stdout.contains("\u{1b}[34mxz\u{1b}[0m \u{1b}[36m5.2.6\u{1b}[0m-\u{1b}[3;96mh166bdaf_0\u{1b}[0m (\u{1b}[95mlinux-64\u{1b}[0m): \u{1b}[33mLGPL-2.1 and GPL-2.0"));
        output.assert().success();
    }
}

#[rstest]
fn test_remote_whitelist_check(
    #[with(
        // CONFIG PATH
        Some(PathBuf::from("tests/test_remote_whitelist/pixi.toml")), 
        // LOCKFILE PATHS
        Some(vec!["tests/default_pixi.lock".into()])
    )]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);
    let output = String::from_utf8(out).unwrap();

    assert!(output.contains(
        "There were \u{1b}[32m242\u{1b}[0m safe licenses and \u{1b}[31m300\u{1b}[0m unsafe licenses."
    ));
    assert!(result.is_err());
}

#[test]
fn test_multiple_whitelists_check() {
    // Create a temporary file for pixi.toml
    let mut temp_file = NamedTempFile::new().unwrap();
    let file_content = r#"[tool.conda-deny]
                                license-whitelist = [
                                "tests/default_license_whitelist.toml",
                                "https://raw.githubusercontent.com/Quantco/conda-deny/refs/heads/main/tests/default_license_whitelist.rs"]"#;
    temp_file
        .as_file_mut()
        .write_all(file_content.as_bytes())
        .unwrap();
    let mut out = out();
    // Inject the temporary file's path into check_config
    let temp_path = Some(temp_file.path().to_path_buf());
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
        "There were \u{1b}[32m344\u{1b}[0m safe licenses and \u{1b}[31m198\u{1b}[0m unsafe licenses."
    ));
    assert!(result.is_err());
}

#[rstest]
fn test_config_with_platform_and_env(
    #[with(
        // CONFIG PATH
        Some(PathBuf::from("tests/test_platform_env_spec/pixi.toml")),
        // LOCKFILE PATHS
        Some(vec!["tests/default_pixi.lock".into()])
    )]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);
    let output = String::from_utf8(out).unwrap();

    assert!(output.contains(
        "There were \u{1b}[32m27\u{1b}[0m safe licenses and \u{1b}[31m21\u{1b}[0m unsafe licenses."
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
        "There were \u{1b}[32m458\u{1b}[0m safe licenses and \u{1b}[31m84\u{1b}[0m unsafe licenses."
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
    let line_count = String::from_utf8(out).unwrap().split("\n").count();
    let expected_line_count = 50;
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
        "There were \u{1b}[32m528\u{1b}[0m safe licenses and \u{1b}[31m14\u{1b}[0m unsafe licenses."
    ));
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
