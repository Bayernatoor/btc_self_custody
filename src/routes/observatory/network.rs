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

/// Network charts page — blocks, adoption, and transaction metrics in one scrollable list.
#[component]
pub fn NetworkChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    view! {
        <Title text="Bitcoin Network Charts: Blocks, Adoption & Transactions | WE HODL BTC"/>
        <Meta name="description" content="Bitcoin network analytics with block size, weight utilization, transaction count, block intervals, chain size growth, SegWit adoption, Taproot usage, witness versions, address types, and RBF trends."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/charts/network"/>
        <ChartPageLayout
            title="Network"
            description="Block size, weight, intervals, adoption trends, and transaction metrics"
            seo_text="Explore Bitcoin's network fundamentals from the genesis block to the latest tip. Block size and weight utilization show how full blocks are relative to the 4 million weight unit consensus limit. Transaction counts and block intervals reveal network throughput and how closely miners track the 10-minute target. Chain size growth charts the cumulative blockchain footprint over 16 years. Adoption charts track the shift from legacy P2PKH to SegWit and Taproot, including key-path versus script-path spend breakdowns that show how Taproot's privacy and programmability features are actually being used."
        >
            {move || match dashboard_data.get() {
                Some(Ok(_)) => {
                    // ── Blocks ────────────────────────────────────────
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

                    let propagation_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::block_propagation_chart(blocks),
                        |_days| crate::stats::charts::no_data_chart("Rapid Consecutive Blocks")
                    );

                    let weekday_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::weekday_activity_chart(blocks),
                        |days| crate::stats::charts::weekday_activity_chart_daily(days)
                    );

                    // ── Adoption ──────────────────────────────────────
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
                    let cumulative_adoption_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::cumulative_adoption_chart(blocks),
                        |days| crate::stats::charts::cumulative_adoption_chart_daily(days)
                    );
                    let multi_velocity_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::multi_velocity_chart(blocks),
                        |days| crate::stats::charts::multi_velocity_chart_daily(days)
                    );
                    let sunset_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::address_sunset_chart(blocks),
                        |days| crate::stats::charts::address_sunset_chart_daily(days)
                    );

                    // ── Transactions ──────────────────────────────────
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
                    let tx_type_evolution_option = chart_memo!(dashboard_data, range, overlay_flags,
                        |blocks| crate::stats::charts::tx_type_evolution_chart(blocks),
                        |_days| crate::stats::charts::no_data_chart("Transaction Type Evolution")
                    );

                    view! {
                        <div class="space-y-10">
                            // ── Blocks ───────────────────────────────
                            <SectionHeading id="section-blocks" title="Blocks"/>
                            <ChartCard title="Transaction Count" description=chart_desc(range, "Number of transactions included in each block", "Average number of transactions per block each day") chart_id="chart-txcount" option=tx_option/>
                            <ChartCard title="Transactions per Second" description=chart_desc(range, "Average TPS calculated from transactions per block interval", "Daily average transactions per second across all blocks") chart_id="chart-tps" option=tps_option info="Bitcoin TPS is calculated per block: tx_count / block_interval_seconds. Unlike traditional payment networks, Bitcoin's TPS varies with block time luck. A 5-minute block with 3,000 txs shows 10 TPS, while the same txs in a 20-minute block shows 2.5 TPS."/>
                            <ChartCard title="Block Size" description=chart_desc(range, "How large each block is in megabytes", "Average block size per day in megabytes") chart_id="chart-size" option=size_option/>
                            <ChartCard title="Weight Utilization" description=chart_desc(range, "How full each block is, as a percentage of the 4 million weight unit limit", "Average daily weight utilization as a percentage of the 4 million weight unit limit") chart_id="chart-weight-util" option=weight_util_option info="The consensus limit is 4,000,000 weight units (4 MWU) per block. Witness data gets a 75% discount, so a block full of SegWit transactions can fit more data than one full of legacy transactions. Consistently high utilization (>90%) means demand for block space is near capacity."/>
                            <ChartCard title="Block Interval" description=chart_desc(range, "Minutes between consecutive blocks. Target is 10 minutes", "Average daily block interval in minutes. Target is 10 minutes") chart_id="chart-interval" option=interval_option info="Block intervals follow a Poisson distribution with a 10-minute average. Short intervals (<1 min) are common and normal. Long intervals (>30 min) happen roughly once per day. Difficulty adjusts every 2,016 blocks to keep the average at 10 minutes."/>
                            <ChartCard title="Avg Transaction Size" description=chart_desc(range, "Average size of a transaction in bytes. Smaller means more efficient use of block space", "Daily average transaction size in bytes. Smaller means more efficient use of block space") chart_id="chart-avg-tx-size" option=avg_tx_size_option info="SegWit and Taproot transactions are typically smaller than legacy because they move signature data to the witness section (which gets a weight discount). A declining trend indicates the network is using block space more efficiently."/>
                            <ChartCard title="Chain Size Growth" description="Total blockchain size over time, showing how fast the chain is growing" chart_id="chart-chain-size" option=chain_size_option/>
                            <ChartCard title="Weekday Activity" description="Average transaction count and fees by day of week. Reveals patterns between weekday and weekend network usage" chart_id="chart-weekday" option=weekday_option/>
                            <ChartCard title="Block Fullness Distribution" description="Distribution of blocks by weight utilization percentage. Shows how many blocks are nearly full vs partially empty" chart_id="chart-fullness-dist" option=fullness_combined info="A histogram of block fullness. Most modern blocks cluster near 100% because miners maximize fee revenue. Empty or near-empty blocks usually appear right after a new block is found (before the miner has received transactions). A spike at lower percentages may indicate a fee market shift."/>
                            <ChartCard title="Block Time Distribution" description="Distribution of time between consecutive blocks. Most cluster near the 10-minute target" chart_id="chart-time-dist" option=time_combined info="Shows how block intervals are distributed. The theoretical distribution is exponential with a 10-minute mean. Most blocks arrive within 20 minutes, but the long tail extends to 60+ minutes. This is normal Poisson process behavior, not a network problem."/>
                            <ChartCard title="Rapid Consecutive Blocks" description="Blocks arriving within 60 seconds of each other, indicating fast mining luck or potential stale block races" chart_id="chart-propagation" option=propagation_option/>

                            // ── Adoption ─────────────────────────────
                            <SectionHeading id="section-adoption" title="Adoption"/>
                            <ChartCard title="SegWit Adoption" description=chart_desc(range, "Percentage of transactions using Segregated Witness", "Daily average SegWit adoption percentage") chart_id="chart-segwit" option=segwit_option info="Counts the percentage of non-coinbase transactions that have at least one witness input. SegWit activated in August 2017 (block 481,824). Adoption grew slowly at first as wallets upgraded, then accelerated. Above 95% means nearly all transactions benefit from the witness discount."/>
                            <ChartCard title="Taproot Outputs" description=chart_desc(range, "New Taproot (P2TR) outputs created per block", "Average Taproot (P2TR) outputs created per block each day") chart_id="chart-taproot" option=taproot_option/>
                            <ChartCard title="Address Type Evolution" description=chart_desc(range, "Output types per block. Watch P2PKH (legacy) shrink as P2WPKH (SegWit) and P2TR (Taproot) grow", "Daily average output types. Watch P2PKH (legacy) shrink as P2WPKH (SegWit) and P2TR (Taproot) grow") chart_id="chart-address-types" option=address_type_option/>
                            <ChartCard title="Address Type Share" description="Each output type as a percentage of total, showing the shift from legacy to SegWit to Taproot" chart_id="chart-address-types-pct" option=address_type_pct_option/>
                            <ChartCard title="Output Type Breakdown" description="Legacy vs SegWit vs Taproot as a percentage of all outputs" chart_id="chart-witness-tx-pct" option=witness_tx_pct_option/>
                            <ChartCard title="Witness Version Comparison" description=chart_desc(range, "SegWit v0 (P2WPKH + P2WSH) vs Taproot (P2TR) output counts per block", "Daily average SegWit v0 vs Taproot output counts") chart_id="chart-witness-versions" option=witness_version_option/>
                            <ChartCard title="Witness Version Share" description="SegWit v0 vs Taproot as a percentage of all witness outputs" chart_id="chart-witness-pct" option=witness_pct_option/>
                            <ChartCard title="Taproot Spend Types" description=chart_desc(range, "Key-path vs script-path spends per block. How Taproot is actually being used", "Daily average key-path vs script-path spends. How Taproot is actually being used") chart_id="chart-taproot-spend-types" option=taproot_spend_type_option info="Key-path spends look like regular single-sig transactions on-chain (privacy win). Script-path spends reveal that a more complex script was involved (multisig, timelocks, etc.). A high key-path ratio means Taproot's privacy benefits are being realized."/>
                            <ChartCard title="Witness Data Share" description="Witness data as percentage of block size. Higher means more SegWit discount savings" chart_id="chart-witness-share" option=witness_share_option info="Witness data receives a 75% weight discount under SegWit rules. A higher witness share means more of the block is discounted data, effectively increasing the block's capacity beyond the old 1 MB limit. Modern blocks typically have 60-70% witness data."/>
                            <ChartCard title="Cumulative Adoption" description=chart_desc(range, "Running total of SegWit transactions and Taproot outputs within this range. Select ALL for lifetime totals", "Cumulative SegWit and Taproot counts within this range. Select ALL for lifetime totals") chart_id="chart-cumulative-adoption" option=cumulative_adoption_option/>
                            <ChartCard title="Adoption Velocity" description="Rate of change for major address types. P2PKH declining, P2WPKH flattening, P2TR growing. Shows the transition between eras" chart_id="chart-multi-velocity" option=multi_velocity_option info="Shows the 30-day rate of change for each address type's share. Positive values mean the type is gaining share, negative means declining. When P2TR velocity is positive and P2PKH is negative, Taproot is actively replacing legacy usage."/>
                            <ChartCard title="P2PKH Sunset Tracker" description="Decline of legacy P2PKH address usage over time. Horizontal lines mark 10% and 5% thresholds" chart_id="chart-p2pkh-sunset" option=sunset_option info="Tracks how quickly P2PKH (legacy '1' addresses) are being phased out. The 90-day moving average smooths out noise. When it crosses below 10% and 5% thresholds, it signals that the Bitcoin ecosystem has largely moved to modern address formats."/>

                            // ── Transactions ─────────────────────────
                            <SectionHeading id="section-tx-metrics" title="Transactions"/>
                            <ChartCard title="RBF Adoption" description=chart_desc(range, "Percentage of transactions opting into Replace-By-Fee per block", "Daily average RBF adoption percentage") chart_id="chart-rbf" option=rbf_option info="Replace-By-Fee (BIP 125) lets senders bump fees on unconfirmed transactions. A transaction signals RBF by setting at least one input's sequence number below 0xfffffffe. Higher adoption means more users/wallets support fee bumping, which is important during fee spikes."/>
                            <ChartCard title="UTXO Flow" description=chart_desc(range, "Inputs spent vs outputs created per block. When outputs exceed inputs, the UTXO set grows", "Daily average inputs spent vs outputs created. When outputs exceed inputs, the UTXO set grows") chart_id="chart-utxo-flow" option=utxo_flow_option info="Every transaction consumes UTXOs (inputs) and creates new ones (outputs). When outputs exceed inputs, the UTXO set grows, increasing the memory requirements for full nodes. Consolidation transactions (many inputs, few outputs) shrink the set."/>
                            <ChartCard title="Transaction Batching" description=chart_desc(range, "Average inputs and outputs per transaction in each block", "Daily average inputs and outputs per transaction") chart_id="chart-batching" option=batching_option info="Higher output counts per transaction indicate batching, where exchanges and services combine multiple payments into one transaction. This is more efficient use of block space. A typical non-batched transaction has 1-2 inputs and 2 outputs (payment + change)."/>
                            <ChartCard title="Largest Transaction" description=chart_desc(range, "Size of the largest transaction in each block. Large transactions may indicate consolidations or complex scripts", "Largest transaction (per-block ranges only)") chart_id="chart-largest-tx" option=largest_tx_option/>
                            <ChartCard title="Transaction Density" description=chart_desc(range, "Transactions per kilobyte of block space. Higher values indicate smaller, more efficient transactions", "Daily average transaction density (transactions per KB)") chart_id="chart-tx-density" option=tx_density_option info="More transactions per KB means the average transaction is smaller and block space is used more efficiently. SegWit and Taproot improve density by moving signatures to the discounted witness section. A rising trend indicates the network is getting more efficient over time."/>
                            <ChartCard title="UTXO Growth Rate" description=chart_desc(range, "Net UTXO set change per block (outputs created minus inputs consumed). Positive means the UTXO set is growing, negative means consolidation", "Daily net UTXO change across all blocks") chart_id="chart-utxo-growth" option=utxo_growth_option/>
                            <ChartCard title="Transaction Type Evolution" description="Breakdown of transactions by input type: Legacy (non-witness), SegWit v0, and Taproot" chart_id="chart-tx-type-evolution" option=tx_type_evolution_option coming_soon=true/>
                        </div>
                    }.into_any()
                }
                Some(Err(_)) => view! {
                    <DataLoadError on_retry=Callback::new(move |_| dashboard_data.refetch())/>
                }.into_any(),
                None => view! { <ChartPageSkeleton count=6/> }.into_any(),
            }}
        </ChartPageLayout>
    }
}
