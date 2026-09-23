//! `/observatory/chart/:slug`: one chart, full page, with the tools that do
//! not fit on a card.
//!
//! Sits inside the `ObservatoryPage` parent route, so `range`, `dashboard_data`
//! and the overlay flags come from the same context the multi-chart pages use.
//! That is deliberate and was verified before building: range selection
//! carries across navigation into this view and back out, which a
//! self-provided state would have broken.
//!
//! Which charts exist and how each is built lives in
//! `stats::charts::registry`, not here. This file knows how to lay one out.

use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::hooks::use_params_map;

use super::components::{Chart, DataLoadError};
use super::helpers::range_to_blocks;
use super::shared::{DashboardData, ObservatoryState};
use crate::stats::charts::kpi::{self, Kpis};
use crate::stats::charts::registry::{
    self, ChartMeta, Daily, MiningChart, Source,
};
use crate::stats::charts::OverlayFlags;
use crate::stats::server_fns::{
    fetch_block_time_histogram, fetch_empty_blocks_by_pool,
    fetch_empty_blocks_monthly, fetch_fullness_histogram,
    fetch_miner_dominance, fetch_miner_dominance_daily, fetch_stats_summary,
};
use crate::stats::types::{uses_daily_aggregates, HistogramBucket, MinerShare};

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = downloadChartCSV)]
    fn download_chart_csv(chart_id: &str, title: &str, range: &str);
    #[wasm_bindgen(js_name = downloadChartPNG)]
    fn download_chart_png(chart_id: &str, title: &str);
    #[wasm_bindgen(js_name = downloadChartJSON)]
    fn download_chart_json(chart_id: &str, title: &str, range: &str);
}

#[cfg(not(feature = "hydrate"))]
fn download_chart_csv(_id: &str, _title: &str, _range: &str) {}
#[cfg(not(feature = "hydrate"))]
fn download_chart_png(_id: &str, _title: &str) {}
#[cfg(not(feature = "hydrate"))]
fn download_chart_json(_id: &str, _title: &str, _range: &str) {}

/// Miner shares plus the two empty-block groupings, as the mining page
/// fetches them. Named so the early "this chart does not need it" return
/// in the resource still pins the Ok type; without it the tuple's first
/// element infers as an unsized slice.
type MiningPayload =
    (Vec<MinerShare>, Vec<HistogramBucket>, Vec<HistogramBucket>);

/// The DOM id the chart canvas gets. Distinct from the card id on the
/// multi-chart pages so both can be mounted without colliding, which matters
/// because the CSV and PNG exports look the instance up by element id.
fn canvas_id(slug: &str) -> String {
    format!("single-chart-{slug}")
}

fn fmt_num(v: f64) -> String {
    let abs = v.abs();
    if abs >= 1e12 {
        format!("{:.1}T", v / 1e12)
    } else if abs >= 1e9 {
        format!("{:.1}G", v / 1e9)
    } else if abs >= 1_000_000.0 {
        format!("{:.2}M", v / 1_000_000.0)
    } else if abs >= 10_000.0 {
        format!("{:.1}k", v / 1_000.0)
    } else if abs >= 100.0 {
        format!("{v:.0}")
    } else if abs >= 1.0 {
        format!("{v:.2}")
    } else if abs >= 0.001 {
        format!("{v:.4}")
    } else if abs > 0.0 {
        // Genesis difficulty is 1 against a present-day 1.6e14, so the low
        // over ALL is real but tiny. "0.0000" claimed it was zero.
        format!("{v:.2e}")
    } else {
        "0".to_string()
    }
}

/// Millisecond timestamp to `YYYY-MM-DD`, for the date under a peak or low.
fn fmt_ms(ms: Option<f64>) -> Option<String> {
    let ms = ms?;
    chrono::DateTime::from_timestamp_millis(ms as i64)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
}

/// Beyond this ratio between the first and last value, a percentage change
/// stops informing: the number is dominated by how small the baseline was.
const PCT_MEANINGFUL_RATIO: f64 = 10_000.0;

/// Which server-side histogram a chart needs over a long range.
///
/// The two distribution charts have no daily builder and read pre-bucketed
/// counts instead, and they read *different* buckets. Naming that here rather
/// than with a bool is what stops one being fetched for the other.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Buckets {
    Fullness,
    Time,
    None,
}

/// The buckets themselves, tagged, so the render arm cannot read a fullness
/// histogram as a block-time one.
#[derive(Clone)]
enum Histogram {
    Fullness(Vec<HistogramBucket>),
    Time(Vec<HistogramBucket>),
}

/// Build a chart from the dashboard rows already loaded for this range.
///
/// Only answers for `Source::Dashboard`, which is what `can_compare` restricts
/// comparison candidates to. The chain-size, fee and distribution charts each
/// need something beyond the rows (a disk figure, a unit, a separate fetch),
/// so they take part in a comparison as the primary chart but never as the
/// overlaid one.
fn build_from_dashboard(
    data: &DashboardData,
    m: &ChartMeta,
) -> Option<serde_json::Value> {
    match (data, m.source) {
        (DashboardData::PerBlock(b), Source::Dashboard { per_block, .. }) => {
            Some(per_block(b))
        }
        (
            DashboardData::Daily(d),
            Source::Dashboard {
                daily: Daily::Fn(f),
                ..
            },
        ) => Some(f(d)),
        _ => None,
    }
}

/// When a peak or low happened, however the chart encodes it. Per-block charts
/// carry a millisecond timestamp on the point; daily charts emit bare numbers
/// and keep their dates on the category axis, so those are matched by position.
fn point_label(p: &kpi::Point, axis_labels: &[String]) -> Option<String> {
    fmt_ms(p.x).or_else(|| axis_labels.get(p.idx).cloned())
}

#[component]
pub fn SingleChartPage() -> impl IntoView {
    let params = use_params_map();
    let slug =
        Signal::derive(move || params.read().get("slug").unwrap_or_default());
    let meta = Signal::derive(move || registry::find(&slug.get()));

    view! {
        <Show
            when=move || meta.get().is_some()
            fallback=move || view! { <UnknownChart slug=slug/> }
        >
            {move || meta.get().map(|m| view! { <ChartView meta=m/> })}
        </Show>
    }
}

#[component]
fn UnknownChart(slug: Signal<String>) -> impl IntoView {
    // A real 404, not a 200 carrying an apology. Every registered slug is
    // an indexable path, so a soft 404 on the rest would invite search
    // engines to index "no such chart" pages for every typo and stale link. Same defect
    // as /blocks/{height} answering 200 with an error body, fixed in
    // fix/input-validation.
    #[cfg(feature = "ssr")]
    {
        if let Some(response) = use_context::<leptos_axum::ResponseOptions>() {
            response.set_status(axum::http::StatusCode::NOT_FOUND);
        }
    }
    // Rendering an empty chart frame for a bad slug would look like a broken
    // chart rather than a wrong URL, so say which it is and offer the way out.
    view! {
        <Title text="Chart not found | We Hodl BTC"/>
        <Meta name="robots" content="noindex"/>
        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-8 text-center">
            <h1 class="text-xl text-white font-semibold mb-2">"No such chart"</h1>
            <p class="text-sm text-white/70 mb-6">
                "There is no chart called \"" {move || slug.get()} "\"."
            </p>
            <div class="flex flex-wrap gap-2 justify-center">
                <a href="/observatory/charts/network" class="text-sm px-3 py-1.5 rounded-lg bg-white/5 text-white/85 hover:text-[#f7931a] transition-colors">"Network"</a>
                <a href="/observatory/charts/fees" class="text-sm px-3 py-1.5 rounded-lg bg-white/5 text-white/85 hover:text-[#f7931a] transition-colors">"Fees"</a>
                <a href="/observatory/charts/mining" class="text-sm px-3 py-1.5 rounded-lg bg-white/5 text-white/85 hover:text-[#f7931a] transition-colors">"Mining"</a>
                <a href="/observatory/charts/embedded" class="text-sm px-3 py-1.5 rounded-lg bg-white/5 text-white/85 hover:text-[#f7931a] transition-colors">"Embedded Data"</a>
            </div>
        </div>
    }
}

#[component]
fn ChartView(meta: &'static ChartMeta) -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;
    let data_loading = state.data_loading;

    // Local toggles for the two charts that have a count/percent switch and
    // the fee unit. Defaults match what the card shows.
    let (pct_mode, set_pct_mode) = signal(true);
    let (fee_sats, set_fee_sats) = signal(false);
    // Collapsing the rail is the lever that actually makes the chart fill a
    // wide monitor: more height alone still leaves a 16:9 plot. Only applies
    // from `lg` up, since below that the rail is stacked underneath and costs
    // the chart no width at all.
    let (rail_open, set_rail_open) = signal(true);
    // Both scales and the comparison live in the shared state, so a refresh
    // or a pasted link restores the view rather than resetting it to linear
    // with nothing laid over it. The single-chart page used to keep its own
    // metric-scale signal, which meant two toggles for one idea and neither
    // of them in the URL.
    let log_scale = state.overlay_log_scale;
    let set_log_scale = state.set_overlay_log_scale;
    let right_log_scale = state.overlay_right_log_scale;
    let set_right_log_scale = state.set_overlay_right_log_scale;
    let compare = state.compare;
    let set_compare = state.set_compare;
    // The stored slug is an intent, not a guarantee: it survives navigation
    // to another chart, and it can name this chart, a chart that cannot be
    // compared with this one, or one with nothing to draw at this range.
    // Resolving it here, on every render, through the registry's single
    // validator is what makes all of those a no-op instead of a broken chart.
    // Which resolution is actually in play, from the data rather than from
    // the range name. `range_to_blocks` maps "custom" to 999,999 so it always
    // reads as daily, while the data resource correctly fetches per-block
    // rows for a short custom window. Everything downstream that asked the
    // range string was therefore wrong for every custom range: a two-day
    // window on a chart with no daily builder built 281 valid points and then
    // covered them with an "unavailable" overlay.
    let resolved_daily = Signal::derive(move || match dashboard_data.get() {
        Some((_, Ok(DashboardData::Daily(_)))) => true,
        Some((_, Ok(DashboardData::PerBlock(_)))) => false,
        // Nothing resolved yet: fall back to what the range implies, so the
        // first paint is not wrong in the common case.
        _ => uses_daily_aggregates(range_to_blocks(&range.get())),
    });

    let compare_meta = Signal::derive(move || {
        // Price and chain size are applied first and take the right axis, so
        // a comparison cannot be drawn beside them. `apply_comparison`
        // already refuses on that ground, structurally; answering None here
        // as well is what keeps the picker, its description and the URL from
        // advertising a comparison the chart is not drawing. A link asking
        // for both used to do exactly that.
        //
        // Overlay wins rather than comparison, matching the precedence
        // already in force between price and chain size, which is the order
        // `apply_overlays` applies them in.
        // The toggles, not the fetched data. `OverlayFlags::has_right_axis`
        // reads the price and chain-size series, which arrive asynchronously,
        // so during SSR and the first client render they are empty and this
        // would resolve a comparison that is about to be displaced the moment
        // the fetch lands. The toggle is true immediately and is what the
        // reader actually asked for.
        if state.overlay_price.get() || state.overlay_chain_size.get() {
            return None;
        }
        registry::comparison_for(meta, &compare.get(), resolved_daily.get())
    });
    // One occupant at a time, the constraint the price and chain-size
    // toggles already enforce between themselves. A comparison joins that
    // group rather than claiming a third axis, because three value axes on
    // one plot is a chart nobody can read.
    let compare_holds_axis =
        Signal::derive(move || compare_meta.get().is_some());
    // Reconcile the stored slug with what can actually be drawn here.
    //
    // Rendering is already safe without this: `compare_meta` resolves to None
    // and nothing is laid over the chart. What this fixes is the picker still
    // showing a name, and the URL still carrying a slug, for a comparison
    // that is not on screen. Both of those read as the feature being broken
    // rather than as this chart not supporting that pairing.
    //
    // Every stale case at once, because they all arrive the same way, by
    // navigating or by pasting a link: the chart was renamed out of the
    // registry, it is this chart, it cannot be compared with this one, or it
    // has no daily builder and the range just grew past the threshold.
    Effect::new(move |_| {
        let slug = compare.get();
        if !slug.is_empty() && compare_meta.get().is_none() {
            set_compare.set(String::new());
        }
    });
    let has_right_axis = Signal::derive(move || {
        overlay_flags.get().has_right_axis() || compare_holds_axis.get()
    });

    let needs_mining = matches!(meta.source, Source::Mining(_));
    let which_buckets = match meta.source {
        Source::FullnessDist => Buckets::Fullness,
        Source::TimeDist => Buckets::Time,
        _ => Buckets::None,
    };
    let needs_buckets = !matches!(which_buckets, Buckets::None);

    // Both resources are created unconditionally so the component's shape does
    // not depend on the slug, but neither fetches unless this chart needs it.
    // Firing four mining requests to render a TPS chart would be a real cost.
    let mining_data = LocalResource::new(move || {
        let r = range.get();
        let custom_from = state.custom_from.get();
        let custom_to = state.custom_to.get();
        // Stamped, for the same reason `DashboardValue` is: this resource
        // keeps the previous window's payload while refetching, so gating the
        // option on the shared rows alone still let a mining chart paint the
        // previous range for a frame.
        let stamp = (r.clone(), custom_from.clone(), custom_to.clone());
        async move {
            let out = async move {
                if !needs_mining {
                    return Err(String::new());
                }
                let stats =
                    fetch_stats_summary().await.map_err(|e| e.to_string())?;
                // The mining queries take heights and the picker gives dates, and
                // the window has to be anchored at **both** ends.
                //
                // Converting the window's duration to a block count and counting
                // back from the tip gives a length, not a position: a request for
                // April 2024 came back holding the most recent 61 days, correctly
                // sized and entirely the wrong period. Silently answering a
                // historical question with recent data is the worst failure this
                // page can have, because nothing about the result looks wrong.
                //
                // So a custom window asks the chain where it sits. The named
                // ranges keep counting back from the tip, which is exactly what
                // they mean.
                let (from, to, from_ts, to_ts) =
                    super::helpers::resolve_window(
                        &r,
                        custom_from.clone(),
                        custom_to.clone(),
                        &stats,
                    )
                    .await?;
                // An empty window, so nothing is fetched at all. The endpoints
                // reject a reversed range, and a 500 would read as a server fault
                // rather than as a range holding no blocks.
                if from > to {
                    return Ok::<MiningPayload, String>((
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                    ));
                }
                let empty_monthly = fetch_empty_blocks_monthly(from, to)
                    .await
                    .map_err(|e| e.to_string())?;
                let empty_by_pool = fetch_empty_blocks_by_pool(from, to)
                    .await
                    .map_err(|e| e.to_string())?;
                // Which resolution, from the window that is actually being
                // fetched rather than from the range name. `range_to_blocks` maps
                // "custom" to 999,999, so this always read as daily even for a
                // two-day window.
                let span = to.saturating_sub(from) + 1;
                let miners = if uses_daily_aggregates(span) {
                    fetch_miner_dominance_daily(from_ts, to_ts)
                        .await
                        .map_err(|e| e.to_string())?
                } else {
                    fetch_miner_dominance(from, to)
                        .await
                        .map_err(|e| e.to_string())?
                };
                Ok::<MiningPayload, String>((
                    miners,
                    empty_monthly,
                    empty_by_pool,
                ))
            }
            .await;
            (stamp, out)
        }
    });

    // The two distribution charts have no daily builder. For long ranges they
    // read server-side histogram buckets instead, which is why they cannot be
    // expressed as a per-block/daily pair like the other 53.
    let buckets = LocalResource::new(move || {
        let r = range.get();
        // Read here so the resource re-runs when the dates change. A custom
        // window that only tracked `range` fetched from timestamp 0 to the
        // tip, which is all of history, and never refetched when the dates
        // were edited because `range` stayed "custom".
        let cf = state.custom_from.get();
        let ct = state.custom_to.get();
        let stamp = (r.clone(), cf.clone(), ct.clone());
        async move {
            let out = async move {
                let n = range_to_blocks(&r);
                if !needs_buckets || !uses_daily_aggregates(n) {
                    return None;
                }
                let stats = fetch_stats_summary().await.ok()?;
                let (_, _, from_ts, to_ts) = super::helpers::resolve_window(
                    &r,
                    cf.clone(),
                    ct.clone(),
                    &stats,
                )
                .await
                .ok()?;
                // Which histogram depends on the chart. Fetching only the
                // fullness one left time-dist with nothing to read, so its daily
                // arm returned empty and the page showed a loading state that
                // never resolved.
                match which_buckets {
                    Buckets::Fullness => {
                        fetch_fullness_histogram(from_ts, to_ts)
                            .await
                            .ok()
                            .map(Histogram::Fullness)
                    }
                    Buckets::Time => fetch_block_time_histogram(from_ts, to_ts)
                        .await
                        .ok()
                        .map(Histogram::Time),
                    Buckets::None => None,
                }
            }
            .await;
            (stamp, out)
        }
    });

    // A `Memo`, not `get_untracked`, and not a plain `.get()` either.
    //
    // Untracked was the bug: the live stats arrive after the first render, so
    // `chain_size_gb` was 0 when the option was built and the arrival never
    // triggered a rebuild. Chain Size then drew Block Data alone, and clicking
    // any unrelated toggle made a second series appear, because that
    // recomputation finally read the value. A scale control that adds data is
    // not a scale control.
    //
    // A plain `.get()` fixes that and rebuilds every chart on every live tick,
    // which is several times a minute for a value that changes when a block
    // arrives. `Memo` compares before notifying, so the rebuild happens when
    // the number actually moves, which is exactly when the chart should
    // change.
    let disk_size_gb = Memo::new(move |_| {
        state
            .cached_live
            .get()
            .map(|s| s.network.chain_size_gb)
            .unwrap_or(0.0)
    });

    let option = Signal::derive(move || {
        // The macro-driven charts return empty while a range refetches so the
        // previous range's rows are never serialized under the new key. Same
        // guard here, or this view flashes a chart built from stale rows.
        if data_loading.get() {
            return String::new();
        }
        let flags = overlay_flags.get();
        let pct = pct_mode.get();
        let sats = fee_sats.get();
        let logv = log_scale.get() && meta.supports_log();

        let cmp = compare_meta.get();

        // `cmp_option` is threaded in rather than read from a signal here so
        // the mining branch, which has no dashboard rows to build a second
        // series from, can pass None without the comparison code caring.
        let decorate = |mut v: serde_json::Value,
                        is_daily: bool,
                        cmp_option: Option<serde_json::Value>|
         -> String {
            if v.is_null() {
                return String::new();
            }
            crate::stats::charts::apply_overlays(&mut v, &flags, is_daily);
            // After the overlays, because `apply_overlays` sets grid.right to
            // a fixed width for its mark-line labels, and adding an axis has
            // to be the last thing that widens the plot or the axis ends up
            // drawn over them.
            if let (Some(other), Some(c)) = (cmp_option, cmp) {
                crate::stats::charts::apply_comparison(
                    &mut v,
                    &other,
                    c.title,
                    c.unit.label(),
                );
            }
            // After the overlays and the comparison, so both axes exist and
            // each is judged on the series it actually holds. The metric's
            // scale is this page's own signal, gated on `supports_log`; the
            // right axis belongs to whatever is occupying it, which is shared
            // state.
            //
            // One call rather than two, because the dropped-point notice has
            // to count both axes before either is sanitised. Calling the two
            // separately gave a notice that counted only the left, so turning
            // on the overlay's log axis dropped points silently.
            crate::stats::charts::apply_scales_with(
                &mut v,
                logv,
                flags.right_log_scale,
            );
            serde_json::to_string(&v).unwrap_or_default()
        };
        let finish = |v: serde_json::Value, is_daily: bool| -> String {
            decorate(v, is_daily, None)
        };

        match meta.source {
            Source::Mining(which) => {
                // The stamp has to match the window now selected, not just
                // be present: this resource holds the previous window's
                // payload while refetching, which is what kept a mining chart
                // painting the old range for a frame after the shared rows
                // had landed.
                let want = (
                    range.get(),
                    state.custom_from.get(),
                    state.custom_to.get(),
                );
                let Some((stamp, Ok((miners, monthly, by_pool)))) =
                    mining_data.get()
                else {
                    return String::new();
                };
                if stamp != want {
                    return String::new();
                }
                let v = match which {
                    MiningChart::Dominance => {
                        crate::stats::charts::miner_dominance_chart(&miners)
                    }
                    MiningChart::Diversity => {
                        crate::stats::charts::mining_diversity_chart(&miners)
                    }
                    MiningChart::EmptyBlocks => {
                        crate::stats::charts::empty_blocks_chart(&monthly)
                    }
                    MiningChart::EmptyByPool => {
                        crate::stats::charts::empty_blocks_by_pool_chart(
                            &by_pool,
                        )
                    }
                };
                // Only the empty-blocks histogram carries a time-ish axis the
                // annotations can land on; a donut has nothing to annotate.
                match which {
                    MiningChart::EmptyBlocks => finish(v, true),
                    _ => serde_json::to_string(&v).unwrap_or_default(),
                }
            }
            _ => {
                let Some((_, Ok(data))) = dashboard_data.get() else {
                    return String::new();
                };
                // Built from the same rows over the same range as the primary
                // chart, which is the whole reason `can_compare` is restricted
                // to dashboard-sourced charts: no second request, and the two
                // series cannot end up describing different periods.
                let cmp_option =
                    cmp.and_then(|c| build_from_dashboard(&data, c));
                let finish = |v: serde_json::Value, is_daily: bool| -> String {
                    decorate(v, is_daily, cmp_option.clone())
                };
                let disk_gb = disk_size_gb.get();
                let offset =
                    state.chain_size_offset.get().map(|(_, b)| b).unwrap_or(0);
                let chain_total = state.chain_size_total.get().unwrap_or(0);
                let unit = if sats { "sats" } else { "btc" };

                match (&data, meta.source) {
                    (
                        DashboardData::PerBlock(blocks),
                        Source::Dashboard { per_block, .. },
                    ) => finish(per_block(blocks), false),
                    (
                        DashboardData::Daily(days),
                        Source::Dashboard { daily, .. },
                    ) => match daily {
                        Daily::Fn(f) => finish(f(days), true),
                        Daily::Unavailable => String::new(),
                    },
                    (DashboardData::PerBlock(blocks), Source::ChainSize) => {
                        finish(
                            crate::stats::charts::chain_size_chart(
                                blocks,
                                disk_gb,
                                offset,
                                chain_total,
                            ),
                            false,
                        )
                    }
                    (DashboardData::Daily(days), Source::ChainSize) => finish(
                        crate::stats::charts::chain_size_chart_daily(
                            days,
                            disk_gb,
                            offset,
                            chain_total,
                        ),
                        true,
                    ),
                    (
                        DashboardData::PerBlock(blocks),
                        Source::DiffAdjustment,
                    ) => finish(
                        crate::stats::charts::difficulty_adjustment_chart(
                            blocks,
                        ),
                        false,
                    ),
                    // Daily needs the retarget blocks, since a day's mean
                    // difficulty is a blend wherever a retarget lands
                    // mid-day. Until they arrive, nothing is drawn rather
                    // than a chart with no bars, which would read as a window
                    // holding no retargets.
                    (DashboardData::Daily(days), Source::DiffAdjustment) => {
                        let Some((_, Ok(rows))) = state.retargets.get() else {
                            return String::new();
                        };
                        finish(
                            crate::stats::charts::difficulty_adjustment_chart_daily(
                                days, &rows,
                            ),
                            true,
                        )
                    }
                    (DashboardData::PerBlock(blocks), Source::Fees) => finish(
                        crate::stats::charts::fees_chart_unit(blocks, unit),
                        false,
                    ),
                    (DashboardData::Daily(days), Source::Fees) => finish(
                        crate::stats::charts::fees_chart_daily_unit(days, unit),
                        true,
                    ),
                    (DashboardData::PerBlock(blocks), Source::FullnessDist) => {
                        let v = if pct {
                            crate::stats::charts::block_fullness_distribution_pct_chart(blocks)
                        } else {
                            crate::stats::charts::block_fullness_distribution_chart(blocks)
                        };
                        serde_json::to_string(&v).unwrap_or_default()
                    }
                    (DashboardData::PerBlock(blocks), Source::TimeDist) => {
                        let v = if pct {
                            crate::stats::charts::block_time_distribution_pct_chart(blocks)
                        } else {
                            crate::stats::charts::block_time_distribution_chart(
                                blocks,
                            )
                        };
                        serde_json::to_string(&v).unwrap_or_default()
                    }
                    // Long ranges: the distributions come from server-side
                    // buckets rather than from the daily aggregates.
                    (DashboardData::Daily(_), Source::FullnessDist) => {
                        // Stamp must match the selected window, same as
                        // the mining arm above.
                        let want = (
                            range.get(),
                            state.custom_from.get(),
                            state.custom_to.get(),
                        );
                        let Some((stamp, Some(Histogram::Fullness(b)))) =
                            buckets.get()
                        else {
                            return String::new();
                        };
                        if stamp != want {
                            return String::new();
                        }
                        let v = if pct {
                            crate::stats::charts::block_fullness_histogram_from_buckets_pct(&b)
                        } else {
                            crate::stats::charts::block_fullness_histogram_from_buckets(&b)
                        };
                        serde_json::to_string(&v).unwrap_or_default()
                    }
                    (DashboardData::Daily(_), Source::TimeDist) => {
                        // Stamp must match the selected window, same as
                        // the mining arm above.
                        let want = (
                            range.get(),
                            state.custom_from.get(),
                            state.custom_to.get(),
                        );
                        let Some((stamp, Some(Histogram::Time(b)))) =
                            buckets.get()
                        else {
                            return String::new();
                        };
                        if stamp != want {
                            return String::new();
                        }
                        let v = if pct {
                            crate::stats::charts::block_time_histogram_from_buckets_pct(&b)
                        } else {
                            crate::stats::charts::block_time_histogram_from_buckets(&b)
                        };
                        serde_json::to_string(&v).unwrap_or_default()
                    }
                    // Mining is handled above; the compiler cannot see that.
                    (_, Source::Mining(_)) => String::new(),
                }
            }
        }
    });

    let kpis = Signal::derive(move || kpi::compute(&option.get(), meta.shape));
    // The same five numbers for the comparison. Seeing the shape of a second
    // series but not being able to read its peak is half a comparison, and
    // the reader would otherwise have to open the other chart to get them.
    // Read off axis 1, with the compared chart's own shape, since that
    // decides whether they are a series, bands or categories.
    let compare_kpis = Signal::derive(move || {
        compare_meta
            .get()
            .map(|c| (c, kpi::compute_axis(&option.get(), c.shape, 1)))
    });

    // A chart with no daily builder draws nothing at long ranges. Saying which
    // ranges it supports is the honest version of an empty frame.
    let daily_gap =
        Signal::derive(move || !meta.has_daily() && resolved_daily.get());

    // True when the dashboard fetch for the selected window came back an
    // error. Borrowed rather than cloned, for the reason `data_loading` is.
    let load_failed = Memo::new(move |_| {
        dashboard_data.with(|v| v.as_ref().is_some_and(|(_, r)| r.is_err()))
    });

    let cid = canvas_id(meta.slug);
    let title_tag = format!("{} | Bitcoin Chart | We Hodl BTC", meta.title);
    let desc_tag = meta_description(meta);

    // Self-referential canonical, absolute, and deliberately WITHOUT the query
    // string. Every category page already carries one; these 63 new indexable
    // URLs did not, and each of them accepts `?range=`, `?overlays=`,
    // `?compare=` and two custom-date parameters. Without this, every
    // combination a reader shares is a separate indexable URL of the same
    // page, which is the classic way a site dilutes 63 pages into thousands.
    let canonical =
        format!("https://www.wehodlbtc.com/observatory/chart/{}", meta.slug);

    view! {
        <Title text=title_tag/>
        <Meta name="description" content=desc_tag/>
        <Link rel="canonical" href=canonical/>

        <div class="mb-4 flex items-center gap-2 text-sm">
            <a
                href=meta.category.page_path()
                class="text-white/60 hover:text-[#f7931a] transition-colors"
            >
                {meta.category.label()} " charts"
            </a>
            <span class="text-white/40">"/"</span>
            <span class="text-white/80">{meta.title}</span>
        </div>

        <div class=move || if rail_open.get() {
            "grid grid-cols-1 lg:grid-cols-[1fr_17rem] gap-3 lg:gap-4 items-start"
        } else {
            "grid grid-cols-1 gap-3 lg:gap-4 items-start"
        }>
            // ── chart column ──────────────────────────────────────────
            <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-4 sm:p-5 lg:p-6 min-w-0">
                // Stacked below `lg`. Side by side, the range row will not
                // shrink below its twelve buttons, so it starved the title of
                // width and wrapped it to one word per line on a phone.
                <div class="flex flex-col gap-2 lg:flex-row lg:items-start lg:justify-between lg:gap-3 mb-3 lg:mb-4">
                    <div class="min-w-0">
                        // No info icon here: the one-liner is directly beneath it
                        // and the long-form About is below the chart, so a
                        // third copy of the same sentence in a bubble would
                        // teach a reader that the icons are noise.
                        <h1 class="text-lg sm:text-xl text-white font-semibold">{meta.title}</h1>
                        // The same one-liner the card shows, switching with
                        // the range the way chart_desc does on the pages, so
                        // the two views describe the metric identically.
                        <p class="text-sm text-white/75 mt-0.5">{move || {
                            if resolved_daily.get() {
                                meta.desc_daily
                            } else {
                                meta.desc_per_block
                            }
                        }}</p>
                    </div>
                    // Range sits in the chart header rather than the rail:
                    // it is one row of buttons and it was costing the chart
                    // 20rem of width to hold it in a column.
                    <div class="flex items-start gap-3 shrink-0">
                        {meta.supports_log().then(|| view! {
                            <ScaleSwitch
                                on=log_scale
                                set_on=set_log_scale
                                // Unlabelled while it is the only switch on
                                // the page, since there is nothing to tell it
                                // apart from.
                                axis_label=Signal::derive(move || if has_right_axis.get() {
                                    meta.unit.label().to_string()
                                } else {
                                    String::new()
                                })
                                title_log="Logarithmic value axis, at any range. Zero and negative points cannot be plotted on it; the chart says how many were left out"
                            />
                        })}
                        // Only while an overlay owns the right axis, because
                        // there is otherwise no second scale to switch. The
                        // label names the axis, since two identical
                        // Linear/Log pairs side by side would not say which
                        // one moves what.
                        <Show when=move || has_right_axis.get()>
                            <ScaleSwitch
                                on=right_log_scale
                                set_on=set_right_log_scale
                                axis_label=Signal::derive(move || right_axis_label(&overlay_flags.get(), compare_meta.get()))
                                title_log="Logarithmic axis for whatever is on the right: the price overlay, chain size, or a compared chart. Only positive values can be drawn on it"
                                // The switch beside it already explains the
                                // difference, and it is the same difference.
                                explain=false
                            />
                        </Show>
                        <RailRange/>
                        <button
                            class="hidden lg:inline-flex items-center text-white/60 hover:text-[#f7931a] transition-colors cursor-pointer p-1 rounded-md hover:bg-white/5 mt-0.5"
                            title=move || if rail_open.get() { "Hide the side panel and widen the chart" } else { "Show the side panel" }
                            on:click=move |_| set_rail_open.update(|v| *v = !*v)
                        >
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.8">
                                <path stroke-linecap="round" stroke-linejoin="round" d=move || if rail_open.get() {
                                    "M13 5l7 7-7 7M5 5l7 7-7 7"
                                } else {
                                    "M11 19l-7-7 7-7M19 19l-7-7 7-7"
                                }/>
                            </svg>
                        </button>
                    </div>
                </div>

                // Height comes from the viewport, not a fixed pixel value,
                // because the point of this view is that the chart dominates.
                // On a wide monitor a fixed 620px next to a 1600px-wide plot
                // is a letterbox; `100dvh` minus the page chrome fills the
                // screen instead and pushes the exports and prose below the
                // fold, which is where they belong.
                //
                // The clamps are what keep one layout across screen types
                // rather than three: a floor so a short laptop still gets a
                // readable plot, and a ceiling so a 4K panel does not stretch
                // one series over 1400px of height. Below `lg` the height is
                // a share of the viewport instead, since a phone in portrait
                // wants a squarer chart than a letterbox.
                //
                // `dvh` rather than `vh` so mobile browsers' collapsing
                // toolbars do not leave a gap. The chart re-fits on its own:
                // stats.js observes each canvas with a ResizeObserver and
                // calls `chart.resize()`.
                <div class="relative w-full h-[58dvh] min-h-[300px] max-h-[520px] sm:h-[62dvh] sm:max-h-[640px] lg:h-[calc(100dvh-20rem)] lg:min-h-[460px] lg:max-h-[920px]">
                    <Chart id=cid.clone() option=option class="w-full h-full".to_string()/>
                    <Show when=move || daily_gap.get()>
                        <div class="absolute inset-0 flex items-center justify-center bg-[#0d2137] rounded-xl px-6">
                            <div class="text-center max-w-sm">
                                <p class="text-white/85 text-sm mb-1">"Not available at this range"</p>
                                // Short, and matching the card's frame word for
                                // word. "1M" rather than "1m" because that is
                                // what the range button reads, and the whole
                                // point of the sentence is to name the button.
                                <p class="text-white/60 text-xs">
                                    "This chart is computed per block. Pick 1M or shorter."
                                </p>
                            </div>
                        </div>
                    </Show>
                    // A failed fetch is an error, not a slow one.
                    //
                    // This page had no error branch at all: the skeleton is
                    // shown whenever the option is empty, and a failed
                    // resource leaves it empty forever, so a 500 read as
                    // "Loading chart data..." until the reader gave up. The
                    // category pages have shown `DataLoadError` with a retry
                    // since before this page existed; it just was not carried
                    // over. Checked first so it wins over the skeleton.
                    <Show when=move || load_failed.get()>
                        <div class="absolute inset-0 flex items-center justify-center bg-[#0d2137] rounded-xl">
                            <DataLoadError on_retry=Callback::new(move |_| {
                                state.dashboard_data.refetch()
                            })/>
                        </div>
                    </Show>
                    <Show when=move || !load_failed.get() && !daily_gap.get() && (option.get().is_empty() || data_loading.get())>
                        <div class="absolute inset-0 flex items-center justify-center bg-[#0d2137] rounded-xl">
                            <span class="text-xs text-white/50">"Loading chart data..."</span>
                        </div>
                    </Show>
                </div>

                {matches!(meta.source, Source::FullnessDist | Source::TimeDist).then(|| view! {
                    <div class="mt-3 flex items-center gap-2">
                        <span class="text-xs text-white/60">"Show as"</span>
                        <button
                            class=move || if pct_mode.get() { "text-xs px-2 py-1 rounded-md bg-[#f7931a]/20 text-[#f7931a] cursor-pointer" } else { "text-xs px-2 py-1 rounded-md bg-white/5 text-white/70 cursor-pointer" }
                            on:click=move |_| set_pct_mode.set(true)
                        >"%"</button>
                        <button
                            class=move || if pct_mode.get() { "text-xs px-2 py-1 rounded-md bg-white/5 text-white/70 cursor-pointer" } else { "text-xs px-2 py-1 rounded-md bg-[#f7931a]/20 text-[#f7931a] cursor-pointer" }
                            on:click=move |_| set_pct_mode.set(false)
                        >"count"</button>
                    </div>
                })}

                {matches!(meta.source, Source::Fees).then(|| view! {
                    <div class="mt-3 flex items-center gap-2">
                        <span class="text-xs text-white/60">"Unit"</span>
                        <button
                            class=move || if fee_sats.get() { "text-xs px-2 py-1 rounded-md bg-white/5 text-white/70 cursor-pointer" } else { "text-xs px-2 py-1 rounded-md bg-[#f7931a]/20 text-[#f7931a] cursor-pointer" }
                            on:click=move |_| set_fee_sats.set(false)
                        >"BTC"</button>
                        <button
                            class=move || if fee_sats.get() { "text-xs px-2 py-1 rounded-md bg-[#f7931a]/20 text-[#f7931a] cursor-pointer" } else { "text-xs px-2 py-1 rounded-md bg-white/5 text-white/70 cursor-pointer" }
                            on:click=move |_| set_fee_sats.set(true)
                        >"sats"</button>
                    </div>
                })}
            </div>

            // ── right rail ────────────────────────────────────────────
            <aside class=move || if rail_open.get() {
                "space-y-4 lg:sticky lg:top-20"
            } else {
                "space-y-4 lg:hidden"
            }>
                <RailSection
                    title="Key facts"
                    explain="Calculated from the points currently plotted, so they follow the range you pick. Zooming with the slider changes only what you see, not these."
                >
                    // "Over selected range", not "visible": these follow the
                    // range slice, not the zoom slider, which changes only the
                    // view. Making them follow zoom needs a datazoom handler.
                    <p class="text-[0.7rem] uppercase tracking-widest text-white/70 mb-2">
                        "over selected range"
                    </p>
                    // The active unit, not the registry's. The fee chart
                    // switches between BTC and sats, and passing the static
                    // declaration printed "2.00M BTC" over a figure that was
                    // two million satoshis.
                    <KeyFacts
                        kpis=kpis
                        reports_change=meta.reports_change()
                        unit=Signal::derive(move || {
                            if matches!(meta.source, Source::Fees) && fee_sats.get() {
                                registry::Unit::Sats
                            } else {
                                meta.unit
                            }
                        })
                    />
                    // Under a rule and behind the comparison's own colour, so
                    // the two sets are never mistaken for one.
                    {move || compare_kpis.get().map(|(c, k)| {
                        // Read off axis 1 of the built option, so "no figures"
                        // means the line is genuinely not on the chart rather
                        // than that the rail failed to find it.
                        //
                        // The picker only offers pairings that can be drawn,
                        // which the conformance suite proves for every pair at
                        // both resolutions. What it cannot know is the
                        // reader's range: the fee spike detector plots nothing
                        // over a week with no spikes, so a valid choice still
                        // arrives empty. Saying so is the difference between a
                        // chart that explains itself and one that looks
                        // broken.
                        // A plain branch rather than `<Show>`: this closure
                        // already reruns whenever the option does, and `Show`
                        // wants children it can call repeatedly, which the
                        // owned `Kpis` here cannot give it.
                        let body = if matches!(k, kpi::Kpis::Loading) {
                            // Not a range problem yet: the compared chart's
                            // option is still in flight, and this block
                            // blamed the range throughout every load.
                            view! {
                                <p class="text-xs text-white/40">"Loading..."</p>
                            }.into_any()
                        } else if matches!(k, kpi::Kpis::Unavailable) {
                            view! {
                                <p class="text-xs text-white/55 leading-relaxed">
                                    "Nothing to draw over this range, so it is not on the chart. Try a longer one."
                                </p>
                            }.into_any()
                        } else {
                            view! {
                                <KeyFacts kpis=Signal::derive(move || k.clone()) unit=c.unit reports_change=c.reports_change()/>
                            }.into_any()
                        };
                        view! {
                            <div class="border-t border-white/10 mt-3 pt-3">
                                <p class="text-[0.7rem] uppercase tracking-widest text-white/70 mb-2 flex items-center gap-1.5">
                                    <span class="inline-block w-2 h-2 rounded-full bg-[#60a5fa] shrink-0"></span>
                                    {c.title}
                                </p>
                                {body}
                            </div>
                        }
                    })}
                </RailSection>

                <RailSection
                    title="Overlays"
                    explain="Extra context drawn on top of the chart. Event markers are vertical lines at dated moments; a comparison series is a second set of numbers on its own axis at the right."
                >
                    <OverlayToggles
                        meta=meta
                        compare=compare
                        set_compare=set_compare
                        compare_meta=compare_meta
                        compare_holds_axis=compare_holds_axis
                        daily=resolved_daily
                    />
                </RailSection>

                {
                    let rel = registry::related(meta, 4);
                    (!rel.is_empty()).then(|| view! {
                        <RailSection
                            title="Related"
                            explain="Charts measuring the same thing or covering the same part of the network, for reading this one in context."
                        >
                            // Chips below `lg`, where four full-width links
                            // are four rows of mostly empty line and each is
                            // a small tap target in a tall list. Back to a
                            // stacked list in the 15rem rail, where a title
                            // rarely fits on one chip.
                            <div class="flex flex-wrap gap-1.5 lg:block lg:space-y-1.5">
                                {rel.into_iter().map(|r| view! {
                                    <a
                                        href=format!("/observatory/chart/{}", r.slug)
                                        class="inline-block px-2.5 py-1.5 rounded-lg bg-white/5 text-sm text-white/85 hover:text-[#f7931a] hover:bg-white/10 transition-colors lg:block lg:px-0 lg:py-0 lg:bg-transparent lg:hover:bg-transparent"
                                    >
                                        {r.title}
                                    </a>
                                }).collect_view()}
                            </div>
                        </RailSection>
                    })
                }
            </aside>
        </div>

        // Exports and prose go full width under the chart, not in the rail:
        // downloading is what you do after reading, and a paragraph needs the
        // width to be readable.
        <div class=move || if rail_open.get() {
            "mt-3 lg:mt-4 space-y-3 lg:space-y-4 lg:pr-[17.75rem]"
        } else {
            "mt-3 lg:mt-4 space-y-3 lg:space-y-4"
        }>
            <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-4">
                <h2 class="text-[0.7rem] uppercase tracking-widest text-white/75 mb-3">
                    "Get this data"
                </h2>
                <div class="flex flex-wrap items-center gap-2">
                    <ExportButton
                        label="CSV"
                        hint="The plotted series as comma-separated values"
                        on_click={
                            let id = cid.clone();
                            move || download_chart_csv(&id, meta.title, &range.get_untracked())
                        }
                    />
                    <ExportButton
                        label="JSON"
                        hint="The plotted series as JSON, with block heights where the chart has them"
                        on_click={
                            let id = cid.clone();
                            move || download_chart_json(&id, meta.title, &range.get_untracked())
                        }
                    />
                    <ExportButton
                        label="PNG"
                        hint="This chart as an image"
                        on_click={
                            let id = cid.clone();
                            move || download_chart_png(&id, meta.title)
                        }
                    />
                    // Two different things, and one note beside three
                    // buttons read as though it covered all of them. CSV and
                    // JSON are the data; PNG is a picture of the chart as it
                    // currently looks, zoom and overlays included.
                    <span class="text-xs text-white/50 ml-1">
                        "CSV and JSON take the selected range without overlay series. PNG captures the chart as it looks now."
                    </span>
                </div>
            </div>

            // Definition then Technical, and both headed, because they answer
            // two different questions: what is this, and can I trust the
            // number. A reader wants one or the other, rarely both, so
            // running them together would make each group read past the one
            // they did not come for. Charts without the long copy fall back
            // to the one-liner rather than an empty heading.
            <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-4 lg:p-5">
                <h2 class="text-[0.7rem] uppercase tracking-widest text-white/75 mb-3">
                    "About this metric"
                </h2>
                {match meta.about {
                    Some(copy) => view! {
                        <div class="max-w-3xl space-y-3">
                            // Not every chart has the definition half yet, so
                            // the subtitle above carries the short answer and
                            // this shows what exists rather than an empty
                            // heading.
                            {copy.definition.map(|d| view! {
                                <p class="text-sm text-white/85 leading-relaxed">
                                    <span class="text-white/70">"Definition. "</span>
                                    {d}
                                </p>
                            })}
                            // Split on a blank line, so a method that covers
                            // several things reads as several paragraphs.
                            //
                            // These run long by design: the whole point of
                            // this section is to say exactly what was
                            // measured and what it excludes. Rendered as one
                            // blob, the interval chart's method ran to ten
                            // unbroken lines covering the per-block
                            // difference, the consensus timestamp rules and
                            // the daily arm's different quantity, which is
                            // three subjects a reader has to separate for
                            // themselves. Copy that does not ask for breaks
                            // still renders as exactly one paragraph.
                            {copy.technical.split("\n\n")
                                .enumerate()
                                .map(|(i, para)| view! {
                                    <p class="text-sm text-white/85 leading-relaxed">
                                        {(i == 0).then(|| view! {
                                            <span class="text-white/70">
                                                "How it is measured. "
                                            </span>
                                        })}
                                        {para.to_string()}
                                    </p>
                                })
                                .collect_view()}
                        </div>
                        <p class="text-xs text-white/55 mt-3">
                            "Measured from my own Bitcoin node. "
                            <a href="/observatory/learn/methodology" class="hover:text-[#f7931a] transition-colors">
                                "Methodology"
                            </a>
                        </p>
                    }.into_any(),
                    // No long copy yet, so there is no definition to show.
                    // This used to print `desc_per_block` again under a
                    // "Definition" heading, which restated the subtitle two
                    // lines above it and, in daily mode, restated the wrong
                    // resolution's subtitle. An absent definition is better
                    // left absent than filled with the sentence beside it.
                    None => view! {
                        <p class="text-xs text-white/55 mt-2">
                            "Measured from my own Bitcoin node. "
                            <a href="/observatory/learn/methodology" class="hover:text-[#f7931a] transition-colors">
                                "Methodology"
                            </a>
                        </p>
                    }.into_any(),
                }}
            </div>
        </div>
        // Without this the view is a dead end: four related charts and the
        // browser back button. The drawer indexes every registered chart.
        <super::shared::ChartDrawer/>
    }
}

/// A Linear/Log pair for one value axis.
///
/// Extracted because the page can show two of them, and two copies of the
/// markup would be two places for the active-state styling to drift apart.
/// The label is what distinguishes them, so it is a signal: it appears only
/// once there is a second switch to be confused with.
#[component]
fn ScaleSwitch(
    on: ReadSignal<bool>,
    set_on: WriteSignal<bool>,
    axis_label: Signal<String>,
    title_log: &'static str,
    /// Whether this switch carries the linear-versus-log explanation.
    ///
    /// The page can show two of these, and they are the same control applied
    /// to different axes, so the explanation is the same for both. Printing
    /// it twice, a few centimetres apart, teaches a reader that the icons
    /// repeat themselves.
    #[prop(default = true)]
    explain: bool,
) -> impl IntoView {
    let cls = move |active: bool| segmented_button(active);
    view! {
        <div class="flex items-center gap-1.5 mt-0.5">
            {explain.then(|| view! {
                <InfoTip text="Linear: equal distances are equal differences, so 0 to 100 takes the same height as 100 to 200. Logarithmic: equal distances are equal ratios, so each step up is a multiplication, and only positive values can be drawn. A series spanning several orders of magnitude, such as price or difficulty, shows its early years on a log axis and flattens into a line along the bottom on a linear one."/>
            })}
            <Show when=move || !axis_label.get().is_empty()>
                <span class="text-[11px] text-white/55 whitespace-nowrap">
                    {move || axis_label.get()}
                </span>
            </Show>
            <div class=SEGMENTED_GROUP>
                <button
                    class=move || cls(!on.get())
                    title="Linear value axis"
                    on:click=move |_| set_on.set(false)
                >
                    "Linear"
                </button>
                <button
                    class=move || cls(on.get())
                    title=title_log
                    on:click=move |_| set_on.set(true)
                >
                    "Log"
                </button>
            </div>
        </div>
    }
}

/// The tray a related set of buttons sits in, so two sets side by side read as
/// two controls rather than one long strip.
///
/// The chart header carries the axis scale and the range next to each other,
/// twelve small buttons in a row, and with only a gap between them "Log" and
/// "1D" looked like neighbours in the same group. The tray is what says where
/// one control ends.
const SEGMENTED_GROUP: &str =
    "flex flex-wrap items-center gap-0.5 p-0.5 rounded-lg \
     bg-black/25 border border-white/10";

/// One button inside a [`SEGMENTED_GROUP`].
///
/// Inactive buttons have no background of their own: the tray is the ground
/// they sit on, and giving them one as well produced the blocky strip this
/// replaced.
fn segmented_button(active: bool) -> &'static str {
    if active {
        "px-2 py-1 text-xs rounded-md bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
    } else {
        "px-2 py-1 text-xs rounded-md text-white/70 hover:text-white/95 hover:bg-white/10 transition-colors cursor-pointer"
    }
}

/// What the right axis is currently showing, for the scale switch beside it.
///
/// The label is the entire reason two Linear/Log pairs side by side are
/// readable, so any case that falls through to the generic word defeats the
/// control. A comparison is checked first because it is the occupant most
/// likely to sit next to a metric switch showing a similar-looking unit, and
/// because this function originally could not see one at all: it read only
/// `OverlayFlags`, which a comparison does not travel in.
///
/// Both overlays can be on at once, in which case they get an axis each and
/// only the first is switchable. "overlay" is honest there, since naming one
/// of two would be wrong half the time.
fn right_axis_label(
    flags: &OverlayFlags,
    compare: Option<&'static ChartMeta>,
) -> String {
    if let Some(c) = compare {
        return c.unit.label().to_string();
    }
    match (
        !flags.price_data.is_empty(),
        !flags.chain_size_data.is_empty(),
    ) {
        (true, false) => "price".to_string(),
        (false, true) => "chain size".to_string(),
        _ => "overlay".to_string(),
    }
}

/// The shared `RangeSelector` lays its twelve presets out in one non-wrapping
/// row with the mode label beside them, sized for the wide settings panel. In
/// a 20rem rail that overflows the panel entirely, so the rail gets its own
/// wrapping grid over the same signals rather than the shared component being
/// reshaped for both.
///
/// **The date inputs belong here, beside the Custom button that asks for
/// them.** Routing that button to the settings panel instead put the control
/// in another corner of the screen, on a tab labelled Axes, with its own
/// picker still collapsed.
///
/// Only the two inputs are duplicated, not the preset row: the row is what
/// overflows a narrow rail. Validation is shared with the panel's picker
/// through `validate_custom_range`, so two pickers cannot disagree about what
/// a valid window is.
#[component]
fn RailRange() -> impl IntoView {
    const PRESETS: &[&str] = &[
        "1d", "1w", "1m", "3m", "6m", "ytd", "1y", "2y", "5y", "10y", "all",
    ];
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let set_range = state.set_range;
    let set_custom_from = state.set_custom_from;
    let set_custom_to = state.set_custom_to;
    let custom_from = state.custom_from;
    let custom_to = state.custom_to;

    // Prefilled from the window in force, so opening this with a custom range
    // selected shows the dates it is showing rather than two empty inputs.
    let (picker_open, set_picker_open) =
        signal(range.get_untracked() == "custom");
    let (local_from, set_local_from) =
        signal(custom_from.get_untracked().unwrap_or_default());
    let (local_to, set_local_to) =
        signal(custom_to.get_untracked().unwrap_or_default());
    let (problem, set_problem) = signal::<Option<&'static str>>(None);

    let apply_custom = move |_| {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        match super::shared::validate_custom_range(
            &local_from.get(),
            &local_to.get(),
            &today,
        ) {
            Ok((from, to)) => {
                set_problem.set(None);
                set_custom_from.set(Some(from));
                set_custom_to.set(Some(to));
                set_range.set("custom".to_string());
            }
            Err(why) => set_problem.set(Some(why)),
        }
    };

    // Whether the selected range is served per block or from daily
    // aggregates. Worth surfacing because it changes what a point means, and
    // nine charts have no daily variant at all.
    let mode = move || {
        let r = range.get();
        if r == "custom" {
            "custom range".to_string()
        } else if uses_daily_aggregates(range_to_blocks(&r)) {
            "daily averages".to_string()
        } else {
            "per block".to_string()
        }
    };

    view! {
        // One column, so the mode line sits under the presets rather than
        // beside them. Both used to be emitted as bare siblings, which made
        // them two items of the header's own flex row: the mode line then
        // competed with the presets for width, and giving it `w-full` to get
        // the info icon inline claimed the entire row and wrapped "Custom"
        // onto a line of its own.
        <div class="flex flex-col items-start lg:items-end min-w-0">
        // **A select below `sm`, the tray above it.**
        //
        // Twelve buttons wrap to two rows on a phone, which costs more
        // vertical space than the chart can spare and reads as a control
        // panel rather than a range picker. The shared `RangeSelector` has
        // shipped exactly this split since it was written (`range.rs:119`);
        // this rail was built for the 20rem desktop column and never got the
        // mobile half, so the single-chart page was the one place on the site
        // still showing the full tray on a phone.
        //
        // `selected` as well as `prop:value`: the prop is applied after
        // hydration, so an SSR page would paint the control empty and read
        // 1D while the chart showed 1Y. AGENTS.md records the trap.
        <select
            aria-label="Time range"
            class="sm:hidden w-full bg-[#0a1a2e] text-white/85 text-sm border \
                   border-white/10 rounded-lg px-2.5 py-2 cursor-pointer \
                   focus:outline-none focus:border-[#f7931a]/40 \
                   [color-scheme:dark]"
            prop:value=move || range.get()
            on:change=move |ev| {
                let v = event_target_value(&ev);
                if v == "custom" {
                    set_picker_open.set(true);
                } else {
                    set_custom_from.set(None);
                    set_custom_to.set(None);
                    set_picker_open.set(false);
                    set_problem.set(None);
                    set_range.set(v);
                }
            }
        >
            {PRESETS.iter().map(|r| {
                let val = r.to_string();
                let label = r.to_uppercase();
                let mine = val.clone();
                view! {
                    <option value=val selected=move || range.get() == mine>
                        {label}
                    </option>
                }
            }).collect_view()}
            <option
                value="custom"
                selected=move || range.get() == "custom"
            >"Custom"</option>
        </select>
        <div class=format!("hidden sm:flex {SEGMENTED_GROUP} justify-start lg:justify-end")>
            {PRESETS.iter().map(|r| {
                let val = r.to_string();
                let label = r.to_uppercase();
                let is_on = {
                    let val = val.clone();
                    move || range.get() == val
                };
                view! {
                    <button
                        class=move || segmented_button(is_on())
                        on:click={
                            let val = val.clone();
                            move |_| {
                                set_custom_from.set(None);
                                set_custom_to.set(None);
                                set_picker_open.set(false);
                                set_problem.set(None);
                                set_range.set(val.clone());
                            }
                        }
                    >
                        {label}
                    </button>
                }
            }).collect_view()}
            // In the same tray as the presets, with a rule in front of it:
            // it is a range like the others, but it opens a panel rather than
            // selecting one.
            <span class="w-px self-stretch bg-white/10 mx-0.5"></span>
            <button
                class=move || segmented_button(range.get() == "custom")
                on:click=move |_| set_picker_open.update(|v| *v = !*v)
                title="Pick an exact date range"
            >
                "Custom"
            </button>
        </div>
        // The dates, under the presets and in the same column, so they appear
        // where the button that asks for them is. Wraps, because the header
        // is narrow on a phone and two inputs plus a button do not fit on one
        // line there.
        <Show when=move || picker_open.get()>
            <div class="flex flex-wrap items-center gap-1.5 mt-1.5 p-1 rounded-lg bg-black/25 border border-white/10">
                <input
                    type="date"
                    min="2009-01-03"
                    max=move || chrono::Utc::now().format("%Y-%m-%d").to_string()
                    aria-label="Range start date"
                    class="bg-[#0d2137] text-white text-xs border border-white/10 rounded-md px-1.5 py-1 focus:outline-none focus:border-[#f7931a]/40"
                    style="color-scheme: dark"
                    prop:value=move || local_from.get()
                    on:input=move |ev| set_local_from.set(event_target_value(&ev))
                />
                <span class="text-white/30 text-xs">"to"</span>
                <input
                    type="date"
                    min="2009-01-03"
                    max=move || chrono::Utc::now().format("%Y-%m-%d").to_string()
                    aria-label="Range end date"
                    class="bg-[#0d2137] text-white text-xs border border-white/10 rounded-md px-1.5 py-1 focus:outline-none focus:border-[#f7931a]/40"
                    style="color-scheme: dark"
                    prop:value=move || local_to.get()
                    on:input=move |ev| set_local_to.set(event_target_value(&ev))
                />
                <button
                    class="px-2.5 py-1 text-xs bg-[#f7931a] text-[#1a1a2e] font-semibold rounded-md cursor-pointer hover:bg-[#f4a949] transition-colors"
                    on:click=apply_custom
                >
                    "Go"
                </button>
            </div>
        </Show>
        // Why nothing happened, when nothing happened.
        <Show when=move || problem.get().is_some()>
            <p class="text-[0.7rem] text-[#f7931a] mt-1">{move || problem.get().unwrap_or_default()}</p>
        </Show>
        <p class="text-[0.7rem] text-white/65 mt-1.5 inline-flex items-center gap-1">
            <span class="whitespace-nowrap">
                <DefinedTerm
                    label=Signal::derive(mode)
                    text="Short ranges plot one point per block, about one every ten minutes. Longer ranges plot one point per day, computed from every block in that day. How it is computed depends on the chart: some average, some total the day up, some are a ratio of the day's sums, and each chart says which. Either way a brief spike inside a day is smoothed away."
                />
            </span>
        </p>
        </div>
    }
}

#[component]
fn ExportButton(
    label: &'static str,
    hint: &'static str,
    on_click: impl Fn() + 'static,
) -> impl IntoView {
    view! {
        <button
            class="text-xs px-3 py-1.5 rounded-lg bg-white/5 text-white/85 hover:text-[#f7931a] hover:bg-white/10 transition-colors cursor-pointer inline-flex items-center gap-1.5"
            title=hint
            on:click=move |_| on_click()
        >
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.8">
                <path stroke-linecap="round" stroke-linejoin="round" d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3"/>
            </svg>
            {label}
        </button>
    }
}

#[component]
fn RailSection(
    title: &'static str,
    /// What this section is for, in a sentence. Empty means no icon.
    #[prop(default = "")]
    explain: &'static str,
    children: Children,
) -> impl IntoView {
    // A native `<details>`, open by default, rather than a signal and a
    // click handler. On a phone the rail stacks under the chart, so three
    // always-open cards mean a long scroll past things the reader may not
    // want; being able to collapse them is the fix. Open by default keeps the
    // desktop rail exactly as it was, and native `<details>` brings its own
    // keyboard handling and renders identically under SSR, where a signal
    // seeded from a media query would not.
    view! {
        <details open class="group/sec bg-[#0d2137] border border-white/10 rounded-2xl p-4">
            // `list-none` kills the marker in Firefox and Chrome;
            // `::-webkit-details-marker` is still needed for Safari, which
            // would otherwise draw a triangle beside our own chevron.
            <summary class="text-[0.7rem] uppercase tracking-widest text-white font-semibold flex items-center gap-1 cursor-pointer list-none [&::-webkit-details-marker]:hidden lg:cursor-default">
                {title}
                {(!explain.is_empty()).then(|| view! { <InfoTip text=explain/> })}
                // The chevron is the only affordance saying this collapses,
                // so it is hidden where collapsing is pointless.
                <svg
                    class="w-3.5 h-3.5 ml-auto text-white/55 transition-transform group-open/sec:rotate-180 lg:hidden"
                    fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2"
                >
                    <path stroke-linecap="round" stroke-linejoin="round" d="m19.5 8.25-7.5 7.5-7.5-7.5"/>
                </svg>
            </summary>
            <div class="mt-3">{children()}</div>
        </details>
    }
}

/// The explanation bubble, shared by both triggers.
///
/// Opens downward: upward overflow at the top of the page goes behind the
/// navbar and then off the document, where it cannot be scrolled to. Anchored
/// right from `lg` up, where these sit near the edge of a 17rem rail, and
/// left below it, where the rail is full width and the trigger follows a
/// label near the left.
///
/// Resets `whitespace`, `text-transform` and the rest because it is placed
/// inside arbitrary copy and inherits whatever that copy set. A label with
/// `truncate` on it once made the bubble refuse to wrap.
const TIP_BUBBLE: &str = "pointer-events-none absolute top-full left-0 lg:left-auto lg:right-0 mt-1.5 w-56 max-w-[calc(100vw-3rem)] z-40 opacity-0 invisible group-hover/tip:opacity-100 group-hover/tip:visible group-focus-within/tip:opacity-100 group-focus-within/tip:visible group-active/tip:opacity-100 group-active/tip:visible transition-opacity duration-150 bg-[#06131f] border border-white/15 rounded-lg px-2.5 py-2 text-[0.7rem] leading-relaxed text-white font-normal tracking-normal normal-case whitespace-normal text-left shadow-lg shadow-black/50";

/// A term that explains itself when hovered, focused or tapped.
///
/// The dotted underline is the affordance, and it replaced an "i" icon on
/// every row. Eleven icons in a 17rem rail competed with the numbers the rail
/// exists to show, and an icon beside every single item teaches people to
/// ignore all of them. A dotted underline under the word being explained is
/// the long-standing convention for "definition available" and puts the
/// affordance on the term itself.
///
/// A `button` rather than a `span`, so it is reachable by keyboard and
/// tappable on a touch screen where there is no hover.
#[component]
fn DefinedTerm(
    #[prop(into)] label: Signal<String>,
    text: &'static str,
) -> impl IntoView {
    view! {
        <span class="relative inline-flex group/tip">
            <button
                type="button"
                class="text-left decoration-dotted decoration-white/40 underline underline-offset-2 hover:decoration-[#f7931a] focus:decoration-[#f7931a] focus:outline-none cursor-help transition-colors"
                aria-label=move || format!("{}: {text}", label.get())
            >
                {move || label.get()}
            </button>
            <span class=TIP_BUBBLE>{text}</span>
        </span>
    }
}

/// An "i" that explains a term on hover, on keyboard focus and on tap.
///
/// The site's purpose is teaching, so a reader who does not know what
/// "observations" or "BIP activations" means has to be able to find out
/// without leaving the chart. That rules out the native `title` attribute,
/// which never appears on a touch screen and waits a second on a desktop.
///
/// No JavaScript: a `<button>` inside a `group` shows the bubble on
/// `group-hover`, on `group-focus-within` and on `group-active`. Three
/// triggers rather than two because Safari does not reliably move focus to a
/// button on click, which would leave a touch user with no way to open this
/// at all; `:active` at least shows it while the finger is down.
/// `type="button"` matters, since several of these sit inside a `<label>`.
#[component]
fn InfoTip(text: &'static str) -> impl IntoView {
    view! {
        <span class="relative inline-flex group/tip align-middle shrink-0">
            <button
                type="button"
                class="w-3.5 h-3.5 rounded-full border border-white/45 text-white/80 hover:text-[#f7931a] hover:border-[#f7931a]/60 focus:text-[#f7931a] focus:border-[#f7931a]/60 focus:outline-none text-[0.6rem] leading-none flex items-center justify-center cursor-help transition-colors normal-case"
                aria-label=format!("What this means: {text}")
            >
                "i"
            </button>
            // Opens **downward**. An upward bubble reads better next to a
            // control at the bottom of a card, and it is wrong everywhere it
            // matters: the scale switch and the range mode sit in the chart
            // header near the top of the page, where opening upward put the
            // text behind the navbar and the advisory banner and then off the
            // top of the window entirely, which cannot be scrolled to.
            // Downward overflow is always reachable, because the document
            // continues below.
            //
            // Right-anchored from `lg` up, where these sit near the right
            // edge of a 15rem rail, and left-anchored below it, where the
            // rail is full width and the icons follow labels near the left.
            // The max-width is the backstop for both.
            //
            // `z-40` clears the chart canvas and the rail cards. It stays
            // under the navbar's `z-30` stacking context rather than fighting
            // it, which is safe now that nothing opens upward into it.
            <span class=TIP_BUBBLE>{text}</span>
        </span>
    }
}

/// The page `<meta name="description">` for a chart.
///
/// The unit clause is dropped for `Unit::Count`, whose label is the word
/// "count", because a sentence ending "measured in count" is generated from a
/// fallback rather than describing the chart. Every other unit names
/// something. Extracted from the component so it can be asserted: a phrase
/// search cannot guard a sentence that exists only in the output.
fn meta_description(meta: &ChartMeta) -> String {
    // `Mixed` and `Count` are not units a sentence can name. Weekday Activity
    // is the one `Mixed` chart, and it shipped the meta description
    // "... measured in mixed, from my own node", which is what a search
    // result showed.
    if meta.unit == registry::Unit::Count || meta.unit == registry::Unit::Mixed
    {
        format!(
            "{} for the Bitcoin network, from my own node. Downloadable as \
             CSV.",
            meta.title
        )
    } else {
        format!(
            "{} for the Bitcoin network, measured in {}, from my own node. \
             Downloadable as CSV.",
            meta.title,
            meta.unit.label()
        )
    }
}

/// What each key figure means, in a sentence a reader new to this can use.
///
/// Here rather than beside the calculation in `kpi.rs` because these explain
/// the rendered label, and the label is chosen here: `kpi.rs` returns
/// `observations` for four different shapes of chart and this file decides
/// what to call it in each. Keeping the words with the label they explain is
/// what stops the two drifting.
fn fact_hint(label: &str) -> &'static str {
    match label {
        "average" => "The mean of every plotted point over the selected range. Not a running average: change the range and this changes with it.",
        "peak" => "The highest single value in the range, and when it happened.",
        "low" => "The lowest single value in the range, and when it happened.",
        "change" => "Last value minus first value over the range. The percentage is shown only when the starting value is large enough for it to mean something.",
        "observations" => "How many data points are plotted. One per block on short ranges, one per day once the range is long enough to use daily aggregates.",
        "latest total" => "The sum of every band at the most recent point in the range.",
        "largest band" => "The band holding the biggest share at the most recent point.",
        "bands" => "How many stacked categories the chart is divided into.",
        "largest" => "The category with the biggest share over the whole range.",
        "its value" => "The value behind that share, in the chart's own unit.",
        "entries" => "How many categories the chart counts.",
        _ => "",
    }
}

#[component]
fn KeyFacts(
    kpis: Signal<Kpis>,
    /// The unit currently plotted, which is not always the registry's: the
    /// fee chart switches between BTC and sats at the reader's request.
    #[prop(into)]
    unit: Signal<registry::Unit>,
    /// Whether "change" means anything for this chart. False where every
    /// point is already a difference, so `last - first` is a change of a
    /// change: see `registry::POINTS_ARE_CHANGES`. The other four figures are
    /// still right, so the tile goes rather than the rail.
    reports_change: bool,
) -> impl IntoView {
    view! {
        {move || match kpis.get() {
            Kpis::Series { average, peak, low, first, last, observations, axis_labels } => {
                let change = last - first;
                // A relative change is only meaningful against a baseline of
                // comparable size. Difficulty over ALL starts at 1 and ends at
                // 1.6e14, which the old guard let through as
                // "+12745078971584212.0%" because it only skipped an exact
                // zero. Past four orders of magnitude the percentage says
                // nothing the absolute change does not say better.
                let pct = (first != 0.0
                    && (last / first).abs() < PCT_MEANINGFUL_RATIO)
                    .then(|| change / first.abs() * 100.0);
                view! {
                    // Two columns of tiles below `lg`, where the rail is
                    // stacked under the chart at full width and a column of
                    // label-value rows leaves most of the line empty. Back to
                    // rows in the 15rem rail, where two columns would not fit.
                    // One column on a phone, two on a tablet, one again in
                    // the desktop rail.
                    //
                    // Two columns at 390px gave each pair about 170px, and
                    // half these rows carry a date under the value, so the
                    // rows came out ragged with an orphan on the last line.
                    // A label-value row across the full width reads in one
                    // pass. The two-column form still earns its place from
                    // `sm` to `lg`, where the width is there and a single
                    // column would leave most of the line empty.
                    <div class="grid grid-cols-1 gap-y-2 sm:grid-cols-2 sm:gap-x-4 lg:grid-cols-1 lg:gap-0 lg:space-y-2">
                        <Fact label="average" value=unit.get().qualify(&fmt_num(average)) note=None/>
                        <Fact label="peak" value=fmt_num(peak.y) note=point_label(&peak, &axis_labels)/>
                        <Fact label="low" value=fmt_num(low.y) note=point_label(&low, &axis_labels)/>
                        {reports_change.then(|| view! {
                            <Fact
                                label="change"
                                value=format!("{}{}", if change >= 0.0 { "+" } else { "" }, fmt_num(change))
                                note=pct.map(|p| format!("{p:+.1}%"))
                            />
                        })}
                        <Fact label="observations" value=observations.to_string() note=None/>
                    </div>
                }.into_any()
            }
            Kpis::Bands { total_latest, dominant, dominant_share_pct, band_count, observations } => {
                view! {
                    // Two columns of tiles below `lg`, where the rail is
                    // stacked under the chart at full width and a column of
                    // label-value rows leaves most of the line empty. Back to
                    // rows in the 15rem rail, where two columns would not fit.
                    // Same responsive split as the time-series rail above.
                    <div class="grid grid-cols-1 gap-y-2 sm:grid-cols-2 sm:gap-x-4 lg:grid-cols-1 lg:gap-0 lg:space-y-2">
                        <Fact label="latest total" value=fmt_num(total_latest) note=None/>
                        <Fact label="largest band" value=dominant note=Some(format!("{dominant_share_pct:.1}% of total"))/>
                        <Fact label="bands" value=band_count.to_string() note=None/>
                        <Fact label="observations" value=observations.to_string() note=None/>
                    </div>
                }.into_any()
            }
            Kpis::Categorical { top_name, top_value, top_share_pct, entries } => {
                view! {
                    // Two columns of tiles below `lg`, where the rail is
                    // stacked under the chart at full width and a column of
                    // label-value rows leaves most of the line empty. Back to
                    // rows in the 15rem rail, where two columns would not fit.
                    // Same responsive split as the two rails above.
                    <div class="grid grid-cols-1 gap-y-2 sm:grid-cols-2 sm:gap-x-4 lg:grid-cols-1 lg:gap-0 lg:space-y-2">
                        <Fact label="largest" value=top_name note=Some(format!("{top_share_pct:.1}% of total"))/>
                        <Fact label="its value" value=fmt_num(top_value) note=None/>
                        <Fact label="entries" value=entries.to_string() note=None/>
                    </div>
                }.into_any()
            }
            // A zero here would be a claim about the data. This is not.
            Kpis::Unavailable => view! {
                <p class="text-sm text-white/60">"Not available for this range."</p>
            }.into_any(),
            // And neither is this one, yet. The chart area beside the rail
            // shows its skeleton while the rows are in flight, and the rail
            // used to assert "Not available for this range" throughout.
            Kpis::Loading => view! {
                <p class="text-sm text-white/40">"Loading..."</p>
            }.into_any(),
            // There is data; it just has no single summary. Worded without a
            // reason because there are two: Batching plots several
            // measurements at one x, and Mining Diversity plots a single
            // gauge value where an average, a peak and a low are the same
            // number three times. Naming the first was false for the second.
            //
            // Not a range problem either way, which is what the shared
            // "Not available for this range" got wrong.
            Kpis::NotSummarizable => view! {
                <p class="text-sm text-white/60">
                    "A single average, peak or change is not meaningful for this chart."
                </p>
            }.into_any(),
        }}
    }
}

#[component]
fn Fact(
    label: &'static str,
    #[prop(into)] value: String,
    /// Not `#[prop(optional)]`: that makes the builder take a `String` and
    /// wrap it, so an `Option` cannot be passed through. Every call site here
    /// has an explicit answer for whether there is a note.
    note: Option<String>,
) -> impl IntoView {
    let hint = fact_hint(label);
    view! {
        <div class="flex items-baseline justify-between gap-3">
            <span class="text-sm text-white/90 shrink-0">
                {if hint.is_empty() {
                    view! { {label} }.into_any()
                } else {
                    view! { <DefinedTerm label=label text=hint/> }.into_any()
                }}
            </span>
            <span class="text-right min-w-0">
                <span class="text-sm text-white font-mono">{value}</span>
                {note.map(|n| view! {
                    <span class="block text-xs text-white/75 font-mono">{n}</span>
                })}
            </span>
        </div>
    }
}

/// The overlays already exist and were effectively hidden behind a floating
/// panel. The rail is the fix asked for: they are simply always on screen.
///
/// Split into two groups because they are two different things wearing one
/// name. Event markers are vertical lines drawn against the chart's own axis.
/// Comparison series are extra data with their own scale, so they claim the
/// right axis, and that axis has exactly one occupant: picking one greys the
/// other, which is the same constraint the compare feature will contend with.
#[component]
fn OverlayToggles(
    meta: &'static ChartMeta,
    compare: ReadSignal<String>,
    set_compare: WriteSignal<String>,
    /// The comparison actually being drawn, resolved by the page.
    ///
    /// Passed in rather than resolved again here. A second reading drifted
    /// from the first the moment axis contention entered the rules, and the
    /// result was a picker describing a comparison that was not on screen.
    compare_meta: Signal<Option<&'static ChartMeta>>,
    compare_holds_axis: Signal<bool>,
    /// Whether the selected range is long enough to use daily aggregates, so
    /// the picker can drop charts that have no daily variant instead of
    /// offering one that would draw nothing.
    daily: Signal<bool>,
) -> impl IntoView {
    let s = expect_context::<ObservatoryState>();
    let price_holds_axis = Signal::derive(move || {
        s.overlay_price.get() || compare_holds_axis.get()
    });
    let size_holds_axis = Signal::derive(move || {
        s.overlay_chain_size.get() || compare_holds_axis.get()
    });
    // The picker is inert while price or chain size holds the axis, the same
    // way each of those is inert while the other does.
    let overlay_holds_axis = Signal::derive(move || {
        s.overlay_price.get() || s.overlay_chain_size.get()
    });
    let groups =
        Signal::derive(move || registry::comparable_with(meta, daily.get()));
    let never = Signal::derive(|| false);
    view! {
        <p class="text-[0.7rem] uppercase tracking-widest text-white/50 mb-1.5">
            "Event markers"
        </p>
        <div class="space-y-1.5">
            <Toggle
                label="Halvings"
                get=s.overlay_halvings set=s.set_overlay_halvings disabled=never
                hint="Every 210,000 blocks, roughly every four years, the block subsidy halves. Fees are the other part of what a miner collects and do not halve with it. Four halvings have happened so far."
            />
            <Toggle
                label="BIP activations"
                get=s.overlay_bips set=s.set_overlay_bips disabled=never
                hint="A BIP is a Bitcoin Improvement Proposal: a design document for a change to the protocol. These lines mark the dates that proposals such as SegWit and Taproot became active on the network."
            />
            <Toggle
                label="Core releases"
                get=s.overlay_core set=s.set_overlay_core disabled=never
                hint="Releases of Bitcoin Core, the software most nodes run. A release does not change consensus rules by itself, but it often changes what transactions a node will relay, which can show up in these charts."
            />
            <Toggle
                label="Events"
                get=s.overlay_events set=s.set_overlay_events disabled=never
                hint="Dated moments outside the protocol that moved the numbers: exchange failures, country-level bans, the first Ordinals inscriptions."
            />
        </div>
        <p class="text-[0.7rem] uppercase tracking-widest text-white/50 mt-3 mb-1.5">
            "Comparison series"
        </p>
        <div class="space-y-1.5">
            // Also disabled when the window is shorter than one price sample,
            // which is a different refusal from the axis being taken and needs
            // its own reason: at 1D the overlay was accepted, added its legend
            // entry, and drew no line at all.
            <Toggle
                label="Price (USD)"
                get=s.overlay_price
                set=s.set_overlay_price
                disabled=Signal::derive(move || size_holds_axis.get() || s.price_sparse.get())
                hint="The BTC price in US dollars, drawn on its own axis on the right. Sampled every 4 days, which is all the source provides for the full history, so a range shorter than that cannot draw it. Price crosses six orders of magnitude, so it usually wants the logarithmic setting beside it."
            />
            <Toggle
                label="Chain size"
                get=s.overlay_chain_size
                set=s.set_overlay_chain_size
                // Also refused on a chart that already plots chain size:
                // laying it over itself drew the same values twice on two
                // axes, which fit their own bounds, so identical numbers
                // landed at different heights. `apply_overlays` refuses to
                // draw it; this stops the control claiming otherwise.
                disabled=Signal::derive(move || {
                    price_holds_axis.get()
                        || meta.unit == registry::Unit::Gigabytes
                })
                disabled_reason=if meta.unit == registry::Unit::Gigabytes {
                    "This chart already plots chain size"
                } else {
                    "The right axis is taken by the other series"
                }
                hint="The total size of the block chain on disk, growing as blocks are added. An archival node keeps every byte of it; a pruned one verifies the same blocks and then discards the old ones."
            />
        </div>
        // A select rather than a list of toggles: 50-odd candidates will not
        // fit in a 15rem rail, and the native control brings its own keyboard
        // handling, type-ahead and mobile picker for nothing.
        <Show when=move || !groups.get().is_empty()>
            <p class="text-[0.7rem] uppercase tracking-widest text-white/50 mt-3 mb-1.5">
                "Compare with"
            </p>
            <select
                // `color-scheme: dark` is what actually fixes the popup. The
                // list of options is drawn by the browser, not by our CSS, so
                // it ignored the dark theme and rendered white; the options
                // then inherited the select's own white text and the whole
                // list was invisible. The explicit colours below are the
                // belt to that brace, since a few platforms honour one and
                // not the other.
                class=move || if overlay_holds_axis.get() {
                    "w-full text-sm bg-white/5 border border-white/10 rounded-md px-2 py-1.5 text-white/50 cursor-not-allowed [color-scheme:dark]"
                } else {
                    "w-full text-sm bg-white/5 border border-white/10 rounded-md px-2 py-1.5 text-white/85 hover:border-white/25 cursor-pointer [color-scheme:dark]"
                }
                prop:disabled=move || overlay_holds_axis.get()
                prop:value=move || compare.get()
                title=move || if overlay_holds_axis.get() {
                    "The right axis is taken by a comparison series"
                } else {
                    "Lay a second chart over this one, on its own axis"
                }
                on:change=move |ev| set_compare.set(event_target_value(&ev))
            >
                // `selected` as well as `prop:value`, because the prop is
                // only applied once WASM has hydrated. Without it a link
                // carrying a comparison paints a chart with two series beside
                // a picker reading "None" until hydration catches up.
                <option
                    class="bg-[#0d2137] text-white"
                    value=""
                    selected=move || compare.get().is_empty()
                >"None"</option>
                <For
                    each=move || groups.get()
                    key=|(cat, members)| (cat.label(), members.len())
                    let:group
                >
                    <optgroup class="bg-[#0d2137] text-white/70" label=group.0.label()>
                        {group.1.iter().map(|c| {
                            // Copied out of the borrow: these are `&'static`
                            // already, and the `selected` closure outlives
                            // the iteration that produced them.
                            let (slug, title) = (c.slug, c.title);
                            view! {
                                <option
                                    class="bg-[#0d2137] text-white"
                                    value=slug
                                    selected=move || compare.get() == slug
                                >{title}</option>
                            }
                        }).collect_view()}
                    </optgroup>
                </For>
            </select>
        </Show>
        // The line under the picker was already spent on a constraint note,
        // so saying what the chosen chart measures costs no space and no new
        // control. It is also the moment the reader most needs it: the legend
        // gives them a name and nothing else, and "UTXO Flow" means nothing
        // until something says what it counts.
        //
        // The constraint note takes the line back when nothing is picked,
        // since that is when it applies.
        {move || match compare_meta.get() {
            Some(c) => view! {
                <p class="text-xs text-white/65 mt-1.5 leading-relaxed">
                    <span class="inline-block w-2 h-2 rounded-full bg-[#60a5fa] mr-1 align-middle"></span>
                    // The same one-liner that chart's own page shows, and it
                    // switches with the range the way that page's does, so a
                    // reader who follows the link is not met with different
                    // words for the same thing.
                    {move || if daily.get() { c.desc_daily } else { c.desc_per_block }}
                    ". "
                    // Clears the comparison on the way out. You are asking
                    // to look at the chart you were comparing against, and it
                    // cannot be compared with itself, so carrying the slug
                    // across is carrying a value that is invalid on arrival.
                    //
                    // Clearing it here rather than letting the reconciling
                    // effect do it after the fact matters, because the URL
                    // writer runs on every state change and would otherwise
                    // stamp `?compare=<this chart>` onto the new page before
                    // the effect cleared it again.
                    <a
                        href=format!("/observatory/chart/{}", c.slug)
                        class="text-white/80 hover:text-[#f7931a] underline decoration-white/20 underline-offset-2 transition-colors"
                        on:click=move |_| set_compare.set(String::new())
                    >
                        "Open its chart"
                    </a>
                </p>
            }.into_any(),
            None => view! {
                <p class="text-xs text-white/45 mt-1.5">
                    "one at a time: they share the right axis"
                </p>
            }.into_any(),
        }}
    }
}

#[component]
fn Toggle(
    label: &'static str,
    get: ReadSignal<bool>,
    set: WriteSignal<bool>,
    /// Greyed and inert when the other occupant already holds the right axis.
    /// Showing why beats silently dropping one of two selected series.
    disabled: Signal<bool>,
    /// Why it is refused, shown on hover. A prop rather than a constant
    /// because one toggle can be refused for two different reasons: the right
    /// axis being taken, or the chart already plotting that series itself.
    /// The old hardcoded sentence would have been simply untrue for the
    /// second, and a wrong explanation is worse than none.
    #[prop(default = "The right axis is taken by the other series")]
    disabled_reason: &'static str,
    /// What this overlay marks, for a reader who has not met the term. Half
    /// of these are Bitcoin vocabulary that the chart otherwise assumes.
    #[prop(default = "")]
    hint: &'static str,
) -> impl IntoView {
    view! {
        <div class="flex items-center gap-1">
        <label
            class=move || if disabled.get() {
                "flex items-center gap-2 opacity-40 cursor-not-allowed min-w-0"
            } else {
                "flex items-center gap-2 cursor-pointer group min-w-0"
            }
            title=move || if disabled.get() { disabled_reason } else { "" }
        >
            <span
                class=move || if get.get() {
                    "w-3.5 h-3.5 rounded border border-[#f7931a] bg-[#f7931a]/30 shrink-0"
                } else {
                    "w-3.5 h-3.5 rounded border border-white/20 group-hover:border-white/40 shrink-0"
                }
            ></span>
            // `aria-label`, because nothing else names this control. The
            // visible word sits in a `DefinedTerm` sibling outside the
            // `<label>` (so that explaining the term does not toggle the
            // overlay), and the span that is inside the label renders only
            // when `hint` is empty, which no call site leaves empty. The
            // result was six checkboxes with an accessible name of "",
            // announced as six indistinguishable "checkbox, not checked".
            // Measured over CDP on 2026-09-21. `prop:disabled` as well, so a
            // refused overlay is refused to the keyboard and not only to the
            // pointer.
            <input
                type="checkbox"
                class="sr-only"
                aria-label=label
                prop:checked=move || get.get()
                prop:disabled=move || disabled.get()
                on:change=move |_| {
                    if !disabled.get() {
                        set.update(|v| *v = !*v);
                    }
                }
            />
            <span class="text-sm text-white/80 group-hover:text-white/90 transition-colors truncate">
                {(hint.is_empty()).then_some(label)}
            </span>
        </label>
        // Outside the label, or explaining the term would toggle the overlay
        // it sits beside.
        {(!hint.is_empty()).then(|| view! {
            <span class="text-sm text-white/80">
                <DefinedTerm label=label text=hint/>
            </span>
        })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    /// No chart's page description may end up saying "measured in count".
    ///
    /// The audit's example was Hash Rate, whose unit was generic `Count` and
    /// which now declares `HashesPerSecond`. The fallback sentence remains
    /// for the 20-odd charts that really are counts, where naming the unit
    /// adds nothing, so the clause is dropped rather than filled.
    ///
    /// Asserted against the generator rather than searched for in the
    /// source, because this sentence exists only in the output.
    #[test]
    fn no_generated_description_names_a_generic_unit() {
        for meta in super::registry::CHARTS {
            let d = super::meta_description(meta);
            assert!(!d.contains("measured in count"), "{}: {d}", meta.slug);
            // `Mixed` is as generic as `Count` and was not exempt, so
            // Weekday Activity shipped "measured in mixed" as its search
            // result. It is the only `Mixed` chart, and the word names no
            // unit, so it takes the same fallback.
            assert!(!d.contains("measured in mixed"), "{}: {d}", meta.slug);
            assert!(d.starts_with(meta.title), "{}: {d}", meta.slug);
            // And the clause is present wherever the unit names something,
            // or dropping it would be hiding the unit rather than the
            // fallback.
            if meta.unit != super::registry::Unit::Count
                && meta.unit != super::registry::Unit::Mixed
            {
                assert!(
                    d.contains("measured in "),
                    "{} declares {:?} but its description does not say so: \
                     {d}",
                    meta.slug,
                    meta.unit
                );
            }
        }
    }

    use super::*;

    /// This file, read back, so the guards below check what is actually
    /// rendered rather than a second list that can drift from it. Same
    /// technique the registry uses against the four page sources.
    const SELF: &str = include_str!("single_chart.rs");

    /// Every attribute value in the file, which for these guards means every
    /// piece of user-facing copy: tooltips, titles and labels.
    fn quoted_strings() -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = SELF;
        while let Some(i) = rest.find('"') {
            rest = &rest[i + 1..];
            match rest.find('"') {
                Some(j) => {
                    out.push(rest[..j].to_string());
                    rest = &rest[j + 1..];
                }
                None => break,
            }
        }
        out
    }

    /// The site's copy rule, enforced where copy is actually written rather
    /// than left to review. An emdash reaching a tooltip is the same defect as
    /// one reaching a chart title, which shipped once already.
    #[test]
    fn no_user_facing_copy_contains_an_emdash() {
        for s in quoted_strings() {
            assert!(
                !s.contains('\u{2014}') && !s.contains('\u{2013}'),
                "dash character in copy: {s}"
            );
        }
    }

    /// A key figure with no explanation is the case this feature exists to
    /// prevent: the reader sees "observations" and has nowhere to go. Adding
    /// a `Fact` without a hint has to fail rather than ship silently.
    #[test]
    fn every_key_figure_explains_itself() {
        let labels: Vec<&str> = SELF
            .split("<Fact")
            .skip(1)
            .filter_map(|chunk| {
                chunk.split("label=\"").nth(1)?.split('"').next()
            })
            .collect();
        assert!(
            labels.len() >= 10,
            "failed to parse Fact labels out of this file, found {}. The \
             parser, not the copy, is probably what broke.",
            labels.len()
        );
        for label in &labels {
            assert!(
                !fact_hint(label).is_empty(),
                "the key figure \"{label}\" has no explanation in fact_hint"
            );
        }
    }

    /// And the reverse, so a renamed label leaves a dead arm behind rather
    /// than an unexplained figure that looks explained in the source.
    #[test]
    fn no_key_figure_explanation_is_orphaned() {
        for label in [
            "average",
            "peak",
            "low",
            "change",
            "observations",
            "latest total",
            "largest band",
            "bands",
            "largest",
            "its value",
            "entries",
        ] {
            assert!(!fact_hint(label).is_empty(), "{label} lost its hint");
            assert!(
                SELF.contains(&format!("label=\"{label}\"")),
                "fact_hint explains \"{label}\", which nothing renders"
            );
        }
        assert_eq!(fact_hint("not a real label"), "");
    }

    /// Two Linear/Log pairs side by side are only readable because each is
    /// labelled, so a case that falls through to the generic word defeats the
    /// control. The comparison case is the one that did: the function read
    /// only OverlayFlags, which a comparison does not travel in.
    #[test]
    fn the_right_axis_switch_names_whatever_is_on_that_axis() {
        use crate::stats::charts::registry;
        let price = OverlayFlags {
            price_data: vec![(1, 1.0)],
            ..Default::default()
        };
        let size = OverlayFlags {
            chain_size_data: vec![(1, 1.0)],
            ..Default::default()
        };
        assert_eq!(right_axis_label(&price, None), "price");
        assert_eq!(right_axis_label(&size, None), "chain size");

        // A comparison names its unit, and takes precedence: it is the
        // occupant, and the flags it is contending with are switched off.
        let compared = registry::CHARTS
            .iter()
            .find(|c| c.can_compare())
            .expect("a comparable chart");
        let label = right_axis_label(&OverlayFlags::default(), Some(compared));
        assert_eq!(label, compared.unit.label());
        assert_ne!(label, "overlay", "fell through to the generic word");

        // Both overlays at once get an axis each and only the first is
        // switchable, so naming one of two would be wrong half the time.
        let both = OverlayFlags {
            price_data: vec![(1, 1.0)],
            chain_size_data: vec![(1, 1.0)],
            ..Default::default()
        };
        assert_eq!(right_axis_label(&both, None), "overlay");
    }
}
