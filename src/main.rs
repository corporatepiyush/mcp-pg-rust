use anyhow::Result;
use clap::Parser;
use mcp_postgres::{Args, config, http, metrics, pool, server};
use tracing::info;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> Result<()> {
    // NOTE: mimalloc reads its `MIMALLOC_*` tuning env vars once, at allocator
    // init, which happens before `main` runs (the `#[global_allocator]` is live
    // from the first allocation). Setting them here had no effect, so the block
    // was removed. To tune mimalloc, export the vars in the process environment
    // before launch, or configure them via the mimalloc crate's build features.

    let args = Args::parse();

    // Install the rustls `ring` crypto provider as the process default up front.
    // Postgres TLS installs it lazily (src/tls.rs), but the data-import HTTP
    // client (reqwest, built with `rustls-no-provider`) relies on the process
    // default for any HTTPS fetch that happens without a prior Postgres TLS
    // connection. Idempotent — the first install wins.
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Initialize logging
    init_tracing(&args.log_level)?;

    info!("Starting MCP PostgreSQL Server");
    info!("Version: {}", env!("CARGO_PKG_VERSION"));
    log_mimalloc_version();

    // Load configuration
    let config = config::Config::from_args(&args)?;

    // Security: refuse to expose a network transport without authentication
    // when bound to a non-loopback address. Loopback-only binds remain open
    // for local development; stdio mode is a trusted local pipe.
    if !args.stdio
        && config.server.auth_token.is_none()
        && !mcp_postgres::auth::is_loopback_host(&config.server.host)
    {
        anyhow::bail!(
            "Refusing to bind to non-loopback host '{}' without an auth token. \
             Set --auth-token or the MCP_AUTH_TOKEN env var, or bind to a loopback address.",
            config.server.host
        );
    }
    if config.server.auth_token.is_some() {
        info!("Transport authentication: ENABLED (token required on TCP and HTTP)");
    }

    // Initialize metrics if enabled
    if args.enable_metrics {
        metrics::init_metrics(args.metrics_port)?;
        info!("Metrics enabled on port {}", args.metrics_port);
    }

    // Create connection pool. The server's request_timeout is enforced at the
    // database as a per-connection statement_timeout so no single query can pin
    // a pooled connection indefinitely. In restricted mode, connections are also
    // set read-only so writes are rejected at the database, not just by tool name.
    let read_only = config.server.access_mode == config::AccessMode::Restricted;
    let pool = std::sync::Arc::new(
        pool::ConnectionPool::with_session_setup(
            &config.database.url,
            config.pool.clone(),
            config.server.request_timeout,
            read_only,
        )
        .await?,
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

/// Log the linked mimalloc version and warn if it is not v3.
///
/// v3 is selected by the absence of `libmimalloc-sys/v2`. Cargo feature
/// unification is additive, so a transitive `v2` feature would silently
/// downgrade the allocator — this surfaces that instead of hiding it.
fn log_mimalloc_version() {
    // SAFETY: mi_version() is a pure read of a compile-time constant.
    let v = unsafe { libmimalloc_sys::mi_version() };
    info!(
        "mimalloc version: {}.{}.{}",
        v / 10000,
        (v / 100) % 100,
        v % 100
    );
    if v < 30000 {
        tracing::warn!(
            "Expected mimalloc v3 but linked v{v}; a dependency enabled the `v2` feature"
        );
    }
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
