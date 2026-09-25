#!/bin/bash
set -e

ROUTER_HOST="${1:-root@192.168.1.1}"

echo "=== 1. 执行自动打包 ==="
./build_apk.sh

APK_PATH=$(ls -t dist/*.apk | head -n 1)
APK_NAME=$(basename "$APK_PATH")

if [ ! -f "$APK_PATH" ]; then
    echo "错误：未找到生成的 apk 包！"
    exit 1
fi

echo "=== 2. 推送 apk 到目标路由器: $ROUTER_HOST ==="
scp "$APK_PATH" "$ROUTER_HOST:/tmp/$APK_NAME"

echo "=== 3. 在路由器上执行安装与配置生效 ==="
ssh "$ROUTER_HOST" "
    if which apk >/dev/null 2>&1; then
        echo '检测到 apk 包管理器，正在执行安装...'
        apk add --allow-untrusted '/tmp/$APK_NAME'
    else
        echo '检测到非 apk 环境，请使用 apk 包管理器安装。'
    fi

    # 清除 LuCI 缓存以展示新菜单
    rm -rf /tmp/luci-indexcache* /tmp/luci-modulecache*
    /etc/init.d/rpcd restart
    
    echo '安装成功！可在 LuCI 网页后台 [网络] -> [Side Route] 体验全新控制台。'
"
