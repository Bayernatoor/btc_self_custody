#!/bin/bash
# Runs on Start9 to forward RPC + ZMQ ports from the host to the bitcoind Podman container.
# The container IP can change on restart, so this script detects it and updates iptables.
#
# Install as cron job (every 5 minutes):
#   sudo crontab -e
#   */5 * * * * /home/start9/start9-iptables.sh >> /home/start9/iptables.log 2>&1
#
# Or run manually after container restart:
#   sudo bash /home/start9/start9-iptables.sh

set -e

# Detect the current container IP
CONTAINER_IP=$(sudo podman inspect bitcoind.embassy | grep -m1 '"IPAddress"' | grep -oP '\d+\.\d+\.\d+\.\d+')

if [ -z "$CONTAINER_IP" ]; then
    echo "$(date): ERROR - Could not detect bitcoind container IP"
    exit 1
fi

# The host LAN IP that the Droplet routes to via WireGuard
HOST_IP="192.168.8.131"

# Ports to forward
PORTS="8332 28332 28333"

# Check if rules already exist with the correct IP
CURRENT_IP=$(sudo iptables -t nat -L PREROUTING -n 2>/dev/null | grep "dpt:8332" | grep -oP 'to:\K[\d.]+' | head -1)

if [ "$CURRENT_IP" = "$CONTAINER_IP" ]; then
    # Rules are already correct
    exit 0
fi

echo "$(date): Container IP changed: $CURRENT_IP -> $CONTAINER_IP. Updating iptables..."

# Remove old DNAT rules for our ports
for PORT in $PORTS; do
    # Remove any existing PREROUTING rules for this port
    while sudo iptables -t nat -D PREROUTING -p tcp -d "$HOST_IP" --dport "$PORT" -j DNAT --to-destination "$CURRENT_IP:$PORT" 2>/dev/null; do true; done
    # Remove any existing FORWARD rules for this port
    while sudo iptables -D FORWARD -p tcp -d "$CURRENT_IP" --dport "$PORT" -j ACCEPT 2>/dev/null; do true; done
done

# Add new rules with the current container IP
for PORT in $PORTS; do
    sudo iptables -t nat -A PREROUTING -p tcp -d "$HOST_IP" --dport "$PORT" -j DNAT --to-destination "$CONTAINER_IP:$PORT"
    sudo iptables -A FORWARD -p tcp -d "$CONTAINER_IP" --dport "$PORT" -j ACCEPT
done

echo "$(date): iptables updated. Forwarding ports $PORTS to $CONTAINER_IP"
