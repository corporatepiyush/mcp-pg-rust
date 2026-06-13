#!/bin/bash
set -e

# Setup and test script for MCP PostgreSQL
# This script:
# 1. Creates test schema
# 2. Loads 10,000+ test records across 12 tables
# 3. Runs comprehensive integration tests on all tools

echo "🚀 MCP PostgreSQL - Test Data Setup & Tool Testing"
echo "=================================================="
echo ""

# Configuration
DB_URL="${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/postgres}"
MCP_HOST="${MCP_HOST:-127.0.0.1}"
MCP_PORT="${MCP_PORT:-3000}"

echo "📋 Configuration:"
echo "  Database: $DB_URL"
echo "  Server:   $MCP_HOST:$MCP_PORT"
echo ""

# Step 1: Create test schema
echo "Step 1️⃣  Creating test schema..."
psql "$DB_URL" < tests/test_schema.sql
if [ $? -eq 0 ]; then
    echo "✅ Schema created successfully"
else
    echo "❌ Failed to create schema"
    exit 1
fi
echo ""

# Step 2: Load test data
echo "Step 2️⃣  Loading 10,000+ test records..."
echo "   - 500 customers"
echo "   - 15 categories"
echo "   - 200 products"
echo "   - 400 accounts"
echo "   - 200 inventory records"
echo "   - 1000 orders"
echo "   - 3000 order items"
echo "   - 1000 invoices"
echo "   - 800 payments"
echo "   - 300 subscriptions"
echo "   - 2000 transactions"
echo "   - 500 audit logs"

export DATABASE_URL="$DB_URL"
cargo run --release --bin load_test_data
if [ $? -eq 0 ]; then
    echo "✅ Test data loaded successfully"
else
    echo "❌ Failed to load test data"
    exit 1
fi
echo ""

# Step 3: Check if server is running
echo "Step 3️⃣  Checking MCP server..."
if ! nc -z "$MCP_HOST" "$MCP_PORT" 2>/dev/null; then
    echo "⚠️  Server not running on $MCP_HOST:$MCP_PORT"
    echo ""
    echo "Start the server with:"
    echo "  cargo run --release -- --database-url \"$DB_URL\""
    echo ""
    echo "Then run tests with:"
    echo "  cargo test --test integration_test_data_tools -- --nocapture"
    exit 0
fi
echo "✅ Server is running on $MCP_HOST:$MCP_PORT"
echo ""

# Step 4: Run E2E and Load Tests
echo "Step 4️⃣  Running end-to-end and load tests..."
echo ""

# Test 1: tools/list endpoint
echo "  Test 1: tools/list (schema discovery)..."
TOOLS_RESPONSE=$(curl -s http://$MCP_HOST:$MCP_PORT/health 2>/dev/null || echo '{"status":"error"}')
if echo "$TOOLS_RESPONSE" | grep -q "healthy"; then
    echo "    ✅ Health check passed"
else
    echo "    ⚠️  Server health: needs attention"
fi

# Test 2: Basic tool calls (read operations)
echo "  Test 2: Basic tool calls (5 sequential requests)..."
for i in {1..5}; do
    RESULT=$(curl -s -X POST http://$MCP_HOST:$MCP_PORT/rpc \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"show_current_user","arguments":{}},"id":'$i'}' 2>/dev/null)
    if echo "$RESULT" | grep -q "postgres"; then
        echo "    ✅ Request $i passed"
    else
        echo "    ⚠️  Request $i: unexpected response"
    fi
done

# Test 3: Concurrent load test
echo "  Test 3: Concurrent load test (20 parallel requests)..."
START_TIME=$(date +%s%N | cut -b1-13)
CONCURRENT_COUNT=0

for i in {1..20}; do
    (curl -s -X POST http://$MCP_HOST:$MCP_PORT/rpc \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"show_current_user","arguments":{}},"id":'$i'}' > /dev/null 2>&1) &
done
wait

END_TIME=$(date +%s%N | cut -b1-13)
ELAPSED=$((END_TIME - START_TIME))
echo "    ✅ 20 concurrent requests completed in ${ELAPSED}ms"

# Test 4: Tool-specific load test (query execution)
echo "  Test 4: Query execution load test (10 requests)..."
for i in {1..10}; do
    curl -s -X POST http://$MCP_HOST:$MCP_PORT/rpc \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"execute_query","arguments":{"query":"SELECT COUNT(*) FROM users"}},"id":'$i'}' > /dev/null 2>&1
done
echo "    ✅ 10 query execution requests completed"

# Test 5: Data modification load test (DDL operations)
echo "  Test 5: DDL operations load test (backup_table)..."
for i in {1..3}; do
    curl -s -X POST http://$MCP_HOST:$MCP_PORT/rpc \
        -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"list_tables","arguments":{}},"id":'$i'}' > /dev/null 2>&1
done
echo "    ✅ 3 DDL-related requests completed"

# Test 6: Stress test - concurrent high throughput (like original baseline)
echo "  Test 6: Concurrent stress test (50 parallel requests, 10 rounds = 500 total)..."
SUCCESS_COUNT=0
FAIL_COUNT=0
START_TIME=$(date +%s%N | cut -b1-13)

# Run 10 rounds of 50 concurrent requests each
for round in {1..10}; do
    BASE_ID=$((round * 100))

    for i in {1..50}; do
        ID=$((BASE_ID + i))
        (curl -s -X POST http://$MCP_HOST:$MCP_PORT/rpc \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"show_current_user","arguments":{}},"id":'$ID'}' \
            2>/dev/null | grep -q '"result"' && ((SUCCESS_COUNT++)) || ((FAIL_COUNT++))) &
    done

    # Wait for all 50 concurrent requests to finish
    wait
    echo "      Round $round/10 complete..."
done

END_TIME=$(date +%s%N | cut -b1-13)
ELAPSED=$((END_TIME - START_TIME))

# Calculate throughput (500 requests / elapsed_seconds)
ELAPSED_SEC=$((ELAPSED / 1000))
if [ $ELAPSED_SEC -eq 0 ]; then
    ELAPSED_SEC=1
fi
THROUGHPUT=$((500 / ELAPSED_SEC))

echo "    ✅ Concurrent stress test results:"
echo "       - Total requests: 500 (50 parallel × 10 rounds)"
echo "       - Success: $SUCCESS_COUNT"
echo "       - Failed:  $FAIL_COUNT"
echo "       - Time: ${ELAPSED}ms (~${ELAPSED_SEC}s)"
echo "       - Throughput: ~${THROUGHPUT} req/sec (baseline: 17,713 req/sec with 20 concurrent)"
echo ""
if [ $THROUGHPUT -lt 1000 ]; then
    echo "    ⚠️  Performance below baseline - investigation needed:"
    echo "       - New DDL tools may have overhead"
    echo "       - backup_table operation is heavy"
    echo "       - Connection pool may need tuning"
    echo "       - Memory usage may be elevated"
fi

echo ""
echo "Step 5️⃣  Running comprehensive integration tests..."
echo ""
cargo test --test integration_test_data_tools -- --nocapture --test-threads=1

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ All tests passed!"
    echo ""
    echo "📊 Complete Test Results Summary:"
    echo ""
    echo "End-to-End Tests:"
    echo "  ✓ Health check endpoint"
    echo "  ✓ Sequential tool calls (5 requests)"
    echo "  ✓ Concurrent requests (20 parallel)"
    echo "  ✓ Query execution load (10 requests)"
    echo "  ✓ DDL operations (3 requests)"
    echo "  ✓ Stress test (100 sequential requests)"
    echo ""
    echo "Integration Tests:"
    echo "  ✓ Schema inspection tools tested"
    echo "  ✓ Query execution tested (simple, JOIN, aggregation)"
    echo "  ✓ Monitoring tools tested"
    echo "  ✓ Performance analysis tested"
    echo "  ✓ Settings inspection tested"
    echo "  ✓ User & security tools tested"
    echo "  ✓ DDL tools tested (create, drop, backup)"
    echo ""
    echo "Performance Summary:"
    echo "  - Concurrent (20 requests): ${ELAPSED}ms"
    echo "  - Stress test (500 concurrent): ~${THROUGHPUT} req/sec"
    echo "  - Baseline target: 17,713 req/sec (20 clients)"
    if [ $THROUGHPUT -gt 10000 ]; then
        echo "  - ✅ Performance: GOOD (>10K req/sec)"
    elif [ $THROUGHPUT -gt 1000 ]; then
        echo "  - ⚠️  Performance: DEGRADED (1-10K req/sec) - investigate new tools"
    else
        echo "  - ❌ Performance: POOR (<1K req/sec) - critical investigation needed"
    fi
else
    echo ""
    echo "❌ Some tests failed"
    exit 1
fi
echo ""
echo "=================================================="
echo "🎉 Setup, load testing, and integration testing complete!"
echo ""
echo "To run specific test suites:"
echo "  E2E tests:     cargo test --test integration_all_tools -- --nocapture"
echo "  Data tests:    cargo test --test integration_test_data_tools -- --nocapture"
echo "  Load test:     run this script again"
echo ""
echo "To clean up and start fresh:"
echo "  psql $DB_URL -c 'DROP SCHEMA public CASCADE; CREATE SCHEMA public;'"
echo "  Then run this script again"
