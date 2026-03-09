pub(crate) mod api;
mod auth;
mod cli;
mod config;
mod db;
mod embed;
mod models;
mod repos;
mod search;
mod slug;
mod storage;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::cli::{Cli, Command};
use crate::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cli = Cli::parse();
    let config = Config::from_cli(&cli);

    // Ensure the data directory exists.
    std::fs::create_dir_all(&config.data_dir)?;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(config).await,
        Command::Migrate => migrate(config).await,
        Command::CreateAdmin { login, password } => create_admin(config, login, password).await,
    }
}

async fn serve(config: Config) -> anyhow::Result<()> {
    let pool = db::init_pool(&config.db_path).await?;
    db::run_migrations(&pool).await?;

    let oidc = match &config.oidc {
        Some(oidc_config) => {
            tracing::info!("OIDC enabled, discovering provider metadata…");
            let provider = api::init_oidc(oidc_config).await?;
            tracing::info!("OIDC provider '{}' ready", provider.display_name);
            Some(std::sync::Arc::new(provider))
        }
        None => {
            tracing::info!("OIDC not configured (set S9_OIDC_ISSUER_URL to enable)");
            None
        }
    };

    let app = api::build_router(pool, oidc);

    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    tracing::info!("listening on {}", config.listen);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn migrate(config: Config) -> anyhow::Result<()> {
    let pool = db::init_pool(&config.db_path).await?;
    db::run_migrations(&pool).await?;
    tracing::info!("migrations complete");
    Ok(())
}

async fn create_admin(_config: Config, _login: String, _password: String) -> anyhow::Result<()> {
    // Implementation deferred to Phase 2 (auth + user tables required).
    tracing::warn!("create-admin is not yet implemented");
    Ok(())
}
