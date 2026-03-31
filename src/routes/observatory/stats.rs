//! Stats Summary Dashboard — at-a-glance counters for any time range.

use leptos::prelude::*;
use leptos_meta::*;

use super::helpers::*;
use super::shared::ObservatoryState;
use crate::stats::server_fns::*;
use crate::stats::types::{MiningPriceSummary, RangeSummary};

// ---------------------------------------------------------------------------
// Stat card component
// ---------------------------------------------------------------------------

#[component]
fn StatCard(
    #[prop(into)] label: &'static str,
    #[prop(into)] value: Signal<String>,
    #[prop(optional, into)] sub: Option<Signal<String>>,
    #[prop(optional, into)] tooltip: Option<&'static str>,
) -> impl IntoView {
    view! {
        <div
            class="bg-[#0d2137] border border-white/10 rounded-xl p-3 sm:p-4"
            title=tooltip.unwrap_or("")
        >
            <p class="text-[10px] sm:text-xs text-white/40 uppercase tracking-wider mb-1">{label}</p>
            <p class="text-base sm:text-lg font-semibold text-[#f7931a] font-mono truncate" title=move || value.get()>
                {move || value.get()}
            </p>
            {sub.map(|s| view! {
                <p class="text-[10px] sm:text-xs text-white/30 mt-0.5 truncate">{move || s.get()}</p>
            })}
        </div>
    }
}

/// Section header within the stats grid.
#[component]
fn SectionHeader(#[prop(into)] title: &'static str) -> impl IntoView {
    view! {
        <div class="col-span-2 sm:col-span-3 lg:col-span-4 mt-4 first:mt-0">
            <h2 class="text-sm font-semibold text-white/60 uppercase tracking-wider border-b border-white/10 pb-2">{title}</h2>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Stats Summary page
// ---------------------------------------------------------------------------

#[component]
pub fn StatsSummaryPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;

    let custom_from = state.custom_from;
    let custom_to = state.custom_to;

    // Compute timestamp range from the selected preset or custom dates
    let ts_range = Signal::derive(move || {
        let r = range.get();
        let now = chrono::Utc::now().timestamp() as u64;
        if r == "custom" {
            let from = custom_from.get()
                .and_then(|s| super::shared::date_to_ts(&s))
                .unwrap_or(0);
            let to = custom_to.get()
                .and_then(|s| super::shared::date_to_ts(&s))
                .map(|t| t + 86_399)
                .unwrap_or(now);
            return (from, to);
        }
        let n = range_to_blocks(&r);
        let seconds = n * 600;
        let from = if n >= 999_999 { 0 } else { now.saturating_sub(seconds) };
        (from, now)
    });

    // Fetch summary data
    let summary = LocalResource::new(move || {
        let (from, to) = ts_range.get();
        async move { fetch_range_summary(from, to).await.ok() }
    });

    let data = Signal::derive(move || summary.get().flatten());

    // Fetch mining + price context
    let mining_price = LocalResource::new(move || {
        let (from, to) = ts_range.get();
        async move { fetch_mining_price_summary(from, to).await.ok() }
    });
    let mp_data = Signal::derive(move || mining_price.get().flatten());

    // Format helper — creates a Signal<String> from a RangeSummary field
    let stat = move |f: fn(&RangeSummary) -> String| -> Signal<String> {
        let d = data;
        Signal::derive(move || d.get().map(|s| f(&s)).unwrap_or_else(|| "\u{2014}".to_string()))
    };
    let mp_stat = move |f: fn(&MiningPriceSummary) -> String| -> Signal<String> {
        let d = mp_data;
        Signal::derive(move || d.get().map(|s| f(&s)).unwrap_or_else(|| "\u{2014}".to_string()))
    };

    // Non-coinbase tx count (for percentages that exclude coinbase)
    let user_tx = move |s: &RangeSummary| -> u64 {
        s.total_tx.saturating_sub(s.block_count)
    };

    // === Network ===
    let blocks = stat(|s| format_number(s.block_count));
    let txs = stat(|s| format_compact(s.total_tx));
    let txs_sub = stat(|s| format!("{} total (incl. coinbase)", format_number(s.total_tx)));
    let avg_size = stat(|s| {
        if s.block_count > 0 {
            format!("{:.2} MB", s.total_size as f64 / s.block_count as f64 / 1_000_000.0)
        } else { "\u{2014}".to_string() }
    });
    let avg_block_time = stat(|s| {
        let total_secs = (s.avg_block_time * 60.0).round() as u64;
        format!("{}:{:02}", total_secs / 60, total_secs % 60)
    });
    let avg_tx_per_block = stat(|s| {
        if s.block_count > 0 {
            format_number_f64(s.total_tx as f64 / s.block_count as f64, 1)
        } else { "\u{2014}".to_string() }
    });
    let weight_util = stat(|s| {
        if s.block_count > 0 {
            format!("{:.1}%", s.total_weight as f64 / s.block_count as f64 / 4_000_000.0 * 100.0)
        } else { "\u{2014}".to_string() }
    });
    let chain_growth = stat(|s| format_data_size(s.total_size));

    // === Fees ===
    let total_fees_btc = stat(|s| format!("{} BTC", format_number_f64(s.total_fees as f64 / 100_000_000.0, 2)));
    let total_fees_sub = stat(|s| format!("{} sats", format_number(s.total_fees)));
    let avg_fee_rate = stat(|s| format!("{:.1} sat/vB", s.avg_fee_rate));
    let avg_fee_per_tx = {
        let d = data;
        Signal::derive(move || d.get().map(|s| {
            let utx = user_tx(&s);
            if utx > 0 {
                format!("{} sats", format_number((s.total_fees as f64 / utx as f64).round() as u64))
            } else { "\u{2014}".to_string() }
        }).unwrap_or_else(|| "\u{2014}".to_string()))
    };
    let avg_median_fee = stat(|s| format!("{} sats", format_number(s.avg_median_fee.round() as u64)));

    // === Adoption ===
    let segwit_pct = {
        let d = data;
        Signal::derive(move || d.get().map(|s| {
            let utx = user_tx(&s);
            if utx > 0 {
                format!("{:.1}%", s.total_segwit_txs as f64 / utx as f64 * 100.0)
            } else { "\u{2014}".to_string() }
        }).unwrap_or_else(|| "\u{2014}".to_string()))
    };
    let taproot_outputs = stat(|s| format_compact(s.total_taproot_outputs));
    let taproot_sub = stat(|s| {
        format!("Key-path: {}  |  Script-path: {}",
            format_compact(s.total_taproot_keypath),
            format_compact(s.total_taproot_scriptpath))
    });
    let witness_pct = stat(|s| format!("{:.1}%", s.witness_pct));
    let total_inputs = stat(|s| format_compact(s.total_inputs));
    let total_outputs = stat(|s| format_compact(s.total_outputs));
    let rbf_pct = {
        let d = data;
        Signal::derive(move || d.get().map(|s| {
            let utx = user_tx(&s);
            if utx > 0 {
                format!("{:.1}%", s.total_rbf as f64 / utx as f64 * 100.0)
            } else { "\u{2014}".to_string() }
        }).unwrap_or_else(|| "\u{2014}".to_string()))
    };

    // === Embedded Data ===
    let inscriptions = stat(|s| format_compact(s.total_inscriptions));
    let inscriptions_sub = stat(|s| format!("{} data", format_data_size(s.total_inscription_bytes)));
    let brc20 = stat(|s| format_compact(s.total_brc20));
    let brc20_sub = stat(|s| {
        if s.total_inscriptions > 0 {
            format!("{:.1}% of inscriptions", s.total_brc20 as f64 / s.total_inscriptions as f64 * 100.0)
        } else { String::new() }
    });
    let runes = stat(|s| format_compact(s.total_runes));
    let runes_sub = stat(|s| format!("{} data", format_data_size(s.total_runes_bytes)));
    let op_return = stat(|s| format_compact(s.total_op_return_count));
    let op_return_sub = stat(|s| format!("{} data", format_data_size(s.total_op_return_bytes)));
    let omni = stat(|s| format_compact(s.total_omni));
    let counterparty = stat(|s| format_compact(s.total_counterparty));

    // === Extremes ===
    let max_block_size = stat(|s| format!("{:.2} MB", s.max_block_size as f64 / 1_000_000.0));
    let max_block_fees = stat(|s| format!("{} BTC", format_number_f64(s.max_block_fees as f64 / 100_000_000.0, 4)));

    // === Volume ===
    let total_btc_transferred = stat(|s| {
        if s.total_output_value > 0 {
            format!("{} BTC", format_number_f64(s.total_output_value as f64 / 100_000_000.0, 2))
        } else {
            "backfill required".to_string()
        }
    });

    // === Mining ===
    let top_pool = mp_stat(|s| s.top_pool_name.clone());
    let top_pool_sub = mp_stat(|s| format!("{} blocks ({:.1}%)", format_number(s.top_pool_blocks), s.top_pool_pct));
    let pool_count = mp_stat(|s| format_number(s.pool_count));
    let empty_blocks = stat(|s| format_number(s.empty_block_count));
    let empty_sub = stat(|s| {
        if s.block_count > 0 {
            format!("{:.2}% of blocks", s.empty_block_count as f64 / s.block_count as f64 * 100.0)
        } else { String::new() }
    });

    // === Price ===
    let price_start = mp_stat(|s| {
        if s.price_start > 0.0 { format!("${}", format_number_f64(s.price_start, 0)) }
        else { "\u{2014}".to_string() }
    });
    let price_end = mp_stat(|s| {
        if s.price_end > 0.0 { format!("${}", format_number_f64(s.price_end, 0)) }
        else { "\u{2014}".to_string() }
    });
    let price_change = mp_stat(|s| {
        if s.price_start > 0.0 {
            let sign = if s.price_change_pct >= 0.0 { "+" } else { "" };
            format!("{sign}{:.1}%", s.price_change_pct)
        } else { "\u{2014}".to_string() }
    });

    view! {
        <Title text="Bitcoin Stats Summary: At-a-Glance Network Counters | WE HODL BTC"/>
        <Meta name="description" content="Bitcoin network summary statistics for any time range. Total transactions, fees, inscriptions, Runes, SegWit adoption, Taproot usage, and embedded data counters."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/stats"/>

        // Page header with range selector
        <div class="relative rounded-2xl overflow-hidden mb-5">
            <img
                src="/img/observatory_hero.png"
                alt="Stats Summary"
                class="w-full h-[100px] sm:h-[120px] lg:h-[140px] object-cover object-center"
            />
            <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/60 to-[#123c64]/30"></div>
            <div class="absolute inset-0 flex flex-col items-center justify-end pb-3 sm:pb-4">
                <h1 class="text-lg sm:text-xl lg:text-2xl font-title text-white mb-0.5 drop-shadow-lg">"Stats Summary"</h1>
                <p class="text-[11px] sm:text-xs text-white/50 max-w-lg mx-auto px-4 text-center drop-shadow">"At-a-glance Bitcoin network counters for any time range"</p>
            </div>
        </div>
        <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 mb-6">
            <div class="flex items-center gap-1.5">
                <p class="text-xs sm:text-sm text-white/40 font-mono">
                    {move || {
                        data.get().map(|s| {
                            let fmt = |ts: u64| {
                                chrono::DateTime::from_timestamp(ts as i64, 0)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                                    .unwrap_or_default()
                            };
                            format!("{} \u{2192} {}", fmt(s.min_timestamp), fmt(s.max_timestamp))
                        }).unwrap_or_default()
                    }}
                </p>
                <span
                    class="text-white/30 hover:text-white/60 cursor-help transition-colors"
                    title="Timestamps reflect the actual first and last block mined in this range, not the query boundaries. Bitcoin blocks are mined at irregular intervals so times won\u{2019}t align exactly with midnight."
                >
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M11.25 11.25l.041-.02a.75.75 0 0 1 1.063.852l-.708 2.836a.75.75 0 0 0 1.063.853l.041-.021M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9-3.75h.008v.008H12V8.25Z"/>
                    </svg>
                </span>
            </div>
            <super::shared::RangeSelector/>
        </div>

        // Stats grid
        <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3 sm:gap-4">
            <SectionHeader title="Network"/>
            <StatCard label="Blocks" value=blocks
                tooltip="Total number of blocks mined in this range"/>
            <StatCard label="Transactions" value=txs sub=txs_sub
                tooltip="Total transactions including one coinbase per block"/>
            <StatCard label="Avg Block Size" value=avg_size
                tooltip="Average raw block size in megabytes (not weight-adjusted)"/>
            <StatCard label="Avg Block Time" value=avg_block_time
                tooltip="Average time between consecutive blocks (target: 10:00)"/>
            <StatCard label="Avg Txs/Block" value=avg_tx_per_block
                tooltip="Average number of transactions per block including coinbase"/>
            <StatCard label="Weight Utilization" value=weight_util
                tooltip="How full blocks are on average, as % of the 4 MWU consensus limit"/>
            <StatCard label="Chain Growth" value=chain_growth
                tooltip="Total raw block data added to the chain in this range"/>
            <StatCard label="BTC Transferred" value=total_btc_transferred
                tooltip="Sum of all non-coinbase output values. Requires backfill for historical data"/>

            <SectionHeader title="Fees"/>
            <StatCard label="Total Fees" value=total_fees_btc sub=total_fees_sub
                tooltip="Sum of all transaction fees paid to miners in this range"/>
            <StatCard label="Avg Fee Rate" value=avg_fee_rate
                tooltip="Average of per-block median fee rates (sat/vB). Reflects typical transaction cost"/>
            <StatCard label="Avg Fee/Tx" value=avg_fee_per_tx
                tooltip="Total fees divided by non-coinbase transaction count"/>
            <StatCard label="Avg Median Fee" value=avg_median_fee
                tooltip="Average of per-block median absolute fees in satoshis"/>

            <SectionHeader title="Adoption"/>
            <StatCard label="SegWit Transactions" value=segwit_pct
                tooltip="Percentage of non-coinbase transactions using SegWit witness data"/>
            <StatCard label="Taproot Outputs" value=taproot_outputs sub=taproot_sub
                tooltip="Total P2TR outputs created. Key-path is simple spends, script-path enables smart contracts"/>
            <StatCard label="Witness Data" value=witness_pct sub=Signal::derive(|| "of total block data".to_string())
                tooltip="Witness bytes as percentage of total block size. Higher = more SegWit usage"/>
            <StatCard label="Total Inputs" value=total_inputs
                tooltip="Total transaction inputs consumed (UTXOs spent)"/>
            <StatCard label="Total Outputs" value=total_outputs
                tooltip="Total transaction outputs created (new UTXOs)"/>
            <StatCard label="RBF Usage" value=rbf_pct
                tooltip="Percentage of non-coinbase transactions signaling Replace-By-Fee (nSequence < 0xFFFFFFFE)"/>

            <SectionHeader title="Embedded Data"/>
            <StatCard label="Inscriptions" value=inscriptions sub=inscriptions_sub
                tooltip="Ordinals inscriptions detected via witness envelope (OP_FALSE OP_IF OP_PUSH 'ord')"/>
            <StatCard label="BRC-20" value=brc20 sub=brc20_sub
                tooltip="BRC-20 token operations (a subset of inscriptions with JSON payload)"/>
            <StatCard label="Runes" value=runes sub=runes_sub
                tooltip="Runes protocol outputs (OP_RETURN with OP_13 prefix, active since block 840,000)"/>
            <StatCard label="OP_RETURN Outputs" value=op_return sub=op_return_sub
                tooltip="All OP_RETURN outputs across all protocols (Runes + Omni + Counterparty + other)"/>
            <StatCard label="Omni Layer" value=omni
                tooltip="Omni Layer protocol outputs (includes Tether USDT on Bitcoin)"/>
            <StatCard label="Counterparty" value=counterparty
                tooltip="Counterparty protocol outputs (XCP, identified by CNTRPRTY marker)"/>

            <SectionHeader title="Mining"/>
            <StatCard label="Top Pool" value=top_pool sub=top_pool_sub
                tooltip="Mining pool that found the most blocks in this range"/>
            <StatCard label="Unique Pools" value=pool_count
                tooltip="Number of distinct mining pools identified by coinbase signature"/>
            <StatCard label="Empty Blocks" value=empty_blocks sub=empty_sub
                tooltip="Blocks with only a coinbase transaction (no user transactions)"/>

            <SectionHeader title="Price"/>
            <StatCard label="Start Price" value=price_start
                tooltip="BTC/USD price at the beginning of the selected range (daily granularity from blockchain.info)"/>
            <StatCard label="End Price" value=price_end
                tooltip="BTC/USD price at the end of the selected range"/>
            <StatCard label="Change" value=price_change
                tooltip="Percentage price change from start to end of range. May show 0% on 1D range due to daily price granularity"/>

            <SectionHeader title="Extremes"/>
            <StatCard label="Largest Block" value=max_block_size
                tooltip="Largest single block by raw size (bytes) in this range"/>
            <StatCard label="Highest Fee Block" value=max_block_fees
                tooltip="Block with the highest total transaction fees in this range"/>
        </div>
    }
}
