//! Fee charts: total fees and subsidy vs fees breakdown.

use leptos::prelude::*;

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
    let (fee_unit, set_fee_unit) = signal("btc".to_string());

    view! {
        <ChartPageLayout
            title="Fees"
            description="Transaction fees earned by miners and the block reward breakdown"
        >
            {move || {
                dashboard_data.get().and_then(|r| r.ok()).map(|_| {
                    let fees_option = Signal::derive(move || {
                        let _r = range.get();
                        let unit = fee_unit.get();
                        let flags = overlay_flags.get();
                        dashboard_data
                            .get()
                            .and_then(|r| r.ok())
                            .map(|data| {
                                let (json, is_daily) = match data {
                                    DashboardData::PerBlock(ref blocks) => {
                                        (crate::stats::charts::fees_chart_unit(blocks, &unit), false)
                                    }
                                    DashboardData::Daily(ref days) => {
                                        (crate::stats::charts::fees_chart_daily_unit(days, &unit), true)
                                    }
                                };
                                if json.is_empty() { return String::new(); }
                                crate::stats::charts::apply_overlays(&json, &flags, is_daily)
                            })
                            .unwrap_or_default()
                    });

                    let subsidy_fees_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::subsidy_vs_fees_chart(blocks),
                        |days| crate::stats::charts::subsidy_vs_fees_chart_daily(days)
                    );

                    view! {
                        <div class="space-y-10">
                            <ChartCard
                                title="Total Fees per Block"
                                description=chart_desc(range, "Total transaction fees earned by miners in each block", "Average daily transaction fees earned by miners per block")
                                chart_id="chart-fees"
                                option=fees_option
                            >
                                <button
                                    class="text-xs text-white/40 hover:text-white/60 px-2 py-1 rounded border border-white/10 cursor-pointer"
                                    on:click=move |_| {
                                        set_fee_unit.update(|u| {
                                            *u = if *u == "sats" { "btc".to_string() } else { "sats".to_string() }
                                        });
                                    }
                                >
                                    {move || if fee_unit.get() == "sats" { "Switch to BTC" } else { "Switch to sats" }}
                                </button>
                            </ChartCard>
                            <ChartCard
                                title="Subsidy vs Fees"
                                description=chart_desc(range, "Block reward breakdown per block. The subsidy halves every 4 years while fees must eventually replace it", "Daily average block reward breakdown. The subsidy halves every 4 years while fees must eventually replace it")
                                chart_id="chart-subsidy-fees"
                                option=subsidy_fees_option
                            />
                        </div>
                    }.into_any()
                }).unwrap_or_else(|| view! { <ChartPageSkeleton count=2/> }.into_any())
            }}
        </ChartPageLayout>
    }
}
