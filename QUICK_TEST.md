# Quick Test Guide

## One-Line Test Everything

```bash
DATABASE_URL="postgres://postgres:postgres@localhost:5432/postgres" \
  ./tests/setup_and_test.sh
```

This creates schema, loads 10,000+ records, and runs 17 integration tests.

## What Gets Created

### 12 Tables with 10,315 Records
- customers (500)
- orders (1000) 
- products (200)
- invoices (1000)
- payments (800)
- transactions (2000)
- ... and 6 more

### 17 Integration Tests
- Schema inspection (list_tables, describe_table, list_indexes, list_schemas)
- Query execution (simple, JOIN, aggregation)
- Monitoring (health analysis, cache hit ratio, unused indexes)
- Performance (EXPLAIN, query statistics)
- Settings (get_setting)
- User/Security (list_users, show_current_user, session info)

## Manual Steps

### 1. Setup
```bash
# Create schema
psql -d mydb < tests/test_schema.sql

# Load data
export DATABASE_URL="postgres://postgres:postgres@localhost:5432/mydb"
cargo run --release --bin load_test_data
```

### 2. Run Server (Terminal 1)
```bash
cargo run --release -- --database-url "postgres://postgres:postgres@localhost:5432/mydb"
```

### 3. Run Tests (Terminal 2)
```bash
cargo test --test integration_test_data_tools -- --nocapture
```

## Reset Data

```bash
# Clean tables and reload
psql mydb -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
./tests/setup_and_test.sh
```

## Test Specific Tool

```bash
cargo test --test integration_test_data_tools test_list_tables -- --nocapture
```

## Key Files

- `tests/test_schema.sql` - Schema definition
- `bin/load_test_data.rs` - Data generator (10K+ records)
- `tests/integration_test_data_tools.rs` - Integration tests (17 tests)
- `tests/setup_and_test.sh` - Automated setup script
- `TEST_SETUP.md` - Full documentation

## Sample Commands

```bash
# Generate data only
DATABASE_URL="postgres://..." cargo run --release --bin load_test_data

# Run one test
cargo test test_execute_query_count_customers -- --nocapture

# Run all tests with output
cargo test --test integration_test_data_tools -- --nocapture --test-threads=1

# Run tests while server is on different port
MCP_PORT=3000 cargo test --test integration_test_data_tools -- --nocapture
```

---

See `TEST_SETUP.md` for detailed documentation.
