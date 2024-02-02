#![allow(unused_imports)]
#[cfg(feature = "ssr")]
use actix_web::main;
#[cfg(feature = "ssr")]
#[cfg(feature = "ssr")]
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    use btc_self_custody::{
        configuration::{self, get_configuration},
        run,
    };
    use sqlx::PgPool;
    use std::net::TcpListener;

    let configuration = get_configuration().expect("Failed to read config");
    let connection_pool =
        PgPool::connect(&configuration.database.connection_string())
            .await
            .expect("Failed to connect to Postgres.");
    let address = format!("127.0.0.1:{}", configuration.application_port);
    let listener = TcpListener::bind(address)?;
    run(listener, connection_pool).await?.await?;
    Ok(())
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for pure client-side testing
    // see lib.rs for hydration function instead
}
