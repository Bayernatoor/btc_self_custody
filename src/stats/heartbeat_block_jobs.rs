//! A poll, sequence C and hashblock can all be first. Share the immutable
//! payload by hash; acceptance is mutable and must be checked at publication.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::{Notify, OnceCell, Semaphore};

use super::{timed_block_rpc, HeartbeatEvent, StatsError, StatsState};
use crate::stats::{
    db,
    rpc::{BitcoinRpc, BlockTxids},
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
    ids: OnceCell<Arc<BlockTxids>>,
    running: AtomicBool,
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
            .filter_map(|job| Some((job.ids.get()?.height, job.hash.clone())))
            .collect()
    }

    /// Preserve first observation time while the poller's DB payload is in flight.
    pub(crate) fn reserve(&self, hash: &str) {
        let _ = self.entry(hash);
    }

    pub(crate) fn seed_polled(&self, info: BlockTxids) {
        if let Some(job) = self.entry(&info.hash) {
            let _ = job.ids.set(Arc::new(info));
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
            ids: OnceCell::new(),
            running: false.into(),
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

    fn retire(
        &self,
        hash: &str,
        pool: &db::DbPool,
        events: &tokio::sync::broadcast::Sender<HeartbeatEvent>,
    ) {
        self.on_disconnect(hash, || {
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
}

/// Publish D immediately and retire the matching durable claims, including
/// hashes no longer in the bounded job cache (or confirmed before restart).
pub(crate) fn retire_block(state: &Arc<StatsState>, hash: &str) {
    state
        .heartbeat_blocks
        .retire(hash, &state.db, &state.heartbeat_tx);
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

    pub(super) async fn accepted_ids(
        &self,
        rpc: &BitcoinRpc,
    ) -> Result<Arc<BlockTxids>, StatsError> {
        let hash = &self.hash;
        let ids = self
            .ids
            .get_or_try_init(|| async {
                retry_block_rpc(hash, "getblock_txids", || {
                    rpc.get_block_txids(hash)
                })
                .await
                .map(Arc::new)
            })
            .await?;
        self.check_height(rpc, ids.height).await?;
        Ok(Arc::clone(ids))
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
            state.heartbeat_blocks.if_current(&job, || {
                if job.ids_sent.swap(true, Ordering::Relaxed) {
                    return;
                }
                state.rpc.invalidate_tip_caches(info.height);
                let event = HeartbeatEvent::BlockTxids {
                    hash: hash.clone(),
                    height: info.height,
                    txids: info.txids.clone(),
                };
                let receivers = state.heartbeat_tx.send(event).unwrap_or(0);
                tracing::info!(source, stage = "block_txids", %hash, height = info.height, txids = info.txids.len(), receivers, "heartbeat_block");
                state.heartbeat_blocks.persist(
                    &state.db,
                    ClaimWrite::Confirm {
                        hash: hash.clone(),
                        retired: Arc::clone(&job.retired),
                        ids: Arc::clone(&info),
                    },
                );
            });
            Ok::<_, StatsError>(info)
        };
        // Always use Core's getblockstats.totalfee so polled and ZMQ-observed
        // live blocks agree on total_fees (avoiding coinbase-minus-subsidy drift).
        let fees = retry_block_rpc(hash, "getblockstats", || {
            state.rpc.get_block_fees(hash)
        });
        let (info, fees) = tokio::join!(ids, fees);
        let result = async {
            let (info, total_fees) = (info?, fees?);
            job.check_height(&state.rpc, info.height).await?;
            state.heartbeat_blocks.if_current(&job, || {
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
