//! Bitcoin Stats dashboard page.
//!
//! Tabs: Dashboard | OP_RETURN | BIP Signaling
//! Data fetched via server functions, charts rendered with ECharts via wasm_bindgen.

use leptos::prelude::*;
use leptos_meta::*;

use crate::extras::spinner::Spinner;
use crate::stats::server_fns::*;
use crate::stats::types::*;

// ---------------------------------------------------------------------------
// wasm_bindgen extern — calls into assets/stats.js
// ---------------------------------------------------------------------------

#[cfg(feature = "hydrate")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "hydrate")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = setChartOption)]
    fn set_chart_option(id: &str, option_json: &str);
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = showBlockDetail)]
    fn show_block_detail(height: u64);
}

#[cfg(not(feature = "hydrate"))]
fn set_chart_option(_id: &str, _json: &str) {}

#[cfg(not(feature = "hydrate"))]
fn show_block_detail(_height: u64) {}

// ---------------------------------------------------------------------------
// Range helper
// ---------------------------------------------------------------------------

fn range_to_blocks(range: &str) -> u64 {
    match range {
        "1d" => 144,
        "1w" => 1_008,
        "1m" => 4_320,
        "3m" => 12_960,
        "6m" => 25_920,
        "1y" => 52_560,
        "2y" => 105_120,
        "5y" => 262_800,
        "10y" => 525_600,
        "all" => 999_999,
        _ => 12_960,
    }
}

// ---------------------------------------------------------------------------
// Chart component
// ---------------------------------------------------------------------------

#[component]
fn Chart(
    #[prop(into)] id: String,
    #[prop(into)] option: Signal<String>,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let id_clone = id.clone();
    Effect::new(move |_| {
        let json = option.get();
        if !json.is_empty() {
            set_chart_option(&id_clone, &json);
        }
    });

    let css_class =
        class.unwrap_or_else(|| "w-full h-[350px] lg:h-[600px]".to_string());

    view! {
        <div id=id class=css_class></div>
    }
}

// ---------------------------------------------------------------------------
// Live stat card
// ---------------------------------------------------------------------------

#[component]
fn LiveCard(
    #[prop(into)] label: String,
    #[prop(into)] value: Signal<String>,
) -> impl IntoView {
    view! {
        <div class="bg-white/[0.07] border border-white/10 rounded-lg p-3 text-center">
            <div class="text-[0.65rem] text-white/50 uppercase tracking-widest mb-1">{label}</div>
            <div class="text-lg lg:text-xl text-[#f7931a] font-bold font-mono truncate">{move || value.get()}</div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Main page
// ---------------------------------------------------------------------------

#[component]
pub fn StatsPage() -> impl IntoView {
    // Check if stats backend is available
    let availability = LocalResource::new(move || fetch_stats_summary());

    let is_available = Signal::derive(move || {
        availability.get().map(|r| r.is_ok()).unwrap_or(false)
    });

    view! {
        <Title text="Bitcoin Stats - WE HODL BTC"/>
        <Show
            when=move || is_available.get()
            fallback=move || view! { <StatsComingSoon/> }
        >
            <StatsContent/>
        </Show>
    }
}

#[component]
fn StatsComingSoon() -> impl IntoView {
    view! {
        <section class="max-w-3xl mx-auto px-6 pt-20 pb-32 opacity-0 animate-fadeinone">
            <div class="flex flex-col items-center justify-center min-h-[60vh] text-center">
                // Bitcoin logo SVG
                <div class="mb-8 opacity-10">
                    <svg class="w-24 h-24 lg:w-32 lg:h-32 text-[#f7931a]" viewBox="0 0 64 64" fill="currentColor">
                        <path d="M63.04 39.741c-4.274 17.143-21.638 27.575-38.783 23.301C7.12 58.768-3.313 41.404.962 24.262 5.234 7.117 22.597-3.317 39.737.957c17.144 4.274 27.576 21.64 23.302 38.784z"/>
                        <path d="M46.11 27.441c.636-4.258-2.606-6.547-7.039-8.074l1.438-5.768-3.512-.875-1.4 5.616c-.923-.23-1.871-.447-2.813-.662l1.41-5.653-3.509-.875-1.439 5.766c-.764-.174-1.514-.346-2.242-.527l.004-.018-4.842-1.209-.934 3.75s2.605.597 2.55.634c1.422.355 1.68 1.296 1.636 2.042l-1.638 6.571c.098.025.225.061.365.117l-.37-.092-2.297 9.205c-.174.432-.615 1.08-1.609.834.035.051-2.552-.637-2.552-.637l-1.743 4.02 4.57 1.139c.85.213 1.683.436 2.502.646l-1.453 5.835 3.507.875 1.44-5.772c.957.26 1.887.5 2.797.726L27.504 50.8l3.511.875 1.453-5.823c5.987 1.133 10.49.676 12.383-4.738 1.527-4.36-.075-6.875-3.225-8.516 2.294-.529 4.022-2.038 4.483-5.157zM38.087 38.69c-1.086 4.36-8.426 2.004-10.807 1.412l1.928-7.729c2.38.594 10.011 1.77 8.88 6.317zm1.085-11.312c-.99 3.966-7.1 1.951-9.083 1.457l1.748-7.01c1.983.494 8.367 1.416 7.335 5.553z" fill="#fff"/>
                    </svg>
                </div>

                <h1 class="text-3xl lg:text-5xl font-title text-white mb-4">"Bitcoin Stats"</h1>
                <div class="w-16 h-0.5 bg-[#f7931a] mx-auto mb-6"></div>

                <p class="text-lg text-white/60 mb-3">"Coming Soon"</p>
                <p class="text-sm text-white/40 max-w-md leading-relaxed mb-10">
                    "Live blockchain metrics, block data analysis, OP_RETURN tracking, and BIP signaling \u{2014} powered by our own Bitcoin full node."
                </p>

                // Feature preview cards
                <div class="grid grid-cols-1 sm:grid-cols-3 gap-4 w-full max-w-2xl">
                    <div class="bg-white/5 border border-white/10 rounded-xl p-5 text-center">
                        <div class="text-2xl mb-2">"#"</div>
                        <div class="text-sm text-white/70 font-medium mb-1">"Live Dashboard"</div>
                        <div class="text-xs text-white/40">"Real-time block, mempool, and network stats"</div>
                    </div>
                    <div class="bg-white/5 border border-white/10 rounded-xl p-5 text-center">
                        <div class="text-2xl mb-2">"\u{21a9}"</div>
                        <div class="text-sm text-white/70 font-medium mb-1">"OP_RETURN Analysis"</div>
                        <div class="text-xs text-white/40">"Track Runes and data carrier usage over time"</div>
                    </div>
                    <div class="bg-white/5 border border-white/10 rounded-xl p-5 text-center">
                        <div class="text-2xl mb-2">"\u{2691}"</div>
                        <div class="text-sm text-white/70 font-medium mb-1">"BIP Signaling"</div>
                        <div class="text-xs text-white/40">"Monitor softfork activation progress"</div>
                    </div>
                </div>
            </div>
        </section>
    }
}

#[component]
fn StatsContent() -> impl IntoView {
    let (tab, set_tab) = signal("dashboard".to_string());
    let (range, set_range) = signal("all".to_string());
    let (fee_unit, set_fee_unit) = signal("sats".to_string());

    // ---- Live stats (auto-refresh 30s) ----
    #[allow(clippy::redundant_closure)]
    let live = LocalResource::new(move || fetch_live_stats());
    let (countdown, set_countdown) = signal(30u32);
    let (last_updated, set_last_updated) = signal("connecting...".to_string());

    // 1-second countdown tick
    leptos_use::use_interval_fn(
        move || {
            set_countdown.update(|c| {
                if *c == 0 {
                    *c = 30;
                    live.refetch();
                    set_last_updated.set(format!(
                        "updated {}",
                        chrono::Local::now().format("%H:%M:%S")
                    ));
                } else {
                    *c -= 1;
                }
            });
        },
        1_000,
    );

    // Mark as updated when first load completes
    Effect::new(move |_| {
        if live.get().is_some() {
            set_last_updated.set(format!(
                "updated {}",
                chrono::Local::now().format("%H:%M:%S")
            ));
        }
    });

    // Helper to extract a field from live stats
    let live_field = move |f: fn(&LiveStats) -> String| -> String {
        live.get()
            .and_then(|r| r.ok())
            .map(|s| f(&s))
            .unwrap_or_else(|| "\u{2014}".to_string())
    };

    let block_height = Signal::derive(move || {
        live_field(|s| format_number(s.blockchain.blocks))
    });
    let chain =
        Signal::derive(move || live_field(|s| s.blockchain.chain.clone()));
    let difficulty = Signal::derive(move || {
        live_field(|s| format!("{:.2}T", s.blockchain.difficulty / 1e12))
    });
    let mempool_size =
        Signal::derive(move || live_field(|s| format_number(s.mempool.size)));
    let mempool_bytes = Signal::derive(move || {
        live_field(|s| format!("{:.1} MB", s.mempool.bytes as f64 / 1e6))
    });
    let price_usd = Signal::derive(move || {
        live_field(|s| {
            format!("${}", format_number_f64(s.network.price_usd, 0))
        })
    });
    let sats_per_dollar = Signal::derive(move || {
        live_field(|s| format_number(s.network.sats_per_dollar))
    });
    let market_cap = Signal::derive(move || {
        live_field(|s| {
            if s.network.market_cap_usd >= 1e12 {
                format!("${:.2}T", s.network.market_cap_usd / 1e12)
            } else {
                format!("${:.1}B", s.network.market_cap_usd / 1e9)
            }
        })
    });
    let utxo_count = Signal::derive(move || {
        live_field(|s| {
            if s.network.utxo_count > 0 {
                format_number(s.network.utxo_count)
            } else {
                "loading...".to_string()
            }
        })
    });
    let chain_size = Signal::derive(move || {
        live_field(|s| format!("{:.1} GB", s.network.chain_size_gb))
    });
    let supply_pct = Signal::derive(move || {
        live_field(|s| format!("{:.2}%", s.network.percent_issued))
    });
    let next_fee = Signal::derive(move || {
        live_field(|s| format!("{:.1} sat/vB", s.next_block_fee))
    });

    // Mempool gauge option
    let gauge_option = Signal::derive(move || {
        live.get()
            .and_then(|r| r.ok())
            .map(|s| {
                crate::stats::charts::mempool_gauge(
                    s.mempool.usage,
                    s.mempool.maxmempool,
                )
            })
            .unwrap_or_default()
    });

    // ---- Dashboard data ----
    let dashboard_data = LocalResource::new(move || {
        let r = range.get();
        async move {
            let stats =
                fetch_stats_summary().await.map_err(|e| e.to_string())?;

            let n = range_to_blocks(&r);
            let is_daily = n > 5_000;

            if is_daily {
                let from_ts = stats.latest_timestamp.saturating_sub(n * 600);
                let days =
                    fetch_daily_aggregates(from_ts, stats.latest_timestamp)
                        .await
                        .map_err(|e| e.to_string())?;
                Ok::<_, String>(DashboardData::Daily(days))
            } else {
                let from =
                    stats.min_height.max(stats.max_height.saturating_sub(n));
                let blocks = fetch_blocks(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(DashboardData::PerBlock(blocks))
            }
        }
    });

    // ---- Dashboard chart options ----
    let size_option = Signal::derive(move || {
        let _r = range.get();
        dashboard_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                DashboardData::PerBlock(ref blocks) => {
                    crate::stats::charts::block_size_chart(blocks, false)
                }
                DashboardData::Daily(ref days) => {
                    crate::stats::charts::block_size_chart_daily(days)
                }
            })
            .unwrap_or_default()
    });

    let tx_option = Signal::derive(move || {
        let _r = range.get();
        dashboard_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                DashboardData::PerBlock(ref blocks) => {
                    crate::stats::charts::tx_count_chart(blocks, false)
                }
                DashboardData::Daily(ref days) => {
                    crate::stats::charts::tx_count_chart_daily(days)
                }
            })
            .unwrap_or_default()
    });

    let fees_option = Signal::derive(move || {
        let _r = range.get();
        let unit = fee_unit.get();
        dashboard_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                DashboardData::PerBlock(ref blocks) => {
                    crate::stats::charts::fees_chart_unit(blocks, &unit)
                }
                DashboardData::Daily(ref days) => {
                    crate::stats::charts::fees_chart_daily_unit(days, &unit)
                }
            })
            .unwrap_or_default()
    });

    let diff_option = Signal::derive(move || {
        let _r = range.get();
        dashboard_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                DashboardData::PerBlock(ref blocks) => {
                    crate::stats::charts::difficulty_chart(blocks, false)
                }
                DashboardData::Daily(ref days) => {
                    crate::stats::charts::difficulty_chart_daily(days)
                }
            })
            .unwrap_or_default()
    });

    let interval_option = Signal::derive(move || {
        let _r = range.get();
        dashboard_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                DashboardData::PerBlock(ref blocks) => {
                    crate::stats::charts::block_interval_chart(blocks)
                }
                DashboardData::Daily(ref days) => {
                    crate::stats::charts::block_interval_chart_daily(days)
                }
            })
            .unwrap_or_default()
    });

    // ---- OP_RETURN data ----
    #[derive(Clone)]
    enum OpData {
        PerBlock(Vec<OpReturnBlock>),
        Daily(Vec<DailyAggregate>),
    }

    let op_data = LocalResource::new(move || {
        let r = range.get();
        async move {
            let stats =
                fetch_stats_summary().await.map_err(|e| e.to_string())?;
            let n = range_to_blocks(&r);

            if n > 5_000 {
                let from_ts = stats.latest_timestamp.saturating_sub(n * 600);
                let days =
                    fetch_daily_aggregates(from_ts, stats.latest_timestamp)
                        .await
                        .map_err(|e| e.to_string())?;
                Ok::<_, String>(OpData::Daily(days))
            } else {
                let from =
                    stats.min_height.max(stats.max_height.saturating_sub(n));
                let blocks = fetch_op_returns(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>(OpData::PerBlock(blocks))
            }
        }
    });

    let op_count_option = Signal::derive(move || {
        op_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                OpData::PerBlock(ref b) => {
                    crate::stats::charts::op_return_count_chart(b, false)
                }
                OpData::Daily(ref d) => {
                    crate::stats::charts::op_return_count_chart_daily(d)
                }
            })
            .unwrap_or_default()
    });

    let op_bytes_option = Signal::derive(move || {
        op_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                OpData::PerBlock(ref b) => {
                    crate::stats::charts::op_return_bytes_chart(b, false)
                }
                OpData::Daily(ref d) => {
                    crate::stats::charts::op_return_bytes_chart_daily(d)
                }
            })
            .unwrap_or_default()
    });

    let runes_pct_option = Signal::derive(move || {
        op_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                OpData::PerBlock(ref b) => {
                    crate::stats::charts::runes_pct_chart(b, false)
                }
                OpData::Daily(ref d) => {
                    crate::stats::charts::runes_pct_chart_daily(d)
                }
            })
            .unwrap_or_default()
    });

    // ---- BIP Signaling data ----
    let (bip_method, set_bip_method) = signal("bit".to_string());
    // Period offset: 0 = current, 1 = previous, etc.
    let (period_offset, set_period_offset) = signal(0u64);

    let signaling_data = LocalResource::new(move || {
        let method = bip_method.get();
        let offset = period_offset.get();
        async move {
            let stats =
                fetch_stats_summary().await.map_err(|e| e.to_string())?;
            let bit = if method == "locktime" { 0 } else { 4 };

            // Calculate period boundaries
            let current_period = stats.max_height / 2016;
            let target_period = current_period.saturating_sub(offset);
            let period_start = target_period * 2016;
            let period_end = (period_start + 2015).min(stats.max_height);

            let blocks_result =
                fetch_signaling(bit, method.clone(), period_start, period_end)
                    .await
                    .map_err(|e| e.to_string())?;

            // Only fetch periods chart data once (for the history chart)
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

    // ---- Tab names and ranges ----
    let tabs = vec![
        ("dashboard", "Dashboard"),
        ("opreturn", "OP_RETURN"),
        ("signaling", "BIP Signaling"),
    ];
    let range_buttons = move || {
        let ranges =
            vec!["1d", "1w", "1m", "3m", "6m", "1y", "2y", "5y", "10y", "all"];
        view! {
            <div class="flex flex-wrap gap-1.5 justify-center mb-6">
                {ranges.into_iter().map(|r| {
                    let r_str = r.to_string();
                    let r_display = r.to_uppercase();
                    let r_clone = r_str.clone();
                    view! {
                        <button
                            class=move || {
                                if range.get() == r_clone {
                                    "px-3 py-1.5 text-xs rounded-lg bg-white/10 text-white border border-white/20 font-medium cursor-pointer"
                                } else {
                                    "px-3 py-1.5 text-xs rounded-lg text-white/50 hover:text-white/80 hover:bg-white/5 transition-all cursor-pointer"
                                }
                            }
                            on:click={
                                let r = r_str.clone();
                                move |_| set_range.set(r.clone())
                            }
                        >
                            {r_display}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>
        }
    };

    view! {
        <Title text="Bitcoin Stats - WE HODL BTC"/>

        <section class="max-w-[1600px] mx-auto px-4 lg:px-10 pt-10 pb-28 opacity-0 animate-fadeinone">
            // Page header
            <div class="text-center mb-8">
                <h1 class="text-3xl lg:text-4xl font-title text-white mb-2">"Bitcoin Stats"</h1>
                <div class="w-12 h-0.5 bg-[#f7931a] mx-auto mt-2 mb-3"></div>
                <p class="text-sm text-white/50 max-w-lg mx-auto">
                    "Live blockchain metrics, block data, OP_RETURN analysis, and BIP signaling tracker."
                </p>
            </div>

            // Tab navigation
            <div class="flex flex-wrap gap-2 justify-center mb-6">
                {tabs.into_iter().map(|(id, label)| {
                    let id = id.to_string();
                    let label = label.to_string();
                    let id_clone = id.clone();
                    view! {
                        <button
                            class=move || {
                                if tab.get() == id_clone {
                                    "px-4 py-2 text-sm rounded-lg bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
                                } else {
                                    "px-4 py-2 text-sm rounded-lg text-white/60 hover:text-white hover:bg-white/5 transition-all cursor-pointer"
                                }
                            }
                            on:click={
                                let id = id.clone();
                                move |_| set_tab.set(id.clone())
                            }
                        >
                            {label}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>

            // ===== DASHBOARD TAB =====
            <div class=move || if tab.get() == "dashboard" { "block" } else { "hidden" }>

                // Live stats panel
                <div class="bg-white/5 border border-white/10 rounded-xl p-4 mb-6 animate-slideup" style="animation-delay: 100ms">
                    <div class="flex items-center gap-2 mb-3 flex-wrap">
                        <div class="w-2 h-2 rounded-full bg-green-500 animate-pulse"></div>
                        <span class="text-sm text-white font-medium">"Live Node Stats"</span>
                        <div class="flex items-center gap-2 ml-auto">
                            <span class="text-xs text-white/30">{move || last_updated.get()}</span>
                            <span class="text-xs text-white/20">
                                {move || format!("{}s", countdown.get())}
                            </span>
                            <button
                                class="text-xs text-white/40 hover:text-white/70 px-2 py-0.5 rounded border border-white/10 hover:border-white/20 cursor-pointer transition-all"
                                on:click=move |_| {
                                    set_countdown.set(30);
                                    live.refetch();
                                    set_last_updated.set(format!("updated {}", chrono::Local::now().format("%H:%M:%S")));
                                }
                            >
                                "Refresh"
                            </button>
                        </div>
                    </div>

                    <Suspense fallback=move || view! {
                        <div class="flex justify-center py-6"><Spinner/></div>
                    }>
                        {move || {
                            let _l = live.get();
                            view! {
                                <div class="grid grid-cols-1 lg:grid-cols-3 gap-4">
                                    // Mempool section
                                    <div class="bg-white/[0.03] border border-white/[0.07] rounded-xl p-4 overflow-hidden">
                                        <h3 class="text-xs font-semibold text-[#f7931a] uppercase tracking-wider mb-3">"Mempool"</h3>
                                        <div class="grid grid-cols-2 gap-2 mb-3">
                                            <LiveCard label="Transactions" value=mempool_size/>
                                            <LiveCard label="Size" value=mempool_bytes/>
                                            <LiveCard label="Next Block Fee" value=next_fee/>
                                        </div>
                                        <div class="flex justify-center">
                                            <Chart id="mempool-gauge".to_string() option=gauge_option class="w-[220px] h-[200px]".to_string()/>
                                        </div>
                                    </div>

                                    // Mining section
                                    <div class="bg-white/[0.03] border border-white/[0.07] rounded-xl p-4">
                                        <h3 class="text-xs font-semibold text-[#f7931a] uppercase tracking-wider mb-3">"Mining"</h3>
                                        <div class="grid grid-cols-2 gap-2 mb-2">
                                            <LiveCard label="Block Height" value=block_height/>
                                            <LiveCard label="Chain" value=chain/>
                                            <LiveCard label="Difficulty" value=difficulty/>
                                            <LiveCard label="Chain Size" value=chain_size/>
                                        </div>
                                    </div>

                                    // Economic section
                                    <div class="bg-white/[0.03] border border-white/[0.07] rounded-xl p-4">
                                        <h3 class="text-xs font-semibold text-[#f7931a] uppercase tracking-wider mb-3">"Economic"</h3>
                                        <div class="grid grid-cols-2 gap-2">
                                            <LiveCard label="Price (USD)" value=price_usd/>
                                            <LiveCard label="Sats/Dollar" value=sats_per_dollar/>
                                            <LiveCard label="Market Cap" value=market_cap/>
                                            <LiveCard label="Supply Issued" value=supply_pct/>
                                            <LiveCard label="UTXO Count" value=utxo_count/>
                                        </div>
                                    </div>
                                </div>
                            }
                        }}
                    </Suspense>
                </div>

                // Range selector
                {range_buttons()}

                // Charts
                <Suspense fallback=move || view! {
                    <div class="flex justify-center py-12"><Spinner/></div>
                }>
                    {move || {
                        let _d = dashboard_data.get();
                        view! {
                            <div class="space-y-10">
                                <div class="bg-white/5 border border-white/10 rounded-2xl p-5 lg:p-6 animate-slideup" style="animation-delay: 150ms">
                                    <Chart id="chart-size" option=size_option/>
                                </div>
                                <div class="bg-white/5 border border-white/10 rounded-2xl p-5 lg:p-6 animate-slideup" style="animation-delay: 200ms">
                                    <Chart id="chart-txcount" option=tx_option/>
                                </div>
                                <div class="bg-white/5 border border-white/10 rounded-2xl p-5 lg:p-6 animate-slideup" style="animation-delay: 250ms">
                                    <div class="flex justify-end mb-1">
                                        <button
                                            class="text-xs text-white/40 hover:text-white/60 px-2 py-1 rounded border border-white/10 cursor-pointer"
                                            on:click=move |_| {
                                                set_fee_unit.update(|u| {
                                                    *u = if *u == "sats" { "btc".to_string() } else { "sats".to_string() }
                                                });
                                            }
                                        >
                                            {move || if fee_unit.get() == "sats" { "Switch to BTC" } else { "Switch to sats" }}
                                        </button>
                                    </div>
                                    <Chart id="chart-fees" option=fees_option/>
                                </div>
                                <div class="bg-white/5 border border-white/10 rounded-2xl p-5 lg:p-6 animate-slideup" style="animation-delay: 300ms">
                                    <Chart id="chart-difficulty" option=diff_option/>
                                </div>
                                <div class="bg-white/5 border border-white/10 rounded-2xl p-5 lg:p-6 animate-slideup" style="animation-delay: 350ms">
                                    <Chart id="chart-interval" option=interval_option/>
                                </div>
                            </div>
                        }
                    }}
                </Suspense>
            </div>

            // ===== OP_RETURN TAB =====
            <div class=move || if tab.get() == "opreturn" { "block" } else { "hidden" }>
                // Range selector
                {range_buttons()}

                <Suspense fallback=move || view! {
                    <div class="flex justify-center py-12"><Spinner/></div>
                }>
                    {move || {
                        let _d = op_data.get();
                        view! {
                            <div class="space-y-10">
                                <div class="bg-white/5 border border-white/10 rounded-2xl p-5 lg:p-6 animate-slideup" style="animation-delay: 100ms">
                                    <Chart id="chart-opreturn-count" option=op_count_option/>
                                </div>
                                <div class="bg-white/5 border border-white/10 rounded-2xl p-5 lg:p-6 animate-slideup" style="animation-delay: 150ms">
                                    <Chart id="chart-opreturn-bytes" option=op_bytes_option/>
                                </div>
                                <div class="bg-white/5 border border-white/10 rounded-2xl p-5 lg:p-6 animate-slideup" style="animation-delay: 200ms">
                                    <Chart id="chart-runes-pct" option=runes_pct_option/>
                                </div>
                            </div>
                        }
                    }}
                </Suspense>
            </div>

            // ===== BIP SIGNALING TAB =====
            <div class=move || if tab.get() == "signaling" { "block" } else { "hidden" }>

                // BIP selector
                <div class="flex flex-wrap gap-2 justify-center mb-6">
                    <button
                        class=move || if bip_method.get() == "bit" {
                            "px-4 py-2 text-sm rounded-lg bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
                        } else {
                            "px-4 py-2 text-sm rounded-lg text-white/60 hover:text-white hover:bg-white/5 transition-all cursor-pointer"
                        }
                        on:click=move |_| set_bip_method.set("bit".to_string())
                    >
                        "BIP-110: OP_RETURN Limits (Bit 4)"
                    </button>
                    <button
                        class=move || if bip_method.get() == "locktime" {
                            "px-4 py-2 text-sm rounded-lg bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
                        } else {
                            "px-4 py-2 text-sm rounded-lg text-white/60 hover:text-white hover:bg-white/5 transition-all cursor-pointer"
                        }
                        on:click=move |_| set_bip_method.set("locktime".to_string())
                    >
                        "BIP-54: Consensus Cleanup (Locktime)"
                    </button>
                </div>

                // BIP info card
                <div class="bg-white/5 border border-white/10 rounded-xl p-4 mb-4 text-sm text-white/70">
                    {move || {
                        if bip_method.get() == "locktime" {
                            view! {
                                <div>
                                    <div class="text-white font-medium mb-1">"BIP-54: Consensus Cleanup"</div>
                                    <p class="text-xs text-white/50">"Fixes timewarp attack, reduces worst-case validation time, prevents Merkle tree weaknesses. Signaled by setting coinbase nLockTime to block height - 1 and nSequence != 0xffffffff."</p>
                                    <p class="text-xs text-white/40 mt-1">"Signal method: Coinbase locktime | Threshold: 95%"</p>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div>
                                    <div class="text-white font-medium mb-1">"BIP-110: OP_RETURN Data Limits"</div>
                                    <p class="text-xs text-white/50">"Caps transaction outputs at 34 bytes and OP_RETURN data at 83 bytes. Temporary softfork \u{2014} expires after 52,416 blocks (~1 year). Modified BIP9: 55% threshold (1,109/2,016). Signaled via version bit 4."</p>
                                    <p class="text-xs text-white/40 mt-1">"Signal method: Version bit 4 | Threshold: 55%"</p>
                                </div>
                            }.into_any()
                        }
                    }}
                </div>

                // Period navigator
                <div class="flex items-center justify-center gap-3 mb-6">
                    <button
                        class="px-3 py-1.5 text-xs rounded-lg text-white/50 hover:text-white hover:bg-white/5 transition-all cursor-pointer"
                        on:click=move |_| set_period_offset.update(|o| *o = (*o + 1).min(11))
                    >
                        "\u{2190} Older"
                    </button>
                    <span class="text-xs text-white/40">
                        {move || {
                            let o = period_offset.get();
                            if o == 0 { "Current Period".to_string() } else { format!("{} periods ago", o) }
                        }}
                    </span>
                    <button
                        class=move || {
                            if period_offset.get() == 0 {
                                "px-3 py-1.5 text-xs rounded-lg text-white/20 cursor-not-allowed"
                            } else {
                                "px-3 py-1.5 text-xs rounded-lg text-white/50 hover:text-white hover:bg-white/5 transition-all cursor-pointer"
                            }
                        }
                        disabled=move || period_offset.get() == 0
                        on:click=move |_| set_period_offset.update(|o| *o = o.saturating_sub(1))
                    >
                        "Newer \u{2192}"
                    </button>
                </div>

                <Suspense fallback=move || view! {
                    <div class="flex justify-center py-12"><Spinner/></div>
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
                                            "Period {} \u{2013} {}: {} signaled / {} mined of 2,016 ({:.1}%) \u{2014} {} remaining \u{2014} threshold: {}%",
                                            format_number(p_start), format_number(p_end),
                                            period_stats.signaled_count, mined, pct, remaining, threshold as u32,
                                        )
                                    } else {
                                        format!(
                                            "Period {} \u{2013} {}: {} signaled / {} blocks ({:.1}%) \u{2014} threshold: {}%",
                                            format_number(p_start), format_number(p_end),
                                            period_stats.signaled_count, mined, pct, threshold as u32,
                                        )
                                    };

                                    // Build grid cells
                                    let grid_cells = blocks.iter().map(|b| {
                                        let color = if b.signaled { "bg-green-500/70" } else { "bg-red-500/30" };
                                        let title = format!("#{} \u{2014} {}{}", b.height, b.miner, if b.signaled { " \u{2713}" } else { "" });
                                        let h = b.height;
                                        view! {
                                            <div
                                                class=format!("w-2.5 h-2.5 rounded-sm cursor-pointer hover:ring-1 hover:ring-white/40 {color}")
                                                title=title
                                                on:click=move |_| { show_block_detail(h); }
                                            ></div>
                                        }
                                    }).collect::<Vec<_>>();

                                    // Filter periods chart to last 20 relevant periods
                                    let start_height = if bip_method.get() == "locktime" { 940_000u64 } else { 936_000 };
                                    let filtered: Vec<_> = periods.iter()
                                        .filter(|p| p.end_height >= start_height)
                                        .cloned()
                                        .collect();
                                    let periods_chart = crate::stats::charts::signaling_periods_chart(&filtered, threshold);

                                    view! {
                                        <div class="space-y-10">
                                            // Progress bar
                                            <div class="bg-white/5 border border-white/10 rounded-xl p-4">
                                                <div class="h-3 bg-white/5 rounded-full overflow-hidden mb-2">
                                                    <div
                                                        class="h-full rounded-full transition-all duration-500"
                                                        style=format!("width: {bar_width}; background: {bar_color}")
                                                    ></div>
                                                </div>
                                                <p class="text-xs text-white/50 text-center">{period_text}</p>
                                            </div>

                                            // Block grid
                                            <div class="bg-white/5 border border-white/10 rounded-xl p-4">
                                                <p class="text-xs text-white/40 mb-2">
                                                    {format!("Blocks {} \u{2013} {} (click for details)", format_number(p_start), format_number(p_end))}
                                                </p>
                                                <div class="flex flex-wrap gap-[3px]">
                                                    {grid_cells}
                                                </div>
                                            </div>

                                            // History chart
                                            <div class="bg-white/5 border border-white/10 rounded-2xl p-5 lg:p-6">
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
            </div>

            // Block detail modal (hidden by default, shown via JS)
            <div id="block-detail-modal" class="hidden fixed inset-0 z-50 flex items-center justify-center p-4">
                <div class="absolute inset-0 bg-black/60" onclick="closeBlockDetail()"></div>
                <div class="relative bg-[#0e2a47] border border-white/15 rounded-2xl shadow-2xl w-full max-w-md max-h-[80vh] overflow-y-auto">
                    <div class="flex items-center justify-between px-5 py-3 border-b border-white/10">
                        <span id="block-detail-title" class="text-white font-medium">"Block"</span>
                        <button
                            class="text-white/40 hover:text-white text-lg cursor-pointer"
                            onclick="closeBlockDetail()"
                        >"\u{2715}"</button>
                    </div>
                    <div id="block-detail-body" class="px-5 py-4 text-sm text-white/80 space-y-1"
                        style="--bd-label: rgba(255,255,255,0.5)"
                    ></div>
                </div>
            </div>
        </section>

        // Inline styles for block detail rows (can't use Tailwind for JS-injected HTML)
        <style>"
            .bd-row { display: flex; justify-content: space-between; padding: 4px 0; }
            .bd-row span:first-child { color: rgba(255,255,255,0.5); }
            .bd-row span:last-child { color: rgba(255,255,255,0.85); font-family: monospace; font-size: 12px; }
            .bd-hash { color: #f7931a !important; }
            .bd-divider { border-top: 1px solid rgba(255,255,255,0.08); margin: 8px 0; }
        "</style>
    }
}

// ---------------------------------------------------------------------------
// Data enum for dashboard
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum DashboardData {
    PerBlock(Vec<BlockSummary>),
    Daily(Vec<DailyAggregate>),
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    bytes
        .rchunks(3)
        .rev()
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join(",")
}

fn format_number_f64(n: f64, decimals: usize) -> String {
    let rounded = format!("{:.prec$}", n, prec = decimals);
    // Add commas to the integer part
    let parts: Vec<&str> = rounded.split('.').collect();
    let int_part = parts[0];
    let bytes = int_part.as_bytes();
    let formatted = bytes
        .rchunks(3)
        .rev()
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join(",");
    if parts.len() > 1 {
        format!("{}.{}", formatted, parts[1])
    } else {
        formatted
    }
}
