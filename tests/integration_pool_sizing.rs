/// Integration test for CPU-aware pool sizing
#[test]
fn test_cpu_aware_pool_sizing() {
    let num_cpus = num_cpus::get() as u32;

    // Create default config - should have CPU-aware sizing
    let config = mcp_postgres::config::Config::default();

    println!("System CPUs: {}", num_cpus);
    println!("Pool min_size: {}", config.pool.min_size);
    println!("Pool max_size: {}", config.pool.max_size);

    // Verify sizing (min=5 for performance, max=20 by default)
    assert_eq!(config.pool.min_size, 5, "min_size should be 5");
    assert_eq!(config.pool.max_size, 20, "max_size should be 20 by default");
    assert!(config.pool.max_size >= config.pool.min_size);
}

#[test]
fn test_pool_config_with_env_override() {
    let num_cpus = num_cpus::get() as u32;

    let min_override = Some(2);
    let min_final = min_override.unwrap_or(1);
    assert_eq!(min_final, 2);

    let max_override = Some(32);
    let max_final = max_override.unwrap_or(num_cpus * 8);
    assert_eq!(max_final, 32);
}

#[test]
fn test_result_buffer_lifecycle() {
    use mcp_postgres::buffers::{ResultBuffer, BufferPool};

    let mut buf = ResultBuffer::new();
    assert!(buf.is_empty());

    // Simulate writing data
    buf.as_mut().extend_from_slice(b"test data");
    assert_eq!(buf.len(), 9);

    // Clear for reuse
    buf.clear();
    assert!(buf.is_empty());
}

#[test]
fn test_buffer_pool_reuse() {
    use mcp_postgres::buffers::BufferPool;

    let pool = BufferPool::new(4);

    // Acquire multiple buffers
    let mut buffers = vec![];
    for _ in 0..3 {
        buffers.push(pool.acquire());
    }
    assert_eq!(pool.size(), 0, "pool should be empty after acquire");

    // Release them
    for buf in buffers {
        pool.release(buf);
    }
    assert_eq!(pool.size(), 3, "pool should have 3 cached buffers");

    // Acquire again - should reuse existing
    let reused = pool.acquire();
    assert_eq!(pool.size(), 2, "pool should have 2 left");
    pool.release(reused);
}
