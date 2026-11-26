//! A poll, sequence C and hashblock can all be first. Share the immutable
//! payload by hash; acceptance is mutable and must be checked at publication.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::sync::{Notify, OnceCell, Semaphore};

use super::{timed_block_rpc, HeartbeatEvent, StatsError, StatsState};
use crate::stats::{
    db,
    heartbeat_state::HeartbeatStore,
    heartbeat_sync::CanonicalBlock,
    rpc::{BitcoinRpc, BlockTxids, PolledBlockSeed},
};

const RECENT_BLOCKS: usize = 12;
const CONCURRENT_BLOCKS: usize = 4;

#[cfg(test)]
#[path = "heartbeat_server_glue_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "heartbeat_claim_tests.rs"]
mod claim_tests;

async fn retry_block_rpc<T, F, Fut>(
    hash: &str,
    method: &'static str,
    mut call: F,
) -> Result<T, StatsError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, StatsError>>,
{
    let mut attempt = 1;
    loop {
        let result = timed_block_rpc(hash, method, attempt, call()).await;
        if result.is_ok() || attempt == 3 {
            return result;
        }
        attempt += 1;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

pub(crate) struct BlockJobs {
    entries: Mutex<VecDeque<Arc<BlockJob>>>,
    permits: Arc<Semaphore>,
    writer: OnceLock<mpsc::Sender<ClaimWrite>>,
}

impl Default for BlockJobs {
    fn default() -> Self {
        Self {
            entries: Default::default(),
            permits: Arc::new(Semaphore::new(CONCURRENT_BLOCKS)),
            writer: OnceLock::new(),
        }
    }
}

pub(crate) struct BlockJob {
    hash: String,
    first_receipt: Instant,
    observed_at: u64,
    ids: OnceCell<Arc<BlockTxids>>,
    /// Immutable payload seeded by the 15s poller (`getblock` verbosity 2) so
    /// a stalled ZMQ `getblock(hash, 1)` or `getblockstats` RPC can resolve
    /// immediately once SQLite already has the block.
    polled_ids: OnceLock<Arc<BlockTxids>>,
    polled_fees: OnceLock<u64>,
    polled_miner: OnceLock<String>,
    polled_ready: Notify,
    running: AtomicBool,
    observed_sent: AtomicBool,
    ids_sent: AtomicBool,
    block_sent: AtomicBool,
    retired: Arc<AtomicBool>,
    finished: Notify,
}

enum ClaimWrite {
    Confirm {
        hash: String,
        retired: Arc<AtomicBool>,
        ids: Arc<BlockTxids>,
    },
    Revoke(String),
}

impl ClaimWrite {
    fn hash(&self) -> &str {
        match self {
            Self::Confirm { hash, .. } => hash,
            Self::Revoke(hash) => hash,
        }
    }

    fn apply(&self, conn: &mut rusqlite::Connection) -> rusqlite::Result<()> {
        match self {
            Self::Confirm { hash, retired, ids } => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let (confirmed, _fees) = db::confirm_heartbeat_txs(
                    conn,
                    &ids.txids,
                    hash,
                    ids.height,
                    now,
                    || !retired.load(Ordering::Acquire),
                )?;
                if !retired.load(Ordering::Acquire) {
                    tracing::info!(
                        %hash,
                        height = ids.height,
                        confirmed,
                        txids = ids.txids.len(),
                        "block {} DB-confirmed {}/{} tracked txs",
                        ids.height,
                        confirmed,
                        ids.txids.len()
                    );
                }
                Ok(())
            }
            Self::Revoke(hash) => db::revoke_heartbeat_claims(conn, hash),
        }
    }
}

fn write_claims(pool: db::DbPool, receiver: mpsc::Receiver<ClaimWrite>) {
    const MAX_ATTEMPTS: u32 = 20;
    for write in receiver {
        let mut attempt = 0u32;
        loop {
            match pool.get() {
                Ok(mut conn) => match write.apply(&mut conn) {
                    Ok(()) => break,
                    Err(e)
                        if matches!(
                            e.sqlite_error_code(),
                            Some(
                                rusqlite::ErrorCode::DatabaseBusy
                                    | rusqlite::ErrorCode::DatabaseLocked
                            )
                        ) =>
                    {
                        attempt += 1;
                        if attempt >= MAX_ATTEMPTS {
                            tracing::error!(
                                hash = write.hash(),
                                attempts = attempt,
                                "heartbeat claim writer giving up after busy retries: {e}"
                            );
                            break;
                        }
                        tracing::warn!(
                            hash = write.hash(),
                            attempt,
                            "heartbeat claim writer busy: {e}"
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            hash = write.hash(),
                            "heartbeat claim write failed: {e}"
                        );
                        break;
                    }
                },
                Err(e) => {
                    attempt += 1;
                    if attempt >= MAX_ATTEMPTS {
                        tracing::error!(
                            hash = write.hash(),
                            attempts = attempt,
                            "heartbeat claim writer giving up waiting for DB pool: {e}"
                        );
                        break;
                    }
                    tracing::warn!(
                        hash = write.hash(),
                        attempt,
                        "heartbeat claim writer waiting for DB: {e}"
                    );
                }
            }
            let backoff_secs = (1u64 << attempt.min(3)).min(5);
            std::thread::sleep(Duration::from_secs(backoff_secs));
        }
    }
}

impl BlockJobs {
    /// Snapshot exact claims already published, including blocks first seen by ZMQ.
    pub(crate) fn published(&self) -> Vec<(u64, String)> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|job| job.ids_sent.load(Ordering::Relaxed))
            .filter_map(|job| {
                let ids = job.ids.get().or_else(|| job.polled_ids.get())?;
                Some((ids.height, job.hash.clone()))
            })
            .collect()
    }

    /// Preserve first observation time while the poller's DB payload is in flight.
    pub(crate) fn reserve(&self, hash: &str) {
        let _ = self.entry(hash);
    }

    pub(crate) fn seed_polled(&self, seed: PolledBlockSeed) {
        if let Some(job) = self.entry(&seed.ids.hash) {
            let ids = Arc::new(seed.ids);
            let _ = job.polled_ids.set(Arc::clone(&ids));
            let _ = job.ids.set(ids);
            if let Some(fees) = seed.exact_fees {
                let _ = job.polled_fees.set(fees);
            }
            if let Some(miner) = seed.miner {
                let _ = job.polled_miner.set(miner);
            }
            job.polled_ready.notify_waiters();
        }
    }
    fn entry(&self, hash: &str) -> Option<Arc<BlockJob>> {
        let mut entries = self.entries.lock().unwrap();
        if let Some(entry) = entries.iter().find(|e| e.hash == hash) {
            return Some(Arc::clone(entry));
        }
        if entries.len() == RECENT_BLOCKS {
            let i = entries
                .iter()
                .position(|entry| Arc::strong_count(entry) == 1)?;
            entries.remove(i);
        }
        let job = Arc::new(BlockJob {
            hash: hash.into(),
            first_receipt: Instant::now(),
            observed_at: now_unix_secs(),
            ids: OnceCell::new(),
            polled_ids: OnceLock::new(),
            polled_fees: OnceLock::new(),
            polled_miner: OnceLock::new(),
            polled_ready: Notify::new(),
            running: false.into(),
            observed_sent: false.into(),
            ids_sent: false.into(),
            block_sent: false.into(),
            retired: Arc::new(AtomicBool::new(false)),
            finished: Notify::new(),
        });
        entries.push_back(Arc::clone(&job));
        Some(job)
    }

    fn on_disconnect(&self, hash: &str, publish: impl FnOnce()) {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|job| {
            if job.hash == hash {
                job.retired.store(true, Ordering::Release);
                job.finished.notify_waiters();
                false
            } else {
                true
            }
        });
        publish();
    }

    #[cfg(test)]
    fn disconnect(&self, hash: &str) {
        self.on_disconnect(hash, || {});
    }

    #[cfg(test)]
    fn retire(
        &self,
        hash: &str,
        pool: &db::DbPool,
        events: &tokio::sync::broadcast::Sender<HeartbeatEvent>,
    ) {
        self.retire_with_store(hash, pool, events, None);
    }

    fn retire_with_store(
        &self,
        hash: &str,
        pool: &db::DbPool,
        events: &tokio::sync::broadcast::Sender<HeartbeatEvent>,
        store: Option<&HeartbeatStore>,
    ) {
        self.on_disconnect(hash, || {
            if let Some(store) = store {
                store.disconnect_block(hash.into());
            }
            let _ = events
                .send(HeartbeatEvent::BlockDisconnected { hash: hash.into() });
            self.persist(pool, ClaimWrite::Revoke(hash.into()));
        });
    }

    /// Enqueue under the publication lock. FIFO also orders D against a new
    /// connection of the *same hash*, for which a hash predicate alone cannot
    /// distinguish a delayed cleanup. Only the dedicated writer touches SQLite.
    fn persist(&self, pool: &db::DbPool, write: ClaimWrite) {
        let sender = self.writer.get_or_init(|| {
            let (sender, receiver) = mpsc::channel::<ClaimWrite>();
            let pool = pool.clone();
            std::thread::spawn(move || write_claims(pool, receiver));
            sender
        });
        if sender.send(write).is_err() {
            tracing::error!("heartbeat claim writer stopped");
        }
    }

    /// A second connection of the same hash is a new observation generation.
    /// Serialize the final local generation check and publication against D.
    pub(super) fn if_current<T>(
        &self,
        job: &Arc<BlockJob>,
        f: impl FnOnce() -> T,
    ) -> Option<T> {
        let entries = self.entries.lock().unwrap();
        entries.iter().any(|entry| Arc::ptr_eq(entry, job)).then(f)
    }

    /// Publish exact block inclusion once for a still-current job.
    ///
    /// Called from both `observe_block` and the shadow sequence task so
    /// `HeartbeatStore::apply_block_inclusion` is guaranteed to run before the
    /// post-connect `getrawmempool` checkpoint starts.
    pub(super) fn publish_inclusion_if_current(
        &self,
        state: &StatsState,
        job: &Arc<BlockJob>,
        info: &Arc<BlockTxids>,
    ) {
        self.if_current(job, || {
            if job.ids_sent.swap(true, Ordering::Relaxed) {
                return;
            }
            let hash = &job.hash;
            state.rpc.invalidate_tip_caches(info.height);
            state.heartbeat_store.apply_block_inclusion(
                hash.clone(),
                info.height,
                info.txids.clone(),
            );
            let event = HeartbeatEvent::BlockTxids {
                hash: hash.clone(),
                height: info.height,
                txids: info.txids.clone(),
            };
            let receivers = state.heartbeat_tx.send(event).unwrap_or(0);
            tracing::info!(
                stage = "block_txids",
                %hash,
                height = info.height,
                txids = info.txids.len(),
                receivers,
                "heartbeat_block"
            );
            self.persist(
                &state.db,
                ClaimWrite::Confirm {
                    hash: hash.clone(),
                    retired: Arc::clone(&job.retired),
                    ids: Arc::clone(info),
                },
            );
        });
    }
}

/// Publish D immediately and retire the matching durable claims, including
/// hashes no longer in the bounded job cache (or confirmed before restart).
pub(crate) fn retire_block(state: &Arc<StatsState>, hash: &str) {
    state.heartbeat_blocks.retire_with_store(
        hash,
        &state.db,
        &state.heartbeat_tx,
        Some(&state.heartbeat_store),
    );
}

impl BlockJob {
    /// A polled child must not overtake its parent's public block event.
    /// Pin and enable the waiter BEFORE checking atomics: `notify_waiters()`
    /// stores no permit, so an unenabled `Notified` misses concurrent wakeups.
    pub(crate) async fn wait_announcement(&self) -> bool {
        loop {
            let finished = self.finished.notified();
            tokio::pin!(finished);
            finished.as_mut().enable();
            if self.retired.load(Ordering::Acquire) {
                return false;
            }
            if self.block_sent.load(Ordering::Acquire) {
                return true;
            }
            if !self.running.load(Ordering::Acquire) {
                return false;
            }
            finished.await;
        }
    }

    fn finish_attempt(&self) {
        self.running.store(false, Ordering::Release);
        self.finished.notify_waiters();
    }

    async fn wait_polled_ids(&self) -> Arc<BlockTxids> {
        loop {
            let notified = self.polled_ready.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if let Some(ids) = self.polled_ids.get() {
                return Arc::clone(ids);
            }
            notified.await;
        }
    }

    async fn wait_polled_fees(&self) -> u64 {
        loop {
            let notified = self.polled_ready.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if let Some(&fees) = self.polled_fees.get() {
                return fees;
            }
            notified.await;
        }
    }

    pub(super) async fn accepted_ids(
        &self,
        rpc: &BitcoinRpc,
    ) -> Result<Arc<BlockTxids>, StatsError> {
        let hash = &self.hash;
        let ids = if let Some(cached) =
            self.ids.get().or_else(|| self.polled_ids.get())
        {
            Arc::clone(cached)
        } else {
            tokio::select! {
                res = self.ids.get_or_try_init(|| async {
                    retry_block_rpc(hash, "getblock_txids", || {
                        rpc.get_block_txids(hash)
                    })
                    .await
                    .map(Arc::new)
                }) => Arc::clone(res?),
                polled = self.wait_polled_ids() => {
                    let _ = self.ids.set(Arc::clone(&polled));
                    polled
                }
            }
        };
        self.check_height(rpc, ids.height).await?;
        Ok(ids)
    }

    async fn check_height(
        &self,
        rpc: &BitcoinRpc,
        height: u64,
    ) -> Result<(), StatsError> {
        let canonical = timed_block_rpc(
            &self.hash,
            "getblockhash",
            1,
            rpc.get_block_hash_fresh(height),
        )
        .await?;
        if canonical != self.hash {
            return Err(StatsError::Rpc(
                "block no longer on the active chain".into(),
            ));
        }
        Ok(())
    }
}

/// Coalesce both complete and in-flight work. A failed lookup can be retried
/// by a later source. Saturation never drops IDs from a supposedly exact list.
pub(crate) fn observe_block(
    state: &Arc<StatsState>,
    hash: &str,
    source: &'static str,
) -> Option<Arc<BlockJob>> {
    let Some(job) = state.heartbeat_blocks.entry(hash) else {
        tracing::warn!(%hash, source, "heartbeat block queue full");
        return None;
    };
    if job.retired.load(Ordering::Acquire) {
        return None;
    }
    state.heartbeat_blocks.if_current(&job, || {
        if !job.observed_sent.swap(true, Ordering::Relaxed) {
            state
                .heartbeat_store
                .observe_block(hash.to_string(), job.observed_at);
        }
    })?;
    if job.running.swap(true, Ordering::Acquire) {
        return Some(job);
    }
    if job.retired.load(Ordering::Acquire)
        || job.block_sent.load(Ordering::Relaxed)
    {
        job.finish_attempt();
        return Some(job);
    }
    let Ok(permit) =
        Arc::clone(&state.heartbeat_blocks.permits).try_acquire_owned()
    else {
        job.finish_attempt();
        tracing::warn!(%hash, source, "heartbeat block workers busy");
        return None;
    };
    let _ = state.heartbeat_tx.send(HeartbeatEvent::BlockMining);
    let state = Arc::clone(state);
    let active_job = Arc::clone(&job);
    tokio::spawn(async move {
        let _permit = permit;
        let job = active_job;
        let hash = &job.hash;
        let ids = async {
            let info = job.accepted_ids(&state.rpc).await?;
            state
                .heartbeat_blocks
                .publish_inclusion_if_current(&state, &job, &info);
            // Resolve mining pool from the poller seed if already available, or
            // from the coinbase tx in parallel with `getblockstats`, bounded to
            // 500ms so it never delays block reveal.
            let miner = if let Some(m) = job.polled_miner.get() {
                Some(m.clone())
            } else {
                match info.txids.first() {
                    Some(cb_txid) => tokio::time::timeout(
                        std::time::Duration::from_millis(500),
                        state.rpc.get_raw_transaction(cb_txid),
                    )
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .and_then(|tx| {
                        tx["vin"][0]["coinbase"].as_str().map(|hex| {
                            crate::stats::classifier::identify_miner(hex)
                        })
                    }),
                    None => None,
                }
            };
            Ok::<_, StatsError>((info, miner))
        };
        // Primary: Core's `getblockstats.totalfee` (typically 0.3-1.0s).
        // Fallback: if `getblockstats` stalls (e.g. back-to-back blocks on
        // Start9) and the 15s poller has already parsed exact per-tx fees from
        // `getblock(hash, 2)`, unblock immediately so live and refreshed views
        // stay in sync with zero coinbase-minus-subsidy drift.
        let fees = async {
            if let Some(&polled) = job.polled_fees.get() {
                return Ok::<u64, StatsError>(polled);
            }
            tokio::select! {
                res = retry_block_rpc(hash, "getblockstats", || {
                    state.rpc.get_block_fees(hash)
                }) => res,
                polled = job.wait_polled_fees() => {
                    tracing::info!(
                        source,
                        stage = "fees_polled_fallback",
                        %hash,
                        elapsed_ms = job.first_receipt.elapsed().as_millis(),
                        "heartbeat_block"
                    );
                    Ok(polled)
                }
            }
        };
        let (ids_res, fees) = tokio::join!(ids, fees);
        let result = async {
            let ((info, miner), total_fees) = (ids_res?, fees?);
            job.check_height(&state.rpc, info.height).await?;
            state.heartbeat_blocks.if_current(&job, || {
                state.heartbeat_store.apply_block_details(CanonicalBlock {
                    height: info.height,
                    hash: hash.clone(),
                    timestamp: info.timestamp,
                    tx_count: info.tx_count,
                    total_fees,
                    size: info.size,
                    weight: info.weight,
                    observed_at: job.observed_at,
                    miner,
                });
                let event = HeartbeatEvent::Block {
                    height: info.height,
                    hash: hash.clone(),
                    timestamp: info.timestamp,
                    tx_count: info.tx_count,
                    size: info.size,
                    weight: info.weight,
                    total_fees,
                    confirmed_count: 0,
                };
                let receivers = state.heartbeat_tx.send(event).unwrap_or(0);
                tracing::info!(source, stage = "block", %hash, height = info.height, elapsed_ms = job.first_receipt.elapsed().as_millis(), receivers, "heartbeat_block");
                job.block_sent.store(true, Ordering::Release);
            });
            Ok::<_, StatsError>(())
        }.await;
        if let Err(e) = result {
            tracing::warn!(source, %hash, "heartbeat block details unavailable: {e}");
        }
        job.finish_attempt();
    });
    Some(job)
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
