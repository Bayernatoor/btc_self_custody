//! REST API endpoints for the stats module.
//!
//! These endpoints serve the Axum JSON API (used by external clients).
//! Leptos frontend components use server functions in `server_fns.rs` instead.
//! All routes are nested under `/api/stats` by `main.rs`.
//!
//! ## Endpoints
//!
//! - `GET /api/stats/blocks?from=&to=` - Block data by height range (default: last 144 blocks)
//! - `GET /api/stats/blocks/:height` - Single block detail
//! - `GET /api/stats/stats` - DB summary (block count, height range)
//! - `GET /api/stats/cache-stats` - Per-method RPC cache hit/miss/error counters
//! - `GET /api/stats/live` - Real-time node + mempool + network stats
//! - `GET /api/stats/op-returns?from=&to=` - OP_RETURN protocol breakdown (default: last 10k blocks)
//! - `GET /api/stats/aggregates/daily?from=&to=` - Daily aggregated metrics by timestamp
//! - `GET /api/stats/signaling?bit=N` or `?method=locktime` - Per-block signaling status
//! - `GET /api/stats/signaling/periods?bit=N` - Signaling % per 2016-block retarget period
//! - `GET /api/stats/heartbeat` - SSE stream for real-time mempool txs and block notifications
//!
//! ## Caching Strategy
//!
//! All handlers use HTTP Cache-Control headers (5-10s max-age) for the browser
//! / CDN layer. Bitcoin Core RPC calls are cached inside `BitcoinRpc` per method
//! (see `rpc_cache.rs`) with TTLs of 1-60s, singleflight dedup against
//! `cs_main`-stall request floods, and stale-on-error fallback. There is no
//! separate handler-level cache for `/live` — the RPC cache handles it.
//! External HTTP price data (mempool.space) retains its own 60s cache with an
//! atomic guard against concurrent refreshes.
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

use super::cache::{Cache, CacheInvalidate, CacheRegistry, CacheTag};
use super::db::{self, DbPool};
use super::error::StatsError;
use super::rpc::{BitcoinRpc, PriceInfo};
use super::types::PricePoint;

/// Generic cache type for range queries: (from, to, results, fetched_at).
type RangeCache<T> = Mutex<Option<(u64, u64, Vec<T>, Instant)>>;

/// Shared application state for the stats module. Holds the DB pool, RPC client,
/// and all in-memory caches. Wrapped in `Arc` and passed to all handlers.
///
/// **Caching policy:** new caches must be added as `Arc<Cache<K, V>>`
/// (see `super::cache`) and registered via `StatsStateBuilder` (coming
/// in PR 2 of the cache-registry rollout). The `cache_registry` field
/// fans out invalidations by tag so trigger paths (block poller, ZMQ
/// hashblock handler, etc.) call `state.invalidate(tag)` once instead
/// of maintaining a hand-rolled list of `.take()` calls per cache.
///
/// The fields below marked `// DEPRECATED` are pre-existing manually
/// locked caches scheduled for migration. Do not add new fields in this
/// shape; use the cache primitive instead.
pub struct StatsState {
    pub db: DbPool,
    pub rpc: BitcoinRpc,
    /// Routes `state.invalidate(tag)` to subscribed caches. Constructed
    /// via `StatsStateBuilder::into_registry()` so every cache built
    /// through the builder is wired in automatically.
    pub cache_registry: CacheRegistry,
    /// DEPRECATED: migrate to `Cache<(), PriceInfo>` via `StatsStateBuilder::cache`.
    /// Cached price with timestamp, refreshed at most every 60 seconds.
    pub price_cache: Mutex<Option<(PriceInfo, Instant)>>,
    /// Guard: prevents multiple concurrent price refreshes.
    /// Will be replaced by the cache primitive's singleflight in the A-era refactor.
    pub price_refreshing: AtomicBool,
    /// DEPRECATED: migrate to `Cache<(), u64>` via `StatsStateBuilder::cache`.
    pub utxo_count: Mutex<Option<u64>>,
    /// Stats summary cache. Invalidated on new block via the registry.
    pub stats_summary_cache: Arc<Cache<(), super::types::StatsSummary>>,
    /// DEPRECATED: migrate to `Cache<(u64, u64), Vec<DailyAggregate>>`.
    /// Cached daily aggregates: (from_ts, to_ts, results, fetched_at). 120s TTL.
    pub daily_cache: RangeCache<super::types::DailyAggregate>,
    /// Block height -> timestamp cache. Immutable data so no TTL or
    /// invalidation; entries are written once and live for the process.
    pub block_ts_cache: Arc<Cache<u64, u64>>,
    /// DEPRECATED: migrate to `Cache<String, (Vec<SignalingBlock>, PeriodStats)>`.
    /// Cached signaling blocks: (cache_key, blocks, period_stats, fetched_at). 60s TTL.
    pub signaling_blocks_cache: Mutex<
        Option<(
            String,
            Vec<super::types::SignalingBlock>,
            super::types::PeriodStats,
            Instant,
        )>,
    >,
    /// DEPRECATED: migrate to `Cache<String, Vec<SignalingPeriod>>`.
    /// Cached signaling periods: (cache_key, results, fetched_at). 60s TTL.
    pub signaling_periods_cache:
        Mutex<Option<(String, Vec<super::db::SignalingPeriod>, Instant)>>,
    /// DEPRECATED: migrate to `Cache<(u64, u64), Vec<PricePoint>>`.
    pub price_history_cache: RangeCache<PricePoint>,
    /// DEPRECATED: migrate to `Cache<(u64, u64), RangeSummary>`.
    /// Cached range summary: (from_ts, to_ts, result, fetched_at). 60s TTL.
    pub range_summary_cache:
        Mutex<Option<(u64, u64, super::types::RangeSummary, Instant)>>,
    /// DEPRECATED: migrate to `Cache<(u64, u64), ExtremesData>`.
    /// Cached extremes: (from_ts, to_ts, result, fetched_at). 60s TTL.
    pub extremes_cache:
        Mutex<Option<(u64, u64, super::types::ExtremesData, Instant)>>,
    /// Broadcast channel for real-time heartbeat events (ZMQ → SSE).
    pub heartbeat_tx: broadcast::Sender<super::zmq_subscriber::HeartbeatEvent>,
    /// Active SSE connection count (guard against connection exhaustion).
    pub sse_connections: AtomicUsize,
}

impl StatsState {
    /// Fan an invalidation event out to every cache registered for `tag`.
    /// Trigger sites (block poller, ZMQ hashblock handler, etc.) call
    /// this once per event; the registry handles fan-out. Replaces the
    /// hand-rolled `.take()` chain that previously lived in `startup.rs`.
    pub fn invalidate(&self, tag: CacheTag) {
        self.cache_registry.invalidate(tag);
    }
}

/// Helper for constructing the cache layer of `StatsState`. Each
/// `cache(...)` call builds a `Cache<K, V>`, registers it with the
/// internal `CacheRegistry` under each provided tag, and returns an
/// `Arc<Cache<K, V>>` the caller stores on the `StatsState` field.
/// `into_registry()` consumes the builder and yields the populated
/// registry, which the caller assigns to `StatsState::cache_registry`.
///
/// Adding a new cache is one line; the registration step is not
/// optional, so a cache cannot ship without an invalidation contract.
#[derive(Default)]
pub struct StatsStateBuilder {
    registry: CacheRegistry,
}

impl StatsStateBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a cache, register it under each tag in `tags`, and
    /// return the `Arc` for the caller to store on `StatsState`.
    pub fn cache<K, V>(
        &mut self,
        ttl: Duration,
        tags: &[CacheTag],
    ) -> Arc<Cache<K, V>>
    where
        K: Eq + std::hash::Hash + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let mut cache = Cache::new(ttl);
        for &tag in tags {
            cache = cache.invalidated_by(tag);
        }
        let arc = Arc::new(cache);
        for &tag in tags {
            self.registry
                .subscribe(tag, arc.clone() as Arc<dyn CacheInvalidate>);
        }
        arc
    }

    /// Consume the builder and return the populated registry.
    pub fn into_registry(self) -> CacheRegistry {
        self.registry
    }
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

/// GET /api/stats/cache-stats - per-method RPC cache hit/miss/error counters.
/// Useful for tuning TTLs in production: if a slot shows low hit rate (<80%)
/// the TTL may be too short or traffic too bursty; if stale_served > 0 Core
/// was unreachable at some point and the stale-on-error fallback kicked in.
pub async fn get_cache_stats(
    State(state): State<SharedStatsState>,
) -> Result<CachedResponse, StatsError> {
    // TTL-based slots (live-stats RPCs): report staleness, errors, age.
    let slots: Vec<serde_json::Value> = state
        .rpc
        .cache_stats()
        .into_iter()
        .map(|(method, stats)| {
            let total = stats.hits + stats.misses;
            let hit_rate = if total == 0 {
                0.0
            } else {
                stats.hits as f64 / total as f64
            };
            serde_json::json!({
                "method": method,
                "hits": stats.hits,
                "misses": stats.misses,
                "errors": stats.errors,
                "stale_served": stats.stale_served,
                "hit_rate": (hit_rate * 1000.0).round() / 1000.0,
                "age_seconds": stats.age_seconds,
            })
        })
        .collect();
    // LRU-based slots (immutable block data): report capacity utilization.
    let blocks: Vec<serde_json::Value> = state
        .rpc
        .block_cache_stats()
        .into_iter()
        .map(|(method, stats)| {
            let total = stats.hits + stats.misses;
            let hit_rate = if total == 0 {
                0.0
            } else {
                stats.hits as f64 / total as f64
            };
            serde_json::json!({
                "method": method,
                "hits": stats.hits,
                "misses": stats.misses,
                "size": stats.size,
                "capacity": stats.capacity,
                "hit_rate": (hit_rate * 1000.0).round() / 1000.0,
            })
        })
        .collect();
    // 0s cache on this endpoint — operators need to see current state
    Ok(cached_json(
        serde_json::json!({ "slots": slots, "blocks": blocks }),
        0,
    ))
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

/// GET /api/stats/live - real-time node, mempool, and network stats.
///
/// No dedicated handler-level cache. The underlying RPC calls are cached in
/// `BitcoinRpc` per method (see `rpc_cache.rs`); that layer handles singleflight
/// dedup, per-method TTLs, and stale-on-error fallback. If any RPC call fell
/// back to a stale value, the response JSON includes `"stale": true` so the
/// client can surface a "data may be outdated" indicator.
pub async fn get_live(
    State(state): State<SharedStatsState>,
) -> Result<CachedResponse, StatsError> {
    let t_total = Instant::now();

    // Parallelize all RPC calls for faster response.
    // Each branch is wrapped to capture per-call latency for slow-request diagnostics.
    let (br, mr, hr, fr) = tokio::join!(
        async {
            let s = Instant::now();
            let r = state.rpc.get_blockchain_info().await;
            (r, s.elapsed())
        },
        async {
            let s = Instant::now();
            let r = state.rpc.get_mempool_info().await;
            (r, s.elapsed())
        },
        async {
            let s = Instant::now();
            let r = state.rpc.get_network_hashps().await;
            (r, s.elapsed())
        },
        async {
            let s = Instant::now();
            let r = state.rpc.estimate_smart_fee(1).await;
            (r, s.elapsed())
        },
    );
    let (blockchain_res, t_blockchain) = br;
    let (mempool_res, t_mempool) = mr;
    let (hashrate_res, t_hashrate) = hr;
    let (fee_res, t_fee) = fr;

    // Track staleness across the four RPCs. Any OR-combines to response-level.
    let mut stale = false;

    let (blockchain, b_stale) = blockchain_res?;
    stale |= b_stale;
    let (mempool, m_stale) = mempool_res?;
    stale |= m_stale;
    let (hashrate, h_stale) = hashrate_res.unwrap_or_else(|e| {
        tracing::warn!("Failed to fetch hashrate: {e}");
        (0.0, false)
    });
    stale |= h_stale;
    let (next_block_fee, f_stale) = fee_res.unwrap_or_else(|e| {
        tracing::warn!("Failed to fetch fee estimate: {e}");
        (0.0, false)
    });
    stale |= f_stale;

    // Price cache: only fetch from mempool.space if cache is >60s old.
    // Atomic guard prevents multiple concurrent HTTP requests on cache miss.
    let t_price_start = Instant::now();
    let mut price_did_fetch = false;
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
            price_did_fetch = true;
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
    let t_price = if price_did_fetch {
        t_price_start.elapsed()
    } else {
        std::time::Duration::ZERO
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

    // Slow-request diagnostic: log per-call timings only when the handler
    // exceeds 1s total or any single upstream call exceeds 500ms. Helps
    // identify which RPC (or the mempool.space price fetch) is responsible
    // for the occasional 10+s "pending" hang seen in production.
    let total = t_total.elapsed();
    let slow_call = std::time::Duration::from_millis(500);
    if total > std::time::Duration::from_secs(1)
        || t_blockchain > slow_call
        || t_mempool > slow_call
        || t_hashrate > slow_call
        || t_fee > slow_call
        || t_price > slow_call
    {
        tracing::warn!(
            "live_stats slow: total={}ms blockchain={}ms mempool={}ms hashrate={}ms fee={}ms price={}ms (price_fetched={})",
            total.as_millis(),
            t_blockchain.as_millis(),
            t_mempool.as_millis(),
            t_hashrate.as_millis(),
            t_fee.as_millis(),
            t_price.as_millis(),
            price_did_fetch,
        );
    }

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
            },
            "stale": stale,
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

/// GET /api/stats/tx/{txid} - fetch full transaction details from our own node.
/// Uses getrawtransaction with verbose=true (requires txindex=1).
/// Replaces the mempool.space API dependency for the tx detail modal.
pub async fn get_tx_detail(
    State(state): State<SharedStatsState>,
    Path(txid): Path<String>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, &'static str)> {
    // Validate txid format (64 hex chars)
    if txid.len() != 64 || !txid.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err((axum::http::StatusCode::BAD_REQUEST, "Invalid txid"));
    }

    match state.rpc.get_raw_transaction(&txid).await {
        Ok(tx) => {
            // Transform the Bitcoin Core JSON into a format the frontend expects.
            // Bitcoin Core returns vin[].prevout for spent outputs (if utxo index),
            // vout[].value in BTC (float), etc.
            let vin = tx["vin"].as_array();
            let vout = tx["vout"].as_array();

            let input_count = vin.map(|v| v.len()).unwrap_or(0);
            let output_count = vout.map(|v| v.len()).unwrap_or(0);
            let size = tx["size"].as_u64().unwrap_or(0);
            let vsize = tx["vsize"].as_u64().unwrap_or(0);
            let weight = tx["weight"].as_u64().unwrap_or(0);

            // Total output value (BTC float -> sats)
            let total_output_sats: u64 = vout
                .map(|outputs| {
                    outputs
                        .iter()
                        .filter_map(|o| o["value"].as_f64())
                        .map(|v| (v * 100_000_000.0).round() as u64)
                        .sum()
                })
                .unwrap_or(0);

            // Fee: if vin prevout values are present (utxo index), compute fee.
            // Otherwise check mempool entry for unconfirmed txs.
            let input_total_sats: u64 = vin
                .map(|inputs| {
                    inputs
                        .iter()
                        .filter_map(|i| i["prevout"]["value"].as_f64())
                        .map(|v| (v * 100_000_000.0).round() as u64)
                        .sum()
                })
                .unwrap_or(0);
            let fee_sats = input_total_sats.saturating_sub(total_output_sats);
            let fee_rate = if vsize > 0 && fee_sats > 0 {
                fee_sats as f64 / vsize as f64
            } else {
                0.0
            };

            // Confirmation status
            let confirmations = tx["confirmations"].as_u64().unwrap_or(0);
            let block_hash = tx["blockhash"].as_str().unwrap_or("");
            let block_height = tx["blockheight"].as_u64();

            // Detect features from vin
            let is_coinbase = vin
                .map(|inputs| inputs.iter().any(|i| !i["coinbase"].is_null()))
                .unwrap_or(false);
            let has_witness = vin
                .map(|inputs| {
                    inputs.iter().any(|i| {
                        i["txinwitness"]
                            .as_array()
                            .map(|w| !w.is_empty())
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            let has_taproot = vin
                .map(|inputs| {
                    inputs.iter().any(|i| {
                        i["prevout"]["scriptPubKey"]["type"].as_str()
                            == Some("witness_v1_taproot")
                    })
                })
                .unwrap_or(false);
            let is_rbf = vin
                .map(|inputs| {
                    inputs.iter().any(|i| {
                        i["sequence"]
                            .as_u64()
                            .map(|s| s < 0xFFFFFFFE)
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);

            let mut features = Vec::new();
            if is_coinbase {
                features.push("Coinbase");
            }
            if has_witness {
                features.push("SegWit");
            }
            if has_taproot {
                features.push("Taproot");
            }
            if is_rbf {
                features.push("RBF");
            }

            Ok(Json(serde_json::json!({
                "txid": txid,
                "inputs": input_count,
                "outputs": output_count,
                "size": size,
                "vsize": vsize,
                "weight": weight,
                "fee": fee_sats,
                "fee_rate": (fee_rate * 10.0).round() / 10.0,
                "total_output": total_output_sats,
                "confirmed": confirmations > 0,
                "confirmations": confirmations,
                "block_hash": block_hash,
                "block_height": block_height,
                "features": features,
            })))
        }
        Err(_) => {
            Err((axum::http::StatusCode::NOT_FOUND, "Transaction not found"))
        }
    }
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

    // Load unconfirmed txs for the current flatline + recent notable txs (including confirmed)
    // for the whale watch feed. mempool_txs only holds unconfirmed; notable_txs holds both.
    let (history, notable_history, last_block_ts) = {
        let two_hours_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(7200);
        let twenty_four_hours_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(86400);
        match state.db.get() {
            Ok(conn) => {
                let block_ts = db::max_height(&conn)
                    .ok()
                    .flatten()
                    .and_then(|h| {
                        db::query_block_timestamp(&conn, h).ok().flatten()
                    })
                    .unwrap_or(0);
                let txs =
                    db::query_recent_mempool_txs(&conn, two_hours_ago, 10000)
                        .unwrap_or_default();
                // Fetch recent notable txs (last 24h, up to 200) for the feed panel.
                // Includes confirmed txs, unlike mempool_txs query.
                let filter = db::NotableFilter {
                    since: Some(twenty_four_hours_ago),
                    ..Default::default()
                };
                let notables = db::query_notable_txs(&conn, &filter, 200, 0)
                    .unwrap_or_default();
                (txs, notables, block_ts)
            }
            Err(_) => (vec![], vec![], 0),
        }
    };

    // State machine: history → notable history → live events
    enum Phase {
        History(Vec<db::MempoolTxRow>, Vec<db::NotableTx>, u64),
        NotableHistory(Vec<db::NotableTx>),
        Live,
    }

    let stream = futures::stream::unfold(
        (Phase::History(history, notable_history, last_block_ts), rx),
        |(phase, mut rx)| async move {
            match phase {
                Phase::History(txs, notables, block_ts) => {
                    let data = if txs.is_empty() {
                        format!("{{\"txs\":[],\"last_block_ts\":{block_ts}}}")
                    } else {
                        let txs_json =
                            serde_json::to_string(&txs).unwrap_or_default();
                        format!("{{\"txs\":{txs_json},\"last_block_ts\":{block_ts}}}")
                    };
                    Some((
                        Ok(Event::default().event("history").data(data)),
                        (Phase::NotableHistory(notables), rx),
                    ))
                }
                Phase::NotableHistory(notables) => {
                    let data = serde_json::to_string(&notables)
                        .unwrap_or_else(|_| "[]".to_string());
                    Some((
                        Ok(Event::default()
                            .event("notable_history")
                            .data(data)),
                        (Phase::Live, rx),
                    ))
                }
                Phase::Live => match rx.recv().await {
                    Ok(event) => {
                        let event_type = match &event {
                            super::zmq_subscriber::HeartbeatEvent::Tx { .. } => "tx",
                            super::zmq_subscriber::HeartbeatEvent::Block { .. } => "block",
                            super::zmq_subscriber::HeartbeatEvent::BlockMining => "block_mining",
                        };
                        let data =
                            serde_json::to_string(&event).unwrap_or_default();
                        Some((
                            Ok(Event::default().event(event_type).data(data)),
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
                },
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
