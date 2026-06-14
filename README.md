# mcp-postgres

High-performance MCP (Model Context Protocol) server for PostgreSQL, written in pure Rust with async/await.

## Overview

**mcp-postgres** brings your PostgreSQL database into Claude and other MCP-compatible AI tools. Execute queries, manage schema, monitor performance, and handle bulk operations—all through a clean JSON-RPC interface.

- **46 PostgreSQL tools** — query execution, schema inspection, DDL operations, batch operations, monitoring, maintenance, replication, and more
- **Dual-protocol transport** — TCP (port 3000) and HTTP/2 (port 3001) for flexibility
- **Sub-10ms latency** — optimized for interactive AI workflows
- **Production-grade** — connection pooling, health checks, input validation, SQL injection prevention
- **Stateless HTTP** — each request is independent (no transaction state across requests)
- **Claude Desktop ready** — works with stdio transport for seamless integration

## Quick Start

### Installation

```bash
# From crates.io
cargo install mcp-postgres

# Or build from source
git clone https://github.com/corporatepiyush/mcp-pg-rust.git
cd mcp-postgres
cargo build --release
```

### Run

```bash
# TCP server (default, port 3000)
mcp-postgres --database-url "postgres://user:pass@localhost:5432/mydb"

# HTTP/2 server (port 3001)
mcp-postgres --database-url "postgres://user:pass@localhost:5432/mydb" --http-port 3001

# Stdio mode for Claude Desktop
mcp-postgres --database-url "postgres://user:pass@localhost:5432/mydb" --stdio

# Restricted (read-only) mode
mcp-postgres --database-url "postgres://user:pass@localhost:5432/mydb" --stdio --access-mode restricted
```

### Claude Desktop Configuration

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "postgres": {
      "command": "mcp-postgres",
      "args": [
        "--database-url",
        "postgres://user:pass@localhost:5432/mydb",
        "--stdio"
      ]
    }
  }
}
```

## Command-Line Options

```
Usage: mcp-postgres [OPTIONS]

Options:
  -d, --database-url <URL>       PostgreSQL connection string
  -H, --host <HOST>              Server host (TCP) [default: 127.0.0.1]
  -p, --port <PORT>              TCP server port [default: 3000]
      --http-port <PORT>         HTTP/2 server port [default: 3001]
      --min-connections <N>      Min pool connections [default: 5]
      --max-connections <N>      Max pool connections [default: 20]
      --log-level <LEVEL>        Log level [default: info]
      --enable-metrics           Enable Prometheus /metrics endpoint
      --metrics-port <PORT>      Metrics port [default: 9090]
      --stdio                    Stdio mode (Claude Desktop)
      --access-mode <MODE>       unrestricted or restricted [default: unrestricted]
  -h, --help                     Print help
  -V, --version                  Print version
```

---

## Protocol & API

All tools follow the [MCP JSON-RPC 2.0](https://spec.modelcontextprotocol.io) specification.

### Request Format

```json
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

### Response Format (Success)

```json
{
  "jsonrpc": "2.0",
  "result": { ... },
  "id": 1
}
```

### Response Format (Error)

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32602,
    "message": "Invalid params: Missing 'sql' parameter"
  },
  "id": 1
}
```

---

## Tools Reference (46 Total)

### Query Execution (6 tools)

**`execute_query`** — Execute SELECT and return rows

```json
{ "sql": "SELECT id, name FROM users LIMIT 10" }
→ { "rows": [[1, "Alice"], [2, "Bob"]] }
```

**`execute_insert`** — Execute INSERT and return rows affected

```json
{ "sql": "INSERT INTO users (name) VALUES ('Charlie')" }
→ { "rows_affected": 1 }
```

**`execute_update`** — Execute UPDATE and return rows affected

```json
{ "sql": "UPDATE users SET status='active' WHERE id=1" }
→ { "rows_affected": 1 }
```

**`execute_delete`** — Execute DELETE and return rows affected

```json
{ "sql": "DELETE FROM users WHERE id=1" }
→ { "rows_affected": 1 }
```

**`explain_query`** — Show query execution plan

```json
{
  "sql": "SELECT * FROM users WHERE id = 1",
  "analyze": true,
  "buffers": true,
  "format": "json"
}
→ { "plan": [...], "options": {...} }
```

**`async_*` variants** — High-performance versions with temporary sync commit disabled

```json
{ "sql": "INSERT INTO large_table VALUES (...)" }
// for async_execute_insert, async_execute_update, async_execute_delete
```

---

### Schema Inspection (8 tools)

**`list_tables`** — List all tables in database

```json
{}
→ { "tables": [{"schema": "public", "name": "users", "type": "BASE TABLE"}, ...] }
```

**`list_schemas`** — List all schemas

```json
{}
→ { "schemas": [{"name": "public", "owner": "postgres"}, ...] }
```

**`list_columns`** — List columns in a table

```json
{ "table": "users" }
→ { "columns": [{"name": "id", "type": "bigint", "nullable": "NO"}, ...] }
```

**`list_indexes`** — List all indexes

```json
{}
→ { "indexes": [{"schema": "public", "table": "users", "name": "users_pkey", ...}, ...] }
```

**`list_triggers`** — List triggers on a table

```json
{ "table": "users" }
→ { "triggers": [...] }
```

**`list_views`** — List all views

```json
{}
→ { "views": [{"schema": "public", "name": "active_users", ...}, ...] }
```

**`list_sequences`** — List all sequences

```json
{}
→ { "sequences": [{"schema": "public", "name": "users_id_seq", ...}, ...] }
```

**`describe_table`** — Get detailed table metadata

```json
{ "table": "users" }
→ { "columns": [...], "constraints": [...], "size": "256 MB", ... }
```

---

### DDL Operations (16 tools)

Create, modify, and drop database objects safely.

**`create_table`** — Create a new table

```json
{
  "table": "users",
  "columns": [
    "id SERIAL PRIMARY KEY",
    "name VARCHAR(255) NOT NULL",
    "email VARCHAR(255) UNIQUE"
  ]
}
```

**`drop_table`** — Drop a table

```json
{ "table": "users" }
```

**`create_view`** — Create a view

```json
{
  "view_name": "active_users",
  "query": "SELECT * FROM users WHERE status='active'"
}
```

**`drop_view`** — Drop a view

```json
{ "view_name": "active_users" }
```

**`alter_view`** — Rename a view

```json
{ "view_name": "active_users", "rename_to": "active_accounts" }
```

**`create_schema`** — Create a schema

```json
{ "schema_name": "analytics" }
```

**`drop_schema`** — Drop a schema

```json
{ "schema_name": "analytics" }
```

**`create_index`** — Create an index

```json
{
  "index_name": "idx_users_email",
  "table": "users",
  "columns": ["email"]
}
```

**`drop_index`** — Drop an index

```json
{ "index_name": "idx_users_email" }
```

**`alter_index`** — Rename an index

```json
{ "index_name": "idx_users_email", "rename_to": "idx_email" }
```

**`create_sequence`** — Create a sequence

```json
{ "sequence_name": "app_id_seq", "start": 1000, "increment": 1 }
```

**`drop_sequence`** — Drop a sequence

```json
{ "sequence_name": "app_id_seq" }
```

**`create_partition`** — Create table partition

```json
{
  "table": "orders",
  "partition_name": "orders_2024",
  "partition_type": "RANGE",
  "column": "created_at",
  "values": "FROM ('2024-01-01') TO ('2025-01-01')"
}
```

**`delete_table_partition`** — Drop a partition

```json
{ "partition_name": "orders_2024" }
```

**`list_partitions`** — List partitions on a table

```json
{ "table": "orders" }
→ { "partitions": [...] }
```

**`backup_table`** — Create a backup copy of a table

```json
{ "table": "users" }
→ Creates table: backup_users with all data
```

---

### Batch Operations (4 tools)

High-performance bulk DML. Max 1000 rows per request.

**`async_batch_insert`** — Insert multiple rows

```json
{
  "table": "users",
  "columns": ["name", "email"],
  "rows": [
    ["Alice", "alice@example.com"],
    ["Bob", "bob@example.com"]
  ],
  "returning": "id"
}
→ { "rows_affected": 2, "inserted_ids": [1, 2] }
```

**`async_batch_update`** — Update multiple rows with different conditions

```json
{
  "table": "users",
  "updates": { "status": "inactive" },
  "where_clauses": ["id = 1", "id = 2"]
}
→ { "rows_affected": 2 }
```

**`async_batch_delete`** — Delete multiple rows

```json
{
  "table": "users",
  "where_clauses": ["id = 1", "id = 2"],
  "returning": "id"
}
→ { "rows_affected": 2, "inserted_ids": [1, 2] }
```

**`async_batch_insert_copy`** — Bulk insert with configurable batch size

```json
{
  "table": "events",
  "columns": ["user_id", "event_type"],
  "rows": [[...5000 rows...]],
  "batch_size": 1000
}
→ { "rows_affected": 5000, "batches": 5 }
```

---

### Monitoring & Analysis (6 tools)

**`list_connections`** — Show active database connections

```json
{}
→ { "connections": [{"pid": 12345, "user": "postgres", "state": "active", ...}, ...] }
```

**`show_current_user`** — Show current user and database

```json
{}
→ { "user": "postgres", "database": "mydb", "version": "PostgreSQL 16" }
```

**`analyze_table`** — Update table statistics

```json
{ "table": "users" }
→ { "status": "success", "action": "ANALYZE", "table": "users" }
```

**`vacuum_table`** — Clean dead tuples and optimize

```json
{ "table": "users" }
→ { "status": "success", "action": "VACUUM", "table": "users" }
```

**`get_table_size`** — Get table size in bytes and human-readable format

```json
{ "table": "users" }
→ { "size": "256 MB", "size_bytes": 268435456 }
```

**`get_database_size`** — Get total database size

```json
{}
→ { "size": "2.5 GB", "size_bytes": 2684354560 }
```

---

### Connection & Security (4 tools)

**`show_running_queries`** — Show all non-idle queries

```json
{}
→ { "queries": [{"pid": 12345, "user": "postgres", "query": "SELECT ...", ...}, ...] }
```

**`list_users`** — List all database users

```json
{}
→ { "users": [{"username": "alice", "superuser": false, "canlogin": true}, ...] }
```

**`list_user_privileges`** — List privileges for a user

```json
{ "username": "alice" }
→ { "privileges": [{"schema": "public", "table": "users", "privilege": "SELECT"}, ...] }
```

**`show_session_info`** — Show current session details

```json
{}
→ { "current_user": "postgres", "current_database": "mydb", "client_address": "127.0.0.1", ... }
```

---

### Configuration (2 tools)

**`show_all_settings`** — List all PostgreSQL settings

```json
{}
→ { "settings": [{"name": "work_mem", "value": "4096", "unit": "kB", ...}, ...] }
```

**`get_setting`** — Get a specific setting

```json
{ "setting": "work_mem" }
→ { "name": "work_mem", "value": "4096", "unit": "kB", "description": "...", ... }
```

---

## Architecture

### Dual-Protocol Design

```
┌─────────────────┐         ┌─────────────────┐
│   TCP Client    │         │   HTTP Client   │
│  (port 3000)    │         │  (port 3001)    │
└────────┬────────┘         └────────┬────────┘
         │                           │
         └─────────────┬─────────────┘
                       │
              ┌────────┴────────┐
              │   JSON-RPC 2.0   │
              │  (MCP Protocol)  │
              └────────┬────────┘
                       │
          ┌────────────┴────────────┐
          │   Tool Dispatcher       │
          │   (46 tools)            │
          └────────────┬────────────┘
                       │
        ┌──────────────┴──────────────┐
        │   Connection Pool           │
        │   (deadpool-postgres)       │
        │   Min: 5, Max: 20           │
        └────────────┬────────────────┘
                     │
            ┌────────┴────────┐
            │  PostgreSQL DB  │
            └─────────────────┘
```

### Key Design Decisions

- **Stateless HTTP** — Each request is independent. Transaction tools (BEGIN, COMMIT, ROLLBACK) not available over HTTP.
- **Connection pooling** — Deadpool maintains 5-20 connections with health checks and idle timeouts.
- **Sub-10ms latency** — Optimized for interactive AI workflows. TCP: < 10ms, HTTP: < 10ms (> 20ms is unacceptable).
- **Input validation** — All parameters validated at system boundary:
  - SQL statements: max 10,000 characters
  - Identifiers: max 255 characters
  - Batch rows: max 1,000 per request
  - SQL injection prevention via identifier validation

---

## License

Apache-2.0

## Support

For issues, questions, or tool requests: [GitHub Issues](https://github.com/corporatepiyush/mcp-pg-rust/issues)
