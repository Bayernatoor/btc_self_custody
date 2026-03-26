# Deployment Log: Droplet + WireGuard + Start9

Completed 2026-03-26. Staging live at `staging.wehodlbtc.com`.

## Droplet Setup

**Existing Droplet:** 2GB RAM / 2 vCPUs / 60GB / LON1 / Debian 12
Already running: Mempool Signet (Docker, ports 8080/8999)

```bash
# System deps
apt update && apt install -y build-essential pkg-config libssl-dev curl wireguard

# Rust + tools
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
rustup target add wasm32-unknown-unknown

# Extra swap needed — 2GB RAM isn't enough for cargo-leptos compilation
fallocate -l 4G /swapfile2 && chmod 600 /swapfile2 && mkswap /swapfile2 && swapon /swapfile2

cargo install cargo-leptos

# Tailwind CLI
curl -sLO https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-linux-x64
chmod +x tailwindcss-linux-x64 && mv tailwindcss-linux-x64 /usr/local/bin/tailwindcss

# App user + directories
useradd -r -m -s /bin/bash wehodlbtc
mkdir -p /opt/wehodlbtc/{app,data}
chown -R wehodlbtc:wehodlbtc /opt/wehodlbtc
```

## WireGuard Tunnel

### Droplet (server)

```bash
wg genkey | tee /etc/wireguard/server_private.key | wg pubkey > /etc/wireguard/server_public.key
chmod 600 /etc/wireguard/server_private.key

cat > /etc/wireguard/wg0.conf << EOF
[Interface]
Address = 10.20.0.1/24
ListenPort = 51820
PrivateKey = <actual key from server_private.key>

[Peer]
PublicKey = <router's public key>
AllowedIPs = 10.20.0.2/32, 192.168.8.0/24
PersistentKeepalive = 25
EOF

chmod 600 /etc/wireguard/wg0.conf
ufw allow 51820/udp
systemctl enable wg-quick@wg0
systemctl start wg-quick@wg0
```

### GL.iNet Router (client)

WireGuard Client config:
```
[Interface]
PrivateKey = <router's private key>
Address = 10.20.0.2/24

[Peer]
PublicKey = <droplet's public key>
Endpoint = 165.227.230.64:51820
AllowedIPs = 10.20.0.1/32
PersistentKeepalive = 25
```

**Critical settings in GL.iNet UI:**
- VPN mode: **Policy Mode** (NOT Global Mode — Global routes all traffic through tunnel)
- Primary Tunnel: target set to `10.20.0.1` only (not "All targets")
- Allow Non-VPN Traffic: **ON**
- Allow Remote Access the LAN Subnet: **ON** (in gear icon options)

### Start9 Bitcoin Core (iptables bridge)

Bitcoin Core runs in a Podman container on Start9. Port 8332 isn't exposed to the host.
Bridge with iptables on Start9:

```bash
# Find container IP (changes on restart!)
sudo podman inspect bitcoind.embassy | grep IPAddress
# e.g. 172.18.0.19

# Forward host:8332 → container:8332
sudo iptables -t nat -A PREROUTING -p tcp -d 192.168.8.131 --dport 8332 -j DNAT --to-destination 172.18.0.19:8332
sudo iptables -t nat -A POSTROUTING -p tcp -d 172.18.0.19 --dport 8332 -j MASQUERADE

# Persist
sudo sh -c 'iptables-save > /etc/iptables.rules'
sudo sh -c 'cat > /etc/network/if-pre-up.d/iptables-restore << "SCRIPT"
#!/bin/sh
/usr/sbin/iptables-restore < /etc/iptables.rules
SCRIPT'
sudo chmod +x /etc/network/if-pre-up.d/iptables-restore
```

**Warning:** Container IP changes when Bitcoin Core restarts on Start9. Must update
iptables rules when this happens. See TODO for automation.

## App Deployment

```bash
# Clone + build
cd /opt/wehodlbtc/app
git clone https://github.com/Bayernatoor/btc_self_custody.git .
cargo leptos build --release

# Upload backfilled DB from local machine
# FROM LOCAL: scp bitcoin_stats.db root@165.227.230.64:/opt/wehodlbtc/data/
chown wehodlbtc:wehodlbtc /opt/wehodlbtc/data/bitcoin_stats.db

# Environment file
cat > /opt/wehodlbtc/.env << 'EOF'
BITCOIN_STATS_RPC_URL=http://192.168.8.131:8332
BITCOIN_STATS_RPC_USER=bitcoin
BITCOIN_STATS_RPC_PASSWORD=r3rgg2qinyiyym5iw65v
BITCOIN_STATS_DB_PATH=/opt/wehodlbtc/data/bitcoin_stats.db
LEPTOS_SITE_ADDR=127.0.0.1:8000
EOF
chmod 600 /opt/wehodlbtc/.env
chown wehodlbtc:wehodlbtc /opt/wehodlbtc/.env

# Systemd service
cat > /etc/systemd/system/wehodlbtc.service << 'EOF'
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
EOF

systemctl daemon-reload
systemctl enable wehodlbtc
systemctl start wehodlbtc

# Fix ownership
chown -R wehodlbtc:wehodlbtc /opt/wehodlbtc/
```

## Nginx + SSL

```bash
# Site config
cat > /etc/nginx/sites-available/wehodlbtc << 'EOF'
server {
    server_name wehodlbtc.com www.wehodlbtc.com staging.wehodlbtc.com;

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
EOF

ln -s /etc/nginx/sites-available/wehodlbtc /etc/nginx/sites-enabled/
nginx -t && systemctl reload nginx

# DNS: A record for staging.wehodlbtc.com → 165.227.230.64 (Namecheap)
certbot --nginx -d staging.wehodlbtc.com
```

## Key Details

- **Start9 IP:** 192.168.8.131
- **Home subnet:** 192.168.8.0/24
- **WG tunnel:** 10.20.0.1 (Droplet) ↔ 10.20.0.2 (Router)
- **Bitcoin Core container IP:** varies (check `sudo podman inspect bitcoind.embassy | grep IPAddress`)
- **RPC creds:** bitcoin / r3rgg2qinyiyym5iw65v
- **Env var for password:** `BITCOIN_STATS_RPC_PASSWORD` (not `_PASS`)
- **DB location:** `/opt/wehodlbtc/data/bitcoin_stats.db`
- **Logs:** `journalctl -u wehodlbtc -f`
- **Backfill locally, SCP up** — don't backfill over the tunnel (too slow, 32 concurrent hits overwhelm it)

## Gotchas Encountered

1. `cargo install cargo-leptos` OOM killed — needed extra 4GB swap
2. GL.iNet Global Mode routes ALL traffic through tunnel — breaks internet
3. Start9 uses Podman not Docker — `apt` is blocked, no `socat` available
4. Bitcoin Core container port not exposed to host — needed iptables NAT
5. Container IP changes on Bitcoin Core restart — iptables must be updated
6. Env var is `BITCOIN_STATS_RPC_PASSWORD` not `BITCOIN_STATS_RPC_PASS`
7. Mempool frontend Docker container stopped during setup — `docker start docker_web_1`
