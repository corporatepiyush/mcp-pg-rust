# Migration Guide — mcp-postgres 4.x → 5.0.0

**Release theme:** MCP specification compliance (protocol revision **`2025-11-25`**).

Version 5.0.0 is a **major** release because the shape of every `tools/call`
response changed. If you consume this server through a standard MCP client
(Claude Desktop, the MCP SDKs, etc.) the upgrade is transparent — those clients
already expect the spec-compliant format. If you wrote a **custom client** that
reads the raw JSON-RPC `result` of `tools/call`, you must update it. See
[Breaking changes](#breaking-changes).

---

## Why this release

Earlier 4.x releases predated large parts of the MCP specification and contained
two concrete protocol violations:

1. **`tools/call` did not return a `CallToolResult`.** Handlers returned raw
   payloads such as `{"tables": [...]}` or `{"success": true}` directly as the
   JSON-RPC `result`. The spec requires a `content` array.
2. **`initialize` advertised `resources` and `prompts` capabilities that had no
   handlers.** A client acting on them received `-32601 Method not found`.

5.0.0 fixes both, advances the negotiated protocol version, and aligns error
and result handling with the spec.

---

## Breaking changes

### 1. `tools/call` now returns a spec-compliant `CallToolResult`

**Before (4.x):**

```jsonc
// result of tools/call for list_tables
{
  "tables": ["users", "orders"]
}
```

**After (5.0.0):**

```jsonc
{
  "content": [
    { "type": "text", "text": "{\"tables\":[\"users\",\"orders\"]}" }
  ],
  "structuredContent": { "tables": ["users", "orders"] },
  "isError": false
}
```

- The original payload is preserved verbatim under **`structuredContent`** (when
  it is a JSON object) and as serialized text in **`content[0].text`**.
- **Migration:** read `result.structuredContent` instead of `result` directly.
  If you only need a display string, read `result.content[0].text`.

### 2. Tool failures are returned as results, not protocol errors

Execution failures (SQL errors, validation failures, restricted-mode and
`import_from_url` policy rejections) no longer come back as JSON-RPC errors.
They are `CallToolResult`s with `isError: true`:

```jsonc
{
  "content": [{ "type": "text", "text": "Database error: relation \"foo\" does not exist" }],
  "isError": true
}
```

This lets the model see the failure and self-correct. **Protocol-level errors**
(malformed request, missing `name`, unknown tool/method) are still returned as
JSON-RPC `error` objects.

**Migration:** check `result.isError` on every `tools/call` response. Do not
assume a successful JSON-RPC response means the tool succeeded.

### 3. `resources` and `prompts` capabilities are no longer advertised

`initialize` previously returned `capabilities.resources` and
`capabilities.prompts`. These were never implemented and are now removed.

**Migration:** if your client branched on these capabilities, it will now
correctly see that they are unavailable. Resources/prompts are tracked on the
roadmap below.

### 4. Negotiated protocol version is now `2025-11-25`

`initialize` returns `"protocolVersion": "2025-11-25"` by default and performs
**version negotiation**: if the client requests a revision this server supports
(`2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05`), that exact revision is
echoed back; otherwise the latest is offered.

**Migration:** none required. Clients pinned to `2024-11-05` continue to work
because the server echoes it back.

---

## New in 5.0.0

- **`instructions`** field in `InitializeResult` — guidance appended to the
  model's system prompt on how to use the tools.
- **`structuredContent`** on every object-valued tool result (MCP 2025-06-18+).
- **Version negotiation** in `initialize`.

---

## Not yet implemented (roadmap)

These remain gaps after 5.0.0 and are intentionally **not** advertised as
capabilities (so clients are never misled):

| Feature | Notes |
|---|---|
| `resources/*` (`postgres://…` URIs) | schema/table/view as readable resources |
| `prompts/*` | `analyze-table`, `optimize-query`, `db-health-check` |
| `logging/setLevel` + `notifications/message` | server→client log streaming |
| `completion/complete` | autocomplete for table/column names |
| `notifications/progress` / `notifications/cancelled` | long-query progress & cancel |
| `tools/list` pagination | 135 tools are returned in one page |
| Streamable HTTP transport | current HTTP transport is POST `/rpc` + legacy SSE |

---

## Upgrade checklist

- [ ] Bump the dependency / reinstall: `cargo install mcp-postgres` (or Homebrew).
- [ ] If you use a standard MCP client: nothing to do.
- [ ] If you use a custom client:
  - [ ] Read tool output from `result.structuredContent` (or `content[0].text`).
  - [ ] Check `result.isError` to detect tool failures.
  - [ ] Stop relying on `resources`/`prompts` capabilities.
- [ ] Verify your client tolerates `protocolVersion: "2025-11-25"` (or pin an
      older supported revision in your `initialize` request).
