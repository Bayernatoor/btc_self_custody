//! Leptos server functions bridging frontend components to the backend.
//!
//! Each `#[server]` function is callable from both server-side rendering and
//! WASM client code. On the server, they extract the shared [`StatsState`] from
//! Axum extensions, query the database (or RPC), and return shared types from
//! `super::types`. On the client, Leptos auto-generates HTTP POST calls to
//! `/api/<endpoint>`.
//!
//! ## Error Handling
//!
//! All server functions use the [`internal_err`] helper which logs the full error
//! server-side (including SQL, RPC details) but returns only a generic "Internal
//! server error" message to the client, preventing information leakage.
//!
//! ## Caching
//!
//! Several functions implement server-side in-memory caching with TTLs:
//! - `fetch_stats_summary`: 60s TTL
//! - `fetch_live_stats`: 10s TTL
//! - `fetch_daily_aggregates`: 120s TTL (keyed by from/to range)
//! - `fetch_signaling` / `fetch_signaling_periods`: 60s TTL (keyed by method+range)
//! - `fetch_price_history`: 1 hour TTL (full dataset cached)
//! - `fetch_range_summary` / `fetch_extremes`: 60s TTL (keyed by from/to range)
//! - `fetch_block_timestamp`: cached forever (block timestamps are immutable)

use leptos::prelude::*;
use leptos::server;

use super::types::*;

#[cfg(feature = "ssr")]
use axum::extract::Extension;

/// Log error details server-side, return generic message to client.
#[cfg(feature = "ssr")]
fn internal_err(context: &str, err: impl std::fmt::Display) -> ServerFnError {
    tracing::error!("{context}: {err}");
    ServerFnError::new("Internal server error")
}

/// Pull the shared [`StatsState`](super::api::StatsState) out of the request's
/// Axum extensions.
///
/// Every server function needs this and the extraction has to happen per
/// request, so it cannot be hoisted out of the functions entirely; but it also
/// does not need to be written out 26 times. The turbofish-heavy `Extension`
/// destructuring obscured what each function actually does, and the error
/// context string had to be repeated identically at every site to keep the
/// client-facing message uniform.
#[cfg(feature = "ssr")]
async fn state() -> Result<std::sync::Arc<super::api::StatsState>, ServerFnError>
{
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract()
            .await
            .map_err(|e| internal_err("Stats unavailable", e))?;
    Ok(state)
}

/// A pooled SQLite connection, for the seventeen server functions that query
/// the database immediately and hold the connection for the whole call.
///
/// `PooledConnection` is owned, holding an `Arc` on the pool, so it outlives
/// this call without borrowing the state it came from.
///
/// **There is deliberately no `state_and_conn()` returning both.** The two
/// functions that need the state and a connection acquire the connection
/// separately and on purpose. `fetch_live_stats` scopes its connection to a
/// block so it is released before the RPC awaits that follow, and
/// `fetch_block_timestamp` checks its cache first and takes a connection only
/// on a miss. A combined helper reads like the tidier option and would quietly
/// undo both: the pool is 16 connections, so holding one across multi-second
/// RPC calls, or across every cache hit on a permanently-cached endpoint,
/// starves every other request.
#[cfg(feature = "ssr")]
async fn conn() -> Result<
    r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
    ServerFnError,
> {
    let state = state().await?;
    state.db.get().map_err(|e| internal_err("DB pool", e))
}

#[server(prefix = "/api", endpoint = "stats_summary")]
pub async fn fetch_stats_summary() -> Result<StatsSummary, ServerFnError> {
    let state = state().await?;

    state
        .stats_summary_cache
        .clone()
        .get_or_compute((), || async move {
            let conn =
                state.db.get().map_err(|e| internal_err("DB pool", e))?;
            let stats = super::db::query_stats(&conn)
                .map_err(|e| internal_err("DB query", e))?;
            Ok::<_, ServerFnError>(match stats {
                Some(s) => StatsSummary {
                    block_count: s.block_count,
                    min_height: s.min_height,
                    max_height: s.max_height,
                    latest_timestamp: s.latest_timestamp,
                },
                None => StatsSummary {
                    block_count: 0,
                    min_height: 0,
                    max_height: 0,
                    latest_timestamp: 0,
                },
            })
        })
        .await
}

/// Fetch the most recent `count` blocks straight from the DB (droplet-local),
/// deriving the tip from `db::max_height` — NO node RPC. The heartbeat's initial
/// timeline uses this so the page renders the historical EKG immediately even
/// when the home node is unreachable, instead of blocking on live stats.
#[server(prefix = "/api", endpoint = "stats_recent_blocks")]
pub async fn fetch_recent_blocks(
    count: u64,
) -> Result<Vec<BlockSummary>, ServerFnError> {
    let count = count.min(MAX_PER_BLOCK_RANGE); // matches fetch_blocks range guard
    let conn = conn().await?;
    let tip = super::db::max_height(&conn)
        .map_err(|e| internal_err("DB query", e))?
        .unwrap_or(0);
    if tip == 0 {
        return Ok(Vec::new());
    }
    // query_blocks is inclusive on both ends, so subtract count-1 to return
    // exactly `count` blocks (tip-count+1 ..= tip), matching the doc contract.
    let from = tip.saturating_sub(count.saturating_sub(1));
    let rows = super::db::query_blocks(&conn, from, tip)
        .map_err(|e| internal_err("DB query", e))?;
    Ok(rows.into_iter().map(BlockSummary::from).collect())
}

#[server(prefix = "/api", endpoint = "stats_blocks")]
pub async fn fetch_blocks(
    from: u64,
    to: u64,
) -> Result<Vec<BlockSummary>, ServerFnError> {
    if from > to {
        return Err(ServerFnError::new("Invalid block range"));
    }
    // Limit range to prevent DoS via huge queries. Must match the client's
    // per-block mode switch exactly: anything the client will request in
    // per-block mode has to be servable here. See MAX_PER_BLOCK_RANGE.
    if block_range_too_large(from, to) {
        return Err(ServerFnError::new("Block range too large"));
    }
    let conn = conn().await?;
    let rows = super::db::query_blocks(&conn, from, to)
        .map_err(|e| internal_err("DB query", e))?;
    Ok(rows.into_iter().map(BlockSummary::from).collect())
}

/// Fetch blocks by timestamp range (for custom date ranges).
#[server(prefix = "/api", endpoint = "stats_blocks_by_ts")]
pub async fn fetch_blocks_by_ts(
    from_ts: u64,
    to_ts: u64,
) -> Result<Vec<BlockSummary>, ServerFnError> {
    if from_ts > to_ts {
        return Err(ServerFnError::new("Invalid timestamp range"));
    }
    let conn = conn().await?;
    let rows = super::db::query_blocks_by_ts(
        &conn,
        from_ts,
        to_ts,
        MAX_PER_BLOCK_RANGE + 1,
    )
    .map_err(|e| internal_err("DB query", e))?;
    // The client only takes this path for windows it estimated at
    // MAX_PER_BLOCK_RANGE blocks or fewer, using span/600. That estimate
    // under-counts whenever blocks came faster than 10 minutes, and nothing
    // stops a direct call asking for the whole chain, so the cap is enforced
    // here rather than trusted from the caller. Fetching one row beyond the
    // cap distinguishes "exactly full" from "truncated".
    if rows.len() as u64 > MAX_PER_BLOCK_RANGE {
        return Err(ServerFnError::new("Block range too large"));
    }
    Ok(rows.into_iter().map(BlockSummary::from).collect())
}

#[server(prefix = "/api", endpoint = "stats_block_detail")]
pub async fn fetch_block_detail(
    height: u64,
) -> Result<Option<BlockDetail>, ServerFnError> {
    let conn = conn().await?;
    let row = super::db::query_block_by_height(&conn, height)
        .map_err(|e| internal_err("DB query", e))?;
    Ok(row.map(|r| BlockDetail {
        height: r.height,
        hash: r.hash,
        timestamp: r.timestamp,
        tx_count: r.tx_count,
        size: r.size,
        weight: r.weight,
        difficulty: r.difficulty,
        op_return_count: r.op_return_count,
        op_return_bytes: r.op_return_bytes,
        runes_count: r.runes_count,
        runes_bytes: r.runes_bytes,
        data_carrier_count: r.data_carrier_count,
        data_carrier_bytes: r.data_carrier_bytes,
        inscription_count: r.inscription_count,
        inscription_bytes: r.inscription_bytes,
        inscription_envelope_bytes: r.inscription_envelope_bytes,
        version: r.version,
        total_fees: r.total_fees,
        median_fee: r.median_fee,
        median_fee_rate: r.median_fee_rate,
        coinbase_locktime: r.coinbase_locktime,
        coinbase_sequence: r.coinbase_sequence,
        miner: r.miner,
        segwit_spend_count: r.segwit_spend_count,
        taproot_spend_count: r.taproot_spend_count,
    }))
}

/// Total block data size (bytes) for all blocks below a given height.
/// Used by the chain size chart to calculate the cumulative offset.
#[server(prefix = "/api", endpoint = "stats_cumulative_size")]
pub async fn fetch_cumulative_size(
    below_height: u64,
) -> Result<u64, ServerFnError> {
    let conn = conn().await?;
    let size = super::db::query_cumulative_size(&conn, below_height)
        .map_err(|e| internal_err("DB query", e))?;
    Ok(size)
}

/// Total block data size (bytes) for all blocks before a given timestamp.
/// Used by the chain size overlay for custom date ranges.
#[server(prefix = "/api", endpoint = "stats_cumulative_size_ts")]
pub async fn fetch_cumulative_size_before_ts(
    before_ts: u64,
) -> Result<u64, ServerFnError> {
    let conn = conn().await?;
    let size = super::db::query_cumulative_size_before_ts(&conn, before_ts)
        .map_err(|e| internal_err("DB query", e))?;
    Ok(size)
}

#[server(prefix = "/api", endpoint = "stats_live")]
pub async fn fetch_live_stats() -> Result<LiveStats, ServerFnError> {
    let state = state().await?;

    // No handler-level cache here — the underlying RPCs are cached in
    // BitcoinRpc per method (see rpc_cache.rs) with singleflight dedup
    // and stale-on-error fallback. The price fetch below has its own
    // 60s cache since it hits an external HTTP API, not Core RPC.

    // Get block height + difficulty from the DB (always current from 60s poll).
    // This avoids stale data when getblockchaininfo RPC is slow/fails.
    let db_stats = {
        let conn = state.db.get().map_err(|e| internal_err("DB pool", e))?;
        super::db::query_stats(&conn)
            .map_err(|e| internal_err("DB query", e))?
    };
    let db_height = db_stats.as_ref().map(|s| s.max_height).unwrap_or(0);
    let db_timestamp =
        db_stats.as_ref().map(|s| s.latest_timestamp).unwrap_or(0);

    // Parallelize RPC calls — all are non-fatal (fall back to defaults).
    // Each returns `(value, is_stale)`; OR the staleness flags together
    // so the response surfaces a single `stale: true` if any RPC fell back
    // to its cached-on-error value.
    let (blockchain_res, mempool_res, hashrate_res, fee_res) = tokio::join!(
        state.rpc.get_blockchain_info(),
        state.rpc.get_mempool_info(),
        state.rpc.get_network_hashps(),
        state.rpc.estimate_smart_fee(1),
    );

    let mut stale = false;

    // Use RPC blockchain info if available, but override block height with DB
    // (DB is always up-to-date from the poll, RPC might be stale/failed)
    let blockchain = match blockchain_res {
        Ok((info, s)) => {
            stale |= s;
            info
        }
        Err(e) => {
            tracing::warn!("Failed to fetch blockchain info: {e}");
            stale = true;
            super::rpc::BlockchainInfo {
                blocks: db_height,
                chain: "main".to_string(),
                difficulty: 0.0,
                verification_progress: 1.0,
                size_on_disk: 0,
                bestblockhash: String::new(),
                time: db_timestamp,
            }
        }
    };
    // Always use DB height — it's the source of truth (updated by poll)
    let block_height = db_height.max(blockchain.blocks);

    let mempool = match mempool_res {
        Ok((info, s)) => {
            stale |= s;
            info
        }
        Err(e) => {
            tracing::warn!("Failed to fetch mempool info: {e}");
            stale = true;
            super::rpc::MempoolInfo {
                size: 0,
                bytes: 0,
                usage: 0,
                total_fee: 0.0,
                maxmempool: 300_000_000,
                mempoolminfee: 0.0,
            }
        }
    };
    let hashrate = match hashrate_res {
        Ok((h, s)) => {
            stale |= s;
            h
        }
        Err(e) => {
            tracing::warn!("Failed to fetch hashrate: {e}");
            stale = true;
            0.0
        }
    };
    let next_block_fee = match fee_res {
        Ok((f, s)) => {
            stale |= s;
            f
        }
        Err(e) => {
            tracing::warn!("Failed to fetch fee estimate: {e}");
            stale = true;
            0.0
        }
    };

    // Price cache: 60s TTL + per-key singleflight inside Cache. The
    // 90s background refresh task writes independently. On fetch
    // failure we surface 0.0 since the TTL-evicted cache has nothing
    // to fall back to; the next background refresh recovers within 90s.
    let price_usd = {
        let state_for_price = std::sync::Arc::clone(&state);
        state
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
            })
    };

    const MAX_SUPPLY: f64 = 21_000_000.0;

    let total_supply = super::types::calc_supply(block_height);

    let percent_issued = (total_supply / MAX_SUPPLY) * 100.0;
    let sats_per_dollar = if price_usd > 0.0 {
        (100_000_000.0 / price_usd).round() as u64
    } else {
        0
    };
    let market_cap = price_usd * total_supply;
    let chain_size_gb = blockchain.size_on_disk as f64 / 1_000_000_000.0;
    let utxo_count = state.utxo_count.get(&()).unwrap_or(0);

    Ok(LiveStats {
        blockchain: LiveBlockchain {
            blocks: block_height,
            chain: blockchain.chain,
            difficulty: blockchain.difficulty,
            verification_progress: blockchain.verification_progress,
            size_on_disk: blockchain.size_on_disk,
            bestblockhash: blockchain.bestblockhash,
            time: blockchain.time,
        },
        mempool: LiveMempool {
            size: mempool.size,
            bytes: mempool.bytes,
            usage: mempool.usage,
            total_fee: mempool.total_fee,
            maxmempool: mempool.maxmempool,
            mempoolminfee: mempool.mempoolminfee,
        },
        next_block_fee,
        network: LiveNetwork {
            price_usd,
            sats_per_dollar,
            market_cap_usd: market_cap,
            total_supply,
            max_supply: MAX_SUPPLY,
            percent_issued: (percent_issued * 100.0).round() / 100.0,
            utxo_count,
            chain_size_gb: (chain_size_gb * 10.0).round() / 10.0,
            hashrate,
        },
        stale,
    })
}

#[server(prefix = "/api", endpoint = "stats_daily_aggregates")]
pub async fn fetch_daily_aggregates(
    from_ts: u64,
    to_ts: u64,
) -> Result<Vec<DailyAggregate>, ServerFnError> {
    if from_ts > to_ts {
        return Err(ServerFnError::new("Invalid timestamp range"));
    }

    let state = state().await?;

    state
        .daily_cache
        .clone()
        .get_or_compute((from_ts, to_ts), || async move {
            let conn =
                state.db.get().map_err(|e| internal_err("DB pool", e))?;
            let rows =
                super::db::query_daily_aggregates_fast(&conn, from_ts, to_ts)
                    .map_err(|e| internal_err("DB query", e))?;
            Ok::<_, ServerFnError>(
                rows.into_iter().map(DailyAggregate::from).collect(),
            )
        })
        .await
}

#[server(prefix = "/api", endpoint = "stats_signaling")]
pub async fn fetch_signaling(
    bit: u32,
    method: String,
    from: u64,
    to: u64,
) -> Result<(Vec<SignalingBlock>, PeriodStats), ServerFnError> {
    let state = state().await?;

    // Cache key includes method, bit, and range
    let cache_key = if method == "locktime" {
        format!("locktime:{from}:{to}")
    } else {
        format!("bit:{bit}:{from}:{to}")
    };

    state
        .signaling_blocks_cache
        .clone()
        .get_or_compute(cache_key, || async move {
            let conn =
                state.db.get().map_err(|e| internal_err("DB pool", e))?;
            let use_locktime = method == "locktime";

            let blocks = if use_locktime {
                super::db::query_signaling_locktime(&conn, from, to)
            } else {
                super::db::query_signaling_bit(&conn, bit, from, to)
            }
            .map_err(|e| internal_err("DB query", e))?;

            let period_start = (to / 2016) * 2016;
            let period_end = period_start + 2015;
            let period_blocks = if use_locktime {
                super::db::query_signaling_locktime(
                    &conn,
                    period_start,
                    period_end,
                )
            } else {
                super::db::query_signaling_bit(
                    &conn,
                    bit,
                    period_start,
                    period_end,
                )
            }
            .map_err(|e| internal_err("DB query", e))?;

            let signaled_count =
                period_blocks.iter().filter(|b| b.signaled).count() as u64;
            let raw_total = period_blocks.len() as u64;
            // "Blocks since adjustment" excludes the retarget block itself
            // (matches mempool.space).
            let mined = if raw_total > 0 { raw_total - 1 } else { 0 };
            let pct = if mined > 0 {
                signaled_count as f64 / mined as f64 * 100.0
            } else {
                0.0
            };

            let signaling_blocks: Vec<SignalingBlock> = blocks
                .into_iter()
                .map(|b| SignalingBlock {
                    height: b.height,
                    timestamp: b.timestamp,
                    signaled: b.signaled,
                    miner: b.miner,
                })
                .collect();

            let period_stats = PeriodStats {
                period_start,
                period_end,
                total_blocks: mined,
                signaled_count,
                signaled_pct: pct,
            };

            Ok::<_, ServerFnError>((signaling_blocks, period_stats))
        })
        .await
}

#[server(prefix = "/api", endpoint = "stats_signaling_periods")]
pub async fn fetch_signaling_periods(
    bit: u32,
    method: String,
) -> Result<Vec<SignalingPeriod>, ServerFnError> {
    let state = state().await?;

    // Cache key: "bit:4" or "locktime"
    let cache_key = if method == "locktime" {
        "locktime".to_string()
    } else {
        format!("bit:{bit}")
    };

    // Cache stores the DB rows (Vec<db::SignalingPeriod>); we map to the
    // public type only on the way out so cache hits and misses share one
    // storage shape.
    let periods = state
        .signaling_periods_cache
        .clone()
        .get_or_compute(cache_key, || async move {
            let conn =
                state.db.get().map_err(|e| internal_err("DB pool", e))?;
            let use_locktime = method == "locktime";
            let rows = if use_locktime {
                super::db::query_signaling_periods_locktime(&conn)
            } else {
                super::db::query_signaling_periods_bit(&conn, bit)
            }
            .map_err(|e| internal_err("DB query", e))?;
            Ok::<_, ServerFnError>(rows)
        })
        .await?;

    Ok(periods
        .into_iter()
        .map(|p| SignalingPeriod {
            start_height: p.start_height,
            end_height: p.end_height,
            signaled_count: p.signaled_count,
            total_blocks: p.total_blocks,
            signaled_pct: p.signaled_pct,
        })
        .collect())
}

#[server(prefix = "/api", endpoint = "stats_miner_dominance")]
pub async fn fetch_miner_dominance(
    from: u64,
    to: u64,
) -> Result<Vec<MinerShare>, ServerFnError> {
    let conn = conn().await?;
    let rows = super::db::query_miner_dominance(&conn, from, to)
        .map_err(|e| internal_err("DB query", e))?;
    let total: u64 = rows.iter().map(|r| r.count).sum();
    Ok(rows
        .into_iter()
        .map(|r| MinerShare {
            miner: r.miner,
            count: r.count,
            percentage: if total > 0 {
                (r.count as f64 / total as f64 * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            },
        })
        .collect())
}

#[server(prefix = "/api", endpoint = "stats_miner_dominance_daily")]
pub async fn fetch_miner_dominance_daily(
    from_ts: u64,
    to_ts: u64,
) -> Result<Vec<MinerShare>, ServerFnError> {
    let conn = conn().await?;
    let rows = super::db::query_miner_dominance_daily(&conn, from_ts, to_ts)
        .map_err(|e| internal_err("DB query", e))?;
    let total: u64 = rows.iter().map(|r| r.count).sum();
    Ok(rows
        .into_iter()
        .map(|r| MinerShare {
            miner: r.miner,
            count: r.count,
            percentage: if total > 0 {
                (r.count as f64 / total as f64 * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            },
        })
        .collect())
}

/// Empty blocks per calendar month, aggregated in SQL.
///
/// Replaces a per-row query that returned every empty block in range: 89,926
/// rows at ALL, which the client then folded into ~210 monthly bars. Both
/// empty-block charts only ever group, so the rows were never needed.
#[server(prefix = "/api", endpoint = "stats_empty_blocks_monthly")]
pub async fn fetch_empty_blocks_monthly(
    from: u64,
    to: u64,
) -> Result<Vec<HistogramBucket>, ServerFnError> {
    if from > to {
        return Err(ServerFnError::new("Invalid block range"));
    }
    let conn = conn().await?;
    let rows = super::db::query_empty_blocks_monthly(&conn, from, to)
        .map_err(|e| internal_err("DB query", e))?;
    Ok(rows
        .into_iter()
        .map(|(label, count)| HistogramBucket { label, count })
        .collect())
}

/// Empty blocks per mining pool, aggregated in SQL, highest first.
#[server(prefix = "/api", endpoint = "stats_empty_blocks_by_pool")]
pub async fn fetch_empty_blocks_by_pool(
    from: u64,
    to: u64,
) -> Result<Vec<HistogramBucket>, ServerFnError> {
    if from > to {
        return Err(ServerFnError::new("Invalid block range"));
    }
    let conn = conn().await?;
    let rows = super::db::query_empty_blocks_by_pool(&conn, from, to)
        .map_err(|e| internal_err("DB query", e))?;
    Ok(rows
        .into_iter()
        .map(|(label, count)| HistogramBucket { label, count })
        .collect())
}

#[server(prefix = "/api", endpoint = "stats_price_history")]
pub async fn fetch_price_history(
    from_ts: u64,
    to_ts: u64,
) -> Result<Vec<PricePoint>, ServerFnError> {
    // Silence unused warnings — range filtering now happens client-side
    let _ = (from_ts, to_ts);

    let state = state().await?;

    // Full dataset cached as a singleton; the from/to args are
    // accepted for API stability but the cache returns the entire
    // set regardless and clients filter as needed.
    state
        .price_history_cache
        .clone()
        .get_or_compute((), || async move {
            let prices = state
                .rpc
                .fetch_price_history_all()
                .await
                .map_err(|e| internal_err("Price history", e))?;
            Ok::<_, ServerFnError>(
                prices
                    .into_iter()
                    .map(|(ts_ms, price)| PricePoint {
                        timestamp_ms: ts_ms,
                        price_usd: price,
                    })
                    .collect(),
            )
        })
        .await
}

#[server(prefix = "/api", endpoint = "stats_block_timestamp")]
pub async fn fetch_block_timestamp(
    height: u64,
) -> Result<Option<u64>, ServerFnError> {
    let state = state().await?;

    // Block timestamps are immutable, cache forever. Conditional cache:
    // only insert positive results so a not-yet-existent height won't
    // cache None (the height may exist later as the chain extends).
    if let Some(ts) = state.block_ts_cache.get(&height) {
        return Ok(Some(ts));
    }
    let conn = state.db.get().map_err(|e| internal_err("DB pool", e))?;
    let result = super::db::query_block_timestamp(&conn, height)
        .map_err(|e| internal_err("DB query", e))?;
    if let Some(ts) = result {
        state.block_ts_cache.insert(height, ts);
    }
    Ok(result)
}

#[server(prefix = "/api", endpoint = "mining_price_summary")]
pub async fn fetch_mining_price_summary(
    from_ts: u64,
    to_ts: u64,
) -> Result<MiningPriceSummary, ServerFnError> {
    let conn = conn().await?;

    // Mining dominance
    let miners = super::db::query_miner_dominance_daily(&conn, from_ts, to_ts)
        .map_err(|e| internal_err("DB query", e))?;
    let total_mined: u64 = miners.iter().map(|m| m.count).sum();
    let (top_name, top_blocks) = miners
        .first()
        .map(|m| (m.miner.clone(), m.count))
        .unwrap_or_else(|| ("Unknown".to_string(), 0));
    let top_pct = if total_mined > 0 {
        top_blocks as f64 / total_mined as f64 * 100.0
    } else {
        0.0
    };
    let pool_count = miners.len() as u64;

    // Price context — use cached price history
    let prices = fetch_price_history(0, 4_000_000_000)
        .await
        .unwrap_or_default();
    let from_ms = from_ts * 1000;
    let to_ms = to_ts * 1000;

    // Find closest price points to range boundaries
    let price_start = prices
        .iter()
        .filter(|p| p.timestamp_ms >= from_ms)
        .map(|p| p.price_usd)
        .next()
        .unwrap_or(0.0);
    let price_end = prices
        .iter()
        .rev()
        .filter(|p| p.timestamp_ms <= to_ms)
        .map(|p| p.price_usd)
        .next()
        .unwrap_or(0.0);
    let price_change_pct = if price_start > 0.0 {
        (price_end - price_start) / price_start * 100.0
    } else {
        0.0
    };

    Ok(MiningPriceSummary {
        top_pool_name: top_name,
        top_pool_blocks: top_blocks,
        top_pool_pct: top_pct,
        pool_count,
        price_start,
        price_end,
        price_change_pct,
    })
}

/// Fetch block fullness histogram (10 buckets) for a timestamp range.
/// Computed server-side so ALL range works without sending 940k rows.
#[server(prefix = "/api", endpoint = "fullness_histogram")]
pub async fn fetch_fullness_histogram(
    from_ts: u64,
    to_ts: u64,
) -> Result<Vec<HistogramBucket>, ServerFnError> {
    if from_ts > to_ts {
        return Err(ServerFnError::new("Invalid timestamp range"));
    }
    let conn = conn().await?;
    let buckets = super::db::query_fullness_histogram(&conn, from_ts, to_ts)
        .map_err(|e| internal_err("DB query", e))?;
    Ok(buckets
        .into_iter()
        .map(|(label, count)| HistogramBucket { label, count })
        .collect())
}

/// Fetch block time distribution histogram (61 buckets) for a timestamp range.
#[server(prefix = "/api", endpoint = "block_time_histogram")]
pub async fn fetch_block_time_histogram(
    from_ts: u64,
    to_ts: u64,
) -> Result<Vec<HistogramBucket>, ServerFnError> {
    if from_ts > to_ts {
        return Err(ServerFnError::new("Invalid timestamp range"));
    }
    let conn = conn().await?;
    let buckets = super::db::query_block_time_histogram(&conn, from_ts, to_ts)
        .map_err(|e| internal_err("DB query", e))?;
    Ok(buckets
        .into_iter()
        .map(|(label, count)| HistogramBucket { label, count })
        .collect())
}

/// Documented pre-exchange BTC/USD prices, oldest first, used only when the
/// blockchain.info series has no point within two days of the target date.
/// Keys are zero-padded `YYYY-MM` (or bare `YYYY`) so lexicographic order is
/// chronological.
#[cfg(feature = "ssr")]
const EARLY_PRICES: &[(&str, f64)] = &[
    ("2009", 0.0),      // no market price
    ("2010-01", 0.0),   // no market
    ("2010-03", 0.003), // early BitcoinMarket.com trades
    ("2010-05", 0.004), // Pizza Day era (~$0.0041)
    ("2010-07", 0.05),  // Mt. Gox opens
    ("2010-08", 0.06),
    ("2010-10", 0.10),
    ("2010-11", 0.25), // brief spike
    ("2010-12", 0.25),
    ("2011-01", 0.30),
    ("2011-02", 1.00), // BTC reaches $1
];

/// Most recent entry in [`EARLY_PRICES`] at or before `year`-`month`, or 0.0
/// (rendered as "unavailable") for any month past the table's last entry.
///
/// The bound is the important part. This is a fallback for when the price
/// series has no point near the target date, and the series covers 2010-07-19
/// onward, so the table is the only source only for dates before that. But the
/// fallback also fires for *every* date when the price fetch fails outright,
/// and an unbounded carry-forward then reports the table's last entry, $1.00
/// from February 2011, as the price of a 2026 date. That is worse than
/// reporting nothing: the Almanac shows an em-dash and hides market cap for
/// 0.0, so "unknown" is representable and honest, whereas "$1" reads as a real
/// figure and is off by orders of magnitude.
///
/// So carry-forward is deliberately confined to the era the table documents.
/// Filling months after it would mean publishing a guess, and 2011 alone moved
/// far enough that the February figure does not describe the rest of it.
#[cfg(feature = "ssr")]
fn early_price_for(year: u32, month: u32) -> f64 {
    let year_month = format!("{}-{:02}", year, month);
    let Some((last_documented, _)) = EARLY_PRICES.last() else {
        return 0.0;
    };
    if year_month.as_str() > *last_documented {
        return 0.0;
    }
    EARLY_PRICES
        .iter()
        .rfind(|(prefix, _)| *prefix <= year_month.as_str())
        .map(|(_, price)| *price)
        .unwrap_or(0.0)
}

/// USD price for a date: the nearest series point within two days, else the
/// documented early-price table.
///
/// **A zero from the series is treated as absent, and that distinction is the
/// whole point.** The blockchain.info series carries entries with
/// `price_usd: 0.0` for dates before a liquid market existed, so `Some(0.0)`
/// means "the series covers this date and had nothing to report", not "the
/// price was zero". Accepting it as a value short-circuits the early-price
/// table for exactly the window that table exists to serve: Pizza Day
/// rendered as unavailable while the table held $0.004 for 2010-05.
///
/// An earlier version of this claimed the series "has no data" before 2011.
/// It has rows; their values are zero. Absent and zero are different, and
/// conflating them is what broke the headline case of the fix that introduced
/// the table lookup.
#[cfg(feature = "ssr")]
fn resolve_price_usd(
    prices: &[PricePoint],
    target_ms: u64,
    year: u32,
    month: u32,
) -> f64 {
    let two_days_ms = 2 * 86_400 * 1000;
    let nearest = prices
        .iter()
        .filter(|p| {
            p.timestamp_ms >= target_ms.saturating_sub(two_days_ms)
                && p.timestamp_ms <= target_ms + two_days_ms
        })
        .min_by_key(|p| {
            (p.timestamp_ms as i64 - target_ms as i64).unsigned_abs()
        })
        .map(|p| p.price_usd)
        .filter(|p| *p > 0.0);

    match nearest {
        Some(p) => p,
        None => early_price_for(year, month),
    }
}

#[server(prefix = "/api", endpoint = "on_this_day")]
pub async fn fetch_on_this_day(
    month: u32,
    day: u32,
) -> Result<OnThisDayData, ServerFnError> {
    let conn = conn().await?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(ServerFnError::new("Invalid date"));
    }
    let month_day = format!("{:02}-{:02}", month, day);
    let rows = super::db::query_on_this_day(&conn, &month_day)
        .map_err(|e| internal_err("DB query", e))?;

    // Fetch price history for annotation
    let prices = fetch_price_history(0, 4_000_000_000)
        .await
        .unwrap_or_default();

    // Notable Bitcoin events by (year, MM-DD, description)
    // (year, MM-DD, title, context, optional_block_height)
    let notable_dates: Vec<(u32, &str, &str, &str, Option<u64>)> = vec![
        (2009, "01-03", "Genesis Block mined",
            "Block 0 contains the famous headline: \"The Times 03/Jan/2009 Chancellor on brink of second bailout for banks\"",
            Some(0)),
        (2009, "01-12", "First BTC transaction (Satoshi \u{2192} Hal Finney)",
            "Satoshi sent 10 BTC to Hal Finney in block 170. Finney reportedly ran Bitcoin on his laptop while fighting ALS",
            Some(170)),
        (2010, "05-22", "Bitcoin Pizza Day",
            "Laszlo Hanyecz paid 10,000 BTC for two Papa John's pizzas, the first known real-world Bitcoin purchase",
            None),
        (2010, "07-17", "Mt. Gox exchange opens",
            "Originally a Magic: The Gathering card trading site. Would grow to handle 70% of all BTC trades before its collapse",
            None),
        (2010, "12-12", "Satoshi's last public post",
            "Satoshi's final message on BitcoinTalk discussed DoS attack prevention. He was never heard from publicly again",
            None),
        (2011, "02-09", "BTC reaches $1",
            "Bitcoin achieved dollar parity, giving it a market cap of roughly $6 million",
            None),
        (2011, "06-19", "Mt. Gox hack",
            "A hacker compromised an auditor's account and dumped thousands of BTC, crashing the price from $17 to $0.01",
            None),
        (2012, "11-28", "First halving",
            "Block reward dropped from 50 to 25 BTC. About 10.5 million BTC (50% of supply) had been mined in just 4 years",
            Some(210_000)),
        (2013, "03-28", "BTC market cap reaches $1 billion",
            "Bitcoin crossed the billion-dollar threshold at ~$92 per coin with 10.9 million BTC in circulation",
            None),
        (2013, "11-29", "BTC reaches $1,000",
            "Driven by Chinese exchange demand, Bitcoin crossed $1,000 for the first time, a 250,000x increase from Pizza Day",
            None),
        (2014, "02-07", "Mt. Gox halts withdrawals",
            "The exchange suspended all withdrawals, later revealing 850,000 BTC (~$450M) had been stolen. It filed for bankruptcy weeks later",
            None),
        (2016, "07-09", "Second halving",
            "Block reward dropped from 25 to 12.5 BTC. Price was ~$650 and would reach $20K within 18 months",
            Some(420_000)),
        (2017, "08-01", "Bitcoin Cash fork",
            "A contentious hard fork created BCH with 8MB blocks. Bitcoin kept its 1MB+SegWit approach. The \"block size war\" ended",
            None),
        (2017, "08-24", "SegWit activates",
            "<a href='https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki' target='_blank' class='text-[#f7931a] hover:underline'>BIP-141</a> activated at block 481,824. Segregated Witness fixed transaction malleability and enabled the Lightning Network",
            Some(481_824)),
        (2017, "11-08", "SegWit2x cancelled",
            "The New York Agreement plan to double the block size was abandoned due to lack of consensus. A pivotal moment for Bitcoin's governance",
            None),
        (2017, "12-17", "BTC reaches $20,000",
            "The peak of the 2017 bull run. FOMO was so intense that Coinbase repeatedly crashed under traffic",
            None),
        (2020, "03-12", "Black Thursday",
            "Bitcoin crashed 50% in hours alongside global markets as COVID panic hit. Liquidation cascades wiped $1B in leveraged positions",
            None),
        (2020, "05-11", "Third halving",
            "Block reward dropped from 12.5 to 6.25 BTC. Price was ~$8,600 and would reach $69K within 18 months",
            Some(630_000)),
        (2021, "02-08", "Tesla buys $1.5B in BTC",
            "Tesla's SEC filing revealed a massive Bitcoin purchase, legitimizing BTC as a corporate treasury asset",
            None),
        (2021, "05-19", "China announces mining ban",
            "China ordered miners to shut down, triggering the largest hashrate migration in Bitcoin's history. Over 50% of mining moved abroad",
            None),
        (2021, "06-09", "El Salvador adopts BTC as legal tender",
            "The first country to make Bitcoin legal tender. President Bukele pushed the \"Bitcoin Law\" through congress",
            None),
        (2021, "09-07", "El Salvador BTC law takes effect",
            "Bitcoin became legal tender alongside the US dollar. The government launched the Chivo wallet with $30 in BTC for every citizen",
            None),
        (2021, "11-10", "BTC ATH ~$69,000",
            "The peak of the 2021 cycle. Bitcoin's market cap briefly exceeded $1.2 trillion",
            None),
        (2021, "11-14", "Taproot activates",
            "<a href='https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki' target='_blank' class='text-[#f7931a] hover:underline'>BIP-341</a> activated at block 709,632. The largest Bitcoin upgrade since SegWit, enabling more private and efficient smart contracts",
            Some(709_632)),
        (2022, "11-11", "FTX files for bankruptcy",
            "Sam Bankman-Fried's exchange collapsed after revelations of massive fraud. ~$8B in customer funds were misused. BTC dropped to $16K",
            None),
        (2023, "01-21", "Ordinals inscriptions launch",
            "Casey Rodarmor launched Ordinal Theory, enabling NFT-like inscriptions in Bitcoin witness data. Sparked a fierce debate about block space usage",
            None),
        (2024, "01-10", "First spot Bitcoin ETFs approved",
            "The SEC approved 11 spot Bitcoin ETFs after a decade of rejections. Over $4B in volume traded on day one",
            None),
        (2024, "03-14", "BTC reaches $73,000",
            "A new all-time high driven by ETF inflows. Bitcoin surpassed silver's market cap",
            None),
        (2024, "04-20", "Fourth halving + Runes launch",
            "Block reward dropped from 6.25 to 3.125 BTC. The Runes protocol launched simultaneously, causing a fee spike as users minted tokens",
            Some(840_000)),
        (2024, "12-05", "BTC breaks $100,000",
            "A psychological milestone 15 years in the making. From $0 to six figures",
            None),
        (2025, "10-06", "BTC ATH ~$126,000",
            "The current all-time high. Bitcoin's market cap surpassed $2.5 trillion",
            None),
    ];

    let years: Vec<OnThisDayYear> = rows
        .into_iter()
        .map(
            |(
                year,
                block_count,
                total_tx,
                total_fees,
                avg_size,
                avg_weight,
                inscriptions,
                runes,
                segwit_txs,
                taproot_outputs,
                first_block,
                last_block,
            )| {
                // Closest price for this date. See `resolve_price_usd`.
                let price_usd =
                    chrono::NaiveDate::from_ymd_opt(year as i32, month, day)
                        .and_then(|d| d.and_hms_opt(12, 0, 0))
                        .map(|dt| {
                            resolve_price_usd(
                                &prices,
                                dt.and_utc().timestamp() as u64 * 1000,
                                year,
                                month,
                            )
                        })
                        .unwrap_or(0.0);

                // Collect events for this date AND year
                let mut events = Vec::new();
                for (event_year, date, title, context, block) in &notable_dates
                {
                    if *date == month_day && *event_year == year {
                        events.push(NotableEvent {
                            title: title.to_string(),
                            context: context.to_string(),
                            block: *block,
                        });
                    }
                }

                let segwit_pct = if total_tx > block_count {
                    segwit_txs as f64 / (total_tx - block_count) as f64 * 100.0
                } else {
                    0.0
                };

                let avg_weight_util = avg_weight / 4_000_000.0 * 100.0;

                OnThisDayYear {
                    year,
                    block_count,
                    total_tx,
                    total_fees,
                    avg_block_size: avg_size,
                    avg_weight_util,
                    total_inscriptions: inscriptions,
                    total_runes: runes,
                    segwit_pct,
                    taproot_outputs,
                    price_usd,
                    events,
                    first_block,
                    last_block,
                }
            },
        )
        .collect();

    Ok(OnThisDayData { month, day, years })
}

#[server(prefix = "/api", endpoint = "range_summary")]
pub async fn fetch_range_summary(
    from_ts: u64,
    to_ts: u64,
) -> Result<RangeSummary, ServerFnError> {
    if from_ts > to_ts {
        return Err(ServerFnError::new("Invalid timestamp range"));
    }

    let state = state().await?;

    state
        .range_summary_cache
        .clone()
        .get_or_compute((from_ts, to_ts), || async move {
            let conn =
                state.db.get().map_err(|e| internal_err("DB pool", e))?;
            super::db::query_range_summary(&conn, from_ts, to_ts)
                .map_err(|e| internal_err("DB query", e))
        })
        .await
}

#[server(prefix = "/api", endpoint = "extremes")]
pub async fn fetch_extremes(
    from_ts: u64,
    to_ts: u64,
) -> Result<ExtremesData, ServerFnError> {
    if from_ts > to_ts {
        return Err(ServerFnError::new("Invalid timestamp range"));
    }

    let state = state().await?;

    state
        .extremes_cache
        .clone()
        .get_or_compute((from_ts, to_ts), || async move {
            let conn =
                state.db.get().map_err(|e| internal_err("DB pool", e))?;
            super::db::query_extremes_with_heights(&conn, from_ts, to_ts)
                .map_err(|e| internal_err("DB query", e))
        })
        .await
}

// ═══════════════════════════════════════════════════════════════════════════
// Notable Transactions (Whale Watch) server functions
// ═══════════════════════════════════════════════════════════════════════════

#[server(prefix = "/api", endpoint = "notable_txs")]
pub async fn fetch_notable_txs(
    filter: NotableTxFilter,
    limit: u64,
    offset: u64,
) -> Result<NotableTxPage, ServerFnError> {
    // Cap limit to prevent abuse
    let limit = limit.min(500);
    let conn = conn().await?;

    let db_filter = super::db::NotableFilter {
        notable_type: filter.notable_type.clone(),
        since: filter.since,
        until: filter.until,
        min_value_usd: filter.min_value_usd,
        confirmed_only: filter.confirmed_only,
        unconfirmed_only: filter.unconfirmed_only,
    };

    let rows = super::db::query_notable_txs(&conn, &db_filter, limit, offset)
        .map_err(|e| internal_err("DB query", e))?;
    let total = super::db::count_notable_txs(&conn, &db_filter)
        .map_err(|e| internal_err("DB count", e))?;

    let items: Vec<NotableTxInfo> =
        rows.into_iter().map(NotableTxInfo::from).collect();

    Ok(NotableTxPage {
        items,
        total,
        offset,
        limit,
    })
}

#[server(prefix = "/api", endpoint = "notable_stats")]
pub async fn fetch_notable_stats(
    since: u64,
) -> Result<NotableStatsInfo, ServerFnError> {
    let conn = conn().await?;

    let stats = super::db::query_notable_stats(&conn, since)
        .map_err(|e| internal_err("DB query", e))?;

    Ok(NotableStatsInfo {
        total_count: stats.total_count,
        total_value_usd: stats.total_value_usd,
        by_type: stats.by_type,
        top_value_usd: stats.top_value_usd,
        top_txid: stats.top_txid,
    })
}

/// Fetch top N notable txs by USD value in a time window (leaderboard).
#[server(prefix = "/api", endpoint = "notable_top")]
pub async fn fetch_notable_top(
    since: u64,
    limit: u64,
) -> Result<Vec<NotableTxInfo>, ServerFnError> {
    let limit = limit.min(50);
    let conn = conn().await?;
    let rows = super::db::query_notable_top(&conn, since, limit)
        .map_err(|e| internal_err("DB query", e))?;
    Ok(rows.into_iter().map(NotableTxInfo::from).collect())
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;

    /// The early-price table is a "last known price" lookup. It was written as
    /// an exact prefix match, so any month not literally listed returned
    /// $0.00 even with a known earlier price: 2010-02, 2010-04, 2010-06,
    /// 2010-09 and every month of 2011 from March on. The 2010 gaps are the
    /// damaging ones, since that window predates the blockchain.info series
    /// and this table is the only source, so the Almanac showed $0.00 for
    /// dates on either side of Pizza Day while showing $0.004 for Pizza Day.
    #[test]
    fn early_price_carries_forward_the_last_known_value() {
        // Explicitly listed months are unchanged.
        assert_eq!(early_price_for(2010, 3), 0.003);
        assert_eq!(early_price_for(2010, 5), 0.004);
        assert_eq!(early_price_for(2011, 2), 1.00);

        // Gaps inside the table's era inherit the previous known price
        // instead of zeroing. These are the damaging ones: the price series
        // starts 2010-07-19, so for these months the table is the only source
        // and the fallback always fires.
        assert_eq!(early_price_for(2010, 4), 0.003, "April inherits March");
        assert_eq!(early_price_for(2010, 6), 0.004, "June inherits May");
        assert_eq!(early_price_for(2010, 9), 0.06, "September inherits August");

        // Before any market existed, and before the table starts.
        assert_eq!(early_price_for(2009, 6), 0.0);
        assert_eq!(early_price_for(2008, 1), 0.0, "predates the table");

        // Monotonic within the era: a later month can never report a lower
        // price than an earlier one, which is what a carry-forward guarantees.
        let mut prev = 0.0;
        for (y, m) in [(2009, 1), (2010, 1), (2010, 6), (2010, 12), (2011, 2)] {
            let p = early_price_for(y, m);
            assert!(p >= prev, "{y}-{m:02} fell to {p} from {prev}");
            prev = p;
        }
    }

    /// A zero in the price series must not be read as a price.
    ///
    /// This is the bug that broke the headline case of the early-price fix:
    /// the blockchain.info series carries rows with `price_usd: 0.0` for dates
    /// before a liquid market, so the nearest-point lookup returned
    /// `Some(0.0)` for 2010-05-22, short-circuited the early-price table, and
    /// the Almanac rendered Pizza Day as unavailable while the table held
    /// $0.004. Absent and zero are different things.
    #[test]
    fn a_zero_in_the_series_falls_back_to_the_early_table() {
        // 2010-05-22 12:00 UTC, the target the Almanac asks for.
        let pizza_day_ms = 1_274_529_600_000u64;

        // The series covers the date but reports zero, as it really does.
        let zeroed = vec![PricePoint {
            timestamp_ms: pizza_day_ms,
            price_usd: 0.0,
        }];
        assert_eq!(
            resolve_price_usd(&zeroed, pizza_day_ms, 2010, 5),
            0.004,
            "a zero from the series must not beat the documented table"
        );

        // No coverage at all: same answer, via the same fallback.
        assert_eq!(resolve_price_usd(&[], pizza_day_ms, 2010, 5), 0.004);

        // A real price in range wins, which is the whole point of preferring
        // the series for dates it actually covers.
        let real = vec![PricePoint {
            timestamp_ms: pizza_day_ms,
            price_usd: 123.45,
        }];
        assert_eq!(resolve_price_usd(&real, pizza_day_ms, 2010, 5), 123.45);

        // Outside the two-day window the series point is ignored.
        let far = vec![PricePoint {
            timestamp_ms: pizza_day_ms + 5 * 86_400 * 1000,
            price_usd: 123.45,
        }];
        assert_eq!(resolve_price_usd(&far, pizza_day_ms, 2010, 5), 0.004);

        // A zeroed series past the table's era still reports unknown, so the
        // bound added earlier is not undone by this change.
        assert_eq!(resolve_price_usd(&zeroed, pizza_day_ms, 2024, 4), 0.0);
    }

    /// Carry-forward must stop at the end of the table's era.
    ///
    /// The fallback fires for every date when the price fetch fails, not just
    /// for pre-exchange ones, so an unbounded version reported February 2011's
    /// $1.00 as the price of any modern date. 0.0 renders as an em-dash with
    /// market cap hidden, so "unknown" is representable; "$1" is not merely
    /// missing, it is wrong by orders of magnitude and looks deliberate.
    #[test]
    fn early_price_does_not_carry_past_the_documented_era() {
        assert_eq!(early_price_for(2011, 2), 1.00, "last documented month");
        assert_eq!(early_price_for(2011, 3), 0.0, "one month past the table");
        assert_eq!(early_price_for(2013, 11), 0.0);
        assert_eq!(early_price_for(2024, 4), 0.0, "halving day, not $1");
        assert_eq!(early_price_for(2026, 9), 0.0, "today, not $1");
    }
}
