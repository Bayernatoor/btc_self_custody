# Tasks

## Completed: v2 Major Overhaul (2026-03-19)

### OP_RETURN Classification
- [x] classifier.rs — Runes (6a5d prefix + ≤6 byte), DataCarrier (>6 byte), SegWit commit (excluded)
- [x] Per-block breakdown: runes_count/bytes, data_carrier_count/bytes
- [x] Stacked area charts with v28/v30 annotation lines
- [x] Runes dominance % chart

### BIP Signaling Tracker
- [x] BIP-110: version bit 4 detection
- [x] BIP-54: coinbase locktime == height-1 detection (NOT version bits)
- [x] Block grid visualization (color-coded signaling)
- [x] Historical periods chart with threshold line
- [x] Period count matches mempool.space convention (excludes retarget block)
- [x] Adaptive decimal precision for small percentages

### Mining Pool Identification
- [x] Coinbase scriptSig ASCII matching (case-insensitive)
- [x] Fallback to coinbase OP_RETURN outputs
- [x] 30+ pools: Foundry, AntPool, ViaBTC, F2Pool, MARA, OCEAN, WhitePool, etc.

### Fee Data
- [x] Total fees per block (coinbase output - subsidy)
- [x] Median fee and median fee rate (from per-tx fee data, requires txindex)
- [x] Next-block fee estimate (estimatesmartfee RPC)

### Frontend Overhaul
- [x] Plotly → ECharts (better performance with 100k+ data points)
- [x] Tab-based layout: Overview, Network, Fees, Mining, OP_RETURN, Signaling
- [x] Time range presets (1D/1W/1M/3M/6M/1Y/2Y/5Y/10Y/ALL)
- [x] Daily aggregation for large ranges (>5k blocks)
- [x] Block detail sidebar with click-to-inspect
- [x] Mobile responsive
- [x] Moving averages on all charts
- [x] Dark navy theme, orange data lines, fullscreen chart overlay

### Infrastructure
- [x] Parallel ingestion (16 concurrent, ~15 blocks/sec at verbosity=2)
- [x] ALTER TABLE migrations (no DB deletion)
- [x] 60-second new block polling
- [x] Backfill: extras (descending) + backward to genesis
- [x] Lock yielding for API responsiveness during backfill
- [x] Price caching (60s from mempool.space)
- [x] Network hashrate (getnetworkhashps RPC)

## Completed: Chart Overlays (2026-03-23)

### Overlay System
- [x] Floating toggle panel (bottom-left, always visible while scrolling)
- [x] Halvings overlay — orange dashed vertical lines at each halving
- [x] BIP Activations overlay — teal dotted lines (P2SH, Strict DER, CLTV, CSV, SegWit, Taproot)
- [x] Bitcoin Core Releases overlay — purple dotted lines (v0.1 through v30)
- [x] Price (USD) overlay — gold line on secondary right Y-axis
- [x] All overlays cumulative (can combine freely)
- [x] Applied to all time-series charts across all tabs

### Price Overlay
- [x] Historical daily prices from blockchain.info API (full history, cached 1hr server-side)
- [x] Live price from mempool.space appended to fill blockchain.info's ~1 week data lag
- [x] Interpolation for daily/category charts (binary search + linear interpolation)
- [x] Filtered to chart's visible time range (Y-axis scales correctly)
- [x] Toolbox buttons repositioned when price axis is active

### New Charts (2026-03-23)
- [x] Empty blocks chart — monthly bar chart (refactored from per-miner scatter)
- [x] Witness Version Comparison — stacked area (SegWit v0 blue + Taproot v1 green)
- [x] Witness Version Share — 100% stacked area (v0 vs v1 % of witness spends)
- [x] Transaction Type Breakdown — 100% stacked area (Legacy grey + v0 blue + v1 green)
- [x] OP_RETURN Block Share — OP_RETURN bytes as % of total block size
- [x] Chain Size Growth — cumulative blockchain size in GB

## Known Issues
- [ ] Price overlay on ALL range sometimes doesn't render on first toggle (needs range switch)
- [ ] Price overlay on 1D range has no data (daily granularity too coarse for per-block)
- [ ] Price tooltip doesn't show in per-block mode (timestamp mismatch between block and daily price data)

## Completed: Embedded Data Tab Restructure (2026-03-23)
- [x] Renamed OP_RETURN tab to "Embedded Data"
- [x] Added Omni Layer classifier (OP_RETURN prefix `6f6d6e69` / "omni")
- [x] Added Counterparty classifier (OP_RETURN prefix `434e545250525459` / "CNTRPRTY")
- [x] Schema migration: added omni_count, omni_bytes, counterparty_count, counterparty_bytes columns
- [x] Updated insert/update/query statements, daily aggregation, types, server functions
- [x] Bumped BACKFILL_VERSION to 3 (triggers re-parse of all blocks)

## Completed: Block Parser v4/v5 + New Charts (2026-03-23)
- [x] Schema migration v4: output types (p2pk/p2pkh/p2sh/p2wpkh/p2wsh/p2tr/multisig/unknown),
      input_count, output_count, rbf_count, witness_bytes
- [x] Schema migration v5: inscription_count, inscription_bytes
- [x] Block parser: single-pass tx iteration, output type classification via scriptPubKey.type,
      RBF detection (nSequence < 0xFFFFFFFE), witness bytes counting, inscription envelope detection
- [x] Parser optimizations: .take() vs .clone(), pre-allocated vecs, identify_miner returns &'static str
- [x] Charts: Address Type Evolution, Witness Data Share, RBF Adoption, UTXO Flow,
      Ordinals Inscriptions, Inscription Block Share
- [x] Production hardening: mutex lock poisoning recovery, serde panic elimination, backfill index

## Completed: Embedded Data Charts Expansion (2026-03-24)
- [x] Unified "All Embedded Data Block Share" — OP_RETURN + inscription bytes as % of block
- [x] Unified count chart — 6 protocols stacked (Runes, Omni, XCP, Other, Inscriptions, Stamps)
- [x] Unified volume chart — same protocols by bytes
- [x] BRC-20 detection — hex pattern match for {"p":"brc-20" in inscription witness data
- [x] Stamps tracking via multisig_count in unified charts
- [x] Full OP_RETURN sub-counts added to BlockSummary for unified data access

## Future Ideas: Deep Linking
- [ ] URL-based tab selection (e.g. /stats?tab=opreturn or /stats#embedded-data)
- [ ] URL-based sub-section selection (e.g. /stats?tab=opreturn&section=protocols)
- [ ] Protocol Guide CTA links directly to the correct tab+section
- [ ] Shareable URLs for specific chart views

## Completed: Protocol Guide Page (2026-03-24)
- [x] Standalone page at /stats/learn/protocols
- [x] Vertical chronological timeline with colored dots
- [x] Click protocol → scrolls to detailed card with description, how it works, fun fact
- [x] Comparison table with prunable/fungible properties
- [x] Colored Coins historical note
- [x] All descriptions reviewed for technical accuracy
- [x] Link from Embedded Data tab to protocol guide
- [ ] Live stats from DB (e.g. "X total Runes outputs since launch") — future enhancement

## Future Ideas: CSV Data Export
- [ ] Download button on each ChartCard (icon next to expand button)
- [ ] Generate CSV client-side in WASM from the chart's existing data
- [ ] Include headers matching the chart series names
- [ ] Filename format: `{chart_name}_{range}_{mode}.csv` (e.g. `block_size_1y_daily.csv`)
- [ ] Per-block mode: timestamp, height, value columns
- [ ] Daily mode: date, value columns
- [ ] Consider a "Download All" option per tab for bulk export

## Completed: Branding
- [x] Renamed from "Chain Pulse" to "The Bitcoin Observatory"

## Completed: Sub-section Navigation (2026-03-24)
- [x] Horizontal pill navigation within Network, Embedded Data, and Mining tabs
- [x] Network: [Blocks] [Adoption] [Transactions]
- [x] Embedded Data: [Overview] [OP_RETURN Protocols] [Witness Embedding]
- [x] Mining: [Difficulty] [Pool Distribution]

## Completed: Data Accuracy Verification (2026-03-24)
- [x] Witness version unit mismatch FIXED — charts now use output counts (p2wpkh+p2wsh, p2tr) consistently
- [x] Omni/Counterparty classifier FIXED — offset-based matching instead of substring search
- [x] Address type totals verified — sum matches output_count minus nulldata (expected gap, documented)
- [x] Chain size ratio acknowledged as approximate (labeled "est." in chart)
- [x] RBF detection verified as BIP 125 correct (BIP 68 overlap is protocol-level, documented)
- [x] Inscription detection pattern verified — false positive probability negligible (~10^-15 per position)

### Known Limitations (documented, not bugs)
- Inscription byte estimate uses fixed 10-byte overhead deduction (actual overhead varies ~11-35 bytes)
- RBF chart counts BIP 68 CSV transactions as RBF signals (protocol ambiguity, not fixable without prevout data)
- Address type chart excludes nulldata, multisig, and unknown outputs from stacked area
- Chain size disk ratio estimate is less accurate for early historical blocks

## Future Ideas: Niche Chart Ideas
- [ ] Block fullness distribution — histogram of weight utilization %, shows miner behavior
- [ ] Fee pressure vs block space — correlate fee rates with weight utilization
- [ ] Coinbase message trends — what miners write in coinbase scripts over time
- [ ] Time between halvings — actual vs expected (shows hashrate impact on schedule)
- [ ] Script complexity — average inputs per tx over time (batching/consolidation trends)
- [ ] Empty block rate by pool — which pools mine empty blocks most often
- [ ] Block propagation proxy — time gap between consecutive blocks (< 1 min = likely same miner)

## Completed: Overview/Mining Dashboard (2026-03-24)
- [x] Previous difficulty change % in Mining live panel
- [x] Average block time in Mining live panel

## Completed: Dashboard UX Polish (2026-03-24)
- [x] Label readability: text-white/50 → #8899aa, 0.65rem → 0.7rem across LiveCard, panels, gauge
- [x] 30s refresh flicker fix: cached_live signal preserves previous data during refetch
- [x] Loading state: "Connecting to node..." spinner while availability check is pending
- [x] SSR SendWrapper panic fix: availability check stays as LocalResource (client-only)
- [x] Responsive sizing: consistent lg:/xl: breakpoints across all pages matching home page
- [x] Price overlay: cached history, loading spinner, instant subsequent toggles
- [x] Overlay panel: 25% larger, z-index above fullscreen charts
- [x] Toolbox positioning: dynamically shifts left based on active right-side axes
- [x] Two-stage chart pipeline: base chart cached, overlay toggle skips chart rebuild
- [x] Tab-guarded chart signals: only active tab recomputes on overlay/data change
- [x] Chain size chart: disk estimate hidden on short ranges, threshold lowered to 20GB

## TODO: Chain Size Growth Chart
- [ ] Investigate chain size growth chart — something still isn't right with the cumulative
      calculation and/or disk size estimate on various ranges. Needs thorough review.

## TODO: Thorough Chart Review
- [ ] Review every chart across all tabs for correctness, visual consistency, and UX
  - Verify data accuracy on all range presets (1D through ALL)
  - Check tooltip formatting and values
  - Verify overlay interactions (price, chain size, halvings, BIPs, etc.)
  - Check mobile responsiveness of charts
  - Verify fullscreen mode works correctly for each chart
  - Review axis labels, legends, and color consistency
  - Check moving averages are computed correctly
  - Verify daily aggregation mode produces correct values
  - Test edge cases: empty data, single data point, very large ranges

## Completed: Stats Route Performance Optimization (2026-03-25)

### Server-Side (scalability for 100+ concurrent users)
- [x] Replace `Mutex<Connection>` with r2d2 SQLite connection pool (8 connections, WAL mode)
- [x] Cache live stats server-side (10s TTL) — 4 RPC calls shared across all clients
- [x] Cache signaling periods (60s TTL) — avoids 900k-row full table scan per request
- [x] Price cache stampede guard — AtomicBool prevents duplicate HTTP fetches on expiry
- [x] Cache-Control headers on REST API responses (/api/stats, /api/live — 10s)

### Database Query Optimization
- [x] `query_stats()`: replaced `COUNT(*)` full scan with `MIN/MAX` primary key lookups (600ms → <1ms)
- [x] `query_signaling_periods_*()`: cached with 60s TTL (full scan only on first request)
- Audited all 19 query functions — remaining queries use indexes correctly:
  - `max_height/min_height`: O(1) B-tree lookups
  - `query_blocks/query_op_returns`: PRIMARY KEY range scans
  - `query_daily_aggregates`: timestamp index + GROUP BY
  - `query_block_by_height/query_block_timestamp`: O(log N) point lookups
  - `query_miner_dominance*`: indexed range scans with grouping
  - `query_signaling_bit/locktime`: PRIMARY KEY range scans
  - `insert_blocks/update_block_extras`: batch transactions, prepared statements

### Client-Side (chart rendering performance)
- [x] Two-stage chart pipeline: base JSON cached separately from overlay application
- [x] Tab-guarded chart signals: only active tab recomputes on overlay/data change
- [x] Eliminated redundant `op_data` fetch — Embedded Data tab uses shared `dashboard_data`
- [x] OP_RETURN + mining chart signals: tab-guarded (no wasted computation)
- [x] Pre-cached chain_size cumulative data (no recompute on overlay toggle)
- [x] `overlay_flags.read()` instead of `.get()` — avoids cloning 4000+ price points per chart

### Remaining Ideas
- [ ] Evaluate ECharts `notMerge` vs merge behavior on option updates
- [ ] Consider reduced data density (more aggressive sampling for large ranges)
- [ ] Consider client-side caching of server responses across range switches

## Completed: Codebase Refactoring (2026-03-24)
- [x] Split `charts.rs` (3239 lines) into 9 module files:
  - `charts/mod.rs` — helpers, constants, colors, build_option, apply_overlays
  - `charts/network.rs` — block size, tx count, interval, weight, chain size, difficulty
  - `charts/adoption.rs` — segwit, taproot, witness versions, address types, witness share
  - `charts/fees.rs` — fees, subsidy vs fees
  - `charts/embedded.rs` — OP_RETURN protocols, inscriptions, block share
  - `charts/mining.rs` — miner dominance, empty blocks
  - `charts/tx_metrics.rs` — RBF, UTXO flow
  - `charts/gauges.rs` — mempool gauge
- [x] Split `routes/stats.rs` (1890 lines) into module:
  - `routes/stats/mod.rs` — main page logic and views (1696 lines)
  - `routes/stats/components.rs` — Chart, ChartCard, LiveCard (144 lines)
  - `routes/stats/helpers.rs` — format_number, range_to_blocks (50 lines)
- [ ] Extract overlay constants (halvings, BIPs, Core releases, events) into a data file
- [ ] Consider a chart registry pattern to reduce signal boilerplate

## Future Ideas
- [ ] Fee rate distribution charts (percentiles)
- [ ] Mempool fee histogram
- [ ] More BIP proposals as they emerge
- [ ] Connection pooling (r2d2-sqlite or PostgreSQL)
- [ ] Hourly price data source for 1D price overlay

## Future Ideas: Main Website
- [ ] KYC-Free / Privacy section (new top-level page)
  - Privacy-focused wallet recommendations (Sparrow, Samourai, Wasabi, etc.)
  - CoinJoin guides and best practices
  - PayJoin explainers and wallet support
  - KYC-free acquisition methods (P2P exchanges, ATMs, mining)
  - UTXO management and coin control
  - Tor/VPN usage with Bitcoin
  - Address reuse prevention
  - Lightning privacy considerations
  - Name TBD — "KYC-Free", "Privacy Guide", etc.
