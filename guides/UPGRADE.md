# Upgrade & Remediation Plan — RESOLVED IN v3.0.0

**v3.0.0** ships all Phase 0–3 fixes as a single major release, including:
- ✅ All tests compile and pass
- ✅ Tool count reconciled at 76 everywhere
- ✅ SQL injection prevented (identifier validation, structured predicates, multi-statement rejection)
- ✅ Durability isolation (`SET LOCAL` instead of bare `SET`)
- ✅ Clippy clean
- ✅ PostgreSQL documentation audit — 4 bugs fixed (autocommit, deadlocks, vacuum columns, base backup view)
- ✅ Integration tests for select tools
- ✅ CI pipeline (.github/workflows/ci.yml)

This document is preserved for historical reference of the remediation plan.

Severity legend: 🔴 ship-blocker · 🟠 security/correctness · 🟡 quality/process

---

## Phase 0 — Stop the bleeding (do first, same day)

### 0.1 🔴 Make the test suite compile
- `cargo test --no-run` currently fails with `E0716: temporary value dropped while borrowed` in `tests/integration_all_tools.rs`.
- Bind the temporary to a `let` before borrowing. Until this compiles, **no release gate is real**.
- Acceptance: `cargo test --no-run` exits 0 for all targets.

### 0.2 🔴 Yank or supersede the broken crate
- v2.1.0 on crates.io was published without a passing test run and with compile-time warnings in shipped bins.
- Options: `cargo yank --version 2.1.0` (keeps existing users, blocks new), then release a fixed **2.1.1** once Phase 1 is green.
- Acceptance: no green-but-broken version is the `max_version` on crates.io.

### 0.3 🔴 Reconcile the tool count everywhere
- Source of truth = dispatch arms in `src/server.rs` (currently **76**, excluding `initialize`).
- Fix README ("46" → real count), SKILLS.md ("46 tools"), and any memory notes. Add a test that asserts `tools.json.len() == <dispatched count>`.
- Acceptance: one number, asserted by a unit test, matched in all docs.

---

## Phase 1 — Security & correctness (before 2.1.1)

### 1.1 🟠 Validate identifiers in batch tools
- `src/actions/batch.rs` interpolates `table` and `column_names` raw (lines ~76, 224, 273, 354) with only a length check.
- Reuse `schema.rs::validate_identifier` (extract it to a shared `crate::validation`), call it on every table/column before interpolation, and quote identifiers with `format!("\"{}\"", ident)` after validation.
- Acceptance: a test injecting `table = 'x"; DROP TABLE y; --'` returns `InvalidParams`, not a query.

### 1.2 🟠 Eliminate the raw `where_clauses` vector
- `async_batch_update` / `async_batch_delete` accept arbitrary SQL WHERE strings — an injection vector exposed as an API.
- Replace with structured predicates `[{column, op, value}]`, validate `column` as an identifier, validate `op` against an allowlist (`=,<,>,<=,>=,<>,IN,LIKE`), and bind `value` as a parameter (`$1...`).
- Acceptance: no caller-supplied string reaches SQL un-parameterized.

### 1.3 🟠 Fix `validate_sql` multi-statement bypass
- First-token checking lets `SELECT 1; DROP TABLE x` through via the simple-query protocol.
- Reject embedded `;` (outside string literals) for the single-statement tools, OR switch execution to the extended/prepared protocol which forbids multiple statements.
- Acceptance: a test with a trailing `; DROP` is rejected.

### 1.4 🟠 Fix session-state leakage on pooled connections
- `query.rs:203-205` and `batch.rs:142-169`: a failed query leaves `synchronous_commit=OFF` on the connection, which returns to the pool poisoned. It also hardcodes `ON` instead of restoring the prior value.
- Preferred fix: set it **per-transaction** — `BEGIN; SET LOCAL synchronous_commit=off; ...; COMMIT;` so it auto-resets and never leaks. `SET LOCAL` is scoped to the transaction and survives no longer than it.
- If keeping session-level SET, wrap in a guard that restores on every exit path (success *and* error), and restore the captured original, not a hardcoded `ON`.
- Acceptance: a test that fails a query, then checks `SHOW synchronous_commit` on a fresh checkout from the pool, sees the original value.

### 1.5 🟠 Resolve the stateless-HTTP vs session-state contradiction
- Decide explicitly: either (a) no tool may mutate session state (use `SET LOCAL` only, inside a txn), or (b) HTTP requests must pin a connection for their duration.
- Document the decision in SKILLS.md §1.1 and enforce it with a grep-based test that fails on any bare `SET ` outside a transaction.

---

## Phase 2 — Quality (before calling it "production-grade")

### 2.1 🟡 Drive clippy to actually zero
- `cargo clippy --all-targets -- -D warnings` currently fails (43 warnings + 2 errors).
- Fix bins too (`load_test_data.rs` PI approximation, unused imports/vars; `measure_latency.rs` dead fields).
- Acceptance: `cargo clippy --all-targets -- -D warnings` exits 0.

### 2.2 🟡 Delete or wire up `validation.rs`
- 21KB / 8 functions with no external callers. Either make the action handlers use it (preferred — removes the duplicated inline validators) or delete it.
- Acceptance: no dead module; one validation path, not three.

### 2.3 🟡 Add real CI (the missing keystone)
- `.github/workflows/ci.yml`: on every PR run `fmt --check`, `clippy --all-targets -D warnings`, `test` against a Postgres service container, and a `tools.json`-vs-dispatch count assertion.
- This is what makes SKILLS.md's 1,100 lines mean anything. Right now none of it is enforced.
- Acceptance: PRs cannot merge red.

---

## Phase 3 — Process & release hygiene

### 3.1 🟡 CHANGELOG + honest SemVer
- Add `CHANGELOG.md`. Document the 1.x→2.x break (whatever it was — it's currently unexplained).
- 2.1.1 entry must state: fixes injection (1.1–1.3), durability leak (1.4), broken tests (0.1).

### 3.2 🟡 Automate Homebrew sync
- Formula pins 2.0.0 tarball/SHA while crates.io serves 2.1.0. The manual sed-in-SKILLS dance already drifted.
- A release workflow should compute the SHA from the pushed tag and open the formula bump automatically.

### 3.3 🟡 Cut SKILLS.md to what is enforced
- Ceremony that no CI checks erodes trust in the parts that matter. Every "STRICT"/"DEAL BREAKER" rule should map to a CI job or be deleted.

---

## Definition of Done for 2.1.1
- [ ] `cargo test` (with a live Postgres) passes — proven, not asserted
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] Injection tests (1.1–1.3) present and passing
- [ ] Durability/session test (1.4) present and passing
- [ ] Tool count single-sourced and asserted
- [ ] CI green on a PR, required for merge
- [ ] CHANGELOG entry written
- [ ] Homebrew formula matches the published crate
