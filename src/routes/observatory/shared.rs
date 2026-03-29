//! Shared state, URL query params, and reusable components for the Observatory.

use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

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
    pub dashboard_data: LocalResource<Result<DashboardData, String>>,
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

    // URL query params are read on mount (above) but not synced back.
    // The navigate() call was causing race conditions with Outlet transitions,
    // interfering with child route mounting and Effect scheduling.

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
            // Pause polling when tab is hidden (saves bandwidth)
            #[cfg(feature = "hydrate")]
            {
                let hidden = leptos::prelude::document().hidden();
                if hidden { return; }
            }
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

    // Shared dashboard data resource — lives in the parent (ObservatoryPage),
    // stays alive across Outlet navigations. Child pages read it from context
    // so there's no re-fetch or loading flash when switching pages.
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
        dashboard_data,
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
// Chart memo macro — pure derivation, no timing issues
// ---------------------------------------------------------------------------

/// Build a chart option as a derived Signal. The closure runs reactively
/// whenever dashboard_data, range, or overlay signals change, and returns
/// the current JSON string. Unlike Effect, this is a pure derivation that
/// doesn't depend on DOM timing.
#[macro_export]
macro_rules! chart_memo {
    ($data:expr, $range:expr, $overlays:expr, |$blocks:ident| $per_block:expr, |$days:ident| $daily:expr) => {{
        use $crate::routes::observatory::shared::DashboardData;
        leptos::prelude::Signal::derive(move || {
            let _r = $range.get();
            let flags = $overlays.get();
            $data
                .get()
                .and_then(|r| r.ok())
                .map(|data| {
                    let (json, is_daily) = match data {
                        DashboardData::PerBlock(ref $blocks) => ($per_block, false),
                        DashboardData::Daily(ref $days) => ($daily, true),
                    };
                    if json.is_empty() { return String::new(); }
                    crate::stats::charts::apply_overlays(&json, &flags, is_daily)
                })
                .unwrap_or_default()
        })
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

    let range_label = move || {
        let n = range_to_blocks(&range.get());
        if n > 5_000 { "daily averages" } else { "per block" }
    };

    view! {
        // Mobile: dropdown + label
        <div class="flex sm:hidden items-center gap-2">
            <div class="relative inline-block">
                <select
                    class="appearance-none bg-[#0a1a2e] text-white/80 text-sm border border-white/10 rounded-xl pl-3 pr-8 py-2 cursor-pointer focus:outline-none focus:border-[#f7931a]/40 transition-colors"
                    prop:value=move || range.get()
                    on:change=move |ev| {
                        use wasm_bindgen::JsCast;
                        if let Some(t) = ev.target() {
                            if let Ok(s) = t.dyn_into::<leptos::web_sys::HtmlSelectElement>() {
                                set_range.set(s.value());
                            }
                        }
                    }
                >
                    {["1d", "1w", "1m", "3m", "6m", "ytd", "1y", "2y", "5y", "10y", "all"].into_iter().map(|r| {
                        let val = r.to_string();
                        let label = r.to_uppercase();
                        view! { <option value=val>{label}</option> }
                    }).collect::<Vec<_>>()}
                </select>
                <svg class="absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none w-3.5 h-3.5 text-white/40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                </svg>
            </div>
            <span class="text-xs text-white/40">{range_label}</span>
        </div>
        // Desktop: button grid + label
        <div class="hidden sm:flex items-center">
            <div class="flex gap-1.5 bg-[#0a1a2e] rounded-xl p-1.5 border border-white/5">
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
            <span class="ml-3 text-xs text-white/60 self-center">{range_label}</span>
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
                        class="group bg-[#0d2137] border border-[#f7931a]/30 hover:border-[#f7931a]/60 text-[#f7931a]/70 hover:text-[#f7931a] rounded-2xl p-4 shadow-lg shadow-black/30 cursor-pointer transition-all hover:scale-105 animate-fadeinone"
                        title="Chart Overlays"
                        on:click=move |_| state.set_overlay_panel_open.set(true)
                    >
                        <svg class="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M6 13.5V3.75m0 9.75a1.5 1.5 0 0 1 0 3m0-3a1.5 1.5 0 0 0 0 3m0 3.75V16.5m12-3V3.75m0 9.75a1.5 1.5 0 0 1 0 3m0-3a1.5 1.5 0 0 0 0 3m0 3.75V16.5m-6-9V3.75m0 3.75a1.5 1.5 0 0 1 0 3m0-3a1.5 1.5 0 0 0 0 3m0 9.75V10.5"/>
                        </svg>
                        <span class="absolute left-full ml-2 top-1/2 -translate-y-1/2 bg-[#0d2137] border border-white/10 text-white/60 text-xs px-2.5 py-1 rounded-lg whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none">"Overlays"</span>
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

    let location = leptos_router::hooks::use_location();

    view! {
        <nav class="flex justify-center mb-6 sm:mb-8 px-1">
            <div class="flex flex-wrap justify-center gap-4 sm:gap-6">
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
                                    "text-sm sm:text-[15px] font-semibold text-[#f7931a] border-b-2 border-[#f7931a] pb-1 transition-all duration-200 whitespace-nowrap"
                                } else {
                                    "text-sm sm:text-[15px] font-medium text-white/40 hover:text-white/70 border-b-2 border-transparent pb-1 transition-all duration-200 whitespace-nowrap"
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

/// Loading skeleton for chart pages (shown while dashboard_data is loading)
#[component]
pub fn ChartPageSkeleton(
    #[prop(default = 3)] count: usize,
) -> impl IntoView {
    view! {
        <div class="space-y-10">
            {(0..count).map(|_| view! {
                <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6">
                    <div class="h-4 w-48 bg-white/5 rounded mb-2"></div>
                    <div class="h-3 w-72 bg-white/5 rounded mb-4"></div>
                    <div class="h-[350px] lg:h-[600px] flex items-center justify-center">
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
            }).collect::<Vec<_>>()}
        </div>
    }
}

/// Page wrapper for chart sub-pages.
/// Renders a slim hero banner with title/description, then a compact toolbar
/// with optional header content (section dropdown) on the left and the range
/// selector on the right.
#[component]
pub fn ChartPageLayout(
    #[prop(into)] title: &'static str,
    #[prop(into)] description: &'static str,
    #[prop(optional, into)] header: Option<ViewFn>,
    children: Children,
) -> impl IntoView {
    view! {
        // Slim hero banner
        <div class="relative rounded-2xl overflow-hidden mb-5">
            <img
                src="/observatory_hero.png"
                alt=title
                class="w-full h-[100px] sm:h-[120px] lg:h-[140px] object-cover object-center"
            />
            <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/60 to-[#123c64]/30"></div>
            <div class="absolute inset-0 flex flex-col items-center justify-end pb-3 sm:pb-4">
                <h2 class="text-lg sm:text-xl lg:text-2xl font-title text-white mb-0.5 drop-shadow-lg">{title}</h2>
                <p class="text-[11px] sm:text-xs text-white/50 max-w-lg mx-auto px-4 text-center drop-shadow">{description}</p>
            </div>
        </div>
        // Compact toolbar: section selector (left) + range (right)
        <div class="flex flex-col sm:flex-row sm:items-start gap-3 mb-6">
            {header.map(|h| view! { <div class="flex items-center gap-3 flex-shrink-0">{h.run()}</div> })}
            <div class="sm:ml-auto">
                <RangeSelector/>
            </div>
        </div>
        {children()}
    }
}
