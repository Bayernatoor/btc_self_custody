//! Whale Watch: dedicated page for browsing notable transactions.
//!
//! Extends the real-time feed on the Heartbeat page with historical browsing,
//! filtering, aggregation stats, and a leaderboard.

use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::hooks::use_query_map;

#[cfg(feature = "hydrate")]
use leptos::web_sys;
#[cfg(feature = "hydrate")]
use wasm_bindgen::prelude::*;

use crate::stats::server_fns::{
    fetch_notable_stats, fetch_notable_top, fetch_notable_txs,
};
use crate::stats::types::{NotableTxFilter, NotableTxInfo};

// JS interop: call window.showTxDetail defined in stats.js.
// Second arg is a JSON string of local data (fee, vsize, value, feeRate).
#[cfg(feature = "hydrate")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = showTxDetail, catch)]
    fn js_show_tx_detail(
        txid: &str,
        local_data: JsValue,
    ) -> Result<(), JsValue>;
}

#[cfg(not(feature = "hydrate"))]
#[allow(dead_code)]
fn js_show_tx_detail(_txid: &str, _local_data: ()) -> Result<(), ()> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Filter enum (matches server-side notable_type strings)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TypeFilter {
    All,
    Whale,
    RoundNumber,
    LargeInscription,
    Consolidation,
    FanOut,
    FeeOutlier,
    OpReturnMsg,
}

impl TypeFilter {
    fn slug(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Whale => "whale",
            Self::RoundNumber => "round_number",
            Self::LargeInscription => "large_inscription",
            Self::Consolidation => "consolidation",
            Self::FanOut => "fan_out",
            Self::FeeOutlier => "fee_outlier",
            Self::OpReturnMsg => "op_return_msg",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Whale => "Whales",
            Self::RoundNumber => "Round Numbers",
            Self::LargeInscription => "Inscriptions",
            Self::Consolidation => "Consolidations",
            Self::FanOut => "Fan-outs",
            Self::FeeOutlier => "Fee Outliers",
            Self::OpReturnMsg => "Messages",
        }
    }

    fn color(self) -> &'static str {
        match self {
            Self::All => "#ffffff",
            Self::Whale => "#ffd700",
            Self::RoundNumber => "#90ee90",
            Self::LargeInscription => "#ff00c8",
            Self::Consolidation => "#a855f7",
            Self::FanOut => "#00d2ff",
            Self::FeeOutlier => "#ff4444",
            Self::OpReturnMsg => "#ffa500",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::All => "All notable transactions",
            Self::Whale => "Total output value over $1,000,000 USD. May include exchange self-sends.",
            Self::RoundNumber => "Exact round BTC amounts (1, 10, 100, 1000). Often human-initiated",
            Self::LargeInscription => "Witness data over 100KB (Ordinals, BRC-20, images)",
            Self::Consolidation => "50+ inputs merged into 3 or fewer outputs (UTXO cleanup)",
            Self::FanOut => "3 or fewer inputs sprayed to 100+ outputs (batch payouts)",
            Self::FeeOutlier => "Fee rate over 2000 sat/vB or absolute fee over 0.1 BTC",
            Self::OpReturnMsg => "Transactions embedding readable ASCII text on-chain",
        }
    }

    fn from_slug(s: &str) -> Self {
        match s {
            "whale" => Self::Whale,
            "round_number" => Self::RoundNumber,
            "large_inscription" => Self::LargeInscription,
            "consolidation" => Self::Consolidation,
            "fan_out" => Self::FanOut,
            "fee_outlier" => Self::FeeOutlier,
            "op_return_msg" => Self::OpReturnMsg,
            _ => Self::All,
        }
    }

    fn to_filter(self) -> Option<String> {
        if self == Self::All {
            None
        } else {
            Some(self.slug().to_string())
        }
    }
}

const FILTERS: &[TypeFilter] = &[
    TypeFilter::All,
    TypeFilter::Whale,
    TypeFilter::RoundNumber,
    TypeFilter::LargeInscription,
    TypeFilter::Consolidation,
    TypeFilter::FanOut,
    TypeFilter::FeeOutlier,
    TypeFilter::OpReturnMsg,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TimeWindow {
    Hour,
    Day,
    Week,
    Month,
    AllTime,
}

impl TimeWindow {
    fn label(self) -> &'static str {
        match self {
            Self::Hour => "1H",
            Self::Day => "24H",
            Self::Week => "7D",
            Self::Month => "30D",
            Self::AllTime => "All",
        }
    }

    fn seconds_ago(self) -> u64 {
        match self {
            Self::Hour => 3600,
            Self::Day => 86400,
            Self::Week => 604800,
            Self::Month => 2_592_000,
            Self::AllTime => 1_900_000_000, // far in the past
        }
    }

    fn since(self) -> u64 {
        let now = now_secs();
        now.saturating_sub(self.seconds_ago())
    }
}

fn now_secs() -> u64 {
    // chrono::Utc::now() works on both SSR (std) and WASM (chrono pulls in js-sys)
    chrono::Utc::now().timestamp() as u64
}

const WINDOWS: &[TimeWindow] = &[
    TimeWindow::Hour,
    TimeWindow::Day,
    TimeWindow::Week,
    TimeWindow::Month,
    TimeWindow::AllTime,
];

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

#[component]
pub fn WhaleWatchPage() -> impl IntoView {
    let query = use_query_map();

    let initial_filter = query
        .read_untracked()
        .get("type")
        .map(|s| TypeFilter::from_slug(&s))
        .unwrap_or(TypeFilter::All);

    let (active_filter, set_active_filter) = signal(initial_filter);
    let (window, set_window) = signal(TimeWindow::Day);
    let (page, set_page) = signal(0u64);

    const PAGE_SIZE: u64 = 50;

    // Reset page to 0 when filter or window changes
    Effect::new(move || {
        let _ = active_filter.get();
        let _ = window.get();
        set_page.set(0);
    });

    // Main tx list resource
    let tx_list = LocalResource::new(move || {
        let filter = active_filter.get();
        let w = window.get();
        let p = page.get();
        async move {
            let f = NotableTxFilter {
                notable_type: filter.to_filter(),
                since: Some(w.since()),
                ..Default::default()
            };
            fetch_notable_txs(f, PAGE_SIZE, p * PAGE_SIZE).await
        }
    });

    // Aggregate stats resource
    let stats = LocalResource::new(move || {
        let w = window.get();
        async move { fetch_notable_stats(w.since()).await }
    });

    // Top-value leaderboard
    let top = LocalResource::new(move || {
        let w = window.get();
        async move { fetch_notable_top(w.since(), 10).await }
    });

    // URL sync
    #[cfg(feature = "hydrate")]
    {
        Effect::new(move || {
            let f = active_filter.get();
            if let Some(win) = web_sys::window() {
                if let Ok(history) = win.history() {
                    let slug = f.slug();
                    let search = if slug == "all" {
                        String::new()
                    } else {
                        format!("?type={slug}")
                    };
                    let path = format!("/observatory/whale-watch{search}");
                    let _ = history.replace_state_with_url(
                        &web_sys::wasm_bindgen::JsValue::NULL,
                        "",
                        Some(&path),
                    );
                }
            }
        });
    }

    let (info_open, set_info_open) = signal(false);

    view! {
        <Title text="Whale Watch: Notable Bitcoin Transactions | WE HODL BTC"/>
        <Meta name="description" content="Real-time and historical browser for notable Bitcoin transactions: whales, round-number transfers, large inscriptions, consolidations, fan-outs, fee outliers, and on-chain messages."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/whale-watch"/>

        // SEO text (hidden, crawlable)
        <p class="sr-only">"Track notable Bitcoin transactions in real-time. Whale Watch detects million-dollar transfers, round-number amounts, UTXO consolidations, exchange batch payouts, large Ordinals inscriptions, extreme fee rates, and readable OP_RETURN messages as they enter the mempool. Browse historical data with filters, leaderboards, and aggregated statistics."</p>

        // Hero banner (matches other Observatory pages)
        <div class="relative rounded-2xl overflow-hidden mb-5">
            <img
                src="/img/observatory_hero.png"
                alt="Whale Watch"
                class="w-full h-[100px] sm:h-[120px] lg:h-[140px] object-cover object-center"
            />
            <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/60 to-[#123c64]/30"></div>
            <div class="absolute inset-0 flex flex-col items-center justify-end pb-3 sm:pb-4">
                <h1 class="text-lg sm:text-xl lg:text-2xl font-title text-white mb-0.5 drop-shadow-lg">"Whale Watch"</h1>
                <p class="text-[11px] sm:text-xs text-white/50 max-w-lg mx-auto px-4 text-center drop-shadow">"Notable transactions detected in real-time from my Bitcoin node"</p>
            </div>
        </div>

        // Info button + time window (toolbar row)
        <div class="flex flex-col sm:flex-row sm:items-center gap-3 mb-5">
            <div class="flex items-center gap-2">
                <button
                    class="text-white/30 hover:text-[#f7931a] transition-colors cursor-pointer shrink-0"
                    title="About Whale Watch"
                    on:click=move |_| set_info_open.update(|v| *v = !*v)
                >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                        <circle cx="12" cy="12" r="10"/>
                        <path d="M12 16v-4M12 8h.01"/>
                    </svg>
                </button>
                <a href="/observatory/heartbeat" class="text-xs text-white/30 hover:text-[#f7931a] transition-colors">
                    "Live Heartbeat \u{2192}"
                </a>
            </div>
            <div class="flex items-center gap-2 sm:ml-auto">
                {WINDOWS.iter().map(|&w| {
                    view! {
                        <button
                            on:click=move |_| set_window.set(w)
                            class=move || {
                                if window.get() == w {
                                    "px-3 py-1.5 rounded-full text-xs font-semibold bg-[#f7931a] text-black whitespace-nowrap transition-all active:scale-95 active:opacity-70"
                                } else {
                                    "px-3 py-1.5 rounded-full text-xs font-medium bg-white/5 text-white/50 hover:bg-white/10 hover:text-white/70 whitespace-nowrap transition-all active:scale-95 active:opacity-70"
                                }
                            }
                        >
                            {w.label()}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>

        // Info panel (toggleable)
        <Show when=move || info_open.get()>
            <div class="mb-5 p-3 sm:p-4 bg-white/[0.03] border border-white/5 rounded-xl text-xs sm:text-sm text-white/60 leading-relaxed space-y-2 opacity-0 animate-fadeinone">
                <p>"Every transaction entering my node's mempool is analyzed in real-time for notable characteristics. Detected transactions are highlighted on the Block Heartbeat EKG and stored permanently for historical browsing."</p>
                <p>
                    <span class="text-[#f7931a]/80 font-semibold">"Work in progress."</span>
                    " Detection rules are still being tuned. Some transactions may be misclassified, some notable patterns may be missed, and thresholds will shift as the feature evolves. Feedback welcome."
                </p>
                <p>
                    <span class="text-white/70 font-semibold">"Historical data is limited to what has been detected since the feature launched."</span>
                    " Older notable transactions are not yet surfaced here. A full backfill from the genesis block is on the roadmap but requires stable detection logic first (no point reprocessing a billion transactions while thresholds are still changing) and historical daily price data for accurate whale classification across Bitcoin's full history."
                </p>
                <p class="text-white/40 text-[11px] sm:text-xs">"Detection uses total output value for whale classification. Exchange self-sends (hot/cold wallet reshuffles) may appear as whales since change output detection requires UTXO data not available from raw transaction parsing."</p>
            </div>
        </Show>

        // Stats cards
        <StatsCards stats=stats/>

        // Type filter pills (rounded, matching Hall of Fame style)
        <div class="flex items-center gap-2 overflow-x-auto pb-2 mb-4 scrollbar-hide">
            {FILTERS.iter().map(|&f| {
                let color = f.color();
                let label = f.label();
                let desc = f.description();
                view! {
                    <button
                        on:click=move |_| set_active_filter.set(f)
                        title=desc
                        class=move || {
                            if active_filter.get() == f {
                                "px-3 py-1.5 rounded-full text-xs font-semibold whitespace-nowrap transition-all flex-shrink-0 active:scale-95"
                            } else {
                                "px-3 py-1.5 rounded-full text-xs font-medium whitespace-nowrap transition-all flex-shrink-0 hover:opacity-80 active:scale-95"
                            }
                        }
                        style=move || {
                            if active_filter.get() == f {
                                format!("background-color: {color}; color: #0a1a2e;")
                            } else {
                                format!("background-color: {color}15; color: {color}cc; border: 1px solid {color}30;")
                            }
                        }
                    >
                        {label}
                    </button>
                }
            }).collect::<Vec<_>>()}
        </div>

        // Leaderboard (top 10 by USD)
        <TopLeaderboard top=top/>

        // Main tx list
        <div class="bg-[#0d2137] border border-white/10 rounded-2xl overflow-hidden mb-4">
            <div class="flex items-center justify-between px-3 sm:px-4 py-2.5 border-b border-white/10">
                <span class="text-[10px] sm:text-xs font-mono text-white/60 uppercase tracking-wider">"Transactions"</span>
                <span class="text-[10px] sm:text-xs font-mono text-white/30">
                    {move || match tx_list.get() {
                        Some(Ok(p)) => format!("{} total", p.total),
                        _ => "Loading...".to_string(),
                    }}
                </span>
            </div>
                <Suspense fallback=|| view! {
                    <div class="px-4 py-8 text-center text-white/30 text-sm">"Loading transactions..."</div>
                }>
                    {move || tx_list.get().map(|result| match result {
                        Ok(page_data) => {
                            if page_data.items.is_empty() {
                                view! {
                                    <div class="px-4 py-12 text-center text-white/30 text-sm italic">
                                        "No notable transactions in this window"
                                    </div>
                                }.into_any()
                            } else {
                                let items = page_data.items;
                                view! {
                                    <div>
                                        {items.into_iter().map(|tx| view! { <TxRow tx=tx/> }).collect::<Vec<_>>()}
                                    </div>
                                }.into_any()
                            }
                        }
                        Err(_) => view! {
                            <div class="px-4 py-8 text-center text-[#ff4444] text-sm">"Failed to load transactions"</div>
                        }.into_any(),
                    })}
                </Suspense>

                // Pagination
                {move || {
                    let current_page = page.get();
                    let total = tx_list.get().and_then(|r| r.ok()).map(|p| p.total).unwrap_or(0);
                    let total_pages = total.div_ceil(PAGE_SIZE.max(1));
                    if total_pages <= 1 {
                        view! { <div></div> }.into_any()
                    } else {
                        view! {
                            <div class="flex items-center justify-between px-3 sm:px-4 py-2 sm:py-2.5 border-t border-white/10">
                                <button
                                    on:click=move |_| {
                                        if current_page > 0 { set_page.set(current_page - 1); }
                                    }
                                    disabled=move || page.get() == 0
                                    class="px-2.5 sm:px-3 py-1 rounded text-[11px] sm:text-xs font-mono bg-white/5 text-white/60 border border-white/10 hover:bg-white/10 active:scale-95 disabled:opacity-30 disabled:cursor-not-allowed transition-all"
                                >
                                    "\u{2190} Prev"
                                </button>
                                <span class="text-[10px] sm:text-xs font-mono text-white/40">
                                    {format!("Page {} of {}", current_page + 1, total_pages)}
                                </span>
                                <button
                                    on:click=move |_| {
                                        let p = page.get_untracked();
                                        if p + 1 < total_pages { set_page.set(p + 1); }
                                    }
                                    disabled=move || {
                                        let p = page.get();
                                        p + 1 >= total_pages
                                    }
                                    class="px-2.5 sm:px-3 py-1 rounded text-[11px] sm:text-xs font-mono bg-white/5 text-white/60 border border-white/10 hover:bg-white/10 active:scale-95 disabled:opacity-30 disabled:cursor-not-allowed transition-all"
                                >
                                    "Next \u{2192}"
                                </button>
                            </div>
                        }.into_any()
                    }
                }}
            </div>

    }
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

#[component]
fn StatsCards(
    stats: LocalResource<
        Result<crate::stats::types::NotableStatsInfo, ServerFnError>,
    >,
) -> impl IntoView {
    view! {
        <Suspense fallback=|| view! {
            <div class="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-4">
                <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4 h-[72px] animate-pulse"></div>
                <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4 h-[72px] animate-pulse"></div>
                <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4 h-[72px] animate-pulse"></div>
                <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4 h-[72px] animate-pulse"></div>
            </div>
        }>
            {move || stats.get().map(|result| match result {
                Ok(s) => {
                    let total_count = s.total_count;
                    let total_usd = s.total_value_usd;
                    let top_usd = s.top_value_usd;
                    let top_txid = s.top_txid.clone().unwrap_or_default();
                    let top_type = s.by_type.first().map(|(t, _, _)| t.clone()).unwrap_or_else(|| "—".to_string());
                    let top_type_count = s.by_type.first().map(|(_, c, _)| *c).unwrap_or(0);
                    view! {
                        <div class="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-5">
                            <StatCard label="Detected".to_string() value={total_count.to_string()} sub="notable transactions".to_string()/>
                            <StatCard label="Combined Value".to_string() value={format!("${}", fmt_usd_short(total_usd))} sub="total output volume".to_string()/>
                            <StatCard label="Largest".to_string() value={format!("${}", fmt_usd_short(top_usd))} sub={
                                if top_txid.is_empty() { "in this window".to_string() } else { format!("{}...", &top_txid[..12.min(top_txid.len())]) }
                            }/>
                            <StatCard label="Top Category".to_string() value={pretty_type(&top_type)} sub={format!("{} detected", top_type_count)}/>
                        </div>
                    }.into_any()
                }
                Err(_) => view! { <div></div> }.into_any(),
            })}
        </Suspense>
    }
}

#[component]
fn StatCard(label: String, value: String, sub: String) -> impl IntoView {
    view! {
        <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4">
            <div class="text-[10px] font-mono text-white/40 uppercase tracking-wider mb-1">{label}</div>
            <div class="text-xl sm:text-2xl font-bold text-[#f7931a] font-mono leading-tight">{value}</div>
            <div class="text-[11px] font-mono text-white/30 mt-0.5">{sub}</div>
        </div>
    }
}

#[component]
fn TopLeaderboard(
    top: LocalResource<Result<Vec<NotableTxInfo>, ServerFnError>>,
) -> impl IntoView {
    view! {
        <div class="bg-[#0d2137] border border-white/10 rounded-2xl overflow-hidden mb-4">
            <div class="px-3 sm:px-4 py-2.5 border-b border-white/10">
                <span class="text-[10px] sm:text-xs font-mono text-[#ffd700]/80 uppercase tracking-wider">
                    "\u{2605} Top 10 by Value"
                </span>
            </div>
            <Suspense fallback=|| view! {
                <div class="px-4 py-6 text-center text-white/30 text-sm">"Loading leaderboard..."</div>
            }>
                {move || top.get().map(|result| match result {
                    Ok(items) if !items.is_empty() => {
                        view! {
                            <div>
                                {items.into_iter().enumerate().map(|(i, tx)| {
                                    let rank = i + 1;
                                    view! { <LeaderRow rank=rank tx=tx/> }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_any()
                    }
                    _ => view! {
                        <div class="px-4 py-6 text-center text-white/30 text-sm italic">"No data in this window"</div>
                    }.into_any(),
                })}
            </Suspense>
        </div>
    }
}

#[component]
fn LeaderRow(rank: usize, tx: NotableTxInfo) -> impl IntoView {
    let tx_for_click = tx.clone();
    let txid_for_click = tx.txid.clone();
    let txid_short = shorten_txid(&tx.txid);
    let type_color = notable_color(&tx.notable_type);
    let type_label = pretty_type(&tx.notable_type);
    let usd_str = fmt_usd_short(tx.value_usd);
    let btc_str = fmt_btc(tx.value);
    let time_str = fmt_time_ago(tx.first_seen);

    view! {
        <div
            class="flex items-baseline gap-x-3 gap-y-0.5 flex-wrap px-3 sm:px-4 py-2 border-b border-white/5 text-xs font-mono cursor-pointer hover:bg-white/5 active:scale-[0.99] transition-all"
            on:click=move |_| show_tx_detail_with_data(&txid_for_click, &tx_for_click)
        >
            <span class="text-white/30 w-5 shrink-0">{format!("#{}", rank)}</span>
            <span class="font-bold shrink-0" style=format!("color: {}", type_color)>
                {format!("${}", usd_str)}
            </span>
            <span class="text-white/50 shrink-0">{btc_str}</span>
            <span class="text-[10px] shrink-0" style=format!("color: {}aa", type_color)>
                {type_label}
            </span>
            <span class="text-white/30 text-[11px] ml-auto shrink-0 hidden sm:inline">{txid_short}</span>
            <span class="text-white/20 text-[10px] shrink-0">{time_str}</span>
        </div>
    }
}

#[component]
fn TxRow(tx: NotableTxInfo) -> impl IntoView {
    let tx_for_click = tx.clone();
    let txid_for_click = tx.txid.clone();
    let txid_short = shorten_txid(&tx.txid);
    let type_color = notable_color(&tx.notable_type);
    let type_label = pretty_type(&tx.notable_type);
    let usd_str = fmt_usd_short(tx.value_usd);
    let btc_str = fmt_btc(tx.value);
    let time_str = fmt_time_ago(tx.first_seen);
    let fee_str = if tx.vsize > 0 {
        format!("{:.1} sat/vB", tx.fee as f64 / tx.vsize as f64)
    } else {
        "—".to_string()
    };
    let io_str = format!("{}in / {}out", tx.input_count, tx.output_count);
    let io_str_mobile = io_str.clone();
    let fee_str_mobile = fee_str.clone();
    let txid_short_mobile = txid_short.clone();
    let confirmed = tx.confirmed_height.is_some();
    let conf_badge = if confirmed {
        format!("#{}", tx.confirmed_height.unwrap_or(0))
    } else {
        "mempool".to_string()
    };
    let msg_text = tx.op_return_text.clone().unwrap_or_default();

    view! {
        <div
            class="px-3 sm:px-4 py-2.5 sm:py-3 border-b border-white/5 text-xs font-mono cursor-pointer hover:bg-white/5 active:scale-[0.99] transition-all"
            on:click=move |_| show_tx_detail_with_data(&txid_for_click, &tx_for_click)
        >
            // Row 1: type + value + BTC (always visible)
            <div class="flex items-baseline gap-x-2 sm:gap-x-3 flex-wrap gap-y-0.5">
                <span class="font-bold shrink-0" style=format!("color: {}", type_color)>
                    {type_label}
                </span>
                <span class="text-[#f7931a] shrink-0">{format!("${}", usd_str)}</span>
                <span class="text-white/50 shrink-0">{btc_str}</span>
                <span class="text-white/40 shrink-0 hidden sm:inline">{io_str}</span>
                <span class="text-white/40 shrink-0 hidden sm:inline">{fee_str}</span>
                <span class="text-white/30 text-[10px] shrink-0">
                    {if confirmed { "\u{2713}" } else { "\u{25CB}" }}
                    " "
                    {conf_badge}
                </span>
                <span class="text-white/20 text-[11px] ml-auto shrink-0">{time_str}</span>
                <span class="text-white/30 text-[11px] shrink-0 hidden sm:inline">{txid_short}</span>
            </div>
            // Row 2: mobile-only compact details
            <div class="flex items-baseline gap-2 mt-0.5 sm:hidden text-[10px] text-white/30">
                <span>{io_str_mobile}</span>
                <span>{fee_str_mobile}</span>
                <span>{txid_short_mobile}</span>
            </div>
            {if !msg_text.is_empty() {
                let quoted = format!("\"{}\"", &msg_text.chars().take(120).collect::<String>());
                view! {
                    <div class="mt-1 text-[10px] sm:text-[11px] text-white/40 italic truncate">{quoted}</div>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
        </div>
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format BTC with enough precision to avoid rounding artifacts.
/// 99.99998 BTC should NOT display as "100.0000 BTC".
fn fmt_btc(sats: u64) -> String {
    let btc = sats as f64 / 1e8;
    if btc >= 100.0 {
        // Show 6 decimals for large values to avoid false round-number appearance
        let s = format!("{:.6}", btc);
        // Trim trailing zeros but keep at least 2 decimals
        let trimmed = s.trim_end_matches('0');
        let trimmed = if trimmed.ends_with('.') {
            &s[..trimmed.len() + 2]
        } else {
            trimmed
        };
        // Ensure at least 2 decimal places
        let dot_pos = trimmed.find('.').unwrap_or(trimmed.len());
        let decimals = trimmed.len() - dot_pos - 1;
        if decimals < 2 {
            format!("{:.2} BTC", btc)
        } else {
            format!("{trimmed} BTC")
        }
    } else if btc >= 1.0 {
        format!("{:.4} BTC", btc)
    } else if btc >= 0.01 {
        format!("{:.6} BTC", btc)
    } else {
        format!("{:.8} BTC", btc)
    }
}

fn shorten_txid(txid: &str) -> String {
    if txid.len() >= 16 {
        format!("{}...{}", &txid[..8], &txid[txid.len() - 6..])
    } else {
        txid.to_string()
    }
}

fn notable_color(t: &str) -> &'static str {
    match t {
        "whale" => "#ffd700",
        "round_number" => "#90ee90",
        "large_inscription" => "#ff00c8",
        "consolidation" => "#a855f7",
        "fan_out" => "#00d2ff",
        "fee_outlier" => "#ff4444",
        "op_return_msg" => "#ffa500",
        _ => "#ffffff",
    }
}

fn pretty_type(t: &str) -> String {
    match t {
        "whale" => "Whale".to_string(),
        "round_number" => "Round #".to_string(),
        "large_inscription" => "Inscription".to_string(),
        "consolidation" => "Consolidation".to_string(),
        "fan_out" => "Fan-out".to_string(),
        "fee_outlier" => "Fee Outlier".to_string(),
        "op_return_msg" => "Message".to_string(),
        _ => t.to_string(),
    }
}

fn fmt_usd_short(v: f64) -> String {
    if v >= 1_000_000_000.0 {
        format!("{:.2}B", v / 1_000_000_000.0)
    } else if v >= 1_000_000.0 {
        format!("{:.2}M", v / 1_000_000.0)
    } else if v >= 1_000.0 {
        format!("{:.1}K", v / 1_000.0)
    } else {
        format!("{:.0}", v)
    }
}

fn fmt_time_ago(ts: u64) -> String {
    let now = now_secs();
    if ts >= now {
        return "just now".to_string();
    }
    let delta = now - ts;
    if delta < 60 {
        format!("{}s ago", delta)
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86400 {
        format!("{}h ago", delta / 3600)
    } else {
        format!("{}d ago", delta / 86400)
    }
}

/// Open the tx detail modal with local data pre-populated.
#[cfg(feature = "hydrate")]
fn show_tx_detail_with_data(txid: &str, tx: &NotableTxInfo) {
    let fee_rate = if tx.vsize > 0 {
        tx.fee as f64 / tx.vsize as f64
    } else {
        0.0
    };
    let json = format!(
        r#"{{"fee":{},"vsize":{},"feeRate":{:.1},"value":{}}}"#,
        tx.fee, tx.vsize, fee_rate, tx.value,
    );
    let local_data =
        web_sys::js_sys::JSON::parse(&json).unwrap_or(JsValue::NULL);
    let _ = js_show_tx_detail(txid, local_data);
}

#[cfg(not(feature = "hydrate"))]
fn show_tx_detail_with_data(_txid: &str, _tx: &NotableTxInfo) {}

/// Open the tx detail modal with just a txid (no local data).
#[allow(dead_code)]
fn show_tx_detail(_txid: &str) {
    #[cfg(feature = "hydrate")]
    {
        let _ = js_show_tx_detail(_txid, JsValue::NULL);
    }
}
