#!/usr/bin/env python3
"""Exercise the built Rust server -> SSE pipeline against an ephemeral offline regtest node.

Usage:
    CARGO_PROFILE_DEV_DEBUG=0 cargo build --features ssr --bin we_hodl_btc
    python3 scripts/test-heartbeat-regtest-smoke.py
    python3 scripts/test-heartbeat-regtest-smoke.py --poll-reorg
"""

import contextlib
import json
import os
import pathlib
import queue
import secrets
import socket
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

binpath = pathlib.Path(
    os.environ.get("WE_HODL_BTC_BIN", pathlib.Path.cwd() / "target/debug/we_hodl_btc")
)
assert binpath.is_file(), f"SSR binary not found: {binpath}"

base_tmp = os.environ.get("TMPDIR") or None
poll_only = "--poll-reorg" in sys.argv

with tempfile.TemporaryDirectory(dir=base_tmp, prefix="hb-smoke-") as tmp, contextlib.ExitStack() as stack:
    with contextlib.ExitStack() as ports:
        sockets = [ports.enter_context(socket.socket()) for _ in range(4)]
        for s in sockets:
            s.bind(("127.0.0.1", 0))
        rpc, zmq_port, app_port, unused = [s.getsockname()[1] for s in sockets]

    password = secrets.token_hex(16)
    endpoint = f"tcp://127.0.0.1:{zmq_port}"
    opts = [
        "-regtest",
        "-conf=/dev/null",
        f"-datadir={tmp}",
        f"-rpcport={rpc}",
        "-rpcuser=smoke",
        f"-rpcpassword={password}",
    ]

    def start(cmd, **kw):
        proc = subprocess.Popen(cmd, **kw)

        def stop():
            proc.terminate()
            try:
                proc.wait(8)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()

        stack.callback(stop)
        return proc

    def cli(*args):
        return subprocess.check_output(
            ["bitcoin-cli", *opts, "-rpcwait", "-rpcwaittimeout=12", *map(str, args)],
            text=True,
            timeout=20,
        ).strip()

    start(
        [
            "bitcoind",
            *opts,
            "-networkactive=0",
            "-listen=0",
            "-fallbackfee=0.0001",
            "-printtoconsole=0",
            f"-zmqpubsequence={endpoint}",
            f"-zmqpubrawtx={endpoint}",
        ]
    )
    cli("createwallet", "smoke")
    addr = cli("getnewaddress")
    cli("generatetoaddress", 105, addr)

    base = f"http://127.0.0.1:{app_port}"
    env = dict(
        os.environ,
        BITCOIN_STATS_RPC_URL=f"http://127.0.0.1:{rpc}",
        BITCOIN_STATS_RPC_USER="smoke",
        BITCOIN_STATS_RPC_PASSWORD=password,
        BITCOIN_STATS_ZMQ_TX=endpoint,
        BITCOIN_STATS_ZMQ_SEQUENCE=endpoint,
        BITCOIN_STATS_ZMQ_BLOCK=f"tcp://127.0.0.1:{unused}",
        BITCOIN_STATS_INITIAL_INGEST="3",
        BITCOIN_STATS_ALLOW_EMPTY_INGEST="1",
        BITCOIN_STATS_DB_PATH=f"{tmp}/stats.db",
        LEPTOS_SITE_ADDR=f"127.0.0.1:{app_port}",
        LEPTOS_OUTPUT_NAME="we_hodl_btc",
        RUST_LOG="info",
        NO_PROXY="127.0.0.1,localhost",
        HTTPS_PROXY="http://127.0.0.1:1",
        HTTP_PROXY="http://127.0.0.1:1",
    )
    if poll_only:
        env.pop("BITCOIN_STATS_ZMQ_SEQUENCE")

    log_path = pathlib.Path(tmp) / f"server-{int(poll_only)}.log"
    log = stack.enter_context(open(log_path, "w"))
    server = start([binpath], env=env, stdout=log, stderr=subprocess.STDOUT)

    deadline = time.monotonic() + 60
    while True:
        try:
            with urllib.request.urlopen(base + "/api/stats/stats", timeout=3) as resp:
                assert resp.status == 200
                break
        except OSError:
            if server.poll() is not None or time.monotonic() > deadline:
                raise
            time.sleep(0.15)

    events = queue.Queue()

    def read_sse():
        try:
            with urllib.request.urlopen(base + "/api/stats/heartbeat", timeout=60) as response:
                assert response.status == 200
                label = ""
                for raw in response:
                    line = raw.decode().strip()
                    if line.startswith("event:"):
                        label = line.partition(":")[2].strip()
                    if line.startswith("data:"):
                        events.put((label, json.loads(line.partition(":")[2])))
        except Exception as exc:
            events.put(("error", repr(exc)))

    threading.Thread(target=read_sse, daemon=True).start()

    def until(predicate, seconds=50):
        limit = time.monotonic() + seconds
        while True:
            event, data = events.get(timeout=max(0, limit - time.monotonic()))
            assert event != "error", data
            if predicate(event, data):
                return

    until(lambda e, d: e == "history")
    a = cli("sendtoaddress", addr, 1)
    b = cli("sendtoaddress", addr, 2)
    seen = set()

    def arrivals(e, d):
        if e == "tx":
            seen.add(d["txid"])
        return {a, b} <= seen

    until(arrivals)

    def stored(predicate):
        limit = time.monotonic() + 10
        with contextlib.closing(sqlite3.connect(f"file:{tmp}/stats.db?mode=ro", uri=True)) as conn:
            while True:
                row = conn.execute(
                    "SELECT confirmed_height FROM mempool_txs WHERE txid = ?", (a,)
                ).fetchone()
                if predicate(row):
                    return
                if time.monotonic() >= limit:
                    raise AssertionError(("wrong persisted confirmation", row))
                time.sleep(0.05)

    stored(lambda row: row == (None,))
    mined = json.loads(cli("generateblock", addr, json.dumps([a])))["hash"]
    expected = json.loads(cli("getblock", mined, 1))
    kinds = set()

    def inclusion(e, d):
        if e in {"block", "block_txids"} and d.get("hash") == mined:
            assert d["height"] == expected["height"]
            if e == "block_txids":
                assert set(expected["tx"]) == set(d["txids"]) and b not in d["txids"]
            if e == "block":
                assert d["size"] == expected["size"] and d["weight"] == expected["weight"]
            kinds.add(e)
        return len(kinds) == 2

    until(inclusion)
    assert json.loads(cli("getrawmempool")) == [b]
    stored(lambda row: row == (expected["height"],))

    cli("invalidateblock", mined)
    if poll_only:
        # Both the replacement and its child exist before the next poll.
        new_hashes = [json.loads(cli("generateblock", addr, "[]"))["hash"] for _ in range(2)]
        seen_events = []

        def changes(e, d):
            if e in {"block_disconnected", "block", "block_txids"}:
                seen_events.append((e, d["hash"]))
            return ("block_disconnected", mined) in seen_events and all(
                ("block", h) in seen_events and ("block_txids", h) in seen_events
                for h in new_hashes
            )

        try:
            until(changes, 45)
        except queue.Empty:
            raise AssertionError(f"missing poll fork events: {seen_events}") from None
        print(
            "Hashblock and sequence disabled: poll retracted old tip and delivered "
            "both canonical blocks and exact IDs across forward reorg."
        )
    else:
        until(lambda e, d: e == "block_disconnected" and d["hash"] == mined)

    assert a in json.loads(cli("getrawmempool"))
    stored(lambda row: row is None or row == (None,))
    print(
        "HTTP 200, 2 real SSE txs, exact block hash/height/txids/whole size/weight "
        "verified without hashblock; other tx stayed in Core."
    )
