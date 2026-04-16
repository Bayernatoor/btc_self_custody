//! Bitcoin Hall of Fame: curated museum of remarkable blocks and transactions.
//!
//! A static collection of notable Bitcoin events defined in `hall_of_fame_data.rs`.
//! Each entry has a date, title, description, category, optional block height,
//! optional txid, and optional source link.
//!
//! Users can filter by category: Highlights (curated subset), All, Milestones,
//! Records, Attacks, Protocol, or Oddities. The active filter is synced to the
//! `?category=` query param. Entries are sorted chronologically. Each card links
//! to the On This Day page and can open the block detail modal.

use chrono::Datelike;
use leptos::prelude::*;
#[cfg(feature = "hydrate")]
use leptos::web_sys;
use leptos_meta::*;
use leptos_router::hooks::use_query_map;

use super::components::show_block_detail;
use super::hall_of_fame_data::HALL_OF_FAME;
use crate::stats::types::{HallOfFameEntry, HofCategory};

// ---------------------------------------------------------------------------
// Filter state
// ---------------------------------------------------------------------------

/// Which filter is active. `None` means "Highlights".
#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    Highlights,
    All,
    Category(HofCategory),
}

impl Filter {
    fn label(self) -> &'static str {
        match self {
            Self::Highlights => "Highlights",
            Self::All => "All",
            Self::Category(c) => c.label(),
        }
    }

    #[allow(dead_code)] // used in #[cfg(feature = "hydrate")] URL sync
    fn slug(self) -> &'static str {
        match self {
            Self::Highlights => "highlights",
            Self::All => "all",
            Self::Category(HofCategory::Milestones) => "milestones",
            Self::Category(HofCategory::Records) => "records",
            Self::Category(HofCategory::Attacks) => "attacks",
            Self::Category(HofCategory::Protocol) => "protocol",
            Self::Category(HofCategory::Oddities) => "oddities",
        }
    }

    fn from_slug(s: &str) -> Self {
        match s {
            "all" => Self::All,
            "milestones" => Self::Category(HofCategory::Milestones),
            "records" => Self::Category(HofCategory::Records),
            "attacks" => Self::Category(HofCategory::Attacks),
            "protocol" => Self::Category(HofCategory::Protocol),
            "oddities" => Self::Category(HofCategory::Oddities),
            _ => Self::Highlights,
        }
    }

    fn matches(self, entry: &HallOfFameEntry) -> bool {
        match self {
            Self::Highlights => entry.highlight,
            Self::All => true,
            Self::Category(c) => entry.category == c,
        }
    }
}

const FILTERS: &[Filter] = &[
    Filter::Highlights,
    Filter::All,
    Filter::Category(HofCategory::Milestones),
    Filter::Category(HofCategory::Records),
    Filter::Category(HofCategory::Attacks),
    Filter::Category(HofCategory::Protocol),
    Filter::Category(HofCategory::Oddities),
];

// ---------------------------------------------------------------------------
// Page component
// ---------------------------------------------------------------------------

/// Hall of Fame page. Reads initial filter from `?category=` query param, renders
/// filter pill bar, entry count, and a responsive card grid sorted chronologically.
#[component]
pub fn HallOfFamePage() -> impl IntoView {
    let query = use_query_map();

    let initial_filter = query
        .read_untracked()
        .get("category")
        .map(|s| Filter::from_slug(&s))
        .unwrap_or(Filter::Highlights);

    let (active_filter, set_active_filter) = signal(initial_filter);

    // Filtered + sorted entries
    let filtered = Signal::derive(move || {
        let f = active_filter.get();
        let mut entries: Vec<&HallOfFameEntry> =
            HALL_OF_FAME.iter().filter(|e| f.matches(e)).collect();
        entries.sort_by_key(|e| e.date);
        entries
    });

    let total = HALL_OF_FAME.len();

    // URL sync: update ?category= on filter change (client only)
    #[cfg(feature = "hydrate")]
    {
        Effect::new(move || {
            let f = active_filter.get();
            let slug = f.slug();
            if let Some(window) = web_sys::window() {
                if let Ok(history) = window.history() {
                    let search = if slug == "highlights" {
                        String::new()
                    } else {
                        format!("?category={slug}")
                    };
                    let path = format!("/observatory/hall-of-fame{search}");
                    let _ = history.replace_state_with_url(
                        &web_sys::wasm_bindgen::JsValue::NULL,
                        "",
                        Some(&path),
                    );
                }
            }
        });
    }

    // Scroll to hash fragment on initial mount only
    #[cfg(feature = "hydrate")]
    leptos::prelude::set_timeout(
        move || {
            if let Some(window) = web_sys::window() {
                if let Ok(hash) = window.location().hash() {
                    let id = hash.trim_start_matches('#');
                    if !id.is_empty() {
                        if let Some(doc) = window.document() {
                            if let Some(el) = doc.get_element_by_id(id) {
                                el.scroll_into_view();
                            }
                        }
                    }
                }
            }
        },
        std::time::Duration::from_millis(300),
    );

    view! {
        <Title text="The Archives: Remarkable Blocks & Transactions | WE HODL BTC"/>
        <Meta name="description" content="The Archives: a curated collection of Bitcoin's most remarkable blocks and transactions. Genesis block, Pizza Day, SegWit, Taproot, the 184 billion BTC bug, record-breaking blocks, and more."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/hall-of-fame"/>

        // Hero
        <div class="relative rounded-2xl overflow-hidden mb-5">
            <img
                src="/img/observatory_hero.png"
                alt="The Archives"
                class="w-full h-[100px] sm:h-[120px] lg:h-[140px] object-cover object-center"
            />
            <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/60 to-[#123c64]/30"></div>
            <div class="absolute inset-0 flex flex-col items-center justify-end pb-3 sm:pb-4">
                <h1 class="text-lg sm:text-xl lg:text-2xl font-title text-white mb-0.5 drop-shadow-lg">"The Archives"</h1>
                <p class="text-[11px] sm:text-xs text-white/50 max-w-lg mx-auto px-4 text-center drop-shadow">"Remarkable blocks, transactions, and events that shaped Bitcoin"</p>
            </div>
        </div>

        // Filter bar
        <div class="flex items-center gap-2 overflow-x-auto pb-2 mb-2 scrollbar-hide">
            {FILTERS.iter().map(|f| {
                let f = *f;
                let is_active = Signal::derive(move || active_filter.get() == f);
                view! {
                    <button
                        class=move || if is_active.get() {
                            "px-3 py-1.5 rounded-full text-xs font-semibold bg-[#f7931a] text-black whitespace-nowrap transition-all flex-shrink-0"
                        } else {
                            "px-3 py-1.5 rounded-full text-xs font-medium bg-white/5 text-white/50 hover:bg-white/10 hover:text-white/70 whitespace-nowrap transition-all flex-shrink-0"
                        }
                        on:click=move |_| set_active_filter.set(f)
                    >
                        {f.label()}
                    </button>
                }
            }).collect::<Vec<_>>()}
        </div>

        // Count
        <p class="text-xs text-white/30 mb-4">
            {move || {
                let count = filtered.get().len();
                format!("Showing {} of {} entries", count, total)
            }}
        </p>

        // Card grid
        <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
            {move || {
                filtered.get().into_iter().map(|entry| {
                    view! { <HofEntryCard entry=entry/> }
                }).collect::<Vec<_>>()
            }}
        </div>
    }
}

// ---------------------------------------------------------------------------
// Entry card
// ---------------------------------------------------------------------------

/// Individual Hall of Fame entry card with category badge, date, title, description,
/// and links (mempool.space TX, block detail modal, On This Day, source reference).
#[component]
fn HofEntryCard(entry: &'static HallOfFameEntry) -> impl IntoView {
    let cat_color = entry.category.color();
    let cat_label = entry.category.label();
    let border_style = format!("border-left: 3px solid {cat_color}");
    let badge_style =
        format!("background: {}20; color: {cat_color}", cat_color);

    // Format date nicely + build On This Day link
    let parsed_date =
        chrono::NaiveDate::parse_from_str(entry.date, "%Y-%m-%d").ok();
    let date_display = parsed_date
        .map(|d| d.format("%b %d, %Y").to_string())
        .unwrap_or_else(|| entry.date.to_string());
    let on_this_day_link = parsed_date.map(|d| {
        format!(
            "/observatory/on-this-day?date={:02}-{:02}&year={}",
            d.month(),
            d.day(),
            d.year()
        )
    });

    let tx_link = entry.txid.map(|t| format!("https://mempool.space/tx/{t}"));
    let tx_short = entry.txid.map(|t| {
        if t.len() > 16 {
            format!("{}...{}", &t[..8], &t[t.len() - 8..])
        } else {
            t.to_string()
        }
    });

    let has_block_in_db = entry.block.is_some();
    let block_height = entry.block.unwrap_or(0);

    view! {
        <div
            id=entry.slug
            class="bg-[#0d2137] border border-white/10 rounded-xl p-4 sm:p-5 hover:border-white/20 transition-all"
            style=border_style
        >
            // Header: badge + date
            <div class="flex items-center justify-between mb-2">
                <span
                    class="text-[10px] font-semibold uppercase tracking-wider px-2 py-0.5 rounded-full"
                    style=badge_style
                >{cat_label}</span>
                {match on_this_day_link {
                    Some(href) => view! {
                        <a
                            href=href
                            class="text-[11px] text-white/40 hover:text-[#f7931a]/70 font-mono transition-colors"
                            title="View this date on On This Day"
                        >{date_display.clone()}</a>
                    }.into_any(),
                    None => view! {
                        <span class="text-[11px] text-white/40 font-mono">{date_display.clone()}</span>
                    }.into_any(),
                }}
            </div>

            // Title
            <h3 class="text-sm sm:text-base text-white font-bold mb-2 leading-snug">{entry.title}</h3>

            // Description
            <p class="text-xs sm:text-sm text-white/60 leading-relaxed mb-3">{entry.description}</p>

            // Links row
            <div class="flex flex-wrap items-center gap-2">
                // TX link
                {tx_link.map(|href| {
                    let short = tx_short.clone().unwrap_or_default();
                    view! {
                        <a
                            href=href
                            target="_blank"
                            rel="noopener"
                            class="inline-flex items-center gap-1 text-[11px] text-[#a78bfa] hover:text-[#c4b5fd] font-mono transition-colors"
                        >
                            "TX "{short}
                            <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"/>
                            </svg>
                        </a>
                    }
                })}

                // View Block button (opens BlockDetailModal with live DB data)
                {if has_block_in_db {
                    Some(view! {
                        <button
                            class="text-[11px] text-[#f7931a]/70 hover:text-[#f7931a] font-medium transition-colors cursor-pointer"
                            on:click=move |_| show_block_detail(block_height)
                        >"View Block Data"</button>
                    })
                } else {
                    None
                }}

                // Source / reference link
                {entry.source.map(|(label, url)| {
                    view! {
                        <a
                            href=url
                            target="_blank"
                            rel="noopener"
                            class="inline-flex items-center gap-1 text-[11px] text-[#34d399] hover:text-[#6ee7b7] transition-colors"
                        >
                            {label}
                            <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"/>
                            </svg>
                        </a>
                    }
                })}
            </div>
        </div>
    }
}
