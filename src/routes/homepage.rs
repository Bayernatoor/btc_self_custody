use leptos::prelude::*;
use leptos_meta::*;

/// Renders the home page of the application.
#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <Title text="WE HODL BTC - Bitcoin Self-Custody Guides"/>

        // Hero
        <section aria-label="Hero" class="grid gap-2 mx-auto justify-items-center max-w-5xl mt-14 px-6 opacity-0 animate-fadeinone md:grid-cols-1 lg:grid-cols-2 lg:max-w-6xl md:my-24 lg:pb-28 lg:px-8">
            <div class="flex flex-col text-center text-white leading-normal md:text-center lg:text-left md:pt-8 lg:pt-0">
                <h1 class="text-4xl font-title font-normal tracking-tight sm:text-5xl md:text-6xl lg:text-[5rem]">
                    "Be your" <br/> "own bank"
                </h1>
                <div class="lg:hidden flex flex-col justify-center pb-8 pt-5">
                    <div class="h-auto w-24 md:w-28 mx-auto">
                        <img src="./../../../bitcoin_logo.png" alt="Bitcoin logo" width="112" height="112"/>
                    </div>
                </div>
                <p class="text-sm max-w-xl mb-6 sm:text-base md:text-lg lg:text-xl md:my-8 leading-relaxed">
                    "Opinionated guides that cut through the noise to help you easily self-custody your Bitcoin."
                </p>
                <a href="/guides">
                    <button
                        role="button"
                        class="text-sm lg:text-base text-center bg-[#f79231] w-36 lg:w-40 mx-auto font-semibold no-underline border-none rounded-lg py-2.5 lg:py-3 px-4 lg:px-5 hover:bg-[#f4a949] cursor-pointer shadow-md hover:shadow-lg transition-all duration-300 lg:mx-0"
                    >
                        <span>"Start Hodling"</span>
                    </button>
                </a>
            </div>
            <div class="invisible flex flex-col justify-center lg:visible">
                <div class="h-auto w-28 md:w-40 lg:w-60 mx-auto">
                    <img src="./../../../bitcoin_logo.png" alt="Bitcoin logo" width="208" height="208"/>
                </div>
            </div>
        </section>

        // Why Self-Custody
        <section aria-label="Why self-custody" class="max-w-5xl mx-auto px-6 lg:max-w-6xl pt-8 pb-24 lg:pb-28 lg:px-8">
            <div class="text-center mb-10">
                <h2 class="text-xl sm:text-2xl lg:text-3xl font-title text-white mb-2">"Why Self-Custody?"</h2>
                <div class="w-12 h-0.5 bg-[#f7931a] mx-auto mt-2"></div>
            </div>
            <div class="grid gap-4 md:grid-cols-3">
                <div class="bg-white/5 border border-white/10 rounded-xl p-5 opacity-0 animate-slideup" style="animation-delay: 100ms">
                    <div class="w-9 h-9 rounded-lg bg-[#f7931a]/10 flex items-center justify-center mb-3">
                        <svg class="w-5 h-5 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"/>
                        </svg>
                    </div>
                    <h3 class="text-base font-semibold text-white mb-1.5">"Your Keys, Your Coins"</h3>
                    <p class="text-sm text-white/60 leading-relaxed">
                        "When you hold your own keys, no exchange, bank, or third party can freeze, seize, or lose your bitcoin. True ownership means true control."
                    </p>
                </div>
                <div class="bg-white/5 border border-white/10 rounded-xl p-5 opacity-0 animate-slideup" style="animation-delay: 200ms">
                    <div class="w-9 h-9 rounded-lg bg-[#f7931a]/10 flex items-center justify-center mb-3">
                        <svg class="w-5 h-5 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"/>
                        </svg>
                    </div>
                    <h3 class="text-base font-semibold text-white mb-1.5">"Eliminate Counterparty Risk"</h3>
                    <p class="text-sm text-white/60 leading-relaxed">
                        "Exchanges get hacked. Companies go bankrupt. Trusted third parties are security holes. Self-custody removes the middleman entirely."
                    </p>
                </div>
                <div class="bg-white/5 border border-white/10 rounded-xl p-5 opacity-0 animate-slideup" style="animation-delay: 300ms">
                    <div class="w-9 h-9 rounded-lg bg-[#f7931a]/10 flex items-center justify-center mb-3">
                        <svg class="w-5 h-5 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 104 0 2 2 0 012-2h1.064M15 20.488V18a2 2 0 012-2h3.064M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>
                        </svg>
                    </div>
                    <h3 class="text-base font-semibold text-white mb-1.5">"Financial Sovereignty"</h3>
                    <p class="text-sm text-white/60 leading-relaxed">
                        "Send and receive bitcoin anywhere, anytime, without permission. No borders, no business hours, no gatekeepers."
                    </p>
                </div>
            </div>
        </section>

        // Guide levels preview
        <section aria-label="Guide levels" class="max-w-5xl mx-auto px-6 lg:max-w-6xl pb-28 lg:pb-32 lg:px-8">
            <div class="text-center mb-10">
                <h2 class="text-xl sm:text-2xl lg:text-3xl font-title text-white mb-2">"A Guide For Every Level"</h2>
                <div class="w-12 h-0.5 bg-[#f7931a] mx-auto mt-2 mb-4"></div>
                <p class="text-sm sm:text-[0.9rem] text-white/50 max-w-lg mx-auto">"Whether you're stacking your first sats or securing generational wealth."</p>
            </div>
            <div class="grid gap-4 md:grid-cols-3">
                <a href="/guides" class="block opacity-0 animate-slideup" style="animation-delay: 100ms">
                    <div class="group bg-white/5 border border-white/10 rounded-xl p-5 hover:bg-white/10 hover:border-white/20 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 h-full">
                        <div class="text-[#f7931a] text-xs font-semibold uppercase tracking-widest mb-2">"Basic"</div>
                        <h3 class="text-base font-semibold text-white mb-1.5 group-hover:text-[#f4a949] transition-colors">"Mobile & Desktop Wallets"</h3>
                        <p class="text-sm text-white/50 leading-relaxed">
                            "Get started with Blue Wallet, Green Wallet, or Sparrow. Generate your keys and take possession of your bitcoin in minutes."
                        </p>
                    </div>
                </a>
                <a href="/guides" class="block opacity-0 animate-slideup" style="animation-delay: 200ms">
                    <div class="group bg-white/5 border border-white/10 rounded-xl p-5 hover:bg-white/10 hover:border-white/20 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 h-full">
                        <div class="text-[#f7931a] text-xs font-semibold uppercase tracking-widest mb-2">"Intermediate"</div>
                        <h3 class="text-base font-semibold text-white mb-1.5 group-hover:text-[#f4a949] transition-colors">"Hardware Wallet & Node"</h3>
                        <p class="text-sm text-white/50 leading-relaxed">
                            "Level up with a Coldcard hardware wallet and your own Bitcoin node. Standards-based security and real privacy."
                        </p>
                    </div>
                </a>
                <a href="/guides" class="block opacity-0 animate-slideup" style="animation-delay: 300ms">
                    <div class="group bg-white/5 border border-white/10 rounded-xl p-5 hover:bg-white/10 hover:border-white/20 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 h-full">
                        <div class="text-[#f7931a] text-xs font-semibold uppercase tracking-widest mb-2">"Advanced"</div>
                        <h3 class="text-base font-semibold text-white mb-1.5 group-hover:text-[#f4a949] transition-colors">"Multisig Setup"</h3>
                        <p class="text-sm text-white/50 leading-relaxed">
                            "2-of-3 multisig with multiple signing devices, steel seed backups, and geographic separation. Protect generational wealth."
                        </p>
                    </div>
                </a>
            </div>
        </section>

        // Observatory callout
        <section aria-label="The Bitcoin Observatory" class="max-w-5xl mx-auto px-6 lg:max-w-6xl pb-28 lg:pb-32 lg:px-8">
            <div class="text-center mb-10">
                <h2 class="text-xl sm:text-2xl lg:text-3xl font-title text-white mb-2">"Explore the Blockchain"</h2>
                <div class="w-12 h-0.5 bg-[#f7931a] mx-auto mt-2 mb-4"></div>
                <p class="text-sm sm:text-[0.9rem] text-white/50 max-w-lg mx-auto">"Live data from my full Bitcoin node."</p>
            </div>

            // Hero banner
            <a href="/observatory" class="block opacity-0 animate-slideup" style="animation-delay: 100ms">
                <div class="group relative rounded-2xl overflow-hidden mb-6 hover:scale-[1.01] active:scale-[0.99] transition-transform duration-200">
                    <img
                        src="/observatory_hero.png"
                        alt="The Bitcoin Observatory"
                        class="w-full h-[140px] sm:h-[180px] lg:h-[220px] object-cover object-center"
                        loading="lazy"
                    />
                    <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/50 to-transparent"></div>
                    <div class="absolute inset-0 flex flex-col items-center justify-end pb-4 sm:pb-6">
                        <h3 class="text-lg sm:text-xl lg:text-2xl font-title text-white mb-1 drop-shadow-lg group-hover:text-[#f4a949] transition-colors">"The Bitcoin Observatory"</h3>
                        <p class="text-xs sm:text-sm text-white/50 drop-shadow">"Live blockchain analytics, charts, and BIP signaling tracker"</p>
                    </div>
                </div>
            </a>

            // Feature cards
            <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                <a href="/observatory" class="block opacity-0 animate-slideup" style="animation-delay: 200ms">
                    <div class="group bg-white/5 border border-white/10 rounded-xl p-4 hover:bg-white/10 hover:border-white/20 transition-all duration-200 h-full">
                        <div class="w-8 h-8 rounded-lg bg-[#f7931a]/10 flex items-center justify-center mb-2.5">
                            <svg class="w-4 h-4 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z"/>
                            </svg>
                        </div>
                        <h4 class="text-sm font-semibold text-white mb-1 group-hover:text-[#f4a949] transition-colors">"Live Dashboard"</h4>
                        <p class="text-xs text-white/40 leading-relaxed">"Real-time mempool, mining stats, difficulty, from my own node."</p>
                    </div>
                </a>
                <a href="/observatory/charts/network" class="block opacity-0 animate-slideup" style="animation-delay: 300ms">
                    <div class="group bg-white/5 border border-white/10 rounded-xl p-4 hover:bg-white/10 hover:border-white/20 transition-all duration-200 h-full">
                        <div class="w-8 h-8 rounded-lg bg-[#f7931a]/10 flex items-center justify-center mb-2.5">
                            <svg class="w-4 h-4 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4h18M4 4h16v12a1 1 0 01-1 1H5a1 1 0 01-1-1V4z"/>
                            </svg>
                        </div>
                        <h4 class="text-sm font-semibold text-white mb-1 group-hover:text-[#f4a949] transition-colors">"30+ Charts"</h4>
                        <p class="text-xs text-white/40 leading-relaxed">"Block size, fees, SegWit and Taproot adoption, mining pools, and transaction metrics."</p>
                    </div>
                </a>
                <a href="/observatory/charts/embedded" class="block opacity-0 animate-slideup" style="animation-delay: 400ms">
                    <div class="group bg-white/5 border border-white/10 rounded-xl p-4 hover:bg-white/10 hover:border-white/20 transition-all duration-200 h-full">
                        <div class="w-8 h-8 rounded-lg bg-[#f7931a]/10 flex items-center justify-center mb-2.5">
                            <svg class="w-4 h-4 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4"/>
                            </svg>
                        </div>
                        <h4 class="text-sm font-semibold text-white mb-1 group-hover:text-[#f4a949] transition-colors">"Embedded Data"</h4>
                        <p class="text-xs text-white/40 leading-relaxed">"Track Runes, Ordinals, BRC-20, and OP_RETURN protocols across every block."</p>
                    </div>
                </a>
                <a href="/observatory/signaling" class="block opacity-0 animate-slideup" style="animation-delay: 500ms">
                    <div class="group bg-white/5 border border-white/10 rounded-xl p-4 hover:bg-white/10 hover:border-white/20 transition-all duration-200 h-full">
                        <div class="w-8 h-8 rounded-lg bg-[#f7931a]/10 flex items-center justify-center mb-2.5">
                            <svg class="w-4 h-4 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"/>
                            </svg>
                        </div>
                        <h4 class="text-sm font-semibold text-white mb-1 group-hover:text-[#f4a949] transition-colors">"BIP Signaling"</h4>
                        <p class="text-xs text-white/40 leading-relaxed">"Monitor miner signaling for active Bitcoin Improvement Proposals in real time."</p>
                    </div>
                </a>
            </div>
        </section>
    }
}
