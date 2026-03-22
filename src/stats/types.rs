//! Shared types for the stats module.
//! These compile for both SSR and WASM targets.

use serde::{Deserialize, Serialize};

/// Summary stats about the database.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatsSummary {
    pub block_count: u64,
    pub min_height: u64,
    pub max_height: u64,
    pub latest_timestamp: u64,
}

/// Summary block row (from query_blocks).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockSummary {
    pub height: u64,
    pub hash: String,
    pub timestamp: u64,
    pub tx_count: u64,
    pub size: u64,
    pub weight: u64,
    pub difficulty: f64,
    pub total_fees: u64,
    pub median_fee: u64,
    pub median_fee_rate: f64,
    pub segwit_spend_count: u64,
    pub taproot_spend_count: u64,
}

/// Full block detail (from query_block_by_height).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockDetail {
    pub height: u64,
    pub hash: String,
    pub timestamp: u64,
    pub tx_count: u64,
    pub size: u64,
    pub weight: u64,
    pub difficulty: f64,
    pub op_return_count: u64,
    pub op_return_bytes: u64,
    pub runes_count: u64,
    pub runes_bytes: u64,
    pub data_carrier_count: u64,
    pub data_carrier_bytes: u64,
    pub version: u32,
    pub total_fees: u64,
    pub median_fee: u64,
    pub median_fee_rate: f64,
    pub coinbase_locktime: u64,
    pub coinbase_sequence: u64,
    pub miner: String,
    pub segwit_spend_count: u64,
    pub taproot_spend_count: u64,
}

/// OP_RETURN block data (from query_op_returns).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpReturnBlock {
    pub height: u64,
    pub timestamp: u64,
    pub tx_count: u64,
    pub op_return_count: u64,
    pub op_return_bytes: u64,
    pub runes_count: u64,
    pub runes_bytes: u64,
    pub data_carrier_count: u64,
    pub data_carrier_bytes: u64,
}

/// Daily aggregated metrics (from query_daily_aggregates).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DailyAggregate {
    pub date: String,
    pub block_count: u64,
    pub avg_size: f64,
    pub avg_weight: f64,
    pub avg_tx_count: f64,
    pub avg_difficulty: f64,
    pub total_op_return_count: u64,
    pub total_op_return_bytes: u64,
    pub total_runes_count: u64,
    pub total_runes_bytes: u64,
    pub total_data_carrier_count: u64,
    pub total_data_carrier_bytes: u64,
    pub total_fees: u64,
    pub avg_segwit_spend_count: f64,
    pub avg_taproot_spend_count: f64,
}

/// Per-block signaling status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalingBlock {
    pub height: u64,
    pub timestamp: u64,
    pub signaled: bool,
    pub miner: String,
}

/// Signaling summary for a retarget period.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalingPeriod {
    pub start_height: u64,
    pub end_height: u64,
    pub signaled_count: u64,
    pub total_blocks: u64,
    pub signaled_pct: f64,
}

/// Period stats for the current signaling window.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeriodStats {
    pub period_start: u64,
    pub period_end: u64,
    pub total_blocks: u64,
    pub signaled_count: u64,
    pub signaled_pct: f64,
}

/// Live node, mempool, and network stats.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveStats {
    pub blockchain: LiveBlockchain,
    pub mempool: LiveMempool,
    pub next_block_fee: f64,
    pub network: LiveNetwork,
}

/// Blockchain info from getblockchaininfo.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveBlockchain {
    pub blocks: u64,
    pub chain: String,
    pub difficulty: f64,
    pub verification_progress: f64,
    pub size_on_disk: u64,
    pub bestblockhash: String,
}

/// Mempool info from getmempoolinfo.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveMempool {
    pub size: u64,
    pub bytes: u64,
    pub usage: u64,
    pub total_fee: f64,
    pub maxmempool: u64,
    pub mempoolminfee: f64,
}

/// Derived network stats (price, supply, etc.).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveNetwork {
    pub price_usd: f64,
    pub sats_per_dollar: u64,
    pub market_cap_usd: f64,
    pub total_supply: f64,
    pub max_supply: f64,
    pub percent_issued: f64,
    pub utxo_count: u64,
    pub chain_size_gb: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinerShare {
    pub miner: String,
    pub count: u64,
    pub percentage: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmptyBlock {
    pub height: u64,
    pub timestamp: u64,
    pub miner: String,
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

const HALVING_INTERVAL: u64 = 210_000;
const INITIAL_SUBSIDY: f64 = 50.0;

/// Calculate total BTC supply at a given block height.
pub fn calc_supply(height: u64) -> f64 {
    let mut supply = 0.0;
    let mut subsidy = INITIAL_SUBSIDY;
    let mut remaining = height + 1;

    loop {
        let blocks_in_era = remaining.min(HALVING_INTERVAL);
        supply += blocks_in_era as f64 * subsidy;
        remaining -= blocks_in_era;
        if remaining == 0 {
            break;
        }
        subsidy /= 2.0;
        if subsidy < 1e-8 {
            break;
        }
    }

    supply
}
