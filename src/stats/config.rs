//! Stats module configuration, loaded from environment variables.
//!
//! Returns `None` when `BITCOIN_STATS_RPC_URL` is not set (dormant mode).

use std::path::PathBuf;

pub struct StatsConfig {
    pub rpc_url: String,
    pub rpc_user: String,
    pub rpc_password: String,
    pub db_path: PathBuf,
    pub initial_ingest_count: u64,
    /// ZMQ endpoint for raw transactions (e.g. tcp://192.168.8.131:28333)
    pub zmq_tx_url: Option<String>,
    /// ZMQ endpoint for block hashes (e.g. tcp://192.168.8.131:28332)
    pub zmq_block_url: Option<String>,
}

impl StatsConfig {
    /// Load from environment variables. Returns None if BITCOIN_STATS_RPC_URL is not set.
    pub fn load() -> Option<Self> {
        let rpc_url = std::env::var("BITCOIN_STATS_RPC_URL").ok()?;
        let rpc_user = std::env::var("BITCOIN_STATS_RPC_USER")
            .unwrap_or_else(|_| "testnode".to_string());
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
