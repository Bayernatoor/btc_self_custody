use super::*;
use axum::{routing::post, Json, Router};
use serde_json::{json, Value};
use std::sync::atomic::AtomicUsize;

/// Exercise the actual HTTP requests, wire decoding and shared jobs. The
/// stub can change its active hash independently of the returned block data.
#[tokio::test]
async fn poll_reuses_full_payload_and_retired_jobs_cannot_publish() {
    let hash = "12".repeat(32);
    let id = "ab".repeat(32);
    let fetches = Arc::new(AtomicUsize::new(0));
    let canonical = Arc::new(AtomicBool::new(true));
    let loaded = Arc::new(AtomicBool::new(false));
    let batches = Arc::new(AtomicUsize::new(0));
    let handler = {
        let (fetches, canonical, loaded) =
            (fetches.clone(), canonical.clone(), loaded.clone());
        let (hash, id) = (hash.clone(), id.clone());
        let batches = batches.clone();
        move |Json(request): Json<Value>| {
            let response = if let Some(batch) = request.as_array() {
                batches.fetch_add(1, Ordering::Relaxed);
                assert_eq!(batch[0]["method"], "getmempoolinfo");
                assert_eq!(batch[1]["method"], "getrawmempool");
                assert_eq!(batch[1]["params"], json!([false, true]));
                json!([
                    {"id":1,"result":{"loaded":loaded.load(Ordering::Relaxed)},"error":null},
                    {"id":2,"result":{"mempool_sequence":9007199254741001u64,"txids":[]},"error":null}
                ])
            } else {
                let result = match request["method"].as_str().unwrap() {
                    "getmempoolinfo" => {
                        json!({"size": crate::stats::heartbeat_membership::MAX_MEMBERS + 1})
                    }
                    "getblockhash" => {
                        if canonical.load(Ordering::Relaxed) {
                            json!(hash)
                        } else {
                            json!("ff".repeat(32))
                        }
                    }
                    "getblock" => {
                        assert_eq!(
                            request["params"],
                            json!([hash, 2]),
                            "poll issued another full payload lookup"
                        );
                        fetches.fetch_add(1, Ordering::Relaxed);
                        json!({"hash":hash,"height":155,"time":1700000123,"confirmations":1,"size":226,"weight":904,"nTx":1,"tx":[{"txid":id,"vin":[{"coinbase":"03000000"}],"vout":[]}]})
                    }
                    other => panic!("unexpected RPC {other}"),
                };
                json!({"result":result,"error":null})
            };
            async move { Json(response) }
        }
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/", post(handler)))
            .await
            .unwrap()
    });
    let rpc = BitcoinRpc::new(url, String::new(), String::new());
    assert!(matches!(
        rpc.membership_checkpoint(true).await,
        Err(StatsError::CheckpointCapacity)
    ));
    assert_eq!(
        batches.load(Ordering::Relaxed),
        0,
        "oversized retry fetched the full ID list"
    );
    assert!(rpc.membership_checkpoint(false).await.is_err());
    loaded.store(true, Ordering::Relaxed);
    let cp = rpc.membership_checkpoint(false).await.unwrap();
    assert!(cp.txids.is_empty());
    assert_eq!(cp.mempool_sequence, 9007199254741001);

    let jobs = BlockJobs::default();
    let (_, ids) = rpc.fetch_polled_block(155).await.unwrap();
    jobs.seed_polled(ids.unwrap());
    let poll = jobs.entry(&hash).unwrap();
    let sequence = jobs.entry(&hash).unwrap();
    let (a, b) =
        tokio::join!(poll.accepted_ids(&rpc), sequence.accepted_ids(&rpc));
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(a.txids, [id]);
    assert_eq!(a.weight, 904);
    assert_eq!(fetches.load(Ordering::Relaxed), 1);

    canonical.store(false, Ordering::Relaxed);
    assert!(
        poll.accepted_ids(&rpc).await.is_err(),
        "cached payload must not cache acceptance"
    );
    jobs.disconnect(&hash);
    let again = jobs.entry(&hash).unwrap();
    assert_eq!(jobs.if_current(&poll, || true), None);
    assert_eq!(jobs.if_current(&again, || true), Some(true));
    server.abort();
}

#[tokio::test]
async fn wait_announcement_wakes_without_lost_notification() {
    let jobs = BlockJobs::default();
    let hash = "34".repeat(32);
    for _ in 0..200 {
        jobs.disconnect(&hash);
        let job = jobs.entry(&hash).unwrap();
        job.running.store(true, Ordering::Release);
        let waiter_job = Arc::clone(&job);
        let waiter =
            tokio::spawn(async move { waiter_job.wait_announcement().await });
        tokio::task::yield_now().await;
        job.block_sent.store(true, Ordering::Release);
        job.finish_attempt();
        let announced = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("wait_announcement hung on lost wakeup")
            .unwrap();
        assert!(announced);
    }
}

#[test]
fn observe_block_serialization_cannot_publish_after_retire() {
    let jobs = BlockJobs::default();
    let store = HeartbeatStore::new();
    let hash = "56".repeat(32);

    let job = jobs.entry(&hash).unwrap();
    // Simulate first observe_block inside if_current.
    let first = jobs.if_current(&job, || {
        if !job.observed_sent.swap(true, Ordering::Relaxed) {
            store.observe_block(hash.clone(), job.observed_at);
            true
        } else {
            false
        }
    });
    assert_eq!(first, Some(true));
    assert_eq!(store.current_revision().1, 1);

    // Second observe_block for the same active job is deduplicated by observed_sent.
    let duplicate = jobs.if_current(&job, || {
        if !job.observed_sent.swap(true, Ordering::Relaxed) {
            store.observe_block(hash.clone(), job.observed_at);
            true
        } else {
            false
        }
    });
    assert_eq!(duplicate, Some(false));
    assert_eq!(store.current_revision().1, 1);

    // Retire via on_disconnect and verify stale job cannot publish BlockObserved.
    jobs.on_disconnect(&hash, || {
        store.disconnect_block(hash.clone());
    });
    assert_eq!(store.current_revision().1, 2);
    let late_job = jobs.if_current(&job, || {
        store.observe_block(hash.clone(), job.observed_at);
    });
    assert!(late_job.is_none());
    assert_eq!(store.current_revision().1, 2);
    let snap: crate::stats::heartbeat_sync::HeartbeatSnapshot =
        serde_json::from_slice(&store.snapshot().json).expect("valid snapshot");
    assert!(snap.pending_blocks.is_empty());
}

#[tokio::test]
async fn polled_seed_unblocks_stalled_block_ids_and_fees_waiters() {
    let hash = "77".repeat(32);
    let cb_txid = "88".repeat(32);
    let spend_txid = "99".repeat(32);
    let server_hash = hash.clone();

    let app = Router::new().route(
        "/",
        post(move |Json(req): Json<Value>| {
            let h = server_hash.clone();
            async move {
                let method = req["method"].as_str().unwrap_or("");
                let result = match method {
                    "getblockhash" => json!(h),
                    "getblock" => {
                        // Simulate a stalled ZMQ `getblock(hash, 1)` RPC.
                        tokio::time::sleep(Duration::from_secs(10)).await;
                        json!({})
                    }
                    _ => json!(null),
                };
                Json(json!({ "result": result, "error": null }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let rpc = BitcoinRpc::new(format!("http://{addr}"), "u".into(), "p".into());
    let jobs = Arc::new(BlockJobs::default());
    let job = jobs.entry(&hash).unwrap();

    let seed = PolledBlockSeed {
        ids: BlockTxids {
            hash: hash.clone(),
            height: 977_709,
            confirmations: 1,
            timestamp: 1_764_000_000,
            size: 1_650_000,
            weight: 3_992_000,
            tx_count: 2,
            txids: vec![cb_txid.clone(), spend_txid.clone()],
        },
        exact_fees: Some(42_500),
        miner: Some("Foundry USA".to_string()),
    };

    let jobs_bg = Arc::clone(&jobs);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        jobs_bg.seed_polled(seed);
    });

    let started = Instant::now();
    let (ids_res, fees_val) =
        tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(job.accepted_ids(&rpc), job.wait_polled_fees())
        })
        .await
        .expect("polled seed must unblock stalled RPCs well before 2s");

    let ids = ids_res.expect("accepted_ids resolved via polled seed");
    assert_eq!(ids.height, 977_709);
    assert_eq!(ids.txids, vec![cb_txid, spend_txid]);
    assert!(
        job.ids.get().is_some(),
        "polled ids must be cached in OnceCell after select! drops get_or_try_init"
    );
    assert_eq!(fees_val, 42_500);
    assert_eq!(
        job.polled_miner.get().map(String::as_str),
        Some("Foundry USA")
    );
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "expected sub-second wakeup, took {:?}",
        started.elapsed()
    );

    // Retiring a stalled job unblocks `accepted_ids` immediately instead of hanging a permit.
    let stalled_hash = "66".repeat(32);
    let stalled_job = jobs.entry(&stalled_hash).unwrap();
    let jobs_retire = Arc::clone(&jobs);
    let retire_hash = stalled_hash.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(25)).await;
        jobs_retire.disconnect(&retire_hash);
    });
    let retire_res = tokio::time::timeout(
        Duration::from_secs(1),
        stalled_job.accepted_ids(&rpc),
    )
    .await
    .expect("retired job must abort stalled RPC immediately");
    assert!(retire_res.is_err());

    server.abort();
}
