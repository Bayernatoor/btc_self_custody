//! Shared types for the stats module.
//!
//! These types compile for both SSR (server-side rendering) and WASM targets,
//! enabling Leptos server functions and frontend components to share the same
//! data structures without duplication. All types derive Serialize/Deserialize
//! for JSON transport between server and client.

use serde::{Deserialize, Serialize};

/// High-level summary of what the database contains.
/// Used by the frontend to determine available block range and freshness.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatsSummary {
    /// Number of blocks stored in the database.
    pub block_count: u64,
    /// Lowest block height in the database (may not be 0 during backfill).
    pub min_height: u64,
    /// Highest block height in the database (chain tip).
    pub max_height: u64,
    /// Unix timestamp of the most recent block.
    pub latest_timestamp: u64,
}

/// Per-block summary row returned by range queries. Contains all metrics
/// extracted during ingestion for chart rendering and table display.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockSummary {
    /// Block height (0 = genesis).
    pub height: u64,
    /// Block hash as hex string.
    pub hash: String,
    /// Block timestamp in unix seconds.
    pub timestamp: u64,
    /// Number of transactions including coinbase.
    pub tx_count: u64,
    /// Block size in bytes.
    pub size: u64,
    /// Block weight in weight units (max 4,000,000 WU).
    pub weight: u64,
    /// Mining difficulty target.
    pub difficulty: f64,
    /// Total transaction fees in satoshis (coinbase output minus subsidy).
    pub total_fees: u64,
    /// Median transaction fee in satoshis.
    pub median_fee: u64,
    /// Median fee rate in sat/vB.
    pub median_fee_rate: f64,
    /// Number of transactions with at least one SegWit input.
    pub segwit_spend_count: u64,
    /// Number of P2TR (taproot) outputs created in this block.
    pub taproot_spend_count: u64,
    /// Number of P2PK (pay-to-pubkey) outputs.
    pub p2pk_count: u64,
    /// Number of P2PKH (pay-to-pubkey-hash) outputs.
    pub p2pkh_count: u64,
    /// Number of P2SH (pay-to-script-hash) outputs.
    pub p2sh_count: u64,
    /// Number of P2WPKH (native SegWit v0 keyhash) outputs.
    pub p2wpkh_count: u64,
    /// Number of P2WSH (native SegWit v0 scripthash) outputs.
    pub p2wsh_count: u64,
    /// Number of P2TR (taproot v1) outputs.
    pub p2tr_count: u64,
    /// Number of bare multisig outputs.
    pub multisig_count: u64,
    /// Number of outputs with unrecognized script types.
    pub unknown_script_count: u64,
    /// Total number of transaction inputs (excluding coinbase).
    pub input_count: u64,
    /// Total number of transaction outputs (excluding coinbase).
    pub output_count: u64,
    /// Number of transactions signaling RBF (nSequence < 0xFFFFFFFE, excluding CSV).
    pub rbf_count: u64,
    /// Total witness data size in bytes (including varint overhead).
    pub witness_bytes: u64,
    /// Number of Ordinals inscriptions detected in witness data.
    pub inscription_count: u64,
    /// Total inscription content size in bytes (excluding envelope overhead).
    pub inscription_bytes: u64,
    /// Full witness item bytes for inscriptions (includes envelope opcodes + payload).
    pub inscription_envelope_bytes: u64,
    /// Number of BRC-20 token operations (subset of inscriptions).
    pub brc20_count: u64,
    /// Total OP_RETURN outputs (excludes SegWit commitments).
    pub op_return_count: u64,
    /// Total OP_RETURN data size in bytes.
    pub op_return_bytes: u64,
    /// Number of Runes protocol OP_RETURN outputs (post-block 840,000 only).
    pub runes_count: u64,
    /// Total Runes OP_RETURN data size in bytes.
    pub runes_bytes: u64,
    /// Number of Omni Layer OP_RETURN outputs.
    pub omni_count: u64,
    /// Total Omni Layer OP_RETURN data size in bytes.
    pub omni_bytes: u64,
    /// Number of Counterparty OP_RETURN outputs.
    pub counterparty_count: u64,
    /// Total Counterparty OP_RETURN data size in bytes.
    pub counterparty_bytes: u64,
    /// Number of generic data carrier OP_RETURN outputs.
    pub data_carrier_count: u64,
    /// Total generic data carrier OP_RETURN size in bytes.
    pub data_carrier_bytes: u64,
    /// Number of taproot key-path spends (single 64-65 byte Schnorr signature).
    pub taproot_keypath_count: u64,
    /// Number of taproot script-path spends (control block present).
    pub taproot_scriptpath_count: u64,
    /// Total value of non-coinbase outputs in satoshis.
    pub total_output_value: u64,
    /// Total value of non-coinbase inputs in satoshis (from prevout).
    pub total_input_value: u64,
    /// 10th percentile fee rate in sat/vB.
    pub fee_rate_p10: f64,
    /// 90th percentile fee rate in sat/vB.
    pub fee_rate_p90: f64,
    /// Number of Stamps protocol outputs (bare multisig with fake pubkeys).
    pub stamps_count: u64,
    /// Size of the largest transaction in bytes.
    pub largest_tx_size: u64,
    // --- Backfill v10 fields (0 for historical blocks until backfill completes) ---
    /// Largest individual transaction fee in satoshis.
    pub max_tx_fee: u64,
    /// Total fees from inscription transactions in satoshis.
    pub inscription_fees: u64,
    /// Total fees from Runes transactions in satoshis.
    pub runes_fees: u64,
    /// Number of transactions with only legacy inputs.
    pub legacy_tx_count: u64,
    /// Number of transactions with any SegWit v0 input (no Taproot).
    pub segwit_tx_count: u64,
    /// Number of transactions with any Taproot input.
    pub taproot_tx_count: u64,
    /// Decoded ASCII text from the coinbase transaction.
    pub coinbase_text: String,
    /// 25th percentile fee rate in sat/vB.
    pub fee_rate_p25: f64,
    /// 75th percentile fee rate in sat/vB.
    pub fee_rate_p75: f64,
}

/// Full block detail for the single-block detail page. Includes coinbase
/// metadata (miner, locktime, sequence) not present in the summary view.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockDetail {
    pub height: u64,
    pub hash: String,
    /// Block timestamp in unix seconds.
    pub timestamp: u64,
    pub tx_count: u64,
    /// Block size in bytes.
    pub size: u64,
    /// Block weight in weight units.
    pub weight: u64,
    pub difficulty: f64,
    pub op_return_count: u64,
    /// Total OP_RETURN data size in bytes.
    pub op_return_bytes: u64,
    pub runes_count: u64,
    pub runes_bytes: u64,
    pub data_carrier_count: u64,
    pub data_carrier_bytes: u64,
    pub inscription_count: u64,
    /// Total inscription content size in bytes (payload only).
    pub inscription_bytes: u64,
    /// Full witness item bytes for inscriptions (envelope + payload).
    pub inscription_envelope_bytes: u64,
    /// Block version field (used for BIP9 soft fork signaling).
    pub version: u32,
    /// Total fees in satoshis.
    pub total_fees: u64,
    /// Median transaction fee in satoshis.
    pub median_fee: u64,
    /// Median fee rate in sat/vB.
    pub median_fee_rate: f64,
    /// Coinbase transaction nLockTime (used for BIP-54 signaling).
    pub coinbase_locktime: u64,
    /// Coinbase first input nSequence (used for BIP-54 signaling).
    pub coinbase_sequence: u64,
    /// Identified mining pool name (e.g. "Foundry USA", "OCEAN / 234 Alberta").
    pub miner: String,
    /// Number of transactions with SegWit inputs.
    pub segwit_spend_count: u64,
    /// Number of P2TR outputs created.
    pub taproot_spend_count: u64,
}

/// Per-block OP_RETURN breakdown for the OP_RETURN analysis charts.
/// Counts and byte sizes are split by protocol (Runes, Omni, Counterparty, generic).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpReturnBlock {
    pub height: u64,
    /// Block timestamp in unix seconds.
    pub timestamp: u64,
    pub tx_count: u64,
    /// Block size in bytes.
    pub size: u64,
    /// Total OP_RETURN outputs (all protocols combined, excludes SegWit commitments).
    pub op_return_count: u64,
    /// Total OP_RETURN data in bytes (all protocols combined).
    pub op_return_bytes: u64,
    pub runes_count: u64,
    pub runes_bytes: u64,
    pub omni_count: u64,
    pub omni_bytes: u64,
    pub counterparty_count: u64,
    pub counterparty_bytes: u64,
    /// Generic data carrier OP_RETURNs not matching any known protocol.
    pub data_carrier_count: u64,
    pub data_carrier_bytes: u64,
}

/// Daily aggregated metrics for long-range trend charts.
///
/// Fields prefixed with `avg_` are per-block averages for that day (e.g. avg_size
/// is the mean block size across all blocks mined that day). Fields prefixed with
/// `total_` are day-wide sums. This distinction matters because ~144 blocks are
/// mined per day, so totals scale with block count while averages do not.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DailyAggregate {
    /// Date string in YYYY-MM-DD format (UTC).
    pub date: String,
    /// Number of blocks mined on this day.
    pub block_count: u64,
    /// Average block size in bytes (per block).
    pub avg_size: f64,
    /// Average block weight in weight units (per block).
    pub avg_weight: f64,
    /// Average transaction count per block.
    pub avg_tx_count: f64,
    /// Average difficulty (per block, should be constant within a retarget period).
    pub avg_difficulty: f64,
    /// Total OP_RETURN outputs across all blocks this day.
    pub total_op_return_count: u64,
    /// Total OP_RETURN bytes across all blocks this day.
    pub total_op_return_bytes: u64,
    /// Total Runes OP_RETURN outputs this day.
    pub total_runes_count: u64,
    pub total_runes_bytes: u64,
    pub total_omni_count: u64,
    pub total_omni_bytes: u64,
    pub total_counterparty_count: u64,
    pub total_counterparty_bytes: u64,
    pub total_data_carrier_count: u64,
    pub total_data_carrier_bytes: u64,
    /// Total fees in satoshis across all blocks this day.
    pub total_fees: u64,
    /// Average SegWit transaction count per block.
    pub avg_segwit_spend_count: f64,
    /// Average taproot output count per block.
    pub avg_taproot_spend_count: f64,
    pub avg_p2pk_count: f64,
    pub avg_p2pkh_count: f64,
    pub avg_p2sh_count: f64,
    pub avg_p2wpkh_count: f64,
    pub avg_p2wsh_count: f64,
    pub avg_p2tr_count: f64,
    pub avg_multisig_count: f64,
    pub avg_unknown_script_count: f64,
    /// Average input count per block.
    pub avg_input_count: f64,
    /// Average output count per block.
    pub avg_output_count: f64,
    /// Average RBF-signaling transaction count per block.
    pub avg_rbf_count: f64,
    /// Average witness data size in bytes per block.
    pub avg_witness_bytes: f64,
    pub avg_inscription_count: f64,
    pub avg_inscription_bytes: f64,
    pub avg_brc20_count: f64,
    pub avg_taproot_keypath_count: f64,
    pub avg_taproot_scriptpath_count: f64,
    /// Average 10th percentile fee rate in sat/vB per block.
    pub avg_fee_rate_p10: f64,
    /// Average 90th percentile fee rate in sat/vB per block.
    pub avg_fee_rate_p90: f64,
    pub avg_stamps_count: f64,
    /// Average median fee rate in sat/vB per block.
    pub avg_median_fee_rate: f64,
    /// Total non-coinbase output value in satoshis for the day.
    pub total_output_value: u64,
    /// Total input value in satoshis for the day.
    pub total_input_value: u64,
    // --- v11 fields ---
    /// Average inscription envelope bytes per block (full witness item size).
    pub avg_inscription_envelope_bytes: f64,
    /// Total inscription fees in satoshis for the day.
    pub total_inscription_fees: u64,
    /// Total Runes fees in satoshis for the day.
    pub total_runes_fees: u64,
    /// Average legacy (non-witness) transaction count per block.
    pub avg_legacy_tx_count: f64,
    /// Average SegWit v0 transaction count per block.
    pub avg_segwit_tx_count: f64,
    /// Average Taproot transaction count per block.
    pub avg_taproot_tx_count: f64,
    /// Average 25th percentile fee rate in sat/vB per block.
    pub avg_fee_rate_p25: f64,
    /// Average 75th percentile fee rate in sat/vB per block.
    pub avg_fee_rate_p75: f64,
}

/// Single-row aggregate summary for an arbitrary timestamp range, used by the
/// Stats Dashboard overview cards. All `total_` fields are sums across every
/// block in the range; `avg_` fields are averages.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RangeSummary {
    /// Number of blocks in the range.
    pub block_count: u64,
    /// Total transactions (including coinbase) across all blocks.
    pub total_tx: u64,
    /// Total block data size in bytes.
    pub total_size: u64,
    /// Total block weight in weight units.
    pub total_weight: u64,
    /// Total fees in satoshis across all blocks.
    pub total_fees: u64,
    /// Average median fee rate in sat/vB across blocks.
    pub avg_fee_rate: f64,
    /// Average fee per user transaction in satoshis (total_fees / user_txs).
    pub avg_fee_per_tx: f64,
    /// Average median fee in satoshis per block.
    pub avg_median_fee: f64,
    /// Average time between blocks in minutes.
    pub avg_block_time: f64,
    /// Total SegWit transactions across all blocks.
    pub total_segwit_txs: u64,
    /// Total taproot (P2TR) outputs created.
    pub total_taproot_outputs: u64,
    /// Total taproot key-path spends.
    pub total_taproot_keypath: u64,
    /// Total taproot script-path spends.
    pub total_taproot_scriptpath: u64,
    pub total_p2pkh: u64,
    pub total_p2sh: u64,
    pub total_p2wpkh: u64,
    pub total_p2wsh: u64,
    pub total_p2tr: u64,
    pub total_inputs: u64,
    pub total_outputs: u64,
    /// Total RBF-signaling transactions.
    pub total_rbf: u64,
    /// Total witness data in bytes.
    pub total_witness_bytes: u64,
    pub total_inscriptions: u64,
    /// Total inscription content in bytes (payload only).
    pub total_inscription_bytes: u64,
    /// Total inscription envelope bytes (payload + witness overhead).
    pub total_inscription_envelope_bytes: u64,
    pub total_brc20: u64,
    pub total_op_return_count: u64,
    pub total_op_return_bytes: u64,
    pub total_runes: u64,
    pub total_runes_bytes: u64,
    pub total_omni: u64,
    pub total_counterparty: u64,
    pub total_data_carrier: u64,
    /// Total non-coinbase output value in satoshis.
    pub total_output_value: u64,
    /// Unix timestamp of the earliest block in range.
    pub min_timestamp: u64,
    /// Unix timestamp of the latest block in range.
    pub max_timestamp: u64,
    // -- Extremes --
    /// Largest single block size in bytes within the range.
    pub max_block_size: u64,
    /// Highest single block fee total in satoshis within the range.
    pub max_block_fees: u64,
    /// Number of empty blocks (coinbase-only, tx_count <= 1).
    pub empty_block_count: u64,
    /// Highest median fee rate (sat/vB) seen in any single block.
    pub max_fee_rate: f64,
    // -- Derived percentages --
    /// Witness data as a percentage of total block size.
    pub witness_pct: f64,
}

/// A single extreme record: the peak integer value and the block where it occurred.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtremeRecord {
    /// The extreme value (e.g. block size in bytes, fee total in satoshis).
    pub value: u64,
    /// Block height where this extreme occurred.
    pub height: u64,
    /// Unix timestamp of the block.
    pub timestamp: u64,
    /// Mining pool that mined this block.
    pub miner: String,
}

/// Float variant of ExtremeRecord for metrics like fee rates (sat/vB).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtremeRecordF64 {
    pub value: f64,
    pub height: u64,
    pub timestamp: u64,
    pub miner: String,
}

/// Collection of extreme (record-breaking) values for a time range.
/// Each field identifies the block that holds the maximum for that metric.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtremesData {
    /// Block with the largest size in bytes.
    pub largest_block: ExtremeRecord,
    /// Block with the highest total fees in satoshis.
    pub highest_fee_block: ExtremeRecord,
    /// Block with the highest median fee rate in sat/vB.
    pub peak_fee_rate: ExtremeRecordF64,
    /// Block with the highest 90th percentile fee rate in sat/vB.
    pub peak_p90_fee_rate: ExtremeRecordF64,
    /// Block with the most transactions.
    pub most_txs: ExtremeRecord,
    /// Block containing the largest single transaction (by size in bytes).
    pub largest_tx: ExtremeRecord,
    /// Block with the most inputs.
    pub most_inputs: ExtremeRecord,
    /// Block with the most outputs.
    pub most_outputs: ExtremeRecord,
    /// Block with the most Ordinals inscriptions.
    pub most_inscriptions: ExtremeRecord,
    /// Block with the most Runes protocol outputs.
    pub most_runes: ExtremeRecord,
    /// Block with the most OP_RETURN outputs.
    pub most_op_returns: ExtremeRecord,
    /// Block with the most RBF-signaling transactions.
    pub most_rbf: ExtremeRecord,
    /// Block with the most taproot outputs.
    pub most_taproot: ExtremeRecord,
    /// Block with the highest total output value (largest settlement volume).
    pub highest_value: ExtremeRecord,
    /// Total empty blocks (coinbase-only) in the range.
    pub empty_block_count: u64,
    /// Total blocks in the range.
    pub block_count: u64,
}

// ---------------------------------------------------------------------------
// Hall of Fame
// ---------------------------------------------------------------------------

/// Category for Hall of Fame entries, used for filtering and color-coding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HofCategory {
    /// Key moments in Bitcoin history (halvings, price milestones).
    Milestones,
    /// On-chain records (biggest block, highest fees, etc.).
    Records,
    /// Notable attacks and exploits.
    Attacks,
    /// Protocol upgrades and soft forks.
    Protocol,
    /// Unusual or quirky on-chain events.
    Oddities,
}

impl HofCategory {
    /// Human-readable label for display.
    pub fn label(self) -> &'static str {
        match self {
            Self::Milestones => "Milestones",
            Self::Records => "Records",
            Self::Attacks => "Attacks",
            Self::Protocol => "Protocol",
            Self::Oddities => "Oddities",
        }
    }

    /// CSS hex color for the category badge.
    pub fn color(self) -> &'static str {
        match self {
            Self::Milestones => "#f7931a",
            Self::Records => "#60a5fa",
            Self::Attacks => "#ef4444",
            Self::Protocol => "#a78bfa",
            Self::Oddities => "#34d399",
        }
    }

}

/// Pre-computed histogram bucket: label + count.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistogramBucket {
    pub label: String,
    pub count: u64,
}

/// A curated Hall of Fame entry - compile-time constant.
/// These are hardcoded notable Bitcoin events displayed on the Hall of Fame page.
#[derive(Clone, Debug)]
pub struct HallOfFameEntry {
    /// URL-safe identifier for deep linking.
    pub slug: &'static str,
    /// Short display title.
    pub title: &'static str,
    /// Longer description of the event.
    pub description: &'static str,
    pub category: HofCategory,
    /// Date string for display (e.g. "2009-01-03").
    pub date: &'static str,
    /// Associated block height, if applicable.
    pub block: Option<u64>,
    /// Associated transaction ID, if applicable.
    pub txid: Option<&'static str>,
    /// Whether to visually highlight this entry.
    pub highlight: bool,
    /// Optional (label, url) for an external reference article or source.
    pub source: Option<(&'static str, &'static str)>,
}

/// A notable event with title and context.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotableEvent {
    pub title: String,
    pub context: String,
    pub block: Option<u64>,
}

/// A single year's aggregated data for the "On This Day" feature.
/// Shows how Bitcoin looked on this calendar date in each year.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OnThisDayYear {
    pub year: u32,
    /// Number of blocks mined on this date in this year.
    pub block_count: u64,
    /// Total transactions across all blocks on this date.
    pub total_tx: u64,
    /// Total fees in satoshis.
    pub total_fees: u64,
    /// Average block size in bytes.
    pub avg_block_size: f64,
    /// Average weight utilization as a percentage of 4M WU limit.
    pub avg_weight_util: f64,
    pub total_inscriptions: u64,
    pub total_runes: u64,
    /// Percentage of transactions using SegWit inputs.
    pub segwit_pct: f64,
    /// Total taproot outputs created.
    pub taproot_outputs: u64,
    /// Approximate BTC/USD price on this date (from blockchain.info or hardcoded early prices).
    pub price_usd: f64,
    /// Notable events that occurred on this exact date in this year.
    pub events: Vec<NotableEvent>,
    /// First block height on this date.
    pub first_block: u64,
    /// Last block height on this date.
    pub last_block: u64,
}

/// Full "On This Day" response containing data for every year on a given month+day.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OnThisDayData {
    pub month: u32,
    pub day: u32,
    /// One entry per year that had blocks on this calendar date, newest first.
    pub years: Vec<OnThisDayYear>,
}

/// Mining pool dominance and BTC/USD price context for a time range.
/// Displayed on the Stats Dashboard sidebar.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MiningPriceSummary {
    /// Name of the mining pool with the most blocks in this range.
    pub top_pool_name: String,
    /// Number of blocks mined by the top pool.
    pub top_pool_blocks: u64,
    /// Top pool's share as a percentage of total blocks.
    pub top_pool_pct: f64,
    /// Number of distinct mining pools seen in this range.
    pub pool_count: u64,
    /// BTC/USD price at the start of the range.
    pub price_start: f64,
    /// BTC/USD price at the end of the range.
    pub price_end: f64,
    /// Price change as a percentage ((end - start) / start * 100).
    pub price_change_pct: f64,
}

/// Per-block signaling status for BIP soft fork tracking (version bits or BIP-54 locktime).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalingBlock {
    pub height: u64,
    /// Block timestamp in unix seconds.
    pub timestamp: u64,
    /// Whether this block signaled support (version bit set, or BIP-54 locktime match).
    pub signaled: bool,
    /// Mining pool that mined this block.
    pub miner: String,
}

/// Signaling summary for a 2016-block retarget period.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignalingPeriod {
    /// First block height in this retarget period.
    pub start_height: u64,
    /// Last block height in this retarget period.
    pub end_height: u64,
    /// Number of blocks that signaled support.
    pub signaled_count: u64,
    /// Total blocks mined in this period.
    pub total_blocks: u64,
    /// Percentage of blocks signaling (signaled / total * 100).
    pub signaled_pct: f64,
}

/// Current retarget period stats, used for the live signaling progress bar.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeriodStats {
    /// First block height of the current retarget period.
    pub period_start: u64,
    /// Last block height of the current retarget period (period_start + 2015).
    pub period_end: u64,
    /// Blocks mined so far in this period (excludes the retarget block itself).
    pub total_blocks: u64,
    /// Blocks signaling support so far.
    pub signaled_count: u64,
    /// Current signaling percentage.
    pub signaled_pct: f64,
}

/// Real-time node, mempool, and network stats. Refreshed every 10 seconds
/// and cached server-side. Combines data from multiple RPC calls and external APIs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveStats {
    pub blockchain: LiveBlockchain,
    pub mempool: LiveMempool,
    /// Estimated fee rate (sat/vB) to confirm in the next block (estimatesmartfee target=1).
    pub next_block_fee: f64,
    pub network: LiveNetwork,
}

/// Blockchain state from getblockchaininfo RPC.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveBlockchain {
    /// Current block height (chain tip).
    pub blocks: u64,
    /// Network name ("main", "test", "regtest").
    pub chain: String,
    pub difficulty: f64,
    /// IBD verification progress (0.0 to 1.0).
    pub verification_progress: f64,
    /// Total blockchain data size on disk in bytes.
    pub size_on_disk: u64,
    /// Hash of the current chain tip block.
    pub bestblockhash: String,
    /// Timestamp of the chain tip block in unix seconds.
    pub time: u64,
}

/// Mempool state from getmempoolinfo RPC.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveMempool {
    /// Number of unconfirmed transactions.
    pub size: u64,
    /// Total size of all mempool transactions in bytes.
    pub bytes: u64,
    /// Memory usage of the mempool in bytes.
    pub usage: u64,
    /// Total fees of all mempool transactions in BTC.
    pub total_fee: f64,
    /// Maximum mempool size in bytes (default 300MB).
    pub maxmempool: u64,
    /// Minimum fee rate (BTC/kvB) to enter the mempool.
    pub mempoolminfee: f64,
}

/// Derived network-level stats combining RPC data and external APIs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveNetwork {
    /// Current BTC/USD price from mempool.space API.
    pub price_usd: f64,
    /// Satoshis per US dollar (100M / price_usd).
    pub sats_per_dollar: u64,
    /// Market capitalization in USD (price * circulating supply).
    pub market_cap_usd: f64,
    /// Total BTC mined so far (circulating supply).
    pub total_supply: f64,
    /// Maximum possible supply (21,000,000 BTC).
    pub max_supply: f64,
    /// Percentage of max supply already mined.
    pub percent_issued: f64,
    /// Total UTXO count from gettxoutsetinfo.
    pub utxo_count: u64,
    /// Blockchain size on disk in gigabytes.
    pub chain_size_gb: f64,
    /// Estimated network hash rate in hashes/second from getnetworkhashps.
    pub hashrate: f64,
}

/// Historical BTC/USD price data point from blockchain.info API.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PricePoint {
    /// Timestamp in milliseconds since epoch.
    pub timestamp_ms: u64,
    /// BTC/USD price at this point.
    pub price_usd: f64,
}

/// A mining pool's share of blocks in a given range.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MinerShare {
    /// Mining pool name (e.g. "Foundry USA", "AntPool").
    pub miner: String,
    /// Number of blocks mined.
    pub count: u64,
    /// Percentage of total blocks in the range.
    pub percentage: f64,
}

/// An empty block (coinbase-only, no user transactions).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmptyBlock {
    pub height: u64,
    /// Block timestamp in unix seconds.
    pub timestamp: u64,
    /// Mining pool that mined this empty block.
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
