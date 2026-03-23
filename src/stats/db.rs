//! SQLite database: schema, migrations, and query functions.
//!
//! Schema evolves via ALTER TABLE migrations checked on startup.
//! Genesis block (height 0) is intentionally excluded from backfill
//! since its 50 BTC output is unspendable and has no meaningful fee data.

use std::path::Path;

use rusqlite::{params, Connection};

use super::rpc::Block;

/// Bump this when adding new columns that require re-fetching existing blocks.
/// The backfill loop processes all blocks with backfill_version < BACKFILL_VERSION.
pub const BACKFILL_VERSION: u64 = 2;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;

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
              data_carrier_count, data_carrier_bytes, version, total_fees, miner,
              median_fee, median_fee_rate, coinbase_locktime, coinbase_sequence,
              segwit_spend_count, taproot_spend_count,
              backfill_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
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
            "UPDATE blocks SET version = ?1, total_fees = ?2, miner = ?3, median_fee = ?4, median_fee_rate = ?5, coinbase_locktime = ?6, coinbase_sequence = ?7, segwit_spend_count = ?8, taproot_spend_count = ?9, backfill_version = ?10 WHERE height = ?11",
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
}

pub fn query_blocks(
    conn: &Connection,
    from: u64,
    to: u64,
) -> rusqlite::Result<Vec<BlockRow>> {
    let mut stmt = conn.prepare(
        "SELECT height, hash, timestamp, tx_count, size, weight, difficulty,
                total_fees, median_fee, median_fee_rate,
                segwit_spend_count, taproot_spend_count
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
                data_carrier_count, data_carrier_bytes, version, total_fees, miner,
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
                version: row.get::<_, u32>(13)?,
                total_fees: row.get(14)?,
                miner: row.get(15)?,
                median_fee: row.get(16)?,
                median_fee_rate: row.get(17)?,
                coinbase_locktime: row.get(18)?,
                coinbase_sequence: row.get(19)?,
                segwit_spend_count: row.get(20)?,
                taproot_spend_count: row.get(21)?,
            }))
        },
    )
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
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
                runes_count, runes_bytes, data_carrier_count, data_carrier_bytes
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
            data_carrier_count: row.get(8)?,
            data_carrier_bytes: row.get(9)?,
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
    pub total_data_carrier_count: u64,
    pub total_op_return_bytes: u64,
    pub total_runes_bytes: u64,
    pub total_data_carrier_bytes: u64,
    pub total_fees: u64,
    pub avg_segwit_spend_count: f64,
    pub avg_taproot_spend_count: f64,
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
                SUM(op_return_count), SUM(runes_count), SUM(data_carrier_count),
                SUM(op_return_bytes), SUM(runes_bytes), SUM(data_carrier_bytes),
                SUM(total_fees),
                AVG(segwit_spend_count), AVG(taproot_spend_count)
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
            total_data_carrier_count: row.get(8)?,
            total_op_return_bytes: row.get(9)?,
            total_runes_bytes: row.get(10)?,
            total_data_carrier_bytes: row.get(11)?,
            total_fees: row.get(12)?,
            avg_segwit_spend_count: row.get(13)?,
            avg_taproot_spend_count: row.get(14)?,
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

#[derive(serde::Serialize)]
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
    conn.query_row(
        "SELECT COUNT(*), COALESCE(MIN(height),0), COALESCE(MAX(height),0), COALESCE(MAX(timestamp),0) FROM blocks",
        [],
        |row| {
            let count: u64 = row.get(0)?;
            if count == 0 {
                return Ok(None);
            }
            Ok(Some(Stats {
                block_count: count,
                min_height: row.get(1)?,
                max_height: row.get(2)?,
                latest_timestamp: row.get(3)?,
            }))
        },
    )
}
