//! Ethereum JSON-RPC client

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::error::{Result, RpcError};
use crate::types::Block;

/// JSON-RPC request
#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a, T: Serialize> {
    jsonrpc: &'a str,
    method: &'a str,
    params: T,
    id: u64,
}

/// JSON-RPC response
#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    result: Option<T>,
    error: Option<JsonRpcError>,
    #[allow(dead_code)]
    id: Option<u64>,
}

/// JSON-RPC error
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// Ethereum RPC client
pub struct EthRpcClient {
    client: Client,
    endpoint: String,
    request_id: AtomicU64,
}

impl EthRpcClient {
    /// Create a new RPC client
    pub fn new(endpoint: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            endpoint: endpoint.to_string(),
            request_id: AtomicU64::new(1),
        })
    }

    /// Get next request ID
    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Make a JSON-RPC call
    async fn call<P: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method,
            params,
            id: self.next_id(),
        };

        let response = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .await?;

        let rpc_response: JsonRpcResponse<R> = response.json().await?;

        if let Some(error) = rpc_response.error {
            return Err(RpcError::JsonRpcError {
                code: error.code,
                message: error.message,
            });
        }

        rpc_response
            .result
            .ok_or_else(|| RpcError::InvalidResponse("Missing result field".to_string()))
    }

    /// Get the latest block number
    pub async fn get_block_number(&self) -> Result<u64> {
        let hex: String = self.call("eth_blockNumber", ()).await?;
        let hex = hex.strip_prefix("0x").unwrap_or(&hex);
        u64::from_str_radix(hex, 16)
            .map_err(|e| RpcError::ParseError(format!("Invalid block number: {}", e)))
    }

    /// Get block by number (with full transactions)
    pub async fn get_block_by_number(&self, block_number: u64) -> Result<Option<Block>> {
        let hex_number = format!("0x{:x}", block_number);
        let params = (hex_number, true); // true = include full transactions

        self.call("eth_getBlockByNumber", params).await
    }

    /// Get the latest block
    #[allow(dead_code)]
    pub async fn get_latest_block(&self) -> Result<Option<Block>> {
        let params = ("latest", true);
        self.call("eth_getBlockByNumber", params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Integration test - requires RPC endpoint
    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_get_block_number() {
        let client = EthRpcClient::new("https://eth.llamarpc.com").unwrap();
        let block_number = client.get_block_number().await.unwrap();
        println!("Current block number: {}", block_number);
        assert!(block_number > 0);
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_block_by_number() {
        let client = EthRpcClient::new("https://eth.llamarpc.com").unwrap();
        let block = client.get_block_by_number(1).await.unwrap();
        assert!(block.is_some());
        let block = block.unwrap();
        println!("Block 1 hash: {}", block.hash);
        assert_eq!(block.block_number(), Some(1));
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_latest_block() {
        let client = EthRpcClient::new("https://eth.llamarpc.com").unwrap();
        let block = client.get_latest_block().await.unwrap();
        assert!(block.is_some());
        let block = block.unwrap();
        println!(
            "Latest block: {} with {} transactions",
            block.number,
            block.transactions.len()
        );
    }
}
