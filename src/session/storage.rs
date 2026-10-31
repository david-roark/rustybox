use super::Session;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct StorageManager {
    file_path: PathBuf,
    is_portable: bool,
}

impl StorageManager {
    pub fn new() -> Self {
        // Check if portable mode is active (sessions.json exists next to exe or in current dir)
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        let portable_path = exe_dir.join("sessions.json");
        let local_portable = PathBuf::from("sessions.json");

        if portable_path.exists() {
            Self {
                file_path: portable_path,
                is_portable: true,
            }
        } else if local_portable.exists() {
            Self {
                file_path: local_portable,
                is_portable: true,
            }
        } else {
            let config_dir = dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("rust-remote-toolbox");
            let _ = fs::create_dir_all(&config_dir);
            Self {
                file_path: config_dir.join("sessions.json"),
                is_portable: false,
            }
        }
    }

    pub fn is_portable(&self) -> bool {
        self.is_portable
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    pub fn load_sessions(&self) -> Vec<Session> {
        if !self.file_path.exists() {
            return self.default_sessions();
        }
        match fs::read_to_string(&self.file_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| self.default_sessions()),
            Err(_) => self.default_sessions(),
        }
    }

    pub fn save_sessions(&self, sessions: &[Session]) -> Result<(), String> {
        if let Some(parent) = self.file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let data = serde_json::to_string_pretty(sessions).map_err(|e| e.to_string())?;
        fs::write(&self.file_path, data).map_err(|e| e.to_string())
    }

    pub fn export_to_file(&self, path: &Path, sessions: &[Session]) -> Result<(), String> {
        let data = serde_json::to_string_pretty(sessions).map_err(|e| e.to_string())?;
        fs::write(path, data).map_err(|e| e.to_string())
    }

    pub fn import_from_file(&self, path: &Path) -> Result<Vec<Session>, String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    pub fn encrypt_with_master_password(password: &str, plaintext: &str) -> Result<String, String> {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        let key = hasher.finalize();

        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
        let nonce_bytes = [0x42u8; 12]; // standard fixed nonce for file vault or random
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(base64::engine::general_purpose::STANDARD.encode(ciphertext))
    }

    pub fn decrypt_with_master_password(password: &str, encrypted_b64: &str) -> Result<String, String> {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        let key = hasher.finalize();

        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(encrypted_b64)
            .map_err(|e| e.to_string())?;
        let nonce_bytes = [0x42u8; 12];
        let nonce = Nonce::from_slice(&nonce_bytes);

        let decrypted = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| "Invalid master password or corrupted data".to_string())?;
        String::from_utf8(decrypted).map_err(|e| e.to_string())
    }

    fn default_sessions(&self) -> Vec<Session> {
        vec![
            Session::new_local("Local Terminal", "Default"),
            Session::new_ssh("Demo SSH (Testnet)", "Servers", "test.rebex.net", 22, "demo"),
            Session::new_ssh("Production Web", "Servers", "192.168.1.100", 22, "root"),
        ]
    }
}
