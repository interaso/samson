use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub db_path: String,
    pub poll_interval: u64,
    pub api_host: String,
    pub api_port: u16,
    pub metrics_host: String,
    pub metrics_port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let db_path = std::env::var("DATABASE_PATH").unwrap_or_else(|_| "samson.db".to_string());

        let poll_interval = std::env::var("POLL_INTERVAL")
            .unwrap_or_else(|_| "1".to_string())
            .parse::<u64>()
            .context("POLL_INTERVAL must be a valid number")?;

        if poll_interval == 0 {
            anyhow::bail!("POLL_INTERVAL must be greater than 0");
        }

        let api_host = std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

        let api_port = std::env::var("API_PORT")
            .unwrap_or_else(|_| "3030".to_string())
            .parse::<u16>()
            .context("API_PORT must be a valid port number (0-65535)")?;

        let metrics_host = std::env::var("METRICS_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

        let metrics_port = std::env::var("METRICS_PORT")
            .unwrap_or_else(|_| "9090".to_string())
            .parse::<u16>()
            .context("METRICS_PORT must be a valid port number (0-65535)")?;

        Ok(Self {
            db_path,
            poll_interval,
            api_host,
            api_port,
            metrics_host,
            metrics_port,
        })
    }
}
