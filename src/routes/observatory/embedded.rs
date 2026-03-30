//! Embedded Data charts: OP_RETURN protocols, inscriptions.

use leptos::prelude::*;
use leptos_meta::*;

use super::components::*;
use super::helpers::chart_desc;
use super::shared::*;
use crate::chart_memo;

#[component]
pub fn EmbeddedChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    // Sub-section navigation — created OUTSIDE the reactive closure
    let (section, set_section) = signal("overview".to_string());

    view! {
        <Title text="Bitcoin Embedded Data: OP_RETURN, Inscriptions & Runes | WE HODL BTC"/>
        <Meta name="description" content="Track non-financial data embedded in Bitcoin blocks: OP_RETURN protocols (Runes, Omni, Counterparty), Ordinals inscriptions, BRC-20 tokens, and Stamps usage with counts, volumes, and block share over time."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/charts/embedded"/>
        <ChartPageLayout
            title="Embedded Data"
            description="OP_RETURN protocols, Ordinals inscriptions, and on-chain data usage"
            seo_text="Analyze non-financial data embedded in Bitcoin transactions. OP_RETURN protocols like Runes, Omni Layer, and Counterparty use dedicated outputs for on-chain data. Ordinals inscriptions and BRC-20 tokens store data in witness fields. Stamps use bare multisig encoding. These charts track the count, volume, and block share of each protocol over time."
            header=move || view! {
                <div class="relative inline-block">
                    <select
                        aria-label="Chart section"
                        class="appearance-none bg-[#0a1a2e] text-white/80 text-sm border border-white/10 rounded-xl pl-3 pr-8 py-2 cursor-pointer focus:outline-none focus:border-[#f7931a]/40 transition-colors"
                        prop:value=move || section.get()
                        on:change=move |ev| {
                            use wasm_bindgen::JsCast;
                            if let Some(t) = ev.target() {
                                if let Ok(s) = t.dyn_into::<leptos::web_sys::HtmlSelectElement>() {
                                    set_section.set(s.value());
                                }
                            }
                        }
                    >
                        <option value="overview">"Overview"</option>
                        <option value="protocols">"Protocols"</option>
                        <option value="witness">"Inscriptions"</option>
                    </select>
                    <svg class="absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none w-3.5 h-3.5 text-white/40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                    </svg>
                </div>
                <a href="/observatory/learn/protocols"
                    class="text-xs text-white/30 hover:text-[#f7931a] transition-colors flex items-center gap-1.5"
                >
                    "Protocol guide \u{2192}"
                </a>
            }
        >
            // --- Overview sub-section ---
            <Show when=move || section.get() == "overview" fallback=|| ()>
                {move || {
                    dashboard_data.get().and_then(|r| r.ok()).map(|_| {
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

                        view! {
                            <div class="space-y-10">
                                <ChartCard title="All Embedded Data — Block Share" description=chart_desc(range, "How much of each block is non-financial data (OP_RETURN outputs plus witness inscriptions)", "Daily average non-financial data share per block") chart_id="chart-all-embedded-share" option=all_embedded_share_option/>
                                <ChartCard title="All Embedded Data — Count" description=chart_desc(range, "Outputs per block by protocol: Runes, Omni, Counterparty, Ordinals, BRC-20, Stamps, and other data", "Daily average embedded outputs per block by protocol") chart_id="chart-unified-count" option=unified_count_option/>
                                <ChartCard title="All Embedded Data — Volume" description=chart_desc(range, "Bytes of data embedded per block by protocol", "Daily average bytes of data embedded per block by protocol") chart_id="chart-unified-volume" option=unified_volume_option/>
                            </div>
                        }.into_any()
                    }).unwrap_or_else(|| view! { <ChartPageSkeleton count=3/> }.into_any())
                }}
            </Show>

            // --- Protocols sub-section ---
            <Show when=move || section.get() == "protocols" fallback=|| ()>
                {move || {
                    dashboard_data.get().and_then(|r| r.ok()).map(|_| {
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

                        view! {
                            <div class="space-y-10">
                                <ChartCard title="OP_RETURN Count" description=chart_desc(range, "Number of OP_RETURN outputs per block by protocol", "Daily average OP_RETURN outputs per block by protocol") chart_id="chart-opreturn-count" option=op_count_option/>
                                <ChartCard title="OP_RETURN Volume" description=chart_desc(range, "Bytes of data stored in OP_RETURN outputs per block by protocol", "Daily average OP_RETURN bytes per block by protocol") chart_id="chart-opreturn-bytes" option=op_bytes_option/>
                                <ChartCard title="OP_RETURN Protocol Share" description="Which protocols are using the most OP_RETURN outputs. Runes dominate since their 2024 launch" chart_id="chart-runes-pct" option=runes_pct_option/>
                                <ChartCard title="OP_RETURN Block Share" description=chart_desc(range, "OP_RETURN data as a percentage of each block's size", "Daily average OP_RETURN data as a percentage of block size") chart_id="chart-op-block-share" option=op_block_share_option/>
                            </div>
                        }.into_any()
                    }).unwrap_or_else(|| view! { <ChartPageSkeleton count=4/> }.into_any())
                }}
            </Show>

            // --- Inscriptions sub-section ---
            <Show when=move || section.get() == "witness" fallback=|| ()>
                {move || {
                    dashboard_data.get().and_then(|r| r.ok()).map(|_| {
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
                                <ChartCard title="Ordinals Inscriptions" description=chart_desc(range, "Inscriptions per block: images, text, and other data stored in witness data", "Daily average inscriptions per block") chart_id="chart-inscriptions" option=inscription_option/>
                                <ChartCard title="Inscription Block Share" description=chart_desc(range, "Inscription data as a percentage of each block's size", "Daily average inscription data as a percentage of block size") chart_id="chart-inscription-share" option=inscription_share_option/>
                            </div>
                        }.into_any()
                    }).unwrap_or_else(|| view! { <ChartPageSkeleton count=2/> }.into_any())
                }}
            </Show>
        </ChartPageLayout>
    }
}
