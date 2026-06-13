use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub pool: PoolConfig,
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub connection_timeout: Duration,
    pub statement_cache_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub request_timeout: Duration,
    pub max_request_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub min_size: u32,
    pub max_size: u32,
    pub connection_lifetime: Duration,
    pub connection_idle_timeout: Duration,
    pub queue_timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Config {
    pub fn from_args(args: &crate::Args) -> Result<Self> {
        let database_url = args.database_url.clone()
            .or_else(|| std::env::var("DATABASE_URL").ok())
            .unwrap_or_else(|| "postgres://postgres:postgres@localhost:5432/postgres".to_string());

        Ok(Config {
            database: DatabaseConfig {
                url: database_url,
                connection_timeout: Duration::from_secs(10),
                statement_cache_size: 100,
            },
            server: ServerConfig {
                host: args.host.clone(),
                port: args.port,
                request_timeout: Duration::from_secs(30),
                max_request_size: 10 * 1024 * 1024, // 10MB
            },
            pool: PoolConfig {
                min_size: args.min_connections,
                max_size: args.max_connections,
                connection_lifetime: Duration::from_secs(30 * 60), // 30 minutes
                connection_idle_timeout: Duration::from_secs(5 * 60), // 5 minutes
                queue_timeout: Duration::from_secs(10),
            },
            metrics: MetricsConfig {
                enabled: args.enable_metrics,
                port: args.metrics_port,
            },
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database: DatabaseConfig {
                url: "postgres://postgres:postgres@localhost:5432/postgres".to_string(),
                connection_timeout: Duration::from_secs(10),
                statement_cache_size: 100,
            },
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 3000,
                request_timeout: Duration::from_secs(30),
                max_request_size: 10 * 1024 * 1024,
            },
            pool: PoolConfig {
                min_size: 5,
                max_size: 20,
                connection_lifetime: Duration::from_secs(30 * 60),
                connection_idle_timeout: Duration::from_secs(5 * 60),
                queue_timeout: Duration::from_secs(10),
            },
            metrics: MetricsConfig {
                enabled: false,
                port: 9090,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_config_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.server.host, "127.0.0.1");
        assert_eq!(cfg.server.port, 3000);
        assert_eq!(cfg.server.request_timeout, Duration::from_secs(30));
        assert_eq!(cfg.server.max_request_size, 10 * 1024 * 1024);
    }

    #[test]
    fn test_database_config_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.database.url, "postgres://postgres:postgres@localhost:5432/postgres");
        assert_eq!(cfg.database.connection_timeout, Duration::from_secs(10));
        assert_eq!(cfg.database.statement_cache_size, 100);
    }

    #[test]
    fn test_pool_config_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.pool.min_size, 5);
        assert_eq!(cfg.pool.max_size, 20);
        assert_eq!(cfg.pool.connection_lifetime, Duration::from_secs(30 * 60));
        assert_eq!(cfg.pool.connection_idle_timeout, Duration::from_secs(5 * 60));
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
    fn test_config_from_args() {
        let args = crate::Args::parse_from(&["test", "--host", "0.0.0.0", "--port", "8080"]);
        let cfg = Config::from_args(&args).unwrap();
        assert_eq!(cfg.server.host, "0.0.0.0");
        assert_eq!(cfg.server.port, 8080);
    }

    #[test]
    fn test_database_config_from_env() {
        // Without DATABASE_URL set, should use a default
        let args = crate::Args::parse_from(&["test"]);
        let cfg = Config::from_args(&args).unwrap();
        assert!(cfg.database.url.len() > 10);
    }

    #[test]
    fn test_pool_config_values() {
        let cfg = Config::default();
        assert!(cfg.pool.min_size > 0);
        assert!(cfg.pool.max_size >= cfg.pool.min_size);
        assert!(cfg.pool.connection_lifetime > cfg.pool.connection_idle_timeout);
    }

    #[test]
    fn test_server_config_debug() {
        let cfg = Config::default();
        let debug = format!("{:?}", cfg);
        assert!(debug.contains("127.0.0.1"));
        assert!(debug.contains("3000"));
    }
}
