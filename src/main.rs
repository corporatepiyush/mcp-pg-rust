use anyhow::Result;
use clap::Parser;
use mcp_postgres::{Args, config, http, metrics, pool, server};
use tracing::info;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> Result<()> {
    // Configure mimalloc v3 before any allocations
    // Memory efficiency for high-throughput server
    // SAFETY: set_var is unsafe in Rust 2024 due to potential data races,
    // but this runs in single-threaded context before any threads are spawned.
    unsafe { std::env::set_var("MIMALLOC_PAGE_RESET", "0") }; // Don't reset pages (reuse faster)
    unsafe { std::env::set_var("MIMALLOC_DECOMMIT_DELAY", "1000") }; // Decommit unused pages after 1s
    unsafe { std::env::set_var("MIMALLOC_ARENA_EAGER_COMMIT", "1") }; // Eager commit for predictable latency
    unsafe { std::env::set_var("MIMALLOC_LARGE_OS_PAGES", "1") }; // Use large pages (2MB) to reduce TLB misses
    unsafe { std::env::set_var("MIMALLOC_EAGER_REGION_COMMIT", "1") }; // Eagerly commit regions for fast allocation
    unsafe { std::env::set_var("MIMALLOC_RESET_DELAY", "0") }; // No delay resetting freed allocations

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
        pool::ConnectionPool::new(&config.database.url, config.pool.clone()).await?,
    );
    info!(
        "Connection pool initialized: min={}, max={}",
        config.pool.min_size, config.pool.max_size
    );

    // Create server
    let mcp_server = server::MCPServer::new(config.clone(), pool.clone());
    info!("Server initialized successfully");

    // Run server (TCP, HTTP, or stdio mode)
    if args.stdio {
        info!("Running in stdio mode");
        mcp_server.run_stdio().await?;
    } else {
        // Start both TCP and HTTP servers in parallel
        info!("Starting TCP server on port {}", args.port);
        info!("Starting HTTP/2 server on port {}", args.http_port);

        let tcp_handle = tokio::spawn(async move {
            if let Err(e) = mcp_server.run().await {
                eprintln!("TCP server error: {}", e);
            }
        });

        let http_config = config.clone();
        let http_pool = pool.clone();
        let http_port = args.http_port;
        let http_handle = tokio::spawn(async move {
            if let Err(e) = http::create_http_server(http_pool, http_config, http_port).await {
                eprintln!("HTTP server error: {}", e);
            }
        });

        // Wait for either server to exit
        tokio::select! {
            _ = tcp_handle => info!("TCP server exited"),
            _ = http_handle => info!("HTTP server exited"),
        }
    }

    info!("Server shutdown complete");
    Ok(())
}

fn init_tracing(log_level: &str) -> Result<()> {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(log_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    Ok(())
}
