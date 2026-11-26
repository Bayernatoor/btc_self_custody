//! Heartbeat v2 browser client state, pure view helpers, and hydrate sync loop.
//!
//! Pure types and formatting/reconciliation helpers compile on both `ssr` and
//! `hydrate` targets so they are directly unit-tested under `cargo test --features ssr`.
//! The browser network loop (`HeartbeatClient`) compiles only under `hydrate`.

use serde::{Deserialize, Serialize};

use super::heartbeat_sync::{
    ApplyEffect, CanonicalBlock, CompactTx, ConfirmedBlock, Coverage,
    HeartbeatMirror, PendingBlock, QueuePosition, TxFlags,
};

// ---------------------------------------------------------------------------
// Shared view types (SSR + hydrate)
// ---------------------------------------------------------------------------

/// Lifecycle state of a transaction currently pinned in the Heartbeat detail panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PinnedTxStatus {
    Unconfirmed,
    Confirmed {
        block_hash: String,
        block_height: u64,
    },
    /// Departed the node's mempool without a known confirmation (RBF or eviction).
    Removed,
}

/// Compact input or output preview row returned by `/api/stats/tx/{txid}`
/// (capped server-side to 25 entries each).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct TxIoEntry {
    pub label: String,
    #[serde(default)]
    pub value: Option<u64>,
    /// Full copyable address or `txid:vout` outpoint when applicable.
    #[serde(default)]
    pub copy: Option<String>,
}

/// Deep node details fetched asynchronously from `/api/stats/tx/{txid}`.
#[derive(Clone, Debug, PartialEq)]
pub struct TxDeepDetails {
    pub inputs: Option<u64>,
    pub outputs: Option<u64>,
    pub size: Option<u64>,
    pub weight: Option<u64>,
    pub confirmations: Option<u64>,
    pub input_items: Vec<TxIoEntry>,
    pub output_items: Vec<TxIoEntry>,
    pub features: Vec<String>,
}

/// Reactive view model for the in-place pinned transaction panel.
#[derive(Clone, Debug, PartialEq)]
pub struct PinnedTxView {
    pub txid: String,
    pub status: PinnedTxStatus,
    pub fee: Option<u64>,
    pub vsize: Option<u32>,
    pub value: Option<u64>,
    /// Unix timestamp in seconds when first seen; `0` means already waiting (time unknown).
    pub first_seen: u64,
    pub flags: u8,
    pub op_return_text: Option<String>,
    pub on_canvas: bool,
    pub deep: Option<TxDeepDetails>,
    pub deep_loading: bool,
    /// Estimated solo-fee-rate queue depth while `status == Unconfirmed`.
    pub queue_position: Option<QueuePosition>,
}

impl PinnedTxView {
    /// Fee rate in sat/vB when both fee and a non-zero vsize are known.
    pub fn fee_rate_sat_per_vb(&self) -> Option<f64> {
        let fee = self.fee?;
        let vsize = self.vsize?;
        (vsize > 0).then(|| fee as f64 / f64::from(vsize))
    }
}

/// Classification of a user query in the unified Heartbeat search bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchQuery {
    /// Explicit `tx:` prefix (1..=64 hex) or bare 8..=63 hex prefix.
    Tx(String),
    /// Integer block height (optional `#` or `block:` prefix, optional commas).
    BlockHeight(u64),
    /// Explicit `block:<64-hex>` block hash.
    BlockHash(String),
    /// Bare 64-hex string without prefix (could be either a txid or a block hash).
    AmbiguousHex64(String),
}

/// Initial search parameter parsed from the page URL (`?tx=...` or `?block=...`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitialSearchParam {
    Tx(String),
    Block(String),
}

/// Active historical-window view metadata mirrored from the JS canvas engine.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HistoryModeInfo {
    pub target_height: u64,
    #[serde(default)]
    pub target_hash: String,
    pub from_height: u64,
    pub to_height: u64,
}

#[derive(Deserialize)]
struct BlockWindowPayloadMeta {
    target_height: u64,
    #[serde(default)]
    target_hash: Option<String>,
    #[serde(default)]
    from_height: Option<u64>,
    #[serde(default)]
    to_height: Option<u64>,
    #[serde(default)]
    blocks: Vec<BlockWindowRowHeight>,
}

#[derive(Deserialize)]
struct BlockWindowRowHeight {
    height: u64,
}

/// Extract `HistoryModeInfo` from either a JS `__hbHistoryModeChange` payload
/// or a `/api/stats/heartbeat/block/{query}` HTTP response body.
pub fn parse_history_mode_info(json: &str) -> Option<HistoryModeInfo> {
    let meta: BlockWindowPayloadMeta = serde_json::from_str(json).ok()?;
    let from_height = meta
        .from_height
        .or_else(|| meta.blocks.first().map(|b| b.height))
        .unwrap_or(meta.target_height);
    let to_height = meta
        .to_height
        .or_else(|| meta.blocks.last().map(|b| b.height))
        .unwrap_or(meta.target_height);
    Some(HistoryModeInfo {
        target_height: meta.target_height,
        target_hash: meta.target_hash.unwrap_or_default(),
        from_height,
        to_height,
    })
}

/// Pending block metadata for the top-of-canvas cue banner while block details are in flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingBlockBanner {
    pub hash: String,
    pub observed_at: u64,
    /// Client-side Unix timestamp (seconds) used to compute the 2s display grace
    /// period without being skewed by client/server clock drift.
    pub received_at_client: u64,
}

/// Deserialization shape for `/api/stats/tx/{txid}` JSON responses.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct RawTxDetailResponse {
    #[serde(default)]
    pub inputs: Option<u64>,
    #[serde(default)]
    pub outputs: Option<u64>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub vsize: Option<u64>,
    #[serde(default)]
    pub weight: Option<u64>,
    #[serde(default)]
    pub fee: Option<u64>,
    #[serde(default)]
    pub total_output: Option<u64>,
    #[serde(default)]
    pub confirmed: Option<bool>,
    #[serde(default)]
    pub confirmations: Option<u64>,
    #[serde(default)]
    pub block_hash: Option<String>,
    #[serde(default)]
    pub block_height: Option<u64>,
    #[serde(default)]
    pub input_items: Vec<TxIoEntry>,
    #[serde(default)]
    pub output_items: Vec<TxIoEntry>,
    #[serde(default)]
    pub features: Vec<String>,
}

/// Rolling 60-second mempool arrival rate tracker (`tx/s`), fed by live
/// `MembershipAdd` deltas after snapshot install.
///
/// Waits 3 seconds after reset before reporting a rate so page load never
/// flashes a misleading `0.0 tx/s`, then divides by the elapsed observation
/// window clamped to `[3s, 60s]` so early readings converge smoothly.
#[derive(Clone, Debug, Default)]
pub struct ArrivalRateTracker {
    samples_ms: std::collections::VecDeque<f64>,
    started_at_ms: Option<f64>,
}

impl ArrivalRateTracker {
    const WINDOW_MS: f64 = 60_000.0;
    const MIN_WARMUP_MS: f64 = 3_000.0;

    pub fn reset(&mut self, now_ms: f64) {
        self.samples_ms.clear();
        self.started_at_ms = Some(now_ms);
    }

    pub fn record(&mut self, now_ms: f64) {
        if self.started_at_ms.is_none() {
            self.started_at_ms = Some(now_ms);
        }
        self.samples_ms.push_back(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: f64) {
        let cutoff = now_ms - Self::WINDOW_MS;
        while let Some(&front) = self.samples_ms.front() {
            if front < cutoff {
                self.samples_ms.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn rate_tx_per_sec(&mut self, now_ms: f64) -> Option<f64> {
        let started = self.started_at_ms?;
        self.prune(now_ms);
        let elapsed_ms = (now_ms - started).max(0.0);
        if elapsed_ms < Self::MIN_WARMUP_MS {
            return None;
        }
        let window_secs = (elapsed_ms.min(Self::WINDOW_MS)) / 1000.0;
        Some(self.samples_ms.len() as f64 / window_secs)
    }
}

// ---------------------------------------------------------------------------
// Pure helpers (SSR + hydrate)
// ---------------------------------------------------------------------------

/// Fixed fee-rate band label and hex colour matching the canvas brick palette.
pub fn fee_band_for_rate(
    fee_rate: Option<f64>,
) -> (&'static str, &'static str) {
    match fee_rate {
        None => ("Fee pending", "#64748b"),
        Some(r) if !r.is_finite() => ("Fee pending", "#64748b"),
        Some(r) if r < 1.0 => ("< 1 sat/vB", "#2563eb"),
        Some(r) if r < 2.0 => ("1-2 sat/vB", "#38bdf8"),
        Some(r) if r < 5.0 => ("2-5 sat/vB", "#00e676"),
        Some(r) if r < 15.0 => ("5-15 sat/vB", "#f7931a"),
        Some(r) if r < 50.0 => ("15-50 sat/vB", "#ff5722"),
        Some(_) => ("50+ sat/vB", "#f44336"),
    }
}

/// Format an optional sat/vB fee rate with `"0.0 sat/vB"` for verified zero,
/// 2 decimals for `0 < r < 1`, and 1 decimal at or above 1.
pub fn format_fee_rate(fee_rate: Option<f64>) -> String {
    match fee_rate {
        None => "Pending".to_string(),
        Some(r) if !r.is_finite() => "Pending".to_string(),
        Some(0.0) => "0.0 sat/vB".to_string(),
        Some(r) if r > 0.0 && r < 1.0 => format!("{r:.2} sat/vB"),
        Some(r) => format!("{r:.1} sat/vB"),
    }
}

/// Highest-priority notable classification label and hex colour for a `TxFlags` bitmask.
///
/// Priority order: Whale, Round #, Large Inscription, Consolidation, Fan-out, Fee Outlier,
/// OP_RETURN Msg.
pub fn primary_notable_label(
    flags: u8,
) -> Option<(&'static str, &'static str)> {
    if TxFlags::contains(flags, TxFlags::WHALE) {
        Some(("Whale", "#ffd700"))
    } else if TxFlags::contains(flags, TxFlags::ROUND_NUMBER) {
        Some(("Round #", "#90ee90"))
    } else if TxFlags::contains(flags, TxFlags::LARGE_INSCRIPTION) {
        Some(("Large Inscription", "#ff00c8"))
    } else if TxFlags::contains(flags, TxFlags::CONSOLIDATION) {
        Some(("Consolidation", "#a855f7"))
    } else if TxFlags::contains(flags, TxFlags::FAN_OUT) {
        Some(("Fan-out", "#00d2ff"))
    } else if TxFlags::contains(flags, TxFlags::FEE_OUTLIER) {
        Some(("Fee Outlier", "#ff4444"))
    } else if TxFlags::contains(flags, TxFlags::OP_RETURN_MSG) {
        Some(("OP_RETURN Msg", "#ffa500"))
    } else {
        None
    }
}

/// Construct a pinned transaction view from an unconfirmed `CompactTx`.
pub fn pinned_from_compact(tx: &CompactTx, on_canvas: bool) -> PinnedTxView {
    PinnedTxView {
        txid: tx.txid.clone(),
        status: PinnedTxStatus::Unconfirmed,
        fee: tx.fee,
        vsize: tx.vsize,
        value: tx.value,
        first_seen: tx.first_seen,
        flags: tx.flags,
        op_return_text: tx.op_return_text.clone(),
        on_canvas,
        deep: None,
        deep_loading: false,
        queue_position: None,
    }
}

/// Construct a pinned transaction view from a confirmed lookup match.
pub fn pinned_from_confirmed(
    txid: String,
    block_hash: String,
    block_height: u64,
) -> PinnedTxView {
    PinnedTxView {
        txid,
        status: PinnedTxStatus::Confirmed {
            block_hash,
            block_height,
        },
        fee: None,
        vsize: None,
        value: None,
        first_seen: 0,
        flags: TxFlags::NONE,
        op_return_text: None,
        on_canvas: false,
        deep: None,
        deep_loading: false,
        queue_position: None,
    }
}

/// Apply a `BlockDisconnected` event directly to a pinned view before mirror sync.
///
/// If the pinned transaction was confirmed in the disconnected block hash, retract
/// its confirmation cleanly. Subsequent `update_pinned_from_mirror` (or a follow-up
/// `MembershipAdd`) will promote it back to `Unconfirmed` if the node re-admits it.
pub fn apply_block_disconnected_to_pinned(
    pinned: &mut PinnedTxView,
    disconnected_hash: &str,
) {
    if matches!(
        &pinned.status,
        PinnedTxStatus::Confirmed { block_hash, .. } if block_hash == disconnected_hash
    ) {
        pinned.status = PinnedTxStatus::Removed;
        pinned.on_canvas = false;
    }
}

/// Reconcile an open pinned transaction view in place against the latest mirror state.
///
/// Preserves asynchronously fetched `deep` details and previously known fee/vsize/value
/// when transitioning across states (for example Unconfirmed -> Confirmed or reorg re-add).
pub fn update_pinned_from_mirror(
    pinned: &mut PinnedTxView,
    mirror: &HeartbeatMirror,
) {
    update_pinned_from_mirror_ex(pinned, mirror, true);
}

/// Variant of [`update_pinned_from_mirror`] that lets hot delta paths throttle
/// the O(N) mempool queue scan while still immediately updating metadata,
/// status transitions, and clearing `queue_position` on confirmation/removal.
pub fn update_pinned_from_mirror_ex(
    pinned: &mut PinnedTxView,
    mirror: &HeartbeatMirror,
    refresh_queue: bool,
) {
    if let Some(tx) = mirror.get_tx(&pinned.txid) {
        let rate_changed = pinned.fee != tx.fee || pinned.vsize != tx.vsize;
        pinned.status = PinnedTxStatus::Unconfirmed;
        if tx.fee.is_some() {
            pinned.fee = tx.fee;
        }
        if tx.vsize.is_some() {
            pinned.vsize = tx.vsize;
        }
        if tx.value.is_some() {
            pinned.value = tx.value;
        }
        if tx.first_seen > 0 {
            pinned.first_seen = tx.first_seen;
        }
        if tx.has_details() {
            pinned.flags = tx.flags;
            pinned.op_return_text = tx.op_return_text.clone();
        } else {
            pinned.flags |= tx.flags;
            if tx.op_return_text.is_some() {
                pinned.op_return_text = tx.op_return_text.clone();
            }
        }
        if pinned.fee.is_none() || pinned.vsize.is_none_or(|v| v == 0) {
            pinned.queue_position = None;
        } else if refresh_queue
            || rate_changed
            || pinned.queue_position.is_none()
        {
            pinned.queue_position = mirror.queue_position(&pinned.txid);
        }
        return;
    }

    if let Some((block_hash, block_height)) = mirror.confirmed_in(&pinned.txid)
    {
        pinned.status = PinnedTxStatus::Confirmed {
            block_hash: block_hash.to_string(),
            block_height,
        };
        pinned.on_canvas = false;
        pinned.queue_position = None;
        return;
    }

    if matches!(pinned.status, PinnedTxStatus::Unconfirmed) {
        pinned.status = PinnedTxStatus::Removed;
        pinned.on_canvas = false;
        pinned.queue_position = None;
    }
}

/// Merge deep node details from `/api/stats/tx/{txid}` into an existing pinned view.
pub fn apply_deep_details(
    pinned: &mut PinnedTxView,
    raw: &RawTxDetailResponse,
) {
    pinned.deep_loading = false;
    pinned.deep = Some(TxDeepDetails {
        inputs: raw.inputs,
        outputs: raw.outputs,
        size: raw.size,
        weight: raw.weight,
        confirmations: raw.confirmations,
        input_items: raw.input_items.clone(),
        output_items: raw.output_items.clone(),
        features: raw.features.clone(),
    });
    if pinned.fee.is_none() {
        pinned.fee = raw.fee;
    }
    if pinned.vsize.is_none() {
        if let Some(vsize) = raw
            .vsize
            .and_then(|v| u32::try_from(v).ok())
            .filter(|&v| v > 0)
        {
            pinned.vsize = Some(vsize);
        }
    }
    if pinned.value.is_none() {
        if let Some(val) = raw.total_output {
            pinned.value = Some(val);
        }
    }
    if let PinnedTxStatus::Confirmed {
        block_hash,
        block_height,
    } = &mut pinned.status
    {
        if block_hash.is_empty() {
            if let Some(h) = raw.block_hash.as_deref().filter(|s| !s.is_empty())
            {
                *block_hash = h.to_string();
            }
        }
        if *block_height == 0 {
            if let Some(h) = raw.block_height.filter(|&h| h > 0) {
                *block_height = h;
            }
        }
    }
}

/// Validate a decimal block height string (either plain ASCII digits or standard
/// thousands-grouped digits like `840,000`) and return its 1..=10 raw digits.
/// Rejects malformed comma placement such as `8,4,0000`, `,840`, or `0,000`.
fn parse_decimal_height_digits(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    let digits = if raw.contains(',') {
        let mut parts = raw.split(',');
        let first = parts.next()?;
        if !(1..=3).contains(&first.len())
            || !first.bytes().all(|b| b.is_ascii_digit())
            || first.starts_with('0')
        {
            return None;
        }
        let mut out = String::with_capacity(raw.len());
        out.push_str(first);
        for group in parts {
            if group.len() != 3 || !group.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            out.push_str(group);
        }
        out
    } else {
        if !raw.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        raw.to_string()
    };
    if !(1..=10).contains(&digits.len()) {
        return None;
    }
    Some(digits)
}

/// Build the user-facing not-found feedback for an `AmbiguousHex64` query whose
/// TX lookup missed and whose block-hash fallback returned 404, preserving any
/// partial-coverage or node-unavailable context.
#[cfg_attr(not(any(feature = "hydrate", test)), allow(dead_code))]
fn ambiguous_hex_not_found_message(
    coverage: Option<&Coverage>,
    tx_lookup_unavailable: bool,
) -> &'static str {
    if coverage.is_some_and(|c| !c.is_complete()) {
        "Not found (mempool coverage is currently partial)"
    } else if tx_lookup_unavailable || coverage.is_none() {
        "Not found (node currently unavailable)"
    } else {
        "Not found"
    }
}

/// Classify a unified Heartbeat search query into a transaction, block height,
/// block hash, or ambiguous 64-character hex string.
///
/// UTF-8 safe: never slices raw byte offsets without `.get(..)` or `strip_prefix`.
pub fn parse_search_query(raw: &str) -> Option<SearchQuery> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Explicit `tx:` prefix (case-insensitive) -> validate 1..=64 ASCII hex.
    if trimmed
        .get(..3)
        .is_some_and(|p| p.eq_ignore_ascii_case("tx:"))
    {
        let rest = trimmed.get(3..).unwrap_or("").trim();
        if (1..=64).contains(&rest.len())
            && rest.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Some(SearchQuery::Tx(rest.to_ascii_lowercase()));
        }
        return None;
    }

    // Explicit `block:` prefix (case-insensitive) or leading `#`.
    let explicit_block = if trimmed
        .get(..6)
        .is_some_and(|p| p.eq_ignore_ascii_case("block:"))
    {
        Some(trimmed.get(6..).unwrap_or("").trim())
    } else {
        trimmed.strip_prefix('#').map(str::trim)
    };

    if let Some(rest) = explicit_block {
        if rest.is_empty() {
            return None;
        }
        if let Some(digits) = parse_decimal_height_digits(rest) {
            if let Ok(h) = digits.parse::<u64>() {
                return Some(SearchQuery::BlockHeight(h));
            }
            return None;
        }
        if rest.len() == 64 && rest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Some(SearchQuery::BlockHash(rest.to_ascii_lowercase()));
        }
        return None;
    }

    // Bare query (no prefix):
    // 1. Block height: valid 1..=10 digit decimal (with optional standard commas)
    //    that has 1..=7 digits, or parses as u64 <= 10_000_000 without a leading
    //    zero (so 8-char hex txid prefixes like "00001234" stay Tx).
    if let Some(digits) = parse_decimal_height_digits(trimmed) {
        if digits.len() <= 7
            || (!digits.starts_with('0')
                && digits.parse::<u64>().is_ok_and(|h| h <= 10_000_000))
        {
            if let Ok(h) = digits.parse::<u64>() {
                return Some(SearchQuery::BlockHeight(h));
            }
        }
    }

    // 2. 64-char hex without prefix -> ambiguous (could be txid or block hash).
    if trimmed.len() == 64 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Some(SearchQuery::AmbiguousHex64(trimmed.to_ascii_lowercase()));
    }

    // 3. 8..=63-char hex without prefix -> txid prefix.
    if (8..=63).contains(&trimmed.len())
        && trimmed.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Some(SearchQuery::Tx(trimmed.to_ascii_lowercase()));
    }

    None
}

/// UTF-8 safe percent-decoder for URL query parameter values.
fn percent_decode_param(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_nibble(bytes[i + 1]);
                let lo = hex_nibble(bytes[i + 2]);
                if let (Some(h), Some(l)) = (hi, lo) {
                    out.push((h << 4) | l);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Parse `?tx=...` or `?block=...` from a URL query string.
///
/// Decodes percent-encoded characters such as `%23` (`#`) and `%3A` (`:`) cleanly
/// and validates the result without raw byte-index slicing.
pub fn parse_initial_search_param(search: &str) -> Option<InitialSearchParam> {
    let query = search.strip_prefix('?').unwrap_or(search);
    for pair in query.split('&') {
        let Some((key, raw_val)) = pair.split_once('=') else {
            continue;
        };
        if key == "tx" {
            let Some(decoded) = percent_decode_param(raw_val) else {
                continue;
            };
            if let Some(norm) = normalize_tx_query(&decoded) {
                if norm.len() >= 8 {
                    return Some(InitialSearchParam::Tx(norm));
                }
            }
        } else if key == "block" {
            let Some(decoded) = percent_decode_param(raw_val) else {
                continue;
            };
            let trimmed = decoded.trim();
            let candidate = if trimmed
                .get(..6)
                .is_some_and(|p| p.eq_ignore_ascii_case("block:"))
                || trimmed.starts_with('#')
            {
                trimmed.to_string()
            } else {
                format!("block:{trimmed}")
            };
            match parse_search_query(&candidate) {
                Some(SearchQuery::BlockHeight(h)) => {
                    return Some(InitialSearchParam::Block(h.to_string()));
                }
                Some(SearchQuery::BlockHash(hash)) => {
                    return Some(InitialSearchParam::Block(hash));
                }
                _ => {}
            }
        }
    }
    None
}

/// Normalize a user-supplied txid query (trim, strip optional `tx:` prefix, lowercase,
/// validate 1..=64 ASCII hex characters).
pub fn normalize_tx_query(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let without_prefix = if trimmed
        .get(..3)
        .is_some_and(|p| p.eq_ignore_ascii_case("tx:"))
    {
        trimmed.get(3..).unwrap_or("").trim()
    } else {
        trimmed
    };
    if without_prefix.is_empty() || without_prefix.len() > 64 {
        return None;
    }
    if !without_prefix.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(without_prefix.to_ascii_lowercase())
}

/// Parse `?tx=<hex>` (or `&tx=<hex>`) from a URL query string and validate >= 8 hex chars.
pub fn parse_tx_param_from_search(search: &str) -> Option<String> {
    let query = search.strip_prefix('?').unwrap_or(search);
    for pair in query.split('&') {
        let Some((key, raw_val)) = pair.split_once('=') else {
            continue;
        };
        if key != "tx" {
            continue;
        }
        // Handle percent-encoded `tx:` prefix (`tx%3A` / `tx%3a`) if shared via URL.
        let unescaped = if raw_val
            .get(..5)
            .is_some_and(|p| p.eq_ignore_ascii_case("tx%3a"))
        {
            raw_val.get(5..).unwrap_or("")
        } else {
            raw_val
        };
        if let Some(norm) = normalize_tx_query(unescaped) {
            if norm.len() >= 8 {
                return Some(norm);
            }
        }
    }
    None
}

#[derive(Serialize)]
struct CanvasSnapshotPayload<'a> {
    transactions: Vec<&'a CompactTx>,
    recent_blocks: &'a [CanonicalBlock],
    pending_blocks: &'a [PendingBlock],
    recent_confirmations: Vec<ConfirmedBlock>,
    coverage: &'a Coverage,
}

/// Serialize the mirror's current state into the JSON payload expected by
/// `window.hbV2InstallSnapshot(payloadJson, isResync)`.
pub fn build_canvas_snapshot_json(mirror: &HeartbeatMirror) -> String {
    let payload = CanvasSnapshotPayload {
        transactions: mirror.ordered_members(),
        recent_blocks: mirror.recent_blocks(),
        pending_blocks: mirror.pending_blocks(),
        recent_confirmations: mirror.recent_confirmations(),
        coverage: mirror.coverage(),
    };
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

/// Serialize an `ApplyEffect` into the JSON payload expected by `window.hbV2ApplyEffect`.
pub fn build_canvas_effect_json(effect: &ApplyEffect) -> String {
    serde_json::to_string(effect).unwrap_or_else(|_| "{}".to_string())
}

/// Map transport and coverage state into the connection badge state string.
pub fn connection_state_for_mirror(
    connected: bool,
    reconnecting: bool,
    syncing: bool,
    coverage: &Coverage,
) -> &'static str {
    if reconnecting {
        "disconnected"
    } else if syncing {
        "syncing"
    } else if !connected {
        "disconnected"
    } else if !coverage.is_complete() {
        "partial"
    } else {
        "live"
    }
}

/// Exponential backoff delay in milliseconds: `2000 * 1.5^min(retries, 8)` capped at 15,000 ms.
pub fn retry_delay_ms(retries: u32) -> i32 {
    let exp = retries.min(8) as i32;
    let raw = 2000.0 * 1.5_f64.powi(exp);
    (raw.round() as i32).min(15_000)
}

/// Format an integer with comma thousands separators.
pub fn format_u64_commas(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    bytes
        .rchunks(3)
        .rev()
        .map(|c| std::str::from_utf8(c).expect("ASCII digits are valid UTF-8"))
        .collect::<Vec<_>>()
        .join(",")
}

// ---------------------------------------------------------------------------
// JS interop bridge (hydrate externs + non-hydrate SSR stubs)
// ---------------------------------------------------------------------------

#[cfg(feature = "hydrate")]
mod js_bridge {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = hbV2InstallSnapshot, catch)]
        fn js_hb_v2_install_snapshot(
            json: &str,
            is_resync: bool,
        ) -> Result<(), JsValue>;

        #[wasm_bindgen(js_name = hbV2ApplyEffect, catch)]
        fn js_hb_v2_apply_effect(json: &str) -> Result<(), JsValue>;

        #[wasm_bindgen(js_name = hbV2FocusTx, catch)]
        fn js_hb_v2_focus_tx(txid: &str) -> Result<bool, JsValue>;

        #[wasm_bindgen(js_name = hbV2SetPinnedTx, catch)]
        fn js_hb_v2_set_pinned_tx(txid: Option<&str>) -> Result<(), JsValue>;

        #[wasm_bindgen(js_name = hbV2SetSpotlightBand, catch)]
        fn js_hb_v2_set_spotlight_band(band: i32) -> Result<(), JsValue>;

        // Pass JS Number (`f64`), not `u64` (BigInt), because JS compares
        // `seg.height === height` with strict equality against Number heights.
        #[wasm_bindgen(js_name = hbV2FocusBlockHeight, catch)]
        fn js_hb_v2_focus_block_height(height: f64) -> Result<bool, JsValue>;

        #[wasm_bindgen(js_name = hbV2SetConnectionState, catch)]
        fn js_hb_v2_set_connection_state(state: &str) -> Result<(), JsValue>;

        #[wasm_bindgen(js_name = hbV2EnterHistoryMode, catch)]
        fn js_hb_v2_enter_history_mode(
            payload_json: &str,
        ) -> Result<bool, JsValue>;

        #[wasm_bindgen(js_name = hbV2ExitHistoryMode, catch)]
        fn js_hb_v2_exit_history_mode() -> Result<(), JsValue>;

        #[wasm_bindgen(js_name = hbV2FocusPendingSlot, catch)]
        fn js_hb_v2_focus_pending_slot(hash: &str) -> Result<bool, JsValue>;

        #[wasm_bindgen(js_name = hbV2IsPendingSlotOffscreen, catch)]
        fn js_hb_v2_is_pending_slot_offscreen(
            hash: &str,
        ) -> Result<bool, JsValue>;

        #[wasm_bindgen(js_name = getHeartbeatRecentBlocks, catch)]
        fn js_get_heartbeat_recent_blocks() -> Result<String, JsValue>;

        #[wasm_bindgen(js_name = renderRhythmStrip, catch)]
        fn js_render_rhythm_strip(
            canvas_id: &str,
            blocks_json: &str,
        ) -> Result<(), JsValue>;
    }

    pub fn hb_v2_install_snapshot(json: &str, is_resync: bool) {
        let _ = js_hb_v2_install_snapshot(json, is_resync);
    }

    pub fn hb_v2_apply_effect(json: &str) {
        let _ = js_hb_v2_apply_effect(json);
    }

    pub fn hb_v2_focus_tx(txid: &str) -> bool {
        js_hb_v2_focus_tx(txid).unwrap_or(false)
    }

    pub fn hb_v2_set_pinned_tx(txid: Option<&str>) {
        let _ = js_hb_v2_set_pinned_tx(txid);
    }

    pub fn hb_v2_set_spotlight_band(band: i32) {
        let _ = js_hb_v2_set_spotlight_band(band);
    }

    pub fn hb_v2_focus_block_height(height: u64) -> bool {
        js_hb_v2_focus_block_height(height as f64).unwrap_or(false)
    }

    pub fn hb_v2_set_connection_state(state: &str) {
        let _ = js_hb_v2_set_connection_state(state);
    }

    pub fn hb_v2_enter_history_mode(payload_json: &str) -> bool {
        js_hb_v2_enter_history_mode(payload_json).unwrap_or(false)
    }

    pub fn hb_v2_exit_history_mode() {
        let _ = js_hb_v2_exit_history_mode();
    }

    pub fn hb_v2_focus_pending_slot(hash: &str) -> bool {
        js_hb_v2_focus_pending_slot(hash).unwrap_or(false)
    }

    pub fn hb_v2_is_pending_slot_offscreen(hash: &str) -> bool {
        js_hb_v2_is_pending_slot_offscreen(hash).unwrap_or(false)
    }

    pub fn refresh_rhythm_strip_from_canvas() {
        if let Ok(recent) = js_get_heartbeat_recent_blocks() {
            if recent.len() > 2 {
                let _ = js_render_rhythm_strip("rhythm-strip-canvas", &recent);
            }
        }
    }
}

#[cfg(not(feature = "hydrate"))]
mod js_bridge {
    pub fn hb_v2_install_snapshot(_json: &str, _is_resync: bool) {}
    pub fn hb_v2_apply_effect(_json: &str) {}
    pub fn hb_v2_focus_tx(_txid: &str) -> bool {
        false
    }
    pub fn hb_v2_set_pinned_tx(_txid: Option<&str>) {}
    pub fn hb_v2_set_spotlight_band(_band: i32) {}
    pub fn hb_v2_focus_block_height(_height: u64) -> bool {
        false
    }
    pub fn hb_v2_set_connection_state(_state: &str) {}
    pub fn hb_v2_enter_history_mode(_payload_json: &str) -> bool {
        false
    }
    pub fn hb_v2_exit_history_mode() {}
    pub fn hb_v2_focus_pending_slot(_hash: &str) -> bool {
        false
    }
    pub fn hb_v2_is_pending_slot_offscreen(_hash: &str) -> bool {
        false
    }
}

pub use js_bridge::{
    hb_v2_apply_effect, hb_v2_enter_history_mode, hb_v2_exit_history_mode,
    hb_v2_focus_block_height, hb_v2_focus_pending_slot, hb_v2_focus_tx,
    hb_v2_install_snapshot, hb_v2_is_pending_slot_offscreen,
    hb_v2_set_connection_state, hb_v2_set_pinned_tx, hb_v2_set_spotlight_band,
};

// ---------------------------------------------------------------------------
// Hydrate-only v2 mirror client
// ---------------------------------------------------------------------------

#[cfg(feature = "hydrate")]
mod client {
    use std::cell::{Cell, RefCell};
    use std::rc::{Rc, Weak};

    use leptos::prelude::*;
    use leptos::web_sys::{
        self, js_sys, AbortController, Event, EventSource, MessageEvent,
        RequestInit, Response,
    };
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    use super::{
        ambiguous_hex_not_found_message, apply_block_disconnected_to_pinned,
        apply_deep_details, build_canvas_effect_json,
        build_canvas_snapshot_json, connection_state_for_mirror,
        format_u64_commas, hb_v2_apply_effect, hb_v2_enter_history_mode,
        hb_v2_exit_history_mode, hb_v2_focus_block_height,
        hb_v2_focus_pending_slot, hb_v2_focus_tx, hb_v2_install_snapshot,
        hb_v2_set_connection_state, hb_v2_set_pinned_tx, js_bridge,
        normalize_tx_query, parse_history_mode_info, parse_search_query,
        pinned_from_compact, pinned_from_confirmed, retry_delay_ms,
        update_pinned_from_mirror, update_pinned_from_mirror_ex,
        ArrivalRateTracker, HistoryModeInfo, InitialSearchParam,
        PendingBlockBanner, PinnedTxStatus, PinnedTxView, RawTxDetailResponse,
        SearchQuery,
    };
    use crate::stats::heartbeat_sync::{
        ApplyEffect, ApplyOutcome, Coverage, FeeBandSummary, HeartbeatDelta,
        HeartbeatMirror, HeartbeatSnapshot, MirrorTxMatch, TxFlags,
        TxLookupStatus,
    };

    /// Reactive signals owned by `HeartbeatPage` and updated by `HeartbeatClient`.
    #[derive(Clone, Copy)]
    pub struct HeartbeatClientSignals {
        pub set_loading: WriteSignal<bool>,
        pub set_conn_state: WriteSignal<String>,
        pub set_coverage_note: WriteSignal<Option<String>>,
        pub set_pinned_tx: WriteSignal<Option<PinnedTxView>>,
        pub pinned_tx: ReadSignal<Option<PinnedTxView>>,
        pub set_search_feedback: WriteSignal<Option<(bool, String)>>,
        pub set_sse_height: WriteSignal<u64>,
        pub sse_height: ReadSignal<u64>,
        pub set_sse_ts: WriteSignal<u64>,
        pub set_pending_block: WriteSignal<Option<PendingBlockBanner>>,
        pub set_history_mode: WriteSignal<Option<HistoryModeInfo>>,
        pub set_fee_bands: WriteSignal<FeeBandSummary>,
        pub set_arrival_rate: WriteSignal<Option<f64>>,
    }

    type EventListenerEntry = (&'static str, Closure<dyn FnMut(Event)>);

    fn client_now_unix_secs() -> u64 {
        (js_sys::Date::now() / 1000.0).max(0.0) as u64
    }

    #[derive(Default)]
    struct ClientState {
        mirror: HeartbeatMirror,
        event_source: Option<EventSource>,
        es_listeners: Vec<EventListenerEntry>,
        snapshot_abort: Option<AbortController>,
        lookup_abort: Option<AbortController>,
        block_lookup_abort: Option<AbortController>,
        retry_timer_id: Option<i32>,
        retry_closure: Option<Closure<dyn FnMut()>>,
        retry_count: u32,
        connected: bool,
        resume_attempted: bool,
        initial_search_param: Option<InitialSearchParam>,
        history_mode: Option<HistoryModeInfo>,
        pending_block: Option<PendingBlockBanner>,
        visibility_closure: Option<Closure<dyn FnMut(Event)>>,
        select_tx_closure: Option<Closure<dyn FnMut(String)>>,
        clear_pinned_closure: Option<Closure<dyn FnMut()>>,
        history_mode_change_closure: Option<Closure<dyn FnMut(JsValue)>>,
        resync_canvas_closure: Option<Closure<dyn FnMut()>>,
        /// Timestamp (ms) of the last O(N) pinned-tx queue scan so high-rate
        /// mempool bursts throttle queue updates to <= once every 2.5s.
        last_queue_refresh_ms: f64,
        /// Timestamp (ms) + last emitted value of the O(N) fee-band summary scan
        /// (throttled to <= once every 2.0s during steady-state deltas).
        last_bands_refresh_ms: f64,
        last_fee_bands: FeeBandSummary,
        /// Rolling 60s arrival rate tracker fed by live `MembershipAdd` deltas.
        arrival_tracker: ArrivalRateTracker,
        destroyed: bool,
    }

    struct ClientInner {
        /// Incremented on `destroy()` to invalidate every callback from this mount.
        generation: Cell<u64>,
        /// Incremented on every snapshot/SSE sync cycle and on `destroy()`.
        cycle_gen: Cell<u64>,
        /// Incremented on every tx search/select/clear and on `destroy()`.
        lookup_gen: Cell<u64>,
        /// Incremented on every block-window lookup, exit-to-LIVE, and `destroy()`.
        block_lookup_gen: Cell<u64>,
        signals: HeartbeatClientSignals,
        state: RefCell<ClientState>,
    }

    /// Browser-side Heartbeat v2 sync client with deterministic generation-guarded cleanup.
    #[derive(Clone)]
    pub struct HeartbeatClient {
        inner: Rc<ClientInner>,
    }

    impl HeartbeatClient {
        pub fn new(signals: HeartbeatClientSignals) -> Self {
            Self {
                inner: Rc::new(ClientInner {
                    generation: Cell::new(1),
                    cycle_gen: Cell::new(1),
                    lookup_gen: Cell::new(1),
                    block_lookup_gen: Cell::new(1),
                    signals,
                    state: RefCell::new(ClientState::default()),
                }),
            }
        }

        fn downgrade(&self) -> Weak<ClientInner> {
            Rc::downgrade(&self.inner)
        }

        fn is_mount_live(&self, mount_gen: u64) -> bool {
            self.inner.generation.get() == mount_gen
        }

        fn is_cycle_live(&self, mount_gen: u64, cycle_gen: u64) -> bool {
            self.inner.generation.get() == mount_gen
                && self.inner.cycle_gen.get() == cycle_gen
        }

        fn is_lookup_live(&self, mount_gen: u64, lookup_gen: u64) -> bool {
            self.inner.generation.get() == mount_gen
                && self.inner.lookup_gen.get() == lookup_gen
        }

        fn is_block_lookup_live(
            &self,
            mount_gen: u64,
            block_lookup_gen: u64,
        ) -> bool {
            self.inner.generation.get() == mount_gen
                && self.inner.block_lookup_gen.get() == block_lookup_gen
        }

        /// Start the v2 snapshot + SSE sync loop and attach window/document listeners.
        pub fn start(&self, initial_search_param: Option<InitialSearchParam>) {
            let mount_gen = self.inner.generation.get();

            // Register visibilitychange listener (reconnect only if stream is closed).
            let vis_client = self.downgrade();
            let vis_cb = Closure::<dyn FnMut(Event)>::new(move |_ev: Event| {
                let Some(vis_client) = upgrade_client(&vis_client) else {
                    return;
                };
                if !vis_client.is_mount_live(mount_gen) {
                    return;
                }
                let Some(doc) = web_sys::window().and_then(|w| w.document())
                else {
                    return;
                };
                if doc.hidden() {
                    return;
                }
                let needs_resync = {
                    let st = vis_client.inner.state.borrow();
                    if st.destroyed || st.snapshot_abort.is_some() {
                        false
                    } else {
                        match &st.event_source {
                            None => true,
                            Some(es) => {
                                es.ready_state() == EventSource::CLOSED
                                    || !st.connected
                                        && es.ready_state()
                                            != EventSource::CONNECTING
                            }
                        }
                    }
                };
                if needs_resync {
                    vis_client.fetch_and_connect(true);
                }
            });

            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                let _ = doc.add_event_listener_with_callback(
                    "visibilitychange",
                    vis_cb.as_ref().unchecked_ref(),
                );
            }

            // `?block=` queries hit the DB-backed `/api/stats/heartbeat/block/{query}`
            // endpoint directly and do not depend on the live mempool snapshot.
            let (immediate_block, deferred_param) = match initial_search_param {
                Some(InitialSearchParam::Block(query)) => (Some(query), None),
                other => (None, other),
            };

            {
                let mut st = self.inner.state.borrow_mut();
                st.initial_search_param = deferred_param;
                st.visibility_closure = Some(vis_cb);
            }

            self.install_js_callbacks(mount_gen);
            self.fetch_and_connect(false);
            if let Some(query) = immediate_block {
                self.search(&format!("block:{query}"));
            }
        }

        fn install_js_callbacks(&self, mount_gen: u64) {
            // Register canvas/feed -> WASM selection and history-mode bridges on window.
            let sel_client = self.downgrade();
            let select_cb =
                Closure::<dyn FnMut(String)>::new(move |txid: String| {
                    let Some(sel_client) = upgrade_client(&sel_client) else {
                        return;
                    };
                    if !sel_client.is_mount_live(mount_gen) {
                        return;
                    }
                    sel_client.select_tx_from_canvas(&txid);
                });

            let clr_client = self.downgrade();
            let clear_cb = Closure::<dyn FnMut()>::new(move || {
                let Some(clr_client) = upgrade_client(&clr_client) else {
                    return;
                };
                if !clr_client.is_mount_live(mount_gen) {
                    return;
                }
                clr_client.clear_pinned_tx();
            });

            let hm_client = self.downgrade();
            let history_mode_cb =
                Closure::<dyn FnMut(JsValue)>::new(move |val: JsValue| {
                    let Some(hm_client) = upgrade_client(&hm_client) else {
                        return;
                    };
                    if !hm_client.is_mount_live(mount_gen) {
                        return;
                    }
                    let info_json = val.as_string().unwrap_or_default();
                    if info_json.trim().is_empty() {
                        let _ = hm_client.begin_block_lookup_generation();
                        hm_client.set_history_mode_if_changed(None);
                        return;
                    }
                    let next = parse_history_mode_info(&info_json);
                    hm_client.set_history_mode_if_changed(next);
                });

            let resync_client = self.downgrade();
            let resync_cb = Closure::<dyn FnMut()>::new(move || {
                let Some(resync_client) = upgrade_client(&resync_client) else {
                    return;
                };
                if !resync_client.is_mount_live(mount_gen) {
                    return;
                }
                let snapshot_json = {
                    let st = resync_client.inner.state.borrow();
                    if st.destroyed || st.mirror.revision().is_none() {
                        None
                    } else {
                        Some(build_canvas_snapshot_json(&st.mirror))
                    }
                };
                if let Some(json) = snapshot_json {
                    hb_v2_install_snapshot(&json, true);
                }
            });

            if let Some(win) = web_sys::window() {
                let _ = js_sys::Reflect::set(
                    &win,
                    &JsValue::from_str("__hbSelectTx"),
                    select_cb.as_ref().unchecked_ref(),
                );
                let _ = js_sys::Reflect::set(
                    &win,
                    &JsValue::from_str("__hbClearPinnedTx"),
                    clear_cb.as_ref().unchecked_ref(),
                );
                let _ = js_sys::Reflect::set(
                    &win,
                    &JsValue::from_str("__hbHistoryModeChange"),
                    history_mode_cb.as_ref().unchecked_ref(),
                );
                let _ = js_sys::Reflect::set(
                    &win,
                    &JsValue::from_str("__hbResyncCanvasFromMirror"),
                    resync_cb.as_ref().unchecked_ref(),
                );
            }

            {
                let mut st = self.inner.state.borrow_mut();
                st.select_tx_closure = Some(select_cb);
                st.clear_pinned_closure = Some(clear_cb);
                st.history_mode_change_closure = Some(history_mode_cb);
                st.resync_canvas_closure = Some(resync_cb);
            }
        }

        fn set_history_mode_if_changed(&self, next: Option<HistoryModeInfo>) {
            let changed = {
                let mut st = self.inner.state.borrow_mut();
                if st.history_mode != next {
                    st.history_mode = next.clone();
                    true
                } else {
                    false
                }
            };
            if changed {
                let _ = self.inner.signals.set_history_mode.try_set(next);
            }
        }

        fn set_pending_block_if_changed(
            &self,
            next: Option<PendingBlockBanner>,
        ) {
            let changed = {
                let mut st = self.inner.state.borrow_mut();
                if st.pending_block != next {
                    st.pending_block = next.clone();
                    true
                } else {
                    false
                }
            };
            if changed {
                let _ = self.inner.signals.set_pending_block.try_set(next);
            }
        }

        /// Close active EventSource, abort in-flight fetches, clear timers, and detach all listeners.
        pub fn destroy(&self) {
            self.inner
                .generation
                .set(self.inner.generation.get().wrapping_add(1));
            self.inner
                .cycle_gen
                .set(self.inner.cycle_gen.get().wrapping_add(1));
            self.inner
                .lookup_gen
                .set(self.inner.lookup_gen.get().wrapping_add(1));
            self.inner
                .block_lookup_gen
                .set(self.inner.block_lookup_gen.get().wrapping_add(1));

            let (
                es,
                es_listeners,
                snap_abort,
                lookup_abort,
                block_lookup_abort,
                timer_id,
                _retry_cb,
                vis_cb,
                _sel_cb,
                _clr_cb,
                _hm_cb,
                _resync_cb,
            ) = {
                let mut st = self.inner.state.borrow_mut();
                st.destroyed = true;
                st.connected = false;
                st.history_mode = None;
                st.pending_block = None;
                st.mirror = HeartbeatMirror::default();
                (
                    st.event_source.take(),
                    std::mem::take(&mut st.es_listeners),
                    st.snapshot_abort.take(),
                    st.lookup_abort.take(),
                    st.block_lookup_abort.take(),
                    st.retry_timer_id.take(),
                    st.retry_closure.take(),
                    st.visibility_closure.take(),
                    st.select_tx_closure.take(),
                    st.clear_pinned_closure.take(),
                    st.history_mode_change_closure.take(),
                    st.resync_canvas_closure.take(),
                )
            };

            teardown_event_source(es, es_listeners);
            if let Some(ac) = snap_abort {
                ac.abort();
            }
            if let Some(ac) = lookup_abort {
                ac.abort();
            }
            if let Some(ac) = block_lookup_abort {
                ac.abort();
            }
            if let Some(win) = web_sys::window() {
                if let Some(id) = timer_id {
                    win.clear_timeout_with_handle(id);
                }
                let _ = js_sys::Reflect::delete_property(
                    &win,
                    &JsValue::from_str("__hbSelectTx"),
                );
                let _ = js_sys::Reflect::delete_property(
                    &win,
                    &JsValue::from_str("__hbClearPinnedTx"),
                );
                let _ = js_sys::Reflect::delete_property(
                    &win,
                    &JsValue::from_str("__hbHistoryModeChange"),
                );
                let _ = js_sys::Reflect::delete_property(
                    &win,
                    &JsValue::from_str("__hbResyncCanvasFromMirror"),
                );
                if let Some(doc) = win.document() {
                    if let Some(cb) = vis_cb {
                        let _ = doc.remove_event_listener_with_callback(
                            "visibilitychange",
                            cb.as_ref().unchecked_ref(),
                        );
                    }
                }
            }
        }

        fn close_event_source(&self) {
            let (es, es_listeners) = {
                let mut st = self.inner.state.borrow_mut();
                st.connected = false;
                (st.event_source.take(), std::mem::take(&mut st.es_listeners))
            };
            teardown_event_source(es, es_listeners);
        }

        fn clear_retry_timer(&self) {
            let (timer_id, _cb) = {
                let mut st = self.inner.state.borrow_mut();
                (st.retry_timer_id.take(), st.retry_closure.take())
            };
            if let Some(id) = timer_id {
                if let Some(win) = web_sys::window() {
                    win.clear_timeout_with_handle(id);
                }
            }
        }

        fn set_conn_state(&self, state: &str) {
            let _ =
                self.inner.signals.set_conn_state.try_set(state.to_string());
            hb_v2_set_connection_state(state);
        }

        /// Fetch `/api/stats/heartbeat/snapshot`, install it into the mirror, and open `/api/stats/heartbeat/v2`.
        fn fetch_and_connect(&self, is_resync: bool) {
            let mount_gen = self.inner.generation.get();
            let cycle_gen = self.inner.cycle_gen.get().wrapping_add(1);
            self.inner.cycle_gen.set(cycle_gen);

            self.close_event_source();
            self.clear_retry_timer();

            let prev_abort = {
                let mut st = self.inner.state.borrow_mut();
                if st.destroyed {
                    return;
                }
                st.resume_attempted = true;
                st.snapshot_abort.take()
            };
            if let Some(ac) = prev_abort {
                ac.abort();
            }

            self.set_conn_state("syncing");

            let Ok(controller) = AbortController::new() else {
                self.schedule_retry(mount_gen, cycle_gen);
                return;
            };
            let fetch_controller = controller.clone();
            {
                let mut st = self.inner.state.borrow_mut();
                st.snapshot_abort = Some(controller);
            }

            let ok_client = self.downgrade();
            let err_client = self.downgrade();
            fetch_text_with_abort(
                "/api/stats/heartbeat/snapshot",
                &fetch_controller,
                move |status, body| {
                    let Some(ok_client) = upgrade_client(&ok_client) else {
                        return;
                    };
                    if !ok_client.is_cycle_live(mount_gen, cycle_gen) {
                        return;
                    }
                    {
                        ok_client.inner.state.borrow_mut().snapshot_abort =
                            None;
                    }
                    if status != 200 {
                        let _ =
                            ok_client.inner.signals.set_loading.try_set(false);
                        ok_client.set_conn_state("disconnected");
                        ok_client.schedule_retry(mount_gen, cycle_gen);
                        return;
                    }
                    let Ok(snapshot) =
                        serde_json::from_str::<HeartbeatSnapshot>(&body)
                    else {
                        let _ =
                            ok_client.inner.signals.set_loading.try_set(false);
                        ok_client.set_conn_state("disconnected");
                        ok_client.schedule_retry(mount_gen, cycle_gen);
                        return;
                    };
                    ok_client.install_snapshot_and_open_stream(
                        snapshot, is_resync, mount_gen, cycle_gen,
                    );
                },
                move |_err| {
                    let Some(err_client) = upgrade_client(&err_client) else {
                        return;
                    };
                    if !err_client.is_cycle_live(mount_gen, cycle_gen) {
                        return;
                    }
                    {
                        err_client.inner.state.borrow_mut().snapshot_abort =
                            None;
                    }
                    let _ = err_client.inner.signals.set_loading.try_set(false);
                    err_client.set_conn_state("disconnected");
                    err_client.schedule_retry(mount_gen, cycle_gen);
                },
            );
        }

        fn install_snapshot_and_open_stream(
            &self,
            snapshot: HeartbeatSnapshot,
            is_resync: bool,
            mount_gen: u64,
            cycle_gen: u64,
        ) {
            if !self.is_cycle_live(mount_gen, cycle_gen) {
                return;
            }

            let existing_pinned =
                self.inner.signals.pinned_tx.try_get_untracked().flatten();

            let extracted = {
                let mut st = self.inner.state.borrow_mut();
                if st.destroyed {
                    return;
                }
                if st.mirror.install_snapshot(snapshot).is_err() {
                    return;
                }
                st.resume_attempted = false;
                let Some((epoch, seq)) = st.mirror.revision() else {
                    return;
                };
                let epoch = epoch.to_string();
                let snapshot_json = build_canvas_snapshot_json(&st.mirror);
                let coverage_note =
                    st.mirror.coverage().reason().map(str::to_string);
                let newest_block = st
                    .mirror
                    .recent_blocks()
                    .last()
                    .map(|b| (b.height, b.timestamp));
                let client_now = client_now_unix_secs();
                let pending_banner =
                    st.mirror.pending_blocks().last().map(|pb| {
                        let received_at_client = st
                            .pending_block
                            .as_ref()
                            .filter(|existing| existing.hash == pb.hash)
                            .map(|existing| existing.received_at_client)
                            .unwrap_or_else(|| {
                                if pb.observed_at > 0 {
                                    pb.observed_at.min(client_now)
                                } else {
                                    client_now
                                }
                            });
                        PendingBlockBanner {
                            hash: pb.hash.clone(),
                            observed_at: pb.observed_at,
                            received_at_client,
                        }
                    });
                let updated_pinned =
                    existing_pinned.as_ref().and_then(|existing| {
                        let mut pv = existing.clone();
                        update_pinned_from_mirror(&mut pv, &st.mirror);
                        (&pv != existing).then_some(pv)
                    });
                let now_ms = js_sys::Date::now();
                let fee_bands = st.mirror.fee_band_summary();
                st.last_fee_bands = fee_bands;
                st.last_bands_refresh_ms = now_ms;
                st.arrival_tracker.reset(now_ms);
                // Consume on the first successful snapshot install regardless of
                // `is_resync` so a transient initial snapshot failure cannot drop
                // `?tx=` deep links.
                let initial_search = st.initial_search_param.take();
                (
                    epoch,
                    seq,
                    snapshot_json,
                    coverage_note,
                    newest_block,
                    pending_banner,
                    updated_pinned,
                    initial_search,
                    fee_bands,
                )
            };

            let (
                epoch,
                seq,
                snapshot_json,
                coverage_note,
                newest_block,
                pending_banner,
                updated_pinned,
                initial_search,
                fee_bands,
            ) = extracted;

            let _ = self.inner.signals.set_coverage_note.try_set(coverage_note);
            let _ = self.inner.signals.set_fee_bands.try_set(fee_bands);
            let _ = self.inner.signals.set_arrival_rate.try_set(None);
            self.set_pending_block_if_changed(pending_banner);

            if let Some(pv) = updated_pinned {
                let _ = self.inner.signals.set_pinned_tx.try_set(Some(pv));
            }

            if let Some((height, ts)) = newest_block {
                self.advance_header_block(height, ts);
            }

            hb_v2_install_snapshot(&snapshot_json, is_resync);
            let _ = self.inner.signals.set_loading.try_set(false);

            if let Some(param) = initial_search {
                match param {
                    InitialSearchParam::Tx(tx) => {
                        self.search(&format!("tx:{tx}"));
                    }
                    InitialSearchParam::Block(block) => {
                        self.search(&format!("block:{block}"));
                    }
                }
            }

            self.open_event_source(&epoch, seq, mount_gen, cycle_gen);
        }

        fn open_event_source(
            &self,
            epoch: &str,
            seq: u64,
            mount_gen: u64,
            cycle_gen: u64,
        ) {
            if !self.is_cycle_live(mount_gen, cycle_gen) {
                return;
            }
            let encoded_epoch: String =
                js_sys::encode_uri_component(epoch).into();
            let url = format!(
                "/api/stats/heartbeat/v2?epoch={encoded_epoch}&after={seq}"
            );
            let Ok(es) = EventSource::new(&url) else {
                self.set_conn_state("disconnected");
                self.schedule_retry(mount_gen, cycle_gen);
                return;
            };

            // 1. open
            let open_client = self.downgrade();
            let on_open =
                Closure::<dyn FnMut(Event)>::new(move |_ev: Event| {
                    let Some(open_client) = upgrade_client(&open_client) else {
                        return;
                    };
                    if !open_client.is_cycle_live(mount_gen, cycle_gen) {
                        return;
                    }
                    let (cs, note) = {
                        let mut st = open_client.inner.state.borrow_mut();
                        st.retry_count = 0;
                        st.connected = true;
                        st.resume_attempted = false;
                        let cs = connection_state_for_mirror(
                            true,
                            false,
                            false,
                            st.mirror.coverage(),
                        );
                        let note =
                            st.mirror.coverage().reason().map(str::to_string);
                        (cs, note)
                    };
                    open_client.set_conn_state(cs);
                    let _ = open_client
                        .inner
                        .signals
                        .set_coverage_note
                        .try_set(note);
                });

            // 2. delta
            let delta_client = self.downgrade();
            let on_delta =
                Closure::<dyn FnMut(Event)>::new(move |ev: Event| {
                    let Some(delta_client) = upgrade_client(&delta_client)
                    else {
                        return;
                    };
                    if !delta_client.is_cycle_live(mount_gen, cycle_gen) {
                        return;
                    }
                    let Ok(msg) = ev.dyn_into::<MessageEvent>() else {
                        return;
                    };
                    let Some(raw) = msg.data().as_string() else {
                        return;
                    };
                    let Ok(delta) =
                        serde_json::from_str::<HeartbeatDelta>(&raw)
                    else {
                        web_sys::console::warn_1(&JsValue::from_str(
                            "[heartbeat-v2] ignoring malformed delta frame",
                        ));
                        return;
                    };
                    delta_client.handle_delta(delta, mount_gen, cycle_gen);
                });

            // 3. resync_required: immediately close EventSource and fetch fresh snapshot.
            let resync_client = self.downgrade();
            let on_resync =
                Closure::<dyn FnMut(Event)>::new(move |_ev: Event| {
                    let Some(resync_client) = upgrade_client(&resync_client)
                    else {
                        return;
                    };
                    if !resync_client.is_cycle_live(mount_gen, cycle_gen) {
                        return;
                    }
                    resync_client.close_event_source();
                    resync_client.fetch_and_connect(true);
                });

            // 4. error: immediately close EventSource and schedule exponential backoff retry.
            let err_client = self.downgrade();
            let on_error =
                Closure::<dyn FnMut(Event)>::new(move |_ev: Event| {
                    let Some(err_client) = upgrade_client(&err_client) else {
                        return;
                    };
                    if !err_client.is_cycle_live(mount_gen, cycle_gen) {
                        return;
                    }
                    err_client.close_event_source();
                    err_client.set_conn_state("disconnected");
                    err_client.schedule_retry(mount_gen, cycle_gen);
                });

            let listeners: Vec<EventListenerEntry> = vec![
                ("open", on_open),
                ("delta", on_delta),
                ("resync_required", on_resync),
                ("error", on_error),
            ];
            for (name, cb) in &listeners {
                let _ = es.add_event_listener_with_callback(
                    name,
                    cb.as_ref().unchecked_ref(),
                );
            }

            {
                let mut st = self.inner.state.borrow_mut();
                st.event_source = Some(es);
                st.es_listeners = listeners;
            }
        }

        fn handle_delta(
            &self,
            delta: HeartbeatDelta,
            mount_gen: u64,
            cycle_gen: u64,
        ) {
            if !self.is_cycle_live(mount_gen, cycle_gen) {
                return;
            }
            let existing_pinned =
                self.inner.signals.pinned_tx.try_get_untracked().flatten();

            struct AppliedStep {
                effect_json: String,
                block_update: Option<(u64, u64)>,
                disconnected_reconcile: Option<Option<(u64, u64)>>,
                pending_update: Option<Option<PendingBlockBanner>>,
                coverage_update: Option<(&'static str, Option<String>)>,
                updated_pinned: Option<PinnedTxView>,
                fee_bands_update: Option<FeeBandSummary>,
            }

            enum Step {
                Ignore,
                Resync,
                Apply(Box<AppliedStep>),
            }

            let step = {
                let mut st = self.inner.state.borrow_mut();
                let (outcome, effect) =
                    st.mirror.apply_delta_with_effect(&delta);
                match outcome {
                    ApplyOutcome::DuplicateIgnored => Step::Ignore,
                    ApplyOutcome::ResyncNeeded(_) => Step::Resync,
                    ApplyOutcome::Applied => {
                        let Some(eff) = effect else {
                            return;
                        };
                        let effect_json = build_canvas_effect_json(&eff);
                        let block_update = match &eff {
                            ApplyEffect::BlockDetails(blk) => {
                                Some((blk.height, blk.timestamp))
                            }
                            _ => None,
                        };
                        let pending_update = match &eff {
                            ApplyEffect::BlockObserved {
                                hash,
                                observed_at,
                            } => Some(Some(PendingBlockBanner {
                                hash: hash.clone(),
                                observed_at: *observed_at,
                                received_at_client: client_now_unix_secs(),
                            })),
                            ApplyEffect::BlockDetails(_)
                            | ApplyEffect::BlockDisconnected { .. } => {
                                let client_now = client_now_unix_secs();
                                Some(st.mirror.pending_blocks().last().map(
                                    |pb| {
                                        let received_at_client = st
                                            .pending_block
                                            .as_ref()
                                            .filter(|existing| {
                                                existing.hash == pb.hash
                                            })
                                            .map(|existing| {
                                                existing.received_at_client
                                            })
                                            .unwrap_or_else(|| {
                                                if pb.observed_at > 0 {
                                                    pb.observed_at
                                                        .min(client_now)
                                                } else {
                                                    client_now
                                                }
                                            });
                                        PendingBlockBanner {
                                            hash: pb.hash.clone(),
                                            observed_at: pb.observed_at,
                                            received_at_client,
                                        }
                                    },
                                ))
                            }
                            _ => None,
                        };
                        let coverage_update = match &eff {
                            ApplyEffect::CoverageChanged { coverage } => {
                                let cs = connection_state_for_mirror(
                                    st.connected,
                                    false,
                                    false,
                                    coverage,
                                );
                                Some((
                                    cs,
                                    coverage.reason().map(str::to_string),
                                ))
                            }
                            _ => None,
                        };
                        let disconnected_hash = match &eff {
                            ApplyEffect::BlockDisconnected { hash } => {
                                Some(hash.as_str())
                            }
                            _ => None,
                        };
                        let disconnected_reconcile =
                            if disconnected_hash.is_some() {
                                Some(
                                    st.mirror
                                        .recent_blocks()
                                        .last()
                                        .map(|b| (b.height, b.timestamp)),
                                )
                            } else {
                                None
                            };
                        let force_queue_refresh = matches!(
                            eff,
                            ApplyEffect::BlockInclusion { .. }
                                | ApplyEffect::BlockDisconnected { .. }
                        );
                        let now_ms = js_sys::Date::now();
                        if matches!(eff, ApplyEffect::MembershipAdd { .. }) {
                            st.arrival_tracker.record(now_ms);
                        }
                        let bands_elapsed_ok =
                            (now_ms - st.last_bands_refresh_ms) >= 2_000.0;
                        let fee_bands_update =
                            if force_queue_refresh || bands_elapsed_ok {
                                st.last_bands_refresh_ms = now_ms;
                                let next = st.mirror.fee_band_summary();
                                if next != st.last_fee_bands {
                                    st.last_fee_bands = next;
                                    Some(next)
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                        let updated_pinned =
                            existing_pinned.as_ref().and_then(|existing| {
                                let mut pv = existing.clone();
                                if let Some(dhash) = disconnected_hash {
                                    apply_block_disconnected_to_pinned(
                                        &mut pv, dhash,
                                    );
                                }
                                let elapsed_ok = (now_ms
                                    - st.last_queue_refresh_ms)
                                    >= 2_500.0;
                                let refresh_queue =
                                    force_queue_refresh || elapsed_ok;
                                update_pinned_from_mirror_ex(
                                    &mut pv,
                                    &st.mirror,
                                    refresh_queue,
                                );
                                if refresh_queue
                                    && matches!(
                                        pv.status,
                                        PinnedTxStatus::Unconfirmed
                                    )
                                {
                                    st.last_queue_refresh_ms = now_ms;
                                }
                                (&pv != existing).then_some(pv)
                            });
                        Step::Apply(Box::new(AppliedStep {
                            effect_json,
                            block_update,
                            disconnected_reconcile,
                            pending_update,
                            coverage_update,
                            updated_pinned,
                            fee_bands_update,
                        }))
                    }
                }
            };

            match step {
                Step::Ignore => {}
                Step::Resync => {
                    self.close_event_source();
                    self.fetch_and_connect(true);
                }
                Step::Apply(applied) => {
                    let AppliedStep {
                        effect_json,
                        block_update,
                        disconnected_reconcile,
                        pending_update,
                        coverage_update,
                        updated_pinned,
                        fee_bands_update,
                    } = *applied;
                    hb_v2_apply_effect(&effect_json);
                    if let Some(bands) = fee_bands_update {
                        let _ = self.inner.signals.set_fee_bands.try_set(bands);
                    }
                    if let Some(pb) = pending_update {
                        self.set_pending_block_if_changed(pb);
                    }
                    if let Some((height, ts)) = block_update {
                        self.advance_header_block(height, ts);
                        js_bridge::refresh_rhythm_strip_from_canvas();
                    }
                    if let Some(remaining_tip) = disconnected_reconcile {
                        self.reconcile_header_block(remaining_tip);
                        js_bridge::refresh_rhythm_strip_from_canvas();
                    }
                    if let Some((cs, note)) = coverage_update {
                        self.set_conn_state(cs);
                        let _ =
                            self.inner.signals.set_coverage_note.try_set(note);
                    }
                    if let Some(pv) = updated_pinned {
                        let _ =
                            self.inner.signals.set_pinned_tx.try_set(Some(pv));
                    }
                }
            }
        }

        fn advance_header_block(&self, height: u64, ts: u64) {
            let current = self
                .inner
                .signals
                .sse_height
                .try_get_untracked()
                .unwrap_or(0);
            if height > current {
                let _ = self.inner.signals.set_sse_height.try_set(height);
                if ts > 0 {
                    let now = (js_sys::Date::now() / 1000.0) as u64;
                    let _ = self.inner.signals.set_sse_ts.try_set(ts.min(now));
                }
            }
        }

        fn reconcile_header_block(&self, remaining_tip: Option<(u64, u64)>) {
            match remaining_tip {
                Some((height, ts)) => {
                    let _ = self.inner.signals.set_sse_height.try_set(height);
                    if ts > 0 {
                        let now = (js_sys::Date::now() / 1000.0) as u64;
                        let _ =
                            self.inner.signals.set_sse_ts.try_set(ts.min(now));
                    }
                }
                None => {
                    let _ = self.inner.signals.set_sse_height.try_set(0);
                    let _ = self.inner.signals.set_sse_ts.try_set(0);
                }
            }
        }

        fn schedule_retry(&self, mount_gen: u64, cycle_gen: u64) {
            if !self.is_cycle_live(mount_gen, cycle_gen) {
                return;
            }
            self.clear_retry_timer();
            let delay = {
                let mut st = self.inner.state.borrow_mut();
                if st.destroyed {
                    return;
                }
                let base = retry_delay_ms(st.retry_count);
                st.retry_count = st.retry_count.saturating_add(1);
                let jitter = 0.7 + js_sys::Math::random() * 0.6;
                (base as f64 * jitter).round() as i32
            };

            let retry_client = self.downgrade();
            let cb = Closure::<dyn FnMut()>::new(move || {
                let Some(retry_client) = upgrade_client(&retry_client) else {
                    return;
                };
                if !retry_client.is_cycle_live(mount_gen, cycle_gen) {
                    return;
                }
                let resume_target = {
                    let mut st = retry_client.inner.state.borrow_mut();
                    st.retry_timer_id = None;
                    if !st.resume_attempted
                        && st.mirror.resync_reason().is_none()
                    {
                        if let Some((epoch, seq)) = st.mirror.revision() {
                            let target = (epoch.to_string(), seq);
                            st.resume_attempted = true;
                            Some(target)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                };
                if let Some((epoch, seq)) = resume_target {
                    retry_client.set_conn_state("syncing");
                    retry_client
                        .open_event_source(&epoch, seq, mount_gen, cycle_gen);
                } else {
                    retry_client.fetch_and_connect(true);
                }
            });

            if let Some(win) = web_sys::window() {
                if let Ok(id) = win
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        cb.as_ref().unchecked_ref(),
                        delay,
                    )
                {
                    let mut st = self.inner.state.borrow_mut();
                    st.retry_timer_id = Some(id);
                    st.retry_closure = Some(cb);
                }
            }
        }

        fn begin_lookup_generation(&self) -> (u64, u64) {
            let mount_gen = self.inner.generation.get();
            let lookup_gen = self.inner.lookup_gen.get().wrapping_add(1);
            self.inner.lookup_gen.set(lookup_gen);
            let prev = self.inner.state.borrow_mut().lookup_abort.take();
            if let Some(ac) = prev {
                ac.abort();
            }
            (mount_gen, lookup_gen)
        }

        fn begin_block_lookup_generation(&self) -> (u64, u64) {
            let mount_gen = self.inner.generation.get();
            let block_lookup_gen =
                self.inner.block_lookup_gen.get().wrapping_add(1);
            self.inner.block_lookup_gen.set(block_lookup_gen);
            let prev = self.inner.state.borrow_mut().block_lookup_abort.take();
            if let Some(ac) = prev {
                ac.abort();
            }
            (mount_gen, block_lookup_gen)
        }

        /// Unified search for a transaction (full txid or >= 8 hex prefix),
        /// block height (`840000`, `#840000`, `840,000`, `block:840000`), or block hash.
        pub fn search(&self, raw_query: &str) {
            let Some(query) = parse_search_query(raw_query) else {
                let _ = self.inner.signals.set_search_feedback.try_set(Some((
                    true,
                    "Enter a txid (8+ hex), block height (#840000), or block hash"
                        .to_string(),
                )));
                return;
            };

            match query {
                SearchQuery::Tx(needle) => {
                    if needle.len() < 8 {
                        let _ = self.inner.signals.set_search_feedback.try_set(
                            Some((
                                true,
                                "Enter at least 8 hex characters of a txid"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    let (mount_gen, lookup_gen) =
                        self.begin_lookup_generation();
                    let local_match = {
                        let st = self.inner.state.borrow();
                        st.mirror.search_tx(&needle)
                    };
                    if let Some(m) = local_match {
                        self.handle_local_tx_match(m, mount_gen, lookup_gen);
                        return;
                    }
                    if needle.len() < 64 {
                        let _ = self.inner.signals.set_search_feedback.try_set(
                            Some((
                                true,
                                "Not found in live mempool (paste full 64-char txid to search confirmed blocks)"
                                    .to_string(),
                            )),
                        );
                        return;
                    }
                    let _ = self.inner.signals.set_search_feedback.try_set(
                        Some((false, "Searching node...".to_string())),
                    );
                    self.fetch_server_tx_lookup(
                        &needle, true, false, mount_gen, lookup_gen,
                    );
                }
                SearchQuery::BlockHeight(height) => {
                    let (mount_gen, block_gen) =
                        self.begin_block_lookup_generation();
                    let in_live_mode =
                        self.inner.state.borrow().history_mode.is_none();
                    if in_live_mode && hb_v2_focus_block_height(height) {
                        let _ = self.inner.signals.set_search_feedback.try_set(
                            Some((
                                false,
                                format!(
                                    "Focused block #{}",
                                    format_u64_commas(height)
                                ),
                            )),
                        );
                        return;
                    }
                    let _ =
                        self.inner.signals.set_search_feedback.try_set(Some((
                            false,
                            format!(
                                "Loading block #{}...",
                                format_u64_commas(height)
                            ),
                        )));
                    self.fetch_block_window(
                        &height.to_string(),
                        Some(height),
                        None,
                        None,
                        mount_gen,
                        block_gen,
                    );
                }
                SearchQuery::BlockHash(hash) => {
                    let (mount_gen, block_gen) =
                        self.begin_block_lookup_generation();
                    let (in_live_mode, local_height) = {
                        let st = self.inner.state.borrow();
                        let h = st
                            .mirror
                            .recent_blocks()
                            .iter()
                            .find(|b| b.hash.eq_ignore_ascii_case(&hash))
                            .map(|b| b.height);
                        (st.history_mode.is_none(), h)
                    };
                    if let Some(h) = local_height {
                        if in_live_mode && hb_v2_focus_block_height(h) {
                            let _ =
                                self.inner.signals.set_search_feedback.try_set(
                                    Some((
                                        false,
                                        format!(
                                            "Focused block #{}",
                                            format_u64_commas(h)
                                        ),
                                    )),
                                );
                            return;
                        }
                    }
                    let _ = self
                        .inner
                        .signals
                        .set_search_feedback
                        .try_set(Some((false, "Loading block...".to_string())));
                    self.fetch_block_window(
                        &hash,
                        local_height,
                        None,
                        None,
                        mount_gen,
                        block_gen,
                    );
                }
                SearchQuery::AmbiguousHex64(hex) => {
                    let (in_live_mode, local_block_height, local_tx) = {
                        let st = self.inner.state.borrow();
                        let blk_h = st
                            .mirror
                            .recent_blocks()
                            .iter()
                            .find(|b| b.hash.eq_ignore_ascii_case(&hex))
                            .map(|b| b.height);
                        let tx_m = if blk_h.is_none() {
                            st.mirror.search_tx(&hex)
                        } else {
                            None
                        };
                        (st.history_mode.is_none(), blk_h, tx_m)
                    };
                    if let Some(h) = local_block_height {
                        let (mount_gen, block_gen) =
                            self.begin_block_lookup_generation();
                        if in_live_mode && hb_v2_focus_block_height(h) {
                            let _ =
                                self.inner.signals.set_search_feedback.try_set(
                                    Some((
                                        false,
                                        format!(
                                            "Focused block #{}",
                                            format_u64_commas(h)
                                        ),
                                    )),
                                );
                            return;
                        }
                        let _ = self.inner.signals.set_search_feedback.try_set(
                            Some((
                                false,
                                format!(
                                    "Loading block #{}...",
                                    format_u64_commas(h)
                                ),
                            )),
                        );
                        self.fetch_block_window(
                            &hex,
                            Some(h),
                            None,
                            None,
                            mount_gen,
                            block_gen,
                        );
                        return;
                    }
                    if let Some(m) = local_tx {
                        let (mount_gen, lookup_gen) =
                            self.begin_lookup_generation();
                        self.handle_local_tx_match(m, mount_gen, lookup_gen);
                        return;
                    }

                    let _ = self.inner.signals.set_search_feedback.try_set(
                        Some((false, "Searching node...".to_string())),
                    );
                    if hex.starts_with("00000000") {
                        let (mount_gen, block_gen) =
                            self.begin_block_lookup_generation();
                        self.fetch_block_window(
                            &hex,
                            None,
                            Some(hex.clone()),
                            None,
                            mount_gen,
                            block_gen,
                        );
                    } else {
                        let (mount_gen, lookup_gen) =
                            self.begin_lookup_generation();
                        self.fetch_server_tx_lookup(
                            &hex, true, true, mount_gen, lookup_gen,
                        );
                    }
                }
            }
        }

        /// Backwards-compatible alias delegating to unified `search`.
        pub fn search_tx(&self, raw_query: &str) {
            self.search(raw_query);
        }

        /// Focus a block spike on the live canvas, or load its historical ~40-block window.
        pub fn view_block_height(&self, height: u64) {
            let (mount_gen, block_gen) = self.begin_block_lookup_generation();
            let in_live_mode = self.inner.state.borrow().history_mode.is_none();
            if in_live_mode && hb_v2_focus_block_height(height) {
                let _ = self.inner.signals.set_search_feedback.try_set(Some((
                    false,
                    format!("Focused block #{}", format_u64_commas(height)),
                )));
                return;
            }
            let _ = self.inner.signals.set_search_feedback.try_set(Some((
                false,
                format!("Loading block #{}...", format_u64_commas(height)),
            )));
            self.fetch_block_window(
                &height.to_string(),
                Some(height),
                None,
                None,
                mount_gen,
                block_gen,
            );
        }

        /// Exit historical block window mode and return to the live Heartbeat stream.
        pub fn exit_history_mode(&self) {
            let _ = self.begin_block_lookup_generation();
            let _ = self.inner.signals.set_search_feedback.try_set(None);
            hb_v2_exit_history_mode();
        }

        /// Focus the canvas camera on the pending-block slot for `hash`.
        pub fn focus_pending_slot(&self, hash: &str) {
            let _ = hb_v2_focus_pending_slot(hash);
        }

        /// Recompute and publish the rolling 60-second arrival rate (`tx/s`).
        /// Called on the page's existing 1s UI timer so quiet periods smoothly
        /// decay toward `0.0 tx/s` without allocating a second timer.
        pub fn refresh_arrival_rate(&self) {
            let now_ms = js_sys::Date::now();
            let rate = {
                let mut st = self.inner.state.borrow_mut();
                if st.destroyed {
                    return;
                }
                st.arrival_tracker.rate_tx_per_sec(now_ms)
            };
            let _ = self.inner.signals.set_arrival_rate.try_set(rate);
        }

        fn handle_local_tx_match(
            &self,
            m: MirrorTxMatch,
            mount_gen: u64,
            lookup_gen: u64,
        ) {
            match m {
                MirrorTxMatch::Unconfirmed(tx) => {
                    let on_canvas = hb_v2_focus_tx(&tx.txid);
                    if !on_canvas {
                        hb_v2_set_pinned_tx(Some(&tx.txid));
                    }
                    let mut pv = pinned_from_compact(&tx, on_canvas);
                    {
                        let mut st = self.inner.state.borrow_mut();
                        pv.queue_position = st.mirror.queue_position(&tx.txid);
                        st.last_queue_refresh_ms = js_sys::Date::now();
                    }
                    pv.deep_loading = true;
                    let txid = tx.txid.clone();
                    let _ = self.inner.signals.set_pinned_tx.try_set(Some(pv));
                    let msg = if on_canvas {
                        "Found in mempool".to_string()
                    } else {
                        "In node mempool (outside rendered window)".to_string()
                    };
                    let _ = self
                        .inner
                        .signals
                        .set_search_feedback
                        .try_set(Some((false, msg)));
                    self.fetch_deep_tx_details(&txid, mount_gen, lookup_gen);
                }
                MirrorTxMatch::Confirmed {
                    txid,
                    block_hash,
                    block_height,
                } => {
                    hb_v2_set_pinned_tx(None);
                    let mut pv = pinned_from_confirmed(
                        txid.clone(),
                        block_hash,
                        block_height,
                    );
                    pv.deep_loading = true;
                    let _ = self.inner.signals.set_pinned_tx.try_set(Some(pv));
                    let _ =
                        self.inner.signals.set_search_feedback.try_set(Some((
                            false,
                            format!(
                                "Confirmed in block #{}",
                                format_u64_commas(block_height)
                            ),
                        )));
                    self.fetch_deep_tx_details(&txid, mount_gen, lookup_gen);
                }
            }
        }

        /// Fetch `/api/stats/heartbeat/block/{query}?radius=40` and enter canvas history mode.
        fn fetch_block_window(
            &self,
            query: &str,
            requested_height: Option<u64>,
            fallback_tx_on_404: Option<String>,
            not_found_msg: Option<&'static str>,
            mount_gen: u64,
            block_lookup_gen: u64,
        ) {
            let Ok(controller) = AbortController::new() else {
                return;
            };
            let fetch_controller = controller.clone();
            {
                self.inner.state.borrow_mut().block_lookup_abort =
                    Some(controller);
            }

            let encoded: String = js_sys::encode_uri_component(query).into();
            let url = format!("/api/stats/heartbeat/block/{encoded}?radius=40");
            let fb_ok = fallback_tx_on_404.clone();
            let fb_err = fallback_tx_on_404;
            let ok_client = self.downgrade();
            let err_client = self.downgrade();

            fetch_text_with_abort(
                &url,
                &fetch_controller,
                move |status, body| {
                    let Some(ok_client) = upgrade_client(&ok_client) else {
                        return;
                    };
                    if !ok_client
                        .is_block_lookup_live(mount_gen, block_lookup_gen)
                    {
                        return;
                    }
                    {
                        ok_client.inner.state.borrow_mut().block_lookup_abort =
                            None;
                    }
                    if status == 200 {
                        let parsed_info = parse_history_mode_info(&body);
                        let target_height = parsed_info
                            .as_ref()
                            .map(|info| info.target_height)
                            .or(requested_height);
                        if hb_v2_enter_history_mode(&body) {
                            if let Some(info) = parsed_info {
                                ok_client
                                    .set_history_mode_if_changed(Some(info));
                            }
                            let msg = match target_height {
                                Some(h) => format!(
                                    "Historical view: block #{}",
                                    format_u64_commas(h)
                                ),
                                None => "Historical view loaded".to_string(),
                            };
                            let _ = ok_client
                                .inner
                                .signals
                                .set_search_feedback
                                .try_set(Some((false, msg)));
                        } else {
                            let _ = ok_client
                                .inner
                                .signals
                                .set_search_feedback
                                .try_set(Some((
                                    true,
                                    "Unable to display historical block window"
                                        .to_string(),
                                )));
                        }
                        return;
                    }

                    if status == 404 {
                        if let Some(txid) = fb_ok {
                            let (tx_mount_gen, tx_lookup_gen) =
                                ok_client.begin_lookup_generation();
                            ok_client.fetch_server_tx_lookup(
                                &txid,
                                true,
                                false,
                                tx_mount_gen,
                                tx_lookup_gen,
                            );
                            return;
                        }
                        let msg = if let Some(custom) = not_found_msg {
                            custom.to_string()
                        } else if let Some(h) = requested_height {
                            format!(
                                "Block #{} not found in database",
                                format_u64_commas(h)
                            )
                        } else {
                            "Block not found in database".to_string()
                        };
                        let _ = ok_client
                            .inner
                            .signals
                            .set_search_feedback
                            .try_set(Some((true, msg)));
                        return;
                    }

                    if let Some(txid) = fb_ok {
                        let (tx_mount_gen, tx_lookup_gen) =
                            ok_client.begin_lookup_generation();
                        ok_client.fetch_server_tx_lookup(
                            &txid,
                            true,
                            false,
                            tx_mount_gen,
                            tx_lookup_gen,
                        );
                        return;
                    }
                    let _ =
                        ok_client.inner.signals.set_search_feedback.try_set(
                            Some((
                                true,
                                "Failed to load block window".to_string(),
                            )),
                        );
                },
                move |_err| {
                    let Some(err_client) = upgrade_client(&err_client) else {
                        return;
                    };
                    if !err_client
                        .is_block_lookup_live(mount_gen, block_lookup_gen)
                    {
                        return;
                    }
                    {
                        err_client
                            .inner
                            .state
                            .borrow_mut()
                            .block_lookup_abort = None;
                    }
                    if let Some(txid) = fb_err {
                        let (tx_mount_gen, tx_lookup_gen) =
                            err_client.begin_lookup_generation();
                        err_client.fetch_server_tx_lookup(
                            &txid,
                            true,
                            false,
                            tx_mount_gen,
                            tx_lookup_gen,
                        );
                        return;
                    }
                    let _ =
                        err_client.inner.signals.set_search_feedback.try_set(
                            Some((
                                true,
                                "Failed to load block window".to_string(),
                            )),
                        );
                },
            );
        }

        /// Select a transaction clicked on the canvas or in The Lookout without jumping the camera.
        pub fn select_tx_from_canvas(&self, raw_txid: &str) {
            let Some(txid) = normalize_tx_query(raw_txid) else {
                return;
            };
            let (mount_gen, lookup_gen) = self.begin_lookup_generation();

            let local = {
                let st = self.inner.state.borrow();
                st.mirror.lookup_tx(&txid)
            };

            match local {
                TxLookupStatus::Unconfirmed(tx) => {
                    hb_v2_set_pinned_tx(Some(&tx.txid));
                    let mut pv = pinned_from_compact(&tx, true);
                    {
                        let mut st = self.inner.state.borrow_mut();
                        pv.queue_position = st.mirror.queue_position(&tx.txid);
                        st.last_queue_refresh_ms = js_sys::Date::now();
                    }
                    pv.deep_loading = true;
                    let id = tx.txid.clone();
                    let _ = self.inner.signals.set_pinned_tx.try_set(Some(pv));
                    self.fetch_deep_tx_details(&id, mount_gen, lookup_gen);
                }
                TxLookupStatus::Confirmed {
                    txid,
                    block_hash,
                    block_height,
                } => {
                    hb_v2_set_pinned_tx(None);
                    let mut pv = pinned_from_confirmed(
                        txid.clone(),
                        block_hash,
                        block_height,
                    );
                    pv.deep_loading = true;
                    let _ = self.inner.signals.set_pinned_tx.try_set(Some(pv));
                    self.fetch_deep_tx_details(&txid, mount_gen, lookup_gen);
                }
                TxLookupStatus::Absent { .. } => {
                    if txid.len() == 64 {
                        self.fetch_server_tx_lookup(
                            &txid, false, false, mount_gen, lookup_gen,
                        );
                    }
                }
            }
        }

        /// Clear the pinned transaction panel and canvas pin highlight.
        pub fn clear_pinned_tx(&self) {
            let _ = self.begin_lookup_generation();
            let _ = self.inner.signals.set_pinned_tx.try_set(None);
            hb_v2_set_pinned_tx(None);
        }

        fn fetch_server_tx_lookup(
            &self,
            txid: &str,
            focus_camera: bool,
            fallback_to_block: bool,
            mount_gen: u64,
            lookup_gen: u64,
        ) {
            let Ok(controller) = AbortController::new() else {
                return;
            };
            let fetch_controller = controller.clone();
            {
                self.inner.state.borrow_mut().lookup_abort = Some(controller);
            }

            let url = format!("/api/stats/heartbeat/tx/{txid}");
            let txid_ok = txid.to_string();
            let txid_err = txid.to_string();
            let ok_client = self.downgrade();
            let err_client = self.downgrade();

            fetch_text_with_abort(
                &url,
                &fetch_controller,
                move |status, body| {
                    let Some(ok_client) = upgrade_client(&ok_client) else {
                        return;
                    };
                    if !ok_client.is_lookup_live(mount_gen, lookup_gen) {
                        return;
                    }
                    if status == 200 {
                        if let Ok(parsed) =
                            serde_json::from_str::<TxLookupStatus>(&body)
                        {
                            match parsed {
                                TxLookupStatus::Unconfirmed(tx) => {
                                    let on_canvas = if focus_camera {
                                        let focused = hb_v2_focus_tx(&tx.txid);
                                        if !focused {
                                            hb_v2_set_pinned_tx(Some(&tx.txid));
                                        }
                                        focused
                                    } else {
                                        hb_v2_set_pinned_tx(Some(&tx.txid));
                                        false
                                    };
                                    let mut pv =
                                        pinned_from_compact(&tx, on_canvas);
                                    if let (Some(f), Some(vs)) =
                                        (tx.fee, tx.vsize)
                                    {
                                        let mut st =
                                            ok_client.inner.state.borrow_mut();
                                        pv.queue_position =
                                            st.mirror.queue_position_for_rate(
                                                f,
                                                vs,
                                                Some(&tx.txid),
                                            );
                                        st.last_queue_refresh_ms =
                                            js_sys::Date::now();
                                    }
                                    pv.deep_loading = true;
                                    let id = tx.txid.clone();
                                    let _ = ok_client
                                        .inner
                                        .signals
                                        .set_pinned_tx
                                        .try_set(Some(pv));
                                    if focus_camera {
                                        let _ = ok_client
                                            .inner
                                            .signals
                                            .set_search_feedback
                                            .try_set(Some((
                                                false,
                                                "Found in node mempool"
                                                    .to_string(),
                                            )));
                                    }
                                    ok_client.fetch_deep_tx_details(
                                        &id, mount_gen, lookup_gen,
                                    );
                                    return;
                                }
                                TxLookupStatus::Confirmed {
                                    txid,
                                    block_hash,
                                    block_height,
                                } => {
                                    hb_v2_set_pinned_tx(None);
                                    let mut pv = pinned_from_confirmed(
                                        txid.clone(),
                                        block_hash,
                                        block_height,
                                    );
                                    pv.deep_loading = true;
                                    let _ = ok_client
                                        .inner
                                        .signals
                                        .set_pinned_tx
                                        .try_set(Some(pv));
                                    if focus_camera {
                                        let _ = ok_client
                                            .inner
                                            .signals
                                            .set_search_feedback
                                            .try_set(Some((
                                                false,
                                                format!(
                                                    "Confirmed in block #{}",
                                                    format_u64_commas(
                                                        block_height
                                                    )
                                                ),
                                            )));
                                    }
                                    ok_client.fetch_deep_tx_details(
                                        &txid, mount_gen, lookup_gen,
                                    );
                                    return;
                                }
                                TxLookupStatus::Absent { coverage } => {
                                    ok_client.fetch_historical_tx_fallback(
                                        &txid_ok,
                                        Some(coverage),
                                        focus_camera,
                                        fallback_to_block,
                                        mount_gen,
                                        lookup_gen,
                                    );
                                    return;
                                }
                            }
                        }
                    }
                    ok_client.fetch_historical_tx_fallback(
                        &txid_ok,
                        None,
                        focus_camera,
                        fallback_to_block,
                        mount_gen,
                        lookup_gen,
                    );
                },
                move |_err| {
                    let Some(err_client) = upgrade_client(&err_client) else {
                        return;
                    };
                    if !err_client.is_lookup_live(mount_gen, lookup_gen) {
                        return;
                    }
                    err_client.fetch_historical_tx_fallback(
                        &txid_err,
                        None,
                        focus_camera,
                        fallback_to_block,
                        mount_gen,
                        lookup_gen,
                    );
                },
            );
        }

        /// Fallback to `/api/stats/tx/{txid}` for confirmed txs older than the 10k confirmation ring.
        fn fetch_historical_tx_fallback(
            &self,
            txid: &str,
            coverage: Option<Coverage>,
            focus_camera: bool,
            fallback_to_block: bool,
            mount_gen: u64,
            lookup_gen: u64,
        ) {
            let Ok(controller) = AbortController::new() else {
                return;
            };
            let fetch_controller = controller.clone();
            {
                self.inner.state.borrow_mut().lookup_abort = Some(controller);
            }

            let url = format!("/api/stats/tx/{txid}");
            let txid_owned = txid.to_string();
            let txid_err = txid.to_string();
            let cov_err = coverage.clone();
            let ok_client = self.downgrade();
            let err_client = self.downgrade();

            fetch_text_with_abort(
                &url,
                &fetch_controller,
                move |status, body| {
                    let Some(ok_client) = upgrade_client(&ok_client) else {
                        return;
                    };
                    if !ok_client.is_lookup_live(mount_gen, lookup_gen) {
                        return;
                    }
                    if status == 200 {
                        if let Ok(raw) =
                            serde_json::from_str::<RawTxDetailResponse>(&body)
                        {
                            let is_confirmed = raw.confirmed.unwrap_or(false)
                                || raw.confirmations.unwrap_or(0) > 0;
                            let block_height = raw.block_height.unwrap_or(0);
                            let block_hash =
                                raw.block_hash.clone().unwrap_or_default();
                            let (tx_status, on_canvas, feedback_msg) =
                                if is_confirmed {
                                    hb_v2_set_pinned_tx(None);
                                    let msg = if block_height > 0 {
                                        format!(
                                            "Confirmed in block #{}",
                                            format_u64_commas(block_height)
                                        )
                                    } else {
                                        format!(
                                            "Confirmed ({} confirmations)",
                                            format_u64_commas(
                                                raw.confirmations.unwrap_or(1)
                                            )
                                        )
                                    };
                                    (
                                        PinnedTxStatus::Confirmed {
                                            block_hash,
                                            block_height,
                                        },
                                        false,
                                        msg,
                                    )
                                } else {
                                    let on_c = if focus_camera {
                                        hb_v2_focus_tx(&txid_owned)
                                    } else {
                                        hb_v2_set_pinned_tx(Some(&txid_owned));
                                        false
                                    };
                                    (
                                        PinnedTxStatus::Unconfirmed,
                                        on_c,
                                        "Found on node".to_string(),
                                    )
                                };

                            let mut pv = PinnedTxView {
                                txid: txid_owned.clone(),
                                status: tx_status,
                                fee: None,
                                vsize: None,
                                value: None,
                                first_seen: 0,
                                flags: TxFlags::NONE,
                                op_return_text: None,
                                on_canvas,
                                deep: None,
                                deep_loading: false,
                                queue_position: None,
                            };
                            apply_deep_details(&mut pv, &raw);
                            if matches!(pv.status, PinnedTxStatus::Unconfirmed)
                            {
                                if let (Some(f), Some(vs)) = (pv.fee, pv.vsize)
                                {
                                    let mut st =
                                        ok_client.inner.state.borrow_mut();
                                    pv.queue_position =
                                        st.mirror.queue_position_for_rate(
                                            f,
                                            vs,
                                            Some(&txid_owned),
                                        );
                                    st.last_queue_refresh_ms =
                                        js_sys::Date::now();
                                }
                            }
                            let _ = ok_client
                                .inner
                                .signals
                                .set_pinned_tx
                                .try_set(Some(pv));
                            if focus_camera {
                                let _ = ok_client
                                    .inner
                                    .signals
                                    .set_search_feedback
                                    .try_set(Some((false, feedback_msg)));
                            }
                            return;
                        }
                    }

                    if fallback_to_block {
                        let msg = ambiguous_hex_not_found_message(
                            coverage.as_ref(),
                            status >= 500,
                        );
                        let (blk_mount_gen, blk_gen) =
                            ok_client.begin_block_lookup_generation();
                        ok_client.fetch_block_window(
                            &txid_owned,
                            None,
                            None,
                            Some(msg),
                            blk_mount_gen,
                            blk_gen,
                        );
                        return;
                    }

                    if focus_camera {
                        let msg = match coverage {
                            Some(ref c) if !c.is_complete() => {
                                "Not in node mempool (coverage is currently partial)"
                                    .to_string()
                            }
                            _ => "Not in node mempool".to_string(),
                        };
                        let _ = ok_client
                            .inner
                            .signals
                            .set_search_feedback
                            .try_set(Some((true, msg)));
                    }
                },
                move |_err| {
                    let Some(err_client) = upgrade_client(&err_client) else {
                        return;
                    };
                    if !err_client.is_lookup_live(mount_gen, lookup_gen) {
                        return;
                    }
                    if fallback_to_block {
                        let msg = ambiguous_hex_not_found_message(
                            cov_err.as_ref(),
                            true,
                        );
                        let (blk_mount_gen, blk_gen) =
                            err_client.begin_block_lookup_generation();
                        err_client.fetch_block_window(
                            &txid_err,
                            None,
                            None,
                            Some(msg),
                            blk_mount_gen,
                            blk_gen,
                        );
                        return;
                    }
                    if focus_camera {
                        let msg = match cov_err {
                            Some(ref c) if !c.is_complete() => {
                                "Not in node mempool (coverage is currently partial)"
                                    .to_string()
                            }
                            _ => "Not in node mempool".to_string(),
                        };
                        let _ = err_client
                            .inner
                            .signals
                            .set_search_feedback
                            .try_set(Some((true, msg)));
                    }
                },
            );
        }

        fn fetch_deep_tx_details(
            &self,
            txid: &str,
            mount_gen: u64,
            lookup_gen: u64,
        ) {
            let Ok(controller) = AbortController::new() else {
                return;
            };
            let fetch_controller = controller.clone();
            {
                self.inner.state.borrow_mut().lookup_abort = Some(controller);
            }

            let url = format!("/api/stats/tx/{txid}");
            let txid_ok = txid.to_string();
            let txid_err = txid.to_string();
            let ok_client = self.downgrade();
            let err_client = self.downgrade();

            fetch_text_with_abort(
                &url,
                &fetch_controller,
                move |status, body| {
                    let Some(ok_client) = upgrade_client(&ok_client) else {
                        return;
                    };
                    if !ok_client.is_lookup_live(mount_gen, lookup_gen) {
                        return;
                    }
                    if status == 200 {
                        if let Ok(raw) =
                            serde_json::from_str::<RawTxDetailResponse>(&body)
                        {
                            let _ = ok_client
                                .inner
                                .signals
                                .set_pinned_tx
                                .try_update(|opt| {
                                    if let Some(p) = opt.as_mut() {
                                        if p.txid == txid_ok {
                                            apply_deep_details(p, &raw);
                                        }
                                    }
                                });
                            return;
                        }
                    }
                    let _ = ok_client.inner.signals.set_pinned_tx.try_update(
                        |opt| {
                            if let Some(p) = opt.as_mut() {
                                if p.txid == txid_ok {
                                    p.deep_loading = false;
                                }
                            }
                        },
                    );
                },
                move |_err| {
                    let Some(err_client) = upgrade_client(&err_client) else {
                        return;
                    };
                    if !err_client.is_lookup_live(mount_gen, lookup_gen) {
                        return;
                    }
                    let _ = err_client.inner.signals.set_pinned_tx.try_update(
                        |opt| {
                            if let Some(p) = opt.as_mut() {
                                if p.txid == txid_err {
                                    p.deep_loading = false;
                                }
                            }
                        },
                    );
                },
            );
        }
    }

    fn upgrade_client(weak: &Weak<ClientInner>) -> Option<HeartbeatClient> {
        weak.upgrade().map(|inner| HeartbeatClient { inner })
    }

    type FetchCallbacks =
        (Box<dyn FnOnce(u16, String)>, Box<dyn FnOnce(String)>);

    fn teardown_event_source(
        es: Option<EventSource>,
        listeners: Vec<EventListenerEntry>,
    ) {
        if let Some(source) = es {
            for (name, cb) in &listeners {
                let _ = source.remove_event_listener_with_callback(
                    name,
                    cb.as_ref().unchecked_ref(),
                );
            }
            source.close();
        }
        drop(listeners);
    }

    fn fetch_text_with_abort(
        url: &str,
        controller: &AbortController,
        on_ok: impl FnOnce(u16, String) + 'static,
        on_err: impl FnOnce(String) + 'static,
    ) {
        let Some(win) = web_sys::window() else {
            on_err("window unavailable".to_string());
            return;
        };
        let init = RequestInit::new();
        init.set_signal(Some(&controller.signal()));
        let promise = win.fetch_with_str_and_init(url, &init);

        // Hold both callbacks in one shared slot so whichever settlement path
        // runs takes and drops both closures immediately instead of pinning
        // captures on the uninvoked branch.
        let callbacks: Rc<RefCell<Option<FetchCallbacks>>> =
            Rc::new(RefCell::new(Some((Box::new(on_ok), Box::new(on_err)))));
        let cbs_resp = Rc::clone(&callbacks);
        let cbs_fetch_err = Rc::clone(&callbacks);

        let on_resp = Closure::once(move |val: JsValue| {
            let Ok(resp) = val.dyn_into::<Response>() else {
                if let Some((_, on_err)) = cbs_resp.borrow_mut().take() {
                    on_err("invalid fetch response".to_string());
                }
                return;
            };
            let status = resp.status();
            match resp.text() {
                Ok(text_promise) => {
                    let cbs_text_ok = Rc::clone(&cbs_resp);
                    let cbs_text_err = Rc::clone(&cbs_resp);
                    let on_text_ok = Closure::once(move |txt: JsValue| {
                        let body = txt.as_string().unwrap_or_default();
                        if let Some((on_ok, _)) =
                            cbs_text_ok.borrow_mut().take()
                        {
                            on_ok(status, body);
                        }
                    });
                    let on_text_err = Closure::once(move |e: JsValue| {
                        if let Some((_, on_err)) =
                            cbs_text_err.borrow_mut().take()
                        {
                            on_err(format!("{e:?}"));
                        }
                    });
                    let _ = text_promise.then2(&on_text_ok, &on_text_err);
                    on_text_ok.forget();
                    on_text_err.forget();
                }
                Err(e) => {
                    if let Some((_, on_err)) = cbs_resp.borrow_mut().take() {
                        on_err(format!("{e:?}"));
                    }
                }
            }
        });

        let on_fetch_err = Closure::once(move |e: JsValue| {
            if let Some((_, on_err)) = cbs_fetch_err.borrow_mut().take() {
                on_err(format!("{e:?}"));
            }
        });

        let _ = promise.then2(&on_resp, &on_fetch_err);
        on_resp.forget();
        on_fetch_err.forget();
    }
}

#[cfg(feature = "hydrate")]
pub use client::{HeartbeatClient, HeartbeatClientSignals};

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::heartbeat_sync::{
        DeltaKind, HeartbeatDelta, HeartbeatSnapshot, PROTOCOL_VERSION,
    };

    const EPOCH: &str = "epoch-test-1";

    fn txid(seed: u8) -> String {
        format!("{seed:02x}").repeat(32)
    }

    fn base_snapshot(seq: u64, txs: Vec<CompactTx>) -> HeartbeatSnapshot {
        HeartbeatSnapshot {
            protocol_version: PROTOCOL_VERSION,
            stream_epoch: EPOCH.to_string(),
            snapshot_id: format!("snap-{seq}"),
            seq,
            generated_at: 1_700_000_000,
            node_observed_at: Some(1_700_000_000),
            coverage: Coverage::Complete,
            node_mempool: None,
            pending_blocks: vec![],
            recent_blocks: vec![],
            recent_confirmations: vec![],
            transactions: txs,
        }
    }

    fn delta(seq: u64, kind: DeltaKind) -> HeartbeatDelta {
        HeartbeatDelta {
            protocol_version: PROTOCOL_VERSION,
            stream_epoch: EPOCH.to_string(),
            seq,
            observed_at: 1_700_000_000 + seq,
            kind,
        }
    }

    #[test]
    fn fee_band_covers_pending_sub_one_and_six_fixed_bands() {
        assert_eq!(fee_band_for_rate(None), ("Fee pending", "#64748b"));
        assert_eq!(
            fee_band_for_rate(Some(f64::NAN)),
            ("Fee pending", "#64748b")
        );
        assert_eq!(fee_band_for_rate(Some(0.0)), ("< 1 sat/vB", "#2563eb"));
        assert_eq!(fee_band_for_rate(Some(0.45)), ("< 1 sat/vB", "#2563eb"));
        assert_eq!(fee_band_for_rate(Some(0.999)), ("< 1 sat/vB", "#2563eb"));
        assert_eq!(fee_band_for_rate(Some(1.0)), ("1-2 sat/vB", "#38bdf8"));
        assert_eq!(fee_band_for_rate(Some(1.99)), ("1-2 sat/vB", "#38bdf8"));
        assert_eq!(fee_band_for_rate(Some(2.0)), ("2-5 sat/vB", "#00e676"));
        assert_eq!(fee_band_for_rate(Some(4.9)), ("2-5 sat/vB", "#00e676"));
        assert_eq!(fee_band_for_rate(Some(5.0)), ("5-15 sat/vB", "#f7931a"));
        assert_eq!(fee_band_for_rate(Some(14.9)), ("5-15 sat/vB", "#f7931a"));
        assert_eq!(fee_band_for_rate(Some(15.0)), ("15-50 sat/vB", "#ff5722"));
        assert_eq!(fee_band_for_rate(Some(49.9)), ("15-50 sat/vB", "#ff5722"));
        assert_eq!(fee_band_for_rate(Some(50.0)), ("50+ sat/vB", "#f44336"));
        assert_eq!(fee_band_for_rate(Some(250.5)), ("50+ sat/vB", "#f44336"));
    }

    #[test]
    fn format_fee_rate_handles_pending_sub_one_and_above_one() {
        assert_eq!(format_fee_rate(None), "Pending");
        assert_eq!(format_fee_rate(Some(f64::INFINITY)), "Pending");
        assert_eq!(format_fee_rate(Some(0.0)), "0.0 sat/vB");
        assert_eq!(format_fee_rate(Some(0.45)), "0.45 sat/vB");
        assert_eq!(format_fee_rate(Some(0.994)), "0.99 sat/vB");
        assert_eq!(format_fee_rate(Some(1.0)), "1.0 sat/vB");
        assert_eq!(format_fee_rate(Some(12.34)), "12.3 sat/vB");
    }

    #[test]
    fn primary_notable_label_follows_priority_order() {
        assert_eq!(primary_notable_label(TxFlags::NONE), None);
        assert_eq!(
            primary_notable_label(TxFlags::ALL).map(|(l, _)| l),
            Some("Whale")
        );
        assert_eq!(
            primary_notable_label(
                TxFlags::ROUND_NUMBER | TxFlags::OP_RETURN_MSG
            )
            .map(|(l, _)| l),
            Some("Round #")
        );
        assert_eq!(
            primary_notable_label(
                TxFlags::LARGE_INSCRIPTION | TxFlags::CONSOLIDATION
            )
            .map(|(l, _)| l),
            Some("Large Inscription")
        );
        assert_eq!(
            primary_notable_label(TxFlags::CONSOLIDATION | TxFlags::FAN_OUT)
                .map(|(l, _)| l),
            Some("Consolidation")
        );
        assert_eq!(
            primary_notable_label(TxFlags::FAN_OUT | TxFlags::FEE_OUTLIER)
                .map(|(l, _)| l),
            Some("Fan-out")
        );
        assert_eq!(
            primary_notable_label(
                TxFlags::FEE_OUTLIER | TxFlags::OP_RETURN_MSG
            )
            .map(|(l, _)| l),
            Some("Fee Outlier")
        );
        assert_eq!(
            primary_notable_label(TxFlags::OP_RETURN_MSG).map(|(l, _)| l),
            Some("OP_RETURN Msg")
        );
    }

    #[test]
    fn update_pinned_from_mirror_tracks_full_tx_lifecycle_in_place() {
        let id = txid(0xaa);
        let blk_hash = txid(0xbb);

        let mut mirror = HeartbeatMirror::default();
        mirror
            .install_snapshot(base_snapshot(
                10,
                vec![CompactTx::pending(id.clone(), 1_700_000_005)],
            ))
            .unwrap();

        // 1. Start pinned as Unconfirmed (fee pending) with deep details loaded.
        let mut pinned = pinned_from_compact(mirror.get_tx(&id).unwrap(), true);
        pinned.deep = Some(TxDeepDetails {
            inputs: Some(2),
            outputs: Some(3),
            size: Some(280),
            weight: Some(896),
            confirmations: Some(0),
            input_items: vec![],
            output_items: vec![],
            features: vec!["SegWit".to_string(), "RBF".to_string()],
        });
        assert_eq!(pinned.status, PinnedTxStatus::Unconfirmed);
        assert_eq!(pinned.fee, None);
        assert_eq!(pinned.queue_position, None);

        // 2. TxDetails enriches fee/vsize/value/flags/op_return while preserving deep details.
        mirror.apply_delta(delta(
            11,
            DeltaKind::TxDetails {
                txid: id.clone(),
                fee: 1_500,
                vsize: 224,
                value: Some(12_500_000),
                flags: TxFlags::OP_RETURN_MSG,
                op_return_text: Some("gm bitcoin".to_string()),
                first_seen: None,
            },
        ));
        update_pinned_from_mirror(&mut pinned, &mirror);
        assert_eq!(pinned.status, PinnedTxStatus::Unconfirmed);
        assert_eq!(pinned.fee, Some(1_500));
        assert_eq!(pinned.vsize, Some(224));
        assert_eq!(pinned.value, Some(12_500_000));
        assert_eq!(pinned.flags, TxFlags::OP_RETURN_MSG);
        assert_eq!(pinned.op_return_text.as_deref(), Some("gm bitcoin"));
        assert!(pinned.deep.is_some());
        assert_eq!(
            pinned.queue_position,
            Some(crate::stats::heartbeat_sync::QueuePosition {
                txs_ahead: 0,
                vbytes_ahead: 0,
                est_blocks: 1,
                unenriched_count: 0,
            })
        );

        // 3. BlockObserved + BlockInclusion confirms tx in place, preserving fee/vsize/value
        //    and clearing queue_position.
        mirror.apply_delta(delta(
            12,
            DeltaKind::BlockObserved {
                hash: blk_hash.clone(),
                observed_at: 1_700_000_012,
            },
        ));
        mirror.apply_delta(delta(
            13,
            DeltaKind::BlockInclusion {
                hash: blk_hash.clone(),
                height: 976_728,
                txids: vec![id.clone()],
            },
        ));
        update_pinned_from_mirror(&mut pinned, &mirror);
        assert_eq!(
            pinned.status,
            PinnedTxStatus::Confirmed {
                block_hash: blk_hash.clone(),
                block_height: 976_728,
            }
        );
        assert!(!pinned.on_canvas);
        assert_eq!(pinned.queue_position, None);
        assert_eq!(pinned.fee, Some(1_500));
        assert_eq!(pinned.vsize, Some(224));
        assert_eq!(pinned.value, Some(12_500_000));

        // 4. BlockDisconnected retracts confirmation; re-admission returns it to Unconfirmed.
        mirror.apply_delta(delta(
            14,
            DeltaKind::BlockDisconnected {
                hash: blk_hash.clone(),
            },
        ));
        apply_block_disconnected_to_pinned(&mut pinned, &blk_hash);
        update_pinned_from_mirror(&mut pinned, &mirror);
        assert_eq!(pinned.status, PinnedTxStatus::Removed);

        mirror.apply_delta(delta(
            15,
            DeltaKind::MembershipAdd {
                txid: id.clone(),
                first_seen: 1_700_000_015,
            },
        ));
        update_pinned_from_mirror(&mut pinned, &mirror);
        assert_eq!(pinned.status, PinnedTxStatus::Unconfirmed);
        // Preserves known fee/vsize/value across reorg re-add.
        assert_eq!(pinned.fee, Some(1_500));
        assert_eq!(pinned.vsize, Some(224));

        // 5. MembershipRemove transitions Unconfirmed -> Removed (RBF / eviction).
        mirror.apply_delta(delta(
            16,
            DeltaKind::MembershipRemove {
                txids: vec![id.clone()],
            },
        ));
        update_pinned_from_mirror(&mut pinned, &mirror);
        assert_eq!(pinned.status, PinnedTxStatus::Removed);
        assert!(!pinned.on_canvas);
    }

    #[test]
    fn normalize_tx_query_and_parse_tx_param_validate_inputs() {
        let full = txid(0xab);
        assert_eq!(normalize_tx_query(&full), Some(full.clone()));
        assert_eq!(
            normalize_tx_query(&format!("  TX:{}  ", full.to_uppercase())),
            Some(full.clone())
        );
        assert_eq!(
            normalize_tx_query("tx:AbCd1234"),
            Some("abcd1234".to_string())
        );
        assert_eq!(normalize_tx_query(""), None);
        assert_eq!(normalize_tx_query("   "), None);
        assert_eq!(normalize_tx_query("tx:"), None);
        assert_eq!(normalize_tx_query("not-hex!"), None);
        assert_eq!(normalize_tx_query(&"a".repeat(65)), None);

        assert_eq!(
            parse_tx_param_from_search(&format!("?foo=1&tx={full}&bar=2")),
            Some(full.clone())
        );
        assert_eq!(
            parse_tx_param_from_search("?tx=ABCDEF12"),
            Some("abcdef12".to_string())
        );
        assert_eq!(
            parse_tx_param_from_search("?tx=tx%3AABCDEF12"),
            Some("abcdef12".to_string())
        );
        // Shorter than 8 hex chars rejected for URL param.
        assert_eq!(parse_tx_param_from_search("?tx=abcdef1"), None);
        assert_eq!(parse_tx_param_from_search("?other=abcdef12"), None);
    }

    #[test]
    fn normalize_tx_query_and_parse_tx_param_are_utf8_safe() {
        // Multi-byte UTF-8 characters spanning byte offsets 3 and 5 must never panic.
        assert_eq!(normalize_tx_query("abécd1234"), None);
        assert_eq!(normalize_tx_query("€12345678"), None);
        assert_eq!(normalize_tx_query("🔥abcd1234"), None);
        assert_eq!(normalize_tx_query("abcdé1234"), None);
        assert_eq!(parse_tx_param_from_search("?tx=ab%C3%A9cd1234"), None);
        assert_eq!(parse_tx_param_from_search("?tx=é"), None);
        assert_eq!(parse_tx_param_from_search("?tx=abcdé1234"), None);
        assert_eq!(parse_tx_param_from_search("?tx=€12345678"), None);
        assert_eq!(parse_tx_param_from_search("?tx=🔥abcd1234"), None);
    }

    #[test]
    fn apply_deep_details_preserves_verified_zero_sat_fee() {
        let mut pinned =
            pinned_from_compact(&CompactTx::pending(txid(0x55), 100), true);
        pinned.deep_loading = true;
        let raw = RawTxDetailResponse {
            inputs: Some(1),
            outputs: Some(1),
            size: Some(110),
            vsize: Some(110),
            weight: Some(440),
            fee: Some(0),
            total_output: Some(0),
            confirmed: Some(false),
            confirmations: Some(0),
            block_hash: None,
            block_height: None,
            input_items: vec![TxIoEntry {
                label: "bc1qtestin".to_string(),
                value: Some(1_000),
                copy: Some("bc1qtestin".to_string()),
            }],
            output_items: vec![TxIoEntry {
                label: "Anchor".to_string(),
                value: Some(0),
                copy: None,
            }],
            features: vec!["Taproot".to_string(), "RBF".to_string()],
        };
        apply_deep_details(&mut pinned, &raw);
        assert!(!pinned.deep_loading);
        assert_eq!(pinned.fee, Some(0));
        assert_eq!(pinned.vsize, Some(110));
        assert_eq!(pinned.value, Some(0));
        assert_eq!(pinned.fee_rate_sat_per_vb(), Some(0.0));
        assert_eq!(format_fee_rate(pinned.fee_rate_sat_per_vb()), "0.0 sat/vB");
        let deep = pinned.deep.as_ref().expect("deep details populated");
        assert_eq!(deep.input_items.len(), 1);
        assert_eq!(deep.output_items[0].label, "Anchor");
        assert_eq!(deep.features, vec!["Taproot", "RBF"]);
    }

    #[test]
    fn arrival_rate_tracker_warms_up_windows_and_decays() {
        let mut tracker = ArrivalRateTracker::default();
        assert_eq!(tracker.rate_tx_per_sec(1_000.0), None);

        tracker.reset(10_000.0);
        // Under 3s warmup -> None (avoids flashing 0.0 tx/s at boot).
        assert_eq!(tracker.rate_tx_per_sec(12_500.0), None);

        // 10 arrivals over first 5s -> 2.0 tx/s.
        for i in 0..10 {
            tracker.record(10_000.0 + (i as f64) * 500.0);
        }
        let r5 = tracker.rate_tx_per_sec(15_000.0).unwrap();
        assert!((r5 - 2.0).abs() < 1e-6, "expected 2.0 tx/s, got {r5}");

        // After 60s full window with 120 arrivals -> 2.0 tx/s.
        let mut steady = ArrivalRateTracker::default();
        steady.reset(0.0);
        for i in 0..120 {
            steady.record((i as f64) * 500.0);
        }
        let r60 = steady.rate_tx_per_sec(60_000.0).unwrap();
        assert!((r60 - 2.0).abs() < 1e-6, "expected 2.0 tx/s, got {r60}");

        // 65s after last sample -> all pruned -> decays cleanly to 0.0 tx/s.
        let r_quiet = steady.rate_tx_per_sec(130_000.0).unwrap();
        assert_eq!(r_quiet, 0.0);
    }

    #[test]
    fn connection_state_for_mirror_respects_precedence() {
        let complete = Coverage::Complete;
        let partial = Coverage::Partial {
            reason: "hydrating".to_string(),
        };
        let unknown = Coverage::Unknown {
            reason: "starting".to_string(),
        };

        assert_eq!(
            connection_state_for_mirror(true, true, true, &complete),
            "disconnected"
        );
        assert_eq!(
            connection_state_for_mirror(true, false, true, &complete),
            "syncing"
        );
        assert_eq!(
            connection_state_for_mirror(false, false, false, &complete),
            "disconnected"
        );
        assert_eq!(
            connection_state_for_mirror(true, false, false, &partial),
            "partial"
        );
        assert_eq!(
            connection_state_for_mirror(true, false, false, &unknown),
            "partial"
        );
        assert_eq!(
            connection_state_for_mirror(true, false, false, &complete),
            "live"
        );
    }

    #[test]
    fn build_canvas_snapshot_json_emits_expected_wire_shape() {
        let mut mirror = HeartbeatMirror::default();
        let tx = CompactTx {
            txid: txid(0x11),
            first_seen: 1_700_000_001,
            fee: Some(500),
            vsize: Some(200),
            value: Some(100_000),
            flags: TxFlags::NONE,
            op_return_text: None,
            observed_order: 0,
        };
        let confirmed_tx = txid(0x22);
        let blk_hash = txid(0xbb);
        let mut snap = base_snapshot(42, vec![tx]);
        snap.recent_confirmations.push(ConfirmedBlock {
            hash: blk_hash.clone(),
            height: 900_000,
            txids: vec![confirmed_tx.clone()],
        });
        mirror.install_snapshot(snap).unwrap();

        let json = build_canvas_snapshot_json(&mirror);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["transactions"].as_array().unwrap().len(), 1);
        let tuple = parsed["transactions"][0].as_array().unwrap();
        assert_eq!(tuple[0].as_str().unwrap(), txid(0x11));
        assert_eq!(tuple[1].as_u64().unwrap(), 1_700_000_001);
        assert_eq!(tuple[2].as_u64().unwrap(), 500);
        assert_eq!(tuple[3].as_u64().unwrap(), 200);
        assert_eq!(tuple[4].as_u64().unwrap(), 100_000);
        assert_eq!(parsed["recent_blocks"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["pending_blocks"].as_array().unwrap().len(), 0);
        let confs = parsed["recent_confirmations"].as_array().unwrap();
        assert_eq!(confs.len(), 1);
        assert_eq!(confs[0]["hash"].as_str().unwrap(), blk_hash);
        assert_eq!(confs[0]["height"].as_u64().unwrap(), 900_000);
        assert_eq!(
            confs[0]["txids"].as_array().unwrap()[0].as_str().unwrap(),
            confirmed_tx
        );
        assert_eq!(parsed["coverage"]["status"].as_str().unwrap(), "complete");
    }

    #[test]
    fn retry_delay_exponential_backoff_caps_at_15s() {
        assert_eq!(retry_delay_ms(0), 2_000);
        assert_eq!(retry_delay_ms(1), 3_000);
        assert_eq!(retry_delay_ms(2), 4_500);
        assert_eq!(retry_delay_ms(5), 15_000);
        assert_eq!(retry_delay_ms(8), 15_000);
        assert_eq!(retry_delay_ms(20), 15_000);
    }

    #[test]
    fn parse_search_query_classifies_heights_hashes_txids_and_ambiguous_hex() {
        let hex64 = txid(0xab);
        let blk_hash = format!("00000000{}", "cd".repeat(28));

        // Heights: bare, commas, `#`, `block:`
        assert_eq!(parse_search_query("0"), Some(SearchQuery::BlockHeight(0)));
        assert_eq!(
            parse_search_query("840000"),
            Some(SearchQuery::BlockHeight(840_000))
        );
        assert_eq!(
            parse_search_query("  840,000  "),
            Some(SearchQuery::BlockHeight(840_000))
        );
        assert_eq!(
            parse_search_query("#840000"),
            Some(SearchQuery::BlockHeight(840_000))
        );
        assert_eq!(
            parse_search_query("# 840,000"),
            Some(SearchQuery::BlockHeight(840_000))
        );
        assert_eq!(
            parse_search_query("block:840000"),
            Some(SearchQuery::BlockHeight(840_000))
        );
        assert_eq!(
            parse_search_query("BLOCK: 1,000,000"),
            Some(SearchQuery::BlockHeight(1_000_000))
        );
        assert_eq!(
            parse_search_query("10,000,000"),
            Some(SearchQuery::BlockHeight(10_000_000))
        );

        // Explicit block:<64-hex> and #<64-hex>
        assert_eq!(
            parse_search_query(&format!("block:{}", blk_hash.to_uppercase())),
            Some(SearchQuery::BlockHash(blk_hash.clone()))
        );
        assert_eq!(
            parse_search_query(&format!("#{blk_hash}")),
            Some(SearchQuery::BlockHash(blk_hash.clone()))
        );

        // Explicit tx: prefix (1..=64 hex)
        assert_eq!(
            parse_search_query("tx:AbCd1234"),
            Some(SearchQuery::Tx("abcd1234".to_string()))
        );
        assert_eq!(
            parse_search_query("TX:beef"),
            Some(SearchQuery::Tx("beef".to_string()))
        );
        assert_eq!(
            parse_search_query(&format!("tx:{}", hex64.to_uppercase())),
            Some(SearchQuery::Tx(hex64.clone()))
        );

        // Bare 8..=63 hex -> Tx (including 8 numeric digits > 10_000_000 or leading zeros)
        assert_eq!(
            parse_search_query("deadbeef"),
            Some(SearchQuery::Tx("deadbeef".to_string()))
        );
        assert_eq!(
            parse_search_query("00001234"),
            Some(SearchQuery::Tx("00001234".to_string()))
        );
        assert_eq!(
            parse_search_query("12345678"),
            Some(SearchQuery::Tx("12345678".to_string()))
        );

        // Bare 64-char hex -> AmbiguousHex64
        assert_eq!(
            parse_search_query(&hex64.to_uppercase()),
            Some(SearchQuery::AmbiguousHex64(hex64.clone()))
        );
        assert_eq!(
            parse_search_query(&blk_hash),
            Some(SearchQuery::AmbiguousHex64(blk_hash))
        );

        // Invalid inputs
        assert_eq!(parse_search_query(""), None);
        assert_eq!(parse_search_query("   "), None);
        assert_eq!(parse_search_query("abc"), None);
        assert_eq!(parse_search_query("tx:"), None);
        assert_eq!(parse_search_query("block:"), None);
        assert_eq!(parse_search_query("#"), None);
        assert_eq!(parse_search_query("block:deadbeef"), None);
        assert_eq!(parse_search_query("block:not-a-block"), None);
        assert_eq!(parse_search_query("tx:zzzzzzzz"), None);
        assert_eq!(parse_search_query(&"a".repeat(65)), None);

        // Digit cap (1..=10) and thousands-comma validation.
        assert_eq!(
            parse_search_query("block:1,234,567,890"),
            Some(SearchQuery::BlockHeight(1_234_567_890))
        );
        assert_eq!(parse_search_query("block:12345678901"), None);
        assert_eq!(parse_search_query("#99999999999999999999"), None);
        assert_eq!(parse_search_query("8,4,0000"), None);
        assert_eq!(parse_search_query("block:8,4,0000"), None);
        assert_eq!(parse_search_query("#8,4,0000"), None);
        assert_eq!(parse_search_query(",840,000"), None);
        assert_eq!(parse_search_query("840,000,"), None);
        assert_eq!(parse_search_query("0,000"), None);
        assert_eq!(parse_search_query("1234,567"), None);
    }

    #[test]
    fn ambiguous_hex_not_found_preserves_partial_and_unavailable_hints() {
        let complete = Coverage::Complete;
        let partial = Coverage::Partial {
            reason: "hydrating".to_string(),
        };
        assert_eq!(
            ambiguous_hex_not_found_message(Some(&complete), false),
            "Not found"
        );
        assert!(ambiguous_hex_not_found_message(Some(&partial), false)
            .contains("partial"));
        assert!(ambiguous_hex_not_found_message(Some(&partial), true)
            .contains("partial"));
        assert!(ambiguous_hex_not_found_message(None, false)
            .contains("unavailable"));
        assert!(ambiguous_hex_not_found_message(Some(&complete), true)
            .contains("unavailable"));
    }

    #[test]
    fn parse_search_query_is_utf8_safe() {
        // Multi-byte UTF-8 characters spanning byte offsets 1, 3, and 6 must never panic.
        assert_eq!(parse_search_query("abécd1234"), None);
        assert_eq!(parse_search_query("€12345678"), None);
        assert_eq!(parse_search_query("🔥840000"), None);
        assert_eq!(parse_search_query("txé:abcd1234"), None);
        assert_eq!(parse_search_query("blocké:840000"), None);
        assert_eq!(parse_search_query("#é840000"), None);
        assert_eq!(parse_search_query("ééé"), None);
    }

    #[test]
    fn parse_initial_search_param_handles_block_and_tx_params() {
        let full_tx = txid(0xde);
        let blk_hash = format!("00000000{}", "ef".repeat(28));

        assert_eq!(
            parse_initial_search_param("?block=840000"),
            Some(InitialSearchParam::Block("840000".to_string()))
        );
        assert_eq!(
            parse_initial_search_param("?block=%23840000"),
            Some(InitialSearchParam::Block("840000".to_string()))
        );
        assert_eq!(
            parse_initial_search_param("?block=block%3A840%2C000"),
            Some(InitialSearchParam::Block("840000".to_string()))
        );
        assert_eq!(
            parse_initial_search_param(&format!(
                "?foo=1&block={}&bar=2",
                blk_hash.to_uppercase()
            )),
            Some(InitialSearchParam::Block(blk_hash))
        );
        assert_eq!(
            parse_initial_search_param(&format!("?tx={full_tx}")),
            Some(InitialSearchParam::Tx(full_tx))
        );
        assert_eq!(
            parse_initial_search_param("?tx=tx%3AABCDEF12"),
            Some(InitialSearchParam::Tx("abcdef12".to_string()))
        );
        // Invalid earlier param does not hide a valid later param.
        assert_eq!(
            parse_initial_search_param("?tx=%FF&block=840000"),
            Some(InitialSearchParam::Block("840000".to_string()))
        );
        // Invalid or short params rejected safely, including multi-byte UTF-8.
        assert_eq!(parse_initial_search_param("?tx=abcdef1"), None);
        assert_eq!(parse_initial_search_param("?block=notablock"), None);
        assert_eq!(parse_initial_search_param("?block=🔥840000"), None);
        assert_eq!(parse_initial_search_param("?block=%C3%A9"), None);
        assert_eq!(parse_initial_search_param("?tx=🔥abcd1234"), None);
        assert_eq!(parse_initial_search_param("?other=840000"), None);
    }

    #[test]
    fn history_mode_info_deserializes_with_extra_fields() {
        let json = r#"{
            "target_height": 840000,
            "target_hash": "0000000000000000000320283a032748cef8bc278972eb12977ed62928c4569f",
            "from_height": 839980,
            "to_height": 840020,
            "blocks": []
        }"#;
        let info: HistoryModeInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.target_height, 840_000);
        assert_eq!(
            info.target_hash,
            "0000000000000000000320283a032748cef8bc278972eb12977ed62928c4569f"
        );
        assert_eq!(info.from_height, 839_980);
        assert_eq!(info.to_height, 840_020);

        // Also handles `/api/stats/heartbeat/block/{query}` response without explicit from/to.
        let api_json = r#"{
            "target_height": 840000,
            "target_hash": "0000000000000000000320283a032748cef8bc278972eb12977ed62928c4569f",
            "tip_height": 976000,
            "blocks": [
                {"height": 839980},
                {"height": 840000},
                {"height": 840020}
            ]
        }"#;
        let parsed = parse_history_mode_info(api_json).unwrap();
        assert_eq!(parsed, info);
    }
}
