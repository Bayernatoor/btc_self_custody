//! RPC membership and notification frames collected independently from the
//! same isolated node. Regenerate with scripts/record-heartbeat-regtest.py.

use std::collections::BTreeSet;

use serde::Deserialize;

use super::shadow::decode_sequence;
use crate::stats::heartbeat_membership::HeartbeatMembership;

#[derive(Deserialize)]
struct Oracle {
    subscribed: [String; 2],
    block: Block,
    stages: Vec<Stage>,
}

#[derive(Deserialize)]
struct Block {
    hash: String,
    tx: Vec<String>,
}

#[derive(Deserialize)]
struct Stage {
    name: String,
    frames: Vec<[String; 2]>,
    snapshot: Snapshot,
}

#[derive(Deserialize)]
struct Snapshot {
    txids: Vec<String>,
    mempool_sequence: u64,
}

fn deliver(view: &mut HeartbeatMembership, frame: &[String; 2]) {
    use bitcoin::hex::FromHex;
    let [body, counter] = frame.each_ref().map(|s| Vec::from_hex(s).unwrap());
    let (transport, event) = decode_sequence(&body, &counter).unwrap();
    view.notify(transport, event);
}

fn install(view: &mut HeartbeatMembership, stage: &Stage) {
    let ticket = view.begin_checkpoint().expect("subscribed");
    assert!(view.finish_checkpoint(
        ticket,
        stage.snapshot.txids.clone(),
        stage.snapshot.mempool_sequence,
        true
    ));
}

fn matches_core(view: &HeartbeatMembership, stage: &Stage) {
    assert_eq!(
        view.members().collect::<BTreeSet<_>>(),
        stage.snapshot.txids.iter().collect(),
        "{}",
        stage.name
    );
}

#[test]
fn real_core_frames_match_rpc_sets_through_replacement_mining_and_reorg() {
    let oracle: Oracle = serde_json::from_str(include_str!(
        "../../tests/fixtures/heartbeat-core-sequence.json"
    ))
    .unwrap();
    let [empty, admission, replacement, second, mined, late, reorg] =
        oracle.stages.as_slice()
    else {
        panic!("regtest trace is missing a checkpoint");
    };
    let mut view = HeartbeatMembership::default();
    deliver(&mut view, &oracle.subscribed);
    install(&mut view, empty);
    matches_core(&view, empty);
    for stage in [admission, replacement, second] {
        for frame in &stage.frames {
            deliver(&mut view, frame);
        }
        assert!(view.complete());
        matches_core(&view, stage);
    }

    for frame in &mined.frames {
        deliver(&mut view, frame);
    }
    assert!(!view.complete());
    let inclusion = view.block_ticket(&oracle.block.hash).unwrap();
    // Arrival happens before the block lookup completes. Applying the block
    // should produce Core's *later* set, which includes this new admission.
    for frame in &late.frames {
        deliver(&mut view, frame);
    }
    assert!(view.apply_block(
        inclusion.clone(),
        &oracle.block.hash,
        &oracle.block.tx
    ));
    matches_core(&view, late);
    assert!(!view.complete());
    install(&mut view, late);
    for frame in &reorg.frames {
        deliver(&mut view, frame);
    }
    assert!(!view.apply_block(inclusion, &oracle.block.hash, &oracle.block.tx));
    matches_core(&view, reorg);
    assert!(!view.complete());
    install(&mut view, reorg);
    assert!(view.complete());
    matches_core(&view, reorg);
}
