# End-to-End Latency Measurement

Measure HTTP server latency for all MCP tools with comprehensive statistics.

## Quick Start

### 1. Start the Server (Terminal 1)
```bash
DATABASE_URL="postgres://postgres:postgres@localhost:5432/postgres" \
  cargo run --release -- --http-port 3001
```

### 2. Run Latency Measurement (Terminal 2)
```bash
cargo run --release --bin measure_latency
```

## What Gets Measured

### Per-Tool Testing (50 iterations each)
- ✅ tools/list - List all available tools
- ✅ list_tables - List database tables
- ✅ describe_table - Describe table structure
- ✅ execute_query (simple) - SELECT 1 query
- ✅ execute_query (moderate) - Query with LIMIT
- ✅ execute_query (complex) - Complex aggregation query
- ✅ get_cache_hit_ratio - Cache metrics
- ✅ analyze_db_health - Full health check
- ✅ get_setting - Configuration setting
- ✅ show_current_user - Current user info

### Concurrent Load Test
- **20 concurrent clients**
- **10 requests per client**
- **Total: 200 concurrent requests**
- Measures aggregate throughput and latency under load

## Metrics Reported

### Per Tool
```
Min:    X.XXms      - Minimum latency
Max:    X.XXms      - Maximum latency
Avg:    X.XXms      - Average latency
P50:    X.XXms      - Median (50th percentile)
P95:    X.XXms      - 95th percentile
P99:    X.XXms      - 99th percentile
```

### Concurrent Load
```
Total requests:     200
Total time:         X.XXs
Requests/sec:       XXX.X
Min latency:        X.XXms
Max latency:        X.XXms
Avg latency:        X.XXms
P50 latency:        X.XXms
P95 latency:        X.XXms
P99 latency:        X.XXms
```

### Summary Table
```
Tool                      Avg (ms)   P95 (ms)   P99 (ms)   Max (ms)
────────────────────────────────────────────────────────────────
execute_query                5.23      8.15      12.43      15.67
list_tables                  4.81      7.23       9.12      11.34
...
```

## Performance Classification

Tools are classified as:
- ⭐ **Excellent** (P95 < 10ms) - Sub-10ms latency
- ✅ **Good** (P95 < 20ms) - Good performance
- ⚠️ **Acceptable** (P95 < 50ms) - Acceptable performance
- ❌ **Slow** (P95 ≥ 50ms) - Needs optimization

## Output Example

```
🔍 MCP PostgreSQL - HTTP Server Latency Measurement
═══════════════════════════════════════════════════

Testing connection to http://127.0.0.1:3001...
✅ Server is running

📊 Latency Test Cases:

Testing: execute_query - Simple SELECT
  Min:    3.12ms
  Max:    15.23ms
  Avg:    5.45ms
  P50:    4.89ms
  P95:    8.12ms
  P99:    12.34ms

... (more tools)

⚡ Concurrent Load Test (20 clients × 10 requests)
─────────────────────────────────────────────────
Total requests:        200
Total time:            3.42s
Requests/sec:          58.5

Min latency:           2.34ms
Max latency:           145.67ms
Avg latency:           18.45ms
P50 latency:           15.23ms
P95 latency:           42.56ms
P99 latency:           89.34ms

📈 Summary Table
────────────────────────────────────────────────────────────
Tool                      Avg (ms)   P95 (ms)   P99 (ms)   Max (ms)
────────────────────────────────────────────────────────────
tools/list                   4.23       6.12       9.34      12.45
list_tables                  4.81       7.23       9.12      11.34
describe_table               5.12       8.45      11.23      14.56
execute_query               5.45       8.12      12.34      15.23
...

🎯 Performance Classification:
  ⭐ Excellent  (P95 < 10ms):   3 tools
  ✅ Good       (P95 < 20ms):   5 tools
  ⚠️  Acceptable (P95 < 50ms):   2 tools
  ❌ Slow       (P95 ≥ 50ms):   0 tools

✨ Measurement complete!
```

## Interpreting Results

### Excellent Performance (P95 < 10ms)
- Fast request handling
- Minimal overhead
- Good for real-time applications

### Good Performance (P95 < 20ms)
- Acceptable for most use cases
- Some variability under load
- Suitable for interactive applications

### Acceptable Performance (P95 < 50ms)
- Slower queries or heavy operations
- May benefit from optimization
- Acceptable for background operations

### Slow Performance (P95 ≥ 50ms)
- Needs investigation
- Check for:
  - Complex SQL queries
  - Database connection latency
  - Network overhead
  - Server resource constraints

## Optimization Tips

### If Latency is High:
1. **Check database**: Is the query slow?
   ```bash
   cargo run --release --bin measure_latency
   # If execute_query is slow, profile the SQL
   ```

2. **Check connection pool**: Is pool contention occurring?
   - Monitor pool.rs statistics
   - Check min/max pool sizes

3. **Check network**: Is HTTP overhead significant?
   - Compare TCP vs HTTP latency
   - Check for TLS overhead

4. **Check CPU/Memory**: Are resources constrained?
   ```bash
   top -l 1 | grep mcp-postgres
   ```

## Advanced Usage

### Modify Test Cases
Edit `bin/measure_latency.rs` to customize:
- `test_cases` vector: Change which tools are tested
- `iterations`: Adjust number of iterations (default: 50)
- `concurrent_client_count`: Change concurrency level
- `requests_per_client`: Change load per client

### Re-compile After Changes
```bash
cargo build --release --bin measure_latency
```

## Troubleshooting

### Server Not Running
```
❌ Server not running on 127.0.0.1:3001

Start the server with:
  cargo run --release -- --http-port 3001
```

### Connection Refused
- Verify HTTP port is 3001
- Check firewall settings
- Ensure server is listening on 0.0.0.0:3001

### High Variance in Results
- Close other applications
- Increase iteration count for better statistics
- Check system load with `top` or `Activity Monitor`

## Related Tools

- `bin/generate_load.rs` - Sustained load generation
- `bin/benchmark.rs` - Criterion benchmarking
- `bin/load_test_data.rs` - Test data generation
- `bin/batch_load.rs` - Batch operation testing

---

**Last Updated**: 2026-06-13  
**Purpose**: Measure end-to-end HTTP/2 latency for all 25 PostgreSQL tools  
**Requirements**: Running PostgreSQL server accessible via connection string
