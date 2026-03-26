# Migration Plan: App Platform → Droplet + WireGuard

## Overview

Migrate wehodlbtc.com from DigitalOcean App Platform to the existing Droplet (LON1),
add WireGuard tunnel to home Start9 node, and set up a deployment workflow.

**Droplet:** 2 GB RAM / 2 AMD vCPUs / 60 GB Disk / LON1 / Debian 12 x64
**Currently runs:** Mempool Signet at mempool.wehodlbtc.com

---

## Phase 1: Prepare the Droplet

### 1.1 Install dependencies

```bash
# On the Droplet
sudo apt update && sudo apt upgrade -y
sudo apt install -y build-essential pkg-config libssl-dev curl nginx certbot python3-certbot-nginx wireguard

# Install Rust (if not already)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup target add wasm32-unknown-unknown

# Install cargo-leptos (builds the full-stack app)
cargo install cargo-leptos

# Install Tailwind CSS CLI
curl -sLO https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-linux-x64
chmod +x tailwindcss-linux-x64
sudo mv tailwindcss-linux-x64 /usr/local/bin/tailwindcss
```

### 1.2 Create app user and directories

```bash
sudo useradd -r -m -s /bin/bash wehodlbtc
sudo mkdir -p /opt/wehodlbtc/{app,data}
sudo chown -R wehodlbtc:wehodlbtc /opt/wehodlbtc
```

---

## Phase 2: WireGuard Tunnel (Droplet ↔ Home)

### 2.1 Generate keys on Droplet

```bash
# On Droplet
wg genkey | tee /etc/wireguard/server_private.key | wg pubkey > /etc/wireguard/server_public.key
chmod 600 /etc/wireguard/server_private.key
```

### 2.2 Configure Droplet WireGuard server

```bash
# /etc/wireguard/wg0.conf
[Interface]
Address = 10.20.0.1/24
ListenPort = 51820
PrivateKey = <droplet_private_key>

# Home router peer
[Peer]
PublicKey = <home_router_public_key>
AllowedIPs = 10.20.0.2/32, 192.168.X.0/24   # <-- include Start9 LAN subnet
```

**Important:** Replace `192.168.X.0/24` with your actual home LAN subnet so the
Droplet can reach Start9 via the tunnel.

### 2.3 Enable and start WireGuard

```bash
sudo systemctl enable wg-quick@wg0
sudo systemctl start wg-quick@wg0
```

### 2.4 Firewall: allow WireGuard UDP port

```bash
sudo ufw allow 51820/udp
```

### 2.5 Configure GL.iNet router (WireGuard Client)

1. Log into GL.iNet admin panel
2. Go to **VPN → WireGuard Client**
3. Add new profile:
   - **Endpoint:** `<droplet_public_ip>:51820`
   - **Public Key:** Contents of `server_public.key` from Droplet
   - **Private Key:** Generate on router (or manually with `wg genkey`)
   - **Address:** `10.20.0.2/24`
   - **Allowed IPs:** `10.20.0.1/32` (only route Droplet traffic through tunnel)
   - **Persistent Keepalive:** `25` (keeps NAT mapping alive)
4. Enable the profile
5. Copy the router's public key back to the Droplet's `[Peer]` section

### 2.6 Configure Start9 Bitcoin Core RPC

On Start9, update Bitcoin Core config to allow RPC from the tunnel:

```
# In bitcoin.conf (via Start9 UI or SSH)
rpcallowip=10.20.0.0/24
rpcbind=0.0.0.0          # or specific interface
```

**Note:** Start9 may manage bitcoin.conf through its UI. Check if you can add
custom RPC settings there. The key is allowing the WireGuard subnet.

### 2.7 Test the tunnel

```bash
# From Droplet
ping 10.20.0.2                              # Should reach home router
curl --user <rpc_user>:<rpc_pass> \
  --data-binary '{"method":"getblockchaininfo"}' \
  http://192.168.X.Y:8332/                  # Start9 LAN IP via tunnel
```

If this returns blockchain info, the tunnel is working.

---

## Phase 3: Deploy the App

### 3.1 Create a deployment branch

```bash
# On your local machine
git checkout -b production
git push origin production
```

This branch will be what the Droplet pulls from. Your `master` branch continues
deploying to App Platform (until you're ready to switch DNS).

### 3.2 Clone repo on Droplet

```bash
# On Droplet, as wehodlbtc user
sudo -u wehodlbtc bash
cd /opt/wehodlbtc/app
git clone https://github.com/Bayernatoor/btc_self_custody.git .
git checkout production
```

### 3.3 Upload the SQLite database

```bash
# From your LOCAL machine (where the backfill was done)
scp bitcoin_stats.db root@<droplet_ip>:/opt/wehodlbtc/data/bitcoin_stats.db
sudo chown wehodlbtc:wehodlbtc /opt/wehodlbtc/data/bitcoin_stats.db
```

### 3.4 Build the app on the Droplet

```bash
# As wehodlbtc user
cd /opt/wehodlbtc/app
cargo leptos build --release
```

This will take a while on first build (~5-10 min on 2 vCPUs). Subsequent builds
are faster (incremental).

### 3.5 Create environment file

```bash
# /opt/wehodlbtc/.env
BITCOIN_STATS_RPC_URL=http://192.168.X.Y:8332
BITCOIN_STATS_RPC_USER=<your_rpc_user>
BITCOIN_STATS_RPC_PASS=<your_rpc_pass>
BITCOIN_STATS_DB_PATH=/opt/wehodlbtc/data/bitcoin_stats.db
LEPTOS_SITE_ADDR=127.0.0.1:8000
```

### 3.6 Create systemd service

```bash
# /etc/systemd/system/wehodlbtc.service
[Unit]
Description=WE HODL BTC - Bitcoin Observatory
After=network.target wg-quick@wg0.service

[Service]
Type=simple
User=wehodlbtc
Group=wehodlbtc
WorkingDirectory=/opt/wehodlbtc/app
EnvironmentFile=/opt/wehodlbtc/.env
ExecStart=/opt/wehodlbtc/app/target/release/we_hodl_btc
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable wehodlbtc
sudo systemctl start wehodlbtc
```

### 3.7 Verify the app is running

```bash
curl http://127.0.0.1:8000/          # Should return HTML
curl http://127.0.0.1:8000/api/stats/live  # Should return JSON with live data
```

---

## Phase 4: Nginx Reverse Proxy + SSL

### 4.1 Configure Nginx

```nginx
# /etc/nginx/sites-available/wehodlbtc
server {
    listen 80;
    server_name wehodlbtc.com www.wehodlbtc.com;

    location / {
        proxy_pass http://127.0.0.1:8000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

```bash
sudo ln -s /etc/nginx/sites-available/wehodlbtc /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

### 4.2 Get SSL certificate

```bash
sudo certbot --nginx -d wehodlbtc.com -d www.wehodlbtc.com
```

Certbot will auto-configure HTTPS and set up auto-renewal.

---

## Phase 5: DNS Cutover

### 5.1 Test with a subdomain first (optional but recommended)

1. Create an A record: `staging.wehodlbtc.com → <droplet_ip>`
2. Add `staging.wehodlbtc.com` to the Nginx config
3. Get SSL cert for it: `sudo certbot --nginx -d staging.wehodlbtc.com`
4. Test everything works at `https://staging.wehodlbtc.com`

### 5.2 Switch DNS

1. Update A record: `wehodlbtc.com → <droplet_ip>`
2. Update A record: `www.wehodlbtc.com → <droplet_ip>`
3. Wait for DNS propagation (~5-30 min)
4. Verify site works
5. Once confirmed, remove the App Platform deployment

---

## Phase 6: Deployment Workflow

### Option A: Simple pull + rebuild (recommended to start)

Create a deploy script on the Droplet:

```bash
#!/bin/bash
# /opt/wehodlbtc/deploy.sh
set -e
cd /opt/wehodlbtc/app
git pull origin production
cargo leptos build --release
sudo systemctl restart wehodlbtc
echo "Deployed at $(date)"
```

To deploy: push to `production` branch, then SSH in and run `./deploy.sh`.

### Option B: GitHub Actions (future enhancement)

Set up a GitHub Action on the `production` branch that SSHs into the Droplet
and runs the deploy script. This gives you push-to-deploy like App Platform.

```yaml
# .github/workflows/deploy.yml
name: Deploy to Droplet
on:
  push:
    branches: [production]
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: appleboy/ssh-action@v1
        with:
          host: ${{ secrets.DROPLET_IP }}
          username: wehodlbtc
          key: ${{ secrets.SSH_KEY }}
          script: /opt/wehodlbtc/deploy.sh
```

---

## Phase 7: DB Updates (when needed)

If BACKFILL_VERSION bumps (new columns requiring re-parse):

```bash
# 1. Run backfill LOCALLY against your node (fast, local network)
#    Start the app locally, let it backfill, wait for completion

# 2. Stop the app on Droplet
sudo systemctl stop wehodlbtc

# 3. Upload new DB
scp bitcoin_stats.db root@<droplet_ip>:/opt/wehodlbtc/data/bitcoin_stats.db
sudo chown wehodlbtc:wehodlbtc /opt/wehodlbtc/data/bitcoin_stats.db

# 4. Restart
sudo systemctl start wehodlbtc
```

---

## Rollback Plan

If anything goes wrong:
- App Platform is still running on `master` branch
- DNS can be pointed back to App Platform in minutes
- No data loss — SQLite DB stays on Droplet, App Platform has its own state

---

## Checklist

- [ ] Install dependencies on Droplet
- [ ] Generate WireGuard keys
- [ ] Configure WireGuard on Droplet (server)
- [ ] Configure WireGuard on GL.iNet router (client)
- [ ] Update Start9 Bitcoin Core RPC config
- [ ] Test tunnel connectivity + RPC access
- [ ] Create `production` branch
- [ ] Clone repo on Droplet
- [ ] Upload SQLite DB via SCP
- [ ] Build app on Droplet
- [ ] Create .env file with RPC credentials
- [ ] Create systemd service
- [ ] Verify app works on localhost:8000
- [ ] Configure Nginx reverse proxy
- [ ] Get SSL certificate
- [ ] Test with staging subdomain
- [ ] Switch DNS to Droplet
- [ ] Verify everything works
- [ ] Remove App Platform deployment
- [ ] Set up deploy script or GitHub Actions
