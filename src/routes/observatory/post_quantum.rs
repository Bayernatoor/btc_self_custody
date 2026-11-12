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
                    ", not coins held. An output counted here may have been spent years ago. For what is
                     actually unspent, see "
                    <a href="#section-snapshot" class="text-[#f7931a] hover:underline">
                        "the snapshot"
                    </a>
                    " further down, taken from our own node."
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
                    let value_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::pq_exposed_value_chart(blocks),
                        |days| crate::stats::charts::pq_exposed_value_chart_daily(days)
                    );
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
                            <ChartCard title="Value in Exposed Outputs" description=chart_desc(range, "Share of the value each block creates that goes into outputs publishing a public key", "Share of the value created that goes into outputs publishing a public key, by day") chart_id="chart-pq-exposed-value" option=value_option/>
                            <ChartCard title="Exposure by Construction" description=chart_desc(range, "Share of outputs created whose public key is on chain from the moment they exist", "Share of outputs created whose public key is on chain, totalled over each day") chart_id="chart-pq-exposure" option=exposure_option/>
                            <ChartCard title="P2PK Outputs Created" description=chart_desc(range, "Outputs paying a raw public key, the oldest exposed form, still created today", "Outputs paying a raw public key, averaged per block over each day") chart_id="chart-pq-p2pk-creation" option=p2pk_option/>

                            <SectionHeading id="section-snapshot" title="What is unspent today"/>
                            <SnapshotSection/>

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

/// The unspent output set, measured once.
///
/// Everything above this point is a flow: outputs created, by block or by
/// day. This is the stock, and the two answer different questions. It sits
/// on the page rather than in the chart registry because the registry's whole
/// model is a function of blocks or days over a selected range, and a
/// snapshot has neither: it is one measurement at one height, refreshed a few
/// times a year.
#[component]
fn SnapshotSection() -> impl IntoView {
    use crate::stats::charts::utxo_snapshot_data as snap;

    let era = Signal::derive(|| {
        serde_json::to_string(
            &crate::stats::charts::snapshot_exposed_by_year_chart(),
        )
        .unwrap_or_default()
    });
    let composition = Signal::derive(|| {
        serde_json::to_string(
            &crate::stats::charts::snapshot_composition_chart(),
        )
        .unwrap_or_default()
    });

    let thousands = |n: u64| {
        n.to_string()
            .as_bytes()
            .rchunks(3)
            .rev()
            .map(|c| String::from_utf8_lossy(c).to_string())
            .collect::<Vec<_>>()
            .join(",")
    };
    let exposed_utxos = snap::exposed_utxos();
    let exposed_btc = snap::exposed_sats() as f64 / 1e8;
    let pct_utxos = 100.0 * exposed_utxos as f64 / snap::TOTAL_UTXOS as f64;
    let pct_value =
        100.0 * snap::exposed_sats() as f64 / snap::TOTAL_SATS as f64;

    view! {
        <div class="space-y-6">
            <div class="rounded-xl border border-white/10 bg-[#0d2137] px-4 py-3 text-sm text-white/70 leading-relaxed">
                <p class="mb-2">
                    "Every chart above counts outputs as they were "
                    <strong class="text-white/90">"created"</strong>
                    ". This section counts what is still "
                    <strong class="text-white/90">"unspent"</strong>
                    ", which is a different number and a different story: 371.7 million Taproot outputs
                     have been created and 52.9 million have not been spent."
                </p>
                <p>
                    "Taken from this site's own node on "{snap::TAKEN}", at block "
                    {thousands(snap::BASE_HEIGHT)}
                    ". The total it accounts for matches what Bitcoin Core reports for that same
                     block to the satoshi, which is the check that makes the rest of it worth
                     publishing."
                </p>
            </div>

            <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div class="rounded-xl border border-white/10 bg-white/[0.03] px-4 py-4">
                    <div class="text-2xl font-semibold text-[#f7931a]">
                        {thousands(exposed_utxos)}
                    </div>
                    <div class="text-sm text-white/70 mt-1">
                        "unspent outputs publish a public key, "
                        {format!("{pct_utxos:.1}%")}" of the set"
                    </div>
                </div>
                <div class="rounded-xl border border-white/10 bg-white/[0.03] px-4 py-4">
                    <div class="text-2xl font-semibold text-[#f7931a]">
                        {format!("{} BTC", thousands(exposed_btc as u64))}
                    </div>
                    <div class="text-sm text-white/70 mt-1">
                        "sits in them, "{format!("{pct_value:.1}%")}
                        " of all the value in the set"
                    </div>
                </div>
            </div>

            <div class="rounded-xl border border-white/10 bg-white/[0.02] p-4">
                <h3 class="text-base font-semibold text-white/90">
                    "The Unspent Set by Output Type"
                </h3>
                <p class="text-sm text-white/60 mt-1 mb-2">
                    "Share of unspent outputs against share of the value in them"
                </p>
                <Chart id="chart-snapshot-composition" option=composition class="w-full h-[360px]"/>
                <p class="text-sm text-white/70 leading-relaxed mt-2">
                    "The two bars pull apart hardest on the two types that publish a key. P2PK is
                     0.03% of the unspent outputs and 8.5% of the value in them, an average of 38
                     BTC each. Taproot is the mirror image: a third of the outputs and about one
                     percent of the value, averaging 0.004 BTC, which is dust and inscription
                     outputs. Counting outputs and weighing coins give opposite answers here."
                </p>
            </div>

            <div class="rounded-xl border border-white/10 bg-white/[0.02] p-4">
                <h3 class="text-base font-semibold text-white/90">
                    "Exposed Value by the Year It Was Created"
                </h3>
                <p class="text-sm text-white/60 mt-1 mb-2">
                    "Unspent BTC in key-publishing outputs, by the year the output was made"
                </p>
                <Chart id="chart-snapshot-era" option=era class="w-full h-[400px]"/>
                <p class="text-sm text-white/70 leading-relaxed mt-2">
                    "34,131 outputs created before the first halving hold 1.71 million BTC, which is
                     88% of all exposed value and about 8.5% of every coin in the set. The dashed
                     line reaches 88% at 2012 and then barely moves for a decade. Almost all of it
                     is P2PK, mining rewards paid straight to a raw public key, which is how coins
                     were taken in the earliest years."
                </p>
            </div>

            <div class="rounded-xl border border-white/10 bg-[#0d2137] px-4 py-3 text-sm text-white/70 leading-relaxed">
                <p class="mb-2">
                    "These are "
                    <strong class="text-white/90">"creation"</strong>
                    " years. An output made in 2009 and unspent since has not moved in seventeen
                     years, and nothing here says whether that is because the key is lost or because
                     nobody chose to spend it. Coins that are lost cannot be stolen in any sense
                     that matters to whoever held them, so this is not a figure for coins at risk."
                </p>
                <p>
                    "It is also still exposure "
                    <strong class="text-white/90">"by construction only"</strong>
                    ". An output that publishes a hash reveals its key when it is spent, so a reused
                     address holding a balance is exposed and is counted nowhere here. That makes
                     the figures above a floor rather than a total."
                </p>
            </div>
        </div>
    }
}
