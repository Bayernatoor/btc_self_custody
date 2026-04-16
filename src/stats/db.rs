//! SQLite database: schema, migrations, and query functions.
//!
//! ## Schema
//!
//! The primary table is `blocks` with `height` as the primary key and one column
//! per metric extracted during ingestion. A secondary `mempool_txs` table tracks
//! unconfirmed transactions for the heartbeat SSE feature.
//!
//! ## Migrations
//!
//! Schema evolves via ALTER TABLE migrations checked on startup. Each migration
//! probes for a column's existence with a dummy `SELECT ... LIMIT 0` and adds it
//! if missing. This is safe to run repeatedly and handles any upgrade path.
//!
//! ## Backfill Versioning
//!
//! When new metrics are added (new columns), existing blocks need to be re-fetched
//! from RPC to populate them. The `backfill_version` column tracks which version
//! each block was last processed at. Blocks with `backfill_version < BACKFILL_VERSION`
//! are queued for re-processing by the extras backfill task.
//!
//! ## WAL Mode
//!
//! The database uses WAL (Write-Ahead Logging) journal mode for concurrent
//! read/write access. This allows API queries to run simultaneously with
//! background ingestion without blocking.
//!
//! Genesis block (height 0) is intentionally excluded from backfill since its
//! 50 BTC output is unspendable and has no meaningful fee data.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use super::rpc::Block;

/// Bump this when adding new columns that require re-fetching existing blocks.
/// The backfill loop processes all blocks with backfill_version < BACKFILL_VERSION.
/// v11: recompute max_tx_fee, inscription_fees, runes_fees, inscription_envelope_bytes
pub const BACKFILL_VERSION: u64 = 11;

/// Type alias for the connection pool used throughout the stats module.
pub type DbPool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

/// Create an r2d2 connection pool with WAL mode, 5s busy timeout, and
/// foreign keys enabled. Runs all schema migrations on one connection before
/// returning.
pub fn open_pool(
    path: &Path,
    pool_size: u32,
) -> Result<DbPool, Box<dyn std::error::Error>> {
    let manager =
        r2d2_sqlite::SqliteConnectionManager::file(path).with_init(|conn| {
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA busy_timeout = 5000;
                 PRAGMA foreign_keys = ON;",
            )?;
            Ok(())
        });

    let pool = r2d2::Pool::builder().max_size(pool_size).build(manager)?;

    // Run schema migrations on one connection
    {
        let conn = pool.get()?;
        init_schema(&conn)?;
    }

    Ok(pool)
}

/// Run all schema creation and migrations on a connection.
pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    // Create table if not exists (original schema)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS blocks (
            height              INTEGER PRIMARY KEY,
            hash                TEXT    NOT NULL UNIQUE,
            timestamp           INTEGER NOT NULL,
            tx_count            INTEGER NOT NULL,
            size                INTEGER NOT NULL,
            weight              INTEGER NOT NULL,
            difficulty          REAL    NOT NULL,
            op_return_count     INTEGER NOT NULL DEFAULT 0,
            op_return_bytes     INTEGER NOT NULL DEFAULT 0,
            runes_count         INTEGER NOT NULL DEFAULT 0,
            runes_bytes         INTEGER NOT NULL DEFAULT 0,
            data_carrier_count  INTEGER NOT NULL DEFAULT 0,
            data_carrier_bytes  INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_blocks_timestamp ON blocks(timestamp);",
    )?;

    // Collect existing column names with a single PRAGMA query (replaces 15+ SELECT probes)
    let cols: std::collections::HashSet<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(blocks)")?;
        let names = stmt.query_map([], |row| row.get::<_, String>(1))?;
        names.filter_map(|r| r.ok()).collect()
    };
    let has = |col: &str| cols.contains(col);

    if !has("version") {
        tracing::info!("Migrating: adding version, total_fees, miner columns");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN version INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN total_fees INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN miner TEXT NOT NULL DEFAULT '';",
        )?;
    }
    if !has("median_fee") {
        tracing::info!("Migrating: adding median_fee, median_fee_rate columns");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN median_fee INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN median_fee_rate REAL NOT NULL DEFAULT 0;",
        )?;
    }
    if !has("coinbase_locktime") {
        tracing::info!("Migrating: adding coinbase_locktime column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN coinbase_locktime INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    if !has("coinbase_sequence") {
        tracing::info!("Migrating: adding coinbase_sequence column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN coinbase_sequence INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    if !has("segwit_spend_count") {
        tracing::info!(
            "Migrating: adding segwit_spend_count, taproot_spend_count columns"
        );
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN segwit_spend_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN taproot_spend_count INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    if !has("omni_count") {
        tracing::info!("Migrating: adding omni/counterparty protocol columns");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN omni_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN omni_bytes INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN counterparty_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN counterparty_bytes INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    if !has("p2pkh_count") {
        tracing::info!("Migrating: adding output type counts, input/output counts, rbf_count, witness_bytes");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN p2pk_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN p2pkh_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN p2sh_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN p2wpkh_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN p2wsh_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN p2tr_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN unknown_script_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN input_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN output_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN rbf_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN witness_bytes INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    if !has("multisig_count") {
        tracing::info!("Migrating: adding multisig_count column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN multisig_count INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    if !has("inscription_count") {
        tracing::info!(
            "Migrating: adding inscription_count, inscription_bytes columns"
        );
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN inscription_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN inscription_bytes INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    if !has("brc20_count") {
        tracing::info!("Migrating: adding brc20_count column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN brc20_count INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    if !has("backfill_version") {
        tracing::info!("Migrating: adding backfill_version column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN backfill_version INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    if !has("taproot_keypath_count") {
        tracing::info!("Migrating: adding taproot_keypath_count, taproot_scriptpath_count columns");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN taproot_keypath_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN taproot_scriptpath_count INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    if !has("total_output_value") {
        tracing::info!("Migrating: adding total_output_value column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN total_output_value INTEGER NOT NULL DEFAULT 0;",
        )?;
    }
    if !has("total_input_value") {
        tracing::info!("Migrating: adding total_input_value, fee percentiles, stamps_count, largest_tx_size");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN total_input_value INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN fee_rate_p10 REAL NOT NULL DEFAULT 0.0;
             ALTER TABLE blocks ADD COLUMN fee_rate_p90 REAL NOT NULL DEFAULT 0.0;
             ALTER TABLE blocks ADD COLUMN stamps_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN largest_tx_size INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    // Ensure indexes exist (safe to run every startup).
    // Extremes indexes accelerate ORDER BY col DESC LIMIT 1 queries.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_blocks_backfill ON blocks(backfill_version);
         CREATE INDEX IF NOT EXISTS idx_blocks_month_day ON blocks(strftime('%m-%d', datetime(timestamp, 'unixepoch')));
         CREATE INDEX IF NOT EXISTS idx_blocks_size ON blocks(size);
         CREATE INDEX IF NOT EXISTS idx_blocks_total_fees ON blocks(total_fees);
         CREATE INDEX IF NOT EXISTS idx_blocks_tx_count ON blocks(tx_count);
         CREATE INDEX IF NOT EXISTS idx_blocks_median_fee_rate ON blocks(median_fee_rate);
         CREATE INDEX IF NOT EXISTS idx_blocks_input_count ON blocks(input_count);
         CREATE INDEX IF NOT EXISTS idx_blocks_output_count ON blocks(output_count);",
    )?;

    // Mempool transactions table for heartbeat ZMQ data
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS mempool_txs (
            txid            TEXT    PRIMARY KEY,
            fee             INTEGER NOT NULL,
            vsize           INTEGER NOT NULL,
            value           INTEGER NOT NULL,
            first_seen      INTEGER NOT NULL,
            confirmed_height INTEGER,
            confirmed_at    INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_mempool_first_seen ON mempool_txs(first_seen);
        CREATE INDEX IF NOT EXISTS idx_mempool_unconfirmed ON mempool_txs(confirmed_height)
            WHERE confirmed_height IS NULL;",
    )?;

    // Migration: add notable_type and value_usd to mempool_txs for whale watch
    {
        let has_notable: bool = conn
            .prepare("SELECT notable_type FROM mempool_txs LIMIT 0")
            .is_ok();
        if !has_notable {
            tracing::info!(
                "Migrating: adding notable_type, value_usd to mempool_txs"
            );
            conn.execute_batch(
                "ALTER TABLE mempool_txs ADD COLUMN notable_type TEXT;
                 ALTER TABLE mempool_txs ADD COLUMN value_usd REAL;
                 CREATE INDEX IF NOT EXISTS idx_mempool_notable ON mempool_txs(notable_type)
                     WHERE notable_type IS NOT NULL;",
            )?;
        }
    }

    // Migration: add input_count and output_count for SSE history replay.
    // Without these, notable txs replayed on reconnect show "0 in / 0 out"
    // in the tooltip even when the server-side detection used the real counts.
    {
        let has_io_counts: bool = conn
            .prepare("SELECT input_count FROM mempool_txs LIMIT 0")
            .is_ok();
        if !has_io_counts {
            tracing::info!(
                "Migrating: adding input_count, output_count to mempool_txs"
            );
            conn.execute_batch(
                "ALTER TABLE mempool_txs ADD COLUMN input_count INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE mempool_txs ADD COLUMN output_count INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
    }

    // Persistent notable transactions table (separate from mempool_txs).
    // Holds all notable txs regardless of confirmation status, indefinitely
    // (until manually pruned). This powers the dedicated whale-watch page
    // with historical browsing, leaderboards, and aggregations.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS notable_txs (
            txid                TEXT    PRIMARY KEY,
            notable_type        TEXT    NOT NULL,
            fee                 INTEGER NOT NULL,
            vsize               INTEGER NOT NULL,
            value               INTEGER NOT NULL,
            max_output_value    INTEGER NOT NULL DEFAULT 0,
            value_usd           REAL    NOT NULL DEFAULT 0,
            input_count         INTEGER NOT NULL DEFAULT 0,
            output_count        INTEGER NOT NULL DEFAULT 0,
            witness_bytes       INTEGER NOT NULL DEFAULT 0,
            op_return_text      TEXT,
            first_seen          INTEGER NOT NULL,
            confirmed_height    INTEGER,
            confirmed_at        INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_notable_type ON notable_txs(notable_type);
        CREATE INDEX IF NOT EXISTS idx_notable_first_seen ON notable_txs(first_seen DESC);
        CREATE INDEX IF NOT EXISTS idx_notable_value_usd ON notable_txs(value_usd DESC);
        CREATE INDEX IF NOT EXISTS idx_notable_confirmed_height ON notable_txs(confirmed_height);",
    )?;

    // Pre-computed daily aggregates table. Populated incrementally on new
    // blocks so queries read directly instead of re-aggregating 940k+ rows.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS daily_blocks (
            day                        TEXT PRIMARY KEY,
            block_count                INTEGER NOT NULL,
            avg_size                   REAL NOT NULL,
            avg_weight                 REAL NOT NULL,
            avg_tx_count               REAL NOT NULL,
            avg_difficulty             REAL NOT NULL,
            total_op_return_count      INTEGER NOT NULL DEFAULT 0,
            total_runes_count          INTEGER NOT NULL DEFAULT 0,
            total_omni_count           INTEGER NOT NULL DEFAULT 0,
            total_counterparty_count   INTEGER NOT NULL DEFAULT 0,
            total_data_carrier_count   INTEGER NOT NULL DEFAULT 0,
            total_op_return_bytes      INTEGER NOT NULL DEFAULT 0,
            total_runes_bytes          INTEGER NOT NULL DEFAULT 0,
            total_omni_bytes           INTEGER NOT NULL DEFAULT 0,
            total_counterparty_bytes   INTEGER NOT NULL DEFAULT 0,
            total_data_carrier_bytes   INTEGER NOT NULL DEFAULT 0,
            total_fees                 INTEGER NOT NULL DEFAULT 0,
            avg_segwit_spend_count     REAL NOT NULL DEFAULT 0,
            avg_taproot_spend_count    REAL NOT NULL DEFAULT 0,
            avg_p2pk_count             REAL NOT NULL DEFAULT 0,
            avg_p2pkh_count            REAL NOT NULL DEFAULT 0,
            avg_p2sh_count             REAL NOT NULL DEFAULT 0,
            avg_p2wpkh_count           REAL NOT NULL DEFAULT 0,
            avg_p2wsh_count            REAL NOT NULL DEFAULT 0,
            avg_p2tr_count             REAL NOT NULL DEFAULT 0,
            avg_multisig_count         REAL NOT NULL DEFAULT 0,
            avg_unknown_script_count   REAL NOT NULL DEFAULT 0,
            avg_input_count            REAL NOT NULL DEFAULT 0,
            avg_output_count           REAL NOT NULL DEFAULT 0,
            avg_rbf_count              REAL NOT NULL DEFAULT 0,
            avg_witness_bytes          REAL NOT NULL DEFAULT 0,
            avg_inscription_count      REAL NOT NULL DEFAULT 0,
            avg_inscription_bytes      REAL NOT NULL DEFAULT 0,
            avg_brc20_count            REAL NOT NULL DEFAULT 0,
            avg_taproot_keypath_count  REAL NOT NULL DEFAULT 0,
            avg_taproot_scriptpath_count REAL NOT NULL DEFAULT 0,
            avg_fee_rate_p10           REAL NOT NULL DEFAULT 0,
            avg_fee_rate_p90           REAL NOT NULL DEFAULT 0,
            avg_stamps_count           REAL NOT NULL DEFAULT 0,
            avg_median_fee_rate        REAL NOT NULL DEFAULT 0,
            total_output_value         INTEGER NOT NULL DEFAULT 0,
            total_input_value          INTEGER NOT NULL DEFAULT 0
        );",
    )?;

    // Reorgs log table
    init_reorgs_table(conn)?;

    // Migration: add value columns to daily_blocks if missing
    {
        let daily_cols: std::collections::HashSet<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(daily_blocks)")?;
            let names = stmt.query_map([], |row| row.get::<_, String>(1))?;
            names.filter_map(|r| r.ok()).collect()
        };
        if !daily_cols.contains("total_output_value") {
            tracing::info!("Migrating daily_blocks: adding value columns");
            conn.execute_batch(
                "ALTER TABLE daily_blocks ADD COLUMN total_output_value INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE daily_blocks ADD COLUMN total_input_value INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        if !daily_cols.contains("avg_inscription_envelope_bytes") {
            tracing::info!("Migrating daily_blocks: adding v11 columns");
            conn.execute_batch(
                "ALTER TABLE daily_blocks ADD COLUMN avg_inscription_envelope_bytes REAL NOT NULL DEFAULT 0;
                 ALTER TABLE daily_blocks ADD COLUMN total_inscription_fees INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE daily_blocks ADD COLUMN total_runes_fees INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE daily_blocks ADD COLUMN avg_legacy_tx_count REAL NOT NULL DEFAULT 0;
                 ALTER TABLE daily_blocks ADD COLUMN avg_segwit_tx_count REAL NOT NULL DEFAULT 0;
                 ALTER TABLE daily_blocks ADD COLUMN avg_taproot_tx_count REAL NOT NULL DEFAULT 0;
                 ALTER TABLE daily_blocks ADD COLUMN avg_fee_rate_p25 REAL NOT NULL DEFAULT 0;
                 ALTER TABLE daily_blocks ADD COLUMN avg_fee_rate_p75 REAL NOT NULL DEFAULT 0;",
            )?;
        }
    }

    if !has("max_tx_fee") {
        tracing::info!("Migrating: adding v10 columns (max_tx_fee, protocol fees, tx types, coinbase_text, fee percentiles)");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN max_tx_fee INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN inscription_fees INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN runes_fees INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN legacy_tx_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN segwit_tx_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN taproot_tx_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN coinbase_text TEXT NOT NULL DEFAULT '';
             ALTER TABLE blocks ADD COLUMN fee_rate_p25 REAL NOT NULL DEFAULT 0.0;
             ALTER TABLE blocks ADD COLUMN fee_rate_p75 REAL NOT NULL DEFAULT 0.0;",
        )?;
    }
    if !has("inscription_envelope_bytes") {
        tracing::info!("Migrating: adding inscription_envelope_bytes column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN inscription_envelope_bytes INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    Ok(())
}

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    init_schema(&conn)?;
    Ok(conn)
}

/// Return the highest block height in the database, or None if empty.
pub fn max_height(conn: &Connection) -> rusqlite::Result<Option<u64>> {
    conn.query_row("SELECT MAX(height) FROM blocks", [], |row| {
        row.get::<_, Option<u64>>(0)
    })
}

/// Return the lowest block height in the database, or None if empty.
pub fn min_height(conn: &Connection) -> rusqlite::Result<Option<u64>> {
    conn.query_row("SELECT MIN(height) FROM blocks", [], |row| {
        row.get::<_, Option<u64>>(0)
    })
}

/// Batch insert blocks within a single transaction. Uses INSERT OR IGNORE
/// so duplicate heights are silently skipped. Sets backfill_version to the
/// current BACKFILL_VERSION.
pub fn insert_blocks(
    conn: &Connection,
    blocks: &[Block],
) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare_cached(
            "INSERT OR IGNORE INTO blocks
             (height, hash, timestamp, tx_count, size, weight, difficulty,
              op_return_count, op_return_bytes, runes_count, runes_bytes,
              omni_count, omni_bytes, counterparty_count, counterparty_bytes,
              data_carrier_count, data_carrier_bytes, version, total_fees, miner,
              median_fee, median_fee_rate, coinbase_locktime, coinbase_sequence,
              segwit_spend_count, taproot_spend_count,
              taproot_keypath_count, taproot_scriptpath_count,
              p2pk_count, p2pkh_count, p2sh_count, p2wpkh_count, p2wsh_count,
              p2tr_count, multisig_count, unknown_script_count,
              input_count, output_count, rbf_count, witness_bytes,
              inscription_count, inscription_bytes, inscription_envelope_bytes, brc20_count,
              total_output_value, total_input_value,
              fee_rate_p10, fee_rate_p90, stamps_count, largest_tx_size,
              max_tx_fee, inscription_fees, runes_fees,
              legacy_tx_count, segwit_tx_count, taproot_tx_count,
              coinbase_text, fee_rate_p25, fee_rate_p75,
              backfill_version)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,?36,?37,?38,?39,?40,?41,?42,?43,?44,?45,?46,?47,?48,?49,?50,?51,?52,?53,?54,?55,?56,?57,?58,?59,?60)",
        )?;
        for block in blocks {
            stmt.execute(params![
                block.height,
                block.hash,
                block.time,
                block.n_tx,
                block.size,
                block.weight,
                block.difficulty,
                block.op_return_count,
                block.op_return_bytes,
                block.runes_count,
                block.runes_bytes,
                block.omni_count,
                block.omni_bytes,
                block.counterparty_count,
                block.counterparty_bytes,
                block.data_carrier_count,
                block.data_carrier_bytes,
                block.version,
                block.total_fees,
                block.miner,
                block.median_fee,
                block.median_fee_rate,
                block.coinbase_locktime,
                block.coinbase_sequence,
                block.segwit_spend_count,
                block.taproot_spend_count,
                block.taproot_keypath_count,
                block.taproot_scriptpath_count,
                block.p2pk_count,
                block.p2pkh_count,
                block.p2sh_count,
                block.p2wpkh_count,
                block.p2wsh_count,
                block.p2tr_count,
                block.multisig_count,
                block.unknown_script_count,
                block.input_count,
                block.output_count,
                block.rbf_count,
                block.witness_bytes,
                block.inscription_count,
                block.inscription_bytes,
                block.inscription_envelope_bytes,
                block.brc20_count,
                block.total_output_value,
                block.total_input_value,
                block.fee_rate_p10,
                block.fee_rate_p90,
                block.stamps_count,
                block.largest_tx_size,
                block.max_tx_fee,
                block.inscription_fees,
                block.runes_fees,
                block.legacy_tx_count,
                block.segwit_tx_count,
                block.taproot_tx_count,
                block.coinbase_text,
                block.fee_rate_p25,
                block.fee_rate_p75,
                BACKFILL_VERSION,
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// Count blocks needing backfill (backfill_version below current)
pub fn count_needs_backfill(conn: &Connection) -> rusqlite::Result<u64> {
    conn.query_row(
        "SELECT COUNT(*) FROM blocks WHERE backfill_version < ?1",
        params![BACKFILL_VERSION],
        |row| row.get(0),
    )
}

/// Get heights of blocks needing backfill, ordered by height DESC (newest first).
pub fn heights_needing_backfill(
    conn: &Connection,
    limit: u64,
) -> rusqlite::Result<Vec<u64>> {
    let mut stmt = conn.prepare(
        "SELECT height FROM blocks WHERE backfill_version < ?1 ORDER BY height DESC LIMIT ?2",
    )?;
    let rows =
        stmt.query_map(params![BACKFILL_VERSION, limit], |row| row.get(0))?;
    rows.collect()
}

/// Re-write all computed columns for existing blocks and bump their
/// backfill_version to BACKFILL_VERSION. Used by the extras backfill task.
pub fn update_block_extras(
    conn: &Connection,
    blocks: &[Block],
) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare_cached(
            "UPDATE blocks SET version = ?1, total_fees = ?2, miner = ?3, median_fee = ?4, median_fee_rate = ?5, coinbase_locktime = ?6, coinbase_sequence = ?7, segwit_spend_count = ?8, taproot_spend_count = ?9, omni_count = ?10, omni_bytes = ?11, counterparty_count = ?12, counterparty_bytes = ?13, runes_count = ?14, runes_bytes = ?15, data_carrier_count = ?16, data_carrier_bytes = ?17, p2pk_count = ?18, p2pkh_count = ?19, p2sh_count = ?20, p2wpkh_count = ?21, p2wsh_count = ?22, p2tr_count = ?23, multisig_count = ?24, unknown_script_count = ?25, input_count = ?26, output_count = ?27, rbf_count = ?28, witness_bytes = ?29, inscription_count = ?30, inscription_bytes = ?31, inscription_envelope_bytes = ?32, brc20_count = ?33, taproot_keypath_count = ?34, taproot_scriptpath_count = ?35, total_output_value = ?36, total_input_value = ?37, fee_rate_p10 = ?38, fee_rate_p90 = ?39, stamps_count = ?40, largest_tx_size = ?41, max_tx_fee = ?42, inscription_fees = ?43, runes_fees = ?44, legacy_tx_count = ?45, segwit_tx_count = ?46, taproot_tx_count = ?47, coinbase_text = ?48, fee_rate_p25 = ?49, fee_rate_p75 = ?50, backfill_version = ?51 WHERE height = ?52",
        )?;
        for block in blocks {
            stmt.execute(params![
                block.version,
                block.total_fees,
                block.miner,
                block.median_fee,
                block.median_fee_rate,
                block.coinbase_locktime,
                block.coinbase_sequence,
                block.segwit_spend_count,
                block.taproot_spend_count,
                block.omni_count,
                block.omni_bytes,
                block.counterparty_count,
                block.counterparty_bytes,
                block.runes_count,
                block.runes_bytes,
                block.data_carrier_count,
                block.data_carrier_bytes,
                block.p2pk_count,
                block.p2pkh_count,
                block.p2sh_count,
                block.p2wpkh_count,
                block.p2wsh_count,
                block.p2tr_count,
                block.multisig_count,
                block.unknown_script_count,
                block.input_count,
                block.output_count,
                block.rbf_count,
                block.witness_bytes,
                block.inscription_count,
                block.inscription_bytes,
                block.inscription_envelope_bytes,
                block.brc20_count,
                block.taproot_keypath_count,
                block.taproot_scriptpath_count,
                block.total_output_value,
                block.total_input_value,
                block.fee_rate_p10,
                block.fee_rate_p90,
                block.stamps_count,
                block.largest_tx_size,
                block.max_tx_fee,
                block.inscription_fees,
                block.runes_fees,
                block.legacy_tx_count,
                block.segwit_tx_count,
                block.taproot_tx_count,
                block.coinbase_text,
                block.fee_rate_p25,
                block.fee_rate_p75,
                BACKFILL_VERSION,
                block.height
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

// === Query types ===

/// Row returned by `query_blocks` and `query_blocks_by_ts` - per-block summary data.
#[derive(serde::Serialize)]
pub struct BlockRow {
    pub height: u64,
    pub hash: String,
    pub timestamp: u64,
    pub tx_count: u64,
    pub size: u64,
    pub weight: u64,
    pub difficulty: f64,
    pub total_fees: u64,
    pub median_fee: u64,
    pub median_fee_rate: f64,
    pub segwit_spend_count: u64,
    pub taproot_spend_count: u64,
    pub p2pk_count: u64,
    pub p2pkh_count: u64,
    pub p2sh_count: u64,
    pub p2wpkh_count: u64,
    pub p2wsh_count: u64,
    pub p2tr_count: u64,
    pub multisig_count: u64,
    pub unknown_script_count: u64,
    pub input_count: u64,
    pub output_count: u64,
    pub rbf_count: u64,
    pub witness_bytes: u64,
    pub inscription_count: u64,
    pub inscription_bytes: u64,
    pub inscription_envelope_bytes: u64,
    pub brc20_count: u64,
    pub op_return_count: u64,
    pub op_return_bytes: u64,
    pub runes_count: u64,
    pub runes_bytes: u64,
    pub omni_count: u64,
    pub omni_bytes: u64,
    pub counterparty_count: u64,
    pub counterparty_bytes: u64,
    pub data_carrier_count: u64,
    pub data_carrier_bytes: u64,
    pub taproot_keypath_count: u64,
    pub taproot_scriptpath_count: u64,
    pub total_output_value: u64,
    pub total_input_value: u64,
    pub fee_rate_p10: f64,
    pub fee_rate_p90: f64,
    pub stamps_count: u64,
    pub largest_tx_size: u64,
    // --- Backfill v10 fields ---
    pub max_tx_fee: u64,
    pub inscription_fees: u64,
    pub runes_fees: u64,
    pub legacy_tx_count: u64,
    pub segwit_tx_count: u64,
    pub taproot_tx_count: u64,
    pub coinbase_text: String,
    pub fee_rate_p25: f64,
    pub fee_rate_p75: f64,
}

impl From<BlockRow> for super::types::BlockSummary {
    fn from(r: BlockRow) -> Self {
        Self {
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
            inscription_envelope_bytes: r.inscription_envelope_bytes,
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
            max_tx_fee: r.max_tx_fee,
            inscription_fees: r.inscription_fees,
            runes_fees: r.runes_fees,
            legacy_tx_count: r.legacy_tx_count,
            segwit_tx_count: r.segwit_tx_count,
            taproot_tx_count: r.taproot_tx_count,
            coinbase_text: r.coinbase_text,
            fee_rate_p25: r.fee_rate_p25,
            fee_rate_p75: r.fee_rate_p75,
        }
    }
}

/// Query blocks by height range [from, to] inclusive, ordered by height ASC.
pub fn query_blocks(
    conn: &Connection,
    from: u64,
    to: u64,
) -> rusqlite::Result<Vec<BlockRow>> {
    let mut stmt = conn.prepare(
        "SELECT height, hash, timestamp, tx_count, size, weight, difficulty,
                total_fees, median_fee, median_fee_rate,
                segwit_spend_count, taproot_spend_count,
                p2pk_count, p2pkh_count, p2sh_count, p2wpkh_count, p2wsh_count,
                p2tr_count, multisig_count, unknown_script_count,
                input_count, output_count, rbf_count, witness_bytes,
                inscription_count, inscription_bytes, inscription_envelope_bytes, brc20_count,
                op_return_count, op_return_bytes,
                runes_count, runes_bytes, omni_count, omni_bytes,
                counterparty_count, counterparty_bytes,
                data_carrier_count, data_carrier_bytes,
                taproot_keypath_count, taproot_scriptpath_count,
                total_output_value, total_input_value,
                fee_rate_p10, fee_rate_p90, stamps_count, largest_tx_size,
                max_tx_fee, inscription_fees, runes_fees,
                legacy_tx_count, segwit_tx_count, taproot_tx_count,
                coinbase_text, fee_rate_p25, fee_rate_p75
         FROM blocks WHERE height >= ?1 AND height <= ?2 ORDER BY height ASC",
    )?;
    let rows = stmt.query_map(params![from, to], |row| {
        Ok(BlockRow {
            height: row.get(0)?,
            hash: row.get(1)?,
            timestamp: row.get(2)?,
            tx_count: row.get(3)?,
            size: row.get(4)?,
            weight: row.get(5)?,
            difficulty: row.get(6)?,
            total_fees: row.get(7)?,
            median_fee: row.get(8)?,
            median_fee_rate: row.get(9)?,
            segwit_spend_count: row.get(10)?,
            taproot_spend_count: row.get(11)?,
            p2pk_count: row.get(12)?,
            p2pkh_count: row.get(13)?,
            p2sh_count: row.get(14)?,
            p2wpkh_count: row.get(15)?,
            p2wsh_count: row.get(16)?,
            p2tr_count: row.get(17)?,
            multisig_count: row.get(18)?,
            unknown_script_count: row.get(19)?,
            input_count: row.get(20)?,
            output_count: row.get(21)?,
            rbf_count: row.get(22)?,
            witness_bytes: row.get(23)?,
            inscription_count: row.get(24)?,
            inscription_bytes: row.get(25)?,
            inscription_envelope_bytes: row.get(26)?,
            brc20_count: row.get(27)?,
            op_return_count: row.get(28)?,
            op_return_bytes: row.get(29)?,
            runes_count: row.get(30)?,
            runes_bytes: row.get(31)?,
            omni_count: row.get(32)?,
            omni_bytes: row.get(33)?,
            counterparty_count: row.get(34)?,
            counterparty_bytes: row.get(35)?,
            data_carrier_count: row.get(36)?,
            data_carrier_bytes: row.get(37)?,
            taproot_keypath_count: row.get(38)?,
            taproot_scriptpath_count: row.get(39)?,
            total_output_value: row.get(40)?,
            total_input_value: row.get(41)?,
            fee_rate_p10: row.get(42)?,
            fee_rate_p90: row.get(43)?,
            stamps_count: row.get(44)?,
            largest_tx_size: row.get(45)?,
            max_tx_fee: row.get(46)?,
            inscription_fees: row.get(47)?,
            runes_fees: row.get(48)?,
            legacy_tx_count: row.get(49)?,
            segwit_tx_count: row.get(50)?,
            taproot_tx_count: row.get(51)?,
            coinbase_text: row.get(52)?,
            fee_rate_p25: row.get(53)?,
            fee_rate_p75: row.get(54)?,
        })
    })?;
    rows.collect()
}

/// Query blocks by timestamp range (for custom date ranges).
pub fn query_blocks_by_ts(
    conn: &Connection,
    from_ts: u64,
    to_ts: u64,
) -> rusqlite::Result<Vec<BlockRow>> {
    let mut stmt = conn.prepare(
        "SELECT height, hash, timestamp, tx_count, size, weight, difficulty,
                total_fees, median_fee, median_fee_rate,
                segwit_spend_count, taproot_spend_count,
                p2pk_count, p2pkh_count, p2sh_count, p2wpkh_count, p2wsh_count,
                p2tr_count, multisig_count, unknown_script_count,
                input_count, output_count, rbf_count, witness_bytes,
                inscription_count, inscription_bytes, inscription_envelope_bytes, brc20_count,
                op_return_count, op_return_bytes,
                runes_count, runes_bytes, omni_count, omni_bytes,
                counterparty_count, counterparty_bytes,
                data_carrier_count, data_carrier_bytes,
                taproot_keypath_count, taproot_scriptpath_count,
                total_output_value, total_input_value,
                fee_rate_p10, fee_rate_p90, stamps_count, largest_tx_size,
                max_tx_fee, inscription_fees, runes_fees,
                legacy_tx_count, segwit_tx_count, taproot_tx_count,
                coinbase_text, fee_rate_p25, fee_rate_p75
         FROM blocks WHERE timestamp >= ?1 AND timestamp <= ?2 ORDER BY height ASC",
    )?;
    let rows = stmt.query_map(params![from_ts, to_ts], |row| {
        Ok(BlockRow {
            height: row.get(0)?,
            hash: row.get(1)?,
            timestamp: row.get(2)?,
            tx_count: row.get(3)?,
            size: row.get(4)?,
            weight: row.get(5)?,
            difficulty: row.get(6)?,
            total_fees: row.get(7)?,
            median_fee: row.get(8)?,
            median_fee_rate: row.get(9)?,
            segwit_spend_count: row.get(10)?,
            taproot_spend_count: row.get(11)?,
            p2pk_count: row.get(12)?,
            p2pkh_count: row.get(13)?,
            p2sh_count: row.get(14)?,
            p2wpkh_count: row.get(15)?,
            p2wsh_count: row.get(16)?,
            p2tr_count: row.get(17)?,
            multisig_count: row.get(18)?,
            unknown_script_count: row.get(19)?,
            input_count: row.get(20)?,
            output_count: row.get(21)?,
            rbf_count: row.get(22)?,
            witness_bytes: row.get(23)?,
            inscription_count: row.get(24)?,
            inscription_bytes: row.get(25)?,
            inscription_envelope_bytes: row.get(26)?,
            brc20_count: row.get(27)?,
            op_return_count: row.get(28)?,
            op_return_bytes: row.get(29)?,
            runes_count: row.get(30)?,
            runes_bytes: row.get(31)?,
            omni_count: row.get(32)?,
            omni_bytes: row.get(33)?,
            counterparty_count: row.get(34)?,
            counterparty_bytes: row.get(35)?,
            data_carrier_count: row.get(36)?,
            data_carrier_bytes: row.get(37)?,
            taproot_keypath_count: row.get(38)?,
            taproot_scriptpath_count: row.get(39)?,
            total_output_value: row.get(40)?,
            total_input_value: row.get(41)?,
            fee_rate_p10: row.get(42)?,
            fee_rate_p90: row.get(43)?,
            stamps_count: row.get(44)?,
            largest_tx_size: row.get(45)?,
            max_tx_fee: row.get(46)?,
            inscription_fees: row.get(47)?,
            runes_fees: row.get(48)?,
            legacy_tx_count: row.get(49)?,
            segwit_tx_count: row.get(50)?,
            taproot_tx_count: row.get(51)?,
            coinbase_text: row.get(52)?,
            fee_rate_p25: row.get(53)?,
            fee_rate_p75: row.get(54)?,
        })
    })?;
    rows.collect()
}

/// Full block detail row returned by `query_block_by_height`.
/// Includes coinbase metadata (version, miner, locktime, sequence).
#[derive(serde::Serialize)]
pub struct FullBlockRow {
    pub height: u64,
    pub hash: String,
    pub timestamp: u64,
    pub tx_count: u64,
    pub size: u64,
    pub weight: u64,
    pub difficulty: f64,
    pub op_return_count: u64,
    pub op_return_bytes: u64,
    pub runes_count: u64,
    pub runes_bytes: u64,
    pub data_carrier_count: u64,
    pub data_carrier_bytes: u64,
    pub inscription_count: u64,
    pub inscription_bytes: u64,
    pub inscription_envelope_bytes: u64,
    pub version: u32,
    pub total_fees: u64,
    pub median_fee: u64,
    pub median_fee_rate: f64,
    pub coinbase_locktime: u64,
    pub coinbase_sequence: u64,
    pub miner: String,
    pub segwit_spend_count: u64,
    pub taproot_spend_count: u64,
}

/// Query a single block by height. Returns None if not found.
pub fn query_block_by_height(
    conn: &Connection,
    height: u64,
) -> rusqlite::Result<Option<FullBlockRow>> {
    conn.query_row(
        "SELECT height, hash, timestamp, tx_count, size, weight, difficulty,
                op_return_count, op_return_bytes, runes_count, runes_bytes,
                data_carrier_count, data_carrier_bytes, inscription_count, inscription_bytes,
                inscription_envelope_bytes,
                version, total_fees, miner,
                median_fee, median_fee_rate, coinbase_locktime, coinbase_sequence,
                segwit_spend_count, taproot_spend_count
         FROM blocks WHERE height = ?1",
        params![height],
        |row| {
            Ok(Some(FullBlockRow {
                height: row.get(0)?,
                hash: row.get(1)?,
                timestamp: row.get(2)?,
                tx_count: row.get(3)?,
                size: row.get(4)?,
                weight: row.get(5)?,
                difficulty: row.get(6)?,
                op_return_count: row.get(7)?,
                op_return_bytes: row.get(8)?,
                runes_count: row.get(9)?,
                runes_bytes: row.get(10)?,
                data_carrier_count: row.get(11)?,
                data_carrier_bytes: row.get(12)?,
                inscription_count: row.get(13)?,
                inscription_bytes: row.get(14)?,
                inscription_envelope_bytes: row.get(15)?,
                version: row.get::<_, u32>(16)?,
                total_fees: row.get(17)?,
                miner: row.get(18)?,
                median_fee: row.get(19)?,
                median_fee_rate: row.get(20)?,
                coinbase_locktime: row.get(21)?,
                coinbase_sequence: row.get(22)?,
                segwit_spend_count: row.get(23)?,
                taproot_spend_count: row.get(24)?,
            }))
        },
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

/// Total block data size (in bytes) for all blocks below a given height.
pub fn query_cumulative_size(
    conn: &Connection,
    below_height: u64,
) -> rusqlite::Result<u64> {
    conn.query_row(
        "SELECT COALESCE(SUM(size), 0) FROM blocks WHERE height < ?1",
        params![below_height],
        |row| row.get(0),
    )
}

/// Cumulative block size before a given timestamp (for custom date ranges).
pub fn query_cumulative_size_before_ts(
    conn: &Connection,
    before_ts: u64,
) -> rusqlite::Result<u64> {
    conn.query_row(
        "SELECT COALESCE(SUM(size), 0) FROM blocks WHERE timestamp < ?1",
        params![before_ts],
        |row| row.get(0),
    )
}

/// "On This Day" — aggregate block data grouped by year for a given month+day.
pub fn query_on_this_day(
    conn: &Connection,
    month_day: &str, // "04-01" format
) -> rusqlite::Result<
    Vec<(u32, u64, u64, u64, f64, f64, u64, u64, u64, u64, u64, u64)>,
> {
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%Y', datetime(timestamp, 'unixepoch')) AS INTEGER) as year,
                COUNT(*) as block_count,
                SUM(tx_count), SUM(total_fees),
                AVG(size), AVG(weight),
                SUM(inscription_count), SUM(runes_count),
                SUM(segwit_spend_count), SUM(taproot_spend_count),
                MIN(height), MAX(height)
         FROM blocks
         WHERE strftime('%m-%d', datetime(timestamp, 'unixepoch')) = ?1
         GROUP BY year
         ORDER BY year DESC",
    )?;
    let rows = stmt.query_map(params![month_day], |row| {
        Ok((
            row.get::<_, u32>(0)?,  // year
            row.get::<_, u64>(1)?,  // block_count
            row.get::<_, u64>(2)?,  // total_tx
            row.get::<_, u64>(3)?,  // total_fees
            row.get::<_, f64>(4)?,  // avg_size
            row.get::<_, f64>(5)?,  // avg_weight
            row.get::<_, u64>(6)?,  // inscriptions
            row.get::<_, u64>(7)?,  // runes
            row.get::<_, u64>(8)?,  // segwit_txs
            row.get::<_, u64>(9)?,  // taproot_outputs
            row.get::<_, u64>(10)?, // first_block
            row.get::<_, u64>(11)?, // last_block
        ))
    })?;
    rows.collect()
}

/// Per-block OP_RETURN data returned by `query_op_returns`.
#[derive(serde::Serialize)]
pub struct OpReturnRow {
    pub height: u64,
    pub timestamp: u64,
    pub tx_count: u64,
    pub size: u64,
    pub op_return_count: u64,
    pub op_return_bytes: u64,
    pub runes_count: u64,
    pub runes_bytes: u64,
    pub omni_count: u64,
    pub omni_bytes: u64,
    pub counterparty_count: u64,
    pub counterparty_bytes: u64,
    pub data_carrier_count: u64,
    pub data_carrier_bytes: u64,
}

/// Query OP_RETURN protocol breakdown by height range [from, to] inclusive.
pub fn query_op_returns(
    conn: &Connection,
    from: u64,
    to: u64,
) -> rusqlite::Result<Vec<OpReturnRow>> {
    let mut stmt = conn.prepare(
        "SELECT height, timestamp, tx_count, size, op_return_count, op_return_bytes,
                runes_count, runes_bytes, omni_count, omni_bytes,
                counterparty_count, counterparty_bytes,
                data_carrier_count, data_carrier_bytes
         FROM blocks WHERE height >= ?1 AND height <= ?2
         ORDER BY height ASC",
    )?;
    let rows = stmt.query_map(params![from, to], |row| {
        Ok(OpReturnRow {
            height: row.get(0)?,
            timestamp: row.get(1)?,
            tx_count: row.get(2)?,
            size: row.get(3)?,
            op_return_count: row.get(4)?,
            op_return_bytes: row.get(5)?,
            runes_count: row.get(6)?,
            runes_bytes: row.get(7)?,
            omni_count: row.get(8)?,
            omni_bytes: row.get(9)?,
            counterparty_count: row.get(10)?,
            counterparty_bytes: row.get(11)?,
            data_carrier_count: row.get(12)?,
            data_carrier_bytes: row.get(13)?,
        })
    })?;
    rows.collect()
}

/// Daily aggregated row returned by `query_daily_aggregates`.
/// avg_ fields are per-block averages; total_ fields are day-wide sums.
#[derive(serde::Serialize)]
pub struct DailyRow {
    pub date: String,
    pub block_count: u64,
    pub avg_size: f64,
    pub avg_weight: f64,
    pub avg_tx_count: f64,
    pub avg_difficulty: f64,
    pub total_op_return_count: u64,
    pub total_runes_count: u64,
    pub total_omni_count: u64,
    pub total_counterparty_count: u64,
    pub total_data_carrier_count: u64,
    pub total_op_return_bytes: u64,
    pub total_runes_bytes: u64,
    pub total_omni_bytes: u64,
    pub total_counterparty_bytes: u64,
    pub total_data_carrier_bytes: u64,
    pub total_fees: u64,
    pub avg_segwit_spend_count: f64,
    pub avg_taproot_spend_count: f64,
    pub avg_p2pk_count: f64,
    pub avg_p2pkh_count: f64,
    pub avg_p2sh_count: f64,
    pub avg_p2wpkh_count: f64,
    pub avg_p2wsh_count: f64,
    pub avg_p2tr_count: f64,
    pub avg_multisig_count: f64,
    pub avg_unknown_script_count: f64,
    pub avg_input_count: f64,
    pub avg_output_count: f64,
    pub avg_rbf_count: f64,
    pub avg_witness_bytes: f64,
    pub avg_inscription_count: f64,
    pub avg_inscription_bytes: f64,
    pub avg_brc20_count: f64,
    pub avg_taproot_keypath_count: f64,
    pub avg_taproot_scriptpath_count: f64,
    pub avg_fee_rate_p10: f64,
    pub avg_fee_rate_p90: f64,
    pub avg_stamps_count: f64,
    pub avg_median_fee_rate: f64,
    pub total_output_value: u64,
    pub total_input_value: u64,
    // --- v11 fields ---
    pub avg_inscription_envelope_bytes: f64,
    pub total_inscription_fees: u64,
    pub total_runes_fees: u64,
    pub avg_legacy_tx_count: f64,
    pub avg_segwit_tx_count: f64,
    pub avg_taproot_tx_count: f64,
    pub avg_fee_rate_p25: f64,
    pub avg_fee_rate_p75: f64,
}

/// Rebuild a single day's row in the daily_blocks table by re-aggregating
/// from the raw blocks table. Called after new block ingestion.
pub fn refresh_daily_block(
    conn: &Connection,
    day: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO daily_blocks
            (day, block_count, avg_size, avg_weight, avg_tx_count, avg_difficulty,
             total_op_return_count, total_runes_count, total_omni_count,
             total_counterparty_count, total_data_carrier_count,
             total_op_return_bytes, total_runes_bytes, total_omni_bytes,
             total_counterparty_bytes, total_data_carrier_bytes,
             total_fees, avg_segwit_spend_count, avg_taproot_spend_count,
             avg_p2pk_count, avg_p2pkh_count, avg_p2sh_count,
             avg_p2wpkh_count, avg_p2wsh_count, avg_p2tr_count,
             avg_multisig_count, avg_unknown_script_count,
             avg_input_count, avg_output_count, avg_rbf_count, avg_witness_bytes,
             avg_inscription_count, avg_inscription_bytes, avg_brc20_count,
             avg_taproot_keypath_count, avg_taproot_scriptpath_count,
             avg_fee_rate_p10, avg_fee_rate_p90, avg_stamps_count, avg_median_fee_rate,
             total_output_value, total_input_value,
             avg_inscription_envelope_bytes, total_inscription_fees, total_runes_fees,
             avg_legacy_tx_count, avg_segwit_tx_count, avg_taproot_tx_count,
             avg_fee_rate_p25, avg_fee_rate_p75)
         SELECT date(datetime(timestamp, 'unixepoch')),
                COUNT(*), AVG(size), AVG(weight), AVG(tx_count), AVG(difficulty),
                SUM(op_return_count), SUM(runes_count), SUM(omni_count),
                SUM(counterparty_count), SUM(data_carrier_count),
                SUM(op_return_bytes), SUM(runes_bytes), SUM(omni_bytes),
                SUM(counterparty_bytes), SUM(data_carrier_bytes),
                SUM(total_fees),
                AVG(segwit_spend_count), AVG(taproot_spend_count),
                AVG(p2pk_count), AVG(p2pkh_count), AVG(p2sh_count),
                AVG(p2wpkh_count), AVG(p2wsh_count), AVG(p2tr_count),
                AVG(multisig_count), AVG(unknown_script_count),
                AVG(input_count), AVG(output_count), AVG(rbf_count), AVG(witness_bytes),
                AVG(inscription_count), AVG(inscription_bytes), AVG(brc20_count),
                AVG(taproot_keypath_count), AVG(taproot_scriptpath_count),
                AVG(fee_rate_p10), AVG(fee_rate_p90), AVG(stamps_count), AVG(median_fee_rate),
                SUM(total_output_value), SUM(total_input_value),
                AVG(inscription_envelope_bytes), SUM(inscription_fees), SUM(runes_fees),
                AVG(legacy_tx_count), AVG(segwit_tx_count), AVG(taproot_tx_count),
                AVG(fee_rate_p25), AVG(fee_rate_p75)
         FROM blocks
         WHERE date(datetime(timestamp, 'unixepoch')) = ?1",
        params![day],
    )?;
    Ok(())
}

/// Populate the entire daily_blocks table from scratch. Used on first run
/// when the table is empty but blocks already exist.
pub fn rebuild_all_daily_blocks(conn: &Connection) -> rusqlite::Result<u64> {
    let count: u64 =
        conn.query_row("SELECT COUNT(*) FROM daily_blocks", [], |r| r.get(0))?;
    let block_count: u64 =
        conn.query_row("SELECT COUNT(*) FROM blocks", [], |r| r.get(0))?;

    // Only rebuild if blocks exist but daily_blocks is empty
    if count > 0 || block_count == 0 {
        return Ok(count);
    }

    tracing::info!(
        "Building daily_blocks table from {} blocks...",
        block_count
    );
    conn.execute_batch(
        "INSERT OR REPLACE INTO daily_blocks
            (day, block_count, avg_size, avg_weight, avg_tx_count, avg_difficulty,
             total_op_return_count, total_runes_count, total_omni_count,
             total_counterparty_count, total_data_carrier_count,
             total_op_return_bytes, total_runes_bytes, total_omni_bytes,
             total_counterparty_bytes, total_data_carrier_bytes,
             total_fees, avg_segwit_spend_count, avg_taproot_spend_count,
             avg_p2pk_count, avg_p2pkh_count, avg_p2sh_count,
             avg_p2wpkh_count, avg_p2wsh_count, avg_p2tr_count,
             avg_multisig_count, avg_unknown_script_count,
             avg_input_count, avg_output_count, avg_rbf_count, avg_witness_bytes,
             avg_inscription_count, avg_inscription_bytes, avg_brc20_count,
             avg_taproot_keypath_count, avg_taproot_scriptpath_count,
             avg_fee_rate_p10, avg_fee_rate_p90, avg_stamps_count, avg_median_fee_rate,
             total_output_value, total_input_value,
             avg_inscription_envelope_bytes, total_inscription_fees, total_runes_fees,
             avg_legacy_tx_count, avg_segwit_tx_count, avg_taproot_tx_count,
             avg_fee_rate_p25, avg_fee_rate_p75)
         SELECT date(datetime(timestamp, 'unixepoch')),
                COUNT(*), AVG(size), AVG(weight), AVG(tx_count), AVG(difficulty),
                SUM(op_return_count), SUM(runes_count), SUM(omni_count),
                SUM(counterparty_count), SUM(data_carrier_count),
                SUM(op_return_bytes), SUM(runes_bytes), SUM(omni_bytes),
                SUM(counterparty_bytes), SUM(data_carrier_bytes),
                SUM(total_fees),
                AVG(segwit_spend_count), AVG(taproot_spend_count),
                AVG(p2pk_count), AVG(p2pkh_count), AVG(p2sh_count),
                AVG(p2wpkh_count), AVG(p2wsh_count), AVG(p2tr_count),
                AVG(multisig_count), AVG(unknown_script_count),
                AVG(input_count), AVG(output_count), AVG(rbf_count), AVG(witness_bytes),
                AVG(inscription_count), AVG(inscription_bytes), AVG(brc20_count),
                AVG(taproot_keypath_count), AVG(taproot_scriptpath_count),
                AVG(fee_rate_p10), AVG(fee_rate_p90), AVG(stamps_count), AVG(median_fee_rate),
                SUM(total_output_value), SUM(total_input_value),
                AVG(inscription_envelope_bytes), SUM(inscription_fees), SUM(runes_fees),
                AVG(legacy_tx_count), AVG(segwit_tx_count), AVG(taproot_tx_count),
                AVG(fee_rate_p25), AVG(fee_rate_p75)
         FROM blocks
         GROUP BY date(datetime(timestamp, 'unixepoch'))"
    )?;
    let new_count: u64 =
        conn.query_row("SELECT COUNT(*) FROM daily_blocks", [], |r| r.get(0))?;
    tracing::info!("Built {} daily_blocks rows", new_count);
    Ok(new_count)
}

/// Query pre-computed daily aggregates from the daily_blocks table.
/// Falls back to raw aggregation if the table is empty.
pub fn query_daily_aggregates_fast(
    conn: &Connection,
    from_ts: u64,
    to_ts: u64,
) -> rusqlite::Result<Vec<DailyRow>> {
    // Convert timestamps to date strings for the pre-computed table
    let from_day = timestamp_to_date(from_ts);
    let to_day = timestamp_to_date(to_ts);

    let mut stmt = conn.prepare(
        "SELECT day, block_count, avg_size, avg_weight, avg_tx_count, avg_difficulty,
                total_op_return_count, total_runes_count, total_omni_count,
                total_counterparty_count, total_data_carrier_count,
                total_op_return_bytes, total_runes_bytes, total_omni_bytes,
                total_counterparty_bytes, total_data_carrier_bytes,
                total_fees, avg_segwit_spend_count, avg_taproot_spend_count,
                avg_p2pk_count, avg_p2pkh_count, avg_p2sh_count,
                avg_p2wpkh_count, avg_p2wsh_count, avg_p2tr_count,
                avg_multisig_count, avg_unknown_script_count,
                avg_input_count, avg_output_count, avg_rbf_count, avg_witness_bytes,
                avg_inscription_count, avg_inscription_bytes, avg_brc20_count,
                avg_taproot_keypath_count, avg_taproot_scriptpath_count,
                avg_fee_rate_p10, avg_fee_rate_p90, avg_stamps_count, avg_median_fee_rate,
                total_output_value, total_input_value,
                avg_inscription_envelope_bytes, total_inscription_fees, total_runes_fees,
                avg_legacy_tx_count, avg_segwit_tx_count, avg_taproot_tx_count,
                avg_fee_rate_p25, avg_fee_rate_p75
         FROM daily_blocks
         WHERE day >= ?1 AND day <= ?2
         ORDER BY day ASC",
    )?;
    let rows = stmt.query_map(params![from_day, to_day], |row| {
        Ok(DailyRow {
            date: row.get(0)?,
            block_count: row.get(1)?,
            avg_size: row.get(2)?,
            avg_weight: row.get(3)?,
            avg_tx_count: row.get(4)?,
            avg_difficulty: row.get(5)?,
            total_op_return_count: row.get(6)?,
            total_runes_count: row.get(7)?,
            total_omni_count: row.get(8)?,
            total_counterparty_count: row.get(9)?,
            total_data_carrier_count: row.get(10)?,
            total_op_return_bytes: row.get(11)?,
            total_runes_bytes: row.get(12)?,
            total_omni_bytes: row.get(13)?,
            total_counterparty_bytes: row.get(14)?,
            total_data_carrier_bytes: row.get(15)?,
            total_fees: row.get(16)?,
            avg_segwit_spend_count: row.get(17)?,
            avg_taproot_spend_count: row.get(18)?,
            avg_p2pk_count: row.get(19)?,
            avg_p2pkh_count: row.get(20)?,
            avg_p2sh_count: row.get(21)?,
            avg_p2wpkh_count: row.get(22)?,
            avg_p2wsh_count: row.get(23)?,
            avg_p2tr_count: row.get(24)?,
            avg_multisig_count: row.get(25)?,
            avg_unknown_script_count: row.get(26)?,
            avg_input_count: row.get(27)?,
            avg_output_count: row.get(28)?,
            avg_rbf_count: row.get(29)?,
            avg_witness_bytes: row.get(30)?,
            avg_inscription_count: row.get(31)?,
            avg_inscription_bytes: row.get(32)?,
            avg_brc20_count: row.get(33)?,
            avg_taproot_keypath_count: row.get(34)?,
            avg_taproot_scriptpath_count: row.get(35)?,
            avg_fee_rate_p10: row.get(36)?,
            avg_fee_rate_p90: row.get(37)?,
            avg_stamps_count: row.get(38)?,
            avg_median_fee_rate: row.get(39)?,
            total_output_value: row.get::<_, Option<u64>>(40)?.unwrap_or(0),
            total_input_value: row.get::<_, Option<u64>>(41)?.unwrap_or(0),
            avg_inscription_envelope_bytes: row
                .get::<_, Option<f64>>(42)?
                .unwrap_or(0.0),
            total_inscription_fees: row.get::<_, Option<u64>>(43)?.unwrap_or(0),
            total_runes_fees: row.get::<_, Option<u64>>(44)?.unwrap_or(0),
            avg_legacy_tx_count: row.get::<_, Option<f64>>(45)?.unwrap_or(0.0),
            avg_segwit_tx_count: row.get::<_, Option<f64>>(46)?.unwrap_or(0.0),
            avg_taproot_tx_count: row.get::<_, Option<f64>>(47)?.unwrap_or(0.0),
            avg_fee_rate_p25: row.get::<_, Option<f64>>(48)?.unwrap_or(0.0),
            avg_fee_rate_p75: row.get::<_, Option<f64>>(49)?.unwrap_or(0.0),
        })
    })?;

    let result: Vec<DailyRow> = rows.filter_map(|r| r.ok()).collect();

    // Fallback to raw aggregation if pre-computed table has no data
    if result.is_empty() {
        return query_daily_aggregates(conn, from_ts, to_ts);
    }

    Ok(result)
}

/// Convert a unix timestamp to a "YYYY-MM-DD" date string (UTC).
pub fn timestamp_to_date(ts: u64) -> String {
    let secs = ts as i64;
    let days = secs / 86400;
    // Compute date from days since epoch using civil calendar algorithm
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Aggregate block data by UTC date within a timestamp range (raw scan).
/// Groups by `date(datetime(timestamp, 'unixepoch'))` and computes AVG/SUM.
/// Prefer `query_daily_aggregates_fast` which reads from the pre-computed table.
pub fn query_daily_aggregates(
    conn: &Connection,
    from_ts: u64,
    to_ts: u64,
) -> rusqlite::Result<Vec<DailyRow>> {
    let mut stmt = conn.prepare(
        "SELECT date(datetime(timestamp, 'unixepoch')) as day,
                COUNT(*) as block_count,
                AVG(size), AVG(weight), AVG(tx_count), AVG(difficulty),
                SUM(op_return_count), SUM(runes_count), SUM(omni_count),
                SUM(counterparty_count), SUM(data_carrier_count),
                SUM(op_return_bytes), SUM(runes_bytes), SUM(omni_bytes),
                SUM(counterparty_bytes), SUM(data_carrier_bytes),
                SUM(total_fees),
                AVG(segwit_spend_count), AVG(taproot_spend_count),
                AVG(p2pk_count), AVG(p2pkh_count), AVG(p2sh_count),
                AVG(p2wpkh_count), AVG(p2wsh_count), AVG(p2tr_count),
                AVG(multisig_count), AVG(unknown_script_count),
                AVG(input_count), AVG(output_count), AVG(rbf_count),
                AVG(witness_bytes),
                AVG(inscription_count), AVG(inscription_bytes),
                AVG(brc20_count),
                AVG(taproot_keypath_count), AVG(taproot_scriptpath_count),
                AVG(fee_rate_p10), AVG(fee_rate_p90), AVG(stamps_count),
                AVG(median_fee_rate),
                SUM(total_output_value), SUM(total_input_value),
                AVG(inscription_envelope_bytes), SUM(inscription_fees), SUM(runes_fees),
                AVG(legacy_tx_count), AVG(segwit_tx_count), AVG(taproot_tx_count),
                AVG(fee_rate_p25), AVG(fee_rate_p75)
         FROM blocks
         WHERE timestamp >= ?1 AND timestamp <= ?2
         GROUP BY day
         ORDER BY day ASC",
    )?;
    let rows = stmt.query_map(params![from_ts, to_ts], |row| {
        Ok(DailyRow {
            date: row.get(0)?,
            block_count: row.get(1)?,
            avg_size: row.get(2)?,
            avg_weight: row.get(3)?,
            avg_tx_count: row.get(4)?,
            avg_difficulty: row.get(5)?,
            total_op_return_count: row.get(6)?,
            total_runes_count: row.get(7)?,
            total_omni_count: row.get(8)?,
            total_counterparty_count: row.get(9)?,
            total_data_carrier_count: row.get(10)?,
            total_op_return_bytes: row.get(11)?,
            total_runes_bytes: row.get(12)?,
            total_omni_bytes: row.get(13)?,
            total_counterparty_bytes: row.get(14)?,
            total_data_carrier_bytes: row.get(15)?,
            total_fees: row.get(16)?,
            avg_segwit_spend_count: row.get(17)?,
            avg_taproot_spend_count: row.get(18)?,
            avg_p2pk_count: row.get(19)?,
            avg_p2pkh_count: row.get(20)?,
            avg_p2sh_count: row.get(21)?,
            avg_p2wpkh_count: row.get(22)?,
            avg_p2wsh_count: row.get(23)?,
            avg_p2tr_count: row.get(24)?,
            avg_multisig_count: row.get(25)?,
            avg_unknown_script_count: row.get(26)?,
            avg_input_count: row.get(27)?,
            avg_output_count: row.get(28)?,
            avg_rbf_count: row.get(29)?,
            avg_witness_bytes: row.get(30)?,
            avg_inscription_count: row.get(31)?,
            avg_inscription_bytes: row.get(32)?,
            avg_brc20_count: row.get(33)?,
            avg_taproot_keypath_count: row.get(34)?,
            avg_taproot_scriptpath_count: row.get(35)?,
            avg_fee_rate_p10: row.get(36)?,
            avg_fee_rate_p90: row.get(37)?,
            avg_stamps_count: row.get(38)?,
            avg_median_fee_rate: row.get(39)?,
            total_output_value: row.get::<_, Option<u64>>(40)?.unwrap_or(0),
            total_input_value: row.get::<_, Option<u64>>(41)?.unwrap_or(0),
            avg_inscription_envelope_bytes: row
                .get::<_, Option<f64>>(42)?
                .unwrap_or(0.0),
            total_inscription_fees: row.get::<_, Option<u64>>(43)?.unwrap_or(0),
            total_runes_fees: row.get::<_, Option<u64>>(44)?.unwrap_or(0),
            avg_legacy_tx_count: row.get::<_, Option<f64>>(45)?.unwrap_or(0.0),
            avg_segwit_tx_count: row.get::<_, Option<f64>>(46)?.unwrap_or(0.0),
            avg_taproot_tx_count: row.get::<_, Option<f64>>(47)?.unwrap_or(0.0),
            avg_fee_rate_p25: row.get::<_, Option<f64>>(48)?.unwrap_or(0.0),
            avg_fee_rate_p75: row.get::<_, Option<f64>>(49)?.unwrap_or(0.0),
        })
    })?;
    rows.collect()
}

/// Single-row aggregate summary for an arbitrary timestamp range.
pub fn query_range_summary(
    conn: &Connection,
    from_ts: u64,
    to_ts: u64,
) -> rusqlite::Result<super::types::RangeSummary> {
    conn.query_row(
        "SELECT COUNT(*),
                SUM(tx_count), SUM(size), SUM(weight),
                SUM(total_fees), AVG(median_fee_rate),
                MIN(timestamp), MAX(timestamp),
                SUM(segwit_spend_count), SUM(taproot_spend_count),
                SUM(taproot_keypath_count), SUM(taproot_scriptpath_count),
                SUM(p2pkh_count), SUM(p2sh_count),
                SUM(p2wpkh_count), SUM(p2wsh_count), SUM(p2tr_count),
                SUM(input_count), SUM(output_count), SUM(rbf_count),
                SUM(witness_bytes),
                SUM(inscription_count), SUM(inscription_bytes), SUM(brc20_count),
                SUM(op_return_count), SUM(op_return_bytes),
                SUM(runes_count), SUM(runes_bytes),
                SUM(omni_count), SUM(counterparty_count), SUM(data_carrier_count),
                SUM(total_output_value),
                MAX(size), MAX(total_fees),
                SUM(CASE WHEN tx_count <= 1 THEN 1 ELSE 0 END),
                AVG(median_fee),
                MAX(median_fee_rate),
                SUM(inscription_envelope_bytes)
         FROM blocks
         WHERE timestamp >= ?1 AND timestamp <= ?2",
        params![from_ts, to_ts],
        |row| {
            let block_count: u64 = row.get(0)?;
            let min_ts: u64 = row.get::<_, Option<u64>>(6)?.unwrap_or(0);
            let max_ts: u64 = row.get::<_, Option<u64>>(7)?.unwrap_or(0);
            let avg_block_time = if block_count > 1 {
                (max_ts - min_ts) as f64 / (block_count - 1) as f64 / 60.0
            } else {
                0.0
            };
            let total_tx: u64 = row.get(1)?;
            let total_fees: u64 = row.get(4)?;
            let user_tx = total_tx.saturating_sub(block_count);
            Ok(super::types::RangeSummary {
                block_count,
                total_tx,
                total_size: row.get(2)?,
                total_weight: row.get(3)?,
                total_fees,
                avg_fee_rate: row.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
                avg_fee_per_tx: if user_tx > 0 {
                    total_fees as f64 / user_tx as f64
                } else {
                    0.0
                },
                avg_median_fee: row.get::<_, Option<f64>>(35)?.unwrap_or(0.0),
                avg_block_time,
                min_timestamp: min_ts,
                max_timestamp: max_ts,
                total_segwit_txs: row.get(8)?,
                total_taproot_outputs: row.get(9)?,
                total_taproot_keypath: row.get(10)?,
                total_taproot_scriptpath: row.get(11)?,
                total_p2pkh: row.get(12)?,
                total_p2sh: row.get(13)?,
                total_p2wpkh: row.get(14)?,
                total_p2wsh: row.get(15)?,
                total_p2tr: row.get(16)?,
                total_inputs: row.get(17)?,
                total_outputs: row.get(18)?,
                total_rbf: row.get(19)?,
                total_witness_bytes: row.get(20)?,
                total_inscriptions: row.get(21)?,
                total_inscription_bytes: row.get(22)?,
                total_inscription_envelope_bytes: row.get::<_, Option<u64>>(37)?.unwrap_or(0),
                total_brc20: row.get(23)?,
                total_op_return_count: row.get(24)?,
                total_op_return_bytes: row.get(25)?,
                total_runes: row.get(26)?,
                total_runes_bytes: row.get(27)?,
                total_omni: row.get(28)?,
                total_counterparty: row.get(29)?,
                total_data_carrier: row.get(30)?,
                total_output_value: row.get::<_, Option<u64>>(31)?.unwrap_or(0),
                max_block_size: row.get::<_, Option<u64>>(32)?.unwrap_or(0),
                max_block_fees: row.get::<_, Option<u64>>(33)?.unwrap_or(0),
                empty_block_count: row.get::<_, Option<u64>>(34)?.unwrap_or(0),
                max_fee_rate: row.get::<_, Option<f64>>(36)?.unwrap_or(0.0),
                witness_pct: if row.get::<_, Option<u64>>(2)?.unwrap_or(0) > 0 {
                    row.get::<_, Option<u64>>(20)?.unwrap_or(0) as f64
                        / row.get::<_, Option<u64>>(2)?.unwrap_or(1) as f64
                        * 100.0
                } else {
                    0.0
                },
            })
        },
    )
}

/// Per-block signaling status returned by signaling queries.
#[derive(serde::Serialize)]
pub struct SignalingBlock {
    pub height: u64,
    pub timestamp: u64,
    pub signaled: bool,
    pub miner: String,
}

/// Query per-block BIP9 version bit signaling status for a height range.
/// Checks whether `version & (1 << bit)` is set in each block.
pub fn query_signaling_bit(
    conn: &Connection,
    bit: u32,
    from: u64,
    to: u64,
) -> rusqlite::Result<Vec<SignalingBlock>> {
    let mask = 1i64 << bit;
    let mut stmt = conn.prepare(
        "SELECT height, timestamp, (version & ?1) != 0 as signaled, miner
         FROM blocks WHERE height >= ?2 AND height <= ?3
         ORDER BY height ASC",
    )?;
    let rows = stmt.query_map(params![mask, from, to], |row| {
        Ok(SignalingBlock {
            height: row.get(0)?,
            timestamp: row.get(1)?,
            signaled: row.get(2)?,
            miner: row.get(3)?,
        })
    })?;
    rows.collect()
}

/// BIP-54 signaling: coinbase locktime == height - 1 AND sequence != 0xffffffff
pub fn query_signaling_locktime(
    conn: &Connection,
    from: u64,
    to: u64,
) -> rusqlite::Result<Vec<SignalingBlock>> {
    let mut stmt = conn.prepare(
        "SELECT height, timestamp, (coinbase_locktime = height - 1 AND coinbase_sequence != 4294967295) as signaled, miner
         FROM blocks WHERE height >= ?1 AND height <= ?2
         ORDER BY height ASC",
    )?;
    let rows = stmt.query_map(params![from, to], |row| {
        Ok(SignalingBlock {
            height: row.get(0)?,
            timestamp: row.get(1)?,
            signaled: row.get(2)?,
            miner: row.get(3)?,
        })
    })?;
    rows.collect()
}

/// Per-retarget-period signaling summary.
#[derive(Clone, serde::Serialize)]
pub struct SignalingPeriod {
    pub period: u64,
    pub start_height: u64,
    pub end_height: u64,
    pub signaled_count: u64,
    pub total_blocks: u64,
    pub signaled_pct: f64,
}

/// Aggregate BIP9 version bit signaling by 2016-block retarget period.
pub fn query_signaling_periods_bit(
    conn: &Connection,
    bit: u32,
) -> rusqlite::Result<Vec<SignalingPeriod>> {
    let mask = 1i64 << bit;
    let mut stmt = conn.prepare(
        "SELECT height / 2016 as period,
                MIN(height), MAX(height),
                SUM(CASE WHEN (version & ?1) != 0 THEN 1 ELSE 0 END),
                COUNT(*)
         FROM blocks
         GROUP BY period
         ORDER BY period ASC",
    )?;
    let rows = stmt.query_map(params![mask], |row| {
        let signaled: u64 = row.get(3)?;
        let total: u64 = row.get(4)?;
        Ok(SignalingPeriod {
            period: row.get(0)?,
            start_height: row.get(1)?,
            end_height: row.get(2)?,
            signaled_count: signaled,
            total_blocks: total,
            signaled_pct: if total > 0 {
                signaled as f64 / total as f64 * 100.0
            } else {
                0.0
            },
        })
    })?;
    rows.collect()
}

/// Aggregate BIP-54 locktime signaling by 2016-block retarget period.
/// A block signals if coinbase_locktime == height - 1 AND coinbase_sequence != 0xFFFFFFFF.
pub fn query_signaling_periods_locktime(
    conn: &Connection,
) -> rusqlite::Result<Vec<SignalingPeriod>> {
    let mut stmt = conn.prepare(
        "SELECT height / 2016 as period,
                MIN(height), MAX(height),
                SUM(CASE WHEN coinbase_locktime = height - 1 AND coinbase_sequence != 4294967295 THEN 1 ELSE 0 END),
                COUNT(*)
         FROM blocks
         GROUP BY period
         ORDER BY period ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let signaled: u64 = row.get(3)?;
        let total: u64 = row.get(4)?;
        Ok(SignalingPeriod {
            period: row.get(0)?,
            start_height: row.get(1)?,
            end_height: row.get(2)?,
            signaled_count: signaled,
            total_blocks: total,
            signaled_pct: if total > 0 {
                signaled as f64 / total as f64 * 100.0
            } else {
                0.0
            },
        })
    })?;
    rows.collect()
}

/// Database summary stats returned by `query_stats`.
#[derive(serde::Serialize)]
pub struct Stats {
    pub block_count: u64,
    pub min_height: u64,
    pub max_height: u64,
    pub latest_timestamp: u64,
}

/// Miner block counts for a height range
#[derive(serde::Serialize)]
pub struct MinerCount {
    pub miner: String,
    pub count: u64,
}

/// Query mining pool block counts for a height range, ordered by count DESC.
pub fn query_miner_dominance(
    conn: &Connection,
    from: u64,
    to: u64,
) -> rusqlite::Result<Vec<MinerCount>> {
    let mut stmt = conn.prepare(
        "SELECT miner, COUNT(*) as cnt FROM blocks
         WHERE height >= ?1 AND height <= ?2 AND miner != ''
         GROUP BY miner ORDER BY cnt DESC",
    )?;
    let rows = stmt.query_map(params![from, to], |row| {
        Ok(MinerCount {
            miner: row.get(0)?,
            count: row.get(1)?,
        })
    })?;
    rows.collect()
}

/// Query mining pool block counts for a timestamp range, ordered by count DESC.
pub fn query_miner_dominance_daily(
    conn: &Connection,
    from_ts: u64,
    to_ts: u64,
) -> rusqlite::Result<Vec<MinerCount>> {
    let mut stmt = conn.prepare(
        "SELECT miner, COUNT(*) as cnt FROM blocks
         WHERE timestamp >= ?1 AND timestamp <= ?2 AND miner != ''
         GROUP BY miner ORDER BY cnt DESC",
    )?;
    let rows = stmt.query_map(params![from_ts, to_ts], |row| {
        Ok(MinerCount {
            miner: row.get(0)?,
            count: row.get(1)?,
        })
    })?;
    rows.collect()
}

/// Empty blocks (tx_count == 1, coinbase only) for a height range
pub fn query_empty_blocks(
    conn: &Connection,
    from: u64,
    to: u64,
) -> rusqlite::Result<Vec<(u64, u64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT height, timestamp, miner FROM blocks
         WHERE height >= ?1 AND height <= ?2 AND tx_count <= 1
         ORDER BY height ASC",
    )?;
    let rows = stmt.query_map(params![from, to], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?;
    rows.collect()
}

/// Get database summary (block count, height range, latest timestamp).
/// Uses MIN/MAX on the primary key (instant B-tree lookup) instead of COUNT(*)
/// to avoid a full table scan on 900k+ rows.
pub fn query_stats(conn: &Connection) -> rusqlite::Result<Option<Stats>> {
    // Use MIN/MAX on primary key (instant B-tree lookup) and derive count.
    // Avoids COUNT(*) which forces a full table scan on 900k+ rows.
    // timestamp uses idx_blocks_timestamp index for MAX.
    conn.query_row(
        "SELECT COALESCE(MIN(height),0), COALESCE(MAX(height),0),
                (SELECT timestamp FROM blocks ORDER BY height DESC LIMIT 1)
         FROM blocks",
        [],
        |row| {
            let min_h: u64 = row.get(0)?;
            let max_h: u64 = row.get(1)?;
            let latest_ts: Option<u64> = row.get(2)?;
            match latest_ts {
                Some(ts) => Ok(Some(Stats {
                    block_count: max_h - min_h + 1,
                    min_height: min_h,
                    max_height: max_h,
                    latest_timestamp: ts,
                })),
                None => Ok(None),
            }
        },
    )
}

/// Get the timestamp of a block at a specific height.
pub fn query_block_timestamp(
    conn: &Connection,
    height: u64,
) -> rusqlite::Result<Option<u64>> {
    conn.query_row(
        "SELECT timestamp FROM blocks WHERE height = ?1",
        params![height],
        |row| row.get(0),
    )
    .optional()
}

// === Mempool transaction functions ===

/// Insert a mempool transaction (ignore if txid already exists).
#[allow(clippy::too_many_arguments)]
pub fn insert_mempool_tx(
    conn: &Connection,
    txid: &str,
    fee: u64,
    vsize: u32,
    value: u64,
    first_seen: u64,
    notable_type: Option<&str>,
    value_usd: Option<f64>,
    input_count: u64,
    output_count: u64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO mempool_txs
             (txid, fee, vsize, value, first_seen, notable_type, value_usd,
              input_count, output_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            txid, fee, vsize, value, first_seen, notable_type, value_usd,
            input_count, output_count
        ],
    )?;
    Ok(())
}

/// Mark a list of txids as confirmed in a specific block.
/// Mark a list of txids as confirmed in a specific block.
/// Uses batched IN clauses (100 per batch) for ~10x faster execution.
/// Returns (confirmed_count, total_fees_sats) — fees summed from our mempool data.
pub fn confirm_mempool_txs(
    conn: &Connection,
    txids: &[String],
    height: u64,
    confirmed_at: u64,
) -> rusqlite::Result<(u64, u64)> {
    if txids.is_empty() {
        return Ok((0, 0));
    }
    let tx = conn.unchecked_transaction()?;
    let mut total_fees = 0u64;
    let mut count = 0u64;
    let chunk_size = 100;

    // Sum fees before confirming (only for txs we have in our mempool)
    for chunk in txids.chunks(chunk_size) {
        let placeholders: Vec<&str> = chunk.iter().map(|_| "?").collect();
        let sql = format!(
            "SELECT COALESCE(SUM(fee), 0) FROM mempool_txs
             WHERE txid IN ({}) AND confirmed_height IS NULL",
            placeholders.join(",")
        );
        let mut stmt = tx.prepare(&sql)?;
        for (i, txid) in chunk.iter().enumerate() {
            stmt.raw_bind_parameter(i + 1, txid.as_str())?;
        }
        let mut rows = stmt.raw_query();
        if let Some(row) = rows.next()? {
            total_fees += row.get::<_, i64>(0).unwrap_or(0) as u64;
        }
    }

    // Now confirm them
    for chunk in txids.chunks(chunk_size) {
        let placeholders: Vec<&str> = chunk.iter().map(|_| "?").collect();
        let sql = format!(
            "UPDATE mempool_txs SET confirmed_height = ?1, confirmed_at = ?2
             WHERE txid IN ({}) AND confirmed_height IS NULL",
            placeholders.join(",")
        );
        let mut stmt = tx.prepare(&sql)?;
        let mut param_idx = 1;
        stmt.raw_bind_parameter(param_idx, height as i64)?;
        param_idx += 1;
        stmt.raw_bind_parameter(param_idx, confirmed_at as i64)?;
        param_idx += 1;
        for txid in chunk {
            stmt.raw_bind_parameter(param_idx, txid.as_str())?;
            param_idx += 1;
        }
        count += stmt.raw_execute()? as u64;
    }
    tx.commit()?;
    Ok((count, total_fees))
}

/// Query recent unconfirmed mempool transactions (for SSE history).
pub fn query_recent_mempool_txs(
    conn: &Connection,
    since: u64,
    limit: u64,
) -> rusqlite::Result<Vec<MempoolTxRow>> {
    let mut stmt = conn.prepare_cached(
        "SELECT txid, fee, vsize, value, first_seen, notable_type, value_usd,
                input_count, output_count
         FROM mempool_txs
         WHERE confirmed_height IS NULL AND first_seen >= ?1
         ORDER BY first_seen DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![since, limit], |row| {
        Ok(MempoolTxRow {
            txid: row.get(0)?,
            fee: row.get(1)?,
            vsize: row.get(2)?,
            value: row.get(3)?,
            first_seen: row.get(4)?,
            notable_type: row.get(5)?,
            value_usd: row.get(6)?,
            input_count: row.get(7)?,
            output_count: row.get(8)?,
        })
    })?;
    rows.collect()
}

/// Prune old transactions (confirmed + stale unconfirmed). Keep last N days.
pub fn prune_mempool_txs(
    conn: &Connection,
    older_than: u64,
) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM mempool_txs WHERE first_seen < ?1",
        params![older_than],
    )
}

/// Row from the mempool_txs table, used for SSE history on connect.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MempoolTxRow {
    pub txid: String,
    /// Transaction fee in satoshis.
    pub fee: u64,
    /// Virtual size in vbytes.
    pub vsize: u32,
    /// Total output value in satoshis.
    pub value: u64,
    /// Unix timestamp when the transaction was first observed via ZMQ.
    pub first_seen: u64,
    /// Notable transaction type: "whale", "fee_outlier", "consolidation", "fan_out",
    /// "large_inscription", "ancient_spend", or null for normal transactions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notable_type: Option<String>,
    /// Estimated USD value at time of detection (for whale txs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_usd: Option<f64>,
    /// Number of inputs in the transaction.
    pub input_count: u64,
    /// Number of outputs in the transaction.
    pub output_count: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
// Notable Transactions (Whale Watch) — persistent, long-lived table
// ═══════════════════════════════════════════════════════════════════════════

/// A notable transaction record used for the Whale Watch feature.
/// Derives Default so filters and partial queries are easy.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct NotableTx {
    pub txid: String,
    pub notable_type: String,
    pub fee: u64,
    pub vsize: u32,
    pub value: u64,
    pub max_output_value: u64,
    pub value_usd: f64,
    pub input_count: u64,
    pub output_count: u64,
    pub witness_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_return_text: Option<String>,
    pub first_seen: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_height: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_at: Option<u64>,
}

/// Insert a notable tx. Uses INSERT OR IGNORE so duplicates on reconnect are safe.
#[allow(clippy::too_many_arguments)]
pub fn insert_notable_tx(
    conn: &Connection,
    txid: &str,
    notable_type: &str,
    fee: u64,
    vsize: u32,
    value: u64,
    max_output_value: u64,
    value_usd: f64,
    input_count: u64,
    output_count: u64,
    witness_bytes: u64,
    op_return_text: Option<&str>,
    first_seen: u64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO notable_txs
         (txid, notable_type, fee, vsize, value, max_output_value, value_usd,
          input_count, output_count, witness_bytes, op_return_text, first_seen)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            txid,
            notable_type,
            fee,
            vsize,
            value,
            max_output_value,
            value_usd,
            input_count,
            output_count,
            witness_bytes,
            op_return_text,
            first_seen
        ],
    )?;
    Ok(())
}

/// Mark notable txs as confirmed when a block confirms them.
/// Takes a list of txids and updates confirmed_height/confirmed_at for any matches.
pub fn confirm_notable_txs(
    conn: &Connection,
    txids: &[String],
    height: u64,
    confirmed_at: u64,
) -> rusqlite::Result<u64> {
    if txids.is_empty() {
        return Ok(0);
    }
    let tx = conn.unchecked_transaction()?;
    let mut total = 0u64;
    for chunk in txids.chunks(100) {
        let placeholders: Vec<&str> = chunk.iter().map(|_| "?").collect();
        let sql = format!(
            "UPDATE notable_txs SET confirmed_height = ?1, confirmed_at = ?2
             WHERE txid IN ({}) AND confirmed_height IS NULL",
            placeholders.join(",")
        );
        let mut stmt = tx.prepare(&sql)?;
        let mut param_idx = 1;
        stmt.raw_bind_parameter(param_idx, height as i64)?;
        param_idx += 1;
        stmt.raw_bind_parameter(param_idx, confirmed_at as i64)?;
        param_idx += 1;
        for txid in chunk {
            stmt.raw_bind_parameter(param_idx, txid.as_str())?;
            param_idx += 1;
        }
        total += stmt.raw_execute()? as u64;
    }
    tx.commit()?;
    Ok(total)
}

/// Filter parameters for notable tx queries.
#[derive(Debug, Clone, Default)]
pub struct NotableFilter {
    pub notable_type: Option<String>,
    pub since: Option<u64>, // first_seen >= since
    pub until: Option<u64>, // first_seen <= until
    pub min_value_usd: Option<f64>,
    pub confirmed_only: bool,
    pub unconfirmed_only: bool,
}

/// Query notable txs with optional filters and pagination.
pub fn query_notable_txs(
    conn: &Connection,
    filter: &NotableFilter,
    limit: u64,
    offset: u64,
) -> rusqlite::Result<Vec<NotableTx>> {
    let mut conditions: Vec<String> = Vec::new();
    let mut idx = 1usize;

    if filter.notable_type.is_some() {
        conditions.push(format!("notable_type = ?{idx}"));
        idx += 1;
    }
    if filter.since.is_some() {
        conditions.push(format!("first_seen >= ?{idx}"));
        idx += 1;
    }
    if filter.until.is_some() {
        conditions.push(format!("first_seen <= ?{idx}"));
        idx += 1;
    }
    if filter.min_value_usd.is_some() {
        conditions.push(format!("value_usd >= ?{idx}"));
        idx += 1;
    }
    if filter.confirmed_only {
        conditions.push("confirmed_height IS NOT NULL".to_string());
    }
    if filter.unconfirmed_only {
        conditions.push("confirmed_height IS NULL".to_string());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let limit_idx = idx;
    let offset_idx = idx + 1;
    let sql = format!(
        "SELECT txid, notable_type, fee, vsize, value, max_output_value, value_usd,
                input_count, output_count, witness_bytes, op_return_text,
                first_seen, confirmed_height, confirmed_at
         FROM notable_txs {where_clause}
         ORDER BY first_seen DESC
         LIMIT ?{limit_idx} OFFSET ?{offset_idx}"
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut param_idx = 1usize;
    if let Some(ref t) = filter.notable_type {
        stmt.raw_bind_parameter(param_idx, t.as_str())?;
        param_idx += 1;
    }
    if let Some(s) = filter.since {
        stmt.raw_bind_parameter(param_idx, s as i64)?;
        param_idx += 1;
    }
    if let Some(u) = filter.until {
        stmt.raw_bind_parameter(param_idx, u as i64)?;
        param_idx += 1;
    }
    if let Some(m) = filter.min_value_usd {
        stmt.raw_bind_parameter(param_idx, m)?;
        param_idx += 1;
    }
    stmt.raw_bind_parameter(param_idx, limit as i64)?;
    stmt.raw_bind_parameter(param_idx + 1, offset as i64)?;

    let mut rows = stmt.raw_query();
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        result.push(NotableTx {
            txid: row.get(0)?,
            notable_type: row.get(1)?,
            fee: row.get(2)?,
            vsize: row.get(3)?,
            value: row.get(4)?,
            max_output_value: row.get(5)?,
            value_usd: row.get(6)?,
            input_count: row.get(7)?,
            output_count: row.get(8)?,
            witness_bytes: row.get(9)?,
            op_return_text: row.get(10)?,
            first_seen: row.get(11)?,
            confirmed_height: row.get(12)?,
            confirmed_at: row.get(13)?,
        });
    }
    Ok(result)
}

/// Count notable txs matching a filter (for pagination).
pub fn count_notable_txs(
    conn: &Connection,
    filter: &NotableFilter,
) -> rusqlite::Result<u64> {
    let mut conditions: Vec<String> = Vec::new();
    let mut idx = 1usize;

    if filter.notable_type.is_some() {
        conditions.push(format!("notable_type = ?{idx}"));
        idx += 1;
    }
    if filter.since.is_some() {
        conditions.push(format!("first_seen >= ?{idx}"));
        idx += 1;
    }
    if filter.until.is_some() {
        conditions.push(format!("first_seen <= ?{idx}"));
        idx += 1;
    }
    if filter.min_value_usd.is_some() {
        conditions.push(format!("value_usd >= ?{idx}"));
        #[allow(unused_assignments)]
        {
            idx += 1;
        }
    }
    if filter.confirmed_only {
        conditions.push("confirmed_height IS NOT NULL".to_string());
    }
    if filter.unconfirmed_only {
        conditions.push("confirmed_height IS NULL".to_string());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!("SELECT COUNT(*) FROM notable_txs {where_clause}");
    let mut stmt = conn.prepare(&sql)?;
    let mut param_idx = 1usize;
    if let Some(ref t) = filter.notable_type {
        stmt.raw_bind_parameter(param_idx, t.as_str())?;
        param_idx += 1;
    }
    if let Some(s) = filter.since {
        stmt.raw_bind_parameter(param_idx, s as i64)?;
        param_idx += 1;
    }
    if let Some(u) = filter.until {
        stmt.raw_bind_parameter(param_idx, u as i64)?;
        param_idx += 1;
    }
    if let Some(m) = filter.min_value_usd {
        stmt.raw_bind_parameter(param_idx, m)?;
    }

    let mut rows = stmt.raw_query();
    if let Some(row) = rows.next()? {
        Ok(row.get::<_, i64>(0)? as u64)
    } else {
        Ok(0)
    }
}

/// Aggregate statistics for notable txs in a time window.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct NotableStats {
    pub total_count: u64,
    pub total_value_usd: f64,
    pub by_type: Vec<(String, u64, f64)>, // (type, count, total_usd)
    pub top_value_usd: f64,
    pub top_txid: Option<String>,
}

/// Get aggregate stats for notable txs in a time window.
pub fn query_notable_stats(
    conn: &Connection,
    since: u64,
) -> rusqlite::Result<NotableStats> {
    // Total count + total USD
    let (total_count, total_value_usd): (u64, f64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(value_usd), 0)
         FROM notable_txs WHERE first_seen >= ?1",
        params![since],
        |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, f64>(1)?)),
    )?;

    // By type
    let mut stmt = conn.prepare(
        "SELECT notable_type, COUNT(*), COALESCE(SUM(value_usd), 0)
         FROM notable_txs WHERE first_seen >= ?1
         GROUP BY notable_type
         ORDER BY COUNT(*) DESC",
    )?;
    let by_type: Vec<(String, u64, f64)> = stmt
        .query_map(params![since], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)? as u64,
                r.get::<_, f64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // Top single tx
    let top: Option<(String, f64)> = conn
        .query_row(
            "SELECT txid, value_usd FROM notable_txs
             WHERE first_seen >= ?1
             ORDER BY value_usd DESC LIMIT 1",
            params![since],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)),
        )
        .ok();

    Ok(NotableStats {
        total_count,
        total_value_usd,
        by_type,
        top_value_usd: top.as_ref().map(|(_, v)| *v).unwrap_or(0.0),
        top_txid: top.map(|(t, _)| t),
    })
}

/// Query txids that are still unconfirmed (survived block confirmation).
pub fn query_unconfirmed_txids(
    conn: &Connection,
    limit: u64,
) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare_cached(
        "SELECT txid FROM mempool_txs
         WHERE confirmed_height IS NULL
         ORDER BY first_seen DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |row| row.get(0))?;
    rows.collect()
}

/// Query extreme records with the block heights where each MAX occurred.
/// Uses subqueries to find the specific block for each extreme.
/// Query extreme records for a time range.
///
/// Uses a two-pass approach: one scan to find all MAX values, then individual
/// lookups by (column = max_value) to retrieve the block details. With the
/// column indexes, the second pass lookups are near-instant.
pub fn query_extremes_with_heights(
    conn: &Connection,
    from_ts: u64,
    to_ts: u64,
) -> rusqlite::Result<super::types::ExtremesData> {
    use super::types::{ExtremeRecord, ExtremeRecordF64, ExtremesData};

    // Pass 1: single scan to get all MAX values + counts
    let row = conn.query_row(
        "SELECT
            MAX(size), MAX(total_fees), MAX(median_fee_rate), MAX(fee_rate_p90),
            MAX(tx_count), MAX(largest_tx_size), MAX(input_count), MAX(output_count),
            MAX(inscription_count), MAX(runes_count), MAX(op_return_count),
            MAX(rbf_count), MAX(taproot_spend_count), MAX(total_output_value),
            SUM(CASE WHEN tx_count <= 1 THEN 1 ELSE 0 END), COUNT(*)
         FROM blocks WHERE timestamp >= ?1 AND timestamp <= ?2",
        params![from_ts, to_ts],
        |row| {
            Ok((
                row.get::<_, Option<u64>>(0)?.unwrap_or(0),   // max_size
                row.get::<_, Option<u64>>(1)?.unwrap_or(0),   // max_fees
                row.get::<_, Option<f64>>(2)?.unwrap_or(0.0), // max_median_fee_rate
                row.get::<_, Option<f64>>(3)?.unwrap_or(0.0), // max_p90_fee_rate
                row.get::<_, Option<u64>>(4)?.unwrap_or(0),   // max_tx_count
                row.get::<_, Option<u64>>(5)?.unwrap_or(0),   // max_largest_tx
                row.get::<_, Option<u64>>(6)?.unwrap_or(0),   // max_inputs
                row.get::<_, Option<u64>>(7)?.unwrap_or(0),   // max_outputs
                row.get::<_, Option<u64>>(8)?.unwrap_or(0),   // max_inscriptions
                row.get::<_, Option<u64>>(9)?.unwrap_or(0),   // max_runes
                row.get::<_, Option<u64>>(10)?.unwrap_or(0),  // max_op_returns
                row.get::<_, Option<u64>>(11)?.unwrap_or(0),  // max_rbf
                row.get::<_, Option<u64>>(12)?.unwrap_or(0),  // max_taproot
                row.get::<_, Option<u64>>(13)?.unwrap_or(0),  // max_output_value
                row.get::<_, Option<u64>>(14)?.unwrap_or(0),  // empty_count
                row.get::<_, Option<u64>>(15)?.unwrap_or(0),  // block_count
            ))
        },
    )?;

    let (
        max_size,
        max_fees,
        max_mfr,
        max_p90,
        max_txs,
        max_ltx,
        max_in,
        max_out,
        max_ins,
        max_run,
        max_opr,
        max_rbf,
        max_tap,
        max_val,
        empty_count,
        block_count,
    ) = row;

    // Pass 2: look up the block that holds each maximum (index-assisted)
    fn lookup_u64(
        conn: &Connection,
        col: &str,
        val: u64,
        from_ts: u64,
        to_ts: u64,
    ) -> rusqlite::Result<ExtremeRecord> {
        let sql = format!(
            "SELECT {col}, height, timestamp, miner FROM blocks
             WHERE {col} = ?1 AND timestamp >= ?2 AND timestamp <= ?3
             LIMIT 1",
            col = col
        );
        conn.query_row(&sql, params![val, from_ts, to_ts], |r| {
            Ok(ExtremeRecord {
                value: r.get::<_, Option<u64>>(0)?.unwrap_or(0),
                height: r.get(1)?,
                timestamp: r.get(2)?,
                miner: r.get(3)?,
            })
        })
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                Ok(ExtremeRecord::default())
            }
            other => Err(other),
        })
    }

    fn lookup_f64(
        conn: &Connection,
        col: &str,
        val: f64,
        from_ts: u64,
        to_ts: u64,
    ) -> rusqlite::Result<ExtremeRecordF64> {
        let sql = format!(
            "SELECT {col}, height, timestamp, miner FROM blocks
             WHERE {col} = ?1 AND timestamp >= ?2 AND timestamp <= ?3
             LIMIT 1",
            col = col
        );
        conn.query_row(&sql, params![val, from_ts, to_ts], |r| {
            Ok(ExtremeRecordF64 {
                value: r.get::<_, Option<f64>>(0)?.unwrap_or(0.0),
                height: r.get(1)?,
                timestamp: r.get(2)?,
                miner: r.get(3)?,
            })
        })
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                Ok(ExtremeRecordF64::default())
            }
            other => Err(other),
        })
    }

    Ok(ExtremesData {
        largest_block: lookup_u64(conn, "size", max_size, from_ts, to_ts)?,
        highest_fee_block: lookup_u64(
            conn,
            "total_fees",
            max_fees,
            from_ts,
            to_ts,
        )?,
        peak_fee_rate: lookup_f64(
            conn,
            "median_fee_rate",
            max_mfr,
            from_ts,
            to_ts,
        )?,
        peak_p90_fee_rate: lookup_f64(
            conn,
            "fee_rate_p90",
            max_p90,
            from_ts,
            to_ts,
        )?,
        most_txs: lookup_u64(conn, "tx_count", max_txs, from_ts, to_ts)?,
        largest_tx: lookup_u64(
            conn,
            "largest_tx_size",
            max_ltx,
            from_ts,
            to_ts,
        )?,
        most_inputs: lookup_u64(conn, "input_count", max_in, from_ts, to_ts)?,
        most_outputs: lookup_u64(
            conn,
            "output_count",
            max_out,
            from_ts,
            to_ts,
        )?,
        most_inscriptions: lookup_u64(
            conn,
            "inscription_count",
            max_ins,
            from_ts,
            to_ts,
        )?,
        most_runes: lookup_u64(conn, "runes_count", max_run, from_ts, to_ts)?,
        most_op_returns: lookup_u64(
            conn,
            "op_return_count",
            max_opr,
            from_ts,
            to_ts,
        )?,
        most_rbf: lookup_u64(conn, "rbf_count", max_rbf, from_ts, to_ts)?,
        most_taproot: lookup_u64(
            conn,
            "taproot_spend_count",
            max_tap,
            from_ts,
            to_ts,
        )?,
        highest_value: lookup_u64(
            conn,
            "total_output_value",
            max_val,
            from_ts,
            to_ts,
        )?,
        empty_block_count: empty_count,
        block_count,
    })
}

/// Get the stored hash for a block at a given height. Returns None if not found.
pub fn query_block_hash(
    conn: &Connection,
    height: u64,
) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT hash FROM blocks WHERE height = ?1",
        params![height],
        |row| row.get(0),
    )
    .optional()
}

/// Delete a block at a specific height (for reorg correction).
pub fn delete_block(conn: &Connection, height: u64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM blocks WHERE height = ?1", params![height])
}

/// Create the reorgs table to log detected chain reorganizations.
pub fn init_reorgs_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS reorgs (
            height          INTEGER NOT NULL,
            stale_hash      TEXT NOT NULL,
            canonical_hash  TEXT NOT NULL,
            detected_at     INTEGER NOT NULL,
            PRIMARY KEY (height, stale_hash)
        );",
    )
}

/// Log a detected reorg (stale block replaced by canonical block).
pub fn insert_reorg(
    conn: &Connection,
    height: u64,
    stale_hash: &str,
    canonical_hash: &str,
) -> rusqlite::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    conn.execute(
        "INSERT OR IGNORE INTO reorgs (height, stale_hash, canonical_hash, detected_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![height, stale_hash, canonical_hash, now],
    )?;
    Ok(())
}

/// Bucket counts for block weight fullness distribution (10 buckets: 0-10% .. 90-100%).
/// Computed server-side so ALL range doesn't send 940k rows to the client.
pub fn query_fullness_histogram(
    conn: &Connection,
    from_ts: u64,
    to_ts: u64,
) -> rusqlite::Result<Vec<(String, u64)>> {
    let mut stmt = conn.prepare(
        "SELECT CAST(CASE
            WHEN weight * 100.0 / 4000000.0 >= 100 THEN 9
            ELSE weight * 100.0 / 4000000.0 / 10
         END AS INT) AS bucket, COUNT(*)
         FROM blocks
         WHERE timestamp >= ?1 AND timestamp <= ?2
         GROUP BY bucket
         ORDER BY bucket ASC",
    )?;
    let labels = [
        "0-10%", "10-20%", "20-30%", "30-40%", "40-50%", "50-60%", "60-70%",
        "70-80%", "80-90%", "90-100%",
    ];
    let mut result = [0u64; 10];
    let rows = stmt.query_map(params![from_ts, to_ts], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, u64>(1)?))
    })?;
    for (bucket, count) in rows.flatten() {
        let idx = (bucket as usize).min(9);
        result[idx] = count;
    }
    Ok(labels
        .iter()
        .zip(result.iter())
        .map(|(l, c)| (l.to_string(), *c))
        .collect())
}

/// Bucket counts for inter-block time distribution (61 buckets: 0-1min .. 59-60min, 60+).
/// Uses LAG window function so the entire computation runs in SQL.
pub fn query_block_time_histogram(
    conn: &Connection,
    from_ts: u64,
    to_ts: u64,
) -> rusqlite::Result<Vec<(String, u64)>> {
    let mut stmt = conn.prepare(
        "WITH intervals AS (
            SELECT timestamp - LAG(timestamp) OVER (ORDER BY height) AS gap
            FROM blocks
            WHERE timestamp >= ?1 AND timestamp <= ?2
        )
        SELECT CAST(CASE
            WHEN gap IS NULL THEN -1
            WHEN gap / 60 >= 60 THEN 60
            ELSE gap / 60
        END AS INT) AS bucket, COUNT(*)
        FROM intervals
        WHERE gap IS NOT NULL AND gap >= 0
        GROUP BY bucket
        ORDER BY bucket ASC",
    )?;
    let mut labels: Vec<String> =
        (0..60).map(|i| format!("{}-{}", i, i + 1)).collect();
    labels.push("60+".to_string());
    let mut result = vec![0u64; 61];
    let rows = stmt.query_map(params![from_ts, to_ts], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, u64>(1)?))
    })?;
    for (bucket, count) in rows.flatten() {
        if bucket >= 0 {
            let idx = (bucket as usize).min(60);
            result[idx] = count;
        }
    }
    Ok(labels.into_iter().zip(result).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn test_mempool_insert_and_query() {
        let conn = setup_db();
        insert_mempool_tx(
            &conn, "abc123", 500, 200, 1_000_000, 1700000000, None, None, 0, 0,
        )
        .unwrap();

        let txs = query_recent_mempool_txs(&conn, 0, 100).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].txid, "abc123");
        assert_eq!(txs[0].fee, 500);
        assert_eq!(txs[0].vsize, 200);
        assert_eq!(txs[0].value, 1_000_000);
        assert_eq!(txs[0].first_seen, 1700000000);
    }

    #[test]
    fn test_mempool_insert_duplicate_ignored() {
        let conn = setup_db();
        insert_mempool_tx(
            &conn, "dup_tx", 100, 150, 500_000, 1700000000, None, None, 0, 0,
        )
        .unwrap();
        insert_mempool_tx(
            &conn, "dup_tx", 200, 250, 600_000, 1700000001, None, None, 0, 0,
        )
        .unwrap();

        let count: u64 = conn
            .query_row("SELECT COUNT(*) FROM mempool_txs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // Original values preserved (INSERT OR IGNORE)
        let txs = query_recent_mempool_txs(&conn, 0, 100).unwrap();
        assert_eq!(txs[0].fee, 100);
    }

    #[test]
    fn test_mempool_confirm() {
        let conn = setup_db();
        insert_mempool_tx(
            &conn, "tx1", 100, 150, 500_000, 1700000000, None, None, 0, 0,
        )
        .unwrap();
        insert_mempool_tx(
            &conn, "tx2", 200, 250, 600_000, 1700000001, None, None, 0, 0,
        )
        .unwrap();
        insert_mempool_tx(
            &conn, "tx3", 300, 350, 700_000, 1700000002, None, None, 0, 0,
        )
        .unwrap();

        let txids = vec!["tx1".to_string(), "tx2".to_string()];
        let (confirmed, fees) =
            confirm_mempool_txs(&conn, &txids, 800_000, 1700001000).unwrap();
        assert_eq!(confirmed, 2);
        assert_eq!(fees, 300); // tx1=100 + tx2=200

        // Verify confirmed_height is set
        let height: Option<u64> = conn
            .query_row(
                "SELECT confirmed_height FROM mempool_txs WHERE txid = 'tx1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(height, Some(800_000));
    }

    #[test]
    fn test_mempool_query_unconfirmed_only() {
        let conn = setup_db();
        insert_mempool_tx(
            &conn, "tx1", 100, 150, 500_000, 1700000000, None, None, 0, 0,
        )
        .unwrap();
        insert_mempool_tx(
            &conn, "tx2", 200, 250, 600_000, 1700000001, None, None, 0, 0,
        )
        .unwrap();
        insert_mempool_tx(
            &conn, "tx3", 300, 350, 700_000, 1700000002, None, None, 0, 0,
        )
        .unwrap();

        // Confirm tx1
        confirm_mempool_txs(&conn, &["tx1".to_string()], 800_000, 1700001000)
            .unwrap();

        // query_recent_mempool_txs only returns unconfirmed
        let txs = query_recent_mempool_txs(&conn, 0, 100).unwrap();
        assert_eq!(txs.len(), 2);
        let txids: Vec<&str> = txs.iter().map(|t| t.txid.as_str()).collect();
        assert!(txids.contains(&"tx2"));
        assert!(txids.contains(&"tx3"));
        assert!(!txids.contains(&"tx1"));
    }

    #[test]
    fn test_mempool_prune() {
        let conn = setup_db();
        // Old tx (timestamp 1000)
        insert_mempool_tx(
            &conn, "old_tx", 100, 150, 500_000, 1000, None, None, 0, 0,
        )
        .unwrap();
        // Recent tx (timestamp 2000)
        insert_mempool_tx(
            &conn, "new_tx", 200, 250, 600_000, 2000, None, None, 0, 0,
        )
        .unwrap();

        // Prune anything older than 1500
        let pruned = prune_mempool_txs(&conn, 1500).unwrap();
        assert_eq!(pruned, 1);

        let txs = query_recent_mempool_txs(&conn, 0, 100).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].txid, "new_tx");
    }

    #[test]
    fn test_mempool_query_unconfirmed_txids() {
        let conn = setup_db();
        insert_mempool_tx(
            &conn, "tx1", 100, 150, 500_000, 1700000000, None, None, 0, 0,
        )
        .unwrap();
        insert_mempool_tx(
            &conn, "tx2", 200, 250, 600_000, 1700000001, None, None, 0, 0,
        )
        .unwrap();
        insert_mempool_tx(
            &conn, "tx3", 300, 350, 700_000, 1700000002, None, None, 0, 0,
        )
        .unwrap();

        // Confirm tx2
        confirm_mempool_txs(&conn, &["tx2".to_string()], 800_000, 1700001000)
            .unwrap();

        let unconfirmed = query_unconfirmed_txids(&conn, 100).unwrap();
        assert_eq!(unconfirmed.len(), 2);
        assert!(unconfirmed.contains(&"tx1".to_string()));
        assert!(unconfirmed.contains(&"tx3".to_string()));
        assert!(!unconfirmed.contains(&"tx2".to_string()));
    }

    /// Helper: insert a minimal test block with key fields.
    fn insert_test_block(
        conn: &Connection,
        height: u64,
        timestamp: u64,
        weight: u64,
        tx_count: u64,
        total_fees: u64,
    ) {
        let size = weight / 4;
        conn.execute(
            "INSERT OR REPLACE INTO blocks (
                height, hash, timestamp, tx_count, size, weight, difficulty,
                op_return_count, op_return_bytes, runes_count, runes_bytes,
                data_carrier_count, data_carrier_bytes,
                total_fees, median_fee_rate, segwit_spend_count, taproot_spend_count,
                input_count, output_count, total_output_value, total_input_value
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, 1.0,
                0, 0, 0, 0, 0, 0,
                ?7, 1.0, ?8, 0, ?9, ?10, 50000000, 50100000
            )",
            rusqlite::params![
                height as i64,
                format!("hash_{}", height),
                timestamp as i64,
                tx_count as i64,
                size as i64,
                weight as i64,
                total_fees as i64,
                (tx_count / 2) as i64,
                (tx_count * 2) as i64,
                (tx_count * 3) as i64,
            ],
        ).unwrap();
    }

    #[test]
    fn test_timestamp_to_date() {
        assert_eq!(timestamp_to_date(0), "1970-01-01");
        assert_eq!(timestamp_to_date(1231006505), "2009-01-03"); // genesis block
        assert_eq!(timestamp_to_date(1713571200), "2024-04-20"); // 4th halving
    }

    #[test]
    fn test_fullness_histogram() {
        let conn = setup_db();
        // Block at 50% weight (2M of 4M)
        insert_test_block(&conn, 1, 1700000000, 2_000_000, 100, 1000);
        // Block at 99% weight (3.96M of 4M)
        insert_test_block(&conn, 2, 1700000600, 3_960_000, 200, 2000);
        // Block at 10% weight
        insert_test_block(&conn, 3, 1700001200, 400_000, 50, 500);

        let hist = query_fullness_histogram(&conn, 0, 9_999_999_999).unwrap();
        assert_eq!(hist.len(), 10);
        // 10% block should be in bucket "10-20%"  (index 1)
        assert_eq!(hist[1].1, 1);
        // 50% block should be in bucket "50-60%" (index 5)
        assert_eq!(hist[5].1, 1);
        // 99% block should be in bucket "90-100%" (index 9)
        assert_eq!(hist[9].1, 1);
        // Other buckets should be 0
        assert_eq!(hist[0].1, 0);
    }

    #[test]
    fn test_block_time_histogram() {
        let conn = setup_db();
        // 3 blocks: 10 min apart, then 30 sec apart
        insert_test_block(&conn, 1, 1700000000, 3_900_000, 100, 1000);
        insert_test_block(&conn, 2, 1700000600, 3_900_000, 100, 1000); // +600s = 10 min
        insert_test_block(&conn, 3, 1700000630, 3_900_000, 100, 1000); // +30s = 0.5 min

        let hist = query_block_time_histogram(&conn, 0, 9_999_999_999).unwrap();
        assert_eq!(hist.len(), 61);
        // 30s interval -> bucket "0-1" (index 0)
        assert_eq!(hist[0].1, 1);
        // 600s interval -> bucket "9-10" (index 9, since 600/60 = 10, but integer division gives 10 which maps to "10-11")
        // Actually 600/60 = 10, CAST(10 AS INT) = 10 -> bucket index 10
        assert_eq!(hist[10].1, 1);
    }

    #[test]
    fn test_daily_blocks_rebuild_and_query() {
        let conn = setup_db();
        // Two blocks on same day
        insert_test_block(&conn, 1, 1700000000, 3_000_000, 100, 1000);
        insert_test_block(&conn, 2, 1700000600, 3_500_000, 200, 2000);
        // One block on next day
        insert_test_block(&conn, 3, 1700086400, 2_000_000, 50, 500);

        let count = rebuild_all_daily_blocks(&conn).unwrap();
        assert_eq!(count, 2); // 2 distinct days

        let rows =
            query_daily_aggregates_fast(&conn, 0, 9_999_999_999).unwrap();
        assert_eq!(rows.len(), 2);
        // First day: 2 blocks
        assert_eq!(rows[0].block_count, 2);
        assert_eq!(rows[0].total_fees, 3000); // 1000 + 2000
                                              // Second day: 1 block
        assert_eq!(rows[1].block_count, 1);
        assert_eq!(rows[1].total_fees, 500);
    }

    #[test]
    fn test_daily_blocks_value_columns() {
        let conn = setup_db();
        insert_test_block(&conn, 1, 1700000000, 3_000_000, 100, 1000);
        insert_test_block(&conn, 2, 1700000600, 3_000_000, 200, 2000);

        rebuild_all_daily_blocks(&conn).unwrap();
        let rows =
            query_daily_aggregates_fast(&conn, 0, 9_999_999_999).unwrap();
        assert_eq!(rows.len(), 1);
        // Each block has total_output_value=50000000, so day total = 100000000
        assert_eq!(rows[0].total_output_value, 100_000_000);
        assert_eq!(rows[0].total_input_value, 100_200_000);
    }

    #[test]
    fn test_reorg_detection_functions() {
        let conn = setup_db();
        insert_test_block(&conn, 100, 1700000000, 3_000_000, 100, 1000);

        // Verify stored hash
        let hash = query_block_hash(&conn, 100).unwrap();
        assert_eq!(hash, Some("hash_100".to_string()));

        // Non-existent block returns None
        let missing = query_block_hash(&conn, 999).unwrap();
        assert!(missing.is_none());

        // Delete block
        let deleted = delete_block(&conn, 100).unwrap();
        assert_eq!(deleted, 1);
        assert!(query_block_hash(&conn, 100).unwrap().is_none());

        // Insert reorg record
        insert_reorg(&conn, 100, "stale_hash", "canonical_hash").unwrap();
        let reorg_count: u64 = conn
            .query_row("SELECT COUNT(*) FROM reorgs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(reorg_count, 1);
    }

    #[test]
    fn test_refresh_daily_block() {
        let conn = setup_db();
        insert_test_block(&conn, 1, 1700000000, 3_000_000, 100, 1000);
        rebuild_all_daily_blocks(&conn).unwrap();

        // Add another block on the same day
        insert_test_block(&conn, 2, 1700000600, 3_500_000, 200, 5000);
        let day = timestamp_to_date(1700000000);
        refresh_daily_block(&conn, &day).unwrap();

        let rows =
            query_daily_aggregates_fast(&conn, 0, 9_999_999_999).unwrap();
        assert_eq!(rows[0].block_count, 2);
        assert_eq!(rows[0].total_fees, 6000); // 1000 + 5000
    }
}
