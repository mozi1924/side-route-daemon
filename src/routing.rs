use anyhow::Result;
use log::{debug, info};
use std::process::Command;

pub struct RoutingManager {
    table_id: u32,
    mark_id: u32,
    lan_interface: String,
}

impl RoutingManager {
    pub fn new(table_id: u32, mark_id: u32, lan_interface: String) -> Self {
        Self {
            table_id,
            mark_id,
            lan_interface,
        }
    }

    /// 应用策略路由
    pub fn apply_routing(&self, side_router_ipv6: &str) -> Result<()> {
        // 1. 确保局域网本地地址优先走 main 路由表
        self.exec_quiet(&["-6", "rule", "add", "to", "fe80::/10", "table", "main", "pref", "9998"]);
        self.exec_quiet(&["-6", "rule", "add", "to", "fdb0::/8", "table", "main", "pref", "9999"]);
        self.exec_quiet(&["-6", "rule", "add", "to", "fdc3::/8", "table", "main", "pref", "9999"]);

        // 2. 挂载 fwmark 路由规则（先删再加防重复）
        self.exec_quiet(&["-6", "rule", "del", "fwmark", &self.mark_id.to_string(), "table", &self.table_id.to_string()]);
        self.exec(&["-6", "rule", "add", "fwmark", &self.mark_id.to_string(), "table", &self.table_id.to_string(), "pref", "10000"])?;

        // 3. 刷新 table 200 并添加默认路由
        self.exec_quiet(&["-6", "route", "flush", "table", &self.table_id.to_string()]);
        self.exec(&[
            "-6", "route", "add", "default", "via", side_router_ipv6, "dev", &self.lan_interface, "table", &self.table_id.to_string()
        ])?;

        info!(
            "Applied policy routing (table {}, default via {} dev {})",
            self.table_id, side_router_ipv6, self.lan_interface
        );
        Ok(())
    }

    /// 清空策略路由（旁路由离线或退出时降级直连）
    pub fn clear_routing(&self) {
        self.exec_quiet(&["-6", "rule", "del", "fwmark", &self.mark_id.to_string(), "table", &self.table_id.to_string()]);
        self.exec_quiet(&["-6", "route", "flush", "table", &self.table_id.to_string()]);
        self.exec_quiet(&["-6", "rule", "del", "to", "fe80::/10", "table", "main", "pref", "9998"]);
        self.exec_quiet(&["-6", "rule", "del", "to", "fdb0::/8", "table", "main", "pref", "9999"]);
        self.exec_quiet(&["-6", "rule", "del", "to", "fdc3::/8", "table", "main", "pref", "9999"]);
        debug!("Cleared routing table {} and related rules", self.table_id);
    }

    fn exec(&self, args: &[&str]) -> Result<()> {
        let output = Command::new("ip").args(args).output()?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("ip command failed: {}", err);
        }
        Ok(())
    }

    fn exec_quiet(&self, args: &[&str]) {
        let _ = Command::new("ip").args(args).output();
    }
}
