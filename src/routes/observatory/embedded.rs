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
            }
        >
            // Error overlay
            {move || match dashboard_data.get() {
                Some(Err(_)) => Some(view! {
                    <DataLoadError on_retry=Callback::new(move |_| dashboard_data.refetch())/>
                }),
                _ => None,
            }}

            {
                    // All chart signals at component level (persist across refetches)
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
                    let inscription_envelope_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::inscription_envelope_chart(blocks),
                        |days| crate::stats::charts::inscription_envelope_chart_daily(days)
                    );
                    let inscription_fee_share_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::inscription_fee_share_chart(blocks),
                        |days| crate::stats::charts::inscription_fee_share_chart_daily(days)
                    );
                    let protocol_fee_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::protocol_fee_competition_chart(blocks),
                        |days| crate::stats::charts::protocol_fee_competition_chart_daily(days)
                    );

                    view! {
                        <div class="space-y-10">
                            // ── Overview ─────────────────────────
                            <SectionHeading id="section-overview" title="Overview"/>
                            <ChartCard title="All Embedded Data — Block Share" description=chart_desc(range, "How much of each block is non-financial data (OP_RETURN outputs plus witness inscriptions)", "Daily average non-financial data share per block") chart_id="chart-all-embedded-share" option=all_embedded_share_option info="Combines OP_RETURN data (in outputs) and inscription data (in witness) as a percentage of total block size. These are disjoint categories that together represent all classified embedded data."/>
                            <ChartCard title="All Embedded Data — Count" description=chart_desc(range, "Outputs per block by protocol: Runes, Omni, Counterparty, Ordinals, BRC-20, Stamps, and other data", "Daily average embedded outputs per block by protocol") chart_id="chart-unified-count" option=unified_count_option info="Stacked count of embedded data items by protocol. BRC-20 is a subset of Inscriptions (do not add them). Runes, Omni, and Counterparty are mutually exclusive subsets of OP_RETURN. See the Methodology page for the full taxonomy."/>
                            <ChartCard title="All Embedded Data — Volume" description=chart_desc(range, "Bytes of data embedded per block by protocol", "Daily average bytes of data embedded per block by protocol") chart_id="chart-unified-volume" option=unified_volume_option/>

                            // ── Protocols ────────────────────────
                            <SectionHeading id="section-protocols" title="OP_RETURN"/>
                            <ChartCard title="OP_RETURN Count" description=chart_desc(range, "Number of OP_RETURN outputs per block by protocol", "Daily average OP_RETURN outputs per block by protocol") chart_id="chart-opreturn-count" option=op_count_option/>
                            <ChartCard title="OP_RETURN Volume" description=chart_desc(range, "Bytes of data stored in OP_RETURN outputs per block by protocol", "Daily average OP_RETURN bytes per block by protocol") chart_id="chart-opreturn-bytes" option=op_bytes_option info="Byte counts include the full scriptPubKey: the OP_RETURN opcode, push opcodes, and the protocol payload. This is the actual on-chain storage footprint of each OP_RETURN output."/>
                            <ChartCard title="OP_RETURN Protocol Share" description="Which protocols are using the most OP_RETURN outputs. Runes dominate since their 2024 launch" chart_id="chart-runes-pct" option=runes_pct_option/>
                            <ChartCard title="OP_RETURN Block Share" description=chart_desc(range, "OP_RETURN data as a percentage of each block's size", "Daily average OP_RETURN data as a percentage of block size") chart_id="chart-op-block-share" option=op_block_share_option/>

                            // ── Inscriptions ─────────────────────
                            <SectionHeading id="section-witness" title="Ordinals & Witness Data"/>
                            <ChartCard title="Ordinals Inscriptions" description=chart_desc(range, "Inscriptions per block: images, text, and other data stored in witness data", "Daily average inscriptions per block") chart_id="chart-inscriptions" option=inscription_option info="Counts standard Ordinals inscription envelopes detected in witness data (OP_FALSE OP_IF 'ord' pattern). One transaction can contain multiple inscriptions. Cursed and non-standard envelopes are not currently detected."/>
                            <ChartCard title="Inscription Block Share" description=chart_desc(range, "Total inscription witness data (payload + envelope overhead) as a percentage of each block", "Daily average inscription data as a percentage of block size") chart_id="chart-inscription-share" option=inscription_share_option info="Includes both the inscription content (images, text, JSON) and the witness envelope structure (OP_FALSE OP_IF, push opcodes, 'ord' marker). This represents the true on-chain footprint. Witness data gets a 75% weight discount, so inscriptions consume less block weight than their raw byte size suggests."/>
                            <ChartCard title="Inscription Payload vs Envelope" description=chart_desc(range, "Breakdown of inscription witness data into actual content (payload) and protocol overhead (envelope structure)", "Daily average inscription payload vs envelope overhead per block") chart_id="chart-inscription-envelope" option=inscription_envelope_option info="Every Ordinals inscription wraps content in a witness envelope: OP_FALSE OP_IF ... OP_ENDIF with push opcodes and the 'ord' marker. The overhead is typically 10-15% of total inscription bytes. Higher overhead ratios indicate smaller inscriptions (like BRC-20 JSON operations) where the fixed envelope cost is a larger fraction."/>
                            <ChartCard title="Inscription Fee Share" description=chart_desc(range, "Percentage of total transaction fees paid by inscription-bearing transactions", "Daily inscription fee revenue as a percentage of total fees") chart_id="chart-inscription-fee-share" option=inscription_fee_share_option info="Tracks how much of the block's fee revenue comes from transactions containing Ordinals inscriptions. During high-demand periods like BRC-20 launches, inscription fees can spike significantly as inscribers compete for block space."/>
                            <ChartCard title="Protocol Fee Competition" description=chart_desc(range, "Fee revenue breakdown: standard transactions vs Ordinals inscriptions vs Runes protocol", "Daily fee revenue by protocol type") chart_id="chart-protocol-fee-competition" option=protocol_fee_option info="Shows how total fee revenue is split between standard Bitcoin transactions, Ordinals inscription transactions, and Runes protocol transactions. Reveals which protocol type is driving fee pressure at any given time."/>
                        </div>
                    }
            }
        </ChartPageLayout>
    }
}
