//! Server functions that wrap stats DB queries for Leptos.
//!
//! Each function extracts the shared `StatsState` from the Axum extensions,
//! queries the database (or RPC), and returns shared types from `super::types`.

use leptos::prelude::*;
use leptos::server;

use super::types::*;

#[cfg(feature = "ssr")]
use axum::extract::Extension;

#[server(prefix = "/api", endpoint = "stats_summary")]
pub async fn fetch_stats_summary() -> Result<StatsSummary, ServerFnError> {
    use std::time::Instant;

    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;

    // Return cached result if fresh (< 60 seconds)
    {
        let cached = state
            .stats_summary_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some((ref summary, ref ts)) = *cached {
            if ts.elapsed().as_secs() < 60 {
                return Ok(summary.clone());
            }
        }
    }

    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let stats = super::db::query_stats(&conn)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;
    let result = match stats {
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
    };

    // Cache the result
    *state
        .stats_summary_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner()) =
        Some((result.clone(), Instant::now()));

    Ok(result)
}

#[server(prefix = "/api", endpoint = "stats_blocks")]
pub async fn fetch_blocks(
    from: u64,
    to: u64,
) -> Result<Vec<BlockSummary>, ServerFnError> {
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let rows = super::db::query_blocks(&conn, from, to)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|r| BlockSummary {
            height: r.height,
            hash: r.hash,
            timestamp: r.timestamp,
            tx_count: r.tx_count,
            size: r.size,
            weight: r.weight,
            difficulty: r.difficulty,
            total_fees: r.total_fees,
            median_fee: r.median_fee,
            median_fee_rate: r.median_fee_rate,
            segwit_spend_count: r.segwit_spend_count,
            taproot_spend_count: r.taproot_spend_count,
            p2pk_count: r.p2pk_count,
            p2pkh_count: r.p2pkh_count,
            p2sh_count: r.p2sh_count,
            p2wpkh_count: r.p2wpkh_count,
            p2wsh_count: r.p2wsh_count,
            p2tr_count: r.p2tr_count,
            multisig_count: r.multisig_count,
            unknown_script_count: r.unknown_script_count,
            input_count: r.input_count,
            output_count: r.output_count,
            rbf_count: r.rbf_count,
            witness_bytes: r.witness_bytes,
            inscription_count: r.inscription_count,
            inscription_bytes: r.inscription_bytes,
            brc20_count: r.brc20_count,
            op_return_count: r.op_return_count,
            op_return_bytes: r.op_return_bytes,
            runes_count: r.runes_count,
            runes_bytes: r.runes_bytes,
            omni_count: r.omni_count,
            omni_bytes: r.omni_bytes,
            counterparty_count: r.counterparty_count,
            counterparty_bytes: r.counterparty_bytes,
            data_carrier_count: r.data_carrier_count,
            data_carrier_bytes: r.data_carrier_bytes,
            taproot_keypath_count: r.taproot_keypath_count,
            taproot_scriptpath_count: r.taproot_scriptpath_count,
            total_output_value: r.total_output_value,
            total_input_value: r.total_input_value,
            fee_rate_p10: r.fee_rate_p10,
            fee_rate_p90: r.fee_rate_p90,
            stamps_count: r.stamps_count,
            largest_tx_size: r.largest_tx_size,
        })
        .collect())
}

/// Fetch blocks by timestamp range (for custom date ranges).
#[server(prefix = "/api", endpoint = "stats_blocks_by_ts")]
pub async fn fetch_blocks_by_ts(
    from_ts: u64,
    to_ts: u64,
) -> Result<Vec<BlockSummary>, ServerFnError> {
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let rows = super::db::query_blocks_by_ts(&conn, from_ts, to_ts)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|r| BlockSummary {
            height: r.height,
            hash: r.hash,
            timestamp: r.timestamp,
            tx_count: r.tx_count,
            size: r.size,
            weight: r.weight,
            difficulty: r.difficulty,
            total_fees: r.total_fees,
            median_fee: r.median_fee,
            median_fee_rate: r.median_fee_rate,
            segwit_spend_count: r.segwit_spend_count,
            taproot_spend_count: r.taproot_spend_count,
            p2pk_count: r.p2pk_count,
            p2pkh_count: r.p2pkh_count,
            p2sh_count: r.p2sh_count,
            p2wpkh_count: r.p2wpkh_count,
            p2wsh_count: r.p2wsh_count,
            p2tr_count: r.p2tr_count,
            multisig_count: r.multisig_count,
            unknown_script_count: r.unknown_script_count,
            input_count: r.input_count,
            output_count: r.output_count,
            rbf_count: r.rbf_count,
            witness_bytes: r.witness_bytes,
            inscription_count: r.inscription_count,
            inscription_bytes: r.inscription_bytes,
            brc20_count: r.brc20_count,
            op_return_count: r.op_return_count,
            op_return_bytes: r.op_return_bytes,
            runes_count: r.runes_count,
            runes_bytes: r.runes_bytes,
            omni_count: r.omni_count,
            omni_bytes: r.omni_bytes,
            counterparty_count: r.counterparty_count,
            counterparty_bytes: r.counterparty_bytes,
            data_carrier_count: r.data_carrier_count,
            data_carrier_bytes: r.data_carrier_bytes,
            taproot_keypath_count: r.taproot_keypath_count,
            taproot_scriptpath_count: r.taproot_scriptpath_count,
            total_output_value: r.total_output_value,
            total_input_value: r.total_input_value,
            fee_rate_p10: r.fee_rate_p10,
            fee_rate_p90: r.fee_rate_p90,
            stamps_count: r.stamps_count,
            largest_tx_size: r.largest_tx_size,
        })
        .collect())
}

#[server(prefix = "/api", endpoint = "stats_block_detail")]
pub async fn fetch_block_detail(
    height: u64,
) -> Result<Option<BlockDetail>, ServerFnError> {
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let row = super::db::query_block_by_height(&conn, height)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;
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
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let size = super::db::query_cumulative_size(&conn, below_height)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;
    Ok(size)
}

/// Total block data size (bytes) for all blocks before a given timestamp.
/// Used by the chain size overlay for custom date ranges.
#[server(prefix = "/api", endpoint = "stats_cumulative_size_ts")]
pub async fn fetch_cumulative_size_before_ts(
    before_ts: u64,
) -> Result<u64, ServerFnError> {
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let size = super::db::query_cumulative_size_before_ts(&conn, before_ts)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;
    Ok(size)
}

#[server(prefix = "/api", endpoint = "stats_live")]
pub async fn fetch_live_stats() -> Result<LiveStats, ServerFnError> {
    use std::time::Instant;

    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;

    // Return cached result if fresh (< 10 seconds old)
    {
        let cached = state.live_cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((ref stats, ref ts)) = *cached {
            if ts.elapsed().as_secs() < 10 {
                return Ok(stats.clone());
            }
        }
    }

    // Get block height + difficulty from the DB (always current from 60s poll).
    // This avoids stale data when getblockchaininfo RPC is slow/fails.
    let db_stats = {
        let conn = state
            .db
            .get()
            .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
        super::db::query_stats(&conn)
            .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?
    };
    let db_height = db_stats.as_ref().map(|s| s.max_height).unwrap_or(0);
    let db_timestamp =
        db_stats.as_ref().map(|s| s.latest_timestamp).unwrap_or(0);

    // Parallelize RPC calls — all are non-fatal (fall back to defaults)
    let (blockchain_res, mempool_res, hashrate_res, fee_res) = tokio::join!(
        state.rpc.get_blockchain_info(),
        state.rpc.get_mempool_info(),
        state.rpc.get_network_hashps(),
        state.rpc.estimate_smart_fee(1),
    );

    // Use RPC blockchain info if available, but override block height with DB
    // (DB is always up-to-date from the poll, RPC might be stale/failed)
    let blockchain = blockchain_res.unwrap_or_else(|e| {
        tracing::warn!("Failed to fetch blockchain info: {e}");
        super::rpc::BlockchainInfo {
            blocks: db_height,
            chain: "main".to_string(),
            difficulty: 0.0,
            verification_progress: 1.0,
            size_on_disk: 0,
            bestblockhash: String::new(),
            time: db_timestamp,
        }
    });
    // Always use DB height — it's the source of truth (updated by poll)
    let block_height = db_height.max(blockchain.blocks);

    let mempool = mempool_res.unwrap_or_else(|e| {
        tracing::warn!("Failed to fetch mempool info: {e}");
        super::rpc::MempoolInfo {
            size: 0,
            bytes: 0,
            usage: 0,
            total_fee: 0.0,
            maxmempool: 300_000_000,
            mempoolminfee: 0.0,
        }
    });
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
        use std::sync::atomic::Ordering;
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
            // We won the refresh race — fetch and update cache
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
            // Cache hit or another request is already refreshing — use cached value
            cached.map(|(p, _)| p.usd).unwrap_or(0.0)
        }
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
    let utxo_count = state
        .utxo_count
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .unwrap_or(0);

    let result = LiveStats {
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
    };

    // Cache the result
    *state.live_cache.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((result.clone(), Instant::now()));

    Ok(result)
}

#[server(prefix = "/api", endpoint = "stats_op_returns")]
pub async fn fetch_op_returns(
    from: u64,
    to: u64,
) -> Result<Vec<OpReturnBlock>, ServerFnError> {
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let rows = super::db::query_op_returns(&conn, from, to)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|r| OpReturnBlock {
            height: r.height,
            timestamp: r.timestamp,
            tx_count: r.tx_count,
            size: r.size,
            op_return_count: r.op_return_count,
            op_return_bytes: r.op_return_bytes,
            runes_count: r.runes_count,
            runes_bytes: r.runes_bytes,
            omni_count: r.omni_count,
            omni_bytes: r.omni_bytes,
            counterparty_count: r.counterparty_count,
            counterparty_bytes: r.counterparty_bytes,
            data_carrier_count: r.data_carrier_count,
            data_carrier_bytes: r.data_carrier_bytes,
        })
        .collect())
}

#[server(prefix = "/api", endpoint = "stats_daily_aggregates")]
pub async fn fetch_daily_aggregates(
    from_ts: u64,
    to_ts: u64,
) -> Result<Vec<DailyAggregate>, ServerFnError> {
    use std::time::Instant;

    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;

    // Return cached result if same range requested within 30s
    {
        let cached =
            state.daily_cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((ref f, ref t, ref data, ref ts)) = *cached {
            if *f == from_ts && *t == to_ts && ts.elapsed().as_secs() < 120 {
                return Ok(data.clone());
            }
        }
    }

    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let rows = super::db::query_daily_aggregates(&conn, from_ts, to_ts)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;
    let result: Vec<DailyAggregate> = rows
        .into_iter()
        .map(|r| DailyAggregate {
            date: r.date,
            block_count: r.block_count,
            avg_size: r.avg_size,
            avg_weight: r.avg_weight,
            avg_tx_count: r.avg_tx_count,
            avg_difficulty: r.avg_difficulty,
            total_op_return_count: r.total_op_return_count,
            total_op_return_bytes: r.total_op_return_bytes,
            total_runes_count: r.total_runes_count,
            total_runes_bytes: r.total_runes_bytes,
            total_omni_count: r.total_omni_count,
            total_omni_bytes: r.total_omni_bytes,
            total_counterparty_count: r.total_counterparty_count,
            total_counterparty_bytes: r.total_counterparty_bytes,
            total_data_carrier_count: r.total_data_carrier_count,
            total_data_carrier_bytes: r.total_data_carrier_bytes,
            total_fees: r.total_fees,
            avg_segwit_spend_count: r.avg_segwit_spend_count,
            avg_taproot_spend_count: r.avg_taproot_spend_count,
            avg_p2pk_count: r.avg_p2pk_count,
            avg_p2pkh_count: r.avg_p2pkh_count,
            avg_p2sh_count: r.avg_p2sh_count,
            avg_p2wpkh_count: r.avg_p2wpkh_count,
            avg_p2wsh_count: r.avg_p2wsh_count,
            avg_p2tr_count: r.avg_p2tr_count,
            avg_multisig_count: r.avg_multisig_count,
            avg_unknown_script_count: r.avg_unknown_script_count,
            avg_input_count: r.avg_input_count,
            avg_output_count: r.avg_output_count,
            avg_rbf_count: r.avg_rbf_count,
            avg_witness_bytes: r.avg_witness_bytes,
            avg_inscription_count: r.avg_inscription_count,
            avg_inscription_bytes: r.avg_inscription_bytes,
            avg_brc20_count: r.avg_brc20_count,
            avg_taproot_keypath_count: r.avg_taproot_keypath_count,
            avg_taproot_scriptpath_count: r.avg_taproot_scriptpath_count,
        })
        .collect();

    // Cache the result
    *state.daily_cache.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((from_ts, to_ts, result.clone(), Instant::now()));

    Ok(result)
}

#[server(prefix = "/api", endpoint = "stats_signaling")]
pub async fn fetch_signaling(
    bit: u32,
    method: String,
    from: u64,
    to: u64,
) -> Result<(Vec<SignalingBlock>, PeriodStats), ServerFnError> {
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let use_locktime = method == "locktime";

    let blocks = if use_locktime {
        super::db::query_signaling_locktime(&conn, from, to)
    } else {
        super::db::query_signaling_bit(&conn, bit, from, to)
    }
    .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

    // Period stats: retarget block boundary
    let period_start = (to / 2016) * 2016;
    let period_end = period_start + 2015;
    let period_blocks = if use_locktime {
        super::db::query_signaling_locktime(&conn, period_start, period_end)
    } else {
        super::db::query_signaling_bit(&conn, bit, period_start, period_end)
    }
    .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

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

    let signaling_blocks: Vec<SignalingBlock> = blocks
        .into_iter()
        .map(|b| SignalingBlock {
            height: b.height,
            timestamp: b.timestamp,
            signaled: b.signaled,
            miner: b.miner,
        })
        .collect();

    Ok((
        signaling_blocks,
        PeriodStats {
            period_start,
            period_end,
            total_blocks: mined,
            signaled_count,
            signaled_pct: pct,
        },
    ))
}

#[server(prefix = "/api", endpoint = "stats_signaling_periods")]
pub async fn fetch_signaling_periods(
    bit: u32,
    method: String,
) -> Result<Vec<SignalingPeriod>, ServerFnError> {
    use std::time::Instant;

    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;

    // Cache key: "bit:4" or "locktime"
    let cache_key = if method == "locktime" {
        "locktime".to_string()
    } else {
        format!("bit:{bit}")
    };

    // Return cached result if fresh (< 60 seconds)
    {
        let cached = state
            .signaling_periods_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some((ref key, ref data, ref ts)) = *cached {
            if key == &cache_key && ts.elapsed().as_secs() < 60 {
                return Ok(data
                    .iter()
                    .map(|p| SignalingPeriod {
                        start_height: p.start_height,
                        end_height: p.end_height,
                        signaled_count: p.signaled_count,
                        total_blocks: p.total_blocks,
                        signaled_pct: p.signaled_pct,
                    })
                    .collect());
            }
        }
    }

    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let use_locktime = method == "locktime";

    let periods = if use_locktime {
        super::db::query_signaling_periods_locktime(&conn)
    } else {
        super::db::query_signaling_periods_bit(&conn, bit)
    }
    .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

    // Cache the result
    *state
        .signaling_periods_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner()) =
        Some((cache_key, periods.clone(), Instant::now()));

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
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let rows = super::db::query_miner_dominance(&conn, from, to)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;
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
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let rows = super::db::query_miner_dominance_daily(&conn, from_ts, to_ts)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;
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

#[server(prefix = "/api", endpoint = "stats_empty_blocks")]
pub async fn fetch_empty_blocks(
    from: u64,
    to: u64,
) -> Result<Vec<EmptyBlock>, ServerFnError> {
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let rows = super::db::query_empty_blocks(&conn, from, to)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|(height, timestamp, miner)| EmptyBlock {
            height,
            timestamp,
            miner,
        })
        .collect())
}

#[server(prefix = "/api", endpoint = "stats_price_history")]
pub async fn fetch_price_history(
    from_ts: u64,
    to_ts: u64,
) -> Result<Vec<PricePoint>, ServerFnError> {
    use std::time::Instant;

    // Silence unused warnings — range filtering now happens client-side
    let _ = (from_ts, to_ts);

    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;

    // Return cached full dataset if fresh (< 1 hour old)
    {
        let cached = state
            .price_history_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some((_, _, ref data, ref ts)) = *cached {
            if ts.elapsed().as_secs() < 3600 {
                return Ok(data.clone());
            }
        }
    }

    // Fetch full history from blockchain.info (daily granularity, all time)
    let prices =
        state.rpc.fetch_price_history_all().await.map_err(|e| {
            ServerFnError::new(format!("Price history error: {e}"))
        })?;

    let all_points: Vec<PricePoint> = prices
        .into_iter()
        .map(|(ts_ms, price)| PricePoint {
            timestamp_ms: ts_ms,
            price_usd: price,
        })
        .collect();

    // Cache full dataset
    *state
        .price_history_cache
        .lock()
        .unwrap_or_else(|e| e.into_inner()) =
        Some((0, u64::MAX, all_points.clone(), Instant::now()));

    Ok(all_points)
}

#[server(prefix = "/api", endpoint = "stats_block_timestamp")]
pub async fn fetch_block_timestamp(
    height: u64,
) -> Result<Option<u64>, ServerFnError> {
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;

    // Block timestamps are immutable — cache forever
    {
        let cache = state
            .block_ts_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(&ts) = cache.get(&height) {
            return Ok(Some(ts));
        }
    }

    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    let result = super::db::query_block_timestamp(&conn, height)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

    // Cache if found
    if let Some(ts) = result {
        state
            .block_ts_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(height, ts);
    }

    Ok(result)
}

#[server(prefix = "/api", endpoint = "mining_price_summary")]
pub async fn fetch_mining_price_summary(
    from_ts: u64,
    to_ts: u64,
) -> Result<MiningPriceSummary, ServerFnError> {
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;

    // Mining dominance
    let miners = super::db::query_miner_dominance_daily(&conn, from_ts, to_ts)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;
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
    let prices = fetch_price_history(0, 4_000_000_000).await.unwrap_or_default();
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

#[server(prefix = "/api", endpoint = "on_this_day")]
pub async fn fetch_on_this_day(
    month: u32,
    day: u32,
) -> Result<OnThisDayData, ServerFnError> {
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;

    let month_day = format!("{:02}-{:02}", month, day);
    let rows = super::db::query_on_this_day(&conn, &month_day)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))?;

    // Fetch price history for annotation
    let prices = fetch_price_history(0, 4_000_000_000).await.unwrap_or_default();

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
            "A psychological milestone 15 years in the making. From $0 to six figures, powered by ETF adoption and institutional demand",
            None),
        (2025, "10-06", "BTC ATH ~$126,000",
            "The current all-time high. Bitcoin's market cap surpassed $2.5 trillion",
            None),
    ];

    let years: Vec<OnThisDayYear> = rows
        .into_iter()
        .map(|(year, block_count, total_tx, total_fees, avg_size, avg_weight,
               inscriptions, runes, segwit_txs, taproot_outputs, _total_tx2,
               first_block, last_block)| {
            // Pre-exchange era prices (before blockchain.info data)
            // These are well-documented historical prices for early Bitcoin
            let early_prices: &[(&str, f64)] = &[
                ("2009", 0.0),       // No market price
                ("2010-01", 0.0),    // No market
                ("2010-03", 0.003),  // Early BitcoinMarket.com trades
                ("2010-05", 0.004),  // Pizza Day era (~$0.0041)
                ("2010-07", 0.05),   // Mt. Gox opens
                ("2010-08", 0.06),
                ("2010-10", 0.10),
                ("2010-11", 0.25),   // Brief spike
                ("2010-12", 0.25),
                ("2011-01", 0.30),
                ("2011-02", 1.00),   // BTC reaches $1
            ];

            // Find closest price: try blockchain.info first, fall back to early prices
            let price_usd = chrono::NaiveDate::from_ymd_opt(year as i32, month, day)
                .and_then(|d| d.and_hms_opt(12, 0, 0))
                .map(|dt| {
                    let target_ms = dt.and_utc().timestamp() as u64 * 1000;
                    let two_days_ms = 2 * 86_400 * 1000;
                    // Try blockchain.info data first
                    let api_price = prices
                        .iter()
                        .filter(|p| {
                            p.timestamp_ms >= target_ms.saturating_sub(two_days_ms)
                                && p.timestamp_ms <= target_ms + two_days_ms
                        })
                        .min_by_key(|p| {
                            (p.timestamp_ms as i64 - target_ms as i64).unsigned_abs()
                        })
                        .map(|p| p.price_usd);

                    if let Some(p) = api_price {
                        return p;
                    }

                    // Fall back to early price table
                    let year_month = format!("{}-{:02}", year, month);
                    let year_str = year.to_string();
                    for (prefix, price) in early_prices.iter().rev() {
                        if year_month.starts_with(prefix) || year_str.starts_with(prefix) {
                            return *price;
                        }
                    }
                    0.0
                })
                .unwrap_or(0.0);

            // Collect events for this date AND year
            let mut events = Vec::new();
            for (event_year, date, title, context, block) in &notable_dates {
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
        })
        .collect();

    Ok(OnThisDayData { month, day, years })
}

#[server(prefix = "/api", endpoint = "range_summary")]
pub async fn fetch_range_summary(
    from_ts: u64,
    to_ts: u64,
) -> Result<RangeSummary, ServerFnError> {
    let Extension(state): Extension<std::sync::Arc<super::api::StatsState>> =
        leptos_axum::extract().await.map_err(|e| {
            ServerFnError::new(format!("Stats not available: {e}"))
        })?;
    let conn = state
        .db
        .get()
        .map_err(|e| ServerFnError::new(format!("DB pool: {e}")))?;
    super::db::query_range_summary(&conn, from_ts, to_ts)
        .map_err(|e| ServerFnError::new(format!("DB error: {e}")))
}
