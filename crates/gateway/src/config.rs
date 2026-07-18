use std::{env, net::SocketAddr, path::PathBuf, str::FromStr, time::Duration};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_path: PathBuf,
    pub ollama_host: String,
    pub ollama_port: u16,
    pub model: String,
    pub token: String,
    pub tools: ToolConfig,
}

#[derive(Debug, Clone)]
pub struct ToolConfig {
    pub timeout: Duration,
    pub fetch_max_bytes: usize,
    pub page_max_chars: usize,
    pub max_rounds: u32,
    pub search_max_results: usize,
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(15),
            fetch_max_bytes: 1_000_000,
            page_max_chars: 6_000,
            max_rounds: 8,
            search_max_results: 5,
        }
    }
}

impl ToolConfig {
    fn from_env() -> Result<Self> {
        let default = Self::default();
        Ok(Self {
            timeout: Duration::from_secs(env_parse(
                "BRIDGE_TOOL_TIMEOUT_SECS",
                default.timeout.as_secs(),
            )?),
            fetch_max_bytes: env_parse("BRIDGE_TOOL_FETCH_MAX_BYTES", default.fetch_max_bytes)?,
            page_max_chars: env_parse("BRIDGE_TOOL_PAGE_MAX_CHARS", default.page_max_chars)?,
            max_rounds: env_parse("BRIDGE_TOOL_MAX_ROUNDS", default.max_rounds)?,
            search_max_results: env_parse(
                "BRIDGE_SEARCH_MAX_RESULTS",
                default.search_max_results,
            )?,
        })
    }
}

fn env_parse<T: FromStr>(name: &str, default: T) -> Result<T> {
    match env::var(name) {
        Ok(value) => value
            .parse()
            .map_err(|_| anyhow::anyhow!("{name} must be a number, got '{value}'")),
        Err(_) => Ok(default),
    }
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
            tools: ToolConfig::from_env()?,
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
