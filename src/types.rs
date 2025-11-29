//! Ethereum data types

use serde::{Deserialize, Serialize};

/// Ethereum block with transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub number: String,           // Hex string
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: String,        // Hex string
    pub gas_used: String,
    pub gas_limit: String,
    pub miner: String,
    pub transactions: Vec<Transaction>,
}

/// Ethereum transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,       // None for contract creation
    pub value: String,            // Hex string (wei)
    pub gas: String,
    pub gas_price: Option<String>,
    pub input: String,
    pub nonce: String,
    pub block_number: Option<String>,
    pub block_hash: Option<String>,
    pub transaction_index: Option<String>,
}

/// Parsed block for indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedBlock {
    pub number: u64,
    pub hash: String,
    pub timestamp: u64,
    pub tx_count: usize,
}

/// Parsed transaction for indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedTransaction {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value_wei: String,
    pub value_eth: f64,
    pub gas: u64,
    pub block_number: u64,
}

impl Block {
    /// Parse block number from hex
    pub fn block_number(&self) -> Option<u64> {
        parse_hex_u64(&self.number)
    }

    /// Parse timestamp from hex
    pub fn timestamp_u64(&self) -> Option<u64> {
        parse_hex_u64(&self.timestamp)
    }

    /// Convert to indexed block
    pub fn to_indexed(&self) -> Option<IndexedBlock> {
        Some(IndexedBlock {
            number: self.block_number()?,
            hash: self.hash.clone(),
            timestamp: self.timestamp_u64()?,
            tx_count: self.transactions.len(),
        })
    }
}

impl Transaction {
    /// Convert to indexed transaction
    pub fn to_indexed(&self, block_number: u64) -> IndexedTransaction {
        let value_wei = self.value.clone();
        let value_eth = parse_wei_to_eth(&self.value).unwrap_or(0.0);
        let gas = parse_hex_u64(&self.gas).unwrap_or(0);

        IndexedTransaction {
            hash: self.hash.clone(),
            from: self.from.clone(),
            to: self.to.clone(),
            value_wei,
            value_eth,
            gas,
            block_number,
        }
    }
}

/// Parse hex string to u64
pub fn parse_hex_u64(hex: &str) -> Option<u64> {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    u64::from_str_radix(hex, 16).ok()
}

/// Parse wei (hex) to ETH (f64)
pub fn parse_wei_to_eth(hex: &str) -> Option<f64> {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    let wei = u128::from_str_radix(hex, 16).ok()?;
    Some(wei as f64 / 1e18)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_u64() {
        assert_eq!(parse_hex_u64("0x10"), Some(16));
        assert_eq!(parse_hex_u64("0x1234"), Some(4660));
        assert_eq!(parse_hex_u64("1234"), Some(4660));
    }

    #[test]
    fn test_parse_wei_to_eth() {
        // 1 ETH = 1e18 wei
        assert!((parse_wei_to_eth("0xde0b6b3a7640000").unwrap() - 1.0).abs() < 0.0001);
    }
}
