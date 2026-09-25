use anyhow::Result;
use log::{debug, info, warn};
use std::io::Write;
use std::process::Command;

pub const TABLE_NAME: &str = "side_route_guard";
pub const INBOUND_CT_MARK: u32 = 0x200;

pub struct NftablesManager {
    wan_interface: String,
    mark_id: u32,
}

impl NftablesManager {
    pub fn new(wan_interface: String, mark_id: u32) -> Self {
        Self {
            wan_interface,
            mark_id,
        }
    }

    pub fn update_params(&mut self, wan_interface: String, mark_id: u32) {
        self.wan_interface = wan_interface;
        self.mark_id = mark_id;
    }

    pub fn apply_rules(
        &self,
        client_macs: &[String],
        enable_ipv4: bool,
        side_ipv4: Option<&str>,
        enable_ipv6: bool,
        side_ipv6: Option<&str>,
        hijack_dns: bool,
    ) -> Result<()> {
        let mut ruleset = String::new();

        ruleset.push_str(&format!("table inet {} {{\n", TABLE_NAME));

        ruleset.push_str("    chain prerouting {\n");
        ruleset.push_str("        type filter hook prerouting priority mangle - 5; policy accept;\n");

        ruleset.push_str(&format!(
            "        iifname \"{}\" ct state new ct mark set 0x{:x}\n",
            self.wan_interface, INBOUND_CT_MARK
        ));

        ruleset.push_str(&format!(
            "        ct mark 0x{:x} accept\n",
            INBOUND_CT_MARK
        ));

        ruleset.push_str("        ip daddr { 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 224.0.0.0/4, 255.255.255.255 } accept\n");
        ruleset.push_str("        ip6 daddr { fe80::/10, fc00::/7, ff00::/8 } accept\n");

        for mac in client_macs {
            if enable_ipv4 && side_ipv4.is_some() {
                ruleset.push_str(&format!(
                    "        ether saddr {} ip version 4 counter meta mark set {}\n",
                    mac, self.mark_id
                ));
            }
            if enable_ipv6 && side_ipv6.is_some() {
                ruleset.push_str(&format!(
                    "        ether saddr {} ip6 version 6 counter meta mark set {}\n",
                    mac, self.mark_id
                ));
            }
        }
        ruleset.push_str("    }\n");

        ruleset.push_str("    chain dstnat {\n");
        ruleset.push_str("        type nat hook prerouting priority dstnat - 5; policy accept;\n");
        if hijack_dns && !client_macs.is_empty() {
            for mac in client_macs {
                if enable_ipv4 {
                    if let Some(v4) = side_ipv4 {
                        ruleset.push_str(&format!(
                            "        ether saddr {} meta l4proto {{ tcp, udp }} th dport 53 counter dnat ip to {}:53\n",
                            mac, v4
                        ));
                    }
                }
                if enable_ipv6 {
                    if let Some(v6) = side_ipv6 {
                        ruleset.push_str(&format!(
                            "        ether saddr {} meta l4proto {{ tcp, udp }} th dport 53 counter dnat ip6 to [{}]:53\n",
                            mac, v6
                        ));
                    }
                }
            }
        }
        ruleset.push_str("    }\n");

        if hijack_dns && !client_macs.is_empty() {
            ruleset.push_str("    chain postrouting {\n");
            ruleset.push_str("        type nat hook postrouting priority srcnat; policy accept;\n");
            if enable_ipv4 {
                if let Some(v4) = side_ipv4 {
                    ruleset.push_str(&format!(
                        "        ip daddr {} th dport 53 counter masquerade\n",
                        v4
                    ));
                }
            }
            if enable_ipv6 {
                if let Some(v6) = side_ipv6 {
                    ruleset.push_str(&format!(
                        "        ip6 daddr {} th dport 53 counter masquerade\n",
                        v6
                    ));
                }
            }
            ruleset.push_str("    }\n");
        }
        ruleset.push_str("}\n");

        debug!("Applying nftables ruleset:\n{}", ruleset);

        self.clear_rules();

        let mut child = Command::new("nft")
            .arg("-f")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(ruleset.as_bytes())?;
        }

        let output = child.wait_with_output()?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to apply nftables ruleset: {}", err);
            anyhow::bail!("nftables error: {}", err);
        }

        info!(
            "Successfully applied nftables ruleset ({} clients, IPv4: {:?}, IPv6: {:?})",
            client_macs.len(),
            side_ipv4,
            side_ipv6
        );

        Ok(())
    }

    pub fn clear_rules(&self) {
        let _ = Command::new("nft")
            .args(["delete", "table", "inet", TABLE_NAME])
            .output();
        debug!("Cleared nftables table inet {}", TABLE_NAME);
    }
}
