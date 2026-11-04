use serde::{Deserialize, Serialize};

pub mod import_export;
pub mod storage;
pub mod ui;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMethod {
    Password,
    PrivateKey,
    Agent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionType {
    LocalShell {
        custom_shell: Option<String>,
        working_dir: Option<String>,
    },
    Ssh {
        host: String,
        port: u16,
        user: String,
        auth_method: AuthMethod,
        key_path: Option<String>,
        jump_host: Option<String>,
        tunnel_x11: bool,
    },
    Serial {
        port_name: String,
        baud_rate: u32,
    },
    Telnet {
        host: String,
        port: u16,
    },
    Rdp {
        host: String,
        port: u16,
        user: String,
    },
    Vnc {
        host: String,
        port: u16,
    },
    Ftp {
        host: String,
        port: u16,
        user: String,
    },
    Wsl {
        distro: String,
        user: Option<String>,
        working_dir: Option<String>,
    },
}

impl SessionType {
    pub fn is_remote(&self) -> bool {
        matches!(
            self,
            SessionType::Ssh { .. }
                | SessionType::Telnet { .. }
                | SessionType::Ftp { .. }
                | SessionType::Rdp { .. }
                | SessionType::Vnc { .. }
        )
    }

    pub fn remote_host_and_user(&self) -> (String, String) {
        match self {
            SessionType::Ssh { host, user, .. } => (host.clone(), user.clone()),
            SessionType::Telnet { host, .. } => (host.clone(), "telnet".into()),
            SessionType::Rdp { host, user, .. } => (host.clone(), user.clone()),
            SessionType::Vnc { host, .. } => (host.clone(), "vnc".into()),
            SessionType::Ftp { host, user, .. } => (host.clone(), user.clone()),
            SessionType::Wsl { distro, user, .. } => (distro.clone(), user.clone().unwrap_or_else(|| "wsl".into())),
            SessionType::Serial { port_name, .. } => (port_name.clone(), "serial".into()),
            SessionType::LocalShell { .. } => ("LOCALHOST".into(), "user".into()),
        }
    }
}

fn default_ssh_browser_type() -> String {
    "SFTP protocol".to_string()
}

fn default_font_size() -> u16 {
    14
}

fn default_color_scheme() -> String {
    "RustyBox Dark (Default)".to_string()
}

fn default_true() -> bool {
    true
}

fn default_gateway_port() -> u16 {
    22
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub folder: String,
    pub session_type: SessionType,
    pub auto_sftp: bool,
    pub created_at: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub save_password: bool,
    // Extended RustyBox Session settings (backward-compatible via #[serde(default)])
    #[serde(default = "default_ssh_browser_type")]
    pub ssh_browser_type: String,
    #[serde(default)]
    pub remote_command: String,
    #[serde(default)]
    pub do_not_exit_after_cmd: bool,
    #[serde(default = "default_true")]
    pub ssh_compression: bool,
    #[serde(default = "default_font_size")]
    pub terminal_font_size: u16,
    #[serde(default = "default_color_scheme")]
    pub terminal_color_scheme: String,
    #[serde(default)]
    pub backspace_sends_ctrl_h: bool,
    #[serde(default = "default_true")]
    pub right_click_paste: bool,
    #[serde(default)]
    pub log_terminal_output: bool,
    #[serde(default)]
    pub log_directory: String,
    #[serde(default)]
    pub use_ssh_gateway: bool,
    #[serde(default)]
    pub gateway_host: String,
    #[serde(default = "default_gateway_port")]
    pub gateway_port: u16,
    #[serde(default)]
    pub gateway_user: String,
    #[serde(default)]
    pub gateway_key: String,
    #[serde(default = "default_true")]
    pub ssh_keepalive: bool,
    #[serde(default)]
    pub tab_color: String,
    #[serde(default)]
    pub comments: String,
    #[serde(default)]
    pub last_connected_at: Option<String>,
    #[serde(default)]
    pub connect_count: u32,
}

impl Session {
    pub fn new_local(name: &str, folder: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            folder: folder.to_string(),
            session_type: SessionType::LocalShell {
                custom_shell: None,
                working_dir: None,
            },
            auto_sftp: true,
            created_at: chrono::Local::now().to_rfc3339(),
            password: None,
            save_password: false,
            ssh_browser_type: default_ssh_browser_type(),
            remote_command: String::new(),
            do_not_exit_after_cmd: false,
            ssh_compression: true,
            terminal_font_size: 14,
            terminal_color_scheme: default_color_scheme(),
            backspace_sends_ctrl_h: false,
            right_click_paste: true,
            log_terminal_output: false,
            log_directory: String::new(),
            use_ssh_gateway: false,
            gateway_host: String::new(),
            gateway_port: 22,
            gateway_user: String::new(),
            gateway_key: String::new(),
            ssh_keepalive: true,
            tab_color: String::new(),
            comments: String::new(),
            last_connected_at: None,
            connect_count: 0,
        }
    }

    pub fn new_ssh(name: &str, folder: &str, host: &str, port: u16, user: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            folder: folder.to_string(),
            session_type: SessionType::Ssh {
                host: host.to_string(),
                port,
                user: user.to_string(),
                auth_method: AuthMethod::Password,
                key_path: None,
                jump_host: None,
                tunnel_x11: false,
            },
            auto_sftp: true,
            created_at: chrono::Local::now().to_rfc3339(),
            password: None,
            save_password: false,
            ssh_browser_type: default_ssh_browser_type(),
            remote_command: String::new(),
            do_not_exit_after_cmd: false,
            ssh_compression: true,
            terminal_font_size: 14,
            terminal_color_scheme: default_color_scheme(),
            backspace_sends_ctrl_h: false,
            right_click_paste: true,
            log_terminal_output: false,
            log_directory: String::new(),
            use_ssh_gateway: false,
            gateway_host: String::new(),
            gateway_port: 22,
            gateway_user: String::new(),
            gateway_key: String::new(),
            ssh_keepalive: true,
            tab_color: String::new(),
            comments: String::new(),
            last_connected_at: None,
            connect_count: 0,
        }
    }

    /// Marks the session as connected right now and increments its connection count.
    pub fn mark_connected(&mut self) {
        self.last_connected_at = Some(chrono::Local::now().to_rfc3339());
        self.connect_count = self.connect_count.saturating_add(1);
    }

    /// Returns a concise human-readable connection endpoint subtitle (e.g. `root@10.0.0.5:22`).
    pub fn connection_subtitle(&self) -> String {
        match &self.session_type {
            SessionType::Ssh {
                host, port, user, ..
            } => {
                let u = if user.trim().is_empty() {
                    "root"
                } else {
                    user.trim()
                };
                format!("{}@{}:{}", u, host.trim(), port)
            }
            SessionType::Rdp { host, port, user } => {
                format!("rdp://{}@{}:{}", user.trim(), host.trim(), port)
            }
            SessionType::Ftp { host, port, user } => {
                format!("sftp://{}@{}:{}", user.trim(), host.trim(), port)
            }
            SessionType::Telnet { host, port } => {
                format!("telnet://{}:{}", host.trim(), port)
            }
            SessionType::Vnc { host, port } => {
                format!("vnc://{}:{}", host.trim(), port)
            }
            SessionType::Serial {
                port_name,
                baud_rate,
            } => {
                format!("{} @ {} baud", port_name.trim(), baud_rate)
            }
            SessionType::Wsl { distro, user, .. } => {
                if let Some(u) = user.as_ref().filter(|s| !s.trim().is_empty()) {
                    format!("WSL: {} ({})", distro.trim(), u.trim())
                } else {
                    format!("WSL: {}", distro.trim())
                }
            }
            SessionType::LocalShell { custom_shell, .. } => custom_shell
                .as_ref()
                .filter(|s| !s.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| "Local Shell".into()),
        }
    }

    /// Returns a formatted `YYYY-MM-DD HH:MM` string for `last_connected_at`, if present.
    pub fn last_connected_display(&self) -> Option<String> {
        let raw = self.last_connected_at.as_ref()?;
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
            Some(dt.format("%Y-%m-%d %H:%M").to_string())
        } else {
            Some(raw.clone())
        }
    }

    /// Checks whether the session matches a case-insensitive search query on name, host, user, or folder.
    pub fn matches_filter(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        self.name.to_lowercase().contains(&q)
            || self.folder.to_lowercase().contains(&q)
            || self.connection_subtitle().to_lowercase().contains(&q)
    }

    /// Formats the session label in RustyBox style: `51.79.157.8 (ruby) - dev-vn`
    pub fn rusty_label(&self) -> String {
        match &self.session_type {
            SessionType::Ssh { host, user, .. } => {
                let h = host.trim();
                let u = if user.trim().is_empty() { "root" } else { user.trim() };
                if h.is_empty() || self.name.starts_with(h) {
                    self.name.clone()
                } else if self.name == h {
                    format!("{} ({})", h, u)
                } else {
                    format!("{} ({}) - {}", h, u, self.name)
                }
            }
            SessionType::Rdp { host, user, .. } => {
                if host.trim().is_empty() || self.name.starts_with(host.trim()) {
                    self.name.clone()
                } else {
                    format!("{} ({}) - {}", host.trim(), user.trim(), self.name)
                }
            }
            _ => self.name.clone(),
        }
    }

    pub fn host_str(&self) -> Option<String> {
        match &self.session_type {
            SessionType::Ssh { host, .. }
            | SessionType::Telnet { host, .. }
            | SessionType::Rdp { host, .. }
            | SessionType::Vnc { host, .. }
            | SessionType::Ftp { host, .. } => {
                let trimmed = host.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_mark_connected_and_subtitle() {
        let mut s = Session::new_ssh("Prod API", "Production", "10.0.1.25", 2222, "deploy");
        assert!(s.last_connected_at.is_none());
        assert_eq!(s.connect_count, 0);
        assert_eq!(s.connection_subtitle(), "deploy@10.0.1.25:2222");

        s.mark_connected();
        assert!(s.last_connected_at.is_some());
        assert_eq!(s.connect_count, 1);
        assert!(s.last_connected_display().is_some());

        assert!(s.matches_filter("prod"));
        assert!(s.matches_filter("10.0.1.25"));
        assert!(s.matches_filter("deploy"));
        assert!(!s.matches_filter("staging"));
    }
}
