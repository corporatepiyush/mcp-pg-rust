# mcp-postgres v4.0.4 — Security & Optimization Review + Upgrade Plan

> Status: proposed. Targets a `4.1.0` (new flags = minor). Severity legend: 🔴 high · 🟠 medium · 🟡 low/hardening.

## Executive summary

The performance/concurrency design is strong (lock-free pool, cache-line alignment, pre-serialized responses). The largest exposure is the **trust/transport model**: both the TCP (`:3000`) and HTTP (`:3001`) transports expose full DDL/DML/user-management with **no authentication, no TLS, no per-request timeout, and no request-size cap**. Several action modules build SQL via string interpolation in ways that bypass the otherwise-good `validate_identifier`/`quote_ident` discipline. There is also a meaningful correctness bug in `execute_query` type handling and a dead mimalloc-tuning block.

---

## A. Security findings

### 🔴 A1 — No authentication on TCP or HTTP transports
`src/server.rs:88` (`run`), `src/http.rs:32` (`/rpc`). Anyone who can reach the port gets full database control (default `unrestricted` mode: `drop_table`, `create_user`, `grant_privileges`, `terminate_connection`, …). Default bind is `127.0.0.1` (`src/lib.rs:24`), but `-H 0.0.0.0` turns it into an open DB proxy. No token/bearer/mTLS check exists.

**Fix:** require a shared secret (`--auth-token` / `MCP_AUTH_TOKEN`) checked on every TCP line and as `Authorization: Bearer` on HTTP; reject before dispatch. Non-loopback binds must refuse to start without a token.

### 🔴 A2 — SSRF in `import_from_url`
`src/actions/data_io.rs:47` does `reqwest::get(url)` on a fully client-controlled URL with no scheme/host allowlist. Can reach cloud metadata (`169.254.169.254`), internal services, and `localhost` admin ports, then ingest the body into a table.

**Fix:** allowlist schemes (`https`/`http`), resolve host and reject private/loopback/link-local/ULA ranges (RFC1918, 127/8, 169.254/16, ::1, fc00::/7), set response-size cap + timeout, disable redirects (or re-validate each hop). Gate behind an opt-in flag (`--allow-url-import`).

### 🟠 A3 — SQL injection via un-validated interpolation
- `grant_privileges` / `revoke_privileges` — `privilege` interpolated raw (`src/actions/user_mgmt.rs:308-314, 373-379`).
- `import_from_url` — `columns` interpolated raw (`data_io.rs:61`); `delimiter` only single-quote-escaped though COPY needs a single char.
- `export_csv` — `query` accepted if it merely starts with `SELECT` (`data_io.rs:113`), wrapped into `COPY (…) TO STDOUT`.

The extended protocol blocks stacked `;` statements, so this is privilege-escalation/semantic-injection rather than classic stacking — still worth closing. **Fix:** allowlist privilege keywords, validate `columns` as a comma list of identifiers, enforce single-char delimiter.

### 🟠 A4 — No TLS to PostgreSQL
`src/pool.rs:42` connects with `NoTls`. Credentials/data in cleartext — unacceptable for a remote DB. **Fix:** support `sslmode`/rustls (e.g. `tokio-postgres-rustls`) honoring the connection string.

### 🟠 A5 — Weak read-only ("restricted") enforcement
Restricted mode (`server.rs:275`) only blocks tools flagged `write:true`. But `execute_query` is `write:false` and accepts any statement starting with `SELECT` — which can call a volatile function that writes. **Fix:** in restricted mode run each statement under `BEGIN; SET TRANSACTION READ ONLY; …; COMMIT` (or connect with a read-only role / `default_transaction_read_only=on`).

### 🟠 A6 — `validate_sql` multi-statement scanner is naive
`src/actions/query.rs:30-44` toggles only on `'` — ignores dollar-quoting (`$$…$$`), double-quoted identifiers, and `--`/`/* */` comments, producing both false rejects and false accepts. Defense-in-depth only (protocol already blocks stacking); correct it or de-emphasize in favor of A5.

### 🟡 A7 — Passwords built into SQL text
`create_user`/`alter_user`/`create_role`/`alter_role` interpolate the password with `replace('\'', "''")` (e.g. `user_mgmt.rs:36,99,208`). `CREATE ROLE` can't bind a parameter, so string-building is unavoidable, but: validate password has no NUL/newline, and ensure SQL containing it is never logged (audit `server.rs:524` — currently logs the error, not the SQL; keep it that way).

### 🟡 A8 — `async_batch_delete`/`update` OR-join WHERE clauses
`build_where_sql` joins predicates with `" OR "` (`batch.rs:79`). Multiple conditions widen the affected set — dangerous default for DELETE/UPDATE. **Fix:** default to `AND`, or make the join explicit per request.

### 🟡 A9 — Transport DoS surface
- TCP `read_line` (`server.rs:127`) reads into an unbounded `String`.
- No idle/read timeout on accepted TCP sockets (slowloris).
- `config.server.request_timeout` (30s) is **set but never applied** — no `statement_timeout`, so one slow query pins a pool connection.
- `export_csv` buffers the entire result (up to 100k rows) in RAM then again as a JSON string (`data_io.rs:139-147`).

**Fix:** cap line length; set socket read timeout; apply `SET statement_timeout` per request; stream/limit export size.

---

## B. Optimization findings

### 🟠 B1 — `execute_query` type inference is slow *and* lossy (correctness)
`src/actions/query.rs:62-78` chains `try_get::<bool>→i32→i64→f32→f64→String`.
1. **Data loss:** `NUMERIC`, `DATE`/`TIMESTAMP(TZ)`, `UUID`, `JSON`/`JSONB`, `BYTEA`, arrays, non-UTF8 text all fall through to `Value::Null` — silently corrupting results.
2. **Cost:** each failed `try_get` decodes + builds an error, per cell.

**Fix:** read `row.columns()[i].type_()` once per column, dispatch on the OID, map the full common type set. Same pattern duplicated in `batch.rs` RETURNING handling.

### 🟠 B2 — `tools/list` parses + re-serializes ~50 KB every call
`server.rs:258` deserializes `TOOLS_LIST_RESPONSE` into a `Value`, then `process_one_line` re-serializes it. **Fix:** special-case `tools/list` to splice the cached `{"tools":[…]}` bytes straight into the JSON-RPC envelope, skipping the `Value` round-trip.

### 🟠 B3 — mimalloc tuning block is dead code
`src/main.rs:15-20` sets `MIMALLOC_*` env vars *after* the global allocator has initialized (read at process init) — no-ops. Comment claims "mimalloc v3" while `Cargo.toml` pins v0.1 (v2.1.x C lib). **Fix:** set via wrapper features/real init hook, or delete the block and the misleading comments.

### 🟡 B4 — `Config` cloned per connection
`server.rs:102` clones `Config` (owns `String` url + host) per accepted TCP connection. **Fix:** `Arc<Config>`.

### 🟡 B5 — Giant string `match` dispatch
`server.rs:291-520` is ~135 sequential string compares while `tools.rs` already uses `binary_search`. **Fix (optional):** dispatch through a sorted `name → fn pointer` table (also removes the documented match/registry drift risk).

### 🟡 B6 — Misc
- `async_sync_commit_execute` uses 4 round-trips for BEGIN/SET LOCAL/exec/COMMIT (`query.rs:267`).
- `import_from_url` returns `rows_imported: 0` hard-coded (`data_io.rs:74`) — reports success without a real count.
- `reqwest = "0.11"` (current 0.12) pulled in with `json` for one `get`; trim features.

---

## C. Phased upgrade plan

**Phase 0 — Triage / guardrails (ship first, low risk)**
- B3: delete/repair the dead mimalloc block + fix the "v3" comments.
- A9c: apply `request_timeout` as `statement_timeout` on each pooled connection.
- B6: fix `import_from_url` row count (or document it returns 0).
- CHANGELOG/SKILLS.md notes; start a `4.1.0` line.

**Phase 1 — Authn/transport hardening (headline fix)**
- A1: `--auth-token`/`MCP_AUTH_TOKEN`; constant-time compare; enforce on TCP + HTTP; refuse non-loopback bind without a token.
- A9a/b: cap TCP line length; socket read timeout.
- A4: optional TLS to PG via rustls, driven by `sslmode`.
- B4: `Arc<Config>`.

**Phase 2 — Injection & SSRF closure**
- A2: SSRF allowlist + private-range block + size/time caps + opt-in flag.
- A3: allowlist `privilege`; validate `columns`/`delimiter`.
- A5: real read-only enforcement (`SET TRANSACTION READ ONLY`) in restricted mode.
- A8: batch WHERE join → `AND` (or explicit); migration note (behavior change).
- A6: tighten or retire the `validate_sql` semicolon scanner.

**Phase 3 — Correctness & perf**
- B1: OID-based decoding for `execute_query` + batch RETURNING (highest-value non-security item).
- B2: zero-copy `tools/list` splice.
- A9d: stream/limit `export_csv`.
- B5: table-driven dispatch (optional).

**Phase 4 — Verification**
- Tests: auth accept/reject; SSRF blocked ranges; privilege allowlist; read-only mode rejects writing functions; `execute_query` round-trips numeric/timestamp/uuid/json; oversized-line rejection.
- Re-run `cargo clippy --all-targets` + benches to confirm no hot-path regression.

### Suggested first step
Phase 1 (A1 auth) most reduces real-world risk; Phase 3/B1 is the most impactful correctness fix. Recommend starting with **A1 + B1** for `4.1.0`.
