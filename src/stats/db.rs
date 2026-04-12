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
/// v8: OCEAN sub-miners, RBF excludes CSV, witness byte overhead, inscription byte
///     overhead, Runes height-gated to 840k+
pub const BACKFILL_VERSION: u64 = 9;

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

    // Migration: add version, total_fees, miner columns if missing
    let has_version: bool =
        conn.prepare("SELECT version FROM blocks LIMIT 0").is_ok();
    if !has_version {
        tracing::info!("Migrating: adding version, total_fees, miner columns");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN version INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN total_fees INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN miner TEXT NOT NULL DEFAULT '';",
        )?;
    }

    // Migration: add median_fee, median_fee_rate columns if missing
    let has_median: bool = conn
        .prepare("SELECT median_fee FROM blocks LIMIT 0")
        .is_ok();
    if !has_median {
        tracing::info!("Migrating: adding median_fee, median_fee_rate columns");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN median_fee INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN median_fee_rate REAL NOT NULL DEFAULT 0;",
        )?;
    }

    // Migration: add coinbase_locktime column if missing
    let has_locktime: bool = conn
        .prepare("SELECT coinbase_locktime FROM blocks LIMIT 0")
        .is_ok();
    if !has_locktime {
        tracing::info!("Migrating: adding coinbase_locktime column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN coinbase_locktime INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    // Migration: add coinbase_sequence column if missing
    let has_sequence: bool = conn
        .prepare("SELECT coinbase_sequence FROM blocks LIMIT 0")
        .is_ok();
    if !has_sequence {
        tracing::info!("Migrating: adding coinbase_sequence column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN coinbase_sequence INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    // Migration: add segwit/taproot spend counts
    let has_segwit: bool = conn
        .prepare("SELECT segwit_spend_count FROM blocks LIMIT 0")
        .is_ok();
    if !has_segwit {
        tracing::info!(
            "Migrating: adding segwit_spend_count, taproot_spend_count columns"
        );
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN segwit_spend_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN taproot_spend_count INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    // Migration: add omni/counterparty protocol columns
    let has_omni: bool = conn
        .prepare("SELECT omni_count FROM blocks LIMIT 0")
        .is_ok();
    if !has_omni {
        tracing::info!(
            "Migrating: adding omni_count, omni_bytes, counterparty_count, counterparty_bytes columns"
        );
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN omni_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN omni_bytes INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN counterparty_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN counterparty_bytes INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    // Migration: add output type counts, tx metrics, and witness size columns
    let has_p2pkh: bool = conn
        .prepare("SELECT p2pkh_count FROM blocks LIMIT 0")
        .is_ok();
    if !has_p2pkh {
        tracing::info!(
            "Migrating: adding output type counts, input/output counts, rbf_count, witness_bytes"
        );
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

    // Migration: add multisig_count (split from unknown_script_count)
    let has_multisig: bool = conn
        .prepare("SELECT multisig_count FROM blocks LIMIT 0")
        .is_ok();
    if !has_multisig {
        tracing::info!("Migrating: adding multisig_count column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN multisig_count INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    // Migration: add inscription tracking columns
    let has_inscriptions: bool = conn
        .prepare("SELECT inscription_count FROM blocks LIMIT 0")
        .is_ok();
    if !has_inscriptions {
        tracing::info!(
            "Migrating: adding inscription_count, inscription_bytes columns"
        );
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN inscription_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN inscription_bytes INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    // Migration: add brc20_count column
    let has_brc20: bool = conn
        .prepare("SELECT brc20_count FROM blocks LIMIT 0")
        .is_ok();
    if !has_brc20 {
        tracing::info!("Migrating: adding brc20_count column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN brc20_count INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    // Migration: add backfill_version column if missing
    // Tracks which backfill pass has been applied. Blocks with
    // backfill_version < BACKFILL_VERSION are re-fetched on startup.
    let has_bf_version: bool = conn
        .prepare("SELECT backfill_version FROM blocks LIMIT 0")
        .is_ok();
    if !has_bf_version {
        tracing::info!("Migrating: adding backfill_version column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN backfill_version INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    // Migration: add taproot key-path/script-path spend counts
    let has_keypath: bool = conn
        .prepare("SELECT taproot_keypath_count FROM blocks LIMIT 0")
        .is_ok();
    if !has_keypath {
        tracing::info!("Migrating: adding taproot_keypath_count, taproot_scriptpath_count columns");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN taproot_keypath_count INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE blocks ADD COLUMN taproot_scriptpath_count INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    let has_output_value: bool = conn
        .prepare("SELECT total_output_value FROM blocks LIMIT 0")
        .is_ok();
    if !has_output_value {
        tracing::info!("Migrating: adding total_output_value column");
        conn.execute_batch(
            "ALTER TABLE blocks ADD COLUMN total_output_value INTEGER NOT NULL DEFAULT 0;",
        )?;
    }

    let has_input_value: bool = conn
        .prepare("SELECT total_input_value FROM blocks LIMIT 0")
        .is_ok();
    if !has_input_value {
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
            avg_median_fee_rate        REAL NOT NULL DEFAULT 0
        );",
    )?;

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
              inscription_count, inscription_bytes, brc20_count,
              total_output_value, total_input_value,
              fee_rate_p10, fee_rate_p90, stamps_count, largest_tx_size,
              backfill_version)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,?36,?37,?38,?39,?40,?41,?42,?43,?44,?45,?46,?47,?48,?49,?50)",
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
                block.brc20_count,
                block.total_output_value,
                block.total_input_value,
                block.fee_rate_p10,
                block.fee_rate_p90,
                block.stamps_count,
                block.largest_tx_size,
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
            "UPDATE blocks SET version = ?1, total_fees = ?2, miner = ?3, median_fee = ?4, median_fee_rate = ?5, coinbase_locktime = ?6, coinbase_sequence = ?7, segwit_spend_count = ?8, taproot_spend_count = ?9, omni_count = ?10, omni_bytes = ?11, counterparty_count = ?12, counterparty_bytes = ?13, runes_count = ?14, runes_bytes = ?15, data_carrier_count = ?16, data_carrier_bytes = ?17, p2pk_count = ?18, p2pkh_count = ?19, p2sh_count = ?20, p2wpkh_count = ?21, p2wsh_count = ?22, p2tr_count = ?23, multisig_count = ?24, unknown_script_count = ?25, input_count = ?26, output_count = ?27, rbf_count = ?28, witness_bytes = ?29, inscription_count = ?30, inscription_bytes = ?31, brc20_count = ?32, taproot_keypath_count = ?33, taproot_scriptpath_count = ?34, total_output_value = ?35, total_input_value = ?36, fee_rate_p10 = ?37, fee_rate_p90 = ?38, stamps_count = ?39, largest_tx_size = ?40, backfill_version = ?41 WHERE height = ?42",
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
                block.brc20_count,
                block.taproot_keypath_count,
                block.taproot_scriptpath_count,
                block.total_output_value,
                block.total_input_value,
                block.fee_rate_p10,
                block.fee_rate_p90,
                block.stamps_count,
                block.largest_tx_size,
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
                inscription_count, inscription_bytes, brc20_count,
                op_return_count, op_return_bytes,
                runes_count, runes_bytes, omni_count, omni_bytes,
                counterparty_count, counterparty_bytes,
                data_carrier_count, data_carrier_bytes,
                taproot_keypath_count, taproot_scriptpath_count,
                total_output_value, total_input_value,
                fee_rate_p10, fee_rate_p90, stamps_count, largest_tx_size
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
            brc20_count: row.get(26)?,
            op_return_count: row.get(27)?,
            op_return_bytes: row.get(28)?,
            runes_count: row.get(29)?,
            runes_bytes: row.get(30)?,
            omni_count: row.get(31)?,
            omni_bytes: row.get(32)?,
            counterparty_count: row.get(33)?,
            counterparty_bytes: row.get(34)?,
            data_carrier_count: row.get(35)?,
            data_carrier_bytes: row.get(36)?,
            taproot_keypath_count: row.get(37)?,
            taproot_scriptpath_count: row.get(38)?,
            total_output_value: row.get(39)?,
            total_input_value: row.get(40)?,
            fee_rate_p10: row.get(41)?,
            fee_rate_p90: row.get(42)?,
            stamps_count: row.get(43)?,
            largest_tx_size: row.get(44)?,
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
                inscription_count, inscription_bytes, brc20_count,
                op_return_count, op_return_bytes,
                runes_count, runes_bytes, omni_count, omni_bytes,
                counterparty_count, counterparty_bytes,
                data_carrier_count, data_carrier_bytes,
                taproot_keypath_count, taproot_scriptpath_count,
                total_output_value, total_input_value,
                fee_rate_p10, fee_rate_p90, stamps_count, largest_tx_size
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
            brc20_count: row.get(26)?,
            op_return_count: row.get(27)?,
            op_return_bytes: row.get(28)?,
            runes_count: row.get(29)?,
            runes_bytes: row.get(30)?,
            omni_count: row.get(31)?,
            omni_bytes: row.get(32)?,
            counterparty_count: row.get(33)?,
            counterparty_bytes: row.get(34)?,
            data_carrier_count: row.get(35)?,
            data_carrier_bytes: row.get(36)?,
            taproot_keypath_count: row.get(37)?,
            taproot_scriptpath_count: row.get(38)?,
            total_output_value: row.get(39)?,
            total_input_value: row.get(40)?,
            fee_rate_p10: row.get(41)?,
            fee_rate_p90: row.get(42)?,
            stamps_count: row.get(43)?,
            largest_tx_size: row.get(44)?,
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
                version: row.get::<_, u32>(15)?,
                total_fees: row.get(16)?,
                miner: row.get(17)?,
                median_fee: row.get(18)?,
                median_fee_rate: row.get(19)?,
                coinbase_locktime: row.get(20)?,
                coinbase_sequence: row.get(21)?,
                segwit_spend_count: row.get(22)?,
                taproot_spend_count: row.get(23)?,
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
}

/// Rebuild a single day's row in the daily_blocks table by re-aggregating
/// from the raw blocks table. Called after new block ingestion.
pub fn refresh_daily_block(conn: &Connection, day: &str) -> rusqlite::Result<()> {
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
             avg_fee_rate_p10, avg_fee_rate_p90, avg_stamps_count, avg_median_fee_rate)
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
                AVG(fee_rate_p10), AVG(fee_rate_p90), AVG(stamps_count), AVG(median_fee_rate)
         FROM blocks
         WHERE date(datetime(timestamp, 'unixepoch')) = ?1",
        params![day],
    )?;
    Ok(())
}

/// Populate the entire daily_blocks table from scratch. Used on first run
/// when the table is empty but blocks already exist.
pub fn rebuild_all_daily_blocks(conn: &Connection) -> rusqlite::Result<u64> {
    let count: u64 = conn.query_row(
        "SELECT COUNT(*) FROM daily_blocks", [], |r| r.get(0),
    )?;
    let block_count: u64 = conn.query_row(
        "SELECT COUNT(*) FROM blocks", [], |r| r.get(0),
    )?;

    // Only rebuild if blocks exist but daily_blocks is empty
    if count > 0 || block_count == 0 {
        return Ok(count);
    }

    tracing::info!("Building daily_blocks table from {} blocks...", block_count);
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
             avg_fee_rate_p10, avg_fee_rate_p90, avg_stamps_count, avg_median_fee_rate)
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
                AVG(fee_rate_p10), AVG(fee_rate_p90), AVG(stamps_count), AVG(median_fee_rate)
         FROM blocks
         GROUP BY date(datetime(timestamp, 'unixepoch'))"
    )?;
    let new_count: u64 = conn.query_row(
        "SELECT COUNT(*) FROM daily_blocks", [], |r| r.get(0),
    )?;
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
                avg_fee_rate_p10, avg_fee_rate_p90, avg_stamps_count, avg_median_fee_rate
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
                AVG(median_fee_rate)
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
                MAX(median_fee_rate)
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
pub fn insert_mempool_tx(
    conn: &Connection,
    txid: &str,
    fee: u64,
    vsize: u32,
    value: u64,
    first_seen: u64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO mempool_txs (txid, fee, vsize, value, first_seen)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![txid, fee, vsize, value, first_seen],
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
        "SELECT txid, fee, vsize, value, first_seen
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
            MAX(rbf_count), MAX(taproot_spend_count),
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
                row.get::<_, Option<u64>>(13)?.unwrap_or(0),  // empty_count
                row.get::<_, Option<u64>>(14)?.unwrap_or(0),  // block_count
            ))
        },
    )?;

    let (max_size, max_fees, max_mfr, max_p90, max_txs, max_ltx, max_in,
         max_out, max_ins, max_run, max_opr, max_rbf, max_tap,
         empty_count, block_count) = row;

    // Pass 2: look up the block that holds each maximum (index-assisted)
    fn lookup_u64(
        conn: &Connection, col: &str, val: u64, from_ts: u64, to_ts: u64,
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
                height: r.get(1)?, timestamp: r.get(2)?, miner: r.get(3)?,
            })
        })
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(ExtremeRecord::default()),
            other => Err(other),
        })
    }

    fn lookup_f64(
        conn: &Connection, col: &str, val: f64, from_ts: u64, to_ts: u64,
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
                height: r.get(1)?, timestamp: r.get(2)?, miner: r.get(3)?,
            })
        })
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(ExtremeRecordF64::default()),
            other => Err(other),
        })
    }

    Ok(ExtremesData {
        largest_block: lookup_u64(conn, "size", max_size, from_ts, to_ts)?,
        highest_fee_block: lookup_u64(conn, "total_fees", max_fees, from_ts, to_ts)?,
        peak_fee_rate: lookup_f64(conn, "median_fee_rate", max_mfr, from_ts, to_ts)?,
        peak_p90_fee_rate: lookup_f64(conn, "fee_rate_p90", max_p90, from_ts, to_ts)?,
        most_txs: lookup_u64(conn, "tx_count", max_txs, from_ts, to_ts)?,
        largest_tx: lookup_u64(conn, "largest_tx_size", max_ltx, from_ts, to_ts)?,
        most_inputs: lookup_u64(conn, "input_count", max_in, from_ts, to_ts)?,
        most_outputs: lookup_u64(conn, "output_count", max_out, from_ts, to_ts)?,
        most_inscriptions: lookup_u64(conn, "inscription_count", max_ins, from_ts, to_ts)?,
        most_runes: lookup_u64(conn, "runes_count", max_run, from_ts, to_ts)?,
        most_op_returns: lookup_u64(conn, "op_return_count", max_opr, from_ts, to_ts)?,
        most_rbf: lookup_u64(conn, "rbf_count", max_rbf, from_ts, to_ts)?,
        most_taproot: lookup_u64(conn, "taproot_spend_count", max_tap, from_ts, to_ts)?,
        empty_block_count: empty_count,
        block_count,
    })
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
        insert_mempool_tx(&conn, "abc123", 500, 200, 1_000_000, 1700000000)
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
        insert_mempool_tx(&conn, "dup_tx", 100, 150, 500_000, 1700000000)
            .unwrap();
        insert_mempool_tx(&conn, "dup_tx", 200, 250, 600_000, 1700000001)
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
        insert_mempool_tx(&conn, "tx1", 100, 150, 500_000, 1700000000).unwrap();
        insert_mempool_tx(&conn, "tx2", 200, 250, 600_000, 1700000001).unwrap();
        insert_mempool_tx(&conn, "tx3", 300, 350, 700_000, 1700000002).unwrap();

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
        insert_mempool_tx(&conn, "tx1", 100, 150, 500_000, 1700000000).unwrap();
        insert_mempool_tx(&conn, "tx2", 200, 250, 600_000, 1700000001).unwrap();
        insert_mempool_tx(&conn, "tx3", 300, 350, 700_000, 1700000002).unwrap();

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
        insert_mempool_tx(&conn, "old_tx", 100, 150, 500_000, 1000).unwrap();
        // Recent tx (timestamp 2000)
        insert_mempool_tx(&conn, "new_tx", 200, 250, 600_000, 2000).unwrap();

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
        insert_mempool_tx(&conn, "tx1", 100, 150, 500_000, 1700000000).unwrap();
        insert_mempool_tx(&conn, "tx2", 200, 250, 600_000, 1700000001).unwrap();
        insert_mempool_tx(&conn, "tx3", 300, 350, 700_000, 1700000002).unwrap();

        // Confirm tx2
        confirm_mempool_txs(&conn, &["tx2".to_string()], 800_000, 1700001000)
            .unwrap();

        let unconfirmed = query_unconfirmed_txids(&conn, 100).unwrap();
        assert_eq!(unconfirmed.len(), 2);
        assert!(unconfirmed.contains(&"tx1".to_string()));
        assert!(unconfirmed.contains(&"tx3".to_string()));
        assert!(!unconfirmed.contains(&"tx2".to_string()));
    }
}
