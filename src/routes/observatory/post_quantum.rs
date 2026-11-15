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

/// Thousands separators, because every figure in this section is large enough
/// to be misread by an order of magnitude without them.
fn thousands(n: u64) -> String {
    n.to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|c| String::from_utf8_lossy(c).to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Post-quantum exposure page.
#[component]
pub fn PostQuantumChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    // This section defaults to ALL, not to the observatory's 1Y.
    //
    // Every chart here is a seventeen-year story. P2PK creation collapsed in
    // 2012 and Taproot arrived in 2021, so a one-year window opens on the
    // flattest part of each one, and P2PK Outputs Created reads as noise
    // rather than as "almost never" because its whole shape is off screen.
    //
    // Set from the page rather than from `provide_observatory_state`, which
    // runs once for the whole observatory and so would not fire when a
    // reader arrives here from another chart page without a full reload.
    // An explicit `?range=` in the URL always wins, so a permalink and the
    // reader's own choice both survive.
    let set_range = state.set_range;
    Effect::new(move |ran_before: Option<()>| {
        if ran_before.is_some() {
            return;
        }
        let explicit = leptos_router::hooks::use_query_map()
            .read_untracked()
            .get("range")
            .is_some_and(|r| !r.is_empty());
        if !explicit {
            set_range.set("all".to_string());
        }
    });

    view! {
        <Title text="Bitcoin Post-Quantum Exposure: Which Outputs Publish a Public Key | We Hodl BTC"/>
        <Meta name="description" content="Which Bitcoin outputs publish a public key on chain, measured from my own Bitcoin node. P2PK, bare multisig and Taproot expose a key when created; P2PKH, P2SH and SegWit v0 publish only a hash. Counts outputs created, not coins held."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/charts/post-quantum"/>
        <ChartPageLayout
            title="Post-Quantum"
            description="Which outputs have published a public key on chain, and which have published only a hash"
            seo_text="Quantum risk in Bitcoin is a question about public keys rather than addresses. An algorithm that recovers a private key from a public key only matters for coins whose public key has appeared on chain. Three output types publish one the moment they are created: P2PK and bare multisig carry keys in the output script, and a Taproot output is itself a tweaked public key. P2PKH, P2SH, P2WPKH and P2WSH publish only a hash, and reveal a key when they are spent. These charts count outputs created, measured from my own Bitcoin node, and say nothing about when or whether any of it becomes exploitable."
            intro=move || view! {
                // The warning rides on the summary line so it cannot be
                // missed; the reasoning sits behind the toggle so it cannot
                // block the first chart.
                <div class="mb-8">
                    <Caveat summary=view! {
                        "These charts count outputs "
                        <strong class="text-white/90">"created"</strong>
                        ", not coins held, and only the ones exposed "
                        <strong class="text-white/90">"by construction"</strong>
                        "."
                    }.into_any()>
                        <p class="mb-2">
                            "An output counted here may have been spent years ago. For what is actually
                             unspent, see "
                            <a href="#section-snapshot" class="text-[#f7931a] hover:underline">
                                "the snapshot"
                            </a>
                            " further down, measured from my own Bitcoin node."
                        </p>
                        <p class="mb-2">
                            "Exposure "
                            <strong class="text-white/90">"by construction"</strong>
                            " means outputs that publish a key the moment they exist. An address that
                             publishes only a hash reveals its key when it is spent, so a reused address
                             holding a balance is exposed too. That is not counted anywhere here."
                        </p>
                        <p>
                            "The two limits pull in opposite directions, so this page does not claim a net
                             figure. Leaving out reuse makes these numbers low. Counting outputs created
                             rather than outputs still unspent makes them high, because an output spent
                             years ago is still counted here. Nothing on this page says when, or whether,
                             any of this becomes exploitable."
                        </p>
                    </Caveat>
                </div>
                <div class="mb-6">
                    <SectionHeading
                        id="section-exposure"
                        title="Exposed by construction"
                        lede="Outputs that publish a public key the moment they are created. The first chart weighs them by the bitcoin in them, the second counts them, and since early 2026 the two have moved in opposite directions: Taproot's share of outputs created has fallen while its share of value has risen."
                    />
                </div>
            }
        >
            {move || match dashboard_data.get() {
                Some((_, Err(_))) => Some(view! {
                    <DataLoadError on_retry=Callback::new(move |_| dashboard_data.refetch())/>
                }),
                _ => None,
            }}

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
                            <ChartCard title="Value Exposed by Construction" description=chart_desc(range, "Share of the bitcoin each block creates that lands in outputs publishing a public key", "Share of the bitcoin created each day that lands in outputs publishing a public key") chart_id="chart-pq-exposed-value" option=value_option/>
                            <ChartCard title="Exposure by Construction" description=chart_desc(range, "Share of outputs created whose public key is on chain from the moment they exist", "Share of outputs created whose public key is on chain, totalled over each day") chart_id="chart-pq-exposure" option=exposure_option/>
                            <ChartCard title="P2PK Outputs Created" description=chart_desc(range, "Outputs paying a raw public key, the oldest exposed form, still created today", "Outputs paying a raw public key, averaged per block over each day") chart_id="chart-pq-p2pk-creation" option=p2pk_option/>

                            <SectionHeading
                                id="section-snapshot"
                                title="What is still unspent"
                                stamp=format!(
                                    "As of {} \u{b7} block {}",
                                    crate::stats::charts::utxo_snapshot_data::TAKEN,
                                    thousands(crate::stats::charts::utxo_snapshot_data::BASE_HEIGHT),
                                )
                                lede="The same question asked of the coins that are still there, rather than of every output ever created. These figures are a single measurement, not a live reading."
                            />
                            <SnapshotSection/>

                            // Two closing notes, side by side. `items-start`
                            // so the collapsible one can grow without
                            // stretching its neighbour.
                            <div class="grid grid-cols-1 lg:grid-cols-2 gap-4 items-start">
                                <Caveat summary=view! {
                                    "None of this is a count of bitcoin "
                                    <strong class="text-white/90">"at risk"</strong>
                                    ": these are creation years, reuse is not counted, and a lost key cannot be stolen."
                                }.into_any()>
                                    <p class="mb-2">
                                        "The year shown is when each output was "
                                        <strong class="text-white/90">"created"</strong>
                                        ", not when anything last moved. An output made in 2009 and never spent has sat
                                         untouched for seventeen years, and nothing on chain says whether that is
                                         because the private key was lost or because nobody chose to spend it. A coin
                                         whose key is lost cannot be stolen in any sense that matters to whoever once
                                         held it, so none of this is a count of bitcoin at risk."
                                    </p>
                                    <p>
                                        "It is also still exposure "
                                        <strong class="text-white/90">"by construction only"</strong>
                                        ". An output that commits to a hash reveals its public key at the moment it is
                                         spent, so an address that has been spent from and still holds a balance is
                                         exposed too, and is counted nowhere here. The figures above are a floor, not a
                                         total."
                                    </p>
                                </Caveat>
                                // Taproot creation already has a chart. Linking
                                // to it rather than plotting the same column
                                // again under a post-quantum title.
                                <div class="rounded-xl border border-white/10 bg-[#0d2137] px-4 py-3 text-sm text-white/70 leading-relaxed">
                                    "Taproot is the largest exposed-by-construction type by a wide margin,
                                     and its exposure is structural rather than a flaw: putting the key in
                                     the output is what makes Taproot outputs small and makes a cooperative
                                     spend look like a single signature. The rate it is created at is
                                     plotted on "
                                    <a href="/observatory/chart/taproot" class="text-[#f7931a] hover:underline">
                                        "Taproot Outputs"
                                    </a>
                                    "."
                                </div>
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

    let taproot_unspent = snap::BY_KIND
        .iter()
        .find(|(k, _, _)| *k == "P2TR")
        .map(|(_, c, _)| *c)
        .unwrap_or(0);
    let exposed_utxos = snap::exposed_utxos();
    let exposed_btc = snap::exposed_sats() as f64 / 1e8;
    let pct_utxos = 100.0 * exposed_utxos as f64 / snap::TOTAL_UTXOS as f64;
    let pct_value =
        100.0 * snap::exposed_sats() as f64 / snap::TOTAL_SATS as f64;

    view! {
        <div class="space-y-6">
            <Caveat summary=view! {
                "The "
                <strong class="text-white/90">"UTXO set"</strong>
                ", short for unspent transaction outputs: every coin that exists and can still be
                 spent. Every coin you hold is an entry in it."
            }.into_any()>
                <p class="mb-2">
                    "Taken from my own Bitcoin node on "{snap::TAKEN}", at block "
                    {thousands(snap::BASE_HEIGHT)}
                    ", using Bitcoin Core's "
                    <code class="text-white/85">"dumptxoutset"</code>
                    ". Every one of the "{thousands(snap::TOTAL_UTXOS)}
                    " outputs in that file was decoded and added up, and the total comes to the same
                     satoshi Bitcoin Core reports for that block."
                </p>
                <p>
                    "Every chart above counts outputs at the moment they were "
                    <strong class="text-white/90">"created"</strong>
                    ", which is a different number entirely. By that same block "
                    {format!("{:.1}", snap::P2TR_CREATED as f64 / 1e6)}
                    " million Taproot outputs had been created since 2021, and only "
                    {format!("{:.1}", taproot_unspent as f64 / 1e6)}
                    " million of them were still unspent. The rest have been spent and no longer
                     exist as coins."
                </p>
            </Caveat>

            <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <div class="rounded-xl border border-white/10 bg-white/[0.03] px-4 py-4">
                    <div class="flex items-start gap-3">
                        <div class="text-2xl font-semibold text-[#f7931a]">
                            {thousands(exposed_utxos)}
                        </div>
                        <span class="ml-auto shrink-0 text-[10px] uppercase tracking-wider text-white/40 border border-white/10 rounded px-1.5 py-0.5 mt-1">
                            {snap::taken_short()}
                        </span>
                    </div>
                    <div class="text-sm text-white/70 mt-1">
                        "unspent outputs already show their public key on chain, "
                        {format!("{pct_utxos:.1}%")}" of the UTXO set"
                    </div>
                </div>
                <div class="rounded-xl border border-white/10 bg-white/[0.03] px-4 py-4">
                    <div class="flex items-start gap-3">
                        <div class="text-2xl font-semibold text-[#f7931a]">
                            {format!("{} BTC", thousands(exposed_btc as u64))}
                        </div>
                        <span class="ml-auto shrink-0 text-[10px] uppercase tracking-wider text-white/40 border border-white/10 rounded px-1.5 py-0.5 mt-1">
                            {snap::taken_short()}
                        </span>
                    </div>
                    <div class="text-sm text-white/70 mt-1">
                        "is held in those outputs, "{format!("{pct_value:.1}%")}
                        " of every bitcoin in existence"
                    </div>
                </div>
            </div>

            <div class="rounded-xl border border-white/10 bg-white/[0.02] p-4">
                <div class="flex items-baseline gap-3">
                    <h3 class="text-base font-semibold text-white/90">
                        "The UTXO Set by Output Type"
                    </h3>
                    <span class="ml-auto shrink-0 text-[10px] uppercase tracking-wider text-white/40 border border-white/10 rounded px-1.5 py-0.5">
                        {snap::taken_short()}
                    </span>
                </div>
                <p class="text-sm text-white/60 mt-1 mb-2">
                    "How many unspent outputs each address type holds, against how much bitcoin is
                     in them"
                </p>
                <Chart id="chart-snapshot-composition" option=composition class="w-full h-[360px]"/>
                <p class="text-sm text-white/70 leading-relaxed mt-2">
                    "Counting outputs and weighing them give opposite answers, and the two types
                     that publish a key are where they diverge most. P2PK, the original format that
                     pays straight to a public key with no address in between, is 0.03% of unspent
                     outputs and 8.5% of the bitcoin in them: 44,513 outputs averaging 38.5 BTC each.
                     Taproot runs the other way, holding a third of the outputs and roughly one
                     percent of the value, averaging 0.004 BTC apiece, which is the size of a
                     typical inscription or dust output rather than a stored balance."
                </p>
            </div>

            <div class="rounded-xl border border-white/10 bg-white/[0.02] p-4">
                <div class="flex items-baseline gap-3">
                    <h3 class="text-base font-semibold text-white/90">
                        "Exposed Bitcoin by the Year It Was Created"
                    </h3>
                    <span class="ml-auto shrink-0 text-[10px] uppercase tracking-wider text-white/40 border border-white/10 rounded px-1.5 py-0.5">
                        {snap::taken_short()}
                    </span>
                </div>
                <p class="text-sm text-white/60 mt-1 mb-2">
                    "Unspent bitcoin sitting in key-publishing outputs, grouped by the year the
                     output was created"
                </p>
                <Chart id="chart-snapshot-era" option=era class="w-full h-[400px]"/>
                <p class="text-sm text-white/70 leading-relaxed mt-2">
                    "34,178 outputs created in Bitcoin's first four years hold 1.71 million BTC
                     between them: 88% of all the exposed bitcoin, and 8.5% of every bitcoin that
                     exists. The dashed line reaches 88% at 2012 and then barely moves for a decade.
                     Almost all of it is P2PK, block rewards paid straight to a raw public key,
                     which is simply how mined coins were paid out in the earliest years."
                </p>
            </div>

        </div>
    }
}

/// A one-line summary that opens into the full explanation.
///
/// The page had eight paragraphs of caveat before and between its charts.
/// Every one of them is worth reading and none of them is worth blocking the
/// first chart, so the summary line carries the warning and the detail is one
/// click away.
#[component]
fn Caveat(#[prop(into)] summary: AnyView, children: Children) -> impl IntoView {
    view! {
        <details class="group rounded-xl border border-white/10 bg-[#0d2137] overflow-hidden">
            <summary class="cursor-pointer list-none select-none px-4 py-3 flex items-baseline gap-4 hover:bg-white/[0.03] [&::-webkit-details-marker]:hidden">
                <span class="text-sm text-white/70 leading-relaxed">{summary}</span>
                <span class="ml-auto shrink-0 text-xs text-[#f7931a] whitespace-nowrap">
                    "How this is measured"
                    <span class="inline-block ml-1 transition-transform group-open:rotate-180">
                        "\u{25be}"
                    </span>
                </span>
            </summary>
            <div class="px-4 pb-4 pt-3 border-t border-white/10 text-sm text-white/70 leading-relaxed">
                {children()}
            </div>
        </details>
    }
}
