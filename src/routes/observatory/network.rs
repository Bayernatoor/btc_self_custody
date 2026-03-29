//! Network charts: blocks, adoption, transaction metrics.

use leptos::prelude::*;

use crate::chart_memo;
use super::components::*;
use super::shared::*;

#[component]
pub fn NetworkChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    // Sub-section navigation — created OUTSIDE the reactive closure
    let (section, set_section) = signal("blocks".to_string());

    view! {
        <ChartPageLayout
            title="Network"
            description="Block size, weight, intervals, adoption trends, and transaction metrics"
            header=move || view! {
                <div class="flex flex-wrap gap-2 justify-center mb-6">
                    {[("blocks", "Blocks"), ("adoption", "Adoption"), ("tx-metrics", "Transactions")].into_iter().map(|(id, label)| {
                        let id_str = id.to_string();
                        let id_clone = id_str.clone();
                        view! {
                            <button
                                class=move || {
                                    if section.get() == id_clone {
                                        "px-4 py-1.5 text-xs rounded-lg bg-white/10 text-white font-semibold border border-white/20 cursor-pointer"
                                    } else {
                                        "px-4 py-1.5 text-xs rounded-lg text-white/40 hover:text-white/70 hover:bg-white/5 transition-all cursor-pointer"
                                    }
                                }
                                on:click={
                                    let id = id_str.clone();
                                    move |_| set_section.set(id.clone())
                                }
                            >
                                {label}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>
            }
        >
            {move || {
                dashboard_data.get().and_then(|r| r.ok()).map(|_| {
                    // --- Block charts ---
                    let size_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::block_size_chart(blocks),
                        |days| crate::stats::charts::block_size_chart_daily(days)
                    );

                    let weight_util_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::weight_utilization_chart(blocks),
                        |days| crate::stats::charts::weight_utilization_chart_daily(days)
                    );

                    let tx_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::tx_count_chart(blocks),
                        |days| crate::stats::charts::tx_count_chart_daily(days)
                    );

                    let avg_tx_size_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::avg_tx_size_chart(blocks),
                        |days| crate::stats::charts::avg_tx_size_chart_daily(days)
                    );

                    let interval_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::block_interval_chart(blocks),
                        |days| crate::stats::charts::block_interval_chart_daily(days)
                    );

                    let chain_size_option = Signal::derive(move || {
                        let _r = range.get();
                        let flags = overlay_flags.get();
                        let disk_gb = state.cached_live.get_untracked()
                            .map(|s| s.network.chain_size_gb)
                            .unwrap_or(0.0);
                        dashboard_data
                            .get()
                            .and_then(|r| r.ok())
                            .map(|data| {
                                let (json, is_daily) = match data {
                                    DashboardData::PerBlock(ref blocks) => {
                                        (crate::stats::charts::chain_size_chart(blocks, disk_gb), false)
                                    }
                                    DashboardData::Daily(ref days) => {
                                        (crate::stats::charts::chain_size_chart_daily(days, disk_gb), true)
                                    }
                                };
                                if json.is_empty() { return String::new(); }
                                crate::stats::charts::apply_overlays(&json, &flags, is_daily)
                            })
                            .unwrap_or_default()
                    });

                    // --- Adoption charts ---
                    let segwit_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::segwit_adoption_chart(blocks),
                        |days| crate::stats::charts::segwit_adoption_chart_daily(days)
                    );

                    let taproot_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::taproot_chart(blocks),
                        |days| crate::stats::charts::taproot_chart_daily(days)
                    );

                    let witness_version_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::witness_version_chart(blocks),
                        |days| crate::stats::charts::witness_version_chart_daily(days)
                    );

                    let witness_pct_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::witness_version_pct_chart(blocks),
                        |days| crate::stats::charts::witness_version_pct_chart_daily(days)
                    );

                    let witness_tx_pct_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::witness_version_tx_pct_chart(blocks),
                        |days| crate::stats::charts::witness_version_tx_pct_chart_daily(days)
                    );

                    let address_type_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::address_type_chart(blocks),
                        |days| crate::stats::charts::address_type_chart_daily(days)
                    );

                    let address_type_pct_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::address_type_pct_chart(blocks),
                        |days| crate::stats::charts::address_type_pct_chart_daily(days)
                    );

                    let taproot_spend_type_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::taproot_spend_type_chart(blocks),
                        |days| crate::stats::charts::taproot_spend_type_chart_daily(days)
                    );

                    let witness_share_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::witness_share_chart(blocks),
                        |days| crate::stats::charts::witness_share_chart_daily(days)
                    );

                    // --- Transaction metrics ---
                    let rbf_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::rbf_chart(blocks),
                        |days| crate::stats::charts::rbf_chart_daily(days)
                    );

                    let utxo_flow_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::utxo_flow_chart(blocks),
                        |days| crate::stats::charts::utxo_flow_chart_daily(days)
                    );

                    let batching_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::batching_chart(blocks),
                        |days| crate::stats::charts::batching_chart_daily(days)
                    );

                    view! {
                        // --- Blocks sub-section ---
                        <div class=move || if section.get() == "blocks" { "space-y-10" } else { "hidden" }>
                            <ChartCard title="Block Size" description="How large each block is in megabytes" chart_id="chart-size" option=size_option/>
                            <ChartCard title="Weight Utilization" description="How full each block is, as a percentage of the 4 MWU limit" chart_id="chart-weight-util" option=weight_util_option/>
                            <ChartCard title="Transaction Count" description="Number of transactions included in each block" chart_id="chart-txcount" option=tx_option/>
                            <ChartCard title="Avg Transaction Size" description="Average size of a transaction in bytes. Smaller means more efficient use of block space" chart_id="chart-avg-tx-size" option=avg_tx_size_option/>
                            <ChartCard title="Block Interval" description="Minutes between consecutive blocks. Target is 10 minutes" chart_id="chart-interval" option=interval_option/>
                            <ChartCard title="Chain Size Growth" description="Total blockchain size over time, showing how fast the chain is growing" chart_id="chart-chain-size" option=chain_size_option/>
                        </div>

                        // --- Adoption sub-section ---
                        <div class=move || if section.get() == "adoption" { "space-y-10" } else { "hidden" }>
                            <ChartCard title="SegWit Adoption" description="Percentage of transactions using Segregated Witness" chart_id="chart-segwit" option=segwit_option/>
                            <ChartCard title="Taproot Outputs" description="New Taproot (P2TR) outputs created per block" chart_id="chart-taproot" option=taproot_option/>
                            <ChartCard title="Witness Version Comparison" description="SegWit v0 vs Taproot v1 spend counts. How quickly is Taproot catching up?" chart_id="chart-witness-versions" option=witness_version_option/>
                            <ChartCard title="Witness Version Share" description="SegWit v0 vs Taproot v1 as a percentage of all witness spends" chart_id="chart-witness-pct" option=witness_pct_option/>
                            <ChartCard title="Output Type Breakdown" description="Legacy vs SegWit vs Taproot as a percentage of all outputs" chart_id="chart-witness-tx-pct" option=witness_tx_pct_option/>
                            <ChartCard title="Address Type Evolution" description="Output types over time. Watch P2PKH (legacy) shrink as P2WPKH (SegWit) and P2TR (Taproot) grow" chart_id="chart-address-types" option=address_type_option/>
                            <ChartCard title="Address Type Share" description="Each output type as a percentage of total, showing the shift from legacy to SegWit to Taproot" chart_id="chart-address-types-pct" option=address_type_pct_option/>
                            <ChartCard title="Taproot Spend Types" description="Key-path (private, efficient) vs script-path (complex scripts, inscriptions). How Taproot is actually being used" chart_id="chart-taproot-spend-types" option=taproot_spend_type_option/>
                            <ChartCard title="Witness Data Share" description="Witness data as percentage of block size. Higher means more SegWit discount savings" chart_id="chart-witness-share" option=witness_share_option/>
                        </div>

                        // --- Transaction Metrics sub-section ---
                        <div class=move || if section.get() == "tx-metrics" { "space-y-10" } else { "hidden" }>
                            <ChartCard title="RBF Adoption" description="Percentage of transactions opting into Replace-By-Fee, which allows fee bumping stuck transactions" chart_id="chart-rbf" option=rbf_option/>
                            <ChartCard title="UTXO Flow" description="Inputs spent vs outputs created per block. When outputs exceed inputs, the UTXO set grows" chart_id="chart-utxo-flow" option=utxo_flow_option/>
                            <ChartCard title="Transaction Batching" description="Average inputs and outputs per transaction. More outputs per tx means exchanges are batching payments" chart_id="chart-batching" option=batching_option/>
                        </div>
                    }.into_any()
                }).unwrap_or_else(|| view! { <ChartPageSkeleton count=6/> }.into_any())
            }}
        </ChartPageLayout>
    }
}
