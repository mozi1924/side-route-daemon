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

    /// 应用独立表规则：包含外网入站连接跟踪保护与旁路由分流
    pub fn apply_rules(
        &self,
        client_macs: &[String],
        side_router_ipv6: &str,
    ) -> Result<()> {
        let mut ruleset = String::new();

        // 建立独立的 inet table
        ruleset.push_str(&format!("table inet {} {{\n", TABLE_NAME));

        // 1. mangle prerouting chain: 用于流量打标记
        ruleset.push_str("    chain prerouting {\n");
        ruleset.push_str("        type filter hook prerouting priority mangle - 5; policy accept;\n");

        // (a) 来自 WAN 的新连接，打上 0x200 的连接标记
        ruleset.push_str(&format!(
            "        iifname \"{}\" ct state new ct mark set 0x{:x}\n",
            self.wan_interface, INBOUND_CT_MARK
        ));

        // (b) 凡是属于外网入站连接的数据包（回包），直接 accept，跳过后续所有打标！
        ruleset.push_str(&format!(
            "        ct mark 0x{:x} accept\n",
            INBOUND_CT_MARK
        ));

        // (c) 客户端主动外发流量：如果属于旁路由白名单 MAC，打上策略路由标记
        for mac in client_macs {
            ruleset.push_str(&format!(
                "        ether saddr {} counter meta mark set {}\n",
                mac, self.mark_id
            ));
        }
        ruleset.push_str("    }\n");

        // 2. dstnat chain: 用于 DNS (53) 劫持到旁路由
        ruleset.push_str("    chain dstnat {\n");
        ruleset.push_str("        type nat hook prerouting priority dstnat - 5; policy accept;\n");
        for mac in client_macs {
            ruleset.push_str(&format!(
                "        ether saddr {} udp dport 53 counter dnat ip6 to [{}]:53\n",
                mac, side_router_ipv6
            ));
            ruleset.push_str(&format!(
                "        ether saddr {} tcp dport 53 counter dnat ip6 to [{}]:53\n",
                mac, side_router_ipv6
            ));
        }
        ruleset.push_str("    }\n");
        ruleset.push_str("}\n");

        debug!("Applying nftables ruleset:\n{}", ruleset);

        // 先清理可能存在的旧表
        self.clear_rules();

        // 原子化载入 ruleset
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
            "Successfully applied nftables ruleset ({} clients -> [{}])",
            client_macs.len(),
            side_router_ipv6
        );
        Ok(())
    }

    /// 清空独立表（容灾降级或退出时调用）
    pub fn clear_rules(&self) {
        let _ = Command::new("nft")
            .args(["delete", "table", "inet", TABLE_NAME])
            .output();
        debug!("Cleared nftables table inet {}", TABLE_NAME);
    }
}
