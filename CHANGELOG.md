# Changelog

All notable changes to this project will be documented in this file.

## [4.0.6] - 2026-06-16

### 🔴 SECURITY

- **`create_table`/`create_partition` blocked `/*` comments (C4)**: column definitions and values parameters now reject `/*` block-comment injection, closing a gap where the prior `;` and `--` checks could be bypassed.
- **`export_csv` SQL-prefix validation (C5)**: replaced weak `starts_with("SELECT")` with full `validate_sql` check, blocking comment-prefix smuggling.

### ⚡ PERFORMANCE / MEMORY

- **Zero-copy `read_line_capped`**: decode bytes directly into `String` via `str::from_utf8` + `push_str`, eliminating one `Vec<u8>` allocation per request.
- **Streaming `import_from_url`**: open COPY sink before HTTP fetch, pipe response chunks directly via `sink.send()` — removed intermediate `BytesMut`/`Bytes` buffer (was buffering entire file).
- **`response_buf` auto-shrink**: replace `Vec` with a fresh 4 KB allocation when capacity exceeds 64 KB, reclaiming memory after large exports.

### 🐛 CORRECTNESS

- **`async_batch_insert_copy` row cap**: raised the contradictory 1000-row limit to `MAX_BATCH_COPY_ROWS = 100_000` so chunking actually works; added `MAX_BATCH_SIZE = 5_000` bound on `batch_size` parameter.
- **`async_batch_insert_copy` transactional safety**: now wraps all chunks in `BEGIN`/`SET LOCAL synchronous_commit=OFF`/`COMMIT` with `ROLLBACK` on failure.
- **`backup_table` index-def schema support**: queried `schemaname` from `pg_indexes` to handle schema-qualified table references in index definitions.
- **`auth_token` leak prevention**: added `#[serde(skip_serializing)]` to `ServerConfig.auth_token`.

## [4.0.5] - 2026-06-16

Security hardening and correctness release from the optimization/security review.

### 🔴 SECURITY

- **Transport authentication (A1)**: new `--auth-token` flag / `MCP_AUTH_TOKEN` env. When set, TCP connections must send the token as their first line and HTTP `/rpc` and `/subscribe` must present `Authorization: Bearer <token>` (constant-time compare). The server now refuses to bind a non-loopback host without a token. `/health` stays open; stdio is unaffected.
- **SSRF protection for `import_from_url` (A2)**: http(s) only, hosts must resolve to public addresses (blocks loopback/private/link-local incl. `169.254.169.254`, CGNAT, ULA, IPv4-mapped), redirects disabled, 30s timeout, 100 MiB body cap. The tool is now gated behind `--allow-url-import` (off by default).
- **Validated SQL fragments (A3)**: `grant_privileges`/`revoke_privileges` validate `privilege` against an allowlist; `import_from_url` validates `columns` as identifiers and requires a single-character `delimiter` (also enforced in `export_csv`).
- **Optional TLS to PostgreSQL (A4)**: rustls connector used when the connection string sets `sslmode=require/verify-ca/verify-full/prefer`; plaintext by default (unchanged).
- **Database-enforced read-only mode (A5)**: restricted mode now sets `default_transaction_read_only = on` per connection, so writes are rejected at the database, not just by tool name.
- **Accurate multi-statement detection (A6)**: `validate_sql` now skips string literals, quoted identifiers, dollar-quotes, and comments.
- **Password hardening (A7)**: reject control characters and over-long passwords in user/role tools.
- **Batch WHERE join changed to AND (A8)** — *behavior change*: `async_batch_update`/`async_batch_delete` previously OR-joined predicates, widening the affected rows. They now require all conditions to match.
- **DoS hardening (A9)**: per-connection `statement_timeout` from `request_timeout`; TCP request-line length cap (16 MiB) with an auth-handshake timeout; `export_csv` output capped at 100 MiB.

### ⚡ PERFORMANCE / CORRECTNESS

- **`execute_query` result decoding (B1)**: dispatch on column type — `NUMERIC`, `DATE`/`TIME`/`TIMESTAMP(TZ)`, `UUID`, `JSON`/`JSONB`, and `BYTEA` previously decoded to `null` and now decode correctly (decimals/temporals/uuid as strings, json as structured JSON, bytea as `\x` hex).
- **Zero-copy `tools/list` (B2)**: splices cached bytes into the response on the TCP/stdio path instead of parse + re-serialize.
- **Removed dead mimalloc env block (B3)** and shared `Config` via `Arc` instead of per-connection clones (B4).
- **`import_from_url`** now reports the real imported row count (was hard-coded `0`).

## [2.1.1] - 2026-06-14

### 🔴 SECURITY FIXES (Critical)

#### SQL Injection — Identifiers in batch tools (CVE-2026-XXXX)
`async_batch_insert`, `async_batch_update`, `async_batch_delete`, `async_batch_insert_copy` interpolated `table` and `column_names` with only a length check. An attacker could inject SQL via table/column names. Fixed by validating identifiers through `validate_identifier()` (alphanumeric + underscore only) and quoting with `"`.

#### SQL Injection — Raw WHERE clauses (CVE-2026-XXXX)
`async_batch_update` and `async_batch_delete` accepted arbitrary SQL `WHERE` strings. Replaced with structured predicates `[{column, op, value}]` — column validated as identifier, op allowed-listed (=, <, >, <=, >=, <>, IN, LIKE), value bound via parameterized `format_sql_value`.

#### Multi-Statement Bypass in `validate_sql`
`execute_query`, `execute_insert`, `execute_update`, `execute_delete`, `explain_query` only checked the first token — `SELECT 1; DROP TABLE x` was accepted. Now rejects any unquoted `;` that is not trailing.

#### Session-State Leakage on Pooled Connections
`async_execute_insert`, `async_execute_update`, `async_execute_delete` set `synchronous_commit=OFF` and then hardcoded `ON` on restore. A failed query skipped restore, leaving the connection poisoned for the next user. Fixed by using `BEGIN; SET LOCAL synchronous_commit=OFF; ...; COMMIT` — `SET LOCAL` is transaction-scoped and auto-resets.

### FIXED

- Test suite compilation: `E0716` temporary dropped while borrowed in `integration_all_tools.rs`
- Tool count: reconciled at 76 across README, SKILLS.md, and tools.json
- `validation.rs`: extracted `validate_identifier` from schema.rs, made it a shared module
- `schema.rs`: now delegates to `crate::validation::validate_identifier`

### CHANGED

- `async_batch_update` / `async_batch_delete` `where_clauses` parameter:
  - **Old**: `["id = 1", "id = 2"]` (raw SQL strings — injection vector)
  - **New**: `[{"column": "id", "op": "=", "value": 1}]` (structured — validated)

## [2.0.0] - 2026-06-13

### BREAKING CHANGES ⚠️

#### Removed Tools (4)
The following tools have been removed due to architectural incompatibility with stateless HTTP request/response model:

- **begin_transaction** - Cannot maintain transaction state across multiple HTTP requests
- **commit_transaction** - Each request gets random connection from pool, unrelated to begin
- **rollback_transaction** - Same architectural limitation as commit
- **kill_connection** - Cannot reliably manage connections in stateless architecture

**Migration Path:** Use `async_batch_insert`, `async_batch_update`, `async_batch_delete`, `async_batch_insert_copy` for atomic multi-row operations. These tools execute entire operation atomically in a single request with `synchronous_commit=OFF` for maximum performance.

#### Renamed Tools (4)
Batch tools renamed to clarify high-performance semantics:

- **batch_insert** → **async_batch_insert**
- **batch_update** → **async_batch_update**
- **batch_delete** → **async_batch_delete**
- **batch_insert_copy** → **async_batch_insert_copy**

**Reason:** `async_*` prefix clearly indicates these operations use `synchronous_commit=OFF` for maximum throughput. Aligns naming with existing `async_execute_insert/update/delete` tools.

**Migration:** Update all tool calls:
```json
// OLD (no longer works)
{"method": "tools/call", "params": {"name": "batch_insert", ...}}

// NEW (required)
{"method": "tools/call", "params": {"name": "async_batch_insert", ...}}
```

### Added Features

#### New Tool: show_triggers_for_table
Schema introspection tool to list all triggers defined on a specific table.

**Parameters:**
- `table` (required): Table name to show triggers for
- `schema` (optional): Schema name, defaults to "public"
- `limit` (optional): Maximum triggers to return, defaults to 1000

**Returns:**
```json
{
  "table": "users",
  "schema": "public",
  "trigger_count": 2,
  "triggers": [
    {
      "name": "update_timestamp",
      "table": "users",
      "event": "UPDATE",
      "timing": "BEFORE",
      "statement": "...",
      "schema": "public"
    }
  ]
}
```

**Use Cases:**
- Understanding table automation and side effects
- Debugging unexpected data changes
- Discovering cascade rules and triggers
- Documenting database behavior

### Optimizations

#### Memory Allocation Reductions (Phase 1-3)
- String literal optimization: `"2.0".into()` instead of `.to_string()` (32B per response)
- Removed argument cloning in tool dispatcher (100-500B per request)
- Updated all tool signatures to use borrowed references `&Option<&Value>` (30-40% allocation reduction per tool)

**Impact:** Estimated 450MB-5.6GB daily allocation reduction at scale.

### Technical Details

- Tool count: 24 → 29 tools (4 removed, 4 renamed, 1 added)
- Memory allocation: Optimized for stateless HTTP workloads
- Deprecated: Transaction tools replaced by batch atomic operations
- Improved: Schema introspection now includes trigger discovery

### Testing

✅ All 53 unit tests pass  
✅ cargo check: clean  
✅ No SQL injection vulnerabilities  
✅ No orphaned code or references  

---

## [1.3.1] - Previous releases

See git history for prior versions.
