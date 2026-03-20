pub mod app;
pub mod extras;
pub mod helpers;
pub mod routes;

// --- Server modules (commented out until DB is reintroduced) ---
// pub mod configuration;
// pub mod server;
// pub mod telemetry;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::App;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
