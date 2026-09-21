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
    /// The dashboard payload **and the window it answers**. See
    /// `DashboardValue`: pairing them is what makes `data_loading` correct.
    pub dashboard_data: LocalResource<DashboardValue>,
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
    pub overlay_log_scale: ReadSignal<bool>,
    pub set_overlay_log_scale: WriteSignal<bool>,
    pub overlay_right_log_scale: ReadSignal<bool>,
    pub set_overlay_right_log_scale: WriteSignal<bool>,
    /// The chart laid over the current one, by slug, or empty for none.
    ///
    /// Shared rather than page-local so it survives a refresh and can be
    /// shared in a link. That means it also survives navigation to another
    /// chart, where it may name that chart or one that cannot be compared
    /// with it, so **nothing may render this without resolving it through
    /// `registry::comparison_for` first**. It is a stored intent, not a
    /// guarantee.
    pub compare: ReadSignal<String>,
    pub set_compare: WriteSignal<String>,
    pub price_loading: Signal<bool>,
    /// True while the selected window is shorter than one price sample
    /// interval, so the price overlay would draw nothing. See
    /// `PRICE_SAMPLE_INTERVAL_SECS`.
    pub price_sparse: Signal<bool>,
    // Unified chart-settings panel (one floating button, tabs for Overlays + Range).
    // Replaces the earlier split between OverlayPanel and FloatingRangePicker.
    pub chart_settings_open: ReadSignal<bool>,
    pub set_chart_settings_open: WriteSignal<bool>,
    pub chart_settings_tab: ReadSignal<ChartSettingsTab>,
    pub set_chart_settings_tab: WriteSignal<ChartSettingsTab>,
    // chart JSON cache — persists across Outlet navigations
    pub chart_cache: ChartCache,
    /// Total bytes of chain before the visible window, so the chain-size
    /// chart and its overlay can show absolute rather than range-relative
    /// values. Exposed here because the network page needs the same number:
    /// it previously ran an identical LocalResource of its own, so every range
    /// change fired two identical requests for it.
    pub chain_size_offset: LocalResource<u64>,
    /// Block data for the whole chain today, in bytes.
    ///
    /// Calibrates the chain-size chart's disk estimate. Range-independent by
    /// design, which is the whole point: deriving the ratio from the selected
    /// window pinned every historical window's disk line to today's disk size.
    /// It has no reactive dependencies, so it is fetched once per page rather
    /// than per range change.
    pub chain_size_total: LocalResource<u64>,
    /// The difficulty retargets inside the loaded daily window, plus the
    /// epoch before it.
    ///
    /// Read by Difficulty Adjustment at daily resolution and by nothing else.
    /// Held here rather than in the mining page because the single-chart view
    /// needs the same rows, and because it is derived from
    /// `dashboard_data`: the retargets always describe the window the days
    /// describe. Empty at per-block ranges, where difficulty is read off the
    /// blocks themselves.
    ///
    /// The cost of holding it here is one request per daily range change on
    /// every observatory page, including the ones with no difficulty chart,
    /// which is the same trade `chain_size_offset` already makes. It is a
    /// 2.1ms query returning at most ~480 rows for the whole chain.
    pub retargets: LocalResource<Result<Vec<Retarget>, String>>,
    // true while the dashboard data resource hasn't yet resolved a value
    // for the *currently selected* range/custom-window.
    pub data_loading: Signal<bool>,
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

/// The last second of a date, for the **inclusive** end of a window.
///
/// Every timestamp query in `db.rs` is inclusive at both ends, and the tip
/// timestamp that the named ranges pass as their end is a real block time, so
/// an exclusive convention would drop the newest block from every chart. The
/// window therefore has to be closed, and a date picked as "to" has to become
/// the last instant of that day rather than the first instant of the next one.
///
/// Adding a whole day was the bug. It reads as "include the entire end day"
/// and is correct against a half-open query, but `query_daily_aggregates_fast`
/// reads the pre-computed table with `day <= date(to_ts)`, and `date` of the
/// next midnight is the next day: a request for 1 January to 31 March 2020
/// returned 92 rows ending on 1 April. The raw twin has
/// `timestamp <= to_ts`, which instead let a single block landing exactly on
/// midnight open a one-block partial day. One convention, one helper, so the
/// two cannot disagree again.
pub fn date_to_ts_end(date: &str) -> Option<u64> {
    date_to_ts(date).map(|t| t + 86_399)
}

/// Create a `LocalResource` that fetches dashboard data for the given range.
/// Returns `PerBlock` data for short ranges (under ~5000 blocks) or `Daily`
/// aggregates for longer ranges. Supports custom date ranges via from/to params.
/// The window a fetch was issued for: the range name and the two custom dates.
pub type WindowKey = (String, Option<String>, Option<String>);

/// How far apart the price samples are, in seconds.
///
/// **Measured, not assumed.** `fetch_price_history_all` asks blockchain.info
/// for `timespan=all`, and that endpoint reduces resolution for a long span:
/// on 2026-09-21 it returned 1,618 points spaced exactly 345,600,000 ms apart,
/// which is four days. It is the only price series the site fetches, and it is
/// cached for an hour and then filtered client-side, so **every** range gets
/// four-day sampling rather than the daily figure the overlay's copy used to
/// claim.
///
/// Probe, against a running server:
///
/// ```text
/// curl -s -X POST localhost:8000/api/stats_price_history \
///   -d 'from_ts=0&to_ts=4000000000' | head -c 300
/// ```
///
/// If a future endpoint change tightens the spacing, this constant is the one
/// place to correct, and the toggle it gates relaxes on its own.
pub const PRICE_SAMPLE_INTERVAL_SECS: u64 = 4 * 86_400;

/// Whether a window of `span_secs` is too short for the price series to draw.
///
/// A line needs two points. Samples sit one interval apart, so a window
/// narrower than the interval can contain at most one of them, and one point
/// with `symbol: "none"` paints nothing.
pub fn price_window_too_short(span_secs: u64) -> bool {
    span_secs < PRICE_SAMPLE_INTERVAL_SECS
}

/// A dashboard payload and **the window it answers**, which travel together.
///
/// The window is part of the value rather than recorded beside it, because
/// every version of "record it beside it" has been wrong, twice in one day:
///
/// - An `Effect` stamping the ambient range when the resource held any value.
///   A `LocalResource` keeps its previous payload while refetching, so that
///   stamped the new range against the old data.
/// - The fetch stamping its own window on the way out. That reads correctly
///   and is worse: the write happens *inside* the future, so it lands before
///   the future's value reaches the resource. The loading flag then cleared one
///   step ahead of the data every single time, turning an occasional flash into
///   a certain one.
///
/// Carried in the value, the stamp cannot arrive early or late, because it
/// arrives as the data. There is no ordering left to get wrong. The result is
/// inside so a failed fetch still says which window failed, rather than
/// leaving the flag stuck and the skeleton spinning.
pub type DashboardValue = (WindowKey, Result<DashboardData, String>);

pub fn create_dashboard_resource(
    range: ReadSignal<String>,
    custom_from: ReadSignal<Option<String>>,
    custom_to: ReadSignal<Option<String>>,
) -> LocalResource<DashboardValue> {
    LocalResource::new(move || {
        let r = range.get();
        let cf = custom_from.get();
        let ct = custom_to.get();
        let stamp = (r.clone(), cf.clone(), ct.clone());
        async move {
            let out = async move {
                let stats =
                    fetch_stats_summary().await.map_err(|e| e.to_string())?;

                // Custom date range: timestamp-based queries directly
                if r == "custom" {
                    if let (Some(from_str), Some(to_str)) = (cf, ct) {
                        let from_ts = date_to_ts(&from_str).unwrap_or(0);
                        let to_ts = date_to_ts_end(&to_str)
                            .unwrap_or(stats.latest_timestamp);
                        let approx_blocks = to_ts.saturating_sub(from_ts) / 600;
                        if uses_daily_aggregates(approx_blocks) {
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
                let is_daily = uses_daily_aggregates(n);

                if is_daily {
                    let from_ts =
                        stats.latest_timestamp.saturating_sub(n * 600);
                    let days =
                        fetch_daily_aggregates(from_ts, stats.latest_timestamp)
                            .await
                            .map_err(|e| e.to_string())?;
                    Ok::<_, String>(DashboardData::Daily(days))
                } else {
                    let from = stats
                        .min_height
                        .max(stats.max_height.saturating_sub(n));
                    let blocks = fetch_blocks(from, stats.max_height)
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok(DashboardData::PerBlock(blocks))
                }
            }
            .await;
            (stamp, out)
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

    // A permalink carries the view, not just the data: which range, which
    // annotations, which scale on each axis and what is laid over the chart.
    // Anything omitted here silently resets on refresh, which is how the
    // scale toggles behaved before and read as the control not working.
    let initial_scale_log = query
        .read_untracked()
        .get("scale")
        .is_some_and(|v| v == "log");
    let initial_right_scale_log = query
        .read_untracked()
        .get("overlay_scale")
        .is_some_and(|v| v == "log");
    // Stored unvalidated on purpose. Validity depends on which chart is being
    // viewed, which this function does not know, so the page resolves it
    // through `registry::comparison_for` on every render instead.
    let initial_compare = query
        .read_untracked()
        .get("compare")
        .filter(|s| !s.is_empty())
        .unwrap_or_default();

    let initial_custom_from =
        query.read_untracked().get("from").filter(|s| !s.is_empty());
    let initial_custom_to =
        query.read_untracked().get("to").filter(|s| !s.is_empty());

    let (range, set_range) = signal(initial_range);
    let (custom_from, set_custom_from) = signal(initial_custom_from);
    let (custom_to, set_custom_to) = signal(initial_custom_to);

    // `data_loading` is wired further down, once `dashboard_data` exists.

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

    // Loading signal: true while the live (range, custom_from, custom_to)
    // hasn't yet been resolved by the dashboard_data resource.
    //
    // Why this matters: LocalResource.get() returns the *previous* resolved
    // value while a refetch is in flight (the value cell isn't cleared on
    // input change), so chart_memo never produces an empty string and the
    // ChartCard skeleton wouldn't otherwise show. The earlier "n >= 2Y"
    // gate skipped feedback for shorter range switches entirely — fine on
    // desktop, painful on slower mobile devices where even a 1Y → 1M
    // recompute (~28 charts × ~4,320 per-block rows) takes several seconds.
    //
    // We track which (range, from, to) the resource last resolved for, and
    // derive `data_loading` as "those don't match the live values yet."
    //
    // **The window is read off the payload**, so "has the new data arrived" is
    // answered by the data and not by a stamp kept beside it. `DashboardValue`
    // records the two ways keeping it beside the data failed on 2026-09-21,
    // the second of which made the flash certain rather than occasional.
    //
    // An out-of-order completion, where a superseded fetch lands after a newer
    // one, shows the older window and puts the skeleton back until the newer
    // one arrives. That is a spurious skeleton rather than a stale chart, which
    // is the right way round for this to fail.
    let data_loading = Signal::derive(move || {
        let want = (range.get(), custom_from.get(), custom_to.get());
        dashboard_data.get().is_none_or(|(have, _)| have != want)
    });

    // Whether the selected window is too short for the price series to draw.
    //
    // `PRICE_SAMPLE_INTERVAL_SECS` explains the number. A window shorter than
    // one sampling interval holds at most one historical sample, and one point
    // with `symbol: "none"` draws nothing at all: the legend gains a "Price
    // (USD)" entry and the plot gains no line. Reported on 2026-09-21 from a
    // 1D chart, where the overlay was offered, accepted, and silently drew
    // nothing.
    //
    // Read off the loaded rows rather than the range name, because a custom
    // window of two days is just as short and `range_to_blocks` maps "custom"
    // to 999,999. Daily resolution starts at thousands of blocks, so those
    // windows are never short enough to matter.
    let price_sparse = Signal::derive(move || {
        dashboard_data
            .get()
            .and_then(|(_, r)| r.ok())
            .and_then(|data| match data {
                DashboardData::PerBlock(blocks) => {
                    let first = blocks.first()?.timestamp;
                    let last = blocks.last()?.timestamp;
                    Some(price_window_too_short(last.saturating_sub(first)))
                }
                DashboardData::Daily(_) => Some(false),
            })
            .unwrap_or(false)
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

    // A timestamp past any block, so this asks for the whole chain. Not
    // `u64::MAX`, which cannot bind to the query's i64 parameter; 4e9 is the
    // year 2096 and comfortably inside it.
    let chain_size_total = LocalResource::new(|| async move {
        fetch_cumulative_size_before_ts(4_000_000_000)
            .await
            .unwrap_or(0)
    });

    // The retarget blocks inside the loaded window, for the one chart that
    // cannot read them off the daily rows.
    //
    // Keyed on the resolved days rather than on the range, which is what
    // keeps this honest: the window asked for here is exactly the window the
    // days describe, so the two payloads cannot end up covering different
    // periods. Deriving it from `range` again would be a second
    // interpretation of the same question, and the two would drift the first
    // time either changed.
    //
    // Empty for per-block ranges, which read difficulty off the blocks they
    // already have.
    let retargets = LocalResource::new(move || {
        let window =
            dashboard_data
                .get()
                .and_then(|(_, r)| r.ok())
                .and_then(|data| match data {
                    DashboardData::Daily(ref days) => {
                        let first = days.first()?;
                        let last = days.last()?;
                        Some((
                            date_to_ts(&first.date)?,
                            date_to_ts_end(&last.date)?,
                        ))
                    }
                    DashboardData::PerBlock(_) => None,
                });
        async move {
            match window {
                // The error is kept rather than flattened to an empty list.
                // An empty list is a real answer here ("no retarget in this
                // window"), so a failed fetch that became one would draw a
                // chart asserting that nothing happened. The consumer shows
                // the loading state instead.
                Some((from_ts, to_ts)) => {
                    fetch_retargets(from_ts, to_ts).await.map_err(|e| {
                        leptos::logging::warn!("Retarget fetch failed: {e}");
                        e.to_string()
                    })
                }
                None => Ok(Vec::new()),
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
                .and_then(|(_, r)| r.ok())
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

    // Not an annotation like the others, but it belongs to the same bundle:
    // it is applied after the base chart is built and so shares the cache
    // generation. Naming it `overlay_*` keeps it with the flags it travels
    // with rather than implying it draws something.
    let (overlay_log_scale, set_overlay_log_scale) = signal(initial_scale_log);
    // The right axis, which only exists while price or chain size is on. Kept
    // separate from `overlay_log_scale` so a reader can put price on a log
    // axis, where six decades are readable, without moving the metric they
    // came to look at onto a scale they did not ask for.
    let (overlay_right_log_scale, set_overlay_right_log_scale) =
        signal(initial_right_scale_log);
    let (compare, set_compare) = signal(initial_compare);

    let overlay_flags = Signal::derive(move || {
        // `price_sparse` as well as the toggle, so a window too short to draw
        // the series does not carry it. The toggle is a stored preference and
        // survives a range change, so without this a reader who turned price
        // on at 1Y and then moved to 1D kept a "Price (USD)" legend entry
        // beside no line, and the toggle stayed checked while disabled.
        let price_data = if overlay_price.get() && !price_sparse.get() {
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
            log_scale: overlay_log_scale.get(),
            right_log_scale: overlay_right_log_scale.get(),
        }
    });

    // Chart JSON cache — invalidate when range or overlay flags change
    let chart_cache: ChartCache = Arc::new(Mutex::new(HashMap::new()));

    let state = ObservatoryState {
        range,
        set_range,
        chain_size_offset,
        chain_size_total,
        retargets,
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
        overlay_log_scale,
        set_overlay_log_scale,
        overlay_right_log_scale,
        set_overlay_right_log_scale,
        compare,
        set_compare,
        price_loading,
        price_sparse,
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
            let scales = super::url_sync::Scales {
                left_log: overlay_log_scale.get(),
                right_log: overlay_right_log_scale.get(),
            };
            let cmp = compare.get();
            if first {
                first = false;
                return;
            }
            let pathname = location.pathname.get();
            // Only write to the entry that is actually on screen.
            //
            // This writes with `replaceState`, which edits whichever history
            // entry is current. The effect also re-runs when the router's
            // pathname changes, so during a navigation it can fire while the
            // browser still has the previous entry current, stamping the new
            // page's query string onto the old page's history entry. Going
            // back then lands on a URL that never existed.
            //
            // Comparing the router's view of the path against the browser's
            // is what tells the two apart: equal means the navigation has
            // settled and this entry is ours to edit.
            let live = leptos::prelude::window()
                .location()
                .pathname()
                .unwrap_or_default();
            if live != pathname {
                return;
            }
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
                scales,
                &cmp,
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
