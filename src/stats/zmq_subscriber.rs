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
use super::notable::{
    classify_notable, extract_readable_text, has_inscription_marker, ParsedTx,
};

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
        /// Largest single output value in sats. Used for round_number display.
        #[serde(skip_serializing_if = "is_zero")]
        max_output_value: u64,
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

// Detection thresholds, classifier, and ParsedTx now live in `super::notable`.

fn is_zero_f64(v: &f64) -> bool {
    *v == 0.0
}

fn is_zero(v: &u64) -> bool {
    *v == 0
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
                match subscribe_tx_and_sequence(&state, &sender, &url, &bt)
                    .await
                {
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
                tracing::info!(
                    "ZMQ: first sequence event received (body len={})",
                    frames[1].len()
                );
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
                tracing::debug!(
                    "ZMQ: unexpected topic '{}' (len={})",
                    topic,
                    frames[0].len()
                );
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
            tracing::debug!(
                "ZMQ: skipped rawtx (consecutive failures, likely block flood)"
            );
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

        // Classify this tx. The returned flags expose every category
        // directly so the SSE broadcast doesn't have to re-run the threshold
        // checks (a previous hand-rolled duplicate got out of sync once).
        let flags = classify_notable(&parsed, fee, fee_rate, price_usd);
        let notable_type = flags.primary_type();

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

        // Compute USD value for any notable tx for broadcast/persistence.
        // Uses total output value (what users expect to see); the whale
        // threshold check itself already happened inside classify_notable.
        let is_notable = flags.is_notable();
        let broadcast_usd = if is_notable && price_usd > 0.0 {
            parsed.value as f64 * price_usd / 100_000_000.0
        } else {
            0.0
        };

        // Store in DB (with notable info for persistence across restarts)
        if let Ok(conn) = state.db.get() {
            let value_usd_opt = if is_notable && broadcast_usd > 0.0 {
                Some(broadcast_usd)
            } else {
                None
            };
            let _ = db::insert_mempool_tx(
                &conn,
                &db::MempoolTxInsert {
                    txid: &parsed.txid,
                    fee,
                    vsize,
                    value: parsed.value,
                    first_seen: now,
                    notable_type,
                    value_usd: value_usd_opt,
                    input_count: parsed.input_count,
                    output_count: parsed.output_count,
                    op_return_text: parsed.op_return_text.as_deref(),
                },
            );

            // Also persist to notable_txs table (separate from mempool_txs, never pruned).
            if is_notable {
                let _ = db::insert_notable_tx(
                    &conn,
                    &db::NotableTxInsert {
                        txid: &parsed.txid,
                        notable_type: notable_type.unwrap_or(""),
                        fee,
                        vsize,
                        value: parsed.value,
                        max_output_value: parsed.max_output_value,
                        value_usd: broadcast_usd,
                        input_count: parsed.input_count,
                        output_count: parsed.output_count,
                        witness_bytes: parsed.witness_bytes,
                        op_return_text: parsed.op_return_text.as_deref(),
                        first_seen: now,
                    },
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
            whale: flags.whale,
            value_usd: broadcast_usd,
            fee_outlier: flags.fee_outlier,
            consolidation: flags.consolidation,
            fan_out: flags.fan_out,
            large_inscription: flags.large_inscription,
            round_number: flags.round_number,
            op_return_msg: flags.op_return_msg,
            op_return_text: parsed.op_return_text.unwrap_or_default(),
            input_count: parsed.input_count,
            output_count: parsed.output_count,
            max_output_value: parsed.max_output_value,
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
    fn process_with_time(
        &mut self,
        event_type: char,
        now: std::time::Instant,
    ) -> bool {
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
                    tracing::error!("ZMQ: DB error for block {height}: {e}");
                    Ok((0, 0))
                }
            }
        });
        let fees_fut = state.rpc.get_block_total_fee(block_info.height);

        let (confirm_result, fees_result) = tokio::join!(confirm_fut, fees_fut);

        let confirmed_count = match confirm_result {
            Ok(Ok((count, _))) => count,
            Ok(Err(e)) => {
                tracing::error!(
                    "ZMQ: DB error confirming txs for block {}: {e}",
                    block_info.height
                );
                0
            }
            Err(e) => {
                tracing::error!(
                    "ZMQ: spawn_blocking panicked for block {}: {e}",
                    block_info.height
                );
                0
            }
        };
        let total_fees = match fees_result {
            Ok(fees) => fees,
            Err(e) => {
                tracing::warn!(
                    "ZMQ: getblockstats failed for block {}: {e}",
                    block_info.height
                );
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

        // Bust tip-dependent RPC caches now that a new block has been fully
        // processed. Without this, /api/stats/live would keep returning the
        // previous block height until BLOCKCHAIN_INFO_TTL (30s) expired, AND
        // the reorg detector's block-hash LRU could hold pre-reorg hashes
        // for heights in the reorg window, masking real reorgs.
        state.rpc.invalidate_tip_caches(block_info.height);

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
//
// Classification logic, threshold constants, and the `ParsedTx` type now
// live in `super::notable`. This section owns only byte-level parsing:
// reading varints, SHA-256d for txids, and walking the segwit serialization
// to populate a `notable::ParsedTx`.

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
        // Detect OP_RETURN: 0x6a prefix, scan for readable ASCII.
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

    // For txid: we need the non-witness serialization (version + inputs + outputs + locktime)
    // Build it by stripping segwit marker/flag and witness data
    let mut total_witness_bytes = 0u64;
    let mut has_inscription = false;
    let txid = if is_segwit {
        // Non-witness serialization: version(4) + vin + vout + locktime(4)
        // We need to reconstruct this from the original data
        let mut stripped = Vec::with_capacity(data.len());
        stripped.extend_from_slice(&data[0..4]); // version

        // Copy from after segwit marker to start of witness data
        // The witness data starts after all outputs, which is at `cursor`
        stripped.extend_from_slice(&data[6..cursor]); // skip 4 (version) + 2 (marker+flag)

        // Skip witness data to find locktime, tracking total witness bytes
        // and scanning for the Ordinals inscription envelope marker.
        let mut wit_cursor = cursor;
        for _ in 0..input_count {
            let wit_count = read_varint(data, &mut wit_cursor)?;
            for _ in 0..wit_count {
                let item_len = read_varint(data, &mut wit_cursor)? as usize;
                if wit_cursor + item_len > data.len() {
                    return None;
                }
                total_witness_bytes += item_len as u64;
                // Scan this witness item for the Ordinals inscription envelope.
                if !has_inscription
                    && has_inscription_marker(
                        &data[wit_cursor..wit_cursor + item_len],
                    )
                {
                    has_inscription = true;
                }
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
        has_inscription,
        max_output_value,
        op_return_text,
    })
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

    // Classifier and extract_readable_text unit tests have moved to
    // `super::super::notable`. What remains here are parser tests
    // (inscription envelope detection in witness, OP_RETURN detection in
    // outputs) and a handful of real-world fixtures that exercise the full
    // parse → classify pipeline end to end.

    // --- INSCRIPTION ENVELOPE DETECTION IN PARSER ---

    #[test]
    fn test_inscription_envelope_in_witness() {
        // Build a minimal segwit tx with inscription envelope in witness
        let mut tx = Vec::new();
        tx.extend_from_slice(&1u32.to_le_bytes()); // version
        tx.push(0x00);
        tx.push(0x01); // segwit marker
        tx.push(0x01); // 1 input
        tx.extend_from_slice(&[0u8; 32]); // prev txid
        tx.extend_from_slice(&0u32.to_le_bytes()); // prev vout
        tx.push(0x00); // scriptSig length = 0
        tx.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // sequence
        tx.push(0x01); // 1 output
        tx.extend_from_slice(&50000u64.to_le_bytes()); // value
        tx.push(0x01);
        tx.push(0x51); // scriptPubKey: OP_TRUE
                       // Witness: 1 item containing inscription envelope
        tx.push(0x01); // 1 witness item
                       // Build a witness item with the envelope marker
        let mut witness_item = vec![0x00, 0x63, 0x03, 0x6f, 0x72, 0x64]; // OP_FALSE OP_IF OP_PUSH3 "ord"
        witness_item.extend_from_slice(&[0xAA; 200]); // fake inscription payload
        tx.push(witness_item.len() as u8); // item length
        tx.extend_from_slice(&witness_item);
        tx.extend_from_slice(&0u32.to_le_bytes()); // locktime

        let parsed = parse_raw_tx(&tx).expect("should parse");
        assert!(parsed.has_inscription);
        assert!(parsed.witness_bytes >= 200);
    }

    #[test]
    fn test_no_inscription_envelope_in_normal_witness() {
        // Normal segwit tx without inscription
        let mut tx = Vec::new();
        tx.extend_from_slice(&1u32.to_le_bytes());
        tx.push(0x00);
        tx.push(0x01);
        tx.push(0x01);
        tx.extend_from_slice(&[0u8; 32]);
        tx.extend_from_slice(&0u32.to_le_bytes());
        tx.push(0x00);
        tx.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        tx.push(0x01);
        tx.extend_from_slice(&50000u64.to_le_bytes());
        tx.push(0x01);
        tx.push(0x51);
        // Normal P2WPKH witness: 2 items (sig + pubkey)
        tx.push(0x02); // 2 witness items
        tx.push(0x48); // 72 byte signature
        tx.extend_from_slice(&[0xAA; 72]);
        tx.push(0x21); // 33 byte pubkey
        tx.extend_from_slice(&[0xBB; 33]);
        tx.extend_from_slice(&0u32.to_le_bytes());

        let parsed = parse_raw_tx(&tx).expect("should parse");
        assert!(!parsed.has_inscription);
    }

    // --- OP_RETURN DETECTION IN PARSER ---

    #[test]
    fn test_op_return_detected_in_output() {
        // Build a legacy tx with an OP_RETURN output containing readable text
        let mut tx = Vec::new();
        tx.extend_from_slice(&1u32.to_le_bytes()); // version
        tx.push(0x01); // 1 input
        tx.extend_from_slice(&[0u8; 32]); // prev txid
        tx.extend_from_slice(&0u32.to_le_bytes()); // prev vout
        tx.push(0x00); // scriptSig length = 0
        tx.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes()); // sequence
        tx.push(0x02); // 2 outputs
                       // Output 0: normal
        tx.extend_from_slice(&50000u64.to_le_bytes());
        tx.push(0x01);
        tx.push(0x51); // OP_TRUE
                       // Output 1: OP_RETURN with text
        tx.extend_from_slice(&0u64.to_le_bytes()); // 0 value
        let msg = b"Hello from Bitcoin blockchain test!";
        let script_len = 1 + 1 + msg.len(); // OP_RETURN + push + data
        tx.push(script_len as u8);
        tx.push(0x6a); // OP_RETURN
        tx.push(msg.len() as u8); // push N bytes
        tx.extend_from_slice(msg);
        tx.extend_from_slice(&0u32.to_le_bytes()); // locktime

        let parsed = parse_raw_tx(&tx).expect("should parse");
        assert!(parsed.op_return_text.is_some());
        assert!(parsed
            .op_return_text
            .unwrap()
            .contains("Hello from Bitcoin"));
    }

    #[test]
    fn test_op_return_binary_not_detected() {
        // OP_RETURN with binary data (Runes etc) should NOT be detected as text
        let mut tx = Vec::new();
        tx.extend_from_slice(&1u32.to_le_bytes());
        tx.push(0x01);
        tx.extend_from_slice(&[0u8; 32]);
        tx.extend_from_slice(&0u32.to_le_bytes());
        tx.push(0x00);
        tx.extend_from_slice(&0xFFFFFFFFu32.to_le_bytes());
        tx.push(0x01);
        tx.extend_from_slice(&0u64.to_le_bytes());
        // OP_RETURN with binary data
        let binary_data = [
            0x6a, 0x5d, 0x14, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0a,
        ];
        tx.push(binary_data.len() as u8);
        tx.extend_from_slice(&binary_data);
        tx.extend_from_slice(&0u32.to_le_bytes());

        let parsed = parse_raw_tx(&tx).expect("should parse");
        assert!(parsed.op_return_text.is_none());
    }

    // --- REAL-WORLD TX INTEGRATION TESTS ---
    // These use actual raw tx hex from mainnet to validate our parser
    // produces the correct fields for each detection type.

    #[test]
    fn test_real_whale_tx() {
        // txid: 6b70840bad4f5da8b0c61b00bf782cd85c241ebcb38e78b3c70249759c010dcf
        // 8 inputs, 15 outputs, 51.15 BTC total (whale at $100k)
        let hex = "01000000000108f7dfe87cefe0ef4dc78ffa71a38c5f335b9e9b647c315c9d666f328898da8df20000000000ffffffffdad6e8af7f34c918ffd363cafd83014bcba81e721220e2c53a712a7cf005bf210000000000ffffffff6790a5ef9cf354c4d182f90920ef525f141fd4f5696c7a4060806381d0f932040100000000ffffffffb000c9a69ef605410ea9a4c850a38c903f7c9d064cca3b5dd0a7d16fb10522650100000000ffffffff5ab53a567e842d4d805e04454deb2d939fccda9ece6f251679ae9b45fc6273890100000000ffffffffd6cbdf1b0f0500b5c96ed2e2f84e101890cae6f5ef8ab3d8f27ac58a9bcebcaa0100000000ffffffff755ea8572fc700e705991407429539050cd9da2cb0250ef9838735d73edbd9ed0100000000ffffffffa83ac3468118e07ad3989f73bb22d37eedcf1ddf56d8f2e8632bc976825afb800100000000ffffffff0f9ba907000000000017a914a6059d3e1bcb6e804e67c0929f27d38a5350d2c687820c040000000000220020467d42c580ee359538eaaac31c8e8e2caef8774a3881aec662df9e93c332dde351fc0000000000001976a914365e396e1b31a2960322fa0b6ae3e444a7177d7b88ac043d080000000000160014e7ee299d297e0fe41cee102a4ea7cf2026aae952330b020000000000160014c2d92562f55d1cb270168eff80637addd3d3a16da3ce000000000000160014d5214713f1d6b04a142f3e69d6806fc0b55c4d18ba4e580000000000160014b2077ce6dc7905fa42ed0db46e5bc2e08fccb4591410030000000000220020812d3cb2d0e102aa3f7c62b515c6475c8f2b7b487365f156394ff338e85452bb8f600f0000000000160014fa41e2ba60034ffb98ef604e5577ff417f34e9bc80aa0c000000000016001436084fdaf3c5be031bcc15859017f22b4a6eea7289040a00000000001600146e79d7c732a39af7581b77c227d327e8e3d0799e12480a0000000000160014d0646d3ccee8ce2e00c0b69f1ea990fa93fdfdc93fa51e0000000000160014e0b2b1f8e2bdbf6567fbb58592ac54ccf1f8cce4fbbe100000000000160014bf26026381c2db8842d260bda93ba7f03f3f1506671d16300100000016001424c8881db1970b80f3650e43bb96a0b53cb4ebc1024730440220432a347398c128459840d52bde8164f1146e84ee0272ec2a282b6746bb76982c02200e41d217591088e353e3efb01b5b29878868bcb12f748c0dfb7c5cba610092df012103dab9c0bd64a8b09cc45319c2af642ae06ca74a361b4dc086365a6b09b482796902473044022019a59a4ff37f2b1d45dd803ef86d1b54688546abb42455dace301fee9175b6d9022026ce767c92c59b2a4c83599cd4f8ecc496cc5755a812e8fa210e148eb3869026012102a5cbd14715c936dc917bcd0220235b5d6404d223c0c43b34d780614b533f222002483045022100cc21e7cb6077a8804a6c7e8ef60042c8376da2eb51504f4fd1b47899a412f1500220405f4917cdb29d5853b7f40f9dc81041e0dcf647ce20049ba95c29628ee2206d012102243c96ec52de618a10c13f5756026f877af427203c37df59183dd1be8ee4eaa102483045022100d6fe20f04c29dd2d015fb8c3faf13c2f9e32b7adfbd31d8ed1dd86590c8d93ec02203499f293568839e10249f0382a36cc69f7466bc6fe47186321d2e9d83d859372012102243c96ec52de618a10c13f5756026f877af427203c37df59183dd1be8ee4eaa10247304402207d564de4fa9463801f67443dcfd77960c10acb03a630b60d529baf1c6a4c934902200abd3bd55a028f2a2b59bdae6173538bcc78f686390785bb2893776d7157a70d012102243c96ec52de618a10c13f5756026f877af427203c37df59183dd1be8ee4eaa10247304402202b322a8634089a030f0dfe3beee4a4a133d310c7c74bff817bee510648b6f8650220687f5591f3089d5d39a7620df84726a8aef44f9ff67ba1c4f3fadeb2fbb2c1a0012102243c96ec52de618a10c13f5756026f877af427203c37df59183dd1be8ee4eaa102473044022028f409e29115981c5b9d371b13c3940c3c85abf1760bdf0fb41cbbacd6d4215e02206e0818e6ad24545f8308e8069e317748a0ec70e4d4b5378706f1b1be2f2ef0b2012102243c96ec52de618a10c13f5756026f877af427203c37df59183dd1be8ee4eaa102483045022100e8710468dbeaa2b040810387e8ec5b19436507bf34e48a7cf08f34a3bf83486c02207f7764215d5083cb95861399f68fcd00625e6d0913c9d36aae7d7514a1a38ecc01210399bb5e3f47f03e9b52e3a48b1aafce512863327a9d81b92410ba1cfccdab9e0700000000";
        let data = hex_decode(hex);
        let parsed = parse_raw_tx(&data).expect("should parse real whale tx");

        // Verify parser output matches known tx metadata
        assert_eq!(
            parsed.txid,
            "6b70840bad4f5da8b0c61b00bf782cd85c241ebcb38e78b3c70249759c010dcf"
        );
        assert_eq!(parsed.input_count, 8);
        assert_eq!(parsed.output_count, 15);
        assert!(!parsed.has_inscription);
        assert!(parsed.op_return_text.is_none());

        // 51.15543905 BTC = 5_115_543_905 sats
        assert_eq!(parsed.value, 5_115_543_905);

        // Classify: at $100k, 51.15 BTC = $5.1M -> whale
        assert_eq!(
            classify_notable(&parsed, 5000, 5.0, 100_000.0).primary_type(),
            Some("whale")
        );
        // At $10k, 51.15 BTC = $511k -> not whale
        assert_eq!(
            classify_notable(&parsed, 5000, 5.0, 10_000.0).primary_type(),
            None
        );
    }

    #[test]
    fn test_real_runestone_op_return_not_text() {
        // txid: 3d2b39abe41878bf59b9584afb8e50b20ae177190c7890c8175106099ca7d3a7
        // Runes protocol tx with OP_RETURN 6a5d... (binary Runestone payload).
        // Must NOT be detected as op_return_msg (it's binary protocol data).
        let hex = "0200000000010505f031c7adc676bad15cccf511106c2adf5318a2bba6e6ce4c3a2a39d4535fa30000000000ffffffff5430d727ff536c9292450d421e364e48531242a11d0f0f67a1d66595011565a60000000000ffffffff4376dc10d7e46523e323f569678f2d0245189e69427007f8429e8809c120ffa60100000000fffffffff493a80c3f0c62fcddaf0491a130ad1cbd3db4cd69da55fcc2cd03f49e60e4840200000000ffffffff89989ab646339aad624923851eb69f8ee9f3ca4fe82c23573118b8897d5d01420500000000ffffffff074a01000000000000225120e3552a2c24a4238a7344f655f04ceb0d14f381fbd9a4b90278d88257125444215827040000000000225120c01dcf308ab6e8e0791741beda33a700406a94621eb9a1ee22bc95f3ea7bc1e04a01000000000000225120707233b829840dfa85b440f8d8330e360e1ce32336eba4ff7c0c8b33ff5ea0d73f1306000000000016001457c4f4d8ac5f032d75a9cbf3d9f47537df01b87e4a01000000000000225120c72f0248f2f51b1a27474a748b49dabe2f29d3e2c55cca2d8939931f5b60bd1800000000000000001f6a5d1c00c0a23303fbf5998116000000b7b4f38a1d020000fdf2968dda01044281a80000000000225120f69d1fe8495729022a8777dc6c0572e6539e9c2e0e1adeddda52f8a10a5865c00140b06232f035168b376552877ea1b20ea42f275705417b04cf15aa09637982b91db8f2b9cda7506f78c52a87a9e74cf23cf16405d9668258f224ddb4768e13e344014073f83ee6885197b3a96570e0668d91776b5862c38f703058dc0e406c5f303f8e0a05925d669f4d4d2f6abd0b4542553c2545008b8ee5e07aa4b0f2cc730faeef01401949dd0b8afe961cd76f569075a1604baffec6d8bacb02363a4c66f67089fc9aa00e872cf0514ad25927c04baf182cf2c6b4ff863106521957f1a7fadc9404fc014072e481fe92f5df685bed7bf2ebdd6fdc660740a52eeabaaf4737a4860754bb7c72dba265bc7c3b163b03785f10c6042a7cad2a02644806caf3e302db987f99ac0141f9a1134a8b39c0c8c298eebf08006f3297518cb95e81bdd3c9dafc27e83ccd6e9ad1c9245905c19b072eed140ad12fca63db37aa5d13f8cdcb30fee879c88aa10100000000";
        let data = hex_decode(hex);
        let parsed = parse_raw_tx(&data).expect("should parse Runestone tx");

        assert_eq!(
            parsed.txid,
            "3d2b39abe41878bf59b9584afb8e50b20ae177190c7890c8175106099ca7d3a7"
        );
        assert_eq!(parsed.input_count, 5);
        assert_eq!(parsed.output_count, 7);
        // OP_RETURN payload is binary Runes data, not readable text
        assert!(
            parsed.op_return_text.is_none(),
            "Runestone binary should NOT be detected as text, got: {:?}",
            parsed.op_return_text
        );
        // Should not be flagged as notable at all
        assert_eq!(
            classify_notable(&parsed, 2_000, 3.0, 100_000.0).primary_type(),
            None
        );
    }

    #[test]
    fn test_real_small_inscription_not_flagged() {
        // txid: 6c3d7281f99cb3fd0b7c1a8efc5ca4e62a9e523fe81af7033419e600a15857b2
        // 1 in / 1 out, 217 bytes witness, HAS inscription envelope but too small.
        // This is a BRC-20 transfer (text/plain, 64 bytes payload).
        // Should NOT be flagged as large_inscription since witness < 100KB.
        let hex = "0200000000010102ff05fcf2669c140bca4fac2e7dc169ec45b52c95a6854ef1c1e0aaf7f565910000000000ffffffff014a01000000000000160014675ae0072f4515c9085cef2d3c5de690bb8707cb034003199cfc1b94e4077da9b4c5127709d36c03cd3c662e58b3297e857fdba867d9f3f8ef52508514688c6fb32f6e002e1279d3f4ad53164f1606c7df45bd90231b7820a063299cf5a7f3181b001db7b9e6f6ce65717bba7648ef9e8f4d9dba42694194ac0063036f726401010a746578742f706c61696e00407b2270223a226272632d3230222c226f70223a227472616e73666572222c227469636b223a22f09d9b91222c22616d74223a223134303030303030303030227d6821c1a063299cf5a7f3181b001db7b9e6f6ce65717bba7648ef9e8f4d9dba4269419400000000";
        let data = hex_decode(hex);
        let parsed =
            parse_raw_tx(&data).expect("should parse small inscription tx");

        assert_eq!(
            parsed.txid,
            "6c3d7281f99cb3fd0b7c1a8efc5ca4e62a9e523fe81af7033419e600a15857b2"
        );
        assert_eq!(parsed.input_count, 1);
        assert_eq!(parsed.output_count, 1);
        assert!(parsed.has_inscription); // envelope IS present
        assert!(parsed.witness_bytes < 100_000); // but witness is tiny

        // Should NOT be classified as notable (too small for large_inscription)
        assert_eq!(
            classify_notable(&parsed, 200, 1.5, 100_000.0).primary_type(),
            None
        );
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
            if state.process_with_time('R', t1 + Duration::from_millis(i * 50))
            {
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

