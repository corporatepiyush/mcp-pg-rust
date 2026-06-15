//! PostgreSQL connection pool — lock-free implementation.
//!
//! Uses `LockFreePool<tokio_postgres::Client>` internally.  No mutexes,
//! no semaphores, no kernel transitions on the hot path — only CAS loops
//! on `crossbeam::queue::ArrayQueue` and atomic size tracking.
//!
//! The `acquire()` method returns a `PooledConnection` which auto-returns
//! to the pool on `Drop`.  There is no explicit `release()` needed.

use std::time::Duration;
use tokio_postgres::{Client, NoTls};
use tracing::debug;

use crate::config::PoolConfig;
use crate::errors::{MCPError, Result as MCPResult};
use crate::lockfree_pool::{BoxFuture, CreateFn, LockFreePool, PoolConfig as LFPoolConfig, PooledConnection, PoolError, ValidateFn};

/// Wrapper around the lock-free connection pool.
pub struct ConnectionPool {
    inner: LockFreePool<Client>,
    max_size: u32,
}

impl ConnectionPool {
    pub async fn new(connection_string: &str, config: PoolConfig) -> anyhow::Result<Self> {
        debug!("Creating lock-free connection pool: max_size={}", config.max_size);

        let conn_string = connection_string.to_string();
        let create_timeout = Duration::from_secs(5);

        let create = {
            let cs = conn_string.clone();
            Box::new(move || {
                let cs = cs.clone();
                Box::pin(async move {
                    let (client, connection) = tokio_postgres::connect(&cs, NoTls)
                        .await
                        .map_err(|e| e.to_string())?;
                    tokio::spawn(connection);
                    Ok(client)
                }) as BoxFuture<'static, Result<Client, String>>
            }) as CreateFn<Client>
        };

        let validate = Box::new(|client: &Client| {
            !client.is_closed()
        }) as ValidateFn<Client>;

        let lf_config = LFPoolConfig {
            max_size: config.max_size,
            create_timeout,
            wait_timeout: config.queue_timeout,
        };

        let pool = LockFreePool::new(create, validate, &lf_config);

        // Test the pool by acquiring a connection
        let test_conn = pool.acquire().await.map_err(|e| {
            anyhow::anyhow!("Failed to establish database connection: {e}")
        })?;
        drop(test_conn);

        Ok(Self {
            inner: pool,
            max_size: config.max_size,
        })
    }

    /// Acquire a connection from the pool.
    ///
    /// Returns a `PooledConnection<Client>` which implements `Deref<Target = Client>`
    /// and automatically returns to the pool when dropped.
    pub async fn acquire(&self) -> MCPResult<PooledConnection<Client>> {
        self.inner.acquire().await.map_err(|e| match e {
            PoolError::Timeout => {
                MCPError::PoolError("Connection pool timeout: no connection available".into())
            }
            PoolError::Closed => {
                MCPError::PoolError("Connection pool is closed".into())
            }
            PoolError::CreateFailed(msg) => {
                MCPError::PoolError(format!("Failed to create connection: {msg}"))
            }
        })
    }

    /// Release a connection back to the pool.
    ///
    /// With `PooledConnection`, this is automatic on `Drop`.  This method
    /// exists for backward compatibility with existing callers.
    pub fn release(&self, _conn: PooledConnection<Client>) {
        // Connection auto-returns to pool on Drop
    }

    pub fn active_count(&self) -> u32 {
        self.inner.status().size
    }

    pub const fn max_size(&self) -> u32 {
        self.max_size
    }

    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    /// Close the pool, dropping all idle connections.
    pub fn close(&self) {
        self.inner.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[test]
    fn test_config() {
        let cfg = PoolConfig {
            min_size: 2,
            max_size: 10,
            queue_timeout: Duration::from_secs(10),
        };
        assert!(cfg.max_size >= cfg.min_size);
    }

    #[tokio::test]
    async fn test_pool_create_and_acquire() {
        // This test requires a real PostgreSQL instance.
        // It's a no-op if DATABASE_URL is not set.
        if std::env::var("DATABASE_URL").is_err() && std::env::var("PGHOST").is_err() {
            eprintln!("Skipping: no database available");
            return;
        }
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
        let config = PoolConfig {
            min_size: 1,
            max_size: 5,
            queue_timeout: Duration::from_secs(5),
        };
        let pool = ConnectionPool::new(&url, config).await.unwrap();
        assert_eq!(pool.max_size(), 5);
        let conn = pool.acquire().await.unwrap();
        assert!(!conn.is_closed());
        pool.release(conn);
        sleep(Duration::from_millis(50)).await;
        assert!(pool.active_count() > 0);
    }
}
