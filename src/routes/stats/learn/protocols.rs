//! Protocol Guide: educational page about Bitcoin data embedding protocols.
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
        year: "2013/07",
        method: "OP_RETURN",
        method_detail: "Commonly encodes token operations in OP_RETURN outputs using the \"omni\" magic prefix (hex: 6f6d6e69). Earlier versions used other encoding methods.",
        color: "#3b82f6",

        description: "One of the earliest token protocols on Bitcoin, originally launched as Mastercoin in 2013. Tether (USDT) initially launched on Omni in 2014 before activity later shifted primarily to Ethereum and Tron.",
        how_it_works: "Omni commonly encodes protocol messages in OP_RETURN outputs using the 4-byte \"omni\" marker, followed by binary payloads for operations such as send, issue, and trade. Omni-aware nodes index and interpret these messages to track token balances.",
        fun_fact: "During 2017-2018, Omni-related transactions represented a significant portion of all OP_RETURN activity on Bitcoin, largely driven by Tether transfers.",
        status: "Low activity",
        prunable: true,
        fungible: true,
    },
    Protocol {
        id: "counterparty",
        name: "Counterparty",
        year: "2014/01",
        method: "OP_RETURN",
        method_detail: "Messages identified by the \"CNTRPRTY\" prefix (hex: 434e545250525459), embedded via OP_RETURN and historically also multisig-style encodings.",
        color: "#f59e0b",

        description: "Created XCP tokens through a proof-of-burn process in which roughly 2,100 BTC were sent to an unspendable address. Counterparty enabled token issuance, a decentralized exchange, and other financial primitives on Bitcoin.",
        how_it_works: "Counterparty protocol messages were embedded in Bitcoin transactions using methods including OP_RETURN and multisig-style encodings. Messages are identified by the \"CNTRPRTY\" prefix and decoded by Counterparty nodes to execute token operations.",
        fun_fact: "The BTC burned to create XCP was worth roughly $1.8M at the time. At today's prices it would be worth considerably more.",
        status: "Some activity",
        prunable: true,
        fungible: true,
    },
    Protocol {
        id: "stamps",
        name: "Stamps / SRC-20",
        year: "2023/03",
        method: "Bare Multisig",
        method_detail: "Embeds data in bare multisig output scripts (not P2SH-wrapped). Supposed \"public keys\" in the multisig actually encode chunks of image data.",
        color: "#94a3b8",

        description: "Embeds image data in bare multisig-style output scripts rather than witness data. Unlike Ordinals inscriptions, the encoded data is placed in non-witness transaction outputs; while such outputs remain unspent, they consume UTXO set space rather than prunable witness space. This persistence was a deliberate design goal.",
        how_it_works: "A transaction creates bare multisig outputs in which some supposed \"public keys\" actually encode chunks of image data. Stamp-aware indexers reconstruct the asset data from these script elements. While these outputs remain unspent, they occupy UTXO set space on every full node.",
        fun_fact: "Stamps emerged partly as a response to discussions in the Bitcoin community about potentially pruning or filtering Ordinals witness data.",
        status: "Active",
        prunable: false,
        fungible: false,
    },
    Protocol {
        id: "ordinals",
        name: "Ordinals",
        year: "2023/01/21",
        method: "Witness Data",
        method_detail: "Inscribes data inside a Taproot witness script using an envelope: OP_FALSE OP_IF OP_PUSH \"ord\" [content_type] [data] OP_ENDIF.",
        color: "#ec4899",

        description: "Created by Casey Rodarmor. Introduces \"ordinal theory,\" a convention for assigning serial numbers to individual satoshis, and enables inscribing arbitrary data (images, text, HTML) into the witness field of transactions. Inscription data benefits from the SegWit witness discount: witness bytes count at one quarter the weight of non-witness bytes.",
        how_it_works: "The inscription is placed inside a Taproot script-path spend. The witness contains an envelope with OP_FALSE OP_IF to create a no-op branch that carries the data. Since it's in the witness, it benefits from the 75% weight discount introduced by SegWit.",
        fun_fact: "The first known inscription (block 774,628) was a pixel art image. Inscription activity grew rapidly, with over a million inscriptions created within the first few months.",
        status: "Active",
        prunable: true,
        fungible: false,
    },
    Protocol {
        id: "brc20",
        name: "BRC-20",
        year: "2023/03/08",
        method: "Witness (JSON)",
        method_detail: "Ordinals inscriptions, typically with text content containing JSON such as {\"p\":\"brc-20\",\"op\":\"mint\",...}.",
        color: "#f472b6",

        description: "An experimental fungible-token convention built on Ordinals inscriptions. Uses JSON payloads with deploy, mint, and transfer operations. Bitcoin's consensus rules do not validate BRC-20 state; off-chain indexers interpret the inscriptions to determine token balances. BRC-20 activity was a major contributor to elevated fees and mempool congestion in mid-2023.",
        how_it_works: "A BRC-20 operation is an Ordinals inscription, typically with text content containing JSON like {\"p\":\"brc-20\",\"op\":\"mint\",\"tick\":\"ordi\",\"amt\":\"1000\"}. External indexers parse these inscriptions, apply the BRC-20 rules, and compute token balances. None of this logic exists in Bitcoin itself.",
        fun_fact: "BRC-20 is entirely off-protocol: if two indexers disagree on how to interpret the JSON rules, users could see different token balances depending on which indexer they trust.",
        status: "Active",
        prunable: true,
        fungible: true,
    },
    Protocol {
        id: "runes",
        name: "Runes",
        year: "2024/04/20",
        method: "OP_RETURN",
        method_detail: "Stores a \"runestone\" message in an OP_RETURN output using compact integer-based encoding for token operations.",
        color: "#ff6b6b",

        description: "A fungible token protocol created by Casey Rodarmor as a simpler, more UTXO-native alternative to BRC-20. Runes launched at Bitcoin's fourth halving block.",
        how_it_works: "Runes stores a \"runestone\" message in an OP_RETURN output using compact encoded fields for operations such as etching, minting, and transfers. Token balances are assigned to UTXOs, so transfers happen through normal UTXO spending and creation rather than inscription-indexed account balances.",
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
        <Title text="Embedding Protocols | The Bitcoin Observatory"/>
        <section class="max-w-6xl mx-auto px-6 pt-12 pb-24 opacity-0 animate-fadeinone">

            // Hero
            <div class="text-center mb-14">
                <a href="/stats" class="text-sm text-white/40 hover:text-white/60 transition-colors">
                    "\u{2190} Back to The Bitcoin Observatory"
                </a>
                <h1 class="text-4xl lg:text-5xl font-title text-white mt-4 mb-3">
                    "Bitcoin Embedding Protocols"
                </h1>
                <div class="w-16 h-0.5 bg-[#f7931a] mx-auto mb-5"></div>
                <p class="text-base lg:text-lg text-white/70 max-w-2xl mx-auto leading-relaxed">
                    "Since 2013, developers have found ways to embed non-financial data into Bitcoin's blockchain. From token protocols to digital art, each approach uses a different part of the transaction structure, with different trade-offs for cost, pruning, and chain impact."
                </p>
            </div>

            // Vertical timeline
            <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-6 lg:p-8 mb-10">
                <h2 class="text-sm text-white/60 uppercase tracking-widest font-semibold mb-6">"Timeline"</h2>
                <div class="relative pl-8">
                    // Vertical line
                    <div class="absolute left-3 top-0 bottom-0 w-px bg-white/10"></div>

                    {PROTOCOLS.iter().map(|p| {
                        let id = p.id.to_string();
                        let id_click = id.clone();
                        let id_check = id.clone();
                        view! {
                            <button
                                class="relative flex items-center gap-4 w-full text-left py-3 px-4 -ml-4 rounded-xl cursor-pointer transition-all hover:bg-white/5 group"
                                class=("bg-white/5", move || active_protocol.get() == id_check)
                                on:click={
                                    let id = id_click.clone();
                                    let scroll_id = p.id.to_string();
                                    move |_| {
                                        set_active_protocol.update(|a| {
                                            if *a == id { a.clear() } else { *a = id.clone() }
                                        });
                                        // Smooth scroll to the protocol card via JS
                                        let target = format!("protocol-{}", scroll_id);
                                        leptos::prelude::document()
                                            .get_element_by_id(&target)
                                            .map(|el| {
                                                // Use basic scroll_into_view (smooth via CSS scroll-behavior)
                                                el.scroll_into_view();
                                            });
                                    }
                                }
                            >
                                // Dot on the timeline
                                <div class="absolute left-[-1.07rem] w-3 h-3 rounded-full border-2 border-[#0d2137] shrink-0"
                                    style=format!("background: {}", p.color)
                                ></div>
                                // Year
                                <span class="text-xs text-white/40 font-mono w-[4.5rem] shrink-0">{p.year}</span>
                                // Name + method
                                <div class="flex items-center gap-2.5 flex-1 min-w-0">
                                    <span class="text-base text-white/80 font-semibold group-hover:text-white transition-colors">{p.name}</span>
                                    <span class="text-xs px-2 py-0.5 rounded-full border text-white/50 shrink-0"
                                        style=format!("border-color: {}50", p.color)
                                    >{p.method}</span>
                                </div>
                                // Status
                                <span class="text-xs text-white/30 shrink-0 hidden lg:block">{p.status}</span>
                                // Down arrow (scroll to details)
                                <svg class="w-4 h-4 text-white/20 group-hover:text-white/40 transition-colors shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7"/>
                                </svg>
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                </div>
            </div>

            // Protocol cards
            <div class="space-y-8">
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
                            <div class="p-6 lg:p-8">
                                <div class="flex items-start gap-4">
                                    <div class="w-1.5 h-14 rounded-full shrink-0" style=format!("background: {}", p.color)></div>
                                    <div class="flex-1">
                                        <div class="flex items-baseline flex-wrap gap-x-3 gap-y-1 mb-3">
                                            <h3 class="text-2xl text-white font-semibold">{p.name}</h3>
                                            <span class="text-sm text-white/50 font-mono">{p.year}</span>
                                            <span class="text-sm px-2.5 py-0.5 rounded-full border text-white/70"
                                                style=format!("border-color: {}50", p.color)
                                            >{p.method}</span>
                                            <span class="text-sm px-2 py-0.5 rounded-full bg-white/5 text-white/50">{p.status}</span>
                                        </div>
                                        <p class="text-base text-white/70 leading-relaxed">{p.description}</p>
                                    </div>
                                </div>
                            </div>

                            // How it works
                            <div class="border-t border-white/5 px-6 lg:px-8 py-5 bg-[#0a1a2e]">
                                <h4 class="text-xs text-[#8899aa] uppercase tracking-widest mb-3">"How it works"</h4>
                                <p class="text-sm text-white/65 leading-relaxed mb-3">{p.how_it_works}</p>
                                <code class="text-sm text-[#f7931a]/80 font-mono">{p.method_detail}</code>
                            </div>

                            // Footer: fun fact + properties
                            <div class="border-t border-white/5 px-6 lg:px-8 py-5 flex flex-col lg:flex-row lg:items-center gap-3">
                                <p class="text-sm text-white/50 italic flex-1">
                                    <span class="text-[#f7931a]/70">{"\u{1f4a1} "}</span>
                                    {p.fun_fact}
                                </p>
                                <div class="flex gap-2 shrink-0">
                                    <span class="text-xs px-2.5 py-1 rounded border border-white/15"
                                        class=("text-green-400/80", p.prunable)
                                        class=("text-red-400/80", !p.prunable)
                                    >
                                        {if p.prunable { "Prunable" } else { "Unprunable" }}
                                    </span>
                                    <span class="text-xs px-2.5 py-1 rounded border border-white/15 text-white/50">
                                        {if p.fungible { "Fungible tokens" } else { "Non-fungible / data" }}
                                    </span>
                                </div>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>

            // Historical note: Colored Coins
            <div class="bg-[#0a1a2e] border border-white/5 rounded-xl p-6 mt-10">
                <div class="flex items-start gap-3">
                    <span class="text-white/30 text-xl shrink-0">{"\u{1f4dc}"}</span>
                    <div>
                        <h4 class="text-base text-white/60 font-semibold mb-2">"Historical note: Colored Coins (2012\u{2013}2015)"</h4>
                        <p class="text-sm text-white/50 leading-relaxed">
                            "Colored Coins was the earliest concept for representing tokens on Bitcoin, predating all protocols listed above. Rather than a single protocol, it was a family of incompatible implementations (Open Assets, EPOBC, ChromaWay) that used various techniques, from OP_RETURN markers to transaction output ordering, to \"color\" specific satoshis as representing real-world assets. Most implementations relied on external metadata servers, making on-chain detection unreliable. Colored Coins saw limited adoption but were historically significant as the first tokenization experiments on Bitcoin, directly inspiring later protocols like Counterparty and Omni. They are not tracked in the Observatory charts due to their ambiguous on-chain footprint and low volume."
                        </p>
                    </div>
                </div>
            </div>

            // Comparison table
            <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-6 lg:p-8 mt-10">
                <h2 class="text-sm text-white/60 uppercase tracking-widest font-semibold mb-5">"Comparison"</h2>
                <div class="overflow-x-auto">
                    <table class="w-full text-base">
                        <thead>
                            <tr class="text-left text-white/50 text-xs uppercase tracking-wider">
                                <th class="pb-3 pr-4">"Protocol"</th>
                                <th class="pb-3 pr-4">"Year"</th>
                                <th class="pb-3 pr-4">"Embedding Method"</th>
                                <th class="pb-3 pr-4">"Prunable?"</th>
                                <th class="pb-3 pr-4">"Token Type"</th>
                                <th class="pb-3">"Status"</th>
                            </tr>
                        </thead>
                        <tbody class="text-white/70">
                            {PROTOCOLS.iter().map(|p| {
                                view! {
                                    <tr class="border-t border-white/5">
                                        <td class="py-3 pr-4 font-medium text-white/85">
                                            <div class="flex items-center gap-2">
                                                <div class="w-2 h-2 rounded-full" style=format!("background: {}", p.color)></div>
                                                {p.name}
                                            </div>
                                        </td>
                                        <td class="py-3 pr-4 font-mono text-sm">{p.year}</td>
                                        <td class="py-3 pr-4">{p.method}</td>
                                        <td class="py-3 pr-4">
                                            {if p.prunable {
                                                view! { <span class="text-green-400/70">"Yes"</span> }.into_any()
                                            } else {
                                                view! { <span class="text-red-400/70">"No"</span> }.into_any()
                                            }}
                                        </td>
                                        <td class="py-3 pr-4">{if p.fungible { "Fungible" } else { "Non-fungible" }}</td>
                                        <td class="py-3 text-sm">{p.status}</td>
                                    </tr>
                                }
                            }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
            </div>

            // CTA to charts
            <div class="text-center mt-12">
                <a href="/stats"
                    class="inline-flex items-center gap-2 px-6 py-3 bg-[#f7931a] text-white text-base font-medium rounded-xl hover:bg-[#f4a949] hover:scale-[1.02] active:scale-[0.98] transition-all duration-200"
                >
                    "Explore The Bitcoin Observatory"
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                    </svg>
                </a>
                <p class="text-xs text-white/30 mt-3">"View embedded data charts in the Embedded Data tab"</p>
            </div>
        </section>
    }
}
