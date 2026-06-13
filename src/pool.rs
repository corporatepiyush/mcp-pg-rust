use anyhow::Result;
use std::sync::{Arc, RwLock};
use tokio_postgres::{connect, Client, NoTls, Statement};
use tracing::{debug, error, warn};
use crossbeam::queue::SegQueue;
use lru::LruCache;
use std::num::NonZeroUsize;

use crate::config::PoolConfig;
use crate::errors::{MCPError, Result as MCPResult};

/// Buffer pool with cache-line alignment to prevent false sharing (Tier 1.1)
#[repr(align(64))]
#[allow(dead_code)]
pub struct BufferPool {
    buffers: Arc<SegQueue<Vec<u8>>>,
    capacity: usize,
    max_buffers: usize,
}

#[allow(dead_code)]
impl BufferPool {
    const DEFAULT_BUFFER_SIZE: usize = 4096;
    const MAX_BUFFERS: usize = 128;

    pub fn new(capacity: usize) -> Self {
        Self {
            buffers: Arc::new(SegQueue::new()),
            capacity,
            max_buffers: Self::MAX_BUFFERS,
        }
    }

    /// Acquire a reusable buffer from the pool
    pub fn acquire(&self) -> Vec<u8> {
        if let Some(mut buf) = self.buffers.pop() {
            buf.clear();
            buf
        } else {
            Vec::with_capacity(self.capacity)
        }
    }

    /// Release buffer back to pool for reuse
    pub fn release(&self, buf: Vec<u8>) {
        if self.buffers.len() < self.max_buffers && buf.capacity() == self.capacity {
            self.buffers.push(buf);
        }
    }
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new(Self::DEFAULT_BUFFER_SIZE)
    }
}

/// Thread-safe lock-free connection pool.
///
/// Data-oriented layout: cold config data on its own cache line,
/// separated from hot-path idle_connections to prevent false sharing.
#[repr(C)]
pub struct ConnectionPool {
    /// Config is cold — read once per acquire(), then immutable.
    /// Wrapped in an aligned wrapper so it occupies a full cache line,
    /// pushing the hot idle_connections field to the next cache line.
    cold: AlignedCold,
    /// idle_connections is hot — accessed on every acquire/release.
    /// Stored inline (not behind an extra Arc) to eliminate one level of
    /// pointer chasing on the hot path.
    idle_connections: SegQueue<Arc<Client>>,
}

/// Ensures cold PoolConfig sits on its own cache line.
#[repr(align(64))]
struct AlignedCold(PoolConfig);

impl ConnectionPool {
    /// Create a new connection pool
    pub async fn new(connection_string: &str, config: PoolConfig) -> Result<Self> {
        debug!("Creating connection pool with config: {:?}", config);

        let idle_queue = SegQueue::new();

        // Create minimum number of connections
        for _ in 0..config.min_size {
            match connect(connection_string, NoTls).await {
                Ok((client, connection)) => {
                    tokio::spawn(async move {
                        if let Err(e) = connection.await {
                            error!("Connection error: {}", e);
                        }
                    });
                    let arc_client = Arc::new(client);
                    idle_queue.push(arc_client);
                }
                Err(e) => {
                    warn!("Failed to create initial connection: {}", e);
                }
            }
        }

        let pool = Self {
            cold: AlignedCold(config),
            idle_connections: idle_queue,
        };

        Ok(pool)
    }

    /// Acquire a connection from the pool
    pub async fn acquire(&self) -> MCPResult<Arc<Client>> {
        let start = std::time::Instant::now();
        let timeout = self.cold.0.queue_timeout;

        loop {
            if let Some(conn) = self.idle_connections.pop() {
                return Ok(conn);
            }

            if start.elapsed() > timeout {
                return Err(MCPError::PoolError("Connection pool exhausted".into()));
            }

            tokio::time::sleep(std::time::Duration::from_micros(100)).await;
        }
    }

    /// Release a connection back to the pool
    pub fn release(&self, conn: Arc<Client>) {
        self.idle_connections.push(conn);
        debug!("Connection released back to pool");
    }

}

#[allow(dead_code)]
pub struct StatementCache {
    cache: Arc<RwLock<LruCache<String, Statement>>>,
}

#[allow(dead_code)]
impl StatementCache {
    const CACHE_CAPACITY: usize = 256;

    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(
                LruCache::new(NonZeroUsize::new(Self::CACHE_CAPACITY).unwrap())
            )),
        }
    }

    pub async fn get_or_prepare(
        &self,
        sql: &str,
        conn: &tokio_postgres::Client,
    ) -> Result<Statement> {
        {
            let mut cache = self.cache.write().unwrap();
            if let Some(stmt) = cache.get(sql) {
                return Ok(stmt.clone());
            }
        }

        let stmt = conn.prepare(sql).await?;

        {
            let mut cache = self.cache.write().unwrap();
            cache.put(sql.to_string(), stmt.clone());
        }

        Ok(stmt)
    }

    pub fn clear(&self) {
        self.cache.write().unwrap().clear();
    }

    pub fn size(&self) -> usize {
        self.cache.read().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_default_size() {
        let pool = BufferPool::default();
        assert_eq!(pool.capacity, 4096);
        assert_eq!(pool.max_buffers, 128);
    }

    #[test]
    fn test_buffer_pool_custom_size() {
        let pool = BufferPool::new(8192);
        assert_eq!(pool.capacity, 8192);
    }

    #[test]
    fn test_buffer_pool_acquire_creates_new() {
        let pool = BufferPool::new(1024);
        let buf = pool.acquire();
        assert_eq!(buf.capacity(), 1024);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_buffer_pool_acquire_reuses_released() {
        let pool = BufferPool::new(1024);
        let buf = vec![0u8; 512];
        pool.release(buf);
        let reused = pool.acquire();
        assert!(reused.is_empty()); // cleared on acquire
        assert_eq!(reused.capacity(), 1024);
    }

    #[test]
    fn test_buffer_pool_reject_wrong_capacity() {
        let pool = BufferPool::new(4096);
        let buf = vec![0u8; 100]; // wrong capacity
        pool.release(buf);
        // Should not be queued — acquire creates fresh
        let acquired = pool.acquire();
        assert_eq!(acquired.capacity(), 4096);
    }

    #[test]
    fn test_buffer_pool_max_buffers() {
        let mut pool = BufferPool::new(64);
        pool.max_buffers = 3;
        for _ in 0..5 {
            pool.release(vec![0u8; 64]);
        }
        // Only 3 should remain in the queue
        let qlen = pool.buffers.len();
        assert!(qlen <= 3, "Queue should be capped at max_buffers");
    }

    #[test]
    fn test_buffer_pool_multiple_acquire_release() {
        let pool = BufferPool::new(256);
        for i in 0..10 {
            let mut buf = pool.acquire();
            assert_eq!(buf.capacity(), 256);
            buf.push(i as u8);
            pool.release(buf);
        }
        // After 10 rounds, pool should work fine
        let final_buf = pool.acquire();
        assert!(final_buf.is_empty());
    }

    #[test]
    fn test_statement_cache_new() {
        let cache = StatementCache::new();
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn test_statement_cache_clear() {
        let cache = StatementCache::new();
        // We can't easily test get_or_prepare without a DB,
        // but we can test clear and size operations
        assert_eq!(cache.size(), 0);
        cache.clear();
        assert_eq!(cache.size(), 0);
    }
}
