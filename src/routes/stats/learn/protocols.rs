//! Protocol Guide — educational page about Bitcoin data embedding protocols.
//! Route: /stats/learn/protocols

use leptos::prelude::*;
use leptos_meta::*;

// ---------------------------------------------------------------------------
// Protocol data
// ---------------------------------------------------------------------------

struct Protocol {
    id: &'static str,
    name: &'static str,
    year: &'static str,
    method: &'static str,
    method_detail: &'static str,
    color: &'static str,
    // Timeline positioning (percentage of 2012-2026 range)
    timeline_left: &'static str,
    timeline_row: u8,
    description: &'static str,
    how_it_works: &'static str,
    fun_fact: &'static str,
    status: &'static str,
    prunable: bool,
    fungible: bool,
}

const PROTOCOLS: &[Protocol] = &[
    Protocol {
        id: "omni",
        name: "Omni Layer",
        year: "2013",
        method: "OP_RETURN",
        method_detail: "Encodes token operations in OP_RETURN outputs using the \"omni\" magic prefix (hex: 6f6d6e69).",
        color: "#3b82f6",
        timeline_left: "7%",
        timeline_row: 0,
        description: "One of the earliest token-on-Bitcoin protocols, originally launched as Mastercoin. Tether (USDT) initially launched on Omni in 2014 before later migrating primarily to Ethereum and Tron.",
        how_it_works: "Transactions include an OP_RETURN output starting with the 4-byte \"omni\" marker, followed by encoded token operation data (send, issue, trade). Omni nodes index these outputs to track token balances.",
        fun_fact: "During 2017-2018, Omni-related transactions represented a significant portion of all OP_RETURN activity on Bitcoin, largely driven by Tether transfers.",
        status: "Low activity",
        prunable: true,
        fungible: true,
    },
    Protocol {
        id: "counterparty",
        name: "Counterparty",
        year: "2014",
        method: "OP_RETURN",
        method_detail: "Uses the \"CNTRPRTY\" prefix (hex: 434e545250525459) in OP_RETURN data to encode token operations.",
        color: "#f59e0b",
        timeline_left: "14%",
        timeline_row: 1,
        description: "Created XCP tokens through a proof-of-burn process where approximately 2,100 BTC were sent to an unspendable address. Enables token issuance, a decentralized exchange, and other financial primitives on Bitcoin.",
        how_it_works: "OP_RETURN outputs contain the 8-byte \"CNTRPRTY\" marker followed by AES-encrypted or plaintext protocol messages. Counterparty nodes decode these messages to execute token operations.",
        fun_fact: "The BTC burned to create XCP was worth around $1.8M at the time \u{2014} at today's prices it would be worth considerably more.",
        status: "Some activity",
        prunable: true,
        fungible: true,
    },
    Protocol {
        id: "stamps",
        name: "Stamps / SRC-20",
        year: "2023",
        method: "Bare Multisig",
        method_detail: "Embeds data in bare multisig output scripts (not P2SH-wrapped). Each fake public key in the multisig encodes ~32 bytes of image data.",
        color: "#94a3b8",
        timeline_left: "78.5%",
        timeline_row: 2,
        description: "Embeds image data directly in bare multisig output scripts. Unlike witness-based inscriptions, this data lives in the UTXO set and cannot be pruned by nodes \u{2014} it persists on every full node indefinitely. This design choice was intentional.",
        how_it_works: "A transaction creates outputs with bare multisig scripts (e.g. 1-of-3) where 2 of the 3 \"public keys\" are actually chunks of encoded image data. Since these are unspent outputs, they remain in every node's UTXO database permanently.",
        fun_fact: "Stamps emerged partly as a response to discussions in the Bitcoin community about potentially pruning or filtering Ordinals witness data.",
        status: "Active",
        prunable: false,
        fungible: false,
    },
    Protocol {
        id: "ordinals",
        name: "Ordinals",
        year: "Jan 2023",
        method: "Witness Data",
        method_detail: "Inscribes data inside a Taproot witness script using an envelope: OP_FALSE OP_IF OP_PUSH \"ord\" [content_type] [data] OP_ENDIF.",
        color: "#ec4899",
        timeline_left: "78.5%",
        timeline_row: 0,
        description: "Created by Casey Rodarmor. Introduces \"ordinal theory\" \u{2014} a convention for assigning serial numbers to individual satoshis \u{2014} and enables inscribing arbitrary data (images, text, HTML) into the witness field of transactions. Inscription data benefits from the SegWit witness discount, paying roughly 1/4 the fee rate of equivalent non-witness data.",
        how_it_works: "The inscription is placed inside a Taproot script-path spend. The witness contains an envelope with OP_FALSE OP_IF to create a no-op branch that carries the data. Since it's in the witness, it benefits from the 75% weight discount introduced by SegWit.",
        fun_fact: "The first known inscription (block 774,628) was a pixel art image. Inscription activity grew rapidly, with over a million inscriptions created within the first few months.",
        status: "Active",
        prunable: true,
        fungible: false,
    },
    Protocol {
        id: "brc20",
        name: "BRC-20",
        year: "Mar 2023",
        method: "Witness (JSON)",
        method_detail: "JSON inscriptions in Taproot witness data containing {\"p\":\"brc-20\",...} with deploy, mint, or transfer operations.",
        color: "#f472b6",
        timeline_left: "80%",
        timeline_row: 3,
        description: "An experimental fungible-token convention built on Ordinals inscriptions. Uses JSON payloads with deploy, mint, and transfer operations. Bitcoin's consensus rules do not validate BRC-20 state \u{2014} off-chain indexers interpret the inscriptions to determine token balances. BRC-20 activity was a major contributor to elevated fees and mempool congestion in mid-2023.",
        how_it_works: "A BRC-20 operation is an Ordinals inscription with content type \"text/plain\" containing JSON like {\"p\":\"brc-20\",\"op\":\"mint\",\"tick\":\"ordi\",\"amt\":\"1000\"}. External indexers parse all inscriptions, apply the BRC-20 rules, and compute token balances \u{2014} none of this logic exists in Bitcoin itself.",
        fun_fact: "BRC-20 is entirely off-protocol \u{2014} if two indexers disagree on how to interpret the JSON rules, users could see different token balances depending on which indexer they trust.",
        status: "Active",
        prunable: true,
        fungible: true,
    },
    Protocol {
        id: "runes",
        name: "Runes",
        year: "Apr 2024",
        method: "OP_RETURN",
        method_detail: "Uses OP_RETURN with OP_13 prefix (hex: 6a5d) followed by protocol-specific encoding for token operations.",
        color: "#ff6b6b",
        timeline_left: "85.7%",
        timeline_row: 1,
        description: "Also created by Casey Rodarmor (Ordinals). A fungible token protocol that uses OP_RETURN outputs for token operations (etching, minting, transferring). Designed as a simpler, more UTXO-friendly alternative to BRC-20. Launched at the exact halving block.",
        how_it_works: "Runes encodes token data in OP_RETURN outputs prefixed with OP_13 (0x5d). The protocol assigns token balances to specific UTXOs, so transferring tokens means spending and creating UTXOs \u{2014} much more aligned with Bitcoin's native model than inscription-based tokens.",
        fun_fact: "The simultaneous Runes launch and halving at block 840,000 caused a significant fee spike as users competed to etch the first tokens, with reported fee rates exceeding 1,000 sat/vB.",
        status: "Active",
        prunable: true,
        fungible: true,
    },
];

// ---------------------------------------------------------------------------
// Page component
// ---------------------------------------------------------------------------

#[component]
pub fn ProtocolGuidePage() -> impl IntoView {
    let (active_protocol, set_active_protocol) = signal(String::new());

    view! {
        <Title text="Embedding Protocols — The Bitcoin Observatory"/>
        <section class="max-w-5xl mx-auto px-6 pt-12 pb-24 opacity-0 animate-fadeinone">

            // Hero
            <div class="text-center mb-12">
                <a href="/stats" class="text-xs text-white/30 hover:text-white/50 transition-colors">
                    "\u{2190} Back to The Bitcoin Observatory"
                </a>
                <h1 class="text-3xl lg:text-4xl font-title text-white mt-4 mb-3">
                    "Bitcoin Embedding Protocols"
                </h1>
                <div class="w-16 h-0.5 bg-[#f7931a] mx-auto mb-4"></div>
                <p class="text-sm lg:text-base text-white/50 max-w-2xl mx-auto leading-relaxed">
                    "Since 2013, developers have found ways to embed non-financial data into Bitcoin's blockchain. From token protocols to digital art, each approach uses a different part of the transaction structure \u{2014} with different trade-offs for cost, pruning, and chain impact."
                </p>
            </div>

            // Timeline
            <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 mb-10">
                <h2 class="text-sm text-white/40 uppercase tracking-widest font-semibold mb-4">"Timeline"</h2>
                <div class="relative h-32 lg:h-28 mb-2">
                    // Year markers
                    <div class="absolute inset-x-0 bottom-0 flex justify-between text-[0.65rem] text-white/25 px-1">
                        {["2012", "2014", "2016", "2018", "2020", "2022", "2024", "2026"].into_iter().map(|y| {
                            view! { <span>{y}</span> }
                        }).collect::<Vec<_>>()}
                    </div>
                    // Baseline
                    <div class="absolute inset-x-0 bottom-5 h-px bg-white/10"></div>
                    // Halving markers
                    {["7%", "28.5%", "57%", "85.7%"].into_iter().map(|left| {
                        view! {
                            <div class="absolute bottom-3 w-px h-3 bg-[#f7931a]/30" style=format!("left: {left}")></div>
                        }
                    }).collect::<Vec<_>>()}

                    // Protocol bars
                    {PROTOCOLS.iter().map(|p| {
                        let id = p.id.to_string();
                        let id_click = id.clone();
                        let id_check = id.clone();
                        let bottom = format!("{}px", 20 + p.timeline_row as u32 * 22);
                        view! {
                            <button
                                class="absolute h-5 rounded-full cursor-pointer transition-all hover:brightness-125 border-2 flex items-center pl-2"
                                class=("border-white/40", move || active_protocol.get() == id_check)
                                class=("border-transparent", move || active_protocol.get() != id.clone())
                                style=format!("left: {}; right: 1%; bottom: {}; background: {}", p.timeline_left, bottom, p.color)
                                title=format!("{} ({})", p.name, p.year)
                                on:click={
                                    let id = id_click.clone();
                                    move |_| set_active_protocol.update(|a| {
                                        if *a == id { a.clear() } else { *a = id.clone() }
                                    })
                                }
                            >
                                <span class="text-[0.6rem] text-white/90 font-medium truncate pr-2">{p.name}</span>
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>
            </div>

            // Protocol cards
            <div class="space-y-6">
                {PROTOCOLS.iter().map(|p| {
                    let id = p.id.to_string();
                    let id_scroll = id.clone();
                    view! {
                        <div
                            id=format!("protocol-{}", p.id)
                            class="bg-[#0d2137] border border-white/10 rounded-2xl overflow-hidden transition-all"
                            class=("ring-1", move || active_protocol.get() == id_scroll)
                            class=("ring-white/20", move || active_protocol.get() == id.clone())
                        >
                            // Header
                            <div class="p-5 lg:p-6">
                                <div class="flex items-start gap-4">
                                    <div class="w-1.5 h-12 rounded-full shrink-0" style=format!("background: {}", p.color)></div>
                                    <div class="flex-1">
                                        <div class="flex items-baseline flex-wrap gap-x-3 gap-y-1 mb-2">
                                            <h3 class="text-xl text-white font-semibold">{p.name}</h3>
                                            <span class="text-sm text-white/30 font-mono">{p.year}</span>
                                            <span class="text-xs px-2.5 py-0.5 rounded-full border text-white/60"
                                                style=format!("border-color: {}50", p.color)
                                            >{p.method}</span>
                                            <span class="text-xs px-2 py-0.5 rounded-full bg-white/5 text-white/40">{p.status}</span>
                                        </div>
                                        <p class="text-sm text-white/60 leading-relaxed">{p.description}</p>
                                    </div>
                                </div>
                            </div>

                            // How it works
                            <div class="border-t border-white/5 px-5 lg:px-6 py-4 bg-[#0a1a2e]">
                                <h4 class="text-xs text-white/40 uppercase tracking-widest mb-2">"How it works"</h4>
                                <p class="text-sm text-white/50 leading-relaxed mb-2">{p.how_it_works}</p>
                                <code class="text-xs text-[#f7931a]/60 font-mono">{p.method_detail}</code>
                            </div>

                            // Footer: fun fact + properties
                            <div class="border-t border-white/5 px-5 lg:px-6 py-4 flex flex-col lg:flex-row lg:items-center gap-3">
                                <p class="text-xs text-white/40 italic flex-1">
                                    <span class="text-[#f7931a]/60">{"\u{1f4a1} "}</span>
                                    {p.fun_fact}
                                </p>
                                <div class="flex gap-2 shrink-0">
                                    <span class="text-[0.65rem] px-2 py-0.5 rounded border border-white/10"
                                        class=("text-green-400/60", p.prunable)
                                        class=("text-red-400/60", !p.prunable)
                                    >
                                        {if p.prunable { "Prunable" } else { "Unprunable" }}
                                    </span>
                                    <span class="text-[0.65rem] px-2 py-0.5 rounded border border-white/10 text-white/40">
                                        {if p.fungible { "Fungible tokens" } else { "Non-fungible / data" }}
                                    </span>
                                </div>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>

            // Historical note: Colored Coins
            <div class="bg-[#0a1a2e] border border-white/5 rounded-xl p-5 mt-8">
                <div class="flex items-start gap-3">
                    <span class="text-white/20 text-lg shrink-0">{"\u{1f4dc}"}</span>
                    <div>
                        <h4 class="text-sm text-white/50 font-semibold mb-1">"Historical note: Colored Coins (2012\u{2013}2015)"</h4>
                        <p class="text-xs text-white/40 leading-relaxed">
                            "Colored Coins was the earliest concept for representing tokens on Bitcoin, predating all protocols listed above. Rather than a single protocol, it was a family of incompatible implementations (Open Assets, EPOBC, ChromaWay) that used various techniques \u{2014} from OP_RETURN markers to transaction output ordering \u{2014} to \"color\" specific satoshis as representing real-world assets. Most implementations relied on external metadata servers, making on-chain detection unreliable. Colored Coins saw limited adoption but were historically significant as the first tokenization experiments on Bitcoin, directly inspiring later protocols like Counterparty and Omni. They are not tracked in the Observatory charts due to their ambiguous on-chain footprint and low volume."
                        </p>
                    </div>
                </div>
            </div>

            // Comparison table
            <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6 mt-10">
                <h2 class="text-sm text-white/40 uppercase tracking-widest font-semibold mb-4">"Comparison"</h2>
                <div class="overflow-x-auto">
                    <table class="w-full text-sm">
                        <thead>
                            <tr class="text-left text-white/40 text-xs uppercase tracking-wider">
                                <th class="pb-3 pr-4">"Protocol"</th>
                                <th class="pb-3 pr-4">"Year"</th>
                                <th class="pb-3 pr-4">"Embedding Method"</th>
                                <th class="pb-3 pr-4">"Prunable?"</th>
                                <th class="pb-3 pr-4">"Token Type"</th>
                                <th class="pb-3">"Status"</th>
                            </tr>
                        </thead>
                        <tbody class="text-white/60">
                            {PROTOCOLS.iter().map(|p| {
                                view! {
                                    <tr class="border-t border-white/5">
                                        <td class="py-2.5 pr-4 font-medium text-white/80">
                                            <div class="flex items-center gap-2">
                                                <div class="w-2 h-2 rounded-full" style=format!("background: {}", p.color)></div>
                                                {p.name}
                                            </div>
                                        </td>
                                        <td class="py-2.5 pr-4 font-mono text-xs">{p.year}</td>
                                        <td class="py-2.5 pr-4">{p.method}</td>
                                        <td class="py-2.5 pr-4">
                                            {if p.prunable {
                                                view! { <span class="text-green-400/70">"Yes"</span> }.into_any()
                                            } else {
                                                view! { <span class="text-red-400/70">"No"</span> }.into_any()
                                            }}
                                        </td>
                                        <td class="py-2.5 pr-4">{if p.fungible { "Fungible" } else { "Non-fungible" }}</td>
                                        <td class="py-2.5 text-xs">{p.status}</td>
                                    </tr>
                                }
                            }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
            </div>

            // CTA to charts
            <div class="text-center mt-10">
                <a href="/stats"
                    class="inline-flex items-center gap-2 px-5 py-2.5 bg-[#f7931a] text-white text-sm font-medium rounded-xl hover:bg-[#f4a949] hover:scale-[1.02] active:scale-[0.98] transition-all duration-200"
                >
                    "View Embedded Data Charts"
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                    </svg>
                </a>
            </div>
        </section>
    }
}
