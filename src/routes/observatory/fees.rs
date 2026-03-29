//! Fee charts: total fees and subsidy vs fees breakdown.

use leptos::prelude::*;

use crate::chart_signal;
use super::components::*;
use super::shared::*;

#[component]
pub fn FeeChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    let (fee_unit, set_fee_unit) = signal("btc".to_string());

    let fees_option = {
        let (cached, set_cached) = signal(String::new());
        Effect::new(move |_| {
            let _r = range.get();
            let unit = fee_unit.get();
            let flags = overlay_flags.get();
            let result = dashboard_data
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
                });
            if let Some(r) = result { set_cached.set(r); }
        });
        Signal::derive(move || cached.get())
    };

    let subsidy_fees_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::subsidy_vs_fees_chart(blocks),
        |days| crate::stats::charts::subsidy_vs_fees_chart_daily(days)
    );

    view! {
        <ChartPageLayout
            title="Fees"
            description="Transaction fees earned by miners and the block reward breakdown"
        >
            <div class="space-y-10">
                <ChartCard
                    title="Total Fees per Block"
                    description="Total transaction fees earned by miners in each block"
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
                    description="Block reward breakdown. The subsidy halves every 4 years while fees must eventually replace it"
                    chart_id="chart-subsidy-fees"
                    option=subsidy_fees_option
                />
            </div>
        </ChartPageLayout>
    }
}
