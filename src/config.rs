use std::net::SocketAddr;
use std::path::PathBuf;

use crate::cli::Cli;

/// OIDC provider configuration. Present only when all required env vars are set.
#[derive(Debug, Clone)]
pub struct OidcConfig {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub display_name: String,
}

/// Application configuration derived from CLI arguments and environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub listen: SocketAddr,
    pub db_path: PathBuf,
    pub oidc: Option<OidcConfig>,
}

impl Config {
    /// Build configuration from parsed CLI arguments.
    pub fn from_cli(cli: &Cli) -> Self {
        let db_path = cli.data_dir.join("s9.db");

        let oidc = match (
            &cli.oidc_issuer_url,
            &cli.oidc_client_id,
            &cli.oidc_client_secret,
        ) {
            (Some(issuer_url), Some(client_id), Some(client_secret)) => Some(OidcConfig {
                issuer_url: issuer_url.clone(),
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
                display_name: cli.oidc_display_name.clone(),
            }),
            _ => None,
        };

        Self {
            data_dir: cli.data_dir.clone(),
            listen: cli.listen,
            db_path,
            oidc,
        }
    }
}
