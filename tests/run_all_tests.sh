#!/bin/bash
set -e

# Build → Start Server → Run All Integration Tests → Stop Server
# Usage: ./tests/run_all_tests.sh [database-url]
# Default database URL: postgres://postgres:postgres@localhost:5432/postgres

DB_URL="${1:-postgres://postgres:postgres@localhost:5432/postgres}"
MCP_PORT="${MCP_PORT:-3000}"
BINARY="./target/release/mcp-postgres"

echo "================================================"
echo "  MCP PostgreSQL — Full Integration Test Runner"
echo "================================================"
echo ""

# Step 1: Build
echo "Step 1/5: Building release binary..."
cargo build --release --quiet
echo "  ✓ Binary built at $BINARY"
echo ""

# Step 2: Kill any stale server on the port
echo "Step 2/5: Ensuring port $MCP_PORT is free..."
OLD_PID=$(lsof -ti :"$MCP_PORT" 2>/dev/null || true)
if [ -n "$OLD_PID" ]; then
    echo "  ⚠️  Killing stale server (PID $OLD_PID) on port $MCP_PORT..."
    kill "$OLD_PID" 2>/dev/null || true
    # Wait for the port to be released
    for i in $(seq 1 10); do
        if ! nc -z 127.0.0.1 "$MCP_PORT" 2>/dev/null; then
            echo "  ✓ Port $MCP_PORT freed"
            break
        fi
        sleep 1
    done
    if nc -z 127.0.0.1 "$MCP_PORT" 2>/dev/null; then
        echo "  ❌ Could not free port $MCP_PORT. Try: kill -9 $OLD_PID"
        exit 1
    fi
else
    echo "  ✓ Port $MCP_PORT is available"
fi
echo ""

# Step 3: Start server
echo "Step 3/5: Starting MCP PostgreSQL server..."
SERVER_LOG=$(mktemp)
$BINARY --database-url "$DB_URL" > "$SERVER_LOG" 2>&1 &
SERVER_PID=$!

# Wait for server to be ready (up to 15 seconds)
for i in $(seq 1 15); do
    if nc -z 127.0.0.1 "$MCP_PORT" 2>/dev/null; then
        echo "  ✓ Server started (PID $SERVER_PID) on port $MCP_PORT"
        break
    fi
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "  ❌ Server failed to start. Log:"
        cat "$SERVER_LOG"
        rm -f "$SERVER_LOG"
        exit 1
    fi
    sleep 1
done

if ! nc -z 127.0.0.1 "$MCP_PORT" 2>/dev/null; then
    echo "  ❌ Server did not become ready within 15 seconds. Log:"
    cat "$SERVER_LOG"
    rm -f "$SERVER_LOG"
    kill "$SERVER_PID" 2>/dev/null || true
    exit 1
fi
echo ""

# Step 4: Run integration tests
echo "Step 4/5: Running integration tests..."
set +e
cargo test --test integration_all_tools -- --nocapture --test-threads=1
TEST_EXIT_CODE=$?
set -e

echo ""
if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo "  ✓ All integration tests passed"
else
    echo "  ❌ Some integration tests failed (exit code $TEST_EXIT_CODE)"
fi
echo ""

# Step 5: Stop server
echo "Step 5/5: Stopping server..."
kill "$SERVER_PID" 2>/dev/null || true
wait "$SERVER_PID" 2>/dev/null || true
rm -f "$SERVER_LOG"
echo "  ✓ Server stopped"
echo ""

echo "================================================"
if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo "  RESULT: ALL TESTS PASSED"
else
    echo "  RESULT: TESTS FAILED (exit code $TEST_EXIT_CODE)"
fi
echo "================================================"

exit $TEST_EXIT_CODE
