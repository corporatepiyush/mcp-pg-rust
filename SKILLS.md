# SKILLS: Automated SDLC & Agent Workflow Guide

**Type**: SDLC Automation & Constraint Checking | **For**: Development Process

---

## 1. CONSTRAINTS & ARCHITECTURAL DECISIONS

### Key Constraints

**Protocol Architecture**:
- TCP and HTTP servers operate independently
- Each HTTP request gets a random connection from pool
- Cannot maintain transaction state across requests
- All operations must be atomic within single request

**Testing Strategy**:
- Unit tests for protocol parsing and validation
- Integration tests for tool functionality
- Dual-protocol testing for parity verification
- Load testing for concurrent behavior
- All tests use REAL database (no mocks)

**Code Quality**:
- Zero compiler warnings in library code
- Idiomatic Rust patterns throughout
- Comprehensive error handling
- Input validation at system boundaries

---

## 1.5 CONTROL FLOW DECISIONS & POST-ACTIVITY VERIFICATION

### Critical Decision Tree

```
START TASK
├─ Tool parameter validation
│  ├─ DECISION: Parameter names match actual tool schema?
│  │  ├─ NO → FIX: Verify against tools/list response
│  │  │        ACTION: Use 'sql' not 'query', validate all param names
│  │  └─ YES → Continue
│  └─ DECISION: Required parameters provided?
│     ├─ NO → FIX: Add missing required params
│     │        EXAMPLE: list_triggers needs 'table' param
│     └─ YES → Continue
│
├─ Tool existence verification
│  ├─ DECISION: Tool exists in tools/list?
│  │  ├─ NO → FIX: Use alternative tool
│  │  │        MAPPING: list_databases → use list_schemas
│  │  │        MAPPING: show_table_structure → use describe_table
│  │  │        ACTION: Query tools/list before test
│  │  └─ YES → Continue
│  └─ DECISION: Tool is implemented (not returning Method not found)?
│     ├─ NO → FIX: Verify implementation exists in src/actions/
│     │        ACTION: grep for tool name in source
│     └─ YES → Continue
│
├─ Database state verification
│  ├─ DECISION: Required test data exists?
│  │  ├─ NO → FIX: Load test data first
│  │  │        ACTION: Run load_test_data binary
│  │  │        ACTION: Verify tables exist via list_tables
│  │  └─ YES → Continue
│  └─ DECISION: Trigger prerequisites met (for list_triggers)?
│     ├─ NO → FIX: Use table that exists
│     │        ACTION: Query available tables first
│     └─ YES → Continue
│
├─ Protocol verification (TCP vs HTTP)
│  ├─ DECISION: Both TCP and HTTP working?
│  │  ├─ TCP FAIL → FIX: Check server on port 3000
│  │  │         ACTION: nc -zv 127.0.0.1 3000
│  │  ├─ HTTP FAIL → FIX: Check server on port 3001
│  │  │          ACTION: curl http://127.0.0.1:3001/health
│  │  └─ BOTH OK → Continue
│  └─ DECISION: Same success rate on both protocols?
│     ├─ NO → FIX: Investigate protocol-specific issue
│     │        ACTION: Run dual_protocol tests
│     │        ACTION: Check latency difference
│     └─ YES → Continue
│
└─ Proceed with task
```

### Post-Activity Verification Checklist

**AFTER RUNNING ANY TEST SUITE:**

- [ ] **Success Rate Check**
  - [ ] Minimum 90% success rate? (NOT 40%!)
  - [ ] If < 90%: STOP and investigate
  - [ ] Root causes:
    - [ ] Wrong parameter names (e.g., 'query' vs 'sql')
    - [ ] Tool doesn't exist (use tools/list to verify)
    - [ ] Missing required parameters
    - [ ] Database object doesn't exist

- [ ] **Latency Verification**
  - [ ] TCP latencies reasonable (typically 0-25ms)?
  - [ ] HTTP latencies reasonable (typically 0-75ms with first-request overhead)?
  - [ ] If outlier found:
    - [ ] Check for first-request overhead or cache population
    - [ ] Verify it's not a database-dependent slow query

- [ ] **Protocol Parity Check**
  - [ ] TCP and HTTP have same success rate?
  - [ ] Both protocols return same result format?
  - [ ] Latency difference < 15x (HTTP slower is normal)?

- [ ] **Tool Availability Check**
  - [ ] All tools in test are in tools/list response?
  - [ ] No "Method not found" errors in logs?
  - [ ] Run diagnostic:
    ```bash
    curl -s http://127.0.0.1:3001/rpc \
      -d '{"jsonrpc":"2.0","method":"tools/list","params":{},"id":1}' \
      -H "Content-Type: application/json" | jq '.result.tools[] | .name'
    ```

**AFTER MODIFYING TOOL TESTS:**

- [ ] **Parameter Validation**
  - [ ] All tool parameters match tool definition in tools.json?
  - [ ] Required parameters are always provided?
  - [ ] Parameter types are correct (string, number, boolean, object)?

- [ ] **Test Coverage**
  - [ ] Both TCP and HTTP tested for each tool?
  - [ ] Success cases tested?
  - [ ] Error cases tested?
  - [ ] Edge cases (empty params, null values) tested?

- [ ] **Result Validation**
  - [ ] Response contains jsonrpc, id, result/error?
  - [ ] Error responses have code and message?
  - [ ] Success responses have non-null result?

**AFTER DATABASE OPERATIONS (CREATE/DROP/MODIFY):**

- [ ] **Immediate Verification (within 5 seconds)**
  - [ ] Object exists/doesn't exist as expected?
  - [ ] list_tables shows new table?
  - [ ] list_indexes shows new index?
  - [ ] list_schemas shows new schema?

- [ ] **Backup Verification**
  - [ ] backup_table created backup_<name> table?
  - [ ] Backup contains all columns from original?
  - [ ] Backup contains all rows from original?
  - [ ] Backup created before any dangerous operation?

- [ ] **Cleanup Verification**
  - [ ] No orphaned objects left from test?
  - [ ] No duplicate indexes?
  - [ ] All temporary tables dropped?

**AFTER PERFORMANCE TESTING:**

- [ ] **Baseline Comparison**
  - [ ] Latencies consistent with previous runs?
  - [ ] Throughput consistent with previous baseline?
  - [ ] No significant regression detected?

- [ ] **Load Test Health**
  - [ ] High success rate under concurrent load?
  - [ ] No timeouts or connection resets?
  - [ ] Connection pool handling concurrent requests?
  - [ ] Memory usage stable?

---

## 2. AUTOMATED TEST WORKFLOW

### 2.1 Unit Test Execution

**TRIGGER**: Before every commit, on code changes in src/

**PROCEDURE**:
```bash
cargo test --lib
```

**ACCEPTANCE CRITERIA**:
- ✅ All tests pass
- ✅ No compiler warnings (except allow(dead_code) for intentional)
- ✅ Zero test panics
- ✅ Test execution completes in < 30 seconds

**FAILURE ACTION**: Block commit, report which tests failed, require fix

---

### 2.2 Integration Test Execution

**TRIGGER**: Before release, after functional changes

**PREREQUISITES**:
- PostgreSQL running on localhost:5432
- Database accessible via `postgres://piyush:@localhost:5432/postgres`
- Server NOT already running on port 3000
- Load test data already in database (run load_test_data binary)

**PROCEDURE**:

**Step 1**: Start server in background
```bash
DATABASE_URL="postgres://piyush:@localhost:5432/postgres" \
  cargo run --release -- --http-port 3001 > /tmp/server.log 2>&1 &
sleep 3
```

**Step 2**: Verify server is running
```bash
nc -zv 127.0.0.1 3000 || exit 1
curl -s http://127.0.0.1:3001/health | jq -r '.status' | grep -q "healthy" || exit 1
```

**Step 3**: Run dual-protocol integration tests (CRITICAL for protocol parity)
```bash
cargo test --test integration_dual_protocol -- --nocapture --test-threads=1
# MUST SEE: "TCP  │ Success: 10/10 (100.0%) │ ..." and "HTTP │ Success: 10/10 (100.0%) │ ..."
# FAIL IF: < 90% success rate on either protocol
```

**Step 4**: Run integration tests
```bash
cargo test --test integration_all_tools -- --nocapture --test-threads=1
```

**Step 5**: Run data tests
```bash
cargo test --test integration_test_data_tools -- --nocapture --test-threads=1
```

**Step 6**: Run load tests (Rust-based, not shell)
```bash
cargo test --test load_test -- --ignored --nocapture
# MUST SEE: "GOOD" or "EXCELLENT" performance tier
# FAIL IF: "DEGRADED" or "CRITICAL"
```

**Step 7**: Cleanup
```bash
pkill -f "mcp-postgres"
```

**POST-EXECUTION VERIFICATION** (from Section 1.5):
- [ ] Dual-protocol test: 100% success on both TCP (port 3000) and HTTP (port 3001)?
  - [ ] If one protocol fails: INVESTIGATE protocol-specific issue
  - [ ] If both fail: Tool doesn't exist (verify with tools/list)
- [ ] Individual test latencies make sense?
  - [ ] TCP should be 2-20ms per call
  - [ ] HTTP should be 1-200ms per call (first call may be slower)
  - [ ] If outlier > 200ms: Check if it's first request or database-dependent
- [ ] All assertions passed (no panics)?
  - [ ] If panic: Read error message, likely parameter or data mismatch

**ACCEPTANCE CRITERIA**:
- ✅ Dual-protocol tests: 100% success (10/10 on both TCP and HTTP)
- ✅ All 12 integration_all_tools tests pass
- ✅ All 17 integration_test_data_tools tests pass
- ✅ Load test: 100% success, GOOD or EXCELLENT tier
- ✅ No #[ignore] annotations on any test
- ✅ All tools tested (no missing tool tests)
- ✅ Response validation successful
- ✅ All tests use REAL database (verified via SQL queries in logs)

**DDL Integration Tests (NEW - Must pass before merging DDL tools)**:

Each test creates schema objects, verifies they exist, and cleans up:

**Tables (create_table, drop_table)**:
```bash
# Create table with columns
tools/call create_table {table: "test_ddl_table", columns: ["id SERIAL PRIMARY KEY", "name VARCHAR(255) NOT NULL"]}
# Verify table exists in list_tables
tools/call list_tables {}
# Drop table
tools/call drop_table {table: "test_ddl_table"}
```

**Views (create_view, drop_view, alter_view)**:
```bash
# Create base table
tools/call create_table {table: "test_base", columns: ["id SERIAL PRIMARY KEY", "val INT"]}
# Create view
tools/call create_view {view_name: "test_view", query: "SELECT id, val FROM test_base"}
# Alter view (rename)
tools/call alter_view {view_name: "test_view", rename_to: "test_view_renamed"}
# Drop view
tools/call drop_view {view_name: "test_view_renamed"}
# Cleanup
tools/call drop_table {table: "test_base"}
```

**Schemas (create_schema, drop_schema)**:
```bash
# Create schema
tools/call create_schema {schema_name: "test_schema"}
# Verify in list_schemas
tools/call list_schemas {}
# Drop schema
tools/call drop_schema {schema_name: "test_schema"}
```

**Indexes (create_index, drop_index, alter_index)**:
```bash
# Create table
tools/call create_table {table: "test_idx_table", columns: ["id SERIAL PRIMARY KEY", "email VARCHAR(255)"]}
# Create index
tools/call create_index {index_name: "idx_test_email", table: "test_idx_table", columns: ["email"]}
# Alter index (rename)
tools/call alter_index {index_name: "idx_test_email", rename_to: "idx_test_email_v2"}
# Drop index
tools/call drop_index {index_name: "idx_test_email_v2"}
# Cleanup
tools/call drop_table {table: "test_idx_table"}
```

**Sequences (create_sequence, drop_sequence)**:
```bash
# Create sequence
tools/call create_sequence {sequence_name: "test_seq", start: 100, increment: 1}
# Verify nextval works
tools/call execute_query {query: "SELECT nextval('test_seq')"}
# Drop sequence
tools/call drop_sequence {sequence_name: "test_seq"}
```

**Partitions (create_partition, drop_partition, list_partitions)**:
```bash
# Create partitioned table
tools/call execute_query {query: "CREATE TABLE test_parts (id INT, data TEXT) PARTITION BY RANGE (id)"}
# Create partition
tools/call create_partition {table: "test_parts", partition_name: "test_parts_1", partition_type: "RANGE", column: "id", values: "FROM (1) TO (100)"}
# List partitions
tools/call list_partitions {table: "test_parts"}
# Drop partition
tools/call drop_partition {partition_name: "test_parts_1"}
# Cleanup
tools/call drop_table {table: "test_parts", cascade: true}
```

**Data Safety (backup_table)**:
```bash
# Create table with data
tools/call create_table {table: "important_data", columns: ["id SERIAL PRIMARY KEY", "data TEXT"]}
tools/call execute_insert {table: "important_data", columns: ["data"], rows: [["critical"]]}
# CRITICAL: Backup before any risky operation
tools/call backup_table {table: "important_data"}
# Table is now safe: backup_important_data contains full copy with data
# If original is dropped: data recoverable from backup_important_data
# Drop with confidence (data is safe)
tools/call drop_table {table: "important_data"}
# Recovery: data still exists in backup_important_data
```

**FAILURE ACTION**: Block commit, list failing tests, require fix before retry

---

### 2.2.5 Dual-Protocol Integration Test

**NEW**: Tests both TCP and HTTP with statistics tracking and comparison

**TRIGGER**: After any protocol-related changes, as part of integration testing

**WHAT IT TESTS**:
- Both TCP (port 3000) and HTTP (port 3001) for each tool
- Side-by-side latency comparison
- Success/failure rates per protocol
- Protocol parity (same result on both transports)

**PROCEDURE**:
```bash
# Start server
cargo run --bin mcp-postgres -- --database-url "$DATABASE_URL" &
sleep 4

# Run dual-protocol tests
cargo test --test integration_dual_protocol -- --nocapture --test-threads=1

# Kill server
pkill -f "mcp-postgres"
```

**EXPECTED OUTPUT**:
```
✓ TCP    0ms | ✓ HTTP    1ms | show_current_user
✓ TCP    1ms | ✓ HTTP    1ms | list_schemas
✓ TCP    5ms | ✓ HTTP    1ms | list_triggers
...
TCP  │ Success: 10/10 (100.0%) │ Avg latency: 5.6ms
HTTP │ Success: 10/10 (100.0%) │ Avg latency: 20.1ms
```

**ACCEPTANCE CRITERIA**:
- ✅ Both TCP and HTTP: 100% success (10/10)
- ✅ TCP latency: 2-20ms per request
- ✅ HTTP latency: 1-200ms per request
- ✅ Protocol parity: Same success rate on both
- ✅ Per-tool comparison table generated

**FAILURE ACTION**: 
- If TCP fails: Check TCP server on port 3000
- If HTTP fails: Check HTTP server on port 3001
- If one protocol succeeds but other fails: Protocol-specific issue detected
- If both fail: Tool likely doesn't exist (verify tools/list)

---

### 2.3 HTTP Server Test Execution

**TRIGGER**: After any HTTP/axum changes, before HTTP-related releases

**PREREQUISITES**:
- Server running on port 3001 with HTTP/2
- curl available
- jq available for JSON parsing

**PROCEDURE**:

**Test 1: Health Endpoint**
```bash
STATUS=$(curl -s http://127.0.0.1:3001/health | jq -r '.status')
[ "$STATUS" = "healthy" ] || exit 1
```

**Test 2: tools/list via HTTP**
```bash
TOOLS=$(curl -s -X POST http://127.0.0.1:3001/rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/list","id":1}' | \
  jq -r '.result.tools | length')
[ "$TOOLS" -gt 0 ] || exit 1
```

**Test 3: tools/call via HTTP**
```bash
RESULT=$(curl -s -X POST http://127.0.0.1:3001/rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"show_current_user","arguments":{}},"id":1}' | \
  jq -r '.result.user')
[ -n "$RESULT" ] || exit 1
```

**Test 4: Error Handling**
```bash
ERROR=$(curl -s -X POST http://127.0.0.1:3001/rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"nonexistent","arguments":{}},"id":1}' | \
  jq -r '.error.code')
[ "$ERROR" = "-32601" ] || exit 1
```

**ACCEPTANCE CRITERIA**:
- ✅ Health endpoint responds with status=healthy
- ✅ tools/list returns all available tools
- ✅ tools/call executes tool correctly
- ✅ Error handling returns proper JSON-RPC error code
- ✅ HTTP/2 response headers valid
- ✅ All responses include jsonrpc, id, and result/error

**FAILURE ACTION**: Block commit, report which HTTP test failed, require fix

---

## 3. PERFORMANCE VALIDATION WORKFLOW

**TRIGGER**: After optimization changes or performance-related modifications

**PROCEDURE**:
1. Run load tests with concurrent requests
2. Measure latency distribution across all tools
3. Verify connection pooling under load
4. Check memory stability

**ACCEPTANCE CRITERIA**:
- ✅ Latencies consistent with baseline
- ✅ Throughput stable under concurrent load
- ✅ No connection pool errors
- ✅ Success rate high (> 90%)
- ✅ No timeouts or resets
- ✅ Memory usage stable

**FAILURE ACTION**: 
- Block release
- Report specific tools with latency issues
- Investigate root cause before proceeding

---

## 4. MCP COMPLIANCE VALIDATION WORKFLOW

**TRIGGER**: After protocol changes or tool modifications, before release

**PROCEDURE**:
1. Verify JSON-RPC protocol compliance (jsonrpc: "2.0", id matching, proper result/error)
2. Test initialize method returns protocol info
3. Test tools/list returns all tools with required fields
4. Test tools/call executes tools correctly
5. Validate error responses have code and message
6. Validate tools.json structure (all required fields present)
7. Verify no duplicate tool names

**ACCEPTANCE CRITERIA**:
- ✅ All JSON-RPC responses properly formatted
- ✅ initialize returns protocol version, capabilities, server info
- ✅ tools/list returns all tools with name, description, inputSchema
- ✅ tools/call executes tool and returns result
- ✅ All responses have jsonrpc and id
- ✅ Error responses have code and message
- ✅ tools.json is valid
- ✅ All tools have required fields
- ✅ No duplicate tool names

**FAILURE ACTION**: Block release, fix protocol or tool definition issues

---

## 5. FUNCTIONAL VALIDATION WORKFLOW

### 5.1 Tool Correctness Test

**TRIGGER**: After tool implementation or modification

**PROCEDURE FOR EACH TOOL**:

```rust
#[test]
fn test_tool_<name>_correctness() {
    // 1. Valid input test
    let result = tcp_request("<tool_name>", json!({"valid": "input"}));
    assert!(result.is_ok());
    assert!(result.unwrap().get("result").is_some());
    
    // 2. Invalid input test
    let invalid = tcp_request("<tool_name>", json!({"wrong": "type"}));
    assert!(invalid.is_err() || invalid.unwrap().get("error").is_some());
    
    // 3. Required param test
    let missing = tcp_request("<tool_name>", json!({}));
    assert!(missing.is_err() || missing.unwrap().get("error").is_some());
}
```

**ACCEPTANCE CRITERIA**:
- ✅ Tool accepts valid inputs and returns result
- ✅ Tool rejects invalid inputs with proper error
- ✅ Tool validates required parameters
- ✅ Response JSON is valid and parseable
- ✅ All 34 tools have at least 1 correctness test

**FAILURE ACTION**: BLOCK commit, fix failing tool test

---

### 5.2 Input Validation Test

**TRIGGER**: After validation rule changes

**TEST MATRIX**:

| Tool | Test Case | Expected |
|------|-----------|----------|
| execute_query | sql = "" | Error: Required |
| execute_query | sql length > 10K | Error: Too long |
| execute_query | sql = "DROP TABLE" | Error: Not SELECT |
| execute_delete | sql without WHERE | Error: Safety |
| batch_insert | rows > 1000 | Error: Too many |
| describe_table | table = "" | Error: Required |
| get_setting | nonexistent setting | Error: Not found |

**PROCEDURE**:
```bash
# For each test case in matrix:
RESP=$(echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"<tool>","arguments":<input>},"id":1}' | nc 127.0.0.1 3000)
ERROR=$(echo "$RESP" | jq -r '.error.code')
[ -n "$ERROR" ] && [ "$ERROR" != "null" ] || exit 1
```

**ACCEPTANCE CRITERIA**:
- ✅ All invalid inputs return error responses
- ✅ Error messages are helpful and actionable
- ✅ Error messages include suggestions
- ✅ No invalid inputs execute (safety first)

**FAILURE ACTION**: BLOCK, report which validation failed

---

## 6. BUILD WORKFLOW

### 6.1 Compilation Check

**TRIGGER**: Every commit

**PROCEDURE**:
```bash
cargo check
cargo build --release
```

**ACCEPTANCE CRITERIA**:
- ✅ Zero compilation errors
- ✅ No unsafe code unless documented
- ✅ All warnings resolved (fix or use #[allow] with reason)
- ✅ Build completes in < 60 seconds (release)

**FAILURE ACTION**: BLOCK, show compiler errors

---

### 6.2 Dependency Audit

**TRIGGER**: Before release

**PROCEDURE**:
```bash
cargo audit
cargo outdated --format list
```

**ACCEPTANCE CRITERIA**:
- ✅ Zero security vulnerabilities
- ✅ All critical updates applied
- ✅ No deprecated dependencies

**FAILURE ACTION**: BLOCK, update vulnerable dependencies

---

## 7. RELEASE WORKFLOW

### 7.1 Pre-Release Checklist

**GATE**: Must pass ALL checks before release

**CHECKS**:
```
[ ] Unit tests pass (cargo test --lib)
[ ] Integration tests pass (all 34 tests)
[ ] HTTP server tests pass (health, tools/list, tools/call, errors)
[ ] Performance baseline met (P95 < 10ms, 17K req/sec)
[ ] MCP compliance verified (initialize, tools/list, tools/call)
[ ] Tool validation tests pass
[ ] Input validation tests pass
[ ] Compilation clean (no errors, warnings resolved)
[ ] Security audit passed
[ ] Version number incremented in Cargo.toml
[ ] CHANGELOG.md updated
[ ] tools.json has exactly 34 tools
[ ] Homebrew formula will be updated after crates.io release
[ ] Chocolatey package will be updated after crates.io release
```

**EXECUTION**:
```bash
#!/bin/bash
set -e

echo "=== RELEASE VALIDATION GATE ==="

# 1. Compile
cargo build --release

# 2. Unit tests
cargo test --lib

# 3. Integration tests
DATABASE_URL="postgres://piyush:@localhost:5432/postgres" \
  cargo run --release -- --http-port 3001 > /tmp/server.log 2>&1 &
sleep 3
cargo test --test integration_all_tools -- --nocapture --test-threads=1
cargo test --test integration_test_data_tools -- --nocapture --test-threads=1
pkill -f "mcp-postgres --http-port"

# 4. Performance
./target/release/measure_latency | grep -E "(P95|Requests/sec)"

# 5. Tools validation
jq '. | length' tools.json | grep -q "^25$"

# 6. Security
cargo audit

echo "=== ALL CHECKS PASSED ==="
```

**FAILURE ACTION**: BLOCK RELEASE, list which checks failed

---

### 7.2 Release Publication

**TRIGGER**: After pre-release checklist passes

**PROCEDURE**:

**Step 1**: Tag release
```bash
git tag v1.3.0
git push origin v1.3.0
```

**Step 2**: Publish to crates.io
```bash
cargo publish
```

**Step 3**: Create GitHub release
```bash
gh release create v1.3.0 --title "v1.3.0" --notes "Release notes"
```

**Step 4**: Update Package Managers

**4a. Update Homebrew Formula (macOS)**
```bash
# After successful crates.io publication, update the homebrew formula
# 1. Get the tarball SHA256 from the GitHub release
cd /tmp
curl -L https://github.com/corporatepiyush/mcp-pg-rust/archive/refs/tags/v1.3.0.tar.gz -o mcp-postgres-1.3.0.tar.gz
SHA256=$(shasum -a 256 mcp-postgres-1.3.0.tar.gz | awk '{print $1}')

# 2. Update the formula in the main repo
cd /path/to/mcp-postgres
sed -i '' "s/sha256 \".*\"/sha256 \"$SHA256\"/" homebrew-mcp-postgres/Formula/mcp_postgres.rb
sed -i '' "s|tags/v[0-9.]*|tags/v[VERSION]|g" homebrew-mcp-postgres/Formula/mcp_postgres.rb

# 3. Verify the update
grep "sha256\|tags/v" homebrew-mcp-postgres/Formula/mcp_postgres.rb

# 4. Commit changes (don't push yet - see 4c)
git add homebrew-mcp-postgres/Formula/mcp_postgres.rb
```

**4b. Update Chocolatey Package (Windows)**
```powershell
# 1. Get Windows binary from release and calculate SHA256
$zipUrl = "https://github.com/corporatepiyush/mcp-pg-rust/releases/download/v[VERSION]/mcp-postgres-x86_64-pc-windows-gnu.zip"
$outputPath = "$env:TEMP\mcp-postgres-[VERSION].zip"
Invoke-WebRequest -Uri $zipUrl -OutFile $outputPath
$sha256 = (Get-FileHash -Path $outputPath -Algorithm SHA256).Hash

# 2. Update version in nuspec
$nuspecPath = "chocolatey-mcp-postgres\mcp-postgres.nuspec"
(Get-Content $nuspecPath) -replace '<version>.*</version>', '<version>[VERSION]</version>' | Set-Content $nuspecPath

# 3. Update URL and checksum in install script
$installScript = "chocolatey-mcp-postgres\tools\chocolateyinstall.ps1"
(Get-Content $installScript) -replace 'v[0-9.]*', 'v[VERSION]' | Set-Content $installScript
(Get-Content $installScript) -replace "checksum = '.*'", "checksum = '$sha256'" | Set-Content $installScript

# 4. Commit changes (don't push yet - see 4c)
git add chocolatey-mcp-postgres/
```

**4c. Push all changes**
```bash
git commit -m "Update package managers (Homebrew, Chocolatey) for [VERSION] release"
git push
```

**ACCEPTANCE CRITERIA**:
- ✅ Git tag created and pushed
- ✅ Package published to crates.io
- ✅ GitHub release created
- ✅ Homebrew formula updated with new version and SHA256
- ✅ Formula changes committed and pushed
- ✅ Documentation updated

---

## 8. ROLLBACK WORKFLOW

### 8.1 Rollback Trigger Conditions

**Automatic rollback if**:
- Any integration test fails after deployment
- Performance regression detected
- MCP compliance violation detected
- Security vulnerability discovered
- Latency degradation significant
- Success rate drops substantially

**PROCEDURE**:
```bash
# 1. Identify last good version
git log --oneline | head -5

# 2. Revert to last known good
git revert HEAD
git push

# 3. Re-run test suite
./tests/integration_all_tools.rs
./target/release/measure_latency

# 4. Notify
echo "Rolled back due to [reason]"
```

---

## 9. AGENT TASK PATTERNS

### Pattern 1: Add New Tool

**INPUT**: Tool specification
- name: string (lowercase_underscore)
- description: string (comprehensive)
- parameters: dict of {name: type, description, required}

**AGENT STEPS**:
1. Create function in src/actions/\*.rs
2. Add to tools.json with schema
3. Add validation in src/validation.rs
4. Add to dispatcher in src/server.rs
5. Write integration test
6. Run unit + integration tests
7. Measure latency (must be P95 < 10ms)
8. Verify MCP compliance
9. Commit with message: "Add tool: <name>"

**VERIFICATION**: Integration test passes, latency acceptable, no regressions

---

### Pattern 2: Fix Performance Regression

**INPUT**: Baseline latency > target

**AGENT STEPS**:
1. Measure current baseline (measure_latency)
2. Identify regressed tools
3. Review recent changes to those tools
4. Propose optimization (with measurement)
5. Implement change
6. Re-measure latency
7. Verify all tools P95 < 10ms
8. Verify throughput >= 17K req/sec
9. Run integration tests
10. Commit with message: "Optimize: [what changed]"

**VERIFICATION**: Latency back to baseline, no new regressions

---

### Pattern 3: Fix Test Failure

**INPUT**: Test failing

**AGENT STEPS**:
1. Run failing test with --nocapture
2. Examine error message
3. Identify root cause (DB, validation, logic)
4. Fix in appropriate source file
5. Re-run test
6. Run all related tests
7. Commit with message: "Fix: [test name]"

**VERIFICATION**: All tests pass

---

### Pattern 4: Update MCP Compliance

**INPUT**: MCP spec change

**AGENT STEPS**:
1. Review spec change
2. Update protocol.rs if needed
3. Update tool schema if needed
4. Update validation rules if needed
5. Run compliance tests
6. Run integration tests
7. Verify HTTP server still works
8. Commit with message: "MCP: [what changed]"

**VERIFICATION**: All compliance checks pass

---

## 10. AUTOMATED GATES & THRESHOLDS

### Performance Gates

| Metric | Threshold | Action |
|--------|-----------|--------|
| P95 latency (any tool) | > 10ms | BLOCK |
| Concurrent throughput | < 17K req/sec | BLOCK |
| Memory per connection | > 5MB | WARN |
| Response size | > 1MB | WARN |

### Test Gates

| Metric | Threshold | Action |
|--------|-----------|--------|
| Test pass rate | < 100% | BLOCK |
| Tool coverage | < 29 tools tested | BLOCK |
| Integration tests | < 29 passing | BLOCK |
| HTTP tests | Any fail | BLOCK |

### Compliance Gates

| Metric | Threshold | Action |
|--------|-----------|--------|
| tools.json count | != 29 | BLOCK |
| JSON-RPC format | Non-compliant | BLOCK |
| MCP protocol version | Missing | BLOCK |
| Error responses | Invalid format | BLOCK |

### Code Gates

| Metric | Threshold | Action |
|--------|-----------|--------|
| Compilation errors | > 0 | BLOCK |
| Security vulnerabilities | > 0 | BLOCK |
| #[ignore] annotations | > 0 | BLOCK |

---

## 11. QUICK REFERENCE FOR AGENTS

### Before ANY code change:
```bash
git status
git diff
```

### After code change:
```bash
cargo test --lib                          # Unit tests
cargo build --release                     # Compile
./target/release/measure_latency          # Perf check
```

### Before commit:
```bash
cargo test --test integration_all_tools -- --nocapture
```

### Before release:
```bash
./run_release_validation.sh  # Runs all gates
```

### Current baseline to NOT regress:
- All P95 latencies < 10ms
- Concurrent throughput >= 17,713 req/sec
- All 34 tests passing
- 29 tools exactly
- 100% MCP v1.0 compliance

---

## Guides Folder

Additional reference documentation available in `/guides`:
- **MCP_COMPLIANCE.md** - Validation rules, error formats, per-tool constraints
- **MCP_SPEC_VERIFICATION.md** - Compliance test results and verification procedures
- **LATENCY_MEASUREMENT.md** - How to measure and interpret latency
- **TEST_SETUP.md** - Test environment setup and procedures
- **QUICK_TEST.md** - Quick reference for common test commands
- **OPTIMIZATIONS.md** - Performance tuning parameters

See [guides/INDEX.md](./guides/INDEX.md) for complete guide listing.

---

**For coding agents: Follow the workflows above. Each section is a task gate. If a task fails, stop and report the failure before proceeding.**
