//! Network charts page: Blocks, Adoption, and Transactions in a single scrollable list.
//!
//! All charts render in one flat page with section headings. IntersectionObserver
//! lazy-inits ECharts so offscreen charts don't compute until scrolled into view.

use leptos::prelude::*;
use leptos_meta::*;

use super::components::*;
use super::helpers::chart_desc;
use super::shared::*;
use crate::chart_memo;
use crate::stats::types::uses_daily_aggregates;

/// Network charts page — blocks, adoption, and transaction metrics in one scrollable list.
#[component]
pub fn NetworkChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let custom_from = state.custom_from;
    let custom_to = state.custom_to;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    // All chart signals created at component level so they persist across data refetches.
    // chart_memo! returns empty string when data is None (loading) — individual charts
    // show their own skeleton. This prevents unmounting during range changes, preserving
    // fullscreen and toggle state.

    // ── Blocks ────────────────────────────────────────
    let size_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::block_size_chart(blocks),
        |days| crate::stats::charts::block_size_chart_daily(days)
    );
    let weight_util_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::weight_utilization_chart(blocks),
        |days| crate::stats::charts::weight_utilization_chart_daily(days)
    );
    let tx_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::tx_count_chart(blocks),
        |days| crate::stats::charts::tx_count_chart_daily(days)
    );
    let avg_tx_size_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::avg_tx_size_chart(blocks),
        |days| crate::stats::charts::avg_tx_size_chart_daily(days)
    );
    let interval_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::block_interval_chart(blocks),
        |days| crate::stats::charts::block_interval_chart_daily(days)
    );
    let tps_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::tps_chart(blocks),
        |days| crate::stats::charts::tps_chart_daily(days)
    );

    // Tracked through a `Memo` rather than read untracked. The live stats
    // arrive after the first render, so an untracked read left the disk series
    // permanently absent here too; a memo rebuilds when the number moves
    // rather than on every live tick. Same fix as the single-chart view.
    let disk_size_gb = Memo::new(move |_| {
        state
            .cached_live
            .get()
            .map(|s| s.network.chain_size_gb)
            .unwrap_or(0.0)
    });

    let chain_size_option = Signal::derive(move || {
        let _r = range.get();
        let flags = overlay_flags.get();
        let disk_gb = disk_size_gb.get();
        let offset = state.chain_size_offset.get().unwrap_or(0);
        let chain_total = state.chain_size_total.get().unwrap_or(0);
        dashboard_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| {
                let (mut value, is_daily) = match data {
                    DashboardData::PerBlock(ref blocks) => (
                        crate::stats::charts::chain_size_chart(
                            blocks,
                            disk_gb,
                            offset,
                            chain_total,
                        ),
                        false,
                    ),
                    DashboardData::Daily(ref days) => (
                        crate::stats::charts::chain_size_chart_daily(
                            days,
                            disk_gb,
                            offset,
                            chain_total,
                        ),
                        true,
                    ),
                };
                if value.is_null() {
                    return String::new();
                }
                crate::stats::charts::apply_overlays(
                    &mut value, &flags, is_daily,
                );
                serde_json::to_string(&value).unwrap_or_default()
            })
            .unwrap_or_default()
    });

    let fullness_dist_option = Signal::derive(move || {
        let _r = range.get();
        dashboard_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| {
                let value = match data {
                    DashboardData::PerBlock(ref blocks) => {
                        crate::stats::charts::block_fullness_distribution_chart(
                            blocks,
                        )
                    }
                    DashboardData::Daily(_) => return String::new(),
                };
                serde_json::to_string(&value).unwrap_or_default()
            })
            .unwrap_or_default()
    });
    let fullness_server = LocalResource::new(move || {
        let r = range.get();
        let cf = custom_from.get();
        let ct = custom_to.get();
        async move {
            let n = crate::routes::observatory::helpers::range_to_blocks(&r);
            if !uses_daily_aggregates(n) {
                return None;
            }
            let stats =
                crate::stats::server_fns::fetch_stats_summary().await.ok()?;
            // Resolved rather than derived: a custom window read as the
            // whole chain until 2026-09-21, because range_to_blocks maps
            // "custom" to 999,999.
            let (_, _, from_ts, to_ts) =
                crate::routes::observatory::helpers::resolve_window(
                    &r, cf, ct, &stats,
                )
                .await
                .ok()?;
            let buckets = crate::stats::server_fns::fetch_fullness_histogram(
                from_ts, to_ts,
            )
            .await
            .ok()?;
            let value =
                crate::stats::charts::block_fullness_histogram_from_buckets(
                    &buckets,
                );
            Some(serde_json::to_string(&value).unwrap_or_default())
        }
    });
    let fullness_combined = Signal::derive(move || {
        let local = fullness_dist_option.get();
        if !local.is_empty() {
            return local;
        }
        fullness_server.get().flatten().unwrap_or_default()
    });

    let fullness_dist_pct_option = Signal::derive(move || {
        let _r = range.get();
        dashboard_data.get().and_then(|r| r.ok()).map(|data| {
            let value = match data {
                DashboardData::PerBlock(ref blocks) =>
                    crate::stats::charts::block_fullness_distribution_pct_chart(blocks),
                DashboardData::Daily(_) => return String::new(),
            };
            serde_json::to_string(&value).unwrap_or_default()
        }).unwrap_or_default()
    });
    let fullness_server_pct = LocalResource::new(move || {
        let r = range.get();
        let cf = custom_from.get();
        let ct = custom_to.get();
        async move {
            let n = crate::routes::observatory::helpers::range_to_blocks(&r);
            if !uses_daily_aggregates(n) {
                return None;
            }
            let stats =
                crate::stats::server_fns::fetch_stats_summary().await.ok()?;
            // Resolved rather than derived: a custom window read as the
            // whole chain until 2026-09-21, because range_to_blocks maps
            // "custom" to 999,999.
            let (_, _, from_ts, to_ts) =
                crate::routes::observatory::helpers::resolve_window(
                    &r, cf, ct, &stats,
                )
                .await
                .ok()?;
            let buckets = crate::stats::server_fns::fetch_fullness_histogram(
                from_ts, to_ts,
            )
            .await
            .ok()?;
            let value =
                crate::stats::charts::block_fullness_histogram_from_buckets_pct(
                    &buckets,
                );
            Some(serde_json::to_string(&value).unwrap_or_default())
        }
    });
    let fullness_combined_pct = Signal::derive(move || {
        let local = fullness_dist_pct_option.get();
        if !local.is_empty() {
            return local;
        }
        fullness_server_pct.get().flatten().unwrap_or_default()
    });
    let (fullness_pct_mode, set_fullness_pct_mode) = signal(true);
    let fullness_toggled = Signal::derive(move || {
        if fullness_pct_mode.get() {
            fullness_combined_pct.get()
        } else {
            fullness_combined.get()
        }
    });

    let time_dist_option = Signal::derive(move || {
        let _r = range.get();
        dashboard_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| {
                let value = match data {
                    DashboardData::PerBlock(ref blocks) => {
                        crate::stats::charts::block_time_distribution_chart(
                            blocks,
                        )
                    }
                    DashboardData::Daily(_) => return String::new(),
                };
                serde_json::to_string(&value).unwrap_or_default()
            })
            .unwrap_or_default()
    });
    let time_server = LocalResource::new(move || {
        let r = range.get();
        let cf = custom_from.get();
        let ct = custom_to.get();
        async move {
            let n = crate::routes::observatory::helpers::range_to_blocks(&r);
            if !uses_daily_aggregates(n) {
                return None;
            }
            let stats =
                crate::stats::server_fns::fetch_stats_summary().await.ok()?;
            // Resolved rather than derived: a custom window read as the
            // whole chain until 2026-09-21, because range_to_blocks maps
            // "custom" to 999,999.
            let (_, _, from_ts, to_ts) =
                crate::routes::observatory::helpers::resolve_window(
                    &r, cf, ct, &stats,
                )
                .await
                .ok()?;
            let buckets = crate::stats::server_fns::fetch_block_time_histogram(
                from_ts, to_ts,
            )
            .await
            .ok()?;
            let value = crate::stats::charts::block_time_histogram_from_buckets(
                &buckets,
            );
            Some(serde_json::to_string(&value).unwrap_or_default())
        }
    });
    let time_combined = Signal::derive(move || {
        let local = time_dist_option.get();
        if !local.is_empty() {
            return local;
        }
        time_server.get().flatten().unwrap_or_default()
    });

    let time_dist_pct_option = Signal::derive(move || {
        let _r = range.get();
        dashboard_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| {
                let value = match data {
                    DashboardData::PerBlock(ref blocks) => {
                        crate::stats::charts::block_time_distribution_pct_chart(
                            blocks,
                        )
                    }
                    DashboardData::Daily(_) => return String::new(),
                };
                serde_json::to_string(&value).unwrap_or_default()
            })
            .unwrap_or_default()
    });
    let time_server_pct = LocalResource::new(move || {
        let r = range.get();
        let cf = custom_from.get();
        let ct = custom_to.get();
        async move {
            let n = crate::routes::observatory::helpers::range_to_blocks(&r);
            if !uses_daily_aggregates(n) {
                return None;
            }
            let stats =
                crate::stats::server_fns::fetch_stats_summary().await.ok()?;
            // Resolved rather than derived: a custom window read as the
            // whole chain until 2026-09-21, because range_to_blocks maps
            // "custom" to 999,999.
            let (_, _, from_ts, to_ts) =
                crate::routes::observatory::helpers::resolve_window(
                    &r, cf, ct, &stats,
                )
                .await
                .ok()?;
            let buckets = crate::stats::server_fns::fetch_block_time_histogram(
                from_ts, to_ts,
            )
            .await
            .ok()?;
            let value =
                crate::stats::charts::block_time_histogram_from_buckets_pct(
                    &buckets,
                );
            Some(serde_json::to_string(&value).unwrap_or_default())
        }
    });
    let time_combined_pct = Signal::derive(move || {
        let local = time_dist_pct_option.get();
        if !local.is_empty() {
            return local;
        }
        time_server_pct.get().flatten().unwrap_or_default()
    });
    let (time_pct_mode, set_time_pct_mode) = signal(true);
    let time_toggled = Signal::derive(move || {
        if time_pct_mode.get() {
            time_combined_pct.get()
        } else {
            time_combined.get()
        }
    });

    let propagation_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::block_propagation_chart(blocks),
        |_days| crate::stats::charts::no_data_chart("Rapid Consecutive Blocks")
    );

    let weekday_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::weekday_activity_chart(blocks),
        |days| crate::stats::charts::weekday_activity_chart_daily(days)
    );

    // ── Adoption ──────────────────────────────────────
    let segwit_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::segwit_adoption_chart(blocks),
        |days| crate::stats::charts::segwit_adoption_chart_daily(days)
    );
    let taproot_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::taproot_chart(blocks),
        |days| crate::stats::charts::taproot_chart_daily(days)
    );
    let witness_version_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::witness_version_chart(blocks),
        |days| crate::stats::charts::witness_version_chart_daily(days)
    );
    let witness_pct_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::witness_version_pct_chart(blocks),
        |days| crate::stats::charts::witness_version_pct_chart_daily(days)
    );
    let witness_tx_pct_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::witness_version_tx_pct_chart(blocks),
        |days| crate::stats::charts::witness_version_tx_pct_chart_daily(days)
    );
    let address_type_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::address_type_chart(blocks),
        |days| crate::stats::charts::address_type_chart_daily(days)
    );
    let address_type_pct_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::address_type_pct_chart(blocks),
        |days| crate::stats::charts::address_type_pct_chart_daily(days)
    );
    let taproot_spend_type_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::taproot_spend_type_chart(blocks),
        |days| crate::stats::charts::taproot_spend_type_chart_daily(days)
    );
    let witness_share_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::witness_share_chart(blocks),
        |days| crate::stats::charts::witness_share_chart_daily(days)
    );
    let cumulative_adoption_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::cumulative_adoption_chart(blocks),
        |days| crate::stats::charts::cumulative_adoption_chart_daily(days)
    );
    let multi_velocity_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::multi_velocity_chart(blocks),
        |days| crate::stats::charts::multi_velocity_chart_daily(days)
    );
    let sunset_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::address_sunset_chart(blocks),
        |days| crate::stats::charts::address_sunset_chart_daily(days)
    );

    // ── Transactions ──────────────────────────────────
    let rbf_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::rbf_chart(blocks),
        |days| crate::stats::charts::rbf_chart_daily(days)
    );
    let utxo_flow_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::utxo_flow_chart(blocks),
        |days| crate::stats::charts::utxo_flow_chart_daily(days)
    );
    let batching_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::batching_chart(blocks),
        |days| crate::stats::charts::batching_chart_daily(days)
    );
    let largest_tx_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::largest_tx_chart(blocks),
        |days| crate::stats::charts::largest_tx_chart_daily(days)
    );
    let tx_density_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::tx_density_chart(blocks),
        |days| crate::stats::charts::tx_density_chart_daily(days)
    );
    let utxo_growth_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::utxo_growth_chart(blocks),
        |days| crate::stats::charts::utxo_growth_chart_daily(days)
    );
    let tx_type_evolution_option = chart_memo!(
        dashboard_data,
        range,
        overlay_flags,
        |blocks| crate::stats::charts::tx_type_evolution_chart(blocks),
        |_days| crate::stats::charts::no_data_chart(
            "Transaction Type Evolution"
        )
    );

    view! {
        <Title text="Bitcoin Network Charts: Blocks, Adoption & Transactions | We Hodl BTC"/>
        <Meta name="description" content="Bitcoin network analytics with block size, weight utilization, transaction count, block intervals, chain size growth, SegWit adoption, Taproot usage, witness versions, address types, and RBF trends."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/charts/network"/>
        <ChartPageLayout
            title="Network"
            description="Block size, weight, intervals, adoption trends, and transaction metrics"
            seo_text="Explore Bitcoin's network fundamentals from the genesis block to the latest tip. Block size and weight utilization show how full blocks are relative to the 4 million weight unit consensus limit. Transaction counts and block intervals reveal network throughput and how widely block intervals scatter around their ten-minute average. Chain size growth charts the cumulative blockchain footprint since 2009. Adoption charts track the shift from legacy P2PKH to SegWit and Taproot, including key-path versus script-path spend breakdowns that show how Taproot's privacy and programmability features are actually being used."
        >
            // Error overlay (only on actual errors, not during refetch)
            {move || match dashboard_data.get() {
                Some(Err(_)) => Some(view! {
                    <DataLoadError on_retry=Callback::new(move |_| dashboard_data.refetch())/>
                }),
                _ => None,
            }}

            <div class="space-y-10">
                // ── Blocks ───────────────────────────────
                <SectionHeading id="section-blocks" title="Blocks"/>
                <ChartCard title="Transaction Count" description=chart_desc(range, "Number of transactions included in each block", "Average number of transactions per block each day") chart_id="chart-txcount" option=tx_option/>
                <ChartCard title="Transactions per Second" description=chart_desc(range, "Transactions in each block over the seconds since the previous one", "Each day's transactions over the 86,400 seconds in a day") chart_id="chart-tps" option=tps_option/>
                <ChartCard title="Block Size" description=chart_desc(range, "How large each block is in megabytes", "Average block size per day in megabytes") chart_id="chart-size" option=size_option/>
                <ChartCard title="Weight Utilization" description=chart_desc(range, "How full each block is, as a percentage of the 4 million weight unit limit", "Average daily weight utilization as a percentage of the 4 million weight unit limit") chart_id="chart-weight-util" option=weight_util_option/>
                <ChartCard title="Block Interval" description=chart_desc(range, "Minutes between consecutive blocks. Target is 10 minutes", "Minutes per block implied by each day's block count. Target is 10 minutes") chart_id="chart-interval" option=interval_option/>
                <ChartCard title="Avg Transaction Size" description=chart_desc(range, "Average size of a transaction in bytes. Smaller means more efficient use of block space", "Daily average transaction size in bytes. Smaller means more efficient use of block space") chart_id="chart-avg-tx-size" option=avg_tx_size_option/>
                <ChartCard title="Chain Size Growth" description="Total blockchain size over time, showing how fast the chain is growing" chart_id="chart-chain-size" option=chain_size_option/>
                <ChartCard title="Weekday Activity" description="Average transaction count and fees per block, grouped by UTC day of the week" chart_id="chart-weekday" option=weekday_option/>
                <ChartCard title="Block Fullness Distribution" description="Distribution of blocks by weight utilization percentage. Shows how many blocks are nearly full vs partially empty" chart_id="chart-fullness-dist" option=fullness_toggled>
                    <button
                        class=move || if fullness_pct_mode.get() { "text-xs px-2 py-1 rounded-md cursor-pointer transition-colors bg-[#f7931a]/20 text-[#f7931a]" } else { "text-xs px-2 py-1 rounded-md cursor-pointer transition-colors bg-white/10 text-white/50" }
                        on:click=move |_| set_fullness_pct_mode.update(|v| *v = !*v)
                    >
                        {move || if fullness_pct_mode.get() { "%" } else { "#" }}
                    </button>
                </ChartCard>
                <ChartCard title="Block Time Distribution" description="How long each block waited for the one before it. Ten minutes is the average, not the typical gap" chart_id="chart-time-dist" option=time_toggled>
                    <button
                        class=move || if time_pct_mode.get() { "text-xs px-2 py-1 rounded-md cursor-pointer transition-colors bg-[#f7931a]/20 text-[#f7931a]" } else { "text-xs px-2 py-1 rounded-md cursor-pointer transition-colors bg-white/10 text-white/50" }
                        on:click=move |_| set_time_pct_mode.update(|v| *v = !*v)
                    >
                        {move || if time_pct_mode.get() { "%" } else { "#" }}
                    </button>
                </ChartCard>
                <ChartCard title="Rapid Consecutive Blocks" description="Consecutive blocks whose header timestamps are less than 60 seconds apart. Miners choose those timestamps, so this is not a measure of propagation" chart_id="chart-propagation" option=propagation_option/>

                // ── Adoption ─────────────────────────────
                <SectionHeading id="section-adoption" title="Adoption"/>
                <ChartCard title="SegWit Adoption" description=chart_desc(range, "Percentage of transactions using Segregated Witness", "Daily average SegWit adoption percentage") chart_id="chart-segwit" option=segwit_option/>
                <ChartCard title="Taproot Outputs" description=chart_desc(range, "New Taproot (P2TR) outputs created per block", "Average Taproot (P2TR) outputs created per block each day") chart_id="chart-taproot" option=taproot_option/>
                <ChartCard title="Address Type Evolution" description=chart_desc(range, "Counts of outputs by script type in each block, one band per type", "Daily totals of outputs by script type, one band per type") chart_id="chart-address-types" option=address_type_option/>
                <ChartCard title="Address Type Share" description="Each output type as a percentage of total, showing the shift from legacy to SegWit to Taproot" chart_id="chart-address-types-pct" option=address_type_pct_option/>
                <ChartCard title="Output Type Breakdown" description="Legacy vs SegWit vs Taproot as a percentage of all outputs" chart_id="chart-witness-tx-pct" option=witness_tx_pct_option/>
                <ChartCard title="Witness Version Comparison" description=chart_desc(range, "SegWit v0 (P2WPKH + P2WSH) vs Taproot (P2TR) output counts per block", "Daily average SegWit v0 vs Taproot output counts") chart_id="chart-witness-versions" option=witness_version_option/>
                <ChartCard title="Witness Version Share" description="SegWit v0 vs Taproot as a percentage of all witness outputs" chart_id="chart-witness-pct" option=witness_pct_option/>
                <ChartCard title="Taproot Spend Types" description=chart_desc(range, "Key-path against script-path spends per block. Which spends revealed a script and which revealed nothing", "Daily average key-path against script-path spends. Which spends revealed a script and which revealed nothing") chart_id="chart-taproot-spend-types" option=taproot_spend_type_option/>
                <ChartCard title="Witness Data Share" description="Witness data as percentage of block size. Higher means more SegWit discount savings" chart_id="chart-witness-share" option=witness_share_option/>
                <ChartCard title="Cumulative Adoption" description=chart_desc(range, "Running total of native SegWit v0 and Taproot outputs created within this range. Select ALL for the full stored history", "Running total of native SegWit v0 and Taproot outputs created within this range. Select ALL for the full stored history") chart_id="chart-cumulative-adoption" option=cumulative_adoption_option/>
                <ChartCard title="Adoption Velocity" description="Change in each output type's share of outputs, in percentage points over a trailing window: 144 blocks per block, 30 days daily" chart_id="chart-multi-velocity" option=multi_velocity_option/>
                <ChartCard title="P2PKH Sunset Tracker" description="Decline of legacy P2PKH address usage over time. Horizontal lines mark 10% and 5% thresholds" chart_id="chart-p2pkh-sunset" option=sunset_option/>

                // ── Transactions ─────────────────────────
                <SectionHeading id="section-tx-metrics" title="Transactions"/>
                <ChartCard title="Explicit RBF Signaling" description=chart_desc(range, "Share of each block's transactions whose inputs signal replaceability under BIP 125", "Daily share of transactions signalling replaceability under BIP 125") chart_id="chart-rbf" option=rbf_option/>
                <ChartCard title="UTXO Flow" description=chart_desc(range, "Inputs spent vs outputs created per block. When outputs exceed inputs, the UTXO set grows", "Daily average inputs spent vs outputs created. When outputs exceed inputs, the UTXO set grows") chart_id="chart-utxo-flow" option=utxo_flow_option/>
                <ChartCard title="Transaction Batching" description=chart_desc(range, "Average inputs and outputs per transaction in each block", "Daily average inputs and outputs per transaction") chart_id="chart-batching" option=batching_option/>
                <ChartCard title="Largest Transaction" description=chart_desc(range, "Size of the largest transaction in each block. Large transactions may indicate consolidations or complex scripts", "Largest transaction (per-block ranges only)") chart_id="chart-largest-tx" option=largest_tx_option/>
                <ChartCard title="Transaction Density" description=chart_desc(range, "Transactions per kilobyte of block space. Higher values indicate smaller, more efficient transactions", "Daily average transaction density (transactions per KB)") chart_id="chart-tx-density" option=tx_density_option/>
                <ChartCard title="UTXO Growth Rate" description=chart_desc(range, "Estimated net change in the output set per block, from non-coinbase transactions and excluding detected OP_RETURN. Positive means more outputs were created than consumed", "Estimated net change in the output set per day, from non-coinbase transactions and excluding detected OP_RETURN") chart_id="chart-utxo-growth" option=utxo_growth_option/>
                <ChartCard title="Transaction Type Evolution" description="Breakdown of transactions by input type: Legacy (non-witness), SegWit v0, and Taproot" chart_id="chart-tx-type-evolution" option=tx_type_evolution_option/>
            </div>
        </ChartPageLayout>
    }
}
