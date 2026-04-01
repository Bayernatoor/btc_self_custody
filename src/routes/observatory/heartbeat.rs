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
}

#[cfg(not(feature = "hydrate"))]
fn init_heartbeat(_: &str) {}
#[cfg(not(feature = "hydrate"))]
fn push_heartbeat_blocks(_: &str, _: bool) {}
#[cfg(not(feature = "hydrate"))]
fn update_heartbeat_live(_: &str) {}
#[cfg(not(feature = "hydrate"))]
fn destroy_heartbeat() {}

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

    // Fetch recent blocks for initial waveform history
    let initial_blocks = LocalResource::new(move || async move {
        let height = cached_live
            .get()
            .map(|s| s.blockchain.blocks)
            .unwrap_or(0);
        if height == 0 {
            return Vec::new();
        }
        let from = height.saturating_sub(20);
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

            // Build block events with inter-block time (replay=true for compressed history)
            let json = blocks_to_json(&blocks);
            push_heartbeat_blocks(&json, true);

            // Store last height
            if let Some(last) = blocks.last() {
                last_height.set(last.height);
            }
        }
    });

    // Watch for new blocks via LiveStats polling
    let last_height2 = last_height.clone();
    let initialized2 = initialized.clone();
    Effect::new(move || {
        let Some(live) = cached_live.get() else {
            return;
        };
        let current_height = live.blockchain.blocks;

        // Forward live metrics for color computation
        let live_json = format!(
            r#"{{"next_block_fee":{},"mempool_mb":{:.1},"block_time":{}}}"#,
            live.next_block_fee,
            live.mempool.bytes as f64 / 1_000_000.0,
            live.blockchain.time,
        );
        update_heartbeat_live(&live_json);

        // Check for new blocks
        let prev = last_height2.get();
        if prev > 0 && current_height > prev && initialized2.get() {
            last_height2.set(current_height);
            // Fetch the new block(s)
            leptos::task::spawn_local(async move {
                if let Ok(blocks) =
                    crate::stats::server_fns::fetch_blocks(prev + 1, current_height).await
                {
                    if !blocks.is_empty() {
                        let json = blocks_to_json(&blocks);
                        push_heartbeat_blocks(&json, false);
                    }
                }
            });
        } else if prev == 0 && current_height > 0 {
            last_height2.set(current_height);
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

    let time_since = Signal::derive(move || {
        cached_live.get().map(|s| {
            let now = chrono::Utc::now().timestamp() as u64;
            let elapsed = now.saturating_sub(s.blockchain.time);
            if elapsed < 60 {
                format!("{}s ago", elapsed)
            } else {
                format!("{}m {}s ago", elapsed / 60, elapsed % 60)
            }
        }).unwrap_or_else(|| "waiting...".to_string())
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
            <div class="relative bg-[#0d2137] border border-white/10 rounded-2xl overflow-hidden">
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
