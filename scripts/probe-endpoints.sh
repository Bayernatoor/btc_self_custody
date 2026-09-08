#!/usr/bin/env bash
# Hit every endpoint against a running dev server and report status + shape.
#
# Exists because a range cap shipped returning 500 for a client error and four
# green CI checks did not notice: nothing in the suite asserted a status code.
# This probes the responses themselves, on real data.
#
#   cargo leptos watch          # in another terminal
#   bash scripts/probe-endpoints.sh
#
# Reads nothing but GET/POST; makes no writes. Safe to run against prod by
# setting BASE, though /cache-stats and /live will reflect that node.

BASE="${BASE:-http://127.0.0.1:8000}"
# Ask the server for its own tip rather than hardcoding one, so the ranges
# probed stay current as the chain advances. Falls back to a fixed height if
# the server is down, which the first probe below will report anyway.
if [ -z "${TIP:-}" ]; then
  TIP=$(curl -s -m 10 "${BASE}/api/stats/stats" 2>/dev/null \
        | grep -o '"max_height":[0-9]*' | head -1 | cut -d: -f2)
fi
TIP="${TIP:-966043}"

pass=0; fail=0; suspect=0

# probe <expected-status> <label> <curl-args...>
probe() {
  local want="$1" label="$2"; shift 2
  local out code body
  out=$(curl -s -m 25 -w $'\n%{http_code}' "$@" 2>/dev/null)
  code="${out##*$'\n'}"; body="${out%$'\n'*}"
  local mark
  if [ "$code" = "$want" ]; then mark="ok  "; pass=$((pass+1))
  else mark="BAD "; fail=$((fail+1)); fi
  printf '%s %-3s (want %-3s) %-34s %s\n' "$mark" "$code" "$want" "$label" "$(printf '%s' "$body" | head -c 90 | tr -d '\n')"
}

# note <label> <curl-args...>: status recorded, not asserted
note() {
  local label="$1"; shift
  local out code body
  out=$(curl -s -m 25 -w $'\n%{http_code}' "$@" 2>/dev/null)
  code="${out##*$'\n'}"; body="${out%$'\n'*}"
  printf '??  %-3s %-45s %s\n' "$code" "$label" "$(printf '%s' "$body" | head -c 90 | tr -d '\n')"
  suspect=$((suspect+1))
}

sfn() { local ep="$1"; shift; printf '%s' "$BASE/api/$ep"; }

echo "== axum GET routes, happy path =="
probe 200 "/stats"                 "$BASE/api/stats/stats"
probe 200 "/cache-stats"           "$BASE/api/stats/cache-stats"
probe 200 "/blocks (default 144)"  "$BASE/api/stats/blocks"
probe 200 "/blocks?range"          "$BASE/api/stats/blocks?from=$((TIP-10))&to=$TIP"
probe 200 "/blocks/{height}"       "$BASE/api/stats/blocks/840000"
probe 200 "/op-returns narrow"     "$BASE/api/stats/op-returns?from=$((TIP-5000))&to=$TIP"
probe 200 "/aggregates/daily"      "$BASE/api/stats/aggregates/daily?from=1700000000&to=1710000000"
probe 200 "/signaling"             "$BASE/api/stats/signaling?bit=2"
probe 200 "/signaling/periods"     "$BASE/api/stats/signaling/periods?bit=2"
probe 200 "/live"                  "$BASE/api/stats/live"

echo
echo "== axum error paths =="
probe 400 "/op-returns over-wide"  "$BASE/api/stats/op-returns?from=0&to=$TIP"
probe 400 "/daily reversed range"  "$BASE/api/stats/aggregates/daily?from=1710000000&to=1700000000"
probe 400 "/tx bad txid"           "$BASE/api/stats/tx/nothex"
probe 404 "/tx unknown txid"       "$BASE/api/stats/tx/$(printf '0%.0s' {1..64})"
# These three were the open questions this script was written to answer, and
# they are now settled, so they are assertions rather than notes. A recorded
# status nobody compares against anything is not a check.
probe 404 "/blocks/{height} absent"    "$BASE/api/stats/blocks/99999999"
probe 400 "/signaling bit=99"          "$BASE/api/stats/signaling?bit=99"
probe 400 "/signaling/periods bit=99"  "$BASE/api/stats/signaling/periods?bit=99"
probe 200 "/signaling bit=28 (max ok)" "$BASE/api/stats/signaling?bit=28"
note      "/blocks reversed range"         "$BASE/api/stats/blocks?from=$TIP&to=100"

echo
echo "== server fns, happy path (PostUrl encoding, not JSON) =="
probe 200 "stats_summary"          -X POST "$(sfn stats_summary)"
probe 200 "stats_live"             -X POST "$(sfn stats_live)"
probe 200 "stats_recent_blocks"    -X POST "$(sfn stats_recent_blocks)"        -d 'count=10'
probe 200 "stats_blocks"           -X POST "$(sfn stats_blocks)"               -d "from=$((TIP-100))&to=$TIP"
probe 200 "stats_blocks_by_ts"     -X POST "$(sfn stats_blocks_by_ts)"         -d 'from_ts=1757000000&to_ts=1757600000'
probe 200 "stats_block_detail"     -X POST "$(sfn stats_block_detail)"         -d 'height=840000'
probe 200 "stats_cumulative_size"  -X POST "$(sfn stats_cumulative_size)"      -d 'below_height=840000'
probe 200 "stats_cumulative_ts"    -X POST "$(sfn stats_cumulative_size_ts)"   -d 'before_ts=1700000000'
probe 200 "stats_daily_aggregates" -X POST "$(sfn stats_daily_aggregates)"     -d 'from_ts=1700000000&to_ts=1710000000'
probe 200 "stats_signaling"        -X POST "$(sfn stats_signaling)"            -d "bit=2&method=bit&from=$((TIP-2016))&to=$TIP"
probe 200 "stats_signaling_periods" -X POST "$(sfn stats_signaling_periods)"   -d 'bit=2&method=bit'
probe 200 "stats_miner_dominance"  -X POST "$(sfn stats_miner_dominance)"      -d "from=$((TIP-1000))&to=$TIP"
probe 200 "stats_miner_dom_daily"  -X POST "$(sfn stats_miner_dominance_daily)" -d 'from_ts=1700000000&to_ts=1710000000'
probe 200 "empty_blocks_monthly"   -X POST "$(sfn stats_empty_blocks_monthly)" -d "from=0&to=$TIP"
probe 200 "empty_blocks_by_pool"   -X POST "$(sfn stats_empty_blocks_by_pool)" -d "from=0&to=$TIP"
probe 200 "stats_price_history"    -X POST "$(sfn stats_price_history)"        -d 'from_ts=0&to_ts=4000000000'
probe 200 "stats_block_timestamp"  -X POST "$(sfn stats_block_timestamp)"      -d 'height=840000'
probe 200 "mining_price_summary"   -X POST "$(sfn mining_price_summary)"       -d 'from_ts=1700000000&to_ts=1710000000'
probe 200 "fullness_histogram"     -X POST "$(sfn fullness_histogram)"         -d 'from_ts=0&to_ts=4000000000'
probe 200 "block_time_histogram"   -X POST "$(sfn block_time_histogram)"       -d 'from_ts=0&to_ts=4000000000'
probe 200 "on_this_day"            -X POST "$(sfn on_this_day)"                -d 'month=5&day=22'
probe 200 "range_summary"          -X POST "$(sfn range_summary)"              -d 'from_ts=1700000000&to_ts=1710000000'
probe 200 "extremes"               -X POST "$(sfn extremes)"                   -d 'from_ts=1700000000&to_ts=1710000000'
probe 200 "notable_stats"          -X POST "$(sfn notable_stats)"              -d 'since=1750000000'
probe 200 "notable_top"            -X POST "$(sfn notable_top)"                -d 'since=1750000000&limit=5'

# Nested struct arg. PostUrl encoding of a struct is serde_qs bracket form, but
# recorded rather than asserted: if the encoding is wrong the failure looks
# identical to a broken endpoint, which is the `Args|missing field` trap.
note "notable_txs (bracket form)"  -X POST "$(sfn notable_txs)" -d 'filter[confirmed_only]=false&filter[unconfirmed_only]=false&limit=5&offset=0'
note "notable_txs (flat form)"     -X POST "$(sfn notable_txs)" -d 'confirmed_only=false&unconfirmed_only=false&limit=5&offset=0'

# SSE: streams until disconnected, so cap it and confirm only that it opens and
# the first frame is the history event. The 256-connection limit lives here too.
hb_body=$(curl -s -m 3 "$BASE/api/stats/heartbeat" 2>/dev/null | head -c 60 | tr -d '\n')
hb_code=$(curl -s -m 3 -o /dev/null -w '%{http_code}' "$BASE/api/stats/heartbeat" 2>/dev/null)
printf '??  %-3s %-45s %s\n' "$hb_code" "/heartbeat (SSE, 3s cap)" "$hb_body"

echo
# Server fns are expected to answer 500 even for a client mistake, and that is
# not a finding to chase: server_fn 0.8.11 hardcodes INTERNAL_SERVER_ERROR in
# Res::error_response, so a ServerFnError cannot carry a 4xx. What to check
# here is the body, which should read "Bad request: ..." for a caller error
# versus "Internal server error" for a genuine fault. Left as notes because the
# distinction is in the text, not the code.
echo "== server fn error paths: 500 expected, read the BODY not the code =="
note "stats_blocks reversed"        -X POST "$(sfn stats_blocks)"        -d "from=$TIP&to=100"
note "stats_blocks over-wide"       -X POST "$(sfn stats_blocks)"        -d "from=0&to=$TIP"
note "stats_blocks_by_ts over-wide" -X POST "$(sfn stats_blocks_by_ts)"  -d 'from_ts=0&to_ts=4000000000'
note "stats_daily reversed"         -X POST "$(sfn stats_daily_aggregates)" -d 'from_ts=1710000000&to_ts=1700000000'
note "range_summary reversed"       -X POST "$(sfn range_summary)"       -d 'from_ts=1710000000&to_ts=1700000000'
note "extremes reversed"            -X POST "$(sfn extremes)"            -d 'from_ts=1710000000&to_ts=1700000000'
note "fullness_histogram reversed"  -X POST "$(sfn fullness_histogram)"  -d 'from_ts=1710000000&to_ts=1700000000'
note "on_this_day month=13"         -X POST "$(sfn on_this_day)"         -d 'month=13&day=1'
note "on_this_day day=99"           -X POST "$(sfn on_this_day)"         -d 'month=1&day=99'
note "stats_block_detail absent"    -X POST "$(sfn stats_block_detail)"  -d 'height=99999999'
note "stats_recent_blocks huge"     -X POST "$(sfn stats_recent_blocks)" -d 'count=999999'
note "missing required arg"         -X POST "$(sfn stats_blocks)"        -d 'from=1'

echo
echo "asserted: $pass ok, $fail unexpected;  $suspect recorded for review"
[ "$fail" -eq 0 ] || echo "^ investigate the BAD lines first"
