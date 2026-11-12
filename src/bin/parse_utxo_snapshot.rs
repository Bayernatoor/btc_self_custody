//! Aggregate a `dumptxoutset` snapshot into the handful of numbers the site shows.
//!
//!   cargo run --release --features ssr --bin parse_utxo_snapshot -- <file> [--json]
//!
//! Reads the snapshot once, streaming, and prints totals by script type plus
//! the exposed-by-construction breakdown by the year each output was created.
//! Nothing is written to the database: the output is reference data, refreshed
//! a few times a year, and belongs in the repo rather than in a sync path
//! between thunder and the droplet.
//!
//! **This answers a stock question, which no other tool here can.** Every
//! chart on the site is a flow derived from blocks: outputs created, never
//! outputs still unspent. 371.7 million P2TR outputs have been created and
//! 52.9 million are unspent, and conflating the two is the easiest way to
//! publish a wrong number on this subject.
//!
//! Mapping a creation height to a year needs the block timestamps, so this
//! reads `bitcoin_stats.db` for that and nothing else. Heights above the local
//! tip are reported separately rather than guessed at.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;

use we_hodl_btc::stats::utxo_snapshot::parse;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        eprintln!("usage: parse_utxo_snapshot <snapshot.dat> [--json]");
        std::process::exit(2);
    };
    let as_json = args.iter().any(|a| a == "--json");

    // Height to year, from our own blocks. A snapshot carries creation
    // heights; a reader thinks in years.
    let db_path = std::env::var("BITCOIN_STATS_DB_PATH")
        .unwrap_or_else(|_| "bitcoin_stats.db".into());
    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap_or_else(|e| panic!("open {db_path}: {e}"));

    // One row per 10,000 blocks is enough to place a height in a year, and
    // avoids holding a million-entry map.
    let mut year_of: Vec<(u32, u16)> = conn
        .prepare(
            "SELECT (height/10000)*10000,
                    CAST(strftime('%Y', datetime(MIN(timestamp),'unixepoch')) AS INTEGER)
             FROM blocks GROUP BY height/10000 ORDER BY 1",
        )
        .and_then(|mut s| {
            s.query_map([], |r| Ok((r.get::<_, i64>(0)? as u32, r.get::<_, i64>(1)? as u16)))
                .and_then(|rows| rows.collect())
        })
        .unwrap_or_else(|e| panic!("read heights: {e}"));
    year_of.sort_unstable();
    let year_for = |h: u32| -> Option<u16> {
        match year_of.binary_search_by_key(&h, |(k, _)| *k) {
            Ok(i) => Some(year_of[i].1),
            Err(0) => None,
            Err(i) => Some(year_of[i - 1].1),
        }
    };

    let file = File::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
    let reader = BufReader::with_capacity(1 << 22, file);

    let mut by_kind: BTreeMap<&'static str, (u64, u128)> = BTreeMap::new();
    let mut exposed_by_year: BTreeMap<u16, (u64, u128)> = BTreeMap::new();
    let mut exposed_unknown_year: (u64, u128) = (0, 0);
    let mut total_coins: u64 = 0;
    let mut total_sats: u128 = 0;

    let started = std::time::Instant::now();
    let header = parse(reader, |c| {
        let e = by_kind.entry(c.kind.label()).or_default();
        e.0 += 1;
        e.1 += u128::from(c.value_sats);
        total_coins += 1;
        total_sats += u128::from(c.value_sats);
        if c.kind.is_exposed() {
            match year_for(c.height) {
                Some(y) => {
                    let e = exposed_by_year.entry(y).or_default();
                    e.0 += 1;
                    e.1 += u128::from(c.value_sats);
                }
                None => {
                    exposed_unknown_year.0 += 1;
                    exposed_unknown_year.1 += u128::from(c.value_sats);
                }
            }
        }
    })
    .unwrap_or_else(|e| panic!("parse {path}: {e}"));

    let btc = |s: u128| -> f64 { s as f64 / 1e8 };
    let exposed_coins: u64 = by_kind
        .iter()
        .filter(|(k, _)| is_exposed_label(k))
        .map(|(_, v)| v.0)
        .sum();
    let exposed_sats: u128 = by_kind
        .iter()
        .filter(|(k, _)| is_exposed_label(k))
        .map(|(_, v)| v.1)
        .sum();

    if as_json {
        println!("{{");
        println!(
            "  \"base_height\": {},",
            header_height(&conn, &header.base_hash_hex())
        );
        println!("  \"base_hash\": \"{}\",", header.base_hash_hex());
        println!("  \"coins_count\": {total_coins},");
        println!("  \"value_sats\": {total_sats},");
        println!("  \"by_kind\": {{");
        let n = by_kind.len();
        for (i, (k, (c, v))) in by_kind.iter().enumerate() {
            let comma = if i + 1 < n { "," } else { "" };
            println!("    \"{k}\": {{ \"utxos\": {c}, \"sats\": {v} }}{comma}");
        }
        println!("  }},");
        println!("  \"exposed_by_year\": {{");
        let n = exposed_by_year.len();
        for (i, (y, (c, v))) in exposed_by_year.iter().enumerate() {
            let comma = if i + 1 < n { "," } else { "" };
            println!("    \"{y}\": {{ \"utxos\": {c}, \"sats\": {v} }}{comma}");
        }
        println!("  }}");
        println!("}}");
        return;
    }

    println!("snapshot      {path}");
    println!("base_hash     {}", header.base_hash_hex());
    println!("coins_count   {} (header)", header.coins_count);
    println!(
        "parsed in     {:.1} min\n",
        started.elapsed().as_secs_f64() / 60.0
    );

    println!(
        "{:<16}{:>14}{:>8}   {:>18}{:>8}",
        "TYPE", "UTXOs", "share", "BTC", "share"
    );
    let mut rows: Vec<_> = by_kind.iter().collect();
    rows.sort_by_key(|(_, (c, _))| std::cmp::Reverse(*c));
    for (k, (c, v)) in rows {
        println!(
            "{k:<16}{c:>14}{:>7.2}%   {:>18.2}{:>7.2}%",
            100.0 * *c as f64 / total_coins as f64,
            btc(*v),
            100.0 * *v as f64 / total_sats as f64
        );
    }
    println!(
        "{:<16}{:>14}{:>7.2}%   {:>18.2}{:>7.2}%",
        "TOTAL",
        total_coins,
        100.0,
        btc(total_sats),
        100.0
    );

    println!(
        "\nEXPOSED BY CONSTRUCTION  {exposed_coins} UTXOs ({:.2}%)   {:.2} BTC ({:.2}%)",
        100.0 * exposed_coins as f64 / total_coins as f64,
        btc(exposed_sats),
        100.0 * exposed_sats as f64 / total_sats as f64
    );
    println!("\n{:<8}{:>14}{:>18}{:>10}", "YEAR", "UTXOs", "BTC", "cum %");
    let mut cum: u128 = 0;
    for (y, (c, v)) in &exposed_by_year {
        cum += v;
        println!(
            "{y:<8}{c:>14}{:>18.2}{:>9.2}%",
            btc(*v),
            100.0 * cum as f64 / exposed_sats as f64
        );
    }
    if exposed_unknown_year.0 > 0 {
        println!(
            "{:<8}{:>14}{:>18.2}   (created above the local tip)",
            "?",
            exposed_unknown_year.0,
            btc(exposed_unknown_year.1)
        );
    }

    println!("\nORACLES");
    println!(
        "  coins  {total_coins} vs header {}  {}",
        header.coins_count,
        if total_coins == header.coins_count {
            "MATCH"
        } else {
            "MISMATCH"
        }
    );
    println!(
        "  value  {:.8} BTC     compare against: gettxoutsetinfo none <base_height>",
        btc(total_sats)
    );
}

fn is_exposed_label(l: &str) -> bool {
    matches!(l, "P2PK" | "bare multisig" | "P2TR")
}

/// The height of the snapshot's base block, looked up in our own chain so the
/// output records it without the caller passing it in.
fn header_height(conn: &rusqlite::Connection, hash_hex: &str) -> i64 {
    conn.query_row(
        "SELECT height FROM blocks WHERE hash = ?1",
        rusqlite::params![hash_hex],
        |r| r.get(0),
    )
    .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    /// The labels the aggregator treats as exposed must be exactly the labels
    /// `ScriptKind::is_exposed` reports, or the summary and the breakdown
    /// disagree about what the headline figure covers.
    #[test]
    fn the_exposed_label_list_matches_the_type() {
        use we_hodl_btc::stats::utxo_snapshot::ScriptKind::*;
        for k in [
            P2PK,
            P2PKH,
            P2SH,
            P2WPKH,
            P2WSH,
            P2TR,
            BareMultisig,
            P2A,
            OtherWitness,
            Other,
        ] {
            assert_eq!(
                super::is_exposed_label(k.label()),
                k.is_exposed(),
                "{} disagrees between the label list and the type",
                k.label()
            );
        }
    }
}
