mod api;
mod auth;
mod cli;
mod config;
mod db;
mod embed;
mod models;

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

    let app = api::build_router();

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
