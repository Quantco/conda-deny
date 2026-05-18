use std::path::PathBuf;

use clap_verbosity_flag::{ErrorLevel, Verbosity};

use clap::{Parser, ValueHint};
use clap_complete::Shell;
use rattler_conda_types::Platform;

use crate::OutputFormat;

#[derive(Parser, Debug)]
#[command(
    name = "conda-deny",
    about = "Check and list licenses of pixi and conda environments",
    version = env!("CARGO_PKG_VERSION")
)]
pub struct Cli {
    #[command(flatten)]
    pub verbose: Verbosity<ErrorLevel>,

    #[command(subcommand)]
    pub command: CondaDenyCliConfig,

    /// Path to the conda-deny config file
    #[arg(short, long, global = true, value_hint = ValueHint::FilePath)]
    pub config: Option<PathBuf>,
}

#[derive(clap::Subcommand, Debug)]
pub enum CondaDenyCliConfig {
    /// Check licenses of pixi or conda environment against a allowlist
    Check {
        /// Path to the pixi lockfile(s), can be glob patterns
        #[arg(short, long, value_hint = ValueHint::AnyPath)]
        lockfile: Option<Vec<String>>,

        /// Path to the conda prefix(es)
        #[arg(
            long,
            global = true,
            conflicts_with_all = ["platform", "environment", "lockfile"],
            value_hint = ValueHint::DirPath
        )]
        prefix: Option<Vec<PathBuf>>,

        /// Platform(s) to check
        #[arg(short, long)]
        platform: Option<Vec<Platform>>,

        /// Pixi environment(s) to check
        #[arg(short, long)]
        environment: Option<Vec<String>>,

        /// Check against OSI licenses instead of custom license allowlists.
        #[arg(long)]
        osi: Option<bool>,

        /// Ignore when encountering pypi packages instead of failing.
        #[arg(long)]
        ignore_pypi: Option<bool>,

        /// Output format
        #[arg(short, long)]
        output: Option<OutputFormat>,
    },
    /// List all packages and their licenses in your conda or pixi environment
    List {
        /// Path to the pixi lockfile(s)
        #[arg(short, long, value_hint = ValueHint::AnyPath)]
        lockfile: Option<Vec<String>>,

        /// Path to the conda prefix(es)
        #[arg(long, global = true, value_hint = ValueHint::DirPath)]
        prefix: Option<Vec<PathBuf>>,

        /// Platform(s) to list
        #[arg(short, long)]
        platform: Option<Vec<Platform>>,

        /// Pixi environment(s) to list
        #[arg(short, long)]
        environment: Option<Vec<String>>,

        /// Ignore when encountering pypi packages instead of failing.
        #[arg(long)]
        ignore_pypi: Option<bool>,

        /// Output format
        #[arg(short, long)]
        output: Option<OutputFormat>,
    },

    /// Bundle all dependency licenses in a directory
    Bundle {
        /// Path to the pixi lockfile(s)
        #[arg(short, long, value_hint = ValueHint::AnyPath)]
        lockfile: Option<Vec<String>>,

        /// Path to the conda prefix(es)
        #[arg(long, global = true, value_hint = ValueHint::DirPath)]
        prefix: Option<Vec<PathBuf>>,

        /// Platform(s) to bundle
        #[arg(short, long)]
        platform: Option<Vec<Platform>>,

        /// Pixi environment(s) to bundle
        #[arg(short, long)]
        environment: Option<Vec<String>>,

        /// Ignore when encountering pypi packages instead of failing.
        #[arg(long)]
        ignore_pypi: Option<bool>,

        /// Directory to bundle licenses into
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        directory: Option<PathBuf>,
    },

    /// Generate shell completions
    Completion {
        /// Shell to generate completions for
        #[arg(long)]
        shell: Shell,
    },
}

impl CondaDenyCliConfig {
    pub fn lockfile(&self) -> Option<Vec<String>> {
        match self {
            CondaDenyCliConfig::Check { lockfile, .. } => lockfile.clone(),
            CondaDenyCliConfig::List { lockfile, .. } => lockfile.clone(),
            CondaDenyCliConfig::Bundle { lockfile, .. } => lockfile.clone(),
            CondaDenyCliConfig::Completion { .. } => None,
        }
    }

    pub fn prefix(&self) -> Option<Vec<PathBuf>> {
        match self {
            CondaDenyCliConfig::Check { prefix, .. } => prefix.clone(),
            CondaDenyCliConfig::List { prefix, .. } => prefix.clone(),
            CondaDenyCliConfig::Bundle { prefix, .. } => prefix.clone(),
            CondaDenyCliConfig::Completion { .. } => None,
        }
    }

    pub fn platform(&self) -> Option<Vec<Platform>> {
        match self {
            CondaDenyCliConfig::Check { platform, .. } => platform.clone(),
            CondaDenyCliConfig::List { platform, .. } => platform.clone(),
            CondaDenyCliConfig::Bundle { platform, .. } => platform.clone(),
            CondaDenyCliConfig::Completion { .. } => None,
        }
    }

    pub fn environment(&self) -> Option<Vec<String>> {
        match self {
            CondaDenyCliConfig::Check { environment, .. } => environment.clone(),
            CondaDenyCliConfig::List { environment, .. } => environment.clone(),
            CondaDenyCliConfig::Bundle { environment, .. } => environment.clone(),
            CondaDenyCliConfig::Completion { .. } => None,
        }
    }

    pub fn ignore_pypi(&self) -> Option<bool> {
        match self {
            CondaDenyCliConfig::Check { ignore_pypi, .. } => *ignore_pypi,
            CondaDenyCliConfig::List { ignore_pypi, .. } => *ignore_pypi,
            CondaDenyCliConfig::Bundle { ignore_pypi, .. } => *ignore_pypi,
            CondaDenyCliConfig::Completion { .. } => None,
        }
    }

    pub fn output(&self) -> Option<OutputFormat> {
        match self {
            CondaDenyCliConfig::Check { output, .. } => *output,
            CondaDenyCliConfig::List { output, .. } => *output,
            CondaDenyCliConfig::Bundle { .. } => None,
            CondaDenyCliConfig::Completion { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_with_config() {
        let cli =
            Cli::try_parse_from(vec!["conda-deny", "list", "--config", "custom.toml"]).unwrap();
        assert_eq!(cli.config, Some("custom.toml".into()));
    }

    #[test]
    fn test_cli_with_config_new_order() {
        let cli =
            Cli::try_parse_from(vec!["conda-deny", "check", "--config", "custom.toml"]).unwrap();
        assert_eq!(cli.config, Some("custom.toml".into()));
        match cli.command {
            CondaDenyCliConfig::Check { .. } => {}
            _ => panic!("Expected check subcommand with --config"),
        }
    }

    #[test]
    fn test_cli_with_check_arguments() {
        let cli = Cli::try_parse_from(vec!["conda-deny", "check", "--osi", "true"]).unwrap();
        match cli.command {
            CondaDenyCliConfig::Check { osi, .. } => {
                assert_eq!(osi, Some(true));
            }
            _ => panic!("Expected check subcommand with --include-safe"),
        }
    }

    #[test]
    fn test_cli_with_completion_arguments() {
        let cli = Cli::try_parse_from(vec!["conda-deny", "completion", "--shell", "bash"]).unwrap();
        match cli.command {
            CondaDenyCliConfig::Completion { shell } => {
                assert_eq!(shell, Shell::Bash);
            }
            _ => panic!("Expected completion subcommand"),
        }
    }
}
