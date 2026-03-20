use leptos::prelude::Get;
use leptos_router::hooks::use_location;

// get the current path via the location
pub fn get_current_path() -> String {
    use_location().pathname.get()
}
