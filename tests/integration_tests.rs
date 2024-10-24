#[cfg(test)]
mod tests {
    use assert_cmd::prelude::*;
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
}
