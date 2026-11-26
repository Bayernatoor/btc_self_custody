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

use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use tempfile::TempDir;
use tokio::sync::broadcast;
use tower::ServiceExt;

use super::api::{StatsState, StatsStateBuilder, MAX_SSE_CONNECTIONS};
use super::cache::CacheTag;
use super::db::{self, DbPool};
use super::heartbeat_state::{HeartbeatStore, CHECKPOINT_RESET_DIFF_THRESHOLD};
use super::heartbeat_sync::{
    ApplyOutcome, CanonicalBlock, Coverage, HeartbeatDelta, HeartbeatMirror,
    HeartbeatSnapshot, NodeMempoolAggregate, ResyncRequired, TxFlags,
    TxLookupStatus, PROTOCOL_VERSION,
};
use super::rpc::BitcoinRpc;
use super::startup::build_api_router;

struct TestApp {
    router: Router,
    state: Arc<StatsState>,
    // Held to keep the tempdir alive for the life of the test.
    _tempdir: TempDir,
}

impl TestApp {
    fn new() -> Self {
        Self::with_heartbeat_store(Arc::new(HeartbeatStore::new()))
    }

    fn with_heartbeat_store(heartbeat_store: Arc<HeartbeatStore>) -> Self {
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

        // Mirror the production cache layer constructed in startup.rs.
        // Same builder, same tags; if the production list grows or shifts,
        // mirror it here to keep tests honest.
        use super::types;
        let mut cb = StatsStateBuilder::new();
        let price_cache = cb.cache::<(), super::rpc::PriceInfo>(
            "price",
            Duration::from_secs(60),
            &[],
        );
        let utxo_count = cb.cache::<(), u64>("utxo_count", Duration::MAX, &[]);
        let stats_summary_cache = cb.cache::<(), types::StatsSummary>(
            "stats_summary",
            Duration::from_secs(60),
            &[CacheTag::OnNewBlock],
        );
        let daily_cache = cb.cache::<(u64, u64), Vec<types::DailyAggregate>>(
            "daily_aggregates",
            Duration::from_secs(120),
            &[CacheTag::OnNewBlock],
        );
        let block_ts_cache =
            cb.cache::<u64, u64>("block_timestamps", Duration::MAX, &[]);
        let time_histogram_cache = cb
            .cache::<(u64, u64), Vec<types::HistogramBucket>>(
                "time_histogram",
                Duration::from_secs(120),
                &[CacheTag::OnNewBlock],
            );
        let fullness_histogram_cache = cb
            .cache::<(u64, u64), Vec<types::HistogramBucket>>(
                "fullness_histogram",
                Duration::from_secs(120),
                &[CacheTag::OnNewBlock],
            );
        let signaling_blocks_cache = cb.cache::<String, (
            Vec<types::SignalingBlock>,
            types::PeriodStats,
        )>(
            "signaling_blocks",
            Duration::from_secs(60),
            &[],
        );
        let signaling_periods_cache = cb
            .cache::<String, Vec<super::db::SignalingPeriod>>(
                "signaling_periods",
                Duration::from_secs(60),
                &[],
            );
        let price_history_cache = cb.cache::<(), Vec<types::PricePoint>>(
            "price_history",
            Duration::from_secs(3600),
            &[],
        );
        let range_summary_cache = cb.cache::<(u64, u64), types::RangeSummary>(
            "range_summary",
            Duration::from_secs(60),
            &[CacheTag::OnNewBlock],
        );
        let extremes_cache = cb.cache::<(u64, u64), types::ExtremesData>(
            "extremes",
            Duration::from_secs(60),
            &[CacheTag::OnNewBlock],
        );
        let state = Arc::new(StatsState {
            db: pool,
            rpc,
            cache_registry: cb.into_registry(),
            price_cache,
            utxo_count,
            stats_summary_cache,
            daily_cache,
            time_histogram_cache,
            fullness_histogram_cache,
            block_ts_cache,
            signaling_blocks_cache,
            signaling_periods_cache,
            price_history_cache,
            range_summary_cache,
            extremes_cache,
            heartbeat_tx,
            heartbeat_store,
            heartbeat_blocks: Default::default(),
            sse_connections: AtomicUsize::new(0),
        });

        let router = build_api_router(Arc::clone(&state));

        Self {
            router,
            state,
            _tempdir: tempdir,
        }
    }

    async fn send(&self, req: Request<Body>) -> axum::response::Response {
        self.router
            .clone()
            .oneshot(req)
            .await
            .expect("oneshot succeeded")
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
        // The fields under test are named above; the rest default to zero,
        // which is meaningful here because every one is a counter or a value.
        ..Default::default()
    };
    db::insert_blocks(&conn, &[block]).expect("insert block");
}

// ---------------------------------------------------------------------------
// /stats: database summary
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
// /blocks: range query
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
// /blocks/:height: single block detail
// ---------------------------------------------------------------------------

#[tokio::test]
async fn block_detail_returns_inserted_block() {
    let app = TestApp::new();
    seed_block(&app, 850_000, "a1b2c3d4", 1_710_000_000);

    let (status, body) = app.get("/blocks/850000").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["height"], 850_000);
    assert_eq!(body["hash"], "a1b2c3d4");
    assert!(body.get("landmarks").is_none());

    // Landmark block (First Halving #210,000) carries curated Archives context.
    seed_block(&app, 210_000, "000000000000048b", 1_354_116_278);
    let (status, body) = app.get("/blocks/210000").await;
    assert_eq!(status, StatusCode::OK);
    let landmarks = body["landmarks"].as_array().expect("landmarks array");
    assert_eq!(landmarks.len(), 1);
    assert_eq!(landmarks[0]["title"], "First Halving");
    assert_eq!(landmarks[0]["category"], "Milestones");
}

#[tokio::test]
async fn block_detail_missing_height_returns_404() {
    let app = TestApp::new();
    let (status, body) = app.get("/blocks/999999").await;
    // Was 200 carrying {"error": "Block not found"}, which is worse than
    // either a 200 or a 404: every caller had to inspect the body to learn the
    // request failed. The previous version of this test locked in that shape
    // and invited exactly this change.
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "Block not found");
}

// ---------------------------------------------------------------------------
// /cache-stats: RPC cache observability
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cache_stats_cold_start_reports_zero_counters() {
    let app = TestApp::new();
    let (status, body) = app.get("/cache-stats").await;
    assert_eq!(status, StatusCode::OK);

    let slots = body["slots"].as_array().expect("slots is array");
    // The three unconditional TTL slots are always present; estimatesmartfee
    // is lazily created per-target on first call, so it may not appear yet.
    let methods: Vec<_> =
        slots.iter().filter_map(|s| s["method"].as_str()).collect();
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
    let block_methods: Vec<_> =
        blocks.iter().filter_map(|s| s["method"].as_str()).collect();
    assert!(block_methods.contains(&"getblockhash"));
    assert!(block_methods.contains(&"getblock"));
    for slot in blocks {
        assert_eq!(slot["hits"], 0);
        assert_eq!(slot["misses"], 0);
        assert_eq!(slot["size"], 0);
        assert!(slot["capacity"].as_u64().unwrap() > 0);
    }

    // Application caches: every Cache<K, V> built through the builder
    // shows up by name with zeroed counters until something exercises
    // it. Keeping this list in sync with the production builder is the
    // contract that catches "added a cache, forgot to surface it" bugs.
    let application = body["application"]
        .as_array()
        .expect("application is array");
    let names: Vec<_> = application
        .iter()
        .filter_map(|c| c["name"].as_str())
        .collect();
    for expected in [
        "price",
        "utxo_count",
        "stats_summary",
        "daily_aggregates",
        "block_timestamps",
        "signaling_blocks",
        "signaling_periods",
        "price_history",
        "range_summary",
        "extremes",
    ] {
        assert!(
            names.contains(&expected),
            "missing application cache `{expected}` in /cache-stats"
        );
    }
    for c in application {
        assert_eq!(c["hits"], 0);
        assert_eq!(c["misses"], 0);
        assert_eq!(c["size"], 0);
    }
}

// ---------------------------------------------------------------------------
// /live: real-time stats (RPC path, should error against closed port)
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
// /signaling/periods: BIP signaling aggregates
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

/// An out-of-range version bit must be a 400, on both signaling endpoints.
///
/// This is the bug that made the whole probing exercise worth it.
/// `/signaling?bit=99` returned no response at all: the mask is built as
/// `1i64 << bit` from an unvalidated query parameter, and a shift past the word
/// width panics in a debug build, killing the connection. A release build is
/// arguably worse, masking the shift to `bit % 64` so `bit=99` answers as if it
/// were bit 35 and looks like data. Nothing here asserted status codes, so the
/// suite could not see either outcome.
#[tokio::test]
async fn out_of_range_version_bit_is_a_client_error() {
    let app = TestApp::new();
    for uri in [
        "/signaling?bit=99&from=0&to=100",
        "/signaling/periods?bit=99",
        "/signaling?bit=29&from=0&to=100",
        "/signaling/periods?bit=64",
    ] {
        let (status, _) = app.get(uri).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "{uri} should be rejected as a client error"
        );
    }
}

/// The highest signalable bit must still be accepted, so the guard rejects
/// only what BIP9 cannot express rather than trimming the valid range.
#[tokio::test]
async fn highest_valid_version_bit_is_accepted() {
    let app = TestApp::new();
    let (status, _) = app.get("/signaling/periods?bit=28").await;
    assert_eq!(status, StatusCode::OK);
}

// ---------------------------------------------------------------------------
// /aggregates/daily: timestamp range
// ---------------------------------------------------------------------------

#[tokio::test]
async fn over_wide_range_is_a_client_error_not_a_server_fault() {
    // A caller asking for too much has not broken the server. Returning 500
    // misattributes it, logs at error! where it buries real faults, and tells
    // the caller nothing about how to fix the request.
    let app = TestApp::new();
    let (status, body) = app.get("/op-returns?from=0&to=900000").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("too large"),
        "the message must tell the caller what to change: {body}"
    );

    // A span inside the cap is served normally.
    let (status, _) = app.get("/op-returns?from=0&to=100").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn daily_aggregates_empty_range_is_ok() {
    let app = TestApp::new();
    let (status, body) = app.get("/aggregates/daily?from=0&to=1").await;
    assert_eq!(status, StatusCode::OK);
    // Locks in that zero-block ranges don't error, historically a
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
    assert_eq!(app.state.block_ts_cache.get(&800_000), Some(1_700_000_000));

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

// ---------------------------------------------------------------------------
// Heartbeat Phase 2: /heartbeat/snapshot, /heartbeat/v2, /heartbeat/tx/{txid}
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct SseFrame {
    event: String,
    id: Option<String>,
    data: String,
}

struct SseReader {
    stream: axum::body::BodyDataStream,
    buf: String,
}

impl SseReader {
    fn from_response(response: axum::response::Response) -> Self {
        Self {
            stream: response.into_body().into_data_stream(),
            buf: String::new(),
        }
    }

    async fn next_event(&mut self) -> Option<SseFrame> {
        use futures::StreamExt;
        loop {
            if let Some(pos) = self.buf.find("\n\n") {
                let raw = self.buf[..pos].to_string();
                self.buf.drain(..pos + 2);
                if let Some(frame) = parse_sse_frame(&raw) {
                    return Some(frame);
                }
                continue;
            }
            let chunk = tokio::time::timeout(
                Duration::from_secs(2),
                self.stream.next(),
            )
            .await
            .expect("timed out waiting for SSE chunk")?;
            let bytes = chunk.expect("SSE body chunk ok");
            self.buf.push_str(
                std::str::from_utf8(&bytes).expect("SSE payload is UTF-8"),
            );
        }
    }
}

fn parse_sse_frame(raw: &str) -> Option<SseFrame> {
    let mut event = String::new();
    let mut id = None;
    let mut data_lines = Vec::new();
    for line in raw.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("event:") {
            event = rest.trim_start().to_string();
        } else if let Some(rest) = line.strip_prefix("id:") {
            id = Some(rest.trim_start().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim_start().to_string());
        }
    }
    if event.is_empty() && data_lines.is_empty() {
        return None;
    }
    Some(SseFrame {
        event,
        id,
        data: data_lines.join("\n"),
    })
}

fn test_hex(seed: u64) -> String {
    format!("{seed:064x}")
}

async fn fetch_snapshot(app: &TestApp) -> HeartbeatSnapshot {
    let req = Request::builder()
        .uri("/heartbeat/snapshot")
        .body(Body::empty())
        .expect("build request");
    let resp = app.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read snapshot body");
    serde_json::from_slice(&bytes).expect("valid HeartbeatSnapshot JSON")
}

#[tokio::test]
async fn heartbeat_snapshot_returns_200_with_epoch_and_seq_headers() {
    let app = TestApp::new();
    let tx1 = test_hex(1);
    app.state
        .heartbeat_store
        .install_checkpoint(&[&tx1], true, 1_700_000_000);

    let req = Request::builder()
        .uri("/heartbeat/snapshot")
        .body(Body::empty())
        .expect("build request");
    let resp = app.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let headers = resp.headers().clone();
    assert_eq!(
        headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/json")
    );
    assert_eq!(
        headers
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-store")
    );
    let epoch_hdr = headers
        .get("x-heartbeat-epoch")
        .and_then(|v| v.to_str().ok())
        .expect("X-Heartbeat-Epoch header present");
    let seq_hdr = headers
        .get("x-heartbeat-seq")
        .and_then(|v| v.to_str().ok())
        .expect("X-Heartbeat-Seq header present");

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("read body");
    let snap: HeartbeatSnapshot =
        serde_json::from_slice(&bytes).expect("valid HeartbeatSnapshot");
    assert_eq!(snap.protocol_version, PROTOCOL_VERSION);
    assert_eq!(snap.stream_epoch, epoch_hdr);
    assert_eq!(snap.seq.to_string(), seq_hdr);
    assert_eq!(snap.coverage, Coverage::Complete);
    assert_eq!(snap.transactions.len(), 1);
    assert_eq!(snap.transactions[0].txid, tx1);
}

#[tokio::test]
async fn heartbeat_v2_rejects_missing_and_malformed_cursors_with_400() {
    let app = TestApp::new();

    for uri in [
        "/heartbeat/v2",
        "/heartbeat/v2?epoch=abc",
        "/heartbeat/v2?after=0",
        "/heartbeat/v2?epoch=&after=0",
        "/heartbeat/v2?epoch=abc&after=notanumber",
        "/heartbeat/v2?epoch=abc&after=-1",
    ] {
        let req = Request::builder()
            .uri(uri)
            .body(Body::empty())
            .expect("build request");
        let resp = app.send(req).await;
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "{uri} must return 400 Bad Request"
        );
    }

    for bad_header in ["no-colon", ":10", "epoch:abc", "epoch:-5"] {
        let req = Request::builder()
            .uri("/heartbeat/v2")
            .header("Last-Event-ID", bad_header)
            .body(Body::empty())
            .expect("build request");
        let resp = app.send(req).await;
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "Last-Event-ID={bad_header} must return 400 Bad Request"
        );
    }
}

#[tokio::test]
async fn heartbeat_v2_enforces_sse_limit_with_503_and_releases_guard_on_drop() {
    use std::sync::atomic::Ordering;

    let app = TestApp::new();
    let (epoch, seq) = app.state.heartbeat_store.current_revision();
    let uri = format!("/heartbeat/v2?epoch={epoch}&after={seq}");

    // Saturate the global SSE counter.
    app.state
        .sse_connections
        .store(MAX_SSE_CONNECTIONS, Ordering::Relaxed);
    let req = Request::builder()
        .uri(&uri)
        .body(Body::empty())
        .expect("build request");
    let resp = app.send(req).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        app.state.sse_connections.load(Ordering::Relaxed),
        MAX_SSE_CONNECTIONS,
        "rejected 503 must not leak SSE connection counter"
    );

    // Open a valid stream and verify headers + permit release on drop.
    app.state.sse_connections.store(0, Ordering::Relaxed);
    let req = Request::builder()
        .uri(&uri)
        .body(Body::empty())
        .expect("build request");
    let resp = app.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.contains("text/event-stream"),
        "expected text/event-stream, got {content_type}"
    );
    assert_eq!(
        resp.headers()
            .get("x-accel-buffering")
            .and_then(|v| v.to_str().ok()),
        Some("no")
    );
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-cache")
    );
    assert_eq!(app.state.sse_connections.load(Ordering::Relaxed), 1);

    drop(resp);
    assert_eq!(
        app.state.sse_connections.load(Ordering::Relaxed),
        0,
        "dropping the SSE stream must release the connection permit"
    );
}

#[tokio::test]
async fn heartbeat_tx_lookup_validates_txid_and_reports_all_statuses() {
    let app = TestApp::new();
    let tx_unconfirmed = test_hex(0x11);
    let tx_confirmed = test_hex(0x22);
    let tx_absent = test_hex(0x33);
    let block_hash = test_hex(0xbb);

    // Invalid txids return 400.
    for bad in ["short", &"z".repeat(64)] {
        let req = Request::builder()
            .uri(format!("/heartbeat/tx/{bad}"))
            .body(Body::empty())
            .expect("build request");
        let resp = app.send(req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // Seed mempool and enrich tx_unconfirmed.
    app.state.heartbeat_store.install_checkpoint(
        &[&tx_unconfirmed, &tx_confirmed],
        true,
        1_700_000_000,
    );
    app.state.heartbeat_store.enrich_tx(
        tx_unconfirmed.clone(),
        1_500,
        141,
        50_000,
        TxFlags::OP_RETURN_MSG,
        Some("hodl".to_string()),
    );

    // Uppercase hex lookup still resolves to unconfirmed tx.
    let (status, body) = app
        .get(&format!(
            "/heartbeat/tx/{}",
            tx_unconfirmed.to_ascii_uppercase()
        ))
        .await;
    assert_eq!(status, StatusCode::OK);
    let parsed: TxLookupStatus =
        serde_json::from_value(body).expect("parse TxLookupStatus");
    match parsed {
        TxLookupStatus::Unconfirmed(tx) => {
            assert_eq!(tx.txid, tx_unconfirmed);
            assert_eq!(tx.fee, Some(1_500));
            assert_eq!(tx.vsize, Some(141));
            assert_eq!(tx.value, Some(50_000));
            assert_eq!(tx.flags, TxFlags::OP_RETURN_MSG);
            assert_eq!(tx.op_return_text.as_deref(), Some("hodl"));
        }
        other => panic!("expected Unconfirmed, got {other:?}"),
    }

    // Confirm tx_confirmed via BlockObserved + BlockInclusion.
    app.state
        .heartbeat_store
        .observe_block(block_hash.clone(), 1_700_000_010);
    app.state.heartbeat_store.apply_block_inclusion(
        block_hash.clone(),
        900_000,
        vec![tx_confirmed.clone()],
    );
    let (status, body) =
        app.get(&format!("/heartbeat/tx/{tx_confirmed}")).await;
    assert_eq!(status, StatusCode::OK);
    let parsed: TxLookupStatus =
        serde_json::from_value(body).expect("parse TxLookupStatus");
    assert_eq!(
        parsed,
        TxLookupStatus::Confirmed {
            txid: tx_confirmed.clone(),
            block_hash: block_hash.clone(),
            block_height: 900_000,
        }
    );

    // Absent tx reports current coverage.
    let (status, body) = app.get(&format!("/heartbeat/tx/{tx_absent}")).await;
    assert_eq!(status, StatusCode::OK);
    let parsed: TxLookupStatus =
        serde_json::from_value(body).expect("parse TxLookupStatus");
    assert_eq!(
        parsed,
        TxLookupStatus::Absent {
            coverage: Coverage::Complete
        }
    );

    // Disconnecting the block clears the confirmed status.
    app.state.heartbeat_store.disconnect_block(block_hash);
    let (status, body) =
        app.get(&format!("/heartbeat/tx/{tx_confirmed}")).await;
    assert_eq!(status, StatusCode::OK);
    let parsed: TxLookupStatus =
        serde_json::from_value(body).expect("parse TxLookupStatus");
    assert!(matches!(parsed, TxLookupStatus::Absent { .. }));
}

#[tokio::test]
async fn heartbeat_v2_atomic_replay_to_live_handoff_has_no_gaps_or_duplicates()
{
    let app = TestApp::new();
    let tx0 = test_hex(1);
    app.state
        .heartbeat_store
        .install_checkpoint(&[&tx0], true, 1_700_000_000);

    let snap = fetch_snapshot(&app).await;
    let mut mirror = HeartbeatMirror::default();
    mirror.install_snapshot(snap.clone()).expect("install snap");

    // Publish 3 deltas AFTER snapshot fetch and BEFORE /v2 connect.
    app.state
        .heartbeat_store
        .observe_add(test_hex(2), 1_700_000_001);
    app.state
        .heartbeat_store
        .observe_add(test_hex(3), 1_700_000_002);
    app.state.heartbeat_store.enrich_tx(
        test_hex(2),
        1_200,
        140,
        75_000,
        TxFlags::NONE,
        None,
    );

    // Connect using Last-Event-ID header at the snapshot's revision.
    let req = Request::builder()
        .uri("/heartbeat/v2")
        .header(
            "Last-Event-ID",
            format!("{}:{}", snap.stream_epoch, snap.seq),
        )
        .body(Body::empty())
        .expect("build request");
    let resp = app.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let mut reader = SseReader::from_response(resp);

    // Publish 2 more live deltas after subscribing.
    app.state
        .heartbeat_store
        .observe_add(test_hex(4), 1_700_000_003);
    app.state.heartbeat_store.observe_remove(vec![test_hex(3)]);

    let mut seen_seqs = Vec::new();
    for expected_seq in (snap.seq + 1)..=(snap.seq + 5) {
        let frame = reader
            .next_event()
            .await
            .expect("expected delta frame on stream");
        assert_eq!(frame.event, "delta");
        assert_eq!(
            frame.id.as_deref(),
            Some(format!("{}:{expected_seq}", snap.stream_epoch).as_str())
        );
        let delta: HeartbeatDelta =
            serde_json::from_str(&frame.data).expect("parse delta");
        seen_seqs.push(delta.seq);
        assert_eq!(mirror.apply_delta(delta), ApplyOutcome::Applied);
    }

    let expected: Vec<u64> = ((snap.seq + 1)..=(snap.seq + 5)).collect();
    assert_eq!(seen_seqs, expected);
    assert!(mirror.has_tx(&test_hex(1)));
    assert!(mirror.has_tx(&test_hex(2)));
    assert!(!mirror.has_tx(&test_hex(3)));
    assert!(mirror.has_tx(&test_hex(4)));
}

#[tokio::test]
async fn heartbeat_v2_continuous_vs_rejoining_client_equivalence() {
    use std::sync::atomic::Ordering;

    // Small replay cap (4 events) so we can deterministically trigger
    // `cursor_expired` without allocating thousands of deltas.
    let store =
        Arc::new(HeartbeatStore::with_replay_limits(4, 16 * 1024 * 1024));
    let app = TestApp::with_heartbeat_store(store);

    let tx_a = test_hex(10);
    let tx_b = test_hex(11);
    let tx_c = test_hex(12);
    let tx_d = test_hex(13);
    let tx_e = test_hex(14);
    let tx_f = test_hex(15);
    let block_1 = test_hex(0xa1);
    let block_2 = test_hex(0xa2);

    // Seed initial checkpoint.
    app.state.heartbeat_store.install_checkpoint(
        &[&tx_a, &tx_b],
        true,
        1_700_000_000,
    );

    // Both clients start at snapshot S.
    let snap_s = fetch_snapshot(&app).await;
    let mut client_a = HeartbeatMirror::default();
    let mut client_b = HeartbeatMirror::default();
    client_a
        .install_snapshot(snap_s.clone())
        .expect("client A install S");
    client_b
        .install_snapshot(snap_s.clone())
        .expect("client B install S");

    // Client A stays continuously connected from S.
    let req_a = Request::builder()
        .uri(format!(
            "/heartbeat/v2?epoch={}&after={}",
            snap_s.stream_epoch, snap_s.seq
        ))
        .body(Body::empty())
        .expect("build request A");
    let resp_a = app.send(req_a).await;
    assert_eq!(resp_a.status(), StatusCode::OK);
    let mut reader_a = SseReader::from_response(resp_a);

    // Emit a rich mix of live deltas (well over the 4-event replay cap).
    app.state
        .heartbeat_store
        .observe_add(tx_c.clone(), 1_700_000_001);
    app.state.heartbeat_store.enrich_tx(
        tx_c.clone(),
        12_000,
        220,
        500_000_000,
        TxFlags::WHALE | TxFlags::OP_RETURN_MSG,
        Some("genesis".to_string()),
    );
    app.state.heartbeat_store.observe_remove(vec![tx_b.clone()]);
    app.state
        .heartbeat_store
        .update_node_mempool(Some(NodeMempoolAggregate {
            tx_count: 2,
            vbytes: 360,
            observed_at: 1_700_000_005,
            stale: false,
        }));
    app.state
        .heartbeat_store
        .observe_block(block_1.clone(), 1_700_000_010);
    app.state.heartbeat_store.apply_block_inclusion(
        block_1.clone(),
        900_001,
        vec![tx_a.clone()],
    );
    app.state
        .heartbeat_store
        .apply_block_details(CanonicalBlock {
            height: 900_001,
            hash: block_1.clone(),
            timestamp: 1_700_000_010,
            tx_count: 1,
            total_fees: 25_000,
            size: 1_000,
            weight: 4_000,
            observed_at: 1_700_000_010,
        });
    app.state
        .heartbeat_store
        .observe_block(block_2.clone(), 1_700_000_020);
    app.state.heartbeat_store.disconnect_block(block_2);
    app.state
        .heartbeat_store
        .observe_add(tx_d.clone(), 1_700_000_025);
    app.state.heartbeat_store.set_coverage(Coverage::Partial {
        reason: "block_disconnected".to_string(),
    });

    let (_, mid_seq) = app.state.heartbeat_store.current_revision();
    assert!(mid_seq > snap_s.seq + 4, "must exceed replay window");

    // Client B tries to resume from expired cursor S -> receives resync_required & EOF.
    let req_b_stale = Request::builder()
        .uri(format!(
            "/heartbeat/v2?epoch={}&after={}",
            snap_s.stream_epoch, snap_s.seq
        ))
        .body(Body::empty())
        .expect("build stale request B");
    let resp_b_stale = app.send(req_b_stale).await;
    assert_eq!(resp_b_stale.status(), StatusCode::OK);
    let mut reader_b_stale = SseReader::from_response(resp_b_stale);
    let resync_frame = reader_b_stale
        .next_event()
        .await
        .expect("expected resync_required frame");
    assert_eq!(resync_frame.event, "resync_required");
    let resync: ResyncRequired =
        serde_json::from_str(&resync_frame.data).expect("parse resync");
    assert_eq!(resync.reason, "cursor_expired");
    assert_eq!(resync.stream_epoch, snap_s.stream_epoch);
    assert_eq!(resync.latest_seq, mid_seq);
    assert!(
        reader_b_stale.next_event().await.is_none(),
        "resync stream must terminate cleanly"
    );
    drop(reader_b_stale);

    // Client B fetches fresh snapshot at M and subscribes to trailing deltas.
    let snap_m = fetch_snapshot(&app).await;
    assert_eq!(snap_m.seq, mid_seq);
    client_b
        .install_snapshot(snap_m.clone())
        .expect("client B install M");

    let req_b = Request::builder()
        .uri(format!(
            "/heartbeat/v2?epoch={}&after={}",
            snap_m.stream_epoch, snap_m.seq
        ))
        .body(Body::empty())
        .expect("build request B");
    let resp_b = app.send(req_b).await;
    assert_eq!(resp_b.status(), StatusCode::OK);
    let mut reader_b = SseReader::from_response(resp_b);

    // Emit final live deltas to revision R.
    app.state
        .heartbeat_store
        .observe_add(tx_e.clone(), 1_700_000_030);
    app.state.heartbeat_store.enrich_tx(
        tx_e.clone(),
        500,
        110,
        10_000,
        TxFlags::NONE,
        None,
    );
    let (_, target_seq) = app.state.heartbeat_store.current_revision();

    // Drain Client A continuously from S to R.
    while client_a.revision().unwrap().1 < target_seq {
        let frame = reader_a.next_event().await.expect("client A delta");
        assert_eq!(frame.event, "delta");
        let delta: HeartbeatDelta =
            serde_json::from_str(&frame.data).expect("parse delta A");
        assert_eq!(client_a.apply_delta(delta), ApplyOutcome::Applied);
    }

    // Drain Client B from M to R.
    while client_b.revision().unwrap().1 < target_seq {
        let frame = reader_b.next_event().await.expect("client B delta");
        assert_eq!(frame.event, "delta");
        let delta: HeartbeatDelta =
            serde_json::from_str(&frame.data).expect("parse delta B");
        assert_eq!(client_b.apply_delta(delta), ApplyOutcome::Applied);
    }

    assert!(
        client_a.same_observable_state(&client_b),
        "continuous client A and rejoining client B must converge to identical observable state"
    );
    assert_eq!(
        client_a.confirmed_in(&tx_a),
        Some((block_1.as_str(), 900_001))
    );
    assert_eq!(
        client_b.confirmed_in(&tx_a),
        Some((block_1.as_str(), 900_001)),
        "rejoining client B must recover recent confirmations from snapshot without replaying BlockInclusion"
    );

    // Part 2: Large checkpoint discontinuity (> 2,000 diff) rotates stream_epoch
    // and sends `resync_required` to live subscribers.
    let large_members: Vec<String> =
        (1000..(1000 + CHECKPOINT_RESET_DIFF_THRESHOLD as u64 + 10))
            .map(test_hex)
            .collect();
    let large_refs: Vec<&String> = large_members.iter().collect();
    app.state.heartbeat_store.install_checkpoint(
        &large_refs,
        true,
        1_700_000_100,
    );

    let reset_frame_a = reader_a
        .next_event()
        .await
        .expect("client A receives resync_required on epoch reset");
    assert_eq!(reset_frame_a.event, "resync_required");
    let reset_a: ResyncRequired =
        serde_json::from_str(&reset_frame_a.data).expect("parse reset resync");
    assert_eq!(reset_a.reason, "checkpoint_reset");
    assert_ne!(reset_a.stream_epoch, snap_s.stream_epoch);
    assert_eq!(reset_a.latest_seq, 1);
    assert!(
        reader_a.next_event().await.is_none(),
        "epoch reset stream must close cleanly"
    );

    let reset_frame_b = reader_b
        .next_event()
        .await
        .expect("client B receives resync_required on epoch reset");
    assert_eq!(reset_frame_b.event, "resync_required");
    assert!(reader_b.next_event().await.is_none());

    drop(reader_a);
    drop(reader_b);
    assert_eq!(app.state.sse_connections.load(Ordering::Relaxed), 0);

    // Both clients rejoin on the new epoch and converge again.
    let snap_new = fetch_snapshot(&app).await;
    assert_eq!(snap_new.stream_epoch, reset_a.stream_epoch);
    assert_eq!(snap_new.seq, 1);
    assert_eq!(snap_new.transactions.len(), large_members.len());
    client_a
        .install_snapshot(snap_new.clone())
        .expect("client A new epoch");
    client_b
        .install_snapshot(snap_new.clone())
        .expect("client B new epoch");
    assert_eq!(
        client_a.confirmed_in(&tx_a),
        Some((block_1.as_str(), 900_001)),
        "checkpoint reset must retain recent confirmations"
    );
    assert_eq!(
        client_b.confirmed_in(&tx_a),
        Some((block_1.as_str(), 900_001))
    );

    app.state
        .heartbeat_store
        .observe_add(tx_f.clone(), 1_700_000_110);
    let snap_after = fetch_snapshot(&app).await;
    client_a
        .install_snapshot(snap_after.clone())
        .expect("client A post-add");
    client_b
        .install_snapshot(snap_after)
        .expect("client B post-add");
    assert!(client_a.same_observable_state(&client_b));
}

#[tokio::test]
async fn heartbeat_v2_prefers_last_event_id_over_query_params() {
    let app = TestApp::new();
    let tx0 = test_hex(1);
    app.state
        .heartbeat_store
        .install_checkpoint(&[&tx0], true, 1_700_000_000);
    let (epoch, seq_base) = app.state.heartbeat_store.current_revision();
    assert_eq!(seq_base, 1);

    // Emit seq 2, 3, 4, 5.
    for i in 2..=5u64 {
        app.state
            .heartbeat_store
            .observe_add(test_hex(i), 1_700_000_000 + i);
    }

    // EventSource auto-reconnects retain the original `?epoch=E&after=1` query
    // string while sending `Last-Event-ID: E:3`. The server must prefer
    // `Last-Event-ID` and resume after seq 3 (emitting seq 4, then seq 5).
    let req = Request::builder()
        .uri(format!("/heartbeat/v2?epoch={epoch}&after=1"))
        .header("Last-Event-ID", format!("{epoch}:3"))
        .body(Body::empty())
        .expect("build request");
    let resp = app.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let mut reader = SseReader::from_response(resp);

    let first = reader.next_event().await.expect("expected seq 4 frame");
    assert_eq!(first.event, "delta");
    let d4: HeartbeatDelta =
        serde_json::from_str(&first.data).expect("parse delta 4");
    assert_eq!(d4.seq, 4);

    let second = reader.next_event().await.expect("expected seq 5 frame");
    assert_eq!(second.event, "delta");
    let d5: HeartbeatDelta =
        serde_json::from_str(&second.data).expect("parse delta 5");
    assert_eq!(d5.seq, 5);
}

// =========================================================================
// Heartbeat Phase 4: /heartbeat/block/{query} historical block spike windows
// =========================================================================

#[tokio::test]
async fn heartbeat_block_window_lookup_by_height_prefixes_and_hash() {
    let app = TestApp::new();
    // Seed blocks 40..=65 so block 50 has a full radius=5 window [45..=55]
    // plus parent 44 with a distinctive timestamp gap.
    for h in 40..=65u64 {
        let ts = if h == 44 {
            1_700_000_000 + 44 * 600 - 123
        } else {
            1_700_000_000 + h * 600
        };
        seed_block(&app, h, &test_hex(h), ts);
    }

    // 1. Lookup by integer height with explicit radius=5 -> 11 blocks [45..=55].
    //    max_returned=55 <= tip_height(65) - 6 = 59 -> deep history Cache-Control.
    let resp = app
        .send(
            Request::builder()
                .uri("/heartbeat/block/50?radius=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("public, max-age=3600")
    );
    let raw = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(body["target_height"], 50);
    assert_eq!(body["target_hash"], test_hex(50));
    assert_eq!(body["tip_height"], 65);
    let blocks = body["blocks"].as_array().expect("blocks array");
    assert_eq!(blocks.len(), 11);
    assert_eq!(blocks[0]["height"], 45);
    assert_eq!(blocks[10]["height"], 55);
    // Block 45 must use parent block 44's timestamp: (45*600) - (44*600 - 123) = 723.
    assert_eq!(blocks[0]["inter_block_seconds"], 723);
    assert_eq!(blocks[1]["inter_block_seconds"], 600);

    // Near-tip window (block 63, max_returned=65 > tip_height(65) - 6) -> Cache-Control: no-store.
    let near_tip_resp = app
        .send(
            Request::builder()
                .uri("/heartbeat/block/63?radius=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(near_tip_resp.status(), StatusCode::OK);
    assert_eq!(
        near_tip_resp
            .headers()
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-store")
    );

    // 2. Lookup by `#50` (URL-encoded `%2350`), `block:50`, `BLOCK:50`, and comma-formatted height.
    for uri in [
        "/heartbeat/block/%2350?radius=5",
        "/heartbeat/block/block:50?radius=5",
        "/heartbeat/block/BLOCK:50?radius=5",
    ] {
        let (status, body) = app.get(uri).await;
        assert_eq!(status, StatusCode::OK, "expected 200 for {uri}");
        assert_eq!(body["target_height"], 50);
        assert_eq!(body["target_hash"], test_hex(50));
        assert_eq!(body["blocks"].as_array().unwrap().len(), 11);
    }

    // Comma-formatted decimal height (e.g. 1,000).
    seed_block(&app, 1_000, &test_hex(1_000), 1_700_600_000);
    let (status, body) = app.get("/heartbeat/block/1,000?radius=5").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["target_height"], 1_000);
    assert_eq!(body["target_hash"], test_hex(1_000));
    assert_eq!(body["tip_height"], 1_000);

    // 3. Lookup by mixed-case 64-hex hash.
    let hash_50_mixed = {
        let h = test_hex(50);
        format!("{}{}", h[..32].to_ascii_uppercase(), &h[32..])
    };
    let (status, body) = app
        .get(&format!("/heartbeat/block/{hash_50_mixed}?radius=5"))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["target_height"], 50);
    assert_eq!(body["target_hash"], test_hex(50));
    assert_eq!(body["blocks"].as_array().unwrap().len(), 11);

    // Radius clamping: radius=1 clamps up to 5 (11 blocks centered on 50).
    let (status, body) = app.get("/heartbeat/block/50?radius=1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["blocks"].as_array().unwrap().len(), 11);
}

#[tokio::test]
async fn heartbeat_block_window_genesis_zero_and_negative_mtp_deltas() {
    let app = TestApp::new();
    seed_block(&app, 0, &test_hex(0), 1_231_006_505);
    seed_block(&app, 1, &test_hex(1), 1_231_469_665);
    seed_block(&app, 2, &test_hex(2), 1_231_469_665); // 0-second delta
    seed_block(&app, 3, &test_hex(3), 1_231_469_650); // -15s MTP delta
    seed_block(&app, 4, &test_hex(4), 1_231_470_000); // +350s delta

    // Genesis block 0 -> inter_block_seconds is JSON null (never 600).
    let (status, body) = app.get("/heartbeat/block/0?radius=5").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["target_height"], 0);
    assert_eq!(body["target_hash"], test_hex(0));
    assert_eq!(body["tip_height"], 4);

    let blocks = body["blocks"].as_array().expect("blocks array");
    assert_eq!(blocks.len(), 5);
    assert!(
        blocks[0]["inter_block_seconds"].is_null(),
        "genesis inter_block_seconds must be null, got {:?}",
        blocks[0]["inter_block_seconds"]
    );
    assert_eq!(
        blocks[1]["inter_block_seconds"],
        1_231_469_665 - 1_231_006_505
    );
    assert_eq!(blocks[2]["inter_block_seconds"], 0);
    assert_eq!(blocks[3]["inter_block_seconds"], -15);
    assert_eq!(blocks[4]["inter_block_seconds"], 350);
}

#[tokio::test]
async fn heartbeat_block_window_not_found_and_malformed_queries() {
    let app = TestApp::new();
    seed_block(&app, 10, &test_hex(10), 1_700_000_000);

    // Missing height -> 404.
    let (status, body) = app.get("/heartbeat/block/999999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "Block not found");

    // Missing 64-hex hash -> 404.
    let missing_hash = test_hex(999_999);
    let (status, body) =
        app.get(&format!("/heartbeat/block/{missing_hash}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "Block not found");

    // Malformed queries (including multi-byte UTF-8 around byte offset 6 and
    // invalid comma grouping) -> 400, no panic.
    for bad in [
        "not-a-block",
        "ab%C3%A9cd",
        "a%C3%A9%C3%A9%C3%A9",
        "12345678901", // 11 digits (> 10 max)
        "8,4,0000",
        ",840,000",
        "840,000,",
        "0,000",
        "1234,567",
        "zzzz",
    ] {
        let (status, body) = app.get(&format!("/heartbeat/block/{bad}")).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "expected 400 for malformed query {bad:?}"
        );
        assert_eq!(
            body["error"],
            "Expected a block height or 64-character hex block hash"
        );
    }
}

#[tokio::test]
async fn heartbeat_block_range_returns_parent_derived_deltas_and_validates_bounds(
) {
    let app = TestApp::new();
    // Seed genesis + 1..=5 with 0s and negative MTP deltas, plus 39..=60 with
    // a distinctive timestamp offset on parent block 39.
    seed_block(&app, 0, &test_hex(0), 1_231_006_505);
    seed_block(&app, 1, &test_hex(1), 1_231_469_665);
    seed_block(&app, 2, &test_hex(2), 1_231_469_665); // 0s delta
    seed_block(&app, 3, &test_hex(3), 1_231_469_650); // -15s MTP delta
    seed_block(&app, 4, &test_hex(4), 1_231_470_000); // +350s delta
    seed_block(&app, 5, &test_hex(5), 1_231_470_600); // +600s delta

    for h in 39..=60u64 {
        let ts = if h == 39 {
            1_700_000_000 + 39 * 600 - 111
        } else {
            1_700_000_000 + h * 600
        };
        seed_block(&app, h, &test_hex(h), ts);
    }

    // 1. Near-tip range 40..=55 (max_returned=55 > tip_height(60) - 6 = 54) ->
    //    200, 16 blocks, exact parent-derived delta on block 40, tip_height
    //    populated, and Cache-Control: no-store.
    let resp = app
        .send(
            Request::builder()
                .uri("/heartbeat/blocks?from=40&to=55")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-store")
    );
    let raw = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(body["from_height"], 40);
    assert_eq!(body["to_height"], 55);
    assert_eq!(body["tip_height"], 60);
    let blocks = body["blocks"].as_array().expect("blocks array");
    assert_eq!(blocks.len(), 16);
    assert_eq!(blocks[0]["height"], 40);
    assert_eq!(blocks[15]["height"], 55);
    // Block 40 uses parent 39's timestamp: (40*600) - (39*600 - 111) = 711.
    assert_eq!(blocks[0]["inter_block_seconds"], 711);
    assert_eq!(blocks[1]["inter_block_seconds"], 600);

    // 1b. Deep-history range 40..=54 (max_returned=54 <= tip_height(60) - 6) ->
    //     Cache-Control: public, max-age=3600.
    let deep_resp = app
        .send(
            Request::builder()
                .uri("/heartbeat/blocks?from=40&to=54")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(deep_resp.status(), StatusCode::OK);
    assert_eq!(
        deep_resp
            .headers()
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("public, max-age=3600")
    );

    // 2. Genesis range 0..=5 -> 200, genesis inter_block_seconds is null.
    let (status, body) = app.get("/heartbeat/blocks?from=0&to=5").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["from_height"], 0);
    assert_eq!(body["to_height"], 5);
    assert_eq!(body["tip_height"], 60);
    let blocks = body["blocks"].as_array().expect("blocks array");
    assert_eq!(blocks.len(), 6);
    assert_eq!(blocks[0]["height"], 0);
    assert!(
        blocks[0]["inter_block_seconds"].is_null(),
        "genesis inter_block_seconds must be null, got {:?}",
        blocks[0]["inter_block_seconds"]
    );
    assert_eq!(
        blocks[1]["inter_block_seconds"],
        1_231_469_665 - 1_231_006_505
    );
    assert_eq!(blocks[2]["inter_block_seconds"], 0);
    assert_eq!(blocks[3]["inter_block_seconds"], -15);

    // 3. Reversed range -> 400.
    let (status, body) = app.get("/heartbeat/blocks?from=60&to=50").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].is_string());

    // 4. Over-cap range (0..=500 is 501 blocks) -> 400; 0..=499 (500 blocks) -> 200.
    let (status, body) = app.get("/heartbeat/blocks?from=0&to=500").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].is_string());

    let (status, _body) = app.get("/heartbeat/blocks?from=0&to=499").await;
    assert_eq!(status, StatusCode::OK);

    // 5. Above tip, valid range -> 200, empty blocks array, Cache-Control: no-store.
    let empty_resp = app
        .send(
            Request::builder()
                .uri("/heartbeat/blocks?from=900000&to=900010")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(empty_resp.status(), StatusCode::OK);
    assert_eq!(
        empty_resp
            .headers()
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-store")
    );
    let raw = axum::body::to_bytes(empty_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(body["from_height"], 900_000);
    assert_eq!(body["to_height"], 900_010);
    assert_eq!(body["tip_height"], 60);
    assert_eq!(body["blocks"].as_array().unwrap().len(), 0);
}
