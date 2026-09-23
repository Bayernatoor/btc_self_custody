//! Chart sidebar drawer for the Observatory.
//!
//! Collapsible sidebar listing all observatory charts organized by page and section.
//! Clicking a chart name scrolls to it (same page) or navigates (cross-page).
//!
//! The overlay is portaled to `<body>` and spans the full viewport height. Both
//! are load-bearing; see the comments on `ChartDrawer` for why.

use leptos::portal::Portal;
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

/// How many charts the index lists, for the header.
///
/// Counted from the drawer's own entries rather than from the registry, so
/// the number describes what is actually on screen. A test holds the two
/// lists together, so they agree; if they ever stop, this should say what the
/// reader can see.
fn chart_count() -> usize {
    drawer_pages()
        .iter()
        .flat_map(|p| p.sections.iter())
        .map(|s| s.charts.len())
        .sum()
}

/// The chart's registry slug, from the card element id the drawer stores.
///
/// Drawer entries hold `card-chart-<slug>`, the id of the card wrapper on a
/// multi-chart page. The single-chart route keys off the bare slug, and the
/// test below checks the same mapping, so the derivation lives here rather
/// than being written out at each use.
///
/// Ungated so the tests can reach it: the only non-test caller is inside the
/// hydrate-only click handler, which does not exist in the `ssr` build where
/// tests run.
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
fn slug_of(card_id: &str) -> &str {
    card_id
        .strip_prefix("card-chart-")
        .or_else(|| card_id.strip_prefix("chart-"))
        .unwrap_or(card_id)
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
                        DrawerChart {
                            label: "Transaction Count",
                            card_id: "card-chart-txcount",
                        },
                        DrawerChart {
                            label: "TPS",
                            card_id: "card-chart-tps",
                        },
                        DrawerChart {
                            label: "Block Size",
                            card_id: "card-chart-size",
                        },
                        DrawerChart {
                            label: "Weight Utilization",
                            card_id: "card-chart-weight-util",
                        },
                        DrawerChart {
                            label: "Block Interval",
                            card_id: "card-chart-interval",
                        },
                        DrawerChart {
                            label: "Avg Transaction Size",
                            card_id: "card-chart-avg-tx-size",
                        },
                        DrawerChart {
                            label: "Chain Size Growth",
                            card_id: "card-chart-chain-size",
                        },
                        DrawerChart {
                            label: "Weekday Activity",
                            card_id: "card-chart-weekday",
                        },
                        DrawerChart {
                            label: "Block Fullness Distribution",
                            card_id: "card-chart-fullness-dist",
                        },
                        DrawerChart {
                            label: "Block Time Distribution",
                            card_id: "card-chart-time-dist",
                        },
                        DrawerChart {
                            label: "Rapid Consecutive Blocks",
                            card_id: "card-chart-propagation",
                        },
                    ],
                },
                DrawerSection {
                    label: "Adoption",
                    charts: vec![
                        DrawerChart {
                            label: "SegWit Adoption",
                            card_id: "card-chart-segwit",
                        },
                        DrawerChart {
                            label: "Taproot Outputs",
                            card_id: "card-chart-taproot",
                        },
                        DrawerChart {
                            label: "Address Type Evolution",
                            card_id: "card-chart-address-types",
                        },
                        DrawerChart {
                            label: "Address Type Share",
                            card_id: "card-chart-address-types-pct",
                        },
                        DrawerChart {
                            label: "Output Type Breakdown",
                            card_id: "card-chart-witness-tx-pct",
                        },
                        DrawerChart {
                            label: "Witness Version Comparison",
                            card_id: "card-chart-witness-versions",
                        },
                        DrawerChart {
                            label: "Witness Version Share",
                            card_id: "card-chart-witness-pct",
                        },
                        DrawerChart {
                            label: "Taproot Spend Types",
                            card_id: "card-chart-taproot-spend-types",
                        },
                        DrawerChart {
                            label: "Witness Data Share",
                            card_id: "card-chart-witness-share",
                        },
                        DrawerChart {
                            label: "Cumulative Adoption",
                            card_id: "card-chart-cumulative-adoption",
                        },
                        DrawerChart {
                            label: "Adoption Velocity",
                            card_id: "card-chart-multi-velocity",
                        },
                        DrawerChart {
                            label: "P2PKH Sunset Tracker",
                            card_id: "card-chart-p2pkh-sunset",
                        },
                    ],
                },
                DrawerSection {
                    label: "Transactions",
                    charts: vec![
                        DrawerChart {
                            label: "Explicit RBF Signaling",
                            card_id: "card-chart-rbf",
                        },
                        DrawerChart {
                            label: "UTXO Flow",
                            card_id: "card-chart-utxo-flow",
                        },
                        DrawerChart {
                            label: "Transaction Batching",
                            card_id: "card-chart-batching",
                        },
                        DrawerChart {
                            label: "Largest Transaction",
                            card_id: "card-chart-largest-tx",
                        },
                        DrawerChart {
                            label: "Transaction Density",
                            card_id: "card-chart-tx-density",
                        },
                        DrawerChart {
                            label: "UTXO Growth Rate",
                            card_id: "card-chart-utxo-growth",
                        },
                        DrawerChart {
                            label: "Transaction Type Evolution",
                            card_id: "card-chart-tx-type-evolution",
                        },
                    ],
                },
            ],
        },
        DrawerPage {
            label: "Fees",
            path_prefix: "/observatory/charts/fees",
            sections: vec![DrawerSection {
                label: "",
                charts: vec![
                    DrawerChart {
                        label: "Total Fees per Block",
                        card_id: "card-chart-fees",
                    },
                    DrawerChart {
                        label: "Median Fee Rate",
                        card_id: "card-chart-median-rate",
                    },
                    DrawerChart {
                        label: "Fee Rate Bands",
                        card_id: "card-chart-fee-heatmap",
                    },
                    DrawerChart {
                        label: "Avg Fee per Transaction",
                        card_id: "card-chart-avg-fee-tx",
                    },
                    DrawerChart {
                        label: "Subsidy vs Fees",
                        card_id: "card-chart-subsidy-fees",
                    },
                    DrawerChart {
                        label: "Fee Revenue Share",
                        card_id: "card-chart-fee-revenue-share",
                    },
                    DrawerChart {
                        label: "BTC Transferred Volume",
                        card_id: "card-chart-btc-volume",
                    },
                    DrawerChart {
                        label: "Halving Era Comparison",
                        card_id: "card-chart-halving-era",
                    },
                    DrawerChart {
                        label: "Fee Pressure vs Block Space",
                        card_id: "card-chart-fee-pressure",
                    },
                    DrawerChart {
                        label: "Fee Spike Detector",
                        card_id: "card-chart-fee-spikes",
                    },
                    DrawerChart {
                        label: "Max Transaction Fee",
                        card_id: "card-chart-max-tx-fee",
                    },
                    DrawerChart {
                        label: "Protocol Fee Revenue",
                        card_id: "card-chart-protocol-fees",
                    },
                ],
            }],
        },
        DrawerPage {
            label: "Mining",
            path_prefix: "/observatory/charts/mining",
            sections: vec![
                DrawerSection {
                    label: "Difficulty",
                    charts: vec![
                        DrawerChart {
                            label: "Difficulty",
                            card_id: "card-chart-difficulty",
                        },
                        DrawerChart {
                            label: "Hash Rate",
                            card_id: "card-chart-hash-rate",
                        },
                        DrawerChart {
                            label: "Difficulty Adjustment",
                            card_id: "card-chart-diff-adjustment",
                        },
                        DrawerChart {
                            label: "Difficulty Ribbon",
                            card_id: "card-chart-diff-ribbon",
                        },
                    ],
                },
                DrawerSection {
                    label: "Mining Pools",
                    charts: vec![
                        DrawerChart {
                            label: "Mining Pool Share",
                            card_id: "card-chart-miner-dominance",
                        },
                        DrawerChart {
                            label: "Mining Diversity Index",
                            card_id: "card-chart-diversity",
                        },
                        DrawerChart {
                            label: "Empty Blocks",
                            card_id: "card-chart-empty-blocks",
                        },
                        DrawerChart {
                            label: "Empty Blocks by Pool",
                            card_id: "card-chart-empty-by-pool",
                        },
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
                        DrawerChart {
                            label: "Detected Embedded, Block Share",
                            card_id: "card-chart-all-embedded-share",
                        },
                        DrawerChart {
                            label: "Detected Embedded, Count",
                            card_id: "card-chart-unified-count",
                        },
                        DrawerChart {
                            label: "Detected Embedded, Volume",
                            card_id: "card-chart-unified-volume",
                        },
                    ],
                },
                DrawerSection {
                    label: "OP_RETURN",
                    charts: vec![
                        DrawerChart {
                            label: "OP_RETURN Count",
                            card_id: "card-chart-opreturn-count",
                        },
                        DrawerChart {
                            label: "OP_RETURN Volume",
                            card_id: "card-chart-opreturn-bytes",
                        },
                        DrawerChart {
                            label: "OP_RETURN Protocol Share",
                            card_id: "card-chart-runes-pct",
                        },
                        DrawerChart {
                            label: "OP_RETURN Block Share",
                            card_id: "card-chart-op-block-share",
                        },
                    ],
                },
                DrawerSection {
                    label: "Ordinals & Witness Data",
                    charts: vec![
                        DrawerChart {
                            label: "Inscription Count",
                            card_id: "card-chart-inscriptions",
                        },
                        DrawerChart {
                            label: "Inscription Block Share",
                            card_id: "card-chart-inscription-share",
                        },
                        DrawerChart {
                            label: "Payload vs Envelope",
                            card_id: "card-chart-inscription-envelope",
                        },
                        DrawerChart {
                            label: "Inscription Fee Share",
                            card_id: "card-chart-inscription-fee-share",
                        },
                        DrawerChart {
                            label: "Protocol Fee Competition",
                            card_id: "card-chart-protocol-fee-competition",
                        },
                    ],
                },
                DrawerSection {
                    label: "Miner Signals",
                    charts: vec![DrawerChart {
                        label: "Coinbase Message Length",
                        card_id: "card-chart-coinbase-msg-length",
                    }],
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

    view! {
        // Toggle tab fixed on the left edge
        // Section-nav drawer trigger. Deliberately quieter than the primary
        // Chart Settings FAB on the right — outline-on-dark so it reads as
        // a secondary aid rather than a primary action. Icon and border both
        // bumped to full orange (from /70 and /30 opacity) so it's still
        // clearly clickable without competing with the solid-fill FAB.
        <button
            style="z-index: 10001"
            // Narrow and tall rather than square. A wide tab reads as a
            // button that happens to be at the edge and takes width from the
            // chart; a thin one that runs a fifth of the viewport reads as a
            // handle, which is what it is, and is a bigger target for being
            // noticed at all despite covering less area.
            // Border weight and colours match the settings button on the
            // opposite edge, so the two floating affordances read as a pair
            // rather than as one control and one decoration.
            // A 5-unit sliver on a phone, the full tab from `sm` up.
            //
            // This is `fixed`, so the page has to reserve whatever width it
            // occupies or it sits on the content. At 28px that reservation
            // cost more than the handle was worth: every card on every
            // observatory page lost it, to avoid a control that is itself too
            // wide for the viewport. A sliver costs 20px, keeps the whole
            // 20vh height as the tap target, and opens the same drawer.
            //
            // The label goes with it: "CHARTS" set vertically needs width
            // this does not have, and a clipped word is worse than none. The
            // icon and the `aria-label` still say what it is.
            class="fixed left-0 top-1/3 -translate-y-1/4 h-[20vh] min-h-[7rem] w-5 sm:w-8 flex flex-col items-center justify-center gap-2 bg-[#0d2137] border-[3px] border-l-0 border-[#f7931a]/70 hover:border-[#ffa534] rounded-r-lg cursor-pointer hover:bg-[#143050] hover:w-7 sm:hover:w-9 transition-all group shadow-lg shadow-black/30"
            on:click=move |_| set_open.set(true)
            title="Chart index"
        >
            <svg class="w-3 h-3 sm:w-4 sm:h-4 text-[#f7931a]/80 group-hover:text-[#ffa534] transition-colors shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M4 6h16M4 12h16M4 18h16"/>
            </svg>
            // Vertical, because a handle this narrow has room for the word
            // only if it runs with the tab rather than across it. Says what
            // opens, which an icon alone never did.
            <span
                class="hidden sm:block text-[10px] font-bold tracking-[0.18em] uppercase text-[#f7931a]/80 group-hover:text-[#ffa534] transition-colors whitespace-nowrap"
                style="writing-mode: vertical-rl"
            >
                "Charts"
            </span>
        </button>

        // Portaled to <body> so the overlay escapes ObservatoryPage's stacking
        // context. That section carries `opacity-0 animate-fadeinone`, and a
        // forwards-filled opacity animation keeps creating a stacking context
        // after it has finished, not just while it runs. That confined these
        // children to the section, so the sticky navbar (z-30, root context)
        // painted over the panel however high its z-index went. Verified by
        // rendering the three cases side by side: a descendant of a plain
        // ancestor overlays the navbar, a descendant of the finished animation
        // does not.
        <Portal>
            // Backdrop
            <Show when=move || open.get()>
                <div
                    style="z-index: 10002"
                    class="fixed inset-0 bg-black/50 transition-opacity"
                    on:click=move |_| set_open.set(false)
                />
            </Show>

            // Drawer panel. Full height rather than offset to sit below the
            // navbar: every hardcoded top goes stale as soon as anything above
            // the navbar changes height. `top-[48px]` was already short of the
            // ~53px navbar (~65px at 2xl) before the advisory banner pushed the
            // navbar down and buried the panel's first ~120px.
            <div
                style=move || format!(
                    "z-index: 10003; transform: translateX({}); transition: transform 0.25s ease-in-out;",
                    if open.get() { "0" } else { "-100%" }
                )
                class="fixed inset-y-0 left-0 w-80 sm:w-[22rem] bg-[#0d2137] border-r border-white/10 overflow-y-auto shadow-2xl shadow-black/50"
            >
                // Header. Says what the list is for, not just what it is
                // called: "Chart Index" alone left a reader to work out
                // whether this was navigation or settings.
                <div class="px-4 py-3.5 border-b border-white/10 sticky top-0 bg-[#0d2137] z-10">
                    <div class="flex items-start justify-between gap-3">
                        <div>
                            <h2 class="text-base font-semibold text-white">"All charts"</h2>
                            <p class="text-[0.7rem] text-white/40 mt-0.5">
                                {format!("{} metrics from my own node", chart_count())}
                            </p>
                        </div>
                        <button
                            class="text-white/30 hover:text-white/70 cursor-pointer p-1 -mr-1 -mt-0.5 rounded-md hover:bg-white/5 transition-colors"
                            aria-label="Close the chart index"
                            on:click=move |_| set_open.set(false)
                        >
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
                            </svg>
                        </button>
                    </div>
                </div>

                // Content
                <nav class="p-3">
                    // Built here rather than hoisted above the view: `Portal`
                    // takes its children as `Fn`, so the closure cannot move a
                    // captured Vec out of its environment.
                    {drawer_pages().into_iter().map(|page| {
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
                                    <span class="ml-1.5 font-normal normal-case tracking-normal text-white/25">
                                        {page.sections.iter().map(|s| s.charts.len()).sum::<usize>()}
                                    </span>
                                </div>
                                // Sections
                                {page.sections.into_iter().map(|section| {
                                    let has_label = !section.label.is_empty();
                                    view! {
                                        <div class="ml-1">
                                            {if has_label {
                                                Some(view! {
                                                    <div class="text-[11px] text-white/50 font-semibold uppercase tracking-wider px-2 pt-3 pb-1">
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
                                                                // 13px and py-1.5, up from 12px and py-1. Sixty-one
                                                                // entries at the old size read as a wall of grey
                                                                // rather than a list of things to pick.
                                                                class="w-full text-left text-[13px] text-white/65 hover:text-white hover:bg-white/[0.07] rounded-md px-3 py-1.5 cursor-pointer transition-colors"
                                                                on:click=move |_| {
                                                                    set_open.set(false);
                                                                    #[cfg(feature = "hydrate")]
                                                                    {
                                                                        // Reading a chart on its own page and picking
                                                                        // another from the index means you want that
                                                                        // chart's page, not the grid it came from.
                                                                        // Sending you back to the multi-chart view was
                                                                        // a demotion disguised as navigation.
                                                                        let on_single = leptos::prelude::window()
                                                                            .location()
                                                                            .pathname()
                                                                            .unwrap_or_default()
                                                                            .starts_with("/observatory/chart/");
                                                                        let url = if on_single {
                                                                            format!("/observatory/chart/{}", slug_of(card_id))
                                                                        } else if let Some(el) =
                                                                            leptos::prelude::document().get_element_by_id(card_id)
                                                                        {
                                                                            // Same page, flat layout: every card is in
                                                                            // the DOM, so scroll rather than reload.
                                                                            el.scroll_into_view();
                                                                            return;
                                                                        } else {
                                                                            format!("{}#{}", path_prefix, card_id)
                                                                        };
                                                                        let _ = leptos::prelude::window().location().set_href(&url);
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
        </Portal>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every drawer entry must point at a chart some page actually renders,
    /// and every rendered chart must be reachable from the drawer.
    ///
    /// The drawer is a hand-maintained index parallel to the pages, so it
    /// drifts silently in both directions: a removed chart leaves a nav item
    /// that scrolls nowhere, and a new chart is unreachable from the menu.
    /// Dropping the Input vs Output Value chart left exactly that dangling
    /// entry and nothing failed. It was caught by eye, which is the part worth
    /// fixing.
    ///
    /// Page sources are embedded with `include_str!` so this stays a hermetic
    /// unit test rather than reading the filesystem at runtime. When the chart
    /// registry lands (notes/single-chart-view-spec.md) the drawer should
    /// derive from it and this whole class of drift disappears, at which point
    /// this test can go.
    #[test]
    fn drawer_links_only_to_charts_that_exist() {
        const PAGES: &[&str] = &[
            include_str!("../network.rs"),
            include_str!("../fees.rs"),
            include_str!("../mining.rs"),
            include_str!("../embedded.rs"),
        ];

        let rendered: Vec<String> = PAGES
            .iter()
            .flat_map(|src| {
                src.split("chart_id=\"").skip(1).filter_map(|rest| {
                    rest.split('"').next().map(str::to_string)
                })
            })
            .collect();
        assert!(
            rendered.len() > 50,
            "expected to parse chart ids out of the page sources, found {}",
            rendered.len()
        );

        let mut dangling = Vec::new();
        let mut linked = Vec::new();
        for page in drawer_pages() {
            for section in &page.sections {
                for chart in &section.charts {
                    // Drawer ids are the card wrapper: "card-" + chart_id.
                    let id = chart
                        .card_id
                        .strip_prefix("card-")
                        .unwrap_or(chart.card_id);
                    linked.push(id.to_string());
                    if !rendered.iter().any(|r| r == id) {
                        dangling.push(format!(
                            "{:?} -> {} (no page renders it)",
                            chart.label, chart.card_id
                        ));
                    }
                }
            }
        }
        assert!(
            dangling.is_empty(),
            "drawer entries pointing at charts that do not exist:\n  {}",
            dangling.join("\n  ")
        );

        let unreachable: Vec<&String> = rendered
            .iter()
            .filter(|r| !linked.iter().any(|l| l == *r))
            .collect();
        assert!(
            unreachable.is_empty(),
            "charts rendered but not linked from the drawer: {unreachable:?}"
        );
    }
}

#[cfg(test)]
mod registry_link_tests {
    use super::*;

    /// From a single-chart page the drawer navigates to another chart's own
    /// page, so every entry has to name a chart the registry knows. An entry
    /// that does not is a 404 reachable from the index of all charts.
    ///
    /// This is the third hand-maintained list of the same charts (pages,
    /// drawer, registry), and the direction that matters most now that the
    /// drawer produces URLs rather than scroll targets.
    #[test]
    fn every_drawer_entry_resolves_to_a_chart_page() {
        let mut missing = Vec::new();
        let mut count = 0;
        for page in drawer_pages() {
            for section in &page.sections {
                for chart in &section.charts {
                    count += 1;
                    let slug = slug_of(chart.card_id);
                    if crate::stats::charts::registry::find(slug).is_none() {
                        missing.push((chart.label, chart.card_id, slug));
                    }
                }
            }
        }
        assert!(
            count > 50,
            "parsed only {count} drawer entries; the drawer, not the \
             registry, is probably what changed"
        );
        assert!(
            missing.is_empty(),
            "drawer entries with no chart page, which now 404 from the \
             chart index: {missing:?}"
        );
    }

    /// The mapping the link depends on, pinned in both shapes the ids take.
    #[test]
    fn a_card_id_reduces_to_its_slug() {
        assert_eq!(slug_of("card-chart-difficulty"), "difficulty");
        assert_eq!(slug_of("chart-difficulty"), "difficulty");
        assert_eq!(slug_of("difficulty"), "difficulty");
    }
}
