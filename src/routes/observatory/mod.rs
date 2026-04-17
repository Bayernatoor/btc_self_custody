//! The Bitcoin Observatory - live blockchain analytics dashboard.
//!
//! This is the parent module for all observatory pages. `ObservatoryPage` is the
//! parent route component that provides shared state (range, overlays, live stats,
//! chart cache) via Leptos context and renders a common shell (hero banner,
//! navigation tabs, overlay panel, block detail modal) around child pages via `Outlet`.
//!
//! Routes:
//!   /observatory                     -> Readings (live network instruments)
//!   /observatory/charts/network      -> Network charts (blocks, adoption, tx metrics)
//!   /observatory/charts/fees         -> Fee charts (total fees, subsidy breakdown)
//!   /observatory/charts/mining       -> Mining charts (difficulty, pool distribution)
//!   /observatory/charts/embedded     -> Embedded data charts (OP_RETURN, inscriptions)
//!   /observatory/signaling           -> BIP signaling tracker (version bits + coinbase)
//!   /observatory/stats               -> The Logbook (network observations by range)
//!   /observatory/on-this-day         -> Almanac (date-based historical lookup)
//!   /observatory/hall-of-fame        -> The Archives (curated notable events)
//!   /observatory/heartbeat           -> Heartbeat (live EKG animation)
//!   /observatory/whale-watch         -> The Lookout (notable tx watcher)
//!   /observatory/learn/protocols     -> Protocol guide

pub mod components;
pub mod helpers;
pub mod learn;
pub mod shared;

mod embedded;
mod fees;
mod hall_of_fame;
mod hall_of_fame_data;
mod heartbeat;
mod mining;
mod network;
mod on_this_day;
mod overview;
mod signaling;
mod stats;
mod whale_watch;

pub use embedded::EmbeddedChartsPage;
pub use fees::FeeChartsPage;
pub use hall_of_fame::HallOfFamePage;
pub use heartbeat::HeartbeatPage;
pub use mining::MiningChartsPage;
pub use network::NetworkChartsPage;
pub use on_this_day::OnThisDayPage;
pub use overview::ObservatoryOverview;
pub use signaling::SignalingPage;
pub use stats::StatsSummaryPage;
pub use whale_watch::WhaleWatchPage;

use leptos::prelude::*;

use shared::*;

// ---------------------------------------------------------------------------
// Parent route view: always renders Outlet unconditionally so ParentRoute
// child navigation works. Shared state is provided via context here.
// ---------------------------------------------------------------------------

/// Parent route component for the observatory. Provides `ObservatoryState` and
/// `LiveContext` via context, renders the hero banner (dashboard only), navigation
/// tabs, overlay panel, block detail modal, and the child page via `Outlet`.
#[component]
pub fn ObservatoryPage() -> impl IntoView {
    let _state = provide_observatory_state();

    let location = leptos_router::hooks::use_location();
    let on_dashboard =
        Signal::derive(move || location.pathname.get() == "/observatory");

    view! {
        // Title and meta description are set per sub-page for SEO
        <section class="max-w-[1750px] mx-auto px-3 sm:px-4 lg:px-8 pt-6 sm:pt-10 pb-28 opacity-0 animate-fadeinone">
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
                            "Real-time readings, historical records, and 50+ charts of the Bitcoin network."
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
