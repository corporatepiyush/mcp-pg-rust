use anyhow::Result;
use clap::Parser;
use tracing::info;
use mcp_postgres::{config, pool, server, metrics, Args};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

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
