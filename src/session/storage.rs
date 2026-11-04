use super::Session;
use aes_gcm::{
    aead::{rand_core::RngCore, Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_BACKUP_FILES: usize = 3;

fn default_version() -> String {
    "1.3".to_string()
}

fn default_ui_scale() -> f32 {
    1.0
}

fn default_font_size() -> f32 {
    14.0
}

fn default_theme_name() -> String {
    "RustyBox Dark (Default)".to_string()
}

fn default_true() -> bool {
    true
}

pub fn default_quick_macros() -> Vec<(String, String)> {
    vec![
        ("htop".into(), "htop\n".into()),
        ("docker ps".into(), "docker ps\n".into()),
        ("df -h".into(), "df -h\n".into()),
        ("free -m".into(), "free -m\n".into()),
        ("netstat".into(), "netstat -tlpn\n".into()),
        ("systemctl".into(), "systemctl status\n".into()),
        ("uptime".into(), "uptime\n".into()),
        ("clear".into(), "clear\n".into()),
    ]
}

pub fn default_sessions_list() -> Vec<Session> {
    vec![
        Session::new_local("Local Terminal", "Default"),
        Session::new_ssh("Demo SSH (Testnet)", "Servers", "test.rebex.net", 22, "demo"),
        Session::new_ssh("Production Web", "Servers", "192.168.1.100", 22, "root"),
    ]
}

pub const ENC_PREFIX: &str = "enc:v1:";
const VAULT_CHECK_PLAINTEXT: &str = "rustybox-vault-ok";

/// Stored in RustyBox.json: salt for Argon2id + an encrypted check value used to verify the master password.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VaultMeta {
    pub salt: String,
    pub check: String,
}

pub fn is_encrypted(s: &str) -> bool {
    s.starts_with(ENC_PREFIX)
}

pub fn derive_key(master: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    argon2::Argon2::default()
        .hash_password_into(master.as_bytes(), salt, &mut key)
        .map_err(|e| e.to_string())?;
    Ok(key)
}

/// Random nonce per call. Output: `enc:v1:<nonce_b64>:<ciphertext_b64>`
pub fn encrypt_str(key: &[u8; 32], plaintext: &str) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ct = cipher.encrypt(&nonce, plaintext.as_bytes()).map_err(|e| e.to_string())?;
    let b = &base64::engine::general_purpose::STANDARD;
    Ok(format!("{}{}:{}", ENC_PREFIX, b.encode(nonce), b.encode(ct)))
}

pub fn decrypt_str(key: &[u8; 32], enc: &str) -> Result<String, String> {
    let rest = enc.strip_prefix(ENC_PREFIX).ok_or("not an encrypted value")?;
    let (n, c) = rest.split_once(':').ok_or("malformed encrypted value")?;
    let b = &base64::engine::general_purpose::STANDARD;
    let nonce_bytes = b.decode(n).map_err(|e| e.to_string())?;
    let ct = b.decode(c).map_err(|e| e.to_string())?;
    if nonce_bytes.len() != 12 {
        return Err("bad nonce".into());
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let pt = cipher
        .decrypt(Nonce::from_slice(&nonce_bytes), ct.as_ref())
        .map_err(|_| "Wrong master password or corrupted data".to_string())?;
    String::from_utf8(pt).map_err(|e| e.to_string())
}

/// Creates new vault metadata + key for a master password (fresh random salt).
pub fn create_vault(master: &str) -> Result<(VaultMeta, [u8; 32]), String> {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let key = derive_key(master, &salt)?;
    let check = encrypt_str(&key, VAULT_CHECK_PLAINTEXT)?;
    let meta = VaultMeta {
        salt: base64::engine::general_purpose::STANDARD.encode(salt),
        check,
    };
    Ok((meta, key))
}

/// Verifies master password against metadata; returns the key on success.
pub fn unlock_vault(meta: &VaultMeta, master: &str) -> Result<[u8; 32], String> {
    let salt = base64::engine::general_purpose::STANDARD
        .decode(&meta.salt)
        .map_err(|e| e.to_string())?;
    let key = derive_key(master, &salt)?;
    match decrypt_str(&key, &meta.check) {
        Ok(v) if v == VAULT_CHECK_PLAINTEXT => Ok(key),
        _ => Err("Wrong master password".into()),
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Credential {
    pub name: String,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RustyBoxConfig {
    #[serde(default)]
    pub vault: Option<VaultMeta>,
    /// 0 = Always, 1 = Never, 2 = Ask
    #[serde(default)]
    pub password_save_mode: u8,
    #[serde(default)]
    pub save_key_passphrases: bool,
    #[serde(default)]
    pub credentials: Vec<Credential>,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_theme_name")]
    pub theme_name: String,
    #[serde(default = "default_true")]
    pub paste_protection_enabled: bool,
    #[serde(default = "default_quick_macros")]
    pub quick_macros: Vec<(String, String)>,
    pub sessions: Vec<Session>,
}

impl Default for RustyBoxConfig {
    fn default() -> Self {
        Self {
            vault: None,
            password_save_mode: 0,
            save_key_passphrases: false,
            credentials: Vec::new(),
            version: default_version(),
            ui_scale: default_ui_scale(),
            font_size: default_font_size(),
            theme_name: default_theme_name(),
            paste_protection_enabled: true,
            quick_macros: default_quick_macros(),
            sessions: default_sessions_list(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct BackupFileInfo {
    pub unix_ts: u64,
    pub file_name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct StorageManager {
    data_dir: PathBuf,
    file_path: PathBuf,
    is_portable: bool,
    vault_key: Option<[u8; 32]>,
    is_vault_configured: bool,
}

impl StorageManager {
    pub fn new() -> Self {
        // Strictly relative to the application folder: ./data/RustyBox.json (never %TEMP% or %APPDATA%)
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        let data_dir = exe_dir.join("data");
        let file_path = data_dir.join("RustyBox.json");

        let is_vault_configured = if file_path.exists() {
            std::fs::read_to_string(&file_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<RustyBoxConfig>(&raw).ok())
                .map(|cfg| cfg.vault.is_some())
                .unwrap_or(false)
        } else {
            false
        };

        Self {
            data_dir,
            file_path,
            is_portable: true,
            vault_key: None,
            is_vault_configured,
        }
    }

    pub fn is_vault_configured(&self) -> bool {
        self.is_vault_configured
    }

    pub fn set_vault_configured(&mut self, configured: bool) {
        self.is_vault_configured = configured;
    }

    pub fn set_vault_key(&mut self, key: Option<[u8; 32]>) {
        self.vault_key = key;
    }

    pub fn is_vault_unlocked(&self) -> bool {
        self.vault_key.is_some()
    }

    pub fn vault_key(&self) -> Option<[u8; 32]> {
        self.vault_key
    }

    /// True if the config holds any plaintext (not yet encrypted) password.
    pub fn has_plaintext_passwords(config: &RustyBoxConfig) -> bool {
        config.sessions.iter().any(|s| s.password.as_deref().map_or(false, |p| !p.is_empty() && !is_encrypted(p)))
            || config.credentials.iter().any(|c| !c.password.is_empty() && !is_encrypted(&c.password))
    }

    /// True when plaintext passwords exist in memory but cannot be written (no vault key).
    pub fn has_unsaved_plaintext(&self, config: &RustyBoxConfig) -> bool {
        self.vault_key.is_none() && Self::has_plaintext_passwords(config)
    }

    /// Builds the on-disk representation: passwords are encrypted with the vault key if configured.
    /// Already-encrypted values pass through unchanged (a locked vault keeps its data).
    pub fn prepare_for_disk(&self, config: &RustyBoxConfig) -> Result<RustyBoxConfig, String> {
        let mut out = config.clone();
        let conv = |p: &str| -> Result<Option<String>, String> {
            if p.is_empty() {
                return Ok(Some(String::new()));
            }
            if is_encrypted(p) {
                if out.vault.is_none() {
                    if let Some(k) = &self.vault_key {
                        if let Ok(dec) = decrypt_str(k, p) {
                            return Ok(Some(dec));
                        }
                    }
                }
                return Ok(Some(p.to_string()));
            }
            match &self.vault_key {
                Some(k) => Ok(Some(encrypt_str(k, p)?)),
                None => Ok(Some(p.to_string())),
            }
        };
        for s in out.sessions.iter_mut() {
            if let Some(p) = s.password.clone() {
                s.password = conv(&p)?;
            }
        }
        for c in out.credentials.iter_mut() {
            c.password = conv(&c.password)?.unwrap_or_default();
        }
        Ok(out)
    }

    pub fn is_portable(&self) -> bool {
        self.is_portable
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    pub fn load_sessions(&self) -> Vec<Session> {
        self.load_config().sessions
    }

    pub fn save_sessions(&self, sessions: &[Session]) -> Result<(), String> {
        let mut cfg = self.load_config();
        cfg.sessions = sessions.to_vec();
        self.save_config(&cfg)
    }

    pub fn load_credentials(&self) -> Vec<Credential> {
        self.load_config().credentials
    }

    pub fn save_credentials(&self, credentials: &[Credential]) -> Result<(), String> {
        let mut cfg = self.load_config();
        cfg.credentials = credentials.to_vec();
        self.save_config(&cfg)
    }

    pub fn save_preferences(&self, ui_scale: f32, font_size: f32, theme_name: &str, paste_protection: bool) -> Result<(), String> {
        let mut cfg = self.load_config();
        cfg.ui_scale = ui_scale;
        cfg.font_size = font_size;
        cfg.theme_name = theme_name.to_string();
        cfg.paste_protection_enabled = paste_protection;
        self.save_config(&cfg)
    }

    pub fn parse_config_str(content: &str) -> Option<RustyBoxConfig> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(cfg) = serde_json::from_str::<RustyBoxConfig>(trimmed) {
            return Some(cfg);
        }
        if let Ok(sessions) = serde_json::from_str::<Vec<Session>>(trimmed) {
            return Some(RustyBoxConfig {
                sessions,
                ..RustyBoxConfig::default()
            });
        }
        None
    }

    /// Lists all `data/RustyBox.json.<unixtime>` backup files sorted newest-first.
    pub fn list_backups(&self) -> Vec<BackupFileInfo> {
        let mut backups = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.data_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if let Some(suffix) = file_name.strip_prefix("RustyBox.json.") {
                    if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                        if let Ok(unix_ts) = suffix.parse::<u64>() {
                            backups.push(BackupFileInfo {
                                unix_ts,
                                file_name,
                                path: entry.path(),
                            });
                        }
                    }
                }
            }
        }
        backups.sort_by(|a, b| b.unix_ts.cmp(&a.unix_ts));
        backups
    }

    /// Retains at most `max_keep` newest `data/RustyBox.json.<unixtime>` files and removes older ones.
    pub fn prune_backups(&self, max_keep: usize) {
        let backups = self.list_backups();
        for old in backups.into_iter().skip(max_keep) {
            let _ = fs::remove_file(old.path);
        }
    }

    fn write_atomic_raw(&self, data: &str) -> Result<(), String> {
        fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        let part_path = self.data_dir.join("RustyBox.json.part");

        let write_part = (|| -> std::io::Result<()> {
            let mut f = File::create(&part_path)?;
            f.write_all(data.as_bytes())?;
            f.sync_all()?;
            Ok(())
        })();

        let res = if write_part.is_ok() {
            if fs::rename(&part_path, &self.file_path).is_ok() {
                Ok(())
            } else {
                let _ = fs::remove_file(&part_path);
                fs::write(&self.file_path, data).map_err(|e| e.to_string())
            }
        } else {
            fs::write(&self.file_path, data).map_err(|e| e.to_string())
        };

        #[cfg(unix)]
        if res.is_ok() {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.data_dir, fs::Permissions::from_mode(0o700));
            let _ = fs::set_permissions(&self.file_path, fs::Permissions::from_mode(0o600));
        }

        res
    }

    /// Startup check:
    /// 1. If `data/RustyBox.json` exists and is valid -> load and use it.
    /// 2. If `data/RustyBox.json` exists but is corrupted/malformed -> auto-recover from newest valid `data/RustyBox.json.<unixtime>` backup.
    /// 3. If `data/RustyBox.json` does not exist -> treat as brand-new configuration and create `data/RustyBox.json`.
    pub fn load_config(&self) -> RustyBoxConfig {
        if self.file_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.file_path) {
                if let Some(cfg) = Self::parse_config_str(&content) {
                    return cfg;
                }
            }

            // Corrupted or malformed RustyBox.json: attempt automatic recovery from newest valid backup
            for bak in self.list_backups() {
                if let Ok(bak_content) = fs::read_to_string(&bak.path) {
                    if let Some(recovered_cfg) = Self::parse_config_str(&bak_content) {
                        if let Ok(clean_json) = serde_json::to_string_pretty(&recovered_cfg) {
                            let _ = self.write_atomic_raw(&clean_json);
                        }
                        return recovered_cfg;
                    }
                }
            }
        }

        let fresh = RustyBoxConfig::default();
        if let Ok(initial_json) = serde_json::to_string_pretty(&fresh) {
            let _ = self.write_atomic_raw(&initial_json);
        }
        fresh
    }

    /// Saves updated configuration to `data/RustyBox.json`.
    /// When `create_backup` is true and a valid `data/RustyBox.json` already exists with different content:
    /// 1. Backs up current `data/RustyBox.json` -> `data/RustyBox.json.<unixtime>` BEFORE updating.
    /// 2. Rotates backups to keep at most `MAX_BACKUP_FILES` (3) newest files.
    /// 3. Atomically writes and replaces `data/RustyBox.json`.
    pub fn save_config_with_options(&self, config: &RustyBoxConfig, create_backup: bool) -> Result<(), String> {
        fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        let disk_cfg = self.prepare_for_disk(config)?;
        let new_data = serde_json::to_string_pretty(&disk_cfg).map_err(|e| e.to_string())?;

        if self.file_path.exists() {
            if let Ok(old_content) = fs::read_to_string(&self.file_path) {
                if old_content == new_data {
                    return Ok(());
                }

                if create_backup && Self::parse_config_str(&old_content).is_some() {
                    let now_ts = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let max_existing = self.list_backups().first().map(|b| b.unix_ts).unwrap_or(0);
                    let ts = if now_ts <= max_existing {
                        max_existing + 1
                    } else {
                        now_ts
                    };
                    let backup_path = self.data_dir.join(format!("RustyBox.json.{}", ts));

                    // Must finish backup BEFORE updating RustyBox.json
                    fs::copy(&self.file_path, &backup_path)
                        .map_err(|e| format!("Backup failed before update: {}", e))?;

                    self.prune_backups(MAX_BACKUP_FILES);
                }
            }
        }

        self.write_atomic_raw(&new_data)
    }

    pub fn save_config(&self, config: &RustyBoxConfig) -> Result<(), String> {
        self.save_config_with_options(config, true)
    }

    pub fn restore_from_backup(&self, backup_path: &Path) -> Result<RustyBoxConfig, String> {
        let content = fs::read_to_string(backup_path).map_err(|e| e.to_string())?;
        let cfg = Self::parse_config_str(&content)
            .ok_or_else(|| "Selected backup file has invalid JSON format".to_string())?;
        let clean_json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
        self.write_atomic_raw(&clean_json)?;
        Ok(cfg)
    }

    pub fn export_to_file(&self, path: &Path, config: &RustyBoxConfig) -> Result<(), String> {
        let disk_cfg = self.prepare_for_disk(config)?;
        let data = serde_json::to_string_pretty(&disk_cfg).map_err(|e| e.to_string())?;
        fs::write(path, data).map_err(|e| e.to_string())
    }

    pub fn import_from_file(&self, path: &Path) -> Result<RustyBoxConfig, String> {
        let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
        Self::parse_config_str(&content)
            .ok_or_else(|| "Invalid RustyBox JSON configuration format".to_string())
    }

    #[cfg(test)]
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        let file_path = data_dir.join("RustyBox.json");
        Self {
            data_dir,
            file_path,
            is_portable: true,
            vault_key: None,
            is_vault_configured: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_before_update_max_3_and_corrupt_recovery() {
        let base = PathBuf::from("/home/longkaka/.hermes/profiles/agent12/cache/scratch")
            .join(format!("storage_unit_test_{}", uuid::Uuid::new_v4()));
        let data_dir = base.join("data");
        let storage = StorageManager::with_data_dir(data_dir.clone());

        // 1. Fresh launch: creates RustyBox.json with 3 default sessions and 0 backups
        let mut cfg = storage.load_config();
        assert!(storage.file_path().exists());
        assert_eq!(cfg.sessions.len(), 3);
        assert_eq!(storage.list_backups().len(), 0);

        // 2. Add 5 new accounts sequentially -> each save backs up old RustyBox.json to RustyBox.json.<unixtime>
        //    and prunes so strictly at most 3 newest backups remain!
        for i in 1..=5 {
            cfg.sessions.push(Session::new_ssh(
                &format!("Added Account #{}", i),
                "Production",
                &format!("10.0.0.{}", i),
                22,
                "admin",
            ));
            storage.save_config(&cfg).expect("save_config must succeed");

            let backups = storage.list_backups();
            let expected_count = i.min(MAX_BACKUP_FILES);
            assert_eq!(backups.len(), expected_count);
        }

        // Verify current RustyBox.json has 3 + 5 = 8 accounts
        let reloaded = storage.load_config();
        assert_eq!(reloaded.sessions.len(), 8);

        // Verify strictly 3 newest backups exist, and the newest backup holds the state right BEFORE the 5th addition (7 accounts)
        let backups = storage.list_backups();
        assert_eq!(backups.len(), 3);
        assert!(backups[0].unix_ts > backups[1].unix_ts);
        assert!(backups[1].unix_ts > backups[2].unix_ts);

        let newest_bak_content = fs::read_to_string(&backups[0].path).unwrap();
        let newest_bak_cfg = StorageManager::parse_config_str(&newest_bak_content).unwrap();
        assert_eq!(newest_bak_cfg.sessions.len(), 7);

        // 3. Simulate file corruption / malformed JSON on RustyBox.json
        fs::write(storage.file_path(), "{{{ CORRUPTED MALFORMED JSON !!!").unwrap();

        // Reloading must auto-recover from newest valid RustyBox.json.<unixtime> backup (7 accounts)
        // and must NOT overwrite the 3 valid backups with the corrupted text!
        let recovered = storage.load_config();
        assert_eq!(recovered.sessions.len(), 7);
        assert_eq!(storage.list_backups().len(), 3);

        // RustyBox.json on disk is repaired back to valid JSON
        let repaired_on_disk = fs::read_to_string(storage.file_path()).unwrap();
        assert!(StorageManager::parse_config_str(&repaired_on_disk).is_some());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_credentials_load_save() {
        let base = PathBuf::from("/home/longkaka/.hermes/profiles/agent12/cache/scratch")
            .join(format!("cred_unit_test_{}", uuid::Uuid::new_v4()));
        let storage = StorageManager::with_data_dir(base.clone());
        let creds = vec![
            Credential {
                name: "Vultr Trial".into(),
                username: "root".into(),
                password: "mR@6x?U(P$,hQ8RA".into(),
            }
        ];
        storage.save_credentials(&creds).unwrap();
        let loaded = storage.load_credentials();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Vultr Trial");
        assert_eq!(loaded[0].username, "root");
        assert_eq!(loaded[0].password, "mR@6x?U(P$,hQ8RA");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn test_vault_encrypts_passwords_on_disk() {
        let base = PathBuf::from("/home/longkaka/.hermes/profiles/agent12/cache/scratch")
            .join(format!("vault_unit_test_{}", uuid::Uuid::new_v4()));
        let mut storage = StorageManager::with_data_dir(base.join("data"));
        let mut cfg = storage.load_config();
        cfg.sessions[1].password = Some("S3cret-Pass!".into());
        cfg.sessions[1].save_password = true;

        // Without key: plaintext is preserved so user credentials are not lost
        storage.save_config(&cfg).unwrap();
        let on_disk = fs::read_to_string(storage.file_path()).unwrap();
        assert!(on_disk.contains("S3cret-Pass!"));

        // With key: file contains only enc:v1 and no plaintext
        let (meta, key) = create_vault("master-1").unwrap();
        cfg.vault = Some(meta.clone());
        storage.set_vault_key(Some(key));
        storage.save_config(&cfg).unwrap();
        let on_disk = fs::read_to_string(storage.file_path()).unwrap();
        assert!(!on_disk.contains("S3cret-Pass!"));
        assert!(on_disk.contains("enc:v1:"));

        // Reload + unlock: wrong password fails, right one decrypts
        let loaded = storage.load_config();
        let enc = loaded.sessions[1].password.clone().unwrap();
        assert!(is_encrypted(&enc));
        assert!(unlock_vault(loaded.vault.as_ref().unwrap(), "wrong").is_err());
        let k2 = unlock_vault(loaded.vault.as_ref().unwrap(), "master-1").unwrap();
        assert_eq!(decrypt_str(&k2, &enc).unwrap(), "S3cret-Pass!");

        // Random nonce: same plaintext encrypts differently
        assert_ne!(encrypt_str(&k2, "x").unwrap(), encrypt_str(&k2, "x").unwrap());

        // Locked (no key) save keeps encrypted values intact
        let mut locked = StorageManager::with_data_dir(base.join("data"));
        locked.set_vault_key(None);
        locked.save_config(&loaded).unwrap();
        let again = locked.load_config();
        assert_eq!(again.sessions[1].password.as_deref(), Some(enc.as_str()));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_master_vault_credentials_and_sessions_disk_flow() {
        let base = PathBuf::from("/home/longkaka/.hermes/profiles/agent12/cache/scratch")
            .join(format!("master_vault_flow_{}", uuid::Uuid::new_v4()));
        let mut storage = StorageManager::with_data_dir(base.join("data"));
        let mut cfg = storage.load_config();

        // 1. Setup plaintext credentials & sessions
        cfg.credentials.push(Credential {
            name: "Vultr Trial".into(),
            username: "root".into(),
            password: "mR@6x?U(P$,hQ8RA".into(),
        });
        cfg.sessions[0].password = Some("MySuperSecret123!".into());
        cfg.sessions[0].save_password = true;

        // Plaintext save: both passwords visible in JSON on disk
        storage.save_config(&cfg).unwrap();
        let json_plain = fs::read_to_string(storage.file_path()).unwrap();
        assert!(json_plain.contains("mR@6x?U(P$,hQ8RA"));
        assert!(json_plain.contains("MySuperSecret123!"));
        assert!(json_plain.contains("\"vault\": null"));

        // 2. Enable Master Password Vault
        let (meta, key) = create_vault("LongSecretKey2026!").unwrap();
        cfg.vault = Some(meta);
        storage.set_vault_key(Some(key));
        storage.save_config(&cfg).unwrap();

        // 3. Verify JSON on disk is 100% encrypted
        let json_enc = fs::read_to_string(storage.file_path()).unwrap();
        assert!(!json_enc.contains("mR@6x?U(P$,hQ8RA"), "Plaintext credential leaked to disk!");
        assert!(!json_enc.contains("MySuperSecret123!"), "Plaintext session password leaked to disk!");
        assert!(json_enc.contains("enc:v1:"));
        assert!(json_enc.contains("\"salt\":"));
        assert!(json_enc.contains("\"check\":"));

        // 4. Simulate app restart (locked state)
        let mut restarted_storage = StorageManager::with_data_dir(base.join("data"));
        assert!(!restarted_storage.is_vault_unlocked());
        let loaded_locked = restarted_storage.load_config();
        assert!(loaded_locked.vault.is_some());
        assert!(is_encrypted(&loaded_locked.credentials[0].password));
        assert!(is_encrypted(loaded_locked.sessions[0].password.as_ref().unwrap()));

        // 5. Unlock vault with correct master password
        let unlocked_key = unlock_vault(loaded_locked.vault.as_ref().unwrap(), "LongSecretKey2026!").unwrap();
        restarted_storage.set_vault_key(Some(unlocked_key));
        assert!(restarted_storage.is_vault_unlocked());

        // Decrypt in memory
        let cred_dec = decrypt_str(&unlocked_key, &loaded_locked.credentials[0].password).unwrap();
        let sess_dec = decrypt_str(&unlocked_key, loaded_locked.sessions[0].password.as_ref().unwrap()).unwrap();
        assert_eq!(cred_dec, "mR@6x?U(P$,hQ8RA");
        assert_eq!(sess_dec, "MySuperSecret123!");

        // 6. Disable/Remove Master Password (store plaintext)
        let mut unlocked_cfg = loaded_locked.clone();
        unlocked_cfg.vault = None;
        unlocked_cfg.credentials[0].password = cred_dec;
        unlocked_cfg.sessions[0].password = Some(sess_dec);
        restarted_storage.set_vault_key(None);
        restarted_storage.save_config(&unlocked_cfg).unwrap();

        let json_reverted = fs::read_to_string(restarted_storage.file_path()).unwrap();
        assert!(json_reverted.contains("mR@6x?U(P$,hQ8RA"));
        assert!(json_reverted.contains("MySuperSecret123!"));
        assert!(json_reverted.contains("\"vault\": null"));

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn test_ui_scale_and_preferences_persistence() {
        let base = std::env::temp_dir().join(format!("test_prefs_{}", uuid::Uuid::new_v4()));
        let storage = StorageManager::with_data_dir(base.join("data"));

        // Default ui_scale should be 1.0
        let cfg = storage.load_config();
        assert_eq!(cfg.ui_scale, 1.0);
        assert_eq!(cfg.font_size, 14.0);

        // Update preferences
        storage.save_preferences(1.25, 16.0, "Dracula", false).unwrap();

        let loaded = storage.load_config();
        assert_eq!(loaded.ui_scale, 1.25);
        assert_eq!(loaded.font_size, 16.0);
        assert_eq!(loaded.theme_name, "Dracula");
        assert_eq!(loaded.paste_protection_enabled, false);

        let _ = fs::remove_dir_all(&base);
    }
}
