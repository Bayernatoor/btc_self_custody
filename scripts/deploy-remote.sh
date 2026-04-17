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

log "Restarting service"
sudo systemctl restart wehodlbtc

# Give it a moment to either come up cleanly or crash on a port bind / config error
sleep 3

if ! sudo systemctl is-active --quiet wehodlbtc; then
    log "Service failed to start — rolling back"
    rm -f "${BIN_PATH}"
    rm -rf "${SITE_PATH}"
    [ -f "${BIN_PATH}.prev" ] && mv "${BIN_PATH}.prev" "${BIN_PATH}"
    [ -d "${SITE_PATH}.prev" ] && mv "${SITE_PATH}.prev" "${SITE_PATH}"
    sudo systemctl restart wehodlbtc
    die "rolled back to previous release; check: sudo journalctl -u wehodlbtc -n 50"
fi

# Optional HTTP health check — confirms the app is actually serving, not just running
if command -v curl >/dev/null 2>&1; then
    if ! curl -fsS -o /dev/null -m 10 http://127.0.0.1:8000/; then
        log "HTTP health check failed — rolling back"
        rm -f "${BIN_PATH}"
        rm -rf "${SITE_PATH}"
        [ -f "${BIN_PATH}.prev" ] && mv "${BIN_PATH}.prev" "${BIN_PATH}"
        [ -d "${SITE_PATH}.prev" ] && mv "${SITE_PATH}.prev" "${SITE_PATH}"
        sudo systemctl restart wehodlbtc
        die "rolled back to previous release; check: sudo journalctl -u wehodlbtc -n 50"
    fi
fi

log "Cleaning up previous release"
rm -f "${BIN_PATH}.prev"
rm -rf "${SITE_PATH}.prev"
rm -rf "${STAGING_DIR}"

log "Deploy successful"
