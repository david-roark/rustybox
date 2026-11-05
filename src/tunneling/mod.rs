use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use parking_lot::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TunnelMode {
    LocalForward,   // Listen on local port, forward to remote target (-L)
    RemoteForward,  // Listen on remote port, forward to local machine (-R)
    DynamicSocks5,  // SOCKS5 dynamic proxy (-D)
}

impl TunnelMode {
    pub fn label(&self) -> &'static str {
        match self {
            TunnelMode::LocalForward => "Local Forward (-L)",
            TunnelMode::RemoteForward => "Remote Forward (-R)",
            TunnelMode::DynamicSocks5 => "Dynamic SOCKS5 (-D)",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            TunnelMode::LocalForward => "-L",
            TunnelMode::RemoteForward => "-R",
            TunnelMode::DynamicSocks5 => "-D",
        }
    }
}

fn default_ssh_port() -> u16 {
    22
}

fn default_ssh_user() -> String {
    "root".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TunnelConfig {
    pub id: String,
    pub name: String,
    pub mode: TunnelMode,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub ssh_server: String,
    #[serde(default = "default_ssh_port")]
    pub ssh_port: u16,
    #[serde(default = "default_ssh_user")]
    pub ssh_user: String,
    #[serde(default)]
    pub ssh_key_path: Option<String>,
}

pub fn default_tunnels() -> Vec<TunnelConfig> {
    vec![
        TunnelConfig {
            id: "1".into(),
            name: "MySQL Database Tunnel".into(),
            mode: TunnelMode::LocalForward,
            local_port: 3307,
            remote_host: "127.0.0.1".into(),
            remote_port: 3306,
            ssh_server: "db-bastion.internal".into(),
            ssh_port: 22,
            ssh_user: "root".into(),
            ssh_key_path: None,
        },
        TunnelConfig {
            id: "2".into(),
            name: "Web Staging Preview".into(),
            mode: TunnelMode::LocalForward,
            local_port: 8088,
            remote_host: "10.0.0.5".into(),
            remote_port: 80,
            ssh_server: "jump.company.com".into(),
            ssh_port: 22,
            ssh_user: "ubuntu".into(),
            ssh_key_path: None,
        },
        TunnelConfig {
            id: "3".into(),
            name: "SOCKS5 Proxy Tunnel".into(),
            mode: TunnelMode::DynamicSocks5,
            local_port: 1080,
            remote_host: "127.0.0.1".into(),
            remote_port: 1080,
            ssh_server: "proxy-gateway.internal".into(),
            ssh_port: 22,
            ssh_user: "root".into(),
            ssh_key_path: None,
        },
    ]
}

pub struct TunnelManager {
    pub tunnels: Vec<TunnelConfig>,
    pub active_states: Vec<Arc<AtomicBool>>,
    pub child_pids: Vec<Arc<Mutex<Option<u32>>>>,
    pub status_messages: Vec<String>,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self::with_tunnels(default_tunnels())
    }

    pub fn with_tunnels(tunnels: Vec<TunnelConfig>) -> Self {
        let mut states = Vec::with_capacity(tunnels.len());
        let mut pids = Vec::with_capacity(tunnels.len());
        let mut msgs = Vec::with_capacity(tunnels.len());
        for _ in 0..tunnels.len() {
            states.push(Arc::new(AtomicBool::new(false)));
            pids.push(Arc::new(Mutex::new(None)));
            msgs.push("Stopped".to_string());
        }

        Self {
            tunnels,
            active_states: states,
            child_pids: pids,
            status_messages: msgs,
        }
    }

    pub fn is_running(&self, idx: usize) -> bool {
        if idx < self.active_states.len() {
            self.active_states[idx].load(Ordering::Relaxed)
        } else {
            false
        }
    }

    pub fn start_tunnel(&mut self, idx: usize, runtime: &tokio::runtime::Handle) {
        if idx >= self.tunnels.len() {
            return;
        }

        let tunnel = self.tunnels[idx].clone();
        let state = Arc::clone(&self.active_states[idx]);
        if state.load(Ordering::Relaxed) {
            return;
        }

        state.store(true, Ordering::Relaxed);
        let pid_slot = Arc::clone(&self.child_pids[idx]);

        // If SSH gateway is configured, use OpenSSH CLI (-N -L/-R/-D) for production grade tunneling
        if !tunnel.ssh_server.trim().is_empty() && tunnel.ssh_server != "127.0.0.1" && tunnel.ssh_server != "localhost" {
            let mut cmd = std::process::Command::new("ssh");
            cmd.arg("-N"); // Do not execute remote shell command
            cmd.arg("-o").arg("ExitOnForwardFailure=yes");
            cmd.arg("-o").arg("ServerAliveInterval=15");
            cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");

            if tunnel.ssh_port != 22 && tunnel.ssh_port != 0 {
                cmd.arg("-p").arg(tunnel.ssh_port.to_string());
            }

            if let Some(ref key) = tunnel.ssh_key_path {
                let trimmed = key.trim();
                if !trimmed.is_empty() {
                    cmd.arg("-i").arg(trimmed);
                }
            }

            match tunnel.mode {
                TunnelMode::LocalForward => {
                    let forward_spec = format!("{}:{}:{}", tunnel.local_port, tunnel.remote_host, tunnel.remote_port);
                    cmd.arg("-L").arg(&forward_spec);
                }
                TunnelMode::RemoteForward => {
                    let forward_spec = format!("{}:{}:{}", tunnel.remote_port, tunnel.remote_host, tunnel.local_port);
                    cmd.arg("-R").arg(&forward_spec);
                }
                TunnelMode::DynamicSocks5 => {
                    cmd.arg("-D").arg(tunnel.local_port.to_string());
                }
            }

            let destination = if tunnel.ssh_user.trim().is_empty() {
                tunnel.ssh_server.clone()
            } else {
                format!("{}@{}", tunnel.ssh_user.trim(), tunnel.ssh_server.trim())
            };
            cmd.arg(&destination);

            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                // CREATE_NO_WINDOW = 0x08000000 to prevent console flashing
                cmd.creation_flags(0x08000000);
            }

            match cmd.spawn() {
                Ok(child) => {
                    *pid_slot.lock() = Some(child.id());
                    self.status_messages[idx] = format!("Running (PID {})", child.id());
                    return;
                }
                Err(e) => {
                    self.status_messages[idx] = format!("SSH launch failed: {}", e);
                    // Fall back to Tokio direct TCP forwarder if local forward
                }
            }
        }

        // Tokio fallback for LocalForward
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

        self.status_messages[idx] = format!("Active on :{}", local_port);
    }

    pub fn stop_tunnel(&mut self, idx: usize) {
        if idx >= self.active_states.len() {
            return;
        }

        self.active_states[idx].store(false, Ordering::Relaxed);
        let pid_opt = self.child_pids[idx].lock().take();

        if let Some(pid) = pid_opt {
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/PID", &pid.to_string()])
                    .spawn();
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .spawn();
            }
        }

        self.status_messages[idx] = "Stopped".to_string();
    }

    pub fn add_tunnel(&mut self, config: TunnelConfig) {
        self.tunnels.push(config);
        self.active_states.push(Arc::new(AtomicBool::new(false)));
        self.child_pids.push(Arc::new(Mutex::new(None)));
        self.status_messages.push("Stopped".to_string());
    }

    pub fn update_tunnel(&mut self, idx: usize, config: TunnelConfig) {
        if idx < self.tunnels.len() {
            if self.is_running(idx) {
                self.stop_tunnel(idx);
            }
            self.tunnels[idx] = config;
        }
    }

    pub fn remove_tunnel(&mut self, idx: usize) {
        if idx < self.tunnels.len() {
            self.stop_tunnel(idx);
            self.tunnels.remove(idx);
            self.active_states.remove(idx);
            self.child_pids.remove(idx);
            self.status_messages.remove(idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_manager_lifecycle_and_crud() {
        let mut manager = TunnelManager::new();
        assert_eq!(manager.tunnels.len(), 3);
        assert!(!manager.is_running(0));

        let new_t = TunnelConfig {
            id: "test_t".into(),
            name: "Redis Tunnel".into(),
            mode: TunnelMode::LocalForward,
            local_port: 6379,
            remote_host: "127.0.0.1".into(),
            remote_port: 6379,
            ssh_server: "redis.internal".into(),
            ssh_port: 22,
            ssh_user: "root".into(),
            ssh_key_path: None,
        };

        manager.add_tunnel(new_t.clone());
        assert_eq!(manager.tunnels.len(), 4);
        assert_eq!(manager.tunnels[3].name, "Redis Tunnel");

        manager.remove_tunnel(3);
        assert_eq!(manager.tunnels.len(), 3);
    }
}
