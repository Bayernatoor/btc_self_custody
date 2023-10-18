use leptos::*;
use leptos_router::*;

// get the current path via the RouteContext
pub fn get_current_path() -> String {
    // Retrieve the URL path of the current route
    let current_page = use_route().path();

    current_page 
}
