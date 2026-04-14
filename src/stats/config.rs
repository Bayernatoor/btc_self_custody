//! Stats module configuration, loaded from environment variables.
//!
//! The stats module operates in "dormant mode" when `BITCOIN_STATS_RPC_URL` is
//! not set - the application starts normally but without any block stats features.
//! This allows the same binary to run in environments without a Bitcoin node
//! (e.g. local development, staging) without errors.

use std::path::PathBuf;

/// Configuration for connecting to a Bitcoin Core node and storing block data.
/// All fields are loaded from environment variables via [`StatsConfig::load`].
pub struct StatsConfig {
    /// Bitcoin Core JSON-RPC URL. Env: `BITCOIN_STATS_RPC_URL` (required).
    pub rpc_url: String,
    /// RPC username. Env: `BITCOIN_STATS_RPC_USER` (default: "bitcoin").
    pub rpc_user: String,
    /// RPC password. Env: `BITCOIN_STATS_RPC_PASSWORD` (default: empty).
    pub rpc_password: String,
    /// Path to the SQLite database file. Env: `BITCOIN_STATS_DB_PATH` (default: "./bitcoin_stats.db").
    pub db_path: PathBuf,
    /// Number of recent blocks to ingest on first run (when DB is empty).
    /// Env: `BITCOIN_STATS_INITIAL_INGEST` (default: 1,000,000).
    pub initial_ingest_count: u64,
    /// ZMQ endpoint for raw transactions (e.g. "tcp://127.0.0.1:28333").
    /// Env: `BITCOIN_STATS_ZMQ_TX` (optional - disables heartbeat if unset).
    pub zmq_tx_url: Option<String>,
    /// ZMQ endpoint for block hashes (e.g. "tcp://127.0.0.1:28332").
    /// Env: `BITCOIN_STATS_ZMQ_BLOCK` (optional - disables heartbeat if unset).
    pub zmq_block_url: Option<String>,
}

impl StatsConfig {
    /// Load from environment variables. Returns None if BITCOIN_STATS_RPC_URL is not set.
    pub fn load() -> Option<Self> {
        let rpc_url = std::env::var("BITCOIN_STATS_RPC_URL").ok()?;
        let rpc_user = std::env::var("BITCOIN_STATS_RPC_USER")
            .unwrap_or_else(|_| "bitcoin".to_string());
        let rpc_password =
            std::env::var("BITCOIN_STATS_RPC_PASSWORD").unwrap_or_default();
        let db_path = std::env::var("BITCOIN_STATS_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./bitcoin_stats.db"));
        let initial_ingest_count =
            std::env::var("BITCOIN_STATS_INITIAL_INGEST")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1_000_000);

        let zmq_tx_url = std::env::var("BITCOIN_STATS_ZMQ_TX").ok();
        let zmq_block_url = std::env::var("BITCOIN_STATS_ZMQ_BLOCK").ok();

        Some(Self {
            rpc_url,
            rpc_user,
            rpc_password,
            db_path,
            initial_ingest_count,
            zmq_tx_url,
            zmq_block_url,
        })
    }
}
