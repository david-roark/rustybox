use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use parking_lot::Mutex;

pub mod remote;
pub mod ui;

use remote::{RemoteSftpClient, SftpEvent, SftpTargetConfig, TransferProgressInfo};

#[derive(Clone, Debug)]
pub struct ConflictItem {
    pub is_upload: bool,
    pub src_path: PathBuf,
    pub dest_path: PathBuf,
    pub file_name: String,
}

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub permissions: String,
    pub owner: String,
    pub group: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SftpSortColumn {
    Name,
    Size,
    Modified,
    Owner,
    Group,
    Access,
}

pub fn format_size_kb(size: u64) -> String {
    if size == 0 {
        return "0".to_string();
    }
    let kb = (size + 1023) / 1024;
    let s = kb.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    let len = s.len();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn format_modified_time(time: Option<SystemTime>) -> String {
    match time {
        Some(t) => {
            let dt: chrono::DateTime<chrono::Local> = chrono::DateTime::from(t);
            dt.format("%Y-%m-%d %H:%M").to_string()
        }
        None => String::new(),
    }
}

#[derive(Clone)]
pub struct RemoteSessionState {
    pub label: String,
    pub config: SftpTargetConfig,
    pub client: Arc<Mutex<RemoteSftpClient>>,
    pub is_connected: bool,
    pub is_loading: bool,
    pub current_path: PathBuf,
    pub path_input: String,
    pub path_history: Vec<PathBuf>,
    pub remote_raw_entries: Vec<FileEntry>,
    pub remote_home_dir: Option<PathBuf>,
    pub last_synced_terminal_cwd: Option<PathBuf>,
    pub selected_file: Option<PathBuf>,
    pub remote_disk_stats: Option<(u64, u64, f32)>,
    pub remote_telemetry: Option<crate::telemetry::RemoteSystemMetrics>,
    pub last_telemetry_poll: Option<Instant>,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalOpenMode {
    TextEditor,
    OpenWithDialog,
    DefaultProgram,
}

pub struct FileExplorer {
    pub current_path: PathBuf,
    pub path_input: String,
    pub path_history: Vec<PathBuf>,
    pub entries: Vec<FileEntry>,
    pub selected_file: Option<PathBuf>,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    // Remote SFTP State & Multi-Server Pool
    pub is_remote: bool,
    pub is_loading: bool,
    pub remote_label: Option<String>,
    pub remote_client: Option<Arc<Mutex<RemoteSftpClient>>>,
    pub remote_disk_stats: Option<(u64, u64, f32)>,
    pub remote_telemetry: Option<crate::telemetry::RemoteSystemMetrics>,
    pub remote_raw_entries: Vec<FileEntry>,
    pub remote_home_dir: Option<PathBuf>,
    pub last_synced_terminal_cwd: Option<PathBuf>,
    pub remote_sessions: HashMap<String, RemoteSessionState>,
    // View & RustyBox SFTP options
    pub show_hidden: bool,
    pub sort_column: SftpSortColumn,
    pub sort_ascending: bool,
    pub follow_terminal_folder: bool,
    pub remote_monitoring_enabled: bool,
    pub active_remote_label: Option<String>,
    // Modal quick editor
    pub editor_open: bool,
    pub editor_file_path: Option<PathBuf>,
    pub editor_remote_label: Option<String>,
    pub editor_content: String,
    pub editor_status: Option<String>,
    // Modal Chmod (RustyBox TFORMCHMOD)
    pub chmod_open: bool,
    pub chmod_target: Option<PathBuf>,
    pub chmod_remote_label: Option<String>,
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
    // Modal rename dialog
    pub show_rename: bool,
    pub rename_target: Option<PathBuf>,
    pub rename_new_name: String,
    // Modal properties dialog
    pub properties_open: bool,
    pub properties_target: Option<FileEntry>,
    // Terminal cd request
    pub requested_cd_terminal: Option<String>,
    // Terminal paste path request (MobaXterm middle click / menu)
    pub requested_terminal_paste: Option<String>,
    // External open request
    pub pending_external_open: Option<(PathBuf, ExternalOpenMode)>,
    // Diff / Compare Viewer
    pub diff_open: bool,
    pub diff_file_a: String,
    pub diff_file_b: String,
    pub diff_content_a: String,
    pub diff_content_b: String,
    // Transfer progress & Conflict resolution
    pub transfer_progress: Option<TransferProgressInfo>,
    pub pending_conflict: Option<ConflictItem>,
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
            remote_telemetry: None,
            remote_raw_entries: Vec::new(),
            remote_home_dir: None,
            last_synced_terminal_cwd: None,
            remote_sessions: HashMap::new(),
            show_hidden: true,
            sort_column: SftpSortColumn::Name,
            sort_ascending: true,
            follow_terminal_folder: false,
            remote_monitoring_enabled: true,
            active_remote_label: None,
            editor_open: false,
            editor_file_path: None,
            editor_remote_label: None,
            editor_content: String::new(),
            editor_status: None,
            chmod_open: false,
            chmod_target: None,
            chmod_remote_label: None,
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
            show_rename: false,
            rename_target: None,
            rename_new_name: String::new(),
            properties_open: false,
            properties_target: None,
            requested_cd_terminal: None,
            requested_terminal_paste: None,
            pending_external_open: None,
            diff_open: false,
            diff_file_a: String::new(),
            diff_file_b: String::new(),
            diff_content_a: String::new(),
            diff_content_b: String::new(),
            transfer_progress: None,
            pending_conflict: None,
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

    pub fn save_active_remote_state(&mut self) {
        if !self.is_remote {
            return;
        }
        let Some(ref label) = self.remote_label else {
            return;
        };
        if let Some(sess) = self.remote_sessions.get_mut(label) {
            sess.is_loading = self.is_loading;
            sess.current_path = self.current_path.clone();
            sess.path_input = self.path_input.clone();
            sess.path_history = self.path_history.clone();
            sess.remote_raw_entries = self.remote_raw_entries.clone();
            sess.remote_home_dir = self.remote_home_dir.clone();
            sess.last_synced_terminal_cwd = self.last_synced_terminal_cwd.clone();
            sess.selected_file = self.selected_file.clone();
            sess.remote_disk_stats = self.remote_disk_stats;
            sess.remote_telemetry = self.remote_telemetry.clone();
            sess.error_message = self.error_message.clone();
            sess.status_message = self.status_message.clone();
        }
    }

    pub fn load_active_remote_state(&mut self, label: &str) {
        if let Some(sess) = self.remote_sessions.get(label).cloned() {
            self.is_remote = true;
            self.remote_label = Some(label.to_string());
            self.active_remote_label = Some(label.to_string());
            self.remote_client = Some(sess.client);
            self.is_loading = sess.is_loading;
            self.current_path = sess.current_path;
            self.path_input = sess.path_input;
            self.path_history = sess.path_history;
            self.remote_raw_entries = sess.remote_raw_entries;
            self.remote_home_dir = sess.remote_home_dir;
            self.last_synced_terminal_cwd = sess.last_synced_terminal_cwd;
            self.selected_file = sess.selected_file;
            self.remote_disk_stats = sess.remote_disk_stats;
            self.remote_telemetry = sess.remote_telemetry;
            self.error_message = sess.error_message;
            self.status_message = sess.status_message;
            self.apply_remote_filter_and_sort();
        }
    }

    pub fn connect_remote(&mut self, cfg: SftpTargetConfig) {
        let label = format!("{}@{}:{}", cfg.user, cfg.host, cfg.port);
        if self.is_remote && self.remote_label.as_deref() == Some(&label) {
            return;
        }

        // Persist the outgoing remote session state before switching
        self.save_active_remote_state();

        if let Some(existing) = self.remote_sessions.get_mut(&label) {
            // If credentials changed (e.g., Vault unlocked) and previous connection attempt failed, retry connect
            if !existing.is_connected && !existing.is_loading && existing.config != cfg {
                existing.config = cfg.clone();
                existing.is_loading = true;
                existing.error_message = None;
                existing.client.lock().connect(cfg);
            }
            self.load_active_remote_state(&label);
            return;
        }

        let default_dir = if cfg.user == "root" {
            PathBuf::from("/root")
        } else {
            PathBuf::from(format!("/home/{}", cfg.user))
        };
        let path_input = Self::format_display_path(&default_dir);

        let mut client_inner = RemoteSftpClient::new();
        client_inner.connect(cfg.clone());
        let client_arc = Arc::new(Mutex::new(client_inner));

        let new_session = RemoteSessionState {
            label: label.clone(),
            config: cfg,
            client: client_arc,
            is_connected: false,
            is_loading: true,
            current_path: default_dir.clone(),
            path_input,
            path_history: vec![default_dir.clone()],
            remote_raw_entries: Vec::new(),
            remote_home_dir: Some(default_dir),
            last_synced_terminal_cwd: None,
            selected_file: None,
            remote_disk_stats: None,
            remote_telemetry: None,
            last_telemetry_poll: None,
            error_message: None,
            status_message: None,
        };

        self.remote_sessions.insert(label.clone(), new_session);
        self.load_active_remote_state(&label);
    }

    pub fn disconnect_remote(&mut self) {
        if let Some(label) = self.remote_label.take() {
            if let Some(sess) = self.remote_sessions.remove(&label) {
                sess.client.lock().disconnect();
            }
        } else if let Some(ref client) = self.remote_client {
            client.lock().disconnect();
        }
        self.remote_client = None;
        self.is_remote = false;
        self.is_loading = false;
        self.active_remote_label = None;
        self.remote_disk_stats = None;
        self.remote_telemetry = None;
        self.remote_raw_entries.clear();
        self.remote_home_dir = None;
        self.last_synced_terminal_cwd = None;
        self.current_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        self.path_input = Self::format_display_path(&self.current_path);
        self.refresh();
    }

    pub fn set_sort_column(&mut self, col: SftpSortColumn) {
        if self.sort_column == col {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_column = col;
            self.sort_ascending = true;
        }
        if self.is_remote && !self.remote_raw_entries.is_empty() {
            self.apply_remote_filter_and_sort();
            self.save_active_remote_state();
        } else {
            self.refresh();
        }
    }

    pub fn apply_remote_filter_and_sort(&mut self) {
        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for item in &self.remote_raw_entries {
            if !self.show_hidden && item.is_hidden {
                continue;
            }
            if item.is_dir {
                dirs.push(item.clone());
            } else {
                files.push(item.clone());
            }
        }

        let asc = self.sort_ascending;
        let col = self.sort_column;
        let cmp_fn = |a: &FileEntry, b: &FileEntry| -> std::cmp::Ordering {
            let ordering = match col {
                SftpSortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SftpSortColumn::Size => a.size.cmp(&b.size),
                SftpSortColumn::Modified => match (a.modified, b.modified) {
                    (Some(ma), Some(mb)) => ma.cmp(&mb),
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (None, None) => std::cmp::Ordering::Equal,
                },
                SftpSortColumn::Owner => a.owner.to_lowercase().cmp(&b.owner.to_lowercase()),
                SftpSortColumn::Group => a.group.to_lowercase().cmp(&b.group.to_lowercase()),
                SftpSortColumn::Access => a.permissions.cmp(&b.permissions),
            };
            if asc {
                ordering
            } else {
                ordering.reverse()
            }
        };

        dirs.sort_by(cmp_fn);
        files.sort_by(cmp_fn);

        self.entries.clear();
        self.entries.extend(dirs);
        self.entries.extend(files);

        if let Some(ref sel) = self.selected_file {
            if !self.entries.iter().any(|e| &e.path == sel) {
                self.selected_file = None;
            }
        }
    }

    pub fn toggle_show_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        if self.is_remote && !self.remote_raw_entries.is_empty() {
            self.apply_remote_filter_and_sort();
            self.save_active_remote_state();
        } else {
            self.refresh();
        }
    }

    pub fn toggle_sort_order(&mut self) {
        self.sort_ascending = !self.sort_ascending;
        if self.is_remote && !self.remote_raw_entries.is_empty() {
            self.apply_remote_filter_and_sort();
            self.save_active_remote_state();
        } else {
            self.refresh();
        }
    }

    pub fn poll(&mut self) {
        self.save_active_remote_state();

        let mut opened_editor: Option<(String, PathBuf, String)> = None;
        let mut editor_saved_target: Option<String> = None;

        for (label, sess) in self.remote_sessions.iter_mut() {
            let mut events = Vec::new();
            {
                let c = sess.client.lock();
                while let Some(evt) = c.try_recv() {
                    events.push(evt);
                }
            }

            let mut needs_dir_refresh = false;

            for evt in events {
                match evt {
                    SftpEvent::Connected { initial_dir, .. } => {
                        sess.is_connected = true;
                        sess.is_loading = false;
                        let init_path = PathBuf::from(&initial_dir);
                        sess.remote_home_dir = Some(init_path.clone());
                        sess.current_path = init_path.clone();
                        sess.path_input = Self::format_display_path(&sess.current_path);
                        if !sess.path_history.contains(&sess.current_path) {
                            sess.path_history.insert(0, sess.current_path.clone());
                        }
                    }
                    SftpEvent::ConnectionFailed(err) => {
                        sess.is_connected = false;
                        sess.error_message = Some(format!("SFTP Error: {}", err));
                        sess.is_loading = false;
                    }
                    SftpEvent::DirListed { dir, entries } => {
                        sess.is_connected = true;
                        sess.current_path = PathBuf::from(&dir);
                        sess.path_input = Self::format_display_path(&sess.current_path);
                        sess.remote_raw_entries = entries;
                        sess.is_loading = false;
                        sess.error_message = None;
                        if !sess.path_history.contains(&sess.current_path) {
                            sess.path_history.insert(0, sess.current_path.clone());
                            if sess.path_history.len() > 12 {
                                sess.path_history.truncate(12);
                            }
                        }
                    }
                    SftpEvent::TransferProgress(info) => {
                        sess.is_loading = true;
                        if self.remote_label.as_deref() == Some(label.as_str()) {
                            self.transfer_progress = Some(info);
                        }
                    }
                    SftpEvent::TransferFinished(msg) => {
                        sess.status_message = Some(msg);
                        sess.is_loading = false;
                        sess.error_message = None;
                        if self.remote_label.as_deref() == Some(label.as_str()) {
                            self.transfer_progress = None;
                        }
                        needs_dir_refresh = true;
                    }
                    SftpEvent::FileRead { path, content } => {
                        sess.is_loading = false;
                        if self.diff_open && self.diff_content_a.starts_with("Loading remote") {
                            self.diff_content_a = content;
                        } else {
                            opened_editor = Some((label.clone(), PathBuf::from(path), content));
                        }
                    }
                    SftpEvent::FileWritten { .. } => {
                        sess.is_loading = false;
                        editor_saved_target = Some(label.clone());
                        needs_dir_refresh = true;
                    }
                    SftpEvent::OperationSuccess(msg) => {
                        sess.status_message = Some(msg);
                        sess.is_loading = false;
                        sess.error_message = None;
                        if let Some((ref p, mode)) = self.pending_external_open.take() {
                            if p.exists() {
                                Self::launch_external_file(p, mode);
                            }
                        }
                        needs_dir_refresh = true;
                    }
                    SftpEvent::OperationError(err) => {
                        sess.error_message = Some(err);
                        sess.is_loading = false;
                        if self.remote_label.as_deref() == Some(label.as_str()) {
                            self.transfer_progress = None;
                        }
                    }
                    SftpEvent::DiskUsage { used, total, pct } => {
                        sess.remote_disk_stats = Some((used, total, pct));
                    }
                    SftpEvent::Telemetry(metrics) => {
                        sess.remote_telemetry = Some(metrics.clone());
                        if self.remote_label.as_deref() == Some(label.as_str()) {
                            self.remote_telemetry = Some(metrics);
                        }
                    }
                }
            }

            if needs_dir_refresh {
                sess.is_loading = true;
                let path_str = sess.current_path.to_string_lossy().to_string();
                sess.client.lock().list_dir(&path_str);
            }
        }

        if self.remote_monitoring_enabled {
            let now = Instant::now();
            let is_transferring = self.transfer_progress.is_some();
            for (_label, sess) in self.remote_sessions.iter_mut() {
                if sess.is_connected && !is_transferring {
                    let should_poll = sess.last_telemetry_poll
                        .map(|t| now.duration_since(t) >= Duration::from_millis(2500))
                        .unwrap_or(true);
                    if should_poll {
                        sess.client.lock().fetch_telemetry();
                        sess.last_telemetry_poll = Some(now);
                    }
                }
            }
        }

        if let Some((label, path, content)) = opened_editor {
            self.editor_remote_label = Some(label);
            self.editor_file_path = Some(path);
            self.editor_content = content;
            self.editor_status = None;
            self.editor_open = true;
        }
        if let Some(target_label) = editor_saved_target {
            self.editor_status = Some(format!("Saved successfully to {}!", target_label));
        }

        if self.is_remote {
            if let Some(active_label) = self.remote_label.clone() {
                self.load_active_remote_state(&active_label);
            }
        }
    }

    pub fn get_active_telemetry(&self) -> Option<crate::telemetry::RemoteSystemMetrics> {
        if self.is_remote {
            if let Some(ref label) = self.remote_label {
                if let Some(sess) = self.remote_sessions.get(label) {
                    return sess.remote_telemetry.clone();
                }
            }
            return self.remote_telemetry.clone();
        }
        None
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

    pub fn sync_terminal_cwd(&mut self, target_dir: &str) {
        if !self.follow_terminal_folder {
            return;
        }
        let clean = target_dir.trim();
        if clean.is_empty() {
            return;
        }

        let default_home = || -> String {
            if let Some(ref h) = self.remote_home_dir {
                h.to_string_lossy().to_string()
            } else if let Some(ref lbl) = self.remote_label {
                if lbl.starts_with("root@") {
                    "/root".to_string()
                } else if let Some((u, _)) = lbl.split_once('@') {
                    format!("/home/{}", u)
                } else {
                    "/root".to_string()
                }
            } else {
                "/root".to_string()
            }
        };

        if self.is_remote {
            let resolved = if clean == "~" {
                default_home()
            } else if let Some(sub) = clean.strip_prefix("~/") {
                let home = default_home();
                format!("{}/{}", home.trim_end_matches('/'), sub)
            } else if clean.starts_with('/') {
                clean.to_string()
            } else {
                return;
            };

            let norm_resolved = if resolved.len() > 1 && resolved.ends_with('/') {
                resolved.trim_end_matches('/').to_string()
            } else {
                resolved
            };
            let cur_str = self.current_path.to_string_lossy().to_string();
            let norm_cur = if cur_str.len() > 1 && cur_str.ends_with('/') {
                cur_str.trim_end_matches('/').to_string()
            } else {
                cur_str
            };

            if norm_resolved != norm_cur {
                self.set_path(PathBuf::from(norm_resolved));
            }
        } else {
            let target_path = PathBuf::from(clean);
            if target_path.is_dir() && target_path != self.current_path {
                self.set_path(target_path);
            }
        }
    }

    pub fn extract_cwd_from_prompt(prompt_line: &str, home_dir: Option<&Path>) -> Option<PathBuf> {
        let trimmed = prompt_line.trim_end();
        let without_sigil = trimmed
            .strip_suffix('$')
            .or_else(|| trimmed.strip_suffix('#'))
            .or_else(|| trimmed.strip_suffix('%'))?
            .trim_end();

        // Guard against commands typed after a prompt that also happen to end in $/#!/%
        if without_sigil.contains("$ ") || without_sigil.contains("# ") || without_sigil.contains("% ") {
            return None;
        }

        let raw_dir = if let Some(without_bracket) = without_sigil.strip_suffix(']') {
            let (_, dir_part) = without_bracket.rsplit_once(' ')?;
            dir_part.trim()
        } else {
            let (_, dir_part) = without_sigil.rsplit_once(':')?;
            dir_part.trim()
        };

        if raw_dir.is_empty()
            || raw_dir.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '$' | ';' | '|'))
        {
            return None;
        }

        if raw_dir == "~" {
            return home_dir.map(|h| h.to_path_buf());
        }
        if let Some(sub) = raw_dir.strip_prefix("~/") {
            return home_dir.map(|h| h.join(sub));
        }
        if raw_dir.starts_with('/') {
            return Some(PathBuf::from(raw_dir));
        }
        None
    }

    pub fn sync_remote_prompt_cwd(&mut self, prompt_line: &str) {
        if !self.follow_terminal_folder || !self.is_remote {
            return;
        }
        if let Some(target) = Self::extract_cwd_from_prompt(prompt_line, self.remote_home_dir.as_deref()) {
            if self.last_synced_terminal_cwd.as_ref() != Some(&target) {
                self.last_synced_terminal_cwd = Some(target.clone());
                if target != self.current_path {
                    self.set_path(target);
                }
            }
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
            self.save_active_remote_state();
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
                        let (owner, group) = Self::get_local_owner_group(&metadata);

                        let item = FileEntry {
                            name,
                            path,
                            is_dir,
                            is_hidden,
                            size,
                            modified,
                            permissions,
                            owner,
                            group,
                        };

                        if is_dir {
                            dirs.push(item);
                        } else {
                            files.push(item);
                        }
                    }
                }

                let asc = self.sort_ascending;
                let col = self.sort_column;
                let cmp_fn = |a: &FileEntry, b: &FileEntry| -> std::cmp::Ordering {
                    let ordering = match col {
                        SftpSortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                        SftpSortColumn::Size => a.size.cmp(&b.size),
                        SftpSortColumn::Modified => match (a.modified, b.modified) {
                            (Some(ma), Some(mb)) => ma.cmp(&mb),
                            (Some(_), None) => std::cmp::Ordering::Greater,
                            (None, Some(_)) => std::cmp::Ordering::Less,
                            (None, None) => std::cmp::Ordering::Equal,
                        },
                        SftpSortColumn::Owner => a.owner.to_lowercase().cmp(&b.owner.to_lowercase()),
                        SftpSortColumn::Group => a.group.to_lowercase().cmp(&b.group.to_lowercase()),
                        SftpSortColumn::Access => a.permissions.cmp(&b.permissions),
                    };
                    if asc {
                        ordering
                    } else {
                        ordering.reverse()
                    }
                };

                dirs.sort_by(cmp_fn);
                files.sort_by(cmp_fn);

                self.entries.extend(dirs);
                self.entries.extend(files);
            }
            Err(e) => {
                self.error_message = Some(format!("Cannot read dir: {}", e));
            }
        }
    }

    fn get_local_owner_group(meta: &Option<fs::Metadata>) -> (String, String) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Some(m) = meta {
                let uid = m.uid();
                let gid = m.gid();
                let u = if uid == 0 {
                    "root".to_string()
                } else {
                    std::env::var("USER").unwrap_or_else(|_| uid.to_string())
                };
                let g = if gid == 0 {
                    "root".to_string()
                } else {
                    std::env::var("USER").unwrap_or_else(|_| gid.to_string())
                };
                return (u, g);
            }
        }
        let user = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "user".to_string());
        (user.clone(), user)
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

    pub fn can_edit_selected(&self) -> bool {
        let Some(ref selected) = self.selected_file else {
            return false;
        };
        if let Some(entry) = self.entries.iter().find(|e| &e.path == selected) {
            return !entry.is_dir;
        }
        if let Some(entry) = self.remote_raw_entries.iter().find(|e| &e.path == selected) {
            return !entry.is_dir;
        }
        !self.is_remote && selected.is_file()
    }

    pub fn open_editor(&mut self, path: PathBuf) {
        if self.entries.iter().any(|e| e.path == path && e.is_dir)
            || self.remote_raw_entries.iter().any(|e| e.path == path && e.is_dir)
        {
            return;
        }
        if self.is_remote {
            let path_str = path.to_string_lossy().to_string();
            if let Some(ref client) = self.remote_client {
                self.is_loading = true;
                client.lock().read_file(&path_str);
            }
            self.save_active_remote_state();
            return;
        }

        const MAX_EDITOR_BYTES: u64 = 2 * 1024 * 1024;
        if let Ok(meta) = fs::metadata(&path) {
            if meta.is_dir() {
                return;
            }
            if meta.len() > MAX_EDITOR_BYTES {
                self.error_message = Some(
                    "File is too large for Quick Editor (> 2 MB).".to_string(),
                );
                return;
            }
        }

        match fs::read_to_string(&path) {
            Ok(content) => {
                self.editor_remote_label = None;
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
            if let Some(ref target_label) = self.editor_remote_label {
                let path_str = path.to_string_lossy().to_string();
                if let Some(sess) = self.remote_sessions.get_mut(target_label) {
                    sess.is_loading = true;
                    sess.client.lock().write_file(&path_str, &self.editor_content);
                }
                if self.remote_label.as_deref() == Some(target_label.as_str()) {
                    self.is_loading = true;
                }
                return;
            }
            if self.is_remote {
                let path_str = path.to_string_lossy().to_string();
                if let Some(ref client) = self.remote_client {
                    self.is_loading = true;
                    client.lock().write_file(&path_str, &self.editor_content);
                }
                self.save_active_remote_state();
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
            let is_dir = self
                .entries
                .iter()
                .find(|e| e.path == path)
                .or_else(|| self.remote_raw_entries.iter().find(|e| e.path == path))
                .map(|e| e.is_dir)
                .unwrap_or(false);
            let path_str = path.to_string_lossy().to_string();
            if let Some(ref client) = self.remote_client {
                self.is_loading = true;
                client.lock().delete(&path_str, is_dir);
            }
            if self.selected_file.as_deref() == Some(path) {
                self.selected_file = None;
            }
            self.save_active_remote_state();
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
                self.is_loading = true;
                client.lock().create_dir(&remote_p);
            }
            self.save_active_remote_state();
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
                self.is_loading = true;
                client.lock().create_file(&remote_p);
            }
            self.save_active_remote_state();
            return;
        }

        let new_path = self.current_path.join(name);
        if let Err(e) = fs::write(&new_path, "") {
            self.error_message = Some(format!("Create file failed: {}", e));
        } else {
            self.refresh();
        }
    }

    pub fn cancel_current_transfer(&mut self) {
        if let Some(ref client) = self.remote_client {
            client.lock().cancel_transfer();
        }
        self.transfer_progress = None;
        self.status_message = Some("Transfer cancelled.".into());
    }

    pub fn upload_file(&mut self, local_path: PathBuf) {
        let Some(file_name) = local_path.file_name() else {
            return;
        };
        let name_str = file_name.to_string_lossy().to_string();
        let curr = self.current_path.to_string_lossy().to_string();
        let remote_p = if curr.ends_with('/') {
            format!("{}{}", curr, name_str)
        } else {
            format!("{}/{}", curr, name_str)
        };
        let remote_path_buf = PathBuf::from(&remote_p);

        // Conflict check: if file already exists in current remote folder, ask user!
        let exists_remote = self.entries.iter().any(|e| e.name == name_str && !e.is_dir)
            || self.remote_raw_entries.iter().any(|e| e.name == name_str && !e.is_dir);

        if exists_remote {
            self.pending_conflict = Some(ConflictItem {
                is_upload: true,
                src_path: local_path,
                dest_path: remote_path_buf,
                file_name: name_str,
            });
            return;
        }

        self.execute_upload(local_path, remote_p);
    }

    pub fn upload_folder(&mut self, local_path: PathBuf) {
        let Some(folder_name) = local_path.file_name() else {
            return;
        };
        let name_str = folder_name.to_string_lossy().to_string();
        let curr = self.current_path.to_string_lossy().to_string();
        let remote_p = if curr.ends_with('/') {
            format!("{}{}", curr, name_str)
        } else {
            format!("{}/{}", curr, name_str)
        };
        self.execute_upload(local_path, remote_p);
    }

    pub fn execute_upload(&mut self, local_path: PathBuf, remote_p: String) {
        if let Some(ref client) = self.remote_client {
            self.is_loading = true;
            client.lock().upload(local_path, remote_p);
        }
        self.save_active_remote_state();
    }

    pub fn download_file(&mut self, remote_path: &Path, local_dest: PathBuf) {
        let name_str = remote_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "downloaded_file".to_string());

        // Conflict check: if local destination already exists, prompt user!
        if local_dest.exists() {
            self.pending_conflict = Some(ConflictItem {
                is_upload: false,
                src_path: remote_path.to_path_buf(),
                dest_path: local_dest,
                file_name: name_str,
            });
            return;
        }

        self.execute_download(remote_path, local_dest);
    }

    pub fn execute_download(&mut self, remote_path: &Path, local_dest: PathBuf) {
        let remote_str = remote_path.to_string_lossy().to_string();
        if let Some(ref client) = self.remote_client {
            self.is_loading = true;
            client.lock().download(remote_str, local_dest);
        }
        self.save_active_remote_state();
    }

    pub fn resolve_conflict_overwrite(&mut self) {
        if let Some(conflict) = self.pending_conflict.take() {
            if conflict.is_upload {
                self.execute_upload(conflict.src_path, conflict.dest_path.to_string_lossy().to_string());
            } else {
                self.execute_download(&conflict.src_path, conflict.dest_path);
            }
        }
    }

    pub fn resolve_conflict_rename(&mut self) {
        if let Some(conflict) = self.pending_conflict.take() {
            let parent = conflict.dest_path.parent().unwrap_or_else(|| Path::new("."));
            let stem = Path::new(&conflict.file_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("file");
            let ext = Path::new(&conflict.file_name)
                .extension()
                .and_then(|e| e.to_str());

            // Generate unique name like `name (1).ext`
            let mut counter = 1;
            loop {
                let new_name = match ext {
                    Some(e) => format!("{} ({}).{}", stem, counter, e),
                    None => format!("{} ({})", stem, counter),
                };
                let candidate = parent.join(&new_name);

                let collision = if conflict.is_upload {
                    self.entries.iter().any(|e| e.name == new_name)
                        || self.remote_raw_entries.iter().any(|e| e.name == new_name)
                } else {
                    candidate.exists()
                };

                if !collision {
                    if conflict.is_upload {
                        self.execute_upload(conflict.src_path, candidate.to_string_lossy().to_string());
                    } else {
                        self.execute_download(&conflict.src_path, candidate);
                    }
                    break;
                }
                counter += 1;
            }
        }
    }

    pub fn resolve_conflict_skip(&mut self) {
        self.pending_conflict = None;
    }

    pub fn parse_permissions_str(perms: &str) -> Option<[bool; 9]> {
        let bytes = perms.as_bytes();
        if bytes.len() < 10 {
            return None;
        }
        Some([
            bytes[1] == b'r',
            bytes[2] == b'w',
            bytes[3] == b'x' || bytes[3] == b's' || bytes[3] == b'S',
            bytes[4] == b'r',
            bytes[5] == b'w',
            bytes[6] == b'x' || bytes[6] == b's' || bytes[6] == b'S',
            bytes[7] == b'r',
            bytes[8] == b'w',
            bytes[9] == b'x' || bytes[9] == b't' || bytes[9] == b'T',
        ])
    }

    pub fn open_chmod(&mut self, path: PathBuf) {
        let perm_str = self
            .entries
            .iter()
            .find(|e| e.path == path)
            .or_else(|| self.remote_raw_entries.iter().find(|e| e.path == path))
            .map(|e| e.permissions.clone())
            .or_else(|| {
                if !self.is_remote {
                    fs::metadata(&path)
                        .ok()
                        .map(|m| Self::format_permissions(Some(&m), m.is_dir()))
                } else {
                    None
                }
            });

        if let Some(p_str) = perm_str {
            if let Some(bits) = Self::parse_permissions_str(&p_str) {
                self.chmod_u_r = bits[0];
                self.chmod_u_w = bits[1];
                self.chmod_u_x = bits[2];
                self.chmod_g_r = bits[3];
                self.chmod_g_w = bits[4];
                self.chmod_g_x = bits[5];
                self.chmod_o_r = bits[6];
                self.chmod_o_w = bits[7];
                self.chmod_o_x = bits[8];
            }
        }

        self.chmod_remote_label = if self.is_remote {
            self.remote_label.clone()
        } else {
            None
        };
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

            if let Some(ref target_label) = self.chmod_remote_label {
                let path_str = path.to_string_lossy().to_string();
                if let Some(sess) = self.remote_sessions.get_mut(target_label) {
                    sess.is_loading = true;
                    sess.client.lock().chmod(&path_str, mode);
                }
                if self.remote_label.as_deref() == Some(target_label.as_str()) {
                    self.is_loading = true;
                }
                self.chmod_open = false;
                return;
            }

            if self.is_remote {
                let path_str = path.to_string_lossy().to_string();
                if let Some(ref client) = self.remote_client {
                    self.is_loading = true;
                    client.lock().chmod(&path_str, mode);
                }
                self.save_active_remote_state();
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

    pub fn open_rename(&mut self, path: PathBuf) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        self.rename_new_name = name;
        self.rename_target = Some(path);
        self.show_rename = true;
    }

    pub fn rename_entry(&mut self, old_path: &Path, new_name: &str) {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            self.error_message = Some("Name cannot be empty".to_string());
            return;
        }
        let parent = old_path.parent().unwrap_or(&self.current_path);
        let new_path = parent.join(trimmed);
        if self.is_remote {
            if let Some(ref client) = self.remote_client {
                self.is_loading = true;
                client.lock().rename(
                    old_path.to_string_lossy().to_string(),
                    new_path.to_string_lossy().to_string(),
                );
            }
        } else {
            match fs::rename(old_path, &new_path) {
                Ok(_) => {
                    self.status_message = Some("Renamed successfully".to_string());
                    self.refresh();
                }
                Err(e) => {
                    self.error_message = Some(format!("Rename failed: {}", e));
                }
            }
        }
    }

    pub fn open_properties(&mut self, entry: FileEntry) {
        self.properties_target = Some(entry);
        self.properties_open = true;
    }

    pub fn open_external(&mut self, remote_path: &Path, mode: ExternalOpenMode) {
        let file_name = remote_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "temp_file".to_string());
        let temp_cache_dir = std::env::temp_dir().join("rustybox_cache");
        let _ = fs::create_dir_all(&temp_cache_dir);
        let local_dest = temp_cache_dir.join(&file_name);

        if self.is_remote {
            self.pending_external_open = Some((local_dest.clone(), mode));
            self.download_file(remote_path, local_dest);
        } else {
            Self::launch_external_file(remote_path, mode);
        }
    }

    pub fn launch_external_file(local_path: &Path, mode: ExternalOpenMode) {
        let path_str = local_path.to_string_lossy().to_string();
        #[cfg(target_os = "windows")]
        {
            match mode {
                ExternalOpenMode::TextEditor => {
                    let _ = std::process::Command::new("notepad.exe").arg(&path_str).spawn();
                }
                ExternalOpenMode::OpenWithDialog => {
                    let _ = std::process::Command::new("rundll32.exe")
                        .args(["shell32.dll,OpenAs_RunDLL", &path_str])
                        .spawn();
                }
                ExternalOpenMode::DefaultProgram => {
                    use std::os::windows::process::CommandExt;
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", &path_str])
                        .creation_flags(0x08000000)
                        .spawn();
                }
            }
        }
        #[cfg(target_os = "macos")]
        {
            match mode {
                ExternalOpenMode::TextEditor => {
                    let _ = std::process::Command::new("open").args(["-e", &path_str]).spawn();
                }
                _ => {
                    let _ = std::process::Command::new("open").arg(&path_str).spawn();
                }
            }
        }
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            match mode {
                ExternalOpenMode::TextEditor => {
                    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "xdg-open".to_string());
                    let _ = std::process::Command::new(editor).arg(&path_str).spawn();
                }
                _ => {
                    let _ = std::process::Command::new("xdg-open").arg(&path_str).spawn();
                }
            }
        }
    }

    pub fn open_compare(&mut self, file_a_path: &Path, file_b_path: PathBuf) {
        let content_b = match fs::read_to_string(&file_b_path) {
            Ok(c) => c,
            Err(e) => format!("[Cannot read comparison file: {}]", e),
        };
        let file_a_name = file_a_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "File A".to_string());
        let file_b_name = file_b_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "File B".to_string());

        if self.is_remote {
            if let Some(ref client) = self.remote_client {
                let path_str = file_a_path.to_string_lossy().to_string();
                self.is_loading = true;
                client.lock().read_file(&path_str);
                self.diff_content_a = format!("Loading remote file {}...", path_str);
            }
        } else {
            self.diff_content_a = match fs::read_to_string(file_a_path) {
                Ok(c) => c,
                Err(e) => format!("[Cannot read file: {}]", e),
            };
        }
        self.diff_content_b = content_b;
        self.diff_file_a = file_a_name;
        self.diff_file_b = file_b_name;
        self.diff_open = true;
    }

    pub fn upload_file_to(&mut self, local_path: PathBuf, target_dir: &Path) {
        if let Some(file_name) = local_path.file_name() {
            let name_str = file_name.to_string_lossy().to_string();
            let curr = target_dir.to_string_lossy().to_string();
            let remote_p = if curr.ends_with('/') {
                format!("{}{}", curr, name_str)
            } else {
                format!("{}/{}", curr, name_str)
            };
            if self.is_remote {
                if let Some(ref client) = self.remote_client {
                    self.is_loading = true;
                    client.lock().upload(local_path, remote_p);
                }
            } else {
                let dest = target_dir.join(file_name);
                match fs::copy(&local_path, &dest) {
                    Ok(_) => {
                        self.status_message = Some("Copied locally".to_string());
                        self.refresh();
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Copy failed: {}", e));
                    }
                }
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sftp::remote::{candidate_ssh_key_paths, expand_tilde_path};

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

    #[test]
    fn test_remote_filter_hidden_and_sort_order() {
        let mut explorer = FileExplorer::new();
        explorer.is_remote = true;
        explorer.show_hidden = true;
        explorer.sort_ascending = true;

        explorer.remote_raw_entries = vec![
            FileEntry {
                name: "zeta.txt".into(),
                path: PathBuf::from("/root/zeta.txt"),
                is_dir: false,
                is_hidden: false,
                size: 120,
                modified: None,
                permissions: "-rw-r--r--".into(),
                owner: "root".into(),
                group: "root".into(),
            },
            FileEntry {
                name: ".bashrc".into(),
                path: PathBuf::from("/root/.bashrc"),
                is_dir: false,
                is_hidden: true,
                size: 45,
                modified: None,
                permissions: "-rw-------".into(),
                owner: "root".into(),
                group: "root".into(),
            },
            FileEntry {
                name: "alpha".into(),
                path: PathBuf::from("/root/alpha"),
                is_dir: true,
                is_hidden: false,
                size: 4096,
                modified: None,
                permissions: "drwxr-xr-x".into(),
                owner: "root".into(),
                group: "root".into(),
            },
            FileEntry {
                name: ".ssh".into(),
                path: PathBuf::from("/root/.ssh"),
                is_dir: true,
                is_hidden: true,
                size: 4096,
                modified: None,
                permissions: "drwx------".into(),
                owner: "root".into(),
                group: "root".into(),
            },
        ];

        explorer.apply_remote_filter_and_sort();
        let names: Vec<&str> = explorer.entries.iter().map(|e| e.name.as_str()).collect();
        // Directories first (ascending), then files (ascending)
        assert_eq!(names, vec![".ssh", "alpha", ".bashrc", "zeta.txt"]);

        // Toggle hidden off -> dotfiles/dotfolders must be hidden immediately
        explorer.toggle_show_hidden();
        assert!(!explorer.show_hidden);
        let names_no_hidden: Vec<&str> = explorer.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names_no_hidden, vec!["alpha", "zeta.txt"]);

        // Toggle hidden back on and reverse sort order (Z-A)
        explorer.toggle_show_hidden();
        explorer.toggle_sort_order();
        assert!(!explorer.sort_ascending);
        let names_desc: Vec<&str> = explorer.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names_desc, vec!["alpha", ".ssh", "zeta.txt", ".bashrc"]);
    }

    #[test]
    fn test_can_edit_selected_for_remote_entries() {
        let mut explorer = FileExplorer::new();
        explorer.is_remote = true;
        let remote_file = PathBuf::from("/nonexistent_local_path_on_vps/nginx.conf");
        let remote_dir = PathBuf::from("/nonexistent_local_path_on_vps/conf.d");

        explorer.entries = vec![
            FileEntry {
                name: "conf.d".into(),
                path: remote_dir.clone(),
                is_dir: true,
                is_hidden: false,
                size: 4096,
                modified: None,
                permissions: "drwxr-xr-x".into(),
                owner: "root".into(),
                group: "root".into(),
            },
            FileEntry {
                name: "nginx.conf".into(),
                path: remote_file.clone(),
                is_dir: false,
                is_hidden: false,
                size: 1024,
                modified: None,
                permissions: "-rw-r--r--".into(),
                owner: "root".into(),
                group: "root".into(),
            },
        ];

        explorer.selected_file = Some(remote_file);
        assert!(
            explorer.can_edit_selected(),
            "Remote file should be editable even if its path does not exist locally"
        );

        explorer.selected_file = Some(remote_dir);
        assert!(
            !explorer.can_edit_selected(),
            "Remote directory must not be editable in Quick Editor"
        );
    }

    #[test]
    fn test_open_chmod_populates_current_permissions() {
        let mut explorer = FileExplorer::new();
        explorer.is_remote = true;
        let file_path = PathBuf::from("/root/deploy.sh");
        explorer.entries = vec![FileEntry {
            name: "deploy.sh".into(),
            path: file_path.clone(),
            is_dir: false,
            is_hidden: false,
            size: 512,
            modified: None,
            permissions: "-rwxr-x---".into(),
            owner: "root".into(),
            group: "root".into(),
        }];

        explorer.open_chmod(file_path);
        assert!(explorer.chmod_open);
        // User: rwx (true, true, true)
        assert!(explorer.chmod_u_r && explorer.chmod_u_w && explorer.chmod_u_x);
        // Group: r-x (true, false, true)
        assert!(explorer.chmod_g_r && !explorer.chmod_g_w && explorer.chmod_g_x);
        // Others: --- (false, false, false)
        assert!(!explorer.chmod_o_r && !explorer.chmod_o_w && !explorer.chmod_o_x);
    }

    #[test]
    fn test_extract_cwd_from_prompt_and_sync_remote_prompt_cwd() {
        let home = PathBuf::from("/home/ubuntu");
        assert_eq!(
            FileExplorer::extract_cwd_from_prompt("ubuntu@prod-web:/var/www/html$ ", Some(&home)),
            Some(PathBuf::from("/var/www/html"))
        );
        assert_eq!(
            FileExplorer::extract_cwd_from_prompt("ubuntu@prod-web:~/projects/api$ ", Some(&home)),
            Some(PathBuf::from("/home/ubuntu/projects/api"))
        );
        assert_eq!(
            FileExplorer::extract_cwd_from_prompt("[root@centos /etc/systemd/system]# ", None),
            Some(PathBuf::from("/etc/systemd/system"))
        );
        // Typing a command after prompt sigil must not trigger false CWD extraction
        assert_eq!(
            FileExplorer::extract_cwd_from_prompt("ubuntu@prod-web:/var/www$ ls -la /tmp", Some(&home)),
            None
        );

        let mut explorer = FileExplorer::new();
        explorer.is_remote = true;
        explorer.follow_terminal_folder = true;
        explorer.remote_home_dir = Some(home);
        explorer.current_path = PathBuf::from("/home/ubuntu");

        explorer.sync_remote_prompt_cwd("ubuntu@prod-web:/var/log$ ");
        assert_eq!(explorer.current_path, PathBuf::from("/var/log"));

        // Manual navigation in SFTP should not be overwritten if terminal prompt hasn't changed
        explorer.current_path = PathBuf::from("/etc");
        explorer.sync_remote_prompt_cwd("ubuntu@prod-web:/var/log$ ");
        assert_eq!(explorer.current_path, PathBuf::from("/etc"));

        // When terminal cd's to a new folder, SFTP follows again
        explorer.sync_remote_prompt_cwd("ubuntu@prod-web:~/app$ ");
        assert_eq!(explorer.current_path, PathBuf::from("/home/ubuntu/app"));
    }

    #[test]
    fn test_expand_tilde_and_candidate_ssh_key_paths() {
        if let Some(home) = dirs::home_dir() {
            assert_eq!(expand_tilde_path("~/.ssh/my_custom_key"), home.join(".ssh/my_custom_key"));
            let candidates = candidate_ssh_key_paths(Some("~/.ssh/custom_ed25519"));
            assert_eq!(candidates.first(), Some(&home.join(".ssh/custom_ed25519")));
            assert!(candidates.contains(&home.join(".ssh/id_ed25519")));
            assert!(candidates.contains(&home.join(".ssh/id_rsa")));
        }
    }

    #[test]
    fn test_multi_server_sftp_pool_isolated_state_and_tab_switching() {
        let mut explorer = FileExplorer::new();

        let cfg_a = SftpTargetConfig {
            host: "198.51.100.10".to_string(),
            port: 22,
            user: "root".to_string(),
            password: Some("pass-a".to_string()),
            key_path: None,
        };
        let cfg_b = SftpTargetConfig {
            host: "198.51.100.20".to_string(),
            port: 22,
            user: "ubuntu".to_string(),
            password: Some("pass-b".to_string()),
            key_path: None,
        };

        // 1. Connect to Server A and simulate browsing /var/log/nginx with an ongoing upload
        explorer.connect_remote(cfg_a.clone());
        let client_a = explorer.remote_client.clone().unwrap();
        explorer.current_path = PathBuf::from("/var/log/nginx");
        explorer.path_input = "/var/log/nginx/".to_string();
        explorer.remote_raw_entries = vec![FileEntry {
            name: "access.log".into(),
            path: PathBuf::from("/var/log/nginx/access.log"),
            is_dir: false,
            is_hidden: false,
            size: 2048,
            modified: None,
            permissions: "-rw-r--r--".into(),
            owner: "root".into(),
            group: "root".into(),
        }];
        explorer.apply_remote_filter_and_sort();
        explorer.selected_file = Some(PathBuf::from("/var/log/nginx/access.log"));
        explorer.is_loading = true; // Simulate ongoing file transfer on Server A

        // 2. Switch to Server B while Server A is transferring
        explorer.connect_remote(cfg_b.clone());
        assert_eq!(explorer.remote_label.as_deref(), Some("ubuntu@198.51.100.20:22"));
        let client_b = explorer.remote_client.clone().unwrap();
        assert!(
            !Arc::ptr_eq(&client_a, &client_b),
            "Server A and Server B must have independent SFTP worker clients"
        );
        assert_eq!(explorer.current_path, PathBuf::from("/home/ubuntu"));
        assert!(explorer.entries.is_empty());

        // Simulate Server B listing /etc/systemd
        explorer.current_path = PathBuf::from("/etc/systemd");
        explorer.remote_raw_entries = vec![FileEntry {
            name: "system.conf".into(),
            path: PathBuf::from("/etc/systemd/system.conf"),
            is_dir: false,
            is_hidden: false,
            size: 512,
            modified: None,
            permissions: "-rw-r--r--".into(),
            owner: "root".into(),
            group: "root".into(),
        }];
        explorer.apply_remote_filter_and_sort();
        explorer.is_loading = false;

        // 3. Switch back to Server A -> must restore Server A's exact state and client without reconnecting
        explorer.connect_remote(cfg_a);
        assert_eq!(explorer.remote_label.as_deref(), Some("root@198.51.100.10:22"));
        assert!(Arc::ptr_eq(&client_a, explorer.remote_client.as_ref().unwrap()));
        assert_eq!(explorer.current_path, PathBuf::from("/var/log/nginx"));
        assert_eq!(explorer.entries.len(), 1);
        assert_eq!(explorer.entries[0].name, "access.log");
        assert_eq!(explorer.selected_file, Some(PathBuf::from("/var/log/nginx/access.log")));
        assert!(explorer.is_loading, "Server A's ongoing transfer state must be preserved");

        // 4. Switch back to Server B -> must restore Server B's exact state (/etc/systemd)
        explorer.connect_remote(cfg_b);
        assert_eq!(explorer.remote_label.as_deref(), Some("ubuntu@198.51.100.20:22"));
        assert!(Arc::ptr_eq(&client_b, explorer.remote_client.as_ref().unwrap()));
        assert_eq!(explorer.current_path, PathBuf::from("/etc/systemd"));
        assert_eq!(explorer.entries.len(), 1);
        assert_eq!(explorer.entries[0].name, "system.conf");
        assert!(!explorer.is_loading);
    }

    #[test]
    fn test_conflict_detection_and_resolution_rename() {
        let mut explorer = FileExplorer::new();
        explorer.is_remote = true;
        explorer.current_path = PathBuf::from("/var/www");
        explorer.entries = vec![FileEntry {
            name: "index.html".into(),
            path: PathBuf::from("/var/www/index.html"),
            is_dir: false,
            is_hidden: false,
            size: 100,
            modified: None,
            permissions: "-rw-r--r--".into(),
            owner: "www-data".into(),
            group: "www-data".into(),
        }];

        // Try uploading a local file also named index.html
        let temp_dir = std::env::temp_dir();
        let local_file = temp_dir.join("index.html");
        let _ = std::fs::write(&local_file, "hello");

        explorer.upload_file(local_file.clone());
        assert!(explorer.pending_conflict.is_some(), "Collision must be detected for duplicate filename");
        let conflict = explorer.pending_conflict.as_ref().unwrap();
        assert_eq!(conflict.file_name, "index.html");

        // Test rename resolution logic
        explorer.pending_conflict = Some(ConflictItem {
            is_upload: true,
            src_path: local_file.clone(),
            dest_path: PathBuf::from("/var/www/index.html"),
            file_name: "index.html".into(),
        });
        explorer.resolve_conflict_rename();
        assert!(explorer.pending_conflict.is_none());

        let _ = std::fs::remove_file(&local_file);
    }

    #[test]
    fn test_local_disk_available_space_and_safety_buffer() {
        let temp_dir = std::env::temp_dir();
        let space_res = crate::sftp::remote::get_local_available_space(&temp_dir);
        assert!(space_res.is_ok());
        let avail = space_res.unwrap();
        assert!(avail > 0);
        assert_eq!(crate::sftp::remote::OS_SAFETY_BUFFER_BYTES, 1024 * 1024 * 1024);
    }
    #[test]
    fn test_file_explorer_local_rename_and_properties() {
        let temp_dir = std::env::temp_dir().join(format!("sftp_test_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&temp_dir);
        let test_file = temp_dir.join("sample.txt");
        let _ = fs::write(&test_file, "hello world");

        let mut explorer = FileExplorer::new();
        explorer.set_path(temp_dir.clone());
        explorer.refresh();

        // Test open_rename
        explorer.open_rename(test_file.clone());
        assert!(explorer.show_rename);
        assert_eq!(explorer.rename_new_name, "sample.txt");

        // Test rename_entry
        explorer.rename_entry(&test_file, "sample_renamed.txt");
        let renamed_file = temp_dir.join("sample_renamed.txt");
        assert!(renamed_file.exists());
        assert!(!test_file.exists());

        // Test open_properties
        let entry = FileEntry {
            name: "sample_renamed.txt".to_string(),
            path: renamed_file.clone(),
            is_dir: false,
            is_hidden: false,
            size: 11,
            modified: None,
            permissions: "rw-r--r--".to_string(),
            owner: "php".to_string(),
            group: "php".to_string(),
        };
        explorer.open_properties(entry);
        assert!(explorer.properties_open);
        assert_eq!(explorer.properties_target.as_ref().map(|e| e.name.as_str()), Some("sample_renamed.txt"));

        // Test open_compare
        let file_b = temp_dir.join("sample_b.txt");
        let _ = fs::write(&file_b, "hello rust");
        explorer.open_compare(&renamed_file, file_b.clone());
        assert!(explorer.diff_open);
        assert_eq!(explorer.diff_file_a, "sample_renamed.txt");
        assert_eq!(explorer.diff_file_b, "sample_b.txt");
        assert_eq!(explorer.diff_content_a, "hello world");
        assert_eq!(explorer.diff_content_b, "hello rust");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_file_explorer_telemetry_event_delivery_behavior() {
        use std::sync::mpsc::channel;

        let mut explorer = FileExplorer::new();
        let (cmd_tx, _cmd_rx) = channel();
        let (event_tx, event_rx) = channel();

        let client = Arc::new(Mutex::new(RemoteSftpClient::new_mock(cmd_tx, event_rx)));
        let label = "ubuntu@103.57.220.111:22".to_string();

        let sess = RemoteSessionState {
            label: label.clone(),
            config: SftpTargetConfig {
                host: "103.57.220.111".to_string(),
                port: 22,
                user: "ubuntu".to_string(),
                password: None,
                key_path: None,
            },
            client,
            is_connected: true,
            is_loading: false,
            current_path: PathBuf::from("/home/ubuntu"),
            path_input: "/home/ubuntu".to_string(),
            path_history: vec![],
            remote_raw_entries: vec![],
            remote_home_dir: Some(PathBuf::from("/home/ubuntu")),
            last_synced_terminal_cwd: None,
            selected_file: None,
            remote_disk_stats: None,
            remote_telemetry: None,
            last_telemetry_poll: None,
            error_message: None,
            status_message: None,
        };

        explorer.remote_sessions.insert(label.clone(), sess);
        explorer.load_active_remote_state(&label);

        // Initially no telemetry metrics
        assert!(explorer.get_active_telemetry().is_none());

        // Worker emits real Telemetry event
        let mut metrics = crate::telemetry::RemoteSystemMetrics::default();
        metrics.cores = Some(8);
        metrics.cpu_pct = Some(4);
        metrics.ram_total_gb = Some(24.0);
        metrics.ram_used_gb = Some(14.6);
        metrics.ram_pct = Some(61);
        metrics.disk_pct = Some(86);
        metrics.uptime = Some("Up: 19h 11m".into());
        metrics.net_rate = Some("▲254.0K  ▼79.0K".into());

        event_tx.send(SftpEvent::Telemetry(metrics.clone())).unwrap();

        // Calling poll() must process the event and populate active telemetry
        explorer.poll();

        let active = explorer.get_active_telemetry();
        assert!(active.is_some(), "Telemetry must be available after poll()");
        let telem = active.unwrap();
        assert_eq!(telem.cores, Some(8));
        assert_eq!(telem.cpu_pct, Some(4));
        assert_eq!(telem.ram_pct, Some(61));
        assert_eq!(telem.disk_pct, Some(86));
        assert_eq!(telem.uptime, Some("Up: 19h 11m".into()));
        assert_eq!(telem.net_rate, Some("▲254.0K  ▼79.0K".into()));
    }

    #[test]
    fn test_file_explorer_telemetry_polling_behavior_and_transfer_suppression() {
        use std::sync::mpsc::channel;
        use crate::sftp::remote::SftpCommand;

        let mut explorer = FileExplorer::new();
        explorer.remote_monitoring_enabled = true;

        let (cmd_tx, cmd_rx) = channel();
        let (_event_tx, event_rx) = channel();

        let client = Arc::new(Mutex::new(RemoteSftpClient::new_mock(cmd_tx, event_rx)));
        let label = "ubuntu@103.57.220.111:22".to_string();

        let sess = RemoteSessionState {
            label: label.clone(),
            config: SftpTargetConfig {
                host: "103.57.220.111".to_string(),
                port: 22,
                user: "ubuntu".to_string(),
                password: None,
                key_path: None,
            },
            client,
            is_connected: true,
            is_loading: false,
            current_path: PathBuf::from("/home/ubuntu"),
            path_input: "/home/ubuntu".to_string(),
            path_history: vec![],
            remote_raw_entries: vec![],
            remote_home_dir: Some(PathBuf::from("/home/ubuntu")),
            last_synced_terminal_cwd: None,
            selected_file: None,
            remote_disk_stats: None,
            remote_telemetry: None,
            last_telemetry_poll: None,
            error_message: None,
            status_message: None,
        };

        explorer.remote_sessions.insert(label.clone(), sess);
        explorer.load_active_remote_state(&label);

        // Case 1: When idle, poll() should dispatch SftpCommand::FetchTelemetry
        explorer.poll();
        let cmd = cmd_rx.try_recv();
        assert!(matches!(cmd, Ok(SftpCommand::FetchTelemetry)), "Idle connected session must trigger FetchTelemetry");

        // Case 2: During active file transfer, poll() must NOT trigger FetchTelemetry to protect bandwidth
        explorer.transfer_progress = Some(TransferProgressInfo {
            file_name: "large_iso.img".into(),
            is_upload: true,
            transferred_bytes: 500_000_000,
            total_bytes: 1_000_000_000,
            bytes_per_sec: 25_000_000.0,
            percent: 50.0,
        });

        // Reset last_telemetry_poll so it would otherwise be eligible
        if let Some(s) = explorer.remote_sessions.get_mut(&label) {
            s.last_telemetry_poll = None;
        }

        explorer.poll();
        let suppressed_cmd = cmd_rx.try_recv();
        assert!(suppressed_cmd.is_err(), "Telemetry poll must be suppressed during active file transfer");
    }
}
