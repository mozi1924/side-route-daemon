mod config;
mod health;
mod nftables;
mod routing;
mod status;

use anyhow::Result;
use config::AppConfig;
use health::HealthChecker;
use log::{error, info, warn};
use nftables::NftablesManager;
use routing::RoutingManager;
use status::{current_timestamp, DaemonStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal::unix::{signal, SignalKind};

pub const STATUS_FILE_PATH: &str = "/tmp/sideroute.json";

#[derive(Debug, PartialEq, Eq)]
enum DaemonState {
    Offline,
    Disabled,
    Active {
        side_ipv4: Option<String>,
        side_ipv6: Option<String>,
        mac_signature: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Starting Side-Route Daemon (Dual-Stack Failover & Inbound Protection)...");

    let mut cfg = AppConfig::load();
    let mut health_checker = HealthChecker::new(
        cfg.side_router_mac.clone(),
        cfg.lan_interface.clone(),
        cfg.side_router_ipv4.clone(),
    );
    let mut nftables = NftablesManager::new(cfg.wan_interface.clone(), cfg.mark_id);
    let mut routing = RoutingManager::new(cfg.table_id, cfg.mark_id, cfg.lan_interface.clone());

    info!("Performing initial cleanup...");
    nftables.clear_rules();
    routing.clear_routing();

    let mut daemon_status = DaemonStatus {
        running: true,
        online: false,
        side_mac: cfg.side_router_mac.clone(),
        side_ipv4: None,
        side_ipv6: None,
        active_clients: cfg.client_macs.clone(),
        client_count: cfg.client_macs.len(),
        last_check_timestamp: current_timestamp(),
        message: "Daemon initialized. Starting health checks...".to_string(),
    };
    daemon_status.write_to_file(STATUS_FILE_PATH);

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

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

    while running.load(Ordering::SeqCst) {
        let new_cfg = AppConfig::load();
        cfg = new_cfg;

        let check_interval = Duration::from_secs(cfg.check_interval_secs.max(1));

        if !cfg.enabled {
            if current_state != DaemonState::Disabled {
                info!("Side-route daemon is disabled by configuration. Cleaning up...");
                nftables.clear_rules();
                routing.clear_routing();
                current_state = DaemonState::Disabled;

                daemon_status.online = false;
                daemon_status.side_mac = cfg.side_router_mac.clone();
                daemon_status.side_ipv4 = None;
                daemon_status.side_ipv6 = None;
                daemon_status.active_clients = cfg.client_macs.clone();
                daemon_status.client_count = cfg.client_macs.len();
                daemon_status.last_check_timestamp = current_timestamp();
                daemon_status.message = "Service disabled in config".to_string();
                daemon_status.write_to_file(STATUS_FILE_PATH);
            }
            tokio::time::sleep(check_interval).await;
            continue;
        }

        health_checker.update_params(
            cfg.side_router_mac.clone(),
            cfg.lan_interface.clone(),
            cfg.side_router_ipv4.clone(),
        );
        nftables.update_params(cfg.wan_interface.clone(), cfg.mark_id);
        routing.update_params(cfg.table_id, cfg.mark_id, cfg.lan_interface.clone());

        let client_macs = &cfg.client_macs;
        let mac_sig = format!("{}:{}", client_macs.join(","), cfg.hijack_dns);

        let side_info = health_checker.check(cfg.enable_ipv4, cfg.enable_ipv6);

        match side_info {
            Some(info) => {
                let need_update = match &current_state {
                    DaemonState::Offline | DaemonState::Disabled => true,
                    DaemonState::Active {
                        side_ipv4,
                        side_ipv6,
                        mac_signature,
                    } => {
                        side_ipv4 != &info.ipv4
                            || side_ipv6 != &info.ipv6
                            || mac_signature != &mac_sig
                    }
                };

                if need_update {
                    info!(
                        "Side-router is ONLINE (IPv4: {:?}, IPv6: {:?}). Applying rules for {} clients...",
                        info.ipv4,
                        info.ipv6,
                        client_macs.len()
                    );

                    let mut success = true;

                    if let Err(e) = routing.apply_routing(
                        cfg.enable_ipv4,
                        info.ipv4.as_deref(),
                        cfg.enable_ipv6,
                        info.ipv6.as_deref(),
                    ) {
                        error!("Failed to apply routing: {}", e);
                        success = false;
                    }

                    if let Err(e) = nftables.apply_rules(
                        client_macs,
                        cfg.enable_ipv4,
                        info.ipv4.as_deref(),
                        cfg.enable_ipv6,
                        info.ipv6.as_deref(),
                        cfg.hijack_dns,
                    ) {
                        error!("Failed to apply nftables: {}", e);
                        success = false;
                    }

                    if success {
                        current_state = DaemonState::Active {
                            side_ipv4: info.ipv4.clone(),
                            side_ipv6: info.ipv6.clone(),
                            mac_signature: mac_sig,
                        };
                        info!("Side-route successfully activated and secured.");

                        daemon_status.online = true;
                        daemon_status.side_mac = info.mac.clone();
                        daemon_status.side_ipv4 = info.ipv4.clone();
                        daemon_status.side_ipv6 = info.ipv6.clone();
                        daemon_status.active_clients = client_macs.clone();
                        daemon_status.client_count = client_macs.len();
                        daemon_status.last_check_timestamp = current_timestamp();
                        daemon_status.message = format!(
                            "Active (IPv4: {}, IPv6: {}) - {} clients routed",
                            info.ipv4.as_deref().unwrap_or("none"),
                            info.ipv6.as_deref().unwrap_or("none"),
                            client_macs.len()
                        );
                        daemon_status.write_to_file(STATUS_FILE_PATH);
                    } else {
                        warn!("Rule application encountered errors, falling back to direct connection...");
                        routing.clear_routing();
                        nftables.clear_rules();
                        current_state = DaemonState::Offline;

                        daemon_status.online = false;
                        daemon_status.last_check_timestamp = current_timestamp();
                        daemon_status.message = "Error applying rules; fallbacked to direct".to_string();
                        daemon_status.write_to_file(STATUS_FILE_PATH);
                    }
                } else {
                    daemon_status.last_check_timestamp = current_timestamp();
                    daemon_status.write_to_file(STATUS_FILE_PATH);
                }
            }
            None => {
                if current_state != DaemonState::Offline {
                    warn!(
                        "Side-router [{}] is UNREACHABLE / OFFLINE! Triggering failover...",
                        cfg.side_router_mac
                    );
                    routing.clear_routing();
                    nftables.clear_rules();
                    current_state = DaemonState::Offline;
                    info!("Failover complete. All LAN devices transparently fallback to main router direct connection.");

                    daemon_status.online = false;
                    daemon_status.side_mac = cfg.side_router_mac.clone();
                    daemon_status.side_ipv4 = None;
                    daemon_status.side_ipv6 = None;
                    daemon_status.last_check_timestamp = current_timestamp();
                    daemon_status.message = "Side router unreachable. Fallback direct active.".to_string();
                    daemon_status.write_to_file(STATUS_FILE_PATH);
                } else {
                    daemon_status.last_check_timestamp = current_timestamp();
                    daemon_status.write_to_file(STATUS_FILE_PATH);
                }
            }
        }

        tokio::time::sleep(check_interval).await;
    }

    info!("Shutting down daemon and cleaning up all rules...");
    nftables.clear_rules();
    routing.clear_routing();

    daemon_status.running = false;
    daemon_status.online = false;
    daemon_status.message = "Daemon stopped".to_string();
    daemon_status.last_check_timestamp = current_timestamp();
    daemon_status.write_to_file(STATUS_FILE_PATH);

    info!("Cleanup complete. Goodbye!");
    Ok(())
}
