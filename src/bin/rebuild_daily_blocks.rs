//! One-shot utility: rebuild every `daily_blocks` row from `blocks`.
//!
//! `daily_blocks` is the pre-computed per-day rollup that every chart range
//! above 5,000 blocks renders from (3M through ALL, plus custom ranges). It is
//! maintained incrementally by `db::insert_blocks` / `db::update_block_extras`,
//! which re-aggregate the days they touch. That covers everything written from
//! now on; it does not repair what already drifted:
//!
//!   - Columns added by a later `ALTER TABLE ... DEFAULT 0` keep that default in
//!     every pre-existing daily row. The extras backfill populates them in
//!     `blocks` but, before this fix, nothing re-aggregated the affected days.
//!   - Days whose blocks arrived while the rollup was keyed on the system
//!     clock's "today" rather than the block's own timestamp.
//!   - Days that never got a row at all because the process was down when their
//!     blocks were ingested.
//!
//! `db::rebuild_all_daily_blocks` deliberately refuses to run once the table is
//! non-empty (correct for first-run startup, useless for repair), so this calls
//! `db::rebuild_daily_blocks_force`, which runs the same grouped scan
//! unconditionally. One pass over `blocks` rather than a seek per day.
//!
//! Usage (from the repo root):
//!
//!   BITCOIN_STATS_RPC_URL=http://127.0.0.1:8332 \
//!   BITCOIN_STATS_DB_PATH=./bitcoin_stats.db \
//!   cargo run --bin rebuild_daily_blocks --features ssr
//!
//! `BITCOIN_STATS_RPC_URL` is only read because `StatsConfig::load()` requires
//! it; this utility makes no RPC calls and reads nothing but the local database.
//!
//! Safe to run while the server is up: SQLite WAL mode allows a concurrent
//! reader/writer, and `INSERT OR REPLACE` is idempotent. It does hold a write
//! transaction for the duration of the scan, so the block poller's insert may
//! wait on it (the pool's 5s busy_timeout applies). Prefer a quiet moment.

use we_hodl_btc::stats::{config::StatsConfig, db};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,rebuild_daily_blocks=info".into()),
        )
        .init();

    let config = StatsConfig::load()
        .expect("BITCOIN_STATS_RPC_URL must be set (see StatsConfig::load)");
    let pool = db::open_pool(&config.db_path, 2).expect("open SQLite pool");
    let conn = pool.get().expect("get connection");

    let before_days: u64 = conn
        .query_row("SELECT COUNT(*) FROM daily_blocks", [], |r| r.get(0))
        .unwrap_or(0);
    let missing_before = db::daily_blocks_missing_days(&conn).unwrap_or(0);
    println!(
        "before: {before_days} daily rows, {missing_before} day(s) present in blocks but missing here"
    );

    let started = std::time::Instant::now();
    let after_days = match db::rebuild_daily_blocks_force(&conn) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("rebuild failed: {e}");
            std::process::exit(1);
        }
    };
    let missing_after = db::daily_blocks_missing_days(&conn).unwrap_or(0);

    println!(
        "after:  {after_days} daily rows, {missing_after} missing, took {:.1}s",
        started.elapsed().as_secs_f64()
    );

    if missing_after != 0 {
        eprintln!(
            "WARNING: {missing_after} day(s) still missing after a full rebuild. \
             That should be impossible unless blocks were written during the scan; \
             re-run and investigate if it persists."
        );
        std::process::exit(1);
    }
    println!("daily_blocks is consistent with blocks.");
}
