pub mod actions;
pub mod auth;
pub mod config;
pub mod errors;
pub mod http;
pub mod lockfree_pool;
pub mod metrics;
pub mod pool;
pub mod protocol;
pub mod server;
pub mod ssrf;
pub mod tls;
pub mod tools;
pub mod validation;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "MCP PostgreSQL Server")]
#[command(about = "High-performance Model Context Protocol server for PostgreSQL", long_about = None)]
pub struct Args {
    /// PostgreSQL connection string
    #[arg(short, long)]
    pub database_url: Option<String>,

    /// Server host
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    pub host: String,

    /// TCP server port
    #[arg(short = 'p', long, default_value = "3000")]
    pub port: u16,

    /// HTTP server port
    #[arg(long, default_value = "3001")]
    pub http_port: u16,

    /// Minimum pool connections (default: 1)
    #[arg(long)]
    pub min_connections: Option<u32>,

    /// Maximum pool connections (default: 8 * num_cpus)
    #[arg(long)]
    pub max_connections: Option<u32>,

    /// Seconds to wait for a free pooled connection before returning a pool
    /// timeout error. `0` fails immediately when the pool is saturated
    /// (non-blocking). Default: 10.
    #[arg(long, default_value = "10")]
    pub acquire_timeout_secs: u64,

    /// Log level
    #[arg(short, long, default_value = "info")]
    pub log_level: String,

    /// Enable metrics endpoint
    #[arg(long)]
    pub enable_metrics: bool,

    /// Metrics port
    #[arg(long, default_value = "9090")]
    pub metrics_port: u16,

    /// Run in stdio mode for MCP compatibility (Claude Desktop)
    #[arg(long)]
    pub stdio: bool,

    /// Access mode: unrestricted (full read/write) or restricted (read-only)
    #[arg(long, default_value = "unrestricted")]
    pub access_mode: config::AccessMode,

    /// Shared secret required for TCP/HTTP requests (falls back to env
    /// MCP_AUTH_TOKEN). Required when binding to a non-loopback address.
    /// Not used in stdio mode.
    #[arg(long)]
    pub auth_token: Option<String>,

    /// Allow the import_from_url tool to make outbound HTTP fetches.
    /// Disabled by default to reduce SSRF exposure.
    #[arg(long)]
    pub allow_url_import: bool,

    /// Path to a PEM certificate chain to serve the HTTP transport over TLS
    /// (HTTPS). Requires --tls-key. Falls back to the MCP_TLS_CERT env var.
    /// When unset, the HTTP transport stays plaintext.
    #[arg(long)]
    pub tls_cert: Option<String>,

    /// Path to the PEM private key matching --tls-cert. Falls back to the
    /// MCP_TLS_KEY env var.
    #[arg(long)]
    pub tls_key: Option<String>,

    // ── Tool exposure ────────────────────────────────────────────────────
    // No tools are exposed unless explicitly enabled. Each flag below turns on
    // one category of tools (hidden from tools/list and rejected from
    // tools/call when its category is disabled). Use --enable-all for every
    // category at once.
    /// Expose ALL tool categories (overrides the individual flags).
    #[arg(long)]
    pub enable_all: bool,

    /// Enable Query tools: execute/explain/async-execute SQL and data sampling.
    #[arg(long)]
    pub enable_query: bool,

    /// Enable Batch tools: bulk insert/update/delete and COPY ingestion.
    #[arg(long)]
    pub enable_batch: bool,

    /// Enable Schema tools: read-only inspection and DDL generation.
    #[arg(long)]
    pub enable_schema: bool,

    /// Enable DDL tools: create/drop/alter/rename of database objects.
    #[arg(long)]
    pub enable_ddl: bool,

    /// Enable Admin tools: vacuum, reindex, analyze, truncate, cancel/terminate.
    #[arg(long)]
    pub enable_admin: bool,

    /// Enable Monitoring tools: stats, connections, transactions, replication,
    /// configuration, and health checks.
    #[arg(long)]
    pub enable_monitoring: bool,

    /// Enable Security tools: roles, users, privileges, and security audits.
    #[arg(long)]
    pub enable_security: bool,

    /// Enable Data I/O tools: CSV export and URL/file import.
    #[arg(long)]
    pub enable_data_io: bool,

    /// Enable Extension tools: pgvector, TimescaleDB, BM25, extension mgmt.
    #[arg(long)]
    pub enable_extensions: bool,
}

impl Args {
    /// Resolve the set of enabled tool categories from the `--enable-*` flags.
    /// `--enable-all` turns on every category; otherwise only the categories
    /// whose individual flag is set are enabled. With no flags, the result is
    /// empty and no tools are exposed.
    pub fn enabled_categories(&self) -> Vec<tools::ToolCategory> {
        use tools::ToolCategory as C;
        if self.enable_all {
            return C::ALL.to_vec();
        }
        let mut cats = Vec::new();
        let mut push = |on: bool, cat: C| {
            if on {
                cats.push(cat);
            }
        };
        push(self.enable_query, C::Query);
        push(self.enable_batch, C::Batch);
        push(self.enable_schema, C::Schema);
        push(self.enable_ddl, C::Ddl);
        push(self.enable_admin, C::Admin);
        push(self.enable_monitoring, C::Monitoring);
        push(self.enable_security, C::Security);
        push(self.enable_data_io, C::DataIo);
        push(self.enable_extensions, C::Extensions);
        cats
    }
}
