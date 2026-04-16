//! Data Methodology: documents how embedded protocol data is detected, counted, and measured.
//! Route: /observatory/learn/methodology

use leptos::prelude::*;
use leptos_meta::*;

#[component]
pub fn MethodologyPage() -> impl IntoView {
    view! {
        <Title text="Data Methodology | WE HODL BTC"/>
        <Meta name="description" content="Complete data methodology for WE HODL BTC observatory: block metrics, fee calculations, address type classification, mining pool identification, embedded protocol detection, price data sourcing, and daily aggregation."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/learn/methodology"/>

        <div class="max-w-4xl mx-auto space-y-8 pb-16">
            // Header
            <div class="text-center">
                <h1 class="text-2xl sm:text-3xl font-title text-white mb-2">"Data Methodology"</h1>
                <p class="text-sm text-white/50 max-w-2xl mx-auto">"How we source, compute, and classify every metric on the observatory. From raw RPC data to chart-ready aggregates."</p>
            </div>

            // ── Taxonomy ─────────────────────────────────────────
            <Section title="Data Taxonomy">
                <p>"All embedded data on Bitcoin falls into two disjoint categories based on where it lives in a transaction:"</p>
                <div class="bg-[#0a1a2e] border border-white/10 rounded-xl p-5 sm:p-6 mt-3 font-mono text-base space-y-3">
                    <p class="text-[#f7931a] font-semibold">"Total Embedded Data = Witness Data + OP_RETURN Data"</p>
                    <div class="ml-4 text-white/70 space-y-1">
                        <p>"Witness Data " <span class="text-white/40">"(segregated witness fields)"</span></p>
                        <p class="ml-4 text-white/60">"Inscriptions " <span class="text-white/40">"(Ordinals)"</span></p>
                        <p class="ml-8 text-white/50">"BRC-20 " <span class="text-white/40">"(subset of Inscriptions)"</span></p>
                    </div>
                    <div class="ml-4 text-white/70 space-y-1">
                        <p>"OP_RETURN Data " <span class="text-white/40">"(nulldata script outputs)"</span></p>
                        <p class="ml-4 text-white/50">"= Runes + Omni Layer + Counterparty + Other"</p>
                    </div>
                    <div class="ml-4 text-white/70 space-y-1">
                        <p>"Stamps " <span class="text-white/40">"(bare multisig outputs with fake pubkeys)"</span></p>
                        <p class="ml-4 text-white/50">"count only, no byte tracking"</p>
                    </div>
                </div>
                <p class="mt-3 text-white/60">"Inscriptions and OP_RETURN are guaranteed disjoint (different parts of the transaction). BRC-20 is always a subset of Inscriptions. Runes, Omni, Counterparty, and Other are mutually exclusive subsets of OP_RETURN."</p>
            </Section>

            // ── What We Count ────────────────────────────────────
            <Section title="What Each Count Means">
                <MetricRow
                    name="Inscriptions"
                    unit="witness envelopes"
                    desc="Number of standard Ordinals inscription envelopes found in witness data. One transaction can contain multiple inscriptions."
                />
                <MetricRow
                    name="BRC-20"
                    unit="inscription envelopes"
                    desc="Subset of inscriptions whose body contains the BRC-20 JSON marker. Always less than or equal to inscription count."
                />
                <MetricRow
                    name="OP_RETURN outputs"
                    unit="transaction outputs"
                    desc="Outputs whose scriptPubKey begins with OP_RETURN (0x6a). Coinbase SegWit commitment outputs are excluded."
                />
                <MetricRow
                    name="Runes"
                    unit="OP_RETURN outputs"
                    desc="OP_RETURN outputs matching the Runes protocol signature (OP_RETURN OP_13 prefix, or tiny scripts at block 840,000+)."
                />
                <MetricRow
                    name="Omni Layer"
                    unit="OP_RETURN outputs"
                    desc="OP_RETURN outputs containing the 'omni' ASCII marker (hex: 6f6d6e69) in their payload."
                />
                <MetricRow
                    name="Counterparty"
                    unit="OP_RETURN outputs"
                    desc="OP_RETURN outputs containing the 'CNTRPRTY' ASCII marker (hex: 434e545250525459)."
                />
                <MetricRow
                    name="Stamps"
                    unit="multisig outputs"
                    desc="Bare multisig outputs containing at least one fake public key (33-byte key not starting with 0x02 or 0x03). Count only."
                />
            </Section>

            // ── Byte Accounting ──────────────────────────────────
            <Section title="Byte Accounting">
                <p>"We track two byte measures for inscriptions and one for OP_RETURN:"</p>
                <div class="space-y-3 mt-3">
                    <ByteRow
                        name="inscription_bytes (payload)"
                        desc="Content body only. Envelope overhead (OP_FALSE, OP_IF, 'ord' marker, content-type section, separator, OP_ENDIF) is subtracted. Typically 8-20 bytes of overhead removed per inscription."
                    />
                    <ByteRow
                        name="inscription_envelope_bytes (full)"
                        desc="Full witness item bytes including the complete envelope structure and payload. Comparable to OP_RETURN script bytes."
                    />
                    <ByteRow
                        name="OP_RETURN bytes (script)"
                        desc="Full scriptPubKey byte length: includes the OP_RETURN opcode, push opcodes, and payload data. This is the on-chain serialized size."
                    />
                </div>
                <p class="mt-3 text-white/60">"For comparable cross-category analysis, use inscription_envelope_bytes alongside OP_RETURN bytes (both represent full on-chain serialized footprint). Use inscription_bytes (payload) when analyzing content size alone."</p>
            </Section>

            // ── Detection Rules ──────────────────────────────────
            <Section title="Detection Heuristics">
                <DetectionRow
                    name="Inscriptions"
                    rule="Hex pattern 0063036f7264 in witness data (OP_FALSE OP_IF OP_PUSH3 'ord')."
                    confidence="High"
                    note="Standard Ordinals envelopes only. Cursed, non-standard, and malformed inscriptions are not detected."
                />
                <DetectionRow
                    name="BRC-20"
                    rule="Substring match for hex 7b2270223a226272632d3230 inside an inscription body."
                    confidence="Medium"
                    note="Matches the JSON fragment {\"p\":\"brc-20. No JSON validation is performed, so malformed BRC-20 payloads are included."
                />
                <DetectionRow
                    name="Runes"
                    rule="OP_RETURN starting with OP_13 (0x5d) at block >= 840,000, or tiny scripts <= 6 bytes at block >= 840,000."
                    confidence="High"
                    note="Matches the protocol specification. Before block 840,000 the same pattern is classified as generic OP_RETURN."
                />
                <DetectionRow
                    name="Omni Layer"
                    rule="OP_RETURN payload contains ASCII 'omni' (hex: 6f6d6e69)."
                    confidence="Medium"
                    note="OP_RETURN encoding only. Historical bare multisig (Class A/B) encodings are not detected."
                />
                <DetectionRow
                    name="Counterparty"
                    rule="OP_RETURN payload contains ASCII 'CNTRPRTY' (hex: 434e545250525459)."
                    confidence="Medium"
                    note="OP_RETURN encoding only. Historical multisig and pubkeyhash encodings are not detected."
                />
                <DetectionRow
                    name="Stamps"
                    rule="Bare multisig output with at least one 33-byte key not starting with 0x02/0x03."
                    confidence="Medium"
                    note="Fake pubkey detection. May miss some encoding variants."
                />
            </Section>

            // ── Known Exclusions ─────────────────────────────────
            <Section title="Known Exclusions">
                <p>"The following data embedding techniques are " <strong>"not currently detected"</strong> ":"</p>
                <ul class="list-disc list-inside text-white/70 space-y-1.5 mt-2">
                    <li>"Historical Omni Layer encodings using bare multisig (Class A) or other pre-OP_RETURN methods"</li>
                    <li>"Historical Counterparty encodings using multisig or pubkeyhash patterns"</li>
                    <li>"Cursed, unbound, or non-standard inscription envelopes"</li>
                    <li>"Witness data stuffing outside the standard Ordinals envelope"</li>
                    <li>"P2SH redeemScript data stuffing"</li>
                    <li>"Annex field data (BIP 341)"</li>
                    <li>"Unknown or emerging protocols using novel encoding methods"</li>
                </ul>
                <p class="mt-3 text-white/50 text-sm">"These exclusions mean total embedded data figures are conservative lower bounds."</p>
            </Section>

            // ── Data Source ──────────────────────────────────────
            <Section title="Data Source">
                <p>"All data is derived from confirmed blocks fetched from a local Bitcoin Core node at verbosity level 2 (full transaction details). Block data is ingested incrementally via 60-second polling and stored in SQLite."</p>
                <p class="mt-2">"Duplicate payloads across transactions are counted separately. Reorgs are detected and corrected within 15 seconds."</p>
            </Section>

            // ── Non-Overlapping Formula ──────────────────────────
            <Section title="Non-Overlapping Total">
                <p>"To compute a non-overlapping total of embedded data from the dashboard:"</p>
                <div class="bg-[#0a1a2e] border border-white/10 rounded-xl p-5 mt-3 font-mono text-base">
                    <p class="text-[#f7931a]">"total_bytes = inscription_envelope_bytes + op_return_bytes"</p>
                    <p class="text-[#f7931a] mt-1">"total_count = inscription_count + op_return_count"</p>
                </div>
                <p class="mt-3 text-white/60">"Do not add BRC-20 to Inscriptions, or Runes to OP_RETURN, as these are subsets."</p>
            </Section>

            // ── Block Metrics ─────────────────────────────────────
            <Section title="Block Metrics">
                <p>"All block data is fetched from Bitcoin Core at " <strong>"verbosity level 2"</strong> " (full transaction details including inputs, outputs, witness data, and script types)."</p>
                <div class="space-y-2 mt-3">
                    <MetricRow name="Block Size" unit="bytes" desc="Raw serialized block size as reported by Bitcoin Core. Includes header, transaction data, and witness data."/>
                    <MetricRow name="Weight" unit="weight units" desc="BIP 141 block weight: base_size * 3 + total_size. Consensus limit is 4,000,000 WU. Weight utilization is weight / 4,000,000 * 100%."/>
                    <MetricRow name="Transaction Count" unit="transactions" desc="Total transactions in the block including the coinbase transaction."/>
                    <MetricRow name="Block Interval" unit="seconds" desc="Difference between this block's timestamp and the previous block's timestamp. Target is 600 seconds (10 minutes)."/>
                </div>
            </Section>

            // ── Fee Calculations ─────────────────────────────────
            <Section title="Fee Calculations">
                <p>"Fees are derived from transaction data, not from any external fee estimation API."</p>
                <div class="space-y-2 mt-3">
                    <MetricRow name="Total Fees" unit="satoshis" desc="Sum of all transaction fees in the block. Each fee = sum(inputs) - sum(outputs). Equivalently: coinbase output value minus the block subsidy (50 BTC halved every 210,000 blocks)."/>
                    <MetricRow name="Per-TX Fee" unit="satoshis" desc="For each non-coinbase transaction: sum of input values minus sum of output values. Requires txindex for input value lookups."/>
                    <MetricRow name="Median Fee Rate" unit="sat/vB" desc="Median of all per-transaction fee rates in the block. Fee rate = fee / virtual_size. Virtual size = weight / 4."/>
                    <MetricRow name="Fee Percentiles" unit="sat/vB" desc="p10, p25, p75, p90 fee rates computed from the sorted list of per-transaction fee rates. Used for the Fee Rate Bands chart."/>
                    <MetricRow name="Max TX Fee" unit="satoshis" desc="Largest individual transaction fee in the block. Highlights fat-finger fees and high-priority transactions."/>
                    <MetricRow name="Protocol Fees" unit="satoshis" desc="If a transaction contains an inscription or Runes output, its entire fee is attributed to that protocol. A transaction can only be attributed to one protocol."/>
                </div>
            </Section>

            // ── Address Type Classification ──────────────────────
            <Section title="Address Type Classification">
                <p>"Output types are classified from the " <code class="text-white/60">"scriptPubKey.type"</code> " field returned by Bitcoin Core:"</p>
                <div class="space-y-2 mt-3">
                    <MetricRow name="P2PK" unit="outputs" desc="Pay-to-Public-Key (type: 'pubkey'). Early Bitcoin transactions, rarely used after 2010."/>
                    <MetricRow name="P2PKH" unit="outputs" desc="Pay-to-Public-Key-Hash (type: 'pubkeyhash'). Legacy addresses starting with '1'."/>
                    <MetricRow name="P2SH" unit="outputs" desc="Pay-to-Script-Hash (type: 'scripthash'). Addresses starting with '3', used for multisig and wrapped SegWit."/>
                    <MetricRow name="P2WPKH" unit="outputs" desc="Pay-to-Witness-Public-Key-Hash (type: 'witness_v0_keyhash'). Native SegWit addresses starting with 'bc1q'."/>
                    <MetricRow name="P2WSH" unit="outputs" desc="Pay-to-Witness-Script-Hash (type: 'witness_v0_scripthash'). Native SegWit multisig and complex scripts."/>
                    <MetricRow name="P2TR" unit="outputs" desc="Pay-to-Taproot (type: 'witness_v1_taproot'). Taproot addresses starting with 'bc1p'. Available since block 709,632."/>
                </div>
                <p class="mt-3 text-white/60">"SegWit adoption % is calculated as the percentage of non-coinbase transactions with at least one witness input. Taproot spend types (key-path vs script-path) are detected from witness stack structure."</p>
            </Section>

            // ── Mining Pool Identification ────────────────────────
            <Section title="Mining Pool Identification">
                <p>"Mining pools are identified by matching patterns in the coinbase transaction:"</p>
                <div class="space-y-2 mt-3">
                    <MetricRow name="Primary method" unit="coinbase text" desc="The coinbase scriptSig is decoded to ASCII and matched against known pool signatures (case-insensitive). Covers 30+ pools: Foundry, AntPool, ViaBTC, F2Pool, MARA, OCEAN, SpiderPool, and others."/>
                    <MetricRow name="Fallback method" unit="coinbase outputs" desc="If text matching fails, OP_RETURN outputs in the coinbase transaction are checked for pool identifiers."/>
                    <MetricRow name="OCEAN miners" unit="template detection" desc="OCEAN pool uses a decentralized template model. Individual OCEAN template miners are identified and attributed separately."/>
                    <MetricRow name="Unknown" unit="unidentified" desc="Blocks that match no known pool signature are labeled 'Unknown'. The HHI diversity index excludes Unknown miners to avoid inflating concentration metrics."/>
                </div>
            </Section>

            // ── Price Data ───────────────────────────────────────
            <Section title="Price Data">
                <p>"Bitcoin price data is used for the price overlay on charts."</p>
                <div class="space-y-2 mt-3">
                    <MetricRow name="Source" unit="API" desc="Historical daily prices from blockchain.info/charts API. Covers 2011 to present. Pre-2011 prices are hardcoded from historical records."/>
                    <MetricRow name="Live price" unit="mempool.space" desc="Current price from mempool.space API, cached for 60 seconds to avoid excessive requests."/>
                    <MetricRow name="Chart overlay" unit="interpolation" desc="For per-block charts, daily price data is interpolated to each block's timestamp using linear interpolation between surrounding price points. For daily charts, prices are matched by date."/>
                </div>
            </Section>

            // ── Daily Aggregates ─────────────────────────────────
            <Section title="Daily Aggregates">
                <p>"For time ranges longer than ~35 days (5,000 blocks), per-block data is rolled up into daily aggregates for performance."</p>
                <div class="space-y-2 mt-3">
                    <MetricRow name="Aggregation" unit="daily" desc="Each day's blocks are averaged (or summed where appropriate). Dates are derived from block timestamps in UTC."/>
                    <MetricRow name="Incremental updates" unit="automatic" desc="The daily_blocks table is updated incrementally as new blocks arrive. Only the current day's row is recomputed."/>
                    <MetricRow name="Range switching" unit="automatic" desc="Charts automatically switch between per-block and daily data based on the selected time range. Short ranges (1D-1M) show per-block data; longer ranges (3M+) show daily aggregates."/>
                </div>
                <p class="mt-3 text-white/60">"Some charts are only available on per-block ranges (scatter plots, histograms) while others are only meaningful on daily ranges (adoption velocity, sunset tracker). The chart description updates to reflect which mode is active."</p>
            </Section>

            // Back link
            <div class="text-center pt-4">
                <a href="/observatory/learn" class="text-sm text-white/40 hover:text-[#f7931a] transition-colors">
                    "Back to Learn"
                </a>
            </div>
        </div>
    }
}

// ── Helper components ────────────────────────────────────────────

#[component]
fn Section(#[prop(into)] title: String, children: Children) -> impl IntoView {
    view! {
        <section class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 sm:p-6">
            <h2 class="text-lg text-white font-semibold mb-4">{title}</h2>
            <div class="text-[15px] text-white/75 leading-relaxed">
                {children()}
            </div>
        </section>
    }
}

#[component]
fn MetricRow(
    #[prop(into)] name: &'static str,
    #[prop(into)] unit: &'static str,
    #[prop(into)] desc: &'static str,
) -> impl IntoView {
    view! {
        <div class="flex flex-col sm:flex-row gap-1 sm:gap-4 py-2 border-b border-white/5 last:border-0">
            <div class="sm:w-40 shrink-0">
                <span class="text-white/90 font-medium">{name}</span>
                <span class="text-white/40 text-sm ml-1">"("{unit}")"</span>
            </div>
            <p class="text-white/70">{desc}</p>
        </div>
    }
}

#[component]
fn ByteRow(
    #[prop(into)] name: &'static str,
    #[prop(into)] desc: &'static str,
) -> impl IntoView {
    view! {
        <div class="bg-white/[0.03] rounded-lg p-4">
            <p class="text-white/85 font-mono text-sm mb-1">{name}</p>
            <p class="text-white/60 text-sm">{desc}</p>
        </div>
    }
}

#[component]
fn DetectionRow(
    #[prop(into)] name: &'static str,
    #[prop(into)] rule: &'static str,
    #[prop(into)] confidence: &'static str,
    #[prop(into)] note: &'static str,
) -> impl IntoView {
    let badge_color = match confidence {
        "High" => "bg-green-500/20 text-green-400",
        "Medium" => "bg-yellow-500/20 text-yellow-400",
        _ => "bg-red-500/20 text-red-400",
    };
    view! {
        <div class="py-3 border-b border-white/5 last:border-0">
            <div class="flex items-center gap-2 mb-1">
                <span class="text-white/90 font-medium">{name}</span>
                <span class=format!("text-xs px-2 py-0.5 rounded-full {}", badge_color)>{confidence}</span>
            </div>
            <p class="text-white/60 text-sm font-mono mb-1">{rule}</p>
            <p class="text-white/50 text-sm">{note}</p>
        </div>
    }
}
