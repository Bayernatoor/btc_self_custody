//! TTL + singleflight cache for Bitcoin Core RPC responses.
//!
//! Problem being solved: Bitcoin Core acquires the global `cs_main` mutex during
//! block validation. Any RPC that touches chain state (getblockchaininfo,
//! getmempoolinfo, etc.) queues behind that lock. On a resource-constrained
//! node, validation can take 3-8 seconds. Without caching, every client poll
//! during that window stacks another blocked RPC call, which compounds the
//! stall and can overflow Core's `rpcworkqueue`.
//!
//! The cache solves two problems at once:
//! - **Load reduction**: at N concurrent requests per TTL window, only 1 actually
//!   hits Core. The rest return a still-valid cached value instantly.
//! - **Singleflight**: when a cache miss happens during a stall, only ONE upstream
//!   request is issued. Any other requests that arrive during that window wait on
//!   the single in-flight call instead of piling on.
//!
//! Stale-on-error: if an RPC call fails and a previous value is cached, the
//! cache returns that stale value with `is_stale = true`. Callers can surface this via a
//! `stale` flag in their response payload. Fresh fetches always bypass cache via
//! the `_fresh` method variants on [`crate::stats::rpc::BitcoinRpc`].

use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::sync::Notify;

use super::error::StatsError;

/// Single cached slot for one RPC method. Holds the last known value, a
/// singleflight gate (`in_flight`) that ensures at most one upstream call is
/// active at a time, and per-slot hit/miss counters for observability.
pub struct CachedSlot<T: Clone + Send + 'static> {
    inner: Mutex<SlotInner<T>>,
    notify: Notify,
    hits: AtomicU64,
    misses: AtomicU64,
    errors: AtomicU64,
    stale_served: AtomicU64,
}

struct SlotInner<T> {
    last: Option<(T, Instant)>,
    in_flight: bool,
}

/// Snapshot of a slot's counters for the `/api/stats/cache-stats` endpoint.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SlotStats {
    pub hits: u64,
    pub misses: u64,
    pub errors: u64,
    pub stale_served: u64,
    /// Seconds since the last successful refresh. `None` if the slot has never
    /// been filled.
    pub age_seconds: Option<u64>,
}

impl<T: Clone + Send + 'static> CachedSlot<T> {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(SlotInner {
                last: None,
                in_flight: false,
            }),
            notify: Notify::new(),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            stale_served: AtomicU64::new(0),
        }
    }

    /// Returns a cached value if it exists and is within `ttl`, otherwise
    /// calls `fetch`. Only one `fetch` call is in flight per slot at any time;
    /// concurrent callers that hit a miss wait on the in-flight call rather
    /// than issuing their own.
    ///
    /// Returns `(value, is_stale)`. `is_stale` is true when the upstream fetch
    /// failed and the cache fell back to the last known good value.
    ///
    /// # Invariants / caller guarantees
    ///
    /// - `ttl` must be non-zero. `Duration::ZERO` treats every call as
    ///   expired, producing miss-every-time behavior that defeats the cache
    ///   (no error, just wasted work).
    /// - `fetch` should be idempotent: if the 250ms wait-timeout fires
    ///   (defense-in-depth only — the Notified::enable() contract normally
    ///   guarantees wake-up), the loop retries and may call `fetch` again
    ///   on a subsequent iteration.
    /// - `is_stale: true` only appears when `fetch` errors AND a previous
    ///   value was cached. First-ever errors propagate as `Err`.
    /// - The stale-on-error path logs a warning via `tracing::warn!` so
    ///   operators can see when Core is flapping even if clients don't
    ///   surface the `is_stale` flag.
    pub async fn get_or_fetch<F, Fut>(
        &self,
        ttl: Duration,
        fetch: F,
    ) -> Result<(T, bool), StatsError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, StatsError>>,
    {
        loop {
            // Pre-register for notification BEFORE acquiring the lock.
            // enable() guarantees this future will receive any subsequent
            // notify_waiters() call even if the `.await` hasn't been reached yet.
            // Without this, a waiter that drops the lock can miss the
            // fetcher's signal if the fetcher finishes in that narrow window,
            // causing the waiter to park forever — which in testing produced
            // +1 miss / +0 hits from a burst of 20 concurrent callers instead
            // of +1 miss / +19 hits.
            let notified = self.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            // Critical section: decide what this caller should do. The lock is
            // never held across an await below.
            let action = {
                let mut state = self.inner.lock().unwrap_or_else(|e| {
                    // Poisoned mutex — recover rather than crashing the whole
                    // service. A panic in a previous holder doesn't prevent us
                    // from continuing to serve reads and writes.
                    e.into_inner()
                });
                if let Some((val, t)) = &state.last {
                    if t.elapsed() < ttl {
                        self.hits.fetch_add(1, Ordering::Relaxed);
                        return Ok((val.clone(), false));
                    }
                }
                if state.in_flight {
                    Action::Wait
                } else {
                    state.in_flight = true;
                    self.misses.fetch_add(1, Ordering::Relaxed);
                    Action::Fetch
                }
            };

            match action {
                Action::Wait => {
                    // Bounded wait with timeout as a defense-in-depth backstop.
                    // enable() above should guarantee no missed signals, but
                    // the timeout means even in pathological cases the loop retries
                    // rather than hanging the request forever. 250ms is
                    // imperceptible to users and the loop re-checks cache.
                    let _ = tokio::time::timeout(
                        Duration::from_millis(250),
                        notified.as_mut(),
                    )
                    .await;
                    continue;
                }
                Action::Fetch => {
                    // RAII guard ensures in_flight is cleared and waiters are
                    // notified on every exit path — including panic, error,
                    // and future-drop (handler cancellation). See
                    // `InFlightGuard` for the repro that motivated this.
                    let _guard = InFlightGuard {
                        inner: &self.inner,
                        notify: &self.notify,
                    };
                    let result = fetch().await;
                    let mut state = self
                        .inner
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());

                    match result {
                        Ok(val) => {
                            state.last = Some((val.clone(), Instant::now()));
                            // `state` drops first (releases lock), then
                            // `_guard` drops (clears in_flight + notifies).
                            return Ok((val, false));
                        }
                        Err(e) => {
                            self.errors.fetch_add(1, Ordering::Relaxed);
                            // Stale-on-error: if there's a previous value,
                            // hand it back with is_stale=true. The TTL is
                            // NOT extended — next caller will try upstream
                            // again, giving Core a chance to recover.
                            let stale = state.last.as_ref().map(|(v, _)| v.clone());
                            drop(state);
                            if let Some(v) = stale {
                                self.stale_served.fetch_add(1, Ordering::Relaxed);
                                tracing::warn!(
                                    "RPC fetch failed, serving stale value: {e}"
                                );
                                return Ok((v, true));
                            }
                            return Err(e);
                        }
                    }
                }
            }
        }
    }

    /// Forcibly invalidates the cached entry so the next caller refetches.
    /// Used by ZMQ `hashblock` handlers to refresh tip-dependent caches as
    /// soon as a new block arrives.
    pub fn invalidate(&self) {
        let mut state = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        state.last = None;
    }

    pub fn stats(&self) -> SlotStats {
        let state = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let age_seconds = state.last.as_ref().map(|(_, t)| t.elapsed().as_secs());
        SlotStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            stale_served: self.stale_served.load(Ordering::Relaxed),
            age_seconds,
        }
    }
}

impl<T: Clone + Send + 'static> Default for CachedSlot<T> {
    fn default() -> Self {
        Self::new()
    }
}

enum Action {
    Wait,
    Fetch,
}

/// RAII guard that guarantees `in_flight` is cleared and waiters are notified
/// on every exit path from `Action::Fetch`, including:
/// - normal success / error returns,
/// - panics inside the user-supplied fetch future,
/// - future cancellation (e.g. axum drops the handler when the client
///   disconnects, or a `tokio::select!` branch loses).
///
/// Before this guard existed, a cancelled fetcher left `in_flight=true`
/// forever. Subsequent callers queued behind it, looped via the 250ms Notify
/// backstop indefinitely, and the slot was effectively dead until the app
/// restarted. Repro: observed on prod 2026-04-20 when Bitcoin Core's
/// container IP shifted, the first live-stats fetch hung on a half-dead
/// pooled TCP connection, nginx cancelled the handler at its 60s proxy
/// timeout, and `in_flight` stuck on forever.
struct InFlightGuard<'a, T: Clone + Send + 'static> {
    inner: &'a Mutex<SlotInner<T>>,
    notify: &'a Notify,
}

impl<'a, T: Clone + Send + 'static> Drop for InFlightGuard<'a, T> {
    fn drop(&mut self) {
        let mut state = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        state.in_flight = false;
        drop(state);
        self.notify.notify_waiters();
    }
}

// ---------------------------------------------------------------------------
// LRU cache for immutable RPC responses (historical blocks, block hashes).
// ---------------------------------------------------------------------------

/// Bounded LRU cache keyed by `K`, values of type `V`. No TTL — entries stay
/// until evicted by capacity pressure. Used for RPC responses that never
/// change after confirmation (e.g. `getblockhash(height)`, `getblock(hash)`
/// for confirmed blocks). On reorg, affected entries must be explicitly
/// invalidated by callers.
///
/// Uses a HashMap + monotonic access-counter approach rather than a proper
/// doubly-linked-list LRU: eviction is O(n) over capacity but access is O(1).
/// At the capacities in use (500-10_000) this is negligible and the code is
/// much simpler than a hand-rolled intrusive list.
pub struct LruSlot<K, V>
where
    K: Eq + Hash + Clone + Send + 'static,
    V: Clone + Send + 'static,
{
    inner: Mutex<LruInner<K, V>>,
    capacity: usize,
    hits: AtomicU64,
    misses: AtomicU64,
}

struct LruInner<K, V> {
    entries: HashMap<K, (V, u64)>,
    counter: u64,
}

/// Snapshot of an LruSlot's counters for observability.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LruStats {
    pub hits: u64,
    pub misses: u64,
    pub size: usize,
    pub capacity: usize,
}

impl<K, V> LruSlot<K, V>
where
    K: Eq + Hash + Clone + Send + 'static,
    V: Clone + Send + 'static,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(LruInner {
                entries: HashMap::with_capacity(capacity),
                counter: 0,
            }),
            capacity,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    /// Look up a cached value. Hit updates the access-recency counter so
    /// frequently-used entries are protected from eviction.
    pub fn get(&self, key: &K) -> Option<V> {
        let mut state = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        // Bump counter first (releases the immutable view needed next),
        // then get_mut to update the entry's recency marker.
        state.counter += 1;
        let counter = state.counter;
        let result = state.entries.get_mut(key).map(|entry| {
            entry.1 = counter;
            entry.0.clone()
        });
        drop(state);
        if result.is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    /// Insert a value. If at capacity, evicts the least-recently-used entry
    /// first. Subsequent gets for `key` will hit the cache.
    pub fn put(&self, key: K, value: V) {
        let mut state = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        state.counter += 1;
        let counter = state.counter;
        if state.entries.len() >= self.capacity
            && !state.entries.contains_key(&key)
        {
            // Find and remove the entry with the lowest access-counter.
            if let Some(oldest_key) = state
                .entries
                .iter()
                .min_by_key(|(_, (_, c))| *c)
                .map(|(k, _)| k.clone())
            {
                state.entries.remove(&oldest_key);
            }
        }
        state.entries.insert(key, (value, counter));
    }

    /// Explicit invalidation — used on reorg to drop stale entries for the
    /// affected heights/hashes.
    pub fn remove(&self, key: &K) {
        let mut state = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        state.entries.remove(key);
    }

    pub fn stats(&self) -> LruStats {
        let state = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        LruStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            size: state.entries.len(),
            capacity: self.capacity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn returns_cached_value_within_ttl() {
        let slot = CachedSlot::<i32>::new();
        let calls = Arc::new(AtomicU32::new(0));

        for _ in 0..5 {
            let c = calls.clone();
            let (v, stale) = slot
                .get_or_fetch(Duration::from_secs(60), || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok(42)
                })
                .await
                .unwrap();
            assert_eq!(v, 42);
            assert!(!stale);
        }

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let stats = slot.stats();
        assert_eq!(stats.hits, 4);
        assert_eq!(stats.misses, 1);
    }

    #[tokio::test]
    async fn refetches_after_ttl_expires() {
        let slot = CachedSlot::<i32>::new();
        let calls = Arc::new(AtomicU32::new(0));

        let c1 = calls.clone();
        slot.get_or_fetch(Duration::from_millis(50), || async move {
            c1.fetch_add(1, Ordering::SeqCst);
            Ok(1)
        })
        .await
        .unwrap();

        tokio::time::sleep(Duration::from_millis(80)).await;

        let c2 = calls.clone();
        slot.get_or_fetch(Duration::from_millis(50), || async move {
            c2.fetch_add(1, Ordering::SeqCst);
            Ok(2)
        })
        .await
        .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn singleflight_dedups_concurrent_misses() {
        let slot = Arc::new(CachedSlot::<i32>::new());
        let calls = Arc::new(AtomicU32::new(0));

        let mut handles = Vec::new();
        for _ in 0..20 {
            let slot = slot.clone();
            let calls = calls.clone();
            handles.push(tokio::spawn(async move {
                slot.get_or_fetch(Duration::from_secs(60), || async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    // Simulate slow upstream call
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(42)
                })
                .await
                .unwrap()
            }));
        }

        for h in handles {
            let (v, _stale) = h.await.unwrap();
            assert_eq!(v, 42);
        }

        // Critical assertion: 20 concurrent callers, exactly 1 upstream call.
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    /// Stress-tests the enable()-before-drop race fix. With a fast fetch
    /// (no artificial delay), the fetcher can complete before waiters reach
    /// their await point. Before the fix, this produced "1 miss, 0 hits"
    /// because waiters parked forever on a signal that fired before they
    /// registered. With enable(), all waiters either see a fresh cache on
    /// retry OR get the signal even if it was issued between drop-lock and
    /// the `.await`.
    #[tokio::test]
    async fn singleflight_no_race_with_fast_fetch() {
        for _ in 0..10 {
            let slot = Arc::new(CachedSlot::<i32>::new());
            let calls = Arc::new(AtomicU32::new(0));
            let mut handles = Vec::new();
            for _ in 0..50 {
                let slot = slot.clone();
                let calls = calls.clone();
                handles.push(tokio::spawn(async move {
                    slot.get_or_fetch(Duration::from_secs(60), || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok(42)
                    })
                    .await
                    .unwrap()
                }));
            }

            // All 50 callers must complete — none hang.
            let results = futures::future::join_all(handles).await;
            assert_eq!(results.len(), 50);
            for r in results {
                let (v, _stale) = r.expect("task panicked");
                assert_eq!(v, 42);
            }

            // Singleflight: at most 1 upstream call per iteration.
            // (Could be 1 if all 50 queued cleanly; could be more if some
            //  arrived after the first completed and TTL hasn't expired.)
            assert_eq!(calls.load(Ordering::SeqCst), 1);

            let stats = slot.stats();
            // Critical: hits + misses must equal exactly 50 (no dropped calls).
            assert_eq!(stats.hits + stats.misses, 50);
            assert_eq!(stats.misses, 1);
            assert_eq!(stats.hits, 49);
        }
    }

    #[tokio::test]
    async fn stale_on_error_returns_previous_value() {
        let slot = CachedSlot::<i32>::new();

        // Seed with a good value
        slot.get_or_fetch(Duration::from_millis(50), || async { Ok(100) })
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(80)).await;

        // Next fetch errors; should fall back to stale
        let (v, is_stale) = slot
            .get_or_fetch(Duration::from_millis(50), || async {
                Err(StatsError::Rpc("upstream down".to_string()))
            })
            .await
            .unwrap();

        assert_eq!(v, 100);
        assert!(is_stale);
        assert_eq!(slot.stats().stale_served, 1);
    }

    #[tokio::test]
    async fn error_without_cached_value_propagates() {
        let slot = CachedSlot::<i32>::new();

        let result: Result<(i32, bool), StatsError> = slot
            .get_or_fetch(Duration::from_secs(60), || async {
                Err(StatsError::Rpc("no data yet".to_string()))
            })
            .await;

        assert!(result.is_err());
        assert_eq!(slot.stats().errors, 1);
        assert_eq!(slot.stats().stale_served, 0);
    }

    /// Regression test for the in_flight-leak-on-cancellation bug.
    ///
    /// Scenario: a fetcher future hangs (e.g. half-dead pooled TCP socket to a
    /// Bitcoin Core container that silently moved IPs). nginx upstream-times-out
    /// and the axum handler future is dropped. Before the RAII guard fix,
    /// this left in_flight=true forever and every subsequent caller looped on
    /// the 250ms Notify backstop until the app was manually restarted.
    ///
    /// With the guard, cancellation triggers Drop which clears in_flight and
    /// notifies waiters, so the next caller can acquire a fresh fetch slot.
    #[tokio::test]
    async fn cancelled_fetch_does_not_leak_inflight_state() {
        let slot = Arc::new(CachedSlot::<i32>::new());

        // Spawn a fetcher that hangs forever, simulating a stuck RPC.
        let slot_clone = Arc::clone(&slot);
        let hanging = tokio::spawn(async move {
            slot_clone
                .get_or_fetch(Duration::from_secs(60), || async move {
                    // Longer than the test could ever wait — aborted before
                    // this resolves.
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                    Ok::<i32, StatsError>(42)
                })
                .await
        });

        // Yield so the spawned task gets scheduled and enters Action::Fetch.
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Cancel the stuck fetcher (equivalent to the axum handler being
        // dropped when nginx times out and closes the upstream socket).
        hanging.abort();
        let _ = hanging.await; // drain the JoinError

        // The slot must now be usable again. Bound with timeout — if the
        // guard regressed and in_flight leaked, this would loop on the 250ms
        // Notify backstop and exceed our 2s budget.
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            slot.get_or_fetch(Duration::from_secs(60), || async { Ok(100) }),
        )
        .await;

        let (v, stale) = result
            .expect("recovery fetch timed out — in_flight leaked")
            .expect("recovery fetch errored");
        assert_eq!(v, 100);
        assert!(!stale);
    }

    #[tokio::test]
    async fn invalidate_forces_refetch() {
        let slot = CachedSlot::<i32>::new();
        let calls = Arc::new(AtomicU32::new(0));

        for _ in 0..3 {
            let c = calls.clone();
            slot.get_or_fetch(Duration::from_secs(60), || async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(1)
            })
            .await
            .unwrap();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        slot.invalidate();

        let c = calls.clone();
        slot.get_or_fetch(Duration::from_secs(60), || async move {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(1)
        })
        .await
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    // ---- LruSlot tests ----

    #[test]
    fn lru_basic_get_put() {
        let lru = LruSlot::<u64, String>::new(10);
        assert!(lru.get(&1).is_none());
        lru.put(1, "hello".to_string());
        assert_eq!(lru.get(&1), Some("hello".to_string()));

        let s = lru.stats();
        assert_eq!(s.hits, 1);
        assert_eq!(s.misses, 1);
        assert_eq!(s.size, 1);
        assert_eq!(s.capacity, 10);
    }

    #[test]
    fn lru_evicts_least_recently_used_when_full() {
        let lru = LruSlot::<u64, String>::new(3);
        lru.put(1, "a".into());
        lru.put(2, "b".into());
        lru.put(3, "c".into());
        assert_eq!(lru.stats().size, 3);

        // Access 1 — makes it most recent
        assert_eq!(lru.get(&1), Some("a".into()));
        // Access 3 — now 2 is the LRU
        assert_eq!(lru.get(&3), Some("c".into()));

        // Insert 4 — must evict 2 (least recently used)
        lru.put(4, "d".into());
        assert!(lru.get(&2).is_none(), "key 2 should have been evicted");
        assert_eq!(lru.get(&1), Some("a".into()));
        assert_eq!(lru.get(&3), Some("c".into()));
        assert_eq!(lru.get(&4), Some("d".into()));
    }

    #[test]
    fn lru_overwrite_does_not_evict() {
        let lru = LruSlot::<u64, String>::new(2);
        lru.put(1, "a".into());
        lru.put(2, "b".into());
        // Overwriting an existing key shouldn't trigger eviction.
        lru.put(1, "new".into());
        assert_eq!(lru.stats().size, 2);
        assert_eq!(lru.get(&1), Some("new".into()));
        assert_eq!(lru.get(&2), Some("b".into()));
    }

    #[test]
    fn lru_remove_drops_entry() {
        let lru = LruSlot::<u64, String>::new(10);
        lru.put(1, "a".into());
        lru.put(2, "b".into());
        lru.remove(&1);
        assert!(lru.get(&1).is_none());
        assert_eq!(lru.get(&2), Some("b".into()));
    }
}
