//! Data Methodology: documents how embedded protocol data is detected, counted, and measured.
//! Route: /observatory/learn/methodology

use leptos::prelude::*;
use leptos_meta::*;

#[component]
pub fn MethodologyPage() -> impl IntoView {
    view! {
        <Title text="Data Methodology | WE HODL BTC"/>
        <Meta name="description" content="How WE HODL BTC detects and measures embedded protocol data on Bitcoin: inscriptions, OP_RETURN protocols, BRC-20, Runes, and more."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/learn/methodology"/>

        <div class="max-w-4xl mx-auto space-y-8 pb-16">
            // Header
            <div class="text-center">
                <h1 class="text-2xl sm:text-3xl font-title text-white mb-2">"Data Methodology"</h1>
                <p class="text-sm text-white/50 max-w-2xl mx-auto">"How we detect, count, and measure embedded protocol data on the Bitcoin blockchain."</p>
            </div>

            // ── Taxonomy ─────────────────────────────────────────
            <Section title="Data Taxonomy">
                <p>"All embedded data on Bitcoin falls into two disjoint categories based on where it lives in a transaction:"</p>
                <div class="bg-[#0a1a2e] border border-white/10 rounded-xl p-5 mt-3 font-mono text-sm space-y-2">
                    <p class="text-white/80">"Total Embedded Data = Witness Data + OP_RETURN Data"</p>
                    <div class="ml-4 text-white/60 space-y-1">
                        <p>"Witness Data (segregated witness fields)"</p>
                        <p class="ml-4 text-white/40">"Inscriptions (Ordinals)"</p>
                        <p class="ml-8 text-white/30">"BRC-20 (subset of Inscriptions)"</p>
                    </div>
                    <div class="ml-4 text-white/60 space-y-1">
                        <p>"OP_RETURN Data (nulldata script outputs)"</p>
                        <p class="ml-4 text-white/40">"= Runes + Omni Layer + Counterparty + Other"</p>
                    </div>
                    <div class="ml-4 text-white/60 space-y-1">
                        <p>"Stamps (bare multisig outputs with fake pubkeys)"</p>
                        <p class="ml-4 text-white/30">"count only, no byte tracking"</p>
                    </div>
                </div>
                <p class="mt-3 text-white/50 text-sm">"Inscriptions and OP_RETURN are guaranteed disjoint (different parts of the transaction). BRC-20 is always a subset of Inscriptions. Runes, Omni, Counterparty, and Other are mutually exclusive subsets of OP_RETURN."</p>
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
                <p class="mt-3 text-white/50 text-sm">"For comparable cross-category analysis, use inscription_envelope_bytes alongside OP_RETURN bytes (both represent full on-chain serialized footprint). Use inscription_bytes (payload) when analyzing content size alone."</p>
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
                <ul class="list-disc list-inside text-white/60 text-sm space-y-1 mt-2">
                    <li>"Historical Omni Layer encodings using bare multisig (Class A) or other pre-OP_RETURN methods"</li>
                    <li>"Historical Counterparty encodings using multisig or pubkeyhash patterns"</li>
                    <li>"Cursed, unbound, or non-standard inscription envelopes"</li>
                    <li>"Witness data stuffing outside the standard Ordinals envelope"</li>
                    <li>"P2SH redeemScript data stuffing"</li>
                    <li>"Annex field data (BIP 341)"</li>
                    <li>"Unknown or emerging protocols using novel encoding methods"</li>
                </ul>
                <p class="mt-2 text-white/40 text-xs">"These exclusions mean total embedded data figures are conservative lower bounds."</p>
            </Section>

            // ── Data Source ──────────────────────────────────────
            <Section title="Data Source">
                <p>"All data is derived from confirmed blocks fetched from a local Bitcoin Core node at verbosity level 2 (full transaction details). Block data is ingested incrementally via 60-second polling and stored in SQLite."</p>
                <p class="mt-2">"Duplicate payloads across transactions are counted separately. Reorgs are detected and corrected within 15 seconds."</p>
            </Section>

            // ── Non-Overlapping Formula ──────────────────────────
            <Section title="Non-Overlapping Total">
                <p>"To compute a non-overlapping total of embedded data from the dashboard:"</p>
                <div class="bg-[#0a1a2e] border border-white/10 rounded-xl p-4 mt-3 font-mono text-sm">
                    <p class="text-[#f7931a]">"total_bytes = inscription_envelope_bytes + op_return_bytes"</p>
                    <p class="text-[#f7931a] mt-1">"total_count = inscription_count + op_return_count"</p>
                </div>
                <p class="mt-2 text-white/50 text-sm">"Do not add BRC-20 to Inscriptions, or Runes to OP_RETURN, as these are subsets."</p>
            </Section>

            // Back link
            <div class="text-center pt-4">
                <a href="/observatory/charts/embedded" class="text-sm text-white/40 hover:text-[#f7931a] transition-colors">
                    "Back to Embedded Data charts"
                </a>
            </div>
        </div>
    }
}

// ── Helper components ────────────────────────────────────────────

#[component]
fn Section(
    #[prop(into)] title: String,
    children: Children,
) -> impl IntoView {
    view! {
        <section class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 sm:p-6">
            <h2 class="text-lg text-white font-semibold mb-3">{title}</h2>
            <div class="text-sm text-white/70 leading-relaxed">
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
                <span class="text-white/30 text-xs ml-1">"("{unit}")"</span>
            </div>
            <p class="text-white/60">{desc}</p>
        </div>
    }
}

#[component]
fn ByteRow(
    #[prop(into)] name: &'static str,
    #[prop(into)] desc: &'static str,
) -> impl IntoView {
    view! {
        <div class="bg-white/[0.03] rounded-lg p-3">
            <p class="text-white/80 font-mono text-xs mb-1">{name}</p>
            <p class="text-white/50 text-xs">{desc}</p>
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
                <span class=format!("text-[10px] px-1.5 py-0.5 rounded-full {}", badge_color)>{confidence}</span>
            </div>
            <p class="text-white/60 text-xs font-mono mb-1">{rule}</p>
            <p class="text-white/40 text-xs">{note}</p>
        </div>
    }
}
