//! Integration tests for the `/api/stats/*` REST endpoints.
//!
//! These tests drive the same Axum router used in production (via
//! [`super::startup::build_api_router`]) against a per-test tempfile SQLite
//! DB and a stub RPC client. No real HTTP server is bound; requests flow
//! directly into the router via `tower::ServiceExt::oneshot`.
//!
//! Design choices:
//! - **Inline module, not `tests/*.rs`**: a separate integration-test binary
//!   would require relinking the full dependency closure (leptos, axum,
//!   reqwest, bundled sqlite, zeromq, …), which peaks at several GB during
//!   linking and reliably OOMed on the development machine. Living inside
//!   the lib folds these into the existing unit-test binary.
//! - **In-process router via `oneshot`**: faster and more hermetic than a
//!   bound TCP listener; no port contention between tests.
//! - **Tempfile SQLite per test**: each `TestApp` gets an isolated on-disk
//!   DB in its own `TempDir`. WAL mode + schema run normally. Cleanup is
//!   automatic on `Drop`.
//! - **Stub `BitcoinRpc`** pointing at a closed loopback port: RPC-backed
//!   endpoints exercise the connection-refused path deterministically,
//!   DB-backed endpoints are unaffected.

use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use tempfile::TempDir;
use tokio::sync::broadcast;
use tower::ServiceExt;

use super::api::{StatsState, StatsStateBuilder};
use super::cache::CacheTag;
use super::db::{self, DbPool};
use super::rpc::BitcoinRpc;
use super::startup::build_api_router;
use std::time::Duration;

struct TestApp {
    router: Router,
    state: Arc<StatsState>,
    // Held to keep the tempdir alive for the life of the test.
    _tempdir: TempDir,
}

impl TestApp {
    fn new() -> Self {
        let tempdir = tempfile::tempdir().expect("create tempdir");
        let db_path = tempdir.path().join("test.db");

        let pool: DbPool =
            db::open_pool(&db_path, 4).expect("open test DB pool");

        // URL intentionally points at a closed port. Tests that invoke RPC
        // will get a connection-refused error and can assert on that path.
        let rpc = BitcoinRpc::new(
            "http://127.0.0.1:1/".to_string(),
            "test".to_string(),
            "test".to_string(),
        );

        let (heartbeat_tx, _) = broadcast::channel(64);

        let mut cb = StatsStateBuilder::new();
        let stats_summary_cache = cb
            .cache::<(), super::types::StatsSummary>(
                Duration::from_secs(60),
                &[CacheTag::OnNewBlock],
            );
        let block_ts_cache =
            cb.cache::<u64, u64>(Duration::MAX, &[]);

        let state = Arc::new(StatsState {
            db: pool,
            rpc,
            cache_registry: cb.into_registry(),
            price_cache: Mutex::new(None),
            price_refreshing: AtomicBool::new(false),
            utxo_count: Mutex::new(None),
            stats_summary_cache,
            daily_cache: Mutex::new(None),
            block_ts_cache,
            signaling_blocks_cache: Mutex::new(None),
            signaling_periods_cache: Mutex::new(None),
            price_history_cache: Mutex::new(None),
            range_summary_cache: Mutex::new(None),
            extremes_cache: Mutex::new(None),
            heartbeat_tx,
            sse_connections: AtomicUsize::new(0),
        });

        let router = build_api_router(Arc::clone(&state));

        Self {
            router,
            state,
            _tempdir: tempdir,
        }
    }

    /// Fire a GET request against the router and parse the JSON response.
    /// Returns `(status, parsed body)`. Panics on non-JSON responses.
    async fn get(&self, uri: &str) -> (StatusCode, serde_json::Value) {
        let req = Request::builder()
            .uri(uri)
            .body(Body::empty())
            .expect("build request");
        let response = self
            .router
            .clone()
            .oneshot(req)
            .await
            .expect("oneshot succeeded");
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body bytes");
        let body: serde_json::Value = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("response was valid JSON")
        };
        (status, body)
    }
}

/// Insert a synthetic block row so DB-backed endpoints have something to
/// return. Fields default to zeroes; tests that care about specific values
/// should query and assert on what they set explicitly.
fn seed_block(app: &TestApp, height: u64, hash: &str, time: u64) {
    let conn = app.state.db.get().expect("DB connection");
    use super::rpc::Block;
    let block = Block {
        hash: hash.to_string(),
        height,
        time,
        n_tx: 1,
        size: 1000,
        weight: 4000,
        difficulty: 1.0,
        op_return_count: 0,
        op_return_bytes: 0,
        runes_count: 0,
        runes_bytes: 0,
        omni_count: 0,
        omni_bytes: 0,
        counterparty_count: 0,
        counterparty_bytes: 0,
        data_carrier_count: 0,
        data_carrier_bytes: 0,
        version: 0x20000000,
        total_fees: 0,
        median_fee: 0,
        median_fee_rate: 0.0,
        coinbase_locktime: 0,
        coinbase_sequence: 0xffffffff,
        miner: "TestMiner".to_string(),
        segwit_spend_count: 0,
        taproot_spend_count: 0,
        p2pk_count: 0,
        p2pkh_count: 0,
        p2sh_count: 0,
        p2wpkh_count: 0,
        p2wsh_count: 0,
        p2tr_count: 0,
        multisig_count: 0,
        unknown_script_count: 0,
        taproot_keypath_count: 0,
        taproot_scriptpath_count: 0,
        input_count: 0,
        output_count: 1,
        rbf_count: 0,
        witness_bytes: 0,
        inscription_count: 0,
        inscription_bytes: 0,
        inscription_envelope_bytes: 0,
        brc20_count: 0,
        total_output_value: 0,
        total_input_value: 0,
        fee_rate_p10: 0.0,
        fee_rate_p90: 0.0,
        stamps_count: 0,
        largest_tx_size: 0,
        max_tx_fee: 0,
        inscription_fees: 0,
        runes_fees: 0,
        legacy_tx_count: 0,
        segwit_tx_count: 1,
        taproot_tx_count: 0,
        coinbase_text: String::new(),
        fee_rate_p25: 0.0,
        fee_rate_p75: 0.0,
    };
    db::insert_blocks(&conn, &[block]).expect("insert block");
}

// ---------------------------------------------------------------------------
// /stats — database summary
// ---------------------------------------------------------------------------

#[tokio::test]
async fn stats_empty_db_returns_zeros() {
    let app = TestApp::new();
    let (status, body) = app.get("/stats").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["block_count"], 0);
    assert_eq!(body["min_height"], 0);
    assert_eq!(body["max_height"], 0);
}

#[tokio::test]
async fn stats_after_seeding_reflects_inserted_block() {
    let app = TestApp::new();
    seed_block(&app, 800_000, "deadbeef", 1_700_000_000);
    seed_block(&app, 800_001, "cafebabe", 1_700_000_600);

    let (status, body) = app.get("/stats").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["block_count"], 2);
    assert_eq!(body["min_height"], 800_000);
    assert_eq!(body["max_height"], 800_001);
    assert_eq!(body["latest_timestamp"], 1_700_000_600);
}

// ---------------------------------------------------------------------------
// /blocks — range query
// ---------------------------------------------------------------------------

#[tokio::test]
async fn blocks_range_query_returns_inclusive_range() {
    let app = TestApp::new();
    for h in 100..=105u64 {
        seed_block(&app, h, &format!("hash{h:08x}"), 1_700_000_000 + h);
    }

    let (status, body) = app.get("/blocks?from=101&to=103").await;
    assert_eq!(status, StatusCode::OK);
    let blocks = body["blocks"].as_array().expect("blocks is array");
    assert_eq!(blocks.len(), 3, "range 101..=103 inclusive = 3 blocks");
    assert_eq!(blocks[0]["height"], 101);
    assert_eq!(blocks[2]["height"], 103);
}

#[tokio::test]
async fn blocks_range_empty_when_no_matches() {
    let app = TestApp::new();
    seed_block(&app, 100, "hash", 1_700_000_000);

    let (status, body) = app.get("/blocks?from=500&to=600").await;
    assert_eq!(status, StatusCode::OK);
    let blocks = body["blocks"].as_array().expect("blocks is array");
    assert!(blocks.is_empty());
}

// ---------------------------------------------------------------------------
// /blocks/:height — single block detail
// ---------------------------------------------------------------------------

#[tokio::test]
async fn block_detail_returns_inserted_block() {
    let app = TestApp::new();
    seed_block(&app, 850_000, "a1b2c3d4", 1_710_000_000);

    let (status, body) = app.get("/blocks/850000").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["height"], 850_000);
    assert_eq!(body["hash"], "a1b2c3d4");
}

#[tokio::test]
async fn block_detail_missing_height_returns_error_shape() {
    let app = TestApp::new();
    let (status, body) = app.get("/blocks/999999").await;
    // Current handler returns 200 with {"error": "Block not found"}.
    // This test locks in that contract — if the response is changed to a
    // proper 404, update the expectations here.
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["error"], "Block not found");
}

// ---------------------------------------------------------------------------
// /cache-stats — RPC cache observability
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cache_stats_cold_start_reports_zero_counters() {
    let app = TestApp::new();
    let (status, body) = app.get("/cache-stats").await;
    assert_eq!(status, StatusCode::OK);

    let slots = body["slots"].as_array().expect("slots is array");
    // The three unconditional TTL slots are always present; estimatesmartfee
    // is lazily created per-target on first call, so it may not appear yet.
    let methods: Vec<_> = slots
        .iter()
        .filter_map(|s| s["method"].as_str())
        .collect();
    assert!(methods.contains(&"getmempoolinfo"));
    assert!(methods.contains(&"getblockchaininfo"));
    assert!(methods.contains(&"getnetworkhashps"));

    for slot in slots {
        assert_eq!(slot["hits"], 0);
        assert_eq!(slot["misses"], 0);
        assert_eq!(slot["errors"], 0);
        assert_eq!(slot["stale_served"], 0);
    }

    let blocks = body["blocks"].as_array().expect("blocks is array");
    let block_methods: Vec<_> = blocks
        .iter()
        .filter_map(|s| s["method"].as_str())
        .collect();
    assert!(block_methods.contains(&"getblockhash"));
    assert!(block_methods.contains(&"getblock"));
    for slot in blocks {
        assert_eq!(slot["hits"], 0);
        assert_eq!(slot["misses"], 0);
        assert_eq!(slot["size"], 0);
        assert!(slot["capacity"].as_u64().unwrap() > 0);
    }
}

// ---------------------------------------------------------------------------
// /live — real-time stats (RPC path, should error against closed port)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn live_without_rpc_surfaces_error() {
    // BitcoinRpc URL is http://127.0.0.1:1/ which refuses connection.
    // get_live fires all 4 RPCs in parallel; the first to error bubbles up,
    // and with no cached value to fall back to on a fresh TestApp, the
    // response must be a server error.
    let app = TestApp::new();
    let (status, _body) = app.get("/live").await;
    assert!(
        status.is_server_error(),
        "expected server error when RPC unreachable, got {status}"
    );
}

// ---------------------------------------------------------------------------
// /signaling/periods — BIP signaling aggregates
// ---------------------------------------------------------------------------

#[tokio::test]
async fn signaling_periods_on_empty_db_returns_empty_shape() {
    let app = TestApp::new();
    let (status, body) = app.get("/signaling/periods?bit=4").await;
    assert_eq!(status, StatusCode::OK);
    // Exact shape varies by bit number; just lock that it returned valid
    // JSON and the handler didn't panic on an empty DB.
    assert!(body.is_object() || body.is_array());
}

// ---------------------------------------------------------------------------
// /aggregates/daily — timestamp range
// ---------------------------------------------------------------------------

#[tokio::test]
async fn daily_aggregates_empty_range_is_ok() {
    let app = TestApp::new();
    let (status, body) = app.get("/aggregates/daily?from=0&to=1").await;
    assert_eq!(status, StatusCode::OK);
    // Locks in that zero-block ranges don't error — historically a
    // regression site when new aggregation columns were added.
    assert!(body.is_object() || body.is_array());
}

// ---------------------------------------------------------------------------
// Cache registry integration: simulated block event invalidates the right
// caches without touching the trigger code (block poller / ZMQ handler).
// This is the contract that makes the registry safe to ship: adding a new
// tagged cache means it gets invalidated automatically; an untagged cache
// is left alone by tag-routed events.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invalidate_on_new_block_clears_migrated_caches_via_registry() {
    use super::types::StatsSummary;

    let app = TestApp::new();

    // Seed the migrated stats_summary_cache directly. Goes through the
    // cache primitive's insert(); no DB query, no fetcher.
    app.state.stats_summary_cache.insert(
        (),
        StatsSummary {
            block_count: 1,
            min_height: 100,
            max_height: 100,
            latest_timestamp: 1_700_000_000,
        },
    );
    assert!(
        app.state.stats_summary_cache.get(&()).is_some(),
        "stats_summary_cache populated"
    );

    // Seed block_ts_cache (no tags) so we can verify it survives.
    app.state.block_ts_cache.insert(800_000, 1_700_000_000);
    assert_eq!(
        app.state.block_ts_cache.get(&800_000),
        Some(1_700_000_000)
    );

    // Fire the same call the block poller makes. This is the entire
    // public surface for invalidation; the test does not import or
    // touch any trigger-side code.
    app.state.invalidate(CacheTag::OnNewBlock);

    // Tagged: cleared.
    assert!(
        app.state.stats_summary_cache.get(&()).is_none(),
        "stats_summary_cache should be cleared by OnNewBlock"
    );
    // Untagged: untouched.
    assert_eq!(
        app.state.block_ts_cache.get(&800_000),
        Some(1_700_000_000),
        "block_ts_cache has no tags and must not be cleared by OnNewBlock"
    );
}

#[tokio::test]
async fn invalidate_on_price_refresh_does_not_touch_stats_cache() {
    use super::types::StatsSummary;

    let app = TestApp::new();
    app.state.stats_summary_cache.insert(
        (),
        StatsSummary {
            block_count: 1,
            min_height: 100,
            max_height: 100,
            latest_timestamp: 1_700_000_000,
        },
    );

    // Different tag: stats_summary_cache subscribes only to OnNewBlock,
    // so OnPriceRefresh must not clear it.
    app.state.invalidate(CacheTag::OnPriceRefresh);

    assert!(
        app.state.stats_summary_cache.get(&()).is_some(),
        "stats_summary_cache must survive a tag it didn't subscribe to"
    );
}
