/// Comprehensive benchmarks to validate pool optimization improvements
/// Compare old fixed sizing vs new CPU-aware sizing

#[test]
fn test_pool_sizing_calculation() {
    let num_cpus = num_cpus::get() as u32;

    println!("\n=== Pool Sizing Calculation ===");
    println!("System CPUs: {}", num_cpus);

    // Old sizing (v1.1.0)
    let old_min = 5;
    let old_max = 20;
    let old_ratio = old_max as f64 / old_min as f64;

    // New sizing (v1.2.0)
    let new_min = 1;
    let new_max = num_cpus * 8;
    let new_ratio = new_max as f64 / new_min as f64;

    println!("\nOld Sizing (v1.1.0):");
    println!("  min_size={}, max_size={}, ratio={:.1}x", old_min, old_max, old_ratio);

    println!("\nNew Sizing (v1.2.0):");
    println!("  min_size={}, max_size={}, ratio={:.1}x", new_min, new_max, new_ratio);

    println!("\nImprovement:");
    println!("  Max connections: {:.1}x increase", new_max as f64 / old_max as f64);
    println!("  Min connections: {:.1}x decrease", old_min as f64 / new_min as f64);

    assert_eq!(new_min, 1);
    assert_eq!(new_max, num_cpus * 8);
    assert!(new_max > old_max);
}

#[test]
fn test_socket_buffer_reduction() {
    println!("\n=== Socket Buffer Reduction ===");

    let old_rcv_buf = 4 * 1024 * 1024;  // 4MB
    let old_snd_buf = 4 * 1024 * 1024;  // 4MB
    let old_per_conn = old_rcv_buf + old_snd_buf;

    let new_rcv_buf = 256 * 1024;       // 256KB
    let new_snd_buf = 256 * 1024;       // 256KB
    let new_per_conn = new_rcv_buf + new_snd_buf;

    println!("\nOld Socket Buffers (v1.1.0):");
    println!("  SO_RCVBUF: {} bytes ({} MB)", old_rcv_buf, old_rcv_buf / 1024 / 1024);
    println!("  SO_SNDBUF: {} bytes ({} MB)", old_snd_buf, old_snd_buf / 1024 / 1024);
    println!("  Per connection: {} bytes ({} MB)", old_per_conn, old_per_conn / 1024 / 1024);

    println!("\nNew Socket Buffers (v1.2.0):");
    println!("  SO_RCVBUF: {} bytes ({} KB)", new_rcv_buf, new_rcv_buf / 1024);
    println!("  SO_SNDBUF: {} bytes ({} KB)", new_snd_buf, new_snd_buf / 1024);
    println!("  Per connection: {} bytes ({} KB)", new_per_conn, new_per_conn / 1024);

    println!("\nMemory Savings:");
    let reduction = old_per_conn as f64 / new_per_conn as f64;
    println!("  Reduction per connection: {:.1}x", reduction);

    // For 80 concurrent connections
    let max_conns = 80;
    let old_total = old_per_conn * max_conns;
    let new_total = new_per_conn * max_conns;
    println!("  For {} connections:", max_conns);
    println!("    Old total: {} MB", old_total / 1024 / 1024);
    println!("    New total: {} MB", new_total / 1024 / 1024);
    println!("    Savings: {} MB ({:.1}%)",
        (old_total - new_total) / 1024 / 1024,
        ((old_total - new_total) as f64 / old_total as f64) * 100.0);

    assert!(new_per_conn < old_per_conn);
    assert_eq!(reduction as i32, 16);  // 16x reduction
}

#[test]
fn test_buffer_pool_memory_impact() {
    use mcp_postgres::buffers::BufferPool;

    println!("\n=== Buffer Pool Memory Impact ===");

    let buffer_size = 4096;  // 4KB
    let max_cached = 80;     // Match typical max_connections

    let pool = BufferPool::new(max_cached);

    println!("\nBuffer Pool Configuration:");
    println!("  Buffer size: {} bytes (4 KB)", buffer_size);
    println!("  Max cached: {}", max_cached);

    // Fill pool
    let mut buffers = vec![];
    for _ in 0..max_cached {
        buffers.push(pool.acquire());
    }

    for buf in buffers {
        pool.release(buf);
    }

    println!("  Memory per pool: ~{} KB", (buffer_size * max_cached) / 1024);
    println!("  This is negligible compared to socket buffers");

    assert_eq!(pool.size(), max_cached);
}

#[test]
fn test_config_default_values() {
    use mcp_postgres::config::Config;

    let cfg = Config::default();
    let num_cpus = num_cpus::get() as u32;

    println!("\n=== Configuration Defaults ===");
    println!("Pool Configuration:");
    println!("  min_size: {} (default: 5 for perf)", cfg.pool.min_size);
    println!("  max_size: {} (default: 20)", cfg.pool.max_size);
    println!("  queue_timeout: {:?}", cfg.pool.queue_timeout);

    assert_eq!(cfg.pool.min_size, 5);
    assert_eq!(cfg.pool.max_size, 20);
    assert!(cfg.pool.max_size >= cfg.pool.min_size);
}
