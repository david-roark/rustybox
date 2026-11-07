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

    pub fn port(&self) -> Option<u16> {
        match self {
            SessionType::Ssh { port, .. } => Some(*port),
            SessionType::Telnet { port, .. } => Some(*port),
            SessionType::Rdp { port, .. } => Some(*port),
            SessionType::Vnc { port, .. } => Some(*port),
            SessionType::Ftp { port, .. } => Some(*port),
            _ => None,
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
    #[serde(default = "default_true")]
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
    #[serde(default)]
    pub terminal_font_family: String,
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
            terminal_font_family: String::new(),
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
            terminal_font_family: String::new(),
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

    /// Records `s` as connected inside `sessions` list:
    /// - Updates `last_connected_at` and increments `connect_count` if matched by `id` or `(host, port, user)`.
    /// - Appends `s` to `sessions` if it is a new Quick Connect / ad-hoc session not yet saved.
    pub fn record_in_list(sessions: &mut Vec<Session>, s: &mut Session) {
        s.mark_connected();
        if let Some(existing) = sessions.iter_mut().find(|item| item.id == s.id) {
            existing.last_connected_at = s.last_connected_at.clone();
            existing.connect_count = existing.connect_count.saturating_add(1);
            s.connect_count = existing.connect_count;
        } else if let SessionType::Ssh {
            ref host,
            port,
            ref user,
            ..
        } = s.session_type
        {
            if let Some(existing) = sessions.iter_mut().find(|item| {
                matches!(
                    &item.session_type,
                    SessionType::Ssh {
                        host: eh,
                        port: ep,
                        user: eu,
                        ..
                    } if eh == host && *ep == port && eu == user
                )
            }) {
                existing.last_connected_at = s.last_connected_at.clone();
                existing.connect_count = existing.connect_count.saturating_add(1);
                s.id = existing.id.clone();
                s.connect_count = existing.connect_count;
            } else {
                sessions.push(s.clone());
            }
        } else {
            sessions.push(s.clone());
        }
    }

    /// Returns sessions that have been connected at least once (`last_connected_at.is_some()`),
    /// filtered by `query` and sorted from most-recently connected to oldest.
    pub fn recent_sessions(sessions: &[Session], query: &str) -> Vec<Session> {
        let mut recent: Vec<Session> = sessions
            .iter()
            .filter(|s| s.last_connected_at.is_some() && s.matches_filter(query))
            .cloned()
            .collect();
        recent.sort_by(|a, b| b.last_connected_at.cmp(&a.last_connected_at));
        recent
    }

    /// Returns all saved sessions matching `query`.
    pub fn saved_sessions(sessions: &[Session], query: &str) -> Vec<Session> {
        sessions
            .iter()
            .filter(|s| s.matches_filter(query))
            .cloned()
            .collect()
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

    #[test]
    fn test_connection_subtitle_all_session_types() {
        let mut s = Session::new_local("Local", "Default");
        assert_eq!(s.connection_subtitle(), "Local Shell");

        s.session_type = SessionType::LocalShell {
            custom_shell: Some("/bin/zsh".into()),
            working_dir: None,
        };
        assert_eq!(s.connection_subtitle(), "/bin/zsh");

        s.session_type = SessionType::Rdp {
            host: "192.168.1.50".into(),
            port: 3389,
            user: "Administrator".into(),
        };
        assert_eq!(s.connection_subtitle(), "rdp://Administrator@192.168.1.50:3389");

        s.session_type = SessionType::Ftp {
            host: "files.example.com".into(),
            port: 21,
            user: "ftpuser".into(),
        };
        assert_eq!(s.connection_subtitle(), "sftp://ftpuser@files.example.com:21");

        s.session_type = SessionType::Telnet {
            host: "10.0.0.1".into(),
            port: 23,
        };
        assert_eq!(s.connection_subtitle(), "telnet://10.0.0.1:23");

        s.session_type = SessionType::Vnc {
            host: "10.0.0.2".into(),
            port: 5901,
        };
        assert_eq!(s.connection_subtitle(), "vnc://10.0.0.2:5901");

        s.session_type = SessionType::Serial {
            port_name: "COM3".into(),
            baud_rate: 115200,
        };
        assert_eq!(s.connection_subtitle(), "COM3 @ 115200 baud");

        s.session_type = SessionType::Wsl {
            distro: "Ubuntu-24.04".into(),
            user: Some("david".into()),
            working_dir: None,
        };
        assert_eq!(s.connection_subtitle(), "WSL: Ubuntu-24.04 (david)");
    }

    #[test]
    fn test_record_in_list_updates_existing_and_persists_quick_connect() {
        let s1 = Session::new_ssh("Web-01", "Prod", "10.10.1.1", 22, "ubuntu");
        let s2 = Session::new_ssh("DB-01", "Prod", "10.10.1.2", 22, "postgres");
        let mut sessions = vec![s1.clone(), s2.clone()];

        // Initially no recent connections
        assert!(Session::recent_sessions(&sessions, "").is_empty());
        assert_eq!(Session::saved_sessions(&sessions, "").len(), 2);

        // 1. Connect to existing session by ID
        let mut launch_s1 = s1.clone();
        Session::record_in_list(&mut sessions, &mut launch_s1);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].connect_count, 1);
        assert!(sessions[0].last_connected_at.is_some());

        // 2. Quick Connect to the same host:port:user (different temporary UUID) deduplicates!
        let mut quick_dup = Session::new_ssh("ubuntu@10.10.1.1", "Quick Connect", "10.10.1.1", 22, "ubuntu");
        Session::record_in_list(&mut sessions, &mut quick_dup);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].connect_count, 2);
        assert_eq!(quick_dup.id, s1.id);

        // 3. Quick Connect to a brand-new host adds it to saved & recent sessions
        let mut quick_new = Session::new_ssh("root@192.168.99.99", "Quick Connect", "192.168.99.99", 2222, "root");
        Session::record_in_list(&mut sessions, &mut quick_new);
        assert_eq!(sessions.len(), 3);
        assert_eq!(sessions[2].folder, "Quick Connect");
        assert_eq!(sessions[2].connect_count, 1);
    }

    #[test]
    fn test_recent_and_saved_sessions_sorting_and_filtering() {
        let mut s1 = Session::new_ssh("Alpha", "Staging", "10.0.0.1", 22, "alice");
        s1.last_connected_at = Some("2026-11-04T10:00:00+07:00".into());
        s1.connect_count = 2;

        let mut s2 = Session::new_ssh("Beta", "Production", "10.0.0.2", 22, "bob");
        s2.last_connected_at = Some("2026-11-04T12:30:00+07:00".into());
        s2.connect_count = 5;

        let s3 = Session::new_ssh("Gamma (Never Connected)", "Production", "10.0.0.3", 22, "charlie");

        let sessions = vec![s1, s2, s3];

        // Recent connections only include connected sessions, ordered newest first (Beta -> Alpha)
        let recent = Session::recent_sessions(&sessions, "");
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].name, "Beta");
        assert_eq!(recent[1].name, "Alpha");
        assert_eq!(recent[0].last_connected_display().as_deref(), Some("2026-11-04 12:30"));

        // Filter recent by user or folder
        let recent_staging = Session::recent_sessions(&sessions, "staging");
        assert_eq!(recent_staging.len(), 1);
        assert_eq!(recent_staging[0].name, "Alpha");

        // Saved sessions include all 3, and filter works across name/folder/host/user
        assert_eq!(Session::saved_sessions(&sessions, "").len(), 3);
        assert_eq!(Session::saved_sessions(&sessions, "production").len(), 2);
        assert_eq!(Session::saved_sessions(&sessions, "charlie").len(), 1);
        assert_eq!(Session::saved_sessions(&sessions, "10.0.0.2").len(), 1);
    }

    #[test]
    fn test_backward_compatible_deserialization_and_edit_preservation() {
        // Legacy JSON without last_connected_at or connect_count
        let legacy_json = r#"{
            "id": "legacy-1",
            "name": "Legacy Server",
            "folder": "Servers",
            "session_type": {
                "Ssh": {
                    "host": "172.16.0.10",
                    "port": 22,
                    "user": "admin",
                    "auth_method": "Password",
                    "key_path": null,
                    "jump_host": null,
                    "tunnel_x11": false
                }
            },
            "auto_sftp": true,
            "created_at": "2026-01-01T00:00:00Z"
        }"#;
        let mut parsed: Session = serde_json::from_str(legacy_json).expect("legacy session JSON must deserialize");
        assert!(parsed.last_connected_at.is_none());
        assert_eq!(parsed.connect_count, 0);

        // Mark connected, then open in NewSessionDialog and verify ID matches
        parsed.mark_connected();
        let mut dlg = crate::session::ui::NewSessionDialog::new();
        dlg.open_for_edit(&parsed);
        dlg.session_name = "Legacy Server Renamed".into();
        let edited = dlg.to_session();
        assert_eq!(edited.id, "legacy-1");
        assert_eq!(edited.name, "Legacy Server Renamed");
    }
}
