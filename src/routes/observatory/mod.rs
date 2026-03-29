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

mod overview;
mod network;
mod fees;
mod mining;
mod embedded;
mod signaling;

pub use overview::ObservatoryOverview;
pub use network::NetworkChartsPage;
pub use fees::FeeChartsPage;
pub use mining::MiningChartsPage;
pub use embedded::EmbeddedChartsPage;
pub use signaling::SignalingPage;

use leptos::prelude::*;
use leptos_meta::*;

use shared::*;

// ---------------------------------------------------------------------------
// Parent route view: always renders Outlet unconditionally so ParentRoute
// child navigation works. Shared state is provided via context here.
// ---------------------------------------------------------------------------

#[component]
pub fn ObservatoryPage() -> impl IntoView {
    let _state = provide_observatory_state();

    let location = leptos_router::hooks::use_location();
    let on_dashboard = Signal::derive(move || location.pathname.get() == "/observatory");

    view! {
        <Title text="The Bitcoin Observatory - WE HODL BTC"/>
        <section class="max-w-[1750px] mx-auto px-3 sm:px-4 lg:px-8 pt-6 sm:pt-10 pb-28 opacity-0 animate-fadeinone overflow-x-hidden">
            // Hero branding — only on dashboard
            <Show when=move || on_dashboard.get()>
                <div class="text-center mb-6 sm:mb-10">
                    <h1 class="text-2xl sm:text-3xl lg:text-4xl font-title text-white mb-2 sm:mb-3">"The Bitcoin Observatory"</h1>
                    <div class="w-12 sm:w-16 h-0.5 bg-[#f7931a] mx-auto mt-2 sm:mt-3 mb-3 sm:mb-4"></div>
                    <p class="text-sm sm:text-base text-white/50 max-w-xl mx-auto px-2">
                        "Live blockchain metrics, block data, embedded data analysis, and BIP signaling tracker."
                    </p>
                </div>
            </Show>
            <ObservatoryNav/>
            <OverlayPanel/>
            <leptos_router::components::Outlet/>
            <BlockDetailModal/>
        </section>
    }
}
