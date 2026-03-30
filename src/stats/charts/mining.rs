use serde_json::json;
use super::*;

const PIE_COLORS: [&str; 11] = [
    "#4ecdc4", "#f7931a", "#ff6b6b", "#bb8fff", "#2ecc71", "#e74c3c",
    "#3498db", "#e67e22", "#1abc9c", "#9b59b6", "#95a5a6",
];

/// Miner dominance donut chart.
pub fn miner_dominance_chart(miners: &[MinerShare]) -> String {
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

    serde_json::to_string(&json!({
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
    }))
    .unwrap_or_default()
}

/// Empty blocks scatter chart.
pub fn empty_blocks_chart(blocks: &[EmptyBlock]) -> String {
    if blocks.is_empty() {
        return no_data_chart("No empty blocks in this range");
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
