#!/bin/bash
# Remote deploy script — runs on the droplet after CI has rsynced artifacts
# into /opt/wehodlbtc/staging/. Performs an atomic swap of the binary and
# site directory, restarts the service, and rolls back if the service fails
# to come up healthy.
#
# Invariants:
#   - Never modifies /opt/wehodlbtc/data/ (SQLite DBs)
#   - Never modifies /opt/wehodlbtc/.env (runtime config)
#   - Leaves systemd, nginx, wireguard alone
#   - On failure, restores the previous binary and site dir if they exist

set -euo pipefail

APP_DIR="/opt/wehodlbtc/app"
STAGING_DIR="/opt/wehodlbtc/staging"
BIN_PATH="${APP_DIR}/target/release/we_hodl_btc"
SITE_PATH="${APP_DIR}/target/site"
TOOLS_PATH="${APP_DIR}/tools"

log() { echo "==> $*"; }
die() { echo "!! $*" >&2; exit 1; }

[ -f "${STAGING_DIR}/we_hodl_btc" ] || die "staging binary missing at ${STAGING_DIR}/we_hodl_btc"
[ -d "${STAGING_DIR}/site" ] || die "staging site dir missing at ${STAGING_DIR}/site"

log "Preparing target directory"
mkdir -p "${APP_DIR}/target/release"

log "Backing up current running artifacts"
if [ -f "${BIN_PATH}" ]; then
    mv "${BIN_PATH}" "${BIN_PATH}.prev"
fi
if [ -d "${SITE_PATH}" ]; then
    rm -rf "${SITE_PATH}.prev"
    mv "${SITE_PATH}" "${SITE_PATH}.prev"
fi

log "Installing new artifacts"
mv "${STAGING_DIR}/we_hodl_btc" "${BIN_PATH}"
mv "${STAGING_DIR}/site" "${SITE_PATH}"
chmod +x "${BIN_PATH}"

# One-shot maintenance utilities (rebuild_daily_blocks, backfill_missing_heights).
# Not part of the served release: no .prev copy and no rollback, because they
# only read the DB and rewrite derived tables, so an older app with a newer tool
# is harmless. Guarded on existence so this script still works against an
# artifact built before CI started shipping them.
if [ -d "${STAGING_DIR}/tools" ]; then
    log "Installing maintenance utilities"
    mkdir -p "${TOOLS_PATH}"
    for tool in "${STAGING_DIR}"/tools/*; do
        [ -f "$tool" ] || continue
        install -m 755 "$tool" "${TOOLS_PATH}/$(basename "$tool")"
    done
    rm -rf "${STAGING_DIR}/tools"
    ls -la "${TOOLS_PATH}"
fi

log "Restarting service"
sudo systemctl restart wehodlbtc

# Give systemd a moment to surface an immediate-startup crash
# (port-bind failure, config error) before we inspect state.
sleep 5

rollback() {
    log "$1 — rolling back"
    rm -f "${BIN_PATH}"
    rm -rf "${SITE_PATH}"
    [ -f "${BIN_PATH}.prev" ] && mv "${BIN_PATH}.prev" "${BIN_PATH}"
    [ -d "${SITE_PATH}.prev" ] && mv "${SITE_PATH}.prev" "${SITE_PATH}"
    sudo systemctl restart wehodlbtc
    die "rolled back to previous release; check: sudo journalctl -u wehodlbtc -n 50"
}

if ! sudo systemctl is-active wehodlbtc >/dev/null 2>&1; then
    rollback "Service failed to start"
fi

# HTTP health check with bounded retry. Server startup is not instant:
# RPC client init, ZMQ subscribe, mempool seed, and forward-ingestion
# catch-up all run before /  serves. A single shot gambled against
# variable startup time; previous deploys flagged a failure when the
# binary was simply still warming up. Poll up to ~30s before giving up.
if command -v curl >/dev/null 2>&1; then
    HEALTHY=0
    for attempt in $(seq 1 15); do
        if curl -fsS -o /dev/null -m 5 http://127.0.0.1:8000/; then
            HEALTHY=1
            log "Health check passed on attempt ${attempt}"
            break
        fi
        sleep 2
    done
    [ "${HEALTHY}" = "1" ] || rollback "HTTP health check failed after retries"
fi

log "Cleaning up previous release"
rm -f "${BIN_PATH}.prev"
rm -rf "${SITE_PATH}.prev"
rm -rf "${STAGING_DIR}"

log "Deploy successful"
