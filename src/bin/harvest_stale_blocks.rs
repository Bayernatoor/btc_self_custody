//! Record the orphaned blocks the node still remembers.
//!
//!   BITCOIN_STATS_DB_PATH=./bitcoin_stats.db \
//!   BITCOIN_STATS_RPC_URL=... BITCOIN_STATS_RPC_USER=... \
//!   BITCOIN_STATS_RPC_PASSWORD=... \
//!   cargo run --release --features ssr --bin harvest_stale_blocks
//!
//! Purely additive: it writes only to `stale_blocks`, never to `blocks`, and
//! is idempotent on hash, so running it twice changes nothing.
//!
//! **Why it exists, and why it is worth running before you need it.**
//! `getchaintips` reports the forks a node currently tracks, and that set
//! shrinks when the node is pruned or rebuilt. `docs/node-rebuild.md` exists,
//! so that has happened here before. These blocks were mined, propagated and
//! briefly counted; once the node forgets them they cannot be recovered from
//! the chain, because the chain is precisely what excluded them.
//!
//! Measured 2026-10-18: 30 non-active tips, of which 12 `valid-fork` and 16
//! `valid-headers` answer `getblock`, and 2 `headers-only` do not. The last
//! two are still recorded, because knowing a fork existed at that height is
//! itself unreconstructable.
//!
//! What this is NOT: a history of orphans. It is a sample of what one node
//! retained. Anything before its oldest tip, and anything it never saw, is
//! gone. Copy built on this must say "observed by this node".

use we_hodl_btc::stats::{config::StatsConfig, db, rpc::BitcoinRpc};

#[tokio::main]
async fn main() {
    let config = match StatsConfig::load() {
        Some(c) => c,
        None => {
            eprintln!("BITCOIN_STATS_RPC_URL is not set");
            std::process::exit(1);
        }
    };
    let rpc = BitcoinRpc::new(
        config.rpc_url.clone(),
        config.rpc_user.clone(),
        config.rpc_password.clone(),
    );
    let pool = db::open_pool(&config.db_path, 2).expect("open database");
    let conn = pool.get().expect("connection");
    db::init_stale_blocks_table(&conn).expect("create stale_blocks");

    let tips = match rpc.get_chain_tips().await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("getchaintips failed: {e}");
            std::process::exit(1);
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let (mut stored, mut without_data, mut failed) = (0u64, 0u64, 0u64);

    for tip in &tips {
        let status = tip["status"].as_str().unwrap_or("");
        if status == "active" {
            continue;
        }
        let hash = tip["hash"].as_str().unwrap_or("").to_string();
        let height = tip["height"].as_u64().unwrap_or(0);
        let branch_len = tip["branchlen"].as_u64().unwrap_or(0);

        // The block that won at this height, read from our own `blocks` rather
        // than the node, so it names what the site actually shows as canonical.
        let canonical: Option<String> = conn
            .query_row(
                "SELECT hash FROM blocks WHERE height = ?1",
                rusqlite::params![height],
                |r| r.get(0),
            )
            .ok();

        // `get_block_fresh` parses verbosity 2, so it hands back miner and
        // fees already computed. A `headers-only` tip fails here, which is the
        // expected case for two of them and not an error worth stopping on.
        let detail = rpc.get_block_fresh(&hash).await.ok();
        if detail.is_none() {
            without_data += 1;
        }

        let miner = detail.as_ref().map(|b| b.miner.clone());
        match db::insert_stale_block(
            &conn,
            &hash,
            height,
            canonical.as_deref(),
            detail.as_ref().map(|b| b.time),
            detail.as_ref().map(|b| b.n_tx),
            detail.as_ref().map(|b| b.size),
            detail.as_ref().map(|b| b.weight),
            miner.as_deref(),
            detail.as_ref().map(|b| b.total_fees),
            branch_len,
            status,
            now,
            "harvest",
        ) {
            Ok(_) => stored += 1,
            Err(e) => {
                failed += 1;
                eprintln!("  {hash} at {height}: {e}");
            }
        }
    }

    let total: u64 = conn
        .query_row("SELECT COUNT(*) FROM stale_blocks", [], |r| r.get(0))
        .unwrap_or(0);
    println!(
        "stale blocks: {stored} tips recorded ({without_data} header-only, \
         {failed} failed); table now holds {total}"
    );
}
