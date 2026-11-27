//! Block Heartbeat - live EKG visualization of Bitcoin block arrivals.
//!
//! Each block produces a PQRST waveform spike on a canvas sweep line,
//! like a hospital cardiac monitor. The flatline between beats is the
//! real wait for the next block. Color shifts with network stress.
//!
//! Architecture:
//! - The heavy animation logic lives in `/js/heartbeat.js` (canvas rendering,
//!   waveform generation, sweep line, glow effects).
//! - This Rust module handles: JS interop via `wasm_bindgen`, initial data
//!   loading (last 2016 blocks), live block detection from `cached_live`,
//!   vital signs display (heart rate, blood pressure, temperature, immune
//!   system), organism status, rhythm strip, and all HTML controls.
//! - SSR stubs are provided for all JS functions so the server can render
//!   the page skeleton without WASM.
//! - Constants: `RETARGET_PERIOD` (2016 blocks), `BRADYCARDIA_THRESHOLD`
//!   (0.7x target rate), `TACHYCARDIA_THRESHOLD` (1.3x target rate).

use leptos::portal::Portal;
use leptos::prelude::*;
use leptos_meta::*;

#[cfg(feature = "hydrate")]
use wasm_bindgen::prelude::*;

use crate::stats::hall_of_fame_data::HALL_OF_FAME;
use crate::stats::heartbeat_client::{
    fee_band_for_rate, format_fee_rate, hb_v2_exit_history_mode,
    hb_v2_focus_block_height, hb_v2_focus_pending_slot, hb_v2_focus_tx,
    hb_v2_is_pending_slot_offscreen, hb_v2_set_pinned_tx,
    hb_v2_set_spotlight_band, primary_notable_label, HistoryModeInfo,
    PendingBlockBanner, PinnedTxStatus, PinnedTxView,
};
#[cfg(feature = "hydrate")]
use crate::stats::heartbeat_client::{
    parse_initial_search_param, HeartbeatClient, HeartbeatClientSignals,
    InitialSearchParam,
};
use crate::stats::heartbeat_sync::{
    FeeBandStat, FeeBandSummary, QueuePosition,
};
use crate::stats::types::HofCategory;

// ---------------------------------------------------------------------------
// JS interop: heartbeat.js functions
// ---------------------------------------------------------------------------

#[cfg(feature = "hydrate")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = initHeartbeat)]
    fn init_heartbeat(canvas_id: &str);

    #[wasm_bindgen(js_name = pushHeartbeatBlocks)]
    fn push_heartbeat_blocks(json: &str, replay: bool);

    #[wasm_bindgen(js_name = updateHeartbeatLive)]
    fn update_heartbeat_live(json: &str);

    #[wasm_bindgen(js_name = setHeartbeatLandmarks)]
    fn set_heartbeat_landmarks(json: &str);

    #[wasm_bindgen(js_name = destroyHeartbeat)]
    fn destroy_heartbeat();

    // Phase 2: Vital signs
    #[wasm_bindgen(js_name = getHeartbeatVitals)]
    fn get_heartbeat_vitals() -> String;

    // Phase 3: Rhythm strip
    #[wasm_bindgen(js_name = renderRhythmStrip)]
    fn render_rhythm_strip(canvas_id: &str, blocks_json: &str);

    // Capture
    #[wasm_bindgen(js_name = heartbeatDownloadCapture)]
    fn heartbeat_download_capture(vitals_json: &str);

    // Freshest block on the SSE/ZMQ-driven timeline (JSON {height, timestamp})
    #[wasm_bindgen(js_name = getHeartbeatLatestBlock)]
    fn get_heartbeat_latest_block() -> String;

    // SSE connection state: "live" | "stale" | "disconnected"
    #[wasm_bindgen(js_name = getHeartbeatConnectionState)]
    fn get_heartbeat_connection_state() -> String;

    // Block detail modal defined in stats.js (pass JS Number via f64).
    #[wasm_bindgen(js_name = showBlockDetail, catch)]
    fn js_show_block_detail(height: f64) -> Result<(), JsValue>;
}

#[cfg(feature = "hydrate")]
fn show_block_detail_js(height: u64) {
    let _ = js_show_block_detail(height as f64);
}

#[cfg(not(feature = "hydrate"))]
fn show_block_detail_js(_height: u64) {}

#[cfg(not(feature = "hydrate"))]
fn init_heartbeat(_: &str) {}
#[cfg(not(feature = "hydrate"))]
fn push_heartbeat_blocks(_: &str, _: bool) {}
#[cfg(not(feature = "hydrate"))]
fn update_heartbeat_live(_: &str) {}
#[cfg(not(feature = "hydrate"))]
fn set_heartbeat_landmarks(_: &str) {}
#[cfg(not(feature = "hydrate"))]
fn destroy_heartbeat() {}
#[cfg(not(feature = "hydrate"))]
fn get_heartbeat_vitals() -> String {
    "{}".to_string()
}
#[cfg(not(feature = "hydrate"))]
fn render_rhythm_strip(_: &str, _: &str) {}
#[cfg(not(feature = "hydrate"))]
#[allow(dead_code)] // used only by the commented-out Capture Moment button (kept for later)
fn heartbeat_download_capture(_: &str) {}
#[cfg(not(feature = "hydrate"))]
#[allow(dead_code)]
fn get_heartbeat_latest_block() -> String {
    "{}".to_string()
}
#[cfg(not(feature = "hydrate"))]
#[allow(dead_code)]
fn get_heartbeat_connection_state() -> String {
    "live".to_string()
}

const RETARGET_PERIOD: u64 = 2016;
const BRADYCARDIA_THRESHOLD: f64 = 0.7;
const TACHYCARDIA_THRESHOLD: f64 = 1.3;

// ---------------------------------------------------------------------------
// Heartbeat page component
// ---------------------------------------------------------------------------

/// Block Heartbeat page. Initializes the JS canvas animation, feeds it block data,
/// and renders vital signs, organism status, rhythm strip, and controls.
#[component]
pub fn HeartbeatPage() -> impl IntoView {
    let state = expect_context::<super::shared::ObservatoryState>();
    let cached_live = state.cached_live;

    // Track last-seen block height to detect new blocks
    let last_height = std::rc::Rc::new(std::cell::Cell::new(0u64));
    let initialized = std::rc::Rc::new(std::cell::Cell::new(false));
    let (loading, set_loading) = signal(true);

    // On-demand guide & shortcuts overlay opened by the (i) button in the
    // control dock. Starts hidden so first-time visitors see the opening
    // mempool animation unobstructed; rendered directly in SSR HTML (via CSS
    // `hidden` rather than `<Show>`) so its click-to-dismiss handler attaches
    // reliably at hydration.
    let (show_hint, set_show_hint) = signal(false);

    // Heartbeat v2 sync + transaction inspection signals
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (sse_height, set_sse_height) = signal(0u64);
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (sse_ts, set_sse_ts) = signal(0u64);
    // SSE connection state for the status badge:
    // "live" | "syncing" | "partial" | "stale" | "disconnected".
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (conn_state, set_conn_state) = signal("live".to_string());
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (coverage_note, set_coverage_note) = signal(Option::<String>::None);
    let (pinned_tx, set_pinned_tx) = signal(Option::<PinnedTxView>::None);
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (pinned_queue, set_pinned_queue) =
        signal(Option::<QueuePosition>::None);
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (pending_block, set_pending_block) =
        signal(Option::<PendingBlockBanner>::None);
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (history_mode, set_history_mode) =
        signal(Option::<HistoryModeInfo>::None);
    let (recent_block_pairs, set_recent_block_pairs) =
        signal(Vec::<(u64, u64)>::new());
    let (search_query, set_search_query) = signal(String::new());
    let (search_feedback, set_search_feedback) =
        signal(Option::<(bool, String)>::None);
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (copied_txid, set_copied_txid) = signal(false);
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (copied_history_link, set_copied_history_link) = signal(false);
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (copied_tx_link, set_copied_tx_link) = signal(false);
    let (show_landmarks, set_show_landmarks) = signal(false);
    let (landmark_cat, set_landmark_cat) = signal(Option::<HofCategory>::None);
    let (landmark_filter, set_landmark_filter) = signal(String::new());
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (fee_bands, set_fee_bands) = signal(FeeBandSummary::default());
    let (spotlight_band, set_spotlight_band) = signal(Option::<usize>::None);
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (arrival_rate, set_arrival_rate) = signal(Option::<f64>::None);
    let (io_expanded, set_io_expanded) = signal(false);
    let (copied_io, set_copied_io) = signal(Option::<String>::None);
    let (last_io_txid, set_last_io_txid) = signal(Option::<String>::None);
    Effect::new(move || {
        let cur = pinned_tx.get().map(|t| t.txid);
        if cur != last_io_txid.get_untracked() {
            set_last_io_txid.set(cur);
            set_io_expanded.set(false);
            set_copied_io.set(None);
        }
    });
    let landmark_total = heartbeat_landmarks().len();

    #[cfg(feature = "hydrate")]
    let hb_client = StoredValue::new_local(Option::<HeartbeatClient>::None);

    // Signals for vital signs display
    let (hr_display, set_hr_display) = signal("--:--".to_string());
    let (hr_label, set_hr_label) = signal("Waiting...".to_string());
    let (hr_color, set_hr_color) = signal("#00e676".to_string());
    let (hr_subtitle, set_hr_subtitle) = signal(String::new());

    let (bp_display, set_bp_display) = signal("-- / --".to_string());
    let (bp_label, set_bp_label) = signal("Waiting...".to_string());
    let (bp_color, set_bp_color) = signal("#00e676".to_string());

    // Single source of truth for node-aggregate mempool totals, shared by the
    // bottom info bar and the Temperature tile so they can never disagree.
    let mempool_summary = Signal::derive(move || {
        format_mempool_summary(cached_live.get().as_ref())
    });
    let mempool_display = Signal::derive(move || mempool_summary.get().bar);
    let temp_display = Signal::derive(move || mempool_summary.get().tile_vmb);
    let temp_label =
        Signal::derive(move || mempool_summary.get().tile_tx_label);
    let (temp_color, set_temp_color) = signal("#00e676".to_string());
    let (temp_subtitle, set_temp_subtitle) = signal(String::new());

    let (immune_display, set_immune_display) = signal("-- EH/s".to_string());
    let (immune_label, set_immune_label) = signal("Waiting...".to_string());
    let (immune_color, set_immune_color) = signal("#00e676".to_string());

    // Period start timestamp (first block in current retarget period)
    let (period_start_ts, set_period_start_ts) = signal(0u64);

    // Blocks until next difficulty adjustment
    let blocks_until_retarget = Signal::derive(move || {
        cached_live
            .get()
            .map(|s| {
                let height = s.blockchain.blocks;
                let blocks_in_epoch = height % RETARGET_PERIOD;
                RETARGET_PERIOD - blocks_in_epoch
            })
            .unwrap_or(0)
    });

    // Fetch blocks for the initial timeline (current retarget period = 2016 plus 1
    // parent block so all 2,016 initial blocks carry exact parent-derived intervals).
    // DB-only (fetch_recent_blocks derives the tip from db::max_height, no node
    // RPC), and NOT gated on cached_live, so the historical EKG renders
    // immediately on load even when the home node is unreachable. The live parts
    // (SSE txs, vitals, STALE indicator) fill in separately once/if the node is up.
    let initial_blocks = LocalResource::new(move || async move {
        crate::stats::server_fns::fetch_recent_blocks(RETARGET_PERIOD + 1)
            .await
            .unwrap_or_default()
    });

    // Initialize canvas, push initial blocks, and start the v2 mirror client
    let init_initialized = initialized.clone();
    let init_last_height = last_height.clone();
    Effect::new(move || {
        let initialized = &init_initialized;
        let last_height = &init_last_height;
        if initialized.get() {
            return;
        }
        if let Some(blocks) = initial_blocks.get() {
            let blocks = blocks.clone();

            // Always init the canvas and connect the v2 stream once the resource
            // resolves, EVEN IF the DB returned no blocks (empty DB, or a
            // transient DB pool/query error that unwrap_or_default() collapsed to
            // []). The live feed, vitals and STALE indicator must work regardless
            // of DB state, and the historical EKG simply starts empty and fills
            // from the snapshot + delta stream.
            init_heartbeat("heartbeat-canvas");
            set_heartbeat_landmarks(&landmarks_to_json());
            initialized.set(true);
            // Under hydrate, keep the loading overlay up briefly until the
            // first v2 snapshot frames the mempool (with a 2.5s fallback if the
            // node is offline) so refresh never flashes zoomed-in spikes and
            // then snaps out for the brick laydown.
            #[cfg(not(feature = "hydrate"))]
            set_loading.set(false);
            #[cfg(feature = "hydrate")]
            leptos::prelude::set_timeout(
                move || set_loading.set(false),
                std::time::Duration::from_millis(2500),
            );

            // Seed last height from LiveStats regardless; the replay path below
            // maxes it with the freshest replayed block when history is present.
            let live_height = cached_live
                .get_untracked()
                .map(|s| s.blockchain.blocks)
                .unwrap_or(0);

            if blocks.is_empty() {
                last_height.set(live_height);
            } else {
                // Store period start timestamp for heart rate calculation
                // Use the block at the retarget boundary, not the first fetched block
                {
                    let current_height =
                        blocks.last().map(|b| b.height).unwrap_or(0);
                    let period_start_height =
                        (current_height / RETARGET_PERIOD) * RETARGET_PERIOD;
                    let period_block =
                        blocks.iter().find(|b| b.height == period_start_height);
                    if let Some(pb) = period_block {
                        set_period_start_ts.set(pb.timestamp);
                    } else if let Some(first) = blocks.first() {
                        // Fallback: first block (shouldn't happen with 2016 blocks)
                        set_period_start_ts.set(first.timestamp);
                    }
                }

                // Seed recent 25 block timestamps for Rust-computed 20-block average cadence
                let tail_start = blocks.len().saturating_sub(25);
                let seeded_pairs: Vec<(u64, u64)> = blocks[tail_start..]
                    .iter()
                    .map(|b| (b.height, b.timestamp))
                    .collect();
                set_recent_block_pairs.set(seeded_pairs);

                // Build block events with exact parent-derived signed inter-block times
                let json = if blocks.len() > RETARGET_PERIOD as usize {
                    let start = blocks.len() - RETARGET_PERIOD as usize;
                    blocks_to_json_with_parent(
                        &blocks[start..],
                        Some(&blocks[start - 1]),
                    )
                } else {
                    blocks_to_json(&blocks)
                };
                push_heartbeat_blocks(&json, true);

                // Store last height: use the latest from LiveStats (not the last replayed block)
                // to avoid missing blocks that arrived between fetch and now
                let replay_height =
                    blocks.last().map(|b| b.height).unwrap_or(0);
                last_height.set(std::cmp::max(live_height, replay_height));

                // Render rhythm strip with last 144 blocks (24hr)
                let strip_json = if blocks.len() > 144 {
                    let start = blocks.len() - 144;
                    blocks_to_json_with_parent(
                        &blocks[start..],
                        Some(&blocks[start - 1]),
                    )
                } else {
                    blocks_to_json(&blocks)
                };
                render_rhythm_strip("rhythm-strip-canvas", &strip_json);
            }

            #[cfg(feature = "hydrate")]
            {
                let initial_search_param = leptos::prelude::window()
                    .location()
                    .search()
                    .ok()
                    .and_then(|s| parse_initial_search_param(&s));
                if let Some(ref param) = initial_search_param {
                    match param {
                        InitialSearchParam::Tx(q)
                        | InitialSearchParam::Block(q) => {
                            set_search_query.set(q.clone());
                        }
                    }
                }
                let client = HeartbeatClient::new(HeartbeatClientSignals {
                    set_loading,
                    set_conn_state,
                    set_coverage_note,
                    set_pinned_tx,
                    pinned_tx,
                    set_pinned_queue,
                    set_search_feedback,
                    set_sse_height,
                    sse_height,
                    set_sse_ts,
                    set_pending_block,
                    set_history_mode,
                    set_fee_bands,
                    set_arrival_rate,
                });
                hb_client.set_value(Some(client.clone()));
                client.start(initial_search_param);
            }
        }
    });

    // Helper: refresh vital signs from LiveStats (Rust) + JS canvas vitals
    let refresh_vitals = move || {
        // Blood Pressure: computed directly in Rust from Bitcoin Core's
        // estimatesmartfee(1) (next block) and estimatesmartfee(6) (~1 hour),
        // with graceful fallback to mempoolminfee when fee history is absent.
        if let Some(live) = cached_live.get_untracked() {
            let (display, label, color) = compute_blood_pressure(
                live.next_block_fee,
                live.hour_fee,
                live.mempool.mempoolminfee,
                live.mempool_available,
            );
            set_bp_display.set(display);
            set_bp_label.set(label);
            set_bp_color.set(color.to_string());
        }

        let vitals_json = get_heartbeat_vitals();
        // Parse the JSON manually (no serde dependency needed)
        if let Some(v) = parse_vitals_json(&vitals_json) {
            // Heart Rate: compute from period start timestamp stored at init
            let period_ts = period_start_ts.get();
            if let Some(live) = cached_live.get_untracked() {
                let current_ts = live.blockchain.time;
                let blocks_in = live.blockchain.blocks % RETARGET_PERIOD;
                let span = current_ts.saturating_sub(period_ts) as f64;
                if period_ts > 0 && span > 0.0 && blocks_in > 1 {
                    let avg_secs = span / blocks_in as f64;
                    let avg_u = avg_secs.round() as u64;
                    set_hr_display.set(format!(
                        "{}:{:02}",
                        avg_u / 60,
                        avg_u % 60
                    ));
                    let bpm = 600.0 / avg_secs;
                    set_hr_subtitle
                        .set(format!("{:.0}% of target rate", bpm * 100.0));
                    let (label, color) = if bpm < BRADYCARDIA_THRESHOLD {
                        ("Bradycardia", "#42a5f5")
                    } else if bpm <= TACHYCARDIA_THRESHOLD {
                        ("Normal", "#00e676")
                    } else {
                        ("Tachycardia", "#f7931a")
                    };
                    set_hr_label.set(label.to_string());
                    set_hr_color.set(color.to_string());
                }
            }

            // Temperature: mempool vMB and tx count come reactively from
            // `mempool_summary` (`format_mempool_summary`); only the derived
            // temperature subtitle and colour come from JS vitals, and only
            // when the node reported an actual mempool observation.
            if cached_live
                .get_untracked()
                .is_some_and(|l| l.mempool_available)
            {
                set_temp_subtitle.set(format!(
                    "{:.1}\u{00B0}C \u{00b7} {}",
                    v.temp_c, v.temp_label
                ));
                set_temp_color.set(v.temp_color);
            } else {
                set_temp_subtitle.set(String::new());
            }

            // Immune System: hashrate + retarget context
            set_immune_display.set(format!("{:.1} EH/s", v.immune_eh));
            set_immune_label.set(format!(
                "{} \u{00b7} Retarget in ~{} blocks",
                v.immune_label,
                blocks_until_retarget.get_untracked()
            ));
            set_immune_color.set(v.immune_color);
        }
    };

    // Keep recent block timestamps in sync as new blocks arrive (or reorg).
    Effect::new(move || {
        let h = sse_height.get();
        let t = sse_ts.get();
        if h > 0 && t > 0 {
            set_recent_block_pairs.update(|pairs| {
                record_recent_block_pair(pairs, h, t);
            });
        }
    });

    // Forward live metrics to JS for color + vital signs computation.
    // Block detection is handled entirely by SSE (heartbeat.js connects
    // to /api/stats/heartbeat which streams real block data from ZMQ).
    // LiveStats only updates the vital signs panel and network stress color.
    Effect::new(move || {
        let Some(live) = cached_live.get() else {
            return;
        };

        if live.blockchain.blocks > 0 && live.blockchain.time > 0 {
            let live_h = live.blockchain.blocks;
            let live_t = live.blockchain.time;
            set_recent_block_pairs.update(|pairs| {
                if pairs.last().is_none_or(|&(last_h, _)| live_h > last_h) {
                    record_recent_block_pair(pairs, live_h, live_t);
                }
            });
        }

        // Skip when every live RPC call failed (server returns zeroed defaults).
        // Retaining the previous JS state keeps non-mempool vitals showing
        // their last known values instead of 0s, while `mempool_summary`
        // independently transitions to the unavailable placeholder.
        let rpc_failed = !live.mempool_available
            && live.network.hashrate == 0.0
            && live.next_block_fee == 0.0;
        if rpc_failed {
            set_temp_subtitle.set(String::new());
            return;
        }

        // Forward live metrics for color + vital signs computation. Omit
        // mempool fields when `getmempoolinfo` failed so JS does not treat
        // zeroed placeholders as an empty mempool.
        let mempool_fields = if live.mempool_available {
            format!(
                r#""mempool_mb":{:.1},"mempool_min_fee":{},"#,
                live.mempool.bytes as f64 / 1_000_000.0,
                btc_per_kvb_to_sat_per_vb(live.mempool.mempoolminfee),
            )
        } else {
            String::new()
        };
        let live_json = format!(
            r#"{{"next_block_fee":{},{mempool_fields}"block_time":{},"hashrate_eh":{:.1},"difficulty":{},"block_height":{}}}"#,
            live.next_block_fee,
            live.blockchain.time,
            live.network.hashrate / 1e18,
            live.blockchain.difficulty,
            live.blockchain.blocks,
        );
        update_heartbeat_live(&live_json);

        // Refresh vitals display
        refresh_vitals();
    });

    // Cleanup v2 mirror client and canvas animation on navigate away
    on_cleanup(move || {
        #[cfg(feature = "hydrate")]
        {
            let client = hb_client.try_update_value(|c| c.take()).flatten();
            if let Some(c) = client {
                c.destroy();
            }
        }
        destroy_heartbeat();
    });

    // Reactive display values
    let block_height = Signal::derive(move || {
        let live = cached_live.get().map(|s| s.blockchain.blocks).unwrap_or(0);
        let height = live.max(sse_height.get());
        if height == 0 {
            "---".to_string()
        } else {
            format!("#{}", super::helpers::format_number(height))
        }
    });

    // Whether the data behind the page is stale, as opposed to the browser's
    // connection to us.
    //
    // **The same predicate `NodeStatusBanner` uses, deliberately.** That
    // banner renders on every observatory page including this one, and reads
    // `!connected || stale`. This badge read neither, so on 2026-10-23, when
    // the node lost its peers for several hours, the page carried an amber
    // "Home node not currently responding" banner at the top and a green
    // LIVE badge inside the card. Two indicators contradicting each other is
    // worse than one being absent, and the badge is the one inside the thing
    // a reader is watching.
    //
    // Sharing the predicate rather than re-deriving it means they cannot
    // drift apart again: `connected` goes false when the live-stats call
    // itself errors, `stale` when it succeeded but the node did not answer.
    let live_ctx = expect_context::<super::shared::LiveContext>();
    let connected = live_ctx.connected;
    let live_resource = live_ctx.live;
    let node_stale = Signal::derive(move || {
        // Suppressed until the live resource has resolved once, the same
        // guard the banner carries and for the same reason: `connected`
        // starts `false`, so without this every first paint accuses the node
        // of being down. Caught in SSR, where the badge rendered NODE STALE
        // before any fetch had happened.
        if live_resource.get().is_none() {
            return false;
        }
        !connected.get() || cached_live.get().is_some_and(|l| l.stale)
    });

    let status_color = Signal::derive(move || {
        status_for(&conn_state.get(), node_stale.get()).0
    });
    let status_label = Signal::derive(move || {
        status_for(&conn_state.get(), node_stale.get())
            .1
            .to_string()
    });
    let status_tip = Signal::derive(move || {
        let base = status_for(&conn_state.get(), node_stale.get()).2;
        if conn_state.get() == "partial" && !node_stale.get() {
            if let Some(reason) = coverage_note.get() {
                return format!("{base} ({reason})");
            }
        }
        base.to_string()
    });

    // Tick counter that increments every second for the live countdown, and
    // polls the freshest block from the SSE/ZMQ timeline into the sse_* signals.
    // Only runs in hydrate builds; suppress unused warnings for the server build.
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (tick, set_tick) = signal(0u64);

    #[cfg(feature = "hydrate")]
    {
        let handle = leptos::prelude::set_interval_with_handle(
            move || {
                set_tick.update(|t| *t += 1);
                // Poll the freshest block from the SSE/ZMQ timeline while in live
                // mode. Monotonic: only advance, never regress, so a lagging source
                // cannot flicker the header backwards (reorg retraction goes through
                // HeartbeatClient::handle_delta directly).
                if history_mode.get_untracked().is_none() {
                    let latest = get_heartbeat_latest_block();
                    let h = extract_json_f64(&latest, "height") as u64;
                    let t = extract_json_f64(&latest, "timestamp") as u64;
                    if h > sse_height.get_untracked() {
                        set_sse_height.set(h);
                        // Stamp the block's timestamp CLAMPED to our wall clock at
                        // arrival. Block timestamps are the miner's clock and often run
                        // seconds-to-minutes ahead of real time; using the raw value
                        // would freeze the "Ns ago" timer at 0 (saturating_sub) until
                        // wall-clock catches up. Clamping at arrival makes it count from
                        // 0, while a genuinely old tip (loaded from history) keeps its
                        // real age since t < now there. Tied to the height bump so it's
                        // stamped once per block, not re-clamped every tick.
                        let now = chrono::Utc::now().timestamp() as u64;
                        if t > 0 {
                            set_sse_ts.set(t.min(now));
                        }
                    }
                }
                // Only promote "live" to "stale" from the JS stall detector so
                // v2 states ("syncing", "partial", "disconnected") stay authoritative.
                let js_cs = get_heartbeat_connection_state();
                if js_cs == "stale" && conn_state.get_untracked() == "live" {
                    set_conn_state.set("stale".to_string());
                }
                if let Some(client) = hb_client.get_value() {
                    client.refresh_arrival_rate();
                }
            },
            std::time::Duration::from_secs(1),
        );
        on_cleanup(move || {
            if let Ok(h) = handle {
                h.clear();
            }
        });
    }

    // Clamp future-stamped miner header timestamps to our first observation time
    // per height so the "Last block: Ns ago" arrival timer never freezes at 0s
    // when a miner's clock is ahead of wall time (A14).
    let (live_tip_anchor, set_live_tip_anchor) = signal((0u64, 0u64));
    Effect::new(move |_| {
        if let Some(live) = cached_live.get() {
            let h = live.blockchain.blocks;
            let t = live.blockchain.time;
            if h > 0 && t > 0 {
                let (prev_h, _) = live_tip_anchor.get_untracked();
                if h != prev_h {
                    let now = chrono::Utc::now().timestamp().max(0) as u64;
                    set_live_tip_anchor.set((h, t.min(now)));
                }
            }
        }
    });

    let elapsed_since_last_block = Signal::derive(move || {
        let _ = tick.get(); // re-run every tick
                            // Prefer our arrival-clamped SSE timestamp whenever SSE height >= RPC height
                            // (equal RPC/SSE heights must not revert to an unclamped future miner header).
        let live = cached_live.get();
        let live_h = live.as_ref().map(|s| s.blockchain.blocks).unwrap_or(0);
        let live_t = live.as_ref().map(|s| s.blockchain.time).unwrap_or(0);
        let (anchor_h, anchor_t) = live_tip_anchor.get();
        let effective_live_t = if anchor_h == live_h && anchor_t > 0 {
            anchor_t
        } else {
            live_t
        };
        let s_h = sse_height.get();
        let s_t = sse_ts.get();
        let ts = if s_h >= live_h && s_t > 0 {
            s_t
        } else {
            effective_live_t
        };
        if ts == 0 {
            return None;
        }
        let now = chrono::Utc::now().timestamp().max(0) as u64;
        Some(now.saturating_sub(ts))
    });

    let time_since =
        Signal::derive(move || match elapsed_since_last_block.get() {
            None => "waiting...".to_string(),
            Some(elapsed) if elapsed < 60 => format!("{}s ago", elapsed),
            Some(elapsed) => format!("{}m {}s ago", elapsed / 60, elapsed % 60),
        });

    let pending_block_age = Signal::derive(move || {
        let _ = tick.get();
        let pb = pending_block.get()?;
        let now = chrono::Utc::now().timestamp().max(0) as u64;
        Some(now.saturating_sub(pb.received_at_client))
    });

    let avg_block_secs = Signal::derive(move || {
        compute_recent_avg_block_secs(&recent_block_pairs.get())
    });

    let network_pulse = Signal::derive(move || {
        let live = cached_live.get();
        let (
            mempool_available,
            mempool_tx_count,
            mempool_vbytes,
            next_block_fee,
            mempool_min_fee,
        ) = match live.as_ref() {
            Some(l) => (
                l.mempool_available,
                l.mempool.size,
                l.mempool.bytes,
                l.next_block_fee,
                btc_per_kvb_to_sat_per_vb(l.mempool.mempoolminfee),
            ),
            None => (false, 0, 0, 0.0, 0.0),
        };
        compute_network_pulse(
            node_stale.get(),
            mempool_available,
            mempool_tx_count,
            mempool_vbytes,
            next_block_fee,
            mempool_min_fee,
            elapsed_since_last_block.get().unwrap_or(0),
            avg_block_secs.get(),
            pending_block_age.get(),
        )
    });

    let fee_display = Signal::derive(move || {
        cached_live
            .get()
            .filter(|s| s.mempool.size > 0 || s.next_block_fee > 0.0)
            .map(|s| format!("{:.1} sat/vB", s.next_block_fee))
            .unwrap_or_else(|| "-- sat/vB".to_string())
    });

    view! {
        <Title text="Block Heartbeat | We Hodl BTC"/>
        <Meta name="description" content="Watch Bitcoin breathe. A live EKG visualization of block arrivals, where every spike tells a story of transactions, fees, and network activity."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/heartbeat"/>

        // On large screens (>1800px viewport) the whole heartbeat page breaks out
        // of the 1750px observatory container to ~95vw, centered on the viewport,
        // so the banner, EKG, vitals, rhythm strip and feed all align to one
        // impactful wide column. Below 1800px it's unchanged (fills the container).
        <div class="space-y-4 sm:space-y-5 relative min-[1800px]:w-[95vw] min-[1800px]:left-1/2 min-[1800px]:-translate-x-1/2">
            // Hero banner
            <div class="relative rounded-2xl overflow-hidden">
                <img
                    src="/img/observatory_hero.png"
                    alt="Block Heartbeat"
                    class="w-full h-[76px] sm:h-[96px] lg:h-[116px] object-cover object-center"
                />
                <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/60 to-[#123c64]/30"></div>
                <div class="absolute inset-0 flex flex-col items-center justify-end pb-2.5 sm:pb-3">
                    <h1 class="text-lg sm:text-2xl lg:text-3xl font-title text-white mb-0.5 drop-shadow-lg">"Block Heartbeat"</h1>
                    <p class="text-[11px] sm:text-sm text-white/65 max-w-xl mx-auto px-4 text-center drop-shadow">
                        "A live EKG of Bitcoin \u{b7} spikes are blocks, bricks are transactions"
                    </p>
                </div>
            </div>

            // EKG Canvas card
            // Sleek 2-bar terminal layout on desktop (single top header + single
            // unified bottom bar) and compact 2-row top + 1-row controls on mobile,
            // maximizing vertical canvas space on both phones and 13-14" laptops.
            <div id="heartbeat-card" class="relative bg-[#0d2137] border border-white/10 rounded-2xl overflow-hidden flex flex-col max-sm:h-[calc(100vh-155px)] max-sm:min-h-[380px] sm:h-[calc(100vh-185px)] sm:min-h-[440px]">
                // Unified top header bar:
                // - lg+: 1 sleek row (LIVE + height | compact search + Landmarks | Last block + fullscreen)
                // - <lg: 2 clean rows (status/timer row + compact search/Landmarks row)
                <div class="flex flex-col lg:flex-row lg:items-center lg:justify-between gap-1.5 lg:gap-3 px-2.5 sm:px-3.5 py-1.5 sm:py-2 border-b border-white/10 bg-[#0a1929]/55">
                    // Status + tip height (left on all viewports; timer shares this row on <lg)
                    <div class="flex items-center justify-between lg:justify-start gap-2 sm:gap-2.5 shrink-0">
                        <div class="flex items-center gap-2">
                            <div
                                class="flex items-center gap-1.5 cursor-help"
                                data-tip=move || status_tip.get()
                                tabindex="0"
                            >
                                <span class="relative flex h-2 w-2">
                                    <span
                                        class="animate-ping absolute inline-flex h-full w-full rounded-full opacity-60"
                                        style=move || format!("background-color: {}", status_color.get())
                                    ></span>
                                    <span
                                        class="relative inline-flex rounded-full h-2 w-2"
                                        style=move || format!("background-color: {}", status_color.get())
                                    ></span>
                                </span>
                                <span class="text-[11px] sm:text-xs text-white/60 font-mono">{status_label}</span>
                            </div>
                            <span class="text-xs sm:text-sm text-[#00e676] font-mono font-semibold tabular-nums">{block_height}</span>
                        </div>
                        // Mobile/tablet timer + fullscreen sits right of status on row 1
                        <div class="flex lg:hidden items-center gap-2 text-[11px] sm:text-xs text-[#00e676] font-mono tabular-nums">
                            <span class="truncate">
                                <span class="hidden sm:inline text-white/55">"Last block: "</span>
                                {time_since}
                            </span>
                            <button
                                id="heartbeat-fullscreen-btn-mobile"
                                class="text-white/40 hover:text-[#00e676] transition-colors cursor-pointer hidden"
                                title="Toggle fullscreen"
                                on:click=move |_| {
                                    #[cfg(feature = "hydrate")]
                                    {
                                        let doc = leptos::prelude::document();
                                        if doc.fullscreen_element().is_some() {
                                            doc.exit_fullscreen();
                                        } else if let Some(el) = doc.get_element_by_id("heartbeat-card") {
                                            let _ = el.request_fullscreen();
                                        }
                                    }
                                }
                            >
                                <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 3.75v4.5m0-4.5h4.5m-4.5 0L9 9M3.75 20.25v-4.5m0 4.5h4.5m-4.5 0L9 15M20.25 3.75h-4.5m4.5 0v4.5m0-4.5L15 9m5.25 11.25h-4.5m4.5 0v-4.5m0 4.5L15 15"/>
                                </svg>
                            </button>
                        </div>
                    </div>

                    // Compact search + 1-click Landmarks modal trigger (center on lg+, row 2 on <lg)
                    <div class="flex items-center gap-1.5 flex-1 lg:max-w-xl min-w-0">
                        <div class="relative flex-1 min-w-0 flex items-center">
                            <input
                                id="heartbeat-tx-search"
                                type="text"
                                autocomplete="off"
                                autocapitalize="off"
                                spellcheck="false"
                                placeholder="Search txid, #height, or block hash..."
                                aria-label="Search transaction ID, block height, or block hash"
                                class="w-full pl-2.5 pr-7 py-1 rounded-lg bg-white/[0.06] border border-white/10 text-xs sm:text-xs font-mono text-white/90 placeholder-white/40 focus:border-[#00e676]/60 focus:bg-white/[0.09] focus:outline-none transition-colors"
                                prop:value=move || search_query.get()
                                on:input=move |ev| {
                                    set_search_query.set(event_target_value(&ev));
                                    if search_feedback.get_untracked().is_some() {
                                        set_search_feedback.set(None);
                                    }
                                }
                                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                    if ev.key() == "Enter" {
                                        ev.prevent_default();
                                        #[cfg(feature = "hydrate")]
                                        {
                                            let q = search_query.get_untracked();
                                            let client = hb_client.try_with_value(|c| c.clone()).flatten();
                                            if let Some(c) = client {
                                                c.search(&q);
                                            }
                                        }
                                    } else if ev.key() == "Escape" {
                                        set_search_query.set(String::new());
                                        set_search_feedback.set(None);
                                    }
                                }
                            />
                            <Show when=move || !search_query.get().is_empty()>
                                <button
                                    type="button"
                                    aria-label="Clear search"
                                    title="Clear search"
                                    class="absolute right-1.5 text-white/45 hover:text-white/85 text-xs font-mono px-1 cursor-pointer transition-colors"
                                    on:click=move |_| {
                                        set_search_query.set(String::new());
                                        set_search_feedback.set(None);
                                    }
                                >
                                    "\u{00D7}"
                                </button>
                            </Show>
                        </div>
                        <button
                            type="button"
                            aria-label="Search"
                            title="Search transaction or block"
                            class="inline-flex items-center justify-center gap-1 h-7 px-2 sm:px-2.5 rounded-lg bg-[#00e676]/15 hover:bg-[#00e676]/25 border border-[#00e676]/40 text-xs font-mono font-semibold text-[#00e676] transition-colors cursor-pointer shrink-0"
                            on:click=move |_| {
                                #[cfg(feature = "hydrate")]
                                {
                                    let q = search_query.get_untracked();
                                    let client = hb_client.try_with_value(|c| c.clone()).flatten();
                                    if let Some(c) = client {
                                        c.search(&q);
                                    }
                                }
                            }
                        >
                            <svg class="w-3.5 h-3.5 sm:hidden" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2.2">
                                <circle cx="11" cy="11" r="7"/>
                                <path stroke-linecap="round" stroke-linejoin="round" d="M20 20l-3.5-3.5"/>
                            </svg>
                            <span class="hidden sm:inline">"Search"</span>
                        </button>
                        <button
                            type="button"
                            id="heartbeat-landmarks-more-btn"
                            aria-haspopup="dialog"
                            title="Browse curated historic Landmark Blocks"
                            class="inline-flex items-center gap-1 h-7 px-2 sm:px-2.5 rounded-lg bg-[#f7931a]/12 hover:bg-[#f7931a]/22 border border-[#f7931a]/40 hover:border-[#f7931a]/70 text-[11px] sm:text-xs font-mono font-semibold text-[#f7931a] whitespace-nowrap shrink-0 cursor-pointer transition-colors"
                            on:click=move |_| {
                                set_landmark_cat.set(None);
                                set_landmark_filter.set(String::new());
                                set_show_landmarks.set(true);
                            }
                        >
                            <span>"\u{2605}"</span>
                            <span class="hidden sm:inline">{format!("Landmarks ({landmark_total})")}</span>
                            <span class="sm:hidden">"Landmarks"</span>
                        </button>
                    </div>

                    // Desktop-only timer + fullscreen (right on lg+)
                    <div class="hidden lg:flex items-center gap-2.5 text-xs lg:text-sm text-[#00e676] font-mono tabular-nums shrink-0">
                        <span class="truncate">
                            <span class="text-white/75">"Last block: "</span>
                            {time_since}
                        </span>
                        // Fullscreen toggle: hidden on iOS where the API isn't supported
                        <button
                            id="heartbeat-fullscreen-btn"
                            class="text-white/40 hover:text-[#00e676] transition-colors cursor-pointer hidden"
                            title="Toggle fullscreen"
                            on:click=move |_| {
                                #[cfg(feature = "hydrate")]
                                {
                                    let doc = leptos::prelude::document();
                                    if doc.fullscreen_element().is_some() {
                                        doc.exit_fullscreen();
                                    } else if let Some(el) = doc.get_element_by_id("heartbeat-card") {
                                        let _ = el.request_fullscreen();
                                    }
                                }
                            }
                        >
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 3.75v4.5m0-4.5h4.5m-4.5 0L9 9M3.75 20.25v-4.5m0 4.5h4.5m-4.5 0L9 15M20.25 3.75h-4.5m4.5 0v4.5m0-4.5L15 9m5.25 11.25h-4.5m4.5 0v-4.5m0 4.5L15 15"/>
                            </svg>
                        </button>
                    </div>
                </div>

                // Optional search feedback banner (only rendered when a search returns a notice/error)
                <Show when=move || search_feedback.get().is_some()>
                    {move || {
                        search_feedback.get().map(|(is_err, msg)| {
                            let bar_class = if is_err {
                                "flex items-center justify-between gap-2 px-3 py-1 border-b border-[#f44336]/25 bg-[#f44336]/10 text-[11px] font-mono text-[#ff8a80]"
                            } else {
                                "flex items-center justify-between gap-2 px-3 py-1 border-b border-[#00e676]/20 bg-[#00e676]/10 text-[11px] font-mono text-[#00e676]"
                            };
                            let title_msg = msg.clone();
                            view! {
                                <div class=bar_class>
                                    <span class="truncate" title=title_msg>{msg}</span>
                                    <button
                                        type="button"
                                        aria-label="Dismiss notice"
                                        class="text-white/60 hover:text-white cursor-pointer shrink-0 px-1"
                                        on:click=move |_| set_search_feedback.set(None)
                                    >
                                        "\u{00D7}"
                                    </button>
                                </div>
                            }
                        })
                    }}
                </Show>

                // Canvas with overlays
                // The canvas flexes to fill all card space left over after the
                // status/search/control/info bars: on both mobile and desktop the card
                // is now viewport-height, so flex-1 absorbs the remainder. (Also
                // how it fills during fullscreen when JS sets the card to 100vh.)
                <div id="heartbeat-canvas-wrap" class="relative flex-1 min-h-0 sm:min-h-[250px]">
                    <canvas
                        id="heartbeat-canvas"
                        class="block absolute inset-0 w-full h-full"
                    ></canvas>

                    // Pending-Block Cue Banner (shown only after 2s grace period in live mode)
                    <Show when=move || {
                        history_mode.get().is_none()
                            && pending_block.get().is_some()
                            && pending_block_age.get().is_some_and(|age| age >= 2)
                    }>
                        {move || {
                            let Some(pb) = pending_block.get() else {
                                return view! { <div></div> }.into_any();
                            };
                            let hash_for_check = pb.hash.clone();
                            let hash_for_click = pb.hash.clone();
                            view! {
                                <div
                                    id="heartbeat-pending-block-banner"
                                    class="absolute top-3 left-1/2 -translate-x-1/2 z-20 flex items-center gap-2 px-3 py-1.5 rounded-full bg-[#0a1929]/95 border border-[#38bdf8]/40 shadow-lg backdrop-blur-md text-xs font-mono text-[#bae6fd] whitespace-nowrap max-w-[calc(100%-1.5rem)]"
                                >
                                    <span class="relative flex h-2 w-2 shrink-0">
                                        <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-[#38bdf8] opacity-75"></span>
                                        <span class="relative inline-flex rounded-full h-2 w-2 bg-[#38bdf8]"></span>
                                    </span>
                                    <span class="truncate">
                                        {move || {
                                            let age = pending_block_age.get().unwrap_or(0);
                                            format!("Block received \u{00b7} fetching details ({age}s)")
                                        }}
                                    </span>
                                    <Show when=move || {
                                        let _ = tick.get();
                                        hb_v2_is_pending_slot_offscreen(&hash_for_check)
                                    }>
                                        {
                                            let focus_hash = hash_for_click.clone();
                                            view! {
                                                <button
                                                    type="button"
                                                    class="px-2 py-0.5 rounded-md bg-[#38bdf8]/15 hover:bg-[#38bdf8]/30 border border-[#38bdf8]/45 text-[11px] font-semibold text-[#38bdf8] cursor-pointer transition-colors shrink-0"
                                                    on:click=move |_| {
                                                        #[cfg(feature = "hydrate")]
                                                        {
                                                            let client = hb_client.try_with_value(|c| c.clone()).flatten();
                                                            if let Some(c) = client {
                                                                c.focus_pending_slot(&focus_hash);
                                                                return;
                                                            }
                                                        }
                                                        let _ = hb_v2_focus_pending_slot(&focus_hash);
                                                    }
                                                >
                                                    "View slot"
                                                </button>
                                            }
                                        }
                                    </Show>
                                </div>
                            }.into_any()
                        }}
                    </Show>

                    // Historical View Banner (when inspecting a historical block window)
                    <Show when=move || history_mode.get().is_some()>
                        {move || {
                            let Some(hm) = history_mode.get() else {
                                return view! { <div></div> }.into_any();
                            };
                            let target_height = hm.target_height;
                            let landmark_match = heartbeat_landmarks()
                                .into_iter()
                                .find(|lm| lm.height == target_height);
                            let range_label = format!(
                                "Block #{} (#{}\u{2013}#{})",
                                super::helpers::format_number(hm.target_height),
                                super::helpers::format_number(hm.from_height),
                                super::helpers::format_number(hm.to_height),
                            );
                            view! {
                                <div
                                    id="heartbeat-history-mode-banner"
                                    class="absolute top-2 left-2 right-2 sm:top-3 sm:left-3 sm:right-auto z-20 flex flex-col gap-1.5 px-2.5 sm:px-3 py-1.5 rounded-xl bg-[#0a1929]/95 border border-[#f7931a]/45 shadow-xl backdrop-blur-md text-[11px] sm:text-xs font-mono text-white/90 sm:max-w-[min(38rem,calc(100%-1.5rem))]"
                                >
                                    <div class="flex flex-wrap items-center gap-1.5 sm:gap-2">
                                        <span class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wider bg-[#f7931a]/20 border border-[#f7931a]/50 text-[#f7931a]">
                                            "HISTORICAL VIEW"
                                        </span>
                                        <span class="text-white/85 font-semibold tabular-nums">
                                            {range_label}
                                        </span>
                                        <button
                                            type="button"
                                            class="px-2 py-0.5 rounded-md bg-white/10 hover:bg-white/20 border border-white/15 text-[11px] text-white/85 cursor-pointer transition-colors"
                                            on:click=move |_| show_block_detail_js(target_height)
                                        >
                                            "Block details"
                                        </button>
                                        <button
                                            type="button"
                                            class="px-2 py-0.5 rounded-md bg-white/5 hover:bg-white/15 border border-white/15 text-[11px] text-white/75 cursor-pointer transition-colors"
                                            on:click=move |_| {
                                                #[cfg(feature = "hydrate")]
                                                {
                                                    if let Some(win) = leptos::web_sys::window() {
                                                        let origin = win
                                                            .location()
                                                            .origin()
                                                            .unwrap_or_else(|_| "https://www.wehodlbtc.com".to_string());
                                                        let url = format!("{origin}/observatory/heartbeat?block={target_height}");
                                                        let _ = win.navigator().clipboard().write_text(&url);
                                                        set_copied_history_link.set(true);
                                                        leptos::prelude::set_timeout(
                                                            move || {
                                                                let _ = set_copied_history_link.try_set(false);
                                                            },
                                                            std::time::Duration::from_millis(1500),
                                                        );
                                                    }
                                                }
                                            }
                                        >
                                            {move || if copied_history_link.get() { "Link copied" } else { "Copy link" }}
                                        </button>
                                        <button
                                            type="button"
                                            class="px-2.5 py-0.5 rounded-md bg-[#00e676]/20 hover:bg-[#00e676]/30 border border-[#00e676]/55 text-[11px] font-semibold text-[#00e676] cursor-pointer transition-colors"
                                            on:click=move |_| {
                                                #[cfg(feature = "hydrate")]
                                                {
                                                    let client = hb_client.try_with_value(|c| c.clone()).flatten();
                                                    if let Some(c) = client {
                                                        c.exit_history_mode();
                                                        return;
                                                    }
                                                }
                                                hb_v2_exit_history_mode();
                                            }
                                        >
                                            "Return to LIVE"
                                        </button>
                                    </div>
                                    {landmark_match.map(|lm| {
                                        let lm_color = lm.category.color();
                                        view! {
                                            <div class="flex flex-col sm:flex-row sm:items-baseline gap-1 sm:gap-2 pt-1 border-t border-white/10 font-sans text-[11px] sm:text-xs">
                                                <span
                                                    class="font-mono font-semibold shrink-0"
                                                    style=format!("color: {lm_color}")
                                                >
                                                    {format!("\u{2605} {} ({})", lm.title, lm.date)}
                                                </span>
                                                <span class="text-white/75 line-clamp-2 sm:line-clamp-1">
                                                    {lm.description}
                                                </span>
                                            </div>
                                        }
                                    })}
                                </div>
                            }.into_any()
                        }}
                    </Show>

                    // In-place live Pinned Transaction Detail Panel (visible in fullscreen too)
                    <Show when=move || pinned_tx.get().is_some()>
                        {
                            move || {
                                let Some(tx) = pinned_tx.get() else {
                                    return view! { <div></div> }.into_any();
                                };
                                let txid_full = tx.txid.clone();
                                let txid_short = short_txid(&txid_full);
                                #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
                                let txid_copy = txid_full.clone();
                                #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
                                let txid_share = txid_full.clone();
                                let txid_locate = txid_full.clone();
                                let mempool_url = format!("https://mempool.space/tx/{txid_full}");

                                let fee_rate = tx.fee_rate_sat_per_vb();
                                let (band_label, band_color) = fee_band_for_rate(fee_rate);
                                let fee_rate_short = if fee_rate.is_some() {
                                    format_fee_rate(fee_rate)
                                } else {
                                    band_label.to_string()
                                };
                                let fee_rate_tip = format!("Fee band: {band_label}");
                                let fee_paid_text = tx
                                    .fee
                                    .map(|f| format!("{} sats", super::helpers::format_number(f)))
                                    .unwrap_or_else(|| "Pending...".to_string());
                                let size_text = tx
                                    .vsize
                                    .map(|v| format!("{} vB", super::helpers::format_number(u64::from(v))))
                                    .unwrap_or_else(|| "Pending...".to_string());
                                let weight_text = tx
                                    .deep
                                    .as_ref()
                                    .and_then(|d| d.weight)
                                    .map(|w| format!("{} WU", super::helpers::format_number(w)))
                                    .unwrap_or_else(|| {
                                        tx.vsize
                                            .map(|v| format!("~{} WU", super::helpers::format_number(u64::from(v) * 4)))
                                            .unwrap_or_else(|| "--".to_string())
                                    });
                                let value_text = tx
                                    .value
                                    .map(|sats| format!("{:.8} BTC", sats as f64 / 1e8))
                                    .unwrap_or_else(|| "Pending...".to_string());
                                let first_seen_text = format_first_seen(tx.first_seen);
                                let notable = primary_notable_label(tx.flags);
                                let header_features: Vec<String> = tx
                                    .deep
                                    .as_ref()
                                    .map(|d| d.features.clone())
                                    .unwrap_or_default();
                                let op_return = tx
                                    .op_return_text
                                    .as_deref()
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_string);
                                let status_for_header = tx.status.clone();
                                let status_for_footer = tx.status.clone();
                                let is_unconfirmed = matches!(tx.status, PinnedTxStatus::Unconfirmed);
                                let deep = tx.deep.clone();
                                let deep_loading = tx.deep_loading;

                                view! {
                                    <div
                                        id="heartbeat-pinned-tx-panel"
                                        class="absolute inset-x-2 top-2 sm:inset-x-auto sm:top-3 sm:right-3 lg:top-4 lg:right-4 z-20 w-auto sm:w-[350px] lg:w-[400px] 2xl:w-[430px] bg-[#071320]/95 border border-white/15 rounded-xl shadow-2xl backdrop-blur-md text-xs lg:text-[13px] font-mono text-white/85 overflow-hidden max-h-[calc(100%-1rem)] sm:max-h-[calc(100%-1.5rem)] overflow-y-auto"
                                    >
                                        // Header: status pill + protocol feature chips + close button
                                        <div class="flex items-start justify-between gap-2 px-3.5 py-2.5 border-b border-white/10 bg-white/[0.02]">
                                            <div class="flex flex-wrap items-center gap-1.5 min-w-0">
                                                {match status_for_header {
                                                    PinnedTxStatus::Unconfirmed => view! {
                                                        <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] lg:text-[11px] font-bold tracking-wider bg-[#00e676]/15 border border-[#00e676]/40 text-[#00e676]">
                                                            <span class="w-1.5 h-1.5 rounded-full bg-[#00e676]"></span>
                                                            "IN MEMPOOL"
                                                        </span>
                                                    }.into_any(),
                                                    PinnedTxStatus::Confirmed { block_height, .. } => {
                                                        let badge_text = if block_height > 0 {
                                                            format!(
                                                                "CONFIRMED #{}",
                                                                super::helpers::format_number(block_height)
                                                            )
                                                        } else {
                                                            "CONFIRMED".to_string()
                                                        };
                                                        view! {
                                                            <div class="flex flex-wrap items-center gap-1.5">
                                                                <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] lg:text-[11px] font-bold tracking-wider bg-[#f7931a]/15 border border-[#f7931a]/40 text-[#f7931a]">
                                                                    <span class="w-1.5 h-1.5 rounded-full bg-[#00e676]"></span>
                                                                    {badge_text}
                                                                </span>
                                                                {(block_height > 0).then(|| view! {
                                                                    <button
                                                                        type="button"
                                                                        class="px-2 py-0.5 rounded bg-[#f7931a]/15 hover:bg-[#f7931a]/25 border border-[#f7931a]/40 text-[10px] font-semibold text-[#f7931a] cursor-pointer transition-colors"
                                                                        on:click=move |_| {
                                                                            #[cfg(feature = "hydrate")]
                                                                            {
                                                                                let client = hb_client.try_with_value(|c| c.clone()).flatten();
                                                                                if let Some(c) = client {
                                                                                    c.view_block_height(block_height);
                                                                                    return;
                                                                                }
                                                                            }
                                                                            let _ = hb_v2_focus_block_height(block_height);
                                                                        }
                                                                    >
                                                                        "View block"
                                                                    </button>
                                                                    <button
                                                                        type="button"
                                                                        class="px-2 py-0.5 rounded bg-white/5 hover:bg-white/15 border border-white/15 text-[10px] font-semibold text-white/80 cursor-pointer transition-colors"
                                                                        on:click=move |_| show_block_detail_js(block_height)
                                                                    >
                                                                        "Details"
                                                                    </button>
                                                                })}
                                                            </div>
                                                        }.into_any()
                                                    }
                                                    PinnedTxStatus::Removed => view! {
                                                        <div class="flex flex-col gap-0.5">
                                                            <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] lg:text-[11px] font-bold tracking-wider bg-[#f44336]/15 border border-[#f44336]/35 text-[#ff8a80] w-fit">
                                                                "REMOVED FROM MEMPOOL"
                                                            </span>
                                                            <span class="text-[10px] text-white/60">"Replaced or evicted"</span>
                                                        </div>
                                                    }.into_any(),
                                                }}
                                                {header_features.into_iter().map(|feat| {
                                                    let (fg, border, bg, tip) = match feat.as_str() {
                                                        "RBF" => (
                                                            "#f7931a",
                                                            "rgba(247,147,26,0.4)",
                                                            "rgba(247,147,26,0.12)",
                                                            "Replace-By-Fee (BIP-125): sender can bump the fee before confirmation",
                                                        ),
                                                        "Taproot" => (
                                                            "#c084fc",
                                                            "rgba(192,132,252,0.4)",
                                                            "rgba(192,132,252,0.12)",
                                                            "Uses Taproot (SegWit v1 / P2TR) outputs",
                                                        ),
                                                        "SegWit" => (
                                                            "#38bdf8",
                                                            "rgba(56,189,248,0.4)",
                                                            "rgba(56,189,248,0.12)",
                                                            "Uses Segregated Witness (BIP-141) witness data",
                                                        ),
                                                        "Coinbase" => (
                                                            "#ffd700",
                                                            "rgba(255,215,0,0.4)",
                                                            "rgba(255,215,0,0.12)",
                                                            "Block reward coinbase transaction",
                                                        ),
                                                        _ => (
                                                            "#cbd5e1",
                                                            "rgba(203,213,225,0.3)",
                                                            "rgba(203,213,225,0.10)",
                                                            "Transaction script/protocol feature",
                                                        ),
                                                    };
                                                    view! {
                                                        <span
                                                            data-tip=tip
                                                            class="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-semibold tracking-wide border"
                                                            style=format!("color:{fg};border-color:{border};background:{bg}")
                                                        >
                                                            {feat}
                                                        </span>
                                                    }
                                                }).collect_view()}
                                                {notable.map(|(label, color)| view! {
                                                    <span
                                                        class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wide border"
                                                        style=format!("color: {color}; border-color: {color}66; background-color: {color}1a")
                                                    >
                                                        {label}
                                                    </span>
                                                })}
                                            </div>
                                            <button
                                                type="button"
                                                aria-label="Close transaction panel"
                                                title="Close"
                                                class="w-6 h-6 shrink-0 rounded-md bg-white/5 hover:bg-white/15 text-white/70 hover:text-white flex items-center justify-center text-sm cursor-pointer transition-colors"
                                                on:click=move |_| {
                                                    set_copied_txid.set(false);
                                                    #[cfg(feature = "hydrate")]
                                                    {
                                                        let client = hb_client.try_with_value(|c| c.clone()).flatten();
                                                        if let Some(c) = client {
                                                            c.clear_pinned_tx();
                                                            return;
                                                        }
                                                    }
                                                    set_pinned_tx.set(None);
                                                    hb_v2_set_pinned_tx(None);
                                                }
                                            >
                                                "\u{00D7}"
                                            </button>
                                        </div>

                                        // Body
                                        <div class="px-3.5 py-3 space-y-3">
                                            // Labeled TXID + Copy
                                            <div class="pb-2 border-b border-white/10 space-y-1">
                                                <div class="text-[10px] lg:text-[11px] text-white/80 font-medium tracking-wide uppercase">
                                                    "Transaction ID"
                                                </div>
                                                <div class="flex items-center justify-between gap-2">
                                                    <span class="text-xs text-white font-medium truncate select-all" data-tip=txid_full.clone()>
                                                        {txid_short}
                                                    </span>
                                                    <button
                                                        type="button"
                                                        class="px-2 py-0.5 rounded bg-white/10 hover:bg-white/20 text-[11px] text-white/95 cursor-pointer transition-colors shrink-0"
                                                        on:click=move |_| {
                                                            #[cfg(feature = "hydrate")]
                                                            {
                                                                if let Some(win) = leptos::web_sys::window() {
                                                                    let clipboard = win.navigator().clipboard();
                                                                    let _ = clipboard.write_text(&txid_copy);
                                                                    set_copied_txid.set(true);
                                                                    leptos::prelude::set_timeout(
                                                                        move || {
                                                                            let _ = set_copied_txid.try_set(false);
                                                                        },
                                                                        std::time::Duration::from_millis(1500),
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    >
                                                        {move || if copied_txid.get() { "Copied" } else { "Copy" }}
                                                    </button>
                                                </div>
                                            </div>

                                            // Queue position hero readout while unconfirmed.
                                            // Reads `pinned_queue` inside its own reactive scope so 2.5s
                                            // queue refreshes update only these text nodes without remounting
                                            // the panel, resetting I/O scroll, or detaching active tooltips.
                                            {is_unconfirmed.then(|| {
                                                let q_view = Signal::derive(move || {
                                                    match pinned_queue.get() {
                                                        Some(q) => format_queue_position(q),
                                                        None => (
                                                            "Awaiting fee details".to_string(),
                                                            String::new(),
                                                            "#e2e8f0",
                                                        ),
                                                    }
                                                });
                                                view! {
                                                    <div
                                                        class="rounded-lg bg-white/[0.05] border border-white/15 border-l-2 px-3 py-2"
                                                        style=move || format!("border-left-color: {}", q_view.get().2)
                                                        data-tip="Estimated from solo fee rate (sat/vB) across observed mempool transactions.\nCPFP packages and miner policy can move a transaction ahead or behind."
                                                    >
                                                        <div class="flex items-baseline justify-between gap-2">
                                                            <span class="text-[10px] lg:text-[11px] text-white/85 font-medium tracking-wide uppercase">
                                                                "Queue Position (est.)"
                                                            </span>
                                                            <span
                                                                class="text-sm lg:text-[15px] font-bold tabular-nums"
                                                                style=move || format!("color: {}", q_view.get().2)
                                                            >
                                                                {move || q_view.get().0}
                                                            </span>
                                                        </div>
                                                        <div
                                                            class=move || {
                                                                if q_view.get().1.is_empty() {
                                                                    "hidden"
                                                                } else {
                                                                    "text-[11px] text-white/90 tabular-nums mt-0.5"
                                                                }
                                                            }
                                                        >
                                                            {move || q_view.get().1}
                                                        </div>
                                                    </div>
                                                }
                                            })}

                                            // Clean aligned 3x2 Key Metrics Grid
                                            <div class="grid grid-cols-2 gap-x-4 gap-y-2.5 text-xs lg:text-[13px]">
                                                <div data-tip=fee_rate_tip>
                                                    <div class="text-[10px] lg:text-[11px] text-white/80 font-medium tracking-wide uppercase">"Fee Rate"</div>
                                                    <div class="flex items-center gap-1.5 text-white font-semibold tabular-nums mt-0.5">
                                                        <span
                                                            class="w-2 h-2 rounded-full shrink-0"
                                                            style=format!("background-color: {band_color}")
                                                        ></span>
                                                        <span>{fee_rate_short}</span>
                                                    </div>
                                                </div>
                                                <div>
                                                    <div class="text-[10px] lg:text-[11px] text-white/80 font-medium tracking-wide uppercase">"Fee Paid"</div>
                                                    <div class="text-white font-semibold tabular-nums mt-0.5">{fee_paid_text}</div>
                                                </div>
                                                <div>
                                                    <div class="text-[10px] lg:text-[11px] text-white/80 font-medium tracking-wide uppercase">"Output Value"</div>
                                                    <div class="text-white font-semibold tabular-nums mt-0.5">{value_text}</div>
                                                </div>
                                                <div>
                                                    <div class="text-[10px] lg:text-[11px] text-white/80 font-medium tracking-wide uppercase">"Size"</div>
                                                    <div class="text-white font-semibold tabular-nums mt-0.5">{size_text}</div>
                                                </div>
                                                <div>
                                                    <div class="text-[10px] lg:text-[11px] text-white/80 font-medium tracking-wide uppercase">"First Observed"</div>
                                                    <div class="text-white font-semibold tabular-nums mt-0.5">{first_seen_text}</div>
                                                </div>
                                                <div>
                                                    <div class="text-[10px] lg:text-[11px] text-white/80 font-medium tracking-wide uppercase">"Weight"</div>
                                                    <div class="text-white font-semibold tabular-nums mt-0.5">{weight_text}</div>
                                                </div>
                                            </div>

                                            // OP_RETURN text (strictly text nodes, never inner_html)
                                            {op_return.map(|msg| view! {
                                                <div class="rounded-lg bg-white/[0.04] border border-[#ffa500]/30 px-2.5 py-2 space-y-1">
                                                    <div class="text-[10px] lg:text-[11px] text-[#ffa500] uppercase font-semibold">"OP_RETURN Message"</div>
                                                    <p class="text-[11px] lg:text-xs text-white/90 break-words whitespace-pre-wrap">{msg}</p>
                                                </div>
                                            })}

                                            // Collapsible Inputs & Outputs disclosure (replaces redundant counts row)
                                            {if let Some(d) = deep {
                                                let total_in = d.inputs.unwrap_or(d.input_items.len() as u64);
                                                let total_out = d.outputs.unwrap_or(d.output_items.len() as u64);
                                                let has_io_items = !d.input_items.is_empty() || !d.output_items.is_empty();
                                                let in_items = d.input_items.clone();
                                                let out_items = d.output_items.clone();
                                                let extra_in = total_in.saturating_sub(in_items.len() as u64);
                                                let extra_out = total_out.saturating_sub(out_items.len() as u64);
                                                let io_toggle_label = format!(
                                                    "Inputs & Outputs ({} \u{2192} {})",
                                                    super::helpers::format_number(total_in),
                                                    super::helpers::format_number(total_out),
                                                );
                                                view! {
                                                    <div>
                                                        {has_io_items.then(|| {
                                                            view! {
                                                                <div class="rounded-lg bg-white/[0.04] border border-white/15 overflow-hidden">
                                                                    <button
                                                                        type="button"
                                                                        aria-expanded=move || if io_expanded.get() { "true" } else { "false" }
                                                                        class="w-full flex items-center justify-between gap-2 px-2.5 py-1.5 text-[11px] font-mono text-white/95 hover:text-white hover:bg-white/[0.06] cursor-pointer transition-colors"
                                                                        on:click=move |_| set_io_expanded.update(|v| *v = !*v)
                                                                    >
                                                                        <span>{io_toggle_label}</span>
                                                                        <span class="text-[10px] text-white/80">
                                                                            {move || if io_expanded.get() { "\u{25BE}" } else { "\u{25B8}" }}
                                                                        </span>
                                                                    </button>
                                                                    <div
                                                                        class=move || {
                                                                            if io_expanded.get() {
                                                                                "px-2.5 pb-2.5 pt-1 border-t border-white/10 max-h-52 overflow-y-auto space-y-2.5 text-[10px] lg:text-[11px]"
                                                                            } else {
                                                                                "hidden"
                                                                            }
                                                                        }
                                                                    >
                                                                            {(!in_items.is_empty()).then(|| view! {
                                                                                <div class="space-y-1">
                                                                                    <div class="text-[10px] text-white/85 font-semibold uppercase tracking-wider">
                                                                                        {format!("Inputs ({})", super::helpers::format_number(total_in))}
                                                                                    </div>
                                                                                    {in_items.iter().enumerate().map(|(idx, entry)| {
                                                                                        let short_lbl = short_io_label(&entry.label);
                                                                                        let amt = entry
                                                                                            .value
                                                                                            .map(|s| format!("{:.8} BTC", s as f64 / 1e8))
                                                                                            .unwrap_or_else(|| "--".to_string());
                                                                                        let row_key = format!("in:{idx}");
                                                                                        let copy_val = entry.copy.clone();
                                                                                        let full_lbl = copy_val.clone().unwrap_or_else(|| entry.label.clone());
                                                                                        view! {
                                                                                            <div class="flex items-center justify-between gap-2 py-0.5 border-b border-white/[0.05] last:border-b-0">
                                                                                                {if let Some(copy_str) = copy_val {
                                                                                                    let row_key_click = row_key.clone();
                                                                                                    let row_key_view = row_key.clone();
                                                                                                    let tip = format!("Click to copy: {full_lbl}");
                                                                                                    view! {
                                                                                                        <button
                                                                                                            type="button"
                                                                                                            data-tip=tip
                                                                                                            class="group inline-flex items-center gap-1.5 text-left text-white/80 hover:text-white truncate min-w-0 cursor-pointer transition-colors"
                                                                                                            on:click=move |_| {
                                                                                                                #[cfg(feature = "hydrate")]
                                                                                                                {
                                                                                                                    if let Some(win) = leptos::web_sys::window() {
                                                                                                                        let _ = win.navigator().clipboard().write_text(&copy_str);
                                                                                                                        let k = row_key_click.clone();
                                                                                                                        set_copied_io.set(Some(k.clone()));
                                                                                                                        set_timeout(
                                                                                                                            move || {
                                                                                                                                let _ = set_copied_io.try_update(|cur| {
                                                                                                                                    if cur.as_deref() == Some(k.as_str()) {
                                                                                                                                        *cur = None;
                                                                                                                                    }
                                                                                                                                });
                                                                                                                            },
                                                                                                                            std::time::Duration::from_millis(1500),
                                                                                                                        );
                                                                                                                    }
                                                                                                                }
                                                                                                                #[cfg(not(feature = "hydrate"))]
                                                                                                                {
                                                                                                                    let _ = (&copy_str, &row_key_click);
                                                                                                                }
                                                                                                            }
                                                                                                        >
                                                                                                            <span class="truncate">{short_lbl}</span>
                                                                                                            {move || {
                                                                                                                if copied_io.get().as_deref() == Some(row_key_view.as_str()) {
                                                                                                                    view! {
                                                                                                                        <span class="text-[9px] font-semibold text-[#00e676] shrink-0">"Copied"</span>
                                                                                                                    }.into_any()
                                                                                                                } else {
                                                                                                                    view! {
                                                                                                                        <svg class="w-3 h-3 shrink-0 text-white/65 group-hover:text-white transition-colors" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                                                                                                            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                                                                                                                            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                                                                                                                        </svg>
                                                                                                                    }.into_any()
                                                                                                                }
                                                                                                            }}
                                                                                                        </button>
                                                                                                    }.into_any()
                                                                                                } else {
                                                                                                    view! {
                                                                                                        <span class="text-white/80 truncate min-w-0" title=full_lbl>{short_lbl}</span>
                                                                                                    }.into_any()
                                                                                                }}
                                                                                                <span class="text-white/95 font-medium tabular-nums shrink-0">{amt}</span>
                                                                                            </div>
                                                                                        }
                                                                                    }).collect_view()}
                                                                                    {(extra_in > 0).then(|| view! {
                                                                                        <div class="text-[10px] text-white/55 pt-0.5">
                                                                                            {format!("+ {} more inputs (see mempool.space)", super::helpers::format_number(extra_in))}
                                                                                        </div>
                                                                                    })}
                                                                                </div>
                                                                            })}
                                                                            {(!out_items.is_empty()).then(|| view! {
                                                                                <div class="space-y-1">
                                                                                    <div class="text-[10px] text-white/85 font-semibold uppercase tracking-wider">
                                                                                        {format!("Outputs ({})", super::helpers::format_number(total_out))}
                                                                                    </div>
                                                                                    {out_items.iter().enumerate().map(|(idx, entry)| {
                                                                                        let short_lbl = short_io_label(&entry.label);
                                                                                        let amt = entry
                                                                                            .value
                                                                                            .map(|s| format!("{:.8} BTC", s as f64 / 1e8))
                                                                                            .unwrap_or_else(|| "--".to_string());
                                                                                        let row_key = format!("out:{idx}");
                                                                                        let copy_val = entry.copy.clone();
                                                                                        let full_lbl = copy_val.clone().unwrap_or_else(|| entry.label.clone());
                                                                                        view! {
                                                                                            <div class="flex items-center justify-between gap-2 py-0.5 border-b border-white/[0.05] last:border-b-0">
                                                                                                {if let Some(copy_str) = copy_val {
                                                                                                    let row_key_click = row_key.clone();
                                                                                                    let row_key_view = row_key.clone();
                                                                                                    let tip = format!("Click to copy: {full_lbl}");
                                                                                                    view! {
                                                                                                        <button
                                                                                                            type="button"
                                                                                                            data-tip=tip
                                                                                                            class="group inline-flex items-center gap-1.5 text-left text-white/80 hover:text-white truncate min-w-0 cursor-pointer transition-colors"
                                                                                                            on:click=move |_| {
                                                                                                                #[cfg(feature = "hydrate")]
                                                                                                                {
                                                                                                                    if let Some(win) = leptos::web_sys::window() {
                                                                                                                        let _ = win.navigator().clipboard().write_text(&copy_str);
                                                                                                                        let k = row_key_click.clone();
                                                                                                                        set_copied_io.set(Some(k.clone()));
                                                                                                                        set_timeout(
                                                                                                                            move || {
                                                                                                                                let _ = set_copied_io.try_update(|cur| {
                                                                                                                                    if cur.as_deref() == Some(k.as_str()) {
                                                                                                                                        *cur = None;
                                                                                                                                    }
                                                                                                                                });
                                                                                                                            },
                                                                                                                            std::time::Duration::from_millis(1500),
                                                                                                                        );
                                                                                                                    }
                                                                                                                }
                                                                                                                #[cfg(not(feature = "hydrate"))]
                                                                                                                {
                                                                                                                    let _ = (&copy_str, &row_key_click);
                                                                                                                }
                                                                                                            }
                                                                                                        >
                                                                                                            <span class="truncate">{short_lbl}</span>
                                                                                                            {move || {
                                                                                                                if copied_io.get().as_deref() == Some(row_key_view.as_str()) {
                                                                                                                    view! {
                                                                                                                        <span class="text-[9px] font-semibold text-[#00e676] shrink-0">"Copied"</span>
                                                                                                                    }.into_any()
                                                                                                                } else {
                                                                                                                    view! {
                                                                                                                        <svg class="w-3 h-3 shrink-0 text-white/65 group-hover:text-white transition-colors" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                                                                                                            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                                                                                                                            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                                                                                                                        </svg>
                                                                                                                    }.into_any()
                                                                                                                }
                                                                                                            }}
                                                                                                        </button>
                                                                                                    }.into_any()
                                                                                                } else {
                                                                                                    view! {
                                                                                                        <span class="text-white/80 truncate min-w-0" title=full_lbl>{short_lbl}</span>
                                                                                                    }.into_any()
                                                                                                }}
                                                                                                <span class="text-white/95 font-medium tabular-nums shrink-0">{amt}</span>
                                                                                            </div>
                                                                                        }
                                                                                    }).collect_view()}
                                                                                    {(extra_out > 0).then(|| view! {
                                                                                        <div class="text-[10px] text-white/55 pt-0.5">
                                                                                            {format!("+ {} more outputs (see mempool.space)", super::helpers::format_number(extra_out))}
                                                                                        </div>
                                                                                    })}
                                                                                </div>
                                                                            })}
                                                                    </div>
                                                                </div>
                                                            }
                                                        })}
                                                    </div>
                                                }.into_any()
                                            } else if deep_loading {
                                                view! {
                                                    <div class="pt-1.5 border-t border-white/10 text-[10px] lg:text-xs text-white/65">
                                                        "Loading inputs, outputs, weight..."
                                                    </div>
                                                }.into_any()
                                            } else {
                                                view! { <div></div> }.into_any()
                                            }}
                                        </div>

                                        // Footer actions
                                        <div class="flex flex-wrap items-center justify-between gap-2 px-3.5 py-2 lg:px-4 lg:py-2.5 border-t border-white/10 bg-white/[0.02]">
                                            <div class="flex items-center gap-2">
                                                {match status_for_footer {
                                                    PinnedTxStatus::Unconfirmed => view! {
                                                        <button
                                                            type="button"
                                                            class="px-2.5 py-1 lg:px-3 lg:py-1.5 rounded-md bg-[#00e676]/15 hover:bg-[#00e676]/25 border border-[#00e676]/40 text-[11px] lg:text-xs 2xl:text-sm font-semibold text-[#00e676] cursor-pointer transition-colors"
                                                            on:click=move |_| {
                                                                let focused = hb_v2_focus_tx(&txid_locate);
                                                                if !focused {
                                                                    set_search_feedback.set(Some((
                                                                        false,
                                                                        "In node mempool (outside rendered window)".to_string(),
                                                                    )));
                                                                }
                                                            }
                                                        >
                                                            "Locate brick"
                                                        </button>
                                                    }.into_any(),
                                                    PinnedTxStatus::Confirmed { block_height, .. } if block_height > 0 => view! {
                                                        <button
                                                            type="button"
                                                            class="px-2.5 py-1 lg:px-3 lg:py-1.5 rounded-md bg-[#f7931a]/15 hover:bg-[#f7931a]/25 border border-[#f7931a]/40 text-[11px] lg:text-xs 2xl:text-sm font-semibold text-[#f7931a] cursor-pointer transition-colors"
                                                            on:click=move |_| {
                                                                #[cfg(feature = "hydrate")]
                                                                {
                                                                    let client = hb_client.try_with_value(|c| c.clone()).flatten();
                                                                    if let Some(c) = client {
                                                                        c.view_block_height(block_height);
                                                                        return;
                                                                    }
                                                                }
                                                                let _ = hb_v2_focus_block_height(block_height);
                                                            }
                                                        >
                                                            "View block"
                                                        </button>
                                                        <button
                                                            type="button"
                                                            class="px-2.5 py-1 lg:px-3 lg:py-1.5 rounded-md bg-white/5 hover:bg-white/15 border border-white/15 text-[11px] lg:text-xs 2xl:text-sm font-semibold text-white/75 cursor-pointer transition-colors"
                                                            on:click=move |_| show_block_detail_js(block_height)
                                                        >
                                                            "Details"
                                                        </button>
                                                    }.into_any(),
                                                    _ => view! { <span></span> }.into_any(),
                                                }}
                                                <button
                                                    type="button"
                                                    class="px-2.5 py-1 lg:px-3 lg:py-1.5 rounded-md bg-white/5 hover:bg-white/15 border border-white/15 text-[11px] lg:text-xs 2xl:text-sm font-semibold text-white/75 cursor-pointer transition-colors"
                                                    on:click=move |_| {
                                                        #[cfg(feature = "hydrate")]
                                                        {
                                                            if let Some(win) = leptos::web_sys::window() {
                                                                let origin = win
                                                                    .location()
                                                                    .origin()
                                                                    .unwrap_or_else(|_| "https://www.wehodlbtc.com".to_string());
                                                                let url = format!("{origin}/observatory/heartbeat?tx={txid_share}");
                                                                let _ = win.navigator().clipboard().write_text(&url);
                                                                set_copied_tx_link.set(true);
                                                                leptos::prelude::set_timeout(
                                                                    move || {
                                                                        let _ = set_copied_tx_link.try_set(false);
                                                                    },
                                                                    std::time::Duration::from_millis(1500),
                                                                );
                                                            }
                                                        }
                                                    }
                                                >
                                                    {move || if copied_tx_link.get() { "Link copied" } else { "Copy link" }}
                                                </button>
                                            </div>
                                            <a
                                                href=mempool_url
                                                target="_blank"
                                                rel="noopener noreferrer"
                                                class="text-[11px] lg:text-xs text-white/80 hover:text-[#f7931a] transition-colors"
                                            >
                                                "mempool.space \u{2192}"
                                            </a>
                                        </div>
                                    </div>
                                }.into_any()
                            }
                        }
                    </Show>

                    // Loading overlay (initial page load)
                    <Show when=move || loading.get()>
                        <div class="absolute inset-0 flex flex-col items-center justify-center bg-[#0d2137]/95 z-10">
                            <div class="text-4xl animate-bounce" style="animation-duration: 0.8s">
                                "\u{26CF}\u{FE0F}"
                            </div>
                            <p class="mt-3 text-sm text-white/50 font-mono animate-pulse">
                                "Loading chart data..."
                            </p>
                        </div>
                    </Show>
                    // Mining overlay (block being processed, controlled by JS)
                    <div id="heartbeat-mining-overlay" class="absolute inset-0 flex flex-col items-center justify-center bg-[#0d2137]/80 z-10 hidden pointer-events-none">
                        <div class="text-4xl animate-bounce" style="animation-duration: 0.8s">
                            "\u{26CF}\u{FE0F}"
                        </div>
                        <p class="mt-3 text-sm text-white/50 font-mono animate-pulse">
                            "New block found..."
                        </p>
                    </div>
                    // On-demand guide & shortcuts overlay (opened by the (i) button
                    // in the control dock; always in SSR HTML with `hidden` toggled
                    // so click-to-dismiss hydrates cleanly without blocking first visit).
                    <div
                        id="heartbeat-hint-overlay"
                        class=move || {
                            if show_hint.get() {
                                "absolute inset-0 z-20 flex items-center justify-center bg-[#0d2137]/90 cursor-pointer"
                            } else {
                                "hidden"
                            }
                        }
                        on:click=move |_| set_show_hint.set(false)
                    >
                            <div class="max-w-md w-full mx-4 bg-[#0a1a2e] border border-white/15 rounded-xl p-5 sm:p-6 shadow-2xl space-y-4 text-center">
                                <p class="text-white/90 text-sm sm:text-base leading-relaxed">
                                    "Each "
                                    <span class="text-[#f7931a] font-semibold">"spike"</span>
                                    " is a block. Each "
                                    <span class="text-[#f7931a] font-semibold">"brick"</span>
                                    " is a transaction."
                                </p>
                                <div class="text-white/55 text-xs sm:text-sm space-y-1.5">
                                    <p>"Drag to scroll through history"</p>
                                    <p>"Scroll wheel or pinch to zoom"</p>
                                    <p>"Click a spike for block details"</p>
                                    <p>"Click a brick for transaction info"</p>
                                </div>
                                <div class="hidden sm:block pt-3 border-t border-white/10 text-left space-y-2">
                                    <div class="text-[10px] font-mono uppercase tracking-wider text-white/35 text-center">"Keyboard shortcuts"</div>
                                    <div class="grid grid-cols-2 gap-x-6 gap-y-2 text-[11px] font-mono text-white/60">
                                        <div class="flex items-center justify-between gap-2 whitespace-nowrap">
                                            <span>"Pause / play"</span>
                                            <kbd class="px-1.5 py-0.5 rounded bg-white/10 border border-white/10 text-[10px] text-white/85">"Space"</kbd>
                                        </div>
                                        <div class="flex items-center justify-between gap-2 whitespace-nowrap">
                                            <span>"Follow LIVE"</span>
                                            <kbd class="px-1.5 py-0.5 rounded bg-white/10 border border-white/10 text-[10px] text-white/85">"L"</kbd>
                                        </div>
                                        <div class="flex items-center justify-between gap-2 whitespace-nowrap">
                                            <span>"Step blocks"</span>
                                            <span class="inline-flex items-center gap-1">
                                                <kbd class="px-1.5 py-0.5 rounded bg-white/10 border border-white/10 text-[10px] text-white/85">"\u{2190}"</kbd>
                                                <kbd class="px-1.5 py-0.5 rounded bg-white/10 border border-white/10 text-[10px] text-white/85">"\u{2192}"</kbd>
                                            </span>
                                        </div>
                                        <div class="flex items-center justify-between gap-2 whitespace-nowrap">
                                            <span>"Fit view"</span>
                                            <kbd class="px-1.5 py-0.5 rounded bg-white/10 border border-white/10 text-[10px] text-white/85">"F"</kbd>
                                        </div>
                                        <div class="flex items-center justify-between gap-2 whitespace-nowrap">
                                            <span>"Search"</span>
                                            <kbd class="px-1.5 py-0.5 rounded bg-white/10 border border-white/10 text-[10px] text-white/85">"/"</kbd>
                                        </div>
                                        <div class="flex items-center justify-between gap-2 whitespace-nowrap">
                                            <span>"Dismiss"</span>
                                            <kbd class="px-1.5 py-0.5 rounded bg-white/10 border border-white/10 text-[10px] text-white/85">"Esc"</kbd>
                                        </div>
                                    </div>
                                </div>
                                <p class="text-white/30 text-[10px] sm:text-xs pt-2 border-t border-white/10">"Tap anywhere to dismiss"</p>
                            </div>
                        </div>

                </div>

                // Unified bottom bar:
                // - lg+: 1 sleek row (mempool + tx/s | compact control dock | Next block fee)
                // - <lg: 1 clean single-row control strip + slim mempool/fee strip below
                <div class="flex flex-col lg:flex-row lg:items-center lg:justify-between gap-1.5 lg:gap-3 px-2.5 sm:px-3.5 py-1.5 border-t border-white/10 bg-[#0a1929]/65">
                    // Mempool total + prominent live 60s tx/s badge (left on lg+, row 2 left on <lg)
                    <div class="order-2 lg:order-1 flex items-center justify-between lg:justify-start gap-2 text-xs sm:text-sm text-[#00e676] font-mono tabular-nums">
                        <span class="whitespace-nowrap inline-flex items-center gap-2">
                            <span>{mempool_display}</span>
                            {move || format_arrival_rate(arrival_rate.get()).map(|rate_str| view! {
                                <span
                                    class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md bg-[#38bdf8]/15 border border-[#38bdf8]/45 text-xs sm:text-sm font-bold text-[#7dd3fc] shadow-sm"
                                    data-tip="Rolling 60-second live mempool admission rate observed by my node."
                                >
                                    <span class="w-1.5 h-1.5 rounded-full bg-[#38bdf8] animate-pulse shrink-0"></span>
                                    <span>{rate_str}</span>
                                </span>
                            })}
                        </span>
                        <span class="lg:hidden whitespace-nowrap">
                            <span class="text-white/75">"Next: "</span>
                            {fee_display}
                        </span>
                    </div>

                    // Single-row compact control dock (fits cleanly on 360px+ phones with zero orphan wrap)
                    <div class="order-1 lg:order-2 flex items-center justify-center">
                        <div class="flex items-center justify-center gap-1 sm:gap-1.5 max-w-full overflow-x-auto [scrollbar-width:none] sm:overflow-visible rounded-lg bg-[#071320]/90 border border-white/10 px-1.5 sm:px-2 py-1 shadow-md">
                            // Zoom cluster
                            <div class="flex items-center gap-0.5 rounded-md bg-white/[0.04] border border-white/[0.06] p-0.5">
                                <button aria-label="Zoom out" onclick="handleControlClick('zoomOut')"
                                    class="group relative w-6 h-6 sm:w-7 sm:h-7 shrink-0 rounded text-white/70 hover:bg-white/10 hover:text-white transition-all cursor-pointer flex items-center justify-center">
                                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2.2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M5 12h14"/>
                                    </svg>
                                    <span class="hidden sm:block pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded-md bg-[#071320] border border-white/15 text-[10px] font-mono text-white/85 whitespace-nowrap shadow-lg opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-20">"Zoom out (-)"</span>
                                </button>
                                <span id="heartbeat-zoom-label" class="hidden sm:inline text-[11px] text-white/60 font-mono tabular-nums w-9 text-center select-none">"1.6x"</span>
                                <button aria-label="Zoom in" onclick="handleControlClick('zoomIn')"
                                    class="group relative w-6 h-6 sm:w-7 sm:h-7 shrink-0 rounded text-white/70 hover:bg-white/10 hover:text-white transition-all cursor-pointer flex items-center justify-center">
                                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2.2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 5v14M5 12h14"/>
                                    </svg>
                                    <span class="hidden sm:block pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded-md bg-[#071320] border border-white/15 text-[10px] font-mono text-white/85 whitespace-nowrap shadow-lg opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-20">"Zoom in (+)"</span>
                                </button>
                            </div>

                            // Playback & block step cluster
                            <div class="flex items-center gap-0.5 rounded-md bg-white/[0.04] border border-white/[0.06] p-0.5">
                                <button aria-label="Previous block" onclick="handleControlClick('prev')"
                                    class="group relative w-6 h-6 sm:w-7 sm:h-7 shrink-0 rounded text-white/70 hover:bg-white/10 hover:text-white transition-all cursor-pointer flex items-center justify-center">
                                    <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
                                        <path d="M6 6h2v12H6V6zm3.5 6l8.5 6V6l-8.5 6z"/>
                                    </svg>
                                    <span class="hidden sm:block pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded-md bg-[#071320] border border-white/15 text-[10px] font-mono text-white/85 whitespace-nowrap shadow-lg opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-20">"Previous block (\u{2190})"</span>
                                </button>
                                <button id="heartbeat-btn-pause" aria-label="Pause or play" aria-pressed="false" onclick="handleControlClick('pause')"
                                    class="group relative w-6 h-6 sm:w-7 sm:h-7 shrink-0 rounded bg-[#f7931a]/15 text-[#f7931a] hover:bg-[#f7931a]/25 transition-all cursor-pointer flex items-center justify-center">
                                    <svg id="heartbeat-pause-svg" class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
                                        <path d="M7 5h3.5v14H7V5zm6.5 0H17v14h-3.5V5z"/>
                                    </svg>
                                    <svg id="heartbeat-play-svg" class="w-3.5 h-3.5 hidden" fill="currentColor" viewBox="0 0 24 24">
                                        <path d="M8 5.14v13.72L19 12 8 5.14z"/>
                                    </svg>
                                    <span class="hidden sm:block pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded-md bg-[#071320] border border-white/15 text-[10px] font-mono text-white/85 whitespace-nowrap shadow-lg opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-20">"Pause / play (Space)"</span>
                                </button>
                                <button aria-label="Next block" onclick="handleControlClick('next')"
                                    class="group relative w-6 h-6 sm:w-7 sm:h-7 shrink-0 rounded text-white/70 hover:bg-white/10 hover:text-white transition-all cursor-pointer flex items-center justify-center">
                                    <svg class="w-3.5 h-3.5" fill="currentColor" viewBox="0 0 24 24">
                                        <path d="M16 6h2v12h-2V6zM6 18l8.5-6L6 6v12z"/>
                                    </svg>
                                    <span class="hidden sm:block pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded-md bg-[#071320] border border-white/15 text-[10px] font-mono text-white/85 whitespace-nowrap shadow-lg opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-20">"Next block (\u{2192})"</span>
                                </button>
                            </div>

                            // Primary LIVE pill + Fit
                            <div class="flex items-center gap-1">
                                <button id="heartbeat-btn-live" aria-label="Follow the live head" onclick="handleControlClick('live')"
                                    class="group relative flex items-center gap-1 h-7 px-2 sm:px-2.5 shrink-0 rounded-md border text-[10px] sm:text-[11px] font-mono font-bold tracking-wider cursor-pointer transition-all"
                                    style="color:#00e676;background:rgba(0,230,118,0.12);border-color:rgba(0,230,118,0.4)">
                                    <span id="heartbeat-live-dot" class="w-1.5 h-1.5 rounded-full" style="background:#00e676"></span>
                                    "LIVE"
                                    <span class="hidden sm:block pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded-md bg-[#071320] border border-white/15 text-[10px] font-mono font-normal tracking-normal text-white/85 whitespace-nowrap shadow-lg opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-20">"Follow live head (L)"</span>
                                </button>
                                <button aria-label="Fit whole mempool" onclick="handleControlClick('fit')"
                                    class="group relative w-7 h-7 shrink-0 rounded-md bg-white/[0.04] border border-white/[0.06] text-white/70 hover:bg-white/10 hover:text-white transition-all cursor-pointer flex items-center justify-center">
                                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M4 8V5a1 1 0 0 1 1-1h3m8 0h3a1 1 0 0 1 1 1v3m0 8v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"/>
                                    </svg>
                                    <span class="hidden sm:block pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded-md bg-[#071320] border border-white/15 text-[10px] font-mono text-white/85 whitespace-nowrap shadow-lg opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-20">"Fit view (F)"</span>
                                </button>
                            </div>

                            // Utility cluster: Sound & Help
                            <div class="flex items-center gap-0.5 rounded-md bg-white/[0.04] border border-white/[0.06] p-0.5">
                                <button
                                    id="heartbeat-btn-sound"
                                    aria-label="Sound on a new block"
                                    aria-pressed="true"
                                    title="Block sound on (M)"
                                    onclick="handleControlClick('sound')"
                                    class="group relative w-6 h-6 sm:w-7 sm:h-7 shrink-0 rounded text-white/70 hover:bg-white/10 hover:text-white transition-all cursor-pointer flex items-center justify-center"
                                    style="color:#00e676;background:rgba(0,230,118,0.14)"
                                >
                                    <svg id="heartbeat-sound-off-svg" class="w-3.5 h-3.5 hidden" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M11 5L6 9H3v6h3l5 4V5zM22 9l-6 6m0-6l6 6"/>
                                    </svg>
                                    <svg id="heartbeat-sound-on-svg" class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M11 5L6 9H3v6h3l5 4V5zm4.5 3.5a5 5 0 0 1 0 7m2.8-9.8a9 9 0 0 1 0 12.6"/>
                                    </svg>
                                    <span class="hidden sm:block pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded-md bg-[#071320] border border-white/15 text-[10px] font-mono text-white/85 whitespace-nowrap shadow-lg opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-20">"Block sound (M)"</span>
                                </button>
                                <button aria-label="How to read this and keyboard shortcuts" title="Guide & shortcuts"
                                    on:click=move |_| set_show_hint.set(true)
                                    class="group relative w-6 h-6 sm:w-7 sm:h-7 shrink-0 rounded text-white/70 hover:bg-white/10 hover:text-white transition-all cursor-pointer flex items-center justify-center">
                                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                        <circle cx="12" cy="12" r="9"/>
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 11v5m0-8h.01"/>
                                    </svg>
                                    <span class="hidden sm:block pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded-md bg-[#071320] border border-white/15 text-[10px] font-mono text-white/85 whitespace-nowrap shadow-lg opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-20">"Guide & shortcuts"</span>
                                </button>
                            </div>
                        </div>
                    </div>

                    // Desktop next-block fee (right on lg+)
                    <span class="hidden lg:inline order-3 whitespace-nowrap text-xs lg:text-sm text-[#00e676] font-mono tabular-nums">
                        <span class="text-white/75">"Next block: "</span>
                        {fee_display}
                    </span>
                </div>
            </div>

            // ── Interactive Fee-Band Spotlight Bar (promoted right below canvas) ──
            <div class="bg-[#0d2137]/80 border border-white/10 rounded-xl px-3 sm:px-4 py-2.5 space-y-2">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <span class="text-[11px] sm:text-xs text-white/85 font-mono font-semibold uppercase tracking-wider">
                        "Fee Bands (sat/vB)"
                    </span>
                    {move || match spotlight_band.get() {
                        Some(active_idx) => {
                            let (name, color) = FEE_BAND_META[active_idx];
                            view! {
                                <button
                                    type="button"
                                    class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[11px] sm:text-xs font-mono font-semibold border cursor-pointer transition-colors hover:bg-white/10"
                                    style=format!("color:{color};border-color:{color}80;background:{color}20")
                                    on:click=move |_| {
                                        set_spotlight_band.set(None);
                                        hb_v2_set_spotlight_band(-1);
                                    }
                                >
                                    <span>{format!("Spotlight: {name}")}</span>
                                    <span class="text-white/85">"\u{00d7}"</span>
                                </button>
                            }.into_any()
                        }
                        None => view! {
                            <span class="text-[11px] sm:text-xs text-white/80 font-mono">
                                "Tap a band to spotlight on canvas"
                            </span>
                        }.into_any(),
                    }}
                </div>
                <div class="flex sm:flex-wrap items-center gap-1.5 overflow-x-auto sm:overflow-visible [scrollbar-width:none] pb-0.5 sm:pb-0 text-[11px] sm:text-xs text-white/95 font-mono">
                    {FEE_BAND_META.iter().enumerate().map(|(idx, &(band_label, band_color))| {
                        view! {
                            <button
                                type="button"
                                aria-pressed=move || if spotlight_band.get() == Some(idx) { "true" } else { "false" }
                                data-tip=move || {
                                    let stat = fee_bands.get().bands[idx];
                                    format_band_chip_tooltip(band_label, stat, spotlight_band.get() == Some(idx))
                                }
                                class=move || {
                                    match spotlight_band.get() {
                                        Some(active) if active == idx => "inline-flex items-center gap-1.5 px-2 py-1 rounded-md border whitespace-nowrap shrink-0 cursor-pointer transition-all shadow-sm",
                                        Some(_) => "inline-flex items-center gap-1.5 px-2 py-1 rounded-md bg-white/[0.03] border border-white/10 whitespace-nowrap shrink-0 cursor-pointer transition-all opacity-45 hover:opacity-85",
                                        None => "inline-flex items-center gap-1.5 px-2 py-1 rounded-md bg-white/[0.05] hover:bg-white/[0.10] border border-white/15 hover:border-white/30 whitespace-nowrap shrink-0 cursor-pointer transition-all",
                                    }
                                }
                                style=move || {
                                    if spotlight_band.get() == Some(idx) {
                                        format!("border-color:{band_color};background:{band_color}22;color:#ffffff")
                                    } else {
                                        String::new()
                                    }
                                }
                                on:click=move |_| {
                                    let next = if spotlight_band.get_untracked() == Some(idx) {
                                        None
                                    } else {
                                        Some(idx)
                                    };
                                    set_spotlight_band.set(next);
                                    hb_v2_set_spotlight_band(next.map(|i| i as i32).unwrap_or(-1));
                                }
                            >
                                <span
                                    class="w-2.5 h-2.5 rounded-sm shrink-0"
                                    style=format!("background-color:{band_color}")
                                ></span>
                                <span class="font-semibold text-white">{band_label}</span>
                                {move || {
                                    let stat = fee_bands.get().bands[idx];
                                    let txt = format_band_chip_metrics(stat);
                                    (!txt.is_empty()).then(|| view! {
                                        <span class="text-[11px] text-white/80 tabular-nums">{txt}</span>
                                    })
                                }}
                            </button>
                        }
                    }).collect_view()}
                </div>
            </div>

            // ── Phase 2: Vital Signs Panel ────────────────────
            <div class="grid grid-cols-2 lg:grid-cols-4 gap-3">
                // Heart Rate
                <VitalTile
                    label="Heart Rate"
                    value=hr_display
                    unit=" avg"
                    status=hr_label
                    color=hr_color
                    subtitle=Signal::derive(move || hr_subtitle.get())
                    tip="Average time between blocks this difficulty period (target: 10:00).\nBelow 10:00 (Tachycardia): faster than target.\nAbove 10:00 (Bradycardia): slower than target.\nDifficulty adjusts every 2,016 blocks."
                />

                // Blood Pressure
                <VitalTile
                    label="Blood Pressure"
                    value=bp_display
                    unit=" sat/vB"
                    status=bp_label
                    color=bp_color
                    tip="Fee pressure from Bitcoin Core estimatesmartfee.\nSystolic (left): next-block target (~10 min).\nDiastolic (right): 6-block target (~1 hour)."
                />

                // Temperature (mempool + node RAM meter)
                <VitalTile
                    label="Temperature"
                    value=temp_display
                    unit=" vMB"
                    status=temp_label
                    color=temp_color
                    subtitle=Signal::derive(move || temp_subtitle.get())
                    meter=Signal::derive(move || format_mempool_ram(cached_live.get().as_ref()))
                    tip="Mempool backlog in virtual megabytes plus my node's RAM usage vs. its maxmempool ceiling.\nWhen RAM fills, nodes evict the lowest-fee transactions and raise their minimum relay fee."
                />

                // Immune System
                <VitalTile
                    label="Immune System"
                    value=immune_display
                    unit=""
                    status=immune_label
                    color=immune_color
                    tip="Estimated network hashrate in exahashes per second (EH/s).\nHigher hashrate increases the physical energy and hardware cost required to attack the chain."
                />
            </div>

            // ── Phase 3: 24-Hour Rhythm Strip ─────────────────
            <div class="bg-[#0d2137] border border-white/10 rounded-2xl overflow-hidden">
                <div class="flex flex-col sm:flex-row sm:items-baseline sm:justify-between px-3 sm:px-4 py-2 gap-0.5 sm:gap-0 border-b border-white/5">
                    <span class="text-xs text-white/80 font-mono font-semibold uppercase tracking-wider">"24-Hour Rhythm Strip"</span>
                    <span class="text-[11px] sm:text-xs text-white/75 font-mono">"Last 144 blocks \u{00b7} one full difficulty day"</span>
                </div>
                <canvas
                    id="rhythm-strip-canvas"
                    class="w-full"
                    style="height: 100px"
                ></canvas>
                // Block scrubber + detail panel
                <div class="px-3 sm:px-4 py-2 border-t border-white/5">
                    <input
                        type="range"
                        id="rhythm-strip-slider"
                        min="0" max="143" value="143"
                        class="w-full h-1 accent-[#00e676] cursor-pointer"
                        style="opacity: 0.75"
                    />
                    <div id="rhythm-strip-detail" class="flex flex-wrap items-baseline gap-x-4 gap-y-0.5 mt-1.5 min-h-[20px] text-xs font-mono text-white/85 tabular-nums">
                    </div>
                </div>
            </div>

            // ── Phase 4: Network Pulse (Rust-computed live telemetry) ──
            <div class="relative overflow-hidden bg-[#0d2137]/80 border border-white/10 rounded-xl flex">
                // Live-colored left diagnostic rail
                <div
                    class="w-1 shrink-0"
                    style=move || format!("background-color: {}", network_pulse.get().color)
                ></div>
                <div class="flex-1 min-w-0 px-3.5 sm:px-5 py-3.5 sm:py-4 flex flex-col lg:flex-row lg:items-center lg:justify-between gap-3 lg:gap-6">
                    <div class="min-w-0 space-y-1.5">
                        <div class="flex flex-wrap items-center gap-2">
                            <span
                                class="w-2 h-2 rounded-full shrink-0"
                                style=move || {
                                    let c = network_pulse.get().color;
                                    format!("background-color: {c}; box-shadow: 0 0 8px {c}88;")
                                }
                            ></span>
                            <span class="text-xs text-white/80 font-mono font-semibold uppercase tracking-wider shrink-0">
                                "Network Pulse"
                            </span>
                            <span
                                class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs sm:text-[13px] font-mono font-semibold border leading-tight"
                                style=move || {
                                    let c = network_pulse.get().color;
                                    format!("color: {c}; background-color: {c}1a; border-color: {c}55;")
                                }
                            >
                                {move || network_pulse.get().headline}
                            </span>
                        </div>
                        <p class="text-xs sm:text-sm text-white/85 leading-relaxed">
                            {move || network_pulse.get().summary}
                        </p>
                    </div>
                    <div class="grid grid-cols-3 gap-2 lg:shrink-0 lg:w-[390px]">
                        <div class="rounded-lg bg-[#071320]/70 border border-white/10 px-2.5 sm:px-3 py-2 flex flex-col gap-0.5 text-center sm:text-left min-w-0">
                            <span class="text-[10px] sm:text-[11px] font-mono font-medium uppercase tracking-wider text-white/75 truncate">
                                "Depth"
                            </span>
                            <span class="text-xs sm:text-sm font-mono font-semibold text-white tabular-nums truncate">
                                {move || network_pulse.get().depth_pill}
                            </span>
                        </div>
                        <div class="rounded-lg bg-[#071320]/70 border border-white/10 px-2.5 sm:px-3 py-2 flex flex-col gap-0.5 text-center sm:text-left min-w-0">
                            <span class="text-[10px] sm:text-[11px] font-mono font-medium uppercase tracking-wider text-white/75 truncate">
                                "20-Blk Avg"
                            </span>
                            <span class="text-xs sm:text-sm font-mono font-semibold text-white tabular-nums truncate">
                                {move || network_pulse.get().cadence_pill}
                            </span>
                        </div>
                        <div class="rounded-lg bg-[#071320]/70 border border-white/10 px-2.5 sm:px-3 py-2 flex flex-col gap-0.5 text-center sm:text-left min-w-0">
                            <span class="text-[10px] sm:text-[11px] font-mono font-medium uppercase tracking-wider text-white/75 truncate">
                                "Next Block"
                            </span>
                            <span class="text-xs sm:text-sm font-mono font-semibold text-white tabular-nums truncate">
                                {move || network_pulse.get().fee_pill}
                            </span>
                        </div>
                    </div>
                </div>
            </div>

            // EKG Wait Interval Legend (continuous 7-stop progressive wait ramp)
            <div class="bg-[#0d2137]/60 border border-white/10 rounded-xl p-4 sm:p-5 space-y-2.5">
                <div class="flex flex-wrap items-center justify-between gap-2">
                    <span class="text-xs text-white/80 font-mono font-semibold uppercase tracking-wider">"EKG Wait Interval"</span>
                    <span class="text-[11px] sm:text-xs text-white/75 font-mono">"Larger outlined bricks highlight notable transactions"</span>
                </div>
                <div class="space-y-1.5">
                    <div
                        class="h-2.5 w-full rounded-full border border-white/10"
                        style="background: linear-gradient(to right, #00e676 0%, #38bdf8 17%, #facc15 33%, #f97316 50%, #ff4500 67%, #dc2626 83%, #c026d3 100%)"
                    ></div>
                    <div class="grid grid-cols-7 gap-0.5 text-center text-[10px] sm:text-xs font-mono text-white/90 tabular-nums">
                        <span class="text-[#00e676]">"0\u{2013}5m"</span>
                        <span class="text-[#38bdf8]">"10m"</span>
                        <span class="text-[#facc15]">"20m"</span>
                        <span class="text-[#f97316]">"30m"</span>
                        <span class="text-[#ff6a33]">"45m"</span>
                        <span class="text-[#f87171]">"60m"</span>
                        <span class="text-[#e879f9]">"90m+"</span>
                    </div>
                </div>
                <p class="text-xs text-white/80 leading-relaxed">
                    "Flatline and spike color shifts continuously along this 7-stop wait ramp as minutes pass between blocks, keeping long droughts visually distinct."
                </p>
            </div>

            // Historic Landmarks browser modal (portaled to <body> so ObservatoryPage's
            // forwards-filled fade animation stacking context cannot trap it under the navbar)
            <Portal>
                <Show when=move || show_landmarks.get()>
                    <div
                        id="heartbeat-landmarks-modal"
                        role="dialog"
                        aria-modal="true"
                        aria-label="Landmark Blocks"
                        class="fixed inset-0 flex items-end sm:items-center justify-center sm:p-4"
                        style="z-index: 10004"
                    >
                        // Backdrop
                        <div
                            class="absolute inset-0 bg-black/70 backdrop-blur-sm"
                            on:click=move |_| set_show_landmarks.set(false)
                        ></div>

                        // Modal surface
                        <div class="relative w-full sm:max-w-2xl max-h-[85vh] sm:max-h-[80vh] flex flex-col bg-[#0a1929] border border-white/15 rounded-t-2xl sm:rounded-2xl shadow-2xl overflow-hidden">
                            // Header + filter + category tabs
                            <div class="px-4 sm:px-5 pt-4 pb-3 border-b border-white/10 bg-[#071320]/90 space-y-3 shrink-0">
                                <div class="flex items-start justify-between gap-3">
                                    <div class="min-w-0">
                                        <h2 class="text-sm sm:text-base font-semibold text-white">
                                            "Landmark Blocks"
                                        </h2>
                                        <p class="text-xs text-white/80 mt-0.5">
                                            "Jump the Heartbeat EKG to any curated block from The Archives."
                                        </p>
                                    </div>
                                    <button
                                        id="heartbeat-landmarks-close"
                                        type="button"
                                        aria-label="Close landmark blocks browser"
                                        class="w-7 h-7 rounded-lg bg-white/10 hover:bg-white/20 border border-white/15 text-white/80 hover:text-white flex items-center justify-center cursor-pointer transition-colors shrink-0"
                                        on:click=move |_| set_show_landmarks.set(false)
                                    >
                                        "\u{2715}"
                                    </button>
                                </div>

                                // Featured quick-jump highlights inside the modal
                                <div class="flex items-center gap-1.5 overflow-x-auto [scrollbar-width:none] pb-0.5">
                                    <span class="text-[11px] font-mono font-semibold text-white/80 uppercase tracking-wider shrink-0">"Featured:"</span>
                                    {[
                                        ("Genesis", 0u64, "#0", "Genesis block (Jan 3, 2009)"),
                                        ("Pizza", 57_043, "#57,043", "Bitcoin Pizza Day block (May 22, 2010)"),
                                        ("Highest Fee", 409_008, "#409,008", "All-time highest fee block (291.53 BTC, Apr 26, 2016)"),
                                        ("SegWit", 481_824, "#481,824", "SegWit activation block (Aug 24, 2017)"),
                                        ("Taproot", 709_632, "#709,632", "Taproot activation block (Nov 14, 2021)"),
                                        ("Largest", 836_964, "#836,964", "Largest block by raw size (3.99 MB, Mar 30, 2024)"),
                                        ("4th Halving", 840_000, "#840,000", "4th Halving & Runes launch (37.63 BTC fees, Apr 20, 2024)"),
                                    ]
                                    .into_iter()
                                    .map(|(name, height, tag, tip)| {
                                        let q_str = format!("#{height}");
                                        view! {
                                            <button
                                                type="button"
                                                data-tip=tip
                                                class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-white/[0.06] hover:bg-[#f7931a]/20 border border-white/15 hover:border-[#f7931a]/60 text-[10px] font-mono text-white/90 hover:text-white whitespace-nowrap shrink-0 cursor-pointer transition-colors"
                                                on:click=move |_| {
                                                    set_show_landmarks.set(false);
                                                    set_search_query.set(q_str.clone());
                                                    set_search_feedback.set(None);
                                                    #[cfg(feature = "hydrate")]
                                                    {
                                                        let client = hb_client.try_with_value(|c| c.clone()).flatten();
                                                        if let Some(c) = client {
                                                            c.search(&q_str);
                                                        }
                                                    }
                                                }
                                            >
                                                <span>{name}</span>
                                                <span class="text-[#f7931a]/85">{tag}</span>
                                            </button>
                                        }
                                    })
                                    .collect_view()}
                                </div>

                                // Instant text filter
                                <div class="relative">
                                    <input
                                        type="text"
                                        id="heartbeat-landmarks-filter"
                                        placeholder="Filter by event, year, category, or block height..."
                                        class="w-full pl-3 pr-8 py-1.5 rounded-lg bg-[#0d2137] border border-white/20 text-xs sm:text-sm font-mono text-white placeholder-white/60 focus:outline-none focus:border-[#f7931a]/60"
                                        prop:value=move || landmark_filter.get()
                                        on:input=move |ev| set_landmark_filter.set(event_target_value(&ev))
                                        on:keydown=move |ev| {
                                            if ev.key() == "Escape" {
                                                set_show_landmarks.set(false);
                                            }
                                        }
                                    />
                                    <Show when=move || !landmark_filter.get().is_empty()>
                                        <button
                                            type="button"
                                            aria-label="Clear filter"
                                            class="absolute right-2 top-1/2 -translate-y-1/2 text-white/45 hover:text-white text-xs cursor-pointer px-1"
                                            on:click=move |_| set_landmark_filter.set(String::new())
                                        >
                                            "\u{2715}"
                                        </button>
                                    </Show>
                                </div>

                                // Category filter pills
                                <div class="flex flex-wrap items-center gap-1.5">
                                    {move || {
                                        let all = heartbeat_landmarks();
                                        let active_cat = landmark_cat.get();
                                        let cats: [(Option<HofCategory>, &str, &str); 6] = [
                                            (None, "All", "#f7931a"),
                                            (Some(HofCategory::Milestones), HofCategory::Milestones.label(), HofCategory::Milestones.color()),
                                            (Some(HofCategory::Records), HofCategory::Records.label(), HofCategory::Records.color()),
                                            (Some(HofCategory::Protocol), HofCategory::Protocol.label(), HofCategory::Protocol.color()),
                                            (Some(HofCategory::Attacks), HofCategory::Attacks.label(), HofCategory::Attacks.color()),
                                            (Some(HofCategory::Oddities), HofCategory::Oddities.label(), HofCategory::Oddities.color()),
                                        ];
                                        cats.into_iter().map(|(cat_opt, label, color)| {
                                            let count = all.iter().filter(|item| cat_opt.is_none_or(|c| item.category == c)).count();
                                            let is_active = active_cat == cat_opt;
                                            let style_str = if is_active {
                                                format!("color: {color}; background-color: {color}22; border-color: {color}66;")
                                            } else {
                                                String::new()
                                            };
                                            let cls = if is_active {
                                                "inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-mono font-semibold border cursor-pointer transition-colors"
                                            } else {
                                                "inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-mono text-white/85 hover:text-white bg-white/[0.05] hover:bg-white/10 border border-white/15 cursor-pointer transition-colors"
                                            };
                                            view! {
                                                <button
                                                    type="button"
                                                    class=cls
                                                    style=style_str
                                                    on:click=move |_| set_landmark_cat.set(cat_opt)
                                                >
                                                    <span>{label}</span>
                                                    <span class="opacity-85 tabular-nums">{format!("({count})")}</span>
                                                </button>
                                            }
                                        }).collect_view()
                                    }}
                                </div>
                            </div>

                            // Scrollable landmark cards
                            <div class="flex-1 overflow-y-auto p-3.5 sm:p-4">
                                {move || {
                                    let cat = landmark_cat.get();
                                    let q = landmark_filter.get();
                                    let items: Vec<HeartbeatLandmark> = heartbeat_landmarks()
                                        .into_iter()
                                        .filter(|item| landmark_matches(item, cat, &q))
                                        .collect();

                                    if items.is_empty() {
                                        view! {
                                            <div class="py-10 text-center space-y-2">
                                                <p class="text-xs sm:text-sm text-white/80 font-mono">
                                                    "No landmark blocks match that filter."
                                                </p>
                                                <button
                                                    type="button"
                                                    class="px-3 py-1 rounded-lg bg-white/10 hover:bg-white/15 border border-white/15 text-xs font-mono text-white/80 cursor-pointer transition-colors"
                                                    on:click=move |_| {
                                                        set_landmark_cat.set(None);
                                                        set_landmark_filter.set(String::new());
                                                    }
                                                >
                                                    "Reset filter"
                                                </button>
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                                                {items.into_iter().map(|item| {
                                                    let height = item.height;
                                                    let q_str = format!("#{height}");
                                                    let height_tag = format!("#{}", super::helpers::format_number(height));
                                                    let cat_color = item.category.color();
                                                    let cat_label = item.category.label();
                                                    view! {
                                                        <button
                                                            type="button"
                                                            class="group text-left p-3 rounded-xl bg-white/[0.04] hover:bg-[#f7931a]/12 border border-white/10 hover:border-[#f7931a]/50 transition-all cursor-pointer flex flex-col gap-1.5"
                                                            on:click=move |_| {
                                                                set_show_landmarks.set(false);
                                                                set_search_query.set(q_str.clone());
                                                                set_search_feedback.set(None);
                                                                #[cfg(feature = "hydrate")]
                                                                {
                                                                    let client = hb_client.try_with_value(|c| c.clone()).flatten();
                                                                    if let Some(c) = client {
                                                                        c.search(&q_str);
                                                                    }
                                                                }
                                                            }
                                                        >
                                                            <div class="flex items-center justify-between gap-2 text-[10px] font-mono">
                                                                <span
                                                                    class="inline-flex items-center gap-1.5 uppercase tracking-wider font-semibold"
                                                                    style=format!("color: {cat_color}")
                                                                >
                                                                    <span
                                                                        class="w-1.5 h-1.5 rounded-full shrink-0"
                                                                        style=format!("background-color: {cat_color}")
                                                                    ></span>
                                                                    {cat_label}
                                                                </span>
                                                                <span class="text-white/80 tabular-nums">{item.date}</span>
                                                            </div>
                                                            <div class="flex items-start justify-between gap-2">
                                                                <span class="text-xs sm:text-[13px] font-semibold text-white leading-snug">
                                                                    {item.title}
                                                                </span>
                                                                <span class="px-1.5 py-0.5 rounded bg-[#f7931a]/15 border border-[#f7931a]/35 text-[10px] sm:text-[11px] font-mono font-semibold text-[#f7931a] tabular-nums shrink-0">
                                                                    {height_tag}
                                                                </span>
                                                            </div>
                                                            <p class="text-xs text-white/80 leading-relaxed line-clamp-2">
                                                                {item.description}
                                                            </p>
                                                        </button>
                                                    }
                                                }).collect_view()}
                                            </div>
                                        }.into_any()
                                    }
                                }}
                            </div>

                            // Footer
                            <div class="px-4 sm:px-5 py-2.5 border-t border-white/10 bg-[#071320]/90 flex flex-wrap items-center justify-between gap-2 text-xs font-mono shrink-0">
                                <span class="text-white/80 tabular-nums">
                                    {move || {
                                        let cat = landmark_cat.get();
                                        let q = landmark_filter.get();
                                        let all = heartbeat_landmarks();
                                        let shown = all.iter().filter(|i| landmark_matches(i, cat, &q)).count();
                                        format!("Showing {shown} of {} curated blocks", all.len())
                                    }}
                                </span>
                                <a
                                    href="/observatory/archives"
                                    class="text-[#f7931a] hover:text-[#ffb347] font-medium transition-colors"
                                >
                                    "Browse full stories in The Archives \u{2192}"
                                </a>
                            </div>
                        </div>
                    </div>
                </Show>
            </Portal>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Vital Signs Tile component
// ---------------------------------------------------------------------------

/// Optional thin progress meter rendered at the bottom of a `VitalTile`
/// (used by the Temperature tile for node mempool RAM usage).
#[derive(Clone, Debug, PartialEq)]
struct TileMeter {
    pct: f64,
    color: &'static str,
    caption: String,
}

#[component]
fn VitalTile(
    label: &'static str,
    #[prop(into)] value: Signal<String>,
    unit: &'static str,
    #[prop(into)] status: Signal<String>,
    color: ReadSignal<String>,
    #[prop(optional, into)] subtitle: Option<Signal<String>>,
    #[prop(optional, into)] meter: Option<Signal<Option<TileMeter>>>,
    #[prop(optional)] tip: Option<&'static str>,
) -> impl IntoView {
    view! {
        <div
            class="bg-[#0d2137] border border-white/10 rounded-xl px-3 sm:px-4 py-2.5 sm:py-3 flex flex-col gap-1 min-w-0"
            data-tip=tip.unwrap_or("")
            tabindex=if tip.is_some() { "0" } else { "-1" }
        >
            <span class="text-xs text-white/80 font-mono font-semibold uppercase tracking-wider">{label}</span>
            <div class="flex flex-wrap items-baseline gap-x-1 gap-y-0">
                <span
                    class="text-lg sm:text-3xl font-mono font-bold tabular-nums"
                    style=move || format!("color: {}", color.get())
                >
                    {value}
                </span>
                <span
                    class="text-xs font-mono font-medium"
                    style=move || format!("color: {}cc", color.get())
                >
                    {unit}
                </span>
            </div>
            <span
                class="text-xs font-mono font-medium truncate"
                style=move || format!("color: {}", color.get())
            >
                {status}
            </span>
            {subtitle.map(|sub| view! {
                <span class="text-xs text-white/80 font-mono">{sub}</span>
            })}
            {meter.map(|m| view! {
                {move || m.get().map(|tm| {
                    let clamped = tm.pct.clamp(0.0, 100.0);
                    let cap_title = tm.caption.clone();
                    view! {
                        <div class="mt-1 space-y-1">
                            <div class="h-1.5 w-full rounded-full bg-white/15 overflow-hidden">
                                <div
                                    class="h-full rounded-full transition-all duration-300"
                                    style=format!("width:{clamped:.1}%;background:{}", tm.color)
                                ></div>
                            </div>
                            <span class="block text-xs text-white/80 font-mono tabular-nums truncate" title=cap_title>
                                {tm.caption}
                            </span>
                        </div>
                    }
                })}
            })}
        </div>
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Fixed fee-band labels and canvas colors ordered identically to
/// `heartbeat_sync::fee_band_index` (`0..=6`).
const FEE_BAND_META: [(&str, &str); 7] = [
    ("Pending", "#64748b"),
    ("< 1", "#2563eb"),
    ("1\u{2013}2", "#38bdf8"),
    ("2\u{2013}5", "#00e676"),
    ("5\u{2013}15", "#f7931a"),
    ("15\u{2013}50", "#ff5722"),
    ("50+", "#f44336"),
];

/// Compact inline metrics for a Fee Bands legend chip (e.g. `12.4k · 4.2 vMB`).
/// Returns empty when zero so empty bands stay tidy.
fn format_band_chip_metrics(stat: FeeBandStat) -> String {
    if stat.tx_count == 0 {
        return String::new();
    }
    let count_str = if stat.tx_count >= 1_000 {
        format!("{:.1}k", stat.tx_count as f64 / 1_000.0)
    } else {
        stat.tx_count.to_string()
    };
    if stat.vbytes >= 100_000 {
        format!(
            "{count_str} \u{00b7} {:.1} vMB",
            stat.vbytes as f64 / 1_000_000.0
        )
    } else if stat.vbytes >= 1_000 {
        format!("{count_str} \u{00b7} {} kvB", (stat.vbytes + 500) / 1_000)
    } else {
        count_str
    }
}

/// Detailed hover tooltip for a Fee Bands legend chip.
fn format_band_chip_tooltip(
    label: &str,
    stat: FeeBandStat,
    is_active: bool,
) -> String {
    let action = if is_active {
        "Click to clear spotlight"
    } else {
        "Click to spotlight on canvas"
    };
    let count = super::helpers::format_number(stat.tx_count);
    let tx_word = if stat.tx_count == 1 { "tx" } else { "txs" };
    if stat.vbytes > 0 {
        let vmb = stat.vbytes as f64 / 1_000_000.0;
        format!("{label} sat/vB: {count} {tx_word} ({vmb:.2} vMB) \u{00b7} {action}")
    } else {
        format!("{label}: {count} {tx_word} \u{00b7} {action}")
    }
}

/// Format the rolling 60s mempool arrival rate (`tx/s`) for the canvas footer.
fn format_arrival_rate(rate: Option<f64>) -> Option<String> {
    let r = rate.filter(|v| v.is_finite() && *v >= 0.0)?;
    if r >= 100.0 {
        Some(format!("{r:.0} tx/s"))
    } else {
        Some(format!("{r:.1} tx/s"))
    }
}

/// Format node mempool RAM usage (`usage / maxmempool`) plus elevated eviction
/// minimum fee (`mempoolminfee`) for the Temperature vital tile.
fn format_mempool_ram(
    live: Option<&crate::stats::types::LiveStats>,
) -> Option<TileMeter> {
    let live = live.filter(|l| l.mempool_available)?;
    let max = live.mempool.maxmempool;
    if max == 0 {
        return None;
    }
    let usage = live.mempool.usage;
    let pct = (usage as f64 / max as f64 * 100.0).clamp(0.0, 100.0);
    let usage_mb = (usage as f64 / 1_000_000.0).round() as u64;
    let max_mb = (max as f64 / 1_000_000.0).round() as u64;
    let color = if pct >= 90.0 {
        "#f44336"
    } else if pct >= 75.0 {
        "#f7931a"
    } else {
        "#38bdf8"
    };
    // Bitcoin Core reports `mempoolminfee` in BTC/kvB (`* 1e5` -> sat/vB).
    let min_sat_vb = live.mempool.mempoolminfee * 100_000.0;
    let caption = if min_sat_vb > 1.05 {
        format!(
            "RAM {usage_mb} / {max_mb} MB ({pct:.0}%) \u{00b7} Min {min_sat_vb:.1} sat/vB"
        )
    } else {
        format!("RAM {usage_mb} / {max_mb} MB ({pct:.0}%)")
    };
    Some(TileMeter {
        pct,
        color,
        caption,
    })
}

/// Formatted node-aggregate mempool figures shared by the bottom info bar and
/// the Temperature vital tile so the two displays can never disagree.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MempoolDisplay {
    bar: String,
    tile_vmb: String,
    tile_tx_label: String,
}

/// Format node-aggregate mempool totals from `LiveStats`.
///
/// Distinguishes a verified empty mempool (`mempool_available == true`,
/// `size == 0`) from an unavailable RPC (`None` or `!mempool_available`) so
/// the UI clears prior positive counts on an empty mempool and never renders
/// a fabricated `0` when `getmempoolinfo` fails.
fn format_mempool_summary(
    live: Option<&crate::stats::types::LiveStats>,
) -> MempoolDisplay {
    match live {
        Some(live) if live.mempool_available => {
            let count = super::helpers::format_number(live.mempool.size);
            let vmb = format!("{:.1}", live.mempool.bytes as f64 / 1_000_000.0);
            MempoolDisplay {
                bar: format!("{count} tx ({vmb} vMB)"),
                tile_vmb: vmb,
                tile_tx_label: format!("{count} tx in mempool"),
            }
        }
        _ => MempoolDisplay {
            bar: "-- tx (-- vMB)".to_string(),
            tile_vmb: "--.-".to_string(),
            tile_tx_label: "-- tx in mempool".to_string(),
        },
    }
}

/// Serialize blocks to JSON for the JS animation engine.
/// Computes exact signed inter-block intervals from consecutive parent timestamps,
/// emitting `null` when no immediate parent block is available (never fabricating 600s).
fn blocks_to_json(blocks: &[crate::stats::types::BlockSummary]) -> String {
    blocks_to_json_with_parent(blocks, None)
}

/// Serialize blocks to JSON, optionally using an immediately preceding `parent` block
/// to compute the exact signed `inter_block_seconds` for `blocks[0]`.
fn blocks_to_json_with_parent(
    blocks: &[crate::stats::types::BlockSummary],
    parent: Option<&crate::stats::types::BlockSummary>,
) -> String {
    use std::fmt::Write;
    let mut buf = String::from("[");
    for (i, b) in blocks.iter().enumerate() {
        let prev = if i > 0 { Some(&blocks[i - 1]) } else { parent };
        let inter: Option<i64> = prev.and_then(|p| {
            (p.height.checked_add(1) == Some(b.height))
                .then(|| b.timestamp as i64 - p.timestamp as i64)
        });
        if i > 0 {
            buf.push(',');
        }
        let hash_json =
            serde_json::to_string(&b.hash).unwrap_or_else(|_| "\"\"".into());
        let miner_json =
            serde_json::to_string(&b.miner).unwrap_or_else(|_| "\"\"".into());
        let _ = write!(
            buf,
            r#"{{"height":{},"hash":{},"timestamp":{},"tx_count":{},"total_fees":{},"size":{},"weight":{},"miner":{},"inter_block_seconds":"#,
            b.height,
            hash_json,
            b.timestamp,
            b.tx_count,
            b.total_fees,
            b.size,
            b.weight,
            miner_json,
        );
        match inter {
            Some(secs) => {
                let _ = write!(buf, "{secs}}}");
            }
            None => {
                buf.push_str("null}");
            }
        }
    }
    buf.push(']');
    buf
}

/// Record a `(height, timestamp)` pair in ascending height order (retracting any
/// orphaned entries at or above `height` on reorg) and keep the newest 25 entries.
fn record_recent_block_pair(
    pairs: &mut Vec<(u64, u64)>,
    height: u64,
    timestamp: u64,
) {
    pairs.retain(|&(h, _)| h < height);
    pairs.push((height, timestamp));
    if pairs.len() > 25 {
        let drop_n = pairs.len() - 25;
        pairs.drain(..drop_n);
    }
}

/// Compute average inter-block interval in seconds over the most recent up to 20
/// consecutive block pairs `(height, timestamp)`.
fn compute_recent_avg_block_secs(pairs: &[(u64, u64)]) -> Option<f64> {
    if pairs.len() < 2 {
        return None;
    }
    let mut total_secs: i64 = 0;
    let mut count: u64 = 0;
    for win in pairs.windows(2).rev() {
        let (h0, t0) = win[0];
        let (h1, t1) = win[1];
        if h0.checked_add(1) == Some(h1) {
            total_secs += t1 as i64 - t0 as i64;
            count += 1;
            if count == 20 {
                break;
            }
        }
    }
    if count == 0 || total_secs <= 0 {
        None
    } else {
        Some(total_secs as f64 / count as f64)
    }
}

/// Rust-computed live Network Pulse diagnostic card view model.
#[derive(Clone, Debug, PartialEq, Eq)]
struct HeartbeatLandmark {
    title: &'static str,
    description: &'static str,
    category: HofCategory,
    date: &'static str,
    height: u64,
}

/// Curated historic blocks derived directly from `HALL_OF_FAME` (the single
/// source of truth shared with The Archives and The Almanac), deduplicated by
/// block height so each famous block appears once under its primary entry.
fn heartbeat_landmarks() -> Vec<HeartbeatLandmark> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for entry in HALL_OF_FAME {
        if let Some(height) = entry.block {
            if seen.insert(height) {
                out.push(HeartbeatLandmark {
                    title: entry.title,
                    description: entry.description,
                    category: entry.category,
                    date: entry.date,
                    height,
                });
            }
        }
    }
    out
}

fn landmarks_to_json() -> String {
    let mut map = serde_json::Map::new();
    for item in heartbeat_landmarks() {
        map.insert(
            item.height.to_string(),
            serde_json::json!({
                "title": item.title,
                "description": item.description,
                "category": item.category.label(),
                "color": item.category.color(),
                "date": item.date,
            }),
        );
    }
    serde_json::Value::Object(map).to_string()
}

/// Case-insensitive filter across category, title, description, date, and block
/// height (matching both `409008` and `#409,008`).
fn landmark_matches(
    item: &HeartbeatLandmark,
    category: Option<HofCategory>,
    query: &str,
) -> bool {
    if let Some(cat) = category {
        if item.category != cat {
            return false;
        }
    }
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return true;
    }
    let digits_q = q.trim_start_matches('#').replace(',', "");
    let height_raw = item.height.to_string();
    let height_fmt = super::helpers::format_number(item.height);
    item.title.to_ascii_lowercase().contains(&q)
        || item.description.to_ascii_lowercase().contains(&q)
        || item.date.to_ascii_lowercase().contains(&q)
        || item.category.label().to_ascii_lowercase().contains(&q)
        || (!digits_q.is_empty() && height_raw.contains(&digits_q))
        || height_fmt.contains(&q)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NetworkPulse {
    headline: &'static str,
    color: &'static str,
    summary: String,
    depth_pill: String,
    cadence_pill: String,
    fee_pill: String,
}

/// Convert Bitcoin Core's `mempoolminfee` (BTC/kvB) into sat/vB, rounded to
/// 0.1 sat/vB with a 0.1 sat/vB floor for any positive non-zero reading.
fn btc_per_kvb_to_sat_per_vb(raw: f64) -> f64 {
    if !raw.is_finite() || raw <= 0.0 {
        return 0.0;
    }
    let rounded = ((raw * 1e8 / 1000.0) * 10.0).round() / 10.0;
    if rounded < 0.1 {
        0.1
    } else {
        rounded
    }
}

/// Compute a factual, authoritative Network Pulse diagnosis directly in Rust
/// from live node, mempool, fee, block-wait, and recent-cadence telemetry.
#[allow(clippy::too_many_arguments)]
fn compute_network_pulse(
    node_stale: bool,
    mempool_available: bool,
    mempool_tx_count: u64,
    mempool_vbytes: u64,
    next_block_fee: f64,
    mempool_min_fee: f64,
    elapsed_secs: u64,
    avg_block_secs: Option<f64>,
    pending_block_age: Option<u64>,
) -> NetworkPulse {
    let mempool_vmb = mempool_vbytes as f64 / 1_000_000.0;
    let clean_next_fee = if next_block_fee.is_finite() && next_block_fee > 0.0 {
        next_block_fee
    } else {
        0.0
    };
    let clean_min_fee = if mempool_min_fee.is_finite() && mempool_min_fee > 0.0
    {
        mempool_min_fee
    } else {
        0.0
    };
    let clean_avg = avg_block_secs.filter(|a| a.is_finite() && *a > 0.0);

    let depth_pill = if !mempool_available {
        "--".to_string()
    } else if mempool_vbytes == 0 {
        "0 blks".to_string()
    } else if mempool_vmb < 1.0 {
        "<1 blk".to_string()
    } else {
        let blks = mempool_vmb.round() as u64;
        if blks == 1 {
            "~1 blk".to_string()
        } else {
            format!("~{} blks", super::helpers::format_number(blks))
        }
    };

    let cadence_pill = match clean_avg {
        Some(avg) => format!("{:.1}m / blk", avg / 60.0),
        None => "--.-m / blk".to_string(),
    };

    let fee_pill = if clean_next_fee > 0.0 {
        if clean_next_fee < 1.0 {
            format!("~{clean_next_fee:.2} sat/vB")
        } else {
            format!("~{clean_next_fee:.1} sat/vB")
        }
    } else if mempool_available && mempool_tx_count == 0 {
        "~0.0 sat/vB".to_string()
    } else {
        "-- sat/vB".to_string()
    };

    let (headline, color) = if node_stale {
        ("Node Stale", "#f7931a")
    } else if pending_block_age.is_some_and(|age| age >= 2) {
        ("Block Received", "#38bdf8")
    } else if !mempool_available {
        if elapsed_secs >= 2700 {
            ("Extended Drought", "#dc2626")
        } else if elapsed_secs >= 1500 {
            ("Long Wait", "#f97316")
        } else {
            ("Awaiting telemetry", "#94a3b8")
        }
    } else if clean_next_fee >= 30.0 {
        ("High Fees", "#dc2626")
    } else if mempool_vmb >= 100.0 || clean_next_fee >= 15.0 {
        ("Congested", "#f97316")
    } else if elapsed_secs >= 2700 {
        ("Extended Drought", "#c026d3")
    } else if elapsed_secs >= 1500 {
        ("Long Wait", "#f97316")
    } else if mempool_vmb >= 20.0 && clean_next_fee < 5.0 {
        ("Backlog Building", "#38bdf8")
    } else if clean_avg.is_some_and(|avg| avg < 420.0) {
        ("Rapid Blocks", "#00e676")
    } else if clean_avg.is_some_and(|avg| avg > 840.0) {
        ("Slow Cadence", "#f7931a")
    } else if mempool_vmb < 5.0 && clean_next_fee <= 2.0 {
        ("Quiet", "#00e676")
    } else {
        ("Steady", "#00e676")
    };

    let tx_str = super::helpers::format_number(mempool_tx_count);
    let wait_mins = elapsed_secs / 60;

    let summary = if node_stale {
        "Node telemetry is delayed; showing the last confirmed readings until the node responds."
            .to_string()
    } else if let Some(age) = pending_block_age.filter(|&a| a >= 2) {
        format!(
            "New block observed {age}s ago; fetching full block measurements and transaction list from the node."
        )
    } else if !mempool_available && elapsed_secs == 0 && clean_avg.is_none() {
        "Awaiting live node telemetry.".to_string()
    } else {
        match headline {
            "Awaiting telemetry" => {
                "Mempool telemetry is temporarily unavailable; displaying the latest confirmed chain state."
                    .to_string()
            }
            "High Fees" => {
                if clean_min_fee >= 2.0 {
                    format!(
                        "Heavy fee competition for next-block space ({clean_next_fee:.1} sat/vB) with {tx_str} transactions queued across {mempool_vmb:.1} vMB and an eviction floor at {clean_min_fee:.1} sat/vB."
                    )
                } else {
                    format!(
                        "Heavy fee competition for next-block space ({clean_next_fee:.1} sat/vB) with {tx_str} transactions queued across {mempool_vmb:.1} vMB."
                    )
                }
            }
            "Congested" => {
                if clean_min_fee >= 2.0 {
                    format!(
                        "Sustained demand is filling the mempool ({mempool_vmb:.1} vMB across {tx_str} transactions), pushing the eviction floor to {clean_min_fee:.1} sat/vB."
                    )
                } else {
                    format!(
                        "Sustained demand is filling the mempool ({mempool_vmb:.1} vMB across {tx_str} transactions), keeping confirmation fees elevated."
                    )
                }
            }
            "Extended Drought" => format!(
                "No block mined in {wait_mins} minutes; transactions continue to queue while miners search for the next valid hash."
            ),
            "Long Wait" => format!(
                "{wait_mins} minutes since the last block, within normal Poisson variance as pending transactions accumulate."
            ),
            "Backlog Building" => format!(
                "Low-fee transactions ({tx_str} across {mempool_vmb:.1} vMB) are queuing behind the tip, while priority fee rates stay calm."
            ),
            "Rapid Blocks" => {
                "Blocks are arriving faster than the 10-minute target, clearing fee-paying demand quickly."
                    .to_string()
            }
            "Slow Cadence" => {
                "Recent block intervals are running slower than the 10-minute target, giving the mempool more time to build between blocks."
                    .to_string()
            }
            "Quiet" => format!(
                "Light mempool activity ({tx_str} transactions across {mempool_vmb:.1} vMB) with minimal fee competition for upcoming blocks."
            ),
            _ => format!(
                "Block cadence and mempool demand are balanced, with {tx_str} unconfirmed transactions queued across {mempool_vmb:.1} vMB."
            ),
        }
    };

    NetworkPulse {
        headline,
        color,
        summary,
        depth_pill,
        cadence_pill,
        fee_pill,
    }
}

/// Format a single fee rate (sat/vB) for the Blood Pressure tile.
fn format_bp_fee(rate: f64) -> String {
    if !rate.is_finite() || rate <= 0.0 {
        "--".to_string()
    } else if rate < 0.1 {
        format!("{rate:.2}")
    } else {
        format!("{rate:.1}")
    }
}

/// Compute the Blood Pressure vital tile purely in Rust from Bitcoin Core's
/// `estimatesmartfee(1)` (next block, systolic) and `estimatesmartfee(6)`
/// (~1 hour, diastolic), falling back to `mempoolminfee` only when `hour_fee`
/// is unavailable (for example on regtest or a freshly-started node).
fn compute_blood_pressure(
    next_block_fee: f64,
    hour_fee: f64,
    mempool_min_fee_btc_kvb: f64,
    mempool_available: bool,
) -> (String, String, &'static str) {
    let systolic = if next_block_fee.is_finite() && next_block_fee > 0.0 {
        next_block_fee
    } else {
        0.0
    };
    let diastolic = if hour_fee.is_finite() && hour_fee > 0.0 {
        hour_fee
    } else if mempool_available {
        btc_per_kvb_to_sat_per_vb(mempool_min_fee_btc_kvb)
    } else {
        0.0
    };

    if systolic == 0.0 && diastolic == 0.0 {
        return (
            "-- / --".to_string(),
            "Awaiting fees".to_string(),
            "#94a3b8",
        );
    }

    let display =
        format!("{} / {}", format_bp_fee(systolic), format_bp_fee(diastolic));
    let bp_avg = if systolic > 0.0 && diastolic > 0.0 {
        (systolic + diastolic) / 2.0
    } else {
        systolic.max(diastolic)
    };
    let (label, color) = if bp_avg < 10.0 {
        ("Normal \u{00b7} Low fee pressure", "#00e676")
    } else if bp_avg < 30.0 {
        ("Elevated \u{00b7} Moderate fee pressure", "#f7931a")
    } else if bp_avg < 80.0 {
        ("High \u{00b7} High fee pressure", "#ff5722")
    } else {
        ("Critical \u{00b7} Extreme fee pressure", "#f44336")
    };
    (display, label.to_string(), color)
}

/// Minimal JSON parsing for vital signs (avoids serde dependency).
struct Vitals {
    temp_c: f64,
    temp_label: String,
    temp_color: String,
    immune_eh: f64,
    immune_label: String,
    immune_color: String,
}

fn extract_json_f64(json: &str, key: &str) -> f64 {
    let needle = format!("\"{}\":", key);
    if let Some(pos) = json.find(&needle) {
        let start = pos + needle.len();
        let rest = &json[start..];
        let end = rest.find([',', '}']).unwrap_or(rest.len());
        rest[..end].trim().parse().unwrap_or(0.0)
    } else {
        0.0
    }
}

fn extract_json_str(json: &str, key: &str) -> String {
    let needle = format!("\"{}\":\"", key);
    if let Some(pos) = json.find(&needle) {
        let start = pos + needle.len();
        let rest = &json[start..];
        let end = rest.find('"').unwrap_or(rest.len());
        rest[..end].to_string()
    } else {
        String::new()
    }
}

fn parse_vitals_json(json: &str) -> Option<Vitals> {
    if json.len() < 3 {
        return None;
    }
    Some(Vitals {
        temp_c: extract_json_f64(json, "temp_c"),
        temp_label: extract_json_str(json, "temp_label"),
        temp_color: extract_json_str(json, "temp_color"),
        immune_eh: extract_json_f64(json, "immune_eh"),
        immune_label: extract_json_str(json, "immune_label"),
        immune_color: extract_json_str(json, "immune_color"),
    })
}

/// Format a [`crate::stats::heartbeat_sync::QueuePosition`] into `(primary, detail, color)`
/// for the Pinned TX Panel. Pure Rust so both SSR and WASM agree and unit tests cover it.
fn format_queue_position(
    q: crate::stats::heartbeat_sync::QueuePosition,
) -> (String, String, &'static str) {
    let primary = if q.est_blocks <= 1 {
        "~Next block".to_string()
    } else {
        format!("~{} blocks", super::helpers::format_number(q.est_blocks))
    };

    let base_detail = if q.txs_ahead == 0 || q.vbytes_ahead == 0 {
        "Top of mempool \u{00b7} no higher-fee txs ahead".to_string()
    } else {
        let vmb_str = if q.vbytes_ahead < 10_000 {
            "<0.01 vMB".to_string()
        } else {
            format!("{:.2} vMB", q.vbytes_ahead as f64 / 1_000_000.0)
        };
        let tx_word = if q.txs_ahead == 1 { "tx" } else { "txs" };
        format!(
            "{vmb_str} of higher-fee txs ahead ({} {tx_word})",
            super::helpers::format_number(q.txs_ahead)
        )
    };

    // Only surface the unenriched caveat when enough transactions lack fee
    // metadata (>= 100) to potentially shift the estimate; 1-2 brand-new ZMQ
    // arrivals awaiting RPC enrichment are routine noise.
    let detail = if q.unenriched_count >= 100 {
        format!(
            "{base_detail} \u{00b7} {} fee-pending excluded",
            super::helpers::format_number(q.unenriched_count)
        )
    } else {
        base_detail
    };

    let color = if q.est_blocks <= 1 {
        "#00e676"
    } else if q.est_blocks <= 6 {
        "#38bdf8"
    } else if q.est_blocks <= 24 {
        "#f7931a"
    } else {
        "#cbd5e1"
    };
    (primary, detail, color)
}

fn short_txid(txid: &str) -> String {
    if txid.len() > 24 {
        if let (Some(prefix), Some(suffix)) =
            (txid.get(..12), txid.get(txid.len() - 8..))
        {
            return format!("{prefix}...{suffix}");
        }
    }
    txid.to_string()
}

/// UTF-8 safe shortening for long Bitcoin addresses inside the collapsible
/// Inputs & Outputs preview; leaves short script labels (`OP_RETURN`,
/// `Coinbase`, `Taproot`) intact.
fn short_io_label(label: &str) -> String {
    if label.len() > 22 {
        if let (Some(prefix), Some(suffix)) =
            (label.get(..10), label.get(label.len() - 6..))
        {
            return format!("{prefix}\u{2026}{suffix}");
        }
    }
    label.to_string()
}

fn format_first_seen(ts: u64) -> String {
    if ts == 0 {
        return "Already waiting (time unknown)".to_string();
    }
    #[cfg(feature = "hydrate")]
    {
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep",
            "Oct", "Nov", "Dec",
        ];
        let now = leptos::web_sys::js_sys::Date::new_0();
        let now_secs = (now.get_time() / 1000.0).max(0.0) as u64;
        let date = leptos::web_sys::js_sys::Date::new(&JsValue::from_f64(
            (ts as f64) * 1000.0,
        ));
        let same_local_day = date.get_full_year() == now.get_full_year()
            && date.get_month() == now.get_month()
            && date.get_date() == now.get_date();
        let older_than_18h = now_secs.saturating_sub(ts) > 18 * 3600;
        if !same_local_day || older_than_18h {
            let month = MONTHS
                .get(date.get_month() as usize)
                .copied()
                .unwrap_or("???");
            format!(
                "{month} {}, {:02}:{:02}:{:02}",
                date.get_date(),
                date.get_hours(),
                date.get_minutes(),
                date.get_seconds()
            )
        } else {
            format!(
                "{:02}:{:02}:{:02}",
                date.get_hours(),
                date.get_minutes(),
                date.get_seconds()
            )
        }
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let now_ts = chrono::Utc::now().timestamp().max(0) as u64;
        format_first_seen_utc(ts, now_ts)
    }
}

#[cfg(not(feature = "hydrate"))]
fn format_first_seen_utc(ts: u64, now_ts: u64) -> String {
    if ts == 0 {
        return "Already waiting (time unknown)".to_string();
    }
    let Some(dt) = chrono::DateTime::from_timestamp(ts as i64, 0) else {
        return "Already waiting (time unknown)".to_string();
    };
    let same_day = chrono::DateTime::from_timestamp(now_ts as i64, 0)
        .is_some_and(|now_dt| dt.date_naive() == now_dt.date_naive());
    let older_than_18h = now_ts.saturating_sub(ts) > 18 * 3600;
    if !same_day || older_than_18h {
        dt.format("%b %-d, %H:%M:%S UTC").to_string()
    } else {
        dt.format("%H:%M:%S UTC").to_string()
    }
}

/// Colour, label and explanation for the liveness badge.
///
/// **Worst-known state wins**, because a reader needs the most severe reason
/// the numbers might be wrong, not the first one that happens to match.
///
/// Pure, and separate from the signals that call it, so the precedence can be
/// tested without a reactive runtime. The two inputs are independent
/// failures: `conn` is the browser's SSE stream, `node_stale` is whether the
/// server fell back to a cached RPC value. On 2026-10-23 the node lost its
/// peers for hours while the stream stayed healthy, and the badge read LIVE
/// over data that had stopped moving, because only the first input was
/// consulted.
fn status_for(
    conn: &str,
    node_stale: bool,
) -> (&'static str, &'static str, &'static str) {
    match conn {
        "disconnected" => (
            "#f44336",
            "RECONNECTING",
            "The live connection to this site dropped and is retrying. The chart holds its last known state until it comes back.",
        ),
        "stale" => (
            "#f7931a",
            "STALE",
            "The live connection is open but has gone quiet. Nothing new has arrived for a while.",
        ),
        // Named apart from the stream's own staleness: the stream is fine
        // here, and knowing which link broke is the difference between
        // waiting and going to look at the node.
        _ if node_stale => (
            "#f7931a",
            "NODE STALE",
            "The connection is fine, but the Bitcoin node behind it did not answer, so these figures are the last values it gave and may be out of date.",
        ),
        "syncing" => (
            "#42a5f5",
            "SYNCING",
            "Syncing mempool snapshot from this site's Bitcoin node.",
        ),
        "partial" => (
            "#f7931a",
            "PARTIAL",
            "Live stream connected, but mempool coverage is currently partial while syncing with the node.",
        ),
        _ => ("#00e676", "LIVE", "Live from this site's own Bitcoin node."),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        blocks_to_json, blocks_to_json_with_parent, compute_network_pulse,
        compute_recent_avg_block_secs, format_mempool_summary,
        record_recent_block_pair, status_for, MempoolDisplay,
    };
    use crate::stats::types::{
        BlockSummary, LiveBlockchain, LiveMempool, LiveNetwork, LiveStats,
    };

    fn sample_live_stats(
        mempool_size: u64,
        mempool_bytes: u64,
        mempool_available: bool,
        stale: bool,
        hashrate: f64,
    ) -> LiveStats {
        LiveStats {
            blockchain: LiveBlockchain {
                blocks: 900_000,
                chain: "main".to_string(),
                difficulty: 100.0,
                verification_progress: 1.0,
                size_on_disk: 0,
                bestblockhash: String::new(),
                time: 1_700_000_000,
            },
            mempool: LiveMempool {
                size: mempool_size,
                bytes: mempool_bytes,
                usage: 0,
                total_fee: 0.0,
                maxmempool: 300_000_000,
                mempoolminfee: 0.00001,
            },
            next_block_fee: 2.0,
            hour_fee: 1.5,
            network: LiveNetwork {
                price_usd: 100_000.0,
                sats_per_dollar: 1_000,
                market_cap_usd: 0.0,
                total_supply: 19_000_000.0,
                max_supply: 21_000_000.0,
                percent_issued: 90.0,
                utxo_count: 0,
                chain_size_gb: 0.0,
                hashrate,
            },
            stale,
            mempool_available,
        }
    }

    /// The source of this file, for the wiring guard below.
    const SELF: &str = include_str!("heartbeat.rs");

    /// `node_stale` must actually come from `LiveStats.stale`.
    ///
    /// The tests above prove `status_for` decides correctly once told the
    /// node is behind. They cannot prove anything tells it: replacing the
    /// signal's body with `false` leaves every one of them passing, which is
    /// precisely the shape of a guard that watches the wrong half.
    ///
    /// Source-read because the alternative is a reactive runtime and a
    /// server-function round trip to assert one field reaches one closure.
    /// Scoped above `mod tests` so this test's own prose cannot satisfy it.
    #[test]
    fn the_badge_reads_the_servers_own_staleness_flag() {
        let code = &SELF[..SELF.find("\n#[cfg(test)]").unwrap_or(SELF.len())];
        let site = code
            .find("let node_stale =")
            .expect("the node_stale binding is gone");
        // Bounded by the binding's own closing brace rather than a fixed
        // width, which went stale the moment the body grew a comment.
        //
        // **Comments stripped.** The body explains why `connected` is part
        // of the predicate, so asserting on the raw text passed even with
        // `connected` deleted from the code: the guard was reading its own
        // justification.
        let body: String = {
            let end = code[site..]
                .find("});")
                .map(|e| site + e + 3)
                .unwrap_or(code.len());
            code[site..end]
                .lines()
                .map(|l| l.split("//").next().unwrap_or(""))
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert!(
            body.contains("cached_live") && body.contains("stale"),
            "node_stale no longer reads LiveStats.stale, so the badge cannot \
             know the node is behind however well it decides: {body}"
        );
        // The banner above this page already renders on `!connected ||
        // stale`. The badge must use both halves or the two can disagree,
        // which is the state this change exists to end.
        // Without the first-paint guard, `connected` starts false and every
        // initial render accuses the node of being down. The banner carries
        // the same suppression; this badge rendered NODE STALE in SSR until
        // it did too.
        assert!(
            body.contains("is_none()"),
            "the first-paint suppression is gone, so every page load will \
             open by claiming the node is not responding: {body}"
        );
        assert!(
            body.contains("connected"),
            "the badge dropped the `connected` half of the predicate, so it \
             can now read LIVE under the banner that says otherwise: {body}"
        );
        assert!(
            code.contains("status_for(&conn_state.get(), node_stale.get())"),
            "the badge signals no longer pass node_stale to status_for, so \
             the second input is ignored"
        );
    }

    /// A healthy stream over a silent node must not read LIVE.
    ///
    /// This is the case that shipped: `conn` is "live" because the browser's
    /// SSE stream is open and the server is answering it, while the node
    /// behind the server has stopped returning fresh data. Both inputs have
    /// to be consulted, and the node's state has to win.
    #[test]
    fn a_healthy_stream_over_a_stale_node_says_so() {
        let (colour, label, tip) = status_for("live", true);
        assert_eq!(
            label, "NODE STALE",
            "a live stream over a stale node read {label:?}, which tells a \
             reader the figures are current when they are not"
        );
        assert_ne!(colour, "#00e676", "green is the all-clear colour");
        assert!(
            tip.contains("node"),
            "the explanation must name the node, or a reader cannot tell \
             this from a dropped connection: {tip}"
        );
    }

    /// The all-clear requires both halves to be healthy.
    #[test]
    fn live_means_both_the_stream_and_the_node_are_fine() {
        assert_eq!(status_for("live", false).1, "LIVE");
        assert_eq!(status_for("live", false).0, "#00e676");
    }

    /// A dropped stream outranks a stale node: with nothing arriving, the
    /// node's state is unknowable and the reader's action is different.
    #[test]
    fn the_worst_known_state_wins() {
        assert_eq!(status_for("disconnected", true).1, "RECONNECTING");
        assert_eq!(status_for("disconnected", false).1, "RECONNECTING");
        assert_eq!(status_for("stale", true).1, "STALE");
    }

    /// Every state explains itself, and no two share wording.
    #[test]
    fn each_state_carries_its_own_explanation() {
        let tips: Vec<&str> = [
            ("live", false),
            ("live", true),
            ("syncing", false),
            ("partial", false),
            ("stale", false),
            ("disconnected", false),
        ]
        .iter()
        .map(|(c, n)| status_for(c, *n).2)
        .collect();
        for t in &tips {
            assert!(t.len() > 30, "a badge with no usable explanation: {t}");
        }
        let mut uniq = tips.clone();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), tips.len(), "two states share an explanation");
    }

    #[test]
    fn syncing_and_partial_status_precedence() {
        let (sync_col, sync_lbl, sync_tip) = status_for("syncing", false);
        assert_eq!(sync_col, "#42a5f5");
        assert_eq!(sync_lbl, "SYNCING");
        assert!(sync_tip.contains("snapshot"));
        // Stale node outranks syncing and partial coverage.
        assert_eq!(status_for("syncing", true).1, "NODE STALE");

        let (part_col, part_lbl, part_tip) = status_for("partial", false);
        assert_eq!(part_col, "#f7931a");
        assert_eq!(part_lbl, "PARTIAL");
        assert!(part_tip.contains("partial"));
        assert_eq!(status_for("partial", true).1, "NODE STALE");
    }

    #[cfg(not(feature = "hydrate"))]
    #[test]
    fn format_first_seen_includes_short_date_when_older_than_today_or_18h() {
        use super::format_first_seen_utc;
        // 2026-11-20 20:00:00 UTC
        let now = chrono::NaiveDate::from_ymd_opt(2026, 11, 20)
            .unwrap()
            .and_hms_opt(20, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() as u64;
        assert_eq!(
            format_first_seen_utc(0, now),
            "Already waiting (time unknown)"
        );
        // Same day, 2 hours earlier -> bare time.
        let recent = now - 2 * 3600;
        assert_eq!(format_first_seen_utc(recent, now), "18:00:00 UTC");
        // Same day, 19 hours earlier (> 18h) -> includes short date.
        let older_same_day = now - 19 * 3600;
        assert_eq!(
            format_first_seen_utc(older_same_day, now),
            "Nov 20, 01:00:00 UTC"
        );
        // Previous day, 16:04:12 UTC -> includes short date.
        let prev_day = chrono::NaiveDate::from_ymd_opt(2026, 11, 19)
            .unwrap()
            .and_hms_opt(16, 4, 12)
            .unwrap()
            .and_utc()
            .timestamp() as u64;
        assert_eq!(
            format_first_seen_utc(prev_day, now),
            "Nov 19, 16:04:12 UTC"
        );
    }

    #[test]
    fn mempool_summary_none_renders_unavailable() {
        assert_eq!(
            format_mempool_summary(None),
            MempoolDisplay {
                bar: "-- tx (-- vMB)".to_string(),
                tile_vmb: "--.-".to_string(),
                tile_tx_label: "-- tx in mempool".to_string(),
            }
        );
    }

    #[test]
    fn mempool_summary_positive_formats_count_and_vmb() {
        let live = sample_live_stats(12_345, 4_260_000, true, false, 600e18);
        assert_eq!(
            format_mempool_summary(Some(&live)),
            MempoolDisplay {
                bar: "12,345 tx (4.3 vMB)".to_string(),
                tile_vmb: "4.3".to_string(),
                tile_tx_label: "12,345 tx in mempool".to_string(),
            }
        );
    }

    #[test]
    fn mempool_summary_verified_zero_clears_prior_positive() {
        let positive = sample_live_stats(5_432, 2_100_000, true, false, 600e18);
        let before = format_mempool_summary(Some(&positive));
        assert_eq!(before.bar, "5,432 tx (2.1 vMB)");
        assert_eq!(before.tile_vmb, "2.1");
        assert_eq!(before.tile_tx_label, "5,432 tx in mempool");

        let empty = sample_live_stats(0, 0, true, false, 0.0);
        assert_eq!(
            format_mempool_summary(Some(&empty)),
            MempoolDisplay {
                bar: "0 tx (0.0 vMB)".to_string(),
                tile_vmb: "0.0".to_string(),
                tile_tx_label: "0 tx in mempool".to_string(),
            }
        );
    }

    #[test]
    fn mempool_summary_rpc_failure_never_renders_fake_zero() {
        // Even when other RPCs succeeded (hashrate > 0), a failed
        // getmempoolinfo must render unavailable rather than 0.
        let failed = sample_live_stats(0, 0, false, true, 650e18);
        assert_eq!(
            format_mempool_summary(Some(&failed)),
            MempoolDisplay {
                bar: "-- tx (-- vMB)".to_string(),
                tile_vmb: "--.-".to_string(),
                tile_tx_label: "-- tx in mempool".to_string(),
            }
        );
    }

    #[test]
    fn mempool_summary_stale_cached_observation_retains_last_valid_totals() {
        let stale_cached =
            sample_live_stats(8_500, 3_400_000, true, true, 620e18);
        assert_eq!(
            format_mempool_summary(Some(&stale_cached)),
            MempoolDisplay {
                bar: "8,500 tx (3.4 vMB)".to_string(),
                tile_vmb: "3.4".to_string(),
                tile_tx_label: "8,500 tx in mempool".to_string(),
            }
        );
    }

    #[test]
    fn bar_and_temperature_tile_share_format_mempool_summary_without_sse_override(
    ) {
        let code = &SELF[..SELF.find("\n#[cfg(test)]").unwrap_or(SELF.len())];
        assert!(
            !code.contains("get_heartbeat_mempool_stats"),
            "SSE timeline mempool count override must stay removed"
        );
        assert!(
            code.contains("format_mempool_summary(cached_live.get().as_ref())"),
            "mempool summary must derive from LiveStats via format_mempool_summary"
        );
        assert!(
            code.contains("mempool_summary.get().bar")
                && code.contains("mempool_summary.get().tile_vmb")
                && code.contains("mempool_summary.get().tile_tx_label"),
            "both bottom bar and Temperature tile must derive from the shared mempool_summary"
        );
    }

    fn sample_block(height: u64, hash: &str, timestamp: u64) -> BlockSummary {
        BlockSummary {
            height,
            hash: hash.to_string(),
            timestamp,
            tx_count: 2_500,
            size: 1_500_000,
            weight: 3_990_000,
            total_fees: 25_000_000,
            ..BlockSummary::default()
        }
    }

    #[test]
    fn blocks_to_json_includes_hash_and_exact_signed_inter_block_seconds() {
        let b0 = sample_block(100, "0000aaaa", 1_700_000_600);
        let b1 = sample_block(101, "0000bbbb", 1_700_000_900); // +300s
        let b2 = sample_block(102, "0000cccc", 1_700_000_900); // 0s
        let b3 = sample_block(103, "0000dddd", 1_700_000_850); // -50s (MTP-valid negative delta)
        let b4 = sample_block(105, "0000eeee", 1_700_001_500); // non-consecutive gap -> null

        let json = blocks_to_json(&[b0.clone(), b1, b2, b3, b4]);
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 5);

        assert_eq!(parsed[0]["hash"].as_str(), Some("0000aaaa"));
        assert!(
            parsed[0]["inter_block_seconds"].is_null(),
            "first block with no parent must emit null, never fabricated 600"
        );
        assert_eq!(parsed[1]["hash"].as_str(), Some("0000bbbb"));
        assert_eq!(parsed[1]["inter_block_seconds"].as_i64(), Some(300));
        assert_eq!(parsed[2]["inter_block_seconds"].as_i64(), Some(0));
        assert_eq!(parsed[3]["inter_block_seconds"].as_i64(), Some(-50));
        assert!(parsed[4]["inter_block_seconds"].is_null());

        // Passing an immediate parent populates index 0 accurately.
        let parent = sample_block(99, "00009999", 1_700_000_180);
        let with_parent = blocks_to_json_with_parent(&[b0], Some(&parent));
        let parsed_parent: Vec<serde_json::Value> =
            serde_json::from_str(&with_parent).unwrap();
        assert_eq!(parsed_parent[0]["inter_block_seconds"].as_i64(), Some(420));
    }

    #[test]
    fn recent_block_pairs_and_average_handle_cadence_and_reorg() {
        let mut pairs = Vec::new();
        for i in 0..25u64 {
            record_recent_block_pair(
                &mut pairs,
                840_000 + i,
                1_700_000_000 + i * 600,
            );
        }
        assert_eq!(pairs.len(), 25);
        assert_eq!(compute_recent_avg_block_secs(&pairs), Some(600.0));

        // Reorg drops heights >= retracted height cleanly.
        record_recent_block_pair(
            &mut pairs,
            840_023,
            1_700_000_000 + 23 * 600 + 120,
        );
        assert_eq!(
            pairs.last(),
            Some(&(840_023, 1_700_000_000 + 23 * 600 + 120))
        );

        assert_eq!(compute_recent_avg_block_secs(&[]), None);
        assert_eq!(
            compute_recent_avg_block_secs(&[(840_000, 1_700_000_000)]),
            None
        );
    }

    #[test]
    fn network_pulse_diagnoses_stale_pending_drought_fees_backlog_quiet_and_unavailable(
    ) {
        // 1. Node Stale outranks everything.
        let stale = compute_network_pulse(
            true,
            true,
            50_000,
            150_000_000,
            80.0,
            10.0,
            3_600,
            Some(600.0),
            Some(10),
        );
        assert_eq!(stale.headline, "Node Stale");
        assert_eq!(stale.color, "#f7931a");

        // 2. Pending block >= 2s grace period triggers Block Received; < 2s does not.
        let pending = compute_network_pulse(
            false,
            true,
            12_000,
            8_000_000,
            2.0,
            1.0,
            5,
            Some(600.0),
            Some(4),
        );
        assert_eq!(pending.headline, "Block Received");
        assert_eq!(pending.color, "#38bdf8");
        assert!(pending.summary.contains("4s ago"));

        let sub_grace = compute_network_pulse(
            false,
            true,
            12_000,
            8_000_000,
            2.0,
            1.0,
            1,
            Some(600.0),
            Some(1),
        );
        assert_ne!(sub_grace.headline, "Block Received");

        // 3. 50-minute drought with low fees -> Extended Drought.
        let drought = compute_network_pulse(
            false,
            true,
            15_000,
            12_000_000,
            2.0,
            1.0,
            3_000,
            Some(612.0),
            None,
        );
        assert_eq!(drought.headline, "Extended Drought");
        assert!(drought.summary.contains("50 minutes"));
        assert_eq!(drought.depth_pill, "~12 blks");
        assert_eq!(drought.cadence_pill, "10.2m / blk");
        assert_eq!(drought.fee_pill, "~2.0 sat/vB");

        // 4. High fees (>= 30 sat/vB).
        let high_fees = compute_network_pulse(
            false,
            true,
            40_000,
            45_000_000,
            42.5,
            5.0,
            300,
            Some(600.0),
            None,
        );
        assert_eq!(high_fees.headline, "High Fees");
        assert!(high_fees.summary.contains("42.5 sat/vB"));

        // 5. Large low-fee backlog (>= 20 vMB, next fee < 5).
        let backlog = compute_network_pulse(
            false,
            true,
            65_000,
            35_000_000,
            2.5,
            1.0,
            400,
            Some(600.0),
            None,
        );
        assert_eq!(backlog.headline, "Backlog Building");
        assert_eq!(backlog.depth_pill, "~35 blks");

        // 6. Quiet mempool (< 5 vMB, next fee <= 2).
        let quiet = compute_network_pulse(
            false,
            true,
            2_100,
            1_800_000,
            1.5,
            1.0,
            240,
            Some(580.0),
            None,
        );
        assert_eq!(quiet.headline, "Quiet");
        assert_eq!(quiet.depth_pill, "~2 blks");

        // 7. Unavailable mempool must NEVER show green Steady or Quiet.
        let unavail = compute_network_pulse(
            false,
            false,
            0,
            0,
            0.0,
            0.0,
            180,
            Some(600.0),
            None,
        );
        assert_eq!(unavail.headline, "Awaiting telemetry");
        assert_eq!(unavail.color, "#94a3b8");
        assert_eq!(unavail.depth_pill, "--");
        assert_eq!(unavail.fee_pill, "-- sat/vB");
        assert!(unavail
            .summary
            .contains("Mempool telemetry is temporarily unavailable"));
        assert!(!unavail.summary.contains("0 unconfirmed"));

        // 8. SSR / first-paint (no live stats yet) -> neutral Awaiting telemetry.
        let first_paint =
            compute_network_pulse(false, false, 0, 0, 0.0, 0.0, 0, None, None);
        assert_eq!(first_paint.headline, "Awaiting telemetry");
        assert_eq!(first_paint.color, "#94a3b8");
        assert_eq!(first_paint.summary, "Awaiting live node telemetry.");

        // 9. Block-timestamp drought/long-wait still surface while mempool is unavailable.
        let unavail_drought = compute_network_pulse(
            false,
            false,
            0,
            0,
            0.0,
            0.0,
            2_700,
            Some(600.0),
            None,
        );
        assert_eq!(unavail_drought.headline, "Extended Drought");
        assert_eq!(unavail_drought.color, "#dc2626");

        let unavail_wait = compute_network_pulse(
            false,
            false,
            0,
            0,
            0.0,
            0.0,
            1_500,
            Some(600.0),
            None,
        );
        assert_eq!(unavail_wait.headline, "Long Wait");
        assert_eq!(unavail_wait.color, "#f97316");
    }

    #[test]
    fn btc_per_kvb_to_sat_per_vb_converts_and_floors_consistently() {
        assert_eq!(super::btc_per_kvb_to_sat_per_vb(0.0), 0.0);
        assert_eq!(super::btc_per_kvb_to_sat_per_vb(-0.00001), 0.0);
        assert_eq!(super::btc_per_kvb_to_sat_per_vb(f64::NAN), 0.0);
        assert_eq!(super::btc_per_kvb_to_sat_per_vb(0.0000001), 0.1);
        assert_eq!(super::btc_per_kvb_to_sat_per_vb(0.000001), 0.1);
        assert_eq!(super::btc_per_kvb_to_sat_per_vb(0.00001), 1.0);
        assert_eq!(super::btc_per_kvb_to_sat_per_vb(0.0000256), 2.6);
    }

    #[test]
    fn blood_pressure_uses_next_block_and_one_hour_targets_with_fallbacks() {
        // Calm mainnet: 1.01 sat/vB next block & 1.01 sat/vB ~1h
        let (disp, label, color) =
            super::compute_blood_pressure(1.01, 1.01, 0.000001, true);
        assert_eq!(disp, "1.0 / 1.0");
        assert_eq!(label, "Normal \u{00b7} Low fee pressure");
        assert_eq!(color, "#00e676");

        // Elevated fee market: next block spikes while 6-block sits lower
        let (disp, label, color) =
            super::compute_blood_pressure(28.4, 12.6, 0.000001, true);
        assert_eq!(disp, "28.4 / 12.6");
        assert_eq!(label, "Elevated \u{00b7} Moderate fee pressure");
        assert_eq!(color, "#f7931a");

        // High fee pressure
        let (disp, label, color) =
            super::compute_blood_pressure(75.0, 45.0, 0.00002, true);
        assert_eq!(disp, "75.0 / 45.0");
        assert_eq!(label, "High \u{00b7} High fee pressure");
        assert_eq!(color, "#ff5722");

        // Critical fee pressure
        let (disp, label, color) =
            super::compute_blood_pressure(140.0, 95.0, 0.00015, true);
        assert_eq!(disp, "140.0 / 95.0");
        assert_eq!(label, "Critical \u{00b7} Extreme fee pressure");
        assert_eq!(color, "#f44336");

        // Regtest / fresh node with no estimatesmartfee(6): falls back to mempoolminfee
        let (disp, label, color) =
            super::compute_blood_pressure(0.0, 0.0, 0.000001, true);
        assert_eq!(disp, "-- / 0.1");
        assert_eq!(label, "Normal \u{00b7} Low fee pressure");
        assert_eq!(color, "#00e676");

        // Both fee estimates & mempool unavailable
        let (disp, label, color) =
            super::compute_blood_pressure(0.0, 0.0, 0.0, false);
        assert_eq!(disp, "-- / --");
        assert_eq!(label, "Awaiting fees");
        assert_eq!(color, "#94a3b8");
    }

    #[test]
    fn heartbeat_landmarks_dedupes_hall_of_fame_and_filters_cleanly() {
        let all = super::heartbeat_landmarks();
        assert!(all.len() >= 30, "expected >= 30 curated block landmarks");

        // Block heights are strictly unique (no duplicate #0 / #840,000 rows).
        let unique_heights: std::collections::HashSet<u64> =
            all.iter().map(|i| i.height).collect();
        assert_eq!(unique_heights.len(), all.len());

        // Covers every Hall of Fame category.
        for cat in [
            super::HofCategory::Milestones,
            super::HofCategory::Records,
            super::HofCategory::Protocol,
            super::HofCategory::Attacks,
            super::HofCategory::Oddities,
        ] {
            assert!(
                all.iter().any(|i| i.category == cat),
                "missing category {cat:?}"
            );
        }

        // Matches by height (with or without #/commas), year, title, and category.
        let highest_fee = all
            .iter()
            .find(|i| i.height == 409_008)
            .expect("highest fee block present");
        assert!(super::landmark_matches(highest_fee, None, "#409,008"));
        assert!(super::landmark_matches(highest_fee, None, "409008"));
        assert!(super::landmark_matches(highest_fee, None, "2016"));
        assert!(super::landmark_matches(
            highest_fee,
            Some(super::HofCategory::Records),
            "highest fee"
        ));
        assert!(!super::landmark_matches(
            highest_fee,
            Some(super::HofCategory::Attacks),
            ""
        ));
    }

    #[test]
    fn format_queue_position_formats_top_next_multi_block_and_unenriched() {
        use crate::stats::heartbeat_sync::QueuePosition;

        let (p, d, c) = super::format_queue_position(QueuePosition {
            txs_ahead: 0,
            vbytes_ahead: 0,
            est_blocks: 1,
            unenriched_count: 0,
        });
        assert_eq!(p, "~Next block");
        assert_eq!(d, "Top of mempool \u{00b7} no higher-fee txs ahead");
        assert_eq!(c, "#00e676");

        let (p, d, c) = super::format_queue_position(QueuePosition {
            txs_ahead: 1,
            vbytes_ahead: 2_500,
            est_blocks: 1,
            unenriched_count: 2,
        });
        assert_eq!(p, "~Next block");
        assert_eq!(d, "<0.01 vMB of higher-fee txs ahead (1 tx)");
        assert_eq!(c, "#00e676");

        let (_, d_many_un, _) = super::format_queue_position(QueuePosition {
            txs_ahead: 1,
            vbytes_ahead: 2_500,
            est_blocks: 1,
            unenriched_count: 250,
        });
        assert_eq!(
            d_many_un,
            "<0.01 vMB of higher-fee txs ahead (1 tx) \u{00b7} 250 fee-pending excluded"
        );

        let (p, d, c) = super::format_queue_position(QueuePosition {
            txs_ahead: 4_200,
            vbytes_ahead: 3_420_000,
            est_blocks: 4,
            unenriched_count: 0,
        });
        assert_eq!(p, "~4 blocks");
        assert_eq!(d, "3.42 vMB of higher-fee txs ahead (4,200 txs)");
        assert_eq!(c, "#38bdf8");

        let (p, d, c) = super::format_queue_position(QueuePosition {
            txs_ahead: 22_000,
            vbytes_ahead: 14_800_000,
            est_blocks: 15,
            unenriched_count: 0,
        });
        assert_eq!(p, "~15 blocks");
        assert_eq!(d, "14.80 vMB of higher-fee txs ahead (22,000 txs)");
        assert_eq!(c, "#f7931a");

        assert_eq!(super::short_io_label("OP_RETURN"), "OP_RETURN");
        assert_eq!(
            super::short_io_label("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh"),
            "bc1qxy2kgd\u{2026}hx0wlh"
        );

        let (_, _, c) = super::format_queue_position(QueuePosition {
            txs_ahead: 65_000,
            vbytes_ahead: 31_000_000,
            est_blocks: 32,
            unenriched_count: 0,
        });
        assert_eq!(c, "#cbd5e1");
    }

    #[test]
    fn v2_1_helpers_format_fee_bands_arrival_rate_and_mempool_ram() {
        use crate::stats::heartbeat_sync::FeeBandStat;

        assert_eq!(
            super::format_band_chip_metrics(FeeBandStat {
                tx_count: 0,
                vbytes: 0,
            }),
            ""
        );
        assert_eq!(
            super::format_band_chip_metrics(FeeBandStat {
                tx_count: 42,
                vbytes: 0,
            }),
            "42"
        );
        assert_eq!(
            super::format_band_chip_metrics(FeeBandStat {
                tx_count: 150,
                vbytes: 34_200,
            }),
            "150 \u{00b7} 34 kvB"
        );
        assert_eq!(
            super::format_band_chip_metrics(FeeBandStat {
                tx_count: 12_450,
                vbytes: 4_250_000,
            }),
            "12.4k \u{00b7} 4.2 vMB"
        );

        assert_eq!(super::format_arrival_rate(None), None);
        assert_eq!(super::format_arrival_rate(Some(f64::NAN)), None);
        assert_eq!(
            super::format_arrival_rate(Some(6.42)),
            Some("6.4 tx/s".to_string())
        );
        assert_eq!(
            super::format_arrival_rate(Some(124.7)),
            Some("125 tx/s".to_string())
        );

        assert_eq!(super::format_mempool_ram(None), None);
        let mut live = sample_live_stats(1_000, 1_000_000, true, false, 600.0);
        live.mempool.usage = 166_000_000;
        live.mempool.maxmempool = 300_000_000;
        live.mempool.mempoolminfee = 0.00001;
        let m1 = super::format_mempool_ram(Some(&live)).unwrap();
        assert!((m1.pct - 55.333).abs() < 0.1);
        assert_eq!(m1.color, "#38bdf8");
        assert_eq!(m1.caption, "RAM 166 / 300 MB (55%)");

        // Elevated RAM (>= 75%) and raised min fee (> 1.05 sat/vB)
        live.mempool.usage = 240_000_000;
        live.mempool.mempoolminfee = 0.000024;
        let m2 = super::format_mempool_ram(Some(&live)).unwrap();
        assert_eq!(m2.color, "#f7931a");
        assert_eq!(
            m2.caption,
            "RAM 240 / 300 MB (80%) \u{00b7} Min 2.4 sat/vB"
        );

        // Critical RAM (>= 90%)
        live.mempool.usage = 285_000_000;
        let m3 = super::format_mempool_ram(Some(&live)).unwrap();
        assert_eq!(m3.color, "#f44336");
    }
}
