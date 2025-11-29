//! Ethereum RPC Client & Mini Indexer
//!
//! A lightweight Rust service for querying Ethereum blocks and transactions.

mod error;
mod fetcher;
mod indexer;
mod metrics;
mod rpc_client;
mod types;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use fetcher::{Fetcher, FetcherConfig, FetcherMetrics};
use indexer::Indexer;
use metrics::{AppState, RpcLatencyMetrics};
use rpc_client::EthRpcClient;
use tokio::sync::RwLock;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

const DEFAULT_RPC_ENDPOINT: &str = "https://ethereum-rpc.publicnode.com";
const DEFAULT_INDEX_FILE: &str = "data/index.json";
const DEFAULT_METRICS_PORT: u16 = 9090;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if present
    let _ = dotenvy::dotenv();

    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Ethereum RPC Client & Mini Indexer");

    // Get config from env
    let rpc_endpoint =
        std::env::var("ETH_RPC_ENDPOINT").unwrap_or_else(|_| DEFAULT_RPC_ENDPOINT.to_string());
    let index_file =
        std::env::var("INDEX_FILE").unwrap_or_else(|_| DEFAULT_INDEX_FILE.to_string());
    let metrics_port: u16 = std::env::var("METRICS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_METRICS_PORT);

    // Ensure data directory exists
    if let Some(parent) = std::path::Path::new(&index_file).parent() {
        std::fs::create_dir_all(parent)?;
    }

    info!("RPC endpoint: {}", rpc_endpoint);
    info!("Index file: {}", index_file);

    // Create components
    let client = Arc::new(EthRpcClient::new(&rpc_endpoint)?);
    let indexer = Arc::new(Indexer::new(&index_file)?);
    let fetcher_metrics = Arc::new(FetcherMetrics::new());
    let rpc_latency = Arc::new(RwLock::new(RpcLatencyMetrics::default()));

    // Show current state
    let stats = indexer.stats().await;
    info!(
        "Indexer state: last_block={}, blocks={}, transactions={}",
        stats.last_block, stats.total_blocks, stats.total_transactions
    );

    // Get current chain head
    let chain_head = client.get_block_number().await?;
    info!("Current Ethereum block number: {}", chain_head);

    // Check run mode
    let mode = std::env::var("RUN_MODE").unwrap_or_else(|_| "test".to_string());

    match mode.as_str() {
        "server" => run_server_mode(client, indexer, fetcher_metrics, rpc_latency, metrics_port).await,
        "continuous" => run_continuous(client, indexer, fetcher_metrics, rpc_latency, metrics_port).await,
        "fetch" => run_fetch_blocks(client, indexer, fetcher_metrics).await,
        _ => run_test_mode(client, indexer).await,
    }
}

/// Run in test mode - fetch a few blocks and display results
async fn run_test_mode(
    client: Arc<EthRpcClient>,
    indexer: Arc<Indexer>,
) -> anyhow::Result<()> {
    info!("Running in TEST mode");

    let chain_head = client.get_block_number().await?;
    let start_block = chain_head.saturating_sub(2);
    
    for block_num in start_block..=chain_head {
        info!("Fetching block {}...", block_num);
        
        if let Some(block) = client.get_block_by_number(block_num).await? {
            let indexed_block = block.to_indexed().ok_or_else(|| {
                anyhow::anyhow!("Failed to parse block {}", block_num)
            })?;
            
            let indexed_txs: Vec<_> = block
                .transactions
                .iter()
                .map(|tx| tx.to_indexed(block_num))
                .collect();
            
            let tx_count = indexed_txs.len();
            indexer.index_block(indexed_block, indexed_txs).await?;
            
            info!(
                "Indexed block {}: hash={}..., {} transactions",
                block_num, &block.hash[..16], tx_count
            );
        }
    }

    let stats = indexer.stats().await;
    info!(
        "Final state: last_block={}, blocks={}, transactions={}",
        stats.last_block, stats.total_blocks, stats.total_transactions
    );

    info!("Test mode completed successfully!");
    Ok(())
}

/// Run fetch mode - fetch a batch of blocks
async fn run_fetch_blocks(
    client: Arc<EthRpcClient>,
    indexer: Arc<Indexer>,
    metrics: Arc<FetcherMetrics>,
) -> anyhow::Result<()> {
    info!("Running in FETCH mode");

    let config = FetcherConfig::default();
    let fetcher = Fetcher::new(client, indexer.clone(), config, metrics.clone());

    let _chain_head = fetcher.fetch_range(0, 0).await?;
    
    let last_indexed = indexer.last_block().await;
    let target = last_indexed + 10;
    
    info!("Fetching blocks {} to {}...", last_indexed + 1, target);
    let fetched = fetcher.fetch_range(last_indexed + 1, target).await?;
    
    info!("Fetched {} blocks", fetched);
    info!("Metrics: blocks_fetched={}, rpc_errors={}", 
        metrics.get_blocks_fetched(), 
        metrics.get_rpc_errors()
    );

    let stats = indexer.stats().await;
    info!(
        "Final state: last_block={}, blocks={}, transactions={}",
        stats.last_block, stats.total_blocks, stats.total_transactions
    );

    Ok(())
}


/// Run server mode - only metrics server, no fetching
async fn run_server_mode(
    _client: Arc<EthRpcClient>,
    indexer: Arc<Indexer>,
    fetcher_metrics: Arc<FetcherMetrics>,
    rpc_latency: Arc<RwLock<RpcLatencyMetrics>>,
    port: u16,
) -> anyhow::Result<()> {
    info!("Running in SERVER mode (metrics only)");

    let app_state = AppState {
        indexer,
        fetcher_metrics,
        rpc_latency,
        start_time: Instant::now(),
    };

    let app = metrics::metrics_router(app_state);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    info!("Metrics server listening on http://{}", addr);
    info!("  /metrics - Prometheus metrics");
    info!("  /health  - Health check");
    info!("  /stats   - JSON stats");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Run continuous mode - fetching + metrics server
async fn run_continuous(
    client: Arc<EthRpcClient>,
    indexer: Arc<Indexer>,
    fetcher_metrics: Arc<FetcherMetrics>,
    rpc_latency: Arc<RwLock<RpcLatencyMetrics>>,
    port: u16,
) -> anyhow::Result<()> {
    info!("Running in CONTINUOUS mode (fetcher + metrics server)");

    let app_state = AppState {
        indexer: indexer.clone(),
        fetcher_metrics: fetcher_metrics.clone(),
        rpc_latency,
        start_time: Instant::now(),
    };

    // Start metrics server in background
    let app = metrics::metrics_router(app_state);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    info!("Metrics server listening on http://{}", addr);
    
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    // Setup fetcher
    let config = FetcherConfig {
        poll_interval_ms: 2000,
        max_blocks_per_batch: 5,
        confirmations: 1,
    };
    
    let fetcher = Fetcher::new(client, indexer.clone(), config, fetcher_metrics.clone());
    let shutdown = Arc::new(AtomicBool::new(false));
    
    // Setup Ctrl+C handler
    let shutdown_clone = shutdown.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Shutdown signal received");
        shutdown_clone.store(true, Ordering::SeqCst);
    });

    // Run fetcher loop
    fetcher.run(shutdown).await?;

    let stats = indexer.stats().await;
    info!(
        "Final state: last_block={}, blocks={}, transactions={}",
        stats.last_block, stats.total_blocks, stats.total_transactions
    );

    Ok(())
}
