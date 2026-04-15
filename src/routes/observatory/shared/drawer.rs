//! Chart sidebar drawer for the Observatory.
//!
//! Collapsible sidebar listing all observatory charts organized by page and section.
//! Clicking a chart name scrolls to it (same page) or navigates (cross-page).

use leptos::prelude::*;
use leptos_router::hooks::use_location;

/// Entry in the chart drawer: a chart name and its HTML element ID for scrolling.
struct DrawerChart {
    label: &'static str,
    card_id: &'static str,
}

/// A section (or subsection) of charts within the drawer.
struct DrawerSection {
    label: &'static str,
    charts: Vec<DrawerChart>,
}

/// A top-level page grouping in the drawer.
struct DrawerPage {
    label: &'static str,
    path_prefix: &'static str,
    sections: Vec<DrawerSection>,
}

fn drawer_pages() -> Vec<DrawerPage> {
    vec![
        DrawerPage {
            label: "Network",
            path_prefix: "/observatory/charts/network",
            sections: vec![
                DrawerSection {
                    label: "Blocks",
                    charts: vec![
                        DrawerChart { label: "Transaction Count", card_id: "card-chart-txcount" },
                        DrawerChart { label: "TPS", card_id: "card-chart-tps" },
                        DrawerChart { label: "Block Size", card_id: "card-chart-size" },
                        DrawerChart { label: "Weight Utilization", card_id: "card-chart-weight-util" },
                        DrawerChart { label: "Block Interval", card_id: "card-chart-interval" },
                        DrawerChart { label: "Avg Transaction Size", card_id: "card-chart-avg-tx-size" },
                        DrawerChart { label: "Chain Size Growth", card_id: "card-chart-chain-size" },
                        DrawerChart { label: "Weekday Activity", card_id: "card-chart-weekday" },
                        DrawerChart { label: "Block Fullness Distribution", card_id: "card-chart-fullness-dist" },
                        DrawerChart { label: "Block Time Distribution", card_id: "card-chart-time-dist" },
                        DrawerChart { label: "Rapid Consecutive Blocks", card_id: "card-chart-propagation" },
                    ],
                },
                DrawerSection {
                    label: "Adoption",
                    charts: vec![
                        DrawerChart { label: "SegWit Adoption", card_id: "card-chart-segwit" },
                        DrawerChart { label: "Taproot Outputs", card_id: "card-chart-taproot" },
                        DrawerChart { label: "Address Type Evolution", card_id: "card-chart-address-types" },
                        DrawerChart { label: "Address Type Share", card_id: "card-chart-address-types-pct" },
                        DrawerChart { label: "Output Type Breakdown", card_id: "card-chart-witness-tx-pct" },
                        DrawerChart { label: "Witness Version Comparison", card_id: "card-chart-witness-versions" },
                        DrawerChart { label: "Witness Version Share", card_id: "card-chart-witness-pct" },
                        DrawerChart { label: "Taproot Spend Types", card_id: "card-chart-taproot-spend-types" },
                        DrawerChart { label: "Witness Data Share", card_id: "card-chart-witness-share" },
                        DrawerChart { label: "Cumulative Adoption", card_id: "card-chart-cumulative-adoption" },
                        DrawerChart { label: "Adoption Velocity", card_id: "card-chart-multi-velocity" },
                        DrawerChart { label: "P2PKH Sunset Tracker", card_id: "card-chart-p2pkh-sunset" },
                    ],
                },
                DrawerSection {
                    label: "Transactions",
                    charts: vec![
                        DrawerChart { label: "RBF Adoption", card_id: "card-chart-rbf" },
                        DrawerChart { label: "UTXO Flow", card_id: "card-chart-utxo-flow" },
                        DrawerChart { label: "Batching Efficiency", card_id: "card-chart-batching" },
                        DrawerChart { label: "Largest Transaction", card_id: "card-chart-largest-tx" },
                        DrawerChart { label: "Transaction Density", card_id: "card-chart-tx-density" },
                        DrawerChart { label: "UTXO Growth Rate", card_id: "card-chart-utxo-growth" },
                        DrawerChart { label: "Transaction Type Evolution", card_id: "card-chart-tx-type-evolution" },
                    ],
                },
            ],
        },
        DrawerPage {
            label: "Fees",
            path_prefix: "/observatory/charts/fees",
            sections: vec![
                DrawerSection {
                    label: "",
                    charts: vec![
                        DrawerChart { label: "Total Fees per Block", card_id: "card-chart-fees" },
                        DrawerChart { label: "Median Fee Rate", card_id: "card-chart-median-rate" },
                        DrawerChart { label: "Fee Rate Bands", card_id: "card-chart-fee-heatmap" },
                        DrawerChart { label: "Avg Fee per Transaction", card_id: "card-chart-avg-fee-tx" },
                        DrawerChart { label: "Subsidy vs Fees", card_id: "card-chart-subsidy-fees" },
                        DrawerChart { label: "Fee Revenue Share", card_id: "card-chart-fee-revenue-share" },
                        DrawerChart { label: "BTC Transferred Volume", card_id: "card-chart-btc-volume" },
                        DrawerChart { label: "Input vs Output Value", card_id: "card-chart-value-flow" },
                        DrawerChart { label: "Halving Era Comparison", card_id: "card-chart-halving-era" },
                        DrawerChart { label: "Fee Pressure vs Block Space", card_id: "card-chart-fee-pressure" },
                        DrawerChart { label: "Fee Spike Detector", card_id: "card-chart-fee-spikes" },
                        DrawerChart { label: "Max Transaction Fee", card_id: "card-chart-max-tx-fee" },
                        DrawerChart { label: "Protocol Fee Revenue", card_id: "card-chart-protocol-fees" },
                    ],
                },
            ],
        },
        DrawerPage {
            label: "Mining",
            path_prefix: "/observatory/charts/mining",
            sections: vec![
                DrawerSection {
                    label: "Difficulty",
                    charts: vec![
                        DrawerChart { label: "Difficulty", card_id: "card-chart-difficulty" },
                        DrawerChart { label: "Difficulty Ribbon", card_id: "card-chart-diff-ribbon" },
                    ],
                },
                DrawerSection {
                    label: "Mining Pools",
                    charts: vec![
                        DrawerChart { label: "Mining Pool Share", card_id: "card-chart-miner-dominance" },
                        DrawerChart { label: "Mining Diversity Index", card_id: "card-chart-diversity" },
                        DrawerChart { label: "Empty Blocks", card_id: "card-chart-empty-blocks" },
                        DrawerChart { label: "Empty Blocks by Pool", card_id: "card-chart-empty-by-pool" },
                    ],
                },
            ],
        },
        DrawerPage {
            label: "Embedded Data",
            path_prefix: "/observatory/charts/embedded",
            sections: vec![
                DrawerSection {
                    label: "Overview",
                    charts: vec![
                        DrawerChart { label: "All Embedded Share", card_id: "card-chart-all-embedded-share" },
                        DrawerChart { label: "All Embedded Count", card_id: "card-chart-unified-count" },
                        DrawerChart { label: "All Embedded Volume", card_id: "card-chart-unified-volume" },
                    ],
                },
                DrawerSection {
                    label: "OP_RETURN",
                    charts: vec![
                        DrawerChart { label: "OP_RETURN Count", card_id: "card-chart-opreturn-count" },
                        DrawerChart { label: "OP_RETURN Volume", card_id: "card-chart-opreturn-bytes" },
                        DrawerChart { label: "OP_RETURN Protocol Share", card_id: "card-chart-runes-pct" },
                        DrawerChart { label: "OP_RETURN Block Share", card_id: "card-chart-op-block-share" },
                    ],
                },
                DrawerSection {
                    label: "Ordinals & Witness Data",
                    charts: vec![
                        DrawerChart { label: "Inscription Count", card_id: "card-chart-inscriptions" },
                        DrawerChart { label: "Inscription Block Share", card_id: "card-chart-inscription-share" },
                        DrawerChart { label: "Payload vs Envelope", card_id: "card-chart-inscription-envelope" },
                        DrawerChart { label: "Inscription Fee Share", card_id: "card-chart-inscription-fee-share" },
                        DrawerChart { label: "Protocol Fee Competition", card_id: "card-chart-protocol-fee-competition" },
                    ],
                },
            ],
        },
    ]
}

/// Collapsible sidebar drawer listing all observatory charts organized by page
/// and section. Clicking a chart name scrolls to it. The current page is highlighted.
#[allow(unused_variables)]
#[component]
pub fn ChartDrawer() -> impl IntoView {
    let (open, set_open) = signal(false);
    let location = use_location();

    let pages = drawer_pages();

    view! {
        // Toggle tab fixed on the left edge
        <button
            style="z-index: 10001"
            class="fixed left-0 top-1/3 bg-[#0d2137] border border-l-0 border-[#f7931a]/30 rounded-r-xl px-2 sm:px-2.5 py-5 sm:py-6 cursor-pointer hover:bg-[#143050] hover:border-[#f7931a]/60 hover:scale-105 transition-all group shadow-lg shadow-black/30"
            on:click=move |_| set_open.set(true)
            title="Chart index"
        >
            <svg class="w-5 h-5 sm:w-6 sm:h-6 text-[#f7931a]/70 group-hover:text-[#f7931a] transition-colors" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"/>
            </svg>
        </button>

        // Backdrop
        <Show when=move || open.get()>
            <div
                style="z-index: 10002"
                class="fixed inset-0 bg-black/50 transition-opacity"
                on:click=move |_| set_open.set(false)
            />
        </Show>

        // Drawer panel
        <div
            style=move || format!(
                "z-index: 10003; transform: translateX({}); transition: transform 0.25s ease-in-out;",
                if open.get() { "0" } else { "-100%" }
            )
            class="fixed top-[48px] left-0 bottom-0 w-72 bg-[#0d2137] border-r border-white/10 overflow-y-auto"
        >
            // Header
            <div class="flex items-center justify-between px-4 py-3 border-b border-white/10">
                <span class="text-sm font-semibold text-white/80">"Chart Index"</span>
                <button
                    class="text-white/30 hover:text-white/60 cursor-pointer"
                    on:click=move |_| set_open.set(false)
                >
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
                    </svg>
                </button>
            </div>

            // Content
            <nav class="p-3">
                {pages.into_iter().map(|page| {
                    let path_prefix = page.path_prefix;
                    view! {
                        <div class="mb-3">
                            // Page heading
                            <div class=move || {
                                let current = location.pathname.get();
                                if current.starts_with(path_prefix) {
                                    "text-sm font-bold text-[#f7931a] uppercase tracking-wider px-2 py-1.5 border-b border-[#f7931a]/20 mb-1"
                                } else {
                                    "text-sm font-bold text-white/50 uppercase tracking-wider px-2 py-1.5 border-b border-white/5 mb-1"
                                }
                            }>
                                {page.label}
                            </div>
                            // Sections
                            {page.sections.into_iter().map(|section| {
                                let has_label = !section.label.is_empty();
                                view! {
                                    <div class="ml-1">
                                        {if has_label {
                                            Some(view! {
                                                <div class="text-[11px] text-white/45 font-semibold uppercase tracking-wider px-2 pt-2 pb-1">
                                                    {section.label}
                                                </div>
                                            })
                                        } else {
                                            None
                                        }}
                                        <ul class="space-y-0">
                                            {section.charts.into_iter().map(|chart| {
                                                let card_id = chart.card_id;
                                                view! {
                                                    <li>
                                                        <button
                                                            class="w-full text-left text-[12px] text-white/60 hover:text-white hover:bg-white/5 rounded-md px-3 py-1 cursor-pointer transition-colors"
                                                            on:click=move |_| {
                                                                set_open.set(false);
                                                                #[cfg(feature = "hydrate")]
                                                                {
                                                                    // All charts are in the DOM (flat layout), so direct
                                                                    // scroll works for same-page. Cross-page: navigate via href.
                                                                    if let Some(el) = leptos::prelude::document().get_element_by_id(card_id) {
                                                                        el.scroll_into_view();
                                                                    } else {
                                                                        let url = format!("{}#{}", path_prefix, card_id);
                                                                        let _ = leptos::prelude::window().location().set_href(&url);
                                                                    }
                                                                }
                                                            }
                                                        >
                                                            {chart.label}
                                                        </button>
                                                    </li>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </ul>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </nav>
        </div>
    }
}
