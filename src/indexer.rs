//! JSON-based indexer for blocks and transactions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{Result, RpcError};
use crate::types::{IndexedBlock, IndexedTransaction};

/// Indexed data stored in JSON
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexedData {
    pub last_block: u64,
    pub blocks: HashMap<u64, IndexedBlock>,
    pub transactions: HashMap<String, IndexedTransaction>,
}

/// JSON-based indexer
#[derive(Clone)]
pub struct Indexer {
    data: Arc<RwLock<IndexedData>>,
    file_path: String,
}

impl Indexer {
    /// Create a new indexer, loading existing data if available
    pub fn new(file_path: &str) -> Result<Self> {
        let path = Path::new(file_path);
        let data = if path.exists() {
            let content = fs::read_to_string(file_path)
                .map_err(|e| RpcError::InvalidResponse(format!("Failed to read index: {}", e)))?;
            
            // Handle empty files
            if content.trim().is_empty() {
                IndexedData::default()
            } else {
                serde_json::from_str(&content)
                    .map_err(|e| RpcError::InvalidResponse(format!("Failed to parse index: {}", e)))?
            }
        } else {
            IndexedData::default()
        };

        Ok(Self {
            data: Arc::new(RwLock::new(data)),
            file_path: file_path.to_string(),
        })
    }

    /// Get the last indexed block number
    pub async fn last_block(&self) -> u64 {
        self.data.read().await.last_block
    }

    /// Check if a block is already indexed
    pub async fn has_block(&self, block_number: u64) -> bool {
        self.data.read().await.blocks.contains_key(&block_number)
    }

    /// Index a block and its transactions
    pub async fn index_block(
        &self,
        block: IndexedBlock,
        transactions: Vec<IndexedTransaction>,
    ) -> Result<()> {
        let mut data = self.data.write().await;

        let block_number = block.number;

        // Store block
        data.blocks.insert(block_number, block);

        // Store transactions
        for tx in transactions {
            data.transactions.insert(tx.hash.clone(), tx);
        }

        // Update last block if this is newer
        if block_number > data.last_block {
            data.last_block = block_number;
        }

        // Persist to file
        self.save_to_file(&data)?;

        Ok(())
    }

    /// Get a block by number
    pub async fn get_block(&self, block_number: u64) -> Option<IndexedBlock> {
        self.data.read().await.blocks.get(&block_number).cloned()
    }

    /// Get a transaction by hash
    #[allow(dead_code)]
    pub async fn get_transaction(&self, hash: &str) -> Option<IndexedTransaction> {
        self.data.read().await.transactions.get(hash).cloned()
    }

    /// Get indexer stats
    pub async fn stats(&self) -> IndexerStats {
        let data = self.data.read().await;
        IndexerStats {
            last_block: data.last_block,
            total_blocks: data.blocks.len(),
            total_transactions: data.transactions.len(),
        }
    }

    /// Save data to file
    fn save_to_file(&self, data: &IndexedData) -> Result<()> {
        let content = serde_json::to_string_pretty(data)
            .map_err(|e| RpcError::InvalidResponse(format!("Failed to serialize: {}", e)))?;

        fs::write(&self.file_path, content)
            .map_err(|e| RpcError::InvalidResponse(format!("Failed to write: {}", e)))?;

        Ok(())
    }
}

/// Indexer statistics
#[derive(Debug, Clone, Serialize)]
pub struct IndexerStats {
    pub last_block: u64,
    pub total_blocks: usize,
    pub total_transactions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_indexer_basic() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_string_lossy().to_string();

        let indexer = Indexer::new(&path).unwrap();

        // Initially empty
        assert_eq!(indexer.last_block().await, 0);
        assert!(!indexer.has_block(1).await);

        // Index a block
        let block = IndexedBlock {
            number: 1,
            hash: "0xabc".to_string(),
            timestamp: 1234567890,
            tx_count: 2,
        };

        let txs = vec![
            IndexedTransaction {
                hash: "0xtx1".to_string(),
                from: "0xfrom".to_string(),
                to: Some("0xto".to_string()),
                value_wei: "1000".to_string(),
                value_eth: 0.000001,
                gas: 21000,
                block_number: 1,
            },
            IndexedTransaction {
                hash: "0xtx2".to_string(),
                from: "0xfrom2".to_string(),
                to: None,
                value_wei: "0".to_string(),
                value_eth: 0.0,
                gas: 100000,
                block_number: 1,
            },
        ];

        indexer.index_block(block, txs).await.unwrap();

        // Verify indexed
        assert_eq!(indexer.last_block().await, 1);
        assert!(indexer.has_block(1).await);

        let stored_block = indexer.get_block(1).await.unwrap();
        assert_eq!(stored_block.hash, "0xabc");

        let stored_tx = indexer.get_transaction("0xtx1").await.unwrap();
        assert_eq!(stored_tx.from, "0xfrom");

        // Check stats
        let stats = indexer.stats().await;
        assert_eq!(stats.total_blocks, 1);
        assert_eq!(stats.total_transactions, 2);
    }

    #[tokio::test]
    async fn test_indexer_persistence() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_string_lossy().to_string();

        // Create and index
        {
            let indexer = Indexer::new(&path).unwrap();
            let block = IndexedBlock {
                number: 100,
                hash: "0xpersist".to_string(),
                timestamp: 9999,
                tx_count: 0,
            };
            indexer.index_block(block, vec![]).await.unwrap();
        }

        // Reload and verify
        {
            let indexer = Indexer::new(&path).unwrap();
            assert_eq!(indexer.last_block().await, 100);
            let block = indexer.get_block(100).await.unwrap();
            assert_eq!(block.hash, "0xpersist");
        }
    }
}
