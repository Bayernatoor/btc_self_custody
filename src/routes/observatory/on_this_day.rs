//! "On This Day in Bitcoin" — what happened on today's date across every year.

use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::hooks::use_query_map;

use super::helpers::*;
use crate::stats::server_fns::*;
use crate::stats::types::OnThisDayYear;

/// Color temperature based on fee density (fees per block in BTC).
fn fee_color(total_fees: u64, block_count: u64) -> &'static str {
    if block_count == 0 {
        return "#3b82f6"; // cold blue
    }
    let btc_per_block = total_fees as f64 / block_count as f64 / 100_000_000.0;
    if btc_per_block >= 1.0 {
        "#ef4444" // hot red
    } else if btc_per_block >= 0.3 {
        "#f97316" // orange
    } else if btc_per_block >= 0.1 {
        "#f7931a" // bitcoin orange
    } else if btc_per_block >= 0.01 {
        "#eab308" // warm yellow
    } else if btc_per_block > 0.0 {
        "#60a5fa" // light blue
    } else {
        "#3b82f6" // cold blue
    }
}

/// Weight utilization bar (visual block fullness).
fn fullness_bar(pct: f64) -> String {
    let filled = ((pct / 10.0).round() as usize).min(10);
    let empty = 10 - filled;
    format!(
        "{}{}",
        "\u{2588}".repeat(filled),
        "\u{2591}".repeat(empty)
    )
}

#[component]
fn YearCard(year: OnThisDayYear) -> impl IntoView {
    let color = fee_color(year.total_fees, year.block_count);
    let bitcoin_age = year.year - 2009;
    let age_label = if bitcoin_age == 0 {
        "Genesis year".to_string()
    } else {
        format!("Year {bitcoin_age}")
    };

    let fees_btc = year.total_fees as f64 / 100_000_000.0;
    let price_str = if year.price_usd > 0.0 {
        format!("${}", format_number_f64(year.price_usd, 0))
    } else {
        "\u{2014}".to_string()
    };

    let has_events = !year.events.is_empty();

    view! {
        <div
            class="bg-[#0d2137] border border-white/10 rounded-xl overflow-hidden transition-all hover:border-white/20"
            style=format!("border-left: 4px solid {color}")
        >
            <div class="p-4 sm:p-5">
                // Year header
                <div class="flex items-center justify-between mb-3">
                    <div class="flex items-center gap-3">
                        <span class="text-2xl sm:text-3xl font-title text-white font-bold">{year.year}</span>
                        <span class="text-xs text-white/30 bg-white/5 rounded-full px-2.5 py-0.5">{age_label}</span>
                    </div>
                    <span class="text-xs text-white/20 font-mono">
                        {format!("#{}\u{2013}#{}", format_number(year.first_block), format_number(year.last_block))}
                    </span>
                </div>

                // Event badges
                {if has_events {
                    let badges = year.events.iter().map(|e| {
                        view! {
                            <span class="inline-flex items-center gap-1 text-xs bg-[#f7931a]/20 text-[#f7931a] rounded-full px-2.5 py-1 font-medium">
                                <svg class="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                                    <path fill-rule="evenodd" d="M10 18a8 8 0 1 0 0-16 8 8 0 0 0 0 16Zm.75-11.25a.75.75 0 0 0-1.5 0v2.5h-2.5a.75.75 0 0 0 0 1.5h2.5v2.5a.75.75 0 0 0 1.5 0v-2.5h2.5a.75.75 0 0 0 0-1.5h-2.5v-2.5Z" clip-rule="evenodd"/>
                                </svg>
                                {e.clone()}
                            </span>
                        }
                    }).collect::<Vec<_>>();
                    view! { <div class="flex flex-wrap gap-1.5 mb-3">{badges}</div> }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}

                // Stats grid
                <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-3 text-sm">
                    <div>
                        <p class="text-[10px] text-white/30 uppercase tracking-wider">"Blocks"</p>
                        <p class="text-white font-mono">{format_number(year.block_count)}</p>
                    </div>
                    <div>
                        <p class="text-[10px] text-white/30 uppercase tracking-wider">"Transactions"</p>
                        <p class="text-white font-mono">{format_compact(year.total_tx)}</p>
                    </div>
                    <div>
                        <p class="text-[10px] text-white/30 uppercase tracking-wider">"Fees"</p>
                        <p class="font-mono" style=format!("color: {color}")>
                            {format!("{:.4} BTC", fees_btc)}
                        </p>
                    </div>
                    <div>
                        <p class="text-[10px] text-white/30 uppercase tracking-wider">"Price"</p>
                        <p class="text-white font-mono">{price_str}</p>
                    </div>
                    <div>
                        <p class="text-[10px] text-white/30 uppercase tracking-wider">"SegWit"</p>
                        <p class="text-white font-mono">
                            {if year.segwit_pct > 0.0 { format!("{:.0}%", year.segwit_pct) } else { "\u{2014}".to_string() }}
                        </p>
                    </div>
                    <div>
                        <p class="text-[10px] text-white/30 uppercase tracking-wider">"Block Fullness"</p>
                        <p class="text-xs font-mono tracking-tighter" style=format!("color: {color}")>
                            {fullness_bar(year.avg_weight_util)}
                            {format!(" {:.0}%", year.avg_weight_util)}
                        </p>
                    </div>
                </div>

                // Extra metrics row (only if data exists)
                {if year.total_inscriptions > 0 || year.total_runes > 0 || year.taproot_outputs > 0 {
                    view! {
                        <div class="flex flex-wrap gap-x-4 gap-y-1 mt-2 pt-2 border-t border-white/5 text-xs text-white/40">
                            {(year.taproot_outputs > 0).then(|| view! {
                                <span>{format!("Taproot: {}", format_compact(year.taproot_outputs))}</span>
                            })}
                            {(year.total_inscriptions > 0).then(|| view! {
                                <span>{format!("Inscriptions: {}", format_compact(year.total_inscriptions))}</span>
                            })}
                            {(year.total_runes > 0).then(|| view! {
                                <span>{format!("Runes: {}", format_compact(year.total_runes))}</span>
                            })}
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </div>
        </div>
    }
}

#[component]
pub fn OnThisDayPage() -> impl IntoView {
    let query = use_query_map();
    let now = chrono::Utc::now();
    let default_date = format!("{:02}-{:02}", now.month(), now.day());

    let initial_date = query
        .read_untracked()
        .get("date")
        .filter(|s| s.len() == 5 && s.contains('-'))
        .unwrap_or(default_date);

    let (selected_date, set_selected_date) = signal(initial_date);

    let month_day = Signal::derive(move || {
        let d = selected_date.get();
        let parts: Vec<&str> = d.split('-').collect();
        if parts.len() == 2 {
            (
                parts[0].parse::<u32>().unwrap_or(1),
                parts[1].parse::<u32>().unwrap_or(1),
            )
        } else {
            (1, 1)
        }
    });

    let data = LocalResource::new(move || {
        let (m, d) = month_day.get();
        async move { fetch_on_this_day(m, d).await.ok() }
    });

    let display_date = Signal::derive(move || {
        let (m, d) = month_day.get();
        let date = chrono::NaiveDate::from_ymd_opt(2024, m, d); // 2024 is leap year for Feb 29
        date.map(|dt| dt.format("%B %-d").to_string())
            .unwrap_or_else(|| format!("{m}/{d}"))
    });

    let nav_prev = move |_| {
        let (m, d) = month_day.get_untracked();
        let date = chrono::NaiveDate::from_ymd_opt(2024, m, d)
            .and_then(|dt| dt.pred_opt());
        if let Some(prev) = date {
            let new = format!("{:02}-{:02}", prev.month(), prev.day());
            set_selected_date.set(new.clone());
            #[cfg(feature = "hydrate")]
            {
                let window = leptos::prelude::window();
                let pathname = window.location().pathname().unwrap_or_default();
                let url = format!("{pathname}?date={new}");
                let _ = window.history().expect("history").replace_state_with_url(
                    &wasm_bindgen::JsValue::NULL, "", Some(&url),
                );
            }
        }
    };

    let nav_next = move |_| {
        let (m, d) = month_day.get_untracked();
        let date = chrono::NaiveDate::from_ymd_opt(2024, m, d)
            .and_then(|dt| dt.succ_opt());
        if let Some(next) = date {
            let new = format!("{:02}-{:02}", next.month(), next.day());
            set_selected_date.set(new.clone());
            #[cfg(feature = "hydrate")]
            {
                let window = leptos::prelude::window();
                let pathname = window.location().pathname().unwrap_or_default();
                let url = format!("{pathname}?date={new}");
                let _ = window.history().expect("history").replace_state_with_url(
                    &wasm_bindgen::JsValue::NULL, "", Some(&url),
                );
            }
        }
    };

    let nav_today = move |_| {
        let now = chrono::Utc::now();
        let new = format!("{:02}-{:02}", now.month(), now.day());
        set_selected_date.set(new.clone());
        #[cfg(feature = "hydrate")]
        {
            let window = leptos::prelude::window();
            let pathname = window.location().pathname().unwrap_or_default();
            let url = format!("{pathname}?date={new}");
            let _ = window.history().expect("history").replace_state_with_url(
                &wasm_bindgen::JsValue::NULL, "", Some(&url),
            );
        }
    };

    use chrono::Datelike;

    view! {
        <Title text=move || format!("On This Day in Bitcoin: {} | WE HODL BTC", display_date.get())/>
        <Meta name="description" content="What happened on today's date across every year of Bitcoin's existence. Compare blocks, fees, prices, and milestones from 2009 to present."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/on-this-day"/>

        // Header
        <div class="relative rounded-2xl overflow-hidden mb-5">
            <img
                src="/img/observatory_hero.png"
                alt="On This Day in Bitcoin"
                class="w-full h-[100px] sm:h-[120px] lg:h-[140px] object-cover object-center"
            />
            <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/60 to-[#123c64]/30"></div>
            <div class="absolute inset-0 flex flex-col items-center justify-end pb-3 sm:pb-4">
                <h1 class="text-lg sm:text-xl lg:text-2xl font-title text-white mb-0.5 drop-shadow-lg">"On This Day in Bitcoin"</h1>
                <p class="text-[11px] sm:text-xs text-white/50 max-w-lg mx-auto px-4 text-center drop-shadow">"What happened on this date across every year of Bitcoin"</p>
            </div>
        </div>

        // Date navigation
        <div class="flex items-center justify-center gap-4 mb-8">
            <button
                class="text-white/40 hover:text-white/80 cursor-pointer p-2 rounded-lg hover:bg-white/5 transition-colors"
                on:click=nav_prev
                title="Previous day"
            >
                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7"/>
                </svg>
            </button>
            <div class="text-center">
                <p class="text-2xl sm:text-3xl font-title text-[#f7931a] font-bold">{move || display_date.get()}</p>
                <p class="text-xs text-white/30 mt-0.5">{move || {
                    let d = data.get().flatten();
                    d.map(|otd| format!("{} years of data", otd.years.len())).unwrap_or_default()
                }}</p>
            </div>
            <button
                class="text-white/40 hover:text-white/80 cursor-pointer p-2 rounded-lg hover:bg-white/5 transition-colors"
                on:click=nav_next
                title="Next day"
            >
                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7"/>
                </svg>
            </button>
            <button
                class="text-xs text-white/40 hover:text-white/70 px-3 py-1.5 rounded-lg border border-white/10 hover:border-white/20 cursor-pointer transition-colors"
                on:click=nav_today
            >
                "Today"
            </button>
        </div>

        // Year cards
        <div class="space-y-3">
            {move || {
                let d = data.get().flatten();
                match d {
                    Some(otd) => {
                        if otd.years.is_empty() {
                            view! {
                                <div class="text-center text-white/30 py-20">
                                    <p class="text-lg">"No blocks mined on this date"</p>
                                    <p class="text-sm mt-1">"Bitcoin may not have existed yet, or no blocks fell on this calendar day."</p>
                                </div>
                            }.into_any()
                        } else {
                            let cards = otd.years.into_iter().map(|year| {
                                view! { <YearCard year=year/> }
                            }).collect::<Vec<_>>();
                            view! { <div class="space-y-3">{cards}</div> }.into_any()
                        }
                    }
                    None => view! {
                        <div class="flex justify-center py-20">
                            <div class="animate-pulse flex items-center gap-2 text-white/30">
                                <div class="w-3 h-3 rounded-full bg-[#f7931a]/30 animate-ping"></div>
                                "Loading Bitcoin history..."
                            </div>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
