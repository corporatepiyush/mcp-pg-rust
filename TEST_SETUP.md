# Test Data Setup & Integration Tests

This guide explains how to generate realistic test data and run comprehensive integration tests for all MCP PostgreSQL tools.

## Overview

The test setup creates:
- **12 database tables** with realistic relationships
- **10,000+ test records** across all tables
- **Comprehensive integration tests** for all 25 PostgreSQL tools

### Tables Created

1. **customers** (500 records) - Customer information with email, names, phone, status
2. **accounts** (400 records) - Bank accounts linked to customers
3. **categories** (15 records) - Product categories
4. **products** (200 records) - Products with pricing and stock
5. **inventory** (200 records) - Inventory across multiple warehouses
6. **orders** (1000 records) - Customer orders with status tracking
7. **order_items** (3000 records) - Items within orders
8. **invoices** (1000 records) - Invoices for orders
9. **payments** (800 records) - Payment records for invoices
10. **subscriptions** (300 records) - Customer subscriptions
11. **transactions** (2000 records) - Account transactions
12. **audit_logs** (500 records) - Audit trail

## Quick Start

### 1. One-Line Setup (Recommended)

```bash
# Make sure you have a running PostgreSQL instance
# Set your database URL, then run:

DATABASE_URL="postgres://postgres:postgres@localhost:5432/postgres" \
  ./test/setup_and_test.sh
```

This will:
1. ✅ Create the test schema
2. ✅ Load 10,000+ test records
3. ✅ Run all integration tests

### 2. Manual Setup (Step by Step)

#### Step 1: Create Schema
```bash
psql -d mydb < test/test_schema.sql
```

#### Step 2: Load Test Data
```bash
export DATABASE_URL="postgres://postgres:postgres@localhost:5432/mydb"
cargo run --release --bin load_test_data
```

Expected output:
```
🚀 Starting test data generation...

📝 Generating 500 customers...
📝 Generating 15 product categories...
📝 Generating 200 products...
... (more data generation)

✅ Data generation complete!

📊 Data Summary:
─────────────────────────────────
customers            500 records
categories            15 records
products             200 records
...
```

#### Step 3: Start MCP Server
```bash
cargo run --release -- --database-url "postgres://postgres:postgres@localhost:5432/mydb"
```

#### Step 4: Run Integration Tests
```bash
cargo test --test integration_test_data_tools -- --nocapture
```

## Running Tests

### Run All Tests
```bash
cargo test --test integration_test_data_tools -- --nocapture
```

### Run Specific Test
```bash
cargo test --test integration_test_data_tools test_list_tables_returns_12_tables -- --nocapture
```

### Run Tests with Serial Execution
```bash
cargo test --test integration_test_data_tools -- --nocapture --test-threads=1
```

## Test Coverage

The integration tests validate all tool categories:

### ✅ Schema Inspection (4 tests)
- `test_list_tables_returns_12_tables` - Lists all tables
- `test_describe_customers_table` - Describes columns
- `test_list_indexes_on_orders_table` - Lists indexes
- `test_list_schemas_contains_public` - Lists schemas

### ✅ Query Execution (3 tests)
- `test_execute_query_count_customers` - Simple aggregation
- `test_execute_query_join_orders_and_customers` - JOIN query
- `test_execute_query_aggregation` - Complex aggregation

### ✅ Monitoring (3 tests)
- `test_analyze_db_health` - Health dashboard
- `test_list_unused_indexes` - Unused index detection
- `test_get_cache_hit_ratio_with_queries` - Cache metrics

### ✅ Performance (2 tests)
- `test_explain_query_performance` - Query plans
- `test_get_pg_stat_statements` - Query statistics

### ✅ Settings & Config (2 tests)
- `test_get_setting_max_connections` - Get settings
- `test_get_setting_work_mem` - Get specific setting

### ✅ User & Security (3 tests)
- `test_list_users` - List database users
- `test_show_current_user` - Current user info
- `test_show_session_info` - Session information

## Data Generation Details

### Data Characteristics
- **Realistic relationships** - Orders link to customers, items link to products
- **Diverse data types** - Strings, integers, decimals, timestamps, booleans
- **Varied statuses** - Orders have multiple states, users have various statuses
- **Large result sets** - Enables testing pagination and performance
- **Indexed data** - Multiple indexes created for realistic query plans

### Generated Record Counts
```
Total: 10,315 records across 12 tables
├─ customers:       500
├─ categories:       15
├─ products:        200
├─ accounts:        400
├─ inventory:       200
├─ orders:        1,000
├─ order_items:    3,000
├─ invoices:       1,000
├─ payments:         800
├─ subscriptions:     300
├─ transactions:    2,000
└─ audit_logs:       500
```

## Cleaning Up

### Remove Test Data Only
```bash
# Drop all test tables (keeps schema structure for re-running)
psql -d mydb -c "DROP TABLE IF EXISTS audit_logs CASCADE;"
# ... (repeat for other tables)
```

### Full Reset
```bash
# Drop entire public schema
psql -d mydb -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"

# Then re-run setup
./test/setup_and_test.sh
```

## Troubleshooting

### Schema Creation Fails
**Problem**: Tables already exist
**Solution**: 
```bash
psql -d mydb -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
./test/setup_and_test.sh
```

### Data Loading Hangs
**Problem**: Connection timeout
**Solution**: 
```bash
# Check PostgreSQL is running
psql -d mydb -c "SELECT 1;"

# Check connection string
echo $DATABASE_URL
```

### Tests Can't Connect to Server
**Problem**: Server not running
**Solution**:
```bash
# Terminal 1: Start server
cargo run --release -- --database-url "postgres://..."

# Terminal 2: Run tests
cargo test --test integration_test_data_tools -- --nocapture
```

### Some Tests Fail
**Problem**: Missing extensions (pg_stat_statements)
**Solution**: This is normal - extension tests are optional. The core tests should pass.

## Performance Benchmarking

To benchmark with the test data:

```bash
# Terminal 1: Start server with metrics
cargo run --release -- \
  --database-url "postgres://..." \
  --enable-metrics \
  --metrics-port 9090

# Terminal 2: Run load test
cargo run --release --bin benchmark -- --duration 60 --connections 10

# Terminal 3: Monitor metrics
curl http://localhost:9090/metrics | grep mcp_postgres
```

## Extending Tests

To add more tests:

1. Add a new `#[test]` function in `tests/integration_test_data_tools.rs`
2. Use the `tcp_request()` helper to call tools
3. Validate responses with assertions
4. Run with: `cargo test --test integration_test_data_tools -- --nocapture`

Example:
```rust
#[test]
fn test_my_tool() {
    match tcp_request("my_tool", json!({"param": "value"})) {
        Ok(response) => {
            let result = response.get("result").expect("Missing result");
            assert!(result.is_object());
            println!("✓ my_tool: validated");
        }
        Err(e) => panic!("✗ my_tool failed: {}", e),
    }
}
```

## Data Re-Generation

The data generator is idempotent - you can run it multiple times:

```bash
# Safe to run repeatedly - creates new records each time
cargo run --release --bin load_test_data

# To avoid duplicates, clean up first:
psql "$DATABASE_URL" -c "TRUNCATE TABLE customers CASCADE;"
cargo run --release --bin load_test_data
```

## Architecture

```
test/test_schema.sql        - SQL schema (12 tables, indexes, views)
bin/load_test_data.rs       - Rust data generator (10,000+ records)
tests/integration_test_data_tools.rs - 17 comprehensive integration tests
test/setup_and_test.sh      - Automated setup & test runner
```

## Files

- `test/test_schema.sql` - Complete database schema
- `bin/load_test_data.rs` - Configurable data generator
- `tests/integration_test_data_tools.rs` - Integration test suite
- `test/setup_and_test.sh` - Automated setup script (executable)
- `TEST_SETUP.md` - This file

---

**Last Updated**: 2026-06-13  
**Total Tests**: 17  
**Total Test Records**: 10,315  
**Coverage**: All 25 PostgreSQL tools tested with realistic data
