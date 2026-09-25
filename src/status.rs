use serde::{Deserialize, Serialize};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub online: bool,
    pub side_mac: String,
    pub side_ipv4: Option<String>,
    pub side_ipv6: Option<String>,
    pub active_clients: Vec<String>,
    pub client_count: usize,
    pub last_check_timestamp: u64,
    pub message: String,
}

impl Default for DaemonStatus {
    fn default() -> Self {
        Self {
            running: false,
            online: false,
            side_mac: String::new(),
            side_ipv4: None,
            side_ipv6: None,
            active_clients: Vec::new(),
            client_count: 0,
            last_check_timestamp: current_timestamp(),
            message: "Initializing...".to_string(),
        }
    }
}

impl DaemonStatus {
    pub fn write_to_file(&self, path: &str) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }
}

pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
