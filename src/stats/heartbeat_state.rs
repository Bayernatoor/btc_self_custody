//! SSR-only authoritative in-memory state owner for Heartbeat Phase 2 sync.
//!
//! [`HeartbeatStore`] wraps [`HeartbeatMirror`] behind a fast in-memory
//! `std::sync::RwLock` so the server and every connected client run the exact
//! same deterministic state reducer. The lock is strictly in-memory: it is
//! never held across RPC, SQLite, network I/O, or snapshot JSON serialization.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use tokio::sync::broadcast;
use uuid::Uuid;

use super::heartbeat_sync::{
    ApplyOutcome, CanonicalBlock, CompactTx, Coverage, DeltaKind,
    HeartbeatDelta, HeartbeatMirror, HeartbeatSnapshot, NodeMempoolAggregate,
    ResyncRequired, TxLookupStatus, TxMetadataHydration, PROTOCOL_VERSION,
};

/// Maximum number of sequenced deltas retained in the in-memory replay buffer.
pub const MAX_REPLAY_EVENTS: usize = 10_000;

/// Approximate memory cap (in bytes) for the in-memory replay buffer so a burst
/// of large `BlockInclusion` frames cannot exhaust droplet RAM.
pub const MAX_REPLAY_BYTES: usize = 16 * 1024 * 1024;

/// Capacity of the live broadcast channel fanning deltas and resync signals out
/// to connected SSE subscribers.
pub const LIVE_CHANNEL_CAPACITY: usize = 4_096;

/// Maximum combined `added + removed` count applied incrementally on a
/// checkpoint before rotating `stream_epoch` and requesting a full resync.
pub const CHECKPOINT_RESET_DIFF_THRESHOLD: usize = 1_000;

/// Maximum age of a cached serialized snapshot while its `seq` remains inside
/// the replay window.
pub const SNAPSHOT_CACHE_TTL: Duration = Duration::from_secs(2);

/// Event fanned out to connected `/heartbeat/v2` SSE streams.
#[derive(Clone, Debug)]
pub enum StoreEvent {
    Delta(Arc<HeartbeatDelta>),
    Resync(Arc<ResyncRequired>),
}

/// Immutable serialized snapshot payload shared across concurrent visitors.
#[derive(Clone, Debug)]
pub struct SerializedSnapshot {
    pub snapshot_id: String,
    pub stream_epoch: String,
    pub seq: u64,
    pub json: Bytes,
    pub built_at: Instant,
}

/// Result of an atomic replay-to-live subscription attempt.
#[derive(Debug)]
pub enum SubscribeResult {
    Stream {
        replay: Vec<Arc<HeartbeatDelta>>,
        live_rx: broadcast::Receiver<Arc<StoreEvent>>,
        last_replayed: u64,
    },
    Resync(ResyncRequired),
}

struct ReplayEntry {
    delta: Arc<HeartbeatDelta>,
    approx_bytes: usize,
}

struct StoreInner {
    mirror: HeartbeatMirror,
    stream_epoch: String,
    seq: u64,
    initialized: bool,
    node_observed_at: Option<u64>,
    replay: VecDeque<ReplayEntry>,
    replay_bytes: usize,
}

/// Authoritative server-side owner of Heartbeat v2 state, replay buffer,
/// serialized snapshot cache, and live event fan-out.
pub struct HeartbeatStore {
    inner: RwLock<StoreInner>,
    /// Separate lock from `inner` so snapshot serialization runs completely
    /// outside any lock. Whenever both locks are acquired, `inner` is always
    /// locked before `snapshot_cache`.
    snapshot_cache: RwLock<Option<Arc<SerializedSnapshot>>>,
    /// Coalesces concurrent async snapshot cache-miss builds so a thundering
    /// herd after `resync_required` serializes the snapshot once. Never held
    /// while waiting on another async lock; `inner` and `snapshot_cache` are
    /// only briefly locked inside it.
    snapshot_build_lock: tokio::sync::Mutex<()>,
    live_tx: broadcast::Sender<Arc<StoreEvent>>,
    max_replay_events: usize,
    max_replay_bytes: usize,
}

impl Default for HeartbeatStore {
    fn default() -> Self {
        Self::new()
    }
}

impl HeartbeatStore {
    /// Construct a fresh store in the `Unknown("uninitialized")` state at `seq = 0`.
    pub fn new() -> Self {
        Self::with_replay_limits(MAX_REPLAY_EVENTS, MAX_REPLAY_BYTES)
    }

    /// Construct a store with custom replay bounds (used by tests to exercise
    /// event-count and byte-budget eviction without large allocations).
    pub(crate) fn with_replay_limits(
        max_replay_events: usize,
        max_replay_bytes: usize,
    ) -> Self {
        let stream_epoch = Uuid::new_v4().to_string();
        let now = now_unix_secs();
        let mut mirror = HeartbeatMirror::default();
        mirror
            .install_snapshot(HeartbeatSnapshot {
                protocol_version: PROTOCOL_VERSION,
                stream_epoch: stream_epoch.clone(),
                snapshot_id: Uuid::new_v4().to_string(),
                seq: 0,
                generated_at: now,
                node_observed_at: None,
                coverage: Coverage::default(),
                node_mempool: None,
                pending_blocks: Vec::new(),
                recent_blocks: Vec::new(),
                recent_confirmations: Vec::new(),
                transactions: Vec::new(),
            })
            .expect("initial HeartbeatSnapshot is always valid");

        let (live_tx, _) = broadcast::channel(LIVE_CHANNEL_CAPACITY);

        Self {
            inner: RwLock::new(StoreInner {
                mirror,
                stream_epoch,
                seq: 0,
                initialized: false,
                node_observed_at: None,
                replay: VecDeque::new(),
                replay_bytes: 0,
            }),
            snapshot_cache: RwLock::new(None),
            snapshot_build_lock: tokio::sync::Mutex::new(()),
            live_tx,
            max_replay_events,
            max_replay_bytes,
        }
    }

    fn read_inner(&self) -> RwLockReadGuard<'_, StoreInner> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write_inner(&self) -> RwLockWriteGuard<'_, StoreInner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }

    fn read_snapshot_cache(
        &self,
    ) -> RwLockReadGuard<'_, Option<Arc<SerializedSnapshot>>> {
        self.snapshot_cache
            .read()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn write_snapshot_cache(
        &self,
    ) -> RwLockWriteGuard<'_, Option<Arc<SerializedSnapshot>>> {
        self.snapshot_cache
            .write()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Return the current `(stream_epoch, seq)` revision.
    pub fn current_revision(&self) -> (String, u64) {
        let inner = self.read_inner();
        (inner.stream_epoch.clone(), inner.seq)
    }

    /// Whether at least one authoritative checkpoint has been installed.
    pub fn is_initialized(&self) -> bool {
        self.read_inner().initialized
    }

    /// True when at least one pending block was observed within `max_wait_secs`
    /// and has not yet received its `BlockInclusion` delta (`height.is_none()`).
    ///
    /// Used by the shadow sequence task to avoid running a `getrawmempool`
    /// checkpoint ahead of an in-flight `getblock_txids` call, which would
    /// otherwise strip mined transactions via `checkpoint_reset` before
    /// `apply_block_inclusion` can record their confirmations and animate their
    /// absorption into the block spike.
    pub fn has_pending_inclusion(
        &self,
        now_secs: u64,
        max_wait_secs: u64,
    ) -> bool {
        let inner = self.read_inner();
        inner.mirror.pending_blocks().iter().any(|b| {
            b.height.is_none()
                && now_secs.saturating_sub(b.observed_at) < max_wait_secs
        })
    }

    /// Look up a single transaction's status against current mempool membership
    /// and retained recent confirmations.
    pub fn lookup_tx(&self, txid: &str) -> TxLookupStatus {
        self.read_inner().mirror.lookup_tx(txid)
    }

    fn can_reuse_cached(
        cached: &SerializedSnapshot,
        inner: &StoreInner,
    ) -> bool {
        if cached.stream_epoch != inner.stream_epoch {
            return false;
        }
        if cached.built_at.elapsed() >= SNAPSHOT_CACHE_TTL {
            return false;
        }
        if cached.seq == inner.seq {
            return true;
        }
        if cached.seq > inner.seq {
            return false;
        }
        let Some(first_needed) = cached.seq.checked_add(1) else {
            return false;
        };
        inner
            .replay
            .front()
            .is_some_and(|entry| entry.delta.seq <= first_needed)
    }

    fn cached_snapshot(&self) -> Option<Arc<SerializedSnapshot>> {
        let inner = self.read_inner();
        let cache = self.read_snapshot_cache();
        cache
            .as_ref()
            .filter(|cached| Self::can_reuse_cached(cached, &inner))
            .map(Arc::clone)
    }

    fn clone_snapshot_for_serialization(&self) -> HeartbeatSnapshot {
        let inner = self.read_inner();
        inner
            .mirror
            .to_snapshot(
                Uuid::new_v4().to_string(),
                now_unix_secs(),
                inner.node_observed_at,
            )
            .expect("HeartbeatStore mirror is always initialized")
    }

    fn serialize_snapshot(
        snapshot: HeartbeatSnapshot,
    ) -> Arc<SerializedSnapshot> {
        let json_vec = serde_json::to_vec(&snapshot)
            .expect("HeartbeatSnapshot serialization is infallible");
        Arc::new(SerializedSnapshot {
            snapshot_id: snapshot.snapshot_id,
            stream_epoch: snapshot.stream_epoch,
            seq: snapshot.seq,
            json: Bytes::from(json_vec),
            built_at: Instant::now(),
        })
    }

    fn store_snapshot_if_usable(&self, serialized: &Arc<SerializedSnapshot>) {
        let inner = self.read_inner();
        if Self::can_reuse_cached(serialized, &inner) {
            let mut cache = self.write_snapshot_cache();
            let should_store = cache.as_ref().is_none_or(|existing| {
                !Self::can_reuse_cached(existing, &inner)
                    || existing.seq < serialized.seq
            });
            if should_store {
                *cache = Some(Arc::clone(serialized));
            }
        }
    }

    /// Return a shared immutable serialized snapshot, coalescing concurrent
    /// cache-miss rebuilds and running JSON serialization on `spawn_blocking`
    /// so Tokio worker threads never block on large mempool snapshots.
    ///
    /// A cached snapshot is reused across subsequent deltas while its
    /// `stream_epoch` matches, its age is within [`SNAPSHOT_CACHE_TTL`], and
    /// its `seq` remains inside the replay window so `/heartbeat/v2` can catch
    /// callers up seamlessly.
    pub async fn snapshot_async(&self) -> Arc<SerializedSnapshot> {
        if let Some(cached) = self.cached_snapshot() {
            return cached;
        }

        let _build_guard = self.snapshot_build_lock.lock().await;
        if let Some(cached) = self.cached_snapshot() {
            return cached;
        }

        let snapshot = self.clone_snapshot_for_serialization();
        let serialized = tokio::task::spawn_blocking(move || {
            Self::serialize_snapshot(snapshot)
        })
        .await
        .expect("snapshot serialization task panicked");

        self.store_snapshot_if_usable(&serialized);
        serialized
    }

    /// Synchronous snapshot helper used by non-async tests.
    pub fn snapshot(&self) -> Arc<SerializedSnapshot> {
        if let Some(cached) = self.cached_snapshot() {
            return cached;
        }

        let snapshot = self.clone_snapshot_for_serialization();
        let serialized = Self::serialize_snapshot(snapshot);
        self.store_snapshot_if_usable(&serialized);
        serialized
    }

    /// Atomically subscribe to live events and collect any missing replay
    /// deltas after `(epoch, after)`.
    ///
    /// Subscribes to `live_tx` before reading `replay` while holding the read
    /// lock so no delta published concurrently can slip between replay and live
    /// handoff.
    pub fn subscribe_from(
        &self,
        epoch: Option<&str>,
        after: Option<u64>,
    ) -> SubscribeResult {
        let inner = self.read_inner();
        let live_rx = self.live_tx.subscribe();

        if epoch != Some(inner.stream_epoch.as_str()) {
            return SubscribeResult::Resync(ResyncRequired {
                protocol_version: PROTOCOL_VERSION,
                stream_epoch: inner.stream_epoch.clone(),
                reason: "epoch_mismatch".to_string(),
                latest_seq: inner.seq,
            });
        }

        let Some(cursor) = after else {
            return SubscribeResult::Resync(ResyncRequired {
                protocol_version: PROTOCOL_VERSION,
                stream_epoch: inner.stream_epoch.clone(),
                reason: "missing_cursor".to_string(),
                latest_seq: inner.seq,
            });
        };

        if cursor == inner.seq {
            return SubscribeResult::Stream {
                replay: Vec::new(),
                live_rx,
                last_replayed: cursor,
            };
        }

        if cursor > inner.seq {
            return SubscribeResult::Resync(ResyncRequired {
                protocol_version: PROTOCOL_VERSION,
                stream_epoch: inner.stream_epoch.clone(),
                reason: "cursor_ahead".to_string(),
                latest_seq: inner.seq,
            });
        }

        let Some(first_needed) = cursor.checked_add(1) else {
            return SubscribeResult::Resync(ResyncRequired {
                protocol_version: PROTOCOL_VERSION,
                stream_epoch: inner.stream_epoch.clone(),
                reason: "cursor_expired".to_string(),
                latest_seq: inner.seq,
            });
        };

        let Some(front) = inner.replay.front() else {
            return SubscribeResult::Resync(ResyncRequired {
                protocol_version: PROTOCOL_VERSION,
                stream_epoch: inner.stream_epoch.clone(),
                reason: "cursor_expired".to_string(),
                latest_seq: inner.seq,
            });
        };

        if front.delta.seq > first_needed {
            return SubscribeResult::Resync(ResyncRequired {
                protocol_version: PROTOCOL_VERSION,
                stream_epoch: inner.stream_epoch.clone(),
                reason: "cursor_expired".to_string(),
                latest_seq: inner.seq,
            });
        }

        let start_idx = (first_needed - front.delta.seq) as usize;
        let replay: Vec<Arc<HeartbeatDelta>> = inner
            .replay
            .iter()
            .skip(start_idx)
            .map(|entry| Arc::clone(&entry.delta))
            .collect();

        debug_assert_eq!(replay.first().map(|d| d.seq), Some(first_needed));
        debug_assert_eq!(replay.last().map(|d| d.seq), Some(inner.seq));

        SubscribeResult::Stream {
            replay,
            live_rx,
            last_replayed: inner.seq,
        }
    }

    /// Admit a mempool transaction observed via ZMQ `sequence` `A`.
    pub fn observe_add(&self, txid: String, first_seen: u64) {
        let mut inner = self.write_inner();
        if inner.mirror.has_tx(&txid) {
            return;
        }
        self.emit_delta_locked(
            &mut inner,
            first_seen,
            DeltaKind::MembershipAdd { txid, first_seen },
        );
    }

    /// Remove a batch of non-block departed transactions observed via ZMQ
    /// `sequence` `R`. Skips empty batches or txids not currently tracked.
    pub fn observe_remove(&self, mut txids: Vec<String>) {
        if txids.is_empty() {
            return;
        }
        let mut inner = self.write_inner();
        txids.retain(|txid| inner.mirror.has_tx(txid));
        if txids.is_empty() {
            return;
        }
        let now = now_unix_secs();
        self.emit_delta_locked(
            &mut inner,
            now,
            DeltaKind::MembershipRemove { txids },
        );
    }

    /// Attach fee/vsize/value/notable details from ZMQ `rawtx`.
    ///
    /// Strictly details-only: if `txid` is not currently a member, this is a
    /// no-op so late or orphaned `rawtx` frames never admit or resurrect a tx.
    pub fn enrich_tx(
        &self,
        txid: String,
        fee: u64,
        vsize: u32,
        value: u64,
        flags: u8,
        op_return_text: Option<String>,
    ) {
        let mut inner = self.write_inner();
        let Some(existing) = inner.mirror.get_tx(&txid) else {
            return;
        };
        if existing.fee == Some(fee)
            && existing.vsize == Some(vsize)
            && existing.value == Some(value)
            && (existing.flags | flags) == existing.flags
            && (op_return_text.is_none()
                || existing.op_return_text == op_return_text)
        {
            return;
        }
        let now = now_unix_secs();
        self.emit_delta_locked(
            &mut inner,
            now,
            DeltaKind::TxDetails {
                txid,
                fee,
                vsize,
                value: Some(value),
                flags,
                op_return_text,
                first_seen: None,
            },
        );
    }

    /// Record a newly observed block hash before its inclusion list or summary
    /// lands. Idempotent for hashes already pending or canonical.
    pub fn observe_block(&self, hash: String, observed_at: u64) {
        let mut inner = self.write_inner();
        if inner.mirror.pending_blocks().iter().any(|b| b.hash == hash)
            || inner.mirror.recent_blocks().iter().any(|b| b.hash == hash)
        {
            return;
        }
        self.emit_delta_locked(
            &mut inner,
            observed_at,
            DeltaKind::BlockObserved { hash, observed_at },
        );
    }

    /// Publish exact transaction inclusion for an accepted block hash.
    pub fn apply_block_inclusion(
        &self,
        hash: String,
        height: u64,
        txids: Vec<String>,
    ) {
        let mut inner = self.write_inner();
        let now = now_unix_secs();
        self.emit_delta_locked(
            &mut inner,
            now,
            DeltaKind::BlockInclusion {
                hash,
                height,
                txids,
            },
        );
    }

    /// Publish canonical block measurements once `getblock` and `getblockstats`
    /// have completed.
    pub fn apply_block_details(&self, block: CanonicalBlock) {
        let mut inner = self.write_inner();
        let observed_at = now_unix_secs();
        self.emit_delta_locked(
            &mut inner,
            observed_at,
            DeltaKind::BlockDetails(block),
        );
    }

    /// Retract a disconnected block hash (reorg).
    pub fn disconnect_block(&self, hash: String) {
        let mut inner = self.write_inner();
        let now = now_unix_secs();
        self.emit_delta_locked(
            &mut inner,
            now,
            DeltaKind::BlockDisconnected { hash },
        );
    }

    /// Update mempool coverage status, emitting only when the status changes.
    pub fn set_coverage(&self, coverage: Coverage) {
        let mut inner = self.write_inner();
        if *inner.mirror.coverage() == coverage {
            return;
        }
        let now = now_unix_secs();
        self.emit_delta_locked(
            &mut inner,
            now,
            DeltaKind::CoverageChanged { coverage },
        );
    }

    /// Update node-reported aggregate mempool counters, skipping unchanged
    /// `(tx_count, vbytes, stale)` readings so frequent polling does not spam
    /// `MetricsUpdated` deltas.
    pub fn update_node_mempool(&self, agg: Option<NodeMempoolAggregate>) {
        let mut inner = self.write_inner();
        let unchanged = match (inner.mirror.node_mempool(), agg.as_ref()) {
            (None, None) => true,
            (Some(prev), Some(next)) => {
                prev.tx_count == next.tx_count
                    && prev.vbytes == next.vbytes
                    && prev.stale == next.stale
            }
            _ => false,
        };
        if unchanged {
            return;
        }
        let observed_at =
            agg.as_ref().map_or_else(now_unix_secs, |a| a.observed_at);
        if let Some(a) = &agg {
            inner.node_observed_at = Some(a.observed_at);
        }
        self.emit_delta_locked(
            &mut inner,
            observed_at,
            DeltaKind::MetricsUpdated { node_mempool: agg },
        );
    }

    /// Reconcile the store against an authoritative membership checkpoint.
    ///
    /// On initial bootstrap or when the symmetric difference exceeds
    /// [`CHECKPOINT_RESET_DIFF_THRESHOLD`], atomically replaces membership
    /// (preserving enriched details on surviving txids), rotates
    /// `stream_epoch`, clears the replay log, resets `seq = 1`, invalidates the
    /// snapshot cache, and notifies live subscribers with `ResyncRequired`.
    ///
    /// Otherwise emits minimal incremental `MembershipRemove`, `MembershipAdd`,
    /// and `CoverageChanged` deltas so connected clients stay synced without a
    /// reconnect.
    pub fn install_checkpoint(
        &self,
        members: &[&String],
        complete: bool,
        observed_at: u64,
    ) {
        self.install_checkpoint_with_hydration(
            members,
            complete,
            observed_at,
            &[],
        );
    }

    /// Reconcile the store against an authoritative membership checkpoint,
    /// applying optional details-only `initial_hydration` entries onto the
    /// checkpoint snapshot before installing and publishing on the reset path.
    pub fn install_checkpoint_with_hydration(
        &self,
        members: &[&String],
        complete: bool,
        observed_at: u64,
        initial_hydration: &[TxMetadataHydration],
    ) {
        let target_coverage = if complete {
            Coverage::Complete
        } else {
            Coverage::Partial {
                reason: "reconciling".to_string(),
            }
        };

        let mut inner = self.write_inner();

        let incoming_set: HashSet<&str> =
            members.iter().map(|s| s.as_str()).collect();

        let mut removed = Vec::new();
        for (txid, _) in inner.mirror.members() {
            if !incoming_set.contains(txid) {
                removed.push(txid.to_string());
            }
        }

        let mut added = Vec::new();
        for &txid in &incoming_set {
            if !inner.mirror.has_tx(txid) {
                added.push(txid.to_string());
            }
        }
        added.sort_unstable();

        let total_diff = added.len().saturating_add(removed.len());
        let must_reset =
            !inner.initialized || total_diff > CHECKPOINT_RESET_DIFF_THRESHOLD;

        if must_reset {
            let was_initial = !inner.initialized;
            let new_epoch = Uuid::new_v4().to_string();

            let mut snapshot = inner
                .mirror
                .to_snapshot(
                    Uuid::new_v4().to_string(),
                    now_unix_secs(),
                    Some(observed_at),
                )
                .expect("HeartbeatStore mirror is always initialized");
            snapshot.stream_epoch = new_epoch.clone();
            snapshot.seq = 1;
            snapshot.coverage = target_coverage;
            snapshot.node_observed_at = Some(observed_at);

            let mut surviving_by_id: HashMap<String, CompactTx> = snapshot
                .transactions
                .into_iter()
                .filter(|tx| incoming_set.contains(tx.txid.as_str()))
                .map(|tx| (tx.txid.clone(), tx))
                .collect();

            let mut tx_by_id: HashMap<&str, CompactTx> =
                HashMap::with_capacity(incoming_set.len());
            for &txid in &incoming_set {
                let tx = surviving_by_id
                    .remove(txid)
                    .unwrap_or_else(|| CompactTx::pending(txid.to_string(), 0));
                tx_by_id.insert(txid, tx);
            }
            for entry in initial_hydration {
                if let Some(tx) = tx_by_id.get_mut(entry.txid.as_str()) {
                    tx.apply_hydration(entry);
                }
            }
            snapshot.transactions = tx_by_id.into_values().collect();

            inner
                .mirror
                .install_snapshot(snapshot)
                .expect("generated checkpoint snapshot is valid");
            inner.stream_epoch = new_epoch.clone();
            inner.seq = 1;
            inner.initialized = true;
            inner.node_observed_at = Some(observed_at);
            inner.replay.clear();
            inner.replay_bytes = 0;

            *self.write_snapshot_cache() = None;

            let resync = Arc::new(ResyncRequired {
                protocol_version: PROTOCOL_VERSION,
                stream_epoch: new_epoch,
                reason: if was_initial {
                    "initial_checkpoint".to_string()
                } else {
                    "checkpoint_reset".to_string()
                },
                latest_seq: 1,
            });
            let _ = self.live_tx.send(Arc::new(StoreEvent::Resync(resync)));
            return;
        }

        inner.node_observed_at = Some(observed_at);

        if !removed.is_empty() {
            self.emit_delta_locked(
                &mut inner,
                observed_at,
                DeltaKind::MembershipRemove { txids: removed },
            );
        }

        for txid in added {
            self.emit_delta_locked(
                &mut inner,
                observed_at,
                DeltaKind::MembershipAdd {
                    txid,
                    first_seen: 0,
                },
            );
        }

        if *inner.mirror.coverage() != target_coverage {
            self.emit_delta_locked(
                &mut inner,
                observed_at,
                DeltaKind::CoverageChanged {
                    coverage: target_coverage,
                },
            );
        }
    }

    /// Return up to `max_count` mempool member txids that are still missing
    /// `fee`/`vsize` details and have been waiting at least `min_age_secs`
    /// (giving the normal ZMQ `rawtx` enrichment path time to arrive first).
    ///
    /// Txids present in `exclude` (for example entries currently backing off
    /// after an RPC error) are skipped *before* applying `max_count` so a
    /// handful of unresolvable txids can never starve other pending members.
    pub fn unenriched_txids(
        &self,
        max_count: usize,
        min_age_secs: u64,
        now_secs: u64,
        exclude: &HashSet<String>,
    ) -> Vec<String> {
        if max_count == 0 {
            return Vec::new();
        }
        let inner = self.read_inner();
        if !inner.initialized || inner.mirror.pending_details_count() == 0 {
            return Vec::new();
        }
        inner
            .mirror
            .members()
            .filter(|(txid, tx)| {
                !exclude.contains(*txid)
                    && (tx.fee.is_none() || tx.vsize.is_none())
                    && (tx.first_seen == 0
                        || now_secs.saturating_sub(tx.first_seen)
                            >= min_age_secs)
            })
            .take(max_count)
            .map(|(txid, _)| txid.to_string())
            .collect()
    }

    /// Details-only bulk metadata hydration for existing mempool members
    /// (for example from SQLite history or `getrawmempool(true)`).
    ///
    /// Strictly details-only: never admits membership for unknown txids.
    /// Returns the number of distinct members whose metadata changed.
    ///
    /// Small updates (`<= 256` members on an initialized store whose updated
    /// members all carry `fee` and `vsize`) emit incremental
    /// `DeltaKind::TxDetails` frames without rotating `stream_epoch`.
    /// Larger bulk hydrations (`> 256` members) on an initialized store update
    /// the mirror in place, clear `snapshot_cache`, and always rotate
    /// `stream_epoch` (`seq = 1`) with
    /// `ResyncRequired { reason: "metadata_hydrated" }` so resuming clients
    /// cannot replay across an unlogged bulk mutation.
    pub fn hydrate_details(&self, entries: &[TxMetadataHydration]) -> usize {
        if entries.is_empty() {
            return 0;
        }

        let mut inner = self.write_inner();
        let mut updated_txids = Vec::new();
        let mut seen_updated = HashSet::new();

        for entry in entries {
            if inner.mirror.hydrate_member_details(entry)
                && seen_updated.insert(entry.txid.clone())
            {
                updated_txids.push(entry.txid.clone());
            }
        }

        if updated_txids.is_empty() {
            return 0;
        }

        let updated_count = updated_txids.len();
        let can_emit_deltas = inner.initialized
            && updated_count <= 256
            && updated_txids.iter().all(|txid| {
                inner
                    .mirror
                    .get_tx(txid)
                    .is_some_and(|tx| tx.fee.is_some() && tx.vsize.is_some())
            });

        if can_emit_deltas {
            let now = now_unix_secs();
            for txid in updated_txids {
                if let Some(tx) = inner.mirror.get_tx(&txid).cloned() {
                    if let (Some(fee), Some(vsize)) = (tx.fee, tx.vsize) {
                        self.emit_delta_locked(
                            &mut inner,
                            now,
                            DeltaKind::TxDetails {
                                txid,
                                fee,
                                vsize,
                                value: tx.value,
                                flags: tx.flags,
                                op_return_text: tx.op_return_text,
                                first_seen: (tx.first_seen > 0)
                                    .then_some(tx.first_seen),
                            },
                        );
                    }
                }
            }
            *self.write_snapshot_cache() = None;
            return updated_count;
        }

        *self.write_snapshot_cache() = None;
        if inner.initialized {
            let new_epoch = Uuid::new_v4().to_string();
            inner.mirror.reset_revision(new_epoch.clone(), 1);
            inner.stream_epoch = new_epoch.clone();
            inner.seq = 1;
            inner.replay.clear();
            inner.replay_bytes = 0;

            let resync = Arc::new(ResyncRequired {
                protocol_version: PROTOCOL_VERSION,
                stream_epoch: new_epoch,
                reason: "metadata_hydrated".to_string(),
                latest_seq: 1,
            });
            let _ = self.live_tx.send(Arc::new(StoreEvent::Resync(resync)));
        }

        updated_count
    }

    fn emit_delta_locked(
        &self,
        inner: &mut StoreInner,
        observed_at: u64,
        kind: DeltaKind,
    ) {
        let next_seq = inner
            .seq
            .checked_add(1)
            .expect("heartbeat store sequence number exhausted");

        let delta = HeartbeatDelta {
            protocol_version: PROTOCOL_VERSION,
            stream_epoch: inner.stream_epoch.clone(),
            seq: next_seq,
            observed_at,
            kind,
        };

        let outcome = inner.mirror.apply_delta(delta.clone());
        debug_assert_eq!(
            outcome,
            ApplyOutcome::Applied,
            "server mirror must always apply its own sequenced delta"
        );
        inner.seq = next_seq;

        let approx_bytes = estimate_delta_bytes(&delta);
        let arc_delta = Arc::new(delta);

        inner.replay.push_back(ReplayEntry {
            delta: Arc::clone(&arc_delta),
            approx_bytes,
        });
        inner.replay_bytes = inner.replay_bytes.saturating_add(approx_bytes);

        while !inner.replay.is_empty()
            && (inner.replay.len() > self.max_replay_events
                || inner.replay_bytes > self.max_replay_bytes)
        {
            if let Some(evicted) = inner.replay.pop_front() {
                inner.replay_bytes =
                    inner.replay_bytes.saturating_sub(evicted.approx_bytes);
            }
        }

        // Send while holding the write lock so `subscribe_from` (which takes
        // the read lock and calls `live_tx.subscribe()` first) sees a strict
        // total order between `replay` and `live_rx`.
        let _ = self.live_tx.send(Arc::new(StoreEvent::Delta(arc_delta)));
    }
}

fn estimate_delta_bytes(delta: &HeartbeatDelta) -> usize {
    let base = std::mem::size_of::<HeartbeatDelta>() + delta.stream_epoch.len();
    let payload = match &delta.kind {
        DeltaKind::MembershipAdd { txid, .. } => {
            txid.capacity().max(txid.len()) + 24
        }
        DeltaKind::MembershipRemove { txids } => {
            txids.capacity() * std::mem::size_of::<String>()
                + txids
                    .iter()
                    .map(|s| s.capacity().max(s.len()))
                    .sum::<usize>()
        }
        DeltaKind::TxDetails {
            txid,
            op_return_text,
            ..
        } => {
            txid.capacity().max(txid.len())
                + 24
                + op_return_text
                    .as_ref()
                    .map_or(0, |s| s.capacity().max(s.len()) + 24)
        }
        DeltaKind::BlockObserved { hash, .. } => {
            hash.capacity().max(hash.len()) + 24
        }
        DeltaKind::BlockDetails(block) => {
            block.hash.capacity().max(block.hash.len()) + 24
        }
        DeltaKind::BlockInclusion { hash, txids, .. } => {
            hash.capacity().max(hash.len())
                + 24
                + txids.capacity() * std::mem::size_of::<String>()
                + txids
                    .iter()
                    .map(|s| s.capacity().max(s.len()))
                    .sum::<usize>()
        }
        DeltaKind::BlockDisconnected { hash } => {
            hash.capacity().max(hash.len()) + 24
        }
        DeltaKind::CoverageChanged { coverage } => {
            coverage.reason().map_or(0, |r| r.len() + 24)
        }
        DeltaKind::MetricsUpdated { .. } => 0,
    };
    base + payload
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::heartbeat_sync::TxFlags;

    fn hex_id(seed: u64) -> String {
        format!("{seed:064x}")
    }

    #[test]
    fn enrich_tx_never_admits_non_member() {
        let store = HeartbeatStore::new();
        let txid = hex_id(1);
        let (_, seq_before) = store.current_revision();

        store.enrich_tx(
            txid.clone(),
            1_000,
            141,
            50_000,
            TxFlags::WHALE,
            Some("ghost".to_string()),
        );

        let (_, seq_after) = store.current_revision();
        assert_eq!(seq_before, seq_after);
        assert!(matches!(
            store.lookup_tx(&txid),
            TxLookupStatus::Absent { .. }
        ));
    }

    #[test]
    fn snapshot_cache_reuses_within_replay_window_and_invalidates_on_expiry_or_epoch(
    ) {
        // Replay cap = 2 so we can test both in-window reuse and replay expiry.
        let store = HeartbeatStore::with_replay_limits(2, MAX_REPLAY_BYTES);
        let s1 = store.snapshot();
        let s2 = store.snapshot();
        assert!(Arc::ptr_eq(&s1, &s2));
        assert_eq!(s1.json.as_ptr(), s2.json.as_ptr());

        // Subsequent deltas within the replay window and TTL reuse `s1`,
        // because a client fetching `s1` can replay from `s1.seq` via `/v2`.
        store.observe_add(hex_id(1), 1_700_000_001);
        store.observe_add(hex_id(2), 1_700_000_002);
        let s3 = store.snapshot();
        assert!(Arc::ptr_eq(&s1, &s3));
        assert!(matches!(
            store.subscribe_from(Some(&s3.stream_epoch), Some(s3.seq)),
            SubscribeResult::Stream { ref replay, .. } if replay.len() == 2
        ));

        // A third delta evicts `s1.seq + 1` from the 2-event replay buffer:
        // cache must miss and rebuild at the latest seq.
        store.observe_add(hex_id(3), 1_700_000_003);
        let s4 = store.snapshot();
        assert!(!Arc::ptr_eq(&s1, &s4));
        assert_eq!(s4.seq, 3);

        // An epoch reset via initial checkpoint must invalidate the cache.
        let tx = hex_id(4);
        store.install_checkpoint(&[&tx], true, 1_700_000_004);
        let s5 = store.snapshot();
        assert!(!Arc::ptr_eq(&s4, &s5));
        assert_ne!(s4.stream_epoch, s5.stream_epoch);
        assert_eq!(s5.seq, 1);
    }

    #[tokio::test]
    async fn snapshot_async_coalesces_concurrent_misses() {
        let store = Arc::new(HeartbeatStore::new());
        let tx = hex_id(1);
        store.install_checkpoint(&[&tx], true, 1_700_000_000);

        let mut tasks = Vec::new();
        for _ in 0..16 {
            let store = Arc::clone(&store);
            tasks.push(tokio::spawn(
                async move { store.snapshot_async().await },
            ));
        }

        let mut results = Vec::with_capacity(tasks.len());
        for task in tasks {
            results.push(task.await.expect("snapshot task"));
        }
        for snap in &results[1..] {
            assert!(
                Arc::ptr_eq(&results[0], snap),
                "concurrent cache misses must share one serialized snapshot"
            );
        }
    }

    #[test]
    fn update_node_mempool_skips_unchanged_readings_regardless_of_observed_at()
    {
        let store = HeartbeatStore::new();
        let (_, seq0) = store.current_revision();

        // None -> None is a no-op.
        store.update_node_mempool(None);
        assert_eq!(store.current_revision().1, seq0);

        store.update_node_mempool(Some(NodeMempoolAggregate {
            tx_count: 100,
            vbytes: 50_000,
            observed_at: 1_700_000_001,
            stale: false,
        }));
        let seq1 = store.current_revision().1;
        assert_eq!(seq1, seq0 + 1);

        // Same (tx_count, vbytes, stale) with newer observed_at does not emit.
        store.update_node_mempool(Some(NodeMempoolAggregate {
            tx_count: 100,
            vbytes: 50_000,
            observed_at: 1_700_000_002,
            stale: false,
        }));
        assert_eq!(store.current_revision().1, seq1);

        // Changed stale flag emits.
        store.update_node_mempool(Some(NodeMempoolAggregate {
            tx_count: 100,
            vbytes: 50_000,
            observed_at: 1_700_000_003,
            stale: true,
        }));
        assert_eq!(store.current_revision().1, seq1 + 1);

        // Transition back to None emits.
        store.update_node_mempool(None);
        assert_eq!(store.current_revision().1, seq1 + 2);
    }

    #[test]
    fn checkpoint_preserves_details_on_reset_and_diffs_incrementally_when_small(
    ) {
        let store = HeartbeatStore::new();
        let tx1 = hex_id(1);
        let tx2 = hex_id(2);
        let tx3 = hex_id(3);

        // Pre-checkpoint observation + enrichment of tx1.
        store.observe_add(tx1.clone(), 1_700_000_000);
        store.enrich_tx(
            tx1.clone(),
            2_500,
            200,
            100_000,
            TxFlags::ROUND_NUMBER,
            None,
        );
        let (initial_epoch, _) = store.current_revision();

        // First checkpoint always resets epoch and preserves tx1's details.
        let initial_members = [&tx1, &tx2];
        store.install_checkpoint(&initial_members, true, 1_700_000_010);
        let (epoch_after_init, seq_after_init) = store.current_revision();
        assert_ne!(epoch_after_init, initial_epoch);
        assert_eq!(seq_after_init, 1);

        match store.lookup_tx(&tx1) {
            TxLookupStatus::Unconfirmed(tx) => {
                assert_eq!(tx.fee, Some(2_500));
                assert_eq!(tx.vsize, Some(200));
                assert_eq!(tx.flags, TxFlags::ROUND_NUMBER);
            }
            other => panic!("expected unconfirmed tx1, got {other:?}"),
        }

        // Small diff checkpoint (remove tx2, add tx3) stays on the same epoch.
        let next_members = [&tx1, &tx3];
        store.install_checkpoint(&next_members, true, 1_700_000_020);
        let (epoch_after_small, seq_after_small) = store.current_revision();
        assert_eq!(epoch_after_small, epoch_after_init);
        assert!(seq_after_small > seq_after_init);
        assert!(matches!(
            store.lookup_tx(&tx2),
            TxLookupStatus::Absent { .. }
        ));
        assert!(matches!(
            store.lookup_tx(&tx3),
            TxLookupStatus::Unconfirmed(_)
        ));
    }

    #[test]
    fn replay_buffer_enforces_event_and_byte_caps() {
        // Event cap = 3.
        let store = HeartbeatStore::with_replay_limits(3, MAX_REPLAY_BYTES);
        let (epoch, _) = store.current_revision();
        for i in 1..=5u64 {
            store.observe_add(hex_id(i), 1_700_000_000 + i);
        }
        // Only seqs 3, 4, 5 remain in replay: after=2 succeeds, after=1 expires.
        match store.subscribe_from(Some(&epoch), Some(2)) {
            SubscribeResult::Stream {
                replay,
                last_replayed,
                ..
            } => {
                let seqs: Vec<u64> = replay.iter().map(|d| d.seq).collect();
                assert_eq!(seqs, vec![3, 4, 5]);
                assert_eq!(last_replayed, 5);
            }
            other => panic!("expected replay stream, got {other:?}"),
        }
        match store.subscribe_from(Some(&epoch), Some(1)) {
            SubscribeResult::Resync(r) => {
                assert_eq!(r.reason, "cursor_expired");
                assert_eq!(r.latest_seq, 5);
            }
            other => panic!("expected resync, got {other:?}"),
        }

        // Byte cap = 1024 bytes: large BlockInclusion evicts earlier deltas.
        let byte_capped = HeartbeatStore::with_replay_limits(100, 1024);
        let (epoch_b, _) = byte_capped.current_revision();
        byte_capped.observe_add(hex_id(1), 100);
        let big_txids: Vec<String> = (10..30).map(hex_id).collect();
        byte_capped.apply_block_inclusion(hex_id(99), 900_000, big_txids);
        // The large BlockInclusion exceeds 1024 bytes on its own, so replay is
        // evicted and a reconnect from 0 cleanly requests a resync.
        assert!(matches!(
            byte_capped.subscribe_from(Some(&epoch_b), Some(0)),
            SubscribeResult::Resync(ResyncRequired { ref reason, .. })
                if reason == "cursor_expired"
        ));
    }

    #[test]
    fn mainnet_block_inclusion_before_checkpoint_avoids_reset_and_records_confirmations(
    ) {
        let store = HeartbeatStore::new();
        let initial_txids: Vec<String> = (1..=5_000u64).map(hex_id).collect();
        let initial_refs: Vec<&String> = initial_txids.iter().collect();
        store.install_checkpoint(&initial_refs, true, 1_700_000_000);

        let (epoch_before, seq_before) = store.current_revision();
        let mut live_rx = match store
            .subscribe_from(Some(&epoch_before), Some(seq_before))
        {
            SubscribeResult::Stream { live_rx, .. } => live_rx,
            other => panic!("expected live stream subscription, got {other:?}"),
        };

        let block_hash = hex_id(0xbb);
        let block_height = 900_000u64;
        let mined_txids: Vec<String> = initial_txids[..3_000].to_vec();

        store.observe_block(block_hash.clone(), 1_700_000_010);
        store.apply_block_inclusion(
            block_hash.clone(),
            block_height,
            mined_txids.clone(),
        );

        let new_txids: Vec<String> = (5_001..=5_005u64).map(hex_id).collect();
        let mut post_block_members: Vec<&String> =
            initial_txids[3_000..].iter().collect();
        post_block_members.extend(new_txids.iter());

        store.install_checkpoint(&post_block_members, true, 1_700_000_015);

        let (epoch_after, seq_after) = store.current_revision();
        assert_eq!(
            epoch_after, epoch_before,
            "stream_epoch must not rotate when BlockInclusion precedes checkpoint"
        );
        assert!(seq_after > seq_before);

        while let Ok(ev) = live_rx.try_recv() {
            assert!(
                matches!(ev.as_ref(), StoreEvent::Delta(_)),
                "no ResyncRequired should be emitted, got {ev:?}"
            );
        }

        for txid in &mined_txids {
            assert_eq!(
                store.lookup_tx(txid),
                TxLookupStatus::Confirmed {
                    txid: txid.clone(),
                    block_hash: block_hash.clone(),
                    block_height,
                }
            );
        }

        let snap: HeartbeatSnapshot =
            serde_json::from_slice(&store.snapshot().json)
                .expect("valid HeartbeatSnapshot");
        assert_eq!(snap.transactions.len(), 2_005);
        for txid in &post_block_members {
            assert!(matches!(
                store.lookup_tx(txid),
                TxLookupStatus::Unconfirmed(_)
            ));
        }
    }

    #[test]
    fn checkpoint_reset_preserves_recent_confirmations_and_pending_block_fence()
    {
        let store = HeartbeatStore::new();
        let tx_mined = hex_id(1);
        let tx_before_b2 = hex_id(2);
        let tx_after_b2 = hex_id(3);
        let block_1 = hex_id(0xb1);
        let block_2 = hex_id(0xb2);

        store.install_checkpoint(
            &[&tx_mined, &tx_before_b2],
            true,
            1_700_000_000,
        );
        store.observe_block(block_1.clone(), 1_700_000_005);
        store.apply_block_inclusion(
            block_1.clone(),
            900_000,
            vec![tx_mined.clone()],
        );

        // Observe pending block_2, then admit tx_after_b2 after block_2.
        store.observe_block(block_2.clone(), 1_700_000_010);
        store.observe_add(tx_after_b2.clone(), 1_700_000_012);

        // Force a checkpoint reset (> CHECKPOINT_RESET_DIFF_THRESHOLD added).
        let mut reset_members: Vec<String> =
            (100..(100 + CHECKPOINT_RESET_DIFF_THRESHOLD as u64 + 5))
                .map(hex_id)
                .collect();
        reset_members.push(tx_before_b2.clone());
        reset_members.push(tx_after_b2.clone());
        let reset_refs: Vec<&String> = reset_members.iter().collect();
        store.install_checkpoint(&reset_refs, true, 1_700_000_020);

        // Confirmation from block_1 survives the reset.
        assert_eq!(
            store.lookup_tx(&tx_mined),
            TxLookupStatus::Confirmed {
                txid: tx_mined,
                block_hash: block_1,
                block_height: 900_000,
            }
        );

        // Pending block_2's normalized observed_order fence survives the reset:
        // BlockInclusion for block_2 confirms tx_before_b2 but keeps tx_after_b2.
        store.apply_block_inclusion(
            block_2.clone(),
            900_001,
            vec![tx_before_b2.clone(), tx_after_b2.clone()],
        );
        assert_eq!(
            store.lookup_tx(&tx_before_b2),
            TxLookupStatus::Confirmed {
                txid: tx_before_b2,
                block_hash: block_2,
                block_height: 900_001,
            }
        );
        assert!(matches!(
            store.lookup_tx(&tx_after_b2),
            TxLookupStatus::Unconfirmed(_)
        ));
    }

    #[test]
    fn install_checkpoint_sets_unknown_first_seen_only_for_new_members() {
        let store = HeartbeatStore::new();
        let tx_existing = hex_id(1);
        let tx_boot = hex_id(2);
        let tx_incremental = hex_id(3);

        // Pre-checkpoint live arrival has a real first_seen timestamp.
        store.observe_add(tx_existing.clone(), 1_700_000_111);

        // Initial checkpoint (reset path) preserves tx_existing's first_seen
        // and stamps newly discovered tx_boot with first_seen = 0.
        store.install_checkpoint(
            &[&tx_existing, &tx_boot],
            true,
            1_700_000_222,
        );
        match store.lookup_tx(&tx_existing) {
            TxLookupStatus::Unconfirmed(tx) => {
                assert_eq!(tx.first_seen, 1_700_000_111);
            }
            other => panic!("expected tx_existing, got {other:?}"),
        }
        match store.lookup_tx(&tx_boot) {
            TxLookupStatus::Unconfirmed(tx) => {
                assert_eq!(tx.first_seen, 0);
            }
            other => panic!("expected tx_boot, got {other:?}"),
        }

        // Incremental checkpoint stamps newly discovered tx_incremental with 0.
        store.install_checkpoint(
            &[&tx_existing, &tx_boot, &tx_incremental],
            true,
            1_700_000_333,
        );
        match store.lookup_tx(&tx_incremental) {
            TxLookupStatus::Unconfirmed(tx) => {
                assert_eq!(tx.first_seen, 0);
            }
            other => panic!("expected tx_incremental, got {other:?}"),
        }
    }

    #[test]
    fn hydrate_details_is_details_only_invalidates_cache_and_notifies_cleanly()
    {
        let store = HeartbeatStore::new();
        let tx1 = hex_id(10);
        let tx2 = hex_id(11);
        let tx_unknown = hex_id(12);

        store.install_checkpoint(&[&tx1, &tx2], true, 1_700_000_000);
        let (epoch0, seq0) = store.current_revision();

        // Unknown txid is never admitted and returns 0.
        assert_eq!(
            store.hydrate_details(&[TxMetadataHydration {
                txid: tx_unknown.clone(),
                first_seen: Some(1_700_000_005),
                fee: Some(1_000),
                vsize: Some(140),
                value: Some(50_000),
                flags: TxFlags::WHALE,
                op_return_text: Some("ghost".to_string()),
            }]),
            0
        );
        assert!(matches!(
            store.lookup_tx(&tx_unknown),
            TxLookupStatus::Absent { .. }
        ));
        assert_eq!(store.current_revision(), (epoch0.clone(), seq0));

        // Populate snapshot cache and attach a live subscriber.
        let snap_before = store.snapshot();
        let mut live_rx = match store.subscribe_from(Some(&epoch0), Some(seq0))
        {
            SubscribeResult::Stream { live_rx, .. } => live_rx,
            other => panic!("expected live stream, got {other:?}"),
        };

        // Small partial-value + first_seen-backfilling hydration emits an
        // incremental DeltaKind::TxDetails with ZERO epoch rotation!
        assert_eq!(
            store.hydrate_details(&[TxMetadataHydration {
                txid: tx1.clone(),
                first_seen: Some(1_699_999_500),
                fee: Some(2_000),
                vsize: Some(200),
                value: None,
                flags: TxFlags::NONE,
                op_return_text: None,
            }]),
            1
        );
        let (epoch1, seq1) = store.current_revision();
        assert_eq!(epoch1, epoch0);
        assert_eq!(seq1, seq0 + 1);
        match live_rx.try_recv().expect("partial delta event").as_ref() {
            StoreEvent::Delta(d) => assert!(matches!(
                &d.kind,
                DeltaKind::TxDetails {
                    txid,
                    fee: 2_000,
                    vsize: 200,
                    value: None,
                    flags,
                    op_return_text: None,
                    first_seen: Some(1_699_999_500),
                } if txid == &tx1 && *flags == TxFlags::NONE
            )),
            other => panic!("expected TxDetails delta, got {other:?}"),
        }
        match store.lookup_tx(&tx1) {
            TxLookupStatus::Unconfirmed(tx) => {
                assert_eq!(tx.first_seen, 1_699_999_500);
                assert_eq!(tx.fee, Some(2_000));
                assert_eq!(tx.vsize, Some(200));
                assert_eq!(tx.value, None);
            }
            other => panic!("expected hydrated tx1, got {other:?}"),
        }

        // Small complete-detail hydration emits incremental DeltaKind::TxDetails
        // without rotating epoch and invalidates snapshot_cache.
        assert_eq!(
            store.hydrate_details(&[TxMetadataHydration {
                txid: tx1.clone(),
                first_seen: None,
                fee: Some(2_000),
                vsize: Some(200),
                value: Some(500_000),
                flags: TxFlags::ROUND_NUMBER,
                op_return_text: None,
            }]),
            1
        );
        let (epoch2, seq2) = store.current_revision();
        assert_eq!(epoch2, epoch0);
        assert_eq!(seq2, seq1 + 1);
        match live_rx.try_recv().expect("delta event").as_ref() {
            StoreEvent::Delta(d) => assert!(matches!(
                &d.kind,
                DeltaKind::TxDetails {
                    txid,
                    fee: 2_000,
                    vsize: 200,
                    value: Some(500_000),
                    flags,
                    op_return_text: None,
                    first_seen: Some(1_699_999_500),
                } if txid == &tx1 && *flags == TxFlags::ROUND_NUMBER
            )),
            other => panic!("expected TxDetails delta, got {other:?}"),
        }
        let snap_mid = store.snapshot();
        assert!(!Arc::ptr_eq(&snap_before, &snap_mid));

        // Bulk hydration (> 256 members) updates in place, clears snapshot_cache,
        // and ALWAYS rotates stream_epoch + resets seq = 1 + emits ResyncRequired.
        let bulk_store = HeartbeatStore::new();
        let bulk_ids: Vec<String> = (100..370u64).map(hex_id).collect();
        let bulk_refs: Vec<&String> = bulk_ids.iter().collect();
        bulk_store.install_checkpoint(&bulk_refs, true, 1_700_000_000);
        let (bulk_epoch0, bulk_seq0) = bulk_store.current_revision();
        assert_eq!(bulk_seq0, 1);

        // Even with ZERO subscribers and NO cached snapshot, bulk hydration
        // rotates stream_epoch so a resuming client at bulk_epoch0 cannot
        // replay across an unlogged bulk mutation.
        let bulk_entries: Vec<TxMetadataHydration> = bulk_ids
            .iter()
            .map(|id| TxMetadataHydration {
                txid: id.clone(),
                first_seen: Some(1_699_999_800),
                fee: Some(1_500),
                vsize: Some(150),
                value: None,
                flags: TxFlags::NONE,
                op_return_text: None,
            })
            .collect();
        assert_eq!(bulk_store.hydrate_details(&bulk_entries), bulk_ids.len());
        let (bulk_epoch1, bulk_seq1) = bulk_store.current_revision();
        assert_ne!(bulk_epoch1, bulk_epoch0);
        assert_eq!(bulk_seq1, 1);
        assert!(matches!(
            bulk_store.subscribe_from(Some(&bulk_epoch0), Some(bulk_seq0)),
            SubscribeResult::Resync(ResyncRequired { ref reason, .. })
                if reason == "epoch_mismatch"
        ));

        // With an active subscriber, a second bulk hydration (> 256) also sends
        // ResyncRequired { reason: "metadata_hydrated" }.
        let mut bulk_rx = match bulk_store
            .subscribe_from(Some(&bulk_epoch1), Some(bulk_seq1))
        {
            SubscribeResult::Stream { live_rx, .. } => live_rx,
            other => panic!("expected live stream, got {other:?}"),
        };
        let bulk_entries2: Vec<TxMetadataHydration> = bulk_ids
            .iter()
            .map(|id| TxMetadataHydration {
                txid: id.clone(),
                first_seen: None,
                fee: None,
                vsize: None,
                value: Some(123_456),
                flags: TxFlags::NONE,
                op_return_text: None,
            })
            .collect();
        assert_eq!(bulk_store.hydrate_details(&bulk_entries2), bulk_ids.len());
        let (bulk_epoch2, bulk_seq2) = bulk_store.current_revision();
        assert_ne!(bulk_epoch2, bulk_epoch1);
        assert_eq!(bulk_seq2, 1);
        match bulk_rx.try_recv().expect("resync event").as_ref() {
            StoreEvent::Resync(r) => {
                assert_eq!(r.reason, "metadata_hydrated");
                assert_eq!(r.stream_epoch, bulk_epoch2);
                assert_eq!(r.latest_seq, 1);
            }
            other => panic!("expected ResyncRequired, got {other:?}"),
        }

        // Subsequent live delta on the new epoch applies cleanly.
        bulk_store.observe_add(hex_id(999), 1_700_000_999);
        assert_eq!(bulk_store.current_revision(), (bulk_epoch2, 2));
    }

    #[test]
    fn install_checkpoint_with_hydration_applies_details_before_initial_checkpoint(
    ) {
        let store = HeartbeatStore::new();
        let tx_live = hex_id(1);
        let tx_sqlite = hex_id(2);
        let tx_unknown = hex_id(3);

        // Live pre-checkpoint observation establishes a known first_seen > 0.
        store.observe_add(tx_live.clone(), 1_700_000_500);
        let (epoch0, seq0) = store.current_revision();
        let mut live_rx = match store.subscribe_from(Some(&epoch0), Some(seq0))
        {
            SubscribeResult::Stream { live_rx, .. } => live_rx,
            other => panic!("expected live stream, got {other:?}"),
        };

        let hydration = vec![
            TxMetadataHydration {
                txid: tx_live.clone(),
                first_seen: Some(1_699_999_000),
                fee: Some(1_200),
                vsize: Some(140),
                value: Some(50_000),
                flags: TxFlags::ROUND_NUMBER,
                op_return_text: None,
            },
            TxMetadataHydration {
                txid: tx_sqlite.clone(),
                first_seen: Some(1_700_000_100),
                fee: Some(3_400),
                vsize: Some(220),
                value: Some(900_000),
                flags: TxFlags::WHALE,
                op_return_text: Some("boot".to_string()),
            },
            TxMetadataHydration {
                txid: tx_unknown.clone(),
                first_seen: Some(1_700_000_200),
                fee: Some(500),
                vsize: Some(100),
                value: Some(10_000),
                flags: TxFlags::NONE,
                op_return_text: None,
            },
        ];

        store.install_checkpoint_with_hydration(
            &[&tx_live, &tx_sqlite],
            true,
            1_700_000_600,
            &hydration,
        );

        let (epoch1, seq1) = store.current_revision();
        assert_ne!(epoch1, epoch0);
        assert_eq!(seq1, 1);

        // Exactly ONE ResyncRequired (initial_checkpoint), no second rotation.
        match live_rx.try_recv().expect("initial resync").as_ref() {
            StoreEvent::Resync(r) => {
                assert_eq!(r.reason, "initial_checkpoint");
                assert_eq!(r.stream_epoch, epoch1);
                assert_eq!(r.latest_seq, 1);
            }
            other => {
                panic!("expected initial_checkpoint resync, got {other:?}")
            }
        }
        assert!(live_rx.try_recv().is_err());

        // Unknown hydration txid was never admitted.
        assert!(matches!(
            store.lookup_tx(&tx_unknown),
            TxLookupStatus::Absent { .. }
        ));

        // Live member got hydrated details while preserving its live first_seen.
        match store.lookup_tx(&tx_live) {
            TxLookupStatus::Unconfirmed(tx) => {
                assert_eq!(tx.first_seen, 1_700_000_500);
                assert_eq!(tx.fee, Some(1_200));
                assert_eq!(tx.vsize, Some(140));
                assert_eq!(tx.value, Some(50_000));
                assert_eq!(tx.flags, TxFlags::ROUND_NUMBER);
            }
            other => panic!("expected tx_live, got {other:?}"),
        }

        // SQLite-hydrated checkpoint member has all fields populated immediately.
        match store.lookup_tx(&tx_sqlite) {
            TxLookupStatus::Unconfirmed(tx) => {
                assert_eq!(tx.first_seen, 1_700_000_100);
                assert_eq!(tx.fee, Some(3_400));
                assert_eq!(tx.vsize, Some(220));
                assert_eq!(tx.value, Some(900_000));
                assert_eq!(tx.flags, TxFlags::WHALE);
                assert_eq!(tx.op_return_text.as_deref(), Some("boot"));
            }
            other => panic!("expected tx_sqlite, got {other:?}"),
        }
    }

    #[test]
    fn unenriched_txids_respects_age_and_resolves_zero_fee_truc_tx() {
        let store = HeartbeatStore::new();
        let tx_old = hex_id(0x41);
        let tx_fresh = hex_id(0x42);
        let tx_done = hex_id(0x43);
        let empty = HashSet::new();

        store.install_checkpoint(&[&tx_old, &tx_done], true, 1_700_000_000);
        store.observe_add(tx_fresh.clone(), 1_700_000_098);
        store.enrich_tx(tx_done.clone(), 500, 140, 10_000, 0, None);

        // At now = 1_700_000_100, min_age_secs = 5: tx_old (first_seen=0) is
        // eligible, tx_fresh (age 2s) is too new, tx_done already has details.
        let pending = store.unenriched_txids(32, 5, 1_700_000_100, &empty);
        assert_eq!(pending, vec![tx_old.clone()]);

        // Hydrating a genuine 0-sat TRUC/package tx resolves it via incremental
        // delta and removes it from unenriched_txids without epoch rotation.
        let (epoch_before, _) = store.current_revision();
        let updated = store.hydrate_details(&[TxMetadataHydration {
            txid: tx_old.clone(),
            fee: Some(0),
            vsize: Some(139),
            value: None,
            flags: 0,
            op_return_text: None,
            first_seen: None,
        }]);
        assert_eq!(updated, 1);
        assert_eq!(store.current_revision().0, epoch_before);
        assert!(store
            .unenriched_txids(32, 5, 1_700_000_100, &empty)
            .is_empty());

        // Once tx_fresh ages past 5s, it becomes eligible.
        assert_eq!(
            store.unenriched_txids(32, 5, 1_700_000_105, &empty),
            vec![tx_fresh]
        );
    }

    #[test]
    fn unenriched_txids_excludes_backing_off_entries_before_limit() {
        let store = HeartbeatStore::new();
        let mut backing_off = HashSet::new();
        let mut checkpoint_ids = Vec::new();
        for i in 0..70u64 {
            let id = hex_id(i);
            if i < 66 {
                backing_off.insert(id.clone());
            }
            checkpoint_ids.push(id);
        }
        let refs: Vec<&String> = checkpoint_ids.iter().collect();
        store.install_checkpoint(&refs, true, 1_700_000_000);

        // Even though 66 unenriched txids exceed max_count=32, excluding them
        // before `.take(32)` ensures all 4 remaining eligible txids are returned.
        let mut eligible =
            store.unenriched_txids(32, 5, 1_700_000_100, &backing_off);
        eligible.sort();
        let mut expected: Vec<String> = (66..70u64).map(hex_id).collect();
        expected.sort();
        assert_eq!(eligible, expected);
    }

    #[test]
    fn has_pending_inclusion_tracks_unresolved_blocks_with_timeout() {
        let store = HeartbeatStore::new();
        let tx1 = hex_id(1);
        store.install_checkpoint(&[&tx1], true, 1_700_000_000);

        assert!(!store.has_pending_inclusion(1_700_000_005, 90));

        let block_hash = hex_id(99);
        store.observe_block(block_hash.clone(), 1_700_000_010);
        // Freshly observed block without BlockInclusion blocks premature checkpoint.
        assert!(store.has_pending_inclusion(1_700_000_010, 90));
        assert!(store.has_pending_inclusion(1_700_000_099, 90));
        // Past the 90s fallback ceiling, checkpoints are allowed again.
        assert!(!store.has_pending_inclusion(1_700_000_100, 90));

        // Once BlockInclusion arrives, height is populated and the hold clears immediately.
        store.apply_block_inclusion(block_hash, 900_000, vec![tx1]);
        assert!(!store.has_pending_inclusion(1_700_000_020, 90));
    }
}
