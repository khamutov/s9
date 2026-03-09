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
