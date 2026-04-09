#![recursion_limit = "512"]

/// SSR server - Axum serves pre-rendered HTML + static assets.
/// The WASM client hydrates on load for client-side interactivity.
/// When BITCOIN_STATS_RPC_URL is set, the stats module activates
/// and serves blockchain analytics at /api/stats/ and /observatory.
#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::{response::Redirect, routing::get, Router};
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use tokio::net::TcpListener;
    use we_hodl_btc::app::{shell, App};

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let conf = get_configuration(None).unwrap();
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    // Try to initialize stats module (returns None if BITCOIN_STATS_RPC_URL not set)
    let stats = we_hodl_btc::stats::startup::init().await;

    let mut app = Router::new()
        // Legacy /stats redirect (301 for SEO)
        .route(
            "/stats",
            get(|| async { Redirect::permanent("/observatory") }),
        )
        .route(
            "/stats/learn/protocols",
            get(|| async {
                Redirect::permanent("/observatory/learn/protocols")
            }),
        )
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    if let Some((stats_state, stats_router, zmq_tx, zmq_block)) = stats {
        use axum::Extension;
        tracing::info!("Stats module active — API at /api/stats/");
        app = app
            .nest("/api/stats", stats_router)
            .layer(Extension(stats_state.clone()));
        we_hodl_btc::stats::startup::spawn_background_tasks(
            stats_state,
            zmq_tx,
            zmq_block,
        );
    } else {
        tracing::info!("Stats module dormant (BITCOIN_STATS_RPC_URL not set)");
    }

    // Compress all responses (gzip/brotli). ~70% size reduction on API JSON
    // and HTML. Negotiated via Accept-Encoding header automatically.
    let app = app.layer(tower_http::compression::CompressionLayer::new());

    let listener = TcpListener::bind(&addr).await.unwrap();
    tracing::info!("Server listening on http://{}", addr);
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
fn main() {}
