//! Stats module initialization and background task spawning.

use axum::routing::get;
use axum::Router;
use std::sync::{Arc, Mutex};

use super::api::{self, StatsState};
use super::config::StatsConfig;
use super::db;
use super::ingest;
use super::rpc::BitcoinRpc;

/// Initialize the stats module. Returns None if not configured.
pub async fn init() -> Option<(Arc<StatsState>, Router)> {
    let config = StatsConfig::load()?;

    tracing::info!("Stats module: connecting to {}", config.rpc_url);

    let rpc =
        BitcoinRpc::new(config.rpc_url, config.rpc_user, config.rpc_password);
    let conn =
        db::open(&config.db_path).expect("Failed to open stats database");

    // Forward ingestion (catch up to tip)
    if let Err(e) = ingest::run(&rpc, &conn, config.initial_ingest_count).await
    {
        tracing::error!("Stats forward ingestion failed: {e}");
    }

    let state = Arc::new(StatsState {
        db: Mutex::new(conn),
        rpc,
        price_cache: Mutex::new(None),
        utxo_count: Mutex::new(None),
    });

    // Build the API router
    let router = Router::new()
        .route("/blocks", get(api::get_blocks))
        .route("/blocks/{height}", get(api::get_block_detail))
        .route("/stats", get(api::get_stats))
        .route("/live", get(api::get_live))
        .route("/op-returns", get(api::get_op_returns))
        .route("/aggregates/daily", get(api::get_daily_aggregates))
        .route("/signaling", get(api::get_signaling))
        .route("/signaling/periods", get(api::get_signaling_periods))
        .with_state(Arc::clone(&state));

    Some((state, router))
}

/// Spawn background tasks (call after server starts listening).
pub fn spawn_background_tasks(state: Arc<StatsState>) {
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

    // Poll for new blocks every 60 seconds
    {
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                ingest::poll_new_blocks(&state.rpc, &state.db).await;
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
}
