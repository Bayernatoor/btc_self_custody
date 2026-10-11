//! Bitcoin Core JSON-RPC client.
//!
//! Thin wrapper over reqwest that handles HTTP Basic authentication and the
//! JSON-RPC 1.0 protocol used by Bitcoin Core. The primary data extraction
//! method is [`BitcoinRpc::get_block`] which fetches a block at verbosity=2
//! (full transaction data) and computes all metrics in a single pass:
//!
//! - Block metadata (size, weight, difficulty, version)
//! - Fee statistics (total fees, median fee, median/p10/p90 fee rates in sat/vB)
//! - OP_RETURN classification (Runes, Omni, Counterparty, generic data carriers)
//! - Output script type counts (P2PKH, P2SH, P2WPKH, P2WSH, P2TR, multisig, etc.)
//! - Miner identification (from coinbase scriptSig and OP_RETURN outputs)
//! - BIP-54 signaling (coinbase nLockTime and nSequence)
//! - Taproot key-path vs script-path spend detection
//! - Ordinals inscription and BRC-20 detection in witness data
//! - Stamps protocol detection (bare multisig with fake pubkeys)
//!
//! Also provides methods for mempool queries, network stats, UTXO set info,
//! and price data from external APIs.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use super::classifier::{self, OpReturnType};
use super::error::StatsError;
use super::rpc_cache::{CachedSlot, LruSlot, LruStats, SlotStats};

/// Calculate the encoded size of a Bitcoin CompactSize (varint) for a given value.
/// Used to accurately count witness byte overhead.
fn varint_size(n: u64) -> u64 {
    if n < 0xFD {
        1
    } else if n <= 0xFFFF {
        3
    } else if n <= 0xFFFF_FFFF {
        5
    } else {
        9
    }
}

/// Core's `IsUnspendable`: a script starting with `OP_RETURN`, or one larger
/// than `MAX_SCRIPT_SIZE`. Such an output never enters the UTXO set.
///
/// Not the same as the `nulldata` Solver label, which additionally requires
/// the remainder to be push-only. Block 470,000's coinbase holds an
/// `OP_RETURN "RSKBLOCK:"` that fails that extra test and is `nonstandard`,
/// yet is still unspendable.
fn is_unspendable(hex: &str) -> bool {
    hex.starts_with("6a") || hex.len() > 20_000
}

/// Core's `CPubKey::ValidSize`: the header byte must agree with the length.
///
/// 33 bytes means a compressed key and the header must be 0x02 or 0x03.
/// 65 bytes means an uncompressed key and the header must be 0x04, 0x06 or
/// 0x07. Anything else is not a key as far as Core is concerned, however well
/// it fits the surrounding script.
///
/// This is the check `rust-bitcoin`'s `is_p2pk` does not do, and the omission
/// was not academic: the 2012-era outputs that embedded English text where a
/// public key belongs match the *shape* of P2PK exactly. Height 181,732 has
/// three in one transaction, which Core calls `nonstandard` and a shape-only
/// classifier calls `pubkey`.
fn valid_pubkey_size(k: &[u8]) -> bool {
    match k.first() {
        Some(0x02 | 0x03) => k.len() == 33,
        Some(0x04 | 0x06 | 0x07) => k.len() == 65,
        _ => false,
    }
}

/// Core's `MatchPayToPubkey`: `<push 33|65> <key> OP_CHECKSIG`, with the key
/// passing `ValidSize`.
fn is_core_p2pk(s: &bitcoin::Script) -> bool {
    use bitcoin::opcodes::all::OP_CHECKSIG;
    let b = s.as_bytes();
    for len in [65usize, 33] {
        if b.len() == len + 2
            && b[0] as usize == len
            && b[len + 1] == OP_CHECKSIG.to_u8()
            && valid_pubkey_size(&b[1..=len])
        {
            return true;
        }
    }
    false
}

/// Core's `TxoutType::NULL_DATA`: `OP_RETURN` followed by a **push-only**
/// remainder.
///
/// `rust-bitcoin`'s `is_op_return` tests the first byte alone, so it accepts
/// scripts Core rejects: `6a76` is `OP_RETURN OP_DUP`, and `6a02ff` is a
/// truncated push. Core labels both `nonstandard`, which matters because they
/// would otherwise be counted and classified as embedded data.
fn is_core_nulldata(s: &bitcoin::Script) -> bool {
    use bitcoin::opcodes::all::OP_RETURN;
    use bitcoin::script::Instruction;
    let b = s.as_bytes();
    if b.first() != Some(&OP_RETURN.to_u8()) {
        return false;
    }
    let rest = bitcoin::Script::from_bytes(&b[1..]);
    // `IsPushOnly`: every opcode must parse and be no greater than OP_16. A
    // parse failure is itself a rejection, which is what makes a truncated
    // push non-standard.
    for ins in rest.instructions() {
        match ins {
            Ok(Instruction::PushBytes(_)) => {}
            Ok(Instruction::Op(op)) if op.to_u8() <= 0x60 => {}
            _ => return false,
        }
    }
    true
}

/// Core's `GetScriptNumber` plus `CheckMinimalPush`, taking the opcode that
/// actually encoded the number rather than only its value.
///
/// Takes `(offset, instruction)` because `instructions()` alone collapses a
/// direct push and a `PUSHDATA1` of the same bytes into one `PushBytes`, and
/// Core rejects the second. A value-only version of this rule still called
/// `OP_1 <17 keys> PUSHDATA1(0x11) OP_CHECKMULTISIG` a multisig where Core
/// says `nonstandard`. The script byte at `offset` is the opcode, which
/// closes that.
fn script_number_at(
    script: &bitcoin::Script,
    offset: usize,
    ins: &bitcoin::script::Instruction,
) -> Option<i64> {
    use bitcoin::script::Instruction;
    match ins {
        Instruction::Op(op) => {
            let n = op.to_u8();
            match n {
                0x00 => Some(0),
                0x51..=0x60 => Some(i64::from(n - 0x50)),
                _ => None,
            }
        }
        Instruction::PushBytes(b) => {
            let d = b.as_bytes();
            let opcode = *script.as_bytes().get(offset)?;
            if d.is_empty() {
                return None;
            }
            // A value a small-integer opcode could express must use it.
            if d.len() == 1 && (d[0] <= 16 || d[0] == 0x81) {
                return None;
            }
            // Up to 75 bytes Core requires a direct push, so the opcode is
            // the length. This is what rejects the PUSHDATA1 forms.
            if opcode as usize != d.len() || d.len() > 4 {
                return None;
            }
            if d[d.len() - 1] & 0x7f == 0 {
                return None;
            }
            let mut v: i64 = 0;
            for (i, byte) in d.iter().enumerate() {
                v |= i64::from(*byte) << (8 * i);
            }
            if d[d.len() - 1] & 0x80 != 0 {
                v &= !(0x80i64 << (8 * (d.len() - 1)));
                v = -v;
            }
            Some(v)
        }
    }
}

/// Core's `MatchMultisig`: `<m> <key>..<key> <n> OP_CHECKMULTISIG`, every key
/// passing `ValidSize`, `n` matching the key count, `m <= n`, and at most
/// `MAX_PUBKEYS_PER_MULTISIG` keys.
///
/// Bare multisig is the encoding the Stamps protocol abuses, so
/// `multisig_count` feeds the embedded-data charts rather than being a
/// curiosity. An independent review found the previous implementation both
/// too strict, rejecting the 1-of-17 that Core accepts, and too lax, taking
/// any 33- or 65-byte push as a key.
fn is_bare_multisig(s: &bitcoin::Script) -> bool {
    use bitcoin::opcodes::all::OP_CHECKMULTISIG;
    use bitcoin::script::Instruction;

    const MAX_PUBKEYS_PER_MULTISIG: i64 = 20;

    let pairs: Vec<(usize, Instruction)> =
        match s.instruction_indices().collect() {
            Ok(v) => v,
            Err(_) => return false,
        };
    if pairs.len() < 4 {
        return false;
    }
    let ins: Vec<Instruction> = pairs.iter().map(|(_, i)| *i).collect();
    if !matches!(ins.last(), Some(Instruction::Op(op)) if *op == OP_CHECKMULTISIG)
    {
        return false;
    }
    let last_n = pairs.len() - 2;
    let (Some(m), Some(n)) = (
        script_number_at(s, pairs[0].0, &pairs[0].1),
        script_number_at(s, pairs[last_n].0, &pairs[last_n].1),
    ) else {
        return false;
    };
    let keys = &ins[1..ins.len() - 2];
    if n != keys.len() as i64 || m < 1 || m > n || n > MAX_PUBKEYS_PER_MULTISIG
    {
        return false;
    }
    keys.iter().all(|i| {
        matches!(i, Instruction::PushBytes(b) if valid_pubkey_size(b.as_bytes()))
    })
}

/// Core's `TxoutType::ANCHOR`: witness v1 with the exact two-byte program
/// `0x4e73`, standardised in Core 28 as the ephemeral anchor and used by
/// Lightning for CPFP fee bumping.
///
/// `rust-bitcoin`'s `is_p2tr` requires a 32-byte program, so this does not
/// collide with Taproot; matched on the whole script because the pattern is
/// four bytes and exact.
fn is_p2a(s: &bitcoin::Script) -> bool {
    s.as_bytes() == [0x51, 0x02, 0x4e, 0x73]
}

/// The string Core would put in `scriptPubKey.type`.
///
/// Only the labels `parse_block_value` actually branches on need to be exact.
/// Everything else falls through its `_` arm into `unknown_script_count`, and
/// Core's `nonstandard`, `witness_unknown` and `anchor` all land there too, so
/// returning `"other"` for them is identical in effect rather than merely
/// close.
fn script_type(s: &bitcoin::Script) -> &'static str {
    if is_bare_multisig(s) {
        "multisig"
    } else if is_p2a(s) {
        "anchor"
    } else if is_core_nulldata(s) {
        "nulldata"
    } else if is_core_p2pk(s) {
        "pubkey"
    } else if s.is_p2pkh() {
        "pubkeyhash"
    } else if s.is_p2sh() {
        "scripthash"
    } else if s.is_p2wpkh() {
        "witness_v0_keyhash"
    } else if s.is_p2wsh() {
        "witness_v0_scripthash"
    } else if s.is_p2tr() {
        "witness_v1_taproot"
    } else {
        "other"
    }
}

/// Core's `asm` for a bare multisig script, which is the only script whose
/// `asm` anything reads: the Stamps detector splits it on whitespace looking
/// for 33-byte pushes that are not real compressed pubkeys. Emitted in Core's
/// shape, `M <key> <key> .. N OP_CHECKMULTISIG`, so that detector behaves
/// identically rather than merely seeing a plausible string.
fn multisig_asm(s: &bitcoin::Script) -> String {
    use bitcoin::script::Instruction;
    let mut parts: Vec<String> = Vec::new();
    for ins in s.instructions().flatten() {
        match ins {
            Instruction::PushBytes(b) => {
                parts.push(
                    b.as_bytes()
                        .iter()
                        .map(|x| format!("{x:02x}"))
                        .collect::<String>(),
                );
            }
            Instruction::Op(op) => {
                let n = op.to_u8();
                if (0x51..=0x60).contains(&n) {
                    parts.push((n - 0x50).to_string());
                } else {
                    parts.push(format!("{op:?}"));
                }
            }
        }
    }
    parts.join(" ")
}

/// Bitcoin Core's `GetDifficulty`, reproduced exactly.
///
/// `rust-bitcoin`'s `difficulty_float` differs from Core in the last few ULP,
/// which showed up as 14 of 28 blocks disagreeing in the raw-parity test at a
/// relative error of about 3e-16. That is far too small to matter for any
/// chart, and `difficulty` is one of the columns no sweep writes, so a
/// tolerance would have been defensible. This is ten lines and lets the test
/// assert exact equality on every column instead, which is worth more than
/// the tolerance would have saved.
fn core_difficulty(bits: u32) -> f64 {
    let mut shift = (bits >> 24) & 0xff;
    let mut diff = f64::from(0x0000ffffu32) / f64::from(bits & 0x00ff_ffff);
    while shift < 29 {
        diff *= 256.0;
        shift += 1;
    }
    while shift > 29 {
        diff /= 256.0;
        shift -= 1;
    }
    diff
}

/// Build the subset of a verbosity-2 `getblock` response that
/// `parse_block_value` reads, from a consensus-decoded block.
///
/// **Synthesising the old input rather than porting the parser is the whole
/// point.** `parse_block_value` is 450 lines of classification that took a
/// long time to get right; a second implementation of it reading bytes would
/// be two code paths that must agree about what 51 columns mean forever. The
/// OP_RETURN two-rule defect is exactly what that costs when they drift. So
/// the only new surface is this function, and the differential test has one
/// thing to check rather than fifty.
///
/// `height` is passed in because a block does not carry its own height except
/// via BIP34, which pre-dates block 227,836.
fn synthesize_block_value(block: &bitcoin::Block, height: u64) -> Value {
    let txs: Vec<Value> = block
        .txdata
        .iter()
        .enumerate()
        .map(|(i, tx)| {
            let vin: Vec<Value> = tx
                .input
                .iter()
                .map(|txin| {
                    let witness: Vec<Value> = txin
                        .witness
                        .iter()
                        .map(|w| {
                            json!(w
                                .iter()
                                .map(|x| format!("{x:02x}"))
                                .collect::<String>())
                        })
                        .collect();
                    let mut v = json!({ "sequence": txin.sequence.0 });
                    // Core omits `txinwitness` entirely on a non-witness
                    // input, and `parse_block_value` keys witness detection
                    // off the field being *present* rather than non-empty.
                    // Emitting an empty array made every transaction look
                    // like a SegWit spend: legacy_tx_count went to zero and
                    // witness_bytes gained the 2-byte marker for every tx in
                    // the chain.
                    if !witness.is_empty() {
                        v["txinwitness"] = json!(witness);
                    }
                    // Core marks the coinbase input with a `coinbase` field
                    // holding the scriptSig hex, in place of txid/vout.
                    if i == 0 {
                        v["coinbase"] = json!(txin
                            .script_sig
                            .as_bytes()
                            .iter()
                            .map(|x| format!("{x:02x}"))
                            .collect::<String>());
                    }
                    v
                })
                .collect();

            let vout: Vec<Value> = tx
                .output
                .iter()
                .map(|o| {
                    let ty = script_type(&o.script_pubkey);
                    let hex: String = o
                        .script_pubkey
                        .as_bytes()
                        .iter()
                        .map(|x| format!("{x:02x}"))
                        .collect();
                    let mut spk = json!({ "type": ty, "hex": hex });
                    if ty == "multisig" {
                        spk["asm"] = json!(multisig_asm(&o.script_pubkey));
                    }
                    json!({
                        // Core reports value in BTC, and the parser converts
                        // back to sats. Kept in BTC so the arithmetic is the
                        // same one, rounding included.
                        "value": o.value.to_sat() as f64 / 100_000_000.0,
                        "scriptPubKey": spk,
                    })
                })
                .collect();

            json!({
                "version": tx.version.0,
                "size": tx.total_size(),
                "vsize": tx.vsize(),
                "locktime": tx.lock_time.to_consensus_u32(),
                "vin": vin,
                "vout": vout,
            })
        })
        .collect();

    json!({
        "hash": block.block_hash().to_string(),
        "height": height,
        "time": block.header.time,
        "nTx": block.txdata.len(),
        "size": block.total_size(),
        "weight": block.weight().to_wu(),
        "difficulty": core_difficulty(block.header.bits.to_consensus()),
        "version": block.header.version.to_consensus(),
        "tx": txs,
    })
}

/// JSON-RPC response envelope for [`BitcoinRpc::call_typed`]. `result` is
/// deserialized straight into the caller's `T`; `result` is `None` on an error
/// response (Core sets it null), which is why it's `Option`.
#[derive(serde::Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<Value>,
}

/// Minimal subset of a `getrawmempool true` entry, only the two fields the
/// heartbeat backfill needs. serde skips the ~dozen other fields (wtxid, weight,
/// time, ancestor/descendant accounting, depends, spentby, flags) without
/// allocating them, so deserializing into this via [`BitcoinRpc::call_typed`]
/// costs a fraction of the full Value tree.
#[derive(serde::Deserialize)]
struct RawMempoolEntry {
    vsize: u32,
    fees: RawMempoolFees,
}

#[derive(serde::Deserialize)]
struct RawMempoolFees {
    base: f64,
}

/// Bitcoin Core JSON-RPC client. Holds a persistent HTTP client with
/// connection pooling, configurable timeouts, and per-method response caches
/// (see [`rpc_cache`](super::rpc_cache)) that protect Core from request floods
/// during `cs_main` stalls at block validation time.
pub struct BitcoinRpc {
    client: Client,
    url: String,
    user: String,
    password: String,
    // Cache slots for the hot read-only RPCs polled by the dashboard.
    // Each slot holds the last successful response, gates concurrent fetches
    // via singleflight, and falls back to stale-on-error when upstream fails.
    mempool_info_cache: Arc<CachedSlot<MempoolInfo>>,
    blockchain_info_cache: Arc<CachedSlot<BlockchainInfo>>,
    network_hashps_cache: Arc<CachedSlot<f64>>,
    // estimate_smart_fee is parameterized by target blocks, so one slot is
    // held per observed target. Lazily created on first call per target.
    smart_fee_caches: Arc<Mutex<HashMap<u64, Arc<CachedSlot<f64>>>>>,
    // LRU caches for immutable block data. Blocks at confirmed heights never
    // change their content, so they can stay cached indefinitely, only
    // capacity pressure evicts. On reorg detection, specific entries are invalidated
    // explicitly by the reorg handler.
    block_hash_cache: Arc<LruSlot<u64, String>>,
    block_data_cache: Arc<LruSlot<String, Block>>,
}

// Per-method TTLs. Tuned to balance data freshness against load reduction.
// See src/stats/rpc_cache.rs for the rationale on why "short" TTLs still
// produce high hit rates under real request volume.

// 13s: spans two full 6-second poll intervals, producing a cached
// miss-hit-hit pattern (67% hit rate single-tab, higher under load).
// Mempool tx count isn't critical data like the chain tip; users who
// want up-to-the-second values can click the manual Refresh. The
// real-time heartbeat SSE continues to stream per-tx arrivals with
// no delay regardless of this TTL.
const MEMPOOL_INFO_TTL: Duration = Duration::from_secs(13);

// 30s: `getblockchaininfo` returns data that only changes when a new
// block arrives. The ZMQ `hashblock` handler in zmq_subscriber::subscribe_blocks
// calls `BitcoinRpc::invalidate_tip_caches()` on every new block so the next
// poll picks up the fresh tip. 30s is a safety backstop for the rare case
// where a ZMQ message is dropped (network blip, subscriber restart), worst
// case the user sees tip height stale for 30s until the backstop kicks in.
const BLOCKCHAIN_INFO_TTL: Duration = Duration::from_secs(30);

const NETWORK_HASHPS_TTL: Duration = Duration::from_secs(60);
const SMART_FEE_TTL: Duration = Duration::from_secs(10);

// Depth (in blocks) over which the reorg detector in ingest::verify_recent_blocks
// compares stored hashes against canonical. Kept in sync with the `depth`
// argument passed from startup.rs. invalidate_tip_caches below uses this to
// clear block_hash_cache entries in the same window on every hashblock event,
// so the next reorg sweep sees fresh canonical data, without this, a reorg
// against a still-cached stale (height → hash) pair would be silently missed.
pub const REORG_DETECTION_DEPTH: u64 = 6;

// Bounded LRU capacities for immutable block data. Block hashes are tiny
// (~80 bytes each: HashMap overhead + u64 key + heap-allocated hex string),
// so 10k fits in well under 1 MB. Parsed Block structs are heavier,
// ~40 scalar fields plus a few heap strings (hash, miner, coinbase_text),
// typically 500 bytes to 2 KB depending on coinbase_text length. 500 entries
// caps worst-case at ~1 MB. These serve the block detail modal, Lookout tx
// lookups, and any Observatory path that navigates to a specific confirmed
// block. If a new feature causes heavy block-browsing traffic, bump the
// data capacity before the hash capacity: each Block use touches both but
// block_data is the more expensive one to miss.
const BLOCK_HASH_CACHE_CAPACITY: usize = 10_000;
const BLOCK_DATA_CACHE_CAPACITY: usize = 500;

/// Response from `getblockchaininfo` RPC.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct BlockchainInfo {
    pub blocks: u64,
    pub chain: String,
    pub difficulty: f64,
    #[serde(rename = "verificationprogress")]
    pub verification_progress: f64,
    pub size_on_disk: u64,
    pub bestblockhash: String,
    #[serde(default)]
    pub time: u64,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct MempoolInfo {
    pub size: u64,
    pub bytes: u64,
    pub usage: u64,
    #[serde(default)]
    pub total_fee: f64,
    pub maxmempool: u64,
    pub mempoolminfee: f64,
}

/// Response from `gettxoutsetinfo` RPC. Used to get the total UTXO count.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TxoutSetInfo {
    /// Total number of unspent transaction outputs.
    pub txouts: u64,
    /// Total value of all UTXOs in BTC.
    pub total_amount: f64,
}

/// BTC/USD price from mempool.space `/api/v1/prices` endpoint.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct PriceInfo {
    #[serde(rename = "USD")]
    pub usd: f64,
    pub time: u64,
}

/// Fee and size data for a single mempool transaction from `getmempoolentry` RPC.
#[derive(Debug)]
pub struct MempoolEntryInfo {
    /// Transaction fee in satoshis.
    pub fee: u64,
    /// Virtual size in vbytes.
    pub vsize: u32,
}

/// Block metadata and transaction ID list from `getblock` verbosity=1.
/// Used by ZMQ block handler for fast block processing without full tx data.
#[derive(Debug)]
pub struct BlockTxids {
    pub height: u64,
    pub timestamp: u64,
    /// Block size in bytes (0 when fetched via getblockheader).
    pub size: u64,
    /// Block weight in weight units (0 when fetched via getblockheader).
    pub weight: u64,
    pub tx_count: u64,
    /// All transaction IDs in the block (empty when fetched via getblockheader).
    pub txids: Vec<String>,
}

/// Fully parsed block data from `getblock` verbosity=2.
/// Contains all metrics computed in a single pass over the block's transactions.
///
/// `Default` exists so tests can build a block by naming only the fields under
/// test; every field is a number or a String, so the derived zero value is
/// meaningful rather than a placeholder for something unrepresentable.
///
/// `Serialize` is here for `raw_parity::raw_matches_json`, which compares two
/// `Block`s field by field through `serde_json`. Doing it reflectively rather
/// than by naming 55 fields means a column added later is compared without
/// anyone remembering to add it.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Block {
    pub hash: String,
    pub height: u64,
    pub time: u64,
    pub n_tx: u64,
    pub size: u64,
    pub weight: u64,
    pub difficulty: f64,
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
    pub version: u32,
    pub total_fees: u64,
    pub median_fee: u64,
    pub median_fee_rate: f64,
    pub coinbase_locktime: u64,
    pub coinbase_sequence: u64,
    pub miner: String,
    pub segwit_spend_count: u64,
    /// Taproot v1 witness outputs created (not spends). Named _spend_ for historical reasons.
    pub taproot_spend_count: u64,
    /// Pay-to-anchor outputs (`OP_1 PUSH2 0x4e73`), the ephemeral anchor
    /// standardised in Core 28 and used by Lightning for CPFP fee bumping.
    /// Core labels these `anchor`.
    ///
    /// **A subset of `unknown_script_count`, not a sibling.** Anchors are
    /// still counted there too, so that the eight-type denominator used
    /// across the chart builders does not shrink by ~7% and inflate every
    /// other share. Promoting them to a ninth classified type is an open
    /// decision; see the parser for the full reasoning.
    pub p2a_count: u64,
    /// Largest single OP_RETURN script in the block, in bytes.
    ///
    /// `op_return_bytes` is a total and so cannot separate "more OP_RETURNs"
    /// from "bigger OP_RETURNs". This separates them, which is what makes the
    /// effect of a datacarrier policy change visible at all.
    ///
    /// **It does not prove v30 was used, and an earlier version of this
    /// comment claimed it did.** 83 bytes was the *standardness* ceiling, not
    /// a consensus one, so oversized OP_RETURNs predate v30: height 244,591
    /// carries a 137-byte `nulldata`. What this measures is a threshold being
    /// crossed, which is a weaker and truer claim.
    ///
    /// Non-coinbase only, like every other counter in this loop.
    pub op_return_max_bytes: u64,
    /// OP_RETURN outputs whose **script** exceeds 83 bytes.
    ///
    /// Script bytes, not payload bytes, and the distinction is not pedantic:
    /// an 80-byte payload pushed with PUSHDATA2 occupies 84 script bytes and
    /// trips this counter. 83 is `OP_RETURN` + a one-byte push + 80 payload,
    /// and `>` rather than `>=` matches Core's `IsStandard`, which compares
    /// the whole script size against the limit with `>`.
    pub op_return_over_80_count: u64,
    /// Non-coinbase transactions by version. v3 is TRUC.
    pub tx_v1_count: u64,
    pub tx_v2_count: u64,
    pub tx_v3_count: u64,
    /// Satoshis created into output types whose public key is exposed by
    /// construction, for the post-quantum section. Creation, not holdings:
    /// current exposed supply needs a UTXO snapshot.
    pub p2pk_value: u64,
    pub p2tr_value: u64,
    pub multisig_value: u64,
    /// Every output the coinbase transaction creates, OP_RETURN included.
    ///
    /// `output_count` is now every output in the block, which is
    /// structurally true but is not what every consumer wants. Batching
    /// measures outputs per *user* transaction, so it needs
    /// `output_count - coinbase_output_count`. Without this column that
    /// subtraction is unrecoverable after a sweep, because nothing else
    /// stored distinguishes the coinbase's share.
    pub coinbase_output_count: u64,
    /// Outputs that can never be spent, by Core's `IsUnspendable`: the
    /// script begins with `OP_RETURN`, or exceeds `MAX_SCRIPT_SIZE`.
    ///
    /// Counted over **every** output, coinbase included, and deliberately
    /// not derived from the `nulldata` label. Those are different sets, and
    /// the difference is real chain data: block 470,000's coinbase carries
    /// `OP_RETURN "RSKBLOCK:"`, which is not push-only, so Core's Solver
    /// calls it `nonstandard` while `IsUnspendable` still excludes it from
    /// the UTXO set. Counting by label gave -4,297 there against Core's
    /// -4,298.
    ///
    /// UTXO growth is `output_count - unspendable_output_count -
    /// input_count`. Keeping this separate from `op_return_count` is what
    /// lets that stay true: `op_return_count` answers a question about user
    /// data and carries policy decisions, while this answers a question
    /// about the UTXO set and carries none.
    pub unspendable_output_count: u64,
    /// Bytes of the witness **items** that matched the BRC-20 marker.
    ///
    /// Not the witness of the matching transaction, which is what an earlier
    /// version of this comment said. The unit matters because `brc20_bytes`
    /// cannot be subtracted from `inscription_bytes`: that field holds an
    /// estimated payload after envelope overhead, while this holds whole
    /// matching items, so the residual can go negative. CQ-19's proposed
    /// volume split needs a consistent basis and does not yet have one.
    ///
    /// Two envelopes inside one item count once, and an item mixing BRC-20
    /// with unrelated data counts in full; a substring test cannot partition
    /// either case.
    pub brc20_bytes: u64,
    // Output script type counts
    pub p2pk_count: u64,
    pub p2pkh_count: u64,
    pub p2sh_count: u64,
    pub p2wpkh_count: u64,
    pub p2wsh_count: u64,
    pub p2tr_count: u64,
    pub multisig_count: u64,
    pub unknown_script_count: u64,
    // Taproot spend type counts (inputs spending P2TR outputs)
    pub taproot_keypath_count: u64,
    pub taproot_scriptpath_count: u64,
    // Transaction-level metrics
    pub input_count: u64,
    pub output_count: u64,
    pub rbf_count: u64,
    // Size breakdown
    pub witness_bytes: u64,
    // Ordinals inscriptions
    pub inscription_count: u64,
    pub inscription_bytes: u64, // payload only (envelope overhead subtracted)
    pub inscription_envelope_bytes: u64, // full witness item bytes including envelope
    pub brc20_count: u64,
    // Total value of non-coinbase outputs (satoshis)
    pub total_output_value: u64,
    // Total value of non-coinbase inputs (satoshis)
    pub total_input_value: u64,
    // Fee rate percentiles (sat/vB)
    pub fee_rate_p10: f64,
    pub fee_rate_p90: f64,
    // Stamps protocol (bare multisig encoding with specific pattern)
    pub stamps_count: u64,
    // Largest transaction in the block (bytes)
    pub largest_tx_size: u64,
    // --- Backfill v10 fields ---
    /// Largest individual transaction fee in satoshis.
    pub max_tx_fee: u64,
    /// Total fees from transactions containing Ordinals inscriptions (sats).
    pub inscription_fees: u64,
    /// Total fees from transactions containing Runes protocol outputs (sats).
    pub runes_fees: u64,
    /// Number of transactions with only legacy (non-witness) inputs.
    pub legacy_tx_count: u64,
    /// Number of transactions with any SegWit v0 input (but no Taproot).
    pub segwit_tx_count: u64,
    /// Number of transactions with any Taproot (v1) input.
    pub taproot_tx_count: u64,
    /// Decoded ASCII text from the coinbase scriptSig.
    pub coinbase_text: String,
    /// 25th percentile fee rate in sat/vB.
    pub fee_rate_p25: f64,
    /// 75th percentile fee rate in sat/vB.
    pub fee_rate_p75: f64,
}

impl BitcoinRpc {
    /// Create a new RPC client.
    ///
    /// Timeout strategy:
    /// - `timeout(30s)`: per-request total budget. Bitcoin Core can stall up to
    ///   several seconds during block validation; 30s gives comfortable headroom
    ///   without letting a dead upstream hang a handler indefinitely.
    /// - `connect_timeout(10s)`: fail fast when the TCP handshake can't complete
    ///   (e.g. WireGuard tunnel renegotiating, node offline, routing broken).
    /// - `pool_idle_timeout(30s)`: drop keep-alive sockets after 30s idle. Without
    ///   this, if Bitcoin Core's container restarts and its internal IP shifts,
    ///   the NAT on the intermediate router quietly blackholes our pooled
    ///   connections: subsequent reuses hit a dead socket and the per-request
    ///   30s timeout kicks in on EVERY new request. With a 30s pool idle cap,
    ///   worst-case dead-pool lifetime is bounded.
    /// - `tcp_keepalive(30s)`: kernel-level probe interval. If the path
    ///   silently drops (common with NAT table churn), the OS detects a dead
    ///   socket and marks it ERROR, so reqwest's pool stops handing it out.
    pub fn new(url: String, user: String, password: String) -> Self {
        Self::with_timeout(url, user, password, Duration::from_secs(30))
    }

    /// Same client with a caller-chosen request timeout.
    ///
    /// 30s is right for live operation, where a slow response means something
    /// is wrong and failing fast is the correct answer. It is too tight for
    /// bulk work against verbosity 2: a 4 MB block becomes ~26 MB of JSON
    /// that the node emits at about 1.1 MB/s, which exceeds 30s on its own.
    /// Measured 2026-10-09; the same blocks come back in under 1.4s at
    /// verbosity 0.
    pub fn with_timeout(
        url: String,
        user: String,
        password: String,
        timeout: Duration,
    ) -> Self {
        Self {
            client: Client::builder()
                .timeout(timeout)
                .connect_timeout(std::time::Duration::from_secs(10))
                .pool_idle_timeout(std::time::Duration::from_secs(30))
                .tcp_keepalive(std::time::Duration::from_secs(30))
                // Bind the local socket to IPv4, which confines every
                // connection to IPv4.
                //
                // hyper does not implement happy eyeballs: it tries resolved
                // addresses in order and does not race or fall back, so a
                // blackholed AAAA route stalls the whole request until the
                // timeout. curl falls back and succeeds, which is what made
                // this look like a server-side problem for four wrong
                // hypotheses on 2026-09-08. Both external APIs are dual-stack
                // and the Bitcoin node is reached over IPv4 on the LAN, so
                // nothing here needs IPv6.
                //
                // The trigger on that box was ProtonVPN's IPv6 leak
                // protection, but the point is not to depend on the host
                // having a working IPv6 route for an outbound call that does
                // not need one.
                .local_address(std::net::IpAddr::V4(
                    std::net::Ipv4Addr::UNSPECIFIED,
                ))
                .build()
                .expect("Failed to build HTTP client"),
            url,
            user,
            password,
            mempool_info_cache: Arc::new(CachedSlot::new()),
            blockchain_info_cache: Arc::new(CachedSlot::new()),
            network_hashps_cache: Arc::new(CachedSlot::new()),
            smart_fee_caches: Arc::new(Mutex::new(HashMap::new())),
            block_hash_cache: Arc::new(LruSlot::new(BLOCK_HASH_CACHE_CAPACITY)),
            block_data_cache: Arc::new(LruSlot::new(BLOCK_DATA_CACHE_CAPACITY)),
        }
    }

    /// Invalidate caches whose values change with every new block.
    ///
    /// Called from the ZMQ `hashblock` handler with the new block's height.
    /// Invalidates:
    /// - `blockchain_info_cache` so the next `/api/stats/live` poll picks up
    ///   the fresh tip height / difficulty / best block hash. Without this,
    ///   the 30s backstop TTL would hold stale tip data for up to 30s.
    /// - `block_hash_cache` entries within the reorg-detection window
    ///   `[height - REORG_DETECTION_DEPTH ..= height]`. The reorg detector
    ///   compares DB's stored hashes against canonical via `get_block_hash`.
    ///   If a reorged height's cached entry is stale, the comparison silently
    ///   returns "no reorg": a correctness bug. Invalidating here forces
    ///   the next sweep to fetch fresh canonical data.
    ///
    /// Not invalidated (and why):
    /// - `smart_fee`: smoothed across many blocks, one-block staleness invisible.
    /// - `network_hashps`: long moving average, same reasoning.
    /// - `mempool_info`: drops confirmed txs naturally; TTL short enough that
    ///   one block of staleness isn't worth the invalidation bookkeeping.
    /// - `block_data_cache`: keyed by hash, not height. Historical block data
    ///   for hashes that still exist is still valid. Reorged hashes are
    ///   explicitly invalidated via `invalidate_block_caches` when the reorg
    ///   detector catches them in `ingest::verify_recent_blocks`.
    pub fn invalidate_tip_caches(&self, new_tip_height: u64) {
        self.blockchain_info_cache.invalidate();
        let low = new_tip_height.saturating_sub(REORG_DETECTION_DEPTH);
        for h in low..=new_tip_height {
            self.block_hash_cache.remove(&h);
        }
    }

    /// Drop cached entries for a block that's been reorged away. Called from
    /// `ingest::verify_recent_blocks` after it detects a stored hash no longer
    /// matches the canonical chain. Without this, subsequent lookups at the
    /// affected height / old hash would return the stale pre-reorg data from
    /// the block-hash and block-data LRUs.
    pub fn invalidate_block_caches(&self, height: u64, hash: &str) {
        self.block_hash_cache.remove(&height);
        self.block_data_cache.remove(&hash.to_string());
    }

    /// Snapshot of block-cache LRU counters for `/api/stats/cache-stats`.
    /// Reports hits, misses, current size, and capacity per LRU.
    pub fn block_cache_stats(&self) -> Vec<(&'static str, LruStats)> {
        vec![
            ("getblockhash", self.block_hash_cache.stats()),
            ("getblock", self.block_data_cache.stats()),
        ]
    }

    /// Snapshot of per-slot cache counters for `/api/stats/cache-stats`.
    /// Returns `(method, stats)` tuples. Methods not listed aren't cached.
    pub fn cache_stats(&self) -> Vec<(String, SlotStats)> {
        let mut stats = vec![
            (
                "getmempoolinfo".to_string(),
                self.mempool_info_cache.stats(),
            ),
            (
                "getblockchaininfo".to_string(),
                self.blockchain_info_cache.stats(),
            ),
            (
                "getnetworkhashps".to_string(),
                self.network_hashps_cache.stats(),
            ),
        ];
        // Each per-target smart-fee slot appears with its target in the key
        // (e.g. "estimatesmartfee:1") so the endpoint surfaces each one.
        let map = self
            .smart_fee_caches
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for (target, slot) in map.iter() {
            stats.push((format!("estimatesmartfee:{target}"), slot.stats()));
        }
        stats
    }

    fn smart_fee_slot(&self, target: u64) -> Arc<CachedSlot<f64>> {
        let mut map = self
            .smart_fee_caches
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.entry(target)
            .or_insert_with(|| Arc::new(CachedSlot::new()))
            .clone()
    }

    /// Send a JSON-RPC 1.0 request to Bitcoin Core.
    async fn call(
        &self,
        method: &str,
        params: &[Value],
    ) -> Result<Value, StatsError> {
        let body = json!({
            "jsonrpc": "1.0",
            "id": "bitcoin_stats",
            "method": method,
            "params": params,
        });

        let resp = self
            .client
            .post(&self.url)
            .basic_auth(&self.user, Some(&self.password))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(StatsError::Rpc(format!(
                "RPC returned status {}",
                resp.status()
            )));
        }

        let mut result: Value = resp.json().await?;

        if let Some(error) = result.get("error") {
            if !error.is_null() {
                return Err(StatsError::Rpc(format!("RPC error: {error}")));
            }
        }

        Ok(result["result"].take())
    }

    /// Like [`Self::call`] but deserializes the response body DIRECTLY into `T`
    /// instead of into a generic `serde_json::Value` first. For a large,
    /// known-shape response this avoids materializing a Value node per field:
    /// serde only allocates what `T` declares and silently skips the rest. Used
    /// for `getrawmempool verbose`, which is tens of MB of mostly-discarded
    /// fields: a full Value tree of that is a ~50-90MB transient spike on the
    /// memory-tight droplet. `call` stays for the small / dynamically-shaped
    /// responses where a Value is fine.
    async fn call_typed<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: &[Value],
    ) -> Result<T, StatsError> {
        let body = json!({
            "jsonrpc": "1.0",
            "id": "bitcoin_stats",
            "method": method,
            "params": params,
        });

        let resp = self
            .client
            .post(&self.url)
            .basic_auth(&self.user, Some(&self.password))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(StatsError::Rpc(format!(
                "RPC returned status {}",
                resp.status()
            )));
        }

        // reqwest buffers the body then `from_slice`s straight into T, no
        // intermediate Value tree.
        let parsed: RpcResponse<T> = resp.json().await?;
        if let Some(error) = parsed.error {
            if !error.is_null() {
                return Err(StatsError::Rpc(format!("RPC error: {error}")));
            }
        }
        parsed.result.ok_or_else(|| {
            StatsError::Rpc("RPC response missing result".to_string())
        })
    }

    /// Call `getblockchaininfo` - returns chain state, tip height, difficulty.
    /// Cached with a 30s TTL acting as a backstop; the primary freshness
    /// mechanism is explicit invalidation from the ZMQ `hashblock` handler
    /// (see [`Self::invalidate_tip_caches`]) which fires every time a new
    /// block arrives. Returns `(info, is_stale)` where `is_stale` is true
    /// when the upstream call failed and the cache fell back to the last known
    /// value. Use [`Self::get_blockchain_info_fresh`] for paths that must
    /// bypass the cache (ingest, ZMQ refreshes).
    pub async fn get_blockchain_info(
        &self,
    ) -> Result<(BlockchainInfo, bool), StatsError> {
        self.blockchain_info_cache
            .get_or_fetch(BLOCKCHAIN_INFO_TTL, || {
                self.get_blockchain_info_fresh()
            })
            .await
    }

    /// Force-refresh `getblockchaininfo`, bypassing the cache.
    pub async fn get_blockchain_info_fresh(
        &self,
    ) -> Result<BlockchainInfo, StatsError> {
        let result = self.call("getblockchaininfo", &[]).await?;
        serde_json::from_value(result)
            .map_err(|e| StatsError::Rpc(e.to_string()))
    }

    /// Estimated network hash rate (hashes per second). Cached with a 60s TTL.
    /// Returns `(hashrate, is_stale)`.
    pub async fn get_network_hashps(&self) -> Result<(f64, bool), StatsError> {
        self.network_hashps_cache
            .get_or_fetch(NETWORK_HASHPS_TTL, || {
                self.get_network_hashps_fresh()
            })
            .await
    }

    /// Force-refresh `getnetworkhashps`, bypassing the cache.
    pub async fn get_network_hashps_fresh(&self) -> Result<f64, StatsError> {
        let result = self.call("getnetworkhashps", &[]).await?;
        result.as_f64().ok_or_else(|| {
            StatsError::Rpc("Expected number for hashps".to_string())
        })
    }

    /// Estimate fee rate (sat/vB) to confirm within `target` blocks.
    /// Cached per target with a 10s TTL. Returns `(rate, is_stale)`.
    pub async fn estimate_smart_fee(
        &self,
        target: u64,
    ) -> Result<(f64, bool), StatsError> {
        let slot = self.smart_fee_slot(target);
        slot.get_or_fetch(SMART_FEE_TTL, || {
            self.estimate_smart_fee_fresh(target)
        })
        .await
    }

    /// Force-refresh `estimatesmartfee`, bypassing the cache.
    pub async fn estimate_smart_fee_fresh(
        &self,
        target: u64,
    ) -> Result<f64, StatsError> {
        let result = self.call("estimatesmartfee", &[json!(target)]).await?;
        // Returns feerate in BTC/kvB, convert to sat/vB: * 1e8 / 1000
        let btc_per_kvb = result["feerate"].as_f64().unwrap_or(0.0);
        Ok(btc_per_kvb * 100_000.0) // BTC/kvB -> sat/vB
    }

    /// Call `getblockhash` - returns the block hash at a given height.
    ///
    /// Cached via [`LruSlot`] since the hash at a given height is immutable
    /// (absent a reorg, which is handled by explicit invalidation, see
    /// [`Self::invalidate_block_caches`]). Capacity is 10k entries, enough
    /// to hold hashes for the last ~70 days of blocks.
    pub async fn get_block_hash(
        &self,
        height: u64,
    ) -> Result<String, StatsError> {
        if let Some(hash) = self.block_hash_cache.get(&height) {
            return Ok(hash);
        }
        let hash = self.get_block_hash_fresh(height).await?;
        self.block_hash_cache.put(height, hash.clone());
        Ok(hash)
    }

    /// Put a known-wrong hash in the cache, to stand in for a pre-reorg
    /// response landing after invalidation. Tests only.
    #[cfg(test)]
    pub fn poison_block_hash_cache_for_test(&self, height: u64, hash: String) {
        self.block_hash_cache.put(height, hash);
    }

    /// Force-refresh `getblockhash`, bypassing the cache.
    pub async fn get_block_hash_fresh(
        &self,
        height: u64,
    ) -> Result<String, StatsError> {
        let result = self.call("getblockhash", &[json!(height)]).await?;
        result.as_str().map(|s| s.to_string()).ok_or_else(|| {
            StatsError::Rpc("Expected string for block hash".to_string())
        })
    }

    /// Fetch a block by hash, preferring the LRU cache for previously-seen
    /// blocks. Confirmed blocks are immutable, so a cache hit saves a full
    /// verbosity=2 RPC call and the subsequent single-pass parser. Call
    /// [`Self::get_block_fresh`] to bypass the cache, or
    /// [`Self::invalidate_block_caches`] on reorg to drop affected entries.
    pub async fn get_block(&self, hash: &str) -> Result<Block, StatsError> {
        if let Some(block) = self.block_data_cache.get(&hash.to_string()) {
            return Ok(block);
        }
        let block = self.get_block_fresh(hash).await?;
        self.block_data_cache.put(block.hash.clone(), block.clone());
        Ok(block)
    }

    /// Force-refresh a block, bypassing the cache. The core data extraction
    /// function: parses the full verbosity=2 JSON response to compute fees,
    /// median stats, OP_RETURN classification, and miner ID in a single pass.
    pub async fn get_block_fresh(
        &self,
        hash: &str,
    ) -> Result<Block, StatsError> {
        let result = self.call("getblock", &[json!(hash), json!(2)]).await?;
        Ok(Self::parse_block_value(&result))
    }

    /// Fetch a block as raw consensus bytes and derive the same `Block`.
    ///
    /// `getblock` verbosity 2 makes the node decode every transaction into
    /// JSON, expanding a 1.6 MB block to about 10 MB. Measured 2026-10-09
    /// against the Start9 over LAN, that caps a full-chain sweep at ~3 MB/s,
    /// or roughly 16 days. Verbosity 0 ships the block as hex and does almost
    /// no work on the node: 35 MB/s, and it scales with concurrency where
    /// verbosity 2 does not. Details in `notes/raw-block-sweep-scope.md`.
    ///
    /// **`fee` is the one thing lost.** Core computes per-transaction fees
    /// from its undo data, which is not in the block, so every fee-derived
    /// field here reads zero. Callers must not write those columns from this
    /// path; `getblockstats` is the cheap source for the block-level ones.
    pub async fn get_block_raw(
        &self,
        hash: &str,
        height: u64,
    ) -> Result<Block, StatsError> {
        use bitcoin::consensus::encode::deserialize;
        use bitcoin::hex::FromHex;

        let result = self.call("getblock", &[json!(hash), json!(0)]).await?;
        let hex = result.as_str().ok_or_else(|| {
            StatsError::Rpc(
                "getblock verbosity 0 did not return a string".into(),
            )
        })?;
        let bytes = Vec::<u8>::from_hex(hex)
            .map_err(|e| StatsError::Rpc(format!("block hex: {e}")))?;
        let block: bitcoin::Block = deserialize(&bytes)
            .map_err(|e| StatsError::Rpc(format!("block decode: {e}")))?;
        Ok(Self::parse_block_value(&synthesize_block_value(
            &block, height,
        )))
    }

    /// Everything a verbosity-2 `getblock` response means, with no I/O.
    ///
    /// Split out of `get_block_fresh` so a second fetch path can reuse it
    /// verbatim. This is 450 lines of classification that took a long time to
    /// get right, and the one thing a raw-block parser must not do is grow a
    /// second copy of it: the OP_RETURN two-rule defect is what happens when
    /// two code paths disagree about what a column means.
    ///
    /// Total and infallible by construction. Every field reads through
    /// `unwrap_or`, so a malformed response yields zeros rather than an
    /// error, which is why callers get a `Block` and not a `Result`.
    pub(crate) fn parse_block_value(result: &Value) -> Block {
        // Hoisted: the coinbase section below is the first to touch it.
        let mut largest_tx_size = 0u64;
        // Hoisted above the coinbase section, which is now the first
        // place these are touched: coinbase outputs are classified by
        // script type like any other.
        let mut p2pk_count = 0u64;
        let mut p2pkh_count = 0u64;
        let mut p2sh_count = 0u64;
        let mut p2wpkh_count = 0u64;
        let mut p2wsh_count = 0u64;
        let mut p2tr_count = 0u64;
        let mut multisig_count = 0u64;
        let mut unknown_script_count = 0u64;
        let mut p2a_count = 0u64;
        let mut output_count = 0u64;
        let mut coinbase_output_count = 0u64;
        let mut unspendable_output_count = 0u64;
        let mut taproot_spend_count = 0u64;
        // Declared up here because the coinbase section below is the first
        // place they are touched; see the comment there for why the coinbase
        // counts toward these three and not toward `total_output_value`.
        let mut p2pk_value = 0u64;
        let mut p2tr_value = 0u64;
        let mut multisig_value = 0u64;
        let hash = result["hash"].as_str().unwrap_or_default().to_string();
        let height = result["height"].as_u64().unwrap_or(0);
        let time = result["time"].as_u64().unwrap_or(0);
        let n_tx = result["nTx"].as_u64().unwrap_or(0);
        let size = result["size"].as_u64().unwrap_or(0);
        let weight = result["weight"].as_u64().unwrap_or(0);
        let difficulty = result["difficulty"].as_f64().unwrap_or(0.0);
        let version = result["version"].as_u64().unwrap_or(0) as u32;

        // === Coinbase extraction: fees, miner ID, locktime, text ===
        let mut total_fees = 0u64;
        let mut coinbase_text = String::new();
        let mut coinbase_locktime = 0u64;
        let mut coinbase_sequence = 0xFFFF_FFFFu64;
        let mut miner = String::from("Unknown");
        if let Some(txs) = result["tx"].as_array() {
            if let Some(coinbase_tx) = txs.first() {
                // BIP-54 signaling: coinbase nLockTime == height - 1 AND nSequence != 0xffffffff
                coinbase_locktime =
                    coinbase_tx["locktime"].as_u64().unwrap_or(0);
                if let Some(vin) = coinbase_tx["vin"].as_array() {
                    if let Some(first_vin) = vin.first() {
                        coinbase_sequence = first_vin["sequence"]
                            .as_u64()
                            .unwrap_or(0xFFFF_FFFF);
                    }
                }

                // Total fees = coinbase output value - block subsidy
                let mut coinbase_value = 0u64;
                if let Some(vouts) = coinbase_tx["vout"].as_array() {
                    for vout in vouts {
                        let sats = vout["value"]
                            .as_f64()
                            .map(|v| (v * 100_000_000.0).round() as u64)
                            .unwrap_or(0);
                        coinbase_value += sats;
                        coinbase_output_count += 1;
                        if is_unspendable(
                            vout["scriptPubKey"]["hex"].as_str().unwrap_or(""),
                        ) {
                            unspendable_output_count += 1;
                        }
                        // The exposed-value columns count coinbase outputs,
                        // unlike `total_output_value` beside them, and the
                        // difference is deliberate.
                        //
                        // `total_output_value` excludes the coinbase because
                        // it measures transfers, and newly issued coins are
                        // not a transfer. These three measure value placed
                        // into script types whose public key is on chain, and
                        // the 2009-2012 coinbase rewards paid to raw public
                        // keys are the canonical case of exactly that:
                        // Satoshi's coins are P2PK coinbase outputs.
                        //
                        // Excluding them read 0 where the truth was 50 BTC in
                        // every early block, which an independent oracle
                        // caught at heights 1, 57,043 and 170,000. The
                        // differential test could not: both of its paths run
                        // this function, so a wrong population is wrong
                        // identically on both sides.
                        // Coinbase outputs are classified by script type
                        // exactly as every other output is, and are counted.
                        //
                        // The nine classified types are a partition of
                        // *spendable* outputs, and a coinbase payout is one.
                        // Excluding it made block 1 report no outputs at all,
                        // and the early era, which is almost entirely
                        // coinbase, read as empty on every address-type
                        // chart. Modern blocks move by ~0.05%, since a
                        // coinbase carries a handful of outputs against
                        // thousands.
                        //
                        // `op_return_count` is deliberately *not* included
                        // here and stays non-coinbase: merged-mining
                        // commitments are not user data, which is the
                        // decision the sweep exists to apply. The classified
                        // denominator excludes OP_RETURN entirely, so the two
                        // choices do not interact.
                        match vout["scriptPubKey"]["type"].as_str() {
                            Some("pubkey") => {
                                p2pk_count += 1;
                                p2pk_value += sats;
                                output_count += 1;
                            }
                            Some("pubkeyhash") => {
                                p2pkh_count += 1;
                                output_count += 1;
                            }
                            Some("scripthash") => {
                                p2sh_count += 1;
                                output_count += 1;
                            }
                            Some("witness_v0_keyhash") => {
                                p2wpkh_count += 1;
                                output_count += 1;
                            }
                            Some("witness_v0_scripthash") => {
                                p2wsh_count += 1;
                                output_count += 1;
                            }
                            Some("witness_v1_taproot") => {
                                p2tr_count += 1;
                                p2tr_value += sats;
                                taproot_spend_count += 1;
                                output_count += 1;
                            }
                            Some("multisig") => {
                                multisig_count += 1;
                                multisig_value += sats;
                                output_count += 1;
                            }
                            Some("anchor") => {
                                p2a_count += 1;
                                unknown_script_count += 1;
                                output_count += 1;
                            }
                            // `nulldata` is outside the nine classified
                            // types but inside `output_count`, matching how
                            // the non-coinbase loop treats it. The
                            // embedded-data columns still skip coinbase
                            // OP_RETURNs by decision; only the output tally
                            // counts them.
                            //
                            // This is queue item E, coinbase outputs for UTXO
                            // growth: `output_count` was short by the whole
                            // coinbase, measured at 1 to 5 outputs per block
                            // against mempool.space.
                            Some("nulldata") => output_count += 1,
                            _ => {
                                unknown_script_count += 1;
                                output_count += 1;
                            }
                        }
                    }
                }
                // The coinbase counts toward the block's largest transaction.
                //
                // It is a transaction, and the chart's copy says "the biggest
                // single transaction in each block". Excluding it made 89,939
                // coinbase-only blocks store 0, which the chart drew as no
                // data: 9.3% of the chain, almost all of it the early era.
                // No modern block changes, since a coinbase is never the
                // largest there.
                if let Some(sz) = coinbase_tx["size"].as_u64() {
                    if sz > largest_tx_size {
                        largest_tx_size = sz;
                    }
                }

                let subsidy = classifier::block_subsidy(height);
                total_fees = coinbase_value.saturating_sub(subsidy);

                // Miner ID: check coinbase scriptSig first
                if let Some(vin) = coinbase_tx["vin"].as_array() {
                    if let Some(first_vin) = vin.first() {
                        if let Some(coinbase_hex) =
                            first_vin["coinbase"].as_str()
                        {
                            miner = classifier::identify_miner(coinbase_hex);
                            // Extract printable ASCII from coinbase for graffiti
                            let cb_bytes: Vec<u8> = (0..coinbase_hex.len())
                                .step_by(2)
                                .filter_map(|i| {
                                    coinbase_hex.get(i..i + 2).and_then(|s| {
                                        u8::from_str_radix(s, 16).ok()
                                    })
                                })
                                .collect();
                            coinbase_text = cb_bytes
                                .iter()
                                .filter(|&&b| (0x20..=0x7e).contains(&b))
                                .map(|&b| b as char)
                                .collect::<String>()
                                .trim()
                                .chars()
                                .take(200) // cap at 200 chars
                                .collect();
                        }
                    }
                }

                // Fallback: check coinbase OP_RETURN outputs for miner tag
                if miner == "Unknown" {
                    if let Some(vouts) = coinbase_tx["vout"].as_array() {
                        for vout in vouts {
                            if vout["scriptPubKey"]["type"].as_str()
                                == Some("nulldata")
                            {
                                if let Some(hex) =
                                    vout["scriptPubKey"]["hex"].as_str()
                                {
                                    let found = classifier::identify_miner(hex);
                                    if found != "Unknown" {
                                        miner = found.to_string();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // === Single pass over non-coinbase txs: fees, OP_RETURN, outputs, inputs, RBF, witness ===
        let mut inscription_count = 0u64;
        let mut inscription_bytes = 0u64; // payload only (envelope overhead subtracted)
        let mut inscription_envelope_bytes = 0u64; // full witness item bytes (envelope + payload)
        let mut brc20_count = 0u64;

        let cap = n_tx.saturating_sub(1) as usize;
        let mut tx_fees: Vec<u64> = Vec::with_capacity(cap);
        let mut tx_fee_rates: Vec<f64> = Vec::with_capacity(cap);
        let mut op_return_count = 0u64;
        let mut op_return_bytes = 0u64;
        let mut runes_count = 0u64;
        let mut runes_bytes = 0u64;
        let mut omni_count = 0u64;
        let mut omni_bytes = 0u64;
        let mut counterparty_count = 0u64;
        let mut counterparty_bytes = 0u64;
        let mut data_carrier_count = 0u64;
        let mut data_carrier_bytes = 0u64;
        let mut segwit_spend_count = 0u64;
        let mut op_return_max_bytes = 0u64;
        let mut op_return_over_80_count = 0u64;
        let mut tx_v1_count = 0u64;
        let mut tx_v2_count = 0u64;
        let mut tx_v3_count = 0u64;
        let mut brc20_bytes = 0u64;
        let mut input_count = 0u64;
        let mut rbf_count = 0u64;
        let mut witness_bytes = 0u64;
        let mut taproot_keypath_count = 0u64;
        let mut taproot_scriptpath_count = 0u64;
        let mut total_output_value = 0u64;
        let mut stamps_count = 0u64;
        let mut max_tx_fee = 0u64;
        let mut inscription_fees = 0u64;
        let mut runes_fees = 0u64;
        let mut legacy_tx_count = 0u64;
        let mut segwit_tx_count = 0u64;
        let mut taproot_tx_count = 0u64;

        if let Some(txs) = result["tx"].as_array() {
            for tx in txs.iter().skip(1) {
                // Transaction version. v3 is TRUC, which Lightning's next
                // generation is built on and which is only just appearing.
                // Counted rather than stored per transaction, since the
                // blocks table is one row per block.
                //
                // Excludes the coinbase, so these three sum to `nTx - 1`
                // rather than `nTx`. That matches `legacy_tx_count`,
                // `segwit_tx_count` and `taproot_tx_count` beside them, all
                // of which describe user transactions; the coinbase has no
                // inputs to classify and its version is the miner's choice
                // rather than a wallet's. Checked against an oracle at ten
                // heights, where the gap is exactly one every time.
                match tx["version"].as_u64() {
                    Some(1) => tx_v1_count += 1,
                    Some(2) => tx_v2_count += 1,
                    Some(3) => tx_v3_count += 1,
                    _ => {}
                }

                // --- Fees ---
                let mut this_fee_sats = 0u64;
                if let Some(fee_btc) = tx["fee"].as_f64() {
                    this_fee_sats = (fee_btc * 100_000_000.0).round() as u64;
                    tx_fees.push(this_fee_sats);
                    if this_fee_sats > max_tx_fee {
                        max_tx_fee = this_fee_sats;
                    }
                    if let Some(vsize) = tx["vsize"].as_u64() {
                        if vsize > 0 {
                            tx_fee_rates
                                .push(this_fee_sats as f64 / vsize as f64);
                        }
                    }
                }
                // Per-tx protocol flags (set during output scan, used for fee attribution)
                let mut tx_has_inscription = false;
                let mut tx_has_runes = false;
                let mut tx_has_taproot_input = false;
                let mut inputs_without_witness = 0u64;

                // --- Inputs: counting, SegWit detection, RBF, witness bytes ---
                // Track largest tx
                if let Some(sz) = tx["size"].as_u64() {
                    if sz > largest_tx_size {
                        largest_tx_size = sz;
                    }
                }

                let mut has_witness = false;
                let mut is_rbf = false;
                if let Some(vins) = tx["vin"].as_array() {
                    input_count += vins.len() as u64;
                    for vin in vins {
                        // Witness detection + byte counting + inscription detection
                        if vin["txinwitness"].as_array().is_none() {
                            inputs_without_witness += 1;
                        }
                        if let Some(wit) = vin["txinwitness"].as_array() {
                            has_witness = true;
                            // Witness overhead: item count varint per input
                            let wit_items = wit.len() as u64;
                            witness_bytes += varint_size(wit_items);
                            for item in wit {
                                if let Some(hex) = item.as_str() {
                                    let item_bytes = (hex.len() as u64) / 2;
                                    // Each item is prefixed by a length varint
                                    witness_bytes += varint_size(item_bytes);
                                    witness_bytes += item_bytes;
                                    // Ordinals inscription envelope:
                                    // OP_FALSE(00) OP_IF(63) OP_PUSH3(03) "ord"(6f7264)
                                    if hex.contains("0063036f7264") {
                                        inscription_count += 1;
                                        tx_has_inscription = true;
                                        // Calculate actual envelope overhead instead of fixed estimate.
                                        // Envelope: OP_FALSE(1) OP_IF(1) OP_PUSH3(1) "ord"(3) = 6 bytes header
                                        // + content-type marker OP_PUSH1(1) + type length(1) + type bytes(variable)
                                        // + separator OP_0(1) + content-length push(1-3) + OP_ENDIF(1)
                                        // Approximate: find envelope start, subtract from total
                                        let overhead = if let Some(env_pos) =
                                            hex.find("0063036f7264")
                                        {
                                            // Bytes before envelope + envelope header (6 bytes = 12 hex chars)
                                            // + content-type section (scan for 00 separator after header)
                                            let after_header = env_pos + 12; // past "0063036f7264"
                                                                             // Find OP_0 separator (00) after content-type
                                            let separator_pos = hex
                                                [after_header..]
                                                .find("00")
                                                .map(|p| after_header + p)
                                                .unwrap_or(after_header);
                                            // overhead = header(6) + content-type section + separator(1) + OP_ENDIF(1)
                                            let ct_bytes = (separator_pos
                                                - after_header)
                                                / 2;
                                            (6 + ct_bytes + 2) as u64
                                        } else {
                                            10 // fallback
                                        };
                                        inscription_bytes +=
                                            item_bytes.saturating_sub(overhead);
                                        inscription_envelope_bytes +=
                                            item_bytes;
                                        // BRC-20: inscription containing {"p":"brc-20"
                                        if hex.contains(
                                            "7b2270223a226272632d3230",
                                        ) {
                                            brc20_count += 1;
                                            // Estimated payload bytes, the
                                            // same basis as
                                            // `inscription_bytes`, so the
                                            // CQ-19 volume split can subtract
                                            // one from the other.
                                            //
                                            // It held whole witness items
                                            // until 2026-10-11, which could
                                            // not be subtracted from an
                                            // envelope-adjusted payload
                                            // without going negative. Two
                                            // envelopes inside one item still
                                            // count once, and a mixed item
                                            // still counts in full; a
                                            // substring test cannot partition
                                            // either, and the chart copy has
                                            // to say so.
                                            brc20_bytes += item_bytes
                                                .saturating_sub(overhead);
                                        }
                                    }
                                }
                            }
                            // Taproot spend type detection:
                            // Key-path: exactly 1 witness element (64-65 byte Schnorr sig)
                            // Script-path: 2+ elements, last element starts with 0xc0 or 0xc1
                            //              (taproot control block version byte)
                            let wit_len = wit.len();
                            if wit_len == 1 {
                                if let Some(hex) = wit[0].as_str() {
                                    let byte_len = hex.len() / 2;
                                    if byte_len == 64 || byte_len == 65 {
                                        taproot_keypath_count += 1;
                                        tx_has_taproot_input = true;
                                    }
                                }
                            } else if wit_len >= 2 {
                                if let Some(last) =
                                    wit.last().and_then(|v| v.as_str())
                                {
                                    if last.starts_with("c0")
                                        || last.starts_with("c1")
                                    {
                                        taproot_scriptpath_count += 1;
                                        tx_has_taproot_input = true;
                                    }
                                }
                            }
                        }
                        // RBF: nSequence < 0xFFFFFFFE on any input signals
                        // replaceability. This is BIP 125 as written and as
                        // Core enforces it, with no carve-out.
                        //
                        // There was a carve-out here, skipping inputs that
                        // spend a BIP 68 relative timelock on the grounds that
                        // they set a low sequence for a different reason. It
                        // never excluded anything, and could not:
                        //
                        //   - it read `vin.prevout`, which `getblock` returns
                        //     only at verbosity 3. This call is verbosity 2.
                        //     Probed against the node on 2026-09-25 at block
                        //     900,000: prevout on 0 of 4,386 non-coinbase
                        //     inputs at verbosity 2, 4,386 of 4,386 at 3.
                        //   - it looked for OP_CHECKSEQUENCEVERIFY in the
                        //     prevout's scriptPubKey asm, where a P2WSH or
                        //     P2SH output shows a hash rather than a script.
                        //     A Lightning close would not have matched even
                        //     with prevout present.
                        //
                        // Removed rather than repaired, because the carve-out
                        // was the deviation: a CSV-locked input really is
                        // replaceable under BIP 125. Measured at verbosity 3
                        // over blocks 850,000, 900,000 and the three at the
                        // tip, 0 of 8,733 signalling transactions would have
                        // been excluded, so no stored value changes.
                        if let Some(seq) = vin["sequence"].as_u64() {
                            if seq < 0xFFFF_FFFE {
                                is_rbf = true;
                            }
                        }
                    }
                }
                if has_witness {
                    segwit_spend_count += 1;
                    // BIP 141 witness marker (0x00) + flag (0x01) = 2 bytes per witness tx
                    witness_bytes += 2;
                    // Every input of a witness transaction serializes a
                    // witness stack, and a non-witness input serializes an
                    // empty one: a single 0x00 count byte. Core omits those
                    // from `txinwitness` entirely, so counting only the
                    // present arrays lost one byte per such input.
                    //
                    // Small and real: 128 bytes missing from 935,472 at
                    // height 970,800, where 128 mixed-input transactions each
                    // carried one. Independently established by checking
                    // `tx.size - (tx.weight - tx.size) / 3` against the sum.
                    witness_bytes += inputs_without_witness;
                }
                if is_rbf {
                    rbf_count += 1;
                }

                // --- Outputs: counting, script type classification, OP_RETURN ---
                if let Some(vouts) = tx["vout"].as_array() {
                    output_count += vouts.len() as u64;
                    for vout in vouts {
                        if is_unspendable(
                            vout["scriptPubKey"]["hex"].as_str().unwrap_or(""),
                        ) {
                            unspendable_output_count += 1;
                        }
                        // Sum output values (excludes coinbase: measures actual transfers)
                        let sats = vout["value"]
                            .as_f64()
                            .map(|v| (v * 100_000_000.0).round() as u64)
                            .unwrap_or(0);
                        total_output_value += sats;
                        match vout["scriptPubKey"]["type"].as_str() {
                            Some("pubkey") => {
                                p2pk_count += 1;
                                p2pk_value += sats;
                            }
                            Some("pubkeyhash") => p2pkh_count += 1,
                            Some("scripthash") => p2sh_count += 1,
                            Some("witness_v0_keyhash") => p2wpkh_count += 1,
                            Some("witness_v0_scripthash") => p2wsh_count += 1,
                            // Core 28's pay-to-anchor, counted on its own
                            // **and** left in the unknown bucket.
                            //
                            // The overlap is deliberate and temporary. These
                            // had reached ~7% of outputs while sitting in
                            // `unknown_script_count`, which is where the
                            // eight-type denominator picks them up. Moving
                            // them out without also adding `p2a_count` to
                            // that denominator would shrink it by ~7% and
                            // silently inflate every other type's share, on
                            // 28 call sites across the chart builders whose
                            // copy says "eight classified types".
                            //
                            // So the column is additive for now: new data,
                            // zero change to any existing chart. Whether
                            // anchors become a ninth classified type is a
                            // semantics decision about live charts, recorded
                            // as open in `backfill-scope-2026-09-30.md`.
                            // Until it is taken, `p2a_count` is a subset of
                            // `unknown_script_count`, not a sibling of it.
                            // Counted here **and** left in the unknown
                            // bucket, deliberately and temporarily.
                            //
                            // Promoting anchors to a ninth classified type
                            // needs them added to four denominators in
                            // `adoption.rs`, threaded through `BlockSummary`
                            // for the per-block arm, and drawn as a ninth
                            // line in `multi-velocity`, whose copy says the
                            // lines sum to zero. Without all of that the
                            // denominator loses ~7% and every other share
                            // inflates.
                            //
                            // The parser and database layers for the
                            // promotion are done; the chart layer is not. So
                            // this stays a subset of `unknown_script_count`
                            // until that lands. See
                            // `notes/overnight-2026-10-11.md`.
                            Some("anchor") => {
                                p2a_count += 1;
                                unknown_script_count += 1;
                            }
                            Some("witness_v1_taproot") => {
                                p2tr_count += 1;
                                p2tr_value += sats;
                                taproot_spend_count += 1;
                            }
                            Some("nulldata") => {
                                // OP_RETURN classification
                                if let Some(hex) =
                                    vout["scriptPubKey"]["hex"].as_str()
                                {
                                    let bytes = (hex.len() as u64) / 2;
                                    // Tracked before classification, so the
                                    // witness commitment and every protocol
                                    // are measured alike: the question these
                                    // answer is how big a payload policy
                                    // allowed, not whose it was.
                                    if bytes > op_return_max_bytes {
                                        op_return_max_bytes = bytes;
                                    }
                                    // 83 = OP_RETURN + pushdata + 80 payload,
                                    // the old datacarrier ceiling as it
                                    // appears on chain.
                                    if bytes > 83 {
                                        op_return_over_80_count += 1;
                                    }
                                    let classification =
                                        classifier::classify(hex, height);
                                    match classification {
                                        OpReturnType::SegwitCommit => continue,
                                        OpReturnType::Runes => {
                                            runes_count += 1;
                                            runes_bytes += bytes;
                                            tx_has_runes = true;
                                        }
                                        OpReturnType::Omni => {
                                            omni_count += 1;
                                            omni_bytes += bytes;
                                        }
                                        OpReturnType::Counterparty => {
                                            counterparty_count += 1;
                                            counterparty_bytes += bytes;
                                        }
                                        OpReturnType::DataCarrier => {
                                            data_carrier_count += 1;
                                            data_carrier_bytes += bytes;
                                        }
                                    }
                                    op_return_count += 1;
                                    op_return_bytes += bytes;
                                }
                            }
                            Some("multisig") => {
                                multisig_count += 1;
                                multisig_value += sats;
                                // Stamps: bare multisig with fake pubkeys (data encoding).
                                // Real compressed pubkeys start with 02 or 03. If the hex
                                // contains keys that don't, it's likely a Stamps output.
                                if let Some(_hex) =
                                    vout["scriptPubKey"]["hex"].as_str()
                                {
                                    // Stamps multisig pattern: 1-of-N where N keys contain data.
                                    // The asm starts with "1 <key1> <key2> ... N OP_CHECKMULTISIG"
                                    // KNOWN BROKEN: this never fires. Stamps prefix
                                    // their data keys with 02/03 precisely to
                                    // look like valid compressed pubkeys, so
                                    // testing the prefix tests the wrong
                                    // property; validity means the point is on
                                    // the secp256k1 curve. Measured 2026-09:
                                    // zero detections chain-wide against 2.78M
                                    // bare multisig outputs. Kept rather than
                                    // deleted so stamps_count keeps a defined
                                    // meaning until a real decoder replaces it.
                                    // Check if any 33-byte "key" doesn't start with 02/03.
                                    if let Some(asm) =
                                        vout["scriptPubKey"]["asm"].as_str()
                                    {
                                        let parts: Vec<&str> =
                                            asm.split_whitespace().collect();
                                        // Valid multisig: M key1 key2 ... N OP_CHECKMULTISIG
                                        if parts.len() >= 4 {
                                            let has_fake_key = parts
                                                [1..parts.len() - 2]
                                                .iter()
                                                .any(|k| {
                                                    k.len() == 66
                                                        && !k.starts_with("02")
                                                        && !k.starts_with("03")
                                                });
                                            if has_fake_key {
                                                stamps_count += 1;
                                            }
                                        }
                                    }
                                }
                            }
                            Some("nonstandard") => {
                                unknown_script_count += 1;
                            }
                            _ => unknown_script_count += 1,
                        }
                    }
                }

                // Per-tx fee attribution by protocol
                if tx_has_inscription {
                    inscription_fees += this_fee_sats;
                }
                if tx_has_runes {
                    runes_fees += this_fee_sats;
                }

                // Classify tx by input type (taproot > segwit > legacy)
                if tx_has_taproot_input {
                    taproot_tx_count += 1;
                } else if has_witness {
                    segwit_tx_count += 1;
                } else {
                    legacy_tx_count += 1;
                }
            }
        }

        tx_fees.sort_unstable();
        tx_fee_rates.sort_unstable_by(|a, b| {
            a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
        });
        let median_fee = if tx_fees.is_empty() {
            0
        } else {
            tx_fees[(tx_fees.len() - 1) / 2]
        };
        let median_fee_rate = if tx_fee_rates.is_empty() {
            0.0
        } else {
            tx_fee_rates[(tx_fee_rates.len() - 1) / 2]
        };
        let fee_rate_p10 = if tx_fee_rates.len() >= 10 {
            tx_fee_rates[tx_fee_rates.len() / 10]
        } else {
            0.0
        };
        let fee_rate_p25 = if tx_fee_rates.len() >= 4 {
            tx_fee_rates[tx_fee_rates.len() / 4]
        } else {
            0.0
        };
        let fee_rate_p75 = if tx_fee_rates.len() >= 4 {
            tx_fee_rates[tx_fee_rates.len() * 3 / 4]
        } else {
            0.0
        };
        let fee_rate_p90 = if tx_fee_rates.len() >= 10 {
            tx_fee_rates[tx_fee_rates.len() * 9 / 10]
        } else {
            0.0
        };

        Block {
            hash,
            height,
            time,
            n_tx,
            size,
            weight,
            difficulty,
            op_return_count,
            op_return_bytes,
            runes_count,
            runes_bytes,
            omni_count,
            omni_bytes,
            counterparty_count,
            counterparty_bytes,
            data_carrier_count,
            data_carrier_bytes,
            version,
            total_fees,
            median_fee,
            median_fee_rate,
            coinbase_locktime,
            coinbase_sequence,
            miner,
            segwit_spend_count,
            taproot_spend_count,
            p2a_count,
            op_return_max_bytes,
            op_return_over_80_count,
            tx_v1_count,
            tx_v2_count,
            tx_v3_count,
            p2pk_value,
            p2tr_value,
            multisig_value,
            brc20_bytes,
            coinbase_output_count,
            unspendable_output_count,
            taproot_keypath_count,
            taproot_scriptpath_count,
            p2pk_count,
            p2pkh_count,
            p2sh_count,
            p2wpkh_count,
            p2wsh_count,
            p2tr_count,
            multisig_count,
            unknown_script_count,
            input_count,
            output_count,
            rbf_count,
            witness_bytes,
            inscription_count,
            inscription_bytes,
            inscription_envelope_bytes,
            brc20_count,
            total_output_value,
            // Derived, not observed. This used to sum `vin.prevout.value`,
            // but `getblock` is called at verbosity 2 and Core only returns
            // prevout at verbosity 3, so the field silently never matched and
            // the column was 0 for all 965,970 blocks while one chart plotted
            // it as data. Verbosity 3 would fix the read at the cost of a much
            // larger response on a 4GB box; the accounting identity gives the
            // same number for free, and exactly: every satoshi entering the
            // non-coinbase transactions either leaves as an output or is the
            // fee. `total_output_value` skips the coinbase (txs.iter().skip(1))
            // and `total_fees` is the block's fee total, so the two sides match.
            total_input_value: total_output_value + total_fees,
            fee_rate_p10,
            fee_rate_p90,
            stamps_count,
            largest_tx_size,
            max_tx_fee,
            inscription_fees,
            runes_fees,
            legacy_tx_count,
            segwit_tx_count,
            taproot_tx_count,
            coinbase_text,
            fee_rate_p25,
            fee_rate_p75,
        }
    }

    /// Fetch a block by height over the raw path.
    ///
    /// The sweep's fetch. Costs one `getblockhash` plus one `getblock`
    /// verbosity 0, against the ~10 MB of JSON verbosity 2 makes the node
    /// build. Forward ingestion deliberately keeps the JSON path, because it
    /// needs per-transaction fees and runs once per block rather than a
    /// million times.
    pub async fn fetch_block_raw_by_height(
        &self,
        height: u64,
    ) -> Result<Block, StatsError> {
        // The hash is fetched fresh, deliberately, not through the cache.
        //
        // `invalidate_block_caches` clears a height on reorg, but a
        // `getblockhash` issued before that can land after it and reinstall
        // the orphan. The sweep would then fetch the orphan block, match the
        // not-yet-repaired stored row, and stamp it as swept; the verifier
        // reads the same poisoned cache and agrees. At the edge of the
        // six-block verification window the row is never revisited.
        //
        // A hash response is tens of bytes against a block of megabytes, so
        // skipping the cache costs essentially nothing here, and the sweep is
        // the one caller that reads every height exactly once anyway.
        let hash = self.get_block_hash_fresh(height).await?;
        self.get_block_raw(&hash, height).await
    }

    /// Convenience method: fetch a block by height (calls getblockhash then getblock).
    pub async fn fetch_block_by_height(
        &self,
        height: u64,
    ) -> Result<Block, StatsError> {
        let hash = self.get_block_hash(height).await?;
        self.get_block(&hash).await
    }

    /// Get mempool entry for a specific txid. Returns fee (sats) and vsize.
    pub async fn get_mempool_entry(
        &self,
        txid: &str,
    ) -> Result<MempoolEntryInfo, StatsError> {
        let result = self.call("getmempoolentry", &[json!(txid)]).await?;
        let fee_btc = result["fees"]["base"]
            .as_f64()
            .or_else(|| result["fee"].as_f64())
            .ok_or_else(|| {
                StatsError::Rpc(format!(
                    "No fee field in mempool entry for {txid}"
                ))
            })?;
        let fee_sats = (fee_btc * 100_000_000.0).round() as u64;
        let vsize = result["vsize"].as_u64().unwrap_or(0) as u32;
        Ok(MempoolEntryInfo {
            fee: fee_sats,
            vsize,
        })
    }

    /// Get block header (fast, ~1KB response). Used for immediate block broadcast.
    pub async fn get_block_header(
        &self,
        hash: &str,
    ) -> Result<BlockTxids, StatsError> {
        let result = self.call("getblockheader", &[json!(hash)]).await?;
        Ok(BlockTxids {
            height: result["height"].as_u64().unwrap_or(0),
            timestamp: result["time"].as_u64().unwrap_or(0),
            size: 0,   // not in header
            weight: 0, // not in header
            tx_count: result["nTx"].as_u64().unwrap_or(0),
            txids: vec![], // not in header
        })
    }

    /// Get block metadata + txid list (verbosity=1, no full tx data).
    pub async fn get_block_txids(
        &self,
        hash: &str,
    ) -> Result<BlockTxids, StatsError> {
        let result = self.call("getblock", &[json!(hash), json!(1)]).await?;
        let height = result["height"].as_u64().unwrap_or(0);
        let timestamp = result["time"].as_u64().unwrap_or(0);
        let size = result["size"].as_u64().unwrap_or(0);
        let weight = result["weight"].as_u64().unwrap_or(0);
        let tx_count = result["nTx"].as_u64().unwrap_or(0);
        let txids = result["tx"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        Ok(BlockTxids {
            height,
            timestamp,
            size,
            weight,
            tx_count,
            txids,
        })
    }

    /// Get all mempool entries with fee/vsize data (verbose=true).
    /// Used on startup to seed the mempool_txs table for full coverage.
    pub async fn get_raw_mempool_verbose(
        &self,
    ) -> Result<Vec<(String, u64, u32)>, StatsError> {
        // Deserialize straight into the typed subset (RawMempoolEntry) via
        // call_typed rather than a serde_json::Value: the verbose response is
        // tens of MB of mostly-discarded fields, and a full Value tree of that
        // is a ~50-90MB transient allocation spike on the memory-tight droplet.
        // This keeps only vsize + fees.base.
        let map: std::collections::HashMap<String, RawMempoolEntry> =
            self.call_typed("getrawmempool", &[json!(true)]).await?;
        let mut entries = Vec::with_capacity(map.len());
        for (txid, entry) in map {
            let fee_sats = (entry.fees.base * 100_000_000.0).round() as u64;
            if entry.vsize > 0 {
                entries.push((txid, fee_sats, entry.vsize));
            }
        }
        Ok(entries)
    }

    /// Get the current mempool as a plain txid list (`getrawmempool false`).
    /// Much lighter than the verbose form: used to reconcile the mempool_txs
    /// table against the node's real mempool (prune txs that left via RBF /
    /// eviction / confirmation).
    pub async fn get_raw_mempool(&self) -> Result<Vec<String>, StatsError> {
        let result = self.call("getrawmempool", &[json!(false)]).await?;
        let arr = result
            .as_array()
            .ok_or_else(|| StatsError::Rpc("expected array".to_string()))?;
        let mut txids = Vec::with_capacity(arr.len());
        for v in arr {
            if let Some(s) = v.as_str() {
                txids.push(s.to_string());
            }
        }
        Ok(txids)
    }

    /// Get (size, weight, total_fees) for a block via a single getblockstats
    /// call by hash. The response is small (a few scalars), so this is fast
    /// even over WireGuard: used together with `get_block_header` to broadcast
    /// the heartbeat block spike immediately, without waiting on the full txid
    /// list from `getblock(hash, 1)`.
    pub async fn get_block_stats_brief(
        &self,
        hash: &str,
    ) -> Result<(u64, u64, u64), StatsError> {
        let result = self
            .call(
                "getblockstats",
                &[
                    json!(hash),
                    json!(["total_size", "total_weight", "totalfee"]),
                ],
            )
            .await?;
        let size = result["total_size"].as_u64().unwrap_or(0);
        let weight = result["total_weight"].as_u64().unwrap_or(0);
        let total_fees = result["totalfee"].as_u64().unwrap_or(0);
        Ok((size, weight, total_fees))
    }

    /// Call `getmempoolinfo` - returns mempool size, fee stats, memory usage.
    /// Cached with a 13s TTL. Mempool contents change every second as new
    /// txs arrive, but tx count drifting by 13s is imperceptible in the UI
    /// and the real-time heartbeat SSE streams per-tx arrivals instantly.
    /// 13s spans two full 6-second dashboard poll intervals, producing a
    /// ~67% hit rate for single-tab traffic and higher under concurrent load.
    /// Returns `(info, is_stale)`.
    pub async fn get_mempool_info(
        &self,
    ) -> Result<(MempoolInfo, bool), StatsError> {
        self.mempool_info_cache
            .get_or_fetch(MEMPOOL_INFO_TTL, || self.get_mempool_info_fresh())
            .await
    }

    /// Force-refresh `getmempoolinfo`, bypassing the cache.
    pub async fn get_mempool_info_fresh(
        &self,
    ) -> Result<MempoolInfo, StatsError> {
        let result = self.call("getmempoolinfo", &[]).await?;
        serde_json::from_value(result)
            .map_err(|e| StatsError::Rpc(e.to_string()))
    }

    /// Fetch UTXO set info with hash_type="none" (faster, skips hash computation).
    pub async fn get_txout_set_info(&self) -> Result<TxoutSetInfo, StatsError> {
        // gettxoutsetinfo scans the entire UTXO set (~165M entries).
        // Takes 60-120s on fast hardware, up to 5-10 min on a Raspberry Pi.
        let long_client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| StatsError::Rpc(e.to_string()))?;
        let body = serde_json::json!({
            "jsonrpc": "1.0",
            "id": "utxo",
            "method": "gettxoutsetinfo",
            "params": [json!("none")]
        });
        let resp = long_client
            .post(&self.url)
            .basic_auth(&self.user, Some(&self.password))
            .json(&body)
            .send()
            .await
            .map_err(|e| StatsError::Rpc(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(StatsError::Rpc(format!(
                "RPC returned status {}",
                resp.status()
            )));
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| StatsError::Rpc(e.to_string()))?;
        serde_json::from_value(json["result"].clone())
            .map_err(|e| StatsError::Rpc(e.to_string()))
    }

    /// Fetch a transaction by txid using getrawtransaction with verbose=true.
    /// Requires txindex=1 on the node. Returns the full decoded tx JSON.
    pub async fn get_raw_transaction(
        &self,
        txid: &str,
    ) -> Result<Value, StatsError> {
        self.call("getrawtransaction", &[json!(txid), json!(true)])
            .await
    }

    /// Fetch BTC/USD price from mempool.space API.
    pub async fn fetch_price(&self) -> Result<PriceInfo, StatsError> {
        let resp = self
            .client
            .get("https://mempool.space/api/v1/prices")
            .send()
            .await?;
        let price: PriceInfo = resp.json().await?;
        Ok(price)
    }

    /// Fetch full historical daily BTC/USD prices from blockchain.info.
    /// Returns Vec of (timestamp_ms, price_usd) covering all available history.
    pub async fn fetch_price_history_all(
        &self,
    ) -> Result<Vec<(u64, f64)>, StatsError> {
        let url =
            "https://api.blockchain.info/charts/market-price?timespan=all&format=json";
        let resp = self.client.get(url).send().await?;

        if !resp.status().is_success() {
            return Err(StatsError::Rpc(format!(
                "blockchain.info returned {}",
                resp.status()
            )));
        }

        // Read as text and parse explicitly, rather than `resp.json()`.
        //
        // A 200 from this endpoint does not guarantee JSON: it can carry a
        // rate-limit notice, an interstitial page, or compressed bytes we did
        // not ask for. `resp.json()` collapses every one of those into
        // reqwest's "error decoding response body", which says nothing about
        // what arrived, and the caller then logs a generic 500. Diagnosing the
        // 2026-09-08 outage took several round trips for exactly that reason.
        // Quoting a prefix of the body makes the next failure self-explaining.
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("(none)")
            .to_string();
        let encoding = resp
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("identity")
            .to_string();
        let text = resp.text().await?;
        let body: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| {
                StatsError::Rpc(format!(
                    "blockchain.info body is not JSON ({e}); \
                     content-type={content_type} \
                     content-encoding={encoding} len={} \
                     first 200 chars: {:?}",
                    text.len(),
                    text.chars().take(200).collect::<String>()
                ))
            })?;
        let values = body["values"].as_array().ok_or_else(|| {
            StatsError::Rpc(format!(
                "blockchain.info JSON has no values array; top-level keys: {:?}",
                body.as_object()
                    .map(|o| o.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default()
            ))
        })?;

        let result: Vec<(u64, f64)> = values
            .iter()
            .filter_map(|p| {
                let obj = p.as_object()?;
                let ts = obj.get("x")?.as_u64()?;
                let price = obj.get("y")?.as_f64()?;
                Some((ts * 1000, price))
            })
            .collect();

        Ok(result)
    }
}

#[cfg(test)]
mod raw_parity {
    use super::*;
    use bitcoin::ScriptBuf;

    /// Heights chosen for the awkward cases, not for coverage.
    ///
    /// A uniform sample is almost all modern P2WPKH-heavy blocks and would
    /// miss every format transition. These are the places where a parser is
    /// most likely to disagree with Core: before BIP34 put height in the
    /// coinbase, the 2015 stress test, the first SegWit and Taproot blocks,
    /// the first inscription and the first Runes block, either side of the
    /// OP_RETURN rule change at ~942,000, and the largest block in the chain.
    const HEIGHTS: &[u64] = &[
        1, 57043, 170000, 227835, 227836, 227837, 278000, 364292, 367853,
        481823, 481824, 481825, 709631, 709632, 709633, 767430, 775000, 836964,
        839842, 840000, 892279, 900000, 930000, 941999, 942000, 942001, 960000,
        968000,
        // Recent blocks, and not for coverage. Without these the sample held
        // no pay-to-anchor outputs and no OP_RETURN above 83 bytes, so
        // `p2a_count`, `op_return_max_bytes` and `op_return_over_80_count`
        // compared zero against zero and asserted nothing. Both phenomena
        // post-date 968,000.
        969000, 970000, 970500, 970800,
        // Added 2026-10-11 after an independent review found a classifier
        // disagreement the 28-block sample could not see. 181,732 carries
        // three outputs that embed text where a public key belongs, which
        // Core calls `nonstandard` and a shape-only check called `pubkey`.
        // 244,591 carries a 137-byte OP_RETURN, which disproves the claim
        // that 83 bytes was an absolute ceiling before v30.
        181732, 244591,
        // Broader era coverage, since the sample was thin between 2012 and
        // 2015 and held almost no P2PK or bare multisig.
        1000, 120000, 200000, 230000, 300000, 350000, 400000, 500000, 600000,
        650000, 800000, 850000, 880000,
    ];

    fn rpc_from_env() -> Option<BitcoinRpc> {
        let url = std::env::var("BITCOIN_STATS_RPC_URL").ok()?;
        let user = std::env::var("BITCOIN_STATS_RPC_USER").ok()?;
        let pass = std::env::var("BITCOIN_STATS_RPC_PASSWORD").ok()?;
        // Generous, because the verbosity-2 side of the comparison is the
        // slow one: the largest blocks exceed the production 30s timeout on
        // their own.
        Some(BitcoinRpc::with_timeout(
            url,
            user,
            pass,
            Duration::from_secs(300),
        ))
    }

    /// Fields a raw block cannot carry, because Core derives them from its
    /// undo data rather than from the block. Compared and reported, never
    /// asserted, and a sweep over this path must not write their columns.
    fn fee_derived(name: &str) -> bool {
        matches!(
            name,
            "total_fees"
                | "median_fee"
                | "median_fee_rate"
                | "fee_rate_p10"
                | "fee_rate_p25"
                | "fee_rate_p75"
                | "fee_rate_p90"
                | "max_tx_fee"
                | "inscription_fees"
                | "runes_fees"
                | "total_input_value"
        )
    }

    /// Every field of two `Block`s, as (name, json) pairs, so the comparison
    /// is exhaustive by construction. Adding a column to `Block` makes it
    /// compared here without anyone remembering to.
    fn fields(b: &Block) -> Vec<(String, Value)> {
        let v = serde_json::to_value(b).expect("Block serializes");
        match v {
            Value::Object(m) => m.into_iter().collect(),
            _ => panic!("Block is not an object"),
        }
    }

    /// Independent oracle: the new columns computed from the node by a
    /// second implementation, compared against what the parser produces.
    ///
    /// `raw_matches_json` cannot do this job. For nine of the ten new columns
    /// both of its paths run the same `parse_block_value`, so a wrong
    /// threshold or a wrong population is wrong identically on both sides and
    /// passes green. The fixture is built by a Python implementation written
    /// from each column's definition rather than from this code, and carries
    /// both population variants so the coinbase question is visible rather
    /// than assumed.
    ///
    /// ```text
    /// cargo test --features ssr oracle_new_columns -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore]
    async fn oracle_new_columns() {
        let rpc = rpc_from_env().expect("env");
        let path = std::env::var("ORACLE_FIXTURE")
            .unwrap_or_else(|_| "/tmp/claude-1000/oracle.json".into());
        let raw = std::fs::read_to_string(&path).expect("oracle fixture");
        let fx: Value = serde_json::from_str(&raw).expect("fixture json");

        // This gate passed on a fixture of `{}` until 2026-10-11, checking
        // zero heights and zero values, and it accepted *either* population
        // for every field, which is the exact regression it exists to catch.
        // An independent review demonstrated both. The floor and the required
        // heights below are what stop it being vacuous again.
        let n = fx.as_object().map(|o| o.len()).unwrap_or(0);
        assert!(
            n >= 10,
            "oracle fixture holds {n} heights; too few to assert anything"
        );
        for required in ["1", "57043", "970800"] {
            assert!(
                fx.get(required).is_some(),
                "oracle fixture is missing height {required}, which is the \
                 case that gives this gate its meaning"
            );
        }

        println!(
            "{:>8} {:>22} {:>12} {:>12} {:>12}",
            "height", "column", "ours", "oracle_nocb", "oracle_all"
        );
        // Fields whose population is every output, coinbase included. The
        // rest are non-coinbase by design; see the parser for each decision.
        const ALL_OUTPUT_FIELDS: &[&str] =
            &["p2a_count", "p2pk_value", "p2tr_value", "multisig_value"];

        let mut disagree_both = Vec::new();
        for (h, v) in fx.as_object().expect("obj") {
            let height: u64 = h.parse().unwrap();
            let hash = rpc.get_block_hash(height).await.unwrap();
            let b = rpc.get_block_fresh(&hash).await.unwrap();
            let pairs: &[(&str, u64, &str)] = &[
                ("p2a_count", b.p2a_count, "p2a"),
                ("op_return_max_bytes", b.op_return_max_bytes, "or_max"),
                (
                    "op_return_over_80_count",
                    b.op_return_over_80_count,
                    "or_gt83",
                ),
                ("tx_v1_count", b.tx_v1_count, "v1"),
                ("tx_v2_count", b.tx_v2_count, "v2"),
                ("tx_v3_count", b.tx_v3_count, "v3"),
                ("p2pk_value", b.p2pk_value, "p2pk_val"),
                ("p2tr_value", b.p2tr_value, "p2tr_val"),
                ("multisig_value", b.multisig_value, "ms_val"),
                ("brc20_bytes", b.brc20_bytes, "brc20_bytes"),
            ];
            for (name, ours, key) in pairs {
                // Missing keys must not read as zero, or a fixture that
                // simply lacks a field silently agrees with everything.
                let nocb = v["nocb"][key].as_u64().unwrap_or_else(|| {
                    panic!("fixture height {height} has no nocb.{key}")
                });
                let all = v["all"][key].as_u64().unwrap_or_else(|| {
                    panic!("fixture height {height} has no all.{key}")
                });
                if *ours != nocb || nocb != all {
                    println!("{height:>8} {name:>22} {ours:>12} {nocb:>12} {all:>12}");
                }
                // One population per field, named here, rather than "either
                // will do". `p2pk_value` matching `nocb` is precisely the bug
                // that shipped: 0 where the answer was 50 BTC.
                let want = if ALL_OUTPUT_FIELDS.contains(name) {
                    all
                } else {
                    nocb
                };
                if *ours != want {
                    disagree_both.push(format!(
                        "height {height} {name}: ours={ours} expected={want} \
                         (nocb={nocb} all={all})"
                    ));
                }
            }
        }
        println!("\nrows above are where ours != nocb, or the two populations differ");
        assert!(
            disagree_both.is_empty(),
            "parser disagrees with the population each field is supposed \
             to measure:\n  {}",
            disagree_both.join("\n  ")
        );
    }

    /// Our script classifier must agree with Core's, script for script.
    ///
    /// The spike this replaces sampled 2,459 outputs and **printed** its
    /// disagreement count rather than asserting it, so it could not fail. Its
    /// fixture also held only three `pubkey` scripts, which is why it missed
    /// that `rust-bitcoin`'s `is_p2pk` checks shape without validating the
    /// key header: the 2012-era outputs embedding text where a key belongs
    /// match the shape exactly and Core calls them `nonstandard`.
    ///
    /// The fixture is (script hex, Core's label) from real blocks spanning
    /// 2009 to 2026, weighted toward the rare types rather than uniformly,
    /// plus adversarial scripts labelled by Core's own `decodescript`.
    ///
    /// ```text
    /// cargo test --features ssr the_classifier_agrees_with_core -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore]
    fn the_classifier_agrees_with_core() {
        use bitcoin::hex::FromHex;
        use std::collections::BTreeMap;

        let path = std::env::var("SCRIPT_ORACLE")
            .unwrap_or_else(|_| "/tmp/claude-1000/script_oracle.json".into());
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("fixture {path}: {e}"));
        let rows: Vec<Value> =
            serde_json::from_str(&raw).expect("fixture json");
        assert!(
            rows.len() > 500,
            "fixture holds only {} scripts; too small to mean anything",
            rows.len()
        );

        // Core labels we do not model, every one of which our `_` arm sends
        // to `unknown_script_count` exactly as Core's would. Agreeing on the
        // bucket is what matters, not on the string.
        let unmodelled = |t: &str| {
            matches!(t, "nonstandard" | "witness_unknown" | "witness_v1_anchor")
        };

        let mut agree: BTreeMap<String, u64> = BTreeMap::new();
        let mut bad: Vec<String> = Vec::new();
        for r in &rows {
            let core = r["type"].as_str().unwrap_or("?");
            let hex = r["hex"].as_str().unwrap_or("");
            let bytes = Vec::<u8>::from_hex(hex).expect("fixture hex");
            let script = ScriptBuf::from_bytes(bytes);
            let mine = script_type(&script);
            let ok = mine == core || (unmodelled(core) && mine == "other");
            if ok {
                *agree.entry(core.to_string()).or_default() += 1;
            } else {
                bad.push(format!("core={core:<18} ours={mine:<18} {hex}"));
            }
        }
        println!("\nagreed:");
        for (t, n) in &agree {
            println!("  {n:6}  {t}");
        }
        assert!(
            bad.is_empty(),
            "{} of {} scripts classified differently from Core:\n  {}",
            bad.len(),
            rows.len(),
            bad.join("\n  ")
        );
        println!("\n{} scripts, zero disagreements", rows.len());
    }

    /// Coinbase outputs are classified, and the classified types plus
    /// OP_RETURN account for every output in a block.
    ///
    /// Two properties, checked against the node rather than against
    /// ourselves. The early chain is the case that matters: block 1 is a
    /// single P2PK coinbase output and used to report nothing at all.
    #[tokio::test]
    #[ignore]
    async fn the_classified_types_account_for_every_output() {
        let rpc = rpc_from_env().expect("env");
        for h in [1u64, 57043, 170000, 481824, 900000, 970800] {
            let hash = rpc.get_block_hash(h).await.unwrap();
            let b = rpc.get_block_fresh(&hash).await.unwrap();
            // Core's own count of OP_RETURN outputs, which sit inside
            // `output_count` but outside the nine classified types.
            let blk = rpc
                .call("getblock", &[json!(hash), json!(2)])
                .await
                .unwrap();
            let nulldata: u64 = blk["tx"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|t| t["vout"].as_array().unwrap())
                .filter(|v| {
                    v["scriptPubKey"]["type"].as_str() == Some("nulldata")
                })
                .count() as u64;
            let classified = b.p2pk_count
                + b.p2pkh_count
                + b.p2sh_count
                + b.p2wpkh_count
                + b.p2wsh_count
                + b.p2tr_count
                + b.multisig_count
                // `p2a_count` is deliberately absent: it is a subset of
                // `unknown_script_count` until the chart denominators carry
                // it, so adding it here would double-count anchors.
                + b.unknown_script_count;
            println!(
                "h={h:>7} output_count={:>6} classified={classified:>6}                  p2pk={:>5} p2a={:>5} unknown={:>5}",
                b.output_count, b.p2pk_count, b.p2a_count, b.unknown_script_count
            );
            assert_eq!(
                classified + nulldata,
                b.output_count,
                "height {h}: the nine classified types plus OP_RETURN outputs must account for every output in the block"
            );
        }
        // Anchors are a subset of the unknown bucket, not a sibling of it.
        // This is the invariant that makes the sum above correct, and it is
        // what must be removed when anchors become a ninth classified type.
        for h in [970800u64, 969000] {
            let hash = rpc.get_block_hash(h).await.unwrap();
            let b = rpc.get_block_fresh(&hash).await.unwrap();
            assert!(
                b.p2a_count > 0,
                "height {h} should carry anchors, or this asserts nothing"
            );
            assert!(
                b.p2a_count <= b.unknown_script_count,
                "height {h}: p2a_count {} exceeds unknown_script_count {}, \
                 so anchors are no longer a subset",
                b.p2a_count,
                b.unknown_script_count
            );
        }

        // Block 1 is one P2PK coinbase output and nothing else. Before
        // coinbase outputs were classified this reported zero.
        let hash = rpc.get_block_hash(1).await.unwrap();
        let b = rpc.get_block_fresh(&hash).await.unwrap();
        assert_eq!(b.p2pk_count, 1, "block 1 is a single P2PK coinbase output");
        assert_eq!(b.output_count, 1, "and it is the block's only output");
        assert_eq!(b.p2pk_value, 5_000_000_000, "worth the 50 BTC subsidy");
    }

    /// What the sweep will write, checked against the node before 970,000
    /// rows are rewritten.
    ///
    /// **This is the gate the sweep's purpose demanded and did not have.**
    /// `raw_matches_json` compares a fresh JSON parse against a fresh raw
    /// parse; both run `parse_block_value`, so both apply the *current* rule
    /// and agree. Nothing was checking that the current rule is the right one
    /// against the chain, nor what the sweep would do to the stored values.
    ///
    /// For every column whose meaning this change moves, the expectation is
    /// computed here from Core's own classification rather than from our
    /// parser, and asserted. The stored value is read alongside and reported,
    /// so the size and shape of the rewrite is visible in advance.
    ///
    /// ```text
    /// set -a; source .env; set +a
    /// cargo test --features ssr what_the_sweep_will_write -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore]
    async fn what_the_sweep_will_write() {
        let rpc = rpc_from_env().expect("env");
        let conn = rusqlite::Connection::open_with_flags(
            "bitcoin_stats.db",
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("bitcoin_stats.db");

        // Dense either side of the OP_RETURN rule boundary, measured between
        // 941,875 and 942,171, plus every era and the awkward blocks.
        let mut heights: Vec<u64> = vec![
            1, 1000, 57043, 120000, 170000, 181732, 200000, 244591, 278000,
            350000, 400000, 481824, 500000, 600000, 709632, 767430, 800000,
            840000, 900000, 930000, 960000, 968000, 970800,
        ];
        heights.extend((941_800..=942_300).step_by(50));
        // Named by the third independent review: 470,000 carries a coinbase
        // OP_RETURN that is `nonstandard` yet unspendable, and the other
        // three are the consensus exceptions where a coinbase adds no UTXO.
        heights.extend([0u64, 91_842, 91_880, 470_000]);
        heights.sort_unstable();

        // The witness commitment is an OP_RETURN the parser deliberately
        // never counts, in either rule.
        const COMMIT: &str = "6a24aa21a9ed";

        let mut mismatches: Vec<String> = Vec::new();
        let mut op_return_delta = 0i64;
        let mut output_delta = 0i64;
        let mut rows = 0u64;

        for h in heights {
            let hash = rpc.get_block_hash(h).await.unwrap();
            let blk = rpc
                .call("getblock", &[json!(hash), json!(2)])
                .await
                .unwrap();
            let got = rpc.get_block_raw(&hash, h).await.unwrap();

            // ---- expectation, from Core's labels only ----
            let txs = blk["tx"].as_array().unwrap();
            let mut want: std::collections::BTreeMap<&str, u64> =
                Default::default();
            let mut want_or_count = 0u64;
            let mut want_or_bytes = 0u64;
            for (i, tx) in txs.iter().enumerate() {
                for vo in tx["vout"].as_array().unwrap() {
                    let spk = &vo["scriptPubKey"];
                    let ty = spk["type"].as_str().unwrap_or("?");
                    let hex = spk["hex"].as_str().unwrap_or("");
                    *want.entry("output_count").or_default() += 1;
                    match ty {
                        "pubkey" => *want.entry("p2pk").or_default() += 1,
                        "pubkeyhash" => *want.entry("p2pkh").or_default() += 1,
                        "scripthash" => *want.entry("p2sh").or_default() += 1,
                        "witness_v0_keyhash" => {
                            *want.entry("p2wpkh").or_default() += 1
                        }
                        "witness_v0_scripthash" => {
                            *want.entry("p2wsh").or_default() += 1
                        }
                        "witness_v1_taproot" => {
                            *want.entry("p2tr").or_default() += 1
                        }
                        "multisig" => *want.entry("multisig").or_default() += 1,
                        // Anchors are still a subset of the unknown bucket.
                        "anchor" => {
                            *want.entry("p2a").or_default() += 1;
                            *want.entry("unknown").or_default() += 1;
                        }
                        "nulldata" => {
                            // Non-coinbase only, commitment excluded: the
                            // rule the sweep exists to apply everywhere.
                            if i > 0 && !hex.starts_with(COMMIT) {
                                want_or_count += 1;
                                want_or_bytes += (hex.len() / 2) as u64;
                            }
                        }
                        _ => *want.entry("unknown").or_default() += 1,
                    }
                }
            }
            let g = |k: &str| want.get(k).copied().unwrap_or(0);

            // UTXO growth, compared against Core's own answer.
            //
            // This previously recomputed `outputs - nulldata - inputs` from
            // Core's labels, which is the same wrong spendability assumption
            // the code had, so the gate agreed with the bug. An independent
            // review passed height 470,000 through it unchanged despite a
            // proven disagreement, and made all five gates pass an erroneous
            // parser by doubling one column and subtracting it from another,
            // because only the combination was asserted.
            //
            // So: ask `getblockstats`, and assert the inputs to the formula
            // independently as well as its result.
            let stats = rpc.call("getblockstats", &[json!(h)]).await.unwrap();
            let core_utxo = stats["utxo_increase_actual"].as_i64().unwrap();

            let want_unspendable: u64 = txs
                .iter()
                .flat_map(|t| t["vout"].as_array().unwrap())
                .filter(|v| {
                    v["scriptPubKey"]["hex"]
                        .as_str()
                        .map(is_unspendable)
                        .unwrap_or(false)
                })
                .count() as u64;
            let want_spent: u64 = txs
                .iter()
                .skip(1)
                .map(|t| t["vin"].as_array().unwrap().len() as u64)
                .sum();

            // Each component on its own, so two errors cannot cancel.
            for (name, ours, expected) in [
                (
                    "unspendable_output_count",
                    got.unspendable_output_count,
                    want_unspendable,
                ),
                ("input_count", got.input_count, want_spent),
            ] {
                if ours != expected {
                    mismatches.push(format!(
                        "height {h:>7} {name:<22} parser={ours} node={expected}"
                    ));
                }
            }

            const NO_UTXO_FROM_COINBASE: &[u64] = &[0, 91_842, 91_880];
            let adjust = i64::from(NO_UTXO_FROM_COINBASE.contains(&h))
                * got.coinbase_output_count as i64;
            let ours_utxo = got.output_count as i64
                - got.unspendable_output_count as i64
                - got.input_count as i64
                - adjust;
            if ours_utxo != core_utxo {
                mismatches.push(format!(
                    "height {h:>7} utxo_increase_actual   formula={ours_utxo} core={core_utxo}"
                ));
            }

            let checks: &[(&str, u64, u64)] = &[
                ("output_count", got.output_count, g("output_count")),
                ("p2pk_count", got.p2pk_count, g("p2pk")),
                ("p2pkh_count", got.p2pkh_count, g("p2pkh")),
                ("p2sh_count", got.p2sh_count, g("p2sh")),
                ("p2wpkh_count", got.p2wpkh_count, g("p2wpkh")),
                ("p2wsh_count", got.p2wsh_count, g("p2wsh")),
                ("p2tr_count", got.p2tr_count, g("p2tr")),
                ("multisig_count", got.multisig_count, g("multisig")),
                (
                    "unknown_script_count",
                    got.unknown_script_count,
                    g("unknown"),
                ),
                ("p2a_count", got.p2a_count, g("p2a")),
                ("op_return_count", got.op_return_count, want_or_count),
                ("op_return_bytes", got.op_return_bytes, want_or_bytes),
                // Serialized witness bytes, derived from Core's own size and
                // weight rather than from the witness arrays it reports.
                // `size - (weight - size) / 3` is the witness portion by
                // definition, and it counts the empty stacks Core omits from
                // `txinwitness`.
                (
                    "witness_bytes",
                    got.witness_bytes,
                    txs.iter()
                        .skip(1)
                        .map(|t| {
                            let sz = t["size"].as_u64().unwrap_or(0);
                            let wt = t["weight"].as_u64().unwrap_or(0);
                            sz.saturating_sub(wt.saturating_sub(sz) / 3)
                        })
                        .sum::<u64>(),
                ),
                // A coinbase is a transaction, so it counts toward the
                // block's largest.
                (
                    "largest_tx_size",
                    got.largest_tx_size,
                    txs.iter()
                        .map(|t| t["size"].as_u64().unwrap_or(0))
                        .max()
                        .unwrap_or(0),
                ),
                (
                    "coinbase_output_count",
                    got.coinbase_output_count,
                    txs[0]["vout"].as_array().unwrap().len() as u64,
                ),
            ];
            for (name, ours, expected) in checks {
                if ours != expected {
                    mismatches.push(format!(
                        "height {h:>7} {name:<22} parser={ours} node={expected}"
                    ));
                }
            }

            // ---- what the rewrite does to the stored row ----
            let stored: (i64, i64, i64) = conn
                .query_row(
                    "SELECT op_return_count, output_count, backfill_version \
                     FROM blocks WHERE height = ?1",
                    [h],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap_or((-1, -1, -1));
            if stored.0 >= 0 {
                rows += 1;
                op_return_delta += got.op_return_count as i64 - stored.0;
                output_delta += got.output_count as i64 - stored.1;
            }
        }

        println!(
            "\nsampled {rows} stored rows\n  op_return_count total change: \
             {op_return_delta:+}\n  output_count total change:    {output_delta:+}"
        );
        assert!(
            mismatches.is_empty(),
            "the parser disagrees with the node on {} value(s); the sweep \
             would write these:\n  {}",
            mismatches.len(),
            mismatches.join("\n  ")
        );
    }

    /// A bounded dry run of the sweep, against a copy of the real database.
    ///
    /// Everything else proves the raw writer's SQL *cannot* reach the fee
    /// columns. This proves the writer *does not*, by running it and reading
    /// the rows back. It also exercises `init_schema` against the schema
    /// actually on disk, which is the only way the dead daily migration was
    /// visible: a freshly created database takes the CREATE TABLE path and
    /// looks fine.
    ///
    /// Copy the database first; this writes.
    ///
    /// ```text
    /// cp bitcoin_stats.db "$TMPDIR/dry.db"
    /// set -a; source .env; set +a
    /// SWEEP_DRY_DB="$TMPDIR/dry.db" \
    ///   cargo test --features ssr the_sweep_dry_run -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore]
    async fn the_sweep_dry_run() {
        let path = match std::env::var("SWEEP_DRY_DB") {
            Ok(p) => p,
            Err(_) => panic!("set SWEEP_DRY_DB to a COPY of the database"),
        };
        assert!(
            !path.ends_with("bitcoin_stats.db"),
            "refusing to run against what looks like the live database"
        );
        let rpc = rpc_from_env().expect("env");

        // `open_pool` runs `init_schema`, so the migrations are under test
        // here too, against a real schema rather than an empty one.
        let pool = crate::stats::db::open_pool(std::path::Path::new(&path), 2)
            .expect("open the copy");
        let conn = pool.get().expect("conn");

        // Rows the sweep will actually change, not whichever come first.
        //
        // `heights_needing_backfill` returns newest-first, and the newest
        // rows here already carried post-sweep values: written at v13
        // semantics by a server start, then reset to v12. Sweeping them was a
        // no-op, so "fees preserved" and "rollup refreshed" were both
        // satisfied by nothing happening. Picking rows still on the old
        // semantics is what makes this test mean anything.
        let heights: Vec<u64> = {
            let mut st = conn
                .prepare(
                    "SELECT height FROM blocks WHERE backfill_version <= 11 \
                     ORDER BY height DESC LIMIT 40",
                )
                .unwrap();
            st.query_map([], |r| r.get::<_, u64>(0))
                .unwrap()
                .filter_map(Result::ok)
                .collect()
        };
        let outputs_before: Vec<(u64, u64)> = heights
            .iter()
            .map(|h| {
                let v: u64 = conn
                    .query_row(
                        "SELECT output_count FROM blocks WHERE height = ?1",
                        [*h],
                        |r| r.get(0),
                    )
                    .unwrap();
                (*h, v)
            })
            .collect();
        assert!(
            heights.len() >= 20,
            "expected work to do, got {}",
            heights.len()
        );

        // Fee columns before, for exactly the rows about to be rewritten.
        let fee_cols = "median_fee, median_fee_rate, fee_rate_p10, fee_rate_p25, \
                        fee_rate_p75, fee_rate_p90, max_tx_fee, inscription_fees, \
                        runes_fees";
        let snapshot = |c: &rusqlite::Connection, h: u64| -> Vec<f64> {
            c.query_row(
                &format!("SELECT {fee_cols} FROM blocks WHERE height = ?1"),
                [h],
                |r| {
                    Ok((0..9)
                        .map(|i| r.get::<_, f64>(i).unwrap_or(-1.0))
                        .collect::<Vec<f64>>())
                },
            )
            .unwrap_or_default()
        };
        let before: Vec<(u64, Vec<f64>)> =
            heights.iter().map(|h| (*h, snapshot(&conn, *h))).collect();
        let nonzero_fees = before
            .iter()
            .filter(|(_, v)| v.iter().any(|x| *x > 0.0))
            .count();
        assert!(
            nonzero_fees >= 5,
            "only {nonzero_fees} sampled rows carry fee data, so preserving \
             it would prove nothing"
        );

        // Fetch and write exactly as the sweep does.
        let mut blocks = Vec::new();
        for h in &heights {
            blocks.push(
                rpc.fetch_block_raw_by_height(*h).await.expect("raw fetch"),
            );
        }
        for b in &blocks {
            assert_eq!(
                b.median_fee, 0,
                "raw blocks carry no fees by definition"
            );
        }
        crate::stats::db::update_block_extras_raw(&conn, &blocks)
            .expect("the raw writer");

        // Fees survived, version moved, structure was rewritten.
        let mut preserved = 0;
        for (h, was) in &before {
            let now = snapshot(&conn, *h);
            assert_eq!(
                *was, now,
                "height {h}: the sweep changed fee data it must not touch"
            );
            preserved += 1;
        }
        for h in &heights {
            let v: u64 = conn
                .query_row(
                    "SELECT backfill_version FROM blocks WHERE height = ?1",
                    [*h],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(
                v,
                crate::stats::db::BACKFILL_VERSION,
                "height {h} was not stamped as swept"
            );
        }
        let written: u64 = conn
            .query_row(
                "SELECT COUNT(*) FROM blocks WHERE height IN \
                 (SELECT height FROM blocks WHERE backfill_version = ?1)",
                [crate::stats::db::BACKFILL_VERSION],
                |r| r.get(0),
            )
            .unwrap();
        // The sweep must have actually changed something, or everything
        // below is satisfied by a no-op.
        let changed = outputs_before
            .iter()
            .filter(|(h, was)| {
                let now: u64 = conn
                    .query_row(
                        "SELECT output_count FROM blocks WHERE height = ?1",
                        [*h],
                        |r| r.get(0),
                    )
                    .unwrap();
                now != *was
            })
            .count();
        assert!(
            changed >= heights.len() / 2,
            "only {changed} of {} rows changed output_count; the sample is \
             already swept and this test asserts nothing",
            heights.len()
        );

        // The daily rollup must have been refreshed inside the writer's
        // transaction. Removing that call passed every gate until a review
        // demonstrated it, because nothing read daily_blocks back.
        let days: Vec<String> = {
            let mut st = conn
                .prepare(
                    "SELECT DISTINCT date(timestamp,'unixepoch') FROM blocks \
                     WHERE height IN (SELECT value FROM json_each(?1))",
                )
                .unwrap();
            let ids = serde_json::to_string(&heights).unwrap();
            let rows = st
                .query_map([ids], |r| r.get::<_, String>(0))
                .unwrap()
                .filter_map(Result::ok)
                .collect();
            rows
        };
        assert!(!days.is_empty(), "swept blocks resolved to no days");
        for day in &days {
            let (blocks_in_day, rolled): (u64, f64) = conn
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM blocks WHERE \
                     date(timestamp,'unixepoch') = ?1), \
                     (SELECT avg_output_count FROM daily_blocks WHERE day = ?1)",
                    [day],
                    |r| Ok((r.get(0)?, r.get::<_, Option<f64>>(1)?.unwrap_or(-1.0))),
                )
                .unwrap_or((0, -1.0));
            let direct: f64 = conn
                .query_row(
                    "SELECT AVG(output_count) FROM blocks WHERE \
                     date(timestamp,'unixepoch') = ?1",
                    [day],
                    |r| r.get::<_, Option<f64>>(0).map(|v| v.unwrap_or(-1.0)),
                )
                .unwrap_or(-1.0);
            assert!(
                (rolled - direct).abs() < 0.001,
                "day {day}: daily_blocks holds avg_output_count {rolled} \
                 against {direct} aggregated directly from {blocks_in_day} \
                 blocks, so the rollup did not refresh"
            );
        }

        println!(
            "\ndry run: {} blocks swept, fee data preserved on {preserved} rows \
             ({nonzero_fees} of them carrying real fees), {written} rows now at \
             version {}, {} daily rollups verified against direct aggregation",
            heights.len(),
            crate::stats::db::BACKFILL_VERSION,
            days.len()
        );
    }

    /// A reorg cannot graft stale-fork data onto a canonical row.
    ///
    /// The sweep fetches by height and writes minutes later. If that height
    /// reorgs in between, `verify_recent_blocks` replaces the row, and a
    /// height-only WHERE would paste the stale fork's metrics onto it:
    /// canonical hash, canonical fees, wrong structure, stamped as swept, and
    /// invisible to the hash check because the hash was never wrong.
    ///
    /// Simulated rather than argued: take a real block, change the stored
    /// hash to stand for a reorged replacement, run the writer, and assert it
    /// touched nothing.
    #[tokio::test]
    #[ignore]
    async fn a_reorg_cannot_corrupt_a_swept_row() {
        let path = std::env::var("SWEEP_DRY_DB")
            .expect("set SWEEP_DRY_DB to a COPY of the database");
        assert!(
            !path.ends_with("bitcoin_stats.db"),
            "refusing the live database"
        );
        let rpc = rpc_from_env().expect("env");
        let pool = crate::stats::db::open_pool(std::path::Path::new(&path), 2)
            .expect("open the copy");
        let conn = pool.get().expect("conn");

        let h: u64 = conn
            .query_row("SELECT MAX(height) FROM blocks", [], |r| r.get(0))
            .unwrap();
        let block = rpc.fetch_block_raw_by_height(h).await.expect("fetch");

        // Stand in for a reorg: the row at this height now holds a different
        // block, exactly as it would after verify_recent_blocks replaced it.
        conn.execute(
            "UPDATE blocks SET hash = ?1, backfill_version = 0, output_count = \
             999999 WHERE height = ?2",
            rusqlite::params!["0000reorged", h],
        )
        .unwrap();

        crate::stats::db::update_block_extras_raw(&conn, &[block])
            .expect("writer must not error, only decline to match");

        let (ver, outs): (u64, u64) = conn
            .query_row(
                "SELECT backfill_version, output_count FROM blocks WHERE height = ?1",
                [h],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            outs, 999_999,
            "the writer overwrote a row whose hash had changed under it"
        );
        assert_eq!(
            ver, 0,
            "the row was stamped as swept despite not being written, so no \
             later pass would ever revisit it"
        );
        println!("\nreorged row at {h} left untouched and still eligible");
    }

    /// The three fixes made on 2026-10-11 after the fifth review, each
    /// pinned so reverting it fails here.
    ///
    /// Two of that review's four findings were defects in fixes from the
    /// review before it. Fixes need tests as much as features do.
    mod fifth_review_fixes {
        use super::*;

        /// The sweep resolves hashes fresh, not through the cache.
        ///
        /// A `getblockhash` issued before a reorg can land after
        /// `invalidate_block_caches` and reinstall the orphan. The sweep
        /// would then fetch the orphan, match the not-yet-repaired row, and
        /// stamp it swept. Poisoning the cache here stands in for that.
        #[tokio::test]
        #[ignore]
        async fn the_sweep_does_not_trust_the_hash_cache() {
            let rpc = rpc_from_env().expect("env");
            let h: u64 = 800_000;
            let canonical = rpc.get_block_hash_fresh(h).await.unwrap();

            // Warm the cache, then poison it the way a late response would.
            let _ = rpc.get_block_hash(h).await.unwrap();
            rpc.poison_block_hash_cache_for_test(h, "deadbeef".repeat(8));

            let got = rpc.fetch_block_raw_by_height(h).await.expect("fetch");
            assert_eq!(
                got.hash, canonical,
                "the sweep read a poisoned cache entry instead of asking the \
                 node, so a reorg could hand it an orphan block"
            );
        }

        /// A sweep against an unreachable node stops instead of spinning.
        ///
        /// The earlier guard only fired when a batch fetched blocks and wrote
        /// none. With every fetch failing, `blocks` is empty, the guard was
        /// skipped, and the loop retried with no backoff: a controlled
        /// responder saw 893 requests in 250ms.
        #[tokio::test]
        #[ignore]
        async fn a_dead_node_stops_the_sweep_rather_than_spinning() {
            let path = std::env::var("SWEEP_DRY_DB")
                .expect("set SWEEP_DRY_DB to a COPY of the database");
            assert!(!path.ends_with("bitcoin_stats.db"), "refusing live db");
            let pool =
                crate::stats::db::open_pool(std::path::Path::new(&path), 2)
                    .expect("open copy");
            // Port 1 refuses instantly, so every fetch fails.
            let dead = BitcoinRpc::new(
                "http://127.0.0.1:1/".into(),
                "x".into(),
                "x".into(),
            );
            let start = std::time::Instant::now();
            tokio::time::timeout(
                std::time::Duration::from_secs(120),
                crate::stats::ingest::backfill_extras(&dead, &pool),
            )
            .await
            .expect("the sweep must return, not spin until killed");
            println!(
                "\nsweep against a dead node returned in {:.1}s",
                start.elapsed().as_secs_f64()
            );
        }
    }

    /// The raw path must agree with the verbosity-2 path on every column a
    /// sweep would write.
    ///
    /// ```text
    /// set -a; source .env; set +a
    /// cargo test --features ssr raw_matches_json -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore]
    async fn raw_matches_json() {
        let Some(rpc) = rpc_from_env() else {
            panic!(
                "set BITCOIN_STATS_RPC_URL/USER/PASSWORD; source .env first"
            );
        };

        let mut mismatches: Vec<String> = Vec::new();
        let mut fee_gap: std::collections::BTreeSet<String> =
            Default::default();
        let mut checked = 0u64;

        for &h in HEIGHTS {
            let hash = rpc.get_block_hash(h).await.expect("getblockhash");
            let from_json =
                rpc.get_block_fresh(&hash).await.expect("verbosity 2");
            let from_raw =
                rpc.get_block_raw(&hash, h).await.expect("verbosity 0");

            for ((n1, v1), (n2, v2)) in
                fields(&from_json).into_iter().zip(fields(&from_raw))
            {
                assert_eq!(n1, n2, "field order diverged");
                checked += 1;
                if v1 == v2 {
                    continue;
                }
                if fee_derived(&n1) {
                    fee_gap.insert(n1);
                    continue;
                }
                // `difficulty` is compared within a few ULP, because our
                // own JSON parse is the inexact step.
                //
                // Probed 2026-10-10 at block 481,823. Core reports
                // 923233068448.9053 identically from getblockheader,
                // getblock verbosity 1 and verbosity 2, and
                // `core_difficulty` reproduces that double bit for bit. But
                // `serde_json` parses that text to 0x426ade9f54b41cf7 where
                // `str::parse::<f64>` gives 0x426ade9f54b41cf8, one ULP
                // higher and correct:
                //
                //     serde_json::from_str::<f64>("923233068448.9053")
                //         != "923233068448.9053".parse::<f64>()
                //
                // So the verbosity-2 path carries a value one ULP below the
                // truth and the raw path is the more accurate of the two.
                // An earlier version of this comment blamed Core for
                // disagreeing with itself across two RPCs. That was wrong: it
                // compared a string slice of the wire text against a parsed
                // value and charged our own rounding to the node.
                //
                // 1e-15 is about four ULP here, far below anything a chart
                // renders, and no sweep writes this column.
                if n1 == "difficulty" {
                    if let (Some(a), Some(b)) = (v1.as_f64(), v2.as_f64()) {
                        let rel =
                            (a - b).abs() / a.abs().max(f64::MIN_POSITIVE);
                        if rel <= 1e-15 {
                            continue;
                        }
                    }
                }
                mismatches.push(format!(
                    "height {h:>7}  {n1:<28} json={v1}  raw={v2}"
                ));
            }
        }

        println!(
            "\ncompared {checked} field values across {} blocks",
            HEIGHTS.len()
        );
        println!(
            "fee-derived fields differing as expected: {}",
            fee_gap.iter().cloned().collect::<Vec<_>>().join(", ")
        );
        if !mismatches.is_empty() {
            println!("\n--- MISMATCHES ---");
            for m in &mismatches {
                println!("{m}");
            }
        }
        assert!(
            mismatches.is_empty(),
            "{} field(s) differ between the raw and JSON paths",
            mismatches.len()
        );
    }
}
