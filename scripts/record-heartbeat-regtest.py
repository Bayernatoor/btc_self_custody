#!/usr/bin/env python3
"""Print a fresh Heartbeat membership oracle (bitcoind, bitcoin-cli and pyzmq).

Only an ephemeral, offline regtest node is used. The RPC snapshots are the
expected sets; the sequence frames are independent inputs to the Rust test.
"""

import json
from pathlib import Path
import socket
import subprocess
import tempfile

import zmq


def record(directory):
    with socket.socket() as port, socket.socket() as zmq_port:
        port.bind(("127.0.0.1", 0))
        zmq_port.bind(("127.0.0.1", 0))
        rpc_port = port.getsockname()[1]
        endpoint = f"tcp://127.0.0.1:{zmq_port.getsockname()[1]}"
    options = ["-regtest", f"-datadir={directory}", "-conf=/dev/null", f"-rpcport={rpc_port}"]
    node = subprocess.Popen(["bitcoind", *options, "-networkactive=0", "-listen=0",
                             "-fallbackfee=0.0001", "-printtoconsole=0", f"-zmqpubsequence={endpoint}"])

    def cli(*args):
        return subprocess.check_output(["bitcoin-cli", *options, "-rpcwait", "-rpcwaittimeout=15",
                                        *map(str, args)], text=True, timeout=25).strip()

    try:
        with zmq.Context() as context, context.socket(zmq.SUB) as sub:
            sub.setsockopt(zmq.SUBSCRIBE, b"sequence")
            sub.connect(endpoint)
            version = json.loads(cli("getnetworkinfo"))["subversion"]
            cli("createwallet", "heartbeat-fixture")
            address = cli("getnewaddress")
            tip = json.loads(cli("generatetoaddress", 105, address))[-1]

            def frames():
                cli("syncwithvalidationinterfacequeue")
                result = []
                while sub.poll(150):
                    topic, body, counter = sub.recv_multipart()
                    assert topic == b"sequence" and len(counter) == 4
                    result.append([body.hex(), counter.hex()])
                return result

            # Seeing the last warm-up notification proves the subscription is ready.
            warmup = frames()
            assert warmup and warmup[-1][0] == tip + "43", "regtest subscription not ready"
            assert json.loads(cli("getmempoolinfo"))["loaded"]
            stages = []

            def checkpoint(name):
                observed = frames()
                snapshot = json.loads(cli("getrawmempool", "false", "true"))
                stages.append(dict(name=name, frames=observed, snapshot=snapshot))
                return set(snapshot["txids"])

            assert checkpoint("empty") == set()
            first = cli("sendtoaddress", address, 1)
            assert checkpoint("admission") == {first}
            replacement = json.loads(cli("bumpfee", first, '{"fee_rate":50}'))["txid"]
            assert checkpoint("replacement") == {replacement}
            sibling = cli("sendtoaddress", address, 2)
            assert checkpoint("second admission") == {replacement, sibling}
            mined = json.loads(cli("generateblock", address, json.dumps([replacement])))["hash"]
            block = json.loads(cli("getblock", mined, 1))
            assert checkpoint("one mined") == {sibling}
            later = cli("sendtoaddress", address, 3)
            assert checkpoint("admission before inclusion lookup finishes") == {later, sibling}
            cli("invalidateblock", mined)
            # A descendant may leave if its coinbase input is no longer mature.
            # Core's checkpoint, not the pre-disconnect set, is the oracle.
            assert replacement in checkpoint("disconnect and re-admission")
            assert all(stage["frames"] for stage in stages[1:])
            return dict(core=version, subscribed=warmup[-1],
                        block=dict(hash=mined, tx=block["tx"]), stages=stages)
    finally:
        node.terminate()
        node.wait(timeout=20)


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="hb-regtest-") as path:
        print(json.dumps(record(Path(path)), indent=2))
