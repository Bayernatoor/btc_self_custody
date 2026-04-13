//! Shared state, URL query params, and reusable components for the Observatory.
//!
//! Central module that wires together the observatory's reactive state:
//! - `ObservatoryState` holds the time range, overlay toggles, dashboard data
//!   resource, live stats cache, and chart JSON cache. Provided as context by
//!   the parent `ObservatoryPage` and consumed by all child chart pages.
//! - `LiveContext` carries the auto-refreshing live stats resource and countdown
//!   timer, used by the overview dashboard.
//! - `chart_memo!` macro builds a derived `Signal<String>` that computes ECharts
//!   JSON from dashboard data, applies overlay mark lines, and caches the result
//!   so tab switches are instant.
//! - URL sync helpers keep the browser address bar in sync with range, overlays,
//!   and section state via `history.replaceState` (no router navigation).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_query_map};

use super::helpers::*;
use crate::stats::charts::OverlayFlags;
use crate::stats::server_fns::*;
use crate::stats::types::*;

// ---------------------------------------------------------------------------
// URL sync helpers (client-only)
// ---------------------------------------------------------------------------

/// Extract a query param value from a raw search string (e.g. "?range=3m&overlays=halvings").
#[cfg(feature = "hydrate")]
fn get_query_param(search: &str, key: &str) -> Option<String> {
    let qs = search.strip_prefix('?').unwrap_or(search);
    qs.split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
}

/// Build a query string from key-value pairs, omitting empty values.
#[cfg(feature = "hydrate")]
fn build_query_string(params: &[(&str, Option<String>)]) -> String {
    let parts: Vec<String> = params
        .iter()
        .filter_map(|(k, v)| v.as_ref().map(|val| format!("{k}={val}")))
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

/// Update the browser URL bar to reflect current Observatory state
/// without triggering a Leptos router navigation (uses history.replaceState).
#[cfg(feature = "hydrate")]
fn sync_url_to_state(
    pathname: &str,
    range: &str,
    overlays: &[(&str, bool)],
    section: Option<&str>,
    custom_from: Option<&str>,
    custom_to: Option<&str>,
) {
    let range_param = if range != "1y" {
        Some(range.to_string())
    } else {
        None
    };
    let active: Vec<&str> = overlays
        .iter()
        .filter(|(_, on)| *on)
        .map(|(name, _)| *name)
        .collect();
    let overlays_param = if active.is_empty() {
        None
    } else {
        Some(active.join(","))
    };
    let section_param = section.map(|s| s.to_string());
    let from_param = custom_from.map(|s| s.to_string());
    let to_param = custom_to.map(|s| s.to_string());

    let qs = build_query_string(&[
        ("range", range_param),
        ("overlays", overlays_param),
        ("section", section_param),
        ("from", from_param),
        ("to", to_param),
    ]);
    let hash = leptos::prelude::window()
        .location()
        .hash()
        .unwrap_or_default();
    let url = format!("{pathname}{qs}{hash}");
    let _ = leptos::prelude::window()
        .history()
        .expect("history")
        .replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&url));
}

/// Update just the `section` query param in the current URL.
/// Pass `None` to remove it (default section).
#[cfg(feature = "hydrate")]
pub fn update_section_in_url(section: Option<&str>) {
    let window = leptos::prelude::window();
    let pathname = window.location().pathname().unwrap_or_default();
    let search = window.location().search().unwrap_or_default();
    let hash = window.location().hash().unwrap_or_default();
    let qs = search.strip_prefix('?').unwrap_or(&search);

    // Rebuild params, replacing section
    let mut parts: Vec<String> = qs
        .split('&')
        .filter(|p| !p.is_empty() && !p.starts_with("section="))
        .map(|p| p.to_string())
        .collect();
    if let Some(s) = section {
        parts.push(format!("section={s}"));
    }
    let new_qs = if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    };
    let url = format!("{pathname}{new_qs}{hash}");
    let _ = window.history().expect("history").replace_state_with_url(
        &wasm_bindgen::JsValue::NULL,
        "",
        Some(&url),
    );
}

/// Build the full shareable URL for a specific chart, including current state.
#[cfg(feature = "hydrate")]
pub fn build_share_url(chart_id: &str) -> String {
    let window = leptos::prelude::window();
    let origin = window.location().origin().unwrap_or_default();
    let pathname = window.location().pathname().unwrap_or_default();
    let search = window.location().search().unwrap_or_default();
    format!("{origin}{pathname}{search}#{chart_id}")
}

/// Client-side chart JSON cache keyed by a composite string of chart ID, range,
/// data fingerprint, and overlay flags. Persists at the parent level across
/// Outlet navigations so switching tabs does not recompute charts.
/// Uses Arc<Mutex> for Send+Sync compatibility with SSR.
pub type ChartCache = Arc<Mutex<HashMap<String, String>>>;

// ---------------------------------------------------------------------------
// Data enum for dashboard
// ---------------------------------------------------------------------------

/// Dashboard data payload, either per-block (short ranges, under ~5000 blocks)
/// or daily aggregates (longer ranges like 1Y+). Chart builders accept both.
#[derive(Clone)]
pub enum DashboardData {
    PerBlock(Vec<BlockSummary>),
    Daily(Vec<DailyAggregate>),
}

// ---------------------------------------------------------------------------
// Shared observatory state (provided via context)
// ---------------------------------------------------------------------------

/// Shared reactive state for the entire observatory, provided via Leptos context.
/// Holds the selected time range, overlay toggle signals, the dashboard data resource,
/// cached live node stats, and a chart JSON cache that persists across tab navigations.
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
    // chart JSON cache — persists across Outlet navigations
    pub chart_cache: ChartCache,
    // true briefly when range changes (before new data arrives)
    pub data_loading: ReadSignal<bool>,
    // Custom date range (set when range == "custom")
    pub custom_from: ReadSignal<Option<String>>,
    pub set_custom_from: WriteSignal<Option<String>>,
    pub custom_to: ReadSignal<Option<String>>,
    pub set_custom_to: WriteSignal<Option<String>>,
}

/// Parse a "YYYY-MM-DD" date string to a Unix timestamp (midnight UTC).
pub fn date_to_ts(date: &str) -> Option<u64> {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp() as u64)
}

/// Create a `LocalResource` that fetches dashboard data for the given range.
/// Returns `PerBlock` data for short ranges (under ~5000 blocks) or `Daily`
/// aggregates for longer ranges. Supports custom date ranges via from/to params.
pub fn create_dashboard_resource(
    range: ReadSignal<String>,
    custom_from: ReadSignal<Option<String>>,
    custom_to: ReadSignal<Option<String>>,
) -> LocalResource<Result<DashboardData, String>> {
    LocalResource::new(move || {
        let r = range.get();
        let cf = custom_from.get();
        let ct = custom_to.get();
        async move {
            let stats =
                fetch_stats_summary().await.map_err(|e| e.to_string())?;

            // Custom date range — use timestamp-based queries directly
            if r == "custom" {
                if let (Some(from_str), Some(to_str)) = (cf, ct) {
                    let from_ts = date_to_ts(&from_str).unwrap_or(0);
                    let to_ts = date_to_ts(&to_str)
                        .map(|t| t + 86_400) // include entire end day (midnight next day)
                        .unwrap_or(stats.latest_timestamp);
                    let approx_blocks = to_ts.saturating_sub(from_ts) / 600;
                    if approx_blocks > 5_000 {
                        let days = fetch_daily_aggregates(from_ts, to_ts)
                            .await
                            .map_err(|e| e.to_string())?;
                        return Ok::<_, String>(DashboardData::Daily(days));
                    } else {
                        let blocks = fetch_blocks_by_ts(from_ts, to_ts)
                            .await
                            .map_err(|e| e.to_string())?;
                        return Ok(DashboardData::PerBlock(blocks));
                    }
                }
            }

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
    let initial_range = query
        .read_untracked()
        .get("range")
        .filter(|r| !r.is_empty())
        .unwrap_or_else(|| "1y".to_string());

    let initial_overlays: Vec<String> = query
        .read_untracked()
        .get("overlays")
        .filter(|s| !s.is_empty())
        .map(|s| s.split(',').map(|s| s.to_string()).collect())
        .unwrap_or_default();

    let initial_custom_from =
        query.read_untracked().get("from").filter(|s| !s.is_empty());
    let initial_custom_to =
        query.read_untracked().get("to").filter(|s| !s.is_empty());

    let (range, set_range) = signal(initial_range);
    let (custom_from, set_custom_from) = signal(initial_custom_from);
    let (custom_to, set_custom_to) = signal(initial_custom_to);

    // Loading signal: true when range changes, false when data arrives
    let (data_loading, set_data_loading) = signal(false);
    {
        // Set loading=true for heavy ranges (2Y+) so the user sees
        // immediate feedback. Lighter ranges compute fast enough.
        let mut first = true;
        Effect::new(move |_| {
            let r = range.get();
            if first {
                first = false;
            } else {
                let n = range_to_blocks(&r);
                if n >= 105_120 {
                    // 2Y+ (105,120 blocks)
                    set_data_loading.set(true);
                }
            }
        });
    }

    // Overlay toggles — initialized from URL
    let (overlay_halvings, set_overlay_halvings) =
        signal(initial_overlays.iter().any(|s| s == "halvings"));
    let (overlay_bips, set_overlay_bips) =
        signal(initial_overlays.iter().any(|s| s == "bips"));
    let (overlay_core, set_overlay_core) =
        signal(initial_overlays.iter().any(|s| s == "core"));
    let (overlay_price, set_overlay_price) =
        signal(initial_overlays.iter().any(|s| s == "price"));
    let (overlay_chain_size, set_overlay_chain_size) =
        signal(initial_overlays.iter().any(|s| s == "chain_size"));
    let (overlay_events, set_overlay_events) =
        signal(initial_overlays.iter().any(|s| s == "events"));
    let (overlay_panel_open, set_overlay_panel_open) = signal(false);

    // URL query params are read on mount (above) and synced back via
    // history.replaceState (see Effect at end of function). Direct replaceState
    // avoids the race conditions that navigate() caused with Outlet transitions.

    // Price history: fetch once when enabled, cache so toggling overlay is instant
    let price_history_resource = LocalResource::new(move || {
        let enabled = overlay_price.get();
        async move {
            if !enabled {
                return Vec::new();
            }
            let mut data: Vec<(u64, f64)> =
                match fetch_price_history(0, 4_000_000_000).await {
                    Ok(pts) => pts
                        .into_iter()
                        .map(|p| (p.timestamp_ms, p.price_usd))
                        .collect(),
                    Err(e) => {
                        leptos::logging::warn!(
                            "Price history fetch failed: {e}"
                        );
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

    let (cached_price_history, set_cached_price_history) =
        signal::<Vec<(u64, f64)>>(Vec::new());
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

    let (countdown, set_countdown) = signal(5u32);
    let (_last_updated, set_last_updated) = signal("connecting...".to_string());

    leptos_use::use_interval_fn(
        move || {
            // Pause polling when tab is hidden (saves bandwidth)
            #[cfg(feature = "hydrate")]
            {
                let hidden = leptos::prelude::document().hidden();
                if hidden {
                    return;
                }
            }
            set_countdown.update(|c| {
                if *c == 0 {
                    *c = 5;
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
    let dashboard_data =
        create_dashboard_resource(range, custom_from, custom_to);

    // Clear loading when new data arrives
    Effect::new(move |_| {
        if dashboard_data.get().is_some() {
            set_data_loading.set(false);
        }
    });

    // Fetch cumulative size offset (total bytes before visible window)
    let chain_size_offset = LocalResource::new(move || {
        let r = range.get();
        let cf = custom_from.get();
        async move {
            if r == "custom" {
                // Custom range: use timestamp-based cumulative size query
                if let Some(from_str) = cf {
                    let from_ts = date_to_ts(&from_str).unwrap_or(0);
                    if from_ts == 0 {
                        return 0u64;
                    }
                    return fetch_cumulative_size_before_ts(from_ts)
                        .await
                        .unwrap_or(0);
                }
                return 0u64;
            }
            let n = range_to_blocks(&r);
            if n >= 999_999 {
                return 0u64; // ALL range starts from genesis
            }
            let stats = fetch_stats_summary().await.ok();
            let from_height = stats
                .map(|s| s.min_height.max(s.max_height.saturating_sub(n)))
                .unwrap_or(0);
            if from_height > 0 {
                fetch_cumulative_size(from_height).await.unwrap_or(0)
            } else {
                0u64
            }
        }
    });

    // Pre-compute chain size cumulative data (with offset for absolute values)
    let cached_chain_size_data = {
        let (cached, set_cached) = signal::<Vec<(u64, f64)>>(Vec::new());
        Effect::new(move |_| {
            let offset_bytes = chain_size_offset.get().unwrap_or(0);
            let result = dashboard_data
                .get()
                .and_then(|r| r.ok())
                .map(|data| {
                    let mut cumulative: f64 =
                        offset_bytes as f64 / 1_000_000_000.0;
                    match data {
                        DashboardData::PerBlock(ref blocks) => blocks
                            .iter()
                            .map(|b| {
                                cumulative += b.size as f64 / 1_000_000_000.0;
                                (
                                    b.timestamp * 1000,
                                    (cumulative * 1000.0).round() / 1000.0,
                                )
                            })
                            .collect(),
                        DashboardData::Daily(ref days) => days
                            .iter()
                            .filter_map(|d| {
                                cumulative += d.avg_size * d.block_count as f64
                                    / 1_000_000_000.0;
                                let ts = chrono::NaiveDate::parse_from_str(
                                    &d.date, "%Y-%m-%d",
                                )
                                .map(|dt| {
                                    dt.and_hms_opt(12, 0, 0)
                                        .unwrap()
                                        .and_utc()
                                        .timestamp()
                                        as u64
                                        * 1000
                                })
                                .ok()?;
                                Some((
                                    ts,
                                    (cumulative * 1000.0).round() / 1000.0,
                                ))
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

    // Chart JSON cache — invalidate when range or overlay flags change
    let chart_cache: ChartCache = Arc::new(Mutex::new(HashMap::new()));

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
        chart_cache,
        data_loading,
        custom_from,
        set_custom_from,
        custom_to,
        set_custom_to,
    };

    // Sync state changes back to URL via history.replaceState (bypasses router)
    #[cfg(feature = "hydrate")]
    {
        let location = leptos_router::hooks::use_location();
        let mut first = true;
        Effect::new(move |_| {
            let r = range.get();
            let cf = custom_from.get();
            let ct = custom_to.get();
            let overlays = [
                ("halvings", overlay_halvings.get()),
                ("bips", overlay_bips.get()),
                ("core", overlay_core.get()),
                ("price", overlay_price.get()),
                ("chain_size", overlay_chain_size.get()),
                ("events", overlay_events.get()),
            ];
            if first {
                first = false;
                return;
            }
            let pathname = location.pathname.get();
            let search = leptos::prelude::window()
                .location()
                .search()
                .unwrap_or_default();
            let current_section = get_query_param(&search, "section");
            sync_url_to_state(
                &pathname,
                &r,
                &overlays,
                current_section.as_deref(),
                cf.as_deref(),
                ct.as_deref(),
            );
        });
    }

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
            let full_key = format!(
                "{}:{}:{}:{}",
                cache_key,
                r,
                data_fp,
                flags.cache_key()
            );
            // Check cache
            if let Ok(c) = cache.lock() {
                if let Some(cached) = c.get(&full_key) {
                    return cached.clone();
                }
            }
            let result = data_opt
                .map(|data| {
                    let (mut value, is_daily) = match data {
                        DashboardData::PerBlock(ref $blocks) => {
                            ($per_block, false)
                        }
                        DashboardData::Daily(ref $days) => ($daily, true),
                    };
                    if value.is_null() {
                        return String::new();
                    }
                    $crate::stats::charts::apply_overlays(
                        &mut value, &flags, is_daily,
                    );
                    serde_json::to_string(&value).unwrap_or_default()
                })
                .unwrap_or_default();
            if !result.is_empty() {
                if let Ok(mut c) = cache.lock() {
                    // Evict oldest entries when cache grows too large (>200 entries)
                    if c.len() > 200 {
                        c.clear();
                    }
                    c.insert(full_key, result.clone());
                }
            }
            result
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
    let set_custom_from = state.set_custom_from;
    let set_custom_to = state.set_custom_to;

    let (picker_open, set_picker_open) = signal(false);
    let (local_from, set_local_from) = signal(String::new());
    let (local_to, set_local_to) = signal(String::new());

    let range_label = move || {
        let r = range.get();
        if r == "custom" {
            "custom range"
        } else {
            let n = range_to_blocks(&r);
            if n > 5_000 {
                "daily averages"
            } else {
                "per block"
            }
        }
    };

    let apply_custom = move |_| {
        let f = local_from.get();
        let t = local_to.get();
        if f.is_empty() || t.is_empty() {
            return;
        }
        // Validate: from <= to, not before genesis, not in the future
        if f.as_str() > t.as_str() {
            return;
        }
        if f.as_str() < "2009-01-03" {
            return;
        }
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let to_clamped = if t.as_str() > today.as_str() { today } else { t };
        set_custom_from.set(Some(f));
        set_custom_to.set(Some(to_clamped));
        set_range.set("custom".to_string());
        set_picker_open.set(false);
    };

    let select_preset = move |r: String| {
        set_custom_from.set(None);
        set_custom_to.set(None);
        set_picker_open.set(false);
        set_range.set(r);
    };

    let presets = [
        "1d", "1w", "1m", "3m", "6m", "ytd", "1y", "2y", "5y", "10y", "all",
    ];

    view! {
        <div class="flex flex-col gap-2">
            // Mobile: dropdown + label
            <div class="flex sm:hidden items-center gap-2">
                <div class="relative inline-block">
                    <select
                        aria-label="Time range"
                        class="appearance-none bg-[#0a1a2e] text-white/80 text-sm border border-white/10 rounded-xl pl-3 pr-8 py-2 cursor-pointer focus:outline-none focus:border-[#f7931a]/40 transition-colors"
                        prop:value=move || range.get()
                        on:change=move |ev| {
                            use wasm_bindgen::JsCast;
                            if let Some(t) = ev.target() {
                                if let Ok(s) = t.dyn_into::<leptos::web_sys::HtmlSelectElement>() {
                                    if s.value() == "custom" {
                                        set_picker_open.set(true);
                                    } else {
                                        select_preset(s.value());
                                    }
                                }
                            }
                        }
                    >
                        {presets.into_iter().map(|r| {
                            let val = r.to_string();
                            let label = r.to_uppercase();
                            view! { <option value=val>{label}</option> }
                        }).collect::<Vec<_>>()}
                        <option value="custom">"Custom"</option>
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
                    {presets.into_iter().map(|r| {
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
                                    move |_| select_preset(r.clone())
                                }
                            >
                                {r_display}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                    <button
                        class=move || {
                            if range.get() == "custom" {
                                "px-3 py-1 text-xs rounded-lg bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
                            } else {
                                "px-3 py-1 text-xs rounded-lg text-white/40 hover:text-white/70 hover:bg-white/5 transition-all cursor-pointer"
                            }
                        }
                        on:click=move |_| set_picker_open.update(|v| *v = !*v)
                    >
                        "Custom"
                    </button>
                </div>
                <span class="ml-3 text-xs text-white/60 self-center">{range_label}</span>
            </div>
            // Date picker (shown when Custom is active/clicked)
            <Show when=move || picker_open.get()>
                <div class="flex items-center gap-2 bg-[#0a1a2e] rounded-xl p-2 border border-white/10">
                    <input
                        type="date"
                        min="2009-01-03"
                        max=move || chrono::Utc::now().format("%Y-%m-%d").to_string()
                        class="bg-[#0d2137] text-white text-xs border border-white/10 rounded-lg px-2 py-1.5 focus:outline-none focus:border-[#f7931a]/40"
                        style="color-scheme: dark"
                        prop:value=move || local_from.get()
                        on:input=move |ev| {
                                set_local_from.set(event_target_value(&ev));
                        }
                    />
                    <span class="text-white/30 text-xs">"to"</span>
                    <input
                        type="date"
                        min="2009-01-03"
                        max=move || chrono::Utc::now().format("%Y-%m-%d").to_string()
                        class="bg-[#0d2137] text-white text-xs border border-white/10 rounded-lg px-2 py-1.5 focus:outline-none focus:border-[#f7931a]/40"
                        style="color-scheme: dark"
                        prop:value=move || local_to.get()
                        on:input=move |ev| {
                                set_local_to.set(event_target_value(&ev));
                        }
                    />
                    <button
                        class="px-3 py-1.5 text-xs bg-[#f7931a] text-[#1a1a2e] font-semibold rounded-lg cursor-pointer hover:bg-[#f4a949] transition-colors"
                        on:click=apply_custom
                    >
                        "Go"
                    </button>
                </div>
            </Show>
        </div>
    }
}

/// Floating overlay panel (hidden on dashboard since there are no charts)
#[component]
pub fn OverlayPanel() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let location = leptos_router::hooks::use_location();
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

    let location = leptos_router::hooks::use_location();

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
            {header.map(|h| view! { <div class="flex items-center gap-3 flex-shrink-0">{h.run()}</div> })}
            <div class="sm:ml-auto">
                <RangeSelector/>
            </div>
        </div>
        {children()}
        <FloatingRangePicker/>
        <ChartDrawer/>
    }
}

/// Floating range picker button in the bottom-left corner.
/// Opens a popover with the full range selector when clicked.
#[component]
fn FloatingRangePicker() -> impl IntoView {
    let (open, set_open) = signal(false);

    view! {
        <div style="z-index: 10000" class="fixed bottom-6 right-6">
            // Toggle button
            <button
                class="w-11 h-11 sm:w-14 sm:h-14 rounded-full bg-[#0d2137] border border-[#f7931a]/30 shadow-lg shadow-black/30 flex items-center justify-center cursor-pointer hover:border-[#f7931a]/60 hover:scale-105 active:scale-95 transition-all"
                on:click=move |_| set_open.update(|v| *v = !*v)
                title="Change time range"
            >
                <svg class="w-5 h-5 sm:w-6 sm:h-6 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"/>
                </svg>
            </button>
            // Popover
            <Show when=move || open.get()>
                <div class="absolute bottom-14 right-0 bg-[#0d2137] border border-white/10 rounded-2xl shadow-2xl shadow-black/50 p-3 min-w-[280px]">
                    <div class="flex items-center justify-between mb-2">
                        <span class="text-xs text-white/50 font-medium">"Time Range"</span>
                        <button
                            class="text-white/30 hover:text-white/60 cursor-pointer"
                            on:click=move |_| set_open.set(false)
                        >
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
                            </svg>
                        </button>
                    </div>
                    <RangeSelector/>
                </div>
            </Show>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Chart sidebar drawer
// ---------------------------------------------------------------------------

/// Entry in the chart drawer: a chart name and its HTML element ID for scrolling.
struct DrawerChart {
    label: &'static str,
    card_id: &'static str,
}

/// A section (or subsection) of charts within the drawer.
struct DrawerSection {
    label: &'static str,
    /// URL section key (e.g. "blocks", "adoption"). Empty if page has no sub-sections.
    section_key: &'static str,
    charts: Vec<DrawerChart>,
}

/// A top-level page grouping in the drawer.
struct DrawerPage {
    label: &'static str,
    path_prefix: &'static str,
    sections: Vec<DrawerSection>,
}

fn drawer_pages() -> Vec<DrawerPage> {
    vec![
        DrawerPage {
            label: "Network",
            path_prefix: "/observatory/charts/network",
            sections: vec![
                DrawerSection {
                    label: "Blocks",
                    section_key: "blocks",
                    charts: vec![
                        DrawerChart { label: "Block Size", card_id: "card-chart-size" },
                        DrawerChart { label: "Weight Utilization", card_id: "card-chart-weight-util" },
                        DrawerChart { label: "Transaction Count", card_id: "card-chart-txcount" },
                        DrawerChart { label: "TPS", card_id: "card-chart-tps" },
                        DrawerChart { label: "Avg Transaction Size", card_id: "card-chart-avg-tx-size" },
                        DrawerChart { label: "Block Interval", card_id: "card-chart-interval" },
                        DrawerChart { label: "Chain Size Growth", card_id: "card-chart-chain-size" },
                        DrawerChart { label: "Block Fullness Distribution", card_id: "card-chart-fullness-dist" },
                        DrawerChart { label: "Block Time Distribution", card_id: "card-chart-time-dist" },
                        DrawerChart { label: "Rapid Consecutive Blocks", card_id: "card-chart-propagation" },
                        DrawerChart { label: "Weekday Activity", card_id: "card-chart-weekday" },
                    ],
                },
                DrawerSection {
                    label: "Adoption",
                    section_key: "adoption",
                    charts: vec![
                        DrawerChart { label: "SegWit Adoption", card_id: "card-chart-segwit" },
                        DrawerChart { label: "Taproot Outputs", card_id: "card-chart-taproot" },
                        DrawerChart { label: "Witness Version Comparison", card_id: "card-chart-witness-versions" },
                        DrawerChart { label: "Witness Version Share", card_id: "card-chart-witness-pct" },
                        DrawerChart { label: "Output Type Breakdown", card_id: "card-chart-witness-tx-pct" },
                        DrawerChart { label: "Address Type Evolution", card_id: "card-chart-address-types" },
                        DrawerChart { label: "Address Type Share", card_id: "card-chart-address-types-pct" },
                        DrawerChart { label: "Witness Data Share", card_id: "card-chart-witness-share" },
                        DrawerChart { label: "Taproot Spend Types", card_id: "card-chart-taproot-spend-types" },
                        DrawerChart { label: "Taproot Adoption Velocity", card_id: "card-chart-taproot-velocity" },
                        DrawerChart { label: "Cumulative Adoption", card_id: "card-chart-cumulative-adoption" },
                        DrawerChart { label: "P2PKH Sunset Tracker", card_id: "card-chart-p2pkh-sunset" },
                    ],
                },
                DrawerSection {
                    label: "Transactions",
                    section_key: "transactions",
                    charts: vec![
                        DrawerChart { label: "RBF Adoption", card_id: "card-chart-rbf" },
                        DrawerChart { label: "UTXO Flow", card_id: "card-chart-utxo-flow" },
                        DrawerChart { label: "Batching Efficiency", card_id: "card-chart-batching" },
                        DrawerChart { label: "Transaction Density", card_id: "card-chart-tx-density" },
                        DrawerChart { label: "UTXO Growth Rate", card_id: "card-chart-utxo-growth" },
                    ],
                },
            ],
        },
        DrawerPage {
            label: "Fees",
            path_prefix: "/observatory/charts/fees",
            sections: vec![
                DrawerSection {
                    label: "",
                    section_key: "",
                    charts: vec![
                        DrawerChart { label: "Total Fees per Block", card_id: "card-chart-fees" },
                        DrawerChart { label: "Avg Fee per Transaction", card_id: "card-chart-avg-fee-tx" },
                        DrawerChart { label: "Median Fee Rate", card_id: "card-chart-median-rate" },
                        DrawerChart { label: "Fee Rate Bands", card_id: "card-chart-fee-band" },
                        DrawerChart { label: "Subsidy vs Fees", card_id: "card-chart-subsidy-fees" },
                        DrawerChart { label: "Fee Revenue Share", card_id: "card-chart-fee-revenue-share" },
                        DrawerChart { label: "BTC Transferred Volume", card_id: "card-chart-btc-volume" },
                        DrawerChart { label: "Input vs Output Value", card_id: "card-chart-value-flow" },
                        DrawerChart { label: "Fee Pressure vs Block Space", card_id: "card-chart-fee-pressure" },
                        DrawerChart { label: "Fee Spike Detector", card_id: "card-chart-fee-spikes" },
                        DrawerChart { label: "Halving Era Comparison", card_id: "card-chart-halving-era" },
                    ],
                },
            ],
        },
        DrawerPage {
            label: "Mining",
            path_prefix: "/observatory/charts/mining",
            sections: vec![
                DrawerSection {
                    label: "Difficulty",
                    section_key: "difficulty",
                    charts: vec![
                        DrawerChart { label: "Difficulty", card_id: "card-chart-difficulty" },
                        DrawerChart { label: "Difficulty Ribbon", card_id: "card-chart-diff-ribbon" },
                    ],
                },
                DrawerSection {
                    label: "Pool Distribution",
                    section_key: "pools",
                    charts: vec![
                        DrawerChart { label: "Mining Pool Share", card_id: "card-chart-miner-dominance" },
                        DrawerChart { label: "Mining Diversity Index", card_id: "card-chart-diversity" },
                        DrawerChart { label: "Empty Blocks", card_id: "card-chart-empty-blocks" },
                        DrawerChart { label: "Empty Blocks by Pool", card_id: "card-chart-empty-by-pool" },
                    ],
                },
            ],
        },
        DrawerPage {
            label: "Embedded Data",
            path_prefix: "/observatory/charts/embedded",
            sections: vec![
                DrawerSection {
                    label: "Overview",
                    section_key: "overview",
                    charts: vec![
                        DrawerChart { label: "All Embedded Share", card_id: "card-chart-all-embedded-share" },
                        DrawerChart { label: "All Embedded Count", card_id: "card-chart-unified-count" },
                        DrawerChart { label: "All Embedded Volume", card_id: "card-chart-unified-volume" },
                    ],
                },
                DrawerSection {
                    label: "Protocols",
                    section_key: "protocols",
                    charts: vec![
                        DrawerChart { label: "OP_RETURN Count", card_id: "card-chart-opreturn-count" },
                        DrawerChart { label: "OP_RETURN Volume", card_id: "card-chart-opreturn-bytes" },
                        DrawerChart { label: "OP_RETURN Protocol Share", card_id: "card-chart-runes-pct" },
                        DrawerChart { label: "OP_RETURN Block Share", card_id: "card-chart-op-block-share" },
                    ],
                },
                DrawerSection {
                    label: "Inscriptions",
                    section_key: "inscriptions",
                    charts: vec![
                        DrawerChart { label: "Inscription Count", card_id: "card-chart-inscriptions" },
                        DrawerChart { label: "Inscription Share", card_id: "card-chart-inscription-share" },
                    ],
                },
            ],
        },
    ]
}

/// Collapsible sidebar drawer listing all observatory charts organized by page
/// and section. Clicking a chart name scrolls to it. The current page is highlighted.
#[allow(unused_variables)]
#[component]
pub fn ChartDrawer() -> impl IntoView {
    let (open, set_open) = signal(false);
    let location = use_location();

    let pages = drawer_pages();

    view! {
        // Toggle tab fixed on the left edge
        <button
            style="z-index: 10001"
            class="fixed left-0 top-1/2 -translate-y-1/2 bg-[#0d2137] border border-l-0 border-white/10 rounded-r-lg px-1.5 py-4 cursor-pointer hover:bg-[#143050] transition-colors group"
            on:click=move |_| set_open.set(true)
            title="Chart index"
        >
            <svg class="w-4 h-4 text-white/50 group-hover:text-white/80 transition-colors" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"/>
            </svg>
        </button>

        // Backdrop
        <Show when=move || open.get()>
            <div
                style="z-index: 10002"
                class="fixed inset-0 bg-black/50 transition-opacity"
                on:click=move |_| set_open.set(false)
            />
        </Show>

        // Drawer panel
        <div
            style=move || format!(
                "z-index: 10003; transform: translateX({}); transition: transform 0.25s ease-in-out;",
                if open.get() { "0" } else { "-100%" }
            )
            class="fixed top-[48px] left-0 bottom-0 w-72 bg-[#0d2137] border-r border-white/10 overflow-y-auto"
        >
            // Header
            <div class="flex items-center justify-between px-4 py-3 border-b border-white/10">
                <span class="text-sm font-semibold text-white/80">"Chart Index"</span>
                <button
                    class="text-white/30 hover:text-white/60 cursor-pointer"
                    on:click=move |_| set_open.set(false)
                >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
                    </svg>
                </button>
            </div>

            // Content
            <nav class="p-3">
                {pages.into_iter().map(|page| {
                    let path_prefix = page.path_prefix;
                    view! {
                        <div class="mb-3">
                            // Page heading
                            <div class=move || {
                                let current = location.pathname.get();
                                if current.starts_with(path_prefix) {
                                    "text-sm font-bold text-[#f7931a] uppercase tracking-wider px-2 py-1.5 border-b border-[#f7931a]/20 mb-1"
                                } else {
                                    "text-sm font-bold text-white/50 uppercase tracking-wider px-2 py-1.5 border-b border-white/5 mb-1"
                                }
                            }>
                                {page.label}
                            </div>
                            // Sections
                            {page.sections.into_iter().map(|section| {
                                let has_label = !section.label.is_empty();
                                let section_key = section.section_key;
                                view! {
                                    <div class="ml-1">
                                        {if has_label {
                                            Some(view! {
                                                <div class="text-[11px] text-white/45 font-semibold uppercase tracking-wider px-2 pt-2 pb-1">
                                                    {section.label}
                                                </div>
                                            })
                                        } else {
                                            None
                                        }}
                                        <ul class="space-y-0">
                                            {section.charts.into_iter().map(|chart| {
                                                let card_id = chart.card_id;
                                                view! {
                                                    <li>
                                                        <button
                                                            class="w-full text-left text-[12px] text-white/60 hover:text-white hover:bg-white/5 rounded-md px-3 py-1 cursor-pointer transition-colors"
                                                            on:click=move |_| {
                                                                set_open.set(false);
                                                                #[cfg(feature = "hydrate")]
                                                                {
                                                                    let current = location.pathname.get_untracked();
                                                                    if !current.starts_with(path_prefix) {
                                                                        // Navigate to the correct page with section param and hash
                                                                        let url = if section_key.is_empty() {
                                                                            format!("{}#{}", path_prefix, card_id)
                                                                        } else {
                                                                            format!("{}?section={}#{}", path_prefix, section_key, card_id)
                                                                        };
                                                                        let _ = leptos::prelude::window().location().set_href(&url);
                                                                    } else if let Some(el) = leptos::prelude::document().get_element_by_id(card_id) {
                                                                        el.scroll_into_view();
                                                                    }
                                                                }
                                                            }
                                                        >
                                                            {chart.label}
                                                        </button>
                                                    </li>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </ul>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </nav>
        </div>
    }
}
