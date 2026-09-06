mod config;
mod health;
mod nftables;
mod routing;

use anyhow::Result;
use config::AppConfig;
use health::HealthChecker;
use log::{error, info, warn};
use nftables::NftablesManager;
use routing::RoutingManager;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};

#[derive(Debug, PartialEq, Eq)]
enum DaemonState {
    Offline,
    Active {
        side_ip: String,
        mac_signature: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Starting Side-Route Daemon (IPv6 Failover & Inbound Protection)...");

    let cfg = AppConfig::load();
    let table_id = cfg.table_id;
    let mark_id = cfg.mark_id;
    let wan_if = cfg.wan_interface.clone();
    let lan_if = cfg.lan_interface.clone();
    let side_mac = cfg.side_router_mac.clone();
    let check_interval = Duration::from_secs(cfg.check_interval_secs);

    let mut health_checker = HealthChecker::new(side_mac.clone(), lan_if.clone());
    let nftables = NftablesManager::new(wan_if.clone(), mark_id);
    let routing = RoutingManager::new(table_id, mark_id, lan_if.clone());

    // 初始清理，确保环境干净
    info!("Performing initial cleanup...");
    nftables.clear_rules();
    routing.clear_routing();

    // 优雅退出标志
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // 监听退出信号
    tokio::spawn(async move {
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to register SIGTERM handler: {}", e);
                return;
            }
        };
        let mut sigint = match signal(SignalKind::interrupt()) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to register SIGINT handler: {}", e);
                return;
            }
        };

        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM, preparing to shutdown...");
            }
            _ = sigint.recv() => {
                info!("Received SIGINT, preparing to shutdown...");
            }
        }
        r.store(false, Ordering::SeqCst);
    });

    let mut current_state = DaemonState::Offline;

    info!(
        "Daemon initialized. Monitoring side-router [{}] every {}s...",
        side_mac, cfg.check_interval_secs
    );

    while running.load(Ordering::SeqCst) {
        let bypass_macs = cfg.get_bypass_macs();
        let mac_sig = bypass_macs.join(",");

        let side_info = health_checker.check();

        match side_info {
            Some(info) => {
                let need_update = match &current_state {
                    DaemonState::Offline => true,
                    DaemonState::Active { side_ip, mac_signature } => {
                        side_ip != &info.ipv6 || mac_signature != &mac_sig
                    }
                };

                if need_update {
                    info!(
                        "Side-router is ONLINE (IP: [{}]). Applying rules for {} clients...",
                        info.ipv6, bypass_macs.len()
                    );

                    let mut success = true;
                    if let Err(e) = routing.apply_routing(&info.ipv6) {
                        error!("Failed to apply routing: {}", e);
                        success = false;
                    }

                    if let Err(e) = nftables.apply_rules(&bypass_macs, &info.ipv6) {
                        error!("Failed to apply nftables: {}", e);
                        success = false;
                    }

                    if success {
                        current_state = DaemonState::Active {
                            side_ip: info.ipv6,
                            mac_signature: mac_sig,
                        };
                        info!("Side-route successfully activated and secured.");
                    } else {
                        warn!("Rule application encountered errors, falling back to direct...");
                        routing.clear_routing();
                        nftables.clear_rules();
                        current_state = DaemonState::Offline;
                    }
                }
            }
            None => {
                if current_state != DaemonState::Offline {
                    warn!("Side-router [{}] is UNREACHABLE / OFFLINE! Triggering failover...", side_mac);
                    routing.clear_routing();
                    nftables.clear_rules();
                    current_state = DaemonState::Offline;
                    info!("Failover complete. All LAN devices transparently fallback to main router direct connection.");
                }
            }
        }

        tokio::time::sleep(check_interval).await;
    }

    info!("Shutting down daemon and cleaning up all rules...");
    nftables.clear_rules();
    routing.clear_routing();
    info!("Cleanup complete. Goodbye!");

    Ok(())
}
