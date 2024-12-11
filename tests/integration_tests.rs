use assert_cmd::prelude::*;
use conda_deny::cli::CondaDenyCliConfig;
use conda_deny::{
    check, get_config_options, list, CondaDenyCheckConfig, CondaDenyConfig, CondaDenyListConfig,
};
use rstest::{fixture, rstest};
use std::path::{Path, PathBuf};
use std::process::Command;

#[fixture]
fn list_config(
    #[default(PathBuf::from("examples/simple-python/pixi.toml"))] config: PathBuf,
    #[default(None)] lockfile: Option<Vec<String>>,
    #[default(None)] prefix: Option<Vec<String>>,
    #[default(None)] platform: Option<Vec<String>>,
    #[default(None)] environment: Option<Vec<String>>,
) -> CondaDenyListConfig {
    let cli = CondaDenyCliConfig::List {
        lockfile: lockfile,
        prefix: prefix,
        platform: platform,
        environment: environment,
    };

    let config = get_config_options(
        Some(config.to_str().unwrap().into()), // todo: prettier
        cli,
    )
    .unwrap();

    match config {
        CondaDenyConfig::List(list_config) => list_config,
        _ => panic!(),
    }
}

#[fixture]
fn check_config(
    #[default(PathBuf::from("examples/simple-python/pixi.toml"))] config: PathBuf,
    #[default(None)] lockfile: Option<Vec<String>>,
    #[default(None)] prefix: Option<Vec<String>>,
    #[default(None)] platform: Option<Vec<String>>,
    #[default(None)] environment: Option<Vec<String>>,
    #[default(true)] include_safe: bool,
    #[default(None)] osi: Option<bool>,
    #[default(None)] ignore_pypi: Option<bool>,
) -> CondaDenyCheckConfig {
    let cli = CondaDenyCliConfig::Check {
        lockfile,
        prefix,
        platform,
        environment,
        include_safe,
        osi,
        ignore_pypi,
    };

    let config = get_config_options(
        Some(config.to_str().unwrap().into()), // todo: prettier
        cli,
    );
    let config = config.unwrap();

    match config {
        CondaDenyConfig::Check(check_config) => check_config,
        _ => panic!(),
    }
}

#[fixture]
fn change_directory(#[default(PathBuf::from("examples/simple-python"))] path: PathBuf) {
    std::env::set_current_dir(path).unwrap();
}

#[fixture]
fn out() -> Vec<u8> {
    Vec::new()
}

#[rstest]
#[case("check")]
#[case("list")]
fn test_default_use_case(#[case] subcommand: &str) {
    let test_dir = Path::new("tests/test_end_to_end/test_default_use_case");

    let mut command = Command::cargo_bin("conda-deny").unwrap();
    command.arg(subcommand).current_dir(test_dir);
    command.assert().success();
}

#[rstest]
#[case("check")]
#[case("list")]
fn test_default_use_case_pyproject(#[case] subcommand: &str) {
    let test_dir = Path::new("tests/test_end_to_end/test_default_use_case_pyproject");

    let mut command = Command::cargo_bin("conda-deny").unwrap();
    command.arg(subcommand).current_dir(test_dir);
    command.assert().failure();
}

#[rstest]
fn test_remote_whitelist_check(
    #[with(PathBuf::from("tests/test_end_to_end/test_remote_whitelist/pixi.toml"))]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);
    assert!(result.is_err());
}

#[rstest]
fn test_remote_whitelist_list(
    #[with(
        PathBuf::from("tests/test_end_to_end/test_remote_whitelist/pixi.toml"),
        None,
        Some(vec!["tests/test_conda_prefixes/test-env".into()]),
    )]
    list_config: CondaDenyListConfig,
    mut out: Vec<u8>,
) {
    // todo: this test doesn't make sense
    let result = list(&list_config, &mut out);
    assert!(result.is_ok());
}

#[rstest]
fn test_multiple_whitelists_check(
    #[with(
        PathBuf::from("tests/test_end_to_end/test_multiple_whitelists/pixi.toml")
    )] check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);
    assert!(result.is_err());
}

#[rstest]
fn test_multiple_whitelists_list(
    #[with(
        PathBuf::from("tests/test_end_to_end/test_multiple_whitelists/pixi.toml")
    )] list_config: CondaDenyListConfig,
    mut out: Vec<u8>,
) {
    // todo this test doesn't make sense
    let result = list(&list_config, &mut out);
    assert!(result.is_ok());
}

#[rstest]
fn test_config_with_platform_and_env(
    #[with(PathBuf::from("tests/test_end_to_end/test_platform_env_spec/pixi.toml"))]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);
    assert!(result.is_err());
}

#[rstest]
fn test_osi_check(
    #[with(
        PathBuf::from("tests/test_end_to_end/test_osi_check/pixi.toml"),
        None,
        None,
        None,
        None,
        true,
        Some(true)
    )]
    check_config: CondaDenyCheckConfig,
    mut out: Vec<u8>,
) {
    let result = check(check_config, &mut out);

    assert!(result.is_err());
    // todo assert lines
}

#[rstest]
fn test_prefix_list(
    #[with(
    PathBuf::from("tests/test_end_to_end/test_prefix_list/pixi.toml"), None, Some(vec!["../../../tests/test_conda_prefixes/test-env".into()])
)]
    list_config: CondaDenyListConfig,
    mut out: Vec<u8>,
) {
    // When --prefix is specified, only the license information for the conda-meta directory in the specified prefix should be listed
    // License information from pixi.lock should not be listed
    let result = list(&list_config, &mut out);
    assert!(result.is_ok());
    let line_count = String::from_utf8(out).unwrap().split("\n").count();
    let expected_line_count = 50;
    assert_eq!(
        line_count, expected_line_count,
        "Unexpected number of output lines"
    );

    println!("Output has {} lines", line_count);
}

#[test]
fn test_exception_check() {
    let cli = CondaDenyCliConfig::Check {
        lockfile: None,
        prefix: None,
        platform: None,
        environment: None,
        include_safe: true,
        osi: None,
        ignore_pypi: None,
    };

    let config = get_config_options(
        Some("tests/test_end_to_end/test_exception_use_case/pixi.toml".into()), // todo: prettier
        cli,
    );

    assert!(config.is_err());
    // todo error message
}
