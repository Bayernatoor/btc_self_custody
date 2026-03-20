use leptos::prelude::*;
use leptos_meta::*;

/// Renders the About page of the application.
#[component]
pub fn AboutPage() -> impl IntoView {
    view! {
        <Title text="About | WE HODL BTC"/>
        <div class="max-w-3xl mx-auto mt-10 mb-24 px-6 opacity-0 animate-fadeinone md:max-w-4xl lg:max-w-5xl lg:px-8 md:my-20">

            // Header
            <header class="text-center mb-10">
                <h1 class="text-[1.65rem] text-[#f7931a] font-semibold leading-tight font-title md:text-[2rem] lg:text-[2.5rem]">
                    "WE HODL BTC"
                </h1>
                <div class="w-16 h-0.5 bg-[#f7931a] mx-auto mt-3 mb-5"></div>
                <p class="text-[0.9rem] text-white/80 max-w-2xl mx-auto leading-relaxed">
                    "WE HODL BTC helps you take true ownership of your bitcoin — whether it's your first 100 sats or your life savings. Self-sovereignty starts with self-custody."
                </p>
            </header>

            // Cards grid
            <div class="grid gap-6 md:grid-cols-3">

                // About card
                <div class="bg-white/5 border border-white/10 rounded-xl p-5">
                    <h2 class="text-base text-[#f7931a] font-semibold mb-3">"About"</h2>
                    <p class="text-[0.85rem] text-white/80 leading-relaxed">
                        "I go by Bayer, I am a Bitcoiner who believes bitcoin is the most significant discovery of our time.
                        In a world conditioned to spend endlessly, bitcoin rewards those who embrace saving, fostering a mindset
                        of low time preference and incentivizing long-term thinking and planning."
                    </p>
                    <div class="flex flex-col gap-1.5 mt-4 text-[0.85rem]">
                        <a
                            class="inline-flex items-center gap-1.5 text-blue-400 hover:text-blue-300 transition-colors"
                            href="./../../../public_key.asc"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            <svg class="w-3.5 h-3.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"/>
                            </svg>
                            "PGP Key"
                        </a>
                        <a
                            class="inline-flex items-center gap-1.5 text-blue-400 hover:text-blue-300 transition-colors"
                            href="https://primal.net/bayer"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            <svg class="w-3.5 h-3.5 shrink-0" fill="currentColor" viewBox="0 0 512 512">
                                <path d="M278.5 215.6 23 471c-9.4 9.4-9.4 24.6 0 33.9s24.6 9.4 33.9 0l57-57h68c49.7 0 97.9-14.4 139-41 11.1-7.2 5.5-23-7.8-23-5.1 0-9.2-4.1-9.2-9.2 0-4.1 2.7-7.6 6.5-8.8l81-24.3c2.5-.8 4.8-2.1 6.7-4l22.4-22.4c10.1-10.1 2.9-27.3-11.3-27.3H377c-5.1 0-9.2-4.1-9.2-9.2 0-4.1 2.7-7.6 6.5-8.8l112-33.6c4-1.2 7.4-3.9 9.3-7.7 10.8-21 16.4-44.5 16.4-68.6 0-41-16.3-80.3-45.3-109.3l-5.5-5.5C432.3 16.3 393 0 352 0s-80.3 16.3-109.3 45.3L139 149c-48 48-75 113.1-75 181v55.3l189.6-189.5c6.2-6.2 16.4-6.2 22.6 0 5.4 5.4 6.1 13.6 2.2 19.8z"/>
                            </svg>
                            "Nostr"
                        </a>
                        <a
                            class="inline-flex items-center gap-1.5 text-blue-400 hover:text-blue-300 transition-colors"
                            href="https://github.com/Bayernatoor"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            <svg class="w-3.5 h-3.5 shrink-0" fill="currentColor" viewBox="0 0 496 512">
                                <path d="M165.9 397.4c0 2-2.3 3.6-5.2 3.6-3.3.3-5.6-1.3-5.6-3.6 0-2 2.3-3.6 5.2-3.6 3-.3 5.6 1.3 5.6 3.6zm-31.1-4.5c-.7 2 1.3 4.3 4.3 4.9 2.6 1 5.6 0 6.2-2s-1.3-4.3-4.3-5.2c-2.6-.7-5.5.3-6.2 2.3zm44.2-1.7c-2.9.7-4.9 2.6-4.6 4.9.3 2 2.9 3.3 5.9 2.6 2.9-.7 4.9-2.6 4.6-4.6-.3-1.9-3-3.2-5.9-2.9zM244.8 8C106.1 8 0 113.3 0 252c0 110.9 69.8 205.8 169.5 239.2 12.8 2.3 17.3-5.6 17.3-12.1 0-6.2-.3-40.4-.3-61.4 0 0-70 15-84.7-29.8 0 0-11.4-29.1-27.8-36.6 0 0-22.9-15.7 1.6-15.4 0 0 24.9 2 38.6 25.8 21.9 38.6 58.6 27.5 72.9 20.9 2.3-16 8.8-27.1 16-33.7-55.9-6.2-112.3-14.3-112.3-110.5 0-27.5 7.6-41.3 23.6-58.9-2.6-6.5-11.1-33.3 2.6-67.9 20.9-6.5 69 27 69 27 20-5.6 41.5-8.5 62.8-8.5s42.8 2.9 62.8 8.5c0 0 48.1-33.6 69-27 13.7 34.7 5.2 61.4 2.6 67.9 16 17.7 25.8 31.5 25.8 58.9 0 96.5-58.9 104.2-114.8 110.5 9.2 7.9 17 22.9 17 46.4 0 33.7-.3 75.4-.3 83.6 0 6.5 4.6 14.4 17.3 12.1C428.2 457.8 496 362.9 496 252 496 113.3 383.5 8 244.8 8z"/>
                            </svg>
                            "GitHub"
                        </a>
                    </div>
                </div>

                // Donate card
                <div class="bg-white/5 border border-white/10 rounded-xl p-5">
                    <h2 class="text-base text-[#f7931a] font-semibold mb-3">"Donate"</h2>
                    <p class="text-[0.85rem] text-white/80 leading-relaxed mb-4">
                        "Your contributions help keep the project running and are greatly appreciated."
                    </p>
                    <div class="flex flex-col gap-3 text-[0.85rem]">
                        <div>
                            <span class="text-white/50 text-xs uppercase tracking-wide">"Lightning"</span>
                            <a
                                class="block text-blue-400 hover:text-blue-300 transition-colors mt-0.5 break-all"
                                href="lightning:bayer@primal.net"
                            >
                                "bayer@primal.net"
                            </a>
                        </div>
                        <div>
                            <span class="text-white/50 text-xs uppercase tracking-wide">"PayNym (BIP47)"</span>
                            <a
                                class="block text-blue-400 hover:text-blue-300 transition-colors mt-0.5"
                                href="https://paynym.is/+wildhaze2Ff"
                                target="_blank"
                                rel="noopener noreferrer"
                            >
                                "+wildhaze2Ff"
                            </a>
                        </div>
                        <div>
                            <span class="text-white/50 text-xs uppercase tracking-wide">"On-chain"</span>
                            <a
                                class="block text-blue-400 hover:text-blue-300 transition-colors mt-0.5 text-[0.75rem] break-all"
                                href="bitcoin:bc1pg3l4kqvurd3w350mgr4amcplj7ar70gqyck9hzfu75w5ylrvl3rst84h3d"
                            >
                                "bc1pg3l4kqvurd3w350mgr4amcplj7ar70gqyck9hzfu75w5ylrvl3rst84h3d"
                            </a>
                            <img
                                class="h-auto w-28 mt-2 rounded"
                                src="./../../../bitcoin_donation_address_qr.png"
                                alt="On-chain bitcoin donation QR code"
                            />
                        </div>
                    </div>
                </div>

                // Contribute card
                <div class="bg-white/5 border border-white/10 rounded-xl p-5">
                    <h2 class="text-base text-[#f7931a] font-semibold mb-3">"Contribute"</h2>
                    <p class="text-[0.85rem] text-white/80 leading-relaxed">
                        "This project is entirely free and open-sourced under an MIT license.
                        Contributions are always welcome — feel free to open an "
                        <a
                            class="text-blue-400 hover:text-blue-300 transition-colors"
                            href="https://github.com/Bayernatoor/btc_self_custody"
                            target="_blank"
                            rel="noopener noreferrer"
                        >
                            "issue on GitHub"
                        </a>
                        "."
                    </p>
                    <div class="mt-4 pt-4 border-t border-white/10">
                        <p class="text-[0.85rem] text-white/80">
                            <span class="text-white font-medium">"Questions? "</span>
                            "Reach out via Nostr or "
                            <a
                                class="text-blue-400 hover:text-blue-300 transition-colors"
                                href="mailto:wehodlbtc@pm.me"
                            >
                                "email"
                            </a>
                            "."
                        </p>
                    </div>
                </div>

            </div>
        </div>
    }
}
