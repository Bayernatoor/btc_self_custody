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
use std::sync::{Arc, Mutex};
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

    let state = Arc::new(StatsState {
        db: pool,
        rpc,
        live_cache: Mutex::new(None),
        price_cache: Mutex::new(None),
        price_refreshing: std::sync::atomic::AtomicBool::new(false),
        utxo_count: Mutex::new(None),
        stats_summary_cache: Mutex::new(None),
        daily_cache: Mutex::new(None),
        block_ts_cache: Mutex::new(std::collections::HashMap::new()),
        signaling_blocks_cache: Mutex::new(None),
        signaling_periods_cache: Mutex::new(None),
        price_history_cache: Mutex::new(None),
        range_summary_cache: Mutex::new(None),
        extremes_cache: Mutex::new(None),
        heartbeat_tx,
        sse_connections: std::sync::atomic::AtomicUsize::new(0),
    });

    // Build the API router
    let router = Router::new()
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
        .with_state(Arc::clone(&state));

    Some((state, router, config.zmq_tx_url, config.zmq_block_url))
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
            // Warm extremes cache
            if let Ok(conn) = state.db.get() {
                if let Ok(extremes) =
                    db::query_extremes_with_heights(&conn, from_ts, to_ts)
                {
                    *state
                        .extremes_cache
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = Some((
                        from_ts,
                        to_ts,
                        extremes,
                        std::time::Instant::now(),
                    ));
                }
                // Warm range summary cache
                if let Ok(summary) =
                    db::query_range_summary(&conn, from_ts, to_ts)
                {
                    *state
                        .range_summary_cache
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = Some((
                        from_ts,
                        to_ts,
                        summary,
                        std::time::Instant::now(),
                    ));
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
                    Ok(info) => {
                        *state.utxo_count.lock().unwrap() = Some(info.txouts)
                    }
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
                    *state
                        .price_cache
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) =
                        Some((price, std::time::Instant::now()));
                }
                Err(e) => tracing::warn!("Initial price fetch failed: {e}"),
            }
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(90)).await;
                match state.rpc.fetch_price().await {
                    Ok(price) => {
                        *state
                            .price_cache
                            .lock()
                            .unwrap_or_else(|e| e.into_inner()) =
                            Some((price, std::time::Instant::now()));
                    }
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

                // Verify last 6 blocks match canonical chain (detect reorgs)
                ingest::verify_recent_blocks(&state.rpc, &state.db, 6).await;

                // Check if new blocks were added; if so, clear stale caches
                let new_height = state
                    .db
                    .get()
                    .ok()
                    .and_then(|c| db::max_height(&c).ok().flatten())
                    .unwrap_or(0);
                if new_height > last_height {
                    last_height = new_height;
                    // Invalidate server-side caches
                    state
                        .range_summary_cache
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take();
                    state
                        .extremes_cache
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take();
                    state
                        .stats_summary_cache
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take();
                    state
                        .daily_cache
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .take();
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
