//! Error types for the Ethereum RPC client

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("JSON-RPC error: code={code}, message={message}")]
    JsonRpcError { code: i64, message: String },

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Hex decode error: {0}")]
    #[allow(dead_code)]
    HexDecodeError(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

pub type Result<T> = std::result::Result<T, RpcError>;
