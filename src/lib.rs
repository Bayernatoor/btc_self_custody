//! WE HODL BTC — Bitcoin self-custody guide website.
//!
//! Leptos 0.8 fullstack app (SSR + WASM hydration) with Axum backend.
//!
//! # Module layout
//! - `app`    — Router, HTML shell, and meta tags
//! - `guides` — Static definitions for wallets, levels, platforms, products
//! - `routes` — Page components (homepage, guide selector, guide pages, FAQ, about, blog)
//! - `extras` — Reusable UI components (navbar, footer, stepper, accordion, buttons, spinner)

#![recursion_limit = "512"]

pub mod app;
pub mod extras;
pub mod guides;
pub mod helpers;
pub mod routes;
pub mod stats;

/// WASM entry point — hydrates the server-rendered HTML.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::App;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
