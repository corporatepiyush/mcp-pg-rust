# mcp-pg-rust

High-performance MCP (Model Context Protocol) server for PostgreSQL, written in Rust.

## Features

- **59 database tools** — schema inspection, queries, monitoring, maintenance, security, replication, transactions, batch operations, health analysis
- **Lock-free connection pool** — high throughput with minimal contention
- **Dual transport** — TCP (HTTP-like) and stdio (Claude Desktop compatible)
- **Thread-local metrics** — zero-allocation sharded counters (no lock contention)
- **Data-oriented design** — cache-line aligned hot data, no false sharing
- **~20,000 req/s** — with 10 concurrent clients under realistic workload
- **Restricted mode** — `--access-mode=restricted` for read-only operation, blocking all write tools at dispatch level
- **PG 18 compatible** — works with PostgreSQL 15–18, tested on PG 18

## Quick Start

```bash
# Install from source
cargo install mcp-postgres

# Run with TCP transport (default)
mcp-postgres --database-url "host=127.0.0.1 dbname=mydb"

# Run in stdio mode for Claude Desktop / MCP clients
mcp-postgres --database-url "host=127.0.0.1 dbname=mydb" --stdio

# Run in restricted (read-only) mode
mcp-postgres --database-url "host=127.0.0.1 dbname=mydb" --stdio --access-mode restricted
```

### Usage

```
Usage: mcp-postgres [OPTIONS]

Options:
  -d, --database-url <URL>       PostgreSQL connection string
  -H, --host <HOST>              Server host [default: 127.0.0.1]
  -p, --port <PORT>              Server port [default: 3000]
      --min-connections <N>      Minimum pool connections [default: 5]
      --max-connections <N>      Maximum pool connections [default: 20]
      --log-level <LEVEL>        Log level [default: info]
      --enable-metrics           Enable Prometheus /metrics endpoint
      --metrics-port <PORT>      Metrics port [default: 9090]
      --stdio                    Run in stdio mode (for Claude Desktop)
      --access-mode <MODE>       Access mode: unrestricted or restricted [default: unrestricted]
  -h, --help                     Print help
  -V, --version                  Print version
```

### Claude Desktop Configuration

Add to your `claude_desktop_config.json`:

```jsonc
{
  "mcpServers": {
    "postgres": {
      "command": "mcp-postgres",
      "args": ["--database-url", "host=127.0.0.1 dbname=mydb", "--stdio"]
    }
  }
}
```

## Tools Reference

All tools follow the [MCP JSON-RPC 2.0](https://spec.modelcontextprotocol.io) specification.

### Request Format

```jsonc
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "<tool_name>",
    "arguments": { ... }
  },
  "id": 1
}
```

---

### Schema Inspection

#### `list_tables`
List all user tables with schema and type.

```jsonc
// Request:  {}
// Response: { "tables": [{ "schema": "public", "name": "users", "type": "BASE TABLE" }, ...] }
```

#### `describe_table`
Describe a table's columns, types, nullability, and defaults.

| Param | Type | Required |
|-------|------|----------|
| `table` | string | yes |

```jsonc
// Request:  { "table": "users" }
// Response: { "columns": [{ "name": "id", "type": "bigint", "nullable": "NO", "default": "nextval(...)", "position": 1 }, ...] }
```

#### `get_object_details`
Rich schema introspection for a single table — columns, constraints, indexes, foreign keys, descriptions, and size.

| Param | Type | Required | Default |
|-------|------|----------|---------|
| `table` | string | yes | — |
| `schema` | string | no | `"public"` |

```jsonc
// Request:  { "table": "users", "schema": "public" }
// Response: { "table": "users", "schema": "public", "size": "256 MB", "row_estimate": 10000,
//   "columns": [...], "indexes": [...], "constraints": [...], "foreign_keys": [...],
//   "description": "Main user accounts table" }
```

#### `list_indexes`
List all indexes with their definitions.

```jsonc
// Request:  {}
// Response: { "indexes": [{ "schema": "public", "table": "users", "name": "users_pkey", "definition": "CREATE INDEX ..." }, ...] }
```

#### `list_schemas`
List all non-system schemas.

```jsonc
// Request:  {}
// Response: { "schemas": [{ "name": "public", "owner": "postgres" }, ...] }
```

#### `show_constraints`
List all table constraints.

```jsonc
// Request:  {}
// Response: { "constraints": [{ "schema": "public", "table": "users", "name": "users_pkey", "type": "PRIMARY KEY" }, ...] }
```

---

### Query Execution

#### `execute_query`
Execute a SELECT query and return rows as arrays.

| Param | Type | Required |
|-------|------|----------|
| `sql` | string | yes |

```jsonc
// Request:  { "sql": "SELECT id, name FROM users LIMIT 2" }
// Response: { "rows": [[1, "Alice"], [2, "Bob"]] }
```

#### `execute_insert`
Execute an INSERT and return rows affected.

| Param | Type | Required |
|-------|------|----------|
| `sql` | string | yes |

```jsonc
// Request:  { "sql": "INSERT INTO users (name) VALUES ('Charlie')" }
// Response: { "rows_affected": 1 }
```

#### `execute_update`
Execute an UPDATE and return rows affected.

| Param | Type | Required |
|-------|------|----------|
| `sql` | string | yes |

```jsonc
// Request:  { "sql": "UPDATE users SET name = 'Charlie' WHERE id = 3" }
// Response: { "rows_affected": 1 }
```

#### `execute_delete`
Execute a DELETE and return rows affected.

| Param | Type | Required |
|-------|------|----------|
| `sql` | string | yes |

```jsonc
// Request:  { "sql": "DELETE FROM users WHERE id = 3" }
// Response: { "rows_affected": 1 }
```

#### `explain_query`
Show the execution plan for a query with configurable options.

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `sql` | string | yes | — | Query to explain |
| `analyze` | boolean | no | false | Execute the query (EXPLAIN ANALYZE) |
| `buffers` | boolean | no | false | Show buffer usage |
| `format` | string | no | `"json"` | Output format: json, yaml, text |

```jsonc
// Request:  { "sql": "SELECT * FROM users WHERE id = 1", "analyze": true, "buffers": true, "format": "json" }
// Response: { "plan": [ /* PostgreSQL EXPLAIN JSON tree */ ], "options": { "analyze": true, "buffers": true, "format": "json" } }
```

---

### Batch Operations (High-Performance Bulk DML)

#### `batch_insert`
High-performance multi-row insert. Temporarily disables `synchronous_commit` for maximum throughput.

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `table` | string | yes | Target table |
| `columns` | string[] | yes | Column names |
| `rows` | array[] | yes | Array of value arrays |
| `returning` | string | no | Column to return (e.g. `"id"`) |

```jsonc
// Request:  { "table": "users", "columns": ["name", "email"], "rows": [["Alice", "a@x.com"], ["Bob", "b@x.com"]] }
// Response: { "rows_affected": 2 }

// With RETURNING:
// Request:  { "table": "users", "columns": ["name"], "rows": [["Charlie"]], "returning": "id" }
// Response: { "rows_affected": 1, "inserted_ids": [42] }
```

#### `batch_insert_copy`
Batch insert with configurable batch size for massive bulk loads.

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `table` | string | yes | — | Target table |
| `columns` | string[] | yes | — | Column names |
| `rows` | array[] | yes | — | Array of value arrays |
| `batch_size` | integer | no | 1000 | Rows per INSERT statement |

```jsonc
// Request:  { "table": "users", "columns": ["name"], "rows": [["a"], ["b"], ... 5000 rows], "batch_size": 1000 }
// Response: { "rows_affected": 5000, "batches": 5 }
```

#### `batch_update`
Bulk update with multiple WHERE clauses (each clause applied independently).

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `table` | string | yes | Target table |
| `updates` | object | yes | Column → value mappings |
| `where_clauses` | string[] | yes | Array of WHERE conditions |

```jsonc
// Request:  { "table": "users", "updates": { "status": "inactive" }, "where_clauses": ["id = 1", "id = 2"] }
// Response: { "rows_affected": 2 }
```

#### `batch_delete`
Bulk deletion with OR-combined WHERE clauses.

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `table` | string | yes | Target table |
| `where_clauses` | string[] | yes | OR-combined conditions |
| `returning` | string | no | Column to return |

```jsonc
// Request:  { "table": "users", "where_clauses": ["id = 1", "id = 2"] }
// Response: { "rows_affected": 2 }
```

---

### Monitoring

#### `analyze_db_health`
Unified database health dashboard — buffer cache hit ratio, connection utilization, unused/duplicate indexes, vacuum progress, tables needing vacuum, and tables with excessive sequential scans.

```jsonc
// Request:  {}
// Response: {
//   "buffer_cache": { "hit_ratio_pct": 99.5, "status": "healthy" },
//   "connections": { "active": 3, "waiting": 1, "idle_in_transaction": 0, "max": 100, "utilization_pct": 4.0, "status": "healthy" },
//   "indexes": { "unused": [...], "duplicate_candidates": [...], "total_unused": 0 },
//   "vacuum": { "in_progress": [], "tables_needing_vacuum": [] },
//   "performance": { "tables_with_high_seq_scans": [] }
// }
```

#### `list_unused_indexes`
List all indexes with zero scans — candidates for removal to reduce write overhead.

```jsonc
// Request:  {}
// Response: { "unused_indexes": [{ "schema": "public", "table": "users", "index": "users_email_idx", "scans": 0, "tuples_read": 0, "tuples_fetched": 0 }], "count": 0 }
```

#### `list_duplicate_indexes`
Identify potentially duplicate or overlapping indexes.

```jsonc
// Request:  {}
// Response: { "duplicate_indexes": [{ "schema": "public", "table": "users", "index": "users_name_idx", "duplicate_of": "users_name_idx2", "size": "64 MB" }], "count": 0 }
```

#### `show_vacuum_progress`
Real-time VACUUM operation monitoring.

```jsonc
// Request:  {}
// Response: { "vacuum_in_progress": false, "message": "No VACUUM operations currently in progress" }
// Response (active): { "vacuum_in_progress": true, "operations": [{ "schema": "public", "table": "users", "phase": "scanning heap", "blocks_total": 1000, "blocks_scanned": 500, "blocks_vacuumed": 200, "blocks_remaining": 500, "progress_pct": 50.0, "index_vacuum_count": 2, "max_dead_tuple_bytes": 1048576 }] }
```

#### `get_table_stats`
Live row counts, dead tuples, and vacuum history from `pg_stat_user_tables`.

```jsonc
// Request:  {}
// Response: { "tables": [{ "schema": "public", "table": "users", "live_tuples": 1000, "dead_tuples": 5, "last_vacuum": null, "last_autovacuum": "..." }, ...] }
```

#### `get_index_stats`
Index scan and tuple read statistics.

```jsonc
// Request:  {}
// Response: { "indexes": [{ "schema": "public", "table": "users", "index": "users_pkey", "scans": 42, "tuples_read": 100, "tuples_fetched": 90 }, ...] }
```

#### `show_database_size`
Size of each database.

```jsonc
// Request:  {}
// Response: { "databases": [{ "name": "mydb", "size": "12 GB", "size_bytes": 12884901888 }, ...] }
```

#### `show_table_size`
Total size of each user table (including indexes and TOAST).

```jsonc
// Request:  {}
// Response: { "tables": [{ "schema": "public", "table": "users", "size": "256 MB", "size_bytes": 268435456 }, ...] }
```

#### `get_cache_hit_ratio`
Buffer cache hit ratio from `pg_statio_user_tables`.

```jsonc
// Request:  {}
// Response: { "cache_hit_ratio": 0.99, "percentage": 99.0 }
```

---

### Connection Management

#### `list_connections`
List all active connections (excluding self).

```jsonc
// Request:  {}
// Response: { "connections": [{ "pid": 12345, "user": "postgres", "application": "psql", "state": "active", "state_change": "2026-06-13 10:00:00", "backend_start": "2026-06-13 09:00:00", "query_start": "2026-06-13 10:00:00" }, ...] }
```

#### `kill_connection`
Terminate a specific connection by PID.

| Param | Type | Required |
|-------|------|----------|
| `pid` | integer | yes |

```jsonc
// Request:  { "pid": 12345 }
// Response: { "pid": 12345, "terminated": true }
```

#### `show_current_user`
Show current user, database, and PostgreSQL version.

```jsonc
// Request:  {}
// Response: { "user": "postgres", "database": "mydb", "version": "PostgreSQL 16.4 on ..." }
```

#### `show_running_queries`
Show all non-idle queries.

```jsonc
// Request:  {}
// Response: { "queries": [{ "pid": 12345, "user": "postgres", "application": "psql", "state": "active", "query": "SELECT ...", "query_start": "..." }, ...] }
```

#### `show_connection_summary`
Aggregate connection counts by state.

```jsonc
// Request:  {}
// Response: { "summary": [{ "state": "active", "count": 3 }, { "state": "idle", "count": 7 }] }
```

---

### Maintenance

#### `vacuum_analyze`
Run VACUUM ANALYZE on a specific table or the entire database.

| Param | Type | Required |
|-------|------|----------|
| `table` | string | no (omitting vacuums entire DB) |

```jsonc
// Request:  { "table": "users" }
// Response: { "status": "success", "action": "VACUUM ANALYZE", "table": "users" }
```

#### `analyze_table`
Update table statistics.

| Param | Type | Required |
|-------|------|----------|
| `table` | string | yes |

```jsonc
// Request:  { "table": "users" }
// Response: { "status": "success", "action": "ANALYZE", "table": "users" }
```

#### `reindex_table`
Rebuild all indexes on a table.

| Param | Type | Required |
|-------|------|----------|
| `table` | string | yes |

```jsonc
// Request:  { "table": "users" }
// Response: { "status": "success", "action": "REINDEX", "table": "users" }
```

#### `get_pg_stat_statements`
Top 50 queries by total execution time (requires `pg_stat_statements` extension).

```jsonc
// Request:  {}
// Response: { "statements": [{ "query": "SELECT * FROM users WHERE id = $1", "calls": 100, "mean_time_ms": 0.5, "max_time_ms": 2.0, "total_time_ms": 50.0 }, ...] }
```

#### `reset_statistics`
Reset all PostgreSQL statistics counters.

```jsonc
// Request:  {}
// Response: { "status": "success", "action": "reset_statistics", "message": "All statistics counters have been reset" }
```

---

### Security

#### `list_users`
List all database users and their attributes.

```jsonc
// Request:  {}
// Response: { "users": [{ "username": "postgres", "superuser": true, "createdb": true, "canlogin": true, "valid_until": null }, ...] }
```

#### `list_user_privileges`
List table-level privileges for a specific user.

| Param | Type | Required |
|-------|------|----------|
| `username` | string | yes |

```jsonc
// Request:  { "username": "alice" }
// Response: { "privileges": [{ "grantee": "alice", "schema": "public", "table": "users", "privilege": "SELECT" }, ...] }
```

#### `list_role_memberships`
List role-to-role memberships.

```jsonc
// Request:  {}
// Response: { "memberships": [{ "member": "alice", "role": "readonly", "admin": false }, ...] }
```

#### `list_database_privileges`
List ACLs for all non-template databases.

```jsonc
// Request:  {}
// Response: { "databases": [{ "database": "mydb", "acl": "postgres=C*T*/postgres+..." }, ...] }
```

#### `show_session_info`
Current session's client/server address and port.

```jsonc
// Request:  {}
// Response: { "current_user": "postgres", "current_database": "mydb", "client_address": "127.0.0.1", "client_port": 54321, "server_address": "127.0.0.1", "server_port": 5432 }
```

---

### Configuration

#### `show_all_settings`
List all non-internal PostgreSQL settings.

```jsonc
// Request:  {}
// Response: { "settings": [{ "name": "checkpoint_timeout", "value": "300", "unit": "s", "description": "Sets maximum time between automatic WAL checkpoints", "context": "sighup" }, ...] }
```

#### `get_setting`
Get a specific PostgreSQL setting with full metadata.

| Param | Type | Required |
|-------|------|----------|
| `setting` | string | yes |

```jsonc
// Request:  { "setting": "work_mem" }
// Response: { "name": "work_mem", "value": "4096", "unit": "kB", "description": "Sets the maximum memory to be used for query workspaces", "context": "user", "type": "integer", "source": "default" }
```

#### `show_memory_settings`
Key memory configuration settings.

```jsonc
// Request:  {}
// Response: { "memory_settings": [{ "name": "shared_buffers", "value": "128", "unit": "MB" }, { "name": "work_mem", "value": "4096", "unit": "kB" }, ...] }
```

#### `show_performance_settings`
Performance-related settings.

```jsonc
// Request:  {}
// Response: { "performance_settings": [{ "name": "max_connections", "value": "100" }, { "name": "synchronous_commit", "value": "on" }, ...] }
```

#### `show_log_settings`
All logging-related settings.

```jsonc
// Request:  {}
// Response: { "log_settings": [{ "name": "log_min_duration_statement", "value": "-1", "unit": "ms" }, ...] }
```

---

### Replication

#### `show_replication_status`
WAL replay status and uptime.

```jsonc
// Request:  {}
// Response: { "is_wal_replay_paused": false, "last_wal_receive_lsn": "0/1234567", "last_wal_replay_lsn": "0/1234567", "uptime": "02:15:30" }
```

#### `list_replication_slots`
List all replication slots.

```jsonc
// Request:  {}
// Response: { "replication_slots": [{ "slot_name": "slot1", "slot_type": "physical", "database": null, "active": true, "restart_lsn": "0/1234567", "confirmed_flush_lsn": null }, ...] }
```

#### `list_standby_servers`
List connected standby servers with replication lag.

```jsonc
// Request:  {}
// Response: { "standbys": [{ "client_address": "10.0.0.2", "client_port": 5432, "state": "streaming", "sync_state": "sync", "write_lag": null, "flush_lag": null, "replay_lag": null }, ...] }
```

#### `show_wal_info`
Current WAL position and size.

```jsonc
// Request:  {}
// Response: { "current_wal_lsn": "0/1234567", "current_wal_insert_lsn": "0/1234567", "wal_replay_paused": false, "wal_size_bytes": 123456789 }
```

#### `show_base_backup_progress`
Show base backup progress (PG 17+).

```jsonc
// Request:  {}
// Response: { "phase": "streaming database files", "backup_total": 1000000000, "backup_streamed": 500000000, "tablespaces_total": 1, "tablespaces_streamed": 1 }
```

---

### Transactions

#### `show_active_transactions`
Show all transactions in progress.

```jsonc
// Request:  {}
// Response: { "transactions": [{ "pid": 12345, "user": "postgres", "application": "psql", "state": "active", "xact_start": "2026-06-13 10:00:00", "query_start": "2026-06-13 10:00:00", "query": "UPDATE ..." }, ...] }
```

#### `show_locks`
Show all locks with their holders and queries.

```jsonc
// Request:  {}
// Response: { "locks": [{ "pid": 12345, "user": "postgres", "application": "psql", "lock_type": "ExclusiveLock", "granted": true, "fastpath": false, "query_start": "2026-06-13 10:00:00", "query": "UPDATE ..." }, ...] }
```

#### `show_waiting_locks`
Show all locks that are waiting (not granted).

```jsonc
// Request:  {}
// Response: { "waiting_locks": [{ "pid": 12345, "user": "postgres", "lock_type": "ExclusiveLock", "query_start": "2026-06-13 10:00:00", "query": "UPDATE ..." }, ...] }
```

#### `begin_transaction`
Begin a new transaction with optional isolation level.

| Param | Type | Required | Default |
|-------|------|----------|---------|
| `isolation_level` | string | no | `"READ COMMITTED"` |

Valid levels: `SERIALIZABLE`, `REPEATABLE READ`, `READ COMMITTED`, `READ UNCOMMITTED`.

```jsonc
// Request:  { "isolation_level": "SERIALIZABLE" }
// Response: { "status": "success", "action": "BEGIN", "isolation_level": "SERIALIZABLE" }
```

#### `commit_transaction`
Commit the current transaction.

```jsonc
// Request:  {}
// Response: { "status": "success", "action": "COMMIT" }
```

#### `rollback_transaction`
Roll back the current transaction.

```jsonc
// Request:  {}
// Response: { "status": "success", "action": "ROLLBACK" }
```

#### `show_transaction_isolation`
Show current transaction isolation level.

```jsonc
// Request:  {}
// Response: { "isolation_level": "read committed", "available_levels": ["serializable", "repeatable read", "read committed", "read uncommitted"] }
```

#### `show_deadlocks`
Detect potential deadlock situations.

```jsonc
// Request:  {}
// Response: { "potential_deadlocks": [{ "pid": 12345, "user": "postgres", "application": "psql", "state": "active", "query_start": "2026-06-13 10:00:00", "query": "UPDATE ..." }, ...] }
```

#### `show_autocommit_status`
Show whether autocommit is enabled.

```jsonc
// Request:  {}
// Response: { "autocommit": true, "value": "on" }
```

#### `show_transaction_timeout`
Show current `statement_timeout` setting.

```jsonc
// Request:  {}
// Response: { "statement_timeout": "30s" }
```

---

## Architecture

```
                   ┌──────────────────┐
                   │   MCP Client      │
                   │ (Claude Desktop)  │
                   └────────┬─────────┘
                            │
              ┌─────────────┴─────────────┐
              │       stdio / TCP         │
              │    (JSON-RPC 2.0)         │
              └─────────────┬─────────────┘
                            │
                   ┌────────┴────────┐
                   │   MCPServer      │
                   │  (tokio/TCP)     │
                   │  (tokio/stdio)   │
                   └────────┬────────┘
                            │
                   ┌────────┴────────┐
                   │  ConnectionPool  │
                   │ (lock-free       │
                   │  SegQueue)       │
                   │  ┌──┐ ┌──┐ ┌──┐ │
                   │  │C1│ │C2│ │C3│ │
                   │  └──┘ └──┘ └──┘ │
                   └────────┬────────┘
                            │
                   ┌────────┴────────┐
                   │   PostgreSQL     │
                   └─────────────────┘
```

### Performance Design

- **Hot/cold data separation** — pool configuration sits on its own cache line, away from the frequently-accessed idle connection queue
- **Thread-local sharded metrics** — request counting uses per-CPU `AtomicU64` shards instead of a synchronized queue (single `fetch_add(Relaxed)` per request)
- **Lock-free connection pool** — `crossbeam::SegQueue` with no mutex contention
- **Mimalloc** — fast allocation/deallocation with tuned page reset and eager commit
- **Fat LTO + panic=abort** — release profile optimizes aggressively

## Benchmark

```bash
# Terminal 1: Start server
mcp-postgres --database-url "host=127.0.0.1 dbname=mydb" --log-level error &

# Terminal 2: Run benchmark (10 concurrent clients, 10 seconds)
cargo run --release --bin benchmark
```

```
=== Results ===
Concurrency: 10
Duration: 10.0s
Total Requests: 208,333
Requests/sec: ~20,800
Avg Latency: ~48µs
```

## Test Suite

```bash
# Unit tests (63 tests, no DB required)
cargo test

# Integration tests (16 tests, requires running server)
cargo test -- --ignored

# Full suite
# Terminal 1: mcp-postgres --log-level error &
# Terminal 2: cargo test -- --ignored
```

## Development

```bash
# Clone and build
git clone https://github.com/corporatepiyush/mcp-pg-rust.git
cd mcp-pg-rust
cargo build --release

# Test schema (optional)
psql -d mydb -f test/schema.sql
```

## License

Apache-2.0
