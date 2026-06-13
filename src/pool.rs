use anyhow::Result;
use deadpool_postgres::{Pool, Config as DeadpoolConfig, PoolConfig as DeadpoolPoolConfig, Runtime, Object};
use tokio_postgres::NoTls;
use tracing::debug;
use std::sync::Arc;

use crate::config::PoolConfig;
use crate::errors::{MCPError, Result as MCPResult};

/// Connection pool wrapper using deadpool-postgres
pub struct ConnectionPool {
    pool: Pool,
    max_size: u32,
}

impl ConnectionPool {
    pub async fn new(connection_string: &str, config: PoolConfig) -> Result<Self> {
        debug!("Creating connection pool with config: {:?}", config);

        let cfg = DeadpoolConfig {
            url: Some(connection_string.to_string()),
            pool: Some(DeadpoolPoolConfig {
                max_size: config.max_size as usize,
                timeouts: deadpool_postgres::Timeouts {
                    wait: Some(config.queue_timeout),
                    create: Some(std::time::Duration::from_secs(5)),
                    recycle: Some(std::time::Duration::from_secs(300)), // Recycle connections after 5 minutes
                },
                queue_mode: deadpool::managed::QueueMode::Lifo,
            }),
            ..Default::default()
        };

        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;

        // Test the pool by acquiring a connection
        let _conn = pool.get().await
            .map_err(|e| anyhow::anyhow!("Failed to establish database connection: {}", e))?;

        Ok(Self {
            pool,
            max_size: config.max_size,
        })
    }

    /// Acquire a connection from the pool
    /// Returns Arc<Object> which dereferences to Client
    pub async fn acquire(&self) -> MCPResult<Arc<Object>> {
        self.pool
            .get()
            .await
            .map(|obj| Arc::new(obj))
            .map_err(|_| MCPError::PoolError("Connection pool exhausted".into()))
    }

    /// Release a connection back to the pool (handled automatically by deadpool)
    pub fn release(&self, _conn: Arc<Object>) {
        // deadpool automatically returns connections to the pool
    }

    pub fn active_count(&self) -> u32 {
        self.pool.status().size as u32
    }

    pub fn max_size(&self) -> u32 {
        self.max_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_config() {
        let cfg = PoolConfig {
            min_size: 2,
            max_size: 10,
            queue_timeout: Duration::from_secs(10),
        };
        assert!(cfg.max_size >= cfg.min_size);
    }
}
