use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_path: PathBuf,
    pub ollama_host: String,
    pub ollama_port: u16,
    pub model: String,
    pub token: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bind = env::var("BRIDGE_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8787".into())
            .parse()
            .context("BRIDGE_BIND must be a socket address")?;

        let database_path = env::var_os("BRIDGE_DATABASE")
            .map(PathBuf::from)
            .unwrap_or_else(default_database_path);

        let token = if let Ok(value) = env::var("BRIDGE_API_TOKEN") {
            value
        } else {
            let path = env::var_os("BRIDGE_TOKEN_FILE")
                .map(PathBuf::from)
                .unwrap_or_else(default_token_path);
            std::fs::read_to_string(&path)
                .with_context(|| format!("unable to read token file {}", path.display()))?
                .trim()
                .to_owned()
        };
        if token.len() < 32 {
            bail!("Bridge API token must contain at least 32 characters");
        }

        Ok(Self {
            bind,
            database_path,
            ollama_host: env::var("BRIDGE_OLLAMA_HOST")
                .unwrap_or_else(|_| "http://127.0.0.1".into()),
            ollama_port: env::var("BRIDGE_OLLAMA_PORT")
                .unwrap_or_else(|_| "11434".into())
                .parse()
                .context("BRIDGE_OLLAMA_PORT must be a port number")?,
            model: env::var("BRIDGE_MODEL").unwrap_or_else(|_| "gemma4:26b".into()),
            token,
        })
    }
}

fn application_support() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/Application Support/Bridge")
}

fn default_database_path() -> PathBuf {
    application_support().join("bridge.db")
}

fn default_token_path() -> PathBuf {
    application_support().join("token")
}
