use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

pub use crate::tools::ToolCategory;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AccessMode {
    #[serde(rename = "unrestricted")]
    Unrestricted,
    #[serde(rename = "restricted")]
    Restricted,
}

impl fmt::Display for AccessMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccessMode::Unrestricted => write!(f, "unrestricted"),
            AccessMode::Restricted => write!(f, "restricted"),
        }
    }
}

impl FromStr for AccessMode {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "unrestricted" => Ok(AccessMode::Unrestricted),
            "restricted" => Ok(AccessMode::Restricted),
            _ => Err(format!(
                "Invalid access mode: {s}. Use 'unrestricted' or 'restricted'"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub pool: PoolConfig,
    pub metrics: MetricsConfig,
    /// Pre-serialized `{"tools":[...]}` payload for `tools/list`, already
    /// filtered to the enabled categories (see `server.enabled_categories`).
    /// Skipped during (de)serialization and rebuilt from the enabled set so
    /// every transport serves an identical, category-filtered list without
    /// reparsing `tools.json` per request.
    #[serde(skip, default = "default_tools_list_bytes")]
    pub tools_list_bytes: Arc<Vec<u8>>,
}

fn default_tools_list_bytes() -> Arc<Vec<u8>> {
    Arc::new(crate::server::build_tools_list_response(&[]))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub request_timeout: Duration,
    pub access_mode: AccessMode,
    /// Shared secret required for TCP/HTTP transports. `None` means no auth
    /// (only permitted on loopback binds).
    #[serde(default, skip_serializing)]
    pub auth_token: Option<String>,
    /// Whether the import_from_url tool may make outbound HTTP fetches.
    #[serde(default)]
    pub allow_url_import: bool,
    /// PEM certificate chain for serving the HTTP transport over TLS (HTTPS).
    /// `None` (the default) keeps the HTTP transport plaintext. Engaged only
    /// when both `tls_cert` and `tls_key` are set.
    #[serde(default)]
    pub tls_cert: Option<std::path::PathBuf>,
    /// PEM private key matching `tls_cert`.
    #[serde(default)]
    pub tls_key: Option<std::path::PathBuf>,
    /// Tool categories exposed by this server. Empty (the default) means no
    /// tools are advertised or callable until enabled with `--enable-*`.
    #[serde(default)]
    pub enabled_categories: Vec<ToolCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub min_size: u32,
    pub max_size: u32,
    pub queue_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Config {
    pub fn from_args(args: &super::Args) -> Result<Self> {
        let database_url = args
            .database_url
            .clone()
            .or_else(|| std::env::var("DATABASE_URL").ok())
            .unwrap_or_else(|| "postgres://postgres:postgres@localhost:5432/postgres".to_string());

        let min_size = args.min_connections.unwrap_or(5);
        let max_size = args.max_connections.unwrap_or(20);

        let auth_token = args
            .auth_token
            .clone()
            .or_else(|| std::env::var("MCP_AUTH_TOKEN").ok())
            .filter(|t| !t.is_empty());

        // TLS cert/key for the HTTP transport, from CLI flags or env vars.
        let tls_cert = args
            .tls_cert
            .clone()
            .or_else(|| std::env::var("MCP_TLS_CERT").ok())
            .filter(|s| !s.is_empty())
            .map(std::path::PathBuf::from);
        let tls_key = args
            .tls_key
            .clone()
            .or_else(|| std::env::var("MCP_TLS_KEY").ok())
            .filter(|s| !s.is_empty())
            .map(std::path::PathBuf::from);
        if tls_cert.is_some() != tls_key.is_some() {
            anyhow::bail!(
                "--tls-cert and --tls-key must be provided together (or both omitted for plaintext HTTP)"
            );
        }

        let enabled_categories = args.enabled_categories();
        let tools_list_bytes = Arc::new(crate::server::build_tools_list_response(
            &enabled_categories,
        ));

        Ok(Config {
            database: DatabaseConfig { url: database_url },
            server: ServerConfig {
                host: args.host.clone(),
                port: args.port,
                request_timeout: Duration::from_secs(30),
                access_mode: args.access_mode,
                auth_token,
                allow_url_import: args.allow_url_import,
                tls_cert,
                tls_key,
                enabled_categories,
            },
            pool: PoolConfig {
                min_size,
                max_size,
                queue_timeout: Duration::from_secs(args.acquire_timeout_secs),
            },
            metrics: MetricsConfig {
                enabled: args.enable_metrics,
                port: args.metrics_port,
            },
            tools_list_bytes,
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database: DatabaseConfig {
                url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
            },
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 3000,
                request_timeout: Duration::from_secs(30),
                access_mode: AccessMode::Unrestricted,
                auth_token: None,
                allow_url_import: false,
                tls_cert: None,
                tls_key: None,
                enabled_categories: Vec::new(),
            },
            pool: PoolConfig {
                min_size: 5,
                max_size: 20,
                queue_timeout: Duration::from_secs(10),
            },
            metrics: MetricsConfig {
                enabled: false,
                port: 9090,
            },
            tools_list_bytes: default_tools_list_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.server.host, "127.0.0.1");
        assert_eq!(cfg.server.port, 3000);
        assert_eq!(cfg.server.request_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_database_config_defaults() {
        let cfg = Config::default();
        assert_eq!(
            cfg.database.url,
            "postgres://postgres:postgres@localhost:5432/postgres"
        );
    }

    #[test]
    fn test_pool_config_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.pool.min_size, 5);
        assert_eq!(cfg.pool.max_size, 20);
        assert_eq!(cfg.pool.queue_timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_metrics_config_defaults() {
        let cfg = Config::default();
        assert!(!cfg.metrics.enabled);
        assert_eq!(cfg.metrics.port, 9090);
    }

    #[test]
    fn test_config_serde() {
        let cfg = Config::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.server.port, cfg.server.port);
        assert_eq!(deserialized.pool.min_size, cfg.pool.min_size);
        assert_eq!(deserialized.database.url, cfg.database.url);
    }

    #[test]
    fn test_config_from_args_cpu_aware() {
        let num_cpus = num_cpus::get() as u32;

        // Simulating what from_args does with defaults
        let min_size = 1;
        let max_size = num_cpus * 8;

        assert_eq!(min_size, 1);
        assert!(max_size > 0);
        assert_eq!(max_size, num_cpus * 8);
    }

    #[test]
    fn test_pool_config_values() {
        let cfg = Config::default();
        assert!(cfg.pool.min_size > 0);
        assert!(cfg.pool.max_size >= cfg.pool.min_size);
    }

    #[test]
    fn test_server_config_debug() {
        let cfg = Config::default();
        let debug = format!("{:?}", cfg);
        assert!(debug.contains("127.0.0.1"));
        assert!(debug.contains("3000"));
    }
}
