# WE HODL BTC — Data & Metrics Guide

A reference for how each metric is computed, what it measures, and common pitfalls.

---

## Fee Metrics

### Fee Rate (sat/vB)
The cost per virtual byte to include a transaction in a block. Computed as `fee_sats / vsize`. Virtual size accounts for the SegWit discount: witness data is counted at 1/4 weight. A typical P2WPKH transaction is ~110 vbytes, so at 5 sat/vB the fee would be ~550 sats.

### Median Fee Rate
For each block, all transaction fee rates are sorted ascending. The value at the midpoint is the median. We use lower-median: `fee_rates[(len - 1) / 2]`, consistent with Bitcoin Core. This represents what a "typical" transaction in the block paid.

### Fee Rate Percentiles (p10 / p90)
- **p10 (10th percentile):** `fee_rates[len / 10]` — 10% of transactions paid less than this. Represents the cheapest transactions that still got confirmed.
- **p90 (90th percentile):** `fee_rates[len * 9 / 10]` — only 10% of transactions paid more. Represents the most urgent/expensive transactions.
- **Interpretation:** The gap between p10 and p90 is the fee spread. Narrow band = calm fee market, wide band = active fee market with high variance.
- **Minimum data:** Requires at least 10 transactions in a block to compute meaningful percentiles. Blocks with fewer return 0.

### Avg Fee per Transaction
`total_fees / (tx_count - block_count)` — total fees divided by non-coinbase transaction count. Tells you the average absolute cost in satoshis to get a transaction confirmed, regardless of transaction size.

### Total Fees
Sum of all transaction fees in the range, in satoshis. This is the total revenue miners earned from fees (excluding the block subsidy).

---

## Network Metrics

### Transactions per Second (TPS)
- **Per-block:** `tx_count / seconds_since_previous_block` for each block. The first block in a range has TPS = 0 (no previous block for interval).
- **Daily:** `(avg_tx_count * block_count) / 86,400` — total transactions that day divided by seconds in a day.
- **Note:** Bitcoin typically runs 3-7 TPS. This is a throughput metric, not a capacity limit.

### Weight Utilization
`block_weight / 4,000,000 * 100` — how full each block is as a percentage of the 4 million weight unit (MWU) consensus limit. 100% = completely full block.

### Block Interval
Time in minutes between consecutive blocks. Target is 10 minutes. Computed as `(block_N.timestamp - block_N-1.timestamp) / 60`. Early Bitcoin (2009-2010) had extremely erratic intervals due to low hashrate.

### Chain Size
Cumulative sum of raw block sizes in bytes. For non-ALL ranges, an offset is fetched (`SUM(size) WHERE height < start_height`) so the chart shows absolute chain size, not growth from zero.

---

## Adoption Metrics

### SegWit Transactions %
Percentage of non-coinbase transactions that have witness data (`has_witness == true`). A transaction counts as SegWit if ANY of its inputs use witness data. This is higher than SegWit's share of outputs because a single SegWit transaction can create legacy outputs.

### Taproot Outputs
Count of P2TR (Pay-to-Taproot, `bc1p...`) outputs created. Note: our field `taproot_spend_count` is named historically but actually counts outputs created, not inputs spent.

### Taproot Key-path vs Script-path
- **Key-path:** Input has exactly 1 witness element of 64-65 bytes (a Schnorr signature). Simple single-key spend.
- **Script-path:** Input has 2+ witness elements, last starts with `0xc0` or `0xc1` (control block). Complex script execution.

### RBF Usage %
Percentage of non-coinbase transactions signaling Replace-By-Fee via `nSequence < 0xFFFFFFFE`. Gated to blocks after Feb 23, 2016 (Bitcoin Core v0.12.0 introduced BIP-125 opt-in RBF). Pre-BIP-125, low nSequence values were used for other purposes (nLockTime, original Satoshi replacement mechanism).

### Witness Data %
`total_witness_bytes / total_block_size * 100` — what percentage of block data is witness (signature) data. Higher = more SegWit usage. Witness data gets the 75% weight discount.

---

## Embedded Data Metrics

### Inscriptions
Ordinals inscriptions detected via the witness envelope pattern: `OP_FALSE OP_IF OP_PUSH3 "ord"` (`00 63 03 6f7264` in hex). Byte count uses dynamic envelope overhead calculation.

### BRC-20
A subset of inscriptions. Detected by checking if the inscription contains `{"p":"brc-20"` in the witness data. Always counted within total inscriptions (not double-counted in stacked charts).

### Runes
Detected via `OP_RETURN` outputs starting with `OP_13` (`6a5d` hex). Height-gated to block 840,000+ to prevent false positives from pre-launch OP_RETURN data that coincidentally matched the pattern.

### Stamps (not yet tracked)
Stamps are built on top of the Counterparty protocol. They use Counterparty's encoding with a `STAMP:` prefix in the description field, and store data in bare multisig outputs. Detection requires decoding the Counterparty payload first, which we don't currently do. The "Counterparty" category in our OP_RETURN charts includes all Counterparty transactions (tokens, DEX orders, Stamps, etc.) — we cannot distinguish Stamps from other Counterparty usage without protocol-level decoding. The spike in Counterparty activity from 2023+ is likely a mix of Stamps and renewed XCP activity.

### OP_RETURN Classification
Each OP_RETURN output is classified by checking the payload at the correct offset after the push opcode:
- **Runes:** `OP_13` prefix, height >= 840,000
- **Omni:** Contains `"omni"` (`6f6d6e69` hex) in payload
- **Counterparty:** Contains `"CNTRPRTY"` (`434e545250525459` hex) in payload
- **SegWit Commit:** Coinbase OP_RETURN (excluded from counts)
- **Data Carrier:** Everything else

---

## Price Data

### Source
Daily average BTC/USD price from blockchain.info API (`/charts/market-price?timespan=all&format=json`). This is the volume-weighted average across exchanges for each calendar day, not open/close.

### Pre-Exchange Era (2009-2011)
Hardcoded approximate prices for dates before blockchain.info data begins:
- 2009: $0 (no market)
- Early 2010: ~$0.003
- May 2010 (Pizza Day): ~$0.004
- Jul 2010 (Mt. Gox): ~$0.05
- Late 2010: $0.10-$0.25
- Feb 2011: $1.00

### Market Cap
Computed as `calc_supply(block_height) * price_usd`. Supply is calculated from the halving schedule, not from UTXO set scanning.

---

## Supply Calculation

`calc_supply(height)` sums subsidy across all halving eras up to the given block:
- Era 0 (blocks 0-209,999): 50 BTC/block
- Era 1 (blocks 210,000-419,999): 25 BTC/block
- Era 2 (blocks 420,000-629,999): 12.5 BTC/block
- Era 3 (blocks 630,000-839,999): 6.25 BTC/block
- Era 4 (blocks 840,000+): 3.125 BTC/block

Theoretical maximum: 20,999,999.9769 BTC (not exactly 21M due to integer rounding of halvings). Additionally ~100+ BTC permanently lost from unclaimed coinbase rewards and the unspendable genesis block.

---

## Transaction Volume

Sum of all non-coinbase output values in satoshis. **Includes change outputs**, so the same BTC can be counted multiple times across transactions. This is closer to "gross transaction volume" than "net economic transfer." A more accurate transfer metric would require UTXO analysis to separate change from actual payments, which we don't currently do.

---

## Time & Grouping

### All Times UTC
Block timestamps are Unix epoch (UTC). All grouping (daily aggregates, On This Day) uses `datetime(timestamp, 'unixepoch')` which is UTC. Mempool.space localizes to the user's timezone; we deliberately do not, for consistency across users.

### Daily Aggregates
Blocks grouped by `date(datetime(timestamp, 'unixepoch'))` — calendar day in UTC. Fields are either `AVG()` (per-block averages) or `SUM()` (totals). Ranges over ~5,000 blocks (~35 days) automatically switch to daily aggregates for performance.

### Range Presets
| Range | Blocks | Mode |
|-------|--------|------|
| 1D | 144 | Per-block |
| 1W | 1,008 | Per-block |
| 1M | 4,320 | Per-block |
| 3M | 12,960 | Daily |
| 6M | 25,920 | Daily |
| YTD | Variable | Daily |
| 1Y | 52,560 | Daily |
| 2Y+ | 105,120+ | Daily |
| ALL | All blocks | Daily |
| Custom | Variable | Auto (per-block if < ~35 days) |
