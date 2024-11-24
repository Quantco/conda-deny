#[cfg(test)]
mod tests {
    use assert_cmd::prelude::*;
    use core::str;
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn test_default_use_case_check() {
        let test_dir = Path::new("tests/test_end_to_end/test_default_use_case");

        let output = Command::cargo_bin("conda-deny")
            .unwrap()
            .arg("check")
            .current_dir(test_dir)
            .output()
            .expect("Failed to execute command");

        let stdout = str::from_utf8(&output.stdout).expect("Failed to convert output to string");

        let exit_status = output.status.code().unwrap();
        assert_eq!(exit_status, 1, "Unexpected exit status");

        let line_count = stdout.lines().count();
        let expected_line_count = 297;
        assert_eq!(
            line_count, expected_line_count,
            "Unexpected number of output lines"
        );
    }

    #[test]
    fn test_default_use_case_list() {
        let test_dir = Path::new("tests/test_end_to_end/test_default_use_case");

        let output = Command::cargo_bin("conda-deny")
            .unwrap()
            .arg("list")
            .current_dir(test_dir)
            .output()
            .expect("Failed to execute command");

        let stdout = str::from_utf8(&output.stdout).expect("Failed to convert output to string");

        let exit_status = output.status.code().unwrap();
        assert_eq!(exit_status, 0, "Unexpected exit status");

        let line_count = stdout.lines().count();
        let expected_line_count = 543;
        assert_eq!(
            line_count, expected_line_count,
            "Unexpected number of output lines"
        );
    }

    #[test]
    fn test_default_use_case_pyproject_check() {
        let test_dir = Path::new("tests/test_end_to_end/test_default_use_case_pyproject");

        let output = Command::cargo_bin("conda-deny")
            .unwrap()
            .arg("check")
            .current_dir(test_dir)
            .output()
            .expect("Failed to execute command");

        let exit_status = output.status.code().unwrap();
        assert_eq!(exit_status, 1, "Unexpected exit status");

        let stdout = str::from_utf8(&output.stdout).expect("Failed to convert output to string");

        let line_count = stdout.lines().count();
        let expected_line_count = 289;
        assert_eq!(
            line_count, expected_line_count,
            "Unexpected number of output lines"
        );
    }

    #[test]
    fn test_default_use_case_pyproject_list() {
        let test_dir = Path::new("tests/test_end_to_end/test_default_use_case_pyproject");

        let output = Command::cargo_bin("conda-deny")
            .unwrap()
            .arg("list")
            .current_dir(test_dir)
            .output()
            .expect("Failed to execute command");

        let stdout = str::from_utf8(&output.stdout).expect("Failed to convert output to string");

        let exit_status = output.status.code().unwrap();
        assert_eq!(exit_status, 0, "Unexpected exit status");

        let line_count = stdout.lines().count();
        let expected_line_count = 543;
        assert_eq!(
            line_count, expected_line_count,
            "Unexpected number of output lines"
        );
    }

    #[test]
    fn test_remote_whitelist_check() {
        let test_dir = Path::new("tests/test_end_to_end/test_remote_whitelist");

        let output = Command::cargo_bin("conda-deny")
            .unwrap()
            .arg("check")
            .current_dir(test_dir)
            .output()
            .expect("Failed to execute command");

        let exit_status = output.status.code().unwrap();
        assert_eq!(exit_status, 1, "Unexpected exit status");

        let stdout = str::from_utf8(&output.stdout).expect("Failed to convert output to string");

        let line_count = stdout.lines().count();
        let expected_line_count = 307;
        assert_eq!(
            line_count, expected_line_count,
            "Unexpected number of output lines"
        );

        assert!(stdout.contains("There were 242 safe licenses and 300 unsafe licenses."));
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

        let output = Command::cargo_bin("conda-deny")
            .unwrap()
            .arg("check")
            .current_dir(test_dir)
            .output()
            .expect("Failed to execute command");

        let exit_status = output.status.code().unwrap();
        assert_eq!(exit_status, 1, "Unexpected exit status");

        let stdout = str::from_utf8(&output.stdout).expect("Failed to convert output to string");

        let line_count = stdout.lines().count();
        let expected_line_count = 205;
        assert_eq!(
            line_count, expected_line_count,
            "Unexpected number of output lines"
        );
    }

    #[test]
    fn test_multiple_whitelists_list() {
        let test_dir = Path::new("tests/test_end_to_end/test_multiple_whitelists");

        let output = Command::cargo_bin("conda-deny")
            .unwrap()
            .arg("list")
            .current_dir(test_dir)
            .output()
            .expect("Failed to execute command");

        let stdout = str::from_utf8(&output.stdout).expect("Failed to convert output to string");

        let exit_status = output.status.code().unwrap();
        assert_eq!(exit_status, 0, "Unexpected exit status");

        let line_count = stdout.lines().count();

        let expected_line_count = 543;
        assert_eq!(
            line_count, expected_line_count,
            "Unexpected number of output lines"
        );
    }

    #[test]
    fn test_config_with_platform_and_env() {
        let test_dir = Path::new("tests/test_end_to_end/test_platform_env_spec");

        let output = Command::cargo_bin("conda-deny")
            .unwrap()
            .arg("check")
            .current_dir(test_dir)
            .output()
            .expect("Failed to execute command");

        let stdout = str::from_utf8(&output.stdout).expect("Failed to convert output to string");

        let exit_status = output.status.code().unwrap();
        assert_eq!(exit_status, 1, "Unexpected exit status");

        let line_count = stdout.lines().count();
        let expected_line_count = 28;
        assert_eq!(
            line_count, expected_line_count,
            "Unexpected number of output lines."
        );

        assert!(stdout.contains("There were 27 safe licenses and 21 unsafe licenses."));
    }

    #[test]
    fn test_osi_check() {
        let test_dir = Path::new("tests/test_end_to_end/test_osi_check");

        let output = Command::cargo_bin("conda-deny")
            .unwrap()
            .arg("check")
            .arg("--osi")
            .current_dir(test_dir)
            .output()
            .expect("Failed to execute command");

        let stdout = str::from_utf8(&output.stdout).expect("Failed to convert output to string");

        let exit_status = output.status.code().unwrap();
        assert_eq!(exit_status, 1, "Unexpected exit status");

        let line_count = stdout.lines().count();
        let expected_line_count = 91;
        assert_eq!(
            line_count, expected_line_count,
            "Unexpected number of output lines."
        );

        assert!(stdout.contains("There were 458 safe licenses and 84 unsafe licenses."));
    }

    #[test]
    fn test_prefix_list() {
        // When --prefix is specified, only the license information for the conda-meta directory in the specified prefix should be listed
        // License information from pixi.lock should not be listed

        let test_dir = Path::new("tests/test_end_to_end/test_prefix_list");

        let output = Command::cargo_bin("conda-deny")
            .unwrap()
            .arg("list")
            .arg("--prefix")
            .arg("../../../tests/test_conda_prefixes/test-env")
            .current_dir(test_dir)
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Command did not execute successfully"
        );

        let stdout = str::from_utf8(&output.stdout).expect("Failed to convert output to string");

        let line_count = stdout.lines().count();

        let expected_line_count = 50;
        assert_eq!(
            line_count, expected_line_count,
            "Unexpected number of output lines"
        );
    }

    #[test]
    fn test_exception_check() {
        let test_dir = Path::new("tests/test_end_to_end/test_exception_use_case");

        let output = Command::cargo_bin("conda-deny")
            .unwrap()
            .arg("check")
            .current_dir(test_dir)
            .output()
            .expect("Failed to execute command");

        let stdout = str::from_utf8(&output.stdout).expect("Failed to convert output to string");

        let line_count = stdout.lines().count();

        let expected_line_count = 10;

        assert_eq!(
            line_count, expected_line_count,
            "Unexpected number of output lines"
        );

        assert!(stdout.contains("poppler 24.8.0-h37b219d_1 (osx-arm64): GPL-2.0-only"));
    }
}
