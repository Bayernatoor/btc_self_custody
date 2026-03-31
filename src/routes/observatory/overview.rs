//! Observatory Dashboard: live node stats, difficulty adjustment, halving countdown.

use leptos::prelude::*;
use leptos_meta::*;

use super::components::*;
use super::helpers::*;
use super::shared::*;
use crate::stats::server_fns::*;

#[component]
pub fn ObservatoryOverview() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let live_ctx = expect_context::<LiveContext>();
    let cached_live = state.cached_live;

    let live_field =
        move |f: fn(&crate::stats::types::LiveStats) -> String| -> String {
            cached_live
                .get()
                .map(|s| f(&s))
                .unwrap_or_else(|| "\u{2014}".to_string())
        };

    let block_height = Signal::derive(move || {
        live_field(|s| format_number(s.blockchain.blocks))
    });
    let chain_size = Signal::derive(move || {
        live_field(|s| format!("{:.1} GB", s.network.chain_size_gb))
    });
    let difficulty = Signal::derive(move || {
        live_field(|s| format!("{:.2}T", s.blockchain.difficulty / 1e12))
    });
    let hashrate = Signal::derive(move || {
        live_field(|s| {
            let h = s.network.hashrate;
            if h >= 1e18 {
                format!("{:.0} EH/s", h / 1e18)
            } else if h >= 1e15 {
                format!("{:.0} PH/s", h / 1e15)
            } else {
                format!("{:.0} TH/s", h / 1e12)
            }
        })
    });
    let mempool_size =
        Signal::derive(move || live_field(|s| format_number(s.mempool.size)));
    let mempool_bytes = Signal::derive(move || {
        live_field(|s| format!("{:.1} MB", s.mempool.bytes as f64 / 1e6))
    });
    let price_usd = Signal::derive(move || {
        live_field(|s| {
            format!("${}", format_number_f64(s.network.price_usd, 0))
        })
    });
    let sats_per_dollar = Signal::derive(move || {
        live_field(|s| format_number(s.network.sats_per_dollar))
    });
    let market_cap = Signal::derive(move || {
        live_field(|s| {
            if s.network.market_cap_usd >= 1e12 {
                format!("${:.2}T", s.network.market_cap_usd / 1e12)
            } else {
                format!("${:.1}B", s.network.market_cap_usd / 1e9)
            }
        })
    });
    let utxo_count = Signal::derive(move || {
        live_field(|s| {
            if s.network.utxo_count > 0 {
                format_number(s.network.utxo_count)
            } else {
                "loading...".to_string()
            }
        })
    });
    let supply_pct = Signal::derive(move || {
        live_field(|s| format!("{:.2}%", s.network.percent_issued))
    });
    let total_supply = Signal::derive(move || {
        live_field(|s| {
            format!("{} BTC", format_number_f64(s.network.total_supply, 0))
        })
    });
    let next_fee = Signal::derive(move || {
        live_field(|s| format!("{:.1} sat/vB", s.next_block_fee))
    });

    let gauge_option = Signal::derive(move || {
        cached_live
            .get()
            .map(|s| {
                crate::stats::charts::mempool_gauge(
                    s.mempool.usage,
                    s.mempool.maxmempool,
                )
            })
            .unwrap_or_default()
    });

    // Halving countdown
    let raw_block_height = Signal::derive(move || {
        cached_live.get().map(|s| s.blockchain.blocks).unwrap_or(0)
    });

    let next_halving_height = Signal::derive(move || {
        let h = raw_block_height.get();
        if h == 0 {
            return 0u64;
        }
        ((h / 210_000) + 1) * 210_000
    });

    let halving_blocks_remaining = Signal::derive(move || {
        let nh = next_halving_height.get();
        let h = raw_block_height.get();
        if nh == 0 {
            return 0u64;
        }
        nh.saturating_sub(h)
    });

    let halving_progress_pct = Signal::derive(move || {
        let remaining = halving_blocks_remaining.get();
        let elapsed = 210_000u64.saturating_sub(remaining);
        (elapsed as f64 / 210_000.0 * 100.0 * 10.0).round() / 10.0
    });

    let current_subsidy_btc = Signal::derive(move || {
        let h = raw_block_height.get();
        if h == 0 {
            return "---".to_string();
        }
        let halvings = h / 210_000;
        if halvings >= 64 {
            return "0".to_string();
        }
        let sats = 5_000_000_000u64 >> halvings;
        format!("{:.4} BTC", sats as f64 / 100_000_000.0)
    });

    let next_subsidy_btc = Signal::derive(move || {
        let h = raw_block_height.get();
        if h == 0 {
            return "---".to_string();
        }
        let next_halvings = (h / 210_000) + 1;
        if next_halvings >= 64 {
            return "0 BTC".to_string();
        }
        let sats = 5_000_000_000u64 >> next_halvings;
        format!("{:.4} BTC", sats as f64 / 100_000_000.0)
    });

    // Difficulty adjustment predictor
    let diff_period_start = Signal::derive(move || {
        let h = raw_block_height.get();
        if h == 0 {
            return 0u64;
        }
        (h / 2016) * 2016
    });

    let diff_blocks_into_period = Signal::derive(move || {
        raw_block_height
            .get()
            .saturating_sub(diff_period_start.get())
    });

    let diff_blocks_remaining = Signal::derive(move || {
        2016u64.saturating_sub(diff_blocks_into_period.get())
    });

    let diff_progress_pct = Signal::derive(move || {
        let into = diff_blocks_into_period.get();
        (into as f64 / 2016.0 * 100.0 * 10.0).round() / 10.0
    });

    let diff_est_remaining_days = Signal::derive(move || {
        let remaining = diff_blocks_remaining.get();
        format!("{:.1}", remaining as f64 * 10.0 / 1440.0)
    });

    let period_start_ts = LocalResource::new(move || {
        let ps = diff_period_start.get();
        async move {
            if ps == 0 {
                return 0u64;
            }
            fetch_block_timestamp(ps).await.ok().flatten().unwrap_or(0)
        }
    });

    let prev_period_start_ts = LocalResource::new(move || {
        let ps = diff_period_start.get();
        async move {
            if ps < 2016 {
                return 0u64;
            }
            fetch_block_timestamp(ps - 2016)
                .await
                .ok()
                .flatten()
                .unwrap_or(0)
        }
    });

    let prev_diff_change = Signal::derive(move || {
        let ps = diff_period_start.get();
        if ps < 2016 {
            return "\u{2014}".to_string();
        }
        let prev_ts = prev_period_start_ts.get().unwrap_or(0);
        let curr_ts = period_start_ts.get().unwrap_or(0);
        if prev_ts == 0 || curr_ts == 0 || curr_ts <= prev_ts {
            return "\u{2014}".to_string();
        }
        let actual_time = (curr_ts - prev_ts) as f64;
        let target_time = 2016.0 * 600.0;
        let change = (target_time / actual_time - 1.0) * 100.0;
        let rounded = (change * 100.0).round() / 100.0;
        if rounded >= 0.0 {
            format!("+{:.2}%", rounded)
        } else {
            format!("{:.2}%", rounded)
        }
    });

    let avg_block_time = Signal::derive(move || {
        let blocks_in = diff_blocks_into_period.get();
        if blocks_in < 2 {
            return "\u{2014}".to_string();
        }
        let start_ts = period_start_ts.get().unwrap_or(0);
        if start_ts == 0 {
            return "\u{2014}".to_string();
        }
        let current_ts =
            cached_live.get().map(|s| s.blockchain.time).unwrap_or(0);
        if current_ts <= start_ts {
            return "\u{2014}".to_string();
        }
        let elapsed_secs = (current_ts - start_ts) as f64;
        let avg_secs = (elapsed_secs / blocks_in as f64).round() as u64;
        format!("{}:{:02}", avg_secs / 60, avg_secs % 60)
    });

    // Measured avg block time in minutes (from current difficulty period),
    // falls back to 10.0 if not enough data yet. Used for halving estimation.
    let measured_block_min = Signal::derive(move || {
        let blocks_in = diff_blocks_into_period.get();
        if blocks_in < 10 {
            return 10.0f64;
        }
        let start_ts = period_start_ts.get().unwrap_or(0);
        if start_ts == 0 {
            return 10.0;
        }
        let current_ts =
            cached_live.get().map(|s| s.blockchain.time).unwrap_or(0);
        if current_ts <= start_ts {
            return 10.0;
        }
        (current_ts - start_ts) as f64 / 60.0 / blocks_in as f64
    });

    let halving_est_date = Signal::derive(move || {
        let remaining = halving_blocks_remaining.get();
        if remaining == 0 {
            return "\u{2014}".to_string();
        }
        let avg_min = measured_block_min.get();
        let days = remaining as f64 * avg_min / 1440.0;
        let est = chrono::Utc::now()
            + chrono::Duration::seconds((days * 86400.0) as i64);
        est.format("%b %d, %Y").to_string()
    });

    let halving_est_days = Signal::derive(move || {
        let remaining = halving_blocks_remaining.get();
        let avg_min = measured_block_min.get();
        (remaining as f64 * avg_min / 1440.0 * 10.0).round() / 10.0
    });

    let diff_expected_change = Signal::derive(move || {
        let blocks_in = diff_blocks_into_period.get();
        if blocks_in < 10 {
            return "\u{2014}".to_string();
        }
        let start_ts = period_start_ts.get().unwrap_or(0);
        if start_ts == 0 {
            return "\u{2014}".to_string();
        }
        let current_ts =
            cached_live.get().map(|s| s.blockchain.time).unwrap_or(0);
        if current_ts <= start_ts {
            return "\u{2014}".to_string();
        }
        let elapsed = (current_ts - start_ts) as f64;
        let projected = elapsed * 2016.0 / blocks_in as f64;
        let target = 2016.0 * 600.0;
        let change = (target / projected - 1.0) * 100.0;
        let rounded = (change * 10.0).round() / 10.0;
        if rounded >= 0.0 {
            format!("+{:.1}%", rounded)
        } else {
            format!("{:.1}%", rounded)
        }
    });

    let diff_est_date = Signal::derive(move || {
        let remaining = diff_blocks_remaining.get();
        if remaining == 0 {
            return "\u{2014}".to_string();
        }
        let days = remaining as f64 * 10.0 / 1440.0;
        let est = chrono::Utc::now()
            + chrono::Duration::seconds((days * 86400.0) as i64);
        est.format("%b %d, %Y").to_string()
    });

    view! {
        <Title text="Bitcoin Observatory: Live Blockchain Dashboard | WE HODL BTC"/>
        <Meta name="description" content="Live Bitcoin network dashboard with real-time block height, difficulty, hashrate, mempool stats, price, supply, UTXO count, halving countdown, and difficulty adjustment predictions."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory"/>

        // Live stats panel
        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-6 lg:p-8 mb-8">
            <div class="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-2 mb-3">
                <div class="flex items-center gap-2">
                    <div
                        class=move || if live_ctx.connected.get() {
                            "w-2.5 h-2.5 rounded-full bg-green-500 animate-pulse"
                        } else if cached_live.get().is_some() {
                            "w-2.5 h-2.5 rounded-full bg-yellow-500 animate-pulse"
                        } else {
                            "w-2.5 h-2.5 rounded-full bg-red-500/60"
                        }
                    ></div>
                    <span class="text-lg text-white font-bold">"Live Node Stats"</span>
                    {move || if !live_ctx.connected.get() && cached_live.get().is_some() {
                        view! { <span class="text-xs text-yellow-500/80">"(reconnecting...)"</span> }.into_any()
                    } else if !live_ctx.connected.get() && cached_live.get().is_none() {
                        view! { <span class="text-xs text-red-400/80">"(disconnected)"</span> }.into_any()
                    } else {
                        view! { <span></span> }.into_any()
                    }}
                </div>
                <div class="flex items-center gap-2 sm:ml-auto">
                    <span class="text-xs text-white/30">{move || live_ctx.last_updated.get()}</span>
                    <span class="text-xs text-white/20">
                        {move || format!("{}s", live_ctx.countdown.get())}
                    </span>
                    {
                        let (cooldown, set_cooldown) = signal(0u32);
                        // Tick down the cooldown every second
                        leptos_use::use_interval_fn(
                            move || {
                                set_cooldown.update(|c| if *c > 0 { *c -= 1; });
                            },
                            1_000,
                        );
                        view! {
                            <button
                                class=move || {
                                    if cooldown.get() > 0 {
                                        "text-xs text-white/20 px-2 py-0.5 rounded border border-white/5 cursor-not-allowed transition-all inline-flex items-center gap-1.5"
                                    } else {
                                        "text-xs text-white/40 hover:text-white/70 px-2 py-0.5 rounded border border-white/10 hover:border-white/20 cursor-pointer transition-all active:scale-95 inline-flex items-center gap-1.5 group"
                                    }
                                }
                                on:click=move |_| {
                                    if cooldown.get_untracked() > 0 { return; }
                                    set_cooldown.set(10);
                                    live_ctx.set_countdown.set(30);
                                    live_ctx.live.refetch();
                                    live_ctx.set_last_updated.set(format!("updated {}", chrono::Local::now().format("%H:%M:%S")));
                                }
                            >
                                <svg
                                    class=move || {
                                        if cooldown.get() > 0 {
                                            "w-3 h-3 animate-spin text-[#f7931a]/40"
                                        } else {
                                            "w-3 h-3 group-hover:rotate-180 transition-transform duration-300"
                                        }
                                    }
                                    fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2"
                                >
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/>
                                </svg>
                                {move || {
                                    let c = cooldown.get();
                                    if c > 0 { format!("{}s", c) } else { "Refresh".to_string() }
                                }}
                            </button>
                        }
                    }
                </div>
            </div>

            <div class="grid grid-cols-1 lg:grid-cols-3 gap-4">
                // Mempool section
                <div class="bg-[#0a1a2e] border border-white/10 rounded-xl p-5 overflow-hidden">
                    <h3 class="text-sm font-bold text-[#f7931a] uppercase tracking-widest mb-4">"Mempool"</h3>
                    <div class="grid grid-cols-2 gap-3 mb-3">
                        <LiveCard label="Transactions" value=mempool_size tooltip="Unconfirmed transactions waiting in the mempool"/>
                        <LiveCard label="Size" value=mempool_bytes tooltip="Total size of all unconfirmed transactions in megabytes"/>
                        <LiveCard label="Next Block Fee" value=next_fee tooltip="Estimated minimum fee rate to be included in the next block"/>
                    </div>
                    <div class="flex justify-center">
                        <Show when=move || cached_live.get().is_some()>
                            <Chart id="mempool-gauge".to_string() option=gauge_option class="w-[220px] h-[200px]".to_string()/>
                        </Show>
                    </div>
                </div>

                // Mining section
                <div class="bg-[#0a1a2e] border border-white/10 rounded-xl p-5">
                    <h3 class="text-sm font-bold text-[#f7931a] uppercase tracking-widest mb-4">"Mining"</h3>
                    <div class="grid grid-cols-2 gap-3 mb-2">
                        <LiveCard label="Block Height" value=block_height tooltip="The latest confirmed block number in the blockchain"/>
                        <LiveCard label="Difficulty" value=difficulty tooltip="Current mining difficulty in trillions. Adjusts every 2,016 blocks (~2 weeks) to target 10-minute block times"/>
                        <LiveCard label="Hashrate" value=hashrate tooltip="Estimated total network hash power based on current difficulty and block times"/>
                        <LiveCard label="Chain Size" value=chain_size tooltip="Total size of all block data plus undo files and indexes on disk"/>
                        <LiveCard label="Avg Block Time" value=avg_block_time tooltip="Average time between blocks over the current difficulty period (target: 10 minutes)"/>
                        <LiveCard label="Last Retarget" value=prev_diff_change tooltip="Percentage difficulty change at the most recent retarget (every 2,016 blocks)"/>
                    </div>
                </div>

                // Economic section
                <div class="bg-[#0a1a2e] border border-white/10 rounded-xl p-5">
                    <h3 class="text-sm font-bold text-[#f7931a] uppercase tracking-widest mb-4">"Economic"</h3>
                    <div class="grid grid-cols-2 gap-3">
                        <LiveCard label="Price (USD)" value=price_usd tooltip="Current BTC/USD spot price"/>
                        <LiveCard label="Sats/Dollar" value=sats_per_dollar tooltip="How many satoshis (0.00000001 BTC) one US dollar buys"/>
                        <LiveCard label="Market Cap" value=market_cap tooltip="Total supply multiplied by current price. Circulating market capitalization"/>
                        <LiveCard label="Total Supply" value=total_supply tooltip="Total BTC mined so far. The theoretical max is 20,999,999.9769 BTC due to rounding of halving rewards. Additionally ~100+ BTC are permanently lost from unclaimed rewards and the unspendable genesis coinbase"/>
                        <LiveCard label="% Issued" value=supply_pct tooltip="Percentage of the 21M hard cap that has been mined"/>
                        <LiveCard label="UTXO Count" value=utxo_count tooltip="Total unspent transaction outputs in the UTXO set. Each represents a spendable coin"/>
                    </div>
                </div>
            </div>
        </div>

        // Difficulty adjustment predictor
        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 mt-8">
            <div class="flex items-baseline justify-between mb-3">
                <h3 class="text-base sm:text-lg text-white font-semibold">"Next Difficulty Adjustment"</h3>
                <span class="text-xs text-white/40 font-mono">{move || diff_est_date.get()}</span>
            </div>
            <div class="grid grid-cols-2 lg:grid-cols-4 gap-4">
                <div class="text-center">
                    <div class="text-[0.6rem] sm:text-[0.7rem] text-[#8899aa] uppercase tracking-widest mb-1">"Period Start"</div>
                    <div class="text-sm sm:text-lg text-[#f7931a] font-bold font-mono">{move || format_number(diff_period_start.get())}</div>
                </div>
                <div class="text-center">
                    <div class="text-[0.6rem] sm:text-[0.7rem] text-[#8899aa] uppercase tracking-widest mb-1">"Blocks Into Period"</div>
                    <div class="text-sm sm:text-lg text-[#f7931a] font-bold font-mono">{move || format_number(diff_blocks_into_period.get())}</div>
                </div>
                <div class="text-center">
                    <div class="text-[0.6rem] sm:text-[0.7rem] text-[#8899aa] uppercase tracking-widest mb-1">"Blocks Remaining"</div>
                    <div class="text-sm sm:text-lg text-[#f7931a] font-bold font-mono">{move || format_number(diff_blocks_remaining.get())}</div>
                </div>
                <div class="text-center">
                    <div class="text-[0.6rem] sm:text-[0.7rem] text-[#8899aa] uppercase tracking-widest mb-1">"Est. Days Left"</div>
                    <div class="text-sm sm:text-lg text-[#f7931a] font-bold font-mono">{move || diff_est_remaining_days.get()}</div>
                </div>
            </div>
            <div class="mt-4 px-1">
                <div class="flex items-center justify-between mb-1.5">
                    <span class="text-xs text-white/40">"Progress"</span>
                    <span class="text-xs text-[#f7931a] font-mono font-semibold">{move || format!("{:.1}%", diff_progress_pct.get())}</span>
                </div>
                <div class="w-full h-2.5 bg-white/5 rounded-full overflow-hidden border border-white/10">
                    <div
                        class="h-full bg-gradient-to-r from-[#f7931a] to-[#fbbf24] rounded-full transition-all duration-500"
                        style=move || format!("width: {}%", diff_progress_pct.get())
                    ></div>
                </div>
            </div>
            <div class="mt-3 text-center">
                <span class="text-xs text-white/40">"Expected change: "</span>
                <span class="text-xs text-white/70 font-mono font-semibold">{move || diff_expected_change.get()}</span>
            </div>
        </div>

        // Halving countdown
        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 mt-8">
            <div class="flex items-baseline justify-between mb-3">
                <h3 class="text-base sm:text-lg text-white font-semibold">"Next Halving"</h3>
                <span class="text-xs text-white/40 font-mono">{move || halving_est_date.get()}</span>
            </div>
            <div class="grid grid-cols-2 lg:grid-cols-4 gap-4">
                <div class="text-center">
                    <div class="text-[0.6rem] sm:text-[0.7rem] text-[#8899aa] uppercase tracking-widest mb-1">"Target Height"</div>
                    <div class="text-sm sm:text-lg text-[#f7931a] font-bold font-mono">{move || format_number(next_halving_height.get())}</div>
                </div>
                <div class="text-center">
                    <div class="text-[0.6rem] sm:text-[0.7rem] text-[#8899aa] uppercase tracking-widest mb-1">"Blocks Remaining"</div>
                    <div class="text-sm sm:text-lg text-[#f7931a] font-bold font-mono">{move || format_number(halving_blocks_remaining.get())}</div>
                </div>
                <div class="text-center">
                    <div class="text-[0.6rem] sm:text-[0.7rem] text-[#8899aa] uppercase tracking-widest mb-1">"Est. Days"</div>
                    <div class="text-sm sm:text-lg text-[#f7931a] font-bold font-mono">{move || format!("{:.1}", halving_est_days.get())}</div>
                </div>
                <div class="text-center">
                    <div class="text-[0.6rem] sm:text-[0.7rem] text-[#8899aa] uppercase tracking-widest mb-1">"Current Subsidy"</div>
                    <div class="text-sm sm:text-lg text-[#f7931a] font-bold font-mono">{move || current_subsidy_btc.get()}</div>
                </div>
            </div>
            <div class="mt-4 px-1">
                <div class="flex items-center justify-between mb-1.5">
                    <span class="text-xs text-white/40">"Progress through epoch"</span>
                    <span class="text-xs text-[#f7931a] font-mono font-semibold">{move || format!("{:.1}%", halving_progress_pct.get())}</span>
                </div>
                <div class="w-full h-2.5 bg-white/5 rounded-full overflow-hidden border border-white/10">
                    <div
                        class="h-full bg-gradient-to-r from-[#f7931a] to-[#fbbf24] rounded-full transition-all duration-500"
                        style=move || format!("width: {}%", halving_progress_pct.get())
                    ></div>
                </div>
            </div>
            <div class="mt-3 text-center">
                <span class="text-xs text-white/40">"Next subsidy: "</span>
                <span class="text-xs text-white/70 font-mono font-semibold">{move || next_subsidy_btc.get()}</span>
            </div>
        </div>
    }
}
