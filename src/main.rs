#![allow(unused_imports)]
#[cfg(feature = "ssr")]
use {
    actix_web::main,
    btc_self_custody::configuration::get_configuration,
    btc_self_custody::run,
    btc_self_custody::telemetry::{get_subscriber, init_subscriber},
    sqlx::PgPool,
    std::net::TcpListener,
};

#[cfg(feature = "ssr")]
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber =
        get_subscriber("wehodlbtc".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

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
