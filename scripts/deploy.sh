#!/bin/bash
# Deploy to production Droplet
# Usage: ssh root@165.227.230.64 'bash -s' < scripts/deploy.sh
#    or: run directly on the Droplet

set -e

cd /opt/wehodlbtc/app
echo "==> Pulling latest code..."
git checkout -- assets/sw.js 2>/dev/null  # reset SW cache stamp from previous deploy
git pull origin master

echo "==> Updating service worker cache version..."
DEPLOY_TS=$(date +%s)
sed -i "s/var CACHE_NAME = 'wehodlbtc-[^']*'/var CACHE_NAME = 'wehodlbtc-${DEPLOY_TS}'/" assets/sw.js

echo "==> Running tests..."
cargo test || { echo "==> TESTS FAILED — aborting deploy"; exit 1; }

echo "==> Building release..."
cargo leptos build --release

echo "==> Restarting service..."
systemctl restart wehodlbtc

echo "==> Verifying..."
sleep 2
systemctl is-active wehodlbtc && echo "==> Deploy successful!" || echo "==> FAILED — check logs: journalctl -u wehodlbtc -n 30"
