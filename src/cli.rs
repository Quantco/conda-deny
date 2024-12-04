use clap::ArgAction;
use clap_verbosity_flag::{ErrorLevel, Verbosity};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "conda-deny", about = "Check and list licenses of pixi and conda environments", version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(flatten)]
    pub verbose: Verbosity<ErrorLevel>,

    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, global = true)]
    pub config: Option<String>,

    #[arg(long, global = true)]
    pub prefix: Option<Vec<String>>,

    #[arg(short, long)]
    pub lockfile: Option<Vec<String>>,

    #[arg(short, long)]
    pub platform: Option<Vec<String>>,

    #[arg(short, long)]
    pub environment: Option<Vec<String>>,
}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    Check {
        #[arg(short, long, action = ArgAction::SetTrue)]
        include_safe: bool,

        #[arg(short, long, action = ArgAction::SetTrue)]
        osi: bool,

        /// Ignore when encountering pypi packages instead of failing.
        #[arg(long, action = ArgAction::SetTrue)]
        ignore_pypi: bool,
    },
    List {},
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_check_include_safe() {
        let cli = Cli::try_parse_from(vec!["conda-deny", "check", "--include-safe"]).unwrap();
        match cli.command {
            Commands::Check { include_safe, .. } => {
                assert!(include_safe);
            }
            _ => panic!("Expected check subcommand with --include-safe"),
        }
    }

    #[test]
    fn test_cli_with_config() {
        let cli =
            Cli::try_parse_from(vec!["conda-deny", "list", "--config", "custom.toml"]).unwrap();
        assert_eq!(cli.config.as_deref(), Some("custom.toml"));
    }

    #[test]
    fn test_cli_with_config_new_order() {
        let cli =
            Cli::try_parse_from(vec!["conda-deny", "check", "--config", "custom.toml"]).unwrap();
        assert_eq!(cli.config.as_deref(), Some("custom.toml"));
        match cli.command {
            Commands::Check { include_safe, .. } => {
                assert!(!include_safe);
            }
            _ => panic!("Expected check subcommand with --config"),
        }
    }

    #[test]
    fn test_cli_with_check_arguments() {
        let cli = Cli::try_parse_from(vec!["conda-deny", "check", "--include-safe"]).unwrap();
        match cli.command {
            Commands::Check { include_safe, .. } => {
                assert!(include_safe);
            }
            _ => panic!("Expected check subcommand with --include-safe"),
        }
    }

    #[test]
    fn test_cli_with_check_osi() {
        let cli = Cli::try_parse_from(vec!["conda-deny", "check", "--osi"]).unwrap();
        match cli.command {
            Commands::Check { osi, .. } => {
                assert!(osi);
            }
            _ => panic!("Expected check subcommand with --osi"),
        }
    }
}
