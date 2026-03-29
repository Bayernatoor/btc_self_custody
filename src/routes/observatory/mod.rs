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

    view! {
        <Title text="The Bitcoin Observatory - WE HODL BTC"/>
        <section class="max-w-[1750px] mx-auto px-3 sm:px-4 lg:px-8 pt-6 sm:pt-10 pb-28 opacity-0 animate-fadeinone overflow-x-hidden">
            <ObservatoryNav/>
            <OverlayPanel/>
            <leptos_router::components::Outlet/>
            <BlockDetailModal/>
        </section>
    }
}
