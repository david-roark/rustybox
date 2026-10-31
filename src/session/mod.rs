use serde::{Deserialize, Serialize};

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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub folder: String,
    pub session_type: SessionType,
    pub auto_sftp: bool,
    pub created_at: String,
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
        }
    }
}
