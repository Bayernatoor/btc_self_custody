//! The Bitcoin Observatory - live blockchain analytics dashboard.
//!
//! This is the parent module for all observatory pages. `ObservatoryPage` is the
//! parent route component that provides shared state (range, overlays, live stats,
//! chart cache) via Leptos context and renders a common shell (hero banner,
//! navigation tabs, overlay panel, block detail modal) around child pages via `Outlet`.
//!
//! Routes:
//!   /observatory                     -> Readings (live network instruments)
//!   /observatory/heartbeat           -> Heartbeat (live EKG animation)
//!   /observatory/lookout             -> The Lookout (notable tx watcher)
//!   /observatory/signaling           -> BIP signaling tracker (version bits + coinbase)
//!   /observatory/logbook             -> The Logbook (network observations by range)
//!   /observatory/almanac             -> Almanac (date-based historical lookup)
//!   /observatory/archives            -> The Archives (curated notable events)
//!   /observatory/charts/network      -> Network charts (blocks, adoption, tx metrics)
//!   /observatory/charts/fees         -> Fee charts (total fees, subsidy breakdown)
//!   /observatory/charts/mining       -> Mining charts (difficulty, pool distribution)
//!   /observatory/charts/embedded     -> Embedded data charts (OP_RETURN, inscriptions)
//!   /observatory/learn/protocols     -> Protocol guide
//!
//! Legacy paths with 301 redirects (declared in `src/app.rs`):
//!   /observatory/stats         -> /observatory/logbook
//!   /observatory/on-this-day   -> /observatory/almanac
//!   /observatory/hall-of-fame  -> /observatory/archives
//!   /observatory/whale-watch   -> /observatory/lookout

pub mod components;
pub mod helpers;
pub mod learn;
pub mod shared;

mod embedded;
mod fees;
mod hall_of_fame;
mod heartbeat;
mod mining;
mod network;
mod on_this_day;
mod overview;
mod signaling;
mod single_chart;
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
pub use single_chart::SingleChartPage;
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
    // The single-chart view is meant to be dominated by the chart, so it drops
    // the section nav and takes the full width. The nav is still one click
    // away: the breadcrumb goes back to the chart's page, and the chart
    // drawer on the left edge reaches every registered chart directly.
    let solo_chart = Signal::derive(move || {
        location.pathname.get().starts_with("/observatory/chart/")
    });

    view! {
        // Title and meta description are set per sub-page for SEO
        // **No fade on this shell, deliberately.** Every other page on the
        // site opens with `opacity-0 animate-fadeinone`, a one second fade.
        // This is a data page: the reader came for a number and the chart is
        // already the slow part, so a second of deliberate invisibility on top
        // of the fetch is cost with no benefit. Content paints as soon as it
        // exists.
        //
        // Removing it also removes the stacking context that a forwards-filled
        // opacity animation leaves behind for good, which is what confined the
        // chart drawer and the block modal and forced both into a `Portal`.
        // Those portals stay: they are correct on their own terms and nothing
        // about this change requires unpicking them.
        <section
            class=move || if solo_chart.get() {
                // `pl-7` and `pb-24` below `sm` reserve the space two fixed
                // elements occupy: the drawer handle, a `w-5` sliver there, and
                // the settings button (`fixed right-4 bottom-4`). Neither is
                // in the flow, so on a phone they sat on top of the cards,
                // clipping "observations" and "OVERLAYS" on the left and
                // covering the export row at the bottom. Desktop never showed
                // it because the margins are wide enough to absorb both.
                "max-w-none mx-auto pl-7 pr-2 sm:px-3 lg:px-5 pt-3 sm:pt-4 pb-24 sm:pb-12"
            } else {
                "max-w-[1750px] mx-auto pl-7 pr-3 sm:px-4 lg:px-8 pt-6 sm:pt-10 pb-28"
            }
        >
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
            <Show when=move || !solo_chart.get()>
                <ObservatoryNav/>
            </Show>
            <components::NodeStatusBanner/>
            <ChartSettingsPanel/>
            <leptos_router::components::Outlet/>
            <BlockDetailModal/>
        </section>
    }
}
