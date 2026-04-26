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

/// Object-safe trait so the registry can hold `Arc<dyn CacheInvalidate>`
/// across heterogeneous `Cache<K, V>` types. Per-key invalidation is
/// not exposed here because the registry path is always full-clear; if
/// a future caller needs targeted clears it should hold an
/// `Arc<Cache<K, V>>` directly and call `invalidate_key`.
pub trait CacheInvalidate: Send + Sync {
    fn invalidate(&self);
}

/// In-memory cache with TTL and tag-based invalidation.
///
/// Multi-key (HashMap-backed): different keys cache independently. Each
/// entry has its own TTL deadline measured from when it was stored.
/// `invalidate()` clears all entries; `invalidate_key(k)` clears one.
///
/// **Not bounded.** Entries accumulate without LRU eviction. Acceptable
/// for the current call sites (single-key TTL caches, range-keyed
/// caches that thrash naturally, and `block_ts_cache` whose worst case
/// is ~24MB at full chain). The A-era refactor adds bounding.
///
/// **No singleflight.** A concurrent miss runs the fetcher twice;
/// whichever finishes second overwrites. Acceptable at current traffic;
/// A adds proper singleflight.
pub struct Cache<K, V> {
    inner: Mutex<HashMap<K, (V, Instant)>>,
    ttl: Duration,
    tags: Vec<CacheTag>,
}

impl<K, V> Cache<K, V>
where
    K: Eq + Hash + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Construct a cache that expires entries after `ttl`. Use
    /// `Cache::permanent()` for caches over immutable data
    /// (e.g. block timestamps).
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            ttl,
            tags: Vec::new(),
        }
    }

    /// Construct a cache that never expires by TTL. Still respects
    /// explicit `invalidate()` calls. For data that is genuinely
    /// immutable (block hashes, historical timestamps).
    pub fn permanent() -> Self {
        Self::new(Duration::MAX)
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
    /// Generic over the fetcher's error type so each call site keeps
    /// its native error (`StatsError`, `ServerFnError`, etc.) without
    /// wrapping. The `E: Send + 'static` bound is forward-looking: a
    /// future singleflight implementation will share an in-flight
    /// future across tasks, which requires a sendable error type.
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
        // Read fast path. Lock is dropped before the await below.
        {
            let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((v, ts)) = guard.get(&key) {
                if ts.elapsed() < self.ttl {
                    return Ok(v.clone());
                }
            }
        }
        // Cache miss or stale. Fetch with the lock RELEASED.
        let fresh = fetcher().await?;
        // A concurrent miss may have filled this entry in the meantime;
        // we just overwrite. A's singleflight will dedupe upstream.
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(key, (fresh.clone(), Instant::now()));
        Ok(fresh)
    }

    /// Raw read. Returns the cached value for `key` if present and
    /// within TTL, else `None`. For call sites that conditionally cache
    /// (e.g. only insert positive lookup results) and need direct
    /// cache access rather than `get_or_compute`'s fetcher pattern.
    pub fn get(&self, key: &K) -> Option<V> {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.get(key).and_then(|(v, ts)| {
            if ts.elapsed() < self.ttl {
                Some(v.clone())
            } else {
                None
            }
        })
    }

    /// Raw write. Stores or overwrites the entry for `key` with
    /// the current timestamp. Pairs with `get` for conditional caching.
    pub fn insert(&self, key: K, value: V) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.insert(key, (value, Instant::now()));
    }

    /// Clear the entry for `key` if present. No-op otherwise.
    pub fn invalidate_key(&self, key: &K) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.remove(key);
    }
}

impl<K, V> CacheInvalidate for Cache<K, V>
where
    K: Send + Sync + 'static,
    V: Send + Sync + 'static,
{
    fn invalidate(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.clear();
    }
}

/// Routes `invalidate(tag)` calls to all caches registered under that
/// tag. Caches register at construction via `StatsStateBuilder::cache`,
/// so registration is not optional and a cache cannot ship without an
/// invalidation contract.
#[derive(Default)]
pub struct CacheRegistry {
    subscribers: HashMap<CacheTag, Vec<Arc<dyn CacheInvalidate>>>,
}

impl CacheRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe a cache to a tag. The builder calls this once per tag
    /// the cache was constructed with; direct callers should generally
    /// go through the builder rather than registering by hand.
    pub fn subscribe(
        &mut self,
        tag: CacheTag,
        cache: Arc<dyn CacheInvalidate>,
    ) {
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
            Cache::new(Duration::from_secs(60));
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
            Cache::new(Duration::from_millis(10));
        let calls = Arc::new(AtomicUsize::new(0));

        let _ = fetch(&cache, 1, "a", &calls).await;
        tokio::time::sleep(Duration::from_millis(25)).await;
        let _ = fetch(&cache, 1, "b", &calls).await;

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn different_keys_cache_independently() {
        let cache: Cache<u32, &'static str> =
            Cache::new(Duration::from_secs(60));
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
        let cache: Cache<u32, &'static str> = Cache::permanent();

        assert_eq!(cache.get(&1), None);
        cache.insert(1, "a");
        assert_eq!(cache.get(&1), Some("a"));
        // Different key still uncached
        assert_eq!(cache.get(&2), None);
    }

    #[tokio::test]
    async fn raw_get_respects_ttl() {
        let cache: Cache<u32, &'static str> =
            Cache::new(Duration::from_millis(10));
        cache.insert(1, "a");
        assert_eq!(cache.get(&1), Some("a"));
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert_eq!(cache.get(&1), None, "expired entry should not return");
    }

    #[tokio::test]
    async fn permanent_cache_does_not_expire() {
        let cache: Cache<u32, &'static str> = Cache::permanent();
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
            Cache::new(Duration::from_secs(60));
        let calls = Arc::new(AtomicUsize::new(0));

        let _ = fetch(&cache, 1, "a", &calls).await;
        cache.invalidate();
        let _ = fetch(&cache, 1, "b", &calls).await;

        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn invalidate_key_only_clears_matching() {
        let cache: Cache<u32, &'static str> =
            Cache::new(Duration::from_secs(60));
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
            Cache::new(Duration::from_secs(60))
                .invalidated_by(CacheTag::OnNewBlock),
        );
        let price_cache: Arc<Cache<(), &'static str>> = Arc::new(
            Cache::new(Duration::from_secs(60))
                .invalidated_by(CacheTag::OnPriceRefresh),
        );

        let mut registry = CacheRegistry::new();
        registry.subscribe(
            CacheTag::OnNewBlock,
            block_cache.clone() as Arc<dyn CacheInvalidate>,
        );
        registry.subscribe(
            CacheTag::OnPriceRefresh,
            price_cache.clone() as Arc<dyn CacheInvalidate>,
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
            Cache::new(Duration::from_secs(60))
                .invalidated_by(CacheTag::OnNewBlock),
        );
        let mut registry = CacheRegistry::new();
        registry.subscribe(
            CacheTag::OnNewBlock,
            cache.clone() as Arc<dyn CacheInvalidate>,
        );
        // OnPriceRefresh has no subscribers; must not panic
        registry.invalidate(CacheTag::OnPriceRefresh);
    }

    #[tokio::test]
    async fn one_cache_can_subscribe_to_multiple_tags() {
        let cache: Arc<Cache<(), &'static str>> = Arc::new(
            Cache::new(Duration::from_secs(60))
                .invalidated_by(CacheTag::OnNewBlock)
                .invalidated_by(CacheTag::OnPriceRefresh),
        );
        let mut registry = CacheRegistry::new();
        for tag in cache.tags() {
            registry.subscribe(*tag, cache.clone() as Arc<dyn CacheInvalidate>);
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
}
