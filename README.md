# mcp-postgres

High-performance MCP (Model Context Protocol) server for PostgreSQL, written in pure Rust with async/await.

## Overview

**mcp-postgres** brings your PostgreSQL database into Claude and other MCP-compatible AI tools. Execute queries, manage schema, monitor performance, and handle bulk operations—all through a clean JSON-RPC interface.

- **76 PostgreSQL tools** — query execution, schema inspection, DDL operations, batch operations, monitoring, maintenance, replication, transactions, and more
- **PostgreSQL documentation-compliant** — all queries verified against official PG docs (v16-18). Uses correct view/column names across PG versions with graceful fallbacks
- **Dual-protocol transport** — TCP (port 3000) and HTTP/2 (port 3001) for flexibility
- **Sub-10ms latency** — optimized for interactive AI workflows
- **Production-grade** — connection pooling, health checks, input validation, SQL injection prevention
- **Stateless HTTP** — each request is independent (no transaction state across requests)
- **Claude Desktop ready** — works with stdio transport for seamless integration

## Quick Start

### Installation

**See [INSTALLATION.md](./guides/INSTALLATION.md) for complete instructions** covering:
- crates.io, source build, and Homebrew installation
- Configuration and verification
- Claude Desktop setup
- Troubleshooting

Quick start:
```bash
# From crates.io (easiest)
cargo install mcp-postgres

# Or from Homebrew (macOS)
brew tap corporatepiyush/mcp-postgres
brew install mcp-postgres
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

See [guides/INSTALLATION.md](./guides/INSTALLATION.md) for complete setup and troubleshooting.

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

## Tools Reference (76 Total)

### Query Execution (8 tools)
`execute_query`, `execute_insert`, `execute_update`, `execute_delete`, `explain_query`, `async_execute_insert`, `async_execute_update`, `async_execute_delete`

### Schema Inspection (8 tools)
`list_tables`, `describe_table`, `list_schemas`, `list_indexes`, `list_triggers`, `show_constraints`, `list_partitions`, `get_object_details`

### DDL Operations (15 tools)
`create_table`, `drop_table`, `create_view`, `drop_view`, `alter_view`, `create_schema`, `drop_schema`, `create_sequence`, `drop_sequence`, `create_index`, `drop_index`, `alter_index`, `create_partition`, `drop_partition`, `backup_table`

### Batch Operations (4 tools)
`async_batch_insert`, `async_batch_update`, `async_batch_delete`, `async_batch_insert_copy`

### Database Monitoring (10 tools)
`get_table_stats`, `get_index_stats`, `show_database_size`, `show_table_size`, `get_cache_hit_ratio`, `analyze_table`, `vacuum_analyze`, `reindex_table`, `get_pg_stat_statements`, `reset_statistics`

### Connection Management (4 tools)
`list_connections`, `show_current_user`, `show_running_queries`, `show_connection_summary`

### Security & Users (5 tools)
`list_users`, `list_user_privileges`, `list_role_memberships`, `list_database_privileges`, `show_session_info`

### Configuration (5 tools)
`show_all_settings`, `get_setting`, `show_memory_settings`, `show_performance_settings`, `show_log_settings`

### Transaction Monitoring (7 tools)
`show_active_transactions`, `show_locks`, `show_waiting_locks`, `show_transaction_isolation`, `show_deadlocks`, `show_autocommit_status`, `show_transaction_timeout`

### Replication (5 tools)
`show_replication_status`, `list_replication_slots`, `list_standby_servers`, `show_wal_info`, `show_base_backup_progress`

### Database Health (4 tools)
`analyze_db_health`, `list_unused_indexes`, `list_duplicate_indexes`, `show_vacuum_progress`

### Maintenance (1 tool)
`truncate_table`

---

## Version 3.0.0 Highlights

- **PostgreSQL Documentation Audit**: All SQL queries verified against official PostgreSQL documentation (v16–18). Fixed 4 bugs found during audit:
  - `show_autocommit_status`: removed dead `SHOW autocommit` call (GUC removed in PG 7.4)
  - `show_deadlocks`: replaced unreliable `state='disabled'` filter with `pg_blocking_pids()` for accurate blocked-process detection
  - `analyze_db_health`/`show_vacuum_progress`: fixed nonexistent `max_dead_tuple_index_pages` column; added PG version-aware fallback for `max_dead_tuple_bytes` vs `max_dead_tuples`
  - `show_base_backup_progress`: fixed wrong view name `pg_stat_basebackup` (never existed); corrected to `pg_stat_progress_basebackup` (PG 13+)
- **Security Hardening** (v2.1.1): SQL injection prevention, SET LOCAL isolation, structured predicates
- **76 tools with integration tests** — coverage for all tool categories

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
           │   (76 tools)            │
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
