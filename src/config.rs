use std::net::SocketAddr;
use std::path::PathBuf;

use crate::cli::Cli;

/// Application configuration derived from CLI arguments and environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub listen: SocketAddr,
    pub db_path: PathBuf,
}

impl Config {
    /// Build configuration from parsed CLI arguments.
    pub fn from_cli(cli: &Cli) -> Self {
        let db_path = cli.data_dir.join("s9.db");
        Self {
            data_dir: cli.data_dir.clone(),
            listen: cli.listen,
            db_path,
        }
    }
}
