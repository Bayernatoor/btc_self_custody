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
//! Stale-on-error: if an RPC call fails and we have a previous value cached, we
//! return the stale value with `is_stale = true`. Callers can surface this via a
//! `stale` flag in their response payload. Fresh fetches always bypass cache via
//! the `_fresh` method variants on [`crate::stats::rpc::BitcoinRpc`].

use std::future::Future;
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
    /// failed and we fell back to the last known good value.
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
            // notify_waiters() call even if we haven't reached `.await` yet.
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
                    // the timeout means even in pathological cases we retry
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
                    let result = fetch().await;
                    let mut state = self
                        .inner
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    state.in_flight = false;

                    match result {
                        Ok(val) => {
                            state.last = Some((val.clone(), Instant::now()));
                            drop(state);
                            self.notify.notify_waiters();
                            return Ok((val, false));
                        }
                        Err(e) => {
                            self.errors.fetch_add(1, Ordering::Relaxed);
                            // Stale-on-error: if we have a previous value,
                            // hand it back with is_stale=true. The TTL is
                            // NOT extended — next caller will try upstream
                            // again, giving Core a chance to recover.
                            let stale = state.last.as_ref().map(|(v, _)| v.clone());
                            drop(state);
                            self.notify.notify_waiters();
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
}
