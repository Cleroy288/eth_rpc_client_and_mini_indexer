# Metrics Documentation

## Overview

The Ethereum RPC Client & Mini Indexer exposes Prometheus-compatible metrics at `/metrics` endpoint.

## Endpoints

| Endpoint | Description | Format |
|----------|-------------|--------|
| `/metrics` | Prometheus metrics | text/plain |
| `/health` | Health check | text/plain |
| `/stats` | Statistics | JSON |

## Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `METRICS_PORT` | `9090` | Port for metrics server |

## Run Modes

```bash
# Server mode (metrics only)
RUN_MODE=server cargo run

# Continuous mode (fetcher + metrics)
RUN_MODE=continuous cargo run
```

## Prometheus Metrics

### Indexer Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `indexer_blocks_indexed_total` | counter | Total number of blocks indexed |
| `indexer_last_block` | gauge | Last indexed block number |
| `indexer_transactions_total` | counter | Total transactions indexed |

### Fetcher Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `fetcher_blocks_fetched_total` | counter | Total blocks fetched from RPC |

### RPC Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `rpc_errors_total` | counter | Total RPC errors encountered |
| `rpc_requests_total` | counter | Total RPC requests made |
| `rpc_latency_seconds` | gauge | Average RPC latency in seconds |
| `rpc_latency_last_ms` | gauge | Last RPC request latency (ms) |

### Process Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `process_uptime_seconds` | counter | Process uptime in seconds |

## Example Output

```
# HELP indexer_blocks_indexed_total Total number of blocks indexed
# TYPE indexer_blocks_indexed_total counter
indexer_blocks_indexed_total 14

# HELP indexer_last_block Last indexed block number
# TYPE indexer_last_block gauge
indexer_last_block 23906318

# HELP rpc_errors_total Total RPC errors encountered
# TYPE rpc_errors_total counter
rpc_errors_total 0

# HELP rpc_latency_seconds Average RPC latency in seconds
# TYPE rpc_latency_seconds gauge
rpc_latency_seconds 0.150000
```

## Grafana Integration

Add Prometheus as a data source pointing to `http://localhost:9090/metrics`.

Example queries:
- Blocks indexed: `indexer_blocks_indexed_total`
- Indexing rate: `rate(indexer_blocks_indexed_total[5m])`
- RPC errors: `rpc_errors_total`
- Latency: `rpc_latency_seconds`
