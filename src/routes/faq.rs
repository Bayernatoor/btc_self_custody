use crate::extras::accordion_menu::AccordionMenu;
use crate::extras::schema::{static_docs, StaticJsonLd};
use leptos::prelude::*;
use leptos_meta::*;

/// Renders the FAQ / Help Desk page.
#[component]
pub fn FaqPage() -> impl IntoView {
    view! {
        <Title text="Bitcoin FAQ & Help Desk | We Hodl BTC"/>
        <Meta name="description" content="Frequently asked questions about Bitcoin, self-custody, wallets, mining, transaction fees, private keys, and The Bitcoin Observatory blockchain analytics."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/faq"/>
        <StaticJsonLd doc=static_docs::FAQ/>

        <div class="max-w-5xl mx-auto px-6 mt-14 mb-24 opacity-0 animate-fadeinone lg:max-w-6xl lg:px-8 md:my-24">

            // Header
            <header class="text-center mb-12">
                <h1 class="text-3xl text-[#f7931a] font-semibold leading-tight font-title lg:text-4xl">
                    "The Bitcoin Help Desk"
                </h1>
                <div class="w-16 h-0.5 bg-[#f7931a] mx-auto mt-3 mb-5"></div>

                // Quote
                <div class="max-w-xl mx-auto mb-6">
                    <p class="text-base text-white/60 italic leading-relaxed">
                        "\"Free software is a matter of liberty, not price. To understand the concept, you should think of 'free' as in 'free speech,' not as in 'free beer'.\""
                    </p>
                    <p class="text-xs text-white/40 mt-1">"— Richard Stallman"</p>
                </div>

                // Intro
                <div class="bg-white/5 border border-white/10 rounded-xl p-5 max-w-2xl mx-auto">
                    <p class="text-sm text-white/75 lg:text-[0.95rem] leading-relaxed">
                        "The guides are opinionated, recommending only a few options to cut through the noise and streamline the self-custody process.
                        All software recommendations use open and permissive licenses - you don't need to trust me, you can verify it yourself."
                    </p>
                </div>
            </header>

            // FAQ Accordion
            <section>
                <h2 class="text-base text-[#f7931a] font-semibold text-center mb-4">"Commonly Asked Questions"</h2>
                <AccordionMenu faq_name="general".to_string()/>
            </section>

            // Contact
            <section class="mt-10 pt-8 border-t border-white/10">
                <div class="flex flex-col items-center text-center">
                    <h3 class="text-base text-[#f7931a] font-semibold mb-4">"Need More Help?"</h3>
                    <a
                        class="inline-flex items-center gap-2 px-5 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white/75 hover:bg-white/10 hover:border-white/20 transition-all duration-200"
                        href="mailto:wehodlbtc@pm.me"
                    >
                        <svg class="w-4 h-4 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/>
                        </svg>
                        "wehodlbtc@pm.me"
                    </a>
                </div>
            </section>
        </div>
    }
}
