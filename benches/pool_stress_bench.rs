use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Benchmark pool acquire/release under various concurrency levels
/// This measures pool efficiency as concurrency increases
fn bench_pool_concurrency(c: &mut Criterion) {
    let mut group = c.benchmark_group("pool_concurrency");
    group.sample_size(10);

    // Note: This would require setting up a real pool with DB connection
    // For now, we'll focus on lock-free queue primitives

    let concurrency_levels = vec![1, 2, 4, 8, 16, 32];

    for concurrency in concurrency_levels {
        group.throughput(Throughput::Elements(concurrency as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads", concurrency)),
            &concurrency,
            |b, &concurrency| {
                use crossbeam::queue::SegQueue;
                use std::sync::Arc;

                let queue = Arc::new(SegQueue::new());

                // Pre-populate queue
                for i in 0..concurrency * 4 {
                    queue.push(Arc::new(i));
                }

                let counter = Arc::new(AtomicU64::new(0));

                b.iter(|| {
                    let handles: Vec<_> = (0..concurrency)
                        .map(|_| {
                            let queue = Arc::clone(&queue);
                            let counter = Arc::clone(&counter);
                            std::thread::spawn(move || {
                                for _ in 0..100 {
                                    if let Some(item) = queue.pop() {
                                        counter.fetch_add(1, Ordering::Relaxed);
                                        queue.push(item);
                                    }
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark the atomic counter overhead in pool acquire
fn bench_atomic_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("atomic_overhead");

    let counter = Arc::new(AtomicU64::new(0));
    let concurrency_levels = vec![1, 4, 8, 16];

    for concurrency in concurrency_levels {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads", concurrency)),
            &concurrency,
            |b, &concurrency| {
                let counter = Arc::clone(&counter);
                b.iter(|| {
                    let handles: Vec<_> = (0..concurrency)
                        .map(|_| {
                            let counter = Arc::clone(&counter);
                            std::thread::spawn(move || {
                                for _ in 0..1000 {
                                    counter.fetch_add(1, Ordering::Relaxed);
                                    counter.fetch_sub(1, Ordering::Relaxed);
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark notify overhead
fn bench_notify_contention(c: &mut Criterion) {
    use tokio::sync::Notify;

    let mut group = c.benchmark_group("notify_contention");
    group.sample_size(10);

    let concurrency_levels = vec![1, 2, 4, 8];

    for concurrency in concurrency_levels {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads", concurrency)),
            &concurrency,
            |b, &concurrency| {
                let notify = Arc::new(Notify::new());
                let rt = tokio::runtime::Runtime::new().unwrap();

                b.iter(|| {
                    let handles: Vec<_> = (0..concurrency)
                        .map(|i| {
                            let notify = Arc::clone(&notify);
                            std::thread::spawn(move || {
                                if i == 0 {
                                    // Notifier thread
                                    for _ in 0..100 {
                                        notify.notify_one();
                                    }
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_pool_concurrency,
    bench_atomic_overhead,
    bench_notify_contention,
);
criterion_main!(benches);
