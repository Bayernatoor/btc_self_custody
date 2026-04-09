//! "On This Day in Bitcoin" — what happened on today's date across every year.

use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::hooks::use_query_map;

use super::components::DataLoadError;
use super::helpers::*;
use crate::stats::server_fns::*;
use crate::stats::types::calc_supply;
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
    format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty))
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
    let price_str = if year.price_usd >= 1.0 {
        format!("${}", format_number_f64(year.price_usd, 0))
    } else if year.price_usd > 0.0 {
        format!("${:.4}", year.price_usd)
    } else {
        "\u{2014}".to_string()
    };
    let supply = calc_supply(year.last_block);
    let supply_str = format!("\u{20bf}{}", format_number_f64(supply, 0));
    let mcap_str = if year.price_usd > 0.0 {
        let mcap = supply * year.price_usd;
        if mcap >= 1e12 {
            format!("${:.2}T mcap", mcap / 1e12)
        } else if mcap >= 1e9 {
            format!("${:.1}B mcap", mcap / 1e9)
        } else if mcap >= 1e6 {
            format!("${:.0}M mcap", mcap / 1e6)
        } else {
            format!("${:.0} mcap", mcap)
        }
    } else {
        String::new()
    };

    let has_events = !year.events.is_empty();

    view! {
        <div
            id=format!("year-{}", year.year)
            class="bg-[#0d2137] border border-white/10 rounded-xl transition-all hover:border-white/20"
            style=format!("border-left: 4px solid {color}")
        >
            <div class="p-3 sm:p-5">
                // Year header
                <div class="flex items-center justify-between mb-2 sm:mb-3">
                    <div class="flex items-center gap-2 sm:gap-3">
                        <span class="text-xl sm:text-3xl font-title text-white font-bold">{year.year}</span>
                        <span class="text-xs text-white/50 bg-white/5 rounded-full px-2.5 py-0.5">{age_label}</span>
                    </div>
                    <span class="text-xs text-white/60 font-mono">
                        {format!("#{}\u{2013}#{}", format_number(year.first_block), format_number(year.last_block))}
                    </span>
                </div>

                // Event badges with context
                {if has_events {
                    let events = year.events.iter().map(|e| {
                        view! {
                            <div class="bg-[#f7931a]/10 border border-[#f7931a]/20 rounded-xl p-3 sm:p-4">
                                <div class="flex items-center gap-2 mb-1.5">
                                    <svg class="w-4 h-4 text-[#f7931a] shrink-0" fill="currentColor" viewBox="0 0 20 20">
                                        <path fill-rule="evenodd" d="M10 18a8 8 0 1 0 0-16 8 8 0 0 0 0 16Zm.75-11.25a.75.75 0 0 0-1.5 0v2.5h-2.5a.75.75 0 0 0 0 1.5h2.5v2.5a.75.75 0 0 0 1.5 0v-2.5h2.5a.75.75 0 0 0 0-1.5h-2.5v-2.5Z" clip-rule="evenodd"/>
                                    </svg>
                                    <span class="text-sm font-semibold text-[#f7931a]">{e.title.clone()}</span>
                                    {e.block.map(|height| view! {
                                        <a
                                            href=format!("https://mempool.space/block/{height}")
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="text-[10px] text-white/50 hover:text-[#f7931a] transition-colors ml-auto"
                                        >
                                            {format!("Block #{}", super::helpers::format_number(height))}
                                            " \u{2192}"
                                        </a>
                                    })}
                                </div>
                                <p class="text-xs text-white/60 leading-relaxed pl-6" inner_html=e.context.clone()></p>
                            </div>
                        }
                    }).collect::<Vec<_>>();
                    view! { <div class="space-y-2 mb-3">{events}</div> }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}

                // Stats grid — top 4 always visible, last 2 hidden on mobile
                <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-2 sm:gap-3 text-sm">
                    <div data-tip="Blocks mined on this day (00:00-23:59 UTC)" tabindex="0">
                        <p class="text-[10px] sm:text-[11px] text-white/50 uppercase tracking-wider">"Blocks"</p>
                        <p class="text-white font-mono text-xs sm:text-sm">{format_number(year.block_count)}</p>
                    </div>
                    <div data-tip="Total transactions this day (includes 1 coinbase per block, early blocks with only coinbase still show a count)" tabindex="0">
                        <p class="text-[10px] sm:text-[11px] text-white/50 uppercase tracking-wider">"Txs"</p>
                        <p class="text-white font-mono text-xs sm:text-sm">{format_compact(year.total_tx)}</p>
                    </div>
                    <div data-tip="Total miner fees paid this day" tabindex="0">
                        <p class="text-[10px] sm:text-[11px] text-white/50 uppercase tracking-wider">"Fees"</p>
                        <p class="font-mono text-xs sm:text-sm" style=format!("color: {color}")>
                            {format!("\u{20bf}{:.4}", fees_btc)}
                        </p>
                    </div>
                    <div data-tip="Daily average BTC/USD price (blockchain.info)" tabindex="0">
                        <p class="text-[10px] sm:text-[11px] text-white/50 uppercase tracking-wider">"Price"</p>
                        <p class="text-white font-mono text-xs sm:text-sm">{price_str}</p>
                        {(!mcap_str.is_empty()).then(|| view! {
                            <p class="text-[10px] text-white/50">{mcap_str.clone()}</p>
                        })}
                    </div>
                    <div class="hidden sm:block" data-tip="Total BTC mined as of this date" tabindex="0">
                        <p class="text-[11px] text-white/50 uppercase tracking-wider">"Supply"</p>
                        <p class="text-white font-mono">{supply_str}</p>
                    </div>
                    <div class="hidden sm:block" data-tip="Average block weight as % of 4 MWU limit" tabindex="0">
                        <p class="text-[11px] text-white/50 uppercase tracking-wider">"Block Fullness"</p>
                        <p class="text-xs font-mono tracking-tighter" style=format!("color: {color}")>
                            {fullness_bar(year.avg_weight_util)}
                            {format!(" {:.0}%", year.avg_weight_util)}
                        </p>
                    </div>
                </div>

                // Extra metrics row — hidden on mobile
                <div class="hidden sm:flex flex-wrap gap-x-4 gap-y-1 mt-2 pt-2 border-t border-white/5 text-xs text-white/50">
                    <span data-tip="Block reward per block in this era (halves every 210,000 blocks)" tabindex="0">{
                        let era = year.last_block / 210_000;
                        let subsidy = 50.0_f64 / 2.0_f64.powi(era as i32);
                        format!("Subsidy: \u{20bf}{}", if subsidy >= 1.0 { format!("{:.0}", subsidy) } else { format!("{:.4}", subsidy) })
                    }</span>
                    {(year.segwit_pct > 0.0).then(|| view! {
                        <span data-tip="% of non-coinbase transactions using SegWit" tabindex="0">{format!("SegWit: {:.0}%", year.segwit_pct)}</span>
                    })}
                    {(year.taproot_outputs > 0).then(|| view! {
                        <span data-tip="P2TR outputs created this day" tabindex="0">{format!("Taproot: {}", format_compact(year.taproot_outputs))}</span>
                    })}
                    {(year.total_inscriptions > 0).then(|| view! {
                        <span data-tip="Ordinals inscriptions embedded in witness data" tabindex="0">{format!("Inscriptions: {}", format_compact(year.total_inscriptions))}</span>
                    })}
                    {(year.total_runes > 0).then(|| view! {
                        <span data-tip="Runes protocol OP_RETURN outputs on this day" tabindex="0">{format!("Runes: {}", format_compact(year.total_runes))}</span>
                    })}
                </div>
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
        async move { fetch_on_this_day(m, d).await.map_err(|e| e.to_string()) }
    });

    let display_date = Signal::derive(move || {
        let (m, d) = month_day.get();
        let date = chrono::NaiveDate::from_ymd_opt(2024, m, d); // 2024 is leap year for Feb 29
        date.map(|dt| dt.format("%B %-d").to_string())
            .unwrap_or_else(|| format!("{m}/{d}"))
    });

    let notable_select_ref: NodeRef<leptos::html::Select> = NodeRef::new();

    // Reset the Notable Dates dropdown to placeholder
    let reset_notable = move || {
        #[cfg(feature = "hydrate")]
        if let Some(el) = notable_select_ref.get() {
            el.set_value("");
        }
    };

    let nav_prev = move |_| {
        let (m, d) = month_day.get_untracked();
        let date = chrono::NaiveDate::from_ymd_opt(2024, m, d)
            .and_then(|dt| dt.pred_opt());
        if let Some(prev) = date {
            let new = format!("{:02}-{:02}", prev.month(), prev.day());
            set_selected_date.set(new.clone());
            reset_notable();
            #[cfg(feature = "hydrate")]
            {
                let window = leptos::prelude::window();
                let pathname = window.location().pathname().unwrap_or_default();
                let url = format!("{pathname}?date={new}");
                let _ =
                    window.history().expect("history").replace_state_with_url(
                        &wasm_bindgen::JsValue::NULL,
                        "",
                        Some(&url),
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
            reset_notable();
            #[cfg(feature = "hydrate")]
            {
                let window = leptos::prelude::window();
                let pathname = window.location().pathname().unwrap_or_default();
                let url = format!("{pathname}?date={new}");
                let _ =
                    window.history().expect("history").replace_state_with_url(
                        &wasm_bindgen::JsValue::NULL,
                        "",
                        Some(&url),
                    );
            }
        }
    };

    let nav_today = move |_| {
        let now = chrono::Utc::now();
        let new = format!("{:02}-{:02}", now.month(), now.day());
        set_selected_date.set(new.clone());
        reset_notable();
        #[cfg(feature = "hydrate")]
        {
            let window = leptos::prelude::window();
            let pathname = window.location().pathname().unwrap_or_default();
            let url = format!("{pathname}?date={new}");
            let _ = window.history().expect("history").replace_state_with_url(
                &wasm_bindgen::JsValue::NULL,
                "",
                Some(&url),
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
                <p class="text-[11px] sm:text-xs text-white/50 max-w-lg mx-auto px-4 text-center drop-shadow">"What happened on this date across every year of Bitcoin \u{b7} All times UTC"</p>
            </div>
        </div>

        // Date navigation
        <div class="flex flex-col items-center gap-2 mb-8">
            // Row 1: arrows + date
            <div class="flex items-center gap-3">
                <button
                    class="text-white/50 hover:text-white/80 cursor-pointer p-2 rounded-lg hover:bg-white/5 transition-colors"
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
                        let d = data.get().and_then(|r| r.ok());
                        d.map(|otd| format!("{} years of data", otd.years.len())).unwrap_or_default()
                    }}</p>
                </div>
                <button
                    class="text-white/50 hover:text-white/80 cursor-pointer p-2 rounded-lg hover:bg-white/5 transition-colors"
                    on:click=nav_next
                    title="Next day"
                >
                    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7"/>
                    </svg>
                </button>
            </div>
            // Row 2: today/picker + notable dates
            <div class="flex flex-col sm:flex-row items-center gap-2 justify-center">
            {move || {
                let now = chrono::Utc::now();
                let today = format!("{:02}-{:02}", now.month(), now.day());
                let is_today = selected_date.get() == today;
                let year = now.year();
                if is_today {
                    // Already on today — show date picker to jump to any day
                    view! {
                        <input
                            type="date"
                            class="bg-[#0a1a2e] text-white/80 text-xs border border-white/10 rounded-lg px-3 py-1.5 cursor-pointer focus:outline-none focus:border-[#f7931a]/40 transition-colors"
                            style="color-scheme: dark;"
                            min=format!("{year}-01-01")
                            max=format!("{year}-12-31")
                            on:change=move |ev| {
                                let val = event_target_value(&ev);
                                // Extract MM-DD from YYYY-MM-DD
                                if val.len() >= 10 {
                                    let md = val[5..10].to_string();
                                    set_selected_date.set(md.clone());
                                    reset_notable();
                                    #[cfg(feature = "hydrate")]
                                    {
                                        let window = leptos::prelude::window();
                                        let pathname = window.location().pathname().unwrap_or_default();
                                        let url = format!("{pathname}?date={md}");
                                        let _ = window.history().expect("history").replace_state_with_url(
                                            &wasm_bindgen::JsValue::NULL, "", Some(&url),
                                        );
                                    }
                                }
                            }
                        />
                    }.into_any()
                } else {
                    // Not on today — show Today button
                    view! {
                        <button
                            class="text-xs text-white/50 hover:text-white/70 px-3 py-1.5 rounded-lg border border-white/10 hover:border-white/20 cursor-pointer transition-colors"
                            on:click=nav_today
                        >
                            "Today"
                        </button>
                    }.into_any()
                }
            }}
            <div class="relative inline-block">
                <select
                    node_ref=notable_select_ref
                    aria-label="Notable dates"
                    class="appearance-none bg-[#0a1a2e] text-white/60 text-xs border border-white/10 rounded-lg pl-3 pr-7 py-1.5 cursor-pointer focus:outline-none focus:border-[#f7931a]/40 transition-colors"
                    on:change=move |ev| {
                        use wasm_bindgen::JsCast;
                        if let Some(t) = ev.target() {
                            if let Ok(s) = t.dyn_into::<leptos::web_sys::HtmlSelectElement>() {
                                let val = s.value();
                                if !val.is_empty() {
                                    // Format: "MM-DD:YYYY" — date + target year for scrolling
                                    let parts: Vec<&str> = val.split(':').collect();
                                    let date = parts[0].to_string();
                                    let scroll_year = parts.get(1).unwrap_or(&"").to_string();
                                    set_selected_date.set(date.clone());
                                    #[cfg(feature = "hydrate")]
                                    {
                                        let window = leptos::prelude::window();
                                        let pathname = window.location().pathname().unwrap_or_default();
                                        let url = format!("{pathname}?date={date}");
                                        let _ = window.history().expect("history").replace_state_with_url(
                                            &wasm_bindgen::JsValue::NULL, "", Some(&url),
                                        );
                                        // Scroll to the event's year after data loads
                                        if !scroll_year.is_empty() {
                                            let target_id = format!("year-{scroll_year}");
                                            leptos::prelude::set_timeout(move || {
                                                if let Some(el) = leptos::prelude::document().get_element_by_id(&target_id) {
                                                    let rect = el.get_bounding_client_rect();
                                                    let offset = leptos::prelude::window().scroll_y().unwrap_or(0.0) + rect.top() - 80.0;
                                                    let _ = leptos::prelude::window().scroll_to_with_x_and_y(0.0, offset);
                                                }
                                            }, std::time::Duration::from_millis(500));
                                        }
                                    }
                                    // Keep selection visible (don't reset to placeholder)
                                }
                            }
                        }
                    }
                >
                    <option value="" disabled selected>"Notable Dates"</option>
                    <option value="01-03:2009">"Jan 3 \u{2013} Genesis Block (2009)"</option>
                    <option value="01-12:2009">"Jan 12 \u{2013} First Transaction (2009)"</option>
                    <option value="05-22:2010">"May 22 \u{2013} Pizza Day (2010)"</option>
                    <option value="12-12:2010">"Dec 12 \u{2013} Satoshi\u{2019}s Last Post (2010)"</option>
                    <option value="02-09:2011">"Feb 9 \u{2013} BTC Reaches $1 (2011)"</option>
                    <option value="06-19:2011">"Jun 19 \u{2013} Mt. Gox Hack (2011)"</option>
                    <option value="11-28:2012">"Nov 28 \u{2013} First Halving (2012)"</option>
                    <option value="03-28:2013">"Mar 28 \u{2013} $1B Market Cap (2013)"</option>
                    <option value="07-09:2016">"Jul 9 \u{2013} Second Halving (2016)"</option>
                    <option value="08-01:2017">"Aug 1 \u{2013} BCH Fork (2017)"</option>
                    <option value="08-24:2017">"Aug 24 \u{2013} SegWit Activates (2017)"</option>
                    <option value="11-08:2017">"Nov 8 \u{2013} SegWit2x Cancelled (2017)"</option>
                    <option value="12-17:2017">"Dec 17 \u{2013} BTC $20K (2017)"</option>
                    <option value="03-12:2020">"Mar 12 \u{2013} Black Thursday (2020)"</option>
                    <option value="05-11:2020">"May 11 \u{2013} Third Halving (2020)"</option>
                    <option value="05-19:2021">"May 19 \u{2013} China Mining Ban (2021)"</option>
                    <option value="11-10:2021">"Nov 10 \u{2013} BTC ATH $69K (2021)"</option>
                    <option value="11-14:2021">"Nov 14 \u{2013} Taproot Activates (2021)"</option>
                    <option value="11-11:2022">"Nov 11 \u{2013} FTX Bankruptcy (2022)"</option>
                    <option value="01-10:2024">"Jan 10 \u{2013} Spot ETFs Approved (2024)"</option>
                    <option value="04-20:2024">"Apr 20 \u{2013} Fourth Halving + Runes (2024)"</option>
                    <option value="12-05:2024">"Dec 5 \u{2013} BTC $100K (2024)"</option>
                    <option value="10-06:2025">"Oct 6 \u{2013} BTC ATH $126K (2025)"</option>
                </select>
                <svg class="absolute right-2 top-1/2 -translate-y-1/2 pointer-events-none w-3 h-3 text-white/40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                </svg>
            </div>
            </div>
        </div>

        // Year cards
        <div class="space-y-3 min-h-[60vh]">
            {move || match data.get() {
                Some(Ok(otd)) => {
                    // Check if date is after genesis (Jan 3)
                    let is_after_genesis = otd.month > 1
                        || (otd.month == 1 && otd.day >= 3);
                    let has_2009 = otd.years.iter().any(|y| y.year == 2009);

                    if otd.years.is_empty() && !is_after_genesis {
                        view! {
                            <div class="text-center text-white/30 py-20">
                                <p class="text-lg">"No blocks mined on this date"</p>
                                <p class="text-sm mt-1">"Bitcoin didn\u{2019}t exist yet \u{2014} the genesis block was mined on January 3, 2009."</p>
                            </div>
                        }.into_any()
                    } else {
                        let mut cards: Vec<leptos::tachys::view::any_view::AnyView> = otd.years.into_iter().map(|year| {
                            view! { <YearCard year=year/> }.into_any()
                        }).collect();

                        // Append a 2009 placeholder if date is after genesis but no 2009 data
                        if is_after_genesis && !has_2009 {
                            cards.push(view! {
                                <div id="year-2009" class="bg-[#0d2137] border border-white/10 rounded-xl overflow-hidden" style="border-left: 4px solid #3b82f6">
                                    <div class="p-4 sm:p-5">
                                        <div class="flex items-center gap-3 mb-2">
                                            <span class="text-2xl sm:text-3xl font-title text-white font-bold">"2009"</span>
                                            <span class="text-xs text-white/50 bg-white/5 rounded-full px-2.5 py-0.5">"Genesis year"</span>
                                        </div>
                                        <p class="text-sm text-white/50">"No blocks were mined on this date."</p>
                                        <p class="text-xs text-white/40 mt-2 leading-relaxed">"In Bitcoin\u{2019}s earliest days, Satoshi was often the only miner. After the genesis block on January 3rd, the next block wasn\u{2019}t mined until January 9th, a gap of over 5 days. Some believe this was intentional, giving people time to see the announcement on the cryptography mailing list and begin mining. Others think Satoshi was still testing the software. Either way, entire days with zero blocks were common in 2009."</p>
                                    </div>
                                </div>
                            }.into_any());
                        }
                        view! { <div class="space-y-3">{cards}</div> }.into_any()
                    }
                }
                Some(Err(_)) => view! {
                    <DataLoadError on_retry=Callback::new(move |_| data.refetch())/>
                }.into_any(),
                None => view! {
                    <div class="flex justify-center py-20">
                        <div class="animate-pulse flex items-center gap-2 text-white/30">
                            <div class="w-3 h-3 rounded-full bg-[#f7931a]/30 animate-ping"></div>
                            "Loading Bitcoin history..."
                        </div>
                    </div>
                }.into_any(),
            }}
        </div>
    }
}
