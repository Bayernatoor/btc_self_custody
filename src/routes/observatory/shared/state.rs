//! Observatory shared state and data resource creation.
//!
//! `ObservatoryState` is provided via Leptos context by the parent `ObservatoryPage`
//! and consumed by all child chart pages. It holds the time range, overlay toggles,
//! dashboard data resource, live stats cache, and chart JSON cache.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use leptos::prelude::*;
use leptos_router::hooks::use_query_map;

use crate::routes::observatory::helpers::*;
use crate::stats::charts::OverlayFlags;
use crate::stats::server_fns::*;
use crate::stats::types::*;

// ---------------------------------------------------------------------------
// Chart cache
// ---------------------------------------------------------------------------

/// Client-side chart JSON cache keyed by a composite string of chart ID, range,
/// data fingerprint, and overlay flags. Persists at the parent level across
/// Outlet navigations so switching tabs does not recompute charts.
/// Uses Arc<Mutex> for Send+Sync compatibility with SSR.
pub type ChartCache = Arc<Mutex<HashMap<String, (u64, String)>>>;
pub static CHART_CACHE_SEQ: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Which tab is active in the unified chart-settings panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChartSettingsTab {
    Overlays,
    Range,
}

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
    // Unified chart-settings panel (one floating button, tabs for Overlays + Range).
    // Replaces the earlier split between OverlayPanel and FloatingRangePicker.
    pub chart_settings_open: ReadSignal<bool>,
    pub set_chart_settings_open: WriteSignal<bool>,
    pub chart_settings_tab: ReadSignal<ChartSettingsTab>,
    pub set_chart_settings_tab: WriteSignal<ChartSettingsTab>,
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
    let (chart_settings_open, set_chart_settings_open) = signal(false);
    // Default tab is Range — users change the time window far more often
    // than they toggle overlays, so leading with Range matches their
    // actual reach-for-this-first behavior.
    let (chart_settings_tab, set_chart_settings_tab) =
        signal(ChartSettingsTab::Range);

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

    // Live stats (auto-refresh every 6s via the countdown interval below)
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
        chart_settings_open,
        set_chart_settings_open,
        chart_settings_tab,
        set_chart_settings_tab,
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
            let current_section =
                super::url_sync::get_query_param(&search, "section");
            super::url_sync::sync_url_to_state(
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
