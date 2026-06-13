# SKILLS: Automated SDLC & Agent Workflow Guide

**Type**: SDLC Automation & Constraint Checking | **For**: Development Process

---

## 1. CONSTRAINTS & ARCHITECTURAL DECISIONS

### 1.1 Protocol Architecture Constraints

**TCP Server (Port 3000)**:
- Direct JSON-RPC 2.0 protocol over TCP socket
- Stateful connection per client
- Supports parameterized queries via tokio_postgres Client
- Latency baseline: < 10ms per request (STRICT: > 10ms is not acceptable, > 20ms is deal breaker)
- No connection pooling needed (one client per connection)

**HTTP/2 Server (Port 3001)**:
- JSON-RPC 2.0 POST requests to `/rpc` endpoint
- Stateless (each request is independent)
- Connection pooling via `deadpool::postgres` (Pool<Client>)
- Each request randomly selects connection from pool
- Latency baseline: < 10ms per request (STRICT: > 10ms is not acceptable, > 20ms is deal breaker; pool init overhead acceptable only for first request)
- Health endpoint: GET `/health` returns `{"status": "healthy"}`

**Critical**: HTTP cannot maintain transaction state across requests. No transaction tools (begin_transaction, commit_transaction, rollback_transaction, kill_connection) are implemented.

**JSON-RPC Protocol Requirement**:
```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "<tool_name>",
    "arguments": { "<param>": value }
  },
  "id": 1
}
```

### 1.2 Tool Definitions (46 Total)

**CRITICAL**: Parameter names are case-sensitive and must match exactly. Verify against `tools/list` response.

**Query Execution Tools**:
- `execute_query`: params = `{"sql": "SELECT..."}` (NOT "query")
- `execute_insert`: params = `{"sql": "INSERT..."}`
- `execute_update`: params = `{"sql": "UPDATE..."}`
- `execute_delete`: params = `{"sql": "DELETE..."}`
- `explain_query`: params = `{"sql": "SELECT...", "analyze": bool, "buffers": bool, "format": "json|text|yaml"}`
- `async_execute_insert`: params = `{"sql": "INSERT..."}`
- `async_execute_update`: params = `{"sql": "UPDATE..."}`
- `async_execute_delete`: params = `{"sql": "DELETE..."}`

**Connection Tools**:
- `show_current_user`: params = `{}`
- `list_connections`: params = `{}`

**Schema Inspection Tools**:
- `list_tables`: params = `{}`
- `list_schemas`: params = `{}`
- `list_columns`: params = `{"table": "table_name"}` (REQUIRED)
- `list_indexes`: params = `{}`
- `list_triggers`: params = `{"table": "table_name"}` (REQUIRED)
- `list_views`: params = `{}`
- `list_sequences`: params = `{}`
- `describe_table`: params = `{"table": "table_name"}` (REQUIRED)

**DDL Tools (Create/Alter/Drop)**:
- `create_table`: params = `{"table": "name", "columns": ["id SERIAL PRIMARY KEY", "name VARCHAR(255)"]}`
- `drop_table`: params = `{"table": "name"}`
- `create_view`: params = `{"view_name": "name", "query": "SELECT..."}`
- `drop_view`: params = `{"view_name": "name"}`
- `alter_view`: params = `{"view_name": "name", "rename_to": "new_name"}`
- `create_schema`: params = `{"schema_name": "name"}`
- `drop_schema`: params = `{"schema_name": "name"}`
- `create_index`: params = `{"index_name": "name", "table": "table_name", "columns": ["col1", "col2"]}`
- `drop_index`: params = `{"index_name": "name"}`
- `alter_index`: params = `{"index_name": "name", "rename_to": "new_name"}`
- `create_sequence`: params = `{"sequence_name": "name", "start": 1, "increment": 1}`
- `drop_sequence`: params = `{"sequence_name": "name"}`
- `create_partition`: params = `{"table": "name", "partition_name": "name_1", "partition_type": "RANGE", "column": "id", "values": "FROM (1) TO (100)"}`
- `delete_table_partition`: params = `{"partition_name": "name_1"}`
- `list_partitions`: params = `{"table": "name"}`
- `backup_table`: params = `{"table": "name"}` (creates backup_<name> table)

**Batch Operation Tools**:
- `async_batch_insert`: params = `{"table": "name", "columns": ["col1", "col2"], "rows": [[val1, val2], ...], "returning": "id"}`
- `async_batch_update`: params = `{"table": "name", "updates": {"col1": val1}, "where_clauses": ["col=val"]}`
- `async_batch_delete`: params = `{"table": "name", "where_clauses": ["col=val"], "returning": "id"}`
- `async_batch_insert_copy`: params = `{"table": "name", "columns": [...], "rows": [...], "batch_size": 1000}`

**Database Utility Tools**:
- `analyze_table`: params = `{"table": "name"}`
- `vacuum_table`: params = `{"table": "name"}`
- `get_table_size`: params = `{"table": "name"}`
- `get_database_size`: params = `{}`

### 1.3 Code Quality Standards

**Compilation**:
- `cargo build --release` must succeed with ZERO errors
- `cargo clippy --lib` must return ZERO warnings in library code
- No `unwrap()` in production code (use `?` operator or proper error handling)
- All error types must implement `std::error::Error`

**Testing Requirements**:
- `cargo test --lib` must pass with 100% success rate
- All tests use REAL PostgreSQL database (never mock)
- No `#[ignore]` attributes on any test
- Test database accessible at `DATABASE_URL` environment variable
- Default: `postgres://piyush:@localhost:5432/postgres`

### 1.4 Database Connection Requirements

**Prerequisites** (must be verified BEFORE any test):
```bash
# 1. PostgreSQL running
psql -U piyush -d postgres -c "SELECT version();" || exit 1

# 2. Database accessible
psql postgres://piyush:@localhost:5432/postgres -c "SELECT 1;" || exit 1

# 3. Test data loaded
psql -U piyush -d postgres -c "SELECT COUNT(*) FROM users;" || exit 1
```

**If any check fails**: STOP immediately, fix database connection, then retry.

---

## 2. CONTROL FLOW DECISION TREES

### 2.1 Pre-Task Validation

**BEFORE writing any test code:**

```
START
├─ PARSE: Extract tool name from task
├─ VERIFY: Query tools/list for tool existence
│  ├─ DECISION: Tool in response?
│  │  ├─ NO  → ERROR: Tool does not exist
│  │  │        ACTION: Grep src/actions/ to find correct tool name
│  │  └─ YES → Continue to parameter validation
│  │
├─ EXTRACT: Get tool's "inputSchema" from tools/list response
├─ VALIDATE: For each parameter in test
│  ├─ DECISION: Parameter name in inputSchema?
│  │  ├─ NO  → ERROR: Invalid parameter name
│  │  │        ACTION: Use exact name from inputSchema
│  │  │        EXAMPLE: "query" is WRONG, "sql" is CORRECT
│  │  └─ YES → Check type
│  │
│  ├─ DECISION: Parameter marked "required"?
│  │  ├─ YES → MUST provide value
│  │  │        DECISION: Value is correct type?
│  │  │        ├─ NO  → ERROR: Type mismatch
│  │  │        │        ACTION: Convert to correct type
│  │  │        │        EXAMPLE: list_triggers needs table:string not table:number
│  │  │        └─ YES → OK
│  │  └─ NO  → Optional, can omit
│  │
├─ DATABASE STATE: Check if referenced objects exist
│  ├─ DECISION: list_triggers test?
│  │  ├─ YES → VERIFY: Table exists via list_tables
│  │  │        ├─ NOT FOUND → Create test table first
│  │  │        └─ FOUND → OK
│  │  └─ NO → Skip
│  │
├─ PROTOCOL: Prepare both TCP and HTTP variants
│  ├─ TCP:  Connect to 127.0.0.1:3000 and send JSON-RPC
│  ├─ HTTP: POST to http://127.0.0.1:3001/rpc with Content-Type: application/json
│  └─ BOTH must succeed for parity
│
└─ PROCEED to test execution
```

### 2.2 Test Execution Validation

**DURING test execution, BEFORE asserting results:**

```
REQUEST SENT
├─ WAIT for response (timeout: 5s per request)
├─ PARSE JSON response
│  ├─ DECISION: Valid JSON?
│  │  ├─ NO  → FAIL: Response not JSON
│  │  │        ACTION: Check server logs
│  │  └─ YES → Continue
│  │
├─ VALIDATE: Response has required fields
│  ├─ DECISION: Has "jsonrpc"?
│  │  ├─ NO  → FAIL: Missing jsonrpc field
│  │  └─ YES → Continue
│  │
│  ├─ DECISION: Has "id" matching request?
│  │  ├─ NO  → FAIL: ID mismatch (protocol violation)
│  │  └─ YES → Continue
│  │
│  ├─ DECISION: Has "result" or "error"?
│  │  ├─ NEITHER → FAIL: Invalid response (no result or error)
│  │  ├─ ERROR  → Check error.code and error.message
│  │  │           (normal for invalid inputs)
│  │  └─ RESULT → Check result structure
│  │
├─ ERROR HANDLING (if "error" field present)
│  ├─ DECISION: error.code = -32602 (Invalid params)?
│  │  ├─ YES → Likely wrong parameter name or missing required param
│  │  │        ACTION: Re-run pre-task validation
│  │  └─ NO → Continue
│  │
│  ├─ DECISION: error.code = -32601 (Method not found)?
│  │  ├─ YES → Tool doesn't exist
│  │  │        ACTION: Verify against tools/list
│  │  └─ NO → Continue
│  │
│  ├─ DECISION: error.code = -32700 (Parse error)?
│  │  ├─ YES → JSON not valid
│  │  │        ACTION: Check JSON syntax
│  │  └─ NO → Other error
│  │
├─ RESULT VALIDATION (if "result" field present)
│  ├─ DECISION: Result is null?
│  │  ├─ YES → QUESTION: Is null expected?
│  │  │        ├─ NO  → May indicate error
│  │  │        └─ YES → Continue
│  │  └─ NO → Inspect result structure
│  │
└─ ASSERTION: Verify result matches expected
```

### 2.3 Post-Activity Verification

**AFTER every test execution, BEFORE considering test complete:**

```
TEST COMPLETED
├─ SUCCESS RATE CHECK
│  ├─ MEASUREMENT: Count passed / total tests
│  ├─ DECISION: Rate >= 90%?
│  │  ├─ NO  → FAIL: Below minimum threshold
│  │  │        ACTION: Stop, investigate each failure
│  │  │        ├─ Wrong parameters? Fix parameter names
│  │  │        ├─ Tool doesn't exist? Verify with tools/list
│  │  │        ├─ Missing data? Load test data
│  │  │        └─ Retry after fixes
│  │  └─ YES → Continue
│  │
├─ LATENCY CHECK (STRICT REQUIREMENTS)
│  ├─ TCP LATENCY
│  │  ├─ MEASUREMENT: Average of all TCP requests
│  │  ├─ DECISION: < 10ms?
│  │  │  ├─ < 5ms   → Excellent
│  │  │  ├─ 5-10ms  → Good
│  │  │  ├─ 10-20ms → Not acceptable (> 10ms is not good)
│  │  │  └─ > 20ms  → DEAL BREAKER, STOP AND INVESTIGATE
│  │  └─ If > 10ms: INVESTIGATE immediately
│  │
│  ├─ HTTP LATENCY
│  │  ├─ MEASUREMENT: Average of all HTTP requests
│  │  ├─ DECISION: < 10ms?
│  │  │  ├─ < 5ms   → Excellent
│  │  │  ├─ 5-10ms  → Good
│  │  │  ├─ 10-20ms → Not acceptable (> 10ms is not good)
│  │  │  └─ > 20ms  → DEAL BREAKER, STOP AND INVESTIGATE
│  │  └─ Exception: First request may have pool init overhead, acceptable if subsequent requests < 10ms
│  │  └─ If avg > 10ms: INVESTIGATE immediately
│  │
│  ├─ DIFFERENCE CHECK
│  │  ├─ MEASUREMENT: HTTP avg / TCP avg
│  │  ├─ DECISION: Ratio < 15x?
│  │  │  ├─ YES → Normal (pool overhead)
│  │  │  └─ NO → Protocol issue
│  │  └─ Continue
│  │
├─ PROTOCOL PARITY CHECK
│  ├─ DECISION: TCP and HTTP have same success rate?
│  │  ├─ NO  → Protocol-specific issue
│  │  │        ACTION: Check server logs, verify both protocols working
│  │  │        └─ Retry after server restart
│  │  └─ YES → Continue
│  │
│  ├─ DECISION: Same result format on both?
│  │  ├─ NO  → Serialization issue
│  │  │        ACTION: Inspect response diff
│  │  └─ YES → Continue
│  │
├─ TOOL AVAILABILITY CHECK
│  ├─ ACTION: Query tools/list endpoint
│  ├─ DECISION: All tested tools present?
│  │  ├─ NO  → Missing tool error
│  │  │        ACTION: Add tool implementation or skip test
│  │  └─ YES → Continue
│  │
│  ├─ DECISION: No "Method not found" errors in logs?
│  │  ├─ NO  → Tool not implemented
│  │  │        ACTION: Check src/actions/ for implementation
│  │  └─ YES → OK
│  │
└─ TEST PASSED: All checks complete
```

---

## 3. UNIT TEST WORKFLOW

**TRIGGER**: Before every commit, on changes to `src/` or `Cargo.toml`

**Prerequisites**:
- Cargo installed and in PATH
- Rust toolchain >= 1.70
- No other cargo process running

**Procedure**:

```bash
# Step 1: Clean previous builds
cargo clean --release

# Step 2: Build in release mode
cargo build --release 2>&1 | tee /tmp/build.log
BUILD_EXIT=$?
if [ $BUILD_EXIT -ne 0 ]; then
  echo "FAIL: cargo build failed"
  cat /tmp/build.log
  exit 1
fi

# Step 3: Run clippy (linter)
cargo clippy --lib 2>&1 | tee /tmp/clippy.log
CLIPPY_WARNINGS=$(grep -c "warning:" /tmp/clippy.log || echo 0)
if [ $CLIPPY_WARNINGS -gt 0 ]; then
  echo "FAIL: $CLIPPY_WARNINGS clippy warnings"
  grep "warning:" /tmp/clippy.log
  exit 1
fi

# Step 4: Run unit tests
cargo test --lib -- --nocapture --test-threads=1 2>&1 | tee /tmp/tests.log
TEST_RESULT=$?

# Step 5: Verify test results
if [ $TEST_RESULT -ne 0 ]; then
  echo "FAIL: Unit tests failed"
  grep -A 5 "test result:" /tmp/tests.log
  exit 1
fi

# Step 6: Count passing tests
TESTS_PASSED=$(grep "test result: ok" /tmp/tests.log | grep -o "[0-9]* passed" | head -1)
echo "PASS: $TESTS_PASSED"
```

**Acceptance Criteria**:
- ✅ Build succeeds with zero errors
- ✅ Zero clippy warnings in library code (`--lib`)
- ✅ 100% unit test success rate (all tests pass)
- ✅ No test panics (if panic, fix root cause)
- ✅ Execution completes in < 30 seconds
- ✅ No `#[ignore]` annotations on any test

**Failure Action**: BLOCK commit, show exact test failure, require fix before retry.

---

## 4. INTEGRATION TEST WORKFLOW

**TRIGGER**: Before release, after functional code changes, every 10 commits

**Prerequisites** (VERIFY BEFORE STARTING):
```bash
# Check 1: PostgreSQL running
psql --version || { echo "FAIL: psql not found"; exit 1; }

# Check 2: Database accessible
PGPASSWORD="" psql -h localhost -U piyush -d postgres -c "SELECT version();" || { echo "FAIL: Database not accessible"; exit 1; }

# Check 3: Database accessible via connection string
psql "postgres://piyush:@localhost:5432/postgres" -c "SELECT 1;" || { echo "FAIL: Connection string failed"; exit 1; }

# Check 4: No server already running
nc -zv 127.0.0.1 3000 2>&1 && { echo "FAIL: Port 3000 already in use"; exit 1; } || echo "Port 3000 free"
nc -zv 127.0.0.1 3001 2>&1 && { echo "FAIL: Port 3001 already in use"; exit 1; } || echo "Port 3001 free"

# Check 5: Database has test data
psql "postgres://piyush:@localhost:5432/postgres" -c "SELECT COUNT(*) FROM users;" | grep -q "[0-9]" || { echo "FAIL: Test data missing, run load_test_data"; exit 1; }
```

**Procedure**:

```bash
#!/bin/bash
set -e

export DATABASE_URL="postgres://piyush:@localhost:5432/postgres"

echo "=== STEP 1: Start Server ==="
# Start in background, capture PID
cargo run --release -- --http-port 3001 > /tmp/server.log 2>&1 &
SERVER_PID=$!
echo "Server started (PID: $SERVER_PID)"

# Wait for startup
sleep 4

echo "=== STEP 2: Verify Server Health ==="
# TCP check
if ! nc -zv 127.0.0.1 3000 2>&1 | grep -q "succeeded"; then
  echo "FAIL: TCP port 3000 not responding"
  kill $SERVER_PID || true
  exit 1
fi
echo "✓ TCP port 3000 responding"

# HTTP check
HTTP_STATUS=$(curl -s http://127.0.0.1:3001/health | jq -r '.status // "error"')
if [ "$HTTP_STATUS" != "healthy" ]; then
  echo "FAIL: HTTP health check failed (status: $HTTP_STATUS)"
  cat /tmp/server.log | tail -20
  kill $SERVER_PID || true
  exit 1
fi
echo "✓ HTTP port 3001 healthy"

echo "=== STEP 3: Run Dual-Protocol Tests ==="
cargo test --test integration_dual_protocol -- --nocapture --test-threads=1 2>&1 | tee /tmp/dual_test.log
DUAL_RESULT=$?

if [ $DUAL_RESULT -ne 0 ]; then
  echo "FAIL: Dual-protocol tests failed"
  tail -30 /tmp/dual_test.log
  kill $SERVER_PID || true
  exit 1
fi

# Verify both protocols at 100%
if ! grep -q "Success: 10/10 (100.0%)" /tmp/dual_test.log; then
  echo "FAIL: Not all dual-protocol tests passed"
  grep "Success:" /tmp/dual_test.log
  kill $SERVER_PID || true
  exit 1
fi
echo "✓ Dual-protocol tests: TCP 100%, HTTP 100%"

echo "=== STEP 4: Run All-Tools Integration Tests ==="
cargo test --test integration_all_tools -- --nocapture --test-threads=1 2>&1 | tee /tmp/all_tools_test.log
TOOLS_RESULT=$?

if [ $TOOLS_RESULT -ne 0 ]; then
  echo "FAIL: All-tools tests failed"
  tail -30 /tmp/all_tools_test.log
  kill $SERVER_PID || true
  exit 1
fi
echo "✓ All-tools tests passed"

echo "=== STEP 5: Verify Test Data Tools ==="
cargo test --test integration_test_data_tools -- --nocapture --test-threads=1 2>&1 | tee /tmp/data_tools_test.log
DATA_RESULT=$?

if [ $DATA_RESULT -ne 0 ]; then
  echo "FAIL: Data tools tests failed"
  tail -30 /tmp/data_tools_test.log
  kill $SERVER_PID || true
  exit 1
fi
echo "✓ Data tools tests passed"

echo "=== STEP 6: Run Load Tests ==="
cargo test --test load_test -- --nocapture 2>&1 | tee /tmp/load_test.log
LOAD_RESULT=$?

if [ $LOAD_RESULT -ne 0 ]; then
  echo "FAIL: Load tests failed"
  tail -30 /tmp/load_test.log
  kill $SERVER_PID || true
  exit 1
fi

# Verify load test tier
if grep -q "EXCELLENT\|GOOD" /tmp/load_test.log; then
  echo "✓ Load test: EXCELLENT or GOOD tier"
else
  echo "WARN: Load test not at expected tier"
fi

echo "=== STEP 7: Cleanup ==="
kill $SERVER_PID || true
sleep 2

echo "=== ALL INTEGRATION TESTS PASSED ==="
```

**Acceptance Criteria**:
- ✅ Server starts and listens on both TCP 3000 and HTTP 3001
- ✅ HTTP `/health` endpoint returns `{"status": "healthy"}`
- ✅ Dual-protocol tests: TCP 10/10 (100%), HTTP 10/10 (100%)
- ✅ All-tools integration tests: 12/12 pass
- ✅ Data-tools tests: 17/17 pass
- ✅ Load test: GOOD or EXCELLENT tier (not ACCEPTABLE)
- ✅ No `#[ignore]` on any test
- ✅ No panics in logs
- ✅ Server shuts down cleanly

**Failure Action**: BLOCK commit, list failing tests, require investigation and fix.

---

## 4.1 Heap Allocation Tracking (Performance Monitoring)

**TRIGGER**: After load tests, before release, on performance-critical changes

**PURPOSE**: Ensure memory usage is stable, not growing unbounded. Detect memory leaks and allocation inefficiencies.

**Procedure (Using Rust heaptrack)**:

```bash
# Step 1: Install heaptrack (macOS)
brew install heaptrack

# Step 2: Build server with debug symbols
cargo build --release

# Step 3: Run server under heaptrack
heaptrack ./target/release/mcp-postgres &
SERVER_PID=$!
sleep 2

# Step 4: Run sustained load (1000 requests over 30 seconds)
for i in {1..1000}; do
  curl -s http://127.0.0.1:3001/rpc \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_query","arguments":{"sql":"SELECT 1"}},"id":'$i'}' > /dev/null &
  
  # Rate limit: ~33 req/sec
  if [ $((i % 33)) -eq 0 ]; then
    sleep 1
  fi
done

wait

# Step 5: Stop server
kill $SERVER_PID
sleep 2

# Step 6: Analyze heaptrack output
heaptrack_print heaptrack.mcp-postgres.*.gz > /tmp/heaptrack_report.txt

# Step 7: Check results
echo "=== Heap Allocation Report ==="
grep -A 5 "total allocations" /tmp/heaptrack_report.txt
grep -A 5 "peak heap" /tmp/heaptrack_report.txt
grep -A 5 "peak RSS" /tmp/heaptrack_report.txt
```

**Procedure (Using Valgrind on Linux)**:

```bash
# Install valgrind
sudo apt-get install valgrind

# Run with massif (memory profiler)
valgrind --tool=massif --massif-out-file=/tmp/massif.out \
  ./target/release/mcp-postgres &
SERVER_PID=$!
sleep 2

# Run load test (as above)
for i in {1..1000}; do
  curl -s http://127.0.0.1:3001/rpc ... &
done
wait

kill $SERVER_PID
sleep 2

# Analyze with ms_print
ms_print /tmp/massif.out | head -50
```

**Procedure (Using Rust memory profiling)**:

```bash
# Enable dhat-heap in Cargo.toml (dev dependency)
[dev-dependencies]
dhat = "0.3"

# In main.rs (with cfg guard)
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Allocator = dhat::Allocator;

#[cfg(feature = "dhat-heap")]
fn main() {
  let _guard = dhat::Allocator::new_frame();
  // ... server code ...
}

# Run with dhat enabled
cargo run --release --features dhat-heap &
SERVER_PID=$!
sleep 2

# Run load test
# ... curl requests ...

kill $SERVER_PID

# Check dhat output
cat dhat-heap.json | jq '.total_allocations'
```

**Acceptance Criteria**:
- ✅ Peak heap < 100MB during sustained load
- ✅ Memory growth < 10MB over 1000 requests (stable, not leaking)
- ✅ No allocation patterns showing exponential growth
- ✅ Average allocation size reasonable (no huge allocations)
- ✅ No repeated allocations that should be reused

**Failure Action**: 
- If peak heap > 100MB: INVESTIGATE (memory leak likely)
- If growth > 10MB over 1000 requests: INVESTIGATE (inefficient allocation pattern)
- If allocation spikes detected: Check for unbounded collections or string concatenation
- Run with specific problematic tools to isolate issue

**Common Memory Issues**:
- String concatenation in loops (use String::with_capacity or format! carefully)
- Unbounded Vec growth (use Vec::with_capacity or limit size)
- Connection pool leak (verify connections are released)
- JSON serialization creating copies (use serde streaming if possible)

---

## 5. TOOL PARAMETER VALIDATION BEFORE USE

**CRITICAL**: Must be done BEFORE writing test code.

```bash
# Step 1: Get tools list
TOOLS_RESPONSE=$(curl -s http://127.0.0.1:3001/rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/list","params":{},"id":1}')

# Step 2: Extract tool info
TOOL_NAME="execute_query"
TOOL_INFO=$(echo "$TOOLS_RESPONSE" | jq ".result.tools[] | select(.name==\"$TOOL_NAME\")")

# Step 3: Verify tool exists
if [ -z "$TOOL_INFO" ]; then
  echo "FAIL: Tool '$TOOL_NAME' not found"
  echo "Available tools: $(echo "$TOOLS_RESPONSE" | jq '.result.tools[].name' | head -10)"
  exit 1
fi

# Step 4: Get required parameters
REQUIRED_PARAMS=$(echo "$TOOL_INFO" | jq -r '.inputSchema.required[]')
echo "Required params: $REQUIRED_PARAMS"

# Step 5: Get all parameters
ALL_PARAMS=$(echo "$TOOL_INFO" | jq -r '.inputSchema.properties | keys[]')
echo "All params: $ALL_PARAMS"

# Step 6: Verify parameter types
PARAM_TYPES=$(echo "$TOOL_INFO" | jq '.inputSchema.properties | to_entries[] | "\(.key): \(.value.type)"')
echo "Parameter types: $PARAM_TYPES"
```

**Example: execute_query validation**:
```bash
# Correct parameter name
curl -X POST http://127.0.0.1:3001/rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_query","arguments":{"sql":"SELECT 1"}},"id":1}'

# WRONG - will fail with -32602 (Invalid params)
curl -X POST http://127.0.0.1:3001/rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_query","arguments":{"query":"SELECT 1"}},"id":1}'
```

---

## 6. RELEASE WORKFLOW

**TRIGGER**: When version in `Cargo.toml` is incremented

**STRICT PREREQUISITES**:
- [ ] Branch is `main`
- [ ] Working tree is clean (`git status` shows nothing)
- [ ] Latest commits are pulled (`git pull`)
- [ ] `Cargo.toml` version matches intended release
- [ ] CHANGELOG updated with new version section
- [ ] All documentation updated

### 6.1 Pre-Release Validation Gate

**PROCEDURE**:
```bash
#!/bin/bash
set -e

echo "=== RELEASE VALIDATION GATE ==="

# 1. Version check
CARGO_VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
echo "Release version: $CARGO_VERSION"

# 2. Git check
if [ -n "$(git status --porcelain)" ]; then
  echo "FAIL: Working tree not clean"
  git status
  exit 1
fi
echo "✓ Working tree clean"

# 3. Unit tests
echo "Running unit tests..."
cargo test --lib || { echo "FAIL: Unit tests"; exit 1; }
echo "✓ Unit tests passed"

# 4. Integration tests (requires server)
echo "Running integration tests..."
export DATABASE_URL="postgres://piyush:@localhost:5432/postgres"
cargo run --release -- --http-port 3001 > /tmp/server.log 2>&1 &
SERVER_PID=$!
sleep 4

cargo test --test integration_dual_protocol -- --nocapture --test-threads=1 || { kill $SERVER_PID; echo "FAIL: Dual-protocol tests"; exit 1; }
cargo test --test integration_all_tools -- --nocapture --test-threads=1 || { kill $SERVER_PID; echo "FAIL: All-tools tests"; exit 1; }

kill $SERVER_PID
echo "✓ Integration tests passed"

# 5. Security audit
echo "Running cargo audit..."
cargo audit || { echo "FAIL: Cargo audit"; exit 1; }
echo "✓ No known vulnerabilities"

# 6. Tools count validation
TOOLS_COUNT=$(jq 'length' tools.json)
if [ "$TOOLS_COUNT" -ne 46 ]; then
  echo "FAIL: Expected 46 tools, found $TOOLS_COUNT"
  exit 1
fi
echo "✓ 46 tools present"

echo "=== ALL PRE-RELEASE CHECKS PASSED ==="
echo "Ready to publish v$CARGO_VERSION"
```

### 6.2 Release Publication

**Step 1: Create and push git tag**:
```bash
# Verify version in Cargo.toml
VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)

# Create annotated tag
git tag -a "v$VERSION" -m "Release v$VERSION"

# Verify tag created
git tag -l | grep "v$VERSION"

# Push tag to remote
git push origin "v$VERSION"

# Verify remote
git ls-remote origin | grep "v$VERSION"
```

**Step 2: Publish to crates.io**:
```bash
# Verify not already published
cargo search mcp-postgres | grep "mcp-postgres = \"$VERSION\"" && { echo "Already published"; exit 1; }

# Publish
cargo publish

# Verify published (may take 1-2 minutes)
sleep 60
cargo search mcp-postgres | grep "mcp-postgres = \"$VERSION\"" || { echo "FAIL: Not found on crates.io"; exit 1; }
```

### 6.3 Update Homebrew Formula

**STRICT**: Only after crates.io publication succeeds.

**Step 1: Get release tarball SHA256**:
```bash
VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)

cd /tmp
curl -L -o "mcp-postgres-$VERSION.tar.gz" "https://github.com/corporatepiyush/mcp-pg-rust/archive/refs/tags/v$VERSION.tar.gz"

ACTUAL_SHA=$(shasum -a 256 "mcp-postgres-$VERSION.tar.gz" | awk '{print $1}')
echo "SHA256: $ACTUAL_SHA"
```

**Step 2: Update formula file**:
```bash
FORMULA_PATH="homebrew-mcp-postgres/Formula/mcp_postgres.rb"

# Backup original
cp "$FORMULA_PATH" "$FORMULA_PATH.bak"

# Replace version in URL
sed -i '' "s|/archive/refs/tags/v[0-9.]*\.tar\.gz|/archive/refs/tags/v$VERSION.tar.gz|g" "$FORMULA_PATH"

# Replace SHA256
sed -i '' "s/sha256 \"[a-f0-9]\{64\}\"/sha256 \"$ACTUAL_SHA\"/g" "$FORMULA_PATH"

# Verify changes
diff "$FORMULA_PATH.bak" "$FORMULA_PATH"
```

**Step 3: Test formula locally** (on macOS):
```bash
# Verify syntax
ruby -c "$FORMULA_PATH"

# Test installation
brew install --build-from-source "$FORMULA_PATH"

# Verify binary
which mcp-postgres
mcp-postgres --version

# Uninstall
brew uninstall mcp-postgres
```

**Step 4: Commit and push**:
```bash
git add "homebrew-mcp-postgres/Formula/mcp_postgres.rb"
git commit -m "Update Homebrew formula for v$VERSION release"
git push origin main
```

**Acceptance Criteria**:
- ✅ Git tag v[VERSION] created and pushed
- ✅ Package published to crates.io (verified with `cargo search`)
- ✅ GitHub release artifact available
- ✅ SHA256 matches calculated value
- ✅ Formula syntax valid (Ruby check)
- ✅ Formula committed and pushed

**Failure Action**: If any step fails, STOP and investigate. Do NOT proceed to next step.

---

## 7. ROLLBACK PROCEDURE

**TRIGGER CONDITIONS**:
- Integration test fails after deployment
- Performance regression detected (latency > 2x baseline)
- Security vulnerability discovered
- Success rate drops below 90%
- MCP compliance violation
- Server crash

**Procedure**:
```bash
#!/bin/bash
set -e

echo "=== ROLLBACK INITIATED ==="

# Step 1: Identify last known good version
echo "Recent versions:"
git log --oneline | head -10

# Step 2: Revert current commit
git revert HEAD

# Step 3: Verify revert
git log --oneline | head -3

# Step 4: Push revert
git push origin main

# Step 5: Re-run tests
export DATABASE_URL="postgres://piyush:@localhost:5432/postgres"
cargo test --lib || { echo "FAIL: Unit tests after rollback"; exit 1; }

cargo run --release -- --http-port 3001 > /tmp/server.log 2>&1 &
SERVER_PID=$!
sleep 4

cargo test --test integration_all_tools -- --nocapture --test-threads=1 || { kill $SERVER_PID; echo "FAIL: Integration tests after rollback"; exit 1; }

kill $SERVER_PID

echo "=== ROLLBACK SUCCESSFUL ==="
echo "Rolled back from previous version"
```

---

## 8. COMMON ERROR DIAGNOSIS

### Error: `-32602 (Invalid params)`

**Causes**:
1. Parameter name is wrong (e.g., "query" instead of "sql")
2. Parameter type is wrong (e.g., string instead of array)
3. Required parameter missing
4. Unrecognized parameter

**Fix**:
```bash
# Step 1: Get correct tool definition
curl -s http://127.0.0.1:3001/rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/list","params":{},"id":1}' | \
  jq '.result.tools[] | select(.name=="YOUR_TOOL")'

# Step 2: Compare your parameters against "inputSchema.properties"
# Step 3: Ensure all "required" parameters are present
# Step 4: Ensure types match (string, number, array, object)
```

### Error: `-32601 (Method not found)`

**Causes**:
1. Tool doesn't exist in implementation
2. Tool name is misspelled
3. Tool not listed in tools.json

**Fix**:
```bash
# Step 1: Verify tool exists
grep -r "tool_name" src/actions/

# Step 2: Check tools.json
jq '.[] | select(.name=="tool_name")' tools.json

# Step 3: Check src/main.rs for tool registration
grep "tool_name" src/main.rs
```

### Error: `Protocol violation (missing id or jsonrpc)`

**Causes**:
1. JSON response malformed
2. Server returned non-JSON
3. Network error

**Fix**:
```bash
# Step 1: Check server logs
tail -50 /tmp/server.log

# Step 2: Verify JSON validity
curl http://127.0.0.1:3001/rpc | jq . || echo "Invalid JSON"

# Step 3: Restart server
pkill -f "mcp-postgres"
cargo run --release -- --http-port 3001 &
```

### Error: `Test latency > 200ms`

**Causes**:
1. Slow database query
2. Connection pool initialization (first request)
3. Network latency
4. System load

**Fix**:
```bash
# Step 1: Check database query performance
EXPLAIN ANALYZE <your_query>;

# Step 2: Check connection pool
ps aux | grep postgres | head -5

# Step 3: Monitor system load
top -n 1 | head -20

# Step 4: Retry test (pool may have initialized)
cargo test --test integration_dual_protocol
```

---

## 9. AGENT BEHAVIORAL GUARDRAILS

### 9.1 Before Any Code Change

```
BEFORE modifying:
├─ src/actions/*.rs
├─ Cargo.toml
├─ tests/*.rs
├─ tools.json
├─ SKILLS.md
└─ homebrew-mcp-postgres/

MUST:
1. Ask user for confirmation
2. Show exact changes you intend to make
3. Verify against SKILLS.md procedures
4. Check for side effects
```

### 9.2 Before Any Release

```
MUST RUN (in order):
1. cargo test --lib (100% pass required)
2. Integration tests (90%+ success required)
3. Dual-protocol tests (100% on both protocols)
4. Load tests (GOOD or EXCELLENT tier)
5. cargo audit (zero vulnerabilities)

IF ANY FAILS:
├─ STOP immediately
├─ Investigate root cause
├─ Do NOT proceed to next step
├─ Report to user with exact failure
└─ Wait for user guidance
```

### 9.3 Before Updating Homebrew Formula

```
MUST:
1. Ask user for confirmation
2. Verify crates.io publication succeeded
3. Calculate SHA256 from actual GitHub release
4. Show formula diff before commit
5. Verify file syntax (ruby -c check)
6. Wait for user approval before push
```

### 9.4 Parameter Validation Standard

```
For EVERY tool call, BEFORE test execution:

├─ Verify tool exists: tools/list check
├─ Extract inputSchema from tools/list
├─ For each parameter:
│  ├─ Check name matches exactly
│  ├─ Check type matches
│  ├─ If required: must be provided
│  ├─ If optional: can be omitted
│  └─ Report any mismatches
└─ Proceed only after all checks pass
```

### 9.5 Memory Profiling Standard

```
AFTER load tests or performance-critical changes:

├─ Run heap allocation tracking
├─ Measure:
│  ├─ Peak heap memory
│  ├─ Memory growth over 1000+ requests
│  ├─ Allocation patterns
│  └─ Connection pool memory
├─ DECISION: Peak < 100MB?
│  ├─ NO  → INVESTIGATE (likely memory leak)
│  └─ YES → Continue
├─ DECISION: Growth < 10MB per 1000 requests?
│  ├─ NO  → INVESTIGATE (inefficient allocation)
│  └─ YES → Continue
└─ DECISION: No exponential growth patterns?
   ├─ YES found → INVESTIGATE (unbounded growth issue)
   └─ NO patterns → PASS memory check
```

---

## 10. REFERENCE: Tool Implementation Status

| Tool Name | Implementation | Status | Tests |
|-----------|----------------|--------|-------|
| execute_query | src/actions/query.rs | ✅ | dual_protocol, all_tools |
| execute_insert | src/actions/query.rs | ✅ | dual_protocol, all_tools |
| execute_update | src/actions/query.rs | ✅ | dual_protocol, all_tools |
| execute_delete | src/actions/query.rs | ✅ | dual_protocol, all_tools |
| explain_query | src/actions/query.rs | ✅ | dual_protocol, all_tools |
| async_execute_* | src/actions/query.rs | ✅ | all_tools |
| show_current_user | src/actions/connection.rs | ✅ | dual_protocol |
| list_connections | src/actions/connection.rs | ✅ | all_tools |
| list_tables | src/actions/schema.rs | ✅ | dual_protocol, all_tools |
| list_schemas | src/actions/schema.rs | ✅ | dual_protocol, all_tools |
| list_columns | src/actions/schema.rs | ✅ | all_tools |
| list_indexes | src/actions/schema.rs | ✅ | all_tools |
| list_triggers | src/actions/schema.rs | ✅ | dual_protocol, all_tools |
| list_views | src/actions/schema.rs | ✅ | all_tools |
| list_sequences | src/actions/schema.rs | ✅ | all_tools |
| describe_table | src/actions/schema.rs | ✅ | all_tools |
| create_table | src/actions/schema.rs | ✅ | all_tools |
| drop_table | src/actions/schema.rs | ✅ | all_tools |
| create_view | src/actions/schema.rs | ✅ | all_tools |
| drop_view | src/actions/schema.rs | ✅ | all_tools |
| alter_view | src/actions/schema.rs | ✅ | all_tools |
| create_schema | src/actions/schema.rs | ✅ | all_tools |
| drop_schema | src/actions/schema.rs | ✅ | all_tools |
| create_index | src/actions/schema.rs | ✅ | all_tools |
| drop_index | src/actions/schema.rs | ✅ | all_tools |
| alter_index | src/actions/schema.rs | ✅ | all_tools |
| create_sequence | src/actions/schema.rs | ✅ | all_tools |
| drop_sequence | src/actions/schema.rs | ✅ | all_tools |
| create_partition | src/actions/schema.rs | ✅ | all_tools |
| delete_table_partition | src/actions/schema.rs | ✅ | all_tools |
| list_partitions | src/actions/schema.rs | ✅ | all_tools |
| backup_table | src/actions/schema.rs | ✅ | all_tools |
| async_batch_insert | src/actions/batch.rs | ✅ | all_tools |
| async_batch_update | src/actions/batch.rs | ✅ | all_tools |
| async_batch_delete | src/actions/batch.rs | ✅ | all_tools |
| async_batch_insert_copy | src/actions/batch.rs | ✅ | all_tools |
| analyze_table | src/actions/utility.rs | ✅ | all_tools |
| vacuum_table | src/actions/utility.rs | ✅ | all_tools |
| get_table_size | src/actions/utility.rs | ✅ | all_tools |
| get_database_size | src/actions/utility.rs | ✅ | all_tools |

---

## Reference Guides

Comprehensive guides available in `/guides` folder:

### Performance & Optimization
- **[OPTIMIZATION_STRATEGIES.md](./guides/OPTIMIZATION_STRATEGIES.md)** - Manual vs self-optimizing loops, hybrid approach, decision framework (READ FIRST for optimization strategy)
- **[CODE_OPTIMIZATION.md](./guides/CODE_OPTIMIZATION.md)** - Verified optimizations, measured regressions, profiling guide, benchmarking strategies (TACTICAL implementation for mcp-postgres)
- **[LOW_LEVEL_OPTIMIZATION.md](./guides/LOW_LEVEL_OPTIMIZATION.md)** - Hardware-level optimization: cache lines, mechanical sympathy, CPU architecture, lock-free concurrency (UNIVERSAL principles applicable across all systems)
- **[OPTIMIZATIONS.md](./guides/OPTIMIZATIONS.md)** - Performance tuning parameters

### Compliance & Verification
- **[MCP_COMPLIANCE.md](./guides/MCP_COMPLIANCE.md)** - Input validation rules, error formats, per-tool constraints
- **[MCP_SPEC_VERIFICATION.md](./guides/MCP_SPEC_VERIFICATION.md)** - Compliance test results and verification procedures

### Testing & Measurement
- **[LATENCY_MEASUREMENT.md](./guides/LATENCY_MEASUREMENT.md)** - How to measure and interpret latency
- **[TEST_SETUP.md](./guides/TEST_SETUP.md)** - Test environment setup and procedures
- **[QUICK_TEST.md](./guides/QUICK_TEST.md)** - Quick reference for running tests

### Navigation
- **[INDEX.md](./guides/INDEX.md)** - Complete guide listing and navigation

---

**Last Updated**: 2026-06-14  
**Version**: SDLC Process (version-agnostic)  
**Authority**: Source of truth for all development procedures
