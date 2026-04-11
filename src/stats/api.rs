//! REST API endpoints for the stats module.
//!
//! These endpoints serve the Axum JSON API (used by external clients).
//! Leptos frontend components use server functions in `server_fns.rs` instead.
//!
//! ## Endpoints
//!
//! - `GET /api/blocks?from=&to=` - Block data by height range (default: last 144 blocks)
//! - `GET /api/blocks/:height` - Single block detail
//! - `GET /api/stats` - DB summary (block count, height range)
//! - `GET /api/live` - Real-time node + mempool + network stats (10s cache)
//! - `GET /api/op-returns?from=&to=` - OP_RETURN protocol breakdown (default: last 10k blocks)
//! - `GET /api/aggregates/daily?from=&to=` - Daily aggregated metrics by timestamp
//! - `GET /api/signaling?bit=N` or `?method=locktime` - Per-block signaling status
//! - `GET /api/signaling/periods?bit=N` - Signaling % per 2016-block retarget period
//! - `GET /api/heartbeat` - SSE stream for real-time mempool txs and block notifications
//!
//! ## Caching Strategy
//!
//! Most endpoints use HTTP Cache-Control headers (5-10s max-age). The `/api/live`
//! endpoint additionally caches the assembled response server-side for 10s. Price
//! data from mempool.space is cached for 60s with an atomic guard to prevent
//! concurrent refresh requests.
//!
//! ## SSE Connection Limiting
//!
//! The heartbeat SSE endpoint is capped at 256 concurrent connections via an
//! `AtomicUsize` counter with an RAII guard that decrements on drop.

use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::broadcast;

use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::stream::Stream;
use futures::StreamExt;
use serde::Deserialize;

/// Wrap a JSON value with a `Cache-Control: public, max-age=N` header.
fn cached_json(
    value: serde_json::Value,
    max_age: u32,
) -> ([(header::HeaderName, String); 1], Json<serde_json::Value>) {
    (
        [(header::CACHE_CONTROL, format!("public, max-age={max_age}"))],
        Json(value),
    )
}

type CachedResponse =
    ([(header::HeaderName, String); 1], Json<serde_json::Value>);

use super::db::{self, DbPool};
use super::error::StatsError;
use super::rpc::{BitcoinRpc, PriceInfo};
use super::types::PricePoint;

/// Generic cache type for range queries: (from, to, results, fetched_at).
type RangeCache<T> = Mutex<Option<(u64, u64, Vec<T>, Instant)>>;

/// Shared application state for the stats module. Holds the DB pool, RPC client,
/// and all in-memory caches. Wrapped in `Arc` and passed to all handlers.
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
    pub stats_summary_cache:
        Mutex<Option<(super::types::StatsSummary, Instant)>>,
    /// Cached daily aggregates: (from_ts, to_ts, results, fetched_at). 120s TTL.
    pub daily_cache: RangeCache<super::types::DailyAggregate>,
    /// Cached block timestamps: height → timestamp. Immutable data, never expires.
    pub block_ts_cache: Mutex<std::collections::HashMap<u64, u64>>,
    /// Cached signaling blocks: (cache_key, blocks, period_stats, fetched_at). 60s TTL.
    pub signaling_blocks_cache: Mutex<
        Option<(
            String,
            Vec<super::types::SignalingBlock>,
            super::types::PeriodStats,
            Instant,
        )>,
    >,
    /// Cached signaling periods: (cache_key, results, fetched_at). 60s TTL.
    pub signaling_periods_cache:
        Mutex<Option<(String, Vec<super::db::SignalingPeriod>, Instant)>>,
    /// Cached price history: (from_ts, to_ts, data, fetched_at).
    pub price_history_cache: RangeCache<PricePoint>,
    /// Cached range summary: (from_ts, to_ts, result, fetched_at). 60s TTL.
    pub range_summary_cache:
        Mutex<Option<(u64, u64, super::types::RangeSummary, Instant)>>,
    /// Cached extremes: (from_ts, to_ts, result, fetched_at). 60s TTL.
    pub extremes_cache:
        Mutex<Option<(u64, u64, super::types::ExtremesData, Instant)>>,
    /// Broadcast channel for real-time heartbeat events (ZMQ → SSE).
    pub heartbeat_tx: broadcast::Sender<super::zmq_subscriber::HeartbeatEvent>,
    /// Active SSE connection count (guard against connection exhaustion).
    pub sse_connections: AtomicUsize,
}

/// Maximum concurrent SSE connections before rejecting new ones.
const MAX_SSE_CONNECTIONS: usize = 256;

/// Arc-wrapped stats state, used as Axum extractor in all handlers.
pub type SharedStatsState = Arc<StatsState>;

/// RAII guard that decrements SSE connection count on drop.
struct SseConnectionGuard(Arc<StatsState>);

impl Drop for SseConnectionGuard {
    fn drop(&mut self) {
        self.0.sse_connections.fetch_sub(1, Ordering::Relaxed);
    }
}

const MAX_SUPPLY: f64 = 21_000_000.0;

/// Query parameters for block height range endpoints.
#[derive(Deserialize)]
pub struct BlocksQuery {
    /// Start block height (inclusive). Default: max_height - 144.
    pub from: Option<u64>,
    /// End block height (inclusive). Default: max_height.
    pub to: Option<u64>,
}

/// Query parameters for timestamp range endpoints.
#[derive(Deserialize)]
pub struct TimestampQuery {
    /// Start timestamp in unix seconds. Default: 0.
    pub from: Option<u64>,
    /// End timestamp in unix seconds. Default: u64::MAX.
    pub to: Option<u64>,
}

/// Query parameters for signaling endpoints.
#[derive(Deserialize)]
pub struct SignalingQuery {
    /// Version bit number (0-28) for BIP9 signaling. Default: 0.
    pub bit: Option<u32>,
    /// Signaling method: "bit" (default) or "locktime" (BIP-54).
    pub method: Option<String>,
    pub from: Option<u64>,
    pub to: Option<u64>,
}

/// Query parameters for signaling periods endpoint.
#[derive(Deserialize)]
pub struct SignalingPeriodsQuery {
    pub bit: Option<u32>,
    pub method: Option<String>,
}

/// GET /api/blocks - query blocks by height range.
/// Defaults to the last 144 blocks (~1 day) if no range is specified.
pub async fn get_blocks(
    State(state): State<SharedStatsState>,
    Query(params): Query<BlocksQuery>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let conn = state
        .db
        .get()
        .map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;

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

/// GET /api/blocks/:height - single block detail with coinbase metadata.
pub async fn get_block_detail(
    State(state): State<SharedStatsState>,
    Path(height): Path<u64>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let conn = state
        .db
        .get()
        .map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;
    let block = db::query_block_by_height(&conn, height)?;
    match block {
        Some(b) => Ok(Json(
            serde_json::to_value(b)
                .map_err(|e| StatsError::Rpc(e.to_string()))?,
        )),
        None => Ok(Json(serde_json::json!({ "error": "Block not found" }))),
    }
}

/// GET /api/stats - database summary (block count, height range). Cache: 10s.
pub async fn get_stats(
    State(state): State<SharedStatsState>,
) -> Result<CachedResponse, StatsError> {
    let conn = state
        .db
        .get()
        .map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;
    let stats = db::query_stats(&conn)?;
    match stats {
        Some(s) => Ok(cached_json(
            serde_json::to_value(s)
                .map_err(|e| StatsError::Rpc(e.to_string()))?,
            10,
        )),
        None => Ok(cached_json(
            serde_json::json!({
                "block_count": 0,
                "min_height": 0,
                "max_height": 0,
                "latest_timestamp": 0
            }),
            10,
        )),
    }
}

/// Serve last cached LiveStats with a stale flag when RPC is unreachable.
fn serve_stale_live(
    state: &SharedStatsState,
) -> Option<Result<CachedResponse, StatsError>> {
    tracing::warn!("RPC unreachable — serving stale LiveStats");
    let cached = state
        .live_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    cached.map(|(stats, _ts)| {
        let mut val = serde_json::to_value(&stats)
            .unwrap_or_else(|_| serde_json::json!({}));
        val["stale"] = serde_json::json!(true);
        Ok(cached_json(val, 5))
    })
}

/// GET /api/live - real-time node, mempool, and network stats. Cache: 10s.
/// Parallelizes RPC calls and serves stale data with a `stale` flag if RPC is down.
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

    // If core RPC calls fail, serve last cached LiveStats with stale flag
    let blockchain = match blockchain_res {
        Ok(b) => b,
        Err(e) => {
            return serve_stale_live(&state).unwrap_or(Err(e));
        }
    };
    let mempool = match mempool_res {
        Ok(m) => m,
        Err(e) => {
            return serve_stale_live(&state).unwrap_or(Err(e));
        }
    };
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
        let cached = state
            .price_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let need_refresh = match &cached {
            Some((_, ts)) => ts.elapsed().as_secs() > 60,
            None => true,
        };
        if need_refresh && !state.price_refreshing.swap(true, Ordering::AcqRel)
        {
            let result = match state.rpc.fetch_price().await {
                Ok(p) => {
                    let usd = p.usd;
                    *state
                        .price_cache
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) =
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

    let utxo_count = state
        .utxo_count
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .unwrap_or(0);

    Ok(cached_json(
        serde_json::json!({
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
        }),
        10,
    ))
}

/// GET /api/op-returns - OP_RETURN protocol breakdown by height range.
/// Defaults to the last 10,000 blocks if no range specified.
pub async fn get_op_returns(
    State(state): State<SharedStatsState>,
    Query(params): Query<BlocksQuery>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let conn = state
        .db
        .get()
        .map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;

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

/// GET /api/aggregates/daily - daily aggregated metrics by timestamp range.
pub async fn get_daily_aggregates(
    State(state): State<SharedStatsState>,
    Query(params): Query<TimestampQuery>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let conn = state
        .db
        .get()
        .map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;

    let from_ts = params.from.unwrap_or(0);
    let to_ts = params.to.unwrap_or(u64::MAX);

    let days = db::query_daily_aggregates(&conn, from_ts, to_ts)?;
    Ok(Json(serde_json::json!({ "days": days })))
}

/// GET /api/signaling - per-block signaling status (version bits or BIP-54 locktime).
/// Also returns period stats for the current 2016-block retarget window.
pub async fn get_signaling(
    State(state): State<SharedStatsState>,
    Query(params): Query<SignalingQuery>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let conn = state
        .db
        .get()
        .map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;
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

/// GET /api/signaling/periods - signaling percentage per retarget period (all time).
pub async fn get_signaling_periods(
    State(state): State<SharedStatsState>,
    Query(params): Query<SignalingPeriodsQuery>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let conn = state
        .db
        .get()
        .map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;
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

/// SSE endpoint for real-time heartbeat events (mempool txs + blocks).
/// On connect: sends recent tx history from DB, then streams live events.
pub async fn get_heartbeat_sse(
    State(state): State<SharedStatsState>,
) -> Result<
    (
        [(header::HeaderName, String); 1],
        Sse<impl Stream<Item = Result<Event, Infallible>>>,
    ),
    (axum::http::StatusCode, &'static str),
> {
    // Reject if too many concurrent SSE connections
    let prev = state.sse_connections.fetch_add(1, Ordering::Relaxed);
    if prev >= MAX_SSE_CONNECTIONS {
        state.sse_connections.fetch_sub(1, Ordering::Relaxed);
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Too many connections",
        ));
    }
    // Guard decrements connection count on drop (panic safety + client disconnect)
    let guard = SseConnectionGuard(Arc::clone(&state));

    let rx = state.heartbeat_tx.subscribe();

    // Load unconfirmed txs for the current flatline. The mempool_txs table
    // is sparsely populated after restarts (ZMQ only records NEW txs), so
    // many unconfirmed txs have no first_seen entry. Use a generous 2-hour
    // window to catch as many as possible. The query filters confirmed_height
    // IS NULL so only genuinely unconfirmed txs are returned.
    let (history, last_block_ts) = {
        let two_hours_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(7200);
        match state.db.get() {
            Ok(conn) => {
                let block_ts = db::max_height(&conn)
                    .ok()
                    .flatten()
                    .and_then(|h| db::query_block_timestamp(&conn, h).ok().flatten())
                    .unwrap_or(0);
                let txs = db::query_recent_mempool_txs(&conn, two_hours_ago, 10000)
                    .unwrap_or_default();
                (txs, block_ts)
            }
            Err(_) => (vec![], 0),
        }
    };

    // State machine: first emit history, then stream live events
    enum Phase {
        History(Vec<db::MempoolTxRow>, u64), // txs + last_block_timestamp
        Live,
    }

    let stream = futures::stream::unfold(
        (Phase::History(history, last_block_ts), rx),
        |(phase, mut rx)| async move {
            match phase {
                Phase::History(txs, block_ts) => {
                    if txs.is_empty() {
                        return Some((
                            Ok(Event::default().event("history").data(
                                format!(
                                "{{\"txs\":[],\"last_block_ts\":{block_ts}}}"
                            ),
                            )),
                            (Phase::Live, rx),
                        ));
                    }
                    let txs_json =
                        serde_json::to_string(&txs).unwrap_or_default();
                    let data = format!(
                        "{{\"txs\":{txs_json},\"last_block_ts\":{block_ts}}}"
                    );
                    Some((
                        Ok(Event::default().event("history").data(data)),
                        (Phase::Live, rx),
                    ))
                }
                Phase::Live => {
                    match rx.recv().await {
                        Ok(event) => {
                            let event_type = match &event {
                            super::zmq_subscriber::HeartbeatEvent::Tx { .. } => "tx",
                            super::zmq_subscriber::HeartbeatEvent::Block { .. } => "block",
                            super::zmq_subscriber::HeartbeatEvent::BlockMining => "block_mining",
                        };
                            let data = serde_json::to_string(&event)
                                .unwrap_or_default();
                            Some((
                                Ok(Event::default()
                                    .event(event_type)
                                    .data(data)),
                                (Phase::Live, rx),
                            ))
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::debug!(
                                "SSE client lagged, skipped {n} events"
                            );
                            Some((
                                Ok(Event::default()
                                    .event("lag")
                                    .data(format!("{{\"skipped\":{n}}}"))),
                                (Phase::Live, rx),
                            ))
                        }
                        Err(broadcast::error::RecvError::Closed) => None,
                    }
                }
            }
        },
    );

    let guarded_stream = stream.chain(futures::stream::once(async move {
        drop(guard);
        Ok(Event::default().comment(""))
    }));

    // X-Accel-Buffering: no tells nginx to not buffer this response (required for SSE)
    Ok((
        [(
            header::HeaderName::from_static("x-accel-buffering"),
            "no".to_string(),
        )],
        Sse::new(guarded_stream)
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15))),
    ))
}
