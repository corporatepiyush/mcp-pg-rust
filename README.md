# mcp-postgres

[![Crates.io](https://img.shields.io/crates/v/mcp-postgres)](https://crates.io/crates/mcp-postgres)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024-edition-orange)](https://github.com/corporatepiyush/mcp-pg-rust)

**mcp-postgres** is a high-performance MCP server that brings PostgreSQL into Claude Desktop and any MCP-compatible AI tool. 135 PostgreSQL tools, lock-free connection pooling, sub-10ms latency.

> **Tools are opt-in (5.2.0+).** No tools are exposed by default. You enable
> them one *category* at a time with `--enable-<category>` flags (or
> `--enable-all`). A server started with no enable flags advertises an **empty**
> tool list and rejects every `tools/call`. See
> [Tool Exposure](#tool-exposure-opt-in-by-category).

> **MCP suite.** One of four high-performance MCP servers written in Rust —
> [mcp-postgres](https://github.com/corporatepiyush/mcp-pg-rust) ·
> [mcp-filesystem](https://github.com/corporatepiyush/mcp-filesystem-rust) ·
> [mcp-memory](https://github.com/corporatepiyush/mcp-memory) ·
> [mcp-web-search](https://github.com/corporatepiyush/mcp-web-search).
> All implement MCP protocol revision **`2025-11-25`**.

## Quick Start

### Install

```bash
# From crates.io
cargo install mcp-postgres

# Or from Homebrew (macOS)
brew tap corporatepiyush/mcp-postgres
brew install mcp-postgres
```

### Run

Tools are opt-in — pass one or more `--enable-<category>` flags (or
`--enable-all`). Without them the server exposes no tools.

```bash
# Stdio mode (for Claude Desktop) — expose read-only query + schema tools
mcp-postgres --database-url "postgres://user:pass@localhost:5432/mydb" --stdio \
  --enable-query --enable-schema

# TCP server (port 3000) — expose everything
mcp-postgres --database-url "postgres://user:pass@localhost:5432/mydb" --enable-all

# HTTP/2 server (port 3001) — query + monitoring only
mcp-postgres --database-url "postgres://user:pass@localhost:5432/mydb" --http-port 3001 \
  --enable-query --enable-monitoring
```

### Claude Desktop

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "postgres": {
      "command": "mcp-postgres",
      "args": ["--database-url", "postgres://user:pass@localhost:5432/mydb", "--stdio", "--enable-all"]
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
      --tls-cert <PATH>          PEM cert chain to serve HTTP over TLS (HTTPS)
      --tls-key <PATH>           PEM private key matching --tls-cert

  Tool exposure (none enabled by default — see "Tool Exposure"):
      --enable-all               Expose every category (overrides the flags below)
      --enable-query             Query: execute/explain/async-execute + sampling
      --enable-batch             Batch: bulk insert/update/delete + COPY
      --enable-schema            Schema: read-only inspection + DDL generation
      --enable-ddl               DDL: create/drop/alter/rename objects
      --enable-admin             Admin: vacuum/reindex/analyze/truncate + sessions
      --enable-monitoring        Monitoring: stats, conns, txns, replication, config
      --enable-security          Security: roles, users, privileges, audits
      --enable-data-io           Data I/O: CSV export + URL/file import
      --enable-extensions        Extensions: pgvector, TimescaleDB, BM25, ext mgmt

  -h, --help                     Print help
  -V, --version                  Print version
```

### TLS (HTTPS)

The HTTP/2 transport can be served over TLS (rustls, `ring` provider — the same
provider used for `sslmode`-gated PostgreSQL connections, no OpenSSL/aws-lc).
Provide a PEM certificate chain and private key via `--tls-cert`/`--tls-key` or
the `MCP_TLS_CERT`/`MCP_TLS_KEY` environment variables and the HTTP server speaks
HTTPS instead of plaintext. The two must be supplied together or startup is
refused; when neither is set the HTTP transport stays plaintext (the default).
The raw TCP transport is unaffected.

```bash
mcp-postgres --http-port 3001 --tls-cert ./cert.pem --tls-key ./key.pem
```

---

## MCP Compliance

Implements the [Model Context Protocol](https://modelcontextprotocol.io) revision **`2025-11-25`** over [JSON-RPC 2.0](https://spec.modelcontextprotocol.io), via TCP, HTTP/2, or stdio.

| Area | Support |
|---|---|
| Transports | stdio, TCP (3000), HTTP/2 (3001) |
| Protocol version | `2025-11-25`, negotiates down to `2025-06-18` / `2025-03-26` / `2024-11-05` |
| `initialize` | ✅ version negotiation + `instructions` |
| `tools/list`, `tools/call` | ✅ (135 tools) |
| `CallToolResult` | ✅ `content[]` + `structuredContent` + `isError` |
| Capabilities advertised | `tools` only — nothing is advertised that isn't implemented |
| `resources` · `prompts` · `logging` · `completion` | ❌ roadmap — see [MIGRATION.md](./MIGRATION.md) |

**Request:**

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": { "name": "list_tables", "arguments": {} },
  "id": 1
}
```

**Result** — a spec-compliant `CallToolResult`. The payload is available as a
machine-readable `structuredContent` object and as serialized `text`; tool
failures come back with `isError: true` (not as JSON-RPC protocol errors) so the
model can self-correct:

```json
{
  "content": [{ "type": "text", "text": "{\"tables\":[\"users\",\"orders\"]}" }],
  "structuredContent": { "tables": ["users", "orders"] },
  "isError": false
}
```

Upgrading from 4.x? The result shape changed — see **[MIGRATION.md](./MIGRATION.md)**.

---

## Tool Exposure (opt-in by category)

Every tool belongs to exactly one of **9 categories**. **Nothing is exposed
until you enable its category** at startup — disabled tools are hidden from
`tools/list` and rejected from `tools/call` as if they did not exist. This lets
you hand an agent precisely the surface area it needs (e.g. read-only `query` +
`monitoring`) and nothing more.

| Flag | Category | What it exposes |
|------|----------|-----------------|
| `--enable-query` | **Query** | `execute_query`, `execute_insert/update/delete`, `async_execute_*`, `explain_query`, `sample_data` |
| `--enable-batch` | **Batch** | `async_batch_insert/update/delete`, `async_batch_insert_copy` |
| `--enable-schema` | **Schema** (read) | `list_tables`, `describe_table`, `list_indexes/schemas/triggers/partitions`, `show_constraints`, `get_object_details`, `list_databases`, `table_dependencies`, `generate_create_*_ddl` |
| `--enable-ddl` | **DDL** (write) | `create/drop/alter` table·view·schema·sequence·index·partition, `backup_table`, `add/drop/rename_column`, `alter_column_type`, `rename_table/index/schema`, FK & constraint ops, `clone_table_schema`, `create_database` |
| `--enable-admin` | **Admin** | `vacuum`, `vacuum_full`, `vacuum_analyze`, `analyze_table`, `reindex_table/database`, `reset_statistics`, `truncate_table`, `cancel_query`, `terminate_connection` |
| `--enable-monitoring` | **Monitoring** (read) | table/index stats, sizes, cache ratio, `pg_stat_statements`, connections, running/blocked queries, transactions, locks, deadlocks, replication, WAL, settings, health checks, `suggest_indexes`, bloat |
| `--enable-security` | **Security** | `list/create/alter/drop_user`, role ops, `grant/revoke_privileges`, privilege listings, `security_audit`, `audit_role_usage` |
| `--enable-data-io` | **Data I/O** | `export_csv`, `import_from_url` (also gated behind `--allow-url-import`) |
| `--enable-extensions` | **Extensions** | pgvector (`vector_search`, `create_vector_index`, `list_vector_columns`), TimescaleDB (`create_hypertable`, `show_chunks`, retention/compression policies, continuous aggregates), BM25 full-text, and generic `create/drop/list_extension` |
| `--enable-all` | *(all of the above)* | Every category. Overrides the individual flags. |

```bash
# Read-only analyst: inspect schema and run SELECTs, nothing else
mcp-postgres -d "$DATABASE_URL" --stdio --enable-query --enable-schema --enable-monitoring

# Full access
mcp-postgres -d "$DATABASE_URL" --stdio --enable-all
```

Category gating composes with the existing `--access-mode restricted` (which
additionally blocks all write tools) and `--allow-url-import` controls.

---

## Tools (135 Total)

The headings below are the human-readable groupings. To map each one to the
`--enable-<category>` flag that exposes it, see the
[Tool Exposure](#tool-exposure-opt-in-by-category) table above.

<details>
<summary><b>⚡ Query Execution</b> (8) · <code>--enable-query</code> — execute_query, execute_insert, execute_update, execute_delete, explain_query, async_execute_insert, async_execute_update, async_execute_delete</summary>
<p>Fire off raw SQL, run parameterized inserts/updates/deletes with automatic type coercion, and peek under the hood with <code>EXPLAIN ANALYZE</code> plans. Async variants let you fire-and-forget long-running operations without blocking your AI workflow.</p>
<p><b>🪄 Key moves:</b> <code>execute_query</code> for ad-hoc SQL, <code>explain_query</code> to spot missing indexes or seq-scans, <code>async_execute_*</code> for bulk writes that outlive the request.</p>
</details>

<details>
<summary><b>🔍 Schema Inspection</b> (8) — list_tables, describe_table, list_schemas, list_indexes, list_triggers, show_constraints, list_partitions, get_object_details</summary>
<p>Peel back the layers of your database: list every table across schemas, drill into column types and nullability, inspect index definitions, trigger functions, check constraints, and navigate partitioned table hierarchies.</p>
<p><b>🪄 Key moves:</b> <code>describe_table</code> is your go-to for column metadata, <code>get_object_details</code> shows DDL + stats in one shot, <code>list_partitions</code> maps your partitioning tree.</p>
</details>

<details>
<summary><b>🏗️ DDL Operations</b> (15) — create/drop table, view, schema, sequence, index, partition, alter_view, backup_table</summary>
<p>Full DDL surface for schema evolution. Spin up tables with typed columns, create views to simplify complex queries, generate sequences for auto-increment IDs, build indexes for performance, and partition large tables for manageability.</p>
<p><b>🪄 Key moves:</b> <code>backup_table</code> snapshots a table before risky DDL, <code>alter_view</code> redefines without dropping, <code>create_partition</code> attaches new ranges to existing partition trees.</p>
</details>

<details>
<summary><b>📦 Batch Operations</b> (4) — async_batch_insert, async_batch_update, async_batch_delete, async_batch_insert_copy</summary>
<p>Move mountains of data in a single call. Insert thousands of rows with structured arrays, run bulk updates and deletes with filtered predicates, or use <code>async_batch_insert_copy</code> leveraging PostgreSQL's COPY protocol for wire-speed ingestion.</p>
<p><b>🪄 Key moves:</b> <code>async_batch_insert_copy</code> is 2-3x faster than row-by-row INSERT for 10K+ rows, <code>async_batch_update</code> handles conditional multi-row updates atomically.</p>
</details>

<details>
<summary><b>📊 Database Monitoring</b> (10) — table/index stats, database/table size, cache hit ratio, vacuum, analyze, pg_stat_statements, reset_statistics</summary>
<p>See how your database is really doing. Track table and index usage stats, measure disk consumption per database or table, calculate cache hit ratios, kick off VACUUM and ANALYZE, and query <code>pg_stat_statements</code> to find your top CPU-hungry queries.</p>
<p><b>🪄 Key moves:</b> <code>get_cache_hit_ratio</code> tells you if your shared_buffers are sized right, <code>get_pg_stat_statements</code> surfaces slow queries by total time, <code>analyze_table</code> refreshes planner stats on-demand.</p>
</details>

<details>
<summary><b>🔌 Connection Management</b> (4) — list_connections, show_current_user, show_running_queries, show_connection_summary</summary>
<p>See who's connected, what they're running, and how connections are distributed. Diagnose connection bloat, find runaway queries, and identify which application is hogging the pool.</p>
<p><b>🪄 Key moves:</b> <code>show_running_queries</code> catches long-running queries in-flight, <code>show_connection_summary</code> groups by state/application for a bird's-eye view.</p>
</details>

<details>
<summary><b>🔐 Security & Users</b> (5) — list_users, user/role privileges, database privileges, session_info</summary>
<p>Audit your security posture: enumerate database roles and their login capabilities, inspect table-level and schema-level GRANTs, check role membership chains, and review database-level ACLs.</p>
<p><b>🪄 Key moves:</b> <code>list_user_privileges</code> surfaces exactly what each user can SELECT/INSERT/UPDATE/DELETE, <code>list_role_memberships</code> reveals privilege escalation paths through role inheritance.</p>
</details>

<details>
<summary><b>⚙️ Configuration</b> (5) — all_settings, get_setting, memory/performance/log_settings</summary>
<p>Navigate the sprawling world of <code>postgresql.conf</code> without grepping. View every GUC parameter, look up specific settings by name, and filter by category to zero in on memory tuning, performance knobs, or logging config.</p>
<p><b>🪄 Key moves:</b> <code>show_memory_settings</code> pulls shared_buffers, work_mem, maintenance_work_mem in one shot, <code>get_setting</code> for a quick <code>SHOW</code> of any parameter.</p>
</details>

<details>
<summary><b>🔄 Transaction Monitoring</b> (7) — active_transactions, locks, waiting_locks, isolation, deadlocks, autocommit, transaction_timeout</summary>
<p>Dive into the transaction machinery: detect long-running idle-in-transaction sessions, map lock contention chains with <code>pg_blocking_pids</code>, identify deadlocks, check isolation levels, and monitor transaction age to prevent bloat.</p>
<p><b>🪄 Key moves:</b> <code>show_waiting_locks</code> shows who's blocked by whom, <code>show_deadlocks</code> queries <code>pg_stat_activity</code> for blocked processes, <code>show_transaction_timeout</code> checks <code>idle_in_transaction_session_timeout</code>.</p>
</details>

<details>
<summary><b>📋 Replication</b> (5) — replication_status, replication_slots, standby_servers, wal_info, base_backup_progress</summary>
<p>Keep your replicas healthy. Monitor streaming replication lag in bytes and time, inspect replication slots for WAL accumulation, list standby servers with their flush/replay positions, and track <code>pg_stat_progress_basebackup</code> for ongoing backups.</p>
<p><b>🪄 Key moves:</b> <code>show_replication_status</code> shows sender/receiver pairs with lag, <code>list_replication_slots</code> helps you spot slots that are consuming too much WAL.</p>
</details>

<details>
<summary><b>🏥 Database Health</b> (4) — analyze_db_health, unused/duplicate indexes, vacuum_progress</summary>
<p>Get a comprehensive wellness check: scan for unused indexes that waste write IO, detect duplicate indexes that bloat storage, monitor VACUUM progress across all databases, and get a health score with actionable recommendations.</p>
<p><b>🪄 Key moves:</b> <code>list_unused_indexes</code> finds indexes with zero scans since stats reset, <code>show_vacuum_progress</code> tracks autovacuum workers in real-time.</p>
</details>

<details>
<summary><b>🗑️ Maintenance</b> (1) — truncate_table</summary>
<p>Safely and quickly remove all rows from a table while preserving the table structure. Performs privilege validation before executing, and supports <code>RESTART IDENTITY</code> for serial column reset.</p>
<p><b>🪄 Key moves:</b> Blows away table data faster than <code>DELETE</code> — ideal for staging tables and temp data cleanup.</p>
</details>

<details>
<summary><b>🧠 Index Advisor</b> (1) — suggest_indexes</summary>
<p>Analyzes your query workload from <code>pg_stat_statements</code> and suggests index candidates based on WHERE clauses, JOIN conditions, and ORDER BY patterns. Each suggestion includes the estimated impact and DDL to create it.</p>
<p><b>🪄 Key moves:</b> Run after capturing a representative workload — the suggestions get smarter the more queries pg_stat_statements has sampled.</p>
</details>

<details>
<summary><b>🔬 Performance Audit</b> (2) — audit_performance, analyze_query_performance</summary>
<p>Deep-dive query forensics with buffer-level analysis. Inspect shared hits vs reads, execution time breakdowns by plan node, row estimate accuracy, and temp file spill detection. <code>audit_performance</code> runs a full sweep across your top queries.</p>
<p><b>🪄 Key moves:</b> <code>analyze_query_performance</code> with <code>EXPLAIN (ANALYZE, BUFFERS)</code> catches seq-scans on large tables, misestimated row counts, and sort spills to disk.</p>
</details>

<details>
<summary><b>🛡️ Security Audit</b> (3) — audit_security, audit_user_permissions, audit_role_hierarchy</summary>
<p>Run a comprehensive security posture review: find users with superuser privileges, detect excessive schema-level GRANTs, map role inheritance chains that could lead to privilege escalation, and audit columns containing sensitive-sounding names.</p>
<p><b>🪄 Key moves:</b> <code>audit_security</code> produces an executive summary with severity ratings, <code>audit_role_hierarchy</code> visualizes role grant chains that might bypass intended restrictions.</p>
</details>

<details>
<summary><b>🔎 Text Search</b> (6) — search_vector, levenshtein_search, trigram_search, soundex_search, metaphone_search, full_text_config</summary>
<p>Unlock fuzzy and phonetic search across your text data. Search by vector similarity via pgvector, find approximate matches with Levenshtein distance, use trigram similarity for flexible substring matching, or try Soundex/Metaphone for phonetic lookups when you don't know the spelling.</p>
<p><b>🪄 Key moves:</b> <code>trigram_search</code> handles typos and partial matches gracefully, <code>soundex_search</code> finds names that sound alike, <code>full_text_config</code> lets you inspect <code>tsvector</code> configuration.</p>
</details>

<details>
<summary><b>🧩 Extension Management</b> (5) — list_extensions, install_extension, remove_extension, update_extension, extension_details</summary>
<p>Manage your PostgreSQL extension ecosystem. List installed extensions with versions, install new extensions from available packages, upgrade to latest versions, remove unused ones, and inspect extension dependencies to understand what cascade effects to expect.</p>
<p><b>🪄 Key moves:</b> <code>extension_details</code> shows extension-to-extension dependency chains, <code>install_extension</code> supports <code>CASCADE</code> for dependency auto-install.</p>
</details>

<details>
<summary><b>📐 Schema Alter</b> (7) — add/drop/alter_column, add/drop_constraint, set_default, drop_default</summary>
<p>Evolve your schema surgically without writing raw DDL. Add nullable or NOT NULL columns with default values, change column types (when castable), drop obsolete columns, manage CHECK and UNIQUE constraints, and set or remove column defaults.</p>
<p><b>🪄 Key moves:</b> <code>add_column</code> with <code>NOT NULL</code> + default backfills existing rows, <code>drop_default</code> stops future inserts from getting the default without touching existing data.</p>
</details>

<details>
<summary><b>🕒 Session Management</b> (3) — set_session_setting, show_session_settings, reset_session_setting</summary>
<p>Twist session-level config knobs without editing <code>postgresql.conf</code>. Uses <code>SET LOCAL</code> scoped to the transaction for safety — settings auto-revert on commit/rollback so you never accidentally leave a modified config behind.</p>
<p><b>🪄 Key moves:</b> <code>set_session_setting</code> for per-query statement_timeout or work_mem tweaks, <code>reset_session_setting</code> reverts a single setting to its cluster default.</p>
</details>

<details>
<summary><b>👤 User Management</b> (3) — create_user, alter_user, drop_user</summary>
<p>Manage database role lifecycle with safety guards. Create users with password and login privileges, alter existing roles (rename, change password, toggle superuser/createrole flags), and drop users with ownership reassignment and dependency checks.</p>
<p><b>🪄 Key moves:</b> <code>drop_user</code> refuses if the user owns objects (prevents accidental CASCADE carnage), <code>alter_user</code> can set per-role GUC defaults like <code>statement_timeout</code>.</p>
</details>

<details>
<summary><b>💾 Data Tools</b> (10) — export_table, import_table, show_table_data, search_data, compare_data, data_profile, find_duplicates, find_orphans, show_pg_stat_user_indexes, show_table_bloat</summary>
<p>Explore, profile, and clean your data. Export tables to structured results, search across columns with pattern matching, compare two datasets row-by-row, profile column distributions and null ratios, find duplicate rows and orphaned foreign keys, and estimate table bloat to plan VACUUMs.</p>
<p><b>🪄 Key moves:</b> <code>data_profile</code> gives you min/max/null_counts/approx_distinct per column in one shot, <code>find_orphans</code> detects FK violations where parent rows went missing, <code>show_table_bloat</code> estimates wasted space.</p>
</details>

<details>
<summary><b>⚗️ Migration Helpers</b> (7) — generate_migration, apply_migration, rollback_migration, list_migrations, show_migration_status, migration_history, validate_migration</summary>
<p>End-to-end schema migration workflow. Generate timestamped migration files from DDL templates, apply pending migrations in order, rollback the last applied migration, track which migrations have been run, and validate the current schema state against the migration log.</p>
<p><b>🪄 Key moves:</b> <code>validate_migration</code> detects schema drift (manual changes that skipped the migration system), <code>rollback_migration</code> reverts safely using generated down-scripts.</p>
</details>

<details>
<summary><b>📈 Vector Database</b> (1) — search_similarity</summary>
<p>pgvector-powered similarity search for embeddings and vector data. Search by L2 distance, inner product, or cosine similarity with optional metadata filters. Supports indexing (IVFFlat, HNSW) for fast approximate nearest-neighbor lookups.</p>
<p><b>🪄 Key moves:</b> Perfect for RAG workflows — feed in your embedding vectors and retrieve the top-K most semantically similar results with a single call.</p>
</details>

<details>
<summary><b>📆 Time-Series</b> (1) — analyze_timescale</summary>
<p>TimescaleDB integration for time-series workloads. Inspect hypertable chunks, analyze compression ratios, review retention policies, and get recommendations for chunk interval tuning based on your ingestion patterns.</p>
<p><b>🪄 Key moves:</b> Run <code>analyze_timescale</code> to check if your compression policy is keeping up with ingestion volume, or if chunk intervals need resizing.</p>
</details>

<details>
<summary><b>📂 Data I/O</b> (6) — import_csv, export_csv, import_json, export_json, show_table_data_paginated, bulk_load_from_file</summary>
<p>Move data in and out of PostgreSQL in every common format. Import CSV/JSON with automatic type detection and error handling, export query results to CSV/JSON, paginate through large result sets with keyset pagination, and bulk-load from server-side files.</p>
<p><b>🪄 Key moves:</b> <code>show_table_data_paginated</code> handles million-row tables without blowing up your context, <code>bulk_load_from_file</code> uses server-side COPY for zero-network-overhead ingestion.</p>
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

## Versioning & Compatibility

Follows [Semantic Versioning](https://semver.org). The current line is **5.x**,
which targets MCP revision `2025-11-25`. The `5.0.0` release changed the
`tools/call` result shape to be spec-compliant — see **[MIGRATION.md](./MIGRATION.md)**
and the [CHANGELOG](./CHANGELOG.md).

| mcp-postgres | MCP revision (default) | Negotiates |
|---|---|---|
| 5.x | `2025-11-25` | `2025-06-18`, `2025-03-26`, `2024-11-05` |
| ≤ 4.x | `2024-11-05` | — |

## License

Apache-2.0

## Support

[GitHub Issues](https://github.com/corporatepiyush/mcp-pg-rust/issues)
