//! The Bitcoin Observatory dashboard page.
//!
//! Tabs: Overview | Network | Fees | Mining | Embedded Data | Signaling
//! Data fetched via server functions, charts rendered with ECharts via wasm_bindgen.

mod components;
mod helpers;

use components::*;
use helpers::*;

use leptos::prelude::*;
use leptos_meta::*;

use crate::stats::server_fns::*;
use crate::stats::types::*;

// ---------------------------------------------------------------------------
// Main page
// ---------------------------------------------------------------------------

#[component]
pub fn StatsPage() -> impl IntoView {
    // Check if stats backend is available
    #[allow(clippy::redundant_closure)]
    let availability = LocalResource::new(move || fetch_stats_summary());

    let is_available = Signal::derive(move || {
        availability.get().map(|r| r.is_ok()).unwrap_or(false)
    });

    view! {
        <Title text="The Bitcoin Observatory - WE HODL BTC"/>
        <Show
            when=move || is_available.get()
            fallback=move || view! { <StatsComingSoon/> }
        >
            <StatsContent/>
        </Show>
    }
}

#[component]
fn StatsComingSoon() -> impl IntoView {
    view! {
        <section class="max-w-3xl mx-auto px-6 pt-20 pb-32 opacity-0 animate-fadeinone">
            <div class="flex flex-col items-center justify-center min-h-[60vh] text-center">
                // Bitcoin logo SVG
                <div class="mb-8 opacity-10">
                    <svg class="w-24 h-24 lg:w-32 lg:h-32 text-[#f7931a]" viewBox="0 0 64 64" fill="currentColor">
                        <path d="M63.04 39.741c-4.274 17.143-21.638 27.575-38.783 23.301C7.12 58.768-3.313 41.404.962 24.262 5.234 7.117 22.597-3.317 39.737.957c17.144 4.274 27.576 21.64 23.302 38.784z"/>
                        <path d="M46.11 27.441c.636-4.258-2.606-6.547-7.039-8.074l1.438-5.768-3.512-.875-1.4 5.616c-.923-.23-1.871-.447-2.813-.662l1.41-5.653-3.509-.875-1.439 5.766c-.764-.174-1.514-.346-2.242-.527l.004-.018-4.842-1.209-.934 3.75s2.605.597 2.55.634c1.422.355 1.68 1.296 1.636 2.042l-1.638 6.571c.098.025.225.061.365.117l-.37-.092-2.297 9.205c-.174.432-.615 1.08-1.609.834.035.051-2.552-.637-2.552-.637l-1.743 4.02 4.57 1.139c.85.213 1.683.436 2.502.646l-1.453 5.835 3.507.875 1.44-5.772c.957.26 1.887.5 2.797.726L27.504 50.8l3.511.875 1.453-5.823c5.987 1.133 10.49.676 12.383-4.738 1.527-4.36-.075-6.875-3.225-8.516 2.294-.529 4.022-2.038 4.483-5.157zM38.087 38.69c-1.086 4.36-8.426 2.004-10.807 1.412l1.928-7.729c2.38.594 10.011 1.77 8.88 6.317zm1.085-11.312c-.99 3.966-7.1 1.951-9.083 1.457l1.748-7.01c1.983.494 8.367 1.416 7.335 5.553z" fill="#fff"/>
                    </svg>
                </div>

                <h1 class="text-3xl lg:text-5xl font-title text-white mb-4">"The Bitcoin Observatory"</h1>
                <div class="w-16 h-0.5 bg-[#f7931a] mx-auto mb-6"></div>

                <p class="text-lg text-white/60 mb-3">"Coming Soon"</p>
                <p class="text-sm text-white/40 max-w-md leading-relaxed mb-10">
                    "Live blockchain metrics, block data analysis, embedded data tracking, and BIP signaling \u{2014} powered by our own Bitcoin full node."
                </p>

                // Feature preview cards
                <div class="grid grid-cols-1 sm:grid-cols-3 gap-4 w-full max-w-2xl">
                    <div class="bg-white/5 border border-white/10 rounded-xl p-5 text-center">
                        <div class="text-2xl mb-2">"#"</div>
                        <div class="text-sm text-white/70 font-medium mb-1">"Live Dashboard"</div>
                        <div class="text-xs text-white/40">"Real-time block, mempool, and network stats"</div>
                    </div>
                    <div class="bg-white/5 border border-white/10 rounded-xl p-5 text-center">
                        <div class="text-2xl mb-2">"\u{21a9}"</div>
                        <div class="text-sm text-white/70 font-medium mb-1">"Embedded Data"</div>
                        <div class="text-xs text-white/40">"Track Runes, Omni, Counterparty, and data embedding protocols"</div>
                    </div>
                    <div class="bg-white/5 border border-white/10 rounded-xl p-5 text-center">
                        <div class="text-2xl mb-2">"\u{2691}"</div>
                        <div class="text-sm text-white/70 font-medium mb-1">"BIP Signaling"</div>
                        <div class="text-xs text-white/40">"Monitor softfork activation progress"</div>
                    </div>
                </div>
            </div>
        </section>
    }
}

#[component]
fn StatsContent() -> impl IntoView {
    let (tab, set_tab) = signal("overview".to_string());
    let (range, set_range) = signal("all".to_string());
    let (fee_unit, set_fee_unit) = signal("sats".to_string());

    // Sub-section navigation within tabs
    let (network_section, set_network_section) = signal("blocks".to_string());
    let (embedded_section, set_embedded_section) = signal("overview".to_string());
    let (mining_section, set_mining_section) = signal("difficulty".to_string());

    // ---- Live stats (auto-refresh 30s) ----
    #[allow(clippy::redundant_closure)]
    let live = LocalResource::new(move || fetch_live_stats());

    // Overlay toggles
    let (overlay_halvings, set_overlay_halvings) = signal(false);
    let (overlay_bips, set_overlay_bips) = signal(false);
    let (overlay_core, set_overlay_core) = signal(false);
    let (overlay_price, set_overlay_price) = signal(false);
    let (overlay_chain_size, set_overlay_chain_size) = signal(false);
    let (overlay_events, set_overlay_events) = signal(false);
    let (overlay_panel_open, set_overlay_panel_open) = signal(false);

    // Price history resource — fetches full history + live price once when enabled
    let price_history = LocalResource::new(move || {
        let enabled = overlay_price.get();
        async move {
            if !enabled {
                return Vec::new();
            }
            let mut data: Vec<(u64, f64)> = match fetch_price_history(0, 4_000_000_000).await {
                Ok(pts) => pts.into_iter().map(|p| (p.timestamp_ms, p.price_usd)).collect(),
                Err(e) => {
                    leptos::logging::warn!("Price history fetch failed: {e}");
                    Vec::new()
                }
            };
            // Append live price to fill blockchain.info's ~1 week data gap
            if let Ok(live_stats) = fetch_live_stats().await {
                let now_ms = chrono::Utc::now().timestamp() as u64 * 1000;
                if live_stats.network.price_usd > 0.0 {
                    let last_ts = data.last().map(|&(ts, _)| ts).unwrap_or(0);
                    if now_ms > last_ts {
                        data.push((now_ms, live_stats.network.price_usd));
                    }
                }
            }
            data
        }
    });

    // overlay_flags is defined after dashboard_data (needs chain size data from it)
    let (countdown, set_countdown) = signal(30u32);
    let (last_updated, set_last_updated) = signal("connecting...".to_string());

    // 1-second countdown tick
    leptos_use::use_interval_fn(
        move || {
            set_countdown.update(|c| {
                if *c == 0 {
                    *c = 30;
                    live.refetch();
                    set_last_updated.set(format!(
                        "updated {}",
                        chrono::Local::now().format("%H:%M:%S")
                    ));
                } else {
                    *c -= 1;
                }
            });
        },
        1_000,
    );

    // Mark as updated when first load completes
    Effect::new(move |_| {
        if live.get().is_some() {
            set_last_updated.set(format!(
                "updated {}",
                chrono::Local::now().format("%H:%M:%S")
            ));
        }
    });

    // Helper to extract a field from live stats
    let live_field = move |f: fn(&LiveStats) -> String| -> String {
        live.get()
            .and_then(|r| r.ok())
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

    // Mempool gauge option
    let gauge_option = Signal::derive(move || {
        live.get()
            .and_then(|r| r.ok())
            .map(|s| {
                crate::stats::charts::mempool_gauge(
                    s.mempool.usage,
                    s.mempool.maxmempool,
                )
            })
            .unwrap_or_default()
    });

    // ---- Dashboard data ----
    let dashboard_data = LocalResource::new(move || {
        let r = range.get();
        async move {
            let stats =
                fetch_stats_summary().await.map_err(|e| e.to_string())?;

            let n = range_to_blocks(&r);
            let is_daily = n > 5_000;

            if is_daily {
                let from_ts = stats.latest_timestamp.saturating_sub(n * 600);
                let days =
                    fetch_daily_aggregates(from_ts, stats.latest_timestamp)
                        .await
                        .map_err(|e| e.to_string())?;
                Ok::<_, String>(DashboardData::Daily(days))
            } else {
                let from =
                    stats.min_height.max(stats.max_height.saturating_sub(n));
                let blocks = fetch_blocks(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(DashboardData::PerBlock(blocks))
            }
        }
    });

    let overlay_flags = Signal::derive(move || {
        let price_data = if overlay_price.get() {
            price_history.get().unwrap_or_default()
        } else {
            Vec::new()
        };

        let chain_size_data = if overlay_chain_size.get() {
            dashboard_data
                .get()
                .and_then(|r| r.ok())
                .map(|data| {
                    let mut cumulative: f64 = 0.0;
                    match data {
                        DashboardData::PerBlock(ref blocks) => blocks
                            .iter()
                            .map(|b| {
                                cumulative += b.size as f64 / 1_000_000_000.0;
                                (b.timestamp * 1000, (cumulative * 1000.0).round() / 1000.0)
                            })
                            .collect(),
                        DashboardData::Daily(ref days) => days
                            .iter()
                            .filter_map(|d| {
                                cumulative += d.avg_size * d.block_count as f64 / 1_000_000_000.0;
                                let ts = chrono::NaiveDate::parse_from_str(&d.date, "%Y-%m-%d")
                                    .map(|dt| {
                                        dt.and_hms_opt(12, 0, 0)
                                            .unwrap()
                                            .and_utc()
                                            .timestamp() as u64
                                            * 1000
                                    })
                                    .ok()?;
                                Some((ts, (cumulative * 1000.0).round() / 1000.0))
                            })
                            .collect(),
                    }
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        crate::stats::charts::OverlayFlags {
            halvings: overlay_halvings.get(),
            bip_activations: overlay_bips.get(),
            core_releases: overlay_core.get(),
            events: overlay_events.get(),
            price_data,
            chain_size_data,
        }
    });

    // ---- Dashboard chart options (with overlay support) ----
    // Helper: build a chart option signal that applies overlays.
    // `chart_fn` returns (json_string, is_daily) so apply_overlays knows the axis type.
    macro_rules! chart_signal {
        ($data:expr, $range:expr, $overlays:expr, |$blocks:ident| $per_block:expr, |$days:ident| $daily:expr) => {
            Signal::derive(move || {
                let _r = $range.get();
                let flags = $overlays.get();
                $data
                    .get()
                    .and_then(|r| r.ok())
                    .map(|data| match data {
                        DashboardData::PerBlock(ref $blocks) => {
                            let json = $per_block;
                            crate::stats::charts::apply_overlays(&json, &flags, false)
                        }
                        DashboardData::Daily(ref $days) => {
                            let json = $daily;
                            crate::stats::charts::apply_overlays(&json, &flags, true)
                        }
                    })
                    .unwrap_or_default()
            })
        };
    }

    let size_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::block_size_chart(blocks),
        |days| crate::stats::charts::block_size_chart_daily(days)
    );

    let tx_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::tx_count_chart(blocks),
        |days| crate::stats::charts::tx_count_chart_daily(days)
    );

    let fees_option = Signal::derive(move || {
        let _r = range.get();
        let unit = fee_unit.get();
        let flags = overlay_flags.get();
        dashboard_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                DashboardData::PerBlock(ref blocks) => {
                    let json = crate::stats::charts::fees_chart_unit(blocks, &unit);
                    crate::stats::charts::apply_overlays(&json, &flags, false)
                }
                DashboardData::Daily(ref days) => {
                    let json = crate::stats::charts::fees_chart_daily_unit(days, &unit);
                    crate::stats::charts::apply_overlays(&json, &flags, true)
                }
            })
            .unwrap_or_default()
    });

    let diff_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::difficulty_chart(blocks),
        |days| crate::stats::charts::difficulty_chart_daily(days)
    );

    let interval_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::block_interval_chart(blocks),
        |days| crate::stats::charts::block_interval_chart_daily(days)
    );

    let weight_util_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::weight_utilization_chart(blocks),
        |days| crate::stats::charts::weight_utilization_chart_daily(days)
    );

    let subsidy_fees_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::subsidy_vs_fees_chart(blocks),
        |days| crate::stats::charts::subsidy_vs_fees_chart_daily(days)
    );

    let avg_tx_size_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::avg_tx_size_chart(blocks),
        |days| crate::stats::charts::avg_tx_size_chart_daily(days)
    );

    // Halving countdown derived values
    let raw_block_height = Signal::derive(move || {
        live.get()
            .and_then(|r| r.ok())
            .map(|s| s.blockchain.blocks)
            .unwrap_or(0)
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

    let halving_est_date = Signal::derive(move || {
        let remaining = halving_blocks_remaining.get();
        if remaining == 0 {
            return "\u{2014}".to_string();
        }
        let days = remaining as f64 * 10.0 / 1440.0;
        let est = chrono::Utc::now()
            + chrono::Duration::seconds((days * 86400.0) as i64);
        est.format("%b %d, %Y").to_string()
    });

    let halving_est_days = Signal::derive(move || {
        let remaining = halving_blocks_remaining.get();
        (remaining as f64 * 10.0 / 1440.0 * 10.0).round() / 10.0
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

    // Difficulty adjustment predictor derived values
    let diff_period_start = Signal::derive(move || {
        let h = raw_block_height.get();
        if h == 0 {
            return 0u64;
        }
        (h / 2016) * 2016
    });

    let diff_blocks_into_period = Signal::derive(move || {
        let h = raw_block_height.get();
        let ps = diff_period_start.get();
        h.saturating_sub(ps)
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

    // Fetch the period start block timestamp for difficulty change calculation
    let period_start_ts = LocalResource::new(move || {
        let ps = diff_period_start.get();
        async move {
            if ps == 0 {
                return 0u64;
            }
            fetch_block_timestamp(ps).await.ok().flatten().unwrap_or(0)
        }
    });

    // Expected difficulty change: (target_time / projected_time - 1) * 100%
    let diff_expected_change = Signal::derive(move || {
        let blocks_in = diff_blocks_into_period.get();
        if blocks_in < 10 {
            return "\u{2014}".to_string();
        }
        let start_ts = period_start_ts.get().unwrap_or(0);
        if start_ts == 0 {
            return "\u{2014}".to_string();
        }
        // Get current block timestamp (non-reactive)
        let guard = live.read_untracked();
        let current_ts = guard
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(|s| s.blockchain.time)
            .unwrap_or(0);
        if current_ts <= start_ts {
            return "\u{2014}".to_string();
        }

        let elapsed = (current_ts - start_ts) as f64;
        // Project total period time: elapsed * 2016 / blocks_in
        let projected = elapsed * 2016.0 / blocks_in as f64;
        // Target total time: 2016 * 600 = 1,209,600 seconds
        let target = 2016.0 * 600.0;
        // Change = (target / projected - 1) * 100
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

    // ---- OP_RETURN data ----
    #[derive(Clone)]
    enum OpData {
        PerBlock(Vec<OpReturnBlock>),
        Daily(Vec<DailyAggregate>),
    }

    let op_data = LocalResource::new(move || {
        let r = range.get();
        async move {
            let stats =
                fetch_stats_summary().await.map_err(|e| e.to_string())?;
            let n = range_to_blocks(&r);

            if n > 5_000 {
                let from_ts = stats.latest_timestamp.saturating_sub(n * 600);
                let days =
                    fetch_daily_aggregates(from_ts, stats.latest_timestamp)
                        .await
                        .map_err(|e| e.to_string())?;
                Ok::<_, String>(OpData::Daily(days))
            } else {
                let from =
                    stats.min_height.max(stats.max_height.saturating_sub(n));
                let blocks = fetch_op_returns(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>(OpData::PerBlock(blocks))
            }
        }
    });

    let op_count_option = Signal::derive(move || {
        let flags = overlay_flags.get();
        op_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                OpData::PerBlock(ref b) => {
                    let json = crate::stats::charts::op_return_count_chart(b);
                    crate::stats::charts::apply_overlays(&json, &flags, false)
                }
                OpData::Daily(ref d) => {
                    let json = crate::stats::charts::op_return_count_chart_daily(d);
                    crate::stats::charts::apply_overlays(&json, &flags, true)
                }
            })
            .unwrap_or_default()
    });

    let op_bytes_option = Signal::derive(move || {
        let flags = overlay_flags.get();
        op_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                OpData::PerBlock(ref b) => {
                    let json = crate::stats::charts::op_return_bytes_chart(b);
                    crate::stats::charts::apply_overlays(&json, &flags, false)
                }
                OpData::Daily(ref d) => {
                    let json = crate::stats::charts::op_return_bytes_chart_daily(d);
                    crate::stats::charts::apply_overlays(&json, &flags, true)
                }
            })
            .unwrap_or_default()
    });

    let runes_pct_option = Signal::derive(move || {
        let flags = overlay_flags.get();
        op_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                OpData::PerBlock(ref b) => {
                    let json = crate::stats::charts::runes_pct_chart(b);
                    crate::stats::charts::apply_overlays(&json, &flags, false)
                }
                OpData::Daily(ref d) => {
                    let json = crate::stats::charts::runes_pct_chart_daily(d);
                    crate::stats::charts::apply_overlays(&json, &flags, true)
                }
            })
            .unwrap_or_default()
    });

    let op_block_share_option = Signal::derive(move || {
        let flags = overlay_flags.get();
        op_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                OpData::PerBlock(ref b) => {
                    let json = crate::stats::charts::op_return_block_share_chart(b);
                    crate::stats::charts::apply_overlays(&json, &flags, false)
                }
                OpData::Daily(ref d) => {
                    let json = crate::stats::charts::op_return_block_share_chart_daily(d);
                    crate::stats::charts::apply_overlays(&json, &flags, true)
                }
            })
            .unwrap_or_default()
    });

    // ---- Mining tab data ----
    let mining_data = LocalResource::new(move || {
        let r = range.get();
        async move {
            let stats =
                fetch_stats_summary().await.map_err(|e| e.to_string())?;
            let n = range_to_blocks(&r);
            let is_daily = n > 5_000;

            if is_daily {
                let from_ts = stats.latest_timestamp.saturating_sub(n * 600);
                let miners = fetch_miner_dominance_daily(
                    from_ts,
                    stats.latest_timestamp,
                )
                .await
                .map_err(|e| e.to_string())?;
                // For empty blocks in daily mode, use height range approximation
                let from =
                    stats.min_height.max(stats.max_height.saturating_sub(n));
                let empty = fetch_empty_blocks(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>((miners, empty))
            } else {
                let from =
                    stats.min_height.max(stats.max_height.saturating_sub(n));
                let miners = fetch_miner_dominance(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?;
                let empty = fetch_empty_blocks(from, stats.max_height)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok((miners, empty))
            }
        }
    });

    let miner_chart_option = Signal::derive(move || {
        mining_data
            .get()
            .and_then(|r| r.ok())
            .map(|(ref miners, _)| {
                crate::stats::charts::miner_dominance_chart(miners)
            })
            .unwrap_or_default()
    });

    let empty_blocks_option = Signal::derive(move || {
        let flags = overlay_flags.get();
        mining_data
            .get()
            .and_then(|r| r.ok())
            .map(|(_, ref empty)| {
                let json = crate::stats::charts::empty_blocks_chart(empty);
                // Empty blocks chart uses category axis (monthly bars)
                crate::stats::charts::apply_overlays(&json, &flags, true)
            })
            .unwrap_or_default()
    });

    // ---- SegWit / Taproot chart options (from dashboard_data) ----
    let segwit_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::segwit_adoption_chart(blocks),
        |days| crate::stats::charts::segwit_adoption_chart_daily(days)
    );

    let taproot_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::taproot_chart(blocks),
        |days| crate::stats::charts::taproot_chart_daily(days)
    );

    let witness_version_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::witness_version_chart(blocks),
        |days| crate::stats::charts::witness_version_chart_daily(days)
    );

    let chain_size_option = Signal::derive(move || {
        let _r = range.get();
        let flags = overlay_flags.get();
        // Read disk size without reactive tracking to avoid 30s chart resets
        let disk_gb = {
            let guard = live.read_untracked();
            guard
                .as_ref()
                .and_then(|r| r.as_ref().ok())
                .map(|s| s.network.chain_size_gb)
                .unwrap_or(0.0)
        };
        dashboard_data
            .get()
            .and_then(|r| r.ok())
            .map(|data| match data {
                DashboardData::PerBlock(ref blocks) => {
                    let json = crate::stats::charts::chain_size_chart(blocks, disk_gb);
                    crate::stats::charts::apply_overlays(&json, &flags, false)
                }
                DashboardData::Daily(ref days) => {
                    let json = crate::stats::charts::chain_size_chart_daily(days, disk_gb);
                    crate::stats::charts::apply_overlays(&json, &flags, true)
                }
            })
            .unwrap_or_default()
    });

    let witness_pct_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::witness_version_pct_chart(blocks),
        |days| crate::stats::charts::witness_version_pct_chart_daily(days)
    );

    let witness_tx_pct_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::witness_version_tx_pct_chart(blocks),
        |days| crate::stats::charts::witness_version_tx_pct_chart_daily(days)
    );

    let address_type_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::address_type_chart(blocks),
        |days| crate::stats::charts::address_type_chart_daily(days)
    );

    let witness_share_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::witness_share_chart(blocks),
        |days| crate::stats::charts::witness_share_chart_daily(days)
    );

    let rbf_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::rbf_chart(blocks),
        |days| crate::stats::charts::rbf_chart_daily(days)
    );

    let utxo_flow_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::utxo_flow_chart(blocks),
        |days| crate::stats::charts::utxo_flow_chart_daily(days)
    );

    let inscription_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::inscription_chart(blocks),
        |days| crate::stats::charts::inscription_chart_daily(days)
    );

    let inscription_share_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::inscription_share_chart(blocks),
        |days| crate::stats::charts::inscription_share_chart_daily(days)
    );

    let all_embedded_share_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::all_embedded_share_chart(blocks),
        |days| crate::stats::charts::all_embedded_share_chart_daily(days)
    );

    // ---- BIP Signaling data ----
    let (bip_method, set_bip_method) = signal("bit".to_string());
    // Period offset: 0 = current, 1 = previous, etc.
    let (period_offset, set_period_offset) = signal(0u64);

    let signaling_data = LocalResource::new(move || {
        let method = bip_method.get();
        let offset = period_offset.get();
        async move {
            let stats =
                fetch_stats_summary().await.map_err(|e| e.to_string())?;
            let bit = if method == "locktime" { 0 } else { 4 };

            // Calculate period boundaries
            let current_period = stats.max_height / 2016;
            let target_period = current_period.saturating_sub(offset);
            let period_start = target_period * 2016;
            let period_end = (period_start + 2015).min(stats.max_height);

            let blocks_result =
                fetch_signaling(bit, method.clone(), period_start, period_end)
                    .await
                    .map_err(|e| e.to_string())?;

            // Only fetch periods chart data once (for the history chart)
            let periods = fetch_signaling_periods(bit, method)
                .await
                .map_err(|e| e.to_string())?;

            Ok::<_, String>((
                blocks_result,
                periods,
                period_start,
                period_end,
                current_period,
                target_period,
            ))
        }
    });

    // ---- Tab names and ranges ----
    let tabs = vec![
        ("overview", "Overview"),
        ("network", "Network"),
        ("fees", "Fees"),
        ("mining", "Mining"),
        ("opreturn", "Embedded Data"),
        ("signaling", "Signaling"),
    ];

    view! {
        <Title text="The Bitcoin Observatory - WE HODL BTC"/>

        <section class="max-w-[1600px] mx-auto px-4 lg:px-10 pt-10 pb-28 opacity-0 animate-fadeinone">
            // Page header
            <div class="text-center mb-10">
                <h1 class="text-4xl lg:text-5xl font-title text-white mb-3">"The Bitcoin Observatory"</h1>
                <div class="w-16 h-0.5 bg-[#f7931a] mx-auto mt-3 mb-4"></div>
                <p class="text-base text-white/50 max-w-xl mx-auto">
                    "Live blockchain metrics, block data, embedded data analysis, and BIP signaling tracker."
                </p>
            </div>

            // Tab navigation
            <div class="flex flex-wrap gap-3 justify-center mb-8">
                {tabs.into_iter().map(|(id, label)| {
                    let id = id.to_string();
                    let label = label.to_string();
                    let id_clone = id.clone();
                    view! {
                        <button
                            class=move || {
                                if tab.get() == id_clone {
                                    "px-5 py-2.5 text-base rounded-xl bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer transition-all"
                                } else {
                                    "px-5 py-2.5 text-base rounded-xl text-white/50 hover:text-white hover:bg-white/10 border border-transparent hover:border-white/10 transition-all cursor-pointer"
                                }
                            }
                            on:click={
                                let id = id.clone();
                                move |_| set_tab.set(id.clone())
                            }
                        >
                            {label}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>

            // Range selector (hidden on overview and signaling tabs)
            <div class=move || {
                let t = tab.get();
                if t == "overview" || t == "signaling" {
                    "hidden"
                } else {
                    "flex justify-center mb-8"
                }
            }>
                <div class="inline-flex flex-wrap gap-1.5 bg-[#0a1a2e] rounded-xl p-1.5 border border-white/5">
                    {["1d", "1w", "1m", "3m", "6m", "1y", "2y", "5y", "10y", "all"].into_iter().map(|r| {
                        let r_str = r.to_string();
                        let r_display = r.to_uppercase();
                        let r_clone = r_str.clone();
                        view! {
                            <button
                                class=move || {
                                    if range.get() == r_clone {
                                        "px-3 py-1 text-xs rounded-lg bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
                                    } else {
                                        "px-3 py-1 text-xs rounded-lg text-white/40 hover:text-white/70 hover:bg-white/5 transition-all cursor-pointer"
                                    }
                                }
                                on:click={
                                    let r = r_str.clone();
                                    move |_| set_range.set(r.clone())
                                }
                            >
                                {r_display}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>
                <span class="ml-3 text-xs text-white/30 self-center">
                    {move || {
                        let n = range_to_blocks(&range.get());
                        if n > 5_000 { "daily averages" } else { "per block" }
                    }}
                </span>
            </div>

            // ===== FLOATING OVERLAY PANEL =====
            <div class="fixed left-4 bottom-4 z-50">
                <Show
                    when=move || overlay_panel_open.get()
                    fallback=move || view! {
                        <button
                            class="bg-[#0d2137] border border-white/10 hover:border-[#f7931a]/50 text-white/60 hover:text-white rounded-xl p-3 shadow-lg cursor-pointer transition-all"
                            title="Chart Overlays"
                            on:click=move |_| set_overlay_panel_open.set(true)
                        >
                            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M6 13.5V3.75m0 9.75a1.5 1.5 0 0 1 0 3m0-3a1.5 1.5 0 0 0 0 3m0 3.75V16.5m12-3V3.75m0 9.75a1.5 1.5 0 0 1 0 3m0-3a1.5 1.5 0 0 0 0 3m0 3.75V16.5m-6-9V3.75m0 3.75a1.5 1.5 0 0 1 0 3m0-3a1.5 1.5 0 0 0 0 3m0 9.75V10.5"/>
                            </svg>
                        </button>
                    }
                >
                    <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4 shadow-xl min-w-[180px]">
                        <div class="flex items-center justify-between mb-3">
                            <span class="text-xs text-white/50 uppercase tracking-widest font-semibold">"Overlays"</span>
                            <button
                                class="text-white/30 hover:text-white/60 cursor-pointer p-0.5"
                                on:click=move |_| set_overlay_panel_open.set(false)
                            >
                                <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>
                                </svg>
                            </button>
                        </div>
                        <div class="space-y-2">
                            <label class="flex items-center gap-2 cursor-pointer group">
                                <input
                                    type="checkbox"
                                    class="accent-[#f7931a] w-3.5 h-3.5 cursor-pointer"
                                    prop:checked=move || overlay_halvings.get()
                                    on:change=move |_| set_overlay_halvings.update(|v| *v = !*v)
                                />
                                <span class="text-sm text-white/60 group-hover:text-white/80 transition-colors">"Halvings"</span>
                                <span class="text-xs text-[#f7931a]/60 ml-auto">"- -"</span>
                            </label>
                            <label class="flex items-center gap-2 cursor-pointer group">
                                <input
                                    type="checkbox"
                                    class="accent-[#4ecdc4] w-3.5 h-3.5 cursor-pointer"
                                    prop:checked=move || overlay_bips.get()
                                    on:change=move |_| set_overlay_bips.update(|v| *v = !*v)
                                />
                                <span class="text-sm text-white/60 group-hover:text-white/80 transition-colors">"BIP Activations"</span>
                                <span class="text-xs text-[#4ecdc4]/60 ml-auto">"\u{2026}"</span>
                            </label>
                            <label class="flex items-center gap-2 cursor-pointer group">
                                <input
                                    type="checkbox"
                                    class="accent-[#a855f7] w-3.5 h-3.5 cursor-pointer"
                                    prop:checked=move || overlay_core.get()
                                    on:change=move |_| set_overlay_core.update(|v| *v = !*v)
                                />
                                <span class="text-sm text-white/60 group-hover:text-white/80 transition-colors">"Core Releases"</span>
                                <span class="text-xs text-[#a855f7]/60 ml-auto">"\u{2026}"</span>
                            </label>
                            <label class="flex items-center gap-2 cursor-pointer group">
                                <input
                                    type="checkbox"
                                    class="accent-[#ef4444] w-3.5 h-3.5 cursor-pointer"
                                    prop:checked=move || overlay_events.get()
                                    on:change=move |_| set_overlay_events.update(|v| *v = !*v)
                                />
                                <span class="text-sm text-white/60 group-hover:text-white/80 transition-colors">"Events"</span>
                                <span class="text-xs text-[#ef4444]/60 ml-auto">"\u{2605}"</span>
                            </label>
                            <label class="flex items-center gap-2 cursor-pointer group">
                                <input
                                    type="checkbox"
                                    class="accent-[#e6c84e] w-3.5 h-3.5 cursor-pointer"
                                    prop:checked=move || overlay_price.get()
                                    on:change=move |_| set_overlay_price.update(|v| *v = !*v)
                                />
                                <span class="text-sm text-white/60 group-hover:text-white/80 transition-colors">"Price (USD)"</span>
                                <span class="text-xs text-[#e6c84e]/60 ml-auto">"$"</span>
                            </label>
                            <label class="flex items-center gap-2 cursor-pointer group">
                                <input
                                    type="checkbox"
                                    class="accent-[#10b981] w-3.5 h-3.5 cursor-pointer"
                                    prop:checked=move || overlay_chain_size.get()
                                    on:change=move |_| set_overlay_chain_size.update(|v| *v = !*v)
                                />
                                <span class="text-sm text-white/60 group-hover:text-white/80 transition-colors">"Chain Size"</span>
                                <span class="text-xs text-[#10b981]/60 ml-auto">"GB"</span>
                            </label>
                        </div>
                    </div>
                </Show>
            </div>

            // ===== OVERVIEW TAB =====
            <div class=move || if tab.get() == "overview" { "block" } else { "hidden" }>

                // Live stats panel
                <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-6 lg:p-8 mb-8">
                    <div class="flex items-center gap-2 mb-3 flex-wrap">
                        <div class="w-2.5 h-2.5 rounded-full bg-green-500 animate-pulse"></div>
                        <span class="text-lg text-white font-bold">"Live Node Stats"</span>
                        <div class="flex items-center gap-2 ml-auto">
                            <span class="text-xs text-white/30">{move || last_updated.get()}</span>
                            <span class="text-xs text-white/20">
                                {move || format!("{}s", countdown.get())}
                            </span>
                            <button
                                class="text-xs text-white/40 hover:text-white/70 px-2 py-0.5 rounded border border-white/10 hover:border-white/20 cursor-pointer transition-all"
                                on:click=move |_| {
                                    set_countdown.set(30);
                                    live.refetch();
                                    set_last_updated.set(format!("updated {}", chrono::Local::now().format("%H:%M:%S")));
                                }
                            >
                                "Refresh"
                            </button>
                        </div>
                    </div>

                    <div class="grid grid-cols-1 lg:grid-cols-3 gap-4">
                        // Mempool section
                        <div class="bg-[#0a1a2e] border border-white/10 rounded-xl p-5 overflow-hidden">
                            <h3 class="text-sm font-bold text-[#f7931a] uppercase tracking-widest mb-4">"Mempool"</h3>
                            <div class="grid grid-cols-2 gap-3 mb-3">
                                <LiveCard label="Transactions" value=mempool_size/>
                                <LiveCard label="Size" value=mempool_bytes/>
                                <LiveCard label="Next Block Fee" value=next_fee/>
                            </div>
                            <div class="flex justify-center">
                                <Chart id="mempool-gauge".to_string() option=gauge_option class="w-[220px] h-[200px]".to_string()/>
                            </div>
                        </div>

                        // Mining section
                        <div class="bg-[#0a1a2e] border border-white/10 rounded-xl p-5">
                            <h3 class="text-sm font-bold text-[#f7931a] uppercase tracking-widest mb-4">"Mining"</h3>
                            <div class="grid grid-cols-2 gap-3 mb-2">
                                <LiveCard label="Block Height" value=block_height/>
                                <LiveCard label="Difficulty" value=difficulty/>
                                <LiveCard label="Hashrate" value=hashrate/>
                                <LiveCard label="Chain Size" value=chain_size/>
                            </div>
                        </div>

                        // Economic section
                        <div class="bg-[#0a1a2e] border border-white/10 rounded-xl p-5">
                            <h3 class="text-sm font-bold text-[#f7931a] uppercase tracking-widest mb-4">"Economic"</h3>
                            <div class="grid grid-cols-2 gap-3">
                                <LiveCard label="Price (USD)" value=price_usd/>
                                <LiveCard label="Sats/Dollar" value=sats_per_dollar/>
                                <LiveCard label="Market Cap" value=market_cap/>
                                <LiveCard label="Total Supply" value=total_supply/>
                                <LiveCard label="% Issued" value=supply_pct/>
                                <LiveCard label="UTXO Count" value=utxo_count/>
                            </div>
                        </div>
                    </div>
                </div>

                // Difficulty adjustment predictor
                <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 mt-8">
                    <div class="flex items-baseline justify-between mb-3">
                        <h3 class="text-lg text-white font-semibold">"Next Difficulty Adjustment"</h3>
                        <span class="text-xs text-white/40 font-mono">{move || diff_est_date.get()}</span>
                    </div>
                    <div class="grid grid-cols-2 lg:grid-cols-4 gap-4">
                        <div class="text-center">
                            <div class="text-[0.65rem] text-white/50 uppercase tracking-widest mb-1">"Period Start"</div>
                            <div class="text-lg text-[#f7931a] font-bold font-mono">{move || format_number(diff_period_start.get())}</div>
                        </div>
                        <div class="text-center">
                            <div class="text-[0.65rem] text-white/50 uppercase tracking-widest mb-1">"Blocks Into Period"</div>
                            <div class="text-lg text-[#f7931a] font-bold font-mono">{move || format_number(diff_blocks_into_period.get())}</div>
                        </div>
                        <div class="text-center">
                            <div class="text-[0.65rem] text-white/50 uppercase tracking-widest mb-1">"Blocks Remaining"</div>
                            <div class="text-lg text-[#f7931a] font-bold font-mono">{move || format_number(diff_blocks_remaining.get())}</div>
                        </div>
                        <div class="text-center">
                            <div class="text-[0.65rem] text-white/50 uppercase tracking-widest mb-1">"Est. Days Left"</div>
                            <div class="text-lg text-[#f7931a] font-bold font-mono">{move || diff_est_remaining_days.get()}</div>
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
                        <h3 class="text-lg text-white font-semibold">"Next Halving"</h3>
                        <span class="text-xs text-white/40 font-mono">{move || halving_est_date.get()}</span>
                    </div>
                    <div class="grid grid-cols-2 lg:grid-cols-4 gap-4">
                        <div class="text-center">
                            <div class="text-[0.65rem] text-white/50 uppercase tracking-widest mb-1">"Target Height"</div>
                            <div class="text-lg text-[#f7931a] font-bold font-mono">{move || format_number(next_halving_height.get())}</div>
                        </div>
                        <div class="text-center">
                            <div class="text-[0.65rem] text-white/50 uppercase tracking-widest mb-1">"Blocks Remaining"</div>
                            <div class="text-lg text-[#f7931a] font-bold font-mono">{move || format_number(halving_blocks_remaining.get())}</div>
                        </div>
                        <div class="text-center">
                            <div class="text-[0.65rem] text-white/50 uppercase tracking-widest mb-1">"Est. Days"</div>
                            <div class="text-lg text-[#f7931a] font-bold font-mono">{move || format!("{:.1}", halving_est_days.get())}</div>
                        </div>
                        <div class="text-center">
                            <div class="text-[0.65rem] text-white/50 uppercase tracking-widest mb-1">"Current Subsidy"</div>
                            <div class="text-lg text-[#f7931a] font-bold font-mono">{move || current_subsidy_btc.get()}</div>
                        </div>
                    </div>
                    <div class="mt-4 px-1">
                        <div class="flex items-center justify-between mb-1.5">
                            <span class="text-xs text-white/40">"Progress"</span>
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
                        <span class="text-xs text-white/60 font-mono">{move || next_subsidy_btc.get()}</span>
                    </div>
                </div>

            </div>

            // ===== NETWORK TAB =====
            <div class=move || if tab.get() == "network" { "block" } else { "hidden" }>

                // Sub-section pills
                <div class="flex flex-wrap gap-2 justify-center mb-6">
                    {[("blocks", "Blocks"), ("adoption", "Adoption"), ("tx-metrics", "Transactions")].into_iter().map(|(id, label)| {
                        let id_str = id.to_string();
                        let id_clone = id_str.clone();
                        view! {
                            <button
                                class=move || {
                                    if network_section.get() == id_clone {
                                        "px-4 py-1.5 text-xs rounded-lg bg-white/10 text-white font-semibold border border-white/20 cursor-pointer"
                                    } else {
                                        "px-4 py-1.5 text-xs rounded-lg text-white/40 hover:text-white/70 hover:bg-white/5 transition-all cursor-pointer"
                                    }
                                }
                                on:click={
                                    let id = id_str.clone();
                                    move |_| set_network_section.set(id.clone())
                                }
                            >
                                {label}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                <Suspense fallback=move || view! {
                    <div class="space-y-10">
                        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 h-[450px] animate-pulse"></div>
                        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 h-[450px] animate-pulse"></div>
                    </div>
                }>
                    {move || {
                        let _d = dashboard_data.get();
                        view! {
                            // --- Blocks sub-section ---
                            <div class=move || if network_section.get() == "blocks" { "space-y-10" } else { "hidden" }>
                                <ChartCard title="Block Size" description="Raw block size in megabytes over time" chart_id="chart-size" option=size_option/>
                                <ChartCard title="Weight Utilization" description="Block weight as percentage of the 4 MWU limit" chart_id="chart-weight-util" option=weight_util_option/>
                                <ChartCard title="Transaction Count" description="Number of transactions per block" chart_id="chart-txcount" option=tx_option/>
                                <ChartCard title="Avg Transaction Size" description="Average transaction size in bytes (block size / tx count)" chart_id="chart-avg-tx-size" option=avg_tx_size_option/>
                                <ChartCard title="Block Interval" description="Time between consecutive blocks in minutes" chart_id="chart-interval" option=interval_option/>
                                <ChartCard title="Chain Size Growth" description="Cumulative blockchain size — visualize growth acceleration after protocol changes" chart_id="chart-chain-size" option=chain_size_option/>
                            </div>

                            // --- Adoption sub-section ---
                            <div class=move || if network_section.get() == "adoption" { "space-y-10" } else { "hidden" }>
                                <ChartCard title="SegWit Adoption" description="Percentage of transactions using Segregated Witness" chart_id="chart-segwit" option=segwit_option/>
                                <ChartCard title="Taproot Outputs" description="Number of Taproot (v1 witness) outputs created per block" chart_id="chart-taproot" option=taproot_option/>
                                <ChartCard title="Witness Version Comparison" description="SegWit v0 vs Taproot v1 witness spends — stacked to show total and relative adoption" chart_id="chart-witness-versions" option=witness_version_option/>
                                <ChartCard title="Witness Version Share" description="SegWit v0 vs Taproot v1 as percentage of total witness spends" chart_id="chart-witness-pct" option=witness_pct_option/>
                                <ChartCard title="Transaction Type Breakdown" description="Legacy vs SegWit v0 vs Taproot v1 as percentage of all transactions" chart_id="chart-witness-tx-pct" option=witness_tx_pct_option/>
                                <ChartCard title="Address Type Evolution" description="Output script types over time — P2PKH, P2SH, P2WPKH, P2WSH, P2TR, P2PK" chart_id="chart-address-types" option=address_type_option/>
                                <ChartCard title="Witness Data Share" description="Witness data as percentage of total block size — shows SegWit discount impact" chart_id="chart-witness-share" option=witness_share_option/>
                            </div>

                            // --- Transaction Metrics sub-section ---
                            <div class=move || if network_section.get() == "tx-metrics" { "space-y-10" } else { "hidden" }>
                                <ChartCard title="RBF Adoption" description="Percentage of transactions signaling Replace-By-Fee (nSequence < 0xFFFFFFFE)" chart_id="chart-rbf" option=rbf_option/>
                                <ChartCard title="UTXO Flow" description="Inputs consumed vs outputs created per block — net UTXO set growth" chart_id="chart-utxo-flow" option=utxo_flow_option/>
                            </div>
                        }
                    }}
                </Suspense>
            </div>

            // ===== FEES TAB =====
            <div class=move || if tab.get() == "fees" { "block" } else { "hidden" }>

                <Suspense fallback=move || view! {
                    <div class="space-y-10">
                        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 h-[450px] animate-pulse"></div>
                        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 h-[450px] animate-pulse"></div>
                    </div>
                }>
                    {move || {
                        let _d = dashboard_data.get();
                        view! {
                            <div class="space-y-10">
                                <ChartCard
                                    title="Total Fees per Block"
                                    description="Total fees collected by miners per block"
                                    chart_id="chart-fees"
                                    option=fees_option
                                >
                                    <button
                                        class="text-xs text-white/40 hover:text-white/60 px-2 py-1 rounded border border-white/10 cursor-pointer"
                                        on:click=move |_| {
                                            set_fee_unit.update(|u| {
                                                *u = if *u == "sats" { "btc".to_string() } else { "sats".to_string() }
                                            });
                                        }
                                    >
                                        {move || if fee_unit.get() == "sats" { "Switch to BTC" } else { "Switch to sats" }}
                                    </button>
                                </ChartCard>
                                <ChartCard
                                    title="Subsidy vs Fees"
                                    description="Block reward breakdown: subsidy (coinbase) vs transaction fees in BTC"
                                    chart_id="chart-subsidy-fees"
                                    option=subsidy_fees_option
                                />
                            </div>
                        }
                    }}
                </Suspense>
            </div>

            // ===== MINING TAB =====
            <div class=move || if tab.get() == "mining" { "block" } else { "hidden" }>

                // Sub-section pills
                <div class="flex flex-wrap gap-2 justify-center mb-6">
                    {[("difficulty", "Difficulty"), ("pools", "Pool Distribution")].into_iter().map(|(id, label)| {
                        let id_str = id.to_string();
                        let id_clone = id_str.clone();
                        view! {
                            <button
                                class=move || {
                                    if mining_section.get() == id_clone {
                                        "px-4 py-1.5 text-xs rounded-lg bg-white/10 text-white font-semibold border border-white/20 cursor-pointer"
                                    } else {
                                        "px-4 py-1.5 text-xs rounded-lg text-white/40 hover:text-white/70 hover:bg-white/5 transition-all cursor-pointer"
                                    }
                                }
                                on:click={
                                    let id = id_str.clone();
                                    move |_| set_mining_section.set(id.clone())
                                }
                            >
                                {label}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                <Suspense fallback=move || view! {
                    <div class="space-y-10">
                        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 h-[450px] animate-pulse"></div>
                    </div>
                }>
                    {move || {
                        let _d = dashboard_data.get();
                        let _m = mining_data.get();
                        view! {
                            // --- Difficulty sub-section ---
                            <div class=move || if mining_section.get() == "difficulty" { "space-y-10" } else { "hidden" }>
                                <ChartCard title="Difficulty" description="Mining difficulty target, adjusts every 2,016 blocks" chart_id="chart-difficulty" option=diff_option/>
                            </div>

                            // --- Pool Distribution sub-section ---
                            <div class=move || if mining_section.get() == "pools" { "space-y-10" } else { "hidden" }>
                                <ChartCard title="Miner Dominance" description="Mining pool market share for the selected period" chart_id="chart-miner-dominance" option=miner_chart_option/>
                                <ChartCard title="Empty Blocks" description="Blocks containing only the coinbase transaction (no user transactions)" chart_id="chart-empty-blocks" option=empty_blocks_option/>
                            </div>
                        }
                    }}
                </Suspense>
            </div>

            // ===== EMBEDDED DATA TAB =====
            <div class=move || if tab.get() == "opreturn" { "block" } else { "hidden" }>

                // Sub-section pills
                <div class="flex flex-wrap gap-2 justify-center mb-6">
                    {[("overview", "Overview"), ("protocols", "OP_RETURN Protocols"), ("witness", "Witness Embedding")].into_iter().map(|(id, label)| {
                        let id_str = id.to_string();
                        let id_clone = id_str.clone();
                        view! {
                            <button
                                class=move || {
                                    if embedded_section.get() == id_clone {
                                        "px-4 py-1.5 text-xs rounded-lg bg-white/10 text-white font-semibold border border-white/20 cursor-pointer"
                                    } else {
                                        "px-4 py-1.5 text-xs rounded-lg text-white/40 hover:text-white/70 hover:bg-white/5 transition-all cursor-pointer"
                                    }
                                }
                                on:click={
                                    let id = id_str.clone();
                                    move |_| set_embedded_section.set(id.clone())
                                }
                            >
                                {label}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                <Suspense fallback=move || view! {
                    <div class="space-y-10">
                        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 h-[450px] animate-pulse"></div>
                    </div>
                }>
                    {move || {
                        let _d = op_data.get();
                        let _dd = dashboard_data.get();
                        view! {
                            // --- Overview sub-section (unified view) ---
                            <div class=move || if embedded_section.get() == "overview" { "space-y-10" } else { "hidden" }>
                                <ChartCard title="All Embedded Data — Block Share" description="OP_RETURN + Ordinals inscription data as percentage of total block size" chart_id="chart-all-embedded-share" option=all_embedded_share_option/>
                            </div>

                            // --- OP_RETURN Protocols sub-section ---
                            <div class=move || if embedded_section.get() == "protocols" { "space-y-10" } else { "hidden" }>
                                <ChartCard title="Embedded Data Count" description="OP_RETURN outputs by protocol (Runes, Omni, Counterparty, Other)" chart_id="chart-opreturn-count" option=op_count_option/>
                                <ChartCard title="Embedded Data Volume" description="Data volume in OP_RETURN outputs by protocol (bytes)" chart_id="chart-opreturn-bytes" option=op_bytes_option/>
                                <ChartCard title="Protocol Dominance" description="Share of OP_RETURN outputs by protocol — Runes, Omni, Counterparty, Other" chart_id="chart-runes-pct" option=runes_pct_option/>
                                <ChartCard title="OP_RETURN Block Share" description="OP_RETURN data as percentage of total block size" chart_id="chart-op-block-share" option=op_block_share_option/>
                            </div>

                            // --- Witness Embedding sub-section ---
                            <div class=move || if embedded_section.get() == "witness" { "space-y-10" } else { "hidden" }>
                                <ChartCard title="Ordinals Inscriptions" description="Number of Ordinals inscriptions detected per block (witness envelope pattern)" chart_id="chart-inscriptions" option=inscription_option/>
                                <ChartCard title="Inscription Block Share" description="Inscription data as percentage of total block size" chart_id="chart-inscription-share" option=inscription_share_option/>
                            </div>
                        }
                    }}
                </Suspense>
            </div>

            // ===== SIGNALING TAB =====
            <div class=move || if tab.get() == "signaling" { "block" } else { "hidden" }>

                // BIP selector
                <div class="flex flex-wrap gap-2 justify-center mb-6">
                    <button
                        class=move || if bip_method.get() == "bit" {
                            "px-5 py-2.5 text-base rounded-xl bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer transition-all"
                        } else {
                            "px-5 py-2.5 text-base rounded-xl text-white/50 hover:text-white hover:bg-white/10 border border-transparent hover:border-white/10 transition-all cursor-pointer"
                        }
                        on:click=move |_| set_bip_method.set("bit".to_string())
                    >
                        "BIP-110: OP_RETURN Limits (Bit 4)"
                    </button>
                    <button
                        class=move || if bip_method.get() == "locktime" {
                            "px-5 py-2.5 text-base rounded-xl bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer transition-all"
                        } else {
                            "px-5 py-2.5 text-base rounded-xl text-white/50 hover:text-white hover:bg-white/10 border border-transparent hover:border-white/10 transition-all cursor-pointer"
                        }
                        on:click=move |_| set_bip_method.set("locktime".to_string())
                    >
                        "BIP-54: Consensus Cleanup (Locktime)"
                    </button>
                </div>

                // BIP info card
                <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 mb-6">
                    {move || {
                        if bip_method.get() == "locktime" {
                            view! {
                                <div>
                                    <h3 class="text-lg text-white font-semibold mb-2">"BIP-54: Consensus Cleanup"</h3>
                                    <p class="text-sm text-white/60 leading-relaxed mb-3">"Fixes timewarp attack, reduces worst-case validation time (2,500 sigop limit), prevents 64-byte transaction exploits, and eliminates duplicate coinbase issues. After activation, all blocks must set coinbase nLockTime = height - 1 and nSequence != 0xffffffff as a consensus rule."</p>
                                    <p class="text-sm text-white/60 leading-relaxed mb-3">"The chart below tracks miners already complying with the coinbase requirement. This may indicate readiness, not formal BIP-9 signaling."</p>
                                    <p class="text-sm text-[#f7931a]/70 font-mono">"Tracking: Coinbase locktime compliance | Activation threshold: 95%"</p>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div>
                                    <h3 class="text-lg text-white font-semibold mb-2">"BIP-110: OP_RETURN Data Limits"</h3>
                                    <p class="text-sm text-white/60 leading-relaxed mb-3">"Caps transaction outputs at 34 bytes and OP_RETURN data at 83 bytes. Temporary softfork \u{2014} expires after 52,416 blocks (~1 year). Modified BIP9: 55% threshold (1,109/2,016). Signaled via version bit 4."</p>
                                    <p class="text-sm text-[#f7931a]/70 font-mono">"Signal: Version bit 4 | Threshold: 55%"</p>
                                </div>
                            }.into_any()
                        }
                    }}
                </div>

                // Period navigator
                <div class="flex items-center justify-center gap-4 mb-8">
                    <button
                        class="inline-flex items-center gap-2 px-4 py-2 text-sm rounded-xl text-white/70 border border-white/10 hover:text-white hover:border-white/25 hover:bg-white/5 transition-all cursor-pointer"
                        on:click=move |_| set_period_offset.update(|o| *o = (*o + 1).min(11))
                    >
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
                        </svg>
                        "Older"
                    </button>
                    <span class="text-sm text-white/60 font-medium min-w-[140px] text-center">
                        {move || {
                            let o = period_offset.get();
                            if o == 0 { "Current Period".to_string() } else { format!("{} periods ago", o) }
                        }}
                    </span>
                    <button
                        class=move || {
                            if period_offset.get() == 0 {
                                "inline-flex items-center gap-2 px-4 py-2 text-sm rounded-xl text-white/20 border border-white/5 cursor-not-allowed"
                            } else {
                                "inline-flex items-center gap-2 px-4 py-2 text-sm rounded-xl text-white/70 border border-white/10 hover:text-white hover:border-white/25 hover:bg-white/5 transition-all cursor-pointer"
                            }
                        }
                        disabled=move || period_offset.get() == 0
                        on:click=move |_| set_period_offset.update(|o| *o = o.saturating_sub(1))
                    >
                        "Newer"
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                        </svg>
                    </button>
                </div>

                <Suspense fallback=move || view! {
                    <div class="space-y-10">
                        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 h-[450px] animate-pulse"></div>
                        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 h-[450px] animate-pulse"></div>
                    </div>
                }>
                    {move || {
                        signaling_data.get().map(|result| {
                            match result {
                                Ok(((ref blocks, ref period_stats), ref periods, p_start, p_end, _current_p, _target_p)) => {
                                    let threshold = if bip_method.get() == "locktime" { 95.0 } else { 55.0 };
                                    let mined = period_stats.total_blocks;
                                    let is_current = period_offset.get() == 0;
                                    let remaining = if is_current { 2016u64.saturating_sub(mined) } else { 0 };
                                    let pct = period_stats.signaled_pct;
                                    let bar_width = format!("{}%", (mined as f64 / 2016.0 * 100.0).min(100.0));
                                    let bar_color = if pct >= threshold { "#2ecc71" } else { "#e74c3c" };

                                    let period_text = if is_current {
                                        format!(
                                            "Period {} \u{2013} {}: {} signaled / {} mined of 2,016 ({:.1}%) \u{2014} {} remaining \u{2014} threshold: {}%",
                                            format_number(p_start), format_number(p_end),
                                            period_stats.signaled_count, mined, pct, remaining, threshold as u32,
                                        )
                                    } else {
                                        format!(
                                            "Period {} \u{2013} {}: {} signaled / {} blocks ({:.1}%) \u{2014} threshold: {}%",
                                            format_number(p_start), format_number(p_end),
                                            period_stats.signaled_count, mined, pct, threshold as u32,
                                        )
                                    };

                                    // Build grid cells
                                    let grid_cells = blocks.iter().map(|b| {
                                        let color = if b.signaled { "bg-green-500/70" } else { "bg-red-500/30" };
                                        let title = format!("#{} \u{2014} {}{}", b.height, b.miner, if b.signaled { " \u{2713}" } else { "" });
                                        let h = b.height;
                                        view! {
                                            <div
                                                class=format!("w-3.5 h-3.5 lg:w-4 lg:h-4 rounded-sm cursor-pointer hover:ring-1 hover:ring-white/50 {color}")
                                                title=title
                                                on:click=move |_| { show_block_detail(h); }
                                            ></div>
                                        }
                                    }).collect::<Vec<_>>();

                                    // Filter periods chart to last 20 relevant periods
                                    let start_height = if bip_method.get() == "locktime" { 940_000u64 } else { 936_000 };
                                    let filtered: Vec<_> = periods.iter()
                                        .filter(|p| p.end_height >= start_height)
                                        .cloned()
                                        .collect();
                                    let periods_chart = crate::stats::charts::signaling_periods_chart(&filtered, threshold);

                                    view! {
                                        <div class="space-y-10">
                                            // Progress bar
                                            <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4">
                                                <div class="h-3 bg-white/5 rounded-full overflow-hidden mb-2">
                                                    <div
                                                        class="h-full rounded-full transition-all duration-500"
                                                        style=format!("width: {bar_width}; background: {bar_color}")
                                                    ></div>
                                                </div>
                                                <p class="text-sm text-white/60 text-center font-mono">{period_text}</p>
                                            </div>

                                            // Block grid
                                            <div class="bg-[#0d2137] border border-white/10 rounded-xl p-4">
                                                <p class="text-sm text-white/50 mb-3">
                                                    {format!("Blocks {} \u{2013} {} (click for details)", format_number(p_start), format_number(p_end))}
                                                </p>
                                                <div class="flex flex-wrap gap-1">
                                                    {grid_cells}
                                                </div>
                                            </div>

                                            // History chart
                                            <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6">
                                                <Chart id="chart-signaling-periods" option=Signal::derive(move || periods_chart.clone())/>
                                            </div>
                                        </div>
                                    }.into_any()
                                }
                                Err(ref e) => {
                                    let msg = format!("Error loading signaling data: {e}");
                                    view! { <p class="text-center text-white/40 text-sm">{msg}</p> }.into_any()
                                }
                            }
                        })
                    }}
                </Suspense>
            </div>

            // Block detail modal (hidden by default, shown via JS)
            <div id="block-detail-modal" class="hidden fixed inset-0 z-50 flex items-center justify-center p-4">
                <div class="absolute inset-0 bg-black/60" onclick="closeBlockDetail()"></div>
                <div class="relative bg-[#0e2a47] border border-white/15 rounded-2xl shadow-2xl w-full max-w-md max-h-[80vh] overflow-y-auto">
                    <div class="flex items-center justify-between px-5 py-3 border-b border-white/10">
                        <span id="block-detail-title" class="text-white font-medium">"Block"</span>
                        <button
                            class="text-white/40 hover:text-white text-lg cursor-pointer"
                            onclick="closeBlockDetail()"
                        >"\u{2715}"</button>
                    </div>
                    <div id="block-detail-body" class="px-5 py-4 text-sm text-white/80 space-y-1"
                        style="--bd-label: rgba(255,255,255,0.5)"
                    ></div>
                </div>
            </div>
        </section>

        // Inline styles for block detail rows (can't use Tailwind for JS-injected HTML)
        <style>"
            .bd-row { display: flex; justify-content: space-between; padding: 4px 0; }
            .bd-row span:first-child { color: rgba(255,255,255,0.5); }
            .bd-row span:last-child { color: rgba(255,255,255,0.85); font-family: monospace; font-size: 12px; }
            .bd-hash { color: #f7931a !important; }
            .bd-divider { border-top: 1px solid rgba(255,255,255,0.08); margin: 8px 0; }
        "</style>
    }
}

// ---------------------------------------------------------------------------
// Data enum for dashboard
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum DashboardData {
    PerBlock(Vec<BlockSummary>),
    Daily(Vec<DailyAggregate>),
}
