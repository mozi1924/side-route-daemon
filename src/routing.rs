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

    pub fn update_params(&mut self, table_id: u32, mark_id: u32, lan_interface: String) {
        self.table_id = table_id;
        self.mark_id = mark_id;
        self.lan_interface = lan_interface;
    }

    pub fn apply_routing(
        &self,
        enable_ipv4: bool,
        side_ipv4: Option<&str>,
        enable_ipv6: bool,
        side_ipv6: Option<&str>,
    ) -> Result<()> {
        let table_str = self.table_id.to_string();
        let mark_str = self.mark_id.to_string();

        // Disable ICMP send_redirects on LAN interface to prevent ICMP redirect storm to clients
        self.disable_send_redirects();

        if enable_ipv4 {
            if let Some(v4) = side_ipv4 {
                self.exec_quiet(&["-4", "rule", "add", "to", "10.0.0.0/8", "table", "main", "pref", "9990"]);
                self.exec_quiet(&["-4", "rule", "add", "to", "172.16.0.0/12", "table", "main", "pref", "9991"]);
                self.exec_quiet(&["-4", "rule", "add", "to", "192.168.0.0/16", "table", "main", "pref", "9992"]);
                self.exec_quiet(&["-4", "rule", "add", "to", "224.0.0.0/4", "table", "main", "pref", "9993"]);

                self.exec_quiet(&["-4", "rule", "del", "fwmark", &mark_str, "table", &table_str]);
                self.exec(&["-4", "rule", "add", "fwmark", &mark_str, "table", &table_str, "pref", "10000"])?;

                self.exec_quiet(&["-4", "route", "flush", "table", &table_str]);
                self.exec(&[
                    "-4", "route", "add", "default", "via", v4, "dev", &self.lan_interface, "table", &table_str
                ])?;

                info!(
                    "Applied IPv4 policy routing (table {}, default via {} dev {})",
                    self.table_id, v4, self.lan_interface
                );
            }
        } else {
            self.clear_ipv4_routing();
        }

        if enable_ipv6 {
            if let Some(v6) = side_ipv6 {
                self.exec_quiet(&["-6", "rule", "add", "to", "fe80::/10", "table", "main", "pref", "9990"]);
                self.exec_quiet(&["-6", "rule", "add", "to", "fc00::/7", "table", "main", "pref", "9991"]);
                self.exec_quiet(&["-6", "rule", "add", "to", "ff00::/8", "table", "main", "pref", "9992"]);

                self.exec_quiet(&["-6", "rule", "del", "fwmark", &mark_str, "table", &table_str]);
                self.exec(&["-6", "rule", "add", "fwmark", &mark_str, "table", &table_str, "pref", "10000"])?;

                self.exec_quiet(&["-6", "route", "flush", "table", &table_str]);
                self.exec(&[
                    "-6", "route", "add", "default", "via", v6, "dev", &self.lan_interface, "table", &table_str
                ])?;

                info!(
                    "Applied IPv6 policy routing (table {}, default via {} dev {})",
                    self.table_id, v6, self.lan_interface
                );
            }
        } else {
            self.clear_ipv6_routing();
        }

        Ok(())
    }

    pub fn clear_routing(&self) {
        self.clear_ipv4_routing();
        self.clear_ipv6_routing();
        debug!("Cleared all routing tables and rules (table {})", self.table_id);
    }

    fn clear_ipv4_routing(&self) {
        let table_str = self.table_id.to_string();
        let mark_str = self.mark_id.to_string();
        self.exec_quiet(&["-4", "rule", "del", "fwmark", &mark_str, "table", &table_str]);
        self.exec_quiet(&["-4", "route", "flush", "table", &table_str]);
        self.exec_quiet(&["-4", "rule", "del", "to", "10.0.0.0/8", "table", "main", "pref", "9990"]);
        self.exec_quiet(&["-4", "rule", "del", "to", "172.16.0.0/12", "table", "main", "pref", "9991"]);
        self.exec_quiet(&["-4", "rule", "del", "to", "192.168.0.0/16", "table", "main", "pref", "9992"]);
        self.exec_quiet(&["-4", "rule", "del", "to", "224.0.0.0/4", "table", "main", "pref", "9993"]);
    }

    fn clear_ipv6_routing(&self) {
        let table_str = self.table_id.to_string();
        let mark_str = self.mark_id.to_string();
        self.exec_quiet(&["-6", "rule", "del", "fwmark", &mark_str, "table", &table_str]);
        self.exec_quiet(&["-6", "route", "flush", "table", &table_str]);
        self.exec_quiet(&["-6", "rule", "del", "to", "fe80::/10", "table", "main", "pref", "9990"]);
        self.exec_quiet(&["-6", "rule", "del", "to", "fc00::/7", "table", "main", "pref", "9991"]);
        self.exec_quiet(&["-6", "rule", "del", "to", "ff00::/8", "table", "main", "pref", "9992"]);
    }

    fn disable_send_redirects(&self) {
        let paths = [
            "/proc/sys/net/ipv4/conf/all/send_redirects",
            "/proc/sys/net/ipv4/conf/default/send_redirects",
        ];
        for path in &paths {
            let _ = std::fs::write(path, "0");
        }
        let lan_path = format!("/proc/sys/net/ipv4/conf/{}/send_redirects", self.lan_interface);
        let _ = std::fs::write(&lan_path, "0");
        debug!("Disabled send_redirects on all, default, and {}", self.lan_interface);
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
