use std::sync::atomic::{AtomicU64, Ordering};
use std::hash::{DefaultHasher, Hash, Hasher};
use anyhow::Result;
use once_cell::sync::Lazy;

/// Per-CPU-shard counters (data-oriented: no queue allocations on hot path).
/// Each shard is on its own cache line to prevent false sharing.
/// Producers increment atomics directly; consumers sum all shards.
const NUM_SHARDS: usize = 16;

#[repr(align(64))]
struct MetricShard {
    requests: AtomicU64,
    errors: AtomicU64,
}

static SHARDS: Lazy<[MetricShard; NUM_SHARDS]> = Lazy::new(|| {
    [0; NUM_SHARDS].map(|_| MetricShard {
        requests: AtomicU64::new(0),
        errors: AtomicU64::new(0),
    })
});

thread_local! {
    static THREAD_SHARD: usize = calc_thread_shard();
}

fn calc_thread_shard() -> usize {
    const MASK: usize = NUM_SHARDS - 1;
    let tid = std::thread::current().id();
    let mut hasher = DefaultHasher::new();
    tid.hash(&mut hasher);
    hasher.finish() as usize & MASK
}

/// Increment request count on the calling thread's shard (cheap: one atomic add).
#[inline]
pub fn inc_requests() {
    let shard = THREAD_SHARD.with(|s| *s);
    SHARDS[shard].requests.fetch_add(1, Ordering::Relaxed);
}

/// Increment error count on the calling thread's shard.
#[inline]
pub fn inc_errors() {
    let shard = THREAD_SHARD.with(|s| *s);
    SHARDS[shard].errors.fetch_add(1, Ordering::Relaxed);
}

/// Read-and-reset all shard counters, returning totals.
pub fn drain_counters() -> (u64, u64) {
    let mut total_reqs = 0u64;
    let mut total_errs = 0u64;
    for shard in SHARDS.iter() {
        total_reqs += shard.requests.swap(0, Ordering::Relaxed);
        total_errs += shard.errors.swap(0, Ordering::Relaxed);
    }
    (total_reqs, total_errs)
}

pub fn init_metrics(port: u16) -> Result<()> {
    use prometheus::{Encoder, IntCounter, Registry, TextEncoder};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use std::net::SocketAddr;

    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()?;

    let registry = Arc::new(Registry::new());
    let request_total = IntCounter::new("requests_total", "Total requests processed")?;
    let error_total = IntCounter::new("request_errors_total", "Total request errors")?;
    registry.register(Box::new(request_total.clone()))?;
    registry.register(Box::new(error_total.clone()))?;

    // Background task: drain shard counters into Prometheus counters
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            let (reqs, errs) = drain_counters();
            if reqs > 0 {
                request_total.inc_by(reqs);
            }
            if errs > 0 {
                error_total.inc_by(errs);
            }
        }
    });

    // Metrics HTTP endpoint
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .expect("Failed to bind metrics server");

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let reg = Arc::clone(&registry);
                    tokio::spawn(async move {
                        let (mut reader, mut writer) = tokio::io::split(stream);
                        let mut buf = vec![0; 1024];

                        if let Ok(n) = reader.read(&mut buf).await {
                            let request = String::from_utf8_lossy(&buf[..n]);
                            if request.starts_with("GET /metrics") {
                                let encoder = TextEncoder::new();
                                let metric_families = reg.gather();
                                let mut metrics_buf = Vec::new();
                                if encoder.encode(&metric_families, &mut metrics_buf).is_ok() {
                                    let response = format!(
                                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n{}",
                                        String::from_utf8_lossy(&metrics_buf)
                                    );
                                    let _ = writer.write_all(response.as_bytes()).await;
                                }
                            }
                        }
                    });
                }
                Err(e) => eprintln!("Metrics server error: {}", e),
            }
        }
    });

    Ok(())
}
