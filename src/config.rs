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

/// SMTP configuration. Present only when `S9_SMTP_HOST` is set.
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub tls: String,
    pub from: String,
    pub base_url: String,
}

/// Application configuration derived from CLI arguments and environment variables.
#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub listen: SocketAddr,
    pub db_path: PathBuf,
    pub oidc: Option<OidcConfig>,
    pub smtp: Option<SmtpConfig>,
    pub notification_delay: i64,
}

impl Config {
    /// Build configuration from parsed CLI arguments.
    ///
    /// Panics if SMTP host is set but required companion fields (`S9_SMTP_FROM`,
    /// `S9_BASE_URL`) are missing.
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

        let smtp = cli.smtp_host.as_ref().map(|host| {
            let from = cli
                .smtp_from
                .as_ref()
                .expect("S9_SMTP_FROM is required when S9_SMTP_HOST is set");
            let base_url = cli
                .base_url
                .as_ref()
                .expect("S9_BASE_URL is required when S9_SMTP_HOST is set");

            SmtpConfig {
                host: host.clone(),
                port: cli.smtp_port,
                username: cli.smtp_username.clone(),
                password: cli.smtp_password.clone(),
                tls: cli.smtp_tls.clone(),
                from: from.clone(),
                base_url: base_url.clone(),
            }
        });

        Self {
            data_dir: cli.data_dir.clone(),
            listen: cli.listen,
            db_path,
            oidc,
            smtp,
            notification_delay: cli.notification_delay,
        }
    }
}
