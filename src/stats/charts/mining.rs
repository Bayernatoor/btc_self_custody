//! Mining chart builders: miner dominance donut, empty blocks by month,
//! empty blocks by pool, and mining diversity index (HHI).

use super::*;
use serde_json::json;

const PIE_COLORS: [&str; 11] = [
    "#4ecdc4", "#f7931a", "#ff6b6b", "#bb8fff", "#2ecc71", "#e74c3c",
    "#3498db", "#e67e22", "#1abc9c", "#9b59b6", "#95a5a6",
];

/// Miner dominance donut chart.
pub fn miner_dominance_chart(miners: &[MinerShare]) -> serde_json::Value {
    if miners.is_empty() {
        return no_data_chart("Miner Dominance");
    }

    // Top 10 + "Other"
    let (top, rest) = if miners.len() > 10 {
        (&miners[..10], &miners[10..])
    } else {
        (miners, &[][..])
    };

    let mut pie_data: Vec<serde_json::Value> = top
        .iter()
        .map(|m| {
            json!({
                "name": m.miner,
                "value": m.count
            })
        })
        .collect();

    if !rest.is_empty() {
        let other_count: u64 = rest.iter().map(|m| m.count).sum();
        pie_data.push(json!({
            "name": "Other",
            "value": other_count
        }));
    }

    let colors: Vec<&str> =
        PIE_COLORS.iter().copied().take(pie_data.len()).collect();

    json!({
        "backgroundColor": "transparent",
        "color": colors,
        "tooltip": {
            "trigger": "item",
            "formatter": "{b}: {c} blocks ({d}%)",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "legend": { "show": false },
        "series": [{
            "name": "Miners",
            "type": "pie",
            "radius": ["40%", "70%"],
            "center": ["50%", "50%"],
            "avoidLabelOverlap": true,
            "itemStyle": {
                "borderRadius": 4,
                "borderColor": "#0e2a47",
                "borderWidth": 2
            },
            "label": {
                "show": true,
                "color": "#ccc",
                "fontSize": 10,
                "formatter": "{b}\n{d}%"
            },
            "labelLine": {
                "length": 10,
                "length2": 8
            },
            "emphasis": {
                "label": { "fontSize": 13, "fontWeight": "bold" }
            },
            "data": pie_data
        }]
    })
}

/// Empty blocks scatter chart.
pub fn empty_blocks_chart(blocks: &[EmptyBlock]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart_with_hint("No empty blocks in this range", "Try a longer range (1Y or ALL) to find empty blocks");
    }

    // Group empty blocks by month for a bar chart
    let mut monthly: std::collections::BTreeMap<String, u64> =
        std::collections::BTreeMap::new();
    for b in blocks {
        // Convert timestamp to YYYY-MM
        let dt = chrono::DateTime::from_timestamp(b.timestamp as i64, 0)
            .unwrap_or_default();
        let month = dt.format("%Y-%m").to_string();
        *monthly.entry(month).or_default() += 1;
    }

    let months: Vec<String> = monthly.keys().cloned().collect();
    let counts: Vec<u64> = monthly.values().copied().collect();

    build_option(json!({
        "xAxis": {
            "type": "category",
            "data": months,
            "axisLabel": { "color": "#aaa", "rotate": 45, "fontSize": 10 },
            "axisLine": { "lineStyle": { "color": "#555" } }
        },
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": false },
        "grid": { "left": 45, "right": 20, "top": 25, "bottom": 80 },
        "series": [{
            "name": "Empty Blocks",
            "type": "bar",
            "data": counts,
            "itemStyle": { "color": DATA_COLOR },
            "barMaxWidth": 20
        }]
    }))
}

/// Empty blocks grouped by mining pool. Shows which pools mine the most
/// coinbase-only blocks as a horizontal bar chart.
pub fn empty_blocks_by_pool_chart(blocks: &[EmptyBlock]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart_with_hint("No empty blocks in this range", "Try a longer range (1Y or ALL) to find empty blocks");
    }

    let mut pool_counts: std::collections::BTreeMap<&str, u64> =
        std::collections::BTreeMap::new();
    for b in blocks {
        *pool_counts.entry(&b.miner).or_default() += 1;
    }

    // Sort by count descending
    let mut sorted: Vec<(&str, u64)> = pool_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let pools: Vec<&str> = sorted.iter().map(|(name, _)| *name).collect();
    let counts: Vec<u64> = sorted.iter().map(|(_, count)| *count).collect();

    build_option(json!({
        "xAxis": {
            "type": "value",
            "name": "Empty Blocks",
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.1)" } }
        },
        "yAxis": {
            "type": "category",
            "data": pools,
            "inverse": true,
            "axisLabel": { "color": "#ccc", "fontSize": 11 },
            "axisLine": { "lineStyle": { "color": "#555" } }
        },
        "grid": { "left": 120, "right": 30, "top": 25, "bottom": 30 },
        "tooltip": tooltip_axis(),
        "series": [{
            "name": "Empty Blocks",
            "type": "bar",
            "data": counts,
            "itemStyle": { "color": DATA_COLOR },
            "barMaxWidth": 24
        }]
    }))
}

/// Mining diversity index (Herfindahl-Hirschman Index) computed from pool shares.
/// HHI ranges from 0 (perfectly distributed) to 10,000 (single miner).
/// Lower = more decentralized mining. Displayed as a single gauge-style value.
pub fn mining_diversity_chart(miners: &[MinerShare]) -> serde_json::Value {
    if miners.is_empty() {
        return no_data_chart("Mining Diversity");
    }

    // Exclude "Unknown" miners from HHI - early blocks have unidentifiable
    // miners lumped under one label, which inflates concentration artificially.
    let known: Vec<&MinerShare> = miners.iter().filter(|m| m.miner != "Unknown").collect();
    let total: u64 = known.iter().map(|m| m.count).sum();
    if total == 0 {
        return no_data_chart("Mining Diversity");
    }

    let hhi: f64 = known
        .iter()
        .map(|m| {
            let share = m.count as f64 / total as f64 * 100.0;
            share * share
        })
        .sum();

    let hhi_rounded = (hhi * 10.0).round() / 10.0;

    // Interpret: <1000 = competitive, 1000-1800 = moderate, >1800 = concentrated
    let (label, color) = if hhi < 1000.0 {
        ("Competitive", "#22c55e")
    } else if hhi < 1800.0 {
        ("Moderate", "#f59e0b")
    } else {
        ("Concentrated", "#ef4444")
    };

    let pool_count = known.len();

    json!({
        "backgroundColor": "transparent",
        "series": [{
            "type": "gauge",
            "startAngle": 200,
            "endAngle": -20,
            "min": 0,
            "max": 5000,
            "splitNumber": 5,
            "center": ["50%", "60%"],
            "radius": "85%",
            "progress": { "show": true, "roundCap": true, "width": 12 },
            "pointer": { "show": false },
            "axisLine": {
                "roundCap": true,
                "lineStyle": { "width": 12, "color": [[0.2, "#22c55e"], [0.36, "#f59e0b"], [1.0, "#ef4444"]] }
            },
            "axisTick": { "show": false },
            "splitLine": { "show": false },
            "axisLabel": { "show": false },
            "title": {
                "show": true,
                "offsetCenter": [0, "75%"],
                "fontSize": 13,
                "color": color
            },
            "detail": {
                "valueAnimation": false,
                "fontSize": 28,
                "fontWeight": "bold",
                "offsetCenter": [0, "40%"],
                "color": "#fff",
                "formatter": format!("{{value}}\n{pool_count} pools")
            },
            "data": [{
                "value": hhi_rounded,
                "name": label
            }]
        }]
    })
}
