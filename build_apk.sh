#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

PKG_NAME="luci-app-sideroute"
PKG_VERSION="1.0.0-r1"
ARCH="x86_64"
OUTPUT_DIR="$SCRIPT_DIR/dist"
STAGING_DIR="$SCRIPT_DIR/target/apk-staging"
SDK_DIR="/home/mozi/openwrt-sdk/openwrt-sdk-25.12.5-x86-64_gcc-14.3.0_musl.Linux-x86_64"

echo "=========================================================="
echo " 正在构建 $PKG_NAME v$PKG_VERSION for OpenWrt 25.x ($ARCH)"
echo "=========================================================="

# 1. 确认 OpenWrt SDK 工具链与 apk 打包工具
HOST_APK="$SDK_DIR/staging_dir/host/bin/apk"
if [ ! -x "$HOST_APK" ]; then
    if which apk >/dev/null 2>&1; then
        HOST_APK="$(which apk)"
    else
        echo "错误：未找到 apk 打包工具 (apk-tools 3.x)。"
        exit 1
    fi
fi
echo "使用打包工具: $HOST_APK ($($HOST_APK --version 2>&1 | head -n 1))"

# 2. 准备 musl gcc wrapper
mkdir -p target
cat << 'EOF' > target/musl-gcc-wrapper.sh
#!/bin/bash
export STAGING_DIR="/home/mozi/openwrt-sdk/openwrt-sdk-25.12.5-x86-64_gcc-14.3.0_musl.Linux-x86_64/staging_dir"
TOOLCHAIN_DIR="$STAGING_DIR/toolchain-x86_64_gcc-14.3.0_musl"
GCC_LIB="$TOOLCHAIN_DIR/lib/gcc/x86_64-openwrt-linux-musl/14.3.0"
LIB_DIR="$TOOLCHAIN_DIR/lib"

args=()
for arg in "$@"; do
    case "$arg" in
        rcrt1.o) args+=("$LIB_DIR/crt1.o") ;;
        crt1.o) args+=("$LIB_DIR/crt1.o") ;;
        crti.o) args+=("$LIB_DIR/crti.o") ;;
        crtn.o) args+=("$LIB_DIR/crtn.o") ;;
        crtbeginS.o) args+=("$GCC_LIB/crtbegin.o") ;;
        crtendS.o) args+=("$GCC_LIB/crtend.o") ;;
        -static-pie) args+=("-static" "-no-pie") ;;
        *) args+=("$arg") ;;
    esac
done

exec "$TOOLCHAIN_DIR/bin/x86_64-openwrt-linux-musl-gcc" \
    -L"$LIB_DIR" \
    -L"$GCC_LIB" \
    "${args[@]}"
EOF
chmod +x target/musl-gcc-wrapper.sh

# 3. 编译纯静态 musl Rust 二进制
BIN_PATH="target/x86_64-unknown-linux-musl/release/side-route-daemon"
echo "--> 正在编译 Rust 二进制 (纯静态 x86_64-unknown-linux-musl)..."
RUSTC_BOOTSTRAP=1 \
RUSTFLAGS="-C linker=$SCRIPT_DIR/target/musl-gcc-wrapper.sh" \
cargo build --release -Z build-std=std,panic_abort --target x86_64-unknown-linux-musl

if [ ! -f "$BIN_PATH" ]; then
    echo "错误：未找到构建二进制 $BIN_PATH"
    exit 1
fi
echo "二进制编译成功，大小: $(ls -lh "$BIN_PATH" | awk '{print $5}')"

# 4. 准备 apk 打包 rootfs 布局
echo "--> 正在收集文件布局..."
rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR/usr/bin"
mkdir -p "$STAGING_DIR/etc/config"
mkdir -p "$STAGING_DIR/etc/init.d"
mkdir -p "$STAGING_DIR/usr/share/luci/menu.d"
mkdir -p "$STAGING_DIR/usr/share/rpcd/acl.d"
mkdir -p "$STAGING_DIR/www/luci-static/resources/view/sideroute"

# 拷贝二进制
cp "$BIN_PATH" "$STAGING_DIR/usr/bin/side-route-daemon"
chmod 755 "$STAGING_DIR/usr/bin/side-route-daemon"

# 拷贝配置与服务脚本
cp package/luci-app-sideroute/root/etc/config/sideroute "$STAGING_DIR/etc/config/sideroute"
cp package/luci-app-sideroute/root/etc/init.d/sideroute "$STAGING_DIR/etc/init.d/sideroute"
chmod 755 "$STAGING_DIR/etc/init.d/sideroute"

# 拷贝 LuCI 界面资源
cp package/luci-app-sideroute/root/usr/share/luci/menu.d/luci-app-sideroute.json "$STAGING_DIR/usr/share/luci/menu.d/luci-app-sideroute.json"
cp package/luci-app-sideroute/root/usr/share/rpcd/acl.d/luci-app-sideroute.json "$STAGING_DIR/usr/share/rpcd/acl.d/luci-app-sideroute.json"
cp package/luci-app-sideroute/root/www/luci-static/resources/view/sideroute/overview.js "$STAGING_DIR/www/luci-static/resources/view/sideroute/overview.js"

# 5. 打包生成 .apk (OpenWrt v3 apk 格式)
mkdir -p "$OUTPUT_DIR"
APK_FILE="$OUTPUT_DIR/${PKG_NAME}_${PKG_VERSION}_${ARCH}.apk"
rm -f "$APK_FILE"

echo "--> 正在生成 apk 软件包: $APK_FILE"
"$HOST_APK" mkpkg \
    --output "$APK_FILE" \
    --files "$STAGING_DIR" \
    --info "name:$PKG_NAME" \
    --info "version:$PKG_VERSION" \
    --info "arch:$ARCH" \
    --info "description:Side-Route Daemon & LuCI controller (IPv4/IPv6 Dual-Stack & Inbound Protection)" \
    --info "depends:nftables ip-full"

echo "=========================================================="
echo " 成功生成 OpenWrt 25.x 软件包:"
echo " 路径: $APK_FILE"
echo " 大小: $(ls -lh "$APK_FILE" | awk '{print $5}')"
echo ""
echo " 路由器端安装命令:"
echo "   apk add --allow-untrusted $(basename "$APK_FILE")"
echo "=========================================================="
