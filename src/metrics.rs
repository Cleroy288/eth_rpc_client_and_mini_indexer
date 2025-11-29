//! Prometheus-compatible metrics endpoint
//!
//! Exposes metrics in Prometheus text format at /metrics

use axum::{
    extract::State,
    response::IntoResponse,
    routing::get,
    Router,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use crate::fetcher::FetcherMetrics;
use crate::indexer::Indexer;

/// Application state shared with Axum handlers
#[derive(Clone)]
pub struct AppState {
    pub indexer: Arc<Indexer>,
    pub fetcher_metrics: Arc<FetcherMetrics>,
    pub rpc_latency: Arc<RwLock<RpcLatencyMetrics>>,
    pub start_time: Instant,
}

/// RPC latency tracking
#[derive(Debug, Default)]
pub struct RpcLatencyMetrics {
    pub total_requests: u64,
    pub total_latency_ms: u64,
    pub last_latency_ms: u64,
}

impl RpcLatencyMetrics {
    pub fn record(&mut self, latency_ms: u64) {
        self.total_requests += 1;
        self.total_latency_ms += latency_ms;
        self.last_latency_ms = latency_ms;
    }

    pub fn avg_latency_seconds(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.total_latency_ms as f64 / self.total_requests as f64) / 1000.0
        }
    }
}


/// Create the metrics router
pub fn metrics_router(state: AppState) -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .route("/stats", get(stats_handler))
        .with_state(state)
}

/// Health check endpoint
async fn health_handler() -> impl IntoResponse {
    "OK"
}

/// Stats endpoint (JSON format)
async fn stats_handler(State(state): State<AppState>) -> impl IntoResponse {
    let indexer_stats = state.indexer.stats().await;
    let fetcher_metrics = &state.fetcher_metrics;
    let rpc_latency = state.rpc_latency.read().await;
    let uptime_secs = state.start_time.elapsed().as_secs();

    let stats = serde_json::json!({
        "indexer": {
            "last_block": indexer_stats.last_block,
            "total_blocks": indexer_stats.total_blocks,
            "total_transactions": indexer_stats.total_transactions
        },
        "fetcher": {
            "blocks_fetched": fetcher_metrics.get_blocks_fetched(),
            "rpc_errors": fetcher_metrics.get_rpc_errors()
        },
        "rpc": {
            "total_requests": rpc_latency.total_requests,
            "avg_latency_seconds": rpc_latency.avg_latency_seconds(),
            "last_latency_ms": rpc_latency.last_latency_ms
        },
        "uptime_seconds": uptime_secs
    });

    axum::Json(stats)
}

/// Prometheus metrics endpoint
async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    let indexer_stats = state.indexer.stats().await;
    let fetcher_metrics = &state.fetcher_metrics;
    let rpc_latency = state.rpc_latency.read().await;
    let uptime_secs = state.start_time.elapsed().as_secs();

    // Build Prometheus-compatible output
    let mut output = String::new();

    // Indexer metrics
    output.push_str("# HELP indexer_blocks_indexed_total Total number of blocks indexed\n");
    output.push_str("# TYPE indexer_blocks_indexed_total counter\n");
    output.push_str(&format!(
        "indexer_blocks_indexed_total {}\n",
        indexer_stats.total_blocks
    ));

    output.push_str("# HELP indexer_last_block Last indexed block number\n");
    output.push_str("# TYPE indexer_last_block gauge\n");
    output.push_str(&format!("indexer_last_block {}\n", indexer_stats.last_block));

    output.push_str("# HELP indexer_transactions_total Total number of transactions indexed\n");
    output.push_str("# TYPE indexer_transactions_total counter\n");
    output.push_str(&format!(
        "indexer_transactions_total {}\n",
        indexer_stats.total_transactions
    ));

    // Fetcher metrics
    output.push_str("# HELP fetcher_blocks_fetched_total Total blocks fetched from RPC\n");
    output.push_str("# TYPE fetcher_blocks_fetched_total counter\n");
    output.push_str(&format!(
        "fetcher_blocks_fetched_total {}\n",
        fetcher_metrics.get_blocks_fetched()
    ));

    // RPC metrics
    output.push_str("# HELP rpc_errors_total Total RPC errors encountered\n");
    output.push_str("# TYPE rpc_errors_total counter\n");
    output.push_str(&format!(
        "rpc_errors_total {}\n",
        fetcher_metrics.get_rpc_errors()
    ));

    output.push_str("# HELP rpc_requests_total Total RPC requests made\n");
    output.push_str("# TYPE rpc_requests_total counter\n");
    output.push_str(&format!("rpc_requests_total {}\n", rpc_latency.total_requests));

    output.push_str("# HELP rpc_latency_seconds Average RPC latency in seconds\n");
    output.push_str("# TYPE rpc_latency_seconds gauge\n");
    output.push_str(&format!(
        "rpc_latency_seconds {:.6}\n",
        rpc_latency.avg_latency_seconds()
    ));

    output.push_str("# HELP rpc_latency_last_ms Last RPC request latency in milliseconds\n");
    output.push_str("# TYPE rpc_latency_last_ms gauge\n");
    output.push_str(&format!("rpc_latency_last_ms {}\n", rpc_latency.last_latency_ms));

    // Process metrics
    output.push_str("# HELP process_uptime_seconds Process uptime in seconds\n");
    output.push_str("# TYPE process_uptime_seconds counter\n");
    output.push_str(&format!("process_uptime_seconds {}\n", uptime_secs));

    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        output,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_latency_metrics() {
        let mut metrics = RpcLatencyMetrics::default();
        
        assert_eq!(metrics.avg_latency_seconds(), 0.0);
        
        metrics.record(100);
        metrics.record(200);
        
        assert_eq!(metrics.total_requests, 2);
        assert_eq!(metrics.total_latency_ms, 300);
        assert_eq!(metrics.last_latency_ms, 200);
        assert!((metrics.avg_latency_seconds() - 0.15).abs() < 0.001);
    }
}
