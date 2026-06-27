//! Single source of truth for all MCP tools.
//!
//! Each entry declares the tool name, its category, whether it requires a DB
//! connection, and whether it is a write operation (blocked in restricted mode).
//!
//! Adding a tool here automatically populates existence checks, category
//! gating, and restricted-mode enforcement.  The dispatch `match` in
//! `server.rs` must also be kept in sync — if they diverge, the test
//! `test_tool_registry_matches_dispatch` will fail.
//!
//! ## Tool categories & exposure
//!
//! Tools are grouped into [`ToolCategory`] banners. **No tool is exposed by
//! default** — each category must be explicitly enabled at startup with the
//! matching `--enable-<slug>` flag (or `--enable-all`). A tool that belongs to a
//! disabled category is hidden from `tools/list` and rejected from `tools/call`
//! as if it did not exist.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Coarse capability groups used to selectively expose tools at startup.
///
/// Keep this list at or below ten variants — it maps one-to-one to a
/// `--enable-<slug>` command-line flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolCategory {
    /// Ad-hoc SQL: execute/explain/async-execute and data sampling.
    Query,
    /// Bulk insert/update/delete and COPY-based ingestion.
    Batch,
    /// Read-only schema inspection and DDL generation.
    Schema,
    /// Schema modification: create/drop/alter/rename of database objects.
    Ddl,
    /// Maintenance and session control: vacuum, reindex, analyze, truncate,
    /// cancel/terminate.
    Admin,
    /// Read-only diagnostics: stats, connections, transactions, replication,
    /// configuration, and health checks.
    Monitoring,
    /// Roles, users, privileges, and security audits.
    Security,
    /// Data import/export (CSV, URL fetch).
    DataIo,
    /// PostgreSQL extensions & specialized features: pgvector, TimescaleDB,
    /// BM25 full-text search, and generic extension management.
    Extensions,
}

impl ToolCategory {
    /// Every category, in declaration order. Used to build the CLI surface and
    /// the `--enable-all` set.
    pub const ALL: &'static [ToolCategory] = &[
        ToolCategory::Query,
        ToolCategory::Batch,
        ToolCategory::Schema,
        ToolCategory::Ddl,
        ToolCategory::Admin,
        ToolCategory::Monitoring,
        ToolCategory::Security,
        ToolCategory::DataIo,
        ToolCategory::Extensions,
    ];

    /// Stable kebab-case slug used in the `--enable-<slug>` flag and config.
    pub const fn slug(self) -> &'static str {
        match self {
            ToolCategory::Query => "query",
            ToolCategory::Batch => "batch",
            ToolCategory::Schema => "schema",
            ToolCategory::Ddl => "ddl",
            ToolCategory::Admin => "admin",
            ToolCategory::Monitoring => "monitoring",
            ToolCategory::Security => "security",
            ToolCategory::DataIo => "data-io",
            ToolCategory::Extensions => "extensions",
        }
    }
}

impl fmt::Display for ToolCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

impl FromStr for ToolCategory {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_lowercase().replace('_', "-").as_str() {
            "query" => Ok(ToolCategory::Query),
            "batch" => Ok(ToolCategory::Batch),
            "schema" => Ok(ToolCategory::Schema),
            "ddl" => Ok(ToolCategory::Ddl),
            "admin" => Ok(ToolCategory::Admin),
            "monitoring" => Ok(ToolCategory::Monitoring),
            "security" => Ok(ToolCategory::Security),
            "data-io" => Ok(ToolCategory::DataIo),
            "extensions" => Ok(ToolCategory::Extensions),
            _ => Err(format!("Unknown tool category: {s}")),
        }
    }
}

pub struct ToolMeta {
    pub name: &'static str,
    pub category: ToolCategory,
    pub needs_db: bool,
    pub write: bool,
}

use ToolCategory::{
    Admin, Batch, DataIo, Ddl, Extensions, Monitoring, Query, Schema, Security,
};

#[rustfmt::skip]
pub const ALL_TOOLS: &[ToolMeta] = &[
    ToolMeta { name: "add_column",                 category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "add_compression_policy",     category: Extensions, needs_db: true,  write: true  },
    ToolMeta { name: "add_continuous_aggregate",   category: Extensions, needs_db: true,  write: true  },
    ToolMeta { name: "add_foreign_key",            category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "add_retention_policy",       category: Extensions, needs_db: true,  write: true  },
    ToolMeta { name: "add_unique_constraint",      category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "alter_column_type",          category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "alter_index",                category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "alter_role",                 category: Security,   needs_db: true,  write: true  },
    ToolMeta { name: "alter_user",                 category: Security,   needs_db: true,  write: true  },
    ToolMeta { name: "alter_view",                 category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "analyze_db_health",          category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "analyze_table",              category: Admin,      needs_db: true,  write: true  },
    ToolMeta { name: "analyze_table_bloat",        category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "async_batch_delete",         category: Batch,      needs_db: true,  write: true  },
    ToolMeta { name: "async_batch_insert",         category: Batch,      needs_db: true,  write: true  },
    ToolMeta { name: "async_batch_insert_copy",    category: Batch,      needs_db: true,  write: true  },
    ToolMeta { name: "async_batch_update",         category: Batch,      needs_db: true,  write: true  },
    ToolMeta { name: "async_execute_delete",       category: Query,      needs_db: true,  write: true  },
    ToolMeta { name: "async_execute_insert",       category: Query,      needs_db: true,  write: true  },
    ToolMeta { name: "async_execute_update",       category: Query,      needs_db: true,  write: true  },
    ToolMeta { name: "backup_table",               category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "bm25_force_merge",           category: Extensions, needs_db: true,  write: true  },
    ToolMeta { name: "bm25_index_stats",           category: Extensions, needs_db: true,  write: false },
    ToolMeta { name: "cancel_query",               category: Admin,      needs_db: true,  write: true  },
    ToolMeta { name: "clone_table_schema",         category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "compress_chunk",             category: Extensions, needs_db: true,  write: true  },
    ToolMeta { name: "create_bm25_index",          category: Extensions, needs_db: true,  write: true  },
    ToolMeta { name: "create_database",            category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "create_extension",           category: Extensions, needs_db: true,  write: true  },
    ToolMeta { name: "create_hypertable",          category: Extensions, needs_db: true,  write: true  },
    ToolMeta { name: "create_index",               category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "create_partition",           category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "create_role",                category: Security,   needs_db: true,  write: true  },
    ToolMeta { name: "create_schema",              category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "create_sequence",            category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "create_table",               category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "create_user",                category: Security,   needs_db: true,  write: true  },
    ToolMeta { name: "create_vector_index",        category: Extensions, needs_db: true,  write: true  },
    ToolMeta { name: "create_view",                category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "describe_table",             category: Schema,     needs_db: true,  write: false },
    ToolMeta { name: "drop_bm25_index",            category: Extensions, needs_db: true,  write: true  },
    ToolMeta { name: "drop_column",                category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "drop_constraint",            category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "drop_extension",             category: Extensions, needs_db: true,  write: true  },
    ToolMeta { name: "drop_foreign_key",           category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "drop_index",                 category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "drop_partition",             category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "drop_role",                  category: Security,   needs_db: true,  write: true  },
    ToolMeta { name: "drop_schema",                category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "drop_sequence",              category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "drop_table",                 category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "drop_user",                  category: Security,   needs_db: true,  write: true  },
    ToolMeta { name: "drop_view",                  category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "execute_delete",             category: Query,      needs_db: true,  write: true  },
    ToolMeta { name: "execute_insert",             category: Query,      needs_db: true,  write: true  },
    ToolMeta { name: "execute_query",              category: Query,      needs_db: true,  write: false },
    ToolMeta { name: "execute_update",             category: Query,      needs_db: true,  write: true  },
    ToolMeta { name: "explain_query",              category: Query,      needs_db: true,  write: false },
    ToolMeta { name: "export_csv",                 category: DataIo,     needs_db: true,  write: false },
    ToolMeta { name: "find_missing_fk_indexes",    category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "find_tables_without_pk",     category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "generate_create_index_ddl",  category: Schema,     needs_db: true,  write: false },
    ToolMeta { name: "generate_create_table_ddl",  category: Schema,     needs_db: true,  write: false },
    ToolMeta { name: "get_cache_hit_ratio",        category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "get_index_stats",            category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "get_object_details",         category: Schema,     needs_db: true,  write: false },
    ToolMeta { name: "get_pg_stat_statements",     category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "get_setting",                category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "get_table_stats",            category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "grant_privileges",           category: Security,   needs_db: true,  write: true  },
    ToolMeta { name: "import_from_url",            category: DataIo,     needs_db: true,  write: true  },
    ToolMeta { name: "list_bm25_indexes",          category: Extensions, needs_db: true,  write: false },
    ToolMeta { name: "list_connections",           category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "list_database_privileges",   category: Security,   needs_db: true,  write: false },
    ToolMeta { name: "list_databases",             category: Schema,     needs_db: true,  write: false },
    ToolMeta { name: "list_duplicate_indexes",     category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "list_extensions",            category: Extensions, needs_db: true,  write: false },
    ToolMeta { name: "list_indexes",               category: Schema,     needs_db: true,  write: false },
    ToolMeta { name: "list_partitions",            category: Schema,     needs_db: true,  write: false },
    ToolMeta { name: "list_replication_slots",     category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "list_role_memberships",      category: Security,   needs_db: true,  write: false },
    ToolMeta { name: "list_schemas",               category: Schema,     needs_db: false, write: false },
    ToolMeta { name: "list_standby_servers",       category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "list_tables",                category: Schema,     needs_db: false, write: false },
    ToolMeta { name: "list_triggers",              category: Schema,     needs_db: true,  write: false },
    ToolMeta { name: "list_unused_indexes",        category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "list_user_privileges",       category: Security,   needs_db: true,  write: false },
    ToolMeta { name: "list_users",                 category: Security,   needs_db: true,  write: false },
    ToolMeta { name: "list_vector_columns",        category: Extensions, needs_db: true,  write: false },
    ToolMeta { name: "reindex_database",           category: Admin,      needs_db: true,  write: true  },
    ToolMeta { name: "reindex_table",              category: Admin,      needs_db: true,  write: true  },
    ToolMeta { name: "rename_column",              category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "rename_index",               category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "rename_schema",              category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "rename_table",               category: Ddl,        needs_db: true,  write: true  },
    ToolMeta { name: "reset_statistics",           category: Admin,      needs_db: true,  write: true  },
    ToolMeta { name: "revoke_privileges",          category: Security,   needs_db: true,  write: true  },
    ToolMeta { name: "sample_data",                category: Query,      needs_db: true,  write: false },
    ToolMeta { name: "search_bm25",                category: Extensions, needs_db: true,  write: false },
    ToolMeta { name: "show_active_transactions",   category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_all_settings",          category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_autocommit_status",     category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_base_backup_progress",  category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_blocked_queries",       category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_chunks",                category: Extensions, needs_db: true,  write: false },
    ToolMeta { name: "show_connection_summary",    category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_constraints",           category: Schema,     needs_db: false, write: false },
    ToolMeta { name: "show_current_user",          category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_database_size",         category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_deadlocks",             category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_hypertable_details",    category: Extensions, needs_db: true,  write: false },
    ToolMeta { name: "show_locks",                 category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_log_settings",          category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_memory_settings",       category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_performance_settings",  category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_replication_status",    category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_running_queries",       category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_session_info",          category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_table_size",            category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_transaction_isolation", category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_transaction_timeout",   category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_vacuum_progress",       category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_waiting_locks",         category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "show_wal_info",              category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "suggest_indexes",            category: Monitoring, needs_db: true,  write: false },
    ToolMeta { name: "table_dependencies",         category: Schema,     needs_db: true,  write: false },
    ToolMeta { name: "terminate_connection",       category: Admin,      needs_db: true,  write: true  },
    ToolMeta { name: "truncate_table",             category: Admin,      needs_db: true,  write: true  },
    ToolMeta { name: "vacuum",                     category: Admin,      needs_db: true,  write: true  },
    ToolMeta { name: "vacuum_analyze",             category: Admin,      needs_db: true,  write: true  },
    ToolMeta { name: "vacuum_full",                category: Admin,      needs_db: true,  write: true  },
    ToolMeta { name: "vector_search",              category: Extensions, needs_db: true,  write: false },
];

#[inline]
fn lookup(name: &str) -> Option<&'static ToolMeta> {
    ALL_TOOLS
        .binary_search_by(|t| t.name.as_bytes().cmp(name.as_bytes()))
        .ok()
        .map(|i| &ALL_TOOLS[i])
}

#[inline]
pub fn tool_exists(name: &str) -> bool {
    lookup(name).is_some()
}

#[inline]
pub fn is_write_tool(name: &str) -> bool {
    lookup(name).map(|t| t.write).unwrap_or(false)
}

#[inline]
pub fn needs_db(name: &str) -> bool {
    lookup(name).map(|t| t.needs_db).unwrap_or(false)
}

/// The category a tool belongs to, or `None` if the tool is unknown.
#[inline]
pub fn category_of(name: &str) -> Option<ToolCategory> {
    lookup(name).map(|t| t.category)
}

/// Whether a tool is callable given the set of enabled categories. A tool is
/// available only if it exists *and* its category is enabled. Unknown tools and
/// tools in disabled categories are both treated as unavailable.
#[inline]
pub fn is_tool_available(name: &str, enabled: &[ToolCategory]) -> bool {
    category_of(name).is_some_and(|c| enabled.contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tools_unique() {
        let mut names: Vec<&str> = ALL_TOOLS.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(
            names.len(),
            ALL_TOOLS.len(),
            "Duplicate tool names in ALL_TOOLS"
        );
    }

    #[test]
    fn test_all_tools_sorted() {
        // binary_search in `lookup` requires the array to stay name-sorted.
        let mut sorted = ALL_TOOLS.iter().map(|t| t.name).collect::<Vec<_>>();
        sorted.sort_unstable();
        let actual = ALL_TOOLS.iter().map(|t| t.name).collect::<Vec<_>>();
        assert_eq!(actual, sorted, "ALL_TOOLS must be sorted by name");
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
    fn test_every_tool_has_category() {
        // category_of must resolve for every registered tool.
        for meta in ALL_TOOLS {
            assert_eq!(category_of(meta.name), Some(meta.category));
        }
        assert_eq!(category_of("nonexistent_tool"), None);
    }

    #[test]
    fn test_is_tool_available_gating() {
        // Disabled by default: empty set exposes nothing.
        assert!(!is_tool_available("execute_query", &[]));
        // Enabling the owning category exposes it.
        assert!(is_tool_available("execute_query", &[ToolCategory::Query]));
        // A different category does not.
        assert!(!is_tool_available("execute_query", &[ToolCategory::Ddl]));
        // Unknown tools are never available, even with everything enabled.
        assert!(!is_tool_available("nonexistent_tool", ToolCategory::ALL));
    }

    #[test]
    fn test_category_slug_roundtrip() {
        for &cat in ToolCategory::ALL {
            assert_eq!(cat.slug().parse::<ToolCategory>().unwrap(), cat);
        }
        // Accepts underscores as a convenience alias.
        assert_eq!("data_io".parse::<ToolCategory>().unwrap(), ToolCategory::DataIo);
        assert!("bogus".parse::<ToolCategory>().is_err());
    }

    #[test]
    fn test_categories_within_limit() {
        assert!(
            ToolCategory::ALL.len() <= 10,
            "tool categories must map to at most ten CLI flags"
        );
    }

    #[test]
    fn test_all_tools_registered_in_tools_json() {
        let content = std::fs::read_to_string("tools.json").expect("Failed to read tools.json");
        let json: serde_json::Value =
            serde_json::from_str(&content).expect("tools.json is not valid JSON");
        let json_tools = json.as_array().expect("tools.json must be an array");

        let json_names: Vec<&str> = json_tools
            .iter()
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

        assert_eq!(
            json_names.len(),
            ALL_TOOLS.len(),
            "tools.json has {} tools but ALL_TOOLS has {}",
            json_names.len(),
            ALL_TOOLS.len()
        );
    }
}
