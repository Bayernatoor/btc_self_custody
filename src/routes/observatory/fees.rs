//! Fee charts: total fees per block, avg fee per tx, median fee rate, fee rate
//! bands (10th/50th/90th percentiles), and subsidy-vs-fees breakdown.
//!
//! Shows how miners are compensated and the long-term transition from block
//! subsidy to fee-based security as each halving cuts the subsidy in half.

use leptos::prelude::*;
use leptos_meta::*;

use super::components::*;
use super::helpers::chart_desc;
use super::shared::*;
use crate::chart_memo;

/// Fee charts page showing miner revenue composition and fee pressure over time.
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

                    let fee_revenue_share_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::fee_revenue_share_chart(blocks),
                        |days| crate::stats::charts::fee_revenue_share_chart_daily(days)
                    );

                    let btc_volume_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::btc_volume_chart(blocks),
                        |days| crate::stats::charts::btc_volume_chart_daily(days)
                    );

                    let value_flow_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::value_flow_chart(blocks),
                        |days| crate::stats::charts::value_flow_chart_daily(days)
                    );

                    let fee_pressure_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::fee_pressure_chart(blocks),
                        |_days| crate::stats::charts::no_data_chart("Fee Pressure vs Block Space")
                    );
                    let fee_spike_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::fee_spike_chart(blocks),
                        |_days| crate::stats::charts::no_data_chart("Fee Spike Detector")
                    );
                    let halving_era_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::halving_era_chart(blocks),
                        |days| crate::stats::charts::halving_era_chart_daily(days)
                    );
                    let fee_heatmap_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::fee_rate_heatmap_chart(blocks),
                        |_days| crate::stats::charts::no_data_chart("Fee Rate Bands (Full)")
                    );
                    let max_tx_fee_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::max_tx_fee_chart(blocks),
                        |_days| crate::stats::charts::no_data_chart("Max Transaction Fee")
                    );
                    let protocol_fees_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::protocol_fee_breakdown_chart(blocks),
                        |_days| crate::stats::charts::no_data_chart("Protocol Fee Revenue")
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
                            <ChartCard
                                title="Fee Revenue Share"
                                description=chart_desc(range, "Percentage of total block reward that comes from fees rather than subsidy", "Daily average fee revenue as a percentage of total block reward")
                                chart_id="chart-fee-revenue-share"
                                option=fee_revenue_share_option
                            />
                            <ChartCard
                                title="BTC Transferred Volume"
                                description=chart_desc(range, "Total non-coinbase output value per block in BTC", "Daily total non-coinbase output value in BTC")
                                chart_id="chart-btc-volume"
                                option=btc_volume_option
                            />
                            <ChartCard
                                title="Input vs Output Value"
                                description=chart_desc(range, "Total input value vs output value per block in BTC. The gap between the lines represents fees extracted by miners", "Daily total input and output value in BTC. The gap represents fees extracted by miners")
                                chart_id="chart-value-flow"
                                option=value_flow_option
                            />
                            <ChartCard
                                title="Fee Pressure vs Block Space"
                                description="Scatter plot showing the relationship between block fullness and fee rates. Clusters in the top-right indicate high-demand periods"
                                chart_id="chart-fee-pressure"
                                option=fee_pressure_option
                            />
                            <ChartCard
                                title="Fee Spike Detector"
                                description="Highlights blocks where the median fee rate exceeded 5x the trailing 144-block average. Red dots mark fee spike events"
                                chart_id="chart-fee-spikes"
                                option=fee_spike_option
                            />
                            <ChartCard
                                title="Halving Era Comparison"
                                description="Side-by-side comparison of average block metrics across Bitcoin's halving eras. Shows how the network evolves between halvings"
                                chart_id="chart-halving-era"
                                option=halving_era_option
                            />
                            <ChartCard
                                title="Fee Rate Bands (Full)"
                                description="Fee rate percentiles from p10 to p90 showing the full spread of fee rates per block"
                                chart_id="chart-fee-heatmap"
                                option=fee_heatmap_option
                                coming_soon=true
                            />
                            <ChartCard
                                title="Max Transaction Fee"
                                description="Largest individual transaction fee per block in BTC. Fat-finger fees and high-priority transactions stand out"
                                chart_id="chart-max-tx-fee"
                                option=max_tx_fee_option
                                coming_soon=true
                            />
                            <ChartCard
                                title="Protocol Fee Revenue"
                                description="Fee revenue breakdown by protocol: Ordinals inscriptions, Runes, and other transactions"
                                chart_id="chart-protocol-fees"
                                option=protocol_fees_option
                                coming_soon=true
                            />
                        </div>
                    }.into_any()
                }
                Some(Err(_)) => view! {
                    <DataLoadError on_retry=Callback::new(move |_| dashboard_data.refetch())/>
                }.into_any(),
                None => view! { <ChartPageSkeleton count=2/> }.into_any(),
            }}
        </ChartPageLayout>
    }
}
