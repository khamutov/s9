pub(crate) mod api;
mod auth;
mod cli;
mod config;
mod db;
pub(crate) mod email;
mod embed;
pub(crate) mod events;
mod models;
pub(crate) mod notifications;
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

    let slug_cache = match slug::SlugCache::new(&pool).await {
        Ok(cache) => {
            tracing::info!("slug cache loaded");
            Some(cache)
        }
        Err(e) => {
            tracing::warn!("slug cache init failed (slugs will be unavailable): {e}");
            None
        }
    };

    storage::init_dirs(&config.data_dir).await?;

    let email_sender = match &config.smtp {
        Some(smtp_cfg) => {
            if smtp_cfg.tls == "none" {
                tracing::warn!(
                    "SMTP connection is unencrypted. Set S9_SMTP_TLS=starttls or tls for production use."
                );
            }
            match email::EmailSender::from_config(smtp_cfg) {
                Ok(sender) => {
                    tracing::info!("SMTP configured, email notifications enabled");
                    Some(sender)
                }
                Err(e) => {
                    tracing::error!("failed to initialize SMTP transport: {e}");
                    None
                }
            }
        }
        None => {
            tracing::info!("SMTP not configured, email notifications disabled");
            None
        }
    };

    let smtp_enabled = email_sender.is_some();
    let notif_producer = notifications::NotificationProducer::new(
        pool.clone(),
        config.notification_delay,
        smtp_enabled,
    );

    // Spawn notification worker background task per DD 0.6 §10 / §19 step 8.
    let cancel = tokio_util::sync::CancellationToken::new();
    tokio::spawn(email::notification_worker(
        pool.clone(),
        email_sender,
        cancel.clone(),
    ));

    let event_bus = events::EventBus::new();
    let app = api::build_router(
        pool,
        oidc,
        slug_cache,
        config.data_dir.clone(),
        event_bus,
        notif_producer,
    );

    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    tracing::info!("listening on {}", config.listen);
    axum::serve(listener, app).await?;
    cancel.cancel();

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
