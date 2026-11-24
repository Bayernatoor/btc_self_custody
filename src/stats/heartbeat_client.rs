//! Heartbeat v2 browser client state, pure view helpers, and hydrate sync loop.
//!
//! Pure types and formatting/reconciliation helpers compile on both `ssr` and
//! `hydrate` targets so they are directly unit-tested under `cargo test --features ssr`.
//! The browser network loop (`HeartbeatClient`) compiles only under `hydrate`.

use serde::{Deserialize, Serialize};

use super::heartbeat_sync::{
    ApplyEffect, CanonicalBlock, CompactTx, ConfirmedBlock, Coverage,
    HeartbeatMirror, PendingBlock, TxFlags,
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

/// Deep node details fetched asynchronously from `/api/stats/tx/{txid}`.
#[derive(Clone, Debug, PartialEq)]
pub struct TxDeepDetails {
    pub inputs: Option<u64>,
    pub outputs: Option<u64>,
    pub size: Option<u64>,
    pub weight: Option<u64>,
    pub confirmations: Option<u64>,
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
}

impl PinnedTxView {
    /// Fee rate in sat/vB when both fee and a non-zero vsize are known.
    pub fn fee_rate_sat_per_vb(&self) -> Option<f64> {
        let fee = self.fee?;
        let vsize = self.vsize?;
        (vsize > 0).then(|| fee as f64 / f64::from(vsize))
    }
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
    if let Some(tx) = mirror.get_tx(&pinned.txid) {
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
        return;
    }

    if let Some((block_hash, block_height)) = mirror.confirmed_in(&pinned.txid)
    {
        pinned.status = PinnedTxStatus::Confirmed {
            block_hash: block_hash.to_string(),
            block_height,
        };
        pinned.on_canvas = false;
        return;
    }

    if matches!(pinned.status, PinnedTxStatus::Unconfirmed) {
        pinned.status = PinnedTxStatus::Removed;
        pinned.on_canvas = false;
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

        // Pass JS Number (`f64`), not `u64` (BigInt), because JS compares
        // `seg.height === height` with strict equality against Number heights.
        #[wasm_bindgen(js_name = hbV2FocusBlockHeight, catch)]
        fn js_hb_v2_focus_block_height(height: f64) -> Result<bool, JsValue>;

        #[wasm_bindgen(js_name = hbV2SetConnectionState, catch)]
        fn js_hb_v2_set_connection_state(state: &str) -> Result<(), JsValue>;

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

    pub fn hb_v2_focus_block_height(height: u64) -> bool {
        js_hb_v2_focus_block_height(height as f64).unwrap_or(false)
    }

    pub fn hb_v2_set_connection_state(state: &str) {
        let _ = js_hb_v2_set_connection_state(state);
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
    pub fn hb_v2_focus_block_height(_height: u64) -> bool {
        false
    }
    pub fn hb_v2_set_connection_state(_state: &str) {}
}

pub use js_bridge::{
    hb_v2_apply_effect, hb_v2_focus_block_height, hb_v2_focus_tx,
    hb_v2_install_snapshot, hb_v2_set_connection_state, hb_v2_set_pinned_tx,
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
        apply_block_disconnected_to_pinned, apply_deep_details,
        build_canvas_effect_json, build_canvas_snapshot_json,
        connection_state_for_mirror, format_u64_commas, hb_v2_apply_effect,
        hb_v2_focus_tx, hb_v2_install_snapshot, hb_v2_set_connection_state,
        hb_v2_set_pinned_tx, js_bridge, normalize_tx_query,
        pinned_from_compact, pinned_from_confirmed, retry_delay_ms,
        update_pinned_from_mirror, PinnedTxStatus, PinnedTxView,
        RawTxDetailResponse,
    };
    use crate::stats::heartbeat_sync::{
        ApplyEffect, ApplyOutcome, Coverage, HeartbeatDelta, HeartbeatMirror,
        HeartbeatSnapshot, MirrorTxMatch, TxFlags, TxLookupStatus,
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
    }

    type EventListenerEntry = (&'static str, Closure<dyn FnMut(Event)>);

    #[derive(Default)]
    struct ClientState {
        mirror: HeartbeatMirror,
        event_source: Option<EventSource>,
        es_listeners: Vec<EventListenerEntry>,
        snapshot_abort: Option<AbortController>,
        lookup_abort: Option<AbortController>,
        retry_timer_id: Option<i32>,
        retry_closure: Option<Closure<dyn FnMut()>>,
        retry_count: u32,
        connected: bool,
        resume_attempted: bool,
        initial_tx_param: Option<String>,
        visibility_closure: Option<Closure<dyn FnMut(Event)>>,
        select_tx_closure: Option<Closure<dyn FnMut(String)>>,
        clear_pinned_closure: Option<Closure<dyn FnMut()>>,
        destroyed: bool,
    }

    struct ClientInner {
        /// Incremented on `destroy()` to invalidate every callback from this mount.
        generation: Cell<u64>,
        /// Incremented on every snapshot/SSE sync cycle and on `destroy()`.
        cycle_gen: Cell<u64>,
        /// Incremented on every tx search/select/clear and on `destroy()`.
        lookup_gen: Cell<u64>,
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

        /// Start the v2 snapshot + SSE sync loop and attach window/document listeners.
        pub fn start(&self, initial_tx_param: Option<String>) {
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

            // Register canvas/feed -> WASM selection bridge on window.
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
            }

            {
                let mut st = self.inner.state.borrow_mut();
                st.initial_tx_param = initial_tx_param;
                st.visibility_closure = Some(vis_cb);
                st.select_tx_closure = Some(select_cb);
                st.clear_pinned_closure = Some(clear_cb);
            }

            self.fetch_and_connect(false);
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

            let (
                es,
                es_listeners,
                snap_abort,
                lookup_abort,
                timer_id,
                _retry_cb,
                vis_cb,
                _sel_cb,
                _clr_cb,
            ) = {
                let mut st = self.inner.state.borrow_mut();
                st.destroyed = true;
                st.connected = false;
                st.mirror = HeartbeatMirror::default();
                (
                    st.event_source.take(),
                    std::mem::take(&mut st.es_listeners),
                    st.snapshot_abort.take(),
                    st.lookup_abort.take(),
                    st.retry_timer_id.take(),
                    st.retry_closure.take(),
                    st.visibility_closure.take(),
                    st.select_tx_closure.take(),
                    st.clear_pinned_closure.take(),
                )
            };

            teardown_event_source(es, es_listeners);
            if let Some(ac) = snap_abort {
                ac.abort();
            }
            if let Some(ac) = lookup_abort {
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
                let updated_pinned =
                    existing_pinned.as_ref().and_then(|existing| {
                        let mut pv = existing.clone();
                        update_pinned_from_mirror(&mut pv, &st.mirror);
                        (&pv != existing).then_some(pv)
                    });
                let initial_tx = if !is_resync {
                    st.initial_tx_param.take()
                } else {
                    None
                };
                (
                    epoch,
                    seq,
                    snapshot_json,
                    coverage_note,
                    newest_block,
                    updated_pinned,
                    initial_tx,
                )
            };

            let (
                epoch,
                seq,
                snapshot_json,
                coverage_note,
                newest_block,
                updated_pinned,
                initial_tx,
            ) = extracted;

            let _ = self.inner.signals.set_coverage_note.try_set(coverage_note);

            if let Some(pv) = updated_pinned {
                let _ = self.inner.signals.set_pinned_tx.try_set(Some(pv));
            }

            if let Some((height, ts)) = newest_block {
                self.advance_header_block(height, ts);
            }

            hb_v2_install_snapshot(&snapshot_json, is_resync);
            let _ = self.inner.signals.set_loading.try_set(false);

            if let Some(tx_param) = initial_tx {
                self.search_tx(&tx_param);
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
                coverage_update: Option<(&'static str, Option<String>)>,
                updated_pinned: Option<PinnedTxView>,
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
                        let updated_pinned =
                            existing_pinned.as_ref().and_then(|existing| {
                                let mut pv = existing.clone();
                                if let Some(dhash) = disconnected_hash {
                                    apply_block_disconnected_to_pinned(
                                        &mut pv, dhash,
                                    );
                                }
                                update_pinned_from_mirror(&mut pv, &st.mirror);
                                (&pv != existing).then_some(pv)
                            });
                        Step::Apply(Box::new(AppliedStep {
                            effect_json,
                            block_update,
                            coverage_update,
                            updated_pinned,
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
                        coverage_update,
                        updated_pinned,
                    } = *applied;
                    hb_v2_apply_effect(&effect_json);
                    if let Some((height, ts)) = block_update {
                        self.advance_header_block(height, ts);
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

        /// Search for a transaction by full txid or >= 8 hex character prefix.
        pub fn search_tx(&self, raw_query: &str) {
            let Some(needle) = normalize_tx_query(raw_query) else {
                let _ = self.inner.signals.set_search_feedback.try_set(Some((
                    true,
                    "Enter at least 8 hex characters of a txid".to_string(),
                )));
                return;
            };
            if needle.len() < 8 {
                let _ = self.inner.signals.set_search_feedback.try_set(Some((
                    true,
                    "Enter at least 8 hex characters of a txid".to_string(),
                )));
                return;
            }

            let (mount_gen, lookup_gen) = self.begin_lookup_generation();

            // Step 1: instant local mirror search.
            let local_match = {
                let st = self.inner.state.borrow();
                st.mirror.search_tx(&needle)
            };

            if let Some(m) = local_match {
                match m {
                    MirrorTxMatch::Unconfirmed(tx) => {
                        let on_canvas = hb_v2_focus_tx(&tx.txid);
                        if !on_canvas {
                            hb_v2_set_pinned_tx(Some(&tx.txid));
                        }
                        let mut pv = pinned_from_compact(&tx, on_canvas);
                        pv.deep_loading = true;
                        let txid = tx.txid.clone();
                        let _ =
                            self.inner.signals.set_pinned_tx.try_set(Some(pv));
                        let msg = if on_canvas {
                            "Found in mempool".to_string()
                        } else {
                            "In node mempool (outside rendered window)"
                                .to_string()
                        };
                        let _ = self
                            .inner
                            .signals
                            .set_search_feedback
                            .try_set(Some((false, msg)));
                        self.fetch_deep_tx_details(
                            &txid, mount_gen, lookup_gen,
                        );
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
                        let _ =
                            self.inner.signals.set_pinned_tx.try_set(Some(pv));
                        let _ = self.inner.signals.set_search_feedback.try_set(
                            Some((
                                false,
                                format!(
                                    "Confirmed in block #{}",
                                    format_u64_commas(block_height)
                                ),
                            )),
                        );
                        self.fetch_deep_tx_details(
                            &txid, mount_gen, lookup_gen,
                        );
                    }
                }
                return;
            }

            // Step 2: server fallback for full 64-char txids.
            if needle.len() < 64 {
                let _ = self.inner.signals.set_search_feedback.try_set(Some((
                    true,
                    "Not found in live mempool (paste full 64-char txid to search confirmed blocks)"
                        .to_string(),
                )));
                return;
            }

            let _ = self
                .inner
                .signals
                .set_search_feedback
                .try_set(Some((false, "Searching node...".to_string())));
            self.fetch_server_tx_lookup(&needle, true, mount_gen, lookup_gen);
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
                            &txid, false, mount_gen, lookup_gen,
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
                            };
                            apply_deep_details(&mut pv, &raw);
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
        });
        assert_eq!(pinned.status, PinnedTxStatus::Unconfirmed);
        assert_eq!(pinned.fee, None);

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

        // 3. BlockObserved + BlockInclusion confirms tx in place, preserving fee/vsize/value.
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
        };
        apply_deep_details(&mut pinned, &raw);
        assert!(!pinned.deep_loading);
        assert_eq!(pinned.fee, Some(0));
        assert_eq!(pinned.vsize, Some(110));
        assert_eq!(pinned.value, Some(0));
        assert_eq!(pinned.fee_rate_sat_per_vb(), Some(0.0));
        assert_eq!(format_fee_rate(pinned.fee_rate_sat_per_vb()), "0.0 sat/vB");
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
}
