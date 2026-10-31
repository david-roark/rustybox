use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub mod ui;

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Clone, Debug)]
pub struct FileExplorer {
    pub current_path: PathBuf,
    pub entries: Vec<FileEntry>,
    pub selected_file: Option<PathBuf>,
    pub error_message: Option<String>,
    // Modal quick editor
    pub editor_open: bool,
    pub editor_file_path: Option<PathBuf>,
    pub editor_content: String,
    pub editor_status: Option<String>,
    // Modal Chmod (MobaXterm TFORMCHMOD)
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
    // New item dialogs
    pub show_new_folder: bool,
    pub new_folder_name: String,
    pub show_new_file: bool,
    pub new_file_name: String,
}

impl FileExplorer {
    pub fn new() -> Self {
        let initial_path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let mut explorer = Self {
            current_path: initial_path,
            entries: Vec::new(),
            selected_file: None,
            error_message: None,
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
        };
        explorer.refresh();
        explorer
    }

    pub fn set_path(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.current_path = path;
            self.refresh();
        }
    }

    pub fn go_up(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            self.current_path = parent.to_path_buf();
            self.refresh();
        }
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        self.error_message = None;

        match fs::read_dir(&self.current_path) {
            Ok(read_dir) => {
                let mut dirs = Vec::new();
                let mut files = Vec::new();

                for entry_res in read_dir {
                    if let Ok(entry) = entry_res {
                        let path = entry.path();
                        let name = entry.file_name().to_string_lossy().to_string();
                        let metadata = entry.metadata().ok();
                        let is_dir = metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                        let modified = metadata.and_then(|m| m.modified().ok());

                        let item = FileEntry {
                            name,
                            path,
                            is_dir,
                            size,
                            modified,
                        };

                        if is_dir {
                            dirs.push(item);
                        } else {
                            files.push(item);
                        }
                    }
                }

                dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

                self.entries.extend(dirs);
                self.entries.extend(files);
            }
            Err(e) => {
                self.error_message = Some(format!("Cannot read dir: {}", e));
            }
        }
    }

    pub fn open_editor(&mut self, path: PathBuf) {
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
        let res = if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        };
        if let Err(e) = res {
            self.error_message = Some(format!("Delete failed: {}", e));
        } else {
            self.refresh();
        }
    }

    pub fn create_folder(&mut self, name: &str) {
        let new_dir = self.current_path.join(name);
        if let Err(e) = fs::create_dir(&new_dir) {
            self.error_message = Some(format!("Create folder failed: {}", e));
        } else {
            self.refresh();
        }
    }

    pub fn create_file(&mut self, name: &str) {
        let new_path = self.current_path.join(name);
        if let Err(e) = fs::write(&new_path, "") {
            self.error_message = Some(format!("Create file failed: {}", e));
        } else {
            self.refresh();
        }
    }

    pub fn open_chmod(&mut self, path: PathBuf) {
        self.chmod_target = Some(path);
        self.chmod_open = true;
    }

    pub fn apply_chmod(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(ref path) = self.chmod_target {
                let mut mode = 0o000;
                if self.chmod_u_r { mode |= 0o400; }
                if self.chmod_u_w { mode |= 0o200; }
                if self.chmod_u_x { mode |= 0o100; }
                if self.chmod_g_r { mode |= 0o040; }
                if self.chmod_g_w { mode |= 0o020; }
                if self.chmod_g_x { mode |= 0o010; }
                if self.chmod_o_r { mode |= 0o004; }
                if self.chmod_o_w { mode |= 0o002; }
                if self.chmod_o_x { mode |= 0o001; }
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
            }
        }
        self.chmod_open = false;
        self.refresh();
    }
}
