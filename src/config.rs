use std::fs::metadata;
use std::path::PathBuf;

use clap::Parser;

/// Serve files in a directory on a HTTP endpoint
#[derive(Parser, Clone, Debug)]
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

impl Config {
    /// Create a config from the environment
    pub fn from_env() -> anyhow::Result<Self> {
        let config = Config::parse();

        // check for the existence of base dir
        metadata(&config.base_dir)?;

        Ok(config)
    }
}
