use std::path::PathBuf;

use clap::ArgAction;
use clap_verbosity_flag::{ErrorLevel, Verbosity};

use clap::Parser;
use rattler_conda_types::Platform;

#[derive(Parser, Debug)]
#[command(name = "conda-deny", about = "Check and list licenses of pixi and conda environments", version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(flatten)]
    pub verbose: Verbosity<ErrorLevel>,

    #[command(subcommand)]
    pub command: CondaDenyCliConfig,

    /// Path to the conda-deny config file
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,
}

#[derive(clap::Subcommand, Debug)]
pub enum CondaDenyCliConfig {
    Check {
        /// Path to the pixi lockfile(s)
        #[arg(short, long)]
        lockfile: Option<Vec<PathBuf>>,

        /// Path to the conda prefix(es)
        #[arg(long, global = true)]
        prefix: Option<Vec<PathBuf>>,

        /// Platform(s) to check
        #[arg(short, long)]
        platform: Option<Vec<Platform>>,

        /// Environment(s) to check
        #[arg(short, long)]
        environment: Option<Vec<String>>,

        #[arg(short, long, action = ArgAction::SetTrue)]
        include_safe: bool,

        #[arg(short, long)]
        osi: Option<bool>,

        /// Ignore when encountering pypi packages instead of failing.
        #[arg(long)]
        ignore_pypi: Option<bool>,
    },
    List {
        /// Path to the pixi lockfile(s)
        #[arg(short, long)]
        lockfile: Option<Vec<PathBuf>>,

        /// Path to the conda prefix(es)
        #[arg(long, global = true)]
        prefix: Option<Vec<PathBuf>>,

        /// Platform(s) to list
        #[arg(short, long)]
        platform: Option<Vec<Platform>>,

        /// Environment(s) to list
        #[arg(short, long)]
        environment: Option<Vec<String>>,

        /// Ignore when encountering pypi packages instead of failing.
        #[arg(long)]
        ignore_pypi: Option<bool>,
    },
}

impl CondaDenyCliConfig {
    pub fn lockfile(&self) -> Option<Vec<PathBuf>> {
        match self {
            CondaDenyCliConfig::Check { lockfile, .. } => lockfile.clone(),
            CondaDenyCliConfig::List { lockfile, .. } => lockfile.clone(),
        }
    }

    pub fn prefix(&self) -> Option<Vec<PathBuf>> {
        match self {
            CondaDenyCliConfig::Check { prefix, .. } => prefix.clone(),
            CondaDenyCliConfig::List { prefix, .. } => prefix.clone(),
        }
    }

    pub fn platform(&self) -> Option<Vec<Platform>> {
        match self {
            CondaDenyCliConfig::Check { platform, .. } => platform.clone(),
            CondaDenyCliConfig::List { platform, .. } => platform.clone(),
        }
    }

    pub fn environment(&self) -> Option<Vec<String>> {
        match self {
            CondaDenyCliConfig::Check { environment, .. } => environment.clone(),
            CondaDenyCliConfig::List { environment, .. } => environment.clone(),
        }
    }

    pub fn ignore_pypi(&self) -> Option<bool> {
        match self {
            CondaDenyCliConfig::Check { ignore_pypi, .. } => *ignore_pypi,
            CondaDenyCliConfig::List { ignore_pypi, .. } => *ignore_pypi,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_check_include_safe() {
        let cli = Cli::try_parse_from(vec!["conda-deny", "check", "--include-safe"]).unwrap();
        match cli.command {
            CondaDenyCliConfig::Check { include_safe, .. } => {
                assert!(include_safe);
            }
            _ => panic!("Expected check subcommand with --include-safe"),
        }
    }

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
            CondaDenyCliConfig::Check { include_safe, .. } => {
                assert!(!include_safe);
            }
            _ => panic!("Expected check subcommand with --config"),
        }
    }

    #[test]
    fn test_cli_with_check_arguments() {
        let cli = Cli::try_parse_from(vec!["conda-deny", "check", "--include-safe"]).unwrap();
        match cli.command {
            CondaDenyCliConfig::Check { include_safe, .. } => {
                assert!(include_safe);
            }
            _ => panic!("Expected check subcommand with --include-safe"),
        }
    }
}
