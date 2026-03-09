use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::path::PathBuf;

/// S9 — issue tracker.
#[derive(Parser, Debug)]
#[command(name = "s9", version)]
pub struct Cli {
    /// Directory for SQLite database and attachments.
    #[arg(long, env = "S9_DATA_DIR", default_value = "./data")]
    pub data_dir: PathBuf,

    /// Address and port to listen on.
    #[arg(long, env = "S9_LISTEN", default_value = "127.0.0.1:8080")]
    pub listen: SocketAddr,

    /// OIDC issuer URL (e.g. `https://idp.example.com/realm`). Enables OIDC when set.
    #[arg(long, env = "S9_OIDC_ISSUER_URL")]
    pub oidc_issuer_url: Option<String>,

    /// OIDC client ID registered with the identity provider.
    #[arg(long, env = "S9_OIDC_CLIENT_ID")]
    pub oidc_client_id: Option<String>,

    /// OIDC client secret.
    #[arg(long, env = "S9_OIDC_CLIENT_SECRET")]
    pub oidc_client_secret: Option<String>,

    /// Display name for the OIDC login button.
    #[arg(long, env = "S9_OIDC_DISPLAY_NAME", default_value = "SSO")]
    pub oidc_display_name: String,

    /// SMTP server hostname. When unset, email notifications are disabled.
    #[arg(long, env = "S9_SMTP_HOST")]
    pub smtp_host: Option<String>,

    /// SMTP server port.
    #[arg(long, env = "S9_SMTP_PORT", default_value = "587")]
    pub smtp_port: u16,

    /// SMTP authentication username.
    #[arg(long, env = "S9_SMTP_USERNAME")]
    pub smtp_username: Option<String>,

    /// SMTP authentication password.
    #[arg(long, env = "S9_SMTP_PASSWORD")]
    pub smtp_password: Option<String>,

    /// SMTP TLS mode: none, starttls, or tls.
    #[arg(long, env = "S9_SMTP_TLS", default_value = "starttls")]
    pub smtp_tls: String,

    /// Sender email address for notifications. Required when SMTP is configured.
    #[arg(long, env = "S9_SMTP_FROM")]
    pub smtp_from: Option<String>,

    /// Base URL for links in emails (e.g. https://bugs.example.com).
    #[arg(long, env = "S9_BASE_URL")]
    pub base_url: Option<String>,

    /// Seconds to delay before sending notifications (batching window).
    #[arg(long, env = "S9_NOTIFICATION_DELAY", default_value = "120")]
    pub notification_delay: i64,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start the HTTP server (default).
    Serve,

    /// Run pending database migrations.
    Migrate,

    /// Create an admin user.
    CreateAdmin {
        /// Login name for the admin user.
        #[arg(long)]
        login: String,

        /// Password for the admin user.
        #[arg(long)]
        password: String,
    },
}
