#!/usr/bin/env bash
#
# Run the tests that CI cannot: the ones needing the real database or the
# real node. Everything here is `#[ignore]`d, which is correct, because a
# GitHub runner has neither. The cost of that correctness is that nothing
# runs them at all unless someone remembers, and on 2026-11-19 that turned
# out to mean `the_copy_still_matches_the_chain` had been failing since
# September: a chart claimed P2PK outputs appear in one block in seven
# hundred, and a burst starting 2026-11-15 had made it one in a hundred.
#
# So this exists to be run, not to be read. Before a deploy, or weekly.
#
#   scripts/check-against-chain.sh              # database and node checks
#   scripts/check-against-chain.sh --with-sweep # also the sweep-safety tests,
#                                               # which copy the database first
#
# A prerequisite that is missing prints SKIP and says what is absent. A skip
# is not a pass and is counted separately, because the whole failure this
# script addresses is a check that quietly does not run.

set -uo pipefail
cd "$(dirname "$0")/.."

WITH_SWEEP=0
[[ "${1:-}" == "--with-sweep" ]] && WITH_SWEEP=1

PASS=0 FAIL=0 SKIP=0
FAILED_NAMES=()

run() { # run <label> <test-filter>
    printf '\n\033[1m%s\033[0m\n' "$1"
    local log="${TMPDIR:-/tmp}/check-against-chain-$$.log"
    cargo test --features ssr "$2" -- --ignored --nocapture >"$log" 2>&1
    local status=$?
    tail -40 "$log"

    # A filter matching nothing exits 0 having run nothing, which is this
    # script's own failure mode turned on itself: on the first real run
    # "sweep distrusts the hash cache" matched no test and was counted a
    # pass, because that test lives under `fifth_review_fixes::` and the
    # filter did not say so. Count the harness lines rather than trust the
    # exit code.
    local ran
    ran=$(grep -cE '^test .+ \.\.\. (ok|FAILED)' "$log")
    rm -f "$log"

    if ((status != 0)); then
        FAIL=$((FAIL + 1))
        FAILED_NAMES+=("$1")
    elif ((ran == 0)); then
        printf '  \033[31mNO MATCH\033[0m filter %s ran no tests\n' "$2"
        FAIL=$((FAIL + 1))
        FAILED_NAMES+=("$1 (no match)")
    else
        PASS=$((PASS + 1))
    fi
}

skip() { # skip <label> <reason>
    printf '\n\033[1m%s\033[0m\n  \033[33mSKIP\033[0m %s\n' "$1" "$2"
    SKIP=$((SKIP + 1))
}

# ---------------------------------------------------------------------------
# Needs bitcoin_stats.db. These are the drift guards: every figure quoted in
# chart copy, re-measured against the chain.
# ---------------------------------------------------------------------------
if [[ -f bitcoin_stats.db ]]; then
    run "copy still matches the chain"   the_copy_still_matches_the_chain
    run "copy still matches the records" the_copy_still_matches_the_records
    run "activation timestamps"          activation_timestamps_match_the_chain
    run "difficulty acceptance windows"  the_r3_acceptance_windows_against_the_live_database
    run "daily reader vs the rollup"     the_daily_reader_returns_what_the_rollup_stores
else
    skip "database checks (5)" "no bitcoin_stats.db in $(pwd)"
fi

# ---------------------------------------------------------------------------
# Needs the node. `.env` is gitignored, so this is per-machine by design.
# ---------------------------------------------------------------------------
if [[ -f .env ]]; then
    set -a; . ./.env; set +a
fi

if [[ -n "${BITCOIN_STATS_RPC_URL:-}" ]] &&
   curl -s --max-time 5 -o /dev/null "$BITCOIN_STATS_RPC_URL"; then
    run "raw block parity vs Core"       raw_parity::raw_matches_json
    run "coinbase payout address"        raw_parity::payout_address_is_the_largest_output
    run "every output is classified"     raw_parity::the_classified_types_account_for_every_output
    run "what the sweep will write"      raw_parity::what_the_sweep_will_write
    run "sweep distrusts the hash cache" fifth_review_fixes::the_sweep_does_not_trust_the_hash_cache
else
    skip "node checks (5)" "BITCOIN_STATS_RPC_URL unset or unreachable; source .env"
fi

# ---------------------------------------------------------------------------
# Sweep safety. Writes, so it insists on a copy rather than the live file.
# Opt-in because the copy is most of a gigabyte.
# ---------------------------------------------------------------------------
SWEEP_COPY=""
cleanup() { [[ -n "$SWEEP_COPY" ]] && rm -f "$SWEEP_COPY" "$SWEEP_COPY"-shm "$SWEEP_COPY"-wal; }
trap cleanup EXIT

if [[ "$WITH_SWEEP" == "1" && -f bitcoin_stats.db ]]; then
    SWEEP_COPY="${TMPDIR:-/tmp}/sweep-dry-$$.db"
    cp bitcoin_stats.db "$SWEEP_COPY"
    export SWEEP_DRY_DB="$SWEEP_COPY"
    run "sweep dry run"             raw_parity::the_sweep_dry_run
    run "a reorg cannot corrupt"    raw_parity::a_reorg_cannot_corrupt_a_swept_row
    run "a dead node stops"         fifth_review_fixes::a_dead_node_stops_the_sweep_rather_than_spinning
else
    skip "sweep safety (3)" "pass --with-sweep; they need SWEEP_DRY_DB set to a copy"
fi

# ---------------------------------------------------------------------------
# The classifier oracle. Deliberately not run, and this is a known gap: the
# fixture is a hand-built list of (script hex, Core's label) that lived in a
# tmp directory, nothing in the repo regenerates it, and a wiped /tmp means
# the test can never pass again on a fresh machine. Recorded here rather
# than silently omitted. See `the_classifier_agrees_with_core`.
# ---------------------------------------------------------------------------
if [[ -n "${SCRIPT_ORACLE:-}" && -f "${SCRIPT_ORACLE:-}" ]]; then
    run "classifier agrees with Core" raw_parity::the_classifier_agrees_with_core
    run "oracle covers new columns"   raw_parity::oracle_new_columns
else
    skip "classifier oracle (2)" "SCRIPT_ORACLE unset; no generator exists for it"
fi

printf '\n\033[1m%d passed, %d failed, %d skipped\033[0m\n' "$PASS" "$FAIL" "$SKIP"
if ((FAIL > 0)); then
    printf '\033[31mfailed:\033[0m %s\n' "${FAILED_NAMES[*]}"
    exit 1
fi
((SKIP > 0)) && printf 'A skip is not a pass. Fix the prerequisite or delete the test.\n'
exit 0
