# MCP Compliance & Input Validation

## MCP Specification Compliance

This implementation follows the **Model Context Protocol (MCP) v1.0** specification from the official site: https://spec.modelcontextprotocol.org/

### Tool Definition Structure (MCP Compliant)

Each tool in `tools.json` contains:

```json
{
  "name": "tool_name",
  "description": "Detailed description of what the tool does",
  "inputSchema": {
    "type": "object",
    "properties": {
      "param_name": {
        "type": "string",
        "description": "What this parameter does"
      }
    },
    "required": ["param_name"]
  }
}
```

### MCP Specification Requirements Met

✅ **Tool Naming** - All tools have valid names (lowercase, underscores)  
✅ **Descriptions** - All tools have comprehensive descriptions with examples  
✅ **Input Schema** - All use valid JSON Schema format  
✅ **Required Parameters** - Clearly marked in "required" array  
✅ **Parameter Types** - All parameters have explicit types (string, array, number, etc.)  
✅ **Parameter Descriptions** - All parameters have detailed descriptions  
✅ **Constraints Documentation** - Max lengths, ranges, and examples provided  
✅ **Enum Values** - Format option uses proper enum with examples  

## Input Validation System

A comprehensive validation module (`src/validation.rs`) validates all tool inputs and provides helpful error messages.

### Validation Features

#### 1. **Type Checking**
```
❌ Expected string, got number
💡 Suggestion: Example: {"table": "users"}
```

#### 2. **Required Parameter Validation**
```
❌ Required parameter 'sql' missing
💡 Suggestion: Include SQL: {"sql": "<SELECT statement>"}
```

#### 3. **Constraint Validation**
- **Max length**: 10,000 chars for SQL, 255 for identifiers
- **Range**: batch_size must be 100-5,000
- **Array size**: batch_insert max 1,000 rows, batch_insert_copy max 100,000 rows
- **Enum values**: format must be json, text, xml, or yaml

#### 4. **Business Logic Validation**
- **DELETE/UPDATE safety**: Warns if WHERE clause missing
- **Query type checking**: execute_query only accepts SELECT, etc.
- **Row count validation**: Prevents oversized batches
- **Empty value checking**: Tables/columns can't be empty strings

### Validation Error Format

All validation errors include:
1. **Tool Name** - Which tool had the error
2. **Parameter** - Which parameter triggered the error
3. **Error Message** - Clear description of what's wrong
4. **Suggestion** - Actionable fix with examples

Example:
```
❌ Validation Error in tool 'batch_insert' parameter 'rows': 
   Too many rows: 2000 (max 1000)
💡 Suggestion: Split into multiple calls with max 1000 rows each
```

## Tool-Specific Validation Rules

### Schema Inspection Tools

**describe_table**
- ✅ table (required, string, 1-255 chars)
- Optional: schema (string, default "public")

**get_object_details**
- ✅ table (required, string)
- Optional: schema (string)

### Query Execution Tools

**execute_query**
- ✅ sql (required, string, 1-10000 chars)
- Must start with SELECT or WITH
- Returns rows as arrays

**execute_insert**
- ✅ sql (required, string)
- Must start with INSERT
- Supports RETURNING clause
- Max 10,000 chars

**execute_update**
- ✅ sql (required, string)
- Must start with UPDATE
- ⚠️ Requires WHERE clause (validated)
- Max 10,000 chars

**execute_delete**
- ✅ sql (required, string)
- Must start with DELETE
- ⚠️ Requires WHERE clause (validated)
- Max 10,000 chars

### Batch Operations

**batch_insert**
- ✅ table (required, string, non-empty)
- ✅ columns (required, array of strings)
- ✅ rows (required, array of arrays, max 1,000)
- Optional: returning (string, column name to return)

**batch_insert_copy**
- ✅ table (required, string)
- ✅ columns (required, array of strings)
- ✅ rows (required, array of arrays, max 100,000)
- Optional: batch_size (integer, 100-5,000, default 1,000)

### Query Analysis

**explain_query**
- ✅ sql (required, string, 1-10000 chars)
- Optional: analyze (boolean, default false)
- Optional: buffers (boolean, default false)
- Optional: format (enum: json|text|xml|yaml, default json)

### Configuration

**get_setting**
- ✅ setting_name (required, string, non-empty)
- Examples: "max_connections", "shared_buffers", "work_mem"

## Error Messages by Category

### 1. Missing Required Parameters
```json
{
  "tool": "describe_table",
  "param": "table",
  "error": "Required parameter 'table' missing",
  "suggestion": "Include 'table' parameter: {\"table\": \"users\"}"
}
```

### 2. Type Mismatches
```json
{
  "tool": "batch_insert",
  "param": "columns",
  "error": "Expected array of strings, got object",
  "suggestion": "Example: {\"columns\": [\"email\", \"name\", \"created_at\"]}"
}
```

### 3. Constraint Violations
```json
{
  "tool": "execute_query",
  "param": "sql",
  "error": "SQL too long: 15000 characters (max 10,000)",
  "suggestion": "Break the query into smaller parts or use a subquery"
}
```

### 4. Business Logic Violations
```json
{
  "tool": "execute_delete",
  "param": "sql",
  "error": "DELETE without WHERE clause will delete all rows",
  "suggestion": "Add a WHERE clause: DELETE FROM users WHERE <condition>"
}
```

### 5. Invalid Enum Values
```json
{
  "tool": "explain_query",
  "param": "format",
  "error": "Invalid format 'csv' (must be json, text, xml, or yaml)",
  "suggestion": "Use one of: json (default), text, xml, yaml"
}
```

## Validation Usage in Code

```rust
use mcp_postgres::validation::validate_tool_input;

// Validate input before tool execution
let arguments = json!({"table": "users"});
match validate_tool_input("describe_table", &arguments) {
    Ok(()) => {
        // Valid input - proceed with tool execution
        println!("✅ Input valid, executing tool...");
    }
    Err(errors) => {
        // Invalid input - return error messages to client
        for error in errors {
            println!("{}", error); // Prints formatted error with suggestion
        }
    }
}
```

## MCP Compliance Checklist

- [x] All tools have name, description, inputSchema
- [x] All inputSchema use valid JSON Schema format
- [x] All parameters have descriptions
- [x] All required parameters listed in "required" array
- [x] All parameter types explicitly defined
- [x] Constraint documentation (max/min, length, enum)
- [x] Examples provided for all parameters
- [x] Validation errors are helpful and actionable
- [x] Error messages include suggestions
- [x] Safety warnings for destructive operations
- [x] Type checking for all parameters
- [x] Range validation for numeric parameters
- [x] Enum validation for restricted values

## Validation Testing

The validation module includes unit tests:

```bash
cargo test validation
```

Tests cover:
- Missing required parameters
- Type mismatches
- Constraint violations
- Valid inputs
- Edge cases

## Security Considerations

Validation provides:
- **SQL Injection Prevention**: Validates query types (SELECT vs INSERT)
- **Destructive Operation Safety**: Warns about DELETE/UPDATE without WHERE
- **Resource Protection**: Enforces row count limits for batch operations
- **DoS Prevention**: Limits SQL statement length to 10,000 chars
- **Configuration Safety**: Validates setting names exist before querying

## References

- **MCP Specification**: https://spec.modelcontextprotocol.org/
- **JSON Schema Standard**: https://json-schema.org/
- **PostgreSQL Documentation**: https://www.postgresql.org/docs/

---

**Last Updated**: 2026-06-13  
**Compliance Level**: Full MCP v1.0  
**Validation Coverage**: 22/22 tools  
**Test Coverage**: Unit tests for validation module
