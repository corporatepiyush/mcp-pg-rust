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

# Step 4: Run integration tests
echo "Step 4️⃣  Running comprehensive tool tests..."
echo ""
cargo test --test integration_test_data_tools -- --nocapture --test-threads=1

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ All tests passed!"
    echo ""
    echo "📊 Test Results Summary:"
    echo "  ✓ Schema inspection tools tested"
    echo "  ✓ Query execution tested (simple, JOIN, aggregation)"
    echo "  ✓ Monitoring tools tested"
    echo "  ✓ Performance analysis tested"
    echo "  ✓ Settings inspection tested"
    echo "  ✓ User & security tools tested"
else
    echo ""
    echo "❌ Some tests failed"
    exit 1
fi
echo ""
echo "=================================================="
echo "🎉 Setup and testing complete!"
echo ""
echo "To run tests again without recreating data:"
echo "  cargo test --test integration_test_data_tools -- --nocapture"
echo ""
echo "To clean up and start fresh:"
echo "  psql $DB_URL -c 'DROP SCHEMA public CASCADE; CREATE SCHEMA public;'"
echo "  Then run this script again"
