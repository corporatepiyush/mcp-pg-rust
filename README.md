# mcp-postgres

[![Crates.io](https://img.shields.io/crates/v/mcp-postgres)](https://crates.io/crates/mcp-postgres)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024-edition-orange)](https://github.com/corporatepiyush/mcp-pg-rust)

**mcp-postgres** is a high-performance MCP server that brings PostgreSQL into Claude Desktop and any MCP-compatible AI tool. 135 PostgreSQL tools, lock-free connection pooling, sub-10ms latency.

## Quick Start

```bash
# Install
cargo install mcp-postgres

# Run (stdio mode for Claude Desktop)
mcp-postgres --database-url "postgres://user:pass@localhost:5432/mydb" --stdio
```

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "postgres": {
      "command": "mcp-postgres",
      "args": ["--database-url", "postgres://user:pass@localhost:5432/mydb", "--stdio"]
    }
  }
}
```

**[Complete setup guide →](./guides/INSTALLATION.md)**

---

## Why mcp-postgres?

| Feature | mcp-postgres | DIY / psql |
|---------|-------------|------------|
| **135 purpose-built tools** | Schema inspection, DDL, monitoring, replication, batch ops, security audit, text search, extensions, maintenance, and more | You build every query from scratch |
| **Lock-free connection pool** | Zero-mutex `crossbeam::ArrayQueue` — pure CAS loops, no kernel overhead | Deadpool or manual `Mutex<VecDeque>` |
| **Dual-protocol** | TCP (3000) + HTTP/2 (3001) + stdio — one binary, three transports | Multiple servers to wire up |
| **Sub-10ms latency** | Allocated for AI interactivity — hot path is allocation-free | Unpredictable |
| **SQL injection prevention** | Every identifier validated, `quote_ident` sanitization, structured predicates | Manual parameterization |
| **PG version-aware** | Queries verified against PG 16–18 docs, graceful fallbacks for version differences | Version-specific failures |

## Command-Line Options

```
Usage: mcp-postgres [OPTIONS]

Options:
  -d, --database-url <URL>       PostgreSQL connection string
  -H, --host <HOST>              TCP server host             [127.0.0.1]
  -p, --port <PORT>              TCP server port             [3000]
      --http-port <PORT>         HTTP/2 server port          [3001]
      --min-connections <N>      Min pool connections        [5]
      --max-connections <N>      Max pool connections        [20]
      --log-level <LEVEL>        Log level                   [info]
      --enable-metrics           Prometheus /metrics endpoint
      --metrics-port <PORT>      Metrics port                [9090]
      --stdio                    Stdio mode (Claude Desktop)
      --access-mode <MODE>       unrestricted, restricted    [unrestricted]
  -h, --help                     Print help
  -V, --version                  Print version
```

---

## Protocol & API

[JSON-RPC 2.0](https://spec.modelcontextprotocol.io) over TCP, HTTP/2, or stdio.

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": { "name": "list_tables", "arguments": {} },
  "id": 1
}
```

---

## Tools (135 Total)

<details>
<summary><b>⚡ Query Execution</b> (8) — execute_query, execute_insert, execute_update, execute_delete, explain_query, async_execute_insert, async_execute_update, async_execute_delete</summary>
Execute raw SQL, parameterized inserts/updates/deletes, explain plan analysis, async variants for long-running operations.
</details>

<details>
<summary><b>🔍 Schema Inspection</b> (8) — list_tables, describe_table, list_schemas, list_indexes, list_triggers, show_constraints, list_partitions, get_object_details</summary>
Introspect database structure: tables, columns, indexes, triggers, constraints, partitions, and object metadata.
</details>

<details>
<summary><b>🏗️ DDL Operations</b> (15) — create/drop table, view, schema, sequence, index, partition, alter_view, backup_table</summary>
Full DDL surface: create and drop database objects, alter views, manage partitions, backup tables.
</details>

<details>
<summary><b>📦 Batch Operations</b> (4) — async_batch_insert, async_batch_update, async_batch_delete, async_batch_insert_copy</summary>
High-throughput batch operations with COPY protocol support for bulk data loading.
</details>

<details>
<summary><b>📊 Database Monitoring</b> (10) — table/index stats, database/table size, cache hit ratio, vacuum, analyze, pg_stat_statements, reset_statistics</summary>
Monitor database performance: size tracking, cache efficiency, query statistics, maintenance operations.
</details>

<details>
<summary><b>🔌 Connection Management</b> (4) — list_connections, show_current_user, show_running_queries, show_connection_summary</summary>
View active connections, running queries, and connection distribution.
</details>

<details>
<summary><b>🔐 Security & Users</b> (5) — list_users, user/role privileges, database privileges, session_info</summary>
Audit users, roles, privileges, and session information.
</details>

<details>
<summary><b>⚙️ Configuration</b> (5) — all_settings, get_setting, memory/performance/log_settings</summary>
View and inspect PostgreSQL configuration settings by category.
</details>

<details>
<summary><b>🔄 Transaction Monitoring</b> (7) — active_transactions, locks, waiting_locks, isolation, deadlocks, autocommit, transaction_timeout</summary>
Monitor transaction state, lock contention, deadlocks, and isolation levels.
</details>

<details>
<summary><b>📋 Replication</b> (5) — replication_status, replication_slots, standby_servers, wal_info, base_backup_progress</summary>
Track replication state, WAL activity, and backup progress.
</details>

<details>
<summary><b>🏥 Database Health</b> (4) — analyze_db_health, unused/duplicate indexes, vacuum_progress</summary>
Health checks: index efficiency, vacuum progress, overall database wellness.
</details>

<details>
<summary><b>🗑️ Maintenance</b> (1) — truncate_table</summary>
Safe table truncation with proper privilege checking.
</details>

<details>
<summary><b>🧠 Index Advisor</b> (1) — suggest_indexes</summary>
Analyze query patterns and suggest optimal indexes.
</details>

<details>
<summary><b>🔬 Performance Audit</b> (2) — audit_performance, analyze_query_performance</summary>
Deep-dive query performance analysis with buffer-level and execution-time breakdowns.
</details>

<details>
<summary><b>🛡️ Security Audit</b> (3) — audit_security, audit_user_permissions, audit_role_hierarchy</summary>
Comprehensive security posture review: permissions, roles, sensitive data exposure.
</details>

<details>
<summary><b>🔎 Text Search</b> (6) — search_vector, levenshtein_search, trigram_search, soundex_search, metaphone_search, full_text_config</summary>
Full-text search with pgvector, fuzzystrmatch, pg_trgm: vector similarity, fuzzy matching, phonetic search.
</details>

<details>
<summary><b>🧩 Extension Management</b> (5) — list_extensions, install_extension, remove_extension, update_extension, extension_details</summary>
Manage PostgreSQL extensions: list, install, upgrade, remove, inspect dependencies.
</details>

<details>
<summary><b>📐 Schema Alter</b> (7) — add/drop/alter_column, add/drop_constraint, set_default, drop_default</summary>
Schema migration tools: add/drop/alter columns, manage constraints and defaults.
</details>

<details>
<summary><b>🕒 Session Management</b> (3) — set_session_setting, show_session_settings, reset_session_setting</summary>
Manage session-level configuration with SET LOCAL isolation for safety.
</details>

<details>
<summary><b>👤 User Management</b> (3) — create_user, alter_user, drop_user</summary>
Create and manage database users with proper privilege validation.
</details>

<details>
<summary><b>💾 Data Tools</b> (10) — export_table, import_table, show_table_data, search_data, compare_data, data_profile, find_duplicates, find_orphans, show_pg_stat_user_indexes, show_table_bloat</summary>
Data exploration: export, import, search, compare, profile, find anomalies, estimate bloat.
</details>

<details>
<summary><b>⚗️ Migration Helpers</b> (7) — generate_migration, apply_migration, rollback_migration, list_migrations, show_migration_status, migration_history, validate_migration</summary>
Database migration workflow: generate, apply, rollback, track, and validate schema changes.
</details>

<details>
<summary><b>📈 Vector Database</b> (1) — search_similarity</summary>
pgvector-powered similarity search for embeddings and vector data.
</details>

<details>
<summary><b>📆 Time-Series</b> (1) — analyze_timescale</summary>
TimescaleDB integration for time-series data analysis and compression management.
</details>

<details>
<summary><b>📂 Data I/O</b> (6) — import_csv, export_csv, import_json, export_json, show_table_data_paginated, bulk_load_from_file</summary>
CSV/JSON import and export with pagination support and bulk file loading.
</details>

---

## Performance

### Lock-Free Connection Pool (v4.0.0)

No mutexes. No semaphores. Just CAS loops.

```
┌─────────────┐     ┌──────────────────┐     ┌─────────────┐
│  Task 1     │     │  crossbeam::     │     │  Task 2     │
│  (acquire)  │────▶│  ArrayQueue      │◀────│  (release)  │
└─────────────┘     │  CachePadded     │     └─────────────┘
                    │  Head/Tail       │
                    │  (CAS only)      │
                    └──────────────────┘
```

| Metric | Deadpool (v3.x) | LockFreePool (v4.0.0) |
|--------|----------------|----------------------|
| Lock acquisitions per acquire+release | 3+ (Mutex + Semaphore) | 0 |
| Allocation on hot path | Yes (VecDeque growth) | Zero (pre-allocated) |
| False sharing | Likely (adjacent fields) | Cache-padded atomics |
| Inlining | Cross-crate, opaque | Monomorphic, LTO-friendly |
| Dependencies | 2 (deadpool + deadpool-postgres) | 0 external (crossbeam already in tree) |

Sub-10ms latency is guaranteed by design: zero allocation on the hot path, monomorphic dispatch, cache-line-isolated atomics, and a single-pointer CAS for connection handoff.

### Architecture

```
┌─────────────────┐         ┌─────────────────┐
│   TCP Client    │         │   HTTP Client   │
│  (port 3000)    │         │  (port 3001)    │
└────────┬────────┘         └────────┬────────┘
         └─────────────┬─────────────┘
                       │
              ┌────────┴────────┐
              │   JSON-RPC 2.0  │
              │  (MCP Protocol) │
              └────────┬────────┘
                       │
          ┌────────────┴────────────┐
          │   Tool Dispatcher       │
          │   (135 tools)           │
          └────────────┬────────────┘
                       │
          ┌────────────┴────────────┐
          │   Connection Pool       │
          │   (lock-free, CAS-only) │
          │   Min: 5, Max: 20       │
          └────────────┬────────────┘
                       │
              ┌────────┴────────┐
              │  PostgreSQL DB  │
              └─────────────────┘
```

### Key Design Principles

- **Stateless HTTP** — Each request is independent. Transaction state isolated per-connection.
- **Lock-free pooling** — `crossbeam::ArrayQueue` with `CachePadded` atomics. Zero mutex acquisitions.
- **Input validation at the boundary** — SQL capped at 10K chars, identifiers at 255, batch rows at 1K. SQL injection prevention via `quote_ident`.
- **PG version-aware queries** — Verified against PG 16–18. Graceful fallbacks when views/columns differ across versions.

---

## License

Apache-2.0

## Support

[GitHub Issues](https://github.com/corporatepiyush/mcp-pg-rust/issues)
