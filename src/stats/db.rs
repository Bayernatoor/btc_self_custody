//! SQLite database: schema, migrations, and query functions.
//!
//! Schema evolves via ALTER TABLE migrations checked on startup.
//! Genesis block (height 0) is intentionally excluded from backfill
//! since its 50 BTC output is unspendable and has no meaningful fee data.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use super::rpc::Block;

/// Bump this when adding new columns that require re-fetching existing blocks.
/// The backfill loop processes all blocks with backfill_version < BACKFILL_VERSION.
pub const BACKFILL_VERSION: u64 = 7;

/// Type alias for the connection pool used throughout the stats module.
pub type DbPool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

/// Create a connection pool with WAL mode and proper initialization.
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

    // Ensure indexes exist (safe to run every startup)
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_blocks_backfill ON blocks(backfill_version);",
    )?;

    Ok(())
}

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    init_schema(&conn)?;
    Ok(conn)
}

pub fn max_height(conn: &Connection) -> rusqlite::Result<Option<u64>> {
    conn.query_row("SELECT MAX(height) FROM blocks", [], |row| {
        row.get::<_, Option<u64>>(0)
    })
}

pub fn min_height(conn: &Connection) -> rusqlite::Result<Option<u64>> {
    conn.query_row("SELECT MIN(height) FROM blocks", [], |row| {
        row.get::<_, Option<u64>>(0)
    })
}

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
              backfill_version)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,?36,?37,?38,?39,?40,?41,?42,?43,?44)",
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

/// Get heights of blocks needing backfill
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

/// Update version, total_fees, miner for existing blocks and mark as backfilled
pub fn update_block_extras(
    conn: &Connection,
    blocks: &[Block],
) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    {
        let mut stmt = tx.prepare_cached(
            "UPDATE blocks SET version = ?1, total_fees = ?2, miner = ?3, median_fee = ?4, median_fee_rate = ?5, coinbase_locktime = ?6, coinbase_sequence = ?7, segwit_spend_count = ?8, taproot_spend_count = ?9, omni_count = ?10, omni_bytes = ?11, counterparty_count = ?12, counterparty_bytes = ?13, runes_count = ?14, runes_bytes = ?15, data_carrier_count = ?16, data_carrier_bytes = ?17, p2pk_count = ?18, p2pkh_count = ?19, p2sh_count = ?20, p2wpkh_count = ?21, p2wsh_count = ?22, p2tr_count = ?23, multisig_count = ?24, unknown_script_count = ?25, input_count = ?26, output_count = ?27, rbf_count = ?28, witness_bytes = ?29, inscription_count = ?30, inscription_bytes = ?31, brc20_count = ?32, taproot_keypath_count = ?33, taproot_scriptpath_count = ?34, backfill_version = ?35 WHERE height = ?36",
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
                BACKFILL_VERSION,
                block.height
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}

// === Query types ===

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
}

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
                taproot_keypath_count, taproot_scriptpath_count
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
        })
    })?;
    rows.collect()
}

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
}

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
                AVG(taproot_keypath_count), AVG(taproot_scriptpath_count)
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
        })
    })?;
    rows.collect()
}

#[derive(serde::Serialize)]
pub struct SignalingBlock {
    pub height: u64,
    pub timestamp: u64,
    pub signaled: bool,
    pub miner: String,
}

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

#[derive(Clone, serde::Serialize)]
pub struct SignalingPeriod {
    pub period: u64,
    pub start_height: u64,
    pub end_height: u64,
    pub signaled_count: u64,
    pub total_blocks: u64,
    pub signaled_pct: f64,
}

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

/// Daily miner dominance
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
