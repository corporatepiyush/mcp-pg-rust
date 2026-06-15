pub mod config;
pub mod server;
pub mod pool;
pub mod protocol;
pub mod actions;
pub mod errors;
pub mod metrics;
pub mod http;
pub mod validation;
pub mod tools;
pub mod lockfree_pool;

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
}
