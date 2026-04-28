//! Generic in-memory cache primitive with tag-based invalidation.
//!
//! This module is the single sanctioned place for new caches in
//! `StatsState`. Any new cache should be a `Cache<K, V>` registered
//! against a `CacheTag` via `StatsStateBuilder::cache(...)` (see
//! `api.rs`), not a hand-rolled `Mutex<Option<...>>` field.
//!
//! ## Why this exists
//!
//! Pre-existing caches were hand-rolled `Mutex<Option<(K, V, Instant)>>`
//! fields, each with its own lock-check-ttl-return-or-fetch boilerplate
//! at every call site. Invalidation on a new block was a manual block of
//! `.take()` calls in `startup.rs`; forgetting one silently served stale
//! data for 60 to 120 seconds after every block. The registry below
//! collapses both: a single primitive owns the read-and-fetch path, and
//! invalidation routes by tag so adding a cache cannot also add a silent
//! stale-data bug.
//!
//! ## B-now-A-later contract
//!
//! Today `Cache<K, V>` is `Mutex<HashMap<K, (V, Instant)>>` with no
//! bounding and no singleflight. The public API (`get_or_compute`,
//! `get`, `insert`, `invalidate`, `invalidate_key`) is intentionally
//! the same API a future bounded-LRU plus singleflight implementation
//! would expose. When that swap happens, this file changes; call sites
//! do not.

use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Events that invalidate one or more caches. Add a variant when a new
/// invalidation trigger is needed (e.g. `OnReorg`, `OnConfigChange`).
/// Caches register the tags they care about at construction; the
/// registry fans `state.invalidate(tag)` calls out to subscribers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CacheTag {
    /// Chain tip advanced. Invalidates anything keyed by current
    /// height/timestamp or anything that includes today's daily bucket.
    OnNewBlock,
    /// Price refresh task wrote a new value. Currently no cache other
    /// than `price_cache` itself cares; reserved for future fan-out.
    OnPriceRefresh,
}

/// Snapshot of a cache's runtime metrics. Used by the
/// `/api/stats/cache-stats` handler to surface per-cache observability.
#[derive(Clone, Copy, Debug)]
pub struct CacheStats {
    /// Stable identifier (matches the `name` passed at construction).
    pub name: &'static str,
    /// Current entry count.
    pub size: usize,
    /// Total successful reads served from the cache.
    pub hits: u64,
    /// Total reads that triggered a fetch.
    pub misses: u64,
    /// Total direct writes via `insert()` (background warmup, periodic
    /// refresh tasks). These bypass the request path so they don't
    /// count as misses; tracking them separately makes
    /// `size > misses` legible for caches with background warmup.
    pub refreshes: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Object-safe trait so the registry can hold heterogeneous `Cache<K, V>`
/// instances behind `Arc<dyn CacheCell>`. Covers both invalidation
/// (full-clear by tag) and observability (size + hit/miss counters).
/// Per-key invalidation is not on the trait because the registry path
/// is always full-clear; callers who need targeted clears hold the
/// concrete `Arc<Cache<K, V>>` and call `invalidate_key`.
pub trait CacheCell: Send + Sync {
    fn invalidate(&self);
    fn stats(&self) -> CacheStats;
}

/// In-memory cache with TTL, tag-based invalidation, and per-key
/// singleflight on fetches.
///
/// Multi-key (HashMap-backed): different keys cache independently. Each
/// entry has its own TTL deadline measured from when it was stored.
/// `invalidate()` clears all entries; `invalidate_key(k)` clears one.
///
/// **Singleflight on `get_or_compute`.** When N concurrent callers all
/// see a cache miss for the same key, only one runs the fetcher; the
/// others wait on a per-key `OnceCell` and observe the same result.
/// On fetcher error, the slot is dropped so the next caller can retry.
/// Eliminates thundering-herd to upstream services (mempool.space,
/// SQLite long queries) when traffic spikes against a cold key.
///
/// **Not bounded.** Entries accumulate without LRU eviction. Acceptable
/// for the current call sites (single-key TTL caches, range-keyed
/// caches that thrash naturally, and `block_ts_cache` whose worst case
/// is ~24MB at full chain). Bounded LRU is the A.2 follow-up.
pub struct Cache<K, V> {
    name: &'static str,
    inner: Mutex<HashMap<K, (V, Instant)>>,
    /// Per-key singleflight slots. A `OnceCell` initializes exactly
    /// once; concurrent callers all observe the same result. On
    /// initialization failure the cell stays empty and subsequent
    /// callers retry. Slots are removed after the value lands in
    /// `inner` (and the entry is no longer needed).
    inflight: Mutex<HashMap<K, Arc<tokio::sync::OnceCell<V>>>>,
    ttl: Duration,
    tags: Vec<CacheTag>,
    hits: AtomicU64,
    misses: AtomicU64,
    refreshes: AtomicU64,
}

impl<K, V> Cache<K, V>
where
    K: Eq + Hash + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Construct a cache that expires entries after `ttl`. `name` is a
    /// stable identifier surfaced in `/api/stats/cache-stats`. Use
    /// `Cache::permanent(name)` for caches over immutable data
    /// (e.g. block timestamps).
    pub fn new(name: &'static str, ttl: Duration) -> Self {
        Self {
            name,
            inner: Mutex::new(HashMap::new()),
            inflight: Mutex::new(HashMap::new()),
            ttl,
            tags: Vec::new(),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            refreshes: AtomicU64::new(0),
        }
    }

    /// Construct a cache that never expires by TTL. Still respects
    /// explicit `invalidate()` calls. For data that is genuinely
    /// immutable (block hashes, historical timestamps).
    pub fn permanent(name: &'static str) -> Self {
        Self::new(name, Duration::MAX)
    }

    /// Fluent: register an invalidation tag this cache responds to.
    /// Repeatable; multiple tags allowed. The builder in `api.rs`
    /// chains this for each tag passed at construction.
    pub fn invalidated_by(mut self, tag: CacheTag) -> Self {
        self.tags.push(tag);
        self
    }

    /// Read the registered tag list. Used by the builder when wiring
    /// the cache into the registry.
    pub fn tags(&self) -> &[CacheTag] {
        &self.tags
    }

    /// Return cached value for `key` if present and within TTL.
    /// Otherwise run `fetcher`, store the result, and return it.
    ///
    /// **Singleflight:** if multiple callers hit the same key while a
    /// fetch is in flight, only one fetcher runs; the others wait on
    /// a `OnceCell` and observe the same value. On fetcher error the
    /// cell remains uninitialized and the next caller's fetcher takes
    /// over (so transient failures don't permanently poison the slot).
    ///
    /// Generic over the fetcher's error type so each call site keeps
    /// its native error (`StatsError`, `ServerFnError`, etc.) without
    /// wrapping. `E: Send + 'static` is required because the in-flight
    /// future is shared across tasks via `OnceCell`.
    pub async fn get_or_compute<F, Fut, E>(
        &self,
        key: K,
        fetcher: F,
    ) -> Result<V, E>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<V, E>>,
        E: Send + 'static,
    {
        // Read fast path. Lock is dropped before any await below.
        {
            let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((v, ts)) = guard.get(&key) {
                if ts.elapsed() < self.ttl {
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    return Ok(v.clone());
                }
            }
        }
        self.misses.fetch_add(1, Ordering::Relaxed);

        // Singleflight: get-or-create the per-key slot. The first
        // caller's fetcher runs; concurrent callers' fetchers are
        // dropped without running (the OnceCell observes the first
        // initialization).
        let slot: Arc<tokio::sync::OnceCell<V>> = {
            let mut inflight =
                self.inflight.lock().unwrap_or_else(|e| e.into_inner());
            inflight
                .entry(key.clone())
                .or_insert_with(|| Arc::new(tokio::sync::OnceCell::new()))
                .clone()
        };

        let result = slot
            .get_or_try_init(|| async {
                let fresh = fetcher().await?;
                let mut guard =
                    self.inner.lock().unwrap_or_else(|e| e.into_inner());
                guard.insert(key.clone(), (fresh.clone(), Instant::now()));
                Ok::<V, E>(fresh)
            })
            .await
            .cloned();

        // Remove the slot now that the value (or error) has been
        // observed. A subsequent caller will create a fresh slot.
        // Compare data pointers (dyn-fat-pointer-safe via addr_eq)
        // to avoid removing a different slot installed after ours.
        {
            let mut inflight =
                self.inflight.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(current) = inflight.get(&key) {
                if Arc::ptr_eq(current, &slot) {
                    inflight.remove(&key);
                }
            }
        }

        result
    }

    /// Raw read. Returns the cached value for `key` if present and
    /// within TTL, else `None`. For call sites that conditionally cache
    /// (e.g. only insert positive lookup results) and need direct
    /// cache access rather than `get_or_compute`'s fetcher pattern.
    pub fn get(&self, key: &K) -> Option<V> {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let result = guard.get(key).and_then(|(v, ts)| {
            if ts.elapsed() < self.ttl {
                Some(v.clone())
            } else {
                None
            }
        });
        if result.is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    /// Raw write. Stores or overwrites the entry for `key` with
    /// the current timestamp. Pairs with `get` for conditional caching.
    /// Counted as a `refresh` (not a miss) since it bypasses the
    /// request path.
    pub fn insert(&self, key: K, value: V) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(key, (value, Instant::now()));
        self.refreshes.fetch_add(1, Ordering::Relaxed);
    }

    /// Clear the entry for `key` if present. No-op otherwise.
    pub fn invalidate_key(&self, key: &K) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.remove(key);
    }
}

impl<K, V> CacheCell for Cache<K, V>
where
    K: Send + Sync + 'static,
    V: Send + Sync + 'static,
{
    fn invalidate(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.clear();
    }

    fn stats(&self) -> CacheStats {
        let size = self
            .inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len();
        CacheStats {
            name: self.name,
            size,
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            refreshes: self.refreshes.load(Ordering::Relaxed),
        }
    }
}

/// Routes `invalidate(tag)` calls to all caches registered under that
/// tag, and exposes an enumeration of every registered cache for
/// observability. Caches register at construction via
/// `StatsStateBuilder::cache`, so registration is not optional and a
/// cache cannot ship without an invalidation contract.
///
/// `all` is the de-duplicated flat inventory used for stats; the tag
/// map can hold the same cache under multiple keys (one per tag) and
/// invoking it twice is harmless (clear is idempotent), but enumerating
/// it twice in a stats response would be confusing.
#[derive(Default)]
pub struct CacheRegistry {
    subscribers: HashMap<CacheTag, Vec<Arc<dyn CacheCell>>>,
    all: Vec<Arc<dyn CacheCell>>,
}

impl CacheRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a cache to the flat inventory used for stats enumeration.
    /// Idempotent on identity (same `Arc` is not added twice).
    /// `addr_eq` (data-pointer comparison) is required because
    /// `dyn CacheCell` is a fat pointer; `==` on the raw pointers
    /// would also compare vtables.
    pub fn register(&mut self, cache: Arc<dyn CacheCell>) {
        if !self.all.iter().any(|c| {
            std::ptr::addr_eq(Arc::as_ptr(c), Arc::as_ptr(&cache))
        }) {
            self.all.push(cache);
        }
    }

    /// Subscribe a cache to a tag. The builder calls this once per tag
    /// the cache was constructed with.
    pub fn subscribe(&mut self, tag: CacheTag, cache: Arc<dyn CacheCell>) {
        self.subscribers.entry(tag).or_default().push(cache);
    }

    /// Fire `invalidate()` on every cache subscribed to `tag`. No-op
    /// if no caches are subscribed.
    pub fn invalidate(&self, tag: CacheTag) {
        if let Some(subs) = self.subscribers.get(&tag) {
            for sub in subs {
                sub.invalidate();
            }
        }
    }

    /// Snapshot every registered cache. Used by the `/cache-stats`
    /// handler to surface per-cache hit/miss/size to operators.
    pub fn all_stats(&self) -> Vec<CacheStats> {
        self.all.iter().map(|c| c.stats()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::Infallible;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Run a get_or_compute that returns `value` and increments `calls`
    /// each time the fetcher executes. Used to distinguish a cache hit
    /// from a recompute.
    async fn fetch(
        cache: &Cache<u32, &'static str>,
        key: u32,
        value: &'static str,
        calls: &Arc<AtomicUsize>,
    ) -> &'static str {
        let calls = calls.clone();
        cache
            .get_or_compute(key, || async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<_, Infallible>(value)
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn cold_hit_runs_fetcher_then_caches() {
        let cache: Cache<u32, &'static str> =
            Cache::new("test", Duration::from_secs(60));
        let calls = Arc::new(AtomicUsize::new(0));

        let v1 = fetch(&cache, 1, "a", &calls).await;
        let v2 = fetch(&cache, 1, "b", &calls).await;

        assert_eq!(v1, "a");
        assert_eq!(v2, "a", "second call returned cached, fetcher's value ignored");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn ttl_expiry_re_runs_fetcher() {
        let cache: Cache<u32, &'static str> =
            Cache::new("test", Duration::from_millis(10));
        let calls = Arc::new(AtomicUsize::new(0));

        let _ = fetch(&cache, 1, "a", &calls).await;
        tokio::time::sleep(Duration::from_millis(25)).await;
        let _ = fetch(&cache, 1, "b", &calls).await;

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn different_keys_cache_independently() {
        let cache: Cache<u32, &'static str> =
            Cache::new("test", Duration::from_secs(60));
        let calls = Arc::new(AtomicUsize::new(0));

        let _ = fetch(&cache, 1, "a", &calls).await;
        let _ = fetch(&cache, 2, "b", &calls).await;
        // Going back to key 1 should hit cache (multi-key, no thrash)
        let v1 = fetch(&cache, 1, "c", &calls).await;
        let v2 = fetch(&cache, 2, "d", &calls).await;

        assert_eq!(v1, "a", "key 1 still cached");
        assert_eq!(v2, "b", "key 2 still cached");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn raw_get_and_insert_for_conditional_caching() {
        let cache: Cache<u32, &'static str> = Cache::permanent("test");

        assert_eq!(cache.get(&1), None);
        cache.insert(1, "a");
        assert_eq!(cache.get(&1), Some("a"));
        // Different key still uncached
        assert_eq!(cache.get(&2), None);
    }

    #[tokio::test]
    async fn raw_get_respects_ttl() {
        let cache: Cache<u32, &'static str> =
            Cache::new("test", Duration::from_millis(10));
        cache.insert(1, "a");
        assert_eq!(cache.get(&1), Some("a"));
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert_eq!(cache.get(&1), None, "expired entry should not return");
    }

    #[tokio::test]
    async fn permanent_cache_does_not_expire() {
        let cache: Cache<u32, &'static str> = Cache::permanent("test");
        let calls = Arc::new(AtomicUsize::new(0));

        let _ = fetch(&cache, 1, "a", &calls).await;
        // Even after a real wait, no expiry
        tokio::time::sleep(Duration::from_millis(15)).await;
        let _ = fetch(&cache, 1, "b", &calls).await;

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn full_invalidate_clears_entry() {
        let cache: Cache<u32, &'static str> =
            Cache::new("test", Duration::from_secs(60));
        let calls = Arc::new(AtomicUsize::new(0));

        let _ = fetch(&cache, 1, "a", &calls).await;
        cache.invalidate();
        let _ = fetch(&cache, 1, "b", &calls).await;

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn invalidate_key_only_clears_matching() {
        let cache: Cache<u32, &'static str> =
            Cache::new("test", Duration::from_secs(60));
        let calls = Arc::new(AtomicUsize::new(0));

        let _ = fetch(&cache, 1, "a", &calls).await;

        cache.invalidate_key(&999);
        let _ = fetch(&cache, 1, "b", &calls).await;
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "non-matching key invalidation must not clear stored entry"
        );

        cache.invalidate_key(&1);
        let _ = fetch(&cache, 1, "c", &calls).await;
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "matching key invalidation must clear, forcing a re-fetch"
        );
    }

    /// Headline test: register two caches with different tags, fire
    /// invalidation for one tag, assert only the matching cache cleared.
    /// This is the contract that makes the registry safe to rely on.
    #[tokio::test]
    async fn registry_routes_only_to_tagged_subscribers() {
        let block_cache: Arc<Cache<(), &'static str>> = Arc::new(
            Cache::new("test", Duration::from_secs(60))
                .invalidated_by(CacheTag::OnNewBlock),
        );
        let price_cache: Arc<Cache<(), &'static str>> = Arc::new(
            Cache::new("test", Duration::from_secs(60))
                .invalidated_by(CacheTag::OnPriceRefresh),
        );

        let mut registry = CacheRegistry::new();
        registry.subscribe(
            CacheTag::OnNewBlock,
            block_cache.clone() as Arc<dyn CacheCell>,
        );
        registry.subscribe(
            CacheTag::OnPriceRefresh,
            price_cache.clone() as Arc<dyn CacheCell>,
        );

        let block_calls = Arc::new(AtomicUsize::new(0));
        let price_calls = Arc::new(AtomicUsize::new(0));

        // Populate both
        let _ = block_cache
            .get_or_compute((), {
                let c = block_calls.clone();
                || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, Infallible>("blk")
                }
            })
            .await
            .unwrap();
        let _ = price_cache
            .get_or_compute((), {
                let c = price_calls.clone();
                || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, Infallible>("prc")
                }
            })
            .await
            .unwrap();
        assert_eq!(block_calls.load(Ordering::SeqCst), 1);
        assert_eq!(price_calls.load(Ordering::SeqCst), 1);

        // Fire OnNewBlock -> only block_cache should clear
        registry.invalidate(CacheTag::OnNewBlock);

        let _ = block_cache
            .get_or_compute((), {
                let c = block_calls.clone();
                || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, Infallible>("blk")
                }
            })
            .await
            .unwrap();
        let _ = price_cache
            .get_or_compute((), {
                let c = price_calls.clone();
                || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, Infallible>("prc")
                }
            })
            .await
            .unwrap();
        assert_eq!(
            block_calls.load(Ordering::SeqCst),
            2,
            "OnNewBlock cache should re-fetch after its tag fired"
        );
        assert_eq!(
            price_calls.load(Ordering::SeqCst),
            1,
            "OnPriceRefresh cache must not be cleared by OnNewBlock"
        );

        // Inverse: fire OnPriceRefresh -> only price_cache clears
        registry.invalidate(CacheTag::OnPriceRefresh);

        let _ = block_cache
            .get_or_compute((), {
                let c = block_calls.clone();
                || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, Infallible>("blk")
                }
            })
            .await
            .unwrap();
        let _ = price_cache
            .get_or_compute((), {
                let c = price_calls.clone();
                || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, Infallible>("prc")
                }
            })
            .await
            .unwrap();
        assert_eq!(
            block_calls.load(Ordering::SeqCst),
            2,
            "OnNewBlock cache must not be cleared by OnPriceRefresh"
        );
        assert_eq!(
            price_calls.load(Ordering::SeqCst),
            2,
            "OnPriceRefresh cache should re-fetch after its tag fired"
        );
    }

    #[tokio::test]
    async fn registry_invalidate_unknown_tag_is_noop() {
        let cache: Arc<Cache<(), &'static str>> = Arc::new(
            Cache::new("test", Duration::from_secs(60))
                .invalidated_by(CacheTag::OnNewBlock),
        );
        let mut registry = CacheRegistry::new();
        registry.subscribe(
            CacheTag::OnNewBlock,
            cache.clone() as Arc<dyn CacheCell>,
        );
        // OnPriceRefresh has no subscribers; must not panic
        registry.invalidate(CacheTag::OnPriceRefresh);
    }

    #[tokio::test]
    async fn one_cache_can_subscribe_to_multiple_tags() {
        let cache: Arc<Cache<(), &'static str>> = Arc::new(
            Cache::new("test", Duration::from_secs(60))
                .invalidated_by(CacheTag::OnNewBlock)
                .invalidated_by(CacheTag::OnPriceRefresh),
        );
        let mut registry = CacheRegistry::new();
        for tag in cache.tags() {
            registry.subscribe(*tag, cache.clone() as Arc<dyn CacheCell>);
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let _ = cache
            .get_or_compute((), {
                let c = calls.clone();
                || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, Infallible>("v")
                }
            })
            .await
            .unwrap();

        // Either tag should clear it
        registry.invalidate(CacheTag::OnPriceRefresh);
        let _ = cache
            .get_or_compute((), {
                let c = calls.clone();
                || async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, Infallible>("v")
                }
            })
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn stats_track_hits_and_misses() {
        let cache: Cache<u32, &'static str> =
            Cache::new("trk", Duration::from_secs(60));
        let calls = Arc::new(AtomicUsize::new(0));

        // First call: miss + fetch
        let _ = fetch(&cache, 1, "a", &calls).await;
        // Second call same key: hit
        let _ = fetch(&cache, 1, "a", &calls).await;
        // Different key: miss + fetch
        let _ = fetch(&cache, 2, "b", &calls).await;

        let s = cache.stats();
        assert_eq!(s.name, "trk");
        assert_eq!(s.size, 2);
        assert_eq!(s.misses, 2);
        assert_eq!(s.hits, 1);
        assert!((s.hit_rate() - 1.0 / 3.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn raw_get_increments_hits_and_misses() {
        let cache: Cache<u32, &'static str> = Cache::permanent("raw");
        cache.insert(1, "a");
        let _ = cache.get(&1); // hit
        let _ = cache.get(&2); // miss

        let s = cache.stats();
        assert_eq!(s.hits, 1);
        assert_eq!(s.misses, 1);
    }

    #[tokio::test]
    async fn registry_enumerates_all_caches_once() {
        let a: Arc<Cache<(), &'static str>> = Arc::new(
            Cache::new("a", Duration::from_secs(60))
                .invalidated_by(CacheTag::OnNewBlock)
                .invalidated_by(CacheTag::OnPriceRefresh),
        );
        let b: Arc<Cache<(), &'static str>> = Arc::new(
            Cache::new("b", Duration::from_secs(60))
                .invalidated_by(CacheTag::OnNewBlock),
        );
        let c: Arc<Cache<(), &'static str>> =
            Arc::new(Cache::permanent("c"));

        let mut registry = CacheRegistry::new();
        // Mirror what the builder does: each cache registers once for
        // the flat inventory, then subscribes per-tag.
        let a_cell: Arc<dyn CacheCell> = a.clone();
        let b_cell: Arc<dyn CacheCell> = b.clone();
        let c_cell: Arc<dyn CacheCell> = c.clone();
        registry.register(a_cell.clone());
        registry.register(b_cell.clone());
        registry.register(c_cell.clone());
        registry.subscribe(CacheTag::OnNewBlock, a_cell.clone());
        registry.subscribe(CacheTag::OnPriceRefresh, a_cell);
        registry.subscribe(CacheTag::OnNewBlock, b_cell);

        let names: Vec<_> =
            registry.all_stats().into_iter().map(|s| s.name).collect();
        assert_eq!(names.len(), 3, "no duplicates even though `a` has two tags");
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
        assert!(names.contains(&"c"));
    }

    /// Headline singleflight test: many concurrent callers hitting the
    /// same key on a cold cache should share one fetcher invocation.
    /// This is the contract that lets us delete the manual AtomicBool
    /// guard around the price cache and have the same protection
    /// extend to every other cache automatically.
    #[tokio::test]
    async fn singleflight_dedups_concurrent_missers() {
        let cache: Arc<Cache<u32, &'static str>> = Arc::new(Cache::new(
            "sf",
            Duration::from_secs(60),
        ));
        let calls = Arc::new(AtomicUsize::new(0));

        // Fire 50 concurrent get_or_compute calls for the SAME key. The
        // fetcher pauses briefly to ensure all callers race the cold
        // path before the first one finishes; with singleflight, only
        // the first registers the fetcher and the rest wait on the
        // OnceCell.
        let mut handles = Vec::with_capacity(50);
        for _ in 0..50 {
            let cache = cache.clone();
            let calls = calls.clone();
            handles.push(tokio::spawn(async move {
                cache
                    .get_or_compute(1u32, || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        // Just enough delay for other tasks to enqueue.
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Ok::<_, std::convert::Infallible>("v")
                    })
                    .await
                    .unwrap()
            }));
        }
        for h in handles {
            assert_eq!(h.await.unwrap(), "v");
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "fetcher must run exactly once across 50 concurrent missers"
        );
    }

    /// Singleflight on errors must NOT permanently poison the slot:
    /// after a failed fetch, the next caller's fetcher takes over.
    #[tokio::test]
    async fn singleflight_retries_after_fetcher_error() {
        let cache: Arc<Cache<u32, &'static str>> = Arc::new(Cache::new(
            "sf-err",
            Duration::from_secs(60),
        ));
        let attempts = Arc::new(AtomicUsize::new(0));

        // First call: fetcher fails.
        let res1: Result<&'static str, &'static str> = cache
            .get_or_compute(1u32, {
                let attempts = attempts.clone();
                || async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err("boom")
                }
            })
            .await;
        assert!(res1.is_err());

        // Second call: a fresh fetcher runs and succeeds.
        let res2: Result<&'static str, &'static str> = cache
            .get_or_compute(1u32, {
                let attempts = attempts.clone();
                || async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Ok("v")
                }
            })
            .await;
        assert_eq!(res2.unwrap(), "v");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    /// Different keys never share a singleflight slot.
    #[tokio::test]
    async fn singleflight_does_not_collapse_distinct_keys() {
        let cache: Arc<Cache<u32, &'static str>> = Arc::new(Cache::new(
            "sf-keys",
            Duration::from_secs(60),
        ));
        let calls = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::with_capacity(10);
        for k in 0..10u32 {
            let cache = cache.clone();
            let calls = calls.clone();
            handles.push(tokio::spawn(async move {
                cache
                    .get_or_compute(k, || async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok::<_, std::convert::Infallible>("v")
                    })
                    .await
                    .unwrap()
            }));
        }
        for h in handles {
            let _ = h.await;
        }
        assert_eq!(
            calls.load(Ordering::SeqCst),
            10,
            "each distinct key gets its own fetcher invocation"
        );
    }
}
