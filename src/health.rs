use log::{debug, warn};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideRouterInfo {
    pub ipv6: String,
    pub mac: String,
}

pub struct HealthChecker {
    side_mac: String,
    lan_interface: String,
    last_known_ip: Option<String>,
}

impl HealthChecker {
    pub fn new(side_mac: String, lan_interface: String) -> Self {
        Self {
            side_mac: side_mac.to_lowercase(),
            lan_interface,
            last_known_ip: None,
        }
    }

    /// 执行一次探测：返回存活的旁路由 IPv6 地址
    pub fn check(&mut self) -> Option<SideRouterInfo> {
        // 1. 如果有上次成功的 IP，先快速 ping 一次确认是否仍然存活
        if let Some(ref ip) = self.last_known_ip {
            if self.ping_ipv6(ip) {
                return Some(SideRouterInfo {
                    ipv6: ip.clone(),
                    mac: self.side_mac.clone(),
                });
            } else {
                debug!("Last known IP {} is unreachable, rediscovering...", ip);
            }
        }

        // 2. 刷新 NDP 缓存（唤起组播探测）
        let _ = Command::new("ping6")
            .args(["-c", "1", "-W", "1", "-I", &self.lan_interface, "ff02::1"])
            .output();

        // 3. 读取当前邻居表中与目标 MAC 关联的所有 IPv6
        let candidates = self.lookup_candidates();
        if candidates.is_empty() {
            warn!("No IPv6 neighbor entry found for MAC {}", self.side_mac);
            self.last_known_ip = None;
            return None;
        }

        // 4. 排序优先级：
        // 首选 ULA (fd..)，次选公网 GUA (240..)，再次选链路本地 (fe80..)
        let mut sorted = candidates;
        sorted.sort_by_key(|ip| {
            if ip.starts_with("fd") {
                0
            } else if ip.starts_with("24") || ip.starts_with("20") {
                1
            } else if ip.starts_with("fe80") {
                2
            } else {
                3
            }
        });

        // 5. 挨个 ping 测试连通性
        for ip in sorted {
            if self.ping_ipv6(&ip) {
                self.last_known_ip = Some(ip.clone());
                return Some(SideRouterInfo {
                    ipv6: ip,
                    mac: self.side_mac.clone(),
                });
            }
        }

        self.last_known_ip = None;
        None
    }

    fn lookup_candidates(&self) -> Vec<String> {
        let output = match Command::new("ip")
            .args(["-6", "neigh", "show", "dev", &self.lan_interface])
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                warn!("Failed to execute 'ip -6 neigh': {}", e);
                return Vec::new();
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut ips = Vec::new();

        for line in stdout.lines() {
            let lower = line.to_lowercase();
            if lower.contains(&self.side_mac) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if !parts.is_empty() {
                    let ip = parts[0].to_string();
                    ips.push(ip);
                }
            }
        }
        ips
    }

    fn ping_ipv6(&self, ip: &str) -> bool {
        let status = Command::new("ping6")
            .args(["-c", "1", "-W", "1", "-I", &self.lan_interface, ip])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match status {
            Ok(s) => s.success(),
            Err(_) => false,
        }
    }
}
