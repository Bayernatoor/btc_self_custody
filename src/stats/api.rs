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
//! separate handler-level cache for `/live`: the RPC cache handles it.
//! External HTTP price data (mempool.space) retains its own 60s cache with an
//! atomic guard against concurrent refreshes.
//!
//! ## SSE Connection Limiting
//!
//! The heartbeat SSE endpoint is capped at 256 concurrent connections via an
//! `AtomicUsize` counter with an RAII guard that decrements on drop.

use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::broadcast;

use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::stream::Stream;
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

use super::cache::{Cache, CacheCell, CacheRegistry, CacheTag};
use super::db::{self, DbPool};
use super::error::StatsError;
use super::rpc::{BitcoinRpc, PriceInfo};
use super::types::PricePoint;

/// Shared application state for the stats module. Holds the DB pool,
/// RPC client, and all in-memory caches. Wrapped in `Arc` and passed
/// to all handlers.
///
/// All caches go through `Cache<K, V>` (see `super::cache`) and are
/// constructed via `StatsStateBuilder` (below). Trigger sites for
/// invalidation (block poller, ZMQ `hashblock` handler) call
/// `state.invalidate(tag)`; the registry fans out to every cache
/// subscribed to that tag. Adding a new cache is one line and the
/// invalidation wiring cannot be forgotten.
pub struct StatsState {
    pub db: DbPool,
    pub rpc: BitcoinRpc,
    /// Routes `state.invalidate(tag)` to subscribed caches. Constructed
    /// via `StatsStateBuilder::into_registry()` so every cache built
    /// through the builder is wired in automatically.
    pub cache_registry: CacheRegistry,
    /// Latest BTC/USD price. 60s TTL. Background task at startup.rs
    /// refreshes every 90s; no invalidation tag because the refresh
    /// task owns the freshness contract. Singleflight is handled by
    /// `Cache::get_or_compute` so concurrent readers on cold cache
    /// share one upstream HTTP request.
    pub price_cache: Arc<Cache<(), PriceInfo>>,
    /// UTXO set size. Background task at startup.rs overwrites every
    /// 10 minutes; readers do not refresh on miss (return 0 instead).
    /// `permanent` because TTL doesn't apply, the background task is
    /// the freshness mechanism.
    pub utxo_count: Arc<Cache<(), u64>>,
    /// Stats summary cache. Invalidated on new block via the registry.
    pub stats_summary_cache: Arc<Cache<(), super::types::StatsSummary>>,
    /// Daily aggregate cache, keyed by (from_ts, to_ts). 120s TTL.
    /// Invalidated on new block (today's bucket may shift).
    pub daily_cache: Arc<Cache<(u64, u64), Vec<super::types::DailyAggregate>>>,
    /// Block height -> timestamp cache. Immutable data so no TTL or
    /// invalidation; entries are written once and live for the process.
    pub block_ts_cache: Arc<Cache<u64, u64>>,
    /// Signaling per-block cache, keyed by `"bit:N:from:to"` or
    /// `"locktime:from:to"`. 60s TTL, no invalidation tag (preserves
    /// pre-existing TTL-only behavior; the cache key already changes
    /// when from/to advance).
    pub signaling_blocks_cache: Arc<
        Cache<
            String,
            (Vec<super::types::SignalingBlock>, super::types::PeriodStats),
        >,
    >,
    /// Signaling-periods cache, keyed by `"bit:N"` or `"locktime"`.
    /// 60s TTL, no invalidation tag (TTL-only, matches pre-existing).
    pub signaling_periods_cache:
        Arc<Cache<String, Vec<super::db::SignalingPeriod>>>,
    /// Full price-history dataset (singleton). 1h TTL, no invalidation
    /// tag. Pre-existing code keyed by `(0, u64::MAX)` and ignored
    /// the key on read; collapsed to `()` here.
    pub price_history_cache: Arc<Cache<(), Vec<PricePoint>>>,
    /// Range summary cache, keyed by (from_ts, to_ts). 60s TTL.
    /// Invalidated on new block.
    pub range_summary_cache: Arc<Cache<(u64, u64), super::types::RangeSummary>>,
    /// The two histogram endpoints, keyed by (from_ts, to_ts).
    ///
    /// These were the only range-keyed endpoints without a cache, and the
    /// time histogram is the most expensive query the site runs: a `LAG`
    /// window function over every block in the window, which cannot use an
    /// index for its ordering. Measured against the 968,041-row table it
    /// takes **1.53 s at the ALL range** and 0.42 s over five years, on a
    /// desktop; the droplet has 2 vCPU. The answer is identical for every
    /// visitor until the next block, so it was being recomputed per request
    /// for nothing.
    pub time_histogram_cache:
        Arc<Cache<(u64, u64), Vec<super::types::HistogramBucket>>>,
    pub fullness_histogram_cache:
        Arc<Cache<(u64, u64), Vec<super::types::HistogramBucket>>>,
    /// Extremes cache, keyed by (from_ts, to_ts). 60s TTL.
    /// Invalidated on new block.
    pub extremes_cache: Arc<Cache<(u64, u64), super::types::ExtremesData>>,
    /// Broadcast channel for real-time heartbeat events (ZMQ → SSE).
    pub heartbeat_tx: broadcast::Sender<super::zmq_subscriber::HeartbeatEvent>,
    /// Authoritative in-memory state owner for Heartbeat Phase 2 sync.
    pub heartbeat_store: Arc<super::heartbeat_state::HeartbeatStore>,
    pub(crate) heartbeat_blocks: super::zmq_subscriber::BlockJobs,
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

    /// Construct a cache, register it under each tag in `tags` (and in
    /// the flat inventory used for stats), then return the `Arc` for
    /// the caller to store on `StatsState`. `name` is a stable string
    /// surfaced in `/api/stats/cache-stats`.
    pub fn cache<K, V>(
        &mut self,
        name: &'static str,
        ttl: Duration,
        tags: &[CacheTag],
    ) -> Arc<Cache<K, V>>
    where
        K: Eq + std::hash::Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        self.cache_with_capacity(name, ttl, tags, None)
    }

    /// As [`cache`](Self::cache), with an explicit entry cap. Needed for
    /// caches whose working set is legitimately large (`block_timestamps`
    /// holds one entry per height by design), since the default cap is sized
    /// for the range-keyed caches it exists to bound.
    pub fn cache_with_capacity<K, V>(
        &mut self,
        name: &'static str,
        ttl: Duration,
        tags: &[CacheTag],
        capacity: Option<usize>,
    ) -> Arc<Cache<K, V>>
    where
        K: Eq + std::hash::Hash + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let mut cache = Cache::new(name, ttl);
        if let Some(capacity) = capacity {
            cache = cache.with_capacity(capacity);
        }
        for &tag in tags {
            cache = cache.invalidated_by(tag);
        }
        let arc = Arc::new(cache);
        let cell: Arc<dyn CacheCell> = arc.clone();
        self.registry.register(cell.clone());
        for &tag in tags {
            self.registry.subscribe(tag, cell.clone());
        }
        arc
    }

    /// Consume the builder and return the populated registry.
    pub fn into_registry(self) -> CacheRegistry {
        self.registry
    }
}

/// Maximum concurrent SSE connections before rejecting new ones.
pub(crate) const MAX_SSE_CONNECTIONS: usize = 256;

/// Largest height span `/op-returns` will serve. Matches its own default
/// window, so it clamps only explicitly over-wide requests.
const MAX_OP_RETURN_SPAN: u64 = 10_000;

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
        Some(b) => {
            let mut value = serde_json::to_value(b)
                .map_err(|e| StatsError::Rpc(e.to_string()))?;
            let landmarks: Vec<serde_json::Value> =
                super::hall_of_fame_data::HALL_OF_FAME
                    .iter()
                    .filter(|e| e.block == Some(height))
                    .map(|e| {
                        serde_json::json!({
                            "slug": e.slug,
                            "title": e.title,
                            "description": e.description,
                            "category": e.category.label(),
                            "color": e.category.color(),
                            "date": e.date,
                            "txid": e.txid,
                        })
                    })
                    .collect();
            if !landmarks.is_empty() {
                if let Some(obj) = value.as_object_mut() {
                    obj.insert(
                        "landmarks".to_string(),
                        serde_json::Value::Array(landmarks),
                    );
                }
            }
            Ok(Json(value))
        }
        // A 200 carrying {"error": ...} is worse than either a 200 or a 404:
        // every caller has to inspect the body to learn it failed.
        None => Err(StatsError::NotFound("Block not found".into())),
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
    // Application-level caches (Cache<K, V> in cache.rs). Reports the
    // entry count and hit/miss counters captured by the primitive.
    // Capacity is omitted because B has no bound; A will add a
    // capacity field once bounded LRU lands.
    let application: Vec<serde_json::Value> = state
        .cache_registry
        .all_stats()
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "name": s.name,
                "size": s.size,
                "hits": s.hits,
                "misses": s.misses,
                "refreshes": s.refreshes,
                "hit_rate": (s.hit_rate() * 1000.0).round() / 1000.0,
            })
        })
        .collect();
    // 0s cache on this endpoint: operators need to see current state
    Ok(cached_json(
        serde_json::json!({
            "slots": slots,
            "blocks": blocks,
            "application": application,
        }),
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
    let (br, mr, hr, fr, hfr) = tokio::join!(
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
        async {
            let s = Instant::now();
            let r = state.rpc.estimate_smart_fee(6).await;
            (r, s.elapsed())
        },
    );
    let (blockchain_res, t_blockchain) = br;
    let (mempool_res, t_mempool) = mr;
    let (hashrate_res, t_hashrate) = hr;
    let (fee_res, t_fee) = fr;
    let (hour_fee_res, _t_hour_fee) = hfr;

    // Feed the latest aggregate mempool reading into Heartbeat v2 state before
    // bubbling up any RPC error so both fresh readings and outages propagate.
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    match &mempool_res {
        Ok((info, m_stale)) => {
            let observed_at = now_secs.saturating_sub(
                state.rpc.mempool_cache_age_secs().unwrap_or(0),
            );
            state.heartbeat_store.update_node_mempool(Some(
                super::heartbeat_sync::NodeMempoolAggregate {
                    tx_count: info.size,
                    vbytes: info.bytes,
                    observed_at,
                    stale: *m_stale,
                },
            ));
        }
        Err(_) => {
            state.heartbeat_store.update_node_mempool(None);
        }
    }

    // Track staleness across the four RPCs. Any OR-combines to response-level.
    let mut stale = false;

    let (blockchain, b_stale) = blockchain_res?;
    stale |= b_stale;
    let (mempool, m_stale) = mempool_res?;
    stale |= m_stale;
    // A hard RPC failure means the value below is a zero placeholder, not a
    // reading, so it must mark the response stale. Returning `false` here
    // claimed the zeros were current, while the server-fn twin
    // (`fetch_live_stats`) sets `stale = true` on the same path. The client
    // renders a "data may be outdated" banner off this flag, so the two
    // entry points disagreed about whether to warn the user.
    let (hashrate, h_stale) = hashrate_res.unwrap_or_else(|e| {
        tracing::warn!("Failed to fetch hashrate: {e}");
        (0.0, true)
    });
    stale |= h_stale;
    let (next_block_fee, f_stale) = fee_res.unwrap_or_else(|e| {
        tracing::warn!("Failed to fetch fee estimate: {e}");
        (0.0, true)
    });
    stale |= f_stale;
    let (hour_fee, hf_stale) = hour_fee_res.unwrap_or_else(|e| {
        tracing::warn!("Failed to fetch 6-block fee estimate: {e}");
        (0.0, true)
    });
    stale |= hf_stale;

    // Price cache: 60s TTL + per-key singleflight inside Cache.
    // Concurrent missers all wait on the same fetch, so only one HTTP
    // call to mempool.space goes out per stale window. The 90s
    // background task refreshes independently. On fetch failure we
    // surface 0.0 (caller treats price=0 as unavailable); the cache
    // has already TTL-evicted, so no stale fallback. Mempool.space
    // outages are rare and the next background refresh recovers
    // within 90s.
    let t_price_start = Instant::now();
    let cache_was_warm = state.price_cache.get(&()).is_some();
    let state_for_price = Arc::clone(&state);
    let price_usd = state
        .price_cache
        .clone()
        .get_or_compute((), move || async move {
            state_for_price.rpc.fetch_price().await
        })
        .await
        .map(|p| p.usd)
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to fetch price: {e}");
            0.0
        });
    let price_did_fetch = !cache_was_warm;
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

    let utxo_count = state.utxo_count.get(&()).unwrap_or(0);

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
            "hour_fee": hour_fee,
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

    // Caller-controlled range on a public endpoint: bound it. The default
    // window is 10k blocks, so this only clamps explicit over-wide requests.
    if to.saturating_sub(from) > MAX_OP_RETURN_SPAN {
        return Err(StatsError::BadRequest(format!(
            "Block range too large: max {MAX_OP_RETURN_SPAN} blocks"
        )));
    }
    let blocks = db::query_op_returns(&conn, from, to)?;
    Ok(Json(serde_json::json!({ "blocks": blocks })))
}

/// GET /api/aggregates/daily - daily aggregated metrics by timestamp range.
pub async fn get_daily_aggregates(
    State(state): State<SharedStatsState>,
    Query(params): Query<TimestampQuery>,
) -> Result<CachedResponse, StatsError> {
    let from_ts = params.from.unwrap_or(0);
    let to_ts = params.to.unwrap_or(u64::MAX);
    if from_ts > to_ts {
        return Err(StatsError::BadRequest("Invalid timestamp range".into()));
    }

    // Read the pre-computed rollup and share the 120s cache with the server-fn
    // twin. This handler was left on `query_daily_aggregates`, the raw
    // GROUP BY, when the other side was migrated: with `to` defaulting to
    // u64::MAX it scanned all 965k rows on every uncached public request.
    let days = state
        .daily_cache
        .clone()
        .get_or_compute((from_ts, to_ts), || async move {
            let conn = state
                .db
                .get()
                .map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;
            let rows = db::query_daily_aggregates_fast(&conn, from_ts, to_ts)?;
            Ok::<_, StatsError>(
                rows.into_iter()
                    .map(super::types::DailyAggregate::from)
                    .collect(),
            )
        })
        .await?;
    Ok(cached_json(serde_json::json!({ "days": days }), 60))
}

/// GET /api/signaling - per-block signaling status (version bits or BIP-54 locktime).
/// Also returns period stats for the current 2016-block retarget window.
pub async fn get_signaling(
    State(state): State<SharedStatsState>,
    Query(params): Query<SignalingQuery>,
) -> Result<Json<serde_json::Value>, StatsError> {
    let bit = params.bit.unwrap_or(0);
    if super::types::version_bit_out_of_range(bit) {
        return Err(StatsError::BadRequest(format!(
            "bit must be 0..={}",
            super::types::MAX_VERSION_BIT
        )));
    }
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
        db::query_signaling_bit(&conn, bit, from, to)?
    };

    // Period stats: retarget block (period_start) is the boundary between periods.
    // "Blocks since adjustment" starts at period_start (inclusive), matching mempool.space.
    let period_start = (to / 2016) * 2016;
    let period_end = period_start + 2015;
    // Query from period_start but report mined as tip - period_start (excluding the retarget block)
    let period_blocks = if use_locktime {
        db::query_signaling_locktime(&conn, period_start, period_end)?
    } else {
        db::query_signaling_bit(&conn, bit, period_start, period_end)?
    };
    // The whole 2,016-block period is the population for both halves. See the
    // twin of this block in `server_fns.rs` for why dropping the retarget
    // block from either half is wrong.
    let signaled_count =
        period_blocks.iter().filter(|b| b.signaled).count() as u64;
    let mined = period_blocks.len() as u64;
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
    // Validate before taking a pooled connection, as get_signaling does: a
    // rejected request should not occupy one of the 16 slots to be told no.
    let bit = params.bit.unwrap_or(0);
    if super::types::version_bit_out_of_range(bit) {
        return Err(StatsError::BadRequest(format!(
            "bit must be 0..={}",
            super::types::MAX_VERSION_BIT
        )));
    }
    let conn = state
        .db
        .get()
        .map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;
    let use_locktime = params.method.as_deref() == Some("locktime");
    let periods = if use_locktime {
        db::query_signaling_periods_locktime(&conn)?
    } else {
        db::query_signaling_periods_bit(&conn, bit)?
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
            let block_time =
                tx["blocktime"].as_u64().or_else(|| tx["time"].as_u64());

            // Authoritative first-seen observation timestamp from our live mirror
            // (zero-cost O(log n) in-memory lookup when unconfirmed).
            let first_seen = match state.heartbeat_store.lookup_tx(&txid) {
                super::heartbeat_sync::TxLookupStatus::Unconfirmed(m)
                    if m.first_seen > 0 =>
                {
                    Some(m.first_seen)
                }
                _ => None,
            };

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

            // Cap preview lists to 25 entries each so 500-input consolidations or
            // 200-output fan-outs stay fast and lightweight on the droplet and UI.
            const MAX_IO_PREVIEW: usize = 25;
            // Returns `(display_label, copy_value)` where `copy_value` is `Some`
            // only for concrete addresses (never generic script types).
            let script_label =
                |spk: &serde_json::Value| -> Option<(String, Option<String>)> {
                    if let Some(addr) = spk["address"].as_str() {
                        if !addr.is_empty() {
                            return Some((
                                addr.to_string(),
                                Some(addr.to_string()),
                            ));
                        }
                    }
                    let kind = match spk["type"].as_str()? {
                        "nulldata" => "OP_RETURN",
                        "witness_v1_taproot" => "Taproot",
                        "witness_v0_keyhash" => "SegWit",
                        "witness_v0_scripthash" => "SegWit script",
                        "scripthash" => "P2SH",
                        "pubkeyhash" => "P2PKH",
                        "pubkey" => "P2PK",
                        "anchor" => "Anchor",
                        other => other,
                    };
                    Some((kind.to_string(), None))
                };
            let input_items: Vec<serde_json::Value> = vin
                .map(|inputs| {
                    inputs
                        .iter()
                        .take(MAX_IO_PREVIEW)
                        .map(|i| {
                            if !i["coinbase"].is_null() {
                                return serde_json::json!({
                                    "label": "Coinbase",
                                    "value": serde_json::Value::Null,
                                    "copy": serde_json::Value::Null,
                                });
                            }
                            let (label, copy) =
                                script_label(&i["prevout"]["scriptPubKey"])
                                    .or_else(|| {
                                        let prev_tx = i["txid"].as_str()?;
                                        let prev_vout = i["vout"].as_u64()?;
                                        let prefix =
                                            prev_tx.get(..8).unwrap_or(prev_tx);
                                        Some((
                                            format!(
                                                "{prefix}\u{2026}:{prev_vout}"
                                            ),
                                            Some(format!(
                                                "{prev_tx}:{prev_vout}"
                                            )),
                                        ))
                                    })
                                    .unwrap_or_else(|| {
                                        ("Input".to_string(), None)
                                    });
                            let val_sats = i["prevout"]["value"]
                                .as_f64()
                                .map(|v| (v * 100_000_000.0).round() as u64);
                            serde_json::json!({
                                "label": label,
                                "value": val_sats,
                                "copy": copy,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let output_items: Vec<serde_json::Value> = vout
                .map(|outputs| {
                    outputs
                        .iter()
                        .take(MAX_IO_PREVIEW)
                        .map(|o| {
                            let (label, copy) =
                                script_label(&o["scriptPubKey"])
                                    .unwrap_or_else(|| {
                                        ("Output".to_string(), None)
                                    });
                            let val_sats = o["value"]
                                .as_f64()
                                .map(|v| (v * 100_000_000.0).round() as u64);
                            serde_json::json!({
                                "label": label,
                                "value": val_sats,
                                "copy": copy,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            Ok(Json(serde_json::json!({
                "txid": txid,
                "inputs": input_count,
                "outputs": output_count,
                "input_items": input_items,
                "output_items": output_items,
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
                "block_time": block_time,
                "first_seen": first_seen,
                "features": features,
            })))
        }
        Err(_) => {
            Err((axum::http::StatusCode::NOT_FOUND, "Transaction not found"))
        }
    }
}

/// Query parameters for `GET /api/stats/heartbeat/v2`.
#[derive(Deserialize)]
pub struct HeartbeatV2Query {
    pub epoch: Option<String>,
    pub after: Option<String>,
}

/// Resolve `(epoch, after)` preferring the `Last-Event-ID: <e>:<seq>` header
/// when present and non-empty (so EventSource auto-reconnects advance past the
/// initial URL cursor), falling back to `?epoch=<e>&after=<seq>` query params.
fn parse_heartbeat_v2_cursor(
    params: &HeartbeatV2Query,
    headers: &axum::http::HeaderMap,
) -> Result<(String, u64), (axum::http::StatusCode, &'static str)> {
    if let Some(raw_header) = headers.get("last-event-id") {
        let header_val = raw_header.to_str().map_err(|_| {
            (
                axum::http::StatusCode::BAD_REQUEST,
                "invalid Last-Event-ID header",
            )
        })?;
        if !header_val.is_empty() {
            let (epoch, seq_str) = header_val.rsplit_once(':').ok_or((
                axum::http::StatusCode::BAD_REQUEST,
                "invalid Last-Event-ID format; expected <epoch>:<seq>",
            ))?;
            if epoch.is_empty() {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    "invalid Last-Event-ID: empty epoch",
                ));
            }
            let after = seq_str.parse::<u64>().map_err(|_| {
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    "invalid Last-Event-ID: seq must be u64",
                )
            })?;
            return Ok((epoch.to_string(), after));
        }
    }

    match (&params.epoch, &params.after) {
        (Some(epoch), Some(after_str)) => {
            if epoch.is_empty() {
                return Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    "epoch must not be empty",
                ));
            }
            let after = after_str.parse::<u64>().map_err(|_| {
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    "after must be an unsigned 64-bit integer",
                )
            })?;
            Ok((epoch.clone(), after))
        }
        (Some(_), None) | (None, Some(_)) => Err((
            axum::http::StatusCode::BAD_REQUEST,
            "both epoch and after query parameters are required",
        )),
        (None, None) => Err((
            axum::http::StatusCode::BAD_REQUEST,
            "missing epoch/after query parameters or Last-Event-ID header",
        )),
    }
}

/// GET /api/stats/heartbeat/snapshot - shared immutable JSON snapshot of the
/// Heartbeat v2 state mirror.
pub async fn get_heartbeat_snapshot(
    State(state): State<SharedStatsState>,
) -> impl axum::response::IntoResponse {
    let snap = state.heartbeat_store.snapshot_async().await;
    (
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (header::CACHE_CONTROL, "no-store".to_string()),
            (
                header::HeaderName::from_static("x-heartbeat-epoch"),
                snap.stream_epoch.clone(),
            ),
            (
                header::HeaderName::from_static("x-heartbeat-seq"),
                snap.seq.to_string(),
            ),
        ],
        snap.json.clone(),
    )
}

/// GET /api/stats/heartbeat/v2 - sequenced Heartbeat v2 SSE delta stream with
/// atomic replay-to-live handoff and explicit `resync_required` signalling.
///
/// Browser client contract on `resync_required`: immediately call `es.close()`,
/// fetch `/api/stats/heartbeat/snapshot`, install the snapshot, and open a new
/// `EventSource` at the snapshot's `(stream_epoch, seq)`. Never allow
/// `EventSource` auto-reconnect to reuse a stale `Last-Event-ID` after the
/// server terminates the stream with `resync_required`.
pub async fn get_heartbeat_v2_sse(
    State(state): State<SharedStatsState>,
    Query(params): Query<HeartbeatV2Query>,
    headers: axum::http::HeaderMap,
) -> Result<
    (
        [(header::HeaderName, String); 2],
        Sse<impl Stream<Item = Result<Event, Infallible>>>,
    ),
    (axum::http::StatusCode, &'static str),
> {
    let (epoch, after) = parse_heartbeat_v2_cursor(&params, &headers)?;

    // Share the global SSE connection budget with the legacy stream.
    let prev = state.sse_connections.fetch_add(1, Ordering::Relaxed);
    if prev >= MAX_SSE_CONNECTIONS {
        state.sse_connections.fetch_sub(1, Ordering::Relaxed);
        return Err((
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Too many connections",
        ));
    }
    let guard = SseConnectionGuard(Arc::clone(&state));

    let sub = state
        .heartbeat_store
        .subscribe_from(Some(&epoch), Some(after));

    enum V2Phase {
        Replay {
            replay:
                std::vec::IntoIter<Arc<super::heartbeat_sync::HeartbeatDelta>>,
            live_rx:
                broadcast::Receiver<Arc<super::heartbeat_state::StoreEvent>>,
            last_replayed: u64,
        },
        Live {
            live_rx:
                broadcast::Receiver<Arc<super::heartbeat_state::StoreEvent>>,
            last_replayed: u64,
        },
        Resync(Option<super::heartbeat_sync::ResyncRequired>),
        Done,
    }

    let initial_phase = match sub {
        super::heartbeat_state::SubscribeResult::Stream {
            replay,
            live_rx,
            last_replayed,
        } => {
            if replay.is_empty() {
                V2Phase::Live {
                    live_rx,
                    last_replayed,
                }
            } else {
                V2Phase::Replay {
                    replay: replay.into_iter(),
                    live_rx,
                    last_replayed,
                }
            }
        }
        super::heartbeat_state::SubscribeResult::Resync(resync) => {
            V2Phase::Resync(Some(resync))
        }
    };

    let stream = futures::stream::unfold(
        (initial_phase, guard, Arc::clone(&state), epoch),
        |(mut phase, guard, state, expected_epoch)| async move {
            loop {
                match phase {
                    V2Phase::Resync(Some(resync)) => {
                        let data = serde_json::to_string(&resync)
                            .unwrap_or_else(|_| "{}".to_string());
                        return Some((
                            Ok(Event::default()
                                .event("resync_required")
                                .data(data)),
                            (V2Phase::Done, guard, state, expected_epoch),
                        ));
                    }
                    V2Phase::Resync(None) | V2Phase::Done => {
                        drop(guard);
                        return None;
                    }
                    V2Phase::Replay {
                        mut replay,
                        live_rx,
                        last_replayed,
                    } => {
                        if let Some(delta) = replay.next() {
                            let id =
                                format!("{}:{}", delta.stream_epoch, delta.seq);
                            let data = serde_json::to_string(delta.as_ref())
                                .unwrap_or_default();
                            return Some((
                                Ok(Event::default()
                                    .event("delta")
                                    .id(id)
                                    .data(data)),
                                (
                                    V2Phase::Replay {
                                        replay,
                                        live_rx,
                                        last_replayed,
                                    },
                                    guard,
                                    state,
                                    expected_epoch,
                                ),
                            ));
                        }
                        phase = V2Phase::Live {
                            live_rx,
                            last_replayed,
                        };
                    }
                    V2Phase::Live {
                        mut live_rx,
                        mut last_replayed,
                    } => match live_rx.recv().await {
                        Ok(event) => match event.as_ref() {
                            super::heartbeat_state::StoreEvent::Delta(
                                delta,
                            ) => {
                                if delta.stream_epoch != expected_epoch {
                                    let (stream_epoch, latest_seq) = state
                                        .heartbeat_store
                                        .current_revision();
                                    let resync =
                                        super::heartbeat_sync::ResyncRequired {
                                            protocol_version:
                                                super::heartbeat_sync::PROTOCOL_VERSION,
                                            stream_epoch,
                                            reason: "epoch_mismatch"
                                                .to_string(),
                                            latest_seq,
                                        };
                                    let data = serde_json::to_string(&resync)
                                        .unwrap_or_else(|_| "{}".to_string());
                                    return Some((
                                        Ok(Event::default()
                                            .event("resync_required")
                                            .data(data)),
                                        (
                                            V2Phase::Done,
                                            guard,
                                            state,
                                            expected_epoch,
                                        ),
                                    ));
                                }
                                if delta.seq <= last_replayed {
                                    phase = V2Phase::Live {
                                        live_rx,
                                        last_replayed,
                                    };
                                    continue;
                                }
                                last_replayed = delta.seq;
                                let id = format!(
                                    "{}:{}",
                                    delta.stream_epoch, delta.seq
                                );
                                let data =
                                    serde_json::to_string(delta.as_ref())
                                        .unwrap_or_default();
                                return Some((
                                    Ok(Event::default()
                                        .event("delta")
                                        .id(id)
                                        .data(data)),
                                    (
                                        V2Phase::Live {
                                            live_rx,
                                            last_replayed,
                                        },
                                        guard,
                                        state,
                                        expected_epoch,
                                    ),
                                ));
                            }
                            super::heartbeat_state::StoreEvent::Resync(
                                resync,
                            ) => {
                                let data =
                                    serde_json::to_string(resync.as_ref())
                                        .unwrap_or_else(|_| "{}".to_string());
                                return Some((
                                    Ok(Event::default()
                                        .event("resync_required")
                                        .data(data)),
                                    (
                                        V2Phase::Done,
                                        guard,
                                        state,
                                        expected_epoch,
                                    ),
                                ));
                            }
                        },
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            let (stream_epoch, latest_seq) =
                                state.heartbeat_store.current_revision();
                            let resync =
                                super::heartbeat_sync::ResyncRequired {
                                    protocol_version:
                                        super::heartbeat_sync::PROTOCOL_VERSION,
                                    stream_epoch,
                                    reason: "stream_lagged".to_string(),
                                    latest_seq,
                                };
                            let data = serde_json::to_string(&resync)
                                .unwrap_or_else(|_| "{}".to_string());
                            return Some((
                                Ok(Event::default()
                                    .event("resync_required")
                                    .data(data)),
                                (V2Phase::Done, guard, state, expected_epoch),
                            ));
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            drop(guard);
                            return None;
                        }
                    },
                }
            }
        },
    );

    Ok((
        [
            (
                header::HeaderName::from_static("x-accel-buffering"),
                "no".to_string(),
            ),
            (header::CACHE_CONTROL, "no-cache".to_string()),
        ],
        Sse::new(stream)
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15))),
    ))
}

/// GET /api/stats/heartbeat/tx/{txid} - lightweight server-assisted lookup
/// against the live Heartbeat mirror and retained confirmation cache.
pub async fn get_heartbeat_tx_lookup(
    State(state): State<SharedStatsState>,
    Path(txid): Path<String>,
) -> Result<
    (
        [(header::HeaderName, &'static str); 1],
        Json<super::heartbeat_sync::TxLookupStatus>,
    ),
    (axum::http::StatusCode, &'static str),
> {
    if txid.len() != 64 || !txid.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err((axum::http::StatusCode::BAD_REQUEST, "Invalid txid"));
    }
    let normalized = txid.to_ascii_lowercase();
    let status = state.heartbeat_store.lookup_tx(&normalized);
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(status)))
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatBlockWindowParams {
    pub radius: Option<u64>,
}

#[derive(Debug, serde::Serialize)]
pub struct HeartbeatBlockWindowResponse {
    pub target_height: u64,
    pub target_hash: String,
    pub tip_height: u64,
    pub blocks: Vec<db::HeartbeatBlockWindowRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HeartbeatBlockQuery {
    Height(u64),
    Hash(String),
}

/// Parse a decimal block height (1..=10 ASCII digits), accepting either plain
/// digits or standard thousands-grouped digits (e.g. `840,000`) and rejecting
/// malformed comma placement (e.g. `8,4,0000`, `,840`, `0,000`).
fn parse_formatted_block_height(raw: &str) -> Option<u64> {
    if raw.is_empty() {
        return None;
    }
    let digits = if raw.contains(',') {
        let mut parts = raw.split(',');
        let first = parts.next()?;
        if !(1..=3).contains(&first.len())
            || !first.bytes().all(|b| b.is_ascii_digit())
            || first.starts_with('0')
        {
            return None;
        }
        let mut out = String::with_capacity(raw.len());
        out.push_str(first);
        for group in parts {
            if group.len() != 3 || !group.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            out.push_str(group);
        }
        out
    } else {
        if !raw.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        raw.to_string()
    };
    if !(1..=10).contains(&digits.len()) {
        return None;
    }
    digits.parse::<u64>().ok()
}

/// Deep historical block windows/ranges (>= 6 confirmations below tip) are
/// effectively immutable and safe to cache for an hour; near-tip or empty
/// responses must remain `no-store`.
fn heartbeat_history_cache_control(
    blocks: &[db::HeartbeatBlockWindowRow],
    tip_height: u64,
) -> &'static str {
    let max_returned = blocks.last().map(|b| b.height).unwrap_or(0);
    if !blocks.is_empty() && tip_height >= 6 && max_returned <= tip_height - 6 {
        "public, max-age=3600"
    } else {
        "no-store"
    }
}

/// Parse a block lookup path parameter: trims whitespace, strips an optional
/// leading `#` or case-insensitive `block:` prefix (UTF-8 safe), validates
/// optional thousands separators, and classifies either a 1..=10-digit height
/// or a 64-character hex block hash.
fn parse_heartbeat_block_query(raw: &str) -> Option<HeartbeatBlockQuery> {
    let mut q = raw.trim();
    if let Some(rest) = q.strip_prefix('#') {
        q = rest.trim();
    } else if q
        .get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("block:"))
    {
        q = q.get(6..)?.trim();
        if let Some(rest) = q.strip_prefix('#') {
            q = rest.trim();
        }
    }

    if let Some(h) = parse_formatted_block_height(q) {
        return Some(HeartbeatBlockQuery::Height(h));
    }

    if q.len() == 64 && q.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Some(HeartbeatBlockQuery::Hash(q.to_ascii_lowercase()));
    }

    None
}

/// GET /api/stats/heartbeat/block/{query} - centered historical block window
/// for on-demand Heartbeat spike rendering, resolved by height or block hash.
pub async fn get_heartbeat_block_window(
    State(state): State<SharedStatsState>,
    Path(raw_query): Path<String>,
    Query(params): Query<HeartbeatBlockWindowParams>,
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    let log_q: String = raw_query.chars().take(72).collect();

    let Some(parsed) = parse_heartbeat_block_query(&raw_query) else {
        tracing::warn!(
            query = ?log_q,
            "Rejected malformed heartbeat block window query"
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Expected a block height or 64-character hex block hash"
            })),
        )
            .into_response();
    };

    let radius = params.radius.unwrap_or(20).clamp(5, 50);
    let pool = state.db.clone();

    let result = tokio::task::spawn_blocking(
        move || -> Result<Option<HeartbeatBlockWindowResponse>, StatsError> {
            let conn = pool
                .get()
                .map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;

            let target_height = match parsed {
                HeartbeatBlockQuery::Height(h) => h,
                HeartbeatBlockQuery::Hash(ref hash) => {
                    match db::query_block_height_by_hash(&conn, hash)? {
                        Some(h) => h,
                        None => return Ok(None),
                    }
                }
            };

            let blocks =
                db::query_heartbeat_block_window(&conn, target_height, radius)?;
            let Some(target_row) =
                blocks.iter().find(|b| b.height == target_height)
            else {
                return Ok(None);
            };
            let target_hash = target_row.hash.clone();
            let tip_height = db::max_height(&conn)?.unwrap_or(target_height);

            Ok(Some(HeartbeatBlockWindowResponse {
                target_height,
                target_hash,
                tip_height,
                blocks,
            }))
        },
    )
    .await;

    match result {
        Ok(Ok(Some(response))) => {
            let cache_control = heartbeat_history_cache_control(
                &response.blocks,
                response.tip_height,
            );
            (
                StatusCode::OK,
                [(header::CACHE_CONTROL, cache_control)],
                Json(response),
            )
                .into_response()
        }
        Ok(Ok(None)) => {
            tracing::warn!(
                query = ?log_q,
                "Heartbeat block window target not found"
            );
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Block not found" })),
            )
                .into_response()
        }
        Ok(Err(err)) => {
            tracing::error!(
                query = ?log_q,
                "Heartbeat block window lookup failed: {err}"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal server error" })),
            )
                .into_response()
        }
        Err(err) => {
            tracing::error!(
                query = ?log_q,
                "Heartbeat block window task failed: {err}"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal server error" })),
            )
                .into_response()
        }
    }
}

/// Maximum number of blocks returned per page by `GET /api/stats/heartbeat/blocks`.
const MAX_HEARTBEAT_BLOCK_RANGE: u64 = 500;

#[derive(Debug, Deserialize)]
pub struct HeartbeatBlockRangeParams {
    pub from: u64,
    pub to: u64,
}

#[derive(Debug, serde::Serialize)]
pub struct HeartbeatBlockRangeResponse {
    pub from_height: u64,
    pub to_height: u64,
    pub tip_height: u64,
    pub blocks: Vec<db::HeartbeatBlockWindowRow>,
}

/// GET /api/stats/heartbeat/blocks?from=H1&to=H2 - lightweight historical block
/// range page for infinite lazy-loading of Heartbeat EKG blocks.
pub async fn get_heartbeat_block_range(
    State(state): State<SharedStatsState>,
    Query(params): Query<HeartbeatBlockRangeParams>,
) -> impl axum::response::IntoResponse {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    if params.from > params.to {
        tracing::warn!(
            from = params.from,
            to = params.to,
            "Rejected reversed heartbeat block range query"
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "from height must be less than or equal to to height"
            })),
        )
            .into_response();
    }

    if (params.to - params.from) >= MAX_HEARTBEAT_BLOCK_RANGE {
        tracing::warn!(
            from = params.from,
            to = params.to,
            max = MAX_HEARTBEAT_BLOCK_RANGE,
            "Rejected over-wide heartbeat block range query"
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!(
                    "Requested range exceeds maximum of {MAX_HEARTBEAT_BLOCK_RANGE} blocks per page"
                )
            })),
        )
            .into_response();
    }

    let pool = state.db.clone();
    let from = params.from;
    let to = params.to;

    let result = tokio::task::spawn_blocking(
        move || -> Result<HeartbeatBlockRangeResponse, StatsError> {
            let conn = pool
                .get()
                .map_err(|e| StatsError::Rpc(format!("DB pool: {e}")))?;

            let blocks = db::query_heartbeat_block_range(&conn, from, to)?;
            let tip_height = db::max_height(&conn)?.unwrap_or(0);

            Ok(HeartbeatBlockRangeResponse {
                from_height: from,
                to_height: to,
                tip_height,
                blocks,
            })
        },
    )
    .await;

    match result {
        Ok(Ok(response)) => {
            let cache_control = heartbeat_history_cache_control(
                &response.blocks,
                response.tip_height,
            );
            (
                StatusCode::OK,
                [(header::CACHE_CONTROL, cache_control)],
                Json(response),
            )
                .into_response()
        }
        Ok(Err(err)) => {
            tracing::error!(
                from = params.from,
                to = params.to,
                "Heartbeat block range query failed: {err}"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal server error" })),
            )
                .into_response()
        }
        Err(err) => {
            tracing::error!(
                from = params.from,
                to = params.to,
                "Heartbeat block range task failed: {err}"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal server error" })),
            )
                .into_response()
        }
    }
}
