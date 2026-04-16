//! Block ingestion pipeline with three modes:
//!
//! 1. **Forward ingestion** (startup, sync): catches up from max_height+1 to chain tip
//! 2. **Extras backfill** (background): re-fetches blocks missing version/fees/median/locktime data
//! 3. **Backward backfill** (background): fills blocks from min_height down to genesis
//!
//! All modes use parallel fetching (32 concurrent RPC calls) with batch DB inserts.
//! Background tasks yield the DB lock between batches so API requests aren't starved.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use futures::stream::{self, StreamExt};
use rusqlite::Connection;

use super::db::DbPool;
use super::rpc::{BitcoinRpc, Block};
use super::{db, error::StatsError};

/// Number of concurrent RPC fetch tasks. Env: `BITCOIN_STATS_RPC_CONCURRENCY` (default: 8).
fn concurrency() -> usize {
    std::env::var("BITCOIN_STATS_RPC_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8)
}
/// Number of blocks to fetch before writing a batch to the database.
/// Keeps the DB lock held briefly so API queries are not starved.
const DB_BATCH_SIZE: usize = 100;

/// Forward ingestion: catch up from max_height+1 to chain tip.
pub async fn run(
    rpc: &BitcoinRpc,
    conn: &Connection,
    initial_count: u64,
) -> Result<(), StatsError> {
    let info = rpc.get_blockchain_info().await?;
    let tip = info.blocks;
    tracing::info!("Chain tip: {} ({})", tip, info.chain);

    let db_max = db::max_height(conn)?;
    let start = match db_max {
        Some(h) => h + 1,
        None => tip.saturating_sub(initial_count),
    };

    if start > tip {
        tracing::info!("Database is up to date");
        return Ok(());
    }

    ingest_range(rpc, conn, start, tip, "Ingesting").await
}

/// Background: check for new blocks and ingest them. Runs every 60 seconds.
pub async fn poll_new_blocks(rpc: &BitcoinRpc, pool: &DbPool) {
    let tip = match rpc.get_blockchain_info().await {
        Ok(info) => info.blocks,
        Err(e) => {
            tracing::warn!("Poll: failed to get chain tip: {e}");
            return;
        }
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Poll: DB pool error: {e}");
            return;
        }
    };

    let db_max = db::max_height(&conn).unwrap_or(None).unwrap_or(0);

    if db_max >= tip {
        return; // already up to date
    }

    let start = db_max + 1;
    let count = tip - start + 1;
    tracing::info!("Poll: ingesting {count} new blocks ({start} -> {tip})");

    // Drop the connection while doing RPC work
    drop(conn);

    let results: Vec<Result<Block, _>> = stream::iter(start..=tip)
        .map(|height| async move { rpc.fetch_block_by_height(height).await })
        .buffer_unordered(concurrency())
        .collect()
        .await;

    let mut blocks = Vec::with_capacity(results.len());
    for result in results {
        match result {
            Ok(b) => blocks.push(b),
            Err(e) => tracing::warn!("Poll: block fetch error: {e}"),
        }
    }
    blocks.sort_by_key(|b| b.height);

    // Get a fresh connection for the insert
    if let Ok(conn) = pool.get() {
        if let Err(e) = db::insert_blocks(&conn, &blocks) {
            tracing::error!("Poll: DB insert error: {e}");
        }
    }

    tracing::info!("Poll: ingested {} blocks up to {tip}", blocks.len());
}

/// Verify the last `depth` blocks in the DB match the canonical chain.
/// If a mismatch is found (stale block from a reorg), delete and re-fetch it.
/// Returns the number of blocks corrected.
pub async fn verify_recent_blocks(
    rpc: &BitcoinRpc,
    pool: &DbPool,
    depth: u64,
) -> u64 {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let db_max = db::max_height(&conn).unwrap_or(None).unwrap_or(0);
    if db_max == 0 {
        return 0;
    }

    let check_from = db_max.saturating_sub(depth);
    let mut corrected = 0u64;

    for height in check_from..=db_max {
        let stored_hash = match db::query_block_hash(&conn, height) {
            Ok(Some(h)) => h,
            _ => continue, // block not in DB, will be fetched by poller
        };

        let canonical_hash = match rpc.get_block_hash(height).await {
            Ok(h) => h,
            Err(_) => continue,
        };

        if stored_hash != canonical_hash {
            tracing::warn!(
                "Reorg detected at height {height}: stored={} canonical={}",
                &stored_hash[..16],
                &canonical_hash[..16]
            );

            // Log the reorg
            let _ =
                db::insert_reorg(&conn, height, &stored_hash, &canonical_hash);

            // Delete the stale block
            let _ = db::delete_block(&conn, height);

            // Re-fetch the canonical block
            match rpc.fetch_block_by_height(height).await {
                Ok(block) => {
                    if let Err(e) = db::insert_blocks(&conn, &[block]) {
                        tracing::error!(
                            "Failed to insert canonical block at {height}: {e}"
                        );
                    } else {
                        tracing::info!("Replaced stale block at height {height} with canonical version");
                        corrected += 1;
                    }
                }
                Err(e) => tracing::error!(
                    "Failed to fetch canonical block at {height}: {e}"
                ),
            }
        }
    }

    if corrected > 0 {
        tracing::info!("Reorg correction: replaced {corrected} stale blocks");
    }
    corrected
}

/// Background: backfill blocks with backfill_version < BACKFILL_VERSION.
pub async fn backfill_extras(rpc: &BitcoinRpc, pool: &DbPool) {
    let needs_backfill = {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Backfill: DB pool error: {e}");
                return;
            }
        };
        db::count_needs_backfill(&conn).unwrap_or(0)
    };

    if needs_backfill == 0 {
        tracing::info!(
            "No blocks need backfill (all at version {})",
            db::BACKFILL_VERSION
        );
        return;
    }

    tracing::info!(
        "Backfilling {needs_backfill} blocks to version {}",
        db::BACKFILL_VERSION
    );
    let started = Instant::now();
    let mut total_done = 0u64;
    let mut total_failed = 0u64;

    loop {
        let heights = {
            let conn = match pool.get() {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("Backfill: DB pool error: {e}");
                    return;
                }
            };
            db::heights_needing_backfill(&conn, DB_BATCH_SIZE as u64)
                .unwrap_or_default()
        };

        if heights.is_empty() {
            break;
        }

        let results: Vec<Result<Block, _>> =
            stream::iter(heights.iter().copied())
                .map(|h| async move { rpc.fetch_block_by_height(h).await })
                .buffer_unordered(concurrency())
                .collect()
                .await;

        let mut blocks = Vec::with_capacity(results.len());
        for result in results {
            match result {
                Ok(b) => blocks.push(b),
                Err(e) => {
                    tracing::warn!("Backfill error: {e}");
                    total_failed += 1;
                    continue;
                }
            }
        }

        {
            let conn = match pool.get() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Backfill: DB pool error: {e}");
                    return;
                }
            };
            if let Err(e) = db::update_block_extras(&conn, &blocks) {
                tracing::error!("Backfill DB error: {e}");
                return;
            }
        }

        total_done += blocks.len() as u64;
        let elapsed = started.elapsed().as_secs_f64();
        let rate = total_done as f64 / elapsed;
        let remaining = needs_backfill.saturating_sub(total_done) as f64 / rate;
        tracing::info!(
            "Backfill progress: {total_done}/{needs_backfill} — {rate:.1} blocks/sec, ~{remaining:.0}s remaining"
        );
    }

    if total_failed > 0 {
        tracing::warn!("Backfill had {total_failed} failed blocks (will retry on next restart)");
    }
    tracing::info!(
        "Backfill complete: {total_done} blocks in {:.1}s",
        started.elapsed().as_secs_f64()
    );
}

/// Background: ingest blocks before current min_height down to genesis.
pub async fn backfill_backwards(rpc: &BitcoinRpc, pool: &DbPool) {
    // Wait a bit for forward ingestion and extras backfill to settle
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    let min_height = {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Backward backfill: DB pool error: {e}");
                return;
            }
        };
        db::min_height(&conn).unwrap_or(Some(0)).unwrap_or(0)
    };

    if min_height == 0 {
        tracing::info!("Backward backfill not needed (already at genesis)");
        return;
    }

    let total = min_height;
    tracing::info!(
        "Backward backfill: ingesting {total} blocks (0 -> {})",
        min_height - 1
    );
    let started = Instant::now();
    let fetched = Arc::new(AtomicU64::new(0));

    // Process in chunks, going from min_height-1 down to 0
    let heights: Vec<u64> = (1..min_height).rev().collect();

    for chunk in heights.chunks(DB_BATCH_SIZE) {
        let fetched_ref = Arc::clone(&fetched);

        let results: Vec<Result<Block, _>> = stream::iter(
            chunk.iter().copied(),
        )
        .map(|height| async move { rpc.fetch_block_by_height(height).await })
        .buffer_unordered(concurrency())
        .collect()
        .await;

        let mut blocks = Vec::with_capacity(results.len());
        for result in results {
            match result {
                Ok(b) => blocks.push(b),
                Err(e) => {
                    tracing::warn!("Backward backfill error at block: {e}");
                    continue;
                }
            }
        }

        blocks.sort_by_key(|b| b.height);

        {
            let conn = match pool.get() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Backward backfill: DB pool error: {e}");
                    return;
                }
            };
            if let Err(e) = db::insert_blocks(&conn, &blocks) {
                tracing::error!("Backward backfill DB error: {e}");
                return;
            }
        }

        let count = fetched_ref
            .fetch_add(blocks.len() as u64, Ordering::Relaxed)
            + blocks.len() as u64;
        let elapsed = started.elapsed().as_secs_f64();
        let rate = count as f64 / elapsed;
        let remaining = (total - count) as f64 / rate;

        if count % 1000 < DB_BATCH_SIZE as u64 {
            tracing::info!(
                "Backward backfill: {count}/{total} — {rate:.1} blocks/sec, ~{remaining:.0}s remaining"
            );
        }
    }

    tracing::info!(
        "Backward backfill complete: {total} blocks in {:.1}s",
        started.elapsed().as_secs_f64()
    );
}

/// Shared ingestion logic for a height range.
async fn ingest_range(
    rpc: &BitcoinRpc,
    conn: &Connection,
    start: u64,
    end: u64,
    label: &str,
) -> Result<(), StatsError> {
    let total = end - start + 1;
    tracing::info!(
        "{label} {total} blocks ({start} -> {end}) with {} concurrent fetches",
        concurrency()
    );

    let started = Instant::now();
    let fetched = Arc::new(AtomicU64::new(0));

    let heights: Vec<u64> = (start..=end).collect();

    for chunk in heights.chunks(DB_BATCH_SIZE) {
        let fetched_ref = Arc::clone(&fetched);

        let results: Vec<Result<Block, StatsError>> = stream::iter(
            chunk.iter().copied(),
        )
        .map(|height| async move { rpc.fetch_block_by_height(height).await })
        .buffer_unordered(concurrency())
        .collect()
        .await;

        let mut blocks = Vec::with_capacity(results.len());
        for result in results {
            blocks.push(result?);
        }

        blocks.sort_by_key(|b| b.height);

        db::insert_blocks(conn, &blocks)?;

        let count = fetched_ref
            .fetch_add(blocks.len() as u64, Ordering::Relaxed)
            + blocks.len() as u64;
        let elapsed = started.elapsed().as_secs_f64();
        let rate = count as f64 / elapsed;
        let remaining = (total - count) as f64 / rate;

        let max_height = blocks.last().map(|b| b.height).unwrap_or(0);
        tracing::info!(
            "{label} up to block {max_height} ({count}/{total}) — {rate:.1} blocks/sec, ~{remaining:.0}s remaining"
        );
    }

    let elapsed = started.elapsed();
    tracing::info!(
        "{label} complete: {total} blocks in {:.1}s ({:.1} blocks/sec)",
        elapsed.as_secs_f64(),
        total as f64 / elapsed.as_secs_f64()
    );

    Ok(())
}
