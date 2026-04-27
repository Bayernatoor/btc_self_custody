//! One-shot utility: fill heights that are missing below the current DB tip.
//!
//! Forward-ingestion can leave gaps when the node is unreachable mid
//! chain-advance. The two background backfill paths don't catch these:
//! `backfill_extras` only re-fetches rows where `backfill_version` is
//! stale (the gap rows don't exist at all), and `backfill_backwards`
//! walks contiguously from `min_height` down so it never reaches a
//! gap near tip. This script queries for missing heights via a
//! recursive CTE and fills each via the same RPC + insert path the
//! running server uses.
//!
//! Usage (from the repo root):
//!
//!   BITCOIN_STATS_RPC_URL=http://127.0.0.1:8332 \
//!   BITCOIN_STATS_RPC_USER=bitcoin \
//!   BITCOIN_STATS_RPC_PASSWORD=... \
//!   BITCOIN_STATS_DB_PATH=./bitcoin_stats.db \
//!   cargo run --bin backfill_missing_heights
//!
//! Env vars match those in `StatsConfig::load()`. Safe to run while the
//! main server is up: `db::insert_blocks` uses `INSERT OR IGNORE`, and
//! SQLite WAL mode allows the concurrent reader/writer.

use we_hodl_btc::stats::{config::StatsConfig, db, rpc::BitcoinRpc};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,backfill_missing_heights=info".into()),
        )
        .init();

    let config = StatsConfig::load()
        .expect("BITCOIN_STATS_RPC_URL must be set (see StatsConfig::load)");
    let pool =
        db::open_pool(&config.db_path, 4).expect("open SQLite pool");
    let rpc = BitcoinRpc::new(
        config.rpc_url,
        config.rpc_user,
        config.rpc_password,
    );

    let missing = find_missing_heights(&pool);
    if missing.is_empty() {
        println!("No gaps. DB is contiguous from 0 to MAX(height).");
        return;
    }
    println!(
        "Found {} missing height(s). First: {}, last: {}.",
        missing.len(),
        missing.first().unwrap(),
        missing.last().unwrap()
    );

    let total = missing.len();
    let mut filled = 0usize;
    let mut failed = 0usize;
    for (idx, height) in missing.iter().enumerate() {
        match rpc.fetch_block_by_height(*height).await {
            Ok(block) => {
                let conn = pool.get().expect("DB connection");
                match db::insert_blocks(&conn, &[block]) {
                    Ok(()) => {
                        filled += 1;
                    }
                    Err(e) => {
                        eprintln!("  height {height}: insert failed: {e}");
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("  height {height}: RPC fetch failed: {e}");
                failed += 1;
            }
        }
        // Periodic progress (every 10, plus the last).
        let n = idx + 1;
        if n % 10 == 0 || n == total {
            println!("  {}/{} done ({} filled, {} failed)", n, total, filled, failed);
        }
    }

    println!("Done. {filled} filled, {failed} failed.");
    if failed > 0 {
        std::process::exit(1);
    }
}

/// Heights in `[0, MAX(height)]` that have no row in `blocks`.
/// Uses a recursive CTE so SQLite walks the range once. Cheap on a
/// few-hundred-thousand-row table; if this ever needs to scale to
/// many millions of heights, switch to a `generate_series` extension.
fn find_missing_heights(pool: &db::DbPool) -> Vec<u64> {
    let conn = pool.get().expect("DB connection");
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE r(n) AS (\
                SELECT 0 \
                UNION ALL \
                SELECT n + 1 FROM r WHERE n < (SELECT MAX(height) FROM blocks)\
             ) \
             SELECT n FROM r WHERE n NOT IN (SELECT height FROM blocks) ORDER BY n",
        )
        .expect("prepare missing-heights CTE");
    stmt.query_map([], |row| row.get::<_, i64>(0).map(|n| n as u64))
        .expect("query missing heights")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect missing heights")
}
