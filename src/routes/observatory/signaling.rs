//! BIP signaling tracker: version bit signaling and coinbase compliance monitoring.
//!
//! Tracks two different types of BIP readiness across 2,016-block retarget periods:
//!
//! **BIP-110 (version bit signaling)**: miners signal support by setting bit 4 in
//! the block header's nVersion field. Uses a 55% activation threshold (1,109 of
//! 2,016 blocks). This is the standard BIP-9 style signaling mechanism.
//!
//! **BIP-54 (coinbase compatibility checking)**: there is no formal signaling
//! mechanism for BIP-54. This tracker checks compatibility by verifying that
//! coinbase nLockTime equals height-1 and nSequence is not 0xFFFFFFFF (timelock
//! not disabled). Uses a 95% threshold.
//!
//! The page shows: a status card with progress bars, per-pool signaling breakdown,
//! a block grid where each cell is a block (green = signaled, red = not), and a
//! period history bar chart. Users can navigate between retarget periods.

use leptos::prelude::*;
use leptos_meta::*;

use super::components::*;
use super::helpers::*;
use crate::stats::server_fns::*;

/// BIP signaling tracker page. Fetches per-block signaling data and period-level
/// aggregates, then renders status card, miner breakdown, block grid, and history chart.
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
        <div class="relative rounded-2xl overflow-hidden mb-6">
            <img
                src="/img/observatory_hero.png"
                alt="BIP Signaling Tracker"
                class="w-full h-[100px] sm:h-[120px] lg:h-[140px] object-cover object-center"
            />
            <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/60 to-[#123c64]/30"></div>
            <div class="absolute inset-0 flex flex-col items-center justify-end pb-3 sm:pb-4">
                <h1 class="text-xl sm:text-2xl lg:text-3xl font-title text-white mb-1 drop-shadow-lg">"Signaling"</h1>
                <p class="text-xs sm:text-sm text-white/60 max-w-xl mx-auto px-4 text-center drop-shadow">"Track miner readiness for proposed Bitcoin protocol upgrades"</p>
            </div>
        </div>

        // SEO: crawlable description for search engines (visually hidden, accessible)
        <p class="sr-only">
            "Track miner readiness for proposed Bitcoin protocol upgrades. The block grid shows per-block signaling, while the period chart tracks progress toward activation thresholds across 2,016-block retarget windows."
        </p>

        // BIP selector + Period navigator in one row
        <div class="flex flex-col sm:flex-row items-center justify-between gap-4 mb-6">
            // BIP selector
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

            // Period navigator
            <div class="flex items-center gap-3">
                <button
                    class=move || {
                        if period_offset.get() >= 11 {
                            "inline-flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg text-white/20 border border-white/5 cursor-not-allowed"
                        } else {
                            "inline-flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg text-white/60 border border-white/10 hover:text-white hover:border-white/25 hover:bg-white/5 transition-all cursor-pointer"
                        }
                    }
                    on:click=move |_| {
                        if period_offset.get_untracked() < 11 {
                            set_period_offset.update(|o| *o += 1);
                        }
                    }
                >
                    <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
                    </svg>
                    "Older"
                </button>
                <span class="text-xs text-white/50 font-medium min-w-[100px] text-center">
                    {move || {
                        let o = period_offset.get();
                        if o == 0 { "Current Period".to_string() } else { format!("{} periods ago", o) }
                    }}
                </span>
                <button
                    class=move || {
                        if period_offset.get() == 0 {
                            "inline-flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg text-white/20 border border-white/5 cursor-not-allowed"
                        } else {
                            "inline-flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg text-white/60 border border-white/10 hover:text-white hover:border-white/25 hover:bg-white/5 transition-all cursor-pointer"
                        }
                    }
                    disabled=move || period_offset.get() == 0
                    on:click=move |_| set_period_offset.update(|o| *o = o.saturating_sub(1))
                >
                    "Newer"
                    <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                    </svg>
                </button>
            </div>
        </div>

        <Suspense fallback=move || view! {
            <div class="space-y-6">
                <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4">
                    <div class="h-3 bg-white/5 rounded-full mb-2"></div>
                    <div class="h-4 w-2/3 mx-auto bg-white/5 rounded mt-2"></div>
                </div>
                <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4 flex items-center justify-center h-32">
                    <span class="text-xs text-white/30">"Loading signaling data..."</span>
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
                            let activated = pct >= threshold;
                            let bar_color = if activated { "#22c55e" } else if pct >= threshold * 0.7 { "#f7931a" } else { "#ef4444" };
                            let status_text = if activated { "Threshold reached" } else if is_current { "In progress" } else { "Did not activate" };
                            let status_color = if activated { "text-green-400" } else if pct >= threshold * 0.7 { "text-[#f7931a]" } else { "text-red-400/70" };

                            // BIP description (compact)
                            let bip_desc: (&str, &str, &str, &str) = if bip_method.get() == "locktime" {
                                ("BIP-54: Consensus Cleanup", "Addresses four protocol vulnerabilities: timewarp attack, worst-case block validation time, 64-byte transaction exploits, and a theoretical edge case in BIP-34's duplicate coinbase txid prevention. There is no formal signaling mechanism for BIP-54. This tracker checks compatibility: coinbase nLockTime = height\u{2009}\u{2212}\u{2009}1 and nSequence != 0xFFFFFFFF (timelock not disabled).", "95%", "https://github.com/bitcoin/bips/blob/master/bip-0054.md")
                            } else {
                                ("BIP-110: OP_RETURN Limits", "Limits new outputs to 34 bytes (except OP_RETURN, which allows up to 83 bytes). Also caps data pushes and witness elements at 256 bytes, restricts spendable witness versions to v0 and v1 (Taproot), and temporarily limits certain Taproot features. Pre-existing UTXOs are exempt. Signals on bit 4 with a 55% threshold (1,109 of 2,016 blocks). Mandatory lock-in around August 2026, auto-expires ~1 year after activation.", "55%", "https://github.com/bitcoin/bips/blob/master/bip-0110.mediawiki")
                            };

                            // Aggregate miner signaling data
                            let mut miner_map: std::collections::BTreeMap<String, (u64, u64)> = std::collections::BTreeMap::new();
                            for b in blocks.iter() {
                                let name = if b.miner.is_empty() { "Unknown".to_string() } else { b.miner.clone() };
                                let entry = miner_map.entry(name).or_insert((0, 0));
                                entry.1 += 1; // total
                                if b.signaled {
                                    entry.0 += 1; // signaled
                                }
                            }
                            // Sort by signaled count descending
                            let mut miner_list: Vec<(String, u64, u64)> = miner_map
                                .into_iter()
                                .map(|(name, (signaled, total))| (name, signaled, total))
                                .collect();
                            miner_list.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)));

                            let grid_cells = blocks.iter().map(|b| {
                                let signaled = b.signaled;
                                let color = if signaled { "bg-green-500/70" } else { "bg-red-500/30" };
                                let marker = if signaled { "\u{2713}" } else { "\u{2717}" };
                                let sig_label = if bip_method.get() == "locktime" { if signaled { "compatible" } else { "not compatible" } } else if signaled { "signaled" } else { "not signaled" };
                                let label = format!("Block {} by {}, {}", b.height, b.miner, sig_label);
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
                                <div class="space-y-6">
                                    // Status card: progress + BIP info combined
                                    <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6">
                                        <div class="flex flex-col lg:flex-row lg:items-start lg:gap-8">
                                            // Left: Signaling stats
                                            <div class="flex-1 mb-4 lg:mb-0">
                                                <div class="flex items-baseline gap-3 mb-3">
                                                    <span class="text-3xl font-bold text-white font-mono">{format!("{:.1}%", pct)}</span>
                                                    <span class=format!("text-sm font-medium {status_color}")>{status_text}</span>
                                                </div>
                                                // Signaling bar (percentage of blocks that signaled)
                                                <div class="mb-1">
                                                    <div class="flex items-center justify-between mb-1">
                                                        <span class="text-[10px] text-white/40">{if bip_method.get() == "locktime" { "Compatibility" } else { "Signaling" }}</span>
                                                        <span class="text-[10px] text-white/40 font-mono">{format!("{} / {} blocks", format_number(period_stats.signaled_count), format_number(mined))}</span>
                                                    </div>
                                                    <div class="h-2.5 bg-white/5 rounded-full overflow-hidden">
                                                        <div
                                                            class="h-full rounded-full transition-all duration-500"
                                                            style=format!("width: {}%; background: {bar_color}", pct.min(100.0))
                                                        ></div>
                                                    </div>
                                                </div>
                                                // Period progress bar (how far through the 2016-block window)
                                                <div class="mb-3">
                                                    <div class="flex items-center justify-between mb-1">
                                                        <span class="text-[10px] text-white/40">"Period progress"</span>
                                                        <span class="text-[10px] text-white/40 font-mono">{format!("{} / 2,016", format_number(mined))}</span>
                                                    </div>
                                                    <div class="h-1.5 bg-white/5 rounded-full overflow-hidden">
                                                        <div
                                                            class="h-full rounded-full bg-white/20 transition-all duration-500"
                                                            style=format!("width: {bar_width}")
                                                        ></div>
                                                    </div>
                                                </div>
                                                <div class="flex flex-wrap gap-x-6 gap-y-1 text-xs text-white/50">
                                                    {if is_current {
                                                        Some(view! { <span>{format!("{} remaining", format_number(remaining))}</span> })
                                                    } else {
                                                        None
                                                    }}
                                                    <span>{format!("threshold: {}%", threshold as u32)}</span>
                                                </div>
                                                <p class="text-xs text-white/30 mt-2 font-mono">
                                                    {format!("Blocks {} \u{2013} {}", format_number(p_start), format_number(p_end))}
                                                </p>
                                            </div>
                                            // Right: BIP description
                                            <div class="lg:max-w-sm lg:border-l lg:border-white/10 lg:pl-8">
                                                <h3 class="text-sm text-white font-semibold mb-1.5">{bip_desc.0}</h3>
                                                <p class="text-xs text-white/50 leading-relaxed mb-2">{bip_desc.1}</p>
                                                <a
                                                    href=bip_desc.3
                                                    target="_blank"
                                                    rel="noopener"
                                                    class="inline-flex items-center gap-1 text-[11px] text-[#f7931a]/70 hover:text-[#f7931a] transition-colors"
                                                >
                                                    "Read BIP specification"
                                                    <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                                        <path stroke-linecap="round" stroke-linejoin="round" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"/>
                                                    </svg>
                                                </a>
                                            </div>
                                        </div>
                                    </div>

                                    // Miner signaling breakdown
                                    <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6">
                                        <h3 class="text-sm text-white/70 font-semibold mb-4">"Mining Pool Signaling"</h3>
                                        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
                                            {miner_list.iter().map(|(name, signaled, total)| {
                                                let pct = if *total > 0 { *signaled as f64 / *total as f64 * 100.0 } else { 0.0 };
                                                let bar_w = format!("{}%", pct.min(100.0));
                                                let color = if pct >= 90.0 { "#22c55e" } else if pct >= 50.0 { "#f7931a" } else if *signaled > 0 { "#ef4444" } else { "#334155" };
                                                let name_display = if name.len() > 20 { format!("{}...", &name[..18]) } else { name.clone() };
                                                view! {
                                                    <div class="bg-white/[0.03] rounded-lg px-3 py-2.5">
                                                        <div class="flex items-center justify-between mb-1.5">
                                                            <span class="text-xs text-white/80 font-medium truncate mr-2">{name_display}</span>
                                                            <span class="text-xs text-white/50 font-mono whitespace-nowrap">{format!("{}/{}", signaled, total)}</span>
                                                        </div>
                                                        <div class="h-1.5 bg-white/5 rounded-full overflow-hidden">
                                                            <div
                                                                class="h-full rounded-full"
                                                                style=format!("width: {bar_w}; background: {color}")
                                                            ></div>
                                                        </div>
                                                    </div>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    </div>

                                    // Block grid
                                    <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6">
                                        <div class="flex items-center justify-between mb-3">
                                            <h3 class="text-sm text-white/70 font-semibold">"Block Grid"</h3>
                                            <div class="flex items-center gap-3 text-[10px] text-white/40">
                                                <span class="flex items-center gap-1"><span class="w-2.5 h-2.5 rounded-sm bg-green-500/70"></span>{if bip_method.get() == "locktime" { "Compatible" } else { "Signaled" }}</span>
                                                <span class="flex items-center gap-1"><span class="w-2.5 h-2.5 rounded-sm bg-red-500/30"></span>{if bip_method.get() == "locktime" { "Not compatible" } else { "Not signaled" }}</span>
                                            </div>
                                        </div>
                                        <div class="flex flex-wrap gap-1">
                                            {grid_cells}
                                        </div>
                                    </div>

                                    // History chart
                                    <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6">
                                        <h3 class="text-sm text-white/70 font-semibold mb-4">"Period History"</h3>
                                        <Chart id="chart-signaling-periods" option=Signal::derive(move || periods_chart.clone())/>
                                    </div>
                                </div>
                            }.into_any()
                        }
                        Err(ref _e) => {
                            view! {
                                <div class="flex flex-col items-center justify-center py-12 gap-3">
                                    <svg class="w-10 h-10 text-[#f7931a]/40" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m9-.75a9 9 0 11-18 0 9 9 0 0118 0zm-9 3.75h.008v.008H12v-.008z"/>
                                    </svg>
                                    <p class="text-sm text-white/50">"Unable to load signaling data"</p>
                                    <p class="text-xs text-white/30">"The Bitcoin node may be temporarily unavailable. Try refreshing in a moment."</p>
                                </div>
                            }.into_any()
                        }
                    }
                })
            }}
        </Suspense>
    }
}
