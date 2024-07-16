#[cfg(feature = "ssr")]
use actix_web::{HttpResponse, Error};

// simple healthcheck endpoint
#[cfg(feature = "ssr")]
pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().finish()
}

/// Allows nostr clients to access my nostr pubkey
#[cfg(feature = "ssr")]
pub async fn nostr_json() -> Result<HttpResponse, Error> {
    match std::fs::read_to_string(".well-known/nostr.json") {
        Ok(file_content) => {
            Ok(HttpResponse::Ok()
               .content_type("application/json")
               .header("Access-Control-Allow-Origin", "*")
               .body(file_content))
        }
        Err(e) => {
            Ok(HttpResponse::BadRequest()
               .content_type("text/plain; charset=utf-8")
               .body(format!("Failed to read nostr.json file: {}", e)))
        }
    }
}
