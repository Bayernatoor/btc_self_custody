//! ZMQ subscriber: connects to bitcoind's ZMQ interface for real-time
//! mempool transactions and block notifications.
//!
//! Subscribes to:
//! - `rawtx` on port 28333: new mempool transactions (decoded for fee/size/value)
//! - `hashblock` on port 28332: instant block detection
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
        timestamp: u64,
        tx_count: u64,
        total_fees: u64,
        size: u64,
        weight: u64,
        confirmed_count: u64,
    },
}

/// Spawn the ZMQ subscriber tasks. Two independent loops: one for txs, one for blocks.
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

    // Transaction subscriber
    {
        let state = Arc::clone(&state);
        let sender = tx_sender.clone();
        let url = zmq_tx_url.clone();
        let bt = Arc::clone(&block_txids);
        tokio::spawn(async move {
            loop {
                tracing::info!("ZMQ: connecting to rawtx at {url}");
                match subscribe_transactions(&state, &sender, &url, &bt).await {
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

/// Subscribe to raw transactions, decode them, look up fees, store in DB.
async fn subscribe_transactions(
    state: &Arc<StatsState>,
    sender: &broadcast::Sender<HeartbeatEvent>,
    url: &str,
    block_txids: &std::sync::Mutex<HashSet<String>>,
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
    block_txids: &std::sync::Mutex<HashSet<String>>,
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
        tracing::info!("ZMQ: new block {block_hash}");

        // Get block metadata and txid list
        let block_info = match get_block_info(state, &block_hash).await {
            Some(info) => info,
            None => continue,
        };

        // Populate the txid filter so the tx subscriber skips block txs
        // but still processes genuine new mempool txs
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

        // Confirm mempool txs in DB on a blocking thread so we don't
        // starve the async runtime during the SQLite write transaction
        let db = state.db.clone();
        let txids = block_info.txids.clone();
        let height = block_info.height;
        let (confirmed_count, total_fees) =
            tokio::task::spawn_blocking(move || {
                if let Ok(conn) = db.get() {
                    db::confirm_mempool_txs(&conn, &txids, height, now)
                        .unwrap_or((0, 0))
                } else {
                    (0, 0)
                }
            })
            .await
            .unwrap_or((0, 0));

        tracing::info!(
            "ZMQ: block {} ({block_hash}) — {confirmed_count}/{} txs confirmed, {:.4} BTC fees from mempool",
            block_info.height,
            block_info.txids.len(),
            total_fees as f64 / 100_000_000.0,
        );

        // Broadcast block event
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

        // Clear the txid filter — tx subscriber now processes everything
        if let Ok(mut set) = block_txids.lock() {
            set.clear();
        }
    }
}

/// Get block metadata and txid list from RPC, with retry for race condition.
/// ZMQ fires before the block is fully validated, so the RPC may not be ready yet.
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
}

/// Prune old mempool transactions (runs periodically).
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
