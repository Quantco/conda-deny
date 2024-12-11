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
    #[default(None)] config: Option<PathBuf>,
    #[default(None)] lockfile: Option<Vec<String>>,
    #[default(None)] prefix: Option<Vec<String>>,
    #[default(None)] platform: Option<Vec<String>>,
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

    let config = get_config_options(
        config.map(|p| p.to_str().unwrap().to_string()), // todo: prettier
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
    #[default(None)] config: Option<PathBuf>,
    #[default(None)] lockfile: Option<Vec<String>>,
    #[default(None)] prefix: Option<Vec<String>>,
    #[default(None)] platform: Option<Vec<String>>,
    #[default(None)] environment: Option<Vec<String>>,
    #[default(None)] osi: Option<bool>,
    #[default(None)] ignore_pypi: Option<bool>,
) -> CondaDenyCheckConfig {
    let cli = CondaDenyCliConfig::Check {
        lockfile,
        prefix,
        platform,
        environment,
        include_safe: true,
        osi,
        ignore_pypi,
    };

    let config = get_config_options(
        config.map(|p| p.to_str().unwrap().to_string()), // todo: prettier
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
#[case("check", "test_default_use_case")]
#[case("list", "test_default_use_case")]
#[case("check", "test_default_use_case_pyproject")]
#[case("list", "test_default_use_case_pyproject")]
fn test_default_use_case_pyproject(#[case] subcommand: &str, #[case] test_name: &str) {
    let path_string = format!("tests/test_end_to_end/{}", test_name);
    let test_dir = Path::new(path_string.as_str());

    let mut command = Command::cargo_bin("conda-deny").unwrap();
    command.arg(subcommand).current_dir(test_dir);
    if subcommand == "check" {
        command.assert().failure();
    } else {
        command.assert().success();
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
fn test_remote_whitelist_list(
    #[with(
        Some(PathBuf::from("tests/test_end_to_end/test_remote_whitelist/pixi.toml")),
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
fn test_multiple_whitelists_list(
    #[with(Some(PathBuf::from("tests/test_end_to_end/test_multiple_whitelists/pixi.toml")), Some(vec!["tests/test_end_to_end/test_multiple_whitelists/pixi.lock".into()]))]
    list_config: CondaDenyListConfig,
    mut out: Vec<u8>,
) {
    // todo this test doesn't make sense
    let result = list(&list_config, &mut out);
    assert!(result.is_ok());
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
    let result = list(&list_config, &mut out);
    assert!(result.is_ok(), "{:?}", result.unwrap_err());
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
