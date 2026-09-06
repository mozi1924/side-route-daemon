#!/bin/bash
set -e

ROUTER_HOST="${1:-root@192.168.1.1}"

echo "=== 正在编译 side-route-daemon (x86_64 musl 纯静态二进制) ==="
cargo build --release --target x86_64-unknown-linux-musl

BIN_PATH="target/x86_64-unknown-linux-musl/release/side-route-daemon"

if [ ! -f "$BIN_PATH" ]; then
    echo "错误：未找到编译产物 $BIN_PATH"
    exit 1
fi

echo "=== 准备推送至路由器: $ROUTER_HOST ==="
ssh "$ROUTER_HOST" "mkdir -p /etc/side-route-daemon"
scp "$BIN_PATH" "$ROUTER_HOST:/usr/bin/side-route-daemon"
ssh "$ROUTER_HOST" "chmod +x /usr/bin/side-route-daemon"

# 如果路由器上还没有配置文件，则推送默认配置文件
ssh "$ROUTER_HOST" "[ -f /etc/side-route-daemon/config.toml ] || cat > /etc/side-route-daemon/config.toml" < service/config.toml

# 推送 procd 服务脚本
scp service/side-route-daemon "$ROUTER_HOST:/etc/init.d/side-route-daemon"
ssh "$ROUTER_HOST" "chmod +x /etc/init.d/side-route-daemon"

echo "=== 启用并启动服务 ==="
ssh "$ROUTER_HOST" "/etc/init.d/side-route-daemon enable"
ssh "$ROUTER_HOST" "/etc/init.d/side-route-daemon restart"

echo "=== 部署完成！查看当前运行状态： ==="
sleep 2
ssh "$ROUTER_HOST" "logread -l 20 | grep side-route || true"
ssh "$ROUTER_HOST" "ip -6 rule show | grep 200 || true"
ssh "$ROUTER_HOST" "nft list table inet side_route_guard || true"
