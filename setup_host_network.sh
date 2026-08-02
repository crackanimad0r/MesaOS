#!/usr/bin/bash
# setup_host_network.sh - Versión ultra-robusta

# 1. Detectar usuario real (no root si se usa sudo)
REAL_USER=${SUDO_USER:-$USER}
IFACE=$(ip route | grep default | awk '{print $5}' | head -n 1)

echo "--- Configuración Maestra de Red ---"
echo "Usuario objetivo: $REAL_USER"
echo "Interfaz salida: $IFACE"

# 2. Limpieza total
sudo ip link delete tap0 2>/dev/null
sudo iptables -F FORWARD 2>/dev/null
sudo iptables -t nat -F POSTROUTING 2>/dev/null

# 3. Crear interfaz TAP persistente
sudo ip tuntap add name tap0 mode tap user "$REAL_USER"
sudo ip addr add 10.0.2.2/24 dev tap0
sudo ip link set tap0 up

# 4. Habilitar el motor de enrutamiento de Linux
sudo sysctl -w net.ipv4.ip_forward=1 > /dev/null
sudo sysctl -w net.ipv4.conf.all.proxy_arp=1 > /dev/null
sudo sysctl -w net.ipv4.conf.tap0.proxy_arp=1 > /dev/null
sudo sysctl -w net.ipv4.conf.$IFACE.proxy_arp=1 > /dev/null

# 5. NAT y Forwarding (Para que MesaOS salga a Internet por tu WiFi)
sudo iptables -A FORWARD -i tap0 -j ACCEPT
sudo iptables -A FORWARD -o tap0 -j ACCEPT
sudo iptables -t nat -A POSTROUTING -o $IFACE -j MASQUERADE

echo "---------------------------------------"
echo "[OK] Interfaz tap0 configurada con IP 10.0.2.2"
echo "Verificación: $(ip addr show tap0 | grep 'inet ')"
echo "---------------------------------------"
