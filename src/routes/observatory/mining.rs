//! Mining charts: Difficulty and Pool Distribution in a single scrollable list.
//!
//! Difficulty charts use the shared dashboard_data resource. Pool distribution
//! uses a separate mining_data fetch (miner dominance + empty blocks).

use leptos::prelude::*;
use leptos_meta::*;

use super::components::*;
use super::helpers::*;
use super::shared::*;
use crate::chart_memo;
use crate::stats::server_fns::*;
use crate::stats::types::uses_daily_aggregates;

/// Mining charts page — difficulty and pool distribution in one scrollable list.
#[component]
pub fn MiningChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;
    let retargets = state.retargets;
    let custom_from = state.custom_from;
    let custom_to = state.custom_to;

    // Mining-specific data (pool dominance + empty blocks).
    //
    // Reads the custom dates as well as the range, which it did not until
    // 2026-09-21. `range_to_blocks("custom")` is 999,999, so `from`
    // saturated to `min_height` and these four charts described the whole
    // chain while the difficulty charts above them described the selected
    // window. Editing the dates never refetched either, because `range`
    // stays "custom" while they change. `resolve_window` is the shared
    // resolution the single-chart view already used.
    let mining_data = LocalResource::new(move || {
        let r = range.get();
        let cf = custom_from.get();
        let ct = custom_to.get();
        async move {
            let stats =
                fetch_stats_summary().await.map_err(|e| e.to_string())?;
            let is_daily = uses_daily_aggregates(range_to_blocks(&r));
            let (from, to, from_ts, to_ts) =
                resolve_window(&r, cf, ct, &stats).await?;

            // An empty window, so nothing is fetched. The endpoints reject a
            // reversed range and a 500 would read as a server fault rather
            // than as a window holding no blocks.
            if from > to {
                return Ok::<_, String>((Vec::new(), Vec::new(), Vec::new()));
            }

            // Both empty-block charts are groupings, so they are aggregated
            // server-side regardless of mode. Only pool dominance differs
            // between per-block and daily.
            let empty_monthly = fetch_empty_blocks_monthly(from, to)
                .await
                .map_err(|e| e.to_string())?;
            let empty_by_pool = fetch_empty_blocks_by_pool(from, to)
                .await
                .map_err(|e| e.to_string())?;

            let miners = if is_daily {
                fetch_miner_dominance_daily(from_ts, to_ts)
                    .await
                    .map_err(|e| e.to_string())?
            } else {
                fetch_miner_dominance(from, to)
                    .await
                    .map_err(|e| e.to_string())?
            };
            Ok::<_, String>((miners, empty_monthly, empty_by_pool))
        }
    });

    view! {
        <Title text="Bitcoin Mining Charts: Difficulty & Pool Distribution | We Hodl BTC"/>
        <Meta name="description" content="Bitcoin mining analytics with difficulty adjustment history, mining pool dominance distribution including OCEAN template miners, and empty block tracking across the network."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/charts/mining"/>
        <ChartPageLayout
            title="Mining"
            description="Difficulty adjustments and mining pool distribution"
            seo_text="Monitor Bitcoin's mining landscape. The difficulty chart tracks the network's computational security as it adjusts every 2,016 blocks. Pool distribution shows which mining pools are producing blocks, with OCEAN template miners identified individually. Empty blocks are tracked historically: 78,800 of the 89,929 coinbase-only blocks in the chain are from 2009 and 2010, and they are rare today. This data shows that a block carried no user transactions, not why."
        >
            // Error overlays
            {move || match dashboard_data.get() {
                Some((_, Err(_))) => Some(view! {
                    <DataLoadError on_retry=Callback::new(move |_| dashboard_data.refetch())/>
                }),
                _ => None,
            }}

            {
                // All chart signals at component level (persist across refetches)
                let diff_option = chart_memo!(dashboard_data, range, overlay_flags,
                    |blocks| crate::stats::charts::difficulty_chart(blocks),
                    |days| crate::stats::charts::difficulty_chart_daily(days)
                );
                let diff_ribbon_option = chart_memo!(dashboard_data, range, overlay_flags,
                    |blocks| crate::stats::charts::difficulty_ribbon_chart(blocks),
                    |days| crate::stats::charts::difficulty_ribbon_chart_daily(days)
                );
                let hash_rate_option = chart_memo!(dashboard_data, range, overlay_flags,
                    |blocks| crate::stats::charts::hash_rate_chart(blocks),
                    |days| crate::stats::charts::hash_rate_chart_daily(days)
                );
                // The daily arm needs the window's retarget blocks, which the
                // daily rows cannot supply: a day's mean difficulty is a
                // blend wherever a retarget lands mid-day.
                //
                // The fourth argument puts those rows in the cache key, and
                // it is load-bearing. `retargets` is derived from the
                // resolved days, so it cannot fetch until they land, and a
                // resource holds its previous value while refetching. Coming
                // from a per-block range it resolves to an empty list, and a
                // chart built from new days plus that stale empty list draws
                // "No difficulty adjustment in this range", which is
                // non-null and so was cached and re-served for the whole
                // session. Keyed on the rows, that answer belongs to the
                // empty list and is replaced when the real rows arrive.
                //
                // A pending or failed fetch still returns null, so the card
                // keeps its loading state rather than asserting an absence.
                let diff_adjustment_option = chart_memo!(dashboard_data, range, overlay_flags,
                    // First and last height plus the count: the rows are
                    // ordered and at most ~480 of them, so this identifies a
                    // window's retargets without hashing the whole list.
                    match retargets.get().map(|r| r.ok()) {
                        Some(Some(rows)) => format!(
                            "r{}:{}:{}",
                            rows.len(),
                            rows.first().map(|r| r.height).unwrap_or(0),
                            rows.last().map(|r| r.height).unwrap_or(0),
                        ),
                        Some(None) => "r:failed".to_string(),
                        None => "r:pending".to_string(),
                    },
                    |blocks| crate::stats::charts::difficulty_adjustment_chart(blocks),
                    |days| match retargets.get().map(|r| r.ok()) {
                        Some(Some(rows)) => crate::stats::charts::difficulty_adjustment_chart_daily(days, &rows),
                        _ => serde_json::Value::Null,
                    }
                );

                let miner_chart_option = Signal::derive(move || {
                    mining_data.get().and_then(|r| r.ok())
                        .map(|(ref miners, _, _)| {
                            let value = crate::stats::charts::miner_dominance_chart(miners);
                            serde_json::to_string(&value).unwrap_or_default()
                        })
                        .unwrap_or_default()
                });
                let empty_blocks_option = Signal::derive(move || {
                    let flags = overlay_flags.get();
                    mining_data.get().and_then(|r| r.ok())
                        .map(|(_, ref monthly, _)| {
                            let mut value = crate::stats::charts::empty_blocks_chart(monthly);
                            if value.is_null() { return String::new(); }
                            crate::stats::charts::apply_overlays(&mut value, &flags, true);
                            serde_json::to_string(&value).unwrap_or_default()
                        })
                        .unwrap_or_default()
                });
                let empty_by_pool_option = Signal::derive(move || {
                    mining_data.get().and_then(|r| r.ok())
                        .map(|(_, _, ref by_pool)| {
                            let value = crate::stats::charts::empty_blocks_by_pool_chart(by_pool);
                            serde_json::to_string(&value).unwrap_or_default()
                        })
                        .unwrap_or_default()
                });
                let diversity_option = Signal::derive(move || {
                    mining_data.get().and_then(|r| r.ok())
                        .map(|(ref miners, _, _)| {
                            let value = crate::stats::charts::mining_diversity_chart(miners);
                            serde_json::to_string(&value).unwrap_or_default()
                        })
                        .unwrap_or_default()
                });

                view! {
                    <div class="space-y-10">
                        <SectionHeading id="section-difficulty" title="Difficulty"/>
                        <ChartCard title="Difficulty" description=chart_desc(range, "Mining difficulty per block, adjusts every 2,016 blocks (~2 weeks)", "Daily mining difficulty, adjusts every 2,016 blocks (~2 weeks)") chart_id="chart-difficulty" option=diff_option/>
                        <ChartCard title="Hash Rate" description=chart_desc(range, "Estimated hashes per second the network is computing, derived from difficulty", "Estimated daily hash rate, derived from difficulty") chart_id="chart-hash-rate" option=hash_rate_option/>
                        <ChartCard title="Difficulty Adjustment" description=chart_desc(range, "How much difficulty moved at each retarget, every 2,016 blocks", "How much difficulty moved at each retarget, every 2,016 blocks") chart_id="chart-diff-adjustment" option=diff_adjustment_option/>
                        <ChartCard title="Difficulty Ribbon" description=chart_desc(range, "Seven moving averages of difficulty, from 9 to 128 blocks. There is no unsmoothed line: every series here is derived", "Seven moving averages of difficulty, from 7 to 128 days. There is no unsmoothed line: every series here is derived") chart_id="chart-diff-ribbon" option=diff_ribbon_option/>

                        <SectionHeading id="section-pools" title="Mining Pools"/>
                        <ChartCard title="Mining Pool Share" description="Share of blocks in the range by identified pool, with unattributed blocks kept separate" chart_id="chart-miner-dominance" option=miner_chart_option/>
                        <ChartCard title="Mining Diversity Index" description="Herfindahl-Hirschman Index (HHI) measuring mining concentration. Below 1000 is competitive, above 1800 is concentrated" chart_id="chart-diversity" option=diversity_option/>
                        <ChartCard title="Empty Blocks" description="Blocks carrying only the coinbase transaction, which is almost always the early chain: 78,800 of the 89,929 are from 2009 and 2010" chart_id="chart-empty-blocks" option=empty_blocks_option/>
                        <ChartCard title="Empty Blocks by Pool" description="Coinbase-only blocks in the range, grouped by the pool that mined them" chart_id="chart-empty-by-pool" option=empty_by_pool_option/>
                    </div>
                }
            }
        </ChartPageLayout>
    }
}
