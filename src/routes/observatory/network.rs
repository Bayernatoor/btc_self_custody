//! Network charts page with three sub-sections: Blocks, Adoption, and Transactions.
//!
//! **Blocks**: block size, weight utilization, transaction count, TPS, avg tx size,
//! block interval, and chain size growth.
//!
//! **Adoption**: SegWit adoption %, Taproot outputs, witness version comparison and
//! share, output type breakdown, address type evolution and share, Taproot spend
//! types (key-path vs script-path), and witness data share.
//!
//! **Transactions**: RBF adoption, UTXO flow (inputs vs outputs), transaction
//! batching (avg inputs/outputs per tx), and largest transaction per block.
//!
//! Each sub-section only computes its charts when active, keeping page load fast.

use leptos::prelude::*;
use leptos_meta::*;

use super::components::*;
use super::helpers::chart_desc;
use super::shared::*;
use crate::chart_memo;

/// Network charts page with sub-section tabs for blocks, adoption, and transaction metrics.
#[component]
pub fn NetworkChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    // Sub-section navigation — initialized from URL, created OUTSIDE the reactive closure
    let query = leptos_router::hooks::use_query_map();
    let initial_section = query
        .read_untracked()
        .get("section")
        .filter(|s| ["blocks", "adoption", "tx-metrics"].contains(&s.as_str()))
        .unwrap_or_else(|| "blocks".to_string());
    let (section, set_section) = signal(initial_section);

    view! {
        <Title text="Bitcoin Network Charts: Blocks, Adoption & Transactions | WE HODL BTC"/>
        <Meta name="description" content="Bitcoin network analytics with block size, weight utilization, transaction count, block intervals, chain size growth, SegWit adoption, Taproot usage, witness versions, address types, and RBF trends."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/charts/network"/>
        <ChartPageLayout
            title="Network"
            description="Block size, weight, intervals, adoption trends, and transaction metrics"
            seo_text="Explore Bitcoin's network fundamentals from the genesis block to the latest tip. Block size and weight utilization show how full blocks are relative to the 4 MWU consensus limit. Transaction counts and block intervals reveal network throughput and how closely miners track the 10-minute target. Chain size growth charts the cumulative blockchain footprint over 16 years. Adoption charts track the shift from legacy P2PKH to SegWit and Taproot, including key-path versus script-path spend breakdowns that show how Taproot's privacy and programmability features are actually being used."
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
                                    let val = s.value();
                                    set_section.set(val.clone());
                                    #[cfg(feature = "hydrate")]
                                    super::shared::update_section_in_url(
                                        if val == "blocks" { None } else { Some(&val) }
                                    );
                                }
                            }
                        }
                    >
                        <option value="blocks">"Blocks"</option>
                        <option value="adoption">"Adoption"</option>
                        <option value="tx-metrics">"Transactions"</option>
                    </select>
                    <svg class="absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none w-3.5 h-3.5 text-white/40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                    </svg>
                </div>
            }
        >
            // Each sub-section is its own reactive closure — only computes charts
            // when active, so navigating to this page only builds 6 charts (blocks)
            // instead of all 15.

            // --- Blocks sub-section (default, computes on page load) ---
            <Show when=move || section.get() == "blocks" fallback=|| ()>
                {move || match dashboard_data.get() {
                    Some(Ok(_)) => {
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
                        let tps_option = chart_memo!(dashboard_data, range, overlay_flags,
                            |blocks| crate::stats::charts::tps_chart(blocks),
                            |days| crate::stats::charts::tps_chart_daily(days)
                        );
                        // Fetch cumulative block data size before the start of
                        // this range so the chain size chart starts at the right offset.
                        let chain_offset = LocalResource::new(move || {
                            let r = range.get();
                            async move {
                                let n = crate::routes::observatory::helpers::range_to_blocks(&r);
                                let stats = crate::stats::server_fns::fetch_stats_summary().await.ok();
                                let from_height = stats.map(|s| s.min_height.max(s.max_height.saturating_sub(n))).unwrap_or(0);
                                if from_height > 0 {
                                    crate::stats::server_fns::fetch_cumulative_size(from_height).await.unwrap_or(0)
                                } else {
                                    0u64
                                }
                            }
                        });

                        let chain_size_option = Signal::derive(move || {
                            let _r = range.get();
                            let flags = overlay_flags.get();
                            let disk_gb = state.cached_live.get_untracked()
                                .map(|s| s.network.chain_size_gb)
                                .unwrap_or(0.0);
                            let offset = chain_offset.get().unwrap_or(0);
                            dashboard_data.get().and_then(|r| r.ok()).map(|data| {
                                let (mut value, is_daily) = match data {
                                    DashboardData::PerBlock(ref blocks) =>
                                        (crate::stats::charts::chain_size_chart(blocks, disk_gb, offset), false),
                                    DashboardData::Daily(ref days) =>
                                        (crate::stats::charts::chain_size_chart_daily(days, disk_gb, offset), true),
                                };
                                if value.is_null() { return String::new(); }
                                crate::stats::charts::apply_overlays(&mut value, &flags, is_daily);
                                serde_json::to_string(&value).unwrap_or_default()
                            }).unwrap_or_default()
                        });

                        // Histograms use server-side computation so they work on ALL range.
                        // On per-block ranges: compute from local data (faster).
                        // On daily ranges: fetch from server SQL query.
                        let fullness_dist_option = Signal::derive(move || {
                            let _r = range.get();
                            dashboard_data.get().and_then(|r| r.ok()).map(|data| {
                                let value = match data {
                                    DashboardData::PerBlock(ref blocks) =>
                                        crate::stats::charts::block_fullness_distribution_chart(blocks),
                                    DashboardData::Daily(_) => return String::new(),
                                };
                                serde_json::to_string(&value).unwrap_or_default()
                            }).unwrap_or_default()
                        });
                        let fullness_server = LocalResource::new(move || {
                            let r = range.get();
                            async move {
                                let n = crate::routes::observatory::helpers::range_to_blocks(&r);
                                if n <= 5_000 { return None; }
                                let stats = crate::stats::server_fns::fetch_stats_summary().await.ok()?;
                                let from_ts = stats.latest_timestamp.saturating_sub(n * 600);
                                let buckets = crate::stats::server_fns::fetch_fullness_histogram(from_ts, stats.latest_timestamp).await.ok()?;
                                let value = crate::stats::charts::block_fullness_histogram_from_buckets(&buckets);
                                Some(serde_json::to_string(&value).unwrap_or_default())
                            }
                        });
                        let fullness_combined = Signal::derive(move || {
                            let local = fullness_dist_option.get();
                            if !local.is_empty() { return local; }
                            fullness_server.get().flatten().unwrap_or_default()
                        });

                        let time_dist_option = Signal::derive(move || {
                            let _r = range.get();
                            dashboard_data.get().and_then(|r| r.ok()).map(|data| {
                                let value = match data {
                                    DashboardData::PerBlock(ref blocks) =>
                                        crate::stats::charts::block_time_distribution_chart(blocks),
                                    DashboardData::Daily(_) => return String::new(),
                                };
                                serde_json::to_string(&value).unwrap_or_default()
                            }).unwrap_or_default()
                        });
                        let time_server = LocalResource::new(move || {
                            let r = range.get();
                            async move {
                                let n = crate::routes::observatory::helpers::range_to_blocks(&r);
                                if n <= 5_000 { return None; }
                                let stats = crate::stats::server_fns::fetch_stats_summary().await.ok()?;
                                let from_ts = stats.latest_timestamp.saturating_sub(n * 600);
                                let buckets = crate::stats::server_fns::fetch_block_time_histogram(from_ts, stats.latest_timestamp).await.ok()?;
                                let value = crate::stats::charts::block_time_histogram_from_buckets(&buckets);
                                Some(serde_json::to_string(&value).unwrap_or_default())
                            }
                        });
                        let time_combined = Signal::derive(move || {
                            let local = time_dist_option.get();
                            if !local.is_empty() { return local; }
                            time_server.get().flatten().unwrap_or_default()
                        });

                        let propagation_option = Signal::derive(move || {
                            let _r = range.get();
                            dashboard_data.get().and_then(|r| r.ok()).map(|data| {
                                let value = match data {
                                    DashboardData::PerBlock(ref blocks) =>
                                        crate::stats::charts::block_propagation_chart(blocks),
                                    DashboardData::Daily(_) =>
                                        crate::stats::charts::no_data_chart("Rapid Consecutive Blocks"),
                                };
                                serde_json::to_string(&value).unwrap_or_default()
                            }).unwrap_or_default()
                        });

                        view! {
                            <div class="space-y-10">
                                <ChartCard title="Block Size" description=chart_desc(range, "How large each block is in megabytes", "Average block size per day in megabytes") chart_id="chart-size" option=size_option/>
                                <ChartCard title="Weight Utilization" description=chart_desc(range, "How full each block is, as a percentage of the 4 MWU limit", "Average daily weight utilization as a percentage of the 4 MWU limit") chart_id="chart-weight-util" option=weight_util_option/>
                                <ChartCard title="Transaction Count" description=chart_desc(range, "Number of transactions included in each block", "Average number of transactions per block each day") chart_id="chart-txcount" option=tx_option/>
                                <ChartCard title="Transactions per Second" description=chart_desc(range, "Average TPS calculated from transactions per block interval", "Daily average transactions per second across all blocks") chart_id="chart-tps" option=tps_option/>
                                <ChartCard title="Avg Transaction Size" description=chart_desc(range, "Average size of a transaction in bytes. Smaller means more efficient use of block space", "Daily average transaction size in bytes. Smaller means more efficient use of block space") chart_id="chart-avg-tx-size" option=avg_tx_size_option/>
                                <ChartCard title="Block Interval" description=chart_desc(range, "Minutes between consecutive blocks. Target is 10 minutes", "Average daily block interval in minutes. Target is 10 minutes") chart_id="chart-interval" option=interval_option/>
                                <ChartCard title="Chain Size Growth" description="Total blockchain size over time, showing how fast the chain is growing" chart_id="chart-chain-size" option=chain_size_option/>
                                <ChartCard title="Block Fullness Distribution" description="Distribution of blocks by weight utilization percentage. Shows how many blocks are nearly full vs partially empty" chart_id="chart-fullness-dist" option=fullness_combined/>
                                <ChartCard title="Block Time Distribution" description="Distribution of time between consecutive blocks. Most cluster near the 10-minute target" chart_id="chart-time-dist" option=time_combined/>
                                <ChartCard title="Rapid Consecutive Blocks" description="Blocks arriving within 60 seconds of each other, indicating fast mining luck or potential stale block races" chart_id="chart-propagation" option=propagation_option/>
                            </div>
                        }.into_any()
                    }
                    Some(Err(_)) => view! {
                        <DataLoadError on_retry=Callback::new(move |_| dashboard_data.refetch())/>
                    }.into_any(),
                    None => view! { <ChartPageSkeleton count=6/> }.into_any(),
                }}
            </Show>

            // --- Adoption sub-section (only computed when tab clicked) ---
            <Show when=move || section.get() == "adoption" fallback=|| ()>
                {move || match dashboard_data.get() {
                    Some(Ok(_)) => {
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

                        let taproot_velocity_option = chart_memo!(dashboard_data, range, overlay_flags,
                            |blocks| crate::stats::charts::taproot_velocity_chart(blocks),
                            |days| crate::stats::charts::taproot_velocity_chart_daily(days)
                        );
                        let cumulative_adoption_option = chart_memo!(dashboard_data, range, overlay_flags,
                            |blocks| crate::stats::charts::cumulative_adoption_chart(blocks),
                            |days| crate::stats::charts::cumulative_adoption_chart_daily(days)
                        );

                        view! {
                            <div class="space-y-10">
                                <ChartCard title="SegWit Adoption" description=chart_desc(range, "Percentage of transactions using Segregated Witness", "Daily average SegWit adoption percentage") chart_id="chart-segwit" option=segwit_option/>
                                <ChartCard title="Taproot Outputs" description=chart_desc(range, "New Taproot (P2TR) outputs created per block", "Average Taproot (P2TR) outputs created per block each day") chart_id="chart-taproot" option=taproot_option/>
                                <ChartCard title="Witness Version Comparison" description=chart_desc(range, "SegWit v0 (P2WPKH + P2WSH) vs Taproot (P2TR) output counts per block", "Daily average SegWit v0 vs Taproot output counts") chart_id="chart-witness-versions" option=witness_version_option/>
                                <ChartCard title="Witness Version Share" description="SegWit v0 vs Taproot as a percentage of all witness outputs" chart_id="chart-witness-pct" option=witness_pct_option/>
                                <ChartCard title="Output Type Breakdown" description="Legacy vs SegWit vs Taproot as a percentage of all outputs" chart_id="chart-witness-tx-pct" option=witness_tx_pct_option/>
                                <ChartCard title="Address Type Evolution" description=chart_desc(range, "Output types per block. Watch P2PKH (legacy) shrink as P2WPKH (SegWit) and P2TR (Taproot) grow", "Daily average output types. Watch P2PKH (legacy) shrink as P2WPKH (SegWit) and P2TR (Taproot) grow") chart_id="chart-address-types" option=address_type_option/>
                                <ChartCard title="Address Type Share" description="Each output type as a percentage of total, showing the shift from legacy to SegWit to Taproot" chart_id="chart-address-types-pct" option=address_type_pct_option/>
                                <ChartCard title="Taproot Spend Types" description=chart_desc(range, "Key-path vs script-path spends per block. How Taproot is actually being used", "Daily average key-path vs script-path spends. How Taproot is actually being used") chart_id="chart-taproot-spend-types" option=taproot_spend_type_option/>
                                <ChartCard title="Witness Data Share" description="Witness data as percentage of block size. Higher means more SegWit discount savings" chart_id="chart-witness-share" option=witness_share_option/>
                                <ChartCard title="Taproot Adoption Velocity" description=chart_desc(range, "Rate of change in Taproot output percentage. Positive values indicate accelerating adoption", "Daily rate of change in Taproot adoption percentage") chart_id="chart-taproot-velocity" option=taproot_velocity_option/>
                                <ChartCard title="Cumulative Adoption" description=chart_desc(range, "Running total of SegWit transactions and Taproot outputs within this range. Select ALL for lifetime totals", "Cumulative SegWit and Taproot counts within this range. Select ALL for lifetime totals") chart_id="chart-cumulative-adoption" option=cumulative_adoption_option/>
                            </div>
                        }.into_any()
                    }
                    Some(Err(_)) => view! {
                        <DataLoadError on_retry=Callback::new(move |_| dashboard_data.refetch())/>
                    }.into_any(),
                    None => view! { <ChartPageSkeleton count=9/> }.into_any(),
                }}
            </Show>

            // --- Transaction Metrics sub-section (only computed when tab clicked) ---
            <Show when=move || section.get() == "tx-metrics" fallback=|| ()>
                {move || match dashboard_data.get() {
                    Some(Ok(_)) => {
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
                        let largest_tx_option = chart_memo!(dashboard_data, range, overlay_flags,
                            |blocks| crate::stats::charts::largest_tx_chart(blocks),
                            |days| crate::stats::charts::largest_tx_chart_daily(days)
                        );

                        let tx_density_option = chart_memo!(dashboard_data, range, overlay_flags,
                            |blocks| crate::stats::charts::tx_density_chart(blocks),
                            |days| crate::stats::charts::tx_density_chart_daily(days)
                        );
                        let utxo_growth_option = chart_memo!(dashboard_data, range, overlay_flags,
                            |blocks| crate::stats::charts::utxo_growth_chart(blocks),
                            |days| crate::stats::charts::utxo_growth_chart_daily(days)
                        );

                        view! {
                            <div class="space-y-10">
                                <ChartCard title="RBF Adoption" description=chart_desc(range, "Percentage of transactions opting into Replace-By-Fee per block", "Daily average RBF adoption percentage") chart_id="chart-rbf" option=rbf_option/>
                                <ChartCard title="UTXO Flow" description=chart_desc(range, "Inputs spent vs outputs created per block. When outputs exceed inputs, the UTXO set grows", "Daily average inputs spent vs outputs created. When outputs exceed inputs, the UTXO set grows") chart_id="chart-utxo-flow" option=utxo_flow_option/>
                                <ChartCard title="Transaction Batching" description=chart_desc(range, "Average inputs and outputs per transaction in each block", "Daily average inputs and outputs per transaction") chart_id="chart-batching" option=batching_option/>
                                <ChartCard title="Largest Transaction" description=chart_desc(range, "Size of the largest transaction in each block. Large transactions may indicate consolidations or complex scripts", "Largest transaction (per-block ranges only)") chart_id="chart-largest-tx" option=largest_tx_option/>
                                <ChartCard title="Transaction Density" description=chart_desc(range, "Transactions per kilobyte of block space. Higher values indicate smaller, more efficient transactions", "Daily average transaction density (transactions per KB)") chart_id="chart-tx-density" option=tx_density_option/>
                                <ChartCard title="UTXO Growth Rate" description=chart_desc(range, "Net UTXO set change per block (outputs created minus inputs consumed). Positive means the UTXO set is growing, negative means consolidation", "Daily net UTXO change across all blocks") chart_id="chart-utxo-growth" option=utxo_growth_option/>
                            </div>
                        }.into_any()
                    }
                    Some(Err(_)) => view! {
                        <DataLoadError on_retry=Callback::new(move |_| dashboard_data.refetch())/>
                    }.into_any(),
                    None => view! { <ChartPageSkeleton count=3/> }.into_any(),
                }}
            </Show>
        </ChartPageLayout>
    }
}
