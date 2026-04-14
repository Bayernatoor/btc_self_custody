//! Embedded Data charts: Overview, Protocols, and Inscriptions in one scrollable list.
//!
//! All charts render flat with section headings. IntersectionObserver lazy-inits
//! ECharts so offscreen charts don't compute until scrolled into view.

use leptos::prelude::*;
use leptos_meta::*;

use super::components::*;
use super::helpers::chart_desc;
use super::shared::*;
use crate::chart_memo;

/// Embedded data charts page — overview, protocols, and inscriptions in one scrollable list.
#[component]
pub fn EmbeddedChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    view! {
        <Title text="Bitcoin Embedded Data: OP_RETURN, Inscriptions & Runes | WE HODL BTC"/>
        <Meta name="description" content="Track non-financial data embedded in Bitcoin blocks: OP_RETURN protocols (Runes, Omni, Counterparty), Ordinals inscriptions, BRC-20 tokens, and Stamps usage with counts, volumes, and block share over time."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/charts/embedded"/>
        <ChartPageLayout
            title="Embedded Data"
            description="OP_RETURN protocols, Ordinals inscriptions, and on-chain data usage"
            seo_text="Analyze non-financial data embedded in Bitcoin transactions. OP_RETURN protocols like Runes, Omni Layer, and Counterparty use dedicated outputs for on-chain data. Ordinals inscriptions and BRC-20 tokens store data in witness fields. Stamps use bare multisig encoding. These charts track the count, volume, and block share of each protocol over time."
            header=move || view! {
                <a href="/observatory/learn/protocols"
                    class="text-xs text-white/30 hover:text-[#f7931a] transition-colors flex items-center gap-1.5"
                >
                    "Protocol guide \u{2192}"
                </a>
                <a href="/observatory/learn/methodology"
                    class="text-xs text-white/30 hover:text-[#f7931a] transition-colors flex items-center gap-1.5"
                >
                    "Methodology \u{2192}"
                </a>
            }
        >
            {move || match dashboard_data.get() {
                Some(Ok(_)) => {
                    // ── Overview ──────────────────────────────────────
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

                    // ── Protocols ─────────────────────────────────────
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

                    // ── Inscriptions ──────────────────────────────────
                    let inscription_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::inscription_chart(blocks),
                        |days| crate::stats::charts::inscription_chart_daily(days)
                    );
                    let inscription_share_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::inscription_share_chart(blocks),
                        |days| crate::stats::charts::inscription_share_chart_daily(days)
                    );

                    view! {
                        <div class="space-y-10">
                            // ── Overview ─────────────────────────
                            <SectionHeading id="section-overview" title="Overview"/>
                            <ChartCard title="All Embedded Data — Block Share" description=chart_desc(range, "How much of each block is non-financial data (OP_RETURN outputs plus witness inscriptions)", "Daily average non-financial data share per block") chart_id="chart-all-embedded-share" option=all_embedded_share_option/>
                            <ChartCard title="All Embedded Data — Count" description=chart_desc(range, "Outputs per block by protocol: Runes, Omni, Counterparty, Ordinals, BRC-20, Stamps, and other data", "Daily average embedded outputs per block by protocol") chart_id="chart-unified-count" option=unified_count_option/>
                            <ChartCard title="All Embedded Data — Volume" description=chart_desc(range, "Bytes of data embedded per block by protocol", "Daily average bytes of data embedded per block by protocol") chart_id="chart-unified-volume" option=unified_volume_option/>

                            // ── Protocols ────────────────────────
                            <SectionHeading id="section-protocols" title="OP_RETURN"/>
                            <ChartCard title="OP_RETURN Count" description=chart_desc(range, "Number of OP_RETURN outputs per block by protocol", "Daily average OP_RETURN outputs per block by protocol") chart_id="chart-opreturn-count" option=op_count_option/>
                            <ChartCard title="OP_RETURN Volume" description=chart_desc(range, "Bytes of data stored in OP_RETURN outputs per block by protocol", "Daily average OP_RETURN bytes per block by protocol") chart_id="chart-opreturn-bytes" option=op_bytes_option/>
                            <ChartCard title="OP_RETURN Protocol Share" description="Which protocols are using the most OP_RETURN outputs. Runes dominate since their 2024 launch" chart_id="chart-runes-pct" option=runes_pct_option/>
                            <ChartCard title="OP_RETURN Block Share" description=chart_desc(range, "OP_RETURN data as a percentage of each block's size", "Daily average OP_RETURN data as a percentage of block size") chart_id="chart-op-block-share" option=op_block_share_option/>

                            // ── Inscriptions ─────────────────────
                            <SectionHeading id="section-witness" title="Ordinals & Witness Data"/>
                            <ChartCard title="Ordinals Inscriptions" description=chart_desc(range, "Inscriptions per block: images, text, and other data stored in witness data", "Daily average inscriptions per block") chart_id="chart-inscriptions" option=inscription_option/>
                            <ChartCard title="Inscription Block Share" description=chart_desc(range, "Inscription data as a percentage of each block's size", "Daily average inscription data as a percentage of block size") chart_id="chart-inscription-share" option=inscription_share_option/>
                        </div>
                    }.into_any()
                }
                Some(Err(_)) => view! {
                    <DataLoadError on_retry=Callback::new(move |_| dashboard_data.refetch())/>
                }.into_any(),
                None => view! { <ChartPageSkeleton count=3/> }.into_any(),
            }}
        </ChartPageLayout>
    }
}
