mod api;
mod config;
mod db;
mod modem;
mod poller;
mod utils;

use anyhow::{Context, Result};
use config::Config;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Samson SMS Daemon");

    // Load and validate configuration
    let config = Config::from_env()?;
    info!("Configuration loaded successfully");

    // Initialize database
    let db = Arc::new(Mutex::new(db::Database::new(&config.db_path)?));
    info!("Database initialized at {}", config.db_path);

    // Initialize ModemManager connection
    let modem_manager = Arc::new(modem::ModemManager::new().await?);
    info!("Connected to ModemManager");

    // Start polling service
    let poller = Arc::new(poller::SmsPoller::new(
        modem_manager.clone(),
        db.clone(),
        config.poll_interval,
    ));

    let poller_handle = tokio::spawn(async move {
        poller.start().await;
    });

    // Start HTTP API server
    let app = api::create_router(db.clone(), modem_manager.clone());
    let bind_addr = format!("{}:{}", config.api_host, config.api_port);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .context(format!("Failed to bind to {}", bind_addr))?;
    info!("HTTP API listening on {}", bind_addr);

    let api_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("API server error: {}", e);
        }
    });

    // Start metrics/health server
    let metrics_app = api::create_metrics_router(modem_manager.clone());
    let metrics_bind_addr = format!("{}:{}", config.metrics_host, config.metrics_port);
    let metrics_listener = tokio::net::TcpListener::bind(&metrics_bind_addr)
        .await
        .context(format!("Failed to bind to {}", metrics_bind_addr))?;
    info!("Metrics API listening on {}", metrics_bind_addr);

    let metrics_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(metrics_listener, metrics_app).await {
            tracing::error!("Metrics server error: {}", e);
        }
    });

    // Setup graceful shutdown
    let shutdown_signal = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install CTRL+C signal handler");
        info!("Shutdown signal received");
    };

    // Wait for shutdown signal or task completion
    tokio::select! {
        _ = shutdown_signal => {
            info!("Initiating graceful shutdown...");
        }
        _ = poller_handle => {
            info!("Poller task ended unexpectedly");
        }
        _ = api_handle => {
            info!("API task ended unexpectedly");
        }
        _ = metrics_handle => {
            info!("Metrics task ended unexpectedly");
        }
    }

    info!("Samson SMS Daemon stopped");
    Ok(())
}
