# Lessons Learned

_Updated as corrections and patterns are captured._

## 2026-03-17: Repo structure
- **Mistake:** Initialized git in the parent `bitcoin/` directory instead of the project subdirectory.
- **Rule:** `bitcoin/` is an umbrella folder, NOT a repo. Each project (e.g. `btc_self_custody/`) gets its own repo.

## 2026-03-18: Never delete the DB
- **Mistake:** Dropped and re-ingested the database multiple times for schema changes.
- **Rule:** Use ALTER TABLE migrations + backfill. Ingestion takes hours — keep existing data.

## 2026-03-19: BIP-54 does NOT use version bits
- **Mistake:** Initially implemented BIP-54 signaling as version bit 19.
- **Rule:** BIP-54 signals via coinbase nLockTime == height - 1. Always verify the actual signaling mechanism for each BIP.

## 2026-03-19: Retarget period counting
- **Mistake:** Counted blocks in period as `tip - period_start + 1` (inclusive of retarget block).
- **Rule:** "Blocks since adjustment" = `tip - period_start` (excludes retarget block). Matches mempool.space convention.

## 2026-03-19: Version bits get reused
- **Mistake:** BIP-54 signaling chart showed false positives from old proposals that used the same bit position.
- **Rule:** Always filter signaling data by the proposal's start height. Bit positions are recycled across different proposals.

## 2026-03-19: Miner identification is case-insensitive
- **Mistake:** "Mined by AntPool" wasn't matching because we checked for `/AntPool/` (with slashes).
- **Rule:** Lowercase the coinbase ASCII and match against lowercase patterns. Miners format their tags inconsistently.

## 2026-03-19: Backfill ordering matters
- **Mistake:** Backfill processed blocks ascending (oldest first), so recent signaling data took hours to appear.
- **Rule:** Process backfill descending (newest first) so the most relevant data appears quickly.

## 2026-03-19: Browser freezes from too much data
- **Mistake:** Sending 100k+ raw blocks to the browser for charting.
- **Rule:** Use server-side daily aggregation for ranges >5k blocks. Only send raw block data for small ranges.

## 2026-03-19: ECharts legend colors must be explicit
- **Mistake:** Set areaStyle color but not itemStyle/lineStyle, causing legend to show default colors different from the chart fill.
- **Rule:** Always set itemStyle.color and lineStyle.color to match areaStyle on ECharts series.

## 2026-03-19: Mutex contention with background tasks
- **Mistake:** Multiple background tasks (backfill + backward ingestion) running simultaneously, starving API requests.
- **Rule:** Run backfills sequentially, yield the DB lock between batches (50ms sleep).

## 2026-03-23: CoinGecko free API requires auth now
- **Mistake:** Used CoinGecko free API for historical price data — returns 403 Forbidden.
- **Rule:** Use blockchain.info `/charts/market-price?timespan=all&format=json` instead (free, no auth, full history).

## 2026-03-23: JavaScript safe integer limits
- **Mistake:** Passed `u64::MAX` as a Leptos server function argument — JS can't represent it (exceeds `Number.MAX_SAFE_INTEGER`).
- **Rule:** Keep server function numeric arguments below `2^53`. Use a reasonable sentinel like `4_000_000_000` instead of `u64::MAX`.

## 2026-03-23: ECharts category axis requires exact matches
- **Mistake:** Price overlay on daily charts used `[date_string, value]` pairs that didn't match chart category strings exactly.
- **Rule:** For category-axis charts, map overlay data to the chart's exact category list. Use interpolation for sparse external data.

## 2026-03-23: blockchain.info data has ~1 week lag
- **Mistake:** Price overlay showed gap at the end of charts because blockchain.info historical data lags ~1 week.
- **Rule:** Supplement historical price data with live spot price to fill the gap to the present.

## 2026-03-23: Leptos reactive signals cause unintended chart resets
- **Mistake:** Reading `live.get()` inside a chart signal caused all charts to re-render every 30 seconds when live stats refreshed, resetting zoom/pan state.
- **Rule:** Use `read_untracked()` when you need a value for computation but don't want reactive updates. Only `.get()` signals that should trigger re-renders.

## 2026-03-23: build_option() shallow merge wipes nested defaults
- **Mistake:** ECharts legend text color disappeared because `"legend": { "show": true }` replaced the entire default legend object (including textStyle, position).
- **Rule:** Deep-merge specific keys (like `legend`) in `build_option()` so partial overrides don't wipe out defaults. Or always include the full nested structure when overriding.

## 2026-03-23: BIP activation dates vs timestamps must be verified
- **Mistake:** Manually entered BIP activation dates were off by 3-10 days from the actual block timestamps.
- **Rule:** Always verify date strings against timestamps programmatically (`python3 datetime.fromtimestamp()`). Never trust manually converted dates.

## 2026-03-23: size_on_disk ≠ sum of block sizes
- **Mistake:** Chain size chart showed 728GB while the overview showed 829.7GB.
- **Rule:** Bitcoin Core's `size_on_disk` includes block files + undo files + indexes. Raw block size sum is ~85% of on-disk size. Show both metrics when possible.

## 2026-03-23: Sparse external data needs interpolation for category charts
- **Mistake:** blockchain.info returns ~1 data point every 3.5 days. Exact date matching on daily category charts produced mostly nulls (dots instead of lines).
- **Rule:** Use binary search + linear interpolation between surrounding data points for smooth overlay lines on category-axis charts.

## 2026-03-24: Don't mix transaction counts with output counts
- **Mistake:** Witness version charts subtracted `taproot_spend_count` (outputs created) from `segwit_spend_count` (transactions with witness). These are different units — the result is meaningless.
- **Rule:** When comparing v0 vs v1, use output type counts from the same unit: `p2wpkh_count + p2wsh_count` for v0, `p2tr_count` for v1. Never mix tx-level and output-level metrics in the same calculation.

## 2026-03-24: Substring search in protocol classifiers causes false positives
- **Mistake:** Omni classifier used `payload.contains("6f6d6e69")` which matches "omni" anywhere in the OP_RETURN data, not just at the protocol-defined offset.
- **Rule:** Protocol markers appear at specific offsets after the push-length opcode. Use `starts_with` at the correct offset (2 for OP_PUSHBYTES, 4 for OP_PUSHDATA1, 6 for OP_PUSHDATA2).

## 2026-03-24: ECharts try/catch brace alignment matters
- **Mistake:** Click handler registration was outside the try block due to mismatched braces — could execute on a broken chart instance after a parse error.
- **Rule:** Always verify brace alignment in JS try/catch blocks. The catch closes the try block; any code after catch runs unconditionally.

## 2026-03-24: Retry loops need exit conditions
- **Mistake:** ECharts loading retry used `setTimeout` without a retry limit — infinite loop if CDN fails.
- **Rule:** Always cap retries (e.g., 25 attempts × 200ms = 5 seconds max). Log a warning when giving up.

## 2026-03-29: Inline `<style>` tags break Leptos hydration
- **Mistake:** Added `<style>` tag with CSS content inside a Leptos `view!` macro for nav spinner styles.
- **Rule:** Never put inline `<style>` or `<script>` content in Leptos view macros. The SSR output won't match what the WASM expects during hydration, silently breaking the entire reactive system. Move all CSS to `style/tailwind.css` and all JS to external `.js` files.

## 2026-03-29: Service worker cache-first breaks deploys for unhashed files
- **Mistake:** SW used cache-first strategy for `/pkg/` files (WASM, JS, CSS). After a deploy with new code, the SW served stale cached WASM, causing hydration failures. Required hard refresh.
- **Rule:** Use network-first for files without cache-busting hashes in their filenames. Leptos build output (`we_hodl_btc.wasm`, `we_hodl_btc.css`) has no hash suffix, so the SW must always check the network first. Cache-first is only safe for files with content hashes in the name.

## 2026-03-29: SW precache list shouldn't include frequently-changing files
- **Mistake:** Precached `stats.js`, `lightbox.js`, etc. — files that change on every deploy. Old versions served from SW cache after deploys.
- **Rule:** Only precache truly static assets (favicons, etc.). Let frequently-changing JS files cache on first use with network-first strategy.

## 2026-03-29: Leptos router updates pathname synchronously on `<a>` click
- **Mistake:** Tried using reactive signals + `<Show>` to display a loading spinner on nav tabs. The spinner never appeared because Leptos router updates `location.pathname` in the same reactive batch as the click handler, so the loading state was set and cleared before the browser could paint.
- **Rule:** For visual feedback on navigation, use DOM manipulation (classList.add) + `preventDefault()` + deferred programmatic navigation via `requestAnimationFrame`. This forces a paint frame between showing the loading state and starting navigation. CSS `::after` pseudo-elements are better than child elements for spinners (no DOM lifecycle issues).

## 2026-03-29: Effect timing races with Signal::derive for cache invalidation
- **Mistake:** Used a separate `Effect` to clear a chart cache when range changed. The Effect and Signal::derive both reacted to `range.get()`, but the derive ran first (returning stale cached data), then the Effect cleared the cache too late.
- **Rule:** Don't use Effects to invalidate caches that Signal::derive reads. Instead, include all invalidation keys (range, overlay state, data fingerprint) directly in the cache key. This way stale entries are never matched regardless of execution order.

## 2026-03-29: Signal::derive cache must track all dependencies
- **Mistake:** Chart cache returned early on cache hit without calling `$data.get()`, so the derive didn't register `dashboard_data` as a reactive dependency. When new data arrived after a range change, the derive never re-ran.
- **Rule:** In a Signal::derive with caching, ALWAYS read all reactive dependencies (`.get()`) before checking the cache. Even if you return from cache, the dependency tracking happens at the point of `.get()`, not at the point of using the value.

## 2026-03-29: ECharts click events don't fire on line charts with `symbol: "none"`
- **Mistake:** Registered ECharts `click` event handler expecting data point clicks on line charts. Clicks never fired because `symbol: "none"` removes the visible data point markers.
- **Rule:** For line charts with hidden symbols, use `chart.getZr().on('click')` (zrender-level click) + `chart.containPixel('grid', point)` + `chart.convertFromPixel()` to find the nearest data point via binary search. ECharts native click events only work on visible elements (scatter dots, bar segments, pie slices).

## 2026-03-29: ECharts tooltip formatter must handle both axis types
- **Mistake:** Custom tooltip formatter used `new Date(first.data[0])` for the header timestamp. Works for time-axis charts (per-block) but breaks on category-axis charts (daily aggregates) where `data[0]` is an index, producing "1/1/1970".
- **Rule:** Check `first.axisType === 'xAxis.category'` and use `first.axisValueLabel` (the formatted label ECharts already computed) for category axes. Only parse timestamps for time axes.

## 2026-03-30: Double serialization is a major WASM performance bottleneck
- **Mistake:** Chart functions serialized JSON via `build_option()→to_string()`, then `apply_overlays()` parsed it back with `from_str()`, mutated, and serialized again with `to_string()`. Two full serialize + one parse per chart.
- **Rule:** Pass `serde_json::Value` through the pipeline. Serialize to String only once at the very end. Changed `build_option` and all 68 chart functions to return `Value`, and `apply_overlays` to take `&mut Value`. Single serialize per chart.

## 2026-03-30: Show loading state for slow operations, not all operations
- **Mistake:** Initially showed loading skeleton on every range change. Fast ranges (1D-1Y) flickered briefly with the skeleton unnecessarily.
- **Rule:** Only show loading indicators for operations that take >200ms. Check the range size (`range_to_blocks() >= 105_120` for 2Y+) before setting loading state. Fast operations should feel instant.

## 2026-03-30: Chain size chart needs absolute offset, not zero-based cumulative
- **Mistake:** Chain size chart started cumulative sum from 0 for every range. On 3M range, the chart showed 0-15 GB (just the growth) instead of 630-645 GB (actual chain size).
- **Rule:** For cumulative charts, query the starting offset from the server (`SELECT SUM(size) FROM blocks WHERE height < start_height`). Add the offset to the running total so the Y-axis shows absolute values. For ALL range, offset is 0 (correct). For shorter ranges, offset positions the chart at the right scale.

## 2026-03-30: RBF percentage can exceed 100% from parser bugs
- **Mistake:** Some blocks stored `rbf_count > tx_count` due to an older parser version that counted RBF per-input instead of per-transaction.
- **Rule:** Always clamp percentage calculations: `pct.min(100.0)`. Defensive clamping prevents visual anomalies from corrupted historical data. Fix the data via backfill, but the chart should never show impossible values regardless.

## 2026-03-30: Bitcoin block data is immutable — leverage this for performance
- **Insight:** Any block 6+ confirmations deep will never change. A chart computed for blocks 0-940,000 today produces the exact same output tomorrow.
- **Rule:** Pre-compute and cache data for the immutable portion of the chain. Only compute the fresh tip (~144-1008 blocks) on the fly. This enables per-block resolution on ALL range (940k+ clickable data points) without recomputing from scratch every time.

## 2026-03-30: IntersectionObserver is simpler via JS than Rust/wasm-bindgen
- **Mistake:** Initially planned to use `web_sys::IntersectionObserver` which requires adding Cargo features, complex `Closure` management, and `forget()` for leaked closures.
- **Rule:** For browser APIs that need callback management, a small JS helper function (called via `wasm_bindgen extern`) is simpler and avoids Cargo dependency bloat. The `setChartOptionLazy` pattern: JS manages the observer + pending state, Rust just calls a single function.
