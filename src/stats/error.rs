//! Unified error type for the stats module.
//! All errors map to HTTP 500 with a JSON body for API responses.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum StatsError {
    #[error("Bitcoin RPC error: {0}")]
    Rpc(String),

    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl IntoResponse for StatsError {
    fn into_response(self) -> Response {
        tracing::error!("{self}");
        // Return generic message to avoid leaking internal details (IPs, paths, SQL errors)
        let body = json!({ "error": "Internal server error" });
        (StatusCode::INTERNAL_SERVER_ERROR, body.to_string()).into_response()
    }
}
