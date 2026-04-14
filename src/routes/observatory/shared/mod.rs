//! Shared state, components, and chart memo macro for the Observatory.
//!
//! This module re-exports from submodules and contains the remaining components
//! (OverlayPanel, ObservatoryNav, BlockDetailModal, ChartPageSkeleton, ChartPageLayout)
//! plus the `chart_memo!` macro.

// Submodules
mod drawer;
mod range;
mod state;
mod url_sync;

// Re-export everything so `use super::shared::*` continues to work
pub use drawer::*;
pub use range::*;
pub use state::*;
#[cfg(feature = "hydrate")]
pub use url_sync::build_share_url;

use leptos::prelude::*;
use leptos_router::hooks::use_location;

// ---------------------------------------------------------------------------
// Chart memo macro — pure derivation, no timing issues
// ---------------------------------------------------------------------------

/// Build a chart option as a derived Signal with parent-level caching.
/// On first compute, stores the result in ObservatoryState::chart_cache.
/// On subsequent evaluations (e.g. returning to a tab), returns the cached
/// value instantly. Cache is cleared by an Effect when range or overlays change.
#[macro_export]
macro_rules! chart_memo {
    ($data:expr, $range:expr, $overlays:expr, |$blocks:ident| $per_block:expr, |$days:ident| $daily:expr) => {{
        use $crate::routes::observatory::shared::DashboardData;
        // Generate a unique cache key from the macro call site
        let cache_key =
            concat!(file!(), ":", line!(), ":", column!()).to_string();
        let state = leptos::prelude::expect_context::<
            $crate::routes::observatory::shared::ObservatoryState,
        >();
        let cache = state.chart_cache.clone();
        leptos::prelude::Signal::derive(move || {
            let r = $range.get();
            let flags = $overlays.get();
            // MUST read data to track it as reactive dependency — otherwise
            // the derive won't re-run when new data arrives after range change.
            let data_opt = $data.get().and_then(|r| r.ok());
            // Include data fingerprint in cache key so stale data never matches
            let data_fp = data_opt
                .as_ref()
                .map(|d| match d {
                    DashboardData::PerBlock(ref b) => b.len(),
                    DashboardData::Daily(ref d) => d.len(),
                })
                .unwrap_or(0);

            // Two-level cache: base chart (expensive) keyed without overlays,
            // final result (with overlays) keyed with full overlay flags.
            let base_key = format!("{}:{}:{}", cache_key, r, data_fp);
            let full_key = format!("{}:{}", base_key, flags.cache_key());

            // Check full cache (base + overlays)
            if let Ok(c) = cache.lock() {
                if let Some((_, cached)) = c.get(&full_key) {
                    return cached.clone();
                }
            }

            // Try to reuse cached base chart (avoids recomputing on overlay toggle)
            let base_json = if let Ok(c) = cache.lock() {
                c.get(&base_key).map(|(_, v)| v.clone())
            } else {
                None
            };

            let (base, is_daily) = if let Some(ref cached_base) = base_json {
                // Determine is_daily from data without recomputing chart
                let daily = data_opt
                    .as_ref()
                    .map(|d| matches!(d, DashboardData::Daily(_)))
                    .unwrap_or(false);
                (cached_base.clone(), daily)
            } else {
                // Compute base chart from scratch
                let computed = data_opt
                    .map(|data| {
                        let (value, daily) = match data {
                            DashboardData::PerBlock(ref $blocks) => {
                                ($per_block, false)
                            }
                            DashboardData::Daily(ref $days) => ($daily, true),
                        };
                        if value.is_null() {
                            (String::new(), daily)
                        } else {
                            (serde_json::to_string(&value).unwrap_or_default(), daily)
                        }
                    })
                    .unwrap_or_default();
                // Cache the base chart
                if !computed.0.is_empty() {
                    if let Ok(mut c) = cache.lock() {
                        let seq = $crate::routes::observatory::shared::CHART_CACHE_SEQ
                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        c.insert(base_key, (seq, computed.0.clone()));
                    }
                }
                computed
            };

            if base.is_empty() {
                return base;
            }

            // Apply overlays on top of base chart
            let result = if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&base) {
                $crate::stats::charts::apply_overlays(&mut value, &flags, is_daily);
                serde_json::to_string(&value).unwrap_or_default()
            } else {
                base
            };

            if !result.is_empty() {
                if let Ok(mut c) = cache.lock() {
                    // LRU-style eviction: keep 150 newest, remove oldest 50+
                    if c.len() > 200 {
                        let mut by_seq: Vec<(String, u64)> = c
                            .iter()
                            .map(|(k, (seq, _))| (k.clone(), *seq))
                            .collect();
                        by_seq.sort_unstable_by_key(|(_, seq)| *seq);
                        let to_remove = by_seq.len().saturating_sub(150);
                        for (key, _) in by_seq.into_iter().take(to_remove) {
                            c.remove(&key);
                        }
                    }
                    let seq = $crate::routes::observatory::shared::CHART_CACHE_SEQ
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    c.insert(full_key, (seq, result.clone()));
                }
            }
            result
        })
    }};
}

// ---------------------------------------------------------------------------
// Reusable components (kept in shared.rs as they're small and tightly coupled)
// ---------------------------------------------------------------------------

/// Floating overlay panel (hidden on dashboard since there are no charts)
#[component]
pub fn OverlayPanel() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let location = use_location();
    let hide_overlays = Signal::derive(move || {
        let path = location.pathname.get();
        path == "/observatory"
            || path == "/observatory/stats"
            || path == "/observatory/on-this-day"
            || path == "/observatory/hall-of-fame"
            || path == "/observatory/heartbeat"
    });

    view! {
        <div style="z-index: 10000" class="fixed left-4 bottom-4" class:hidden=hide_overlays>
            <Show
                when=move || state.overlay_panel_open.get()
                fallback=move || view! {
                    <button
                        class="group bg-[#0d2137] border border-[#f7931a]/30 hover:border-[#f7931a]/60 text-[#f7931a]/70 hover:text-[#f7931a] rounded-2xl p-4 shadow-lg shadow-black/30 cursor-pointer transition-all hover:scale-105 animate-fadeinone"
                        title="Chart Overlays"
                        aria-label="Toggle chart overlays"
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

/// Observatory navigation bar with two-tier layout.
/// Row 1: Experience pages (Dashboard, Overview, On This Day, Heartbeat)
/// Row 2: Data explorer charts (Network, Fees, Mining, Embedded, Signaling)
#[component]
pub fn ObservatoryNav() -> impl IntoView {
    let pages: Vec<(&'static str, &'static str)> = vec![
        ("/observatory", "Dashboard"),
        ("/observatory/stats", "Overview"),
        ("/observatory/on-this-day", "On This Day"),
        ("/observatory/hall-of-fame", "Hall of Fame"),
        ("/observatory/heartbeat", "Heartbeat"),
    ];

    let charts: Vec<(&'static str, &'static str)> = vec![
        ("/observatory/charts/network", "Network"),
        ("/observatory/charts/fees", "Fees"),
        ("/observatory/charts/mining", "Mining"),
        ("/observatory/charts/embedded", "Embedded"),
        ("/observatory/signaling", "Signaling"),
    ];

    let location = use_location();

    // Check if any chart tab is active (to highlight the charts row)
    let on_charts = Signal::derive({
        let chart_prefixes: Vec<String> =
            charts.iter().map(|(h, _)| h.to_string()).collect();
        move || {
            let path = location.pathname.get();
            chart_prefixes.iter().any(|p| path.starts_with(p))
        }
    });

    view! {
        <nav class="flex flex-col items-center mb-6 sm:mb-8 px-1 gap-2 sm:gap-3">
            // Row 1: Pages
            <div class="flex sm:justify-center gap-x-4 sm:gap-x-8 overflow-x-auto scrollbar-hide w-full px-2">
                {pages.into_iter().map(|(href, label)| {
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
                                    "text-sm sm:text-base font-semibold text-[#f7931a] border-b-2 border-[#f7931a] pb-1 transition-all duration-200 whitespace-nowrap active:scale-95 active:opacity-70"
                                } else {
                                    "text-sm sm:text-base font-medium text-white/40 hover:text-white/70 border-b-2 border-transparent pb-1 transition-all duration-200 whitespace-nowrap active:scale-95 active:opacity-70"
                                }
                            }
                        >
                            {label}
                        </a>
                    }
                }).collect::<Vec<_>>()}
            </div>
            // Row 2: Chart explorer tabs (slightly smaller, subtle separator)
            <div class=move || {
                if on_charts.get() {
                    "flex items-center justify-center gap-x-3 sm:gap-x-6 px-3 sm:px-5 py-1.5 rounded-full bg-white/[0.03] border border-white/[0.06]"
                } else {
                    "flex items-center justify-center gap-x-3 sm:gap-x-6 px-3 sm:px-5 py-1.5 rounded-full"
                }
            }>
                <span class="text-[10px] sm:text-[11px] uppercase tracking-widest text-white/20 mr-1 sm:mr-2">"Charts"</span>
                {charts.into_iter().map(|(href, label)| {
                    let href_str = href.to_string();
                    view! {
                        <a
                            href=href
                            class=move || {
                                let path = location.pathname.get();
                                let active = path.starts_with(&href_str);
                                if active {
                                    "text-xs sm:text-[13px] font-semibold text-[#f7931a] border-b border-[#f7931a]/50 pb-0.5 transition-all duration-200 whitespace-nowrap active:scale-95 active:opacity-70"
                                } else {
                                    "text-xs sm:text-[13px] font-medium text-white/30 hover:text-white/60 border-b border-transparent pb-0.5 transition-all duration-200 whitespace-nowrap active:scale-95 active:opacity-70"
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
        <div id="block-detail-modal" class="hidden fixed inset-0 flex items-center justify-center p-4" style="z-index: 10001">
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

        // Transaction detail modal
        <div id="tx-detail-modal" class="hidden fixed inset-0 flex items-center justify-center p-4" style="z-index: 10001">
            <div class="absolute inset-0 bg-black/60" onclick="closeTxDetail()"></div>
            <div class="relative bg-[#0e2a47] border border-white/15 rounded-2xl shadow-2xl w-full max-w-md max-h-[80vh] overflow-y-auto overflow-x-hidden">
                <div class="flex items-center justify-between px-5 py-3 border-b border-white/10">
                    <span id="tx-detail-title" class="text-white font-medium">"Transaction"</span>
                    <button
                        class="text-white/40 hover:text-white text-lg cursor-pointer"
                        onclick="closeTxDetail()"
                    >"\u{2715}"</button>
                </div>
                <div id="tx-detail-body" class="px-5 py-4 text-sm text-white/80 space-y-1"
                    style="--bd-label: rgba(255,255,255,0.5)"
                ></div>
            </div>
        </div>
    }
}

/// Loading skeleton for chart pages (shown while dashboard_data is loading)
#[component]
pub fn ChartPageSkeleton(#[prop(default = 3)] count: usize) -> impl IntoView {
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
    #[prop(optional, into)] seo_text: Option<&'static str>,
    #[prop(optional, into)] header: Option<ViewFn>,
    children: Children,
) -> impl IntoView {
    view! {
        // Slim hero banner
        <div class="relative rounded-2xl overflow-hidden mb-5">
            <img
                src="/img/observatory_hero.png"
                alt=title
                class="w-full h-[100px] sm:h-[120px] lg:h-[140px] object-cover object-center"
            />
            <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/60 to-[#123c64]/30"></div>
            <div class="absolute inset-0 flex flex-col items-center justify-end pb-3 sm:pb-4">
                <h1 class="text-lg sm:text-xl lg:text-2xl font-title text-white mb-0.5 drop-shadow-lg">{title}</h1>
                <p class="text-[11px] sm:text-xs text-white/50 max-w-lg mx-auto px-4 text-center drop-shadow">{description}</p>
            </div>
        </div>
        // SEO: crawlable description for search engines (visually hidden, accessible)
        {seo_text.map(|text| view! {
            <p class="sr-only">{text}</p>
        })}
        // Compact toolbar: section selector (left) + range (right)
        <div class="flex flex-col sm:flex-row sm:items-start gap-3 mb-6">
            <div class="flex items-center gap-3 flex-shrink-0">
                {header.map(|h| view! { <>{h.run()}</> })}
                <a href="/observatory/learn/methodology"
                    class="text-xs text-white/30 hover:text-[#f7931a] transition-colors"
                >
                    "Methodology"
                </a>
            </div>
            <div class="sm:ml-auto">
                <RangeSelector/>
            </div>
        </div>
        {children()}
        <FloatingRangePicker/>
        <ChartDrawer/>
    }
}

