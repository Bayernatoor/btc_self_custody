pub mod app;
pub mod configuration;
pub mod extras;
pub mod helpers;
pub mod routes;
pub mod server;
#[cfg(feature = "ssr")]
use actix_web::dev::Server;
use cfg_if::cfg_if;

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
pub async fn run() -> Result<Server, std::io::Error> {
    use actix_files::Files;
    use actix_web::*;
    use app::*;
    use leptos::*;
    use leptos_actix::{generate_route_list, LeptosRoutes};
    use server::health_check::health_check;

    let conf = get_configuration(None).await.unwrap();
    let addr = conf.leptos_options.site_addr;
    // Generate the list of routes in your Leptos App
    let routes = generate_route_list(|| view! {<App/> });
    logging::log!("Server listening on: {}", &addr);

    let server = HttpServer::new(move || {
        let leptos_options = &conf.leptos_options;
        let site_root = &leptos_options.site_root;

        App::new()
            .route("/api/{tail:.*}", leptos_actix::handle_server_fns())
            .route("/server/health_check", web::get().to(health_check))
            .leptos_routes(
                leptos_options.to_owned(),
                routes.to_owned(),
                || view! {<App/> },
            )
            .service(Files::new("/", site_root))
        //.wrap(middleware::Compress::default())
    })
    .bind(&addr)?
    .run();
    Ok(server)
}
