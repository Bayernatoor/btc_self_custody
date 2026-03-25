//! REST API endpoints.
//!
//! Endpoints:
//! - GET /api/blocks?from=&to=          -- raw block data (default: last 144 blocks)
//! - GET /api/blocks/:height            -- single block detail
//! - GET /api/stats                     -- DB summary (block count, height range)
//! - GET /api/live                      -- real-time node + mempool + network stats
//! - GET /api/op-returns?from=&to=      -- OP_RETURN classification data (default: last 10k blocks)
//! - GET /api/aggregates/daily?from=&to= -- daily aggregated metrics (timestamps)
//! - GET /api/signaling?bit=N or method=locktime -- per-block signaling status
//! - GET /api/signaling/periods?bit=N   -- signaling % per retarget period

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::Json;
use serde::Deserialize;

/// Helper: wrap a JSON response with Cache-Control header.
fn cached_json(
    value: serde_json::Value,
    max_age: u32,
) -> ([(header::HeaderName, String); 1], Json<serde_json::Value>) {
    (
        [(header::CACHE_CONTROL, format!("public, max-age={max_age}"))],
        Json(value),
    )
}

type CachedResponse = ([(header::HeaderName, String); 1], Json<serde_json::Value>);

use super::db::{self, DbPool};
use super::error::StatsError;
use super::rpc::{BitcoinRpc, PriceInfo};
use super::types::PricePoint;

pub struct StatsState {
    pub db: DbPool,
    pub rpc: BitcoinRpc,
    /// Cached live stats result, refreshed at most every 10 seconds.
    pub live_cache: Mutex<Option<(super::types::LiveStats, Instant)>>,
    /// Cached price with timestamp, refreshed at most every 60 seconds.
    pub price_cache: Mutex<Option<(PriceInfo, Instant)>>,
    /// Guard: prevents multiple concurrent price refreshes.
    pub price_refreshing: AtomicBool,
    pub utxo_count: Mutex<Option<u64>>,
    /// Cached stats summary: (result, fetched_at). 60s TTL.
    pub stats_summary_cache: Mutex<Option<(super::types::StatsSummary, Instant)>>,
    /// Cached daily aggregates: (from_ts, to_ts, results, fetched_at). 120s TTL.
    pub daily_cache: Mutex<Option<(u64, u64, Vec<super::types::DailyAggregate>, Instant)>>,
    /// Cached block timestamps: height → timestamp. Immutable data, never expires.
    pub block_ts_cache: Mutex<std::collections::HashMap<u64, u64>>,
    /// Cached signaling periods: (cache_key, results, fetched_at). 60s TTL.
    pub signaling_periods_cache: Mutex<Option<(String, Vec<super::db::SignalingPeriod>, Instant)>>,
    /// Cached price history: (from_ts, to_ts, data, fetched_at).
    pub price_history_cache: Mutex<Option<(u64, u64, Vec<PricePoint>, Instant)>>,
}

pub type SharedStatsState = Arc<StatsState>;

const MAX_SUPPLY: f64 = 21_000_000.0;

#[derive(Deserialize)]
pub struct BlocksQuery {
    pub from: Option<u64>,
    pub to: Option<u64>,
}

#[derive(Deserialize)]
pub struct TimestampQuery {
    pub from: Option<u64>,
    pub to: Option<u64>,
}

#[derive(Deserialize)]
pub struct SignalingQuery {
    pub bit: Option<u32>,
    pub method: Option<String>, // "bit" (default) or "locktime"
    pub from: Option<u64>,
    pub to: Option<u64>,
}

#[derive(Deserialize)]
pub struct SignalingPeriodsQuery {
    pub bit: Option<u32>,
    pub method: Option<String>,
}

pub async fn get_blocks(
    State(state): State<SharedStatsState>,
    Query(params): Query<BlocksQuery>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let conn = state.db.get().map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;

    let (from, to) = match (params.from, params.to) {
        (Some(f), Some(t)) => (f, t),
        _ => {
            let stats = db::query_stats(&conn)?;
            match stats {
                Some(s) => {
                    let to = params.to.unwrap_or(s.max_height);
                    let from = params.from.unwrap_or(to.saturating_sub(144));
                    (from, to)
                }
                None => return Ok(Json(serde_json::json!({ "blocks": [] }))),
            }
        }
    };

    let blocks = db::query_blocks(&conn, from, to)?;
    Ok(Json(serde_json::json!({ "blocks": blocks })))
}

pub async fn get_block_detail(
    State(state): State<SharedStatsState>,
    Path(height): Path<u64>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let conn = state.db.get().map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;
    let block = db::query_block_by_height(&conn, height)?;
    match block {
        Some(b) => Ok(Json(serde_json::to_value(b).map_err(|e| StatsError::Rpc(e.to_string()))?)),
        None => Ok(Json(serde_json::json!({ "error": "Block not found" }))),
    }
}

pub async fn get_stats(
    State(state): State<SharedStatsState>,
) -> Result<CachedResponse, StatsError> {
    let conn = state.db.get().map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;
    let stats = db::query_stats(&conn)?;
    match stats {
        Some(s) => Ok(cached_json(serde_json::to_value(s).map_err(|e| StatsError::Rpc(e.to_string()))?, 10)),
        None => Ok(cached_json(serde_json::json!({
            "block_count": 0,
            "min_height": 0,
            "max_height": 0,
            "latest_timestamp": 0
        }), 10)),
    }
}

pub async fn get_live(
    State(state): State<SharedStatsState>,
) -> Result<CachedResponse, StatsError> {
    // Parallelize all RPC calls for faster response
    let (blockchain_res, mempool_res, hashrate_res, fee_res) = tokio::join!(
        state.rpc.get_blockchain_info(),
        state.rpc.get_mempool_info(),
        state.rpc.get_network_hashps(),
        state.rpc.estimate_smart_fee(1),
    );

    let blockchain = blockchain_res?;
    let mempool = mempool_res?;
    let hashrate = hashrate_res.unwrap_or_else(|e| {
        tracing::warn!("Failed to fetch hashrate: {e}");
        0.0
    });
    let next_block_fee = fee_res.unwrap_or_else(|e| {
        tracing::warn!("Failed to fetch fee estimate: {e}");
        0.0
    });

    // Price cache: only fetch from mempool.space if cache is >60s old.
    // Atomic guard prevents multiple concurrent HTTP requests on cache miss.
    let price_usd = {
        let cached = state.price_cache.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let need_refresh = match &cached {
            Some((_, ts)) => ts.elapsed().as_secs() > 60,
            None => true,
        };
        if need_refresh && !state.price_refreshing.swap(true, Ordering::AcqRel) {
            let result = match state.rpc.fetch_price().await {
                Ok(p) => {
                    let usd = p.usd;
                    *state.price_cache.lock().unwrap_or_else(|e| e.into_inner()) =
                        Some((p, Instant::now()));
                    usd
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch price: {e}");
                    cached.map(|(p, _)| p.usd).unwrap_or(0.0)
                }
            };
            state.price_refreshing.store(false, Ordering::Release);
            result
        } else {
            cached.map(|(p, _)| p.usd).unwrap_or(0.0)
        }
    };

    let total_supply = super::types::calc_supply(blockchain.blocks);
    let percent_issued = (total_supply / MAX_SUPPLY) * 100.0;

    let sats_per_dollar = if price_usd > 0.0 {
        (100_000_000.0 / price_usd).round() as u64
    } else {
        0
    };
    let market_cap = price_usd * total_supply;
    let chain_size_gb = blockchain.size_on_disk as f64 / 1_000_000_000.0;

    let utxo_count = state.utxo_count.lock().unwrap_or_else(|e| e.into_inner()).unwrap_or(0);

    Ok(cached_json(serde_json::json!({
        "blockchain": blockchain,
        "mempool": mempool,
        "next_block_fee": next_block_fee,
        "network": {
            "price_usd": price_usd,
            "sats_per_dollar": sats_per_dollar,
            "market_cap_usd": market_cap,
            "total_supply": total_supply,
            "max_supply": MAX_SUPPLY,
            "percent_issued": (percent_issued * 100.0).round() / 100.0,
            "utxo_count": utxo_count,
            "chain_size_gb": (chain_size_gb * 10.0).round() / 10.0,
            "hashrate": hashrate
        }
    }), 10))
}

pub async fn get_op_returns(
    State(state): State<SharedStatsState>,
    Query(params): Query<BlocksQuery>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let conn = state.db.get().map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;

    let (from, to) = match (params.from, params.to) {
        (Some(f), Some(t)) => (f, t),
        _ => {
            let stats = db::query_stats(&conn)?;
            match stats {
                Some(s) => {
                    let to = params.to.unwrap_or(s.max_height);
                    let from = params.from.unwrap_or(to.saturating_sub(10000));
                    (from, to)
                }
                None => return Ok(Json(serde_json::json!({ "blocks": [] }))),
            }
        }
    };

    let blocks = db::query_op_returns(&conn, from, to)?;
    Ok(Json(serde_json::json!({ "blocks": blocks })))
}

pub async fn get_daily_aggregates(
    State(state): State<SharedStatsState>,
    Query(params): Query<TimestampQuery>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let conn = state.db.get().map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;

    let from_ts = params.from.unwrap_or(0);
    let to_ts = params.to.unwrap_or(u64::MAX);

    let days = db::query_daily_aggregates(&conn, from_ts, to_ts)?;
    Ok(Json(serde_json::json!({ "days": days })))
}

pub async fn get_signaling(
    State(state): State<SharedStatsState>,
    Query(params): Query<SignalingQuery>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let conn = state.db.get().map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;
    let use_locktime = params.method.as_deref() == Some("locktime");

    let stats = db::query_stats(&conn)?;
    let (from, to) = match stats {
        Some(s) => {
            let to = params.to.unwrap_or(s.max_height);
            let from = params.from.unwrap_or(to.saturating_sub(2016));
            (from, to)
        }
        None => {
            return Ok(Json(
                serde_json::json!({ "blocks": [], "period_stats": null }),
            ))
        }
    };

    let blocks = if use_locktime {
        db::query_signaling_locktime(&conn, from, to)?
    } else {
        db::query_signaling_bit(&conn, params.bit.unwrap_or(0), from, to)?
    };

    // Period stats: retarget block (period_start) is the boundary between periods.
    // "Blocks since adjustment" starts at period_start (inclusive), matching mempool.space.
    let period_start = (to / 2016) * 2016;
    let period_end = period_start + 2015;
    // Query from period_start but report mined as tip - period_start (excluding the retarget block)
    let period_blocks = if use_locktime {
        db::query_signaling_locktime(&conn, period_start, period_end)?
    } else {
        db::query_signaling_bit(
            &conn,
            params.bit.unwrap_or(0),
            period_start,
            period_end,
        )?
    };
    let signaled_count =
        period_blocks.iter().filter(|b| b.signaled).count() as u64;
    let raw_total = period_blocks.len() as u64;
    // "Blocks since adjustment" excludes the retarget block itself (matches mempool.space)
    let mined = if raw_total > 0 { raw_total - 1 } else { 0 };
    let pct = if mined > 0 {
        signaled_count as f64 / mined as f64 * 100.0
    } else {
        0.0
    };

    Ok(Json(serde_json::json!({
        "blocks": blocks,
        "period_stats": {
            "period_start": period_start,
            "period_end": period_end,
            "total_blocks": mined,
            "signaled_count": signaled_count,
            "signaled_pct": pct
        }
    })))
}

pub async fn get_signaling_periods(
    State(state): State<SharedStatsState>,
    Query(params): Query<SignalingPeriodsQuery>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let conn = state.db.get().map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;
    let use_locktime = params.method.as_deref() == Some("locktime");
    let periods = if use_locktime {
        db::query_signaling_periods_locktime(&conn)?
    } else {
        db::query_signaling_periods_bit(&conn, params.bit.unwrap_or(0))?
    };
    Ok(Json(serde_json::json!({
        "periods": periods
    })))
}
