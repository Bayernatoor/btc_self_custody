//! Heartbeat's observed membership, independent of metadata and rendering.
//!
//! Bitcoin Core v29.3 takes the IDs and `GetSequence()` under one pool lock
//! ([RPC source](https://github.com/bitcoin/bitcoin/blob/v29.3/src/rpc/mempool.cpp#L361-L377)).
//! A/R carry the value **before** the increment
//! ([counter](https://github.com/bitcoin/bitcoin/blob/v29.3/src/txmempool.h#L700-L707),
//! [admission](https://github.com/bitcoin/bitcoin/blob/v29.3/src/validation.cpp#L1407),
//! [removal](https://github.com/bitcoin/bitcoin/blob/v29.3/src/txmempool.cpp#L520-L532)).
//! Thus an event numbered exactly `snapshot.mempool_sequence` must be applied.
//! Core exercises this fence in `test/functional/interface_zmq.py::test_mempool_sync`.
//! Block inclusions also increment the counter but emit no R. C/D have no
//! mempool sequence; neither their transport counters nor a block's tx count
//! can repair this fence. Get another checkpoint instead.

use std::collections::{HashMap, VecDeque};

/// Caps bound both the live table and its temporary replacement during replay.
/// Exceeding a cap loses coverage; a prefix is never a complete set.
pub const MAX_MEMBERS: usize = 500_000;
const MAX_BUFFERED: usize = 16_384;
const MAX_BLOCKS: usize = 144;

#[derive(Debug)]
pub enum Notification {
    Add { txid: String, sequence: u64 },
    Remove { txid: String, sequence: u64 },
    Connect { hash: String },
    Disconnect { hash: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckpointTicket(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockTicket {
    hash: String,
    connect: u64,
}

#[derive(Debug, Clone)]
struct Delta {
    txid: String,
    sequence: u64,
    observed: u64,
    add: bool,
}

type Members = HashMap<String, u64>;

impl Delta {
    /// Apply the observation even to a partial view. False means the delta is
    /// inconsistent with a *complete* starting set or cannot fit in the cap.
    fn apply(self, members: &mut Members) -> bool {
        if self.add {
            if members.len() >= MAX_MEMBERS && !members.contains_key(&self.txid)
            {
                return false;
            }
            members.insert(self.txid, self.observed).is_none()
        } else {
            members.remove(&self.txid).is_some()
        }
    }
}

#[derive(Debug)]
struct Pending {
    ticket: CheckpointTicket,
    minimum_next: u64,
    deltas: Vec<Delta>,
}

/// One instance per ordered sequence source. Member stamps and tickets are
/// process-local observation order, not Core or transport sequence numbers.
#[derive(Default, Debug)]
pub struct HeartbeatMembership {
    members: Members,
    clock: u64,
    last_transport: Option<u32>,
    last_sequence: Option<u64>,
    /// Skip older, still-in-flight notifications already reflected in the RPC.
    snapshot_floor: u64,
    /// Some only while every delta after the installed snapshot is accounted for.
    expected: Option<u64>,
    pending: Option<Pending>,
    accepted: VecDeque<BlockTicket>,
    /// Members retained across a disconnect are last-known, not proven members
    /// of a subsequent block connect in the new observation epoch.
    epoch_start: u64,
}

fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl HeartbeatMembership {
    pub fn complete(&self) -> bool {
        self.expected.is_some()
    }

    pub fn members(&self) -> impl ExactSizeIterator<Item = &String> {
        self.members.keys()
    }

    fn tick(&mut self) -> u64 {
        self.clock = self
            .clock
            .checked_add(1)
            .expect("observation clock exhausted");
        self.clock
    }

    fn incomplete(&mut self) {
        self.expected = None;
        self.pending = None;
    }

    /// Explicit socket failure, malformed message, or inferred transport loss.
    pub fn transport_gap(&mut self) {
        self.incomplete();
        self.accepted.clear();
        self.epoch_start = self.tick();
        self.last_transport = None;
        self.last_sequence = None;
        self.snapshot_floor = 0;
    }

    /// Apply one ordered sequence notification. Returns `true` when a valid
    /// `Connect`/`Disconnect` or a post-fence (`sequence >= snapshot_floor`)
    /// `Add`/`Remove` delta was applied, and `false` when the notification was
    /// ignored as pre-snapshot backlog or rejected as malformed.
    pub fn notify(&mut self, transport: u32, event: Notification) -> bool {
        if self
            .last_transport
            .is_some_and(|n| n.wrapping_add(1) != transport)
        {
            self.transport_gap();
        }
        self.last_transport = Some(transport);
        use Notification::*;
        let (txid, sequence, add) = match event {
            Add { txid, sequence } => (txid, sequence, true),
            Remove { txid, sequence } => (txid, sequence, false),
            Connect { hash } if valid_id(&hash) => {
                self.incomplete();
                // Core's C/D notifications carry no mempool sequence, so when a
                // checkpoint advances `snapshot_floor` past `last_sequence + 1`
                // (including via block-removal sequence increments), this C
                // cannot be fenced against the snapshot. Skipping the
                // optimistic block ticket and letting the immediate checkpoint
                // reconcile preserves safety at the cost of no fast-path
                // `apply_block` until the next A/R arrives.
                let caught_up = self.last_sequence.unwrap_or(0)
                    >= self.snapshot_floor.saturating_sub(1);
                if caught_up && self.block_ticket(&hash).is_none() {
                    if self.accepted.len() == MAX_BLOCKS {
                        self.accepted.pop_front();
                    }
                    let connect = self.tick();
                    self.accepted.push_back(BlockTicket { hash, connect });
                }
                return true;
            }
            Disconnect { hash } if valid_id(&hash) => {
                self.incomplete();
                self.accepted.retain(|block| block.hash != hash);
                return true;
            }
            _ => {
                self.transport_gap();
                return false;
            }
        };
        if !valid_id(&txid) || sequence == 0 || sequence == u64::MAX {
            self.transport_gap();
            return false;
        }
        if self.last_sequence.is_some_and(|last| sequence <= last) {
            // A Core restart/reset is not a late pre-snapshot notification.
            self.transport_gap();
            self.last_transport = Some(transport);
        }
        self.last_sequence = Some(sequence);
        if sequence < self.snapshot_floor {
            return false;
        }
        let delta = Delta {
            txid,
            sequence,
            add,
            observed: self.tick(),
        };
        if let Some(pending) = &mut self.pending {
            if pending.deltas.len() == MAX_BUFFERED {
                self.incomplete();
            } else {
                pending.deltas.push(delta.clone());
            }
        }
        let consistent = delta.apply(&mut self.members);
        self.expected = self
            .expected
            .filter(|n| *n == sequence && consistent)
            .map(|_| sequence + 1);
        true
    }

    /// Start **after** at least one valid sequence message has proved that
    /// the subscription is receiving, **before** both readiness and IDs RPCs.
    /// Returns `None` if no transport frame has been received since startup or
    /// the last `transport_gap()`, so callers do not issue doomed RPCs.
    pub fn begin_checkpoint(&mut self) -> Option<CheckpointTicket> {
        self.last_transport?;
        let ticket = CheckpointTicket(self.tick());
        self.pending = Some(Pending {
            ticket,
            minimum_next: self
                .snapshot_floor
                .max(self.last_sequence.unwrap_or(0) + 1),
            deltas: Vec::new(),
        });
        Some(ticket)
    }

    pub fn abort_checkpoint(&mut self, ticket: CheckpointTicket) {
        if self.pending.as_ref().is_some_and(|p| p.ticket == ticket) {
            self.incomplete();
        }
    }

    /// Atomically replace from *all* IDs, replaying contiguous post-fence
    /// notifications. `loaded` is a fresh `getmempoolinfo.loaded` reading
    /// from the same Core instance: initial load attempted, not source age.
    pub fn finish_checkpoint(
        &mut self,
        ticket: CheckpointTicket,
        ids: Vec<String>,
        next: u64,
        loaded: bool,
    ) -> bool {
        if self.pending.as_ref().is_none_or(|p| p.ticket != ticket) {
            return false;
        }
        let pending = self.pending.take().unwrap();
        self.expected = None;
        if !loaded || ids.len() > MAX_MEMBERS {
            return false;
        }
        if next < pending.minimum_next {
            // A snapshot requested after observing N cannot report next <= N
            // from the same instance. Do not strand the next attempt behind
            // the old (possibly much higher) floor if Core restarted.
            self.transport_gap();
            return false;
        }
        let mut members = Members::with_capacity(ids.len());
        for id in ids {
            if !valid_id(&id) || members.insert(id, ticket.0).is_some() {
                return false;
            }
        }
        let mut expected = next;
        for delta in pending.deltas.into_iter().filter(|d| d.sequence >= next) {
            if delta.sequence != expected || !delta.apply(&mut members) {
                return false;
            }
            expected += 1;
        }
        self.members = members;
        self.snapshot_floor = next;
        self.expected = Some(expected);
        true
    }

    /// Only ordered, comparable C observations authorize changes to members.
    /// Older work beyond the retained accepted-hash window expires safely.
    pub fn block_ticket(&self, hash: &str) -> Option<BlockTicket> {
        self.accepted.iter().find(|b| b.hash == hash).cloned()
    }

    /// The source must obtain the exact inclusion list for `returned_hash`.
    /// True means this job still matches the accepted connection. Only the
    /// intersection with membership observed before that C can be removed.
    pub fn apply_block(
        &mut self,
        ticket: BlockTicket,
        returned_hash: &str,
        ids: &[String],
    ) -> bool {
        if returned_hash != ticket.hash || !self.accepted.contains(&ticket) {
            return false;
        }
        for id in ids {
            if self
                .members
                .get(id)
                .is_some_and(|n| self.epoch_start < *n && *n < ticket.connect)
            {
                self.members.remove(id);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn id(n: u64) -> String {
        format!("{n:064x}")
    }

    fn ids(numbers: &[u64]) -> Vec<String> {
        numbers.iter().copied().map(id).collect()
    }

    fn tx(n: u64, sequence: u64, add: bool) -> Notification {
        let txid = id(n);
        if add {
            Notification::Add { txid, sequence }
        } else {
            Notification::Remove { txid, sequence }
        }
    }

    fn connect(n: u64) -> Notification {
        Notification::Connect { hash: id(n) }
    }

    fn disconnect(n: u64) -> Notification {
        Notification::Disconnect { hash: id(n) }
    }

    fn assert_members(view: &HeartbeatMembership, expected: &[u64]) {
        assert_eq!(
            view.members().cloned().collect::<BTreeSet<_>>(),
            ids(expected).into_iter().collect()
        );
    }

    fn checkpoint(view: &mut HeartbeatMembership, numbers: &[u64], next: u64) {
        let ticket = view.begin_checkpoint().expect("transport active");
        assert!(view.finish_checkpoint(ticket, ids(numbers), next, true));
    }

    #[test]
    fn all_snapshot_positions_and_delivery_delays_match_the_reference_sets() {
        // Replace, re-admit, and remove. The independently enumerated Core
        // sets define the RPC result at each cut, including the empty set.
        let ops = [
            (3, true),
            (2, false),
            (4, true),
            (3, false),
            (2, true),
            (4, false),
            (2, false),
        ];
        let sets: &[&[u64]] =
            &[&[2], &[2, 3], &[3], &[3, 4], &[4], &[2, 4], &[2], &[]];
        let first = (1u64 << 54) + 1;
        let frame = u32::MAX - 2;
        for (cut, snapshot) in sets.iter().enumerate() {
            for delivered in 0..=ops.len() {
                let mut view = HeartbeatMembership::default();
                view.notify(frame, tx(2, first - 1, true));
                let ticket = view.begin_checkpoint().unwrap();
                let receive = |view: &mut HeartbeatMembership, i: usize| {
                    let (n, add) = ops[i];
                    view.notify(
                        frame.wrapping_add(i as u32 + 1),
                        tx(n, first + i as u64, add),
                    );
                };
                for i in 0..delivered {
                    receive(&mut view, i);
                }
                assert!(view.finish_checkpoint(
                    ticket,
                    ids(snapshot),
                    first + cut as u64,
                    true
                ));
                assert_members(&view, sets[delivered.max(cut)]);
                for i in delivered..ops.len() {
                    receive(&mut view, i);
                    assert!(view.complete());
                    assert_members(&view, sets[(i + 1).max(cut)]);
                }
            }
        }
    }

    #[test]
    fn authoritative_equal_counts_and_empty_require_a_ready_subscribed_source()
    {
        let mut view = HeartbeatMembership::default();
        assert!(view.begin_checkpoint().is_none());
        view.notify(4, tx(10, 90, true));
        checkpoint(&mut view, &[10, 11], 92);
        checkpoint(&mut view, &[12, 13], 96);
        assert_members(&view, &[12, 13]);
        let ticket = view.begin_checkpoint().unwrap();
        assert!(!view.finish_checkpoint(ticket, vec![], 98, false));
        assert!(!view.complete());
        assert_members(&view, &[12, 13]);
        checkpoint(&mut view, &[], 98);
        assert!(view.complete());
        assert_members(&view, &[]);
    }

    #[test]
    fn lost_transport_and_unsequenced_chain_events_invalidate_inflight_snapshots(
    ) {
        for (frame, event) in
            [(52, tx(2, 11, true)), (51, connect(2)), (51, disconnect(2))]
        {
            let mut view = HeartbeatMembership::default();
            view.notify(50, tx(1, 10, true));
            checkpoint(&mut view, &[1], 11);
            let ticket = view.begin_checkpoint().unwrap();
            view.notify(frame, event);
            assert!(!view.complete());
            assert!(!view.finish_checkpoint(ticket, vec![], 15, true));
            checkpoint(&mut view, &[3], 16);
            assert_members(&view, &[3]);
        }
    }

    #[test]
    fn only_latest_rpc_can_install_and_reset_cannot_be_mistaken_for_backlog() {
        let mut view = HeartbeatMembership::default();
        view.notify(10, tx(1, 100, true));
        let obsolete = view.begin_checkpoint().unwrap();
        let current = view.begin_checkpoint().unwrap();
        view.abort_checkpoint(obsolete);
        assert!(view.finish_checkpoint(current, ids(&[1, 2]), 102, true));
        assert!(!view.finish_checkpoint(obsolete, vec![], 101, true));
        // Regression even with a coincidentally consecutive transport counter.
        view.notify(11, tx(3, 1, true));
        assert!(!view.complete());
        checkpoint(&mut view, &[3], 2);
        assert_members(&view, &[3]);
        let failed = view.begin_checkpoint().unwrap();
        view.abort_checkpoint(failed);
        assert!(!view.complete());
        assert!(!view.finish_checkpoint(failed, vec![], 3, true));
    }

    #[test]
    fn skipped_core_numbers_and_contradictions_never_establish_coverage() {
        for (n, seq, add) in [(3, 12, true), (1, 11, true), (2, 11, false)] {
            let mut view = HeartbeatMembership::default();
            view.notify(10, tx(1, 10, true));
            checkpoint(&mut view, &[1], 11);
            let ticket = view.begin_checkpoint().unwrap();
            view.notify(11, tx(n, seq, add));
            assert!(!view.complete());
            assert!(!view.finish_checkpoint(ticket, ids(&[1]), 11, true));
        }
        for (bad_ids, bad_next) in [
            (ids(&[1]), 10),
            (ids(&[1, 1]), 12),
            (vec!["wrong".into()], 12),
            (vec![String::new(); MAX_MEMBERS + 1], 12),
        ] {
            let mut view = HeartbeatMembership::default();
            view.notify(1, tx(1, 10, true));
            let ticket = view.begin_checkpoint().unwrap();
            assert!(!view.finish_checkpoint(ticket, bad_ids, bad_next, true));
            assert_members(&view, &[1]);
        }
    }

    #[test]
    fn lower_rpc_fence_expires_old_jobs_and_can_recover_on_a_new_observation() {
        let mut view = HeartbeatMembership::default();
        view.notify(1, tx(1, 99, true));
        view.notify(2, connect(9));
        let job = view.block_ticket(&id(9)).unwrap();
        checkpoint(&mut view, &[1], 1_000);
        let ticket = view.begin_checkpoint().unwrap();
        assert!(!view.finish_checkpoint(ticket, ids(&[2]), 201, true));
        assert!(!view.apply_block(job, &id(9), &ids(&[1])));
        // A regressive snapshot clears transport state so callers wait for a
        // fresh ZMQ frame instead of spinning doomed checkpoint RPCs.
        assert!(view.begin_checkpoint().is_none());
        // Higher than the last observed A/R but below the previous RPC floor.
        view.notify(3, tx(3, 201, true));
        checkpoint(&mut view, &[2, 3], 202);
        assert_members(&view, &[2, 3]);
    }

    #[test]
    fn delayed_exact_block_is_hash_bound_and_only_changes_preconnect_members() {
        let mut view = HeartbeatMembership::default();
        view.notify(1, tx(4, 11, true));
        checkpoint(&mut view, &[1, 2, 3, 4], 12);
        view.notify(2, connect(20));
        let ticket = view.block_ticket(&id(20)).unwrap();
        assert!(!view.complete());
        view.notify(3, tx(5, 15, true)); // Ordinary work is independent of the job.
        view.notify(4, tx(2, 16, false));
        view.notify(5, tx(2, 17, true));
        view.notify(6, connect(21)); // An accepted child does not orphan its parent.
        assert!(!view.apply_block(ticket.clone(), &id(21), &ids(&[3])));
        assert_members(&view, &[1, 2, 3, 4, 5]);
        let block_ids = ids(&[100, 1, 2, 4]); // includes unobserved coinbase
        assert!(view.apply_block(ticket, &id(20), &block_ids));
        assert_members(&view, &[2, 3, 5]);
        assert!(!view.complete());
        let child = view.block_ticket(&id(21)).unwrap();
        // A more recent verified RPC presence also supersedes a block job.
        checkpoint(&mut view, &[2, 3, 5], 18);
        assert!(view.apply_block(child, &id(21), &ids(&[2, 3])));
        assert_members(&view, &[2, 3, 5]);
        assert!(view.complete());
    }

    #[test]
    fn disconnect_and_reconnect_same_hash_cannot_revalidate_an_old_job() {
        let mut view = HeartbeatMembership::default();
        view.notify(1, tx(1, 10, true));
        view.notify(2, connect(5));
        let first = view.block_ticket(&id(5)).unwrap();
        assert!(view.apply_block(first.clone(), &id(5), &ids(&[1])));
        view.notify(3, disconnect(5));
        assert_members(&view, &[]); // Only A or a checkpoint can re-admit it.
        view.notify(4, tx(1, 12, true));
        view.notify(5, connect(5));
        assert!(!view.apply_block(first, &id(5), &ids(&[1])));
        let second = view.block_ticket(&id(5)).unwrap();
        assert_members(&view, &[1]);
        view.transport_gap();
        assert!(!view.apply_block(second, &id(5), &ids(&[1])));
        assert!(!view.complete());
    }

    #[test]
    fn unfenced_connect_in_rpc_backlog_cannot_evict_snapshot_readmission() {
        let mut view = HeartbeatMembership::default();
        view.notify(7, tx(2, 10, true));
        // Core has already connected, disconnected and readmitted tx 2 before
        // answering. Those notifications are still in transport order below.
        checkpoint(&mut view, &[2], 13);
        view.notify(8, connect(10));
        assert!(view.block_ticket(&id(10)).is_none());
        view.notify(9, disconnect(10));
        view.notify(10, tx(2, 12, true));
        assert_members(&view, &[2]);
        view.notify(11, connect(11));
        let current = view.block_ticket(&id(11)).unwrap();
        assert!(view.apply_block(current, &id(11), &ids(&[2])));
        assert_members(&view, &[]);
    }

    #[test]
    fn slow_checkpoint_and_block_jobs_cannot_accumulate_an_unbounded_backlog() {
        let mut view = HeartbeatMembership::default();
        view.notify(0, connect(90));
        checkpoint(&mut view, &[], 1);
        let pending = view.begin_checkpoint().unwrap();
        for i in 1..=MAX_BUFFERED + 1 {
            view.notify(i as u32, tx(1, i as u64, i % 2 == 1));
        }
        assert!(!view.complete());
        assert!(!view.finish_checkpoint(pending, vec![], 1, true));
        assert_members(&view, &[1]);
        let old = view.block_ticket(&id(90)).unwrap();
        for i in 0..MAX_BLOCKS {
            view.notify((MAX_BUFFERED + 2 + i) as u32, connect(100 + i as u64));
        }
        assert!(!view.apply_block(old, &id(90), &ids(&[1])));
    }

    #[test]
    fn notify_returns_false_for_pre_snapshot_backlog_and_true_at_or_above_floor(
    ) {
        let mut view = HeartbeatMembership::default();
        assert!(view.notify(1, tx(1, 10, true)));
        checkpoint(&mut view, &[1, 2], 15);

        // Pre-snapshot backlog (sequence < snapshot_floor = 15) returns false
        // and does not mutate membership.
        assert!(!view.notify(2, tx(3, 12, true)));
        assert!(!view.notify(3, tx(1, 14, false)));
        assert_members(&view, &[1, 2]);
        assert!(view.complete());

        // At or above snapshot_floor (sequence >= 15) returns true and applies.
        assert!(view.notify(4, tx(3, 15, true)));
        assert!(view.notify(5, tx(2, 16, false)));
        assert_members(&view, &[1, 3]);
        assert!(view.complete());

        // Invalid payload returns false.
        assert!(!view.notify(
            6,
            Notification::Add {
                txid: "bad".into(),
                sequence: 17,
            },
        ));
    }
}
