//! Stats Summary Dashboard - at-a-glance counters for any time range.
//!
//! Displays aggregate statistics for the selected range, organized into sections:
//! Network (blocks, txs, avg size, block time, TPS, weight utilization, chain growth),
//! Fees (total fees, avg rate, avg per tx, median fee),
//! Adoption (SegWit %, Taproot outputs, witness data %, RBF usage),
//! Embedded Data (inscriptions, BRC-20, Runes, OP_RETURN, Omni, Counterparty),
//! Mining (top pool, unique pool count), and Price (start, end, change).
//!
//! An "Extremes" hero section at the top shows record-breaking blocks (largest
//! block, most transactions, highest fees, etc.) as clickable cards that open
//! the block detail modal.

use leptos::prelude::*;
use leptos_meta::*;

use super::components::{show_block_detail, DataLoadError};
use super::helpers::*;
use super::shared::ObservatoryState;
use crate::stats::server_fns::*;
use crate::stats::types::{ExtremesData, MiningPriceSummary, RangeSummary};

// ---------------------------------------------------------------------------
// Stat card component
// ---------------------------------------------------------------------------

/// Single stat card displaying a label, value, optional subtitle, and optional tooltip.
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
            data-tip=tooltip.unwrap_or("")
            tabindex=if tooltip.is_some() { "0" } else { "-1" }
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
// Extreme card — clickable, links to mempool.space block
// ---------------------------------------------------------------------------

/// Format bytes as human-readable size.
fn fmt_size(bytes: u64) -> String {
    if bytes >= 1_000_000 {
        format!("{:.2} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Format satoshis as BTC.
fn fmt_btc(sats: u64) -> String {
    let btc = sats as f64 / 100_000_000.0;
    if btc >= 1.0 {
        format!("{:.4} BTC", btc)
    } else {
        format!("{} sats", format_number(sats))
    }
}

/// Format a UNIX timestamp as a short date.
fn fmt_date(ts: u64) -> String {
    if ts == 0 {
        return String::new();
    }
    chrono::DateTime::from_timestamp(ts as i64, 0)
        .map(|dt| dt.format("%b %d, %Y").to_string())
        .unwrap_or_default()
}

/// Clickable extreme/record card that opens the block detail modal on click.
#[component]
fn ExtremeCard(
    #[prop(into)] label: &'static str,
    #[prop(into)] value: String,
    height: u64,
    #[prop(into)] miner: String,
    #[prop(into)] date: String,
    #[prop(optional, into)] tooltip: Option<&'static str>,
) -> impl IntoView {
    let height_str = format_number(height);
    view! {
        <div
            class="bg-[#112d4a] border border-[#f7931a]/15 rounded-xl p-3 sm:p-4 hover:border-[#f7931a]/50 hover:bg-[#153556] transition-all cursor-pointer"
            data-tip=tooltip.unwrap_or("")
            tabindex=if tooltip.is_some() { "0" } else { "-1" }
            on:click=move |_| show_block_detail(height)
        >
            <div class="flex items-center justify-between mb-1.5">
                <p class="text-[10px] sm:text-xs text-[#8899aa] uppercase tracking-wider font-medium">{label}</p>
            </div>
            <p class="text-lg sm:text-xl font-bold text-[#f7931a] font-mono truncate mb-2">{value}</p>
            <div class="flex items-center justify-between gap-2">
                <span class="text-[11px] text-white/60 font-mono truncate">"Block #"{height_str}</span>
                <span class="text-[11px] text-[#f7931a]/50 truncate text-right font-medium">{miner}</span>
            </div>
            <div class="text-[10px] text-white/40 mt-0.5">{date}</div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Extremes hero section
// ---------------------------------------------------------------------------

/// Hero section showing record-breaking blocks as a grid of clickable ExtremeCards.
/// Filters out zero-value entries and includes empty block count if nonzero.
#[component]
fn ExtremesHero(data: ExtremesData) -> impl IntoView {
    let d = data;

    // Build the card list, filtering out zero-value entries
    struct Card {
        label: &'static str,
        value: String,
        height: u64,
        miner: String,
        date: String,
        tooltip: &'static str,
    }

    let mut cards = vec![
        Card {
            label: "Largest Block",
            value: fmt_size(d.largest_block.value),
            height: d.largest_block.height,
            miner: d.largest_block.miner.clone(),
            date: fmt_date(d.largest_block.timestamp),
            tooltip: "The heaviest single block by raw byte size",
        },
        Card {
            label: "Most Transactions",
            value: format_number(d.most_txs.value),
            height: d.most_txs.height,
            miner: d.most_txs.miner.clone(),
            date: fmt_date(d.most_txs.timestamp),
            tooltip: "Block with the most transactions",
        },
        Card {
            label: "Largest Transaction",
            value: fmt_size(d.largest_tx.value),
            height: d.largest_tx.height,
            miner: d.largest_tx.miner.clone(),
            date: fmt_date(d.largest_tx.timestamp),
            tooltip: "Single largest transaction by raw byte size",
        },
        Card {
            label: "Most Inputs",
            value: format_number(d.most_inputs.value),
            height: d.most_inputs.height,
            miner: d.most_inputs.miner.clone(),
            date: fmt_date(d.most_inputs.timestamp),
            tooltip: "Block that consumed the most UTXOs (inputs)",
        },
        Card {
            label: "Most Outputs",
            value: format_number(d.most_outputs.value),
            height: d.most_outputs.height,
            miner: d.most_outputs.miner.clone(),
            date: fmt_date(d.most_outputs.timestamp),
            tooltip: "Block that created the most new UTXOs (outputs)",
        },
        Card {
            label: "Highest Fee Block",
            value: fmt_btc(d.highest_fee_block.value),
            height: d.highest_fee_block.height,
            miner: d.highest_fee_block.miner.clone(),
            date: fmt_date(d.highest_fee_block.timestamp),
            tooltip: "Block that collected the most total transaction fees",
        },
        Card {
            label: "Peak Median Fee Rate",
            value: format!("{:.1} sat/vB", d.peak_fee_rate.value),
            height: d.peak_fee_rate.height,
            miner: d.peak_fee_rate.miner.clone(),
            date: fmt_date(d.peak_fee_rate.timestamp),
            tooltip: "Highest median fee rate in any single block (half of txs paid more than this)",
        },
        Card {
            label: "Highest Fee Rate (P90)",
            value: format!("{} sat/vB", format_number(d.peak_p90_fee_rate.value.round() as u64)),
            height: d.peak_p90_fee_rate.height,
            miner: d.peak_p90_fee_rate.miner.clone(),
            date: fmt_date(d.peak_p90_fee_rate.timestamp),
            tooltip: "Highest 90th-percentile fee rate in any single block (90% of txs paid less than this)",
        },
        Card {
            label: "Most RBF",
            value: format_number(d.most_rbf.value),
            height: d.most_rbf.height,
            miner: d.most_rbf.miner.clone(),
            date: fmt_date(d.most_rbf.timestamp),
            tooltip: "Block with the most Replace-By-Fee signaling transactions",
        },
        Card {
            label: "Most Taproot Spends",
            value: format_number(d.most_taproot.value),
            height: d.most_taproot.height,
            miner: d.most_taproot.miner.clone(),
            date: fmt_date(d.most_taproot.timestamp),
            tooltip: "Block with the most P2TR (Taproot) spend inputs",
        },
        Card {
            label: "Most OP_RETURNs",
            value: format_number(d.most_op_returns.value),
            height: d.most_op_returns.height,
            miner: d.most_op_returns.miner.clone(),
            date: fmt_date(d.most_op_returns.timestamp),
            tooltip: "Block with the most OP_RETURN data outputs",
        },
        Card {
            label: "Most Inscriptions",
            value: format_number(d.most_inscriptions.value),
            height: d.most_inscriptions.height,
            miner: d.most_inscriptions.miner.clone(),
            date: fmt_date(d.most_inscriptions.timestamp),
            tooltip: "Block with the most Ordinals inscriptions",
        },
        Card {
            label: "Most Runes",
            value: format_number(d.most_runes.value),
            height: d.most_runes.height,
            miner: d.most_runes.miner.clone(),
            date: fmt_date(d.most_runes.timestamp),
            tooltip: "Block with the most Runes protocol outputs",
        },
        Card {
            label: "Highest Value Block",
            value: format!("{} BTC", format_number(d.highest_value.value / 100_000_000)),
            height: d.highest_value.height,
            miner: d.highest_value.miner.clone(),
            date: fmt_date(d.highest_value.timestamp),
            tooltip: "Block with the highest total output value (settlement volume)",
        },
    ];

    // Filter out zero-value cards
    cards.retain(|c| c.value != "0" && c.value != "0.0 sat/vB" && c.value != "0 B");

    view! {
        <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3 sm:gap-4 mb-2">
            {cards.into_iter().map(|c| {
                view! {
                    <ExtremeCard
                        label=c.label
                        value=c.value
                        height=c.height
                        miner=c.miner
                        date=c.date
                        tooltip=c.tooltip
                    />
                }
            }).collect::<Vec<_>>()}

            // Empty blocks — not a link, just a stat
            {if d.empty_block_count > 0 {
                let pct = format!(
                    "{:.2}% of {} blocks",
                    d.empty_block_count as f64 / d.block_count.max(1) as f64 * 100.0,
                    format_number(d.block_count),
                );
                Some(view! {
                    <div
                        class="bg-[#112d4a] border border-[#f7931a]/15 rounded-xl p-3 sm:p-4"
                        data-tip="Blocks with only a coinbase transaction (no user transactions)"
                        tabindex="0"
                    >
                        <p class="text-[10px] sm:text-xs text-[#8899aa] uppercase tracking-wider font-medium mb-1.5">"Empty Blocks"</p>
                        <p class="text-lg sm:text-xl font-bold text-[#f7931a] font-mono">{format_number(d.empty_block_count)}</p>
                        <p class="text-[11px] text-white/50 mt-1">{pct}</p>
                    </div>
                })
            } else {
                None
            }}
        </div>
    }
}

// ---------------------------------------------------------------------------
// Stats Summary page
// ---------------------------------------------------------------------------

/// Stats overview page showing aggregate counters and record-breaking blocks for
/// the currently selected time range. Fetches summary, extremes, and mining/price data.
#[component]
pub fn StatsSummaryPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;

    let custom_from = state.custom_from;
    let custom_to = state.custom_to;

    // Compute timestamp range from the selected preset or custom dates.
    // Round `now` to the nearest hour so cache keys stay stable across
    // range switches (otherwise every second produces a new key).
    let ts_range = Signal::derive(move || {
        let r = range.get();
        let now = chrono::Utc::now().timestamp() as u64;
        let now_rounded = now / 3600 * 3600; // snap to hour boundary
        if r == "custom" {
            let from = custom_from
                .get()
                .and_then(|s| super::shared::date_to_ts(&s))
                .unwrap_or(0);
            let to = custom_to
                .get()
                .and_then(|s| super::shared::date_to_ts(&s))
                .map(|t| t + 86_399)
                .unwrap_or(now_rounded);
            return (from, to);
        }
        let n = range_to_blocks(&r);
        let seconds = n * 600;
        let from = if n >= 999_999 {
            0
        } else {
            now_rounded.saturating_sub(seconds)
        };
        (from, now_rounded)
    });

    // Fetch summary data
    let summary = LocalResource::new(move || {
        let (from, to) = ts_range.get();
        async move {
            fetch_range_summary(from, to)
                .await
                .map_err(|e| e.to_string())
        }
    });

    let data = Signal::derive(move || summary.get().and_then(|r| r.ok()));

    // Fetch mining + price context
    let mining_price = LocalResource::new(move || {
        let (from, to) = ts_range.get();
        async move {
            fetch_mining_price_summary(from, to)
                .await
                .map_err(|e| e.to_string())
        }
    });
    let mp_data =
        Signal::derive(move || mining_price.get().and_then(|r| r.ok()));

    // Fetch extremes with block heights
    let extremes_res = LocalResource::new(move || {
        let (from, to) = ts_range.get();
        async move { fetch_extremes(from, to).await.ok() }
    });

    // Format helper — creates a Signal<String> from a RangeSummary field
    let stat = move |f: fn(&RangeSummary) -> String| -> Signal<String> {
        let d = data;
        Signal::derive(move || {
            d.get()
                .map(|s| f(&s))
                .unwrap_or_else(|| "\u{2014}".to_string())
        })
    };
    let mp_stat =
        move |f: fn(&MiningPriceSummary) -> String| -> Signal<String> {
            let d = mp_data;
            Signal::derive(move || {
                d.get()
                    .map(|s| f(&s))
                    .unwrap_or_else(|| "\u{2014}".to_string())
            })
        };

    // Non-coinbase tx count (for percentages that exclude coinbase)
    let user_tx = move |s: &RangeSummary| -> u64 {
        s.total_tx.saturating_sub(s.block_count)
    };

    // === Network ===
    let blocks = stat(|s| format_number(s.block_count));
    let txs = stat(|s| format_compact(s.total_tx));
    let txs_sub = stat(|s| {
        format!("{} total (incl. coinbase)", format_number(s.total_tx))
    });
    let avg_size = stat(|s| {
        if s.block_count > 0 {
            format!(
                "{:.2} MB",
                s.total_size as f64 / s.block_count as f64 / 1_000_000.0
            )
        } else {
            "\u{2014}".to_string()
        }
    });
    let avg_block_time = stat(|s| {
        let total_secs = (s.avg_block_time * 60.0).round() as u64;
        format!("{}:{:02}", total_secs / 60, total_secs % 60)
    });
    let avg_tx_per_block = stat(|s| {
        if s.block_count > 0 {
            format_number_f64(s.total_tx as f64 / s.block_count as f64, 1)
        } else {
            "\u{2014}".to_string()
        }
    });
    let weight_util = stat(|s| {
        if s.block_count > 0 {
            format!(
                "{:.1}%",
                s.total_weight as f64 / s.block_count as f64 / 4_000_000.0
                    * 100.0
            )
        } else {
            "\u{2014}".to_string()
        }
    });
    let chain_growth = stat(|s| format_data_size(s.total_size));
    let avg_tps = stat(|s| {
        if s.block_count > 1 && s.avg_block_time > 0.0 {
            let secs = s.avg_block_time * 60.0;
            let avg_tx_per_block = s.total_tx as f64 / s.block_count as f64;
            format!("{:.1} tx/s", avg_tx_per_block / secs)
        } else {
            "\u{2014}".to_string()
        }
    });

    // === Fees ===
    let total_fees_btc = stat(|s| {
        format!(
            "\u{20bf}{}",
            format_number_f64(s.total_fees as f64 / 100_000_000.0, 2)
        )
    });
    let total_fees_sub =
        stat(|s| format!("{} sats", format_number(s.total_fees)));
    let avg_fee_rate = stat(|s| format!("{:.1} sat/vB", s.avg_fee_rate));
    let avg_fee_per_tx = {
        let d = data;
        Signal::derive(move || {
            d.get()
                .map(|s| {
                    let utx = user_tx(&s);
                    if utx > 0 {
                        format!(
                            "{} sats",
                            format_number(
                                (s.total_fees as f64 / utx as f64).round()
                                    as u64
                            )
                        )
                    } else {
                        "\u{2014}".to_string()
                    }
                })
                .unwrap_or_else(|| "\u{2014}".to_string())
        })
    };
    let avg_median_fee = stat(|s| {
        format!("{} sats", format_number(s.avg_median_fee.round() as u64))
    });

    // === Adoption ===
    let segwit_pct = {
        let d = data;
        Signal::derive(move || {
            d.get()
                .map(|s| {
                    let utx = user_tx(&s);
                    if utx > 0 {
                        format!(
                            "{:.1}%",
                            s.total_segwit_txs as f64 / utx as f64 * 100.0
                        )
                    } else {
                        "\u{2014}".to_string()
                    }
                })
                .unwrap_or_else(|| "\u{2014}".to_string())
        })
    };
    let taproot_outputs = stat(|s| format_compact(s.total_taproot_outputs));
    let taproot_sub = stat(|s| {
        format!(
            "Key-path: {}  |  Script-path: {}",
            format_compact(s.total_taproot_keypath),
            format_compact(s.total_taproot_scriptpath)
        )
    });
    let witness_pct = stat(|s| format!("{:.1}%", s.witness_pct));
    let total_inputs = stat(|s| format_compact(s.total_inputs));
    let total_outputs = stat(|s| format_compact(s.total_outputs));
    let rbf_pct = {
        let d = data;
        Signal::derive(move || {
            d.get()
                .map(|s| {
                    let utx = user_tx(&s);
                    if utx > 0 {
                        format!(
                            "{:.1}%",
                            s.total_rbf as f64 / utx as f64 * 100.0
                        )
                    } else {
                        "\u{2014}".to_string()
                    }
                })
                .unwrap_or_else(|| "\u{2014}".to_string())
        })
    };

    // === Embedded Data ===
    let inscriptions = stat(|s| format_compact(s.total_inscriptions));
    let inscriptions_sub = stat(|s| {
        format!("{} data", format_data_size(s.total_inscription_bytes))
    });
    let brc20 = stat(|s| format_compact(s.total_brc20));
    let brc20_sub = stat(|s| {
        if s.total_inscriptions > 0 {
            format!(
                "{:.1}% of inscriptions",
                s.total_brc20 as f64 / s.total_inscriptions as f64 * 100.0
            )
        } else {
            String::new()
        }
    });
    let runes = stat(|s| format_compact(s.total_runes));
    let runes_sub =
        stat(|s| format!("{} data", format_data_size(s.total_runes_bytes)));
    let op_return = stat(|s| format_compact(s.total_op_return_count));
    let op_return_sub =
        stat(|s| format!("{} data", format_data_size(s.total_op_return_bytes)));
    let omni = stat(|s| format_compact(s.total_omni));
    let counterparty = stat(|s| format_compact(s.total_counterparty));

    // === Volume ===
    let total_btc_transferred = stat(|s| {
        if s.total_output_value > 0 {
            format!(
                "\u{20bf}{}",
                format_number_f64(
                    s.total_output_value as f64 / 100_000_000.0,
                    2
                )
            )
        } else {
            "\u{2014}".to_string()
        }
    });

    // === Mining ===
    let top_pool = mp_stat(|s| s.top_pool_name.clone());
    let top_pool_sub = mp_stat(|s| {
        format!(
            "{} blocks ({:.1}%)",
            format_number(s.top_pool_blocks),
            s.top_pool_pct
        )
    });
    let pool_count = mp_stat(|s| format_number(s.pool_count));

    // === Price ===
    let price_start = mp_stat(|s| {
        if s.price_start >= 1.0 {
            format!("${}", format_number_f64(s.price_start, 0))
        } else if s.price_start > 0.0 {
            format!("${:.4}", s.price_start)
        } else {
            "$0".to_string()
        }
    });
    let price_end = mp_stat(|s| {
        if s.price_end >= 1.0 {
            format!("${}", format_number_f64(s.price_end, 0))
        } else if s.price_end > 0.0 {
            format!("${:.4}", s.price_end)
        } else {
            "\u{2014}".to_string()
        }
    });
    let price_change = mp_stat(|s| {
        if s.price_start > 0.0 {
            let sign = if s.price_change_pct >= 0.0 { "+" } else { "" };
            format!("{sign}{:.1}%", s.price_change_pct)
        } else if s.price_end > 0.0 {
            // Start is $0 (pre-exchange), end has a price
            "\u{221e}".to_string() // infinity symbol
        } else {
            "\u{2014}".to_string()
        }
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
                <h1 class="text-lg sm:text-xl lg:text-2xl font-title text-white mb-0.5 drop-shadow-lg">"Stats Overview"</h1>
                <p class="text-[11px] sm:text-xs text-white/50 max-w-lg mx-auto px-4 text-center drop-shadow">"At-a-glance Bitcoin network counters for any time range"</p>
            </div>
        </div>
        <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2 mb-6">
            <div class="flex items-center gap-1.5">
                // Short dates on mobile, full timestamps on desktop
                <p class="text-xs text-white/40 font-mono sm:hidden">
                    {move || {
                        data.get().map(|s| {
                            let fmt = |ts: u64| {
                                chrono::DateTime::from_timestamp(ts as i64, 0)
                                    .map(|dt| dt.format("%b %d, %Y").to_string())
                                    .unwrap_or_default()
                            };
                            format!("{} \u{2192} {}", fmt(s.min_timestamp), fmt(s.max_timestamp))
                        }).unwrap_or_default()
                    }}
                </p>
                <p class="hidden sm:block text-sm text-white/40 font-mono">
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
                    class="hidden sm:inline-flex items-center justify-center w-4 h-4 rounded-full border border-white/20 text-[9px] font-bold text-white/30 hover:text-[#f7931a] hover:border-[#f7931a]/40 transition-colors"
                    data-tip="Timestamps reflect the actual first and last block mined in this range, not the query boundaries. Bitcoin blocks are mined at irregular intervals so times won\u{2019}t align exactly with midnight."
                    tabindex="0"
                >
                    "\u{20bf}"
                </span>
            </div>
            <super::shared::RangeSelector/>
        </div>

        // Error state
        {move || {
            let has_error = summary.get().map(|r| r.is_err()).unwrap_or(false);
            if has_error {
                view! {
                    <div class="mb-6">
                        <DataLoadError on_retry=Callback::new(move |_| summary.refetch())/>
                    </div>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }
        }}

        // ===================================================================
        // EXTREMES — Hero section at top, clickable cards with block links
        // ===================================================================
        <div class="bg-[#0d2137] border border-[#f7931a]/25 rounded-2xl p-4 sm:p-6 mb-6">
            <h2 class="text-sm font-bold text-[#f7931a] uppercase tracking-widest border-b border-[#f7931a]/25 pb-2 mb-4">"Records"</h2>

            <Suspense fallback=move || view! {
                <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3 sm:gap-4">
                    {(0..8).map(|_| view! {
                        <div class="bg-[#112d4a] border border-white/5 rounded-xl p-4 h-[88px] animate-pulse"></div>
                    }).collect::<Vec<_>>()}
                </div>
            }>
                {move || {
                    let ext = extremes_res.get().flatten();
                    ext.map(|d| view! { <ExtremesHero data=d/> })
                }}
            </Suspense>
        </div>

        // ===================================================================
        // REST OF THE STATS — Network, Fees, Adoption, Embedded, Mining, Price
        // ===================================================================
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
            <StatCard label="Avg TPS" value=avg_tps
                tooltip="Average transactions per second, derived from avg txs/block divided by avg block time"/>
            <StatCard label="Transaction Volume" value=total_btc_transferred
                tooltip="Sum of all non-coinbase output values. Includes change outputs, so the same BTC can be counted multiple times"/>

            <SectionHeader title="Fees"/>
            <StatCard label="Total Fees" value=total_fees_btc sub=total_fees_sub
                tooltip="Sum of all transaction fees paid to miners in this range"/>
            <StatCard label="Avg Fee Rate" value=avg_fee_rate
                tooltip="Average of per-block median fee rates in satoshis per virtual byte. Multiply by transaction vsize to estimate the actual fee paid"/>
            <StatCard label="Avg Fee/Tx" value=avg_fee_per_tx
                tooltip="Total fees divided by non-coinbase transaction count"/>
            <StatCard label="Avg Median Fee" value=avg_median_fee
                tooltip="Average of per-block median absolute fees in satoshis"/>

            <SectionHeader title="Adoption"/>
            <StatCard label="SegWit Transactions" value=segwit_pct
                tooltip="Percentage of transactions with at least one SegWit input. A transaction counts even if it also creates legacy outputs, so this is higher than SegWit's share of total outputs"/>
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

            <SectionHeader title="Price"/>
            <StatCard label="Start Price" value=price_start
                tooltip="BTC/USD price at the beginning of the selected range (daily granularity from blockchain.info)"/>
            <StatCard label="End Price" value=price_end
                tooltip="BTC/USD price at the end of the selected range"/>
            <StatCard label="Change" value=price_change
                tooltip="Percentage price change from start to end of range. May show 0% on 1D range due to daily price granularity"/>
        </div>
    }
}
