//! Bitcoin blockchain stats module.
//!
//! When `BITCOIN_STATS_RPC_URL` is set, this module connects to a Bitcoin Core
//! node, ingests block data into SQLite, and serves analytics via API + server functions.

pub mod charts;
pub mod server_fns;
pub mod types;

#[cfg(feature = "ssr")]
pub mod api;
#[cfg(feature = "ssr")]
pub mod cache;
#[cfg(feature = "ssr")]
pub mod classifier;
#[cfg(feature = "ssr")]
pub mod config;
#[cfg(feature = "ssr")]
pub mod db;
#[cfg(feature = "ssr")]
pub mod error;
#[cfg(feature = "ssr")]
pub mod ingest;
#[cfg(feature = "ssr")]
pub mod notable;
#[cfg(feature = "ssr")]
pub mod rpc;
#[cfg(feature = "ssr")]
pub mod rpc_cache;
#[cfg(feature = "ssr")]
pub mod startup;
#[cfg(feature = "ssr")]
pub mod zmq_subscriber;

// Integration tests covering the full Axum router + DB path. Lives in the
// lib (not under tests/) so it compiles into the existing unit-test binary
// instead of forcing a separate link step — linking the full lib dependency
// closure a second time was hitting OOM during cargo test on this machine.
// Access to pub(crate) items also avoids leaking test-only surface to the
// public API.
#[cfg(all(feature = "ssr", test))]
mod api_integration_tests;
