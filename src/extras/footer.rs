use leptos::prelude::*;

#[component]
pub fn Footer() -> impl IntoView {
    view! {
        <footer class="border-t border-white/10 mt-16">
            <div class="max-w-6xl mx-auto px-6 py-10 lg:px-8">
                <div class="grid grid-cols-2 sm:grid-cols-4 gap-8 lg:gap-12">

                    // Column 1 - Branding
                    <div class="col-span-2 sm:col-span-1">
                        <div class="text-base font-semibold text-white uppercase tracking-wide">"WE HODL BTC"</div>
                        <p class="text-sm text-white/40 mt-2 leading-relaxed">
                            "A free resource to help bitcoiners easily take self-custody, whether it\u{2019}s 100 sats or 100 bitcoin. Also, dive into the Bitcoin rabbit hole and observe the network in real time, with live charts and 900,000+ blocks of history."
                        </p>
                        // Social icons
                        <div class="flex items-center gap-1 mt-4">
                            <a
                                href="https://github.com/Bayernatoor/btc_self_custody"
                                target="_blank"
                                rel="noopener noreferrer"
                                class="p-1.5 rounded-lg text-white/30 hover:text-[#f7931a] hover:bg-white/5 transition-all duration-200"
                                aria-label="GitHub"
                            >
                                <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 496 512">
                                    <path d="M165.9 397.4c0 2-2.3 3.6-5.2 3.6-3.3.3-5.6-1.3-5.6-3.6 0-2 2.3-3.6 5.2-3.6 3-.3 5.6 1.3 5.6 3.6zm-31.1-4.5c-.7 2 1.3 4.3 4.3 4.9 2.6 1 5.6 0 6.2-2s-1.3-4.3-4.3-5.2c-2.6-.7-5.5.3-6.2 2.3zm44.2-1.7c-2.9.7-4.9 2.6-4.6 4.9.3 2 2.9 3.3 5.9 2.6 2.9-.7 4.9-2.6 4.6-4.6-.3-1.9-3-3.2-5.9-2.9zM244.8 8C106.1 8 0 113.3 0 252c0 110.9 69.8 205.8 169.5 239.2 12.8 2.3 17.3-5.6 17.3-12.1 0-6.2-.3-40.4-.3-61.4 0 0-70 15-84.7-29.8 0 0-11.4-29.1-27.8-36.6 0 0-22.9-15.7 1.6-15.4 0 0 24.9 2 38.6 25.8 21.9 38.6 58.6 27.5 72.9 20.9 2.3-16 8.8-27.1 16-33.7-55.9-6.2-112.3-14.3-112.3-110.5 0-27.5 7.6-41.3 23.6-58.9-2.6-6.5-11.1-33.3 2.6-67.9 20.9-6.5 69 27 69 27 20-5.6 41.5-8.5 62.8-8.5s42.8 2.9 62.8 8.5c0 0 48.1-33.6 69-27 13.7 34.7 5.2 61.4 2.6 67.9 16 17.7 25.8 31.5 25.8 58.9 0 96.5-58.9 104.2-114.8 110.5 9.2 7.9 17 22.9 17 46.4 0 33.7-.3 75.4-.3 83.6 0 6.5 4.6 14.4 17.3 12.1C428.2 457.8 496 362.9 496 252 496 113.3 383.5 8 244.8 8z"/>
                                </svg>
                            </a>
                            <a
                                href="https://primal.net/Bayer"
                                target="_blank"
                                rel="noopener noreferrer"
                                class="p-1.5 rounded-lg text-white/30 hover:text-[#f7931a] hover:bg-white/5 transition-all duration-200"
                                aria-label="Nostr"
                            >
                                <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 512 512">
                                    <path d="M278.5 215.6 23 471c-9.4 9.4-9.4 24.6 0 33.9s24.6 9.4 33.9 0l57-57h68c49.7 0 97.9-14.4 139-41 11.1-7.2 5.5-23-7.8-23-5.1 0-9.2-4.1-9.2-9.2 0-4.1 2.7-7.6 6.5-8.8l81-24.3c2.5-.8 4.8-2.1 6.7-4l22.4-22.4c10.1-10.1 2.9-27.3-11.3-27.3H377c-5.1 0-9.2-4.1-9.2-9.2 0-4.1 2.7-7.6 6.5-8.8l112-33.6c4-1.2 7.4-3.9 9.3-7.7 10.8-21 16.4-44.5 16.4-68.6 0-41-16.3-80.3-45.3-109.3l-5.5-5.5C432.3 16.3 393 0 352 0s-80.3 16.3-109.3 45.3L139 149c-48 48-75 113.1-75 181v55.3l189.6-189.5c6.2-6.2 16.4-6.2 22.6 0 5.4 5.4 6.1 13.6 2.2 19.8z"/>
                                </svg>
                            </a>
                            <a
                                href="https://x.com/Bayernatoor"
                                target="_blank"
                                rel="noopener noreferrer"
                                class="p-1.5 rounded-lg text-white/30 hover:text-[#f7931a] hover:bg-white/5 transition-all duration-200"
                                aria-label="X"
                            >
                                <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
                                    <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/>
                                </svg>
                            </a>
                            <a
                                href="mailto:wehodlbtc@pm.me"
                                class="p-1.5 rounded-lg text-white/30 hover:text-[#f7931a] hover:bg-white/5 transition-all duration-200"
                                aria-label="Email"
                            >
                                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/>
                                </svg>
                            </a>
                        </div>
                    </div>

                    // Column 2 - Observatory
                    <div>
                        <h3 class="text-xs font-semibold text-white/60 uppercase tracking-wider mb-3">"Observatory"</h3>
                        <ul class="space-y-2">
                            <li><a href="/observatory" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"Dashboard"</a></li>
                            <li><a href="/observatory/stats" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"Stats Overview"</a></li>
                            <li><a href="/observatory/on-this-day" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"On This Day"</a></li>
                            <li><a href="/observatory/charts/network" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"Network Charts"</a></li>
                            <li><a href="/observatory/charts/fees" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"Fee Charts"</a></li>
                            <li><a href="/observatory/charts/mining" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"Mining Charts"</a></li>
                            <li><a href="/observatory/charts/embedded" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"Embedded Data"</a></li>
                            <li><a href="/observatory/signaling" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"BIP Signaling"</a></li>
                            <li><a href="/observatory/learn" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"Learn"</a></li>
                        </ul>
                    </div>

                    // Column 3 - Guides & Resources
                    <div>
                        <h3 class="text-xs font-semibold text-white/60 uppercase tracking-wider mb-3">"Resources"</h3>
                        <ul class="space-y-2">
                            <li><a href="/guides" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"Self-Custody Guides"</a></li>
                            <li><a href="/observatory/learn/protocols" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"Protocol Guide"</a></li>
                            <li><a href="/faq" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"FAQ"</a></li>
                            <li><a href="/about" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"About"</a></li>
                        </ul>
                    </div>

                    // Column 4 - Open Source
                    <div>
                        <h3 class="text-xs font-semibold text-white/60 uppercase tracking-wider mb-3">"Open Source"</h3>
                        <ul class="space-y-2">
                            <li><a href="https://github.com/Bayernatoor/btc_self_custody" target="_blank" rel="noopener noreferrer" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"Source Code"</a></li>
                            <li><a href="https://github.com/Bayernatoor/btc_self_custody/blob/master/LICENSE" target="_blank" rel="noopener noreferrer" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"GPLv3 License"</a></li>
                            <li><a href="https://github.com/Bayernatoor/btc_self_custody/issues" target="_blank" rel="noopener noreferrer" class="text-sm text-white/30 hover:text-[#f7931a] transition-colors">"Report a Bug"</a></li>
                        </ul>
                    </div>
                </div>

                // Bottom bar
                <div class="mt-8 pt-5 border-t border-white/5 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
                    <span class="text-xs text-white/25">
                        "\u{00a9} " {chrono::Utc::now().format("%Y").to_string()} " WE HODL BTC"
                    </span>
                    <span class="text-xs text-white/25">
                        "Built with "
                        <a href="https://github.com/leptos-rs/leptos" target="_blank" rel="noopener noreferrer" class="hover:text-[#f7931a] transition-colors">"Leptos"</a>
                        " + "
                        <a href="https://www.rust-lang.org" target="_blank" rel="noopener noreferrer" class="hover:text-[#f7931a] transition-colors">"Rust"</a>
                        " by "
                        <a href="https://github.com/Bayernatoor" target="_blank" rel="noopener noreferrer" class="hover:text-[#f7931a] transition-colors">"Bayer"</a>
                    </span>
                </div>
            </div>
        </footer>
    }
}
