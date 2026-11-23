//! The sequence socket is the only writer of the shadow reducer. RPC work is
//! polled alongside receipt; its completion must present the original ticket.
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinSet;
use tokio::time::Instant;
use zeromq::{Socket, SocketRecv, SubSocket};

use super::{block_jobs::BlockJob, StatsError};
use super::{bytes_to_hex, observe_block, HeartbeatEvent, StatsState};
use crate::stats::heartbeat_membership::{
    BlockTicket, CheckpointTicket, HeartbeatMembership, Notification,
};
use crate::stats::rpc::{BlockTxids, MembershipCheckpoint};

// While coverage is complete, every A/R delta is verified sequentially, so a
// 5-minute periodic checkpoint suffices as a safety audit without duplicating
// the 60s legacy DB reconcile. Incomplete coverage still reconciles immediately.
const RECONCILE: Duration = Duration::from_secs(300);
const CAPACITY_BACKOFF: Duration = Duration::from_secs(60);
const POST_CONNECT_DELAY: Duration = Duration::from_secs(5);
const RETRY: Duration = Duration::from_secs(5);
const BATCH_CAP: usize = 2000;

type CheckpointResult =
    (CheckpointTicket, Result<MembershipCheckpoint, StatsError>);
type InclusionResult = (
    Arc<BlockJob>,
    BlockTicket,
    Result<Arc<BlockTxids>, StatsError>,
);

/// Core ZMQ publishes hashes in display order. Only the independent frame
/// counter and the A/R mempool counter are little endian integers.
pub(super) fn decode_sequence(
    body: &[u8],
    counter: &[u8],
) -> Option<(u32, Notification)> {
    let frame = u32::from_le_bytes(counter.try_into().ok()?);
    let id = bytes_to_hex(body.get(..32)?);
    let event = match (*body.get(32)?, &body[33..]) {
        (kind @ (b'A' | b'R'), tail) => {
            let sequence = u64::from_le_bytes(tail.try_into().ok()?);
            if sequence == 0 || sequence == u64::MAX {
                return None;
            }
            if kind == b'A' {
                Notification::Add { txid: id, sequence }
            } else {
                Notification::Remove { txid: id, sequence }
            }
        }
        (b'C', []) => Notification::Connect { hash: id },
        (b'D', []) => Notification::Disconnect { hash: id },
        _ => return None,
    };
    Some((frame, event))
}

pub(crate) fn spawn_sequence(state: Arc<StatsState>, url: String) {
    tokio::spawn(async move {
        let mut view = HeartbeatMembership::default();
        loop {
            if let Err(e) = subscribe(&state, &url, &mut view).await {
                tracing::warn!(
                    "heartbeat sequence subscription restarting: {e}"
                );
            }
            view.transport_gap();
            tokio::time::sleep(RETRY).await;
        }
    });
}

async fn subscribe(
    state: &Arc<StatsState>,
    url: &str,
    view: &mut HeartbeatMembership,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut socket = SubSocket::new();
    socket.connect(url).await?;
    socket.subscribe("sequence").await?;
    let mut seen_frame = false;
    let mut last_frame = Instant::now();
    let mut next_checkpoint = Instant::now();
    // Preflight the first request and any retry after exceeding the budget.
    let mut capacity_limited = true;
    let mut checks = JoinSet::<CheckpointResult>::new();
    let mut inclusions = JoinSet::<InclusionResult>::new();
    let mut removed = HashSet::new();
    let mut flush = tokio::time::interval(Duration::from_millis(500));
    flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        let step: Result<(), Box<dyn std::error::Error + Send + Sync>> = tokio::select! {
            message = socket.recv() => {
                match message {
                    Err(e) => Err(e.into()),
                    Ok(msg) => {
                        let frames = msg.into_vec();
                        let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = (|| {
                            let [topic, body, counter] = frames.as_slice() else { return Err("incomplete sequence frames".into()) };
                            if topic.as_ref() != b"sequence" { return Err("unexpected sequence topic".into()); }
                            let (frame, event) = decode_sequence(body, counter).ok_or("invalid sequence frames")?;
                            if !seen_frame { tracing::info!("heartbeat shadow receiving sequence"); }
                            seen_frame = true;
                            last_frame = Instant::now();
                            let hash = match &event {
                                Notification::Add { txid, .. } => { removed.remove(txid); None },
                                Notification::Remove { txid, .. } => { removed.insert(txid.clone()); None },
                                Notification::Disconnect { hash } => {
                                    super::retire_block(state, hash);
                                    tracing::info!(source = "sequence", stage = "disconnect", %hash, "heartbeat_block");
                                    None
                                },
                                Notification::Connect { hash } => Some(hash.clone()),
                            };
                            let was_complete = view.complete();
                            view.notify(frame, event);
                            if was_complete && !view.complete() {
                                tracing::info!(frame, "heartbeat shadow coverage incomplete; reconciling");
                                // Delay post-Connect full mempool reconciliation briefly so it
                                // does not pile onto getblock/getblockstats and Core's template
                                // check at t=0. Disconnects and transport gaps reconcile now.
                                next_checkpoint = if hash.is_some() {
                                    Instant::now() + POST_CONNECT_DELAY
                                } else {
                                    Instant::now()
                                };
                            }
                            if let Some(hash) = hash {
                                tracing::info!(source = "sequence", stage = "connect", %hash, "heartbeat_block");
                                if let (Some(job), Some(ticket)) = (observe_block(state, &hash, "sequence"), view.block_ticket(&hash)) {
                                    if inclusions.len() < 4 {
                                        let state = Arc::clone(state);
                                        inclusions.spawn(async move {
                                            let result = job.accepted_ids(&state.rpc).await;
                                            (job, ticket, result)
                                        });
                                    } else {
                                        tracing::warn!(%hash, "heartbeat shadow inclusion workers busy; relying on checkpoint");
                                    }
                                }
                            }
                            Ok(())
                        })();
                        result
                    }
                }
            }
            Some(joined) = checks.join_next(), if !checks.is_empty() => {
                match joined {
                    Err(e) => Err(e.into()),
                    Ok((ticket, result)) => {
                        let capacity_hit = matches!(result, Err(StatsError::CheckpointCapacity));
                        if capacity_hit {
                            capacity_limited = true;
                        } else if result.is_ok() {
                            capacity_limited = false;
                        }
                        let installed = match result {
                            Ok(cp) => view.finish_checkpoint(ticket, cp.txids, cp.mempool_sequence, true),
                            Err(e) => { view.abort_checkpoint(ticket); tracing::warn!("heartbeat checkpoint unavailable: {e}"); false }
                        };
                        tracing::info!(installed, members = view.members().len(), complete = view.complete(), "heartbeat shadow checkpoint");
                        next_checkpoint = Instant::now() + if installed {
                            RECONCILE
                        } else if capacity_hit {
                            CAPACITY_BACKOFF
                        } else {
                            RETRY
                        };
                        Ok(())
                    }
                }
            }
            Some(joined) = inclusions.join_next(), if !inclusions.is_empty() => {
                match joined {
                    Err(e) => Err(e.into()),
                    Ok((job, ticket, result)) => {
                        if let Ok(info) = result {
                            state.heartbeat_blocks.if_current(&job, || view.apply_block(ticket, &info.hash, &info.txids));
                        }
                        Ok(())
                    }
                }
            }
            _ = flush.tick() => {
                // Refresh a potentially half-open transport. Silence says nothing
                // about mining, readiness or the cause of a departure.
                flush_removals(state, &mut removed);
                if last_frame.elapsed() > Duration::from_secs(120) {
                    Err("refreshing quiet sequence subscription".into())
                } else {
                    Ok(())
                }
            }
        };
        if let Err(e) = step {
            flush_removals(state, &mut removed);
            return Err(e);
        }
        if removed.len() >= BATCH_CAP {
            flush_removals(state, &mut removed);
        }
        if seen_frame && checks.is_empty() && Instant::now() >= next_checkpoint
        {
            if let Some(ticket) = view.begin_checkpoint() {
                let state = Arc::clone(state);
                checks.spawn(async move {
                    let result =
                        state.rpc.membership_checkpoint(capacity_limited).await;
                    (ticket, result)
                });
            }
        }
    }
}

fn flush_removals(state: &StatsState, removed: &mut HashSet<String>) {
    if !removed.is_empty() {
        let _ = state.heartbeat_tx.send(HeartbeatEvent::TxRemoved {
            txids: removed.drain().collect(),
        });
    }
}
