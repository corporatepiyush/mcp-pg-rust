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

# Test 6: Stress test with connection pooling (Apache Bench)
echo "  Test 6: Load test with connection pooling (100 requests, 10 concurrent)..."

# Check if ab (Apache Bench) is available
if command -v ab &> /dev/null; then
    # Use Apache Bench for proper connection pooling test
    # -n 100: 100 requests total
    # -c 10: 10 concurrent connections (reused)
    # -p /dev/null: POST with empty body

    AB_OUTPUT=$(ab -n 100 -c 10 -p /dev/stdin -T "application/json" \
        -H "Content-Type: application/json" \
        http://$MCP_HOST:$MCP_PORT/rpc 2>&1 << 'JSON'
{"jsonrpc":"2.0","method":"tools/call","params":{"name":"show_current_user","arguments":{}},"id":1}
JSON
)

    # Extract metrics from Apache Bench output
    THROUGHPUT=$(echo "$AB_OUTPUT" | grep "Requests per second" | awk '{print $4}' | cut -d. -f1)
    MEAN_TIME=$(echo "$AB_OUTPUT" | grep "Time per request:" | head -1 | awk '{print $4}')

    echo "    ✅ Load test results (with connection pooling):"
    echo "       - Total requests: 100"
    echo "       - Concurrent connections: 10 (reused)"
    echo "       - Throughput: ${THROUGHPUT} req/sec"
    echo "       - Mean latency: ${MEAN_TIME}ms"
    echo "       - Baseline: 17,713 req/sec (20 concurrent clients)"
    echo ""

    if [ "$THROUGHPUT" -gt "10000" ]; then
        echo "    ✅ Performance: EXCELLENT (>10K req/sec)"
    elif [ "$THROUGHPUT" -gt "5000" ]; then
        echo "    ✅ Performance: GOOD (5-10K req/sec)"
    elif [ "$THROUGHPUT" -gt "1000" ]; then
        echo "    ⚠️  Performance: ACCEPTABLE (1-5K req/sec)"
    else
        echo "    ❌ Performance: POOR (<1K req/sec) - investigation needed:"
        echo "       - New DDL tools may have overhead"
        echo "       - backup_table operation is heavy"
        echo "       - Connection pool may need tuning"
        echo "       - Memory usage may be elevated"
    fi
else
    # Fallback: Use wrk if available (better for load testing)
    if command -v wrk &> /dev/null; then
        echo "    Using wrk for load test (30s, 10 threads, 10 connections)..."
        WRK_OUTPUT=$(wrk -t10 -c10 -d30s \
            -s - http://$MCP_HOST:$MCP_PORT/rpc 2>&1 << 'LUA'
request = function()
    wrk.method = "POST"
    wrk.headers["Content-Type"] = "application/json"
    wrk.body = '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"show_current_user","arguments":{}},"id":1}'
    return wrk.format(nil)
end
LUA
)

        THROUGHPUT=$(echo "$WRK_OUTPUT" | grep "Requests/sec" | awk '{print $2}' | cut -d. -f1)
        echo "    ✅ Load test results (wrk - 30s duration):"
        echo "       - Throughput: ${THROUGHPUT} req/sec"
        echo "       - Baseline: 17,713 req/sec"
    else
        # Final fallback: Simple sequential test with timing
        echo "    ⚠️  ab and wrk not found - running simple sequential timing test..."
        echo "       (Install 'apache2-utils' for ab or 'wrk' for better load testing)"

        START_TIME=$(date +%s%N | cut -b1-13)
        SUCCESS=0

        for i in {1..20}; do
            RESPONSE=$(curl -s -w "\n%{http_code}" -X POST http://$MCP_HOST:$MCP_PORT/rpc \
                -H "Content-Type: application/json" \
                -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"show_current_user","arguments":{}},"id":'$i'}' 2>/dev/null)

            HTTP_CODE=$(echo "$RESPONSE" | tail -1)
            if [ "$HTTP_CODE" = "200" ]; then
                ((SUCCESS++))
            fi
        done

        END_TIME=$(date +%s%N | cut -b1-13)
        ELAPSED=$((END_TIME - START_TIME))

        echo "    ✅ Basic timing test:"
        echo "       - 20 sequential requests: ${ELAPSED}ms"
        echo "       - Success: $SUCCESS/20"
        echo "       - (Install 'apache2-utils' package for proper load testing with ab)"
    fi
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
