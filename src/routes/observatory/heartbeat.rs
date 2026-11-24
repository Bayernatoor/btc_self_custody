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

use leptos::prelude::*;
use leptos_meta::*;

#[cfg(feature = "hydrate")]
use wasm_bindgen::prelude::*;

use crate::stats::heartbeat_client::{
    fee_band_for_rate, format_fee_rate, hb_v2_focus_block_height,
    hb_v2_focus_tx, hb_v2_set_pinned_tx, primary_notable_label, PinnedTxStatus,
    PinnedTxView,
};
#[cfg(feature = "hydrate")]
use crate::stats::heartbeat_client::{
    parse_tx_param_from_search, HeartbeatClient, HeartbeatClientSignals,
};

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

    #[wasm_bindgen(js_name = destroyHeartbeat)]
    fn destroy_heartbeat();

    // Phase 2: Vital signs
    #[wasm_bindgen(js_name = getHeartbeatVitals)]
    fn get_heartbeat_vitals() -> String;

    // Phase 3: Rhythm strip
    #[wasm_bindgen(js_name = renderRhythmStrip)]
    fn render_rhythm_strip(canvas_id: &str, blocks_json: &str);

    #[wasm_bindgen(js_name = getOrganismStatus)]
    fn get_organism_status() -> String;

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

    // The Lookout feed renderer defined in heartbeat-sse.js.
    #[wasm_bindgen(js_name = _notableFeed, catch)]
    fn js_notable_feed(tx: JsValue, opts: JsValue) -> Result<(), JsValue>;
}

#[cfg(feature = "hydrate")]
fn show_block_detail_js(height: u64) {
    let _ = js_show_block_detail(height as f64);
}

#[cfg(not(feature = "hydrate"))]
fn show_block_detail_js(_height: u64) {}

#[cfg(feature = "hydrate")]
fn push_notable_history_item(item: &crate::stats::types::NotableTxInfo) {
    let Ok(json) = serde_json::to_string(item) else {
        return;
    };
    let Ok(val) = leptos::web_sys::js_sys::JSON::parse(&json) else {
        return;
    };
    let opts = leptos::web_sys::js_sys::Object::new();
    let _ = leptos::web_sys::js_sys::Reflect::set(
        &opts,
        &JsValue::from_str("append"),
        &JsValue::TRUE,
    );
    let _ = js_notable_feed(val, opts.into());
}

#[cfg(not(feature = "hydrate"))]
fn init_heartbeat(_: &str) {}
#[cfg(not(feature = "hydrate"))]
fn push_heartbeat_blocks(_: &str, _: bool) {}
#[cfg(not(feature = "hydrate"))]
fn update_heartbeat_live(_: &str) {}
#[cfg(not(feature = "hydrate"))]
fn destroy_heartbeat() {}
#[cfg(not(feature = "hydrate"))]
fn get_heartbeat_vitals() -> String {
    "{}".to_string()
}
#[cfg(not(feature = "hydrate"))]
fn render_rhythm_strip(_: &str, _: &str) {}
#[cfg(not(feature = "hydrate"))]
fn get_organism_status() -> String {
    "{}".to_string()
}
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

    // First-visit hint overlay. Starts visible so it renders in the SSR HTML
    // and hydrates with its dismiss handler attached, starting false and
    // toggling after init breaks hydration (the <Show> body never gets its
    // click handler, leaving an undismissable overlay). For returning visitors
    // we hide it immediately on hydration via a localStorage flag (below), and
    // the (i) button in the control bar reopens it on demand.
    let (show_hint, set_show_hint) = signal(true);
    #[cfg(feature = "hydrate")]
    {
        Effect::new(move || {
            if hint_already_seen() {
                set_show_hint.set(false);
            }
        });
    }

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
    let (search_query, set_search_query) = signal(String::new());
    let (search_feedback, set_search_feedback) =
        signal(Option::<(bool, String)>::None);
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let (copied_txid, set_copied_txid) = signal(false);

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

    // Organism status
    let (org_condition, set_org_condition) = signal("Initializing".to_string());
    let (org_desc, set_org_desc) = signal("Waiting for data...".to_string());
    let (org_color, set_org_color) = signal("#00e676".to_string());

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

    // Fetch blocks for the initial timeline (current retarget period = 2016).
    // DB-only (fetch_recent_blocks derives the tip from db::max_height, no node
    // RPC), and NOT gated on cached_live, so the historical EKG renders
    // immediately on load even when the home node is unreachable. The live parts
    // (SSE txs, vitals, STALE indicator) fill in separately once/if the node is up.
    let initial_blocks = LocalResource::new(move || async move {
        crate::stats::server_fns::fetch_recent_blocks(RETARGET_PERIOD)
            .await
            .unwrap_or_default()
    });

    // Load initial Lookout history so #whale-feed-panel populates on v2 without
    // relying on the legacy v1 SSE history payload.
    #[cfg_attr(not(feature = "hydrate"), allow(unused_variables))]
    let initial_notables = LocalResource::new(move || async move {
        crate::stats::server_fns::fetch_notable_txs(
            crate::stats::types::NotableTxFilter::default(),
            100,
            0,
        )
        .await
        .ok()
    });

    #[cfg(feature = "hydrate")]
    {
        let notables_loaded = std::rc::Rc::new(std::cell::Cell::new(false));
        Effect::new(move || {
            if notables_loaded.get() {
                return;
            }
            if let Some(Some(page)) = initial_notables.get() {
                notables_loaded.set(true);
                for item in &page.items {
                    push_notable_history_item(item);
                }
            }
        });
    }

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

                // Build block events with inter-block time (replay=true for compressed history)
                let json = blocks_to_json(&blocks);
                push_heartbeat_blocks(&json, true);

                // Store last height: use the latest from LiveStats (not the last replayed block)
                // to avoid missing blocks that arrived between fetch and now
                let replay_height =
                    blocks.last().map(|b| b.height).unwrap_or(0);
                last_height.set(std::cmp::max(live_height, replay_height));

                // Render rhythm strip with last 144 blocks (24hr)
                let strip_blocks = if blocks.len() > 144 {
                    &blocks[blocks.len() - 144..]
                } else {
                    &blocks
                };
                let strip_json = blocks_to_json(strip_blocks);
                render_rhythm_strip("rhythm-strip-canvas", &strip_json);
            }

            #[cfg(feature = "hydrate")]
            {
                let initial_tx_param = leptos::prelude::window()
                    .location()
                    .search()
                    .ok()
                    .and_then(|s| parse_tx_param_from_search(&s));
                if let Some(ref q) = initial_tx_param {
                    set_search_query.set(q.clone());
                }
                let client = HeartbeatClient::new(HeartbeatClientSignals {
                    set_loading,
                    set_conn_state,
                    set_coverage_note,
                    set_pinned_tx,
                    pinned_tx,
                    set_search_feedback,
                    set_sse_height,
                    sse_height,
                    set_sse_ts,
                });
                hb_client.set_value(Some(client.clone()));
                client.start(initial_tx_param);
            }
        }
    });

    // Helper: refresh vital signs from JS state
    let refresh_vitals = move || {
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

            // Blood Pressure: compute diastolic directly from LiveStats (JS value unreliable)
            let raw_minfee = cached_live
                .get_untracked()
                .map(|s| s.mempool.mempoolminfee)
                .unwrap_or(0.0);
            // BTC/kB to sat/vB: raw * 1e8 / 1000
            // f64 stores 0.00000100 as ~9.99e-7, losing precision.
            // Round to nearest 0.1 sat/vB since relay fees are always clean multiples.
            let raw_sat_vb = raw_minfee * 1e8 / 1000.0;
            let diastolic = (raw_sat_vb * 10.0 + 0.5).floor() / 10.0;
            let diastolic = if diastolic < 0.1 && raw_minfee > 0.0 {
                0.1
            } else {
                diastolic
            };
            // Use 2 decimals if diastolic is < 0.1, otherwise 1
            let dia_fmt = if diastolic < 0.1 && diastolic > 0.0 {
                format!("{:.2}", diastolic)
            } else {
                format!("{:.1}", diastolic)
            };
            set_bp_display.set(format!("{:.1} / {}", v.bp_systolic, dia_fmt));
            let bp_context = if (v.bp_systolic + diastolic) / 2.0 < 5.0 {
                format!("{} \u{00b7} Low fee environment", v.bp_label)
            } else if (v.bp_systolic + diastolic) / 2.0 < 20.0 {
                format!("{} \u{00b7} Moderate fees", v.bp_label)
            } else {
                format!("{} \u{00b7} High fee pressure", v.bp_label)
            };
            set_bp_label.set(bp_context);
            set_bp_color.set(v.bp_color);

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

        let status_json = get_organism_status();
        if let Some(s) = parse_status_json(&status_json) {
            set_org_condition.set(s.condition);
            set_org_desc.set(s.description);
            set_org_color.set(s.color);
        }
    };

    // Forward live metrics to JS for color + vital signs computation.
    // Block detection is handled entirely by SSE (heartbeat.js connects
    // to /api/stats/heartbeat which streams real block data from ZMQ).
    // LiveStats only updates the vital signs panel and network stress color.
    Effect::new(move || {
        let Some(live) = cached_live.get() else {
            return;
        };

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
                (live.mempool.mempoolminfee * 1e8 / 100.0).round() / 10.0, // BTC/kB to sat/vB, rounded to 1dp
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
                // Poll the freshest block from the SSE/ZMQ timeline. Monotonic:
                // only advance, never regress, so a lagging source can't flicker
                // the header backwards.
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
                // Only promote "live" to "stale" from the JS stall detector so
                // v2 states ("syncing", "partial", "disconnected") stay authoritative.
                let js_cs = get_heartbeat_connection_state();
                if js_cs == "stale" && conn_state.get_untracked() == "live" {
                    set_conn_state.set("stale".to_string());
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

    let time_since = Signal::derive(move || {
        let _ = tick.get(); // re-run every tick
                            // Use the timestamp from whichever source reports the newer block, so
                            // the "last block" timer follows the SSE/ZMQ stream during node hiccups.
        let live = cached_live.get();
        let live_h = live.as_ref().map(|s| s.blockchain.blocks).unwrap_or(0);
        let live_t = live.as_ref().map(|s| s.blockchain.time).unwrap_or(0);
        let ts = if sse_height.get() > live_h {
            sse_ts.get()
        } else {
            live_t
        };
        if ts == 0 {
            return "waiting...".to_string();
        }
        let now = chrono::Utc::now().timestamp() as u64;
        let elapsed = now.saturating_sub(ts);
        if elapsed < 60 {
            format!("{}s ago", elapsed)
        } else {
            format!("{}m {}s ago", elapsed / 60, elapsed % 60)
        }
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
        <div class="space-y-6 relative min-[1800px]:w-[95vw] min-[1800px]:left-1/2 min-[1800px]:-translate-x-1/2">
            // Hero banner
            <div class="relative rounded-2xl overflow-hidden">
                <img
                    src="/img/observatory_hero.png"
                    alt="Block Heartbeat"
                    class="w-full h-[100px] sm:h-[120px] lg:h-[140px] object-cover object-center"
                />
                <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/60 to-[#123c64]/30"></div>
                <div class="absolute inset-0 flex flex-col items-center justify-end pb-3 sm:pb-4">
                    <h1 class="text-xl sm:text-2xl lg:text-3xl font-title text-white mb-1 drop-shadow-lg">"Block Heartbeat"</h1>
                    <p class="text-xs sm:text-sm text-white/60 max-w-xl mx-auto px-4 text-center drop-shadow">
                        "A live EKG of Bitcoin \u{b7} spikes are blocks, bricks are transactions"
                    </p>
                </div>
            </div>

            // EKG Canvas card
            // Mobile (<640px): viewport-filling so canvas stretches to just above controls.
            // Desktop: no height constraint; canvas wrap uses fixed 40vh instead.
            // Card fills the viewport below the nav + hero so ONLY the EKG is in
            // view on load; the vitals/rhythm/feed sit just below the fold. The
            // canvas flexes to fill the card minus the status + control + info
            // bars. (~210px subtracts nav + hero + top padding + the space-y gap;
            // dial it if the fit is slightly off.) min-h keeps it usable on short
            // screens (they scroll a little).
            <div id="heartbeat-card" class="relative bg-[#0d2137] border border-white/10 rounded-2xl overflow-hidden flex flex-col max-sm:h-[calc(100vh-180px)] max-sm:min-h-[350px] sm:h-[calc(100vh-210px)] sm:min-h-[420px]">
                // Status bar
                <div class="flex flex-wrap items-center justify-between px-3 sm:px-4 py-2 sm:py-2.5 gap-x-3 gap-y-1 border-b border-white/5">
                    <div class="flex items-center gap-2 sm:gap-3">
                        // `data-tip` and `tabindex` so the reason is reachable
                        // on hover, on tap and from the keyboard, the way every
                        // other explained term on the site is. A badge that
                        // says NODE STALE without saying what that means sends
                        // the reader to refresh the page, which cannot help.
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
                            <span class="text-xs text-white/50 font-mono">{status_label}</span>
                        </div>
                        <span class="text-xs sm:text-base text-[#00e676] font-mono font-semibold">{block_height}</span>
                    </div>
                    <div class="flex items-center gap-2 sm:gap-3 text-xs sm:text-base text-[#00e676] font-mono">
                        <span class="truncate">"Last block: " {time_since}</span>
                        // Fullscreen toggle: hidden on iOS where the API isn't supported
                        <button
                            id="heartbeat-fullscreen-btn"
                            class="text-white/30 hover:text-[#00e676] transition-colors cursor-pointer hidden"
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

                // Prominent TXID search row (always visible on mobile & desktop, including fullscreen)
                <div class="flex flex-wrap items-center gap-2 px-3 sm:px-4 py-1.5 sm:py-2 border-b border-white/5 bg-[#0a1929]/45">
                    <div class="relative flex-1 min-w-[190px] flex items-center">
                        <input
                            id="heartbeat-tx-search"
                            type="text"
                            placeholder="Search txid (full or 8+ char prefix)..."
                            aria-label="Search transaction ID"
                            class="w-full pl-3 pr-8 py-1.5 rounded-lg bg-white/5 border border-white/10 text-xs sm:text-sm font-mono text-white/90 placeholder-white/35 focus:border-[#00e676]/60 focus:bg-white/[0.07] focus:outline-none transition-colors"
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
                                            c.search_tx(&q);
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
                                class="absolute right-2 text-white/40 hover:text-white/80 text-sm font-mono px-1 cursor-pointer transition-colors"
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
                        class="px-3 py-1.5 rounded-lg bg-[#00e676]/15 hover:bg-[#00e676]/25 border border-[#00e676]/40 text-xs sm:text-sm font-mono font-semibold text-[#00e676] transition-colors cursor-pointer shrink-0"
                        on:click=move |_| {
                            #[cfg(feature = "hydrate")]
                            {
                                let q = search_query.get_untracked();
                                let client = hb_client.try_with_value(|c| c.clone()).flatten();
                                if let Some(c) = client {
                                    c.search_tx(&q);
                                }
                            }
                        }
                    >
                        "Search"
                    </button>
                    <Show when=move || search_feedback.get().is_some()>
                        {move || {
                            search_feedback.get().map(|(is_err, msg)| {
                                let pill_class = if is_err {
                                    "inline-flex items-center px-2.5 py-1 rounded-full text-[11px] font-mono bg-[#f44336]/15 border border-[#f44336]/40 text-[#ff8a80] max-w-full truncate"
                                } else {
                                    "inline-flex items-center px-2.5 py-1 rounded-full text-[11px] font-mono bg-[#00e676]/15 border border-[#00e676]/40 text-[#00e676] max-w-full truncate"
                                };
                                let title_msg = msg.clone();
                                view! {
                                    <span class=pill_class title=title_msg>
                                        {msg}
                                    </span>
                                }
                            })
                        }}
                    </Show>
                </div>

                // Canvas with overlays
                // The canvas flexes to fill all card space left over after the
                // status/search/control/info bars: on both mobile and desktop the card
                // is now viewport-height, so flex-1 absorbs the remainder. (Also
                // how it fills during fullscreen when JS sets the card to 100vh.)
                <div id="heartbeat-canvas-wrap" class="relative flex-1 min-h-0 sm:min-h-[250px]">
                    <canvas
                        id="heartbeat-canvas"
                        class="w-full h-full"
                    ></canvas>

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
                                let txid_locate = txid_full.clone();
                                let mempool_url = format!("https://mempool.space/tx/{txid_full}");

                                let fee_rate = tx.fee_rate_sat_per_vb();
                                let (band_label, band_color) = fee_band_for_rate(fee_rate);
                                let fee_rate_text = if fee_rate.is_some() {
                                    format!("{} \u{00b7} {}", format_fee_rate(fee_rate), band_label)
                                } else {
                                    band_label.to_string()
                                };
                                let fee_paid_text = tx
                                    .fee
                                    .map(|f| format!("{} sats", super::helpers::format_number(f)))
                                    .unwrap_or_else(|| "Pending...".to_string());
                                let size_text = tx
                                    .vsize
                                    .map(|v| format!("{} vB", super::helpers::format_number(u64::from(v))))
                                    .unwrap_or_else(|| "Pending...".to_string());
                                let value_text = tx
                                    .value
                                    .map(|sats| format!("{:.8} BTC", sats as f64 / 1e8))
                                    .unwrap_or_else(|| "Pending...".to_string());
                                let first_seen_text = format_first_seen(tx.first_seen);
                                let notable = primary_notable_label(tx.flags);
                                let op_return = tx
                                    .op_return_text
                                    .as_deref()
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_string);
                                let status_for_header = tx.status.clone();
                                let status_for_footer = tx.status.clone();
                                let deep = tx.deep.clone();
                                let deep_loading = tx.deep_loading;

                                view! {
                                    <div
                                        id="heartbeat-pinned-tx-panel"
                                        class="absolute top-3 right-3 z-20 w-[calc(100%-1.5rem)] max-w-sm bg-[#0a1929]/95 border border-white/15 rounded-xl shadow-2xl backdrop-blur-md text-xs font-mono text-white/85 overflow-hidden max-h-[calc(100%-1.5rem)] overflow-y-auto"
                                    >
                                        // Header: status badge + close button
                                        <div class="flex items-start justify-between gap-2 px-3.5 py-2.5 border-b border-white/10">
                                            <div class="flex flex-wrap items-center gap-1.5 min-w-0">
                                                {match status_for_header {
                                                    PinnedTxStatus::Unconfirmed => view! {
                                                        <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wider bg-[#00e676]/15 border border-[#00e676]/40 text-[#00e676]">
                                                            <span class="w-1.5 h-1.5 rounded-full bg-[#00e676]"></span>
                                                            "IN MEMPOOL"
                                                        </span>
                                                    }.into_any(),
                                                    PinnedTxStatus::Confirmed { block_height, .. } => {
                                                        let badge_text = if block_height > 0 {
                                                            format!(
                                                                "CONFIRMED IN BLOCK #{}",
                                                                super::helpers::format_number(block_height)
                                                            )
                                                        } else {
                                                            "CONFIRMED".to_string()
                                                        };
                                                        view! {
                                                            <div class="flex flex-wrap items-center gap-1.5">
                                                                <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wider bg-[#f7931a]/15 border border-[#f7931a]/40 text-[#f7931a]">
                                                                    <span class="w-1.5 h-1.5 rounded-full bg-[#00e676]"></span>
                                                                    {badge_text}
                                                                </span>
                                                                {(block_height > 0).then(|| view! {
                                                                    <button
                                                                        type="button"
                                                                        class="px-2 py-0.5 rounded bg-[#f7931a]/15 hover:bg-[#f7931a]/25 border border-[#f7931a]/40 text-[10px] font-semibold text-[#f7931a] cursor-pointer transition-colors"
                                                                        on:click=move |_| {
                                                                            let _ = hb_v2_focus_block_height(block_height);
                                                                            show_block_detail_js(block_height);
                                                                        }
                                                                    >
                                                                        "View block"
                                                                    </button>
                                                                })}
                                                            </div>
                                                        }.into_any()
                                                    }
                                                    PinnedTxStatus::Removed => view! {
                                                        <div class="flex flex-col gap-0.5">
                                                            <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wider bg-[#f44336]/15 border border-[#f44336]/35 text-[#ff8a80] w-fit">
                                                                "REMOVED FROM MEMPOOL"
                                                            </span>
                                                            <span class="text-[10px] text-white/45">"Replaced or evicted"</span>
                                                        </div>
                                                    }.into_any(),
                                                }}
                                            </div>
                                            <button
                                                type="button"
                                                aria-label="Close transaction panel"
                                                title="Close"
                                                class="w-6 h-6 shrink-0 rounded-md bg-white/5 hover:bg-white/15 text-white/60 hover:text-white flex items-center justify-center text-sm cursor-pointer transition-colors"
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
                                        <div class="px-3.5 py-2.5 space-y-2.5">
                                            // TXID + Copy
                                            <div class="flex items-center justify-between gap-2 bg-white/[0.04] border border-white/10 rounded-lg px-2.5 py-1.5">
                                                <span class="text-[11px] text-white/80 truncate" title=txid_full.clone()>
                                                    {txid_short}
                                                </span>
                                                <button
                                                    type="button"
                                                    class="px-2 py-0.5 rounded bg-white/10 hover:bg-white/20 text-[10px] text-white/80 cursor-pointer transition-colors shrink-0"
                                                    on:click=move |_| {
                                                        #[cfg(feature = "hydrate")]
                                                        {
                                                            if let Some(win) = leptos::web_sys::window() {
                                                                let clipboard = win.navigator().clipboard();
                                                                let _ = clipboard.write_text(&txid_copy);
                                                                set_copied_txid.set(true);
                                                                leptos::prelude::set_timeout(
                                                                    move || set_copied_txid.set(false),
                                                                    std::time::Duration::from_millis(1500),
                                                                );
                                                            }
                                                        }
                                                    }
                                                >
                                                    {move || if copied_txid.get() { "Copied" } else { "Copy" }}
                                                </button>
                                            </div>

                                            // Key metrics
                                            <div class="grid grid-cols-2 gap-x-3 gap-y-2 text-[11px]">
                                                <div>
                                                    <div class="text-[10px] text-white/40 uppercase">"Fee Paid"</div>
                                                    <div class="text-white/90 font-semibold tabular-nums">{fee_paid_text}</div>
                                                </div>
                                                <div>
                                                    <div class="text-[10px] text-white/40 uppercase">"Size"</div>
                                                    <div class="text-white/90 font-semibold tabular-nums">{size_text}</div>
                                                </div>
                                                <div class="col-span-2">
                                                    <div class="text-[10px] text-white/40 uppercase">"Fee Rate"</div>
                                                    <div class="flex items-center gap-1.5 text-white/90 font-semibold tabular-nums">
                                                        <span
                                                            class="w-2 h-2 rounded-full shrink-0"
                                                            style=format!("background-color: {band_color}")
                                                        ></span>
                                                        <span>{fee_rate_text}</span>
                                                    </div>
                                                </div>
                                                <div>
                                                    <div class="text-[10px] text-white/40 uppercase">"Output Value"</div>
                                                    <div class="text-white/90 font-semibold tabular-nums">{value_text}</div>
                                                </div>
                                                <div>
                                                    <div class="text-[10px] text-white/40 uppercase">"First Observed"</div>
                                                    <div class="text-white/75">{first_seen_text}</div>
                                                </div>
                                            </div>

                                            // Notable tag + OP_RETURN text (strictly text nodes, never inner_html)
                                            {notable.map(|(label, color)| view! {
                                                <div class="flex items-center gap-2 pt-0.5">
                                                    <span
                                                        class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wide border"
                                                        style=format!("color: {color}; border-color: {color}66; background-color: {color}1a")
                                                    >
                                                        {label}
                                                    </span>
                                                </div>
                                            })}
                                            {op_return.map(|msg| view! {
                                                <div class="rounded-lg bg-white/[0.04] border border-[#ffa500]/30 px-2.5 py-2 space-y-0.5">
                                                    <div class="text-[10px] text-[#ffa500] uppercase font-semibold">"OP_RETURN Message"</div>
                                                    <p class="text-[11px] text-white/80 break-words whitespace-pre-wrap">{msg}</p>
                                                </div>
                                            })}

                                            // Deep details from /api/stats/tx/{txid}
                                            {if let Some(d) = deep {
                                                let inputs_str = d
                                                    .inputs
                                                    .map(super::helpers::format_number)
                                                    .unwrap_or_else(|| "--".to_string());
                                                let outputs_str = d
                                                    .outputs
                                                    .map(super::helpers::format_number)
                                                    .unwrap_or_else(|| "--".to_string());
                                                let weight_str = d
                                                    .weight
                                                    .map(|w| format!("{} WU", super::helpers::format_number(w)))
                                                    .unwrap_or_else(|| "--".to_string());
                                                view! {
                                                    <div class="grid grid-cols-3 gap-2 pt-2 border-t border-white/10 text-[11px]">
                                                        <div>
                                                            <div class="text-[10px] text-white/40 uppercase">"Inputs"</div>
                                                            <div class="text-white/85 tabular-nums">{inputs_str}</div>
                                                        </div>
                                                        <div>
                                                            <div class="text-[10px] text-white/40 uppercase">"Outputs"</div>
                                                            <div class="text-white/85 tabular-nums">{outputs_str}</div>
                                                        </div>
                                                        <div>
                                                            <div class="text-[10px] text-white/40 uppercase">"Weight"</div>
                                                            <div class="text-white/85 tabular-nums">{weight_str}</div>
                                                        </div>
                                                    </div>
                                                }.into_any()
                                            } else if deep_loading {
                                                view! {
                                                    <div class="pt-1.5 border-t border-white/10 text-[10px] text-white/40">
                                                        "Loading inputs, outputs, weight..."
                                                    </div>
                                                }.into_any()
                                            } else {
                                                view! { <div></div> }.into_any()
                                            }}
                                        </div>

                                        // Footer actions
                                        <div class="flex flex-wrap items-center justify-between gap-2 px-3.5 py-2 border-t border-white/10 bg-white/[0.02]">
                                            <div class="flex items-center gap-2">
                                                {match status_for_footer {
                                                    PinnedTxStatus::Unconfirmed => view! {
                                                        <button
                                                            type="button"
                                                            class="px-2.5 py-1 rounded-md bg-[#00e676]/15 hover:bg-[#00e676]/25 border border-[#00e676]/40 text-[11px] font-semibold text-[#00e676] cursor-pointer transition-colors"
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
                                                            class="px-2.5 py-1 rounded-md bg-[#f7931a]/15 hover:bg-[#f7931a]/25 border border-[#f7931a]/40 text-[11px] font-semibold text-[#f7931a] cursor-pointer transition-colors"
                                                            on:click=move |_| {
                                                                let _ = hb_v2_focus_block_height(block_height);
                                                                show_block_detail_js(block_height);
                                                            }
                                                        >
                                                            "View block"
                                                        </button>
                                                    }.into_any(),
                                                    _ => view! { <span></span> }.into_any(),
                                                }}
                                            </div>
                                            <a
                                                href=mempool_url
                                                target="_blank"
                                                rel="noopener noreferrer"
                                                class="text-[11px] text-white/50 hover:text-[#f7931a] transition-colors"
                                            >
                                                "mempool.space ->"
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
                    // First-visit hint overlay (see show_hint above). Dismissing
                    // it stamps the localStorage flag so it never auto-shows again;
                    // the (i) button in the control bar reopens it.
                    <Show when=move || show_hint.get()>
                        <div
                            class="absolute inset-0 z-20 flex items-center justify-center bg-[#0d2137]/90 cursor-pointer"
                            on:click=move |_| {
                                #[cfg(feature = "hydrate")]
                                mark_hint_seen();
                                set_show_hint.set(false);
                            }
                        >
                            <div class="max-w-sm mx-4 bg-[#0a1a2e] border border-white/15 rounded-xl p-5 sm:p-6 shadow-2xl space-y-4 text-center">
                                <p class="text-white/90 text-sm sm:text-base leading-relaxed">
                                    "Each "
                                    <span class="text-[#f7931a] font-semibold">"spike"</span>
                                    " is a block. Each "
                                    <span class="text-[#f7931a] font-semibold">"brick"</span>
                                    " is a transaction."
                                </p>
                                <div class="text-white/50 text-xs sm:text-sm space-y-1.5">
                                    <p>"Drag to scroll through history"</p>
                                    <p>"Scroll wheel or pinch to zoom"</p>
                                    <p>"Click a spike for block details"</p>
                                    <p>"Click a brick for transaction info"</p>
                                </div>
                                <p class="text-white/30 text-[10px] sm:text-xs pt-2 border-t border-white/10">"Tap anywhere to dismiss"</p>
                            </div>
                        </div>
                    </Show>

                </div>

                // Control bar (HTML, outside canvas): one centered cluster, zoom,
                // then playback, then the LIVE follow-pill + Fit at the right end.
                <div class="flex items-center justify-center px-3 py-2 border-t border-white/5">
                    <div class="flex items-center gap-1 rounded-lg bg-[#0a1929]/70 border border-white/10 px-1.5 py-1 backdrop-blur-sm">
                        // ── Zoom ──
                        <button aria-label="Zoom out" onclick="handleControlClick('zoomOut')"
                            class="group relative w-8 h-8 rounded-md bg-white/5 text-white/70 hover:bg-white/15 hover:text-white transition-all cursor-pointer flex items-center justify-center text-sm">
                            "\u{2212}"
                            <span class="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded bg-[#0a1929] border border-white/10 text-[10px] text-white/80 whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-10">"Zoom out"</span>
                        </button>
                        <span id="heartbeat-zoom-label" class="hidden sm:inline text-[10px] text-white/40 font-mono w-9 text-center select-none">"1.9x"</span>
                        <button aria-label="Zoom in" onclick="handleControlClick('zoomIn')"
                            class="group relative w-8 h-8 rounded-md bg-white/5 text-white/70 hover:bg-white/15 hover:text-white transition-all cursor-pointer flex items-center justify-center text-sm">
                            "+"
                            <span class="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded bg-[#0a1929] border border-white/10 text-[10px] text-white/80 whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-10">"Zoom in"</span>
                        </button>

                        <div class="w-px h-5 bg-white/10 mx-1"></div>

                        // ── Playback ──
                        <button aria-label="Previous block" onclick="handleControlClick('prev')"
                            class="group relative w-8 h-8 rounded-md bg-white/5 text-white/70 hover:bg-white/15 hover:text-white transition-all cursor-pointer flex items-center justify-center text-sm">
                            "\u{23EE}"
                            <span class="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded bg-[#0a1929] border border-white/10 text-[10px] text-white/80 whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-10">"Previous block"</span>
                        </button>
                        <button id="heartbeat-btn-pause" aria-label="Pause or play" onclick="handleControlClick('pause')"
                            class="group relative w-8 h-8 rounded-md bg-[#f7931a]/20 text-[#f7931a] hover:bg-[#f7931a]/30 transition-all cursor-pointer flex items-center justify-center text-sm">
                            <span id="heartbeat-btn-pause-icon">"\u{23F8}"</span>
                            <span class="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded bg-[#0a1929] border border-white/10 text-[10px] text-white/80 whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-10">"Pause / play"</span>
                        </button>

                        <div class="w-px h-5 bg-white/10 mx-1"></div>

                        // ── View: LIVE follow-pill + Fit (right end) ──
                        <button id="heartbeat-btn-live" aria-label="Follow the live head" onclick="handleControlClick('live')"
                            class="group relative flex items-center gap-1.5 h-8 px-2.5 rounded-md border text-[11px] font-mono font-semibold tracking-wider cursor-pointer transition-all"
                            style="color:#00e676;background:rgba(0,230,118,0.12);border-color:rgba(0,230,118,0.4)">
                            <span id="heartbeat-live-dot" class="w-1.5 h-1.5 rounded-full" style="background:#00e676"></span>
                            "LIVE"
                            <span class="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded bg-[#0a1929] border border-white/10 text-[10px] text-white/80 whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-10">"Follow the live head"</span>
                        </button>
                        <button aria-label="Fit whole mempool" onclick="handleControlClick('fit')"
                            class="group relative w-8 h-8 rounded-md bg-white/5 text-white/70 hover:bg-white/15 hover:text-white transition-all cursor-pointer flex items-center justify-center text-sm">
                            "\u{26F6}"
                            <span class="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded bg-[#0a1929] border border-white/10 text-[10px] text-white/80 whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-10">"Fit whole mempool"</span>
                        </button>

                        <div class="w-px h-5 bg-white/10 mx-1"></div>

                        // ── Block sound: a monitor beep when a block is mined ──
                        // Ships in the off state; initSoundButton() repaints the icon
                        // from localStorage on load. The click is also what unlocks the
                        // AudioContext, which the autoplay policy requires.
                        <button
                            id="heartbeat-btn-sound"
                            aria-label="Sound on a new block"
                            aria-pressed="false"
                            title="Block sound off"
                            onclick="handleControlClick('sound')"
                            class="group relative w-8 h-8 rounded-md bg-white/5 text-white/70 hover:bg-white/15 hover:text-white transition-all cursor-pointer flex items-center justify-center text-sm"
                        >
                            <span id="heartbeat-btn-sound-icon">"\u{1F507}"</span>
                            <span class="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded bg-[#0a1929] border border-white/10 text-[10px] text-white/80 whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-10">"Sound on a new block"</span>
                        </button>

                        // ── Help: reopen the intro hint ──
                        <button aria-label="How to read this" title="How to read this"
                            on:click=move |_| set_show_hint.set(true)
                            class="group relative w-8 h-8 rounded-md bg-white/5 text-white/70 hover:bg-white/15 hover:text-white transition-all cursor-pointer flex items-center justify-center text-sm font-serif italic">
                            "i"
                            <span class="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1 rounded bg-[#0a1929] border border-white/10 text-[10px] text-white/80 whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity duration-75 z-10">"How to read this"</span>
                        </button>
                    </div>
                </div>

                // Bottom info bar: node-aggregate mempool total on the left,
                // next-block fee estimate on the right.
                <div class="flex flex-wrap items-center justify-between gap-x-3 gap-y-1 px-3 sm:px-4 py-1.5 sm:py-2 border-t border-white/5 text-xs sm:text-base text-[#00e676] font-mono">
                    <span class="whitespace-nowrap">{mempool_display}</span>
                    <span class="whitespace-nowrap">"Next block: " {fee_display}</span>
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
                    tip="Average time between blocks this difficulty period. Target is 10:00. Below 10:00 (Tachycardia) = blocks found faster than expected. Above 10:00 (Bradycardia) = slower. Adjusts every 2,016 blocks."
                />

                // Blood Pressure
                <VitalTile
                    label="Blood Pressure"
                    value=bp_display
                    unit=" sat/vB"
                    status=bp_label
                    color=bp_color
                    tip="Fee pressure. Systolic (left) = next block fee rate. Diastolic (right) = mempool minimum fee. Higher numbers mean it costs more to transact."
                />

                // Temperature (mempool)
                <VitalTile
                    label="Temperature"
                    value=temp_display
                    unit=" vMB"
                    status=temp_label
                    color=temp_color
                    subtitle=Signal::derive(move || temp_subtitle.get())
                    tip="Mempool size in virtual megabytes. Under 10 vMB is calm. Over 100 vMB means congestion and rising fees. Temperature maps this to a human-readable scale."
                />

                // Immune System
                <VitalTile
                    label="Immune System"
                    value=immune_display
                    unit=""
                    status=immune_label
                    color=immune_color
                    tip="Network hashrate, the total computational power securing Bitcoin. Higher = more resilient against attacks. Measured in exahashes per second."
                />
            </div>

            // ── Phase 3: 24-Hour Rhythm Strip ─────────────────
            <div class="bg-[#0d2137] border border-white/10 rounded-2xl overflow-hidden">
                <div class="flex flex-col sm:flex-row sm:items-baseline sm:justify-between px-3 sm:px-4 py-2 gap-0.5 sm:gap-0 border-b border-white/5">
                    <span class="text-xs text-white/40 font-mono">"24-HOUR RHYTHM STRIP"</span>
                    <span class="text-[10px] sm:text-[11px] text-white/40 font-mono">"Last 144 blocks \u{00b7} one full difficulty day"</span>
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
                        style="opacity: 0.6"
                    />
                    <div id="rhythm-strip-detail" class="flex flex-wrap items-baseline gap-x-4 gap-y-0.5 mt-1 min-h-[20px] text-xs font-mono text-white/50">
                    </div>
                </div>
            </div>

            // ── Phase 4: Organism Status ──────────────────────
            <div class="bg-[#0d2137]/60 border border-white/5 rounded-xl px-5 py-4"
                 data-tip="Overall network health derived from block timing, fee pressure, mempool congestion, and hashrate. Bitcoin as a living organism: this is its diagnosis."
                 tabindex="0"
            >
                <div class="flex items-baseline gap-2">
                    <span class="text-xs text-white/50 font-mono uppercase tracking-wider">"Organism Status:"</span>
                    <span
                        class="text-sm font-mono font-semibold"
                        style=move || format!("color: {}", org_color.get())
                    >
                        {org_condition}
                    </span>
                </div>
                <p class="text-sm text-white/40 italic mt-1 font-mono">
                    {org_desc}
                </p>
            </div>

            // ── The Lookout Feed ──────────────────────────────
            <div id="whale-feed-panel" class="bg-[#0d2137] border border-[#ffd700]/20 rounded-2xl overflow-hidden hidden">
                // Header: stacks label above buttons on mobile, inline on sm+
                <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 px-4 py-2.5 border-b border-[#ffd700]/10">
                    <div class="flex items-center gap-2 shrink-0">
                        <span class="w-2 h-2 rounded-full bg-[#ffd700] animate-pulse"></span>
                        <span class="text-xs font-mono text-[#ffd700]/80 uppercase tracking-wider">"The Lookout"</span>
                    </div>
                    // Horizontal-scroll row on mobile; wraps inline on sm+
                    <div class="flex items-center gap-1 overflow-x-auto scrollbar-hide -mx-4 px-4 sm:mx-0 sm:px-0 sm:flex-wrap sm:justify-end">
                        <button onclick="window._filterNotable('all')" id="whale-filter-all" title="Show all notable transactions" class="px-2 py-0.5 rounded text-[10px] font-mono whitespace-nowrap shrink-0 bg-white/10 text-white/60 hover:bg-white/20 transition-colors border border-white/[0.08]">"All"</button>
                        <button onclick="window._filterNotable('whale')" id="whale-filter-whale" title="Whales: total output value over $1,000,000 USD. May include exchange self-sends where no value actually changed hands." class="px-2 py-0.5 rounded text-[10px] font-mono whitespace-nowrap shrink-0 bg-transparent border border-white/[0.08] text-[#ffd700]/50 hover:bg-[#ffd700]/10 transition-colors">"Whales"</button>
                        <button onclick="window._filterNotable('round_number')" id="whale-filter-round_number" title="Round-number transfers: exact 1, 10, 100, or 1000 BTC output amounts. Often human-initiated vs exchange automation." class="px-2 py-0.5 rounded text-[10px] font-mono whitespace-nowrap shrink-0 bg-transparent border border-white/[0.08] text-[#90ee90]/60 hover:bg-[#90ee90]/10 transition-colors">"Round #"</button>
                        <button onclick="window._filterNotable('inscription')" id="whale-filter-inscription" title="Large inscriptions: witness data over 100KB (Ordinals, BRC-20, images, JSON)" class="px-2 py-0.5 rounded text-[10px] font-mono whitespace-nowrap shrink-0 bg-transparent border border-white/[0.08] text-[#ff00c8]/50 hover:bg-[#ff00c8]/10 transition-colors">"Inscr."</button>
                        <button onclick="window._filterNotable('consolidation')" id="whale-filter-consolidation" title="Consolidations: 50+ inputs merged into 3 or fewer outputs (exchange cold wallet sweeps, UTXO cleanup)" class="px-2 py-0.5 rounded text-[10px] font-mono whitespace-nowrap shrink-0 bg-transparent border border-white/[0.08] text-[#a855f7]/50 hover:bg-[#a855f7]/10 transition-colors">"Consol."</button>
                        <button onclick="window._filterNotable('fan_out')" id="whale-filter-fan_out" title="Fan-outs: 3 or fewer inputs sprayed to 100+ outputs (exchange batch payouts, mining rewards, airdrops)" class="px-2 py-0.5 rounded text-[10px] font-mono whitespace-nowrap shrink-0 bg-transparent border border-white/[0.08] text-[#00d2ff]/50 hover:bg-[#00d2ff]/10 transition-colors">"Fan-out"</button>
                        <button onclick="window._filterNotable('fee')" id="whale-filter-fee" title="Fee outliers: fee rate over 2000 sat/vB or absolute fee over 0.1 BTC" class="px-2 py-0.5 rounded text-[10px] font-mono whitespace-nowrap shrink-0 bg-transparent border border-white/[0.08] text-[#ff4444]/50 hover:bg-[#ff4444]/10 transition-colors">"Fees"</button>
                        <button onclick="window._filterNotable('op_return')" id="whale-filter-op_return" title="OP_RETURN messages: transactions embedding readable ASCII text on-chain" class="px-2 py-0.5 rounded text-[10px] font-mono whitespace-nowrap shrink-0 bg-transparent border border-white/[0.08] text-[#ffa500]/60 hover:bg-[#ffa500]/10 transition-colors">"Messages"</button>
                    </div>
                </div>
                <div id="whale-feed-list" class="max-h-[240px] overflow-y-auto">
                    <div data-placeholder="1" class="px-4 py-3 text-xs text-white/20 font-mono italic text-center">"Listening for notable transactions..."</div>
                </div>
                <div class="px-3 py-1.5 border-t border-white/5 text-[10px] font-mono text-white/30 text-center">
                    <a href="/observatory/lookout" class="hover:text-[#f7931a] transition-colors">"Visit The Lookout for history and stats \u{2192}"</a>
                </div>
            </div>

            // ── Phase 5: Capture controls (commented out; code kept for later) ──
            /* <div class="flex flex-wrap items-center justify-center gap-3">
                // Capture moment
                <button
                    class="flex items-center gap-2 px-4 py-2 rounded-lg border border-white/10 bg-[#0d2137]/60 text-xs font-mono text-white/50 hover:text-white/80 hover:border-white/20 transition-colors"
                    on:click=move |_| {
                        let vitals = get_heartbeat_vitals();
                        heartbeat_download_capture(&vitals);
                    }
                >
                    <span class="text-base">{"\u{1F4F7}"}</span>
                    <span>"Capture Moment"</span>
                </button>
            </div> */

            // Legends: Transaction Fee Bands + Network Stress
            <div class="space-y-5">
                // Fixed Fee Rate Legend
                <div class="space-y-2">
                    <div class="text-center">
                        <span class="text-xs text-white/50 font-mono uppercase tracking-wider">"Transaction Fee Bands (sat/vB)"</span>
                    </div>
                    <div class="flex flex-wrap justify-center gap-x-5 gap-y-2 text-xs text-white/50 font-mono">
                        <div class="flex items-center gap-1.5">
                            <span class="w-2.5 h-2.5 rounded-sm bg-[#64748b]"></span>
                            "Pending"
                        </div>
                        <div class="flex items-center gap-1.5">
                            <span class="w-2.5 h-2.5 rounded-sm bg-[#2563eb]"></span>
                            "< 1"
                        </div>
                        <div class="flex items-center gap-1.5">
                            <span class="w-2.5 h-2.5 rounded-sm bg-[#38bdf8]"></span>
                            "1-2"
                        </div>
                        <div class="flex items-center gap-1.5">
                            <span class="w-2.5 h-2.5 rounded-sm bg-[#00e676]"></span>
                            "2-5"
                        </div>
                        <div class="flex items-center gap-1.5">
                            <span class="w-2.5 h-2.5 rounded-sm bg-[#f7931a]"></span>
                            "5-15"
                        </div>
                        <div class="flex items-center gap-1.5">
                            <span class="w-2.5 h-2.5 rounded-sm bg-[#ff5722]"></span>
                            "15-50"
                        </div>
                        <div class="flex items-center gap-1.5">
                            <span class="w-2.5 h-2.5 rounded-sm bg-[#f44336]"></span>
                            "50+"
                        </div>
                    </div>
                    <p class="text-center text-[11px] text-white/40 max-w-lg mx-auto">
                        "Larger outlined bricks highlight notable transactions (whales, inscriptions, messages, fee outliers)."
                    </p>
                </div>

                // Network Stress Legend
                <div class="space-y-2">
                    <div class="text-center">
                        <span class="text-xs text-white/50 font-mono uppercase tracking-wider">"Network Stress"</span>
                    </div>
                    <div class="flex flex-wrap justify-center gap-x-5 gap-y-2 text-xs text-white/50">
                        <div class="flex items-center gap-1.5">
                            <span class="w-2.5 h-2.5 rounded-full bg-[#00e676]"></span>
                            "Healthy"
                        </div>
                        <div class="flex items-center gap-1.5">
                            <span class="w-2.5 h-2.5 rounded-full bg-[#42a5f5]"></span>
                            "Steady"
                        </div>
                        <div class="flex items-center gap-1.5">
                            <span class="w-2.5 h-2.5 rounded-full bg-[#f7931a]"></span>
                            "Elevated"
                        </div>
                        <div class="flex items-center gap-1.5">
                            <span class="w-2.5 h-2.5 rounded-full bg-[#ff5722]"></span>
                            "Stressed"
                        </div>
                        <div class="flex items-center gap-1.5">
                            <span class="w-2.5 h-2.5 rounded-full bg-[#f44336]"></span>
                            "Critical"
                        </div>
                    </div>
                    <p class="text-center text-[11px] text-white/40 max-w-md mx-auto">
                        "Color is derived from time since last block, fee pressure, and mempool congestion. It affects the flatline, waveform, and live indicator."
                    </p>
                </div>
            </div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Vital Signs Tile component
// ---------------------------------------------------------------------------

#[component]
fn VitalTile(
    label: &'static str,
    #[prop(into)] value: Signal<String>,
    unit: &'static str,
    #[prop(into)] status: Signal<String>,
    color: ReadSignal<String>,
    #[prop(optional, into)] subtitle: Option<Signal<String>>,
    #[prop(optional)] tip: Option<&'static str>,
) -> impl IntoView {
    view! {
        <div
            class="bg-[#0d2137] border border-white/10 rounded-xl px-3 sm:px-4 py-2.5 sm:py-3 flex flex-col gap-1 min-w-0"
            data-tip=tip.unwrap_or("")
            tabindex=if tip.is_some() { "0" } else { "-1" }
        >
            <span class="text-xs text-white/50 font-mono uppercase tracking-wider">{label}</span>
            <div class="flex items-baseline gap-1">
                <span
                    class="text-xl sm:text-3xl font-mono font-bold tabular-nums"
                    style=move || format!("color: {}", color.get())
                >
                    {value}
                </span>
                <span
                    class="text-xs font-mono"
                    style=move || format!("color: {}80", color.get())
                >
                    {unit}
                </span>
            </div>
            <span
                class="text-[10px] sm:text-xs font-mono truncate"
                style=move || format!("color: {}99", color.get())
            >
                {status}
            </span>
            {subtitle.map(|sub| view! {
                <span class="text-[11px] text-white/50 font-mono">{sub}</span>
            })}
        </div>
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Whether the visitor has already dismissed the intro hint (localStorage flag).
/// Hydrate-only: the server never reads browser storage.
#[cfg(feature = "hydrate")]
fn hint_already_seen() -> bool {
    leptos::prelude::window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item("hb_intro_seen").ok().flatten())
        .as_deref()
        == Some("1")
}

/// Persist that the intro hint has been dismissed so it never auto-shows again.
#[cfg(feature = "hydrate")]
fn mark_hint_seen() {
    if let Ok(Some(s)) = leptos::prelude::window().local_storage() {
        let _ = s.set_item("hb_intro_seen", "1");
    }
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
/// Computes inter-block time from consecutive block timestamps.
fn blocks_to_json(blocks: &[crate::stats::types::BlockSummary]) -> String {
    use std::fmt::Write;
    let mut buf = String::from("[");
    for (i, b) in blocks.iter().enumerate() {
        let prev_ts = if i > 0 {
            blocks[i - 1].timestamp
        } else {
            b.timestamp.saturating_sub(600)
        };
        let inter = b.timestamp.saturating_sub(prev_ts);
        if i > 0 {
            buf.push(',');
        }
        let _ = write!(
            buf,
            r#"{{"height":{},"timestamp":{},"tx_count":{},"total_fees":{},"size":{},"weight":{},"inter_block_seconds":{}}}"#,
            b.height,
            b.timestamp,
            b.tx_count,
            b.total_fees,
            b.size,
            b.weight,
            inter
        );
    }
    buf.push(']');
    buf
}

/// Minimal JSON parsing for vital signs (avoids serde dependency).
struct Vitals {
    bp_systolic: f64,
    bp_label: String,
    bp_color: String,
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
        bp_systolic: extract_json_f64(json, "bp_systolic"),
        bp_label: extract_json_str(json, "bp_label"),
        bp_color: extract_json_str(json, "bp_color"),
        temp_c: extract_json_f64(json, "temp_c"),
        temp_label: extract_json_str(json, "temp_label"),
        temp_color: extract_json_str(json, "temp_color"),
        immune_eh: extract_json_f64(json, "immune_eh"),
        immune_label: extract_json_str(json, "immune_label"),
        immune_color: extract_json_str(json, "immune_color"),
    })
}

struct OrganismStatus {
    condition: String,
    description: String,
    color: String,
}

fn short_txid(txid: &str) -> String {
    if txid.len() > 24 {
        format!("{}...{}", &txid[..12], &txid[txid.len() - 8..])
    } else {
        txid.to_string()
    }
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

fn parse_status_json(json: &str) -> Option<OrganismStatus> {
    if json.len() < 3 {
        return None;
    }
    Some(OrganismStatus {
        condition: extract_json_str(json, "condition"),
        description: extract_json_str(json, "description"),
        color: extract_json_str(json, "color"),
    })
}

#[cfg(test)]
mod tests {
    use super::{format_mempool_summary, status_for, MempoolDisplay};
    use crate::stats::types::{
        LiveBlockchain, LiveMempool, LiveNetwork, LiveStats,
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
}
