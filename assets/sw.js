// Service Worker for WE HODL BTC PWA
// Strategy: cache static assets (app shell), network-first for API/pages

var CACHE_NAME = 'wehodlbtc-v1';
// Small assets to pre-cache on install (large files like WASM cache on first use)
// Only precache truly static assets. JS files that change on deploy
// (stats.js, lightbox.js) are cached on first use instead.
var PRECACHE_ASSETS = [
    '/pkg/we_hodl_btc.css',
    '/wasm-fallback.js',
    '/favicon.svg',
    '/favicon-96x96.png'
];

// Install: pre-cache static assets
self.addEventListener('install', function(event) {
    event.waitUntil(
        caches.open(CACHE_NAME).then(function(cache) {
            return cache.addAll(PRECACHE_ASSETS).catch(function() {
                // Non-fatal: some assets may not exist yet during dev
                console.warn('SW: some assets failed to pre-cache');
            });
        })
    );
    // Don't skipWaiting() here — let the page detect the update
    // and show a banner. The page sends a 'SKIP_WAITING' message
    // when the user clicks "Update".
});

// Listen for skip-waiting message from the page
self.addEventListener('message', function(event) {
    if (event.data === 'SKIP_WAITING') {
        self.skipWaiting();
    }
});

// Activate: clean up old caches
self.addEventListener('activate', function(event) {
    event.waitUntil(
        caches.keys().then(function(names) {
            return Promise.all(
                names.filter(function(n) { return n !== CACHE_NAME; })
                     .map(function(n) { return caches.delete(n); })
            );
        })
    );
    self.clients.claim();
});

// Fetch: cache-first for static assets, network-first for everything else
self.addEventListener('fetch', function(event) {
    var url = new URL(event.request.url);

    // Skip non-GET requests
    if (event.request.method !== 'GET') return;

    // Only handle same-origin requests (skip extensions, analytics, CDNs)
    if (url.origin !== self.location.origin) return;

    // Skip API calls and server functions (always need fresh data)
    if (url.pathname.startsWith('/api/')) return;

    // WASM/JS/CSS in /pkg/: network-first (filenames have no cache-busting
    // hash, so we must always check for fresh versions after deploys)
    if (url.pathname.startsWith('/pkg/')) {
        event.respondWith(
            fetch(event.request).then(function(response) {
                if (response.ok) {
                    var clone = response.clone();
                    caches.open(CACHE_NAME).then(function(cache) {
                        cache.put(event.request, clone);
                    });
                }
                return response;
            }).catch(function() {
                return caches.match(event.request);
            })
        );
        return;
    }

    // Other static assets: cache-first (images, fonts, small JS helpers)
    if (PRECACHE_ASSETS.indexOf(url.pathname) !== -1) {
        event.respondWith(
            caches.match(event.request).then(function(cached) {
                return cached || fetch(event.request).then(function(response) {
                    if (response.ok) {
                        var clone = response.clone();
                        caches.open(CACHE_NAME).then(function(cache) {
                            cache.put(event.request, clone);
                        });
                    }
                    return response;
                });
            })
        );
        return;
    }

    // Everything else (HTML pages): network-first with 4s timeout fallback to cache
    var networkWithTimeout = new Promise(function(resolve, reject) {
        var timer = setTimeout(function() { reject(new Error('timeout')); }, 4000);
        fetch(event.request).then(function(response) {
            clearTimeout(timer);
            if (response.ok) {
                var clone = response.clone();
                caches.open(CACHE_NAME).then(function(cache) {
                    cache.put(event.request, clone);
                });
                resolve(response);
            } else {
                // Server error (5xx, 4xx) — fall back to cache
                reject(new Error('HTTP ' + response.status));
            }
        }).catch(function(err) {
            clearTimeout(timer);
            reject(err);
        });
    });
    event.respondWith(
        networkWithTimeout.catch(function() {
            return caches.match(event.request);
        })
    );
});
