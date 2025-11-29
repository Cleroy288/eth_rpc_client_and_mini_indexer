# Ethereum RPC Client & Mini Indexer

A lightweight Rust service for querying Ethereum blocks and transactions with Prometheus metrics.

## Features

- **RPC Client** - Connect to any Ethereum JSON-RPC endpoint
- **Block Fetcher** - Continuous polling for new blocks
- **JSON Indexer** - Store blocks and transactions locally
- **Prometheus Metrics** - `/metrics` endpoint for monitoring
- **Graceful Shutdown** - Ctrl+C handling

## Tech Stack

- **Rust** + **Tokio** - Async runtime
- **Reqwest** - HTTP client for JSON-RPC
- **Axum** - Web server for metrics
- **Serde** - JSON serialization

## Quick Start

```bash
# Build
cargo build --release

# Run in test mode (fetch 3 blocks)
cargo run

# Run with metrics server
RUN_MODE=server cargo run

# Run continuous indexing + metrics
RUN_MODE=continuous cargo run
```

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `ETH_RPC_ENDPOINT` | `https://ethereum-rpc.publicnode.com` | Ethereum RPC URL |
| `INDEX_FILE` | `data/index.json` | Index storage path |
| `METRICS_PORT` | `9090` | Metrics server port |
| `RUN_MODE` | `test` | `test`, `fetch`, `server`, `continuous` |

## Run Modes

| Mode | Description |
|------|-------------|
| `test` | Fetch 3 recent blocks, display results |
| `fetch` | Fetch 10 blocks from last indexed |
| `server` | Metrics server only (no fetching) |
| `continuous` | Fetcher loop + metrics server |

## Metrics Endpoints

```bash
# Health check
curl http://localhost:9090/health

# Prometheus metrics
curl http://localhost:9090/metrics

# JSON stats
curl http://localhost:9090/stats
```

## Prometheus Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `indexer_blocks_indexed_total` | counter | Total blocks indexed |
| `indexer_last_block` | gauge | Last indexed block |
| `indexer_transactions_total` | counter | Total transactions |
| `rpc_errors_total` | counter | RPC errors |
| `rpc_latency_seconds` | gauge | Average RPC latency |

## Project Structure

```
src/
├── main.rs        # Entry point, run modes
├── error.rs       # Error types
├── types.rs       # Ethereum data types
├── rpc_client.rs  # JSON-RPC client
├── fetcher.rs     # Block fetching loop
├── indexer.rs     # JSON storage
└── metrics.rs     # Prometheus metrics
```

## Tests

```bash
cargo test
```

## License

MIT
