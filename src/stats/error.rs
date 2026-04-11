//! Unified error type for the stats module.
//!
//! All errors - regardless of source (RPC, database, HTTP, config, IO) - are
//! mapped to HTTP 500 with a generic JSON error body. Internal details (IP
//! addresses, file paths, SQL errors) are logged server-side but never exposed
//! to clients. This prevents information leakage while still providing useful
//! debug information in server logs.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Unified error enum covering all failure modes in the stats module.
/// Implements `IntoResponse` so it can be returned directly from Axum handlers.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum StatsError {
    /// Bitcoin Core JSON-RPC call failed or returned an error.
    #[error("Bitcoin RPC error: {0}")]
    Rpc(String),

    /// SQLite query or connection error.
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    /// External HTTP request failed (e.g. price API).
    #[error("HTTP client error: {0}")]
    Http(#[from] reqwest::Error),

    /// Missing or invalid configuration.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Filesystem or other IO error.
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
