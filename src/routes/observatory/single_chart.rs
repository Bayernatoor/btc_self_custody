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
use leptos_meta::{Meta, Title};
use leptos_router::hooks::use_params_map;

use super::components::Chart;
use super::helpers::range_to_blocks;
use super::shared::{DashboardData, ObservatoryState};
use crate::stats::charts::kpi::{self, Kpis};
use crate::stats::charts::registry::{
    self, ChartMeta, Daily, MiningChart, Source,
};
use crate::stats::server_fns::{
    fetch_empty_blocks_by_pool, fetch_empty_blocks_monthly,
    fetch_fullness_histogram, fetch_miner_dominance,
    fetch_miner_dominance_daily, fetch_stats_summary,
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
    if abs >= 1_000_000.0 {
        format!("{:.2}M", v / 1_000_000.0)
    } else if abs >= 10_000.0 {
        format!("{:.1}k", v / 1_000.0)
    } else if abs >= 100.0 {
        format!("{v:.0}")
    } else if abs >= 1.0 {
        format!("{v:.2}")
    } else if abs > 0.0 {
        format!("{v:.4}")
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
    // A real 404, not a 200 carrying an apology. 61 of these paths are
    // indexable, so a soft 404 on the rest would invite search engines to
    // index "no such chart" pages for every typo and stale link. Same defect
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
            <p class="text-sm text-white/50 mb-6">
                "There is no chart called \"" {move || slug.get()} "\"."
            </p>
            <div class="flex flex-wrap gap-2 justify-center">
                <a href="/observatory/charts/network" class="text-sm px-3 py-1.5 rounded-lg bg-white/5 text-white/70 hover:text-[#f7931a] transition-colors">"Network"</a>
                <a href="/observatory/charts/fees" class="text-sm px-3 py-1.5 rounded-lg bg-white/5 text-white/70 hover:text-[#f7931a] transition-colors">"Fees"</a>
                <a href="/observatory/charts/mining" class="text-sm px-3 py-1.5 rounded-lg bg-white/5 text-white/70 hover:text-[#f7931a] transition-colors">"Mining"</a>
                <a href="/observatory/charts/embedded" class="text-sm px-3 py-1.5 rounded-lg bg-white/5 text-white/70 hover:text-[#f7931a] transition-colors">"Embedded Data"</a>
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
    // Off by default: a log axis is the right view for a few charts and a
    // misleading one for a reader who did not ask for it.
    let (log_scale, set_log_scale) = signal(false);

    let needs_mining = matches!(meta.source, Source::Mining(_));
    let needs_buckets =
        matches!(meta.source, Source::FullnessDist | Source::TimeDist);

    // Both resources are created unconditionally so the component's shape does
    // not depend on the slug, but neither fetches unless this chart needs it.
    // Firing four mining requests to render a TPS chart would be a real cost.
    let mining_data = LocalResource::new(move || {
        let r = range.get();
        async move {
            if !needs_mining {
                return Err(String::new());
            }
            let stats =
                fetch_stats_summary().await.map_err(|e| e.to_string())?;
            let n = range_to_blocks(&r);
            let from = stats.min_height.max(stats.max_height.saturating_sub(n));
            let empty_monthly =
                fetch_empty_blocks_monthly(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?;
            let empty_by_pool =
                fetch_empty_blocks_by_pool(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?;
            let miners = if uses_daily_aggregates(n) {
                let from_ts = stats.latest_timestamp.saturating_sub(n * 600);
                fetch_miner_dominance_daily(from_ts, stats.latest_timestamp)
                    .await
                    .map_err(|e| e.to_string())?
            } else {
                fetch_miner_dominance(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?
            };
            Ok::<MiningPayload, String>((miners, empty_monthly, empty_by_pool))
        }
    });

    // The two distribution charts have no daily builder. For long ranges they
    // read server-side histogram buckets instead, which is why they cannot be
    // expressed as a per-block/daily pair like the other 53.
    let buckets = LocalResource::new(move || {
        let r = range.get();
        async move {
            let n = range_to_blocks(&r);
            if !needs_buckets || !uses_daily_aggregates(n) {
                return None;
            }
            let stats = fetch_stats_summary().await.ok()?;
            let from_ts = stats.latest_timestamp.saturating_sub(n * 600);
            fetch_fullness_histogram(from_ts, stats.latest_timestamp)
                .await
                .ok()
        }
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

        let finish = |mut v: serde_json::Value, is_daily: bool| -> String {
            if v.is_null() {
                return String::new();
            }
            crate::stats::charts::apply_overlays(&mut v, &flags, is_daily);
            // After the overlays, so it only ever touches the left axis the
            // metric owns rather than the right one price or chain size added.
            crate::stats::charts::apply_log_scale(&mut v, logv);
            serde_json::to_string(&v).unwrap_or_default()
        };

        match meta.source {
            Source::Mining(which) => {
                let Some(Ok((miners, monthly, by_pool))) = mining_data.get()
                else {
                    return String::new();
                };
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
                let Some(Ok(data)) = dashboard_data.get() else {
                    return String::new();
                };
                let disk_gb = state
                    .cached_live
                    .get_untracked()
                    .map(|s| s.network.chain_size_gb)
                    .unwrap_or(0.0);
                let offset = state.chain_size_offset.get().unwrap_or(0);
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
                                blocks, disk_gb, offset,
                            ),
                            false,
                        )
                    }
                    (DashboardData::Daily(days), Source::ChainSize) => finish(
                        crate::stats::charts::chain_size_chart_daily(
                            days, disk_gb, offset,
                        ),
                        true,
                    ),
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
                        let Some(Some(b)) = buckets.get() else {
                            return String::new();
                        };
                        let v = if pct {
                            crate::stats::charts::block_fullness_histogram_from_buckets_pct(&b)
                        } else {
                            crate::stats::charts::block_fullness_histogram_from_buckets(&b)
                        };
                        serde_json::to_string(&v).unwrap_or_default()
                    }
                    (DashboardData::Daily(_), Source::TimeDist) => {
                        String::new()
                    }
                    // Mining is handled above; the compiler cannot see that.
                    (_, Source::Mining(_)) => String::new(),
                }
            }
        }
    });

    let kpis = Signal::derive(move || kpi::compute(&option.get(), meta.shape));

    // A chart with no daily builder draws nothing at long ranges. Saying which
    // ranges it supports is the honest version of an empty frame.
    let daily_gap = Signal::derive(move || {
        !meta.has_daily()
            && uses_daily_aggregates(range_to_blocks(&range.get()))
    });

    let cid = canvas_id(meta.slug);
    let title_tag = format!("{} | Bitcoin Chart | We Hodl BTC", meta.title);
    let desc_tag = format!(
        "{} for the Bitcoin network, measured in {}, from our own node. \
         Downloadable as CSV.",
        meta.title,
        meta.unit.label()
    );

    view! {
        <Title text=title_tag/>
        <Meta name="description" content=desc_tag/>

        <div class="mb-4 flex items-center gap-2 text-sm">
            <a
                href=meta.category.page_path()
                class="text-white/40 hover:text-[#f7931a] transition-colors"
            >
                {meta.category.label()} " charts"
            </a>
            <span class="text-white/20">"/"</span>
            <span class="text-white/60">{meta.title}</span>
        </div>

        <div class=move || if rail_open.get() {
            "grid grid-cols-1 lg:grid-cols-[1fr_15rem] gap-3 lg:gap-4 items-start"
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
                        <h1 class="text-lg sm:text-xl text-white font-semibold">{meta.title}</h1>
                        // The same one-liner the card shows, switching with
                        // the range the way chart_desc does on the pages, so
                        // the two views describe the metric identically.
                        <p class="text-sm text-white/60 mt-0.5">{move || {
                            if uses_daily_aggregates(range_to_blocks(&range.get())) {
                                meta.desc_daily
                            } else {
                                meta.desc_per_block
                            }
                        }}</p>
                    </div>
                    // Range sits in the chart header rather than the rail:
                    // it is one row of buttons and it was costing the chart
                    // 20rem of width to hold it in a column.
                    <div class="flex items-start gap-2 shrink-0">
                        {meta.supports_log().then(|| view! {
                            <div class="flex items-center gap-1 mt-0.5">
                                <button
                                    class=move || if log_scale.get() {
                                        "px-2 py-1 text-xs rounded-md bg-white/5 text-white/50 hover:text-white/80 cursor-pointer"
                                    } else {
                                        "px-2 py-1 text-xs rounded-md bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
                                    }
                                    title="Linear value axis"
                                    on:click=move |_| set_log_scale.set(false)
                                >
                                    "Linear"
                                </button>
                                <button
                                    class=move || if log_scale.get() {
                                        "px-2 py-1 text-xs rounded-md bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
                                    } else {
                                        "px-2 py-1 text-xs rounded-md bg-white/5 text-white/50 hover:text-white/80 cursor-pointer"
                                    }
                                    title="Logarithmic value axis, at any range. Zero and negative points cannot be plotted on it; the chart says how many were left out"
                                    on:click=move |_| set_log_scale.set(true)
                                >
                                    "Log"
                                </button>
                            </div>
                        })}
                        <RailRange/>
                        <button
                            class="hidden lg:inline-flex items-center text-white/40 hover:text-[#f7931a] transition-colors cursor-pointer p-1 rounded-md hover:bg-white/5 mt-0.5"
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
                                <p class="text-white/70 text-sm mb-1">"Not available at this range"</p>
                                <p class="text-white/40 text-xs">
                                    "This chart is computed per block, so it needs a range short
                                     enough to load individual blocks. Pick 1m or shorter."
                                </p>
                            </div>
                        </div>
                    </Show>
                    <Show when=move || !daily_gap.get() && (option.get().is_empty() || data_loading.get())>
                        <div class="absolute inset-0 flex items-center justify-center bg-[#0d2137] rounded-xl">
                            <span class="text-xs text-white/30">"Mining blocks..."</span>
                        </div>
                    </Show>
                </div>

                {matches!(meta.source, Source::FullnessDist | Source::TimeDist).then(|| view! {
                    <div class="mt-3 flex items-center gap-2">
                        <span class="text-xs text-white/40">"Show as"</span>
                        <button
                            class=move || if pct_mode.get() { "text-xs px-2 py-1 rounded-md bg-[#f7931a]/20 text-[#f7931a] cursor-pointer" } else { "text-xs px-2 py-1 rounded-md bg-white/5 text-white/50 cursor-pointer" }
                            on:click=move |_| set_pct_mode.set(true)
                        >"%"</button>
                        <button
                            class=move || if pct_mode.get() { "text-xs px-2 py-1 rounded-md bg-white/5 text-white/50 cursor-pointer" } else { "text-xs px-2 py-1 rounded-md bg-[#f7931a]/20 text-[#f7931a] cursor-pointer" }
                            on:click=move |_| set_pct_mode.set(false)
                        >"count"</button>
                    </div>
                })}

                {matches!(meta.source, Source::Fees).then(|| view! {
                    <div class="mt-3 flex items-center gap-2">
                        <span class="text-xs text-white/40">"Unit"</span>
                        <button
                            class=move || if fee_sats.get() { "text-xs px-2 py-1 rounded-md bg-white/5 text-white/50 cursor-pointer" } else { "text-xs px-2 py-1 rounded-md bg-[#f7931a]/20 text-[#f7931a] cursor-pointer" }
                            on:click=move |_| set_fee_sats.set(false)
                        >"BTC"</button>
                        <button
                            class=move || if fee_sats.get() { "text-xs px-2 py-1 rounded-md bg-[#f7931a]/20 text-[#f7931a] cursor-pointer" } else { "text-xs px-2 py-1 rounded-md bg-white/5 text-white/50 cursor-pointer" }
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
                <RailSection title="Key facts">
                    // "Over selected range", not "visible": these follow the
                    // range slice, not the zoom slider, which changes only the
                    // view. Making them follow zoom needs a datazoom handler.
                    <p class="text-[0.65rem] uppercase tracking-widest text-white/40 mb-2">
                        "over selected range"
                    </p>
                    <KeyFacts kpis=kpis unit=meta.unit/>
                </RailSection>

                <RailSection title="Annotations">
                    <OverlayToggles/>
                </RailSection>

                {
                    let rel = registry::related(meta, 4);
                    (!rel.is_empty()).then(|| view! {
                        <RailSection title="Related">
                            <div class="space-y-1.5">
                                {rel.into_iter().map(|r| view! {
                                    <a
                                        href=format!("/observatory/chart/{}", r.slug)
                                        class="block text-sm text-white/70 hover:text-[#f7931a] transition-colors"
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
            "mt-3 lg:mt-4 space-y-3 lg:space-y-4 lg:pr-[15.75rem]"
        } else {
            "mt-3 lg:mt-4 space-y-3 lg:space-y-4"
        }>
            <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-4">
                <h2 class="text-[0.7rem] uppercase tracking-widest text-white/55 mb-3">
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
                    <span class="text-xs text-white/30 ml-1">
                        "exports the selected range, without overlay series"
                    </span>
                </div>
            </div>

            // `about` is None for every chart today, so this renders the short
            // description rather than an empty heading. Definition then
            // Technical is the structure the copy will follow: what the metric
            // is, then how it is computed.
            <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-4 lg:p-5">
                <h2 class="text-[0.7rem] uppercase tracking-widest text-white/55 mb-3">
                    "About this metric"
                </h2>
                {match meta.about {
                    Some(copy) => view! {
                        <p class="text-sm text-white/70 leading-relaxed max-w-3xl">{copy}</p>
                    }.into_any(),
                    None => view! {
                        <p class="text-sm text-white/70 leading-relaxed max-w-3xl">
                            <span class="text-white/50">"Definition. "</span>
                            {meta.desc_per_block}
                            "."
                        </p>
                        <p class="text-xs text-white/35 mt-2">
                            "Measured from our own Bitcoin node. "
                            <a href="/observatory/learn/methodology" class="hover:text-[#f7931a] transition-colors">
                                "Methodology"
                            </a>
                        </p>
                    }.into_any(),
                }}
            </div>
        </div>
        // Without this the view is a dead end: four related charts and the
        // browser back button. The drawer indexes all 61.
        <super::shared::ChartDrawer/>
    }
}

/// The shared `RangeSelector` lays its twelve presets out in one non-wrapping
/// row with the mode label beside them, sized for the wide settings panel. In
/// a 20rem rail that overflows the panel entirely, so the rail gets its own
/// wrapping grid over the same signals rather than the shared component being
/// reshaped for both.
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
    let set_panel = state.set_chart_settings_open;
    let set_tab = state.set_chart_settings_tab;

    // Whether the selected range is served per block or from daily
    // aggregates. Worth surfacing because it changes what a point means, and
    // seven charts have no daily variant at all.
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
        <div class="flex flex-wrap justify-start lg:justify-end gap-1">
            {PRESETS.iter().map(|r| {
                let val = r.to_string();
                let label = r.to_uppercase();
                let is_on = {
                    let val = val.clone();
                    move || range.get() == val
                };
                view! {
                    <button
                        class=move || if is_on() {
                            "px-2 py-1 text-xs rounded-md bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
                        } else {
                            "px-2 py-1 text-xs rounded-md bg-white/5 text-white/50 hover:text-white/80 hover:bg-white/10 transition-colors cursor-pointer"
                        }
                        on:click={
                            let val = val.clone();
                            move |_| {
                                set_custom_from.set(None);
                                set_custom_to.set(None);
                                set_range.set(val.clone());
                            }
                        }
                    >
                        {label}
                    </button>
                }
            }).collect_view()}
            <button
                class=move || if range.get() == "custom" {
                    "px-2 py-1 text-xs rounded-md bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
                } else {
                    "px-2 py-1 text-xs rounded-md bg-white/5 text-white/50 hover:text-white/80 hover:bg-white/10 transition-colors cursor-pointer"
                }
                // The date picker itself lives in the chart settings panel.
                // Opening it there beats a second copy in a 20rem rail, which
                // is what overflowed when the shared selector was used here.
                on:click=move |_| {
                    set_tab.set(super::shared::ChartSettingsTab::Range);
                    set_panel.set(true);
                }
                title="Pick an exact date range"
            >
                "Custom"
            </button>
        </div>
        <p class="text-[0.7rem] text-white/45 mt-1.5 lg:text-right">{mode}</p>
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
            class="text-xs px-3 py-1.5 rounded-lg bg-white/5 text-white/70 hover:text-[#f7931a] hover:bg-white/10 transition-colors cursor-pointer inline-flex items-center gap-1.5"
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
fn RailSection(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-4">
            <h2 class="text-[0.7rem] uppercase tracking-widest text-white/55 mb-3">{title}</h2>
            {children()}
        </div>
    }
}

#[component]
fn KeyFacts(kpis: Signal<Kpis>, unit: registry::Unit) -> impl IntoView {
    view! {
        {move || match kpis.get() {
            Kpis::Series { average, peak, low, first, last, observations, axis_labels } => {
                let change = last - first;
                let pct = if first != 0.0 { change / first.abs() * 100.0 } else { 0.0 };
                view! {
                    <div class="space-y-2">
                        <Fact label="average" value=fmt_num(average) note=(unit != registry::Unit::Count).then(|| unit.label().to_string())/>
                        <Fact label="peak" value=fmt_num(peak.y) note=point_label(&peak, &axis_labels)/>
                        <Fact label="low" value=fmt_num(low.y) note=point_label(&low, &axis_labels)/>
                        <Fact
                            label="change"
                            value=format!("{}{}", if change >= 0.0 { "+" } else { "" }, fmt_num(change))
                            note=(first != 0.0).then(|| format!("{pct:+.1}%"))
                        />
                        <Fact label="observations" value=observations.to_string() note=None/>
                    </div>
                }.into_any()
            }
            Kpis::Bands { total_latest, dominant, dominant_share_pct, band_count, observations } => {
                view! {
                    <div class="space-y-2">
                        <Fact label="latest total" value=fmt_num(total_latest) note=None/>
                        <Fact label="largest band" value=dominant note=Some(format!("{dominant_share_pct:.1}% of total"))/>
                        <Fact label="bands" value=band_count.to_string() note=None/>
                        <Fact label="observations" value=observations.to_string() note=None/>
                    </div>
                }.into_any()
            }
            Kpis::Categorical { top_name, top_value, top_share_pct, entries } => {
                view! {
                    <div class="space-y-2">
                        <Fact label="largest" value=top_name note=Some(format!("{top_share_pct:.1}% of total"))/>
                        <Fact label="its value" value=fmt_num(top_value) note=None/>
                        <Fact label="entries" value=entries.to_string() note=None/>
                    </div>
                }.into_any()
            }
            // A zero here would be a claim about the data. This is not.
            Kpis::Unavailable => view! {
                <p class="text-sm text-white/40">"Not available for this range."</p>
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
    view! {
        <div class="flex items-baseline justify-between gap-3">
            <span class="text-xs text-white/55 shrink-0">{label}</span>
            <span class="text-right min-w-0">
                <span class="text-sm text-white font-mono">{value}</span>
                {note.map(|n| view! {
                    <span class="block text-[0.7rem] text-white/55 font-mono">{n}</span>
                })}
            </span>
        </div>
    }
}

/// The overlays already exist and are effectively hidden behind a floating
/// panel. The rail is the fix asked for: they are simply always on screen.
#[component]
fn OverlayToggles() -> impl IntoView {
    let s = expect_context::<ObservatoryState>();
    view! {
        <div class="space-y-1.5">
            <Toggle label="Halvings" get=s.overlay_halvings set=s.set_overlay_halvings/>
            <Toggle label="BIP activations" get=s.overlay_bips set=s.set_overlay_bips/>
            <Toggle label="Core releases" get=s.overlay_core set=s.set_overlay_core/>
            <Toggle label="Events" get=s.overlay_events set=s.set_overlay_events/>
            <Toggle label="Price (right axis)" get=s.overlay_price set=s.set_overlay_price/>
            <Toggle label="Chain size (right axis)" get=s.overlay_chain_size set=s.set_overlay_chain_size/>
        </div>
    }
}

#[component]
fn Toggle(
    label: &'static str,
    get: ReadSignal<bool>,
    set: WriteSignal<bool>,
) -> impl IntoView {
    view! {
        <label class="flex items-center gap-2 cursor-pointer group">
            <span
                class=move || if get.get() {
                    "w-3.5 h-3.5 rounded border border-[#f7931a] bg-[#f7931a]/30 shrink-0"
                } else {
                    "w-3.5 h-3.5 rounded border border-white/20 group-hover:border-white/40 shrink-0"
                }
            ></span>
            <input
                type="checkbox"
                class="sr-only"
                prop:checked=move || get.get()
                on:change=move |_| set.update(|v| *v = !*v)
            />
            <span class="text-sm text-white/60 group-hover:text-white/80 transition-colors">{label}</span>
        </label>
    }
}
