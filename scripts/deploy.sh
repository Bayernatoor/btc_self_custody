#!/bin/bash
# Deploy to production Droplet
# Usage: ssh root@165.227.230.64 'bash -s' < scripts/deploy.sh
#    or: run directly on the Droplet

set -e

cd /opt/wehodlbtc/app
echo "==> Pulling latest code..."
git pull origin master

echo "==> Building release..."
cargo leptos build --release

echo "==> Restarting service..."
systemctl restart wehodlbtc

echo "==> Verifying..."
sleep 2
systemctl is-active wehodlbtc && echo "==> Deploy successful!" || echo "==> FAILED — check logs: journalctl -u wehodlbtc -n 30"
