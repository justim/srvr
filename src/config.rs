use std::fs::metadata;
use std::io;
use std::path::PathBuf;
use std::process::exit;

use clap::Args;
use clap::Command;
use clap::CommandFactory;
use clap::Parser;
use clap_complete::generate;
use clap_complete::Generator;
use clap_complete::Shell;

#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
struct CliConfig {
    /// The actual config for srvr
    #[command(flatten)]
    config: Config,

    /// Generate shell completions
    #[arg(long, value_enum)]
    generate_shell_completions: Option<Shell>,
}

/// Serve files in a directory on a HTTP endpoint
#[derive(Args, Clone, Debug)]
pub struct Config {
    /// The directory to serve to the world
    #[arg(default_value = ".")]
    pub base_dir: PathBuf,

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

        let config = cli_config.config;

        // check for the existence of base dir
        metadata(&config.base_dir)?;

        Ok(config)
    }
}
