//! Single source of truth for all MCP tools.
//!
//! Each entry declares the tool name, whether it requires a DB connection,
//! and whether it is a write operation (blocked in restricted mode).
//!
//! Adding a tool here automatically populates existence checks and
//! restricted-mode enforcement.  The dispatch `match` in `server.rs`
//! must also be kept in sync — if they diverge, the test
//! `test_tool_registry_matches_dispatch` will fail.

pub struct ToolMeta {
    pub name: &'static str,
    pub needs_db: bool,
    pub write: bool,
}

#[rustfmt::skip]
pub const ALL_TOOLS: &[ToolMeta] = &[
    // add_column
    ToolMeta { name: "add_column",                 needs_db: true,  write: true  },
    // add_compression_policy
    ToolMeta { name: "add_compression_policy",     needs_db: true,  write: true  },
    // add_continuous_aggregate
    ToolMeta { name: "add_continuous_aggregate",   needs_db: true,  write: true  },
    // add_foreign_key
    ToolMeta { name: "add_foreign_key",            needs_db: true,  write: true  },
    // add_retention_policy
    ToolMeta { name: "add_retention_policy",       needs_db: true,  write: true  },
    // add_unique_constraint
    ToolMeta { name: "add_unique_constraint",      needs_db: true,  write: true  },
    // alter_column_type
    ToolMeta { name: "alter_column_type",          needs_db: true,  write: true  },
    // alter_index
    ToolMeta { name: "alter_index",                needs_db: true,  write: true  },
    // alter_role
    ToolMeta { name: "alter_role",                 needs_db: true,  write: true  },
    // alter_user
    ToolMeta { name: "alter_user",                 needs_db: true,  write: true  },
    // alter_view
    ToolMeta { name: "alter_view",                 needs_db: true,  write: true  },
    // analyze_db_health
    ToolMeta { name: "analyze_db_health",          needs_db: true,  write: false },
    // analyze_table
    ToolMeta { name: "analyze_table",              needs_db: true,  write: true  },
    // analyze_table_bloat
    ToolMeta { name: "analyze_table_bloat",        needs_db: true,  write: false },
    // async_batch_delete
    ToolMeta { name: "async_batch_delete",         needs_db: true,  write: true  },
    // async_batch_insert
    ToolMeta { name: "async_batch_insert",         needs_db: true,  write: true  },
    // async_batch_insert_copy
    ToolMeta { name: "async_batch_insert_copy",    needs_db: true,  write: true  },
    // async_batch_update
    ToolMeta { name: "async_batch_update",         needs_db: true,  write: true  },
    // async_execute_delete
    ToolMeta { name: "async_execute_delete",       needs_db: true,  write: true  },
    // async_execute_insert
    ToolMeta { name: "async_execute_insert",       needs_db: true,  write: true  },
    // async_execute_update
    ToolMeta { name: "async_execute_update",       needs_db: true,  write: true  },
    // audit_role_usage
    ToolMeta { name: "audit_role_usage",           needs_db: true,  write: false },
    // backup_table
    ToolMeta { name: "backup_table",               needs_db: true,  write: true  },
    // bm25_force_merge
    ToolMeta { name: "bm25_force_merge",           needs_db: true,  write: true  },
    // bm25_index_stats
    ToolMeta { name: "bm25_index_stats",           needs_db: true,  write: false },
    // cancel_query
    ToolMeta { name: "cancel_query",               needs_db: true,  write: true  },
    // clone_table_schema
    ToolMeta { name: "clone_table_schema",         needs_db: true,  write: true  },
    // compress_chunk
    ToolMeta { name: "compress_chunk",             needs_db: true,  write: true  },
    // create_bm25_index
    ToolMeta { name: "create_bm25_index",          needs_db: true,  write: true  },
    // create_database
    ToolMeta { name: "create_database",            needs_db: true,  write: true  },
    // create_extension
    ToolMeta { name: "create_extension",           needs_db: true,  write: true  },
    // create_hypertable
    ToolMeta { name: "create_hypertable",          needs_db: true,  write: true  },
    // create_index
    ToolMeta { name: "create_index",               needs_db: true,  write: true  },
    // create_partition
    ToolMeta { name: "create_partition",           needs_db: true,  write: true  },
    // create_role
    ToolMeta { name: "create_role",                needs_db: true,  write: true  },
    // create_schema
    ToolMeta { name: "create_schema",              needs_db: true,  write: true  },
    // create_sequence
    ToolMeta { name: "create_sequence",            needs_db: true,  write: true  },
    // create_table
    ToolMeta { name: "create_table",               needs_db: true,  write: true  },
    // create_user
    ToolMeta { name: "create_user",                needs_db: true,  write: true  },
    // create_vector_index
    ToolMeta { name: "create_vector_index",        needs_db: true,  write: true  },
    // create_view
    ToolMeta { name: "create_view",                needs_db: true,  write: true  },
    // describe_table
    ToolMeta { name: "describe_table",             needs_db: true,  write: false },
    // drop_bm25_index
    ToolMeta { name: "drop_bm25_index",            needs_db: true,  write: true  },
    // drop_column
    ToolMeta { name: "drop_column",                needs_db: true,  write: true  },
    // drop_constraint
    ToolMeta { name: "drop_constraint",            needs_db: true,  write: true  },
    // drop_extension
    ToolMeta { name: "drop_extension",             needs_db: true,  write: true  },
    // drop_foreign_key
    ToolMeta { name: "drop_foreign_key",           needs_db: true,  write: true  },
    // drop_index
    ToolMeta { name: "drop_index",                 needs_db: true,  write: true  },
    // drop_partition
    ToolMeta { name: "drop_partition",             needs_db: true,  write: true  },
    // drop_role
    ToolMeta { name: "drop_role",                  needs_db: true,  write: true  },
    // drop_schema
    ToolMeta { name: "drop_schema",                needs_db: true,  write: true  },
    // drop_sequence
    ToolMeta { name: "drop_sequence",              needs_db: true,  write: true  },
    // drop_table
    ToolMeta { name: "drop_table",                 needs_db: true,  write: true  },
    // drop_user
    ToolMeta { name: "drop_user",                  needs_db: true,  write: true  },
    // drop_view
    ToolMeta { name: "drop_view",                  needs_db: true,  write: true  },
    // execute_delete
    ToolMeta { name: "execute_delete",             needs_db: true,  write: true  },
    // execute_insert
    ToolMeta { name: "execute_insert",             needs_db: true,  write: true  },
    // execute_query
    ToolMeta { name: "execute_query",              needs_db: true,  write: false },
    // execute_update
    ToolMeta { name: "execute_update",             needs_db: true,  write: true  },
    // explain_query
    ToolMeta { name: "explain_query",              needs_db: true,  write: false },
    // export_csv
    ToolMeta { name: "export_csv",                 needs_db: true,  write: false },
    // find_missing_fk_indexes
    ToolMeta { name: "find_missing_fk_indexes",    needs_db: true,  write: false },
    // find_tables_without_pk
    ToolMeta { name: "find_tables_without_pk",     needs_db: true,  write: false },
    // generate_create_index_ddl
    ToolMeta { name: "generate_create_index_ddl",  needs_db: true,  write: false },
    // generate_create_table_ddl
    ToolMeta { name: "generate_create_table_ddl",  needs_db: true,  write: false },
    // get_cache_hit_ratio
    ToolMeta { name: "get_cache_hit_ratio",        needs_db: true,  write: false },
    // get_index_stats
    ToolMeta { name: "get_index_stats",            needs_db: true,  write: false },
    // get_object_details
    ToolMeta { name: "get_object_details",         needs_db: true,  write: false },
    // get_pg_stat_statements
    ToolMeta { name: "get_pg_stat_statements",     needs_db: true,  write: false },
    // get_setting
    ToolMeta { name: "get_setting",                needs_db: true,  write: false },
    // get_table_stats
    ToolMeta { name: "get_table_stats",            needs_db: true,  write: false },
    // grant_privileges
    ToolMeta { name: "grant_privileges",           needs_db: true,  write: true  },
    // import_from_url
    ToolMeta { name: "import_from_url",            needs_db: true,  write: true  },
    // list_bm25_indexes
    ToolMeta { name: "list_bm25_indexes",          needs_db: true,  write: false },
    // list_connections
    ToolMeta { name: "list_connections",           needs_db: true,  write: false },
    // list_database_privileges
    ToolMeta { name: "list_database_privileges",   needs_db: true,  write: false },
    // list_databases
    ToolMeta { name: "list_databases",             needs_db: true,  write: false },
    // list_duplicate_indexes
    ToolMeta { name: "list_duplicate_indexes",     needs_db: true,  write: false },
    // list_extensions
    ToolMeta { name: "list_extensions",            needs_db: true,  write: false },
    // list_indexes
    ToolMeta { name: "list_indexes",               needs_db: true,  write: false },
    // list_partitions
    ToolMeta { name: "list_partitions",            needs_db: true,  write: false },
    // list_replication_slots
    ToolMeta { name: "list_replication_slots",     needs_db: true,  write: false },
    // list_role_memberships
    ToolMeta { name: "list_role_memberships",      needs_db: true,  write: false },
    // list_schemas
    ToolMeta { name: "list_schemas",               needs_db: false, write: false },
    // list_standby_servers
    ToolMeta { name: "list_standby_servers",       needs_db: true,  write: false },
    // list_tables
    ToolMeta { name: "list_tables",                needs_db: false, write: false },
    // list_triggers
    ToolMeta { name: "list_triggers",              needs_db: true,  write: false },
    // list_unused_indexes
    ToolMeta { name: "list_unused_indexes",        needs_db: true,  write: false },
    // list_user_privileges
    ToolMeta { name: "list_user_privileges",       needs_db: true,  write: false },
    // list_users
    ToolMeta { name: "list_users",                 needs_db: true,  write: false },
    // list_vector_columns
    ToolMeta { name: "list_vector_columns",        needs_db: true,  write: false },
    // reindex_database
    ToolMeta { name: "reindex_database",           needs_db: true,  write: true  },
    // reindex_table
    ToolMeta { name: "reindex_table",              needs_db: true,  write: true  },
    // rename_column
    ToolMeta { name: "rename_column",              needs_db: true,  write: true  },
    // rename_index
    ToolMeta { name: "rename_index",               needs_db: true,  write: true  },
    // rename_schema
    ToolMeta { name: "rename_schema",              needs_db: true,  write: true  },
    // rename_table
    ToolMeta { name: "rename_table",               needs_db: true,  write: true  },
    // reset_statistics
    ToolMeta { name: "reset_statistics",           needs_db: true,  write: true  },
    // revoke_privileges
    ToolMeta { name: "revoke_privileges",          needs_db: true,  write: true  },
    // sample_data
    ToolMeta { name: "sample_data",                needs_db: true,  write: false },
    // search_bm25
    ToolMeta { name: "search_bm25",                needs_db: true,  write: false },
    // security_audit
    ToolMeta { name: "security_audit",             needs_db: true,  write: false },
    // show_active_transactions
    ToolMeta { name: "show_active_transactions",   needs_db: true,  write: false },
    // show_all_settings
    ToolMeta { name: "show_all_settings",          needs_db: true,  write: false },
    // show_autocommit_status
    ToolMeta { name: "show_autocommit_status",     needs_db: true,  write: false },
    // show_base_backup_progress
    ToolMeta { name: "show_base_backup_progress",  needs_db: true,  write: false },
    // show_blocked_queries
    ToolMeta { name: "show_blocked_queries",       needs_db: true,  write: false },
    // show_chunks
    ToolMeta { name: "show_chunks",                needs_db: true,  write: false },
    // show_connection_summary
    ToolMeta { name: "show_connection_summary",    needs_db: true,  write: false },
    // show_constraints
    ToolMeta { name: "show_constraints",           needs_db: false, write: false },
    // show_current_user
    ToolMeta { name: "show_current_user",          needs_db: true,  write: false },
    // show_database_size
    ToolMeta { name: "show_database_size",         needs_db: true,  write: false },
    // show_deadlocks
    ToolMeta { name: "show_deadlocks",             needs_db: true,  write: false },
    // show_hypertable_details
    ToolMeta { name: "show_hypertable_details",    needs_db: true,  write: false },
    // show_locks
    ToolMeta { name: "show_locks",                 needs_db: true,  write: false },
    // show_log_settings
    ToolMeta { name: "show_log_settings",          needs_db: true,  write: false },
    // show_memory_settings
    ToolMeta { name: "show_memory_settings",       needs_db: true,  write: false },
    // show_performance_settings
    ToolMeta { name: "show_performance_settings",  needs_db: true,  write: false },
    // show_replication_status
    ToolMeta { name: "show_replication_status",    needs_db: true,  write: false },
    // show_running_queries
    ToolMeta { name: "show_running_queries",       needs_db: true,  write: false },
    // show_session_info
    ToolMeta { name: "show_session_info",          needs_db: true,  write: false },
    // show_table_size
    ToolMeta { name: "show_table_size",            needs_db: true,  write: false },
    // show_transaction_isolation
    ToolMeta { name: "show_transaction_isolation", needs_db: true,  write: false },
    // show_transaction_timeout
    ToolMeta { name: "show_transaction_timeout",   needs_db: true,  write: false },
    // show_vacuum_progress
    ToolMeta { name: "show_vacuum_progress",       needs_db: true,  write: false },
    // show_waiting_locks
    ToolMeta { name: "show_waiting_locks",         needs_db: true,  write: false },
    // show_wal_info
    ToolMeta { name: "show_wal_info",              needs_db: true,  write: false },
    // suggest_indexes
    ToolMeta { name: "suggest_indexes",            needs_db: true,  write: false },
    // table_dependencies
    ToolMeta { name: "table_dependencies",         needs_db: true,  write: false },
    // terminate_connection
    ToolMeta { name: "terminate_connection",       needs_db: true,  write: true  },
    // truncate_table
    ToolMeta { name: "truncate_table",             needs_db: true,  write: true  },
    // vacuum
    ToolMeta { name: "vacuum",                     needs_db: true,  write: true  },
    // vacuum_analyze
    ToolMeta { name: "vacuum_analyze",             needs_db: true,  write: true  },
    // vacuum_full
    ToolMeta { name: "vacuum_full",                needs_db: true,  write: true  },
    // vector_search
    ToolMeta { name: "vector_search",              needs_db: true,  write: false },
];

#[inline]
pub fn tool_exists(name: &str) -> bool {
    ALL_TOOLS.binary_search_by(|t| t.name.as_bytes().cmp(name.as_bytes())).is_ok()
}

#[inline]
pub fn is_write_tool(name: &str) -> bool {
    ALL_TOOLS.binary_search_by(|t| t.name.as_bytes().cmp(name.as_bytes()))
        .map(|i| ALL_TOOLS[i].write)
        .unwrap_or(false)
}

#[inline]
pub fn needs_db(name: &str) -> bool {
    ALL_TOOLS.binary_search_by(|t| t.name.as_bytes().cmp(name.as_bytes()))
        .map(|i| ALL_TOOLS[i].needs_db)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tools_unique() {
        let mut names: Vec<&str> = ALL_TOOLS.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), ALL_TOOLS.len(), "Duplicate tool names in ALL_TOOLS");
    }

    #[test]
    fn test_tool_exists_known() {
        assert!(tool_exists("execute_query"));
        assert!(tool_exists("list_tables"));
        assert!(tool_exists("import_from_url"));
    }

    #[test]
    fn test_tool_exists_unknown() {
        assert!(!tool_exists("nonexistent_tool"));
    }

    #[test]
    fn test_is_write_tool() {
        assert!(is_write_tool("create_table"));
        assert!(is_write_tool("import_from_url"));
        assert!(!is_write_tool("execute_query"));
        assert!(!is_write_tool("list_tables"));
    }

    #[test]
    fn test_needs_db() {
        assert!(needs_db("execute_query"));
        assert!(!needs_db("list_tables"));
        assert!(!needs_db("list_schemas"));
        assert!(!needs_db("show_constraints"));
    }

    #[test]
    fn test_all_tools_registered_in_tools_json() {
        let content = std::fs::read_to_string("tools.json")
            .expect("Failed to read tools.json");
        let json: serde_json::Value = serde_json::from_str(&content)
            .expect("tools.json is not valid JSON");
        let json_tools = json.as_array().expect("tools.json must be an array");

        let json_names: Vec<&str> = json_tools.iter()
            .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
            .collect();

        for meta in ALL_TOOLS {
            assert!(
                json_names.contains(&meta.name),
                "Tool '{}' is in ALL_TOOLS but missing from tools.json",
                meta.name,
            );
        }

        for name in &json_names {
            assert!(
                tool_exists(name),
                "Tool '{}' is in tools.json but missing from ALL_TOOLS",
                name,
            );
        }

        assert_eq!(json_names.len(), ALL_TOOLS.len(),
            "tools.json has {} tools but ALL_TOOLS has {}",
            json_names.len(), ALL_TOOLS.len());
    }
}
