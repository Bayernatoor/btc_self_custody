use leptos_router::*;

// get the current path via the RouteContext
pub fn get_current_path() -> String {
    // Retrieve the URL path of the current route
    use_route().path()
}
