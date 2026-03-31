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
    pub p2pk_count: u64,
    pub p2pkh_count: u64,
    pub p2sh_count: u64,
    pub p2wpkh_count: u64,
    pub p2wsh_count: u64,
    pub p2tr_count: u64,
    pub multisig_count: u64,
    pub unknown_script_count: u64,
    pub input_count: u64,
    pub output_count: u64,
    pub rbf_count: u64,
    pub witness_bytes: u64,
    pub inscription_count: u64,
    pub inscription_bytes: u64,
    pub brc20_count: u64,
    pub op_return_count: u64,
    pub op_return_bytes: u64,
    pub runes_count: u64,
    pub runes_bytes: u64,
    pub omni_count: u64,
    pub omni_bytes: u64,
    pub counterparty_count: u64,
    pub counterparty_bytes: u64,
    pub data_carrier_count: u64,
    pub data_carrier_bytes: u64,
    pub taproot_keypath_count: u64,
    pub taproot_scriptpath_count: u64,
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
    pub inscription_count: u64,
    pub inscription_bytes: u64,
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
    pub size: u64,
    pub op_return_count: u64,
    pub op_return_bytes: u64,
    pub runes_count: u64,
    pub runes_bytes: u64,
    pub omni_count: u64,
    pub omni_bytes: u64,
    pub counterparty_count: u64,
    pub counterparty_bytes: u64,
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
    pub total_omni_count: u64,
    pub total_omni_bytes: u64,
    pub total_counterparty_count: u64,
    pub total_counterparty_bytes: u64,
    pub total_data_carrier_count: u64,
    pub total_data_carrier_bytes: u64,
    pub total_fees: u64,
    pub avg_segwit_spend_count: f64,
    pub avg_taproot_spend_count: f64,
    pub avg_p2pk_count: f64,
    pub avg_p2pkh_count: f64,
    pub avg_p2sh_count: f64,
    pub avg_p2wpkh_count: f64,
    pub avg_p2wsh_count: f64,
    pub avg_p2tr_count: f64,
    pub avg_multisig_count: f64,
    pub avg_unknown_script_count: f64,
    pub avg_input_count: f64,
    pub avg_output_count: f64,
    pub avg_rbf_count: f64,
    pub avg_witness_bytes: f64,
    pub avg_inscription_count: f64,
    pub avg_inscription_bytes: f64,
    pub avg_brc20_count: f64,
    pub avg_taproot_keypath_count: f64,
    pub avg_taproot_scriptpath_count: f64,
}

/// Aggregated summary for an arbitrary time range (Stats Dashboard).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RangeSummary {
    pub block_count: u64,
    pub total_tx: u64,
    pub total_size: u64,
    pub total_weight: u64,
    pub total_fees: u64,
    pub avg_fee_rate: f64,
    pub avg_block_time: f64,
    pub total_segwit_txs: u64,
    pub total_taproot_outputs: u64,
    pub total_taproot_keypath: u64,
    pub total_taproot_scriptpath: u64,
    pub total_p2pkh: u64,
    pub total_p2sh: u64,
    pub total_p2wpkh: u64,
    pub total_p2wsh: u64,
    pub total_p2tr: u64,
    pub total_inputs: u64,
    pub total_outputs: u64,
    pub total_rbf: u64,
    pub total_witness_bytes: u64,
    pub total_inscriptions: u64,
    pub total_inscription_bytes: u64,
    pub total_brc20: u64,
    pub total_op_return_count: u64,
    pub total_op_return_bytes: u64,
    pub total_runes: u64,
    pub total_runes_bytes: u64,
    pub total_omni: u64,
    pub total_counterparty: u64,
    pub total_data_carrier: u64,
    pub min_timestamp: u64,
    pub max_timestamp: u64,
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
    pub time: u64,
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
    pub hashrate: f64,
}

/// Historical price point (timestamp_ms, price_usd).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PricePoint {
    pub timestamp_ms: u64,
    pub price_usd: f64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supply_genesis() {
        // Block 0: 1 block × 50 BTC = 50
        assert_eq!(calc_supply(0), 50.0);
    }

    #[test]
    fn supply_first_halving_boundary() {
        // Blocks 0-209,999: 210,000 × 50 = 10,500,000
        assert_eq!(calc_supply(209_999), 10_500_000.0);
    }

    #[test]
    fn supply_at_first_halving() {
        // Block 210,000: era 0 (210,000 × 50) + era 1 (1 × 25) = 10,500,025
        assert_eq!(calc_supply(210_000), 10_500_025.0);
    }

    #[test]
    fn supply_second_halving_boundary() {
        // Blocks 0-419,999:
        // Era 0: 210,000 × 50 = 10,500,000
        // Era 1: 210,000 × 25 = 5,250,000
        // Total: 15,750,000
        assert_eq!(calc_supply(419_999), 15_750_000.0);
    }

    #[test]
    fn supply_at_fourth_halving() {
        // Block 840,000:
        // Era 0: 210,000 × 50     = 10,500,000
        // Era 1: 210,000 × 25     = 5,250,000
        // Era 2: 210,000 × 12.5   = 2,625,000
        // Era 3: 210,000 × 6.25   = 1,312,500
        // Era 4: 1 × 3.125        = 3.125
        // Total: 19,687,503.125
        assert_eq!(calc_supply(840_000), 19_687_503.125);
    }

    #[test]
    fn supply_max_approaches_21m() {
        // At a very high block, supply should approach but not exceed 21M
        let supply = calc_supply(10_000_000);
        assert!(supply > 20_999_000.0);
        assert!(supply <= 21_000_000.0);
    }
}
