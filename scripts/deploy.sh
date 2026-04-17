#!/bin/bash
# LEGACY: on-droplet build-and-deploy. Kept as a manual fallback only.
#
# The production deploy pipeline now builds on GitHub Actions and rsyncs the
# release binary + target/site/ into place (see .github/workflows/deploy.yml
# and scripts/deploy-remote.sh). That path avoids running cargo on the 2GB
# droplet and keeps target/ from growing unbounded on disk.
#
# Use this script only if the GitHub Actions pipeline is broken and you need
# to recover prod by SSHing in directly:
#   ssh wehodlbtc@<droplet-ip> 'bash /opt/wehodlbtc/app/scripts/deploy.sh'
# It assumes the repo is still checked out at /opt/wehodlbtc/app, which may
# no longer be true once we prune the source tree from the droplet.

set -e

# Source cargo env (non-interactive SSH doesn't load .bashrc)
source "$HOME/.cargo/env"

cd /opt/wehodlbtc/app
echo "==> Pulling latest code..."
git checkout -- . 2>/dev/null  # discard any local changes from previous build/deploy
git pull origin master

echo "==> Updating service worker cache version..."
DEPLOY_TS=$(date +%s)
sed -i "s/var CACHE_NAME = 'wehodlbtc-[^']*'/var CACHE_NAME = 'wehodlbtc-${DEPLOY_TS}'/" assets/sw.js

echo "==> Running tests..."
cargo test || { echo "==> TESTS FAILED — aborting deploy"; exit 1; }
echo "==> All tests passed..."

echo "==> Building release..."
cargo leptos build --release

echo "==> Restarting service..."
sudo systemctl restart wehodlbtc

echo "==> Verifying..."
sleep 2
sudo systemctl is-active wehodlbtc && echo "==> Deploy successful!" || echo "==> FAILED — check logs: journalctl -u wehodlbtc -n 30"
