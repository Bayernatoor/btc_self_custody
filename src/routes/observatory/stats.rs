//! Stats Summary Dashboard — at-a-glance counters for any time range.

use leptos::prelude::*;
use leptos_meta::*;

use super::helpers::*;
use super::shared::ObservatoryState;
use crate::stats::server_fns::*;
use crate::stats::types::RangeSummary;

// ---------------------------------------------------------------------------
// Stat card component
// ---------------------------------------------------------------------------

#[component]
fn StatCard(
    #[prop(into)] label: &'static str,
    #[prop(into)] value: Signal<String>,
    #[prop(optional, into)] sub: Option<Signal<String>>,
) -> impl IntoView {
    view! {
        <div class="bg-[#0d2137] border border-white/10 rounded-xl p-3 sm:p-4">
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

    // Compute timestamp range from the selected preset
    let ts_range = Signal::derive(move || {
        let r = range.get();
        let now = chrono::Utc::now().timestamp() as u64;
        let n = range_to_blocks(&r);
        let seconds = n * 600; // approximate
        let from = if n >= 999_999 { 0 } else { now.saturating_sub(seconds) };
        (from, now)
    });

    // Fetch summary data
    let summary = LocalResource::new(move || {
        let (from, to) = ts_range.get();
        async move {
            fetch_range_summary(from, to).await.ok()
        }
    });

    let data = Signal::derive(move || summary.get().flatten());

    // Format helpers — each returns a Signal<String>
    let stat = move |f: fn(&RangeSummary) -> String| -> Signal<String> {
        let d = data;
        Signal::derive(move || d.get().map(|s| f(&s)).unwrap_or_else(|| "—".to_string()))
    };

    // Network stats
    let blocks = stat(|s| format_number(s.block_count));
    let txs = stat(|s| format_compact(s.total_tx));
    let txs_sub = stat(|s| format!("{} total", format_number(s.total_tx)));
    let avg_size = stat(|s| {
        if s.block_count > 0 {
            format!("{:.2} MB", s.total_size as f64 / s.block_count as f64 / 1_000_000.0)
        } else {
            "—".to_string()
        }
    });
    let avg_block_time = stat(|s| format!("{:.1} min", s.avg_block_time));
    let weight_util = stat(|s| {
        if s.block_count > 0 {
            format!("{:.1}%", s.total_weight as f64 / s.block_count as f64 / 4_000_000.0 * 100.0)
        } else {
            "—".to_string()
        }
    });
    let chain_growth = stat(|s| format!("{:.1} GB", s.total_size as f64 / 1_000_000_000.0));

    // Fee stats
    let total_fees_btc = stat(|s| format!("{:.2} BTC", s.total_fees as f64 / 100_000_000.0));
    let total_fees_sub = stat(|s| format!("{} sats", format_number(s.total_fees)));
    let avg_fee_rate = stat(|s| format!("{:.1} sat/vB", s.avg_fee_rate));

    // Adoption stats
    let segwit_pct = stat(|s| {
        if s.total_tx > 0 {
            format!("{:.1}%", s.total_segwit_txs as f64 / s.total_tx as f64 * 100.0)
        } else {
            "—".to_string()
        }
    });
    let taproot_outputs = stat(|s| format_compact(s.total_taproot_outputs));
    let taproot_sub = stat(|s| {
        format!("Key-path: {}  |  Script-path: {}",
            format_compact(s.total_taproot_keypath),
            format_compact(s.total_taproot_scriptpath))
    });
    let total_inputs = stat(|s| format_compact(s.total_inputs));
    let total_outputs = stat(|s| format_compact(s.total_outputs));
    let rbf_pct = stat(|s| {
        if s.total_tx > 0 {
            format!("{:.1}%", s.total_rbf as f64 / s.total_tx as f64 * 100.0)
        } else {
            "—".to_string()
        }
    });

    // Embedded data stats
    let inscriptions = stat(|s| format_compact(s.total_inscriptions));
    let inscriptions_sub = stat(|s| format!("{:.2} GB data", s.total_inscription_bytes as f64 / 1_000_000_000.0));
    let brc20 = stat(|s| format_compact(s.total_brc20));
    let runes = stat(|s| format_compact(s.total_runes));
    let runes_sub = stat(|s| format!("{:.2} GB data", s.total_runes_bytes as f64 / 1_000_000_000.0));
    let op_return = stat(|s| format_compact(s.total_op_return_count));
    let op_return_sub = stat(|s| format!("{:.2} GB data", s.total_op_return_bytes as f64 / 1_000_000_000.0));
    let omni = stat(|s| format_compact(s.total_omni));
    let counterparty = stat(|s| format_compact(s.total_counterparty));

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
        <div class="flex justify-end mb-6">
            <super::shared::RangeSelector/>
        </div>

        // Stats grid
        <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3 sm:gap-4">
            <SectionHeader title="Network"/>
            <StatCard label="Blocks" value=blocks/>
            <StatCard label="Transactions" value=txs sub=txs_sub/>
            <StatCard label="Avg Block Size" value=avg_size/>
            <StatCard label="Avg Block Time" value=avg_block_time/>
            <StatCard label="Weight Utilization" value=weight_util/>
            <StatCard label="Chain Growth" value=chain_growth/>

            <SectionHeader title="Fees"/>
            <StatCard label="Total Fees" value=total_fees_btc sub=total_fees_sub/>
            <StatCard label="Avg Fee Rate" value=avg_fee_rate/>

            <SectionHeader title="Adoption"/>
            <StatCard label="SegWit Transactions" value=segwit_pct/>
            <StatCard label="Taproot Outputs" value=taproot_outputs sub=taproot_sub/>
            <StatCard label="Total Inputs" value=total_inputs/>
            <StatCard label="Total Outputs" value=total_outputs/>
            <StatCard label="RBF Usage" value=rbf_pct/>

            <SectionHeader title="Embedded Data"/>
            <StatCard label="Inscriptions" value=inscriptions sub=inscriptions_sub/>
            <StatCard label="BRC-20" value=brc20/>
            <StatCard label="Runes" value=runes sub=runes_sub/>
            <StatCard label="OP_RETURN Outputs" value=op_return sub=op_return_sub/>
            <StatCard label="Omni Layer" value=omni/>
            <StatCard label="Counterparty" value=counterparty/>
        </div>
    }
}
