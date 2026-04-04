//! ZMQ subscriber: connects to bitcoind's ZMQ interface for real-time
//! mempool transactions and block notifications.
//!
//! Subscribes to:
//! - `rawtx` on port 28333: new mempool transactions (decoded for fee/size/value)
//! - `hashblock` on port 28332: instant block detection
//!
//! Transactions are stored in SQLite (`mempool_txs` table) and broadcast
//! to SSE clients via a tokio broadcast channel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::sync::broadcast;
use zeromq::{Socket, SocketRecv, SubSocket};

use super::api::StatsState;
use super::db;

/// Event broadcast to SSE clients.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "type")]
pub enum HeartbeatEvent {
    #[serde(rename = "tx")]
    Tx {
        txid: String,
        fee: u64,
        vsize: u32,
        value: u64,
        fee_rate: f64,
        timestamp: u64,
    },
    #[serde(rename = "block")]
    Block {
        height: u64,
        hash: String,
        confirmed_count: u64,
        unconfirmed_txids: Vec<String>,
    },
}

/// Spawn the ZMQ subscriber tasks. Two independent loops: one for txs, one for blocks.
pub fn spawn(
    state: Arc<StatsState>,
    tx_sender: broadcast::Sender<HeartbeatEvent>,
    zmq_tx_url: String,
    zmq_block_url: String,
) {
    // Shared flag: block subscriber sets this during block processing
    // so the tx subscriber can skip expensive RPC lookups for block txs
    let block_processing = Arc::new(AtomicBool::new(false));

    // Transaction subscriber
    {
        let state = Arc::clone(&state);
        let sender = tx_sender.clone();
        let url = zmq_tx_url.clone();
        let bp = Arc::clone(&block_processing);
        tokio::spawn(async move {
            loop {
                tracing::info!("ZMQ: connecting to rawtx at {url}");
                match subscribe_transactions(&state, &sender, &url, &bp).await {
                    Ok(()) => tracing::warn!(
                        "ZMQ rawtx stream ended, reconnecting..."
                    ),
                    Err(e) => tracing::error!(
                        "ZMQ rawtx error: {e}, reconnecting in 5s..."
                    ),
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });
    }

    // Block subscriber
    {
        let state = Arc::clone(&state);
        let sender = tx_sender;
        let url = zmq_block_url.clone();
        let bp = block_processing;
        tokio::spawn(async move {
            loop {
                tracing::info!("ZMQ: connecting to hashblock at {url}");
                match subscribe_blocks(&state, &sender, &url, &bp).await {
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

/// Subscribe to raw transactions, decode them, look up fees, store in DB.
async fn subscribe_transactions(
    state: &Arc<StatsState>,
    sender: &broadcast::Sender<HeartbeatEvent>,
    url: &str,
    block_processing: &AtomicBool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut socket = SubSocket::new();
    socket.connect(url).await?;
    socket.subscribe("rawtx").await?;
    tracing::info!("ZMQ: subscribed to rawtx");

    let mut tx_count = 0u64;
    let mut parse_fail = 0u64;
    let mut rpc_fail = 0u64;
    let mut consecutive_fail = 0u32;
    loop {
        let msg = socket.recv().await?;
        let frames: Vec<_> = msg.into_vec();
        if frames.len() < 2 {
            continue;
        }

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

        // Skip RPC lookups while a block is being processed — ZMQ sends rawtx
        // for every tx in the block, flooding us with ~5000 already-confirmed txs.
        // Two detection methods:
        // 1. block_processing flag (set by block subscriber after hashblock)
        // 2. Consecutive failure self-throttle (catches rawtx flood BEFORE hashblock)
        if block_processing.load(Ordering::Acquire) {
            consecutive_fail = 0;
            continue;
        }
        if consecutive_fail >= 5 {
            // Likely a block just arrived — skip and drain the ZMQ queue
            // Reset after a short pause to let the flood pass
            tokio::time::sleep(Duration::from_secs(3)).await;
            consecutive_fail = 0;
            tracing::debug!("ZMQ: skipped rawtx flood (consecutive failures)");
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

        // Store in DB
        if let Ok(conn) = state.db.get() {
            let _ = db::insert_mempool_tx(
                &conn,
                &parsed.txid,
                fee,
                vsize,
                parsed.value,
                now,
            );
        }

        // Broadcast to SSE clients
        let _ = sender.send(HeartbeatEvent::Tx {
            txid: parsed.txid,
            fee,
            vsize,
            value: parsed.value,
            fee_rate,
            timestamp: now,
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

/// Subscribe to block hashes, mark confirmed txs, broadcast block events.
async fn subscribe_blocks(
    state: &Arc<StatsState>,
    sender: &broadcast::Sender<HeartbeatEvent>,
    url: &str,
    block_processing: &AtomicBool,
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

        // Block hash is 32 bytes. Bitcoin Core ZMQ already sends in big-endian
        // (display) order, so we just convert to hex directly — no reversal needed.
        let hash_bytes = &frames[1];
        if hash_bytes.len() != 32 {
            tracing::warn!(
                "ZMQ: unexpected hashblock size: {}",
                hash_bytes.len()
            );
            continue;
        }
        let block_hash = bytes_to_hex(hash_bytes);
        tracing::info!("ZMQ: new block {block_hash}");

        // Signal tx subscriber to skip RPC lookups (block txs flood rawtx)
        block_processing.store(true, Ordering::Release);

        // Get block height and txid list
        let (height, txids) = match get_block_info(state, &block_hash).await {
            Some(info) => info,
            None => {
                block_processing.store(false, Ordering::Release);
                continue;
            }
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Mark confirmed transactions in DB and get surviving (unconfirmed) txids
        let (confirmed_count, unconfirmed_txids) =
            if let Ok(conn) = state.db.get() {
                let count = db::confirm_mempool_txs(&conn, &txids, height, now)
                    .unwrap_or(0);
                // Query txids that are still unconfirmed (survived this block)
                let survivors = db::query_unconfirmed_txids(&conn, 5000)
                    .unwrap_or_default();
                (count, survivors)
            } else {
                (0, Vec::new())
            };

        tracing::info!(
            "ZMQ: block {height} ({block_hash}) — {confirmed_count}/{} txs confirmed in our mempool",
            txids.len()
        );

        // Broadcast block event with surviving txids
        let _ = sender.send(HeartbeatEvent::Block {
            height,
            hash: block_hash,
            confirmed_count,
            unconfirmed_txids,
        });

        // Resume tx processing after a short delay (let the rawtx flood from
        // the block pass through the ZMQ socket before we start RPC lookups again)
        tokio::time::sleep(Duration::from_secs(3)).await;
        block_processing.store(false, Ordering::Release);
    }
}

/// Get block height and txid list from RPC, with retry for race condition.
/// ZMQ fires before the block is fully validated, so the RPC may not be ready yet.
async fn get_block_info(
    state: &Arc<StatsState>,
    hash: &str,
) -> Option<(u64, Vec<String>)> {
    for attempt in 0..3 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        match state.rpc.get_block_txids(hash).await {
            Ok((height, txids)) => {
                return Some((height, txids));
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
    value: u64, // sum of output values in sats
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

    // Parse outputs for value
    let mut total_value = 0u64;
    for _ in 0..output_count {
        let value = read_u64_le(data, &mut cursor)?;
        total_value += value;
        let script_len = read_varint(data, &mut cursor)? as usize;
        cursor += script_len; // scriptPubKey
        if cursor > data.len() {
            return None;
        }
    }

    // For txid: we need the non-witness serialization (version + inputs + outputs + locktime)
    // Build it by stripping segwit marker/flag and witness data
    let txid = if is_segwit {
        // Non-witness serialization: version(4) + vin + vout + locktime(4)
        // We need to reconstruct this from the original data
        let mut stripped = Vec::with_capacity(data.len());
        stripped.extend_from_slice(&data[0..4]); // version

        // Copy from after segwit marker to start of witness data
        // The witness data starts after all outputs, which is at `cursor`
        stripped.extend_from_slice(&data[6..cursor]); // skip 4 (version) + 2 (marker+flag)

        // Skip witness data to find locktime
        let mut wit_cursor = cursor;
        for _ in 0..input_count {
            let wit_count = read_varint(data, &mut wit_cursor)?;
            for _ in 0..wit_count {
                let item_len = read_varint(data, &mut wit_cursor)? as usize;
                if wit_cursor + item_len > data.len() {
                    return None;
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

/// Prune old mempool transactions (runs periodically).
pub async fn prune_old_txs(state: &Arc<StatsState>) {
    let seven_days_ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(7 * 24 * 3600);

    if let Ok(conn) = state.db.get() {
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
}
