use anyhow::Result;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_postgres::{connect, Client, NoTls};
use tracing::{debug, error, warn};

use crate::config::PoolConfig;
use crate::errors::{MCPError, Result as MCPResult};

/// Lock-free connection pool using a semaphore for capacity control
/// and a lock-free queue for idle connections.
///
/// Design:
///   - Semaphore tracks total permit count (max_size).
///   - Idle connections live in a lock-free SegQueue.
///   - Acquire: pop idle (fast path), or wait for semaphore + create new.
///   - Release: push back to idle, or drop if unhealthy.
pub struct ConnectionPool {
    config: PoolConfig,
    connection_string: String,
    idle_connections: crossbeam::queue::SegQueue<Arc<Client>>,
    /// Total connections in existence (idle + borrowed), an upper bound.
    active_connections: AtomicU32,
    /// Semaphore with max_size permits — controls lazy creation concurrency.
    semaphore: Semaphore,
}

impl ConnectionPool {
    pub async fn new(connection_string: &str, config: PoolConfig) -> Result<Self> {
        debug!("Creating connection pool with config: {:?}", config);

        let idle_queue = crossbeam::queue::SegQueue::new();
        let mut created = 0u32;

        for _ in 0..config.min_size {
            match connect(connection_string, NoTls).await {
                Ok((client, connection)) => {
                    tokio::spawn(async move {
                        if let Err(e) = connection.await {
                            error!("Connection error: {}", e);
                        }
                    });
                    idle_queue.push(Arc::new(client));
                    created += 1;
                }
                Err(e) => {
                    warn!("Failed to create initial connection: {}", e);
                }
            }
        }

        if created == 0 {
            return Err(anyhow::anyhow!(
                "Failed to establish any database connection. Check DATABASE_URL and ensure PostgreSQL is running."
            ));
        }

        Ok(Self {
            config,
            connection_string: connection_string.to_string(),
            idle_connections: idle_queue,
            active_connections: AtomicU32::new(created),
            semaphore: Semaphore::new(created as usize),
        })
    }

    /// Acquire a connection from the pool.
    ///
    /// Fast path: pop from idle queue.
    /// Slow path: acquire semaphore permit, create new connection.
    pub async fn acquire(&self) -> MCPResult<Arc<Client>> {
        // Fast path: return idle connection immediately
        if let Some(conn) = self.idle_connections.pop() {
            if is_connection_alive(&conn) {
                return Ok(conn);
            }
            // Connection is dead — drop it and fall through to create new
            self.active_connections.fetch_sub(1, Ordering::Relaxed);
        }

        // Slow path: acquire a permit (blocks if at max capacity)
        let permit = tokio::time::timeout(
            self.config.queue_timeout,
            self.semaphore.acquire(),
        )
        .await
        .map_err(|_| MCPError::PoolError("Connection pool exhausted (timeout)".into()))?
        .map_err(|_| MCPError::PoolError("Connection pool semaphore closed".into()))?;

        // Try idle again (one may have been released while we waited)
        if let Some(conn) = self.idle_connections.pop() {
            permit.forget();
            self.active_connections.fetch_add(1, Ordering::Relaxed);
            return Ok(conn);
        }

        // Create new connection
        match connect(&self.connection_string, NoTls).await {
            Ok((client, connection)) => {
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        error!("Lazy connection error: {}", e);
                    }
                });
                permit.forget();
                self.active_connections.fetch_add(1, Ordering::Relaxed);
                Ok(Arc::new(client))
            }
            Err(e) => {
                error!("Failed to create lazy connection: {}", e);
                drop(permit);
                Err(MCPError::PoolError(
                    "Failed to create database connection".into(),
                ))
            }
        }
    }

    /// Release a connection back to the pool.
    pub fn release(&self, conn: Arc<Client>) {
        if is_connection_alive(&conn) {
            self.idle_connections.push(conn);
        } else {
            self.active_connections.fetch_sub(1, Ordering::Relaxed);
        }
        debug!("Connection released back to pool");
    }

    /// Current approximate connection count.
    pub fn active_count(&self) -> u32 {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Maximum pool size.
    pub fn max_size(&self) -> u32 {
        self.config.max_size
    }
}

/// Quick health check — ping the connection without a full round-trip.
fn is_connection_alive(conn: &Client) -> bool {
    // tokio_postgres::Client::is_closed returns true if the connection is broken.
    !conn.is_closed()
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
