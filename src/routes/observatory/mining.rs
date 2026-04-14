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

/// Mining charts page — difficulty and pool distribution in one scrollable list.
#[component]
pub fn MiningChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    // Mining-specific data (pool dominance + empty blocks)
    let mining_data = LocalResource::new(move || {
        let r = range.get();
        async move {
            let stats =
                fetch_stats_summary().await.map_err(|e| e.to_string())?;
            let n = range_to_blocks(&r);
            let is_daily = n > 5_000;

            if is_daily {
                let from_ts = stats.latest_timestamp.saturating_sub(n * 600);
                let miners = fetch_miner_dominance_daily(
                    from_ts,
                    stats.latest_timestamp,
                )
                .await
                .map_err(|e| e.to_string())?;
                let from =
                    stats.min_height.max(stats.max_height.saturating_sub(n));
                let empty = fetch_empty_blocks(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>((miners, empty))
            } else {
                let from =
                    stats.min_height.max(stats.max_height.saturating_sub(n));
                let miners = fetch_miner_dominance(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?;
                let empty = fetch_empty_blocks(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok((miners, empty))
            }
        }
    });

    view! {
        <Title text="Bitcoin Mining Charts: Difficulty & Pool Distribution | WE HODL BTC"/>
        <Meta name="description" content="Bitcoin mining analytics with difficulty adjustment history, mining pool dominance distribution including OCEAN template miners, and empty block tracking across the network."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/charts/mining"/>
        <ChartPageLayout
            title="Mining"
            description="Difficulty adjustments and mining pool distribution"
            seo_text="Monitor Bitcoin's mining landscape. The difficulty chart tracks the network's computational security as it adjusts every 2,016 blocks. Pool distribution shows which mining pools are producing blocks, with OCEAN template miners identified individually. Empty blocks are tracked historically, while common in Bitcoin's early years, they are rare today and typically indicate intentional miner behavior."
        >
            // ── Difficulty (uses shared dashboard_data) ──────────
            {move || match dashboard_data.get() {
                Some(Ok(_)) => {
                    let diff_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::difficulty_chart(blocks),
                        |days| crate::stats::charts::difficulty_chart_daily(days)
                    );
                    let diff_ribbon_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::difficulty_ribbon_chart(blocks),
                        |days| crate::stats::charts::difficulty_ribbon_chart_daily(days)
                    );
                    view! {
                        <div class="space-y-10">
                            <SectionHeading id="section-difficulty" title="Difficulty"/>
                            <ChartCard title="Difficulty" description=chart_desc(range, "Mining difficulty per block, adjusts every 2,016 blocks (~2 weeks)", "Daily mining difficulty, adjusts every 2,016 blocks (~2 weeks)") chart_id="chart-difficulty" option=diff_option/>
                            <ChartCard title="Difficulty Ribbon" description=chart_desc(range, "Multiple moving averages of mining difficulty. When short MAs cross below long MAs, it may indicate miner capitulation", "Daily difficulty ribbon showing 7 moving averages from 7-day to 128-day") chart_id="chart-diff-ribbon" option=diff_ribbon_option info="Seven moving averages of difficulty (7, 14, 25, 40, 60, 90, 128 days) form a ribbon. When the ribbon is wide, difficulty is rising steadily. When it compresses or inverts (short MAs drop below long MAs), difficulty is declining. Historically, ribbon inversions have coincided with periods of miner shutdowns."/>
                        </div>
                    }.into_any()
                }
                Some(Err(_)) => view! {
                    <DataLoadError on_retry=Callback::new(move |_| dashboard_data.refetch())/>
                }.into_any(),
                None => view! { <ChartPageSkeleton count=1/> }.into_any(),
            }}

            // ── Pool Distribution (uses mining_data) ─────────────
            {move || match mining_data.get() {
                Some(Ok(_)) => {
                    let miner_chart_option = Signal::derive(move || {
                        mining_data.get().and_then(|r| r.ok())
                            .map(|(ref miners, _)| {
                                let value = crate::stats::charts::miner_dominance_chart(miners);
                                serde_json::to_string(&value).unwrap_or_default()
                            })
                            .unwrap_or_default()
                    });
                    let empty_blocks_option = Signal::derive(move || {
                        let flags = overlay_flags.get();
                        mining_data.get().and_then(|r| r.ok())
                            .map(|(_, ref empty)| {
                                let mut value = crate::stats::charts::empty_blocks_chart(empty);
                                if value.is_null() { return String::new(); }
                                crate::stats::charts::apply_overlays(&mut value, &flags, true);
                                serde_json::to_string(&value).unwrap_or_default()
                            })
                            .unwrap_or_default()
                    });
                    let empty_by_pool_option = Signal::derive(move || {
                        mining_data.get().and_then(|r| r.ok())
                            .map(|(_, ref empty)| {
                                let value = crate::stats::charts::empty_blocks_by_pool_chart(empty);
                                serde_json::to_string(&value).unwrap_or_default()
                            })
                            .unwrap_or_default()
                    });
                    let diversity_option = Signal::derive(move || {
                        mining_data.get().and_then(|r| r.ok())
                            .map(|(ref miners, _)| {
                                let value = crate::stats::charts::mining_diversity_chart(miners);
                                serde_json::to_string(&value).unwrap_or_default()
                            })
                            .unwrap_or_default()
                    });

                    view! {
                        <div class="space-y-10">
                            <SectionHeading id="section-pools" title="Mining Pools"/>
                            <ChartCard title="Mining Pool Share" description="Which mining pools are finding the most blocks. More distributed is healthier for the network" chart_id="chart-miner-dominance" option=miner_chart_option info="Pools are identified by matching known signatures in the coinbase transaction's scriptSig text. OCEAN template miners are identified individually. Hover slices for block counts and percentages."/>
                            <ChartCard title="Mining Diversity Index" description="Herfindahl-Hirschman Index (HHI) measuring mining concentration. Below 1000 is competitive, above 1800 is concentrated" chart_id="chart-diversity" option=diversity_option info="The HHI is calculated by squaring each pool's market share percentage and summing the results. A monopoly scores 10,000, perfectly distributed mining scores near 0. Below 1,000 (green): competitive. 1,000-1,800 (yellow): moderate concentration. Above 1,800 (red): high concentration. Unknown miners are excluded from the calculation."/>
                            <ChartCard title="Empty Blocks" description="Blocks with no user transactions, usually mined before the pool has received the previous block's transactions" chart_id="chart-empty-blocks" option=empty_blocks_option info="A block with only a coinbase transaction (no user transactions). This happens when a miner finds a block before propagating the previous block's transactions. Common in early Bitcoin, rare today. Modern pools typically include transactions within seconds of receiving a new block."/>
                            <ChartCard title="Empty Blocks by Pool" description="Which mining pools produce the most coinbase-only blocks" chart_id="chart-empty-by-pool" option=empty_by_pool_option/>
                        </div>
                    }.into_any()
                }
                Some(Err(_)) => view! {
                    <div class="flex flex-col items-center justify-center min-h-[200px] gap-4">
                        <p class="text-white/50 font-mono text-sm">"Failed to load data"</p>
                        <button class="px-4 py-2 bg-white/10 hover:bg-white/20 text-white/70 rounded-lg font-mono text-sm cursor-pointer"
                            on:click=move |_| { mining_data.refetch(); }
                        >"Retry"</button>
                    </div>
                }.into_any(),
                None => view! { <ChartPageSkeleton count=2/> }.into_any(),
            }}
        </ChartPageLayout>
    }
}
