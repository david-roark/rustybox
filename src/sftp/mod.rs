use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use parking_lot::Mutex;

pub mod remote;
pub mod ui;

use remote::{RemoteSftpClient, SftpEvent, SftpTargetConfig};

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub permissions: String,
}

pub struct FileExplorer {
    pub current_path: PathBuf,
    pub path_input: String,
    pub path_history: Vec<PathBuf>,
    pub entries: Vec<FileEntry>,
    pub selected_file: Option<PathBuf>,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    // Remote SFTP State
    pub is_remote: bool,
    pub is_loading: bool,
    pub remote_label: Option<String>,
    pub remote_client: Option<Arc<Mutex<RemoteSftpClient>>>,
    pub remote_disk_stats: Option<(u64, u64, f32)>,
    // View & RustyBox SFTP options
    pub show_hidden: bool,
    pub sort_ascending: bool,
    pub follow_terminal_folder: bool,
    pub remote_monitoring_enabled: bool,
    pub active_remote_label: Option<String>,
    // Modal quick editor
    pub editor_open: bool,
    pub editor_file_path: Option<PathBuf>,
    pub editor_content: String,
    pub editor_status: Option<String>,
    // Modal Chmod (RustyBox TFORMCHMOD)
    pub chmod_open: bool,
    pub chmod_target: Option<PathBuf>,
    pub chmod_u_r: bool,
    pub chmod_u_w: bool,
    pub chmod_u_x: bool,
    pub chmod_g_r: bool,
    pub chmod_g_w: bool,
    pub chmod_g_x: bool,
    pub chmod_o_r: bool,
    pub chmod_o_w: bool,
    pub chmod_o_x: bool,
    // New item & delete confirmation dialogs
    pub show_new_folder: bool,
    pub new_folder_name: String,
    pub show_new_file: bool,
    pub new_file_name: String,
    pub pending_delete: Option<PathBuf>,
}

impl FileExplorer {
    pub fn new() -> Self {
        let initial_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let path_input = Self::format_display_path(&initial_path);
        let mut explorer = Self {
            current_path: initial_path.clone(),
            path_input,
            path_history: vec![initial_path],
            entries: Vec::new(),
            selected_file: None,
            error_message: None,
            status_message: None,
            is_remote: false,
            is_loading: false,
            remote_label: None,
            remote_client: None,
            remote_disk_stats: None,
            show_hidden: true,
            sort_ascending: true,
            follow_terminal_folder: false,
            remote_monitoring_enabled: true,
            active_remote_label: None,
            editor_open: false,
            editor_file_path: None,
            editor_content: String::new(),
            editor_status: None,
            chmod_open: false,
            chmod_target: None,
            chmod_u_r: true,
            chmod_u_w: true,
            chmod_u_x: true,
            chmod_g_r: true,
            chmod_g_w: false,
            chmod_g_x: true,
            chmod_o_r: true,
            chmod_o_w: false,
            chmod_o_x: true,
            show_new_folder: false,
            new_folder_name: "new_folder".into(),
            show_new_file: false,
            new_file_name: "new_file.txt".into(),
            pending_delete: None,
        };
        explorer.refresh();
        explorer
    }

    pub fn format_display_path(path: &Path) -> String {
        let raw = path.to_string_lossy().to_string();
        if raw.ends_with('/') || raw.ends_with('\\') {
            raw
        } else if raw.contains('\\') {
            format!("{}\\", raw)
        } else {
            format!("{}/", raw)
        }
    }

    pub fn connect_remote(&mut self, cfg: SftpTargetConfig) {
        let label = format!("{}@{}:{}", cfg.user, cfg.host, cfg.port);
        if self.is_remote && self.remote_label.as_deref() == Some(&label) {
            return;
        }

        self.is_remote = true;
        self.is_loading = true;
        self.remote_label = Some(label.clone());
        self.active_remote_label = Some(label);
        self.error_message = None;
        self.entries.clear();

        let client_arc = if let Some(ref c) = self.remote_client {
            c.clone()
        } else {
            let c = Arc::new(Mutex::new(RemoteSftpClient::new()));
            self.remote_client = Some(c.clone());
            c
        };

        client_arc.lock().connect(cfg);
    }

    pub fn disconnect_remote(&mut self) {
        if let Some(ref client) = self.remote_client {
            client.lock().disconnect();
        }
        self.is_remote = false;
        self.is_loading = false;
        self.remote_label = None;
        self.active_remote_label = None;
        self.remote_disk_stats = None;
        self.current_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        self.path_input = Self::format_display_path(&self.current_path);
        self.refresh();
    }

    pub fn poll(&mut self) {
        let events: Vec<SftpEvent> = if let Some(ref client) = self.remote_client {
            let mut evts = Vec::new();
            let c = client.lock();
            while let Some(evt) = c.try_recv() {
                evts.push(evt);
            }
            evts
        } else {
            Vec::new()
        };

        for evt in events {
            match evt {
                SftpEvent::Connected { target, initial_dir } => {
                    self.is_remote = true;
                    self.remote_label = Some(target.clone());
                    self.active_remote_label = Some(target);
                    self.current_path = PathBuf::from(&initial_dir);
                    self.path_input = Self::format_display_path(&self.current_path);
                    if !self.path_history.contains(&self.current_path) {
                        self.path_history.insert(0, self.current_path.clone());
                    }
                }
                SftpEvent::ConnectionFailed(err) => {
                    self.error_message = Some(format!("SFTP Error: {}", err));
                    self.is_loading = false;
                }
                SftpEvent::DirListed { dir, entries } => {
                    self.current_path = PathBuf::from(&dir);
                    self.path_input = Self::format_display_path(&self.current_path);
                    self.entries = entries;
                    self.is_loading = false;
                    self.error_message = None;
                    if !self.path_history.contains(&self.current_path) {
                        self.path_history.insert(0, self.current_path.clone());
                        if self.path_history.len() > 12 {
                            self.path_history.truncate(12);
                        }
                    }
                }
                SftpEvent::FileRead { path, content } => {
                    self.editor_file_path = Some(PathBuf::from(path));
                    self.editor_content = content;
                    self.editor_status = None;
                    self.editor_open = true;
                    self.is_loading = false;
                }
                SftpEvent::FileWritten { .. } => {
                    self.editor_status = Some("Saved successfully to VPS!".to_string());
                    self.is_loading = false;
                    self.refresh();
                }
                SftpEvent::OperationSuccess(msg) => {
                    self.status_message = Some(msg);
                    self.is_loading = false;
                    self.error_message = None;
                    self.refresh();
                }
                SftpEvent::OperationError(err) => {
                    self.error_message = Some(err);
                    self.is_loading = false;
                }
                SftpEvent::DiskUsage { used, total, pct } => {
                    self.remote_disk_stats = Some((used, total, pct));
                }
            }
        }
    }

    pub fn set_path(&mut self, path: PathBuf) {
        if self.is_remote {
            self.current_path = path.clone();
            self.path_input = Self::format_display_path(&self.current_path);
            self.refresh();
            return;
        }

        if path.is_dir() {
            self.current_path = path.clone();
            self.path_input = Self::format_display_path(&self.current_path);
            if !self.path_history.contains(&path) {
                self.path_history.insert(0, path);
                if self.path_history.len() > 12 {
                    self.path_history.truncate(12);
                }
            }
            self.refresh();
        } else {
            self.error_message = Some(format!("Directory not found: {}", path.display()));
        }
    }

    pub fn sync_terminal_cwd(&mut self, child_pid: Option<u32>) {
        if !self.follow_terminal_folder || self.is_remote {
            return;
        }
        #[cfg(unix)]
        if let Some(pid) = child_pid {
            let proc_cwd = PathBuf::from(format!("/proc/{}/cwd", pid));
            if let Ok(target) = fs::read_link(&proc_cwd) {
                if target.is_dir() && target != self.current_path {
                    self.current_path = target;
                    self.path_input = Self::format_display_path(&self.current_path);
                    self.refresh();
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = child_pid;
        }
    }

    pub fn go_up(&mut self) {
        if self.is_remote {
            let path_str = self.current_path.to_string_lossy().to_string();
            let parent_str = if let Some(parent) = Path::new(&path_str).parent() {
                let p = parent.to_string_lossy().to_string();
                if p.is_empty() { "/".to_string() } else { p }
            } else {
                "/".to_string()
            };
            self.set_path(PathBuf::from(parent_str));
            return;
        }

        if let Some(parent) = self.current_path.parent() {
            let parent_buf = parent.to_path_buf();
            self.set_path(parent_buf);
        }
    }

    pub fn refresh(&mut self) {
        self.error_message = None;
        if self.is_remote {
            if let Some(ref client) = self.remote_client {
                self.is_loading = true;
                let path_str = self.current_path.to_string_lossy().to_string();
                client.lock().list_dir(&path_str);
            }
            return;
        }

        self.entries.clear();
        self.path_input = Self::format_display_path(&self.current_path);

        match fs::read_dir(&self.current_path) {
            Ok(read_dir) => {
                let mut dirs = Vec::new();
                let mut files = Vec::new();

                for entry_res in read_dir {
                    if let Ok(entry) = entry_res {
                        let path = entry.path();
                        let name = entry.file_name().to_string_lossy().to_string();
                        let is_hidden = name.starts_with('.');
                        if !self.show_hidden && is_hidden {
                            continue;
                        }

                        let metadata = entry.metadata().ok();
                        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                        let modified = metadata.as_ref().and_then(|m| m.modified().ok());
                        let permissions = Self::format_permissions(metadata.as_ref(), is_dir);

                        let item = FileEntry {
                            name,
                            path,
                            is_dir,
                            is_hidden,
                            size,
                            modified,
                            permissions,
                        };

                        if is_dir {
                            dirs.push(item);
                        } else {
                            files.push(item);
                        }
                    }
                }

                let asc = self.sort_ascending;
                dirs.sort_by(|a, b| {
                    let cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
                    if asc { cmp } else { cmp.reverse() }
                });
                files.sort_by(|a, b| {
                    let cmp = a.name.to_lowercase().cmp(&b.name.to_lowercase());
                    if asc { cmp } else { cmp.reverse() }
                });

                self.entries.extend(dirs);
                self.entries.extend(files);
            }
            Err(e) => {
                self.error_message = Some(format!("Cannot read dir: {}", e));
            }
        }
    }

    fn format_permissions(meta: Option<&fs::Metadata>, is_dir: bool) -> String {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(m) = meta {
                let mode = m.permissions().mode();
                let mut s = String::with_capacity(10);
                s.push(if is_dir { 'd' } else { '-' });
                let flags = [
                    (0o400, 'r'),
                    (0o200, 'w'),
                    (0o100, 'x'),
                    (0o040, 'r'),
                    (0o020, 'w'),
                    (0o010, 'x'),
                    (0o004, 'r'),
                    (0o002, 'w'),
                    (0o001, 'x'),
                ];
                for (bit, ch) in flags {
                    s.push(if (mode & bit) != 0 { ch } else { '-' });
                }
                return s;
            }
        }
        if is_dir {
            "drwxr-xr-x".to_string()
        } else {
            let readonly = meta.map(|m| m.permissions().readonly()).unwrap_or(false);
            if readonly {
                "-r--r--r--".to_string()
            } else {
                "-rw-r--r--".to_string()
            }
        }
    }

    pub fn open_editor(&mut self, path: PathBuf) {
        if self.is_remote {
            let path_str = path.to_string_lossy().to_string();
            if let Some(ref client) = self.remote_client {
                self.is_loading = true;
                client.lock().read_file(&path_str);
            }
            return;
        }

        match fs::read_to_string(&path) {
            Ok(content) => {
                self.editor_file_path = Some(path);
                self.editor_content = content;
                self.editor_status = None;
                self.editor_open = true;
            }
            Err(e) => {
                self.error_message = Some(format!("Cannot open file: {}", e));
            }
        }
    }

    pub fn save_editor(&mut self) {
        if let Some(ref path) = self.editor_file_path {
            if self.is_remote {
                let path_str = path.to_string_lossy().to_string();
                if let Some(ref client) = self.remote_client {
                    self.is_loading = true;
                    client.lock().write_file(&path_str, &self.editor_content);
                }
                return;
            }

            match fs::write(path, &self.editor_content) {
                Ok(_) => {
                    self.editor_status = Some("Saved successfully!".to_string());
                    self.refresh();
                }
                Err(e) => {
                    self.editor_status = Some(format!("Save error: {}", e));
                }
            }
        }
    }

    pub fn delete_entry(&mut self, path: &Path) {
        if self.is_remote {
            let is_dir = self.entries.iter().find(|e| e.path == path).map(|e| e.is_dir).unwrap_or(false);
            let path_str = path.to_string_lossy().to_string();
            if let Some(ref client) = self.remote_client {
                client.lock().delete(&path_str, is_dir);
            }
            if self.selected_file.as_deref() == Some(path) {
                self.selected_file = None;
            }
            return;
        }

        let res = if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        };
        if let Err(e) = res {
            self.error_message = Some(format!("Delete failed: {}", e));
        } else {
            if self.selected_file.as_deref() == Some(path) {
                self.selected_file = None;
            }
            self.refresh();
        }
    }

    pub fn create_folder(&mut self, name: &str) {
        if self.is_remote {
            let curr = self.current_path.to_string_lossy().to_string();
            let remote_p = if curr.ends_with('/') {
                format!("{}{}", curr, name)
            } else {
                format!("{}/{}", curr, name)
            };
            if let Some(ref client) = self.remote_client {
                client.lock().create_dir(&remote_p);
            }
            return;
        }

        let new_dir = self.current_path.join(name);
        if let Err(e) = fs::create_dir(&new_dir) {
            self.error_message = Some(format!("Create folder failed: {}", e));
        } else {
            self.refresh();
        }
    }

    pub fn create_file(&mut self, name: &str) {
        if self.is_remote {
            let curr = self.current_path.to_string_lossy().to_string();
            let remote_p = if curr.ends_with('/') {
                format!("{}{}", curr, name)
            } else {
                format!("{}/{}", curr, name)
            };
            if let Some(ref client) = self.remote_client {
                client.lock().create_file(&remote_p);
            }
            return;
        }

        let new_path = self.current_path.join(name);
        if let Err(e) = fs::write(&new_path, "") {
            self.error_message = Some(format!("Create file failed: {}", e));
        } else {
            self.refresh();
        }
    }

    pub fn upload_file(&mut self, local_path: PathBuf) {
        if let Some(file_name) = local_path.file_name() {
            let name_str = file_name.to_string_lossy().to_string();
            let curr = self.current_path.to_string_lossy().to_string();
            let remote_p = if curr.ends_with('/') {
                format!("{}{}", curr, name_str)
            } else {
                format!("{}/{}", curr, name_str)
            };
            if let Some(ref client) = self.remote_client {
                self.is_loading = true;
                client.lock().upload(local_path, remote_p);
            }
        }
    }

    pub fn download_file(&mut self, remote_path: &Path, local_dest: PathBuf) {
        let remote_str = remote_path.to_string_lossy().to_string();
        if let Some(ref client) = self.remote_client {
            self.is_loading = true;
            client.lock().download(remote_str, local_dest);
        }
    }

    pub fn open_chmod(&mut self, path: PathBuf) {
        self.chmod_target = Some(path);
        self.chmod_open = true;
    }

    pub fn apply_chmod(&mut self) {
        if let Some(ref path) = self.chmod_target {
            let mut mode = 0u32;
            if self.chmod_u_r { mode |= 0o400; }
            if self.chmod_u_w { mode |= 0o200; }
            if self.chmod_u_x { mode |= 0o100; }
            if self.chmod_g_r { mode |= 0o040; }
            if self.chmod_g_w { mode |= 0o020; }
            if self.chmod_g_x { mode |= 0o010; }
            if self.chmod_o_r { mode |= 0o004; }
            if self.chmod_o_w { mode |= 0o002; }
            if self.chmod_o_x { mode |= 0o001; }

            if self.is_remote {
                let path_str = path.to_string_lossy().to_string();
                if let Some(ref client) = self.remote_client {
                    client.lock().chmod(&path_str, mode);
                }
                self.chmod_open = false;
                return;
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = fs::metadata(path) {
                    let mut perms = meta.permissions();
                    perms.set_mode(mode);
                    let _ = fs::set_permissions(path, perms);
                }
            }
            #[cfg(not(unix))]
            {
                if let Ok(meta) = fs::metadata(path) {
                    let mut perms = meta.permissions();
                    perms.set_readonly(!self.chmod_u_w);
                    let _ = fs::set_permissions(path, perms);
                }
            }
            self.refresh();
        }
        self.chmod_open = false;
    }

    pub fn disk_usage_stats(&self) -> (u64, u64, f32) {
        if self.is_remote {
            if let Some(stats) = self.remote_disk_stats {
                return stats;
            }
            let dir_used: u64 = self.entries.iter().map(|e| e.size).sum();
            let total = 100 * 1_073_741_824u64;
            let used = (8_589_934_592u64).saturating_add(dir_used);
            let pct = (used as f64 / total as f64 * 100.0) as f32;
            return (used, total, pct);
        }

        let dir_used: u64 = self.entries.iter().map(|e| e.size).sum();
        let total = 250 * 1_073_741_824u64;
        let used = (32_589_934_592u64).saturating_add(dir_used);
        let pct = (used as f64 / total as f64 * 100.0) as f32;
        (used, total, pct)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_explorer_remote_sftp_lifecycle() {
        let mut explorer = FileExplorer::new();
        assert!(!explorer.is_remote);

        let cfg = SftpTargetConfig {
            host: "198.51.100.42".to_string(),
            port: 22,
            user: "root".to_string(),
            password: Some("secret".to_string()),
            key_path: None,
        };

        explorer.connect_remote(cfg);
        assert!(explorer.is_remote);
        assert_eq!(explorer.remote_label.as_deref(), Some("root@198.51.100.42:22"));
        assert!(explorer.is_loading);

        explorer.disconnect_remote();
        assert!(!explorer.is_remote);
        assert_eq!(explorer.remote_label, None);
        assert!(!explorer.is_loading);
    }
}

