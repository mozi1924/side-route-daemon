# Side-Route Daemon (OpenWrt IPv6 旁路由高可用守护程序)

专为 OpenWrt (22.03+ / 23.05+ / 24.x / 25.x, fw4/nftables) 设计的 IPv6 旁路由控制守护进程。

## 解决的核心痛点

1. **外网直连入站保护（根治回包被吞）**：
   - 外部通过公网 IPv6 访问内网服务（如 Nextcloud、RustDesk、NAS）时，打上 `0x200` 连接标记。
   - 内网设备回包时精准识别并跳过旁路由标记，原路直接从 WAN 口返回，确保外网直连永不掉线。
2. **旁路由秒级容灾与故障回退（Failover）**：
   - 定时探测旁路由存活状态；
   - 旁路由存活时：自动下发策略路由与 DNS 劫持；
   - 旁路由离线/宕机时：0.1 秒内原子清除所有规则，内网所有设备透明降级为直连主路由，绝不断网。
3. **独立 nftables 表（零污染、零冲突）**：
   - 采用独立表 `table inet side_route_guard`，不向 `/etc/nftables.d/` 生成碎片文件，彻底告别 firewall reload 冲突。
4. **Procd 原生托管**：
   - 遵循 OpenWrt 官方服务标准，崩溃自动拉起，启停干净利落，无僵尸进程。

## 目录结构

- `src/`：Rust 源码
  - `config.rs`：配置读取与 UCI `/etc/config/dhcp` 动态 MAC 解析
  - `health.rs`：旁路由 IPv6 存活探测与动态地址发现
  - `nftables.rs`：nftables 独立表原子规则管理
  - `routing.rs`：Linux 策略路由表管理
  - `main.rs`：状态机主循环与系统信号处理
- `service/`：
  - `side-route-daemon`：OpenWrt procd 服务启动脚本
  - `config.toml`：默认配置文件
- `deploy.sh`：一键编译与部署脚本

## 常用服务管理命令（在 OpenWrt 终端运行）

```bash
# 查看运行状态与日志
logread -f -e side-route-daemon

# 启停服务
/etc/init.d/side-route-daemon start
/etc/init.d/side-route-daemon stop
/etc/init.d/side-route-daemon restart

# 查看当前生效的规则表
nft list table inet side_route_guard
ip -6 rule show | grep 200
ip -6 route show table 200
```
