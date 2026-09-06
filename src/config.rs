use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_side_router_mac")]
    pub side_router_mac: String,

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

    #[serde(default = "default_uci_dhcp_path")]
    pub uci_dhcp_path: String,

    #[serde(default)]
    pub static_client_macs: Vec<String>,
}

fn default_side_router_mac() -> String {
    "bc:24:11:84:71:8b".to_string()
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
fn default_uci_dhcp_path() -> String {
    "/etc/config/dhcp".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            side_router_mac: default_side_router_mac(),
            table_id: default_table_id(),
            mark_id: default_mark_id(),
            wan_interface: default_wan_interface(),
            lan_interface: default_lan_interface(),
            check_interval_secs: default_check_interval_secs(),
            uci_dhcp_path: default_uci_dhcp_path(),
            static_client_macs: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let config_path = "/etc/side-route-daemon/config.toml";
        if Path::new(config_path).exists() {
            match fs::read_to_string(config_path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(cfg) => {
                        info!("Loaded configuration from {}", config_path);
                        return cfg;
                    }
                    Err(e) => {
                        warn!("Failed to parse {}: {}, using defaults", config_path, e);
                    }
                },
                Err(e) => {
                    warn!("Failed to read {}: {}, using defaults", config_path, e);
                }
            }
        }
        info!("Using default configuration");
        Self::default()
    }

    /// 从 UCI /etc/config/dhcp 解析标记为 bypass 的所有 MAC 地址
    pub fn get_bypass_macs(&self) -> Vec<String> {
        let mut macs = HashSet::new();

        // 静态配置中的 MAC
        for mac in &self.static_client_macs {
            let normalized = mac.trim().to_lowercase();
            if is_valid_mac(&normalized) {
                macs.insert(normalized);
            }
        }

        // 从 UCI /etc/config/dhcp 读取
        if Path::new(&self.uci_dhcp_path).exists() {
            if let Ok(content) = fs::read_to_string(&self.uci_dhcp_path) {
                for mac in parse_uci_dhcp_bypass(&content) {
                    macs.insert(mac);
                }
            }
        }

        let mut list: Vec<String> = macs.into_iter().collect();
        list.sort();
        list
    }
}

fn is_valid_mac(s: &str) -> bool {
    s.len() == 17 && s.chars().filter(|&c| c == ':').count() == 5
}

/// 解析 OpenWrt UCI /etc/config/dhcp 文件中的 bypass 匹配项
pub fn parse_uci_dhcp_bypass(content: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut in_tag_match = false;
    let mut current_networkid = String::new();
    let mut current_match = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with("config") {
            // 如果上一个块匹配上了 bypass
            if in_tag_match && current_networkid == "bypass" && !current_match.is_empty() {
                if let Some(mac) = extract_mac_from_match(&current_match) {
                    results.push(mac);
                }
            }
            in_tag_match = line.contains("tag") || line.contains("match");
            current_networkid.clear();
            current_match.clear();
            continue;
        }

        if in_tag_match {
            if line.starts_with("option networkid") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    current_networkid = parts[2].trim_matches('\'').trim_matches('"').to_string();
                }
            } else if line.starts_with("option match") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    current_match = parts[2..].join(" ").trim_matches('\'').trim_matches('"').to_string();
                }
            }
        }
    }

    if in_tag_match && current_networkid == "bypass" && !current_match.is_empty() {
        if let Some(mac) = extract_mac_from_match(&current_match) {
            results.push(mac);
        }
    }

    results
}

fn extract_mac_from_match(match_str: &str) -> Option<String> {
    // 形如 "01:bc:24:11:8e:0b:e8" 或 "*:bc:24:11:8e:0b:e8" 或直接 MAC
    let s = match_str.trim();
    if s.len() >= 17 {
        let mac_part = &s[s.len() - 17..];
        let normalized = mac_part.to_lowercase();
        if is_valid_mac(&normalized) {
            return Some(normalized);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uci_dhcp() {
        let uci_sample = r#"
config tag 'match'
    option networkid 'bypass'
    option match '01:bc:24:11:8e:0b:e8'

config tag 'match'
    option networkid 'guest'
    option match '01:aa:bb:cc:dd:ee:ff'

config tag 'match'
    option networkid 'bypass'
    option match '*:22:47:BF:97:77:CC'
"#;
        let macs = parse_uci_dhcp_bypass(uci_sample);
        assert_eq!(macs.len(), 2);
        assert_eq!(macs[0], "bc:24:11:8e:0b:e8");
        assert_eq!(macs[1], "22:47:bf:97:77:cc");
    }
}
