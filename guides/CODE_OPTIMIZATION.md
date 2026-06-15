# Code Optimization Guide for mcp-postgres

**Version**: 1.3.1  
**Last Updated**: 2026-06-13

This guide covers measured optimization techniques, proven regressions to avoid, and profiling strategies for mcp-postgres.

---

## Table of Contents

1. [Performance Baselines](#performance-baselines)
2. [Verified Optimizations](#verified-optimizations)
3. [Measured Regressions](#measured-regressions)
4. [Memory Optimization](#memory-optimization)
5. [Connection Pool Tuning](#connection-pool-tuning)
6. [Buffer Management](#buffer-management)
7. [Query Optimization](#query-optimization)
8. [Benchmarking Guide](#benchmarking-guide)
9. [Profiling Tools](#profiling-tools)
10. [Optimization Checklist](#optimization-checklist)

---

## Performance Baselines

### Current Latency Targets

**All tools MUST maintain P95 < 10ms**:

```
Metadata operations:        < 1ms    (tools/list, show_current_user)
Simple queries:              < 1ms    (SELECT 1)
Moderate queries:            < 3ms    (LIMIT, aggregations)
Complex queries:             < 3ms    (joins, window functions)
Health analysis:             < 6ms    (analyze_db_health)
```

### Current Throughput

```
Single client:     1,000+ req/sec
20 concurrent:     17,713 req/sec
Sustained:         No degradation over 30+ seconds
```

### Measurement Command

```bash
cargo build --release
./target/release/measure_latency
```

---

## Verified Optimizations

### ✅ 1. Memory Allocator Tuning

**Impact**: 5-15% throughput improvement

**Implementation**:
```rust
// In src/main.rs, environment variables:
export MIMALLOC_LARGE_OS_PAGES=1      // Use large OS pages
export MIMALLOC_PAGE_RESET=0           // Keep memory clean
export MIMALLOC_EAGER_COMMIT=0         // Don't eagerly commit
```

**Why it works**:
- Large OS pages reduce TLB misses
- Page reset=0 avoids zeroing on reuse
- Eager commit=0 defers memory binding

**How to verify**:
```bash
MIMALLOC_LARGE_OS_PAGES=1 \
  MIMALLOC_PAGE_RESET=0 \
  MIMALLOC_EAGER_COMMIT=0 \
  cargo run --release --bin measure_latency
```

### ✅ 2. Connection Pool Sizing

**Impact**: -11% regression if wrong, optimal at min=5/max=20

**Configuration**:
```rust
// src/pool.rs
min_connections: 5           // Min always available
max_connections: 20          // Max based on typical load
idle_timeout: 5 minutes      // Recycle after 5 min idle
```

**Why min=5, max=20**:
- min=1 causes connection churn overhead (11% drop)
- max=8*CPUs causes excessive recycling
- 5-20 is proven optimal across test scenarios

**Trade-offs**:
| Config | Throughput | Memory | Churn |
|--------|-----------|--------|-------|
| min=1, max=8*CPU | 18K | Low | High ❌ |
| min=5, max=20 | 20K | Optimal | Low ✅ |
| min=10, max=50 | 19.5K | High | Medium |

### ✅ 3. Release Build Optimization

**Impact**: 3-5x faster than debug

**Configuration** (Cargo.toml):
```toml
[profile.release]
opt-level = 3              # Maximum optimization
lto = "fat"                # Link-time optimization
codegen-units = 1          # Single codegen for better optimization
strip = true               # Remove symbols
panic = "abort"            # Abort on panic
overflow-checks = false    # Disable overflow checking
```

**Build command**:
```bash
cargo build --release
```

**Verify**:
```bash
# Should be ~2.6 MB (stripped binary)
ls -lh target/release/mcp-postgres
```

### ✅ 4. TCP_NODELAY

**Impact**: Reduced latency for rapid fire requests

**Implementation** (src/server.rs):
```rust
if let Err(e) = socket.set_nodelay(true) {
    warn!("Failed to set TCP_NODELAY: {}", e);
}
```

**Why it works**: Disables Nagle's algorithm, allowing small packets to send immediately

### ✅ 5. 4KB Message Buffers

**Impact**: Optimal for typical JSON-RPC messages

**Configuration** (src/server.rs):
```rust
const BUFFER_CAPACITY: usize = 4096;  // Not 16KB or 8KB
```

**Measurements**:
- 4KB: baseline (optimal)
- 8KB: +2.1% latency
- 16KB: +4.5% latency regression ❌

**Why 4KB**:
- Most JSON-RPC messages < 2KB
- Reduces memory pressure
- Improves cache locality
- Minimal wasted space

---

## Measured Regressions

### ❌ DO NOT: Manual Socket Buffer Tuning

**Impact**: +4.5% latency regression

**Bad code**:
```rust
// NEVER DO THIS:
socket.set_recv_buffer(Some(256 * 1024))?;
socket.set_send_buffer(Some(256 * 1024))?;
```

**Why it fails**:
- Tokio has carefully tuned defaults
- Manual tuning interferes with epoll
- Causes excessive memory allocation
- Increases latency, not improves it

### ❌ DO NOT: Increase Message Buffers

**Impact**: +4.5% latency regression

**Bad code**:
```rust
// NEVER DO THIS:
const BUFFER_CAPACITY: usize = 16384;  // ❌ Causes regression
```

**Why it fails**:
- More memory per connection
- Worse cache behavior
- 16KB rarely needed (JSON-RPC typical: 200-2000 bytes)

### ❌ DO NOT: Small Connection Pools

**Impact**: -11% throughput drop

**Bad configuration**:
```rust
// NEVER DO THIS:
min_connections: 1
max_connections: 8 * num_cpus  // ❌ Excessive churn
```

**Why it fails**:
- Connection creation overhead
- Too much recycling
- Rapid connect/disconnect cycles
- Proven 11% regression in testing

### ❌ DO NOT: Force HTTP/2 Prior Knowledge

**Impact**: Health check failures

**Bad code**:
```rust
// NEVER DO THIS:
let client = Client::builder()
    .http2_prior_knowledge()  // ❌ Breaks connections
    .build()?;
```

**Why it fails**:
- Not all servers support h2c (HTTP/2 without TLS)
- Breaks protocol negotiation
- Server may speak HTTP/1.1 only
- Use default (auto-negotiation) instead

### ❌ DO NOT: Too Much Logging

**Impact**: 3-5% throughput loss

**Bad code**:
```rust
// NEVER DO THIS:
debug!("Request received: {:?}", request);  // Per-request! ❌
```

**Why it fails**:
- Debug string formatting has cost
- Per-request logging multiplies overhead
- Blocks on file I/O
- Use info level only, debug disabled in production

---

## Memory Optimization

### 1. Connection Object Pooling

**Current approach**:
```rust
pub async fn acquire(&self) -> MCPResult<Arc<Object>> {
    self.pool
        .get()
        .await
        .map(|obj| Arc::new(obj))
        .map_err(|_| MCPError::PoolError("Pool exhausted".into()))
}
```

**Benefits**:
- Reuses connections, no allocation per request
- Arc wrapper is 16 bytes overhead
- Dead pool handles lifecycle

### 2. Buffer Reuse

**Current approach**:
```rust
// TCP server
let mut response_buf = Vec::with_capacity(65536);

loop {
    response_buf.clear();
    // ... populate buffer ...
    writer.write_all(&response_buf).await?;
}
```

**Benefits**:
- Single allocation per connection
- Clear() resets without deallocation
- 65KB preallocated once, reused infinitely

### 3. String Interning

**Opportunity**: Not yet implemented

**Potential optimization**:
```rust
// For frequently repeated strings (table names, column names, etc.)
use once_cell::sync::Lazy;

static COMMON_STRINGS: Lazy<HashMap<&'static str, Arc<str>>> = 
    Lazy::new(|| {
        // Cache common values
        HashMap::new()
    });
```

**Impact**: Reduces string allocations for repeated identifiers

---

## Connection Pool Tuning

### Pool Configuration

```rust
Pool {
    min_size: 5              // Keep warm
    max_size: 20             // Scale to typical load
    queue_mode: Lifo         // Reuse recently used
    idle_timeout: 300s       // Recycle after 5 min
    create_timeout: 5s       // Fail fast on DB issues
}
```

### Health Check Strategy

```rust
// LockFreePool validates connections synchronously on acquire:
// - Tests connection before returning from pool
// - Recycles stale connections
// - Retries failed connections
```

### Monitoring

```bash
# Check pool status at runtime:
# pool.status() returns: size, available, waiting
```

### Scaling Guidelines

| Scenario | min | max | Reasoning |
|----------|-----|-----|-----------|
| Single user | 1 | 5 | Minimal resources |
| Small team | 2 | 10 | 2-3 concurrent |
| Standard | 5 | 20 | Current default |
| High concurrency | 10 | 50 | 10+ concurrent users |
| Large deployment | 20 | 100 | Sustained load |

---

## Buffer Management

### Message Buffer Sizing

**Current: 4KB** (TCP/stdio)
```rust
const BUFFER_CAPACITY: usize = 4096;
```

**Analysis**:
- JSON-RPC request: 200-500 bytes typical
- Tool arguments: 100-1000 bytes typical
- Response: 500-5000 bytes typical
- 4KB covers 99%+ of requests

**Burst handling**:
```rust
// If response > 4KB, Vec grows automatically:
let mut response_buf = Vec::with_capacity(4096);
serde_json::to_writer(&mut response_buf, &response)?;
// Vec grows if needed, never panics
```

### HTTP/2 Response Streaming

**Current approach** (src/http.rs):
```rust
// Axum automatically streams large responses
let response = Json(large_result);
// Framework handles chunking
```

**Benefits**:
- No fixed buffer needed
- Streaming prevents OOM
- Client receives data progressively

---

## Query Optimization

### Tool Query Patterns

#### 1. Metadata Queries (Fast)
```sql
-- Uses system catalogs, no row scans
SELECT * FROM information_schema.tables
SELECT * FROM pg_stat_user_tables
```

**P95**: < 1ms

#### 2. Aggregate Queries (Medium)
```sql
-- Uses indexes, aggregation
SELECT COUNT(*) FROM users
SELECT sum(amount) FROM transactions
```

**P95**: < 3ms

#### 3. Complex Queries (Slower)
```sql
-- Full query optimization needed
SELECT u.id, COUNT(o.id) FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.id HAVING COUNT(o.id) > 5
```

**P95**: < 6ms

### Query Optimization Strategies

**1. Use EXPLAIN for analysis**:
```bash
curl -X POST http://127.0.0.1:3001/rpc \
  -d '{
    "method": "tools/call",
    "params": {
      "name": "explain_query",
      "arguments": {
        "sql": "SELECT ...",
        "format": "json",
        "analyze": true
      }
    }
  }'
```

**2. Check for missing indexes**:
```sql
SELECT * FROM pg_stat_user_indexes
WHERE idx_scan = 0  -- Never used
```

**3. Monitor sequential scans**:
```sql
SELECT schemaname, tablename, seq_scan, idx_scan
FROM pg_stat_user_tables
ORDER BY seq_scan DESC
```

---

## Benchmarking Guide

### 1. Latency Measurement

**Tool**: `cargo run --release --bin measure_latency`

**What it measures**:
- 50 iterations per tool
- P50, P95, P99 percentiles
- Concurrent load (20 clients × 10 requests)
- Throughput (req/sec)

**Acceptance criteria**:
```
All tools: P95 < 10ms ✓
Concurrent: > 17K req/sec ✓
No variance: < 5% ✓
```

### 2. Throughput Testing

**Simple load test**:
```bash
#!/bin/bash
for i in {1..1000}; do
  curl -s -X POST http://127.0.0.1:3001/rpc \
    -d '{"method":"tools/call","params":{"name":"execute_query","arguments":{"sql":"SELECT 1"}}}' \
    > /dev/null
done
# Time this and calculate: requests / seconds
```

**Concurrent load test**:
```bash
# Use measure_latency --concurrent
cargo run --release --bin measure_latency
```

### 3. Memory Profiling

**Tool**: `valgrind` (Linux only)

```bash
valgrind --tool=massif ./target/release/mcp-postgres
ms_print massif.out.* > memory_profile.txt
```

**Check for**:
- Memory leaks (should be none)
- Peak memory (should be < 100MB)
- Per-request allocation (should be < 1KB)

### 4. CPU Profiling

**Tool**: `perf` (Linux)

```bash
perf record -g ./target/release/mcp-postgres
perf report
```

**Look for**:
- Hottest functions
- Memory allocation % time
- Lock contention
- System calls frequency

---

## Profiling Tools

### 1. Criterion Benchmarking

**Location**: `benches/`

**Run**:
```bash
cargo bench
```

**Example benchmark**:
```rust
#[bench]
fn bench_connection_pool(b: &mut Bencher) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    b.to_async(&rt).iter(|| async {
        // Measure pool acquisition
        pool.acquire().await
    });
}
```

### 2. Flame Graphs

**Tool**: `flamegraph` crate

```bash
# Install: cargo install flamegraph
cargo flamegraph --bin mcp-postgres -- --http-port 3001
# Output: flamegraph.svg
```

**Interpret**:
- Width = time spent
- Height = call depth
- Color = function
- Wide boxes = optimization targets

### 3. Load Testing Tools

**Apache Bench**:
```bash
ab -n 1000 -c 20 http://127.0.0.1:3001/health
```

**wrk**:
```bash
wrk -t4 -c100 -d30s http://127.0.0.1:3001/health
```

**Custom load test**:
```bash
cargo run --release --bin generate_load
```

---

## Optimization Checklist

### Before Any Optimization

- [ ] Establish baseline: `cargo run --release --bin measure_latency`
- [ ] Document current P95/P99 latencies
- [ ] Record throughput (req/sec)
- [ ] Identify regression threshold (5% drop = rollback)

### During Optimization

- [ ] Make single change only
- [ ] Rebuild with `--release` flag
- [ ] Run latency test 3 times, average results
- [ ] Check all tools, not just one
- [ ] Verify no new regressions in other areas
- [ ] Record before/after metrics

### After Optimization

- [ ] P95 still < 10ms? ✓
- [ ] Throughput maintained? ✓
- [ ] Memory usage acceptable? ✓
- [ ] Works on all platforms? ✓
- [ ] Documented why change helps? ✓
- [ ] Added to SKILLS.md? ✓

### If Regression Found

1. **ROLLBACK immediately**
   ```bash
   git revert <commit>
   cargo build --release
   ```

2. **Re-baseline**
   ```bash
   cargo run --release --bin measure_latency
   ```

3. **Investigate root cause**
   - Check change isolation
   - Run profilers
   - Test in isolation

4. **Document findings**
   - What caused regression
   - Why it was reverted
   - What to try instead

---

## Example: Optimization Workflow

### Scenario: Add new feature, concerned about latency

**Step 1: Baseline**
```bash
cargo build --release
./target/release/measure_latency > baseline.txt
# Record P95 for each tool
```

**Step 2: Implement feature**
```bash
# ... implement code ...
cargo build --release 2>&1 | grep error
```

**Step 3: Test latency**
```bash
./target/release/measure_latency > after.txt
# Compare: diff baseline.txt after.txt
```

**Step 4: Analyze results**
```
Tool: execute_query
  Before P95: 2.19ms
  After P95:  2.24ms
  Change: +0.23% (acceptable, < 5%)
```

**Step 5: Finalize**
```bash
git add .
git commit -m "Add feature: with latency verification"
```

---

## Performance Decision Tree

```
Performance Issue Detected?
│
├─ Is P95 > 10ms on any tool?
│  ├─ YES → Optimize (run profiler)
│  └─ NO → Check throughput
│
├─ Is throughput < 17K req/sec?
│  ├─ YES → Pool sizing or buffer issue
│  └─ NO → Memory check
│
├─ Is memory > 100MB?
│  ├─ YES → Connection leak or buffer growth
│  └─ NO → Performance is acceptable
│
└─ All green? → Ship it! ✅
```

---

## References

- **crossbeam Docs**: https://github.com/crossbeam-rs/crossbeam
- **Tokio Performance**: https://tokio.rs/tokio/topics/performance
- **Mimalloc Tuning**: https://github.com/microsoft/mimalloc
- **PostgreSQL EXPLAIN**: https://www.postgresql.org/docs/current/using-explain.html
- **Rust Performance Book**: https://nnethercote.github.io/perf-book/

---

**Keep performance measured, not assumed. Every optimization must be verified with data.**
