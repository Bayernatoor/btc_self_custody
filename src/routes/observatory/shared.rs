//! Shared state, URL query params, and reusable components for the Observatory.

use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};

use super::helpers::*;
use crate::stats::charts::OverlayFlags;
use crate::stats::server_fns::*;
use crate::stats::types::*;

// ---------------------------------------------------------------------------
// Data enum for dashboard
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub enum DashboardData {
    PerBlock(Vec<BlockSummary>),
    Daily(Vec<DailyAggregate>),
}

// ---------------------------------------------------------------------------
// Shared observatory state (provided via context)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ObservatoryState {
    pub range: ReadSignal<String>,
    pub set_range: WriteSignal<String>,
    pub overlay_flags: Signal<OverlayFlags>,
    pub cached_live: ReadSignal<Option<LiveStats>>,
    // overlay signals (for the panel)
    pub overlay_halvings: ReadSignal<bool>,
    pub set_overlay_halvings: WriteSignal<bool>,
    pub overlay_bips: ReadSignal<bool>,
    pub set_overlay_bips: WriteSignal<bool>,
    pub overlay_core: ReadSignal<bool>,
    pub set_overlay_core: WriteSignal<bool>,
    pub overlay_price: ReadSignal<bool>,
    pub set_overlay_price: WriteSignal<bool>,
    pub overlay_chain_size: ReadSignal<bool>,
    pub set_overlay_chain_size: WriteSignal<bool>,
    pub overlay_events: ReadSignal<bool>,
    pub set_overlay_events: WriteSignal<bool>,
    pub price_loading: Signal<bool>,
    // overlay panel open state
    pub overlay_panel_open: ReadSignal<bool>,
    pub set_overlay_panel_open: WriteSignal<bool>,
}

/// Create a dashboard data resource. Each chart page calls this to get its own
/// resource that fires on mount (avoids stale Effect issues with Outlet navigation).
pub fn create_dashboard_resource(
    range: ReadSignal<String>,
) -> LocalResource<Result<DashboardData, String>> {
    LocalResource::new(move || {
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
    })
}

/// Initialize shared observatory state from URL query params and provide via context.
/// Returns the state struct.
pub fn provide_observatory_state() -> ObservatoryState {
    let query = use_query_map();

    // Read initial values from URL
    let initial_range = query.read_untracked()
        .get("range")
        .filter(|r| !r.is_empty())
        .unwrap_or_else(|| "all".to_string());

    let initial_overlays: Vec<String> = query.read_untracked()
        .get("overlays")
        .filter(|s| !s.is_empty())
        .map(|s| s.split(',').map(|s| s.to_string()).collect())
        .unwrap_or_default();

    let (range, set_range) = signal(initial_range);

    // Overlay toggles — initialized from URL
    let (overlay_halvings, set_overlay_halvings) = signal(initial_overlays.iter().any(|s| s == "halvings"));
    let (overlay_bips, set_overlay_bips) = signal(initial_overlays.iter().any(|s| s == "bips"));
    let (overlay_core, set_overlay_core) = signal(initial_overlays.iter().any(|s| s == "core"));
    let (overlay_price, set_overlay_price) = signal(initial_overlays.iter().any(|s| s == "price"));
    let (overlay_chain_size, set_overlay_chain_size) = signal(initial_overlays.iter().any(|s| s == "chain_size"));
    let (overlay_events, set_overlay_events) = signal(initial_overlays.iter().any(|s| s == "events"));
    let (overlay_panel_open, set_overlay_panel_open) = signal(false);

    // Sync signals back to URL (replace, don't push history)
    // Sync range/overlay state to URL query params (without triggering navigation loops)
    let navigate = use_navigate();
    let last_query = std::cell::RefCell::new(String::new());
    Effect::new(move |_| {
        let r = range.get();
        let mut overlays = Vec::new();
        if overlay_halvings.get() { overlays.push("halvings"); }
        if overlay_bips.get() { overlays.push("bips"); }
        if overlay_core.get() { overlays.push("core"); }
        if overlay_events.get() { overlays.push("events"); }
        if overlay_price.get() { overlays.push("price"); }
        if overlay_chain_size.get() { overlays.push("chain_size"); }

        let mut params = vec![];
        if r != "all" { params.push(format!("range={r}")); }
        if !overlays.is_empty() { params.push(format!("overlays={}", overlays.join(","))); }
        let query = params.join("&");

        // Only navigate if query params actually changed
        if *last_query.borrow() == query { return; }
        *last_query.borrow_mut() = query.clone();

        let path = leptos_router::hooks::use_location().pathname.get_untracked();
        let url = if query.is_empty() { path } else { format!("{path}?{query}") };
        navigate(&url, leptos_router::NavigateOptions {
            replace: true,
            ..Default::default()
        });
    });

    // Price history: fetch once when enabled, cache so toggling overlay is instant
    let price_history_resource = LocalResource::new(move || {
        let enabled = overlay_price.get();
        async move {
            if !enabled {
                return Vec::new();
            }
            let mut data: Vec<(u64, f64)> = match fetch_price_history(0, 4_000_000_000).await {
                Ok(pts) => pts.into_iter().map(|p| (p.timestamp_ms, p.price_usd)).collect(),
                Err(e) => {
                    leptos::logging::warn!("Price history fetch failed: {e}");
                    Vec::new()
                }
            };
            if let Ok(live_stats) = fetch_live_stats().await {
                let now_ms = chrono::Utc::now().timestamp() as u64 * 1000;
                if live_stats.network.price_usd > 0.0 {
                    let last_ts = data.last().map(|&(ts, _)| ts).unwrap_or(0);
                    if now_ms > last_ts {
                        data.push((now_ms, live_stats.network.price_usd));
                    }
                }
            }
            data
        }
    });

    let (cached_price_history, set_cached_price_history) = signal::<Vec<(u64, f64)>>(Vec::new());
    let price_loading = Signal::derive(move || {
        overlay_price.get() && cached_price_history.get().is_empty()
    });
    Effect::new(move |_| {
        if let Some(data) = price_history_resource.get() {
            if !data.is_empty() {
                set_cached_price_history.set(data);
            }
        }
    });

    // Live stats (auto-refresh 30s)
    #[allow(clippy::redundant_closure)]
    let live = LocalResource::new(move || fetch_live_stats());

    let (countdown, set_countdown) = signal(30u32);
    let (_last_updated, set_last_updated) = signal("connecting...".to_string());

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

    let (cached_live, set_cached_live) = signal::<Option<LiveStats>>(None);
    let (connected, set_connected) = signal(false);
    Effect::new(move |_| {
        match live.get() {
            Some(Ok(stats)) => {
                set_cached_live.set(Some(stats));
                set_connected.set(true);
                set_last_updated.set(format!(
                    "updated {}",
                    chrono::Local::now().format("%H:%M:%S")
                ));
            }
            Some(Err(_)) => {
                set_connected.set(false);
            }
            None => {} // still loading
        }
    });

    // Store live-related signals in context for overview page
    provide_context(LiveContext {
        live,
        countdown,
        set_countdown,
        last_updated: _last_updated,
        set_last_updated,
        connected,
    });

    // Note: dashboard_data is NOT shared via context. Each chart page creates
    // its own LocalResource via `create_dashboard_resource()`. This ensures
    // the resource fires on mount even during client-side Outlet navigation.
    // Server-side caching (120s TTL) makes duplicate fetches essentially free.
    let dashboard_data = create_dashboard_resource(range);

    // Pre-compute chain size cumulative data
    let cached_chain_size_data = {
        let (cached, set_cached) = signal::<Vec<(u64, f64)>>(Vec::new());
        Effect::new(move |_| {
            let result = dashboard_data
                .get()
                .and_then(|r| r.ok())
                .map(|data| {
                    let mut cumulative: f64 = 0.0;
                    match data {
                        DashboardData::PerBlock(ref blocks) => blocks
                            .iter()
                            .map(|b| {
                                cumulative += b.size as f64 / 1_000_000_000.0;
                                (b.timestamp * 1000, (cumulative * 1000.0).round() / 1000.0)
                            })
                            .collect(),
                        DashboardData::Daily(ref days) => days
                            .iter()
                            .filter_map(|d| {
                                cumulative += d.avg_size * d.block_count as f64 / 1_000_000_000.0;
                                let ts = chrono::NaiveDate::parse_from_str(&d.date, "%Y-%m-%d")
                                    .map(|dt| {
                                        dt.and_hms_opt(12, 0, 0)
                                            .unwrap()
                                            .and_utc()
                                            .timestamp() as u64
                                            * 1000
                                    })
                                    .ok()?;
                                Some((ts, (cumulative * 1000.0).round() / 1000.0))
                            })
                            .collect(),
                    }
                })
                .unwrap_or_default();
            set_cached.set(result);
        });
        Signal::derive(move || cached.get())
    };

    let overlay_flags = Signal::derive(move || {
        let price_data = if overlay_price.get() {
            cached_price_history.get()
        } else {
            Vec::new()
        };

        let chain_size_data = if overlay_chain_size.get() {
            cached_chain_size_data.get()
        } else {
            Vec::new()
        };

        OverlayFlags {
            halvings: overlay_halvings.get(),
            bip_activations: overlay_bips.get(),
            core_releases: overlay_core.get(),
            events: overlay_events.get(),
            price_data,
            chain_size_data,
        }
    });

    let state = ObservatoryState {
        range,
        set_range,
        overlay_flags,
        cached_live,
        overlay_halvings,
        set_overlay_halvings,
        overlay_bips,
        set_overlay_bips,
        overlay_core,
        set_overlay_core,
        overlay_price,
        set_overlay_price,
        overlay_chain_size,
        set_overlay_chain_size,
        overlay_events,
        set_overlay_events,
        price_loading,
        overlay_panel_open,
        set_overlay_panel_open,
    };

    provide_context(state.clone());
    state
}

/// Live stats context (for overview page auto-refresh UI)
#[derive(Clone)]
pub struct LiveContext {
    pub live: LocalResource<Result<LiveStats, ServerFnError>>,
    pub countdown: ReadSignal<u32>,
    pub set_countdown: WriteSignal<u32>,
    pub last_updated: ReadSignal<String>,
    pub set_last_updated: WriteSignal<String>,
    pub connected: ReadSignal<bool>,
}

// ---------------------------------------------------------------------------
// Chart signal macro (no tab guards needed — each page only builds its own)
// ---------------------------------------------------------------------------

/// Build a chart option signal with two-stage pipeline: base chart cached
/// separately from overlay application. No tab guards needed since each
/// page only creates signals for its own charts.
#[macro_export]
macro_rules! chart_signal {
    ($data:expr, $range:expr, $overlays:expr, |$blocks:ident| $per_block:expr, |$days:ident| $daily:expr) => {{
        use $crate::routes::observatory::shared::DashboardData;
        let (base_json, set_base_json) = leptos::prelude::signal((String::new(), false));
        leptos::prelude::Effect::new(move |_| {
            let _r = $range.get();
            let result = $data
                .get()
                .and_then(|r| r.ok())
                .map(|data| match data {
                    DashboardData::PerBlock(ref $blocks) => ($per_block, false),
                    DashboardData::Daily(ref $days) => ($daily, true),
                });
            if let Some(r) = result {
                set_base_json.set(r);
            }
        });
        let (cached, set_cached) = leptos::prelude::signal(String::new());
        leptos::prelude::Effect::new(move |_| {
            let (ref json, is_daily) = *base_json.read();
            let flags = $overlays.read();
            if json.is_empty() { return; }
            set_cached.set(crate::stats::charts::apply_overlays(json, &*flags, is_daily));
        });
        leptos::prelude::Signal::derive(move || cached.get())
    }};
}

// ---------------------------------------------------------------------------
// Reusable components
// ---------------------------------------------------------------------------

/// Range selector bar (1D through ALL + YTD)
#[component]
pub fn RangeSelector() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let set_range = state.set_range;

    view! {
        <div class="flex justify-end items-center mb-4">
            <div class="grid grid-cols-5 sm:flex gap-1.5 bg-[#0a1a2e] rounded-xl p-1.5 border border-white/5">
                {["1d", "1w", "1m", "3m", "6m", "ytd", "1y", "2y", "5y", "10y", "all"].into_iter().map(|r| {
                    let r_str = r.to_string();
                    let r_display = r.to_uppercase();
                    let r_clone = r_str.clone();
                    view! {
                        <button
                            class=move || {
                                if range.get() == r_clone {
                                    "px-3 py-1 text-xs rounded-lg bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
                                } else {
                                    "px-3 py-1 text-xs rounded-lg text-white/40 hover:text-white/70 hover:bg-white/5 transition-all cursor-pointer"
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
            <span class="ml-3 text-xs text-white/60 self-center">
                {move || {
                    let n = range_to_blocks(&range.get());
                    if n > 5_000 { "daily averages" } else { "per block" }
                }}
            </span>
        </div>
    }
}

/// Floating overlay panel (hidden on dashboard since there are no charts)
#[component]
pub fn OverlayPanel() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let location = leptos_router::hooks::use_location();
    let on_dashboard = Signal::derive(move || location.pathname.get() == "/observatory");

    view! {
        <div style="z-index: 10000" class="fixed left-4 bottom-4" class:hidden=on_dashboard>
            <Show
                when=move || state.overlay_panel_open.get()
                fallback=move || view! {
                    <button
                        class="bg-[#0d2137] border border-white/10 hover:border-[#f7931a]/50 text-white/60 hover:text-white rounded-xl p-3.5 shadow-lg cursor-pointer transition-all"
                        title="Chart Overlays"
                        on:click=move |_| state.set_overlay_panel_open.set(true)
                    >
                        <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M6 13.5V3.75m0 9.75a1.5 1.5 0 0 1 0 3m0-3a1.5 1.5 0 0 0 0 3m0 3.75V16.5m12-3V3.75m0 9.75a1.5 1.5 0 0 1 0 3m0-3a1.5 1.5 0 0 0 0 3m0 3.75V16.5m-6-9V3.75m0 3.75a1.5 1.5 0 0 1 0 3m0-3a1.5 1.5 0 0 0 0 3m0 9.75V10.5"/>
                        </svg>
                    </button>
                }
            >
                <div class="bg-[#0d2137] border border-white/10 rounded-xl p-5 shadow-xl min-w-[210px]">
                    <div class="flex items-center justify-between mb-3">
                        <span class="text-sm text-white/50 uppercase tracking-widest font-semibold">"Overlays"</span>
                        <button
                            class="text-white/30 hover:text-white/60 cursor-pointer p-0.5"
                            on:click=move |_| state.set_overlay_panel_open.set(false)
                        >
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>
                            </svg>
                        </button>
                    </div>
                    <div class="space-y-2.5">
                        <OverlayCheckbox label="Halvings" color="#f7931a" icon="- -" checked=state.overlay_halvings on_toggle=state.set_overlay_halvings/>
                        <OverlayCheckbox label="BIP Activations" color="#4ecdc4" icon="\u{2026}" checked=state.overlay_bips on_toggle=state.set_overlay_bips/>
                        <OverlayCheckbox label="Core Releases" color="#a855f7" icon="\u{2026}" checked=state.overlay_core on_toggle=state.set_overlay_core/>
                        <OverlayCheckbox label="Events" color="#ef4444" icon="\u{2605}" checked=state.overlay_events on_toggle=state.set_overlay_events/>
                        <label class="flex items-center gap-2 cursor-pointer group">
                            <input
                                type="checkbox"
                                class="accent-[#e6c84e] w-4 h-4 cursor-pointer"
                                prop:checked=move || state.overlay_price.get()
                                on:change=move |_| state.set_overlay_price.update(|v| *v = !*v)
                            />
                            <span class="text-[0.9rem] text-white/60 group-hover:text-white/80 transition-colors">"Price (USD)"</span>
                            {move || if state.price_loading.get() {
                                view! {
                                    <svg class="w-4 h-4 ml-auto animate-spin text-[#e6c84e]/60" fill="none" viewBox="0 0 24 24">
                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                    </svg>
                                }.into_any()
                            } else {
                                view! { <span class="text-sm text-[#e6c84e]/60 ml-auto">"$"</span> }.into_any()
                            }}
                        </label>
                        <OverlayCheckbox label="Chain Size" color="#10b981" icon="GB" checked=state.overlay_chain_size on_toggle=state.set_overlay_chain_size/>
                    </div>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn OverlayCheckbox(
    #[prop(into)] label: &'static str,
    #[prop(into)] color: &'static str,
    #[prop(into)] icon: &'static str,
    checked: ReadSignal<bool>,
    on_toggle: WriteSignal<bool>,
) -> impl IntoView {
    view! {
        <label class="flex items-center gap-2 cursor-pointer group">
            <input
                type="checkbox"
                class="w-4 h-4 cursor-pointer"
                style=format!("accent-color: {color}")
                prop:checked=move || checked.get()
                on:change=move |_| on_toggle.update(|v| *v = !*v)
            />
            <span class="text-[0.9rem] text-white/60 group-hover:text-white/80 transition-colors">{label}</span>
            <span class="text-sm ml-auto" style=format!("color: {}60", color)>{icon}</span>
        </label>
    }
}

/// Observatory navigation bar with links to sub-pages
#[component]
pub fn ObservatoryNav() -> impl IntoView {
    let tabs = vec![
        ("/observatory", "Dashboard"),
        ("/observatory/charts/network", "Network"),
        ("/observatory/charts/fees", "Fees"),
        ("/observatory/charts/mining", "Mining"),
        ("/observatory/charts/embedded", "Embedded Data"),
        ("/observatory/signaling", "Signaling"),
    ];

    // Get current path to highlight active tab
    let location = leptos_router::hooks::use_location();

    view! {
        <nav class="flex justify-center mb-6 sm:mb-8 px-1">
            <div class="flex flex-wrap justify-center gap-1 sm:gap-0 sm:inline-flex bg-[#0a1a2e] rounded-2xl p-1 sm:p-1.5 border border-white/10">
                {tabs.into_iter().map(|(href, label)| {
                    let href_str = href.to_string();
                    view! {
                        <a
                            href=href
                            class=move || {
                                let path = location.pathname.get();
                                let active = if href_str == "/observatory" {
                                    path == "/observatory"
                                } else {
                                    path.starts_with(&href_str)
                                };
                                if active {
                                    "px-3 py-1.5 text-xs sm:px-5 sm:py-2 sm:text-sm font-semibold rounded-xl bg-[#f7931a] text-[#0a1a2e] shadow-md shadow-[#f7931a]/20 transition-all duration-200 whitespace-nowrap"
                                } else {
                                    "px-3 py-1.5 text-xs sm:px-5 sm:py-2 sm:text-sm font-medium rounded-xl text-white/50 hover:text-white/80 hover:bg-white/5 transition-all duration-200 whitespace-nowrap"
                                }
                            }
                        >
                            {label}
                        </a>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </nav>
    }
}

/// Block detail modal (shared across all chart pages)
#[component]
pub fn BlockDetailModal() -> impl IntoView {
    view! {
        <div id="block-detail-modal" class="hidden fixed inset-0 z-50 flex items-center justify-center p-4">
            <div class="absolute inset-0 bg-black/60" onclick="closeBlockDetail()"></div>
            <div class="relative bg-[#0e2a47] border border-white/15 rounded-2xl shadow-2xl w-full max-w-md max-h-[80vh] overflow-y-auto overflow-x-hidden">
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

        <style>"
            .bd-row { display: flex; justify-content: space-between; padding: 4px 0; }
            .bd-row span:first-child { color: rgba(255,255,255,0.5); }
            .bd-row span:last-child { color: rgba(255,255,255,0.85); font-family: monospace; font-size: 12px; text-align: right; word-break: break-all; max-width: 60%; }
            .bd-hash { color: #f7931a !important; word-break: break-all; }
            .bd-divider { border-top: 1px solid rgba(255,255,255,0.08); margin: 8px 0; }
        "</style>
    }
}

/// Page wrapper for chart sub-pages.
/// `header` slot renders above the range selector (sub-section pills, links).
/// `children` renders below it (the actual charts).
#[component]
pub fn ChartPageLayout(
    #[prop(into)] title: &'static str,
    #[prop(into)] description: &'static str,
    #[prop(optional, into)] header: Option<ViewFn>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="text-center mb-6">
            <h2 class="text-xl sm:text-2xl font-title text-white mb-1">{title}</h2>
            <p class="text-sm text-white/40 max-w-lg mx-auto">{description}</p>
        </div>
        {header.map(|h| h.run())}
        <RangeSelector/>
        {children()}
    }
}
