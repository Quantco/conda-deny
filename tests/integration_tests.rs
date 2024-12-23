use assert_cmd::prelude::*;
use conda_deny::cli::CondaDenyCliConfig;
use conda_deny::{
    check::check, get_config_options, list::list, CondaDenyCheckConfig, CondaDenyConfig,
    CondaDenyListConfig,
};
use rattler_conda_types::Platform;
use rstest::{fixture, rstest};
use std::path::{Path, PathBuf};
use std::process::Command;

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

    let path_string = format!("tests/test_end_to_end/{}", test_name);
    let test_dir = Path::new(path_string.as_str());

    let output = Command::cargo_bin("conda-deny")
        .unwrap()
        .arg(subcommand)
        .current_dir(test_dir)
        .output()
        .expect("Failed to execute command");

    if subcommand == "check" {
        assert!(str::from_utf8(&output.stdout)
            .expect("Failed to convert output to string")
            .contains("There were 242 safe licenses and 300 unsafe licenses."));
        output.assert().failure();
    } else {
        assert!(str::from_utf8(&output.stdout)
            .expect("Failed to convert output to string")
            .contains("zstandard 0.22.0-py312h721a963_1 (osx-arm64): BSD-3-Clause "));

        assert!(str::from_utf8(&output.stdout)
            .expect("Failed to convert output to string")
            .contains("zlib 1.3.1-hfb2fe0b_1 (osx-arm64): Zlib"));

        assert!(str::from_utf8(&output.stdout)
            .expect("Failed to convert output to string")
            .contains("xz 5.2.6-h166bdaf_0 (linux-64): LGPL-2.1 and GPL-2.0"));
        output.assert().success();
    }
}

#[rstest]
fn test_remote_whitelist_check(
    #[with(Some(PathBuf::from("tests/test_end_to_end/test_remote_whitelist/pixi.toml")), Some(vec!["tests/test_end_to_end/test_remote_whitelist/pixi.lock".into()]))]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);
    assert!(result.is_err());
}

#[rstest]
fn test_multiple_whitelists_check(
    #[with(
        Some(PathBuf::from("tests/test_end_to_end/test_multiple_whitelists/pixi.toml")),
        Some(vec!["tests/test_end_to_end/test_multiple_whitelists/pixi.lock".into()])
    )]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);
    assert!(result.is_err());
}

#[rstest]
fn test_config_with_platform_and_env(
    #[with(
        Some(PathBuf::from("tests/test_end_to_end/test_platform_env_spec/pixi.toml")),
        Some(vec!["tests/test_end_to_end/test_platform_env_spec/pixi.lock".into()])
    )]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);
    assert!(result.is_err());
}

#[rstest]
fn test_osi_check(
    #[with(
        None,
        Some(vec!["tests/test_end_to_end/test_osi_check/pixi.toml".into()]),
        None,
        None,
        None,
        Some(true)
    )]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);

    assert!(result.is_err());
}

#[rstest]
fn test_prefix_list(
    #[with(
        Some(PathBuf::from("tests/test_end_to_end/test_prefix_list/pixi.toml")), None, Some(vec!["tests/test_conda_prefixes/test-env".into()])
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
        "tests/test_end_to_end/test_exception_use_case/config_without_exception.toml"
    )))]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);
    // assert!(result.is_err());
}
