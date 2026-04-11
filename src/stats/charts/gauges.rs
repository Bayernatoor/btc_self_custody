//! Gauge chart builders for dashboard widgets.

#[allow(unused_imports)]
use super::*;
use serde_json::json;

/// Mempool usage gauge chart. Shows current mempool memory usage as a percentage
/// of the configured maxmempool size, with color bands (green/orange/red).
pub fn mempool_gauge(usage: u64, max: u64) -> String {
    let pct = if max > 0 {
        (usage as f64 / max as f64 * 100.0 * 10.0).round() / 10.0
    } else {
        0.0
    };

    serde_json::to_string(&json!({
        "backgroundColor": "transparent",
        "series": [{
            "type": "gauge",
            "center": ["50%", "55%"],
            "radius": "85%",
            "detail": {
                "formatter": "{value}%",
                "color": "#e0e0e0",
                "fontSize": 16,
                "offsetCenter": [0, "65%"]
            },
            "data": [{ "value": pct, "name": "Mempool" }],
            "axisLine": {
                "lineStyle": {
                    "width": 12,
                    "color": [[0.5, "#4ecdc4"], [0.8, "#f7931a"], [1, "#e74c3c"]]
                }
            },
            "pointer": { "width": 3, "length": "65%" },
            "title": { "color": "#8899aa", "fontSize": 12, "offsetCenter": [0, "82%"] },
            "axisTick": { "show": false },
            "splitLine": { "length": 8, "lineStyle": { "color": "#8899aa" } },
            "axisLabel": { "color": "#8899aa", "distance": 15, "fontSize": 10 }
        }]
    }))
    .unwrap_or_default()
}
