//! Fee charts: total fees and subsidy vs fees breakdown.

use leptos::prelude::*;
use leptos_meta::*;

use super::components::*;
use super::helpers::chart_desc;
use super::shared::*;
use crate::chart_memo;

#[component]
pub fn FeeChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    // Fee unit toggle — created OUTSIDE the reactive closure
    let fee_unit = Signal::derive(|| "btc".to_string());

    view! {
        <Title text="Bitcoin Fee Charts: Miner Revenue & Subsidy Breakdown | WE HODL BTC"/>
        <Meta name="description" content="Bitcoin transaction fee analytics showing total fees per block in BTC and sats, daily averages, and the block reward breakdown of subsidy versus fee revenue across all halving eras."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/charts/fees"/>
        <ChartPageLayout
            title="Fees"
            description="Transaction fees earned by miners and the block reward breakdown"
            seo_text="Track how Bitcoin miners are compensated. Total fees per block show the demand for block space in real time, while the subsidy versus fees breakdown reveals the long-term transition from block reward to fee-based security as each halving cuts the subsidy in half."
        >
            {move || match dashboard_data.get() {
                Some(Ok(_)) => {
                    let fees_option = Signal::derive(move || {
                        let _r = range.get();
                        let unit = fee_unit.get();
                        let flags = overlay_flags.get();
                        dashboard_data
                            .get()
                            .and_then(|r| r.ok())
                            .map(|data| {
                                let (mut value, is_daily) = match data {
                                    DashboardData::PerBlock(ref blocks) => {
                                        (crate::stats::charts::fees_chart_unit(blocks, &unit), false)
                                    }
                                    DashboardData::Daily(ref days) => {
                                        (crate::stats::charts::fees_chart_daily_unit(days, &unit), true)
                                    }
                                };
                                if value.is_null() { return String::new(); }
                                crate::stats::charts::apply_overlays(&mut value, &flags, is_daily);
                                serde_json::to_string(&value).unwrap_or_default()
                            })
                            .unwrap_or_default()
                    });

                    let subsidy_fees_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::subsidy_vs_fees_chart(blocks),
                        |days| crate::stats::charts::subsidy_vs_fees_chart_daily(days)
                    );

                    let avg_fee_tx_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::avg_fee_per_tx_chart(blocks),
                        |days| crate::stats::charts::avg_fee_per_tx_chart_daily(days)
                    );

                    let median_rate_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::median_fee_rate_chart(blocks),
                        |days| crate::stats::charts::median_fee_rate_chart_daily(days)
                    );

                    let fee_band_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::fee_rate_band_chart(blocks),
                        |days| crate::stats::charts::fee_rate_band_chart_daily(days)
                    );

                    view! {
                        <div class="space-y-10">
                            <ChartCard
                                title="Total Fees per Block"
                                description=chart_desc(range, "Total transaction fees earned by miners in each block", "Average daily transaction fees earned by miners per block")
                                chart_id="chart-fees"
                                option=fees_option
                            />
                            <ChartCard
                                title="Avg Fee per Transaction"
                                description=chart_desc(range, "Average fee paid per transaction in satoshis (excludes coinbase)", "Daily average fee per transaction in satoshis")
                                chart_id="chart-avg-fee-tx"
                                option=avg_fee_tx_option
                            />
                            <ChartCard
                                title="Median Fee Rate"
                                description=chart_desc(range, "Median fee rate across all transactions in each block", "Median fee rate (per-block ranges only for daily)")
                                chart_id="chart-median-rate"
                                option=median_rate_option
                            />
                            <ChartCard
                                title="Fee Rate Band"
                                description=chart_desc(range, "Fee rate spread: 10th percentile (cheapest), median, and 90th percentile (most urgent)", "Fee rate percentile band (per-block ranges only)")
                                chart_id="chart-fee-band"
                                option=fee_band_option
                            />
                            <ChartCard
                                title="Subsidy vs Fees"
                                description=chart_desc(range, "Block reward breakdown per block. The subsidy halves every 4 years while fees must eventually replace it", "Daily average block reward breakdown. The subsidy halves every 4 years while fees must eventually replace it")
                                chart_id="chart-subsidy-fees"
                                option=subsidy_fees_option
                            />
                        </div>
                    }.into_any()
                }
                Some(Err(_)) => view! {
                    <div class="flex flex-col items-center justify-center min-h-[200px] gap-4">
                        <p class="text-white/50 font-mono text-sm">"Failed to load data"</p>
                        <button class="px-4 py-2 bg-white/10 hover:bg-white/20 text-white/70 rounded-lg font-mono text-sm cursor-pointer"
                            on:click=move |_| { dashboard_data.refetch(); }
                        >"Retry"</button>
                    </div>
                }.into_any(),
                None => view! { <ChartPageSkeleton count=2/> }.into_any(),
            }}
        </ChartPageLayout>
    }
}
