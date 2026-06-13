mod config;
mod server;
mod pool;
mod protocol;
mod actions;
mod errors;
mod metrics;

use anyhow::Result;
use clap::Parser;
use tracing::info;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Parser, Debug)]
#[command(name = "MCP PostgreSQL Server")]
#[command(about = "High-performance Model Context Protocol server for PostgreSQL", long_about = None)]
struct Args {
    /// PostgreSQL connection string
    #[arg(short, long)]
    database_url: Option<String>,

    /// Server host
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    host: String,

    /// Server port
    #[arg(short = 'p', long, default_value = "3000")]
    port: u16,

    /// Minimum pool connections
    #[arg(long, default_value = "5")]
    min_connections: u32,

    /// Maximum pool connections
    #[arg(long, default_value = "20")]
    max_connections: u32,

    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Enable metrics endpoint
    #[arg(long)]
    enable_metrics: bool,

    /// Metrics port
    #[arg(long, default_value = "9090")]
    metrics_port: u16,

    /// Run in stdio mode for MCP compatibility (Claude Desktop)
    #[arg(long)]
    stdio: bool,

    /// Access mode: unrestricted (full read/write) or restricted (read-only)
    #[arg(long, default_value = "unrestricted")]
    access_mode: config::AccessMode,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Configure mimalloc before any allocations
    // Disable page reset (faster short-lived alloc reuse), set decommit delay, eager commit
    std::env::set_var("MIMALLOC_PAGE_RESET", "0");
    std::env::set_var("MIMALLOC_DECOMMIT_DELAY", "500");
    std::env::set_var("MIMALLOC_ARENA_EAGER_COMMIT", "1");

    let args = Args::parse();

    // Initialize logging
    init_tracing(&args.log_level)?;

    info!("Starting MCP PostgreSQL Server");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = config::Config::from_args(&args)?;

    // Initialize metrics if enabled
    if args.enable_metrics {
        metrics::init_metrics(args.metrics_port)?;
        info!("Metrics enabled on port {}", args.metrics_port);
    }

    // Create connection pool
    let pool = std::sync::Arc::new(
        pool::ConnectionPool::new(&config.database.url, config.pool.clone()).await?
    );
    info!("Connection pool initialized: min={}, max={}",
        config.pool.min_size, config.pool.max_size);

    // Create server
    let server = server::MCPServer::new(config, pool);
    info!("Server initialized successfully");

    // Run server (TCP or stdio mode)
    if args.stdio {
        info!("Running in stdio mode");
        server.run_stdio().await?;
    } else {
        server.run().await?;
    }

    info!("Server shutdown complete");
    Ok(())
}

fn init_tracing(log_level: &str) -> Result<()> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(log_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    Ok(())
}
