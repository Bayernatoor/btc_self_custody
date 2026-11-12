//! Post-quantum exposure charts: which outputs published a public key.
//!
//! The editorial rule for this page, from `notes/post-quantum-scope.md`:
//! **state what is exposed, never state when it will break.** No qubit counts,
//! no timelines. The measurements are strong enough unaided, and the
//! discipline is what makes the page worth reading beside every other
//! post-quantum page.

use leptos::prelude::*;
use leptos_meta::*;

use super::components::*;
use super::helpers::chart_desc;
use super::shared::*;
use crate::chart_memo;

/// Post-quantum exposure page.
#[component]
pub fn PostQuantumChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    view! {
        <Title text="Bitcoin Post-Quantum Exposure: Which Outputs Publish a Public Key | We Hodl BTC"/>
        <Meta name="description" content="Which Bitcoin outputs publish a public key on chain, measured from our own node. P2PK, bare multisig and Taproot expose a key when created; P2PKH, P2SH and SegWit v0 publish only a hash. Counts outputs created, not coins held."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/charts/post-quantum"/>
        <ChartPageLayout
            title="Post-Quantum"
            description="Which outputs have published a public key on chain, and which have published only a hash"
            seo_text="Quantum risk in Bitcoin is a question about public keys rather than addresses. An algorithm that recovers a private key from a public key only matters for coins whose public key has appeared on chain. Three output types publish one the moment they are created: P2PK and bare multisig carry keys in the output script, and a Taproot output is itself a tweaked public key. P2PKH, P2SH, P2WPKH and P2WSH publish only a hash, and reveal a key when they are spent. These charts count outputs created, from this site's own node, and say nothing about when or whether any of it becomes exploitable."
        >
            {move || match dashboard_data.get() {
                Some((_, Err(_))) => Some(view! {
                    <DataLoadError on_retry=Callback::new(move |_| dashboard_data.refetch())/>
                }),
                _ => None,
            }}

            // The framing has to arrive before the first chart, because every
            // figure below is a flow and reads as a balance if nobody says
            // otherwise. Two limits, both of which make the numbers low.
            <div class="rounded-xl border border-white/10 bg-[#0d2137] px-4 py-3 mb-6 text-sm text-white/70 leading-relaxed">
                <p class="mb-2">
                    "These charts count outputs "
                    <strong class="text-white/90">"created"</strong>
                    ", not coins held. An output counted here may have been spent years ago. A current
                     figure for exposed coins needs a snapshot of the unspent output set, which this
                     site does not yet take."
                </p>
                <p class="mb-2">
                    "They also count exposure "
                    <strong class="text-white/90">"by construction only"</strong>
                    ": outputs that publish a key the moment they exist. An address that publishes only
                     a hash reveals its key when it is spent, so a reused address holding a balance is
                     exposed too. That is not counted anywhere here."
                </p>
                <p>
                    "The two limits pull in opposite directions, so this page does not claim a net
                     figure. Leaving out reuse makes these numbers low. Counting outputs created rather
                     than outputs still unspent makes them high, because an output spent years ago is
                     still counted here. Nothing on this page says when, or whether, any of this
                     becomes exploitable."
                </p>
            </div>

            {move || {
                    let exposure_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::pq_exposure_chart(blocks),
                        |days| crate::stats::charts::pq_exposure_chart_daily(days)
                    );
                    let p2pk_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::pq_p2pk_creation_chart(blocks),
                        |days| crate::stats::charts::pq_p2pk_creation_chart_daily(days)
                    );

                    view! {
                        <div class="space-y-10">
                            <SectionHeading id="section-exposure" title="Exposed by construction"/>
                            <ChartCard title="Exposure by Construction" description=chart_desc(range, "Share of outputs created whose public key is on chain from the moment they exist", "Share of outputs created whose public key is on chain, totalled over each day") chart_id="chart-pq-exposure" option=exposure_option/>
                            <ChartCard title="P2PK Outputs Created" description=chart_desc(range, "Outputs paying a raw public key, the oldest exposed form, still created today", "Outputs paying a raw public key, averaged per block over each day") chart_id="chart-pq-p2pk-creation" option=p2pk_option/>

                            // Taproot creation already has a chart. Linking to
                            // it rather than plotting the same column again
                            // under a post-quantum title.
                            <div class="rounded-xl border border-white/10 bg-[#0d2137] px-4 py-3 text-sm text-white/70 leading-relaxed">
                                "Taproot is the largest exposed-by-construction type by a wide margin, and
                                 its exposure is structural rather than a flaw: putting the key in the output
                                 is what makes Taproot outputs small and makes a cooperative spend look like
                                 a single signature. The rate it is created at is plotted on "
                                <a href="/observatory/chart/taproot" class="text-[#f7931a] hover:underline">
                                    "Taproot Outputs"
                                </a>
                                "."
                            </div>
                        </div>
                    }
            }}
        </ChartPageLayout>
    }
}
