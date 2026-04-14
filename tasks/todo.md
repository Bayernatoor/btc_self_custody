# Tasks

---

## Known Bugs

- [ ] **Chain size offset on shared custom URL** — opening a custom range shareable link
      may not apply chain size offset correctly on first load (signal init race condition)
- [ ] **Daily aggregate timestamp display** — chain size overlay uses noon UTC for daily
      timestamps, causing tooltip to show previous day. Audit noon conversion and
      consider using date labels instead of timestamps for category charts.
- [ ] **Price overlay on custom date ranges** — reported as showing all-time data instead
      of filtering to selected range. Code logic looks correct (clips by chart bounds).
      Needs exact repro steps to investigate further.
- [ ] **Largest Transaction chart click-to-detail** — cursor appears but block detail modal
      may not open. Click handler uses zrender click with binary search for nearest
      data point. Needs testing — may already be fixed by component-level signal refactor.

---

## Data & Backfill

- [x] **Backfill v10** — 9 new columns added. BACKFILL_VERSION bumped to 10.
      Bug: update_block_extras was missing v10 columns, so backfill computed
      values but never wrote them to DB.
- [x] **Backfill v11** — Fixed update_block_extras to include all v10 columns +
      inscription_envelope_bytes. BACKFILL_VERSION bumped to 11. Running now.
- [ ] **Remove Coming Soon overlays** — once v11 backfill completes, remove
      `coming_soon=true` from Fee Rate Bands, Max TX Fee, Protocol Fee Revenue,
      and TX Type Evolution charts.
- [ ] **Node Mempool Config** — set minrelaytxfee=0.00000100 (0.1 sat/vB), increase maxmempool

---

## Production & Infrastructure

- [ ] **Sudoers path fix on Droplet** — deploy verify step fails
- [ ] **Dual-source failover for LiveStats** — own node primary, mempool.space fallback
- [ ] **Nginx static file serving** — serve static files directly from nginx
- [ ] **Zero-downtime deploy** — start new binary on temp port, health-check, swap nginx
- [ ] **Hardware upgrade** — replace Raspberry Pi 4 with Intel N100 mini PC + NVMe

---

## Charts & Features

- [ ] **Avg Transaction Size by Type** — breakdown by Legacy/P2SH/SegWit v0/Taproot
- [ ] **Deep Linking** — Protocol Guide CTA links to correct Observatory section
- [ ] **Protocol Guide live stats** — pull real counts from DB
- [ ] **Fix Stamps detection** — needs Counterparty protocol decoding. Significant lift.
- [ ] **Highest Value Block record** — add "Highest Output Value" as an extreme record
- [ ] **Performance review (flat pages)** — first load computes all ~30 charts on network
      page. chart_memo! cache helps on subsequent changes. May need lazy signal
      evaluation if users report lag.

---

## UX & Design

- [x] **Chart sidebar navigation** — drawer with hierarchical index, cross-page navigation
- [x] **Flatten chart page sections** — all charts in one scrollable list per page
- [x] **Fullscreen chart exits on range change** — fixed by moving chart signals to
      component level so they persist across data refetches
- [x] **Info icon tooltips on charts** — 33 charts with clickable (i) explanation panels
- [x] **Histogram count/% toggle** — Block Fullness and Block Time distribution charts
- [ ] **Period comparison** — two custom ranges side-by-side on stats page
- [ ] **Block DNA** — deterministic visual fingerprint per block

---

## Performance Optimization

- [ ] Pre-computed chart snapshots (compact binary in chart_snapshots table)
- [ ] Cache query_blocks for common ranges with short TTL
- [ ] Client-side caching of server responses across range switches
- [ ] **Split shared.rs God file (1,550 lines)** — into state.rs (ObservatoryState +
      create_observatory_state), url_sync.rs (URL query param helpers), range.rs
      (RangeSelector + FloatingRangePicker), drawer.rs (ChartDrawer + drawer_pages).
      Keep shared.rs as re-export hub. Needs careful testing due to Leptos hydration sensitivity.
- [ ] **Split db.rs God file (2,500+ lines)** — into schema.rs (init_schema + migrations),
      queries.rs (query_blocks, query_stats, query_daily_aggregates), mining_queries.rs
      (miner_dominance, empty_blocks), insert.rs (insert_blocks, update_block_extras).
      Keep db.rs as re-export hub.
- [ ] **Server-side price history range filtering** — fetch_price_history currently returns
      all 16 years regardless of range. Add from_ts/to_ts params for 80% bandwidth reduction.

---

## Block Heartbeat

- [x] **Mining overlay** — 90s delay, auto-dismiss 120s, clears on block/reconnect
- [x] **Mobile tap** — first tap = tooltip, second tap = block detail
- [x] **Mobile canvas sizing** — viewport-filling on mobile, 40vh on desktop
- [x] **5-min tab reset** — fetch blocks before destroy/init, color fix, suppress flash
- [ ] **Mobile fullscreen** — canvas doesn't fill properly in fullscreen on mobile
- [ ] **Waveform spike variance** — fee difference not visually distinct enough
- [ ] **Sort bricks by fee rate** — higher fee = higher in stack
- [ ] **Full mempool on flatline** — currently limited to 10K txs via SSE
- [ ] **Pinned blip after culling** — null check needed in cull logic
- [ ] **ZMQ sequence tracking** — detect dropped notifications

---

## Testing

- [x] **Rust unit tests** — 104 tests passing
- [ ] **JS test framework** — set up vitest for heartbeat ES modules
- [ ] **Review all tests** — audit for coverage gaps, add classifier/ingest/chart tests

---

## Documentation

- [x] **Methodology page** — full observatory data methodology at /observatory/learn/methodology
- [x] **Learn index page** — hub at /observatory/learn linking to all articles
- [x] **Protocol Guide** — Bitcoin embedding protocols at /observatory/learn/protocols
- [ ] **Expand docs/data-guide.md** — add remaining chart derivation explanations
- [ ] **Public API documentation page** at /api/docs

---

## Future Ideas

### New Charts
- [ ] Block weight composition treemap
- [ ] Coinbase message trends (needs coinbase_text column, v11 backfill populates it)
- [ ] Mempool fee histogram (needs separate mempool analysis)

### Inscriptions & Runes Deep Dive
- [ ] Inscription fees, fee share, cumulative count
- [ ] Runes count share, cumulative count, fees, fee share

### Creative Features
- [ ] **Block DNA** — deterministic visual fingerprint per block

### Notifications & Bot
- [ ] Event detection engine (BIP thresholds, records broken, halvings, price milestones)
- [ ] Twitter/X bot, Nostr bot, email subscriptions, browser push, on-site feed

### Other
- [ ] CSV "Download All" per tab
- [ ] Hourly price source for 1D range overlay
- [ ] KYC-Free / Privacy guide page
- [ ] BIP overlay expansion (BIP-30, BIP-9, BIP-148, BIP-125, BIP-173, etc.)

---

## Known Limitations (documented, not bugs)

- segwit_spend_count counts transactions, not individual SegWit inputs
- taproot_spend_count is P2TR output count (historical naming, charts label correctly)
- Avg tx size includes coinbase (negligible at typical tx counts)
- Daily subsidy on halving day: mix of old/new subsidy (4 data points ever)
- Median fee uses lower-median (consistent with Bitcoin Core)
- Price overlay: no data on 1D range (daily granularity source)
- Stamps detection broken (needs Counterparty protocol decoding)
- Counterparty chart includes all Counterparty activity, not just Stamps
- Inscription bytes = payload only; inscription_envelope_bytes = full witness item
- OP_RETURN bytes = full scriptPubKey (including opcode)
- Historical Omni/Counterparty non-OP_RETURN encodings not detected

---

## Completed

<details>
<summary>Session 2026-04-13/14</summary>

- [x] Heartbeat: mobile tap fix, canvas sizing, 5-min reset race + colors, flash suppression
- [x] Guide images: renamed assets/guides/ to assets/guide-images/ (route conflict fix)
- [x] On This Day: sort dropdown with dark theme
- [x] Coming Soon overlay on v10 charts (restored pending v11 backfill)
- [x] Flatten chart sections: network, mining, embedded (removed Show guards, section dropdowns)
- [x] Drawer simplified: removed section_key, reload hack, update_section_in_url
- [x] Section headings renamed: Mining Pools, OP_RETURN, Ordinals & Witness Data
- [x] Stacked chart tooltips: show all series including zero-value
- [x] Price overlay interpolation for per-block chart tooltips
- [x] Tooltip TypeError fix on overlay series without data arrays
- [x] Removed overlays from Mining Diversity gauge (was crashing)
- [x] Added overlays to 9 manually-derived chart signals
- [x] Converted 9 manual Signal::derive to chart_memo! for caching
- [x] Per-block P2PKH Sunset Tracker and Adoption Velocity
- [x] inscription_envelope_bytes column for consistent byte accounting
- [x] Fixed update_block_extras missing all v10 columns (root cause of empty charts)
- [x] BACKFILL_VERSION bumped to 11
- [x] Methodology page at /observatory/learn/methodology (12 sections)
- [x] Learn index page at /observatory/learn
- [x] Methodology link on all chart pages via ChartPageLayout
- [x] Chart reordering across Network and Fees pages
- [x] Consolidated Fee Rate Bands (removed original, distinct colors on full version)
- [x] Info tooltips on 33 charts (objective, no absolutes)
- [x] Histogram count/% toggle on Block Fullness and Block Time Distribution
- [x] Adoption Velocity chart title fix
- [x] Block fullness info text fix
- [x] MWU spelled out as 'million weight units'
- [x] Fullscreen chart exit on range change fixed (all 4 pages)
- [x] JSON-LD, llms.txt, breadcrumbs updated with Learn/Methodology
- [x] Total Fees description clarified
</details>

<details>
<summary>Earlier sessions (v2-v4, 2026-03-19 to 2026-04-12)</summary>

- [x] OP_RETURN classifier, BIP signaling, mining pools, fee data, ECharts migration
- [x] Chart overlays, embedded data tab, output types, witness bytes, inscriptions
- [x] r2d2 pool, caching, lazy rendering, module split, sub-section navigation
- [x] Taproot key/script path, batching, address types, dashboard polish
- [x] Full chart data audit, 48 unit tests, OCEAN sub-miners, SEO
- [x] Shareable links, CSV export, Stats Dashboard, custom date picker, CI/CD
- [x] On This Day page, TPS, fee charts, stats dashboard additions
- [x] ZMQ subscriber, SSE endpoint, heartbeat JS module split
- [x] Hall of Fame (57 entries), Stats Overview redesign, block detail modal
- [x] Security audit (5 fixes), dead code cleanup (32 files), documentation (35 files)
- [x] DB indexes, pre-computed daily_blocks, cache pre-warming, direct string serialization
- [x] 45+ charts total across all pages
- [x] Stale block / reorg detection
</details>
