use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelMode {
    LocalForward,   // Listen on local port, forward to remote
    DynamicSocks5,  // SOCKS5 proxy
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TunnelConfig {
    pub id: String,
    pub name: String,
    pub mode: TunnelMode,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub ssh_server: String,
}

pub struct ActiveTunnel {
    pub config: TunnelConfig,
    pub running: Arc<AtomicBool>,
}

pub struct TunnelManager {
    pub tunnels: Vec<TunnelConfig>,
    pub active_states: Vec<Arc<AtomicBool>>,
}

impl TunnelManager {
    pub fn new() -> Self {
        let default_tunnels = vec![
            TunnelConfig {
                id: "1".into(),
                name: "MySQL Tunnel (Local 3307 -> DB 3306)".into(),
                mode: TunnelMode::LocalForward,
                local_port: 3307,
                remote_host: "127.0.0.1".into(),
                remote_port: 3306,
                ssh_server: "db-bastion.internal".into(),
            },
            TunnelConfig {
                id: "2".into(),
                name: "Web Staging (Local 8088 -> Web 80)".into(),
                mode: TunnelMode::LocalForward,
                local_port: 8088,
                remote_host: "10.0.0.5".into(),
                remote_port: 80,
                ssh_server: "jump.company.com".into(),
            },
        ];

        let mut states = Vec::new();
        for _ in 0..default_tunnels.len() {
            states.push(Arc::new(AtomicBool::new(false)));
        }

        Self {
            tunnels: default_tunnels,
            active_states: states,
        }
    }

    pub fn start_tunnel(&self, idx: usize, runtime: &tokio::runtime::Handle) {
        if idx >= self.tunnels.len() {
            return;
        }

        let tunnel = &self.tunnels[idx];
        let state = Arc::clone(&self.active_states[idx]);
        if state.load(Ordering::Relaxed) {
            return;
        }

        state.store(true, Ordering::Relaxed);
        let local_port = tunnel.local_port;
        let remote_target = format!("{}:{}", tunnel.remote_host, tunnel.remote_port);
        let state_clone = Arc::clone(&state);

        runtime.spawn(async move {
            let bind_addr = format!("127.0.0.1:{}", local_port);
            if let Ok(listener) = TcpListener::bind(&bind_addr).await {
                while state_clone.load(Ordering::Relaxed) {
                    tokio::select! {
                        accept_res = listener.accept() => {
                            if let Ok((mut client_stream, _)) = accept_res {
                                let target = remote_target.clone();
                                tokio::spawn(async move {
                                    if let Ok(mut server_stream) = TcpStream::connect(&target).await {
                                        let _ = copy_bidirectional(&mut client_stream, &mut server_stream).await;
                                    }
                                });
                            }
                        }
                        _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {}
                    }
                }
            } else {
                state_clone.store(false, Ordering::Relaxed);
            }
        });
    }

    pub fn stop_tunnel(&self, idx: usize) {
        if idx < self.active_states.len() {
            self.active_states[idx].store(false, Ordering::Relaxed);
        }
    }
}
