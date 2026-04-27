//! Stats module initialization and background task spawning.
//!
//! ## Initialization Flow
//!
//! 1. Load config from environment variables (returns None if dormant)
//! 2. Create RPC client and open SQLite connection pool (16 connections, WAL mode)
//! 3. Run forward ingestion to catch up from max_height+1 to chain tip
//! 4. Build shared state with all caches initialized to None
//! 5. Build Axum router with all API endpoints
//!
//! ## Background Tasks (spawned after server starts listening)
//!
//! - **UTXO refresh**: calls `gettxoutsetinfo` every 10 minutes (slow RPC, 60-300s)
//! - **Block poller**: checks for new blocks every 15 seconds via `getblockchaininfo`
//! - **Extras backfill**: re-fetches blocks with outdated backfill_version
//! - **Backward backfill**: fills historical blocks from min_height down to genesis
//! - **Mempool seed**: loads current mempool via `getrawmempool` on startup
//! - **ZMQ subscriber**: real-time tx and block notifications (if configured)
//! - **Mempool pruner**: deletes old mempool_txs entries daily

use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tokio::sync::broadcast;

use super::api::{self, StatsState};
use super::config::StatsConfig;
use super::db;
use super::ingest;
use super::rpc::BitcoinRpc;
use super::zmq_subscriber;

/// Initialize the stats module. Returns `None` if `BITCOIN_STATS_RPC_URL` is not set
/// (dormant mode). Otherwise returns (shared_state, router, zmq_tx_url, zmq_block_url).
pub async fn init(
) -> Option<(Arc<StatsState>, Router, Option<String>, Option<String>)> {
    let config = StatsConfig::load()?;

    tracing::info!("Stats module: connecting to {}", config.rpc_url);

    let rpc =
        BitcoinRpc::new(config.rpc_url, config.rpc_user, config.rpc_password);

    // Open connection pool (16 connections: API readers + background ingestion tasks)
    let pool = db::open_pool(&config.db_path, 16)
        .expect("Failed to open stats database pool");

    // Forward ingestion (catch up to tip) using a pooled connection
    {
        let conn = pool
            .get()
            .expect("Failed to get DB connection for ingestion");
        if let Err(e) =
            ingest::run(&rpc, &conn, config.initial_ingest_count).await
        {
            tracing::error!("Stats forward ingestion failed: {e}");
        }
        // Build pre-computed daily aggregates table if empty (first run)
        if let Err(e) = db::rebuild_all_daily_blocks(&conn) {
            tracing::warn!("Failed to build daily_blocks: {e}");
        }
    }

    // Broadcast channel for heartbeat events (ZMQ → SSE). 4096 buffer handles bursts.
    let (heartbeat_tx, _) = broadcast::channel(4096);

    // Every cache built through StatsStateBuilder is auto-registered
    // with the invalidation registry under each provided tag. Adding
    // a new cache is one line; the wiring cannot be forgotten.
    use std::time::Duration;
    use super::cache::CacheTag;
    use super::types;
    let mut cb = super::api::StatsStateBuilder::new();
    let price_cache = cb.cache::<(), super::rpc::PriceInfo>(
        "price",
        Duration::from_secs(60),
        &[],
    );
    let utxo_count =
        cb.cache::<(), u64>("utxo_count", Duration::MAX, &[]);
    let stats_summary_cache = cb.cache::<(), types::StatsSummary>(
        "stats_summary",
        Duration::from_secs(60),
        &[CacheTag::OnNewBlock],
    );
    let daily_cache = cb.cache::<(u64, u64), Vec<types::DailyAggregate>>(
        "daily_aggregates",
        Duration::from_secs(120),
        &[CacheTag::OnNewBlock],
    );
    let block_ts_cache =
        cb.cache::<u64, u64>("block_timestamps", Duration::MAX, &[]);
    let signaling_blocks_cache = cb
        .cache::<String, (Vec<types::SignalingBlock>, types::PeriodStats)>(
            "signaling_blocks",
            Duration::from_secs(60),
            &[],
        );
    let signaling_periods_cache = cb
        .cache::<String, Vec<super::db::SignalingPeriod>>(
            "signaling_periods",
            Duration::from_secs(60),
            &[],
        );
    let price_history_cache = cb.cache::<(), Vec<types::PricePoint>>(
        "price_history",
        Duration::from_secs(3600),
        &[],
    );
    let range_summary_cache = cb.cache::<(u64, u64), types::RangeSummary>(
        "range_summary",
        Duration::from_secs(60),
        &[CacheTag::OnNewBlock],
    );
    let extremes_cache = cb.cache::<(u64, u64), types::ExtremesData>(
        "extremes",
        Duration::from_secs(60),
        &[CacheTag::OnNewBlock],
    );

    let state = Arc::new(StatsState {
        db: pool,
        rpc,
        cache_registry: cb.into_registry(),
        price_cache,
        utxo_count,
        stats_summary_cache,
        daily_cache,
        block_ts_cache,
        signaling_blocks_cache,
        signaling_periods_cache,
        price_history_cache,
        range_summary_cache,
        extremes_cache,
        heartbeat_tx,
        sse_connections: std::sync::atomic::AtomicUsize::new(0),
    });

    let router = build_api_router(Arc::clone(&state));
    Some((state, router, config.zmq_tx_url, config.zmq_block_url))
}

/// Assemble the stats API router. Extracted from [`init`] so integration
/// tests can build an identical router (same routes, same handlers) around
/// a test-owned `StatsState` with a tempfile DB and stub RPC. Keeping the
/// route list in exactly one place prevents the "tests passed, prod broke"
/// regression class where a new route is wired in prod but forgotten in
/// test setup.
pub(crate) fn build_api_router(state: Arc<StatsState>) -> Router {
    Router::new()
        .route("/blocks", get(api::get_blocks))
        .route("/blocks/{height}", get(api::get_block_detail))
        .route("/stats", get(api::get_stats))
        .route("/cache-stats", get(api::get_cache_stats))
        .route("/live", get(api::get_live))
        .route("/op-returns", get(api::get_op_returns))
        .route("/aggregates/daily", get(api::get_daily_aggregates))
        .route("/signaling", get(api::get_signaling))
        .route("/signaling/periods", get(api::get_signaling_periods))
        .route("/heartbeat", get(api::get_heartbeat_sse))
        .route("/tx/{txid}", get(api::get_tx_detail))
        .with_state(state)
}

/// Spawn all background tasks. Must be called after the server starts listening
/// so that RPC connections and ZMQ subscriptions do not block startup.
pub fn spawn_background_tasks(
    state: Arc<StatsState>,
    zmq_tx_url: Option<String>,
    zmq_block_url: Option<String>,
) {
    // Pre-warm caches for ALL range (the slowest queries).
    // Runs in background so it doesn't block server startup.
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            let (from_ts, to_ts) = match state.db.get() {
                Ok(conn) => {
                    let max_ts = db::query_stats(&conn)
                        .ok()
                        .flatten()
                        .map(|s| s.latest_timestamp)
                        .unwrap_or(0);
                    (0u64, max_ts)
                }
                Err(_) => return,
            };
            if to_ts == 0 {
                return;
            }
            tracing::info!("Pre-warming caches for ALL range...");
            if let Ok(conn) = state.db.get() {
                if let Ok(extremes) =
                    db::query_extremes_with_heights(&conn, from_ts, to_ts)
                {
                    state.extremes_cache.insert((from_ts, to_ts), extremes);
                }
                if let Ok(summary) =
                    db::query_range_summary(&conn, from_ts, to_ts)
                {
                    state
                        .range_summary_cache
                        .insert((from_ts, to_ts), summary);
                }
                tracing::info!("Cache pre-warm complete");
            }
        });
    }

    // UTXO refresh every 10 minutes
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                match state.rpc.get_txout_set_info().await {
                    Ok(info) => state.utxo_count.insert((), info.txouts),
                    Err(e) => tracing::warn!("UTXO refresh failed: {e}"),
                }
                tokio::time::sleep(std::time::Duration::from_secs(600)).await;
            }
        });
    }

    // Background price refresh every 90 seconds.
    // Critical for whale detection: ZMQ subscriber reads price_cache per tx,
    // and if nobody loads the dashboard, the cache stays empty and no whales
    // get flagged. This ensures the price is always fresh regardless of user activity.
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            // Initial fetch immediately so whale detection works from startup
            match state.rpc.fetch_price().await {
                Ok(price) => {
                    tracing::info!(
                        "Price cache initialized: ${:.2}",
                        price.usd
                    );
                    state.price_cache.insert((), price);
                }
                Err(e) => tracing::warn!("Initial price fetch failed: {e}"),
            }
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(90)).await;
                match state.rpc.fetch_price().await {
                    Ok(price) => state.price_cache.insert((), price),
                    Err(e) => tracing::debug!("Price refresh failed: {e}"),
                }
            }
        });
    }

    // Poll for new blocks every 15 seconds, invalidate caches on new data
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            let mut last_height = {
                state
                    .db
                    .get()
                    .ok()
                    .and_then(|c| db::max_height(&c).ok().flatten())
                    .unwrap_or(0)
            };
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                ingest::poll_new_blocks(&state.rpc, &state.db).await;

                // Verify the last blocks against the canonical chain (detect reorgs).
                // Depth must match super::rpc::REORG_DETECTION_DEPTH so the
                // ZMQ hashblock handler invalidates the same window of
                // block_hash_cache entries — otherwise a reorg at a depth
                // verified but not invalidated would be silently missed.
                ingest::verify_recent_blocks(
                    &state.rpc,
                    &state.db,
                    super::rpc::REORG_DETECTION_DEPTH,
                )
                .await;

                // Check if new blocks were added; if so, clear stale caches
                let new_height = state
                    .db
                    .get()
                    .ok()
                    .and_then(|c| db::max_height(&c).ok().flatten())
                    .unwrap_or(0);
                if new_height > last_height {
                    last_height = new_height;
                    // All caches that depend on chain tip subscribe to
                    // OnNewBlock at construction (see startup builder
                    // above); the registry fans this out.
                    state.invalidate(super::cache::CacheTag::OnNewBlock);
                    // Update today's pre-computed daily aggregate
                    if let Ok(conn) = state.db.get() {
                        let today = db::timestamp_to_date(
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        );
                        let _ = db::refresh_daily_block(&conn, &today);
                    }
                }
            }
        });
    }

    // Backfill extras + backward ingestion
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            ingest::backfill_extras(&state.rpc, &state.db).await;
            ingest::backfill_backwards(&state.rpc, &state.db).await;
        });
    }

    // Periodic gap-detection: re-fills missing heights below tip that
    // forward-ingestion can leave when the node is unreachable mid
    // chain-advance. Runs immediately on startup, then every 5 minutes.
    // Cheap when there's nothing to do (one O(1) COUNT query).
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                ingest::backfill_gaps(&state.rpc, &state.db).await;
                tokio::time::sleep(std::time::Duration::from_secs(300))
                    .await;
            }
        });
    }

    // Seed mempool_txs table with current mempool (getrawmempool verbose).
    // ZMQ only sends NEW txs, so without this, the table is empty after
    // restart and the SSE history has no bricks to show.
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            match state.rpc.get_raw_mempool_verbose().await {
                Ok(entries) => {
                    let count = entries.len();
                    if let Ok(conn) = state.db.get() {
                        let mut inserted = 0u64;
                        for (txid, fee, vsize) in &entries {
                            if db::insert_mempool_tx(
                                &conn,
                                &db::MempoolTxInsert {
                                    txid,
                                    fee: *fee,
                                    vsize: *vsize,
                                    first_seen: now,
                                    ..Default::default()
                                },
                            )
                            .is_ok()
                            {
                                inserted += 1;
                            }
                        }
                        tracing::info!(
                            "Mempool seed: {inserted}/{count} txs inserted from getrawmempool"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("Mempool seed failed: {e}");
                }
            }
        });
    }

    // ZMQ subscriber (only if both URLs are configured)
    if let (Some(tx_url), Some(block_url)) = (zmq_tx_url, zmq_block_url) {
        zmq_subscriber::spawn(
            Arc::clone(&state),
            state.heartbeat_tx.clone(),
            tx_url,
            block_url,
        );

        // Prune old mempool txs immediately then daily
        let prune_state = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                zmq_subscriber::prune_old_txs(&prune_state).await;
                tokio::time::sleep(std::time::Duration::from_secs(86400)).await;
            }
        });
    } else {
        tracing::info!("ZMQ subscriber disabled (BITCOIN_STATS_ZMQ_TX / BITCOIN_STATS_ZMQ_BLOCK not set)");
    }

    tracing::info!("Connection pool: 16 connections (WAL mode enabled)");
}
