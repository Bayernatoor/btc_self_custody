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
pub mod rpc;
#[cfg(feature = "ssr")]
pub mod startup;
#[cfg(feature = "ssr")]
pub mod zmq_subscriber;
