//! Learn index page: hub linking to all educational articles.
//! Route: /observatory/learn

use leptos::prelude::*;
use leptos_meta::*;

struct Article {
    title: &'static str,
    description: &'static str,
    href: &'static str,
    icon: &'static str,
}

const ARTICLES: &[Article] = &[
    Article {
        title: "Protocol Guide",
        description: "How Bitcoin data embedding protocols work: Omni Layer, Counterparty, Runes, Ordinals, BRC-20, and Stamps. History, encoding methods, and on-chain footprint.",
        href: "/observatory/learn/protocols",
        icon: r#"<path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 0 0-3.375-3.375h-1.5A1.125 1.125 0 0 1 13.5 7.125v-1.5a3.375 3.375 0 0 0-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 0 0-9-9Z"/>"#,
    },
    Article {
        title: "Data Methodology",
        description: "How we detect, count, and measure embedded protocol data. Byte accounting, detection heuristics, confidence levels, known exclusions, and the non-overlapping total formula.",
        href: "/observatory/learn/methodology",
        icon: r#"<path stroke-linecap="round" stroke-linejoin="round" d="M9.75 3.104v5.714a2.25 2.25 0 0 1-.659 1.591L5 14.5M9.75 3.104c-.251.023-.501.05-.75.082m.75-.082a24.301 24.301 0 0 1 4.5 0m0 0v5.714c0 .597.237 1.17.659 1.591L19.8 15.3M14.25 3.104c.251.023.501.05.75.082M19.8 15.3l-1.57.393A9.065 9.065 0 0 1 12 15a9.065 9.065 0 0 0-6.23.693L5 14.5m14.8.8 1.402 1.402c1.232 1.232.65 3.318-1.067 3.611A48.309 48.309 0 0 1 12 21c-2.773 0-5.491-.235-8.135-.687-1.718-.293-2.3-2.379-1.067-3.61L5 14.5"/>"#,
    },
];

#[component]
pub fn LearnIndexPage() -> impl IntoView {
    view! {
        <Title text="Learn | WE HODL BTC"/>
        <Meta name="description" content="Educational articles about Bitcoin data analysis: protocol guides, methodology documentation, and how to interpret observatory charts."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/observatory/learn"/>

        // Hero (matches other Observatory pages)
        <div class="relative rounded-2xl overflow-hidden mb-5">
            <img
                src="/img/observatory_hero.png"
                alt="Learn"
                class="w-full h-[100px] sm:h-[120px] lg:h-[140px] object-cover object-center"
            />
            <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/60 to-[#123c64]/30"></div>
            <div class="absolute inset-0 flex flex-col items-center justify-end pb-3 sm:pb-4">
                <h1 class="text-lg sm:text-xl lg:text-2xl font-title text-white mb-0.5 drop-shadow-lg">"Learn"</h1>
                <p class="text-[11px] sm:text-xs text-white/50 max-w-lg mx-auto px-4 text-center drop-shadow">"Educational articles about Bitcoin data analysis and the methodology behind the observatory"</p>
            </div>
        </div>

        <div class="max-w-3xl mx-auto space-y-8 pb-16">
            <div class="space-y-4">
                {ARTICLES.iter().map(|article| {
                    view! {
                        <a
                            href=article.href
                            class="block bg-[#0d2137] border border-white/10 rounded-2xl p-6 hover:border-[#f7931a]/30 transition-colors group"
                        >
                            <div class="flex items-start gap-4">
                                <div class="w-10 h-10 rounded-lg bg-[#f7931a]/10 flex items-center justify-center shrink-0 group-hover:bg-[#f7931a]/20 transition-colors">
                                    <svg class="w-5 h-5 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5" inner_html=article.icon></svg>
                                </div>
                                <div>
                                    <h2 class="text-lg text-white font-semibold group-hover:text-[#f7931a] transition-colors">
                                        {article.title}
                                    </h2>
                                    <p class="text-sm text-white/50 mt-1 leading-relaxed">
                                        {article.description}
                                    </p>
                                </div>
                            </div>
                        </a>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}
