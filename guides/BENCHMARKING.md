# Benchmarking Guide

Comprehensive benchmarking methodology for mcp-postgres: TCP throughput, HTTP latency, and server process monitoring.

## Overview

The benchmark suite measures four dimensions:

| Dimension | Tool | What It Measures |
|-----------|------|-----------------|
| TCP throughput | `benchmark` binary | Raw request throughput under varying concurrency |
| HTTP latency | `measure_latency` binary | Per-tool P50/P95/P99 latency and concurrent load |
| Process health | `ps`/`lsof` snapshots | CPU, RSS, threads, file descriptors (idle + under load) |
| Memory stability | `ps` snapshots | RSS growth after sustained load |
| Error audit | `grep` of server log | Unexpected ERROR/WARN entries |

## Quick Start

### 1. Start the Server

```bash
DATABASE_URL="postgres://postgres:postgres@localhost:5432/postgres" \
  cargo run --release -- --http-port 3001
```

### 2. Run TCP Throughput Benchmark

```bash
cargo run --release --bin benchmark -- 10 20
#                    duration (s) ────┘  │
#                    concurrency ────────┘
```

### 3. Run HTTP Latency Profile

```bash
cargo run --release --bin measure_latency
```

### 4. Run Full Suite (Automated)

```bash
bash ~/ai/bench-mcp-postgres.sh
```

The script runs all 8 tests sequentially, collects results, and audits the server log.

---

## TCP Throughput (`bin/benchmark.rs`)

### Usage

```bash
cargo run --release --bin benchmark -- <duration_sec> <concurrency>
```

### What It Tests

Opens `<concurrency>` persistent TCP connections to port 3000 and sends JSON-RPC `ping` requests as fast as possible for `<duration_sec>` seconds. Measures raw protocol throughput without HTTP overhead.

### Metrics

```
=== Results ===
Concurrency: 20
Duration: 10.0s
Total Requests: 74200
Requests/sec: 7415
Avg Latency: 134.9µs
```

### Typical Results (Apple Silicon, localhost)

| Concurrency | Requests/sec | Avg Latency |
|-------------|-------------|-------------|
| 20 | ~7,400 | ~135 µs |
| 100 | ~37,000 | ~27 µs |

### Interpretation

- **High concurrency (100)**: Saturates CPU. Requests/sec should increase but per-request latency drops as more connections compete for cores.
- **Sub-100µs avg latency** at high concurrency indicates lock-free pool is delivering low contention.
- If latency spikes under load, investigate connection pool or SQL query bottlenecks.

---

## HTTP Latency (`bin/measure_latency.rs`)

### Usage

```bash
cargo run --release --bin measure_latency
```

### What It Tests

1. **Per-tool latency** (50 iterations each):
   - `tools/list`, `list_tables`, `describe_table`, `execute_query` (simple/moderate/complex)
   - `get_cache_hit_ratio`, `analyze_db_health`, `get_setting`, `show_current_user`

2. **Concurrent load test** (20 clients × 10 requests = 200 total)

### Metrics

```
Testing: tools/list - List all tools
  Min:    0.40ms
  Max:    0.68ms
  Avg:    0.48ms
  P50:    0.47ms
  P95:    0.57ms
  P99:    0.68ms

Concurrent Load:
  Total requests:        200
  Requests/sec:          6239
  Avg latency:           2.50ms
```

### Performance Classification

| Rating | Criterion | Meaning |
|--------|-----------|---------|
| ⭐ Excellent | P95 < 10ms | Fast request handling, suitable for real-time |
| ✅ Good | P95 < 20ms | Acceptable for interactive use |
| ⚠️ Acceptable | P95 < 50ms | Slower, may benefit from optimization |
| ❌ Slow | P95 ≥ 50ms | Needs investigation |

---

## Process Monitoring

### Snapshot Approach (Used in `bench-mcp.sh`)

The benchmark takes three point-in-time snapshots to avoid perturbing measurements:

| Phase | Timing | What's Captured |
|-------|--------|----------------|
| **Test 4: Idle Baseline** | Before any load | CPU, RSS, threads, FDs, network connections |
| **Test 5: Under Load** | 2s into a TCP benchmark | CPU, RSS, threads — shows peak resource usage |
| **Test 6: Post-Load** | 3s after load stops | RSS — detects leaks or unreleased memory |

Example output:

```
PID:     24163
  PID  %CPU    RSS %MEM COMMAND
24163 352.1  24560  0.1 mcp-postgres

Threads: 10
RSS: 23 MB
File descriptors: 32
Network connections:
  IPv4/TCP: 123
```

### Continuous Monitoring (Background Daemon)

For deeper analysis, run a background monitor alongside a benchmark:

```bash
# Start monitor in background (sampling every 1s)
(while kill -0 $MCP_PID 2>/dev/null; do
  ps -p $MCP_PID -o pid,%cpu,rss,%mem,th 2>/dev/null
  sleep 1
done) > /tmp/mcp-monitor.csv &
MONITOR_PID=$!

# Run benchmark
cargo run --release --bin benchmark -- 30 100

# Stop monitor
kill $MONITOR_PID 2>/dev/null

# Analyze
awk 'NR>1 && /^[0-9]/ { cpu+=$2; rss+=$3; count++ }
     END { print "Avg CPU:", cpu/count, "%  Avg RSS:", rss/count/1024, "MB" }' /tmp/mcp-monitor.csv
```

Or with `watch` for live observation:

```bash
watch -n 0.5 'ps -p $MCP_PID -o pid,%cpu,rss,%mem,th,command 2>/dev/null || echo "(done)"'
```

### What to Watch For

| Metric | Healthy Range | Concern |
|--------|--------------|---------|
| CPU | < 400% (4 cores) | Saturating all cores |
| RSS idle | 10–15 MB | Above 30 MB — investigate |
| RSS under load | < 30 MB | Growing without bound — memory leak |
| Threads | 10–12 | Creeping up — thread leak |
| File descriptors | < 50 | Growing — connection leak |
| Network connections | ~125 per 100 conn | Not returning to baseline — conn leak |

---

## Memory Stability Test

The post-load idle check (Test 6) is the simplest memory leak detector:

```bash
# Before load
RSS_BEFORE=$(ps -p $MCP_PID -o rss= | tr -d ' ')

# Run load
cargo run --release --bin benchmark -- 30 100

# After load (settle 3s)
sleep 3
RSS_AFTER=$(ps -p $MCP_PID -o rss= | tr -d ' ')

# Compare
echo "Before: $((RSS_BEFORE / 1024)) MB  After: $((RSS_AFTER / 1024)) MB"
```

- **Pass**: RSS grows < 20% or returns to near-baseline after settle
- **Fail**: RSS grows monotonically or never releases — likely a leak in pool or connection handling

### mimalloc Note

mimalloc's per-thread caching can show 5–10 MB of "wasted" memory (internal fragmentation ~89% utilization). This is normal and not a leak. The benchmark threshold (128 MB) accounts for this.

---

## Error Log Audit

The server log must be checked after any benchmark run:

```bash
# Count errors
grep -ci "ERROR" /tmp/mcp-bench/server.log

# Categorize
grep "ERROR" /tmp/mcp-bench/server.log | sed 's/.*ERROR//' | sort | uniq -c | sort -rn

# Expected: get_setting errors from measure_latency testing without params
# Unexpected: connection failures, pool exhaustion, protocol errors
```

Expected errors:
- `Tool 'get_setting' error: InvalidParams("Missing 'setting' parameter")` — from `measure_latency` testing `get_setting` without required params (test noise)

Any other ERROR or WARN entry warrants investigation.

---

## HTTP Endpoint Validation

Verifies the server is alive and serving HTTP:

```bash
python3 -c "
import urllib.request, json
req = urllib.request.Request('http://127.0.0.1:3001/rpc',
    data=json.dumps({'jsonrpc':'2.0','method':'tools/list','id':1}).encode(),
    headers={'Content-Type':'application/json'})
resp = urllib.request.urlopen(req, timeout=3)
data = json.loads(resp.read())
tools = data['result'].get('tools', [])
print(f'Tools available: {len(tools)}')
"
```

Expected output: `Tools available: 135`

If this fails, the HTTP endpoint is broken and no subsequent HTTP test results are valid.

---

## Test Dependencies & Ordering

```
┌─────────────────────┐
│ 1. Validate env     │  ← python3, ps, lsof, tail, cut, tr, cargo
├─────────────────────┤
│ 2. Check database   │  ← PostgreSQL must be reachable
├─────────────────────┤
│ 3. Free ports       │  ← Kill stale processes on 3000, 3001
├─────────────────────┤
│ 4. Build binaries   │  ← cargo build --release
├─────────────────────┤
│ 5. Start server     │  ← mcp-postgres --database-url $DB_URL
├─────────────────────┤
│ 6. Sanity check     │  ← TCP ping (retries 3x)
├─────────────────────┤
│ 7. TCP throughput   │  ← benchmark 10 20
├─────────────────────┤
│ 8. TCP saturation   │  ← benchmark 10 100
├─────────────────────┤
│ 9. HTTP latency     │  ← measure_latency
├─────────────────────┤
│ 10. Idle snapshot   │  ← ps, lsof
├─────────────────────┤
│ 11. Load snapshot   │  ← benchmark 5 20 + ps
├─────────────────────┤
│ 12. Memory check    │  ← settle 3s + ps
├─────────────────────┤
│ 13. Error audit     │  ← grep server log
├─────────────────────┤
│ 14. HTTP health     │  ← tools/list via HTTP
└─────────────────────┘
```

---

## Troubleshooting

### `timeout: command not found` (macOS)

macOS lacks GNU `timeout`. The `bench-mcp.sh` script auto-detects and falls back to:
1. `gtimeout` (from `brew install coreutils`)
2. Bash-native background-process killer

```bash
brew install coreutils  # provides gtimeout
```

### `local: can only be used in a function`

Occurs if `local` is used outside a function definition. All `local` declarations must be inside bash functions. Use plain variable assignment at script top level.

### `grep -c` returns "0" twice

When `grep -c` finds zero matches, it prints "0" to stdout and exits with code 1. If piped as `|| echo "0"`, both the stdout "0" and the fallback "0" are captured. Fix:

```bash
# Wrong — double "0"
ERR_COUNT=$(grep -ci "ERROR" log || echo "0")

# Correct
ERR_COUNT=$(grep -ci "ERROR" log) || ERR_COUNT=0
```

### Server fails to start

Check the log:
```bash
cat /tmp/mcp-bench/server.log
```

Common causes:
- Port already in use
- Invalid `--database-url`
- PostgreSQL not running

### Stale results directory

Each run creates a timestamped directory (`~/ai/bench-mcp-results-YYYYMMDD-HHMMSS/`). Clean up with:

```bash
rm -rf ~/ai/bench-mcp-results-*
```

---

## Reference

| File | Purpose |
|------|---------|
| `bin/benchmark.rs` | TCP throughput binary |
| `bin/measure_latency.rs` | HTTP latency profile binary |
| `~/ai/bench-mcp-postgres.sh` | Full automated benchmark suite |
| `/tmp/mcp-bench/server.log` | Server stdout/stderr from benchmark runs |
| `~/ai/bench-mcp-results-*/summary.txt` | Saved benchmark output |

## Related Guides

- [LATENCY_MEASUREMENT.md](./LATENCY_MEASUREMENT.md) — HTTP latency measurement (per-tool profiling)
- [CODE_OPTIMIZATION.md](./CODE_OPTIMIZATION.md) — Verified optimizations and profiling
- [OPTIMIZATIONS.md](./OPTIMIZATIONS.md) — Tuning parameters and bottleneck analysis

---

**Last Updated**: 2026-06-15  
**Requirements**: macOS or Linux, PostgreSQL, Rust toolchain, Python 3