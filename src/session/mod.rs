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
        }
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
