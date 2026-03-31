//! The Bitcoin Observatory.
//!
//! Routes:
//!   /observatory                     -> Dashboard (live stats, difficulty, halving)
//!   /observatory/charts/network      -> Network charts
//!   /observatory/charts/fees         -> Fee charts
//!   /observatory/charts/mining       -> Mining charts
//!   /observatory/charts/embedded     -> Embedded data charts
//!   /observatory/signaling           -> BIP signaling tracker
//!   /observatory/learn/protocols     -> Protocol guide

pub mod components;
pub mod helpers;
pub mod learn;
pub mod shared;

mod embedded;
mod fees;
mod mining;
mod network;
mod overview;
mod signaling;
mod stats;

pub use embedded::EmbeddedChartsPage;
pub use fees::FeeChartsPage;
pub use mining::MiningChartsPage;
pub use network::NetworkChartsPage;
pub use overview::ObservatoryOverview;
pub use signaling::SignalingPage;
pub use stats::StatsSummaryPage;

use leptos::prelude::*;

use shared::*;

// ---------------------------------------------------------------------------
// Parent route view: always renders Outlet unconditionally so ParentRoute
// child navigation works. Shared state is provided via context here.
// ---------------------------------------------------------------------------

#[component]
pub fn ObservatoryPage() -> impl IntoView {
    let _state = provide_observatory_state();

    let location = leptos_router::hooks::use_location();
    let on_dashboard =
        Signal::derive(move || location.pathname.get() == "/observatory");

    view! {
        // Title and meta description are set per sub-page for SEO
        <section class="max-w-[1750px] mx-auto px-3 sm:px-4 lg:px-8 pt-6 sm:pt-10 pb-28 opacity-0 animate-fadeinone overflow-x-hidden">
            // Hero branding — only on dashboard
            <Show when=move || on_dashboard.get()>
                <div class="relative rounded-2xl overflow-hidden mb-6 sm:mb-8">
                    <img
                        src="/img/observatory_hero.png"
                        alt="The Bitcoin Observatory"
                        class="w-full h-[200px] sm:h-[260px] lg:h-[320px] object-cover object-center"
                    />
                    <div class="absolute inset-0 bg-gradient-to-t from-[#123c64] via-[#123c64]/50 to-transparent"></div>
                    <div class="absolute inset-0 flex flex-col items-center justify-end pb-4 sm:pb-5">
                        <h1 class="text-2xl sm:text-3xl lg:text-4xl font-title text-white mb-1.5 drop-shadow-lg">"The Bitcoin Observatory"</h1>
                        <p class="text-sm sm:text-base text-white/60 max-w-lg mx-auto px-4 text-center drop-shadow">
                            "Live blockchain metrics, block data, embedded data analysis, and BIP signaling tracker."
                        </p>
                    </div>
                </div>
            </Show>
            <ObservatoryNav/>
            <OverlayPanel/>
            <leptos_router::components::Outlet/>
            <BlockDetailModal/>
        </section>
    }
}
