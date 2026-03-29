//! Embedded Data charts: OP_RETURN protocols, inscriptions.

use leptos::prelude::*;

use crate::chart_memo;
use super::components::*;
use super::shared::*;

#[component]
pub fn EmbeddedChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    // Sub-section navigation — created OUTSIDE the reactive closure
    let (section, set_section) = signal("overview".to_string());

    view! {
        <ChartPageLayout
            title="Embedded Data"
            description="OP_RETURN protocols, Ordinals inscriptions, and on-chain data usage"
            header=move || view! {
                <div class="flex justify-center mb-4">
                    <a href="/observatory/learn/protocols"
                        class="text-xs text-white/30 hover:text-[#f7931a] transition-colors flex items-center gap-1.5"
                    >
                        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 6.042A8.967 8.967 0 006 3.75c-1.052 0-2.062.18-3 .512v14.25A8.987 8.987 0 016 18c2.305 0 4.408.867 6 2.292m0-14.25a8.966 8.966 0 016-2.292c1.052 0 2.062.18 3 .512v14.25A8.987 8.987 0 0018 18a8.967 8.967 0 00-6 2.292m0-14.25v14.25"/>
                        </svg>
                        "Learn about embedding protocols \u{2192}"
                    </a>
                </div>
                <div class="flex flex-wrap gap-2 justify-center mb-6">
                    {[("overview", "Overview"), ("protocols", "Protocols"), ("witness", "Inscriptions")].into_iter().map(|(id, label)| {
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
                    // --- Overview charts ---
                    let all_embedded_share_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::all_embedded_share_chart(blocks),
                        |days| crate::stats::charts::all_embedded_share_chart_daily(days)
                    );

                    let unified_count_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::unified_embedded_count_chart(blocks),
                        |days| crate::stats::charts::unified_embedded_count_chart_daily(days)
                    );

                    let unified_volume_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::unified_embedded_volume_chart(blocks),
                        |days| crate::stats::charts::unified_embedded_volume_chart_daily(days)
                    );

                    // --- Protocol charts ---
                    let op_count_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::op_return_count_chart(blocks),
                        |days| crate::stats::charts::op_return_count_chart_daily(days)
                    );

                    let op_bytes_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::op_return_bytes_chart(blocks),
                        |days| crate::stats::charts::op_return_bytes_chart_daily(days)
                    );

                    let runes_pct_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::runes_pct_chart(blocks),
                        |days| crate::stats::charts::runes_pct_chart_daily(days)
                    );

                    let op_block_share_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::op_return_block_share_chart(blocks),
                        |days| crate::stats::charts::op_return_block_share_chart_daily(days)
                    );

                    // --- Inscription charts ---
                    let inscription_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::inscription_chart(blocks),
                        |days| crate::stats::charts::inscription_chart_daily(days)
                    );

                    let inscription_share_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::inscription_share_chart(blocks),
                        |days| crate::stats::charts::inscription_share_chart_daily(days)
                    );

                    view! {
                        // --- Overview sub-section ---
                        <div class=move || if section.get() == "overview" { "space-y-10" } else { "hidden" }>
                            <ChartCard title="All Embedded Data — Block Share" description="How much of each block is non-financial data (OP_RETURN outputs plus witness inscriptions)" chart_id="chart-all-embedded-share" option=all_embedded_share_option/>
                            <ChartCard title="All Embedded Data — Count" description="Outputs per block by protocol: Runes, Omni, Counterparty, Ordinals, BRC-20, Stamps, and other data" chart_id="chart-unified-count" option=unified_count_option/>
                            <ChartCard title="All Embedded Data — Volume" description="Bytes of data embedded per block by protocol. Who is using the most block space?" chart_id="chart-unified-volume" option=unified_volume_option/>
                        </div>

                        // --- Protocols sub-section ---
                        <div class=move || if section.get() == "protocols" { "space-y-10" } else { "hidden" }>
                            <ChartCard title="OP_RETURN Count" description="Number of OP_RETURN outputs per block, broken down by protocol (Runes, Omni, Counterparty, Other)" chart_id="chart-opreturn-count" option=op_count_option/>
                            <ChartCard title="OP_RETURN Volume" description="Bytes of data stored in OP_RETURN outputs per block by protocol" chart_id="chart-opreturn-bytes" option=op_bytes_option/>
                            <ChartCard title="OP_RETURN Protocol Share" description="Which protocols are using the most OP_RETURN outputs. Runes dominate since their 2024 launch" chart_id="chart-runes-pct" option=runes_pct_option/>
                            <ChartCard title="OP_RETURN Block Share" description="OP_RETURN data as a percentage of total block size" chart_id="chart-op-block-share" option=op_block_share_option/>
                        </div>

                        // --- Inscriptions sub-section ---
                        <div class=move || if section.get() == "witness" { "space-y-10" } else { "hidden" }>
                            <ChartCard title="Ordinals Inscriptions" description="Inscriptions per block: images, text, and other data stored in witness data since 2023" chart_id="chart-inscriptions" option=inscription_option/>
                            <ChartCard title="Inscription Block Share" description="Inscription data as a percentage of block size. At peak, inscriptions consumed over 50% of block space" chart_id="chart-inscription-share" option=inscription_share_option/>
                        </div>
                    }.into_any()
                }).unwrap_or_else(|| view! { <ChartPageSkeleton count=3/> }.into_any())
            }}
        </ChartPageLayout>
    }
}
