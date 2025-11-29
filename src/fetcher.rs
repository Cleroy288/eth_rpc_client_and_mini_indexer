//! Block fetcher loop - continuously fetches new blocks

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::error::Result;
use crate::indexer::Indexer;
use crate::rpc_client::EthRpcClient;

/// Block fetcher configuration
#[derive(Debug, Clone)]
pub struct FetcherConfig {
    /// Polling interval in milliseconds
    pub poll_interval_ms: u64,
    /// Maximum blocks to fetch per iteration
    pub max_blocks_per_batch: u64,
    /// Number of confirmations before indexing (0 = index immediately)
    pub confirmations: u64,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 2000,
            max_blocks_per_batch: 10,
            confirmations: 0,
        }
    }
}

/// Block fetcher metrics
#[derive(Debug, Default)]
pub struct FetcherMetrics {
    pub blocks_fetched: AtomicU64,
    pub rpc_errors: AtomicU64,
    pub last_block: AtomicU64,
}

impl FetcherMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn inc_blocks_fetched(&self) {
        self.blocks_fetched.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_rpc_errors(&self) {
        self.rpc_errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_last_block(&self, block: u64) {
        self.last_block.store(block, Ordering::Relaxed);
    }

    pub fn get_blocks_fetched(&self) -> u64 {
        self.blocks_fetched.load(Ordering::Relaxed)
    }

    pub fn get_rpc_errors(&self) -> u64 {
        self.rpc_errors.load(Ordering::Relaxed)
    }

    #[allow(dead_code)]
    pub fn get_last_block(&self) -> u64 {
        self.last_block.load(Ordering::Relaxed)
    }
}

/// Block fetcher
pub struct Fetcher {
    client: Arc<EthRpcClient>,
    indexer: Arc<Indexer>,
    config: FetcherConfig,
    metrics: Arc<FetcherMetrics>,
    running: Arc<AtomicBool>,
}

impl Fetcher {
    /// Create a new fetcher
    pub fn new(
        client: Arc<EthRpcClient>,
        indexer: Arc<Indexer>,
        config: FetcherConfig,
        metrics: Arc<FetcherMetrics>,
    ) -> Self {
        Self {
            client,
            indexer,
            config,
            metrics,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the fetcher loop
    pub async fn run(&self, shutdown: Arc<AtomicBool>) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);
        info!("Block fetcher started");

        while !shutdown.load(Ordering::SeqCst) {
            if let Err(e) = self.fetch_new_blocks().await {
                error!("Error fetching blocks: {:?}", e);
                self.metrics.inc_rpc_errors();
            }

            sleep(Duration::from_millis(self.config.poll_interval_ms)).await;
        }

        self.running.store(false, Ordering::SeqCst);
        info!("Block fetcher stopped");
        Ok(())
    }

    /// Fetch and index new blocks
    async fn fetch_new_blocks(&self) -> Result<()> {
        // Get current chain head
        let chain_head = self.client.get_block_number().await?;
        let target_block = chain_head.saturating_sub(self.config.confirmations);

        // Get our last indexed block
        let last_indexed = self.indexer.last_block().await;

        if last_indexed >= target_block {
            return Ok(()); // Already up to date
        }

        // Calculate range to fetch
        let start = last_indexed + 1;
        let end = std::cmp::min(start + self.config.max_blocks_per_batch - 1, target_block);

        info!(
            "Fetching blocks {} to {} (chain head: {})",
            start, end, chain_head
        );

        // Fetch and index each block
        for block_num in start..=end {
            if let Err(e) = self.fetch_and_index_block(block_num).await {
                warn!("Failed to fetch block {}: {:?}", block_num, e);
                self.metrics.inc_rpc_errors();
                break; // Stop on error to maintain sequential indexing
            }
        }

        Ok(())
    }

    /// Fetch a single block and index it
    async fn fetch_and_index_block(&self, block_number: u64) -> Result<()> {
        // Skip if already indexed
        if self.indexer.has_block(block_number).await {
            return Ok(());
        }

        // Fetch block from RPC
        let block = self
            .client
            .get_block_by_number(block_number)
            .await?
            .ok_or_else(|| {
                crate::error::RpcError::InvalidResponse(format!("Block {} not found", block_number))
            })?;

        // Convert to indexed format
        let indexed_block = block.to_indexed().ok_or_else(|| {
            crate::error::RpcError::ParseError("Failed to parse block".to_string())
        })?;

        let indexed_txs: Vec<_> = block
            .transactions
            .iter()
            .map(|tx| tx.to_indexed(block_number))
            .collect();

        let tx_count = indexed_txs.len();

        // Store in indexer
        self.indexer.index_block(indexed_block, indexed_txs).await?;

        // Update metrics
        self.metrics.inc_blocks_fetched();
        self.metrics.set_last_block(block_number);

        info!(
            "Indexed block {} with {} transactions",
            block_number, tx_count
        );

        Ok(())
    }

    /// Fetch a specific range of blocks (for backfill)
    pub async fn fetch_range(&self, start: u64, end: u64) -> Result<u64> {
        let mut fetched = 0;

        for block_num in start..=end {
            if let Err(e) = self.fetch_and_index_block(block_num).await {
                warn!("Failed to fetch block {}: {:?}", block_num, e);
                break;
            }
            fetched += 1;
        }

        Ok(fetched)
    }

    /// Check if fetcher is running
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_config_default() {
        let config = FetcherConfig::default();
        assert_eq!(config.poll_interval_ms, 2000);
        assert_eq!(config.max_blocks_per_batch, 10);
    }

    #[test]
    fn test_fetcher_metrics() {
        let metrics = FetcherMetrics::new();

        metrics.inc_blocks_fetched();
        metrics.inc_blocks_fetched();
        assert_eq!(metrics.get_blocks_fetched(), 2);

        metrics.inc_rpc_errors();
        assert_eq!(metrics.get_rpc_errors(), 1);

        metrics.set_last_block(12345);
        assert_eq!(metrics.get_last_block(), 12345);
    }
}
