//! Reconcile polled ancestry before dispatching block enrichment. The hash at
//! a given height can change while the best height stays equal or advances.
use std::collections::{BTreeMap, BTreeSet};

use super::{BitcoinRpc, StatsError, MAX_POLLER_BROADCAST};

type Chain = BTreeMap<u64, String>;
type AtHeight = (u64, String);
const DEPTH: u64 = 12;

#[derive(Default)]
pub(crate) struct PollChain {
    verified: Chain,
    pending: Chain,
}

impl PollChain {
    pub(super) fn empty(&self) -> bool {
        self.verified.is_empty()
    }

    pub(super) fn dispatched(&mut self, height: u64) {
        self.pending.remove(&height);
    }

    /// Each parent link belongs to the same immutable branch. Usually just
    /// one small header is needed; unchanged tips reuse the whole window.
    pub(super) async fn reconcile(
        &mut self,
        rpc: &BitcoinRpc,
        tip: AtHeight,
        db_height: u64,
        mut known: Vec<AtHeight>,
    ) -> Result<(Vec<String>, Vec<AtHeight>), StatsError> {
        let (top, tip_hash) = tip;
        let mut hash = tip_hash.clone();
        let floor = top.saturating_sub(DEPTH - 1);
        let mut current = Chain::new();
        let mut common = None;
        for height in (floor..=top).rev() {
            if self.verified.get(&height) == Some(&hash) {
                current.extend(
                    self.verified
                        .range(floor..=height)
                        .map(|(h, id)| (*h, id.clone())),
                );
                common = Some(height);
                break;
            }
            current.insert(height, hash.clone());
            if height != floor {
                hash = rpc.poll_parent(&hash, height).await?;
            }
        }
        let before = self
            .verified
            .last_key_value()
            .map_or(db_height, |(h, _)| *h);
        known.extend(
            self.verified
                .range(before.saturating_sub(DEPTH - 1)..)
                .map(|(h, id)| (*h, id.clone())),
        );
        // A jump larger than the walk window can still orphan a recent
        // published claim. Check just those heights (bounded job/DB windows).
        for (h, _) in &known {
            if *h > top || current.contains_key(h) {
                continue;
            }
            let canonical = match self
                .verified
                .get(h)
                .filter(|_| common.is_some_and(|ancestor| *h <= ancestor))
            {
                Some(id) => id.clone(),
                None => rpc.get_block_hash_fresh(*h).await?,
            };
            current.insert(*h, canonical);
        }
        let (retire, announce) = transitions(&known, &current, before);
        if !retire.is_empty()
            && rpc.get_blockchain_info_fresh().await?.bestblockhash != tip_hash
        {
            return Err(StatsError::Rpc(
                "tip changed during poll fork reconciliation".into(),
            ));
        }
        self.pending.retain(|h, hash| current.get(h) == Some(hash));
        self.pending.extend(announce);
        self.verified = current;
        Ok((
            retire,
            self.pending
                .iter()
                .map(|(h, id)| (*h, id.clone()))
                .collect(),
        ))
    }
}

/// Forget descendants first on the old branch; dispatch ancestors first on
/// the new one. A rollback to an unchanged ancestor is only a retirement.
fn transitions(
    known: &[AtHeight],
    current: &Chain,
    before: u64,
) -> (Vec<String>, Vec<AtHeight>) {
    let top = *current.last_key_value().expect("tip present").0;
    let mut retire = BTreeSet::new();
    let mut announce = Chain::new();
    for (h, hash) in known {
        if *h > top {
            retire.insert((*h, hash));
        } else if let Some(replacement) = current.get(h) {
            if replacement != hash {
                retire.insert((*h, hash));
                announce.insert(*h, replacement.clone());
            }
        }
    }
    if top > before && top - before <= MAX_POLLER_BROADCAST as u64 {
        announce.extend(
            current
                .range((before + 1)..)
                .map(|(h, id)| (*h, id.clone())),
        );
    }
    (
        retire.into_iter().rev().map(|(_, id)| id.clone()).collect(),
        announce.into_iter().collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chain(rows: &[(u64, &str)]) -> Chain {
        rows.iter().map(|(h, id)| (*h, id.to_string())).collect()
    }

    #[test]
    fn advancing_fork_retracts_zmq_only_hash_before_parent_and_child() {
        let known = vec![
            (90, "parent".into()),
            (91, "old".into()),
            (91, "old".into()),
        ];
        // Last poll knew only 90. The orphan at 91 was published by ZMQ alone.
        let (retire, announce) = transitions(
            &known,
            &chain(&[(90, "parent"), (91, "new"), (92, "child")]),
            90,
        );
        assert_eq!(retire, ["old"]);
        assert_eq!(announce, [(91, "new".into()), (92, "child".into())]);
    }

    #[test]
    fn rollback_without_replacement_does_not_reannounce_ancestor() {
        let known: Vec<_> =
            chain(&[(80, "ancestor"), (81, "first"), (82, "tip")])
                .into_iter()
                .collect();
        let (retire, announce) =
            transitions(&known, &chain(&[(80, "ancestor")]), 82);
        assert_eq!(retire, ["tip", "first"]);
        assert!(announce.is_empty());
    }

    #[test]
    fn shorter_fork_still_replaces_the_last_surviving_height() {
        let known: Vec<_> =
            chain(&[(80, "ancestor"), (81, "first"), (82, "tip")])
                .into_iter()
                .collect();
        let (retire, announce) =
            transitions(&known, &chain(&[(80, "ancestor"), (81, "other")]), 82);
        assert_eq!(retire, ["tip", "first"]);
        assert_eq!(announce, [(81, "other".into())]);
    }

    #[tokio::test]
    async fn higher_tip_walks_its_own_parents_before_retiring_the_old_fork() {
        use axum::{routing::post, Json, Router};
        use serde_json::{json, Value};
        let handler = |Json(req): Json<Value>| async move {
            let result = match (
                req["method"].as_str(),
                req["params"].get(0).and_then(Value::as_str),
            ) {
                (Some("getblockheader"), Some("child")) => {
                    json!({"hash":"child","height":92,"previousblockhash":"new"})
                }
                (Some("getblockheader"), Some("new")) => {
                    json!({"hash":"new","height":91,"previousblockhash":"parent"})
                }
                (Some("getblockchaininfo"), None) => {
                    json!({"blocks":92,"chain":"regtest","difficulty":1.0,"verificationprogress":1.0,"size_on_disk":1234,"bestblockhash":"child"})
                }
                _ => panic!("unexpected ancestry RPC {req}"),
            };
            Json(json!({"result":result,"error":null}))
        };
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/", post(handler)))
                .await
                .unwrap()
        });
        let rpc = BitcoinRpc::new(url, String::new(), String::new());
        let mut poll = PollChain {
            verified: chain(&[(90, "parent"), (91, "old")]),
            ..Default::default()
        };
        let (retire, publish) = poll
            .reconcile(&rpc, (92, "child".into()), 91, vec![])
            .await
            .unwrap();
        assert_eq!(retire, ["old"]);
        assert_eq!(publish, [(91, "new".into()), (92, "child".into())]);
        poll.dispatched(91);
        let (retire, retry) = poll
            .reconcile(&rpc, (92, "child".into()), 92, vec![])
            .await
            .unwrap();
        assert!(retire.is_empty());
        assert_eq!(retry, [(92, "child".into())]);
        server.abort();
    }
}
