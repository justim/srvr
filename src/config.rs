use std::fs::metadata;
use std::io;
use std::path::PathBuf;
use std::process::exit;

use clap::Args;
use clap::Command;
use clap::CommandFactory;
use clap::Parser;
use clap::ValueHint;
use clap_complete::generate;
use clap_complete::Generator;
use clap_complete::Shell;
use clap_verbosity_flag::InfoLevel;
use clap_verbosity_flag::Verbosity;

use crate::utils::setup_tracing;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Could not open base dir \"{0}\": {1}")]
    InvalidBaseDir(PathBuf, std::io::Error),

    #[error("Could not open fallback path \"{0}\": {1}")]
    InvalidFallbackPath(PathBuf, std::io::Error),
}

#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
struct CliConfig {
    /// The verbosity of the output
    ///
    /// With a minimum of `info` level
    #[command(flatten)]
    pub verbosity: Verbosity<InfoLevel>,

    /// The actual config for srvr
    #[command(flatten)]
    config: Config,

    /// Generate shell completions
    #[arg(long, value_enum, hide = true)]
    generate_shell_completions: Option<Shell>,
}

/// Serve files in a directory on a HTTP endpoint
#[derive(Args, Clone, Debug)]
pub struct Config {
    /// The directory to serve to the world
    #[arg(default_value = ".", value_hint = ValueHint::DirPath)]
    pub base_dir: PathBuf,

    /// The file to use as the fallback file, defaults to `<base_dir>/index.html`
    #[arg(long, short)]
    pub fallback_path: Option<PathBuf>,

    /// The address to run srvr on, defaults to 127.0.0.1:12234
    #[arg(long, short)]
    pub address: Option<String>,

    /// The port to run srvr on, defaults to 12234 (overrides `address`)
    #[arg(long, short)]
    pub port: Option<u16>,
}

/// Print the completions for srvr and `exit(0)`
fn print_completions<G: Generator>(gen: G, cmd: &mut Command) -> ! {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
    exit(0);
}

impl Config {
    /// Create a config from the environment
    pub fn from_env() -> anyhow::Result<Self> {
        let cli_config = CliConfig::parse();

        if let Some(generate_shell_completions) = cli_config.generate_shell_completions {
            let mut cli_command = CliConfig::command();
            print_completions(generate_shell_completions, &mut cli_command);
        }

        setup_tracing(cli_config.verbosity);

        let config = cli_config.config;

        // check for the existence of base dir
        metadata(&config.base_dir)
            .map_err(|err| ConfigError::InvalidBaseDir(config.base_dir.clone(), err))?;

        if let Some(fallback_path) = &config.fallback_path {
            // check for the existence of fallback path
            metadata(fallback_path)
                .map_err(|err| ConfigError::InvalidFallbackPath(fallback_path.clone(), err))?;
        }

        Ok(config)
    }
}
