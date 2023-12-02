#![allow(unused_imports)]
#[cfg(feature = "ssr")]
use actix_web::main;

#[cfg(feature = "ssr")]
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    use btc_self_custody::{
        configuration::{self, get_configuration},
        run,
    };
    let configuration = get_configuration().expect("Failed to read config");
    run().await?.await
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
