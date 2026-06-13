/// Input validation for tool parameters
/// Validates tool arguments against defined schemas and provides helpful error messages

use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub tool: String,
    pub param: String,
    pub error: String,
    pub suggestion: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "❌ Validation Error in tool '{}' parameter '{}': {}\n💡 Suggestion: {}",
            self.tool, self.param, self.error, self.suggestion
        )
    }
}

pub fn validate_tool_input(tool_name: &str, arguments: &Value) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    match tool_name {
        // Schema Inspection Tools
        "list_tables" => {
            // No required parameters
        }
        "describe_table" => {
            if let Some(table) = arguments.get("table") {
                if !table.is_string() {
                    errors.push(ValidationError {
                        tool: tool_name.to_string(),
                        param: "table".to_string(),
                        error: "Expected string, got ".to_string() + &table.type_str(),
                        suggestion: "Example: {\"table\": \"users\"} or {\"table\": \"public.orders\"}".to_string(),
                    });
                } else {
                    let table_str = table.as_str().unwrap();
                    if table_str.is_empty() {
                        errors.push(ValidationError {
                            tool: tool_name.to_string(),
                            param: "table".to_string(),
                            error: "Table name cannot be empty".to_string(),
                            suggestion: "Provide a valid table name like 'users' or 'public.products'".to_string(),
                        });
                    }
                    if table_str.len() > 255 {
                        errors.push(ValidationError {
                            tool: tool_name.to_string(),
                            param: "table".to_string(),
                            error: format!("Table name too long: {} characters (max 255)", table_str.len()),
                            suggestion: "Use a shorter table name".to_string(),
                        });
                    }
                }
            } else {
                errors.push(ValidationError {
                    tool: tool_name.to_string(),
                    param: "table".to_string(),
                    error: "Required parameter missing".to_string(),
                    suggestion: "Include 'table' parameter: {\"table\": \"users\"}".to_string(),
                });
            }
        }

        // Query Execution Tools
        "execute_query" | "execute_insert" | "execute_update" | "execute_delete" => {
            if let Some(sql) = arguments.get("sql") {
                if !sql.is_string() {
                    errors.push(ValidationError {
                        tool: tool_name.to_string(),
                        param: "sql".to_string(),
                        error: format!("Expected string SQL, got {}", sql.type_str()),
                        suggestion: "Example: {\"sql\": \"SELECT * FROM users LIMIT 10\"}".to_string(),
                    });
                } else {
                    let sql_str = sql.as_str().unwrap();
                    if sql_str.is_empty() {
                        errors.push(ValidationError {
                            tool: tool_name.to_string(),
                            param: "sql".to_string(),
                            error: "SQL statement cannot be empty".to_string(),
                            suggestion: "Provide a valid SQL statement".to_string(),
                        });
                    }
                    if sql_str.len() > 10000 {
                        errors.push(ValidationError {
                            tool: tool_name.to_string(),
                            param: "sql".to_string(),
                            error: format!("SQL too long: {} characters (max 10,000)", sql_str.len()),
                            suggestion: "Break the query into smaller parts or use a subquery".to_string(),
                        });
                    }

                    // Validate SQL type for specific tools
                    let sql_upper = sql_str.trim().to_uppercase();
                    match tool_name {
                        "execute_query" => {
                            if !sql_upper.starts_with("SELECT") && !sql_upper.starts_with("WITH") {
                                errors.push(ValidationError {
                                    tool: tool_name.to_string(),
                                    param: "sql".to_string(),
                                    error: "execute_query requires a SELECT statement".to_string(),
                                    suggestion: "Use 'execute_query' only for SELECT queries. Use 'execute_insert', 'execute_update', or 'execute_delete' for modifications.".to_string(),
                                });
                            }
                        }
                        "execute_insert" => {
                            if !sql_upper.starts_with("INSERT") {
                                errors.push(ValidationError {
                                    tool: tool_name.to_string(),
                                    param: "sql".to_string(),
                                    error: "execute_insert requires an INSERT statement".to_string(),
                                    suggestion: "Example: {\"sql\": \"INSERT INTO users (email) VALUES ('user@example.com')\"}".to_string(),
                                });
                            }
                        }
                        "execute_update" => {
                            if !sql_upper.starts_with("UPDATE") {
                                errors.push(ValidationError {
                                    tool: tool_name.to_string(),
                                    param: "sql".to_string(),
                                    error: "execute_update requires an UPDATE statement".to_string(),
                                    suggestion: "Example: {\"sql\": \"UPDATE users SET status = 'active' WHERE id = 1\"}".to_string(),
                                });
                            }
                            if !sql_str.contains("WHERE") && !sql_str.contains("where") {
                                errors.push(ValidationError {
                                    tool: tool_name.to_string(),
                                    param: "sql".to_string(),
                                    error: "UPDATE without WHERE clause will modify all rows".to_string(),
                                    suggestion: "Add a WHERE clause: UPDATE users SET ... WHERE <condition>".to_string(),
                                });
                            }
                        }
                        "execute_delete" => {
                            if !sql_upper.starts_with("DELETE") {
                                errors.push(ValidationError {
                                    tool: tool_name.to_string(),
                                    param: "sql".to_string(),
                                    error: "execute_delete requires a DELETE statement".to_string(),
                                    suggestion: "Example: {\"sql\": \"DELETE FROM users WHERE id = 999\"}".to_string(),
                                });
                            }
                            if !sql_str.contains("WHERE") && !sql_str.contains("where") {
                                errors.push(ValidationError {
                                    tool: tool_name.to_string(),
                                    param: "sql".to_string(),
                                    error: "DELETE without WHERE clause will delete all rows".to_string(),
                                    suggestion: "Add a WHERE clause: DELETE FROM users WHERE <condition>".to_string(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                errors.push(ValidationError {
                    tool: tool_name.to_string(),
                    param: "sql".to_string(),
                    error: "Required parameter 'sql' missing".to_string(),
                    suggestion: format!("Include SQL: {{\"sql\": \"<{} statement>\"}}", tool_name.split('_').nth(1).unwrap_or("SQL")),
                });
            }
        }

        // Batch Insert Tools
        "batch_insert" | "batch_insert_copy" => {
            validate_batch_insert(tool_name, arguments, &mut errors);
        }

        // Explain Query Tool
        "explain_query" => {
            if let Some(sql) = arguments.get("sql") {
                if !sql.is_string() {
                    errors.push(ValidationError {
                        tool: tool_name.to_string(),
                        param: "sql".to_string(),
                        error: "Expected string, got ".to_string() + &sql.type_str(),
                        suggestion: "Example: {\"sql\": \"SELECT * FROM users\"}".to_string(),
                    });
                } else if sql.as_str().unwrap().is_empty() {
                    errors.push(ValidationError {
                        tool: tool_name.to_string(),
                        param: "sql".to_string(),
                        error: "SQL cannot be empty".to_string(),
                        suggestion: "Provide a SELECT query to explain".to_string(),
                    });
                }
            } else {
                errors.push(ValidationError {
                    tool: tool_name.to_string(),
                    param: "sql".to_string(),
                    error: "Required parameter 'sql' missing".to_string(),
                    suggestion: "Include SQL: {\"sql\": \"SELECT * FROM users\"}".to_string(),
                });
            }

            if let Some(format) = arguments.get("format") {
                if let Some(fmt) = format.as_str() {
                    if !["json", "text", "xml", "yaml"].contains(&fmt) {
                        errors.push(ValidationError {
                            tool: tool_name.to_string(),
                            param: "format".to_string(),
                            error: format!("Invalid format '{}' (must be json, text, xml, or yaml)", fmt),
                            suggestion: "Use one of: json (default), text, xml, yaml".to_string(),
                        });
                    }
                }
            }
        }

        // Configuration Tool
        "get_setting" => {
            if let Some(setting) = arguments.get("setting_name") {
                if !setting.is_string() {
                    errors.push(ValidationError {
                        tool: tool_name.to_string(),
                        param: "setting_name".to_string(),
                        error: format!("Expected string, got {}", setting.type_str()),
                        suggestion: "Example: {\"setting_name\": \"max_connections\"}".to_string(),
                    });
                } else if setting.as_str().unwrap().is_empty() {
                    errors.push(ValidationError {
                        tool: tool_name.to_string(),
                        param: "setting_name".to_string(),
                        error: "Setting name cannot be empty".to_string(),
                        suggestion: "Examples: max_connections, shared_buffers, work_mem, effective_cache_size".to_string(),
                    });
                }
            } else {
                errors.push(ValidationError {
                    tool: tool_name.to_string(),
                    param: "setting_name".to_string(),
                    error: "Required parameter 'setting_name' missing".to_string(),
                    suggestion: "Include setting: {\"setting_name\": \"max_connections\"}".to_string(),
                });
            }
        }

        // Object Details Tool
        "get_object_details" => {
            if let Some(table) = arguments.get("table") {
                if !table.is_string() {
                    errors.push(ValidationError {
                        tool: tool_name.to_string(),
                        param: "table".to_string(),
                        error: format!("Expected string, got {}", table.type_str()),
                        suggestion: "Example: {\"table\": \"users\"}".to_string(),
                    });
                } else if table.as_str().unwrap().is_empty() {
                    errors.push(ValidationError {
                        tool: tool_name.to_string(),
                        param: "table".to_string(),
                        error: "Table name cannot be empty".to_string(),
                        suggestion: "Provide a valid table name".to_string(),
                    });
                }
            } else {
                errors.push(ValidationError {
                    tool: tool_name.to_string(),
                    param: "table".to_string(),
                    error: "Required parameter 'table' missing".to_string(),
                    suggestion: "Include table name: {\"table\": \"users\"}".to_string(),
                });
            }

            if let Some(schema) = arguments.get("schema") {
                if !schema.is_string() {
                    errors.push(ValidationError {
                        tool: tool_name.to_string(),
                        param: "schema".to_string(),
                        error: format!("Expected string, got {}", schema.type_str()),
                        suggestion: "Example: {\"schema\": \"public\"}".to_string(),
                    });
                }
            }
        }

        _ => {
            // Unknown tool - no specific validation
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_batch_insert(tool_name: &str, arguments: &Value, errors: &mut Vec<ValidationError>) {
    // Validate table
    if let Some(table) = arguments.get("table") {
        if !table.is_string() {
            errors.push(ValidationError {
                tool: tool_name.to_string(),
                param: "table".to_string(),
                error: format!("Expected string, got {}", table.type_str()),
                suggestion: "Example: {\"table\": \"users\"}".to_string(),
            });
        } else if table.as_str().unwrap().is_empty() {
            errors.push(ValidationError {
                tool: tool_name.to_string(),
                param: "table".to_string(),
                error: "Table name cannot be empty".to_string(),
                suggestion: "Provide a valid table name".to_string(),
            });
        }
    } else {
        errors.push(ValidationError {
            tool: tool_name.to_string(),
            param: "table".to_string(),
            error: "Required parameter 'table' missing".to_string(),
            suggestion: "Include table name: {\"table\": \"users\"}".to_string(),
        });
    }

    // Validate columns
    if let Some(columns) = arguments.get("columns") {
        if !columns.is_array() {
            errors.push(ValidationError {
                tool: tool_name.to_string(),
                param: "columns".to_string(),
                error: format!("Expected array, got {}", columns.type_str()),
                suggestion: "Example: {\"columns\": [\"email\", \"name\", \"created_at\"]}".to_string(),
            });
        } else {
            let cols = columns.as_array().unwrap();
            if cols.is_empty() {
                errors.push(ValidationError {
                    tool: tool_name.to_string(),
                    param: "columns".to_string(),
                    error: "Columns array cannot be empty".to_string(),
                    suggestion: "Provide at least one column name".to_string(),
                });
            }
            for (i, col) in cols.iter().enumerate() {
                if !col.is_string() {
                    errors.push(ValidationError {
                        tool: tool_name.to_string(),
                        param: format!("columns[{}]", i),
                        error: format!("Expected string column name, got {}", col.type_str()),
                        suggestion: "Column names must be strings".to_string(),
                    });
                }
            }
        }
    } else {
        errors.push(ValidationError {
            tool: tool_name.to_string(),
            param: "columns".to_string(),
            error: "Required parameter 'columns' missing".to_string(),
            suggestion: "Include column names: {\"columns\": [\"email\", \"name\"]}".to_string(),
        });
    }

    // Validate rows
    if let Some(rows) = arguments.get("rows") {
        if !rows.is_array() {
            errors.push(ValidationError {
                tool: tool_name.to_string(),
                param: "rows".to_string(),
                error: format!("Expected array of arrays, got {}", rows.type_str()),
                suggestion: "Example: {\"rows\": [[\"user@test.com\", \"John\"], [\"jane@test.com\", \"Jane\"]]}".to_string(),
            });
        } else {
            let rows_arr = rows.as_array().unwrap();
            if rows_arr.is_empty() {
                errors.push(ValidationError {
                    tool: tool_name.to_string(),
                    param: "rows".to_string(),
                    error: "Rows array cannot be empty".to_string(),
                    suggestion: "Provide at least one row of data".to_string(),
                });
            } else {
                let max_rows = if tool_name == "batch_insert" { 1000 } else { 100000 };
                if rows_arr.len() > max_rows {
                    errors.push(ValidationError {
                        tool: tool_name.to_string(),
                        param: "rows".to_string(),
                        error: format!("Too many rows: {} (max {})", rows_arr.len(), max_rows),
                        suggestion: format!("Split into multiple calls with max {} rows each", max_rows),
                    });
                }

                // Validate each row is an array
                for (i, row) in rows_arr.iter().enumerate() {
                    if !row.is_array() {
                        errors.push(ValidationError {
                            tool: tool_name.to_string(),
                            param: format!("rows[{}]", i),
                            error: format!("Row must be array, got {}", row.type_str()),
                            suggestion: "Each row must be an array of values".to_string(),
                        });
                    }
                }
            }
        }
    } else {
        errors.push(ValidationError {
            tool: tool_name.to_string(),
            param: "rows".to_string(),
            error: "Required parameter 'rows' missing".to_string(),
            suggestion: "Include rows: {\"rows\": [[\"value1\", \"value2\"], ...]}".to_string(),
        });
    }

    // Validate batch_size for batch_insert_copy
    if tool_name == "batch_insert_copy" {
        if let Some(batch_size) = arguments.get("batch_size") {
            if !batch_size.is_number() {
                errors.push(ValidationError {
                    tool: tool_name.to_string(),
                    param: "batch_size".to_string(),
                    error: format!("Expected integer, got {}", batch_size.type_str()),
                    suggestion: "Example: {\"batch_size\": 1000}".to_string(),
                });
            } else if let Some(size) = batch_size.as_i64() {
                if size < 100 || size > 5000 {
                    errors.push(ValidationError {
                        tool: tool_name.to_string(),
                        param: "batch_size".to_string(),
                        error: format!("Batch size {} out of range (must be 100-5000)", size),
                        suggestion: "Use default (1000) or set between 100 and 5000".to_string(),
                    });
                }
            }
        }
    }
}

/// Helper trait to get type name from Value
trait ValueType {
    fn type_str(&self) -> &str;
}

impl ValueType for Value {
    fn type_str(&self) -> &str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_required_param() {
        let args = json!({});
        let result = validate_tool_input("describe_table", &args);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_type() {
        let args = json!({"table": 123});
        let result = validate_tool_input("describe_table", &args);
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_input() {
        let args = json!({"table": "users"});
        let result = validate_tool_input("describe_table", &args);
        assert!(result.is_ok());
    }
}
