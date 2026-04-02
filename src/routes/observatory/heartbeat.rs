//! Block Heartbeat — live EKG visualization of Bitcoin block arrivals.
//!
//! Each block produces a PQRST waveform spike on a canvas sweep line,
//! like a hospital cardiac monitor. The flatline between beats is the
//! real wait for the next block. Color shifts with network stress.

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

    // Phase 4: Polish
    #[wasm_bindgen(js_name = heartbeatPulse)]
    fn heartbeat_pulse();

    #[wasm_bindgen(js_name = heartbeatFlash)]
    fn heartbeat_flash();

    #[wasm_bindgen(js_name = getOrganismStatus)]
    fn get_organism_status() -> String;

    // Phase 5: Sound
    #[wasm_bindgen(js_name = heartbeatSoundToggle)]
    fn heartbeat_sound_toggle(enable: bool) -> bool;

    #[wasm_bindgen(js_name = heartbeatSoundIsEnabled)]
    fn heartbeat_sound_is_enabled() -> bool;

    #[wasm_bindgen(js_name = heartbeatPlaySound)]
    fn heartbeat_play_sound();

    // Phase 5: Capture
    #[wasm_bindgen(js_name = heartbeatDownloadCapture)]
    fn heartbeat_download_capture(vitals_json: &str);
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
fn get_heartbeat_vitals() -> String { "{}".to_string() }
#[cfg(not(feature = "hydrate"))]
fn render_rhythm_strip(_: &str, _: &str) {}
#[cfg(not(feature = "hydrate"))]
fn get_heartbeat_recent_blocks() -> String { "[]".to_string() }
#[cfg(not(feature = "hydrate"))]
fn heartbeat_pulse() {}
#[cfg(not(feature = "hydrate"))]
fn heartbeat_flash() {}
#[cfg(not(feature = "hydrate"))]
fn get_organism_status() -> String { "{}".to_string() }
#[cfg(not(feature = "hydrate"))]
fn heartbeat_sound_toggle(_: bool) -> bool { false }
#[cfg(not(feature = "hydrate"))]
#[allow(dead_code)]
fn heartbeat_sound_is_enabled() -> bool { false }
#[cfg(not(feature = "hydrate"))]
fn heartbeat_play_sound() {}
#[cfg(not(feature = "hydrate"))]
fn heartbeat_download_capture(_: &str) {}

const RETARGET_PERIOD: u64 = 2016;
const BRADYCARDIA_THRESHOLD: f64 = 0.7;
const TACHYCARDIA_THRESHOLD: f64 = 1.3;

// ---------------------------------------------------------------------------
// Heartbeat page component
// ---------------------------------------------------------------------------

#[component]
pub fn HeartbeatPage() -> impl IntoView {
    let state = expect_context::<super::shared::ObservatoryState>();
    let cached_live = state.cached_live;

    // Track last-seen block height to detect new blocks
    let last_height = std::rc::Rc::new(std::cell::Cell::new(0u64));
    let initialized = std::rc::Rc::new(std::cell::Cell::new(false));

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

    // Sound toggle state
    let (sound_on, set_sound_on) = signal(false);

    // Period start timestamp (first block in current retarget period)
    let (period_start_ts, set_period_start_ts) = signal(0u64);

    // Blocks until next difficulty adjustment
    let blocks_until_retarget = Signal::derive(move || {
        cached_live.get().map(|s| {
            let height = s.blockchain.blocks;
            let blocks_in_epoch = height % RETARGET_PERIOD;
            let remaining = RETARGET_PERIOD - blocks_in_epoch;
            remaining
        }).unwrap_or(0)
    });

    // Fetch blocks for initial timeline (current retarget period = 2016 blocks)
    let initial_blocks = LocalResource::new(move || async move {
        let height = cached_live
            .get()
            .map(|s| s.blockchain.blocks)
            .unwrap_or(0);
        if height == 0 {
            return Vec::new();
        }
        // Fetch from start of current retarget period
        let period_start = (height / RETARGET_PERIOD) * RETARGET_PERIOD;
        crate::stats::server_fns::fetch_blocks(period_start, height)
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

            // Store period start timestamp for heart rate calculation
            if let Some(first) = blocks.first() {
                set_period_start_ts.set(first.timestamp);
            }

            // Build block events with inter-block time (replay=true for compressed history)
            let json = blocks_to_json(&blocks);
            push_heartbeat_blocks(&json, true);

            // Store last height — use the latest from LiveStats (not the last replayed block)
            // to avoid missing blocks that arrived between fetch and now
            let live_height = cached_live.get_untracked()
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
                if period_ts > 0 && current_ts > period_ts && blocks_in > 1 {
                    let span = (current_ts - period_ts) as f64;
                    let avg_secs = span / blocks_in as f64;
                    let avg_u = avg_secs.round() as u64;
                    set_hr_display.set(format!("{}:{:02}", avg_u / 60, avg_u % 60));
                    let bpm = 600.0 / avg_secs;
                    set_hr_subtitle.set(format!("{:.2} bpm", bpm));
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
            let raw_minfee = cached_live.get_untracked()
                .map(|s| s.mempool.mempoolminfee)
                .unwrap_or(0.0);
            // BTC/kB to sat/vB: raw * 1e8 / 1000
            // f64 stores 0.00000100 as ~9.99e-7, losing precision.
            // Round to nearest 0.1 sat/vB since relay fees are always clean multiples.
            let raw_sat_vb = raw_minfee * 1e8 / 1000.0;
            let diastolic = (raw_sat_vb * 10.0 + 0.5).floor() / 10.0;
            let diastolic = if diastolic < 0.1 && raw_minfee > 0.0 { 0.1 } else { diastolic };
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
                set_temp_subtitle.set(format!("{:.1}\u{00B0}C \u{00b7} {}", v.temp_c, v.temp_label));
            } else {
                set_temp_display.set(format!("{:.1}", v.temp_c));
                set_temp_subtitle.set(String::new());
            }
            set_temp_label.set(format!("{} tx in mempool",
                cached_live.get_untracked()
                    .map(|s| super::helpers::format_number(s.mempool.size))
                    .unwrap_or_else(|| "--".to_string())));
            set_temp_color.set(v.temp_color);

            // Immune System: hashrate + retarget context
            set_immune_display.set(format!("{:.1} EH/s", v.immune_eh));
            set_immune_label.set(format!("{} \u{00b7} Retarget in ~{} blocks",
                v.immune_label,
                blocks_until_retarget.get_untracked()));
            set_immune_color.set(v.immune_color);
        }

        let status_json = get_organism_status();
        if let Some(s) = parse_status_json(&status_json) {
            set_org_condition.set(s.condition);
            set_org_desc.set(s.description);
            set_org_color.set(s.color);
        }
    };

    // Watch for new blocks via LiveStats polling
    let last_height2 = last_height.clone();
    let initialized2 = initialized.clone();
    Effect::new(move || {
        let Some(live) = cached_live.get() else {
            return;
        };
        let current_height = live.blockchain.blocks;

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

        // Check for new blocks (only after init completes)
        if !initialized2.get() {
            return;
        }
        let prev = last_height2.get();
        if prev > 0 && current_height > prev {
            last_height2.set(current_height);

            // Phase 4: flash and pulse on new block
            heartbeat_flash();
            heartbeat_pulse();

            // Phase 5: sound on new block
            if sound_on.get_untracked() {
                heartbeat_play_sound();
            }

            // Fetch the new block(s) with retry chain (ingestion can lag 5-20s)
            #[cfg(feature = "hydrate")]
            {
                let from = prev + 1;
                let to = current_height;
                fn try_fetch(from: u64, to: u64, delays: &'static [u64]) {
                    leptos::task::spawn_local(async move {
                        let blocks = crate::stats::server_fns::fetch_blocks(from, to)
                            .await
                            .unwrap_or_default();
                        if !blocks.is_empty() {
                            let json = blocks_to_json(&blocks);
                            push_heartbeat_blocks(&json, false);
                            let recent = get_heartbeat_recent_blocks();
                            render_rhythm_strip("rhythm-strip-canvas", &recent);
                        } else if !delays.is_empty() {
                            let delay = delays[0];
                            let rest = &delays[1..];
                            leptos::prelude::set_timeout(move || {
                                try_fetch(from, to, rest);
                            }, std::time::Duration::from_secs(delay));
                        } else {

                        }
                    });
                }
                // Try now, then retry at escalating intervals (ingestion polls every 60s)
                try_fetch(from, to, &[5, 15, 30, 45, 60, 90]);
            }
        }
    });

    // Cleanup animation on navigate away
    on_cleanup(|| {
        destroy_heartbeat();
    });

    // Reactive display values
    let block_height = Signal::derive(move || {
        cached_live
            .get()
            .map(|s| format!("#{}", super::helpers::format_number(s.blockchain.blocks)))
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
        on_cleanup(move || { if let Ok(h) = handle { h.clear(); } });
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
        cached_live.get().map(|s| {
            format!("{} tx ({:.1} vMB)",
                super::helpers::format_number(s.mempool.size),
                s.mempool.bytes as f64 / 1_000_000.0)
        }).unwrap_or_default()
    });

    let fee_display = Signal::derive(move || {
        cached_live.get().map(|s| {
            format!("{:.1} sat/vB", s.next_block_fee)
        }).unwrap_or_default()
    });

    view! {
        <Title text="Block Heartbeat | WE HODL BTC"/>
        <Meta name="description" content="Watch Bitcoin breathe. A live EKG visualization of block arrivals, where every spike tells a story of transactions, fees, and network activity."/>

        <div class="space-y-6">
            // EKG Canvas card
            <div id="heartbeat-card" class="relative bg-[#0d2137] border border-white/10 rounded-2xl overflow-hidden">
                // Status bar
                <div class="flex items-center justify-between px-4 py-2.5 border-b border-white/5">
                    <div class="flex items-center gap-3">
                        <div class="flex items-center gap-1.5">
                            <span class="relative flex h-2 w-2">
                                <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-[#00e676] opacity-60"></span>
                                <span class="relative inline-flex rounded-full h-2 w-2 bg-[#00e676]"></span>
                            </span>
                            <span class="text-xs text-white/50 font-mono">"LIVE"</span>
                        </div>
                        <span class="text-sm sm:text-base text-[#00e676] font-mono font-semibold">{block_height}</span>
                    </div>
                    <div class="flex items-center gap-4 text-sm sm:text-base text-[#00e676] font-mono">
                        <span>"Last block: " {time_since}</span>
                    </div>
                </div>

                // Canvas
                <canvas
                    id="heartbeat-canvas"
                    class="w-full"
                    style="height: 320px"
                ></canvas>

                // Bottom info bar
                <div class="flex items-center justify-between px-4 py-2 border-t border-white/5 text-sm sm:text-base text-[#00e676] font-mono">
                    <span>{mempool_display}</span>
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
                <div class="px-4 py-2 border-b border-white/5">
                    <span class="text-xs text-white/40 font-mono">"24-HOUR RHYTHM STRIP"</span>
                </div>
                <canvas
                    id="rhythm-strip-canvas"
                    class="w-full"
                    style="height: 100px"
                ></canvas>
            </div>

            // ── Phase 4: Organism Status ──────────────────────
            <div class="bg-[#0d2137]/60 border border-white/5 rounded-xl px-5 py-4">
                <div class="flex items-baseline gap-2">
                    <span class="text-xs text-white/30 font-mono uppercase tracking-wider">"Condition:"</span>
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

            // ── Phase 5: Sound + Capture controls ─────────────
            <div class="flex flex-wrap items-center justify-center gap-3">
                // Sound toggle
                <button
                    class="flex items-center gap-2 px-4 py-2 rounded-lg border border-white/10 bg-[#0d2137]/60 text-xs font-mono text-white/50 hover:text-white/80 hover:border-white/20 transition-colors"
                    on:click=move |_| {
                        let new_state = !sound_on.get_untracked();
                        let actual = heartbeat_sound_toggle(new_state);
                        set_sound_on.set(actual);
                    }
                >
                    <span class="text-base">
                        {move || if sound_on.get() { "\u{1F50A}" } else { "\u{1F507}" }}
                    </span>
                    <span>
                        {move || if sound_on.get() { "Mute" } else { "Unmute" }}
                    </span>
                </button>

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
            <div class="flex flex-wrap justify-center gap-x-5 gap-y-2 text-xs text-white/40">
                <div class="flex items-center gap-1.5">
                    <span class="w-2.5 h-2.5 rounded-full bg-[#00e676]"></span>
                    "Healthy"
                </div>
                <div class="flex items-center gap-1.5">
                    <span class="w-2.5 h-2.5 rounded-full bg-[#42a5f5]"></span>
                    "Calm"
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

            // Explainer
            <div class="text-center max-w-lg mx-auto">
                <p class="text-xs text-white/25 leading-relaxed">
                    "Every spike is a block. Height encodes fees paid. The dip before each spike reflects the wait since the previous block. Color shifts with network stress. The flatline between beats is Bitcoin, breathing."
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
            class="bg-[#0d2137] border border-white/10 rounded-xl px-4 py-3 flex flex-col gap-1"
            data-tip=tip.unwrap_or("")
            tabindex=if tip.is_some() { "0" } else { "-1" }
        >
            <span class="text-xs text-white/30 font-mono uppercase tracking-wider">{label}</span>
            <div class="flex items-baseline gap-1">
                <span
                    class="text-2xl sm:text-3xl font-mono font-bold tabular-nums"
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
                class="text-xs font-mono"
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
        let prev_ts = if i > 0 { blocks[i - 1].timestamp } else { b.timestamp.saturating_sub(600) };
        let inter = b.timestamp.saturating_sub(prev_ts);
        if i > 0 {
            buf.push(',');
        }
        let _ = write!(
            buf,
            r#"{{"height":{},"timestamp":{},"tx_count":{},"total_fees":{},"size":{},"weight":{},"inter_block_seconds":{}}}"#,
            b.height, b.timestamp, b.tx_count, b.total_fees, b.size, b.weight, inter
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
        let end = rest.find(|c: char| c == ',' || c == '}').unwrap_or(rest.len());
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
    if json.len() < 3 { return None; }
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
    if json.len() < 3 { return None; }
    Some(OrganismStatus {
        condition: extract_json_str(json, "condition"),
        description: extract_json_str(json, "description"),
        color: extract_json_str(json, "color"),
    })
}
