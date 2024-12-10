#[cfg(test)]
mod tests {
    use assert_cmd::prelude::*;
    use conda_deny::cli::CondaDenyCliConfig;
    use conda_deny::{get_config_options, list, CondaDenyConfig};
    use core::str;
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn test_default_use_case_check() {
        let test_dir = Path::new("tests/test_end_to_end/test_default_use_case");

        let mut command = Command::cargo_bin("conda-deny").unwrap();
        command.arg("check").current_dir(test_dir);
        command.assert().failure();
    }

    #[test]
    fn test_default_use_case_list() {
        let test_dir = Path::new("tests/test_end_to_end/test_default_use_case");

        let mut command = Command::cargo_bin("conda-deny").unwrap();
        command.arg("list").current_dir(test_dir);
        command.assert().success();
    }

    #[test]
    fn test_default_use_case_pyproject_check() {
        let test_dir = Path::new("tests/test_end_to_end/test_default_use_case_pyproject");

        let mut command = Command::cargo_bin("conda-deny").unwrap();
        command.arg("check").current_dir(test_dir);
        command.assert().failure();
    }

    #[test]
    fn test_default_use_case_pyproject_list() {
        let test_dir = Path::new("tests/test_end_to_end/test_default_use_case_pyproject");

        let mut command = Command::cargo_bin("conda-deny").unwrap();
        command.arg("list").current_dir(test_dir);
        command.assert().success();
    }

    #[test]
    fn test_remote_whitelist_check() {
        let test_dir = Path::new("tests/test_end_to_end/test_remote_whitelist");

        let mut command = Command::cargo_bin("conda-deny").unwrap();
        command.arg("check").current_dir(test_dir);
        command.assert().failure();
    }

    #[test]
    fn test_remote_whitelist_list() {
        let test_dir = Path::new("tests/test_end_to_end/test_remote_whitelist");

        let mut command = Command::cargo_bin("conda-deny").unwrap();
        command.arg("list").current_dir(test_dir);
        command.assert().success();
    }

    #[test]
    fn test_multiple_whitelists_check() {
        let test_dir = Path::new("tests/test_end_to_end/test_multiple_whitelists");

        let mut command = Command::cargo_bin("conda-deny").unwrap();
        command.arg("check").current_dir(test_dir);
        command.assert().failure();
    }

    #[test]
    fn test_multiple_whitelists_list() {
        let test_dir = Path::new("tests/test_end_to_end/test_multiple_whitelists");

        let mut command = Command::cargo_bin("conda-deny").unwrap();
        command.arg("list").current_dir(test_dir);
        command.assert().success();
    }

    #[test]
    fn test_config_with_platform_and_env() {
        let test_dir = Path::new("tests/test_end_to_end/test_platform_env_spec");

        let mut command = Command::cargo_bin("conda-deny").unwrap();
        command.arg("check").current_dir(test_dir);
        command.assert().failure();
    }

    #[test]
    fn test_osi_check() {
        let test_dir = Path::new("tests/test_end_to_end/test_osi_check");

        let mut command = Command::cargo_bin("conda-deny").unwrap();
        command.arg("check --osi").current_dir(test_dir);
        command.assert().failure();
    }

    #[test]
    fn test_prefix_list() {
        // When --prefix is specified, only the license information for the conda-meta directory in the specified prefix should be listed
        // License information from pixi.lock should not be listed
        let cli = CondaDenyCliConfig::List {
            lockfile: None,
            prefix: Some(vec!["../../../tests/test_conda_prefixes/test-env".into()]),
            platform: None,
            environment: None,
        };

        let config = get_config_options(
            Some("tests/test_end_to_end/test_prefix_list/pixi.toml".into()),
            cli,
        )
        .unwrap();

        let mut out = Vec::new();
        let result = match config {
            CondaDenyConfig::List(list_config) => list(&list_config, &mut out),
            _ => panic!(),
        };
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
        let test_dir = Path::new("tests/test_end_to_end/test_exception_use_case");

        let mut command = Command::cargo_bin("conda-deny").unwrap();
        command.arg("check").current_dir(test_dir);
        command.assert().failure();
    }
}
