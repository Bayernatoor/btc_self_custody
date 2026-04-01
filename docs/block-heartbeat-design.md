# Block Heartbeat — Design Document

Bitcoin is a living organism. This feature makes that tangible.

## The Metaphor

| Bitcoin Concept | Organism Metaphor |
|----------------|-------------------|
| ~10 min block interval | Heartbeat |
| Block arrival | A breath |
| Fee spikes | Adrenaline / fever |
| Empty blocks | Skipped heartbeats |
| Difficulty adjustments | Adaptation |
| Mempool | Digestion |
| Hashrate | Immune system |
| Long block gaps | Holding its breath |

## Page: `/observatory/heartbeat`

### The EKG Line

A continuous sweep line (like a hospital cardiac monitor) moves left-to-right. Each block produces a PQRST waveform spike encoding real data:

- **P wave** (small bump): transaction count
- **QRS spike height**: block fees (taller = more fees)
- **Q depth** (dip before spike): time since previous block (deeper = longer wait)
- **T wave** (recovery): mempool change (positive = digestion, negative = growing)
- **Baseline between beats**: real time between blocks (this is where tension lives)

The line wraps at the right edge like a real monitor. Old trace fades. A phosphor glow effect (`shadowBlur`) mimics a CRT screen. Implemented as a `<canvas>` element with `requestAnimationFrame`.

### Color System

State determined by composite health score (0-100):

| State | Color | When |
|-------|-------|------|
| Healthy | `#00e676` green | ~10 min blocks, normal fees |
| Calm | `#42a5f5` blue | >15 min blocks, quiet mempool |
| Elevated | `#f7931a` orange | Fast blocks, fees rising |
| Stressed | `#ff5722` deep orange | Fee spike, congested mempool |
| Critical | `#f44336` red | >30 min gap, extreme fees |

Health score = weighted average of block_interval_score (35%), fee_score (25%), mempool_score (25%), hashrate_score (15%).

The flatline between beats shifts color as the wait grows: green → amber (15 min) → orange (20 min) → red (30 min). Subtle tremor/jitter after 20 min.

### Vital Signs Panel (4 tiles)

**Heart Rate** — blocks per 10 minutes (rolling 1hr). "1.0 bpm" = exactly on target. Shows "Bradycardia" / "Normal" / "Tachycardia".

**Blood Pressure** — fee rate as systolic/diastolic: `14 / 8 sat/vB`. Systolic = top 25% fee rate, diastolic = bottom 25%. High pressure = expensive to transact.

**Body Temperature** — mempool fullness mapped to temperature scale. 36.5° = healthy (5-15 vMB). 39°+ = fever (>150 vMB congestion).

**Immune System** — hashrate. "845 EH/s — Strength: Excellent". Shows blocks until next difficulty adjustment as "adapting in".

Each tile has a sparkline (24hr), status color, and qualitative label.

### 24-Hour Rhythm Strip

Compressed heartbeat of last 144 blocks. Shows the full day's pattern:
- US trading hours = denser, taller spikes
- Night/weekend = calmer rhythm
- Fee spike events = clusters of tall spikes
- Long gaps = visible flatlines ("cardiac arrest")

### Organism Status Panel

```
Condition:    Healthy — "Steady rhythm. The network hums."
Digesting:    24,821 unconfirmed tx (18.2 vMB)
Metabolism:   Normal — clearing 3,200 tx/block
Growth:       889,441 blocks tall
Next adaptation: ~412 blocks (~2d 21h)
Born:         January 3, 2009 — 17 years, 89 days ago
```

Condition descriptions change with state:
- Healthy: "Steady rhythm. The network hums."
- Calm: "Deep, slow breaths. The network rests."
- Stressed: "Heavy breathing. Block space is scarce."
- Critical: "Holding its breath. Waiting for the next block..."
- Post-difficulty: "Recovering. Adapting to new conditions."
- New ATH hashrate: "Strongest it's ever been."

### Block Arrival Animation (1.5s sequence)

1. **T+0ms** — EKG line flashes white, waveform begins drawing
2. **T+0ms** — Background pulse (page "breathes in")
3. **T+100ms** — Vital signs update with flip animation
4. **T+200ms** — Block height odometer rolls up
5. **T+300ms** — New block slides into Recent Breaths list
6. **T+500ms** — Tiny spike appears on 24-hour strip
7. **T+0ms** (if enabled) — "lub-dub" heartbeat sound

### Sound (optional, off by default)

Web Audio API synthesis — no audio files:
- "Lub": 80Hz sine, 60ms decay
- "Dub" (120ms later): 60Hz sine, 40ms decay
- Scales with state: faster tempo for fast blocks, louder for long-awaited blocks
- Optional ambient 40Hz drone during long waits (subliminal tension)

### Moment Capture (shareability)

"Capture Moment" button generates a 1200x630 share card with:
- Current EKG trace (last ~30s)
- Vital signs values and colors
- Block height and timestamp
- wehodlbtc.com watermark

Auto-detects interesting moments:
- >30 min block gap
- >5x fee spike
- 3+ blocks in <5 min
- Empty block
- Difficulty adjustment

### Mobile

- EKG panel full-width, reduced height (180px)
- Vital signs 2x2 grid, no sparklines
- 24-hour strip compressed to 80px
- Sound/capture in floating bottom bar

## Technical Architecture

### Data Sources (all existing)
- `LiveStats` polled every 30s (block height, mempool, fees, hashrate)
- `fetch_blocks(from, to)` for last 144 blocks
- `LiveContext` for polling countdown and connection status
- Block arrival detected by comparing height changes

### New Code
- `heartbeat.rs` — page component
- `heartbeat.js` — Canvas EKG animation + Web Audio
- One new server function for heartbeat-specific vitals computation

### Build Phases
1. **Foundation** — EKG canvas with sweep animation + real block data
2. **Vital Signs** — 4 tiles with health score computation
3. **24-Hour Rhythm** — compressed history strip
4. **Polish** — color transitions, background breathing, animations
5. **Share & Sound** — moment capture + Web Audio heartbeat

## Design Principles

1. **Data, not decoration** — every visual element encodes real data
2. **Tension lives in the waiting** — the flatline between blocks is the drama
3. **Understate, don't overstate** — subliminal effects > flashy effects
4. **Real-time honesty** — no smoothing, no faking. Show the 45-min gap.
5. **The organism is always healthy** — Bitcoin has never died. The tone is resilience.
