//! Mining charts: difficulty, pool distribution, empty blocks.

use leptos::prelude::*;

use crate::chart_signal;
use super::components::*;
use super::helpers::*;
use super::shared::*;
use crate::stats::server_fns::*;

#[component]
pub fn MiningChartsPage() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let overlay_flags = state.overlay_flags;
    let dashboard_data = create_dashboard_resource(range);

    let (section, set_section) = signal("difficulty".to_string());

    let diff_option = chart_signal!(dashboard_data, range, overlay_flags,
        |blocks| crate::stats::charts::difficulty_chart(blocks),
        |days| crate::stats::charts::difficulty_chart_daily(days)
    );

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

    let miner_chart_option = {
        let (cached, set_cached) = signal(String::new());
        Effect::new(move |_| {
            let result = mining_data
                .get()
                .and_then(|r| r.ok())
                .map(|(ref miners, _)| {
                    crate::stats::charts::miner_dominance_chart(miners)
                })
                .unwrap_or_default();
            if !result.is_empty() { set_cached.set(result); }
        });
        Signal::derive(move || cached.get())
    };

    let empty_blocks_option = {
        let (base_json, set_base_json) = signal(String::new());
        Effect::new(move |_| {
            let result = mining_data
                .get()
                .and_then(|r| r.ok())
                .map(|(_, ref empty)| crate::stats::charts::empty_blocks_chart(empty))
                .unwrap_or_default();
            if !result.is_empty() { set_base_json.set(result); }
        });
        let (cached, set_cached) = signal(String::new());
        Effect::new(move |_| {
            let json = base_json.get();
            let flags = overlay_flags.read();
            if json.is_empty() { return; }
            set_cached.set(crate::stats::charts::apply_overlays(&json, &flags, true));
        });
        Signal::derive(move || cached.get())
    };

    view! {
        <ChartPageLayout
            title="Mining"
            description="Difficulty adjustments and mining pool distribution"
            header=move || view! {
                <div class="flex flex-wrap gap-2 justify-center mb-6">
                    {[("difficulty", "Difficulty"), ("pools", "Pool Distribution")].into_iter().map(|(id, label)| {
                        let id_str = id.to_string();
                        let id_clone = id_str.clone();
                        view! {
                            <button
                                class=move || {
                                    if section.get() == id_clone {
                                        "px-4 py-1.5 text-xs rounded-lg bg-white/10 text-white font-semibold border border-white/20 cursor-pointer"
                                    } else {
                                        "px-4 py-1.5 text-xs rounded-lg text-white/40 hover:text-white/70 hover:bg-white/5 transition-all cursor-pointer"
                                    }
                                }
                                on:click={
                                    let id = id_str.clone();
                                    move |_| set_section.set(id.clone())
                                }
                            >
                                {label}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>
            }
        >
            // --- Difficulty sub-section ---
            <div class=move || if section.get() == "difficulty" { "space-y-10" } else { "hidden" }>
                <ChartCard title="Difficulty" description="Mining difficulty, adjusts every 2,016 blocks (~2 weeks) to maintain 10-minute block targets" chart_id="chart-difficulty" option=diff_option/>
            </div>

            // --- Pool Distribution sub-section ---
            <div class=move || if section.get() == "pools" { "space-y-10" } else { "hidden" }>
                <ChartCard title="Mining Pool Share" description="Which mining pools are finding the most blocks. More distributed is healthier for the network" chart_id="chart-miner-dominance" option=miner_chart_option/>
                <ChartCard title="Empty Blocks" description="Blocks with no user transactions, usually mined before the pool has received the previous block's transactions" chart_id="chart-empty-blocks" option=empty_blocks_option/>
            </div>
        </ChartPageLayout>
    }
}
