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

// ---------------------------------------------------------------------------
// JS interop — heartbeat.js functions
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

    #[wasm_bindgen(js_name = getHeartbeatRecentBlocks)]
    fn get_heartbeat_recent_blocks() -> String;

    #[wasm_bindgen(js_name = getOrganismStatus)]
    fn get_organism_status() -> String;

    // Capture
    #[wasm_bindgen(js_name = heartbeatDownloadCapture)]
    fn heartbeat_download_capture(vitals_json: &str);

    // TX search
    #[wasm_bindgen(js_name = heartbeatSearchTx)]
    fn heartbeat_search_tx(txid: &str) -> bool;
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
#[allow(dead_code)]
fn get_heartbeat_recent_blocks() -> String {
    "[]".to_string()
}
#[cfg(not(feature = "hydrate"))]
fn get_organism_status() -> String {
    "{}".to_string()
}
#[cfg(not(feature = "hydrate"))]
fn heartbeat_download_capture(_: &str) {}
#[cfg(not(feature = "hydrate"))]
#[allow(dead_code)]
fn heartbeat_search_tx(_: &str) -> bool {
    false
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

    // Signals for vital signs display
    let (hr_display, set_hr_display) = signal("--:--".to_string());
    let (hr_label, set_hr_label) = signal("Waiting...".to_string());
    let (hr_color, set_hr_color) = signal("#00e676".to_string());
    let (hr_subtitle, set_hr_subtitle) = signal(String::new());

    let (bp_display, set_bp_display) = signal("-- / --".to_string());
    let (bp_label, set_bp_label) = signal("Waiting...".to_string());
    let (bp_color, set_bp_color) = signal("#00e676".to_string());

    let (temp_display, set_temp_display) = signal("--.-".to_string());
    let (temp_label, set_temp_label) = signal("Waiting...".to_string());
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

    // Fetch blocks for initial timeline (current retarget period = 2016 blocks)
    let initial_blocks = LocalResource::new(move || async move {
        let height =
            cached_live.get().map(|s| s.blockchain.blocks).unwrap_or(0);
        if height == 0 {
            return Vec::new();
        }
        // Fetch last 2016 blocks for timeline history
        let from = height.saturating_sub(RETARGET_PERIOD);
        crate::stats::server_fns::fetch_blocks(from, height)
            .await
            .unwrap_or_default()
    });

    // Initialize canvas and push initial blocks once data is ready
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
            if blocks.is_empty() {
                return;
            }

            init_heartbeat("heartbeat-canvas");
            initialized.set(true);
            set_loading.set(false);

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

            // Store last height — use the latest from LiveStats (not the last replayed block)
            // to avoid missing blocks that arrived between fetch and now
            let live_height = cached_live
                .get_untracked()
                .map(|s| s.blockchain.blocks)
                .unwrap_or(0);
            let replay_height = blocks.last().map(|b| b.height).unwrap_or(0);
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
                    set_hr_subtitle.set(format!("{:.0}% of target rate", bpm * 100.0));
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

            // Temperature: show mempool stats as main, temp as subtitle
            if let Some(live) = cached_live.get_untracked() {
                let vmb = live.mempool.bytes as f64 / 1_000_000.0;
                set_temp_display.set(format!("{:.1}", vmb));
                set_temp_subtitle.set(format!(
                    "{:.1}\u{00B0}C \u{00b7} {}",
                    v.temp_c, v.temp_label
                ));
            } else {
                set_temp_display.set(format!("{:.1}", v.temp_c));
                set_temp_subtitle.set(String::new());
            }
            set_temp_label.set(format!(
                "{} tx in mempool",
                cached_live
                    .get_untracked()
                    .map(|s| super::helpers::format_number(s.mempool.size))
                    .unwrap_or_else(|| "--".to_string())
            ));
            set_temp_color.set(v.temp_color);

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

        // Skip when RPC calls failed (server returns zeroed defaults).
        // Retaining the previous JS state keeps vital signs + bottom bar
        // showing the last known good values instead of 0s.
        let rpc_failed = live.mempool.size == 0
            && live.network.hashrate == 0.0
            && live.next_block_fee == 0.0;
        if rpc_failed {
            return;
        }

        // Forward live metrics for color + vital signs computation
        let live_json = format!(
            r#"{{"next_block_fee":{},"mempool_mb":{:.1},"block_time":{},"hashrate_eh":{:.1},"mempool_min_fee":{},"difficulty":{},"block_height":{}}}"#,
            live.next_block_fee,
            live.mempool.bytes as f64 / 1_000_000.0,
            live.blockchain.time,
            live.network.hashrate / 1e18,
            (live.mempool.mempoolminfee * 1e8 / 100.0).round() / 10.0, // BTC/kB to sat/vB, rounded to 1dp
            live.blockchain.difficulty,
            live.blockchain.blocks,
        );
        update_heartbeat_live(&live_json);

        // Refresh vitals display
        refresh_vitals();
    });

    // Cleanup animation on navigate away
    on_cleanup(|| {
        destroy_heartbeat();
    });

    // Reactive display values
    let block_height = Signal::derive(move || {
        cached_live
            .get()
            .map(|s| {
                format!(
                    "#{}",
                    super::helpers::format_number(s.blockchain.blocks)
                )
            })
            .unwrap_or_else(|| "---".to_string())
    });

    // Tick counter that increments every second for live countdown
    let (tick, set_tick) = signal(0u64);
    let (last_block_ts, set_last_block_ts) = signal(0u64);

    #[cfg(feature = "hydrate")]
    {
        let handle = leptos::prelude::set_interval_with_handle(
            move || set_tick.update(|t| *t += 1),
            std::time::Duration::from_secs(1),
        );
        on_cleanup(move || {
            if let Ok(h) = handle {
                h.clear();
            }
        });
    }

    // Update stored timestamp when LiveStats refreshes
    Effect::new(move || {
        if let Some(s) = cached_live.get() {
            set_last_block_ts.set(s.blockchain.time);
        }
    });

    let time_since = Signal::derive(move || {
        let _ = tick.get(); // re-run every tick
        let ts = last_block_ts.get();
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

    let mempool_display = Signal::derive(move || {
        cached_live
            .get()
            .filter(|s| s.mempool.size > 0 || s.network.hashrate > 0.0)
            .map(|s| {
                format!(
                    "{} tx ({:.1} vMB)",
                    super::helpers::format_number(s.mempool.size),
                    s.mempool.bytes as f64 / 1_000_000.0
                )
            })
            .unwrap_or_else(|| "-- tx (-- vMB)".to_string())
    });

    let fee_display = Signal::derive(move || {
        cached_live
            .get()
            .filter(|s| s.mempool.size > 0 || s.next_block_fee > 0.0)
            .map(|s| format!("{:.1} sat/vB", s.next_block_fee))
            .unwrap_or_else(|| "-- sat/vB".to_string())
    });

    view! {
        <Title text="Block Heartbeat | WE HODL BTC"/>
        <Meta name="description" content="Watch Bitcoin breathe. A live EKG visualization of block arrivals, where every spike tells a story of transactions, fees, and network activity."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/heartbeat"/>

        <div class="space-y-6">
            // Hero banner
            <div class="relative rounded-2xl overflow-hidden">
                <img
                    src="/img/observatory_hero.png"
                    alt="Block Heartbeat"
                    class="w-full h-[100px] sm:h-[120px] lg:h-[140px] object-cover object-center"
                />
                <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/60 to-[#123c64]/30"></div>
                <div class="absolute inset-0 flex flex-col items-center justify-end pb-3 sm:pb-4">
                    <h1 class="text-base sm:text-xl lg:text-2xl font-title text-white mb-0.5 drop-shadow-lg">"Block Heartbeat"</h1>
                    <p class="text-[10px] sm:text-xs text-white/60 max-w-lg mx-auto px-4 text-center drop-shadow">
                        "A live EKG of the Bitcoin network. Each spike is a block, each brick is a transaction."
                    </p>
                </div>
            </div>

            // EKG Canvas card
            // Mobile (<640px): viewport-filling so canvas stretches to just above controls.
            // Desktop: no height constraint; canvas wrap uses fixed 40vh instead.
            <div id="heartbeat-card" class="relative bg-[#0d2137] border border-white/10 rounded-2xl overflow-hidden flex flex-col max-sm:h-[calc(100vh-180px)] max-sm:min-h-[350px]">
                // Status bar
                <div class="flex flex-wrap items-center justify-between px-3 sm:px-4 py-2 sm:py-2.5 gap-x-3 gap-y-1 border-b border-white/5">
                    <div class="flex items-center gap-2 sm:gap-3">
                        <div class="flex items-center gap-1.5">
                            <span class="relative flex h-2 w-2">
                                <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-[#00e676] opacity-60"></span>
                                <span class="relative inline-flex rounded-full h-2 w-2 bg-[#00e676]"></span>
                            </span>
                            <span class="text-xs text-white/50 font-mono">"LIVE"</span>
                        </div>
                        <span class="text-xs sm:text-base text-[#00e676] font-mono font-semibold">{block_height}</span>
                    </div>
                    <div class="flex items-center gap-2 sm:gap-3 text-xs sm:text-base text-[#00e676] font-mono">
                        <span class="truncate">"Last block: " {time_since}</span>
                        // Fullscreen toggle — hidden on iOS where the API isn't supported
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

                // Canvas with overlays
                // Mobile (<640px): flex-1 fills remaining card space above controls
                // (card is viewport-height, so flex-1 absorbs all leftover space).
                // Desktop: fixed h-[40vh] with min-h-[250px]. flex-1 kicks in during
                // fullscreen when JS sets card to 100vh.
                <div id="heartbeat-canvas-wrap" class="relative flex-1 min-h-0 sm:h-[40vh] sm:min-h-[250px]">
                    <canvas
                        id="heartbeat-canvas"
                        class="w-full h-full"
                    ></canvas>

                    // Loading overlay (initial page load)
                    <Show when=move || loading.get()>
                        <div class="absolute inset-0 flex flex-col items-center justify-center bg-[#0d2137]/95 z-10">
                            <div class="text-4xl animate-bounce" style="animation-duration: 0.8s">
                                "\u{26CF}\u{FE0F}"
                            </div>
                            <p class="mt-3 text-sm text-white/50 font-mono animate-pulse">
                                "Mining blocks..."
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
                    // First-visit hint overlay (starts visible, dismissed on click)
                    {
                        let (show_hint, set_show_hint) = signal(true);
                        view! {
                            <Show when=move || show_hint.get()>
                                <div
                                    class="absolute inset-0 z-20 flex items-center justify-center bg-[#0d2137]/90 cursor-pointer"
                                    on:click=move |_| set_show_hint.set(false)
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
                        }
                    }

                </div>

                // Control bar (HTML, outside canvas)
                <div class="flex items-center justify-center gap-1.5 px-3 py-1.5 border-t border-white/5">
                    <span id="heartbeat-zoom-label" class="text-[10px] text-white/30 font-mono mr-2 w-8">"1.9x"</span>
                    <button class="w-7 h-7 sm:w-8 sm:h-8 rounded bg-white/10 text-white/70 hover:bg-white/20 hover:text-white transition-all text-xs sm:text-sm cursor-pointer flex items-center justify-center" title="Toggle cell/brick view" onclick="handleControlClick('mode')">
                        <span id="heartbeat-btn-mode">"\u{25A0}"</span>
                    </button>
                    <button class="w-7 h-7 sm:w-8 sm:h-8 rounded bg-white/10 text-white/70 hover:bg-white/20 hover:text-white transition-all text-xs sm:text-sm cursor-pointer flex items-center justify-center" title="Previous block" onclick="handleControlClick('prev')">
                        "\u{23EE}"
                    </button>
                    <button id="heartbeat-btn-pause" class="w-7 h-7 sm:w-8 sm:h-8 rounded bg-[#f7931a]/20 text-[#f7931a] hover:bg-[#f7931a]/30 transition-all text-xs sm:text-sm cursor-pointer flex items-center justify-center" title="Pause/Play" onclick="handleControlClick('pause')">
                        "\u{23F8}"
                    </button>
                    <button class="w-7 h-7 sm:w-8 sm:h-8 rounded bg-white/10 text-white/70 hover:bg-white/20 hover:text-white transition-all text-xs sm:text-sm cursor-pointer flex items-center justify-center" title="Zoom out" onclick="handleControlClick('zoomOut')">
                        "\u{2212}"
                    </button>
                    <button class="w-7 h-7 sm:w-8 sm:h-8 rounded bg-white/10 text-white/70 hover:bg-white/20 hover:text-white transition-all text-xs sm:text-sm cursor-pointer flex items-center justify-center" title="Zoom in" onclick="handleControlClick('zoomIn')">
                        "+"
                    </button>
                    <button class="w-7 h-7 sm:w-8 sm:h-8 rounded bg-white/10 text-white/70 hover:bg-white/20 hover:text-white transition-all text-xs sm:text-sm cursor-pointer flex items-center justify-center" title="Center on live" onclick="handleControlClick('center')">
                        "\u{2316}"
                    </button>
                    <button id="heartbeat-btn-live" class="w-7 h-7 sm:w-8 sm:h-8 rounded bg-white/10 text-white/70 hover:bg-white/20 hover:text-white transition-all text-xs sm:text-sm cursor-pointer hidden items-center justify-center" title="Jump to live" onclick="handleControlClick('live')">
                        "\u{25C9}"
                    </button>
                </div>

                // Bottom info bar with TX search
                <div class="flex flex-col sm:flex-row items-center justify-between px-3 sm:px-4 py-1.5 sm:py-2 gap-1 sm:gap-2 border-t border-white/5 text-xs sm:text-base text-[#00e676] font-mono">
                    <span>{mempool_display}</span>
                    <div class="flex items-center gap-1.5" title="Searches visible txs on the timeline. Only recent mempool transactions that arrived since the last block are shown.">
                        <input
                            id="heartbeat-tx-search"
                            type="text"
                            placeholder="Search txid..."
                            class="w-28 sm:w-48 px-2 py-0.5 rounded bg-white/5 border border-white/10 text-xs font-mono text-white/70 placeholder-white/30 focus:border-[#00e676]/50 focus:outline-none"
                            on:keydown=move |e: leptos::ev::KeyboardEvent| {
                                if e.key() == "Enter" {
                                    #[cfg(feature = "hydrate")]
                                    {
                                        use wasm_bindgen::JsCast;
                                        let input = e.target().unwrap().unchecked_into::<leptos::web_sys::HtmlInputElement>();
                                        let val = input.value();
                                        if !val.is_empty() {
                                            let found = heartbeat_search_tx(&val);
                                            if !found {
                                                input.set_placeholder("Not found");
                                                let input2 = input.clone();
                                                leptos::prelude::set_timeout(move || {
                                                    input2.set_placeholder("Search txid...");
                                                }, std::time::Duration::from_secs(2));
                                            }
                                            input.set_value("");
                                        }
                                    }
                                }
                            }
                        />
                    </div>
                    <span>"Next block: " {fee_display}</span>
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

            // ── Whale Watch Feed ──────────────────────────────
            <div id="whale-feed-panel" class="bg-[#0d2137] border border-[#ffd700]/20 rounded-2xl overflow-hidden hidden">
                <div class="flex items-center justify-between px-4 py-2.5 border-b border-[#ffd700]/10">
                    <div class="flex items-center gap-2">
                        <span class="w-2 h-2 rounded-full bg-[#ffd700] animate-pulse"></span>
                        <span class="text-xs font-mono text-[#ffd700]/80 uppercase tracking-wider">"Whale Watch"</span>
                    </div>
                    <div class="flex items-center gap-1 flex-wrap">
                        <button onclick="window._filterNotable('all')" id="whale-filter-all" title="Show all notable transactions" class="px-2 py-0.5 rounded text-[10px] font-mono bg-white/10 text-white/60 hover:bg-white/20 transition-colors">"All"</button>
                        <button onclick="window._filterNotable('whale')" id="whale-filter-whale" title="Whales: transfer value over $1,000,000 USD (excluding change output)" class="px-2 py-0.5 rounded text-[10px] font-mono bg-transparent text-[#ffd700]/50 hover:bg-[#ffd700]/10 transition-colors">"Whales"</button>
                        <button onclick="window._filterNotable('round_number')" id="whale-filter-round_number" title="Round-number transfers: exact 1, 10, 100, or 1000 BTC output amounts. Often human-initiated vs exchange automation." class="px-2 py-0.5 rounded text-[10px] font-mono bg-transparent text-[#90ee90]/60 hover:bg-[#90ee90]/10 transition-colors">"Round #"</button>
                        <button onclick="window._filterNotable('inscription')" id="whale-filter-inscription" title="Large inscriptions: witness data over 100KB (Ordinals, BRC-20, images, JSON)" class="px-2 py-0.5 rounded text-[10px] font-mono bg-transparent text-[#ff00c8]/50 hover:bg-[#ff00c8]/10 transition-colors">"Inscr."</button>
                        <button onclick="window._filterNotable('consolidation')" id="whale-filter-consolidation" title="Consolidations: 50+ inputs merged into 3 or fewer outputs (exchange cold wallet sweeps, UTXO cleanup)" class="px-2 py-0.5 rounded text-[10px] font-mono bg-transparent text-[#a855f7]/50 hover:bg-[#a855f7]/10 transition-colors">"Consol."</button>
                        <button onclick="window._filterNotable('fan_out')" id="whale-filter-fan_out" title="Fan-outs: 3 or fewer inputs sprayed to 100+ outputs (exchange batch payouts, mining rewards, airdrops)" class="px-2 py-0.5 rounded text-[10px] font-mono bg-transparent text-[#00d2ff]/50 hover:bg-[#00d2ff]/10 transition-colors">"Fan-out"</button>
                        <button onclick="window._filterNotable('fee')" id="whale-filter-fee" title="Fee outliers: fee rate over 2000 sat/vB or absolute fee over 0.1 BTC" class="px-2 py-0.5 rounded text-[10px] font-mono bg-transparent text-[#ff4444]/50 hover:bg-[#ff4444]/10 transition-colors">"Fees"</button>
                        <button onclick="window._filterNotable('op_return')" id="whale-filter-op_return" title="OP_RETURN messages: transactions embedding readable ASCII text on-chain" class="px-2 py-0.5 rounded text-[10px] font-mono bg-transparent text-[#ffa500]/60 hover:bg-[#ffa500]/10 transition-colors">"Messages"</button>
                    </div>
                </div>
                <div id="whale-feed-list" class="max-h-[240px] overflow-y-auto">
                    <div data-placeholder="1" class="px-4 py-3 text-xs text-white/20 font-mono italic text-center">"Listening for notable transactions..."</div>
                </div>
                <div class="px-3 py-1.5 border-t border-white/5 text-[10px] font-mono text-white/30 text-center">
                    <a href="/observatory/whale-watch" class="hover:text-[#f7931a] transition-colors">"View all history and stats \u{2192}"</a>
                </div>
            </div>

            // ── Phase 5: Capture controls ────────────────────
            <div class="flex flex-wrap items-center justify-center gap-3">
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
            </div>

            // Legend
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
    }
}

// ---------------------------------------------------------------------------
// Vital Signs Tile component
// ---------------------------------------------------------------------------

#[component]
fn VitalTile(
    label: &'static str,
    value: ReadSignal<String>,
    unit: &'static str,
    status: ReadSignal<String>,
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
