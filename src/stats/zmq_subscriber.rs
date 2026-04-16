//! ZMQ subscriber: connects to bitcoind's ZMQ interface for real-time
//! mempool transactions and block notifications.
//!
//! ## ZMQ Topics
//!
//! - `rawtx` (port 28333): Raw serialized transactions entering the mempool.
//!   Parsed to extract txid and total output value, then enriched with fee/vsize
//!   from `getmempoolentry` RPC.
//! - `hashblock` (port 28332): 32-byte block hash after validation completes.
//!   Triggers full block data fetch and mempool tx confirmation.
//! - `sequence` (port 28333): Mempool event stream with single-character type codes:
//!   - `A` = tx added to mempool
//!   - `R` = tx removed from mempool (block or conflict)
//!   - `C` = block connected
//!   - `D` = block disconnected (reorg)
//!
//! ## Mining Detection via Sequence Events
//!
//! When a new block arrives, Bitcoin Core removes transactions from the mempool in
//! a rapid burst of `R` (removed) events. By counting R events within a short time
//! window (3 seconds), the subscriber detects block processing before the slower
//! `hashblock` event fires. This triggers the `BlockMining` SSE event, which shows
//! a mining overlay in the frontend UI.
//!
//! ## Heartbeat Event Types
//!
//! - `Tx`: New mempool transaction with fee, vsize, value, and fee rate.
//! - `Block`: Complete block data (height, hash, fees, size, confirmed tx count).
//!   Sent after full validation and data fetch - all fields populated.
//! - `BlockMining`: Lightweight signal that block processing has started.
//!   Detected via R-event burst. Frontend shows mining animation until Block arrives.
//!
//! Transactions are stored in SQLite (`mempool_txs` table) and broadcast
//! to SSE clients via a tokio broadcast channel.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::sync::broadcast;
use zeromq::{Socket, SocketRecv, SubSocket};

use super::api::StatsState;
use super::db;

/// Event broadcast to SSE clients via the heartbeat endpoint.
/// Tagged with `type` for JSON serialization so the frontend can dispatch by event kind.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "type")]
pub enum HeartbeatEvent {
    /// New mempool transaction with fee and size data.
    #[serde(rename = "tx")]
    Tx {
        txid: String,
        /// Transaction fee in satoshis.
        fee: u64,
        /// Virtual size in vbytes.
        vsize: u32,
        /// Total output value in satoshis.
        value: u64,
        /// Fee rate in sat/vB.
        fee_rate: f64,
        /// Unix timestamp when this tx was observed.
        timestamp: u64,
        /// Whether this tx's total output value exceeds the whale threshold ($500K USD).
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        whale: bool,
        /// Estimated USD value of outputs (value * cached_price / 1e8). Only set for whale txs.
        #[serde(skip_serializing_if = "is_zero_f64")]
        value_usd: f64,
        /// Whether this tx has an unusually high fee rate (>500 sat/vB) or absolute fee (>0.05 BTC).
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        fee_outlier: bool,
        /// Consolidation: many inputs (>50) funneled into few outputs.
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        consolidation: bool,
        /// Fan-out: few inputs sprayed to many outputs (>50). Exchange batch payouts.
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        fan_out: bool,
        /// Large inscription: witness data > 100KB.
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        large_inscription: bool,
        /// Round number transfer: an output matches a round BTC amount (1, 10, 100, 1000).
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        round_number: bool,
        /// OP_RETURN contains readable ASCII text (>= 4 printable chars).
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        op_return_msg: bool,
        /// Decoded OP_RETURN text if op_return_msg is true.
        #[serde(skip_serializing_if = "String::is_empty", default)]
        op_return_text: String,
        /// Number of inputs in the transaction.
        input_count: u64,
        /// Number of outputs in the transaction.
        output_count: u64,
    },
    /// Block found - complete data (fees, size, weight all populated).
    /// Sent after node finishes validation and we fetch all metadata.
    #[serde(rename = "block")]
    Block {
        height: u64,
        hash: String,
        /// Block timestamp in unix seconds.
        timestamp: u64,
        tx_count: u64,
        /// Total fees in satoshis (from getblockstats).
        total_fees: u64,
        /// Block size in bytes.
        size: u64,
        /// Block weight in weight units.
        weight: u64,
        /// Number of mempool txs we had tracked that were confirmed in this block.
        confirmed_count: u64,
    },
    /// Block is being mined/validated - node is processing a new block.
    /// Detected via ZMQ sequence `R` burst (txs being removed from mempool).
    /// Frontend shows mining overlay until the complete Block event arrives.
    #[serde(rename = "block_mining")]
    BlockMining,
}

/// Minimum USD value to flag a transaction as a whale tx.
const WHALE_THRESHOLD_USD: f64 = 1_000_000.0;
/// Fee rate above which a tx is flagged as a fee outlier (sat/vB).
/// Raised from 500 to 2000 to reduce false positives during mempool congestion.
const FEE_RATE_OUTLIER_THRESHOLD: f64 = 2000.0;
/// Absolute fee above which a tx is flagged as a fee outlier (satoshis = 0.1 BTC).
/// Raised from 0.05 BTC to avoid flagging large consolidations.
const FEE_ABSOLUTE_OUTLIER_THRESHOLD: u64 = 10_000_000;
/// Input count above which a tx is flagged as a consolidation (with few outputs).
const CONSOLIDATION_INPUT_THRESHOLD: u64 = 50;
/// Output count above which a tx is flagged as a fan-out (with few inputs).
/// Raised from 50 to 100 to focus on genuine batch payouts.
const FAN_OUT_OUTPUT_THRESHOLD: u64 = 100;
/// Witness data size above which a tx is flagged as a large inscription (bytes).
const LARGE_INSCRIPTION_THRESHOLD: u64 = 100_000;
/// Exact round BTC amounts to detect (in satoshis). Humans often send round numbers.
const ROUND_NUMBER_AMOUNTS: &[u64] = &[
    100_000_000,        // 1 BTC
    1_000_000_000,      // 10 BTC
    10_000_000_000,     // 100 BTC
    100_000_000_000,    // 1000 BTC
];
/// Tolerance for round number detection (sats). Allows 0.001 BTC slop for dust change.
const ROUND_NUMBER_TOLERANCE: u64 = 100_000;
/// Minimum USD value for a round number tx to be flagged (avoids 1 BTC dust at low prices).
const ROUND_NUMBER_MIN_USD: f64 = 100_000.0;

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

/// Spawn the ZMQ subscriber tasks. Both tx/sequence topics share a single socket
/// on port 28333 (ZMQ PUB distributes across separate SUB sockets, so splitting
/// would cause each to see only a fraction of messages). Block notifications use
/// a separate socket on port 28332.
pub fn spawn(
    state: Arc<StatsState>,
    tx_sender: broadcast::Sender<HeartbeatEvent>,
    zmq_tx_url: String,
    zmq_block_url: String,
) {
    // Shared set: block subscriber populates with block txids so the tx
    // subscriber can skip them (block tx flood) while still processing
    // genuine new mempool txs.
    let block_txids: Arc<std::sync::Mutex<HashSet<String>>> =
        Arc::new(std::sync::Mutex::new(HashSet::new()));

    // Transaction + Sequence subscriber (both on port 28333, SAME socket).
    // Must share a single socket — ZMQ PUB distributes messages across
    // separate SUB sockets, so two connections would split the stream.
    {
        let state = Arc::clone(&state);
        let sender = tx_sender.clone();
        let url = zmq_tx_url.clone();
        let bt = Arc::clone(&block_txids);
        tokio::spawn(async move {
            loop {
                tracing::info!("ZMQ: connecting to rawtx+sequence at {url}");
                match subscribe_tx_and_sequence(&state, &sender, &url, &bt).await {
                    Ok(()) => tracing::warn!(
                        "ZMQ rawtx+sequence stream ended, reconnecting..."
                    ),
                    Err(e) => tracing::error!(
                        "ZMQ rawtx+sequence error: {e}, reconnecting in 5s..."
                    ),
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }

    // Block subscriber (hashblock on port 28332)
    {
        let state = Arc::clone(&state);
        let sender = tx_sender;
        let url = zmq_block_url.clone();
        let bt = block_txids;
        tokio::spawn(async move {
            loop {
                tracing::info!("ZMQ: connecting to hashblock at {url}");
                match subscribe_blocks(&state, &sender, &url, &bt).await {
                    Ok(()) => tracing::warn!(
                        "ZMQ hashblock stream ended, reconnecting..."
                    ),
                    Err(e) => tracing::error!(
                        "ZMQ hashblock error: {e}, reconnecting in 5s..."
                    ),
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }

    tracing::info!(
        "ZMQ subscriber spawned (tx: {zmq_tx_url}, block: {zmq_block_url})"
    );
}

/// Subscribe to both rawtx and sequence topics on a single socket.
/// Must share one socket because ZMQ PUB distributes messages across
/// separate SUB connections, causing each to see only a fraction.
async fn subscribe_tx_and_sequence(
    state: &Arc<StatsState>,
    sender: &broadcast::Sender<HeartbeatEvent>,
    url: &str,
    block_txids: &std::sync::Mutex<HashSet<String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut socket = SubSocket::new();
    socket.connect(url).await?;
    socket.subscribe("rawtx").await?;
    socket.subscribe("sequence").await?;
    tracing::info!("ZMQ: subscribed to rawtx+sequence");

    let mut tx_count = 0u64;
    let mut parse_fail = 0u64;
    let mut rpc_fail = 0u64;
    let mut consecutive_fail = 0u32;
    let mut seq_state = SequenceState::default();
    let mut seq_event_count = 0u64;

    loop {
        let msg = socket.recv().await?;
        let frames: Vec<_> = msg.into_vec();
        if frames.len() < 2 {
            continue;
        }

        // First frame is the topic: "rawtx" or "sequence"
        let topic = std::str::from_utf8(&frames[0]).unwrap_or("");

        // Handle sequence events (block mining detection)
        if topic == "sequence" {
            seq_event_count += 1;
            if seq_event_count == 1 {
                tracing::info!("ZMQ: first sequence event received (body len={})", frames[1].len());
            }
            let body = &frames[1];
            if body.len() >= 33 {
                let event_type = body[32] as char;
                // Log sequence stats periodically and on state changes
                if event_type == 'C' || event_type == 'D' {
                    tracing::info!(
                        "ZMQ: sequence event '{event_type}' (total={seq_event_count}, r_count={}, mining_sent={})",
                        seq_state.r_count, seq_state.mining_sent
                    );
                }
                if seq_state.process(event_type) {
                    tracing::info!(
                        "ZMQ: sequence detected block processing ({}+ R events)",
                        seq_state.r_count
                    );
                    let _ = sender.send(HeartbeatEvent::BlockMining);
                }
            }
            continue;
        }

        // Skip non-rawtx topics (e.g. hashtx which shares the port)
        if topic != "rawtx" {
            if tx_count == 0 && seq_event_count == 0 {
                tracing::debug!("ZMQ: unexpected topic '{}' (len={})", topic, frames[0].len());
            }
            continue;
        }

        // Handle rawtx events
        let raw_tx = &frames[1];
        let parsed = match parse_raw_tx(raw_tx) {
            Some(p) => p,
            None => {
                parse_fail += 1;
                if parse_fail <= 5 {
                    tracing::warn!(
                        "ZMQ: failed to parse raw tx ({} bytes)",
                        raw_tx.len()
                    );
                }
                continue;
            }
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // During block processing, ZMQ re-broadcasts every tx in the block.
        // Skip those (they're in the block_txids set) but let genuine new
        // mempool txs through so the SSE stream doesn't go silent.
        if let Ok(set) = block_txids.lock() {
            if !set.is_empty() && set.contains(&parsed.txid) {
                continue;
            }
        }

        // Self-throttle: if 5+ consecutive RPC failures, a block likely arrived
        // before the hashblock event. Skip to let the flood drain.
        if consecutive_fail >= 5 {
            consecutive_fail = 0;
            tracing::debug!("ZMQ: skipped rawtx (consecutive failures, likely block flood)");
            continue;
        }

        // Look up fee + authoritative vsize from mempool entry
        let (fee, vsize) = match state.rpc.get_mempool_entry(&parsed.txid).await
        {
            Ok(entry) => {
                consecutive_fail = 0;
                (entry.fee, entry.vsize)
            }
            Err(_) => {
                rpc_fail += 1;
                consecutive_fail += 1;
                if rpc_fail <= 5 {
                    tracing::debug!(
                        "ZMQ: getmempoolentry failed for {} (may be already confirmed)",
                        parsed.txid
                    );
                }
                continue;
            }
        };

        let fee_rate = if vsize > 0 {
            fee as f64 / vsize as f64
        } else {
            0.0
        };

        // Get cached price once for all USD-based detections
        let price_usd = state
            .price_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|(p, _)| p.usd)
            .unwrap_or(0.0);

        // Whale: use non_change_value (total - largest output) to avoid inflating by change.
        // This gives a better "actual transfer" estimate for self-spends/consolidations.
        let whale = if price_usd > 0.0 {
            let transfer_sats = if parsed.non_change_value > 0 {
                parsed.non_change_value
            } else {
                parsed.value
            };
            let usd = transfer_sats as f64 * price_usd / 100_000_000.0;
            usd >= WHALE_THRESHOLD_USD
        } else {
            false
        };

        // Notable tx detection
        let fee_outlier = fee_rate >= FEE_RATE_OUTLIER_THRESHOLD
            || fee >= FEE_ABSOLUTE_OUTLIER_THRESHOLD;
        let consolidation = parsed.input_count >= CONSOLIDATION_INPUT_THRESHOLD
            && parsed.output_count <= 3;
        let fan_out = parsed.input_count <= 3
            && parsed.output_count >= FAN_OUT_OUTPUT_THRESHOLD;
        let large_inscription = parsed.witness_bytes >= LARGE_INSCRIPTION_THRESHOLD;

        // Round number detection: any output exactly matches a round BTC amount.
        // Only flag if the tx is substantial (at least $100k) to avoid 1 BTC dust.
        let round_number = {
            let matches_round = ROUND_NUMBER_AMOUNTS.iter().any(|&amt| {
                parsed.max_output_value >= amt.saturating_sub(ROUND_NUMBER_TOLERANCE)
                    && parsed.max_output_value <= amt + ROUND_NUMBER_TOLERANCE
            });
            if matches_round && price_usd > 0.0 {
                let max_usd = parsed.max_output_value as f64 * price_usd / 100_000_000.0;
                max_usd >= ROUND_NUMBER_MIN_USD
            } else {
                false
            }
        };

        let op_return_msg = parsed.op_return_text.is_some();

        // Determine notable type (priority order: structural > value > fee > data)
        // Structural patterns (consolidation/fan_out) take priority over fee_outlier
        // because they're more informative signals.
        let notable_type = if whale {
            Some("whale")
        } else if round_number {
            Some("round_number")
        } else if large_inscription {
            Some("large_inscription")
        } else if consolidation {
            Some("consolidation")
        } else if fan_out {
            Some("fan_out")
        } else if fee_outlier {
            Some("fee_outlier")
        } else if op_return_msg {
            Some("op_return_msg")
        } else {
            None
        };

        if let Some(nt) = notable_type {
            tracing::info!(
                "ZMQ: notable tx [{}] {} — {:.4} BTC, {fee} sat fee, {fee_rate:.1} sat/vB, {}in/{}out, {}B witness",
                nt,
                parsed.txid,
                parsed.value as f64 / 100_000_000.0,
                parsed.input_count,
                parsed.output_count,
                parsed.witness_bytes,
            );
        }

        // Compute USD value for any notable tx (not just whales).
        // Uses non_change_value as a rough "transfer" estimate.
        let is_notable = notable_type.is_some();
        let broadcast_usd = if is_notable && price_usd > 0.0 {
            let transfer_sats = if parsed.non_change_value > 0 {
                parsed.non_change_value
            } else {
                parsed.value
            };
            transfer_sats as f64 * price_usd / 100_000_000.0
        } else {
            0.0
        };

        // Store in DB (with notable info for persistence across restarts)
        if let Ok(conn) = state.db.get() {
            let _ = db::insert_mempool_tx(
                &conn,
                &parsed.txid,
                fee,
                vsize,
                parsed.value,
                now,
                notable_type,
                if is_notable && broadcast_usd > 0.0 { Some(broadcast_usd) } else { None },
            );

            // Also persist to notable_txs table (separate from mempool_txs, never pruned).
            if is_notable {
                let _ = db::insert_notable_tx(
                    &conn,
                    &parsed.txid,
                    notable_type.unwrap_or(""),
                    fee,
                    vsize,
                    parsed.value,
                    parsed.max_output_value,
                    broadcast_usd,
                    parsed.input_count,
                    parsed.output_count,
                    parsed.witness_bytes,
                    parsed.op_return_text.as_deref(),
                    now,
                );
            }
        }

        // Broadcast to SSE clients
        let _ = sender.send(HeartbeatEvent::Tx {
            txid: parsed.txid,
            fee,
            vsize,
            value: parsed.value,
            fee_rate,
            timestamp: now,
            whale,
            value_usd: broadcast_usd,
            fee_outlier,
            consolidation,
            fan_out,
            large_inscription,
            round_number,
            op_return_msg,
            op_return_text: parsed.op_return_text.unwrap_or_default(),
            input_count: parsed.input_count,
            output_count: parsed.output_count,
        });

        tx_count += 1;
        if tx_count == 1 {
            tracing::info!(
                "ZMQ: first tx processed — {fee} sats fee, {vsize} vB"
            );
        }
        if tx_count.is_multiple_of(100) {
            tracing::info!("ZMQ: processed {tx_count} transactions (parse_fail={parse_fail}, rpc_fail={rpc_fail})");
        }
    }
}

/// Minimum R events within the time window to trigger a BlockMining event.
const SEQUENCE_R_THRESHOLD: u32 = 5;
/// Time window in seconds to accumulate R events before resetting.
const SEQUENCE_R_WINDOW_SECS: f64 = 3.0;

/// Time-window based state machine for detecting block processing via ZMQ sequence events.
///
/// Counts `R` (removed from mempool) events within a rolling window. When the count
/// crosses `SEQUENCE_R_THRESHOLD`, a `BlockMining` event is emitted once. The state
/// resets on `C` (block connected) or `D` (block disconnected/reorg). `A` (added)
/// events are ignored since they interleave with R events during block processing
/// on slower hardware.
#[derive(Default)]
struct SequenceState {
    /// Number of R events in the current time window.
    r_count: u32,
    /// Start of the current time window (None = no active window).
    window_start: Option<std::time::Instant>,
    /// Whether BlockMining has already been sent for this window.
    mining_sent: bool,
}


impl SequenceState {
    /// Process a sequence event type character. Returns true if BlockMining
    /// should be broadcast (first time threshold is crossed in a time window).
    fn process(&mut self, event_type: char) -> bool {
        self.process_with_time(event_type, std::time::Instant::now())
    }

    /// Testable version that accepts an explicit timestamp.
    fn process_with_time(&mut self, event_type: char, now: std::time::Instant) -> bool {
        match event_type {
            'R' => {
                match self.window_start {
                    Some(start)
                        if now.duration_since(start).as_secs_f64()
                            <= SEQUENCE_R_WINDOW_SECS =>
                    {
                        self.r_count += 1;
                    }
                    _ => {
                        // Start new window
                        self.window_start = Some(now);
                        self.r_count = 1;
                    }
                }
                if self.r_count >= SEQUENCE_R_THRESHOLD && !self.mining_sent {
                    self.mining_sent = true;
                    return true;
                }
            }
            'C' => {
                if self.r_count > 0 || self.mining_sent {
                    tracing::info!(
                        "ZMQ: sequence block connected ({} R events in window, mining_sent={})",
                        self.r_count, self.mining_sent
                    );
                }
                self.r_count = 0;
                self.window_start = None;
                self.mining_sent = false;
            }
            'D' => {
                tracing::warn!("ZMQ: sequence block disconnected (reorg)");
                self.r_count = 0;
                self.window_start = None;
                self.mining_sent = false;
            }
            'A' => {
                // Ignore. A events interleave with R events during block
                // processing, so we let the time window handle expiry.
            }
            _ => {}
        }
        false
    }
}

/// Subscribe to block hashes. After hashblock fires (validation complete),
/// fetch all block data synchronously then broadcast ONE complete event.
/// The sequence subscriber handles the mining overlay during the wait.
async fn subscribe_blocks(
    state: &Arc<StatsState>,
    sender: &broadcast::Sender<HeartbeatEvent>,
    url: &str,
    block_txids: &Arc<std::sync::Mutex<HashSet<String>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut socket = SubSocket::new();
    socket.connect(url).await?;
    socket.subscribe("hashblock").await?;
    tracing::info!("ZMQ: subscribed to hashblock");

    loop {
        let msg = socket.recv().await?;
        let frames: Vec<_> = msg.into_vec();
        if frames.len() < 2 {
            continue;
        }

        let hash_bytes = &frames[1];
        if hash_bytes.len() != 32 {
            tracing::warn!(
                "ZMQ: unexpected hashblock size: {}",
                hash_bytes.len()
            );
            continue;
        }
        let block_hash = bytes_to_hex(hash_bytes);
        tracing::info!("ZMQ: hashblock {block_hash} — fetching full data");

        // Show mining overlay immediately while we fetch block data
        let _ = sender.send(HeartbeatEvent::BlockMining);

        // Node just finished validation — RPC is available now.
        // Fetch all data synchronously for a single complete broadcast.
        let block_info = match get_block_info(state, &block_hash).await {
            Some(info) => info,
            None => continue,
        };

        // Populate the txid filter so the tx subscriber skips block txs
        if let Ok(mut set) = block_txids.lock() {
            set.clear();
            for txid in &block_info.txids {
                set.insert(txid.clone());
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Confirm mempool txs + get authoritative fees in parallel
        let db = state.db.clone();
        let txids = block_info.txids.clone();
        let height = block_info.height;
        let txid_count = txids.len();

        let confirm_fut = tokio::task::spawn_blocking(move || {
            match db.get() {
                Ok(conn) => {
                    // Also mark notable txs as confirmed
                    let _ = db::confirm_notable_txs(&conn, &txids, height, now);
                    db::confirm_mempool_txs(&conn, &txids, height, now)
                }
                Err(e) => {
                    tracing::error!(
                        "ZMQ: DB error for block {height}: {e}"
                    );
                    Ok((0, 0))
                }
            }
        });
        let fees_fut = state.rpc.get_block_total_fee(block_info.height);

        let (confirm_result, fees_result) =
            tokio::join!(confirm_fut, fees_fut);

        let confirmed_count = match confirm_result {
            Ok(Ok((count, _))) => count,
            Ok(Err(e)) => {
                tracing::error!("ZMQ: DB error confirming txs for block {}: {e}", block_info.height);
                0
            }
            Err(e) => {
                tracing::error!("ZMQ: spawn_blocking panicked for block {}: {e}", block_info.height);
                0
            }
        };
        let total_fees = match fees_result {
            Ok(fees) => fees,
            Err(e) => {
                tracing::warn!("ZMQ: getblockstats failed for block {}: {e}", block_info.height);
                0
            }
        };

        tracing::info!(
            "ZMQ: block {} ({block_hash}) — {confirmed_count}/{txid_count} confirmed, {:.4} BTC fees, size={}, weight={}",
            block_info.height,
            total_fees as f64 / 100_000_000.0,
            block_info.size,
            block_info.weight,
        );

        // Broadcast ONE complete block event
        let _ = sender.send(HeartbeatEvent::Block {
            height: block_info.height,
            hash: block_hash,
            timestamp: block_info.timestamp,
            tx_count: block_info.tx_count,
            total_fees,
            size: block_info.size,
            weight: block_info.weight,
            confirmed_count,
        });

        // Don't clear the txid filter here. ZMQ continues re-broadcasting
        // block txs after we finish processing. If we clear now, those stale
        // txs pass the filter, fail getmempoolentry, and the consecutive_fail
        // throttle can drop a genuine new mempool tx. The set is replaced
        // (clear + repopulate) when the next block arrives at line 268 above.
        // Between blocks the set harmlessly contains the previous block's
        // txids — no new mempool tx can collide (txids are unique).
    }
}

/// Get block metadata and txid list from RPC, with retry.
/// hashblock fires after validation, but RPC may still be briefly busy.
async fn get_block_info(
    state: &Arc<StatsState>,
    hash: &str,
) -> Option<super::rpc::BlockTxids> {
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        match state.rpc.get_block_txids(hash).await {
            Ok(info) => {
                return Some(info);
            }
            Err(e) => {
                if attempt < 2 {
                    tracing::debug!("ZMQ: block {hash} not ready yet (attempt {}), retrying...", attempt + 1);
                } else {
                    tracing::error!("ZMQ: failed to get block txids for {hash} after 3 attempts: {e}");
                }
            }
        }
    }
    None
}

// === Raw transaction parsing ===

/// Minimal parsed info from a raw Bitcoin transaction.
struct ParsedTx {
    txid: String,
    value: u64,                   // sum of output values in sats
    input_count: u64,             // number of inputs
    output_count: u64,            // number of outputs
    witness_bytes: u64,           // total witness data size in bytes
    max_output_value: u64,        // largest single output (for round number detection)
    non_change_value: u64,        // total value minus largest output (rough "transfer" estimate)
    op_return_text: Option<String>, // first OP_RETURN output with readable ASCII
}

/// Parse a raw Bitcoin transaction to extract txid and total output value.
/// Handles both legacy and segwit (BIP 141) formats.
fn parse_raw_tx(data: &[u8]) -> Option<ParsedTx> {
    let mut cursor = 0;

    if data.len() < 10 {
        return None;
    }

    // Version: 4 bytes LE
    let _version = read_u32_le(data, &mut cursor)?;

    // Check for segwit marker (0x00) + flag (0x01)
    let is_segwit =
        data.get(cursor) == Some(&0x00) && data.get(cursor + 1) == Some(&0x01);
    if is_segwit {
        cursor += 2;
    }

    // Input count
    let input_count = read_varint(data, &mut cursor)?;

    // Skip inputs (we don't need their data)
    for _ in 0..input_count {
        cursor += 32; // prev txid
        cursor += 4; // prev vout
        let script_len = read_varint(data, &mut cursor)? as usize;
        cursor += script_len; // scriptSig
        cursor += 4; // sequence
        if cursor > data.len() {
            return None;
        }
    }

    // Output count
    let output_count = read_varint(data, &mut cursor)?;

    // Parse outputs for value, track max output, detect OP_RETURN text
    let mut total_value = 0u64;
    let mut max_output_value = 0u64;
    let mut op_return_text: Option<String> = None;
    for _ in 0..output_count {
        let value = read_u64_le(data, &mut cursor)?;
        total_value = total_value.saturating_add(value);
        if value > max_output_value {
            max_output_value = value;
        }
        let script_len = read_varint(data, &mut cursor)? as usize;
        if cursor + script_len > data.len() {
            return None;
        }
        // Detect OP_RETURN: 0x6a prefix, scan for readable ASCII
        if op_return_text.is_none() && script_len >= 4 && data[cursor] == 0x6a {
            let payload = &data[cursor + 1..cursor + script_len];
            if let Some(text) = extract_readable_text(payload) {
                op_return_text = Some(text);
            }
        }
        cursor += script_len; // scriptPubKey
        if cursor > data.len() {
            return None;
        }
    }

    // Sanity check: Bitcoin supply is capped at 21M BTC = 2.1 * 10^15 sats.
    // Any higher value indicates parse corruption.
    const MAX_SUPPLY_SATS: u64 = 21_000_000 * 100_000_000;
    if total_value > MAX_SUPPLY_SATS {
        return None;
    }

    let non_change_value = total_value.saturating_sub(max_output_value);

    // For txid: we need the non-witness serialization (version + inputs + outputs + locktime)
    // Build it by stripping segwit marker/flag and witness data
    let mut total_witness_bytes = 0u64;
    let txid = if is_segwit {
        // Non-witness serialization: version(4) + vin + vout + locktime(4)
        // We need to reconstruct this from the original data
        let mut stripped = Vec::with_capacity(data.len());
        stripped.extend_from_slice(&data[0..4]); // version

        // Copy from after segwit marker to start of witness data
        // The witness data starts after all outputs, which is at `cursor`
        stripped.extend_from_slice(&data[6..cursor]); // skip 4 (version) + 2 (marker+flag)

        // Skip witness data to find locktime, tracking total witness bytes
        let mut wit_cursor = cursor;
        for _ in 0..input_count {
            let wit_count = read_varint(data, &mut wit_cursor)?;
            for _ in 0..wit_count {
                let item_len = read_varint(data, &mut wit_cursor)? as usize;
                if wit_cursor + item_len > data.len() {
                    return None;
                }
                total_witness_bytes += item_len as u64;
                wit_cursor += item_len;
            }
        }

        // Locktime: last 4 bytes after witness
        if wit_cursor + 4 > data.len() {
            return None;
        }
        stripped.extend_from_slice(&data[wit_cursor..wit_cursor + 4]);

        sha256d_hex(&stripped)
    } else {
        // Legacy tx: entire data is the serialization
        sha256d_hex(data)
    };

    Some(ParsedTx {
        txid,
        value: total_value,
        input_count,
        output_count,
        witness_bytes: total_witness_bytes,
        max_output_value,
        non_change_value,
        op_return_text,
    })
}

/// Extract readable ASCII text from OP_RETURN payload.
/// Returns Some(text) if at least 4 consecutive printable chars found.
/// Strips common protocol prefixes (Runes, SRC-20, etc.) to surface actual messages.
fn extract_readable_text(payload: &[u8]) -> Option<String> {
    // Skip push opcodes at start (OP_PUSH* or OP_PUSHDATA*)
    let mut start = 0;
    while start < payload.len() && payload[start] < 0x20 {
        start += 1;
        if start >= payload.len() { return None; }
    }
    let slice = &payload[start..];

    // Require high printable ratio (>= 70%) to filter binary noise
    let printable_count = slice.iter()
        .filter(|&&b| b >= 0x20 && b <= 0x7e)
        .count();
    if printable_count < 8 || printable_count * 100 < slice.len() * 70 {
        return None;
    }

    // Build string from printable chars, collapse runs of non-printable to single space
    let mut result = String::with_capacity(slice.len());
    let mut last_space = false;
    for &b in slice {
        if b >= 0x20 && b <= 0x7e {
            result.push(b as char);
            last_space = false;
        } else if !last_space {
            result.push(' ');
            last_space = true;
        }
    }
    let trimmed = result.trim().to_string();

    // Require substantial alphabetic content (>= 50% letters).
    let letter_count = trimmed.chars().filter(|c| c.is_alphabetic()).count();
    if letter_count < 5 || letter_count * 2 < trimmed.len() {
        return None;
    }

    // Require minimum length for meaningful text
    if trimmed.len() < 8 {
        return None;
    }

    // Require at least one word of 4+ letters (filters "ifi" / "a bc" noise)
    let has_word = trimmed.split_whitespace()
        .any(|w| w.chars().filter(|c| c.is_alphabetic()).count() >= 4);
    if !has_word {
        return None;
    }

    Some(trimmed.chars().take(200).collect())
}

/// Double SHA256, return as reversed hex (Bitcoin txid convention).
fn sha256d_hex(data: &[u8]) -> String {
    let first = Sha256::digest(data);
    let second = Sha256::digest(first);
    bytes_to_hex_reversed(&second)
}

/// Convert bytes to hex string in reversed byte order (Bitcoin txid convention).
fn bytes_to_hex_reversed(bytes: &[u8]) -> String {
    bytes.iter().rev().map(|b| format!("{b:02x}")).collect()
}

/// Convert bytes to hex string in direct order.
fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Read a little-endian u32 from data at cursor position.
fn read_u32_le(data: &[u8], cursor: &mut usize) -> Option<u32> {
    if *cursor + 4 > data.len() {
        return None;
    }
    let val = u32::from_le_bytes(data[*cursor..*cursor + 4].try_into().ok()?);
    *cursor += 4;
    Some(val)
}

/// Read a little-endian u64 from data at cursor position.
fn read_u64_le(data: &[u8], cursor: &mut usize) -> Option<u64> {
    if *cursor + 8 > data.len() {
        return None;
    }
    let val = u64::from_le_bytes(data[*cursor..*cursor + 8].try_into().ok()?);
    *cursor += 8;
    Some(val)
}

/// Read a Bitcoin-style varint from data at cursor position.
fn read_varint(data: &[u8], cursor: &mut usize) -> Option<u64> {
    if *cursor >= data.len() {
        return None;
    }
    let first = data[*cursor];
    *cursor += 1;
    match first {
        0..=0xFC => Some(first as u64),
        0xFD => {
            if *cursor + 2 > data.len() {
                return None;
            }
            let val =
                u16::from_le_bytes(data[*cursor..*cursor + 2].try_into().ok()?);
            *cursor += 2;
            Some(val as u64)
        }
        0xFE => {
            if *cursor + 4 > data.len() {
                return None;
            }
            let val =
                u32::from_le_bytes(data[*cursor..*cursor + 4].try_into().ok()?);
            *cursor += 4;
            Some(val as u64)
        }
        0xFF => {
            if *cursor + 8 > data.len() {
                return None;
            }
            let val =
                u64::from_le_bytes(data[*cursor..*cursor + 8].try_into().ok()?);
            *cursor += 8;
            Some(val)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decode hex string to bytes (test helper).
    fn hex_decode(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    // --- extract_readable_text tests ---

    #[test]
    fn test_extract_readable_text_alphabetic() {
        let text = b"Hello, world from Bitcoin!";
        let result = extract_readable_text(text);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Hello"));
    }

    #[test]
    fn test_extract_readable_text_too_short() {
        let text = b"hi";
        assert_eq!(extract_readable_text(text), None);
    }

    #[test]
    fn test_extract_readable_text_binary_junk() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        assert_eq!(extract_readable_text(&data), None);
    }

    #[test]
    fn test_extract_readable_text_needs_letters() {
        let data = b"1234567890";
        assert_eq!(extract_readable_text(data), None);
    }

    #[test]
    fn test_extract_readable_text_rejects_low_quality() {
        // "=|1ifi T" style noise - has letters but not a real message
        assert_eq!(extract_readable_text(b"=|1ifi T"), None);
        assert_eq!(extract_readable_text(b"x!@#$%^&"), None);
    }

    #[test]
    fn test_extract_readable_text_with_binary_wrapper() {
        // Binary prefix (push opcodes) followed by real text
        let mut data = vec![0x0c]; // push 12 bytes
        data.extend_from_slice(b"Hello Bitcoin!");
        let result = extract_readable_text(&data);
        assert!(result.is_some());
    }

    // --- read_varint tests ---

    #[test]
    fn test_read_varint_single_byte() {
        let data = [0x05];
        let mut cursor = 0;
        assert_eq!(read_varint(&data, &mut cursor), Some(5));
        assert_eq!(cursor, 1);
    }

    #[test]
    fn test_read_varint_fd_prefix() {
        // 0xFD followed by u16 LE: 0x0100 = 256
        let data = [0xFD, 0x00, 0x01];
        let mut cursor = 0;
        assert_eq!(read_varint(&data, &mut cursor), Some(256));
        assert_eq!(cursor, 3);
    }

    #[test]
    fn test_read_varint_fe_prefix() {
        // 0xFE followed by u32 LE: 0x00010001 = 65537
        let data = [0xFE, 0x01, 0x00, 0x01, 0x00];
        let mut cursor = 0;
        assert_eq!(read_varint(&data, &mut cursor), Some(65537));
        assert_eq!(cursor, 5);
    }

    #[test]
    fn test_read_varint_ff_prefix() {
        // 0xFF followed by u64 LE: 0x0000000100000000 = 4294967296
        let data = [0xFF, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let mut cursor = 0;
        assert_eq!(read_varint(&data, &mut cursor), Some(4294967296));
        assert_eq!(cursor, 9);
    }

    #[test]
    fn test_read_varint_empty_returns_none() {
        let data: [u8; 0] = [];
        let mut cursor = 0;
        assert_eq!(read_varint(&data, &mut cursor), None);
    }

    #[test]
    fn test_read_varint_truncated_fd() {
        // 0xFD but only 1 byte after instead of 2
        let data = [0xFD, 0x01];
        let mut cursor = 0;
        assert_eq!(read_varint(&data, &mut cursor), None);
    }

    // --- bytes_to_hex tests ---

    #[test]
    fn test_bytes_to_hex() {
        assert_eq!(bytes_to_hex(&[0x00, 0xab, 0xff]), "00abff");
    }

    #[test]
    fn test_bytes_to_hex_empty() {
        assert_eq!(bytes_to_hex(&[]), "");
    }

    // --- bytes_to_hex_reversed tests ---

    #[test]
    fn test_bytes_to_hex_reversed() {
        assert_eq!(bytes_to_hex_reversed(&[0x00, 0xab, 0xff]), "ffab00");
    }

    #[test]
    fn test_bytes_to_hex_reversed_empty() {
        assert_eq!(bytes_to_hex_reversed(&[]), "");
    }

    // --- sha256d_hex tests ---

    #[test]
    fn test_sha256d_hex_empty() {
        // SHA256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        // SHA256(above) = 5df6e0e2761359d30a8275058e299fcc0381534545f55cf43e41983f5d4c9456
        // Reversed: 56944c5d3f98413ef45cf54545538103cc9f298e0575820ad3591376e2e0f65d
        let result = sha256d_hex(&[]);
        assert_eq!(
            result,
            "56944c5d3f98413ef45cf54545538103cc9f298e0575820ad3591376e2e0f65d"
        );
    }

    // --- parse_raw_tx tests ---

    #[test]
    fn test_parse_raw_tx_satoshi_to_finney() {
        // First Bitcoin transaction: Satoshi -> Hal Finney
        // txid: f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16
        let hex = "0100000001c997a5e56e104102fa209c6a852dd90660a20b2d9c352423edce25857fcd3704000000004847304402204e45e16932b8af514961a1d3a1a25fdf3f4f7732e9d624c6c61548ab5fb8cd410220181522ec8eca07de4860a4acdd12909d831cc56cbbac4622082221a8768d1d0901ffffffff0200ca9a3b00000000434104ae1a62fe09c5f51b13905f07f06b99a2f7159b2225f374cd378d71302fa28414e7aab37397f554a7df5f142c21c1b7303b8a0626f1baded5c72a704f7e6cd84cac00286bee0000000043410411db93e1dcdb8a016b49840f8c53bc1eb68a382e97b1482ecad7b148a6909a5cb2e0eaddfb84ccf9744464f82e160bfa9b8b64f9d4c03f999b8643f656b412a3ac00000000";
        let data = hex_decode(hex);

        let parsed = parse_raw_tx(&data).expect("should parse legacy tx");

        assert_eq!(
            parsed.txid,
            "f4184fc596403b9d638783cf57adfe4c75c605f6356fbc91338530e9831e9e16"
        );
        // Output 0: 10 BTC = 1_000_000_000 sats
        // Output 1: 40 BTC = 4_000_000_000 sats
        assert_eq!(parsed.value, 5_000_000_000);
    }

    #[test]
    fn test_parse_raw_tx_truncated_returns_none() {
        // Less than 10 bytes should return None
        assert!(parse_raw_tx(&[0x01, 0x00, 0x00, 0x00, 0x01]).is_none());
        assert!(parse_raw_tx(&[]).is_none());
    }

    #[test]
    fn test_parse_raw_tx_segwit_detection() {
        // Construct a minimal segwit tx:
        // version(4) + marker(0x00) + flag(0x01) + input_count(1) +
        //   input: prev_txid(32) + prev_vout(4) + scriptSig_len(0) + sequence(4) +
        // output_count(1) +
        //   output: value(8) + scriptPubKey_len(1) + scriptPubKey(1) +
        // witness: 1 item with 1 byte +
        // locktime(4)
        let mut tx = Vec::new();
        // Version
        tx.extend_from_slice(&1u32.to_le_bytes());
        // Segwit marker + flag
        tx.push(0x00);
        tx.push(0x01);
        // 1 input
        tx.push(0x01);
        // prev txid (32 zeros)
        tx.extend_from_slice(&[0u8; 32]);
        // prev vout
        tx.extend_from_slice(&0u32.to_le_bytes());
        // scriptSig length = 0
        tx.push(0x00);
        // sequence
        tx.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        // 1 output
        tx.push(0x01);
        // value: 50000 sats
        tx.extend_from_slice(&50000u64.to_le_bytes());
        // scriptPubKey: OP_TRUE (1 byte)
        tx.push(0x01);
        tx.push(0x51);
        // Witness for input 0: 1 item
        tx.push(0x01);
        // Item: 1 byte (0xAA)
        tx.push(0x01);
        tx.push(0xAA);
        // Locktime
        tx.extend_from_slice(&0u32.to_le_bytes());

        let parsed = parse_raw_tx(&tx).expect("should parse segwit tx");
        assert_eq!(parsed.value, 50000);
        // txid should be a 64-char hex string
        assert_eq!(parsed.txid.len(), 64);
    }

    // --- read_u32_le / read_u64_le boundary tests ---

    #[test]
    fn test_read_u32_le_insufficient_data() {
        let data = [0x01, 0x02, 0x03];
        let mut cursor = 0;
        assert_eq!(read_u32_le(&data, &mut cursor), None);
    }

    #[test]
    fn test_read_u64_le_value() {
        let data = 42u64.to_le_bytes();
        let mut cursor = 0;
        assert_eq!(read_u64_le(&data, &mut cursor), Some(42));
        assert_eq!(cursor, 8);
    }

    // --- SequenceState tests (time-window based) ---

    use std::time::{Duration, Instant};

    #[test]
    fn test_sequence_block_detection_within_window() {
        let mut state = SequenceState::default();
        let t0 = Instant::now();
        // 4 R events within window, should not trigger (threshold=5)
        for i in 0..4 {
            let t = t0 + Duration::from_millis(i * 100);
            assert!(!state.process_with_time('R', t));
        }
        assert_eq!(state.r_count, 4);
        assert!(!state.mining_sent);
        // 5th R triggers
        assert!(state.process_with_time('R', t0 + Duration::from_millis(400)));
        assert!(state.mining_sent);
        assert_eq!(state.r_count, 5);
        // Further R events don't re-trigger
        assert!(!state.process_with_time('R', t0 + Duration::from_millis(500)));
    }

    #[test]
    fn test_sequence_r_events_outside_window_reset() {
        let mut state = SequenceState::default();
        let t0 = Instant::now();
        // 3 R events
        for i in 0..3 {
            state.process_with_time('R', t0 + Duration::from_millis(i * 100));
        }
        assert_eq!(state.r_count, 3);
        // Gap of 5 seconds (outside 3s window), new R starts fresh
        state.process_with_time('R', t0 + Duration::from_secs(5));
        assert_eq!(state.r_count, 1);
    }

    #[test]
    fn test_sequence_a_does_not_reset_r_count() {
        let mut state = SequenceState::default();
        let t0 = Instant::now();
        // R events interleaved with A events (real-world pattern)
        state.process_with_time('R', t0);
        state.process_with_time('R', t0 + Duration::from_millis(50));
        state.process_with_time('A', t0 + Duration::from_millis(80));
        state.process_with_time('R', t0 + Duration::from_millis(100));
        state.process_with_time('A', t0 + Duration::from_millis(130));
        state.process_with_time('R', t0 + Duration::from_millis(150));
        assert_eq!(state.r_count, 4);
        // 5th R triggers even with A interleaving
        assert!(state.process_with_time('R', t0 + Duration::from_millis(200)));
        assert!(state.mining_sent);
    }

    #[test]
    fn test_sequence_c_resets_state() {
        let mut state = SequenceState::default();
        let t0 = Instant::now();
        for i in 0..10 {
            state.process_with_time('R', t0 + Duration::from_millis(i * 50));
        }
        assert!(state.mining_sent);
        assert!(!state.process_with_time('C', t0 + Duration::from_millis(600)));
        assert_eq!(state.r_count, 0);
        assert!(!state.mining_sent);
        assert!(state.window_start.is_none());
    }

    #[test]
    fn test_sequence_reorg_resets() {
        let mut state = SequenceState::default();
        let t0 = Instant::now();
        for i in 0..10 {
            state.process_with_time('R', t0 + Duration::from_millis(i * 50));
        }
        assert!(state.mining_sent);
        state.process_with_time('D', t0 + Duration::from_millis(600));
        assert_eq!(state.r_count, 0);
        assert!(!state.mining_sent);
    }

    #[test]
    fn test_sequence_multiple_blocks() {
        let mut state = SequenceState::default();
        let t0 = Instant::now();
        // First block: 10 R events in 500ms
        for i in 0..10 {
            state.process_with_time('R', t0 + Duration::from_millis(i * 50));
        }
        assert!(state.mining_sent);
        state.process_with_time('C', t0 + Duration::from_secs(1));
        // Normal txs between blocks
        state.process_with_time('A', t0 + Duration::from_secs(2));
        state.process_with_time('A', t0 + Duration::from_secs(3));
        // Second block: 8 R events in 400ms
        let t1 = t0 + Duration::from_secs(600);
        let mut triggered = false;
        for i in 0..8 {
            if state.process_with_time('R', t1 + Duration::from_millis(i * 50)) {
                triggered = true;
            }
        }
        assert!(triggered);
        assert!(state.mining_sent);
        state.process_with_time('C', t1 + Duration::from_secs(1));
        assert!(!state.mining_sent);
    }

    #[test]
    fn test_sequence_slow_evictions_no_false_positive() {
        let mut state = SequenceState::default();
        let t0 = Instant::now();
        // 10 R events spread over 40 seconds (normal evictions, not a block)
        for i in 0..10 {
            state.process_with_time('R', t0 + Duration::from_secs(i * 4));
        }
        // Each R starts a new window since the previous one expired (4s > 3s window)
        assert!(!state.mining_sent);
        assert_eq!(state.r_count, 1); // only the last one counts
    }
}

/// Delete mempool_txs entries older than 7 days. Runs on startup and then daily.
/// Keeps the table from growing unbounded since confirmed txs are never cleaned
/// automatically.
pub async fn prune_old_txs(state: &Arc<StatsState>) {
    let seven_days_ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(7 * 24 * 3600);

    let pool = state.db.clone();
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(conn) = pool.get() {
            match db::prune_mempool_txs(&conn, seven_days_ago) {
                Ok(count) => {
                    if count > 0 {
                        tracing::info!(
                            "ZMQ: pruned {count} old mempool transactions"
                        );
                    }
                }
                Err(e) => tracing::warn!("ZMQ: prune failed: {e}"),
            }
        }
    })
    .await;
}
