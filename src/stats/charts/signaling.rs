//! BIP signaling chart builders: per-block scatter chart and retarget period
//! history bar chart with activation threshold line.

use super::*;
use serde_json::json;

/// Per-block signaling scatter/bar chart.
pub fn signaling_chart(blocks: &[SignalingBlock]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("BIP Signaling");
    }

    let signaled: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), if b.signaled { 1 } else { 0 }]))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": {
            "type": "value", "name": "Signaled",
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": {
                "color": "#aaa",
                "formatter": "{value}"
            },
            "min": 0, "max": 1, "interval": 1,
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Signaled", "type": "scatter", "data": signaled,
                "itemStyle": { "color": DATA_COLOR },
                "symbolSize": 4
            }
        ]
    }))
}

/// Signaling percentage per retarget period bar chart.
pub fn signaling_periods_chart(
    periods: &[SignalingPeriod],
    threshold: f64,
) -> serde_json::Value {
    if periods.is_empty() {
        return no_data_chart("Signaling Periods");
    }

    let cats: Vec<String> =
        periods.iter().map(|p| format_num(p.start_height)).collect();

    let bar_data: Vec<serde_json::Value> = periods
        .iter()
        .map(|p| {
            let color = if p.signaled_pct >= threshold {
                SIGNAL_YES
            } else if p.signaled_pct > 0.0 {
                TARGET_COLOR
            } else {
                "#333"
            };
            json!({
                "value": (p.signaled_pct * 1000.0).round() / 1000.0,
                "itemStyle": { "color": color }
            })
        })
        .collect();

    build_option(json!({
        "xAxis": {
            "type": "category", "data": cats,
            "axisLabel": { "color": "#aaa", "rotate": 45, "fontSize": 10 },
            "axisLine": { "lineStyle": { "color": "#555" } }
        },
        "yAxis": {
            "type": "value", "name": "%", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "grid": { "left": 45, "right": 20, "top": 35, "bottom": 80 },
        "dataZoom": data_zoom(),
        "tooltip": {
            "trigger": "axis",
            "formatter": "{b}<br/>Signaled: {c}%",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "series": [
            {
                "name": "Signaled %", "type": "bar", "data": bar_data,
                "barMaxWidth": 40,
                "markLine": {
                    "silent": true, "symbol": "none",
                    "lineStyle": { "color": "#f7931a", "type": "dashed", "width": 2 },
                    "data": [{ "yAxis": threshold, "label": { "formatter": format!("{}%", threshold), "color": "#f7931a", "fontSize": 12 } }]
                }
            }
        ]
    }))
}
