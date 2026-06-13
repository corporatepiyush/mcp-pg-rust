# SKILLS: Automated SDLC & Agent Workflow Guide

**Version**: 1.3.0 | **Type**: SDLC Automation | **For**: Coding Agents

---

## 1. PROJECT STATE & BASELINE

### Current Production State
- **Version**: 1.3.0
- **Status**: Production Ready ✅
- **24 Tools**: All implemented and MCP v1.0 compliant (removed non-functional transaction tools)
- **Transports**: TCP (3000), HTTP/2 (3001), stdio
- **Test Coverage**: 29 tests (12 integration, 17 data), 100% tool coverage
- **Performance**: P95 < 10ms all tools, 17K+ req/sec concurrent

### Baseline Metrics (DO NOT REGRESS)

**Latency P95 (MUST STAY UNDER 10ms)**:
```
tools/list: 0.19ms
show_current_user: 0.44ms
execute_query (simple): 0.42ms
execute_query (complex): 2.19ms
analyze_db_health: 5.63ms
```

**Throughput**:
- Single client: 1K+ req/sec
- Concurrent (20 clients): 17,713 req/sec
- Target: NO regression from baseline

**MCP Compliance**: 100% (initialize, tools/list, tools/call)

### Memory/Buffer Configuration (FIXED)
```
Pool: min=5, max=20 (proven optimal)
Connection recycle timeout: 5 minutes (idle connections closed after 5min)
Connection create timeout: 5 seconds
Buffers: 4KB (not 16KB - causes regression)
Mimalloc: LARGE_OS_PAGES=1, PAGE_RESET=0, EAGER_COMMIT=0
```

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
```

**Step 3**: Run integration tests
```bash
cargo test --test integration_all_tools -- --nocapture --test-threads=1
```

**Step 4**: Run data tests
```bash
cargo test --test integration_test_data_tools -- --nocapture --test-threads=1
```

**Step 5**: Cleanup
```bash
pkill -f "mcp-postgres --http-port"
```

**ACCEPTANCE CRITERIA**:
- ✅ All 12 integration_all_tools tests pass
- ✅ All 17 integration_test_data_tools tests pass
- ✅ No #[ignore] annotations on any test
- ✅ All 24 tools tested (no missing tool tests)
- ✅ Response validation successful
- ✅ All tests use REAL database (verified via SQL queries in logs)

**FAILURE ACTION**: Block commit, list failing tests, require fix before retry

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
[ "$TOOLS" = "24" ] || exit 1
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
- ✅ tools/list returns exactly 24 tools
- ✅ tools/call executes tool correctly
- ✅ Error handling returns proper JSON-RPC error code
- ✅ HTTP/2 response headers valid
- ✅ All responses include jsonrpc, id, and result/error

**FAILURE ACTION**: Block commit, report which HTTP test failed, require fix

---

## 3. PERFORMANCE VALIDATION WORKFLOW

### 3.1 Latency Measurement

**TRIGGER**: After optimization changes, before release

**PREREQUISITE**: Server running on localhost:3001

**PROCEDURE**:
```bash
cargo build --release --bin measure_latency
./target/release/measure_latency > /tmp/latency_results.txt
```

**ACCEPTANCE CRITERIA**:

**All tools P95 MUST be < 10ms**:
```
✅ tools/list P95 < 0.20ms
✅ show_current_user P95 < 0.50ms
✅ execute_query P95 < 3.00ms
✅ analyze_db_health P95 < 6.00ms
```

**Concurrent load MUST hit target**:
```
✅ Throughput >= 17,000 req/sec
✅ P95 latency under load < 10ms
✅ No timeouts or connection drops
```

**FAILURE ACTION**: 
- If ANY tool exceeds P95 threshold: BLOCK RELEASE
- Report which tool(s) exceeded target
- Provide detailed latency breakdown
- Require performance investigation

**PASSING ACTION**:
- Log baseline metrics
- Compare to previous baseline
- Alert if < 5% improvement OR > 5% regression
- Proceed to next stage

---

### 3.2 Throughput Validation

**TRIGGER**: After pool/connection changes

**PREREQUISITE**: 
- Release build compiled
- Server running

**PROCEDURE**:
```bash
# Create concurrent load test
cargo build --release --bin measure_latency
./target/release/measure_latency | grep "Requests/sec"
```

**ACCEPTANCE CRITERIA**:
- ✅ Concurrent load: >= 17,000 req/sec (baseline)
- ✅ No connection pool errors
- ✅ No timeout errors
- ✅ Stable throughput over 30+ seconds

**FAILURE ACTION**: BLOCK RELEASE, investigate pool configuration

---

## 4. MCP COMPLIANCE VALIDATION WORKFLOW

### 4.1 Protocol Compliance Check

**TRIGGER**: After protocol changes, before release

**PROCEDURE**:

**Test 1: initialize response format**
```bash
RESP=$(echo '{"jsonrpc":"2.0","method":"initialize","params":{},"id":1}' | nc 127.0.0.1 3000)
echo "$RESP" | jq -e '.jsonrpc == "2.0"' >/dev/null || exit 1
echo "$RESP" | jq -e '.result.protocolVersion' >/dev/null || exit 1
echo "$RESP" | jq -e '.result.capabilities' >/dev/null || exit 1
echo "$RESP" | jq -e '.result.serverInfo' >/dev/null || exit 1
```

**Test 2: tools/list format**
```bash
RESP=$(echo '{"jsonrpc":"2.0","method":"tools/list","id":1}' | nc 127.0.0.1 3000)
COUNT=$(echo "$RESP" | jq '.result.tools | length')
[ "$COUNT" = "24" ] || exit 1
# Verify each tool has required fields
echo "$RESP" | jq -e '.result.tools[] | select(.name and .description and .inputSchema)' >/dev/null || exit 1
```

**Test 3: tools/call format**
```bash
RESP=$(echo '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"show_current_user","arguments":{}},"id":1}' | nc 127.0.0.1 3000)
echo "$RESP" | jq -e '.result' >/dev/null || exit 1
echo "$RESP" | jq -e '.jsonrpc == "2.0"' >/dev/null || exit 1
echo "$RESP" | jq -e '.id == 1' >/dev/null || exit 1
```

**ACCEPTANCE CRITERIA**:
- ✅ initialize returns protocolVersion, capabilities, serverInfo
- ✅ tools/list returns 25 tools with name, description, inputSchema
- ✅ tools/call executes tool and returns result
- ✅ All responses have jsonrpc: "2.0"
- ✅ All responses have matching id field
- ✅ Error responses have code and message

**FAILURE ACTION**: BLOCK RELEASE, list which protocol tests failed

---

### 4.2 Tool Definition Validation

**TRIGGER**: After adding/modifying tools

**PROCEDURE**:
```bash
# Validate tools.json syntax
jq . tools.json > /dev/null || exit 1

# Verify all tools have required fields
jq -e '.[] | select(.name and .description and .inputSchema)' tools.json >/dev/null || exit 1

# Verify inputSchema is valid JSON Schema
jq -e '.[] | .inputSchema | select(.type == "object" and has("properties"))' tools.json >/dev/null || exit 1

# Count tools
COUNT=$(jq '. | length' tools.json)
echo "Tools count: $COUNT"
```

**ACCEPTANCE CRITERIA**:
- ✅ tools.json is valid JSON
- ✅ Exactly 24 tools defined
- ✅ Each tool has name, description, inputSchema
- ✅ All inputSchemas have type=object and properties
- ✅ All required parameters listed in required array
- ✅ No duplicate tool names

**FAILURE ACTION**: BLOCK, fix tools.json validation errors

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
- ✅ All 24 tools have at least 1 correctness test

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
[ ] Integration tests pass (all 29 tests)
[ ] HTTP server tests pass (health, tools/list, tools/call, errors)
[ ] Performance baseline met (P95 < 10ms, 17K req/sec)
[ ] MCP compliance verified (initialize, tools/list, tools/call)
[ ] Tool validation tests pass
[ ] Input validation tests pass
[ ] Compilation clean (no errors, warnings resolved)
[ ] Security audit passed
[ ] Version number incremented in Cargo.toml
[ ] CHANGELOG.md updated
[ ] tools.json has exactly 24 tools
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
sed -i '' "s|tags/v[0-9.]*|tags/v1.3.0|g" homebrew-mcp-postgres/Formula/mcp_postgres.rb

# 3. Verify the update
grep "sha256\|tags/v" homebrew-mcp-postgres/Formula/mcp_postgres.rb

# 4. Commit changes (don't push yet - see 4c)
git add homebrew-mcp-postgres/Formula/mcp_postgres.rb
```

**4b. Update Chocolatey Package (Windows)**
```powershell
# 1. Get Windows binary from release and calculate SHA256
$zipUrl = "https://github.com/corporatepiyush/mcp-pg-rust/releases/download/v1.3.0/mcp-postgres-x86_64-pc-windows-gnu.zip"
$outputPath = "$env:TEMP\mcp-postgres-1.3.0.zip"
Invoke-WebRequest -Uri $zipUrl -OutFile $outputPath
$sha256 = (Get-FileHash -Path $outputPath -Algorithm SHA256).Hash

# 2. Update version in nuspec
$nuspecPath = "chocolatey-mcp-postgres\mcp-postgres.nuspec"
(Get-Content $nuspecPath) -replace '<version>.*</version>', '<version>1.3.0</version>' | Set-Content $nuspecPath

# 3. Update URL and checksum in install script
$installScript = "chocolatey-mcp-postgres\tools\chocolateyinstall.ps1"
(Get-Content $installScript) -replace 'v[0-9.]*', 'v1.3.0' | Set-Content $installScript
(Get-Content $installScript) -replace "checksum = '.*'", "checksum = '$sha256'" | Set-Content $installScript

# 4. Commit changes (don't push yet - see 4c)
git add chocolatey-mcp-postgres/
```

**4c. Push all changes**
```bash
git commit -m "Update package managers (Homebrew, Chocolatey) to v1.3.0"
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
- Performance regression > 5%
- MCP compliance violation detected
- Security vulnerability discovered
- P95 latency exceeds 10ms on any tool
- Throughput drops below 17K req/sec

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
| Tool coverage | < 24 tools tested | BLOCK |
| Integration tests | < 29 passing | BLOCK |
| HTTP tests | Any fail | BLOCK |

### Compliance Gates

| Metric | Threshold | Action |
|--------|-----------|--------|
| tools.json count | != 24 | BLOCK |
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
- All 29 tests passing
- 24 tools exactly
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
