// Service Worker for WE HODL BTC PWA
// Strategy: cache static assets (app shell), network-first for API/pages

var CACHE_NAME = 'wehodlbtc-v1';
// Small assets to pre-cache on install (large files like WASM cache on first use)
var PRECACHE_ASSETS = [
    '/pkg/we_hodl_btc.css',
    '/stats.js',
    '/lightbox.js',
    '/sections.js',
    '/jsonld.js',
    '/wasm-fallback.js',
    '/favicon-32x32.png',
    '/favicon-16x16.png'
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

    // Static assets: cache-first (fast repeat loads, cached on first use)
    if (url.pathname.startsWith('/pkg/') ||
        PRECACHE_ASSETS.indexOf(url.pathname) !== -1) {
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

    // Everything else (HTML pages): network-first with cache fallback
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
});
