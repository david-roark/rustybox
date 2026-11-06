use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use super::FileEntry;

pub const OS_SAFETY_BUFFER_BYTES: u64 = 1024 * 1024 * 1024; // 1 GB reserve for OS safety

#[derive(Clone, Debug, PartialEq)]
pub struct SftpTargetConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub key_path: Option<String>,
}

#[derive(Debug)]
pub enum SftpCommand {
    Connect(SftpTargetConfig),
    Disconnect,
    ListDir(String),
    ReadFile(String),
    WriteFile(String, String),
    CreateDir(String),
    CreateFile(String),
    Delete(String, bool), // (remote_path, is_dir)
    Rename(String, String), // (old_path, new_path)
    Chmod(String, u32),
    Upload(PathBuf, String),   // (local_path, remote_path)
    Download(String, PathBuf), // (remote_path, local_path)
    CancelTransfer,
    GetDiskUsage(String),
}

#[derive(Clone, Debug)]
pub struct TransferProgressInfo {
    pub file_name: String,
    pub is_upload: bool,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    pub bytes_per_sec: f64,
    pub percent: f32,
}

#[derive(Debug)]
pub enum SftpEvent {
    Connected {
        target: String,
        initial_dir: String,
    },
    ConnectionFailed(String),
    DirListed {
        dir: String,
        entries: Vec<FileEntry>,
    },
    FileRead {
        path: String,
        content: String,
    },
    FileWritten {
        path: String,
    },
    TransferProgress(TransferProgressInfo),
    TransferFinished(String),
    OperationSuccess(String),
    OperationError(String),
    DiskUsage {
        used: u64,
        total: u64,
        pct: f32,
    },
}

pub struct RemoteSftpClient {
    cmd_tx: Sender<SftpCommand>,
    event_rx: Receiver<SftpEvent>,
    pub cancel_flag: Arc<AtomicBool>,
    pub active_target: Option<String>,
    pub is_connected: bool,
    pub is_busy: bool,
}

impl RemoteSftpClient {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = channel::<SftpCommand>();
        let (event_tx, event_rx) = channel::<SftpEvent>();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_flag_worker = Arc::clone(&cancel_flag);

        thread::Builder::new()
            .name("rustybox-sftp-worker".to_string())
            .spawn(move || {
                run_sftp_worker(cmd_rx, event_tx, cancel_flag_worker);
            })
            .expect("Failed to spawn SFTP worker thread");

        Self {
            cmd_tx,
            event_rx,
            cancel_flag,
            active_target: None,
            is_connected: false,
            is_busy: false,
        }
    }

    pub fn cancel_transfer(&mut self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        let _ = self.cmd_tx.send(SftpCommand::CancelTransfer);
    }

    pub fn connect(&mut self, config: SftpTargetConfig) {
        self.is_busy = true;
        self.active_target = Some(format!("{}@{}:{}", config.user, config.host, config.port));
        let _ = self.cmd_tx.send(SftpCommand::Connect(config));
    }

    pub fn disconnect(&mut self) {
        self.is_connected = false;
        self.is_busy = false;
        self.active_target = None;
        let _ = self.cmd_tx.send(SftpCommand::Disconnect);
    }

    pub fn list_dir(&mut self, path: &str) {
        self.is_busy = true;
        let _ = self.cmd_tx.send(SftpCommand::ListDir(path.to_string()));
    }

    pub fn read_file(&mut self, path: &str) {
        self.is_busy = true;
        let _ = self.cmd_tx.send(SftpCommand::ReadFile(path.to_string()));
    }

    pub fn write_file(&mut self, path: &str, content: &str) {
        self.is_busy = true;
        let _ = self.cmd_tx.send(SftpCommand::WriteFile(path.to_string(), content.to_string()));
    }

    pub fn create_dir(&mut self, path: &str) {
        self.is_busy = true;
        let _ = self.cmd_tx.send(SftpCommand::CreateDir(path.to_string()));
    }

    pub fn create_file(&mut self, path: &str) {
        self.is_busy = true;
        let _ = self.cmd_tx.send(SftpCommand::CreateFile(path.to_string()));
    }

    pub fn delete(&mut self, path: &str, is_dir: bool) {
        self.is_busy = true;
        let _ = self.cmd_tx.send(SftpCommand::Delete(path.to_string(), is_dir));
    }

    pub fn rename(&mut self, from_path: String, to_path: String) {
        self.is_busy = true;
        let _ = self.cmd_tx.send(SftpCommand::Rename(from_path, to_path));
    }

    pub fn chmod(&mut self, path: &str, mode: u32) {
        self.is_busy = true;
        let _ = self.cmd_tx.send(SftpCommand::Chmod(path.to_string(), mode));
    }

    pub fn upload(&mut self, local_path: PathBuf, remote_path: String) {
        self.is_busy = true;
        let _ = self.cmd_tx.send(SftpCommand::Upload(local_path, remote_path));
    }

    pub fn download(&mut self, remote_path: String, local_path: PathBuf) {
        self.is_busy = true;
        let _ = self.cmd_tx.send(SftpCommand::Download(remote_path, local_path));
    }

    pub fn try_recv(&self) -> Option<SftpEvent> {
        self.event_rx.try_recv().ok()
    }
}

fn run_sftp_worker(
    cmd_rx: Receiver<SftpCommand>,
    event_tx: Sender<SftpEvent>,
    cancel_flag: Arc<AtomicBool>,
) {
    let mut current_sess: Option<(
        ssh2::Session,
        ssh2::Sftp,
        SftpTargetConfig,
        HashMap<u32, String>,
        HashMap<u32, String>,
    )> = None;

    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            SftpCommand::CancelTransfer => {
                cancel_flag.store(true, Ordering::SeqCst);
            }

            SftpCommand::Connect(cfg) => {
                // If already connected to the same config, keep it
                if let Some((_, _, ref active_cfg, ..)) = current_sess {
                    if *active_cfg == cfg {
                        let _ = event_tx.send(SftpEvent::Connected {
                            target: format!("{}@{}:{}", cfg.user, cfg.host, cfg.port),
                            initial_dir: if cfg.user == "root" { "/root".into() } else { format!("/home/{}", cfg.user) },
                        });
                        continue;
                    }
                }

                current_sess = None;
                match connect_sftp(&cfg) {
                    Ok((sess, sftp)) => {
                        let initial_dir = if cfg.user == "root" {
                            "/root".to_string()
                        } else {
                            format!("/home/{}", cfg.user)
                        };

                        // Check if initial_dir exists on VPS, fallback to "/" if not
                        let valid_dir = if sftp.stat(Path::new(&initial_dir)).is_ok() {
                            initial_dir
                        } else {
                            "/".to_string()
                        };

                        let target = format!("{}@{}:{}", cfg.user, cfg.host, cfg.port);
                        let _ = event_tx.send(SftpEvent::Connected {
                            target,
                            initial_dir: valid_dir.clone(),
                        });

                        // Cache remote user and group mappings from /etc/passwd and /etc/group
                        let (users, groups) = load_remote_id_cache(&sftp, &cfg.user);

                        // Automatically fetch directory entries for the initial dir
                        if let Ok(entries) = read_sftp_dir(&sftp, &valid_dir, &cfg.user, &users, &groups) {
                            let _ = event_tx.send(SftpEvent::DirListed {
                                dir: valid_dir.clone(),
                                entries,
                            });
                        }

                        // Also fetch remote disk stats
                        if let Ok((used, total, pct)) = fetch_remote_disk(&sess, &valid_dir) {
                            let _ = event_tx.send(SftpEvent::DiskUsage { used, total, pct });
                        }

                        current_sess = Some((sess, sftp, cfg, users, groups));
                    }
                    Err(e) => {
                        let _ = event_tx.send(SftpEvent::ConnectionFailed(e));
                    }
                }
            }

            SftpCommand::Disconnect => {
                current_sess = None;
            }

            SftpCommand::ListDir(dir_path) => {
                if let Some((ref sess, ref sftp, ref cfg, ref users, ref groups)) = current_sess {
                    match read_sftp_dir(sftp, &dir_path, &cfg.user, users, groups) {
                        Ok(entries) => {
                            let _ = event_tx.send(SftpEvent::DirListed {
                                dir: dir_path.clone(),
                                entries,
                            });
                            if let Ok((used, total, pct)) = fetch_remote_disk(sess, &dir_path) {
                                let _ = event_tx.send(SftpEvent::DiskUsage { used, total, pct });
                            }
                        }
                        Err(e) => {
                            let _ = event_tx.send(SftpEvent::OperationError(format!("Read dir error: {}", e)));
                        }
                    }
                } else {
                    let _ = event_tx.send(SftpEvent::OperationError("SFTP session is not connected".into()));
                }
            }

            SftpCommand::ReadFile(remote_path) => {
                if let Some((_, ref sftp, ..)) = current_sess {
                    const MAX_EDITOR_BYTES: u64 = 2 * 1024 * 1024;
                    let p = Path::new(&remote_path);
                    if let Ok(stat) = sftp.stat(p) {
                        if stat.is_dir() {
                            let _ = event_tx.send(SftpEvent::OperationError("Cannot open directory in Quick Editor".into()));
                            continue;
                        }
                        if stat.size.unwrap_or(0) > MAX_EDITOR_BYTES {
                            let _ = event_tx.send(SftpEvent::OperationError(
                                "File is too large for Quick Editor (> 2 MB). Please download it instead.".into(),
                            ));
                            continue;
                        }
                    }
                    match sftp.open(p) {
                        Ok(file) => {
                            let mut buf = Vec::new();
                            if let Err(e) = file.take(MAX_EDITOR_BYTES + 1).read_to_end(&mut buf) {
                                let _ = event_tx.send(SftpEvent::OperationError(format!("Read file failed: {}", e)));
                            } else if buf.len() as u64 > MAX_EDITOR_BYTES {
                                let _ = event_tx.send(SftpEvent::OperationError(
                                    "File is too large for Quick Editor (> 2 MB). Please download it instead.".into(),
                                ));
                            } else {
                                let content = String::from_utf8_lossy(&buf).to_string();
                                let _ = event_tx.send(SftpEvent::FileRead {
                                    path: remote_path,
                                    content,
                                });
                            }
                        }
                        Err(e) => {
                            let _ = event_tx.send(SftpEvent::OperationError(format!("Cannot open remote file: {}", e)));
                        }
                    }
                }
            }

            SftpCommand::WriteFile(remote_path, content) => {
                if let Some((_, ref sftp, ..)) = current_sess {
                    match sftp.create(Path::new(&remote_path)) {
                        Ok(mut file) => {
                            if let Err(e) = file.write_all(content.as_bytes()) {
                                let _ = event_tx.send(SftpEvent::OperationError(format!("Write remote file failed: {}", e)));
                            } else {
                                let _ = event_tx.send(SftpEvent::FileWritten { path: remote_path });
                            }
                        }
                        Err(e) => {
                            let _ = event_tx.send(SftpEvent::OperationError(format!("Create remote file failed: {}", e)));
                        }
                    }
                }
            }

            SftpCommand::CreateDir(remote_path) => {
                if let Some((_, ref sftp, ..)) = current_sess {
                    match sftp.mkdir(Path::new(&remote_path), 0o755) {
                        Ok(_) => {
                            let _ = event_tx.send(SftpEvent::OperationSuccess("Folder created on VPS".into()));
                        }
                        Err(e) => {
                            let _ = event_tx.send(SftpEvent::OperationError(format!("Mkdir failed: {}", e)));
                        }
                    }
                }
            }

            SftpCommand::CreateFile(remote_path) => {
                if let Some((_, ref sftp, ..)) = current_sess {
                    match sftp.create(Path::new(&remote_path)) {
                        Ok(_) => {
                            let _ = event_tx.send(SftpEvent::OperationSuccess("File created on VPS".into()));
                        }
                        Err(e) => {
                            let _ = event_tx.send(SftpEvent::OperationError(format!("Touch failed: {}", e)));
                        }
                    }
                }
            }

            SftpCommand::Delete(remote_path, is_dir) => {
                if let Some((_, ref sftp, ..)) = current_sess {
                    let p = Path::new(&remote_path);
                    let res: Result<(), String> = if is_dir {
                        remove_sftp_dir_recursive(sftp, p, 0)
                    } else {
                        sftp.unlink(p).map_err(|e| e.to_string())
                    };
                    match res {
                        Ok(_) => {
                            let _ = event_tx.send(SftpEvent::OperationSuccess("Deleted on VPS".into()));
                        }
                        Err(e) => {
                            let _ = event_tx.send(SftpEvent::OperationError(format!("Delete failed: {}", e)));
                        }
                    }
                }
            }

            SftpCommand::Rename(from_path, to_path) => {
                if let Some((_, ref sftp, ..)) = current_sess {
                    let from = Path::new(&from_path);
                    let to = Path::new(&to_path);
                    match sftp.rename(from, to, None) {
                        Ok(_) => {
                            let _ = event_tx.send(SftpEvent::OperationSuccess("Renamed on VPS".into()));
                        }
                        Err(e) => {
                            let _ = event_tx.send(SftpEvent::OperationError(format!("Rename failed: {}", e)));
                        }
                    }
                }
            }

            SftpCommand::Chmod(remote_path, mode) => {
                if let Some((_, ref sftp, ..)) = current_sess {
                    let p = Path::new(&remote_path);
                    let stat = ssh2::FileStat {
                        size: None,
                        uid: None,
                        gid: None,
                        perm: Some(mode),
                        atime: None,
                        mtime: None,
                    };
                    match sftp.setstat(p, stat) {
                        Ok(_) => {
                            let _ = event_tx.send(SftpEvent::OperationSuccess("Permissions updated on VPS".into()));
                        }
                        Err(e) => {
                            let _ = event_tx.send(SftpEvent::OperationError(format!("Chmod failed: {}", e)));
                        }
                    }
                }
            }

            SftpCommand::Upload(local_path, remote_path) => {
                cancel_flag.store(false, Ordering::SeqCst);
                if let Some((ref sess, ref sftp, ..)) = current_sess {
                    if local_path.is_dir() {
                        match upload_dir_recursive(sftp, &local_path, &remote_path, &cancel_flag, &event_tx) {
                            Ok(count) => {
                                let folder_name = local_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "folder".to_string());
                                let _ = event_tx.send(SftpEvent::TransferFinished(format!("Uploaded folder '{}' ({} files) to VPS", folder_name, count)));
                            }
                            Err(e) => {
                                let _ = event_tx.send(SftpEvent::OperationError(e));
                            }
                        }
                        continue;
                    }

                    let total_bytes = match std::fs::metadata(&local_path) {
                        Ok(m) => m.len(),
                        Err(e) => {
                            let _ = event_tx.send(SftpEvent::OperationError(format!("Cannot read local file: {}", e)));
                            continue;
                        }
                    };

                    let file_name = local_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "file".to_string());

                    // Pre-check remote available disk space with 1 GB safety buffer
                    let remote_parent = Path::new(&remote_path)
                        .parent()
                        .and_then(|p| p.to_str())
                        .unwrap_or("/");
                    if let Ok((used, total, _)) = fetch_remote_disk(sess, remote_parent) {
                        let avail = total.saturating_sub(used);
                        let needed_with_reserve = total_bytes.saturating_add(OS_SAFETY_BUFFER_BYTES);
                        if avail < needed_with_reserve {
                            let avail_mb = avail as f64 / (1024.0 * 1024.0);
                            let needed_mb = total_bytes as f64 / (1024.0 * 1024.0);
                            let _ = event_tx.send(SftpEvent::OperationError(format!(
                                "Insufficient remote disk space! Need {:.1} MB + 1GB safety reserve, but only {:.1} MB available on VPS.",
                                needed_mb, avail_mb
                            )));
                            continue;
                        }
                    }

                    match std::fs::File::open(&local_path) {
                        Ok(lf) => match sftp.create(Path::new(&remote_path)) {
                            Ok(mut rf) => {
                                let mut lf = std::io::BufReader::with_capacity(256 * 1024, lf);
                                let mut buffer = [0u8; 256 * 1024];
                                let mut transferred = 0u64;
                                let start_time = Instant::now();
                                let mut last_report = Instant::now();
                                let mut transfer_error = None;
                                let mut cancelled = false;

                                loop {
                                    if cancel_flag.load(Ordering::Relaxed) {
                                        cancelled = true;
                                        break;
                                    }

                                    let n = match lf.read(&mut buffer) {
                                        Ok(0) => break,
                                        Ok(n) => n,
                                        Err(e) => {
                                            transfer_error = Some(format!("Local read error: {}", e));
                                            break;
                                        }
                                    };

                                    if let Err(e) = rf.write_all(&buffer[..n]) {
                                        transfer_error = Some(format!("Remote write error: {}", e));
                                        break;
                                    }

                                    transferred += n as u64;

                                    if last_report.elapsed() >= Duration::from_millis(150) || transferred == total_bytes {
                                        last_report = Instant::now();
                                        let elapsed_sec = start_time.elapsed().as_secs_f64().max(0.001);
                                        let speed = transferred as f64 / elapsed_sec;
                                        let pct = if total_bytes > 0 {
                                            (transferred as f32 / total_bytes as f32) * 100.0
                                        } else {
                                            100.0
                                        };

                                        let _ = event_tx.send(SftpEvent::TransferProgress(TransferProgressInfo {
                                            file_name: file_name.clone(),
                                            is_upload: true,
                                            transferred_bytes: transferred,
                                            total_bytes,
                                            bytes_per_sec: speed,
                                            percent: pct,
                                        }));
                                    }
                                }

                                if cancelled {
                                    drop(rf);
                                    let _ = sftp.unlink(Path::new(&remote_path));
                                    let _ = event_tx.send(SftpEvent::TransferFinished("Upload cancelled by user.".into()));
                                } else if let Some(err) = transfer_error {
                                    drop(rf);
                                    let _ = sftp.unlink(Path::new(&remote_path));
                                    let _ = event_tx.send(SftpEvent::OperationError(err));
                                } else {
                                    let _ = rf.flush();
                                    let _ = event_tx.send(SftpEvent::TransferFinished(format!("Uploaded '{}' to VPS", file_name)));
                                }
                            }
                            Err(e) => {
                                let _ = event_tx.send(SftpEvent::OperationError(format!("Cannot create remote file: {}", e)));
                            }
                        },
                        Err(e) => {
                            let _ = event_tx.send(SftpEvent::OperationError(format!("Cannot read local file: {}", e)));
                        }
                    }
                }
            }

            SftpCommand::Download(remote_path, local_path) => {
                cancel_flag.store(false, Ordering::SeqCst);
                if let Some((_, ref sftp, ..)) = current_sess {
                    let p = Path::new(&remote_path);
                    let stat = sftp.stat(p).ok();
                    let is_remote_dir = stat.as_ref().map(|s| s.is_dir()).unwrap_or(false);

                    if is_remote_dir {
                        match download_dir_recursive(sftp, p, &local_path, &cancel_flag, &event_tx) {
                            Ok(count) => {
                                let folder_name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "folder".to_string());
                                let _ = event_tx.send(SftpEvent::TransferFinished(format!("Downloaded folder '{}' ({} files)", folder_name, count)));
                            }
                            Err(e) => {
                                let _ = event_tx.send(SftpEvent::OperationError(e));
                            }
                        }
                        continue;
                    }

                    let total_bytes = stat.and_then(|s| s.size).unwrap_or(0);
                    let file_name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "file".to_string());

                    // Pre-check local available disk space with 1 GB safety buffer
                    let local_parent = local_path.parent().unwrap_or_else(|| Path::new("."));
                    if let Ok(avail) = get_local_available_space(local_parent) {
                        let needed_with_reserve = total_bytes.saturating_add(OS_SAFETY_BUFFER_BYTES);
                        if avail < needed_with_reserve {
                            let avail_mb = avail as f64 / (1024.0 * 1024.0);
                            let needed_mb = total_bytes as f64 / (1024.0 * 1024.0);
                            let _ = event_tx.send(SftpEvent::OperationError(format!(
                                "Insufficient local disk space! Need {:.1} MB + 1GB safety reserve, but only {:.1} MB free on disk.",
                                needed_mb, avail_mb
                            )));
                            continue;
                        }
                    }

                    match sftp.open(p) {
                        Ok(mut rf) => match std::fs::File::create(&local_path) {
                            Ok(lf) => {
                                let mut lf = std::io::BufWriter::with_capacity(256 * 1024, lf);
                                let mut buffer = [0u8; 256 * 1024];
                                let mut transferred = 0u64;
                                let start_time = Instant::now();
                                let mut last_report = Instant::now();
                                let mut transfer_error = None;
                                let mut cancelled = false;

                                loop {
                                    if cancel_flag.load(Ordering::Relaxed) {
                                        cancelled = true;
                                        break;
                                    }

                                    let n = match rf.read(&mut buffer) {
                                        Ok(0) => break,
                                        Ok(n) => n,
                                        Err(e) => {
                                            transfer_error = Some(format!("Remote read error: {}", e));
                                            break;
                                        }
                                    };

                                    if let Err(e) = lf.write_all(&buffer[..n]) {
                                        transfer_error = Some(format!("Local write error: {}", e));
                                        break;
                                    }

                                    transferred += n as u64;

                                    if last_report.elapsed() >= Duration::from_millis(150) || transferred == total_bytes {
                                        last_report = Instant::now();
                                        let elapsed_sec = start_time.elapsed().as_secs_f64().max(0.001);
                                        let speed = transferred as f64 / elapsed_sec;
                                        let pct = if total_bytes > 0 {
                                            (transferred as f32 / total_bytes as f32) * 100.0
                                        } else {
                                            100.0
                                        };

                                        let _ = event_tx.send(SftpEvent::TransferProgress(TransferProgressInfo {
                                            file_name: file_name.clone(),
                                            is_upload: false,
                                            transferred_bytes: transferred,
                                            total_bytes,
                                            bytes_per_sec: speed,
                                            percent: pct,
                                        }));
                                    }
                                }

                                if cancelled {
                                    drop(lf);
                                    let _ = std::fs::remove_file(&local_path);
                                    let _ = event_tx.send(SftpEvent::TransferFinished("Download cancelled by user.".into()));
                                } else if let Some(err) = transfer_error {
                                    drop(lf);
                                    let _ = std::fs::remove_file(&local_path);
                                    let _ = event_tx.send(SftpEvent::OperationError(err));
                                } else {
                                    let _ = lf.flush();
                                    let _ = event_tx.send(SftpEvent::TransferFinished(format!("Downloaded to '{}'", local_path.display())));
                                }
                            }
                            Err(e) => {
                                let _ = event_tx.send(SftpEvent::OperationError(format!("Cannot write local file: {}", e)));
                            }
                        },
                        Err(e) => {
                            let _ = event_tx.send(SftpEvent::OperationError(format!("Cannot open remote file: {}", e)));
                        }
                    }
                }
            }

            SftpCommand::GetDiskUsage(dir_path) => {
                if let Some((ref sess, ..)) = current_sess {
                    if let Ok((used, total, pct)) = fetch_remote_disk(sess, &dir_path) {
                        let _ = event_tx.send(SftpEvent::DiskUsage { used, total, pct });
                    }
                }
            }
        }
    }
}

fn upload_dir_recursive(
    sftp: &ssh2::Sftp,
    local_dir: &Path,
    remote_base: &str,
    cancel_flag: &Arc<AtomicBool>,
    event_tx: &Sender<SftpEvent>,
) -> Result<usize, String> {
    let root_name = local_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "folder".to_string());

    let mut all_dirs = Vec::new();
    let mut all_files = Vec::new();
    let mut total_bytes = 0u64;

    for entry in walkdir::WalkDir::new(local_dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if let Ok(rel) = p.strip_prefix(local_dir) {
            if rel.as_os_str().is_empty() {
                continue;
            }
            if p.is_dir() {
                all_dirs.push(rel.to_path_buf());
            } else if p.is_file() {
                let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
                total_bytes += len;
                all_files.push((p.to_path_buf(), rel.to_path_buf(), len));
            }
        }
    }

    let _ = sftp.mkdir(Path::new(remote_base), 0o755);
    for d in &all_dirs {
        let rem = Path::new(remote_base).join(d);
        let rem_str = rem.to_string_lossy().replace('\\', "/");
        let _ = sftp.mkdir(Path::new(&rem_str), 0o755);
    }

    let start_time = Instant::now();
    let mut last_report = Instant::now();
    let mut transferred = 0u64;
    let file_count = all_files.len();

    for (src, rel, _) in all_files {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("Upload cancelled by user.".to_string());
        }
        let dest = Path::new(remote_base).join(&rel);
        let dest_str = dest.to_string_lossy().replace('\\', "/");
        let display_name = format!("{}/{}", root_name, rel.display().to_string().replace('\\', "/"));

        let lf = std::fs::File::open(&src)
            .map_err(|e| format!("Cannot read local file '{}': {}", src.display(), e))?;
        let mut lf = std::io::BufReader::with_capacity(256 * 1024, lf);

        let mut rf = sftp
            .create(Path::new(&dest_str))
            .map_err(|e| format!("Cannot create remote file '{}': {}", dest_str, e))?;

        let mut buffer = [0u8; 256 * 1024];
        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err("Upload cancelled by user.".to_string());
            }
            let n = lf
                .read(&mut buffer)
                .map_err(|e| format!("Error reading '{}': {}", src.display(), e))?;
            if n == 0 {
                break;
            }
            rf.write_all(&buffer[..n])
                .map_err(|e| format!("Error writing remote '{}': {}", dest_str, e))?;
            transferred += n as u64;

            if last_report.elapsed() >= Duration::from_millis(150) || transferred == total_bytes {
                last_report = Instant::now();
                let elapsed_sec = start_time.elapsed().as_secs_f64().max(0.001);
                let speed = transferred as f64 / elapsed_sec;
                let pct = if total_bytes > 0 {
                    (transferred as f32 / total_bytes as f32) * 100.0
                } else {
                    100.0
                };
                let _ = event_tx.send(SftpEvent::TransferProgress(TransferProgressInfo {
                    file_name: display_name.clone(),
                    is_upload: true,
                    transferred_bytes: transferred,
                    total_bytes,
                    bytes_per_sec: speed,
                    percent: pct,
                }));
            }
        }
        let _ = rf.flush();
    }

    Ok(file_count)
}

fn collect_remote_entries(
    sftp: &ssh2::Sftp,
    remote_dir: &Path,
    base_dir: &Path,
    dirs: &mut Vec<PathBuf>,
    files: &mut Vec<(PathBuf, PathBuf, u64)>,
) -> Result<(), String> {
    let list = sftp.readdir(remote_dir).map_err(|e| e.to_string())?;
    for (p, stat) in list {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name == "." || name == ".." {
            continue;
        }
        if let Ok(rel) = p.strip_prefix(base_dir) {
            let is_symlink = stat.perm.map(|perm| (perm & 0o170000) == 0o120000).unwrap_or(false);
            if stat.is_dir() && !is_symlink {
                dirs.push(rel.to_path_buf());
                let _ = collect_remote_entries(sftp, &p, base_dir, dirs, files);
            } else {
                let sz = stat.size.unwrap_or(0);
                files.push((p.clone(), rel.to_path_buf(), sz));
            }
        }
    }
    Ok(())
}

fn download_dir_recursive(
    sftp: &ssh2::Sftp,
    remote_dir: &Path,
    local_dest: &Path,
    cancel_flag: &Arc<AtomicBool>,
    event_tx: &Sender<SftpEvent>,
) -> Result<usize, String> {
    let root_name = remote_dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "folder".to_string());

    let mut dirs = Vec::new();
    let mut files = Vec::new();
    collect_remote_entries(sftp, remote_dir, remote_dir, &mut dirs, &mut files)?;

    let total_bytes: u64 = files.iter().map(|f| f.2).sum();
    std::fs::create_dir_all(local_dest)
        .map_err(|e| format!("Cannot create local dir '{}': {}", local_dest.display(), e))?;

    for d in &dirs {
        let loc = local_dest.join(d);
        let _ = std::fs::create_dir_all(&loc);
    }

    let start_time = Instant::now();
    let mut last_report = Instant::now();
    let mut transferred = 0u64;
    let file_count = files.len();

    for (rem_f, rel_f, _) in files {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err("Download cancelled by user.".to_string());
        }
        let loc_f = local_dest.join(&rel_f);
        let display_name = format!("{}/{}", root_name, rel_f.display().to_string().replace('\\', "/"));

        let mut rf = sftp
            .open(&rem_f)
            .map_err(|e| format!("Cannot open remote file '{}': {}", rem_f.display(), e))?;

        let lf = std::fs::File::create(&loc_f)
            .map_err(|e| format!("Cannot create local file '{}': {}", loc_f.display(), e))?;
        let mut lf = std::io::BufWriter::with_capacity(256 * 1024, lf);

        let mut buffer = [0u8; 256 * 1024];
        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                return Err("Download cancelled by user.".to_string());
            }
            let n = rf
                .read(&mut buffer)
                .map_err(|e| format!("Error reading remote '{}': {}", rem_f.display(), e))?;
            if n == 0 {
                break;
            }
            lf.write_all(&buffer[..n])
                .map_err(|e| format!("Error writing local '{}': {}", loc_f.display(), e))?;
            transferred += n as u64;

            if last_report.elapsed() >= Duration::from_millis(150) || transferred == total_bytes {
                last_report = Instant::now();
                let elapsed_sec = start_time.elapsed().as_secs_f64().max(0.001);
                let speed = transferred as f64 / elapsed_sec;
                let pct = if total_bytes > 0 {
                    (transferred as f32 / total_bytes as f32) * 100.0
                } else {
                    100.0
                };
                let _ = event_tx.send(SftpEvent::TransferProgress(TransferProgressInfo {
                    file_name: display_name.clone(),
                    is_upload: false,
                    transferred_bytes: transferred,
                    total_bytes,
                    bytes_per_sec: speed,
                    percent: pct,
                }));
            }
        }
        let _ = lf.flush();
    }

    Ok(file_count)
}

pub fn get_local_available_space(_path: &Path) -> Result<u64, String> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;
        use std::os::unix::ffi::OsStrExt;

        let c_path = CString::new(_path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        let mut stat = MaybeUninit::<libc::statvfs>::uninit();
        let ret = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
        if ret == 0 {
            let stat = unsafe { stat.assume_init() };
            // f_bavail is free blocks available to unprivileged users
            let avail = (stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64);
            return Ok(avail);
        }
    }
    // Fallback default: 50 GB
    Ok(50 * 1024 * 1024 * 1024)
}

pub fn expand_tilde_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if trimmed == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    } else if let Some(rest) = trimmed.strip_prefix("~/").or_else(|| trimmed.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(trimmed)
}

pub fn candidate_ssh_key_paths(explicit_key: Option<&str>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(k) = explicit_key {
        let trimmed = k.trim();
        if !trimmed.is_empty() {
            paths.push(expand_tilde_path(trimmed));
        }
    }
    if let Some(home) = dirs::home_dir() {
        let ssh_dir = home.join(".ssh");
        for default_name in ["id_ed25519", "id_ecdsa", "id_rsa", "id_dsa"] {
            let p = ssh_dir.join(default_name);
            if !paths.contains(&p) {
                paths.push(p);
            }
        }
    }
    paths
}

fn remove_sftp_dir_recursive(sftp: &ssh2::Sftp, dir_path: &Path, depth: u32) -> Result<(), String> {
    let raw = dir_path.to_string_lossy();
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "/" || trimmed == "." || trimmed == ".." {
        return Err("Refusing to recursively delete root or invalid directory".to_string());
    }
    if depth > 32 {
        return Err("Maximum directory recursion depth (32) exceeded".to_string());
    }
    let entries = sftp.readdir(dir_path).map_err(|e| e.to_string())?;
    for (entry_path, stat) in entries {
        let name = entry_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if name == "." || name == ".." {
            continue;
        }
        let is_symlink = stat.perm.map(|p| (p & 0o170000) == 0o120000).unwrap_or(false);
        if stat.is_dir() && !is_symlink {
            remove_sftp_dir_recursive(sftp, &entry_path, depth + 1)?;
        } else {
            sftp.unlink(&entry_path).map_err(|e| e.to_string())?;
        }
    }
    sftp.rmdir(dir_path).map_err(|e| e.to_string())
}

fn connect_sftp(cfg: &SftpTargetConfig) -> Result<(ssh2::Session, ssh2::Sftp), String> {
    let addr = format!("{}:{}", cfg.host, cfg.port);
    let tcp = TcpStream::connect_timeout(
        &addr
            .parse()
            .or_else(|_| {
                use std::net::ToSocketAddrs;
                addr.to_socket_addrs()
                    .map_err(|e| format!("DNS lookup failed: {}", e))?
                    .next()
                    .ok_or_else(|| "No IP resolved".to_string())
            })
            .map_err(|e| format!("Invalid address '{}': {}", addr, e))?,
        Duration::from_secs(8),
    )
    .map_err(|e| format!("Cannot connect TCP to {}: {}", addr, e))?;

    let _ = tcp.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = tcp.set_write_timeout(Some(Duration::from_secs(15)));
    let _ = tcp.set_nodelay(true);
    let sock = socket2::SockRef::from(&tcp);
    let _ = sock.set_send_buffer_size(1024 * 1024);
    let _ = sock.set_recv_buffer_size(1024 * 1024);

    let mut sess = ssh2::Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;
    sess.set_tcp_stream(tcp);
    sess.handshake().map_err(|e| format!("SSH handshake failed: {}", e))?;

    // Try authentication
    let mut authenticated = false;

    // 1. Try explicit public key if provided (supporting `~/` expansion)
    if let Some(ref key_path) = cfg.key_path {
        let p = expand_tilde_path(key_path);
        if p.exists() {
            if sess.userauth_pubkey_file(&cfg.user, None, &p, None).is_ok() && sess.authenticated() {
                authenticated = true;
            }
        }
    }

    // 2. Try password
    if !authenticated {
        if let Some(ref pwd) = cfg.password {
            if !pwd.trim().is_empty() {
                if sess.userauth_password(&cfg.user, pwd).is_ok() && sess.authenticated() {
                    authenticated = true;
                }
            }
        }
    }

    // 3. Fallback: try SSH agent
    if !authenticated {
        if sess.userauth_agent(&cfg.user).is_ok() && sess.authenticated() {
            authenticated = true;
        }
    }

    // 4. Fallback: try default OpenSSH keys (~/.ssh/id_ed25519, id_ecdsa, id_rsa, id_dsa)
    if !authenticated {
        for p in candidate_ssh_key_paths(cfg.key_path.as_deref()) {
            if p.exists() {
                if sess.userauth_pubkey_file(&cfg.user, None, &p, None).is_ok() && sess.authenticated() {
                    authenticated = true;
                    break;
                }
            }
        }
    }

    if !authenticated {
        return Err("SSH authentication failed (invalid password or key)".to_string());
    }

    let sftp = sess.sftp().map_err(|e| format!("Failed to initialize SFTP subsystem: {}", e))?;
    Ok((sess, sftp))
}

fn load_remote_id_cache(
    sftp: &ssh2::Sftp,
    default_user: &str,
) -> (HashMap<u32, String>, HashMap<u32, String>) {
    let mut users = HashMap::new();
    let mut groups = HashMap::new();

    users.insert(0, "root".to_string());
    groups.insert(0, "root".to_string());

    if let Ok(mut f) = sftp.open(Path::new("/etc/passwd")) {
        let mut buf = Vec::new();
        if (&mut f).take(65536).read_to_end(&mut buf).is_ok() {
            let content = String::from_utf8_lossy(&buf);
            for line in content.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 4 {
                    let uname = parts[0].to_string();
                    if let Ok(uid) = parts[2].parse::<u32>() {
                        users.insert(uid, uname.clone());
                    }
                    if let Ok(gid) = parts[3].parse::<u32>() {
                        groups.entry(gid).or_insert(uname);
                    }
                }
            }
        }
    }

    if let Ok(mut f) = sftp.open(Path::new("/etc/group")) {
        let mut buf = Vec::new();
        if (&mut f).take(65536).read_to_end(&mut buf).is_ok() {
            let content = String::from_utf8_lossy(&buf);
            for line in content.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    let gname = parts[0].to_string();
                    if let Ok(gid) = parts[2].parse::<u32>() {
                        groups.insert(gid, gname);
                    }
                }
            }
        }
    }

    if !default_user.is_empty() {
        // Ensure default user name is present as a fallback
        if !users.values().any(|v| v == default_user) {
            users.insert(1000, default_user.to_string());
        }
    }

    (users, groups)
}

fn read_sftp_dir(
    sftp: &ssh2::Sftp,
    dir_path: &str,
    default_user: &str,
    users: &HashMap<u32, String>,
    groups: &HashMap<u32, String>,
) -> Result<Vec<FileEntry>, String> {
    let p = Path::new(dir_path);
    let list = sftp.readdir(p).map_err(|e| format!("Readdir error: {}", e))?;

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for (entry_path, stat) in list {
        let name = entry_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| entry_path.to_string_lossy().to_string());

        if name == "." || name == ".." {
            continue;
        }

        let is_dir = stat.is_dir();
        let is_hidden = name.starts_with('.');
        let size = stat.size.unwrap_or(0);
        let modified = stat
            .mtime
            .map(|t| UNIX_EPOCH + Duration::from_secs(t));

        let permissions = format_mode(stat.perm.unwrap_or(0), is_dir);

        let owner = match stat.uid {
            Some(0) => "root".to_string(),
            Some(u) => users.get(&u).cloned().unwrap_or_else(|| {
                if !default_user.is_empty() {
                    default_user.to_string()
                } else {
                    u.to_string()
                }
            }),
            None => {
                if !default_user.is_empty() {
                    default_user.to_string()
                } else {
                    "-".to_string()
                }
            }
        };

        let group = match stat.gid {
            Some(0) => "root".to_string(),
            Some(g) => groups.get(&g).cloned().unwrap_or_else(|| {
                if !default_user.is_empty() {
                    default_user.to_string()
                } else {
                    g.to_string()
                }
            }),
            None => {
                if !default_user.is_empty() {
                    default_user.to_string()
                } else {
                    "-".to_string()
                }
            }
        };

        let item = FileEntry {
            name,
            path: entry_path,
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

    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    dirs.extend(files);
    Ok(dirs)
}

fn format_mode(perm: u32, is_dir: bool) -> String {
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
        s.push(if (perm & bit) != 0 { ch } else { '-' });
    }
    s
}

fn fetch_remote_disk(sess: &ssh2::Session, dir_path: &str) -> Result<(u64, u64, f32), String> {
    let mut ch = sess.channel_session().map_err(|e| format!("Channel error: {}", e))?;
    let cmd = format!("df -k '{}' 2>/dev/null | tail -1", dir_path);
    ch.exec(&cmd).map_err(|e| format!("Exec error: {}", e))?;

    let mut out = String::new();
    let _ = ch.read_to_string(&mut out);
    let _ = ch.wait_close();

    // Parse df output: Filesystem 1K-blocks Used Available Use% Mounted
    let parts: Vec<&str> = out.split_whitespace().collect();
    if parts.len() >= 5 {
        if let (Ok(total_k), Ok(used_k)) = (parts[1].parse::<u64>(), parts[2].parse::<u64>()) {
            let total = total_k * 1024;
            let used = used_k * 1024;
            let pct = if total > 0 {
                (used as f64 / total as f64 * 100.0) as f32
            } else {
                0.0
            };
            return Ok((used, total, pct));
        }
    }

    Ok((8_589_934_592, 107_374_182_400, 8.0))
}
