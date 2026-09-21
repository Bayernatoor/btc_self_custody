#!/usr/bin/env bash
# Launch headless Chrome, drive the served app over CDP, then kill Chrome.
#
#   bash scripts/probe-served-app.sh                      # dev, port 8000
#   BASE=http://127.0.0.1:8011 bash scripts/probe-served-app.sh   # elsewhere
#
# One script because a backgrounded browser does not survive the tool call
# that started it in an agent session: bwrap runs with --die-with-parent, so
# launch, drive and exit have to happen together. A persistent browser has to
# be started by hand in tmux instead.
#
# The app must already be serving at BASE. Start it with `cargo leptos watch`
# or from a release build:
#
#   set -a; . ./.env; set +a
#   LEPTOS_SITE_ROOT=target/site LEPTOS_SITE_ADDR=127.0.0.1:8011 \
#     ./target/release/we_hodl_btc
#
# `--user-data-dir` under TMPDIR is the part that matters: the default profile
# path is outside the sandbox's writable set and Chrome aborts in
# process_singleton_posix.cc before a flag can help.
set -uo pipefail

# 8000 is the port `cargo leptos watch` serves on, per AGENTS.md.
# Override with BASE for a release build on another port.
BASE="${BASE:-http://127.0.0.1:8000}"
# A per-run port by default. Two runs on one port each kill the other's
# Chrome through the trap below, which closes the websocket mid-call.
PORT="${CDP_PORT:-$((9300 + RANDOM % 400))}"
PROFILE="${TMPDIR:-/tmp}/cdp-served-app"

if ! curl -s -o /dev/null -m 10 "$BASE/api/stats/stats"; then
  echo "nothing serving at $BASE" >&2
  echo "start it with 'cargo leptos watch', or set BASE to its port" >&2
  exit 2
fi

google-chrome --headless=new --no-sandbox --disable-gpu \
  --user-data-dir="$PROFILE-$PORT" --disable-dev-shm-usage \
  --remote-debugging-port="$PORT" about:blank \
  > "${TMPDIR:-/tmp}/chrome-served-app.log" 2>&1 &
CHROME=$!
trap 'kill $CHROME 2>/dev/null' EXIT

for _ in $(seq 1 40); do
  curl -s -m 2 "http://127.0.0.1:$PORT/json/version" >/dev/null && break
  sleep 0.25
done

BASE="$BASE" CDP="http://127.0.0.1:$PORT" node scripts/probe-served-app.mjs
