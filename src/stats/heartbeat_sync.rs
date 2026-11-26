//! Shared Phase 2 Heartbeat sync protocol and pure client mirror.
//!
//! This module intentionally depends only on `std`, `serde`, and `serde_json`
//! so the exact same wire types and state reducer compile cleanly into both
//! the SSR server binary and the `wasm32-unknown-unknown` hydrate bundle.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;

use serde::de::{self, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Wire protocol version negotiated on every snapshot, delta, and resync frame.
pub const PROTOCOL_VERSION: u32 = 1;

/// Maximum number of canonical recent blocks retained by [`HeartbeatMirror`].
pub const MAX_RECENT_BLOCKS: usize = 144;

/// Maximum number of confirmed txid entries retained in the mirror's FIFO cache
/// (~3-4 mainnet blocks, ~150-350 KB gzipped when carried in a snapshot).
///
/// The companion `confirmed_order` queue is compacted whenever its length
/// exceeds `confirmed.len() + MAX_CONFIRMED_HISTORY`, bounding its worst-case
/// length to `<= 2 * MAX_CONFIRMED_HISTORY`.
pub const MAX_CONFIRMED_HISTORY: usize = 10_000;

/// Lossless decimal-string `u64` serde helper for JSON boundaries.
///
/// JavaScript `Number` loses integer precision above `2^53 - 1`, so revision
/// and sequence fields (`seq`, `latest_seq`) cross JSON as canonical decimal
/// strings while staying native `u64` in Rust.
pub mod u64_str {
    use std::fmt;

    use serde::de::{self, Visitor};
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct U64StrVisitor;

        impl Visitor<'_> for U64StrVisitor {
            type Value = u64;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(
                    "a decimal string encoding an unsigned 64-bit integer",
                )
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(E::custom(format!(
                        "invalid u64 decimal string: {v:?}"
                    )));
                }
                v.parse::<u64>()
                    .map_err(|_| E::custom(format!("u64 out of range: {v:?}")))
            }
        }

        deserializer.deserialize_str(U64StrVisitor)
    }
}

/// Completeness of the server's observed mempool membership relative to Core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Coverage {
    Complete,
    Partial { reason: String },
    Unknown { reason: String },
}

impl Default for Coverage {
    fn default() -> Self {
        Self::Unknown {
            reason: "uninitialized".to_string(),
        }
    }
}

impl Coverage {
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Complete => None,
            Self::Partial { reason } | Self::Unknown { reason } => {
                Some(reason.as_str())
            }
        }
    }
}

/// Node-reported aggregate mempool counters (for example from `getmempoolinfo`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeMempoolAggregate {
    pub tx_count: u64,
    pub vbytes: u64,
    pub observed_at: u64,
    pub stale: bool,
}

fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}

/// A newly observed block whose full canonical summary is still in flight.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingBlock {
    pub hash: String,
    pub observed_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub observed_order: u64,
}

/// A confirmed canonical block summary on the Heartbeat timeline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalBlock {
    pub height: u64,
    pub hash: String,
    pub timestamp: u64,
    pub tx_count: u64,
    pub total_fees: u64,
    pub size: u64,
    pub weight: u64,
    pub observed_at: u64,
    /// Identified mining pool or solo miner from coinbase, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub miner: Option<String>,
}

/// Notable-transaction bitmask stored in a single `u8`.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
)]
#[serde(transparent)]
pub struct TxFlags(pub u8);

impl TxFlags {
    pub const NONE: u8 = 0;
    pub const WHALE: u8 = 1 << 0;
    pub const FEE_OUTLIER: u8 = 1 << 1;
    pub const CONSOLIDATION: u8 = 1 << 2;
    pub const FAN_OUT: u8 = 1 << 3;
    pub const LARGE_INSCRIPTION: u8 = 1 << 4;
    pub const ROUND_NUMBER: u8 = 1 << 5;
    pub const OP_RETURN_MSG: u8 = 1 << 6;
    pub const ALL: u8 = Self::WHALE
        | Self::FEE_OUTLIER
        | Self::CONSOLIDATION
        | Self::FAN_OUT
        | Self::LARGE_INSCRIPTION
        | Self::ROUND_NUMBER
        | Self::OP_RETURN_MSG;

    pub const fn new(bits: u8) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn has(self, mask: u8) -> bool {
        (self.0 & mask) == mask
    }

    pub const fn contains(bits: u8, mask: u8) -> bool {
        (bits & mask) == mask
    }

    pub const fn is_notable(bits: u8) -> bool {
        (bits & Self::ALL) != 0
    }

    #[allow(clippy::too_many_arguments)]
    pub const fn from_bools(
        whale: bool,
        fee_outlier: bool,
        consolidation: bool,
        fan_out: bool,
        large_inscription: bool,
        round_number: bool,
        op_return_msg: bool,
    ) -> u8 {
        let mut bits = 0u8;
        if whale {
            bits |= Self::WHALE;
        }
        if fee_outlier {
            bits |= Self::FEE_OUTLIER;
        }
        if consolidation {
            bits |= Self::CONSOLIDATION;
        }
        if fan_out {
            bits |= Self::FAN_OUT;
        }
        if large_inscription {
            bits |= Self::LARGE_INSCRIPTION;
        }
        if round_number {
            bits |= Self::ROUND_NUMBER;
        }
        if op_return_msg {
            bits |= Self::OP_RETURN_MSG;
        }
        bits
    }

    pub fn from_notable_type(notable_type: Option<&str>) -> u8 {
        match notable_type {
            Some("whale") => Self::WHALE,
            Some("fee_outlier") => Self::FEE_OUTLIER,
            Some("consolidation") => Self::CONSOLIDATION,
            Some("fan_out") => Self::FAN_OUT,
            Some("large_inscription") => Self::LARGE_INSCRIPTION,
            Some("round_number") => Self::ROUND_NUMBER,
            Some("op_return_msg") => Self::OP_RETURN_MSG,
            _ => Self::NONE,
        }
    }
}

impl From<u8> for TxFlags {
    fn from(bits: u8) -> Self {
        Self(bits)
    }
}

impl From<TxFlags> for u8 {
    fn from(flags: TxFlags) -> Self {
        flags.0
    }
}

/// Compact wire representation of an unconfirmed mempool transaction.
///
/// Serializes as a compact JSON array tuple:
/// `[txid, first_seen, fee, vsize, value, flags, op_return_text, observed_order]`
/// with trailing `None` / default `0` fields omitted so 150k-tx snapshots stay
/// compact in memory and over the wire.
///
/// Exact integer options preserve the distinction between pending/unknown
/// details (`None` / `null`) and verified zero (`Some(0)` / `0`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactTx {
    pub txid: String,
    pub first_seen: u64,
    pub fee: Option<u64>,
    pub vsize: Option<u32>,
    pub value: Option<u64>,
    pub flags: u8,
    pub op_return_text: Option<String>,
    pub observed_order: u64,
}

impl CompactTx {
    /// Construct a member whose fee/vsize/value details have not arrived yet.
    pub fn pending(txid: String, first_seen: u64) -> Self {
        Self {
            txid,
            first_seen,
            fee: None,
            vsize: None,
            value: None,
            flags: TxFlags::NONE,
            op_return_text: None,
            observed_order: 0,
        }
    }

    /// True once fee, vsize, and value are all populated.
    pub fn has_details(&self) -> bool {
        self.fee.is_some() && self.vsize.is_some() && self.value.is_some()
    }

    /// Fee rate in sat/vB when both fee and a non-zero vsize are known.
    pub fn fee_rate_sat_per_vb(&self) -> Option<f64> {
        let fee = self.fee?;
        let vsize = self.vsize?;
        (vsize > 0).then(|| fee as f64 / f64::from(vsize))
    }

    /// Merge details-only metadata onto this transaction in place.
    ///
    /// Never pulls a known `first_seen > 0` earlier from a second clock, so
    /// cross-clock reconcile hydration cannot churn timestamps and a
    /// re-admitted transaction cannot inherit an older lifetime's timestamp.
    pub(crate) fn apply_hydration(
        &mut self,
        entry: &TxMetadataHydration,
    ) -> bool {
        let mut changed = false;
        if let Some(fee) = entry.fee {
            if self.fee != Some(fee) {
                self.fee = Some(fee);
                changed = true;
            }
        }
        if let Some(vsize) = entry.vsize {
            if self.vsize != Some(vsize) {
                self.vsize = Some(vsize);
                changed = true;
            }
        }
        if let Some(value) = entry.value {
            if self.value != Some(value) {
                self.value = Some(value);
                changed = true;
            }
        }
        let merged_flags = self.flags | entry.flags;
        if merged_flags != self.flags {
            self.flags = merged_flags;
            changed = true;
        }
        if let Some(text) = &entry.op_return_text {
            if self.op_return_text.as_ref() != Some(text) {
                self.op_return_text = Some(text.clone());
                changed = true;
            }
        }
        if let Some(ts) = entry.first_seen {
            if self.first_seen == 0 && ts > 0 {
                self.first_seen = ts;
                changed = true;
            }
        }
        changed
    }
}

impl Serialize for CompactTx {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let len = if self.observed_order != 0 {
            8
        } else if self.op_return_text.is_some() {
            7
        } else if self.flags != 0 {
            6
        } else if self.value.is_some() {
            5
        } else if self.vsize.is_some() {
            4
        } else if self.fee.is_some() {
            3
        } else {
            2
        };

        let mut seq = serializer.serialize_seq(Some(len))?;
        seq.serialize_element(&self.txid)?;
        seq.serialize_element(&self.first_seen)?;
        if len >= 3 {
            seq.serialize_element(&self.fee)?;
        }
        if len >= 4 {
            seq.serialize_element(&self.vsize)?;
        }
        if len >= 5 {
            seq.serialize_element(&self.value)?;
        }
        if len >= 6 {
            seq.serialize_element(&self.flags)?;
        }
        if len >= 7 {
            seq.serialize_element(&self.op_return_text)?;
        }
        if len >= 8 {
            seq.serialize_element(&self.observed_order)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for CompactTx {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CompactTxVisitor;

        impl<'de> Visitor<'de> for CompactTxVisitor {
            type Value = CompactTx;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(
                    "a JSON array [txid, first_seen, fee?, vsize?, value?, flags?, op_return_text?, observed_order?]",
                )
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let txid: String = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let first_seen: u64 = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let fee: Option<u64> = seq.next_element()?.flatten();
                let vsize: Option<u32> = seq.next_element()?.flatten();
                let value: Option<u64> = seq.next_element()?.flatten();
                let flags: u8 =
                    seq.next_element::<Option<u8>>()?.flatten().unwrap_or(0);
                let op_return_text: Option<String> =
                    seq.next_element()?.flatten();
                let observed_order: u64 =
                    seq.next_element::<Option<u64>>()?.flatten().unwrap_or(0);

                if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(de::Error::invalid_length(9, &self));
                }

                Ok(CompactTx {
                    txid,
                    first_seen,
                    fee,
                    vsize,
                    value,
                    flags,
                    op_return_text,
                    observed_order,
                })
            }
        }

        deserializer.deserialize_seq(CompactTxVisitor)
    }
}

/// Bounded group of recently confirmed transaction IDs for a single block,
/// carried in snapshots so rejoining mirrors match continuous mirrors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmedBlock {
    pub hash: String,
    pub height: u64,
    pub txids: Vec<String>,
}

/// Full point-in-time state snapshot for bootstrapping or resyncing a mirror.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatSnapshot {
    pub protocol_version: u32,
    pub stream_epoch: String,
    pub snapshot_id: String,
    #[serde(with = "u64_str")]
    pub seq: u64,
    pub generated_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_observed_at: Option<u64>,
    pub coverage: Coverage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_mempool: Option<NodeMempoolAggregate>,
    #[serde(default)]
    pub pending_blocks: Vec<PendingBlock>,
    #[serde(default)]
    pub recent_blocks: Vec<CanonicalBlock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_confirmations: Vec<ConfirmedBlock>,
    #[serde(default)]
    pub transactions: Vec<CompactTx>,
}

/// Ordered incremental state transition emitted on the Heartbeat SSE stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatDelta {
    pub protocol_version: u32,
    pub stream_epoch: String,
    #[serde(with = "u64_str")]
    pub seq: u64,
    pub observed_at: u64,
    pub kind: DeltaKind,
}

/// Tagged delta payload variants.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeltaKind {
    MembershipAdd {
        txid: String,
        first_seen: u64,
    },
    MembershipRemove {
        txids: Vec<String>,
    },
    TxDetails {
        txid: String,
        fee: u64,
        vsize: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<u64>,
        flags: u8,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        op_return_text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        first_seen: Option<u64>,
    },
    BlockObserved {
        hash: String,
        observed_at: u64,
    },
    BlockDetails(CanonicalBlock),
    BlockInclusion {
        hash: String,
        height: u64,
        txids: Vec<String>,
    },
    BlockDisconnected {
        hash: String,
    },
    CoverageChanged {
        coverage: Coverage,
    },
    MetricsUpdated {
        node_mempool: Option<NodeMempoolAggregate>,
    },
}

/// Control frame instructing clients to fetch a fresh snapshot.
///
/// Browser client contract on `resync_required`: immediately call `es.close()`,
/// fetch `/api/stats/heartbeat/snapshot`, install the snapshot, and open a new
/// `EventSource` at the snapshot's `(stream_epoch, seq)`. Never allow browser
/// `EventSource` auto-reconnect to reuse a stale `Last-Event-ID`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResyncRequired {
    pub protocol_version: u32,
    pub stream_epoch: String,
    pub reason: String,
    #[serde(with = "u64_str")]
    pub latest_seq: u64,
}

/// Response payload for lightweight server-assisted transaction lookup
/// (`/api/stats/heartbeat/tx/{txid}`).
///
/// Adjacently tagged because `CompactTx` serializes as a JSON array tuple,
/// which serde cannot flatten into an internally tagged object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "data", rename_all = "snake_case")]
pub enum TxLookupStatus {
    Unconfirmed(CompactTx),
    Confirmed {
        txid: String,
        block_hash: String,
        block_height: u64,
    },
    Absent {
        coverage: Coverage,
    },
}

/// Error returned when [`HeartbeatMirror::install_snapshot`] rejects a snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MirrorError {
    UnsupportedProtocolVersion { expected: u32, got: u32 },
    EmptyStreamEpoch,
}

impl fmt::Display for MirrorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProtocolVersion { expected, got } => {
                write!(
                    f,
                    "unsupported protocol version: expected {expected}, got {got}"
                )
            }
            Self::EmptyStreamEpoch => {
                f.write_str("snapshot stream_epoch must not be empty")
            }
        }
    }
}

impl std::error::Error for MirrorError {}

/// Outcome of attempting to apply one [`HeartbeatDelta`] to [`HeartbeatMirror`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    DuplicateIgnored,
    ResyncNeeded(String),
}

/// Effective state transition emitted alongside [`ApplyOutcome::Applied`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ApplyEffect {
    MembershipAdd {
        txid: String,
        first_seen: u64,
    },
    MembershipRemove {
        txids: Vec<String>,
    },
    TxDetails {
        tx: CompactTx,
    },
    BlockObserved {
        hash: String,
        observed_at: u64,
    },
    BlockDetails(CanonicalBlock),
    BlockInclusion {
        hash: String,
        height: u64,
        confirmed_txids: Vec<String>,
    },
    BlockDisconnected {
        hash: String,
    },
    CoverageChanged {
        coverage: Coverage,
    },
    MetricsUpdated {
        node_mempool: Option<NodeMempoolAggregate>,
    },
}

/// Details-only metadata used to hydrate existing [`HeartbeatMirror`] members
/// without admitting membership.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TxMetadataHydration {
    pub txid: String,
    pub first_seen: Option<u64>,
    pub fee: Option<u64>,
    pub vsize: Option<u32>,
    pub value: Option<u64>,
    pub flags: u8,
    pub op_return_text: Option<String>,
}

/// Result of searching [`HeartbeatMirror`] by full txid or hex prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MirrorTxMatch {
    Unconfirmed(CompactTx),
    Confirmed {
        txid: String,
        block_hash: String,
        block_height: u64,
    },
}

/// Estimated queue depth ahead of an unconfirmed mempool transaction, computed
/// strictly from solo fee rates (`fee / vsize`) across enriched mempool members.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueuePosition {
    /// Unconfirmed transactions with a strictly higher solo fee rate.
    pub txs_ahead: u64,
    /// Total virtual bytes of those higher-fee transactions.
    pub vbytes_ahead: u64,
    /// Estimated confirmation target in blocks (`>= 1`, 1,000,000 vB/block),
    /// counting `(vbytes_ahead + tx_vsize)` so boundary overflows round up honestly.
    pub est_blocks: u64,
    /// Unconfirmed transactions currently missing fee/vsize (excluded from the
    /// higher-fee count so callers can surface the caveat transparently).
    pub unenriched_count: u64,
}

/// Number of fixed fee-rate bands rendered on the Heartbeat canvas & legend:
/// `0 = Pending`, `1 = < 1`, `2 = 1-2`, `3 = 2-5`, `4 = 5-15`, `5 = 15-50`, `6 = 50+`.
pub const FEE_BAND_COUNT: usize = 7;

/// Per-band mempool transaction count and virtual-byte total.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeeBandStat {
    pub tx_count: u64,
    pub vbytes: u64,
}

/// Live breakdown of the mirrored mempool across the 7 fixed canvas fee bands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeeBandSummary {
    pub bands: [FeeBandStat; FEE_BAND_COUNT],
}

/// Classify `(fee, vsize)` into one of the 7 fixed canvas fee bands using exact
/// integer arithmetic so boundary values (`1.0`, `2.0`, `5.0`, `15.0`, `50.0`)
/// never suffer floating-point drift.
pub fn fee_band_index(fee: Option<u64>, vsize: Option<u32>) -> usize {
    let (Some(f), Some(v)) = (fee, vsize) else {
        return 0;
    };
    if v == 0 {
        return 0;
    }
    let f = u128::from(f);
    let v = u128::from(v);
    if f < v {
        1
    } else if f < 2 * v {
        2
    } else if f < 5 * v {
        3
    } else if f < 15 * v {
        4
    } else if f < 50 * v {
        5
    } else {
        6
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MirrorTx {
    tx: CompactTx,
    /// Process-local observation counter. Snapshot members start at 0; each
    /// live `MembershipAdd` and `BlockObserved` advances the counter so
    /// `BlockInclusion` never evicts a tx re-added after `BlockObserved`.
    observed_order: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConfirmedEntry {
    block_hash: String,
    block_height: u64,
    order: u64,
}

/// Snapshot-comparable observable state of a [`HeartbeatMirror`].
///
/// Two mirrors at the same `(stream_epoch, seq)` (one continuously applying
/// deltas and one rejoining from a mid-stream snapshot plus trailing deltas)
/// produce identical `ObservableState`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservableState {
    pub epoch: Option<String>,
    pub seq: u64,
    pub coverage: Coverage,
    pub node_mempool: Option<NodeMempoolAggregate>,
    pub pending_blocks: Vec<PendingBlock>,
    pub recent_blocks: Vec<CanonicalBlock>,
    pub members: BTreeMap<String, CompactTx>,
    pub confirmed: BTreeMap<String, (String, u64)>,
    pub resync_reason: Option<String>,
}

/// Pure, deterministic client/server mirror of the Heartbeat protocol stream.
#[derive(Debug, Default)]
pub struct HeartbeatMirror {
    epoch: Option<String>,
    seq: u64,
    clock: u64,
    members: BTreeMap<String, MirrorTx>,
    block_observed_order: HashMap<String, u64>,
    pending_blocks: Vec<PendingBlock>,
    recent_blocks: Vec<CanonicalBlock>,
    confirmed: HashMap<String, ConfirmedEntry>,
    confirmed_order: VecDeque<(String, u64)>,
    confirmed_counter: u64,
    coverage: Coverage,
    node_mempool: Option<NodeMempoolAggregate>,
    resync_reason: Option<String>,
}

impl HeartbeatMirror {
    /// Atomically replace mirror state with a validated snapshot.
    ///
    /// On error, the mirror's prior coherent state is left untouched.
    pub fn install_snapshot(
        &mut self,
        snapshot: HeartbeatSnapshot,
    ) -> Result<(), MirrorError> {
        if snapshot.protocol_version != PROTOCOL_VERSION {
            return Err(MirrorError::UnsupportedProtocolVersion {
                expected: PROTOCOL_VERSION,
                got: snapshot.protocol_version,
            });
        }
        if snapshot.stream_epoch.is_empty() {
            return Err(MirrorError::EmptyStreamEpoch);
        }

        let mut max_order = 0u64;
        let mut members = BTreeMap::new();
        for mut tx in snapshot.transactions {
            let order = tx.observed_order;
            tx.observed_order = 0;
            max_order = max_order.max(order);
            members.insert(
                tx.txid.clone(),
                MirrorTx {
                    tx,
                    observed_order: order,
                },
            );
        }

        let mut recent_by_hash = HashMap::<String, CanonicalBlock>::new();
        for block in snapshot.recent_blocks {
            recent_by_hash.insert(block.hash.clone(), block);
        }
        let mut recent_blocks: Vec<CanonicalBlock> =
            recent_by_hash.into_values().collect();
        sort_and_cap_recent_blocks(&mut recent_blocks);

        let mut pending_by_hash = HashMap::<String, (PendingBlock, u64)>::new();
        for mut pending in snapshot.pending_blocks {
            if !recent_blocks.iter().any(|b| b.hash == pending.hash) {
                let order = pending.observed_order;
                pending.observed_order = 0;
                pending_by_hash.insert(pending.hash.clone(), (pending, order));
            }
        }
        let mut pending_orders = HashMap::with_capacity(pending_by_hash.len());
        let mut pending_blocks: Vec<PendingBlock> =
            Vec::with_capacity(pending_by_hash.len());
        for (hash, (pending, order)) in pending_by_hash {
            pending_orders.insert(hash, order);
            pending_blocks.push(pending);
        }
        sort_and_cap_pending_blocks(&mut pending_blocks);

        let mut block_observed_order =
            HashMap::with_capacity(pending_blocks.len() + recent_blocks.len());
        for pending in &pending_blocks {
            let order = pending_orders.get(&pending.hash).copied().unwrap_or(0);
            max_order = max_order.max(order);
            block_observed_order.insert(pending.hash.clone(), order);
        }
        for block in &recent_blocks {
            block_observed_order.insert(block.hash.clone(), 0);
        }

        self.epoch = Some(snapshot.stream_epoch);
        self.seq = snapshot.seq;
        self.clock = max_order;
        self.members = members;
        self.block_observed_order = block_observed_order;
        self.pending_blocks = pending_blocks;
        self.recent_blocks = recent_blocks;
        self.confirmed.clear();
        self.confirmed_order.clear();
        self.confirmed_counter = 0;
        for block in snapshot.recent_confirmations {
            for txid in block.txids {
                if !self.members.contains_key(&txid) {
                    self.record_confirmed(
                        txid,
                        block.hash.clone(),
                        block.height,
                    );
                }
            }
        }
        self.coverage = snapshot.coverage;
        self.node_mempool = snapshot.node_mempool;
        self.resync_reason = None;

        Ok(())
    }

    /// Apply a single sequenced delta frame.
    ///
    /// On protocol mismatch, epoch mismatch, uninitialized state, or sequence
    /// gap, sets `resync_reason`, preserves the last coherent state, and
    /// returns [`ApplyOutcome::ResyncNeeded`].
    pub fn apply_delta(&mut self, delta: HeartbeatDelta) -> ApplyOutcome {
        self.apply_delta_with_effect(&delta).0
    }

    /// Apply a single sequenced delta frame and return the effective state
    /// transition for downstream renderers.
    ///
    /// On protocol mismatch, epoch mismatch, uninitialized state, or sequence
    /// gap, sets `resync_reason`, preserves the last coherent state, and
    /// returns `(ApplyOutcome::ResyncNeeded(_), None)`.
    pub fn apply_delta_with_effect(
        &mut self,
        delta: &HeartbeatDelta,
    ) -> (ApplyOutcome, Option<ApplyEffect>) {
        if delta.protocol_version != PROTOCOL_VERSION {
            let reason = format!(
                "unsupported protocol version: expected {}, got {}",
                PROTOCOL_VERSION, delta.protocol_version
            );
            self.resync_reason = Some(reason.clone());
            return (ApplyOutcome::ResyncNeeded(reason), None);
        }

        let Some(current_epoch) = self.epoch.as_deref() else {
            let reason = "no snapshot installed".to_string();
            self.resync_reason = Some(reason.clone());
            return (ApplyOutcome::ResyncNeeded(reason), None);
        };

        if delta.stream_epoch != current_epoch {
            let reason = format!(
                "stream epoch mismatch: expected {current_epoch}, got {}",
                delta.stream_epoch
            );
            self.resync_reason = Some(reason.clone());
            return (ApplyOutcome::ResyncNeeded(reason), None);
        }

        if let Some(reason) = &self.resync_reason {
            return (ApplyOutcome::ResyncNeeded(reason.clone()), None);
        }

        if delta.seq <= self.seq {
            return (ApplyOutcome::DuplicateIgnored, None);
        }

        let Some(expected) = self.seq.checked_add(1) else {
            let reason = "sequence number overflow".to_string();
            self.resync_reason = Some(reason.clone());
            return (ApplyOutcome::ResyncNeeded(reason), None);
        };

        if delta.seq > expected {
            let reason =
                format!("sequence gap: expected {expected}, got {}", delta.seq);
            self.resync_reason = Some(reason.clone());
            return (ApplyOutcome::ResyncNeeded(reason), None);
        }

        let effect = match &delta.kind {
            DeltaKind::MembershipAdd { txid, first_seen } => {
                let order = self.tick();
                // A transaction re-admitted to the mempool is no longer confirmed.
                self.confirmed.remove(txid);
                let mut inserted = false;
                self.members.entry(txid.clone()).or_insert_with(|| {
                    inserted = true;
                    MirrorTx {
                        tx: CompactTx::pending(txid.clone(), *first_seen),
                        observed_order: order,
                    }
                });
                inserted.then(|| ApplyEffect::MembershipAdd {
                    txid: txid.clone(),
                    first_seen: *first_seen,
                })
            }
            DeltaKind::MembershipRemove { txids } => {
                let mut removed = Vec::new();
                for txid in txids {
                    if self.members.remove(txid).is_some() {
                        removed.push(txid.clone());
                    }
                }
                Some(ApplyEffect::MembershipRemove { txids: removed })
            }
            DeltaKind::TxDetails {
                txid,
                fee,
                vsize,
                value,
                flags,
                op_return_text,
                first_seen,
            } => {
                if let Some(member) = self.members.get_mut(txid) {
                    member.tx.fee = Some(*fee);
                    member.tx.vsize = Some(*vsize);
                    if let Some(v) = value {
                        member.tx.value = Some(*v);
                    }
                    member.tx.flags |= *flags;
                    if op_return_text.is_some() {
                        member.tx.op_return_text = op_return_text.clone();
                    }
                    if member.tx.first_seen == 0 {
                        if let Some(ts) = first_seen.filter(|&t| t > 0) {
                            member.tx.first_seen = ts;
                        }
                    }
                    Some(ApplyEffect::TxDetails {
                        tx: member.tx.clone(),
                    })
                } else {
                    None
                }
            }
            DeltaKind::BlockObserved { hash, observed_at } => {
                let order = self.tick();
                self.block_observed_order
                    .entry(hash.clone())
                    .or_insert(order);
                if !self.pending_blocks.iter().any(|b| b.hash == *hash)
                    && !self.recent_blocks.iter().any(|b| b.hash == *hash)
                {
                    self.pending_blocks.push(PendingBlock {
                        hash: hash.clone(),
                        observed_at: *observed_at,
                        height: None,
                        observed_order: 0,
                    });
                    sort_and_cap_pending_blocks(&mut self.pending_blocks);
                    self.prune_block_observed_orders();
                }
                Some(ApplyEffect::BlockObserved {
                    hash: hash.clone(),
                    observed_at: *observed_at,
                })
            }
            DeltaKind::BlockDetails(block) => {
                self.pending_blocks.retain(|b| b.hash != block.hash);
                self.recent_blocks.retain(|b| b.hash != block.hash);
                self.recent_blocks.push(block.clone());
                sort_and_cap_recent_blocks(&mut self.recent_blocks);
                self.prune_block_observed_orders();
                Some(ApplyEffect::BlockDetails(block.clone()))
            }
            DeltaKind::BlockInclusion {
                hash,
                height,
                txids,
            } => {
                let Some(cutoff) = self.block_observed_order.get(hash).copied()
                else {
                    self.seq = delta.seq;
                    return (
                        ApplyOutcome::Applied,
                        Some(ApplyEffect::BlockInclusion {
                            hash: hash.clone(),
                            height: *height,
                            confirmed_txids: Vec::new(),
                        }),
                    );
                };
                if let Some(pending) =
                    self.pending_blocks.iter_mut().find(|b| b.hash == *hash)
                {
                    if pending.height.is_none() {
                        pending.height = Some(*height);
                    }
                }
                let mut confirmed_txids = Vec::new();
                for txid in txids {
                    let eligible = self
                        .members
                        .get(txid)
                        .is_some_and(|m| m.observed_order <= cutoff);
                    if eligible {
                        self.members.remove(txid);
                        self.record_confirmed(
                            txid.clone(),
                            hash.clone(),
                            *height,
                        );
                        confirmed_txids.push(txid.clone());
                    }
                }
                Some(ApplyEffect::BlockInclusion {
                    hash: hash.clone(),
                    height: *height,
                    confirmed_txids,
                })
            }
            DeltaKind::BlockDisconnected { hash } => {
                self.pending_blocks.retain(|b| b.hash != *hash);
                self.recent_blocks.retain(|b| b.hash != *hash);
                self.block_observed_order.remove(hash);
                self.confirmed.retain(|_, entry| entry.block_hash != *hash);
                if self.confirmed_order.len()
                    > self.confirmed.len().saturating_add(MAX_CONFIRMED_HISTORY)
                {
                    self.compact_confirmed_order();
                }
                Some(ApplyEffect::BlockDisconnected { hash: hash.clone() })
            }
            DeltaKind::CoverageChanged { coverage } => {
                self.coverage = coverage.clone();
                Some(ApplyEffect::CoverageChanged {
                    coverage: coverage.clone(),
                })
            }
            DeltaKind::MetricsUpdated { node_mempool } => {
                self.node_mempool = node_mempool.clone();
                Some(ApplyEffect::MetricsUpdated {
                    node_mempool: node_mempool.clone(),
                })
            }
        };

        self.seq = delta.seq;
        (ApplyOutcome::Applied, effect)
    }

    /// Details-only metadata hydration for an existing member.
    ///
    /// Never admits membership: returns `false` immediately if `entry.txid` is
    /// not currently in `self.members`. Returns `true` iff any field changed.
    pub fn hydrate_member_details(
        &mut self,
        entry: &TxMetadataHydration,
    ) -> bool {
        let Some(member) = self.members.get_mut(&entry.txid) else {
            return false;
        };
        member.tx.apply_hydration(entry)
    }

    /// Update the mirror's `(stream_epoch, seq)` revision in place without
    /// cloning member state (used by server-side bulk metadata hydration).
    #[cfg(feature = "ssr")]
    pub(crate) fn reset_revision(&mut self, stream_epoch: String, seq: u64) {
        self.epoch = Some(stream_epoch);
        self.seq = seq;
        self.resync_reason = None;
    }

    /// Current `(stream_epoch, seq)` revision once a snapshot is installed.
    pub fn revision(&self) -> Option<(&str, u64)> {
        self.epoch.as_deref().map(|epoch| (epoch, self.seq))
    }

    /// Number of currently tracked unconfirmed transactions.
    pub fn tx_count(&self) -> usize {
        self.members.len()
    }

    /// Sum of known `vsize` values across tracked members.
    ///
    /// Returns `None` before a snapshot is installed, or when members exist
    /// but none have received `TxDetails` yet. Returns `Some(0)` for an
    /// initialized empty mempool or when verified vbytes sum to zero.
    pub fn known_vbytes(&self) -> Option<u64> {
        self.epoch.as_ref()?;
        if self.members.is_empty() {
            return Some(0);
        }
        let mut sum = 0u64;
        let mut any_known = false;
        for member in self.members.values() {
            if let Some(vsize) = member.tx.vsize {
                any_known = true;
                sum = sum.saturating_add(u64::from(vsize));
            }
        }
        any_known.then_some(sum)
    }

    /// Exact sum of `vsize` across all tracked members, or `None` if
    /// uninitialized or any member still has `vsize: None`.
    pub fn exact_vbytes(&self) -> Option<u64> {
        self.epoch.as_ref()?;
        let mut sum = 0u64;
        for member in self.members.values() {
            let vsize = member.tx.vsize?;
            sum = sum.saturating_add(u64::from(vsize));
        }
        Some(sum)
    }

    /// Number of tracked members still missing `fee`, `vsize`, or `value`.
    pub fn pending_details_count(&self) -> usize {
        self.members
            .values()
            .filter(|m| !m.tx.has_details())
            .count()
    }

    pub fn has_tx(&self, txid: &str) -> bool {
        self.members.contains_key(txid)
    }

    pub fn get_tx(&self, txid: &str) -> Option<&CompactTx> {
        self.members.get(txid).map(|m| &m.tx)
    }

    pub fn members(&self) -> impl ExactSizeIterator<Item = (&str, &CompactTx)> {
        self.members
            .iter()
            .map(|(txid, member)| (txid.as_str(), &member.tx))
    }

    /// Confirmation `(block_hash, block_height)` if confirmed in the retained
    /// confirmation cache.
    pub fn confirmed_in(&self, txid: &str) -> Option<(&str, u64)> {
        self.confirmed
            .get(txid)
            .map(|e| (e.block_hash.as_str(), e.block_height))
    }

    pub fn confirmed_count(&self) -> usize {
        self.confirmed.len()
    }

    pub fn pending_blocks(&self) -> &[PendingBlock] {
        &self.pending_blocks
    }

    pub fn recent_blocks(&self) -> &[CanonicalBlock] {
        &self.recent_blocks
    }

    pub fn coverage(&self) -> &Coverage {
        &self.coverage
    }

    pub fn node_mempool(&self) -> Option<&NodeMempoolAggregate> {
        self.node_mempool.as_ref()
    }

    pub fn resync_reason(&self) -> Option<&str> {
        self.resync_reason.as_deref()
    }

    /// Lookup a transaction against the mirror's current mempool and retained
    /// confirmation cache.
    pub fn lookup_tx(&self, txid: &str) -> TxLookupStatus {
        if let Some(tx) = self.get_tx(txid) {
            return TxLookupStatus::Unconfirmed(tx.clone());
        }
        if let Some((block_hash, block_height)) = self.confirmed_in(txid) {
            return TxLookupStatus::Confirmed {
                txid: txid.to_string(),
                block_hash: block_hash.to_string(),
                block_height,
            };
        }
        TxLookupStatus::Absent {
            coverage: self.coverage.clone(),
        }
    }

    /// Search for a transaction by exact txid or hex prefix (>= 8 chars).
    ///
    /// Checks exact matches in unconfirmed members first, then confirmed
    /// history. For prefixes of length >= 8, searches `self.members` in
    /// O(log n) using its `BTreeMap` order before falling back to a
    /// deterministic lowest-txid scan of `self.confirmed`.
    pub fn search_tx(&self, query: &str) -> Option<MirrorTxMatch> {
        let needle = query.trim().to_ascii_lowercase();
        if needle.is_empty() || !needle.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }

        if let Some(member) = self.members.get(&needle) {
            return Some(MirrorTxMatch::Unconfirmed(member.tx.clone()));
        }
        if let Some(entry) = self.confirmed.get(&needle) {
            return Some(MirrorTxMatch::Confirmed {
                txid: needle,
                block_hash: entry.block_hash.clone(),
                block_height: entry.block_height,
            });
        }

        if needle.len() < 8 {
            return None;
        }

        if let Some((txid, member)) =
            self.members.range(needle.clone()..).next()
        {
            if txid.starts_with(&needle) {
                return Some(MirrorTxMatch::Unconfirmed(member.tx.clone()));
            }
        }

        let mut best: Option<(&String, &ConfirmedEntry)> = None;
        for (txid, entry) in &self.confirmed {
            if txid.starts_with(&needle)
                && best.is_none_or(|(best_txid, _)| txid < best_txid)
            {
                best = Some((txid, entry));
            }
        }
        best.map(|(txid, entry)| MirrorTxMatch::Confirmed {
            txid: txid.clone(),
            block_hash: entry.block_hash.clone(),
            block_height: entry.block_height,
        })
    }

    /// Return all current members ordered deterministically for backlog
    /// placement: unknown-time (`first_seen == 0`) first sorted by `txid`,
    /// followed by known-time (`first_seen > 0`) sorted by `(first_seen, txid)`.
    pub fn ordered_members(&self) -> Vec<&CompactTx> {
        let mut out: Vec<&CompactTx> =
            self.members.values().map(|m| &m.tx).collect();
        out.sort_by(|a, b| {
            a.first_seen
                .cmp(&b.first_seen)
                .then_with(|| a.txid.cmp(&b.txid))
        });
        out
    }

    /// Materialize the observable state for continuous-vs-rejoining equality
    /// assertions.
    pub fn observable_state(&self) -> ObservableState {
        ObservableState {
            epoch: self.epoch.clone(),
            seq: self.seq,
            coverage: self.coverage.clone(),
            node_mempool: self.node_mempool.clone(),
            pending_blocks: self.pending_blocks.clone(),
            recent_blocks: self.recent_blocks.clone(),
            members: self
                .members
                .iter()
                .map(|(txid, m)| (txid.clone(), m.tx.clone()))
                .collect(),
            confirmed: self
                .confirmed
                .iter()
                .map(|(txid, entry)| {
                    (
                        txid.clone(),
                        (entry.block_hash.clone(), entry.block_height),
                    )
                })
                .collect(),
            resync_reason: self.resync_reason.clone(),
        }
    }

    /// Compare two mirrors on their snapshot-comparable observable state.
    pub fn same_observable_state(&self, other: &Self) -> bool {
        self.observable_state() == other.observable_state()
    }

    /// Grouped, deterministically sorted recent block confirmations retained
    /// by the mirror.
    pub fn recent_confirmations(&self) -> Vec<ConfirmedBlock> {
        let mut grouped: BTreeMap<(u64, String), Vec<String>> = BTreeMap::new();
        for (txid, entry) in &self.confirmed {
            grouped
                .entry((entry.block_height, entry.block_hash.clone()))
                .or_default()
                .push(txid.clone());
        }
        grouped
            .into_iter()
            .map(|((height, hash), mut txids)| {
                txids.sort_unstable();
                ConfirmedBlock {
                    hash,
                    height,
                    txids,
                }
            })
            .collect()
    }

    /// Export the mirror's current state as a [`HeartbeatSnapshot`] if
    /// initialized.
    pub fn to_snapshot(
        &self,
        snapshot_id: String,
        generated_at: u64,
        node_observed_at: Option<u64>,
    ) -> Option<HeartbeatSnapshot> {
        let stream_epoch = self.epoch.clone()?;
        let (pending_blocks, transactions) = if self.pending_blocks.is_empty() {
            (
                Vec::new(),
                self.members.values().map(|m| m.tx.clone()).collect(),
            )
        } else {
            let min_pending = self
                .pending_blocks
                .iter()
                .map(|b| {
                    self.block_observed_order.get(&b.hash).copied().unwrap_or(0)
                })
                .min()
                .unwrap_or(0);
            let pending_blocks = self
                .pending_blocks
                .iter()
                .map(|b| {
                    let block_order = self
                        .block_observed_order
                        .get(&b.hash)
                        .copied()
                        .unwrap_or(0);
                    let mut cloned = b.clone();
                    cloned.observed_order =
                        block_order.saturating_sub(min_pending);
                    cloned
                })
                .collect();
            let transactions = self
                .members
                .values()
                .map(|m| {
                    let mut tx = m.tx.clone();
                    tx.observed_order =
                        m.observed_order.saturating_sub(min_pending);
                    tx
                })
                .collect();
            (pending_blocks, transactions)
        };
        let recent_confirmations = self.recent_confirmations();

        Some(HeartbeatSnapshot {
            protocol_version: PROTOCOL_VERSION,
            stream_epoch,
            snapshot_id,
            seq: self.seq,
            generated_at,
            node_observed_at,
            coverage: self.coverage.clone(),
            node_mempool: self.node_mempool.clone(),
            pending_blocks,
            recent_blocks: self.recent_blocks.clone(),
            recent_confirmations,
            transactions,
        })
    }

    /// Estimate queue depth ahead of an unconfirmed mempool member using exact
    /// `u128` cross-multiplied solo fee rates (zero floating-point drift).
    /// Returns `None` if `txid` is not in the mempool or lacks fee/vsize.
    pub fn queue_position(&self, txid: &str) -> Option<QueuePosition> {
        let target = self.members.get(txid)?;
        self.queue_position_for_rate(
            target.tx.fee?,
            target.tx.vsize?,
            Some(txid),
        )
    }

    /// Estimate queue depth ahead of a hypothetical or server-looked-up
    /// unconfirmed transaction with `(fee, vsize)`.
    pub fn queue_position_for_rate(
        &self,
        fee: u64,
        vsize: u32,
        exclude_txid: Option<&str>,
    ) -> Option<QueuePosition> {
        if vsize == 0 {
            return None;
        }
        const BLOCK_VBYTES: u64 = 1_000_000;
        let tx_fee = u128::from(fee);
        let tx_vsize = u128::from(vsize);

        let mut txs_ahead: u64 = 0;
        let mut vbytes_ahead: u64 = 0;
        let mut unenriched_count: u64 = 0;

        for (id, member) in &self.members {
            if exclude_txid.is_some_and(|ex| id == ex) {
                continue;
            }
            match (member.tx.fee, member.tx.vsize) {
                (Some(other_fee), Some(other_vsize)) if other_vsize > 0 => {
                    if u128::from(other_fee) * tx_vsize
                        > tx_fee * u128::from(other_vsize)
                    {
                        txs_ahead += 1;
                        vbytes_ahead =
                            vbytes_ahead.saturating_add(u64::from(other_vsize));
                    }
                }
                _ => {
                    unenriched_count += 1;
                }
            }
        }

        let total_with_self = vbytes_ahead.saturating_add(u64::from(vsize));
        let est_blocks = total_with_self.div_ceil(BLOCK_VBYTES).max(1);

        Some(QueuePosition {
            txs_ahead,
            vbytes_ahead,
            est_blocks,
            unenriched_count,
        })
    }

    /// Aggregate live mempool transaction counts and virtual bytes into the 7
    /// fixed canvas fee bands (`0 = Pending`, `1 = < 1`, ..., `6 = 50+`).
    pub fn fee_band_summary(&self) -> FeeBandSummary {
        let mut summary = FeeBandSummary::default();
        for member in self.members.values() {
            let idx = fee_band_index(member.tx.fee, member.tx.vsize);
            summary.bands[idx].tx_count += 1;
            if let Some(v) = member.tx.vsize {
                summary.bands[idx].vbytes =
                    summary.bands[idx].vbytes.saturating_add(u64::from(v));
            }
        }
        summary
    }

    fn tick(&mut self) -> u64 {
        self.clock = self
            .clock
            .checked_add(1)
            .expect("mirror observation clock exhausted");
        self.clock
    }

    fn record_confirmed(
        &mut self,
        txid: String,
        block_hash: String,
        block_height: u64,
    ) {
        self.confirmed_counter = self
            .confirmed_counter
            .checked_add(1)
            .expect("mirror confirmation counter exhausted");
        let order = self.confirmed_counter;
        self.confirmed.insert(
            txid.clone(),
            ConfirmedEntry {
                block_hash,
                block_height,
                order,
            },
        );
        self.confirmed_order.push_back((txid, order));
        while self.confirmed.len() > MAX_CONFIRMED_HISTORY {
            let Some((old_txid, old_order)) = self.confirmed_order.pop_front()
            else {
                break;
            };
            if self
                .confirmed
                .get(&old_txid)
                .is_some_and(|e| e.order == old_order)
            {
                self.confirmed.remove(&old_txid);
            }
        }
        if self.confirmed_order.len()
            > self.confirmed.len().saturating_add(MAX_CONFIRMED_HISTORY)
        {
            self.compact_confirmed_order();
        }
    }

    fn compact_confirmed_order(&mut self) {
        self.confirmed_order.retain(|(txid, order)| {
            self.confirmed
                .get(txid)
                .is_some_and(|entry| entry.order == *order)
        });
    }

    fn prune_block_observed_orders(&mut self) {
        self.block_observed_order.retain(|hash, _| {
            self.pending_blocks.iter().any(|b| b.hash == *hash)
                || self.recent_blocks.iter().any(|b| b.hash == *hash)
        });
    }
}

fn sort_and_cap_recent_blocks(blocks: &mut Vec<CanonicalBlock>) {
    blocks.sort_by(|a, b| {
        a.height
            .cmp(&b.height)
            .then_with(|| a.observed_at.cmp(&b.observed_at))
            .then_with(|| a.hash.cmp(&b.hash))
    });
    if blocks.len() > MAX_RECENT_BLOCKS {
        let drop_count = blocks.len() - MAX_RECENT_BLOCKS;
        blocks.drain(..drop_count);
    }
}

fn sort_and_cap_pending_blocks(blocks: &mut Vec<PendingBlock>) {
    blocks.sort_by(|a, b| {
        a.observed_at
            .cmp(&b.observed_at)
            .then_with(|| a.hash.cmp(&b.hash))
    });
    if blocks.len() > MAX_RECENT_BLOCKS {
        let drop_count = blocks.len() - MAX_RECENT_BLOCKS;
        blocks.drain(..drop_count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPOCH_A: &str = "epoch-2026-11-23-a";
    const EPOCH_B: &str = "epoch-2026-11-23-b";

    fn hex_id(seed: u8) -> String {
        format!("{seed:02x}").repeat(32)
    }

    fn base_snapshot(seq: u64, txs: Vec<CompactTx>) -> HeartbeatSnapshot {
        HeartbeatSnapshot {
            protocol_version: PROTOCOL_VERSION,
            stream_epoch: EPOCH_A.to_string(),
            snapshot_id: format!("snap-{seq}"),
            seq,
            generated_at: 1_700_000_000,
            node_observed_at: Some(1_700_000_000),
            coverage: Coverage::Complete,
            node_mempool: Some(NodeMempoolAggregate {
                tx_count: txs.len() as u64,
                vbytes: 500,
                observed_at: 1_700_000_000,
                stale: false,
            }),
            pending_blocks: vec![],
            recent_blocks: vec![],
            recent_confirmations: vec![],
            transactions: txs,
        }
    }

    fn delta(seq: u64, kind: DeltaKind) -> HeartbeatDelta {
        HeartbeatDelta {
            protocol_version: PROTOCOL_VERSION,
            stream_epoch: EPOCH_A.to_string(),
            seq,
            observed_at: 1_700_000_000u64.saturating_add(seq),
            kind,
        }
    }

    fn sample_block(height: u64, hash_seed: u8) -> CanonicalBlock {
        CanonicalBlock {
            height,
            hash: hex_id(hash_seed),
            timestamp: 1_700_000_000 + height,
            tx_count: 2500,
            total_fees: 15_000_000,
            size: 1_500_000,
            weight: 3_990_000,
            observed_at: 1_700_000_000 + height,
            miner: None,
        }
    }

    #[test]
    fn u64_str_round_trips_above_js_safe_integer() {
        let large_seq: u64 = (1u64 << 53) + 123_456_789;
        let max_seq: u64 = u64::MAX;

        let snap = base_snapshot(large_seq, vec![]);
        let json = serde_json::to_string(&snap).expect("serialize snapshot");
        assert!(
            json.contains(&format!("\"seq\":\"{large_seq}\"")),
            "seq must serialize as a JSON string: {json}"
        );
        let parsed: HeartbeatSnapshot =
            serde_json::from_str(&json).expect("deserialize snapshot");
        assert_eq!(parsed.seq, large_seq);

        let d = delta(max_seq, DeltaKind::MembershipRemove { txids: vec![] });
        let d_json = serde_json::to_string(&d).expect("serialize delta");
        assert!(d_json.contains(&format!("\"seq\":\"{max_seq}\"")));
        let d_parsed: HeartbeatDelta =
            serde_json::from_str(&d_json).expect("deserialize delta");
        assert_eq!(d_parsed.seq, max_seq);

        let resync = ResyncRequired {
            protocol_version: PROTOCOL_VERSION,
            stream_epoch: EPOCH_A.to_string(),
            reason: "lagged".to_string(),
            latest_seq: max_seq,
        };
        let r_json = serde_json::to_string(&resync).expect("serialize resync");
        assert!(r_json.contains(&format!("\"latest_seq\":\"{max_seq}\"")));
        let r_parsed: ResyncRequired =
            serde_json::from_str(&r_json).expect("deserialize resync");
        assert_eq!(r_parsed.latest_seq, max_seq);

        // Reject bare numeric seq, signed strings, and overflow.
        let bare_num = r_json.replace(&format!("\"{max_seq}\""), "42");
        assert!(serde_json::from_str::<ResyncRequired>(&bare_num).is_err());

        let plus_str = r_json.replace(&format!("\"{max_seq}\""), "\"+42\"");
        assert!(serde_json::from_str::<ResyncRequired>(&plus_str).is_err());

        let overflow = r_json
            .replace(&format!("\"{max_seq}\""), "\"18446744073709551616\"");
        assert!(serde_json::from_str::<ResyncRequired>(&overflow).is_err());
    }

    #[test]
    fn compact_tx_tuple_distinguishes_verified_zero_from_pending_none() {
        let pending = CompactTx::pending(hex_id(1), 1_700_000_001);
        let pending_json =
            serde_json::to_string(&pending).expect("serialize pending");
        assert_eq!(pending_json, format!("[\"{}\",1700000001]", hex_id(1)));
        let pending_rt: CompactTx =
            serde_json::from_str(&pending_json).expect("deserialize");
        assert_eq!(pending_rt, pending);
        assert_eq!(pending_rt.fee, None);
        assert_eq!(pending_rt.vsize, None);
        assert_eq!(pending_rt.value, None);
        assert!(!pending_rt.has_details());

        let zero_fee_and_value = CompactTx {
            txid: hex_id(2),
            first_seen: 1_700_000_002,
            fee: Some(0),
            vsize: Some(110),
            value: Some(0),
            flags: TxFlags::NONE,
            op_return_text: None,
            observed_order: 0,
        };
        let zero_json =
            serde_json::to_string(&zero_fee_and_value).expect("serialize zero");
        assert_eq!(
            zero_json,
            format!("[\"{}\",1700000002,0,110,0]", hex_id(2))
        );
        let zero_rt: CompactTx =
            serde_json::from_str(&zero_json).expect("deserialize");
        assert_eq!(zero_rt, zero_fee_and_value);
        assert_eq!(zero_rt.fee, Some(0));
        assert_eq!(zero_rt.vsize, Some(110));
        assert_eq!(zero_rt.value, Some(0));
        assert!(zero_rt.has_details());
        assert_eq!(zero_rt.fee_rate_sat_per_vb(), Some(0.0));

        // Partial details: verified zero fee, pending vsize/value.
        let fee_only = CompactTx {
            txid: hex_id(3),
            first_seen: 1_700_000_003,
            fee: Some(0),
            vsize: None,
            value: None,
            flags: TxFlags::NONE,
            op_return_text: None,
            observed_order: 0,
        };
        let fee_only_json =
            serde_json::to_string(&fee_only).expect("serialize fee_only");
        assert_eq!(fee_only_json, format!("[\"{}\",1700000003,0]", hex_id(3)));
        assert_eq!(
            serde_json::from_str::<CompactTx>(&fee_only_json)
                .expect("deserialize"),
            fee_only
        );

        // Full tuple with notable flags and OP_RETURN text.
        let flags = TxFlags::WHALE | TxFlags::OP_RETURN_MSG;
        let full = CompactTx {
            txid: hex_id(4),
            first_seen: 1_700_000_004,
            fee: Some(25_000),
            vsize: Some(225),
            value: Some(500_000_000),
            flags,
            op_return_text: Some("hello bitcoin".to_string()),
            observed_order: 0,
        };
        let full_json = serde_json::to_string(&full).expect("serialize full");
        assert_eq!(
            full_json,
            format!(
                "[\"{}\",1700000004,25000,225,500000000,{},\"hello bitcoin\"]",
                hex_id(4),
                flags
            )
        );
        assert_eq!(
            serde_json::from_str::<CompactTx>(&full_json).expect("deserialize"),
            full
        );

        // Optional 8th element is emitted only when observed_order != 0.
        let ordered = CompactTx {
            observed_order: 3,
            ..CompactTx::pending(hex_id(5), 1_700_000_005)
        };
        let ordered_json =
            serde_json::to_string(&ordered).expect("serialize ordered");
        assert_eq!(
            ordered_json,
            format!("[\"{}\",1700000005,null,null,null,0,null,3]", hex_id(5))
        );
        assert_eq!(
            serde_json::from_str::<CompactTx>(&ordered_json)
                .expect("deserialize"),
            ordered
        );

        // Reject too few or too many tuple elements.
        assert!(serde_json::from_str::<CompactTx>("[\"only_txid\"]").is_err());
        assert!(serde_json::from_str::<CompactTx>(
            "[\"tx\",1,2,3,4,5,\"msg\",\"extra\"]"
        )
        .is_err());
        assert!(serde_json::from_str::<CompactTx>(
            "[\"tx\",1,2,3,4,5,\"msg\",7,99]"
        )
        .is_err());
    }

    #[test]
    fn wire_enums_round_trip_serde() {
        let deltas = vec![
            delta(
                1,
                DeltaKind::MembershipAdd {
                    txid: hex_id(1),
                    first_seen: 100,
                },
            ),
            delta(
                2,
                DeltaKind::MembershipRemove {
                    txids: vec![hex_id(1)],
                },
            ),
            delta(
                3,
                DeltaKind::TxDetails {
                    txid: hex_id(2),
                    fee: 0,
                    vsize: 141,
                    value: Some(0),
                    flags: TxFlags::CONSOLIDATION,
                    op_return_text: None,
                    first_seen: None,
                },
            ),
            delta(
                4,
                DeltaKind::BlockObserved {
                    hash: hex_id(9),
                    observed_at: 200,
                },
            ),
            delta(5, DeltaKind::BlockDetails(sample_block(900_000, 9))),
            delta(
                6,
                DeltaKind::BlockInclusion {
                    hash: hex_id(9),
                    height: 900_000,
                    txids: vec![hex_id(2)],
                },
            ),
            delta(7, DeltaKind::BlockDisconnected { hash: hex_id(9) }),
            delta(
                8,
                DeltaKind::CoverageChanged {
                    coverage: Coverage::Partial {
                        reason: "zmq gap".to_string(),
                    },
                },
            ),
            delta(9, DeltaKind::MetricsUpdated { node_mempool: None }),
        ];

        for item in deltas {
            let encoded =
                serde_json::to_string(&item).expect("serialize delta");
            let decoded: HeartbeatDelta =
                serde_json::from_str(&encoded).expect("deserialize");
            assert_eq!(decoded, item);
        }

        let statuses = vec![
            TxLookupStatus::Unconfirmed(CompactTx::pending(hex_id(5), 1234)),
            TxLookupStatus::Confirmed {
                txid: hex_id(6),
                block_hash: hex_id(7),
                block_height: 900_001,
            },
            TxLookupStatus::Absent {
                coverage: Coverage::Unknown {
                    reason: "booting".to_string(),
                },
            },
        ];
        for status in statuses {
            let encoded = serde_json::to_string(&status)
                .expect("serialize lookup status");
            let decoded: TxLookupStatus =
                serde_json::from_str(&encoded).expect("deserialize");
            assert_eq!(decoded, status);
        }
    }

    #[test]
    fn snapshot_validation_preserves_prior_coherent_state() {
        let mut mirror = HeartbeatMirror::default();
        assert_eq!(mirror.revision(), None);
        assert_eq!(mirror.known_vbytes(), None);

        let good = base_snapshot(10, vec![CompactTx::pending(hex_id(1), 100)]);
        mirror.install_snapshot(good).expect("install valid");
        let before = mirror.observable_state();

        let mut bad_ver = base_snapshot(20, vec![]);
        bad_ver.protocol_version = PROTOCOL_VERSION + 1;
        assert_eq!(
            mirror.install_snapshot(bad_ver),
            Err(MirrorError::UnsupportedProtocolVersion {
                expected: PROTOCOL_VERSION,
                got: PROTOCOL_VERSION + 1,
            })
        );
        assert_eq!(mirror.observable_state(), before);

        let mut empty_epoch = base_snapshot(21, vec![]);
        empty_epoch.stream_epoch.clear();
        assert_eq!(
            mirror.install_snapshot(empty_epoch),
            Err(MirrorError::EmptyStreamEpoch)
        );
        assert_eq!(mirror.observable_state(), before);
    }

    #[test]
    fn delta_checks_duplicate_gap_epoch_and_version() {
        let mut mirror = HeartbeatMirror::default();

        // Delta before snapshot requests resync.
        assert!(matches!(
            mirror.apply_delta(delta(
                1,
                DeltaKind::MembershipRemove { txids: vec![] }
            )),
            ApplyOutcome::ResyncNeeded(_)
        ));
        assert!(mirror.resync_reason().is_some());

        // Installing a valid snapshot clears resync_reason.
        mirror
            .install_snapshot(base_snapshot(10, vec![]))
            .expect("install");
        assert_eq!(mirror.resync_reason(), None);
        assert_eq!(mirror.revision(), Some((EPOCH_A, 10)));

        // Duplicates (seq <= self.seq) are ignored without changing state.
        assert_eq!(
            mirror.apply_delta(delta(
                10,
                DeltaKind::MembershipAdd {
                    txid: hex_id(1),
                    first_seen: 100,
                },
            )),
            ApplyOutcome::DuplicateIgnored
        );
        assert_eq!(
            mirror.apply_delta(delta(
                5,
                DeltaKind::MembershipAdd {
                    txid: hex_id(2),
                    first_seen: 100,
                },
            )),
            ApplyOutcome::DuplicateIgnored
        );
        assert_eq!(mirror.tx_count(), 0);
        assert_eq!(mirror.resync_reason(), None);

        // Next sequential delta applies cleanly.
        assert_eq!(
            mirror.apply_delta(delta(
                11,
                DeltaKind::MembershipAdd {
                    txid: hex_id(3),
                    first_seen: 111,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(mirror.revision(), Some((EPOCH_A, 11)));
        assert!(mirror.has_tx(&hex_id(3)));

        // Sequence gap sets resync_reason and preserves state at seq 11.
        let coherent_at_11 = mirror.observable_state();
        assert!(matches!(
            mirror.apply_delta(delta(
                13,
                DeltaKind::MembershipAdd {
                    txid: hex_id(4),
                    first_seen: 113,
                },
            )),
            ApplyOutcome::ResyncNeeded(_)
        ));
        assert!(mirror.resync_reason().is_some());
        assert_eq!(mirror.revision(), Some((EPOCH_A, 11)));
        assert!(!mirror.has_tx(&hex_id(4)));
        assert_eq!(mirror.tx_count(), coherent_at_11.members.len());

        // Re-install snapshot to test epoch mismatch.
        mirror
            .install_snapshot(base_snapshot(11, vec![]))
            .expect("reinstall");
        let mut wrong_epoch =
            delta(12, DeltaKind::MembershipRemove { txids: vec![] });
        wrong_epoch.stream_epoch = EPOCH_B.to_string();
        assert!(matches!(
            mirror.apply_delta(wrong_epoch),
            ApplyOutcome::ResyncNeeded(_)
        ));
        assert_eq!(mirror.revision(), Some((EPOCH_A, 11)));

        // Re-install snapshot to test protocol version mismatch.
        mirror
            .install_snapshot(base_snapshot(11, vec![]))
            .expect("reinstall");
        let mut wrong_version =
            delta(12, DeltaKind::MembershipRemove { txids: vec![] });
        wrong_version.protocol_version = PROTOCOL_VERSION + 9;
        assert!(matches!(
            mirror.apply_delta(wrong_version),
            ApplyOutcome::ResyncNeeded(_)
        ));
        assert_eq!(mirror.revision(), Some((EPOCH_A, 11)));
    }

    #[test]
    fn delayed_tx_details_never_resurrects_removed_member() {
        let mut mirror = HeartbeatMirror::default();
        mirror
            .install_snapshot(base_snapshot(0, vec![]))
            .expect("install");

        let tx1 = hex_id(1);
        assert_eq!(
            mirror.apply_delta(delta(
                1,
                DeltaKind::MembershipAdd {
                    txid: tx1.clone(),
                    first_seen: 100,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(mirror.known_vbytes(), None);

        assert_eq!(
            mirror.apply_delta(delta(
                2,
                DeltaKind::MembershipRemove {
                    txids: vec![tx1.clone()],
                },
            )),
            ApplyOutcome::Applied
        );
        assert!(!mirror.has_tx(&tx1));
        assert_eq!(mirror.known_vbytes(), Some(0));

        // Delayed TxDetails arrives after removal: must NOT resurrect tx1.
        assert_eq!(
            mirror.apply_delta(delta(
                3,
                DeltaKind::TxDetails {
                    txid: tx1.clone(),
                    fee: 1000,
                    vsize: 141,
                    value: Some(50_000),
                    flags: TxFlags::WHALE,
                    op_return_text: Some("ghost".to_string()),
                    first_seen: None,
                },
            )),
            ApplyOutcome::Applied
        );
        assert!(!mirror.has_tx(&tx1));
        assert_eq!(mirror.tx_count(), 0);
        assert_eq!(mirror.get_tx(&tx1), None);
    }

    #[test]
    fn block_inclusion_respects_observation_fence_and_exact_txids() {
        let mut mirror = HeartbeatMirror::default();
        let tx_from_snap = hex_id(10);
        let tx_before_block = hex_id(11);
        let tx_readded_after_block = hex_id(12);
        let tx_unrelated_high_fee = hex_id(13);
        let tx_never_seen = hex_id(14);
        let block_hash = hex_id(0xbb);

        mirror
            .install_snapshot(base_snapshot(
                0,
                vec![CompactTx::pending(tx_from_snap.clone(), 100)],
            ))
            .expect("install");

        // Add tx_before_block and tx_unrelated_high_fee before BlockObserved.
        assert_eq!(
            mirror.apply_delta(delta(
                1,
                DeltaKind::MembershipAdd {
                    txid: tx_before_block.clone(),
                    first_seen: 101,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(
            mirror.apply_delta(delta(
                2,
                DeltaKind::MembershipAdd {
                    txid: tx_unrelated_high_fee.clone(),
                    first_seen: 102,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(
            mirror.apply_delta(delta(
                3,
                DeltaKind::TxDetails {
                    txid: tx_unrelated_high_fee.clone(),
                    fee: 9_999_999,
                    vsize: 100,
                    value: Some(1_000_000),
                    flags: TxFlags::FEE_OUTLIER,
                    op_return_text: None,
                    first_seen: None,
                },
            )),
            ApplyOutcome::Applied
        );

        // Observe the block.
        assert_eq!(
            mirror.apply_delta(delta(
                4,
                DeltaKind::BlockObserved {
                    hash: block_hash.clone(),
                    observed_at: 104,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(mirror.pending_blocks().len(), 1);
        assert_eq!(mirror.pending_blocks()[0].hash, block_hash);

        // Late TxDetails for tx_before_block arrives after BlockObserved:
        // must NOT move its observation order past BlockObserved.
        assert_eq!(
            mirror.apply_delta(delta(
                5,
                DeltaKind::TxDetails {
                    txid: tx_before_block.clone(),
                    fee: 500,
                    vsize: 140,
                    value: Some(10_000),
                    flags: TxFlags::NONE,
                    op_return_text: None,
                    first_seen: None,
                },
            )),
            ApplyOutcome::Applied
        );

        // tx_readded_after_block enters AFTER BlockObserved.
        assert_eq!(
            mirror.apply_delta(delta(
                6,
                DeltaKind::MembershipAdd {
                    txid: tx_readded_after_block.clone(),
                    first_seen: 106,
                },
            )),
            ApplyOutcome::Applied
        );

        // BlockInclusion lists tx_from_snap, tx_before_block,
        // tx_readded_after_block, and tx_never_seen.
        assert_eq!(
            mirror.apply_delta(delta(
                7,
                DeltaKind::BlockInclusion {
                    hash: block_hash.clone(),
                    height: 900_000,
                    txids: vec![
                        tx_from_snap.clone(),
                        tx_before_block.clone(),
                        tx_readded_after_block.clone(),
                        tx_never_seen.clone(),
                    ],
                },
            )),
            ApplyOutcome::Applied
        );

        // Snapshot member and pre-block member are confirmed & removed.
        assert!(!mirror.has_tx(&tx_from_snap));
        assert!(!mirror.has_tx(&tx_before_block));
        assert_eq!(
            mirror.confirmed_in(&tx_from_snap),
            Some((block_hash.as_str(), 900_000))
        );
        assert_eq!(
            mirror.confirmed_in(&tx_before_block),
            Some((block_hash.as_str(), 900_000))
        );

        // Member added after BlockObserved stays in mempool and is NOT confirmed.
        assert!(mirror.has_tx(&tx_readded_after_block));
        assert_eq!(mirror.confirmed_in(&tx_readded_after_block), None);

        // High-fee tx not in BlockInclusion stays in mempool (no fee-rank guessing).
        assert!(mirror.has_tx(&tx_unrelated_high_fee));
        assert_eq!(mirror.confirmed_in(&tx_unrelated_high_fee), None);

        // Unknown txid not in mempool is NOT inserted into confirmed.
        assert_eq!(mirror.confirmed_in(&tx_never_seen), None);

        // Pending block picked up height from BlockInclusion.
        assert_eq!(mirror.pending_blocks()[0].height, Some(900_000));
    }

    #[test]
    fn block_disconnected_retracts_only_matching_hash_without_readmission() {
        let mut mirror = HeartbeatMirror::default();
        let tx_a = hex_id(1);
        let tx_b = hex_id(2);
        let block_a = sample_block(900_000, 0xaa);
        let block_b = sample_block(900_001, 0xbb);

        mirror
            .install_snapshot(base_snapshot(
                0,
                vec![
                    CompactTx::pending(tx_a.clone(), 100),
                    CompactTx::pending(tx_b.clone(), 100),
                ],
            ))
            .expect("install");

        assert_eq!(
            mirror.apply_delta(delta(
                1,
                DeltaKind::BlockObserved {
                    hash: block_a.hash.clone(),
                    observed_at: 101,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(
            mirror.apply_delta(delta(
                2,
                DeltaKind::BlockDetails(block_a.clone())
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(
            mirror.apply_delta(delta(
                3,
                DeltaKind::BlockInclusion {
                    hash: block_a.hash.clone(),
                    height: block_a.height,
                    txids: vec![tx_a.clone()],
                },
            )),
            ApplyOutcome::Applied
        );

        assert_eq!(
            mirror.apply_delta(delta(
                4,
                DeltaKind::BlockObserved {
                    hash: block_b.hash.clone(),
                    observed_at: 104,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(
            mirror.apply_delta(delta(
                5,
                DeltaKind::BlockInclusion {
                    hash: block_b.hash.clone(),
                    height: block_b.height,
                    txids: vec![tx_b.clone()],
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(
            mirror.apply_delta(delta(
                6,
                DeltaKind::BlockDetails(block_b.clone())
            )),
            ApplyOutcome::Applied
        );

        assert_eq!(mirror.recent_blocks().len(), 2);
        assert_eq!(
            mirror.confirmed_in(&tx_a),
            Some((block_a.hash.as_str(), block_a.height))
        );
        assert_eq!(
            mirror.confirmed_in(&tx_b),
            Some((block_b.hash.as_str(), block_b.height))
        );

        // Disconnect block_b only.
        assert_eq!(
            mirror.apply_delta(delta(
                7,
                DeltaKind::BlockDisconnected {
                    hash: block_b.hash.clone(),
                },
            )),
            ApplyOutcome::Applied
        );

        // block_b removed from recent_blocks, block_a untouched.
        assert_eq!(mirror.recent_blocks(), std::slice::from_ref(&block_a));
        // tx_b confirmation retracted, tx_a confirmation untouched.
        assert_eq!(mirror.confirmed_in(&tx_b), None);
        assert_eq!(
            mirror.confirmed_in(&tx_a),
            Some((block_a.hash.as_str(), block_a.height))
        );
        // tx_b must NOT be re-admitted to members implicitly.
        assert!(!mirror.has_tx(&tx_b));
        assert_eq!(mirror.tx_count(), 0);
    }

    #[test]
    fn continuous_and_rejoining_mirrors_match_exactly_at_same_revision() {
        let block_1 = sample_block(900_000, 0x11);
        let block_2 = sample_block(900_001, 0x22);
        let tx1 = hex_id(1);
        let tx2 = hex_id(2);
        let tx3 = hex_id(3);
        let tx4 = hex_id(4);

        let initial = base_snapshot(
            0,
            vec![CompactTx::pending(tx1.clone(), 1_700_000_000)],
        );
        let stream = vec![
            delta(
                1,
                DeltaKind::TxDetails {
                    txid: tx1.clone(),
                    fee: 1_200,
                    vsize: 141,
                    value: Some(50_000),
                    flags: TxFlags::NONE,
                    op_return_text: None,
                    first_seen: None,
                },
            ),
            delta(
                2,
                DeltaKind::MembershipAdd {
                    txid: tx2.clone(),
                    first_seen: 1_700_000_002,
                },
            ),
            delta(
                3,
                DeltaKind::MembershipAdd {
                    txid: tx3.clone(),
                    first_seen: 1_700_000_003,
                },
            ),
            delta(
                4,
                DeltaKind::BlockObserved {
                    hash: block_1.hash.clone(),
                    observed_at: 1_700_000_004,
                },
            ),
            delta(
                5,
                DeltaKind::TxDetails {
                    txid: tx2.clone(),
                    fee: 0,
                    vsize: 200,
                    value: Some(0),
                    flags: TxFlags::OP_RETURN_MSG,
                    op_return_text: Some("anchored".to_string()),
                    first_seen: None,
                },
            ),
            // Mid-stream snapshot cutpoint at seq 5 (with block_1 still pending!).
            delta(
                6,
                DeltaKind::MembershipAdd {
                    txid: tx4.clone(),
                    first_seen: 1_700_000_006,
                },
            ),
            delta(
                7,
                DeltaKind::BlockInclusion {
                    hash: block_1.hash.clone(),
                    height: block_1.height,
                    txids: vec![tx1.clone(), tx4.clone()],
                },
            ),
            delta(8, DeltaKind::BlockDetails(block_1.clone())),
            delta(
                9,
                DeltaKind::MembershipRemove {
                    txids: vec![tx3.clone()],
                },
            ),
            delta(
                10,
                DeltaKind::CoverageChanged {
                    coverage: Coverage::Partial {
                        reason: "checkpoint pending".to_string(),
                    },
                },
            ),
            delta(
                11,
                DeltaKind::MetricsUpdated {
                    node_mempool: Some(NodeMempoolAggregate {
                        tx_count: 2,
                        vbytes: 341,
                        observed_at: 1_700_000_011,
                        stale: false,
                    }),
                },
            ),
            delta(
                12,
                DeltaKind::BlockObserved {
                    hash: block_2.hash.clone(),
                    observed_at: 1_700_000_012,
                },
            ),
            delta(13, DeltaKind::BlockDetails(block_2.clone())),
            delta(
                14,
                DeltaKind::BlockDisconnected {
                    hash: block_2.hash.clone(),
                },
            ),
            delta(
                15,
                DeltaKind::CoverageChanged {
                    coverage: Coverage::Complete,
                },
            ),
        ];

        let mut continuous = HeartbeatMirror::default();
        continuous.install_snapshot(initial).expect("install");

        let mut mid_snapshot = None;
        for d in &stream {
            assert_eq!(
                continuous.apply_delta(d.clone()),
                ApplyOutcome::Applied
            );
            if d.seq == 5 {
                mid_snapshot = continuous.to_snapshot(
                    "snap-5".to_string(),
                    1_700_000_005,
                    Some(1_700_000_005),
                );
            }
        }

        let mut rejoining = HeartbeatMirror::default();
        rejoining
            .install_snapshot(mid_snapshot.expect("mid snapshot"))
            .expect("install mid");

        // Replaying earlier deltas (seq <= 5) is ignored; trailing deltas 6..=15 apply.
        for d in &stream {
            let outcome = rejoining.apply_delta(d.clone());
            if d.seq <= 5 {
                assert_eq!(outcome, ApplyOutcome::DuplicateIgnored);
            } else {
                assert_eq!(outcome, ApplyOutcome::Applied);
            }
        }

        assert!(continuous.same_observable_state(&rejoining));
        assert_eq!(continuous.observable_state(), rejoining.observable_state());
        assert_eq!(continuous.revision(), Some((EPOCH_A, 15)));
        // tx4 arrived at seq 6 AFTER block_1 was observed at seq 4, so BOTH
        // mirrors must have preserved tx4 despite block_1's inclusion list.
        assert!(continuous.has_tx(&tx4));
        assert!(rejoining.has_tx(&tx4));
        assert!(continuous.has_tx(&tx2));
        assert!(!continuous.has_tx(&tx1));
        assert!(!continuous.has_tx(&tx3));
    }

    #[test]
    fn recent_blocks_capped_at_144_keeping_latest_heights() {
        let mut mirror = HeartbeatMirror::default();
        mirror
            .install_snapshot(base_snapshot(0, vec![]))
            .expect("install");

        for i in 0..160u64 {
            let block = CanonicalBlock {
                height: 800_000 + i,
                hash: format!("{i:064x}"),
                timestamp: 1_700_000_000 + i,
                tx_count: 100,
                total_fees: 1_000,
                size: 10_000,
                weight: 40_000,
                observed_at: 1_700_000_000 + i,
                miner: None,
            };
            assert_eq!(
                mirror
                    .apply_delta(delta(i + 1, DeltaKind::BlockDetails(block))),
                ApplyOutcome::Applied
            );
        }

        assert_eq!(mirror.recent_blocks().len(), MAX_RECENT_BLOCKS);
        assert_eq!(mirror.recent_blocks().first().unwrap().height, 800_016);
        assert_eq!(mirror.recent_blocks().last().unwrap().height, 800_159);
    }

    #[test]
    fn lookup_tx_vbytes_and_flags_helpers() {
        let flags =
            TxFlags::from_bools(true, false, true, false, false, true, false);
        assert_eq!(
            flags,
            TxFlags::WHALE | TxFlags::CONSOLIDATION | TxFlags::ROUND_NUMBER
        );
        assert!(TxFlags::contains(flags, TxFlags::WHALE));
        assert!(!TxFlags::contains(flags, TxFlags::FAN_OUT));
        assert!(TxFlags::is_notable(flags));
        assert!(!TxFlags::is_notable(TxFlags::NONE));

        let mut mirror = HeartbeatMirror::default();
        assert_eq!(mirror.known_vbytes(), None);
        assert_eq!(mirror.exact_vbytes(), None);

        let tx1 = hex_id(1);
        let tx2 = hex_id(2);
        let tx3 = hex_id(3);
        let block_hash = hex_id(0xcc);

        mirror
            .install_snapshot(base_snapshot(
                0,
                vec![
                    CompactTx::pending(tx1.clone(), 100),
                    CompactTx::pending(tx2.clone(), 101),
                ],
            ))
            .expect("install");
        assert_eq!(mirror.known_vbytes(), None);
        assert_eq!(mirror.exact_vbytes(), None);
        assert_eq!(mirror.pending_details_count(), 2);

        assert_eq!(
            mirror.apply_delta(delta(
                1,
                DeltaKind::TxDetails {
                    txid: tx1.clone(),
                    fee: 250,
                    vsize: 140,
                    value: Some(10_000),
                    flags,
                    op_return_text: None,
                    first_seen: None,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(mirror.known_vbytes(), Some(140));
        assert_eq!(mirror.exact_vbytes(), None);
        assert_eq!(mirror.pending_details_count(), 1);

        assert_eq!(
            mirror.apply_delta(delta(
                2,
                DeltaKind::BlockObserved {
                    hash: block_hash.clone(),
                    observed_at: 102,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(
            mirror.apply_delta(delta(
                3,
                DeltaKind::BlockInclusion {
                    hash: block_hash.clone(),
                    height: 900_500,
                    txids: vec![tx2.clone()],
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(mirror.known_vbytes(), Some(140));
        assert_eq!(mirror.exact_vbytes(), Some(140));

        assert!(matches!(
            mirror.lookup_tx(&tx1),
            TxLookupStatus::Unconfirmed(ref tx) if tx.txid == tx1 && tx.flags == flags
        ));
        assert_eq!(
            mirror.lookup_tx(&tx2),
            TxLookupStatus::Confirmed {
                txid: tx2,
                block_hash,
                block_height: 900_500,
            }
        );
        assert_eq!(
            mirror.lookup_tx(&tx3),
            TxLookupStatus::Absent {
                coverage: Coverage::Complete,
            }
        );
    }

    #[test]
    fn snapshot_preserves_post_block_add_ordering_across_block_inclusion() {
        let tx_before = hex_id(0x01);
        let tx_after = hex_id(0x02);
        let tx_after_b2 = hex_id(0x03);
        let block_1 = hex_id(0xb1);
        let block_2 = hex_id(0xb2);

        let mut continuous = HeartbeatMirror::default();
        continuous
            .install_snapshot(base_snapshot(
                0,
                vec![CompactTx::pending(tx_before.clone(), 1_700_000_000)],
            ))
            .expect("install initial");

        // Advance through seq 1..3 so BlockObserved(B1) lands at seq 4 and
        // MembershipAdd(tx_after) lands at seq 5.
        for seq in 1..=3u64 {
            assert_eq!(
                continuous.apply_delta(delta(
                    seq,
                    DeltaKind::MembershipRemove { txids: vec![] },
                )),
                ApplyOutcome::Applied
            );
        }
        assert_eq!(
            continuous.apply_delta(delta(
                4,
                DeltaKind::BlockObserved {
                    hash: block_1.clone(),
                    observed_at: 1_700_000_004,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(
            continuous.apply_delta(delta(
                5,
                DeltaKind::MembershipAdd {
                    txid: tx_after.clone(),
                    first_seen: 1_700_000_005,
                },
            )),
            ApplyOutcome::Applied
        );

        // Snapshot at seq 5 (after BlockObserved(B1) and MembershipAdd(X)),
        // round-tripped through JSON to verify wire fidelity and compactness.
        let snap_5 = continuous
            .to_snapshot("snap-5".to_string(), 1_700_000_005, None)
            .expect("to_snapshot");
        let snap_5_json =
            serde_json::to_string(&snap_5).expect("serialize snap_5");
        let tx_before_in_snap = snap_5
            .transactions
            .iter()
            .find(|t| t.txid == tx_before)
            .expect("tx_before in snap");
        let tx_after_in_snap = snap_5
            .transactions
            .iter()
            .find(|t| t.txid == tx_after)
            .expect("tx_after in snap");
        assert_eq!(tx_before_in_snap.observed_order, 0);
        assert_eq!(tx_after_in_snap.observed_order, 1);
        assert_eq!(snap_5.pending_blocks[0].observed_order, 0);
        assert!(
            snap_5_json.contains(&format!("[\"{tx_before}\",1700000000]")),
            "pre-block tx must serialize with 0 extra bytes: {snap_5_json}"
        );

        let mut rejoining = HeartbeatMirror::default();
        let snap_5_rt: HeartbeatSnapshot =
            serde_json::from_str(&snap_5_json).expect("deserialize snap_5");
        rejoining
            .install_snapshot(snap_5_rt)
            .expect("rejoining install snap_5");
        assert!(continuous.same_observable_state(&rejoining));

        // Seq 6: BlockInclusion(B1, [tx_before, tx_after]).
        // Both mirrors must evict tx_before and KEEP tx_after!
        let inc_b1 = delta(
            6,
            DeltaKind::BlockInclusion {
                hash: block_1.clone(),
                height: 900_001,
                txids: vec![tx_before.clone(), tx_after.clone()],
            },
        );
        assert_eq!(
            continuous.apply_delta(inc_b1.clone()),
            ApplyOutcome::Applied
        );
        assert_eq!(rejoining.apply_delta(inc_b1), ApplyOutcome::Applied);
        assert!(continuous.same_observable_state(&rejoining));
        assert!(!continuous.has_tx(&tx_before));
        assert!(!rejoining.has_tx(&tx_before));
        assert!(continuous.has_tx(&tx_after));
        assert!(rejoining.has_tx(&tx_after));

        // Verify post-snapshot clock initialization: a second pending block B2
        // observed at seq 7 and tx_after_b2 added at seq 8 must preserve order
        // across another snapshot cutpoint too.
        let obs_b2 = delta(
            7,
            DeltaKind::BlockObserved {
                hash: block_2.clone(),
                observed_at: 1_700_000_007,
            },
        );
        let add_after_b2 = delta(
            8,
            DeltaKind::MembershipAdd {
                txid: tx_after_b2.clone(),
                first_seen: 1_700_000_008,
            },
        );
        for d in [obs_b2, add_after_b2] {
            assert_eq!(
                continuous.apply_delta(d.clone()),
                ApplyOutcome::Applied
            );
            assert_eq!(rejoining.apply_delta(d), ApplyOutcome::Applied);
        }

        let snap_8_json = serde_json::to_string(
            &rejoining
                .to_snapshot("snap-8".to_string(), 1_700_000_008, None)
                .expect("snap_8"),
        )
        .expect("serialize snap_8");
        let mut third = HeartbeatMirror::default();
        third
            .install_snapshot(
                serde_json::from_str(&snap_8_json).expect("deserialize snap_8"),
            )
            .expect("install snap_8");

        let inc_b2 = delta(
            9,
            DeltaKind::BlockInclusion {
                hash: block_2,
                height: 900_002,
                txids: vec![tx_after.clone(), tx_after_b2.clone()],
            },
        );
        assert_eq!(
            continuous.apply_delta(inc_b2.clone()),
            ApplyOutcome::Applied
        );
        assert_eq!(
            rejoining.apply_delta(inc_b2.clone()),
            ApplyOutcome::Applied
        );
        assert_eq!(third.apply_delta(inc_b2), ApplyOutcome::Applied);

        assert!(continuous.same_observable_state(&rejoining));
        assert!(continuous.same_observable_state(&third));
        assert!(!third.has_tx(&tx_after));
        assert!(third.has_tx(&tx_after_b2));
    }

    #[test]
    fn rejoining_mirror_after_block_inclusion_preserves_confirmations_and_observable_state(
    ) {
        let tx_mined_1 = hex_id(0x11);
        let tx_mined_2 = hex_id(0x12);
        let tx_remaining = hex_id(0x13);
        let block_1 = sample_block(900_100, 0xb1);

        let mut continuous = HeartbeatMirror::default();
        continuous
            .install_snapshot(base_snapshot(
                0,
                vec![
                    CompactTx::pending(tx_mined_1.clone(), 1_700_000_000),
                    CompactTx::pending(tx_mined_2.clone(), 1_700_000_001),
                    CompactTx::pending(tx_remaining.clone(), 1_700_000_002),
                ],
            ))
            .expect("install initial");

        assert_eq!(
            continuous.apply_delta(delta(
                1,
                DeltaKind::BlockObserved {
                    hash: block_1.hash.clone(),
                    observed_at: 1_700_000_010,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(
            continuous.apply_delta(delta(
                2,
                DeltaKind::BlockInclusion {
                    hash: block_1.hash.clone(),
                    height: block_1.height,
                    txids: vec![tx_mined_2.clone(), tx_mined_1.clone()],
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(
            continuous.apply_delta(delta(
                3,
                DeltaKind::BlockDetails(block_1.clone())
            )),
            ApplyOutcome::Applied
        );

        // Snapshot taken AFTER BlockInclusion: rejoining mirror installs this
        // snapshot without ever replaying BlockInclusion.
        let snap_after = continuous
            .to_snapshot(
                "snap-after-inclusion".to_string(),
                1_700_000_020,
                None,
            )
            .expect("to_snapshot");
        assert_eq!(snap_after.recent_confirmations.len(), 1);
        assert_eq!(snap_after.recent_confirmations[0].hash, block_1.hash);
        assert_eq!(snap_after.recent_confirmations[0].height, block_1.height);
        // Sorted deterministically by txid.
        assert_eq!(
            snap_after.recent_confirmations[0].txids,
            vec![tx_mined_1.clone(), tx_mined_2.clone()]
        );

        let snap_json =
            serde_json::to_string(&snap_after).expect("serialize snapshot");
        let snap_rt: HeartbeatSnapshot =
            serde_json::from_str(&snap_json).expect("deserialize snapshot");

        let mut rejoining = HeartbeatMirror::default();
        rejoining
            .install_snapshot(snap_rt)
            .expect("install post-inclusion snapshot");

        assert!(continuous.same_observable_state(&rejoining));
        assert_eq!(
            rejoining.confirmed_in(&tx_mined_1),
            Some((block_1.hash.as_str(), block_1.height))
        );
        assert_eq!(
            rejoining.confirmed_in(&tx_mined_2),
            Some((block_1.hash.as_str(), block_1.height))
        );
        assert!(rejoining.has_tx(&tx_remaining));
        assert_eq!(rejoining.confirmed_in(&tx_remaining), None);
    }

    #[test]
    fn block_inclusion_fails_closed_for_unknown_or_disconnected_hash() {
        let tx1 = hex_id(0x21);
        let tx2 = hex_id(0x22);
        let tx3 = hex_id(0x23);
        let unknown_hash = hex_id(0xe1);
        let disconnected_hash = hex_id(0xe2);

        let mut mirror = HeartbeatMirror::default();
        mirror
            .install_snapshot(base_snapshot(
                0,
                vec![
                    CompactTx::pending(tx1.clone(), 1_700_000_000),
                    CompactTx::pending(tx2.clone(), 1_700_000_001),
                ],
            ))
            .expect("install initial");

        // BlockInclusion for an unobserved hash must fail closed while
        // still advancing seq cleanly.
        assert_eq!(
            mirror.apply_delta(delta(
                1,
                DeltaKind::BlockInclusion {
                    hash: unknown_hash,
                    height: 900_200,
                    txids: vec![tx1.clone()],
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(mirror.revision(), Some((EPOCH_A, 1)));
        assert!(mirror.has_tx(&tx1));
        assert!(mirror.has_tx(&tx2));
        assert_eq!(mirror.confirmed_count(), 0);

        // Observe a block, then disconnect it before a late BlockInclusion arrives.
        assert_eq!(
            mirror.apply_delta(delta(
                2,
                DeltaKind::BlockObserved {
                    hash: disconnected_hash.clone(),
                    observed_at: 1_700_000_010,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(
            mirror.apply_delta(delta(
                3,
                DeltaKind::BlockDisconnected {
                    hash: disconnected_hash.clone(),
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(
            mirror.apply_delta(delta(
                4,
                DeltaKind::BlockInclusion {
                    hash: disconnected_hash,
                    height: 900_201,
                    txids: vec![tx1.clone(), tx2.clone()],
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(mirror.revision(), Some((EPOCH_A, 4)));
        assert!(mirror.has_tx(&tx1));
        assert!(mirror.has_tx(&tx2));
        assert_eq!(mirror.confirmed_count(), 0);

        // Subsequent deltas still apply with no sequence gap.
        assert_eq!(
            mirror.apply_delta(delta(
                5,
                DeltaKind::MembershipAdd {
                    txid: tx3.clone(),
                    first_seen: 1_700_000_020,
                },
            )),
            ApplyOutcome::Applied
        );
        assert_eq!(mirror.revision(), Some((EPOCH_A, 5)));
        assert!(mirror.has_tx(&tx3));
    }

    #[test]
    fn tx_flags_from_notable_type_maps_all_known_variants() {
        assert_eq!(TxFlags::from_notable_type(Some("whale")), TxFlags::WHALE);
        assert_eq!(
            TxFlags::from_notable_type(Some("fee_outlier")),
            TxFlags::FEE_OUTLIER
        );
        assert_eq!(
            TxFlags::from_notable_type(Some("consolidation")),
            TxFlags::CONSOLIDATION
        );
        assert_eq!(
            TxFlags::from_notable_type(Some("fan_out")),
            TxFlags::FAN_OUT
        );
        assert_eq!(
            TxFlags::from_notable_type(Some("large_inscription")),
            TxFlags::LARGE_INSCRIPTION
        );
        assert_eq!(
            TxFlags::from_notable_type(Some("round_number")),
            TxFlags::ROUND_NUMBER
        );
        assert_eq!(
            TxFlags::from_notable_type(Some("op_return_msg")),
            TxFlags::OP_RETURN_MSG
        );
        assert_eq!(TxFlags::from_notable_type(Some("unknown")), TxFlags::NONE);
        assert_eq!(TxFlags::from_notable_type(None), TxFlags::NONE);
    }

    #[test]
    fn apply_delta_with_effect_filters_fenced_inclusions_non_members_and_removals(
    ) {
        let mut mirror = HeartbeatMirror::default();
        let tx_before = hex_id(0x01);
        let tx_after = hex_id(0x02);
        let tx_ghost = hex_id(0x03);
        let tx_remove = hex_id(0x04);
        let block_hash = hex_id(0xb1);
        let unknown_hash = hex_id(0xb2);

        mirror
            .install_snapshot(base_snapshot(
                0,
                vec![
                    CompactTx::pending(tx_before.clone(), 100),
                    CompactTx::pending(tx_remove.clone(), 101),
                ],
            ))
            .expect("install");

        // TxDetails for a non-member is ignored and emits no effect.
        let (outcome, effect) = mirror.apply_delta_with_effect(&delta(
            1,
            DeltaKind::TxDetails {
                txid: tx_ghost.clone(),
                fee: 500,
                vsize: 140,
                value: Some(10_000),
                flags: TxFlags::WHALE,
                op_return_text: Some("ghost".to_string()),
                first_seen: None,
            },
        ));
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(effect, None);

        // TxDetails for a live member returns the updated CompactTx.
        let (outcome, effect) = mirror.apply_delta_with_effect(&delta(
            2,
            DeltaKind::TxDetails {
                txid: tx_before.clone(),
                fee: 750,
                vsize: 150,
                value: Some(25_000),
                flags: TxFlags::ROUND_NUMBER,
                op_return_text: None,
                first_seen: None,
            },
        ));
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(
            effect,
            Some(ApplyEffect::TxDetails {
                tx: CompactTx {
                    txid: tx_before.clone(),
                    first_seen: 100,
                    fee: Some(750),
                    vsize: Some(150),
                    value: Some(25_000),
                    flags: TxFlags::ROUND_NUMBER,
                    op_return_text: None,
                    observed_order: 0,
                },
            })
        );

        // MembershipRemove emits only txids actually removed from self.members.
        let (outcome, effect) = mirror.apply_delta_with_effect(&delta(
            3,
            DeltaKind::MembershipRemove {
                txids: vec![tx_ghost.clone(), tx_remove.clone()],
            },
        ));
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(
            effect,
            Some(ApplyEffect::MembershipRemove {
                txids: vec![tx_remove],
            })
        );

        // BlockObserved then MembershipAdd creates a post-block tx that must
        // not be confirmed by BlockInclusion for `block_hash`.
        let (outcome, effect) = mirror.apply_delta_with_effect(&delta(
            4,
            DeltaKind::BlockObserved {
                hash: block_hash.clone(),
                observed_at: 200,
            },
        ));
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(
            effect,
            Some(ApplyEffect::BlockObserved {
                hash: block_hash.clone(),
                observed_at: 200,
            })
        );

        let (outcome, effect) = mirror.apply_delta_with_effect(&delta(
            5,
            DeltaKind::MembershipAdd {
                txid: tx_after.clone(),
                first_seen: 205,
            },
        ));
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(
            effect,
            Some(ApplyEffect::MembershipAdd {
                txid: tx_after.clone(),
                first_seen: 205,
            })
        );

        // BlockInclusion returns ONLY tx_before (passed fence); tx_after and
        // tx_ghost are excluded.
        let (outcome, effect) = mirror.apply_delta_with_effect(&delta(
            6,
            DeltaKind::BlockInclusion {
                hash: block_hash.clone(),
                height: 900_000,
                txids: vec![tx_before.clone(), tx_after.clone(), tx_ghost],
            },
        ));
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(
            effect,
            Some(ApplyEffect::BlockInclusion {
                hash: block_hash,
                height: 900_000,
                confirmed_txids: vec![tx_before],
            })
        );

        // Unknown block hash fails closed with empty confirmed_txids.
        let (outcome, effect) = mirror.apply_delta_with_effect(&delta(
            7,
            DeltaKind::BlockInclusion {
                hash: unknown_hash.clone(),
                height: 900_001,
                txids: vec![tx_after],
            },
        ));
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(
            effect,
            Some(ApplyEffect::BlockInclusion {
                hash: unknown_hash,
                height: 900_001,
                confirmed_txids: vec![],
            })
        );

        // Duplicate delta returns no effect.
        let (outcome, effect) = mirror.apply_delta_with_effect(&delta(
            7,
            DeltaKind::CoverageChanged {
                coverage: Coverage::Complete,
            },
        ));
        assert_eq!(outcome, ApplyOutcome::DuplicateIgnored);
        assert_eq!(effect, None);
    }

    #[test]
    fn hydrate_member_details_is_details_only_and_preserves_known_fields() {
        let mut mirror = HeartbeatMirror::default();
        let tx_known = hex_id(0x10);
        let tx_unknown = hex_id(0x20);

        mirror
            .install_snapshot(base_snapshot(
                0,
                vec![CompactTx::pending(tx_known.clone(), 0)],
            ))
            .expect("install");

        // Never admits unknown txids.
        assert!(!mirror.hydrate_member_details(&TxMetadataHydration {
            txid: tx_unknown.clone(),
            first_seen: Some(1_700_000_000),
            fee: Some(500),
            vsize: Some(141),
            value: Some(50_000),
            flags: TxFlags::WHALE,
            op_return_text: Some("ghost".to_string()),
        }));
        assert!(!mirror.has_tx(&tx_unknown));

        // Hydrate fee, vsize, value, flags, op_return_text, and fill first_seen == 0.
        assert!(mirror.hydrate_member_details(&TxMetadataHydration {
            txid: tx_known.clone(),
            first_seen: Some(1_700_000_500),
            fee: Some(1_200),
            vsize: Some(200),
            value: Some(100_000),
            flags: TxFlags::WHALE,
            op_return_text: Some("hello".to_string()),
        }));

        let tx = mirror.get_tx(&tx_known).expect("member exists");
        assert_eq!(tx.first_seen, 1_700_000_500);
        assert_eq!(tx.fee, Some(1_200));
        assert_eq!(tx.vsize, Some(200));
        assert_eq!(tx.value, Some(100_000));
        assert_eq!(tx.flags, TxFlags::WHALE);
        assert_eq!(tx.op_return_text.as_deref(), Some("hello"));

        // Re-hydrating with None value/op_return and later first_seen is a no-op
        // and never overwrites known Some fields with None.
        assert!(!mirror.hydrate_member_details(&TxMetadataHydration {
            txid: tx_known.clone(),
            first_seen: Some(1_700_000_999),
            fee: Some(1_200),
            vsize: Some(200),
            value: None,
            flags: TxFlags::NONE,
            op_return_text: None,
        }));
        let tx = mirror.get_tx(&tx_known).expect("member exists");
        assert_eq!(tx.first_seen, 1_700_000_500);
        assert_eq!(tx.value, Some(100_000));
        assert_eq!(tx.op_return_text.as_deref(), Some("hello"));

        // Earlier first_seen must NEVER pull a known first_seen > 0 earlier,
        // while additional flag bits still merge cleanly.
        assert!(!mirror.hydrate_member_details(&TxMetadataHydration {
            txid: tx_known.clone(),
            first_seen: Some(1_700_000_100),
            fee: None,
            vsize: None,
            value: None,
            flags: TxFlags::NONE,
            op_return_text: None,
        }));
        assert!(mirror.hydrate_member_details(&TxMetadataHydration {
            txid: tx_known.clone(),
            first_seen: Some(1_700_000_100),
            fee: None,
            vsize: None,
            value: None,
            flags: TxFlags::OP_RETURN_MSG,
            op_return_text: None,
        }));
        let tx = mirror.get_tx(&tx_known).expect("member exists");
        assert_eq!(tx.first_seen, 1_700_000_500);
        assert_eq!(tx.flags, TxFlags::WHALE | TxFlags::OP_RETURN_MSG);
        assert_eq!(tx.fee, Some(1_200));
        assert_eq!(tx.vsize, Some(200));
        assert_eq!(tx.value, Some(100_000));
    }

    #[test]
    fn tx_details_delta_supports_optional_value_first_seen_and_merges_flags() {
        let mut mirror = HeartbeatMirror::default();
        let tx1 = hex_id(0x31);
        mirror
            .install_snapshot(base_snapshot(
                0,
                vec![CompactTx::pending(tx1.clone(), 0)],
            ))
            .expect("install");

        // Partial TxDetails without value populates fee, vsize, and first_seen (from 0).
        let (outcome, effect) = mirror.apply_delta_with_effect(&delta(
            1,
            DeltaKind::TxDetails {
                txid: tx1.clone(),
                fee: 500,
                vsize: 140,
                value: None,
                flags: TxFlags::WHALE,
                op_return_text: Some("first".to_string()),
                first_seen: Some(1_700_000_500),
            },
        ));
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert!(effect.is_some());
        let tx = mirror.get_tx(&tx1).unwrap();
        assert_eq!(tx.fee, Some(500));
        assert_eq!(tx.vsize, Some(140));
        assert_eq!(tx.value, None);
        assert_eq!(tx.flags, TxFlags::WHALE);
        assert_eq!(tx.op_return_text.as_deref(), Some("first"));
        assert_eq!(tx.first_seen, 1_700_000_500);

        // Follow-up TxDetails with value: Some, flags, op_return_text: None, and earlier
        // first_seen preserves existing op_return_text and first_seen > 0 while merging flags.
        let (outcome, _) = mirror.apply_delta_with_effect(&delta(
            2,
            DeltaKind::TxDetails {
                txid: tx1.clone(),
                fee: 500,
                vsize: 140,
                value: Some(250_000),
                flags: TxFlags::OP_RETURN_MSG,
                op_return_text: None,
                first_seen: Some(1_700_000_100),
            },
        ));
        assert_eq!(outcome, ApplyOutcome::Applied);
        let tx = mirror.get_tx(&tx1).unwrap();
        assert_eq!(tx.value, Some(250_000));
        assert_eq!(tx.flags, TxFlags::WHALE | TxFlags::OP_RETURN_MSG);
        assert_eq!(tx.op_return_text.as_deref(), Some("first"));
        assert_eq!(tx.first_seen, 1_700_000_500);

        // Follow-up TxDetails with value: None preserves previously known value.
        let (outcome, _) = mirror.apply_delta_with_effect(&delta(
            3,
            DeltaKind::TxDetails {
                txid: tx1.clone(),
                fee: 500,
                vsize: 140,
                value: None,
                flags: TxFlags::NONE,
                op_return_text: None,
                first_seen: None,
            },
        ));
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(mirror.get_tx(&tx1).unwrap().value, Some(250_000));
    }

    #[test]
    fn search_tx_supports_exact_prefix_case_and_confirmed_matches() {
        let mut mirror = HeartbeatMirror::default();
        let unconfirmed_a = format!("abcd1234{}", "1".repeat(56));
        let unconfirmed_b = format!("abcd1234{}", "2".repeat(56));
        let confirmed_lo = format!("deadbeef{}", "0".repeat(56));
        let confirmed_hi = format!("deadbeef{}", "f".repeat(56));
        let block_hash = hex_id(0xaa);

        mirror
            .install_snapshot(base_snapshot(
                0,
                vec![
                    CompactTx::pending(unconfirmed_a.clone(), 100),
                    CompactTx::pending(unconfirmed_b.clone(), 101),
                    CompactTx::pending(confirmed_lo.clone(), 102),
                    CompactTx::pending(confirmed_hi.clone(), 103),
                ],
            ))
            .expect("install");

        mirror.apply_delta(delta(
            1,
            DeltaKind::BlockObserved {
                hash: block_hash.clone(),
                observed_at: 200,
            },
        ));
        mirror.apply_delta(delta(
            2,
            DeltaKind::BlockInclusion {
                hash: block_hash.clone(),
                height: 900_123,
                txids: vec![confirmed_hi.clone(), confirmed_lo.clone()],
            },
        ));

        // Invalid queries: empty, whitespace, non-hex, short prefix (< 8 chars).
        assert_eq!(mirror.search_tx(""), None);
        assert_eq!(mirror.search_tx("   "), None);
        assert_eq!(mirror.search_tx("abcd123g"), None);
        assert_eq!(mirror.search_tx("abcd123"), None);
        assert_eq!(mirror.search_tx("deadbee"), None);

        // Exact match with trimming and uppercase normalization.
        assert!(matches!(
            mirror.search_tx(&format!("  {}  ", unconfirmed_b.to_ascii_uppercase())),
            Some(MirrorTxMatch::Unconfirmed(tx)) if tx.txid == unconfirmed_b
        ));
        assert_eq!(
            mirror.search_tx(&confirmed_hi.to_ascii_uppercase()),
            Some(MirrorTxMatch::Confirmed {
                txid: confirmed_hi,
                block_hash: block_hash.clone(),
                block_height: 900_123,
            })
        );

        // Prefix match (>= 8 chars) picks lowest txid deterministically.
        assert!(matches!(
            mirror.search_tx("ABCD1234"),
            Some(MirrorTxMatch::Unconfirmed(tx)) if tx.txid == unconfirmed_a
        ));
        assert_eq!(
            mirror.search_tx("deadbeef"),
            Some(MirrorTxMatch::Confirmed {
                txid: confirmed_lo,
                block_hash,
                block_height: 900_123,
            })
        );

        // Non-matching prefix returns None.
        assert_eq!(mirror.search_tx("cafebabe"), None);
    }

    #[test]
    fn ordered_members_puts_unknown_time_first_then_by_first_seen_and_txid() {
        let mut mirror = HeartbeatMirror::default();
        let tx_u_b = hex_id(0x0b);
        let tx_u_a = hex_id(0x0a);
        let tx_t1_b = hex_id(0x1b);
        let tx_t1_a = hex_id(0x1a);
        let tx_t2 = hex_id(0x20);

        mirror
            .install_snapshot(base_snapshot(
                0,
                vec![
                    CompactTx::pending(tx_t2.clone(), 200),
                    CompactTx::pending(tx_u_b.clone(), 0),
                    CompactTx::pending(tx_t1_b.clone(), 100),
                    CompactTx::pending(tx_u_a.clone(), 0),
                    CompactTx::pending(tx_t1_a.clone(), 100),
                ],
            ))
            .expect("install");

        let ordered: Vec<&str> = mirror
            .ordered_members()
            .into_iter()
            .map(|tx| tx.txid.as_str())
            .collect();
        assert_eq!(
            ordered,
            vec![
                tx_u_a.as_str(),
                tx_u_b.as_str(),
                tx_t1_a.as_str(),
                tx_t1_b.as_str(),
                tx_t2.as_str(),
            ]
        );
    }

    #[test]
    fn queue_position_ranks_by_exact_fee_rate_handles_ties_zero_and_block_boundaries(
    ) {
        let mut mirror = HeartbeatMirror::default();
        let tx_high = hex_id(0x01); // 10 sat/vB, 400_000 vB
        let tx_mid_a = hex_id(0x02); // 5 sat/vB, 600_000 vB
        let tx_mid_b = hex_id(0x03); // 5 sat/vB (tie with mid_a), 200 vB
        let tx_low = hex_id(0x04); // 1 sat/vB, 200 vB
        let tx_zero = hex_id(0x05); // 0 sat/vB (TRUC), 150 vB
        let tx_pending = hex_id(0x06); // fee/vsize unknown

        let make_tx = |id: String, first_seen: u64, fee: u64, vsize: u32| {
            let mut t = CompactTx::pending(id, first_seen);
            t.fee = Some(fee);
            t.vsize = Some(vsize);
            t
        };
        let snap = base_snapshot(
            1,
            vec![
                make_tx(tx_high.clone(), 100, 4_000_000, 400_000),
                make_tx(tx_mid_a.clone(), 101, 3_000_000, 600_000),
                make_tx(tx_mid_b.clone(), 102, 1_000, 200),
                make_tx(tx_low.clone(), 103, 200, 200),
                make_tx(tx_zero.clone(), 104, 0, 150),
                CompactTx::pending(tx_pending.clone(), 105),
            ],
        );
        mirror.install_snapshot(snap).unwrap();

        // Top fee rate: 0 ahead, ~next block, 1 unenriched excluded.
        assert_eq!(
            mirror.queue_position(&tx_high),
            Some(QueuePosition {
                txs_ahead: 0,
                vbytes_ahead: 0,
                est_blocks: 1,
                unenriched_count: 1,
            })
        );

        // Mid tier (5 sat/vB): only tx_high (400k vB) is strictly ahead;
        // tie tx_mid_a is NOT counted ahead. 400_000 + 200 <= 1_000_000 -> 1 block.
        assert_eq!(
            mirror.queue_position(&tx_mid_b),
            Some(QueuePosition {
                txs_ahead: 1,
                vbytes_ahead: 400_000,
                est_blocks: 1,
                unenriched_count: 1,
            })
        );

        // Low tier (1 sat/vB): high (400k) + mid_a (600k) + mid_b (200) = 1_000_200 vB ahead
        // + 200 vB self = 1_000_400 vB -> crosses 1 vMB boundary into ~2 blocks.
        assert_eq!(
            mirror.queue_position(&tx_low),
            Some(QueuePosition {
                txs_ahead: 3,
                vbytes_ahead: 1_000_200,
                est_blocks: 2,
                unenriched_count: 1,
            })
        );

        // 0-sat TRUC tx: all 4 positive-fee txs ahead.
        assert_eq!(
            mirror.queue_position(&tx_zero),
            Some(QueuePosition {
                txs_ahead: 4,
                vbytes_ahead: 1_000_400,
                est_blocks: 2,
                unenriched_count: 1,
            })
        );

        // Pending-fee tx and absent tx return None.
        assert_eq!(mirror.queue_position(&tx_pending), None);
        assert_eq!(mirror.queue_position(&hex_id(0xff)), None);
    }

    #[test]
    fn fee_band_summary_classifies_exact_boundaries_and_sums_vbytes() {
        assert_eq!(fee_band_index(None, Some(200)), 0);
        assert_eq!(fee_band_index(Some(500), None), 0);
        assert_eq!(fee_band_index(Some(500), Some(0)), 0);
        assert_eq!(fee_band_index(Some(0), Some(200)), 1);
        assert_eq!(fee_band_index(Some(199), Some(200)), 1);
        assert_eq!(fee_band_index(Some(200), Some(200)), 2); // exact 1.0 sat/vB -> 1-2
        assert_eq!(fee_band_index(Some(399), Some(200)), 2);
        assert_eq!(fee_band_index(Some(400), Some(200)), 3); // exact 2.0 sat/vB -> 2-5
        assert_eq!(fee_band_index(Some(999), Some(200)), 3);
        assert_eq!(fee_band_index(Some(1_000), Some(200)), 4); // exact 5.0 sat/vB -> 5-15
        assert_eq!(fee_band_index(Some(2_999), Some(200)), 4);
        assert_eq!(fee_band_index(Some(3_000), Some(200)), 5); // exact 15.0 sat/vB -> 15-50
        assert_eq!(fee_band_index(Some(9_999), Some(200)), 5);
        assert_eq!(fee_band_index(Some(10_000), Some(200)), 6); // exact 50.0 sat/vB -> 50+

        let make = |seed: u8, fee: Option<u64>, vsize: Option<u32>| {
            let mut t = CompactTx::pending(hex_id(seed), 100);
            t.fee = fee;
            t.vsize = vsize;
            t
        };
        let mut mirror = HeartbeatMirror::default();
        mirror
            .install_snapshot(base_snapshot(
                1,
                vec![
                    make(1, None, None),              // band 0 (pending)
                    make(2, Some(100), Some(200)),    // band 1 (< 1)
                    make(3, Some(250), Some(200)),    // band 2 (1-2)
                    make(4, Some(600), Some(200)),    // band 3 (2-5)
                    make(5, Some(1_500), Some(150)),  // band 4 (5-15)
                    make(6, Some(4_000), Some(200)),  // band 5 (15-50)
                    make(7, Some(20_000), Some(300)), // band 6 (50+)
                    make(8, Some(15_000), Some(100)), // band 6 (50+)
                ],
            ))
            .expect("install");

        let s = mirror.fee_band_summary();
        assert_eq!(
            s.bands[0],
            FeeBandStat {
                tx_count: 1,
                vbytes: 0
            }
        );
        assert_eq!(
            s.bands[1],
            FeeBandStat {
                tx_count: 1,
                vbytes: 200
            }
        );
        assert_eq!(
            s.bands[2],
            FeeBandStat {
                tx_count: 1,
                vbytes: 200
            }
        );
        assert_eq!(
            s.bands[3],
            FeeBandStat {
                tx_count: 1,
                vbytes: 200
            }
        );
        assert_eq!(
            s.bands[4],
            FeeBandStat {
                tx_count: 1,
                vbytes: 150
            }
        );
        assert_eq!(
            s.bands[5],
            FeeBandStat {
                tx_count: 1,
                vbytes: 200
            }
        );
        assert_eq!(
            s.bands[6],
            FeeBandStat {
                tx_count: 2,
                vbytes: 400
            }
        );
    }
}
