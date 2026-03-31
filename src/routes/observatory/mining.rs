//! Mining charts: difficulty, pool distribution, empty blocks.

use leptos::prelude::*;
use leptos_meta::*;

use super::components::*;
use super::helpers::*;
use super::shared::*;
use crate::chart_memo;
use crate::stats::server_fns::*;

#[component]
pub fn MiningChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = state.dashboard_data;

    // Sub-section navigation — initialized from URL, created OUTSIDE the reactive closure
    let query = leptos_router::hooks::use_query_map();
    let initial_section = query
        .read_untracked()
        .get("section")
        .filter(|s| ["difficulty", "pools"].contains(&s.as_str()))
        .unwrap_or_else(|| "difficulty".to_string());
    let (section, set_section) = signal(initial_section);

    // Mining-specific data (pool dominance + empty blocks)
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

    view! {
        <Title text="Bitcoin Mining Charts: Difficulty & Pool Distribution | WE HODL BTC"/>
        <Meta name="description" content="Bitcoin mining analytics with difficulty adjustment history, mining pool dominance distribution including OCEAN template miners, and empty block tracking across the network."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/charts/mining"/>
        <ChartPageLayout
            title="Mining"
            description="Difficulty adjustments and mining pool distribution"
            seo_text="Monitor Bitcoin's mining landscape. The difficulty chart tracks the network's computational security as it adjusts every 2,016 blocks. Pool distribution shows which mining pools are producing blocks, with OCEAN template miners identified individually. Empty blocks are tracked historically, while common in Bitcoin's early years, they are rare today and typically indicate intentional miner behavior."
            header=move || view! {
                <div class="relative inline-block">
                    <select
                        aria-label="Chart section"
                        class="appearance-none bg-[#0a1a2e] text-white/80 text-sm border border-white/10 rounded-xl pl-3 pr-8 py-2 cursor-pointer focus:outline-none focus:border-[#f7931a]/40 transition-colors"
                        prop:value=move || section.get()
                        on:change=move |ev| {
                            use wasm_bindgen::JsCast;
                            if let Some(t) = ev.target() {
                                if let Ok(s) = t.dyn_into::<leptos::web_sys::HtmlSelectElement>() {
                                    let val = s.value();
                                    set_section.set(val.clone());
                                    #[cfg(feature = "hydrate")]
                                    super::shared::update_section_in_url(
                                        if val == "difficulty" { None } else { Some(&val) }
                                    );
                                }
                            }
                        }
                    >
                        <option value="difficulty">"Difficulty"</option>
                        <option value="pools">"Pool Distribution"</option>
                    </select>
                    <svg class="absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none w-3.5 h-3.5 text-white/40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                    </svg>
                </div>
            }
        >
            // --- Difficulty sub-section ---
            <Show when=move || section.get() == "difficulty" fallback=|| ()>
                {move || {
                    dashboard_data.get().and_then(|r| r.ok()).map(|_| {
                        let diff_option = chart_memo!(dashboard_data, range, overlay_flags,
                            |blocks| crate::stats::charts::difficulty_chart(blocks),
                            |days| crate::stats::charts::difficulty_chart_daily(days)
                        );
                        view! {
                            <div class="space-y-10">
                                <ChartCard title="Difficulty" description=chart_desc(range, "Mining difficulty per block, adjusts every 2,016 blocks (~2 weeks)", "Daily mining difficulty, adjusts every 2,016 blocks (~2 weeks)") chart_id="chart-difficulty" option=diff_option/>
                            </div>
                        }.into_any()
                    }).unwrap_or_else(|| view! { <ChartPageSkeleton count=1/> }.into_any())
                }}
            </Show>

            // --- Pool Distribution sub-section ---
            <Show when=move || section.get() == "pools" fallback=|| ()>
                {move || {
                    mining_data.get().and_then(|r| r.ok()).map(|_| {
                        let miner_chart_option = Signal::derive(move || {
                            mining_data.get().and_then(|r| r.ok())
                                .map(|(ref miners, _)| {
                                    let value = crate::stats::charts::miner_dominance_chart(miners);
                                    serde_json::to_string(&value).unwrap_or_default()
                                })
                                .unwrap_or_default()
                        });
                        let empty_blocks_option = Signal::derive(move || {
                            let flags = overlay_flags.get();
                            mining_data.get().and_then(|r| r.ok())
                                .map(|(_, ref empty)| {
                                    let mut value = crate::stats::charts::empty_blocks_chart(empty);
                                    if value.is_null() { return String::new(); }
                                    crate::stats::charts::apply_overlays(&mut value, &flags, true);
                                    serde_json::to_string(&value).unwrap_or_default()
                                })
                                .unwrap_or_default()
                        });

                        view! {
                            <div class="space-y-10">
                                <ChartCard title="Mining Pool Share" description="Which mining pools are finding the most blocks. More distributed is healthier for the network" chart_id="chart-miner-dominance" option=miner_chart_option/>
                                <ChartCard title="Empty Blocks" description="Blocks with no user transactions, usually mined before the pool has received the previous block's transactions" chart_id="chart-empty-blocks" option=empty_blocks_option/>
                            </div>
                        }.into_any()
                    }).unwrap_or_else(|| view! { <ChartPageSkeleton count=2/> }.into_any())
                }}
            </Show>
        </ChartPageLayout>
    }
}
