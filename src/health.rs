use log::{debug, warn};
use std::fs;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideRouterInfo {
    pub mac: String,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
}

pub struct HealthChecker {
    side_mac: String,
    lan_interface: String,
    configured_ipv4: Option<String>,
    last_known_ipv4: Option<String>,
    last_known_ipv6: Option<String>,
}

impl HealthChecker {
    pub fn new(
        side_mac: String,
        lan_interface: String,
        configured_ipv4: Option<String>,
    ) -> Self {
        Self {
            side_mac: side_mac.to_lowercase(),
            lan_interface,
            configured_ipv4,
            last_known_ipv4: None,
            last_known_ipv6: None,
        }
    }

    pub fn update_params(
        &mut self,
        side_mac: String,
        lan_interface: String,
        configured_ipv4: Option<String>,
    ) {
        let mac_lower = side_mac.to_lowercase();
        if self.side_mac != mac_lower {
            self.side_mac = mac_lower;
            self.last_known_ipv4 = None;
            self.last_known_ipv6 = None;
        }
        self.lan_interface = lan_interface;
        self.configured_ipv4 = configured_ipv4;
    }

    pub fn check(&mut self, enable_v4: bool, enable_v6: bool) -> Option<SideRouterInfo> {
        if self.side_mac.is_empty() {
            return None;
        }

        let mut current_v4 = None;
        let mut current_v6 = None;

        if enable_v4 {
            current_v4 = self.check_ipv4();
        }

        if enable_v6 {
            current_v6 = self.check_ipv6();
        }

        if (enable_v4 && current_v4.is_some()) || (enable_v6 && current_v6.is_some()) {
            Some(SideRouterInfo {
                mac: self.side_mac.clone(),
                ipv4: current_v4,
                ipv6: current_v6,
            })
        } else {
            None
        }
    }

    fn check_ipv4(&mut self) -> Option<String> {
        if let Some(ref static_ip) = self.configured_ipv4 {
            if self.ping_ipv4(static_ip) {
                self.last_known_ipv4 = Some(static_ip.clone());
                return Some(static_ip.clone());
            } else {
                debug!("Configured static IPv4 {} is unreachable", static_ip);
                return None;
            }
        }

        if let Some(ref ip) = self.last_known_ipv4 {
            if self.ping_ipv4(ip) {
                return Some(ip.clone());
            }
        }

        let candidates = self.lookup_candidates_ipv4();
        for ip in candidates {
            if self.ping_ipv4(&ip) {
                self.last_known_ipv4 = Some(ip.clone());
                return Some(ip);
            }
        }

        self.last_known_ipv4 = None;
        None
    }

    fn lookup_candidates_ipv4(&self) -> Vec<String> {
        let mut ips = Vec::new();

        if let Ok(output) = Command::new("ip")
            .args(["-4", "neigh", "show", "dev", &self.lan_interface])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let lower = line.to_lowercase();
                if lower.contains(&self.side_mac) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if !parts.is_empty() {
                        ips.push(parts[0].to_string());
                    }
                }
            }
        }

        if ips.is_empty() {
            if let Ok(arp_content) = fs::read_to_string("/proc/net/arp") {
                for line in arp_content.lines().skip(1) {
                    let lower = line.to_lowercase();
                    if lower.contains(&self.side_mac) {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if !parts.is_empty() {
                            ips.push(parts[0].to_string());
                        }
                    }
                }
            }
        }

        ips
    }

    fn ping_ipv4(&self, ip: &str) -> bool {
        let status = Command::new("ping")
            .args(["-c", "1", "-W", "1", "-I", &self.lan_interface, ip])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match status {
            Ok(s) => s.success(),
            Err(_) => false,
        }
    }

    fn check_ipv6(&mut self) -> Option<String> {
        if let Some(ref ip) = self.last_known_ipv6 {
            if self.ping_ipv6(ip) {
                return Some(ip.clone());
            } else {
                debug!("Last known IPv6 {} is unreachable, rediscovering...", ip);
            }
        }

        let _ = Command::new("ping6")
            .args(["-c", "1", "-W", "1", "-I", &self.lan_interface, "ff02::1"])
            .output();

        let candidates = self.lookup_candidates_ipv6();
        if candidates.is_empty() {
            self.last_known_ipv6 = None;
            return None;
        }

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

        for ip in sorted {
            if self.ping_ipv6(&ip) {
                self.last_known_ipv6 = Some(ip.clone());
                return Some(ip);
            }
        }

        self.last_known_ipv6 = None;
        None
    }

    fn lookup_candidates_ipv6(&self) -> Vec<String> {
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
