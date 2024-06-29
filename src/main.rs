#![allow(unused_imports)]
#[cfg(feature = "ssr")]
use {
    actix_web::main,
    we_hodl_btc::configuration::get_configuration,
    we_hodl_btc::run,
    we_hodl_btc::telemetry::{get_subscriber, init_subscriber},
    sqlx::PgPool,
    sqlx::postgres::PgPoolOptions,
    std::net::TcpListener,
};

#[cfg(feature = "ssr")]
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let subscriber =
        get_subscriber("wehodlbtc".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read config");
    let connection_pool = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy(&configuration.database.connection_string())
        .expect("Failed to connect to Postgres.");
    let address = format!("{}:{}", configuration.application.host, configuration.application.port);
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
