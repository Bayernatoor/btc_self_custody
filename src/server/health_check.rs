#[cfg(feature="ssr")]
use actix_web::HttpResponse;

// simple healthcheck endpoint
#[cfg(feature="ssr")]
pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().finish()
}
