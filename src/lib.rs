pub mod app;
pub mod configuration;
pub mod extras;
pub mod helpers;
pub mod routes;
pub mod server;
pub mod telemetry;
use cfg_if::cfg_if;
#[cfg(feature = "ssr")]
use {
    actix_files::Files,
    actix_web::dev::Server,
    actix_web::middleware::Logger,
    actix_web::*,
    app::*,
    leptos::*,
    leptos_actix::{generate_route_list, LeptosRoutes},
    server::{
        create_post::create_post, health_check::health_check,
        subscriptions::subscribe,
    },
    sqlx::PgPool,
    std::net::TcpListener,
};

cfg_if! {
    if #[cfg(feature = "hydrate")] {

      use wasm_bindgen::prelude::wasm_bindgen;

        #[wasm_bindgen]
        pub fn hydrate() {
          use app::*;
          use leptos::*;

          // initializes logging using the `log` crate
          _ = console_log::init_with_level(log::Level::Debug);

          console_error_panic_hook::set_once();

          leptos::mount_to_body(move || {
              view! {<App/>}
          });
        }
    }
}

#[cfg(feature = "ssr")]
pub async fn run(
    listener: TcpListener,
    db_pool: PgPool,
) -> Result<Server, std::io::Error> {
    // Wrap the pool using web::Data, which boils down to an Arc smart pointer

    let db_pool = web::Data::new(db_pool);
    let conf = get_configuration(None).await.unwrap();

    //let addr = conf.leptos_options.site_addr;
    // Generate the list of routes in your Leptos App
    let routes = generate_route_list(|| view! { <App/> });
    logging::log!("Server listening on: {:?}", listener);

    let server = HttpServer::new(move || {
        let leptos_options = &conf.leptos_options;
        let site_root = &leptos_options.site_root;

        App::new()
            // middlewares are added using the `wrap` method on `app`
            .wrap(Logger::default())
            .route("/api/{tail:.*}", leptos_actix::handle_server_fns())
            .route("/server/health_check", web::get().to(health_check))
            .route("/server/create_post", web::post().to(create_post))
            .route("/server/subscriptions", web::post().to(subscribe))
            .app_data(db_pool.clone())
            .leptos_routes(
                leptos_options.to_owned(),
                routes.to_owned(),
                || view! { <App/> },
            )
            .service(Files::new("/", site_root))
    })
    .listen(listener)?
    .run();
    Ok(server)
}
