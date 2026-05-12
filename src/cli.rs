use std::path::PathBuf;

use clap_verbosity_flag::{ErrorLevel, Verbosity};

use clap::Parser;
use rattler_conda_types::Platform;

use crate::OutputFormat;

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
    /// Check licenses of pixi or conda environment against a allowlist
    Check {
        /// Path to the pixi lockfile(s), can be glob patterns
        #[arg(short, long)]
        lockfile: Option<Vec<String>>,

        /// Path to the conda prefix(es)
        #[arg(long, global = true, conflicts_with_all = ["platform", "environment", "lockfile"])]
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
        #[arg(short, long)]
        lockfile: Option<Vec<String>>,

        /// Path to the conda prefix(es)
        #[arg(long, global = true)]
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
        #[arg(short, long)]
        lockfile: Option<Vec<String>>,

        /// Path to the conda prefix(es)
        #[arg(long, global = true)]
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
        #[arg(short, long)]
        directory: Option<PathBuf>,
    },
}

impl CondaDenyCliConfig {
    pub fn lockfile(&self) -> Option<Vec<String>> {
        match self {
            CondaDenyCliConfig::Check { lockfile, .. } => lockfile.clone(),
            CondaDenyCliConfig::List { lockfile, .. } => lockfile.clone(),
            CondaDenyCliConfig::Bundle { lockfile, .. } => lockfile.clone(),
        }
    }

    pub fn prefix(&self) -> Option<Vec<PathBuf>> {
        match self {
            CondaDenyCliConfig::Check { prefix, .. } => prefix.clone(),
            CondaDenyCliConfig::List { prefix, .. } => prefix.clone(),
            CondaDenyCliConfig::Bundle { prefix, .. } => prefix.clone(),
        }
    }

    pub fn platform(&self) -> Option<Vec<Platform>> {
        match self {
            CondaDenyCliConfig::Check { platform, .. } => platform.clone(),
            CondaDenyCliConfig::List { platform, .. } => platform.clone(),
            CondaDenyCliConfig::Bundle { platform, .. } => platform.clone(),
        }
    }

    pub fn environment(&self) -> Option<Vec<String>> {
        match self {
            CondaDenyCliConfig::Check { environment, .. } => environment.clone(),
            CondaDenyCliConfig::List { environment, .. } => environment.clone(),
            CondaDenyCliConfig::Bundle { environment, .. } => environment.clone(),
        }
    }

    pub fn ignore_pypi(&self) -> Option<bool> {
        match self {
            CondaDenyCliConfig::Check { ignore_pypi, .. } => *ignore_pypi,
            CondaDenyCliConfig::List { ignore_pypi, .. } => *ignore_pypi,
            CondaDenyCliConfig::Bundle { ignore_pypi, .. } => *ignore_pypi,
        }
    }

    pub fn output(&self) -> Option<OutputFormat> {
        match self {
            CondaDenyCliConfig::Check { output, .. } => *output,
            CondaDenyCliConfig::List { output, .. } => *output,
            CondaDenyCliConfig::Bundle { .. } => None,
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
}
