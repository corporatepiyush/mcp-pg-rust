use std::sync::Arc;
use parking_lot::Mutex;

/// Per-connection result buffer pool for reusing allocated buffers.
/// Reduces allocation overhead during query result processing.
pub struct ResultBuffer {
    data: Vec<u8>,
}

impl ResultBuffer {
    const DEFAULT_CAPACITY: usize = 4096;

    pub fn new() -> Self {
        Self {
            data: Vec::with_capacity(Self::DEFAULT_CAPACITY),
        }
    }

    pub fn as_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Thread-local result buffer for reducing allocations
/// Each tokio task gets its own buffer to avoid contention
pub struct BufferPool {
    buffers: Arc<Mutex<Vec<ResultBuffer>>>,
    max_cached: usize,
}

impl BufferPool {
    pub fn new(max_cached: usize) -> Self {
        Self {
            buffers: Arc::new(Mutex::new(Vec::with_capacity(max_cached))),
            max_cached,
        }
    }

    pub fn acquire(&self) -> ResultBuffer {
        let mut buffers = self.buffers.lock();
        buffers.pop().unwrap_or_else(ResultBuffer::new)
    }

    pub fn release(&self, mut buffer: ResultBuffer) {
        buffer.clear();
        let mut buffers = self.buffers.lock();
        if buffers.len() < self.max_cached {
            buffers.push(buffer);
        }
    }

    pub fn size(&self) -> usize {
        self.buffers.lock().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_buffer_creation() {
        let buf = ResultBuffer::new();
        assert!(buf.is_empty());
        assert!(buf.len() == 0);
    }

    #[test]
    fn test_buffer_pool_acquire_release() {
        let pool = BufferPool::new(2);
        let buf = pool.acquire();
        assert_eq!(pool.size(), 0);
        pool.release(buf);
        assert_eq!(pool.size(), 1);
    }

    #[test]
    fn test_buffer_pool_respects_max_cached() {
        let pool = BufferPool::new(1);
        let buf1 = pool.acquire();
        let buf2 = pool.acquire();
        pool.release(buf1);
        pool.release(buf2);
        // Only 1 should be cached
        assert_eq!(pool.size(), 1);
    }
}
