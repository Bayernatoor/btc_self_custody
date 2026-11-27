use super::*;
use rusqlite::{Connection, TransactionBehavior};

fn observe_tx(conn: &Connection, id: &str) {
    db::insert_mempool_tx(
        conn,
        &db::MempoolTxInsert {
            txid: id,
            fee: 17,
            ..Default::default()
        },
    )
    .unwrap();
    db::insert_notable_tx(
        conn,
        &db::NotableTxInsert {
            txid: id,
            notable_type: "whale",
            ..Default::default()
        },
    )
    .unwrap();
}

fn confirm(jobs: &BlockJobs, hash: &str, ids: &[&str]) -> ClaimWrite {
    let job = jobs.entry(hash).unwrap();
    ClaimWrite::Confirm {
        hash: job.hash.clone(),
        retired: Arc::clone(&job.retired),
        ids: Arc::new(BlockTxids {
            hash: hash.into(),
            confirmations: 1,
            height: 123,
            timestamp: 100,
            size: 100,
            weight: 400,
            tx_count: ids.len() as u64,
            txids: ids.iter().map(|id| id.to_string()).collect(),
        }),
    }
}

fn assert_claim(
    conn: &Connection,
    id: &str,
    height: Option<u64>,
    hash: Option<&str>,
) {
    for table in ["mempool_txs", "notable_txs"] {
        let got: (Option<u64>, Option<String>, bool) = conn.query_row(
            &format!("SELECT confirmed_height, confirmed_hash, confirmed_at IS NOT NULL FROM {table} WHERE txid = ?1"),
            [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap();
        assert_eq!(
            (got.0, got.1.as_deref(), got.2),
            (height, hash, height.is_some()),
            "{table}/{id}"
        );
    }
}

#[test]
fn waiting_sqlite_writer_cannot_confirm_a_readmission_after_disconnect() {
    // The busy handler signals that the old write actually reached the SQLite
    // lock. Moving the generation check above BEGIN IMMEDIATE breaks this test.
    static BUSY: OnceLock<mpsc::Sender<()>> = OnceLock::new();
    let (notify, blocked) = mpsc::channel();
    BUSY.set(notify).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("race.db");
    let mut db = Connection::open(&path).unwrap();
    db::init_schema(&db).unwrap();
    let mut writer = Connection::open(&path).unwrap();
    writer
        .busy_handler(Some(|attempt| {
            if attempt == 0 {
                let _ = BUSY.get().unwrap().send(());
            }
            std::thread::sleep(Duration::from_millis(2));
            attempt < 2000
        }))
        .unwrap();

    let held = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let jobs = BlockJobs::default();
    let late = confirm(&jobs, "old", &["readmitted"]);
    let worker = std::thread::spawn(move || late.apply(&mut writer).unwrap());
    blocked
        .recv_timeout(Duration::from_secs(3))
        .expect("old writer never reached SQLite");
    jobs.disconnect("old");
    jobs.reserve("old"); // even a new job of the same hash cannot revive the old write
    observe_tx(&held, "readmitted");
    held.commit().unwrap();
    worker.join().unwrap();

    // Check *before* cleanup: no transient stale commit, even to notable history.
    assert_claim(&db, "readmitted", None, None);
    db::revoke_heartbeat_claims(&mut db, "old").unwrap();
    assert_eq!(
        db::query_unconfirmed_txids(&db, 10).unwrap(),
        ["readmitted"]
    );
}

#[test]
fn late_disconnect_only_revokes_the_matching_hash_without_readmitting() {
    let mut db = Connection::open_in_memory().unwrap();
    db::init_schema(&db).unwrap();
    let jobs = BlockJobs::default();
    for id in ["old-only", "shared", "seen-again", "legacy"] {
        observe_tx(&db, id);
    }
    for table in ["mempool_txs", "notable_txs"] {
        db.execute(&format!("UPDATE {table} SET confirmed_height = 123, confirmed_at = 1 WHERE txid = 'legacy'"), []).unwrap();
    }
    // Reopen the pre-claim schema with its original photos still in place.
    db.execute_batch("
        DROP INDEX idx_mempool_txs_confirmed_hash; DROP INDEX idx_notable_txs_confirmed_hash;
        DROP INDEX idx_mempool_admitted;
        ALTER TABLE mempool_txs DROP COLUMN confirmed_hash;
        ALTER TABLE mempool_txs DROP COLUMN awaiting_readmission;
        ALTER TABLE notable_txs DROP COLUMN confirmed_hash;
    ").unwrap();
    db::init_schema(&db).unwrap();
    assert_claim(&db, "legacy", Some(123), None);
    confirm(
        &jobs,
        "old",
        &["old-only", "shared", "seen-again", "legacy"],
    )
    .apply(&mut db)
    .unwrap();
    assert_claim(&db, "old-only", Some(123), Some("old"));

    // Another source can already know the replacement before D reaches this one.
    confirm(&jobs, "new", &["shared"]).apply(&mut db).unwrap();
    // A delayed pre-block write while confirmed_height is still stamped must
    // NOT clear awaiting_readmission (otherwise a double-spent tx resurrects on Revoke).
    observe_tx(&db, "old-only");
    ClaimWrite::Revoke("old".into()).apply(&mut db).unwrap();
    assert_claim(&db, "legacy", Some(123), None);
    assert_claim(&db, "shared", Some(123), Some("new"));
    assert_claim(&db, "old-only", None, None);
    assert_claim(&db, "seen-again", None, None);
    // Neither disconnected row is active until positive post-revoke evidence arrives.
    assert!(db::query_unconfirmed_txids(&db, 10).unwrap().is_empty());
    observe_tx(&db, "seen-again");
    assert_eq!(
        db::query_unconfirmed_txids(&db, 10).unwrap(),
        ["seen-again"]
    );
    assert_eq!(db::count_unconfirmed_mempool_txs(&db).unwrap(), 1);
    let history = db::query_recent_mempool_txs(&db, 0, 10).unwrap();
    assert_eq!(
        history.iter().map(|r| r.txid.as_str()).collect::<Vec<_>>(),
        ["seen-again"]
    );
    let notables =
        db::query_notable_txs(&db, &Default::default(), 10, 0).unwrap();
    assert_eq!(notables.len(), 4);
    assert_eq!(
        notables
            .iter()
            .find(|r| r.txid == "shared")
            .unwrap()
            .confirmed_hash
            .as_deref(),
        Some("new")
    );

    // Core's reconcile, independently of D, can supply the next admission.
    db::insert_missing_mempool_txs(
        &db,
        &[crate::stats::rpc::VerboseMempoolEntry {
            txid: "old-only".into(),
            fee: 17,
            vsize: 200,
            time: 0,
            is_rbf: false,
            is_segwit: true,
        }],
        200,
        10,
    )
    .unwrap();
    assert_eq!(db::count_unconfirmed_mempool_txs(&db).unwrap(), 2);
}

#[test]
fn queued_disconnect_precedes_a_reconnection_of_the_same_hash() {
    let dir = tempfile::tempdir().unwrap();
    let pool = db::open_pool(&dir.path().join("reconnect.db"), 1).unwrap();
    let jobs = BlockJobs::default();
    let (sender, receiver) = mpsc::channel();
    jobs.writer.set(sender).unwrap();
    let mut conn = pool.get().unwrap();
    observe_tx(&conn, "shared");
    conn.execute_batch("CREATE TABLE changes (hash TEXT);
        CREATE TRIGGER record_change AFTER UPDATE OF confirmed_hash ON notable_txs
        BEGIN INSERT INTO changes VALUES (new.confirmed_hash); END;").unwrap();
    confirm(&jobs, "same", &["shared"])
        .apply(&mut conn)
        .unwrap();
    let (events, mut rx) = tokio::sync::broadcast::channel(4);
    jobs.retire("same", &pool, &events);
    assert!(
        matches!(rx.try_recv(), Ok(HeartbeatEvent::BlockDisconnected { hash }) if hash == "same")
    );
    assert_claim(&conn, "shared", Some(123), Some("same")); // DB is still held
    jobs.persist(&pool, confirm(&jobs, "same", &["shared"]));
    drop(conn);
    drop(jobs);
    write_claims(pool.clone(), receiver);
    let conn = pool.get().unwrap();
    assert_claim(&conn, "shared", Some(123), Some("same"));
    let changes: Vec<Option<String>> = conn
        .prepare("SELECT hash FROM changes ORDER BY rowid")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(changes, [Some("same".into()), None, Some("same".into())]);
}
