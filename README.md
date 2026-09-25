# luci-app-sideroute / Side-Route Daemon

专为 **OpenWrt (24.10 / 25.x, fw4/nftables)** 设计的高可用双栈旁路由守护程序与原生 LuCI 控制台。

针对 OpenWrt 25.12.5 原生打包支持（采用最新的 `.apk` 包管理器与现代 LuCI JavaScript 纯前端架构）。

---

## 🌟 解决的核心痛点

1. **防火墙放行设备保护（根治公网入站与端口转发回包被吞）**：
   - 外部通过公网 IPv6 访问内网设备服务（如 Nextcloud、NAS、RustDesk），或通过主路由 IPv4 端口转发（DNAT / DMZ）进入内网时，在连接跟踪层自动打上 `0x200` 专用连接标记。
   - 设备回包时识别连接标记跳过旁路由规则，原路直接从 WAN 口返回，确保端口放行服务与公网访问永不掉线。
2. **局域网互访直连保护（规避回环与主路由失联）**：
   - 内网设备相互访问、访问主路由管理后台、打印机等私有网段（`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `fe80::/10`, `fc00::/7` 等），自动保持走主路由 main 路由表，绝不转发至旁路由。
3. **IPv4 + IPv6 双栈旁路由支持**：
   - 支持通过旁路由物理网卡 MAC 地址自动从内核 ARP/NDP 表发现其 IPv4 与 IPv6 地址；亦支持在 Web 界面固定指定旁路由 IPv4。
   - 支持单独开关 IPv4 分流、IPv6 分流以及 DNS（端口 53）劫持。
4. **旁路由秒级容灾与故障回退（Failover）**：
   - 定时双栈探测旁路由健康状态；
   - 旁路由存活时：自动下发策略路由、nftables 分流规则与 DNS 劫持；
   - 旁路由宕机/离线时：秒级自动原子清空所有引流规则，内网所有设备透明降级为直连主路由上网，绝不断网。
5. **嵌入原生 LuCI 界面（OpenWrt 25.x 现代前端）**：
   - 位于 **[网络] -> [Side Route]**；
   - 实时动态监控卡片：展示旁路由存活状态、当前绑定的 IPv4/IPv6、受保护客户端设备数；
   - 设备快速选择：自动获取局域网已知主机名、IP 和 MAC，支持下拉点击勾选需分流的设备，无需手动敲命令配置。
6. **适配 OpenWrt 最新 `.apk` 软件包标准**：
   - 从传统的单二进制手动上传升级为标准 OpenWrt v3 apk 软件包，支持在路由器终端执行 `apk add` 一键安装升级。

---

## 📁 目录结构

- `src/`：Rust 源码
  - `config.rs`：配置读取与 UCI `/etc/config/sideroute` 解析（支持配置热重载）
  - `health.rs`：旁路由 IPv4 (ARP) 与 IPv6 (NDP) 动态发现与健康探测
  - `nftables.rs`：独立表 `table inet side_route_guard` 原子规则与 WAN 入站保护
  - `routing.rs`：双栈 Linux 策略路由（table 200 与私网直连保护）
  - `status.rs`：守护进程运行状态上报（供 LuCI 界面异步轮询展示）
  - `main.rs`：状态机主循环与系统信号处理
- `package/luci-app-sideroute/`：OpenWrt 标准软件包结构
  - `Makefile`：官方 buildroot 构建描述文件
  - `root/etc/config/sideroute`：默认 UCI 配置文件
  - `root/etc/init.d/sideroute`：Procd 服务托管脚本
  - `root/usr/share/luci/menu.d/`：LuCI 菜单入口配置
  - `root/usr/share/rpcd/acl.d/`：RPCD ACL 权限声明
  - `root/www/luci-static/resources/view/sideroute/overview.js`：现代 JavaScript LuCI 视图
- `build_apk.sh`：自动编译静态二进制并打包为 OpenWrt 25.x `.apk` 软件包
- `deploy.sh`：一键编译打包并推送安装到目标路由器

---

## 🛠️ 构建与打包

执行以下命令即可一键完成纯静态编译与 apk 打包：

```bash
./build_apk.sh
```

构建完成后产物位于：
`dist/luci-app-sideroute_1.0.0-r1_x86_64.apk`（体积仅约 800KB，包含纯静态 Musl 二进制与全套 LuCI 前端）。

---

## 🚀 安装与部署

### 方式一：一键脚本推送到路由器

```bash
./deploy.sh root@192.168.1.1
```

### 方式二：手动在 OpenWrt 25.x 终端安装

1. 上传 `luci-app-sideroute_1.0.0-r1_x86_64.apk` 至路由器 `/tmp`；
2. 在路由器终端运行：
   ```bash
   apk add --allow-untrusted /tmp/luci-app-sideroute_1.0.0-r1_x86_64.apk
   ```
3. 刷新浏览器并登录 OpenWrt Web 后台，在 **[网络] -> [Side Route]** 即可直接使用。
