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
use crate::extras::schema::{static_docs, StaticJsonLd};

/// Embedded data charts page — overview, protocols, and inscriptions in one scrollable list.
#[component]
pub fn EmbeddedChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    view! {
        <Title text="Bitcoin Embedded Data: OP_RETURN, Inscriptions & Runes | We Hodl BTC"/>
        <Meta name="description" content="Track protocol data and inscription content embedded in Bitcoin blocks: OP_RETURN protocols (Runes, Omni, Counterparty), Ordinals inscriptions and BRC-20 tokens, with counts, volumes and block share over time."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/charts/embedded"/>
        <StaticJsonLd doc=static_docs::DATASET_EMBEDDED/>
        <ChartPageLayout
            title="Embedded Data"
            description="OP_RETURN protocols, Ordinals inscriptions, protocol fee analysis, and miner coinbase messages"
            seo_text="Analyze protocol data and inscription content embedded in Bitcoin transactions. OP_RETURN protocols like Runes, Omni Layer, and Counterparty use dedicated outputs for on-chain data. Ordinals inscriptions and BRC-20 tokens store data in witness fields. Track each protocol's count, volume, block share, fee revenue, and encoding overhead. Miner coinbase messages reveal pool identity and signaling activity."
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
                Some((_, Err(_))) => Some(view! {
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
                    let coinbase_msg_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::coinbase_message_length_chart(blocks),
                        |_days| crate::stats::charts::coinbase_message_length_chart_daily(_days)
                    );

                    view! {
                        <div class="space-y-10">
                            // ── Overview ─────────────────────────
                            <SectionHeading id="section-overview" title="Overview"/>
                            <ChartCard title="Detected Embedded Data, Block Share" description=chart_desc(range, "Share of each block's bytes in detected protocol data: OP_RETURN script bytes plus estimated inscription payload", "Daily average share of block bytes in detected protocol data") chart_id="chart-all-embedded-share" option=all_embedded_share_option/>
                            <ChartCard title="Detected Embedded Data, Count" description=chart_desc(range, "Per block, by protocol: OP_RETURN outputs for the first four bands and inscription-bearing witness items for the last two", "Daily average per block: OP_RETURN outputs and inscription-bearing witness items, by protocol") chart_id="chart-unified-count" option=unified_count_option/>
                            <ChartCard title="Detected Embedded Data, Volume" description=chart_desc(range, "Bytes per block by protocol: exact OP_RETURN script bytes and estimated inscription payload, on one stack", "Daily average bytes per block by protocol, mixing exact OP_RETURN script bytes with estimated inscription payload") chart_id="chart-unified-volume" option=unified_volume_option/>

                            // ── Protocols ────────────────────────
                            <SectionHeading id="section-protocols" title="OP_RETURN"/>
                            <ChartCard title="OP_RETURN Count" description=chart_desc(range, "Number of OP_RETURN outputs per block by protocol", "Daily average OP_RETURN outputs per block by protocol") chart_id="chart-opreturn-count" option=op_count_option/>
                            <ChartCard title="OP_RETURN Volume" description=chart_desc(range, "Bytes of data stored in OP_RETURN outputs per block by protocol", "Daily average OP_RETURN bytes per block by protocol") chart_id="chart-opreturn-bytes" option=op_bytes_option/>
                            <ChartCard title="OP_RETURN Protocol Share" description="Detected OP_RETURN outputs by protocol. Runes were 97.0% of them in 2024, the year they launched, and over 97% so far in 2026" chart_id="chart-runes-pct" option=runes_pct_option/>
                            <ChartCard title="OP_RETURN Block Share" description=chart_desc(range, "OP_RETURN data as a percentage of each block's size", "Daily average OP_RETURN data as a percentage of block size") chart_id="chart-op-block-share" option=op_block_share_option/>

                            // ── Inscriptions ─────────────────────
                            <SectionHeading id="section-witness" title="Ordinals & Witness Data"/>
                            <ChartCard title="Ordinals Inscriptions" description=chart_desc(range, "Inscriptions per block: images, text, and other data stored in witness data", "Daily average inscriptions per block") chart_id="chart-inscriptions" option=inscription_option/>
                            <ChartCard title="Inscription Block Share" description=chart_desc(range, "Total inscription witness data (payload + envelope overhead) as a percentage of each block", "Daily average inscription data as a percentage of block size") chart_id="chart-inscription-share" option=inscription_share_option/>
                            <ChartCard title="Inscription Payload vs Envelope" description=chart_desc(range, "Detected inscription witness bytes split into an estimated payload and the envelope structure around it", "Daily average inscription payload vs envelope overhead per block") chart_id="chart-inscription-envelope" option=inscription_envelope_option/>
                            <ChartCard title="Inscription Fee Share" description=chart_desc(range, "Percentage of total transaction fees paid by inscription-bearing transactions", "Daily inscription fee revenue as a percentage of total fees") chart_id="chart-inscription-fee-share" option=inscription_fee_share_option/>
                            <ChartCard title="Protocol Fee Competition" description=chart_desc(range, "Fees paid by transactions matching each protocol detector, side by side. The detectors can match the same transaction, so these are not parts of one total", "Daily fees paid by transactions matching each protocol detector. The detectors can match the same transaction, so these are not parts of one total") chart_id="chart-protocol-fee-competition" option=protocol_fee_option/>

                            // ── Miner Signals ────────────────────
                            <SectionHeading id="section-coinbase" title="Miner Signals"/>
                            <ChartCard title="Coinbase Message Length" description="Length of decoded ASCII text found in each block's coinbase transaction. Mining pools embed identifiers, timestamps, and occasionally custom messages in this space" chart_id="chart-coinbase-msg-length" option=coinbase_msg_option/>
                        </div>
                    }
            }
        </ChartPageLayout>
    }
}
