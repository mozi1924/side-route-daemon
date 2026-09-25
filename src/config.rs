use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub const DEFAULT_UCI_CONFIG_PATH: &str = "/etc/config/sideroute";
pub const DEFAULT_TOML_CONFIG_PATH: &str = "/etc/side-route-daemon/config.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    #[serde(default = "default_side_router_mac")]
    pub side_router_mac: String,

    #[serde(default)]
    pub side_router_ipv4: Option<String>,

    #[serde(default = "default_table_id")]
    pub table_id: u32,

    #[serde(default = "default_mark_id")]
    pub mark_id: u32,

    #[serde(default = "default_wan_interface")]
    pub wan_interface: String,

    #[serde(default = "default_lan_interface")]
    pub lan_interface: String,

    #[serde(default = "default_check_interval_secs")]
    pub check_interval_secs: u64,

    #[serde(default = "default_true")]
    pub hijack_dns: bool,

    #[serde(default = "default_true")]
    pub enable_ipv4: bool,

    #[serde(default = "default_true")]
    pub enable_ipv6: bool,

    #[serde(default)]
    pub client_macs: Vec<String>,
}

fn default_enabled() -> bool {
    true
}
fn default_side_router_mac() -> String {
    "".to_string()
}
fn default_table_id() -> u32 {
    200
}
fn default_mark_id() -> u32 {
    1
}
fn default_wan_interface() -> String {
    "pppoe-wan".to_string()
}
fn default_lan_interface() -> String {
    "br-lan".to_string()
}
fn default_check_interval_secs() -> u64 {
    3
}
fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            side_router_mac: default_side_router_mac(),
            side_router_ipv4: None,
            table_id: default_table_id(),
            mark_id: default_mark_id(),
            wan_interface: default_wan_interface(),
            lan_interface: default_lan_interface(),
            check_interval_secs: default_check_interval_secs(),
            hijack_dns: default_true(),
            enable_ipv4: default_true(),
            enable_ipv6: default_true(),
            client_macs: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        // 1. 优先读取 OpenWrt UCI 配置 /etc/config/sideroute
        if Path::new(DEFAULT_UCI_CONFIG_PATH).exists() {
            if let Ok(content) = fs::read_to_string(DEFAULT_UCI_CONFIG_PATH) {
                if let Some(cfg) = Self::parse_uci(&content) {
                    info!("Loaded configuration from UCI {}", DEFAULT_UCI_CONFIG_PATH);
                    return cfg;
                }
            }
        }

        // 2. 回退读取 TOML 配置
        if Path::new(DEFAULT_TOML_CONFIG_PATH).exists() {
            if let Ok(content) = fs::read_to_string(DEFAULT_TOML_CONFIG_PATH) {
                match toml::from_str::<AppConfig>(&content) {
                    Ok(mut cfg) => {
                        info!("Loaded configuration from TOML {}", DEFAULT_TOML_CONFIG_PATH);
                        cfg.normalize();
                        return cfg;
                    }
                    Err(e) => {
                        warn!("Failed to parse TOML {}: {}", DEFAULT_TOML_CONFIG_PATH, e);
                    }
                }
            }
        }

        info!("Using default configuration");
        let mut cfg = Self::default();
        cfg.normalize();
        cfg
    }

    /// 规范化配置中的 MAC 地址与字段
    pub fn normalize(&mut self) {
        self.side_router_mac = self.side_router_mac.trim().to_lowercase();
        let mut valid_clients = HashSet::new();
        for mac in &self.client_macs {
            let normalized = mac.trim().to_lowercase();
            if is_valid_mac(&normalized) {
                valid_clients.insert(normalized);
            }
        }
        let mut list: Vec<String> = valid_clients.into_iter().collect();
        list.sort();
        self.client_macs = list;
    }

    /// 从 UCI 文件文本中解析配置
    pub fn parse_uci(content: &str) -> Option<Self> {
        let mut cfg = Self::default();
        let mut in_sideroute_section = false;

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with("config") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts[1].trim_matches('\'').trim_matches('"') == "sideroute" {
                    in_sideroute_section = true;
                } else {
                    in_sideroute_section = false;
                }
                continue;
            }

            if !in_sideroute_section {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }

            let key_type = parts[0];
            let key = parts[1];
            let val = parts[2..].join(" ");
            let val = val.trim().trim_matches('\'').trim_matches('"');

            match (key_type, key) {
                ("option", "enabled") => {
                    cfg.enabled = val == "1" || val.eq_ignore_ascii_case("true") || val.eq_ignore_ascii_case("yes");
                }
                ("option", "side_mac") => {
                    cfg.side_router_mac = val.to_string();
                }
                ("option", "side_ipv4") => {
                    let v = val.trim();
                    if !v.is_empty() {
                        cfg.side_router_ipv4 = Some(v.to_string());
                    } else {
                        cfg.side_router_ipv4 = None;
                    }
                }
                ("option", "table_id") => {
                    if let Ok(v) = val.parse::<u32>() {
                        cfg.table_id = v;
                    }
                }
                ("option", "mark_id") => {
                    if let Ok(v) = val.parse::<u32>() {
                        cfg.mark_id = v;
                    }
                }
                ("option", "wan_interface") => {
                    cfg.wan_interface = val.to_string();
                }
                ("option", "lan_interface") => {
                    cfg.lan_interface = val.to_string();
                }
                ("option", "check_interval") => {
                    if let Ok(v) = val.parse::<u64>() {
                        cfg.check_interval_secs = v;
                    }
                }
                ("option", "hijack_dns") => {
                    cfg.hijack_dns = val == "1" || val.eq_ignore_ascii_case("true");
                }
                ("option", "enable_ipv4") => {
                    cfg.enable_ipv4 = val == "1" || val.eq_ignore_ascii_case("true");
                }
                ("option", "enable_ipv6") => {
                    cfg.enable_ipv6 = val == "1" || val.eq_ignore_ascii_case("true");
                }
                ("list", "client_mac") | ("option", "client_mac") => {
                    for item in val.split(|c: char| c == ',' || c.is_whitespace()) {
                        let trimmed = item.trim();
                        if !trimmed.is_empty() {
                            cfg.client_macs.push(trimmed.to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        cfg.normalize();
        Some(cfg)
    }
}

pub fn is_valid_mac(s: &str) -> bool {
    if s.len() != 17 {
        return false;
    }
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 6 {
        return false;
    }
    parts.iter().all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uci_sideroute() {
        let sample = r#"
config sideroute 'global'
    option enabled '1'
    option side_mac 'BC:24:11:84:71:8B'
    option side_ipv4 '192.168.1.2'
    option wan_interface 'pppoe-wan'
    option lan_interface 'br-lan'
    option table_id '200'
    option mark_id '1'
    option check_interval '5'
    option hijack_dns '1'
    option enable_ipv4 '1'
    option enable_ipv6 '1'
    list client_mac '00:11:22:33:44:55'
    list client_mac 'aa:bb:cc:dd:ee:ff'
"#;
        let cfg = AppConfig::parse_uci(sample).expect("parse should succeed");
        assert!(cfg.enabled);
        assert_eq!(cfg.side_router_mac, "bc:24:11:84:71:8b");
        assert_eq!(cfg.side_router_ipv4.as_deref(), Some("192.168.1.2"));
        assert_eq!(cfg.table_id, 200);
        assert_eq!(cfg.check_interval_secs, 5);
        assert!(cfg.hijack_dns);
        assert_eq!(cfg.client_macs.len(), 2);
        assert_eq!(cfg.client_macs[0], "00:11:22:33:44:55");
        assert_eq!(cfg.client_macs[1], "aa:bb:cc:dd:ee:ff");
    }
}
