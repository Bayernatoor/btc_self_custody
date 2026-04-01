//! BIP signaling tracker: version bit and locktime compliance monitoring.

use leptos::prelude::*;
use leptos_meta::*;

use super::components::*;
use super::helpers::*;
use crate::stats::server_fns::*;

#[component]
pub fn SignalingPage() -> impl IntoView {
    let (bip_method, set_bip_method) = signal("bit".to_string());
    let (period_offset, set_period_offset) = signal(0u64);

    let signaling_data = LocalResource::new(move || {
        let method = bip_method.get();
        let offset = period_offset.get();
        async move {
            let stats =
                fetch_stats_summary().await.map_err(|e| e.to_string())?;
            let bit = if method == "locktime" { 0 } else { 4 };

            let current_period = stats.max_height / 2016;
            let target_period = current_period.saturating_sub(offset);
            let period_start = target_period * 2016;
            let period_end = (period_start + 2015).min(stats.max_height);

            let blocks_result =
                fetch_signaling(bit, method.clone(), period_start, period_end)
                    .await
                    .map_err(|e| e.to_string())?;

            let periods = fetch_signaling_periods(bit, method)
                .await
                .map_err(|e| e.to_string())?;

            Ok::<_, String>((
                blocks_result,
                periods,
                period_start,
                period_end,
                current_period,
                target_period,
            ))
        }
    });

    view! {
        <Title text="BIP Signaling Tracker | WE HODL BTC"/>
        <Meta name="description" content="Track Bitcoin Improvement Proposal signaling in real time. Monitor miner readiness for proposed protocol upgrades via version bit signaling and coinbase compliance across 2,016-block retarget periods."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/signaling"/>

        // Slim hero banner
        <div class="relative rounded-2xl overflow-hidden mb-5">
            <img
                src="/img/observatory_hero.png"
                alt="BIP Signaling Tracker"
                class="w-full h-[100px] sm:h-[120px] lg:h-[140px] object-cover object-center"
            />
            <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/60 to-[#123c64]/30"></div>
            <div class="absolute inset-0 flex flex-col items-center justify-end pb-3 sm:pb-4">
                <h1 class="text-lg sm:text-xl lg:text-2xl font-title text-white mb-0.5 drop-shadow-lg">"Signaling"</h1>
                <p class="text-[11px] sm:text-xs text-white/50 max-w-lg mx-auto px-4 text-center drop-shadow">"Track miner readiness for proposed Bitcoin protocol upgrades"</p>
            </div>
        </div>

        // SEO: crawlable description for search engines (visually hidden, accessible)
        <p class="sr-only">
            "Track miner readiness for proposed Bitcoin protocol upgrades. The block grid shows per-block signaling, while the period chart tracks progress toward activation thresholds across 2,016-block retarget windows."
        </p>

        // BIP selector
        <div class="flex items-center gap-3 mb-6">
            <div class="relative inline-block">
                <select
                    aria-label="BIP proposal"
                    class="appearance-none bg-[#0a1a2e] text-white/80 text-sm border border-white/10 rounded-xl pl-3 pr-8 py-2 cursor-pointer focus:outline-none focus:border-[#f7931a]/40 transition-colors"
                    prop:value=move || bip_method.get()
                    on:change=move |ev| {
                        use wasm_bindgen::JsCast;
                        if let Some(t) = ev.target() {
                            if let Ok(s) = t.dyn_into::<leptos::web_sys::HtmlSelectElement>() {
                                set_bip_method.set(s.value());
                                set_period_offset.set(0);
                            }
                        }
                    }
                >
                    <option value="bit">"BIP-110: OP_RETURN Limits (Bit 4)"</option>
                    <option value="locktime">"BIP-54: Consensus Cleanup (Locktime)"</option>
                </select>
                <svg class="absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none w-3.5 h-3.5 text-white/40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                </svg>
            </div>
        </div>

        // BIP info card
        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 mb-6">
            {move || {
                if bip_method.get() == "locktime" {
                    view! {
                        <div>
                            <h3 class="text-lg text-white font-semibold mb-2">"BIP-54: Consensus Cleanup"</h3>
                            <p class="text-sm text-white/60 leading-relaxed mb-3">"Fixes timewarp attack, reduces worst-case validation time (2,500 sigop limit), prevents 64-byte transaction exploits, and eliminates duplicate coinbase issues. After activation, all blocks must set coinbase nLockTime = height - 1 and nSequence != 0xffffffff as a consensus rule."</p>
                            <p class="text-sm text-white/60 leading-relaxed mb-3">"The chart below tracks miners already complying with the coinbase requirement. This may indicate readiness, not formal BIP-9 signaling."</p>
                            <p class="text-sm text-[#f7931a]/70 font-mono">"Tracking: Coinbase locktime compliance | Activation threshold: 95%"</p>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div>
                            <h3 class="text-lg text-white font-semibold mb-2">"BIP-110: OP_RETURN Data Limits"</h3>
                            <p class="text-sm text-white/60 leading-relaxed mb-3">"Caps transaction outputs at 34 bytes and OP_RETURN data at 83 bytes. Temporary softfork that expires after 52,416 blocks (~1 year). Modified BIP9: 55% threshold (1,109/2,016). Signaled via version bit 4."</p>
                            <p class="text-sm text-[#f7931a]/70 font-mono">"Signal: Version bit 4 | Threshold: 55%"</p>
                        </div>
                    }.into_any()
                }
            }}
        </div>

        // Period navigator
        <div class="flex items-center justify-center gap-4 mb-8">
            <button
                class=move || {
                    if period_offset.get() >= 11 {
                        "inline-flex items-center gap-2 px-4 py-2 text-sm rounded-xl text-white/20 border border-white/5 cursor-not-allowed"
                    } else {
                        "inline-flex items-center gap-2 px-4 py-2 text-sm rounded-xl text-white/70 border border-white/10 hover:text-white hover:border-white/25 hover:bg-white/5 transition-all cursor-pointer"
                    }
                }
                on:click=move |_| {
                    if period_offset.get_untracked() < 11 {
                        set_period_offset.update(|o| *o += 1);
                    }
                }
            >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
                </svg>
                "Older"
            </button>
            <span class="text-sm text-white/60 font-medium min-w-[140px] text-center">
                {move || {
                    let o = period_offset.get();
                    if o == 0 { "Current Period".to_string() } else { format!("{} periods ago", o) }
                }}
            </span>
            <button
                class=move || {
                    if period_offset.get() == 0 {
                        "inline-flex items-center gap-2 px-4 py-2 text-sm rounded-xl text-white/20 border border-white/5 cursor-not-allowed"
                    } else {
                        "inline-flex items-center gap-2 px-4 py-2 text-sm rounded-xl text-white/70 border border-white/10 hover:text-white hover:border-white/25 hover:bg-white/5 transition-all cursor-pointer"
                    }
                }
                disabled=move || period_offset.get() == 0
                on:click=move |_| set_period_offset.update(|o| *o = o.saturating_sub(1))
            >
                "Newer"
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                </svg>
            </button>
        </div>

        <Suspense fallback=move || view! {
            <div class="space-y-10">
                // Progress bar skeleton
                <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4">
                    <div class="h-3 bg-white/5 rounded-full mb-2"></div>
                    <div class="h-4 w-2/3 mx-auto bg-white/5 rounded mt-2"></div>
                </div>
                // Block grid skeleton
                <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4">
                    <div class="flex flex-col items-center gap-3">
                        <div class="animate-pulse">
                            <div class="w-12 h-12 rounded-lg bg-[#f7931a]/10 border border-[#f7931a]/20 flex items-center justify-center">
                                <svg class="w-6 h-6 text-[#f7931a]/40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                                    <rect x="3" y="3" width="18" height="18" rx="2"/>
                                    <path d="M9 3v18M15 3v18M3 9h18M3 15h18"/>
                                </svg>
                            </div>
                        </div>
                        <span class="text-xs text-white/30">"Loading signaling data..."</span>
                    </div>
                </div>
                // History chart skeleton
                <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 h-[400px] flex items-center justify-center">
                    <div class="flex flex-col items-center gap-3">
                        <div class="animate-pulse">
                            <div class="w-12 h-12 rounded-lg bg-[#f7931a]/10 border border-[#f7931a]/20 flex items-center justify-center">
                                <svg class="w-6 h-6 text-[#f7931a]/40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                                    <rect x="3" y="3" width="18" height="18" rx="2"/>
                                    <path d="M9 3v18M15 3v18M3 9h18M3 15h18"/>
                                </svg>
                            </div>
                        </div>
                        <span class="text-xs text-white/30">"Mining blocks..."</span>
                    </div>
                </div>
            </div>
        }>
            {move || {
                signaling_data.get().map(|result| {
                    match result {
                        Ok(((ref blocks, ref period_stats), ref periods, p_start, p_end, _current_p, _target_p)) => {
                            let threshold = if bip_method.get() == "locktime" { 95.0 } else { 55.0 };
                            let mined = period_stats.total_blocks;
                            let is_current = period_offset.get() == 0;
                            let remaining = if is_current { 2016u64.saturating_sub(mined) } else { 0 };
                            let pct = period_stats.signaled_pct;
                            let bar_width = format!("{}%", (mined as f64 / 2016.0 * 100.0).min(100.0));
                            let bar_color = if pct >= threshold { "#2ecc71" } else { "#e74c3c" };

                            let period_text = if is_current {
                                format!(
                                    "Period {} \u{2013} {}: {} signaled / {} mined of 2,016 ({:.1}%) | {} remaining | threshold: {}%",
                                    format_number(p_start), format_number(p_end),
                                    period_stats.signaled_count, mined, pct, remaining, threshold as u32,
                                )
                            } else {
                                format!(
                                    "Period {} \u{2013} {}: {} signaled / {} blocks ({:.1}%) | threshold: {}%",
                                    format_number(p_start), format_number(p_end),
                                    period_stats.signaled_count, mined, pct, threshold as u32,
                                )
                            };

                            let grid_cells = blocks.iter().map(|b| {
                                let signaled = b.signaled;
                                let color = if signaled { "bg-green-500/70" } else { "bg-red-500/30" };
                                let marker = if signaled { "\u{2713}" } else { "\u{2717}" };
                                let label = format!("Block {} by {}, {}", b.height, b.miner, if signaled { "signaled" } else { "not signaled" });
                                let title = format!("#{} | {}{}", b.height, b.miner, if signaled { " \u{2713}" } else { "" });
                                let h = b.height;
                                view! {
                                    <div
                                        role="button"
                                        tabindex="0"
                                        aria-label=label
                                        class=format!("w-3.5 h-3.5 lg:w-4 lg:h-4 rounded-sm cursor-pointer hover:ring-1 hover:ring-white/50 flex items-center justify-center text-[6px] lg:text-[7px] text-white/40 {color}")
                                        title=title
                                        on:click=move |_| { show_block_detail(h); }
                                        on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                            if ev.key() == "Enter" || ev.key() == " " {
                                                ev.prevent_default();
                                                show_block_detail(h);
                                            }
                                        }
                                    >{marker}</div>
                                }
                            }).collect::<Vec<_>>();

                            let start_height = if bip_method.get() == "locktime" { 940_000u64 } else { 936_000 };
                            let filtered: Vec<_> = periods.iter()
                                .filter(|p| p.end_height >= start_height)
                                .cloned()
                                .collect();
                            let periods_chart = serde_json::to_string(&crate::stats::charts::signaling_periods_chart(&filtered, threshold)).unwrap_or_default();

                            view! {
                                <div class="space-y-10">
                                    // Progress bar
                                    <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4">
                                        <div class="h-3 bg-white/5 rounded-full overflow-hidden mb-2">
                                            <div
                                                class="h-full rounded-full transition-all duration-500"
                                                style=format!("width: {bar_width}; background: {bar_color}")
                                            ></div>
                                        </div>
                                        <p class="text-sm text-white/60 text-center font-mono">{period_text}</p>
                                    </div>

                                    // Block grid
                                    <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4">
                                        <p class="text-sm text-white/50 mb-3">
                                            {format!("Blocks {} \u{2013} {} (click for details)", format_number(p_start), format_number(p_end))}
                                        </p>
                                        <div class="flex flex-wrap gap-1">
                                            {grid_cells}
                                        </div>
                                    </div>

                                    // History chart
                                    <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6">
                                        <Chart id="chart-signaling-periods" option=Signal::derive(move || periods_chart.clone())/>
                                    </div>
                                </div>
                            }.into_any()
                        }
                        Err(ref e) => {
                            let msg = format!("Error loading signaling data: {e}");
                            view! { <p class="text-center text-white/40 text-sm">{msg}</p> }.into_any()
                        }
                    }
                })
            }}
        </Suspense>
    }
}
